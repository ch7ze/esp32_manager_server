import { Point2D, Shape, ShapeFactory, ShapeManager } from './models.js';
import { Canvas } from './canvas.js';
import { AbstractShape } from './abstract-shapes.js';

export class SelectionTool implements ShapeFactory {
    label: string = "Auswahl";
    private startPoint: Point2D;
    private selectedShapes: Set<number> = new Set();
    private canvas: Canvas = null;
    
    // Tracking variables for overlapping shapes
    private shapesAtClickPoint: number[] = [];
    private cycleIndex: number = -1;
    private lastClickPoint: Point2D = null;
      // Drag & Drop variables
    private isDragging: boolean = false;
    private dragStartPoint: Point2D = null;
    private shapesBeingDragged: Map<number, Shape> = new Map();
    private dragOffset: Point2D = null;
    private hasLoggedDragStart: boolean = false;
    
    // Performance optimization - redraw throttling
    private redrawThrottleId: number = null;
    private lastRedrawTime: number = 0;
    private readonly REDRAW_THROTTLE_MS = 16; // ~60fps max
    
    constructor(private shapeManager: ShapeManager) {}
    
    setCanvas(canvas: Canvas): void {
        this.canvas = canvas;
    }    handleMouseDown(x: number, y: number, ev: MouseEvent) {
        
        this.startPoint = new Point2D(x, y);
        this.dragStartPoint = new Point2D(x, y);
        this.hasLoggedDragStart = false; // Reset drag logging flag
        
        // Find all shapes at the current position
        this.findAllShapesAt(x, y);
        
        // Check if shape was already selected BEFORE selection logic
        const wasAlreadySelected = this.shapesAtClickPoint.length > 0 ? 
            this.selectedShapes.has(this.shapesAtClickPoint[0]) : false;
        
        console.log(`DEBUG: shapesAtClickPoint=${this.shapesAtClickPoint}, wasAlreadySelected=${wasAlreadySelected}, selectedShapes=${Array.from(this.selectedShapes)}`);
        
        // Handle different selection modes
        try {
            if (ev.altKey) {
                this.handleAltSelection();
            } else {
                this.handleNormalSelection(ev.ctrlKey);
            }
        } catch (error) {
            console.error(`[SelectionTool] CRITICAL ERROR in selection handling:`, error);
            console.error(`[SelectionTool] Error message:`, error.message);
            console.error(`[SelectionTool] Error stack:`, error.stack);
            throw error; // Re-throw to see it in console
        }
        
        
        // Only prepare for dragging if shape was ALREADY selected before this click
        if (this.shapesAtClickPoint.length > 0 && wasAlreadySelected) {
            this.prepareDragOperation(x, y);
        }
        
        // Store click point for future reference
        this.lastClickPoint = new Point2D(x, y);
        
        // Redraw to show selection
        this.redrawCanvas();
    }
    
    private handleAltSelection(): void {
        if (this.shapesAtClickPoint.length === 0) return;
        
        this.clearSelection();
        this.cycleToNextShape();
    }      private handleNormalSelection(isCtrlPressed: boolean): void {
        try {
            if (!isCtrlPressed) {
                this.clearSelection();
            }
            
            if (this.shapesAtClickPoint.length > 0) {
                const shapeToSelect = this.shapesAtClickPoint[0];
                
                // Check if shape is remotely selected by another client
                const remoteSelections = (window as any)._remoteSelections;
                if (remoteSelections && remoteSelections.has(shapeToSelect)) {
                    const remoteSelection = remoteSelections.get(shapeToSelect);
                    console.log(`[SelectionTool] Shape ${shapeToSelect} is locked by ${remoteSelection.clientId} (${remoteSelection.userColor}) - selection blocked`);
                    return; // Block selection
                }
                
                this.selectedShapes.add(shapeToSelect);
                this.publishSelectionEvent(shapeToSelect, 'selected');
                this.cycleIndex = 0;
            }
        } catch (error) {
            console.error(`[SelectionTool] ERROR in handleNormalSelection:`, error);
            console.error(`[SelectionTool] Error stack:`, error.stack);
        }
    }private findAllShapesAt(x: number, y: number): void {
        const point = new Point2D(x, y);
        let shapes = this.getShapesCollection();
        
        this.shapesAtClickPoint = [];
        
        if (!shapes) {
            return;
        }
        
        // Get all shape IDs and sort by z-index (highest first for top-most selection)
        const shapeEntries: { id: number, shape: Shape, zIndex: number }[] = [];
        const shapeIds = Object.keys(shapes);
        
        for (let i = 0; i < shapeIds.length; i++) {
            const id = parseInt(shapeIds[i]);
            const shape = shapes[id];
            const zIndex = (shape as any).getZIndex ? (shape as any).getZIndex() : shape.id;
            shapeEntries.push({ id, shape, zIndex });
        }
        
        // Sort by z-index descending (top-most first)
        shapeEntries.sort((a, b) => b.zIndex - a.zIndex);
        
        // Find shapes containing the point
        for (const entry of shapeEntries) {
            if (this.isPointOnShape(entry.shape, point)) {
                this.shapesAtClickPoint.push(entry.id);
            }
        }
    }
    
