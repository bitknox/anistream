# Fetches libmpv for the Windows app (and the player spike) into desktop/windows/.libmpv/<arch>/, at the pinned
# URL below, and refuses it unless its SHA-256 matches.
#
#   powershell -File desktop/windows/scripts/libmpv.ps1 [-Arch arm64|x64]
#
# mpv's own releases (v0.41.0) ship mpv.exe linked statically, with no libmpv DLL. libmpv for
# Windows comes only from mpv's "development build" pre-release (tag git-release), which CI
# rebuilds from master and replaces: the file name carries the commit and the CI run, so a pinned
# name either downloads exactly this build or fails. Mirror the zip before relying on it in CI.
# docs/desktop-windows-spike.md says why the LGPL build.
param([ValidateSet('arm64', 'x64')][string]$Arch = 'arm64')
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$build = 'v0.41.0-dev-g703e59b0e-40117195812'
$pins = @{
  arm64 = @{ name = "libmpv-$build-aarch64-w64-mingw32-lgpl.zip"; sha256 = '7f5f029d8bfb2a28e72646dc3aa893ef2399e4e13986aea20abeecd1ddf42bea' }
  x64   = @{ name = "libmpv-$build-x86_64-w64-mingw32-lgpl.zip"; sha256 = '4c9d0e3ff428cb12483d0d27736a17d7e14d161024052f526e90f447ef20b452' }
}
$pin = $pins[$Arch]
$url = "https://github.com/mpv-player/mpv/releases/download/git-release/$($pin.name)"

$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root ".libmpv\$Arch"
$stamp = Join-Path $out 'stamp'
if ((Test-Path $stamp) -and ((Get-Content $stamp -Raw).Trim() -eq $pin.sha256)) {
  Write-Host "libmpv.ps1: libmpv $build ($Arch) is in $out"
  exit 0
}

$zip = Join-Path ([IO.Path]::GetTempPath()) $pin.name
Write-Host "libmpv.ps1: fetching $url"
Invoke-WebRequest $url -OutFile $zip
$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
if ($hash -ne $pin.sha256) {
  Remove-Item $zip
  throw "libmpv.ps1: $($pin.name) has SHA-256 $hash, expected $($pin.sha256)"
}
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
Expand-Archive $zip $out
Remove-Item $zip
Set-Content $stamp $pin.sha256
Write-Host "libmpv.ps1: wrote $out"
