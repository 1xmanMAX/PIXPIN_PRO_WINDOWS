# PixPin para Mac, iPad e iPhone

Zona de trabajo de la versión de Apple. Plan completo y decisiones:
[`docs/investigacion/2026-10-15-mac-ipad-iphone.md`](../docs/investigacion/2026-10-15-mac-ipad-iphone.md).

Todo lo de aquí es una **copia tal cual** (copiar y pegar, sin reescribir) de `crates/` y `apps/`
del 15-oct-2026. Se irá arreglando y uniendo poco a poco. La versión de Windows no se toca desde
aquí: sigue viviendo en `crates/` y `apps/`.

## Las tres zonas

| Carpeta | Qué hay | Qué se hace con ello |
|---|---|---|
| [`nucleo/`](nucleo) | Los 22 crates que **compilan sin Windows**: formato de proyectos, sincronizar, lienzo, Word/Excel/PowerPoint/EPUB, timeline, lecciones, la lógica de la interfaz, imagen (`pixpin-codec`), PDF (`pixpin-pdf`), planos DWG/DXF y BIM (`pixpin-cad`, `pixpin-bim`), el asistente (`pixpin-ia`) y el puente con Swift (`pixpin-nucleo`) | **Se usa tal cual.** Es un espacio de trabajo de Rust propio (`apple/Cargo.toml`) que compila y pasa sus pruebas fuera de Windows. Será `PixPinCore.xcframework` para Swift (fase 1). |
| [`por-adaptar/`](por-adaptar) | Crates **casi** puros, con una parte de Windows: `pixpin-voz` (SAPI, ONNX de System32), `pixpin-pila`, `pixpin-lanzador`, `pixpin-aligerar` | **Se queda en Rust**: separar la parte pura con `#[cfg(windows)]` o quitándola, y pasar cada crate a `nucleo/` cuando compile sin Windows. No compila todavía (fuera del espacio de trabajo). |
| [`por-traducir/`](por-traducir) | Lo atado a Windows: `pixpin-shell` (Win32), `pixpin-render` (Direct2D), `pixpin-capture`, `pixpin-record`, `pixpin-ocr`, `pixpin-audio`, `pixpin-web` (WebView2), `pixpin-pin`, `pixpin-notas` (RichEdit; sus fórmulas `$…$`, que son lógica pura, ya viven en `nucleo/pixpin-docs/src/formulas.rs`) y la app `pixpin` | **Referencia para escribir la versión en Swift** con las herramientas de Apple (ScreenCaptureKit, Vision, PDFKit, AVFoundation, `NSPanel`…). No compila ni se compilará: se lee y se traduce pieza a pieza. |

Qué herramienta de Apple sustituye a cada pieza: §3 del plan.

Y además, el puente con Swift:

| Carpeta | Qué hay |
|---|---|
| [`nucleo/pixpin-nucleo`](nucleo/pixpin-nucleo) | La API para Swift con **UniFFI**: abrir una carpeta de datos (`Nucleo`), chats y mensajes **por páginas**, escribir una nota, guardar una nota de voz y su texto (`adjuntar_voz`, `poner_transcripcion`), editar un lienzo (`EditorLienzo`), ajustes, lecciones y el asistente, adjuntar un fichero (`adjuntar`: se copia con `guardar_adjunto` y la clase de `clase_de_nombre`, como al soltarlo en Windows) y resolver su ruta (`ruta_absoluta`, `imagenes_del_lienzo`), el lienzo como **un solo búfer plano** de `f32` (formato en `src/lienzo.rs`), sincronizar por TCP con IPv4 e IPv6 (unirse a un grupo, una vuelta con la galería, escuchar con varios hilos, «Enviar a…», envío con código y chats compartidos con personas: [`docs/sincro.md`](docs/sincro.md)), la **galería de capturas** por páginas con su caducidad (conservar, 7 días más, borrar y devolver: los ficheros de Windows), el **timeline** (días y momentos por páginas), las **tareas** de todos los chats (marcar, apuntar en el Inbox) **buscar en todo** (por páginas y cancelable) y los **planos y modelos 3D** para Metal (`Nucleo::abrir_plano` → `Plano` por capas, `Modelo3dApple`, la cámara `camara_*`: búferes de bytes con la disposición `#[repr(C)]` de `pixpin-cad`, documentada en `src/planos.rs` y `src/modelos3d.rs`). |
| [`PixPinCore/`](PixPinCore) | El paquete Swift (`Package.swift`, macOS 15 / iOS 18): los enlaces generados y el `PixPinCoreFFI.xcframework` (no se versiona). |
| [`herramientas/xcframework.sh`](herramientas/xcframework.sh) | En un Mac: compila las cuatro plataformas, genera los enlaces y hace el xcframework. |

