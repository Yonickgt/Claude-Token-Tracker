; Inno Setup script. Built by CI: iscc /DAppVersion=0.2.0 widget\installer.iss  ->  widget\Output\ClaudeTokenWidget-0.2.0-Setup.exe
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
[Setup]
AppId={{6B1D2C57-4F0A-4E55-9A52-CLAUDETOKENW}
AppName=Claude Token Widget
AppVersion={#AppVersion}
AppPublisher=Yonickgt
DefaultDirName={localappdata}\ClaudeTokenWidget
DefaultGroupName=Claude Token Widget
PrivilegesRequired=lowest
OutputBaseFilename=ClaudeTokenWidget-{#AppVersion}-Setup
SetupIconFile=assets\icon.ico
UninstallDisplayIcon={app}\token-widget.exe
Compression=lzma2
SolidCompression=yes
CloseApplications=yes
RestartApplications=yes

[Files]
Source: "target\release\token-widget.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Claude Token Widget"; Filename: "{app}\token-widget.exe"
Name: "{userdesktop}\Claude Token Widget"; Filename: "{app}\token-widget.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Run]
Filename: "{app}\token-widget.exe"; Description: "Launch Claude Token Widget"; Flags: nowait postinstall skipifsilent
