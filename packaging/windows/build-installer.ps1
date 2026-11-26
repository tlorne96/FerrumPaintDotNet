# Build the Windows installer: dist\FerrumPaint-<version>-Setup.exe
# Needs Rust and Inno Setup 6 (winget install JRSoftware.InnoSetup).
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $repo
if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = Join-Path $repo 'target' }
cargo build --release
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
$iscc = @("$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe", "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe", "$env:ProgramFiles\Inno Setup 6\ISCC.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) { throw 'Inno Setup 6 not found. Install it with: winget install JRSoftware.InnoSetup' }
& $iscc /Qp "/DAppVersion=$version" "/DExePath=$env:CARGO_TARGET_DIR\release\ferrum-paint.exe" packaging\windows\ferrum-paint.iss
if ($LASTEXITCODE -ne 0) { throw 'Inno Setup failed' }
Write-Host "Built dist\FerrumPaint-$version-Setup.exe"