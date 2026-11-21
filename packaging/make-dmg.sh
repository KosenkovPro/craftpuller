#!/usr/bin/env bash
# Собирает образ диска .dmg для macOS.
#
# Внутри образа — пакет приложения и ярлык на папку Applications: открыв
# образ, пользователь перетаскивает значок в «Программы», как в обычных
# программах для macOS.
#
# Использование:
#   packaging/make-dmg.sh dist/CraftPull.app dist/craftpull-macos-universal.dmg
set -euo pipefail

APP="${1:?нужен путь к пакету .app}"
DMG="${2:-dist/craftpull-macos-universal.dmg}"
VOL="${3:-CraftPull}"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"

# hdiutil на раннерах иногда отвечает «Resource busy» — повторяем.
for attempt in 1 2 3 4; do
    if hdiutil create -volname "$VOL" -srcfolder "$STAGE" -ov -format UDZO "$DMG"; then
        break
    fi
    echo "hdiutil не справился, попытка $attempt"
    rm -f "$DMG"
    sleep 15
done
test -f "$DMG"

echo "Образ диска готов: $DMG"
