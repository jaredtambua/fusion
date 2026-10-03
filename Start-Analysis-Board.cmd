@echo off
cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
echo Building and starting the Fusion analysis board...
cargo run --release --bin analysis_board
if errorlevel 1 (
  echo.
  echo Launch failed. Check the error above. Rust and the Windows C++ build tools are required.
  pause
)

