# Motor de audio: lo que enseñan los que ya funcionan

25-oct-2026. El motor de audio nuevo (`crates/pixpin-motor-audio`, ver
[`../motor-de-audio.md`](../motor-de-audio.md)) se escribió entero sin haber corrido nunca en
Windows. Antes de ponerlo delante de un usuario se leyó el código de los proyectos que llevan años
grabando sonido en cualquier equipo, para ver qué hacen ellos que nosotros no, qué hacemos
nosotros que ellos no, y qué rompe en la vida real. Se leyó el código de verdad (clones parciales,
solo para leer), y aquí se cita fichero y función.

Equipo de referencia: Core i3 de 3.ª generación (sin AVX2 ni SSE4.1 en el binario:
`target-cpu=x86-64`), 4 GB, Windows 10 21H2 / 11. Y Android, al menos para que el núcleo valga allí.

---

## 1. OBS Studio — `plugins/win-wasapi` y `libobs`

**Qué es.** La captura WASAPI de OBS: micrófono (`SourceType::Input`), salida del equipo
(`DeviceOutput`) y, desde OBS 28, «captura de audio de aplicación» (`ProcessOutput`). Probablemente
el código WASAPI más usado del mundo en máquinas de todo tipo.

**Qué hace bien (y cómo).**

- **Reconexión** (`win-wasapi.cpp`: `WASAPISource::CaptureThread`, `ReconnectThread`,
  `TryInitialize`). Un hilo de captura con dos juegos de señales (inactivo: salir/parar/iniciar;
  activo: salir/parar/recibir/reiniciar) y un hilo aparte que reintenta. Si el aparato se invalida
  (`ProcessCaptureData` devuelve `false` ante cualquier fallo de `GetNextPacketSize`/`GetBuffer`),
  espera `RECONNECT_INTERVAL` = **3 000 ms** y vuelve a `Start()`. Si lo que pasó es que cambió el
  aparato por defecto (`restartSignal`), `reconnectDuration = 0`: **en el acto**. Solo avisa en el
  registro del primer fallo seguido (`previouslyFailed`).
- **El aviso de aparato por defecto, sin repeticiones** (`WASAPISource::SetDefaultDevice`): filtra
  por flujo y papel y **compara el id** nuevo con el último (`default_id`): Windows repite el aviso.
- **El silencio del loopback** (`WASAPISource::ClearBuffer`): abre un cliente de reproducción en
  el mismo aparato y suelta un buffer de ceros, «Silent loopback fix. Prevents audio stream from
  stopping and messing up timestamps». (Ojo: no lo arranca; lo que mantiene de verdad la mesa en
  marcha es un flujo **arrancado**.) Y espera el evento con **10 ms** de tope en el loopback
  («Windows 7 does not seem to wake up for LOOPBACK»).
- **Fechas** (`ProcessCaptureData`): la QPC de `GetBuffer` (`ts * 100` a ns) para el proceso
  siempre y para los aparatos si `useDeviceTiming`; si no, `os_gettime_ns()` menos lo que dura el
  paquete. Avisa una vez de `AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR`. `AUDCLNT_BUFFERFLAGS_SILENT` →
  un buffer de ceros propio.
- **Captura por proceso** (`InitClient`): formato `WAVE_FORMAT_EXTENSIBLE` flotante con los canales
  y la tasa de OBS (no hay `GetMixFormat`), `INCLUDE_TARGET_PROCESS_TREE`, y el aviso de la
  activación es un `RuntimeClass<ClassicCom, FtmBase, …>` (ágil). Comprueba cada vuelta que la
  ventana sigue viva (`IsWindow(hwnd)`) y si no, reconecta (la busca otra vez por título/clase/exe).
- **Anillo de la tarjeta enorme**: `BUFFER_TIME_100NS` = **5 s**. En modo por evento no añade
  retraso: es holgura ante tirones.
- **El micrófono por defecto es el de comunicaciones** (`InitDevice`: `eCommunications` para la
  entrada, `eConsole` para la salida).
- **Colas de trabajo de tiempo real** (RTWQ, `RtwqPutWaitingWorkItem` en la cola «Capture») en vez
  de un hilo propio, desde Windows 10 1703; con un comentario: «it silently fails to capture
  Desktop Audio for some reason».

