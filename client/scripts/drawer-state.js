// Check if already loaded to prevent duplicate declarations
if (typeof window.drawerState !== 'undefined') {
    console.log('drawerState already loaded, skipping redefinition');
} else {

// Global state manager for the drawer component
const drawerState = {
  canvasShapes: {}, // Canvas-specific shape storage: { canvasId: { shapes } }
  shapes: {}, // Legacy support - will be canvas-specific
  selectedToolIndex: -1,
  hasInitialized: false,
  currentCanvasId: null,
  readOnlyMode: false, // Read-only mode for entire canvas
  
  // Set current canvas ID for canvas-specific operations
  setCurrentCanvas: function(canvasId) {
    console.log(`DrawerState: Switching to canvas ${canvasId}`);
    this.currentCanvasId = canvasId;
    
    // Initialize canvas-specific storage if it doesn't exist
    if (!this.canvasShapes[canvasId]) {
      this.canvasShapes[canvasId] = {};
    }
    
    // Update legacy shapes reference to current canvas
    // But DON'T overwrite if shapes are already loaded for this canvas
    if (Object.keys(this.canvasShapes[canvasId]).length > 0) {
      console.log(`DrawerState: Canvas ${canvasId} has ${Object.keys(this.canvasShapes[canvasId]).length} existing shapes, preserving them`);
      this.shapes = this.canvasShapes[canvasId];
    } else {
      console.log(`DrawerState: Canvas ${canvasId} is empty, initializing`);
      this.shapes = this.canvasShapes[canvasId];
    }
  },

  // Save shapes in memory only (canvas-specific)
  saveShapes: function(shapes, canvasId = null) {
    const targetCanvasId = canvasId || this.currentCanvasId;
    
    if (!targetCanvasId) {
      console.warn('DrawerState: No canvas ID specified for saving shapes');
      return;
    }
    
    // Initialize canvas-specific storage if it doesn't exist
    if (!this.canvasShapes[targetCanvasId]) {
      this.canvasShapes[targetCanvasId] = {};
    }
    
    // Clear current canvas shapes
    this.canvasShapes[targetCanvasId] = {};
    
    for (const id in shapes) {
      const shape = shapes[id];
      
      const fillColor = shape.getFillColor ? shape.getFillColor() : 'transparent';
      const strokeColor = shape.getStrokeColor ? shape.getStrokeColor() : '#000000';
      const zIndex = shape.getZIndex ? shape.getZIndex() : parseInt(id);
      
      const shapeData = {
        id: shape.id,
        fillColor: fillColor,
        strokeColor: strokeColor,
        zIndex: zIndex
      };
      
      if (shape.center && shape.radius) {
        this.canvasShapes[targetCanvasId][id] = {
          ...shapeData,
          type: 'Circle',
          center: { x: shape.center.x, y: shape.center.y },
          radius: shape.radius
        };
      } else if (shape.from && shape.to) {
        const isRectangle = typeof shape.isPointInside === 'function';
        this.canvasShapes[targetCanvasId][id] = {
          ...shapeData,
          type: isRectangle ? 'Rectangle' : 'Line',
          from: { x: shape.from.x, y: shape.from.y },
          to: { x: shape.to.x, y: shape.to.y }
        };
      } else if (shape.p1 && shape.p2 && shape.p3) {
        this.canvasShapes[targetCanvasId][id] = {
          ...shapeData,
          type: 'Triangle',
          p1: { x: shape.p1.x, y: shape.p1.y },
          p2: { x: shape.p2.x, y: shape.p2.y },
          p3: { x: shape.p3.x, y: shape.p3.y }
        };
      }
    }
    
    // Update legacy shapes reference if this is the current canvas
    if (targetCanvasId === this.currentCanvasId) {
      this.shapes = this.canvasShapes[targetCanvasId];
    }
    
    console.log(`DrawerState: Shapes updated for canvas ${targetCanvasId} - no persistence`);
    
    if (window.eventBus) {
      this.notifyStateUpdated();
    }
  },
  
  // Save selected tool (in memory only)
  saveSelectedTool: function(index) {
    this.selectedToolIndex = index;
    
    // Emit tool selection event if we have an event bus
    if (window.eventBus) {
      window.eventBus.publish({
        type: 'TOOL_SELECTED',
        timestamp: Date.now(),
        id: Date.now().toString(36) + Math.random().toString(36).substring(2, 5),
        toolIndex: index
      });
    }
  },
  
  // No state loading - always start fresh
  loadState: function() {
    return false;
  },
  
  setInitialized: function(value = true) {
    this.hasInitialized = value;
  },
  
  // Reset state (canvas-specific)
  reset: function(canvasId = null) {
    const targetCanvasId = canvasId || this.currentCanvasId;
    
    if (targetCanvasId) {
      // Reset only the specific canvas
      if (this.canvasShapes[targetCanvasId]) {
        this.canvasShapes[targetCanvasId] = {};
      }
      
      // Update legacy shapes reference if this is the current canvas
      if (targetCanvasId === this.currentCanvasId) {
        this.shapes = {};
      }
      
      console.log(`DrawerState: Reset canvas ${targetCanvasId}`);
    } else {
      // Reset everything if no canvas specified
      this.canvasShapes = {};
      this.shapes = {};
      this.currentCanvasId = null;
      console.log('DrawerState: Reset all canvas data');
    }
    
    this.selectedToolIndex = -1;
    this.hasInitialized = false;
    
    // Emit a reset event if we have an event bus
    if (window.eventBus) {
      window.eventBus.publish({
        type: 'RESET_STATE',
        timestamp: Date.now(),
        id: Date.now().toString(36) + Math.random().toString(36).substring(2, 5),
        canvasId: targetCanvasId
      });
    }
  },
  
  // Notify that state has been updated
  notifyStateUpdated: function() {
    if (window.eventBus) {
      window.eventBus.publish({
        type: 'STATE_UPDATED',
        timestamp: Date.now(),
        id: Date.now().toString(36) + Math.random().toString(36).substring(2, 5)
      });
    }
  }
};

// Make it globally available
window.drawerState = drawerState;

// Setup event listeners immediately if event system is already available
if (window.eventBus) {
  console.log('Event bus already available, setting up drawer state event listeners');
  setupEventListeners();
}

// Listen for events once the event system is initialized
window.addEventListener('drawer-event-system-ready', () => {
  console.log('Drawer state: Event system ready event received');
  console.log('DRAWER-STATE DEBUG: window.eventBus =', !!window.eventBus);
  console.log('DRAWER-STATE DEBUG: window.canvas =', !!window.canvas);
  if (window.eventBus) {
    setupEventListeners();
  } else {
    console.error('Event bus still not available after ready event');
  }
});

function setupEventListeners() {
  console.log('Setting up event listeners for drawer state');
  
  // Listen for shape creation events
  window.eventBus.subscribe('SHAPE_CREATED', (event) => {
    const canvasId = window.drawerState.currentCanvasId;
    if (!canvasId) return;
    
    // Initialize canvas storage if needed
    if (!window.drawerState.canvasShapes[canvasId]) {
      window.drawerState.canvasShapes[canvasId] = {};
    }
    
    // Only handle this if it's not already in the state (to avoid duplicates during replay)
    if (!window.drawerState.canvasShapes[canvasId][event.shapeId]) {
      console.log(`State manager: Shape created ${event.shapeId} for canvas ${canvasId}`);
      // Update canvas-specific state
      window.drawerState.canvasShapes[canvasId][event.shapeId] = {
        id: event.shapeId,
        type: event.shapeType,
        ...event.data
      };
      
      // Update legacy shapes reference
      window.drawerState.shapes[event.shapeId] = window.drawerState.canvasShapes[canvasId][event.shapeId];
    }
  });
  
  // Listen for shape deletion events
  window.eventBus.subscribe('SHAPE_DELETED', (event) => {
    const canvasId = window.drawerState.currentCanvasId;
    if (!canvasId) return;
    
    console.log(`State manager: Shape deleted ${event.shapeId} from canvas ${canvasId}`);
    
    // Remove from canvas-specific state
    if (window.drawerState.canvasShapes[canvasId] && window.drawerState.canvasShapes[canvasId][event.shapeId]) {
      delete window.drawerState.canvasShapes[canvasId][event.shapeId];
    }
    
    // Remove from legacy shapes reference
    if (window.drawerState.shapes[event.shapeId]) {
      delete window.drawerState.shapes[event.shapeId];
    }
  });
  
  // Listen for shape modification events
  window.eventBus.subscribe('SHAPE_MODIFIED', (event) => {
    const canvasId = window.drawerState.currentCanvasId;
    if (!canvasId) return;
    
    console.log(`State manager: Shape modified ${event.shapeId}.${event.property} in canvas ${canvasId}`);
    
    // Update canvas-specific state
    if (window.drawerState.canvasShapes[canvasId] && window.drawerState.canvasShapes[canvasId][event.shapeId]) {
      window.drawerState.canvasShapes[canvasId][event.shapeId][event.property] = event.value;
    }
    
    // Update legacy shapes reference
    if (window.drawerState.shapes[event.shapeId]) {
      window.drawerState.shapes[event.shapeId][event.property] = event.value;
    }
  });
  
  // Listen for tool selection events
  window.eventBus.subscribe('TOOL_SELECTED', (event) => {
    console.log('State manager: Tool selected', event.toolIndex);
    window.drawerState.selectedToolIndex = event.toolIndex;
  });
  
  // Listen for reset state events (for time travel)
  window.eventBus.subscribe('RESET_STATE', (event) => {
    const canvasId = event.canvasId || window.drawerState.currentCanvasId;
    
    if (canvasId) {
      console.log(`State manager: Resetting state for canvas ${canvasId}`);
      // Reset specific canvas
      if (window.drawerState.canvasShapes[canvasId]) {
        window.drawerState.canvasShapes[canvasId] = {};
      }
      
      // Update legacy shapes reference if this is the current canvas
      if (canvasId === window.drawerState.currentCanvasId) {
        window.drawerState.shapes = {};
      }
    } else {
      console.log('State manager: Resetting all state');
      window.drawerState.canvasShapes = {};
      window.drawerState.shapes = {};
    }
    // Don't reset tool selection as that's UI state
  });
}

} // End of drawerState already loaded check