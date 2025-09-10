using System;
using System.Data.SQLite;
using System.IO;
using System.Reflection;

namespace ESP32_Manager.backend
{
    public class DatabaseManager
    {
        private readonly string _databasePath;
        private int _nextTcpPort = 50000;
        private int _nextUdpPort = 60000;

        public DatabaseManager()
        {
            string exeDirectory = Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location)
                ?? throw new InvalidOperationException("Exe-Pfad nicht gefunden.");
            _databasePath = Path.Combine(exeDirectory, "projects.db");
            InitializeDatabase();
        }

        private void InitializeDatabase()
        {
            if (!File.Exists(_databasePath))
            {
                SQLiteConnection.CreateFile(_databasePath);
            }

            using (var connection = new SQLiteConnection($"Data Source={_databasePath};Version=3;"))
            {
                connection.Open();
                string createProjectsTableQuery = @"
                CREATE TABLE IF NOT EXISTS Projects (
                    Id INTEGER PRIMARY KEY AUTOINCREMENT,
                    ProjectName TEXT NOT NULL,
                    FirmwareData BLOB NOT NULL,
                    BootloaderData BLOB NOT NULL,
                    PartitionsData BLOB NOT NULL,
                    TcpPort INTEGER NOT NULL UNIQUE,
                    UdpPort INTEGER NOT NULL UNIQUE
                )";
                using (var command = new SQLiteCommand(createProjectsTableQuery, connection))
                {
                    command.ExecuteNonQuery();
                }

                // Initialisieren Sie die nächsten verfügbaren Ports
                string getMaxPortsQuery = "SELECT MAX(TcpPort) AS MaxTcpPort, MAX(UdpPort) AS MaxUdpPort FROM Projects";
                using (var command = new SQLiteCommand(getMaxPortsQuery, connection))
                {
                    using (var reader = command.ExecuteReader())
                    {
                        if (reader.Read())
                        {
                            if (!reader.IsDBNull(reader.GetOrdinal("MaxTcpPort")))
                            {
                                _nextTcpPort = reader.GetInt32(reader.GetOrdinal("MaxTcpPort")) + 1;
                            }
                            if (!reader.IsDBNull(reader.GetOrdinal("MaxUdpPort")))
                            {
                                _nextUdpPort = reader.GetInt32(reader.GetOrdinal("MaxUdpPort")) + 1;
                            }
                        }
                    }
                }
            }
        }

        public void AddProjectFiles(string projectName, byte[] firmwareData, byte[] bootloaderData, byte[] partitionsData)
        {
            using (var connection = new SQLiteConnection($"Data Source={_databasePath};Version=3;"))
            {
                connection.Open();

                // Überprüfen, ob das Projekt bereits existiert
                string checkQuery = "SELECT COUNT(*) FROM Projects WHERE ProjectName = @ProjectName";
                using (var checkCommand = new SQLiteCommand(checkQuery, connection))
                {
                    checkCommand.Parameters.AddWithValue("@ProjectName", projectName);
                    long count = (long)checkCommand.ExecuteScalar();

                    if (count > 0)
                    {
                        // Aktualisieren Sie das vorhandene Projekt
                        string updateQuery = "UPDATE Projects SET FirmwareData = @FirmwareData, BootloaderData = @BootloaderData, PartitionsData = @PartitionsData WHERE ProjectName = @ProjectName";
                        using (var updateCommand = new SQLiteCommand(updateQuery, connection))
                        {
                            updateCommand.Parameters.AddWithValue("@ProjectName", projectName);
                            updateCommand.Parameters.AddWithValue("@FirmwareData", firmwareData);
                            updateCommand.Parameters.AddWithValue("@BootloaderData", bootloaderData);
                            updateCommand.Parameters.AddWithValue("@PartitionsData", partitionsData);
                            updateCommand.ExecuteNonQuery();
                        }
                    }
                    else
                    {
                        // Fügen Sie das neue Projekt hinzu
                        string insertQuery = "INSERT INTO Projects (ProjectName, FirmwareData, BootloaderData, PartitionsData, TcpPort, UdpPort) VALUES (@ProjectName, @FirmwareData, @BootloaderData, @PartitionsData, @TcpPort, @UdpPort)";
                        using (var command = new SQLiteCommand(insertQuery, connection))
                        {
                            command.Parameters.AddWithValue("@ProjectName", projectName);
                            command.Parameters.AddWithValue("@FirmwareData", firmwareData);
                            command.Parameters.AddWithValue("@BootloaderData", bootloaderData);
                            command.Parameters.AddWithValue("@PartitionsData", partitionsData);
                            command.Parameters.AddWithValue("@TcpPort", _nextTcpPort);
                            command.Parameters.AddWithValue("@UdpPort", _nextUdpPort);
                            command.ExecuteNonQuery();
                        }

                        // Inkrementieren Sie die Ports für das nächste Projekt
                        _nextTcpPort++;
                        _nextUdpPort++;
                    }
                }
            }
        }

