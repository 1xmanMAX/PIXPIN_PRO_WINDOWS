#!/usr/bin/env bash
# Monta `PixPin.app` (universal: Apple Silicon e Intel), firmada ad hoc, y la
# deja comprimida. Para un Mac (o la CI de macOS), despues de
# `herramientas/xcframework.sh`:
#
#     apple/herramientas/app.sh          # con Xcode si hay xcodegen; si no, SwiftPM
#     apple/herramientas/app.sh xcode    # la app CON sus extensiones
#     apple/herramientas/app.sh spm      # solo la app, con SwiftPM
#
# Dos caminos:
#
# - **xcode** (`apple/xcode/project.yml`, con XcodeGen: `brew install
#   xcodegen`): la app con las App Intents (Siri, Atajos, Spotlight), la
#   extension de Compartir y la vista previa de Quick Look. SwiftPM no sabe
#   hacer extensiones de app. Se compila sin firmar y se firma aqui, de
#   dentro afuera, cada extension con su caja de arena. Deja
#   `apple/app/dist-xcode/PixPin-mac.zip`.
# - **spm** (`swift build`): solo la app, como siempre. Deja
#   `apple/app/dist/PixPin-mac.zip`. Es el respaldo si no hay XcodeGen.
#
# Firma ad hoc: sin cuenta de Apple Developer. Al abrirla la primera vez,
# macOS avisa; se abre con clic derecho > Abrir (o
# `xattr -dr com.apple.quarantine PixPin.app`). Con la cuenta, aqui iran la
# firma Developer ID y el notarizado (plan, §7.1).
set -euo pipefail

APPLE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_SRC="$APPLE/app"
XC="$APPLE/xcode"
VERSION="${PIXPIN_VERSION:-0.1.0}"
COMPILACION="${PIXPIN_COMPILACION:-$(date +%Y%m%d%H%M)}"

MODO="${1:-auto}"
if [ "$MODO" = auto ]; then
    if command -v xcodegen >/dev/null 2>&1 && command -v xcodebuild >/dev/null 2>&1; then
        MODO=xcode
    else
        echo "==> Sin xcodegen: la app sin extensiones (SwiftPM)"
        MODO=spm
    fi
fi

comprimir() {
    local dist="$1"
    echo "==> Comprimir"
    (cd "$dist" && ditto -c -k --keepParent PixPin.app PixPin-mac.zip)
    du -sh "$dist/PixPin.app" "$dist/PixPin-mac.zip"
    echo "==> Listo: $dist/PixPin-mac.zip"
}

# Sin simbolos de depuracion (el nucleo de Rust llega como biblioteca estatica
# y sus simbolos acaban en el ejecutable): la app pesa mucho menos y carga
# antes. `-x` deja los simbolos globales, que la firma y dyld necesitan.
adelgazar() {
    local bin="$1" antes despues
    antes=$(stat -L -f %z "$bin")
    strip -x "$bin"
    despues=$(stat -L -f %z "$bin")
    echo "    $(basename "$bin"): $((antes / 1048576)) MB -> $((despues / 1048576)) MB sin simbolos"
}

