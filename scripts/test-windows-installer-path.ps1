$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Split-Path -Parent $PSScriptRoot
$helperPath = Join-Path $repoRoot "packaging\windows\update-path.ps1"
$powershellPath = (Get-Process -Id $PID).Path
$namespace = "unit-$([Guid]::NewGuid().ToString('N'))"
$testRoot = "Software\poqi-installer-tests\$namespace"
$pathSubKey = "$testRoot\Environment"
$ownerSubKey = "$testRoot\Installer"

function Assert-Equal {
    param($Expected, $Actual, [string]$Message)
    if ($Expected -ne $Actual) {
        throw "$Message Expected '$Expected', got '$Actual'."
    }
}

function Invoke-PathHelper {
    param(
        [Parameter(Mandatory)][ValidateSet("Add", "Remove")][string]$Operation,
        [Parameter(Mandatory)][string]$InstallDirectory,
        [int[]]$AllowedExitCodes = @(0, 10)
    )

    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $powershellPath
    $start.UseShellExecute = $false
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.ArgumentList.Add("-NoLogo")
    $start.ArgumentList.Add("-NoProfile")
    $start.ArgumentList.Add("-NonInteractive")
    $start.ArgumentList.Add("-File")
    $start.ArgumentList.Add($helperPath)
    $start.Environment["POQI_PATH_OPERATION"] = $Operation
    $start.Environment["POQI_PATH_INSTALL_DIRECTORY"] = $InstallDirectory
    $start.Environment["POQI_PATH_REGISTRY_SUBKEY"] = $pathSubKey
    $start.Environment["POQI_PATH_OWNERSHIP_SUBKEY"] = $ownerSubKey

    $process = [Diagnostics.Process]::Start($start)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    if ($AllowedExitCodes -notcontains $process.ExitCode) {
        throw "PATH helper $Operation failed ($($process.ExitCode)): $($stderr.Result)$($stdout.Result)"
    }
    $process.ExitCode
}

function Reset-TestKeys {
    $root = [Microsoft.Win32.Registry]::CurrentUser
    try {
        $root.DeleteSubKeyTree($testRoot, $false)
    }
    catch [ArgumentException] {
        # The key is already absent.
    }
}

function Open-TestKey {
    param([Parameter(Mandatory)][string]$SubKey)
    [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($SubKey, $true)
}

try {
    $installDirectory = "C:\Program Files\poqi test ü & [stable]"

    Reset-TestKeys
    $key = Open-TestKey $pathSubKey
    try {
        $original = 'C:\Windows;%USERPROFILE%\tools;;C:\Other;'
        $key.SetValue("Path", $original, [Microsoft.Win32.RegistryValueKind]::ExpandString)
    }
    finally { $key.Dispose() }

    Assert-Equal 10 (Invoke-PathHelper Add $installDirectory) "The first add should report a change."
    Assert-Equal 0 (Invoke-PathHelper Add $installDirectory) "A repeated add should be idempotent."
    $key = Open-TestKey $pathSubKey
    try {
        Assert-Equal "$original;$installDirectory" ([string]$key.GetValue("Path", $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)) "Add should preserve every existing PATH token and delimiter."
        Assert-Equal ([Microsoft.Win32.RegistryValueKind]::ExpandString) ($key.GetValueKind("Path")) "Add should preserve ExpandString."
    }
    finally { $key.Dispose() }

    $owner = Open-TestKey $ownerSubKey
    try { $owner.SetValue("UnrelatedValue", "keep", [Microsoft.Win32.RegistryValueKind]::String) }
    finally { $owner.Dispose() }

    Assert-Equal 10 (Invoke-PathHelper Remove $installDirectory) "Owned removal should report a change."
    $key = Open-TestKey $pathSubKey
    try {
        Assert-Equal $original ([string]$key.GetValue("Path", $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)) "Remove should restore the original PATH text."
        Assert-Equal ([Microsoft.Win32.RegistryValueKind]::ExpandString) ($key.GetValueKind("Path")) "Remove should preserve ExpandString."
    }
    finally { $key.Dispose() }
    $owner = Open-TestKey $ownerSubKey
    try { Assert-Equal "keep" ([string]$owner.GetValue("UnrelatedValue")) "Uninstall should preserve unrelated ownership-key values." }
    finally { $owner.Dispose() }
    Write-Host "PASS: add, reinstall and owned removal preserve the raw PATH"

    Reset-TestKeys
    $key = Open-TestKey $pathSubKey
    try {
        $preExisting = "C:\One;$installDirectory;C:\Two"
        $key.SetValue("Path", $preExisting, [Microsoft.Win32.RegistryValueKind]::String)
    }
    finally { $key.Dispose() }
    Assert-Equal 0 (Invoke-PathHelper Add $installDirectory) "A pre-existing entry should remain user-owned."
    Assert-Equal 0 (Invoke-PathHelper Remove $installDirectory) "Uninstall should not remove a pre-existing entry."
    $key = Open-TestKey $pathSubKey
    try {
        Assert-Equal $preExisting ([string]$key.GetValue("Path")) "A pre-existing PATH entry should remain unchanged."
        Assert-Equal ([Microsoft.Win32.RegistryValueKind]::String) ($key.GetValueKind("Path")) "String kind should remain unchanged."
    }
    finally { $key.Dispose() }
    Write-Host "PASS: pre-existing PATH entries remain user-owned"

    Reset-TestKeys
    Assert-Equal 10 (Invoke-PathHelper Add $installDirectory) "Adding to a missing PATH should report a change."
    Assert-Equal 10 (Invoke-PathHelper Remove $installDirectory) "Removing the sole owned entry should report a change."
    $key = Open-TestKey $pathSubKey
    try {
        Assert-Equal $false (@($key.GetValueNames()) -contains "Path") "A PATH value created by poqi should be removed, not left empty."
    }
    finally { $key.Dispose() }
    Write-Host "PASS: uninstall restores an originally missing PATH value"

    Reset-TestKeys
    $key = Open-TestKey $pathSubKey
    try { $key.SetValue("Path", "C:\One", [Microsoft.Win32.RegistryValueKind]::String) }
    finally { $key.Dispose() }
    $owner = Open-TestKey $ownerSubKey
    try { $owner.SetValue("PathEntry", "C:\A different poqi", [Microsoft.Win32.RegistryValueKind]::String) }
    finally { $owner.Dispose() }
    Assert-Equal 1 (Invoke-PathHelper Add $installDirectory -AllowedExitCodes @(1)) "A conflicting ownership marker should fail."
    $key = Open-TestKey $pathSubKey
    try { Assert-Equal "C:\One" ([string]$key.GetValue("Path")) "A conflict must not modify PATH." }
    finally { $key.Dispose() }
    Write-Host "PASS: conflicting installer ownership fails without modifying PATH"

    Reset-TestKeys
    Assert-Equal 1 (Invoke-PathHelper Add "C:\unsafe;path" -AllowedExitCodes @(1)) "A semicolon path should fail."
    Write-Host "PASS: paths that cannot be represented safely are rejected"
}
finally {
    Reset-TestKeys
}
