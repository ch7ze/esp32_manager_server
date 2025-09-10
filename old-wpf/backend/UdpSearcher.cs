using System;
using System.Collections.Generic;
using System.Net;
using System.Net.Sockets;
using System.Threading.Tasks;
using System.Timers;

namespace ESP32_Manager.backend
{
    class UdpSearcher
    {
        private string ipAddress = "192.168.1.74";
        private System.Timers.Timer checkTimer;
        private List<ushort> availablePorts;

        public UdpSearcher(List<ushort> availablePorts)
        {
            this.availablePorts = availablePorts;
        }

        public void StartCheckingUdpPorts(Action<ushort> portCallback, int interval = 1000)
        {
            checkTimer = new System.Timers.Timer(interval);
            checkTimer.Elapsed += async (sender, e) => await CheckUdpPorts(portCallback);
            checkTimer.AutoReset = true;
            checkTimer.Enabled = true;
        }

        public void StopCheckingUdpPorts()
        {
            if (checkTimer != null)
            {
                checkTimer.Stop();
                checkTimer.Dispose();
                checkTimer = null;
            }
        }

        public void AddPort(ushort port)
        {
            if (!availablePorts.Contains(port))
            {
                availablePorts.Add(port);
            }
        }

        public void RemovePort(ushort port)
        {
            if (availablePorts.Contains(port))
            {
                availablePorts.Remove(port);
            }
        }

        public void ReplacePorts(List<ushort> newPorts)
        {
            availablePorts = newPorts;
        }

        private async Task CheckUdpPorts(Action<ushort> portCallback)
        {
            foreach (var port in new List<ushort>(availablePorts))
            {
                try
                {
                    using (var udpClient = new UdpClient(new IPEndPoint(IPAddress.Parse(ipAddress), port)))
                    {
                        udpClient.Client.ReceiveTimeout = 1000; // 1 Sekunde Timeout
                        var result = await udpClient.ReceiveAsync();
                        if (result.Buffer.Length > 0)
                        {
                            portCallback(port);
                            availablePorts.Remove(port);
                            return;
                        }
                    }
                }
                catch (SocketException ex) when (ex.SocketErrorCode == SocketError.AddressAlreadyInUse)
                {
                    // Port is already in use, continue to the next port
                }
            }
        }
    }
}
