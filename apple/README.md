# PixPin para Mac, iPad e iPhone

Zona de trabajo de la versión de Apple. Plan completo y decisiones:
[`docs/investigacion/2026-10-15-mac-ipad-iphone.md`](../docs/investigacion/2026-10-15-mac-ipad-iphone.md).

Todo lo de aquí es una **copia tal cual** (copiar y pegar, sin reescribir) de `crates/` y `apps/`
del 15-oct-2026. Se irá arreglando y uniendo poco a poco. La versión de Windows no se toca desde
aquí: sigue viviendo en `crates/` y `apps/`.

## Las tres zonas

| Carpeta | Qué hay | Qué se hace con ello |
|---|---|---|
| [`nucleo/`](nucleo) | Los crates que **no dependen de Windows**: formato de proyectos, sincronizar, lienzo, Word/Excel/PowerPoint/EPUB, timeline, lecciones, la lógica de la interfaz, planos DWG/DXF y BIM (`pixpin-cad`, `pixpin-bim`)… | **Se usa tal cual.** Es un espacio de trabajo de Rust propio (`apple/Cargo.toml`) que compila y pasa sus pruebas fuera de Windows. Será `PixPinCore.xcframework` para Swift (fase 1). |
| [`por-adaptar/`](por-adaptar) | Crates **casi** puros, con una parte de Windows: `pixpin-codec` (WIC), `pixpin-pdf` (ver con Windows.Data.Pdf; escribir es propio), `pixpin-ia` (WinHTTP, Job Objects, DPAPI), `pixpin-voz` (SAPI, ONNX de System32), `pixpin-pila`, `pixpin-lanzador`, `pixpin-aligerar` | **Se queda en Rust**: separar la parte pura con `#[cfg(windows)]` o quitándola, y pasar cada crate a `nucleo/` cuando compile sin Windows. No compila todavía (fuera del espacio de trabajo). |
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

## Lo que falta (orden del plan)

- [ ] Fase 0.3: arreglos de sincronizar (tildes NFC, IPv6, versión del protocolo).
- [ ] Fase 1: crate `pixpin-nucleo` con UniFFI + `PixPinCore.xcframework` + paquete Swift.
- [ ] Fase 2: app de Mac en SwiftUI/AppKit (`apple/app/`).
- [x] `por-adaptar/` → `nucleo/`: `pixpin-cad` y `pixpin-bim`. Lo de Windows (`gpu`, `gpu3d`:
      D3D11 + HLSL; `ventana`, `ventana3d`: Win32; el registro de fuentes de `texto`; el
      `pintar` de `anotado::Anotador`) va con `#[cfg(windows)]`, intacto. Lo demás es puro:
      leer DWG/DXF/IFC/Revit/LandXML, medir, anotar, la página web y los búferes de la tarjeta
      (`Modelo` y `Modelo3d`: `Vec` planos de tipos `#[repr(C)]` con su tamaño comprobado al
      compilar, para Metal sin copias), la cámara 3D (`orbita`) y lo visible por elementos
      (`Modelo3d::visibles`). Fuera de Windows no hay Segoe UI ni Arial Narrow: los textos en
      geometría salen con Arial del Mac, las letras del usuario o las SHX de Hershey; lo demás
      lo escribe Swift con Core Text.
- [ ] Fase 2/3: visor de planos y de modelos 3D en Swift con Metal (traducir los sombreadores
      HLSL de `gpu.rs` y `gpu3d.rs` a MSL).
- [ ] `por-adaptar/` → `nucleo/`: `pixpin-codec` (decodificar con `image`; ImageIO desde Swift),
      `pixpin-pdf` (escribir y unir; ver con PDFKit), `pixpin-ia` (HTTP portable; Llavero en vez
      de DPAPI).
