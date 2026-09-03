# El PixPin original por dentro: cómo está estructurado y en qué se apoya

**Fecha:** 2026-09-03 · **Material:** el portable `PixPin_win_3.4.3.2` que el
usuario dejó en `proyectos de referencia/PixPin` (150 MB, 125 ficheros), más
su propio fichero de ajustes en `%LOCALAPPDATA%\PixPin\Config`.

## 0. Qué se ha mirado y qué no

PixPin es software **propietario** de DepthPixel. Este proyecto es una
implementación personal e independiente y no puede llevar nada suyo.

- **Sí se ha hecho:** listar los ficheros y sus tamaños, identificar con qué
  está construido cada módulo, leer los nombres de las bibliotecas de código
  abierto que lleva incrustadas, leer la superficie pública de su API de
  guiones y leer el fichero de ajustes del propio usuario.
- **No se ha hecho ni se hará:** desensamblar, descompilar ni reconstruir su
  código. `PixPin.exe` es C++ compilado; ahí no hay código que leer, hay
  máquina, y reconstruirlo para copiarlo sería exactamente lo que no
  queremos.

Todo lo que sigue es **cómo está organizado** y **sobre qué está
construido**, que es información de estructura, no de implementación. Sirve
para decidir qué escribimos nosotros y qué resolvemos con una biblioteca
libre.

## 1. La foto general

| | PixPin original | PixPin Max |
|---|---|---|
| Distribución portable | 150 MB, 125 ficheros | 2,2 MB, 1 fichero |
| Lenguaje e interfaz | C++ con MSVC, **Qt 5 + QML** | Rust, Direct2D + DirectComposition |
| Módulos propios | 30 DLL + 2 ejecutables | 17 crates, 1 ejecutable |
| Dependencias nativas de terceros | Qt 5 (25 MB), ONNX Runtime, OpenSSL, FFmpeg, OpenCV | ninguna |
| Modelos de IA que distribuye | 34 MB | ninguno |
| Procesos | principal + auxiliar + gestor de fallos | uno |

## 2. Cómo reparte el código: treinta módulos por dominio

Lo más instructivo del original no es lo que hace, es **cómo lo separa**. No
es un ejecutable monolítico: cada dominio vive en su propia DLL, con nombres
que declaran la responsabilidad. El reparto coincide casi uno a uno con
nuestros crates, lo que confirma que la arquitectura por capas que llevamos
va por donde debe.

| Su módulo | Tamaño | Qué hace | Nuestro equivalente |
|---|---|---|---|
| `PixPin.exe` | 21,9 MB | Aplicación, interfaz, orquestación | `apps/pixpin` |
| `PixWinCapture` | 0,07 MB | Captura por **Desktop Duplication** (dxgi) | `pixpin-capture` |
| `PixWin32CaptureCore` | 0,13 MB | Captura por **Windows Graphics Capture** | `pixpin-capture` |
| `PixScreenManager` | 0,08 MB | Monitores, DPI, disposición | `pixpin-geom::monitores` |
| `PixKeyMouse` | 0,07 MB | Ganchos de teclado y ratón | `pixpin-shell::gestos` |
| `PixConfiguration` | 0,32 MB | Ajustes persistentes | `pixpin-store::ajustes` |
| `PixWidget` | 0,91 MB | Controles de interfaz propios | `pixpin-ui` |
| `PixStyle` | 0,13 MB | Tema y estilos | `pixpin-render` |
| `PixActionsBar` | 0,10 MB | La barra de acciones tras capturar | `pixpin-ui::barra` |
| `PixColorPalette` | 0,09 MB | Selector de color | pendiente (S3-D) |
| `PixLottie` | 0,35 MB | Animaciones de la interfaz | no tenemos |
| `PixNotification`, `PixWindowNotify` | 0,18 MB | Avisos | no tenemos |
| `PixAVCodec` | 7,63 MB | **Codificación** de vídeo y GIF | no tenemos |
| `PixMovie` | 0,53 MB | Reproducción | `pixpin-pin::video` |
| `PixOCR2` | 4,64 MB | Reconocimiento de texto | no tenemos |
| `PixVision` | 5,49 MB | Visión por computador | no tenemos |
| `PixFormulaRec` | 2,97 MB | Fórmulas a LaTeX | no tenemos |
| `PixLatex2MathML` | 1,20 MB | LaTeX a MathML | no tenemos |
| `PixModelRunner` | 0,09 MB | Envoltorio de ONNX Runtime | no tenemos |
| `UiRegionDetector` | 0,09 MB | Región bajo el cursor | `pixpin-shell::uia` |
| `PixUtils`, `PixSystemUtils` | 2,55 MB | Utilidades comunes | repartido |
| `PixProgramManage` | 0,06 MB | Lista de programas a ignorar | no tenemos |
| `PixAuth` | 3,55 MB | Cuenta y licencia | **no queremos** |
| `PixNetwork`, `PixDownload` | 0,17 MB | Red y actualizaciones | **no queremos** |
| `PixStat` | 0,04 MB | Telemetría | **no queremos** |
| `PixPinIcon`, `PixPinContextMenu` | 0,53 MB | Integración con el Explorador | no tenemos |
| `PixPinTutorial` | 4,07 MB | Tutorial incrustado | no tenemos |
| `PixPinAuxiliary.exe` | 0,71 MB | Proceso auxiliar | no tenemos, ni falta |

