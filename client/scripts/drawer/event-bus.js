// Event bus to facilitate communication between components
export class EventBus {
    constructor() {
        this.listeners = {};
    }
    // Singleton pattern
    static getInstance() {
        if (!EventBus.instance) {
            EventBus.instance = new EventBus();
        }
        return EventBus.instance;
    }
    // Register a listener for a specific event type
    subscribe(eventType, callback) {
        if (!this.listeners[eventType]) {
            this.listeners[eventType] = [];
        }
        this.listeners[eventType].push(callback);
    }
    // Register a listener for all event types
    subscribeToAll(callback) {
        this.subscribe('*', callback);
    }
    // Remove a listener
    unsubscribe(eventType, callback) {
        if (!this.listeners[eventType])
            return;
        this.listeners[eventType] = this.listeners[eventType].filter(listener => listener !== callback);
    }
    // Publish an event to all registered listeners
    publish(event) {
        // Make sure the event has a timestamp if not already set
        if (!event.timestamp) {
            event.timestamp = Date.now();
        }
        // Call specific event type listeners
        const eventListeners = this.listeners[event.type] || [];
        eventListeners.forEach(listener => listener(event));
        // Call global listeners that listen to all events
        const globalListeners = this.listeners['*'] || [];
        globalListeners.forEach(listener => listener(event));
    }
}
//# sourceMappingURL=event-bus.js.map