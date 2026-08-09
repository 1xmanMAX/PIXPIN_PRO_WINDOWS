# PixPin PC — Documento maestro de diseño

**Estado:** COMPLETO — Secciones 1, 2 y 3 aprobadas. Pendiente de revisión final del usuario.
**Iniciado:** 2026-08-08 · **Cerrado:** 2026-08-09
**Método:** skill `superpowers:brainstorming` (diseño → aprobación → spec → plan → implementación)

> *PixPin* es marca de DepthPixel. Este proyecto es una implementación personal e
> independiente, igual que la versión Android. Esta nota debe aparecer en el README
> y en el diálogo «Acerca de» desde la primera versión.

---

## 0. Punto de partida

**Directorio de trabajo:** `F:\THE FORGE\PIXPIN PC VERSION MAX` — vacío, proyecto nuevo desde cero. No es todavía un repositorio git.

### 0.1 Repositorio de origen — PIXPIN_PRO_ANDROID

`https://github.com/1xmanMAX/PIXPIN_PRO_ANDROID` · Kotlin + Jetpack Compose · Android 10+ · MIT · 732 tests unitarios JVM.

Medido sobre un clon del repo: **144 archivos `.kt`, 36.102 líneas** en `main`, más **50 archivos y 9.595 líneas** de tests.

| Módulo | Líneas | Contenido |
|---|---:|---|
| `motor` | 14.599 | modelo, geometría, render, PDF — **lógica pura, lo más portable** |
| `pin` | 5.067 | gestión de ventanas, almacenamiento de pines |
| `capture` | 2.220 | MediaProjection, recorte, stitching, exportación |
| `annotate` | 1.371 | entrada de stylus, suavizado de trazo |
| `markdown` | 521 | renderizado markdown |
| `capa` | 491 | dibujo sobre overlay de pantalla |
| `floating` | 464 | bola flotante, tile, servicio |
| `data` | 405 | ajustes, persistencia |
| `clipboard` | 391 | portapapeles, compartir |
| `ui` | 302 | componentes de interfaz |

**Documentos de diseño previos en `docs/`** — material imprescindible antes de especificar cada sub-proyecto:

- `motor.md`
- `2026-07-26-pixpin-android-design.md`
- `2026-07-27-correcciones-estabilidad-fluidez.md`
- `2026-07-28-anotacion-grupos-scroll-design.md`
- `2026-07-30-texto-prioridad-sticker-design.md` (+ `2026-07-30-texto-prioridad-sticker.md`, 74 KB)
- `2026-08-01-markdown-pellizco-sombra-design.md`
- `2026-08-02-croquis-acotado-design.md`
- 19 diagramas SVG (gestos, herramientas, pines, superficies, anclajes, recorte, relleno, salidas…)

### 0.2 El hallazgo que define el proyecto: el motor es Excalidraw

Verificado leyendo el código fuente del módulo `motor`. **No hay formato propietario.** El motor de anotación es un puerto de Excalidraw a Kotlin, extendido con las herramientas de anotación (mosaico, foco, numeración de serie) heredadas del motor de anotación anterior, y con las de medición (cota, escala) heredadas de la aplicación de croquis.

Pruebas concretas encontradas en el código:

- **`ExcalidrawStore.kt`** (179 líneas) — persiste en **`.excalidraw.gz`**: JSON estándar comprimido con gzip, un archivo por pin, imágenes incrustadas en `pins/draw/files/`. Escribe a un temporal y renombra, «porque en esta aplicación el proceso muere sin avisar más de lo normal, y un archivo a medias no abre». Lleva contadores de revisión global y por dibujo para no tirar el historial de deshacer de un pin cuando otro guarda.
- **`Rand.kt`** (51 líneas) — porta **bit a bit** el generador pseudoaleatorio de rough.js: un Lehmer (Park–Miller) con multiplicador 48271 y módulo 2³¹−1. El comentario del propio código explica por qué es innegociable: *«si el generador difiere, un dibujo exportado y reabierto en el navegador se ve distinto aunque el JSON sea idéntico»*. `randomId()` replica el alfabeto y la longitud exactos de nanoid de Excalidraw para que los ids no colisionen al mezclar escenas hechas en los dos sitios.
- **`Scene.kt`** (659 líneas) — fija tres reglas de serialización obligatorias para que el JSON salga idéntico al de la web: `encodeDefaults = true`, `explicitNulls = false`, `ignoreUnknownKeys = true`.
- **`Rough.kt`** (503 líneas) — el renderizado de trazo a mano alzada, la firma visual de Excalidraw.

