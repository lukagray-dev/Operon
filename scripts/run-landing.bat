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


echo Press any key to close this window...
pause >nul

endlocal
exit /b %EXIT_CODE%
