using System;
using System.Net;
using System.Net.Sockets;
using System.Printing;
using System.Text;
using System.Text.Json.Nodes;
using System.Text.Json;
using System.Threading.Tasks;

namespace ESP32_Manager.backend
{
    class RemoteAccess : IDisposable
    {
        private UdpClient udpClient;
        private TcpClient tcpClient;
        private Action<string> updateTcpTextfield;
        private Action<string> updateUdpTextfield;
        private Action<List<string>> updateStartOptions;
        private Action<List<(string Name, UInt32 Value)>> updateChangeableVariables;
        private Action<(string Name, string Value)> incommingVariableInfo;
        private IPAddress ipAddress;
        private ushort _udpPort;
        private ushort _tcpPort;
        private string protocol;
        private CancellationTokenSource cts = new CancellationTokenSource();
        private StringBuilder _tcpBuffer = new StringBuilder();
        private List<string> _functions = new List<string>();
        private List<(string Name, UInt32 Value)> _variables = new List<(string Name, UInt32 Value)>();
        public string SelectedStartOption { get; set; } = "";
        public bool AutoStart { get; set; } = false;


        public RemoteAccess(Action<string> updateUdpTextfield, Action<string> updateTcpTextfield, IPAddress ipAddress, ushort udpPort, ushort tcpPort, string protocol, Action<List<string>> updateStartOptions, Action<List<(string Name, UInt32 Value)>> updateChangeableVariables, Action<(string Name, string Value)> incommingVariableInfo)
        {
            this.ipAddress = ipAddress;
            this._udpPort = udpPort;
            this._tcpPort = tcpPort;
            this.protocol = protocol;

            // UDP wie zuvor binden
            udpClient = new UdpClient(new IPEndPoint(ipAddress, udpPort));

            // Für TCP keinen Port fest binden, sondern nur das Ziel bei Bedarf ansteuern
            tcpClient = new TcpClient();

            this.updateTcpTextfield = updateTcpTextfield;
            this.updateUdpTextfield = updateUdpTextfield;
            this.updateStartOptions = updateStartOptions;
            this.updateChangeableVariables = updateChangeableVariables;
            this.incommingVariableInfo = incommingVariableInfo;
        }

        public async Task StartUdpListener()
        {
            updateUdpTextfield?.Invoke("starting listener");
            while (!cts.Token.IsCancellationRequested)
            {
                try
                {
                    var result = await udpClient.ReceiveAsync(cts.Token);
                    string receivedData = Encoding.UTF8.GetString(result.Buffer);

                    // Prüfen, ob keine TCP-Verbindung besteht
                    if (!tcpClient.Connected)
                    {
                        await ConnectTcp();
                    }

                    // Nach JSON-formatierten Variablen suchen
                    ParseVariableUpdates(receivedData);

                    // Trotzdem den kompletten Text anzeigen
                    updateUdpTextfield?.Invoke(receivedData);
                }
                catch (OperationCanceledException)
                {
                    // Abgebrochen
                    break;
                }
                catch (Exception ex)
                {
                    updateUdpTextfield?.Invoke($"Error: {ex.Message}");
                    break;
                }
            }
        }


        private void ParseVariableUpdates(string input)
        {
            if (string.IsNullOrEmpty(input) || incommingVariableInfo == null)
                return;

            // Regulärer Ausdruck zum Finden von JSON-Objekten im exakten Format {"variableName":"VariableValue"}
            var regex = new System.Text.RegularExpressions.Regex(@"\{""([^""]+)""\s*:\s*""([^""]+)""\}");
            var matches = regex.Matches(input);

            foreach (System.Text.RegularExpressions.Match match in matches)
            {
                if (match.Groups.Count >= 3)
                {
                    string name = match.Groups[1].Value.Trim();
                    string value = match.Groups[2].Value.Trim();

                    // Gefundenes Muster an die GUI-Update-Action weiterleiten
                    incommingVariableInfo?.Invoke((name, value));
                    updateUdpTextfield?.Invoke($"[UDP] Variable erkannt: {name} = {value}");
                }
            }
        }




