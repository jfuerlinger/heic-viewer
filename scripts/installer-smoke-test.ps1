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

# Informational only: CI runners are elevated, and COM ignores per-user (HKCU) registrations
# for the shell's out-of-process thumbnail host when the caller is elevated.
Write-Host "== context menu registration"
$verb = "HEICExplorerPreview.ToJpeg"
foreach ($key in @(
        "$classes\SystemFileAssociations\.heic\shell\$verb",
        "$classes\SystemFileAssociations\.heif\shell\$verb",
        "$classes\SystemFileAssociations\.hif\shell\$verb",
        "$classes\Directory\shell\$verb",
        "$classes\Directory\Background\shell\$verb")) {
    $cmd = (Get-Item "$key\command" -ErrorAction SilentlyContinue)
    $cmdLine = if ($cmd) { $cmd.GetValue("") } else { "" }
    Check ($cmdLine -like "*heic-convert.exe*--lang=en*") "verb $key -> $cmdLine"
}
Check ((Get-Item "$classes\SystemFileAssociations\.heic\shell\$verb").GetValue("MultiSelectModel") -eq "Player") "MultiSelectModel = Player"

$convert = Join-Path (Split-Path -Parent $dllPath) "heic-convert.exe"
$work = Join-Path ([System.IO.Path]::GetTempPath()) "heic-convert-ci"
Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "== heic-convert --quiet on a folder"
New-Item -ItemType Directory -Force "$work\folder" | Out-Null
foreach ($f in "quadrants_srgb.heic", "quadrants_orientation6.heic", "alpha.heic", "tenbit_p3.heic", "with_thumbnail.heic") {
    Copy-Item (Join-Path (Split-Path -Parent $Fixture) $f) "$work\folder"
}
Set-Content "$work\folder\IMG.jpg" "existing" -NoNewline
Copy-Item (Join-Path (Split-Path -Parent $Fixture) "quadrants_srgb.heic") "$work\folder\IMG.heic"
$p = Start-Process -FilePath $convert -ArgumentList "--quiet", "`"$work\folder`"" -Wait -PassThru -RedirectStandardOutput "$work\quiet.txt"
Get-Content "$work\quiet.txt" | Write-Host
Check ($p.ExitCode -eq 0) "heic-convert --quiet exit code $($p.ExitCode)"
Check (@(Get-ChildItem "$work\folder\*.jpg").Count -eq 7) "6 JPEGs written next to the existing IMG.jpg"
Check ((Get-Content "$work\folder\IMG.jpg" -Raw) -eq "existing") "existing IMG.jpg untouched"
Check (Test-Path "$work\folder\IMG (1).jpg") "conflict resolved as 'IMG (1).jpg'"

Write-Host "== multi-selection: 50 processes -> one instance"
New-Item -ItemType Directory -Force "$work\multi", "$work\reports" | Out-Null
1..50 | ForEach-Object { Copy-Item (Join-Path (Split-Path -Parent $Fixture) "quadrants_srgb.heic") "$work\multi\photo$_.heic" }
$env:HEIC_CONVERT_REPORT = "$work\reports"
$env:HEIC_CONVERT_INSTANCE = ".ci$PID"
$procs = 1..50 | ForEach-Object { Start-Process -FilePath $convert -ArgumentList "--lang=en", "`"$work\multi\photo$_.heic`"" -PassThru }
$procs | Wait-Process -Timeout 120
Remove-Item Env:HEIC_CONVERT_REPORT, Env:HEIC_CONVERT_INSTANCE
$reports = @(Get-ChildItem "$work\reports\report-*.txt")
foreach ($r in $reports) { Write-Host "--- $($r.Name)"; Get-Content $r | Write-Host }
Check ($reports.Count -eq 1) "exactly one serving process (found $($reports.Count))"
Check ($reports.Count -ge 1 -and (Get-Content $reports[0] -Raw) -match "converted=50") "that process converted all 50 files"
Check (@(Get-ChildItem "$work\multi\*.jpg").Count -eq 50) "50 JPEGs written"

Write-Host "== shell thumbnail (informational, may fail on elevated CI runners)"
& $Probe --shell $Fixture 256 2>&1 | Write-Host
Write-Host "shell probe exit code: $LASTEXITCODE (ignored)"

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
foreach ($key in @("$classes\SystemFileAssociations\.heic\shell\$verb", "$classes\Directory\shell\$verb", "$classes\Directory\Background\shell\$verb")) {
    Check (-not (Test-Path $key)) "context menu verb removed: $key"
}
$left = @(Get-ChildItem $appDir -Recurse -ErrorAction SilentlyContinue | Where-Object { $_.Extension -ne ".old" })
Check ($left.Count -eq 0) "install directory empty (except locked *.old files)"

Remove-Item -Path "$classes\.heif\ShellEx\$handler" -Force -ErrorAction SilentlyContinue
if ($failed) { Get-Content $log -ErrorAction SilentlyContinue | Select-Object -Last 60; exit 1 }
Write-Host "installer smoke test passed"
# Do not let the exit code of the last native command (e.g. the informational probe) leak.
exit 0
