# Собирает баннер social preview 1280x640 для GitHub (.github/assets/banner.png).
# Использование: .\packaging\make-banner.ps1
$ErrorActionPreference = "Stop"
$projectRoot = Split-Path $PSScriptRoot -Parent
$Icon = Join-Path $projectRoot "craftpull-icon.png"
$Out  = Join-Path $projectRoot ".github\assets\banner.png"
Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Force -Path (Split-Path $Out) | Out-Null

$W = 1280; $H = 640
$bmp = New-Object System.Drawing.Bitmap $W, $H
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit
$g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic

function C([int]$r, [int]$gg, [int]$b, [int]$a = 255) {
    return [System.Drawing.Color]::FromArgb($a, $r, $gg, $b)
}

# --- Фон ---
$bg = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    (New-Object System.Drawing.Point 0, 0),
    (New-Object System.Drawing.Point 0, $H),
    (C 14 17 22), (C 26 33 46))
$g.FillRectangle($bg, 0, 0, $W, $H)
$bg.Dispose()

# --- Акцентное пятно слева под иконкой ---
$path = New-Object System.Drawing.Drawing2D.GraphicsPath
$path.AddEllipse(-160, 40, 620, 560)
$glow = New-Object System.Drawing.Drawing2D.PathGradientBrush($path)
$glow.CenterColor = C 76 141 255 80
$glow.SurroundColors = @((C 76 141 255 0))
$g.FillEllipse($glow, -160, 40, 620, 560)
$path.Dispose(); $glow.Dispose()

# --- Макет окна приложения справа ---
$winX = 700; $winY = 92; $winW = 512; $winH = 456
$brWin = New-Object System.Drawing.SolidBrush (C 20 26 36)
$penWin = New-Object System.Drawing.Pen ((C 46 58 78)), 1.5
$winRect = New-Object System.Drawing.Rectangle $winX, $winY, $winW, $winH
$g.FillRectangle($brWin, $winRect)
$g.DrawRectangle($penWin, $winRect)

# заголовок окна
$brBar = New-Object System.Drawing.SolidBrush (C 28 36 50)
$g.FillRectangle($brBar, $winX, $winY, $winW, 40)
$g.FillRectangle((New-Object System.Drawing.SolidBrush (C 46 58 78)), $winX, $winY + 39, $winW, 1)
$brBar.Dispose()
$dot = @((C 239 90 90), (C 250 197 61), (C 74 222 128))
for ($i = 0; $i -lt 3; $i++) {
    $d = New-Object System.Drawing.SolidBrush $dot[$i]
    $g.FillEllipse($d, $winX + 18 + ($i * 20), $winY + 14, 12, 12)
    $d.Dispose()
}

# левая колонка-карточки
$cardX = $winX + 22
for ($i = 0; $i -lt 3; $i++) {
    $cy = $winY + 62 + ($i * 118)
    $cr = New-Object System.Drawing.Rectangle $cardX, $cy, 116, 96
    $cb = New-Object System.Drawing.SolidBrush (C 27 34 47)
    $g.FillRectangle($cb, $cr)
    $g.DrawRectangle((New-Object System.Drawing.Pen ((C 38 47 62)), 1), $cr)
    $ib = New-Object System.Drawing.SolidBrush (C 76 141 255 70)
    $g.FillRectangle($ib, $cardX + 10, $cy + 10, 76, 50)
    $lb = New-Object System.Drawing.SolidBrush (C 120 132 150)
    $g.FillRectangle($lb, $cardX + 10, $cy + 70, 60, 5)
    $g.FillRectangle($lb, $cardX + 10, $cy + 81, 38, 4)
    $cb.Dispose(); $ib.Dispose(); $lb.Dispose()
}

# правый блок: «загрузки» с полосами прогресса
$px = $winX + 168; $pw = 316
$rows = @(@(0.92, (C 74 222 128)), @(0.61, (C 76 141 255)), @(0.34, (C 192 132 252)))
for ($i = 0; $i -lt 3; $i++) {
    $ry = $winY + 70 + ($i * 118)
    $lb = New-Object System.Drawing.SolidBrush (C 140 152 172)
    $g.FillRectangle($lb, $px, $ry, 150, 7)
    $track = New-Object System.Drawing.SolidBrush (C 38 47 62)
    $g.FillRectangle($track, $px, $ry + 24, $pw, 8)
    $fill = New-Object System.Drawing.SolidBrush $rows[$i][1]
    $g.FillRectangle($fill, $px, $ry + 24, ([int]($pw * $rows[$i][0])), 8)
    $lb.Dispose(); $track.Dispose(); $fill.Dispose()
}
$brWin.Dispose(); $penWin.Dispose()

# --- Иконка приложения ---
$iconImg = [System.Drawing.Image]::FromFile($Icon)
$g.DrawImage($iconImg, (New-Object System.Drawing.Rectangle 92, 74, 208, 208))
$iconImg.Dispose()

# --- Акцентная линия ---
$accent = New-Object System.Drawing.SolidBrush (C 76 141 255)
$g.FillRectangle($accent, 92, 296, 8, 196)
$accent.Dispose()

# --- Заголовок ---
$fontTitle = New-Object System.Drawing.Font "Segoe UI", 76, ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
$brTitle = New-Object System.Drawing.SolidBrush (C 230 237 243)
$g.DrawString("CraftPull", $fontTitle, $brTitle, 118, 318)
$fontTitle.Dispose(); $brTitle.Dispose()

# --- Подзаголовок ---
$fontSub = New-Object System.Drawing.Font "Segoe UI", 27, ([System.Drawing.FontStyle]::Regular), ([System.Drawing.GraphicsUnit]::Pixel)
$brSub = New-Object System.Drawing.SolidBrush (C 190 200 214)
$g.DrawString("Release & source manager for GitHub", $fontSub, $brSub, 120, 418)
$fontSub.Dispose(); $brSub.Dispose()

# --- Подпись снизу ---
$fontFoot = New-Object System.Drawing.Font "Segoe UI", 21, ([System.Drawing.FontStyle]::Regular), ([System.Drawing.GraphicsUnit]::Pixel)
$brFoot = New-Object System.Drawing.SolidBrush (C 138 150 168)
$g.DrawString("Rust + egui   ·   Windows & Ubuntu", $fontFoot, $brFoot, 120, 512)
$fontFoot.Dispose(); $brFoot.Dispose()

# --- Плашки под иконкой ---
$chips = @("Parallel & sequential", "SHA-256", "EN / RU")
$fontChip = New-Object System.Drawing.Font "Segoe UI", 19, ([System.Drawing.FontStyle]::Regular), ([System.Drawing.GraphicsUnit]::Pixel)
$brChipText = New-Object System.Drawing.SolidBrush (C 205 220 235)
$brChipBg = New-Object System.Drawing.SolidBrush (C 33 42 57)
$brChipLine = New-Object System.Drawing.Pen ((C 58 72 94), 1)
$x = 118.0
foreach ($c in $chips) {
    $sz = $g.MeasureString($c, $fontChip)
    $w = $sz.Width + 34; $h = $sz.Height + 20
    $g.FillRectangle($brChipBg, [float]$x, [float]556, [float]$w, [float]$h)
    $g.DrawRectangle($brChipLine, [float]$x, [float]556, [float]$w, [float]$h)
    $g.DrawString($c, $fontChip, $brChipText, [float]($x + 17), [float]562)
    $x += $w + 12
}
$fontChip.Dispose(); $brChipText.Dispose(); $brChipBg.Dispose(); $brChipLine.Dispose()

$g.Dispose()
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Host "баннер -> $Out"

