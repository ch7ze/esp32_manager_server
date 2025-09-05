// Canvas WebSocket Bridge - Connects EventBus to WebSocket for Multi-User Canvas
// Handles bidirectional communication between local EventBus and server WebSocket

// Check if already loaded to prevent duplicate class declarations
if (typeof CanvasWebSocketBridge !== 'undefined') {
    console.log('CanvasWebSocketBridge already loaded, skipping redefinition');
} else {

class CanvasWebSocketBridge {
    constructor() {
        this.currentCanvasId = null;
        this.isInitialized = false;
        this.eventQueueForServer = [];
        this.isConnected = false;
        this.cleanupHandlers = new Set();
        this.webSocketClient = null;
        this.eventHandlersSetup = false;
        this.pendingCanvasRegistration = null;
        
        // Event caching for incremental replay
        this.eventCache = new Map(); // canvasId -> { events: [], lastTimestamp: number }
        this.lastEventTimestamps = new Map(); // canvasId -> timestamp
        
        // Event batching for performance optimization
        this.eventBatch = [];
        this.batchTimeout = null;
        this.isDragOperation = false;
        this.BATCH_DELAY_MS = 200; // Increased to 200ms for better batching
        this.MAX_BATCH_SIZE = 10; // Maximum events per batch
        
        // Server message queue instead of throttling
        this.serverMessageQueue = [];
        this.isProcessingServerMessages = false;
        
        // Canvas redraw throttling
        this.redrawThrottleId = null;
        this.REDRAW_THROTTLE_MS = 16; // ~60fps max
        
        console.log('CanvasWebSocketBridge created');
        
        // Multi-tab debugging info
        this.logMultiTabInfo();
        
        // Setup automatic cleanup for page navigation
        this.setupNavigationCleanup();
        
        // CRITICAL FIX: Listen for Event-System reload to re-register handlers
        this.setupEventSystemReloadListener();
        
        // Wait for dependencies to be ready
        this.initializeWhenReady();
    }
    
    // Setup navigation cleanup handlers
    setupNavigationCleanup() {
        const cleanup = () => {
            console.log('Bridge: Performing automatic cleanup on navigation');
            this.cleanup();
        };
        
        // Add cleanup handlers
        window.addEventListener('beforeunload', cleanup);
        window.addEventListener('pagehide', cleanup);
        
        // Store cleanup handlers for removal
        this.cleanupHandlers.add(() => {
            window.removeEventListener('beforeunload', cleanup);
            window.removeEventListener('pagehide', cleanup);
        });
        
        console.log('Bridge: Navigation cleanup handlers registered');
    }
    
    // Setup Event-System reload listener - CRITICAL FIX for SPA navigation
    setupEventSystemReloadListener() {
        const handleEventSystemReady = () => {
            console.log('Bridge: Event system reloaded detected, re-ensuring event handlers');
            
            // Reset the flag so ensureEventHandlersSetup will re-register
            this.eventHandlersSetup = false;
            
            // Re-setup event handlers with the new EventBus instance
            this.ensureEventHandlersSetup();
        };
        
        // Listen for the drawer-event-system-ready event
        window.addEventListener('drawer-event-system-ready', handleEventSystemReady);
        
        // Add to cleanup handlers so it gets removed on navigation
        this.cleanupHandlers.add(() => {
            window.removeEventListener('drawer-event-system-ready', handleEventSystemReady);
        });
        
        console.log('Bridge: Event-System reload listener registered');
    }
    
    // Log multi-tab session info for debugging
    logMultiTabInfo() {
        const sessionId = sessionStorage.getItem('canvas_session_id') || 'unknown';
        const tabCount = parseInt(localStorage.getItem('active_tab_count') || '0') + 1;
        localStorage.setItem('active_tab_count', tabCount.toString());
        
        if (!sessionStorage.getItem('canvas_session_id')) {
            sessionStorage.setItem('canvas_session_id', Math.random().toString(36).substr(2, 9));
        }
        
        console.log(`Multi-Tab Info: Session ${sessionId}, Estimated tabs: ${tabCount}`);
        
        // Clean up tab count on unload
        window.addEventListener('beforeunload', () => {
            const currentCount = parseInt(localStorage.getItem('active_tab_count') || '1');
            if (currentCount > 1) {
                localStorage.setItem('active_tab_count', (currentCount - 1).toString());
            } else {
                localStorage.removeItem('active_tab_count');
            }
        });
    }
    
    // Comprehensive cleanup method
    cleanup() {
        console.log('Bridge: Starting comprehensive cleanup...');
        
        // Unregister from current canvas
        if (this.currentCanvasId) {
            this.unregisterFromCanvas();
        }
        
        // Clear event queue and cache
        this.eventQueueForServer = [];
        this.eventCache.clear();
        this.lastEventTimestamps.clear();
        
        // Clear server message queue
        this.serverMessageQueue = [];
        this.isProcessingServerMessages = false;
        
        if (this.redrawThrottleId) {
            clearTimeout(this.redrawThrottleId);
            this.redrawThrottleId = null;
        }
        
        // Clear cycle detection timers
        if (this.registrationResetTimer) {
            clearTimeout(this.registrationResetTimer);
            this.registrationResetTimer = null;
        }
        
        // Clear cycle detection data
        this.lastFullReplayTimes = [];
        this.lastResetTime = null;
        this.isCanvasRegistration = false;
        
        // Remove all registered cleanup handlers
        this.cleanupHandlers.forEach(handler => {
            try {
                handler();
            } catch (error) {
                console.error('Error during cleanup handler execution:', error);
            }
        });
        this.cleanupHandlers.clear();
        
        // Clean up WebSocket handlers
        if (this.webSocketClient) {
            // Note: Don't close WebSocket connection as it may be used by other components
            console.log('Bridge: WebSocket client cleanup (keeping connection alive for reuse)');
        }
        
        // Flush any pending batches before cleanup
        if (this.batchTimeout) {
            clearTimeout(this.batchTimeout);
            this.batchTimeout = null;
        }
        this.flushBatch(); // Send any remaining events
        
        console.log('Bridge: Cleanup complete');
    }
    
    // Initialize when both EventBus and WebSocket client are ready - Promise-based
    async initializeWhenReady() {
        console.log('Waiting for dependencies...');
        
        try {
            // Wait for EventBus to be ready
            await this.waitForEventBus();
            
            // Get or create WebSocket client singleton
            const webSocketClient = window.getWebSocketClient ? window.getWebSocketClient() : window.webSocketClient;
            if (!webSocketClient) {
                throw new Error('WebSocket client not available');
            }
            
            await this.initialize();
            
        } catch (error) {
            console.error('Failed to initialize Canvas WebSocket Bridge:', error);
            // Retry using requestAnimationFrame for next tick
            requestAnimationFrame(() => this.initializeWhenReady());
        }
    }
    
    // Wait for EventBus to be available
    waitForEventBus() {
        return new Promise((resolve) => {
            if (window.eventBus) {
                resolve();
                return;
            }
            
            const checkEventBus = () => {
                if (window.eventBus) {
                    resolve();
                } else {
                    requestAnimationFrame(checkEventBus);
                }
            };
            
            checkEventBus();
        });
    }
    
    // Initialize the bridge - Promise-based
    async initialize() {
        if (this.isInitialized) return;
        
        console.log('Initializing Canvas WebSocket Bridge...');
        
        // Use singleton WebSocket client
        this.webSocketClient = window.getWebSocketClient ? window.getWebSocketClient() : window.webSocketClient;
        
        // Setup WebSocket event handlers
        this.setupWebSocketHandlers();
        
        // Setup EventBus event handlers
        this.setupEventBusHandlers();
        
        // Connect WebSocket with promise support - FIXED: Don't claim success until real connection
        try {
            if (this.webSocketClient.connect) {
                await this.webSocketClient.connect();
                console.log('Canvas WebSocket Bridge: Connection process started');
            }
        } catch (error) {
            console.error('Failed to connect WebSocket in bridge:', error);
            // Don't fail initialization, connection can be retried later
        }
        
        this.isInitialized = true;
        // FIXED: Only log "initialized", not "connected" - connection status is separate
        console.log('Canvas WebSocket Bridge initialized (waiting for connection)');
    }
    
    // Setup WebSocket event handlers
    setupWebSocketHandlers() {
        // Handle connection events
        this.webSocketClient.on('connected', () => {
            console.log('Bridge: WebSocket connected - now ready for canvas registration');
            this.isConnected = true;
            
            // Re-register for current canvas if we have one
            if (this.currentCanvasId) {
                console.log(`Bridge: Auto-registering for canvas ${this.currentCanvasId} after connection`);
                this.registerForCanvas(this.currentCanvasId);
            }
            
            // Also handle any pending registration
            if (this.pendingCanvasRegistration) {
                console.log(`Bridge: Processing pending registration for canvas ${this.pendingCanvasRegistration}`);
                this.registerForCanvas(this.pendingCanvasRegistration);
                this.pendingCanvasRegistration = null;
            }
            
            // Send queued events
            this.sendQueuedEvents();
        });
        
        this.webSocketClient.on('disconnected', () => {
            console.log('Bridge: WebSocket disconnected');
            this.isConnected = false;
            
            // Clear event batch on disconnect to prevent memory leaks
            this.clearEventBatch();
        });
        
        this.webSocketClient.on('error', () => {
            console.log('Bridge: WebSocket error occurred');
            this.isConnected = false;
            
            // Clear event batch on error to prevent memory leaks
            this.clearEventBatch();
        });
        
        // Handle server messages (canvas events from other clients)
        this.webSocketClient.on('serverMessage', (serverMessage) => {
            this.queueServerMessage(serverMessage);
        });
        
        this.webSocketClient.on('error', (error) => {
            console.error('Bridge: WebSocket error:', error);
        });
    }
    
    // Setup EventBus event handlers
    setupEventBusHandlers() {
        if (this.eventHandlersSetup) {
            console.log('Bridge: Event handlers already set up, skipping');
            return;
        }
        
        console.log('Bridge: Setting up event handlers for EventBus communication');
        
        // Listen for replay completion to reset registration flags
        this.setupRegistrationCompletionEvent();
        
        // Listen for server acknowledgments
        this.setupServerAcknowledgmentHandling();
        
        // Listen for local events that should be sent to server
        window.eventBus.subscribe('SHAPE_CREATED', (event) => {
            console.log('BRIDGE: Received SHAPE_CREATED event from EventBus:', event);
            if (!event.isFromServer && !event.isReplay) {
                console.log('BRIDGE: Sending SHAPE_CREATED to server...');
                this.sendEventToServer(event);
            } else {
                console.log('BRIDGE: Skipping SHAPE_CREATED (isFromServer:', event.isFromServer, 'isReplay:', event.isReplay, ')');
            }
        });
        
        window.eventBus.subscribe('SHAPE_DELETED', (event) => {
            if (!event.isFromServer && !event.isReplay) {
                this.sendEventToServer(event);
            }
        });
        
        window.eventBus.subscribe('SHAPE_MODIFIED', (event) => {
            if (!event.isFromServer && !event.isReplay) {
                this.sendEventToServer(event);
            }
        });
        
        window.eventBus.subscribe('SHAPE_SELECTED', (event) => {
            if (!event.isFromServer && !event.isReplay) {
                this.sendEventToServer(event);
            }
        });
        
        window.eventBus.subscribe('SHAPE_UNSELECTED', (event) => {
            if (!event.isFromServer && !event.isReplay) {
                this.sendEventToServer(event);
            }
        });
        
        // Mark event handlers as set up
        this.eventHandlersSetup = true;
        console.log('Bridge: Event handlers setup completed');
    }
    
    // Ensure event handlers are set up (idempotent) - BEST PRACTICE: Defensive programming
    ensureEventHandlersSetup() {
        if (this.eventHandlersSetup) {
            console.log('Bridge: Event handlers already confirmed set up');
            return;
        }
        
        if (!window.eventBus) {
            console.log('Bridge: EventBus not available, deferring event handler setup');
            return;
        }
        
        console.log('Bridge: Ensuring event handlers are set up for SPA navigation');
        this.setupEventBusHandlers();
    }
    
    // Setup registration completion event listener
    setupRegistrationCompletionEvent() {
        // Listen for replay-complete event to reset canvas registration flag
        window.eventBus.subscribe('REPLAY_COMPLETED', (event) => {
            if (event.canvasId === this.currentCanvasId) {
                console.log('Bridge: Registration replay complete for canvas', event.canvasId);
                this.isCanvasRegistration = false;
                
                // Clear any pending registration reset timer
                if (this.registrationResetTimer) {
                    clearTimeout(this.registrationResetTimer);
                    this.registrationResetTimer = null;
                }
            }
        });
        
        // Listen for connection-stable event to ensure we're properly connected
        if (this.webSocketClient && this.webSocketClient.on) {
            this.webSocketClient.on('connection-stable', () => {
                console.log('Bridge: WebSocket connection is stable');
                // Connection is stable, we can trust it for operations
            });
        }
    }
    
    // Setup server acknowledgment handling
    setupServerAcknowledgmentHandling() {
        // Handle registration complete acknowledgments from server
        window.eventBus.subscribe('REGISTRATION_COMPLETE', (event) => {
            console.log('Bridge: Received registration complete acknowledgment:', event);
            
            if (event.canvasId === this.currentCanvasId) {
                this.isCanvasRegistration = false;
                
                // Clear any pending registration reset timer
                if (this.registrationResetTimer) {
                    clearTimeout(this.registrationResetTimer);
                    this.registrationResetTimer = null;
                }
                
                console.log(`Bridge: Registration confirmed for canvas ${event.canvasId} with ${event.eventCount} events`);
            }
        });
    }
    
    // Register for canvas events
    registerForCanvas(canvasId) {
        console.log(`Bridge: Registering for canvas ${canvasId}`);
        
        // Unregister from previous canvas if we had one
        if (this.currentCanvasId && this.currentCanvasId !== canvasId) {
            console.log(`Bridge: Switching from canvas ${this.currentCanvasId} to ${canvasId}`);
            this.unregisterFromCanvas();
        }
        
        const wasNewCanvas = !this.currentCanvasId || this.currentCanvasId !== canvasId;
        this.currentCanvasId = canvasId;
        
        // Mark that we're registering for a new canvas (always need full replay)
        this.isCanvasRegistration = true;
        
        // Sync connection state with WebSocket client  
        if (this.webSocketClient && this.webSocketClient.isConnected && !this.isConnected) {
            console.log('Bridge: Syncing connection state with WebSocket client');
            this.isConnected = true;
        }
        
        if (this.isConnected) {
            // Use atomic registration for better reliability
            if (this.webSocketClient.atomicRegisterWithReplay) {
                console.log(`Bridge: Using atomic registration for canvas ${canvasId}`);
                this.webSocketClient.atomicRegisterWithReplay(canvasId);
            } else {
                console.log('Bridge: Using legacy registration for canvas', canvasId);
                this.webSocketClient.registerForCanvas(canvasId);
                
                // Force a fresh event replay if switching canvas (even if connected)
                if (wasNewCanvas) {
                    console.log(`Bridge: Forcing fresh server state for canvas switch to ${canvasId}`);
                    this.requestCanvasState(canvasId);
                }
            }
        } else {
            console.log('Bridge: Not connected, will register when connected');
            
            // Store the canvas ID for retry when connected
            this.pendingCanvasRegistration = canvasId;
            
            // Also try to reconnect if connection was lost
            if (this.webSocketClient && !this.webSocketClient.isConnected) {
                console.log('Bridge: Attempting to reconnect WebSocket...');
                this.webSocketClient.connect();
            }
        }
    }
    
    // Request fresh canvas state from server (for SPA navigation)
    requestCanvasState(canvasId) {
        if (this.isConnected && this.webSocketClient) {
            console.log(`Bridge: Requesting atomic fresh state for canvas ${canvasId}`);
            
            // ATOMIC REGISTRATION: Use atomic operation instead of unregister/register cycle
            // This eliminates the race condition window entirely
            if (this.webSocketClient.atomicRegisterWithReplay) {
                console.log(`Bridge: Using atomic registration for canvas ${canvasId}`);
                return this.webSocketClient.atomicRegisterWithReplay(canvasId);
            } else {
                // Fallback to old method if atomic registration is not available  
                console.log('Bridge: Atomic registration not available, using single registration');
                // Single registration is sufficient - no need for double registration
                // The initial registerForCanvas() call already handled the registration
            }
        }
    }
    
    // Unregister from current canvas with Promise support for reliable cleanup
    async unregisterFromCanvas() {
        if (!this.currentCanvasId) {
            console.log('Bridge: No canvas to unregister from');
            return Promise.resolve();
        }
        
        const canvasId = this.currentCanvasId;
        console.log(`Bridge: Unregistering from canvas ${canvasId}`);
        
        // FIRST: Clear all shape selections before unregistering
        if (window.canvas && window.canvas.selectionTool && window.canvas.selectionTool.clearSelection) {
            console.log('Bridge: Clearing all shape selections before unregistering from canvas');
            window.canvas.selectionTool.clearSelection();
            // Trigger a canvas redraw to show the deselection visually
            if (window.canvas.draw) {
                window.canvas.draw();
            }
        } else {
            console.warn('Bridge: Selection tool not available for cleanup');
        }
        
        try {
            if (this.isConnected && this.webSocketClient) {
                console.log('Bridge: Sending unregister message to server...');
                await this.webSocketClient.unregisterForCanvas(canvasId);
                console.log('Bridge: Canvas unregistration confirmed by server');
            } else {
                console.warn('Bridge: WebSocket not connected, skipping server unregister');
            }
        } catch (error) {
            console.error('Bridge: Failed to unregister from canvas:', error);
            // Don't throw - we want cleanup to continue even if server communication fails
        } finally {
            this.currentCanvasId = null;
        }
    }
    
    // Queue server message for processing
    queueServerMessage(serverMessage) {
        console.log('Bridge: Queueing server message:', serverMessage);
        
        if (!serverMessage.canvasId || !serverMessage.eventsForCanvas) {
            console.warn('Bridge: Invalid server message format');
            return;
        }
        
        // Add to queue
        this.serverMessageQueue.push(serverMessage);
        
        // Start processing if not already processing
        this.processServerMessageQueue();
    }
    
    // Process server message queue sequentially
    async processServerMessageQueue() {
        if (this.serverMessageQueue.length === 0 || this.isProcessingServerMessages) {
            return;
        }
        
        this.isProcessingServerMessages = true;
        const serverMessage = this.serverMessageQueue.shift();
        
        console.log('Bridge: Processing server message from queue:', serverMessage);
        
        try {
            await this.handleServerMessage(serverMessage);
        } catch (error) {
            console.error('Error processing server message:', error);
        } finally {
            this.isProcessingServerMessages = false;
            // Process next message on next animation frame
            if (this.serverMessageQueue.length > 0) {
                requestAnimationFrame(() => this.processServerMessageQueue());
            }
        }
    }

    // Handle server messages (events from other clients)
    async handleServerMessage(serverMessage) {
        console.log('Bridge: Processing server message:', serverMessage);
        console.log('DEBUG: Current canvas ID:', this.currentCanvasId);
        console.log('DEBUG: EventStore available:', !!window.eventStore);
        console.log('DEBUG: replayFromServer method available:', !!(window.eventStore && window.eventStore.replayFromServer));
        
        // Only process events for our current canvas
        if (serverMessage.canvasId !== this.currentCanvasId) {
            console.log(`Bridge: Ignoring events for canvas ${serverMessage.canvasId}, current is ${this.currentCanvasId}`);
            return;
        }
        
        console.log(`Bridge: Processing ${serverMessage.eventsForCanvas.length} events for canvas ${serverMessage.canvasId}`);
        
        // Update event cache for incremental loading
        this.updateEventCache(serverMessage.canvasId, serverMessage.eventsForCanvas);
        
        // Check if this is an incremental update or full reload
        const isIncrementalUpdate = this.isIncrementalUpdate(serverMessage.canvasId, serverMessage.eventsForCanvas);
        
        if (isIncrementalUpdate) {
            console.log('Bridge: Performing incremental event update');
            this.processIncrementalEvents(serverMessage.eventsForCanvas, serverMessage.canvasId);
        } else {
            console.log('Bridge: Performing full event replay');
            this.processFullEventReplay(serverMessage.eventsForCanvas, serverMessage.canvasId);
        }
    }
    
    // Update event cache with new events
    updateEventCache(canvasId, events) {
        if (!this.eventCache.has(canvasId)) {
            this.eventCache.set(canvasId, { events: [], lastTimestamp: 0 });
        }
        
        const cache = this.eventCache.get(canvasId);
        
        // Find newest event timestamp (optimized with early return)
        let newestTimestamp = cache.lastTimestamp;
        for (const event of events) {
            if (event.timestamp && event.timestamp > newestTimestamp) {
                newestTimestamp = event.timestamp;
            }
        }
        
        // Add new events to cache (deduplicated)
        const existingEventIds = new Set(cache.events.map(e => 
            e.shape ? e.shape.id : e.shapeId
        ).filter(Boolean));
        
        const newEvents = events.filter(event => {
            const eventId = event.shape ? event.shape.id : event.shapeId;
            return !eventId || !existingEventIds.has(eventId);
        });
        
        cache.events.push(...newEvents);
        cache.lastTimestamp = newestTimestamp;
        
        // Use more efficient cache management with circular buffer concept
        const maxCacheSize = window._performanceConfig?.maxEventCacheSize || 500;
        if (cache.events.length > maxCacheSize) {
            // Remove oldest 25% to avoid frequent resizing
            const removeCount = Math.floor(maxCacheSize * 0.25);
            cache.events.splice(0, removeCount);
        }
        
        if (!window._isProduction) {
            console.log(`Bridge: Updated event cache for ${canvasId}: ${cache.events.length} events (${newEvents.length} new), newest: ${newestTimestamp}`);
        }
    }
    
    // Check if this is an incremental update
    isIncrementalUpdate(canvasId, events) {
        // RESET_STATE CYCLE FIX: Check if we're in a vicious cycle
        if (this.lastResetTime && (Date.now() - this.lastResetTime) < 5000) {
            console.log('Bridge: Recent RESET_STATE detected, forcing incremental mode to break cycle');
            return true; // Force incremental to break the vicious cycle
        }
        
        // If we just registered for this canvas, always do full replay 
        // Flag will be reset by REGISTRATION_COMPLETE or REPLAY_COMPLETE events
        if (this.isCanvasRegistration) {
            console.log('Bridge: Canvas registration detected, forcing full replay (waiting for completion event)');
            return false;
        }
        
        const cache = this.eventCache.get(canvasId);
        if (!cache || cache.events.length === 0) {
            console.log('Bridge: No cache or empty cache, need full replay');
            return false; // First load, need full replay
        }
        
        // ADVANCED CYCLE DETECTION: Detect rapid full replays (sign of vicious cycle)
        const now = Date.now();
        if (!this.lastFullReplayTimes) {
            this.lastFullReplayTimes = [];
        }
        
        // Clean old timestamps (older than 10 seconds)
        this.lastFullReplayTimes = this.lastFullReplayTimes.filter(time => now - time < 10000);
        
        // If we've had more than 3 full replays in the last 10 seconds, switch to incremental
        if (this.lastFullReplayTimes.length >= 3) {
            console.log('Bridge: Detected rapid full replay cycle (', this.lastFullReplayTimes.length, 'replays), forcing incremental mode');
            return true; // Force incremental to break rapid replay cycle
        }
        
        // FIXED: Much more permissive incremental detection to prevent unnecessary RESET_STATE
        const addShapeEvents = events.filter(e => e.event === 'addShape');
        const removeShapeEvents = events.filter(e => e.event === 'removeShape');
        const modifyShapeEvents = events.filter(e => e.event === 'modifyShape');
        const selectEvents = events.filter(e => e.event === 'selectShape' || e.event === 'unselectShape');
        const userEvents = events.filter(e => e.event === 'userJoined' || e.event === 'userLeft');
        
        // Count all known, safe-to-process events
        const totalSafeEvents = addShapeEvents.length + removeShapeEvents.length + 
                               modifyShapeEvents.length + selectEvents.length + userEvents.length;
        
        // MAJOR FIX: Default to incremental unless there's a specific reason for full replay
        // Only force full replay for truly complex operations or unknown event types
        const hasOnlyKnownSafeEvents = events.length === totalSafeEvents;
        const isReasonableSize = events.length <= 50; // Much higher threshold
        
        // Use incremental processing for all normal drawing operations
        const isIncremental = hasOnlyKnownSafeEvents && isReasonableSize && events.length > 0;
        
        console.log(`Bridge: FIXED Incremental check - events: ${events.length}, adds: ${addShapeEvents.length}, removes: ${removeShapeEvents.length}, modifies: ${modifyShapeEvents.length}, selects: ${selectEvents.length}, users: ${userEvents.length}, safeEvents: ${totalSafeEvents}, result: ${isIncremental}`);
        
        return isIncremental;
    }
    
    // Process incremental events (just add to existing state)
    processIncrementalEvents(events, canvasId) {
        // For incremental events, bypass replayFromServer to avoid RESET_STATE
        // Send events directly to EventBus
        if (window.eventStore && window.eventBus) {
            console.log('Bridge: Processing incremental events directly via EventBus (no reset)');
            
            // Set replay flag to prevent sending back to server
            window._isReplaying = true;
            
            events.forEach(serverEvent => {
                // Handle special user events
                if (this.isUserEvent(serverEvent)) {
                    this.handleUserEvent(serverEvent, canvasId);
                    return;
                }
                
                const eventBusEvent = window.eventStore.convertServerEventToEventBus(serverEvent);
                if (eventBusEvent && window.eventStore.validateEventForReplay(eventBusEvent)) {
                    console.log('Direct incremental event:', eventBusEvent.type, eventBusEvent);
                    window.eventBus.publish({
                        ...eventBusEvent, 
                        isReplay: true, 
                        isFromServer: true, 
                        canvasId: canvasId
                    });
                }
            });
            
            // Clear replay flag
            window._isReplaying = false;
            
            // Trigger redraw
            this.throttledCanvasRedraw();
        } else {
            console.warn('EventStore or EventBus not available for incremental processing');
        }
    }
    
    // Process full event replay (clear and reload all)
    processFullEventReplay(events, canvasId) {
        if (window.eventStore && window.eventStore.replayFromServer) {
            console.log('Bridge: Calling eventStore.replayFromServer for full reload...');
            
            // CYCLE DETECTION: Track full replay timing
            const now = Date.now();
            if (!this.lastFullReplayTimes) {
                this.lastFullReplayTimes = [];
            }
            this.lastFullReplayTimes.push(now);
            
            // Also track RESET_STATE timing for cycle detection
            this.lastResetTime = now;
            
            // Separate user events from canvas events
            const { userEvents, canvasEvents } = this.separateUserAndCanvasEvents(events);
            
            // Process user events immediately
            userEvents.forEach(userEvent => {
                this.handleUserEvent(userEvent, canvasId);
            });
            
            // Deduplicate canvas events by shapeId and timestamp to prevent duplicates
            const deduplicatedEvents = this.deduplicateEvents(canvasEvents);
            if (deduplicatedEvents.length !== canvasEvents.length) {
                console.log(`Bridge: Deduplicated ${canvasEvents.length - deduplicatedEvents.length} duplicate canvas events`);
            }
            
            // CRITICAL FIX: Only perform full replay if there are actual canvas events
            // This prevents RESET_STATE from being triggered by pure user events (userJoined/userLeft)
            if (deduplicatedEvents.length > 0) {
                // Create callback to redraw canvas after replay is complete
                const onReplayComplete = () => {
                    console.log('Bridge: Full replay complete, triggering throttled canvas redraw');
                    this.throttledCanvasRedraw();
                    
                    // CRITICAL FIX: Reset canvas registration flag after successful full replay
                    console.log('Bridge: Full replay complete - resetting isCanvasRegistration flag');
                    this.isCanvasRegistration = false;
                    
                    // Clear reset time immediately after successful replay
                    if (this.lastResetTime === now) { // Only clear if this is still the most recent reset
                        console.log('Bridge: Clearing RESET_STATE timestamp after successful replay');
                        this.lastResetTime = null;
                    }
                };
                
                window.eventStore.replayFromServer(deduplicatedEvents, canvasId, onReplayComplete, true);
            } else {
                console.log('Bridge: Skipping full replay - no canvas events to process (only user events)');
                // No need for RESET_STATE when there are no canvas events to replay
                // Just ensure canvas redraw is triggered if needed
                this.throttledCanvasRedraw();
            }
        } else {
            console.error('Bridge: EventStore not available for replay', {
                eventStore: !!window.eventStore,
                replayFromServer: !!(window.eventStore && window.eventStore.replayFromServer)
            });
        }
    }
    
    // ENHANCED: Deduplicate events to prevent duplicate shape creation and state conflicts
    deduplicateEvents(events) {
        const seenEvents = new Map(); // "eventType:shapeId:property" -> latest event
        const userEvents = [];
        const result = [];
        
        // Sort events by timestamp first for proper deduplication
        const sortedEvents = [...events].sort((a, b) => (a.timestamp || 0) - (b.timestamp || 0));
        
        sortedEvents.forEach(event => {
            // Handle user events separately (always keep them)
            if (event.event === 'userJoined' || event.event === 'userLeft') {
                userEvents.push(event);
                console.log(`Enhanced Dedup: Keeping user event: ${event.event} (${event.displayName})`);
                return;
            }
            
            let key = event.event;
            
            // Create more specific keys for different event types
            if (event.event === 'addShape' && event.shape?.id) {
                key = `addShape:${event.shape.id}`;
                console.log(`Enhanced Dedup: Processing addShape for ${event.shape.id}`);
            } else if (event.event === 'removeShape' && event.shapeId) {
                key = `removeShape:${event.shapeId}`;
                console.log(`Enhanced Dedup: Processing removeShape for ${event.shapeId}`);
            } else if (event.event === 'modifyShape' && event.shapeId && event.property) {
                // Each property modification is tracked separately
                key = `modifyShape:${event.shapeId}:${event.property}`;
                console.log(`Enhanced Dedup: Processing modifyShape for ${event.shapeId}.${event.property}`);
            } else if (event.event === 'selectShape' || event.event === 'unselectShape') {
                // Selection events per shape per client
                const clientId = event.clientId || 'unknown';
                key = `${event.event}:${event.shapeId}:${clientId}`;
                console.log(`Enhanced Dedup: Processing ${event.event} for ${event.shapeId} by ${clientId}`);
            } else {
                // Unknown events - keep all
                result.push(event);
                console.log(`Enhanced Dedup: Keeping unknown event: ${event.event}`);
                return;
            }
            
            // Keep only latest event for each key
            if (!seenEvents.has(key) || (event.timestamp || 0) > (seenEvents.get(key).timestamp || 0)) {
                const oldEvent = seenEvents.get(key);
                seenEvents.set(key, event);
                
                if (oldEvent) {
                    console.log(`Enhanced Dedup: Replaced older ${key} (old: ${oldEvent.timestamp || 0}, new: ${event.timestamp || 0})`);
                } else {
                    console.log(`Enhanced Dedup: Added new ${key} (timestamp: ${event.timestamp || 0})`);
                }
            } else {
                console.log(`Enhanced Dedup: Dropped older ${key} (timestamp: ${event.timestamp || 0})`);
            }
        });
        
        // Combine all deduplicated events
        result.push(...Array.from(seenEvents.values()));
        result.push(...userEvents);
        
        // Sort final result by timestamp
        const finalResult = result.sort((a, b) => (a.timestamp || 0) - (b.timestamp || 0));
        
        const originalCount = events.length;
        const deduplicatedCount = finalResult.length;
        const removedCount = originalCount - deduplicatedCount;
        
        if (removedCount > 0) {
            console.log(`Enhanced Dedup: Removed ${removedCount} duplicates (${originalCount} → ${deduplicatedCount} events)`);
        } else {
            console.log(`Enhanced Dedup: No duplicates found (${originalCount} events)`);
        }
        
        return finalResult;
    }
    
    // Throttled canvas redraw for performance
    throttledCanvasRedraw() {
        if (this.redrawThrottleId) {
            return; // Already scheduled
        }
        
        this.redrawThrottleId = requestAnimationFrame(() => {
            this.redrawThrottleId = null;
            
            if (window.canvas && window.canvas.draw) {
                window.canvas.draw();
                console.log('Bridge: Throttled canvas redraw completed');
            } else {
                console.warn('Bridge: Canvas not available for redraw');
            }
        });
    }
    
    // Send local event to server
    sendEventToServer(event) {
        if (!this.currentCanvasId) {
            console.warn('Bridge: Cannot send event - no canvas registered');
            return;
        }
        
        // Convert EventBus event to server format
        const serverEvent = this.convertEventBusEventToServer(event);
        if (!serverEvent) {
            console.warn('Bridge: Could not convert event to server format:', event);
            return;
        }
        
        console.log('Bridge: Converting and sending event to server:', event.type, '→', serverEvent);
        
        // Critical events that should never be batched (especially color changes)
        const isCriticalEvent = event.type === 'SHAPE_CREATED' || 
                              event.type === 'SHAPE_DELETED' ||
                              (event.type === 'SHAPE_MODIFIED' && 
                               (event.property === 'fillColor' || event.property === 'strokeColor'));
        
        // Determine if this is part of a drag operation
        const isDragRelated = this.isDragRelatedEvent(event);
        
        // Send immediately for critical events or when not connected, otherwise batch only true drag events
        if (!this.isConnected) {
            console.log('Bridge: Queueing event for later send');
            this.eventQueueForServer.push({ canvasId: this.currentCanvasId, event: serverEvent });
        } else if (isCriticalEvent) {
            // Always send color and shape changes immediately to prevent state corruption
            console.log('Bridge: Sending critical event immediately:', event.type, event.property);
            this.webSocketClient.sendCanvasEvents(this.currentCanvasId, [serverEvent]);
        } else if (isDragRelated && this.shouldBatchEvent(event)) {
            this.addToBatch(serverEvent);
        } else {
            // Send immediately for non-drag events
            this.webSocketClient.sendCanvasEvents(this.currentCanvasId, [serverEvent]);
        }
    }
    
    // Check if event is related to drag operations
    isDragRelatedEvent(event) {
        // Only consider position-related events as drag-related, NOT color changes
        if (event.type === 'SHAPE_MODIFIED') {
            return event.property === 'position' || event.property === 'x' || event.property === 'y';
        }
        // Shape creation/deletion during drag operations
        if (event.type === 'SHAPE_CREATED' || event.type === 'SHAPE_DELETED') {
            // Check if this is happening during a drag operation
            return this.isDragOperation;
        }
        return false;
    }
    
    // Determine if event should be batched
    shouldBatchEvent(event) {
        // Only batch during detected drag operations or rapid changes
        return this.isDragOperation || this.detectRapidEvents();
    }
    
    // Detect rapid event patterns that should be batched
    detectRapidEvents() {
        // Simple heuristic: if we have recent events in batch, consider it rapid
        return this.eventBatch.length > 0;
    }
    
    // Add event to batch
    addToBatch(serverEvent) {
        this.eventBatch.push(serverEvent);
        
        // Flush immediately if batch is full
        if (this.eventBatch.length >= this.MAX_BATCH_SIZE) {
            this.flushBatch();
            return;
        }
        
        // Start timeout only if not already running
        if (!this.batchTimeout) {
            this.batchTimeout = setTimeout(() => {
                this.flushBatch();
            }, this.BATCH_DELAY_MS);
        }
        
        console.log(`Bridge: Added event to batch (${this.eventBatch.length}/${this.MAX_BATCH_SIZE} events)`);
    }
    
    // Send all batched events
    flushBatch() {
        if (this.eventBatch.length === 0) {
            return;
        }
        
        console.log(`Bridge: Flushing batch with ${this.eventBatch.length} events`);
        
        if (this.isConnected) {
            this.webSocketClient.sendCanvasEvents(this.currentCanvasId, [...this.eventBatch]);
        }
        
        // Clear batch
        this.eventBatch = [];
        this.batchTimeout = null;
        this.isDragOperation = false;
    }
    
    // Clear event batch without sending (for error/disconnect scenarios)
    clearEventBatch() {
        console.log(`Bridge: Clearing event batch with ${this.eventBatch.length} events due to connection issue`);
        this.eventBatch = [];
        
        if (this.batchTimeout) {
            clearTimeout(this.batchTimeout);
            this.batchTimeout = null;
        }
        
        this.isDragOperation = false;
    }
    
    // Send queued events when connection is restored
    sendQueuedEvents() {
        if (this.eventQueueForServer.length === 0) return;
        
        console.log(`Bridge: Sending ${this.eventQueueForServer.length} queued events`);
        
        const eventsByCanvas = {};
        
        // Group events by canvas
        this.eventQueueForServer.forEach(({ canvasId, event }) => {
            if (!eventsByCanvas[canvasId]) {
                eventsByCanvas[canvasId] = [];
            }
            eventsByCanvas[canvasId].push(event);
        });
        
        // Send events for each canvas
        Object.entries(eventsByCanvas).forEach(([canvasId, events]) => {
            this.webSocketClient.sendCanvasEvents(canvasId, events);
        });
        
        // Clear queue
        this.eventQueueForServer = [];
    }
    
    // Convert EventBus event to server event format
    convertEventBusEventToServer(event) {
        console.log('BRIDGE→SERVER: Converting event:', event.type, event);
        
        switch (event.type) {
            case 'SHAPE_CREATED':
                const serverEvent = this.convertShapeCreatedToServer(event);
                console.log('BRIDGE→SERVER: SHAPE_CREATED converted to:', serverEvent);
                return serverEvent;
            
            case 'SHAPE_DELETED':
                return {
                    event: 'removeShape',
                    shapeId: String(event.shapeId)
                };
            
            case 'SHAPE_MODIFIED':
                // Convert German color names to hex for server
                let value = event.value;
                if (event.property === 'fillColor' || event.property === 'strokeColor') {
                    value = this.convertGermanToHex(event.value);
                }
                
                return {
                    event: 'modifyShape',
                    shapeId: String(event.shapeId),
                    property: event.property,
                    value: value
                };
            
            case 'SHAPE_SELECTED':
                return {
                    event: 'selectShape',
                    shapeId: String(event.shapeId),
                    clientId: event.clientId || 'unknown',
                    userColor: event.userColor || '#666666'
                };
            
            case 'SHAPE_UNSELECTED':
                return {
                    event: 'unselectShape',
                    shapeId: String(event.shapeId),
                    clientId: event.clientId || 'unknown'
                };
            
            default:
                console.warn('Bridge: Unknown event type for server conversion:', event.type);
                return null;
        }
    }
    
    // Convert SHAPE_CREATED event to server format
    convertShapeCreatedToServer(event) {
        if (!event.shapeId || !event.shapeType || !event.data) {
            console.warn('Bridge: Invalid SHAPE_CREATED event:', event);
            return null;
        }
        
        // Convert frontend shape type to server format (uppercase to lowercase)
        const shapeTypeMapping = {
            'Line': 'line',
            'Circle': 'circle',
            'Rectangle': 'rectangle',
            'Triangle': 'triangle'
        };
        
        const serverShapeType = shapeTypeMapping[event.shapeType] || event.shapeType.toLowerCase();
        
        // Convert frontend data to server format
        const serverData = {
            zOrder: event.data.zIndex || 1,
            z_order: event.data.zIndex || 1,
            bgColor: this.convertGermanToHex(event.data.fillColor),
            bg_color: this.convertGermanToHex(event.data.fillColor),
            fgColor: this.convertGermanToHex(event.data.strokeColor) || '#000000',
            fg_color: this.convertGermanToHex(event.data.strokeColor) || '#000000'
        };
        
        // Add shape-specific data
        if (event.data.from && event.data.to) {
            serverData.from = event.data.from;
            serverData.to = event.data.to;
        }
        
        if (event.data.center && event.data.radius) {
            serverData.center = event.data.center;
            serverData.radius = event.data.radius;
        }
        
        if (event.data.p1 && event.data.p2) {
            serverData.p1 = event.data.p1;
            serverData.p2 = event.data.p2;
        }
        
        if (event.data.p3) {
            serverData.p3 = event.data.p3;
        }
        
        console.log('Frontend->Server: Converting colors for shape', event.shapeId);
        console.log('Frontend->Server: Original fillColor:', event.data.fillColor);
        console.log('Frontend->Server: Original strokeColor:', event.data.strokeColor);
        console.log('Frontend->Server: Converted bgColor:', serverData.bgColor);
        console.log('Frontend->Server: Converted fgColor:', serverData.fgColor);
        
        return {
            event: 'addShape',
            shape: {
                type: serverShapeType,
                id: String(event.shapeId),
                data: serverData
            }
        };
    }
    
    // Check if event is a user-related event (userJoined/userLeft)
    isUserEvent(serverEvent) {
        return serverEvent.event === 'userJoined' || serverEvent.event === 'userLeft';
    }
    
    // Separate user events from canvas events
    separateUserAndCanvasEvents(events) {
        const userEvents = [];
        const canvasEvents = [];
        
        events.forEach(event => {
            if (this.isUserEvent(event)) {
                userEvents.push(event);
            } else {
                canvasEvents.push(event);
            }
        });
        
        return { userEvents, canvasEvents };
    }
    
    // Handle user events (join/leave)
    handleUserEvent(userEvent, canvasId) {
        console.log('Bridge: Processing user event:', userEvent);
        
        // Only process events for current canvas
        if (canvasId !== this.currentCanvasId) {
            console.log(`Bridge: Ignoring user event for canvas ${canvasId}, current is ${this.currentCanvasId}`);
            return;
        }
        
        // Multi-Tab Fix: Handle refresh signals for connection count updates
        if (userEvent.userId === 'USER_COUNT_REFRESH') {
            console.log('Bridge: Received connection count refresh signal');
            if (typeof window.refreshCanvasUsers === 'function') {
                window._loadingUsers = false;
                window.refreshCanvasUsers(true); // bypassThrottle = true for refresh events
                console.log('Bridge: User list refreshed due to connection count change');
            }
            return; // Don't show notification for refresh signals
        }
        
        // Refresh users display if function is available
        if (typeof window.refreshCanvasUsers === 'function') {
            console.log('Bridge: Refreshing canvas users display (bypassing throttle)');
            // Bypass throttling for WebSocket events to ensure immediate updates
            window._loadingUsers = false;
            window.refreshCanvasUsers(true); // bypassThrottle = true for WebSocket events
        } else {
            console.warn('Bridge: refreshCanvasUsers function not available');
        }
        
        // Show notification if available
        if (userEvent.event === 'userJoined') {
            this.showUserNotification(`${userEvent.displayName} ist beigetreten`, 'join');
        } else if (userEvent.event === 'userLeft') {
            this.showUserNotification(`${userEvent.displayName} hat verlassen`, 'leave');
        }
    }
    
    // Show user notification
    showUserNotification(message, type = 'info') {
        console.log('Bridge: User notification:', message);
        
        // Create notification element
        const notification = document.createElement('div');
        notification.className = `user-notification user-notification-${type}`;
        notification.textContent = message;
        notification.style.cssText = `
            position: fixed;
            top: 20px;
            right: 20px;
            background: ${type === 'join' ? '#d4edda' : '#f8d7da'};
            color: ${type === 'join' ? '#155724' : '#721c24'};
            border: 1px solid ${type === 'join' ? '#c3e6cb' : '#f1b0b7'};
            border-radius: 6px;
            padding: 8px 16px;
            font-size: 14px;
            font-weight: 500;
            z-index: 10000;
            box-shadow: 0 2px 8px rgba(0,0,0,0.1);
            transform: translateX(100%);
            transition: transform 0.3s ease;
        `;
        
        document.body.appendChild(notification);
        
        // Slide in
        requestAnimationFrame(() => {
            notification.style.transform = 'translateX(0)';
        });
        
        // Auto remove after 3 seconds using animation events
        const removeNotification = () => {
            notification.style.transform = 'translateX(100%)';
            const onTransitionEnd = () => {
                if (notification.parentNode) {
                    notification.parentNode.removeChild(notification);
                }
                notification.removeEventListener('transitionend', onTransitionEnd);
            };
            notification.addEventListener('transitionend', onTransitionEnd);
        };
        
        // Remove after 3 seconds
        setTimeout(removeNotification, 3000);
    }
    
    // Convert German color names to hex values for server
    convertGermanToHex(color) {
        if (!color) return null;
        if (color === 'transparent') return null;
        
        // German to hex mapping
        const germanToHex = {
            'rot': '#ff0000',
            'grün': '#00ff00', 
            'gelb': '#ffff00',
            'blau': '#0000ff',
            'schwarz': '#000000',
            'weiß': '#ffffff',
            'transparent': null
        };
        
        const normalizedColor = color.toLowerCase();
        
        // If it's already a hex color, return it
        if (normalizedColor.startsWith('#')) {
            return color;
        }
        
        // Convert German name to hex
        if (germanToHex.hasOwnProperty(normalizedColor)) {
            return germanToHex[normalizedColor];
        }
        
        // If it's an English color name, try to convert
        const englishToHex = {
            'red': '#ff0000',
            'green': '#00ff00',
            'yellow': '#ffff00', 
            'blue': '#0000ff',
            'black': '#000000',
            'white': '#ffffff'
        };
        
        if (englishToHex.hasOwnProperty(normalizedColor)) {
            return englishToHex[normalizedColor];
        }
        
        // Return original color if no conversion found
        return color;
    }
    
    // Get current status
    getStatus() {
        return {
            isInitialized: this.isInitialized,
            isConnected: this.isConnected,
            currentCanvasId: this.currentCanvasId,
            queuedEvents: this.eventQueueForServer.length
        };
    }
}

// Initialize bridge when script loads - FIXED: Ensure event handlers are set up
if (!window.canvasWebSocketBridge) {
    console.log('Creating Canvas WebSocket Bridge...');
    window.canvasWebSocketBridge = new CanvasWebSocketBridge();

    // Make registerForCanvas globally available for easier access
    window.registerForCanvas = (canvasId) => {
        if (window.canvasWebSocketBridge) {
            window.canvasWebSocketBridge.registerForCanvas(canvasId);
        } else {
            console.error('CanvasWebSocketBridge not available');
        }
    };

    window.unregisterFromCanvas = () => {
        if (window.canvasWebSocketBridge) {
            window.canvasWebSocketBridge.unregisterFromCanvas();
        } else {
            console.error('CanvasWebSocketBridge not available');
        }
    };

    // Dispatch ready event
    console.log('Canvas WebSocket Bridge ready, dispatching ready event');
    window.dispatchEvent(new Event('canvas-websocket-bridge-ready'));
} else {
    console.log('Canvas WebSocket Bridge exists, ensuring event handlers are set up...');
    // CRITICAL FIX: Ensure event handlers are set up for SPA navigation
    window.canvasWebSocketBridge.ensureEventHandlersSetup();
    console.log('Canvas WebSocket Bridge event handlers ensured');
}

} // End of duplicate check