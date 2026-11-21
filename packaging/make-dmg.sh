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
APP_NAME="$(basename "$APP")"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"

# Проверяем, что в образ действительно попал пакет приложения.
if [ ! -d "$STAGE/$APP_NAME" ]; then
    echo "Ошибка: в образ диска не попал пакет $APP_NAME" >&2
    exit 1
fi

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

# Пустой образ означает, что в staging ничего не было.
DMG_SIZE="$(stat -f%z "$DMG")"
if [ "$DMG_SIZE" -lt 1000000 ]; then
    echo "Ошибка: образ диска получился пустым ($DMG_SIZE байт)" >&2
    exit 1
fi

echo "Образ диска готов: $DMG ($DMG_SIZE байт)"
