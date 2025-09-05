# Drawing App - Test System

Dieses Projekt enthält eine umfassende, organisierte Test-Suite für die Drawing App mit fokus auf das Authentifizierungs-System.

## 📁 **Neue Test-Struktur (Reorganisiert)**

```
tests/
├── auth_integration.rs      # 🔗 Backend HTTP API Tests (9 tests)
├── auth_unit.rs            # 🔧 Backend Module Unit Tests (11 tests)  
├── frontend/               # 🌐 Frontend Test Setup (vorbereitet)
│   ├── package.json        # Jest & Playwright Konfiguration
│   ├── test-setup.js       # Globale Test-Utilities
│   ├── e2e/               # Browser End-to-End Tests
│   │   └── auth.spec.js    # Login/Register E2E Tests
│   └── unit/              # JavaScript Unit Tests
│       └── spa.test.js     # SPA Navigation Tests
├── scripts/               # 📋 Test-Automation Scripts
│   ├── run-all-tests.bat  # Alle Tests ausführen
│   ├── run-backend-tests.bat
│   └── run-frontend-tests.bat
└── backend/              # 📂 Erweiterte Backend-Struktur
    ├── integration/      # (für zukünftige Erweiterungen)
    ├── unit/            
    └── fixtures/         # Test-Daten und Helper
```

## 🧪 Test-Übersicht

### Was wird getestet?

**✅ User Registration**
- Erfolgreiche Registrierung neuer Benutzer
- Verhinderung doppelter Email-Adressen  
- Korrekte HTTP-Only Cookie-Erstellung
- Sichere Passwort-Hashing mit bcrypt

**✅ User Login**
- Erfolgreicher Login mit korrekten Credentials
- Fehlschlag bei falschen Passwörtern
- Fehlschlag bei nicht existierenden Benutzern
- JWT Token-Erstellung nach Login

**✅ JWT Token Validation**
- Token-Validierung mit gültigen Cookies
- Ablehnung ohne Cookies
- HTTP-Only Cookie-Handling

**✅ User Logout**
- Erfolgreiche Logout-Funktionalität
- Cookie-Löschung beim Logout
- Token-Invalidierung nach Logout

**✅ Complete Auth Flow**
- Vollständiger Zyklus: Register → Login → Logout → Re-Login
- End-to-End Authentifizierungs-Workflow

## 🚀 **Tests ausführen**

### **Methode 1: Einfaches Batch-Script (Empfohlen)**
```bash
# Alle Tests mit übersichtlicher Ausgabe
run_tests.bat

# Erweiterte Scripts (in tests/scripts/)
tests/scripts/run-all-tests.bat      # Vollständige Test-Suite
tests/scripts/run-backend-tests.bat  # Nur Backend Tests
tests/scripts/run-frontend-tests.bat # Nur Frontend Tests
```

### **Methode 2: Cargo direkt (Einzeln)**
```bash
# Backend Integration Tests (9 tests)
cargo test --test auth_integration

# Backend Unit Tests (11 tests) 
cargo test --test auth_unit

# Alle Backend Tests zusammen
cargo test auth_integration auth_unit

# Mit detaillierter Ausgabe
cargo test --test auth_integration -- --nocapture
```

### **Methode 3: Frontend Tests (Setup vorbereitet)**
```bash
cd tests/frontend
npm install
npm test              # Unit Tests
npm run test:e2e      # E2E Tests (Server muss laufen)
```

### **Methode 4: Kontinuierliche Tests (Entwicklung)**
```bash
# Tests bei Datei-Änderungen automatisch ausführen
cargo watch -x "test --test auth_integration"
cargo watch -x "test --test auth_unit"
```

## 🛠️ Test-Konfiguration

### Dependencies
Die Tests verwenden:
- `reqwest` - HTTP Client für API-Requests
- `tokio-test` - Async Test Framework
- `serde_json` - JSON Serialisierung
- `assert_matches` - Erweiterte Assertions

### Test-Setup
Jeder Test:
1. Startet einen lokalen Test-Server auf einem freien Port
2. Führt HTTP-Requests gegen die echten API-Endpunkte aus
3. Validiert HTTP-Status-Codes und Response-Bodies
4. Überprüft Cookie-Handling und JWT-Token

## 📋 Test-Details

### Integration Tests (`tests/auth_integration_tests.rs`)

**10 Test-Fälle abgedeckt:**
1. `test_user_registration_success` - Erfolgreiche Registrierung
2. `test_user_registration_duplicate_email` - Doppelte Email verhindert
3. `test_user_registration_sets_cookie` - HTTP-Only Cookies gesetzt
4. `test_user_login_success` - Erfolgreicher Login
5. `test_user_login_wrong_password` - Falsches Passwort abgelehnt
6. `test_user_login_nonexistent_user` - Unbekannter User abgelehnt
7. `test_token_validation_success` - Token-Validierung funktioniert
8. `test_token_validation_without_cookie` - Ohne Cookie abgelehnt
9. `test_user_logout` - Logout funktioniert
10. `test_complete_auth_flow` - Vollständiger Auth-Zyklus

### HTTP-Endpunkte getestet
- `POST /api/register` - User-Registrierung
- `POST /api/login` - User-Login  
- `POST /api/logout` - User-Logout
- `GET /api/validate-token` - JWT-Validierung

## 🔒 Sicherheits-Features getestet

- **HTTP-Only Cookies**: JavaScript kann nicht auf Auth-Tokens zugreifen
- **SameSite=Strict**: CSRF-Schutz 
- **Bcrypt Password Hashing**: Sichere Passwort-Speicherung
- **JWT Token Expiration**: 24h Ablaufzeit
- **Input Validation**: Email-Format und Passwort-Anforderungen

## 📊 Test-Ergebnisse

Bei erfolgreicher Ausführung sollten alle 10 Tests bestehen:

```
running 10 tests
test test_user_registration_success ... ok
test test_user_registration_duplicate_email ... ok  
test test_user_registration_sets_cookie ... ok
test test_user_login_success ... ok
test test_user_login_wrong_password ... ok
test test_user_login_nonexistent_user ... ok
test test_token_validation_success ... ok
test test_token_validation_without_cookie ... ok
test test_user_logout ... ok
test test_complete_auth_flow ... ok

test result: ok. 10 passed; 0 failed; 0 ignored
```

## 🔧 Fehlerbehebung

### Häufige Probleme

**Port bereits belegt**: Tests verwenden automatisch freie Ports
**Kompilierungsfehler**: `cargo build` vor Tests ausführen  
**Timeout-Fehler**: Server-Prozesse mit `taskkill` beenden

### Debug-Modus
```bash
# Tests mit Debug-Ausgabe
RUST_LOG=debug cargo test --test auth_integration_tests -- --nocapture
```

## 🎯 Aufgabe A 5.1 Compliance

Diese Tests validieren die vollständige Erfüllung der Aufgabe A 5.1:

✅ **Login-Seite** (`/login`) - Getestet mit HTTP-Requests
✅ **Register-Seite** (`/register`) - Registrierungs-API getestet  
✅ **Home-Seite** (`/`) - Auth-Weiterleitung getestet
✅ **JWT mit Email** - Token-Inhalt validiert
✅ **HTTP-Only Cookies** - Cookie-Attribute überprüft
✅ **Ohne DB-Zugriff** - JWT-Validierung rein token-basiert

Das Authentifizierungs-System erfüllt alle Sicherheits- und Funktionalitäts-Anforderungen der Aufgabenstellung.