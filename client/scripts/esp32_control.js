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
            window.location.href = '/login.html';
        }
    } catch (error) {
        console.error('Auth initialization failed:', error);
        window.location.href = '/login.html';
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
                registerForDevice(actualDeviceId);
            } else {
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

            // First check if identifier is already a deviceId
            const directMatch = devices.find(device => device.deviceId === identifier);
            if (directMatch) {
                return identifier;
            }

            // Then check if identifier is a MAC address
            const macMatch = devices.find(device => device.macAddress === identifier);
            if (macMatch) {
                console.log('Resolved MAC address', identifier, 'to deviceId:', macMatch.deviceId);
                return macMatch.deviceId;
            }

            console.warn('No device found for identifier:', identifier);
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
    if (esp32Websocket && esp32Websocket.readyState === WebSocket.OPEN) {
        esp32Websocket.send(JSON.stringify({
            command: 'registerForDevice',
            deviceId: deviceId
        }));
    }
}

function handleWebSocketMessage(message) {
    if (message.deviceId && message.eventsForDevice) {
        handleDeviceEvents(message.deviceId, message.eventsForDevice);
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
    const device = esp32Devices.get(deviceId);
    if (!device) return;
    
    switch (event.event) {
        case 'esp32ConnectionStatus':
            device.connected = event.connected;
            updateConnectionStatus(deviceId, event.connected);
            break;
            
        case 'esp32UdpBroadcast':
            device.udpMessages.push(`[${new Date().toLocaleTimeString()}] ${event.message}`);
            updateMonitorArea(deviceId, 'udp');
            break;
            
        case 'esp32VariableUpdate':
            device.variables.set(event.variableName, event.variableValue);
            updateVariableMonitor(deviceId, event.variableName, event.variableValue);
            break;
            
        case 'esp32StartOptions':
            device.startOptions = event.options;
            updateStartOptions(deviceId, event.options);
            break;
            
        case 'esp32ChangeableVariables':
            updateVariableControls(deviceId, event.variables);
            break;
            
        case 'userJoined':
            if (event.userId !== 'ESP32_SYSTEM') {
                device.users.push({
                    userId: event.userId,
                    displayName: event.displayName,
                    userColor: event.userColor
                });
                updateDeviceUsers(deviceId);
            }
            break;
            
        case 'userLeft':
            if (event.userId !== 'ESP32_SYSTEM') {
                device.users = device.users.filter(u => u.userId !== event.userId);
                updateDeviceUsers(deviceId);
            }
            break;
            
        default:
            console.log('Unknown ESP32 event:', event);
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
    content.innerHTML = createDeviceContent(device);
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
            ${createDeviceContent(device)}
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
            ${createDeviceContent(device)}
        </div>
    `;
    document.getElementById('esp32-grid').appendChild(gridItem);
}

function createDeviceContent(device) {
    return `
        <!-- Control Panel -->
        <div class="start-options-area">
            <h6><i class="bi bi-play-circle"></i> Device Control</h6>
            <div class="row align-items-end">
                <div class="col-md-4">
                    <label class="form-label">Start Option</label>
                    <select class="form-select" id="${device.id}-start-select">
                        <option value="">Select option...</option>
                    </select>
                </div>
                <div class="col-md-4">
                    <div class="form-check mb-2">
                        <input class="form-check-input" type="checkbox" id="${device.id}-auto-start">
                        <label class="form-check-label" for="${device.id}-auto-start">Auto Start</label>
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
            <div id="${device.id}-variables">
                <p class="text-muted">No variables available</p>
            </div>
        </div>
        
        <!-- Monitors -->
        <div class="row">
            <div class="col-lg-6">
                <h6><i class="bi bi-broadcast"></i> UDP Monitor</h6>
                <div class="monitor-area" id="${device.id}-udp-monitor"></div>
            </div>
            <div class="col-lg-6">
                <h6><i class="bi bi-link-45deg"></i> Variable Monitor</h6>
                <div class="monitor-area" id="${device.id}-variable-monitor"></div>
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
    const device = esp32Devices.get(deviceId);
    const monitorEl = document.getElementById(`${deviceId}-udp-monitor`);
    if (monitorEl && type === 'udp') {
        monitorEl.innerHTML = device.udpMessages.slice(-50).join('<br>');
        monitorEl.scrollTop = monitorEl.scrollHeight;
    }
}

function updateVariableMonitor(deviceId, name, value) {
    const monitorEl = document.getElementById(`${deviceId}-variable-monitor`);
    if (monitorEl) {
        const timestamp = new Date().toLocaleTimeString();
        const existingContent = monitorEl.innerHTML;
        monitorEl.innerHTML = existingContent + `<br>[${timestamp}] ${name}: ${value}`;
        monitorEl.scrollTop = monitorEl.scrollHeight;
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
            command: 'deviceEvent',
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
            command: 'deviceEvent',
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
            command: 'deviceEvent',
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
        .then(() => window.location.href = '/login.html')
        .catch(() => window.location.href = '/login.html');
}

// Handle window resize for responsive layout
window.addEventListener('resize', function() {
    if (esp32Devices.size > 0) {
        showDevicesContainer();
    }
});

})(); // End IIFE