# Las bibliotecas y frameworks de Contents/Frameworks, firmados antes que
# quien los carga.
firmar_frameworks() {
    local dir="$1/Contents/Frameworks" f
    [ -d "$dir" ] || return 0
    for f in "$dir"/*; do
        [ -e "$f" ] || continue
        codesign --force --sign - --options runtime --timestamp=none "$f"
    done
}

# La app de verdad arranca (dyld encuentra el nucleo, nada se cae al abrir):
# unos segundos y se cierra. Si sale antes con error, se ve por que.
arranca() {
    local exe="$1/Contents/MacOS/PixPin" log
    log="$(mktemp)"
    "$exe" >"$log" 2>&1 &
    local pid=$!
    sleep 6
    if kill -0 "$pid" 2>/dev/null; then
        # Que se vea una ventana (la de Proyectos) y no solo el icono de la
        # barra de menus: ventanas normales (capa 0) de ese proceso.
        local cuenta="$APPLE/app/.build/ventanas.swift"
        mkdir -p "$(dirname "$cuenta")"
        cat >"$cuenta" <<'SWIFT'
import CoreGraphics
let pid = Int32(CommandLine.arguments[1])!
let lista = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
let suyas = lista.filter { ($0[kCGWindowOwnerPID as String] as? Int32) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }
print(suyas.count)
SWIFT
        local ventanas
        ventanas="$(swift "$cuenta" "$pid" 2>/dev/null || echo "?")"
        if [ "$ventanas" = "0" ]; then
            echo "::warning::PixPin arranco sin ninguna ventana a la vista"
        fi
        echo "    ventanas a la vista: $ventanas"
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
        echo "    arranca: sigue viva a los 6 s"
    else
        local codigo=0
        wait "$pid" || codigo=$?
        if [ "$codigo" -ne 0 ]; then
            echo "error: PixPin se cerro al arrancar (codigo $codigo)" >&2
            cat "$log" >&2
            exit 1
        fi
        echo "    arranca: termino sola sin error"
    fi
    rm -f "$log"
}

universal() {
    local bin="$1"
    local archs
    archs="$(lipo -archs "$bin")"
    echo "    $(basename "$bin"): $archs"
    local a
    for a in arm64 x86_64; do
        case " $archs " in
            *" $a "*) ;;
            *) echo "error: $bin no es universal (le falta $a: $archs)" >&2; exit 1 ;;
        esac
    done
}

construir_spm() {
    local dist="$APP_SRC/dist"
    local app="$dist/PixPin.app"
    cd "$APP_SRC"
    echo "==> swift build (release, universal)"
    swift build -c release --arch arm64 --arch x86_64 --product PixPin
    local bin
    bin="$(swift build -c release --arch arm64 --arch x86_64 --show-bin-path)/PixPin"

    rm -rf "$dist"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp "$bin" "$app/Contents/MacOS/PixPin"
    adelgazar "$app/Contents/MacOS/PixPin"
    # El nucleo es una biblioteca dinamica (PixPinCore/Package.swift): va en
    # Contents/Frameworks y el ejecutable la busca alli.
    mkdir -p "$app/Contents/Frameworks"
    local lib="$(dirname "$bin")/libPixPinCore.dylib"
    if [ -f "$lib" ]; then
        cp "$lib" "$app/Contents/Frameworks/"
        adelgazar "$app/Contents/Frameworks/libPixPinCore.dylib"
        if ! otool -l "$app/Contents/MacOS/PixPin" | grep -q "@executable_path/../Frameworks"; then
            install_name_tool -add_rpath @executable_path/../Frameworks "$app/Contents/MacOS/PixPin"
        fi
    fi
    sed -e "s/@VERSION@/$VERSION/g" -e "s/@COMPILACION@/$COMPILACION/g" \
        "$APP_SRC/Recursos/Info.plist" > "$app/Contents/Info.plist"

    echo "==> Firma ad hoc"
    firmar_frameworks "$app"
    codesign --force --sign - --options runtime --timestamp=none \
        --entitlements "$APP_SRC/Recursos/PixPin.entitlements" "$app"
    codesign --verify --strict --deep --verbose=2 "$app"
    arranca "$app"
    comprimir "$dist"
}

construir_xcode() {
    local dist="$APP_SRC/dist-xcode"
    local app="$dist/PixPin.app"
    local derivados="$XC/build"

    echo "==> xcodegen (apple/xcode/project.yml)"
    (cd "$XC" && xcodegen generate --spec project.yml)

    echo "==> xcodebuild (release, universal, sin firmar)"
    xcodebuild -project "$XC/PixPin.xcodeproj" -scheme AppPixPin \
        -configuration Release -destination 'generic/platform=macOS' \
        -derivedDataPath "$derivados" -quiet \
        ARCHS="arm64 x86_64" ONLY_ACTIVE_ARCH=NO \
        CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY="" \
        MARKETING_VERSION="$VERSION" CURRENT_PROJECT_VERSION="$COMPILACION" \
        build

    rm -rf "$dist"
    mkdir -p "$dist"
    ditto "$derivados/Build/Products/Release/PixPin.app" "$app"

    echo "==> Lo que lleva dentro"
    universal "$app/Contents/MacOS/PixPin"
    local ext
    for ext in CompartirPixPin VistaPreviaPixPin; do
        if [ ! -d "$app/Contents/PlugIns/$ext.appex" ]; then
            echo "error: falta la extension $ext.appex" >&2
            exit 1
        fi
        universal "$app/Contents/PlugIns/$ext.appex/Contents/MacOS/$ext"
        adelgazar "$app/Contents/PlugIns/$ext.appex/Contents/MacOS/$ext"
        # Un solo nucleo: si Xcode copio el framework tambien dentro de la
        # extension, sobra (la extension lo busca en la app, por su rpath).
        local fx="$app/Contents/PlugIns/$ext.appex/Contents/Frameworks"
        if [ -d "$fx" ]; then
            local f
            for f in "$fx"/*; do
                if [ -e "$app/Contents/Frameworks/$(basename "$f")" ]; then
                    echo "    $ext: fuera la copia repetida de $(basename "$f")"
                    rm -rf "$f"
                fi
            done
            rmdir "$fx" 2>/dev/null || true
        fi
    done
    adelgazar "$app/Contents/MacOS/PixPin"
    if [ -d "$app/Contents/Frameworks" ]; then
        local fw
        for fw in "$app/Contents/Frameworks"/*.framework; do
            [ -d "$fw" ] || continue
            local n; n="$(basename "$fw" .framework)"
            universal "$fw/$n"
            adelgazar "$fw/$n"
        done
        ls "$app/Contents/Frameworks"
    fi
    if [ -d "$app/Contents/Resources/Metadata.appintents" ]; then
        echo "    App Intents: Metadata.appintents"
    else
        echo "::warning::La app no lleva Metadata.appintents: Siri y Atajos no veran sus acciones"
    fi

    echo "==> Firma ad hoc (de dentro afuera)"
    firmar_frameworks "$app"
    codesign --force --sign - --options runtime --timestamp=none \
        --entitlements "$XC/Soporte/Compartir.entitlements" "$app/Contents/PlugIns/CompartirPixPin.appex"
    codesign --force --sign - --options runtime --timestamp=none \
        --entitlements "$XC/Soporte/VistaPrevia.entitlements" "$app/Contents/PlugIns/VistaPreviaPixPin.appex"
    codesign --force --sign - --options runtime --timestamp=none \
        --entitlements "$APP_SRC/Recursos/PixPin.entitlements" "$app"
    codesign --verify --strict --deep --verbose=2 "$app"
    arranca "$app"
    comprimir "$dist"
}

case "$MODO" in
    spm) construir_spm ;;
    xcode) construir_xcode ;;
    *) echo "uso: $0 [auto|xcode|spm]" >&2; exit 2 ;;
esac
