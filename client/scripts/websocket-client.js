// WebSocket Client for Canvas Multi-User Communication
// Handles WebSocket connection, reconnection, and message routing

// Check if already loaded to prevent duplicate class declarations
if (typeof WebSocketClient !== 'undefined') {
    console.log('WebSocketClient already loaded, skipping redefinition');
} else {

// WebSocket connection states for better state management
const WebSocketStates = {
    DISCONNECTED: 'disconnected',
    CONNECTING: 'connecting',
    CONNECTED: 'connected',
    RECONNECTING: 'reconnecting',
    FAILED: 'failed'
};

class WebSocketClient {
    constructor() {
        this.ws = null;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 10;
        this.reconnectDelay = 1000; // Start with 1 second
        this.messageHandlers = {};
        this.heartbeatInterval = null;
        this.lastHeartbeat = null;
        
        // Promise-based acknowledgments for reliable operations
        this.pendingAcks = new Map(); // messageId -> { resolve, reject, timeout }
        this.messageIdCounter = 0;
        
        // State machine for connection management
        this.state = WebSocketStates.DISCONNECTED;
        this.stateChangedAt = Date.now();
        
        console.log('WebSocketClient initialized with state:', this.state);
    }
    
    // Get current connection state
    get isConnected() {
        return this.state === WebSocketStates.CONNECTED;
    }
    
    // Get current connection state
    get isConnecting() {
        return this.state === WebSocketStates.CONNECTING;
    }
    
    // Change state and emit event
    changeState(newState) {
        const oldState = this.state;
        this.state = newState;
        this.stateChangedAt = Date.now();
        
        console.log(`WebSocket state changed: ${oldState} → ${newState}`);
        this.emit('stateChanged', { from: oldState, to: newState, timestamp: this.stateChangedAt });
    }
    
    // Connect to WebSocket server with singleton optimization
    connect(url = null, forceReconnect = false) {
        // Auto-detect WebSocket URL from current page location
        if (!url) {
            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            const host = window.location.host; // Includes port if present
            url = `${protocol}//${host}/channel`;
        }
        // If force reconnect is requested, close existing connection
        if (forceReconnect && this.ws) {
            console.log('Force reconnect requested, closing existing connection');
            this.close();
        }
        
        // Use state machine to check current state
        if (this.state === WebSocketStates.CONNECTING) {
            console.log('WebSocket connection already in progress');
            return Promise.resolve();
        }
        
        if (this.state === WebSocketStates.CONNECTED && !forceReconnect) {
            console.log('WebSocket already connected, reusing existing connection');
            return Promise.resolve();
        }
        
        console.log(`Connecting to WebSocket: ${url} (force: ${forceReconnect})`);
        
        // Change state to connecting
        this.changeState(WebSocketStates.CONNECTING);
        
        return new Promise((resolve, reject) => {
            try {
                this.ws = new WebSocket(url);
                this.setupEventHandlers();
                
                // Store resolve/reject for connection result
                this._connectionPromise = { resolve, reject };
                
                // Set timeout for connection attempt
                const connectionTimeout = setTimeout(() => {
                    if (this._connectionPromise) {
                        this.changeState(WebSocketStates.FAILED);
                        this._connectionPromise.reject(new Error('Connection timeout'));
                        this._connectionPromise = null;
                    }
                }, 5000);
                
                // Clear timeout on success/failure
                this.ws.addEventListener('open', () => {
                    clearTimeout(connectionTimeout);
                });
                
                this.ws.addEventListener('error', () => {
                    clearTimeout(connectionTimeout);
                });
                
            } catch (error) {
                console.error('Failed to create WebSocket connection:', error);
                this.changeState(WebSocketStates.FAILED);
                reject(error);
                this.scheduleReconnect();
            }
        });
    }
    
    // Setup WebSocket event handlers
    setupEventHandlers() {
        this.ws.onopen = (event) => {
            console.log('WebSocket connected successfully');
            this.changeState(WebSocketStates.CONNECTED);
            this.reconnectAttempts = 0;
            this.reconnectDelay = 1000;
            this.startHeartbeat();
            
            // Resolve connection promise if exists
            if (this._connectionPromise) {
                this._connectionPromise.resolve();
                this._connectionPromise = null;
            }
            
            this.emit('connected', event);
        };
        
        this.ws.onmessage = (event) => {
            try {
                const message = JSON.parse(event.data);
                console.log('WebSocket message received:', message);
                console.log('DEBUG: Message type:', typeof message, 'Keys:', Object.keys(message));
                
                
                if (message.canvasId && message.eventsForCanvas) {
                    console.log('DEBUG: Canvas events received for canvas:', message.canvasId, 
                               'Event count:', message.eventsForCanvas.length);
                }
                
                this.handleMessage(message);
            } catch (error) {
                console.error('Error parsing WebSocket message:', error, event.data);
            }
        };
        
        this.ws.onclose = (event) => {
            console.log('WebSocket connection closed:', event.code, event.reason);
            
            // Update state based on close reason
            if (event.code === 1000) {
                this.changeState(WebSocketStates.DISCONNECTED);
            } else {
                this.changeState(WebSocketStates.RECONNECTING);
            }
            
            this.stopHeartbeat();
            
            // Reject pending acknowledgments on connection close
            this.rejectPendingAcks(`Connection closed (code: ${event.code})`);
            
            this.emit('disconnected', event);
            
            // Attempt to reconnect unless it was a deliberate close
            if (event.code !== 1000) {
                this.scheduleReconnect();
            }
        };
        
        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
            
            // Update state to failed if we're not already reconnecting
            if (this.state !== WebSocketStates.RECONNECTING) {
                this.changeState(WebSocketStates.FAILED);
            }
            
            // Reject connection promise if exists
            if (this._connectionPromise) {
                this._connectionPromise.reject(error);
                this._connectionPromise = null;
            }
            
            // Reject all pending acknowledgments
            this.rejectPendingAcks('Connection error');
            
            this.emit('error', error);
        };
    }
    
    // Handle incoming messages
    handleMessage(message) {
        // Check for heartbeat response
        if (message.type === 'pong') {
            this.lastHeartbeat = Date.now();
            return;
        }
        
        // Handle server messages (canvas events)
        if (message.canvasId && message.eventsForCanvas) {
            this.emit('serverMessage', message);
            return;
        }
        
        // Handle other message types
        this.emit('message', message);
    }
    
    // Send message to server
    send(message) {
        // Double-check both internal state and actual WebSocket state
        if (this.state !== WebSocketStates.CONNECTED || 
            !this.ws || 
            this.ws.readyState !== WebSocket.OPEN) {
            console.warn('Cannot send message: WebSocket not connected (state:', this.state, 
                        'readyState:', this.ws ? this.ws.readyState : 'no socket', ')');
            
            // Update internal state if it's out of sync
            if (this.ws && this.ws.readyState !== WebSocket.OPEN && this.state === WebSocketStates.CONNECTED) {
                console.warn('State out of sync - correcting internal state');
                this.changeState(WebSocketStates.DISCONNECTED);
            }
            
            return false;
        }
        
        try {
            const messageString = typeof message === 'string' ? message : JSON.stringify(message);
            this.ws.send(messageString);
            console.log('Message sent to server:', message);
            return true;
        } catch (error) {
            console.error('Error sending message:', error);
            return false;
        }
    }
    
    
    // Reject all pending acknowledgments (on connection error/close)
    rejectPendingAcks(reason) {
        for (const [messageId, pending] of this.pendingAcks) {
            clearTimeout(pending.timeout);
            pending.reject(new Error(`WebSocket operation cancelled: ${reason}`));
        }
        this.pendingAcks.clear();
    }
    
    // Register for canvas events
    registerForCanvas(canvasId) {
        return this.send({
            type: 'registerForCanvas',
            canvasId: canvasId
        });
    }
    
    // Unregister from canvas events with Promise support for reliable cleanup
    unregisterForCanvas(canvasId) {
        // Use fallback approach since server doesn't support acknowledgments yet
        return this.sendWithFallback({
            type: 'unregisterForCanvas',
            canvasId: canvasId
        }, 1000); // 1 second delay for server processing
    }
    
    // Send canvas events to server
    sendCanvasEvents(canvasId, events) {
        return this.send({
            type: 'canvasEvent',
            canvasId: canvasId,
            eventsForCanvas: events
        });
    }
    
    
    // Schedule reconnect attempt
    scheduleReconnect() {
        if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            console.error('Max reconnect attempts reached. Giving up.');
            this.emit('reconnectFailed');
            return;
        }
        
        this.reconnectAttempts++;
        const delay = Math.min(this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1), 30000);
        
        console.log(`Scheduling reconnect attempt ${this.reconnectAttempts} in ${delay}ms`);
        
        setTimeout(() => {
            this.connect();
        }, delay);
    }
    
    // Start heartbeat mechanism
    startHeartbeat() {
        this.stopHeartbeat();
        this.lastHeartbeat = Date.now();
        
        this.heartbeatInterval = setInterval(() => {
            if (this.isConnected && this.ws.readyState === WebSocket.OPEN) {
                // Send ping
                this.send({ type: 'ping', timestamp: Date.now() });
                
                // Check if we received a recent pong (more lenient timeout)
                const now = Date.now();
                if (this.lastHeartbeat && (now - this.lastHeartbeat > 45000)) {
                    console.warn('Heartbeat timeout - connection may be dead, attempting reconnect');
                    // Don't immediately close, try one more ping first
                    this.send({ type: 'ping', timestamp: Date.now() });
                    
                    // Give extra time before closing
                    setTimeout(() => {
                        if (this.isConnected && (Date.now() - this.lastHeartbeat > 60000)) {
                            console.error('Heartbeat completely failed, closing connection');
                            this.ws.close();
                        }
                    }, 15000);
                }
            }
        }, 15000); // Send heartbeat every 15 seconds
    }
    
    // Stop heartbeat mechanism
    stopHeartbeat() {
        if (this.heartbeatInterval) {
            clearInterval(this.heartbeatInterval);
            this.heartbeatInterval = null;
        }
    }
    
    // Close connection
    close() {
        console.log('Closing WebSocket connection');
        this.stopHeartbeat();
        
        if (this.ws) {
            this.ws.close(1000, 'Client disconnect');
        }
        
        this.changeState(WebSocketStates.DISCONNECTED);
        this.ws = null;
    }
    
    // Event handler management
    on(event, handler) {
        if (!this.messageHandlers[event]) {
            this.messageHandlers[event] = [];
        }
        this.messageHandlers[event].push(handler);
    }
    
    off(event, handler) {
        if (!this.messageHandlers[event]) return;
        
        const index = this.messageHandlers[event].indexOf(handler);
        if (index > -1) {
            this.messageHandlers[event].splice(index, 1);
        }
    }
    
    emit(event, data) {
        if (!this.messageHandlers[event]) return;
        
        this.messageHandlers[event].forEach(handler => {
            try {
                handler(data);
            } catch (error) {
                console.error(`Error in ${event} handler:`, error);
            }
        });
    }
    
    // Get connection status
    getStatus() {
        return {
            state: this.state,
            stateChangedAt: this.stateChangedAt,
            isConnected: this.isConnected,
            readyState: this.ws ? this.ws.readyState : WebSocket.CLOSED,
            reconnectAttempts: this.reconnectAttempts,
            lastHeartbeat: this.lastHeartbeat
        };
    }
}

// Singleton instance management
let webSocketClientInstance = null;

// WebSocket Client Singleton Factory
window.getWebSocketClient = function() {
    if (!webSocketClientInstance) {
        console.log('Creating WebSocket client singleton instance...');
        webSocketClientInstance = new WebSocketClient();
        
        // Auto-connect on first access if not already connected
        if (!webSocketClientInstance.isConnected) {
            webSocketClientInstance.connect();
        }
        
        // Dispatch ready event
        console.log('WebSocket client singleton ready, dispatching ready event');
        window.dispatchEvent(new Event('websocket-client-ready'));
    }
    
    return webSocketClientInstance;
};

// Initialize singleton and make it available globally for backward compatibility
if (!window.webSocketClient) {
    window.webSocketClient = window.getWebSocketClient();
} else {
    console.log('WebSocket client already exists, skipping re-initialization');
}

} // End of duplicate check