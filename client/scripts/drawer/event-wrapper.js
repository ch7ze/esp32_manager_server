// Event-based wrapper for the Canvas
// This file creates the bridge between the event-based architecture and the Canvas implementation
// Initialize immediately rather than waiting for DOMContentLoaded
// This ensures it works when loaded dynamically
console.log('Event wrapper initializing...');
// Wait for drawer system to be fully initialized instead of fixed delay
// Listen for canvas creation rather than guessing timing
if (window.canvas && window.eventBus) {
    console.log('EVENT-WRAPPER: Canvas and EventBus already available, setting up immediately');
    setupEventHandlers();
}
else {
    console.log('EVENT-WRAPPER: Waiting for canvas initialization...');
    setTimeout(setupEventHandlers, 200); // Increased delay to allow canvas init
}
// Additionally listen for canvas-ready event to handle canvas recreation during SPA navigation
window.addEventListener('canvas-ready', function () {
    console.log('EVENT-WRAPPER: Received canvas-ready event, re-patching canvas methods...');
    if (window.canvas && window.eventBus) {
        // Re-patch canvas methods after canvas recreation
        patchCanvasMethods();
    }
    else {
        console.warn('EVENT-WRAPPER: canvas-ready event received but canvas or eventBus not available');
    }
});
function setupEventHandlers() {
    var _a, _b;
    // Ensure both the canvas and event system are initialized
    console.log('EVENT-WRAPPER DEBUG: Checking dependencies...');
    console.log('EVENT-WRAPPER DEBUG: window.canvas =', !!window.canvas);
    console.log('EVENT-WRAPPER DEBUG: window.eventBus =', !!window.eventBus);
    console.log('EVENT-WRAPPER DEBUG: window.eventStore =', !!window.eventStore);
    console.log('EVENT-WRAPPER DEBUG: setupEventHandlers called from:', (_b = (_a = new Error().stack) === null || _a === void 0 ? void 0 : _a.split('\n')[1]) === null || _b === void 0 ? void 0 : _b.trim());
    if (!window.canvas || !window.eventBus) {
        console.warn('Canvas or event system not ready yet, retrying in 500ms...');
        console.warn('EVENT-WRAPPER DEBUG: Missing:', !window.canvas ? 'canvas' : '', !window.eventBus ? 'eventBus' : '');
        setTimeout(setupEventHandlers, 500);
        return;
    }
    console.log('Setting up event handlers for Canvas');
    // Subscribe to events
    window.eventBus.subscribe('SHAPE_CREATED', handleShapeCreated);
    window.eventBus.subscribe('SHAPE_DELETED', handleShapeDeleted);
    window.eventBus.subscribe('SHAPE_MODIFIED', handleShapeModified);
    window.eventBus.subscribe('SHAPE_SELECTED', handleShapeSelected);
    window.eventBus.subscribe('SHAPE_UNSELECTED', handleShapeUnselected);
    window.eventBus.subscribe('RESET_STATE', handleResetState);
    window.eventBus.subscribe('REPLAY_COMPLETED', handleReplayCompleted);
    // Override Canvas methods to emit events
    patchCanvasMethods();
}
// Handle shape creation events
function handleShapeCreated(event) {
    if (event.isReplay) {
        console.log('Replaying shape creation:', event.shapeId, event.shapeType, event.data);
        // Set persistent replay flag for this shape to prevent event emission
        if (!window._replayingShapes) {
            window._replayingShapes = new Set();
        }
        window._replayingShapes.add(event.shapeId);
        // Set global flag to prevent emitting further events during replay
        window._isReplaying = true;
        // Create shape from the event data
        try {
            const shape = createShapeFromEventData(event);
            if (shape) {
                console.log(`Successfully created shape with ID ${shape.id}, type ${event.shapeType}`);
                console.log(`Shape properties: fillColor=${shape.getFillColor()}, strokeColor=${shape.getStrokeColor()}`);
                // Bypass the patched method to avoid circular event emission
                if (window.canvas._originalAddShape) {
                    window.canvas._originalAddShape(shape, true, false);
                    console.log(`Added shape with ID ${shape.id} to canvas`);
                }
                else if (window.canvas.addShape) {
                    // Store the original method, if not already patched
                    window.canvas._originalAddShape = window.canvas.addShape;
                    window.canvas._originalAddShape(shape, true, false);
                    console.log(`Added shape with ID ${shape.id} to canvas (using unpatched method)`);
                }
                else {
                    console.error('No addShape method available on canvas');
                }
            }
            else {
                console.error('Failed to create shape from event data:', event);
                // Display available shape classes for debugging
                console.log('Available global shape classes:', 'Point2D:', !!window.Point2D, 'Line:', !!window.Line, 'Circle:', !!window.Circle, 'Rectangle:', !!window.Rectangle, 'Triangle:', !!window.Triangle);
                console.log('Available drawer namespace:', 'Drawer:', !!window.Drawer, 'drawer:', !!window.drawer);
                if (window.Drawer) {
                    console.log('Drawer namespace contents:', Object.keys(window.Drawer));
                }
            }
        }
        catch (error) {
            console.error('Error recreating shape during replay:', error);
        }
        finally {
            // Reset global flag and clean up shape-specific flag
            window._isReplaying = false;
            if (window._replayingShapes && event.shapeId) {
                window._replayingShapes.delete(event.shapeId);
            }
        }
    }
}
// Handle shape deletion events
function handleShapeDeleted(event) {
    var _a, _b;
    if (event.isReplay) {
        console.log('Replaying shape deletion:', event.shapeId);
        // Debug: Check what methods are available on window.canvas
        console.log('DEBUG handleShapeDeleted: window.canvas exists:', !!window.canvas);
        console.log('DEBUG handleShapeDeleted: window.canvas._originalRemoveShapeWithId exists:', !!((_a = window.canvas) === null || _a === void 0 ? void 0 : _a._originalRemoveShapeWithId));
        console.log('DEBUG handleShapeDeleted: window.canvas.removeShapeWithId exists:', !!((_b = window.canvas) === null || _b === void 0 ? void 0 : _b.removeShapeWithId));
        if (window.canvas) {
            console.log('DEBUG handleShapeDeleted: canvas properties:', Object.keys(window.canvas).filter(k => k.includes('remove') || k.includes('original')));
        }
        // Remove the shape by ID
        try {
            // Bypass the patched method to avoid circular event emission
            if (!window.canvas._originalRemoveShapeWithId) {
                console.log('Missing _originalRemoveShapeWithId, storing it now');
                window.canvas._originalRemoveShapeWithId = window.canvas.removeShapeWithId;
            }
            window.canvas._originalRemoveShapeWithId(event.shapeId, true);
        }
        catch (error) {
            console.error('Error removing shape during replay:', error);
        }
    }
}
// Handle shape modification events
function handleShapeModified(event) {
    if (event.isReplay) {
        console.log('Replaying shape modification:', event.shapeId, event.property, event.value);
        // Set replay flag to prevent event loops
        if (!window._replayingShapes) {
            window._replayingShapes = new Set();
        }
        window._replayingShapes.add(event.shapeId);
        window._isReplaying = true;
        try {
            const shape = window.canvas.shapes[event.shapeId];
            if (shape) {
                // Apply the modification based on property type
                switch (event.property) {
                    case 'fillColor':
                        if (shape.setFillColor) {
                            const fillColor = event.value === null ? 'transparent' : event.value;
                            shape.setFillColor(fillColor);
                        }
                        break;
                    case 'strokeColor':
                        if (shape.setStrokeColor) {
                            const strokeColor = event.value === null ? '#000000' : event.value;
                            shape.setStrokeColor(strokeColor);
                        }
                        break;
                    case 'zIndex':
                        if (shape.setZIndex) {
                            shape.setZIndex(event.value);
                        }
                        break;
                    // Add other property handlers as needed
                }
                // Redraw canvas
                window.canvas.draw();
            }
        }
        catch (error) {
            console.error('Error modifying shape during replay:', error);
        }
        finally {
            // Reset global flag and clean up shape-specific flag
            window._isReplaying = false;
            if (window._replayingShapes && event.shapeId) {
                window._replayingShapes.delete(event.shapeId);
            }
        }
    }
}
// Handle reset state events
function handleResetState() {
    // Clear all shapes from the canvas
    try {
        console.log('Handling RESET_STATE event');
        if (window.canvas && window.canvas.shapes) {
            console.log(`Clearing ${Object.keys(window.canvas.shapes).length} shapes from canvas`);
            // Get a copy of shape IDs to avoid modification during iteration
            const shapeIds = Object.keys(window.canvas.shapes).map(id => parseInt(id));
            // Remove each shape without redrawing until the end
            shapeIds.forEach(id => {
                console.log(`Removing shape with ID ${id}`);
                // Bypass the patched method to avoid circular event emission
                window.canvas._originalRemoveShapeWithId(id, false);
            });
            // Redraw the now-empty canvas
            window.canvas.draw();
            console.log('Canvas reset complete');
        }
        else {
            console.warn('Canvas or shapes collection not available for reset');
        }
    }
    catch (error) {
        console.error('Error resetting canvas state:', error);
    }
}
// Handle the completion of event replay
function handleReplayCompleted() {
    console.log('Event replay completed - ensuring shapes are properly initialized');
    // Clear replay flags to allow normal event processing
    if (window._replayingShapes) {
        window._replayingShapes.clear();
        console.log('Cleared all shape replay flags');
    }
    window._isReplaying = false;
    try {
        if (!window.canvas || !window.canvas.shapes) {
            console.error('Canvas or shapes collection not available');
            return;
        }
        // Get all shape IDs
        const shapeIds = Object.keys(window.canvas.shapes);
        console.log(`Checking ${shapeIds.length} shapes after replay`);
        if (shapeIds.length === 0) {
            console.warn('No shapes found after replay. This might indicate an issue with shape creation during replay.');
        }
        // Loop through all shapes to ensure they have proper properties and methods
        for (const id of shapeIds) {
            const shape = window.canvas.shapes[id];
            if (!shape)
                continue;
            console.log(`Checking shape ${id} of type ${shape.constructor ? shape.constructor.name : 'unknown'}`);
            // Log final properties to verify they were set correctly
            console.log(`Shape ${id} final properties:`, {
                type: shape.constructor ? shape.constructor.name : 'unknown',
                fillColor: shape.getFillColor ? shape.getFillColor() : shape.fillColor,
                strokeColor: shape.getStrokeColor ? shape.getStrokeColor() : shape.strokeColor,
                zIndex: shape.getZIndex ? shape.getZIndex() : shape.zIndex
            });
            // Verify the shape has all required methods
            if (!shape.draw) {
                console.warn(`Shape ${id} missing draw method`);
            }
            // Verify selection methods exist
            if (!shape.isPointInside && !shape.isPointNear) {
                console.warn(`Shape ${id} missing selection methods (isPointInside/isPointNear)`);
            }
        }
        // Update canvas ordering if shapes have z-indices
        if (window.canvas.updateOrderedShapeIds) {
            console.log('Updating shape order after replay');
            window.canvas.updateOrderedShapeIds();
        }
        // Redraw canvas to show updated shapes
        window.canvas.draw();
        console.log('All shapes verified and canvas redrawn after replay');
    }
    catch (error) {
        console.error('Error handling replay completion:', error);
    }
}
// Create a shape from event data
function createShapeFromEventData(event) {
    const { shapeType, data } = event;
    try {
        console.log('Creating shape from event data:', shapeType, data);
        // Direct approach using globally exposed classes
        const shapeData = Object.assign({ type: shapeType, id: event.shapeId }, data);
        // First try direct approach using globally exposed createShapeFromData
        if (typeof window.createShapeFromData === 'function') {
            console.log('Using global createShapeFromData utility');
            return window.createShapeFromData(shapeData);
        }
        // Next try with the drawer namespace
        if (window.Drawer && typeof window.Drawer.createShapeFromData === 'function') {
            console.log('Using Drawer.createShapeFromData utility');
            return window.Drawer.createShapeFromData(shapeData);
        }
        // Fall back to direct class instantiation
        console.log('Using direct class instantiation for shape creation');
        // Get the appropriate constructor and Point2D class
        const Point2D = window.Point2D || (window.Drawer && window.Drawer.Point2D);
        if (!Point2D) {
            console.error('Point2D class not found');
            return null;
        }
        let shape = null;
        switch (shapeType) {
            case 'Line':
                const Line = window.Line || (window.Drawer && window.Drawer.Line);
                if (Line && data.from && data.to) {
                    shape = new Line(new Point2D(data.from.x, data.from.y), new Point2D(data.to.x, data.to.y));
                }
                break;
            case 'Rectangle':
                const Rectangle = window.Rectangle || (window.Drawer && window.Drawer.Rectangle);
                if (Rectangle && data.from && data.to) {
                    shape = new Rectangle(new Point2D(data.from.x, data.from.y), new Point2D(data.to.x, data.to.y));
                }
                break;
            case 'Circle':
                const Circle = window.Circle || (window.Drawer && window.Drawer.Circle);
                if (Circle && data.center && data.radius) {
                    shape = new Circle(new Point2D(data.center.x, data.center.y), data.radius);
                }
                break;
            case 'Triangle':
                const Triangle = window.Triangle || (window.Drawer && window.Drawer.Triangle);
                if (Triangle && data.p1 && data.p2 && data.p3) {
                    shape = new Triangle(new Point2D(data.p1.x, data.p1.y), new Point2D(data.p2.x, data.p2.y), new Point2D(data.p3.x, data.p3.y));
                }
                break;
            default:
                console.error('Unknown shape type:', shapeType);
                return null;
        }
        if (shape) {
            // Set ID
            if (event.shapeId !== undefined) {
                shape.id = event.shapeId;
            }
            // Set properties - ensure they are applied correctly
            if (data.fillColor !== undefined && typeof shape.setFillColor === 'function') {
                console.log(`Setting fillColor to ${data.fillColor} for shape ${shape.id}`);
                shape.setFillColor(data.fillColor);
            }
            if (data.strokeColor !== undefined && typeof shape.setStrokeColor === 'function') {
                console.log(`Setting strokeColor to ${data.strokeColor} for shape ${shape.id}`);
                shape.setStrokeColor(data.strokeColor);
            }
            if (data.zIndex !== undefined && typeof shape.setZIndex === 'function') {
                console.log(`Setting zIndex to ${data.zIndex} for shape ${shape.id}`);
                shape.setZIndex(data.zIndex);
            }
            // Verify properties were set correctly
            console.log(`Shape ${shape.id} properties after creation:`, {
                fillColor: shape.getFillColor ? shape.getFillColor() : 'undefined',
                strokeColor: shape.getStrokeColor ? shape.getStrokeColor() : 'undefined',
                zIndex: shape.getZIndex ? shape.getZIndex() : 'undefined'
            });
            console.log(`Created shape with ID ${shape.id} using direct instantiation`);
            return shape;
        }
        console.error('Failed to create shape - required classes not found');
        return null;
    }
    catch (error) {
        console.error(`Error creating ${shapeType}:`, error);
        return null;
    }
}
// Override Canvas methods to emit events
function patchCanvasMethods() {
    if (!window.canvas) {
        console.error('Canvas not available for patching methods');
        return;
    }
    // Check if already patched to prevent multiple patching
    console.log('DEBUG patchCanvasMethods: Checking if already patched...');
    console.log('DEBUG patchCanvasMethods: window.canvas._originalRemoveShapeWithId exists:', !!window.canvas._originalRemoveShapeWithId);
    if (window.canvas._originalRemoveShapeWithId && typeof window.canvas._originalRemoveShapeWithId === 'function') {
        console.log('Canvas methods already patched, skipping');
        console.log('DEBUG patchCanvasMethods: Current canvas instance ID:', window.canvas._instanceId || 'no ID');
        return;
    }
    // Store original methods
    console.log('DEBUG patchCanvasMethods: Storing original methods...');
    console.log('DEBUG patchCanvasMethods: Available canvas methods:', Object.getOwnPropertyNames(window.canvas).filter(name => typeof window.canvas[name] === 'function'));
    console.log('DEBUG patchCanvasMethods: removeShapeWithId exists:', typeof window.canvas.removeShapeWithId);
    window.canvas._originalAddShape = window.canvas.addShape;
    window.canvas._originalRemoveShape = window.canvas.removeShape;
    window.canvas._originalRemoveShapeWithId = window.canvas.removeShapeWithId;
    console.log('DEBUG patchCanvasMethods: Stored _originalRemoveShapeWithId:', typeof window.canvas._originalRemoveShapeWithId);
    // Add a unique identifier to track canvas instance
    window.canvas._instanceId = Date.now().toString(36) + Math.random().toString(36).substring(2, 5);
    console.log('DEBUG patchCanvasMethods: Canvas patched with instance ID:', window.canvas._instanceId);
    // Global set to track temporary shapes
    if (!window._temporaryShapes) {
        window._temporaryShapes = new Set();
    }
    // Override addShape to emit events
    window.canvas.addShape = function (shape, redraw = true, isTemp = false) {
        // Track temporary shapes
        if (isTemp) {
            window._temporaryShapes.add(shape.id);
        }
        else {
            window._temporaryShapes.delete(shape.id);
        }
        // Only emit events for permanent shapes
        if (!isTemp) {
            // More robust method for determining shape type
            let shapeType = 'Unknown';
            // First method: use constructor name if available
            if (shape && shape.constructor && typeof shape.constructor.name === 'string') {
                shapeType = shape.constructor.name;
            }
            // Second method: check properties for more robust type detection
            else if (shape.center && typeof shape.radius === 'number') {
                shapeType = 'Circle';
            }
            else if (shape.from && shape.to && !shape.p1) {
                if (typeof shape.isPointInside === 'function') {
                    shapeType = 'Rectangle';
                }
                else {
                    shapeType = 'Line';
                }
            }
            else if (shape.p1 && shape.p2 && shape.p3) {
                shapeType = 'Triangle';
            }
            console.log(`EVENT: SHAPE_CREATED (${shapeType} ${shape.id})`);
            // Create event data - temporary inline implementation until fully migrated
            const data = {
                fillColor: shape.getFillColor(),
                strokeColor: shape.getStrokeColor(),
                zIndex: shape.getZIndex()
            };
            // Shape-specific properties
            if (shape.from && shape.to && !shape.p1) {
                if (typeof shape.radius === 'undefined') {
                    data.from = { x: shape.from.x, y: shape.from.y };
                    data.to = { x: shape.to.x, y: shape.to.y };
                }
            }
            else if (shape.center && typeof shape.radius === 'number') {
                data.center = { x: shape.center.x, y: shape.center.y };
                data.radius = shape.radius;
            }
            else if (shape.p1 && shape.p2 && shape.p3) {
                data.p1 = { x: shape.p1.x, y: shape.p1.y };
                data.p2 = { x: shape.p2.x, y: shape.p2.y };
                data.p3 = { x: shape.p3.x, y: shape.p3.y };
            }
            const eventData = {
                type: 'SHAPE_CREATED',
                timestamp: Date.now(),
                id: Date.now().toString(36) + Math.random().toString(36).substring(2, 5),
                shapeId: shape.id,
                shapeType: shapeType,
                data: data
            };
            // Emit the event only if not replaying and not dragging
            if (window.eventBus && !window._isReplaying && !window._isDragging) {
                window.eventBus.publish(eventData);
            }
        }
        // Call the original method
        return this._originalAddShape(shape, redraw, isTemp);
    };
    // Override removeShape to emit events
    window.canvas.removeShape = function (shape, redraw = true) {
        // Only emit events for permanent shapes
        if (shape && shape.id && !this.tempShapes[shape.id]) {
            const eventData = {
                type: 'SHAPE_DELETED',
                timestamp: Date.now(),
                id: Date.now().toString(36) + Math.random().toString(36).substring(2, 5),
                shapeId: shape.id
            };
            // Emit the event only if not dragging (except for explicit DELETE events during drag completion)
            const isDraggingButAllowDelete = window._isDragging && eventData.type === 'SHAPE_DELETED';
            if (window.eventBus && (!window._isDragging || isDraggingButAllowDelete)) {
                window.eventBus.publish(eventData);
            }
        }
        // Call the original method
        return this._originalRemoveShape(shape, redraw);
    };
    // Override removeShapeWithId to emit events
    window.canvas.removeShapeWithId = function (id, redraw = true) {
        // Clean up temporary shapes tracking
        if (window._temporaryShapes) {
            window._temporaryShapes.delete(id);
        }
        // Only emit events for permanent shapes
        if (id && this.shapes[id] && !this.tempShapes[id]) {
            const eventData = {
                type: 'SHAPE_DELETED',
                timestamp: Date.now(),
                id: Date.now().toString(36) + Math.random().toString(36).substring(2, 5),
                shapeId: id
            };
            // Emit the event only if not dragging (except for explicit DELETE events during drag completion)
            const isDraggingButAllowDelete = window._isDragging && eventData.type === 'SHAPE_DELETED';
            if (window.eventBus && (!window._isDragging || isDraggingButAllowDelete)) {
                window.eventBus.publish(eventData);
            }
        }
        // Call the original method
        return this._originalRemoveShapeWithId(id, redraw);
    };
    // Also patch methods that modify shapes
    patchShapeModificationMethods();
}
// MIGRATED TO: Canvas.extractShapeData()
// Helper for extracting shape data
/*
function extractShapeData(shape) {
    // Common properties - ensure we get current values
    const data = {
        fillColor: shape.getFillColor ? shape.getFillColor() : (shape.fillColor || 'transparent'),
        strokeColor: shape.getStrokeColor ? shape.getStrokeColor() : (shape.strokeColor || '#000000'),
        zIndex: shape.getZIndex ? shape.getZIndex() : (shape.zIndex || 0)
    };
    
    console.log(`Extracting data for shape ${shape.id}:`, data);
    
    // More robust checking of shape properties
    if (shape.from && shape.to && !shape.p1) {
        // Could be Line or Rectangle
        if (typeof shape.radius === 'undefined') {
            // It's a Line or Rectangle
            return {
                ...data,
                from: { x: shape.from.x, y: shape.from.y },
                to: { x: shape.to.x, y: shape.to.y }
            };
        }
    }
    else if (shape.center && typeof shape.radius === 'number') {
        // It's a Circle
        return {
            ...data,
            center: { x: shape.center.x, y: shape.center.y },
            radius: shape.radius
        };
    }
    else if (shape.p1 && shape.p2 && shape.p3) {
        // It's a Triangle
        return {
            ...data,
            p1: { x: shape.p1.x, y: shape.p1.y },
            p2: { x: shape.p2.x, y: shape.p2.y },
            p3: { x: shape.p3.x, y: shape.p3.y }
        };
    }
    
    return data;
}
*/
// Patch methods that modify shapes
function patchShapeModificationMethods() {
    // Patch the shape classes to emit events when properties are changed
    if (window.AbstractShape) {
        // Check if already patched to prevent multiple patching
        if (window.AbstractShape.prototype._originalSetFillColor) {
            console.log('Shape modification methods already patched, skipping');
            return;
        }
        // Store original methods
        window.AbstractShape.prototype._originalSetFillColor = window.AbstractShape.prototype.setFillColor;
        window.AbstractShape.prototype._originalSetStrokeColor = window.AbstractShape.prototype.setStrokeColor;
        window.AbstractShape.prototype._originalSetZIndex = window.AbstractShape.prototype.setZIndex;
        // Override setFillColor
        window.AbstractShape.prototype.setFillColor = function (color) {
            // Log old color
            const oldColor = this.getFillColor ? this.getFillColor() : this.fillColor;
            // Only log for non-temporary shapes to reduce console spam
            if (this.id < 10000) {
                console.log(`Changing fillColor from ${oldColor} to ${color} for shape ${this.id}`);
            }
            // Call original method
            window.AbstractShape.prototype._originalSetFillColor.call(this, color);
            // Check if we should emit events (not during replay, not for replaying shapes, not for temporary shapes)
            const isGlobalReplaying = !!window._isReplaying;
            const isShapeReplaying = window._replayingShapes && window._replayingShapes.has(this.id);
            const isTemporaryShape = this.id >= 10000 || (window._temporaryShapes && window._temporaryShapes.has(this.id));
            const shouldEmitEvent = window.eventBus &&
                !isGlobalReplaying &&
                !isShapeReplaying &&
                !isTemporaryShape;
            if (shouldEmitEvent) {
                // Get current shape status - use 'this' if shape is not in canvas
                const shape = window.canvas.shapes[this.id] || this;
                const fillColor = shape.getFillColor ? shape.getFillColor() : shape.fillColor;
                const strokeColor = shape.getStrokeColor ? shape.getStrokeColor() : shape.strokeColor;
                const zIndex = shape.getZIndex ? shape.getZIndex() : shape.zIndex;
                console.log(`EVENT: SHAPE_MODIFIED (shape ${this.id}) fillColor: ${color}`);
                window.eventBus.publish({
                    type: 'SHAPE_MODIFIED',
                    timestamp: Date.now(),
                    id: Date.now().toString(36) + Math.random().toString(36).substring(2, 5),
                    shapeId: this.id,
                    property: 'fillColor',
                    value: color,
                    state: {
                        fillColor: fillColor,
                        strokeColor: strokeColor,
                        zIndex: zIndex
                    }
                });
            }
            else {
                // Only log suppression for non-temporary shapes to reduce console spam
                if (!isTemporaryShape) {
                    console.log(`Suppressing fillColor event for shape ${this.id} (globalReplay: ${isGlobalReplaying}, shapeReplay: ${isShapeReplaying})`);
                }
            }
        };
        // Override setStrokeColor
        window.AbstractShape.prototype.setStrokeColor = function (color) {
            const oldColor = this.getStrokeColor ? this.getStrokeColor() : this.strokeColor;
            // Call original method
            window.AbstractShape.prototype._originalSetStrokeColor.call(this, color);
            // Check if we should emit events (not during replay, not for replaying shapes, not for temporary shapes)
            const isGlobalReplaying = !!window._isReplaying;
            const isShapeReplaying = window._replayingShapes && window._replayingShapes.has(this.id);
            const isTemporaryShape = this.id >= 10000 || (window._temporaryShapes && window._temporaryShapes.has(this.id));
            const shouldEmitEvent = window.eventBus &&
                !isGlobalReplaying &&
                !isShapeReplaying &&
                !isTemporaryShape;
            if (shouldEmitEvent) {
                // Get current shape status - use 'this' if shape is not in canvas
                const shape = window.canvas.shapes[this.id] || this;
                const fillColor = shape.getFillColor ? shape.getFillColor() : shape.fillColor;
                const strokeColor = shape.getStrokeColor ? shape.getStrokeColor() : shape.strokeColor;
                const zIndex = shape.getZIndex ? shape.getZIndex() : shape.zIndex;
                console.log(`EVENT: SHAPE_MODIFIED (shape ${this.id}) strokeColor: ${color}`);
                window.eventBus.publish({
                    type: 'SHAPE_MODIFIED',
                    timestamp: Date.now(),
                    id: Date.now().toString(36) + Math.random().toString(36).substring(2, 5),
                    shapeId: this.id,
                    property: 'strokeColor',
                    value: color,
                    state: {
                        fillColor: fillColor,
                        strokeColor: strokeColor,
                        zIndex: zIndex
                    }
                });
            }
            else {
                // Only log suppression for non-temporary shapes to reduce console spam
                if (!isTemporaryShape) {
                    console.log(`Suppressing strokeColor event for shape ${this.id} (globalReplay: ${isGlobalReplaying}, shapeReplay: ${isShapeReplaying})`);
                }
            }
        };
        // Override setZIndex
        window.AbstractShape.prototype.setZIndex = function (zIndex) {
            const oldZIndex = this.getZIndex ? this.getZIndex() : this.zIndex;
            // Call original method
            window.AbstractShape.prototype._originalSetZIndex.call(this, zIndex);
            // Check if we should emit events (not during replay, not for replaying shapes, not for temporary shapes)
            const isGlobalReplaying = !!window._isReplaying;
            const isShapeReplaying = window._replayingShapes && window._replayingShapes.has(this.id);
            const isTemporaryShape = this.id >= 10000 || (window._temporaryShapes && window._temporaryShapes.has(this.id));
            const shouldEmitEvent = window.eventBus &&
                !isGlobalReplaying &&
                !isShapeReplaying &&
                !isTemporaryShape;
            if (shouldEmitEvent) {
                // Get current shape status - use 'this' if shape is not in canvas
                const shape = window.canvas.shapes[this.id] || this;
                const fillColor = shape.getFillColor ? shape.getFillColor() : shape.fillColor;
                const strokeColor = shape.getStrokeColor ? shape.getStrokeColor() : shape.strokeColor;
                const zIndex = shape.getZIndex ? shape.getZIndex() : shape.zIndex;
                window.eventBus.publish({
                    type: 'SHAPE_MODIFIED',
                    timestamp: Date.now(),
                    id: Date.now().toString(36) + Math.random().toString(36).substring(2, 5),
                    shapeId: this.id,
                    property: 'zIndex',
                    value: zIndex,
                    state: {
                        fillColor: fillColor,
                        strokeColor: strokeColor,
                        zIndex: zIndex
                    }
                });
            }
        };
    }
}
// Handle shape selection events
function handleShapeSelected(event) {
    if (!event.isFromServer)
        return; // Only handle remote selections
    console.log('EVENT-WRAPPER: Remote shape selected:', event.shapeId, 'by', event.clientId, 'color:', event.userColor);
    // Store remote selection info for visual feedback
    if (!window._remoteSelections) {
        window._remoteSelections = new Map();
    }
    // Convert shapeId to number for consistency with canvas
    const shapeIdNum = parseInt(event.shapeId);
    console.log('EVENT-WRAPPER: Storing remote selection for shapeId:', shapeIdNum, 'type:', typeof shapeIdNum);
    window._remoteSelections.set(shapeIdNum, {
        clientId: event.clientId,
        userColor: event.userColor
    });
    console.log('EVENT-WRAPPER: _remoteSelections after add:', window._remoteSelections);
    // Trigger canvas redraw to show remote selection
    if (window.canvas && window.canvas.draw) {
        window.canvas.draw();
    }
}
// Handle shape unselection events  
function handleShapeUnselected(event) {
    if (!event.isFromServer)
        return; // Only handle remote unselections
    console.log('EVENT-WRAPPER: Remote shape unselected:', event.shapeId, 'by', event.clientId);
    // Remove remote selection info
    if (window._remoteSelections) {
        const shapeIdNum = parseInt(event.shapeId);
        console.log('EVENT-WRAPPER: Removing remote selection for shapeId:', shapeIdNum);
        window._remoteSelections.delete(shapeIdNum);
        console.log('EVENT-WRAPPER: _remoteSelections after remove:', window._remoteSelections);
    }
    // Trigger canvas redraw to remove remote selection visual
    if (window.canvas && window.canvas.draw) {
        window.canvas.draw();
    }
}
//# sourceMappingURL=event-wrapper.js.map