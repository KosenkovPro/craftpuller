#!/usr/bin/env bash
# Portable-сборка CraftPull для Ubuntu / Linux.
# Собирает один бинарник + иконку + .desktop-файл и упаковывает в tar.gz.
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
OUT="dist"
NAME="craftpull-${VERSION}-linux-x86_64"
STAGE="${OUT}/${NAME}"

echo "==> Сборка release"
cargo build --release

echo "==> Подготовка пакета ${NAME}"
rm -rf "${STAGE}"
mkdir -p "${STAGE}"
cp target/release/craftpull "${STAGE}/craftpull"
chmod +x "${STAGE}/craftpull"

# Иконка 256x256 генерируется самой программой.
target/release/craftpull --export-icon "${STAGE}/craftpull.png"

cp packaging/craftpull.desktop "${STAGE}/craftpull.desktop"
cat > "${STAGE}/install.sh" <<'EOF'
#!/usr/bin/env bash
# Установка portable-версии: /usr/local/bin/craftpull + иконка + .desktop
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
mkdir -p "${PREFIX}/bin" "${PREFIX}/share/applications" "${PREFIX}/share/icons/hicolor/256x256/apps"
install -m 0755 "${HERE}/craftpull" "${PREFIX}/bin/craftpull"
install -m 0644 "${HERE}/craftpull.png" "${PREFIX}/share/icons/hicolor/256x256/apps/craftpull.png"
install -m 0644 "${HERE}/craftpull.desktop" "${PREFIX}/share/applications/craftpull.desktop"
echo "Установлено в ${PREFIX}. Запуск: ${PREFIX}/bin/craftpull"
EOF
chmod +x "${STAGE}/install.sh"

cp README.md "${STAGE}/README.md" 2>/dev/null || true

echo "==> Упаковка"
tar -czf "${OUT}/${NAME}.tar.gz" -C "${OUT}" "${NAME}"
echo "Готово: ${OUT}/${NAME}.tar.gz"
