<#
.SYNOPSIS
    Shared, side-effect-free validation for the Phase 4 installer scripts.

.DESCRIPTION
    The entry-point scripts are the only files that perform filesystem or
    registry work.  This file contains validation, marker, and command-line
    helpers so install and uninstall apply the same safety rules.
#>

Set-StrictMode -Version Latest

$script:InstallerMarkerName = '.redsamurai-install'
$script:InstallerMarkerContent = 'redsamurai-config'

function Assert-SafeAbsolutePath {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true, Position = 0)]
        [AllowEmptyString()]
        [string] $Path,

        [Parameter(Mandatory = $true, Position = 1)]
        [ValidateNotNullOrEmpty()]
        [string] $Name,

        [switch] $DisallowRoot
    )

    if ($null -eq $Path -or [string]::IsNullOrWhiteSpace($Path)) {
        throw "$Name must not be empty."
    }
    if ($Path.Trim() -cne $Path) {
        throw "$Name must not have leading or trailing whitespace."
    }
    if ($Path.IndexOf([char] 0) -ge 0) {
        throw "$Name contains a NUL character."
    }
    if ($Path.Length -gt 32767) {
        throw "$Name is too long."
    }

    # Device and extended-length paths can address objects outside the
    # selected filesystem tree and are not needed by this installer.
    if ($Path -match '^(?:\\\\[.?]|//[.?])') {
        throw "$Name must not use a device or extended path."
    }

    foreach ($character in $Path.ToCharArray()) {
        if ([int] $character -lt 32) {
            throw "$Name contains an invalid control character."
        }
    }
    if ($Path -match '[<>"|?*]') {
        throw "$Name contains invalid Windows path characters."
    }
    if ($Path.Length -gt 2 -and $Path.Substring(2).Contains(':')) {
        throw "$Name contains an invalid drive or stream separator."
    }

    # Reject dot components before GetFullPath can normalize them away.
    $components = $Path -split '[\\/]'
    if ($components -contains '..') {
        throw "$Name contains a parent path component: $Path"
    }
    if ($components -contains '.') {
        throw "$Name contains a dot path component: $Path"
    }
    foreach ($component in $components) {
        if ($component.Length -gt 0 -and
            ($component.EndsWith('.') -or $component.EndsWith(' '))) {
            throw "$Name contains a path component ending in a dot or space."
        }
    }

    # IsPathRooted accepts drive-relative paths such as C:folder.  Require a
    # drive root or a complete UNC server/share root.
    if ($Path -notmatch '^(?:[A-Za-z]:[\\/]|\\\\[^\\/]+[\\/][^\\/]+(?:[\\/]|$))') {
        throw "$Name must be an absolute Windows path: $Path"
    }

    try {
        $fullPath = [IO.Path]::GetFullPath($Path)
    }
    catch {
        throw "$Name is not a valid absolute Windows path."
    }
    if ([string]::IsNullOrWhiteSpace($fullPath)) {
        throw "$Name is not a valid absolute Windows path."
    }
    if ($DisallowRoot -and (Test-IsFileSystemRoot $fullPath)) {
        throw "$Name must not be a filesystem root: $fullPath"
    }
    return $fullPath
}

function Assert-SafeExecutableName {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true, Position = 0)]
        [AllowEmptyString()]
        [string] $Name
    )

    if ($null -eq $Name -or [string]::IsNullOrWhiteSpace($Name)) {
        throw 'ExecutableName must not be empty.'
    }
    if ($Name.Trim() -cne $Name) {
        throw 'ExecutableName must not have leading or trailing whitespace.'
    }
    if ($Name.IndexOf([char] 0) -ge 0) {
        throw 'ExecutableName contains a NUL character.'
    }
    if ($Name.Length -gt 255) {
        throw 'ExecutableName is too long.'
    }
    if ($Name -in @('.', '..') -or $Name -match '[\\/]') {
        throw 'ExecutableName must be a file name, not a path.'
    }
    foreach ($character in $Name.ToCharArray()) {
        if ([int] $character -lt 32) {
            throw 'ExecutableName contains an invalid control character.'
        }
    }
    if ($Name -match '[<>:"|?*]') {
        throw 'ExecutableName contains invalid Windows file-name characters.'
    }
    if ($Name.EndsWith('.') -or $Name.EndsWith(' ')) {
        throw 'ExecutableName must not end in a dot or space.'
    }
    if (-not $Name.EndsWith('.exe', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'ExecutableName must have an .exe extension.'
    }

    $stem = $Name.Split('.')[0]
    if ($stem -match '^(?i:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])$') {
        throw 'ExecutableName uses a reserved Windows file name.'
    }
    return $Name
}

function Test-IsFileSystemRoot {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)] [string] $Path)

    try {
        $root = [IO.Path]::GetPathRoot($Path)
    }
    catch {
        return $false
    }
    if ([string]::IsNullOrWhiteSpace($root)) {
        return $false
    }
    $pathKey = ($Path -replace '[\\/]+$', '')
    $rootKey = ($root -replace '[\\/]+$', '')
    return $pathKey.Equals($rootKey, [StringComparison]::OrdinalIgnoreCase)
}

