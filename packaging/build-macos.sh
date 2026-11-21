#!/usr/bin/env bash
# Portable-сборка CraftPull для macOS (universal: Intel + Apple Silicon).
#
# Собирает .app-пакет с иконкой и упаковывает его в zip, tar.gz, образ диска
# .dmg и установщик .pkg. Отдельно делаются пакеты под каждую архитектуру.
# Сборку нужно запускать на macOS: графическая часть (egui/eframe) требует
# системных библиотек, кросс-компиляция с Linux или Windows невозможна.
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
OUT="dist"
NAME="craftpull-${VERSION}-macos-universal"
STAGE="${OUT}/${NAME}"
APP="${STAGE}/CraftPull.app"
MIN_MACOS="${MIN_MACOS:-11.0}"

# make_app собирает пакет .app из готового бинарника.
make_app() {
    local bin="$1" app="$2"
    rm -rf "$app"
    mkdir -p "${app}/Contents/MacOS" "${app}/Contents/Resources"
    cp "$bin" "${app}/Contents/MacOS/CraftPull"
    chmod +x "${app}/Contents/MacOS/CraftPull"

    # Иконка приложения (.icns собирается из PNG системной утилитой iconutil).
    if [ -f craftpull-icon.png ] && command -v iconutil >/dev/null 2>&1; then
        ICONSET="$(mktemp -d)/CraftPull.iconset"
        mkdir -p "$ICONSET"
        for size in 16 32 128 256 512; do
            sips -z "$size" "$size" craftpull-icon.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
            sips -z $((size * 2)) $((size * 2)) craftpull-icon.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
        done
        iconutil -c icns "$ICONSET" -o "${app}/Contents/Resources/CraftPull.icns"
        rm -rf "$(dirname "$ICONSET")"
    fi

    cat > "${app}/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>CraftPull</string>
  <key>CFBundleDisplayName</key><string>CraftPull</string>
  <key>CFBundleExecutable</key><string>CraftPull</string>
  <key>CFBundleIdentifier</key><string>io.github.kosenkovpro.craftpull</string>
  <key>CFBundleIconFile</key><string>CraftPull</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>${MIN_MACOS}</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>MIT / Apache-2.0</string>
</dict>
</plist>
PLIST
}

echo "==> CraftPull ${VERSION}"
rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null 2>&1 || true

echo "==> Сборка для Apple Silicon (aarch64-apple-darwin)"
cargo build --release --target aarch64-apple-darwin

echo "==> Сборка для Intel (x86_64-apple-darwin)"
cargo build --release --target x86_64-apple-darwin

echo "==> Universal-бинарник (lipo)"
mkdir -p target/universal-apple-darwin/release
lipo -create -output target/universal-apple-darwin/release/craftpull \
  target/aarch64-apple-darwin/release/craftpull \
  target/x86_64-apple-darwin/release/craftpull
lipo -info target/universal-apple-darwin/release/craftpull

echo "==> Подготовка пакета ${NAME}"
rm -rf "${STAGE}"
mkdir -p "${STAGE}"
make_app target/universal-apple-darwin/release/craftpull "${APP}"
cp README.md "${STAGE}/README.md" 2>/dev/null || true

echo "==> Пакеты под отдельные архитектуры"
make_app target/aarch64-apple-darwin/release/craftpull "${OUT}/craftpull-${VERSION}-macos-arm64/CraftPull.app"
make_app target/x86_64-apple-darwin/release/craftpull "${OUT}/craftpull-${VERSION}-macos-x64/CraftPull.app"

echo "==> Упаковка"
( cd "${OUT}" && zip -q -r -y "${NAME}.zip" "${NAME}" )
tar -czf "${OUT}/${NAME}.tar.gz" -C "${OUT}" "${NAME}"
( cd "${OUT}/craftpull-${VERSION}-macos-arm64" && zip -q -r -y "../craftpull-${VERSION}-macos-arm64.zip" "CraftPull.app" )
( cd "${OUT}/craftpull-${VERSION}-macos-x64" && zip -q -r -y "../craftpull-${VERSION}-macos-x64.zip" "CraftPull.app" )

echo "==> Образ диска"
chmod +x packaging/make-dmg.sh packaging/make-pkg.sh
packaging/make-dmg.sh "${APP}" "${OUT}/${NAME}.dmg"

echo "==> Установщик .pkg"
packaging/make-pkg.sh "${APP}" "${OUT}/${NAME}.pkg"

echo "Готово: ${OUT}/${NAME}.zip, ${OUT}/${NAME}.tar.gz, ${OUT}/${NAME}.dmg, ${OUT}/${NAME}.pkg"
