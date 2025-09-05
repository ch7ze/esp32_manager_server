@echo off
echo ========================================
echo Backend Tests - Drawing App
echo ========================================

set QUIET_MODE=0
if "%1"=="--quiet" set QUIET_MODE=1

echo.
echo Building backend...
cd /d "%~dp0..\.."

if %QUIET_MODE% EQU 1 (
    cargo build --quiet
) else (
    cargo build
)

if %ERRORLEVEL% NEQ 0 (
    echo [FAIL] Build failed!
    exit /b 1
)

echo [PASS] Build successful!
echo.

echo Running Backend Integration Tests...
echo.

if %QUIET_MODE% EQU 1 (
    cargo test backend::integration --quiet
) else (
    cargo test backend::integration
)

if %ERRORLEVEL% EQU 0 (
    echo.
    echo [PASS] All backend integration tests passed!
    echo.
    if %QUIET_MODE% EQU 0 (
        echo Integration Tests covered:
        echo • User Registration ^(success, duplicate, cookies^)
        echo • User Login ^(success, wrong password, nonexistent^)
        echo • JWT Token Validation ^(with/without cookies^)
        echo • User Logout ^(token invalidation^)
        echo • Complete Auth Flow ^(register → logout → login^)
        echo.
        echo Running Backend Unit Tests...
        cargo test backend::unit
        
        if %ERRORLEVEL% EQU 0 (
            echo.
            echo [PASS] All backend unit tests passed!
            echo.
            echo Unit Tests covered:
            echo • JWT Token creation and validation
            echo • Password hashing and verification
            echo • User struct functionality
            echo • Cookie helper functions
            echo • Auth response serialization
        )
    )
) else (
    echo [FAIL] Backend tests failed!
    exit /b 1
)

if "%1" NEQ "--quiet" pause