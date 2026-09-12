[CmdletBinding()]
param([Parameter(Mandatory)][string]$DestinationDirectory)

$ErrorActionPreference = 'Stop'
$version = '3.12'
# NSIS-Dev/scoop-nsis bucket/nsis-3.12.json (official maintainer manifest).
$sha256 = '56581f90db321581c5381193d796fffcf2d24b2f8fed2160a6c6a3baa67f2c4f'
$destination = [IO.Path]::GetFullPath($DestinationDirectory)
[void](New-Item -ItemType Directory -Path $destination -Force)
$archive = Join-Path $destination "nsis-$version.zip"
$urls = @(
    "https://downloads.sourceforge.net/project/nsis/NSIS%203/$version/nsis-$version.zip",
    "https://mirrors.mit.edu/macports/distfiles/nsis/nsis-$version.zip"
)
$verified = $false
foreach ($url in $urls) {
    try {
        Invoke-WebRequest -Uri $url -OutFile $archive -TimeoutSec 120
        if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ine $sha256) {
            throw 'NSIS archive checksum mismatch.'
        }
        $verified = $true
        break
    } catch {
        Write-Warning "NSIS download failed verification from ${url}: $($_.Exception.Message)"
    }
}
if (-not $verified) { throw 'Could not obtain the pinned NSIS compiler.' }
Expand-Archive -LiteralPath $archive -DestinationPath $destination -Force
$compiler = Join-Path $destination "nsis-$version/makensis.exe"
if (-not (Test-Path -LiteralPath $compiler -PathType Leaf)) { throw 'NSIS compiler is missing.' }
Write-Output $compiler
