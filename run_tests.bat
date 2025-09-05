@echo off
echo ========================================
echo Drawing App - Complete Test Suite
echo ========================================

set TOTAL_TESTS=0
set PASSED_TESTS=0

echo.
echo Building project...
cargo build --quiet

if %ERRORLEVEL% NEQ 0 (
    echo ❌ Build failed!
    pause
    exit /b 1
)

echo ✅ Build successful!
echo.

echo [1/2] Running Backend Integration Tests...
echo ========================================
cargo test --test auth_integration --quiet
if %ERRORLEVEL% EQU 0 (
    echo ✅ Integration tests passed (9 tests)
    set /a PASSED_TESTS+=9
) else (
    echo ❌ Integration tests failed!
)
set /a TOTAL_TESTS+=9

echo.
echo [2/2] Running Backend Unit Tests...
echo ========================================
cargo test --test auth_unit --quiet
if %ERRORLEVEL% EQU 0 (
    echo ✅ Unit tests passed (11 tests)
    set /a PASSED_TESTS+=11
) else (
    echo ❌ Unit tests failed!
)
set /a TOTAL_TESTS+=11

echo.
echo ========================================
echo Test Results Summary
echo ========================================
echo Total Tests: %TOTAL_TESTS%
echo Passed: %PASSED_TESTS%

if %PASSED_TESTS% EQU %TOTAL_TESTS% (
    echo.
    echo 🎉 All tests passed successfully!
    echo.
    echo ✅ Test Coverage:
    echo.
    echo 📊 Integration Tests (9 tests):
    echo   • User Registration ^(success, duplicates, cookies^)
    echo   • User Login ^(success, wrong password^)
    echo   • JWT Token Validation ^(with/without cookies^)
    echo   • User Logout ^(token invalidation^)
    echo   • Complete Auth Flow ^(register → logout → login^)
    echo.
    echo 🔧 Unit Tests (11 tests):
    echo   • JWT Creation and Validation
    echo   • Password Hashing ^(bcrypt, uniqueness^)
    echo   • User Struct Operations
    echo   • Cookie Helper Functions
    echo   • Auth Response Serialization
    echo.
    echo 🔗 Test Structure:
    echo   tests/auth_integration.rs - HTTP API Tests
    echo   tests/auth_unit.rs - Module Unit Tests
    echo   tests/frontend/ - Frontend Tests ^(prepared^)
    echo   tests/scripts/ - Test Automation
    echo.
) else (
    echo.
    echo ⚠️  %FAILED_TESTS% tests failed. Check output above.
    echo.
)

echo Press any key to exit...
pause >nul