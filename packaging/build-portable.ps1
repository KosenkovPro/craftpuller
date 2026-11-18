# Portable-сборка CraftPull для Windows.
# Собирает release-бинарник, иконку и упаковывает в zip без установщика.
param(
    [string]$OutDir = "dist"
)

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$version = (Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
$name = "craftpull-$version-windows-x64"
$stage = Join-Path $OutDir $name

Write-Host "==> Сборка release"
cargo build --release

Write-Host "==> Подготовка пакета $name"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $stage | Out-Null

Copy-Item "target\release\craftpull.exe" (Join-Path $stage "craftpull.exe")
Copy-Item "README.md" (Join-Path $stage "README.md") -ErrorAction SilentlyContinue

# Иконка приложения: ICO для ярлыка + PNG для документации.
& ".\target\release\craftpull.exe" --export-icon (Join-Path $stage "craftpull.png")

Write-Host "==> Упаковка"
$zip = Join-Path $OutDir "$name.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip -Force
Write-Host "Готово: $zip"
