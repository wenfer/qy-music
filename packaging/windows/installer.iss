#define MyAppName "聆风 (Lingfeng)"
#define MyAppExeName "lingfeng.exe"
#define MyAppPublisher "wenfer"
#define MyAppURL "https://github.com/wenfer/qy-music"

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

#ifndef SourceDir
  #define SourceDir ProjectRoot + "\dist\lingfeng-windows-x86_64"
#endif

#ifndef OutputDir
  #define OutputDir ProjectRoot + "\artifacts"
#endif

#ifndef OutputBaseFileName
  #define OutputBaseFileName "lingfeng-" + AppVersion + "-windows-x86_64-setup"
#endif

#ifndef ArchMode
  #define ArchMode "x64compatible"
#endif

#ifndef ProjectRoot
  #define ProjectRoot "..\.."
#endif

[Setup]
AppId={{5E3F92C1-30A5-4E7B-8DC4-5D813F93E46B}
AppName={#MyAppName}
AppVersion={#AppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\Lingfeng
DisableProgramGroupPage=yes
LicenseFile={#ProjectRoot}\LICENSE
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBaseFileName}
SetupIconFile={#ProjectRoot}\assets\icons\icon.ico
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode={#ArchMode}
UninstallDisplayIcon={app}\icon.ico

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "{#SourceDir}\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ProjectRoot}\assets\icons\icon.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ProjectRoot}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ProjectRoot}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\icon.ico"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon; IconFilename: "{app}\icon.ico"

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
