using Makaretu.Dns;
using System;
using System.Net;
using System.Net.NetworkInformation;

namespace ESP32_Manager.backend
{
    public sealed class MdnsHandler : IDisposable
    {
        private MulticastService _multicastService;
        private ServiceDiscovery _serviceDiscovery;

        /// <summary>
        /// Startet die mDNS-Ankündigung mit dem Hostnamen "ESP32_Manager.local".
        /// </summary>
        public void StartMdns()
        {
            if (_multicastService == null)
            {
                _multicastService = new MulticastService();
                _multicastService.NetworkInterfaceDiscovered += (s, e) => { /* Optionales Logging */ };
                _multicastService.Start();

                _serviceDiscovery = new ServiceDiscovery(_multicastService);
                var serviceProfile = new ServiceProfile("ESP32_Manager", "_udp", 0);
                _serviceDiscovery.Advertise(serviceProfile);
            }
        }

        /// <summary>
        /// Beendet die mDNS-Ankündigung.
        /// </summary>
        public void StopMdns()
        {
            _serviceDiscovery?.Dispose();
            _serviceDiscovery = null;

            _multicastService?.Stop();
            _multicastService?.Dispose();
            _multicastService = null;
        }

        public void Dispose()
        {
            StopMdns();
        }
    }
}
