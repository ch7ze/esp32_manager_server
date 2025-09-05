// Check if already loaded to prevent duplicate declarations
if (typeof window._drawerPageLoaded !== 'undefined') {
    console.log('drawer_page.js already loaded, skipping redefinition');
} else {
    window._drawerPageLoaded = true;

// Load menu-api
function loadMenuApi() {
    return new Promise((resolve) => {
        // Only load script once using import
        import('/scripts/menu-api.js').then(module => {
            window.menuApi = module.default;
            console.log('Menu API loaded and available as window.menuApi');
            resolve();
        }).catch(err => {
            console.error('Error loading menu API:', err);
            resolve(); // Resolve anyway to continue initialization
        });
    });
}

// Load the drawer system ES6 modules and dependencies
async function loadDrawerSystem() {
    try {
        console.log("Lade Drawer System Module...");
        
        // Drawer State should be loaded via SPA architecture (app.js)
        // Just verify it's available
        console.log("🔍 DRAWER-PAGE DEBUG: Checking drawer state availability...");
        console.log("🔍 DRAWER-PAGE DEBUG: window.drawerState =", !!window.drawerState);
        if (window.drawerState) {
            console.log("✓ Drawer State bereits verfügbar (via SPA)");
        } else {
            console.warn("⚠️ Drawer State nicht verfügbar - wird über SPA geladen");
        }
        
        // Event System should be loaded via SPA architecture (app.js)
        // Just verify it's available
        console.log("🔍 DRAWER-PAGE DEBUG: Checking event system availability...");
        console.log("🔍 DRAWER-PAGE DEBUG: window.eventBus =", !!window.eventBus);
        console.log("🔍 DRAWER-PAGE DEBUG: window.eventStore =", !!window.eventStore);
        if (window.eventBus && window.eventStore) {
            console.log("✓ Event System bereits verfügbar (via SPA)");
        } else {
            console.warn("⚠️ Event System nicht verfügbar - wird über SPA geladen");
        }
        
        // Import the main drawer module which provides window.init
        const drawerModule = await import('/scripts/drawer/index.js');
        console.log("✓ Drawer System Module geladen");
        
        // Give the module time to set up window.init
        await new Promise(resolve => setTimeout(resolve, 50));
        
        return drawerModule;
    } catch (error) {
        console.error("Fehler beim Laden des Drawer Systems:", error);
        throw new Error(`Drawer System konnte nicht geladen werden: ${error.message}`);
    }
}

// Functions for adding UI elements
function addResetButton() {
    return new Promise((resolve) => {
        const infoSection = document.getElementById('info-section');
        if (infoSection) {
            const resetButton = document.createElement('button');
            resetButton.textContent = 'Zeichnung zurücksetzen';
            resetButton.style.marginTop = '20px';
            resetButton.style.padding = '8px 16px';
            resetButton.addEventListener('click', function() {
                // Ask for confirmation before resetting
                if (confirm('Möchten Sie wirklich die gesamte Zeichnung zurücksetzen? Diese Aktion kann nicht rückgängig gemacht werden.')) {
                    try {
                        console.log("Versuche Zeichnung zurückzusetzen...");
                        
                        // Clear shapes directly if canvas is available
                        if (window.canvas && window.canvas.shapes) {
                            // Get a copy of shape IDs to avoid modification during iteration
                            const shapeIds = Object.keys(window.canvas.shapes).map(id => parseInt(id));
                            console.log(`Entferne ${shapeIds.length} Shapes direkt aus dem Canvas...`);
                            
                            // Remove each shape
                            shapeIds.forEach(id => {
                                window.canvas.removeShapeWithId(id, false);
                            });
                            
                            // Redraw the now-empty canvas
                            window.canvas.draw();
                        }
                        
                        // Reset the drawer state
                        if (window.drawerState) {
                            console.log("Setze drawerState zurück...");
                            window.drawerState.reset();
                        }
                    } catch (e) {
                        console.error("Fehler beim Zurücksetzen:", e);
                        alert('Fehler beim Zurücksetzen der Zeichnung: ' + e.message);
                    }
                }
            });
            infoSection.appendChild(resetButton);
        }
        resolve();
    });
}

function addUtilityFunctions() {
    return new Promise((resolve) => {
        const mousePosition = document.getElementById('mouse-position');
        const drawArea = document.getElementById("drawArea");
        if (mousePosition && drawArea) {
            drawArea.addEventListener('mousemove', function(event) {
                const rect = drawArea.getBoundingClientRect();
                const x = Math.floor(event.clientX - rect.left);
                const y = Math.floor(event.clientY - rect.top);
                mousePosition.textContent = `${x}, ${y}`;
            });
            
            drawArea.addEventListener('mouseleave', function() {
                mousePosition.textContent = '---,---';
            });
        }
        
        resolve();
    });
}

function updateDebugInfo() {
    const shapesCountEl = document.getElementById('saved-shapes-count');
    
    if (shapesCountEl && window.drawerState && window.drawerState.shapes) {
        const count = Object.keys(window.drawerState.shapes).length;
        shapesCountEl.textContent = count;
        
        shapesCountEl.style.color = count > 0 ? '#007700' : '#770000';
    }
}


// Debug template detection and log DOM state
function debugTemplateDetection() {
    console.log("🔍 Template Detection Debug:");
    console.log(`📄 Current URL: ${window.location.pathname}`);
    console.log(`📄 Document title: ${document.title}`);
    console.log(`📄 Page content: ${document.querySelector('.page-content') ? '✅' : '❌'}`);
    
    // Check for template-specific elements
    const templateElements = [
        { name: 'drawArea', selector: '#drawArea', required: true },
        { name: 'tools', selector: '.tools', required: true },
        { name: 'canvas-users-display', selector: '#canvas-users-display', required: false },
        { name: 'info-section', selector: '#info-section', required: true },
        { name: 'shape-context-menu', selector: '#shape-context-menu', required: false }
    ];
    
    let existingElements = 0;
    let missingElements = [];
    
    templateElements.forEach(element => {
        const exists = document.querySelector(element.selector) !== null;
        if (exists) {
            existingElements++;
            console.log(`📋 ${element.name}: ✅`);
        } else {
            console.log(`📋 ${element.name}: ❌ ${element.required ? '(REQUIRED)' : '(optional)'}`);
            if (element.required) {
                missingElements.push(element.name);
            }
        }
    });
    
    console.log(`📊 Template completeness: ${existingElements}/${templateElements.length} elements found`);
    if (missingElements.length > 0) {
        console.warn(`⚠️ Missing required elements: ${missingElements.join(', ')}`);
    }
    
    // Detect likely template type
    const hasUsersDisplay = document.querySelector('#canvas-users-display') !== null;
    const likelyTemplate = hasUsersDisplay ? 'drawer_page.html' : 'canvas_detail.html';
    console.log(`🎯 Likely template: ${likelyTemplate}`);
    
    return { existingElements, missingElements, likelyTemplate };
}

// Ensure critical DOM elements exist, create them if missing
function ensureCriticalDOMElements() {
    console.log("🔧 Ensuring critical DOM elements exist...");
    
    // First, debug current template state
    const templateInfo = debugTemplateDetection();
    
    // Check and create drawArea canvas
    let drawArea = document.getElementById("drawArea");
    if (!drawArea) {
        console.warn("⚠️ drawArea not found, creating it dynamically");
        drawArea = document.createElement("canvas");
        drawArea.id = "drawArea";
        drawArea.width = 1024;
        drawArea.height = 768;
        drawArea.style.border = "1px solid #000000";
        drawArea.style.backgroundColor = "lightgrey";
        
        // Find insertion point (after tools, before info-section)
        const pageContent = document.querySelector('.page-content');
        const infoSection = document.getElementById('info-section');
        if (pageContent && infoSection) {
            pageContent.insertBefore(drawArea, infoSection);
        } else if (pageContent) {
            pageContent.appendChild(drawArea);
        } else {
            document.body.appendChild(drawArea);
        }
        console.log("✅ Created drawArea canvas");
    }
    
    // Check and create tools list
    let toolsArea = document.querySelector(".tools");
    if (!toolsArea) {
        console.warn("⚠️ .tools not found, creating it dynamically");
        toolsArea = document.createElement("ul");
        toolsArea.className = "tools";
        
        // Insert before drawArea
        if (drawArea.parentNode) {
            drawArea.parentNode.insertBefore(toolsArea, drawArea);
        } else {
            document.body.appendChild(toolsArea);
        }
        console.log("✅ Created .tools list");
    }
    
    // Check and create canvas users display
    let usersDisplay = document.getElementById("canvas-users-display");
    if (!usersDisplay) {
        console.warn("⚠️ canvas-users-display not found, creating it dynamically");
        usersDisplay = createUsersDisplayElement();
        console.log("✅ Created canvas-users-display");
    }
    
    // Check and create info section
    let infoSection = document.getElementById("info-section");
    if (!infoSection) {
        console.warn("⚠️ info-section not found, creating it dynamically");
        infoSection = document.createElement("div");
        infoSection.id = "info-section";
        infoSection.innerHTML = `
            <div id="coordinates">Mausposition: <span id="mouse-position">---, ---</span></div>
            <div id="debug-info" style="margin-top: 10px; font-size: 12px; color: #666;">
                <div>Gespeicherte Shapes: <span id="saved-shapes-count">---</span></div>
            </div>
        `;
        
        // Insert after drawArea
        if (drawArea.parentNode) {
            drawArea.parentNode.insertBefore(infoSection, drawArea.nextSibling);
        } else {
            document.body.appendChild(infoSection);
        }
        console.log("✅ Created info-section");
    }
    
    // Check and create shape context menu
    let contextMenu = document.getElementById("shape-context-menu");
    if (!contextMenu) {
        console.warn("⚠️ shape-context-menu not found, creating it dynamically");
        contextMenu = document.createElement("div");
        contextMenu.id = "shape-context-menu";
        contextMenu.className = "context-menu";
        contextMenu.style.display = "none";
        contextMenu.style.position = "absolute";
        document.body.appendChild(contextMenu);
        console.log("✅ Created shape-context-menu");
    }
    
    console.log("🔧 Critical DOM elements verification complete");
    return { drawArea, toolsArea, usersDisplay, infoSection, contextMenu };
}

// Store the main initialization function to call after script loading
function initializeCanvasPage() {
    console.log("🚀 Drawer-Seite wird initialisiert...");
    
    // Use Promise chain for initialization
    loadMenuApi()
        .then(async () => {
            console.log("✅ Menu API loaded, checking DOM elements...");
            
            // Ensure all critical DOM elements exist
            const elements = ensureCriticalDOMElements();
            
            console.log("✅ DOM elements verified, loading drawer system...");
            
            // Load the modular drawer system first
            await loadDrawerSystem();
            
            // Give DOM elements time to settle
            console.log("🔄 Allowing DOM to settle before drawer initialization...");
            await new Promise(resolve => setTimeout(resolve, 50));
            
            // Re-verify critical DOM elements exist before init
            console.log("🔍 Final DOM verification before drawer init...");
            const finalDrawArea = document.getElementById("drawArea");
            const finalToolsArea = document.querySelector(".tools");
            
            if (!finalDrawArea) {
                throw new Error("drawArea disappeared before init!");
            }
            if (!finalToolsArea) {
                throw new Error("tools area disappeared before init!");
            }
            
            console.log("✅ Final DOM check passed, proceeding with drawer initialization");
            
            // Initialize drawer with DOM-aware retry mechanism
            let initSuccess = false;
            for (let attempt = 1; attempt <= 3; attempt++) {
                if (typeof window.init === 'function') {
                    console.log(`🎯 Init-Funktion gefunden (Versuch ${attempt}), rufe auf...`);
                    try {
                        // Call init with pre-validated DOM state and capture return value for diagnostics
                        console.log("🔧 Calling window.init() with verified DOM state");
                        const initResult = window.init();
                        console.log("✅ Drawer erfolgreich initialisiert", initResult ? `mit Rückgabe: ${typeof initResult}` : '');
                        
                        // Verify initialization was successful by checking created elements
                        const toolElements = finalToolsArea.querySelectorAll('li');
                        console.log(`🔍 Post-init verification: ${toolElements.length} tools created`);
                        
                        if (toolElements.length === 0) {
                            throw new Error("Tools were not created successfully - no li elements found in .tools");
                        }
                        initSuccess = true;
                        break;
                    } catch (e) {
                        console.error(`❌ Fehler bei Initialisierung (Versuch ${attempt}):`, e);
                        console.error(`❌ Error details:`, {
                            name: e.name,
                            message: e.message,
                            stack: e.stack
                        });
                        
                        // Re-check DOM elements are still there
                        const stillHaveDrawArea = document.getElementById("drawArea") !== null;
                        const stillHaveTools = document.querySelector(".tools") !== null;
                        console.error(`❌ DOM state after error: drawArea=${stillHaveDrawArea}, tools=${stillHaveTools}`);
                        
                        if (attempt === 3) {
                            throw e;
                        }
                        // Wait before retry
                        await new Promise(resolve => setTimeout(resolve, 100 * attempt));
                    }
                } else {
                    console.warn(`⏳ Init-Funktion noch nicht verfügbar (Versuch ${attempt}), warte...`);
                    console.warn(`⏳ window.init type: ${typeof window.init}`);
                    if (attempt === 3) {
                        throw new Error("Init-Funktion ist nach 3 Versuchen nicht verfügbar!");
                    }
                    // Wait before retry
                    await new Promise(resolve => setTimeout(resolve, 100 * attempt));
                }
            }
            
            // Correct counter if needed
            if (window.AbstractShape && window.drawerState) {
                const currentMax = window.drawerState.currentShapeId;
                const actualMax = Object.keys(window.canvas.shapes).length > 0 
                    ? Math.max(...Object.keys(window.canvas.shapes).map(id => parseInt(id))) 
                    : 0;
                    
                if (currentMax < actualMax) {
                    console.log(`Korrigiere Shape-ID-Zähler von ${currentMax} auf ${actualMax + 1}`);
                    window.drawerState.currentShapeId = actualMax + 1;
                }
            }
            
            // Create reset button and other UI elements
            addResetButton()
                .then(addUtilityFunctions)
                .then(checkCanvasPermissions)  // Check user permissions for this canvas
                .then(() => {
                    // CRITICAL FIX: Initialize ColorState after DOM is ready
                    if (window.colorState) {
                        console.log('🎨🟡 Re-initializing ColorState after DOM is ready...');
                        window.colorState.setupColorControls();
                    }
                    
                    // Initialize canvas users display with delay to ensure DOM is ready
                    console.log('👥 Initializing canvas users display...');
                    // Small delay to ensure DOM is fully ready and CSS is loaded
                    setTimeout(() => {
                        initializeCanvasUsersDisplay();
                    }, 500);
                    
                    // Register canvas for WebSocket if we're on a canvas detail page
                    registerCanvasForWebSocket();
                    
                    // Setup navigation cleanup handlers
                    setupNavigationCleanup();
                })
                .catch(error => {
                    console.error("Fehler beim Einrichten der UI-Elemente:", error);
                    // Even if something fails, still try to initialize users display
                    try {
                        console.log('👥 Fallback: Initializing canvas users display...');
                        setTimeout(() => {
                            initializeCanvasUsersDisplay();
                        }, 500);
                    } catch (usersError) {
                        console.error("👥 Failed to initialize users display:", usersError);
                    }
                });
        })
        .catch(error => {
            console.error("Fehler während der Initialisierung:", error);
            alert(`Initialisierungsfehler: ${error.message}`);
        });
}

// Check user permissions for current canvas and adjust UI
async function checkCanvasPermissions() {
    return new Promise(async (resolve) => {
        try {
            // Extract canvas ID from URL
            const currentPath = window.location.pathname;
            const canvasMatch = currentPath.match(/^\/canvas\/([^\/]+)$/);
            
            if (!canvasMatch) {
                console.log('⚠️ No canvas ID found in URL - skipping permission check');
                resolve();
                return;
            }
            
            const canvasId = canvasMatch[1];
            console.log('🔐 Checking permissions for canvas:', canvasId);
            
            // Fetch canvas details including user permission
            const response = await fetch(`/api/canvas/${canvasId}`, {
                method: 'GET',
                credentials: 'include'
            });
            
            if (response.ok) {
                const data = await response.json();
                if (data.success && data.canvas) {
                    const userPermission = data.canvas.your_permission;
                    const canvasName = data.canvas.name;
                    const isModerated = data.canvas.is_moderated;
                    
                    console.log(`🔐 User permission for canvas "${canvasName}": ${userPermission}, moderated: ${isModerated}`);
                    
                    // Store permission and moderation status globally for other components to use
                    window.currentCanvasPermission = userPermission;
                    window.currentCanvasModerated = isModerated;
                    
                    // Apply UI restrictions based on permission and moderation status
                    applyPermissionRestrictions(userPermission, canvasName, isModerated);
                } else {
                    console.error('❌ Failed to get canvas permission data');
                }
            } else {
                console.error('❌ Failed to fetch canvas permissions:', response.status);
            }
        } catch (error) {
            console.error('❌ Error checking canvas permissions:', error);
        }
        resolve();
    });
}

// Add Read-Only indicator to the page
function addReadOnlyIndicator(canvasName, reasonText) {
    const pageContent = document.querySelector('.page-content');
    if (pageContent && !document.getElementById('readonly-indicator')) {
        const indicator = document.createElement('div');
        indicator.id = 'readonly-indicator';
        indicator.innerHTML = `
            <div style="
                background-color: #fff3cd;
                border: 1px solid #ffeaa7;
                color: #856404;
                padding: 12px;
                margin-bottom: 20px;
                border-radius: 4px;
                font-weight: bold;
            ">
                🔒 ${reasonText || `Read-Only Modus: Sie können "${canvasName}" nur anzeigen, aber nicht bearbeiten.`}
            </div>
        `;
        
        // Insert after the h1 title
        const title = pageContent.querySelector('h1');
        if (title && title.nextSibling) {
            pageContent.insertBefore(indicator, title.nextSibling);
        } else if (title) {
            title.parentNode.insertBefore(indicator, title.nextSibling);
        }
    }
}

// Robust tool disabling with MutationObserver for dynamic content
function disableDrawingTools() {
    console.log('🔒 Setting up robust drawing tool disabling...');
    
    // Inject CSS rules to ensure styling persists
    injectReadOnlyCSS();
    
    // Set up MutationObserver to watch for dynamically added tools
    setupToolObserver();
    
    // Immediate attempt to disable any existing tools
    applyToolDisabling();
    
    // Periodic re-application as fallback
    const disableInterval = setInterval(() => {
        applyToolDisabling();
    }, 500);
    
    // Stop periodic checks after 10 seconds
    setTimeout(() => {
        clearInterval(disableInterval);
        console.log('🔒 Stopped periodic tool disabling checks');
    }, 10000);
}

// Inject CSS rules with !important to ensure they persist
function injectReadOnlyCSS() {
    if (!document.getElementById('readonly-styles')) {
        const style = document.createElement('style');
        style.id = 'readonly-styles';
        style.textContent = `
            .tools.readonly-disabled {
                opacity: 0.3 !important;
                pointer-events: none !important;
                filter: grayscale(100%) !important;
                position: relative !important;
            }
            
            .tools.readonly-disabled li {
                background-color: #e9ecef !important;
                color: #6c757d !important;
                cursor: not-allowed !important;
                border: 1px solid #dee2e6 !important;
            }
            
            .tools.readonly-disabled li.selected,
            .tools.readonly-disabled li:hover {
                background-color: #e9ecef !important;
                color: #6c757d !important;
            }
            
            .readonly-overlay {
                position: absolute !important;
                top: 0 !important;
                left: 0 !important;
                right: 0 !important;
                bottom: 0 !important;
                background: rgba(255, 255, 255, 0.7) !important;
                pointer-events: none !important;
                z-index: 10 !important;
            }
            
            #drawArea.readonly-mode {
                cursor: not-allowed !important;
                filter: contrast(0.9) brightness(0.95) !important;
                border: 2px solid #6c757d !important;
            }
            
            button.readonly-disabled {
                opacity: 0.3 !important;
                cursor: not-allowed !important;
                filter: grayscale(100%) !important;
                pointer-events: none !important;
            }
        `;
        document.head.appendChild(style);
        console.log('🔒 Injected Read-Only CSS rules');
    }
}

// Set up MutationObserver to watch for dynamic tool additions
function setupToolObserver() {
    const targetNode = document.querySelector('.tools') || document.body;
    
    const observer = new MutationObserver((mutations) => {
        mutations.forEach((mutation) => {
            if (mutation.type === 'childList' && mutation.addedNodes.length > 0) {
                console.log('🔒 Tools DOM changed, re-applying disabling...');
                applyToolDisabling();
            }
        });
    });
    
    observer.observe(targetNode, { 
        childList: true, 
        subtree: true 
    });
    
    console.log('🔒 Tool observer set up');
    return observer;
}

// Apply tool disabling (called repeatedly)
function applyToolDisabling() {
    const toolsList = document.querySelector('.tools');
    if (toolsList) {
        // Add readonly class
        toolsList.classList.add('readonly-disabled');
        
        // Disable all tool items
        const toolItems = toolsList.querySelectorAll('li');
        toolItems.forEach(item => {
            item.classList.add('disabled-tool');
            item.classList.remove('selected', 'active');
        });
        
        // Add overlay if not exists
        if (!toolsList.querySelector('.readonly-overlay')) {
            const overlay = document.createElement('div');
            overlay.className = 'readonly-overlay';
            toolsList.appendChild(overlay);
        }
        
        console.log(`🔒 Disabled ${toolItems.length} tools`);
    }
    
    // Disable color controls and other elements
    const controlSelectors = [
        '.color-control', '.color-picker', '#color-section',
        'button[onclick*="color"]', 'input[type="color"]'
    ];
    
    controlSelectors.forEach(selector => {
        document.querySelectorAll(selector).forEach(element => {
            element.classList.add('readonly-disabled');
        });
    });
    
    // Disable reset and other control buttons
    document.querySelectorAll('button').forEach(button => {
        if (button.textContent.includes('zurücksetzen') || 
            button.textContent.includes('reset') ||
            button.closest('.tools') ||
            button.closest('#info-section')) {
            button.classList.add('readonly-disabled');
            button.disabled = true;
        }
    });
}

// Add aggressive event interception to prevent all interactions
function addDrawingPreventionListeners() {
    console.log('🔒 Setting up aggressive event interception...');
    
    const canvas = document.getElementById('drawArea');
    if (canvas) {
        setupCanvasEventBlocking(canvas);
    }
    
    // Block tool interactions
    setupToolEventBlocking();
    
    // Block keyboard shortcuts
    setupKeyboardBlocking();
    
    // Block any dynamically added event listeners
    setupEventListenerInterception();
}

// Set up comprehensive canvas event blocking
function setupCanvasEventBlocking(canvas) {
    const aggressivePreventInteraction = (e) => {
        // Immediately stop all propagation
        e.stopImmediatePropagation();
        e.preventDefault();
        
        console.log(`🔒 BLOCKED ${e.type} event - Read-Only mode`);
        
        // Show user feedback for primary interaction events
        const feedbackEvents = ['mousedown', 'click', 'touchstart', 'contextmenu', 'dblclick'];
        if (feedbackEvents.includes(e.type)) {
            showReadOnlyFeedback(e);
        }
        
        return false;
    };
    
    // Block all possible canvas interaction events with highest priority
    const eventTypes = [
        'mousedown', 'mouseup', 'mousemove', 'click', 'dblclick',
        'touchstart', 'touchmove', 'touchend', 'touchcancel',
        'pointerdown', 'pointerup', 'pointermove', 'pointercancel',
        'contextmenu', 'selectstart', 'dragstart', 'drag', 'dragend',
        'wheel', 'keydown', 'keyup', 'keypress'
    ];
    
    eventTypes.forEach(eventType => {
        // Use capture phase with highest priority
        canvas.addEventListener(eventType, aggressivePreventInteraction, {
            capture: true,
            passive: false
        });
    });
    
    console.log(`🔒 Blocked ${eventTypes.length} event types on canvas`);
}

// Block tool selection and interaction
function setupToolEventBlocking() {
    // Use event delegation to catch dynamically added tools
    document.addEventListener('click', (e) => {
        if (e.target.closest('.tools') || e.target.classList.contains('tool')) {
            e.stopImmediatePropagation();
            e.preventDefault();
            console.log('🔒 Tool click blocked - Read-Only mode');
            showReadOnlyFeedback(e);
            return false;
        }
    }, { capture: true, passive: false });
    
    // Block any mouse events on tool area
    const toolEvents = ['mousedown', 'mouseup', 'mousemove', 'dblclick', 'contextmenu'];
    toolEvents.forEach(eventType => {
        document.addEventListener(eventType, (e) => {
            if (e.target.closest('.tools')) {
                e.stopImmediatePropagation();
                e.preventDefault();
                return false;
            }
        }, { capture: true, passive: false });
    });
}

// Block keyboard shortcuts globally
function setupKeyboardBlocking() {
    document.addEventListener('keydown', (e) => {
        // Block drawing-related keyboard shortcuts
        const isEditingShortcut = (e.ctrlKey || e.metaKey) && 
            ['c', 'v', 'x', 'z', 'y', 'a', 's'].includes(e.key.toLowerCase());
        
        const isDeleteKey = ['Delete', 'Backspace'].includes(e.key);
        
        const isArrowKey = ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.key);
        
        if (isEditingShortcut || isDeleteKey || isArrowKey) {
            e.stopImmediatePropagation();
            e.preventDefault();
            console.log(`🔒 Keyboard shortcut ${e.key} blocked - Read-Only mode`);
            return false;
        }
    }, { capture: true, passive: false });
}

