cd server
cargo build
if errorlevel 1 (
  exit /b %errorlevel%
)
cd ..
