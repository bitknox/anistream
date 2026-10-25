# Runs the player spike on the signed-in desktop and captures its window, for a machine reached
# over SSH: an SSH session has no desktop, so the run happens in a scheduled task in the console
# user's interactive session, and this script waits for it.
#
#   powershell -File spike-shot.ps1 -Exe <Anistream.PlayerSpike.exe> -Out <dir> -Name <shot>
#              [-At 6,12] [-Wait 30] [-Arguments '<the spike's arguments>']
#
# -At lists the seconds after launch at which to take a PNG each (<Name>-<seconds>.png). The
# spike's report.txt and mpv.log land in -Out as well.
param(
  [Parameter(Mandatory)][string]$Exe,
  [Parameter(Mandatory)][string]$Out,
  [Parameter(Mandatory)][string]$Name,
  # A string, since powershell -File passes "6,12" as one argument.
  [string]$At = '8',
  [int]$Wait = 0,
  [string]$Arguments = '',
  # Capture the whole primary screen, for what shows outside the window: a notification, the
  # media overlay.
  [switch]$Screen,
  # Before each capture: the pointer to 'x,y' on the screen (over the taskbar button, for its
  # thumbnail), and with -Volume a volume key up and down, which opens the media overlay.
  [string]$Point = '',
  [switch]$Volume,
  # Before each capture: Win+T, then this many presses of Right, which shows that taskbar
  # button's thumbnail as a hover would. -1 for none.
  [int]$Taskbar = -1,
  # An anistream:// link opened through the shell 5 s after launch, as a click on it would: it
  # reaches the running copy, which is the one captured.
  [string]$Open = '',
  [switch]$Inner
)
$ErrorActionPreference = 'Stop'
$marks = @($At -split ',' | ForEach-Object { [int]$_ })
New-Item -ItemType Directory -Force $Out | Out-Null

if (-not $Inner) {
  # Outer half: hand the run to the console session, and wait for its done file.
  $done = Join-Path $Out "$Name.done"
  Remove-Item $done -ErrorAction SilentlyContinue
  $quoted = "'" + ($Arguments -replace "'", "''") + "'"
  $screenArg = if ($Screen) { ' -Screen' } else { '' }
  if ($Volume) { $screenArg += ' -Volume' }
  if ($Point) { $screenArg += " -Point '$Point'" }
  if ($Taskbar -ge 0) { $screenArg += " -Taskbar $Taskbar" }
  if ($Open) { $screenArg += " -Open '" + ($Open -replace "'", "''") + "'" }
  $command = "& '$PSCommandPath' -Inner$screenArg -Exe '$Exe' -Out '$Out' -Name '$Name' -At '$At' -Wait $Wait -Arguments $quoted *> '$Out\$Name.task.log'; New-Item '$done' | Out-Null"
  $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
  $task = 'anistream-player-spike'
  $action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -EncodedCommand $encoded"
  $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive -RunLevel Limited
  Register-ScheduledTask -TaskName $task -Action $action -Principal $principal -Force | Out-Null
  Start-ScheduledTask -TaskName $task
  $deadline = (Get-Date).AddSeconds(($marks | Measure-Object -Maximum).Maximum + $Wait + 90)
  while (-not (Test-Path $done) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }
  Unregister-ScheduledTask -TaskName $task -Confirm:$false
  Get-Content "$Out\$Name.task.log" -ErrorAction SilentlyContinue
  if (-not (Test-Path $done)) { throw "spike-shot.ps1: the run did not finish" }
  exit 0
}

# Inner half, on the desktop: start the spike, capture its window at each mark, then close it.
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SpikeWin {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr hwnd, int attr, out RECT rect, int size);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
  [DllImport("user32.dll")] public static extern void keybd_event(byte key, byte scan, uint flags, UIntPtr extra);
}
'@
[SpikeWin]::SetProcessDPIAware() | Out-Null

