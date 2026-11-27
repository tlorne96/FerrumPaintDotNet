; Ferrum Paint - Windows installer (Inno Setup 6)
; Build:  ISCC.exe /DAppVersion=0.1.5 /DExePath=path\to\ferrum-paint.exe packaging\windows\ferrum-paint.iss
; Output: dist\FerrumPaint-<version>-Setup.exe

#ifndef AppVersion
  #define AppVersion "0.1.5"
#endif
#ifndef ExePath
  #define ExePath "..\..\target\release\ferrum-paint.exe"
#endif

[Setup]
AppId={{6F3C2A51-8E2B-4C7D-9A0E-FE22A1D3B9C4}
AppName=Ferrum Paint
AppVersion={#AppVersion}
AppVerName=Ferrum Paint {#AppVersion}
AppPublisher=Tim Lorne
AppPublisherURL=https://github.com/tlorne96/FerrumPaintDotNet
AppSupportURL=https://github.com/tlorne96/FerrumPaintDotNet/issues
AppUpdatesURL=https://github.com/tlorne96/FerrumPaintDotNet/releases
DefaultDirName={autopf}\Ferrum Paint
DefaultGroupName=Ferrum Paint
DisableProgramGroupPage=yes
; Standard choice: "Install for all users" (needs admin) or "Install just for me" (no admin).
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesAssociations=yes
CloseApplications=yes
WizardStyle=modern
SetupIconFile=..\icons\ferrum-paint.ico
UninstallDisplayIcon={app}\ferrum-paint.exe
UninstallDisplayName=Ferrum Paint
OutputDir=..\..\dist
OutputBaseFilename=FerrumPaint-{#AppVersion}-Setup
Compression=lzma2/max
SolidCompression=yes
VersionInfoVersion={#AppVersion}
VersionInfoProductName=Ferrum Paint

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "associate"; Description: "Open .ferrum images with Ferrum Paint"; GroupDescription: "File types:"

[Files]
Source: "{#ExePath}"; DestDir: "{app}"; DestName: "ferrum-paint.exe"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\..\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; DestName: "THIRD_PARTY_NOTICES.txt"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Ferrum Paint"; Filename: "{app}\ferrum-paint.exe"
Name: "{autodesktop}\Ferrum Paint"; Filename: "{app}\ferrum-paint.exe"; Tasks: desktopicon

[Registry]
; .ferrum file association (HKA = per-user or all-users, matching the install mode)
Root: HKA; Subkey: "Software\Classes\.ferrum"; ValueType: string; ValueName: ""; ValueData: "FerrumPaint.Image"; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\.ferrum\OpenWithProgids"; ValueType: string; ValueName: "FerrumPaint.Image"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\FerrumPaint.Image"; ValueType: string; ValueName: ""; ValueData: "Ferrum Paint image"; Flags: uninsdeletekey; Tasks: associate
Root: HKA; Subkey: "Software\Classes\FerrumPaint.Image\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\ferrum-paint.exe"",0"; Tasks: associate
Root: HKA; Subkey: "Software\Classes\FerrumPaint.Image\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\ferrum-paint.exe"" ""%1"""; Tasks: associate
; "Open with" for common image types (does not take over their default app)
Root: HKA; Subkey: "Software\Classes\Applications\ferrum-paint.exe"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "Ferrum Paint"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\ferrum-paint.exe\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\ferrum-paint.exe"" ""%1"""
Root: HKA; Subkey: "Software\Classes\Applications\ferrum-paint.exe\SupportedTypes"; ValueType: string; ValueName: ".png"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\ferrum-paint.exe\SupportedTypes"; ValueType: string; ValueName: ".jpg"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\ferrum-paint.exe\SupportedTypes"; ValueType: string; ValueName: ".jpeg"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\ferrum-paint.exe\SupportedTypes"; ValueType: string; ValueName: ".bmp"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\ferrum-paint.exe\SupportedTypes"; ValueType: string; ValueName: ".gif"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\ferrum-paint.exe\SupportedTypes"; ValueType: string; ValueName: ".webp"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\ferrum-paint.exe\SupportedTypes"; ValueType: string; ValueName: ".tif"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\ferrum-paint.exe\SupportedTypes"; ValueType: string; ValueName: ".tga"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\ferrum-paint.exe\SupportedTypes"; ValueType: string; ValueName: ".ferrum"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\ferrum-paint.exe\SupportedTypes"; ValueType: string; ValueName: ".pdn"; ValueData: ""

[Run]
Filename: "{app}\ferrum-paint.exe"; Description: "{cm:LaunchProgram,Ferrum Paint}"; Flags: nowait postinstall skipifsilent