// Intercept dynamically added event listeners (advanced technique)
function setupEventListenerInterception() {
    // Store original addEventListener
    const originalAddEventListener = EventTarget.prototype.addEventListener;
    
    // Override addEventListener for canvas and tools
    EventTarget.prototype.addEventListener = function(type, listener, options) {
        // Check if this is being added to canvas or tool elements
        if ((this.id === 'drawArea' || this.closest?.('.tools')) && 
            ['mousedown', 'mouseup', 'click', 'touchstart', 'touchend'].includes(type)) {
            
            console.log(`🔒 Intercepted ${type} event listener on`, this.tagName || this.constructor.name);
            
            // Replace the listener with our blocking version
            const blockingListener = (e) => {
                e.stopImmediatePropagation();
                e.preventDefault();
                console.log(`🔒 Intercepted ${type} blocked - Read-Only mode`);
                return false;
            };
            
            // Call original with our blocking listener
            return originalAddEventListener.call(this, type, blockingListener, options);
        }
        
        // For non-canvas/tool elements, use original
        return originalAddEventListener.call(this, type, listener, options);
    };
    
    console.log('🔒 Event listener interception set up');
}

// Disable canvas interactivity completely and hook into Drawer State
function disableCanvasInteractivity() {
    const canvas = document.getElementById('drawArea');
    if (canvas) {
        // Add read-only class for CSS styling
        canvas.classList.add('readonly-mode');
        
        console.log('🔒 Setting up Drawer State read-only mode...');
        
        // Hook into TypeScript Drawer State with retry mechanism
        setupDrawerStateReadOnlyMode();
    }
}