**La lección:** separar por dominio, no por capa técnica. Cada módulo tiene
un nombre que dice qué responsabilidad tiene y se podría quitar sin tocar
los demás. Es lo mismo que hacemos con los crates, y valida el test de capas
que ya tenemos en `apps/pixpin/tests/capas.rs`.

## 3. En qué se apoya

Identificado leyendo los nombres de bibliotecas incrustados en cada módulo.

| Módulo | Sobre qué está construido |
|---|---|
| `PixAVCodec` | **FFmpeg** (libavcodec, libavformat, libavutil) + **x264** + **OpenH264** + OpenCL |
| `PixVision` | **OpenCV** + **Eigen** + OpenCL |
| `PixOCR2` | **OpenCV** + **Eigen** + OpenCL, con modelos propios cifrados |
| `PixModelRunner` | **ONNX Runtime** (13,8 MB) |
| Toda la interfaz | **Qt 5** (Core, Gui, Widgets, Qml, Network, Multimedia) |
| Atajos globales | **qxtglobalshortcut**, biblioteca libre de Qt |
| Red y licencia | **OpenSSL** 1.1 |
| Fallos | **Crashpad**, de Google |

Los modelos que distribuye: dos ficheros de 20 MB y 9,4 MB **cifrados** (los
del reconocimiento de texto), un `paragraph_recognition.onnx` de 3 MB
exportado desde PyTorch 2.11 que es una red de grafos para agrupar párrafos,
y dos modelos Caffe pequeños, un detector y un **aumentador de resolución**.

Nota de licencias que nos afecta: **x264 es GPL-2.0** y FFmpeg es LGPL o GPL
según cómo se compile. Si algún día queremos grabar vídeo, esa es una
decisión con consecuencias, y hay una salida limpia: Media Foundation ya
viene en Windows y ya la usamos para reproducir.

## 4. Su decisión de diseño más copiable: el registro de comandos

Sus atajos **no están cableados**. Cada acción del programa es una entrada de
un registro con esta forma:

- un **nombre de guion** estable, por ejemplo `pixpin.hideOrShowAllPin()`
- un **título traducido** que se ve en la interfaz
- un **atajo opcional**, que es solo un campo más y puede estar vacío
- una marca de **si sale en el menú de la bandeja**
- una marca de si es una acción del sistema o del usuario

Se ve tal cual en el fichero de ajustes: `pixpin.hideOrShowAllPin()` atado a
`Ctrl+2` y visible en la bandeja. Ellos van un paso más allá y lo interpretan
con el motor de guiones de QML, que es lo que les permite que el usuario se
invente acciones nuevas.