**`libobs`.** `media-io/audio-resampler-ffmpeg.c` (`audio_resampler_resample`) es libswresample
de FFmpeg (`swr_convert`, con `swr_get_delay` para corregir la fecha) y **reserva** con
`av_samples_alloc` cuando un bloque viene más grande. La sincronización entre fuentes
(`obs-source.c`: `source_output_audio_data`) **no corrige la deriva de los cristales**: confía en
las fechas; una diferencia menor que `TS_SMOOTHING_THRESHOLD` (**70 ms**) se ignora y se pega a lo
anterior; mayor, se coloca por su fecha (salto); mayor que `MAX_TS_VAR` (**2 s**) se considera un
salto del reloj y se reinicia la línea de la fuente (`handle_ts_jump`). `obs-audio.c`
(`add_audio_buffering`) sube el retraso global si una fuente llega tarde, hasta un máximo.

**Lecciones.** Reintentar en el acto ante un cambio de aparato y espaciado ante un fallo; no
reabrir por un aviso repetido; un loopback necesita a alguien reproduciendo; aceptar temblor de
fechas en el micrófono; distinguir un salto del reloj de un hueco. Y lo que OBS no hace: corregir
la deriva (los usuarios de micrófonos USB ven la desincronización en grabaciones largas: cada
70 ms de deriva acumulada, un salto) y seguir la línea de tiempo mientras no hay aparato.

## 2. wasapi-rs (HEnquist) y cpal (RustAudio)

**wasapi-rs** (`src/api.rs`) es una envoltura fina y muy bien documentada de WASAPI en Rust (la
usa CamillaDSP). Lo que más vale es su documentación de lo que **no funciona** en la captura por
proceso (`AudioClient::new_application_loopback_client`): `GetMixFormat`, `IsFormatSupported`,
`GetDevicePeriod` y `GetCurrentPadding` dan `E_NOTIMPL`, no hay `IAudioClock` ni
`IAudioSessionControl`, y **`GetBufferSize` devuelve «valores enormes como 3131961357» sin error**.
Su `examples/record_application.rs` lo esquiva con un `VecDeque` que crece. Y `examples/aec.rs`
enseña la cancelación de eco del sistema: `IAudioClient2::SetClientProperties` con
`AudioCategory_Communications` antes de `Initialize`, y después
`IAcousticEchoCancellationControl::SetEchoCancellationRenderEndpoint` (Windows 11 22H2+;
`is_aec_supported` mira antes `IAudioEffectsManager`).

**cpal** (`src/host/wasapi/stream.rs`, `device.rs`) es la capa portable de audio de Rust. Lo que
enseña: `boost_current_thread_priority` — MMCSS por `audio_thread_priority`, y **si falla,
`SetThreadPriority(THREAD_PRIORITY_TIME_CRITICAL)`**; los aparatos por defecto se abren por la
ruta `DEVINTERFACE_AUDIO_*` con `ActivateAudioInterfaceAsync` para que Windows mueva el flujo solo
al cambiar el aparato por defecto (enrutado automático); el formato se negocia con
`IsFormatSupported` + el «closest match» y `AUTOCONVERTPCM` solo para la salida; el aviso de
activación es un `#[implement]` de windows-rs.

**Lecciones.** Acotar lo que dice el aparato de sí mismo; una prioridad de respaldo sin MMCSS; la
receta exacta del eco del sistema. Comprobado de paso que el `#[implement]` de windows-rs ya
responde a `IAgileObject` e `IMarshal` (`windows-implement/src/gen.rs`, con `agile` por defecto):
nuestro aviso de activación por proceso ya es ágil, como el `FtmBase` de C++.

## 3. Microsoft — `Windows-classic-samples/Samples/ApplicationLoopback`

`LoopbackCapture.cpp` es la secuencia oficial: `AUDIOCLIENT_ACTIVATION_PARAMS` con
`AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK` y el modo incluir/excluir árbol, en un
`PROPVARIANT` `VT_BLOB` que **apunta a la pila** (vive hasta que vuelve la activación);
`ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, IID_IAudioClient, …, this)` con
el objeto `RuntimeClass<ClassicCom, FtmBase, IActivateAudioInterfaceCompletionHandler>`; espera a
`ActivateCompleted` (que llega por un hilo MTA) con un evento; `Initialize` con
`LOOPBACK | EVENTCALLBACK | AUTOCONVERTPCM` y **un formato dado** (PCM 16 bits 44,1 kHz estéreo en
el ejemplo: «The app can also call GetMixFormat instead» es falso para este cliente, ver
wasapi-rs), `hnsBufferDuration = 0`. El README pide **Windows 10 build 20348** (aunque la API
aparece en la 2004, 19041) y dice que si el proceso no reproduce nada, se recibe silencio.

