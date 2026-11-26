<#
.SYNOPSIS
    Build and install Ferrum Paint for the current Windows user (no admin rights needed).

.DESCRIPTION
    - Builds a release binary with Cargo (installs nothing system-wide).
    - Copies it to %LOCALAPPDATA%\Programs\Ferrum Paint.
    - Creates a Start Menu shortcut (and a desktop shortcut with -DesktopShortcut).
    - Associates .ferrum files with Ferrum Paint for the current user.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File packaging\install-windows.ps1
    powershell -ExecutionPolicy Bypass -File packaging\install-windows.ps1 -DesktopShortcut
#>
param(
    [switch]$DesktopShortcut,
    [switch]$NoFileAssociation
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $rustBin = Join-Path $env:USERPROFILE '.cargo\bin'
    if (Test-Path (Join-Path $rustBin 'cargo.exe')) {
        $env:Path = "$rustBin;$env:Path"
    } else {
        Write-Host 'Rust is not installed. Install it from https://rustup.rs (or: winget install Rustlang.Rustup),' -ForegroundColor Yellow
        Write-Host 'then open a new terminal and run this script again.' -ForegroundColor Yellow
        exit 1
    }
}

# Build outside the repo folder: folders under Documents/OneDrive are sometimes not writable by Cargo.
if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = Join-Path $env:LOCALAPPDATA 'ferrum-paint-target' }
Write-Host "Building Ferrum Paint (release). The first build takes a few minutes..." -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) { Write-Host 'Build failed.' -ForegroundColor Red; exit 1 }
$built = Join-Path $env:CARGO_TARGET_DIR 'release\ferrum-paint.exe'

$dest = Join-Path $env:LOCALAPPDATA 'Programs\Ferrum Paint'
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Get-Process ferrum-paint -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$dest*" } | Stop-Process -Force -ErrorAction SilentlyContinue
Copy-Item $built (Join-Path $dest 'ferrum-paint.exe') -Force
$exe = Join-Path $dest 'ferrum-paint.exe'
$ico = Join-Path $dest 'ferrum-paint.ico'
& $exe --write-icon $ico 256 | Out-Null
# --write-icon runs without a console in release builds; give it a moment to finish writing.
for ($i = 0; $i -lt 20 -and -not (Test-Path $ico); $i++) { Start-Sleep -Milliseconds 250 }

$shell = New-Object -ComObject WScript.Shell
function New-Shortcut($path) {
    $s = $shell.CreateShortcut($path)
    $s.TargetPath = $exe
    $s.WorkingDirectory = $dest
    $s.Description = 'Ferrum Paint image editor'
    if (Test-Path $ico) { $s.IconLocation = "$ico,0" }
    $s.Save()
}
$startMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Ferrum Paint.lnk'
New-Shortcut $startMenu
if ($DesktopShortcut) { New-Shortcut (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Ferrum Paint.lnk') }

if (-not $NoFileAssociation) {
    Start-Process -FilePath $exe -ArgumentList '--register-file-type' -Wait
}

Write-Host ''
Write-Host "Installed to $dest" -ForegroundColor Green
Write-Host 'Start it from the Start Menu ("Ferrum Paint"), or run:' 
Write-Host "  `"$exe`""
if (-not $NoFileAssociation) { Write-Host '.ferrum files now open in Ferrum Paint when double-clicked.' }
