param([string]$Python = '.build\voice-env\Scripts\python.exe')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
& $Python voice/patch_faster_whisper.py
if ($LASTEXITCODE -ne 0) { throw 'Voice dependency preparation failed' }
& $Python -m PyInstaller --noconfirm --onedir --console --name voice-worker --distpath .build/voice-dist-final --workpath .build/voice-pyinstaller-final --specpath .build --collect-all faster_whisper --collect-all piper --collect-all ctranslate2 --collect-all onnxruntime --collect-all sounddevice --collect-all _sounddevice_data --collect-all tokenizers --collect-all pywebrtc_audio --exclude-module av --exclude-module piper.train --exclude-module piper.http_server voice/transcribe.py
if ($LASTEXITCODE -ne 0) { throw 'Voice packaging failed' }
$destination = Join-Path $projectRoot 'src-tauri\resources\voice\runtime'
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Copy-Item -Path '.build\voice-dist-final\voice-worker\*' -Destination $destination -Recurse -Force
Write-Output "Packaged offline voice runtime: $destination"