**Lecciones.** Nuestra activación (`sistema/proceso.rs`) ya seguía la secuencia; faltaba acotar el
tamaño de paquete y recordar que la versión mínima real es más alta que la que dice la API (por eso
la comprobación es la propia activación, con caída a todo el equipo).

## 4. Remuestreo: speexdsp, rubato

**speexdsp** (`libspeexdsp/resample.c`): seno enventanado polifásico con una tabla `quality_map` de
11 calidades (`{48, 8, 0.895, 0.917, KAISER8}` es la 3, «~80 dB», la de VoIP; la 5 es la de
escritorio). Al bajar de tasa **alarga el filtro** por la relación (`update_filter`: `filt_len`
crece con `num_rate/den_rate`) y baja el corte (`downsample_bandwidth`). Con relación arbitraria
interpola la tabla (cúbica, `resampler_basic_interpolate`), y tiene caminos SSE/NEON.

**rubato** (`src/asynchro_sinc.rs`, `sinc_interpolator/`): el `Async` con seno de rubato es lo
mismo con interpolación entre fases y caminos SSE/AVX/NEON elegidos en tiempo de ejecución; y
`src/slip.rs` («a clutch for matching two almost-equal sample rates, slipping a frame when
needed»): para deriva pura, sin filtrar, insertando o quitando un marco con un fundido cruzado de
128.

**Comparación con el nuestro** (`remuestreo.rs`): 48 coeficientes Kaiser β = 7, 128 fases con
interpolación lineal entre fases y posición en coma fija 32.32 → calidad entre la 3 y la 4 de
speexdsp, con mejor interpolación de fase; coste ~48 MAC por marco y canal (speex 3: 48 + la
cúbica). Lo que faltaba: alargar el filtro al bajar mucho (96/192 kHz → 48) y que la suma no fuera
una cadena de dependencias.

## 5. Deriva: zita-ajbridge (`jack-example-tools/tools/zalsa`), `alsa_in`, PipeWire

**zita-ajbridge** (`jackclient.cc`: `Jackclient::jack_process`, `setloop`) es la referencia del
remuestreo adaptativo entre dos relojes. Tres ideas:

1. **El error se mide entre fechas, no con el nivel de la cola**: con las dos últimas fechas de la
   tarjeta (`_t_a0`, `_t_a1`) y los marcos que trajeron (`_k_a0`, `_k_a1`) interpola cuántos
   marcos hay **en este instante** (`err = n + (k_a1 - k_a0) * d1 / d2 + rd - delay`, con `rd` lo
   que retiene el remuestreador): sin el diente de sierra de los paquetes.
2. **La cola se ajusta una vez al empezar** (estado `SYNC2`: `rd_commit(n)` para quitar el error
   inicial) y luego solo se sigue.
3. **Lazo de dos anchos de banda**: 1 Hz los primeros 4 s, 0,05 Hz después; y si la corrección pasa
   del 5 %, «something is really wrong», se reinicia.

**`alsa_in.c`** hace lo mismo más tosco (media de 256 desfases con ventana, `catch_factor`, salto
duro del puntero si se aparta más de `max_diff`). **PipeWire** (`spa/include/spa/utils/dll.h`) es el
mismo lazo de segundo orden (`spa_dll_update`: `z1 += w0*(w1*err - z1); z2 += …; z3 += w2*z2`) con el
ancho de banda bajando de `SPA_DLL_BW_MAX` = 0,128 Hz a `SPA_DLL_BW_MIN` = 0,016 Hz.

**Validación del nuestro** (`deriva.rs`): un PI amortiguado crítico de 30 s (ωn = 1/30 rad/s ≈
0,005 Hz: más lento que el mínimo de PipeWire, aceptable porque nuestra cola es de 50 ms y no de
unos pocos periodos) es el mismo lazo con otra forma, y las pruebas de una hora a ±500 ppm lo
confirman. Lo que faltaba es justo lo de zita: la medida entre fechas y el enganche rápido (con el
lazo lento, una tarjeta a 500 ppm tardaba **90-100 s** en quedar aprendida). Probado al
implementarlo: un lazo rápido con la medida gruesa **oscila** (el diente de sierra de 10 ms por la
ganancia rápida pide miles de ppm, y el límite de cambio por segundo dentro del lazo lo vuelve
inestable): las tres ideas van juntas o no van.

