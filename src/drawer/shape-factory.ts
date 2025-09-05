import { Point2D, Shape } from './models.js';
import { Line } from './shapes/line.js';
import { Circle } from './shapes/circle.js';
import { Rectangle } from './shapes/rectangle.js';
import { Triangle } from './shapes/triangle.js';
import { ShapeCreatedEvent } from './events.js';

export interface ShapeData {
    type: string;
    id: number;
    fillColor?: string;
    strokeColor?: string;
    zIndex?: number;
    // Shape-specific properties
    from?: Point2D;
    to?: Point2D;
    center?: Point2D;
    radius?: number;
    p1?: Point2D;
    p2?: Point2D;
    p3?: Point2D;
}

export class ShapeFactory {
    /**
     * Creates a shape from event data during replay
     */
    static createFromEventData(event: ShapeCreatedEvent): Shape | null {
        const { shapeType, data } = event;
        
        try {
            console.log('Creating shape from event data:', shapeType, data);
            
            let shape: Shape | null = null;
            
            switch (shapeType) {
                case 'Line':
                    if (data.from && data.to) {
                        shape = new Line(
                            new Point2D(data.from.x, data.from.y),
                            new Point2D(data.to.x, data.to.y)
                        );
                    }
                    break;
                    
                case 'Rectangle':
                    if (data.from && data.to) {
                        shape = new Rectangle(
                            new Point2D(data.from.x, data.from.y),
                            new Point2D(data.to.x, data.to.y)
                        );
                    }
                    break;
                    
                case 'Circle':
                    if (data.center && typeof data.radius === 'number') {
                        shape = new Circle(
                            new Point2D(data.center.x, data.center.y),
                            data.radius
                        );
                    }
                    break;
                    
                case 'Triangle':
                    if (data.p1 && data.p2 && data.p3) {
                        shape = new Triangle(
                            new Point2D(data.p1.x, data.p1.y),
                            new Point2D(data.p2.x, data.p2.y),
                            new Point2D(data.p3.x, data.p3.y)
                        );
                    }
                    break;
                    
                default:
                    console.error('Unknown shape type:', shapeType);
                    return null;
            }
            
            if (shape) {
                // Set ID - override the auto-generated one
                (shape as any).id = event.shapeId;
                
                // Set properties - ensure they are applied correctly
                if (data.fillColor !== undefined && typeof shape.setFillColor === 'function') {
                    console.log(`Setting fillColor to ${data.fillColor} for shape ${shape.id}`);
                    shape.setFillColor(data.fillColor);
                }
                
                if (data.strokeColor !== undefined && typeof shape.setStrokeColor === 'function') {
                    console.log(`Setting strokeColor to ${data.strokeColor} for shape ${shape.id}`);
                    shape.setStrokeColor(data.strokeColor);
                }
                
                if (data.zIndex !== undefined) {
                    console.log(`Setting zIndex to ${data.zIndex} for shape ${shape.id}`);
                    shape.setZIndex(data.zIndex);
                }
                
                // Verify properties were set correctly
                console.log(`Shape ${shape.id} properties after creation:`, {
                    fillColor: shape.getFillColor(),
                    strokeColor: shape.getStrokeColor(),
                    zIndex: shape.getZIndex()
                });
                
                console.log(`Created shape with ID ${shape.id} using ShapeFactory`);
                return shape;
            }
            
            console.error('Failed to create shape - invalid data provided');
            return null;
        } catch (error) {
            console.error(`Error creating ${shapeType}:`, error);
            return null;
        }
    }

    /**
     * Extracts shape data for event serialization
     */
    static extractShapeData(shape: Shape): ShapeData {
        // Common properties - ensure we get current values
        const data: ShapeData = {
            type: shape.constructor.name,
            id: shape.id,
            fillColor: shape.getFillColor(),
            strokeColor: shape.getStrokeColor(),
            zIndex: shape.getZIndex()
        };
        
        console.log(`Extracting data for shape ${shape.id}:`, data);
        
        // Add shape-specific properties based on shape type
        const shapeAny = shape as any;
        
        if (shapeAny.from && shapeAny.to && !shapeAny.p1) {
            // Line or Rectangle
            return {
                ...data,
                from: { x: shapeAny.from.x, y: shapeAny.from.y },
                to: { x: shapeAny.to.x, y: shapeAny.to.y }
            };
        } 
        else if (shapeAny.center && typeof shapeAny.radius === 'number') {
            // Circle
            return {
                ...data,
                center: { x: shapeAny.center.x, y: shapeAny.center.y },
                radius: shapeAny.radius
            };
        } 
        else if (shapeAny.p1 && shapeAny.p2 && shapeAny.p3) {
            // Triangle
            return {
                ...data,
                p1: { x: shapeAny.p1.x, y: shapeAny.p1.y },
                p2: { x: shapeAny.p2.x, y: shapeAny.p2.y },
                p3: { x: shapeAny.p3.x, y: shapeAny.p3.y }
            };
        }
        
        return data;
    }

    /**
     * Determines shape type from shape instance
     */
    static getShapeType(shape: Shape): string {
        // First method: Use constructor name if available
        if (shape && shape.constructor && typeof shape.constructor.name === 'string') {
            return shape.constructor.name;
        }
        
        // Second method: Check properties for more robust type detection
        const shapeAny = shape as any;
        
        if (shapeAny.center && typeof shapeAny.radius === 'number') {
            return 'Circle';
        }
        else if (shapeAny.from && shapeAny.to && !shapeAny.p1) {
            if (typeof shapeAny.isPointInside === 'function') {
                return 'Rectangle';
            } else {
                return 'Line';
            }
        }
        else if (shapeAny.p1 && shapeAny.p2 && shapeAny.p3) {
            return 'Triangle';
        }
        
        return 'Unknown';
    }
}