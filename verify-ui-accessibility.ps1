[CmdletBinding()]
param(
    [string]$ExePath
)

# Read-only UI smoke: this script invokes tabs only.  It never clicks Device,
# Apply, profile actions, dialogs, or any control that can reach HID/file I/O.
$ErrorActionPreference = 'Stop'
if ([string]::IsNullOrWhiteSpace($ExePath)) {
    $ExePath = Join-Path -Path $PSScriptRoot -ChildPath 'target/release/redsamurai-config.exe'
}
$exe = (Resolve-Path -LiteralPath $ExePath).Path
$proc = Start-Process -FilePath $exe -PassThru

try {
    if (-not $proc.WaitForInputIdle(10000)) {
        throw 'The editor did not become idle within 10 seconds.'
    }
    Start-Sleep -Milliseconds 800
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes

    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $pidCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
        $proc.Id
    )
$windows = $root.FindAll(
    [System.Windows.Automation.TreeScope]::Children,
    $pidCondition
)
$window = $null
for ($i = 0; $i -lt $windows.Count; $i++) {
    $candidate = $windows.Item($i)
    if ($candidate.Current.ControlType.Id -eq [System.Windows.Automation.ControlType]::Window.Id) {
        $window = $candidate
        break
    }
}
if ($null -eq $window) {
    throw "Could not find a UI Automation window for PID $($proc.Id)."
}

    $trueCondition = [System.Windows.Automation.Condition]::TrueCondition
    function Get-UiNodes {
        $nodes = $window.FindAll(
            [System.Windows.Automation.TreeScope]::Descendants,
            $trueCondition
        )
        $result = @()
        for ($i = 0; $i -lt $nodes.Count; $i++) {
            $node = $nodes.Item($i)
            if ($node.Current.AutomationId) {
                $result += [pscustomobject]@{
                    AutomationId = $node.Current.AutomationId
                    Name = $node.Current.Name
                    ControlType = $node.Current.ControlType.ProgrammaticName
                }
            }
        }
        return $result
    }

    function Assert-HasId([object[]]$nodes, [string]$id) {
        if (-not ($nodes | Where-Object AutomationId -eq $id)) {
            throw "Missing AutomationId: $id"
        }
    }

    function Invoke-UiId([string]$id) {
        $condition = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::AutomationIdProperty,
            $id
        )
        $node = $window.FindFirst(
            [System.Windows.Automation.TreeScope]::Descendants,
            $condition
        )
        if ($null -eq $node) {
            throw "Cannot invoke missing AutomationId: $id"
        }
        $invoke = $node.GetCurrentPattern(
            [System.Windows.Automation.InvokePattern]::Pattern
        )
        $invoke.Invoke()
        Start-Sleep -Milliseconds 180
    }

    $initial = @(Get-UiNodes)
    Assert-HasId $initial 'red-samurai.titlebar.close'
    Assert-HasId $initial 'red-samurai.assignment.row.1'
    Assert-HasId $initial 'red-samurai.slider_MouseSensitivity'
    Assert-HasId $initial 'red-samurai.bottom_6'

    Invoke-UiId 'red-samurai.tab_1'
    $dpi = @(Get-UiNodes)
    Assert-HasId $dpi 'red-samurai.dpi_vslider_0'
    Assert-HasId $dpi 'red-samurai.dpi.stage-enabled.0'

    Invoke-UiId 'red-samurai.tab_2'
    $light = @(Get-UiNodes)
    Assert-HasId $light 'red-samurai.light.palette.0'
    Assert-HasId $light 'red-samurai.light.custom-color'

    Invoke-UiId 'red-samurai.tab_3'
    $info = @(Get-UiNodes)
    Assert-HasId $info 'red-samurai.tab_3'
    Assert-HasId $info 'red-samurai.info.update'

    Invoke-UiId 'red-samurai.tab_0'
    $restored = @(Get-UiNodes)
    Assert-HasId $restored 'red-samurai.slider_MouseSensitivity'
    Assert-HasId $restored 'red-samurai.titlebar.close'

    $all = @($initial + $dpi + $light + $info + $restored)
    $bad = @($all | Where-Object { $_.AutomationId -notlike 'red-samurai.*' })
    if ($bad.Count -gt 0) {
        throw "Found AutomationId outside the red-samurai namespace: $($bad.AutomationId -join ', ')"
    }

    $hash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
    [pscustomobject]@{
        Result = 'PASS'
        Pid = $proc.Id
        Executable = $exe
        Sha256 = $hash
        InitialNodes = $initial.Count
        DpiNodes = $dpi.Count
        LightNodes = $light.Count
        InfoNodes = $info.Count
        RestoredGeneralNodes = $restored.Count
        CheckedIds = @(
            'red-samurai.titlebar.close'
            'red-samurai.assignment.row.1'
            'red-samurai.slider_MouseSensitivity'
            'red-samurai.dpi_vslider_0'
            'red-samurai.light.custom-color'
            'red-samurai.info.update'
            'red-samurai.bottom_6'
        )
    } | ConvertTo-Json -Depth 4
}
finally {
    if ($proc -and -not $proc.HasExited) {
        $proc.CloseMainWindow() | Out-Null
        if (-not $proc.WaitForExit(3000)) {
            $proc.Kill()
        }
    }
}
