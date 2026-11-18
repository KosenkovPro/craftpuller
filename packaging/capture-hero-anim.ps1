# Снимает кадры анимации раскрытия карточки-героя.
# Приложение стартует с флагом --hero: карточка раскрывается сразу,
# а скрипт хватает кадры в плотном цикле, пока идёт переход.
# Использование: powershell -ExecutionPolicy Bypass -File packaging/capture-hero-anim.ps1

param(
    [string]$OutDir = "E:\Open\hero-anim",
    [string]$Slug = "storytold/pdfcraft",
    [int]$Frames = 10
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
public class Anim {
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
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
"@

function Get-AppWindow([int]$procId) {
    $script:best = [IntPtr]::Zero; $script:bestArea = 0
    $cb = [Anim+EnumProc]{
        param($hh, $l)
        $p2 = [uint32]0
        [void][Anim]::GetWindowThreadProcessId($hh, [ref]$p2)
        if ($p2 -eq $procId) {
            $sb = New-Object System.Text.StringBuilder 256
            [void][Anim]::GetClassName($hh, $sb, 256)
            $tb = New-Object System.Text.StringBuilder 256
            [void][Anim]::GetWindowText($hh, $tb, 256)
            if ($sb.ToString() -eq 'Window Class' -and $tb.ToString() -like 'CraftPull*') {
                $r = New-Object Anim+RECT
                [void][Anim]::GetWindowRect($hh, [ref]$r)
                $area = [Math]::Max(0, $r.R - $r.L) * [Math]::Max(0, $r.B - $r.T)
                if ($area -gt $script:bestArea) { $script:bestArea = $area; $script:best = $hh }
            }
        }
        return $true
    }
    [void][Anim]::EnumWindows($cb, [IntPtr]::Zero)
    return $script:best
}

function Save-Shot([IntPtr]$hwnd, [string]$name) {
    $r = New-Object Anim+RECT
    [void][Anim]::GetWindowRect($hwnd, [ref]$r)
    $w = $r.R - $r.L; $h = $r.B - $r.T
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    [void][Anim]::PrintWindow($hwnd, $hdc, 2)
    $g.ReleaseHdc($hdc); $g.Dispose()
    $bmp.Save((Join-Path $outPath $name), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Host ("  -> {0}" -f $name)
}

Write-Host "Снимаю анимацию раскрытия карточки-героя..."
Get-Process craftpull -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 700
$proc = Start-Process -FilePath $exe -ArgumentList @("--view", "grid", "--hero", $Slug, "--no-refresh") -PassThru

$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt 200; $i++) {
    Start-Sleep -Milliseconds 30
    $hwnd = Get-AppWindow $proc.Id
    if ($hwnd -ne [IntPtr]::Zero -and $script:bestArea -gt 40000) { break }
}
[void][Anim]::ShowWindow($hwnd, 9)
[void][Anim]::BringWindowToTop($hwnd)
[void][Anim]::SetForegroundWindow($hwnd)

for ($i = 0; $i -lt $Frames; $i++) {
    Save-Shot $hwnd ("anim-{0:D2}.png" -f $i)
}

Get-Process craftpull -ErrorAction SilentlyContinue | Stop-Process -Force
Write-Host "Готово: $outPath"
