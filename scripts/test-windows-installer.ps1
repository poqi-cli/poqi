[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$PackageDirectory,

    [Parameter(Mandatory)]
    [string]$ReleaseTag,

    [Parameter(Mandatory)]
    [string]$MakensisPath,

    [Parameter(Mandatory)]
    [string]$WorkDirectory
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw "The Windows installer lifecycle test must run on Windows."
}

$versionMatch = [regex]::Match($ReleaseTag, '^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$')
if (-not $versionMatch.Success) {
    throw "ReleaseTag must use the form vX.Y.Z with numeric components."
}
$expectedVersion = $ReleaseTag.Substring(1)

function Assert-True {
    param([bool]$Condition, [Parameter(Mandatory)][string]$Message)
    if (-not $Condition) { throw $Message }
}

function Assert-Equal {
    param($Expected, $Actual, [Parameter(Mandatory)][string]$Message)
    if ($Expected -ne $Actual) { throw $Message }
}

function Assert-SafeChildPath {
    param(
        [Parameter(Mandatory)][string]$Parent,
        [Parameter(Mandatory)][string]$Child
    )

    $parentPath = [IO.Path]::GetFullPath($Parent).TrimEnd('\', '/')
    $childPath = [IO.Path]::GetFullPath($Child)
    $prefix = "$parentPath$([IO.Path]::DirectorySeparatorChar)"
    if (-not $childPath.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to operate outside the installer test work directory."
    }
}

function Remove-TestDirectory {
    param(
        [Parameter(Mandatory)][string]$Parent,
        [Parameter(Mandatory)][string]$Path
    )

    Assert-SafeChildPath -Parent $Parent -Child $Path
    if (-not (Test-Path -LiteralPath $Path)) { return }
    $item = Get-Item -LiteralPath $Path -Force
    Assert-True -Condition $item.PSIsContainer -Message "Refusing to remove a non-directory test path."
    Assert-True -Condition (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0) `
        -Message "Refusing to recursively remove a test directory that is a reparse point."
    $reparsePoints = @(Get-ChildItem -LiteralPath $Path -Recurse -Force |
            Where-Object { ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 })
    Assert-Equal 0 $reparsePoints.Count "Refusing to recursively remove a test directory containing a reparse point."
    Remove-Item -LiteralPath $Path -Recurse -Force
}

function Get-RegistryValueState {
    param(
        [Parameter(Mandatory)][string]$SubKey,
        [Parameter(Mandatory)][string]$Name
    )

    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($SubKey, $false)
    if ($null -eq $key) {
        return [pscustomobject]@{ Exists = $false; Value = $null; Kind = $null }
    }
    try {
        if (@($key.GetValueNames()) -notcontains $Name) {
            return [pscustomobject]@{ Exists = $false; Value = $null; Kind = $null }
        }
        [pscustomobject]@{
            Exists = $true
            Value = $key.GetValue($Name, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            Kind = $key.GetValueKind($Name)
        }
    }
    finally {
        $key.Dispose()
    }
}

function Get-RegistryKeyFingerprint {
    param([Parameter(Mandatory)][string]$SubKey)

    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($SubKey, $false)
    if ($null -eq $key) { return "absent" }
    try {
        $parts = [Collections.Generic.List[string]]::new()
        foreach ($name in @($key.GetValueNames() | Sort-Object)) {
            $kind = $key.GetValueKind($name)
            $value = $key.GetValue($name, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            if ($value -is [byte[]]) {
                $text = [Convert]::ToBase64String($value)
            }
            elseif ($value -is [string[]]) {
                $text = $value -join "`0"
            }
            else {
                $text = [string]$value
            }
            $parts.Add("$name`0$kind`0$text")
        }
        $bytes = [Text.Encoding]::UTF8.GetBytes($parts -join "`n")
        $sha256 = [Security.Cryptography.SHA256]::Create()
        try { [Convert]::ToHexString($sha256.ComputeHash($bytes)) }
        finally { $sha256.Dispose() }
    }
    finally {
        $key.Dispose()
    }
}

