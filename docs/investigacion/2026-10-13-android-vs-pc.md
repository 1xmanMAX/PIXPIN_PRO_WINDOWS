# Android contra PC: lo que Android tiene y el PC no (13-oct-2026)

Pedido del usuario: que PixPin Max (PC, este repositorio) tenga **todas** las funciones de PixPin
Android, siguiendo la lógica de Android. Este documento es el inventario de lo que falta, lo que
está a medias y lo que está hecho de otra forma, y una propuesta de tandas para repartirlo entre
agentes en paralelo.

**Fuentes.** Android leído con `gh api` en su `main` actual (`44c3a772`, v0.122.0): el árbol
entero (693 ficheros `.kt`), `README.md`, `docs/guia.md`, `docs/compatibilidad-pc-9-oct.md`,
`docs/calidad-de-las-fotos.md`, los 300 últimos commits (cuerpos de v0.86 a v0.122) y las
cabeceras de los ficheros que no se conocían. PC: `main` (`4234e37`), `apps/pixpin/src`,
`crates/*` y `docs/investigacion/*` (sobre todo `2026-09-22-lista-de-implementacion.md`, que
ya cerró casi todo hasta la v0.85, y `2026-10-12-que-se-sincroniza.md`).

**Fuera de este inventario:** los **chats compartidos** con otras personas (v0.122, los lleva
otro agente) y lo que el PC ya hace y Android copió del PC (tareas v3, lecciones v2, galería,
Office maquetado, visor de planos y 3D, sincronizar sola…). Lo que solo tiene sentido en un
teléfono va al final, con su equivalente en Windows si lo hay.

## Resumen

El PC está **a la par en casi todo**: chat, proyectos, tareas, capturas y galería, lecciones,
mini-apps, notas, tablas, Office, planos CAD (regla con perpendicular, membrete, SHX de
Hershey, anotar con el motor), visor 3D, compartir (también el enlace temporal) y
sincronización. Lo que falta se concentra en cinco sitios:

1. **Lectores**: la página que sube sola, el PDF «como texto», las voces de Microsoft y que la
   voz siga sonando al cerrar el lector.
2. **Lienzo**: el grafito con sus siete durezas, los instrumentos de graficar (plano
   cartesiano, recta numérica, espacio de tres ejes), la tabla de coordenadas, el modo guía y
   los sólidos isométricos. Todo eso **ya viaja intacto** al PC, pero el PC no lo dibuja.
3. **Croquis 3D**: falta entero. El PC lo sincroniza sin abrirlo y, si se pinea, avisa
   «Todavía no se puede ver aquí» (`apps/pixpin/src/pines.rs:2489`).
4. **Pines y portapapeles**: el pin de color, las pegatinas, la nota paginada y las palabras
   mágicas propias.
5. **Detalles sueltos**: lecciones (fotos y audio en una ya guardada), letras que faltan en un
   plano, «Ponerme al día con este lienzo», el peso del proyecto, el lápiz y el dedo en
   pantallas táctiles, y la Jump List como los atajos del icono del móvil.

## 1. La tabla

Columnas: **Tam.** = tamaño estimado (S = un día, M = dos a cuatro días, L = una semana o
más) · **Sinc.** = si toca lo que viaja entre aparatos · **Prio.** = cuánto lo usaría el
usuario (alta = chat, proyectos, lienzo, lectores, planos; media = hojas, lecciones,
capturas; baja = lo que se usa de vez en cuando).

### 1.1 Lectores (Word, EPUB, PDF)

