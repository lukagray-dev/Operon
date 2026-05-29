@echo off
REM ============================================================================
REM  Operon Landing Page Launcher
REM  Builds and runs the Next.js marketing landing page
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
echo   Marketing Landing Page Launcher (Next.js)
echo ============================================================================
echo.

REM Navigate to landing directory
echo [*] Navigating to landing directory...
cd /d "%~dp0..\landing"

if errorlevel 1 (
    color 0C
    echo [ERROR] Failed to navigate to landing directory!
    echo.
    pause
    exit /b 1
)

echo [OK] Directory: %CD%
echo.

REM Check if Node.js is installed
echo [*] Checking Node.js runtime...
node --version >nul 2>&1

if errorlevel 1 (
    color 0C
    echo [ERROR] Node.js is not installed! Please download it from https://nodejs.org/
    echo.
    pause
    exit /b 1
)

echo [OK] Node.js detected:
node --version
echo.

REM Check if node_modules exists, if not install dependencies
if not exist "node_modules\" (
    echo [*] node_modules folder not found. Running npm install...
    call npm install
    if errorlevel 1 (
        color 0C
        echo [ERROR] Failed to install dependencies! Please check your network.
        echo.
        pause
        exit /b 1
    )
    echo [OK] Dependencies installed successfully.
    echo.
)

REM Start Next.js development server
echo ============================================================================
echo   Starting Next.js development server...
echo   Open http://localhost:3000 in your browser.
echo ============================================================================
echo.

call npm run dev

REM Capture exit code
set EXIT_CODE=%errorlevel%

REM Display completion message
echo.
echo ============================================================================

if %EXIT_CODE% equ 0 (
    color 0A
    echo   Development server stopped successfully
) else (
    color 0C
    echo   Development server exited with error code: %EXIT_CODE%
)

echo ============================================================================
echo.
echo Press any key to close this window...
pause >nul

endlocal
exit /b %EXIT_CODE%