// Set up read-only mode in TypeScript Drawer State
function setupDrawerStateReadOnlyMode() {
    const attemptDrawerStateHook = () => {
        try {
            // Check if drawer state is available
            if (window.drawerState) {
                console.log('✅ Found drawerState, setting read-only mode');
                
                // Set read-only flag
                window.drawerState.readOnlyMode = true;
                
                // Clear any selected tool
                if (window.drawerState.currentTool) {
                    console.log('🔒 Clearing selected tool for Read-Only mode');
                    window.drawerState.currentTool = null;
                }
                
                // Disable tool selection functionality
                const originalSelectTool = window.drawerState.selectTool;
                if (originalSelectTool) {
                    window.drawerState.selectTool = function(tool) {
                        console.log('🔒 Tool selection blocked - Read-Only mode');
                        return false;
                    };
                }
                
                return true; // Success
            }
            
            // Check if canvas object is available for direct manipulation
            if (window.canvas) {
                console.log('✅ Found canvas object, disabling interactions');
                
                // Override canvas interaction methods
                const originalMethods = ['addShape', 'removeShape', 'selectShape'];
                originalMethods.forEach(methodName => {
                    if (window.canvas[methodName]) {
                        const original = window.canvas[methodName];
                        window.canvas[methodName] = function(...args) {
                            console.log(`🔒 Canvas method ${methodName} blocked - Read-Only mode`);
                            return false;
                        };
                    }
                });
                
                // Disable shape selection
                if (window.canvas.selectedShapes) {
                    window.canvas.selectedShapes = [];
                }
                
                return true; // Success
            }
            
            return false; // Not ready yet
            
        } catch (e) {
            console.warn('⚠️ Error setting up drawer state read-only mode:', e);
            return false;
        }
    };
    
    // Try immediately
    if (attemptDrawerStateHook()) {
        return;
    }
    
    // Retry with intervals if not available immediately
    let attempts = 0;
    const maxAttempts = 20;
    const retryInterval = setInterval(() => {
        attempts++;
        
        if (attemptDrawerStateHook()) {
            clearInterval(retryInterval);
            console.log('✅ Drawer State read-only mode set up successfully');
        } else if (attempts >= maxAttempts) {
            clearInterval(retryInterval);
            console.warn('⚠️ Could not set up Drawer State read-only mode after', maxAttempts, 'attempts');
        }
    }, 200);
}

