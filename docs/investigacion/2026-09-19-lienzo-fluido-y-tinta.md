# Lienzo fluido y tinta de baja latencia: qué hacemos hoy y qué hacen los demás

**Fecha:** 2026-09-19 · **Petición del usuario:** «se siente la latencia en todas partes del canvas al
mover, también en el universo… investiga en proyectos de código abierto y foros formas y algoritmos
optimizados de tinta y más para implementarlos… altamente optimizado, rendimiento maximizado, baja
latencia de respuesta y fluidez natural».

**Alcance:** solo investigación. No se ha tocado código. Máquina suelo: Core i3 Ivy Bridge, 4 GB,
Intel HD 4000 (sin AVX2), Windows 10 21H2+.

**Lo ya hecho que NO se vuelve a proponer:** swapchain con objeto de espera y latencia 1
(`Superficie::nueva_baja_latencia`), predicción lineal de la punta (`tinta::prediccion`), copiar solo
la zona sucia de la capa congelada (D148), `Present1` con rectángulo sucio (D125), realizaciones de
geometría para la tinta (D121), estirar la tinta durante el zoom y rehacerla nítida tras 300/500 ms
(D122), `GetMouseMovePointsEx` + historial del lápiz (D105/D107), bucle sin `sleep` con
`MsgWaitForMultipleObjectsEx` (D110), `[rendimiento] medir_fotogramas` (D129), y en los pines el zoom
por transformada de composición (D89).

---

## 0. Resumen en diez líneas

1. **Desplazar y acercar el lienzo repinta la escena entera en cada fotograma.** La capa congelada
   se invalida en cuanto la cámara se mueve (su `Estampa` compara la cámara con igualdad exacta), así
   que un paneo es «pintar todo lo visible» 60 veces por segundo. En el universo ni siquiera hay capa.
2. Dentro de ese «pintar todo», lo caro no es la tinta (ya va con realizaciones), sino **lo que se
   crea por fotograma**: una `ID2D1PathGeometry` por cada polilínea/polígono de las formas rough.js,
   un `IDWriteTextFormat` + `IDWriteTextLayout` por cada texto (dos por rótulo en el universo: medir
   y pintar), y un pincel degradado por galaxia.
3. Excalidraw, tldraw, Chromium y QuickView resuelven el paneo **sin volver a rasterizar**: mueven lo
   ya pintado (caché por elemento con `drawImage`, transformada CSS en el compositor, teselas en el
   compositor, transformada de DirectComposition).
4. La herramienta equivalente en Windows 10 + HD 4000 es **mover un visual de DirectComposition**
   (`SetOffsetX/Y`, `SetTransform`) con la escena en su propia superficie —de ser posible una
   `IDCompositionVirtualSurface` por teselas— y la interfaz en otro visual.
5. En la tinta, la mayor ganancia de latencia que queda en Windows 10 no es presentar antes, sino
   **empezar el fotograma más tarde** (justo antes del plazo de DWM) para que los puntos lleguen más
   frescos: hoy se pinta nada más dispararse la señal y el fotograma espera ~10–13 ms quieto.
6. La segunda: **tinta «húmeda» en un visual propio**, sin copiar ni hornear la escena al pulsar.
7. En el trazo, cada fotograma recalcula el contorno entero **dos veces** (el real y la copia con la
   punta predicha, que además clona el elemento). tldraw corta los trazos a 600 puntos; Xournal++ pinta
   solo el último tramo.
8. Para la fluidez natural sin «goma», el estado del arte abierto es el **Ink Stroke Modeler** de
   Google (Apache-2.0; hay port a Rust MIT/Apache): suavizado de temblor según velocidad, modelo de
   muelle y predictor de Kalman. El **filtro 1 €** es la alternativa mínima (≈30 líneas desde el
   artículo).
9. **Delegated Ink** (`IDCompositionInkTrailDevice`) es de Windows 11: no sirve en la máquina suelo.
   `DXGI_PRESENT_ALLOW_TEARING` no aplica a una swapchain de composición.
10. Antes de tocar nada hacen falta dos mediciones que aún no tenemos: el **fotograma de paneo fuera
    de pantalla** (banco como el del universo, `universo/sesion/medir.rs`, extendido al editor) y la
    **latencia entrada→fotón real** con PresentMon en el equipo del usuario, sin sintetizar entrada.

---

## 1. Cómo funciona HOY un fotograma del editor

Todo en `apps/pixpin/src/ventana_editor.rs::abrir_en_modo`, un único hilo (el de interfaz), un
`MotorRender` (D2D 1.1, fábrica de un hilo, contexto sin `ENABLE_MULTITHREADED_OPTIMIZATIONS`), una
`Superficie` = una swapchain de composición `FLIP_SEQUENTIAL`, 2 búferes, alfa premultiplicado, un
único visual DComp que ocupa toda la ventana.

### 1.1 La vuelta del bucle

| Paso | Qué pasa | Dónde |
|---|---|---|
| 1 | `bombear_pendientes` (PeekMessage acotado) y `tomar_eventos_pendientes`: el ratón llega como `RatonMovido` (con los puntos recuperados de `GetMouseMovePointsEx`) y el lápiz como `Muestra` (historial `WM_POINTER`, presión, subpíxel) | `pixpin-shell/src/overlay.rs:573`, `puntero.rs` |
| 2 | Cada evento pasa por el `Navegador` (rueda, espacio+arrastrar, botón central). Si cambia la cámara → `todo_sucio = true` + `ventana.invalidar()` (`InvalidateRect` de la ventana entera) | `ventana_editor.rs:750-770` |
| 3 | Si no, por el universo (si lo hay), la caja de herramientas y el `Gesto`. El gesto devuelve una región sucia en mundo, que se pasa a pantalla y se acumula en `sucio` | `ventana_editor.rs:1181-1210` |
| 4 | Al **pulsar** con una herramienta que cambia cada fotograma (trazo, arrastre), se **crea** un `ID2D1Bitmap1` del tamaño de la pantalla y se hornea en él toda la escena menos lo excluido (`CapaEstatica::preparar`). Esto ocurre dentro del vaciado de la cola | `ventana_editor.rs:1219-1279`, `capa_estatica.rs:92` |
| 5 | `hay_que_pintar` solo se pone con `EventoOverlay::Pintar`, es decir, cuando Windows genera `WM_PAINT` | `ventana_editor.rs:1318` |
| 6 | Si hay que pintar y la señal de la swapchain está disparada: predicción de la punta (horizonte 28 ms, tope 80 px), zona = sucio ∪ sucio anterior (+ holgura 88 px con predicción), `pintar(...)` | `ventana_editor.rs:1373-1478` |
| 7 | `pintar`: `GetBuffer(0)` + `CreateBitmapFromDxgiSurface` (cada fotograma), `volcar_zona` de la capa si su `Estampa` coincide, y encima lo excluido, la selección, la barra y el panel; `Present1(1, 0, rect_sucio)` | `ventana_editor.rs:1630-1869`, `superficie.rs:278,318` |
| 8 | `decidir_zoom`: si el zoom lleva 300/500 ms quieto, `cache_tinta.fijar_escala` (vacía TODAS las realizaciones) y suelta la capa | `ventana_editor.rs:1485-1499` |
| 9 | `esperar_eventos_o_senal`: `MsgWaitForMultipleObjectsEx` con la señal de latencia y un tope (zoom pendiente, forma rápida, animaciones del universo a 16 ms) | `ventana_editor.rs:1508-1540` |

