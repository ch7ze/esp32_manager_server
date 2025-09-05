@echo off
echo ========================================
echo Frontend Tests - Drawing App
echo ========================================

set QUIET_MODE=0
if "%1"=="--quiet" set QUIET_MODE=1

cd /d "%~dp0..\frontend"

echo.
echo Checking if frontend test dependencies are installed...

if not exist "node_modules" (
    echo Installing frontend test dependencies...
    npm install
    if %ERRORLEVEL% NEQ 0 (
        echo ❌ Failed to install dependencies!
        echo.
        echo To set up frontend tests manually:
        echo 1. cd tests/frontend
        echo 2. npm install
        echo 3. Run tests with: npm test
        exit /b 1
    )
)

echo ✅ Dependencies ready!
echo.

if %QUIET_MODE% EQU 1 (
    echo Running Frontend Unit Tests...
    npm test -- --silent
) else (
    echo Running Frontend Unit Tests...
    npm test
)

if %ERRORLEVEL% EQU 0 (
    echo.
    echo ✅ Frontend unit tests passed!
    
    if %QUIET_MODE% EQU 0 (
        echo.
        echo Frontend Tests covered:
        echo • SPA Navigation logic
        echo • Authentication state management
        echo • Template loading and caching
        echo • Global navigation updates
        echo.
        
        echo To run E2E tests manually:
        echo npm run test:e2e
        echo.
        echo Note: E2E tests require the backend server to be running
        echo Start server with: cargo run
    )
) else (
    echo ❌ Frontend tests failed!
    echo.
    echo Note: If this is the first run, you may need to:
    echo 1. Install dependencies: npm install
    echo 2. Check test configuration in package.json
    exit /b 1
)

if "%1" NEQ "--quiet" pause