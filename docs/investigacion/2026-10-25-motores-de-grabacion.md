# Cómo graban los buenos: OBS, windows-capture, Cap, Microsoft y Android

*25-oct-2026 · sigue a [../motor-de-video.md](../motor-de-video.md)*

## El pedido

> «Busca proyectos en GitHub que hayan hecho motores de video y audio optimizados y que funcionen
> muy bien. Aprende de ellos, lee su código, entiéndelo, y mejora el motor que acabas de
> desarrollar: más estable, confiable en cualquier entorno, y portable a Android o que por lo menos
> funcione muy bien también en Android.»

El motor (`crates/pixpin-motor-video`) **nunca se ha ejecutado en Windows**: compila y sus cuentas
tienen pruebas, pero la tubería de la GPU solo se había razonado. El equipo de referencia es un
Core i3 de tercera generación (Ivy Bridge, Quick Sync de séptima generación), 4 GB, Windows 10
21H2 u 11. Lo que buscaba en otros proyectos era, sobre todo, **lo que pasa cuando algo sale mal**:
la gráfica que se pierde, el portátil con dos gráficas, el monitor que se desconecta, el
codificador que no arranca o se atasca, la pausa, las marcas de tiempo.

## Qué se leyó

Clones superficiales (`--depth 1 --filter=blob:none --sparse`), solo lectura; no se compiló ni se
ejecutó nada de ellos.

