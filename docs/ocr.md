# Reconocer texto (OCR): Windows e Indra

PixPin lee el texto de las imágenes en muchos sitios: la captura de texto,
«Calca esto», los pines (seleccionar texto con el ratón), la galería de
capturas (buscar por lo que pone), los PDF escaneados y las herramientas del
asistente (`tabla_de_imagen`, hoja a lienzo, anotar…), además de
`pixpin-mcp.exe`. **Todos pasan por `pixpin_ocr::reconocer`**, que lee con el
motor que el usuario elige en **Ajustes › Captura › Reconocer texto (OCR)**:

| Motor | Qué es | Coste |
|---|---|---|
| **Windows** (de fábrica) | `Windows.Media.Ocr`, viene con Windows 10/11 | nada |
| **Indra** (opcional) | Indra OCR de Datalab, en el PC y sin conexión | 26 MB que baja el usuario; ~150-250 MB de RAM mientras lee |

Si Indra no está bajado, no carga o falla en una imagen, se lee con el de
Windows (y queda en el registro). La forma del resultado es la misma con los
dos: líneas con su caja y sus palabras, en píxeles de la imagen.

## Indra OCR: qué es

`github.com/datalab-to/indra` (v1.1.2, 22-oct-2026) y los pesos
`huggingface.co/datalab-to/indra-ocr` (etiqueta `v1.1.0`, commit
`bd02167f…`).

- **No es un modelo de lenguaje grande**: son redes pequeñas en ONNX
  (IR 8, opset 17) con un canal clásico alrededor. Detector de renglones
  (`det.onnx`, 0,4 MB), reconocedor CTC latino (`rec.onnx`, 5 MB, ~1,25 M
  parámetros; latín de ~50 idiomas —español incluido—, cirílico, griego y
  manuscrito latino), orientación (`orient.onnx`), orden de lectura y
  bloques de maquetación (`order.onnx`: título, texto, lista, tabla, pie…) y
  un modelo de lenguaje n-grama (`lm.npz`) para las líneas dudosas. Hay
  reconocedores aparte para CJK, árabe/hebreo e índicos/tailandés; PixPin,
  como el escáner, usa solo el latino.
- **Runtime**: ONNX Runtime en CPU, sin PyTorch ni GPU. El paquete de Python
  añade OpenCV y numpy; aquí no hacen falta (ver más abajo).
- **Salida**: líneas con caja y confianza, bloques con etiqueta en orden de
  lectura, texto plano y markdown (este último no se usa).
- **Lo que dice Datalab** (OmniParseBench, 4 vCPU, una página a la vez):
  66,7 de nota general frente a 58,4 de PP-StructureV3; texto 46,7 frente a
  35,8 de PP-OCR y 19,3 de Tesseract; mediana de 212 ms por página (OCR
  solo: 344 ms) frente a 5,5 s de Tesseract. Limitaciones que ellos mismos
  dan: tablas complejas, manuscrito (25 %), escaneos degradados (29,5 %).

### Licencia (exacta)

- **Código**: Apache-2.0. El puerto de PixPin (`crates/pixpin-indra`) es una
  traducción y lleva su aviso (`THIRD-PARTY-NOTICES.md`,
  `crates/pixpin-indra/LICENCIA-INDRA.txt`).
- **Pesos**: «AI Pubs Open RAIL-M License (Modified)» v0.1 de Datalab. Lo que
  importa:
  - Anexo A, 2(a)/(b): **no se pueden usar** si tú (o tu empresa) facturaste
    más de **20 M$** el año anterior o levantaste más de **20 M$** de
    financiación, *salvo uso personal o de investigación*.
  - Anexo A, 2(c): no se pueden usar si ofreces un producto o servicio **que
    compita** con los de Datalab (que vende conversión de documentos por API).
  - §4: quien **redistribuye** los pesos tiene que imponer esas restricciones
    por contrato a sus usuarios y entregar la licencia; §7 pide atribución
    «en conexión con cualquier salida»; §8 extiende la licencia a la salida.
  - Licencias comerciales más amplias: `datalab.to/pricing`.

  **Por eso PixPin no incluye los pesos**: no los redistribuye, los baja el
  propio usuario desde Hugging Face cuando elige Indra, con la licencia dicha
  en la fila de Ajustes y un botón para leerla. Para uso personal (el caso de
  PixPin Max hoy) es gratis. **Una distribución comercial de PixPin con
  Indra activado de fábrica, o venderlo a empresas grandes, necesitaría
  licencia de Datalab.** El de Windows sigue siendo el de fábrica.

## Medido (25-oct-2026)