    private getShapesCollection(): {[p: number]: Shape} {
        if (this.canvas) {
            return this.canvas.getShapes();
        } else if ((this.shapeManager as any).shapes) {
            return (this.shapeManager as any).shapes;
        }
        return null;
    }    private isPointOnShape(shape: Shape, point: Point2D): boolean {
        // First try isPointInside (more precise for filled shapes)
        if (shape.isPointInside) {
            const insideResult = shape.isPointInside(point);
            if (insideResult) {
                console.log(`[SelectionTool] isPointOnShape for shape ${shape.id}: isPointInside=true`);
                return true;
            }
        }
        
        // Then try isPointNear (for edges and lines)
        if (shape.isPointNear) {
            const nearResult = shape.isPointNear(point);
            console.log(`[SelectionTool] isPointOnShape for shape ${shape.id}: isPointInside=false, isPointNear=${nearResult}`);
            return nearResult;
        }
        
        console.log(`[SelectionTool] isPointOnShape for shape ${shape.id}: no detection methods available`);
        return false;
    }
    
    private cycleToNextShape(): void {
        if (this.shapesAtClickPoint.length === 0) return;
        
        this.cycleIndex = (this.cycleIndex + 1) % this.shapesAtClickPoint.length;
        const nextShapeId = this.shapesAtClickPoint[this.cycleIndex];
        
        // Check if shape is remotely selected by another client
        const remoteSelections = (window as any)._remoteSelections;
        if (remoteSelections && remoteSelections.has(nextShapeId)) {
            const remoteSelection = remoteSelections.get(nextShapeId);
            console.log(`[SelectionTool] Shape ${nextShapeId} is locked by ${remoteSelection.clientId} (${remoteSelection.userColor}) - cycle selection blocked`);
            return; // Block selection
        }
        
        this.selectedShapes.add(nextShapeId);
        this.publishSelectionEvent(nextShapeId, 'selected');
    }
    
    private redrawCanvas(): void {
        if (this.canvas) {
            this.canvas.draw();
        } else if (this.shapeManager.redraw) {
            this.shapeManager.redraw();
        }
    }    handleMouseUp(x: number, y: number) {
        console.log(`[SelectionTool] handleMouseUp: isDragging=${this.isDragging}, dragStartPoint=${this.dragStartPoint ? `(${this.dragStartPoint.x},${this.dragStartPoint.y})` : 'null'}`);
        
        if (this.isDragging && this.dragStartPoint) {
            const deltaX = x - this.dragStartPoint.x;
            const deltaY = y - this.dragStartPoint.y;
            const actualMovement = Math.abs(deltaX) > 2 || Math.abs(deltaY) > 2;
            
            console.log(`[SelectionTool] Mouse up: deltaX=${deltaX}, deltaY=${deltaY}, actualMovement=${actualMovement}`);
            
            if (actualMovement) {
                // Only complete drag operation if there was actual movement
                this.completeDragOperation(x, y);
            } else {
                // This was just a click on a selected shape, not a drag
                console.log(`[SelectionTool] Click detected (no movement) - clearing drag state without operation`);
                
                // Clear any temporary shapes
                if (this.canvas) {
                    Object.keys(this.canvas.getTempShapes()).forEach(id => {
                        this.canvas.removeShapeWithId(parseInt(id), false);
                    });
                }
                
                // Clear the dragging flag immediately
                (window as any)._isDragging = false;
                this.shapesBeingDragged.clear();
            }
        }
        
        this.startPoint = undefined;
        this.isDragging = false;
        this.dragStartPoint = null;
        this.shapesBeingDragged.clear();
        this.dragOffset = null;
        this.hasLoggedDragStart = false;
        
        // Clear any pending redraw timeouts to prevent memory leaks
        if (this.redrawThrottleId !== null) {
            clearTimeout(this.redrawThrottleId);
            this.redrawThrottleId = null;
        }
        
        // Clear drag flag to allow normal event processing
        (window as any)._isDragging = false;
    }    handleMouseMove(x: number, y: number) {
        if (this.isDragging && this.dragStartPoint) {
            // Check if this is actual movement (not just minimal mouse jitter)
            const deltaX = x - this.dragStartPoint.x;
            const deltaY = y - this.dragStartPoint.y;
            const actualMovement = Math.abs(deltaX) > 2 || Math.abs(deltaY) > 2;
            
            if (actualMovement) {
                // Only log once when real dragging starts
                if (!this.hasLoggedDragStart) {
                    console.log(`[SelectionTool] Starting actual drag operation with movement: deltaX=${deltaX}, deltaY=${deltaY}`);
                    this.hasLoggedDragStart = true;
                }
                
                this.performDragOperation(x, y);
            }
            // If no actual movement, don't call performDragOperation - this prevents unnecessary shape manipulation
        }
    }
    
