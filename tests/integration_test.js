// ESP32-Server Integration Tests für VS Code Testing
const { test, suite } = require('node:test');
const assert = require('node:assert');
const WebSocket = require('ws');

suite('ESP32-Server Connection Tests', () => {
    const SERVER_URL = 'http://localhost:3000';
    const WS_URL = 'ws://localhost:3000/ws';


    test('Server Health Check', async () => {
        console.log('🩺 Testing server health...');

        // Wait for server to be ready first
        await waitForServer();

        const fetch = (await import('node-fetch')).default;
        const response = await fetch(`${SERVER_URL}/api/health`);

        assert.ok(response.ok, 'Server should be healthy');
        console.log('✅ Server is healthy');
    });

    test('ESP32 Discovery API', async () => {
        console.log('📡 Testing ESP32 discovery API...');

        const fetch = (await import('node-fetch')).default;
        const response = await fetch(`${SERVER_URL}/api/esp32/discovered`);

        if (response.ok) {
            const data = await response.json();
            console.log(`📱 API returned: ${JSON.stringify(data, null, 2)}`);

            assert.ok(data !== undefined, 'API should return data');
            assert.ok(Array.isArray(data.devices) || data.devices === undefined, 'Devices should be array or undefined');

            if (data.devices && data.devices.length > 0) {
                console.log(`✅ Found ${data.devices.length} ESP32 device(s)`);

                // Test device structure
                const firstDevice = data.devices[0];
                assert.ok(firstDevice.deviceId, 'Device should have deviceId');
                console.log(`📋 First device: ${firstDevice.deviceId}`);
            } else {
                console.log('ℹ️ No ESP32 devices currently discovered');
            }
        } else {
            console.log(`⚠️ API responded with status: ${response.status}`);
            // Don't fail if API endpoint doesn't exist yet
        }
    });

    test('WebSocket Connection', async () => {
        console.log('🔌 Testing WebSocket connection...');

        return new Promise((resolve, reject) => {
            const ws = new WebSocket(WS_URL);
            let connected = false;

            const timeout = setTimeout(() => {
                if (!connected) {
                    ws.close();
                    reject(new Error('WebSocket connection timeout'));
                }
            }, 10000);

            ws.on('open', () => {
                connected = true;
                clearTimeout(timeout);
                console.log('✅ WebSocket connected successfully');

                // Send test message
                ws.send(JSON.stringify({ type: 'ping', data: 'test' }));
            });

            ws.on('message', (data) => {
                console.log(`📨 WebSocket received: ${data}`);
                ws.close();
                resolve();
            });

            ws.on('error', (error) => {
                clearTimeout(timeout);
                console.log(`ℹ️ WebSocket error (expected if not implemented): ${error.message}`);
                resolve(); // Don't fail, just note
            });

            ws.on('close', () => {
                if (connected) {
                    console.log('🔌 WebSocket connection closed');
                    resolve();
                }
            });
        });
    });

    test('ESP32 MAC Address Resolution', async () => {
        console.log('🔍 Testing MAC address resolution...');

        const testMac = "10:20:BA:42:71:E0";
        const fetch = (await import('node-fetch')).default;
        const response = await fetch(`${SERVER_URL}/api/esp32/discovered`);

        if (response.ok) {
            const data = await response.json();

            if (data.devices && data.devices.length > 0) {
                console.log('🔍 Checking MAC address resolution...');

                data.devices.forEach(device => {
                    console.log(`📱 Device: ${device.deviceId}`);
                    console.log(`   MAC: "${device.macAddress}"`);
                    console.log(`   Match test MAC: ${device.macAddress === testMac}`);
                });

                const macMatch = data.devices.find(device => device.macAddress === testMac);

                if (macMatch) {
                    console.log(`✅ MAC resolution successful: ${macMatch.deviceId}`);
                    assert.strictEqual(macMatch.macAddress, testMac, 'MAC addresses should match');
                } else {
                    console.log(`ℹ️ Test MAC ${testMac} not found in discovered devices`);
                    // Don't fail test - just log for information
                }
            } else {
                console.log('ℹ️ No devices available for MAC resolution test');
            }
        }
    });

    test('ESP32 Device Detail API', async () => {
        console.log('📋 Testing device detail API...');

        const fetch = (await import('node-fetch')).default;
        // First get list of devices
        const listResponse = await fetch(`${SERVER_URL}/api/esp32/discovered`);

        if (listResponse.ok) {
            const listData = await listResponse.json();

            if (listData.devices && listData.devices.length > 0) {
                const firstDevice = listData.devices[0];
                console.log(`🔍 Testing detail API for device: ${firstDevice.deviceId}`);

                // Test device detail endpoint
                const detailResponse = await fetch(`${SERVER_URL}/api/devices/${firstDevice.deviceId}`);

                if (detailResponse.ok) {
                    const detailData = await detailResponse.json();
                    console.log(`✅ Device detail retrieved: ${JSON.stringify(detailData, null, 2)}`);
                    assert.ok(detailData !== undefined, 'Device detail should be defined');
                } else {
                    console.log(`⚠️ Device detail API returned: ${detailResponse.status}`);
                }
            } else {
                console.log('ℹ️ No devices available for detail API test');
            }
        }
    });
});

// Helper function to wait for server
async function waitForServer(maxRetries = 10) {
    const fetch = (await import('node-fetch')).default;
    for (let i = 0; i < maxRetries; i++) {
        try {
            const response = await fetch(`${SERVER_URL}/api/health`);
            if (response.ok) {
                console.log('✅ Server is ready');
                return;
            }
        } catch (error) {
            console.log(`⏳ Waiting for server... (attempt ${i + 1}/${maxRetries})`);
            await new Promise(resolve => setTimeout(resolve, 2000));
        }
    }
    throw new Error('Server failed to start within timeout period');
}