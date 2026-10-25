# El motor de audio (`crates/pixpin-motor-audio`)

25-oct-2026. Grabar el micrófono y el sonido del equipo **a la vez, bien y gastando poco**: notas
de voz, el micrófono flotante y la grabadora de clases y reuniones (con su vídeo). Equipo de
referencia: Core i3 de 3.ª generación, 4 GB, Windows 10 21H2 / 11.

El vídeo de la reunión es otro motor (`pixpin-motor-video`); este solo le da el sonido, por el
contrato de [PCM](#el-pcm-para-el-video-y-los-demás).

---

## 1. Quién usa qué

| Quién | Por dónde | Qué pide |
|---|---|---|
| Notas de voz, dictado, micrófono flotante | `pixpin_audio::Grabadora::empezar` | micrófono, mono, realce de la voz |
| Grabar clase / reunión | `Grabadora::empezar_con_copia(…, Fuentes::LAS_DOS, copia)` | micrófono + equipo |
| Vídeo de la reunión | la `Copia` de siempre, o `Grabadora::empezar_con_pcm` | bloques i16 con posición |
| Cualquier cosa nueva | `pixpin_motor_audio::MotorAudio::empezar(Opciones { … })` | todo |

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
    pcm: vec![sus],
    ..Opciones::default()
})?;
motor.pausar(); motor.seguir();
let n = motor.niveles();          // pico y RMS de cada fuente y de la mezcla
let e = motor.estadisticas();     // capturado, silencio puesto, deriva en ppm, perdidos…
let r = motor.terminar()?;        // Resumen { marcos, duracion_ms, m4a, estadisticas, … }
```

---

## 2. Los hilos

```
pixpin-microfono ──anillo──┐                  ┌──> pixpin-m4a  (IMFSinkWriter, AAC)
  (WASAPI, «Pro Audio»)    ├─> pixpin-mezcla ─┼──> suscriptores PCM (vídeo, onda, …)
pixpin-equipo ─────anillo──┘   («Audio»)      └──> niveles y cuentas (atómicos)
  (loopback o por proceso)