function Get-DirectoryFingerprint {
    param([Parameter(Mandatory)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Container)) { return "absent" }
    $root = (Resolve-Path -LiteralPath $Path).Path.TrimEnd('\', '/')
    $parts = [Collections.Generic.List[string]]::new()
    foreach ($item in @(Get-ChildItem -LiteralPath $root -Recurse -Force | Sort-Object FullName)) {
        $relative = $item.FullName.Substring($root.Length).TrimStart('\', '/')
        if ($item.PSIsContainer) {
            $parts.Add("D`0$relative")
        }
        else {
            $parts.Add("F`0$relative`0$((Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash)")
        }
    }
    $bytes = [Text.Encoding]::UTF8.GetBytes($parts -join "`n")
    $sha256 = [Security.Cryptography.SHA256]::Create()
    try { [Convert]::ToHexString($sha256.ComputeHash($bytes)) }
    finally { $sha256.Dispose() }
}

function Reset-TestRegistry {
    param([Parameter(Mandatory)][string]$SubKey)

    Assert-True -Condition ($SubKey -match '^Software\\poqi-installer-tests\\[0-9a-f]{32}$') `
        -Message "Refusing to remove an unexpected registry namespace."
    try {
        [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKeyTree($SubKey, $false)
    }
    catch [ArgumentException] {
        # The unique test namespace is already absent.
    }
}

function Set-TestPath {
    param(
        [Parameter(Mandatory)][string]$SubKey,
        [Parameter(Mandatory)][string]$Value
    )

    $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($SubKey, $true)
    if ($null -eq $key) { throw "Could not create the isolated PATH registry key." }
    try { $key.SetValue("Path", $Value, [Microsoft.Win32.RegistryValueKind]::ExpandString) }
    finally { $key.Dispose() }
}

function Get-TestPath {
    param([Parameter(Mandatory)][string]$SubKey)
    Get-RegistryValueState -SubKey $SubKey -Name "Path"
}

function Invoke-HiddenProcess {
    param(
        [Parameter(Mandatory)][string]$FilePath,
        [Parameter(Mandatory)][string]$Arguments,
        [Parameter(Mandatory)][string]$Operation,
        [int[]]$AllowedExitCodes = @(0),
        [int]$TimeoutMilliseconds = 120000
    )

    $process = Start-Process -FilePath $FilePath -ArgumentList $Arguments -WindowStyle Hidden -PassThru
    try {
        if (-not $process.WaitForExit($TimeoutMilliseconds)) {
            try { $process.Kill($true) } catch [InvalidOperationException] { }
            [void]$process.WaitForExit(5000)
            throw "$Operation timed out."
        }
        if ($AllowedExitCodes -notcontains $process.ExitCode) {
            throw "$Operation failed with exit code $($process.ExitCode)."
        }
        $process.ExitCode
    }
    finally {
        $process.Dispose()
    }
}

function Wait-NormalUninstallCompletion {
    param(
        [Parameter(Mandatory)][string]$InstallDirectory,
        [Parameter(Mandatory)][string]$ArpSubKey,
        [Parameter(Mandatory)][string[]]$OwnedRelativePaths,
        [int]$TimeoutMilliseconds = 30000
    )

    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    do {
        $remainingOwnedFiles = @($OwnedRelativePaths | Where-Object {
                Test-Path -LiteralPath (Join-Path $InstallDirectory $_)
            })
        if ($remainingOwnedFiles.Count -eq 0 -and
            (Get-RegistryKeyFingerprint -SubKey $ArpSubKey) -eq "absent" -and
            -not (Test-Path -LiteralPath $InstallDirectory)) {
            return
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)

    throw "Normal-mode uninstall did not finish removing its owned files and registration within the timeout."
}

function Invoke-InstalledPoqi {
    param(
        [Parameter(Mandatory)][string]$BinaryPath,
        [Parameter(Mandatory)][string[]]$Arguments,
        [Parameter(Mandatory)][string]$Operation,
        [Parameter(Mandatory)][string]$IsolatedHome
    )

    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $BinaryPath
    foreach ($argument in $Arguments) { $start.ArgumentList.Add($argument) }
    $start.WorkingDirectory = $IsolatedHome
    $start.UseShellExecute = $false
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.Environment["HOME"] = $IsolatedHome
    $start.Environment["USERPROFILE"] = $IsolatedHome
    $start.Environment["APPDATA"] = Join-Path $IsolatedHome "AppData"
    $start.Environment["XDG_CONFIG_HOME"] = Join-Path $IsolatedHome ".config"
    $start.Environment["POQI_CONFIG_DIR"] = Join-Path $IsolatedHome ".config\poqi"
    [void]$start.Environment.Remove("POQI_DATABASE_URL")

    $process = [Diagnostics.Process]::Start($start)
    try {
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(30000)) {
            try { $process.Kill($true) } catch [InvalidOperationException] { }
            [void]$process.WaitForExit(5000)
            throw "Installed poqi $Operation timed out."
        }
        if ($process.ExitCode -ne 0) {
            [void]$stderr.Result
            throw "Installed poqi $Operation failed with exit code $($process.ExitCode)."
        }
        $stdout.Result
    }
    finally {
        $process.Dispose()
    }
}

function Assert-TestPath {
    param(
        [Parameter(Mandatory)][string]$SubKey,
        [Parameter(Mandatory)][string]$Expected,
        [Parameter(Mandatory)][string]$Message
    )

    $state = Get-TestPath -SubKey $SubKey
    Assert-True $state.Exists "$Message The PATH value is missing."
    Assert-Equal ([Microsoft.Win32.RegistryValueKind]::ExpandString) $state.Kind "$Message The registry kind changed."
    Assert-Equal $Expected ([string]$state.Value) $Message
}

$packagePath = (Resolve-Path -LiteralPath $PackageDirectory -ErrorAction Stop).Path
$repoRoot = Split-Path -Parent $PSScriptRoot
$buildScript = Join-Path $PSScriptRoot "build-windows-installer.ps1"
$workPath = [IO.Path]::GetFullPath($WorkDirectory)
$workRoot = [IO.Path]::GetPathRoot($workPath).TrimEnd('\', '/')
if ($workPath.TrimEnd('\', '/').Equals($workRoot, [StringComparison]::OrdinalIgnoreCase)) {
    throw "WorkDirectory must not be a drive root."
}
$createdWorkDirectory = -not (Test-Path -LiteralPath $workPath)
[void](New-Item -ItemType Directory -Path $workPath -Force)
$workPath = (Resolve-Path -LiteralPath $workPath).Path

$namespace = [Guid]::NewGuid().ToString('N')
$testRegistryRoot = "Software\poqi-installer-tests\$namespace"
$pathSubKey = "$testRegistryRoot\Environment"
$ownerSubKey = "$testRegistryRoot\Installer"
$arpSubKey = "$testRegistryRoot\Uninstall\poqi"
$installDirectory = Join-Path $workPath "install ü spaces"
$binaryPath = Join-Path $installDirectory "poqi.exe"
$shortcutPath = Join-Path $installDirectory "test-shortcuts\poqi.lnk"
$isolatedHome = Join-Path $workPath "isolated home"
$installerPath = Join-Path $workPath "poqi-$ReleaseTag-windows-x86_64-setup.exe"
$profileSentinel = Join-Path $installDirectory "profiles\preserve.profile"
$unrelatedFile = Join-Path $installDirectory "keep-unrelated.txt"
$outsideSentinel = Join-Path $workPath "outside-sentinel.txt"

foreach ($path in @($installDirectory, $isolatedHome, $installerPath, $outsideSentinel)) {
    Assert-SafeChildPath -Parent $workPath -Child $path
}
Assert-True -Condition (-not (Test-Path -LiteralPath $installerPath)) `
    -Message "The work directory already contains the expected test installer."
Assert-True -Condition (-not (Test-Path -LiteralPath $installDirectory)) `
    -Message "The work directory already contains the test install directory."

$productionPathBefore = Get-RegistryValueState -SubKey "Environment" -Name "Path"
$productionArpBefore = Get-RegistryKeyFingerprint -SubKey "Software\Microsoft\Windows\CurrentVersion\Uninstall\poqi"
$startMenuPoqi = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::Programs)) "poqi"
$productionShortcutBefore = Get-DirectoryFingerprint -Path $startMenuPoqi

$payloadFiles = @("poqi.exe", "LICENSE", "THIRD-PARTY-LICENSES.txt", "README.md", "CHANGELOG.md")
$ownedFiles = @($payloadFiles + @("update-path.ps1", "NSIS-LICENSE.txt", "Uninstall.exe"))
$selfDeletingOwnedFiles = @($ownedFiles | Where-Object { $_ -ne "Uninstall.exe" })
$longEntries = 0..180 | ForEach-Object { "C:\poqi-installer-sentinel\tool-$($_.ToString('D3'))" }
$originalRawPath = ((@("C:\Windows", '%USERPROFILE%\poqi-expandable-tools', "") + $longEntries) -join ';') + ';'
$expectedInstalledPath = "$originalRawPath;$installDirectory"

try {
    [void](New-Item -ItemType Directory -Path $isolatedHome -Force)
    [IO.File]::WriteAllText($outsideSentinel, "outside the install directory")
    Reset-TestRegistry -SubKey $testRegistryRoot
    Set-TestPath -SubKey $pathSubKey -Value $originalRawPath

    $null = & $buildScript -PackageDirectory $packagePath -ReleaseTag $ReleaseTag `
        -OutputDirectory $workPath -MakensisPath $MakensisPath -TestNamespace $namespace
    Assert-True -Condition (Test-Path -LiteralPath $installerPath -PathType Leaf) `
        -Message "The namespaced installer was not created at the expected path."

    [void](New-Item -ItemType Directory -Path $installDirectory)
    $collisionPath = Join-Path $installDirectory "LICENSE"
    [IO.File]::WriteAllText($collisionPath, "pre-existing collision sentinel")
    Assert-Equal 1 (Invoke-HiddenProcess -FilePath $installerPath -Arguments "/S /D=$installDirectory" `
            -Operation "Fresh-install collision" -AllowedExitCodes @(1)) `
        "A fresh install over an installer-owned filename should fail."
    Assert-Equal "pre-existing collision sentinel" ([IO.File]::ReadAllText($collisionPath)) `
        "A failed fresh install changed a pre-existing filename collision."
    Assert-TestPath -SubKey $pathSubKey -Expected $originalRawPath `
        -Message "A failed fresh install changed the isolated PATH."
    Assert-Equal "absent" (Get-RegistryKeyFingerprint -SubKey $arpSubKey) `
        "A failed fresh install created an isolated Add or Remove Programs key."
    foreach ($unexpectedFile in @("poqi.exe", "update-path.ps1", "NSIS-LICENSE.txt", "Uninstall.exe")) {
        Assert-True -Condition (-not (Test-Path -LiteralPath (Join-Path $installDirectory $unexpectedFile))) `
            -Message "A failed fresh install created $unexpectedFile."
    }
    Remove-TestDirectory -Parent $workPath -Path $installDirectory
    Write-Host "PASS: fresh install refuses an installer-owned filename collision"

    Assert-Equal 0 (Invoke-HiddenProcess -FilePath $installerPath -Arguments "/S /D=$installDirectory" `
            -Operation "Silent installer") "The initial installer run should succeed."
    Assert-TestPath -SubKey $pathSubKey -Expected $expectedInstalledPath `
        -Message "The initial install did not preserve and append to the raw isolated PATH exactly."
    Assert-Equal 1 (@(([string](Get-TestPath -SubKey $pathSubKey).Value).Split(';') |
                Where-Object { $_ -ieq $installDirectory })).Count `
        "The install directory must occur exactly once in the isolated PATH."

    foreach ($fileName in $payloadFiles) {
        $stagedHash = (Get-FileHash -LiteralPath (Join-Path $packagePath $fileName) -Algorithm SHA256).Hash
        $installedHash = (Get-FileHash -LiteralPath (Join-Path $installDirectory $fileName) -Algorithm SHA256).Hash
        Assert-Equal $stagedHash $installedHash "The installed $fileName does not match the staged package file."
    }
    foreach ($fileName in $ownedFiles) {
        Assert-True -Condition (Test-Path -LiteralPath (Join-Path $installDirectory $fileName) -PathType Leaf) `
            -Message "The installer did not create $fileName."
    }

    $versionOutput = (Invoke-InstalledPoqi -BinaryPath $binaryPath -Arguments @("--version") `
            -Operation "version check" -IsolatedHome $isolatedHome).Trim()
    Assert-Equal "poqi $expectedVersion" $versionOutput "The installed poqi version is incorrect."
    $helpOutput = Invoke-InstalledPoqi -BinaryPath $binaryPath -Arguments @("--help") `
        -Operation "help check" -IsolatedHome $isolatedHome
    Assert-True -Condition ($helpOutput -match '(?m)^Usage: poqi(?:\.exe)? ') `
        -Message "The installed poqi help output is unexpected."

    $arpKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($arpSubKey, $false)
    Assert-True -Condition ($null -ne $arpKey) -Message "The test Add or Remove Programs key is missing."
    try {
        Assert-Equal "poqi" ([string]$arpKey.GetValue("DisplayName")) "The registered display name is incorrect."
        Assert-Equal $expectedVersion ([string]$arpKey.GetValue("DisplayVersion")) "The registered version is incorrect."
        Assert-Equal $installDirectory ([string]$arpKey.GetValue("InstallLocation")) "The registered install location is incorrect."
    }
    finally { $arpKey.Dispose() }

    Assert-True -Condition (Test-Path -LiteralPath $shortcutPath -PathType Leaf) `
        -Message "The isolated installer shortcut is missing."
    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $null
    try {
        $shortcut = $shell.CreateShortcut($shortcutPath)
        $expectedCmd = Join-Path $env:SystemRoot "System32\cmd.exe"
        Assert-True -Condition ([string]::Equals($expectedCmd, $shortcut.TargetPath, [StringComparison]::OrdinalIgnoreCase)) `
            -Message "The shortcut target is not cmd.exe."
        Assert-Equal ('/D /K ""{0}""' -f $binaryPath) $shortcut.Arguments `
            "The shortcut does not quote the installed application path correctly."
    }
    finally {
        if ($null -ne $shortcut) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($shortcut) }
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($shell)
    }

    $preUpgradeHashes = @{}
    foreach ($fileName in $ownedFiles) {
        $preUpgradeHashes[$fileName] = (Get-FileHash -LiteralPath (Join-Path $installDirectory $fileName) -Algorithm SHA256).Hash
    }
    $preUpgradeArp = Get-RegistryKeyFingerprint -SubKey $arpSubKey
    $preUpgradeShortcut = (Get-FileHash -LiteralPath $shortcutPath -Algorithm SHA256).Hash
    $upgradeLock = [IO.File]::Open($binaryPath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
        Assert-Equal 1 (Invoke-HiddenProcess -FilePath $installerPath -Arguments "/S /D=$installDirectory" `
                -Operation "Locked-file upgrade" -AllowedExitCodes @(1)) `
            "Upgrade should fail while the installed application executable is locked."
        Assert-TestPath -SubKey $pathSubKey -Expected $expectedInstalledPath `
            -Message "A failed upgrade changed the raw isolated PATH."
        Assert-Equal $preUpgradeArp (Get-RegistryKeyFingerprint -SubKey $arpSubKey) `
            "A failed upgrade changed the isolated Add or Remove Programs registration."
        Assert-Equal $preUpgradeShortcut (Get-FileHash -LiteralPath $shortcutPath -Algorithm SHA256).Hash `
            "A failed upgrade changed the isolated shortcut."
        foreach ($fileName in $ownedFiles) {
            Assert-Equal $preUpgradeHashes[$fileName] `
                (Get-FileHash -LiteralPath (Join-Path $installDirectory $fileName) -Algorithm SHA256).Hash `
                "A failed upgrade changed previously installed file $fileName."
        }
    }
    finally {
        $upgradeLock.Dispose()
    }

    Assert-Equal 0 (Invoke-HiddenProcess -FilePath $installerPath -Arguments "/S /D=$installDirectory" `
            -Operation "Silent upgrade") "The upgrade installer run should succeed after the file lock is released."
    Assert-TestPath -SubKey $pathSubKey -Expected $expectedInstalledPath `
        -Message "The upgrade changed the raw isolated PATH unexpectedly."
    Assert-Equal 1 (@(([string](Get-TestPath -SubKey $pathSubKey).Value).Split(';') |
                Where-Object { $_ -ieq $installDirectory })).Count `
        "The upgrade duplicated the install directory in the isolated PATH."

    $ownerKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($ownerSubKey, $true)
    Assert-True -Condition ($null -ne $ownerKey) -Message "The installed PATH ownership key is missing."
    try { $ownerKey.SetValue("UnrelatedSentinel", "preserve", [Microsoft.Win32.RegistryValueKind]::String) }
    finally { $ownerKey.Dispose() }

    [void](New-Item -ItemType Directory -Path (Split-Path -Parent $profileSentinel) -Force)
    [IO.File]::WriteAllText($profileSentinel, "profile sentinel")
    [IO.File]::WriteAllText($unrelatedFile, "unrelated sentinel")
    $uninstallerPath = Join-Path $installDirectory "Uninstall.exe"

    $binaryLock = [IO.File]::Open($binaryPath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
        Assert-Equal 1 (Invoke-HiddenProcess -FilePath $uninstallerPath -Arguments "/S _?=$installDirectory" `
                -Operation "Locked-file uninstaller" -AllowedExitCodes @(1)) `
            "Uninstall should fail while the application executable is locked."
        Assert-TestPath -SubKey $pathSubKey -Expected $expectedInstalledPath `
            -Message "A failed locked-file uninstall changed the isolated PATH."
        Assert-True -Condition (Test-Path -LiteralPath $binaryPath -PathType Leaf) `
            -Message "A failed locked-file uninstall removed the application executable."
        Assert-True -Condition (Test-Path -LiteralPath $uninstallerPath -PathType Leaf) `
            -Message "A failed locked-file uninstall removed its retry entry point."
        Assert-True -Condition ((Get-RegistryKeyFingerprint -SubKey $arpSubKey) -ne "absent") `
            -Message "A failed locked-file uninstall removed the isolated Add or Remove Programs key."
    }
    finally {
        $binaryLock.Dispose()
    }

    Assert-Equal 0 (Invoke-HiddenProcess -FilePath $uninstallerPath -Arguments "/S _?=$installDirectory" `
            -Operation "Silent uninstaller") "The silent uninstaller should succeed."

    Assert-TestPath -SubKey $pathSubKey -Expected $originalRawPath `
        -Message "Uninstall did not restore the original raw isolated PATH."
    Assert-Equal "absent" (Get-RegistryKeyFingerprint -SubKey $arpSubKey) `
        "Uninstall did not remove the isolated Add or Remove Programs key."
    $ownerKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($ownerSubKey, $false)
    Assert-True -Condition ($null -ne $ownerKey) `
        -Message "Uninstall removed the PATH ownership key containing unrelated data."
    try {
        Assert-Equal "preserve" ([string]$ownerKey.GetValue("UnrelatedSentinel")) `
            "Uninstall changed unrelated PATH ownership registry data."
        foreach ($ownedValue in @("PathEntry", "PathEntryIndex", "CreatedPathValue")) {
            Assert-True -Condition (@($ownerKey.GetValueNames()) -notcontains $ownedValue) `
                -Message "Uninstall left installer-owned registry value $ownedValue behind."
        }
    }
    finally { $ownerKey.Dispose() }
    foreach ($fileName in $selfDeletingOwnedFiles) {
        Assert-True -Condition (-not (Test-Path -LiteralPath (Join-Path $installDirectory $fileName))) `
            -Message "Uninstall left installer-owned file $fileName behind."
    }
    Assert-True -Condition (Test-Path -LiteralPath $uninstallerPath -PathType Leaf) `
        -Message "The direct-mode test uninstaller did not leave its expected self-delete residue."
    Assert-True -Condition (-not (Test-Path -LiteralPath (Join-Path $installDirectory "test-shortcuts"))) `
        -Message "Uninstall left the isolated shortcut directory behind."
    Assert-Equal "profile sentinel" ([IO.File]::ReadAllText($profileSentinel)) `
        "Uninstall removed or changed the profile sentinel."
    Assert-Equal "unrelated sentinel" ([IO.File]::ReadAllText($unrelatedFile)) `
        "Uninstall removed or changed an unrelated installed-directory file."
    Assert-Equal "outside the install directory" ([IO.File]::ReadAllText($outsideSentinel)) `
        "Uninstall changed a work-directory sentinel."
    Write-Host "PASS: install, upgrade and owned uninstall lifecycle"

    Remove-TestDirectory -Parent $workPath -Path $installDirectory
    Reset-TestRegistry -SubKey $testRegistryRoot
    $preExistingRawPath = "$originalRawPath$installDirectory"
    Set-TestPath -SubKey $pathSubKey -Value $preExistingRawPath

    Assert-Equal 0 (Invoke-HiddenProcess -FilePath $installerPath -Arguments "/S /D=$installDirectory" `
            -Operation "Pre-existing PATH installer") "Install with a pre-existing PATH entry should succeed."
    Assert-TestPath -SubKey $pathSubKey -Expected $preExistingRawPath `
        -Message "Install changed a pre-existing user-owned PATH entry."
    Assert-Equal 0 (Invoke-HiddenProcess -FilePath (Join-Path $installDirectory "Uninstall.exe") `
            -Arguments "/S" -Operation "Normal-mode pre-existing PATH uninstaller") `
        "Uninstall with a pre-existing PATH entry should succeed."
    Wait-NormalUninstallCompletion -InstallDirectory $installDirectory -ArpSubKey $arpSubKey `
        -OwnedRelativePaths @($ownedFiles + @("test-shortcuts\poqi.lnk"))
    Assert-TestPath -SubKey $pathSubKey -Expected $preExistingRawPath `
        -Message "Uninstall removed a pre-existing user-owned PATH entry."
    Assert-True -Condition (-not (Test-Path -LiteralPath $installDirectory)) `
        -Message "Normal-mode uninstall left the install directory behind."
    Write-Host "PASS: normal-mode uninstall self-deletes and preserves a pre-existing PATH entry"

    Reset-TestRegistry -SubKey $testRegistryRoot
    Set-TestPath -SubKey $pathSubKey -Value $originalRawPath
    $ownerKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($ownerSubKey, $true)
    if ($null -eq $ownerKey) { throw "Could not create the isolated ownership key." }
    try {
        $ownerKey.SetValue("PathEntry", (Join-Path $workPath "different install"), [Microsoft.Win32.RegistryValueKind]::String)
    }
    finally { $ownerKey.Dispose() }

    Assert-Equal 1 (Invoke-HiddenProcess -FilePath $installerPath -Arguments "/S /D=$installDirectory" `
            -Operation "Conflicting ownership installer" -AllowedExitCodes @(1)) `
        "An install with conflicting PATH ownership should fail."
    Assert-TestPath -SubKey $pathSubKey -Expected $originalRawPath `
        -Message "A failed install changed the isolated PATH."
    Assert-Equal "absent" (Get-RegistryKeyFingerprint -SubKey $arpSubKey) `
        "A failed install left an isolated Add or Remove Programs key."
    $failedInstallEntries = @(Get-ChildItem -LiteralPath $installDirectory -Force -ErrorAction SilentlyContinue)
    Assert-Equal 0 $failedInstallEntries.Count `
        "A failed install did not roll back all program files."
    Assert-True -Condition (-not (Test-Path -LiteralPath $installDirectory)) `
        -Message "A failed install left its empty destination directory behind."
    Write-Host "PASS: invalid PATH helper ownership fails and rolls back"

    Write-Host "PASS: isolated Windows installer lifecycle ($ReleaseTag)"
}
finally {
    Reset-TestRegistry -SubKey $testRegistryRoot
    if (Test-Path -LiteralPath $installDirectory) {
        Remove-TestDirectory -Parent $workPath -Path $installDirectory
    }
    if (Test-Path -LiteralPath $isolatedHome) {
        Remove-TestDirectory -Parent $workPath -Path $isolatedHome
    }
    Assert-SafeChildPath -Parent $workPath -Child $outsideSentinel
    if (Test-Path -LiteralPath $outsideSentinel) { Remove-Item -LiteralPath $outsideSentinel -Force }
    Assert-SafeChildPath -Parent $workPath -Child $installerPath
    if (Test-Path -LiteralPath $installerPath) { Remove-Item -LiteralPath $installerPath -Force }
    if ($createdWorkDirectory -and (Test-Path -LiteralPath $workPath)) {
        $remaining = @(Get-ChildItem -LiteralPath $workPath -Force)
        if ($remaining.Count -eq 0) { Remove-Item -LiteralPath $workPath -Force }
    }

    $productionPathAfter = Get-RegistryValueState -SubKey "Environment" -Name "Path"
    Assert-Equal $productionPathBefore.Exists $productionPathAfter.Exists `
        "The production user PATH value existence changed during the isolated test."
    Assert-Equal $productionPathBefore.Kind $productionPathAfter.Kind `
        "The production user PATH registry kind changed during the isolated test."
    Assert-Equal ([string]$productionPathBefore.Value) ([string]$productionPathAfter.Value) `
        "The production user PATH value changed during the isolated test."
    Assert-Equal $productionArpBefore `
        (Get-RegistryKeyFingerprint -SubKey "Software\Microsoft\Windows\CurrentVersion\Uninstall\poqi") `
        "The production poqi Add or Remove Programs key changed during the isolated test."
    Assert-Equal $productionShortcutBefore (Get-DirectoryFingerprint -Path $startMenuPoqi) `
        "The production poqi Start Menu directory changed during the isolated test."
}