| # | Función | Android (fichero, versión) | Estado en el PC | Tam. | Sinc. | Prio. |
|---|---|---|---|---|---|---|
| L1 | **Página que sube sola** por palabras por minuto (80–700, de 20 en 20, se recuerda), con «Terminas en 12 min · a las 18:40»; se para al anotar, presentar o escuchar | `motor/AutoDesplazar.kt` (cuentas, `AutoDesplazarTest`), `ui/MandosDeLector.kt` (barra), `ui/VisorHtmlActivity.kt`, `pdf/LectorPdfActivity.kt` · v0.88 (`98a50b9`), en el PDF v0.92 (`800db67`) | **Falta.** Ni en `apps/pixpin/src/visor.rs` ni en `lector_pdf.rs`; solo existe en la página web exportada (`crates/pixpin-motor2d/src/exportar_html/documento.rs`) | S | No | Alta |
| L2 | **PDF «Ver como texto»**: el texto del PDF en líneas y párrafos (guiones, títulos, números de página fuera, **dos columnas en su orden**) y abierto en el lector de Word, para leerlo, escucharlo, cambiar la letra y anotarlo | `motor/PdfAHtml.kt` (`PdfAHtmlTest`), `pdf/LectorPdfActivity.kt` · v0.92 (`800db67`), dos columnas v0.93 (`a6153f8`) | **Falta.** El PC ya saca el texto de cada página para buscar (`pixpin_pdf::texto::PaginaDeTexto`) y tiene el lector de Word (`visor.rs`, `pixpin-docs`); falta juntarlo en párrafos y abrirlo así | M | No | Alta |
| L3 | **Voces de Microsoft** (las neuronales de «Leer en voz alta» de Edge, 300+, gratis, en línea), elegir voz por idioma, caché de MP3; si fallan, sigue la voz local | `motor/EdgeVoz.kt` (protocolo de edge-tts, `EdgeVozTest`, `EdgeVozEnVivoTest`), `ui/VozDeEdge.kt`, `ui/LectorEnVoz.kt` · v0.90 (`6e97552`) | **Falta, y descartado en su día** (`apps/pixpin/src/leer_en_voz.rs`: «no hay voces de Microsoft Edge en línea»). El PC lee con SAPI. **Sin crates nuevos:** WebSocket de WinRT (`Windows.Networking.Sockets.MessageWebSocket`) y el MP3 por Media Foundation, que ya usa `pixpin-audio` | M | No | Media |
| L4 | **La voz sigue al cerrar el lector**, con mandos de reproductor (pausa, párrafo anterior/siguiente) y el marcador verde al día | `ui/LeyendoEnVozService.kt`, `ui/LectorEnVoz.kt` (`delApp`) · v0.89 (`1e66199`) | **Falta**: la lectura vive mientras el lector está abierto (`leer_en_voz.rs`). **Equivalente en Windows:** `SystemMediaTransportControls` (WinRT, con `ISystemMediaTransportControlsInterop` sobre la ventana de mensajes) y las teclas multimedia del teclado | M | No | Media |
| L5 | **Con lápiz, el dedo pasa las hojas** (botón «Dedo: mover / dibujar»), en lectores y planos | `motor/DrawCanvas.kt` (`dedoAlLector`), `pdf/LectorPdfActivity.kt`, `ui/VisorHtmlActivity.kt`, `planos/PlanoActivity.kt` · v0.121 (`60e3208`) | **Falta en pantallas táctiles**: el PC solo distingue el lápiz para leer su presión (`crates/pixpin-shell/src/puntero.rs`); un dedo en la pantalla llega como ratón y dibuja. Equivalente: `WM_POINTER` con `PT_TOUCH`/`PT_PEN` en la ventana del anfitrión de la tinta. Sin pantalla táctil no aplica | M | No | Baja |

### 1.2 Lienzo (y todo lo que usa su motor: hojita, lectores, planos)

| # | Función | Android (fichero, versión) | Estado en el PC | Tam. | Sinc. | Prio. |
|---|---|---|---|---|---|---|
| D1 | **Grafito libre con siete durezas** (4H, 2H, HB, 2B, 4B, 6B, 8B): la curva pasa por cada punto sin esquinas y no se corrige al pararse; los gestos (recta, rectángulo, compás) solo con un segundo quieto y un trazo de 160 px o más | `motor/Element.kt` (`Dureza`), `Renderer.kt` (`cocerElLapiz`), `DrawController.kt`, `DrawProperties.kt`, `PanelLateral.kt`, `Scene.kt` · v0.91 (`057d7ab`), gestos v0.92 (`800db67`) | **A medias**: `crates/pixpin-motor2d/src/tinta/grafito.rs` es el grafito de v0.73–v0.80.2 (sellos, rejilla); `dureza` solo aparece en la lista de valores válidos (`crates/pixpin-proyecto/src/para_el_movil.rs:81`): se conserva, pero se pinta como HB y no se elige | M | **Sí**: campo `dureza` del elemento; lo que el móvil pinta 8B tiene que verse 8B aquí, y al revés | Alta |
| D2 | **Plano cartesiano, recta numérica y espacio de tres ejes**: instrumentos con unidad, pasos de rótulo y de raya, estirado que enseña más números; el espacio gira (`azimut`, `elevacion`) | `motor/Plano.kt`, `motor/Espacio.kt`, `Element.kt` (`pixpin-axes`, `pixpin-number-line`, `pixpin-space`), panel de figuras | **Falta**: viajan intactos como «ajenos» (`crates/pixpin-motor2d/tests/los_veintitres_tipos_del_movil.rs`, `LOS_AJENOS`; `excalidraw.rs:839`). La gráfica de funciones sí está (`grafica.rs`) | M | **Sí**: el PC tiene que dibujarlos, moverlos y estirarlos sin perder ninguna clave | Media |
| D3 | **Tabla de coordenadas**: puntos tecleados por coordenadas, un eje común para todo el dibujo, una tabla por color | `motor/Tablas.kt`, `DrawTablas.kt`, `Scene.kt` (`tablas`), `Renderer.kt`, `DrawSvg.kt`, `PdfLienzo.kt` | **Falta**: la clave `tablas` de la escena viaja intacta, no se pinta | M | **Sí** (clave de la escena) | Media |
| D4 | **Modo guía** (líneas de referencia): un interruptor para trazar guías con cualquier herramienta, dos «mundos» (fuera del modo una guía no se elige ni se borra), esconderlas | `Element.reference`, `Scene.kt` (`referenciasVisibles`), `DrawToolbar.kt`, `VentanaDeAjustes.kt`, `Renderer.kt`, `DrawSvg.kt` | **A medias** desde el 26-sep: las del móvil se pintan al 35 % y vuelven intactas (`extras.referencia`); falta el modo, los dos mundos y esconderlas | S–M | **Sí** (`referenciasVisibles` va en el fichero) | Media |
| D5 | **Sólidos isométricos**: caja, cuña, cilindro, prisma, revolución, extrusión; con su mando de bola, su sombra y sus cortes | `motor/Solido.kt` (2.234 l.), `Proyeccion.kt`, `Csg.kt`, `CaraExacta.kt`, `Mando.kt` (`pixpin-solid`, `formaSolida`) | **Falta** (ajeno, viaja intacto) | L | **Sí** | Baja |
| D6 | Restos del inventario del 26-sep: ángulos que forma un clavo, imán al recolocar un clavo, tiradores ocultos donde hay clavo, medida (largo y ángulo) de una raya ya dibujada desde el panel | `motor/Nudos.kt`, `Iman.kt`, `DrawEditorActivity.kt` | **A medias** (lista del 22-sep, §F-bis) | S | No | Baja |

