using System.Diagnostics;
using System.Text.RegularExpressions;
using System.IO;
using System.Reflection;

namespace ESP32_Manager.backend
{
    class PioProjectHandler
    {
        string _pioProjectPath;

        public PioProjectHandler()
        {
            string exeDirectory = Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location)
                ?? throw new InvalidOperationException("Exe-Pfad nicht gefunden.");
            _pioProjectPath = Path.Combine(exeDirectory, "esp_ota_init");
        }

        public async Task<(string firmwarePath, string bootloaderPath, string partitionsPath)> CreateBinAsync(string projectName, string environment, IProgress<string> progress)
        {
            // Überprüfen, ob projectName gültig ist
            if (!IsValidBuildFlag(projectName))
            {
                throw new ArgumentException("Ungültiger Projektname als Build-Flag.");
            }

            // Define the build flag
            string buildFlag = $"-DPROJECT_NAME=\\\"{projectName}\\\"";

            // Create the process start info
            ProcessStartInfo startInfo = new ProcessStartInfo
            {
                FileName = "pio",
                Arguments = $"run -d \"{_pioProjectPath}\" -e {environment}",
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true
            };

            // Set the environment variable
            startInfo.EnvironmentVariables["PLATFORMIO_BUILD_FLAGS"] = buildFlag;

            // Start the process
            using (Process process = new Process { StartInfo = startInfo })
            {
                process.OutputDataReceived += (sender, e) =>
                {
                    if (!string.IsNullOrEmpty(e.Data))
                    {
                        progress.Report("Output: " + e.Data);
                    }
                };

                process.ErrorDataReceived += (sender, e) =>
                {
                    if (!string.IsNullOrEmpty(e.Data))
                    {
                        progress.Report("Error: " + e.Data);
                    }
                };

                process.Start();
                process.BeginOutputReadLine();
                process.BeginErrorReadLine();
                await process.WaitForExitAsync();

                // Handle the output and error
                if (process.ExitCode == 0)
                {
                    progress.Report("Build successful:");

                    // Assuming the output binary is in the .pio/build/{environment} directory
                    string firmwarePath = Path.Combine(_pioProjectPath, ".pio", "build", environment, "firmware.bin");
                    string bootloaderPath = Path.Combine(_pioProjectPath, ".pio", "build", environment, "bootloader.bin");
                    string partitionsPath = Path.Combine(_pioProjectPath, ".pio", "build", environment, "partitions.bin");

                    if (File.Exists(firmwarePath) && File.Exists(bootloaderPath) && File.Exists(partitionsPath))
                    {
                        return (firmwarePath, bootloaderPath, partitionsPath);
                    }
                    else
                    {
                        throw new FileNotFoundException("Eine oder mehrere erstellte Binärdateien wurden nicht gefunden.");
                    }
                }
                else
                {
                    progress.Report("Build failed:");
                    throw new Exception("Build process failed.");
                }
            }
        }