// Show brief visual feedback when user tries to draw in read-only mode
function showReadOnlyFeedback(e) {
    const canvas = document.getElementById('drawArea');
    if (!canvas) return;
    
    // Create temporary feedback element
    const feedback = document.createElement('div');
    feedback.textContent = '🔒 Nur Lesezugriff';
    feedback.style.cssText = `
        position: absolute;
        left: ${e.clientX + 10}px;
        top: ${e.clientY - 10}px;
        background: rgba(220, 53, 69, 0.9);
        color: white;
        padding: 4px 8px;
        border-radius: 4px;
        font-size: 12px;
        font-weight: bold;
        pointer-events: none;
        z-index: 10000;
        animation: fadeOut 1.5s ease-out forwards;
    `;
    
    // Add fade out animation
    const style = document.createElement('style');
    style.textContent = `
        @keyframes fadeOut {
            0% { opacity: 1; transform: translateY(0); }
            100% { opacity: 0; transform: translateY(-20px); }
        }
    `;
    document.head.appendChild(style);
    
    document.body.appendChild(feedback);
    
    // Remove after animation
    setTimeout(() => {
        if (feedback.parentNode) {
            feedback.parentNode.removeChild(feedback);
        }
        if (style.parentNode) {
            style.parentNode.removeChild(style);
        }
    }, 1500);
}

