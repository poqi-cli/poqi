$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$pwsh = (Get-Process -Id $PID).Path
$testVariables = @(
    "POQI_VERSION",
    "POQI_INSTALL_DIR",
    "POQI_INSTALLER_DRY_RUN"
)

$cases = @(
    @{
        Name = "primary"
        Env = @{ POQI_VERSION = "v0.0.0-primary"; POQI_INSTALL_DIR = "/tmp/poqi-primary"; POQI_INSTALLER_DRY_RUN = "1" }
        Version = "v0.0.0-primary"; ShDir = "/tmp/poqi-primary"; PsDir = "/tmp/poqi-primary"
    },
    @{
        Name = "blank primary directory"
        Env = @{
            POQI_VERSION = "v0.0.0-primary"; POQI_INSTALL_DIR = " "
            POQI_INSTALLER_DRY_RUN = "1"; HOME = "/tmp/poqi-home"; USERPROFILE = "/tmp/poqi-home"
        }
        Version = "v0.0.0-primary"; ShDir = "/tmp/poqi-home/.local/bin"
        PsDir = (Join-Path "/tmp/poqi-home" ".poqi\bin")
    }
)

function Invoke-Installer {
    param([string]$Name, [string]$FilePath, [string[]]$Arguments, [hashtable]$Environment, [string[]]$Expected)

    $start = [System.Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $FilePath
    $start.WorkingDirectory = $repoRoot
    $start.UseShellExecute = $false
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($argument in $Arguments) { $start.ArgumentList.Add($argument) }
    foreach ($variable in $testVariables) { [void]$start.Environment.Remove($variable) }
    foreach ($entry in $Environment.GetEnumerator()) { $start.Environment[$entry.Key] = $entry.Value }

    $process = [System.Diagnostics.Process]::Start($start)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) { throw "$Name failed ($($process.ExitCode)): $($stderr.Result)" }
    foreach ($text in $Expected) {
        if (-not $stdout.Result.Contains($text, [StringComparison]::Ordinal)) {
            throw "$Name did not print '$text'. Output: $($stdout.Result)"
        }
    }
    Write-Host "PASS: $Name"
}

foreach ($case in $cases) {
    $common = @{ Name = "PowerShell $($case.Name)"; FilePath = $pwsh; Arguments = @("-NoProfile", "-File", "scripts/install.ps1"); Environment = $case.Env }
    Invoke-Installer @common -Expected @("https://github.com/poqi-cli/poqi/releases/download/$($case.Version)/poqi-$($case.Version)-windows-x86_64.zip", "dir:     $($case.PsDir)")
}

if ($IsWindows) {
    Write-Host "SKIP: Unix installer smoke tests require a Unix host."
}
else {
    $sh = (Get-Command sh -ErrorAction Stop).Source
    foreach ($case in $cases) {
        Invoke-Installer -Name "Unix $($case.Name)" -FilePath $sh -Arguments @("scripts/install.sh") -Environment $case.Env `
            -Expected @("https://github.com/poqi-cli/poqi/releases/download/$($case.Version)/poqi-$($case.Version)-linux-x86_64.tar.gz", "dir:     $($case.ShDir)")
    }
}