        public async Task ConnectTcp()
        {
            try
            {
                // Bei bestehender Verbindung nichts tun
                if (tcpClient != null && tcpClient.Connected)
                    return;

                // Vorherigen Client ordnungsgemäß schließen, falls vorhanden
                if (tcpClient != null)
                {
                    try
                    {
                        tcpClient.Close();
                        tcpClient.Dispose();
                    }
                    catch (Exception ex)
                    {
                        updateTcpTextfield?.Invoke($"Warnung beim Schließen der vorherigen Verbindung: {ex.Message}");
                    }
                }

                // Neuen Client instanzieren
                tcpClient = new TcpClient();

                // Timeout-Option setzen, damit Verbindungsversuche nicht zu lange dauern
                tcpClient.SendTimeout = 5000;
                tcpClient.ReceiveTimeout = 5000;

                var remoteIp = new IPAddress(new byte[] { 192, 168, 43, 75 });
                updateTcpTextfield?.Invoke("Versuche, eine TCP-Verbindung herzustellen...");

                // CancellationToken für den Verbindungsversuch erstellen
                using var timeoutCts = new CancellationTokenSource(TimeSpan.FromSeconds(5));

                try
                {
                    await tcpClient.ConnectAsync(remoteIp, _tcpPort).WaitAsync(timeoutCts.Token);

                    if (tcpClient.Connected)
                    {
                        updateTcpTextfield?.Invoke($"Verbunden mit {remoteIp}:{_tcpPort}");
                        if (!string.IsNullOrEmpty(SelectedStartOption) && AutoStart)
                        {
                            SendStartOption();
                        }
                    }
                    else
                    {
                        updateTcpTextfield?.Invoke("Verbindung fehlgeschlagen");
                    }
                }
                catch (OperationCanceledException)
                {
                    updateTcpTextfield?.Invoke("Verbindungsversuch nach Zeitüberschreitung abgebrochen");
                    // Client bereinigen
                    tcpClient.Close();
                    tcpClient.Dispose();
                    tcpClient = new TcpClient();
                }
            }
            catch (Exception ex)
            {
                updateTcpTextfield?.Invoke($"Fehler beim Verbinden: {ex.Message}");

                // Client bei Fehler ebenfalls bereinigen
                try
                {
                    if (tcpClient != null)
                    {
                        tcpClient.Close();
                        tcpClient.Dispose();
                        tcpClient = new TcpClient();
                    }
                }
                catch { /* Ignorieren */ }
            }
        }




        public async Task StartTcpListener()
        {
            updateTcpTextfield?.Invoke("Starting TCP listener");
            int consecutiveErrors = 0;

            while (!cts.Token.IsCancellationRequested)
            {
                try
                {
                    // Versuche, eine Verbindung herzustellen, falls keine besteht
                    if (tcpClient == null || !tcpClient.Connected)
                    {
                        await ConnectTcp();

                        // Wenn die Verbindung fehlgeschlagen ist, längere Pause vor dem nächsten Versuch
                        if (!tcpClient.Connected)
                        {
                            // Exponentielles Backoff für wiederholte Fehler
                            int delayMs = Math.Min(1000 * (int)Math.Pow(2, consecutiveErrors), 30000);
                            updateTcpTextfield?.Invoke($"Warte {delayMs / 1000} Sekunden vor dem nächsten Versuch...");
                            await Task.Delay(delayMs, cts.Token);
                            consecutiveErrors++;
                            continue;
                        }

                        // Erfolgreiche Verbindung, Fehler zurücksetzen
                        consecutiveErrors = 0;

                        // Kurze Pause, um der Verbindung Zeit zu geben
                        await Task.Delay(500, cts.Token);
                    }

                    // Rest der Methode bleibt gleich...
                    using (NetworkStream networkStream = tcpClient.GetStream())
                    {
                        // Timeout für Operationen setzen
                        networkStream.ReadTimeout = 10000;
                        networkStream.WriteTimeout = 10000;

                        byte[] buffer = new byte[1024];
                        int bytesRead;

                        while (!cts.Token.IsCancellationRequested &&
                              (bytesRead = await networkStream.ReadAsync(buffer, 0, buffer.Length, cts.Token)) != 0)
                        {
                            // Neuen Chunk an Puffer anhängen
                            string chunk = Encoding.UTF8.GetString(buffer, 0, bytesRead);
                            updateTcpTextfield?.Invoke(chunk);
                            _tcpBuffer.Append(chunk);

                            // Mehrfachversuch, komplette JSONs aus dem Puffer zu ziehen
                            while (TryExtractCompleteJson(_tcpBuffer, out string jsonDocument))
                            {
                                updateTcpTextfield?.Invoke($"[TCP] Komplette JSON: {jsonDocument}");
                                // Verarbeiten der JSON
                                handleIncommingTcp(jsonDocument);
                            }
                        }
                    }

                    // Wenn wir hier landen, wurde die Verbindung geschlossen
                    updateTcpTextfield?.Invoke("TCP Verbindung wurde getrennt. Versuche erneut zu verbinden...");
                    await Task.Delay(1000, cts.Token); // Kurze Pause vor Neuverbindung
                }
                catch (OperationCanceledException)
                {
                    break;
                }
                catch (Exception ex)
                {
                    updateTcpTextfield?.Invoke($"Error: {ex.Message}");
                    consecutiveErrors++;
                    // Längere Pause vor Neuversuch bei wiederholten Fehlern
                    int delayMs = Math.Min(1000 * (int)Math.Pow(2, consecutiveErrors), 30000);
                    await Task.Delay(delayMs, cts.Token);
                }
            }
        }