**Consecuencias, y son grandes:**

1. La interoperabilidad con excalidraw.com y con el plugin de Excalidraw para Obsidian **ya está pagada**. Es una función de cabecera del producto, no un extra.
2. El criterio de corrección del puerto deja de ser una opinión: es **medible byte a byte**.
3. No hay que diseñar formato de archivo. La decisión ya está tomada y es un estándar abierto y documentado.

Ficheros del módulo `motor`, por tamaño (mapa del trabajo de S2):

| Fichero | Líneas | Fichero | Líneas | Fichero | Líneas |
|---|---:|---|---:|---|---:|
| `DrawController.kt` | 1.492 | `Freehand.kt` | 410 | `Bounds.kt` | 230 |
| `Renderer.kt` | 1.291 | `Recorte.kt` | 396 | `ExcalidrawStore.kt` | 179 |
| `DrawEditorActivity.kt` | 1.210 | `Arrows.kt` | 392 | `Barra.kt` | 164 |
| `DrawToolbar.kt` | 962 | `Shapes.kt` | 379 | `DrawProperties.kt` | 157 |
| `Scene.kt` | 659 | `TransformHandles.kt` | 336 | `Tablas.kt` | 150 |
| `DrawCanvas.kt` | 598 | `Transform.kt` | 303 | `Angulos.kt` | 130 |
| `Regiones.kt` | 555 | `Perimetros.kt` | 300 | `Elbow.kt` | 117 |
| `Nudos.kt` | 512 | `PdfLector.kt` | 276 | `History.kt` | 115 |
| `Rough.kt` | 503 | `Organize.kt` | 259 | `EscalaGrafica.kt` | 115 |
| `PdfLectura.kt` | 483 | `Snapping.kt` | 255 | `DrawPdf.kt` | 99 |
| `Element.kt` | 467 | `Collision.kt` | 252 | `Arco.kt` | 99 |
| | | `Medida.kt` | 245 | `DrawExport.kt` | 81 |
| | | `DrawTablas.kt` | 235 | `Rand.kt` | 51 |

### 0.3 Repositorio de referencia — QuickView

`https://github.com/justnullname/QuickView` · C++23 + Direct2D · **GPL-3.0** · **de terceros, no del usuario**.

Medido sobre un clon disperso: **118 archivos, ~88.000 líneas**.

| Fichero | Líneas | Contenido |
|---|---:|---|
| `main.cpp` | 14.039 | monolito: ventana, input, orquestación |
| `ImageLoader.cpp` | 12.891 | decodificación multi-formato |
| `AppStrings.cpp` | 6.671 | i18n |
| `UIRenderer.cpp` | 5.611 | toolkit de UI sobre Direct2D |
| `GeekIconData.cpp` | 5.258 | iconos vectoriales |
| `SettingsOverlay.cpp` | 4.686 | ajustes |
| `HeavyLanePool.cpp` | 3.381 | pool de hilos de decodificación |
| `RenderEngine.cpp` | 2.614 | pipeline de render |
| `CompositionEngine.cpp` | 1.322 | DirectComposition |
| `TileManager.cpp` | 475 | «Titan Tiling» para gigapíxel |

Dependencias vcpkg: `libjpeg-turbo`, `libwebp` (simd), `libavif` (dav1d), `libjxl`, `libraw`, `zlib`, `highway`, `gtest`. Compilador clang-cl con LTO completo.

**Verificado buscando en su código fuente:** QuickView **no tiene** captura de pantalla (ni `GraphicsCapture` ni `DesktopDuplication`), **no tiene** anotación, **no tiene** ventanas `WS_EX_LAYERED`. Su `EditState.h` sólo cubre rotación y volteo sin pérdida. Es un **visor** excelente, no un editor: no aporta nada del núcleo de PixPin.