// Apply UI restrictions based on user permission and moderation status
function applyPermissionRestrictions(permission, canvasName, isModerated) {
    // Check if user should be treated as read-only based on A5.4 specification:
    // - R: Always read-only
    // - W: Can draw only if canvas is NOT moderated
    // - V: Can draw even if canvas is moderated  
    // - M: Can always draw and moderate
    // - O: Can always draw and has all rights
    const isReadOnly = permission === 'R' || (permission === 'W' && isModerated);
    
    if (isReadOnly) {
        let reasonText;
        if (permission === 'R') {
            reasonText = 'Read-Only Modus: Sie können diese Zeichenfläche nur anzeigen, aber nicht bearbeiten.';
        } else if (permission === 'W' && isModerated) {
            reasonText = 'Moderierter Modus: Diese Zeichenfläche ist moderiert. Nur Voice-Benutzer, Moderatoren und Besitzer können zeichnen.';
        }
        
        console.log(`🔒 Applying Read-Only restrictions to UI (${permission} permission, moderated: ${isModerated})`);
        
        // Add read-only indicator to page immediately
        addReadOnlyIndicator(canvasName, reasonText);
        
        // CRITICAL: Set up event blocking IMMEDIATELY, before drawer system loads
        console.log('🔒 Setting up immediate event blocking...');
        addDrawingPreventionListeners();
        
        // Disable/hide drawing tools immediately and continuously
        disableDrawingTools();
        
        // Set up canvas interactivity blocking immediately
        disableCanvasInteractivity();
        
        // Additional safeguards - reapply after drawer system might have loaded
        setTimeout(() => {
            console.log('🔒 Reapplying read-only restrictions (safeguard)');
            applyToolDisabling();
            setupDrawerStateReadOnlyMode();
        }, 1000);
        
        setTimeout(() => {
            console.log('🔒 Final read-only restrictions check');
            applyToolDisabling();
        }, 3000);
    } else {
        console.log(`✅ User has write permission - full functionality available (${permission} permission, moderated: ${isModerated})`);
    }
}

// Function to register canvas for WebSocket communication - FIXED: Wait for real connection
async function registerCanvasForWebSocket() {
    // Extract canvas ID from URL if we're on a canvas detail page
    const currentPath = window.location.pathname;
    const canvasMatch = currentPath.match(/^\/canvas\/([^\/]+)$/);
    
    if (canvasMatch) {
        const canvasId = canvasMatch[1];
        console.log('🔌 Registering for canvas WebSocket events:', canvasId);
        
        // FIXED: Ensure WebSocket bridge is available and properly connected
        if (window.registerForCanvas && window.canvasWebSocketBridge) {
            try {
                // Initialize canvas-specific state management before registration
                initializeCanvasState(canvasId);
                
                // CRITICAL FIX: Wait for REAL WebSocket connection, not just bridge initialization
                console.log('🔌 Checking WebSocket connection status...');
                const bridge = window.canvasWebSocketBridge;
                const wsClient = bridge.webSocketClient;
                
                if (!wsClient || !wsClient.isConnected) {
                    console.log('🔌 WebSocket not connected yet, waiting...');
                    // Use existing event system - listen for the 'connected' event
                    await new Promise((resolve) => {
                        if (wsClient && wsClient.isConnected) {
                            resolve(); // Already connected
                            return;
                        }
                        
                        const onConnected = () => {
                            console.log('🔌 WebSocket connection established, proceeding with registration');
                            wsClient.off('connected', onConnected); // Clean up listener
                            resolve();
                        };
                        
                        if (wsClient) {
                            wsClient.on('connected', onConnected);
                        } else {
                            // Fallback: retry in 500ms if client not available yet
                            setTimeout(resolve, 500);
                        }
                    });
                }
                
                console.log('✅ WebSocket connection confirmed, registering canvas');
                window.registerForCanvas(canvasId);
                console.log('✅ Canvas registered for WebSocket events:', canvasId);
            } catch (error) {
                console.error('❌ Error registering canvas for WebSocket:', error);
            }
        } else {
            console.warn('⚠️ WebSocket bridge not available, retrying in 1s...');
            setTimeout(registerCanvasForWebSocket, 1000);
        }
    } else {
        console.warn('⚠️ No canvas ID found in URL - canvas-specific features may not work correctly');
    }
}

