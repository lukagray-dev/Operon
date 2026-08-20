@echo off
setlocal
cd /d "%~dp0\.."
echo Updating Operon Model Catalog...
python "%~dp0update-model-catalog.py"
if %ERRORLEVEL% equ 0 (
    echo Model catalog updated successfully.
) else (
    echo Failed to update model catalog.
)
pause
