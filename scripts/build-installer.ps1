# IDM Master — Build & Package Script
# Usage: .\scripts\build-installer.ps1 [-Version "0.1.0"]

param(
    [string]$Version = "0.1.0"
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot

Write-Host "=== IDM Master Build Script v$Version ===" -ForegroundColor Cyan

# 1. Build React frontend
Write-Host "`n[1/3] Building React frontend..." -ForegroundColor Yellow
Push-Location $Root
npx vite build
if ($LASTEXITCODE -ne 0) { throw "Vite build failed" }
Pop-Location

# 2. Build Rust backend
Write-Host "`n[2/3] Building Rust backend (release)..." -ForegroundColor Yellow
Push-Location $Root
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "Cargo build failed" }
Pop-Location

# 3. Create NSIS installer
Write-Host "`n[3/3] Creating NSIS installer..." -ForegroundColor Yellow

# Check for NSIS
$makensis = Get-Command makensis -ErrorAction SilentlyContinue
if (-not $makensis) {
    $nsisPath = "${env:ProgramFiles(x86)}\NSIS\makensis.exe"
    if (Test-Path $nsisPath) {
        $makensis = $nsisPath
    } else {
        Write-Host "WARNING: NSIS not found. Skipping installer creation." -ForegroundColor Red
        Write-Host "Install NSIS from: https://nsis.sourceforge.io/Download" -ForegroundColor Yellow
        Write-Host ""
        Write-Host "Build artifacts are ready:" -ForegroundColor Green
        Write-Host "  EXE:  src-tauri\target\release\idm-master-tauri.exe" -ForegroundColor White
        Write-Host "  Dist: dist\" -ForegroundColor White
        Write-Host ""
        Write-Host "To manually create installer:" -ForegroundColor Yellow
        Write-Host "  cd scripts && makensis /DAPP_VERSION=$Version installer.nsi" -ForegroundColor White
        exit 0
    }
}

# Create dist directory
New-Item -ItemType Directory -Force -Path "$Root\dist" | Out-Null

Push-Location "$Root\scripts"
& $makensis "/DAPP_VERSION=$Version" installer.nsi
if ($LASTEXITCODE -ne 0) { throw "NSIS build failed" }
Pop-Location

Write-Host "`n=== Build Complete ===" -ForegroundColor Green
Write-Host "Installer: dist\IDM-Master-Setup-$Version.exe" -ForegroundColor White
