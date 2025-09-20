#pragma once
#include <Arduino.h>
#include <WiFi.h>
#include <ESPmDNS.h>
#include <WiFiUdp.h>
#include <AsyncTCP.h>
#include <vector>
#include <functional>
#include <AsyncUDP.h>
#include <iostream>
#include <map>
#include <ArduinoJson.h>
#include "../config/config.hpp"
#include <atomic>

namespace ota
{

    inline AsyncUDP asyncUdp;

    



    inline void udpOut(const char* data, size_t len)
    {
        asyncUdp.writeTo((const uint8_t*) data, len, udpTargetIP, udpPort);
        Serial.println("udp out");
    }
    
    inline void serialOut(const char* data, size_t len)
    {
        Serial.write(data, len);
    }
    
    class AsyncTCPServer {
        public:
            using ClientConnectCallback = std::function<void(AsyncClient*)>;
            using ClientDisconnectCallback = std::function<void(AsyncClient*)>;
            using DataCallback = std::function<void(AsyncClient*, void*, size_t)>;
            
        private:
            AsyncServer _server;
            uint16_t _port;
            std::vector<AsyncClient*> _clients;
            
            ClientConnectCallback _onClientConnect = nullptr;
            ClientDisconnectCallback _onClientDisconnect = nullptr;
            DataCallback _onData = nullptr;
            SemaphoreHandle_t selectedStartOptionMutex;
            
            StaticJsonDocument<512> _jsonDocFunctions;
            StaticJsonDocument<512> _jsonDocVariables;
            char _jsonBufferFunctions[512];
            char _jsonBufferVariables[512];
    
            std::map<std::string, std::function<void()>> startOptions;
            std::map<std::string, std::reference_wrapper<uint32_t>> changeableVariables;
    
            std::function<void()> selectedStartOption;
            bool validStartOption = false;
    
        public:
            AsyncTCPServer(uint16_t port = 3232);
            void begin();
            size_t getClientCount() const;
            bool sendToAll(const char* data, size_t len);
            bool sendToAll(const char* data);
            bool sendStartOptions(const std::map<std::string, std::function<void()>>& options);
            bool sendChangeableVariables(const std::map<std::string, std::reference_wrapper<uint32_t>>& variables);
            bool sendStartOptions();
            bool sendChangeableVariables();
    
    
            bool getSelectedStartOption(std::function<void()>& option);
    
            
            // Callback setters
            void onClientConnect(ClientConnectCallback cb);
            void onClientDisconnect(ClientDisconnectCallback cb);
            void onData(DataCallback cb);
    
    
            void setStartOptions(std::map<std::string, std::function<void()>> startOptions);
            void setChangeableVariables(std::map<std::string, uint32_t&> changeableVariables);
    
            void addStartOption(std::string name, std::function<void()> function);
            void addChangeableVariable(std::string name, uint32_t& variable);
    
            private:
            void onConnectInternal(AsyncClient* client);
            void onDisconnectInternal(AsyncClient* client);
            void onDataInternal(AsyncClient* client, void* data, size_t len);
            void onErrorInternal(int8_t error);
            void sendStartOptionsInternal();
        };
    
        inline AsyncTCPServer tcpServer;
    
    
        
}



