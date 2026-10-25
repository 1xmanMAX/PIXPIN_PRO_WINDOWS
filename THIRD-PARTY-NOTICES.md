# Avisos de terceros

PixPin Max incluye código traducido a Rust de los siguientes proyectos, bajo sus licencias.

## Excalidraw

Módulos: `crates/pixpin-motor2d/src/tinta/{mod.rs,laser.rs}`, `crates/pixpin-render/src/tinta.rs`, `crates/pixpin-render/src/iconos_excalidraw.rs` (iconos de `packages/excalidraw/components/icons.tsx`; algunos de Tabler Icons, MIT).

MIT License

Copyright (c) 2020 Excalidraw

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## perfect-freehand

Módulo: `crates/pixpin-motor2d/src/tinta/freehand.rs`.

MIT License

Copyright (c) 2021 Stephen Ruiz Ltd

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## Material Icons (Google)

Módulos: `crates/pixpin-render/src/icono/material.rs` y los iconos de `apps/pixpin/src/sincronizar.rs`: trazados `d` de `src/<grupo>/<nombre>/materialicons/24px.svg` del repositorio `google/material-design-icons`, los mismos `Icons.Filled.*` que usa PixPin Android.

Apache License 2.0 — Copyright Google LLC. Texto completo en <https://www.apache.org/licenses/LICENSE-2.0>. Los iconos se usan sin modificar.

## Indra OCR (Datalab)

Módulos: `crates/pixpin-indra/src/*` — traducción a Rust (pasando por el puerto de Kotlin de Escáner Pro Max) de `indra_ocr` v1.1.2 de `github.com/datalab-to/indra` (commit `e20355319b5a`): `pipeline.py`, `det_post.py`, `rec_preproc.py`, `line_mask.py`, `lm.py`, `order_post.py`, `layout_post.py` y `vocab.py`. Cambios: en Rust, sin OpenCV (las operaciones de imagen están reescritas), solo el reconocedor latino, preparación de la página, líneas dudosas y corrección con diccionario añadidas.

Apache License 2.0 — Copyright Datalab. Texto completo en `crates/pixpin-indra/LICENCIA-INDRA.txt`.

**Los pesos del modelo no van con PixPin.** Son de Datalab (`huggingface.co/datalab-to/indra-ocr`, etiqueta v1.1.0) con licencia OpenRAIL-M modificada: gratis para uso personal, investigación y empresas con menos de 20 M$ de ingresos o de financiación que no compitan con Datalab; cualquier otro uso comercial necesita su licencia. El usuario los baja él mismo desde Ajustes (ver `docs/ocr.md`).

## Diccionario español hunspell (RLA-ES)

`es_ES.aff` y `es_ES.dic` de RLA-ES (Santiago Bosio y colaboradores, <https://github.com/sbosio/rla-es>) tal como los distribuye LibreOffice (`LibreOffice/dictionaries`, etiqueta `libreoffice-25.8.2.2`), sin modificar. Triple licencia GPL 3+ / LGPL 3+ / MPL 1.1+; PixPin los usa bajo la MPL. No van con PixPin: se bajan junto al lector Indra.

## ONNX Runtime (Microsoft)

En los Windows que no traen `onnxruntime.dll`, PixPin baja la oficial 1.31.0 (release de `microsoft/onnxruntime`) sin modificar. MIT License — Copyright (c) Microsoft Corporation.