**Conclusión:** aporta ideas, no código. Su velocidad viene sobre todo de librerías upstream (libjpeg-turbo, libjxl, dav1d, LibRaw, Highway, Wuffs) que son BSD/MIT/Apache y **están disponibles desde Rust con el mismo rendimiento**. La GPL sólo se heredaría por su pegamento, no por su potencia.

---

## 1. Decisiones tomadas

| # | Decisión | Elección | Razón |
|---|---|---|---|
| D1 | **Forma del producto** | **Nativo Windows**, no réplica del Android | Windows da atajos globales, multi-monitor, ventanas reales y arrastrar-y-soltar entre apps. La bola flotante deja de ser necesaria. |
| D2 | **Relación con QuickView** | **Proyecto nuevo**; tomar *técnicas*, nunca *código* | Es GPL-3.0 y de terceros. Copiar su código contaminaría la licencia y obligaría a mantener un fork contra un upstream muy activo (1.086 commits, v6.8.0 en junio 2026) con un `main.cpp` de 14.039 líneas. |
| D3 | **Licencia** | **MIT**, igual que el Android | Se preserva al no usar código GPL. Se hace cumplir por CI (ver 3.5). |
| D4 | **Lenguajes** | **Rust + HLSL + librerías C/C++ por FFI** | Ver 1.1. |
| D5 | **Stack gráfico** | **Direct2D + DirectComposition + DirectWrite** vía el crate `windows` | Mismo camino gráfico que hace rápido a QuickView, pero desde Rust. |
| D6 | **Alcance** | **6 sub-proyectos**, cada uno con spec → plan → implementación y una versión usable | El total estimado (~100.000 líneas de Rust) no cabe en una sola especificación. |
| D7 | **Orden de trabajo** | Documento maestro primero, luego S1 | Petición explícita del usuario. |
| D8 | **Formato de archivo** | **`.excalidraw` / `.excalidraw.gz`**, sin contenedor propio | Es lo que ya usa el Android. Da compatibilidad de ida y vuelta gratis e interoperabilidad con excalidraw.com. Ver 0.2 y 3.1. |
| D9 | **Traducción offline** | **Modelos locales descargables a demanda** | Instalador ligero; el usuario baja sólo los pares de idiomas que quiera (~40-80 MB cada uno). Preserva la promesa de privacidad total. |
| D10 | **Windows mínimo** | **Windows 10 21H2** (compilación 19044) | Cubre casi todo el parque. Donde WGC no pueda suprimir el borde amarillo se usa DXGI Desktop Duplication como respaldo. *Refinado por S1-6: el respaldo se decide preguntando al sistema si la capacidad existe, no comparando números de compilación.* |
| D11 | **Nombre** | **PixPin Max**, ejecutable `pixpinmax.exe` | Recoge el nombre de la carpeta de trabajo y se diferencia algo más de la marca ajena. Se mantiene la nota sobre DepthPixel. |
| D12 | **Idiomas en v1** | **Español (`es-ES`) e inglés (`en-US`)**, con Fluent | Español porque es el idioma del autor y de toda la documentación; inglés porque sin él el proyecto no existe fuera de su círculo. Añadir más idiomas después es añadir ficheros. |

### 1.1 Justificación de D4 — el reparto de lenguajes

Premisa corregida durante el diseño: **Rust habla con el hardware exactamente igual de directo que C++**. Mismo backend LLVM, sin runtime, sin recolector de basura, con intrínsecos SIMD, `#[repr(C)]`, punteros crudos y ensamblador en línea. La diferencia no es la distancia al hardware, sino quién comprueba los errores de memoria. Añadir C++ propio no compraría rendimiento.

