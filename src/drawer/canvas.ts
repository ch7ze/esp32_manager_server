import { Shape, ShapeManager, Point2D } from './models.js';
import { SelectionTool } from './tools.js';
import { COLORS, getColorName } from './constants.js';
import { canvasWidth, canvasHeight } from './constants.js';
import { Line } from './shapes/line.js';

export class Canvas implements ShapeManager {
    public shapes: {[p: number]: Shape} = {};
    private tempShapes: {[p: number]: Shape} = {}; 
    private ctx: CanvasRenderingContext2D;
    private selectionTool: SelectionTool;

    // Simplified z-index tracking
    private zIndexCounter: number = 1000;
    
    // State saving debouncing
    private saveStateTimeout: number = null;
    private readonly SAVE_DEBOUNCE_MS = 500; // Wait 500ms before saving
   
    constructor(canvasDomElement: HTMLCanvasElement, toolarea: any) {
        this.shapes = {};
        this.tempShapes = {};
        this.ctx = canvasDomElement.getContext("2d");
        
        // Add event listeners
        canvasDomElement.addEventListener("mousemove",
            createMouseHandler("handleMouseMove"));
        canvasDomElement.addEventListener("mousedown",
            createMouseHandler("handleMouseDown"));
        canvasDomElement.addEventListener("mouseup",
            createMouseHandler("handleMouseUp"));        // Function to create mouse event handlers
        function createMouseHandler(methodName: string) {
            return function(ev: MouseEvent) {
                // Only log for mousedown events to avoid spam
                if (methodName === 'handleMouseDown') {
                    console.log(`[Canvas] createMouseHandler called for ${methodName} at ${new Date().toLocaleTimeString()}`);
                    console.log(`[Canvas] Mouse coordinates: (${ev.clientX - canvasDomElement.getBoundingClientRect().left}, ${ev.clientY - canvasDomElement.getBoundingClientRect().top})`);
                    
                    const selectedShape = toolarea.getSelectedShape();
                    console.log(`[Canvas] Selected shape:`, selectedShape ? (selectedShape as any).label || selectedShape.constructor.name : 'null');
                }
                
                const rect = canvasDomElement.getBoundingClientRect();
                const x = ev.clientX - rect.left;
                const y = ev.clientY - rect.top;
                const selectedShape = toolarea.getSelectedShape();
                
                if (selectedShape && typeof selectedShape[methodName] === 'function') {
                    if (methodName === 'handleMouseDown') {
                        console.log(`[Canvas] Calling ${methodName} on ${(selectedShape as any).label || selectedShape.constructor.name}`);
                    }
                    // Pass the event to the handler in case it needs access to modifier keys
                    selectedShape[methodName](x, y, ev);
                } else {
                    if (methodName === 'handleMouseDown') {
                        console.log(`[Canvas] Cannot call ${methodName}: selectedShape=${!!selectedShape}, method=${typeof (selectedShape as any)?.[methodName]}`);
                    }
                }
            };
        }
        
        // Add context menu event listener
        canvasDomElement.addEventListener("contextmenu", (e: MouseEvent) => {
            e.preventDefault(); // Prevent default browser context menu
            
            // Get mouse coordinates relative to canvas
            const rect = canvasDomElement.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            
            this.showShapeContextMenu(x, y);
            
            return false;
        });

        canvasDomElement.addEventListener("mousemove", (e: MouseEvent) => {
            const rect = canvasDomElement.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            
            if (this.isPointOverShape(x, y)) {
                canvasDomElement.style.cursor = 'pointer';
            } else {
                canvasDomElement.style.cursor = 'default';
            }
        });
    }

    // Set the selection tool reference
    setSelectionTool(tool: SelectionTool) {
        this.selectionTool = tool;
    }    // Update addShape method to save state after adding shapes
    addShape(shape: Shape, redraw: boolean = true, isTemp: boolean = false): this {
        if (isTemp) {
            // Add to temp shapes
            this.tempShapes[shape.id] = shape;
        } else {
            // Check if identical shape already exists
            let isDuplicate = false;
            
            // Quick ID check first
            if (this.shapes[shape.id] !== undefined) {
                console.warn(`Shape with ID ${shape.id} already exists, checking for duplicate`);
                isDuplicate = true;
            } else {
                // Then content comparison using isEqual
                for (const id in this.shapes) {
                    const existingShape = this.shapes[id];
                    if (existingShape && existingShape.isEqual && 
                        existingShape.isEqual(shape)) {
                        console.warn(`Shape content identical to ID ${id}, treating as duplicate`);
                        isDuplicate = true;
                        break;
                    }
                }
            }
            
            if (!isDuplicate) {
                // Add to permanent shapes
                this.shapes[shape.id] = shape;
                
                // Update z-index tracking
                const zIndex = (shape as any).getZIndex ? (shape as any).getZIndex() : shape.id;
                this.zIndexCounter = Math.max(this.zIndexCounter, zIndex);
                
                // Save state after adding a permanent shape
                this.saveState();
            } else {
                console.warn(`Shape not added as it was detected as duplicate`);
            }
        }
        
        return redraw ? this.draw() : this;
    }

