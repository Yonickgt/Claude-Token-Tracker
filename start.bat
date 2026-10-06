@echo off
rem Starts the API (127.0.0.1:8787) and the dashboard (http://localhost:3000).
start "token-tracker-api" /min python "%~dp0server.py"
cd /d "%~dp0web"
if not exist .next call npm run build
call npm start
