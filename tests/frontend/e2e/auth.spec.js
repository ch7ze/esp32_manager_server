// ============================================================================
// END-TO-END AUTH TESTS
// Browser-basierte Tests für Login/Register-Funktionalität
// ============================================================================

const { test, expect } = require('@playwright/test');

// Test-Konfiguration
const BASE_URL = 'http://localhost:3000';

test.describe('Authentication System E2E Tests', () => {
  
  test.beforeEach(async ({ page }) => {
    // Vor jedem Test: Cookies löschen
    await page.context().clearCookies();
  });

  test('User Registration Flow', async ({ page }) => {
    // Zur Register-Seite navigieren
    await page.goto(`${BASE_URL}/register`);
    await expect(page).toHaveTitle(/Registrierung/);
    
    // Formular ausfüllen
    await page.fill('#email', 'e2e-test@example.com');
    await page.fill('#password', 'testpassword123');
    await page.fill('#password-confirm', 'testpassword123');
    
    // Registrierung abschicken
    await page.click('button[type="submit"]');
    
    // Auf Erfolg warten
    await expect(page.locator('#auth-message.success')).toBeVisible();
    await expect(page.locator('#auth-message')).toContainText('erfolgreich');
    
    // Sollte zur Startseite weiterleiten
    await expect(page).toHaveURL(`${BASE_URL}/`);
    
    // Überprüfen ob eingeloggt
    await expect(page.locator('#user-info')).toBeVisible();
    await expect(page.locator('#logout-btn')).toBeVisible();
  });

  test('User Login Flow', async ({ page }) => {
    // Zuerst einen User registrieren (Setup)
    await page.goto(`${BASE_URL}/register`);
    await page.fill('#email', 'login-test@example.com');
    await page.fill('#password', 'loginpass123');
    await page.fill('#password-confirm', 'loginpass123');
    await page.click('button[type="submit"]');
    
    // Ausloggen
    await page.click('#logout-btn');
    await expect(page).toHaveURL(`${BASE_URL}/login`);
    
    // Jetzt einloggen
    await page.fill('#email', 'login-test@example.com');
    await page.fill('#password', 'loginpass123');
    await page.click('button[type="submit"]');
    
    // Erfolg prüfen
    await expect(page.locator('#auth-message.success')).toBeVisible();
    await expect(page).toHaveURL(`${BASE_URL}/`);
    await expect(page.locator('#user-info')).toBeVisible();
  });

  test('Login with Wrong Password', async ({ page }) => {
    // User registrieren
    await page.goto(`${BASE_URL}/register`);
    await page.fill('#email', 'wrong-pass@example.com');
    await page.fill('#password', 'correctpass123');
    await page.fill('#password-confirm', 'correctpass123');
    await page.click('button[type="submit"]');
    
    // Ausloggen
    await page.click('#logout-btn');
    
    // Mit falschem Passwort einloggen
    await page.fill('#email', 'wrong-pass@example.com');
    await page.fill('#password', 'wrongpassword');
    await page.click('button[type="submit"]');
    
    // Fehler-Nachricht prüfen
    await expect(page.locator('#auth-message.error')).toBeVisible();
    await expect(page.locator('#auth-message')).toContainText('Invalid credentials');
    
    // Sollte auf Login-Seite bleiben
    await expect(page).toHaveURL(`${BASE_URL}/login`);
  });

  test('Authentication Redirect Logic', async ({ page }) => {
    // Ohne Login auf geschützte Seite zugreifen
    await page.goto(`${BASE_URL}/`);
    
    // Sollte zu Login weiterleiten
    await expect(page).toHaveURL(`${BASE_URL}/login`);
    
    // Nach erfolgreichem Login sollte Zugriff funktionieren
    // (Hier würden wir einen existierenden User verwenden)
  });

  test('Logout Functionality', async ({ page }) => {
    // User registrieren und einloggen
    await page.goto(`${BASE_URL}/register`);
    await page.fill('#email', 'logout-test@example.com');
    await page.fill('#password', 'logouttest123');
    await page.fill('#password-confirm', 'logouttest123');
    await page.click('button[type="submit"]');
    
    // Prüfen dass eingeloggt
    await expect(page.locator('#logout-btn')).toBeVisible();
    
    // Ausloggen
    await page.click('#logout-btn');
    
    // Sollte zu Login-Seite weiterleiten
    await expect(page).toHaveURL(`${BASE_URL}/login`);
    
    // Versuch auf geschützte Seite zuzugreifen sollte fehlschlagen
    await page.goto(`${BASE_URL}/`);
    await expect(page).toHaveURL(`${BASE_URL}/login`);
  });

  test('SPA Navigation', async ({ page }) => {
    // User registrieren
    await page.goto(`${BASE_URL}/register`);
    await page.fill('#email', 'nav-test@example.com');
    await page.fill('#password', 'navtest123');
    await page.fill('#password-confirm', 'navtest123');
    await page.click('button[type="submit"]');
    
    // Navigation zwischen Seiten testen
    await page.click('a[href="hallo.html"]');
    await expect(page).toHaveURL(/hallo/);
    
    await page.click('a[href="about.html"]');
    await expect(page).toHaveURL(/about/);
    
    // Zurück zur Startseite
    await page.click('a[href="index.html"]');
    await expect(page).toHaveURL(`${BASE_URL}/`);
  });

  test('Drawing Canvas Functionality', async ({ page }) => {
    // User registrieren
    await page.goto(`${BASE_URL}/register`);
    await page.fill('#email', 'draw-test@example.com');
    await page.fill('#password', 'drawtest123');
    await page.fill('#password-confirm', 'drawtest123');
    await page.click('button[type="submit"]');
    
    // Zeichenfläche sollte sichtbar sein
    await expect(page.locator('#drawArea')).toBeVisible();
    await expect(page.locator('.tools')).toBeVisible();
    
    // Tool auswählen
    await page.click('.tools li[data-tool="line"]');
    await expect(page.locator('.tools li.selected')).toContainText('Linie');
    
    // Canvas sollte bereit für Interaktion sein
    const canvas = page.locator('#drawArea');
    await expect(canvas).toBeVisible();
    
    // Mausposition sollte angezeigt werden
    await canvas.hover();
    await expect(page.locator('#mouse-position')).not.toContainText('---, ---');
  });
});