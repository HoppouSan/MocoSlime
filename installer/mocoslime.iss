; Mocoslime Windows installer (Inno Setup 6).
; Build first:
;   cargo build --release
;   flutter build windows --release
;   powershell -File installer\stage.ps1
; Then compile this script with ISCC.

#define MyAppName "Mocoslime"
#define MyAppVersion "0.1.1"
#define MyAppPublisher "Mocoslime Contributors"
#define MyAppURL "https://github.com/HoppouSan/MocoSlime/"
#define MyAppExeName "Mocoslime.exe"

[Setup]
AppId={{8E2F4A1C-3B7D-4E5A-9C1F-A1B2C3D4E5F6}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\Mocoslime
DefaultGroupName=Mocoslime
AllowNoIcons=yes
LicenseFile=..\LICENSE
OutputDir=output
OutputBaseFilename=Mocoslime-{#MyAppVersion}-windows-x64
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Windows 10 (Build 10240+) and Windows 11 (reports as 10.0.22000+).
; Older systems lack the WinRT BLE APIs the app needs.
MinVersion=10.0.10240
PrivilegesRequired=lowest
UninstallDisplayName={#MyAppName}
; No bundled VC++ redist needed: Rust + Flutter are self-contained.

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "autostart"; Description: "Start Mocoslime with Windows"; GroupDescription: "Autostart"; Flags: unchecked

[Files]
Source: "stage\Mocoslime.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "stage\mocoslime_ffi.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "stage\data\*"; DestDir: "{app}\data"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "stage\flutter_windows.dll"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "stage\*.dll"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\LICENSE"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "..\NOTICE"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "..\THIRD_PARTY_LICENSES.md"; DestDir: "{app}\licenses"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; Optional autostart (HKCU, no admin required).
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Mocoslime"; ValueData: """{app}\{#MyAppExeName}"" --minimized"; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{app}\logs"