    removeShapeWithId(id: number, redraw: boolean = true): this {
        if (this.tempShapes[id] !== undefined) {
            delete this.tempShapes[id];
        } else if (this.shapes[id] !== undefined) {
            delete this.shapes[id];
            
            this.saveState();
        }
        
        return redraw ? this.draw() : this;
    }

    draw(): this {
        this.ctx.beginPath();
        this.ctx.fillStyle = 'lightgrey';
        this.ctx.fillRect(0, 0, canvasWidth, canvasHeight);
        this.ctx.stroke();

        // Get selected shape IDs
        const selectedShapeIds = this.selectionTool ? 
            this.selectionTool.getSelectedShapeIds() : new Set<number>();

        // Draw permanent shapes in z-order
        const orderedShapes = this.getOrderedShapes();
        
        this.ctx.fillStyle = 'black';
        for (const shape of orderedShapes) {
            const isLocallySelected = selectedShapeIds.has(shape.id);
            const remoteSelection = (window as any)._remoteSelections ? (window as any)._remoteSelections.get(shape.id) : null;
            shape.draw(this.ctx, isLocallySelected, remoteSelection);
        }
        
        // Then draw temporary shapes (always on top)
        Object.keys(this.tempShapes).map(id => this.tempShapes[Number(id)]).forEach(shape => {
            const isLocallySelected = selectedShapeIds.has(shape.id);
            const remoteSelection = (window as any)._remoteSelections ? (window as any)._remoteSelections.get(shape.id) : null;
            shape.draw(this.ctx, isLocallySelected, remoteSelection);
        });
        
        return this;
    }

    private getOrderedShapes(): Shape[] {
        const shapeArray = Object.keys(this.shapes).map(id => this.shapes[Number(id)]);
        
        // Sort by z-index if available, otherwise by ID
        return shapeArray.sort((a, b) => {
            const aZ = (a as any).getZIndex ? (a as any).getZIndex() : a.id;
            const bZ = (b as any).getZIndex ? (b as any).getZIndex() : b.id;
            return aZ - bZ;
        });
    }

    bringToFront(shapeId: number): this {
        const shape = this.shapes[shapeId];
        if (shape && (shape as any).setZIndex) {
            this.zIndexCounter += 10;
            (shape as any).setZIndex(this.zIndexCounter);
            this.draw();
        }
        return this;
    }

    sendToBack(shapeId: number): this {
        const shape = this.shapes[shapeId];
        if (shape && (shape as any).setZIndex) {
            // Find minimum z-index
            const minZ = Math.min(...Object.keys(this.shapes)
                .map(id => this.shapes[Number(id)])
                .map(s => (s as any).getZIndex ? (s as any).getZIndex() : s.id));
            (shape as any).setZIndex(minZ - 10);
            this.draw();
        }
        return this;
    }

    public saveState(): void {
        this.debouncedSaveState();
    }
    
    private debouncedSaveState(): void {
        // Clear existing timeout
        if (this.saveStateTimeout !== null) {
            clearTimeout(this.saveStateTimeout);
        }
        
        // Schedule new save
        this.saveStateTimeout = setTimeout(() => {
            this.executeSaveState();
            this.saveStateTimeout = null;
        }, this.SAVE_DEBOUNCE_MS);
    }
    
    private executeSaveState(): void {
        // Save to drawerState if available (canvas-specific)
        if (window.drawerState && typeof window.drawerState.saveShapes === 'function') {
            // Get current canvas ID from drawerState or URL
            const canvasId = window.drawerState.currentCanvasId || this.getCurrentCanvasId();
            window.drawerState.saveShapes(this.shapes, canvasId);
            console.log(`Canvas state saved for ${canvasId} with ${Object.keys(this.shapes).length} shapes`);
        }
    }
    