| Proyecto | Qué se leyó |
|---|---|
| [obsproject/obs-studio](https://github.com/obsproject/obs-studio) | `libobs-winrt/winrt-capture.cpp`, `libobs-d3d11/d3d11-subsystem.cpp`, `libobs-d3d11/d3d11-rebuild.cpp`, `plugins/win-capture/{duplicator-monitor-capture.c,window-capture.c}`, `plugins/obs-qsv11/{obs-qsv11.c,QSV_Encoder.cpp}`, `plugins/obs-nvenc/{nvenc.c,nvenc-helpers.c,nvenc-d3d11.c}`, `libobs/{obs-video-gpu-encode.c,obs-encoder.c}` |
| [NiiightmareXD/windows-capture](https://github.com/NiiightmareXD/windows-capture) (Rust) | `src/encoder.rs`, `src/graphics_capture_api.rs`, `src/frame.rs`, `src/d3d11.rs` |
| [CapSoftware/Cap](https://github.com/CapSoftware/Cap) (Rust) | `crates/enc-mediafoundation/src/{mft.rs,video/h264.rs}`, `crates/d3d-adapter/src/lib.rs`, `crates/scap-direct3d/src/lib.rs`, `crates/recording/src/{output_pipeline/frame_pacing.rs,resolution_limits.rs}` |
| [CapSoftware/scap](https://github.com/CapSoftware/scap) (Rust) | `src/capturer/engine/win/mod.rs` |
| [robmikh/Win32CaptureSample](https://github.com/robmikh/Win32CaptureSample) | `Win32CaptureSample/{App.cpp,SimpleCapture.cpp}` |
| [robmikh/SimpleRecorder](https://github.com/robmikh/SimpleRecorder) (C#) | `CaptureEncoder/{Encoder.cs,CaptureFrameWait.cs}` |
| AOSP `frameworks/av/cmds/screenrecord` | `screenrecord.cpp` (`prepareEncoder`, `setDisplayProjection`, `updateDisplayProjection`, `runEncoder`) |
| [yrom/ScreenRecorder](https://github.com/yrom/ScreenRecorder) | `ScreenRecorder.java`, `VideoEncodeConfig.java`, `MicRecorder.java` |
| [HBiSoft/HBRecorder](https://github.com/HBiSoft/HBRecorder) | `ScreenRecordService.java`, `HBRecorderCodecInfo.java` |

## Lo que hace bien cada uno

### OBS Studio

- **La gráfica que se pierde se reconstruye entera.** `device_present` mira el `HRESULT` de
  `Present`; con `DXGI_ERROR_DEVICE_REMOVED` o `_RESET` llama a `gs_device::RebuildDevice`
  (`d3d11-rebuild.cpp`): avisa a cada fuente (`device_loss_release`), suelta todos los objetos de
  la GPU, crea un dispositivo nuevo **en el mismo adaptador**, los vuelve a crear y avisa otra vez
  (`device_loss_rebuild`). La captura de Windows.Graphics.Capture se registra para eso
  (`gs_register_loss_callbacks` en `winrt_capture_init_internal`) y en `_rebuild` crea **otra vez
  el `GraphicsCaptureItem`, el pool y la sesión**. Antes, `LogD3D11ErrorDetails` apunta
  `GetDeviceRemovedReason`.
- **Opciones de WGC por capacidad.** `winrt_capture_cursor_toggle_supported` y
  `winrt_capture_border_toggle_supported` preguntan a `ApiInformation::IsPropertyPresent`; para
  quitar el borde pide antes `GraphicsCaptureAccess::RequestAccessAsync(Borderless)`.
- **No se fía de `ContentSize`.** `on_frame_arrived`: «need GetDesc because ContentSize is not
  reliable»; recrea el pool cuando cambia el tamaño, y si el formato de la superficie ya no es el
  esperado (se encendió el HDR) marca la captura como inactiva para rehacerla.
- **El monitor que se va.** `duplicator-monitor-capture.c`: si la captura no está, la reintenta
  cada `RESET_INTERVAL_SEC` (3 s) y antes vuelve a buscar el monitor **por su identificador de
  dispositivo** (`update_monitor_handle`), porque el `HMONITOR` cambia. `choose_method` usa WGC (y
  no la duplicación de escritorio) cuando el monitor no está en la gráfica de OBS o en un portátil
  con dos gráficas.
- **La ventana minimizada.** `window-capture.c` no captura con `IsIconic` o ventana invisible, y
  busca la ventana otra vez cada cierto tiempo.
- **Peculiaridades por fabricante.** `obs-qsv11.c`: en Sandy Bridge e Ivy Bridge, **cero fotogramas
  B** (`bFrames = 0`) y **tope de 1920×1200** (`cap_resolution`); la plataforma sale del modelo de
  CPU (`qsv_get_cpu_platform`, modelos `0x3a`/`0x3e` = Ivy Bridge). `obs_qsv_create_tex` y
  `nvenc_create_base` caen al codificador **«sin texturas»** (el «Fallback», con los fotogramas en
  memoria) cuando la gráfica del codificador no es la de OBS o no se pueden compartir texturas.
  `nvenc-helpers.c` traduce `NV_ENC_ERR_INCOMPATIBLE_CLIENT_KEY` a «demasiadas sesiones».
- **NV12 con desconfianza.** `HasBadNV12Output` pinta una textura NV12 de prueba y la lee; si sale
  mal desactiva el NV12 por texturas. WARP antiguo, igual (`VendorId 0x1414`).
- **Pausa exacta.** `obs-encoder.c`: `video_pause_check` corta el vídeo en un fotograma,
  `pause_audio`/`unpause_audio` cortan el sonido en la muestra exacta, y `ts_offset` se resta de
  todas las marcas después.
- **Codificación por texturas con cola propia.** `obs-video-gpu-encode.c`: una cola de texturas
  con mutex por clave (`lock_key`/`next_key`) y, si el codificador se atrasa, el mismo fotograma se
  cuenta varias veces (`--tf.count`) y se apunta como saltado.

### windows-capture (Rust)

- `GraphicsCaptureApi::new` pone cada opción de la sesión (cursor, borde, ventanas secundarias,
  `MinUpdateInterval`, `DirtyRegionMode`) **solo si `ApiInformation` dice que está**.
  `Frame::dirty_regions` expone las regiones cambiadas.
- `VideoEncoder` usa `MediaTranscoder` + `MediaStreamSource` con aceleración por hardware: es lo
  más corto, pero **las colas de fotogramas y de sonido son `mpsc::channel()` sin tope** (si el
  codificador se atrasa, la memoria crece) y `build_padded_surface` copia cada fotograma a **una
  sola** superficie reutilizada que el transcodificador puede estar leyendo todavía.
- `send_audio_buffer` **ignora la hora que le dan** y fecha el sonido con su propia cuenta de
  muestras: monotónico pase lo que pase.

### Cap (Rust)

- `d3d-adapter::select_capture_adapter`: nunca una gráfica de software ni **virtual** (Parsec,
  DisplayLink, Splashtop…): vieron cierres sin aviso dentro de Media Foundation con esos
  controladores.
- `enc-mediafoundation`: enumera los MFT con `MFTEnumEx` (hardware primero, software después) y
  apunta su nombre (`MFT_FRIENDLY_NAME_Attribute`). Como **usan el MFT sin escritor** tienen que
  desbloquear los asíncronos (`MF_TRANSFORM_ASYNC_UNLOCK`) y llevar el bucle de eventos
  (`METransformNeedInput`/`HaveOutput`). Su `EncoderHealthMonitor` declara el codificador enfermo
  (30 entradas sin salida, 5 fallos seguidos, 5 s sin salida) y `should_fallback` cambia de
  codificador. **`notify_input_stall_recovered`** existe porque una pausa larga se tomaba por un
  atasco y mataba la grabación al seguir.
- `scap-direct3d`: al parar, `CallbackActivity::wait_idle` espera a que acabe el manejador de
  `FrameArrived` que estuviera corriendo **antes** de cerrar el pool.
- `frame_pacing.rs`: midieron fotogramas de 11 µs (16 de 262) porque el repetido se fechaba en el
  hueco que luego ocupaba un fotograma de verdad que llegaba tarde.

### scap

- Envuelve windows-capture y baja cada fotograma a la CPU. El recorte hace `expect` dentro del
  manejador: un fallo **tumba el proceso**. Es lo que no hay que hacer.

### Microsoft (robmikh)

- `Win32CaptureSample/App.cpp`: «Even if the user or system policy denies access, it's still safe
  to set the IsBorderRequired property»: se pide `RequestAccessAsync(Borderless)` y se pone la
  propiedad igual.
- `SimpleCapture::OnFrameArrived`: copia **solo las regiones cambiadas**, recortándolas a la
  superficie (durante un cambio de tamaño pueden salirse) y recrea el pool al cambiar tamaño o
  formato.
- `SimpleRecorder`: `MediaTranscoder` con hardware, `BufferTime = 0`, y la copia de cada fotograma
  se hace dentro de `MultithreadLock` (el `ID3D10Multithread` del dispositivo), porque el
  codificador usa el mismo dispositivo desde otros hilos.

### Android

- **AOSP `screenrecord`.** `prepareEncoder` configura un `MediaCodec` con entrada por superficie
  (`KEY_COLOR_FORMAT = OMX_COLOR_FormatAndroidOpaque`), `KEY_I_FRAME_INTERVAL`, `KEY_MAX_B_FRAMES`
  y el ritmo de la pantalla; **si no arranca al tamaño nativo reintenta a 720p**
  (`kFallbackWidth`); los lados siempre pares (`floorToEven`). `setDisplayProjection` encaja la
  pantalla **manteniendo su forma** dentro de un vídeo de tamaño fijo y `updateDisplayProjection`
  lo rehace cuando gira el móvil. `runEncoder` añade la pista al `MediaMuxer` con
  `INFO_FORMAT_CHANGED`, no escribe los `BUFFER_FLAG_CODECCONFIG` y, si una marca llega a cero, usa
  el reloj monotónico.
- **yrom/ScreenRecorder.** `MediaProjection` → `VirtualDisplay` → superficie de `MediaCodec` →
  `MediaMuxer`, con AAC del micrófono. Guarda las salidas hasta que las dos pistas tienen formato
  (en listas sin tope) y **resta a cada pista su primera marca por separado**
  (`resetVideoPts`/`resetAudioPts`): si una empieza más tarde, las pistas se separan.
  `MicRecorder.calculateFrameTimestamp` fecha el sonido por muestras contadas.
  `KEY_REPEAT_PREVIOUS_FRAME_AFTER` está comentado («maybe useful»).
- **HBRecorder.** `MediaRecorder` con `VideoSource.SURFACE` y la `VirtualDisplay` encima;
  `MediaProjection.registerCallback(onStop)` para parar si el sistema retira el permiso (desde
  Android 14 es obligatorio registrarlo antes de `createVirtualDisplay`), `setMaxFileSize` y
  `pause()`/`resume()` del propio `MediaRecorder`. `HBRecorderCodecInfo` pregunta a
  `MediaCodecInfo` qué tamaños y ritmos admite cada codificador.

## Lecciones → cambios

| Lección (de dónde) | Cambio | Dónde |
|---|---|---|
| Gráfica perdida: rehacer dispositivo y todo lo de la GPU, recrear la captura (OBS `RebuildDevice`, `winrt_capture_device_loss_rebuild`) | **Aplicado.** Se reconoce por el `HRESULT` (`DXGI_ERROR_DEVICE_REMOVED/HUNG/RESET/DRIVER_INTERNAL_ERROR`, `MF_E_DXGI_NEW_VIDEO_DEVICE`) o mirando `GetDeviceRemovedReason` cada segundo; se crea otro dispositivo, se rehace el procesador, el repartidor (con `ResetDevice` en el mismo gestor de Media Foundation) y la captura **sin cerrar el MP4**. Tope: 3 vueltas cada 10 min; después se cierra limpio. El fotograma que esperaba en la GPU se cambia por uno negro en su instante. | `recuperacion.rs`, `win/motor.rs` (`Hilo::recuperar`), `win/tubo.rs` (`Tubo::recrear`) |
| Elegir la gráfica del monitor; nunca una virtual o de software (OBS `choose_method`, Cap `select_capture_adapter`) | **Aplicado.** Se enumeran las gráficas, se marca la que tiene el monitor grabado (o el de la ventana) y se elige con una regla pura; `pixpin-capture` gana `Dispositivo::sobre_adaptador`. | `adaptador.rs`, `win/grafica.rs`, `pixpin-capture/src/dispositivo.rs` |
| Codificador en otra gráfica → camino «sin texturas» (OBS `nvenc_create_base`, `obs_qsv_create_tex`) | **Aplicado.** Camino nuevo `GraficaEnMemoria`: el NV12 baja a memoria y el escritor carga el MFT de hardware que haya, comprobando que lo es. Las texturas solo se dan si hay un codificador del mismo fabricante que la gráfica (`MFT_ENUM_HARDWARE_VENDOR_ID_Attribute`). | `cadena.rs` (`Camino`, `intentos`), `win/tubo.rs` (`Software { con_grafica }`), `win/escritor.rs` |
| Enumerar los MFT y apuntar su nombre (Cap `EncoderDevice::enumerate`) | **Aplicado.** `codificadores_de_hardware` devuelve nombre y fabricante; se registran al empezar. | `win/grafica.rs` |
| Llevar a mano el MFT asíncrono (Cap `h264.rs`) | **Descartado.** El `IMFSinkWriter` ya desbloquea los MFT asíncronos y lleva su bucle; escribir el nuestro es mucho código que no se puede probar aquí. Lo que sí: el camino `GraficaEnMemoria` cubre los MFT que no aceptan nuestras texturas. | — |
| Quick Sync de Sandy/Ivy Bridge: sin B y tope 1920×1200 (OBS `cap_resolution`) | **Aplicado.** `peculiaridades(fabricante, dispositivo)` por identificador PCI (HD 2000/2500/3000/4000); los caminos de la gráfica nunca le piden más. B ya iban a cero. | `cadena.rs` |
| Reintentar más pequeño si el codificador no arranca (AOSP `kFallbackWidth`) | **Aplicado.** Antes de bajar de camino, la gráfica se reintenta a 1080p y luego a 720p con la misma forma (no si el fallo es que no hay codificador de hardware). | `cadena.rs` (`reintento_mas_pequeno`), `win/motor.rs` (`elegir`) |
| Pedir el permiso antes de quitar el borde (OBS, Win32CaptureSample) | **Aplicado.** `RequestAccessAsync(Borderless)` y luego `IsBorderRequired = false`. | `win/captura.rs` (`Captura::opciones`) |
| Opciones de WGC por capacidad (todos) | **Aplicado.** Cursor, borde, `MinUpdateInterval`, `IncludeSecondaryWindows` (menús de la ventana grabada) y `DirtyRegionMode`, cada uno si está. | `win/captura.rs` |
| Regiones cambiadas (windows-capture, Win32CaptureSample) | **Aplicado.** Con `DirtyRegionMode::ReportOnly`, un fotograma sin ninguna región dentro de la superficie no despierta al motor (el latido lo repite). El primero y el de tras recrear el pool pasan siempre. | `ritmo.rs` (`trae_cambios`), `win/captura.rs` |
| Ventana minimizada (OBS `window-capture.c`) | **Aplicado.** Con `IsIconic` el fotograma se suelta sin mirarlo y el vídeo sigue con la última imagen. | `win/captura.rs` |
| Monitor que se desconecta: reintento cada 3 s buscando por nombre (OBS `update_monitor_handle`) | **Aplicado.** Por `\\.\DISPLAYn`; si ya no está, el principal. Una ventana cerrada no vuelve. | `reapertura.rs`, `win/motor.rs` (`Hilo::reabrir`) |
| Esperar al manejador en marcha antes de cerrar (Cap `CallbackActivity`) | **Aplicado.** Cuenta de manejadores en marcha y bandera de parado; `Drop` espera hasta 500 ms. | `win/captura.rs` |
| Cerrojo del dispositivo alrededor de cada copia (SimpleRecorder `MultithreadLock`) | **Aplicado.** `EnLaGpu` (`ID3D11Multithread::Enter/Leave`) en la conversión, las copias y la bajada del NV12; nunca mientras se llama al escritor. | `win/com.rs`, `win/conversor.rs`, `win/tubo.rs` |
| Color con los espacios de DXGI (los controladores lo respetan mejor) | **Aplicado.** Con `ID3D11VideoContext1`: entrada `RGB_FULL_G22_NONE_P709`, salida `YCBCR_STUDIO_G22_LEFT_P709`, además de los bits de antes. El tipo de medio ya declaraba BT.709 de estudio. | `win/conversor.rs` |
| La pausa no es un atasco (Cap `notify_input_stall_recovered`) | **Aplicado.** El atasco vuelve a contar de cero al seguir y tras una vuelta de la gráfica. | `cola.rs` (`Atasco::reiniciar`), `nucleo.rs` |
| El repetido no ocupa el sitio de uno de verdad que llega tarde (Cap `frame_pacing.rs`) | **Aplicado**, más fácil que en Cap: el repetido todavía espera (un fotograma de retraso), así que si el de verdad llega pegado se tira el repetido y el de verdad va en su instante. | `ritmo.rs` (`ocupa_el_sitio_del_repetido`), `win/tubo.rs` |
| El sonido monotónico pase lo que pase (windows-capture `send_audio_buffer`) | **Aplicado.** `audio::Pista` recorta lo que pisa lo ya escrito (un bloque repetido o que vuelve atrás) y deja los huecos. | `audio.rs`, `nucleo.rs` |
| Un único reloj, no un desfase por pista (lo contrario de yrom `resetVideoPts`/`resetAudioPts`) | **Ya estaba.** El reloj anclado al sonido (`reloj.rs`); se documenta como contrato también para Android. | `reloj.rs`, `docs/motor-de-video.md` |
| Colas con tope (lo contrario de windows-capture y yrom) | **Ya estaba.** Se mantiene en todo lo nuevo (la recuperación no guarda fotogramas). | `cola.rs` |
| Cierre fallido | **Aplicado.** Si `Finalize` falla, un MP4 fragmentado con algo dentro se conserva con su nombre (se abre igual) y uno normal sin índice se borra. | `archivo.rs` (`tras_fallo_al_cerrar`), `win/tubo.rs` (`Parte::cerrar`) |
| Separar lo que decide de lo que es de cada sistema (para Android) | **Aplicado.** `plataforma.rs` (rasgos `Captura`, `Retenido`, `Tuberia`) y `nucleo.rs` (ritmo, reloj, pausa, atasco, sonido, cierre, contadores) sin nada de Windows, probados con una captura y una tubería de mentira; el hilo de Windows queda como backend. | `plataforma.rs`, `nucleo.rs`, `win/` |
| Probar el NV12 del controlador con una textura de prueba (OBS `HasBadNV12Output`) | **Pendiente.** Hay que verlo en el equipo de referencia; si sale mal, el camino de la CPU ya existe. | — |
| Fallo de formato por HDR (OBS `get_pixel_format`) | **Descartado por ahora.** Se pide BGRA8 y Windows lo pasa a SDR; con HDR encendido los colores pueden salir algo apagados. Se apunta en riesgos. | `docs/motor-de-video.md` |
| `MediaTranscoder` en vez de `IMFSinkWriter` (windows-capture, SimpleRecorder) | **Descartado.** Es menos código, pero no deja poner parámetros del codificador, ni ver qué MFT eligió, ni cambiar el dispositivo del gestor tras una pérdida. | — |
| Prioridad de la GPU y latencia máxima (OBS `set_priority`, `increase_maximum_frame_latency`) | **Descartado.** Son para el compositor de OBS (que pinta a 60 fps); un grabador que solo convierte no gana nada y la prioridad en tiempo real puede robarle la GPU a la reunión. | — |

## Lo que queda por probar en el equipo de referencia

1. Las pruebas con `--ignored` (`tests/grabar_pantalla.rs`).
2. **Perder la gráfica a mitad**: `dxcap -forcetdr` (o desinstalar y reinstalar el controlador de
   Intel desde el Administrador de dispositivos) durante una grabación; el registro tiene que decir
   «se perdió la gráfica; se vuelve a montar» y el MP4 seguir.
3. Desconectar el monitor externo grabado y volverlo a conectar.
4. Una ventana minimizada y vuelta a abrir; una ventana con menús desplegables (Windows 11).
5. Ver en el registro qué camino quedó (`camino = grafica | grafica-en-memoria | software | cpu`)
   y los codificadores enumerados.