Equipo de medida: el contenedor de desarrollo (Xeon a 2,1 GHz, 4 vCPU
compartidas, AVX2 y AVX-512, 15 GB). `indra_ocr` 1.1.2 de Python con
`onnxruntime` 1.31.0, `mode="ocr"`, mediana de 3 lecturas tras una de
calentamiento (`herramientas/indra/banco.py`). Las cuatro primeras imágenes
son sintéticas en español con su verdad; la quinta es la muestra del
repositorio de Indra (carta coreana: solo cuenta el tiempo).

| Imagen | Píxeles | 1 hilo | 2 hilos | 4 hilos | CER |
|---|---|---|---|---|---|
| Ajustes de Windows (captura, claro) | 1280×720 | 123 ms | 213 ms | 204 ms | 0,00 % |
| Código en modo oscuro | 1600×900 | 349 ms | 377 ms | 446 ms | 1,6 % |
| Página A4 a 150 ppp | 1240×1754 | 322 ms | 329 ms | 386 ms | 0,00 % |
| La misma «escaneada» (gris, sombra, 1,8°, ruido, desenfoque) | 930×1316 | 272 ms | 373 ms | 365 ms | 0,00 % |
| Muestra del repo (carta) | 1653×2337 | 602 ms | 979 ms | 904 ms | — |

- Carga: 0,6-0,8 s. Memoria del proceso de Python: 290 MB tras cargar y
  420-460 MB de pico, de los que ~97 MB son Python + numpy + OpenCV.
- Más hilos no ayudaron aquí porque las vCPU son compartidas; en un PC de
  verdad, dos hilos rinden más que uno.
- Tesseract no está instalado en el contenedor: no se midió. El escáner sí lo
  midió en su banco de 54 fotos reales (ver abajo).

**Banco del escáner** (Escáner Pro Max, 54 imágenes reales: exámenes,
cuadernos, facturas; `notes_indra.md` de ese proyecto): Indra CER 29,1 /
76,2 % de palabras, PaddleOCR-VL 28,8 / 80,3 % (a ~110 s por página),
Tesseract 53,8 / 46,5 %. Con el «afinado» (preparar la página, líneas dudosas
aparte, diccionario español y vocabulario propio) Indra baja a CER 27,2; en
español impreso (MERIT) pasa de 88,8 a 93,0 % de palabras.

### El puerto a Rust

`crates/pixpin-indra` es el canal entero sin OpenCV: inclinación (la misma
búsqueda de ±45° que el original, con `warpAffine` reescrito), entrada del
detector, componentes conexas, recortes de renglón, lotes por ancho,
decodificación CTC, búsqueda en haz con el LM, cortes de columna, orden de
lectura y bloques, líneas dudosas, hunspell y corrector. Se probó contra
Python con los modelos de verdad (ejemplo `paridad`, que sirve los modelos con
el `onnxruntime` de Python por una tubería; `herramientas/indra/servidor_ort.py`):

| Imagen | CER Rust frente a Python |
|---|---|
| Ajustes de Windows | 0,00 % |
| Código en modo oscuro | 0,00 % |
| Página A4 | 0,00 % |
| Página «escaneada» | 0,00 % |

- Memoria del lado de Rust: **111 MB de pico** leyendo una página de
  1653×2337 (LM 37 MB, diccionario de 55 820 raíces, la página y sus copias).
  ONNX Runtime añade los modelos (~25 MB) y sus tensores (sin arena, como
  pide Indra: con arena crecía sin parar).
- **La preparación de la página se salta en las oscuras**: dividir por el
  «papel» negro volvía blanco el texto claro (el código pasaba de 0 a 1,95 %
  de CER). En capturas claras deja el fondo igual y solo enfoca un poco.

### Veredicto para el equipo de referencia

Core i3 de 3.ª generación (Ivy Bridge: AVX sin AVX2 ni FMA, 2 núcleos /
4 hilos), 4 GB, sin GPU:

- **Sí corre en local, y bien.** Las redes son diminutas (7,3 M parámetros
  en total, 1,25 M el reconocedor); ONNX Runtime usa sus núcleos AVX/SSE
  cuando no hay AVX2. Estimado: 2-3 veces más lento por hilo que el Xeon de
  la medida → **~0,3-1,5 s por captura o página** con dos hilos. No hace
  falta exigir AVX2 ni 8 GB.
- **Memoria**: ~150-250 MB mientras está cargado; se carga al leer la primera
  imagen (~0,5-1 s) y **se suelta a los 3 minutos sin leer nada**. En el modo
  cuidadoso (4 GB o nivel Ligero) usa dos hilos.
