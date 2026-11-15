@echo off
cd /d "%~dp0"

echo ============================================
echo   外置转内置 - build
echo ============================================

echo [1/3] cargo build --release
cargo build --release
if errorlevel 1 (
    echo BUILD FAILED
    pause
    exit /b 1
)

echo [2/3] prepare dist...
if exist dist rmdir /s /q dist
mkdir dist

echo [3/3] copy files...
copy /y "target\release\外置转内置.exe" "dist\" >nul
copy /y "README.md" "dist\" >nul
copy /y "示例配置.ini" "dist\外置转内置.ini" >nul

echo.
echo ============================================
echo   DONE! Output in dist\
echo ============================================
dir /b dist
pause