**Los 27 comandos que expone**, sacados de la superficie pública del
ejecutable:

| Comando | Lo tenemos |
|---|---|
| `screenShot(ShotAction)` | sí |
| `screenShotAndEdit` | no (capturar y entrar directo a anotar) |
| `directScreenShot`, `directScreenShotSpRect` | no (capturar sin overlay, a una región fija) |
| `delayScreenShot` | no (captura con retardo) |
| `longScreenShot` | sí (captura con scroll) |
| `gifScreenShot`, `gifShotPause` | no (**grabar GIF**) |
| `openCustomScreenShot`, `setScreenShotRect`, `getSpRect` | no (regiones guardadas) |
| `genRectUnderCursor` | sí (detección por UI Automation) |
| `pinFromClipBoard` | sí |
| `pinFromFiles`, `pinSelectedFile` | parcial (falta pinear la selección del Explorador) |
| `closeAllPin` | no |
| `hideOrShowAllPin` | parcial (por grupos) |
| `restoreLastClosedPin` | no (**restaurar el último pin cerrado**) |
| `switchPinGroup` | sí |
| `trigMousePenetration` | parcial (solo en la capa viva, no en los pines) |
| `toggleWindowTopmostUnderMouse` | no |
| `disableShortcuts`, `isDisableShortcuts` | no (silenciar los atajos un rato) |
| `translateClipboard` | no |
| `openConfigurationWindow` | no (es nuestra S3-D) |
| `moveCursorBy`, `runSystem` | no |

## 5. Sus piezas propias, una a una: qué hacer con cada una

Para cada cosa que ellos tienen y nosotros no, la pregunta es: ¿la
escribimos, la resolvemos con algo libre, o no la queremos?

### 5.1 Reconocimiento de texto — `PixOCR2` (4,6 MB + 30 MB de modelos)

Sobre OpenCV y Eigen, con modelos cifrados propios.

**Nuestra opción, y es buena: `Windows.Media.Ocr`.** Viene dentro de Windows
10 y 11, funciona sin conexión, cubre unos veinticinco idiomas y **no hay que
distribuir ni un byte de modelo**. Pasa de 34 MB a cero. La calidad es
suficiente para leer texto de una captura de pantalla, que es texto limpio y
renderizado, no una foto torcida.

Si algún día no bastara, la alternativa libre sería PaddleOCR (Apache-2.0)
sobre ONNX Runtime, pero eso son 14 MB de motor más los modelos, y no lo
justifica nada de lo que hacemos hoy.

### 5.2 Codificación de vídeo y GIF — `PixAVCodec` (7,6 MB)

FFmpeg más x264. Es lo que les permite grabar la pantalla en vídeo y en GIF.

**Nuestra opción: Media Foundation para vídeo** (ya la usamos para
reproducir, y trae codificador H.264 por hardware en cualquier equipo
moderno) **y un codificador GIF propio en `pixpin-codec`**. Un GIF es una
paleta de 256 colores y compresión LZW; es de las cosas más agradecidas de
escribir y lo tenemos todo para hacerlo. Si preferimos no escribirlo, el
crate `gif` es MIT.

Con eso evitamos meter FFmpeg y x264, que nos obligarían a decidir sobre la
GPL.

### 5.3 Visión por computador — `PixVision` (5,5 MB)

OpenCV, Eigen y dos modelos Caffe: un detector y un aumentador de
resolución.

**Nuestra opción: no traer OpenCV.** Es una dependencia enorme para lo poco
que necesitaríamos. De lo que hace, lo único con valor claro para nosotros es
**aumentar la resolución** de una captura pequeña, y eso es un capricho, no
una necesidad. Lo demás, detectar la región bajo el cursor, ya lo hacemos
mejor con UI Automation, porque en una interfaz de Windows los límites
reales los da el árbol de accesibilidad, no un modelo mirando píxeles.

### 5.4 Fórmulas — `PixFormulaRec` y `PixLatex2MathML` (4,2 MB)

