# PixPin para Mac, iPad e iPhone

Zona de trabajo de la versión de Apple. Plan completo y decisiones:
[`docs/investigacion/2026-10-15-mac-ipad-iphone.md`](../docs/investigacion/2026-10-15-mac-ipad-iphone.md).

Todo lo de aquí es una **copia tal cual** (copiar y pegar, sin reescribir) de `crates/` y `apps/`
del 15-oct-2026. Se irá arreglando y uniendo poco a poco. La versión de Windows no se toca desde
aquí: sigue viviendo en `crates/` y `apps/`.

## Las tres zonas

| Carpeta | Qué hay | Qué se hace con ello |
|---|---|---|
| [`nucleo/`](nucleo) | Los 17 crates que **no dependen de Windows**: formato de proyectos, sincronizar, lienzo, Word/Excel/PowerPoint/EPUB, timeline, lecciones, la lógica de la interfaz, el asistente (`pixpin-ia`)… | **Se usa tal cual.** Es un espacio de trabajo de Rust propio (`apple/Cargo.toml`) que compila y pasa sus pruebas fuera de Windows. Será `PixPinCore.xcframework` para Swift (fase 1). |
| [`por-adaptar/`](por-adaptar) | Crates **casi** puros, con una parte de Windows: `pixpin-cad` (lector DWG/DXF + 3D en D3D11), `pixpin-bim`, `pixpin-codec` (WIC), `pixpin-pdf` (ver con Windows.Data.Pdf; escribir es propio), `pixpin-voz` (SAPI, ONNX de System32), `pixpin-pila`, `pixpin-lanzador`, `pixpin-aligerar` | **Se queda en Rust**: separar la parte pura con `#[cfg(windows)]` o quitándola, y pasar cada crate a `nucleo/` cuando compile sin Windows. No compila todavía (fuera del espacio de trabajo). |
| [`por-traducir/`](por-traducir) | Lo atado a Windows: `pixpin-shell` (Win32), `pixpin-render` (Direct2D), `pixpin-capture`, `pixpin-record`, `pixpin-ocr`, `pixpin-audio`, `pixpin-web` (WebView2), `pixpin-pin`, `pixpin-notas` (RichEdit; sus fórmulas `$…$`, que son lógica pura, ya viven en `nucleo/pixpin-docs/src/formulas.rs`) y la app `pixpin` | **Referencia para escribir la versión en Swift** con las herramientas de Apple (ScreenCaptureKit, Vision, PDFKit, AVFoundation, `NSPanel`…). No compila ni se compilará: se lee y se traduce pieza a pieza. |

Qué herramienta de Apple sustituye a cada pieza: §3 del plan.

## Compilar y probar el núcleo

```sh
cd apple
cargo test --workspace -- --test-threads=1
```

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
- [ ] Fase 1: crate `pixpin-nucleo` con UniFFI + `PixPinCore.xcframework` + paquete Swift.
- [ ] Fase 2: app de Mac en SwiftUI/AppKit (`apple/app/`).
- [ ] `por-adaptar/` → `nucleo/`: `pixpin-codec` (decodificar con `image`; ImageIO desde Swift),
      `pixpin-cad` y `pixpin-bim` (el lector; el 3D a Metal), `pixpin-pdf` (escribir y unir;
      ver con PDFKit).
- [x] `pixpin-ia` → `nucleo/` (15-oct): compila y pasa sus pruebas sin Windows. Lo de Windows
      va con `cfg(windows)`: bajar por HTTPS (WinHTTP de `pixpin-voz`), la nube por WinHTTP, el
      «trabajo» que mata a `llama-server` y DPAPI. Fuera de Windows: cliente HTTP/1.1 propio
      solo hacia `127.0.0.1` (llama-server y las pruebas de la nube con su servidor falso),
      `llama-server` en su grupo de procesos y muerto al soltarlo, y el trait
      `nube::claves::Llavero` para que Swift guarde las claves en el Llavero
      (`instalar_llavero`; sin él no se guarda nada, nunca en claro). Las herramientas en JSON
      para cualquier motor (Foundation Models desde Swift): `pixpin_ia::puente`.
  - [ ] Swift: bajar modelos y motor con URLSession (luego `descarga::verificar`), HTTPS de la
        nube con URLSession, el `Llavero` con Keychain, y Foundation Models con
        `puente::catalogo_para`/`puente::llamar` (llama.cpp con Metal de respaldo; en iPad e
        iPhone enlazado, sin `llama-server`).
  - [ ] Catálogo del motor para Mac: `modelos::LLAMA_CPP` y `descarga` siguen siendo el zip de
        Windows (`llama-server.exe`).
  - [ ] Si PixPin muere de golpe en Mac, `llama-server` queda huérfano (no hay Job Objects).
  - [ ] El banco (`tests/banco.rs`) no guarda PNG aquí (pintaba con `pixpin-pdf`/`pixpin-shell`).
