import { Point2D, Shape, ShapeManager, ShapeFactory } from '../models.js';
import { AbstractShape, AbstractFactory } from '../abstract-shapes.js';
import { drawSelectionHandle } from '../utils.js';

export class Line extends AbstractShape implements Shape {
    readonly from: Point2D;
    readonly to: Point2D;

    constructor(from: Point2D, to: Point2D) {
        super();
        this.from = from;
        this.to = to;
    }

    draw(ctx: CanvasRenderingContext2D, isSelected: boolean = false, remoteSelection: any = null) {
        ctx.save();
        ctx.beginPath();
        ctx.moveTo(this.from.x, this.from.y);
        ctx.lineTo(this.to.x, this.to.y);
        
        ctx.strokeStyle = this.getStrokeColor();
        ctx.stroke();
        
        // Draw remote selection border if shape is selected by another user
        if (remoteSelection && !isSelected) {
            ctx.save();
            ctx.strokeStyle = remoteSelection.userColor;
            ctx.lineWidth = 5;
            ctx.setLineDash([8, 4]);
            ctx.beginPath();
            ctx.moveTo(this.from.x, this.from.y);
            ctx.lineTo(this.to.x, this.to.y);
            ctx.stroke();
            ctx.restore();
        }
        
        // Draw local selection handles if selected by current user
        if (isSelected) {
            drawSelectionHandle(ctx, this.from.x, this.from.y);
            drawSelectionHandle(ctx, this.to.x, this.to.y);
        }
        ctx.restore();
    }    isPointInside(point: Point2D): boolean {
        // For lines, we consider a point "inside" if it's near the line
        return this.isPointNear(point, 3);
    }
    
    isPointNear(point: Point2D, tolerance: number = 5): boolean {
        // dist = ||(B-A)×(A-P)|| / ||B-A|| 
        const vectorLine = {
            x: this.to.x - this.from.x,
            y: this.to.y - this.from.y
        };
        
        const vectorPoint = {
            x: point.x - this.from.x,
            y: point.y - this.from.y
        };
        
        const lineLength = Math.sqrt(vectorLine.x * vectorLine.x + vectorLine.y * vectorLine.y);
        
        if (lineLength < 0.001) {
            return this.calculateDistance(this.from, point) <= tolerance;
        }
        
        const crossProduct = Math.abs(vectorLine.x * vectorPoint.y - vectorLine.y * vectorPoint.x);
        const distance = crossProduct / lineLength;
        const dotProduct = vectorLine.x * vectorPoint.x + vectorLine.y * vectorPoint.y;
        
        if (dotProduct < 0) {
            return this.calculateDistance(this.from, point) <= tolerance;
        } else if (dotProduct > lineLength * lineLength) {
            return this.calculateDistance(this.to, point) <= tolerance;
        }

        return distance <= tolerance;
    }
    
    isEqual(shape: Shape): boolean {
        // Check if shape is a Line
        if (!(shape instanceof Line)) {
            return false;
        }
        
        const otherLine = shape as Line;
        
        // Compare line endpoints (check both directions)
        return (this.pointsEqual(this.from, otherLine.from) && this.pointsEqual(this.to, otherLine.to)) ||
               (this.pointsEqual(this.from, otherLine.to) && this.pointsEqual(this.to, otherLine.from));
    }
    
    clone(): Shape {
        const clonedLine = new Line(
            new Point2D(this.from.x, this.from.y),
            new Point2D(this.to.x, this.to.y)
        );
        
        // Copy properties
        clonedLine.setStrokeColor(this.strokeColor);
        clonedLine.setZIndex(this.zIndex);
        
        return clonedLine;
    }
}

export class LineFactory extends AbstractFactory<Line> implements ShapeFactory {
    public label: string = "Linie";

    constructor(shapeManager: ShapeManager){
        super(shapeManager);
    }

    createShape(from: Point2D, to: Point2D): Line {
        return new Line(from, to);
    }
}