# One copy runs at a time: a second launch hands its link to the first and exits.
Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($Exe)) -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500
$proc = Start-Process -FilePath $Exe -ArgumentList $Arguments -PassThru -WorkingDirectory (Split-Path $Exe)
$start = Get-Date
$opened = -not $Open
foreach ($mark in ($marks | Sort-Object)) {
  while (((Get-Date) - $start).TotalSeconds -lt $mark) {
    if (-not $opened -and ((Get-Date) - $start).TotalSeconds -ge 5) {
      Start-Process $Open
      $opened = $true
    }
    Start-Sleep -Milliseconds 200
  }
  $proc.Refresh()
  if ($proc.HasExited) { Write-Output "the spike exited with $($proc.ExitCode) before $mark s"; break }
  $hwnd = $proc.MainWindowHandle
  [SpikeWin]::SetForegroundWindow($hwnd) | Out-Null
  Start-Sleep -Milliseconds 300
  if ($Point) {
    $xy = $Point -split ','
    # Moved onto it in steps, as a hand would: the taskbar shows a thumbnail on hover, not on a jump.
    foreach ($dy in 40, 20, 8, 0) {
      [SpikeWin]::SetCursorPos([int]$xy[0], [int]$xy[1] - $dy) | Out-Null
      [SpikeWin]::mouse_event(1, 0, 0, 0, [UIntPtr]::Zero)
      Start-Sleep -Milliseconds 120
    }
    Start-Sleep -Milliseconds 2500
  }
  if ($Taskbar -ge 0) {
    [SpikeWin]::keybd_event(0x5B, 0, 0, [UIntPtr]::Zero)
    [SpikeWin]::keybd_event(0x54, 0, 0, [UIntPtr]::Zero)
    [SpikeWin]::keybd_event(0x54, 0, 2, [UIntPtr]::Zero)
    [SpikeWin]::keybd_event(0x5B, 0, 2, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 600
    if ($Taskbar -gt 0) {
      1..$Taskbar | ForEach-Object {
        [SpikeWin]::keybd_event(0x27, 0, 0, [UIntPtr]::Zero)
        [SpikeWin]::keybd_event(0x27, 0, 2, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 400
      }
    }
    Start-Sleep -Milliseconds 2000
  }
  if ($Volume) {
    # VK_VOLUME_UP then VK_VOLUME_DOWN: the level ends where it was, the overlay shows.
    foreach ($key in 0xAF, 0xAE) {
      [SpikeWin]::keybd_event($key, 0, 0, [UIntPtr]::Zero)
      [SpikeWin]::keybd_event($key, 0, 2, [UIntPtr]::Zero)
      Start-Sleep -Milliseconds 150
    }
    Start-Sleep -Milliseconds 900
  }
  $r = New-Object SpikeWin+RECT
  # DWMWA_EXTENDED_FRAME_BOUNDS: the window as drawn, without the invisible resize borders.
  [SpikeWin]::DwmGetWindowAttribute($hwnd, 9, [ref]$r, 16) | Out-Null
  if ($Screen) {
    Add-Type -AssemblyName System.Windows.Forms
    $b = [Windows.Forms.Screen]::PrimaryScreen.Bounds
    $r.Left = $b.Left; $r.Top = $b.Top; $r.Right = $b.Right; $r.Bottom = $b.Bottom
  }
  $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
  $bmp = New-Object Drawing.Bitmap $w, $h
  $g = [Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object Drawing.Size $w, $h))
  $png = Join-Path $Out "$Name-$mark.png"
  $bmp.Save($png, [Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  Write-Output "captured $png (${w}x${h})"
}
if ($Wait -gt 0) { $proc.WaitForExit($Wait * 1000) | Out-Null }
$proc.Refresh()
if (-not $proc.HasExited) { $proc.CloseMainWindow() | Out-Null; $proc.WaitForExit(5000) | Out-Null }
if (-not $proc.HasExited) { $proc.Kill() }
Write-Output "the spike exited with $($proc.ExitCode)"
