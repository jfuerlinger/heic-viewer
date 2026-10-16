; HEIC Explorer Preview - Inno Setup script (requires Inno Setup 6.3 or newer)
;
; Build (from the repository root, after scripts\stage-dist.ps1):
;   iscc /DAppVersion=0.2.0 /DAppVersionNumeric=0.2.0.0 /DDistDir=..\dist installer\heic-explorer-preview.iss
;
; Silent install:   HEICExplorerPreview-Setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART [/CURRENTUSER | /ALLUSERS]
; Silent uninstall: unins000.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART

#define AppName "HEIC Explorer Preview"
#ifndef AppVersion
  #define AppVersion "0.2.0"
#endif
#ifndef AppVersionNumeric
  #define AppVersionNumeric "0.2.0.0"
#endif
#ifndef DistDir
  #define DistDir "..\dist"
#endif

; Must match CLSID_HEIC_THUMBNAIL_PROVIDER in crates/heic-thumbnail/src/lib.rs.
#define ClsId "{218C7AE3-FDC4-4381-ABCD-D7385AFB7A4A}"
; Shell extension key for IThumbnailProvider handlers.
#define ThumbnailHandler "{e357fccd-a995-4576-b01f-234630154e96}"
#define ProductKey "Software\HEIC Explorer Preview"
; Context menu verb key name (same for files, folders and folder backgrounds).
#define Verb "HEICExplorerPreview.ToJpeg"
#define ConvertExe "{app}\heic-convert.exe"
; Same GUIDs with "{" escaped as "{{" for sections where Inno expands {constants}.
#define ClsIdEsc "{" + ClsId
#define ThumbnailHandlerEsc "{" + ThumbnailHandler

