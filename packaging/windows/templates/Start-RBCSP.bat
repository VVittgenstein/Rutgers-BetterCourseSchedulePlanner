@echo off
setlocal

set "RBCSP_ROOT=%~dp0"

if not exist "%RBCSP_ROOT%RBCSP.exe" (
  echo RBCSP.exe is missing from this folder.
  echo Extract the complete RBCSP archive into one writable folder, then try again.
  pause
  exit /b 2
)

pushd "%RBCSP_ROOT%" >nul 2>&1
if errorlevel 1 (
  echo RBCSP could not open its package folder.
  echo Move the complete extracted folder to a writable location, then try again.
  pause
  exit /b 3
)

"%RBCSP_ROOT%RBCSP.exe"
set "RBCSP_EXIT_CODE=%ERRORLEVEL%"

popd >nul

if not "%RBCSP_EXIT_CODE%"=="0" (
  echo.
  echo RBCSP stopped with exit code %RBCSP_EXIT_CODE%.
  echo See QUICKSTART.md for troubleshooting guidance.
  pause
)

exit /b %RBCSP_EXIT_CODE%
