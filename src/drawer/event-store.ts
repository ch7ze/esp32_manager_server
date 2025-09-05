import { DrawerEvent, generateEventId } from './events.js';
import { EventBus } from './event-bus.js';

// Event store for persisting and replaying events
export class EventStore {
  private static instance: EventStore;
  private events: DrawerEvent[] = [];
  private eventBus: EventBus;
  
  private constructor() {
    this.eventBus = EventBus.getInstance();
    
    // Subscribe to all events to store them
    this.eventBus.subscribeToAll((event: DrawerEvent) => {
      this.persistEvent(event);
    });
  }
  
  // Singleton pattern
  public static getInstance(): EventStore {
    if (!EventStore.instance) {
      EventStore.instance = new EventStore();
    }
    return EventStore.instance;
  }
  
  // Add an event to the store
  private persistEvent(event: DrawerEvent): void {
    this.events.push(event);
    
    // Notify any UI components that are displaying the event log
    this.notifyEventLogUpdated();
  }
  
  // Get all events
  public getEvents(): DrawerEvent[] {
    return [...this.events];
  }
  
  // Get events as formatted string for display
  public getEventsAsString(): string {
    return JSON.stringify(this.events, null, 2);
  }
  
  // Load events from a string (for time travel functionality)
  public loadEvents(eventsJson: string): void {
    try {
      // Parse the events
      const parsedEvents = JSON.parse(eventsJson) as DrawerEvent[];
      
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
    } catch (error) {
      console.error('Error loading events:', error);
      throw new Error(`Failed to load events: ${error.message}`);
    }
  }
  
  // Clear all events
  public clearEvents(): void {
    this.events = [];
    this.notifyEventLogUpdated();
  }
    // Replay all events in chronological order
  public replay(): void {
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
      this.eventBus.publish({...event, isReplay: true});
    }
    
    // Publish a special event to signal that replay is complete
    this.eventBus.publish({
      type: 'REPLAY_COMPLETED',
      timestamp: Date.now(),
      id: generateEventId()
    });
  }
  
  // Notify listeners that the event log has been updated
  private notifyEventLogUpdated(): void {
    this.eventBus.publish({
      type: 'EVENT_LOG_UPDATED',
      timestamp: Date.now(),
      id: generateEventId()
    });
  }
}
