// ============================================================================
// FRONTEND TEST SETUP
// Globale Test-Konfiguration für Frontend-Tests
// ============================================================================

// Mock für fetch API (falls nicht verfügbar)
global.fetch = require('node-fetch');

// Mock für localStorage
const localStorageMock = {
  getItem: jest.fn(),
  setItem: jest.fn(),
  removeItem: jest.fn(),
  clear: jest.fn(),
};
global.localStorage = localStorageMock;

// Mock für sessionStorage
global.sessionStorage = localStorageMock;

// Mock für window.location
Object.defineProperty(window, 'location', {
  value: {
    href: 'http://localhost:3000/',
    pathname: '/',
    search: '',
    hash: '',
    reload: jest.fn(),
  },
  writable: true,
});

// Mock für console um Test-Output sauber zu halten
const originalConsoleError = console.error;
console.error = (...args) => {
  if (typeof args[0] === 'string' && args[0].includes('Warning:')) {
    return;
  }
  originalConsoleError.call(console, ...args);
};

// Globale Test-Utilities
global.testUtils = {
  // Simuliert einen erfolgreichen API-Response
  mockSuccessResponse: (data) => ({
    ok: true,
    status: 200,
    json: async () => data,
    headers: new Map([
      ['content-type', 'application/json'],
      ['set-cookie', 'auth_token=fake-jwt-token; HttpOnly; Path=/; Max-Age=86400']
    ])
  }),
  
  // Simuliert einen API-Fehler
  mockErrorResponse: (status, message) => ({
    ok: false,
    status: status,
    json: async () => ({ success: false, message: message, email: null })
  }),
  
  // Wartet auf DOM-Updates
  waitForNextTick: () => new Promise(resolve => setTimeout(resolve, 0)),
  
  // Simuliert User-Eingaben
  simulateUserInput: (element, value) => {
    element.value = value;
    element.dispatchEvent(new Event('input', { bubbles: true }));
    element.dispatchEvent(new Event('change', { bubbles: true }));
  }
};