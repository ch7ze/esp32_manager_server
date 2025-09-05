// Information about available pages
const pages = {
    'index': {
        title: 'Home',
        template: 'index.html',
        defaultPath: 'index.html',
        scripts: [],
        styles: [],
        requiresAuth: true
    },
    'login': {
        title: 'Login',
        template: 'login.html',
        defaultPath: 'login.html',
        scripts: [],
        styles: [],
        requiresAuth: false
    },
    'register': {
        title: 'Registrierung',
        template: 'register.html',
        defaultPath: 'register.html',
        scripts: [],
        styles: [],
        requiresAuth: false
    },
    'hallo': {
        title: 'Hello',
        template: 'hallo.html',
        defaultPath: 'hallo.html',
        scripts: [],
        styles: [],
        requiresAuth: true
    },
    'about': {
        title: 'About',
        template: 'about.html',
        defaultPath: 'about.html',
        scripts: [],
        styles: [],
        requiresAuth: true
    },
    'drawing_board': {
        title: 'Drawing Board',
        template: 'drawing_board.html',
        defaultPath: 'drawing_board.html',
        scripts: ['drawing_board.js'],
        styles: ['drawing_board.css'],
        requiresAuth: true
    },
    'drawer_page': {
        title: 'Drawer',
        template: 'drawer_page.html',
        defaultPath: 'drawer_page.html',
        scripts: ['websocket-client.js', 'canvas-websocket-bridge.js', 'event-system.js', 'color-state.js', 'drawer-state.js', 'drawer_page.js', 'drawer/event-wrapper.js'],
        styles: ['drawer_page.css'],
        requiresAuth: true
    },
    'debug': {
        title: 'Debug',
        template: 'debug.html',
        defaultPath: 'debug.html',
        scripts: [],
        styles: [],
        requiresAuth: false
    },
    'canvas_detail': {
        title: 'Canvas',
        template: 'canvas_detail.html',
        defaultPath: '/canvas/:id',
        scripts: ['websocket-client.js', 'canvas-websocket-bridge.js', 'event-system.js', 'color-state.js', 'drawer-state.js', 'drawer_page.js', 'drawer/event-wrapper.js'],
        styles: ['drawer_page.css'],
        requiresAuth: true
    },
    'docs': {
        title: 'Dokumentation',
        template: 'docs.html',
        defaultPath: 'docs.html',
        scripts: [],
        styles: [],
        requiresAuth: true
    }
};

// Cache templates to avoid repeated fetches
const templateCache = {};

// Function for loading a template and extracting scripts
async function loadTemplate(templateName) {
  // Return cached template if available
  if (templateCache[templateName]) {
    return templateCache[templateName];
  }
  
  try {
    const response = await fetch(`/templates/${templateName}`);
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }
    const template = await response.text();
    
    // Extract and store scripts separately
    const scriptRegex = /<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi;
    const scripts = template.match(scriptRegex) || [];
    const templateWithoutScripts = template.replace(scriptRegex, '');
    
    // Cache both template and scripts
    templateCache[templateName] = {
      html: templateWithoutScripts,
      scripts: scripts.map(script => {
        // Extract script content between tags
        const scriptContent = script.replace(/<script[^>]*>/, '').replace(/<\/script>/, '');
        return scriptContent;
      })
    };
    
    return templateCache[templateName];
  } catch (error) {
    console.error(`Error loading template ${templateName}:`, error);
    return { html: '<p>Error loading content</p>', scripts: [] };
  }
}

// Authentication utility functions - HTTP-Only Cookie compatible
async function isAuthenticated() {
    // With HTTP-Only cookies we cannot read the cookie directly
    // We need to ask the server if the cookie is valid
    try {
        const response = await fetch('/api/validate-token', {
            method: 'GET',
            credentials: 'include'
        });
        return response.ok;
    } catch (error) {
        console.error('Token validation error:', error);
        return false;
    }
}

