# Grabar sin perder nada

> «Todo tiene que ser rápido y fiable al grabar: si se perdiera algo sería malo, se perdería una
> reunión.» — el usuario, 28-oct-2026

Este documento dice **qué garantiza PixPin Max cuando graba una clase o una reunión** («Grabar
clase», en sus tres modos: pantalla y sonido, audio y capturas, presencial) y cómo lo consigue.
Lo mismo debe valer en Android (ver el final).

## Las garantías

1. **Nada de lo grabado se borra sin que el usuario elija «Descartar»** (o tire la grabación
   con ✕ dos veces mientras graba). Ni al cerrar la barra, ni al soltar la grabadora por un
   fallo, ni cuando algo no se pudo cerrar o guardar.
2. **Si PixPin muere, se va la luz o Windows se reinicia** a mitad de una grabación, al volver a
   abrir PixPin sale:

   > **Grabación cortada** — Se cortó una grabación (Clase 28-10 10h05, 47 min) en «Física».
   > Sí: Recuperar · No: Descartar · Cancelar: preguntar al volver a abrir PixPin

   «Recuperar» mete en el chat la nota de voz, el vídeo, las capturas, las fotos de la cámara,
   los clips y lo que llegó del móvil, **cada cosa en su minuto**, dentro de su carpeta, como si
   se hubiera pulsado «Terminar», y después la pasa a texto de fondo. «Descartar» se vuelve a
   preguntar («No» por defecto) antes de borrar.
3. **Lo que se pierde en un corte de luz es, como mucho, lo de los últimos 5 segundos** de
   sonido y lo que se estuviera escribiendo en ese instante (una captura a medias, el último
   fragmento del vídeo). Un cierre de golpe de PixPin (el proceso muere, pero Windows sigue) no
   pierde nada de lo que ya llegó al sistema.
4. **El sonido es lo último que se deja de grabar.** Si el disco se llena, primero se cierra el
   vídeo (con lo grabado, que vale) y la nota sigue; si el vídeo o la cámara fallan, la nota
   sigue; si el respaldo no se puede escribir, la nota sigue sin él.
5. **Grabar no se vuelve más lento.** El respaldo es una copia del PCM que ya circulaba (el
   motor de audio lo reparte a sus suscriptores), escrita con un búfer de 256 KB; el diario es un
   JSON pequeño que se escribe cada 5 s o al cambiar algo; las capturas van al disco en vez de
   quedarse en memoria (la memoria baja: una reunión presencial son decenas de fotos de 12 MP).

## Cómo

### 1. La nota de voz: un respaldo WAV al lado

El `.m4a` que escribe Media Foundation solo se puede abrir después de `Finalize`, que escribe su
índice (`moov`) al final. Por eso, mientras se graba a prueba de cortes
(`Grabadora::empezar_a_prueba_de_cortes`, en `crates/pixpin-audio/src/entrada.rs`):

- el `.m4a` se escribe, como siempre, en `<temporal>.parte` (ver `motor-de-audio.md`);
- **el mismo PCM** (mono, 16 bits, 48 kHz) va a `<temporal>.respaldo.wav`
  (`crates/pixpin-motor-audio/src/respaldo.rs`). Un WAV no tiene índice que cerrar: cada muestra
  escrita ya vale. La cabecera lleva tamaños «sin fin» y se corrige al cerrar; al leer uno
  cortado, el tamaño sale del fichero. Cada 5 s de sonido se vacía el búfer y se pide
  `sync_data`.

**Por qué WAV y no AAC fragmentado o trozos de `.m4a`**: el WAV lo escribe nuestro código, sin
el escritor de Media Foundation por medio (que es justo lo que muere en un corte); se lee con un
`read` cualquiera; y al recuperar se vuelve a codificar a `.m4a` (`pixpin_motor_audio::
rehacer_m4a`, una hora en pocos segundos), el formato de siempre. El móvil y el transcriptor
(`pixpin-voz`, que lee con `IMFSourceReader`) no ven nunca otra cosa que el `.m4a` de siempre;
un AAC fragmentado lo leen mal el reproductor de Android y el `IMFSourceReader` al saltar.
Cuesta disco (unos 345 MB por hora, que se borran al cerrar bien) y lo vigila la guarda de
espacio.

