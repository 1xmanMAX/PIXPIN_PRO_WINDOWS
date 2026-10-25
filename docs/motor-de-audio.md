# El motor de audio (`crates/pixpin-motor-audio`)

25-oct-2026. Grabar el micrófono y el sonido del equipo **a la vez, bien y gastando poco**: notas
de voz, el micrófono flotante y la grabadora de clases y reuniones (con su vídeo). Equipo de
referencia: Core i3 de 3.ª generación, 4 GB, Windows 10 21H2 / 11.

El vídeo de la reunión es otro motor (`pixpin-motor-video`); este solo le da el sonido, por el
contrato de [PCM](#el-pcm-para-el-video-y-los-demás).

Lo que se aprendió de OBS, wasapi-rs, cpal, el ejemplo ApplicationLoopback de Microsoft,
speexdsp, rubato, zita-ajbridge, PipeWire y Oboe, y qué se cambió por ello, está en
[`investigacion/2026-10-25-motor-de-audio-lecciones.md`](investigacion/2026-10-25-motor-de-audio-lecciones.md).

---

## 1. Quién usa qué

| Quién | Por dónde | Qué pide |
|---|---|---|
| Notas de voz, dictado, micrófono flotante | `pixpin_audio::Grabadora::empezar` | micrófono, mono, realce de la voz |
| Grabar clase / reunión | `Grabadora::empezar_con_copia(…, Fuentes::LAS_DOS, copia)` | micrófono + equipo |
| Vídeo de la reunión | la `Copia` de siempre, o `Grabadora::empezar_con_pcm` | bloques i16 con posición |
| Cualquier cosa nueva | `pixpin_motor_audio::MotorAudio::empezar(Opciones { … })` | todo |
| Otro sistema (Android, pruebas) | `MotorAudio::empezar_en(Arc<impl Sistema>, Opciones)` | lo mismo |

`Grabadora` mantiene su cara de siempre (el `.parte` que se renombra al parar bien, los picos de
la onda, el mínimo de 700 ms, `ErrorAudio` con `SinMicrofono` / `SinSonidoDelEquipo` /
`SinCodificadorAac`), y por debajo es el motor.

```rust
let (sus, rx) = pixpin_motor_audio::suscripcion(512);
let motor = MotorAudio::empezar(Opciones {
    microfono: Some(Microfono::Defecto),
    equipo: Some(Equipo::Proceso(pid_de_chrome)), // o Equipo::Todo
    salida: Salida { m4a: Some(ruta), canales: 2, muestreo: 48_000 },
    realce_voz: true,
    paso_alto: true,
    cancelar_eco: true,             // reunión con altavoces, sin cascos (§5)
    pcm: vec![sus],
    ..Opciones::default()
})?;
motor.pausar(); motor.seguir();
let n = motor.niveles();          // pico y RMS de cada fuente y de la mezcla
let e = motor.estadisticas();     // capturado, silencio puesto, deriva en ppm, reaperturas, saltos…
let r = motor.terminar()?;        // Resumen { marcos, duracion_ms, m4a, estadisticas, … }
```

---

## 2. Las piezas y los hilos

```
                Sistema (Windows / Android / pruebas)
                 ├─ captura(pedido) ─> Plataforma + Aparato        (en el hilo de cada fuente)
                 ├─ salida_aac(…)   ─> SalidaAac                   (en el hilo del .m4a)
                 ├─ prioridad(tarea), ahora()
                 │
pixpin-microfono: capturar() ─ Aparato ─> Toma ──anillo──┐                  ┌──> pixpin-m4a  (escribir_aac)
pixpin-equipo:    capturar() ─ Aparato ─> Toma ──anillo──┴─> pixpin-mezcla ─┼──> suscriptores PCM (vídeo, onda, …)
                                                             (nucleo)       └──> niveles y cuentas (atómicos)
```

Todo lo que no toca el sistema es puro, se prueba en Linux y compila para `aarch64-linux-android`
tal cual: `formato`, `anillo`, `reloj`, `toma`, `deriva`, `remuestreo`, `dsp`, `nucleo`, `pcm`,
`medidas`, `plataforma`, `empuje` y `MotorAudio` mismo. Lo de Windows (`sistema/`, `cfg(windows)`)
es solo el `Sistema` de Windows: WASAPI, los avisos de aparatos, la captura por proceso, Media
Foundation, MMCSS y el QPC.

- **Un hilo por fuente** (`plataforma::capturar`). Con la prioridad más alta que dé el sistema
  (en Windows MMCSS «Pro Audio»; si no está, `THREAD_PRIORITY_TIME_CRITICAL` a mano, como cpal).
  Espera el aviso de la tarjeta **como mucho 10 ms**, vacía todos los paquetes y escribe en su
  anillo. **Nunca espera a nadie**: el anillo hacia la mezcla es una cola de un productor y un
  consumidor sin cerrojos (`anillo.rs`); si no cabe, se tira y se cuenta.
- **La mezcla** (`motor.rs` + `nucleo.rs`), prioridad «Audio» (o `THREAD_PRIORITY_HIGHEST`).
  Cada 10 ms (15,6 con el temporizador por defecto de Windows: da igual) mira el reloj del sistema
  —el mismo QPC con el que se fechan los paquetes— y saca **exactamente** los marcos que tocan.
- **El `.m4a`** (`plataforma::escribir_aac`, con la `SalidaAac` del sistema): un suscriptor más,
  con cola de ~30-45 s. Codificar AAC en un i3 cuesta lo suyo y no puede retrasar la mezcla.

Nada de la captura ni de la mezcla reserva memoria en marcha: anillos, bloques, historial del
remuestreador y la línea de retardo del limitador se reservan al abrir. Lo que un aparato dice
de sí mismo se acota (`toma::marcos_de_paquete`: la captura por proceso de Windows dice tener un
anillo de 3 131 961 357 marcos, wasapi-rs lo documenta).

---

## 3. Captura (WASAPI)

- **Modo compartido, por evento** (`AUDCLNT_STREAMFLAGS_EVENTCALLBACK`), anillo de la tarjeta de
  **1 s** (holgura, no retraso: se vacía en cuanto avisa; OBS pide 5 s).
- **El formato de la mesa** (`GetMixFormat`) tal cual, sin pedirle a Windows que convierta.
  `formato.rs` lee el `WAVEFORMATEX` / `WAVEFORMATEXTENSIBLE` de sus bytes: `SubFormat`
  `KSDATAFORMAT_SUBTYPE_IEEE_FLOAT` (32 y 64 bits) o `…_PCM` (8, 16, 24 empaquetado, 32 con 24
  válidos), y la máscara de canales (si falta o no cuadra, la habitual). Baja a mono o estéreo con
  la matriz ITU (centro a -3 dB en los dos lados, traseros y laterales a -3 dB en el suyo, sin
  subgrave); un micrófono mono va entero a los dos lados. Un bucle por tipo de muestra (sin
  `match` por muestra) y caminos directos para mono, estéreo y estéreo a mono; probados contra la
  matriz con 1, 2, 6 y 8 canales de los seis tipos.
- **Cada paquete**: `GetNextPacketSize` hasta 0; `GetBuffer` → copia/convierte → `ReleaseBuffer`
  con los mismos marcos, siempre. `AUDCLNT_BUFFERFLAGS_SILENT` = silencio aunque el buffer traiga
  basura; `DATA_DISCONTINUITY` se cuenta y hace que su hueco se crea desde 1 ms;
  `TIMESTAMP_ERROR` (o fecha 0, o del futuro) → se fecha con el reloj de ahora menos lo que dura
  (como OBS sin `useDeviceTiming`). Colocar el paquete es `toma.rs`, igual en todas las
  plataformas.
- **El sonido del equipo** es loopback (`AUDCLNT_STREAMFLAGS_LOOPBACK`) sobre los altavoces por
  defecto, **mantenido despierto** con un flujo de silencio en esos mismos altavoces (`Latido`,
  ver abajo). **Por proceso** (`Equipo::Proceso(pid)`, Windows 10 2004+; el ejemplo de Microsoft
  pide la 20348):
  `ActivateAudioInterfaceAsync("VAD\Process_Loopback")` con
  `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK` e `INCLUDE_TARGET_PROCESS_TREE` (el sonido de
  Chrome sale de un hijo). Ese cliente no tiene `GetMixFormat`, `IsFormatSupported`,
  `GetDevicePeriod` ni `GetCurrentPadding` (`E_NOTIMPL`), y su `GetBufferSize` miente: se le da el
  formato (flotante estéreo a 48 kHz, o 16 bits) con `AUTOCONVERTPCM` y se acota el paquete. El
  aviso de la activación lo implementa `#[implement]` de windows-rs, que ya es ágil
  (`IAgileObject` + el *marshaler* libre, lo que en C++ es `FtmBase`), y el hilo es MTA. La
  comprobación de que se puede es la propia activación; si falla al empezar, se graba todo el
  equipo y `Estadisticas::equipo_por_proceso` dice `false`. Si funcionó, al volver a abrir se
  insiste en el proceso (nunca se pasa a todo el equipo a mitad de una reunión). Si el proceso
  termina, Windows entrega silencio: la línea de tiempo sigue.
- **Un aparato cogido en exclusiva** por otra aplicación (`AUDCLNT_E_DEVICE_IN_USE`) o el servicio
  de audio parado (`AUDCLNT_E_SERVICE_NOT_RUNNING`) se dicen con su paso en el error; a mitad de
  grabación se sigue reintentando (§7).

### El silencio del equipo

Windows **no entrega nada** del loopback mientras no suena nada (ni ceros, ni paquetes). Sin
corregirlo, lo de después se pegaría a lo de antes y esa fuente se adelantaría para siempre. Dos
defensas:

1. **El latido** (`sistema/captura.rs`, `Latido`): un cliente de reproducción en los mismos
   altavoces que suelta silencio (`AUDCLNT_BUFFERFLAGS_SILENT`, sin tocar el buffer) en cada
   vuelta. Con un flujo en marcha la mesa no se para y el loopback entrega paquetes (de silencio)
   sin parar: es lo que recomienda la documentación de «Loopback Recording» y lo que intenta el
   `ClearBuffer` de OBS. `EstadisticasFuente::latido` dice si se pudo.
2. **El relleno por el reloj** (`reloj.rs`, `Relleno`), por si no hay latido o falla: con las
   fechas QPC (`pu64QPCPosition`, 100 ns) de cada paquete,
   - un paquete que empieza más de **20 ms** después de donde acabó el anterior → ese hueco, en
     silencio, antes de él (en el micrófono, que no calla nunca, **70 ms**: por debajo es temblor
     de las fechas, el `TS_SMOOTHING_THRESHOLD` de OBS; tras una discontinuidad marcada, 1 ms);
   - sin paquetes durante **60 ms** → silencio al paso del reloj, hasta 30 ms antes de ahora (lo
     que aún puede estar de camino);
   - si se puso silencio de más (el sonido llegó antes de lo supuesto) → se recorta del principio
     de lo que llega. La línea de tiempo queda exacta;
   - cada paquete se coloca contra **su fecha**, no sumando duraciones: un cristal un 0,1 % rápido
     no mete silencios falsos (probado una hora simulada);
   - **un salto de más de 2 s** (el `MAX_TS_VAR` de OBS: el portátil se suspendió, o el proceso
     estuvo congelado) no se rellena: se sigue desde ahí y la mezcla lo trata como una pausa
     (`reloj::avance_de_mezcla`). Sin esto, abrir la tapa tras una hora metía una hora de
     silencio en la nota. `EstadisticasFuente::saltos` lo cuenta.

---

## 4. Reloj y deriva

La salida va **al paso del reloj del sistema** (QPC): un segundo de pared son exactamente 48 000
marcos, sin contar las pausas ni los saltos. Es el mismo reloj que usa el vídeo.

Cada tarjeta tiene su cristal (48 007 Hz uno, 47 995 otro: 0,7 s de separación en una hora). Cada
fuente se remuestrea a la salida con una relación que se ajusta sola (`deriva.rs`), mirando cuánto
espera en su anillo:

- Colchón de **50 ms** (`nucleo::OBJETIVO_MS`): se ceba antes de leer y, al cebar, se tira lo que
  el último paquete trajo de más, para empezar con el anillo **justo** en su sitio (el estado
  SYNC2 de zita-ajbridge).
- **La medida fina** (`Entrada::medida_fina`): lo que hay en el anillo justo al mirar sube un
  paquete de golpe y baja poco a poco (diente de sierra de 10 ms, 30 con unos cascos Bluetooth).
  Cada toma **sella** su anillo con hasta cuándo llega lo escrito; la mezcla le suma lo que la
  tarjeta lleva capturado desde ese sello y le resta su media de 5 s: una cuenta lisa (como la que
  interpola zita-ajbridge entre las fechas de la tarjeta) con el mismo punto de equilibrio.
- PI sobre el error del anillo en segundos, amortiguado crítico, medida suavizada 1 s. Con la
  medida fina, **enganche rápido**: constante de 5 s los primeros 30 s (y el cambio de tono puede
  ir a 2 000 ppm/s), luego 30 s con traspaso de la parte proporcional a la integral (sin tirón),
  como zita-ajbridge (1 Hz → 0,05 Hz) o PipeWire (`SPA_DLL_BW_MAX` → `SPA_DLL_BW_MIN`). Medido:
  una tarjeta a ±500 ppm queda aprendida (±50 ppm) en **14 s** en vez de en 90-100. Sin medida
  fina (una plataforma que no selle) va siempre a 30 s: con el diente de sierra, un lazo rápido
  le daría un tirón al tono en cada paquete. La corrección no cambia más de 200 ppm/s ni pasa de
  ±0,5 %.
- Si un atasco deja más de 50 + 200 ms, se tira lo que sobra; si se queda sin nada, esa fuente es
  silencio hasta tener otra vez su colchón. La integral (lo aprendido de la tarjeta) se conserva.

Mantener el anillo en su nivel es mantener la fuente a la par del reloj: el retraso de cada
fuente queda fijo (~41 ms en las pruebas) y las dos no se separan. Pruebas: diez minutos a +400 y
-400 ppm, con medida gruesa y con fina → el retraso de cada una se mueve menos de 5 ms y entre las
dos hay menos de 20 ms; unos cascos Bluetooth simulados (paquetes de 30 ms, +300 ppm) se enganchan
en 30 s y no se quedan sin nada en cinco minutos; la deriva sola, una hora entera a ±500 ppm.

### El remuestreador

`remuestreo.rs`: seno enventanado (Kaiser β = 7), 48 coeficientes, tabla de 128 fases con
interpolación entre fases, posición en coma fija 32.32 (exacta: el resultado no depende de cómo se
trocee la entrada). Es de la familia de la calidad 3-4 de speexdsp (48 coeficientes, ~80 dB) y
de los `Async` con seno de rubato, con menos coste. Al bajar de tasa el corte baja con ella
(anti-solapamiento) y, al bajar mucho (96 o 192 kHz → 48), **el filtro se alarga** con la relación
(96 y 192 coeficientes, como el `filt_len` de speexdsp): 30 kHz a 192 kHz se colaba a -52 dB con
48 coeficientes y ahora queda por debajo de -60. Los productos van en ocho acumuladores
independientes (sin la cadena de dependencias de una suma en una variable; vectorizable con SSE).
Medido: error < -60 dB de 100 Hz a 15 kHz en 44,1 → 48 kHz; un tono de 23 kHz al bajar a 44,1 queda
por debajo de -50 dB. Coste: 48 MAC por marco y canal, < 1 % de un i3 con dos fuentes estéreo.

---

## 5. Tratamiento (todo en f32, en su sitio)

```
micrófono: paso alto 80 Hz → puerta (opcional) → realce de la voz → ganancia ┐
equipo:                                                          ganancia ──┴→ suma → limitador → i16
```

- **Paso alto** Butterworth 80 Hz (`paso_alto`).
- **Puerta** (`puerta`, apagada por defecto): abre a -50 dBFS, cierra a -56, espera 150 ms, baja a
  -30 dB (no a cero).
- **Realce de la voz** (`realce_voz`): el control de ganancia de siempre de `pixpin-audio` (picos a
  -3 dBFS, de 0 a +24 dB, ataque 10 ms, suelta 300 ms, sin subir el silencio). **Solo al
  micrófono**, como antes.
- **Ganancia** por fuente, cambiable en marcha (`ganancia_microfono` / `ganancia_equipo`), sin
  chasquido.
- **Limitador** sobre la suma: umbral -1 dBFS, 1 ms de mirada, vuelta en 100 ms, y una curva suave
  detrás que nunca deja pasar de 0,9999. **Nunca** hay recorte en seco ni vuelta del entero. Pasar
  a 16 bits redondea sin `f32::round` (sin SSE4.1, el procesador de referencia, es una llamada por
  muestra).

### El eco (`cancelar_eco`)

Grabando una reunión **con altavoces** (sin cascos), lo que dice la otra parte entra dos veces: por
el sonido del equipo y colado por el micrófono, unos milisegundos después (eco, o voz «doble»).
Con `Opciones::cancelar_eco` (apagado por defecto) se le pide a Windows lo que ya sabe hacer:

1. El micrófono se abre **como de comunicaciones** (`IAudioClient2::SetClientProperties` con
   `AudioCategory_Communications`, antes de `Initialize`): el controlador pone su procesado de
   llamadas —cancelador de eco y supresión de ruido— si lo tiene (y Windows 11 24H2 puede poner el
   suyo, Voice Clarity, en los que no).
2. En Windows 11 22H2+, si el micrófono lo admite, se le dice que la referencia son los altavoces
   por defecto (`IAcousticEchoCancellationControl::SetEchoCancellationRenderEndpoint(NULL)`; lo de
   `examples/aec.rs` de wasapi-rs).
3. Abrir un flujo de comunicaciones hace que Windows baje un 80 % el resto del sonido (la reunión
   que se graba): se le dice que no (`IAudioSessionControl2::SetDuckingPreference(TRUE)`).

`EstadisticasFuente::eco` dice qué se consiguió (0 nada, 1 comunicaciones, 2 con referencia). Va
apagado porque cambia el sonido del micrófono (el procesado de llamadas suele llevar supresión de
ruido y control de ganancia del controlador) y porque no se ha probado aún en un equipo de
verdad; `Grabadora` no lo pide todavía. Un cancelador propio en Rust (el MDF de speexdsp:
`libspeexdsp/mdf.c`, filtro adaptativo por bloques en frecuencia) tiene aquí lo más difícil
hecho —la referencia (el sonido del equipo) y el micrófono ya van alineados por el mismo reloj—,
pero son ~1 300 líneas con FFT y un 3-5 % de un i3: queda anotado en la investigación.

---

## 6. Formatos de salida

- **`.m4a`**: AAC-LC en MPEG-4, `IMFSinkWriter` + codificador AAC del sistema. Mono a 96 kbps
  (12 000 B/s, lo más bajo que admite) o estéreo a 128. 44 100 o 48 000 Hz. `Grabadora` escribe
  siempre **mono a 48 000**, como antes: lo leen el móvil, el reproductor del chat
  (`IMFMediaEngine`) y `pixpin-voz`, que lo descodifica a 16 kHz mono con `IMFSourceReader` sin
  mirar con qué tasa se escribió. Un bloque que se perdiera en la cola se escribe como silencio:
  la duración es siempre la buena. Si escribir falla (disco lleno), se deja de escribir, se sigue
  vaciando la cola y **se cierra igual** (`Finalize` siempre, una vez): lo escrito queda legible.

### El PCM, para el vídeo y los demás

`pcm.rs`. `suscripcion(capacidad)` da `(Suscripcion, Receptor)`; la suscripción va en
`Opciones::pcm` (recibe desde el marco 0) o se pide en marcha con `MotorAudio::suscribir_pcm`.

- `BloquePcm { posicion, muestreo, canales, muestras: Vec<i16> }`, intercalado, a tasa y canales
  fijos. `posicion` = marcos desde el principio; en pausa (o en un salto del reloj) no llega nada
  y no avanza.
- Cola **acotada**; la mezcla nunca espera: si un receptor va tarde, el bloque se le tira **solo a
  él** (`Receptor::perdidos`, `Estadisticas::pcm_perdidos`); la posición del siguiente deja ver el
  hueco.
- Al terminar, la cola se cierra: `recibir()` devuelve `None`.
- `Receptor::devolver(bloque)` reutiliza la memoria (opcional).

La `Copia` de `pixpin-audio` (un `Sender<Vec<i16>>` mono) sigue igual para quien ya la usaba.

### Niveles

`niveles()` → pico y RMS lineales (0-1) de cada fuente (tras su ganancia) y de la mezcla. Atómicos,
sin cerrojos. `Grabadora::nivel()` sigue dando 0-32767.

---

## 7. Cambios de aparato

`plataforma::capturar`, el mismo bucle en todas las plataformas, con lo de OBS
(`WASAPISource::CaptureThread` / `ReconnectThread`) y lo que allí falta:

- `IMMNotificationClient` (`sistema/aparatos.rs`) en cada hilo que sigue al aparato por defecto
  (`Microfono::Defecto`, `Equipo::Todo`): el aviso solo sube un contador (en los avisos no se puede
  esperar ni llamar al enumerador) y el hilo de captura lo ve en ≤ 10 ms. Entonces pregunta cuál es
  el de por defecto y **solo vuelve a abrir si es otro** (Windows repite el aviso; OBS compara el
  id en `SetDefaultDevice`).
- Con `AUDCLNT_E_DEVICE_INVALIDATED` (desenchufado) o cualquier error de la tarjeta: se cierra,
  **se sigue poniendo silencio por el reloj** y se vuelve a abrir en el acto; si no se puede, a los
  250 ms, 500 ms, 1 s, 2 s y desde ahí cada 3 s (el `RECONNECT_INTERVAL` de OBS). Solo el primer
  fallo seguido va al registro como aviso. La grabación no se corta y lo de después cae en su
  sitio. Probado en Linux con una plataforma simulada (un micrófono que se va, falla tres veces y
  vuelve a otra tasa: cinco segundos de pared son cinco de anillo).
- El anillo conserva su tasa toda la grabación; si el aparato nuevo va a otra, se remuestrea en la
  toma antes de escribir.
- `Microfono::Aparato(id)`: si no está, se usa el de por defecto.
- Las cuentas (`EstadisticasFuente::reaperturas`, `silencio`, `faltaron`, `tirados`, `perdidos`,
  `discontinuidades`, `saltos`, `ppm`, `eco`, `latido`) dicen qué pasó.

Sin permiso de micrófono en la privacidad de Windows (`E_ACCESSDENIED`) es `SinMicrofono`.

---

## 8. Android

El núcleo (todo menos `sistema/`) compila para Android sin tocar nada
(`cargo clippy -p pixpin-motor-audio --target aarch64-linux-android`). Lo que falta es el
**sistema de Android**: un `impl Sistema` que dé la captura de cada fuente y la salida AAC. Las
pruebas `motor::pruebas::un_sistema_que_empuja_como_android_graba_igual` y las de `empuje.rs`
hacen ya ese camino en Linux.

### Qué se reutiliza y qué es nuevo

| Pieza | En Android |
|---|---|
| `formato` (de bytes a f32) | no hace falta: Android da 16 bits o flotante intercalado, que convierte `empuje` |
| `empuje` (aparato empujado) | **sí**: la llamada de datos de AAudio, o el bucle de `AudioRecord.read` por JNI, empuja aquí |
| `toma`, `reloj`, `deriva`, `remuestreo`, `nucleo`, `dsp`, `pcm`, `plataforma::capturar`, `MotorAudio` | **igual** |
| `plataforma::escribir_aac` | igual; la `SalidaAac` es `MediaCodec` + `MediaMuxer` (por JNI) |
| El reloj (`Sistema::ahora`) | `CLOCK_MONOTONIC` en 100 ns (`clock_gettime`, o `System.nanoTime()/100` en Kotlin): es el de las fechas de AAudio y de `AudioRecord.getTimestamp` |
| La prioridad | la del hilo de AAudio ya es de tiempo real; para `AudioRecord`, `Process.setThreadPriority(THREAD_PRIORITY_URGENT_AUDIO)` |

### La captura

**Micrófono.** Dos caminos, los dos empujan en un `Empujador`:

- **AAudio / Oboe** (nativo, el mejor): `AAudioStreamBuilder_setDirection(INPUT)`,
  `setPerformanceMode(AAUDIO_PERFORMANCE_MODE_POWER_SAVING)` (grabar no necesita baja latencia y así
  la batería dura; `LOW_LATENCY` solo para el micrófono flotante en directo), `setSharingMode(SHARED)`,
  `setFormat(AAUDIO_FORMAT_PCM_I16)`, `setSampleRate(48000)` (si el aparato no lo da, Oboe convierte
  con `setSampleRateConversionQuality(Medium)`; o se deja la suya y remuestrea el motor),
  `setInputPreset(…)` (abajo), `setDataCallback` que hace `empujar_i16(bloque, fin)` con
  `fin = AAudioStream_getTimestamp(CLOCK_MONOTONIC)` llevado al final del bloque, y
  `setErrorCallback`: con `AAUDIO_ERROR_DISCONNECTED` (unos cascos que se quitan; antes de la API 37
  AAudio desconecta en **todo** cambio de ruta) se cierra el flujo **en otro hilo** (Oboe:
  `onErrorAfterClose`), se suelta el `Empujador` y el motor vuelve a abrir con sus reintentos.
- **`AudioRecord`** (Java/Kotlin, cuando no se quiere NDK): un hilo con `read(short[], …)` en bucle
  que llama a una función JNI `empujar`.

**El preset del micrófono**, según para qué:

| Uso | `AudioSource` / `InputPreset` | Por qué |
|---|---|---|
| Notas de voz, dictado (a texto) | `VOICE_RECOGNITION` (el de Oboe por defecto) | sin AGC ni supresión agresiva, latencia baja; el realce lo pone el motor |
| Grabar una clase (el móvil lejos) | `CAMCORDER` (o `MIC`) | micrófono orientado al frente, con su AGC |
| Reunión con altavoz (eco) | `VOICE_COMMUNICATION` | el AEC y el NS del fabricante; o `AcousticEchoCanceler.create(sessionId)` / `NoiseSuppressor.create(sessionId)` si `isAvailable()` |
| Calidad máxima | `UNPROCESSED` (si `AudioManager.getProperty(PROPERTY_SUPPORT_AUDIO_SOURCE_UNPROCESSED)`) | sin nada; muy bajo de volumen: el realce del motor es imprescindible |

`VOICE_COMMUNICATION` es privado por defecto (otro no puede capturarlo a la vez) y `UNPROCESSED`
no; en API 30+ se puede fijar con `setPrivacySensitive`.

**El sonido del equipo** (`AudioPlaybackCapture`, Android 10 / API 29+):

- Necesita **MediaProjection**: el usuario acepta el diálogo de grabar pantalla
  (`MediaProjectionManager.createScreenCaptureIntent()`), y desde Android 14 la proyección se
  pide **cada vez** y solo vale para una sesión.
- Va dentro de un **servicio en primer plano** con `foregroundServiceType="mediaProjection"`
  (permiso `FOREGROUND_SERVICE_MEDIA_PROJECTION` desde la API 34), arrancado **antes** de pedir la
  proyección.
- Solo captura lo que el otro deja: usos `USAGE_MEDIA`, `USAGE_GAME` y `USAGE_UNKNOWN`. **Nunca**
  `USAGE_VOICE_COMMUNICATION` (Meet, Zoom, WhatsApp en llamada: la otra parte de una reunión **no
  se puede grabar** así en Android), ni notificaciones ni el asistente. Una app que lo prohíbe
  (`android:allowAudioPlaybackCapture="false"`, o `setAllowedCapturePolicy(ALLOW_CAPTURE_BY_NONE)`)
  sale en silencio; las que apuntan a una API < 29 lo prohíben por defecto.
- Como en Windows, si no suena nada no llegan datos: el relleno por el reloj lo cubre igual (con
  las fechas de `AudioRecord.getTimestamp`).

### El AAC

`MediaCodec.createEncoderByType("audio/mp4a-latm")` con `KEY_AAC_PROFILE = AACObjectLC`,
`KEY_BIT_RATE` 96 000 (mono) o 128 000, `KEY_SAMPLE_RATE` 48 000, y `MediaMuxer(ruta,
MUXER_OUTPUT_MPEG_4)`: la `SalidaAac` es `escribir` = poner el bloque en un búfer de entrada con
`presentationTimeUs = marcos * 1_000_000 / 48_000` y sacar lo que haya de salida al muxer;
`cerrar` = `BUFFER_FLAG_END_OF_STREAM`, vaciar, `muxer.stop()` y `release()` (siempre, como el
`Finalize` de Windows: sin `stop()` el `.m4a` no se puede abrir). Sale el mismo fichero que en
Windows y el móvil ya sabe leerlo.

### Pseudocódigo (Kotlin + JNI)

```kotlin
// AndroidManifest.xml
// <uses-permission android:name="android.permission.RECORD_AUDIO"/>
// <uses-permission android:name="android.permission.FOREGROUND_SERVICE"/>
// <uses-permission android:name="android.permission.FOREGROUND_SERVICE_MICROPHONE"/>          // API 34+
// <uses-permission android:name="android.permission.FOREGROUND_SERVICE_MEDIA_PROJECTION"/>    // API 34+
// <service android:name=".Grabadora" android:foregroundServiceType="microphone|mediaProjection"/>

object Motor {                     // lo de Rust, por JNI (crate pixpin-motor-audio + un sistema Android)
    external fun empezar(m4a: String, conEquipo: Boolean): Long        // MotorAudio::empezar_en(Android, …)
    external fun empujar(fuente: Int, bloque: ShortArray, n: Int, finNs: Long)  // Empujador::empujar_i16
    external fun cerrarFuente(fuente: Int)                              // suelta el Empujador → reabrir
    external fun terminar(motor: Long): Long                            // Resumen.duracion_ms
}

class Grabadora : Service() {
    fun microfono(preset: Int = MediaRecorder.AudioSource.VOICE_RECOGNITION) = thread(name = "pixpin-mic") {
        Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_AUDIO)
        val min = AudioRecord.getMinBufferSize(48_000, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
        val rec = AudioRecord.Builder()
            .setAudioSource(preset)
            .setAudioFormat(AudioFormat.Builder().setSampleRate(48_000)
                .setChannelMask(AudioFormat.CHANNEL_IN_MONO).setEncoding(AudioFormat.ENCODING_PCM_16BIT).build())
            .setBufferSizeInBytes(maxOf(min, 48_000 * 2 / 5))            // 200 ms de holgura
            .build()
        val bloque = ShortArray(960)                                      // 20 ms
        val ts = AudioTimestamp()
        rec.startRecording()
        while (grabando) {
            val n = rec.read(bloque, 0, bloque.size)                      // bloquea hasta tenerlo
            if (n < 0) break                                              // ERROR_DEAD_OBJECT: se fue
            val fin = if (rec.getTimestamp(ts, AudioTimestamp.TIMEBASE_MONOTONIC) == AudioRecord.SUCCESS)
                ts.nanoTime + (leidos + n - ts.framePosition) * 1_000_000_000L / 48_000
            else System.nanoTime()
            leidos += n
            Motor.empujar(0, bloque, n, fin)
        }
        rec.release(); Motor.cerrarFuente(0)                              // el motor reintenta
    }

    fun equipo(proyeccion: MediaProjection) = thread(name = "pixpin-equipo") {
        val config = AudioPlaybackCaptureConfiguration.Builder(proyeccion)
            .addMatchingUsage(AudioAttributes.USAGE_MEDIA)
            .addMatchingUsage(AudioAttributes.USAGE_GAME)
            .addMatchingUsage(AudioAttributes.USAGE_UNKNOWN)
            .build()
        val rec = AudioRecord.Builder()
            .setAudioPlaybackCaptureConfig(config)
            .setAudioFormat(AudioFormat.Builder().setSampleRate(48_000)
                .setChannelMask(AudioFormat.CHANNEL_IN_STEREO).setEncoding(AudioFormat.ENCODING_PCM_16BIT).build())
            .build()
        // … el mismo bucle, con Motor.empujar(1, …)
    }
}
```

Y del lado de Rust, el sistema de Android (esbozo):

```rust
struct Android { jvm: jni::JavaVM }                      // o ndk::audio (AAudio) sin JVM
impl Sistema for Android {
    type Plataforma = PlataformaAndroid;                 // ahora() = CLOCK_MONOTONIC / 100
    type Salida = MediaCodecAac;                         // escribir / cerrar por JNI
    type Prioridad = ();
    fn captura(&self, p: &Pedido) -> Result<(PlataformaAndroid, Empujado), ErrorMotor> {
        let (empujador, empujado) = empuje(48_000, canales_de(p), p.canales, 2, "");
        dejar_en_jni(p.origen.clone(), empujador);       // Motor.empujar() lo encuentra aquí
        pedir_a_kotlin_que_arranque(&p.origen)?;         // sin permiso → ErrorMotor::SinMicrofono
        Ok((PlataformaAndroid::nueva(p.origen.clone()), empujado))
    }
    // reabrir(): otro empuje, y pedir a Kotlin que vuelva a abrir el AudioRecord.
    …
}
```

---

## 9. Probarlo

En Linux (y en la CI) va todo lo puro y **el motor entero** con sistemas de mentira, con pruebas
de verdad: barridos de senos por el remuestreador (también desde 96 y 192 kHz), anti-solapamiento,
diez minutos de deriva con dos relojes (medida gruesa y fina), el enganche rápido, unos cascos
Bluetooth simulados, una hora de deriva del control, el silencio del equipo, las suspensiones,
el limitador que nunca pasa de la escala, cabeceras `WAVEFORMATEX` de todos los tipos, cada camino
del conversor contra la matriz, el anillo entre dos hilos, el reparto del PCM, un aparato que se
va y vuelve con reintentos, avisos repetidos, el hueco y el fallo del AAC, el motor con pausa y
con un micrófono que se va, y el motor «como Android»:

```
cargo test -p pixpin-motor-audio
cargo clippy -p pixpin-motor-audio --target aarch64-linux-android -- -D warnings
```

En Windows, con micrófono y altavoces de verdad (graba 3 s de micrófono + equipo, con una pausa,
y comprueba duración, `.m4a` y PCM sin huecos; pon algo a sonar para ver el nivel del equipo; la
cancelación de eco y el sonido de un proceso):

```
cargo test -p pixpin-motor-audio -- --ignored --nocapture
cargo test -p pixpin-audio -- --ignored --nocapture
```

Para probar a mano lo que no se automatiza: desenchufar los cascos a mitad (tiene que seguir y
`reaperturas` subir), cambiar el aparato por defecto en la bandeja de sonido, grabar un minuto
con el equipo en silencio (la duración tiene que ser la de pared y `latido` verdadero), cerrar la
tapa del portátil a mitad (al volver, `saltos` = 1 y la duración sin el rato dormido), coger el
micrófono en exclusiva con otra aplicación (el error tiene que decirlo), y una reunión con
altavoces con `cancelar_eco` (la otra parte no debe oírse dos veces).