// Function to update global navigation based on authentication status
function updateGlobalNavigation(authenticated) {
    const mainNav = document.getElementById('main-nav');
    if (!mainNav) return;
    
    if (authenticated) {
        // Show navigation for authenticated users
        mainNav.innerHTML = `
            <a href="/" class="spa-link home-link">Home</a>
            <a href="/hallo" class="spa-link">Hello</a>
            <a href="/docs" class="spa-link">Dokumentation</a>
            <button id="global-logout-btn" class="logout-button">Logout</button>
        `;
        
        // Add logout functionality
        const logoutBtn = document.getElementById('global-logout-btn');
        if (logoutBtn) {
            logoutBtn.addEventListener('click', async function() {
                try {
                    await fetch('/api/logout', {
                        method: 'POST',
                        credentials: 'include'
                    });
                } catch (error) {
                    console.error('Logout error:', error);
                }
                navigateTo('/login');
            });
        }
    } else {
        // Show navigation for non-authenticated users
        mainNav.innerHTML = `
            <a href="/login" class="spa-link">Login</a>
            <a href="/register" class="spa-link">Register</a>
        `;
    }
}

// Function for rendering the page based on the current URL
async function renderPage() {
    const contentContainer = document.getElementById('content-container');
    
    // Analyze URL to determine the current page
    const url = new URL(window.location.href);
    let path = url.pathname.split('/').pop() || 'index.html';
    
    // Handle root path
    if (url.pathname === '/') {
        path = 'index.html';
    }
    
    let pageName = Object.keys(pages).find(page => 
        pages[page].defaultPath === path) || 'index';
    
    // Handle special URL cases
    if (path === 'login') {
        pageName = 'login';
    } else if (path === 'register') {
        pageName = 'register';
    } else if (path === 'docs') {
        pageName = 'docs';
    } else if (path === 'hallo') {
        pageName = 'hallo';
    } else if (path === 'about') {
        pageName = 'about';
    } else if (path === 'drawing_board') {
        pageName = 'drawing_board';
    } else if (path === 'drawer_page') {
        pageName = 'drawer_page';
    } else if (path === 'debug') {
        pageName = 'debug';
    }
    
    // Handle canvas detail pages
    if (url.pathname.startsWith('/canvas/')) {
        pageName = 'canvas_detail';
    }
    
    // Page information
    const pageInfo = pages[pageName];
    
    // Check authentication requirements
    const authenticated = await isAuthenticated();
    
    if (pageInfo.requiresAuth && !authenticated) {
        // Redirect to login page if authentication is required but user is not authenticated
        navigateTo('/login');
        return;
    }
    
    // If user is authenticated but trying to access login/register, redirect to home
    if (!pageInfo.requiresAuth && authenticated && (pageName === 'login' || pageName === 'register')) {
        navigateTo('/');
        return;
    }
    
    // Set document title
    document.title = pageInfo.title;
    
    // Update global navigation based on authentication status
    updateGlobalNavigation(authenticated);
    
    // Load CSS
    const existingStyles = document.querySelectorAll('link[data-dynamic-style]');
    existingStyles.forEach(style => style.remove());
    
    pageInfo.styles.forEach(style => {
        const styleLink = document.createElement('link');
        styleLink.rel = 'stylesheet';
        styleLink.href = `/styles/${style}`;
        styleLink.setAttribute('data-dynamic-style', 'true');
        document.head.appendChild(styleLink);
    });
    
    // Load template and insert into container
    const templateData = await loadTemplate(pageInfo.template);
    contentContainer.innerHTML = templateData.html;
    
    // Execute template scripts immediately after DOM injection
    if (templateData.scripts && templateData.scripts.length > 0) {
        templateData.scripts.forEach((scriptContent, index) => {
            try {
                // Create a new Function to execute the script in global scope
                const executeScript = new Function(scriptContent);
                executeScript();
                console.log(`Template script ${index + 1} executed successfully`);
            } catch (error) {
                console.error(`Error executing template script ${index + 1}:`, error);
            }
        });
    }
    
    // Load and execute scripts in sequence to maintain order
    const existingScripts = document.querySelectorAll('script[data-dynamic-script]');
    existingScripts.forEach(script => script.remove());    // Load scripts in sequence
    async function loadScriptsSequentially() {
        for (const scriptSrc of pageInfo.scripts) {
            await new Promise((resolve, reject) => {
                const scriptElement = document.createElement('script');
                scriptElement.src = `/scripts/${scriptSrc}`;
                scriptElement.setAttribute('data-dynamic-script', 'true');
                scriptElement.onload = () => resolve();
                scriptElement.onerror = (e) => reject(e);
                document.body.appendChild(scriptElement);
                console.log(`Script ${scriptSrc} loaded`);
            });
        }
    }

    loadScriptsSequentially().catch(err => {
        console.error("Failed to load scripts:", err);
    });
}

