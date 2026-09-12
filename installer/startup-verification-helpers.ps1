<#
.SYNOPSIS
    Side-effect-free assertions for the bounded current-user startup check.

.DESCRIPTION
    These helpers keep command-line, identity, and cleanup assertions small
    enough for the installer harness to exercise without launching a process.
    They intentionally do not perform session transitions or filesystem,
    registry, HID, or input operations.
#>
Set-StrictMode -Version Latest

function Get-CurrentUserIdentityName {
    [CmdletBinding()]
    param()

    try {
        $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    }
    catch {
        throw 'Current-user identity could not be inspected.'
    }
    if ($null -eq $identity -or [string]::IsNullOrWhiteSpace($identity.Name)) {
        throw 'Current-user identity could not be inspected.'
    }
    return $identity.Name
}

function Get-CurrentSessionId {
    [CmdletBinding()]
    param()

    try {
        $process = Get-Process -Id $PID -ErrorAction Stop
        return [int]$process.SessionId
    }
    catch {
        throw 'Current desktop session could not be inspected.'
    }
}

function Get-ProcessUserIdentity {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [AllowNull()]
        [object] $ProcessRecord
    )

    if ($null -eq $ProcessRecord) {
        throw 'Process user identity could not be inspected.'
    }
    $processId = [int] $ProcessRecord.ProcessId
    try {
        $owner = Invoke-CimMethod -InputObject $ProcessRecord `
            -MethodName GetOwner -ErrorAction Stop
    }
    catch {
        throw "Process user identity could not be inspected for PID $processId."
    }
    if ($null -eq $owner -or $null -eq $owner.ReturnValue -or
        [int]$owner.ReturnValue -ne 0 -or
        [string]::IsNullOrWhiteSpace([string]$owner.User) -or
        [string]::IsNullOrWhiteSpace([string]$owner.Domain)) {
        throw "Process user identity could not be inspected for PID $processId."
    }
    return ('{0}\{1}' -f [string]$owner.Domain, [string]$owner.User)
}

function Assert-ProcessUserIdentity {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string] $ExpectedIdentity,

        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string] $ActualIdentity,

        [Parameter(Mandatory = $true)]
        [int] $ProcessId
    )

    if ([string]::IsNullOrWhiteSpace($ExpectedIdentity) -or
        [string]::IsNullOrWhiteSpace($ActualIdentity) -or
        -not $ActualIdentity.Equals($ExpectedIdentity, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Started process identity did not match current user for PID $ProcessId."
    }
    return $true
}

function Assert-ProcessSession {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [int] $ExpectedSessionId,

        [Parameter(Mandatory = $true)]
        [int] $ActualSessionId,

        [Parameter(Mandatory = $true)]
        [int] $ProcessId
    )

    if ($ActualSessionId -ne $ExpectedSessionId) {
        throw "Started process session did not match verifier session for PID $ProcessId."
    }
    return $true
}

function Assert-ExactTrayProcessCommandLine {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string] $ExpectedCommand,

        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string] $ActualCommandLine,

        [Parameter(Mandatory = $true)]
        [int] $ProcessId
    )

    if ($null -eq $ActualCommandLine -or
        $ActualCommandLine.Trim() -cne $ExpectedCommand) {
        throw "Started process command line must exactly match the quoted Run command for PID $ProcessId."
    }
    return $true
}

function Assert-ProcessCleanupState {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string] $State
    )

    if ($State -notin @('not-started', 'not-required', 'complete')) {
        throw 'Process cleanup could not be verified; refusing to claim completion.'
    }
    return $true
}
