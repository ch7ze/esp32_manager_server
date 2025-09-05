@echo off
echo Starting WebSocket Debug Session...
echo.

echo Setting debug logging...
set RUST_LOG=debug

echo Starting server...
echo Press Ctrl+C to stop server
echo.

cargo run