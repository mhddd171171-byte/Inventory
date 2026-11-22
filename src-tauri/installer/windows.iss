#ifndef RepoRoot
 #error RepoRoot must be provided
#endif
#ifndef WebViewInstaller
 #error WebViewInstaller must be provided
#endif
#ifndef ReleaseVersion
 #error ReleaseVersion must be provided
#endif
[Setup]
AppId=com.repaircenter.inventorymanager.limitedaccess
AppName=Inventory Manager - Limited Access
AppVersion={#ReleaseVersion}
AppPublisher=Al Mohtarif Repair Center
DefaultDirName={localappdata}\Programs\Inventory Manager Limited Access
DefaultGroupName=Inventory Manager Limited Access
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#RepoRoot}\deliverables
OutputBaseFilename=Inventory Manager Limited Access_{#ReleaseVersion}_x64-setup
SetupIconFile={#RepoRoot}\src-tauri\icons\icon.ico
UninstallDisplayIcon={app}\inventory-manager-limited-access.exe
Compression=zip
SolidCompression=no
CloseApplications=yes
RestartApplications=no
WizardStyle=modern
[Tasks]
Name: desktopicon; Description: Create a desktop shortcut; GroupDescription: Shortcuts:
[Files]
Source: "{#RepoRoot}\src-tauri\target\release\inventory-manager-limited-access.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\src-tauri\resources\limited\seed.json"; DestDir: "{app}\resources\limited"; Flags: ignoreversion
Source: "{#RepoRoot}\src-tauri\resources\licenses\RUST-NOTICES.md"; DestDir: "{app}\resources\licenses"; Flags: ignoreversion
Source: "{#RepoRoot}\src-tauri\resources\licenses\rust\*"; DestDir: "{app}\resources\licenses\rust"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#RepoRoot}\src-tauri\resources\licenses\frontend\*"; DestDir: "{app}\resources\licenses\frontend"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#RepoRoot}\src-tauri\resources\licenses\installer\*"; DestDir: "{app}\resources\licenses\installer"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#WebViewInstaller}"; DestDir: "{tmp}"; DestName: "Inventory-WebView2.exe"; Flags: deleteafterinstall; Check: NeedsWebView
[Icons]
Name: "{group}\Inventory Manager Limited Access"; Filename: "{app}\inventory-manager-limited-access.exe"
Name: "{autodesktop}\Inventory Manager Limited Access"; Filename: "{app}\inventory-manager-limited-access.exe"; Tasks: desktopicon
[Run]
Filename: "{tmp}\Inventory-WebView2.exe"; Parameters: "/silent /install"; Flags: runhidden waituntilterminated; Check: NeedsWebView
Filename: "{app}\inventory-manager-limited-access.exe"; Description: Open Limited Access; Flags: nowait postinstall skipifsilent
[Code]
function NeedsWebView: Boolean;
var Version: String;
begin
 Result := not ((RegQueryStringValue(HKCU32, 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version) and (Version <> '') and (Version <> '0.0.0.0')) or
 (RegQueryStringValue(HKLM32, 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version) and (Version <> '') and (Version <> '0.0.0.0')));
end;