## Compilar y probar el núcleo

```sh
cd apple
cargo test --workspace -- --test-threads=1
```

El puente para Swift:

```sh
apple/herramientas/xcframework.sh   # en un Mac con Xcode: deja PixPinCore listo para Xcode
# en Linux, solo los enlaces (para revisar la API):
cargo build -p pixpin-nucleo --lib
cargo run -p pixpin-nucleo --features bindgen --bin uniffi-bindgen -- \
    generate --library target/debug/libpixpin_nucleo.so --language swift --out-dir target/enlaces
```

El xcframework se compila con el perfil `ffi` (el `release` con `panic = "unwind"`): así un
pánico de Rust llega a Swift como error en vez de cerrar la app.

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

- [ ] Fase 0.3: arreglos de sincronizar: tildes NFC ✅ (`almacen::nombre_seguro`), IPv6 ✅ en el Mac
      (`pixpin-nucleo/src/red.rs`: `IpAddr`, enlace local con `%interfaz`, IPv4 primero). Versión del
      protocolo: diseño compatible preparado y **apagado** (`src/version.rs`, [`docs/sincro.md`](docs/sincro.md));
      el cable sigue en 4 estricta hasta cambiar `Hola` a la vez en Windows y Android.
- [ ] Fase 1: el puente Rust ↔ Swift.
  - [x] Crate `pixpin-nucleo` con UniFFI: carpeta de datos, chats, mensajes por páginas
        (índice de líneas en caché con tope), escribir una nota, lienzo en un búfer plano,
        unirse a un grupo / sincronizar / escuchar por TCP, errores como `enum`.
  - [x] Enlaces de Swift generados + `PixPinCore/Package.swift` + `herramientas/xcframework.sh`.
  - [ ] Ejecutar `xcframework.sh` en un Mac y una app de prueba que lista los proyectos de una
        carpeta copiada de Windows y sincroniza un chat con un PC Windows real.
  - [ ] Lector del búfer del lienzo en Swift (`CGPath` en una pasada).
  - [x] Descubrimiento con Bonjour (`app/Sources/PixPinKit/Sincro/`): `NWBrowser` busca
        `_pixpin._tcp` y filtra por el TXT `g`; la sonda `PING`/`PONG` con `NWConnection` da
        host y puerto; el anuncio es `DNSServiceRegister` sobre el puerto de la `Escucha` de
        Rust (47474 o el que dé el sistema) con el TXT de `Nucleo::anuncio` (igual que
        Windows). Sincroniza solo al aparecer un aparato del grupo y 30 s después de un
        cambio, salvo en modo de bajo consumo.
  - [x] Sincronizar completo (`docs/sincro.md`): la **galería de capturas** en cada vuelta
        (`galeria_sincro.rs`, como `CapturasDelPc`), **«Enviar a…»** un aparato del grupo
        (`suelto`: al chat abierto, al lienzo si es foto o a Mensajes guardados; menú en la
        barra), **enviar y recibir con código** de seis cifras (`envio.rs`, ventana «Enviar y
        recibir por Wi-Fi»), **chats compartidos con personas** (`compartidos.rs`: compartir,
        invitar con QR de CoreImage, entrar, sincronizar con la valla, quitar/salir; claves en
        `sincro/persona.json` como Windows) y la `Escucha` con **4 hilos** (una sonda no espera
        a una vuelta; una sola vuelta por carpeta con `protocolo::ocupar`).
  - [ ] Transporte de Swift sobre `NWConnection` (hoy socket BSD de Rust, IPv4 e IPv6).
  - [ ] El editor del lienzo no mete aún las fotos que llegan con «Enviar a…»
        (`FrenteMac::Lienzo` está en Rust; la app aún no lo pone).
  - [ ] Documentos (`docs`, `hojas`, `office`) en la API.
  - [ ] Transporte de Swift sobre `NWConnection` (hoy socket BSD de Rust, con IPv4 primero);
        el `suelto`, la galería y los chats con personas. La `Escucha` atiende una conexión
        cada vez: mientras dura una vuelta, una sonda de otro espera.
  - [x] Documentos en la API (15-oct): `src/documentos.rs` (la tinta de cada hoja de un PDF,
        Word o PowerPoint con `Nucleo::anotar_hoja` como un `EditorLienzo` en las unidades de
        la hoja, guardada como Windows y Android: `anot-<uid>-p<n>` y su marco `.hoja`;
        marcadores y espacios en `anot-<uid>.marcas`/`.espacios`; aumento y pagina en
        `<doc>.pixpin-lectura`, que no viaja), `src/notas.rs` (`editar_nota`, el Markdown vivo
        de `md_vivo` y las formulas `$…$`), `src/hojas.rs` (`HojaDeCalculo`: mini-app `tabla` y
        `tablas/<ref>.json` del movil, formulas de `pixpin-proyecto`), `src/office.rs` (Word y
        PowerPoint paginados por `pixpin-office`, midiendo con Core Text por `MedidorLetras`) y
        `src/aligerar.rs` (pdfsqueeze: `aligerar_pdf` y `aligerar_en_su_sitio` como Windows).
  - [x] Lecciones en la API (`src/lecciones.rs`, por paginas con cache por fechas), ajustes
        de `pixpinmax.toml` que valen en los dos sistemas (`src/ajustes.rs`: dias de las
        capturas, sincronizacion sola y presencia, con `guardar_conservando`) y el asistente
        (`src/asistente.rs`: `ia_catalogo_para`/`ia_llamar` de `pixpin_ia::puente` y un turno
        con un `llama-server` del usuario en `127.0.0.1:8080`).
  - [ ] Buscar en todo usa un recorrido propio (`src/buscar.rs`): pasar al de `pixpin-lanzador`
        cuando ese crate compile fuera de Windows.
  - [x] CI: `.github/workflows/apple.yml` (macOS 15, Xcode 16.4) compila el xcframework, pasa
        `swift test` (190 pruebas, 15-oct) y sube `PixPin.app` universal como artefacto
        «PixPin-mac» (SwiftPM) y, con XcodeGen, «PixPin-mac-extensiones» (App Intents,
        Compartir y Quick Look, firmadas de dentro afuera). Los binarios van sin simbolos y el
        núcleo (`PixPinCore`, dinámico) va **una sola vez** en `Contents/Frameworks` para la
        app y sus extensiones: 102 MB (42 MB en zip) con todo. La CI abre la app y comprueba
        que sigue viva a los 6 s.
