$ErrorActionPreference = "Stop"

$Repo = "poqi-cli/poqi"
$Version = if (Test-Path Env:POQI_VERSION) { $env:POQI_VERSION } else { "latest" }
if ([string]::IsNullOrWhiteSpace($Version)) {
    throw "POQI_VERSION must not be empty when set."
}

$InstallDir = if (Test-Path Env:POQI_INSTALL_DIR) { $env:POQI_INSTALL_DIR } else { $null }
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Join-Path $env:USERPROFILE ".poqi\bin"
}

$DryRun = if (Test-Path Env:POQI_INSTALLER_DRY_RUN) { $env:POQI_INSTALLER_DRY_RUN } else { "0" }
if ([string]::IsNullOrWhiteSpace($DryRun)) {
    $DryRun = "0"
}

if ($Version -eq "latest") {
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Version = $release.tag_name
}

if ([string]::IsNullOrWhiteSpace($Version)) {
    throw "Could not resolve latest release version."
}

$Target = "windows-x86_64"
$Asset = "poqi-$Version-$Target.zip"
$BaseUrl = "https://github.com/$Repo/releases/download/$Version"

if ($DryRun -eq "1" -or $DryRun -eq "true") {
    Write-Host "poqi installer dry run"
    Write-Host "  version: $Version"
    Write-Host "  target:  $Target"
    Write-Host "  asset:   $Asset"
    Write-Host "  url:     $BaseUrl/$Asset"
    Write-Host "  dir:     $InstallDir"
    exit 0
}

$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) "poqi-install-$([System.Guid]::NewGuid())"

New-Item -ItemType Directory -Path $TempDir | Out-Null

try {
    $ArchivePath = Join-Path $TempDir $Asset
    $ChecksumsPath = Join-Path $TempDir "SHA256SUMS.txt"

    Write-Host "Installing poqi $Version for $Target"
    Invoke-WebRequest -Uri "$BaseUrl/$Asset" -OutFile $ArchivePath
    Invoke-WebRequest -Uri "$BaseUrl/SHA256SUMS.txt" -OutFile $ChecksumsPath

    $escapedAsset = [Regex]::Escape($Asset)
    $checksumLine = Get-Content $ChecksumsPath | Where-Object { $_ -match "\s$escapedAsset$" } | Select-Object -First 1
    if ([string]::IsNullOrWhiteSpace($checksumLine)) {
        throw "Could not find checksum for $Asset."
    }

    $expected = ($checksumLine -split "\s+")[0].ToLowerInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 -Path $ArchivePath).Hash.ToLowerInvariant()
    if ($expected -ne $actual) {
        throw "Checksum mismatch for $Asset."
    }

    Expand-Archive -Path $ArchivePath -DestinationPath $TempDir -Force
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item -Path (Join-Path $TempDir "poqi-$Version-$Target\poqi.exe") -Destination (Join-Path $InstallDir "poqi.exe") -Force

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ([string]::IsNullOrWhiteSpace($userPath)) {
        $userPath = ""
    }

    $pathParts = $userPath -split ";" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    if ($pathParts -notcontains $InstallDir) {
        $newUserPath = if ([string]::IsNullOrWhiteSpace($userPath)) { $InstallDir } else { "$userPath;$InstallDir" }
        [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
        $env:Path = "$env:Path;$InstallDir"
        Write-Host "Added $InstallDir to your user PATH. Open a new terminal if poqi is not found."
    }

    Write-Host "Installed poqi to $(Join-Path $InstallDir "poqi.exe")"
    Write-Host "Run: poqi --help"
}
finally {
    Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue
}
