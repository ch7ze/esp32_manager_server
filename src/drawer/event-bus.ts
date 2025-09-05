import { DrawerEvent } from './events.js';

// Event bus to facilitate communication between components
export class EventBus {
  private static instance: EventBus;
  private listeners: { [eventType: string]: Array<(event: DrawerEvent) => void> } = {};

  private constructor() {}

  // Singleton pattern
  public static getInstance(): EventBus {
    if (!EventBus.instance) {
      EventBus.instance = new EventBus();
    }
    return EventBus.instance;
  }

  // Register a listener for a specific event type
  public subscribe(eventType: string, callback: (event: DrawerEvent) => void): void {
    if (!this.listeners[eventType]) {
      this.listeners[eventType] = [];
    }
    this.listeners[eventType].push(callback);
  }

  // Register a listener for all event types
  public subscribeToAll(callback: (event: DrawerEvent) => void): void {
    this.subscribe('*', callback);
  }

  // Remove a listener
  public unsubscribe(eventType: string, callback: (event: DrawerEvent) => void): void {
    if (!this.listeners[eventType]) return;
    
    this.listeners[eventType] = this.listeners[eventType].filter(
      listener => listener !== callback
    );
  }

  // Publish an event to all registered listeners
  public publish(event: DrawerEvent): void {
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
