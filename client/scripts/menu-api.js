/**
 * Menu API - Framework for creating and using popup menus
 */

// Main API object to export
const menuApi = (function() {
    // Unique identifier for menu elements
    let menuIdCounter = 0;

    /**
     * Menu Item class - represents a clickable item in the menu
     */
    class MenuItem {
        constructor(label, callback) {
            this.label = label;
            this.callback = callback;
            this.id = `menu-item-${++menuIdCounter}`;
        }

        /**
         * Renders the menu item as a DOM element
         * @param {Menu} menu - The parent menu instance
         * @returns {HTMLElement} The DOM element representing this item
         */
        render(menu) {
            const item = document.createElement('div');
            item.className = 'menu-item';
            item.textContent = this.label;
            item.id = this.id;
            
            item.addEventListener('click', (e) => {
                e.stopPropagation();
                // Execute the item's callback with the menu as parameter
                if (typeof this.callback === 'function') {
                    this.callback(menu);
                }
            });
            
            return item;
        }
    }

    /**
     * Menu Radio Option class - represents a group of radio buttons in the menu
     */
    class MenuRadioOption {
        constructor(label, options, selectedKey = null) {
            this.label = label;
            this.options = options || {};
            this.selectedKey = selectedKey;
            this.id = `menu-radio-${++menuIdCounter}`;
            this.onChange = null;
        }

        /**
         * Sets the selected option
         * @param {string} key - The key of the option to select
         */
        setSelected(key) {
            this.selectedKey = key;
            if (this.onChange) {
                this.onChange(key);
            }
        }

        /**
         * Gets the currently selected key
         * @returns {string} The selected key
         */
        getSelected() {
            return this.selectedKey;
        }

        /**
         * Sets the onChange callback
         * @param {Function} callback - Function to call when selection changes
         */
        setOnChange(callback) {
            this.onChange = callback;
        }

        /**
         * Renders the radio options as a DOM element
         * @param {Menu} menu - The parent menu instance
         * @returns {HTMLElement} The DOM element representing this radio group
         */
        render(menu) {
            const container = document.createElement('div');
            container.className = 'menu-radio-group';
            container.id = this.id;
            
            // Add label
            const labelEl = document.createElement('div');
            labelEl.className = 'menu-radio-label';
            labelEl.textContent = this.label;
            container.appendChild(labelEl);
            
            // Add options
            const optionsContainer = document.createElement('div');
            optionsContainer.className = 'menu-radio-options';
            
            Object.keys(this.options).forEach(key => {
                const option = document.createElement('div');
                option.className = 'menu-radio-option';
                
                const radio = document.createElement('input');
                radio.type = 'radio';
                radio.name = this.id;
                radio.value = key;
                radio.id = `${this.id}-${key}`;
                radio.checked = key === this.selectedKey;
                
                const label = document.createElement('label');
                label.htmlFor = radio.id;
                label.textContent = this.options[key];
                
                option.appendChild(radio);
                option.appendChild(label);
                
                radio.addEventListener('change', () => {
                    if (radio.checked) {
                        this.setSelected(key);
                        menu.hide();
                    }
                });
                
                optionsContainer.appendChild(option);
            });
            
            container.appendChild(optionsContainer);
            return container;
        }
    }

    /**
     * Menu Separator class - represents a dividing line in the menu
     */
    class MenuSeparator {
        constructor() {
            this.id = `menu-separator-${++menuIdCounter}`;
        }

        /**
         * Renders the separator as a DOM element
         * @returns {HTMLElement} The DOM element representing this separator
         */
        render() {
            const separator = document.createElement('div');
            separator.className = 'menu-separator';
            separator.id = this.id;
            return separator;
        }
    }

    /**
     * Menu class - represents a popup menu
     */
    class Menu {
        constructor() {
            this.items = [];
            this.id = `menu-${++menuIdCounter}`;
            this.element = null;
            this.overlay = null;
            this.isVisible = false;
        }

        /**
         * Adds an item to the menu
         * @param {MenuItem|MenuSeparator|MenuRadioOption} item - The item to add
         * @returns {Menu} This menu instance for chaining
         */
        addItem(item) {
            this.items.push(item);
            
            // If the menu is already visible, update it
            if (this.isVisible && this.element) {
                this._renderItem(item);
            }
            
            return this;
        }

        /**
         * Adds multiple items to the menu
         * @param {...MenuItem|MenuSeparator|MenuRadioOption} items - The items to add
         * @returns {Menu} This menu instance for chaining
         */
        addItems(...items) {
            items.forEach(item => this.addItem(item));
            return this;
        }

        /**
         * Adds an item at a specific position in the menu
         * @param {MenuItem|MenuSeparator|MenuRadioOption} item - The item to add
         * @param {number} position - The position at which to add the item
         * @returns {Menu} This menu instance for chaining
         */
        addItemAt(item, position) {
            this.items.splice(position, 0, item);
            
            // If the menu is already visible, re-render it
            if (this.isVisible) {
                this._render();
            }
            
            return this;
        }

        /**
         * Removes an item from the menu
         * @param {MenuItem|MenuSeparator|MenuRadioOption} item - The item to remove
         * @returns {Menu} This menu instance for chaining
         */
        removeItem(item) {
            const index = this.items.indexOf(item);
            if (index !== -1) {
                this.items.splice(index, 1);
                
                // If the menu is visible, update it
                if (this.isVisible && this.element) {
                    const itemElement = document.getElementById(item.id);
                    if (itemElement) {
                        itemElement.remove();
                    }
                }
            }
            
            return this;
        }

        /**
         * Shows the menu at the specified coordinates
         * @param {number} x - The x coordinate
         * @param {number} y - The y coordinate
         */
        show(x, y) {
            // Create overlay if it doesn't exist
            if (!this.overlay) {
                this.overlay = document.createElement('div');
                this.overlay.className = 'menu-overlay';
                this.overlay.addEventListener('click', () => {
                    this.hide();
                });
                document.body.appendChild(this.overlay);
            }

            // Create menu element if it doesn't exist
            if (!this.element) {
                this.element = document.createElement('div');
                this.element.className = 'menu';
                this.element.id = this.id;
                document.body.appendChild(this.element);
            }

            // Clear the menu and render all items
            this.element.innerHTML = '';
            this._render();

            // Position the menu
            this.element.style.left = `${x}px`;
            this.element.style.top = `${y}px`;

            // Show the menu and overlay
            this.overlay.style.display = 'block';
            this.element.style.display = 'block';
            this.isVisible = true;

            // Adjust position if menu goes outside viewport
            this._adjustPosition();
        }

        /**
         * Hides the menu
         */
        hide() {
            if (this.element) {
                this.element.style.display = 'none';
            }
            
            if (this.overlay) {
                this.overlay.style.display = 'none';
            }
            
            this.isVisible = false;
        }

        /**
         * Renders all menu items
         * @private
         */
        _render() {
            if (!this.element) return;
            
            this.element.innerHTML = '';
            this.items.forEach(item => this._renderItem(item));
        }

        /**
         * Renders a single menu item
         * @param {MenuItem|MenuSeparator|MenuRadioOption} item - The item to render
         * @private
         */
        _renderItem(item) {
            if (!this.element) return;
            
            const itemElement = item.render(this);
            this.element.appendChild(itemElement);
        }

        /**
         * Adjusts the menu position to ensure it's fully visible
         * @private
         */
        _adjustPosition() {
            if (!this.element) return;
            
            const rect = this.element.getBoundingClientRect();
            const viewportWidth = window.innerWidth;
            const viewportHeight = window.innerHeight;
            
            // Adjust horizontal position if needed
            if (rect.right > viewportWidth) {
                const newLeft = Math.max(0, viewportWidth - rect.width);
                this.element.style.left = `${newLeft}px`;
            }
            
            // Adjust vertical position if needed
            if (rect.bottom > viewportHeight) {
                const newTop = Math.max(0, viewportHeight - rect.height);
                this.element.style.top = `${newTop}px`;
            }
        }
    }

    // Add CSS for radio options
    function addRadioOptionStyles() {
        if (!document.getElementById('menu-radio-styles')) {
            const style = document.createElement('style');
            style.id = 'menu-radio-styles';
            style.textContent = `
                .menu-radio-group {
                    padding: 8px 12px;
                }
                .menu-radio-label {
                    font-weight: bold;
                    margin-bottom: 4px;
                }
                .menu-radio-options {
                    margin-left: 8px;
                }
                .menu-radio-option {
                    padding: 3px 0;
                    display: flex;
                    align-items: center;
                }
                .menu-radio-option label {
                    margin-left: 6px;
                    cursor: pointer;
                }
            `;
            document.head.appendChild(style);
        }
    }

    // Add styles when module loads
    addRadioOptionStyles();

    // Public API
    return {
        /**
         * Creates a new menu instance
         * @returns {Menu} A new menu instance
         */
        createMenu() {
            return new Menu();
        },
        
        /**
         * Creates a new menu item
         * @param {string} label - The label text for the item
         * @param {Function} callback - The function to call when the item is clicked
         * @returns {MenuItem} A new menu item
         */
        createItem(label, callback) {
            return new MenuItem(label, callback);
        },
        
        /**
         * Creates a new radio option group
         * @param {string} label - Group label
         * @param {Object} options - Key-value pairs for options
         * @param {string} selectedKey - Initially selected key
         * @returns {MenuRadioOption} A new radio option group
         */
        createRadioOption(label, options, selectedKey = null) {
            return new MenuRadioOption(label, options, selectedKey);
        },
        
        /**
         * Creates a new menu separator
         * @returns {MenuSeparator} A new menu separator
         */
        createSeparator() {
            return new MenuSeparator();
        }
    };
})();

export default menuApi;