    // Force immediate save (for critical operations like page unload)
    public forceSaveState(): void {
        if (this.saveStateTimeout !== null) {
            clearTimeout(this.saveStateTimeout);
            this.saveStateTimeout = null;
        }
        this.executeSaveState();
    }

    // Helper method to get canvas ID from URL
    private getCurrentCanvasId(): string | null {
        const currentPath = window.location.pathname;
        const canvasMatch = currentPath.match(/^\/canvas\/([^\/]+)$/);
        return canvasMatch ? canvasMatch[1] : null;
    }

    removeShape(shape: Shape, redraw: boolean = true): this {
        return this.removeShapeWithId(shape.id, redraw);
    }

    redraw(): this {
        return this.draw();
    }

    findShapeAt(x: number, y: number): Shape {
        for (let id in this.shapes) {
            const shape = this.shapes[id];
            return shape;
        }
        return null;
    }
    
    getShapeById(id: number): Shape {
        return this.shapes[id];
    }

    private showShapeContextMenu(x: number, y: number) {
        if (!window['menuApi']) {
            console.error("Menu API not available");
            return;
        }
        
        const selectedShapeIds = this.selectionTool.getSelectedShapeIds();
        if (selectedShapeIds.size === 0) {
            return;
        }
        
        const menu = window['menuApi'].createMenu();
        
        const firstShapeId = Array.from(selectedShapeIds)[0];
        const shape = this.getShapeById(firstShapeId);
        if (!shape) {
            return;
        }
    
        const fillColorOptions = {
            [COLORS.TRANSPARENT]: 'transparent',
            [COLORS.RED]: 'rot',
            [COLORS.GREEN]: 'grün',
            [COLORS.YELLOW]: 'gelb',
            [COLORS.BLUE]: 'blau',
            [COLORS.BLACK]: 'schwarz'
        };
        
        const fillColorRadio = window['menuApi'].createRadioOption(
            'Hintergrundfarbe', 
            fillColorOptions, 
            shape.getFillColor()
        );
        
        fillColorRadio.setOnChange((colorValue) => {
            selectedShapeIds.forEach(id => {
                const selectedShape = this.getShapeById(id);
                if (selectedShape && selectedShape.setFillColor && selectedShape.clone) {
                    // Remove old shape first, then create new one with updated color
                    this.removeShapeWithId(id, false);
                    const newShape = selectedShape.clone();
                    newShape.setFillColor(colorValue);
                    this.addShape(newShape, false);
                }
            });
            this.draw();
            this.saveState();
        });
        
        menu.addItem(fillColorRadio);
        menu.addItem(window['menuApi'].createSeparator());
    
        // STROKE COLOR SECTION
        const strokeColorOptions = {
            [COLORS.RED]: 'rot',
            [COLORS.GREEN]: 'grün',
            [COLORS.YELLOW]: 'gelb',
            [COLORS.BLUE]: 'blau',
            [COLORS.BLACK]: 'schwarz'
        };
        
        const strokeColorRadio = window['menuApi'].createRadioOption(
            'Rahmenfarbe', 
            strokeColorOptions, 
            shape.getStrokeColor()
        );
        
        strokeColorRadio.setOnChange((colorValue) => {
            selectedShapeIds.forEach(id => {
                const selectedShape = this.getShapeById(id);
                if (selectedShape && selectedShape.setStrokeColor && selectedShape.clone) {
                    // Remove old shape first, then create new one with updated color
                    this.removeShapeWithId(id, false);
                    const newShape = selectedShape.clone();
                    newShape.setStrokeColor(colorValue);
                    this.addShape(newShape, false);
                }
            });
            this.draw();
            this.saveState();
        });
        
        menu.addItem(strokeColorRadio);
        menu.addItem(window['menuApi'].createSeparator());
    
        // Z-ORDER CONTROLS
        const bringToFrontItem = window['menuApi'].createItem('In den Vordergrund', () => {
            selectedShapeIds.forEach(id => {
                this.bringToFront(id);
            });
            this.saveState();
        });
        menu.addItem(bringToFrontItem);
    
        const sendToBackItem = window['menuApi'].createItem('In den Hintergrund', () => {
            selectedShapeIds.forEach(id => {
                this.sendToBack(id);
            });
            this.saveState();
        });
        menu.addItem(sendToBackItem);
    
        menu.addItem(window['menuApi'].createSeparator());
    
        // DELETE OPTION
        const deleteItem = window['menuApi'].createItem('Löschen', () => {
            selectedShapeIds.forEach(id => {
                this.removeShapeWithId(id, false);
            });
            this.selectionTool.clearSelection();
            this.draw();
            this.saveState();
        });
        menu.addItem(deleteItem);
    
        // Get canvas position for showing the menu
        const canvasElement = document.getElementById("drawArea");
        const rect = canvasElement.getBoundingClientRect();
        
        // Show the menu at the cursor position
        menu.show(x + rect.left, y + rect.top);
    }

