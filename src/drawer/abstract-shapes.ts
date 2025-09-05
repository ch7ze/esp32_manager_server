import { Shape, Point2D, ShapeManager } from './models.js';
import { COLORS } from './constants.js';

export abstract class AbstractShape {
    public static counter: number = 0;
    public static tempCounter: number = 10000;
    readonly id: number;
    protected fillColor: string = COLORS.TRANSPARENT;
    protected strokeColor: string = COLORS.BLACK;
    protected zIndex: number = 0; 

    constructor() {
        this.id = AbstractShape.counter++;
        this.zIndex = this.id;
    }
    
    getFillColor(): string {
        return this.fillColor;
    }

    getStrokeColor(): string {
        return this.strokeColor;
    }

    setFillColor(color: string): void {
        this.fillColor = color;
    }

    setStrokeColor(color: string): void {
        this.strokeColor = color;
    }

    getZIndex(): number {
        return this.zIndex;
    }

    setZIndex(index: number): void {
        this.zIndex = index;
    }
    
    // Abstract draw method that all shapes must implement
    abstract draw(ctx: CanvasRenderingContext2D, isSelected?: boolean, remoteSelection?: any): void;
    
    // Helper method to compare two points for equality with tolerance
    protected pointsEqual(p1: Point2D, p2: Point2D, tolerance: number = 0.001): boolean {
        return Math.abs(p1.x - p2.x) < tolerance && Math.abs(p1.y - p2.y) < tolerance;
    }
    
    // Common helper method to calculate distance between two points
    protected calculateDistance(p1: Point2D, p2: Point2D): number {
        const dx = p2.x - p1.x;
        const dy = p2.y - p1.y;
        return Math.sqrt(dx * dx + dy * dy);
    }
    
    // Base implementation for isEqual that can be overridden by derived classes
    isEqual(shape: Shape): boolean {
        // Basic type check
        if (this.constructor !== shape.constructor) {
            return false;
        }
        
        return this.fillColor === shape.getFillColor() &&
               this.strokeColor === shape.getStrokeColor();
    }
    
    // Method to clone a shape (must be implemented in derived classes)
    clone(): Shape {
        throw new Error("The 'clone' method must be implemented in a derived class");
    }
}

export abstract class AbstractFactory<T extends Shape> {
    protected from: Point2D;
    protected tmpTo: Point2D;
    protected tmpShape: T;
    protected tempShapeId: number | null = null;

    constructor(readonly shapeManager: ShapeManager) {}

    abstract createShape(from: Point2D, to: Point2D): T;

    handleMouseDown(x: number, y: number) {
        this.from = new Point2D(x, y);
    }    handleMouseUp(x: number, y: number) {
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
            if ((window as any).colorState) {
                console.log('AbstractFactory: Applying current colors to new shape', (shape as any).id);
                (window as any).colorState.applyColorsToShape(shape);
                
                // Log final colors applied
                const finalFillColor = typeof shape.getFillColor === 'function' ? shape.getFillColor() : (shape as any).fillColor;
                const finalStrokeColor = typeof shape.getStrokeColor === 'function' ? shape.getStrokeColor() : (shape as any).strokeColor;
                console.log('AbstractFactory: Final shape colors - fill:', finalFillColor, ', stroke:', finalStrokeColor);
            } else {
                console.warn('AbstractFactory: colorState not available, using default colors');
            }
            
            // Add as permanent shape
            this.shapeManager.addShape(shape, true, false);
            
            // Clean up state for next operation
            this.resetState();
            
            console.log('Shape created successfully');
        } catch (error) {
            console.error('Error creating shape:', error);
            // Clean up state on errors too
            this.resetState();
            this.tempShapeId = null;
        }
    }

    handleMouseMove(x: number, y: number) {
        if (!this.from) {
            return;
        }
        
        if (!this.tmpTo || (this.tmpTo.x !== x || this.tmpTo.y !== y)) {
            this.updateTemporaryShape(x, y);
        }
    }
    
    // Helper methods to reduce code duplication
    protected removeTemporaryShape(): void {
        if (this.tempShapeId !== null) {
            this.shapeManager.removeShapeWithId(this.tempShapeId, false);
            this.tempShapeId = null;
        }
    }
    
    protected updateTemporaryShape(x: number, y: number): void {
        this.tmpTo = new Point2D(x, y);
        
        // Remove old temporary shape
        this.removeTemporaryShape();
        
        // Create new temporary shape with special temp ID
        this.tmpShape = this.createShape(this.from, this.tmpTo);
        
        const tempId = AbstractShape.tempCounter++;
        (this.tmpShape as any).id = tempId;
        this.tempShapeId = tempId;
        
        // Apply current colors to temporary shape for preview
        if ((window as any).colorState) {
            (window as any).colorState.applyColorsToShape(this.tmpShape);
        }
        
        // Add as temporary shape
        this.shapeManager.addShape(this.tmpShape, true, true);
    }    protected resetState(): void {
        this.from = undefined;
        this.tmpTo = undefined;
        this.tmpShape = undefined;
        this.tempShapeId = null;
    }
}