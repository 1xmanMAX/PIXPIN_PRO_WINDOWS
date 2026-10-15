# PixPin Max en Mac, iPad e iPhone: evaluación y plan (15-oct-2026)

Pedido del usuario: evaluar si PixPin Max puede tener versión de **Mac** que use lo mejor de
macOS, que **sincronice bien con Windows** (y con Android), decir con claridad **qué no se puede
sincronizar**, y dejar el camino preparado para **iPad e iPhone**. Si es posible, un plan de
implementación.

**Respuesta corta: sí, es viable, y el repositorio está mejor preparado de lo que parece.** Más de
la mitad del código es Rust puro que ya compila fuera de Windows (comprobado en esta rama). Lo
que no se puede llevar tal cual es la capa de ventanas y dibujo (Win32 + Direct2D), y esa es
justo la parte que en Apple conviene hacer nativa para aprovechar sus herramientas.

---

## 1. Lo que se midió

### 1.1 Qué compila ya fuera de Windows

`cargo check -p <crate>` en Linux (sin tocar una línea), 15-oct-2026:

| Compila tal cual ✅ | No compila ❌ (y por qué) |
|---|---|
| `pixpin-geom`, `pixpin-model`, `pixpin-nivel`, `pixpin-lecciones` | `pixpin-store`, `pixpin-proyecto`: **solo** por usar `pixpin_shell::entorno` (rutas) y `pixpin_shell::union` (uniones de carpeta). Dos módulos pequeños. |
| `pixpin-motor2d` (65k líneas, el lienzo entero) | `pixpin-codec`: WIC para HEIC/AVIF/TIFF/RAW y portapapeles. |
| `pixpin-tinta`, `pixpin-timeline` | `pixpin-cad`, `pixpin-bim`: el 3D va con D3D11/HLSL; el *lector* DWG/IFC es puro. |
| `pixpin-docs`, `pixpin-hojas`, `pixpin-office` (Word, Excel, PowerPoint) | El resto (`shell`, `render`, `capture`, `pin`, `notas`, `pdf`, `ocr`, `voz`, `audio`, `web`, `record`, la app): Win32/Direct2D/WinRT por diseño. |
| **`pixpin-sincro`** (20k, el protocolo entero) | |
| `pdfsqueeze-core`, **`pixpin-ui`** (la lógica de la interfaz) | |

En líneas de código: **~160k líneas portables hoy** + ~29k que lo serán con un arreglo pequeño
(`store`, `proyecto`) frente a ~230k de la app y ~110k de crates atados a Windows.

### 1.2 Decisiones del diseño que ayudan mucho

- **`pixpin-motor2d` no dibuja**: produce órdenes (`pintado::Orden`, `ordenes_de_escena`). Esas
  mismas órdenes se pueden pintar con Core Graphics o Metal sin tocar la geometría.
- **`pixpin-ui` es `#![forbid(unsafe_code)]`** y recibe eventos ya traducidos
  (`overlay::EventoEntrada`, `motor2d::gesto::EventoGesto`, `pin::EventoPin`).
- **`pixpin-sincro` es puro**: ni una API de Windows; el TCP se lo pasa la app como `Read + Write`.
  Las rutas que viajan ya son portátiles (`pixpin:files/…`, siempre `/`).
- La prueba de capas (`apps/pixpin/tests/capas.rs`) ya obliga a mantener fronteras limpias.

### 1.3 Lo que estorba

- **No hay un trait de dibujo**: `pixpin-render` es Direct2D y filtra sus tipos
  (`MotorRender::contexto() -> &ID2D1DeviceContext`, `Pintor::bitmap(&ID2D1Bitmap1…)`); `ID2D1*`
  aparece en 24 ficheros de `apps/pixpin/src`.
- **El bucle de la app es Win32** (`GetMessageW` en `pixpin-shell/src/ventana.rs`, bucles modales
  con `MsgWaitForMultipleObjectsEx` en `overlay.rs`). `main.rs` tiene 2.700 líneas que mezclan
  orquestación y Win32.
- **El editor de notas usa el control RichEdit** de Windows (`pixpin-notas`).

---

## 2. La decisión de arquitectura

Tres caminos:

| | A. Portar el Rust tal cual (Metal/wgpu bajo `pixpin-render`, ventanas propias) | **B. Núcleo Rust compartido + interfaz nativa en Swift** | C. Reescribir todo en Swift |
|---|---|---|---|
| Código reutilizado | Máximo en teoría | **~50–60 %: todo lo que importa (formato, sincro, lienzo, documentos)** | 0 % |
| Herramientas de Mac | Hay que llamarlas desde Rust con `objc2`: posible pero incómodo | **Directas: ScreenCaptureKit, Vision, PDFKit, PencilKit, AVFoundation…** | Directas |
| iPad / iPhone | Mal: una interfaz de ratón pensada para Win32 no vale en táctil | **Bien: SwiftUI comparte casi toda la interfaz entre Mac, iPad e iPhone** | Bien |
| Riesgo de que la sincro se desvíe de Windows/Android | Bajo | **Bajo: es el mismo crate `pixpin-sincro`** | **Alto**: tercera implementación del protocolo |
| Esfuerzo | Muy alto (reescribir 110k de capa de sistema en un toolkit no nativo) | Medio-alto | Muy alto |

**Recomendación: B.** El núcleo en Rust (formato, sincro, lienzo, documentos, IA) se compila como
`PixPinCore.xcframework` para macOS (arm64 + x86_64), iOS e iPadOS, con enlaces generados por
**UniFFI** (Mozilla, MIT). Encima, una app **SwiftUI + AppKit** (Mac) / **SwiftUI + UIKit**
(iPad/iPhone) que usa directamente lo mejor de Apple. Una sola verdad del formato y del protocolo,
como ya pasa hoy entre Windows y Android (que es Kotlin y se mantiene en paridad a mano: con B,
Apple no añade un tercer puerto, reutiliza el de Windows).

---

## 3. Qué herramienta de Apple sustituye a cada pieza (y en qué mejora)

| Pieza de Windows | En Apple | ¿Mejora? |
|---|---|---|
| `pixpin-capture`: Windows.Graphics.Capture / DXGI | **ScreenCaptureKit** (`SCScreenshotManager`, `SCStream`): ventanas, zonas, excluir la propia app, HDR | ✅ Más limpio; captura de ventana con sombra y sin fondo nativa |
| `pixpin-record`: Media Foundation MP4 | **ScreenCaptureKit + AVAssetWriter** (HEVC/H.264 por hardware, audio del sistema y micro) | ✅ Audio del sistema sin trucos |
| `pixpin-ocr`: Windows.Media.Ocr | **Vision** (`RecognizeDocumentsRequest` en macOS 26: párrafos, tablas, listas) + **Live Text** (`ImageAnalysisInteraction`) | ✅✅ Mucho mejor: tablas, códigos QR, seleccionar texto directamente sobre el pin |
| Recortar el sujeto (`pin/magia.rs`) | **Vision** `GenerateForegroundInstanceMaskRequest` | ✅ «Levantar sujeto» como en Fotos |
| `pixpin-pdf`: Windows.Data.Pdf para ver | **PDFKit** (ver, buscar, seleccionar texto, formularios); la escritura propia en Rust se queda | ✅ Selección de texto y formularios gratis |
| `pixpin-render`: Direct2D / DirectWrite | **Core Graphics + Core Text** para lo 2D; **Metal** para el lienzo grande; las `Orden` de `motor2d` se pintan igual | ≈ Igual, con mejor texto (Core Text) |
| `pixpin-tinta` / lápiz | **PencilKit** solo como *entrada* (presión, inclinación, `UITouch.predictedTouches`, *squeeze* y *barrel roll* del Pencil Pro); el trazo sigue siendo el de `motor2d` para que sincronice | ✅✅ En iPad, la mejor tinta posible |
| `pixpin-cad` 3D: D3D11 + HLSL | **Metal** (traducir los shaders a MSL) o **RealityKit** para el visor; en Vision Pro, gratis | ≈ / ✅ |
| `pixpin-pin`: ventanas «siempre encima» | **`NSPanel`** flotante (`.floating`, `canJoinAllSpaces`, `fullScreenAuxiliary`), arrastrable a otros Espacios | ≈ Igual; en iPad: **Picture in Picture** para vídeo y Stage Manager |
| `pixpin-notas`: RichEdit | **`NSTextView`/TextKit 2** con `AttributedString`, o el editor Markdown que ya existe en Rust pintado nativo | ✅ |
| `pixpin-web`: WebView2 | **WKWebView** | ≈ |
| `pixpin-audio` / `pixpin-voz`: WASAPI, SAPI, Whisper con ONNX de System32 | **AVAudioEngine**; transcribir con **SpeechAnalyzer** (macOS/iOS 26, en el aparato, sin bajar modelos); leer en voz alta con **AVSpeechSynthesizer** | ✅✅ Sin descargas de Whisper ni Vosk |
| `pixpin-ia`: llama.cpp en proceso aparte | **Foundation Models** (el modelo de Apple en el aparato, gratis, con *tool calling*) como opción por omisión en Mac/iPhone/iPad con Apple Intelligence; llama.cpp con **Metal** como respaldo (el mismo GGUF que en Windows y Android) | ✅✅ |
| `pixpin-codec`: WIC para HEIC/AVIF/RAW | **ImageIO** (`CGImageSource`) | ✅ HEIC y RAW nativos |
| `pixpin-shell`: bandeja, atajos globales, Jump List, Abrir con | **`MenuBarExtra`**, atajos globales (`KeyboardShortcuts` o `RegisterEventHotKey`), *Services*, `UTType` + `Info.plist`, Dock menu | ≈ |
| Flow Launcher (`pixpin-lanzador`) | **App Intents** → aparece en **Spotlight** (macOS 26 ejecuta acciones desde Spotlight), **Atajos** y Siri; opcional extensión de **Raycast/Alfred** reutilizando el lanzador | ✅✅ Sin instalar nada aparte |
| Compartir (`compartir.rs`) | **Share Extension** (mandar a un proyecto desde cualquier app, también en iPhone) y `NSSharingServicePicker` | ✅ |
| Vista previa (`vista_previa.rs`) | **Quick Look Preview Extension** para `.excalidraw`, `.pixpin2d`, DWG… | ✅ |
| Búsqueda (`buscar`) | Además, indexar en **Core Spotlight** | ✅ |
| DPAPI (claves de la nube) | **Llavero** (Keychain) | ✅ Sincroniza solo por iCloud Keychain si se quiere |
| mDNS (`pixpin-shell/src/mdns.rs`) | **Network.framework** (`NWListener`/`NWBrowser`, Bonjour nativo) | ✅ |
| Seguir en otro aparato | **Handoff** (`NSUserActivity`) y **Portapapeles universal** entre Apple | ✅ Nuevo |
| Cámara del iPhone en el Mac | **Continuity Camera**: «Insertar desde iPhone» foto/escaneo al chat | ✅ Nuevo |

