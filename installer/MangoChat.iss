#define MyAppName "Mango Chat"
#ifndef MyAppVersion
  #define MyAppVersion "0.0.0-dev"
#endif
#ifndef MyAppExe
  #define MyAppExe "..\\target\\release\\mangochat.exe"
#endif
#ifndef BuildName
  #define BuildName ""
#endif

#if BuildName != ""
  #define OutputFile "MangoChat-Setup-" + MyAppVersion + "-" + BuildName
#else
  #define OutputFile "MangoChat-Setup-" + MyAppVersion
#endif

[Setup]
AppId={{8E220C8E-3F32-44A9-9C56-70A43F2EEA0D}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher=Mango Chat
DefaultDirName={localappdata}\Programs\MangoChat
DefaultGroupName=Mango Chat
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename={#OutputFile}
Compression=lzma
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\mangochat.exe
SetupIconFile=..\icons\mango.ico
MinVersion=10.0
; The in-app updater waits for the old process to exit before running this installer
; silently, then relaunches the app itself. Do not let Inno relaunch it a second time.
CloseApplications=force
RestartApplications=no
AppCopyright=Copyright (C) Kalyan Sattaluri
VersionInfoCompany=Mango Chat
VersionInfoProductName={#MyAppName}
VersionInfoProductTextVersion={#MyAppVersion}
#if Pos("-", MyAppVersion) == 0
VersionInfoVersion={#MyAppVersion}
VersionInfoProductVersion={#MyAppVersion}
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"

[Files]
Source: "{#MyAppExe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Mango Chat"; Filename: "{app}\mangochat.exe"
Name: "{autodesktop}\Mango Chat"; Filename: "{app}\mangochat.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\mangochat.exe"; Description: "Launch Mango Chat"; Flags: nowait postinstall skipifsilent



