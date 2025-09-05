import { COLORS } from './constants.js';

// Interface for common helper methods
export interface GeometryUtils {
    calculateDistance?(p1: Point2D, p2: Point2D): number;
}

export interface ShapeFactory {
    label: string;
    handleMouseDown(x: number, y: number, ev?: MouseEvent);
    handleMouseUp(x: number, y: number);
    handleMouseMove(x: number, y: number);
}

export interface Shape {
    readonly id: number;
    draw(ctx: CanvasRenderingContext2D, isSelected?: boolean, remoteSelection?: any);

    getFillColor(): string;
    getStrokeColor(): string;
    setFillColor(color: string): void;
    setStrokeColor(color: string): void;
    getZIndex(): number;
    setZIndex(index: number): void;
    isPointInside?(point: Point2D): boolean; 
    isPointNear?(point: Point2D, tolerance?: number): boolean;
    isEqual?(shape: Shape): boolean; // Method to compare shapes
    clone?(): Shape; // Method to clone a shape
}

export class Point2D {
    constructor(readonly x: number, readonly y: number) {}
}

export interface ShapeManager {
    addShape(shape: Shape, redraw?: boolean, isTemp?: boolean): this;
    removeShape(shape: Shape, redraw?: boolean): this;
    removeShapeWithId(id: number, redraw?: boolean): this;
    redraw(): this;
}