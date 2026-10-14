param([ValidateSet('dev','build','build-app','test','test-ai','test-voice','check')][string]$Task='dev')
$ErrorActionPreference='Stop'
$repoRoot=Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $repoRoot
$rustBin=Join-Path $env:USERPROFILE '.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin'
if(Test-Path -LiteralPath $rustBin){$env:PATH="$rustBin;$env:PATH"}
$vsRoot='C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools'
$vcRoot=Join-Path $vsRoot 'VC\Tools\MSVC\14.29.30133'
$sdkRoot='C:\Program Files (x86)\Windows Kits\10'
$sdkVersion='10.0.19041.0'
if(Test-Path -LiteralPath "$vcRoot\bin\Hostx64\x64\cl.exe"){
  $env:PATH="$vcRoot\bin\Hostx64\x64;$sdkRoot\bin\$sdkVersion\x64;$env:PATH"
  $env:INCLUDE="$vcRoot\include;$sdkRoot\Include\$sdkVersion\ucrt;$sdkRoot\Include\$sdkVersion\shared;$sdkRoot\Include\$sdkVersion\um;$sdkRoot\Include\$sdkVersion\winrt"
  $env:LIB="$vcRoot\lib\x64;$sdkRoot\Lib\$sdkVersion\ucrt\x64;$sdkRoot\Lib\$sdkVersion\um\x64"
}
if($Task -eq 'test-ai') { & cargo test --manifest-path src-tauri/Cargo.toml independent_assistant_acceptance -- --ignored --nocapture }
elseif($Task -eq 'test-voice') { & cargo test --manifest-path src-tauri/Cargo.toml packaged_voice_playback_acceptance -- --ignored --nocapture }
elseif($Task -eq 'test' -or $Task -eq 'check') { & cargo $Task --manifest-path src-tauri/Cargo.toml }
elseif($Task -eq 'build-app') { & npx tauri build --no-bundle }
elseif($Task -eq 'build') { & powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-windows.ps1 }
else { & npx tauri $Task }
exit $LASTEXITCODE