### 1.3 Croquis 3D

| # | Función | Android (fichero, versión) | Estado en el PC | Tam. | Sinc. | Prio. |
|---|---|---|---|---|---|---|
| C1 | **Ver un croquis 3D**: los trazos-tubo con su lomo y su flanco, cuerpos (bola, cilindro, cono, anillo), sol, vistas guardadas; girar alrededor de lo que se ve | `croquis3d/Croquis3DLienzo.kt`, `Camara3D.kt`, `Croquis3DEsqueleto.kt`, `Croquis3DPluma.kt`, `Croquis3DTiras.kt`, `Croquis3DAlmacen.kt` · v0.59–v0.85 | **Falta.** Se sincroniza el `croquis3d/<id>.croquis.gz` tal cual (`crates/pixpin-sincro/src/disco.rs:44,727`), pero no se abre: pinearlo da un aviso (`pines.rs:2489`). Encaja en el proceso aparte del visor 3D (`crates/pixpin-cad/src/ventana3d.rs`) | M | Solo lectura: no | Media |
| C2 | **Editar el croquis**: dibujar en el espacio, licuar, grupos, mando, gráficas 3D (`z = f(x,y)`), modelos IFC/OBJ dentro, «En el sitio» no | `croquis3d/Croquis3DControlador.kt` (175 KB), `Croquis3DMando.kt`, `Graficas3D.kt`, `Croquis3DCubo.kt`, `LectorObj.kt`/`LectorIfc.kt` | **Falta** | L | **Sí**: la fusión trazo a trazo de Android (`sincro/Fusion.kt`) tiene que estar en `pixpin-sincro` antes de que el PC escriba croquis | Baja |
| C3 | Croquis a **OBJ** y a la **página web** con su visor (suelo, sombra, niebla, bajar el OBJ) | `croquis3d/ExportarObj.kt`, `ExportarCroquisHtml.kt`, `motor/VisorEspacio.kt` (JS sin dependencias) | **Falta** (va con C1) | M | No | Baja |
| C4 | «Al croquis 3D» desde el visor 3D | `planos/Visor3DActivity.kt` · v0.116 (`593dc08`) | **Falta** (necesita C2) | S | No | Baja |

### 1.4 Pines y portapapeles

| # | Función | Android (fichero, versión) | Estado en el PC | Tam. | Sinc. | Prio. |
|---|---|---|---|---|---|---|
| P1 | **Pin de color**: copiar `#ff8800`, `rgb(…)` o un nombre CSS y pinear da una muestra del color, que se copia de un toque | `clipboard/ContentClassifier.kt` (`ColorPin`, `parseColor`), `pin/PinWindowController.kt` (`ColorPinBody`, `copyColor`) | **Falta**: `crates/pixpin-pin/src/magia.rs::clasificar` solo distingue herramienta, tabla y texto | S | No | Baja |
| P2 | **Pegatinas** (un emoji sobre el pin); en un temporizador, la pegatina fija los minutos | `pin/PinWindowController.kt` («Pegatinas», `openEmojiPicker`, `setEmoji`, `PinChrome.STICKER_SIZE_DP`) | **Falta** | S–M | No | Baja |
| P3 | **Nota pineada en páginas**, con su pie de páginas para saltar | `motormd/Paginado.kt`, `PinWindowController.kt` (`PieDePaginas`, `pasarPaginaEn`) | **Falta**: la nota del pin es un rollo | S–M | No | Baja |
| P4 | **Palabras mágicas propias** en Ajustes (añadir, quitar, volver a las de fábrica) | `clipboard/MagicWord.kt`, `data/SettingsRepository.kt`, `ajustes/CatalogoDeAjustes.kt` | **A medias**: el núcleo lee y escribe (`magia::leer/escribir`, `pines.rs:387`), falta la pestaña de Ajustes | S | No | Baja |

### 1.5 Planos CAD, lecciones, notas, proyectos y sincronizar