Al parar:

| Qué pasó | Qué se hace |
|---|---|
| El `.m4a` se cerró y se renombró | se borra el respaldo |
| El `.m4a` no se cerró (disco, codificador) | se rehace del respaldo en el acto |
| Ni eso | el `.parte` y el respaldo **se quedan**; el diario queda «solo la nota» |
| Menos de 700 ms | un resbalón: se borra todo, como siempre |
| La grabadora se suelta sin parar (un pánico) | se cierra el `.m4a` y **se queda** con el respaldo |

`pixpin_audio::recuperar(temporal)` rehace la nota de lo que quede: el `.parte` si llegó a
cerrarse (tiene `moov`), si no el respaldo; los picos de la onda salen del respaldo.

### 2. El vídeo: MP4 fragmentado, rehecho a normal

El vídeo de la reunión se escribe **fragmentado** (`Opciones::fragmentado`): cada fragmento
lleva su índice, así que lo escrito hasta un corte se puede abrir. Si el escritor no lo acepta
(no todos los Windows 10 lo garantizan con `MFCreateSinkWriterFromURL`), sale un MP4 normal, como
antes.

Un fragmentado sin índice de fragmentos (`mfra`/`sidx`) salta mal en Android en un vídeo de una
hora, así que **al cerrarlo** (si le falta) y **al recuperarlo** se rehace a MP4 normal **sin
volver a codificar** (`pixpin_motor_video::rehacer_mp4`: `IMFSourceReader` en el formato de
origen → `IMFSinkWriter` en ese mismo formato; lo que tarda el disco). Si no se puede, se queda
fragmentado, que también se abre. La decisión, pura, en `archivo::recuperar_video`.

Lo que queda de un corte se mira con `pixpin_sistema::mp4::mirar` (solo las cabeceras de las
cajas de arriba: microsegundos): **cerrado** (`moov`), **fragmentado** (`moof`), **sin índice**
(un MP4 normal cortado: no lo abre nadie) o **nada**. Uno sin índice no se borra: se ofrece
descartarlo.

Antes, `grabar_clase::video::barrer_restos` borraba al empezar cualquier `.video.mp4.parte` de
otra vez; ya no existe.

### 3. El diario: todo lo demás, en el disco en el acto

Al pulsar «Empezar» se crea `<raíz>/grabaciones-a-medias/<marca>/estado.json`
(`crates/pixpin-proyecto/src/grabacion_a_medias.rs`, enganchado a la barra en
`grabar_clase/a_salvo.rs`). Lleva:

- el chat (id y nombre), el **modo**, las fuentes de sonido, el nombre de la nota («Clase 28-10
  10h05»), dónde se graban la nota y el vídeo;
- **el reloj de pared** (`Reloj`): cuándo empezó y acabó, las pausas del usuario y **los huecos**
  (el equipo se suspendió: la nota no los graba, pero la hora siguió). Con él,
  `Diario::ms_en(hora)` pasa una hora de pared a su minuto en la nota — lo que necesitan las
  fotos que llegan con hora y no con minuto (el móvil, `resto.reloj`);
- el **latido**: a qué hora la nota iba por qué minuto (cada 5 s, y en el acto al pausar);
- **cada captura y foto**: su fichero (`captura-N.png/jpg`) se escribe en la carpeta del diario
  en cuanto se hace, y su minuto en la lista; en memoria se queda solo la ruta;
- **cada clip** de la cámara: al empezar (minuto y ruta) y al cerrarse (lo que dura);
- los **marcadores** con su minuto;
- la grabación que se anunció al **móvil**, para recoger lo que llegó de él.

Todo se escribe a un `.tmp`, se lleva al disco (`sync_all`) y se renombra encima: un corte a
mitad de escribirlo deja el anterior entero (y si el bueno quedara roto, se lee el `.tmp`).

