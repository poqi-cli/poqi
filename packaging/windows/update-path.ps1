[CmdletBinding()]
param(
    [ValidateSet("Add", "Remove")]
    [string]$Operation = $env:POQI_PATH_OPERATION,

    [string]$InstallDirectory = $env:POQI_PATH_INSTALL_DIRECTORY,

    [string]$RegistrySubKey = $env:POQI_PATH_REGISTRY_SUBKEY,

    [string]$OwnershipRegistrySubKey = $env:POQI_PATH_OWNERSHIP_SUBKEY
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Get-RegistryValueState {
    param(
        [Parameter(Mandatory)]
        [Microsoft.Win32.RegistryKey]$Key,

        [Parameter(Mandatory)]
        [string]$Name
    )

    $exists = @($Key.GetValueNames()) -contains $Name
    if (-not $exists) {
        return [pscustomobject]@{ Exists = $false; Value = $null; Kind = $null }
    }

    [pscustomobject]@{
        Exists = $true
        Value = $Key.GetValue(
            $Name,
            $null,
            [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
        )
        Kind = $Key.GetValueKind($Name)
    }
}

function Restore-RegistryValue {
    param(
        [Parameter(Mandatory)]
        [Microsoft.Win32.RegistryKey]$Key,

        [Parameter(Mandatory)]
        [string]$Name,

        [Parameter(Mandatory)]
        [pscustomobject]$State
    )

    if ($State.Exists) {
        $Key.SetValue($Name, $State.Value, $State.Kind)
    }
    else {
        $Key.DeleteValue($Name, $false)
    }
}

function ConvertTo-ComparablePathEntry {
    param([AllowEmptyString()][string]$Value)

    if ([string]::IsNullOrWhiteSpace($Value)) {
        return $null
    }

    $candidate = $Value.Trim()
    if ($candidate.Length -ge 2 -and $candidate[0] -eq '"' -and $candidate[$candidate.Length - 1] -eq '"') {
        $candidate = $candidate.Substring(1, $candidate.Length - 2)
    }

    $candidate = [Environment]::ExpandEnvironmentVariables($candidate)
    try {
        $candidate = [IO.Path]::GetFullPath($candidate)
    }
    catch {
        # Keep an unusual but valid PATH token comparable without rewriting it.
    }

    $root = [IO.Path]::GetPathRoot($candidate)
    while ($candidate.Length -gt $root.Length -and ($candidate.EndsWith('\') -or $candidate.EndsWith('/'))) {
        $candidate = $candidate.Substring(0, $candidate.Length - 1)
    }

    $candidate
}

function Get-PathSegments {
    param([AllowEmptyString()][string]$Value)

    $segments = [Collections.Generic.List[object]]::new()
    $start = 0
    $index = 0
    for ($position = 0; $position -le $Value.Length; $position++) {
        if ($position -eq $Value.Length -or $Value[$position] -eq ';') {
            $segments.Add([pscustomobject]@{
                    Index = $index
                    Start = $start
                    Length = $position - $start
                    Text = $Value.Substring($start, $position - $start)
                })
            $index++
            $start = $position + 1
        }
    }
    $segments
}

function Test-PathEntryEqual {
    param(
        [AllowEmptyString()][string]$Left,
        [AllowEmptyString()][string]$Right
    )

    $leftPath = ConvertTo-ComparablePathEntry $Left
    $rightPath = ConvertTo-ComparablePathEntry $Right
    if ($null -eq $leftPath -or $null -eq $rightPath) {
        return $false
    }

    [string]::Equals($leftPath, $rightPath, [StringComparison]::OrdinalIgnoreCase)
}

function Remove-PathSegment {
    param(
        [Parameter(Mandatory)][string]$Value,
        [Parameter(Mandatory)][pscustomobject]$Segment
    )

    if ($Segment.Start + $Segment.Length -lt $Value.Length) {
        return $Value.Remove($Segment.Start, $Segment.Length + 1)
    }
    if ($Segment.Start -gt 0) {
        return $Value.Remove($Segment.Start - 1, $Segment.Length + 1)
    }
    ""
}

function Remove-EmptyRegistryKey {
    param([Parameter(Mandatory)][string]$SubKey)

    $separator = $SubKey.LastIndexOf('\')
    if ($separator -le 0 -or $separator -eq $SubKey.Length - 1) {
        return
    }

    $parentPath = $SubKey.Substring(0, $separator)
    $leafName = $SubKey.Substring($separator + 1)
    $parentKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($parentPath, $true)
    if ($null -eq $parentKey) {
        return
    }

    try {
        $leafKey = $parentKey.OpenSubKey($leafName, $false)
        if ($null -eq $leafKey) {
            return
        }
        try {
            if ($leafKey.ValueCount -eq 0 -and $leafKey.SubKeyCount -eq 0) {
                $leafKey.Dispose()
                $leafKey = $null
                $parentKey.DeleteSubKey($leafName, $false)
            }
        }
        finally {
            if ($null -ne $leafKey) { $leafKey.Dispose() }
        }
    }
    catch [System.IO.IOException] {
        # Another value or child key owns this key, so it must remain.
    }
    finally {
        $parentKey.Dispose()
    }
}

function Assert-Input {
    foreach ($value in @($Operation, $InstallDirectory, $RegistrySubKey, $OwnershipRegistrySubKey)) {
        if ([string]::IsNullOrWhiteSpace($value)) {
            throw "PATH registration input must not be empty."
        }
    }
    if (-not [IO.Path]::IsPathRooted($InstallDirectory)) {
        throw "The poqi install directory must be an absolute path."
    }
    if ($InstallDirectory.IndexOfAny([char[]]@(';', '"', "`r", "`n")) -ge 0) {
        throw "The poqi install directory contains a character that cannot be represented safely in PATH."
    }
}

function Invoke-AddPathEntry {
    param(
        [Microsoft.Win32.RegistryKey]$PathKey,
        [Microsoft.Win32.RegistryKey]$OwnerKey
    )

    $pathState = Get-RegistryValueState $PathKey "Path"
    if ($pathState.Exists -and
        $pathState.Kind -ne [Microsoft.Win32.RegistryValueKind]::String -and
        $pathState.Kind -ne [Microsoft.Win32.RegistryValueKind]::ExpandString) {
        throw "The current user PATH registry value is not a string."
    }

    $ownerState = Get-RegistryValueState $OwnerKey "PathEntry"
    $indexState = Get-RegistryValueState $OwnerKey "PathEntryIndex"
    $createdState = Get-RegistryValueState $OwnerKey "CreatedPathValue"
    if ($ownerState.Exists -and -not (Test-PathEntryEqual ([string]$ownerState.Value) $InstallDirectory)) {
        throw "poqi already owns a different user PATH entry. Uninstall that copy before changing its location."
    }

    $rawPath = if ($pathState.Exists) { [string]$pathState.Value } else { "" }
    $segments = @(Get-PathSegments $rawPath)
    $matching = @($segments | Where-Object { Test-PathEntryEqual $_.Text $InstallDirectory })
    if ($matching.Count -gt 0) {
        # A pre-existing entry stays user-owned. An installer-owned entry keeps its marker.
        return 0
    }

    $newPath = if ([string]::IsNullOrEmpty($rawPath)) {
        $InstallDirectory
    }
    else {
        "$rawPath;$InstallDirectory"
    }
    $kind = if ($pathState.Exists) { $pathState.Kind } else { [Microsoft.Win32.RegistryValueKind]::ExpandString }
    $createdPathValue = if ($createdState.Exists) { [int]$createdState.Value } elseif ($pathState.Exists) { 0 } else { 1 }
    $entryIndex = @(Get-PathSegments $newPath).Count - 1

    try {
        $PathKey.SetValue("Path", $newPath, $kind)
        $OwnerKey.SetValue("PathEntry", $InstallDirectory, [Microsoft.Win32.RegistryValueKind]::String)
        $OwnerKey.SetValue("PathEntryIndex", $entryIndex, [Microsoft.Win32.RegistryValueKind]::DWord)
        $OwnerKey.SetValue("CreatedPathValue", $createdPathValue, [Microsoft.Win32.RegistryValueKind]::DWord)
    }
    catch {
        Restore-RegistryValue $PathKey "Path" $pathState
        Restore-RegistryValue $OwnerKey "PathEntry" $ownerState
        Restore-RegistryValue $OwnerKey "PathEntryIndex" $indexState
        Restore-RegistryValue $OwnerKey "CreatedPathValue" $createdState
        throw
    }
    10
}

function Invoke-RemovePathEntry {
    param(
        [Microsoft.Win32.RegistryKey]$PathKey,
        [Microsoft.Win32.RegistryKey]$OwnerKey
    )

    $ownerState = Get-RegistryValueState $OwnerKey "PathEntry"
    if (-not $ownerState.Exists -or -not (Test-PathEntryEqual ([string]$ownerState.Value) $InstallDirectory)) {
        return 0
    }

    $indexState = Get-RegistryValueState $OwnerKey "PathEntryIndex"
    $createdState = Get-RegistryValueState $OwnerKey "CreatedPathValue"
    $pathState = Get-RegistryValueState $PathKey "Path"
    $changed = $false

    try {
        if ($pathState.Exists) {
            if ($pathState.Kind -ne [Microsoft.Win32.RegistryValueKind]::String -and
                $pathState.Kind -ne [Microsoft.Win32.RegistryValueKind]::ExpandString) {
                throw "The current user PATH registry value is not a string."
            }

            $rawPath = [string]$pathState.Value
            $segments = @(Get-PathSegments $rawPath)
            $ownedSegment = $null
            if ($indexState.Exists) {
                $storedIndex = [int]$indexState.Value
                if ($storedIndex -ge 0 -and $storedIndex -lt $segments.Count -and
                    (Test-PathEntryEqual $segments[$storedIndex].Text $InstallDirectory)) {
                    $ownedSegment = $segments[$storedIndex]
                }
            }
            if ($null -eq $ownedSegment) {
                $ownedSegment = @($segments | Where-Object { Test-PathEntryEqual $_.Text $InstallDirectory }) |
                    Select-Object -Last 1
            }

            if ($null -ne $ownedSegment) {
                $newPath = Remove-PathSegment $rawPath $ownedSegment
                if ([string]::IsNullOrEmpty($newPath) -and $createdState.Exists -and [int]$createdState.Value -eq 1) {
                    $PathKey.DeleteValue("Path", $false)
                }
                else {
                    $PathKey.SetValue("Path", $newPath, $pathState.Kind)
                }
                $changed = $true
            }
        }

        $OwnerKey.DeleteValue("PathEntry", $false)
        $OwnerKey.DeleteValue("PathEntryIndex", $false)
        $OwnerKey.DeleteValue("CreatedPathValue", $false)
    }
    catch {
        Restore-RegistryValue $PathKey "Path" $pathState
        Restore-RegistryValue $OwnerKey "PathEntry" $ownerState
        Restore-RegistryValue $OwnerKey "PathEntryIndex" $indexState
        Restore-RegistryValue $OwnerKey "CreatedPathValue" $createdState
        throw
    }

    if ($changed) { 10 } else { 0 }
}

try {
    Assert-Input
    $ownerKey = $null
    $pathKey = $null
    try {
        if ($Operation -eq "Add") {
            $pathKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($RegistrySubKey, $true)
            $ownerKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($OwnershipRegistrySubKey, $true)
        }
        else {
            $pathKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($RegistrySubKey, $true)
            $ownerKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($OwnershipRegistrySubKey, $true)
            if ($null -eq $ownerKey) {
                exit 0
            }
            if ($null -eq $pathKey) {
                $pathKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($RegistrySubKey, $true)
            }
        }

        if ($null -eq $pathKey -or $null -eq $ownerKey) {
            throw "Could not open the current user registry for PATH registration."
        }

        $result = if ($Operation -eq "Add") {
            Invoke-AddPathEntry $pathKey $ownerKey
        }
        else {
            Invoke-RemovePathEntry $pathKey $ownerKey
        }
    }
    finally {
        if ($null -ne $pathKey) { $pathKey.Dispose() }
        if ($null -ne $ownerKey) { $ownerKey.Dispose() }
    }

    if ($Operation -eq "Remove") {
        Remove-EmptyRegistryKey $OwnershipRegistrySubKey
    }
    exit $result
}
catch {
    [Console]::Error.WriteLine("poqi PATH registration failed: $($_.Exception.Message)")
    exit 1
}