```

- **Uno por fuente** (`sistema/captura.rs`). COM multihilo (`CoInitializeEx(MTA)`), prioridad
  MMCSS «Pro Audio» (`AvSetMmThreadCharacteristicsW`, de vuelta con
  `AvRevertMmThreadCharacteristics`). Espera el evento de la tarjeta **como mucho 10 ms**, vacía
  todos los paquetes y escribe en su anillo. **Nunca espera a nadie**: el anillo hacia la mezcla
  es una cola de un productor y un consumidor sin cerrojos (`anillo.rs`); si no cabe, se tira y
  se cuenta.
- **La mezcla** (`motor.rs` + `nucleo.rs`), prioridad «Audio». Cada 10 ms (15,6 con el temporizador
  por defecto de Windows: da igual) mira el reloj y saca **exactamente** los marcos que tocan.
- **El `.m4a`** (`sistema/m4a.rs`): un suscriptor más, con cola de ~30-45 s. Codificar AAC en un
  i3 cuesta lo suyo y no puede retrasar la mezcla.

Nada de la captura ni de la mezcla reserva memoria en marcha: anillos, bloques, historial del
remuestreador y la línea de retardo del limitador se reservan al abrir.

---

## 3. Captura (WASAPI)

- **Modo compartido, por evento** (`AUDCLNT_STREAMFLAGS_EVENTCALLBACK`), anillo de la tarjeta de
  200 ms (holgura, no retraso).
- **El formato de la mesa** (`GetMixFormat`) tal cual, sin pedirle a Windows que convierta.
  `formato.rs` lee el `WAVEFORMATEX` / `WAVEFORMATEXTENSIBLE` de sus bytes: `SubFormat`
  `KSDATAFORMAT_SUBTYPE_IEEE_FLOAT` (32 y 64 bits) o `…_PCM` (8, 16, 24 empaquetado, 32 con 24
  válidos), y la máscara de canales (si falta o no cuadra, la habitual). Baja a mono o estéreo con
  la matriz ITU (centro a -3 dB en los dos lados, traseros y laterales a -3 dB en el suyo, sin
  subgrave); un micrófono mono va entero a los dos lados.
- **Cada paquete**: `GetNextPacketSize` hasta 0; `GetBuffer` → copia/convierte → `ReleaseBuffer`
  con los mismos marcos, siempre. `AUDCLNT_BUFFERFLAGS_SILENT` = silencio aunque el buffer traiga
  basura; `DATA_DISCONTINUITY` se cuenta; `TIMESTAMP_ERROR` (o fecha 0) → se fecha con el reloj de
  ahora.
- **El sonido del equipo** es loopback (`AUDCLNT_STREAMFLAGS_LOOPBACK`) sobre los altavoces por
  defecto. **Por proceso** (`Equipo::Proceso(pid)`, Windows 10 2004+):
  `ActivateAudioInterfaceAsync("VAD\Process_Loopback")` con
  `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK` e `INCLUDE_TARGET_PROCESS_TREE` (el sonido de
  Chrome sale de un hijo). La comprobación de que se puede es la propia activación; si falla al
  empezar, se graba todo el equipo y `Estadisticas::equipo_por_proceso` dice `false`. Si funcionó,
  al volver a abrir se insiste en el proceso (nunca se pasa a todo el equipo a mitad de una
  reunión).

### El silencio del equipo

Windows **no entrega nada** del loopback mientras no suena nada (ni ceros, ni paquetes). Sin
corregirlo, lo de después se pegaría a lo de antes y esa fuente se adelantaría para siempre.
`reloj.rs` (`Relleno`), con las fechas QPC (`pu64QPCPosition`, 100 ns) de cada paquete:

- Un paquete que empieza más de **20 ms** después de donde acabó el anterior → ese hueco, en
  silencio, antes de él.
- Sin paquetes durante **60 ms** → silencio al paso del reloj, hasta 30 ms antes de ahora (lo que
  aún puede estar de camino).
- Si se puso silencio de más (el sonido llegó antes de lo supuesto) → se recorta del principio de
  lo que llega. La línea de tiempo queda exacta.
- Cada paquete se coloca contra **su fecha**, no sumando duraciones: un cristal un 0,1 % rápido no
  mete silencios falsos (probado una hora simulada).

---

## 4. Reloj y deriva

La salida va **al paso del reloj del sistema** (QPC): un segundo de pared son exactamente 48 000
marcos, sin contar las pausas. Es el mismo reloj que usa el vídeo.

Cada tarjeta tiene su cristal (48 007 Hz uno, 47 995 otro: 0,7 s de separación en una hora). Cada
fuente se remuestrea a la salida con una relación que se ajusta sola (`deriva.rs`), mirando cuánto
espera en su anillo:

- Colchón de **50 ms** (`nucleo::OBJETIVO_MS`): se ceba antes de leer.
- PI sobre el error del anillo en segundos, constante 30 s, amortiguado crítico; medida suavizada
  1 s; la corrección no cambia más de 200 ppm/s ni pasa de ±0,5 %.
- Si un atasco deja más de 50 + 200 ms, se tira lo que sobra; si se queda sin nada, esa fuente es
  silencio hasta tener otra vez su colchón. La integral (lo aprendido de la tarjeta) se conserva.

Mantener el anillo en su nivel es mantener la fuente a la par del reloj: el retraso de cada
fuente queda fijo (~41 ms en las pruebas) y las dos no se separan. Prueba: diez minutos a +400 y
-400 ppm (0,48 s de separación sin corregir) → el retraso de cada una se mueve menos de 5 ms y
entre las dos hay menos de 20 ms (medido: 0,06 ms). La deriva sola se prueba una hora entera a
±500 ppm.

### El remuestreador

`remuestreo.rs`: seno enventanado (Kaiser β = 7), 48 coeficientes, tabla de 128 fases con
interpolación entre fases, posición en coma fija 32.32 (exacta: el resultado no depende de cómo se
trocee la entrada). Al bajar de tasa el corte baja con ella (anti-solapamiento). Medido: error
< -76 dB de 100 Hz a 15 kHz en 44,1 → 48 kHz; un tono de 23 kHz al bajar a 44,1 queda por debajo
de -50 dB. Coste: 48 MAC por marco y canal, < 1 % de un i3 con dos fuentes estéreo.

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
  detrás que nunca deja pasar de 0,9999. **Nunca** hay recorte en seco ni vuelta del entero.

---

## 6. Formatos de salida

- **`.m4a`**: AAC-LC en MPEG-4, `IMFSinkWriter` + codificador AAC del sistema. Mono a 96 kbps
  (12 000 B/s, lo más bajo que admite) o estéreo a 128. 44 100 o 48 000 Hz. `Grabadora` escribe
  siempre **mono a 48 000**, como antes: lo leen el móvil, el reproductor del chat
  (`IMFMediaEngine`) y `pixpin-voz`, que lo descodifica a 16 kHz mono con `IMFSourceReader` sin
  mirar con qué tasa se escribió. Un bloque que se perdiera en la cola se escribe como silencio:
  la duración es siempre la buena.

### El PCM, para el vídeo y los demás

`pcm.rs`. `suscripcion(capacidad)` da `(Suscripcion, Receptor)`; la suscripción va en
`Opciones::pcm` (recibe desde el marco 0) o se pide en marcha con `MotorAudio::suscribir_pcm`.

- `BloquePcm { posicion, muestreo, canales, muestras: Vec<i16> }`, intercalado, a tasa y canales
  fijos. `posicion` = marcos desde el principio; en pausa no llega nada y no avanza.
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

- `IMMNotificationClient` (`sistema/aparatos.rs`) en cada hilo que sigue al aparato por defecto
  (`Microfono::Defecto`, `Equipo::Todo`): el aviso solo sube un contador (en los avisos no se puede
  esperar ni llamar al enumerador) y el hilo de captura lo ve en ≤ 10 ms.
- Al cambiar el de por defecto, o con `AUDCLNT_E_DEVICE_INVALIDATED` (desenchufado) o cualquier
  error de la tarjeta: se cierra, **se sigue poniendo silencio por el reloj** y se vuelve a abrir
  en el acto y luego cada 500 ms. La grabación no se corta y lo de después cae en su sitio.
- El anillo conserva su tasa toda la grabación; si el aparato nuevo va a otra, se remuestrea en la
  captura antes de escribir.
- `Microfono::Aparato(id)`: si no está, se usa el de por defecto.
- Las cuentas (`EstadisticasFuente::reaperturas`, `silencio`, `faltaron`, `tirados`, `perdidos`,
  `discontinuidades`, `ppm`) dicen qué pasó.

Sin permiso de micrófono en la privacidad de Windows (`E_ACCESSDENIED`) es `SinMicrofono`.

---

## 8. Probarlo

En Linux (y en la CI) va todo lo puro, con pruebas de verdad: barridos de senos por el
remuestreador, anti-solapamiento, diez minutos de deriva con dos relojes, una hora de deriva del
control, el silencio del equipo, el limitador que nunca pasa de la escala, cabeceras
`WAVEFORMATEX` de todos los tipos, el anillo entre dos hilos y el reparto del PCM:

```
cargo test -p pixpin-motor-audio
```

En Windows, con micrófono y altavoces de verdad (graba 3 s de micrófono + equipo, con una pausa,
y comprueba duración, `.m4a` y PCM sin huecos; pon algo a sonar para ver el nivel del equipo):

```
cargo test -p pixpin-motor-audio -- --ignored --nocapture
cargo test -p pixpin-audio -- --ignored --nocapture
```

Para probar a mano lo que no se automatiza: desenchufar los cascos a mitad (tiene que seguir y
`reaperturas` subir), cambiar el aparato por defecto en la bandeja de sonido, y grabar un minuto
con el equipo en silencio (la duración tiene que ser la de pared).
