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
ChangesAssociations=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "svgassoc"; Description: "{cm:AssocFileExtension,Ductus,.svg}"

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Registry]
; Ductus pages: Ductus shows up under "Open with" for .svg files and in Settings › Default apps.
; Windows asks once which app to keep; it doesn't let installers take over a file type.
Root: HKA; Subkey: "Software\Classes\Ductus.Page"; ValueType: string; ValueName: ""; ValueData: "Ductus page"; Flags: uninsdeletekey; Tasks: svgassoc
Root: HKA; Subkey: "Software\Classes\Ductus.Page\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\ductus.exe,0"; Tasks: svgassoc
Root: HKA; Subkey: "Software\Classes\Ductus.Page\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\ductus.exe"" ""%1"""; Tasks: svgassoc
Root: HKA; Subkey: "Software\Classes\.svg\OpenWithProgids"; ValueType: string; ValueName: "Ductus.Page"; ValueData: ""; Flags: uninsdeletevalue; Tasks: svgassoc
Root: HKA; Subkey: "Software\Classes\Applications\ductus.exe"; Flags: uninsdeletekey; Tasks: svgassoc
Root: HKA; Subkey: "Software\Classes\Applications\ductus.exe\SupportedTypes"; ValueType: string; ValueName: ".svg"; ValueData: ""; Tasks: svgassoc

[Icons]
Name: "{autoprograms}\Ductus"; Filename: "{app}\ductus.exe"
Name: "{autodesktop}\Ductus"; Filename: "{app}\ductus.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\ductus.exe"; Description: "{cm:LaunchProgram,Ductus}"; Flags: nowait postinstall skipifsilent
; Silent runs are updates from inside Ductus: start it again.
Filename: "{app}\ductus.exe"; Flags: nowait; Check: WizardSilent
