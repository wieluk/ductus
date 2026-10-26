; Built by package.ps1. Per-user by default, so no administrator rights are needed.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{6B0E6C9A-3F1D-4D7B-9A8E-2C5F1D0B7E41}
AppName=Ductus
AppVersion={#AppVersion}
AppPublisher=Ductus
DefaultDirName={autopf}\Ductus
DefaultGroupName=Ductus
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile={#SourceDir}\LICENSE.txt
OutputDir={#OutputDir}
OutputBaseFilename=Ductus-{#AppVersion}-windows-x64-setup
SetupIconFile=..\..\data\icons\app.ductus.Ductus.ico
UninstallDisplayIcon={app}\ductus.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Ductus"; Filename: "{app}\ductus.exe"
Name: "{autodesktop}\Ductus"; Filename: "{app}\ductus.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\ductus.exe"; Description: "{cm:LaunchProgram,Ductus}"; Flags: nowait postinstall skipifsilent
; Silent runs are updates from inside Ductus: start it again.
Filename: "{app}\ductus.exe"; Flags: nowait; Check: WizardSilent