### 1.2 Al desplazar o acercar

- La cámara cambia → `todo_sucio`. La `Estampa` de la capa lleva `(x, y, zoom)` y
  `sigue_valiendo` exige igualdad exacta (`capa_estatica.rs:52`), así que la capa **no vale** en
  ningún fotograma de un paneo. Y fuera de un gesto de dibujo ni siquiera existe.
- Resultado: cada fotograma limpia a blanco y pinta **todos** los candidatos de la rejilla. Por
  elemento: `cache.ordenes(e, zoom)` (cacheado por octava de zoom, bien), y luego por orden:
  - `Orden::Tinta` → `DrawGeometryRealization` (cacheada, barata) salvo tras un cambio de escala.
  - `Orden::Polilinea` / `Poligono` / `Relleno` (todas las formas rough.js: rectángulo, rombo,
    elipse, flecha, relleno de sombreado…) → `a_tuplas` (un `Vec` nuevo) + `CreatePathGeometry` +
    `Open`/`Close` + `DrawGeometry`/`FillGeometry` **en cada fotograma** (`lienzo.rs:637,835`).
    rough.js genera 2 pasadas por borde y decenas de líneas por relleno de sombreado: una sola forma
    con relleno puede ser 20–60 geometrías por fotograma.
  - `Orden::Texto` → `CreateTextFormat` + `CreateTextLayout` + `GetMetrics` por fotograma
    (`lienzo.rs:144-205`).
  - `Orden::Velo`, imágenes: bitmap ya subido, barato.
- Zoom con Ctrl+rueda: saltos discretos del 10 % sin animación (porte fiel de `handleWheel` de
  Excalidraw). Mientras el zoom no reposa, la tinta se pinta con la realización vieja estirada por
  la matriz (D122), pero **se sigue emitiendo cada primitiva** cada fotograma; y al reposar se
  **vacía la caché entera** y se re-teselan todos los trazos visibles en un solo fotograma (pico).
- En el pin ya existe `Superficie::estirar` (D89: transformada del visual, cero repintado); el
  editor no la usa.

### 1.3 En el universo

- `abrir_universo` es el mismo bucle con `Sesion` detrás; **no hay capa congelada** («se hornearía
  sobre blanco y sin astros», D238). Cada fotograma: `s.preparar` (qué se ve, cuadernos,
  miniaturas), `pintar_detras` (baldosas de estrellas de 512 px con paralaje), astros, conexiones y
  nebulosas en píxeles de pantalla, con **muchos textos** (`texto_centrado` = `medir_texto` +
  `texto`: dos layouts por rótulo), y `circulo_degradado` crea `GradientStopCollection` + pincel
  radial por galaxia y fotograma (`lienzo.rs:390`; su propio comentario dice «si la medición lo
  señala, se cachea»).
- El vuelo de cámara, los destellos y los pulsos se animan con un **temporizador de 16 ms**
  (`Sesion::tope_ms`), no con la señal de fotograma: `MsgWaitForMultipleObjectsEx` con 16 ms en un
  reloj de 15,6 ms despierta a 15 o 31 ms, y la animación sale a saltos irregulares aunque cada
  fotograma sea rápido.
- Hay un banco fuera de pantalla en curso (`universo/sesion/medir.rs` + `pixpin-render/src/
  fuera_de_pantalla.rs`, sin commit, de otro agente) que separa `preparar`, `encargar` y total con la
  GPU terminada. Es exactamente el arnés que falta para el editor (§7).

### 1.4 Al dibujar

- Pulsar: se crea y hornea la capa congelada (**~24 MB a 3000×2000**, 8 MB a 1080p, 4 MB a
  1366×768) con **toda la escena visible**: el primer fotograma del trazo paga una escena entera
  más una asignación de vídeo. En la HD 4000 con memoria compartida, esa asignación no es gratis.
- Cada fotograma: copia de la zona sucia de la capa (D148), y encima el trazo en curso: el gesto
  añade los puntos, `ordenes_a_distancia` recalcula **el contorno entero** de perfect-freehand
  (D112: `last = true`), `CreatePathGeometry`, `FillGeometry`. Con predicción, `con_punta` **clona el
  elemento entero** (todos los puntos y presiones) y se recalcula **otra vez** el contorno entero
  (`ventana_editor.rs:1756-1765`, `prediccion.rs:110`). El trazo real no se pinta en ese caso, pero
  su contorno sí queda en la caché de órdenes. Coste: 2× O(n) por fotograma y dos asignaciones de n.
- `Present1(1, …)` con el rectángulo sucio.
- La latencia medida por el propio proyecto: la punta iba ~30 ms detrás del cursor
  (`prediccion.rs:4-7`), y «presentar antes no lo arregló».

### 1.5 Dónde sospecho que está el coste (por orden)

| # | Sospecha | Síntoma que produce | Cómo confirmarlo |
|---|---|---|---|
| S4 | Paneo = escena entera por fotograma, con geometrías y layouts creados en cada uno | «Latencia en todas partes al mover», peor cuanto más dibujo | Banco fuera de pantalla: ms de `encargar` en paneo frente a cámara quieta, y contadores de objetos creados |
| S5 | Universo sin capa + textos + degradados por fotograma | Tirones en el universo aun con pocos astros | El banco de `medir.rs` ya separa `encargar`: ver cuánto es texto |
| S6 | Latencia de programación: se pinta al principio del intervalo de refresco y se espera | La punta ~30 ms detrás aunque pintar cueste 3 ms | Registrar `t(señal) → t(present)` y la hora estimada de composición (`IDCompositionDevice::GetFrameStatistics`) |
| S7 | Hornear la capa al pulsar | Primer tramo del trazo «tarde» o con salto | Medir `preparar` aparte (hoy cae dentro de `vaciar`) |
| S8 | Contorno ×2 y clonado por fotograma en trazos largos | Se nota más al final de un trazo largo | La puerta de E1 (5.000 puntos) ahora con predicción |
| S9 | Animaciones por temporizador y no por señal de fotograma | Vuelo de cámara irregular | Varianza del intervalo entre presentes (PresentMon `MsBetweenPresents`) |
| S10 | `CreateBitmapFromDxgiSurface` por fotograma | Coste fijo pequeño | Contador + tiempo en el banco |

---

## 2. Lo que hacen otros proyectos

### 2.1 Excalidraw (MIT)

