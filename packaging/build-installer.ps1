# Build the Windows installer: dist\ferrite-v<version>-windows-x64-setup.exe
#
#   .\packaging\build-installer.ps1            # build the game, then the installer
#   .\packaging\build-installer.ps1 -SkipBuild # package what is in target\release
#
# Needs Inno Setup 6 (winget install JRSoftware.InnoSetup). The version
# comes from Cargo.toml; -Version, when given, must match it.
param(
    [string]$Version = '',
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

. (Join-Path $PSScriptRoot 'release-version.ps1')
$manifestVersion = Get-FerriteWorkspaceVersion
$Version = Resolve-FerriteInstallerVersion -ManifestVersion $manifestVersion -RequestedVersion $Version
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
    cargo build --release -p ferrite -p ferrite-server -p ferrite-forge-mcp
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
}

# Check the actual executables as well as the manifest, especially with
# -SkipBuild: a stale target\release must never get a newer installer label.
Assert-FerriteBinaryVersion -Path (Join-Path $root 'target\release\ferrite.exe') -Version $Version -BinaryName 'FERRITE'
Assert-FerriteBinaryVersion -Path (Join-Path $root 'target\release\ferrite-server.exe') -Version $Version -BinaryName 'ferrite-server'

# Stage exactly what gets installed.
$targetRoot = [System.IO.Path]::GetFullPath((Join-Path $root 'target'))
$stage = [System.IO.Path]::GetFullPath((Join-Path $targetRoot 'installer-stage'))
if (-not $stage.StartsWith($targetRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'Installer staging path escaped the target directory.'
}
if (Test-Path -LiteralPath $stage) {
    if ((Get-Item -LiteralPath $stage).Attributes -band [System.IO.FileAttributes]::ReparsePoint) {
        throw 'Installer staging directory must not be a link or junction.'
    }
    Remove-Item -LiteralPath $stage -Recurse -Force
}
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item (Join-Path $root 'target\release\ferrite.exe'), (Join-Path $root 'target\release\ferrite-server.exe'), (Join-Path $root 'target\release\ferrite-forge-mcp.exe') $stage
Copy-Item (Join-Path $PSScriptRoot 'README.txt'), (Join-Path $PSScriptRoot 'Quantico-OFL.txt'), (Join-Path $PSScriptRoot 'start-server.bat') $stage

& $iscc /Qp "/DAppVersion=$Version" "/DSourceDir=$stage" (Join-Path $PSScriptRoot 'ferrite.iss')
if ($LASTEXITCODE -ne 0) { throw "ISCC failed ($LASTEXITCODE)" }
$out = Join-Path $root "dist\ferrite-v$Version-windows-x64-setup.exe"
Write-Host "installer: $out ($([math]::Round((Get-Item $out).Length / 1MB, 1)) MB)"