// Initialize canvas-specific state management
function initializeCanvasState(canvasId) {
    console.log(`🎯 Initializing canvas-specific state for: ${canvasId}`);
    
    // Check if we're returning to the same canvas (SPA navigation)
    const isReturningToSameCanvas = window._currentCanvasId === canvasId;
    
    if (isReturningToSameCanvas) {
        console.log(`🔄 Returning to same canvas ${canvasId}, performing soft reset`);
        performSoftCanvasReset(canvasId);
    } else {
        console.log(`🆕 Switching to new canvas ${canvasId}, performing full initialization`);
    }
    
    // Set current canvas in drawer state FIRST (before WebSocket registration)
    if (window.drawerState && window.drawerState.setCurrentCanvas) {
        window.drawerState.setCurrentCanvas(canvasId);
        console.log('✅ DrawerState canvas context set');
    } else {
        console.warn('⚠️ DrawerState not available for canvas context setting');
    }
    
    // Clean up any previous canvas state if switching (but preserve server data)
    cleanupPreviousCanvasState(canvasId);
    
    // Update current canvas tracking
    window._currentCanvasId = canvasId;
}

// Perform soft reset for returning to same canvas (SPA navigation)
function performSoftCanvasReset(canvasId) {
    console.log(`🔄 Performing soft canvas reset for: ${canvasId}`);
    
    // Clear any replay flags to ensure fresh event processing
    if (window._canvasReplaying) {
        delete window._canvasReplaying[canvasId];
        console.log('🔄 Cleared canvas replay flags');
    }
    
    if (window._isReplaying) {
        window._isReplaying = false;
        console.log('🔄 Cleared global replay flag');
    }
    
    // Canvas visual state will be cleared and redrawn automatically by the server replay
    
    console.log(`🔄 Soft canvas reset complete for: ${canvasId}`);
}

// Clean up state from previous canvas to prevent contamination
function cleanupPreviousCanvasState(newCanvasId) {
    // Only clean up replay flags - DON'T clear canvas shapes yet
    // Shapes will be cleared and reloaded when server events come in
    
    if (window._canvasReplaying) {
        // Clean up replay flags for other canvases only
        Object.keys(window._canvasReplaying).forEach(canvasId => {
            if (canvasId !== newCanvasId) {
                console.log(`🧹 Cleaning up replay flag for canvas: ${canvasId}`);
                delete window._canvasReplaying[canvasId];
            }
        });
    }
    
    // Clear any lingering global replay flag
    if (window._isReplaying) {
        console.log('🧹 Clearing global replay flag');
        window._isReplaying = false;
    }
    
    console.log(`🧹 Minimal state cleanup complete for canvas: ${newCanvasId} (preserving shapes for server reload)`);
}

// Setup navigation event handlers for proper cleanup
function setupNavigationCleanup() {
    console.log('🚪 Setting up navigation cleanup handlers');
    
    // Get current canvas ID if we're on a canvas page
    const currentPath = window.location.pathname;
    const canvasMatch = currentPath.match(/^\/canvas\/([^\/]+)$/);
    const currentCanvasId = canvasMatch ? canvasMatch[1] : null;
    
    if (!currentCanvasId) {
        console.log('🚪 Not on a canvas page, skipping navigation cleanup setup');
        return;
    }
    
    // Store canvas ID for cleanup handlers
    window._currentCanvasId = currentCanvasId;
    
    // Handle page unload/navigation away from canvas
    const handlePageUnload = (event) => {
        console.log('🚪 Page unloading, cleaning up canvas state');
        performCanvasCleanup(currentCanvasId);
    };
    
    // Handle SPA navigation (popstate for back/forward, beforeunload for page close)
    const handleSPANavigation = (event) => {
        console.log(`🚪 SPA Navigation Event fired! Current path: ${window.location.pathname}, Event type: ${event.type}`);
        // Fix: Wait for navigation to complete to get correct new path
        setTimeout(() => {
            const newPath = window.location.pathname;
            const newCanvasMatch = newPath.match(/^\/canvas\/([^\/]+)$/);
            const newCanvasId = newCanvasMatch ? newCanvasMatch[1] : null;
            
            console.log(`🚪 Checking navigation after timeout - Old: ${currentPath}, New: ${newPath}, CurrentCanvasId: ${currentCanvasId}, NewCanvasId: ${newCanvasId}`);
            
            // If navigating away from canvas or to different canvas
            if (!newCanvasId || newCanvasId !== currentCanvasId) {
                console.log(`🚪 SPA navigation detected: ${currentPath} → ${newPath}`);
                performCanvasCleanup(currentCanvasId);
            }
        }, 50); // 50ms delay to ensure navigation has completed
    };
    
    // Add event listeners
    window.addEventListener('beforeunload', handlePageUnload);
    window.addEventListener('pagehide', handlePageUnload);
    window.addEventListener('popstate', handleSPANavigation);
    
    // Store cleanup function for potential manual cleanup
    window._cleanupCanvasNavigation = () => {
        window.removeEventListener('beforeunload', handlePageUnload);
        window.removeEventListener('pagehide', handlePageUnload);
        window.removeEventListener('popstate', handleSPANavigation);
        console.log('🚪 Navigation cleanup handlers removed');
    };
    
    console.log(`🚪 Navigation cleanup handlers setup complete for canvas: ${currentCanvasId}`);
}

// Perform cleanup when navigating away from canvas
function performCanvasCleanup(canvasId) {
    console.log(`🧹 Performing canvas cleanup for: ${canvasId}`);
    
    // Stop users polling
    stopCanvasUsersPolling();
    
    // Unregister from WebSocket (this now includes shape deselection as first step)
    if (window.unregisterFromCanvas) {
        window.unregisterFromCanvas();
        console.log('🧹 WebSocket unregistered');
    }
    
    // Clean up canvas-specific flags and state
    if (window._canvasReplaying && window._canvasReplaying[canvasId]) {
        delete window._canvasReplaying[canvasId];
        console.log('🧹 Canvas replay flag cleaned');
    }
    
    // Force immediate save current canvas state before leaving (bypass debouncing)
    if (window.canvas && window.canvas.forceSaveState) {
        window.canvas.forceSaveState();
        console.log('🧹 Canvas state force-saved before navigation');
    } else if (window.canvas && window.canvas.saveState) {
        window.canvas.saveState();
        console.log('🧹 Canvas state saved before navigation');
    }
    
    console.log(`🧹 Canvas cleanup complete for: ${canvasId}`);
}

// ============================================================================
// CANVAS USERS DISPLAY - Show connected users at top of canvas
// ============================================================================

let _usersPollingInterval = null;
let _currentCanvasId = null;

