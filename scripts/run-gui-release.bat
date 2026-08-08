@echo off
REM ============================================================================
REM  Operon GUI Release Launcher
REM  Builds and runs the Operon Graphical User Interface in Release Mode
REM ============================================================================

setlocal enabledelayedexpansion

REM Configure build environment for whisper-rs / bindgen / MSVC / CMake
set "PATH=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin;D:\tools\Android\sdk\ndk\29.0.14033849\toolchains\llvm\prebuilt\windows-x86_64\bin;%PATH%"
set "LIBCLANG_PATH=D:\tools\Android\sdk\ndk\29.0.14033849\toolchains\llvm\prebuilt\windows-x86_64\bin"
set "BINDGEN_EXTRA_CLANG_ARGS=--target=x86_64-pc-windows-msvc -I"C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\ucrt" -I"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35207\include" -I"C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\um""

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

REM Build and run the GUI in release mode
echo ============================================================================
echo   Building and launching Operon GUI (Release Mode - Optimized)...
echo ============================================================================
echo.

cargo run --release --bin operon-gui

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
