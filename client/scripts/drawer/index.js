import { Point2D } from './models.js';
import { AbstractShape } from './abstract-shapes.js';
import { Canvas } from './canvas.js';
import { createShapeFromData } from './utils.js';
import { Line, LineFactory } from './shapes/line.js';
import { Circle, CircleFactory } from './shapes/circle.js';
import { Rectangle, RectangleFactory } from './shapes/rectangle.js';
import { Triangle, TriangleFactory } from './shapes/triangle.js';
import { SelectionTool, ToolArea } from './tools.js';
// Main initialization function
function init() {
    console.log("Drawer System init() starting...");
    // Robust DOM element validation
    const canvasDomElm = document.getElementById("drawArea");
    if (!canvasDomElm) {
        const error = new Error("drawArea canvas element not found!");
        console.error("[Drawer Init] Critical DOM element missing:", error);
        throw error;
    }
    console.log("drawArea canvas found:", canvasDomElm);
    // Robust tools menu validation
    const toolsCollection = document.getElementsByClassName("tools");
    const menu = toolsCollection.length > 0 ? toolsCollection[0] : null;
    if (!menu) {
        const error = new Error("tools menu element not found!");
        console.error("[Drawer Init] Critical DOM element missing:", error);
        console.error("[Drawer Init] Available tools elements:", toolsCollection.length);
        throw error;
    }
    console.log("tools menu found:", menu, `(${toolsCollection.length} elements)`);
    // Validate menu is actually appendable
    if (typeof menu.appendChild !== 'function') {
        const error = new Error("tools menu is not a valid DOM element!");
        console.error("[Drawer Init] Invalid DOM element:", typeof menu, menu);
        throw error;
    }
    console.log("DOM validation complete, initializing drawer components...");
    // No state loading - always start fresh
    let canvas;
    const sm = {
        addShape(shape, redraw = true, isTemp = false) {
            if (canvas)
                canvas.addShape(shape, redraw, isTemp);
            return this;
        },
        removeShape(shape, redraw = true) {
            if (canvas)
                canvas.removeShape(shape, redraw);
            return this;
        },
        removeShapeWithId(id, redraw = true) {
            if (canvas)
                canvas.removeShapeWithId(id, redraw);
            return this;
        },
        redraw() {
            if (canvas)
                canvas.draw();
            return this;
        }
    };
    // Create selection tool first with the ShapeManager
    console.log("Creating selection tool...");
    const selectionTool = new SelectionTool(sm);
    // Then add all shape factories including the selection tool
    console.log("Creating shape factories...");
    const shapesSelector = [
        selectionTool,
        new LineFactory(sm),
        new CircleFactory(sm),
        new RectangleFactory(sm),
        new TriangleFactory(sm)
    ];
    console.log(`Created ${shapesSelector.length} shape tools`);
    // Create tool area with validated menu element
    console.log("Creating tool area with menu element:", menu);
    const toolArea = new ToolArea(shapesSelector, menu);
    // Now initialize the canvas
    console.log("Creating canvas with DOM element and tool area...");
    canvas = new Canvas(canvasDomElm, toolArea);
    // Set cross-references between components
    console.log("Setting up component cross-references...");
    canvas.setSelectionTool(selectionTool);
    selectionTool.setCanvas(canvas);
    // Expose the canvas globally
    window.canvas = canvas;
    console.log("Canvas exposed globally");
    console.log("CANVAS DEBUG: window.canvas =", !!window.canvas);
    console.log("CANVAS DEBUG: window.eventBus =", !!window.eventBus);
    console.log("CANVAS DEBUG: window.eventStore =", !!window.eventStore);
    // Dispatch canvas-ready event for event-wrapper to patch methods
    console.log("Dispatching canvas-ready event...");
    window.dispatchEvent(new Event('canvas-ready'));
    // Draw all shapes at once
    console.log("Initial canvas draw...");
    canvas.draw();
    // Select the Selection tool by default with validation
    console.log("Setting default tool selection...");
    const toolElements = menu.getElementsByTagName('li');
    console.log(`Found ${toolElements.length} tool elements in menu`);
    if (toolElements && toolElements.length > 0) {
        console.log("Clicking first tool (SelectionTool)...");
        toolElements[0].click();
        console.log("Default tool selected");
    }
    else {
        console.warn("No tool elements found to select");
    }
    console.log("Drawer System initialization complete!");
    return { canvas, toolArea, selectionTool, menu };
}
// Make sure to expose the initialization function and AbstractShape to the global scope
window.init = init;
window.AbstractShape = AbstractShape;
// Expose shape classes to global scope to enable event replay functionality
window.Point2D = Point2D;
window.Line = Line;
window.Circle = Circle;
window.Rectangle = Rectangle;
window.Triangle = Triangle;
window.createShapeFromData = createShapeFromData;
// Create a drawer namespace to hold all drawing-related classes
window.Drawer = {
    Point2D,
    Line,
    Circle,
    Rectangle,
    Triangle,
    AbstractShape,
    Canvas,
    createShapeFromData,
    utils: {
        createShapeFromData
    }
};
// Export all necessary classes and functions
export { AbstractShape, Point2D, Line, LineFactory, Circle, CircleFactory, Rectangle, RectangleFactory, Triangle, TriangleFactory, SelectionTool, ToolArea, Canvas, createShapeFromData };
//# sourceMappingURL=index.js.map