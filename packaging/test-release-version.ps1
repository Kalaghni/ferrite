# No Cargo build, Inno Setup, network access or test framework required.
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'release-version.ps1')
$checks = 0
function Assert-Equal($actual, $expected) {
    if ($actual -cne $expected) { throw "Expected '$expected', got '$actual'." }
    $script:checks++
}
function Assert-Rejected([scriptblock]$operation, [string]$message) {
    $rejected = $false
    try { & $operation } catch {
        if ($_.Exception.Message -notlike "*$message*") { throw }
        $rejected = $true
    }
    if (-not $rejected) { throw 'Expected version validation to reject this input.' }
    $script:checks++
}
Assert-FerriteReleaseVersion -Version '0.8.1' -ReleaseTag 'v0.8.1'
$checks++
Assert-FerriteReleaseVersion -Version '0.8.1' -ReleaseTag ''
$checks++
Assert-Rejected { Assert-FerriteReleaseVersion -Version '0.7.0' -ReleaseTag 'v0.8.0' } 'does not match'
foreach ($tag in @('0.8.1', 'vv0.8.1', 'v0.8', 'v0.8.1garbage', 'v0.8.1/other', 'v01.8.1', ' v0.8.1', 'V0.8.1')) {
    Assert-Rejected { Assert-FerriteReleaseVersion -Version '0.8.1' -ReleaseTag $tag } 'Invalid release tag'
}
Assert-Equal (Resolve-FerriteInstallerVersion -ManifestVersion '0.8.1') '0.8.1'
Assert-Equal (Resolve-FerriteInstallerVersion -ManifestVersion '0.8.1' -RequestedVersion '0.8.1') '0.8.1'
Assert-Equal (Resolve-FerriteInstallerVersion -ManifestVersion '0.8.1' -RequestedVersion 'v0.8.1') '0.8.1'
Assert-Rejected { Resolve-FerriteInstallerVersion -ManifestVersion '0.7.0' -RequestedVersion '0.8.0' } 'does not match'
Assert-Rejected { Resolve-FerriteInstallerVersion -ManifestVersion '0.8.1' -RequestedVersion '0.0.0-dev' } 'does not match'
Assert-FerriteReportedVersion -Version '0.8.1' -BinaryName 'FERRITE' -ReportedVersion ("FERRITE 0.8.1" + [Environment]::NewLine)
$checks++
Assert-FerriteReportedVersion -Version '0.8.1' -BinaryName 'ferrite-server' -ReportedVersion 'ferrite-server 0.8.1'
$checks++
Assert-Rejected { Assert-FerriteReportedVersion -Version '0.8.1' -BinaryName 'FERRITE' -ReportedVersion 'FERRITE 0.7.0' } 'expected'
Assert-Rejected { Assert-FerriteReportedVersion -Version '0.8.1' -BinaryName 'FERRITE' -ReportedVersion '' } 'expected'
Assert-Rejected { Assert-FerriteReportedVersion -Version '0.8.1' -BinaryName 'ferrite-server' -ReportedVersion 'FERRITE 0.8.1' } 'expected'
$manifest = [System.IO.Path]::GetTempFileName()
try {
    @('[workspace.dependencies]', 'dependency = { version = "9.9.9" }', '[workspace.package]', 'version = "0.8.1"', '[other]', 'version = "8.8.8"') | Set-Content -LiteralPath $manifest
    Assert-Equal (Get-FerriteWorkspaceVersion -ManifestPath $manifest) '0.8.1'
    '[package]' | Set-Content -LiteralPath $manifest
    Assert-Rejected { Get-FerriteWorkspaceVersion -ManifestPath $manifest } 'exactly one'
} finally {
    Remove-Item -LiteralPath $manifest
}
Write-Host "release version checks: $checks passed"