[Setup]
AppId={{5B604925-CC94-4AA3-9C35-2FF1DC31CA74}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Josef Fürlinger
VersionInfoVersion={#AppVersionNumeric}
VersionInfoProductName={#AppName}
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
; Per-user by default (HKCU, no admin); "for all users" (HKLM) via dialog or /ALLUSERS.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog commandline
UsedUserAreasWarning=no
; x64 DLL: an Arm64 Explorer could not load it, so no x64 emulation targets.
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0
; Refreshes Explorer's associations (SHChangeNotify SHCNE_ASSOCCHANGED) after install and uninstall.
ChangesAssociations=yes
; Never offer to close Explorer; locked DLLs are moved aside instead (see [Code]).
CloseApplications=no
LicenseFile={#DistDir}\license-page.txt
OutputDir={#DistDir}\installer
OutputBaseFilename=HEICExplorerPreview-Setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupLogging=yes
UninstallDisplayName={#AppName}

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "de"; MessagesFile: "compiler:Languages\German.isl"

[CustomMessages]
en.ComponentThumbnails=Thumbnails for HEIC/HEIF photos in Explorer
de.ComponentThumbnails=Vorschaubilder für HEIC/HEIF-Fotos im Explorer
en.ComponentContextMenu=Context menu: convert HEIC to JPEG
de.ComponentContextMenu=Kontextmenü: HEIC in JPEG umwandeln
en.VerbFiles=Convert to JPEG
de.VerbFiles=In JPEG umwandeln
en.VerbFolder=Convert all HEIC files to JPEG
de.VerbFolder=Alle HEIC-Dateien in JPEG umwandeln

[Types]
Name: "full"; Description: "{cm:FullInstallation}"
Name: "custom"; Description: "{cm:CustomInstallation}"; Flags: iscustom

[Components]
Name: "main"; Description: "{cm:ComponentThumbnails}"; Types: full custom; Flags: fixed
Name: "contextmenu"; Description: "{cm:ComponentContextMenu}"; Types: full

[Files]
Source: "{#DistDir}\heic_thumbnail.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\heic-convert.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\heif.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\libde265.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\THIRD-PARTY-LICENSES.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs

[Registry]
; COM class. HKA = HKCU\Software\Classes (per user) or HKLM\Software\Classes (all users).
Root: HKA; Subkey: "Software\Classes\CLSID\{#ClsIdEsc}"; ValueType: string; ValueName: ""; ValueData: "{#AppName} Thumbnail Provider"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\CLSID\{#ClsIdEsc}\InprocServer32"; ValueType: string; ValueName: ""; ValueData: "{app}\heic_thumbnail.dll"
Root: HKA; Subkey: "Software\Classes\CLSID\{#ClsIdEsc}\InprocServer32"; ValueType: string; ValueName: "ThreadingModel"; ValueData: "Apartment"
; Thumbnail handler per extension. Previous values are backed up in CurStepChanged and
; restored (or the keys removed) in CurUninstallStepChanged, hence no uninstall flags here.
Root: HKA; Subkey: "Software\Classes\.heic\ShellEx\{#ThumbnailHandlerEsc}"; ValueType: string; ValueName: ""; ValueData: "{#ClsIdEsc}"
Root: HKA; Subkey: "Software\Classes\.heif\ShellEx\{#ThumbnailHandlerEsc}"; ValueType: string; ValueName: ""; ValueData: "{#ClsIdEsc}"
Root: HKA; Subkey: "Software\Classes\.hif\ShellEx\{#ThumbnailHandlerEsc}"; ValueType: string; ValueName: ""; ValueData: "{#ClsIdEsc}"

; Context menu (classic verbs; on Windows 11 under "Show more options").
; Files: one process per selected file; heic-convert.exe merges them into one window.
; MultiSelectModel=Player lifts Explorer's default limit of 15 selected items.
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.heic\shell\{#Verb}"; ValueType: string; ValueName: "MUIVerb"; ValueData: "{cm:VerbFiles}"; Components: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.heic\shell\{#Verb}"; ValueType: string; ValueName: "Icon"; ValueData: "{#ConvertExe}"; Components: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.heic\shell\{#Verb}"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Components: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.heic\shell\{#Verb}\command"; ValueType: string; ValueName: ""; ValueData: """{#ConvertExe}"" --lang={language} ""%1"""; Components: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.heif\shell\{#Verb}"; ValueType: string; ValueName: "MUIVerb"; ValueData: "{cm:VerbFiles}"; Components: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.heif\shell\{#Verb}"; ValueType: string; ValueName: "Icon"; ValueData: "{#ConvertExe}"; Components: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.heif\shell\{#Verb}"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Components: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.heif\shell\{#Verb}\command"; ValueType: string; ValueName: ""; ValueData: """{#ConvertExe}"" --lang={language} ""%1"""; Components: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.hif\shell\{#Verb}"; ValueType: string; ValueName: "MUIVerb"; ValueData: "{cm:VerbFiles}"; Components: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.hif\shell\{#Verb}"; ValueType: string; ValueName: "Icon"; ValueData: "{#ConvertExe}"; Components: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.hif\shell\{#Verb}"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Components: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.hif\shell\{#Verb}\command"; ValueType: string; ValueName: ""; ValueData: """{#ConvertExe}"" --lang={language} ""%1"""; Components: contextmenu
; Right-click on a folder, and on the empty area inside an open folder.
Root: HKA; Subkey: "Software\Classes\Directory\shell\{#Verb}"; ValueType: string; ValueName: "MUIVerb"; ValueData: "{cm:VerbFolder}"; Components: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Directory\shell\{#Verb}"; ValueType: string; ValueName: "Icon"; ValueData: "{#ConvertExe}"; Components: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\shell\{#Verb}\command"; ValueType: string; ValueName: ""; ValueData: """{#ConvertExe}"" --lang={language} ""%1"""; Components: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\{#Verb}"; ValueType: string; ValueName: "MUIVerb"; ValueData: "{cm:VerbFolder}"; Components: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\{#Verb}"; ValueType: string; ValueName: "Icon"; ValueData: "{#ConvertExe}"; Components: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\{#Verb}\command"; ValueType: string; ValueName: ""; ValueData: """{#ConvertExe}"" --lang={language} ""%V"""; Components: contextmenu

[UninstallDelete]
Type: files; Name: "{app}\*.old"

[Code]
const
  OurClsid = '{#ClsId}';
  BackupKey = '{#ProductKey}\ThumbnailHandlerBackup';

function HandlerKey(const Ext: String): String;
begin
  Result := 'Software\Classes\' + Ext + '\ShellEx\{#ThumbnailHandler}';
end;

{ Remembers a foreign handler (e.g. Microsoft's) once; re-installs keep the first backup. }
procedure BackupHandler(const Ext: String);
var
  Existing: String;
begin
  if RegValueExists(HKA, BackupKey, Ext) then
    Exit;
  if RegQueryStringValue(HKA, HandlerKey(Ext), '', Existing) and (Existing <> '') and
     (CompareText(Existing, OurClsid) <> 0) then
  begin
    Log(Format('Backing up thumbnail handler for %s: %s', [Ext, Existing]));
    RegWriteStringValue(HKA, BackupKey, Ext, Existing);
  end;
end;

{ Runs before the uninstall log is processed, so Inno's SHChangeNotify sees the result. }
procedure RestoreOrRemoveHandler(const Ext: String);
var
  Current, Previous: String;
begin
  if not RegQueryStringValue(HKA, HandlerKey(Ext), '', Current) or (CompareText(Current, OurClsid) <> 0) then
  begin
    Log(Format('Thumbnail handler for %s is not ours anymore, leaving it untouched', [Ext]));
    Exit;
  end;
  if RegQueryStringValue(HKA, BackupKey, Ext, Previous) and (Previous <> '') then
  begin
    Log(Format('Restoring thumbnail handler for %s: %s', [Ext, Previous]));
    RegWriteStringValue(HKA, HandlerKey(Ext), '', Previous);
  end
  else
  begin
    Log(Format('Removing thumbnail handler for %s', [Ext]));
    RegDeleteKeyIncludingSubkeys(HKA, HandlerKey(Ext));
    RegDeleteKeyIfEmpty(HKA, 'Software\Classes\' + Ext + '\ShellEx');
    RegDeleteKeyIfEmpty(HKA, 'Software\Classes\' + Ext);
  end;
end;

procedure DeleteStaleFiles(const Dir: String);
var
  FindRec: TFindRec;
begin
  if FindFirst(Dir + '\*.old', FindRec) then
  try
    repeat
      { Fails silently while a thumbnail host process still has the old DLL loaded. }
      DeleteFile(Dir + '\' + FindRec.Name);
    until not FindNext(FindRec);
  finally
    FindClose(FindRec);
  end;
end;

{ A loaded DLL cannot be overwritten or deleted, but it can be renamed. Explorer's thumbnail
  host (dllhost.exe) may keep our DLLs loaded, so move them out of the way first. }
procedure MoveAside(const FileName: String);
var
  Target: String;
begin
  if not FileExists(FileName) then
    Exit;
  Target := FileName + '.' + GetDateTimeString('yyyymmddhhnnss', #0, #0) + '.old';
  if RenameFile(FileName, Target) then
    Log('Moved aside ' + FileName)
  else
    Log('Could not move aside ' + FileName);
end;

{ Removes the context menu verbs, e.g. when the component was deselected on re-install. }
procedure RemoveContextMenu();
begin
  RegDeleteKeyIncludingSubkeys(HKA, 'Software\Classes\SystemFileAssociations\.heic\shell\{#Verb}');
  RegDeleteKeyIncludingSubkeys(HKA, 'Software\Classes\SystemFileAssociations\.heif\shell\{#Verb}');
  RegDeleteKeyIncludingSubkeys(HKA, 'Software\Classes\SystemFileAssociations\.hif\shell\{#Verb}');
  RegDeleteKeyIncludingSubkeys(HKA, 'Software\Classes\Directory\shell\{#Verb}');
  RegDeleteKeyIncludingSubkeys(HKA, 'Software\Classes\Directory\Background\shell\{#Verb}');
end;

procedure MoveAsideBinaries(const Dir: String);
begin
  DeleteStaleFiles(Dir);
  MoveAside(Dir + '\heic-convert.exe');
  MoveAside(Dir + '\heic_thumbnail.dll');
  MoveAside(Dir + '\heif.dll');
  MoveAside(Dir + '\libde265.dll');
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssInstall then
  begin
    BackupHandler('.heic');
    BackupHandler('.heif');
    BackupHandler('.hif');
    MoveAsideBinaries(ExpandConstant('{app}'));
  end;
  if (CurStep = ssPostInstall) and not WizardIsComponentSelected('contextmenu') then
    RemoveContextMenu();
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  case CurUninstallStep of
    usUninstall:
      begin
        RestoreOrRemoveHandler('.heic');
        RestoreOrRemoveHandler('.heif');
        RestoreOrRemoveHandler('.hif');
        RegDeleteKeyIncludingSubkeys(HKA, '{#ProductKey}');
        MoveAsideBinaries(ExpandConstant('{app}'));
      end;
    usPostUninstall:
      begin
        DeleteStaleFiles(ExpandConstant('{app}'));
        RemoveDir(ExpandConstant('{app}'));
      end;
  end;
end;
