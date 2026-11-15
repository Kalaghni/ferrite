# Shared release/installer checks. Dot-source this file; no build is started.
function Get-FerriteWorkspaceVersion {
    param([string]$ManifestPath = (Join-Path (Split-Path $PSScriptRoot -Parent) 'Cargo.toml'))
    $manifest = Get-Content -LiteralPath $ManifestPath -Raw
    $section = [regex]::Match($manifest, '(?ms)^\[workspace\.package\]\s*(?<body>.*?)(?=^\[|\z)')
    $versions = [regex]::Matches($section.Groups['body'].Value, '(?m)^\s*version\s*=\s*"(?<version>[^"]+)"\s*(?:#.*)?$')
    if (-not $section.Success -or $versions.Count -ne 1) {
        throw 'Cargo.toml must declare exactly one [workspace.package] version.'
    }
    return $versions[0].Groups['version'].Value
}

function Assert-FerriteReleaseVersion {
    param([string]$Version, [AllowEmptyString()][string]$ReleaseTag = '')
    # Empty means an ordinary branch/PR build. Published releases use vX.Y.Z.
    if ($ReleaseTag -eq '') { return }
    if ($ReleaseTag -cnotmatch '^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') {
        throw "Invalid release tag '$ReleaseTag'; expected vX.Y.Z."
    }
    if ($ReleaseTag.Substring(1) -cne $Version) {
        throw "Release tag '$ReleaseTag' does not match workspace version '$Version'. Bump Cargo.toml and Cargo.lock before releasing."
    }
}

function Resolve-FerriteInstallerVersion {
    param([string]$ManifestVersion, [AllowEmptyString()][string]$RequestedVersion = '')
    if ($RequestedVersion -eq '') { return $ManifestVersion }
    $requested = if ($RequestedVersion.StartsWith('v')) { $RequestedVersion.Substring(1) } else { $RequestedVersion }
    if ($requested -cne $ManifestVersion) {
        throw "Installer version '$RequestedVersion' does not match workspace version '$ManifestVersion'. Build that version instead of relabelling binaries."
    }
    return $ManifestVersion
}

function Assert-FerriteReportedVersion {
    param([string]$Version, [string]$BinaryName, [AllowEmptyString()][string]$ReportedVersion)
    if ($ReportedVersion.Trim() -cne "$BinaryName $Version") {
        throw "$BinaryName reports '$($ReportedVersion.Trim())', expected '$BinaryName $Version'. Rebuild before packaging."
    }
}

function Assert-FerriteBinaryVersion {
    param([string]$Path, [string]$Version, [string]$BinaryName)
    # Run --version without creating a window, including for the GUI client.
    # Read both streams asynchronously so a failed/old binary cannot deadlock.
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = (Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path
    $info.Arguments = '--version'
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Hidden
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $info
    try {
        [void]$process.Start()
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(10000)) {
            $process.Kill()
            throw "$BinaryName --version did not exit. Rebuild before packaging."
        }
        $output = $stdout.GetAwaiter().GetResult()
        $errorOutput = $stderr.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0) {
            throw "$BinaryName --version failed: $errorOutput"
        }
        Assert-FerriteReportedVersion -Version $Version -BinaryName $BinaryName -ReportedVersion $output
    } finally {
        $process.Dispose()
    }
}
