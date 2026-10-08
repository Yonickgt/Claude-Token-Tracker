# Developer install: builds the widget from source. Needs Git + Rust (https://rustup.rs).
#   irm https://raw.githubusercontent.com/Yonickgt/Claude-Token-Tracker/main/install.ps1 | iex
$ErrorActionPreference = "Stop"
foreach ($t in "git", "cargo") { if (-not (Get-Command $t -ErrorAction SilentlyContinue)) { throw "$t is not installed" } }
$dir = Join-Path $env:LOCALAPPDATA "ClaudeTokenTracker"
if (Test-Path "$dir\.git") { git -C $dir pull --ff-only } else { git clone https://github.com/Yonickgt/Claude-Token-Tracker.git $dir }
Push-Location "$dir\widget"; cargo build --release; Pop-Location
$exe = "$dir\widget\target\release\token-widget.exe"
$lnk = (New-Object -ComObject WScript.Shell).CreateShortcut("$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Claude Token Widget.lnk")
$lnk.TargetPath = $exe; $lnk.WorkingDirectory = Split-Path $exe; $lnk.Save()
Start-Process $exe
Write-Host "Installed. Update later by running this same command again."
