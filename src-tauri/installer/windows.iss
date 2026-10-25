; Build with scripts/build-windows.ps1. All installed runtime paths are relative.
#ifndef RepoRoot
  #error RepoRoot must be provided
#endif
#ifndef WebViewInstaller
  #error WebViewInstaller must be provided
#endif
[Setup]
AppId=com.repaircenter.inventorymanager
AppName=Inventory Manager
AppVersion=2.0.10
AppPublisher=Al Mohtarif Repair Center
DefaultDirName={localappdata}\Programs\Inventory Manager
DefaultGroupName=Inventory Manager
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#RepoRoot}\deliverables
OutputBaseFilename=Inventory Manager_2.0.10_x64-setup
SetupIconFile={#RepoRoot}\src-tauri\icons\icon.ico
UninstallDisplayIcon={app}\inventory-manager.exe
Compression=zip
SolidCompression=no
DiskSpanning=no
CloseApplications=yes
RestartApplications=no
WizardStyle=modern
DisableWelcomePage=no
[Tasks]
Name: desktopicon; Description: Create a desktop shortcut; GroupDescription: Shortcuts:
[Files]
Source: "{#RepoRoot}\src-tauri\target\release\inventory-manager.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\src-tauri\resources\*"; DestDir: "{app}\resources"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#WebViewInstaller}"; DestDir: "{tmp}"; DestName: "Inventory-WebView2.exe"; Flags: deleteafterinstall; Check: NeedsWebView
[Icons]
Name: "{group}\Inventory Manager"; Filename: "{app}\inventory-manager.exe"
Name: "{autodesktop}\Inventory Manager"; Filename: "{app}\inventory-manager.exe"; Tasks: desktopicon
[Run]
Filename: "{tmp}\Inventory-WebView2.exe"; Parameters: "/silent /install"; Flags: runhidden waituntilterminated; Check: NeedsWebView
Filename: "{app}\inventory-manager.exe"; Description: Open Inventory Manager; Flags: nowait postinstall skipifsilent
[InstallDelete]
; Remove only superseded application assets; business data is stored separately.
Type: files; Name: "{app}\resources\ai\Qwen3-0.6B-Q8_0.gguf"
Type: files; Name: "{app}\resources\ai\Qwen3-1.7B-Q4_K_M.gguf"
Type: files; Name: "{app}\uninstall.exe"
Type: files; Name: "{app}\resources\voice\tts\nabra\config.json"
Type: files; Name: "{app}\resources\voice\tts\nabra\af_msa.bin"
Type: files; Name: "{app}\resources\voice\tts\en_US-ljspeech-high.onnx"
Type: files; Name: "{app}\resources\voice\tts\en_US-ljspeech-high.onnx.json"
Type: files; Name: "{app}\resources\voice\tts\MODEL_CARD"
[Code]
function NeedsWebView: Boolean;
var Version: String;
begin
  Result := not ((RegQueryStringValue(HKCU32, 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version) and (Version <> '') and (Version <> '0.0.0.0')) or
    (RegQueryStringValue(HKLM32, 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version) and (Version <> '') and (Version <> '0.0.0.0')));
end;
procedure CurStepChanged(CurStep: TSetupStep);
var LegacyCommand: String;
begin
  if CurStep = ssPostInstall then begin
    // Replace a legacy registration only when it belongs to this same directory.
    if RegQueryStringValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Uninstall\Inventory Manager', 'UninstallString', LegacyCommand) then
      if (CompareText(LegacyCommand, '"' + ExpandConstant('{app}\uninstall.exe') + '"') = 0) or
         (CompareText(LegacyCommand, ExpandConstant('{app}\uninstall.exe')) = 0) then
        RegDeleteKeyIncludingSubkeys(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Uninstall\Inventory Manager');
  end;
end;
