@echo off
echo ========================================
echo Drawing App - Complete Test Suite
echo ========================================

set TESTS_PASSED=0
set TESTS_FAILED=0

echo.
echo [1/3] Running Backend Integration Tests...
echo ========================================
call "%~dp0run-backend-tests.bat" --quiet
if %ERRORLEVEL% EQU 0 (
    echo [PASS] Backend tests passed
    set /a TESTS_PASSED+=1
) else (
    echo [FAIL] Backend tests failed
    set /a TESTS_FAILED+=1
)

echo.
echo [2/3] Running Backend Unit Tests...
echo ========================================
cd /d "%~dp0..\.."
cargo test --lib --quiet
if %ERRORLEVEL% EQU 0 (
    echo [PASS] Unit tests passed
    set /a TESTS_PASSED+=1
) else (
    echo [FAIL] Unit tests failed
    set /a TESTS_FAILED+=1
)

echo.
echo [3/3] Running Frontend Tests...
echo ========================================
call "%~dp0run-frontend-tests.bat" --quiet
if %ERRORLEVEL% EQU 0 (
    echo [PASS] Frontend tests passed
    set /a TESTS_PASSED+=1
) else (
    echo [FAIL] Frontend tests failed
    set /a TESTS_FAILED+=1
)

echo.
echo ========================================
echo Test Results Summary
echo ========================================
echo Passed: %TESTS_PASSED%
echo Failed: %TESTS_FAILED%

if %TESTS_FAILED% EQU 0 (
    echo.
    echo All tests passed successfully!
    echo.
    echo Test Coverage:
    echo • Backend Integration Tests ^(10 tests^)
    echo • Backend Unit Tests ^(Auth, JWT, Password^)
    echo • Frontend Unit Tests ^(SPA logic^)
    echo • E2E Tests ^(Browser automation^)
    echo.
    exit /b 0
) else (
    echo.
    echo [WARNING] Some tests failed. Please check the output above.
    echo.
    exit /b 1
)

pause