        /// <summary>
        /// Liest aus dem StringBuilder fortlaufend JSON-Objekte ({} oder []),
        /// indem eine Klammer-Logik genutzt wird. Liefert true, wenn eines extrahiert wurde.
        /// </summary>
        private bool TryExtractCompleteJson(StringBuilder buffer, out string jsonDocument)
        {
            jsonDocument = null;
            string text = buffer.ToString();

            // Überspringen von Leerzeichen am Anfang
            int startIndex = 0;
            while (startIndex < text.Length && char.IsWhiteSpace(text[startIndex]))
                startIndex++;

            if (startIndex >= text.Length)
                return false;

            char firstChar = text[startIndex];
            // Nur unterstützen wir hier JSON-Objekte {...} oder Arrays [...]
            if (firstChar != '{' && firstChar != '[')
                return false;

            // Klammer-Zählung
            int bracketCount = 0;
            char openBracket = firstChar;
            char closeBracket = (openBracket == '{') ? '}' : ']';

            for (int i = startIndex; i < text.Length; i++)
            {
                if (text[i] == openBracket) bracketCount++;
                else if (text[i] == closeBracket) bracketCount--;

                if (bracketCount == 0)
                {
                    // Hier endet das erste vollständige JSON-Dokument
                    int length = i - startIndex + 1;
                    jsonDocument = text.Substring(startIndex, length);

                    // Nun entfernen wir diesen Teil aus dem Puffer
                    buffer.Remove(0, startIndex + length);
                    return true;
                }
            }

            // Falls noch nicht geschlossen, ist das JSON unvollständig
            return false;
        }



        private void handleIncommingTcp(string receivedData)
        {
            try
            {
                // JSON-Daten dynamisch parsen
                JsonNode jsonNode = JsonNode.Parse(receivedData);
                if (jsonNode != null)
                {
                    handleIncomingTcpJson(jsonNode);
                }
            }
            catch (JsonException ex)
            {
                // String war kein valides JSON
                updateTcpTextfield?.Invoke($"Error: {ex.Message}"); 
            }
        }

        private void handleIncomingTcpJson(JsonNode json)
        {
            // Prüfen, ob ein "changeableVariables"-Array enthalten ist mit der neuen Struktur
            if (json["changeableVariables"] is JsonArray varArray)
            {
                _variables.Clear();
                foreach (var entry in varArray)
                {
                    if (entry is JsonObject varObj)
                    {
                        // Name und Wert aus dem Objekt extrahieren
                        string name = varObj["name"]?.GetValue<string>();
                        UInt32 value = varObj["value"]?.GetValue<UInt32>() ?? 0;

                        if (!string.IsNullOrEmpty(name))
                        {
                            _variables.Add((name, value));
                        }
                    }
                }

                updateTcpTextfield?.Invoke($"changeableVariables empfangen: {string.Join(", ", _variables.Select(v => $"{v.Name}={v.Value}"))}");
                updateChangeableVariables?.Invoke(_variables);
            }

            // Verarbeitung für "startOptions" bleibt unverändert
            if (json["startOptions"] is JsonArray funcArray)
            {
                _functions.Clear();
                foreach (var entry in funcArray)
                {
                    if (entry is JsonValue val && val.TryGetValue<string>(out string strVal))
                    {
                        _functions.Add(strVal);
                    }
                }
                updateTcpTextfield?.Invoke($"startOptions empfangen: {string.Join(",\n ", _functions)}");
                updateStartOptions?.Invoke(_functions);
            }
        }



