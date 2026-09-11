@echo off
setlocal

echo Installing poqi from the latest GitHub release...

if /I "%POQI_INSTALLER_DRY_RUN%"=="1" (
  powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1"
  exit /b %errorlevel%
)

if /I "%POQI_INSTALLER_DRY_RUN%"=="true" (
  powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1"
  exit /b %errorlevel%
)

powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://github.com/poqi-cli/poqi/releases/latest/download/install.ps1 | iex"

if errorlevel 1 (
  echo.
  echo poqi installation failed.
  pause
  exit /b 1
)

echo.
echo poqi installation finished.
echo Open a new terminal and run: poqi --help
pause
