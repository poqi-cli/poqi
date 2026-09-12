[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Archive,

    [Parameter(Mandatory)]
    [ValidateSet("linux-x86_64", "macos-arm64", "macos-x86_64", "windows-x86_64")]
    [string]$Target,

    [Parameter(Mandatory)]
    [ValidatePattern('^v[0-9][0-9A-Za-z.+-]*$')]
    [string]$ReleaseTag,

    [string]$DatabaseUrl
)

$ErrorActionPreference = "Stop"
if (-not $PSBoundParameters.ContainsKey("DatabaseUrl")) {
    $DatabaseUrl = [Environment]::GetEnvironmentVariable("POQI_RELEASE_SMOKE_DATABASE_URL")
}

$archivePath = (Resolve-Path -LiteralPath $Archive).Path
$isWindowsPackage = $Target -eq "windows-x86_64"
$expectedSuffix = if ($isWindowsPackage) { ".zip" } else { ".tar.gz" }

if (-not $archivePath.EndsWith($expectedSuffix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Package for $Target must use the $expectedSuffix archive format."
}

$archiveName = [IO.Path]::GetFileName($archivePath)
$packageName = $archiveName.Substring(0, $archiveName.Length - $expectedSuffix.Length)
$expectedPackageName = "poqi-$ReleaseTag-$Target"
if ($packageName -cne $expectedPackageName) {
    throw "Expected package '$expectedPackageName', got '$packageName'."
}

$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd(
    [IO.Path]::DirectorySeparatorChar,
    [IO.Path]::AltDirectorySeparatorChar
)
$smokeRoot = [IO.Path]::GetFullPath((Join-Path $tempRoot "poqi-package-smoke-$([guid]::NewGuid())"))
$expectedRootPrefix = "$tempRoot$([IO.Path]::DirectorySeparatorChar)"
$pathComparison = if ($IsWindows) { [StringComparison]::OrdinalIgnoreCase } else { [StringComparison]::Ordinal }
if (-not $smokeRoot.StartsWith($expectedRootPrefix, $pathComparison) -or
    -not [IO.Path]::GetFileName($smokeRoot).StartsWith("poqi-package-smoke-", [StringComparison]::Ordinal)) {
    throw "Could not create the package smoke directory under the system temporary directory."
}

try {
    New-Item -ItemType Directory -Path $smokeRoot | Out-Null
    if ($isWindowsPackage) {
        Expand-Archive -LiteralPath $archivePath -DestinationPath $smokeRoot
    } else {
        & tar -xzf $archivePath -C $smokeRoot
        if ($LASTEXITCODE -ne 0) { throw "Could not extract '$archiveName'." }
    }

    $packageRoot = Join-Path $smokeRoot $packageName
    $binaryName = if ($isWindowsPackage) { "poqi.exe" } else { "poqi" }
    $expectedFiles = @("CHANGELOG.md", "LICENSE", "README.md", "THIRD-PARTY-LICENSES.txt", $binaryName) | Sort-Object
    $actualEntries = @(Get-ChildItem -LiteralPath $packageRoot -Force | Select-Object -ExpandProperty Name | Sort-Object)
    $unexpectedEntries = @(Compare-Object -ReferenceObject $expectedFiles -DifferenceObject $actualEntries)
    if ($unexpectedEntries.Count -ne 0) {
        throw "Package contents differ from the expected files: $($unexpectedEntries | Out-String)"
    }

    $noticesPath = Join-Path $packageRoot "THIRD-PARTY-LICENSES.txt"
    $notices = [IO.File]::ReadAllText($noticesPath)
    $requiredNoticeSections = @(
        "RUST PACKAGES",
        "RUST LICENSE TEXTS",
        "NATIVE CODE APPENDIX",
        "pg_query bundled C sources",
        "zstd-sys bundled Zstandard",
        "bzip2-sys bundled libbzip2",
        "libsqlite3-sys bundled SQLite",
        "ring native cryptography code"
    )
    foreach ($section in $requiredNoticeSections) {
        if (-not $notices.Contains($section, [StringComparison]::Ordinal)) {
            throw "Third-party notices are missing the '$section' section."
        }
    }

    $binaryPath = Join-Path $packageRoot $binaryName
    $isolatedHome = Join-Path $smokeRoot "home"
    $isolatedConfig = Join-Path $isolatedHome ".config"
    $isolatedAppData = Join-Path $isolatedHome "AppData"
    New-Item -ItemType Directory -Path $isolatedHome, $isolatedConfig, $isolatedAppData | Out-Null

    function Invoke-PackagedBinary {
        param(
            [string]$Operation,
            [string[]]$Arguments
        )

        $start = [Diagnostics.ProcessStartInfo]::new()
        $start.FileName = $binaryPath
        foreach ($argument in $Arguments) { $start.ArgumentList.Add($argument) }
        $start.WorkingDirectory = $isolatedHome
        $start.UseShellExecute = $false
        $start.RedirectStandardOutput = $true
        $start.RedirectStandardError = $true
        $start.Environment["HOME"] = $isolatedHome
        $start.Environment["USERPROFILE"] = $isolatedHome
        $start.Environment["XDG_CONFIG_HOME"] = $isolatedConfig
        $start.Environment["XDG_DATA_HOME"] = Join-Path $isolatedHome ".local/share"
        $start.Environment["APPDATA"] = $isolatedAppData
        $start.Environment["POQI_CONFIG_DIR"] = $isolatedConfig
        [void]$start.Environment.Remove("POQI_DATABASE_URL")

        $process = [Diagnostics.Process]::Start($start)
        try {
            $stdout = $process.StandardOutput.ReadToEndAsync()
            $stderr = $process.StandardError.ReadToEndAsync()
            if (-not $process.WaitForExit(30000)) {
                try { $process.Kill($true) } catch [InvalidOperationException] { }
                [void]$process.WaitForExit(5000)
                throw "Packaged binary $Operation timed out."
            }
            if ($process.ExitCode -ne 0) {
                [void]$stderr.Result
                throw "Packaged binary $Operation failed with exit code $($process.ExitCode)."
            }
            return $stdout.Result
        } finally {
            $process.Dispose()
        }
    }

    $help = Invoke-PackagedBinary -Operation "help" -Arguments @("--help")
    if ($help -notmatch '(?m)^Usage: poqi(?:\.exe)? ') {
        throw "Packaged binary returned unexpected help output."
    }

    $version = (Invoke-PackagedBinary -Operation "version" -Arguments @("--version")).Trim()
    $expectedVersion = "poqi $($ReleaseTag.Substring(1))"
    if ($version -cne $expectedVersion) {
        throw "Expected version '$expectedVersion', got '$version'."
    }

    if (-not $isWindowsPackage) {
        $doctor = Invoke-PackagedBinary -Operation "doctor" -Arguments @("--doctor")
        if ($doctor -notmatch '(?m)^poqi doctor$' -or
            $doctor -notmatch 'configuration only: no login or network test was performed') {
            throw "Packaged binary returned unexpected doctor output."
        }
    }

    if (-not [string]::IsNullOrWhiteSpace($DatabaseUrl)) {
        $connection = Invoke-PackagedBinary -Operation "connection check" -Arguments @(
                "--check-connection", "--url", $DatabaseUrl, "--connect-timeout", "15"
            )
        if ($connection -notmatch 'Connection OK \(login and SELECT 1 succeeded\)\. No profile was saved\.') {
            throw "Packaged binary returned unexpected connection-check output."
        }
    }

    Write-Host "PASS: $packageName"
} finally {
    if (Test-Path -LiteralPath $smokeRoot) {
        $resolvedSmokeRoot = (Resolve-Path -LiteralPath $smokeRoot).Path
        if (-not $resolvedSmokeRoot.Equals($smokeRoot, $pathComparison) -or
            -not $resolvedSmokeRoot.StartsWith($expectedRootPrefix, $pathComparison) -or
            -not [IO.Path]::GetFileName($resolvedSmokeRoot).StartsWith("poqi-package-smoke-", [StringComparison]::Ordinal)) {
            throw "Refusing to remove an unexpected package smoke directory."
        }
        Remove-Item -LiteralPath $resolvedSmokeRoot -Recurse -Force
    }
}
