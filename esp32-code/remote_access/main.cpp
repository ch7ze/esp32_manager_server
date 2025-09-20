#include <ESP32_OTA_Module.h>
#include <WiFi.h>
#include <esp32_configurable_streambuf.h>

// WiFi Configuration - Change these values
const char* ESP32_OTA_SSID = "Raspberry";
const char* ESP32_OTA_PASSWORD = "55555555";

// UDP Configuration for remote logging
IPAddress ESP32_OTA_UDP_TARGET_IP(192, 168, 137, 1); // Your PC's IP address
const int ESP32_OTA_UDP_PORT = 3232;



// Hostname for OTA
const char* ESP32_OTA_HOSTNAME = "esp32-ota-advanced";

// Example variables for remote control
uint32_t ledBlinkDelay = 1000;
uint32_t sensorReadInterval = 5000;
uint32_t debugLevel = 1;

// Example function for scheduled task
void readSensors() {
    std::cout << "Reading sensors..." << std::endl;
    
    // Simulate sensor readings
    float temperature = 25.5 + random(-50, 51) / 10.0;
    float humidity = 60.0 + random(-100, 101) / 10.0;
    
    std::cout << ota::makeVarInfo("temperature", temperature) << std::endl;
    std::cout << ota::makeVarInfo("humidity", humidity) << std::endl;
}

void blinkLED() {
    static bool ledState = false;
    static unsigned long lastBlink = 0;
    
    if (millis() - lastBlink > ledBlinkDelay) {
        ledState = !ledState;
        digitalWrite(LED_BUILTIN, ledState);
        lastBlink = millis();
        
        if (debugLevel >= 2) {
            std::cout << "LED: " << (ledState ? "ON" : "OFF") << std::endl;
        }
    }
}

void performCalibration() {
    std::cout << "Starting calibration sequence..." << std::endl;
    
    for (int i = 1; i <= 5; i++) {
        std::cout << "Calibration step " << i << "/5" << std::endl;
        delay(1000);
    }
    
    std::cout << "Calibration completed!" << std::endl;
}

void setup() {
    Serial.begin(115200);
    delay(1000);
    
    pinMode(LED_BUILTIN, OUTPUT);

    neopixelWrite(LED_BUILTIN, 255, 0, 0); // Red during setup
    
    // Configure cout to use configurable streambuf for Serial and UDP output
    cs::configurableStreambuf.addOutputFunction(cs::serialOut);
    std::cout.rdbuf(&cs::configurableStreambuf);
    
    std::cout << "ESP32 OTA Module - Advanced PlatformIO Example" << std::endl;
    
    // Connect to WiFi
    WiFi.begin(ESP32_OTA_SSID, ESP32_OTA_PASSWORD);
    std::cout << "Connecting to WiFi";
    
    while (WiFi.status() != WL_CONNECTED) {
        delay(500);
        std::cout << ".";
    }

    neopixelWrite(LED_BUILTIN, 0, 255, 0); // Green on successful connection
    
    std::cout << std::endl << "Connected! IP address: " << WiFi.localIP().toString().c_str() << std::endl;
    

    // Setup OTA with automatic handling in background task
    ota::startAutomaticOTA();
    std::cout << "Automatic OTA started" << std::endl;
    
    // Start TCP server
    ota::tcpServer.begin();
    std::cout << "TCP Server started on port 3232" << std::endl;

    cs::configurableStreambuf.addOutputFunction(ota::udpOut);
    std::cout << "UDP output configured for " << ESP32_OTA_UDP_TARGET_IP.toString().c_str() << ":" << ESP32_OTA_UDP_PORT << std::endl;
    
    // Configure start options for remote control
    ota::tcpServer.addStartOption("read_sensors", readSensors);
    ota::tcpServer.addStartOption("calibrate", performCalibration);
    ota::tcpServer.addStartOption("restart", []() {
        std::cout << "Restarting ESP32..." << std::endl;
        delay(1000);
        ESP.restart();
    });
    ota::tcpServer.addStartOption("factory_reset", []() {
        std::cout << "Factory reset requested!" << std::endl;
        // Add your factory reset logic here
    });
    
    // Configure changeable variables
    ota::tcpServer.addChangeableVariable("ledBlinkDelay", ledBlinkDelay);
    ota::tcpServer.addChangeableVariable("sensorReadInterval", sensorReadInterval);
    ota::tcpServer.addChangeableVariable("debugLevel", debugLevel);
    
    std::cout << "Setup completed!" << std::endl;
    std::cout << "Available remote commands:" << std::endl;
    std::cout << "- Start functions: read_sensors, calibrate, restart, factory_reset" << std::endl;
    std::cout << "- Variables: ledBlinkDelay, sensorReadInterval, debugLevel" << std::endl;
    std::cout << "Example JSON: {\"startOption\":\"read_sensors\"}" << std::endl;
    std::cout << "Example JSON: {\"setVariable\":{\"name\":\"ledBlinkDelay\",\"value\":500}}" << std::endl;
}

void loop() {
    static unsigned long lastSensorRead = 0;
    static unsigned long lastStatus = 0;
    
    // Blink LED with configurable delay
    blinkLED();
    
    // Read sensors periodically
    if (millis() - lastSensorRead > sensorReadInterval) {
        readSensors();
        lastSensorRead = millis();
    }
    
    // Print status every 30 seconds
    if (millis() - lastStatus > 30000) {
        std::cout << "Status: " << ota::tcpServer.getClientCount() << " clients connected" << std::endl;
        std::cout << ota::vInfo(WiFi.localIP().toString()) << std::endl;
        std::cout << ota::vInfo(ESP.getFreeHeap()) << std::endl;
        lastStatus = millis();
    }
    
    // Check for selected start option from remote
    std::function<void()> selectedFunction;
    if (ota::tcpServer.getSelectedStartOption(selectedFunction)) {
        std::cout << "Executing remote function..." << std::endl;
        selectedFunction();
    }
    
    delay(100);
}