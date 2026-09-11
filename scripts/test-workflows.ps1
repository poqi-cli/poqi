param(
    [string]$WorkflowRoot = (Join-Path (Split-Path -Parent $PSScriptRoot) ".github/workflows")
)

$ErrorActionPreference = "Stop"
$failures = [Collections.Generic.List[string]]::new()

foreach ($workflow in Get-ChildItem -LiteralPath $WorkflowRoot -File | Where-Object Extension -in ".yml", ".yaml") {
    $lineNumber = 0
    foreach ($line in Get-Content -LiteralPath $workflow.FullName) {
        $lineNumber++
        if ($line -match '^\s*-?\s*uses:\s*(?<reference>[^\s#]+)') {
            $reference = $Matches.reference
            if (-not $reference.StartsWith("./", [StringComparison]::Ordinal)) {
                $isImmutable = if ($reference.StartsWith("docker://", [StringComparison]::Ordinal)) {
                    $reference -cmatch '@sha256:[0-9a-f]{64}$'
                } else {
                    $separator = $reference.LastIndexOf('@')
                    $revision = if ($separator -ge 0) { $reference.Substring($separator + 1) } else { "" }
                    $revision -cmatch '^[0-9a-f]{40}$'
                }
                if (-not $isImmutable) {
                    $failures.Add("$($workflow.Name):$lineNumber external action is not pinned to a full commit: $reference")
                }
            }
        }

        foreach ($imageReference in @(
            [regex]::Match($line, '^\s*POQI_TESTCONTAINERS_PG_TAG:\s*"?(?<reference>[^"\s]+)').Groups["reference"].Value,
            [regex]::Match($line, '(?:^|\s)postgres:(?<reference>[^\s"'']+)').Groups["reference"].Value
        )) {
            if ($imageReference -and $imageReference -cnotmatch '@sha256:[0-9a-f]{64}$') {
                $failures.Add("$($workflow.Name):$lineNumber container image is not pinned to a digest: $imageReference")
            }
        }
    }
}

if ($failures.Count -ne 0) {
    throw "Workflow dependency policy failed:`n$($failures -join "`n")"
}

Write-Host "PASS: workflow actions and container images use immutable revisions"