| # | Función | Android (fichero, versión) | Estado en el PC | Tam. | Sinc. | Prio. |
|---|---|---|---|---|---|---|
| X1 | **Botón Aa del visor de planos**: qué letras pide el plano y no están, dónde buscarlas, y **añadir SHX/TTF/OTF/TTC propias** (mandan sobre las de PixPin; el plano se relee) | `planos/PlanoActivity.kt`, `LectorDePlanos.kt` (`huellaDeLetras`), y en Rust `herramientas/pixpin-cad-android/src/lib.rs` (`familias_del_usuario`, `pruebas_letras_del_usuario`) · v0.120 (`b516be1`) | **A medias**: el PC las busca solo en Autodesk y `C:\Windows\Fonts` (`crates/pixpin-cad/src/shx.rs:418`, `texto.rs:55`). No dice cuáles faltan ni deja añadir una carpeta propia (un plano de un compañero con su SHX). **El código de Android es Rust**: se trae casi tal cual | S | No (la caché lleva la huella de las letras) | Media |
| X2 | **Lecciones: fotos y audios en una lección ya guardada**, quitar los que sobran, elegir de la galería | `lecciones/AdjuntosDeLaLeccion.kt` (`nuevos`, `viejos`, `quitados`), `LeccionesStore.kt` · v0.102 (`dc40945`) | **A medias**: «las fotos solo van con una nueva» (`apps/pixpin/src/lecciones/ficha.rs:656`); la voz va con el dictado (`almacen.rs:292`) | S | **Sí**: son mensajes que responden a la lección; quitar uno tiene que dejar su marca de borrado | Media |
| X3 | **Fórmulas compuestas en las notas** (`$…$`): fracciones con su raya, raíces, exponentes, llaves | `motormd/Formulas.kt` (árbol), `motormd/FormulaUi.kt` (composición) | **A medias**: el editor (`crates/pixpin-notas/src/editor.rs`, Ctrl+M) y el lector web (`notas_md/lector_web.rs`, `.pp-formula`) la pintan **de color**, sin componer. Ojo: `pixpin-motor2d/src/ecuacion.rs` ya compone ecuaciones para el lienzo; se puede reutilizar | M | No (el `.md` no cambia) | Media |
| X4 | **«Ponerme al día con este lienzo»**: en dos tiempos, primero ver qué tiene cada aparato de este archivo y quién lo tocó más tarde, luego traerlo | `motor/PonerseAlDia.kt`, `DrawEditorActivity.kt` (16-sep) | **Falta**. El PC tiene «Sincronizar sola» y el botón general, que lo cubren en parte | M | **Sí** | Media-baja |
| X5 | **Detalle**: cuánto pesa lo que se edita y su proyecto (PDF, cada lienzo con sus fotos, croquis, notas) | `motor/Detalle.kt` | **No encontrado** en el PC | S | No | Baja |
| X6 | **Hojas de un Excel del chat editadas**: Android en `tablas/libro-<msg>-<i>.json`, el PC en mensajes mini-app `tabla` | `guardados/LibroDelChat.kt` | **Distinto** y ninguno ve las ediciones del otro (A3 de `2026-10-12-que-se-sincroniza.md`). **Pendiente de que el usuario elija formato**: no lanzar hasta entonces | M | **Sí** | Alta (hojas) |

### 1.6 Lo que es del teléfono: no se porta, y su equivalente en Windows

| Android | Por qué no | Equivalente en Windows | Estado |
|---|---|---|---|
| Atajos del icono y del buscador del teléfono (`atajos/Atajos.kt`, `xml/shortcuts.xml`, v0.99) | Son de Android | **Jump List** del icono de la barra de tareas (`ICustomDestinationList`): Capturar, Mensajes guardados, Proyectos, Sincronizar y los tres últimos proyectos, por el mismo camino que los pedidos (`pedidos.rs`) | **Falta** la Jump List (S, prio. baja-media). El buscador ya está (Flow Launcher, `pixpin-flow`) |
| Widgets de tareas y galería (`widget/*`, v0.107) | La pantalla de widgets de Windows 11 pide la app empaquetada (MSIX) y un proveedor COM: demasiado para esto | La lista de Tareas pineada siempre encima (ya existe) y la Jump List | No se hace |
| Bola flotante, mosaico de ajustes rápidos | Son del teléfono | Bandeja y atajo general | Hecho |
| Multitarea con baraja (`ui/Multilienzo.kt`, `RuedaDeAbiertos.kt`) | Windows ya tiene ventanas, Snap y Alt+Tab | Grupos de ventanas guardados (`grupos_ventanas.rs`) | Hecho |
| Notificación de reproductor del lector | — | `SystemMediaTransportControls` | Es **L4** |
| Auricular de las llamadas en el lector | No hay auricular de llamadas en un PC corriente | La salida de comunicaciones ya la usa la llamada secreta (`llamada.rs`) | No se hace |
| Motores de voz Vosk y Google | Distinto por diseño | Whisper sobre el ONNX Runtime de Windows y SAPI | Hecho a su manera |
| «En el sitio» del croquis (cámara + postura del teléfono), esquivar la cámara, 120 Hz, mira al lado del dedo, arreglos de la tableta Huawei, `ZoomDeHoja` | Cámara, sensores, pantalla táctil del móvil | Ninguno útil (con ratón la mira es el cursor) | No se hace |
| Fotos del PC nítidas en el móvil (v0.122, `docs/calidad-de-las-fotos.md`) | Era la lectura del móvil | El PC ya cumple lo que pide: manda la captura sin recomprimir y el `crop` va con `naturalWidth/Height` (`pixpin-motor2d/src/excalidraw.rs:1023,1090`) | Nada que hacer |
| Editar el PDF original (roadmap 10 del móvil) | Falta en los dos | — | Fuera |