        public (byte[] firmwareData, byte[] bootloaderData, byte[] partitionsData) GetProjectFiles(string projectName)
        {
            using (var connection = new SQLiteConnection($"Data Source={_databasePath};Version=3;"))
            {
                connection.Open();
                string selectQuery = "SELECT FirmwareData, BootloaderData, PartitionsData FROM Projects WHERE ProjectName = @ProjectName";
                using (var command = new SQLiteCommand(selectQuery, connection))
                {
                    command.Parameters.AddWithValue("@ProjectName", projectName);
                    using (var reader = command.ExecuteReader())
                    {
                        if (reader.Read())
                        {
                            byte[] firmwareData = (byte[])reader["FirmwareData"];
                            byte[] bootloaderData = (byte[])reader["BootloaderData"];
                            byte[] partitionsData = (byte[])reader["PartitionsData"];
                            return (firmwareData, bootloaderData, partitionsData);
                        }
                    }
                }
            }
            throw new Exception("Projekt nicht gefunden.");
        }

        public void GetProjects()
        {
            using (var connection = new SQLiteConnection($"Data Source={_databasePath};Version=3;"))
            {
                connection.Open();
                string selectQuery = "SELECT * FROM Projects";
                using (var command = new SQLiteCommand(selectQuery, connection))
                {
                    using (var reader = command.ExecuteReader())
                    {
                        while (reader.Read())
                        {
                            string projectName = reader["ProjectName"].ToString();
                            byte[] fileData = (byte[])reader["FileData"];
                            Console.WriteLine($"Id: {reader["Id"]}, ProjectName: {projectName}, FileData Length: {fileData.Length}");
                        }
                    }
                }
            }
        }

        public List<string> getProjectNames()
        {
            List<string> projectNames = new List<string>();

            using (var connection = new SQLiteConnection($"Data Source={_databasePath};Version=3;"))
            {
                connection.Open();
                string selectQuery = "SELECT ProjectName FROM Projects";
                using (var command = new SQLiteCommand(selectQuery, connection))
                {
                    using (var reader = command.ExecuteReader())
                    {
                        while (reader.Read())
                        {
                            projectNames.Add(reader["ProjectName"].ToString());
                        }
                    }
                }
            }

            return projectNames;
        }

        public byte[] getBinData(string projectName)
        {
            byte[] fileData = null;
            using (var connection = new SQLiteConnection($"Data Source={_databasePath};Version=3;"))
            {
                connection.Open();
                string selectQuery = "SELECT FileData FROM Projects WHERE ProjectName = @ProjectName";
                using (var command = new SQLiteCommand(selectQuery, connection))
                {
                    command.Parameters.AddWithValue("@ProjectName", projectName);
                    using (var reader = command.ExecuteReader())
                    {
                        if (reader.Read())
                        {
                            fileData = (byte[])reader["FileData"];
                        }
                    }
                }
            }
            return fileData;
        }

        public (int tcpPort, int udpPort) GetProjectPorts(string projectName)
        {
            using (var connection = new SQLiteConnection($"Data Source={_databasePath};Version=3;"))
            {
                connection.Open();
                string selectQuery = "SELECT TcpPort, UdpPort FROM Projects WHERE ProjectName = @ProjectName";
                using (var command = new SQLiteCommand(selectQuery, connection))
                {
                    command.Parameters.AddWithValue("@ProjectName", projectName);
                    using (var reader = command.ExecuteReader())
                    {
                        if (reader.Read())
                        {
                            int tcpPort = reader.GetInt32(reader.GetOrdinal("TcpPort"));
                            int udpPort = reader.GetInt32(reader.GetOrdinal("UdpPort"));
                            return (tcpPort, udpPort);
                        }
                    }
                }
            }
            throw new Exception("Projekt nicht gefunden.");
        }

        public string GetProjectNameByUdpPort(ushort udpPort)
        {
            using (var connection = new SQLiteConnection($"Data Source={_databasePath};Version=3;"))
            {
                connection.Open();
                string selectQuery = "SELECT ProjectName FROM Projects WHERE UdpPort = @UdpPort";
                using (var command = new SQLiteCommand(selectQuery, connection))
                {
                    command.Parameters.AddWithValue("@UdpPort", udpPort);
                    using (var reader = command.ExecuteReader())
                    {
                        if (reader.Read())
                        {
                            return reader["ProjectName"].ToString();
                        }
                    }
                }
            }
            throw new Exception("Projekt nicht gefunden.");
        }

        public List<ushort> GetAllUdpPorts()
        {
            List<ushort> udpPorts = new List<ushort>();

            using (var connection = new SQLiteConnection($"Data Source={_databasePath};Version=3;"))
            {
                connection.Open();
                string selectQuery = "SELECT UdpPort FROM Projects";
                using (var command = new SQLiteCommand(selectQuery, connection))
                {
                    using (var reader = command.ExecuteReader())
                    {
                        while (reader.Read())
                        {
                            udpPorts.Add((ushort)reader.GetInt32(reader.GetOrdinal("UdpPort")));
                        }
                    }
                }
            }

            return udpPorts;
        }


    }
}
