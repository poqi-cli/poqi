[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$PackageDirectory,

    [Parameter(Mandatory)]
    [string]$ReleaseTag,

    [Parameter(Mandatory)]
    [string]$OutputDirectory,

    [Parameter(Mandatory)]
    [string]$MakensisPath,

    [string]$TestNamespace
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Resolve-ApplicationPath {
    param([Parameter(Mandatory)][string]$Value)

    if (Test-Path -LiteralPath $Value -PathType Leaf) {
        return (Resolve-Path -LiteralPath $Value).Path
    }
    $command = Get-Command $Value -CommandType Application -ErrorAction Stop
    $command.Source
}

function ConvertTo-NsisDefinitionValue {
    param([Parameter(Mandatory)][string]$Value)

    if ($Value.IndexOfAny([char[]]@('"', "`r", "`n")) -ge 0) {
        throw "NSIS definition values cannot contain quotes or newlines."
    }
    $Value.Replace('$', '$$')
}

$versionMatch = [regex]::Match($ReleaseTag, '^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$')
if (-not $versionMatch.Success) {
    throw "ReleaseTag must use the form vX.Y.Z with numeric components."
}
$versionParts = 1..3 | ForEach-Object { [int]$versionMatch.Groups[$_].Value }
if (@($versionParts | Where-Object { $_ -gt 65535 }).Count -gt 0) {
    throw "ReleaseTag components must not exceed 65535 for Windows version metadata."
}
$version = ($versionParts -join '.')
$versionNumber = "$version.0"

if (-not [string]::IsNullOrEmpty($TestNamespace) -and $TestNamespace -notmatch '^[A-Za-z0-9._-]+$') {
    throw "TestNamespace may contain only ASCII letters, digits, dots, underscores and hyphens."
}

$packagePath = (Resolve-Path -LiteralPath $PackageDirectory -ErrorAction Stop).Path
if (-not (Test-Path -LiteralPath $packagePath -PathType Container)) {
    throw "PackageDirectory must be a directory."
}
$requiredFiles = @(
    "poqi.exe",
    "LICENSE",
    "THIRD-PARTY-LICENSES.txt",
    "README.md",
    "CHANGELOG.md"
)
foreach ($fileName in $requiredFiles) {
    $filePath = Join-Path $packagePath $fileName
    if (-not (Test-Path -LiteralPath $filePath -PathType Leaf)) {
        throw "PackageDirectory is missing required file: $fileName"
    }
    if ((Get-Item -LiteralPath $filePath).Length -eq 0) {
        throw "Required package file is empty: $fileName"
    }
}

$compilerPath = Resolve-ApplicationPath $MakensisPath
$outputPath = [IO.Path]::GetFullPath($OutputDirectory)
[void](New-Item -ItemType Directory -Path $outputPath -Force)
$artifactName = "poqi-$ReleaseTag-windows-x86_64-setup.exe"
$artifactPath = Join-Path $outputPath $artifactName
if (Test-Path -LiteralPath $artifactPath) {
    Remove-Item -LiteralPath $artifactPath -Force
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$scriptPath = Join-Path $repoRoot "packaging\windows\poqi.nsi"
$wrapperFiles = @(
    (Join-Path $repoRoot "packaging\windows\update-path.ps1"),
    (Join-Path $repoRoot "packaging\windows\NSIS-LICENSE.txt")
)
$estimatedSize = [math]::Ceiling((($requiredFiles | ForEach-Object {
                    (Get-Item -LiteralPath (Join-Path $packagePath $_)).Length
                } | Measure-Object -Sum).Sum + (($wrapperFiles | ForEach-Object {
                        (Get-Item -LiteralPath $_).Length
                    } | Measure-Object -Sum).Sum)) / 1KB)

$arguments = @(
    "/V4",
    "/WX",
    "/NOCD",
    "/DPRODUCT_VERSION=$(ConvertTo-NsisDefinitionValue $version)",
    "/DPRODUCT_VERSION_NUMBER=$(ConvertTo-NsisDefinitionValue $versionNumber)",
    "/DPACKAGE_DIR=$(ConvertTo-NsisDefinitionValue $packagePath)",
    "/DOUTPUT_FILE=$(ConvertTo-NsisDefinitionValue $artifactPath)",
    "/DESTIMATED_SIZE_KB=$estimatedSize"
)
if (-not [string]::IsNullOrEmpty($TestNamespace)) {
    $arguments += "/DPOQI_TEST_NAMESPACE=$(ConvertTo-NsisDefinitionValue $TestNamespace)"
}
$arguments += $scriptPath

& $compilerPath @arguments
if ($LASTEXITCODE -ne 0) {
    throw "makensis failed with exit code $LASTEXITCODE."
}
if (-not (Test-Path -LiteralPath $artifactPath -PathType Leaf) -or (Get-Item -LiteralPath $artifactPath).Length -eq 0) {
    throw "makensis did not create the expected installer: $artifactPath"
}

Write-Output $artifactPath
