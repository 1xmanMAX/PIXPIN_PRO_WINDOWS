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
   la grafica de la pantalla principal) y protegido para varios hilos
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
   de la GPU. Vistas de entrada y salida guardadas por textura. Si el
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
   reproductor, mire las marcas de tiempo o las duraciones.
5. **El MP4** (`archivo.rs`). Se escribe en `<destino>.parte`; al terminar,
   `Finalize` y se renombra (si no se deja, se copia; si ni eso, el `.parte`
   se queda con su contenido). Soltar el motor sin terminar borra el
   `.parte`. Si la aplicacion se cierra de golpe queda un `.parte` sin
   indice que `grabar_clase::video::barrer_restos` borra la siguiente vez.

### Por que no MP4 fragmentado de salida

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

1. **Grafica** (`Codificador::Hardware`): necesita el procesador de video y un
   MFT H.264 de hardware registrado (`hay_h264_por_hardware`). HEVC solo si se
   pide y la grafica lo tiene (`hay_hevc_por_hardware`); de salida H.264, que
   reproduce cualquier movil Android.
2. **Software** (`Codificador::Software`): el H.264 de Microsoft; la
   conversion y el encogido siguen en la GPU y solo baja el NV12 (una textura
   de escenificacion pequena).
3. **CPU** (`Codificador::Cpu`): lo de antes, `pixpin-record::Codificador`
   (BGRA baja, se encoge en la CPU con `lienzo.rs`, RGB32 por software),
   siempre a lo de «Ligero». Para cuando la grafica no tiene procesador de
   video (una maquina virtual, un controlador roto).

Cada intento que falla borra su `.parte` y deja su motivo en el registro; si
fallan todos, `ErrorMotor::SinCodificador` con los tres motivos.

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
let e = motor.estado(); // fps, fotogramas, caidos, bytes, codificador
let r = motor.terminar()?; // Resumen { ruta, duracion_ms, ... }
```

`MotorVideo` se puede mover entre hilos; todo lo que toca Windows vive en su
hilo «motor-video» (COM multihilo, Media Foundation arrancado una vez por
proceso). Soltarlo sin `terminar` tira la grabacion.

## Como probarlo en Windows

Las cuentas se prueban en cualquier maquina:

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
- Con dos graficas (Intel + NVIDIA), el dispositivo va en la de la pantalla
  principal; si su codificador no esta, se cae a software.
