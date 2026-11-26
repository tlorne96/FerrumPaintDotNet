<#
.SYNOPSIS
    Remove Ferrum Paint installed by install-windows.ps1 (current user).
    Your images and %APPDATA%\Ferrum Paint\settings.ini are kept unless you pass -RemoveSettings.
#>
param([switch]$RemoveSettings)
$ErrorActionPreference = 'Continue'
$dest = Join-Path $env:LOCALAPPDATA 'Programs\Ferrum Paint'
Get-Process ferrum-paint -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$dest*" } | Stop-Process -Force
Remove-Item -Recurse -Force $dest -ErrorAction SilentlyContinue
Remove-Item -Force (Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Ferrum Paint.lnk') -ErrorAction SilentlyContinue
Remove-Item -Force (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Ferrum Paint.lnk') -ErrorAction SilentlyContinue
foreach ($k in 'HKCU:\Software\Classes\.ferrum', 'HKCU:\Software\Classes\FerrumPaint.Image') {
    if (Test-Path $k) { Remove-Item -Recurse -Force $k }
}
if ($RemoveSettings) { Remove-Item -Recurse -Force (Join-Path $env:APPDATA 'Ferrum Paint') -ErrorAction SilentlyContinue }
Write-Host 'Ferrum Paint has been removed.' -ForegroundColor Green