- [ ] Fase 2: app de Mac en SwiftUI/AppKit (`apple/app/`). Hecho y en verde en la CI:
  - [x] Barra de menús, ventana de Proyectos, chat por páginas, notas, crear proyecto.
  - [x] Lector del búfer del lienzo y pintor con Core Graphics.
  - [x] Captura (Ctrl+Option+X, ScreenCaptureKit, ventanas, lupa) → copiar, guardar en
        `capturas/`, pinear, al chat, sacar texto.
  - [ ] Captura como Windows (`app/Sources/PixPinKit/Captura/`, falta verla en verde en la CI):
        anotar encima sin otra ventana (1–5 lápiz, flecha, rectángulo, texto y mosaico con
        `CIPixellate`; colores, grosores, ⌘Z; se quema en la imagen con Core Graphics), barra de
        modos `Z V P S G L T`, medidas tecleadas, 16:9 · 4:3 · 1:1, `R` repetir la última zona
        (por pantalla), lupa con HEX/RGB y `C`; con scroll (`CosidoScroll`, la receta de
        `pixpin-codec/cosido.rs`; baja solo con Accesibilidad), GIF (10 fps, 12 s, sin
        fotogramas repetidos, ImageIO → `capturas/captura-…gif`), «Al chat» con buscador y
        teclas 1–6 (`captura-<ms>.png`), pila de capturas (⌘V pega la tanda) y gestos con
        Option en todo el Mac (apagados de fábrica; `CGEventTap` con Accesibilidad). El pin en
        vivo (`L`, Option + central) es un gancho (`alPinEnVivo`) para cuando existan.
  - [x] Pines flotantes (imagen, PDF, vídeo), Live Text y OCR con Vision.
  - [x] Pines como en Windows (`app/Sources/PixPinKit/Pines/`): **pin en vivo** (tecla `L` al
        capturar o `GestorPines.pinearEnVivo`; `SCStream` de la zona con `IOSurface` directo a la
        capa, 30 fps con cambios y 2 fps quieta por `SCFrameStatus.idle`, recuadro azul fuera de
        las capturas, pausa con doble clic, congelar y manejar a distancia con `CGEvent` si hay
        permiso de Accesibilidad), **barra común** con chapitas de atajo, panel **«Pines
        abiertos»** por colores (⌃2 mostrar/ocultar, cerrar todos con deshacer), **guías azules**
        e imán al soltar, opacidad con Mayús/Opción + rueda, **anotar** (lápiz, flecha,
        rectángulo, texto, marcador; capa o fijado en la foto) y **pines de herramienta**: nota
        Markdown y las mini-apps tareas, contador, ruleta y gastos (documento de
        `pixpin_proyecto::mini` por `pixpin-nucleo/src/miniapps.rs`, guardado en un mensaje
        `MINIAPP` de «Mensajes guardados»). Faltan cronómetro, temporizador y alarma en pin.
  - [x] Adjuntos en el chat: miniaturas con ImageIO y caché con tope, Quick Look, soltar y ⌘V.
  - [x] Sincronizar por la Wi‑Fi (Bonjour) con Windows y Android.
  - [x] Notas de voz (`app/Sources/PixPinKit/Voz/`, en verde en la CI):
        microfono con `AVAudioEngine` → `.m4a` AAC-LC **mono** (el formato de Windows y
        Android; 32 kbps como el movil, 44,1/48 kHz como la tarjeta), `voz_<ms>.m4a` con
        `duracionMs` y `picos` (0–32767, ≤256, uno cada 50 ms) en la misma linea
        (`Nucleo::adjuntar_voz`); el texto en el aparato con `SFSpeechRecognizer`
        (`requiresOnDeviceRecognition`, es-ES o el del sistema) guardado como Windows
        (`Nucleo::poner_transcripcion`: `[m:ss] ...` en `transcripcion` y `estadoDelTexto`).
        Burbuja `VOZ` con un solo `AVAudioPlayer` para toda la app.
  - [x] Grabar la pantalla (`app/Sources/PixPinKit/Grabar/`, en verde en la CI):
        «Grabar pantalla» (⌃⌥R) elige la zona con la captura (Intro o ⌘G graba),
        `SCStream` (YUV 4:2:0, 30 fps) → `AVAssetWriter` H.264 por hardware (sonido del
        sistema opcional, AAC) → `capturas/captura-AAAAMMDD-HHMMSS.mp4`; punto rojo en la
        barra de menus y «Detener grabación». Sin GIF todavia.
  - [x] Lienzo editable (en verde en la CI): `EditorLienzo` en
        `pixpin-nucleo/src/editor.rs` (el `Gesto` y la `Escena` del motor, la goma como el
        movil, lo de encima como `dibujo::pintar::ordenes_encima`, guardar con
        `con_escena` + `escribir` igual que Windows) y `app/Sources/PixPinKit/Lienzo/`
        (`NSView` con pellizco, dos dedos, presion de tableta y Force Touch, ⌘Z/⇧⌘Z, borrar,
        guardado solo 1 s despues del ultimo cambio). Se abre con doble clic en un `DIBUJO`
        o con «Lienzo nuevo» en el chat. Con el lazo, las de medir (cota, calibrar con su
        cajetin, escala grafica), las cuatro de construir (bote, recortar, extender, punto;
        `editor_construir.rs`), el gesto de pararse (`RelojFormaRapida`: una sola espera de
        una vez, reprogramada al pasar el temblor y cancelada al soltar), imagenes pegadas o
        soltadas (PNG por ImageIO → `imagenes/<fileId>` del chat y su entrada en `files`,
        como Windows y el movil; `editor_imagenes.rs`), la barra de estilo (trazo, fondo,
        relleno, grosor, linea; `editor_estilo.rs`), la ventanita del metodo de entrada en el
        cursor del texto y **lapidas** (`isDeleted`) para lo nacido, guardado y borrado, para
        que la fusion de `pixpin-sincro` no lo resucite.
  - [x] Ajustes (⌘,; `app/Sources/PixPinKit/Ajustes/`): tema, atajo de capturar (se graba y
        se vuelve a registrar en Carbon; UserDefaults), dias de la galeria y sincronizacion
        sola (en `pixpinmax.toml`, como Windows), carpeta de datos y «Abrir al iniciar sesion»
        (`SMAppService.mainApp`).
  - [x] Lecciones (`Lecciones/`): lista por paginas, buscar, etiquetas (propuestas ✨),
        repaso y borrar, guardadas como en Windows y Android (`.leccion` + mensaje `ARCHIVO`).
  - [ ] Planos y BIM (`Planos/`, a falta de probarlo en un Mac de verdad): visor de DWG/DXF en
        Metal (los HLSL de Windows en MSL, compilados al vuelo con `makeLibrary(source:)` y las
        tuberías guardadas), parado hasta que algo cambia; trackpad, capas, regla con enganche
        (`Plano::enganchar`), cotas y marcos en la capa `anot-<uid>` + `.hoja` como Windows y
        Android, imprimir en vectores (`NSPrintOperation`). Visor 3D (IFC, Revit, LandXML, puntos,
        DWG en 3D) con la cámara de Rust, niveles, categorías, caja de sección con tapas y
        elegir con un rayo en Rust. Se abren con doble clic en el chat y con «Abrir con» del
        Finder. Falta: anotar a mano (la tinta de la capa solo se ve), la caché `.pxcad` de
        Windows (aquí se lee por capas, sin caché) y las vistas guardadas del 3D.
  - [x] Asistente (`Asistente/`): Foundation Models con las herramientas del puente si hay
        macOS 26 (compila fuera con el SDK de macOS 15), si no un `llama-server` propio en
        127.0.0.1:8080, si no «no hay motor». Con el `llama-server`, **en vivo**: Rust lee el
        SSE y manda el texto a Swift por un `OyenteIa` (interfaz de UniFFI implementado en
        Swift) en tandas de ≤60 ms, y «Parar» lo corta (`CorteIa`). Las claves de la nube, en
        el **Llavero** (`LlaveroDelSistema`: `SecItem*`, servicio `PixPinMax.asistente`),
        instalado al arrancar por el `LlaveroApp` del puente (`src/llavero.rs`).
  - [x] Galería de capturas (rejilla perezosa, caducidad, conservar, 7 días más, borrar con
        deshacer, Vista Rápida), Timeline (días → momentos), Tareas de todos los chats y Buscar
        en todo (⌘F en Proyectos, espera de 200 ms y cancela la anterior).
  - [x] Barrer las capturas caducadas a la Papelera del Mac (`Caducidad/`): al arrancar y
        luego como mucho cada hora con un solo `NSBackgroundActivityScheduler` tolerante; lo
        que se va lo decide Rust con las reglas de Windows (`Nucleo::capturas_caducadas`),
        Swift lo mueve con `FileManager.trashItem` y Rust olvida lo que ya no esta
        (`olvidar_capturas_ausentes`). El plazo es el de `pixpinmax.toml`.
  - [x] Buscar → «Abrir en Proyectos» abre el chat en la pagina del mensaje (centrada en el,
        `Almacen.abrir(chat:en:)`), lo marca y se desplaza hasta el; lo de antes y lo de
        despues se pide al subir o bajar.
  - [x] Proyecto de Xcode con extensiones (`apple/xcode/project.yml`, XcodeGen):
        App Intents en la app (Capturar zona, Nueva nota en proyecto, Buscar en PixPin,
        Sincronizar ahora; frases en español para Siri, Atajos y Spotlight), extension de
        **Compartir** (mandar a un proyecto, con su caja de arena y la excepcion para
        `~/Library/Application Support/PixPinMax/`) y **vista previa de Quick Look** de
        `.excalidraw` y `.pixpin2d` (el mismo lector y pintor del lienzo). La CI la monta con
        `herramientas/app.sh xcode` (artefacto «PixPin-mac-extensiones»); `app.sh spm` sigue
        siendo la app sola de SwiftPM (artefacto «PixPin-mac»). Fuera de `apple/app` para
        que el paquete local no contenga al proyecto.
  - [ ] Barrer las capturas caducadas a la Papelera del Mac (`FileManager.trashItem`).
  - [x] Documentos (`app/Sources/PixPinKit/Documentos/`, falta verlo en verde en la CI):
        Word, Excel, PowerPoint, ODF, RTF, CSV y EPUB al instante en un pin de Vista Rapida
        (`QLPreviewView`); el lector como pin (PDFKit) con lapiz, resaltador, goma, marcadores,
        espacios para anotar y pantalla completa (⌃⌘F o F11), tambien para Word y PowerPoint
        paginados como Office; el editor de notas Markdown (`NSTextView`, formulas compuestas);
        el de hojas de calculo (`NSTableView`, formato de celdas); aligerar PDF (menu del
        mensaje y al entrar al chat, ajuste `[pdf]`) y «Abrir con PixPin» desde el Finder.
        Falta: la tinta de un EPUB (en Windows va por columna reflujada, `anot-<uid>` con su
        `.maqueta`), elegir texto de las hojas de Office con su enlace, y juntar la tinta si
        cambia en el disco con el lector abierto (`lector_pdf/al_dia.rs`).