    getSelectedShapeIds(): Set<number> {
        return this.selectedShapes;
    }      clearSelection() {
        // Publish unselection events for all currently selected shapes
        this.selectedShapes.forEach(shapeId => {
            this.publishSelectionEvent(shapeId, 'unselected');
        });
        this.selectedShapes.clear();
    }

    // Publish shape selection/unselection events
    private publishSelectionEvent(shapeId: number, action: 'selected' | 'unselected'): void {
        if (!(window as any).eventBus || (window as any)._isDragging) {
            return; // Skip events during drag operations or if EventBus unavailable
        }
        
        const eventType = action === 'selected' ? 'SHAPE_SELECTED' : 'SHAPE_UNSELECTED';
        const event = {
            type: eventType,
            shapeId: shapeId,
            clientId: (window as any).clientId || 'unknown',
            userColor: (window as any).currentUserColor || '#666666',
            timestamp: Date.now(),
            id: (window as any).eventBus?.generateEventId ? (window as any).eventBus.generateEventId() : Date.now().toString()
        };
        
        console.log(`SelectionTool: Publishing ${eventType} event for shape ${shapeId}`, event);
        (window as any).eventBus.publish(event);
    }
    
    // Drag & Drop implementation methods
    private prepareDragOperation(x: number, y: number): void {
        if (!this.canvas) {
            console.error("[SelectionTool] Canvas is null in prepareDragOperation");
            return;
        }
        
        this.isDragging = true;
        this.shapesBeingDragged.clear();
        
        // Set drag flag to suppress events during drag operation
        (window as any)._isDragging = true;
        
        // Store all selected shapes that will be dragged
        const shapes = this.canvas.getShapes();
        
        this.selectedShapes.forEach(shapeId => {
            if (shapes[shapeId]) {
                this.shapesBeingDragged.set(shapeId, shapes[shapeId]);
            } else {
                console.warn(`[SelectionTool] Shape ${shapeId} not found in canvas shapes`);
            }
        });
    }
    
