[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet(
        "x86_64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
        "x86_64-pc-windows-msvc"
    )]
    [string]$TargetTriple,

    [Parameter(Mandatory)]
    [string]$OutputPath,

    [string]$CargoAboutPath = "cargo-about",

    [string]$NativeDependencyManifestPath
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
$manifestPath = Join-Path $repoRoot "crates/app/Cargo.toml"
$aboutConfig = Join-Path $repoRoot "about.toml"
$aboutTemplate = Join-Path $repoRoot "about.hbs"
$nativeVersionsPath = if ([string]::IsNullOrWhiteSpace($NativeDependencyManifestPath)) {
    Join-Path $repoRoot "third-party/native-dependencies.json"
} else {
    $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($NativeDependencyManifestPath)
}
$nativeNoticesPath = Join-Path $repoRoot "third-party/native-licenses.txt"
$resolvedOutput = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputPath)
$outputDirectory = Split-Path -Parent $resolvedOutput

if (-not (Test-Path -LiteralPath $outputDirectory)) {
    New-Item -ItemType Directory -Path $outputDirectory | Out-Null
}

$cargoAbout = Get-Command -Name $CargoAboutPath -CommandType Application -ErrorAction Stop
$cargoAboutVersion = (& $cargoAbout.Source --version | Out-String).Trim()
if ($LASTEXITCODE -ne 0) {
    throw "Could not determine the cargo-about version."
}
if ($cargoAboutVersion -cne "cargo-about 0.9.1") {
    throw "Release notices require cargo-about 0.9.1; found '$cargoAboutVersion'."
}

$metadataJson = & cargo metadata --format-version 1 --locked --manifest-path $manifestPath
if ($LASTEXITCODE -ne 0) {
    throw "cargo metadata failed while checking native dependency versions."
}
$metadata = $metadataJson | ConvertFrom-Json
$expectedNativeDependencies = Get-Content -LiteralPath $nativeVersionsPath -Raw | ConvertFrom-Json

foreach ($expected in $expectedNativeDependencies) {
    $matches = @($metadata.packages | Where-Object { $_.name -ceq $expected.package })
    if ($matches.Count -ne 1 -or $matches[0].version -cne $expected.version) {
        $found = @($matches | ForEach-Object { $_.version }) -join ", "
        if ([string]::IsNullOrEmpty($found)) { $found = "not present" }
        throw "Native notices expect $($expected.package) $($expected.version); found $found. Review and update the vendored notices before releasing."
    }

    $packageRoot = Split-Path -Parent $matches[0].manifest_path
    foreach ($sourceFile in $expected.source_files) {
        $sourcePath = Join-Path $packageRoot $sourceFile.path
        if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
            throw "Native notice source is missing: $($expected.package) $($sourceFile.path)."
        }
        $actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $sourcePath).Hash
        if ($actualHash -cne $sourceFile.sha256) {
            throw "Native notice source changed: $($expected.package) $($sourceFile.path). Review and update the vendored notices before releasing."
        }
    }
}

$utf8WithoutBom = [Text.UTF8Encoding]::new($false)
[IO.File]::WriteAllText($resolvedOutput, "", $utf8WithoutBom)
try {
    $aboutArguments = @(
        "generate",
        "--config", $aboutConfig,
        "--manifest-path", $manifestPath,
        "--target", $TargetTriple,
        "--locked",
        "--fail",
        "--output-file", $resolvedOutput,
        $aboutTemplate
    )

    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $cargoAbout.Source
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in $aboutArguments) { $startInfo.ArgumentList.Add($argument) }

    $process = [Diagnostics.Process]::Start($startInfo)
    try {
        $standardOutput = $process.StandardOutput.ReadToEndAsync()
        $standardError = $process.StandardError.ReadToEndAsync()
        $process.WaitForExit()
        $outputText = $standardOutput.Result.Trim()
        $errorText = $standardError.Result.Trim()
        if ($process.ExitCode -ne 0) {
            throw "cargo-about failed to generate notices for $TargetTriple. $errorText"
        }
        if (-not [string]::IsNullOrEmpty($outputText)) { Write-Host $outputText }
        if (-not [string]::IsNullOrEmpty($errorText)) { Write-Warning $errorText }
    } finally {
        $process.Dispose()
    }

    $rustNotices = [IO.File]::ReadAllText($resolvedOutput).TrimEnd()
    $nativeNotices = [IO.File]::ReadAllText($nativeNoticesPath).Trim()
    [IO.File]::WriteAllText(
        $resolvedOutput,
        "$rustNotices`n`n$nativeNotices`n",
        $utf8WithoutBom
    )
} catch {
    if (Test-Path -LiteralPath $resolvedOutput) {
        Remove-Item -LiteralPath $resolvedOutput -Force
    }
    throw
}

Write-Host "Generated third-party notices for $TargetTriple at $resolvedOutput"
