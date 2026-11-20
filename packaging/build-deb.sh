#!/usr/bin/env bash
# Ubuntu / Debian-пакет CraftPull (.deb).
#
# Кладёт бинарник в /usr/bin, иконку — в /usr/share/icons, .desktop-запись —
# в /usr/share/applications. Ставится через: sudo apt install ./craftpull-*.deb
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
OUT="dist"
NAME="craftpull-${VERSION}-linux-x86_64"
STAGE="${OUT}/deb/${NAME}"
DEB="${OUT}/${NAME}.deb"

echo "==> Сборка release"
cargo build --release

echo "==> Подготовка пакета ${NAME}.deb"
rm -rf "${STAGE}"
mkdir -p "${STAGE}/DEBIAN" \
         "${STAGE}/usr/bin" \
         "${STAGE}/usr/share/applications" \
         "${STAGE}/usr/share/icons/hicolor/256x256/apps" \
         "${STAGE}/usr/share/doc/craftpull"

install -m 0755 target/release/craftpull "${STAGE}/usr/bin/craftpull"
install -m 0644 packaging/craftpull.desktop "${STAGE}/usr/share/applications/craftpull.desktop"
# Иконка 256x256 генерируется самой программой.
target/release/craftpull --export-icon "${STAGE}/usr/share/icons/hicolor/256x256/apps/craftpull.png"
install -m 0644 README.md "${STAGE}/usr/share/doc/craftpull/README.md"

cat > "${STAGE}/DEBIAN/control" <<EOF
Package: craftpull
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: amd64
Maintainer: Kosenkov Ivan <support@chekak.store>
Homepage: https://chekak.store
Description: Release and source manager for GitHub repositories
 CraftPull watches GitHub repositories for new releases, downloads the release
 assets built for the current operating system, verifies their SHA-256 checksums
 and removes the outdated versions.
EOF

echo "==> Упаковка"
dpkg-deb --build --root-owner-group "${STAGE}" "${DEB}"
echo "Готово: ${DEB}"
