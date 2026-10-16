# End-to-end check of the installer on a Windows machine (used by CI, safe to run locally):
# silent per-user install -> registry + COM activation from the install dir -> silent
# uninstall -> previous ("foreign") handler restored, our keys gone.
#
# WARNING: temporarily overwrites HKCU\Software\Classes\.heif\ShellEx thumbnail handler.
param(
    [Parameter(Mandatory = $true)][string]$Setup,
    [Parameter(Mandatory = $true)][string]$Probe,
    [string]$Fixture = (Join-Path (Split-Path -Parent $PSScriptRoot) "testdata\quadrants_orientation6.heic")
)
$ErrorActionPreference = "Stop"
$clsid = "{218C7AE3-FDC4-4381-ABCD-D7385AFB7A4A}"
$handler = "{e357fccd-a995-4576-b01f-234630154e96}"
$foreign = "{C7657C4A-9F68-40fa-A4DF-96BC08EB3551}"  # Windows' own photo thumbnail provider
$classes = "HKCU:\Software\Classes"
$failed = $false
function Check($ok, $msg) {
    if ($ok) { Write-Host "ok   $msg" } else { Write-Host "FAIL $msg" -ForegroundColor Red; $script:failed = $true }
}
function HandlerOf($ext) {
    $key = "$classes\$ext\ShellEx\$handler"
    if (Test-Path $key) { (Get-Item $key).GetValue("") } else { $null }
}

# Simulate a pre-existing handler for .heif that must survive install + uninstall.
New-Item -Path "$classes\.heif\ShellEx\$handler" -Force | Out-Null
Set-ItemProperty -Path "$classes\.heif\ShellEx\$handler" -Name "(default)" -Value $foreign

Write-Host "== install"
$log = Join-Path $env:RUNNER_TEMP "setup.log"
if (-not $env:RUNNER_TEMP) { $log = Join-Path $env:TEMP "heic-setup.log" }
$p = Start-Process -FilePath $Setup -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER", "/LOG=`"$log`"" -Wait -PassThru
Check ($p.ExitCode -eq 0) "setup exit code $($p.ExitCode)"

$inproc = (Get-Item "$classes\CLSID\$clsid\InprocServer32" -ErrorAction SilentlyContinue)
$dllPath = if ($inproc) { $inproc.GetValue("") } else { $null }
Check ($dllPath -and (Test-Path $dllPath)) "InprocServer32 -> $dllPath"
Check ($inproc -and $inproc.GetValue("ThreadingModel") -eq "Apartment") "ThreadingModel = Apartment"
foreach ($ext in ".heic", ".heif", ".hif") { Check ((HandlerOf $ext) -eq $clsid) "$ext thumbnail handler registered" }
$backup = Get-ItemProperty "HKCU:\Software\HEIC Explorer Preview\ThumbnailHandlerBackup" -ErrorAction SilentlyContinue
Check ($backup -and $backup.".heif" -eq $foreign) ".heif previous handler backed up"
Check (-not ($backup -and $backup.".heic")) ".heic has no backup (nothing was registered before)"

Write-Host "== COM activation through the registry (DLLs must load from the install dir)"
$out = & $Probe $Fixture 256 2>&1 | Out-String
Write-Host $out
Check ($LASTEXITCODE -eq 0 -and $out -match "^64x96") "probe produced a 64x96 thumbnail"

Write-Host "== shell thumbnail (informational)"
& $Probe --shell $Fixture 256 2>&1 | Write-Host

Write-Host "== uninstall"
$appDir = Split-Path -Parent $dllPath
$uninstaller = Join-Path $appDir "unins000.exe"
Start-Process -FilePath $uninstaller -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART" -Wait
# The uninstaller re-launches itself from %TEMP%; wait until it removed itself.
$deadline = (Get-Date).AddSeconds(90)
while ((Test-Path $uninstaller) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }
Start-Sleep -Seconds 2
Check (-not (Test-Path $uninstaller)) "uninstaller finished"

Check (-not (Test-Path "$classes\CLSID\$clsid")) "CLSID removed"
Check ((HandlerOf ".heic") -eq $null) ".heic handler removed"
Check ((HandlerOf ".hif") -eq $null) ".hif handler removed"
Check ((HandlerOf ".heif") -eq $foreign) ".heif previous handler restored"
Check (-not (Test-Path "HKCU:\Software\HEIC Explorer Preview")) "backup key removed"
$left = @(Get-ChildItem $appDir -Recurse -ErrorAction SilentlyContinue | Where-Object { $_.Extension -ne ".old" })
Check ($left.Count -eq 0) "install directory empty (except locked *.old files)"

Remove-Item -Path "$classes\.heif\ShellEx\$handler" -Force -ErrorAction SilentlyContinue
if ($failed) { Get-Content $log -ErrorAction SilentlyContinue | Select-Object -Last 60; exit 1 }
Write-Host "installer smoke test passed"
