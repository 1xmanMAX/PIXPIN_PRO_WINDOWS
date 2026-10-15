# PixPin para Mac, iPad e iPhone

Zona de trabajo de la versión de Apple. Plan completo y decisiones:
[`docs/investigacion/2026-10-15-mac-ipad-iphone.md`](../docs/investigacion/2026-10-15-mac-ipad-iphone.md).

Todo lo de aquí es una **copia tal cual** (copiar y pegar, sin reescribir) de `crates/` y `apps/`
del 15-oct-2026. Se irá arreglando y uniendo poco a poco. La versión de Windows no se toca desde
aquí: sigue viviendo en `crates/` y `apps/`.

## Las tres zonas

| Carpeta | Qué hay | Qué se hace con ello |
|---|---|---|
| [`nucleo/`](nucleo) | Los 16 crates que **no dependen de Windows**: formato de proyectos, sincronizar, lienzo, Word/Excel/PowerPoint/EPUB, timeline, lecciones, la lógica de la interfaz… | **Se usa tal cual.** Es un espacio de trabajo de Rust propio (`apple/Cargo.toml`) que compila y pasa sus pruebas fuera de Windows. Será `PixPinCore.xcframework` para Swift (fase 1). |
| [`por-adaptar/`](por-adaptar) | Crates **casi** puros, con una parte de Windows: `pixpin-cad` (lector DWG/DXF + 3D en D3D11), `pixpin-bim`, `pixpin-codec` (WIC), `pixpin-pdf` (ver con Windows.Data.Pdf; escribir es propio), `pixpin-ia` (WinHTTP, Job Objects, DPAPI), `pixpin-voz` (SAPI, ONNX de System32), `pixpin-pila`, `pixpin-lanzador`, `pixpin-aligerar` | **Se queda en Rust**: separar la parte pura con `#[cfg(windows)]` o quitándola, y pasar cada crate a `nucleo/` cuando compile sin Windows. No compila todavía (fuera del espacio de trabajo). |
| [`por-traducir/`](por-traducir) | Lo atado a Windows: `pixpin-shell` (Win32), `pixpin-render` (Direct2D), `pixpin-capture`, `pixpin-record`, `pixpin-ocr`, `pixpin-audio`, `pixpin-web` (WebView2), `pixpin-pin`, `pixpin-notas` (RichEdit) y la app `pixpin` | **Referencia para escribir la versión en Swift** con las herramientas de Apple (ScreenCaptureKit, Vision, PDFKit, AVFoundation, `NSPanel`…). No compila ni se compilará: se lee y se traduce pieza a pieza. |

Qué herramienta de Apple sustituye a cada pieza: §3 del plan.

Y además, el puente con Swift:

| Carpeta | Qué hay |
|---|---|
| [`nucleo/pixpin-nucleo`](nucleo/pixpin-nucleo) | La API para Swift con **UniFFI**: abrir una carpeta de datos (`Nucleo`), chats y mensajes **por páginas**, escribir una nota, el lienzo como **un solo búfer plano** de `f32` (formato en `src/lienzo.rs`) y sincronizar por TCP (unirse a un grupo, una vuelta, escuchar). |
| [`PixPinCore/`](PixPinCore) | El paquete Swift (`Package.swift`, macOS 15 / iOS 18): los enlaces generados y el `PixPinCoreFFI.xcframework` (no se versiona). |
| [`herramientas/xcframework.sh`](herramientas/xcframework.sh) | En un Mac: compila las cuatro plataformas, genera los enlaces y hace el xcframework. |

## Compilar y probar el núcleo

```sh
cd apple
cargo test --workspace -- --test-threads=1
```

El puente para Swift:

```sh
apple/herramientas/xcframework.sh   # en un Mac con Xcode: deja PixPinCore listo para Xcode
# en Linux, solo los enlaces (para revisar la API):
cargo build -p pixpin-nucleo --lib
cargo run -p pixpin-nucleo --features bindgen --bin uniffi-bindgen -- \
    generate --library target/debug/libpixpin_nucleo.so --language swift --out-dir target/enlaces
```

El xcframework se compila con el perfil `ffi` (el `release` con `panic = "unwind"`): así un
pánico de Rust llega a Swift como error en vez de cerrar la app.

`apple/rust-toolchain.toml` fija el mismo Rust que Windows con los objetivos de Apple
(`aarch64-apple-darwin`, `x86_64-apple-darwin`, `aarch64-apple-ios`, `aarch64-apple-ios-sim`).

## Reglas para no perderse

1. **El formato y la sincronización tienen que seguir siendo idénticos a los de Windows y
   Android.** Si se arregla algo de `pixpin-sincro` o `pixpin-proyecto` aquí, hay que llevarlo
   también a `crates/` (y al revés). Una vez el núcleo funcione en Mac, la idea es que
   `crates/` y `apple/nucleo/` vuelvan a ser **un solo** código (el de Windows apuntando aquí o
   al revés), para que no se separen.
2. Cuando un crate de `por-adaptar/` compile sin Windows, se mueve a `nucleo/` y se añade a la
   lista de la CI.
3. Cuando una pieza de `por-traducir/` ya esté en Swift, se borra su copia de aquí.

## Lo que falta (orden del plan)

- [ ] Fase 0.3: arreglos de sincronizar (tildes NFC, IPv6, versión del protocolo).
- [ ] Fase 1: el puente Rust ↔ Swift.
  - [x] Crate `pixpin-nucleo` con UniFFI: carpeta de datos, chats, mensajes por páginas
        (índice de líneas en caché con tope), escribir una nota, lienzo en un búfer plano,
        unirse a un grupo / sincronizar / escuchar por TCP, errores como `enum`.
  - [x] Enlaces de Swift generados + `PixPinCore/Package.swift` + `herramientas/xcframework.sh`.
  - [ ] Ejecutar `xcframework.sh` en un Mac y una app de prueba que lista los proyectos de una
        carpeta copiada de Windows y sincroniza un chat con un PC Windows real.
  - [ ] Lector del búfer del lienzo en Swift (`CGPath` en una pasada).
  - [ ] Transporte de Swift sobre `NWConnection` (hoy socket BSD de Rust) y descubrimiento con
        Bonjour (`NWBrowser`); el `suelto`, la galería y los chats con personas.
  - [ ] Documentos (`docs`, `hojas`, `office`), timeline y lecciones en la API.
  - [ ] CI: `macos-latest` compila el xcframework.
- [ ] Fase 2: app de Mac en SwiftUI/AppKit (`apple/app/`).
- [ ] `por-adaptar/` → `nucleo/`: `pixpin-codec` (decodificar con `image`; ImageIO desde Swift),
      `pixpin-cad` y `pixpin-bim` (el lector; el 3D a Metal), `pixpin-pdf` (escribir y unir;
      ver con PDFKit), `pixpin-ia` (HTTP portable; Llavero en vez de DPAPI).
