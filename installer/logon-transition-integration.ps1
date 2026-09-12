<#
.SYNOPSIS
    Runs the explicit two-phase current-user sign-out/logon startup check.

.DESCRIPTION
    Prepare creates a GUID-scoped install fixture, writes one disposable data
    file under an explicitly selected local Documents root, registers the
    exact HKCU Run command, and persists all state needed after sign-out.
    Finish runs after the user signs in again; it verifies that Windows started
    exactly one tray process for the same user and current session, preserves
    the data hash, and then removes only the marker-owned install and fixture
    Run value.  The disposable Documents data is removed only after its hash
    and exact contents have been recorded.

    The script never performs logoff itself because doing so would terminate
    the calling interactive shell.  The operator must run `shutdown.exe /l`
    after Prepare reports `status=prepared`.
#>
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'High')]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Prepare', 'Finish')]
    [string] $Phase,

    [Parameter(Mandatory = $true)]
    [ValidateSet('RED-SAMURAI-LOGON')]
    [string] $LiveConfirmation,

    [Parameter()]
    [AllowEmptyString()]
    [string] $DataRoot,

    [Parameter()]
    [AllowEmptyString()]
    [string] $RunDirectory,

    [Parameter()]
    [ValidateRange(1000, 600000)]
    [int] $TimeoutMs = 420000
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path -Path $PSScriptRoot -ChildPath 'common.ps1')
. (Join-Path -Path $PSScriptRoot -ChildPath 'startup-verification-helpers.ps1')

$ProductName = 'RED SAMURAI 16400DPI Gaming Mouse'
$ExecutableName = 'redsamurai-config.exe'
$RunValueName = $ProductName
$RunKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$TrayArgument = '--tray'
$RepoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$SourceDirectory = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\target\release'))
$InstallScript = Join-Path $PSScriptRoot 'install.ps1'
$UninstallScript = Join-Path $PSScriptRoot 'uninstall.ps1'
$CapturesRoot = Join-Path $RepoRoot 'captures'
$LatestPointer = Join-Path $CapturesRoot 'logon-transition-latest.txt'

function Write-JsonFile {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)] [string] $Path,
        [Parameter(Mandatory = $true)] [object] $Value
    )

    $json = $Value | ConvertTo-Json -Depth 8
    [IO.File]::WriteAllText($Path, $json, [Text.UTF8Encoding]::new($false))
}

function Get-RunProperty {
    [CmdletBinding()]
    param()

    if (-not (Test-Path -LiteralPath $RunKey -PathType Container)) {
        return $null
    }
    $properties = Get-ItemProperty -LiteralPath $RunKey -ErrorAction Stop
    return $properties.PSObject.Properties[$RunValueName]
}