// Create the users display element dynamically if it doesn't exist
function createUsersDisplayElement() {
    console.log('👥 Creating users display element dynamically...');
    
    // Find the page content container
    const pageContent = document.querySelector('.page-content');
    if (!pageContent) {
        console.error('👥 Page content container not found');
        return null;
    }
    
    // Find the h1 title
    const title = pageContent.querySelector('h1');
    if (!title) {
        console.error('👥 H1 title not found');
        return null;
    }
    
    // Create the users display HTML
    const usersDisplay = document.createElement('div');
    usersDisplay.id = 'canvas-users-display';
    usersDisplay.className = 'canvas-users';
    usersDisplay.innerHTML = `
        <div class="users-header">Aktive Benutzer:</div>
        <div id="users-list" class="users-list">
            <span class="users-loading">Lade Benutzerliste...</span>
        </div>
    `;
    
    // Insert after the title
    if (title.nextSibling) {
        pageContent.insertBefore(usersDisplay, title.nextSibling);
    } else {
        title.parentNode.insertBefore(usersDisplay, title.nextSibling);
    }
    
    console.log('👥 Users display element created and inserted into DOM');
    return usersDisplay;
}

// Initialize the canvas users display system
function initializeCanvasUsersDisplay() {
    try {
        console.log('👥 Starting canvas users display initialization...');
        
        const currentPath = window.location.pathname;
        console.log('👥 Current path:', currentPath);
        
        // Use helper function for consistent canvas ID detection
        const currentCanvasId = getCurrentCanvasId();
        
        if (!currentCanvasId) {
            console.log('👥 Not on a canvas page, hiding users display');
            hideCanvasUsersDisplay();
            return;
        }
        
        _currentCanvasId = currentCanvasId;
        console.log('👥 Initializing canvas users display for canvas:', _currentCanvasId);
        
        // Check if display element exists, create it if not
        let displayElement = document.getElementById('canvas-users-display');
        if (!displayElement) {
            console.warn('👥 Users display element not found, creating it dynamically...');
            displayElement = createUsersDisplayElement();
            if (!displayElement) {
                console.error('👥 Failed to create users display element');
                return;
            }
        }
        
        console.log('👥 Users display element found/created:', displayElement);
        
        // Show the users display
        showCanvasUsersDisplay();
        
        // Load users immediately, with retry mechanism for reload scenarios
        loadCanvasUsersWithRetry();
        
        // Start polling for updates every 10 seconds
        startCanvasUsersPolling();
        
        console.log('👥 Canvas users display initialization complete');
    } catch (error) {
        console.error('👥 Error initializing canvas users display:', error);
    }
}

// Show the canvas users display element
function showCanvasUsersDisplay() {
    const display = document.getElementById('canvas-users-display');
    if (display) {
        console.log('👥 Showing canvas users display');
        display.style.display = 'block';
    } else {
        console.error('👥 Cannot show users display - element not found');
    }
}

// Hide the canvas users display element
function hideCanvasUsersDisplay() {
    const display = document.getElementById('canvas-users-display');
    if (display) {
        display.style.display = 'none';
    }
}

// Load users with retry mechanism (helpful for reload scenarios)
async function loadCanvasUsersWithRetry(maxRetries = 3, delay = 1000) {
    for (let attempt = 1; attempt <= maxRetries; attempt++) {
        try {
            console.log(`👥 Loading users attempt ${attempt}/${maxRetries}`);
            await loadCanvasUsers();
            return; // Success, exit retry loop
        } catch (error) {
            console.warn(`👥 Attempt ${attempt} failed:`, error);
            if (attempt < maxRetries) {
                console.log(`👥 Retrying in ${delay}ms...`);
                await new Promise(resolve => setTimeout(resolve, delay));
            } else {
                console.error('👥 All retry attempts failed');
                displayUsersError('Laden fehlgeschlagen');
            }
        }
    }
}

// Load and display the list of users currently connected to the canvas
async function loadCanvasUsers() {
    if (!_currentCanvasId) {
        console.warn('👥 No canvas ID available for loading users');
        return;
    }
    
    // FIX: Nur 1 Request gleichzeitig erlauben
    if (window._loadingUsers) return;
    window._loadingUsers = true;
    
    const usersList = document.getElementById('users-list');
    if (!usersList) {
        console.warn('👥 Users list element not found');
        return;
    }
    
    try {
        console.log('👥 Loading users for canvas:', _currentCanvasId);
        
        // Store canvas ID at request start for validation
        const requestCanvasId = _currentCanvasId;
        
        const response = await fetch(`/api/canvas/${_currentCanvasId}/users`, {
            method: 'GET',
            credentials: 'include'
        });
        
        if (response.ok) {
            const data = await response.json();
            
            // FIX: Nur anzeigen wenn noch derselbe Canvas aktiv
            if (_currentCanvasId !== requestCanvasId) {
                console.log('👥 Canvas changed during request, ignoring response');
                return;
            }
            
            if (data && data.users) {
                displayCanvasUsers(data.users);
            } else {
                console.error('👥 Invalid response format:', data);
                displayUsersError('Ungültige Antwort vom Server');
            }
        } else if (response.status === 401) {
            console.warn('👥 Not authenticated for users list');
            displayUsersError('Nicht authentifiziert');
        } else if (response.status === 403) {
            console.warn('👥 No permission to view users list');
            displayUsersError('Keine Berechtigung');
        } else {
            console.error('👥 Failed to load users list:', response.status);
            displayUsersError('Fehler beim Laden');
        }
    } catch (error) {
        console.error('👥 Error loading canvas users:', error);
        displayUsersError('Netzwerkfehler');
    } finally {
        window._loadingUsers = false;  // Request finished
    }
}

// Extract current user's color from user list for shape selection
async function extractCurrentUserColor(users) {
    try {
        // Get current user info from API
        const response = await fetch('/api/user-info', {
            method: 'GET',
            credentials: 'include'
        });
        
        if (response.ok) {
            const userData = await response.json();
            if (userData.success && userData.user_id) {
                // Find current user in the users list
                const currentUser = users.find(user => user.user_id === userData.user_id);
                if (currentUser && currentUser.user_color) {
                    window.currentUserColor = currentUser.user_color;
                    console.log('🎨 Set current user color for shape selection:', currentUser.user_color);
                } else {
                    console.log('👤 Current user not found in canvas users list or no color available');
                }
            }
        }
    } catch (error) {
        console.error('❌ Error extracting current user color:', error);
    }
}

// Display the list of users in the UI
function displayCanvasUsers(users) {
    const usersList = document.getElementById('users-list');
    if (!usersList) return;
    
    // Clear current content
    usersList.innerHTML = '';
    
    if (!users || users.length === 0) {
        usersList.innerHTML = '<span class="users-empty">Keine anderen Benutzer online</span>';
        return;
    }
    
    console.log('👥 Displaying', users.length, 'users:', users);
    
    // Extract current user's color for shape selection
    extractCurrentUserColor(users);
    
    // Create user badges
    users.forEach(user => {
        const badge = createUserBadge(user);
        usersList.appendChild(badge);
    });
}

