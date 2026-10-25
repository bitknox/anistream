# Installs the Windows app's installer silently, checks what it put in place, starts the app,
# then uninstalls it silently and checks that it took everything back. The windows job in
# .github/workflows/apps-release.yml runs it on each installer it makes.
#
#   powershell -File packaging/windows/check-installer.ps1 <setup.exe> [-Version 2.0.1]
#
# For the current user, as the installer always installs: nothing here needs an administrator.
param(
  [Parameter(Mandatory = $true)][string]$Setup,
  [string]$Version = ''
)
$ErrorActionPreference = 'Stop'

function Fail($message) { throw "check-installer.ps1: $message" }

# Starts a program and waits for it alone, never for what it leaves running: Inno Setup's
# uninstaller hands its work to a copy of itself in the temporary folder, which
# Start-Process -Wait would wait for as well.
function Run($file, $arguments, $what) {
  $process = Start-Process -FilePath $file -ArgumentList $arguments -PassThru
  if (-not $process.WaitForExit(300000)) { $process.Kill(); Fail "$what did not finish within 5 minutes" }
  return $process.ExitCode
}

$dir = Join-Path $env:LOCALAPPDATA 'Programs\anistream-app'
$exe = Join-Path $dir 'anistream.exe'
$shortcut = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\anistream.lnk'
$scheme = 'HKCU:\Software\Classes\anistream'
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{002C5B79-DD92-4D47-B871-D02D4B83AB37}_is1'
$log = Join-Path ([IO.Path]::GetTempPath()) 'anistream-setup.log'

Write-Host "Installing $Setup"
$code = Run $Setup @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/LOG=`"$log`"") 'the installer'
if ($code -ne 0) { Get-Content $log -Tail 30; Fail "the installer exited with $code" }

foreach ($path in @($exe, (Join-Path $dir 'anistream_ffi.dll'), (Join-Path $dir 'libmpv-2.dll'),
    (Join-Path $dir 'third-party\libmpv\LICENSE.LGPL'), (Join-Path $dir 'third-party\LICENSE-anistream.txt'),
    $shortcut)) {
  if (-not (Test-Path $path)) { Fail "no $path after installing" }
}
$command = (Get-ItemProperty "$scheme\shell\open\command").'(default)'
if ($command -ne "`"$exe`" `"%1`"") { Fail "anistream:// opens '$command'" }
if ($null -eq (Get-ItemProperty $scheme).'URL Protocol') { Fail 'anistream:// is not marked as a URL protocol' }
$entry = Get-ItemProperty $uninstallKey
if ($Version -and $entry.DisplayVersion -ne $Version) { Fail "Settings > Apps lists version $($entry.DisplayVersion), not $Version" }
Write-Host "Installed $($entry.DisplayName) $($entry.DisplayVersion) in $dir, $([math]::Round((Get-ChildItem $dir -Recurse | Measure-Object Length -Sum).Sum / 1MB)) MB"

# The app starts and is still running after a while, on whatever display the machine has.
$crash = Join-Path ([IO.Path]::GetTempPath()) 'anistream-windows\crash.txt'
Remove-Item $crash -ErrorAction SilentlyContinue
$app = Start-Process -FilePath $exe -PassThru
Start-Sleep -Seconds 20
if ($app.HasExited) {
  if (Test-Path $crash) { Get-Content $crash -Tail 40 }
  Fail "the app exited with $($app.ExitCode) within 20 seconds"
}
Write-Host "The app is running ($($app.Id)), with $([math]::Round($app.WorkingSet64 / 1MB)) MB in use"
Stop-Process -Id $app.Id -Force
Start-Sleep -Seconds 2

Write-Host 'Uninstalling'
$uninstaller = Join-Path $dir 'unins000.exe'
$code = Run $uninstaller @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') 'the uninstaller'
if ($code -ne 0) { Fail "the uninstaller exited with $code" }
# The uninstaller finishes by deleting itself from a copy in the temporary folder.
for ($i = 0; $i -lt 30 -and (Test-Path $exe); $i++) { Start-Sleep -Seconds 1 }
foreach ($path in @($exe, $shortcut, $scheme, $uninstallKey)) {
  if (Test-Path $path) { Fail "$path is still there after uninstalling" }
}
Write-Host 'Uninstalled: the files, the Start menu entry, anistream:// and the Settings entry are gone'
