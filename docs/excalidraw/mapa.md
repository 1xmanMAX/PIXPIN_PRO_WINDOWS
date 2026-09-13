# Mapa de traducción de Excalidraw a PixPin

Cada fila es un fichero del original que tiene porte en Rust. `novedades-excalidraw.mjs`
lee la columna **Origen** y la **Revisión** para decir qué ha cambiado desde el último porte.

| Origen (repo:ruta) | Revisión portada | Módulo Rust | Notas |
|---|---|---|---|
| npm:perfect-freehand@1.2.0 `getStrokePoints`, `getStrokeOutlinePoints` | 1.2.0 | `pixpin-motor2d::tinta::freehand` | sin taper ni tapas planas |
| excalidraw:packages/laser-pointer/src/state.ts | afa3a65 | `pixpin-motor2d::tinta::laser` | solo `simplify 0`, presión 1 |
| excalidraw:packages/laser-pointer/src/math.ts | afa3a65 | `pixpin-motor2d::tinta::laser` | |
| excalidraw:packages/element/src/shape.ts (`getFreedrawOutlinePoints`, factores) | afa3a65 | `pixpin-motor2d::tinta` | |
| excalidraw:packages/element/src/shape.ts (`getSvgPathFromStroke`) | afa3a65 | `pixpin-render::tinta::pasos_de_tinta` | sin recorte a 2 decimales |
| excalidraw:packages/common/src/constants.ts (`FREEDRAW_STROKE_WIDTH`, `DEFAULT_STROKE_STREAMLINE*`) | afa3a65 | `pixpin-motor2d::tinta` | |
| excalidraw:packages/element/src/types.ts (`StrokeOptions`, freedraw) | afa3a65 | `pixpin-motor2d::{tinta, excalidraw}` | formato |

## Cómo portar una novedad

1. `node herramientas/novedades-excalidraw.mjs` lista los commits que tocan estas rutas.
2. Actualizar `herramientas/oraculo-excalidraw` (versiones npm si cambiaron) y `npm run generar`.
3. `cargo test -p pixpin-motor2d --test oraculo_tinta`: lo rojo es lo que hay que portar.
4. Portar, dejar verde, actualizar la columna **Revisión** de esta tabla.

Fork `zsviczian/excalidraw` (MIT): solo se mira lo que añade sobre el oficial. El plugin
`obsidian-excalidraw-plugin` es **AGPL-3.0**: nunca se porta código.