- **Dos lienzos**: estático (los elementos) e interactivo (selección, tiradores, cursores
  remotos). El estático se re-renderiza con `renderStaticSceneThrottled` (agrupado a un
  `requestAnimationFrame`) y solo cuando cambia su «nonce» de escena o el viewport
  ([DeepWiki, pipeline de canvas](https://deepwiki.com/excalidraw/excalidraw/5.1-canvas-rendering-pipeline),
  [issue #10063](https://github.com/excalidraw/excalidraw/issues/10063)).
- **Caché por elemento en un canvas propio**: `elementWithCanvasCache` (un `WeakMap` elemento →
  canvas rasterizado). Se regenera solo si cambia la versión o el zoom, **y no por el zoom mientras
  `shouldCacheIgnoreZoom`** (el gesto de zoom en curso). Pintar la escena al desplazar es un
  `drawImage` por elemento visible, sin rasterizar nada
  ([`renderElement.ts`](https://github.com/excalidraw/excalidraw/blob/master/packages/element/src/renderElement.ts),
  líneas ~682-727).
- **Culling**: `Renderer.getVisibleCanvasElements` filtra con `isElementInViewport`, memoizado
  ([`Renderer.ts`](https://github.com/excalidraw/excalidraw/blob/master/packages/excalidraw/scene/Renderer.ts)).
- **Tinta**: 1 punto por fotograma y el trazo entero recalculado cada vez (ya lo superamos en
  entrada).
- Qué nos dice: nuestro D121 (realizaciones y no bitmaps) es correcto para la tinta, pero **las
  formas rough.js no tienen equivalente a la caché por elemento**; y durante el paneo Excalidraw
  nunca rasteriza.

### 2.2 tldraw (licencia propia «tldraw license», NO MIT desde la v2: solo ideas)

- Culling con índice espacial: lo que no se ve lleva `display: none`
  ([docs, Culling](https://tldraw.dev/sdk-features/culling),
  [Performance](https://tldraw.dev/sdk-features/performance)).
- **Zoom «eficiente»**: con más de 500 formas, `getEfficientZoomLevel` devuelve un valor estable
  durante el movimiento de cámara y solo se actualiza al pararse (lo mismo que nuestro D122).
- Desplazar/acercar = una transformada CSS en el contenedor: la hace el compositor del navegador
  sobre capas ya rasterizadas.
- **Trazos a mano cortados**: `DrawShapeUtil.options.maxPointsPerShape = 600`; al pasar de 600
  puntos el trazo continúa en una forma nueva (`Drawing.ts`, ~línea 702). Así el recálculo por
  fotograma de perfect-freehand queda acotado. Durante el trazo usa `last: isComplete` (falso en
  vivo).
- Nivel de detalle: a zoom bajo, trazos discontinuos se pintan continuos, rellenos de sombreado
  pasan a color liso.

### 2.3 Krita, MyPaint, Xournal++, Rnote, OpenToonz

| Proyecto | Licencia | Idea útil |
|---|---|---|
| Krita | GPL-3.0 | Lienzo por teselas con «proyección» y actualización solo de las teselas tocadas; estabilizador (retarda la punta a un «hilo» virtual) y suavizado ponderado por distancia/tiempo como opciones del usuario, no por defecto |
| MyPaint / libmypaint | ISC (libmypaint), GPL (app) | Superficie por teselas de 64 px; el pincel filtra la entrada con «slow tracking» (un paso bajo por pincel) configurable |
| Xournal++ | GPL-2.0 | `StrokeHandler` pinta **solo el último segmento** sobre una máscara al añadir un punto (coste O(1) por punto); al soltar, se re-renderiza el trazo entero ([discusión #4072](https://github.com/xournalpp/xournalpp/discussions/4072)) |
| Rnote (Rust) | GPL-3.0 | Suavizado con `ink-stroke-modeler-rs`; imágenes de trazo regeneradas **en hilos** solo para el viewport (`regenerate_rendering_in_viewport_threaded`) con R-tree ([PR #1462](https://github.com/flxzt/rnote/pull/1462)) |
| OpenToonz | BSD-3 (con partes de terceros) | Suavizado de trazo vectorial por media móvil de N puntos («Smooth» de la herramienta Brush) |

### 2.4 Chromium / Edge y Figma

- **Compositor con teselas e «impl-side painting»**: el hilo principal graba listas de pintado; la
  rasterización ocurre en teselas (≈256 px) en hilos de trabajo; el **desplazamiento lo hace el hilo
  del compositor moviendo capas ya rasterizadas**, sin esperar al hilo principal
  ([How cc works](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/how_cc_works.md),
  [Impl-side painting](https://www.chromium.org/developers/design-documents/impl-side-painting/)).
- **Predicción de entrada**: `ui/base/prediction` (BSD-3) tiene predictores lineales, mínimos
  cuadrados y Kalman, y además **remuestrea** la entrada a la hora del fotograma
  (`getPredictedEvents` en la web).
- **Delegated ink trails** (Edge/Chromium, WICG): el SO pinta la punta entre fotogramas de la app
  ([explicación WICG](https://github.com/WICG/ink-enhancement/blob/main/README.md)); Microsoft
  anunció hasta un 240 % de mejora en su prueba ([Windows Central](https://www.windowscentral.com/microsoft-improved-inking-way-240)).
  Llegó con Windows 11 (ver §3).
- Figma: renderizador propio en WebGL con teselas, y «no rasterizar durante la interacción» como
  principio ([Building a professional design tool on the web](https://www.figma.com/blog/building-a-professional-design-tool-on-the-web/)).

### 2.5 Latencia: cuánto importa

- Microsoft Research (Ng et al., UIST 2012): los sistemas táctiles tardan 50–200 ms; con un
  prototipo de ~1 ms, **los usuarios siguen notando mejoras muy por debajo de 10 ms**, y proponen
  una respuesta inmediata de baja calidad seguida de la de alta calidad
  ([ACM](https://dl.acm.org/doi/10.1145/2380116.2380174)). Es la idea «tinta húmeda / seca».
- Raph Levien, «Swapchains and frame pacing»: esperar al objeto de latencia está bien, pero lo
  óptimo es **empezar a pintar en `plazo − percentil 99,99 del tiempo de pintado`**, no nada más
  poder; si no, un equipo más rápido tiene más latencia porque su fotograma espera más
  ([blog](https://raphlinus.github.io/ui/graphics/gpu/2021/10/22/swapchain-frame-pacing.html)).
  Casey Muratori (Handmade Hero) defiende lo mismo: fijar el ritmo al refresco y medirlo.
- Referencia práctica de PresentMon: entrada→fotón < 30 ms excelente, 30–45 muy bien, 45–60 usable
  ([Tom's Hardware](https://www.tomshardware.com/pc-components/gpus/input-latency-is-the-all-too-frequently-missing-piece-of-framegen-enhanced-gaming-performance-analysis)).

---

## 3. Windows: qué API sirve en Windows 10 21H2 + HD 4000

La HD 4000 en Windows 10 se queda en controladores Intel 15.33 (WDDM 1.3, nivel de función 11_0).
Eso descarta todo lo que pida WDDM 2.x (planos de superposición, flip independiente garantizado).
**A comprobar en el equipo con `dxdiag`** antes de dar nada por hecho.

| Técnica | Problema que resuelve | Win10 21H2 + HD 4000 | Veredicto |
|---|---|---|---|
| `FRAME_LATENCY_WAITABLE_OBJECT` + `SetMaximumFrameLatency(1)` | Cola de 3 fotogramas | Sí | **Hecho** |
| Programar el inicio del fotograma con `IDCompositionDevice::GetFrameStatistics` (`nextEstimatedFrameTime`) | La entrada envejece mientras el fotograma pintado espera a DWM | Sí (DComp v1, Windows 8+) | **Recomendado (B1)** |
| `Present1` con rectángulos sucios | DWM recompone menos | Sí | **Hecho**; `pScrollRect` solo es una pista, no copia nada por nosotros |
| `DXGI_PRESENT_ALLOW_TEARING` | Latencia de vsync | Solo con sync 0, flip, y en la práctica solo con flip independiente; una swapchain de composición la compone DWM ([docs](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/variable-refresh-rate-displays)) | **No aplica** |
| Transformadas de visual DComp (`SetOffsetX/Y`, `SetTransform`) | Desplazar/escalar sin repintar | Sí (Windows 8+) | **Recomendado (A1, A2)**, ya usado en el pin (D89) |
| `IDCompositionAnimation` | Animar zoom/vuelo en el compositor, a la cadencia de DWM | Sí | Recomendado (A2, D2) |
| `IDCompositionVirtualSurface` (+ `Trim`, `Resize`) | Superficie enorme dispersa por teselas, solo se asigna lo pintado | Sí, Windows 8+ ([docs](https://learn.microsoft.com/en-us/windows/win32/api/dcomp/nn-dcomp-idcompositionvirtualsurface)); tamaño lógico hasta 2^24 px | Recomendado como fase 2 de A1 |
| `IDCompositionSurface::BeginDraw` con rectángulo | Actualizar solo un trozo; los cambios aparecen **atómicamente** con `Commit` | Sí | Útil para la tinta húmeda (B2) |
| Delegated Ink (`IDCompositionInkTrailDevice`) | El SO pinta la punta entre fotogramas | **No**: llegó con Windows 11 ([docs, sin req. publicado](https://learn.microsoft.com/en-us/windows/win32/api/dcomp/nn-dcomp-idcompositioninktraildevice); probado en Edge sobre Windows 11) | Solo como mejora opcional en Win11, más adelante |
| Windows Ink en escritorio (`IInkDesktopHost` / `InkPresenterDesktop`) | Tinta húmeda pintada por el sistema en su hilo, en nuestro árbol DComp | Sí, Windows 10 ([docs](https://learn.microsoft.com/en-us/windows/win32/api/inkpresenterdesktop/nn-inkpresenterdesktop-iinkdesktophost)) | **No**: su trazo no es perfect-freehand (se vería distinto al soltar), sirve sobre todo al lápiz |
| `EnableMouseInPointer` | Ratón como `WM_POINTER` | Sí | Descartado (D106), se mantiene |
| `GetPointerFrameInfoHistory` / `GetPointerPenInfoHistory` | Muestras fusionadas del lápiz/táctil | Sí | **Hecho** (lápiz); `POINTER_INFO.PerformanceCount` da la hora QPC de cada muestra: úsese para medir (§7) |
| DirectManipulation | Paneo/pinza de touchpad con inercia en el hilo del sistema | Sí | Opcional, después de A1 |

---

## 4. Direct2D en la HD 4000: lo que cuesta de verdad

De la guía oficial [Improving the performance of Direct2D apps](https://learn.microsoft.com/en-us/windows/win32/direct2d/improving-direct2d-performance):

- «Resource creation and deletion on hardware are expensive operations»: reutilizar pinceles,
  geometrías y bitmaps. Nosotros cacheamos pinceles sólidos (hasta 32), no geometrías de formas, ni
  formatos/layouts de texto, ni pinceles degradados.
- **Realizaciones de geometría**: «it is faster to convert them to geometry realizations and
  repeatedly draw the realizations than it is to repeatedly draw the geometries themselves». Existe
  `CreateStrokedGeometryRealization` para trazos con grosor y estilo: es lo que falta para rough.js.
- **Texto**: `DrawTextLayout` con un layout reutilizado, no crearlo cada vez; y fijar
  `D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE` («comparable to ClearType but much faster»). Hoy no se fija
  ningún modo.
- **Rayas**: «Rendering a dashed line is a very expensive operation»; un bitmap en mosaico es más
  barato (nuestras polilíneas discontinuas de rough.js y el marco de selección).
- **Recorte**: `PushAxisAlignedClip` mejor que capas; ya se usa para la zona sucia.
- `ENABLE_MULTITHREADED_OPTIMIZATIONS`: solo afecta a geometrías de trayecto (no a texto, bitmaps
  ni realizaciones) y sube el pico de memoria. Con un i3 de 2 núcleos/4 hilos, ganancia dudosa; solo
  tiene sentido si tras A3 siguen dominando geometrías sin realizar. **A medir, no a adoptar a ciegas.**
- `ID2D1CommandList`: graba y reproduce, pero no ahorra teselado en GPU; para la escena estática
  una lista grabada por tesela solo ahorra el recorrido en CPU. Menos útil que las realizaciones.
- Escena limitada por relleno o por vértices: la guía propone reducir el tamaño del destino y ver si
  el tiempo baja proporcionalmente. Es una prueba de 10 minutos con el banco (§7).

---

## 5. Suavizado y predicción sin «goma»

Hoy: `streamline` de perfect-freehand (0,5 ratón / 0,2 lápiz, D109), que es un paso bajo
exponencial de parámetro fijo `t = 0,15 + (1 − s)·0,85` sobre la posición. Un paso bajo de parámetro
fijo tiene **el mismo retraso a toda velocidad**: justo cuando la mano va rápido (donde el retraso más
se nota) la punta «tira de una goma». Y la predicción es lineal sobre una ventana de 50 ms.

| Técnica | Qué resuelve | Números / fuente | Coste aquí | Licencia |
|---|---|---|---|---|
| **Filtro 1 €** (Casiez, Roussel, Vogel, CHI 2012) | Temblor a baja velocidad sin retraso a alta: el corte del paso bajo sube con la velocidad (`mincutoff`, `beta`) | «less lag using a reference amount of jitter reduction» que los filtros comparados ([artículo](https://gery.casiez.net/publications/CHI2012-casiez.pdf), [repo](https://github.com/casiez/OneEuroFilter)) | Muy bajo: ~30 líneas puras en `pixpin-motor2d`, reimplementadas desde el artículo | Idea pública; el repo del autor no declara licencia en GitHub → se reescribe desde el artículo |
| **Ink Stroke Modeler** (Google) | Suavizado de temblor dependiente de la velocidad + remuestreo + **modelo de muelle-masa** para la posición (curvas naturales, no «goma») + fin de trazo + **predictor de Kalman** con curva de conexión | Es el modelador de tinta de las apps de Google ([docs](https://google.github.io/ink-stroke-modeler/), [C++](https://github.com/google/ink-stroke-modeler)) | Medio: dependencia pura de Rust ([`ink-stroke-modeler-rs`](https://github.com/flxzt/ink-stroke-modeler-rs), la usa Rnote) delante de perfect-freehand | Apache-2.0 (C++); MIT o Apache-2.0 (port Rust) → **se puede usar** |
| Predictores de Chromium (`ui/base/prediction`) | Lineal de 1.º/2.º orden, mínimos cuadrados, Kalman; remuestreo a la hora del fotograma | Base de `getPredictedEvents` | Medio | BSD-3 → se puede adaptar con aviso |
| `androidx.input:input-motionprediction` | Predictor Kalman de Android | — | Medio (Kotlin, portar) | Apache-2.0 |
| Lazy brush (estabilizador de «cuerda») | Precisión máxima a costa de retraso intencionado | [dulnan/lazy-brush](https://github.com/dulnan/lazy-brush) | Bajo | MIT → opción de usuario, **nunca por defecto** |
| Estabilizador de Krita / «slow tracking» de MyPaint | Igual, con más parámetros | — | — | GPL → solo ideas |

**Horizonte de la predicción.** Debe igualar la latencia real (entrada → fotón), no una constante:
con B1 la latencia baja y 28 ms predirían de más (la punta «se adelanta» y luego se retrae, que se
ve como un pelo). Propuesta: horizonte = mediana medida de `hora estimada de composición − hora de
la muestra`, con `GetFrameStatistics` de DComp, acotado y suavizado.

**Paridad con Excalidraw.** Las opciones anteriores cambian la geometría respecto al oráculo de E1.
Por eso van **delante** de perfect-freehand y **detrás de un ajuste** (`[tinta] suavizado =
"excalidraw" | "natural"`): con «excalidraw» los ficheros y el oráculo no cambian.

---

## 6. Lista priorizada de cambios

Valor/esfuerzo: ★★★ alto valor y bajo esfuerzo … ★ bajo. **[M]** = exige primero una medición que
aún no tenemos (§7). Rutas relativas a la raíz del repo.

### (A) Desplazar y acercar el lienzo y el universo sin tirones

**A1 · ★★★ · Cero objetos Direct2D/DirectWrite creados en un fotograma de paneo (cachés de
geometría, texto y degradados).** [M ligera]
- Qué: (1) generalizar `CacheTinta` a `CacheGeometria` con realizaciones **rellenas y trazadas**
  (`CreateStrokedGeometryRealization` con grosor y estilo) para `Poligono`, `Relleno`, `Polilinea` y
  `PolilineaDiscontinua`, con la misma clave `(id, versión, índice)`; (2) caché LRU de
  `IDWriteTextFormat` por `(familia, tamaño)` y de `IDWriteTextLayout` por `(texto, tamaño,
  ancho_max, tramos)` con tope (p. ej. 512), y que `medir_texto` + `texto` compartan el layout;
  (3) caché de `GradientStopCollection`/pincel radial por par de colores (el pincel se re-centra
  con `SetCenter`/`SetRadiusX/Y`); (4) `SetTextAntialiasMode(GRAYSCALE)` una vez en el contexto.
- Dónde: `crates/pixpin-render/src/tinta.rs` (generalizar), `lienzo.rs` (`poligono`, `polilinea*`,
  `disposicion*`, `circulo_degradado`), `motor.rs` (cachés), `apps/pixpin/src/ventana_editor.rs`
  (`dibujar_orden`), `apps/pixpin/src/universo/pintar.rs` (`texto_centrado`).
- Medir: contadores en `MotorRender` (geometrías, layouts, formatos, pinceles creados por
  fotograma) → objetivo **0** en un paneo sostenido; ms de `encargar` en el banco con 2.000 formas
  rough.js con relleno y 300 textos, y en el banco del universo.
- Riesgo: bajo. Memoria de realizaciones (triángulos) con miles de formas: tope y vaciado LRU; al
  perder el dispositivo se vacían como hoy. Una realización se hace a una escala: se regenera con
  la misma regla de D122.
- Por qué primero: es barato, no cambia la arquitectura, y beneficia al paneo, al zoom, al universo
  y a la capa congelada.

**A2 · ★★★ · Zoom por transformada de composición en el editor y el universo, con animación.**
- Qué: durante Ctrl+rueda / pinza, no repintar: `Superficie::estirar` (ya existe, D89) con la escala
  y el punto fijo del cursor, y el fotograma nítido cuando el zoom reposa (el `retardo` de D122, o
  150 ms como QuickView). Cada muesca anima la escala ~90–120 ms con curva cúbica de salida usando
  `IDCompositionAnimation` (la interpola DWM, la app no genera fotogramas). Con la escena y la
  interfaz en visuales separados (ver A3), la barra no se estira.
- Dónde: `crates/pixpin-render/src/superficie.rs` (animación), `apps/pixpin/src/ventana_editor.rs`
  (rama de `nav.accion` de zoom y `decidir_zoom`), `apps/pixpin/src/navegacion.rs`.
- Medir: fotogramas pintados por segundo durante 2 s de rueda (objetivo: ~0 hasta el reposo) y
  `MsBetweenDisplayChange` de PresentMon (DWM a 60 Hz constante).
- Riesgo: bajo-medio. Al alejar se ven bordes sin contenido hasta el fotograma nítido (QuickView lo
  acepta; se rellena con el color del lienzo). Sin A3, la barra de herramientas se estiraría con el
  lienzo: A2 completo depende de separar visuales (primer paso de A3).

**A3 · ★★ · Desplazar moviendo el visual de la escena (compositor), no repintándola.** [M]
- Qué: tres visuales DComp hijos del raíz: **escena** (mundo), **tinta húmeda/selección** (B2) e
  **interfaz** (barra, panel, ruta del universo). Durante un paneo, la escena solo cambia
  `SetOffsetX/Y` + `Commit` (coste CPU/GPU de la app ≈ 0; DWM compone lo que ya iba a componer).
  - Fase 1 (media): superficie de escena del tamaño de la ventana + margen fijo (p. ej. 1 tesela de
    512 px por lado); se repinta al parar el paneo o al agotar el margen.
  - Fase 2 (alta): `IDCompositionVirtualSurface` con teselas de 512 px (como Chromium/MyPaint):
    `BeginDraw(rect)` solo en teselas que entran, `Trim` a viewport + 1 tesela, re-base del origen
    cuando el desplazamiento acumulado se acerca a 2^24 px.
  - En el universo, las baldosas de estrellas en **su propio visual con desplazamiento
    proporcional**: el paralaje sale gratis del compositor.
- Dónde: `crates/pixpin-render/src/superficie.rs` (nuevo `SuperficieEscena` o visuales hijos),
  `apps/pixpin/src/ventana_editor.rs` (bucle y `pintar` partido en escena/interfaz),
  `apps/pixpin/src/universo/pintar.rs` y `estrellas.rs`, `crates/pixpin-render/src/capa_estatica.rs`
  (deja de ser necesaria para arrastres de la cámara).
- Medir: banco (paneo de 120 fotogramas): ms de CPU y GPU por fotograma de paneo antes/después;
  PresentMon en el equipo del usuario: `MsAllInputToPhotonLatency` y fotogramas perdidos al panear.
- Riesgo: medio-alto. Coordinar escena e interfaz entre dos presentes (una swapchain y una
  superficie DComp no se publican atómicamente; si ambas son superficies DComp, sí con un `Commit`).
  Memoria: fase 1 a 3000×2000 + margen ≈ 49 MB frente a los 48 MB de hoy (2 búferes); a 1366×768
  ≈ 10 MB. Todo el cálculo de coordenadas ratón→mundo sigue en la cámara, no en el visual.

**A4 · ★★ · Capa del mundo que sobrevive a la cámara (si A3 no se hace pronto).**
- Qué: la `Estampa` guarda el zoom y un desplazamiento **entero** en píxeles de dispositivo; un paneo
  redondea la cámara a píxel entero mientras dura y pinta la capa con `DrawBitmap` desplazado +
  solo las franjas nuevas (recorte), en lugar de invalidarla. Es la versión «sin compositor» de A3.
- Dónde: `capa_estatica.rs` (`sigue_valiendo` con traslación), `ventana_editor.rs`.
- Medir: igual que A3.
- Riesgo: medio. Cuesta una copia de pantalla completa por fotograma, y el proyecto ya midió 6–8 ms
  por esa copia en el equipo del usuario (D148): **puede no ganar nada en la HD 4000**. Por eso A3
  va por delante; A4 solo si la medición de A1 deja el paneo limitado por relleno y A3 se retrasa.

### (B) Latencia de la tinta (del contacto al píxel)

**B1 · ★★★ · Empezar el fotograma tarde: pintar justo antes del plazo de DWM.** [M]
- Qué: hoy, en cuanto la señal de latencia se dispara (≈ tras la composición anterior), se vacía la
  cola, se pinta en ~3 ms y el fotograma espera ~10–13 ms la siguiente composición: los puntos
  llegan con esa edad. Con `IDCompositionDevice::GetFrameStatistics` (`nextEstimatedFrameTime`,
  `currentCompositionRate`) y la duración de pintado medida (p99 de las últimas 120), dormir con
  `MsgWaitForMultipleObjectsEx` (sin dejar de recibir mensajes) hasta `plazo − p99 − margen` y solo
  entonces vaciar y pintar (Raph Levien, §2.5). Requiere `timeBeginPeriod(1)` solo mientras se dibuja
  o un temporizador de espera de alta resolución (`CREATE_WAITABLE_TIMER_HIGH_RESOLUTION`, Win10
  1803+), y quitarlo en reposo para no gastar batería.
- Dónde: `apps/pixpin/src/ventana_editor.rs` (bloque 1361-1540), `crates/pixpin-render/src/
  superficie.rs` (exponer las estadísticas de DComp), `crates/pixpin-shell/src/overlay.rs`
  (`esperar_eventos_o_senal` con hora límite).
- Medir: latencia interna por punto = `hora de presentación estimada − hora de la muestra`
  (`POINTER_INFO.PerformanceCount` en lápiz, tiempo de `MOUSEMOVEPOINT` en ratón) y
  `MsAllInputToPhotonLatency` de PresentMon. Objetivo: −8 a −12 ms de mediana sin subir los
  fotogramas perdidos.
- Riesgo: medio. Si se calcula mal el p99, se pierde el refresco y se ve un tirón: margen
  adaptativo que crece al fallar y baja despacio. Solo mientras hay gesto; en reposo nada cambia.

**B2 · ★★ · Tinta húmeda en su propio visual; nada de hornear la escena al pulsar.**
- Qué: el trazo en curso (y su punta predicha) se pinta en una superficie DComp transparente encima
  de la escena (la «wet ink» de Windows Ink; Ng et al.); la escena no se toca durante el trazo, así
  que desaparecen `CapaEstatica::preparar` (asignar ~24 MB + pintar toda la escena en el primer
  fotograma del trazo) y la copia por fotograma de D148. Al soltar: la escena pinta el trazo seco y,
  **en el mismo `Commit`** (si ambas son superficies DComp), la húmeda se vacía; si la escena sigue
  siendo swapchain, la húmeda se mantiene un fotograma más para no parpadear.
- Dónde: `crates/pixpin-render/src/superficie.rs`, `apps/pixpin/src/ventana_editor.rs` (`pintar`,
  bloque de la capa en 1211-1282), `capa_estatica.rs` (queda para arrastrar la selección, o se
  retira con A3).
- Medir: tiempo del primer fotograma tras pulsar (hoy incluye `preparar`), ms de pintar por
  fotograma durante el trazo con 1.000 elementos en escena (puerta de E1: ≤ 3 ms).
- Riesgo: medio. Opacidad < 1 o resaltador: la húmeda y la seca a la vez oscurecen un fotograma →
  pintar la húmeda con la opacidad del elemento en una capa y no solapar. Comparte trabajo con A3
  (visuales separados): hacerlos juntos.

**B3 · ★★ · Contorno incremental y predicción sin clonar.**
- Qué: (1) la punta predicha se pinta como **un segmento extra** calculado a partir de la cola (los
  últimos ~32 puntos + el predicho), no clonando el elemento y recalculando todo; (2) congelar el
  trazo en tramos: cada 128 puntos estables, el contorno de ese tramo se realiza una vez y se
  reutiliza (con solape de unos puntos para que la unión no se vea); solo la cola se recalcula (la
  «S3» del spec del 2026-09-13, §4). Alternativa más simple, a lo tldraw: cortar el trazo en otro
  elemento a los 600 puntos.
- Dónde: `crates/pixpin-motor2d/src/tinta/freehand.rs`, `tinta/prediccion.rs` (`con_punta`),
  `crates/pixpin-motor2d/src/pintado.rs`, `apps/pixpin/src/ventana_editor.rs:1756-1765`.
- Medir: puerta ya definida (5.000 puntos < 0,5 ms por fotograma) **con la predicción encendida**;
  oráculo: el contorno por tramos coincide con el entero (tolerancia 0,01) salvo en las uniones, que
  se comprueban aparte.
- Riesgo: medio. perfect-freehand no es local (presión inicial media de 10 puntos, tapas, `last`):
  los tramos congelados deben excluir inicio y fin. El corte a 600 puntos rompe la paridad del
  fichero con Excalidraw (un trazo pasa a ser varios).

**B4 · ★ · Pintar sin pasar por `WM_PAINT`; y estado del backbuffer cacheado.**
- Qué: `hay_que_pintar` se pone directamente cuando algo se ensucia (hoy `invalidar()` →
  `InvalidateRect` → `WM_PAINT`, que Windows solo genera con la cola vacía: la «S2» del spec). Y
  cachear los dos `ID2D1Bitmap1` del backbuffer por puntero de textura (se sueltan en
  `ResizeBuffers`) en vez de `CreateBitmapFromDxgiSurface` en cada fotograma.
- Dónde: `ventana_editor.rs` (todas las llamadas a `ventana.invalidar()` del bucle), `superficie.rs`
  (`empezar`), `motor.rs`.
- Medir: D129 (`puntos` y fotogramas por vuelta) y el contador de bitmaps creados.
- Riesgo: bajo. Hay que seguir validando la ventana para que Windows no repita `WM_PAINT`.

### (C) Fluidez natural del trazo

**C1 · ★★ · Suavizado que se adapta a la velocidad (1 € o Ink Stroke Modeler) delante de
perfect-freehand, con `streamline` más bajo.** [M: prueba del usuario]
- Qué: modo «natural» opcional: la entrada pasa por un filtro 1 € por eje (con la hora real de cada
  muestra, que ya tenemos) o por `ink-stroke-modeler-rs` (suavizado de temblor + muelle-masa +
  remuestreo), y `streamline` baja a ~0,1–0,2 para el ratón. El modo «excalidraw» queda igual y es el
  del oráculo.
- Dónde: nuevo `crates/pixpin-motor2d/src/tinta/suavizado.rs` (puro), `tinta/mod.rs` (opciones),
  `apps/pixpin/src/ventana_editor.rs` (donde el gesto recibe `Mover`), `pixpinmax.toml`.
- Medir (sin GPU): con trazos grabados (fichero de muestras con tiempo, ver §7), el **retraso medio**
  entre la posición filtrada y la cruda a alta velocidad y el **temblor** (energía de alta
  frecuencia) a baja velocidad; objetivo: menos retraso que `streamline 0,5` con igual o menos
  temblor. Y la prueba a ciegas del usuario A/B.
- Riesgo: bajo en código, alto en gusto: por eso detrás de un ajuste. Licencias: 1 € reescrito del
  artículo; el modelador, MIT/Apache.

**C2 · ★★ · Predicción de Kalman con horizonte medido.** [M]
- Qué: sustituir el extrapolador lineal por un Kalman de posición/velocidad/aceleración (el del Ink
  Stroke Modeler, o un porte del de Chromium, BSD-3) y fijar el horizonte a la latencia medida
  (§5), no 28 ms fijos. Mantener las protecciones actuales (tope, apagado al frenar o al girar).
- Dónde: `crates/pixpin-motor2d/src/tinta/prediccion.rs`, `ventana_editor.rs` (constantes
  `HORIZONTE_PREDICCION_MS`, `TOPE_PREDICCION_PX`).
- Medir: con trazos grabados, error medio entre la punta predicha y la posición real a `+horizonte`
  (sin GPU, puro), en rectas, curvas y zigzag; comparar lineal frente a Kalman.
- Riesgo: bajo-medio: una predicción que sobrepasa y se retrae se ve peor que ninguna; el tope sigue.

**C3 · ★ · Movimiento de cámara natural: inercia opcional y paneo pegado al cursor.**
- Qué: el paneo con arrastre debe seguir al cursor 1:1 sin suavizar (ya lo hace); al soltar, inercia
  opcional con decaimiento exponencial (~325 ms, como el desplazamiento cinético de iOS/Android),
  animada con el compositor (A3) o con la señal de fotograma. Pinza de touchpad con
  DirectManipulation más adelante.
- Dónde: `apps/pixpin/src/navegacion.rs` (pura), `ventana_editor.rs`.
- Medir: prueba del usuario; en el banco, que la inercia no pinte más que A3.
- Riesgo: bajo; Excalidraw no tiene inercia: ajuste, apagado por defecto en el editor, encendido en
  el universo si el usuario lo prefiere.

### (D) Ritmo de fotogramas y CPU en reposo

**D1 · ★★★ · Animaciones del universo al ritmo de la señal de fotograma, no de un temporizador de
16 ms.**
- Qué: mientras `vuelo`, destellos o pulsos estén activos, esperar la señal de la swapchain (ya se
  tiene) en vez de `tope_ms = 16`, y calcular `t` con la **hora estimada de presentación**
  (`GetFrameStatistics`), no con `Instant::now()` al pintar. El vuelo de cámara, además, es candidato
  a `IDCompositionAnimation` de la transformada del visual de la escena (A3) con un solo repintado
  al final.
- Dónde: `apps/pixpin/src/universo/sesion.rs` (`tick`, `tope_ms`), `ventana_editor.rs` (espera).
- Medir: PresentMon `MsBetweenPresents` durante un vuelo: desviación estándar objetivo < 1 ms a
  60 Hz (hoy se esperan saltos de 15/31 ms).
- Riesgo: bajo. Si la ventana está tapada la señal no llega: ya existe `ESPERA_MAXIMA_SENAL`.

**D2 · ★★ · Presupuesto por fotograma visible y reposo comprobado.**
- Qué: ampliar D129 con: ms de `preparar` la capa aparte de `vaciar`, elementos pintados, objetos
  D2D/DWrite creados (A1), realizaciones rehechas, y los retrasos de composición de DComp
  (`GetFrameStatistics`); y una prueba de reposo: con el editor y el universo abiertos y quietos,
  0 fotogramas y 0 despertares por segundo durante 10 s (hoy el universo despierta a 16 ms con
  animaciones y en `guardar_en`).
- Dónde: `apps/pixpin/src/medir_fotogramas.rs`, `ventana_editor.rs`, `motor.rs` (contadores con
  `Cell<u32>`, sin coste con la medición apagada).
- Medir: es la medición.
- Riesgo: ninguno.

**D3 · ★ · Rehacer la tinta nítida sin pico tras un zoom.**
- Qué: `fijar_escala` vacía todas las realizaciones y el primer fotograma nítido re-tesela todo lo
  visible. Rehacer por tandas con un tope de ms por fotograma (primero lo más grande en pantalla),
  pintando lo aún no rehecho con la realización vieja estirada; o conservar 2 escalas (la actual y
  la anterior) para un zoom de ida y vuelta.
- Dónde: `crates/pixpin-render/src/tinta.rs`, `ventana_editor.rs:1485-1499`.
- Medir: el fotograma más largo tras soltar Ctrl+rueda con 2.000 trazos visibles.
- Riesgo: bajo; memoria de dos escalas: tope.

### Orden sugerido

| Orden | Cambio | Por qué en ese orden |
|---|---|---|
| 0 | §7 arnés + D2 | Sin números no se sabe qué sirvió |
| 1 | A1 | Barato; ataca S4 y S5 directamente |
| 2 | D1, B4 | Baratos; ritmo y S2 |
| 3 | B1 | La mayor ganancia de latencia disponible en Win10 |
| 4 | A2 + A3 fase 1 + B2 | Comparten los visuales separados; es el cambio de arquitectura |
| 5 | B3, C2, C1 | Calidad del trazo, con trazos grabados para comparar |
| 6 | A3 fase 2, D3, C3 | Refinamiento |

---

## 7. El arnés de medición que falta

Regla del proyecto: el agente no usa la PC del usuario ni sintetiza su entrada. Tres piezas:

### 7.1 Banco fuera de pantalla del editor (en CI local, con GPU, `#[ignore]`)

Extender lo que ya se está haciendo para el universo (`FueraDePantalla` + `medir.rs`, que separa
CPU de encargo y total con la GPU terminada vía `D3D11_QUERY_EVENT`):

- **Escenas generadas** con semilla: 2.000 trazos a mano, 500 formas rough.js con relleno de
  sombreado, 200 flechas, 300 textos, 5 imágenes; y la escena real de un `.pixpin2d` copiado si se
  pasa por variable de entorno (como `PIXPIN_UNIVERSO_REAL`).
- **Guiones de cámara**: quieta ×60, paneo horizontal de 120 fotogramas a 20 px/fotograma, zoom de
  0,5 a 2 en 60 pasos, y el reposo tras el zoom (el pico de D3).
- **Trazos grabados**: un fichero de muestras reales `(x, y, presión, t_ms)` (los guarda el propio
  editor con `[rendimiento] grabar_trazos = true`, apagado por defecto; el usuario lo pasa igual que
  el registro de D129). El banco los reproduce **llamando al `Gesto` directamente**, a la hora de
  cada muestra, sin tocar la cola de mensajes de Windows: no es entrada sintetizada del sistema.
- **Salida**: por fase, media, p95, p99 y máximo de CPU y de GPU; objetos D2D/DWrite creados por
  fotograma; bytes de vídeo asignados. Una línea por guion, comparable entre commits.
- Tamaños: 1366×768 al 100 % (portátil suelo típico) y 3000×2000 al 150 % (equipo del usuario).
- Prueba de «relleno o vértices» (§4): mismo guion a la mitad de resolución.

### 7.2 Pruebas puras (sin GPU, en la CI normal)

- Filtros y predictores (C1, C2) sobre los trazos grabados: retraso, temblor, error de predicción.
- Contorno por tramos contra el entero (B3), con el oráculo de E1.
- Planificador de B1: dado un historial de duraciones y horas de composición, decide la hora de
  empezar y el margen adaptativo (función pura, como `decidir_zoom`).

### 7.3 En el equipo del usuario (sin sintetizar nada)

- **PresentMon** (Intel, [MIT](https://github.com/GameTechDev/PresentMon)): consola con
  `--process_name pixpinmax.exe`; columnas `PresentMode` (confirma «Composed: Flip» y descarta
  sorpresas), `MsBetweenPresents`, `MsBetweenDisplayChange` y **`MsAllInputToPhotonLatency`** (desde
  la primera entrada de teclado/ratón que contribuyó al fotograma hasta que se vio,
  [README](https://raw.githubusercontent.com/GameTechDev/PresentMon/main/README-ConsoleApplication.md)).
  Necesita permisos de administrador o el grupo «Performance Log Users». El usuario dibuja y panea
  10 s como en D130; pasa el CSV.
- **D129 ampliado** (D2): latencia interna por punto con la hora de la muestra
  (`POINTER_INFO.PerformanceCount` / tiempo de `MOUSEMOVEPOINT`) hasta la hora estimada de
  composición.
- La prueba visual de siempre: la cámara lenta del móvil (240 fps) grabando cursor y punta, que da la
  latencia total con el ratón incluido; opcional.

---

## 8. Licencias: qué se puede tomar de cada fuente

PixPin Max es MIT y `cargo-deny` rechaza GPL/AGPL.

| Fuente | Licencia (comprobada con la API de GitHub el 2026-09-19) | Uso permitido |
|---|---|---|
| excalidraw/excalidraw | MIT | Código (ya portado; aviso en `THIRD-PARTY-NOTICES.md`) |
| steveruizok/perfect-freehand | MIT | Código (ya portado) |
| google/ink-stroke-modeler | Apache-2.0 | Código, con aviso y NOTICE |
| flxzt/ink-stroke-modeler-rs | MIT o Apache-2.0 (carpeta `LICENSES`) | Dependencia o código |
| Chromium (`ui/base/prediction`, cc) | BSD-3-Clause | Código adaptado, con aviso |
| androidx `input-motionprediction` | Apache-2.0 | Código adaptado, con aviso |
| dulnan/lazy-brush | MIT | Código |
| GameTechDev/PresentMon | MIT | Herramienta (no se enlaza) |
| mypaint/libmypaint | ISC | Código (permisiva); la app MyPaint es GPL → solo ideas |
| casiez/OneEuroFilter | Sin licencia declarada en el repo | **Reescribir desde el artículo** (el algoritmo es público) |
| tldraw/tldraw | «tldraw license» (propietaria, código visible) | **Solo ideas**, ni una línea |
| KDE/krita | GPL-3.0 | **Solo ideas** |
| xournalpp/xournalpp | GPL-2.0 | **Solo ideas** |
| flxzt/rnote | GPL-3.0 | **Solo ideas** (su dependencia `ink-stroke-modeler-rs` sí es permisiva) |
| opentoonz/opentoonz | BSD-3 con componentes de terceros | Ideas; comprobar fichero a fichero antes de copiar |
| QuickView, obsidian-excalidraw-plugin | GPL-3.0 / AGPL-3.0 | Solo ideas (ya decidido, 2026-09-03) |

---

## 9. Fuentes

- Excalidraw: [renderElement.ts](https://github.com/excalidraw/excalidraw/blob/master/packages/element/src/renderElement.ts),
  [Renderer.ts](https://github.com/excalidraw/excalidraw/blob/master/packages/excalidraw/scene/Renderer.ts),
  [DeepWiki: canvas rendering pipeline](https://deepwiki.com/excalidraw/excalidraw/5.1-canvas-rendering-pipeline),
  [issue #10063](https://github.com/excalidraw/excalidraw/issues/10063),
  [issue #4036 (GC de la caché de canvas)](https://github.com/excalidraw/excalidraw/issues/4036).
- tldraw: [Performance](https://tldraw.dev/sdk-features/performance), [Culling](https://tldraw.dev/sdk-features/culling),
  `packages/tldraw/src/lib/shapes/draw/toolStates/Drawing.ts` y `DrawShapeUtil.tsx`
  (`maxPointsPerShape: 600`) en [tldraw/tldraw](https://github.com/tldraw/tldraw).
- Xournal++: [discusión #4072](https://github.com/xournalpp/xournalpp/discussions/4072).
- Rnote: [PR #1462](https://github.com/flxzt/rnote/pull/1462).
- Ink Stroke Modeler: [docs](https://google.github.io/ink-stroke-modeler/), [C++](https://github.com/google/ink-stroke-modeler),
  [Rust](https://github.com/flxzt/ink-stroke-modeler-rs).
- 1 €: [artículo CHI 2012](https://gery.casiez.net/publications/CHI2012-casiez.pdf), [repo](https://github.com/casiez/OneEuroFilter).
- Lazy brush: [dulnan/lazy-brush](https://github.com/dulnan/lazy-brush).
- Chromium: [How cc works](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/how_cc_works.md),
  [Impl-side painting](https://www.chromium.org/developers/design-documents/impl-side-painting/).
- Delegated ink: [WICG ink-enhancement](https://github.com/WICG/ink-enhancement/blob/main/README.md),
  [IDCompositionInkTrailDevice](https://learn.microsoft.com/en-us/windows/win32/api/dcomp/nn-dcomp-idcompositioninktraildevice),
  [Windows Central, 240 %](https://www.windowscentral.com/microsoft-improved-inking-way-240).
- Windows Ink de escritorio: [IInkDesktopHost](https://learn.microsoft.com/en-us/windows/win32/api/inkpresenterdesktop/nn-inkpresenterdesktop-iinkdesktophost).
- DirectComposition: [IDCompositionVirtualSurface](https://learn.microsoft.com/en-us/windows/win32/api/dcomp/nn-dcomp-idcompositionvirtualsurface).
- DXGI: [Variable refresh rate displays / tearing](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/variable-refresh-rate-displays).
- Direct2D: [Improving the performance of Direct2D apps](https://learn.microsoft.com/en-us/windows/win32/direct2d/improving-direct2d-performance).
- Latencia: [Ng et al., UIST 2012](https://dl.acm.org/doi/10.1145/2380116.2380174),
  [Raph Levien, Swapchains and frame pacing](https://raphlinus.github.io/ui/graphics/gpu/2021/10/22/swapchain-frame-pacing.html),
  [Figma, Building a professional design tool on the web](https://www.figma.com/blog/building-a-professional-design-tool-on-the-web/),
  [Tom's Hardware sobre input-to-photon](https://www.tomshardware.com/pc-components/gpus/input-latency-is-the-all-too-frequently-missing-piece-of-framegen-enhanced-gaming-performance-analysis).
- PresentMon: [repo (MIT)](https://github.com/GameTechDev/PresentMon),
  [README de consola](https://raw.githubusercontent.com/GameTechDev/PresentMon/main/README-ConsoleApplication.md).
- Documentos propios: `docs/superpowers/specs/2026-09-13-e1-tinta-excalidraw-design.md`,
  `docs/superpowers/specs/2026-09-13-lienzo-desde-pin-design.md`,
  `docs/investigacion/2026-09-03-quickview-y-excalidraw-guia.md`.

### Pendiente de verificar (no se afirma como hecho)

- Versión exacta del controlador y WDDM de la HD 4000 del equipo suelo (`dxdiag`).
- Que `IDCompositionInkTrailDevice` no exista en `dcomp.dll` de Windows 10 21H2 (la documentación no
  publica el requisito; todo lo publicado lo sitúa en Windows 11).
- El reparto real del coste del paneo entre geometrías, texto y relleno: es lo primero que da el
  banco de §7.1.