| Lenguaje | Dónde | Por qué ahí |
|---|---|---|
| **Rust** | Todo el código propio: motor, captura, pines, UI, PDF | Seguridad de memoria sin coste + puerto casi mecánico desde Kotlin (clases selladas, tipos suma, `Result`/`Option`, inmutabilidad, pattern matching) |
| **HLSL** | Mosaico, desenfoque, foco, escalado, corrección de color | Aquí está la velocidad real: pixelar 4K en CPU son millones de operaciones; en GPU es un shader. Ningún lenguaje de CPU compite. |
| **C/C++ por FFI** | `dav1d`, `libjxl`, `LibRaw`, `libjpeg-turbo`, `resvg` | Ya existen, afinados a mano con SIMD, enlace con coste cero. Reescribirlos sería absurdo. |

Toolchain propio: **sólo `cargo`**. Sin vcpkg, sin CMake, sin clang-cl.

### 1.2 Librerías Rust candidatas

`windows` (Win32/D2D/DWrite/DComp/WGC/Media Foundation) · `kurbo` (curvas Bézier, suavizado de trazo) · `lyon` (teselación) · `serde_json` (formato Excalidraw) · `flate2` (gzip) · `jxl-oxide` (JXL) · `dav1d` (AVIF) · `zune-jpeg` / libjpeg-turbo · `rawler` (RAW) · `resvg` (SVG) · `pdf-writer` (PDF multipágina, de Typst) · `pdfium-render` o `mupdf` (lectura PDF) · `fluent-rs` (i18n) · `tracing` (registro) · `thiserror` / `anyhow` (errores)

---

## 2. Arquitectura — Sección 1, aprobada

**Principio rector:** las capas de abajo no saben que existe Windows. Toda la lógica delicada —geometría, modelo de documento, undo/redo, serialización— vive en crates puros sin una sola llamada al sistema operativo. Es la misma decisión que ya se tomó en Android («la lógica delicada vive en objetos puros»), la que hace posibles los 732 tests, llevada más lejos.

```
                        apps/pixpin  (el .exe)
                              |
   ------------------------------------------------------------
   L3  APLICACION
       pixpin-ui        toolkit propio: widgets, temas, layout, IME
       pixpin-flow      automatizacion, plantillas, destinos, historial
       pixpin-plugin    API de extension + CLI
   ------------------------------------------------------------
   L2  DOMINIO
       pixpin-capture   WGC/DXGI, monitores, ventanas, scroll+stitching
       pixpin-pin       ventanas layered, grupos, gestos, mini-apps
       pixpin-pdf       leer / escribir / editar / pinear PDF
       pixpin-ocr       Windows.Media.Ocr + traduccion offline
       pixpin-record    Media Foundation, encoder HW, GIF
       pixpin-store     ajustes, historial, portapapeles, modo portable
   ------------------------------------------------------------
   L1  PLATAFORMA
       pixpin-shell     Win32, DirectComposition, bandeja, atajos globales
       pixpin-render    backend Direct2D + DirectWrite
       pixpin-gpu       shaders HLSL: mosaico, blur, foco, escalado, color
       pixpin-codec     FFI: dav1d, libjxl, LibRaw, turbojpeg, resvg
   ------------------------------------------------------------
   L0  PURO   (cero dependencias del SO -- aqui viven los tests)
       pixpin-model     elementos, capas, estilos, undo/redo, formato
       pixpin-geom      kurbo, transformaciones, hit-testing, suavizado
   ------------------------------------------------------------

   Regla: una capa solo depende de capas inferiores. Sin ciclos.
```

**Qué gana cada frontera:**

- **L0 puro** — Compila y testea en segundos, sin GPU ni Windows. Aquí aterrizan las 14.599 líneas de `motor` y los 732 tests, que pasan a ser la **especificación ejecutable** del puerto.
- **`pixpin-render` como frontera única de dibujo** — Todo el dibujo pasa por aquí. Si Direct2D estorbase algún día (o se quisiera Linux), se cambia un crate y nada más se entera.
- **`pixpin-gpu` separado** — Los shaders son el músculo. Aislarlos permite medirlos y optimizarlos sin tocar lógica.
- **`pixpin-codec` aislado** — Todo el C/C++ ajeno queda encerrado tras una frontera `unsafe` pequeña y auditable. El resto del programa es Rust seguro.
- **`pixpin-model` implementa el esquema de Excalidraw** — Un solo sitio decide cómo se lee y se escribe un documento.