function Assert-DirectoryMutationTarget {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)] [string] $Path,
        [Parameter(Mandatory = $true)] [string] $Name
    )

    Assert-NoReparsePointsInPath -Path $Path -Name $Name

    if (-not (Test-Path -LiteralPath $Path)) {
        return
    }
    try {
        $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    }
    catch {
        throw "$Name could not be inspected: $Path"
    }
    if (-not $item.PSIsContainer) {
        throw "$Name exists but is not a directory: $Path"
    }
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "$Name must not be a reparse point: $Path"
    }
}

function Assert-NoReparsePointsInPath {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)] [string] $Path,
        [Parameter(Mandatory = $true)] [string] $Name
    )

    # Walk from the requested path toward the nearest filesystem root.  This
    # checks existing ancestors even when the target itself does not exist,
    # preventing a new directory or file from being created through a junction
    # or symlink outside the selected tree.
    $current = $Path
    while (-not [string]::IsNullOrWhiteSpace($current)) {
        if (Test-Path -LiteralPath $current) {
            try {
                $item = Get-Item -LiteralPath $current -Force -ErrorAction Stop
            }
            catch {
                throw "$Name could not be inspected: $current"
            }
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "$Name must not traverse a reparse point: $current"
            }
        }

        # Use the .NET path routine instead of Split-Path so valid wildcard
        # characters in a Windows file name cannot alter the walk.
        $parent = [IO.Path]::GetDirectoryName($current)
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -ceq $current) {
            break
        }
        $current = $parent
    }
}

function Test-PathTraversesReparsePoint {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string] $Path
    )

    # The default Documents known folder can be redirected into OneDrive or
    # another cloud provider.  Keep the installer fail-closed for caller-
    # supplied paths, but let the default resolver choose a local fallback
    # when an existing ancestor is a reparse point.
    $current = $Path
    while (-not [string]::IsNullOrWhiteSpace($current)) {
        if (Test-Path -LiteralPath $current) {
            try {
                $item = Get-Item -LiteralPath $current -Force -ErrorAction Stop
            }
            catch {
                throw "Path could not be inspected: $current"
            }
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                return $true
            }
        }

        $parent = [IO.Path]::GetDirectoryName($current)
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -ceq $current) {
            break
        }
        $current = $parent
    }
    return $false
}

function Assert-FileMutationTarget {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)] [string] $Path,
        [Parameter(Mandatory = $true)] [string] $Name
    )

    Assert-NoReparsePointsInPath -Path $Path -Name $Name

    if (-not (Test-Path -LiteralPath $Path)) {
        return
    }
    try {
        $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    }
    catch {
        throw "$Name could not be inspected: $Path"
    }
    if ($item.PSIsContainer) {
        throw "$Name exists but is a directory: $Path"
    }
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "$Name must not be a reparse point: $Path"
    }
}

function Assert-NoReparsePointsBelow {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)] [string] $Directory)

    try {
        $entries = @(Get-ChildItem -LiteralPath $Directory -Force -Recurse -ErrorAction Stop)
    }
    catch {
        throw "InstallDirectory contents could not be inspected: $Directory"
    }
    foreach ($entry in $entries) {
        if (($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "InstallDirectory contains a reparse point; refusing to remove it: $($entry.FullName)"
        }
    }
}

function Get-PathComparisonKey {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)] [string] $Path)

    return (($Path -replace '/', '\') -replace '[\\/]+$', '').ToLowerInvariant()
}

function Test-SameOrChildPath {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)] [string] $Candidate,
        [Parameter(Mandatory = $true)] [string] $Ancestor
    )

    $candidateKey = Get-PathComparisonKey $Candidate
    $ancestorKey = Get-PathComparisonKey $Ancestor
    return $candidateKey -eq $ancestorKey -or
        $candidateKey.StartsWith($ancestorKey + '\', [StringComparison]::OrdinalIgnoreCase)
}

function Assert-DirectoriesDoNotOverlap {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)] [string] $InstallDirectory,
        [Parameter(Mandatory = $true)] [string] $DataDirectory
    )

    if (Test-SameOrChildPath -Candidate $DataDirectory -Ancestor $InstallDirectory) {
        throw 'DataDirectory must not be the install directory or a child of InstallDirectory.'
    }
}