    private performDragOperation(x: number, y: number): void {
        if (!this.canvas || this.shapesBeingDragged.size === 0) {
            return;
        }
        
        const deltaX = x - this.dragStartPoint.x;
        const deltaY = y - this.dragStartPoint.y;
        
        // Clear all existing temporary shapes before creating new ones (but not during replay)
        if (!window._isReplaying) {
            Object.keys(this.canvas.getTempShapes()).forEach(id => {
                this.canvas.removeShapeWithId(parseInt(id), false);
            });
        }
        
        // Remove original shapes and add temporary moved shapes
        this.shapesBeingDragged.forEach((originalShape, shapeId) => {
            // Remove original shape temporarily (don't redraw yet)
            this.canvas.removeShapeWithId(shapeId, false);
            
            // Create moved copy and add as temporary shape
            const movedShape = this.createMovedShape(originalShape, deltaX, deltaY);
            if (movedShape) {
                this.canvas.addShape(movedShape, false, true);
            } else {
                console.error(`[SelectionTool] Failed to create moved shape for ${shapeId}`);
            }
        });
        
        // Redraw with throttling for performance
        this.throttledRedraw();
    }
      private completeDragOperation(x: number, y: number): void {
        if (!this.canvas || this.shapesBeingDragged.size === 0) {
            console.log(`[SelectionTool] completeDragOperation: No canvas or no shapes to drag`);
            return;
        }
        
        const deltaX = x - this.dragStartPoint.x;
        const deltaY = y - this.dragStartPoint.y;
        
        console.log(`[SelectionTool] completeDragOperation: deltaX=${deltaX}, deltaY=${deltaY}`);
        
        // If there was actual movement
        if (Math.abs(deltaX) > 2 || Math.abs(deltaY) > 2) {
            console.log(`[SelectionTool] Actual movement detected - completing drag operation`);
            
            // Clear temporary shapes first (no events needed for temp shapes)
            Object.keys(this.canvas.getTempShapes()).forEach(id => {
                this.canvas.removeShapeWithId(parseInt(id), false);
            });
            
            // Create new shapes at final positions and update selection
            const newSelectedShapes = new Set<number>();
            
            // IMPORTANT: Keep _isDragging = true during entire operation to avoid race conditions
            // Only clear flag after ALL events are sent
            
            // Send DELETE events for original shapes (they were removed during drag but event was suppressed)
            this.shapesBeingDragged.forEach((originalShape, oldShapeId) => {
                console.log(`[SelectionTool] Sending delayed DELETE event for original shape ${oldShapeId}`);
                if (window.eventBus) {
                    window.eventBus.publish({
                        type: 'SHAPE_DELETED',
                        timestamp: Date.now(),
                        id: Date.now().toString(36) + Math.random().toString(36).substring(2, 5),
                        shapeId: oldShapeId
                    });
                }
            });
            
            // Now allow events for CREATE operations
            (window as any)._isDragging = false;
            
            this.shapesBeingDragged.forEach((originalShape, oldShapeId) => {
                // A4-Style: Create new shape with new ID (no ID preservation)
                const movedShape = this.createMovedShape(originalShape, deltaX, deltaY);
                if (movedShape) {
                    console.log(`[SelectionTool] A4-Style: Adding final moved shape with NEW ID ${movedShape.id} (was ${oldShapeId})`);
                    this.canvas.addShape(movedShape, false);
                    newSelectedShapes.add(movedShape.id); // Use new ID for selection
                }
            });
            
            // Update selection to new shape IDs
            this.selectedShapes = newSelectedShapes;
            
            // Send SHAPE_SELECTED events for new shape IDs to maintain remote selection
            newSelectedShapes.forEach(newShapeId => {
                this.publishSelectionEvent(newShapeId, 'selected');
            });
            
            // Force immediate save to prevent position loss on reload
            this.canvas.forceSaveState();
            
        } else {
            // No significant movement - just clear isDragging flag and restore shapes if needed
            console.log(`[SelectionTool] No significant movement - treating as click, not drag`);
            
            // Clear any temporary shapes that might have been created
            Object.keys(this.canvas.getTempShapes()).forEach(id => {
                this.canvas.removeShapeWithId(parseInt(id), false);
            });
            
            // Restore original shapes only if they were removed during temp drag preview
            this.shapesBeingDragged.forEach((originalShape, shapeId) => {
                const currentShapes = this.canvas.getShapes();
                if (!currentShapes[shapeId]) {
                    // Shape was removed during drag preview, restore it
                    this.canvas.addShape(originalShape, false);
                }
            });
            
            // Clear the dragging flag immediately since this was just a click
            (window as any)._isDragging = false;
        }
        
        this.executeRedraw(); // Final redraw should be immediate
    }      // Throttled redraw for performance optimization
    private throttledRedraw(): void {
        const now = performance.now();
        
        // If enough time has passed since last redraw, draw immediately
        if (now - this.lastRedrawTime >= this.REDRAW_THROTTLE_MS) {
            this.executeRedraw();
            return;
        }
        
        // Otherwise schedule a redraw for later
        if (this.redrawThrottleId !== null) {
            return; // Already scheduled
        }
        
        const timeUntilNext = this.REDRAW_THROTTLE_MS - (now - this.lastRedrawTime);
        this.redrawThrottleId = setTimeout(() => {
            this.executeRedraw();
            this.redrawThrottleId = null;
        }, timeUntilNext);
    }
    
    private executeRedraw(): void {
        this.lastRedrawTime = performance.now();
        if (this.canvas) {
            this.canvas.draw();
        }
    }