## 2. Las tandas

Reglas comunes a todas (van dentro de cada encargo):

- Worktree propio que sale de `main`, rama con el nombre de la tanda.
- **Seguir la lógica de Android**: leer el Kotlin con `gh api repos/1xmanMAX/PIXPIN_PRO_ANDROID/contents/<ruta> --jq .content | base64 -d`, nunca el clon local (está viejo).
- Pruebas por crate (`cargo test -p <crate>`), no `cargo test` a secas; compilar con `-j 4`.
  Nada de tocar `%APPDATA%`: lo que pida datos, con la instancia de prueba (`instancia_de_prueba.rs`).
- Si se anota algo, con el motor del lienzo (`lector_tinta::Tinta`), nunca una tinta propia.
- Lo que cambie un fichero que viaja: una prueba en `crates/pixpin-proyecto/tests/sincronizar_con_el_movil.rs` que lo lleve y lo traiga sin perder claves.

Reparto por ficheros, para que no se pisen:

| Tanda | Toca | No toca |
|---|---|---|
| 1 Lectores: leer | `visor.rs`, `lector.rs`, `lector_pdf.rs` (sin `voz.rs`), `pixpin-docs` (nuevo `pdf_a_texto.rs`), `pixpin-pdf/src/texto.rs` | `leer_en_voz.rs`, `lector_tinta.rs`, `main.rs` |
| 2 Lectores: voz | `leer_en_voz.rs`, `lector_pdf/voz.rs`, `pixpin-docs/src/voz_alta.rs`, `pixpin-voz` (nuevo `edge.rs`), `pixpin-shell` (nuevo `reproductor_sistema.rs`), `main.rs` (dueño de la lectura) | `visor.rs` y `lector_pdf.rs` salvo el enganche mínimo |
| 3 Lienzo: grafito y guías | `pixpin-motor2d/src/tinta/grafito.rs`, `forma_rapida.rs`, `gesto.rs`, `estilo.rs`, `apps/pixpin/src/dibujo/*`, `panel_dibujo.rs` | Tipos nuevos de elemento |
| 4 Lienzo: instrumentos | `pixpin-motor2d` (nuevos `plano_cartesiano.rs`, `espacio.rs`, `tablas_coordenadas.rs`), brazos nuevos en `excalidraw.rs`, `pintado.rs`, `exportar_svg.rs`, la prueba de los veintitrés | `tinta/`, `gesto.rs` (solo lo justo para la herramienta) |
| 5 Croquis 3D (ver) | `crates/pixpin-cad` (nuevo `croquis.rs` + su ventana), nuevo `apps/pixpin/src/croquis3d.rs`, `pines.rs:2489` (el aviso) | `pixpin-sincro` |
| 6 Pines | `crates/pixpin-pin`, `apps/pixpin/src/pines.rs`, `pines/*`, `ventana_ajustes.rs` (pestaña de palabras) | El lienzo |
| 7 Sueltos | `crates/pixpin-cad/src/shx.rs`, `texto.rs`, `plano_cad.rs` (Aa); `lecciones/*`, `pixpin-lecciones`; `pixpin-notas` y `notas_md/lector_web.rs` (fórmulas); `pixpin-shell` (nuevo `jump_list.rs`) | Lo de las demás |

Conflictos que quedan: **3 y 4** comparten `excalidraw.rs` y el panel (`dibujo/permitidas.rs`):
se integra antes la 3 y la 4 rebasa. **2 y 7** tocan `pixpin-shell` en ficheros distintos y
`lib.rs` con una línea cada una. **5** abre un croquis en el proceso del visor de planos, que no
toca nadie más.

Después de estas, y por este orden: sólidos (D5, tras la 4, que deja la proyección del espacio),
editar el croquis (C2–C4, tras la 5 y con la fusión trazo a trazo en `pixpin-sincro`), «Ponerme
al día» y Detalle (X4, X5), modo lápiz táctil (L5) y los restos de clavos (D6). El Excel (X6)
espera a que el usuario elija formato.

### Tanda 1 — Lectores: la página que sube sola y el PDF como texto

