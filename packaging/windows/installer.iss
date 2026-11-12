; Inno Setup script for Macropad Remapper.
; Built in CI with:  ISCC /DAppVersion=0.2.0 /DSourceDir=<staging dir> /DOutputDir=<dir> installer.iss

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\target\release"
#endif
#ifndef OutputDir
  #define OutputDir "..\..\dist"
#endif

[Setup]
AppId={{E9AF5AE4-D4B5-4F0F-93C0-2FB29D086ED4}
AppName=Macropad Remapper
AppVersion={#AppVersion}
AppVerName=Macropad Remapper {#AppVersion}
AppPublisher=jboneng
AppPublisherURL=https://github.com/jboneng/macropad-remapper
AppSupportURL=https://github.com/jboneng/macropad-remapper/issues
AppUpdatesURL=https://github.com/jboneng/macropad-remapper/releases
DefaultDirName={autopf}\Macropad Remapper
DefaultGroupName=Macropad Remapper
DisableProgramGroupPage=yes
; Per-user install by default (no admin needed); users can choose all-users in the dialog.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=macropad-remapper-{#AppVersion}-windows-x64-setup
SetupIconFile=..\icons\icon.ico
UninstallDisplayIcon={app}\macropad-remapper-gui.exe
LicenseFile=..\..\LICENSE-MIT
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\macropad-remapper-gui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\macropad-remapper.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\user-guide.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\example-profile.yaml"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Macropad Remapper"; Filename: "{app}\macropad-remapper-gui.exe"
Name: "{group}\User guide"; Filename: "https://github.com/jboneng/macropad-remapper/blob/main/docs/user-guide.md"
Name: "{group}\{cm:UninstallProgram,Macropad Remapper}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Macropad Remapper"; Filename: "{app}\macropad-remapper-gui.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\macropad-remapper-gui.exe"; Description: "{cm:LaunchProgram,Macropad Remapper}"; Flags: nowait postinstall skipifsilent
