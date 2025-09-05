import { Point2D } from '../models.js';
import { AbstractShape, AbstractFactory } from '../abstract-shapes.js';
import { drawSelectionHandle } from '../utils.js';
import { COLORS } from '../constants.js';
export class Circle extends AbstractShape {
    constructor(center, radius) {
        super();
        this.center = center;
        this.radius = radius;
    }
    draw(ctx, isSelected = false, remoteSelection = null) {
        ctx.save();
        // Set fill and stroke styles
        ctx.fillStyle = this.fillColor;
        ctx.strokeStyle = this.strokeColor;
        ctx.beginPath();
        ctx.arc(this.center.x, this.center.y, this.radius, 0, 2 * Math.PI);
        // Fill if not transparent
        if (this.fillColor !== COLORS.TRANSPARENT) {
            ctx.fill();
        }
        ctx.stroke();
        // Draw remote selection border if shape is selected by another user
        if (remoteSelection && !isSelected) {
            ctx.save();
            ctx.strokeStyle = remoteSelection.userColor;
            ctx.lineWidth = 3;
            ctx.setLineDash([5, 5]);
            ctx.beginPath();
            ctx.arc(this.center.x, this.center.y, this.radius + 2, 0, 2 * Math.PI);
            ctx.stroke();
            ctx.restore();
        }
        // Draw local selection handles if selected by current user
        if (isSelected) {
            drawSelectionHandle(ctx, this.center.x, this.center.y);
            drawSelectionHandle(ctx, this.center.x, this.center.y - this.radius);
            drawSelectionHandle(ctx, this.center.x + this.radius, this.center.y);
            drawSelectionHandle(ctx, this.center.x, this.center.y + this.radius);
            drawSelectionHandle(ctx, this.center.x - this.radius, this.center.y);
        }
        ctx.restore();
    }
    isPointInside(point) {
        const distance = this.calculateDistance(this.center, point);
        // Check if point is inside the circle (distance <= radius)
        return distance <= this.radius;
    }
    isPointNear(point, tolerance = 5) {
        const distance = this.calculateDistance(this.center, point);
        // Check if point is near the perimeter
        return Math.abs(distance - this.radius) <= tolerance;
    }
    isEqual(shape) {
        // Check if shape is a Circle
        if (!(shape instanceof Circle)) {
            return false;
        }
        const otherCircle = shape;
        // Compare circle centers and radii
        return this.pointsEqual(this.center, otherCircle.center) &&
            Math.abs(this.radius - otherCircle.radius) < 0.001;
    }
    clone() {
        const clonedCircle = new Circle(new Point2D(this.center.x, this.center.y), this.radius);
        // Copy properties
        clonedCircle.setFillColor(this.fillColor);
        clonedCircle.setStrokeColor(this.strokeColor);
        clonedCircle.setZIndex(this.zIndex);
        return clonedCircle;
    }
}
export class CircleFactory extends AbstractFactory {
    constructor(shapeManager) {
        super(shapeManager);
        this.label = "Kreis";
    }
    createShape(from, to) {
        return new Circle(from, CircleFactory.computeRadius(from, to.x, to.y));
    }
    static computeRadius(from, x, y) {
        const xDiff = (from.x - x), yDiff = (from.y - y);
        return Math.sqrt(xDiff * xDiff + yDiff * yDiff);
    }
}
//# sourceMappingURL=circle.js.map