```
Worktree nuevo desde main, rama `lector-leer`. Encargo: en los lectores del PC (Word/EPUB en
apps/pixpin/src/visor.rs, PDF en apps/pixpin/src/lector_pdf.rs, lo común en lector.rs) añade dos
cosas de PixPin Android, con su lógica:

1. «Que la página suba sola» (Android v0.88 98a50b9 y v0.92 800db67): botón de las dos flechas
   en la pastilla; velocidad en PALABRAS POR MINUTO (80-700, paso 20, 230 de fábrica, se
   recuerda por aparato), «Terminas en N min · a las HH:MM», pausa, seguir desde donde se deje
   la rueda, para solo al final y al anotar, presentar o escuchar. Agrandar la letra no cambia
   la velocidad (los px/s salen de palabras del documento / alto del documento).
2. «Ver como texto» del PDF (v0.92 y v0.93 a6153f8): el texto del PDF en líneas y párrafos
   (guiones de fin de línea, títulos por tamaño, números de página sueltos fuera, idioma) y a
   dos columnas en su orden (buscar el pasillo vacío del medio); se abre en el lector de Word,
   así se lee, se escucha, se cambia la letra y se anota como un Word. Un escaneo sin texto
   dice que no se puede.

Lee de Android (gh api, nunca el clon local): app/src/main/java/com/forge/pixpin/motor/AutoDesplazar.kt,
ui/MandosDeLector.kt, ui/VisorHtmlActivity.kt, pdf/LectorPdfActivity.kt, motor/PdfAHtml.kt y
sus pruebas app/src/test/java/com/forge/pixpin/AutoDesplazarTest.kt y PdfAHtmlTest.kt.
Toca en el PC: visor.rs, lector.rs, lector_pdf.rs (no lector_pdf/voz.rs), crates/pixpin-docs
(nuevo pdf_a_texto.rs con la lógica pura) y, si hace falta, crates/pixpin-pdf/src/texto.rs.
No toques leer_en_voz.rs, lector_tinta.rs ni main.rs (otra tanda).
Pruebas: las de AutoDesplazarTest y PdfAHtmlTest pasadas a Rust (mismos números y casos),
una con un PDF de dos columnas real; cargo test -p pixpin-docs y -p pixpin. Prueba a mano en
la app instalada con la instancia de prueba, sin tocar %APPDATA%. Commit en español.
```

### Tanda 2 — Lectores: voces de Microsoft y voz que sigue al cerrar

```
Worktree nuevo desde main, rama `lector-voz`. Encargo: dos cosas de PixPin Android en
«Escuchar» de los lectores del PC (apps/pixpin/src/leer_en_voz.rs, lector_pdf/voz.rs, cuentas
en crates/pixpin-docs/src/voz_alta.rs):

1. Voces de Microsoft (Android v0.90 6e97552): el protocolo de «Leer en voz alta» de Edge
   (Sec-MS-GEC, mensajes de ajustes y SSML, tramas binarias de audio, lista de voces, reintento
   si la hora no cuadra con 403), un párrafo por conexión, tres por delante, MP3 en caché por voz
   y texto con tope, lista de voces guardada una semana; «Voces de Microsoft» y «Elegir voz» en
   el engranaje del lector (se recuerda por idioma). Si falla o no hay red, sigue SAPI desde ese
   párrafo. SIN crates pesados: WebSocket de WinRT (Windows.Networking.Sockets.MessageWebSocket,
   el crate `windows` ya está) y el MP3 con Media Foundation (crates/pixpin-audio). El texto sale
   a Microsoft: dilo en el engranaje como Android.
2. La voz sigue al cerrar el lector (v0.89 1e66199): un solo lector de voz para la app (que
   viva en main.rs, no en la ventana), con SystemMediaTransportControls de Windows (pausa,
   anterior, siguiente, título del documento) y las teclas multimedia; el marcador verde se
   sigue apuntando con el lector cerrado; volver a abrir el documento va al párrafo que suena.

Lee de Android: motor/EdgeVoz.kt, ui/VozDeEdge.kt, ui/LectorEnVoz.kt, ui/LeyendoEnVozService.kt,
y las pruebas EdgeVozTest.kt y EdgeVozEnVivoTest.kt.
Toca: leer_en_voz.rs, lector_pdf/voz.rs, voz_alta.rs, crates/pixpin-voz (nuevo edge.rs),
crates/pixpin-shell (nuevo módulo para los mandos del sistema), main.rs. En visor.rs y
lector_pdf.rs solo el enganche mínimo (otra tanda los toca).
Pruebas: las de EdgeVozTest en Rust (cabeceras, SSML, troceo de tramas, elegir voz); una en
vivo solo con una variable de entorno (como EDGE_EN_VIVO=1); el reloj del reintento; cargo
test -p pixpin-voz y -p pixpin. A mano: escuchar un Word, cerrar el lector, pausar con la tecla
multimedia. Commit en español.
```

### Tanda 3 — Lienzo: grafito con durezas y modo guía