## 6. Android: Oboe (google/oboe), AudioPlaybackCapture, presets

**Oboe** (`docs/FullGuide.md`, `include/oboe/Definitions.h`, `src/aaudio/AudioStreamAAudio.cpp`):

- **Desconexión**: un flujo pasa a `Disconnected` si el aparato se va, hay un error interno o deja
  de ser el principal; **antes de la API 37, todo cambio de ruta desconecta**. Con un
  `AudioStreamErrorCallback`, Oboe cierra el flujo en otro hilo y llama a `onErrorAfterClose`,
  «opening a separate stream is also a valid use of this callback». Hay arreglos de fabricante
  (`QuirksManager.cpp`) y hasta un `ErrorTimeout` que se cambia por `ErrorDisconnected`
  («b/173928197»).
- **`PerformanceMode`**: `LowLatency` (buffers pequeños, camino MMAP si hay) o `PowerSaving`
  (buffers grandes, menos batería). Para grabar, `PowerSaving`.
- **`InputPreset`** (= `MediaRecorder.AudioSource`): `VoiceRecognition` (el de Oboe por defecto,
  «optimized for low latency», sin el procesado de llamadas), `Camcorder`, `VoiceCommunication`
  (AEC/NS del fabricante; privado por defecto), `Unprocessed` («no AGC, the recorded volume may be
  very low»).
- **Conversión de tasa**: `setSampleRateConversionQuality` (por defecto `Medium`).
- **`AllowedCapturePolicy`**: «For privacy, the following Usages can not be recorded:
  VoiceCommunication*, Notification*, Assistance* and Assistant. On Android Q, only Usage::Game and
  Usage::Media may be captured.»

**AudioPlaybackCapture** (API 29+): `AudioPlaybackCaptureConfiguration.Builder(mediaProjection)`
con `addMatchingUsage(USAGE_MEDIA | USAGE_GAME | USAGE_UNKNOWN)`; necesita **MediaProjection**
(diálogo de grabar pantalla; desde Android 14 un consentimiento por sesión) dentro de un servicio
en primer plano de tipo `mediaProjection` (`FOREGROUND_SERVICE_MEDIA_PROJECTION`, API 34+), más
`RECORD_AUDIO`. Una app se excluye con `android:allowAudioPlaybackCapture="false"` o
`setAllowedCapturePolicy`; las de `targetSdk` < 29, por defecto. **Consecuencia para PixPin: en
Android no se puede grabar la otra parte de una llamada de Meet/Zoom/WhatsApp** (van como
`VOICE_COMMUNICATION`); sí el vídeo de una clase (`MEDIA`).

**Lecciones.** El núcleo tiene que aceptar sonido **empujado** desde la llamada de la plataforma
(no se lee como WASAPI), con su fecha de `CLOCK_MONOTONIC`, sin esperar ni reservar; y un aparato
que se desconecta es lo normal y se reabre. El diseño completo está en `motor-de-audio.md` §8.

---

## 7. Lección → cambio

