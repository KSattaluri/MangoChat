#define MyAppName "Jarvis"
#ifndef MyAppVersion
  #define MyAppVersion "0.0.0-dev"
#endif
#ifndef MyAppExe
  #define MyAppExe "..\\target\\release\\jarvis.exe"
#endif
#ifndef BuildName
  #define BuildName ""
#endif

#if BuildName != ""
  #define OutputFile "Jarvis-Setup-" + MyAppVersion + "-" + BuildName
#else
  #define OutputFile "Jarvis-Setup-" + MyAppVersion
#endif

[Setup]
AppId={{8E220C8E-3F32-44A9-9C56-70A43F2EEA0D}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher=Jarvis
DefaultDirName={localappdata}\Programs\Jarvis
DefaultGroupName=Jarvis
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename={#OutputFile}
Compression=lzma
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\jarvis.exe
SetupIconFile=..\\icons\\icon.ico

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"

[Files]
Source: "{#MyAppExe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Jarvis"; Filename: "{app}\jarvis.exe"
Name: "{autodesktop}\Jarvis"; Filename: "{app}\jarvis.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\jarvis.exe"; Description: "Launch Jarvis"; Flags: nowait postinstall skipifsilent

