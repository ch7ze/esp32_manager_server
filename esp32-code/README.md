# ESP32 OTA Module - PlatformIO Example

This example demonstrates advanced usage of the ESP32 OTA Module with PlatformIO, including remote control capabilities, variable management, and structured logging.

## Features Demonstrated

- **Automatic OTA Updates**: Background task handles OTA updates
- **TCP Remote Control**: JSON-based command interface
- **Variable Management**: Runtime configuration via network
- **Structured Logging**: JSON-formatted variable output
- **Periodic Tasks**: Sensor reading simulation
- **LED Control**: Visual feedback with configurable timing

## Hardware Requirements

- ESP32 development board
- Built-in LED (or external LED on GPIO 2)
- WiFi connection

## Setup Instructions

1. **Configure WiFi Credentials**
   ```cpp
   const char* ESP32_OTA_SSID = "YourWiFiName";
   const char* ESP32_OTA_PASSWORD = "YourWiFiPassword";
   ```

2. **Set UDP Target for Logging**
   ```cpp
   IPAddress ESP32_OTA_UDP_TARGET_IP(192, 168, 1, 100); // Your PC's IP
   ```

3. **Build and Upload**
   ```bash
   pio run --target upload
   ```

4. **Monitor Serial Output**
   ```bash
   pio device monitor
   ```

## Remote Control Usage

### Connect to TCP Server
The ESP32 starts a TCP server on port 3232. Connect using telnet, netcat, or a custom client:

```bash
telnet ESP32_IP_ADDRESS 3232
# or
nc ESP32_IP_ADDRESS 3232
```

### Available Commands

#### Execute Functions
```json
{"startOption":"read_sensors"}
{"startOption":"calibrate"}  
{"startOption":"restart"}
{"startOption":"factory_reset"}
```

#### Change Variables
```json
{"setVariable":{"name":"ledBlinkDelay","value":500}}
{"setVariable":{"name":"sensorReadInterval","value":10000}}
{"setVariable":{"name":"debugLevel","value":2}}
```

#### Reset ESP32
```json
{"reset":true}
```

### Variables You Can Modify

- **ledBlinkDelay** (uint32_t): LED blink interval in milliseconds
- **sensorReadInterval** (uint32_t): Sensor reading interval in milliseconds  
- **debugLevel** (uint32_t): Debug output verbosity (0-3)

## OTA Update Process

### First Upload (Serial)
```bash
pio run --target upload
```

### Subsequent Updates (OTA)
1. Uncomment OTA lines in `platformio.ini`:
   ```ini
   upload_protocol = espota
   upload_port = esp32-ota-advanced.local
   ```

2. Upload over WiFi:
   ```bash
   pio run --target upload
   ```

## Expected Output

```
ESP32 OTA Module - Advanced PlatformIO Example
Connecting to WiFi.....
Connected! IP address: 192.168.1.150
Automatic OTA started
TCP Server started on port 3232
Setup completed!
Available remote commands:
- Start functions: read_sensors, calibrate, restart, factory_reset
- Variables: ledBlinkDelay, sensorReadInterval, debugLevel
```

## Troubleshooting

### WiFi Connection Issues
- Verify SSID and password
- Check WiFi signal strength
- Ensure 2.4GHz network (ESP32 doesn't support 5GHz)

### OTA Update Issues  
- Ensure ESP32 and computer are on same network
- Check hostname resolution
- Try IP address instead of hostname

### TCP Connection Issues
- Verify ESP32 IP address from Serial Monitor
- Check firewall settings on your computer
- Ensure port 3232 is not blocked

## Extension Ideas

- Add web interface for easier control
- Implement MQTT for IoT integration
- Add sensor drivers for real hardware
- Create mobile app for remote control
- Add file system management via OTA