    private createMovedShape(originalShape: Shape, deltaX: number, deltaY: number): Shape {
        
        // Import shape classes from window if needed
        const Point2D = (window as any).Point2D;
        const Line = (window as any).Line;
        const Circle = (window as any).Circle;
        const Rectangle = (window as any).Rectangle;
        const Triangle = (window as any).Triangle;
        
        
        if (!Point2D || !Line || !Circle || !Rectangle || !Triangle) {
            console.error("[SelectionTool] Shape classes not available on window");
            return null;
        }
        
        let movedShape: Shape = null;
        const originalShapeType = (originalShape as any).constructor.name;
        
        // Create new shape based on type with moved coordinates
        if ((originalShape as any).from && (originalShape as any).to && !(originalShape as any).center) {
            // Line or Rectangle
            const from = (originalShape as any).from;
            const to = (originalShape as any).to;
            
            
            if ((originalShape as any).constructor.name === 'Line') {
                movedShape = new Line(
                    new Point2D(from.x + deltaX, from.y + deltaY),
                    new Point2D(to.x + deltaX, to.y + deltaY)
                );
            } else {
                movedShape = new Rectangle(
                    new Point2D(from.x + deltaX, from.y + deltaY),
                    new Point2D(to.x + deltaX, to.y + deltaY)
                );
            }
        } else if ((originalShape as any).center) {
            // Circle
            const center = (originalShape as any).center;
            const radius = (originalShape as any).radius;
            
            movedShape = new Circle(
                new Point2D(center.x + deltaX, center.y + deltaY),
                radius
            );
        } else if ((originalShape as any).p1 && (originalShape as any).p2 && (originalShape as any).p3) {
            // Triangle
            const p1 = (originalShape as any).p1;
            const p2 = (originalShape as any).p2;
            const p3 = (originalShape as any).p3;
            
            movedShape = new Triangle(
                new Point2D(p1.x + deltaX, p1.y + deltaY),
                new Point2D(p2.x + deltaX, p2.y + deltaY),
                new Point2D(p3.x + deltaX, p3.y + deltaY)
            );
        } else {
            console.error(`[SelectionTool] Unknown shape structure for shape ${originalShape.id}`);
            console.log(`[SelectionTool] Shape properties:`, Object.keys(originalShape));
        }
        
        if (movedShape) {
            // Assign temporary ID like drawing shapes do
            const tempId = AbstractShape.tempCounter++;
            (movedShape as any).id = tempId;
            
            // Copy properties from original shape
            movedShape.setFillColor(originalShape.getFillColor());
            movedShape.setStrokeColor(originalShape.getStrokeColor());
            
            // Copy Z-Index if both shapes support it
            if ((originalShape as any).getZIndex && (movedShape as any).setZIndex) {
                (movedShape as any).setZIndex((originalShape as any).getZIndex());
            }
        } else {
            console.error("[SelectionTool] Failed to create moved shape");
        }
        
        return movedShape;
    }
}

export class ToolArea {
    private selectedShape: ShapeFactory = undefined;
    private domElements: HTMLElement[] = [];
    
    constructor(shapesSelector: ShapeFactory[], menue: Element) {
        console.log("[ToolArea] Initializing with menu element:", menue);
        console.log("[ToolArea] Creating tools for", shapesSelector?.length || 0, "shape selectors");
        
        // Validate required parameters
        if (!shapesSelector || !Array.isArray(shapesSelector)) {
            throw new Error("[ToolArea] shapesSelector must be a valid array");
        }
        
        if (!menue) {
            throw new Error("[ToolArea] menue (menu DOM element) is required");
        }
        
        if (typeof (menue as any).appendChild !== 'function') {
            throw new Error("[ToolArea] menue must be a valid DOM element with appendChild method");
        }
        
        const domElms: HTMLElement[] = [];
        
        console.log("[ToolArea] Creating DOM elements for tools...");
        shapesSelector.forEach((sl, index) => {
            if (!sl || !sl.label) {
                console.warn(`[ToolArea] Invalid shape selector at index ${index}:`, sl);
                return;
            }
            
            const domSelElement = document.createElement("li");
            domSelElement.innerText = sl.label;
            
            try {
                menue.appendChild(domSelElement);
                domElms.push(domSelElement);
                
                domSelElement.addEventListener("click", () => {
                    console.log(`[ToolArea] Tool selected: ${sl.label} (index: ${index})`);
                    this.selectFactory(sl, domSelElement, index);
                });
                
                console.log(`[ToolArea] Created tool: ${sl.label}`);
            } catch (error) {
                console.error(`[ToolArea] Failed to create tool ${sl.label}:`, error);
                throw error;
            }
        });
        
        console.log(`[ToolArea] Successfully created ${domElms.length} tools`);
        this.domElements = domElms;
    }

    selectFactory(sl: ShapeFactory, domElm: HTMLElement, index: number = -1) {
        const parent = domElm.parentElement;
        if (parent) {
            const allElements = parent.getElementsByTagName('li');
            for (let j = 0; j < allElements.length; j++) {
                allElements[j].classList.remove("marked");
            }
        }
        this.selectedShape = sl;

        domElm.classList.add("marked");
    }

    getSelectedShape(): ShapeFactory {
        return this.selectedShape;
    }
}

export class Shapes {}