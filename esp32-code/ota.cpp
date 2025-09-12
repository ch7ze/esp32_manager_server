#include "ota.hpp"



void ota::setupOTA()
{
	MDNS.begin(HOSTNAME);           // mDNS initialisieren
	MDNS.addService("arduino", "tcp", 3232); // Arduino OTA Service registrieren
	MDNS.addServiceTxt("arduino", "tcp", "mac", WiFi.macAddress()); // MAC-Adresse als TXT-Record hinzufügen

	Serial.println("OTA ready");

	// Set hostname for OTA
	ArduinoOTA.setHostname(HOSTNAME);

	// OTA setup
	ArduinoOTA.onStart([]() {
		String type;
		if (ArduinoOTA.getCommand() == U_FLASH) {
		type = "sketch";
		} else { // U_SPIFFS
		type = "filesystem";
		}
		// NOTE: if updating SPIFFS this would be the place to unmount SPIFFS using SPIFFS.end()
		Serial.println("Start updating " + type);
	});

	ArduinoOTA.onEnd([]() {
		Serial.println("\nEnd");
	});

	ArduinoOTA.onProgress([](unsigned int progress, unsigned int total) {
		Serial.printf("Progress: %u%%\r", (progress / (total / 100)));
	});

	ArduinoOTA.onError([](ota_error_t error) {
		Serial.printf("Error[%u]: ", error);
		if (error == OTA_AUTH_ERROR) {
		Serial.println("Auth Failed");
		} else if (error == OTA_BEGIN_ERROR) {
		Serial.println("Begin Failed");
		} else if (error == OTA_CONNECT_ERROR) {
		Serial.println("Connect Failed");
		} else if (error == OTA_RECEIVE_ERROR) {
		Serial.println("Receive Failed");
		} else if (error == OTA_END_ERROR) {
		Serial.println("End Failed");
		}
	});
	
	ArduinoOTA.begin();
}

void ota::startAutomaticOTA()
{
	xTaskCreatePinnedToCore(
		[](void *pvParameters)
		{
			setupOTA();
			while (true)
			{
				ArduinoOTA.handle();
				delay(3500);
			}
		},
		"OTAHandler",
		10000,
		NULL,
		1,
		NULL,
		0);
}