function Get-ExactTrayProcesses {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)] [string] $ExecutablePath)

    return @(
        Get-CimInstance -ClassName Win32_Process `
            -Filter "Name = '$ExecutableName'" -ErrorAction Stop |
            Where-Object { [string]$_.ExecutablePath -ieq $ExecutablePath }
    )
}

function Save-Qwinsta {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)] [string] $Path)

    try {
        & qwinsta 2>&1 | Set-Content -LiteralPath $Path
    }
    catch {
        "qwinsta failed: $($_.Exception.Message)" | Set-Content -LiteralPath $Path
    }
}

function Get-ProcessCreationUtc {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)] [object] $ProcessRecord)

    try {
        $creation = $ProcessRecord.CreationDate
        if ($creation -is [DateTimeOffset]) {
            return $creation.ToUniversalTime().ToString('o')
        }
        if ($creation -is [DateTime]) {
            return ([DateTime]$creation).ToUniversalTime().ToString('o')
        }

        $creationText = [string]$creation
        try {
            return [System.Management.ManagementDateTimeConverter]::ToDateTime(
                $creationText
            ).ToUniversalTime().ToString('o')
        }
        catch {
            return [DateTimeOffset]::Parse($creationText).ToUniversalTime().ToString('o')
        }
    }
    catch {
        return $null
    }
}

function Stop-ExactTrayProcess {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)] [int] $ProcessId,
        [Parameter(Mandatory = $true)] [string] $ExecutablePath,
        [Parameter(Mandatory = $true)] [string] $ExpectedCommand
    )

    $record = Get-CimInstance -ClassName Win32_Process `
        -Filter "ProcessId = $ProcessId" -ErrorAction SilentlyContinue
    if ($null -eq $record) {
        return 'already-exited'
    }
    if ([string]$record.ExecutablePath -ine $ExecutablePath -or
        ([string]$record.CommandLine).Trim() -cne $ExpectedCommand) {
        throw "Refusing to stop PID $ProcessId after its executable or command line changed."
    }
    Stop-Process -Id $ProcessId -Force -ErrorAction Stop
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    do {
        $remaining = Get-CimInstance -ClassName Win32_Process `
            -Filter "ProcessId = $ProcessId" -ErrorAction SilentlyContinue
        if ($null -eq $remaining) {
            return 'complete'
        }
        Start-Sleep -Milliseconds 100
    }
    while ([DateTime]::UtcNow -lt $deadline)
    throw "Tray process PID $ProcessId remained alive after cleanup."
}

function Get-LocalDataRoot {
    [CmdletBinding()]
    param()

    if (-not [string]::IsNullOrWhiteSpace($DataRoot)) {
        return $DataRoot
    }
    $resolved = [Environment]::GetFolderPath([Environment+SpecialFolder]::MyDocuments)
    if ([string]::IsNullOrWhiteSpace($resolved)) {
        throw 'DataRoot could not be resolved; pass -DataRoot explicitly.'
    }
    return $resolved
}

if ($Phase -ceq 'Prepare') {
    $installAttempted = $false
    $runDirectoryPath = $null
    $dataDirectoryPath = $null
    $expectedRunCommand = $null
    try {
        New-Item -ItemType Directory -Path $CapturesRoot -Force | Out-Null
        $runId = [Guid]::NewGuid().ToString('N')
        if ([string]::IsNullOrWhiteSpace($RunDirectory)) {
            $runDirectoryPath = Join-Path $CapturesRoot ('logon-transition-' + $runId)
        }
        else {
            $runDirectoryPath = Assert-SafeAbsolutePath `
                -Path $RunDirectory -Name 'RunDirectory' -DisallowRoot
        }
        Assert-DirectoryMutationTarget -Path $runDirectoryPath -Name 'RunDirectory'
        if (Test-Path -LiteralPath $runDirectoryPath) {
            $existingEntries = @(Get-ChildItem -LiteralPath $runDirectoryPath -Force -ErrorAction Stop)
            if ($existingEntries.Count -ne 0) {
                throw "RunDirectory must be empty: $runDirectoryPath"
            }
        }
        New-Item -ItemType Directory -Path $runDirectoryPath -Force | Out-Null

        $installDirectory = Join-Path $runDirectoryPath 'install'
        $dataRootPath = Assert-SafeAbsolutePath `
            -Path (Get-LocalDataRoot) -Name 'DataRoot' -DisallowRoot
        Assert-DirectoryMutationTarget -Path $dataRootPath -Name 'DataRoot'
        $dataDirectoryPath = Join-Path $dataRootPath ($ProductName + '-logon-' + $runId)
        $dataDirectoryPath = Assert-SafeAbsolutePath `
            -Path $dataDirectoryPath -Name 'DataDirectory' -DisallowRoot
        Assert-DirectoriesDoNotOverlap -InstallDirectory $installDirectory `
            -DataDirectory $dataDirectoryPath
        Assert-DirectoryMutationTarget -Path $installDirectory -Name 'InstallDirectory'
        Assert-DirectoryMutationTarget -Path $dataDirectoryPath -Name 'DataDirectory'
        if (Test-Path -LiteralPath $dataDirectoryPath) {
            throw "DataDirectory already exists: $dataDirectoryPath"
        }

        $sourceExecutable = Assert-SafeAbsolutePath `
            -Path (Join-Path $SourceDirectory $ExecutableName) -Name 'Source executable'
        Assert-FileMutationTarget -Path $sourceExecutable -Name 'Source executable'
        if (-not (Test-Path -LiteralPath $sourceExecutable -PathType Leaf)) {
            throw "Source executable was not found: $sourceExecutable"
        }
        $destinationExecutable = Assert-SafeAbsolutePath `
            -Path (Join-Path $installDirectory $ExecutableName) -Name 'Destination executable'
        $expectedRunCommand = Get-TrayAutostartCommand -ExecutablePath $destinationExecutable
        Assert-TrayAutostartCommand -ExecutablePath $destinationExecutable `
            -Command $expectedRunCommand | Out-Null

        $existingRun = Get-RunProperty
        if ($null -ne $existingRun) {
            throw 'The product HKCU Run value already exists; refusing the disposable logon test.'
        }

        $whatIfPath = Join-Path $runDirectoryPath 'install-whatif.txt'
        & $InstallScript -SourceDirectory $SourceDirectory `
            -InstallDirectory $installDirectory -DataDirectory $dataDirectoryPath `
            -WhatIf 2>&1 | Tee-Object -FilePath $whatIfPath

        $installAttempted = $true
        $installPath = Join-Path $runDirectoryPath 'install-result.txt'
        & $InstallScript -SourceDirectory $SourceDirectory `
            -InstallDirectory $installDirectory -DataDirectory $dataDirectoryPath `
            -Confirm:$false 2>&1 | Tee-Object -FilePath $installPath

        $dataFile = Join-Path $dataDirectoryPath 'logon-test-data.txt'
        [IO.File]::WriteAllText(
            $dataFile,
            'disposable RED SAMURAI sign-out/logon data',
            [Text.UTF8Encoding]::new($false)
        )
        $markerState = Get-InstallMarkerState `
            -MarkerPath (Get-InstallMarkerPath -InstallDirectory $installDirectory)
        if ($markerState -cne 'Valid') {
            throw 'The live install marker was not valid after Prepare.'
        }
        $runProperty = Get-RunProperty
        if ($null -eq $runProperty -or
            [string]$runProperty.Value -cne $expectedRunCommand) {
            throw 'The HKCU Run command did not match the exact quoted tray command.'
        }

        $identity = Get-CurrentUserIdentityName
        $sessionId = Get-CurrentSessionId
        $preparedUtc = [DateTime]::UtcNow.ToString('o')
        Save-Qwinsta -Path (Join-Path $runDirectoryPath 'qwinsta-before.txt')
        $beforeProcesses = @(Get-ExactTrayProcesses -ExecutablePath $destinationExecutable)
        if ($beforeProcesses.Count -ne 0) {
            throw 'The disposable tray executable was already running before sign-out.'
        }

        $state = [ordered]@{
            schema = 'redsamurai-logon-transition-v1'
            run_id = $runId
            run_directory = $runDirectoryPath
            install_directory = $installDirectory
            install_executable = $destinationExecutable
            data_root = $dataRootPath
            data_directory = $dataDirectoryPath
            data_file = $dataFile
            run_value_name = $RunValueName
            run_command = $expectedRunCommand
            identity = $identity
            prepare_session_id = $sessionId
            prepared_utc = $preparedUtc
            source_sha256 = (Get-FileHash -LiteralPath $sourceExecutable -Algorithm SHA256).Hash
            installed_sha256 = (Get-FileHash -LiteralPath $destinationExecutable -Algorithm SHA256).Hash
            data_sha256 = (Get-FileHash -LiteralPath $dataFile -Algorithm SHA256).Hash
        }
        Write-JsonFile -Path (Join-Path $runDirectoryPath 'state.json') -Value $state
        [IO.File]::WriteAllText(
            $LatestPointer,
            $runDirectoryPath,
            [Text.UTF8Encoding]::new($false)
        )

        [pscustomobject]@{
            status = 'prepared'
            run_directory = $runDirectoryPath
            state_file = (Join-Path $runDirectoryPath 'state.json')
            identity = $identity
            prepare_session_id = $sessionId
            run_command = $expectedRunCommand
            data_sha256 = $state.data_sha256
            next_command = 'shutdown.exe /l'
        } | ConvertTo-Json -Compress
    }
    catch {
        if ($installAttempted -and $null -ne $runDirectoryPath -and
            $null -ne $dataDirectoryPath -and $null -ne $expectedRunCommand) {
            try {
                $currentRun = Get-RunProperty
                $installDirectory = Join-Path $runDirectoryPath 'install'
                if ($null -ne $currentRun -and
                    [string]$currentRun.Value -ceq $expectedRunCommand) {
                    & $UninstallScript -InstallDirectory $installDirectory `
                        -DataDirectory $dataDirectoryPath -Confirm:$false | Out-Null
                }
            }
            catch {
                Write-Warning "Prepare cleanup was not proven: $($_.Exception.Message)"
            }
        }
        throw
    }
    return
}

