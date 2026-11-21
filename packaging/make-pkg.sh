#!/usr/bin/env bash
# Собирает установщик .pkg для macOS — пошаговый, с приветствием и лицензией.
#
# Установщик кладёт CraftPull.app в папку /Applications.
#
# Использование:
#   packaging/make-pkg.sh dist/CraftPull.app dist/craftpull-macos-universal.pkg
set -euo pipefail

APP="${1:?нужен путь к пакету .app}"
PKG="${2:-dist/craftpull-macos-universal.pkg}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
MIN_MACOS="${MIN_MACOS:-11.0}"
ID="io.github.kosenkovpro.craftpull"
RES="packaging/installer"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# Компонентная часть: приложение уезжает в /Applications.
mkdir -p "$WORK/root"
cp -R "$APP" "$WORK/root/"
pkgbuild --root "$WORK/root" \
    --install-location /Applications \
    --identifier "$ID" \
    --version "$VERSION" \
    "$WORK/$ID.pkg"

# Описание установщика: окно приветствия, лицензия, выбор папки.
mkdir -p "$WORK/resources"
cp "$RES/welcome.html" "$RES/license.html" "$WORK/resources/"
sed -e "s/@VERSION@/$VERSION/g" -e "s/@MINMACOS@/$MIN_MACOS/g" \
    "$RES/distribution.xml" > "$WORK/distribution.xml"

productbuild --distribution "$WORK/distribution.xml" \
    --package-path "$WORK" \
    --resources "$WORK/resources" \
    "$PKG"

echo "Установщик готов: $PKG"
