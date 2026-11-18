# Снимает кадры анимации карточки-героя (для проверки качества перехода).
# Использование: powershell -ExecutionPolicy Bypass -File packaging/capture-hero.ps1 -OutDir <dir>

param(
    [string]$OutDir = "E:\Open\hero-shots",
    [string]$ClickX = "",
    [string]$ClickY = "",
    [string[]]$Flags = @("--view", "grid", "--no-refresh"),
    [switch]$NoClick
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path $PSScriptRoot -Parent
$outPath = $OutDir
$exe = Join-Path $projectRoot "target\release\craftpull.exe"
New-Item -ItemType Directory -Force -Path $outPath | Out-Null

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class Shot {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
"@

function Get-AppWindow([int]$procId) {
    $script:best = [IntPtr]::Zero
    $script:bestArea = 0
    $cb = [Shot+EnumProc]{
        param($hh, $l)
        $p2 = [uint32]0
        [void][Shot]::GetWindowThreadProcessId($hh, [ref]$p2)
        if ($p2 -eq $procId) {
            $sb = New-Object System.Text.StringBuilder 256
            [void][Shot]::GetClassName($hh, $sb, 256)
            $tb = New-Object System.Text.StringBuilder 256
            [void][Shot]::GetWindowText($hh, $tb, 256)
            if ($sb.ToString() -eq 'Window Class' -and $tb.ToString() -like 'CraftPull*') {
                $r = New-Object Shot+RECT
                [void][Shot]::GetWindowRect($hh, [ref]$r)
                $area = [Math]::Max(0, $r.R - $r.L) * [Math]::Max(0, $r.B - $r.T)
                if ($area -gt $script:bestArea) { $script:bestArea = $area; $script:best = $hh }
            }
        }
        return $true
    }
    [void][Shot]::EnumWindows($cb, [IntPtr]::Zero)
    return $script:best
}

function Restore-Window([IntPtr]$hwnd) {
    [void][Shot]::ShowWindow($hwnd, 9)
    [void][Shot]::BringWindowToTop($hwnd)
    [void][Shot]::SetForegroundWindow($hwnd)
    Start-Sleep -Milliseconds 400
}

function Start-App([string[]]$flags) {
    Get-Process craftpull -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 700
    $proc = Start-Process -FilePath $exe -ArgumentList $flags -PassThru
    for ($i = 0; $i -lt 40; $i++) {
        Start-Sleep -Milliseconds 400
        $hwnd = Get-AppWindow $proc.Id
        if ($hwnd -ne [IntPtr]::Zero -and $script:bestArea -gt 40000) {
            Restore-Window $hwnd
            return @{ Proc = $proc; Hwnd = $hwnd }
        }
    }
    throw "Окно CraftPull не появилось"
}

function Save-Shot([IntPtr]$hwnd, [string]$name) {
    $r = New-Object Shot+RECT
    [void][Shot]::GetWindowRect($hwnd, [ref]$r)
    $w = $r.R - $r.L
    $h = $r.B - $r.T
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    [void][Shot]::PrintWindow($hwnd, $hdc, 2)
    $g.ReleaseHdc($hdc)
    $g.Dispose()
    $bmp.Save((Join-Path $outPath $name), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Host ("  -> {0} ({1}x{2})" -f $name, $w, $h)
}

Write-Host "Снимаю сетку и анимацию карточки-героя..."

# 1. Обычная сетка.
$app = Start-App $Flags
Start-Sleep -Seconds 3
Save-Shot $app.Hwnd "hero-00-grid.png"

if ($NoClick) {
    Get-Process craftpull -ErrorAction SilentlyContinue | Stop-Process -Force
    Write-Host "Готово: $outPath"
    exit 0
}

# 2. Клик по плитке и кадры перехода.
$r = New-Object Shot+RECT
[void][Shot]::GetWindowRect($app.Hwnd, [ref]$r)
$cx = if ($ClickX) { [int]$ClickX } else { $r.L + 290 }
$cy = if ($ClickY) { [int]$ClickY } else { $r.T + 315 }
Write-Host ("  клик в ({0},{1})" -f $cx, $cy)
[void][Shot]::SetCursorPos($cx, $cy)
Start-Sleep -Milliseconds 150
# Первый клик только активирует окно, второй — по карточке.
[Shot]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 40
[Shot]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 120
[Shot]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 40
[Shot]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)

foreach ($ms in @(60, 90, 120, 150, 200, 260, 400, 700)) {
    Start-Sleep -Milliseconds $ms
    Save-Shot $app.Hwnd ("hero-t{0:D4}.png" -f $ms)
}

Get-Process craftpull -ErrorAction SilentlyContinue | Stop-Process -Force
Write-Host "Готово: $outPath"
