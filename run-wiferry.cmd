@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0run-wiferry.ps1" %*
exit /b %ERRORLEVEL%