Reconocen una fórmula de una imagen y la pasan a LaTeX, y de ahí a MathML.

**Nuestra opción: dejarlo para más adelante.** Si se quiere, la vía libre es
un modelo tipo LaTeX-OCR (MIT) sobre ONNX Runtime. Es la función más cara y
la de menos uso diario de todas las que tienen.

### 5.5 El envoltorio de inferencia — `PixModelRunner` (0,09 MB)

Noventa kilobytes envolviendo trece megas de ONNX Runtime.

**La lección, que sí adoptamos:** el motor de inferencia detrás de una
interfaz mínima, para poder cambiarlo o cargarlo solo cuando hace falta. Si
algún día metemos reconocimiento, va detrás de un crate propio pequeño y se
carga tarde, no al arrancar.

### 5.6 Atajos globales — `qxtglobalshortcut`

Usan una biblioteca de terceros para algo que en Win32 es una llamada,
`RegisterHotKey`, que es lo que ya hacemos. Aquí estamos mejor que ellos: una
dependencia menos.

### 5.7 Cuenta, red, actualizaciones y telemetría — `PixAuth`, `PixNetwork`, `PixDownload`, `PixStat` (3,8 MB)

**No los queremos.** Son casi cuatro megas y toda la superficie de OpenSSL
para cosas que van en contra de lo que este proyecto es: local, sin cuenta y
sin que nada salga del equipo.

### 5.8 Gestor de fallos — Crashpad

**No lo queremos.** Ya escribimos el pánico en el registro local con un
gancho, y Crashpad manda informes fuera.

## 6. Dos diferencias de arquitectura que nos favorecen

**Cómo pintan.** Su interfaz es Qt 5, y distribuyen `d3dcompiler_47.dll`, que
es la señal de que Qt va por ANGLE: OpenGL ES traducido a Direct3D 11. Son
dos capas de traducción entre lo que quieren dibujar y la tarjeta gráfica.
Nosotros hablamos directamente con Direct2D y DirectComposition. Para
ventanas flotantes que se mueven y se amplían, nuestro camino es más corto y
tiene menos latencia. En su ejecutable no aparece rastro de
DirectComposition.

**Cuántos procesos.** Ellos necesitan el principal, uno auxiliar y el gestor
de fallos. Nosotros somos uno solo, con instancia única.

## 7. Qué hacemos con todo esto

**Adoptamos de su estructura**

1. **El registro de comandos** (§4): cada acción con nombre estable, título
   traducido, atajo configurable y marca de aparecer en la bandeja. Es lo
   que da la parte «versátil», y convierte cada función nueva en una entrada
   de una tabla en vez de código repartido por el bucle principal.
2. **La separación por dominio** con nombres que declaran responsabilidad.
   Ya la tenemos; sirve de confirmación.
3. **El motor pesado detrás de una interfaz mínima y cargado tarde**, si
   algún día metemos reconocimiento.

**Resolvemos con lo que ya trae Windows, no con dependencias**

| Su pieza | Nuestra respuesta | Lo que ahorramos |
|---|---|---|
| `PixOCR2` + modelos | `Windows.Media.Ocr` | 34 MB |
| `PixAVCodec` | Media Foundation + GIF propio | 7,6 MB y la GPL |
| `PixVision` | UI Automation, que ya usamos | 5,5 MB |
| `qxtglobalshortcut` | `RegisterHotKey` | una dependencia |

**Descartamos**

Cuenta, licencia, red, actualizaciones, telemetría, gestor de fallos remoto y
tutorial incrustado: 12 MB largos de cosas que no queremos que existan en
este programa.

**Comandos que merece la pena añadir**, por orden de cuánto dan a cambio de
lo que cuestan: restaurar el último pin cerrado, cerrar todos los pines,
capturar con retardo, capturar y entrar directo a anotar, silenciar los
atajos un rato, poner encima la ventana bajo el ratón, pinear lo seleccionado
en el Explorador y, más adelante, grabar GIF.
