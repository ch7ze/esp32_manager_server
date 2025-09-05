// Event handling functionality for client-side

// Check if already loaded to prevent duplicate declarations
if (typeof window.EventBus !== 'undefined') {
    console.log('EventBus already loaded, skipping redefinition');
} else {

// EventBus implementation for client-side events
class EventBus {
    constructor() {
        this.listeners = {};
        console.log('EventBus instance created');
    }
    
    // Subscribe to an event type
    subscribe(eventType, callback) {
        if (!this.listeners[eventType]) {
            this.listeners[eventType] = [];
        }
        this.listeners[eventType].push(callback);
        
        return () => this.unsubscribe(eventType, callback);
    }
    
    // Subscribe to all events
    subscribeToAll(callback) {
        return this.subscribe('*', callback);
    }
    
    // Unsubscribe from an event
    unsubscribe(eventType, callback) {
        if (!this.listeners[eventType]) return;
        
        this.listeners[eventType] = this.listeners[eventType].filter(
            listener => listener !== callback
        );
    }
    
    // Publish an event to subscribers
    publish(event) {
        // Add timestamp if not present
        if (!event.timestamp) {
            event.timestamp = Date.now();
        }
        
        // Add id if not present
        if (!event.id) {
            event.id = this.generateEventId();
        }
        
        // Call specific event type listeners
        const eventListeners = this.listeners[event.type] || [];
        eventListeners.forEach(listener => listener(event));
        
        // Call global listeners that listen to all events
        const globalListeners = this.listeners['*'] || [];
        globalListeners.forEach(listener => listener(event));
    }
    
    // Generate a unique event ID
    generateEventId() {
        return Date.now().toString(36) + Math.random().toString(36).substring(2, 5);
    }
}

// EventStore - No local persistence, events only exist in memory during session
class EventStore {
    constructor(eventBus) {
        this.eventBus = eventBus;
        console.log('EventStore instance created (server-only mode)');
    }
    
    // Server-based event replay - events come from WebSocket
    replayFromServer(serverEvents, canvasId = null, onComplete = null, isFullReplay = true) {
        // Get canvas ID from current state if not provided
        const targetCanvasId = canvasId || (window.drawerState && window.drawerState.currentCanvasId);
        
        console.log(`Starting replay from server events for canvas ${targetCanvasId}:`, serverEvents.length, `(fullReplay: ${isFullReplay})`);
        
        // Create canvas-specific replay flag to avoid event loops
        if (!window._canvasReplaying) {
            window._canvasReplaying = {};
        }
        window._canvasReplaying[targetCanvasId] = true;
        
        // Also set global flag for backward compatibility
        window._isReplaying = true;
        
        try {
            // Only publish RESET_STATE for full replays, not incremental updates
            if (isFullReplay) {
                // Publish a canvas-specific reset event to clear the application state
                this.eventBus.publish({
                    type: 'RESET_STATE',
                    timestamp: Date.now(),
                    id: this.eventBus.generateEventId(),
                    isFromServer: true,
                    canvasId: targetCanvasId
                });
                
                console.log(`Published RESET_STATE for canvas ${targetCanvasId}`);
            } else {
                console.log(`Skipping RESET_STATE for incremental update on canvas ${targetCanvasId}`);
            }
            
            // Wait briefly for RESET_STATE to be processed
            setTimeout(() => {
                try {
                    // Replay each server event in order
                    for (const serverEvent of serverEvents) {
                        // Skip meta-events during replay
                        if (serverEvent.event === 'RESET_STATE') {
                            continue;
                        }
                        
                        // Convert server event format to EventBus format and publish
                        const eventBusEvent = this.convertServerEventToEventBus(serverEvent);
                        if (eventBusEvent) {
                            console.log(`Replaying server event for canvas ${targetCanvasId}:`, eventBusEvent.type, eventBusEvent);
                            
                            // Validate event before replay to prevent corruption
                            if (this.validateEventForReplay(eventBusEvent)) {
                                this.eventBus.publish({
                                    ...eventBusEvent, 
                                    isReplay: true, 
                                    isFromServer: true, 
                                    canvasId: targetCanvasId
                                });
                            } else {
                                console.warn(`Skipping invalid event during replay:`, eventBusEvent);
                            }
                        }
                    }
                    
                    console.log(`Server replay complete for canvas ${targetCanvasId}`);
                    
                    // Call completion callback if provided
                    if (onComplete && typeof onComplete === 'function') {
                        try {
                            onComplete();
                        } catch (callbackError) {
                            console.error(`Error in replay completion callback:`, callbackError);
                        }
                    }
                } catch (error) {
                    console.error(`Error during server event replay for canvas ${targetCanvasId}:`, error);
                } finally {
                    // Remove canvas-specific replay marking
                    if (window._canvasReplaying) {
                        delete window._canvasReplaying[targetCanvasId];
                        
                        // Remove global flag if no canvas is replaying
                        if (Object.keys(window._canvasReplaying).length === 0) {
                            window._isReplaying = false;
                        }
                    }
                }
            }, 100);
        } catch (error) {
            console.error(`Error setting up server replay for canvas ${targetCanvasId}:`, error);
            if (window._canvasReplaying) {
                delete window._canvasReplaying[targetCanvasId];
            }
            window._isReplaying = false;
        }
    }
    
    // Convert server WebSocket event format to EventBus format
    convertServerEventToEventBus(serverEvent) {
        if (serverEvent.event === 'addShape' && serverEvent.shape) {
            // Convert shape type from server format (lowercase) to frontend format (uppercase)
            const shapeTypeMapping = {
                'line': 'Line',
                'circle': 'Circle', 
                'rectangle': 'Rectangle',
                'triangle': 'Triangle'
            };
            
            const frontendShapeType = shapeTypeMapping[serverEvent.shape.type] || serverEvent.shape.type;
            
            // Convert server data format to frontend data format
            const serverData = serverEvent.shape.data;
            console.log('DEBUG: Server data received:', serverData);
            
            console.log('Server->Frontend: Converting colors for shape', serverEvent.shape.id);
            console.log('Server->Frontend: Original bgColor:', serverData.bgColor);
            console.log('Server->Frontend: Original bg_color:', serverData.bg_color);
            console.log('Server->Frontend: Original fgColor:', serverData.fgColor);
            console.log('Server->Frontend: Original fg_color:', serverData.fg_color);
            
            // Helper function to convert hex colors back to German names if possible
            const convertHexToGermanName = (hexColor) => {
                if (!hexColor || typeof hexColor !== 'string') return hexColor;
                
                // Handle special cases first
                if (hexColor === 'transparent' || hexColor === null) {
                    return 'transparent';
                }
                
                // Normalize hex color (remove # if present, convert to lowercase)
                const normalizedHex = hexColor.replace('#', '').toLowerCase();
                
                // Extended map of hex values to German names (consistent with COLOR_MAP)
                const hexToGermanMap = {
                    'ff0000': 'rot',
                    'red': 'rot',
                    '00ff00': 'grün',
                    '008000': 'grün', // dark green
                    'green': 'grün', 
                    'ffff00': 'gelb',
                    'yellow': 'gelb',
                    '0000ff': 'blau',
                    'blue': 'blau',
                    '000000': 'schwarz',
                    'black': 'schwarz',
                    'ffffff': 'weiß',
                    'white': 'weiß',
                    'transparent': 'transparent'
                };
                
                // Check if we have a German equivalent
                if (hexToGermanMap[normalizedHex]) {
                    console.log(`Converting color ${hexColor} back to German name: ${hexToGermanMap[normalizedHex]}`);
                    return hexToGermanMap[normalizedHex];
                }
                
                // Check if it's already a German color name
                const germanColors = ['rot', 'grün', 'gelb', 'blau', 'schwarz', 'weiß', 'transparent'];
                if (germanColors.includes(hexColor.toLowerCase())) {
                    console.log(`Color ${hexColor} is already in German format`);
                    return hexColor.toLowerCase();
                }
                
                // If no German equivalent found, return the hex color with # prefix if needed
                const finalHex = hexColor.startsWith('#') ? hexColor : `#${hexColor}`;
                console.log(`No German equivalent for ${hexColor}, keeping as: ${finalHex}`);
                return finalHex;
            };
            
            // Fix color conversion: handle null values and convert hex back to German names
            let convertedFillColor = 'transparent';
            if (serverData.bgColor !== null && serverData.bgColor !== undefined) {
                convertedFillColor = convertHexToGermanName(serverData.bgColor);
            } else if (serverData.bg_color !== null && serverData.bg_color !== undefined) {
                convertedFillColor = convertHexToGermanName(serverData.bg_color);
            }
            
            let convertedStrokeColor = 'schwarz'; // Use German name as default
            if (serverData.fgColor !== null && serverData.fgColor !== undefined) {
                convertedStrokeColor = convertHexToGermanName(serverData.fgColor);
            } else if (serverData.fg_color !== null && serverData.fg_color !== undefined) {
                convertedStrokeColor = convertHexToGermanName(serverData.fg_color);
            }
            
            console.log('Server->Frontend: Converted fillColor:', convertedFillColor);
            console.log('Server->Frontend: Converted strokeColor:', convertedStrokeColor);
            
            const frontendData = {
                zIndex: serverData.zOrder || serverData.z_order || 1,
                fillColor: convertedFillColor,
                strokeColor: convertedStrokeColor,
                from: serverData.from,
                to: serverData.to,
                center: serverData.center,
                radius: serverData.radius,
                p1: serverData.p1,
                p2: serverData.p2,
                p3: serverData.p3
            };
            
            console.log('SERVER→CLIENT: Converted event data:', frontendData);
            
            return {
                type: 'SHAPE_CREATED',
                shapeId: serverEvent.shape.id,
                shapeType: frontendShapeType,
                data: frontendData,
                timestamp: Date.now(),
                id: this.eventBus.generateEventId()
            };
        } else if (serverEvent.event === 'removeShape' && serverEvent.shapeId) {
            return {
                type: 'SHAPE_DELETED',
                shapeId: serverEvent.shapeId,
                timestamp: Date.now(),
                id: this.eventBus.generateEventId()
            };
        } else if (serverEvent.event === 'modifyShape' && serverEvent.shapeId && serverEvent.property && serverEvent.value !== undefined) {
            return {
                type: 'SHAPE_MODIFIED',
                shapeId: serverEvent.shapeId,
                property: serverEvent.property,
                value: serverEvent.value,
                timestamp: Date.now(),
                id: this.eventBus.generateEventId()
            };
        } else if (serverEvent.event === 'selectShape' && serverEvent.shapeId && serverEvent.clientId) {
            return {
                type: 'SHAPE_SELECTED',
                shapeId: serverEvent.shapeId,
                clientId: serverEvent.clientId,
                userColor: serverEvent.userColor || '#666666',
                timestamp: Date.now(),
                id: this.eventBus.generateEventId()
            };
        } else if (serverEvent.event === 'unselectShape' && serverEvent.shapeId && serverEvent.clientId) {
            return {
                type: 'SHAPE_UNSELECTED',
                shapeId: serverEvent.shapeId,
                clientId: serverEvent.clientId,
                timestamp: Date.now(),
                id: this.eventBus.generateEventId()
            };
        }
        
        console.warn('Unknown server event type:', serverEvent);
        return null;
    }
    
    // Validate event before replay to prevent corruption
    validateEventForReplay(eventBusEvent) {
        if (!eventBusEvent || !eventBusEvent.type) {
            return false;
        }
        
        // Validate SHAPE_CREATED events
        if (eventBusEvent.type === 'SHAPE_CREATED') {
            if (!eventBusEvent.shapeId || !eventBusEvent.shapeType || !eventBusEvent.data) {
                console.warn('Invalid SHAPE_CREATED event - missing required fields');
                return false;
            }
            
            // Validate color values
            if (eventBusEvent.data.fillColor && !this.isValidColor(eventBusEvent.data.fillColor)) {
                console.warn('Invalid fillColor in SHAPE_CREATED:', eventBusEvent.data.fillColor);
                return false;
            }
            if (eventBusEvent.data.strokeColor && !this.isValidColor(eventBusEvent.data.strokeColor)) {
                console.warn('Invalid strokeColor in SHAPE_CREATED:', eventBusEvent.data.strokeColor);
                return false;
            }
        }
        
        // Validate SHAPE_MODIFIED events  
        if (eventBusEvent.type === 'SHAPE_MODIFIED') {
            if (!eventBusEvent.shapeId || !eventBusEvent.property) {
                console.warn('Invalid SHAPE_MODIFIED event - missing required fields');
                return false;
            }
            
            // Validate color modifications
            if ((eventBusEvent.property === 'fillColor' || eventBusEvent.property === 'strokeColor') && 
                !this.isValidColor(eventBusEvent.value)) {
                console.warn('Invalid color value in SHAPE_MODIFIED:', eventBusEvent.value);
                return false;
            }
        }
        
        return true;
    }
    
    // Check if a color value is valid
    isValidColor(color) {
        // Allow null (will be converted to transparent)
        if (color === null || color === undefined) return true;
        
        // Allow empty string (will be converted to transparent)
        if (!color) return false;
        
        // Allow transparent
        if (color === 'transparent') return true;
        
        // Allow hex colors
        if (color.match(/^#[0-9A-Fa-f]{6}$/)) return true;
        
        // Allow German color names
        const validGermanColors = ['rot', 'grün', 'gelb', 'blau', 'schwarz', 'weiß'];
        if (validGermanColors.includes(color.toLowerCase())) return true;
        
        // Allow English color names that might be in the system
        const validEnglishColors = ['red', 'green', 'yellow', 'blue', 'black', 'white'];
        if (validEnglishColors.includes(color.toLowerCase())) return true;
        
        console.log(`Color validation: "${color}" is considered invalid`);
        return false;
    }
    
    // Placeholder methods for compatibility (now handled by server)
    getEvents() { return []; }
    getEventsAsString() { return '[]'; }
    clearEvents() { console.log('Events cleared on server'); }
}

// Initialize event system immediately instead of waiting for DOMContentLoaded
// This ensures it works even when loaded dynamically after the page is already loaded
(function initEventSystem() {
    console.log('Initializing event system...');
    
    // Create event bus and store
    window.eventBus = new EventBus();
    window.eventStore = new EventStore(window.eventBus);
    
    console.log('EVENT-SYSTEM DEBUG: EventBus created:', !!window.eventBus);
    console.log('EVENT-SYSTEM DEBUG: EventStore created:', !!window.eventStore);
    console.log('EVENT-SYSTEM DEBUG: EventBus methods:', window.eventBus ? Object.keys(window.eventBus) : 'none');
    
    // Dispatch event to notify that the event system is ready
    console.log('Event system ready, dispatching ready event');
    window.dispatchEvent(new Event('drawer-event-system-ready'));
})();

} // End of EventBus already loaded check