Las fotos del móvil que llegan grabando ya esperaban en una cola en disco
(`sincro/camara-movil/<grabación>/`); ahora cada una lleva al lado su ficha
(`<fichero>.json`: minuto, duración, de quién) y `camara_del_movil::recuperar_cola` las mete en
la clase recuperada.

Al guardar bien, el diario se borra. Si guardar falla, se queda entero; si solo falló la nota y
quedó su respaldo, se queda «solo la nota» (lo demás ya entró).

### 4. Recuperar al arrancar

`grabar_clase::recuperar::al_arrancar` (llamado desde `main.rs` tras retomar el asistente) mira
si existe la carpeta de diarios — sin ella no lanza ni un hilo — y, en su hilo, por cada diario:

1. `hallar`: qué queda en el disco (la nota ya cerrada, el `.parte`, lo que dura el respaldo, el
   vídeo, qué capturas y clips siguen ahí y se abren);
2. `plan`: de dónde sale la nota (ya lista → `.parte` cerrado → respaldo), el vídeo, lo que dura
   (del respaldo, exacto; si no, del último latido) y qué entra;
3. pregunta; con «Recuperar» rehace la nota y el vídeo, construye el estado de la barra con lo
   del diario y llama al mismo `guardar` que «Terminar»; después, lo del móvil y el texto de
   fondo. Si algo falla, no se toca nada y se avisa.

Lo que no se puede abrir (un vídeo sin índice, un clip sin cerrar) no se borra al recuperar: se
queda en la carpeta del chat.

### 5. El disco

`grabacion_a_medias::espacio`, con `pixpin_sistema::archivos::espacio_libre` de la carpeta del
chat:

- **antes de empezar**: con menos de 500 MB, el vídeo ni empieza (la nota sí);
- **cada 20 s grabando**: con menos de 2 GB se avisa en la barra («Queda poco sitio en el
  disco»); con menos de 500 MB se cierra el vídeo con lo grabado (`Video::cortar`) y se avisa
  («Disco casi lleno: el vídeo se cerró y el audio sigue»).

Si aun así una escritura falla: el `.m4a` deja de escribirse pero se cierra con lo que tenía
(`plataforma::escribir_aac`), el respaldo se calla sin parar nada, y al parar se intenta rehacer
la nota del respaldo; si no hay sitio ni para eso, todo se queda para recuperarlo cuando lo haya.

### 6. Suspender, bloquear, la tapa, desenchufar

- **Suspender / cerrar la tapa**: el motor de audio no rellena un salto de reloj de más de 2 s
  (`motor-de-audio.md`, «el portátil se suspendió»): la nota sigue al volver, sin una hora de
  silencio. El diario apunta el hueco en el reloj de pared (lo detecta el latido: la hora corrió
  más de 10 s por delante de la nota sin estar en pausa) y la barra dice «El equipo se suspendió
  N min; la grabación siguió al volver». El vídeo va al reloj del sonido, así que tampoco tiene
  hueco; si la gráfica se pierde al volver, el motor de vídeo la vuelve a montar
  (`recuperacion.rs`). Si la batería se acaba suspendido, es un corte: se recupera al arrancar.
- **Bloquear la sesión**: no se hace nada a propósito. WASAPI sigue capturando con la sesión
  bloqueada y la barra sigue en su hilo; la captura de pantalla de una sesión bloqueada no da
  imagen nueva y el vídeo repite el último fotograma, como con la pantalla quieta.
- **Cambiar o desenchufar el micrófono o los cascos**: el motor de audio vuelve a abrir el
  aparato (en el acto y con reintentos) y **pone silencio al paso del reloj** mientras tanto, así
  que la nota sigue continua y lo de después cae en su minuto (`motor-de-audio.md`, «Aparatos»).
  El respaldo recibe lo mismo, con el mismo silencio.
- **Desenchufar la cámara a mitad de un clip**: si al pararlo falla pero lo grabado se abre, el
  clip se guarda; si no se abre, **no se borra** (antes se borraba) y queda en la carpeta.

## Pruebas

