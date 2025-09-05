import { generateEventId } from './events.js';
import { EventBus } from './event-bus.js';
// Event store for persisting and replaying events
export class EventStore {
    constructor() {
        this.events = [];
        this.eventBus = EventBus.getInstance();
        // Subscribe to all events to store them
        this.eventBus.subscribeToAll((event) => {
            this.persistEvent(event);
        });
    }
    // Singleton pattern
    static getInstance() {
        if (!EventStore.instance) {
            EventStore.instance = new EventStore();
        }
        return EventStore.instance;
    }
    // Add an event to the store
    persistEvent(event) {
        this.events.push(event);
        // Notify any UI components that are displaying the event log
        this.notifyEventLogUpdated();
    }
    // Get all events
    getEvents() {
        return [...this.events];
    }
    // Get events as formatted string for display
    getEventsAsString() {
        return JSON.stringify(this.events, null, 2);
    }
    // Load events from a string (for time travel functionality)
    loadEvents(eventsJson) {
        try {
            // Parse the events
            const parsedEvents = JSON.parse(eventsJson);
            // Validate that it's an array of events
            if (!Array.isArray(parsedEvents)) {
                throw new Error('Invalid event data: expected an array');
            }
            // Clear existing events
            this.clearEvents();
            // Load and replay each event
            for (const event of parsedEvents) {
                if (!event.type || typeof event.type !== 'string') {
                    console.warn('Skipping invalid event:', event);
                    continue;
                }
                // Ensure the event has an ID and timestamp
                if (!event.id) {
                    event.id = generateEventId();
                }
                if (!event.timestamp) {
                    event.timestamp = Date.now();
                }
                // Store the event without publishing to avoid loops
                this.events.push(event);
            }
            // Notify that the event log has been updated
            this.notifyEventLogUpdated();
            // Trigger a full replay of all events
            this.replay();
        }
        catch (error) {
            console.error('Error loading events:', error);
            throw new Error(`Failed to load events: ${error.message}`);
        }
    }
    // Clear all events
    clearEvents() {
        this.events = [];
        this.notifyEventLogUpdated();
    }
    // Replay all events in chronological order
    replay() {
        // Sort events by timestamp
        const sortedEvents = [...this.events].sort((a, b) => a.timestamp - b.timestamp);
        // Publish a special reset event to clear the application state
        this.eventBus.publish({
            type: 'RESET_STATE',
            timestamp: Date.now(),
            id: generateEventId()
        });
        // Replay each event in order
        for (const event of sortedEvents) {
            // Skip meta-events during replay
            if (event.type === 'RESET_STATE' || event.type === 'EVENT_LOG_UPDATED') {
                continue;
            }
            // Publish the event for handling
            this.eventBus.publish(Object.assign(Object.assign({}, event), { isReplay: true }));
        }
        // Publish a special event to signal that replay is complete
        this.eventBus.publish({
            type: 'REPLAY_COMPLETED',
            timestamp: Date.now(),
            id: generateEventId()
        });
    }
    // Notify listeners that the event log has been updated
    notifyEventLogUpdated() {
        this.eventBus.publish({
            type: 'EVENT_LOG_UPDATED',
            timestamp: Date.now(),
            id: generateEventId()
        });
    }
}
//# sourceMappingURL=event-store.js.map