// Add this function to handle SPA navigation with reliable canvas cleanup
async function navigateTo(url) {
  console.log(`NavigateTo called: ${window.location.pathname} → ${url}`);
  
  // Canvas cleanup when leaving canvas
  const currentPath = window.location.pathname;
  const currentCanvasMatch = currentPath.match(/^\/canvas\/([^\/]+)$/);
  const newCanvasMatch = url.match(/^\/canvas\/([^\/]+)$/);
  
  const currentCanvasId = currentCanvasMatch ? currentCanvasMatch[1] : null;
  const newCanvasId = newCanvasMatch ? newCanvasMatch[1] : null;
  
  // Synchronous canvas cleanup - wait for server confirmation before navigation
  if (currentCanvasId && (!newCanvasId || newCanvasId !== currentCanvasId)) {
    console.log(`Canvas cleanup needed: ${currentPath} → ${url}`);
    try {
      if (window.unregisterFromCanvas) {
        console.log('Waiting for canvas unregistration to complete...');
        await window.unregisterFromCanvas();
        console.log(`Canvas unregistered: ${currentCanvasId}`);
      } else {
        console.warn('unregisterFromCanvas not available');
      }
    } catch (error) {
      console.error('Canvas unregistration failed:', error);
      // Continue navigation even if cleanup fails to avoid hanging the UI
    }
  }
  
  // Update browser history
  history.pushState(null, null, url);
  // Render the new page
  renderPage();
  
  // SPA user refresh: update user list when navigating to canvas
  if (newCanvasMatch) {
    const targetCanvasId = newCanvasMatch[1];
    console.log('SPA Navigation: Refreshing user list for canvas:', targetCanvasId);
    
    // Check if this is a canvas-to-canvas navigation (different canvas)
    const isCanvasToCanvasNavigation = currentCanvasId && targetCanvasId !== currentCanvasId;
    if (isCanvasToCanvasNavigation) {
      console.log('Canvas-to-Canvas navigation detected:', currentCanvasId, '→', targetCanvasId);
    }
    
    // Give DOM time to render, then refresh user list
    setTimeout(() => {
      if (window.refreshCanvasUsers) {
        window.refreshCanvasUsers(true); // bypassThrottle = true for SPA navigation
        console.log('SPA Navigation: User list refreshed for canvas:', targetCanvasId);
      } else {
        console.warn('SPA Navigation: refreshCanvasUsers not available yet, retrying...');
        // Retry once for edge cases where scripts are still loading
        setTimeout(() => {
          if (window.refreshCanvasUsers) {
            window.refreshCanvasUsers(true);
            console.log('SPA Navigation: User list refreshed (retry)');
          }
        }, 500);
      }
    }, 200); // 200ms delay for DOM rendering
  }
}

// Store the original async function
const _navigateToAsync = navigateTo;

// Make navigateTo and renderPage globally available
// Wrapper for backward compatibility - can be called async or sync
window.navigateTo = (url) => {
  _navigateToAsync(url).catch(error => {
    console.error('Navigation failed:', error);
    renderPage(); // Fallback
  });
};
window.renderPage = renderPage;

// Add event delegation for SPA links with async navigation
document.addEventListener('click', function(e) {
  // Find closest anchor tag
  const link = e.target.closest('a.spa-link');
  if (link) {
    e.preventDefault();
    // Handle async navigation with error handling using original async function
    _navigateToAsync(link.href).catch(error => {
      console.error('Navigation failed:', error);
      // Fallback: still try to render page even if cleanup failed
      renderPage();
    });
  }
});

// Initially render the page
document.addEventListener('DOMContentLoaded', renderPage);

// For browser back button support
window.addEventListener('popstate', renderPage);