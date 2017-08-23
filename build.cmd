cd server
call build.cmd
if errorlevel 1 (
  exit /b %errorlevel%
)
cd ..
exit 0
