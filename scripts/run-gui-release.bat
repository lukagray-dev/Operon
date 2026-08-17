@echo off
REM ============================================================================
REM  Operon GUI Release Launcher
REM  Builds and runs the Operon Graphical User Interface in Release Mode
REM ============================================================================

setlocal enabledelayedexpansion

REM Clear screen and display banner
cls
color 0B
echo.
echo     ____                               
echo    / __ \____  ___  _________  ____    
echo   / / / / __ \/ _ \/ ___/ __ \/ __ \   
echo  / /_/ / /_/ /  __/ /  / /_/ / / / /   
echo  \____/ .___/\___/_/   \____/_/ /_/    
echo      /_/                               
echo.
echo ============================================================================
echo   Graphical User Interface Launcher (RELEASE MODE)
echo ============================================================================
echo.

REM Check if Cargo is installed
echo [*] Checking Rust toolchain...
cargo --version >nul 2>&1

if errorlevel 1 (
    color 0C
    echo [ERROR] Cargo not found! Please install Rust from https://rustup.rs/
    echo.
    pause
    exit /b 1
)

echo [OK] Rust toolchain detected
echo.

REM Compile TypeScript UI
echo [*] Compiling TypeScript frontend...
call npx --prefix gui tsc -p gui\tsconfig.json

REM Build and run the GUI in release mode
echo ============================================================================
echo   Building and launching Operon GUI (Release Mode - Optimized)...
echo ============================================================================
echo.

cargo run --release -p operon-gui

REM Capture exit code
set EXIT_CODE=%errorlevel%

REM Display completion message
echo.
echo ============================================================================

if %EXIT_CODE% equ 0 (
    color 0A
    echo   Operon GUI exited successfully
) else (
    color 0C
    echo   Operon GUI exited with error code: %EXIT_CODE%
)

echo ============================================================================
echo.
echo Press any key to close this window...
pause >nul

endlocal
exit /b %EXIT_CODE%