function Quote-WindowsArgument {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string] $Value
    )

    if ($null -eq $Value -or $Value.IndexOf([char] 0) -ge 0) {
        throw 'A command-line argument must not be null or contain NUL.'
    }

    $builder = [Text.StringBuilder]::new()
    [void] $builder.Append([char] 34)
    $backslashes = 0
    foreach ($character in $Value.ToCharArray()) {
        if ($character -eq [char] 92) {
            $backslashes++
            continue
        }
        if ($character -eq [char] 34) {
            for ($index = 0; $index -lt (2 * $backslashes + 1); $index++) {
                [void] $builder.Append([char] 92)
            }
            [void] $builder.Append([char] 34)
        }
        else {
            for ($index = 0; $index -lt $backslashes; $index++) {
                [void] $builder.Append([char] 92)
            }
            [void] $builder.Append($character)
        }
        $backslashes = 0
    }

    # Backslashes immediately before the closing quote must be doubled.
    for ($index = 0; $index -lt (2 * $backslashes); $index++) {
        [void] $builder.Append([char] 92)
    }
    [void] $builder.Append([char] 34)
    return $builder.ToString()
}

function Get-TrayAutostartCommand {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string] $ExecutablePath
    )

    # Keep the Run contract in one helper.  Both install and uninstall must
    # derive the same value, and callers must not be able to register an
    # editor-mode or otherwise ambiguous command by accident.
    $validatedPath = Assert-SafeAbsolutePath -Path $ExecutablePath `
        -Name 'ExecutablePath' -DisallowRoot
    $fileName = [IO.Path]::GetFileName($validatedPath)
    [void] (Assert-SafeExecutableName $fileName)
    return (Quote-WindowsArgument $validatedPath) + ' --tray'
}

function Assert-TrayAutostartCommand {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string] $ExecutablePath,

        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string] $Command
    )

    $expected = Get-TrayAutostartCommand -ExecutablePath $ExecutablePath
    if ($Command -cne $expected) {
        throw 'Current-user Run command must be exactly the quoted executable followed by --tray.'
    }
    return $true
}

function Get-InstallMarkerPath {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)] [string] $InstallDirectory)

    return Join-Path -Path $InstallDirectory -ChildPath $script:InstallerMarkerName
}

function Get-InstallMarkerState {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)] [string] $MarkerPath)

    if (-not (Test-Path -LiteralPath $MarkerPath)) {
        return 'Absent'
    }
    try {
        $item = Get-Item -LiteralPath $MarkerPath -Force -ErrorAction Stop
    }
    catch {
        throw "Install marker could not be inspected: $MarkerPath"
    }
    if ($item.PSIsContainer -or
        (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)) {
        return 'Invalid'
    }
    try {
        $bytes = [IO.File]::ReadAllBytes($MarkerPath)
    }
    catch {
        throw "Install marker could not be read: $MarkerPath"
    }
    $expected = [Text.UTF8Encoding]::new($false).GetBytes($script:InstallerMarkerContent)
    if ([Convert]::ToBase64String($bytes) -ceq [Convert]::ToBase64String($expected)) {
        return 'Valid'
    }
    return 'Invalid'
}

function Write-InstallMarker {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)] [string] $MarkerPath)

    try {
        # Explicitly disable the UTF-8 BOM so Windows PowerShell 5.1 and
        # PowerShell 7 produce the same marker bytes.
        [IO.File]::WriteAllText(
            $MarkerPath,
            $script:InstallerMarkerContent,
            [Text.UTF8Encoding]::new($false)
        )
    }
    catch {
        throw "Install marker could not be written: $MarkerPath"
    }
}

function Get-DefaultInstallDirectory {
    [CmdletBinding()]
    param()

    $localAppData = [Environment]::GetFolderPath(
        [Environment+SpecialFolder]::LocalApplicationData
    )
    if ([string]::IsNullOrWhiteSpace($localAppData)) {
        $localAppData = $env:LOCALAPPDATA
    }
    if ([string]::IsNullOrWhiteSpace($localAppData)) {
        throw 'InstallDirectory default could not be determined.'
    }
    return Join-Path -Path $localAppData -ChildPath 'RED SAMURAI'
}

function Get-DefaultDataDirectory {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)] [string] $ProductName)

    $documents = [Environment]::GetFolderPath(
        [Environment+SpecialFolder]::MyDocuments
    )
    if ([string]::IsNullOrWhiteSpace($documents)) {
        if ([string]::IsNullOrWhiteSpace($env:USERPROFILE)) {
            throw 'DataDirectory default could not be determined.'
        }
        $documents = Join-Path -Path $env:USERPROFILE -ChildPath 'Documents'
    }
    $documentsCandidate = Join-Path -Path $documents -ChildPath $ProductName
    if (-not (Test-PathTraversesReparsePoint -Path $documentsCandidate)) {
        return $documentsCandidate
    }

    # A redirected Documents folder is not a safe mutation boundary.  Keep
    # the default deterministic for both install and uninstall while placing
    # new data below the user's local, non-cloud profile tree.
    if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
        throw 'DataDirectory default could not be determined because LOCALAPPDATA is empty.'
    }
    $fallbackRoot = Join-Path -Path $env:LOCALAPPDATA -ChildPath 'OpenRedSamurai'
    return Join-Path -Path $fallbackRoot -ChildPath $ProductName
}
