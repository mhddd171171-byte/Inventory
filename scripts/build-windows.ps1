param([switch]$SkipAppBuild)
$ErrorActionPreference='Stop'
$repoRoot=Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $repoRoot
if(-not $SkipAppBuild){
  & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'desktop.ps1') build-app
  if($LASTEXITCODE -ne 0){throw 'Desktop compilation failed.'}
}
$compilerCandidates=@((Join-Path $repoRoot '.build/inno/ISCC.exe'),(Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6/ISCC.exe'),(Join-Path $env:LOCALAPPDATA 'Programs/Inno Setup 6/ISCC.exe'))
$compiler=$compilerCandidates | Where-Object {Test-Path -LiteralPath $_} | Select-Object -First 1
if(-not $compiler){throw 'Install the reviewed Inno Setup 6.5.4 compiler on the build machine. It is not needed on customer computers.'}
$webView=Get-ChildItem -LiteralPath (Join-Path $env:LOCALAPPDATA 'tauri/x64') -Filter MicrosoftEdgeWebView2RuntimeInstallerX64.exe -File -Recurse | Select-Object -First 1
if(-not $webView){throw 'The offline Microsoft WebView2 x64 installer is missing from the Tauri build cache.'}
$appExecutable=if($env:CARGO_TARGET_DIR){Join-Path $env:CARGO_TARGET_DIR "release/inventory-manager.exe"}else{Join-Path $repoRoot "src-tauri/target/release/inventory-manager.exe"}
& $compiler "/DAppExecutable=$appExecutable" "/DRepoRoot=$repoRoot" "/DWebViewInstaller=$($webView.FullName)" (Join-Path $repoRoot 'src-tauri/installer/windows.iss')
if($LASTEXITCODE -ne 0){throw 'Offline installer compilation failed.'}
Write-Output (Join-Path $repoRoot 'deliverables/Inventory Manager_2.0.6_x64-setup.exe')
