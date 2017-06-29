cd server
cargo build
if errorlevel 1 (
  exit /b %errorlevel%
)
start /B "" cargo run
cargo test
if errorlevel 1 (
  taskkill /F /IM server.exe
  exit /b %errorlevel%
)
taskkill /F /IM server.exe
cd ..
