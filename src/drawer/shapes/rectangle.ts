import { Point2D, Shape, ShapeManager, ShapeFactory } from '../models.js';
import { AbstractShape, AbstractFactory } from '../abstract-shapes.js';
import { drawSelectionHandle } from '../utils.js';
import { COLORS } from '../constants.js';

export class Rectangle extends AbstractShape implements Shape {
    constructor(readonly from: Point2D, readonly to: Point2D) {
        super();
    }

    draw(ctx: CanvasRenderingContext2D, isSelected: boolean = false, remoteSelection: any = null) {
        ctx.save();
        
        // Set fill and stroke styles
        ctx.fillStyle = this.fillColor;
        ctx.strokeStyle = this.strokeColor;
        
        const width = this.to.x - this.from.x;
        const height = this.to.y - this.from.y;
        
        ctx.beginPath();
        
        // Fill first if not transparent
        if (this.fillColor !== COLORS.TRANSPARENT) {
            ctx.fillRect(this.from.x, this.from.y, width, height);
        }
        
        // Then stroke
        ctx.strokeRect(this.from.x, this.from.y, width, height);
        
        // Draw remote selection border if shape is selected by another user
        if (remoteSelection && !isSelected) {
            ctx.save();
            ctx.strokeStyle = remoteSelection.userColor;
            ctx.lineWidth = 3;
            ctx.setLineDash([5, 5]);
            ctx.strokeRect(this.from.x - 2, this.from.y - 2, width + 4, height + 4);
            ctx.restore();
        }
        
        // Draw local selection handles if selected by current user
        if (isSelected) {
            drawSelectionHandle(ctx, this.from.x, this.from.y);
            drawSelectionHandle(ctx, this.to.x, this.from.y);
            drawSelectionHandle(ctx, this.to.x, this.to.y);
            drawSelectionHandle(ctx, this.from.x, this.to.y);
        }
        
        ctx.restore();
    }
      isPointInside(point: Point2D): boolean {
        // Handle cases where to is to the left or above from
        const minX = Math.min(this.from.x, this.to.x);
        const maxX = Math.max(this.from.x, this.to.x);
        const minY = Math.min(this.from.y, this.to.y);
        const maxY = Math.max(this.from.y, this.to.y);
        
        return point.x >= minX && point.x <= maxX && 
               point.y >= minY && point.y <= maxY;
    }
    
    isPointNear(point: Point2D, tolerance: number = 5): boolean {
        // Check if point is near the rectangle edges
        const minX = Math.min(this.from.x, this.to.x);
        const maxX = Math.max(this.from.x, this.to.x);
        const minY = Math.min(this.from.y, this.to.y);
        const maxY = Math.max(this.from.y, this.to.y);
        
        // Check if point is inside the rectangle (including tolerance)
        if (point.x >= minX - tolerance && point.x <= maxX + tolerance && 
            point.y >= minY - tolerance && point.y <= maxY + tolerance) {
            return true;
        }
        
        return false;
    }
    
    isEqual(shape: Shape): boolean {
        // Check if shape is a Rectangle
        if (!(shape instanceof Rectangle)) {
            return false;
        }
        
        const otherRect = shape as Rectangle;
        
        // Compare corner points
        // Since rectangles can be drawn from different corners,
        // we need to compare normalized coordinates
        const thisMinX = Math.min(this.from.x, this.to.x);
        const thisMaxX = Math.max(this.from.x, this.to.x);
        const thisMinY = Math.min(this.from.y, this.to.y);
        const thisMaxY = Math.max(this.from.y, this.to.y);
        
        const otherMinX = Math.min(otherRect.from.x, otherRect.to.x);
        const otherMaxX = Math.max(otherRect.from.x, otherRect.to.x);
        const otherMinY = Math.min(otherRect.from.y, otherRect.to.y);
        const otherMaxY = Math.max(otherRect.from.y, otherRect.to.y);
        
        return Math.abs(thisMinX - otherMinX) < 0.001 &&
               Math.abs(thisMaxX - otherMaxX) < 0.001 &&
               Math.abs(thisMinY - otherMinY) < 0.001 &&
               Math.abs(thisMaxY - otherMaxY) < 0.001;
    }
    
    clone(): Shape {
        const clonedRect = new Rectangle(
            new Point2D(this.from.x, this.from.y),
            new Point2D(this.to.x, this.to.y)
        );
        
        // Copy properties
        clonedRect.setFillColor(this.fillColor);
        clonedRect.setStrokeColor(this.strokeColor);
        clonedRect.setZIndex(this.zIndex);
        
        return clonedRect;
    }
}

export class RectangleFactory extends AbstractFactory<Rectangle> implements ShapeFactory {
    public label: string = "Rechteck";
    
    constructor(shapeManager: ShapeManager){
        super(shapeManager);
    }

    createShape(from: Point2D, to: Point2D): Rectangle {
        return new Rectangle(from, to);
    }
}