#ifndef AppVersion
  #error AppVersion is required
#endif
[Setup]
AppId={{C27443EB-4029-43DE-A19F-A3BCF876BF86}
AppName=Media Manager
AppVersion={#AppVersion}
VersionInfoVersion={#NumericVersion}
AppPublisher=Exenreco
AppPublisherURL=https://github.com/exenreco/Media-Manager
DefaultDirName={localappdata}\Programs\Media Manager
DefaultGroupName=Media Manager
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename={#OutputName}
LicenseFile={#SourceDir}\LICENSE
UninstallDisplayIcon={app}\Media-Manager.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: desktopicon; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#WebViewBootstrapper}"; DestName: "MicrosoftEdgeWebview2Setup.exe"; Flags: dontcopy

[Icons]
Name: "{group}\Media Manager"; Filename: "{app}\Media-Manager.exe"
Name: "{group}\Uninstall Media Manager"; Filename: "{uninstallexe}"
Name: "{userdesktop}\Media Manager"; Filename: "{app}\Media-Manager.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\Media-Manager.exe"; Description: "Launch Media Manager"; Flags: nowait postinstall skipifsilent

[Code]
function RuntimePresent(Root: Integer): Boolean;
var
  Version: String;
begin
  Result := RegQueryStringValue(Root, 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version);
  if Result then Result := (Version <> '') and (Version <> '0.0.0.0');
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ExitCode: Integer;
begin
  Result := '';
  if RuntimePresent(HKLM32) or RuntimePresent(HKCU32) then Exit;
  ExitCode := 0;
  ExtractTemporaryFile('MicrosoftEdgeWebview2Setup.exe');
  if not Exec(ExpandConstant('{tmp}\MicrosoftEdgeWebview2Setup.exe'), '/silent /install', '', SW_HIDE, ewWaitUntilTerminated, ExitCode) then
    Result := 'Could not start the Microsoft WebView2 runtime installer. Please install WebView2 and retry.'
  else if (ExitCode <> 0) and (ExitCode <> 3010) then
    Result := 'Microsoft WebView2 could not be installed (code ' + IntToStr(ExitCode) + '). Check your internet connection and retry.';
  NeedsRestart := ExitCode = 3010;
end;