---

## 4. Sincronización

### 4.1 Lo que ya funciona sin cambiar el protocolo

El protocolo (`pixpin-sincro`, versión 4) es simétrico y sin maestro: Bonjour/mDNS con tipos
`_pixpin._tcp`, `_pixpinenvio._tcp`, `_pixpinrecibe._tcp`, `_pixpinchat._tcp`; TCP directo con
AES-256-GCM y clave de grupo por PBKDF2; fusión a tres bandas con base por pareja de aparatos
(`base/<otro>/<chat>.json`), parches, lápidas y diff3 por párrafos. Como la app de Apple usaría
**el mismo crate**, un Mac es simplemente otro miembro del grupo (hay 26 letras), igual que hoy
un segundo PC. **Mac ↔ Windows ↔ Android sincronizan con todo lo que hoy sincroniza Windows ↔
Android**, sin tocar formato.

Bonjour es la tecnología nativa de Apple para esto: los tipos de servicio cumplen la RFC 6335 y
el TXT (`g`, `id`, `l`, `n`) es estándar.

### 4.2 Riesgos que hay que arreglar antes (afectan también a Windows/Android)

1. **Unicode NFC/NFD en nombres de fichero.** Hoy las rutas y los resúmenes se comparan byte a
   byte. Un nombre con tilde creado por Finder o llegado de HFS+ puede venir en NFD («a» + «´»)
   y en Windows/Android en NFC («á»): duplicados o un «cambiado» perpetuo. **Arreglo:
   normalizar a NFC en `disco::permitida`/`DiscoPc::ruta` y al leer `read_dir`** (crate
   `unicode-normalization`, que ya está en `Cargo.lock` como dependencia indirecta). Es compatible: lo que ya está
   en NFC no cambia.
2. **Mayúsculas/minúsculas:** APFS y NTFS no las distinguen; Android sí. Ya es un riesgo hoy;
   conviene una prueba que choque `Foto.png` con `foto.png` y decida.
