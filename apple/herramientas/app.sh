#!/usr/bin/env bash
# Monta `PixPin.app` (universal: Apple Silicon e Intel) a partir del paquete
# Swift de `apple/app`, firmada ad hoc, y la deja comprimida en
# `apple/app/dist/PixPin-mac.zip`. Para un Mac (o la CI de macOS), despues de
# `herramientas/xcframework.sh`:
#
#     apple/herramientas/app.sh
#
# Firma ad hoc: sin cuenta de Apple Developer. Al abrirla la primera vez,
# macOS avisa; se abre con clic derecho > Abrir (o
# `xattr -dr com.apple.quarantine PixPin.app`). Con la cuenta, aqui iran la
# firma Developer ID y el notarizado (plan, §7.1).
set -euo pipefail

APPLE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_SRC="$APPLE/app"
DIST="$APP_SRC/dist"
APP="$DIST/PixPin.app"
VERSION="${PIXPIN_VERSION:-0.1.0}"
COMPILACION="${PIXPIN_COMPILACION:-$(date +%Y%m%d%H%M)}"

cd "$APP_SRC"
echo "==> swift build (release, universal)"
swift build -c release --arch arm64 --arch x86_64 --product PixPin
BIN="$(swift build -c release --arch arm64 --arch x86_64 --show-bin-path)/PixPin"

rm -rf "$DIST"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/PixPin"
sed -e "s/@VERSION@/$VERSION/g" -e "s/@COMPILACION@/$COMPILACION/g" \
    "$APP_SRC/Recursos/Info.plist" > "$APP/Contents/Info.plist"

echo "==> Firma ad hoc"
codesign --force --sign - --options runtime --timestamp=none \
    --entitlements "$APP_SRC/Recursos/PixPin.entitlements" "$APP"
codesign --verify --strict --verbose=2 "$APP"

echo "==> Comprimir"
(cd "$DIST" && ditto -c -k --keepParent PixPin.app PixPin-mac.zip)
du -sh "$APP" "$DIST/PixPin-mac.zip"
echo "==> Listo: $DIST/PixPin-mac.zip"