- [ ] `por-adaptar/` → `nucleo/`:
  - [x] `pixpin-codec` (decodificar con `image`; WIC, vista reducida y portapapeles tras
        `#[cfg(windows)]`; ImageIO y `NSPasteboard` desde Swift).
  - [x] `pixpin-pdf` (escribir, unir, texto, plano; ver con `Windows.Data.Pdf` y aligerar tras
        `#[cfg(windows)]`; PDFKit desde Swift).
  - [x] `pixpin-cad` y `pixpin-bim` (ver abajo).
  - [ ] `pixpin-voz`, `pixpin-pila`, `pixpin-lanzador`, `pixpin-aligerar`.
- [ ] CI: el trabajo `apple-nucleo` ya prueba `pixpin-codec` y `pixpin-pdf` (usa
      `--workspace`). Falta un paso que haga
      `cargo check --target x86_64-pc-windows-msvc` de los crates adaptados para que lo de
      `#[cfg(windows)]` no se pudra (la lista `nucleo-portable` es de `crates/`, no de aquí).
- [x] `pixpin-ia` → `nucleo/` (15-oct): compila y pasa sus pruebas sin Windows. Lo de Windows
      va con `cfg(windows)`: bajar por HTTPS (WinHTTP de `pixpin-voz`), la nube por WinHTTP, el
      «trabajo» que mata a `llama-server` y DPAPI. Fuera de Windows: cliente HTTP/1.1 propio
      solo hacia `127.0.0.1` (llama-server y las pruebas de la nube con su servidor falso),
      `llama-server` en su grupo de procesos y muerto al soltarlo, y el trait
      `nube::claves::Llavero` para que Swift guarde las claves en el Llavero
      (`instalar_llavero`; sin él no se guarda nada, nunca en claro). Las herramientas en JSON
      para cualquier motor (Foundation Models desde Swift): `pixpin_ia::puente`.
  - [ ] Swift: bajar modelos y motor con URLSession (luego `descarga::verificar`), HTTPS de la
        nube con URLSession (el `Llavero` con Keychain ya esta), y Foundation Models con
        `puente::catalogo_para`/`puente::llamar` (llama.cpp con Metal de respaldo; en iPad e
        iPhone enlazado, sin `llama-server`).
  - [ ] Catálogo del motor para Mac: `modelos::LLAMA_CPP` y `descarga` siguen siendo el zip de
        Windows (`llama-server.exe`).
  - [ ] Si PixPin muere de golpe en Mac, `llama-server` queda huérfano (no hay Job Objects).
  - [ ] El banco (`tests/banco.rs`) no guarda PNG aquí (pintaba con `pixpin-pdf`/`pixpin-shell`).
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
