@echo off
title LGD FTP DATA SYNC
cd /d "%~dp0"
if exist "lgd-ftp-data-sync.exe" goto run
if exist "..\lgd-ftp-data-sync.exe" cd /d "%~dp0.."
:run
echo Starting LGD FTP DATA SYNC...
echo Log file: %APPDATA%\com.diamonds.ftpuploader\startup.log
echo.
lgd-ftp-data-sync.exe
echo.
echo App exited with code %ERRORLEVEL%
echo If the window never appeared, install WebView2:
echo https://developer.microsoft.com/microsoft-edge/webview2/
echo.
pause
