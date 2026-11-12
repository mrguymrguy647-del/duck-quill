; Duck Quill's setup: DuckQuill-<version>-setup.exe, made by Inno Setup
; (https://jrsoftware.org/isinfo.php) in release.yml:
;
;   iscc /DAppVersion=0.6.0 "/DSourceDir=..\dist\Duck Quill" release\installer.iss
;
; - Installs for your Windows account only, so it never asks for an
;   administrator: %LOCALAPPDATA%\Programs\Duck Quill.
; - Writes only to your account's part of the registry (HKCU): .quill and
;   .artapp open in Duck Quill; PNG gets "Open with -> Duck Quill" only.
; - Your settings, presets and autosaves (%APPDATA%\Duck Quill) and your
;   pictures are never touched, by installing over an older version or by
;   uninstalling.
; - Run with /VERYSILENT by Duck Quill's updater (it uses the tick boxes
;   chosen the first time).

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\dist\Duck Quill"
#endif
#ifndef OutputDir
  #define OutputDir "..\dist"
#endif

[Setup]
; Never change the AppId: Windows knows Duck Quill by it, and an update
; finds the version already installed by it.
AppId={{6E0F3C1A-9B2D-4C57-8E4A-D2B7F05A13C9}
AppName=Duck Quill
AppVersion={#AppVersion}
AppVerName=Duck Quill {#AppVersion}
AppPublisher=Duck Quill
AppPublisherURL=https://github.com/mrguymrguy647-del/duck-quill
AppSupportURL=https://github.com/mrguymrguy647-del/duck-quill
AppUpdatesURL=https://github.com/mrguymrguy647-del/duck-quill/releases
VersionInfoVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Duck Quill
DisableDirPage=auto
DisableProgramGroupPage=yes
DefaultGroupName=Duck Quill
PrivilegesRequired=lowest
UsedUserAreasWarning=no
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=DuckQuill-{#AppVersion}-setup
SetupIconFile=..\brand\duck-quill.ico
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
UninstallDisplayIcon={app}\DuckQuill.exe
UninstallDisplayName=Duck Quill
ChangesAssociations=yes
ChangesEnvironment=yes
; Duck Quill must be closed to replace it (the updater closes it first).
CloseApplications=yes
RestartApplications=no
#ifdef Sign
; Only when a code-signing certificate has been added (see release.yml).
SignTool=signtool
SignedUninstaller=yes
#endif

[Tasks]
Name: "desktopicon"; Description: "Put a Duck Quill icon on the desktop"; Flags: unchecked
Name: "quillpath"; Description: "Let terminals and AI agents find quill.exe (adds the Duck Quill folder to your PATH)"; Flags: unchecked

[Files]
Source: "{#SourceDir}\DuckQuill.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\quill.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\art-cli.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\READ ME FIRST.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\brand\*"; DestDir: "{app}\brand"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#SourceDir}\skills\*"; DestDir: "{app}\skills"; Flags: ignoreversion recursesubdirs createallsubdirs

[InstallDelete]
; The skill folder is replaced whole, so files dropped from it go too.
Type: filesandordirs; Name: "{app}\skills"

[INI]
; Tells Duck Quill it was installed (so it offers Update, not Download).
Filename: "{app}\installed.ini"; Section: "DuckQuill"; Key: "version"; String: "{#AppVersion}"; Flags: uninsdeletesection

[UninstallDelete]
Type: files; Name: "{app}\installed.ini"

[Icons]
Name: "{autoprograms}\Duck Quill"; Filename: "{app}\DuckQuill.exe"; Comment: "Paint, draw and animate"
Name: "{autodesktop}\Duck Quill"; Filename: "{app}\DuckQuill.exe"; Tasks: desktopicon

[Registry]
; Duck Quill pictures: .quill (and .artapp, the old name) open in Duck Quill
; with the duck icon.
Root: HKCU; Subkey: "Software\Classes\DuckQuill.Picture"; ValueType: string; ValueName: ""; ValueData: "Duck Quill picture"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\DuckQuill.Picture\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\DuckQuill.exe,0"
Root: HKCU; Subkey: "Software\Classes\DuckQuill.Picture\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\DuckQuill.exe"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\.quill"; ValueType: string; ValueName: ""; ValueData: "DuckQuill.Picture"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.quill\OpenWithProgids"; ValueType: string; ValueName: "DuckQuill.Picture"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.artapp"; ValueType: string; ValueName: ""; ValueData: "DuckQuill.Picture"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.artapp\OpenWithProgids"; ValueType: string; ValueName: "DuckQuill.Picture"; ValueData: ""; Flags: uninsdeletevalue
; PNG: only an "Open with -> Duck Quill" entry, never the default.
Root: HKCU; Subkey: "Software\Classes\DuckQuill.Image"; ValueType: string; ValueName: ""; ValueData: "PNG image"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\DuckQuill.Image\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\DuckQuill.exe,0"
Root: HKCU; Subkey: "Software\Classes\DuckQuill.Image\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\DuckQuill.exe"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\.png\OpenWithProgids"; ValueType: string; ValueName: "DuckQuill.Image"; ValueData: ""; Flags: uninsdeletevalue
; How "Open with" names the program.
Root: HKCU; Subkey: "Software\Classes\Applications\DuckQuill.exe"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "Duck Quill"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\DuckQuill.exe\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\DuckQuill.exe"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\Applications\DuckQuill.exe\SupportedTypes"; ValueType: string; ValueName: ".quill"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\Applications\DuckQuill.exe\SupportedTypes"; ValueType: string; ValueName: ".artapp"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\Applications\DuckQuill.exe\SupportedTypes"; ValueType: string; ValueName: ".png"; ValueData: ""

[Run]
Filename: "{app}\DuckQuill.exe"; Description: "Launch Duck Quill"; Flags: nowait postinstall skipifsilent

[Code]
const
  EnvironmentKey = 'Environment';

{ True if Dir is one of the folders in the Path value. }
function PathHas(Paths, Dir: string): Boolean;
begin
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Paths) + ';') > 0;
end;

{ Adds the Duck Quill folder to your account's Path (not the system's). }
procedure AddToPath(Dir: string);
var
  Paths: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Paths) then
    Paths := '';
  if PathHas(Paths, Dir) then
    exit;
  if (Paths <> '') and (Copy(Paths, Length(Paths), 1) <> ';') then
    Paths := Paths + ';';
  RegWriteExpandStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Paths + Dir);
end;

{ Takes it out again, leaving every other folder as it was. }
procedure RemoveFromPath(Dir: string);
var
  Paths, Lowered: string;
  P: Integer;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Paths) then
    exit;
  Paths := ';' + Paths + ';';
  Lowered := Uppercase(Paths);
  P := Pos(';' + Uppercase(Dir) + ';', Lowered);
  if P = 0 then
    exit;
  Delete(Paths, P, Length(Dir) + 1);
  { Drop the ; added at each end above. }
  Paths := Copy(Paths, 2, Length(Paths) - 2);
  RegWriteExpandStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Paths);
end;

#ifdef FailHalfway
{ ONLY in a test build made by the install test (install-test.yml), never
  published: dies after the program files have been replaced, as a power
  cut or a crash would, so the test can prove the updater puts the old
  version back. }
procedure ExitProcess(ExitCode: Cardinal);
  external 'ExitProcess@kernel32.dll stdcall';
#endif

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    if WizardIsTaskSelected('quillpath') then
      AddToPath(ExpandConstant('{app}'));
#ifdef FailHalfway
    ExitProcess(5);
#endif
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemoveFromPath(ExpandConstant('{app}'));
end;
