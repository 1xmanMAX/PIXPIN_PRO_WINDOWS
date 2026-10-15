# PixPin para Mac, iPad e iPhone

Zona de trabajo de la versión de Apple. Plan completo y decisiones:
[`docs/investigacion/2026-10-15-mac-ipad-iphone.md`](../docs/investigacion/2026-10-15-mac-ipad-iphone.md).

Todo lo de aquí es una **copia tal cual** (copiar y pegar, sin reescribir) de `crates/` y `apps/`
del 15-oct-2026. Se irá arreglando y uniendo poco a poco. La versión de Windows no se toca desde
aquí: sigue viviendo en `crates/` y `apps/`.

## Las tres zonas

| Carpeta | Qué hay | Qué se hace con ello |
|---|---|---|
| [`nucleo/`](nucleo) | Los 18 crates que **compilan sin Windows**: formato de proyectos, sincronizar, lienzo, Word/Excel/PowerPoint/EPUB, timeline, lecciones, la lógica de la interfaz, imagen (`pixpin-codec`) y PDF (`pixpin-pdf`)… | **Se usa tal cual.** Es un espacio de trabajo de Rust propio (`apple/Cargo.toml`) que compila y pasa sus pruebas fuera de Windows. Será `PixPinCore.xcframework` para Swift (fase 1). |
| [`por-adaptar/`](por-adaptar) | Crates **casi** puros, con una parte de Windows: `pixpin-cad` (lector DWG/DXF + 3D en D3D11), `pixpin-bim`, `pixpin-ia` (WinHTTP, Job Objects, DPAPI), `pixpin-voz` (SAPI, ONNX de System32), `pixpin-pila`, `pixpin-lanzador`, `pixpin-aligerar` | **Se queda en Rust**: separar la parte pura con `#[cfg(windows)]` o quitándola, y pasar cada crate a `nucleo/` cuando compile sin Windows. No compila todavía (fuera del espacio de trabajo). |
| [`por-traducir/`](por-traducir) | Lo atado a Windows: `pixpin-shell` (Win32), `pixpin-render` (Direct2D), `pixpin-capture`, `pixpin-record`, `pixpin-ocr`, `pixpin-audio`, `pixpin-web` (WebView2), `pixpin-pin`, `pixpin-notas` (RichEdit) y la app `pixpin` | **Referencia para escribir la versión en Swift** con las herramientas de Apple (ScreenCaptureKit, Vision, PDFKit, AVFoundation, `NSPanel`…). No compila ni se compilará: se lee y se traduce pieza a pieza. |

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
4. **Lo de Windows no se borra al adaptar: se pone tras `#[cfg(windows)]`**, para que la copia
   siga siendo la misma que la de `crates/` y se pueda volver a unir. Donde la pieza no tiene
   sentido en Apple, la versión de fuera de Windows tiene la misma firma y devuelve un error
   que lo dice (`NoSoportadoAqui`: «en Mac, iPad e iPhone lo hace Swift con …»). Se comprueba
   que Windows sigue compilando con
   `cargo check --target x86_64-pc-windows-msvc -p <crate> --all-targets`.

### Qué hace Swift en lugar de los crates adaptados

- **`pixpin-codec`**: en Rust queda PNG/JPEG/WebP/BMP con `image`, el GIF propio, coser,
  filtros, redimensionar, la orientación EXIF y entender un CF_HTML o un RTF. **HEIC, AVIF,
  TIFF, RAW y leer reducido** (`vista`) los hace **ImageIO** (`CGImageSource`,
  `CGImageSourceCreateThumbnailAtIndex`); el **portapapeles**, `NSPasteboard`/`UIPasteboard`.
- **`pixpin-pdf`**: en Rust queda escribir, anotar, unir, sacar texto y enlaces, el plano
  vectorial, medir letras incrustadas e inspeccionar qué se puede aligerar. **Ver páginas**
  (`Documento`) lo hace **PDFKit**; **aligerar de verdad** (recodificar con WIC y comprobar
  dibujando) también es de Swift (ImageIO + PDFKit, o `pdfsqueeze-core`); y la letra del
  sistema (`letra::del_sistema`, DirectWrite) la busca Core Text y se la pasa a
  `Letra::desde_fichero`. Sin ella, el PDF sale en Helvetica como en Windows.

## Lo que falta (orden del plan)

- [ ] Fase 0.3: arreglos de sincronizar (tildes NFC, IPv6, versión del protocolo).
- [ ] Fase 1: crate `pixpin-nucleo` con UniFFI + `PixPinCore.xcframework` + paquete Swift.
- [ ] Fase 2: app de Mac en SwiftUI/AppKit (`apple/app/`).
- [ ] `por-adaptar/` → `nucleo/`:
  - [x] `pixpin-codec` (decodificar con `image`; WIC, vista reducida y portapapeles tras
        `#[cfg(windows)]`; ImageIO y `NSPasteboard` desde Swift).
  - [x] `pixpin-pdf` (escribir, unir, texto, plano; ver con `Windows.Data.Pdf` y aligerar tras
        `#[cfg(windows)]`; PDFKit desde Swift).
  - [ ] `pixpin-cad` y `pixpin-bim` (el lector; el 3D a Metal).
  - [ ] `pixpin-ia` (HTTP portable; Llavero en vez de DPAPI).
  - [ ] `pixpin-voz`, `pixpin-pila`, `pixpin-lanzador`, `pixpin-aligerar`.
- [ ] CI: el trabajo `apple-nucleo` ya prueba `pixpin-codec` y `pixpin-pdf` (usa
      `--workspace`). Falta un paso que haga
      `cargo check --target x86_64-pc-windows-msvc` de los crates adaptados para que lo de
      `#[cfg(windows)]` no se pudra (la lista `nucleo-portable` es de `crates/`, no de aquí).
