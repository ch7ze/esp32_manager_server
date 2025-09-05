// Color State Management for Canvas Shapes
// Handles global color settings and application to shapes

// Check if already loaded to prevent duplicate declarations
if (typeof window.ColorState !== 'undefined') {
    console.log('ColorState already loaded, skipping redefinition');
} else {

class ColorState {
    constructor() {
        this.currentFillColor = 'transparent';
        this.currentStrokeColor = '#000000';
        this.defaultFillColor = 'transparent';
        this.defaultStrokeColor = '#000000';
        
        console.log('ColorState initialized');
        this.initializeColorControls();
    }
    
    // Initialize color controls if they exist in the DOM
    initializeColorControls() {
        // Wait a bit for DOM to be ready
        setTimeout(() => {
            this.setupColorControls();
        }, 100);
    }
    
    // Setup color control event listeners
    setupColorControls() {
        // Look for fill color control
        const fillColorControl = document.getElementById('fill-color') || 
                                document.querySelector('[data-color-type="fill"]') ||
                                document.querySelector('.fill-color-control');
        
        if (fillColorControl) {
            fillColorControl.addEventListener('change', (e) => {
                this.setFillColor(e.target.value);
            });
            
            // Set initial value if available
            if (fillColorControl.value) {
                this.currentFillColor = fillColorControl.value;
            }
        }
        
        // Look for stroke color control
        const strokeColorControl = document.getElementById('stroke-color') || 
                                  document.querySelector('[data-color-type="stroke"]') ||
                                  document.querySelector('.stroke-color-control');
        
        if (strokeColorControl) {
            strokeColorControl.addEventListener('change', (e) => {
                this.setStrokeColor(e.target.value);
            });
            
            // Set initial value if available
            if (strokeColorControl.value) {
                this.currentStrokeColor = strokeColorControl.value;
            }
        }
        
        // Look for transparency toggle
        const transparentToggle = document.getElementById('transparent-fill') ||
                                 document.querySelector('[data-transparent-toggle]') ||
                                 document.querySelector('.transparent-toggle');
        
        if (transparentToggle) {
            transparentToggle.addEventListener('change', (e) => {
                if (e.target.checked) {
                    this.setFillColor('transparent');
                } else {
                    this.setFillColor(this.defaultFillColor);
                }
            });
        }
        
        console.log('Color controls setup complete');
    }
    
    // Set current fill color
    setFillColor(color) {
        this.currentFillColor = color;
        console.log('Fill color set to:', color);
        
        // Update UI controls if they exist
        this.updateColorControls();
        
        // Emit color change event
        if (window.eventBus) {
            window.eventBus.publish({
                type: 'COLOR_CHANGED',
                colorType: 'fill',
                value: color,
                timestamp: Date.now()
            });
        }
    }
    
    // Set current stroke color
    setStrokeColor(color) {
        this.currentStrokeColor = color;
        console.log('Stroke color set to:', color);
        
        // Update UI controls if they exist
        this.updateColorControls();
        
        // Emit color change event
        if (window.eventBus) {
            window.eventBus.publish({
                type: 'COLOR_CHANGED',
                colorType: 'stroke',
                value: color,
                timestamp: Date.now()
            });
        }
    }
    
    // Get current fill color
    getFillColor() {
        return this.currentFillColor;
    }
    
    // Get current stroke color
    getStrokeColor() {
        return this.currentStrokeColor;
    }
    
    // Apply current colors to a shape
    applyColorsToShape(shape) {
        if (!shape) {
            console.warn('ColorState: Cannot apply colors to null/undefined shape');
            return;
        }
        
        try {
            // Apply fill color
            if (shape.setFillColor && typeof shape.setFillColor === 'function') {
                shape.setFillColor(this.currentFillColor);
                // Only log for non-temporary shapes to reduce console spam
                if (!shape.id || shape.id < 10000) {
                    console.log(`Applied fill color ${this.currentFillColor} to shape ${shape.id || 'unknown'}`);
                }
            }
            
            // Apply stroke color
            if (shape.setStrokeColor && typeof shape.setStrokeColor === 'function') {
                shape.setStrokeColor(this.currentStrokeColor);
                // Only log for non-temporary shapes to reduce console spam
                if (!shape.id || shape.id < 10000) {
                    console.log(`Applied stroke color ${this.currentStrokeColor} to shape ${shape.id || 'unknown'}`);
                }
            }
            
        } catch (error) {
            console.error('Error applying colors to shape:', error);
        }
    }
    
    // Apply colors to multiple shapes
    applyColorsToShapes(shapes) {
        if (!shapes || !Array.isArray(shapes)) {
            console.warn('ColorState: Cannot apply colors to invalid shapes array');
            return;
        }
        
        shapes.forEach(shape => this.applyColorsToShape(shape));
    }
    
    // Update UI color controls to reflect current state
    updateColorControls() {
        // Update fill color control
        const fillColorControl = document.getElementById('fill-color') || 
                                document.querySelector('[data-color-type="fill"]') ||
                                document.querySelector('.fill-color-control');
        
        if (fillColorControl && fillColorControl.value !== this.currentFillColor) {
            fillColorControl.value = this.currentFillColor === 'transparent' ? '' : this.currentFillColor;
        }
        
        // Update stroke color control
        const strokeColorControl = document.getElementById('stroke-color') || 
                                  document.querySelector('[data-color-type="stroke"]') ||
                                  document.querySelector('.stroke-color-control');
        
        if (strokeColorControl && strokeColorControl.value !== this.currentStrokeColor) {
            strokeColorControl.value = this.currentStrokeColor;
        }
        
        // Update transparency toggle
        const transparentToggle = document.getElementById('transparent-fill') ||
                                 document.querySelector('[data-transparent-toggle]') ||
                                 document.querySelector('.transparent-toggle');
        
        if (transparentToggle) {
            transparentToggle.checked = this.currentFillColor === 'transparent';
        }
    }
    
    // Reset colors to defaults
    resetColors() {
        this.currentFillColor = this.defaultFillColor;
        this.currentStrokeColor = this.defaultStrokeColor;
        this.updateColorControls();
        
        console.log('Colors reset to defaults');
    }
    
    // Set default colors
    setDefaults(fillColor, strokeColor) {
        this.defaultFillColor = fillColor || 'transparent';
        this.defaultStrokeColor = strokeColor || '#000000';
        
        console.log('Default colors set:', this.defaultFillColor, this.defaultStrokeColor);
    }
    
    // Get current color state as object
    getColorState() {
        return {
            fillColor: this.currentFillColor,
            strokeColor: this.currentStrokeColor,
            defaultFillColor: this.defaultFillColor,
            defaultStrokeColor: this.defaultStrokeColor
        };
    }
    
    // Set color state from object
    setColorState(state) {
        if (state.fillColor !== undefined) {
            this.setFillColor(state.fillColor);
        }
        if (state.strokeColor !== undefined) {
            this.setStrokeColor(state.strokeColor);
        }
        if (state.defaultFillColor !== undefined) {
            this.defaultFillColor = state.defaultFillColor;
        }
        if (state.defaultStrokeColor !== undefined) {
            this.defaultStrokeColor = state.defaultStrokeColor;
        }
    }
    
    // Create color picker UI (if needed)
    createColorPicker(container, options = {}) {
        if (!container) return;
        
        const fillLabel = document.createElement('label');
        fillLabel.textContent = options.fillLabel || 'Füllfarbe: ';
        
        const fillInput = document.createElement('input');
        fillInput.type = 'color';
        fillInput.id = 'fill-color';
        fillInput.value = this.currentFillColor === 'transparent' ? '#ffffff' : this.currentFillColor;
        fillInput.addEventListener('change', (e) => this.setFillColor(e.target.value));
        
        const transparentCheckbox = document.createElement('input');
        transparentCheckbox.type = 'checkbox';
        transparentCheckbox.id = 'transparent-fill';
        transparentCheckbox.checked = this.currentFillColor === 'transparent';
        transparentCheckbox.addEventListener('change', (e) => {
            this.setFillColor(e.target.checked ? 'transparent' : fillInput.value);
        });
        
        const transparentLabel = document.createElement('label');
        transparentLabel.textContent = options.transparentLabel || ' Transparent';
        transparentLabel.htmlFor = 'transparent-fill';
        
        const strokeLabel = document.createElement('label');
        strokeLabel.textContent = options.strokeLabel || 'Randfarbe: ';
        
        const strokeInput = document.createElement('input');
        strokeInput.type = 'color';
        strokeInput.id = 'stroke-color';
        strokeInput.value = this.currentStrokeColor;
        strokeInput.addEventListener('change', (e) => this.setStrokeColor(e.target.value));
        
        // Append elements
        container.appendChild(fillLabel);
        container.appendChild(fillInput);
        container.appendChild(transparentCheckbox);
        container.appendChild(transparentLabel);
        container.appendChild(document.createElement('br'));
        container.appendChild(strokeLabel);
        container.appendChild(strokeInput);
        
        console.log('Color picker UI created');
    }
}

// Initialize color state when script loads
console.log('Creating ColorState instance...');
window.colorState = new ColorState();

// Make methods globally available for easier access
window.setFillColor = (color) => {
    if (window.colorState) {
        window.colorState.setFillColor(color);
    }
};

window.setStrokeColor = (color) => {
    if (window.colorState) {
        window.colorState.setStrokeColor(color);
    }
};

window.applyColorsToShape = (shape) => {
    if (window.colorState) {
        window.colorState.applyColorsToShape(shape);
    }
};

// Dispatch ready event
console.log('ColorState ready, dispatching ready event');
window.dispatchEvent(new Event('color-state-ready'));

} // End of ColorState already loaded check