| # | Lección (de dónde) | Qué se hizo | Dónde |
|---|---|---|---|
| 1 | Reintentar en el acto tras un cambio de aparato y espaciado tras un fallo (OBS `CaptureThread`, `RECONNECT_INTERVAL` 3 s) | **Aplicado**: en el acto, 250 ms, 500 ms, 1 s, 2 s, 3 s…; solo el primer fallo al registro. Antes: cada 500 ms para siempre, cada fallo al registro | `plataforma::Reintentos`, `capturar` |
| 2 | No reabrir por un aviso repetido (OBS `SetDefaultDevice`) | **Aplicado**: se compara el id del aparato abierto con el de por defecto | `capturar`, `Aparato::id`, `PlataformaWindows::id_por_defecto` |
| 3 | Mantener la línea de tiempo sin aparato (lo que OBS no hace) | **Ya estaba**; ahora el bucle es puro y se prueba con un aparato simulado que se va y vuelve a otra tasa | `plataforma::pruebas` |
| 4 | El loopback necesita alguien reproduciendo (OBS `ClearBuffer`; doc. «Loopback Recording») | **Aplicado**: `Latido`, un flujo de silencio arrancado en los mismos altavoces, rellenado en cada vuelta; el relleno por el reloj queda de respaldo | `sistema/captura.rs` |
| 5 | Temblor de fechas en el micrófono: 70 ms (OBS `TS_SMOOTHING_THRESHOLD`) | **Aplicado** al micrófono (el equipo sigue en 20 ms: sus huecos son de verdad); 1 ms tras `DATA_DISCONTINUITY` | `reloj::UMBRAL_HUECO_MICROFONO` |
| 6 | Un salto del reloj no es un hueco (OBS `MAX_TS_VAR` 2 s, `handle_ts_jump`) | **Aplicado**: ni la toma rellena ni la mezcla saca lo que estuvo dormido; cuenta `saltos`. Antes, abrir el portátil tras una hora metía una hora de silencio (y la mezcla la sacaba de golpe) | `reloj::SALTO_MAXIMO`, `avance_de_mezcla`, `motor::mezclar` |
| 7 | Fechas QPC de `GetBuffer`, y la hora si `TIMESTAMP_ERROR` (OBS `ProcessCaptureData`) | **Ya estaba**; ahora también una fecha del futuro, y la lógica es pura (`toma`) | `toma::Toma::paquete` |
| 8 | Anillo de la tarjeta grande: es holgura (OBS 5 s) | **Aplicado**: 1 s (antes 200 ms) | `ANILLO_TARJETA_100NS` |
| 9 | `GetBufferSize` miente en la captura por proceso (wasapi-rs) | **Aplicado**: el paquete reservado se acota a 1 s de su tasa (antes se reservaban **miles de millones** de f32: la captura por proceso habría muerto al abrir) | `toma::marcos_de_paquete` |
| 10 | Prioridad de respaldo sin MMCSS (cpal `boost_current_thread_priority`) | **Aplicado**: `TIME_CRITICAL` / `HIGHEST` y de vuelta a `NORMAL` | `sistema/com.rs`, `Mmcss` |
| 11 | El aviso de activación tiene que ser ágil (ejemplo de Microsoft, `FtmBase`) | **Comprobado**: el `#[implement]` de windows-rs ya lo es; el hilo es MTA | `sistema/proceso.rs` |
| 12 | Cliente por proceso: formato dado, `AUTOCONVERTPCM`, sin `GetMixFormat` (Microsoft, wasapi-rs, OBS) | **Ya estaba** | `sistema/proceso.rs` |
| 13 | Un aparato en exclusiva (`AUDCLNT_E_DEVICE_IN_USE`) o el servicio parado | **Aplicado**: el error lo dice con su paso; a mitad, se reintenta | `captura::de_initialize` |
| 14 | El eco del sistema: categoría de comunicaciones + referencia (wasapi-rs `examples/aec.rs`) | **Aplicado como opción** (`Opciones::cancelar_eco`, apagada), más `SetDuckingPreference` para que Windows no baje la reunión; `EstadisticasFuente::eco` dice qué se consiguió. `Grabadora` no lo pide aún: hay que oírlo en un equipo de verdad | `captura::pedir_comunicaciones`, `ajustar_eco` |
| 15 | Cancelador propio (speexdsp `mdf.c`) | **Descartado por ahora**: ~1 300 líneas con FFT, 3-5 % de un i3, y el del sistema existe. Anotado: nuestra mezcla ya tiene la referencia alineada por el reloj, que es lo difícil | — |
| 16 | Medir la cola entre fechas, no al mirar (zita `jack_process`) | **Aplicado**: el anillo se sella con hasta cuándo llega; medida = lo que hay + lo capturado desde el sello − su media de 5 s | `anillo::Productor::sellar`, `nucleo::Entrada::medida_fina` |
| 17 | Ajustar la cola una vez al empezar (zita `SYNC2`) | **Aplicado**: al cebar se tira lo que trajo de más el último paquete | `nucleo::Entrada::producir` |
| 18 | Lazo de dos anchos de banda (zita `setloop`, PipeWire `spa_dll`) | **Aplicado** solo con la medida fina: 5 s los primeros 30 s, luego 30 s con traspaso sin tirón. ±500 ppm aprendido en 14 s (antes 90-100) | `deriva::Deriva::observar_fina` |
| 19 | Alargar el filtro al bajar de tasa (speexdsp `update_filter`) | **Aplicado**: ×2 desde 96 kHz, ×4 desde 192; 30 kHz a 192 kHz pasa de -52 dB a < -60 dB | `remuestreo::coeficientes_para` |
| 20 | Sumas en varios carriles / SIMD (speexdsp SSE/NEON, rubato SSE/AVX/NEON) | **Aplicado** sin `unsafe` ni intrínsecos: ocho acumuladores que el compilador vectoriza con SSE (el AVX en tiempo de ejecución no compensa a este tamaño) | `remuestreo::producto` |
| 21 | No reservar en el camino del sonido (OBS sí lo hace en `audio_resampler_resample`) | **Ya estaba**; ahora también al convertir lo empujado (`empuje`) | — |
| 22 | Sin `match` por muestra; `round` sin SSE4.1 es una llamada | **Aplicado**: un bucle por tipo de muestra en `formato`; `a_i16` con suma de medio y truncado; el limitador sin división por marco | `formato::Conversor::convertir`, `dsp::a_i16` |
| 23 | El micrófono por defecto de comunicaciones (OBS `eCommunications`) | **Descartado**: el de PixPin es el de consola, el que el usuario ve como «predeterminado» y el que usaba `pixpin-audio`; cambiarlo mudaría de micrófono a quien tenga unos cascos como comunicaciones | — |
| 24 | Colas de trabajo de tiempo real (OBS RTWQ, Microsoft `MFPutWaitingWorkItem`) | **Descartado**: OBS mismo las apaga para el sonido del equipo por fallos; un hilo con MMCSS es igual de bueno y más sencillo | — |
| 25 | Enrutado automático al aparato por defecto (cpal: `ActivateAudioInterfaceAsync` con `DEVINTERFACE_AUDIO_*`) | **Descartado**: no vale para el loopback y nuestro bucle ya reabre sin perder la línea de tiempo | — |
| 26 | Deslizar marcos en vez de remuestrear (rubato `Slip`) | **Descartado**: el remuestreador ya hace falta para 44,1/48/96 y no cuesta; deslizar mete un pequeño fundido cada pocos segundos | — |
| 27 | Sonido empujado desde la plataforma, con `CLOCK_MONOTONIC` (Oboe/AAudio, AudioRecord) | **Aplicado**: `empuje::Empujador`/`Empujado`; el motor entero probado «como Android» | `empuje.rs`, `motor::pruebas` |
| 28 | Separar lo que es del sistema (todas) | **Aplicado**: `plataforma::Sistema` (captura + AAC + prioridad + reloj); el motor es genérico y se prueba entero en Linux; compila para `aarch64-linux-android` | `plataforma.rs`, `motor.rs` |
| 29 | Un fallo al escribir no puede dejar el fichero sin cerrar | **Aplicado**: se deja de escribir, se sigue vaciando la cola y se cierra siempre (`Finalize` una vez); probado | `plataforma::escribir_aac` |
| 30 | La ventana o el proceso se va (OBS `IsWindow`) | **Descartado**: Windows entrega silencio y la línea sigue; volver a buscar el proceso por su ventana es de quien elige el proceso (la grabadora de reuniones), no del motor | — |
| 31 | En Android no se puede capturar `VOICE_COMMUNICATION` | **Documentado**: la otra parte de una llamada no se graba en Android; sí una clase en vídeo | `motor-de-audio.md` §8 |

## 8. Lo que queda

- **Correrlo en Windows de verdad** (las pruebas `--ignored` de `motor.rs` y `pixpin-audio`, y la
  lista de pruebas a mano de `motor-de-audio.md` §9): el latido, la captura por proceso y el eco
  solo están compilados y razonados contra el código de referencia.
- **Decidir `cancelar_eco` en `Grabadora`** para las reuniones (`Fuentes::LAS_DOS`) cuando se haya
  oído en dos o tres equipos: probablemente sí con altavoces y no con cascos (Windows no dice cuál
  es cuál de forma fiable; se puede mirar `PKEY_AudioEndpoint_FormFactor` del aparato de salida:
  `Headphones`/`Headset` → no hace falta).
- **El sistema de Android** (`impl Sistema`): JNI o `ndk::audio`, y la `SalidaAac` con `MediaCodec`
  + `MediaMuxer`. El diseño y el pseudocódigo están en `motor-de-audio.md` §8.
- Un cancelador de eco propio (MDF) si el del sistema no basta en los equipos sin procesado de
  llamadas del fabricante.
