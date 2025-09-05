import { Point2D } from './models.js';
import { Line } from './shapes/line.js';
import { Rectangle } from './shapes/rectangle.js';
import { Circle } from './shapes/circle.js';
import { Triangle } from './shapes/triangle.js';
import { Shape } from './models.js';
import { COLORS, COLOR_MAP } from './constants.js';

export function drawSelectionHandle(ctx: CanvasRenderingContext2D, x: number, y: number, color: string = (window as any).currentUserColor || "#0066cc") {
    const handleSize = 6;
    ctx.save();
    ctx.fillStyle = color;
    ctx.fillRect(x - handleSize/2, y - handleSize/2, handleSize, handleSize);
    ctx.restore();
}

// Shape factory function to reuse for shape creation from data
export function createShapeFromData(shapeData): Shape {
    if (!shapeData || !shapeData.type) {
        console.error("Invalid shape data", shapeData);
        return null;
    }
    
    try {
        // Create shape based on type
        let shape = null;
        
        // Generic point validation
        const validatePoints = (...points) => {
            return points.every(p => isValidPoint(p));
        };
        
        // Create appropriate shape based on type
        switch(shapeData.type) {
            case 'Line':
                if (!validatePoints(shapeData.from, shapeData.to)) {
                    throw new Error("Invalid Line data: missing or invalid points");
                }
                shape = new Line(
                    new Point2D(shapeData.from.x, shapeData.from.y),
                    new Point2D(shapeData.to.x, shapeData.to.y)
                );
                break;
                
            case 'Rectangle':
                if (!validatePoints(shapeData.from, shapeData.to)) {
                    throw new Error("Invalid Rectangle data: missing or invalid points");
                }
                shape = new Rectangle(
                    new Point2D(shapeData.from.x, shapeData.from.y),
                    new Point2D(shapeData.to.x, shapeData.to.y)
                );
                break;
                
            case 'Circle':
                if (!validatePoints(shapeData.center) || typeof shapeData.radius !== 'number') {
                    throw new Error("Invalid Circle data: missing center or radius");
                }
                shape = new Circle(
                    new Point2D(shapeData.center.x, shapeData.center.y),
                    shapeData.radius
                );
                break;
                
            case 'Triangle':
                if (!validatePoints(shapeData.p1, shapeData.p2, shapeData.p3)) {
                    throw new Error("Invalid Triangle data: missing or invalid points");
                }
                shape = new Triangle(
                    new Point2D(shapeData.p1.x, shapeData.p1.y),
                    new Point2D(shapeData.p2.x, shapeData.p2.y),
                    new Point2D(shapeData.p3.x, shapeData.p3.y)
                );
                break;
                
            default:
                throw new Error("Unknown shape type: " + shapeData.type);
        }
        
        // Set common properties if shape was created
        if (shape) {
            // Set ID
            shape.id = parseInt(shapeData.id);
            
            // Log what colors we're trying to set
            console.log(`createShapeFromData: Setting colors for shape ${shapeData.id}`);
            console.log(`Input fillColor: ${shapeData.fillColor} (valid: ${isValidColor(shapeData.fillColor)})`);
            console.log(`Input strokeColor: ${shapeData.strokeColor} (valid: ${isValidColor(shapeData.strokeColor)})`);
            
            // Set colors with validation and conversion
            const finalFillColor = isValidColor(shapeData.fillColor) ? 
                (COLOR_MAP[shapeData.fillColor] || shapeData.fillColor) : COLORS.TRANSPARENT;
            const finalStrokeColor = isValidColor(shapeData.strokeColor) ? 
                (COLOR_MAP[shapeData.strokeColor] || shapeData.strokeColor) : COLORS.BLACK;
                
            console.log(`Final fillColor: ${finalFillColor}`);
            console.log(`Final strokeColor: ${finalStrokeColor}`);
            
            shape.setFillColor(finalFillColor);
            shape.setStrokeColor(finalStrokeColor);
            
            // Verify colors were actually set
            console.log(`Verification - shape.getFillColor(): ${shape.getFillColor()}`);
            console.log(`Verification - shape.getStrokeColor(): ${shape.getStrokeColor()}`);
            
            // Set z-index if supported
            if (typeof shape.setZIndex === 'function') {
                const zIndex = typeof shapeData.zIndex === 'number' ? 
                    shapeData.zIndex : parseInt(shapeData.id);
                shape.setZIndex(zIndex);
            }
        }
        
        return shape;
    } catch (e) {
        console.error(`Error creating ${shapeData.type}:`, e);
        return null;
    }
}

// Simplified point validation
function isValidPoint(point): boolean {
    return point && typeof point.x === 'number' && typeof point.y === 'number' && 
           !isNaN(point.x) && !isNaN(point.y);
}

// Helper function for color validation
function isValidColor(color): boolean {
    if (typeof color !== 'string') return false;
    
    // Check if it is one of the predefined colors (Hex)
    const predefinedColors = [
        COLORS.RED, COLORS.GREEN, COLORS.BLUE, COLORS.YELLOW, 
        COLORS.BLACK, COLORS.TRANSPARENT
    ];
    if (predefinedColors.indexOf(color) >= 0) return true;
    
    // Check if it is a German color name from COLOR_MAP
    if (COLOR_MAP.hasOwnProperty(color)) return true;
    
    // Check if it is a valid CSS color value (hex, rgb, etc.)
    const s = new Option().style;
    s.color = color;
    return s.color !== '';
}