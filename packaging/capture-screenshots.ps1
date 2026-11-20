# Снимает скриншоты интерфейса CraftPull в docs/ для README.
# Использование: powershell -ExecutionPolicy Bypass -File packaging/capture-screenshots.ps1
#
# Первый запуск обновляет данные (7 запросов к GitHub) и наполняет кэш,
# остальные кадры снимаются с флагом --no-refresh — без обращения к сети.

param(
    [string]$OutDir = "docs"
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path $PSScriptRoot -Parent
$outPath = Join-Path $projectRoot $OutDir
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
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
"@

function Get-AppWindow([int]$procId) {
    # У процесса несколько окон (главное и служебное от трея) — берём самое большое.
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
                if ($area -gt $script:bestArea) {
                    $script:bestArea = $area
                    $script:best = $hh
                }
            }
        }
        return $true
    }
    [void][Shot]::EnumWindows($cb, [IntPtr]::Zero)
    return $script:best
}

function Restore-Window([IntPtr]$hwnd) {
    [void][Shot]::ShowWindow($hwnd, 9)     # SW_RESTORE
    [void][Shot]::BringWindowToTop($hwnd)
    [void][Shot]::SetForegroundWindow($hwnd)
    Start-Sleep -Milliseconds 600
}

function Start-App([string[]]$flags) {
    Get-Process craftpull -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 800
    $proc = Start-Process -FilePath $exe -ArgumentList $flags -PassThru
    # Ждём, пока окно появится и отрисовается.
    for ($i = 0; $i -lt 40; $i++) {
        Start-Sleep -Milliseconds 500
        $hwnd = Get-AppWindow $proc.Id
        if ($hwnd -ne [IntPtr]::Zero -and $script:bestArea -gt 40000) {
            Restore-Window $hwnd
            return @{ Proc = $proc; Hwnd = $hwnd }
        }
    }
    throw "Окно CraftPull не появилось"
}

function Save-Shot([IntPtr]$hwnd, [string]$name) {
    Restore-Window $hwnd
    $r = New-Object Shot+RECT
    [void][Shot]::GetWindowRect($hwnd, [ref]$r)
    $w = $r.R - $r.L
    $h = $r.B - $r.T
    if ($w -lt 200 -or $h -lt 200) { throw "Окно свёрнуто ($w x $h)" }
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    $ok = [Shot]::PrintWindow($hwnd, $hdc, 2)
    $g.ReleaseHdc($hdc)
    $g.Dispose()
    $bmp.Save((Join-Path $outPath $name), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Host ("  -> {0} ({1}x{2}, PrintWindow={3})" -f $name, $w, $h, $ok)
}

Write-Host "Снимаю интерфейс CraftPull..."

# 1. Обновление данных с GitHub: заодно наполняется кэш релизов.
#    Этот кадр показывает журнал с реальными записями проверки.
$app = Start-App @("--view", "list", "--tab", "logs", "--refresh", "--no-icons")
Start-Sleep -Seconds 30
Save-Shot $app.Hwnd "screenshot-log.png"
Get-Process craftpull -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 800

# 2. Остальные кадры — из кэша, без обращения к сети.
#    --no-icons: чужие логотипы приложений не попадают в документацию.
$app = Start-App @("--view", "list", "--no-refresh", "--no-icons")
Start-Sleep -Seconds 4
Save-Shot $app.Hwnd "screenshot-list.png"

$app = Start-App @("--view", "grid", "--no-refresh", "--no-icons")
Start-Sleep -Seconds 4
Save-Shot $app.Hwnd "screenshot-grid.png"

# 3. Карточка-герой: клик по плитке разворачивает её на всю ширину.
$app = Start-App @("--view", "grid", "--hero", "storytold/pdfcraft", "--no-refresh", "--no-icons")
Start-Sleep -Seconds 4
Save-Shot $app.Hwnd "screenshot-hero.png"

$app = Start-App @("--view", "list", "--tab", "settings", "--no-refresh", "--no-icons")
Start-Sleep -Seconds 4
Save-Shot $app.Hwnd "screenshot-settings.png"

# 4. Релизы: панель справа внутри страницы приложения.
$app = Start-App @("--view", "list", "--popup", "storytold/pdfcraft", "--no-refresh", "--no-icons")
Start-Sleep -Seconds 6
Save-Shot $app.Hwnd "screenshot-releases.png"

Get-Process craftpull -ErrorAction SilentlyContinue | Stop-Process -Force
Write-Host "Готово: $outPath"