3. **IPv6 / dirección:** `Vecino.host` es solo `Ipv4Addr` y `ip_local` usa el truco de
   «conectar» a 8.8.8.8. En redes Apple con IPv6 o sin salida a Internet falla. Aceptar
   `IpAddr` y dejar que Network.framework resuelva.
4. **`VERSION = 4` estricta sin negociación:** con tres plataformas que se actualizan a ritmos
   distintos, conviene que el `hola` anuncie un rango (`min..=max`) y se elija el mayor común.

### 4.3 Lo que no se puede sincronizar igual (límites de Apple)

| Qué | Por qué | Qué hacer |
|---|---|---|
| **iPhone/iPad sincronizando en segundo plano** | iOS mata el `TcpListener` y el anuncio Bonjour al pasar a segundo plano. Solo hay ~30 s con `beginBackgroundTask` y ejecuciones esporádicas con `BGAppRefreshTask`/`BGProcessingTask` que el sistema decide. | Como Android hoy: sincroniza **mientras la app está abierta** (y al abrirla, contra quien esté). Un Mac o PC encendido hace de punto de encuentro. Ver 4.4 para el caso «nadie encendido». |
| **Permiso de red local en iOS** (y macOS 15+) | Hace falta `NSLocalNetworkUsageDescription` y declarar los 4 tipos en `NSBonjourServices`; el usuario debe aceptar. | Pedirlo con una pantalla que explique el porqué antes del aviso del sistema. |
| **Atajos globales y pines encima de todo en iPhone/iPad** | iOS no deja que una app dibuje sobre otras ni capture la pantalla de otras apps. | En iPad: Stage Manager, Picture in Picture, *Split View*. Capturas: entran por **Share Extension**, Fotos y el botón de captura del sistema. |
| **Capturar la pantalla en iPhone/iPad** | Solo con **ReplayKit Broadcast Extension** (grabación) — no hay «elegir zona» sobre otras apps. | El editor de capturas trabaja sobre capturas ya hechas (Share Extension / Fotos). |
| **Flow Launcher, Explorador, Jump List, Abrir con** | Son de Windows. | Equivalentes: Spotlight/App Intents, Finder (Quick Look, Services), Dock. No sincronizan nada: son de cada aparato. |
| **Ajustes, pines del escritorio, timeline, `.pixpin-lectura`, alarmas** | Hoy no se sincronizan entre Windows y Android tampoco (ver `2026-10-12-que-se-sincroniza.md`). | Igual en Apple. Los atajos y rutas nunca deberían viajar. Ver Fase 5 para los que sí tendría sentido. |
| **Ficheros de Office en el Mac** | Nada que ver con la sincro: el maquetador propio (`pixpin-office`) ya es puro, se muestra igual. | — |

### 4.4 Opcional: iCloud como «buzón» entre aparatos Apple

Sin servidores propios ni cuentas de PixPin (se mantiene la promesa de «sin nube propia»), se
podría usar el **contenedor iCloud Drive de la app** como un aparato más del grupo que «nunca
está apagado»: cada aparato Apple deja ahí sus parches cifrados con la clave del grupo y recoge
los de los demás. Windows y Android seguirían por Wi‑Fi. Es una decisión de privacidad del
usuario (el README promete «sin nube»). **Aprobado como opción apagada por
defecto** (§7.2).

---

## 4bis. Principio rector: máximo rendimiento con mínimos recursos

Pedido del usuario (15-oct): una app **súper optimizada que no sacrifica nada**: gran
rendimiento, poca RAM y poco consumo, con optimizaciones inteligentes. Es la misma regla de la
versión de Windows (la máquina suelo es un Core i3 de 3.ª generación con 4 GB). En Apple se
traduce en reglas concretas que todo el trabajo debe seguir:

1. **Cruzar el puente Rust↔Swift pocas veces y con mucho dentro.** Nada de una llamada por
   figura o por mensaje: lotes. El lienzo cruza como **un solo búfer plano** de números
   (`Vec<f32>` con las órdenes de `motor2d::pintado` empaquetadas) que Swift recorre para
   construir sus `CGPath`; los chats, por páginas.
2. **Nada en memoria que no se vea.** Listas y chats paginados y perezosos (`LazyVStack`,
   miniaturas con `CGImageSourceCreateThumbnailAtIndex` en vez de decodificar la foto
   entera), cachés con tope y que se vacían con el aviso de memoria del sistema
   (`didReceiveMemoryWarning` / `DispatchSource.makeMemoryPressureSource`).