        public async Task FlashBinAsync(byte[] firmwareData, byte[] bootloaderData, byte[] partitionsData, string port, IProgress<string> progress)
        {
            // Temporäre Dateien erstellen
            string tempFirmwarePath = Path.GetTempFileName();
            string tempBootloaderPath = Path.GetTempFileName();
            string tempPartitionsPath = Path.GetTempFileName();
            try
            {
                await File.WriteAllBytesAsync(tempFirmwarePath, firmwareData);
                await File.WriteAllBytesAsync(tempBootloaderPath, bootloaderData);
                await File.WriteAllBytesAsync(tempPartitionsPath, partitionsData);

                // Create the process start info
                ProcessStartInfo startInfo = new ProcessStartInfo
                {
                    FileName = "esptool",
                    Arguments = $"--chip esp32s3 --port {port} --baud 115200 --before default_reset " +
                        $"--after hard_reset write_flash --erase-all -z --flash_mode dio --flash_size detect " +
                        $"--flash_freq 40m 0x0 \"{tempBootloaderPath}\" 0x8000 \"{tempPartitionsPath}\" 0x10000 \"{tempFirmwarePath}\"",
                    RedirectStandardOutput = true,
                    RedirectStandardError = true,
                    UseShellExecute = false,
                    CreateNoWindow = true
                };

                // Start the process
                using (Process process = new Process { StartInfo = startInfo })
                {
                    process.OutputDataReceived += (sender, e) =>
                    {
                        if (!string.IsNullOrEmpty(e.Data))
                        {
                            progress.Report("Output: " + e.Data);
                        }
                    };

                    process.ErrorDataReceived += (sender, e) =>
                    {
                        if (!string.IsNullOrEmpty(e.Data))
                        {
                            progress.Report("Error: " + e.Data);
                        }
                    };

                    process.Start();
                    process.BeginOutputReadLine();
                    process.BeginErrorReadLine();
                    await process.WaitForExitAsync();

                    // Handle the output and error
                    if (process.ExitCode == 0)
                    {
                        progress.Report("Flash successful:");
                    }
                    else
                    {
                        progress.Report("Flash failed:");
                        throw new Exception("Flash process failed.");
                    }
                }
            }
            finally
            {
                // Temporäre Dateien löschen
                if (File.Exists(tempFirmwarePath))
                {
                    File.Delete(tempFirmwarePath);
                }
                if (File.Exists(tempBootloaderPath))
                {
                    File.Delete(tempBootloaderPath);
                }
                if (File.Exists(tempPartitionsPath))
                {
                    File.Delete(tempPartitionsPath);
                }
            }
        }



        private string GetChipType(string environment)
        {
            string iniFilePath = Path.Combine(_pioProjectPath, "platformio.ini");
            if (!File.Exists(iniFilePath))
            {
                throw new FileNotFoundException("Die platformio.ini Datei wurde nicht gefunden.");
            }

            var lines = File.ReadAllLines(iniFilePath);
            bool inEnvironmentSection = false;
            foreach (var line in lines)
            {
                if (line.Trim().StartsWith($"[env:{environment}]"))
                {
                    inEnvironmentSection = true;
                }
                else if (inEnvironmentSection && line.Trim().StartsWith("board"))
                {
                    string board = line.Split('=')[1].Trim();
                    return GetChipTypeFromBoard(board);
                }
                else if (line.Trim().StartsWith("[env:") && inEnvironmentSection)
                {
                    // Ende der aktuellen Umgebung
                    break;
                }
            }

            throw new InvalidOperationException("Chip-Typ konnte nicht aus der platformio.ini extrahiert werden.");
        }

        private string GetChipTypeFromBoard(string board)
        {
            // Hier können Sie eine Zuordnung von Board zu Chip-Typ hinzufügen
            // Dies ist ein Beispiel, Sie müssen die tatsächlichen Zuordnungen hinzufügen
            var boardToChipMap = new Dictionary<string, string>
            {
                { "esp32dev", "esp32" },
                { "esp32s3dev", "esp32s3" },
                // Fügen Sie weitere Zuordnungen hinzu
            };

            if (boardToChipMap.TryGetValue(board, out string chip))
            {
                return chip;
            }

            throw new InvalidOperationException($"Unbekanntes Board: {board}");
        }



        private bool IsValidBuildFlag(string flag)
        {
            // Überprüfen, ob der Flag-Name gültig ist (nur alphanumerische Zeichen und Unterstriche)
            return Regex.IsMatch(flag, @"^[a-zA-Z_][a-zA-Z0-9_]*$");
        }

        public List<string> GetEnvironments()
        {
            string iniFilePath = Path.Combine(_pioProjectPath, "platformio.ini");
            var environments = new List<string>();

            if (File.Exists(iniFilePath))
            {
                var lines = File.ReadAllLines(iniFilePath);
                foreach (var line in lines)
                {
                    if (line.StartsWith("[env:"))
                    {
                        var envName = line.Substring(5, line.Length - 6);
                        environments.Add(envName);
                    }
                }
            }
            else
            {
                // print iniFilePath
                Console.WriteLine(iniFilePath);
                throw new FileNotFoundException("Die platformio.ini Datei wurde nicht gefunden.");
            }

            return environments;
        }
    }
}
