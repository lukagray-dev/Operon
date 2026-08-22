@echo off
REM ============================================================================
REM  Operon VS Code Extension Release Builder
REM  Compiles TypeScript frontend and builds optimized Release Rust bridge
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
echo   VS Code Extension Builder (RELEASE MODE - OPTIMIZED)
echo ============================================================================
echo.

REM Determine script and repository root paths
set "SCRIPT_DIR=%~dp0"
set "REPO_ROOT=%SCRIPT_DIR%.."
cd /d "%REPO_ROOT%"

REM Check if Cargo (Rust toolchain) is installed
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

REM Check if Node / npm is installed
echo [*] Checking Node.js environment...
call npx --version >nul 2>&1
if errorlevel 1 (
    color 0C
    echo [ERROR] Node.js / npx not found! Please install Node.js from https://nodejs.org/
    echo.
    pause
    exit /b 1
)
echo [OK] Node.js environment detected
echo.

REM 1. Compile TypeScript UI
echo ============================================================================
echo   [1/3] Compiling TypeScript Frontend...
echo ============================================================================
echo.
call npx --prefix vscode\extension tsc -p vscode\extension\tsconfig.json
if errorlevel 1 (
    color 0C
    echo.
    echo [ERROR] TypeScript compilation failed!
    echo.
    pause
    exit /b 1
)
echo [OK] TypeScript compilation succeeded.
echo.

REM 2. Build Rust Native JSON-RPC Bridge in Release Mode
echo ============================================================================
echo   [2/3] Building Native Rust Bridge (operon-vscode-bridge --release)...
echo ============================================================================
echo.
cargo build --release -p operon-vscode-bridge
if errorlevel 1 (
    color 0C
    echo.
    echo [ERROR] Cargo release build failed!
    echo.
    pause
    exit /b 1
)
echo [OK] Optimized Rust bridge binary built successfully.
echo.

REM 3. Package binary into extension bin directory
echo ============================================================================
echo   [3/3] Packaging release binary into extension distribution...
echo ============================================================================
echo.
if not exist "vscode\extension\bin" (
    mkdir "vscode\extension\bin"
)

copy /Y "target\release\operon-vscode-bridge.exe" "vscode\extension\bin\operon-vscode-bridge.exe" >nul
if errorlevel 1 (
    color 0E
    echo [WARNING] Could not copy binary to vscode\extension\bin, extension will load from target\release\
) else (
    echo [OK] Copied optimized operon-vscode-bridge.exe to vscode\extension\bin\
)

REM Display completion message
echo.
echo ============================================================================
color 0A
echo   Operon VS Code Extension release build completed successfully!
echo   Target Output: vscode\extension\
echo ============================================================================
echo.
echo Press any key to close this window...
pause >nul

endlocal
exit /b 0