        public void SendVariable(string variableName, int value)
        {
            try
            {
                // Prüfen, ob TCP-Client verbunden ist
                if (tcpClient == null || !tcpClient.Connected)
                {
                    updateTcpTextfield?.Invoke("Fehler: Keine TCP-Verbindung für das Senden von Variablen");
                    return;
                }

                // JSON mit dem neuen Format erstellen
                var json = new JsonObject
                {
                    ["setVariable"] = new JsonObject
                    {
                        ["name"] = variableName,
                        ["value"] = value
                    }
                };

                // JSON in String umwandeln
                string jsonString = json.ToJsonString();

                updateTcpTextfield?.Invoke($"Sende Variable: {jsonString}");

                // String in Bytes umwandeln und senden
                byte[] buffer = Encoding.UTF8.GetBytes(jsonString);
                tcpClient.GetStream().Write(buffer, 0, buffer.Length);
            }
            catch (Exception ex)
            {
                updateTcpTextfield?.Invoke($"Fehler beim Senden der Variable: {ex.Message}");
            }
        }

        public void SendStartOption()
        {
            try
            {
                // Prüfen, ob TCP-Client verbunden ist
                if (tcpClient == null || !tcpClient.Connected)
                {
                    updateTcpTextfield?.Invoke("Fehler: Keine TCP-Verbindung für das Senden von Startoptionen");
                    return;
                }
                // JSON mit dem neuen Format erstellen
                var json = new JsonObject
                {
                    ["startOption"] = SelectedStartOption
                };
                // JSON in String umwandeln
                string jsonString = json.ToJsonString();
                updateTcpTextfield?.Invoke($"Sende Startoption: {jsonString}");
                // String in Bytes umwandeln und senden
                byte[] buffer = Encoding.UTF8.GetBytes(jsonString);
                tcpClient.GetStream().Write(buffer, 0, buffer.Length);
            }
            catch (Exception ex)
            {
                updateTcpTextfield?.Invoke($"Fehler beim Senden der Startoption: {ex.Message}");
            }
        }

        public void SendReset()
        {
            try
            {
                // Prüfen, ob TCP-Client verbunden ist
                if (tcpClient == null || !tcpClient.Connected)
                {
                    updateTcpTextfield?.Invoke("Fehler: Keine TCP-Verbindung für das Senden eines Resets");
                    return;
                }
                // JSON mit dem neuen Format erstellen
                var json = new JsonObject
                {
                    ["reset"] = true
                };
                // JSON in String umwandeln
                string jsonString = json.ToJsonString();
                updateTcpTextfield?.Invoke($"Sende Reset: {jsonString}");
                // String in Bytes umwandeln und senden
                byte[] buffer = Encoding.UTF8.GetBytes(jsonString);
                tcpClient.GetStream().Write(buffer, 0, buffer.Length);
            }
            catch (Exception ex)
            {
                updateTcpTextfield?.Invoke($"Fehler beim Senden des Resets: {ex.Message}");
            }
        }

        






        // Destruktor
        ~RemoteAccess()
        {
            Dispose(false);
        }

        // Implementierung von IDisposable
        public void Dispose()
        {
            Dispose(true);
            GC.SuppressFinalize(this);
        }

        protected virtual void Dispose(bool disposing)
        {
            if (disposing)
            {
                cts.Cancel();
                // Freigeben von verwalteten Ressourcen
                udpClient?.Close();
                tcpClient?.Close();
            }
            // Freigeben von nicht verwalteten Ressourcen (falls vorhanden)
        }
    }
}

