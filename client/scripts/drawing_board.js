(function() {
    console.log("Original-Zeichenbrett wird initialisiert");
    
    // Import menu API
    import('/scripts/menu-api.js').then(module => {
        const menuApi = module.default;
        initDrawingBoard(menuApi);
    }).catch(err => {
        console.error("Fehler beim Laden des Menü-API:", err);
        initDrawingBoard(null);
    });
    
    function initDrawingBoard(menuApi) {
        // Prüfe, ob wir auf der richtigen Seite sind
        const drawingBoard = document.getElementById('drawing-board');
        const mousePosition = document.getElementById('mouse-position');
        const pointsList = document.getElementById('points-list');
        
        if (!drawingBoard) {
            console.log('Nicht auf der Drawing-Board-Seite, Initialisierung wird übersprungen');
            return;
        }
        
        console.log("Zeichenbrett initialisiert");
        
        // Setup context menu if menuApi is available
        if (menuApi) {
            setupContextMenu(menuApi, drawingBoard);
        }
        
        // Event listener für Mausbewegungen
        drawingBoard.addEventListener('mousemove', function(event) {
            const rect = drawingBoard.getBoundingClientRect();
            const x = Math.floor(event.clientX - rect.left);
            const y = Math.floor(event.clientY - rect.top);
            
            if (mousePosition) {
                mousePosition.textContent = `${x}, ${y}`;
            }
        });
        
        // Event listener für Mausklicks
        drawingBoard.addEventListener('click', function(event) {
            const rect = drawingBoard.getBoundingClientRect();
            const x = Math.floor(event.clientX - rect.left);
            const y = Math.floor(event.clientY - rect.top);
            
            // Create new list entry
            const listItem = document.createElement('li');
            listItem.textContent = `Punkt bei (${x}, ${y})`;
            
            // Add point to the list
            pointsList.appendChild(listItem);
        });
        
        // Mouseleave event
        drawingBoard.addEventListener('mouseleave', function() {
            if (mousePosition) {
                mousePosition.textContent = '---, ---';
            }
        });
    }
    
    // Function to set up and return a context menu
    function setupContextMenu(menuApi, drawingBoard) {
        const menu = menuApi.createMenu();
        
        // Create menu items
        const mItem1 = menuApi.createItem("Punkt hinzufügen", (m) => {
            console.log("Punkt hinzufügen");
            
            // Aktuelle Mausposition verwenden oder eine feste Position
            const rect = drawingBoard.getBoundingClientRect();
            
            // Bei einem Kontextmenü können wir die Position des Menüs verwenden
            // oder die letzte gespeicherte Mausposition
            const x = m.lastX || Math.floor(rect.width / 2);
            const y = m.lastY || Math.floor(rect.height / 2);
            
            // Create new list entry
            const listItem = document.createElement('li');
            listItem.textContent = `Punkt bei (${x}, ${y})`;
            
            // Add point to the list
            const pointsList = document.getElementById('points-list');
            if (pointsList) {
                pointsList.appendChild(listItem);
            }
            
            m.hide(); // Hide the menu
        });
        
        const mItem2 = menuApi.createItem("Letzten Punkt löschen", () => {
            console.log("Letzten Punkt löschen");
            const pointsList = document.getElementById('points-list');
            if (pointsList && pointsList.lastChild) {
                pointsList.removeChild(pointsList.lastChild);
            }
        });
        
        // Create a separator
        const mT1 = menuApi.createSeparator();
        
        // Create another menu item
        const mItem3 = menuApi.createItem("Alle Punkte löschen", (m) => {
            console.log("Alle Punkte löschen");
            const pointsList = document.getElementById('points-list');
            if (pointsList) {
                pointsList.innerHTML = '';
            }
            m.hide();
        });
        
        // Add items to menu
        menu.addItems(mItem1, mItem2);
        menu.addItem(mT1);
        menu.addItem(mItem3);
        
        // Add the menu to the drawing board
        drawingBoard.addEventListener('contextmenu', (e) => {
            e.preventDefault(); // Prevent default context menu
            
            // Calculate position relative to the drawing board
            const rect = drawingBoard.getBoundingClientRect();
            menu.lastX = Math.floor(e.clientX - rect.left);
            menu.lastY = Math.floor(e.clientY - rect.top);
            
            menu.show(e.clientX, e.clientY);
        });
        
        return menu;
    }
})();