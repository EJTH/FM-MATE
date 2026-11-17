@echo off
REM Generate app icons from 512x512.png using ffmpeg

setlocal enabledelayedexpansion

set SOURCE=src-tauri\icons\512x512.png
set ICONS_DIR=src-tauri\icons

REM Check if source file exists
if not exist "%SOURCE%" (
    echo Error: %SOURCE% not found
    exit /b 1
)

REM Create the required sizes
echo Generating icon sizes from %SOURCE%...

ffmpeg -i "%SOURCE%" -vf scale=32:32 "%ICONS_DIR%\32x32.png" -y
if errorlevel 1 goto error

ffmpeg -i "%SOURCE%" -vf scale=128:128 "%ICONS_DIR%\128x128.png" -y
if errorlevel 1 goto error

ffmpeg -i "%SOURCE%" -vf scale=256:256 "%ICONS_DIR%\128x128@2x.png" -y
if errorlevel 1 goto error

echo Icon generation complete!
exit /b 0

:error
echo Error generating icons
exit /b 1
