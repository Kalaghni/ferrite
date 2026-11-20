; FERRITE installer for Windows (Inno Setup 6).
;
; Build it with packaging\build-installer.ps1, or by hand:
;   ISCC /DAppVersion=0.7.0 /DSourceDir=C:\path\to\staged\files packaging\ferrite.iss
; SourceDir must hold ferrite.exe, ferrite-server.exe, README.txt,
; Quantico-OFL.txt and start-server.bat.
;
; It installs for the current user only, under %LOCALAPPDATA%\Programs, and
; never asks for administrator rights. That is on purpose: the game keeps
; its settings (ferrite-client.txt) and a hosted server's player records
; (ferrite-stats.txt) beside the executable, so the folder must be writable.
; Those files are left alone by upgrades and by the uninstaller.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\target\release"
#endif

[Setup]
AppId={{6F0C1B52-7A3D-4E6B-9C41-FE221A0D0007}
AppName=FERRITE
AppVersion={#AppVersion}
AppVerName=FERRITE {#AppVersion}
AppPublisher=Kalaghni
AppPublisherURL=https://github.com/Kalaghni/ferrite
AppSupportURL=https://github.com/Kalaghni/ferrite/issues
AppUpdatesURL=https://github.com/Kalaghni/ferrite/releases
DefaultDirName={localappdata}\Programs\FERRITE
DefaultGroupName=FERRITE
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=ferrite-v{#AppVersion}-windows-x64-setup
SetupIconFile=ferrite.ico
UninstallDisplayIcon={app}\ferrite.exe
UninstallDisplayName=FERRITE
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\ferrite.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\ferrite-server.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\ferrite-forge-mcp.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\README.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\Quantico-OFL.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\start-server.bat"; DestDir: "{app}"; Flags: ignoreversion

; What the launcher's updater leaves beside the game: the previous version
; it kept, files waiting to be deleted, and a one-line notice.
[UninstallDelete]
Type: filesandordirs; Name: "{app}\ferrite-previous"
Type: filesandordirs; Name: "{app}\ferrite-staging"
Type: filesandordirs; Name: "{app}\ferrite-trash"
Type: files; Name: "{app}\ferrite-update-notice.txt"

[Icons]
Name: "{group}\FERRITE"; Filename: "{app}\ferrite.exe"; WorkingDir: "{app}"
Name: "{group}\FERRITE dedicated server"; Filename: "{app}\start-server.bat"; WorkingDir: "{app}"; IconFilename: "{app}\ferrite.exe"
Name: "{group}\Read me"; Filename: "{app}\README.txt"
Name: "{group}\Uninstall FERRITE"; Filename: "{uninstallexe}"
Name: "{autodesktop}\FERRITE"; Filename: "{app}\ferrite.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\ferrite.exe"; Description: "{cm:LaunchProgram,FERRITE}"; WorkingDir: "{app}"; Flags: nowait postinstall skipifsilent