La exportación de imagen no necesita crate propio: la codificación vive en `pixpin-codec` y la orquestación en `pixpin-flow`.

---

## 3. Descomposición y catálogo por sub-proyecto — Sección 2, aprobada

```
S1  CIMIENTOS + CAPTURA          -> v0.1  ya capturas y guardas
                |
S2  MOTOR DE ANOTACION + EDITOR  -> v0.2  ya anotas
                |
S3  PINES + MINI-APPS            -> v0.3  el alma de PixPin
                |
S4  SALIDAS + AUTOMATIZACION     -> v0.4  herramienta de trabajo diario
                |
S5  OCR + TRADUCCION + GRABACION -> v0.5  lo que no existia en Android
                |
S6  VISOR PRO + PDF COMPLETO     -> v1.0  supera a QuickView
```

Cada flecha es una dependencia real: sin captura no hay nada que anotar; sin motor de anotación no hay pin con contenido; el visor pro y el PDF reutilizan el pipeline de tiles y el motor de anotación ya maduros, por eso van al final.

Cada sub-proyecto tendrá su propia especificación y su propio plan. Lo que no está escrito en su bloque, no se construye en esa fase.

### S1 — Cimientos + Captura → v0.1

**Objetivo:** capturar de forma instantánea y fiable, con una app que arranca en frío rápido y no consume nada en reposo.

**Entregables**
- Workspace `cargo` con los **15 crates de librería más el ejecutable** en esqueleto, CI y comprobación de la regla de capas.
- `pixpin-shell` — ventana Win32, DirectComposition, DPI por monitor v2, bandeja del sistema, atajos globales (`RegisterHotKey`), instancia única, arranque con Windows opcional.
- `pixpin-render` — backend Direct2D + DirectWrite con **bucle de render dirigido por eventos, no por fotogramas**. Es lo que garantiza 0% de CPU en reposo.
- `pixpin-capture` — Windows.Graphics.Capture como vía principal y DXGI Desktop Duplication como respaldo (ver D10); captura de monitor, de ventana concreta y de región libre.
- **Overlay de selección** sobre todos los monitores con DPI mixto: lupa con zoom a nivel de píxel, coordenadas en vivo, **snap automático a bordes de ventana y a controles de interfaz** (vía UI Automation), ajuste fino con teclado.
- Captura larga con scroll — puerto de `ScrollMatcher` / `ScrollStitcher`.
- **Cuentagotas global** con lupa y códigos HEX/RGB/HSL.
- `pixpin-store` — ajustes en TOML, modo portable, base de i18n.
- Salida mínima: portapapeles y guardar a PNG/JPG/WebP.

**Criterios de aceptación** — se cumple el presupuesto de rendimiento de 3.4 · funciona con tres monitores de escalado distinto · la captura 4K permanece en textura GPU sin copia a CPU.

**Fuera de alcance:** anotación, pines, PDF, OCR, grabación.

### S2 — Motor de anotación + Editor → v0.2

**Objetivo:** portar el corazón — 14.599 líneas de `motor` más 1.371 de `annotate`.

**Entregables**
- `pixpin-geom` — transformaciones, hit-testing, intersecciones, suavizado de trazo (`kurbo`), teselación (`lyon`).
- `pixpin-model` — esquema de Excalidraw completo, elementos, capas, estilos, undo/redo, lectura y escritura de `.excalidraw.gz`.
- Puerto de `Rough.kt` y `Rand.kt` con fidelidad bit a bit (ver 3.2).
- `pixpin-gpu` — shaders HLSL: mosaico, desenfoque, foco/spotlight, escalado, corrección de color.
- Las cuatro familias de herramientas del Android, completas:
  - **Dibujar** — rectángulo, elipse, rombo, línea, flecha, lápiz, rotulador, marcadores de esquina
  - **Tapar** — mosaico/pixelado, foco, numeración de serie, borrador
  - **Medir** — líneas de cota, escalado, escalas gráficas, ángulos internos, regla y transportador en pantalla
  - **Construir** — cubo de relleno, recortar y extender, guías de alineación