3. **El trabajo pesado, fuera del hilo de la interfaz y con prioridad baja** (`QoS .utility` /
   `.background`): sincronizar, indexar, OCR, transcribir. En portátiles y en iPhone, el
   sistema lo manda a los núcleos de eficiencia.
4. **Lo que hace el hardware, que lo haga el hardware:** vídeo y HEIC por VideoToolbox/ImageIO,
   OCR y recortes por el Neural Engine (Vision), dibujo por Metal/Core Animation. Sin
   decodificadores en software si hay uno en silicio.
5. **Cero copias cuando se pueda:** ficheros con `mmap` (`Data(contentsOf:options:.alwaysMapped)`
   en Swift, `memmap` en Rust para PDFs y planos grandes), imágenes como `IOSurface`
   compartidas entre la captura y el pin.
6. **Nada corriendo sin motivo:** sin temporizadores sondeando; Bonjour y FSEvents avisan.
   La sincronización automática espera a que el usuario se quede quieto (como `QUIETO` en
   Windows) y respeta el modo de bajo consumo (`ProcessInfo.isLowPowerModeEnabled`).
7. **Se mide, no se supone:** cada fase trae sus cifras (RAM en reposo, tiempo de arranque,
   fotogramas al dibujar) con Instruments, y una cifra que empeora se trata como un fallo.

## 5. Plan de implementación

Cada fase deja algo que funciona y se puede probar. Las fases 0 y 1 mejoran también la versión
de Windows y se pueden hacer en Windows/Linux sin un Mac.

### Fase 0 — Preparar el núcleo (sin Mac; 1–2 semanas)

1. ✅ **Hecho:** `atajo`, `entorno` y `union` salen de `pixpin-shell` a un crate nuevo de
   capa 0, **`pixpin-sistema`**, con versión de Windows (la de siempre) y portable (Mac/Linux:
   `~/Library/Application Support`, `localtime_r`, enlaces simbólicos en vez de uniones).
   `pixpin-shell` los reexporta con el mismo nombre, así que el resto del código no cambia.
   `pixpin-store` y `pixpin-proyecto` ya no dependen de `pixpin-shell` y compilan y pasan sus
   pruebas en Linux, incluidas las de sincronizar de verdad por socket.
2. Añadir a la CI un trabajo `ubuntu-latest` (y luego `macos-latest`) que haga
   `cargo test -p` de todos los crates portables. Una prueba guardián (como `capas.rs`) que
   falle si uno de ellos gana una dependencia de `windows`.
3. Arreglos de sincro de §4.2 (NFC, `IpAddr`, rango de versión), con pruebas cruzadas.
4. Separar en `pixpin-codec` y `pixpin-cad` la parte pura (decodificar con `image`, leer DWG/IFC)
   de la de WIC/D3D11, con `cfg`.

**Listo cuando:** `cargo test` de todo el núcleo pasa en Linux y macOS.

### Fase 1 — El puente Rust ↔ Swift (2–3 semanas)

1. Crate nuevo `crates/pixpin-nucleo` (capa 3, sin UI) que expone con **UniFFI** una API
   pequeña y estable: proyectos/chats (listar, leer mensajes, escribir), lienzo (escena +
   `ordenes_de_escena`), documentos (`docs`, `hojas`, `office`), timeline, lecciones y
   **sincronización** (`Sesion`, `Respondedor` con un transporte que Swift implementa sobre
   `NWConnection`).
2. Script `herramientas/xcframework.sh` que compila `aarch64-apple-darwin`,
   `x86_64-apple-darwin`, `aarch64-apple-ios`, `aarch64-apple-ios-sim` y empaqueta
   `PixPinCore.xcframework` + paquete Swift (`Package.swift`).
3. Para la CI: `macos-latest` compila el xcframework.

**Listo cuando:** una app de prueba en Swift lista los proyectos de una carpeta copiada de
Windows y sincroniza un chat con un PC Windows real.

### Fase 2 — PixPin para Mac, lo esencial (6–8 semanas)

Proyecto Xcode en `apps/pixpin-apple/` (un solo *target* multiplataforma).

1. App de barra de menús (`MenuBarExtra`) + ventana de Proyectos/chat en SwiftUI.
2. Captura con **ScreenCaptureKit** y el *overlay* de elegir zona (`NSPanel` a pantalla completa
   por monitor) reutilizando la lógica de `pixpin-ui::overlay`.