```
Worktree nuevo desde main, rama `lienzo-grafito`. Encargo: en el motor del lienzo del PC
(crates/pixpin-motor2d, apps/pixpin/src/dibujo, panel_dibujo.rs) pon al día dos cosas con la
lógica de PixPin Android:

1. Grafito libre con siete durezas (v0.91 057d7ab y gestos v0.92 800db67): 4H, 2H, HB, 2B, 4B,
   6B, 8B (campo `dureza` del elemento, HB sin escribir si es la de fábrica), cada una con su
   grosor de mina y su carga; la curva pasa por cada punto sin esquinas y no se endereza al
   pararse; recta y rectángulo solo con un segundo quieto y 160 px o más en pantalla, compás al
   clavar la punta; una letra no se convierte. Selector en el panel lateral, como Android. Lo
   que viene del móvil con dureza se tiene que ver igual aquí, en PNG, SVG, PDF y la web.
2. Modo guía (líneas de referencia): interruptor para trazar guías con cualquier herramienta,
   dos mundos (fuera del modo una guía no se elige ni se borra, y dentro solo las guías),
   esconderlas (clave `referenciasVisibles` del fichero) y la marca de centro de los óvalos
   guía. Hoy el PC solo las pinta al 35 % (extras.referencia).

Lee de Android: motor/Element.kt (Dureza, reference), Renderer.kt (cocerElLapiz, guías),
DrawController.kt, DrawProperties.kt, PanelLateral.kt, Scene.kt (referenciasVisibles),
DrawToolbar.kt, VentanaDeAjustes.kt, DrawSvg.kt.
Toca: crates/pixpin-motor2d/src/tinta/grafito.rs, forma_rapida.rs, gesto.rs, estilo.rs,
excalidraw.rs (solo los campos `dureza` y `referenciasVisibles`), apps/pixpin/src/dibujo/*,
panel_dibujo.rs. No añadas tipos de elemento (otra tanda).
Pruebas: muestras PNG de las siete durezas al lado de lo que pinta Android para el mismo
trazo; ida y vuelta de `dureza` y `referenciasVisibles` sin cambiar bytes
(crates/pixpin-motor2d/tests y crates/pixpin-proyecto/tests/sincronizar_con_el_movil.rs);
gestos: una letra no se vuelve recta. cargo test -p pixpin-motor2d -p pixpin-proyecto.
Commit en español.
```

### Tanda 4 — Lienzo: plano cartesiano, recta numérica, espacio y tabla de coordenadas

```
Worktree nuevo desde main, rama `lienzo-instrumentos`. Encargo: enseña al motor del PC a
dibujar y editar cuatro cosas de PixPin Android que hoy viajan intactas sin verse:
`pixpin-axes` (plano cartesiano), `pixpin-number-line` (recta numérica), `pixpin-space`
(espacio de tres ejes que gira con azimut y elevación) y la tabla de coordenadas (clave
`tablas` de la escena). Con la lógica de Android: unidad, pasos de rótulo y de raya, estirar
por un lado enseña más números y por una esquina los agranda; el tirador de giro del espacio
da la vuelta al espacio; los puntos de la tabla caen en el eje común. Entrada en el panel de
figuras, como Android; salen en PNG, SVG, PDF y la página web.

Lee de Android: motor/Plano.kt, motor/Espacio.kt, motor/Tablas.kt, motor/DrawTablas.kt,
Element.kt, Scene.kt, Renderer.kt, DrawSvg.kt, PdfLienzo.kt y la prueba TablasTest.kt.
Toca: crates/pixpin-motor2d (módulos nuevos para los tres instrumentos y la tabla; brazos
nuevos en excalidraw.rs, pintado.rs, exportar_svg.rs, exportar.rs), el panel de figuras del
PC. Quita los tres de LOS_AJENOS en tests/los_veintitres_tipos_del_movil.rs (la prueba lo
exige). No toques tinta/ ni la lógica del grafito: otra tanda; si coincidís en excalidraw.rs,
rebasa sobre ella.
Pruebas: «lo que entra vuelve a salir» para los tres tipos y la tabla, con dibujos hechos en
Android; mover y estirar solo cambia lo que debe; muestras PNG frente a Android.
cargo test -p pixpin-motor2d -p pixpin-proyecto. Commit en español.
```

### Tanda 5 — Croquis 3D: abrirlo y verlo

```
Worktree nuevo desde main, rama `croquis-ver`. Encargo: que el PC abra y enseñe un croquis 3D
de PixPin Android (croquis3d/<id>.croquis.gz, ya llega al sincronizar) en vez del aviso de
apps/pixpin/src/pines.rs:2489. Solo ver, sin editar: los trazos como tubos con lomo y flanco
(se afinan en las puntas, engordan con la presión, cambian de tono al girar), los cuerpos
(bola, cilindro, cono, anillo), las hojas planas, las imágenes puestas en el espacio, el sol y
las vistas guardadas; girar alrededor del centro de lo que se ve, mover, acercar. Desde el
chat, desde la hoja del proyecto y como pin. Nada se escribe en el fichero.

Va en el proceso aparte del visor de planos/3D (crates/pixpin-cad, ventana3d.rs), que ya
tiene Direct3D, cámara y caja de sección: reutiliza su cámara y su ventana.
Lee de Android: croquis3d/Croquis3DAlmacen.kt (formato), Croquis3DEsqueleto.kt,
Croquis3DPluma.kt, Croquis3DTiras.kt, Camara3D.kt, Croquis3DLienzo.kt (solo el pintado),
Croquis3DVistas.kt, MarcosMinimos.kt, PulsoDelMundo.kt y motor/VisorEspacio.kt (la misma
proyección en JS, sirve de referencia corta).
Toca: crates/pixpin-cad (nuevo croquis.rs y su ventana), nuevo apps/pixpin/src/croquis3d.rs,
el aviso de pines.rs y el enganche en abrir_hoja.rs. No toques pixpin-sincro.
Pruebas: los casos de app/src/test/java/com/forge/pixpin/Croquis3DTest.kt en Rust (Android
no trae ficheros .croquis.gz de prueba: hazlos con su formato) y la proyección con los números
de Camara3D; muestra PNG de un croquis real; que abrir no
cambie un byte del fichero. cargo test -p pixpin-cad. Commit en español.
```