try {
    if ([string]::IsNullOrWhiteSpace($RunDirectory)) {
        if (-not (Test-Path -LiteralPath $LatestPointer -PathType Leaf)) {
            throw "No Prepare state pointer was found: $LatestPointer"
        }
        $RunDirectory = (Get-Content -LiteralPath $LatestPointer -Raw).Trim()
    }
    $runDirectoryPath = Assert-SafeAbsolutePath `
        -Path $RunDirectory -Name 'RunDirectory' -DisallowRoot
    $statePath = Join-Path $runDirectoryPath 'state.json'
    if (-not (Test-Path -LiteralPath $statePath -PathType Leaf)) {
        throw "Prepare state was not found: $statePath"
    }
    $state = Get-Content -LiteralPath $statePath -Raw | ConvertFrom-Json
    $installDirectory = Assert-SafeAbsolutePath `
        -Path ([string]$state.install_directory) -Name 'InstallDirectory' -DisallowRoot
    $destinationExecutable = Assert-SafeAbsolutePath `
        -Path ([string]$state.install_executable) -Name 'Destination executable' -DisallowRoot
    $dataDirectoryPath = Assert-SafeAbsolutePath `
        -Path ([string]$state.data_directory) -Name 'DataDirectory' -DisallowRoot
    $dataFile = Assert-SafeAbsolutePath `
        -Path ([string]$state.data_file) -Name 'Data file' -DisallowRoot
    $expectedRunCommand = [string]$state.run_command
    $currentIdentity = Get-CurrentUserIdentityName
    if (-not $currentIdentity.Equals([string]$state.identity, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Current identity '$currentIdentity' does not match Prepare identity '$($state.identity)'."
    }
    $currentSessionId = Get-CurrentSessionId
    Save-Qwinsta -Path (Join-Path $runDirectoryPath 'qwinsta-after.txt')

    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    $processes = @()
    do {
        $processes = @(Get-ExactTrayProcesses -ExecutablePath $destinationExecutable)
        if ($processes.Count -eq 1) {
            break
        }
        Start-Sleep -Milliseconds 500
    }
    while ([DateTime]::UtcNow -lt $deadline)

    if ($processes.Count -ne 1) {
        throw "Expected exactly one auto-started tray process, observed $($processes.Count)."
    }
    $process = $processes[0]
    $actualCommand = ([string]$process.CommandLine).Trim()
    if ($actualCommand -cne $expectedRunCommand) {
        throw 'The post-logon tray command line did not exactly match HKCU Run.'
    }
    if ([int]$process.SessionId -ne $currentSessionId) {
        throw "Tray process session $($process.SessionId) did not match current session $currentSessionId."
    }
    $processIdentity = Get-ProcessUserIdentity -ProcessRecord $process
    if (-not $processIdentity.Equals($currentIdentity, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Tray process identity '$processIdentity' did not match '$currentIdentity'."
    }
    $runProperty = Get-RunProperty
    if ($null -eq $runProperty -or [string]$runProperty.Value -cne $expectedRunCommand) {
        throw 'The HKCU Run value changed across sign-out/logon.'
    }
    $dataHashAfterLogon = (Get-FileHash -LiteralPath $dataFile -Algorithm SHA256).Hash
    if ($dataHashAfterLogon -cne [string]$state.data_sha256) {
        throw 'Disposable user data changed across sign-out/logon.'
    }
    $creationUtc = Get-ProcessCreationUtc -ProcessRecord $process
    if (-not [string]::IsNullOrWhiteSpace($creationUtc)) {
        $prepared = [DateTimeOffset]::Parse([string]$state.prepared_utc)
        $created = [DateTimeOffset]::Parse($creationUtc)
        if ($created -le $prepared) {
            throw 'The observed tray process predates the Prepare phase.'
        }
    }

    $result = [ordered]@{
        status = 'logon-observed'
        run_directory = $runDirectoryPath
        identity = $currentIdentity
        prepare_session_id = [int]$state.prepare_session_id
        logon_session_id = $currentSessionId
        session_id_changed = ([int]$state.prepare_session_id -ne $currentSessionId)
        run_command = $expectedRunCommand
        process_id = [int]$process.ProcessId
        process_identity = $processIdentity
        process_session_id = [int]$process.SessionId
        process_creation_utc = $creationUtc
        data_sha256_before = [string]$state.data_sha256
        data_sha256_after_logon = $dataHashAfterLogon
        process_cleanup = 'not-started'
        run_cleanup = 'not-started'
        install_cleanup = 'not-started'
        data_cleanup = 'not-started'
    }
    Write-JsonFile -Path (Join-Path $runDirectoryPath 'logon-observation.json') -Value $result

    $result.process_cleanup = Stop-ExactTrayProcess `
        -ProcessId ([int]$process.ProcessId) `
        -ExecutablePath $destinationExecutable `
        -ExpectedCommand $expectedRunCommand

    $whatIfPath = Join-Path $runDirectoryPath 'uninstall-whatif.txt'
    & $UninstallScript -InstallDirectory $installDirectory `
        -DataDirectory $dataDirectoryPath -WhatIf 2>&1 | Tee-Object -FilePath $whatIfPath
    $uninstallPath = Join-Path $runDirectoryPath 'uninstall-result.txt'
    & $UninstallScript -InstallDirectory $installDirectory `
        -DataDirectory $dataDirectoryPath -Confirm:$false 2>&1 |
        Tee-Object -FilePath $uninstallPath

    $runAfter = Get-RunProperty
    if ($null -ne $runAfter) {
        throw 'The fixture HKCU Run value remained after uninstall.'
    }
    if (Test-Path -LiteralPath $installDirectory) {
        throw 'The marker-owned install directory remained after uninstall.'
    }
    $dataHashBeforeCleanup = (Get-FileHash -LiteralPath $dataFile -Algorithm SHA256).Hash
    if ($dataHashBeforeCleanup -cne [string]$state.data_sha256) {
        throw 'Disposable user data changed during uninstall.'
    }
    Assert-NoReparsePointsBelow -Directory $dataDirectoryPath
    $entries = @(Get-ChildItem -LiteralPath $dataDirectoryPath -Force -ErrorAction Stop)
    if ($entries.Count -ne 1 -or
        $entries[0].PSIsContainer -or
        [string]$entries[0].FullName -cne $dataFile) {
        throw 'Disposable data directory contained unexpected entries.'
    }
    if ($PSCmdlet.ShouldProcess($dataDirectoryPath, 'Remove disposable logon data')) {
        Remove-Item -LiteralPath $dataDirectoryPath -Recurse -Force -ErrorAction Stop
    }
    if (Test-Path -LiteralPath $dataDirectoryPath) {
        throw 'Disposable data directory remained after cleanup.'
    }

    $result.status = 'pass'
    $result.data_sha256_before_uninstall = $dataHashBeforeCleanup
    $result.run_cleanup = 'removed'
    $result.install_cleanup = 'removed'
    $result.data_cleanup = 'removed'
    Write-JsonFile -Path (Join-Path $runDirectoryPath 'logon-result.json') -Value $result
    $result | ConvertTo-Json -Compress
}
catch {
    throw
}
