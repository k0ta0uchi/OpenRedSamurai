# OpenRedSamurai - Register to Task Scheduler with Administrator Privileges (No UAC)
$exePath = Join-Path $env:LOCALAPPDATA "RED SAMURAI\redsamurai-config.exe"

$action = New-ScheduledTaskAction -Execute $exePath -Argument "--tray"
$trigger = New-ScheduledTaskTrigger -AtLogOn
$principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries

Register-ScheduledTask -TaskName "OpenRedSamurai" -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Force

Write-Host "==========================================================" -ForegroundColor Green
Write-Host " OpenRedSamurai has been registered to Task Scheduler!" -ForegroundColor Green
Write-Host " - Runs on logon with Highest privileges (No UAC prompt)" -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green