- Texto con DirectWrite: IME, escrituras complejas, y el sistema de prioridad/sticker de `2026-07-30-texto-prioridad-sticker-design.md`.
- Renderizado markdown (puerto del módulo `markdown`).
- `pixpin-ui` — barra de herramientas, paletas, atajos por herramienta.

**Criterio de aceptación clave:** los tests portados pasan dando los mismos resultados que en Kotlin, y el corpus de round-trip de 3.2 sale idéntico byte a byte.

### S3 — Pines + mini-apps → v0.3

**Objetivo:** el alma de PixPin, y donde Windows deja superar al Android.

**Entregables**
- `pixpin-pin` — ventanas `WS_EX_LAYERED` compuestas por DirectComposition: siempre encima, transparencia real, sombra, sin parpadeo.
- **Arrastrar el contenido de un pin a otras aplicaciones** (OLE drag & drop) — imposible en Android.
- **Proyectar la ventana viva de otra aplicación dentro de un pin** — imposible en Android.
- Grupos, opacidad, gestos, anclajes, restauración al reiniciar.
- Mini-apps: temporizadores, contadores, listas, contabilidad, pizarras.
- Historial de portapapeles con miniaturas y capacidad de pinear cualquier cosa copiada.

### S4 — Salidas + Automatización → v0.4

**Objetivo:** convertirlo en herramienta de trabajo diario.

**Entregables**
- Exportación a imagen (codificación en `pixpin-codec`, orquestación en `pixpin-flow`).
- **PDF multipágina** de salida con `pdf-writer`.
- Formato editable, que es el `.excalidraw` ya implementado en S2.
- `pixpin-flow` — flujos post-captura: renombrado por plantilla, marca de agua automática, copiar+guardar+subir en un paso, perfiles distintos por atajo, historial buscable.
- `pixpin-plugin` — CLI para automatizar desde scripts y documentación pública del formato.

### S5 — OCR + Traducción + Grabación → v0.5

**Objetivo:** lo que en Android no existía.

**Entregables**
- `pixpin-ocr` — Windows.Media.Ocr, offline, 25+ idiomas: seleccionar texto sobre la captura como si fuera un documento, copiar, buscar.
- Traducción con **modelos locales descargables a demanda** (D9), motor tipo Marian/CTranslate2 por FFI.
- `pixpin-record` — Media Foundation con codificación H.264/HEVC por hardware, salida a MP4/WebM/GIF, anotación **durante** la grabación, recorte del clip y audio opcional.

### S6 — Visor pro + PDF completo → v1.0

**Objetivo:** superar a QuickView, porque hace todo lo suyo y además anota.

**Entregables**
- `pixpin-codec` completo — RAW de 30+ cámaras, HDR, JXL, AVIF, WebP, PSD, EXR, SVG y archivos ZIP/CBZ leídos sin descomprimir.
- Tiling para gigapíxel a 60 fps, histograma, EXIF, comparación dual sincronizada, gestión de color.
- `pixpin-pdf` — ver, **anotar reutilizando el motor de S2**, editar (mover, borrar, añadir texto e imágenes), reordenar y combinar páginas, rellenar formularios, firmar, y **pinear una página como ventana flotante**. Punto de partida: `PdfLectura.kt` (483) y `PdfLector.kt` (276) del Android.

### Estimación de tamaño (orden de magnitud, no medición)

| Bloque | Líneas Rust estimadas |
|---|---:|
| Puerto del Android (36.102 Kotlin) | ~40.000 |
| Captura Windows (WGC, overlay, multi-monitor, scroll) | ~8.000 |
| Pines como ventanas layered + mini-apps | ~10.000 |
| OCR + traducción | ~3.000 |
| Grabación con codificación por hardware | ~6.000 |
| Visor pro (RAW/HDR/JXL/gigapíxel/comparación) | ~15.000 |
| PDF: ver, anotar, editar, pinear | ~12.000 |
| Automatización tipo ShareX | ~5.000 |
| **Total** | **~100.000** |

### Funciones desbloqueadas por Windows

El README del Android lista lo que la plataforma no permitía. En Windows sí se puede, y está incorporado arriba:

