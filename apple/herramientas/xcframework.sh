#!/usr/bin/env bash
# Compila el nucleo de Rust (`nucleo/pixpin-nucleo`) como
# `PixPinCore/PixPinCoreFFI.xcframework` y regenera los enlaces de Swift
# (`PixPinCore/Sources/PixPinCore/pixpin_nucleo.swift`). Para un Mac con
# Xcode; se ejecuta desde cualquier sitio:
#
#     apple/herramientas/xcframework.sh
#
# Plataformas (las de `apple/rust-toolchain.toml`):
#   - macOS:            aarch64-apple-darwin + x86_64-apple-darwin, juntas con lipo
#   - iPhone/iPad:      aarch64-apple-ios
#   - simulador de iOS: aarch64-apple-ios-sim (solo Apple Silicon)
#
# Perfil `ffi` (apple/Cargo.toml): el `release` de siempre (LTO fat, una
# unidad de codigo, opt-level 3) pero con `panic = "unwind"`. UniFFI envuelve
# cada llamada en `catch_unwind`, asi que un panico de Rust llega a Swift
# como error en vez de cerrar la app; con el `panic = "abort"` del release de
# Windows, cualquier panico mataria la app de golpe y sin informe. Cuesta
# las tablas de desenrollado (algo mas de tamano), que en una app que no se
# puede caer compensa.
#
# `strip = true` no toca una biblioteca estatica (no hay enlazado); la que
# se adelgaza es la app final, al enlazarla Xcode.
set -euo pipefail

APPLE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$APPLE"

# `PIXPIN_PERFIL` y `PIXPIN_OBJETIVOS` cambian lo de siempre para la CI:
# alli se compila solo el Mac y con el perfil `ffi-rapido` (LTO fino, en
# paralelo), que tarda una fraccion y sirve igual para probar. Lo que se
# publica sale con los valores de fabrica.
PERFIL="${PIXPIN_PERFIL:-ffi}"
CRATE=pixpin-nucleo
LIB=libpixpin_nucleo.a
MODULO=pixpin_nucleoFFI
SALIDA="$APPLE/target/xcframework"
PAQUETE="$APPLE/PixPinCore"
XCF="$PAQUETE/PixPinCoreFFI.xcframework"

# Los minimos de la app (los mismos que Package.swift): sin esto, rustc
# compila para el minimo de su objetivo y el enlazado avisa en cada objeto.
export MACOSX_DEPLOYMENT_TARGET=15.0
export IPHONEOS_DEPLOYMENT_TARGET=18.0

read -r -a OBJETIVOS <<< "${PIXPIN_OBJETIVOS:-aarch64-apple-darwin x86_64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim}"
tiene() { [[ " ${OBJETIVOS[*]} " == *" $1 "* ]]; }

echo "==> Objetivos de Rust"
rustup target add "${OBJETIVOS[@]}" >/dev/null

for t in "${OBJETIVOS[@]}"; do
    echo "==> cargo build $t ($PERFIL)"
    cargo build -p "$CRATE" --lib --profile "$PERFIL" --target "$t"
done

rm -rf "$SALIDA"
mkdir -p "$SALIDA/macos" "$SALIDA/ios" "$SALIDA/ios-sim"

echo "==> lipo: macOS universal"
lipo -create \
    "target/aarch64-apple-darwin/$PERFIL/$LIB" \
    "target/x86_64-apple-darwin/$PERFIL/$LIB" \
    -output "$SALIDA/macos/$LIB"
if tiene aarch64-apple-ios; then cp "target/aarch64-apple-ios/$PERFIL/$LIB" "$SALIDA/ios/$LIB"; fi
if tiene aarch64-apple-ios-sim; then cp "target/aarch64-apple-ios-sim/$PERFIL/$LIB" "$SALIDA/ios-sim/$LIB"; fi

echo "==> Enlaces de Swift (uniffi-bindgen)"
# Se leen de una biblioteca dinamica SIN quitar simbolos (la de depuracion
# de esta maquina): los metadatos de UniFFI viajan como simbolos, y el
# perfil de release los quitaria.
cargo build -p "$CRATE" --lib
ENLACES="$SALIDA/enlaces"
cargo run -q -p "$CRATE" --features bindgen --bin uniffi-bindgen -- \
    generate --library "target/debug/libpixpin_nucleo.dylib" \
    --language swift --out-dir "$ENLACES"

# La cabecera y su module.modulemap en una carpeta con el nombre del modulo:
# dos xcframeworks con un module.modulemap suelto en `Headers/` chocan al
# copiarse al mismo sitio en la compilacion.
CABECERAS="$SALIDA/Headers"
mkdir -p "$CABECERAS/$MODULO"
cp "$ENLACES/$MODULO.h" "$CABECERAS/$MODULO/"
cp "$ENLACES/$MODULO.modulemap" "$CABECERAS/$MODULO/module.modulemap"

mkdir -p "$PAQUETE/Sources/PixPinCore"
cp "$ENLACES/pixpin_nucleo.swift" "$PAQUETE/Sources/PixPinCore/pixpin_nucleo.swift"

echo "==> xcodebuild -create-xcframework"
rm -rf "$XCF"
BIBLIOTECAS=(-library "$SALIDA/macos/$LIB" -headers "$CABECERAS")
if tiene aarch64-apple-ios; then BIBLIOTECAS+=(-library "$SALIDA/ios/$LIB" -headers "$CABECERAS"); fi
if tiene aarch64-apple-ios-sim; then BIBLIOTECAS+=(-library "$SALIDA/ios-sim/$LIB" -headers "$CABECERAS"); fi
xcodebuild -create-xcframework "${BIBLIOTECAS[@]}" -output "$XCF"

echo "==> Listo: $XCF"
du -sh "$XCF"/*/ 2>/dev/null || true
