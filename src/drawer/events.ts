export interface DrawerEvent {
    type: string;
    timestamp: number;
    id: string;
    isReplay?: boolean;
}

// System Events
export interface ResetStateEvent extends DrawerEvent {
    type: 'RESET_STATE';
}

export interface EventLogUpdatedEvent extends DrawerEvent {
    type: 'EVENT_LOG_UPDATED';
}

export interface ReplayCompletedEvent extends DrawerEvent {
    type: 'REPLAY_COMPLETED';
}

// Shape Events
export interface ShapeCreatedEvent extends DrawerEvent {
    type: 'SHAPE_CREATED';
    shapeId: number;
    shapeType: string; // 'Line', 'Circle', 'Rectangle', 'Triangle'
    data: any; // Shape-specific properties
}

export interface ShapeDeletedEvent extends DrawerEvent {
    type: 'SHAPE_DELETED';
    shapeId: number;
}

export interface ShapeModifiedEvent extends DrawerEvent {
    type: 'SHAPE_MODIFIED';
    shapeId: number;
    property: string; // 'fillColor', 'strokeColor', 'zIndex'
    value: any;
}

// Tool Events
export interface ToolSelectedEvent extends DrawerEvent {
    type: 'TOOL_SELECTED';
    toolIndex: number;
}

// Mouse Events
export interface MouseEvent extends DrawerEvent {
    x: number;
    y: number;
}

export interface MouseDownEvent extends MouseEvent {
    type: 'MOUSE_DOWN';
}

export interface MouseMoveEvent extends MouseEvent {
    type: 'MOUSE_MOVE';
}

export interface MouseUpEvent extends MouseEvent {
    type: 'MOUSE_UP';
}

// Helper function for generating unique event IDs
export function generateEventId(): string {
    return Date.now().toString(36) + Math.random().toString(36).substring(2, 5);
}