3. Pines flotantes con `NSPanel`; imagen, vídeo (AVPlayer), PDF (PDFKit).
4. Lienzo: pintar las `Orden` de `motor2d` con Core Graphics (luego Metal si hace falta).
5. OCR con **Vision** + Live Text sobre pines.
6. Sincronización Wi‑Fi con Windows y Android (Network.framework + `pixpin-nucleo`).
7. Distribución: firmado con Developer ID y **notarizado** (fuera de la Mac App Store al
   principio, porque los atajos globales y la captura libre chocan con el *sandbox*);
   actualizaciones con **Sparkle**.

**Listo cuando:** capturar → pinear → mandar al chat → aparece en el PC y en el móvil.

### Fase 3 — Lo que hace mejor al Mac (4–6 semanas)

App Intents (Spotlight, Atajos, Siri) como sustituto de Flow; Share Extension; Quick Look
Extension; Core Spotlight; transcripción con **SpeechAnalyzer**; asistente con **Foundation
Models** (y llama.cpp/Metal de respaldo); recortar sujeto con Vision; grabación con
AVAssetWriter; Continuity Camera; Handoff; visor 3D en Metal; notas con TextKit 2.

### Fase 4 — iPad (6–8 semanas)

Mismo *target* SwiftUI. Lo nuevo: **Apple Pencil** (presión, inclinación, predicción,
*hover*, Pencil Pro) entrando en `motor2d`; Stage Manager y varias ventanas; PiP para vídeos;
teclado y trackpad; sincronización solo en primer plano (§4.3) con aviso claro de con quién se
sincronizó por última vez.

### Fase 5 — iPhone (3–4 semanas)

Interfaz compacta: chat, galería, lectura y anotación sencilla; Share Extension como forma
principal de meter cosas; widgets y Live Activity de «sincronizando». Si se decide, sincronizar
algunos **ajustes** comunes (tema, días de caducidad; nunca atajos ni rutas) en un
`sincro/ajustes.json` «gana el más nuevo por clave», acordado también con Windows y Android.

### Fase 6 — Buzón en iCloud (§4.4)

**Aprobado** como opción apagada por defecto (§7.2). Se adelanta: el formato en la Fase 1 y la
activación en el Mac en la Fase 3.

### Resumen de esfuerzo

| Fase | Duración aprox. (1 persona) | Necesita un Mac |
|---|---|---|
| 0 Núcleo portable | 1–2 sem | No |
| 1 Puente UniFFI | 2–3 sem | Sí (para el xcframework) |
| 2 Mac esencial | 6–8 sem | Sí |
| 3 Mac mejorado | 4–6 sem | Sí |
| 4 iPad | 6–8 sem | Sí + iPad |
| 5 iPhone | 3–4 sem | Sí + iPhone |

Requisitos: un Mac con Apple Silicon y Xcode 26, cuenta de Apple Developer (99 USD/año) para
firmar, notarizar y publicar en iOS.

---

## 6. Primeros pasos concretos en esta rama

Lo que se puede empezar ya, sin Mac y sin riesgo para la versión de Windows:

1. ✅ Fase 0.1: crate `pixpin-sistema`; `pixpin-store`/`pixpin-proyecto` compilan en Linux.
2. ✅ Fase 0.2: trabajo de CI en Linux para el núcleo portable + guardián de dependencias.
3. Fase 0.3: normalización NFC en las rutas de la sincronización, con pruebas.

## 7. Decisiones del usuario (15-oct-2026)

1. **Distribución:** el Mac se publica **fuera de la App Store** al principio (Developer ID +
   notarizado + Sparkle). La App Store se valora más adelante.
2. **iCloud como buzón: sí, como opción apagada por defecto.** Motivo del usuario: alguien que
   solo tiene Mac (y quizá iPhone/iPad), sin Windows encendido, también tiene que poder
   sincronizar. Por eso se adelanta: el formato del buzón se diseña en la Fase 1 (para que el
   núcleo lo soporte desde el principio) y se activa en la app de Mac en la Fase 3, antes del
   iPad, en vez de quedar como Fase 6. El contenido va cifrado con la clave del grupo: iCloud
   solo guarda bytes que no puede leer.
3. **Versión mínima:** **macOS 15**; iPadOS/iOS 18 cuando lleguen. Lo de macOS 26
   (SpeechAnalyzer, Foundation Models) se usa si está y, si no, el respaldo (Whisper/llama.cpp).
4. **Orden:** **Mac primero**; iPad y iPhone después.
