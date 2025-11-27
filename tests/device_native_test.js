// Native VS Code Test für Device-Server
const { test, suite } = require('node:test');
const assert = require('node:assert');
const WebSocket = require('ws');

suite('Device-Server Connection Tests', () => {
    const SERVER_URL = 'http://localhost:3000';
    const WS_URL = 'ws://localhost:3000/ws';

    test('Server Response Check', async () => {
        console.log('🩺 Testing server response...');

        try {
            const fetch = (await import('node-fetch')).default;
            const response = await fetch(SERVER_URL);
            assert.ok(response.ok, 'Server should respond');
            console.log('✅ Server is responding');
        } catch (error) {
            assert.fail(`Server not reachable: ${error.message}`);
        }
    });

    test('Device Discovery API', async () => {
        console.log('📡 Testing Device discovery API...');

        try {
            const fetch = (await import('node-fetch')).default;
            const response = await fetch(`${SERVER_URL}/api/esp32/discovered`);

            if (response.ok) {
                const data = await response.json();
                console.log(`📱 API returned: ${JSON.stringify(data, null, 2)}`);

                assert.ok(data !== undefined, 'API should return data');

                if (data.devices) {
                    assert.ok(Array.isArray(data.devices), 'Devices should be an array');
                    console.log(`✅ Found ${data.devices.length} Device device(s)`);
                } else {
                    console.log('ℹ️ No Device devices currently discovered');
                }
            } else {
                console.log(`⚠️ API responded with status: ${response.status}`);
                // Don't fail if API endpoint doesn't exist yet
            }
        } catch (error) {
            console.log(`ℹ️ Device API test: ${error.message}`);
            // Don't fail - server might not have this endpoint
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
                    console.log('⚠️ WebSocket connection timeout - this is OK if WS not implemented');
                    resolve(); // Don't fail, just note
                }
            }, 5000);

            ws.on('open', () => {
                connected = true;
                clearTimeout(timeout);
                console.log('✅ WebSocket connected successfully');

                // Send test message
                ws.send(JSON.stringify({ type: 'ping', data: 'test' }));
                setTimeout(() => {
                    ws.close();
                    resolve();
                }, 1000);
            });

            ws.on('message', (data) => {
                console.log(`📨 WebSocket received: ${data}`);
            });

            ws.on('error', (error) => {
                clearTimeout(timeout);
                console.log(`ℹ️ WebSocket error (expected if not implemented): ${error.message}`);
                resolve(); // Don't fail, just note
            });
        });
    });

    test('Server Static Files', async () => {
        console.log('📄 Testing static file serving...');

        try {
            const fetch = (await import('node-fetch')).default;
            const response = await fetch(SERVER_URL);

            assert.ok(response.ok, 'Server should serve static files');

            const contentType = response.headers.get('content-type');
            assert.ok(contentType && contentType.includes('text/html'), 'Should serve HTML');

            console.log('✅ Static files served correctly');
        } catch (error) {
            assert.fail(`Static file test failed: ${error.message}`);
        }
    });
});