    // Add a method to get all shapes
    getShapes(): {[p: number]: Shape} {
        return this.shapes;
    }

    getTempShapes(): {[p: number]: Shape} {
        return this.tempShapes;
    }

    private isPointOverShape(x: number, y: number): boolean {
        const point = new Point2D(x, y);
        
        for (const id in this.shapes) {
            const shape = this.shapes[id];
            
            if (shape instanceof Line) {
                if (shape.isPointNear(point)) {
                    return true;
                }
            } else if (shape.isPointInside && shape.isPointInside(point)) {
                return true;
            }
        }
        
        return false;
    }

    // Extract shape data for event system
    private extractShapeData(shape: Shape): any {
        // Common properties - now guaranteed by Shape interface
        const data = {
            fillColor: shape.getFillColor(),
            strokeColor: shape.getStrokeColor(),
            zIndex: shape.getZIndex()
        };
        
        console.log(`CANVAS: Extracting data for shape ${shape.id}:`, data);
        console.log(`CANVAS: Shape fillColor: ${shape.getFillColor()}, strokeColor: ${shape.getStrokeColor()}`);
        
        // Shape-specific properties based on robust shape type detection
        if ((shape as any).from && (shape as any).to && !(shape as any).p1) {
            // Could be Line or Rectangle
            if (typeof (shape as any).radius === 'undefined') {
                // It's a Line or Rectangle
                return {
                    ...data,
                    from: { x: (shape as any).from.x, y: (shape as any).from.y },
                    to: { x: (shape as any).to.x, y: (shape as any).to.y }
                };
            }
        } 
        else if ((shape as any).center && typeof (shape as any).radius === 'number') {
            // It's a Circle
            return {
                ...data,
                center: { x: (shape as any).center.x, y: (shape as any).center.y },
                radius: (shape as any).radius
            };
        } 
        else if ((shape as any).p1 && (shape as any).p2 && (shape as any).p3) {
            // It's a Triangle
            return {
                ...data,
                p1: { x: (shape as any).p1.x, y: (shape as any).p1.y },
                p2: { x: (shape as any).p2.x, y: (shape as any).p2.y },
                p3: { x: (shape as any).p3.x, y: (shape as any).p3.y }
            };
        }
        
        return data;
    }

    // Helper method to identify and remove duplicate shapes from collection
    removeDuplicateShapes(): this {
        const uniqueShapes: {[id: number]: Shape} = {};
        const duplicateIds: number[] = [];
        
        // Iterate over all shapes and check against already processed ones
        Object.keys(this.shapes).forEach(idStr => {
            const id = parseInt(idStr);
            const shape = this.shapes[id];
            let isDuplicate = false;
            
            // Check against already identified unique shapes
            for (const uniqueId in uniqueShapes) {
                const uniqueShape = uniqueShapes[uniqueId];
                if (uniqueShape.isEqual && uniqueShape.isEqual(shape)) {
                    isDuplicate = true;
                    duplicateIds.push(id);
                    console.warn(`Shape with ID ${id} is duplicate of ${uniqueId}, removing`);
                    break;
                }
            }
            
            if (!isDuplicate) {
                uniqueShapes[id] = shape;
            }
        });
        
        // Remove all duplicates
        duplicateIds.forEach(id => {
            delete this.shapes[id];
        });
        
        // If shapes were removed, save state
        if (duplicateIds.length > 0) {
            this.saveState();
            console.log(`${duplicateIds.length} duplicate shapes removed`);
        }
        
        return this;
    }

    // Helper method to clone a shape and add to canvas
    cloneAndAddShape(shapeId: number): Shape {
        const sourceShape = this.shapes[shapeId];
        if (!sourceShape || !sourceShape.clone) {
            console.error(`Shape with ID ${shapeId} does not exist or doesn't support cloning`);
            return null;
        }
        
        // Create clone
        const clonedShape = sourceShape.clone();
        
        // Increase z-index so it appears above original
        if ((clonedShape as any).setZIndex) {
            this.zIndexCounter += 1;
            (clonedShape as any).setZIndex(this.zIndexCounter);
        }
        
        // Add to canvas
        this.addShape(clonedShape);
        
        return clonedShape;
    }
}