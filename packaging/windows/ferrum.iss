; Build only through scripts/release.ps1. Compiling this file never installs Ferrum.
#ifndef AppVersion
  #error AppVersion must come from Cargo.toml via scripts/release.ps1
#endif
#ifndef NumericVersion
  #error NumericVersion must come from the built executable
#endif
#ifndef PayloadDir
  #error PayloadDir must be a verified release staging directory
#endif
#ifndef ReleaseDir
  #error ReleaseDir must be set
#endif

[Setup]
AppId={{35C83E18-76F3-4F51-A9C6-2E8C071833B5}
AppName=Ferrum
AppVersion={#AppVersion}
AppVerName=Ferrum v{#AppVersion}
AppPublisher=Ferrum contributors
DefaultDirName={localappdata}\Programs\Ferrum
DefaultGroupName=Ferrum
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
UsePreviousAppDir=yes
DisableDirPage=yes
DisableProgramGroupPage=yes
Uninstallable=yes
UninstallDisplayName=Ferrum
UninstallDisplayIcon={app}\ferrum.exe
OutputDir={#ReleaseDir}
OutputBaseFilename=Ferrum-v{#AppVersion}-windows-x64-setup
SetupIconFile=..\..\resources\ferrum.ico
VersionInfoVersion={#NumericVersion}
VersionInfoTextVersion={#AppVersion}
VersionInfoProductName=Ferrum
VersionInfoProductVersion={#NumericVersion}
VersionInfoProductTextVersion={#AppVersion}
Compression=lzma2/normal
SolidCompression=yes
AppMutex=Local\SpotlightLauncherPrototype.35C83E18-76F3-4F51-A9C6-2E8C071833B5
SetupMutex=Local\Ferrum.Setup.35C83E18-76F3-4F51-A9C6-2E8C071833B5
CloseApplications=no
RestartApplications=no
AlwaysRestart=no
RestartIfNeededByRun=no
LicenseFile={#PayloadDir}\LICENSE.txt
InfoBeforeFile={#PayloadDir}\INSTALLATION.txt
#ifdef SignedRelease
SignTool=ferrum
SignedUninstaller=yes
#endif

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
; The binary comes first. No restartreplace/forced close or broad deletion rules.
Source: "{#PayloadDir}\ferrum.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\THIRD_PARTY_NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\README.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\INSTALLATION.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{userprograms}\Ferrum"; Filename: "{app}\ferrum.exe"; WorkingDir: "{app}"; IconFilename: "{app}\ferrum.exe"
Name: "{userdesktop}\Ferrum"; Filename: "{app}\ferrum.exe"; WorkingDir: "{app}"; IconFilename: "{app}\ferrum.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\ferrum.exe"; Description: "Launch Ferrum (starts hidden; use your shortcut)"; Flags: postinstall nowait skipifsilent unchecked

[Code]
const
  ProductMutex = 'Local\SpotlightLauncherPrototype.35C83E18-76F3-4F51-A9C6-2E8C071833B5';
  RunKey = 'Software\Microsoft\Windows\CurrentVersion\Run';

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  NeedsRestart := False;
  if CheckForMutexes(ProductMutex) then
    Result := 'Ferrum is running. Open Ferrum, select Exit, then retry. No application will be closed automatically. Cancel to leave the installation unchanged.';
end;

procedure RemoveVerifiedStartupValue(const ValueName: String);
var
  Value, Expected: String;
begin
  Expected := '"' + ExpandConstant('{app}\ferrum.exe') + '"';
  if RegQueryStringValue(HKCU64, RunKey, ValueName, Value) then
  begin
    // Only the exact quoted executable command for THIS installed directory.
    // A portable/development path, extra arguments or unfamiliar value is retained.
    if CompareText(Value, Expected) = 0 then
      if not RegDeleteValue(HKCU64, RunKey, ValueName) then
        RaiseException('Ferrum could not remove its verified startup entry. No files have been removed. Check permissions and retry uninstall.');
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    if CheckForMutexes(ProductMutex) then
      RaiseException('Ferrum was reopened. Select Exit in Ferrum, then retry uninstall. No application will be closed automatically.');
    RemoveVerifiedStartupValue('Ferrum');
    RemoveVerifiedStartupValue('SpotlightLauncherPrototype');
  end;
end;

// No [Registry] startup writes and no [UninstallDelete]. Inno removes only its
// logged program/shortcut files. AppData preferences, cache and documents remain.
