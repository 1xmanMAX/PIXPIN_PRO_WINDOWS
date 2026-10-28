# El motor de video (25-oct-2026)

El usuario: «un motor de video que grabe a buena resolucion y buena calidad
pero que pese menos y gaste menos RAM mientras graba … un motor de video y
uno de audio por separado». Este es el de video: `crates/pixpin-motor-video`.
El de audio es otro crate (`pixpin-motor-audio`); entre los dos solo hay un
contrato ([El sonido](#el-sonido-el-contrato-con-el-motor-de-audio)).

El equipo de referencia es el del usuario: **Core i3 de tercera generacion
(Ivy Bridge, con Quick Sync H.264), 4 GB de RAM, Windows 10 21H2 u 11**. Todo
lo de aqui se decidio pensando en que ese equipo siga yendo bien mientras
graba una reunion de una hora.

## La tuberia

```text
                    hilo del sistema (FrameArrived)          hilo "motor-video"
                    ───────────────────────────────          ──────────────────
Windows.Graphics.Capture                                      recv_timeout(ritmo)
  pool de 2 superficies BGRA ──▶ TryGetNextFrame               │
                                  retiene el ULTIMO  ─────────▶ decidir (ritmo.rs)
                                  (suelta el anterior)          │ Nuevo / Repetir / Esperar
                                  aviso por el canal            ▼
                                                     reloj.fotograma(hora)  ◀── sonido (ancla)
                                                                │
                                         ┌──────────────────────┴───────────────────────┐
                                         ▼                                              │
                      ID3D11VideoProcessor (GPU): BGRA ─▶ NV12 BT.709 16-235,           │
                      encoge al tamano del perfil y pone bandas (geometria::encaje)     │
                      ── se suelta el fotograma de la captura (ya esta en la cola GPU)  │
                                         │                                              │
             ┌───────────────────────────┼───────────────────────────┐                  │
             ▼ Grafica                   ▼ Software                  ▼ Cpu              │
  textura NV12 del repartidor   NV12 propio ─▶ escenificacion   BGRA baja (Bajada),     │
  (MFCreateVideoSampleAllocatorEx,  ─▶ buffer en memoria        encoge en CPU,          │
   MFCreateDXGISurfaceBuffer)       (1,5 B/px, no 4)            pixpin-record           │
             │                           │                           │                  │
             ▼                           ▼                           ▼                  │
  IMFSinkWriter + IMFDXGIDeviceManager   IMFSinkWriter            IMFSinkWriter RGB32   │
  H.264/HEVC de la grafica (MFT HW)      H.264 de Microsoft       (lo de antes)         │
             └───────────────┬───────────┘                                              │
                             ▼                                                          │
                MP4 en <destino>.parte  ◀── pista AAC (poner_audio) ────────────────────┘
                Finalize ─▶ rename (o copia) ─▶ <destino>
```

Paso a paso:

1. **Captura** (`win/captura.rs`). Windows.Graphics.Capture sobre un
   dispositivo D3D11 creado con `VIDEO_SUPPORT` (el de `pixpin-capture`, en
   la grafica que tiene el monitor que se graba: ver
   [Cuando algo sale mal](#cuando-algo-sale-mal)) y protegido para varios hilos
   (`ID3D11Multithread::SetMultithreadProtected`, el equivalente D3D11 de
   `ID3D10Multithread`). Un monitor (`CreateForMonitor`) o una ventana
   (`CreateForWindow`). El manejador de `FrameArrived` **no toca la GPU**:
   saca el fotograma y lo retiene (uno solo, el ultimo), soltando el
   anterior. Asi se arregla lo que tenia `SesionViva` (guardaba la textura
   del pool y soltaba el fotograma: Windows la volvia a llenar mientras se
   leia y salian imagenes rasgadas). No hace falta un anillo de copias: el
   procesador de video lee la superficie **mientras el fotograma sigue
   retenido** y solo despues se suelta, y la cola de la GPU va en orden. Son
   24 MB de memoria de video menos que tres copias a 1080p.
2. **Conversion** (`win/conversor.rs`). `ID3D11VideoProcessor`: BGRA de
   rango completo a NV12 BT.709 de rango de estudio (16-235), encogido al
   tamano del perfil y encajado con bandas negras, en una pasada y sin salir
   de la GPU. El color se dice dos veces: con los bits de siempre y, con
   D3D11.1, con los espacios de DXGI (`RGB_FULL_G22_NONE_P709` ─▶
   `YCBCR_STUDIO_G22_LEFT_P709`), que los controladores respetan mejor. Cada
   tanda de trabajo va con el cerrojo del dispositivo (`com::EnLaGpu`), como
   `SimpleRecorder`. Vistas de entrada y salida guardadas por textura. Si el
   controlador no deja usar la superficie del pool como entrada, se copia
   antes a una textura propia (en la GPU).
3. **Codificacion** (`win/escritor.rs`, `win/tubo.rs`). La textura NV12 del
   repartidor de muestras de Media Foundation va tal cual al MFT de la
   grafica por el `IMFSinkWriter` (`MF_SINK_WRITER_D3D_MANAGER` +
   `MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS = 1`). El codificador:
   - perfil High, sin fotogramas B, un fotograma clave cada dos segundos de
     ritmo maximo (`CODECAPI_AVEncMPVGOPSize`);
   - `PeakConstrainedVBR`: una media que fija lo que pesa una hora y un pico
     para que una diapositiva nueva salga nitida;
   - sin latencia baja (`MF_LOW_LATENCY` no se pone) y sin la regulacion del
     escritor (`MF_SINK_WRITER_DISABLE_THROTTLING`): el tope lo ponen las
     colas del motor;
   - en los tipos de entrada (NV12) y salida: tamano, ritmo, progresivo,
     pixel cuadrado y el color BT.709 de estudio.
   Los parametros van como `pEncodingParameters` de `SetInputMediaType` (el
   escritor se los da al `ICodecAPI` del MFT antes de fijar los tipos). Si el
   codificador no los admite, se prueba con menos y luego sin ninguno. Al
   empezar se comprueba que el MFT elegido es de verdad de hardware
   (`IMFSinkWriterEx::GetTransformForStream` + `MFT_ENUM_HARDWARE_URL_Attribute`).
4. **Un fotograma de retraso**. Con ritmo variable un fotograma dura de 1/30
   s a un segundo. Cada uno se da al codificador cuando llega el siguiente,
   ya con su duracion de verdad; el ultimo se da al terminar hasta donde
   acaba el sonido. Asi el MP4 dura lo mismo que el sonido en cualquier
   reproductor, mire las marcas de tiempo o las duraciones. Y si lo que
   esperaba era un repetido (el latido) y la pantalla cambia justo detras, el
   repetido se tira y el de verdad va en su instante
   (`ritmo::ocupa_el_sitio_del_repetido`): sin eso saldrian fotogramas de un
   milisegundo, lo que Cap midio en el suyo.
5. **El MP4** (`archivo.rs`). Se escribe en `<destino>.parte`; al terminar,
   `Finalize` y se renombra (si no se deja, se copia; si ni eso, el `.parte`
   se queda con su contenido). Soltar el motor sin terminar borra el
   `.parte`. Si la aplicacion se cierra de golpe queda el `.parte`, que ya
   no se borra: lo recupera `grabar_clase::recuperar` (ver
   `docs/grabar-sin-perder-nada.md`).

### Por que no MP4 fragmentado de salida (y por que la clase si lo usa)

**28-oct-2026**: la grabadora de clases y reuniones lo enciende, y al
cerrar (si falta el indice de fragmentos) o al recuperar un corte lo rehace a
MP4 normal sin volver a codificar (`rehacer_mp4`, `archivo::recuperar_video`),
lo que resuelve el punto (2). Lo de abajo sigue valiendo para quien use
`Opciones::de` tal cual.

Un MP4 fragmentado (`MFTranscodeContainerType_FMPEG4`) se puede abrir aunque
la aplicacion se caiga. Esta como opcion (`Opciones::fragmentado`, apagada):
se intenta y, si el escritor no lo acepta, MP4 normal (`Resumen::fragmentado`
dice cual salio). No va de salida porque: (1) en Windows 10 el escritor no lo
garantiza (el sumidero existe desde la 1703, pero no esta documentado para
`MFCreateSinkWriterFromURL`); (2) sin indice de fragmentos (`mfra`/`sidx`) los
reproductores de Android y el propio Media Foundation saltan mal por un video
de una hora; (3) el rescate que ya habia (renombrar, copiar, dejar el
`.parte`) cubre los cierres normales y los fallos del disco. Queda probar en
el equipo de referencia antes de encenderlo.

## Los perfiles

| Perfil | Tamano (sin agrandar) | fps maximo | Media / pico video | Peor caso 1 h (con AAC) |
|---|---|---|---|---|
| «Ligero» | hasta 1280x720 | 10 | 0,30 / 1,5 Mbit/s | ~178 MB |
| «Normal» (de salida) | hasta 1920x1080 | 15 | 0,55 / 3 Mbit/s | ~290 MB |
| «Nitido» | la pantalla tal cual (tope 3840x2160) | 30 | 1,2 / 6 Mbit/s | ~583 MB |

- Los bits escalan con los pixeles respecto a la referencia del perfil, entre
  la mitad y cuatro veces (`perfil::bits_por_segundo`).
- **El peor caso** es la pantalla cambiando sin parar (una reunion con
  camaras). Con una diapositiva quieta salen un fotograma por segundo de unos
  pocos cientos de bytes: una hora de clase con diapositivas queda muy por
  debajo.
- Sonido: AAC a 96 kbps (43 MB por hora), mono o estereo.
- En «Grabar clase», la ficha a la derecha de «Video de esta pantalla» cambia
  el perfil con un clic; con el raton encima, la linea de abajo del titulo
  dice que es cada uno.

## El ritmo variable

`ritmo.rs`: se codifica un fotograma nuevo si la pantalla cambio y paso el
minimo del perfil (1/fps menos un 5 %, para no caer en el fotograma siguiente
del monitor); si no cambio en un segundo, se repite el ultimo (el latido); si
no hay imagen en cinco segundos al empezar, uno negro. El hilo duerme justo lo
que falta para la siguiente decision (`ritmo::espera`). En Windows 11 24H2 se
pide ademas a la captura que no entregue mas deprisa que el tope
(`MinUpdateInterval`).

## El reloj

`reloj.rs`: un solo reloj para las dos pistas, en unidades de 100 ns.

- **Con sonido manda el sonido**: cada bloque deja un ancla (donde acaba en
  el MP4, a que hora llego) y un fotograma capturado a la hora `q` va en
  `ancla + (q - hora del ancla)`, sin pasar un cuarto de segundo por delante
  del sonido. Si el sonido se para (pausa, un tiron) el video se para con el.
  Tras una pausa se espera al primer bloque nuevo.
- **Sin sonido**, el contador del sistema sin las pausas.
- El primer fotograma va siempre en el cero (si no, el video empezaria con un
  hueco). Dos fotogramas nunca en el mismo instante: uno que cae en el
  anterior o antes va un milisegundo detras (es la imagen mas nueva).

## La cadena de caidas

`cadena.rs`, elegida al empezar y dicha en `Estadisticas::codificador` (y en
la linea de abajo de la barra de «Grabar clase»: «Video con la grafica»):

Son cuatro caminos (`cadena::Camino`); los dos primeros se ensenan como
`Codificador::Hardware`:

1. **Grafica** (`Camino::Grafica`): necesita el procesador de video y un MFT
   H.264 de hardware **del fabricante de la grafica del dispositivo**
   (`MFT_ENUM_HARDWARE_VENDOR_ID_Attribute`). HEVC solo si se pide y la
   grafica lo tiene (`hay_hevc_por_hardware`); de salida H.264, que reproduce
   cualquier movil Android. En Sandy Bridge e Ivy Bridge (la del equipo de
   referencia) nunca se le pide mas de 1920x1200 (`cadena::peculiaridades`,
   de OBS); si no arranca a mas de 1080p se reintenta a 1080p y a 720p
   (`cadena::reintento_mas_pequeno`, de `screenrecord` de Android).
2. **La grafica desde memoria** (`Camino::GraficaEnMemoria`): el codificador
   de hardware esta en otra grafica, o no acepta nuestras texturas. El NV12
   baja a memoria como en el de software y el escritor carga el MFT de
   hardware que haya (como el «Fallback» de NVENC en OBS). La CPU sigue sin
   codificar.
3. **Software** (`Codificador::Software`): el H.264 de Microsoft; la
   conversion y el encogido siguen en la GPU y solo baja el NV12 (una textura
   de escenificacion pequena).
4. **CPU** (`Codificador::Cpu`): lo de antes, `pixpin-record::Codificador`
   (BGRA baja, se encoge en la CPU con `lienzo.rs`, RGB32 por software),
   siempre a lo de «Ligero». Para cuando la grafica no tiene procesador de
   video (una maquina virtual, un controlador roto).

Cada intento que falla borra su `.parte` y deja su motivo en el registro; si
fallan todos, `ErrorMotor::SinCodificador` con todos los motivos. El registro
dice el camino que quedo (`camino = grafica | grafica-en-memoria | software |
cpu`) y los codificadores de hardware que hay, con su nombre.

## Cuando algo sale mal

Lo aprendido de OBS, Cap, windows-capture y los ejemplos de Microsoft esta en
[investigacion/2026-10-25-motores-de-grabacion.md](investigacion/2026-10-25-motores-de-grabacion.md).
En corto:

- **En que grafica se graba** (`adaptador.rs`, `win/grafica.rs`): en la que
  tiene conectado el monitor que se graba (el de la ventana, si es una
  ventana), nunca en una virtual (Parsec, DisplayLink...) ni en la de
  software. En un portatil con Intel + NVIDIA la pantalla la pinta la Intel:
  con el dispositivo ahi no hay copias entre graficas y no se pierde cuando
  la NVIDIA se apaga.
- **La grafica que se pierde** (TDR, el controlador que Windows Update
  actualiza a mitad de clase; `recuperacion.rs`, `Hilo::recuperar`): se ve
  por el `HRESULT` de un fallo o mirando `GetDeviceRemovedReason` una vez por
  segundo. Se crea otro dispositivo, se rehace lo que vive en la GPU (el
  procesador de video, el repartidor de texturas con `ResetDevice` en el
  mismo gestor de Media Foundation, la captura) y **el MP4 sigue abierto**:
  el sonido no se corta y el video pone un fotograma negro donde estaba el
  que se perdio. En el camino de la grafica solo vale si la grafica nueva es
  la misma (su codificador es el de antes); si no, se cierra el MP4 con lo
  que haya. Tres vueltas cada diez minutos como mucho; `Estadisticas::vueltas`
  y `Resumen::vueltas` dicen cuantas hubo.
- **El monitor que se desconecta** (`reapertura.rs`): el video sigue con la
  ultima imagen; cada tres segundos se busca el monitor por su nombre
  (`\\.\DISPLAY2`, el `HMONITOR` cambia) y, si ya no esta, el principal.
- **La ventana**: minimizada, sus fotogramas se sueltan sin mirarlos
  (`IsIconic`); cerrada, el video sigue con su ultima imagen hasta que se
  termine. En Windows 11 sus menus y dialogos salen con ella
  (`IncludeSecondaryWindows`).
- **La pantalla quieta**: con Windows 11 24H2 se piden las regiones
  cambiadas y un fotograma sin ninguna no despierta al motor.
- **El borde amarillo** se quita pidiendo antes el permiso
  (`GraphicsCaptureAccess::RequestAccessAsync(Borderless)`).
- **El cierre que falla**: un MP4 fragmentado con algo dentro se conserva;
  uno normal sin indice se borra (`archivo::tras_fallo_al_cerrar`).
- **El sonido que vuelve atras** (un bloque repetido): se recorta lo que pisa
  lo ya escrito (`audio::Pista`).
- **La pausa** no cuenta para el atasco del codificador (el error que Cap
  tuvo que arreglar).

## El nucleo y la plataforma

Lo que decide no sabe de Windows (`nucleo.rs`, `plataforma.rs`): el ritmo
variable, el reloj, la pausa, el sonido, el atasco, el cierre y los
contadores viven en `Nucleo`, que habla con dos rasgos:

- `plataforma::Captura` (+ `Retenido`): retiene el ultimo fotograma sin
  copiarlo, lo da al tomarlo y dice si la fuente se fue;
- `plataforma::Tuberia`: codificador y contenedor juntos (un fotograma
  fechado, repetir, sonido, vaciar hasta el final).

El hilo de Windows (`win/motor.rs`) es un backend: la captura de
Windows.Graphics.Capture y `tubo::EnLaGrafica` (el tubo con su dispositivo)
cumplen los rasgos. Todo lo de fuera de `win/` compila en cualquier sistema y
se prueba con una captura y una tuberia de mentira (`nucleo::pruebas`).

## Colas y memoria

`cola.rs`. Ninguna cola crece sin tope:

| Que | Tope | Memoria a 1080p |
|---|---|---|
| Superficies de la captura | 2 (una retenida) | 16 MB (GPU) |
| Texturas NV12 del repartidor (grafica) | 8 (3 al empezar) | 24 MB (GPU) |
| Fotogramas esperando al software | 6 | 18 MB |
| Mensajes al hilo del motor | 256 | < 1 MB |
| NV12 propio + escenificacion (software) | 1 + 1 | 6 MB |

En la grafica Intel del equipo de referencia la memoria de video es RAM del
sistema: el motor entero se queda en **~40-60 MB** en el camino de la grafica
(el presupuesto era < 80 MB). Si el codificador se atrasa, el fotograma se
salta (`Estadisticas::caidos`); **el sonido nunca**: si su cola esta llena,
`poner_audio` espera. Si el codificador no acepta nada en diez segundos, el
video se corta y se guarda lo que habia (`Resumen::cortado`).

## El sonido: el contrato con el motor de audio

`audio.rs`. Este crate no depende de ningun motor de audio:

- bloques de PCM de 16 bits **intercalados**, a un muestreo fijo (44 100 o
  48 000, lo que admite el AAC de Windows), uno o dos canales
  (`FormatoAudio`);
- cada bloque con su **posicion** en muestras por canal desde el principio;
  en pausa no llegan bloques y la posicion no avanza;
- se empuja con `MotorVideo::poner_audio(&muestras, posicion)`, o se ofrece
  como un `FuenteAudio` (`siguiente(espera) -> Bloque | Nada | Fin`).

Hoy «Grabar clase» usa `CopiaMono`, el adaptador de la copia de la nota de
voz (`pixpin_audio::Grabadora::empezar_con_copia`, bloques mono sin
posicion: la cuenta el adaptador). El motor de audio nuevo solo tiene que dar
un `FuenteAudio` (o empujar con `poner_audio`).

## La forma de usarlo

```rust
let motor = MotorVideo::empezar(
    Opciones {
        perfil: Perfil::Normal,
        audio: Some(FormatoAudio::mono(48_000)),
        ..Opciones::de(monitor_principal()) // o Fuente::Ventana(hwnd)
    },
    destino,
)?;
motor.poner_audio(&bloque, posicion)?; // tantas veces como bloques
motor.pausar(); motor.seguir();
let e = motor.estado(); // fps, fotogramas, caidos, bytes, codificador, vueltas
let r = motor.terminar()?; // Resumen { ruta, duracion_ms, ... }
```

`MotorVideo` se puede mover entre hilos; todo lo que toca Windows vive en su
hilo «motor-video» (COM multihilo, Media Foundation arrancado una vez por
proceso). Soltarlo sin `terminar` tira la grabacion.

## Como probarlo en Windows

Las cuentas y el nucleo (con una captura y una tuberia de mentira) se
prueban en cualquier maquina:

```text
cargo test -p pixpin-motor-video
```

Lo que necesita GPU, escritorio y Media Foundation va en
`crates/pixpin-motor-video/tests/grabar_pantalla.rs`, con `#[ignore]`:

```text
cargo test -p pixpin-motor-video -- --ignored --nocapture
```

- `tres_segundos_de_la_pantalla_con_sonido_dan_un_mp4_con_dos_pistas`: graba
  3 s del monitor principal con un tono de 440 Hz y comprueba que el MP4 tiene
  `moov` y **dos `trak`**, que dura ~3 s, que tiene por lo menos un fotograma
  por segundo y un tamano creible (2 KB - 2 MB). Escribe por la salida si hay
  H.264/HEVC de la grafica, las estadisticas y el resumen (con el codificador
  que quedo).
- `sin_sonido_sale_un_mp4_de_una_pista_con_el_reloj_del_sistema`: mudo, con
  un segundo de pausa que no cuenta.
- `soltar_el_motor_sin_terminar_no_deja_nada`.

En el equipo de referencia conviene mirar ademas, con el Administrador de
tareas, la CPU (con la grafica deberia quedarse en un solo digito) y la
memoria del proceso durante una grabacion de diez minutos con una reunion
abierta.

## Riesgos conocidos

- **No se ha podido ejecutar aqui** (Linux): compila para
  `x86_64-pc-windows-msvc` y pasa clippy, y las cuentas tienen sus pruebas,
  pero la tuberia de la GPU solo se ha razonado. Lo primero es pasar las
  pruebas con `--ignored` en el equipo de referencia.
- Algunos controladores (Intel antiguos) piden `D3D11_BIND_VIDEO_ENCODER` en
  las texturas que leen y otros lo rechazan: se intenta con y sin.
- El codificador de la grafica puede no aceptar `PeakConstrainedVBR`: se cae
  a menos parametros (la media del tipo de salida sigue mandando).
- En Windows 10 la captura lleva el borde amarillo (quitarlo solo se puede
  desde Windows 11).
- La vuelta tras perder la grafica (`Tubo::recrear`) no se ha visto nunca
  funcionar: depende de que el MFT de la grafica acepte el dispositivo nuevo
  por `ResetDevice` (asi lo pide Media Foundation, pero hay controladores que
  no). Si no lo acepta, el siguiente fotograma falla, ya no es una perdida y
  el MP4 se cierra con lo que habia; si ni el cierre va, un MP4 normal se
  pierde. Encender `Opciones::fragmentado` lo evitaria (ver arriba por que no
  va de salida). Probarlo con `dxcap -forcetdr` en el equipo de referencia.
- Con HDR encendido se pide BGRA de 8 bits y Windows lo pasa a SDR: los
  colores pueden salir algo apagados (OBS captura en `R16G16B16A16_FLOAT`).
- No se prueba si el controlador hace bien el NV12 (OBS pinta una textura de
  prueba, `HasBadNV12Output`); si sale mal, el camino de la CPU sigue ahi.

## Android

El motor de Android **no existe todavia**; esto es el diseno, para que el
nucleo que ya hay se reutilice y el comportamiento sea el mismo que en
Windows. Sale de leer AOSP `screenrecord`, yrom/ScreenRecorder y HBRecorder
(ver la investigacion).

### Que se comparte y que no

Todo lo de fuera de `win/` compila para Android sin cambios (no hay nada de
Windows fuera de `cfg(windows)`; en esta maquina solo se pudo comprobar con
`cargo check -p pixpin-motor-video` en Linux, el objetivo
`aarch64-linux-android` no esta instalado). Lo que se comparte, por JNI desde
un crate `cdylib` pequeno que se anadira con la app de Android (no esta en el
espacio de trabajo):

| Pieza | Por JNI (Rust) | En Kotlin |
|---|---|---|
| Perfiles, bits, grupo de imagenes (`perfil.rs`) | si | — |
| Tamano del video y bandas (`geometria::medida_de_salida`, `encaje`) | si | dibujar el rectangulo |
| Ritmo variable (`ritmo::decidir`, `espera`, `ocupa_el_sitio_del_repetido`) | si | el bucle |
| Reloj anclado al sonido (`reloj::Reloj`, un manejador opaco) | si | darle las horas |
| Sonido sin pisarse (`audio::Pista`) | si | — |
| Atasco, vueltas, reintentos (`cola::Atasco`, `recuperacion::Presupuesto`, `cadena::reintento_mas_pequeno`) | si | reaccionar |
| Politica del fichero (`archivo::a_medias`, `tras_fallo_al_cerrar`) | si | renombrar |
| MediaProjection, VirtualDisplay, EGL, MediaCodec, MediaMuxer, AudioRecord, el servicio en primer plano | — | si |

`nucleo::Nucleo` se podria llevar entero con una `Captura` y una `Tuberia`
de JNI, pero en Android el trabajo va por llamadas del sistema
(`onFrameAvailable`, `MediaCodec.Callback`) en sus propios hilos: es mas
natural que Kotlin lleve el bucle y pregunte a Rust lo que decide.

### La tuberia

```text
MediaProjection ─▶ VirtualDisplay (tamano de la pantalla)
                     └▶ SurfaceTexture (GL_TEXTURE_EXTERNAL_OES): retiene el ULTIMO
                          │ onFrameAvailable ─▶ ritmo::decidir (JNI)
                          ▼ Nuevo / Repetir
                   EGL: dibuja el rectangulo de geometria::encaje, con bandas
                        eglPresentationTimeANDROID(reloj.fotograma(hora))
                          ▼
                   superficie de entrada de MediaCodec (H.264, hardware)
                          ▼
AudioRecord (AudioPlaybackCapture + microfono, mezclados por el motor de
audio) ─▶ PCM con su posicion ─▶ MediaCodec AAC ─▶ MediaMuxer (MP4 .parte)
```

- **Por que con EGL en medio** y no la `VirtualDisplay` directa sobre la
  superficie del codificador: asi cada fotograma lleva la marca que dice el
  reloj del sonido (la misma cuenta que en Windows), las bandas al girar el
  movil salen de `geometria::encaje` (AOSP lo hace con
  `setDisplayProjection`, que una app no tiene) y el ritmo variable es el
  nuestro. `SurfaceTexture` se queda solo con el ultimo fotograma
  (`updateTexImage`), igual que la captura de Windows.
- **Ritmo**: `KEY_REPEAT_PREVIOUS_FRAME_AFTER` hace el latido si no se dibuja
  nada en un segundo, y `KEY_MAX_FPS_TO_ENCODER` es el tope (como
  `MinUpdateInterval`). Con el dibujo propio ya los decide `ritmo`, pero se
  ponen igual por si el bucle se para.
- **Reloj**: las horas, todas de `System.nanoTime()` (el reloj monotono, el
  mismo con el que SurfaceFlinger fecha los fotogramas:
  `SurfaceTexture.getTimestamp()`). Para el sonido,
  `AudioRecord.getTimestamp(t, AudioTimestamp.TIMEBASE_MONOTONIC)` da
  exactamente el ancla del reloj (una posicion en muestras y su hora). **Un
  solo reloj para las dos pistas**: yrom resta a cada pista su primera marca
  por separado y se le separan.
- **Pausa**: no se dibuja ni se da sonido; el reloj la descuenta (sin sonido)
  o espera al primer bloque nuevo (con sonido), como en Windows.
- **El sonido de la reunion**: `AudioPlaybackCapture` (Android 10+) solo
  captura los usos `USAGE_MEDIA`, `USAGE_GAME` y `USAGE_UNKNOWN` de apps que
  lo permiten. **Las llamadas (`USAGE_VOICE_COMMUNICATION`, Meet, Zoom) no se
  pueden capturar**: en Android una reunion se graba con el microfono. Hay que
  decirlo en la interfaz.
- **Cuando algo sale mal**: `MediaCodec.CodecException.isTransient` se
  reintenta; `isRecoverable` es el equivalente de la grafica perdida (un
  codificador nuevo con los mismos parametros, `recuperacion::Presupuesto`),
  pero el `MediaMuxer` no admite otra configuracion en la misma pista: si los
  datos de configuracion cambian, se cierra limpio. `MediaProjection.Callback
  .onStop` (el usuario o el sistema quitan el permiso) cierra limpio; desde
  Android 14, `onCapturedContentVisibilityChanged(false)` es la «ventana
  minimizada» (repetir la ultima imagen) y `onCapturedContentResize` el
  cambio de tamano (otro `encaje`).
- **Giro**: `DisplayManager.DisplayListener.onDisplayChanged` ─▶
  `virtualDisplay.resize(...)` al tamano nuevo y otro `encaje`; el video no
  cambia de tamano (igual que en Windows). No se crea otra `VirtualDisplay`:
  desde Android 14 el permiso vale para una sola.
- **Que codificador**: `MediaCodecList(REGULAR_CODECS)`, los de hardware
  primero (`MediaCodecInfo.isHardwareAccelerated`, Android 10+), y
  `VideoCapabilities.areSizeAndRateSupported(w, h, fps)` antes de configurar;
  si `configure` falla, `cadena::reintento_mas_pequeno` (como AOSP, que
  reintenta a 720p).
- **El fichero**: se escribe en `.parte` y se renombra al cerrar bien;
  `MediaMuxer` no hace MP4 fragmentado, asi que si la app muere el `.parte` no
  vale (se barre la vez siguiente, como en Windows).

### Kotlin (esbozo)

```kotlin
// Todo en un servicio en primer plano con foregroundServiceType="mediaProjection".
class MotorVideoAndroid(ctx: Context, datos: Intent, codigo: Int, val perfil: String) {
    val nucleo = Nucleo.nuevo(perfil, muestreo = 48_000, canales = 2) // JNI
    val proyeccion = ctx.getSystemService(MediaProjectionManager::class.java)
        .getMediaProjection(codigo, datos)

    fun empezar(w: Int, h: Int, dpi: Int, destino: File) {
        // Antes de crear la VirtualDisplay (obligatorio desde Android 14).
        proyeccion.registerCallback(object : MediaProjection.Callback() {
            override fun onStop() = terminar()
            override fun onCapturedContentVisibilityChanged(visible: Boolean) {
                nucleo.fuenteVisible(visible)
            }
        }, manejador)

        val (vw, vh) = nucleo.medidas(w, h)           // geometria::medida_de_salida
        val (media, pico) = nucleo.bits(vw, vh)        // perfil::bits_por_segundo
        val fps = nucleo.porSegundo()
        val video = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, vw, vh).apply {
            setInteger(MediaFormat.KEY_COLOR_FORMAT,
                MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface)
            setInteger(MediaFormat.KEY_BIT_RATE, media)
            setInteger(MediaFormat.KEY_BITRATE_MODE,
                MediaCodecInfo.EncoderCapabilities.BITRATE_MODE_VBR)
            setInteger(MediaFormat.KEY_FRAME_RATE, fps)
            setFloat(MediaFormat.KEY_MAX_FPS_TO_ENCODER, fps.toFloat())
            setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 2)                  // segundos
            setLong(MediaFormat.KEY_REPEAT_PREVIOUS_FRAME_AFTER, 1_000_000L) // µs: el latido
            setInteger(MediaFormat.KEY_MAX_B_FRAMES, 0)
            setInteger(MediaFormat.KEY_PROFILE,
                MediaCodecInfo.CodecProfileLevel.AVCProfileHigh)             // si lo admite
            setInteger(MediaFormat.KEY_COLOR_STANDARD, MediaFormat.COLOR_STANDARD_BT709)
            setInteger(MediaFormat.KEY_COLOR_RANGE, MediaFormat.COLOR_RANGE_LIMITED)
            setInteger(MediaFormat.KEY_COLOR_TRANSFER, MediaFormat.COLOR_TRANSFER_SDR_VIDEO)
        }
        val codificador = elegirCodificador(video) // hardware primero; si configure falla,
                                                   // nucleo.reintentoMasPequeno(vw, vh)
        codificador.setCallback(callbackDeVideo, manejador)
        codificador.configure(video, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
        val entrada = codificador.createInputSurface()
        egl = Egl(entrada)                          // EGL14 + EGL_RECORDABLE_ANDROID
        textura = SurfaceTexture(egl.texturaExterna()).apply {
            setDefaultBufferSize(w, h)
            setOnFrameAvailableListener({ hayNuevo = true; despertar() }, manejador)
        }
        pantalla = proyeccion.createVirtualDisplay("pixpin", w, h, dpi,
            DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR, Surface(textura), null, manejador)

        val aac = MediaFormat.createAudioFormat(MediaFormat.MIMETYPE_AUDIO_AAC, 48_000, 2).apply {
            setInteger(MediaFormat.KEY_AAC_PROFILE, MediaCodecInfo.CodecProfileLevel.AACObjectLC)
            setInteger(MediaFormat.KEY_BIT_RATE, 96_000)
            setInteger(MediaFormat.KEY_MAX_INPUT_SIZE, 16_384)
        }
        // El motor de audio da PCM intercalado con su posicion (el mismo contrato que en
        // Windows): AudioPlaybackCaptureConfiguration(USAGE_MEDIA, USAGE_GAME,
        // USAGE_UNKNOWN) + microfono, mezclados.
        muxer = MediaMuxer(nucleo.aMedias(destino.path), MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4)
        codificador.start(); codificadorAac.start()
    }

    // Una vuelta del bucle (en el hilo del manejador, a la hora que diga nucleo.espera).
    fun vuelta() {
        val ahora = System.nanoTime() / 100                      // 100 ns, como el reloj
        when (nucleo.decidir(ahora, hayNuevo)) {                 // ritmo::decidir
            Decision.NUEVO -> { textura.updateTexImage(); hayNuevo = false
                                dibujar(nucleo.fotograma(textura.timestamp / 100)) }
            Decision.REPETIR -> dibujar(nucleo.fotograma(ahora))
            Decision.ESPERAR -> {}
        }
    }

    fun dibujar(t: Long?) {                                      // null: pausa o sin sonido aun
        t ?: return
        egl.dibujar(nucleo.encaje(textura.ancho, textura.alto))  // geometria::encaje
        egl.presentationTime(t * 100)                            // eglPresentationTimeANDROID, ns
        egl.swapBuffers()
    }

    fun sonido(pcm: ShortArray, posicion: Long, ancla: AudioTimestamp) {
        val (salto, desde) = nucleo.admitirSonido(posicion, pcm.size / 2) ?: return // audio::Pista
        nucleo.anclar(desde + pcm.size / 2 - salto, ancla.nanoTime / 100)          // Reloj::sonido
        darAlAac(pcm, salto, ptsUs = nucleo.tiempoDeMuestras(desde) / 10)
    }

    // En las salidas: addTrack con onOutputFormatChanged de las dos pistas, muxer.start(),
    // lo de antes en una cola con tope (dos segundos); sin BUFFER_FLAG_CODEC_CONFIG.
    fun terminar() {
        codificador.signalEndOfInputStream()      // y fin de flujo al AAC; vaciar
        val bien = runCatching { muxer.stop() }.isSuccess
        nucleo.ponerEnSuSitio(bien)               // archivo: renombrar o borrar el .parte
        pantalla.release(); proyeccion.stop()
    }
}
```