// Create a user badge element
function createUserBadge(user) {
    const badge = document.createElement('div');
    badge.className = 'user-badge';
    
    // Apply user color styling if available
    if (user.user_color) {
        badge.classList.add('colored');
        
        // Create a vibrant background with user color
        const lightColor = lightenColor(user.user_color, 0.85);
        badge.style.backgroundColor = lightColor;
        badge.style.borderLeft = `4px solid ${user.user_color}`;
        badge.style.borderColor = user.user_color;
        
        console.log(`🎨 Applied color ${user.user_color} to user ${user.display_name}`);
    } else {
        // Fallback for users without color
        badge.style.backgroundColor = '#e9ecef';
        badge.style.color = '#495057';
    }
    
    // Add multiple connections class if user has more than one connection
    if (user.connection_count > 1) {
        badge.classList.add('multiple');
    }
    
    // Create name span
    const nameSpan = document.createElement('span');
    nameSpan.className = 'user-name';
    nameSpan.textContent = user.display_name || user.user_id;
    nameSpan.title = `${user.display_name || user.user_id} (${user.user_id})`;
    
    // Apply user color as text color (darker for readability)
    if (user.user_color) {
        const darkerColor = darkenColor(user.user_color, 0.6);
        nameSpan.style.color = darkerColor;
    }
    
    badge.appendChild(nameSpan);
    
    // Add connection count if more than 1
    if (user.connection_count > 1) {
        const countSpan = document.createElement('span');
        countSpan.className = 'connection-count';
        countSpan.textContent = user.connection_count.toString();
        countSpan.title = `${user.connection_count} Verbindungen`;
        
        // Style connection count with user color
        if (user.user_color) {
            countSpan.style.backgroundColor = user.user_color;
            countSpan.style.color = 'white';
            countSpan.style.boxShadow = `0 1px 3px ${user.user_color}40`;
        }
        
        badge.appendChild(countSpan);
    }
    
    return badge;
}

// Display loading state
function displayUsersLoading() {
    const usersList = document.getElementById('users-list');
    if (usersList) {
        usersList.innerHTML = '<span class="users-loading">Lade Benutzerliste...</span>';
    }
}

// Display error state
function displayUsersError(message) {
    const usersList = document.getElementById('users-list');
    if (usersList) {
        usersList.innerHTML = `<span class="users-error">${message}</span>`;
    }
}

// Start polling for user updates
function startCanvasUsersPolling() {
    // Clear any existing interval
    stopCanvasUsersPolling();
    
    console.log('👥 User polling disabled - using WebSocket events only');
}

// Stop polling for user updates
function stopCanvasUsersPolling() {
    if (_usersPollingInterval) {
        clearInterval(_usersPollingInterval);
        _usersPollingInterval = null;
        console.log('👥 Stopped users polling');
    }
}

// Force refresh users list (can be called externally when WebSocket events occur)
function refreshCanvasUsers(bypassThrottle = false) {
    // FIX: Throttling nur für normale Calls, nicht für WebSocket Events oder SPA Navigation
    if (!bypassThrottle && Date.now() - (window._lastRefresh || 0) < 3000) {
        console.log('👥 Refresh throttled (use bypassThrottle=true for WebSocket events/SPA)');
        return;
    }
    window._lastRefresh = Date.now();
    
    console.log(`👥 Force refreshing canvas users... (bypass: ${bypassThrottle})`);
    
    // SPA-optimized: Ensure we're on a canvas page before refreshing
    const currentCanvasId = getCurrentCanvasId();
    if (!currentCanvasId) {
        console.log('👥 Not on canvas page, skipping user refresh');
        hideCanvasUsersDisplay();
        return;
    }
    
    // Update current canvas ID and refresh
    _currentCanvasId = currentCanvasId;
    console.log('👥 SPA Refresh: Canvas ID set to', _currentCanvasId);
    
    // Show users display and load users
    showCanvasUsersDisplay();
    loadCanvasUsers();
}

// Helper function to get current canvas ID from URL (SPA-optimized)
function getCurrentCanvasId() {
    const currentPath = window.location.pathname;
    const canvasMatch = currentPath.match(/^\/canvas\/([^\/]+)$/);
    return canvasMatch ? canvasMatch[1] : null;
}

// ============================================================================
// COLOR UTILITY FUNCTIONS
// ============================================================================

// Darken a hex color by a given factor (0.0 = black, 1.0 = original color)
function darkenColor(hex, factor) {
    if (!hex || !hex.startsWith('#')) return hex;
    
    // Remove # and parse hex
    const color = hex.slice(1);
    const num = parseInt(color, 16);
    
    // Extract RGB components
    const r = Math.floor((num >> 16) * factor);
    const g = Math.floor(((num >> 8) & 0x00FF) * factor);
    const b = Math.floor((num & 0x0000FF) * factor);
    
    // Ensure values are within valid range
    const clampedR = Math.max(0, Math.min(255, r));
    const clampedG = Math.max(0, Math.min(255, g));
    const clampedB = Math.max(0, Math.min(255, b));
    
    // Convert back to hex
    return `#${((clampedR << 16) | (clampedG << 8) | clampedB).toString(16).padStart(6, '0')}`;
}

// Lighten a hex color by a given factor (0.0 = original color, 1.0 = white)
function lightenColor(hex, factor) {
    if (!hex || !hex.startsWith('#')) return hex;
    
    // Remove # and parse hex
    const color = hex.slice(1);
    const num = parseInt(color, 16);
    
    // Extract RGB components
    const r = (num >> 16);
    const g = (num >> 8) & 0x00FF;
    const b = num & 0x0000FF;
    
    // Lighten each component
    const lightenedR = Math.floor(r + (255 - r) * factor);
    const lightenedG = Math.floor(g + (255 - g) * factor);
    const lightenedB = Math.floor(b + (255 - b) * factor);
    
    // Ensure values are within valid range
    const clampedR = Math.max(0, Math.min(255, lightenedR));
    const clampedG = Math.max(0, Math.min(255, lightenedG));
    const clampedB = Math.max(0, Math.min(255, lightenedB));
    
    // Convert back to hex
    return `#${((clampedR << 16) | (clampedG << 8) | clampedB).toString(16).padStart(6, '0')}`;
}

// Make functions globally available for debugging and WebSocket bridge
window.refreshCanvasUsers = refreshCanvasUsers;
window.initializeCanvasUsersDisplay = initializeCanvasUsersDisplay;
window.loadCanvasUsers = loadCanvasUsers;
window.darkenColor = darkenColor;
window.lightenColor = lightenColor;

console.log('👥 Global functions registered:', {
    refreshCanvasUsers: typeof window.refreshCanvasUsers,
    initializeCanvasUsersDisplay: typeof window.initializeCanvasUsersDisplay,
    loadCanvasUsers: typeof window.loadCanvasUsers,
    darkenColor: typeof window.darkenColor,
    lightenColor: typeof window.lightenColor
});

} // End of if (!window._drawerPageLoaded) block

// ============================================================================
// CANVAS PAGE INITIALIZATION - Runs on every page load/navigation
// ============================================================================

// Always run canvas page initialization (even if functions were already loaded)
console.log("🔄 Running canvas page initialization...");
initializeCanvasPage();