(function() {
    'use strict';
    
    // Local state for this script execution
    let esp32Websocket = null;
    let esp32Devices = new Map();
    let currentUser = null;

// Get device ID from URL parameter
function getDeviceIdFromUrl() {
    const pathParts = window.location.pathname.split('/');
    if (pathParts[1] === 'devices' && pathParts[2]) {
        return pathParts[2];
    }
    return null;
}

// Initialize page immediately (SPA context)
(async function() {
    await initializeAuth();
    await initializeWebSocket();
})();

async function initializeAuth() {
    try {
        const response = await fetch('/api/user-info', {
            credentials: 'include'
        });
        
        if (response.ok) {
            currentUser = await response.json();
            // User info is now handled by shared navigation in app.js
        } else {
            // Authentication is optional, continue as guest user
            currentUser = {
                success: true,
                authenticated: false,
                user_id: "guest",
                display_name: "Guest User",
                canvas_permissions: {}
            };
        }
    } catch (error) {
        console.error('Auth initialization failed, continuing as guest:', error);
        // Authentication is optional, continue as guest user
        currentUser = {
            success: true,
            authenticated: false,
            user_id: "guest",
            display_name: "Guest User",
            canvas_permissions: {}
        };
    }
}

async function initializeWebSocket() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/channel`;
    
    try {
        esp32Websocket = new WebSocket(wsUrl);
        
        esp32Websocket.onopen = function() {
            console.log('WebSocket connected');
            // Request list of ESP32 devices
            requestDeviceList();
        };
        
        esp32Websocket.onmessage = function(event) {
            handleWebSocketMessage(JSON.parse(event.data));
        };
        
        esp32Websocket.onclose = function() {
            console.log('WebSocket disconnected');
            setTimeout(initializeWebSocket, 3000); // Reconnect after 3s
        };
        
        esp32Websocket.onerror = function(error) {
            console.error('WebSocket error:', error);
        };
        
    } catch (error) {
        console.error('WebSocket initialization failed:', error);
        document.getElementById('loading-state').innerHTML = `
            <div class="alert alert-danger">
                <h4>Connection Failed</h4>
                <p>Could not connect to ESP32 service.</p>
                <button class="btn btn-primary" onclick="initializeWebSocket()">Retry</button>
            </div>
        `;
    }
}

async function requestDeviceList() {
    // Check if we have a specific device identifier from URL (could be MAC or deviceId)
    const urlDeviceIdentifier = getDeviceIdFromUrl();
    if (urlDeviceIdentifier) {
        console.log('Loading specific device from URL:', urlDeviceIdentifier);

        // Check if this is a MAC address or deviceId by querying the discovered devices
        try {
            const actualDeviceId = await resolveDeviceIdentifier(urlDeviceIdentifier);
            if (actualDeviceId) {
                console.log('Successfully resolved to deviceId:', actualDeviceId);
                registerForDevice(actualDeviceId);
            } else {
                console.log('Failed to resolve device identifier:', urlDeviceIdentifier);
                showDeviceNotFound(urlDeviceIdentifier);
            }
        } catch (error) {
            console.error('Error resolving device identifier:', error);
            showNoDeviceIdError();
        }
    } else {
        // No device identifier in URL - show error message
        showNoDeviceIdError();
    }
}

// Resolve MAC address to deviceId by checking discovered devices
async function resolveDeviceIdentifier(identifier) {
    try {
        const response = await fetch('/api/esp32/discovered', {
            method: 'GET',
            credentials: 'include'
        });

        if (response.ok) {
            const data = await response.json();
            const devices = data.devices || [];

            console.log('resolveDeviceIdentifier: Looking for identifier:', identifier);
            console.log('resolveDeviceIdentifier: Available devices:', devices);

            // First check if identifier is already a deviceId
            const directMatch = devices.find(device => device.deviceId === identifier);
            if (directMatch) {
                console.log('resolveDeviceIdentifier: Found direct deviceId match:', directMatch);
                return identifier;
            }

            // Then check if identifier is a MAC address
            console.log('resolveDeviceIdentifier: Checking MAC addresses...');
            devices.forEach(device => {
                console.log(`resolveDeviceIdentifier: Device ${device.deviceId} has MAC: "${device.macAddress}" (comparing with "${identifier}")`);
            });

            const macMatch = devices.find(device => device.macAddress === identifier);
            if (macMatch) {
                console.log('Found MAC address match, device ID should now be MAC address:', identifier);
                // Since we changed the system to use MAC as device ID, return the identifier directly
                return identifier;
            }

            console.warn('No device found for identifier:', identifier);
            console.warn('Available device IDs:', devices.map(d => d.deviceId));
            console.warn('Available MAC addresses:', devices.map(d => d.macAddress));
            return null;
        } else {
            console.error('Failed to fetch discovered devices:', response.status);
            return null;
        }
    } catch (error) {
        console.error('Error resolving device identifier:', error);
        return null;
    }
}

function registerForDevice(deviceId) {
    console.log('Attempting to register for device:', deviceId);
    if (esp32Websocket && esp32Websocket.readyState === WebSocket.OPEN) {
        console.log('WebSocket is open, sending registration request');
        esp32Websocket.send(JSON.stringify({
            type: 'registerForDevice',
            deviceId: deviceId
        }));
    } else {
        console.error('WebSocket not ready, readyState:', esp32Websocket?.readyState);
    }
}

function handleWebSocketMessage(message) {
    console.log('ESP32 FRONTEND DEBUG: Received WebSocket message:', message);
    if (message.deviceId && message.eventsForDevice) {
        console.log('ESP32 FRONTEND DEBUG: Processing', message.eventsForDevice.length, 'events for device', message.deviceId);
        handleDeviceEvents(message.deviceId, message.eventsForDevice);
    } else {
        console.log('ESP32 FRONTEND DEBUG: Message format not recognized:', message);
    }
}

function handleDeviceEvents(deviceId, events) {
    // Ensure device exists in our UI
    if (!esp32Devices.has(deviceId)) {
        createDeviceUI(deviceId);
    }
    
    events.forEach(event => {
        processDeviceEvent(deviceId, event);
    });
}

function createDeviceUI(deviceId) {
    console.log('ESP32 FRONTEND DEBUG: createDeviceUI called for device:', deviceId);
    // Create a more readable device name
    let deviceName = deviceId;
    if (deviceId.startsWith('esp32-')) {
        deviceName = deviceId.replace('esp32-', 'ESP32 ').replace(/-/g, ' ').toUpperCase();
    } else {
        deviceName = deviceId.replace('test-', '').replace(/-/g, ' ').toUpperCase();
    }
    
    const device = {
        id: deviceId,
        name: deviceName,
        connected: false,
        users: [],
        udpMessages: [],
        tcpMessages: [],
        variables: new Map(),
        startOptions: []
    };
    
    esp32Devices.set(deviceId, device);
    renderDevices();
}

function processDeviceEvent(deviceId, event) {
    console.log('ESP32 FRONTEND DEBUG: Processing event for device', deviceId, ':', event);
    const device = esp32Devices.get(deviceId);
    if (!device) {
        console.log('ESP32 FRONTEND DEBUG: Device not found:', deviceId);
        return;
    }

    // Handle new server event format (tagged enum)
    let eventType = null;
    let eventData = null;

    if (event.esp32ConnectionStatus) {
        eventType = 'esp32ConnectionStatus';
        eventData = event.esp32ConnectionStatus;
    } else if (event.esp32UdpBroadcast) {
        eventType = 'esp32UdpBroadcast';
        eventData = event.esp32UdpBroadcast;
    } else if (event.esp32VariableUpdate) {
        eventType = 'esp32VariableUpdate';
        eventData = event.esp32VariableUpdate;
    } else if (event.esp32StartOptions) {
        eventType = 'esp32StartOptions';
        eventData = event.esp32StartOptions;
    } else if (event.esp32ChangeableVariables) {
        eventType = 'esp32ChangeableVariables';
        eventData = event.esp32ChangeableVariables;
    } else if (event.userJoined) {
        eventType = 'userJoined';
        eventData = event.userJoined;
    } else if (event.userLeft) {
        eventType = 'userLeft';
        eventData = event.userLeft;
    } else if (event.event) {
        // Legacy format support
        eventType = event.event;
        eventData = event;
    } else {
        console.log('Unknown ESP32 event format:', event);
        return;
    }

    console.log('ESP32 FRONTEND DEBUG: Event type determined:', eventType, 'with data:', eventData);
    switch (eventType) {
        case 'esp32ConnectionStatus':
            device.connected = eventData.connected;
            updateConnectionStatus(deviceId, eventData.connected);
            break;

        case 'esp32UdpBroadcast':
            console.log('ESP32 FRONTEND DEBUG: Adding UDP message to device:', eventData.message);
            device.udpMessages.push(`[${new Date().toLocaleTimeString()}] ${eventData.message}`);
            console.log('ESP32 FRONTEND DEBUG: UDP messages array now has', device.udpMessages.length, 'messages');
            updateMonitorArea(deviceId, 'udp');
            console.log('ESP32 FRONTEND DEBUG: Called updateMonitorArea for UDP');
            break;

        case 'esp32VariableUpdate':
            console.log('ESP32 FRONTEND DEBUG: Updating variable:', eventData.variableName, '=', eventData.variableValue);
            device.variables.set(eventData.variableName, eventData.variableValue);
            updateVariableMonitor(deviceId, eventData.variableName, eventData.variableValue);
            console.log('ESP32 FRONTEND DEBUG: Called updateVariableMonitor');
            break;

        case 'esp32StartOptions':
            device.startOptions = eventData.options;
            updateStartOptions(deviceId, eventData.options);
            break;

        case 'esp32ChangeableVariables':
            updateVariableControls(deviceId, eventData.variables);
            break;

        case 'userJoined':
            if (eventData.userId !== 'ESP32_SYSTEM') {
                device.users.push({
                    userId: eventData.userId,
                    displayName: eventData.displayName,
                    userColor: eventData.userColor
                });
                updateDeviceUsers(deviceId);
            }
            break;

        case 'userLeft':
            if (eventData.userId !== 'ESP32_SYSTEM') {
                device.users = device.users.filter(u => u.userId !== eventData.userId);
                updateDeviceUsers(deviceId);
            }
            break;

        default:
            console.log('Unknown ESP32 event type:', eventType, eventData);
    }
}

function renderDevices() {
    if (esp32Devices.size === 0) {
        showNoDevicesState();
        return;
    }
    
    hideLoadingState();
    
    // Clear existing content
    document.getElementById('deviceTabs').innerHTML = '';
    document.getElementById('deviceTabContent').innerHTML = '';
    document.getElementById('esp32-stack').innerHTML = '';
    document.getElementById('esp32-grid').innerHTML = '';
    
    const devices = Array.from(esp32Devices.values());
    const urlDeviceId = getDeviceIdFromUrl();
    
    // If we have a specific device ID from URL, only show that device
    const devicesToShow = urlDeviceId ? devices.filter(device => device.id === urlDeviceId) : devices;
    
    if (devicesToShow.length === 0 && urlDeviceId) {
        showSpecificDeviceNotFound(urlDeviceId);
        return;
    }
    
    devicesToShow.forEach((device, index) => {
        createDeviceTabContent(device, index === 0);
        createDeviceStackContent(device);
        createDeviceGridContent(device);
    });
    
    showDevicesContainer();
}

function createDeviceTabContent(device, isActive) {
    // Create tab
    const tab = document.createElement('li');
    tab.className = 'nav-item';
    tab.innerHTML = `
        <button class="nav-link ${isActive ? 'active' : ''}" 
                id="${device.id}-tab" 
                data-bs-toggle="tab" 
                data-bs-target="#${device.id}-content" 
                type="button" 
                role="tab">
            <span class="status-dot ${getStatusClass(device.connected)}"></span>
            ${device.name}
        </button>
    `;
    document.getElementById('deviceTabs').appendChild(tab);
    
    // Create tab content
    const content = document.createElement('div');
    content.className = `tab-pane fade ${isActive ? 'show active' : ''}`;
    content.id = `${device.id}-content`;
    content.setAttribute('role', 'tabpanel');
    content.innerHTML = createDeviceContent(device, 'tab');
    document.getElementById('deviceTabContent').appendChild(content);
}

function createDeviceStackContent(device) {
    const stackItem = document.createElement('div');
    stackItem.className = 'esp32-device-card mb-4';
    stackItem.innerHTML = `
        <div class="esp32-device-header">
            <div>
                <h5 class="mb-1">${device.name}</h5>
                <div class="connection-status">
                    <span class="status-dot ${getStatusClass(device.connected)}"></span>
                    ${getStatusText(device.connected)}
                </div>
            </div>
            <div class="device-users" id="${device.id}-stack-users"></div>
        </div>
        <div class="p-3">
            ${createDeviceContent(device, 'stack')}
        </div>
    `;
    document.getElementById('esp32-stack').appendChild(stackItem);
}

function createDeviceGridContent(device) {
    const gridItem = document.createElement('div');
    gridItem.className = 'esp32-device-card';
    gridItem.innerHTML = `
        <div class="esp32-device-header">
            <div>
                <h5 class="mb-1">${device.name}</h5>
                <div class="connection-status">
                    <span class="status-dot ${getStatusClass(device.connected)}"></span>
                    ${getStatusText(device.connected)}
                </div>
            </div>
            <div class="device-users" id="${device.id}-grid-users"></div>
        </div>
        <div class="p-3">
            ${createDeviceContent(device, 'grid')}
        </div>
    `;
    document.getElementById('esp32-grid').appendChild(gridItem);
}

function createDeviceContent(device, suffix = '') {
    const idPrefix = suffix ? `${device.id}-${suffix}` : device.id;
    return `
        <!-- Control Panel -->
        <div class="start-options-area">
            <h6><i class="bi bi-play-circle"></i> Device Control</h6>
            <div class="row align-items-end">
                <div class="col-md-4">
                    <label class="form-label">Start Option</label>
                    <select class="form-select" id="${idPrefix}-start-select">
                        <option value="">Select option...</option>
                    </select>
                </div>
                <div class="col-md-4">
                    <div class="form-check mb-2">
                        <input class="form-check-input" type="checkbox" id="${idPrefix}-auto-start">
                        <label class="form-check-label" for="${idPrefix}-auto-start">Auto Start</label>
                    </div>
                </div>
                <div class="col-md-4">
                    <button class="btn btn-success me-2" onclick="sendStartOption('${device.id}')">
                        <i class="bi bi-play"></i> Start
                    </button>
                    <button class="btn btn-danger" onclick="sendReset('${device.id}')">
                        <i class="bi bi-arrow-clockwise"></i> Reset
                    </button>
                </div>
            </div>
        </div>

        <!-- Variable Controls -->
        <div class="variable-control">
            <h6><i class="bi bi-sliders"></i> Variable Control</h6>
            <div id="${idPrefix}-variables">
                <p class="text-muted">No variables available</p>
            </div>
        </div>

        <!-- Monitors -->
        <div class="row">
            <div class="col-lg-6">
                <h6><i class="bi bi-broadcast"></i> UDP Monitor</h6>
                <div class="monitor-area" id="${idPrefix}-udp-monitor"></div>
            </div>
            <div class="col-lg-6">
                <h6><i class="bi bi-link-45deg"></i> Variable Monitor</h6>
                <div class="monitor-area" id="${idPrefix}-variable-monitor"></div>
            </div>
        </div>
    `;
}

function getStatusClass(connected) {
    return connected ? 'status-connected' : 'status-disconnected';
}

function getStatusText(connected) {
    return connected ? 'Connected' : 'Disconnected';
}

function hideLoadingState() {
    document.getElementById('loading-state').style.display = 'none';
}

function showNoDevicesState() {
    document.getElementById('loading-state').style.display = 'none';
    document.getElementById('no-devices-state').style.display = 'block';
}

function showNoDeviceIdError() {
    document.getElementById('loading-state').style.display = 'none';
    const noDevicesEl = document.getElementById('no-devices-state');
    noDevicesEl.innerHTML = `
        <div class="alert alert-danger">
            <h4><i class="bi bi-exclamation-triangle"></i> No ESP32 Device Selected</h4>
            <p>No ESP32 device ID found in the URL. This page requires a specific device to connect to.</p>
            <p>Please select an ESP32 device from the main page.</p>
            <a href="/" class="btn btn-primary spa-link">
                <i class="bi bi-house"></i> Go to Home Page
            </a>
        </div>
    `;
    noDevicesEl.style.display = 'block';
}

function showDeviceNotFound(identifier) {
    document.getElementById('loading-state').style.display = 'none';
    const noDevicesEl = document.getElementById('no-devices-state');
    noDevicesEl.innerHTML = `
        <div class="alert alert-warning">
            <h4><i class="bi bi-exclamation-triangle"></i> ESP32 Device Not Found</h4>
            <p>No ESP32 device found with identifier "${identifier}".</p>
            <p>The device may be offline or not yet discovered.</p>
            <button class="btn btn-primary" onclick="refreshDevices()">
                <i class="bi bi-arrow-clockwise"></i> Refresh
            </button>
            <a href="/" class="btn btn-secondary ms-2 spa-link">
                <i class="bi bi-house"></i> Back to Home
            </a>
        </div>
    `;
    noDevicesEl.style.display = 'block';
}

function showSpecificDeviceNotFound(deviceId) {
    document.getElementById('loading-state').style.display = 'none';
    const noDevicesEl = document.getElementById('no-devices-state');
    noDevicesEl.innerHTML = `
        <div class="alert alert-warning">
            <h4><i class="bi bi-exclamation-triangle"></i> ESP32 Device Not Found</h4>
            <p>The ESP32 device with ID "${deviceId}" is not currently available or connected.</p>
            <button class="btn btn-primary" onclick="refreshDevices()">
                <i class="bi bi-arrow-clockwise"></i> Refresh
            </button>
            <a href="/" class="btn btn-secondary ms-2 spa-link">
                <i class="bi bi-house"></i> Back to Home
            </a>
        </div>
    `;
    noDevicesEl.style.display = 'block';
}

function showDevicesContainer() {
    document.getElementById('loading-state').style.display = 'none';
    document.getElementById('no-devices-state').style.display = 'none';
    
    // Show appropriate layout based on screen size
    if (window.innerWidth >= 1400) {
        document.getElementById('esp32-grid').style.display = 'grid';
    } else if (window.innerWidth >= 769) {
        document.getElementById('esp32-tabs').style.display = 'block';
    } else {
        document.getElementById('esp32-stack').style.display = 'block';
    }
}

function updateConnectionStatus(deviceId, connected) {
    // Update all status indicators for this device
    const statusElements = document.querySelectorAll(`[id*="${deviceId}"] .status-dot`);
    statusElements.forEach(el => {
        el.className = `status-dot ${getStatusClass(connected)}`;
    });
}

function updateMonitorArea(deviceId, type) {
    console.log('ESP32 FRONTEND DEBUG: updateMonitorArea called for device:', deviceId, 'type:', type);
    const device = esp32Devices.get(deviceId);

    // Update all monitor variants (tab, stack, grid)
    const suffixes = ['tab', 'stack', 'grid'];
    let updated = false;

    suffixes.forEach(suffix => {
        const monitorId = `${deviceId}-${suffix}-udp-monitor`;
        const monitorEl = document.getElementById(monitorId);

        if (monitorEl && type === 'udp') {
            console.log(`ESP32 FRONTEND DEBUG: Updating ${suffix} UDP monitor with`, device.udpMessages.length, 'messages');
            monitorEl.innerHTML = device.udpMessages.slice(-50).join('<br>');
            monitorEl.scrollTop = monitorEl.scrollHeight;
            updated = true;
        }
    });

    if (updated) {
        console.log('ESP32 FRONTEND DEBUG: UDP monitor updated successfully');
    } else {
        console.log('ESP32 FRONTEND DEBUG: Cannot update UDP monitor - no elements found');
        // Debug: List all elements with similar IDs
        const allElements = document.querySelectorAll('[id*="monitor"]');
        console.log('ESP32 FRONTEND DEBUG: All monitor elements found:', Array.from(allElements).map(el => el.id));
    }
}

function updateVariableMonitor(deviceId, name, value) {
    console.log('ESP32 FRONTEND DEBUG: updateVariableMonitor called for device:', deviceId, 'variable:', name, 'value:', value);

    // Update all variable monitor variants (tab, stack, grid)
    const suffixes = ['tab', 'stack', 'grid'];
    let updated = false;

    suffixes.forEach(suffix => {
        const monitorId = `${deviceId}-${suffix}-variable-monitor`;
        const monitorEl = document.getElementById(monitorId);

        if (monitorEl) {
            const timestamp = new Date().toLocaleTimeString();
            const existingContent = monitorEl.innerHTML;
            monitorEl.innerHTML = existingContent + `<br>[${timestamp}] ${name}: ${value}`;
            monitorEl.scrollTop = monitorEl.scrollHeight;
            updated = true;
        }
    });

    if (updated) {
        console.log('ESP32 FRONTEND DEBUG: Variable monitor updated successfully');
    } else {
        console.log('ESP32 FRONTEND DEBUG: Cannot update variable monitor - element not found');
    }
}

function updateStartOptions(deviceId, options) {
    const selectEl = document.getElementById(`${deviceId}-start-select`);
    if (selectEl) {
        selectEl.innerHTML = '<option value="">Select option...</option>';
        options.forEach(option => {
            const optionEl = document.createElement('option');
            optionEl.value = option;
            optionEl.textContent = option;
            selectEl.appendChild(optionEl);
        });
    }
}

function updateVariableControls(deviceId, variables) {
    const containerEl = document.getElementById(`${deviceId}-variables`);
    if (containerEl) {
        if (variables.length === 0) {
            containerEl.innerHTML = '<p class="text-muted">No variables available</p>';
            return;
        }
        
        containerEl.innerHTML = '';
        variables.forEach(variable => {
            const variableEl = document.createElement('div');
            variableEl.className = 'variable-item';
            variableEl.innerHTML = `
                <div class="variable-name">${variable.name}</div>
                <input type="number" 
                       class="form-control variable-value" 
                       value="${variable.value}"
                       min="0"
                       onkeypress="handleVariableKeyPress(event, '${deviceId}', '${variable.name}')">
                <button class="btn btn-sm btn-outline-primary" 
                        onclick="sendVariable('${deviceId}', '${variable.name}')">
                    <i class="bi bi-send"></i>
                </button>
            `;
            containerEl.appendChild(variableEl);
        });
    }
}

function updateDeviceUsers(deviceId) {
    const device = esp32Devices.get(deviceId);
    ['tabs', 'stack', 'grid'].forEach(layout => {
        const usersEl = document.getElementById(`${deviceId}-${layout}-users`);
        if (usersEl) {
            if (device.users.length === 0) {
                usersEl.innerHTML = '';
            } else {
                usersEl.innerHTML = device.users.map(user => `
                    <span class="user-indicator" style="background-color: ${user.userColor}"></span>
                    ${user.displayName}
                `).join(', ');
            }
        }
    });
}

// Event handlers
function sendStartOption(deviceId) {
    const selectEl = document.getElementById(`${deviceId}-start-select`);
    if (selectEl && selectEl.value && esp32Websocket) {
        esp32Websocket.send(JSON.stringify({
            type: 'deviceEvent',
            deviceId: deviceId,
            eventsForDevice: [{
                event: 'esp32Command',
                command: {
                    startOption: selectEl.value
                }
            }]
        }));
    }
}

function sendReset(deviceId) {
    if (esp32Websocket) {
        esp32Websocket.send(JSON.stringify({
            type: 'deviceEvent',
            deviceId: deviceId,
            eventsForDevice: [{
                event: 'esp32Command',
                command: {
                    reset: true
                }
            }]
        }));
    }
}

function sendVariable(deviceId, variableName) {
    const inputEl = document.querySelector(`#${deviceId}-variables input[onkeypress*="${variableName}"]`);
    if (inputEl && esp32Websocket) {
        const value = parseInt(inputEl.value) || 0;
        esp32Websocket.send(JSON.stringify({
            type: 'deviceEvent',
            deviceId: deviceId,
            eventsForDevice: [{
                event: 'esp32Command',
                command: {
                    setVariable: {
                        name: variableName,
                        value: value
                    }
                }
            }]
        }));
    }
}

function handleVariableKeyPress(event, deviceId, variableName) {
    if (event.key === 'Enter') {
        sendVariable(deviceId, variableName);
    }
}

function refreshDevices() {
    location.reload();
}

function logout() {
    fetch('/api/logout', { method: 'POST', credentials: 'include' })
        .then(() => window.location.href = '/')
        .catch(() => window.location.href = '/');
}

// Handle window resize for responsive layout
window.addEventListener('resize', function() {
    if (esp32Devices.size > 0) {
        showDevicesContainer();
    }
});

})(); // End IIFE