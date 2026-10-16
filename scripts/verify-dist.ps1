# Static checks of the staged binaries. Needs dumpbin (run inside a VS developer shell).
$ErrorActionPreference = "Stop"
$dist = Join-Path (Split-Path -Parent $PSScriptRoot) "dist"
$failed = $false
function Fail($msg) { Write-Host "FAIL: $msg" -ForegroundColor Red; $script:failed = $true }

$exports = dumpbin /nologo /exports "$dist\heic_thumbnail.dll" | Out-String
foreach ($name in @("DllGetClassObject", "DllCanUnloadNow")) {
    if ($exports -notmatch "\b$name\b") { Fail "heic_thumbnail.dll does not export $name" }
}

$deps = dumpbin /nologo /dependents "$dist\heic_thumbnail.dll" | Out-String
Write-Host $deps
# heif.dll must be delay-loaded (listed under "delay load dependencies", not the regular ones).
$regular = ($deps -split "delay load dependencies")[0]
if ($regular -match "heif\.dll") { Fail "heif.dll is a regular (non-delay-loaded) import" }
if ($deps -notmatch "heif\.dll") { Fail "heif.dll is not imported at all" }

foreach ($dll in Get-ChildItem "$dist\*.dll") {
    $d = dumpbin /nologo /dependents $dll.FullName | Out-String
    if ($d -match "(?i)(vcruntime|msvcp)\d+") { Fail "$($dll.Name) depends on the VC++ runtime ($($Matches[0])); expected static CRT" }
    $h = dumpbin /nologo /headers $dll.FullName | Out-String
    if ($h -notmatch "8664 machine \(x64\)") { Fail "$($dll.Name) is not an x64 image" }
}

$heifDeps = dumpbin /nologo /dependents "$dist\heif.dll" | Out-String
if ($heifDeps -notmatch "libde265\.dll") { Fail "heif.dll does not depend on libde265.dll (HEVC decoder missing?)" }
if ($heifDeps -match "(?i)x265") { Fail "heif.dll links x265 (GPL encoder)" }

if ($failed) { exit 1 }
Write-Host "dist verification passed"
