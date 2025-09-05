import { Point2D } from '../models.js';
import { AbstractShape, AbstractFactory } from '../abstract-shapes.js';
import { drawSelectionHandle } from '../utils.js';
import { COLORS } from '../constants.js';
export class Triangle extends AbstractShape {
    constructor(p1, p2, p3) {
        super();
        this.p1 = p1;
        this.p2 = p2;
        this.p3 = p3;
    }
    draw(ctx, isSelected = false, remoteSelection = null) {
        ctx.save();
        // Set fill and stroke styles
        ctx.fillStyle = this.fillColor;
        ctx.strokeStyle = this.strokeColor;
        ctx.beginPath();
        ctx.moveTo(this.p1.x, this.p1.y);
        ctx.lineTo(this.p2.x, this.p2.y);
        ctx.lineTo(this.p3.x, this.p3.y);
        ctx.closePath();
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
            ctx.moveTo(this.p1.x, this.p1.y);
            ctx.lineTo(this.p2.x, this.p2.y);
            ctx.lineTo(this.p3.x, this.p3.y);
            ctx.closePath();
            ctx.stroke();
            ctx.restore();
        }
        // Draw local selection handles if selected by current user
        if (isSelected) {
            drawSelectionHandle(ctx, this.p1.x, this.p1.y);
            drawSelectionHandle(ctx, this.p2.x, this.p2.y);
            drawSelectionHandle(ctx, this.p3.x, this.p3.y);
        }
        ctx.restore();
    }
    isPointInside(point) {
        // Check if point is inside using barycentric coordinates
        const area = 0.5 * (-this.p2.y * this.p3.x + this.p1.y * (-this.p2.x + this.p3.x) +
            this.p1.x * (this.p2.y - this.p3.y) + this.p2.x * this.p3.y);
        const s = 1 / (2 * area) * (this.p1.y * this.p3.x - this.p1.x * this.p3.y +
            (this.p3.y - this.p1.y) * point.x + (this.p1.x - this.p3.x) * point.y);
        const t = 1 / (2 * area) * (this.p1.x * this.p2.y - this.p1.y * this.p2.x +
            (this.p1.y - this.p2.y) * point.x + (this.p2.x - this.p1.x) * point.y);
        return s >= 0 && t >= 0 && (1 - s - t) >= 0;
    }
    isPointNear(point, tolerance = 5) {
        // Check if point is inside the triangle with tolerance
        // First check if it's already inside
        if (this.isPointInside(point)) {
            return true;
        }
        // Check distance to each edge of the triangle
        const edges = [
            { from: this.p1, to: this.p2 },
            { from: this.p2, to: this.p3 },
            { from: this.p3, to: this.p1 }
        ];
        for (const edge of edges) {
            const vectorLine = {
                x: edge.to.x - edge.from.x,
                y: edge.to.y - edge.from.y
            };
            const vectorPoint = {
                x: point.x - edge.from.x,
                y: point.y - edge.from.y
            };
            const lineLength = Math.sqrt(vectorLine.x * vectorLine.x + vectorLine.y * vectorLine.y);
            if (lineLength < 0.001) {
                if (this.calculateDistance(edge.from, point) <= tolerance) {
                    return true;
                }
                continue;
            }
            const crossProduct = Math.abs(vectorLine.x * vectorPoint.y - vectorLine.y * vectorPoint.x);
            const distance = crossProduct / lineLength;
            const dotProduct = vectorLine.x * vectorPoint.x + vectorLine.y * vectorPoint.y;
            let edgeDistance;
            if (dotProduct < 0) {
                edgeDistance = this.calculateDistance(edge.from, point);
            }
            else if (dotProduct > lineLength * lineLength) {
                edgeDistance = this.calculateDistance(edge.to, point);
            }
            else {
                edgeDistance = distance;
            }
            if (edgeDistance <= tolerance) {
                return true;
            }
        }
        return false;
    }
    isEqual(shape) {
        // Basic type check
        if (!(shape instanceof Triangle)) {
            return false;
        }
        const otherTriangle = shape;
        // Compare colors
        if (this.fillColor !== otherTriangle.fillColor ||
            this.strokeColor !== otherTriangle.strokeColor) {
            return false;
        }
        // Compare points directly - no need for complex permutation checks
        // We just need to check if the triangles cover the same area
        // Calculate area for both triangles
        const area1 = this.calculateTriangleArea(this.p1, this.p2, this.p3);
        const area2 = this.calculateTriangleArea(otherTriangle.p1, otherTriangle.p2, otherTriangle.p3);
        // Check if areas are approximately equal
        if (Math.abs(area1 - area2) > 0.001) {
            return false;
        }
        // Check if the triangles share at least one point
        return this.pointsEqual(this.p1, otherTriangle.p1) ||
            this.pointsEqual(this.p1, otherTriangle.p2) ||
            this.pointsEqual(this.p1, otherTriangle.p3) ||
            this.pointsEqual(this.p2, otherTriangle.p1) ||
            this.pointsEqual(this.p2, otherTriangle.p2) ||
            this.pointsEqual(this.p2, otherTriangle.p3) ||
            this.pointsEqual(this.p3, otherTriangle.p1) ||
            this.pointsEqual(this.p3, otherTriangle.p2) ||
            this.pointsEqual(this.p3, otherTriangle.p3);
    }
    // Helper method to calculate triangle area
    calculateTriangleArea(p1, p2, p3) {
        return Math.abs((p1.x * (p2.y - p3.y) + p2.x * (p3.y - p1.y) + p3.x * (p1.y - p2.y)) / 2);
    }
    clone() {
        const clonedTriangle = new Triangle(new Point2D(this.p1.x, this.p1.y), new Point2D(this.p2.x, this.p2.y), new Point2D(this.p3.x, this.p3.y));
        // Copy properties
        clonedTriangle.setFillColor(this.fillColor);
        clonedTriangle.setStrokeColor(this.strokeColor);
        clonedTriangle.setZIndex(this.zIndex);
        return clonedTriangle;
    }
}
export class TriangleFactory extends AbstractFactory {
    constructor(shapeManager) {
        super(shapeManager);
        this.shapeManager = shapeManager;
        this.label = "Dreieck";
        this.pointCount = 0;
    }
    createShape(from, to, third) {
        if (third) {
            return new Triangle(from, to, third);
        }
        else {
            // Calculate the third point for an equilateral triangle
            const dx = to.x - from.x;
            const dy = to.y - from.y;
            const thirdX = from.x + dx * 0.5 - dy * 0.866;
            const thirdY = from.y + dy * 0.5 + dx * 0.866;
            return new Triangle(from, to, new Point2D(thirdX, thirdY));
        }
    }
    handleMouseDown(x, y) {
        if (this.pointCount === 0) {
            // First point
            super.handleMouseDown(x, y);
            this.pointCount = 1;
        }
        else if (this.pointCount === 1) {
            // Second point - save
            this.secondPoint = new Point2D(x, y);
            this.pointCount = 2;
        }
        else {
            // Third point - create final triangle
            const triangle = this.createShape(this.from, this.secondPoint, new Point2D(x, y));
            this.removeTemporaryShape();
            // Apply current colors to the new triangle
            if (window.colorState) {
                console.log('TriangleFactory: Applying current colors to new triangle', triangle.id);
                window.colorState.applyColorsToShape(triangle);
            }
            else {
                console.warn('TriangleFactory: colorState not available, using default colors');
            }
            this.shapeManager.addShape(triangle);
            // Reset for next triangle
            this.resetState();
            this.secondPoint = null;
            this.pointCount = 0;
        }
    }
    handleMouseMove(x, y) {
        if (this.pointCount === 0)
            return;
        if (this.pointCount === 1) {
            // Preview from first to current point
            super.handleMouseMove(x, y);
        }
        else if (this.pointCount === 2) {
            // Preview of entire triangle
            this.removeTemporaryShape();
            // Create temporary triangle
            const tmpTriangle = this.createShape(this.from, this.secondPoint, new Point2D(x, y));
            const tempId = AbstractShape.tempCounter++;
            tmpTriangle.id = tempId;
            this.tempShapeId = tempId;
            // Apply current colors to temporary triangle for preview
            if (window.colorState) {
                window.colorState.applyColorsToShape(tmpTriangle);
            }
            this.shapeManager.addShape(tmpTriangle, true, true);
        }
    }
    handleMouseUp(x, y) {
        // Mouse up is handled by mouse down in this implementation
        // This creates a more click-based interface instead of drag-based
    }
    resetState() {
        super.resetState();
        this.secondPoint = null;
        this.pointCount = 0;
    }
}
//# sourceMappingURL=triangle.js.map