- **Disco/descarga**: 25,6 MB de pesos y diccionario. En los Windows que no
  traen `onnxruntime.dll` en `System32` (Windows 10 y los 11 anteriores a
  24H2) se baja además el oficial 1.31.0: **6,2 MB** de descarga (solo la
  entrada de la DLL del ZIP de 83 MB, con una petición `Range`), 17,3 MB en
  disco, en `<datos>/whisper/onnxruntime.dll` —el sitio que ya mira Whisper,
  que así también funciona en esos equipos—. El de `System32` (1.17 o más
  nuevo) sirve: Indra es opset 17.
- **Con red solo para bajarlo**: después lee sin conexión.

No se ofrece «Indra por API» porque el proyecto no la tiene: la API de
Datalab usa otros modelos (Chandra) y es otro servicio de pago.

## Cómo está hecho

```
pixpin-indra (capa 0, Rust puro, se prueba en cualquier sistema)
  gris, imagen, prep      la página, la inclinación, los recortes
  deteccion, canal        de det.onnx a renglones; la página entera
  lm, vocab               LM n-grama y búsqueda en haz CTC
  orden                   order.onnx, XY-cut, bloques
  dudas, lexico, corrector  el afinado del escáner
  catalogo                ficheros, URL fijadas, tamaños y sha256
        │ rasgo Modelos (detectar / reconocer / orientar / ordenar)
pixpin-ocr (capa 2, Windows)
  windows_ocr             Windows.Media.Ocr (de fábrica, respaldo)
  indra                   los modelos con el ONNX Runtime de Windows
                          (pixpin_voz::ort), descarga con sha256, memoria
  lib                     rasgo Ocr, Config, reconocer / disponible
        │ pixpin_ocr::configurar(Config)
pixpin (app)  ocr_motor   [ocr] de los ajustes → Config; la descarga y su
                          avance en la fila de Ajustes
pixpin-mcp                lee [ocr] al arrancar y usa el mismo motor
```

- **Descarga**: `pixpin_ocr::indra::bajar` baja los ocho ficheros de
  `catalogo::FICHEROS` a `<datos>/indra/` con WinHTTP (el de Whisper), cada
  uno a `.parte` y renombrado al acabar, y comprueba su sha256; uno que no
  cuadra se borra y es un error. Lo ya bajado no se vuelve a bajar.
- **Opt-in**: elegir «Indra» en Ajustes es lo que empieza la descarga; nunca
  se baja sola. Volver a «Windows» corta una descarga a medias; «Borrar
  Indra» quita los pesos.
- **Ajustes** (`[ocr]` del TOML): `motor = "windows" | "indra"`,
  `corregir = true` (diccionario español, solo en páginas en español, nunca
  cifras ni códigos) y `vocabulario` («Mi vocabulario»: términos que Indra
  prefiere en la búsqueda en haz y en la corrección).
- **Líneas dudosas** (confianza 0,3-0,5): Indra las conserva marcadas, pero
  `reconocer` no las devuelve, igual que el escáner las deja fuera del texto
  plano.

Probar con los pesos de verdad en Windows:
`PIXPIN_INDRA_PRUEBA=<datos> cargo test -p pixpin-ocr -- --ignored`.

## Android

La app del escáner (Escáner Pro Max) **ya lleva Indra** como único OCR:
`app/src/main/java/com/scannerpromax/ocr/indra/` (puerto de Kotlin con ONNX
Runtime 1.31.0 para Android, paridad CER 1,3 % frente a Python, con sus
pruebas en `app/src/test/java/com/scannerpromax/ocr/indra/`)
y es de donde sale este puerto de Rust. PixPin Android puede usarlo de tres
maneras, de menos a más trabajo:

1. **Pedírselo al escáner** (ya hecho para escanear: acción
   `com.scannerpromax.action.SCAN`). Una acción `OCR` que reciba una imagen y
   devuelva las líneas en JSON no cuesta nada a PixPin: ni pesos ni runtime.
2. **Copiar el paquete `ocr/indra/`** (y `ocr/lexicon/`) a PixPin Android:
   son ~2 800 líneas sin dependencias de la interfaz. Hace falta
   `com.microsoft.onnxruntime:onnxruntime-android:1.31.0` y, como en el
   escáner, bajar `libonnxruntime.so` aparte (12,9 MB en arm64, solo esa
   entrada del AAR) para no engordar el APK; los pesos son **los mismos
   ficheros** (`catalogo::FICHEROS`, mismas URL fijadas y sha256), así que un
   móvil y un PC leen igual. Memoria: ~60 MB de LM y modelos según el escáner,
   que mide ~0,3 s por página en PC (en el móvil no hay medida publicada).
3. **Llamar a este crate por JNI** (`pixpin-indra` es Rust puro): solo
   tendría sentido si PixPin Android ya compilara Rust; hoy no.

La licencia es la misma en el móvil: los pesos los baja el usuario, no van en
el APK (además el escáner lo descartó por tamaño: el APK pasaría de 30 MB).