- Proyectar la ventana viva de otra aplicación (S3)
- Atajos de teclado globales (S1)
- Arrastrar el contenido de un pin a otras aplicaciones (S3)
- Multi-monitor con DPI mixto (S1)
- Acceso completo al portapapeles (S3/S4)

---

## 4. Decisiones técnicas transversales — Sección 3, aprobada

### 4.1 Formato de archivo

Se mantiene **`.excalidraw` / `.excalidraw.gz`** tal cual, sin contenedor propio (D8).

Qué se compra: compatibilidad de ida y vuelta con la app Android sin escribir una línea de conversión · interoperabilidad real con excalidraw.com y con el plugin de Obsidian · el requisito de «formato abierto y documentado» de la familia Extensibilidad queda cumplido de oficio · y un formato ya probado en producción.

En Rust: `serde_json` con `skip_serializing_if = "Option::is_none"` para replicar `explicitNulls = false`, y `#[serde(default)]` con tolerancia a campos desconocidos, que es el comportamiento por defecto de serde. La escritura a temporal + rename se conserva: en Windows también importa.

Las estructuras zero-copy (`rkyv`, `postcard`) se reservan para cachés internas —miniaturas, tiles, diario de deshacer—, nunca para el formato de archivo.

### 4.2 Fidelidad bit a bit — criterio de corrección del puerto

El Lehmer se traduce a Rust sin ambigüedad, porque `wrapping_mul` es exactamente el desbordamiento con signo de 32 bits de Kotlin y de `Math.imul` en JavaScript:

```rust
// Park–Miller, idéntico a rough.js, a Kotlin y a excalidraw.com
state = state.wrapping_mul(48271);
(state & 0x7FFF_FFFF) as f64 / 2_147_483_648.0
```

**Criterio de aceptación de S2:** un corpus de archivos `.excalidraw` cargados y reescritos por el Rust produce el mismo JSON byte a byte que el Kotlin, y el mismo garabato pixel a pixel para la misma semilla. Si eso pasa, el motor está bien portado.

### 4.3 Portado de los tests

La suite Kotlin son 50 archivos y 9.595 líneas (los 732 tests). Los mayores marcan dónde está el riesgo real:

| Test | Líneas | Test | Líneas |
|---|---:|---|---:|
| `DrawControllerTest` | 862 | `SnappingTest` | 338 |
| `DrawGeometryTest` | 664 | `PlanoTest` | 319 |
| `NudosTest` | 620 | `RecorteTest` | 293 |
| `GuiasTest` | 393 | `ArcoTest` | 286 |
| `RegionesTest` | 383 | `PdfLecturaTest` | 284 |
| `OverlayTouchHandlerTest` | 267 | `MedidaTest` | 252 |
| `InterseccionesTest` | 245 | `CirculoTest` | 243 |
| `AnnotationControllerTest` | 235 | `MarkdownTest` | 205 |
| `FreehandTest` | 201 | `TablasTest` | 199 |
| `BindingTest` | 197 | `FrameTest` | 169 |

Estrategia en dos carriles:

1. **Traducción 1:1** de los tests de comportamiento a `#[test]` de Rust. El test se escribe **antes** que la implementación, herramienta por herramienta.
2. **Ficheros dorados** para todo lo que sea entrada→salida: se extraen los casos y sus resultados esperados a JSON neutral, y *ambos* lenguajes lo consumen. Un mismo fichero valida Kotlin y Rust, y cualquier divergencia futura salta sola.

### 4.4 Presupuesto de rendimiento

Puertas de calidad, no aspiraciones. Se miden en CI donde se pueda.

| Métrica | Objetivo |
|---|---|
| Arranque en frío hasta bandeja | < 300 ms |
| Atajo global → overlay visible | < 50 ms |
| Latencia de trazo | < 8 ms (un fotograma a 120 Hz) |
| CPU en reposo | 0% (render dirigido por eventos) |
| RAM en reposo | < 40 MB |
| RAM con 10 pines abiertos | < 150 MB |
| Mosaico sobre región 4K | < 5 ms (shader HLSL) |
| Tamaño del binario | < 30 MB |

### 4.5 Build, CI y puerta de licencias

