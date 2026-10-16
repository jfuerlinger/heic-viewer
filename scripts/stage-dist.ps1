# Collects everything the installer needs into .\dist and refuses unexpected native DLLs
# (guards against e.g. the GPL x265 encoder sneaking in through vcpkg default features).
param(
    [string]$Triplet = "x64-windows-staticcrt",
    [string]$BuildProfile = "release"
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$vcpkgDir = Join-Path $root "vcpkg_installed\$Triplet"
$dist = Join-Path $root "dist"

if (Test-Path $dist) { Remove-Item $dist -Recurse -Force }
New-Item -ItemType Directory -Path "$dist\licenses" | Out-Null

$expected = @("heif.dll", "libde265.dll")
$found = @(Get-ChildItem "$vcpkgDir\bin\*.dll" | ForEach-Object { $_.Name.ToLowerInvariant() } | Sort-Object)
Write-Host "vcpkg DLLs: $($found -join ', ')"
$unexpected = $found | Where-Object { $expected -notcontains $_ }
$missing = $expected | Where-Object { $found -notcontains $_ }
if ($unexpected -or $missing) {
    throw "Unexpected native DLL set. Unexpected: [$($unexpected -join ', ')] Missing: [$($missing -join ', ')]"
}

Copy-Item (Join-Path $root "target\$BuildProfile\heic_thumbnail.dll") $dist
Copy-Item (Join-Path $root "target\$BuildProfile\heic-convert.exe") $dist
foreach ($dll in $expected) { Copy-Item "$vcpkgDir\bin\$dll" $dist }

Copy-Item (Join-Path $root "LICENSE") "$dist\LICENSE.txt"
Copy-Item (Join-Path $root "THIRD-PARTY-LICENSES.txt") $dist
Copy-Item (Join-Path $root "licenses\*") "$dist\licenses"
# The copyright files vcpkg installed for the exact build that is shipped.
foreach ($port in @("libheif", "libde265")) {
    $copyright = "$vcpkgDir\share\$port\copyright"
    if (-not (Test-Path $copyright)) { throw "missing $copyright" }
    Copy-Item $copyright "$dist\licenses\$port-vcpkg-copyright.txt"
}

# License page shown by the installer: summary + MIT + third-party list + full LGPL/GPL text.
$parts = @(
    (Join-Path $root "installer\license-page-header.txt"),
    (Join-Path $root "LICENSE"),
    (Join-Path $root "THIRD-PARTY-LICENSES.txt"),
    (Join-Path $root "licenses\libheif-COPYING.txt")
)
$separator = "`r`n`r`n==============================================================================`r`n`r`n"
$text = ($parts | ForEach-Object { (Get-Content $_ -Raw -Encoding UTF8).Replace("`r`n", "`n").Replace("`n", "`r`n") }) -join $separator
[System.IO.File]::WriteAllText("$dist\license-page.txt", $text, (New-Object System.Text.UTF8Encoding($true)))

Get-ChildItem $dist -Recurse | Select-Object FullName, Length | Format-Table -AutoSize