### Tanda 6 — Pines: color, pegatinas, nota en páginas y palabras propias

```
Worktree nuevo desde main, rama `pines-android`. Encargo: cuatro cosas de los pines de PixPin
Android, con su lógica:
1. Pin de color: copiar `#rgb`, `#rrggbb`, `#aarrggbb`, `rgb()/rgba()` o un nombre CSS y
   pinear da una muestra con su valor; un clic copia el color. Va antes que la palabra mágica,
   como en ContentClassifier.
2. Pegatinas: un emoji sobre la esquina del pin (elegir, cambiar, quitar); en un temporizador
   la pegatina fija los minutos, como Android.
3. Nota pineada en páginas, con el pie de páginas para saltar (Paginado.kt).
4. Palabras mágicas propias en Ajustes: lista, añadir, quitar, «De fábrica». El núcleo ya
   lee y escribe (crates/pixpin-pin/src/magia.rs).

Lee de Android: clipboard/ContentClassifier.kt, clipboard/MagicWord.kt, pin/PinWindowController.kt
(«Pegatinas», ColorPinBody, PieDePaginas), pin/PinChrome.kt, motormd/Paginado.kt,
ajustes/CatalogoDeAjustes.kt, data/SettingsRepository.kt.
Toca: crates/pixpin-pin (contenido.rs, magia.rs, ventana.rs, menu.rs), apps/pixpin/src/pines.rs,
pines/*, ventana_ajustes.rs (solo la pestaña de las palabras).
Pruebas: clasificador con los casos de ContentClassifierTest.kt; paginado con los de
PaginadoTest.kt; palabras con los de MagicWordTest.kt, y las propias sobreviven a reiniciar. cargo test -p pixpin-pin -p pixpin. Commit en
español.
```

### Tanda 7 — Sueltos: letras de los planos, lecciones, fórmulas y Jump List

```
Worktree nuevo desde main, rama `sueltos-android`. Encargo: cuatro cosas pequeñas de PixPin
Android, cada una en su commit:
1. Planos, botón Aa (v0.120 b516be1): decir qué letras pide el plano y no están, dónde suelen
   estar, y añadir SHX/TTF/OTF/TTC a una carpeta propia de PixPin que manda sobre las de
   Autodesk, Windows y Hershey; el plano se relee (la clave de la caché lleva la huella de las
   letras). El código de Android ya es Rust: herramientas/pixpin-cad-android/src/lib.rs
   (familias_del_usuario, pruebas_letras_del_usuario) y planos/PlanoActivity.kt,
   planos/LectorDePlanos.kt (huellaDeLetras). Toca crates/pixpin-cad/src/shx.rs, texto.rs y
   apps/pixpin/src/plano_cad.rs.
2. Lecciones: añadir fotos y audios a una lección ya guardada, quitar los que sobran y elegir
   de la galería (lecciones/AdjuntosDeLaLeccion.kt, LeccionesStore.kt, v0.102 dc40945). Son
   mensajes que responden a la lección: quitar uno deja su marca de borrado y viaja. Toca
   apps/pixpin/src/lecciones/* y crates/pixpin-lecciones.
3. Fórmulas compuestas en las notas `$…$` (motormd/Formulas.kt, motormd/FormulaUi.kt):
   fracciones, raíces, exponentes, llaves, en el lector web de las notas
   (apps/pixpin/src/notas_md/lector_web.rs) y en el editor (crates/pixpin-notas). Mira antes
   crates/pixpin-motor2d/src/ecuacion.rs y formula.rs, que ya componen para el lienzo.
4. Jump List del icono (equivalente de atajos/Atajos.kt y xml/shortcuts.xml, v0.99): Capturar,
   Mensajes guardados, Proyectos, Sincronizar y los tres últimos proyectos, con
   ICustomDestinationList, por el mismo camino que los pedidos de fuera (apps/pixpin/src/pedidos.rs).
   Nuevo módulo en crates/pixpin-shell.
Pruebas: la de letras del usuario en Rust (la de Android); lecciones: añadir y quitar en una
vieja viaja ida y vuelta (crates/pixpin-proyecto/tests/sincronizar_con_el_movil.rs); fórmulas:
los casos de FormulasTest.kt de Android; Jump List: la lista pura que se arma, probada
sin Windows. cargo test por crate. Commit en español.
```

## 3. Cómo se comprobó y lo que no

- Cada «Falta» sale de buscar en `apps/` y `crates/` los nombres, campos y textos de Android
  (por ejemplo `dureza`, `pixpin-axes`, `referenciasVisibles`, `AutoDesplazar`, `EdgeVoz`,
  `ColorPin`) y de leer la cabecera del módulo del PC que debería tenerlo. Un «A medias» dice qué
  parte está y en qué fichero.
- No se compiló ni se abrió la app (pedido). El aspecto de nada de esto se ha comparado en
  pantalla; las tandas piden muestras PNG para eso.
- No se repasó cada botón de cada pantalla de Android: lo de la v0.85 para atrás lo cerró la
  lista del 22-sep (todo «Hecho» salvo lo que aquí sigue) y lo de después se leyó commit a
  commit. Lo que Android trajo del PC (v0.101–v0.122, salvo lo marcado aquí) no se repite.
