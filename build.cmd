cd server
call build.cmd
cd ..
IF DEFINED %APPVEYOR% (exit 0)