`cargo` únicamente. Rust estable, sin nightly. Objetivos `x86_64-pc-windows-msvc` y `aarch64-pc-windows-msvc`. En release: LTO completo, `codegen-units = 1`, `panic = "abort"`, símbolos despojados.

CI en GitHub Actions:
- Los tests de L0 corren sin GPU ni Windows, en segundos.
- `clippy` con avisos como errores.
- Comprobación automática de la regla de capas (sin ciclos, sin saltos hacia arriba).
- **`cargo-deny` configurado para rechazar cualquier dependencia GPL o AGPL.** Convierte D2 y D3 en una regla que la máquina hace cumplir, en vez de algo que hay que recordar.

### 4.6 Fronteras `unsafe`

Los 15 crates se reparten en dos grupos, sin excepciones ni casos intermedios:

- **`#![forbid(unsafe_code)]` — 6 crates:** `pixpin-geom`, `pixpin-model`, `pixpin-flow`, `pixpin-store`, `pixpin-ui`, `pixpin-plugin`. El compilador impide el `unsafe`, no lo desaconseja.
- **`unsafe` permitido, auditado y documentado con `// SAFETY:` — 9 crates:** `pixpin-shell`, `pixpin-render`, `pixpin-gpu`, `pixpin-codec`, `pixpin-capture`, `pixpin-record`, `pixpin-pin`, `pixpin-ocr`, `pixpin-pdf`. Son exactamente los que hablan con el sistema operativo o con librerías C.

`pixpin-ui` puede estar en el primer grupo porque dibuja a través de `pixpin-render` y nunca llama a Win32 por su cuenta. Si algún día necesitase hacerlo, es señal de que la frontera se ha roto y hay que arreglar el diseño, no relajar la regla.

### 4.7 Empaquetado y distribución

Ejecutable único. **Modo portable de verdad** (ajustes junto al `.exe`, cero rastro en el registro) e instalador MSI para quien lo prefiera. Publicación en WinGet. Comprobación de actualizaciones sólo si el usuario la activa.

### 4.8 Privacidad, idiomas y diagnóstico

**Cero telemetría, cero analíticas, cero cuentas** — igual que el Android. La app no toca la red salvo dos acciones que el usuario pide explícitamente: descargar un modelo de traducción o buscar actualizaciones.

Interfaz con Fluent (`fluent-rs`), que maneja plurales y géneros correctamente en español. Registro con `tracing` a fichero rotativo local, más el volcado de fallos equivalente a `CrashLog.kt`. Nada sale del equipo.

---

## 5. Estado y siguientes pasos

1. ~~Revisión de este documento por el usuario~~ — **aprobado el 2026-08-09**.
2. ~~Inicializar el repositorio git~~ — **hecho**, rama `main`, commit inicial `a884aed`.
3. ~~Brainstorming y especificación de **S1**~~ — **hecha**: [`2026-08-09-s1-cimientos-captura-design.md`](2026-08-09-s1-cimientos-captura-design.md).
4. **Siguiente:** plan de implementación de S1, y a escribir Rust.

### Tareas de arranque de S1

- [ ] Inicializar el workspace `cargo` con los 15 crates de librería, el ejecutable y la comprobación de capas
- [ ] Configurar `cargo-deny` con la lista de licencias permitidas (rechazar GPL/AGPL)
- [ ] Volver a clonar los repos de referencia si hiciera falta:
  - `git clone --depth 1 https://github.com/1xmanMAX/PIXPIN_PRO_ANDROID.git`
  - `git clone --depth 1 --filter=blob:none https://github.com/justnullname/QuickView.git`
- [ ] Leer los 7 documentos de diseño de `docs/` del repo Android antes de especificar S2

### Preguntas abiertas

- ~~Nombre definitivo del producto y del ejecutable~~ — resuelto en D11: **PixPin Max**, `pixpinmax.exe`.
- ~~Idiomas de la interfaz en la v1~~ — resuelto en D12: **español e inglés**.
- El sistema de plugins de la familia Extensibilidad sigue sin sub-proyecto asignado: en S4 sólo entran el CLI y el formato documentado. Los plugins quedan **para después de la v1.0** y necesitarán su propio diseño.
