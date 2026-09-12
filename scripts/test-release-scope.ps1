param(
    [string]$WorkflowPath = (Join-Path (Split-Path -Parent $PSScriptRoot) ".github/workflows/release.yml")
)

$ErrorActionPreference = "Stop"

$expectedIgnoredPaths = @(
    "website/**"
    "docs/**"
    "**/*.md"
    ".github/workflows/website.yml"
    ".github/workflows/release.yml"
    "scripts/test-release-scope.ps1"
)

$workflowLines = Get-Content -LiteralPath $WorkflowPath
$pathsIgnoreLine = [Array]::FindIndex($workflowLines, [Predicate[string]] { param($line) $line -match '^\s+paths-ignore:\s*$' })
if ($pathsIgnoreLine -lt 0) {
    throw "release.yml must define pull_request paths-ignore"
}

$actualIgnoredPaths = [System.Collections.Generic.List[string]]::new()
for ($lineNumber = $pathsIgnoreLine + 1; $lineNumber -lt $workflowLines.Count; $lineNumber++) {
    if ($workflowLines[$lineNumber] -notmatch '^\s+-\s+[''"]?([^''"]+)[''"]?\s*$') {
        break
    }
    $actualIgnoredPaths.Add($Matches[1])
}

if ($actualIgnoredPaths.Count -ne $expectedIgnoredPaths.Count -or
    (Compare-Object -ReferenceObject $expectedIgnoredPaths -DifferenceObject $actualIgnoredPaths)) {
    throw "release.yml paths-ignore must contain exactly: $($expectedIgnoredPaths -join ', ')"
}

function Test-IsIgnoredReleasePath {
    param([Parameter(Mandatory)][string]$Path)

    return $Path -in @(".github/workflows/website.yml", ".github/workflows/release.yml", "scripts/test-release-scope.ps1") -or
        $Path.StartsWith("website/", [StringComparison]::Ordinal) -or
        $Path.StartsWith("docs/", [StringComparison]::Ordinal) -or
        $Path.EndsWith(".md", [StringComparison]::Ordinal)
}

function Test-StartsRelease {
    param([Parameter(Mandatory)][string[]]$ChangedPaths)

    return [bool]($ChangedPaths | Where-Object { -not (Test-IsIgnoredReleasePath -Path $_) } | Select-Object -First 1)
}

$cases = @(
    @{ Name = "website only"; Paths = @("website/src/pages/index.astro"); Expected = $false }
    @{ Name = "documentation only"; Paths = @("docs/connections.md", "README.md"); Expected = $false }
    @{ Name = "website workflow only"; Paths = @(".github/workflows/website.yml"); Expected = $false }
    @{ Name = "nested Markdown only"; Paths = @("crates/app/README.md"); Expected = $false }
    @{ Name = "website and Rust"; Paths = @("website/src/pages/index.astro", "crates/app/src/main.rs"); Expected = $true }
    @{ Name = "documentation and Cargo"; Paths = @("docs/connections.md", "Cargo.lock"); Expected = $true }
    @{ Name = "installer packaging"; Paths = @("scripts/install.sh"); Expected = $true }
    @{ Name = "license package input"; Paths = @("LICENSE"); Expected = $true }
    @{ Name = "release workflow only"; Paths = @(".github/workflows/release.yml"); Expected = $false }
    @{ Name = "release scope test only"; Paths = @("scripts/test-release-scope.ps1"); Expected = $false }
    @{ Name = "first website setup"; Paths = @("website/package.json", "README.md", ".github/workflows/website.yml", ".github/workflows/release.yml", "scripts/test-release-scope.ps1"); Expected = $false }
    @{ Name = "release workflow and Cargo"; Paths = @(".github/workflows/release.yml", "Cargo.lock"); Expected = $true }
    @{ Name = "release scope test and license"; Paths = @("scripts/test-release-scope.ps1", "LICENSE"); Expected = $true }
)

$failures = [System.Collections.Generic.List[string]]::new()
foreach ($case in $cases) {
    $actual = Test-StartsRelease -ChangedPaths $case.Paths
    if ($actual -ne $case.Expected) {
        $failures.Add("$($case.Name): expected release=$($case.Expected), got release=$actual")
    }
}

if ($failures.Count -gt 0) {
    $failures | ForEach-Object { Write-Error $_ }
    exit 1
}

Write-Host "PASS: release scope excludes website/documentation-only changes and includes application/package changes"