- Puras, en Linux: `respaldo.rs` (WAV cerrado, cortado a mitad de una muestra, volcado al disco
  sin cerrar, disco lleno con `/dev/full`), `pixpin_sistema::mp4` (cerrado, cortado, fragmentado
  con y sin `mfra`, tamaños de 64 bits), `archivo::recuperar_video`, y en
  `grabacion_a_medias`: el diario de ida y vuelta, el `.tmp` que salva un diario roto, el reloj
  con pausas y una suspensión de 30 minutos, la línea de tiempo, el plan desde lo que quedó en el
  disco, «solo la nota», «Descartar» que no se lleva lo ajeno y la guarda de espacio. En
  `camara_del_movil`: la cola que sobrevive a un cierre de golpe y entra en su minuto.
- En Windows, con micrófono (`cargo test -p pixpin-audio -- --ignored`):
  `una_grabacion_cortada_de_golpe_se_recupera_del_respaldo` se lanza a sí misma como hijo, graba
  3 s y **aborta el proceso** (ni `parar`, ni `Drop`, ni `Finalize`); el padre comprueba que
  queda un `.parte` que no se abre y el respaldo, y que `recuperar` saca una nota cerrada de más
  de 2 s con picos. `una_grabadora_soltada_sin_parar_se_recupera` suelta la grabadora sin
  pararla y la recupera del `.parte` cerrado.

Queda probar a mano en el equipo de referencia (Windows 10 21H2): que el escritor acepta el MP4
fragmentado con la gráfica (si no, sale normal y lo dice `Resumen::fragmentado`), cuánto tarda
`rehacer_mp4` con una hora de vídeo, y apagar a mitad con el botón.

## Android

Android tiene los mismos cortes y alguno más (el sistema mata la app en segundo plano, el
usuario la cierra desde recientes). Lo mismo, con sus piezas:

- **El servicio en primer plano** (`foregroundServiceType="microphone"` y, con vídeo,
  `mediaProjection`) es lo que evita casi todos los cortes; lo demás es para los que quedan.
- **La nota**: `MediaRecorder`/`MediaMuxer` tampoco escriben el `moov` hasta `stop()`, así que
  vale lo mismo: el PCM del `AudioRecord` a un `.respaldo.wav` al lado (el núcleo de Rust ya
  compila para Android: `pixpin_motor_audio::respaldo` es puro), volcado con `FileDescriptor.
  sync()` cada 5 s. Al recuperar, `MediaCodec` (AAC) + `MediaMuxer` rehacen el `.m4a`.
- **El vídeo**: `MediaMuxer` no hace MP4 fragmentado. O bien se graba en **trozos** de unos
  minutos (`MediaMuxer` nuevo cada N minutos, en un fotograma clave) y al recuperar se juntan sin
  recodificar (`MediaExtractor` → `MediaMuxer`, sumando el desfase de cada trozo), o se pierde
  como mucho el trozo en curso. El sonido nunca depende del vídeo.
- **El diario**: el mismo `estado.json` (mismos campos, `serde` en el núcleo o
  `kotlinx.serialization` en Kotlin), en `filesDir/grabaciones-a-medias/<marca>/`, escrito con
  `AtomicFile` (que es justo el `.tmp` + renombrar). Cada foto de la cámara se escribe ahí en el
  acto, con su minuto.
- **Al abrir la app** (o al volver el servicio con `START_STICKY` sin intent): si hay diarios,
  una tarjeta «Se cortó una grabación (… , 47 min) — Recuperar / Descartar», con la misma
  lógica (`hallar` → `plan`).
- **El disco**: `StatFs(filesDir).availableBytes` con los mismos umbrales (2 GB, 500 MB).
- **Doze y la pantalla apagada**: con el servicio en primer plano el micrófono sigue; el latido
  apunta los huecos igual si el sistema congela el proceso.
- **Auriculares Bluetooth que se van**: `AudioDeviceCallback` y volver a abrir el `AudioRecord`
  poniendo silencio mientras tanto (el `empuje` del motor ya lo hace).
