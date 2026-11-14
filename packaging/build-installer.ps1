# Build the Windows installer: dist\ferrite-v<version>-windows-x64-setup.exe
#
#   .\packaging\build-installer.ps1            # build the game, then the installer
#   .\packaging\build-installer.ps1 -SkipBuild # package what is in target\release
#
# Needs Inno Setup 6 (winget install JRSoftware.InnoSetup). The version
# comes from Cargo.toml unless -Version is given.
param(
    [string]$Version = '',
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

if (-not $Version) {
    $line = Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
    if (-not $line) { throw 'no version in Cargo.toml' }
    $Version = $line.Matches[0].Groups[1].Value
}
$Version = $Version.TrimStart('v')

$iscc = (Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source
if (-not $iscc) {
    $iscc = @(
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $iscc) { throw 'Inno Setup 6 was not found. Install it with: winget install JRSoftware.InnoSetup' }

if (-not $SkipBuild) {
    cargo build --release -p ferrite -p ferrite-server
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
}

# Stage exactly what gets installed.
$stage = Join-Path $root 'target\installer-stage'
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item (Join-Path $root 'target\release\ferrite.exe'), (Join-Path $root 'target\release\ferrite-server.exe') $stage
Copy-Item (Join-Path $PSScriptRoot 'README.txt'), (Join-Path $PSScriptRoot 'Quantico-OFL.txt'), (Join-Path $PSScriptRoot 'start-server.bat') $stage

& $iscc /Qp "/DAppVersion=$Version" "/DSourceDir=$stage" (Join-Path $PSScriptRoot 'ferrite.iss')
if ($LASTEXITCODE -ne 0) { throw "ISCC failed ($LASTEXITCODE)" }
$out = Join-Path $root "dist\ferrite-v$Version-windows-x64-setup.exe"
Write-Host "installer: $out ($([math]::Round((Get-Item $out).Length / 1MB, 1)) MB)"
