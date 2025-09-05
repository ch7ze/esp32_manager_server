import { Point2D } from './models.js';
import { COLORS } from './constants.js';
export class AbstractShape {
    constructor() {
        this.fillColor = COLORS.TRANSPARENT;
        this.strokeColor = COLORS.BLACK;
        this.zIndex = 0;
        this.id = AbstractShape.counter++;
        this.zIndex = this.id;
    }
    getFillColor() {
        return this.fillColor;
    }
    getStrokeColor() {
        return this.strokeColor;
    }
    setFillColor(color) {
        this.fillColor = color;
    }
    setStrokeColor(color) {
        this.strokeColor = color;
    }
    getZIndex() {
        return this.zIndex;
    }
    setZIndex(index) {
        this.zIndex = index;
    }
    // Helper method to compare two points for equality with tolerance
    pointsEqual(p1, p2, tolerance = 0.001) {
        return Math.abs(p1.x - p2.x) < tolerance && Math.abs(p1.y - p2.y) < tolerance;
    }
    // Common helper method to calculate distance between two points
    calculateDistance(p1, p2) {
        const dx = p2.x - p1.x;
        const dy = p2.y - p1.y;
        return Math.sqrt(dx * dx + dy * dy);
    }
    // Base implementation for isEqual that can be overridden by derived classes
    isEqual(shape) {
        // Basic type check
        if (this.constructor !== shape.constructor) {
            return false;
        }
        return this.fillColor === shape.getFillColor() &&
            this.strokeColor === shape.getStrokeColor();
    }
    // Method to clone a shape (must be implemented in derived classes)
    clone() {
        throw new Error("The 'clone' method must be implemented in a derived class");
    }
}
AbstractShape.counter = 0;
AbstractShape.tempCounter = 10000;
export class AbstractFactory {
    constructor(shapeManager) {
        this.shapeManager = shapeManager;
        this.tempShapeId = null;
    }
    handleMouseDown(x, y) {
        this.from = new Point2D(x, y);
    }
    handleMouseUp(x, y) {
        try {
            // Remove temporary shape first
            this.removeTemporaryShape();
            // Check if start point exists
            if (!this.from) {
                console.log('No start point available, ignoring mouseUp');
                return;
            }
            // Create final shape
            const finalTo = new Point2D(x, y);
            console.log('Creating final shape from', this.from.x, this.from.y, 'to', x, y);
            const shape = this.createShape(this.from, finalTo);
            // Apply current colors to the new shape
            if (window.colorState) {
                console.log('AbstractFactory: Applying current colors to new shape', shape.id);
                window.colorState.applyColorsToShape(shape);
                // Log final colors applied
                const finalFillColor = typeof shape.getFillColor === 'function' ? shape.getFillColor() : shape.fillColor;
                const finalStrokeColor = typeof shape.getStrokeColor === 'function' ? shape.getStrokeColor() : shape.strokeColor;
                console.log('AbstractFactory: Final shape colors - fill:', finalFillColor, ', stroke:', finalStrokeColor);
            }
            else {
                console.warn('AbstractFactory: colorState not available, using default colors');
            }
            // Add as permanent shape
            this.shapeManager.addShape(shape, true, false);
            // Clean up state for next operation
            this.resetState();
            console.log('Shape created successfully');
        }
        catch (error) {
            console.error('Error creating shape:', error);
            // Clean up state on errors too
            this.resetState();
            this.tempShapeId = null;
        }
    }
    handleMouseMove(x, y) {
        if (!this.from) {
            return;
        }
        if (!this.tmpTo || (this.tmpTo.x !== x || this.tmpTo.y !== y)) {
            this.updateTemporaryShape(x, y);
        }
    }
    // Helper methods to reduce code duplication
    removeTemporaryShape() {
        if (this.tempShapeId !== null) {
            this.shapeManager.removeShapeWithId(this.tempShapeId, false);
            this.tempShapeId = null;
        }
    }
    updateTemporaryShape(x, y) {
        this.tmpTo = new Point2D(x, y);
        // Remove old temporary shape
        this.removeTemporaryShape();
        // Create new temporary shape with special temp ID
        this.tmpShape = this.createShape(this.from, this.tmpTo);
        const tempId = AbstractShape.tempCounter++;
        this.tmpShape.id = tempId;
        this.tempShapeId = tempId;
        // Apply current colors to temporary shape for preview
        if (window.colorState) {
            window.colorState.applyColorsToShape(this.tmpShape);
        }
        // Add as temporary shape
        this.shapeManager.addShape(this.tmpShape, true, true);
    }
    resetState() {
        this.from = undefined;
        this.tmpTo = undefined;
        this.tmpShape = undefined;
        this.tempShapeId = null;
    }
}
//# sourceMappingURL=abstract-shapes.js.map