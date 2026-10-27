# El móvil como cámara de la reunión del PC (27-oct-2026)

Guía para el agente de **PixPin Android** (y referencia para `apple/`).
Lo pidió el usuario: «ya que estamos en Windows, agregar una función
especial para conectar con Android, para que el MÓVIL tome las fotos y se
envíen directamente y se adjunten aquí» (a la reunión que se está grabando
en el PC). Y luego: «primero el PC le pasa al móvil el tiempo de la
grabación, para que el móvil marque cada foto con el minuto de la grabación
en que se hizo; luego la manda y el PC la pone justo ahí». Y también
**clips de vídeo cortos** grabados con el móvil durante la reunión.

**No hay transporte nuevo ni versión nueva del protocolo** (sigue la 4):
todo va por el canal cifrado del grupo, el mismo puerto y el mismo `suelto`
de «Enviar a tus aparatos sin código» (`al_lienzo`, ver
`docs/investigacion/2026-10-07-archivos-al-chat-abierto-android.md`). Solo
se usa si **los dos** dicen la capacidad `camara1` en el `hola`.

Código de referencia (Rust, el mismo cable byte a byte):

| Qué | Dónde |
| --- | --- |
| El cable, el reloj de la grabación y el minuto de cada foto | `crates/pixpin-sincro/src/camara.rs` |
| `suelto` con `destino: "grabacion"`, el «vale» y la reanudación | `crates/pixpin-sincro/src/al_lienzo.rs` (`responder`) |
| `camara_remota` en el `Respondedor` | `crates/pixpin-sincro/src/protocolo.rs` (`atender_camara`) |
| Lo que hace el PC con cada foto o clip | `crates/pixpin-proyecto/src/camara_del_movil.rs` |
| El móvil de mentira (lo que tiene que hacer Android) | `crates/pixpin-proyecto/tests/movil_como_camara.rs` |
| La barra de «Grabar clase» y los avisos | `apps/pixpin/src/sincronizar/camara.rs` |

## 1. Capacidad

- `camara1` en `Hola.capacidades` (junto a `diario1`, `reanudar1`). El PC
  que de verdad recibe fotos la dice (`CAPACIDADES_CON_CAMARA`); el móvil
  que sabe hacer de cámara, también.
- Sin `camara1` en los dos, no se manda nada de esto. Si se manda igual,
  un aparato de antes contesta `{"error":"No sé qué es «camara_remota»"}`
  (o «… «suelto»») **y la conexión sigue sana**: nunca se corta por un
  tipo desconocido.

## 2. Mensajes

Todos tras el `hola` de siempre, con el canal ya cifrado. Los campos con su
valor por omisión **no se escriben** (`encodeDefaults = false`); lo que no
se conoce se ignora (`ignoreUnknownKeys = true`).

### 2.1 `camara_remota`: el estado de la grabación

El **anuncio** (`Anuncio` en Rust):

```json
{"t":"camara_remota",
 "grabacion":"g-1761559500000",       // id de la grabación (no el de la nota)
 "titulo":"Clase 27-10 10h05",
 "aparato":"PORTATIL-MAX",             // nombre del PC
 "activa":true,                        // grabando (aunque sea en pausa)
 "en_pausa":false,
 "ms_actual":754000,                   // en qué ms de la grabación va AHORA
 "duracion":0,                         // al acabar: lo que duró
 "desde":1761559500000,                // inicio en el reloj del PC
 "pausas":[[1761559600000,1761559630000]],  // [desde, hasta] (hasta 0: sigue)
 "codecs":["h264","hevc"],             // lo que el PC sabe ver (clips)
 "clipMaximoMs":120000,
 "reloj":1761560284000}                // reloj del que escribe ESTE JSON
```

- `grabacion` vacío: el PC no graba nada (o la cámara está apagada en la
  barra). `activa:false` con `grabacion`: ya acabó; **lo que quede por
  subir de esa grabación aún entra** (§4.4).
- Va en los dos sentidos con el mismo `t`:
  - **El móvil pregunta** (dirige; el PC responde): manda
    `{"t":"camara_remota","reloj":t0}` y el PC contesta el anuncio con
    `reloj: t1` (su hora). El móvil apunta `t2` al recibirlo.
    **Preguntar cada 3-5 s** mientras la pantalla de cámara esté abierta (o
    PixPin a la vista con «Cámara para la reunión…»); es el camino que
    siempre funciona.
  - **El PC avisa** (dirige; el móvil responde) al empezar, pausar, seguir,
    acabar o apagar la cámara, a los móviles del grupo que contestan en su
    última dirección: manda el anuncio con `reloj: t0` y el móvil contesta
    `{"t":"camara_remota","reloj":t1}` con su hora. Si el móvil no escucha
    (PixPin cerrado), no pasa nada: lo verá al preguntar.

### 2.2 El desfase de los relojes (como NTP)

```
desfase(t0, t1, t2) = t1 − (t0 + t2) / 2      // lo que va adelantado el OTRO
```

Al preguntar el móvil: `reloj = −desfase(t0_movil, t1_pc, t2_movil)` es lo
que va adelantado el reloj del **móvil** sobre el del PC. Se manda en cada
foto (`reloj`).

### 2.3 Cada foto o clip: un `suelto` a la grabación

```json
{"t":"suelto","nombre":"IMG_20261027_100512.jpg","mime":"image/jpeg",
 "bytes":4182734,"destino":"grabacion",
 "grabacion":"g-1761559500000",
 "ms":754000,                 // minuto de la grabación, calculado en el móvil
 "tomada_en":1761560291000,   // hora del móvil al hacerla
 "reloj":7000,                // desfase del móvil (§2.2)
 "clave":"toma-7f3a…"}        // id de la toma, el mismo en cada intento
```

Un clip (MP4), además `"mime":"video/mp4"` y `"duracionMs":18000` (su `ms` es
**donde empezó**).

```
móvil → suelto (como arriba)
PC    → {}                         vale: mándalo entero
        {"desdeByte":1500000}      vale: ya tengo esto de otro intento
        {"error":"…"}              no (§5): no sale ningún trozo
móvil → TROZO… (bytes − desdeByte) + {"resumen":"<sha256 de lo mandado>"}
PC    → {"t":"listo","donde":"grabacion","ms":754000}
        {"error":"Solo se aceptan fotos (JPEG, PNG o WebP) y clips MP4"}
```

Con varias fotos, otra vez desde `suelto` en la misma conexión; al acabar,
`{"t":"adios"}`.

## 3. Cómo calcula el minuto cada lado

**El móvil lleva su propio reloj de la grabación** con el último anuncio
(`Reloj::seguir_a` en Rust):

- al recibir un anuncio en su hora `h`: «en `h` iba por `ms_actual` y estaba
  (o no) en pausa»;
- después, `ms = ms_actual + (ahora − h)` sin contar lo que lleve en pausa;
  en pausa, **congelado**; al acabar, `duracion`.
- Lo enseña en la pantalla de cámara: «● 12:34 grabando en el PC» (en pausa:
  «‖ 12:34 en pausa»).
- Cada foto lleva **`ms` de ese reloj en el momento del disparo** (no del
  envío). Un clip, `ms` del momento en que **empezó** y `duracionMs`.

**El PC** (`ms_de_la_foto`):

1. con `ms`: ese, recortado a `[0, lo que lleva o duró]`;
2. sin `ms` y con `tomada_en`: pasa su hora al reloj del PC con el desfase
   (`reloj` de la foto o, si no viene, el que midió el PC al avisar) y quita
   lo pausado;
3. sin nada: el minuto en que llega.

Un clip dura `min(duracionMs, final − ms)` (nunca pasa del final).

## 4. Qué hace el PC con lo que llega

1. **Mientras graba**: lo guarda en una cola en disco
   (`<raíz>/sincro/camara-movil/<grabacion>/`) y la barra dice
   «Foto del móvil · 12:34» y «📱 Cámara del móvil · 3».
2. **Al «Terminar»** (la nota de voz solo existe entonces): todo entra en
   la clase con su minuto.
3. **Cómo entra**: igual que una captura:
   - una `IMAGEN` (`.jpg`/`.png`/`.webp`) o un `ARCHIVO` `.mp4` (un vídeo
     para `es_video`) con **sus bytes tal cual** (el EXIF se queda);
   - `resto.enGrabacion = {"de":"<id de la nota>","ms":754000}` (un clip,
     además `"duracionMs"`);
   - `resto.fuente = "movil"` (lo de la cámara del propio PC en la reunión
     presencial dice `"camara"`; las capturas de pantalla no lo llevan),
     `resto.deAparato = "<nombre del móvil>"`, `resto.tomadaEn`. **Nunca
     `origen`** en `resto`: ese nombre ya es un campo del mensaje y la línea
     del cuaderno dejaría de leerse;
   - en la nota, en `resto.capturas` (`[{ms, id}]`) o, el clip, en
     `resto.clips` (`[{ms, id, duracionMs}]`), **la misma lista que los clips
     de la reunión presencial** (`pixpin_proyecto::presencial`);
   - dentro de la carpeta de la grabación, **en su minuto** (no al final),
     con título «📱 ⏱ 12:34» (un clip, «📱 ▶ 12:34»).
4. **Lo que llega tarde** (el móvil se quedó sin red y lo sube después):
   si es de una de las últimas 4 grabaciones, entra al momento en la clase
   ya guardada y sale un aviso en la bandeja («Del móvil a «Clase…» · 12:34»).
   El móvil sabe cuál era por `grabacion`; aunque el anuncio diga
   `activa:false`, **sigue subiendo su cola**.
5. «Estudiar la clase» enseña las fotos del móvil como capturas, y cada clip
   por el mismo camino que los de la cámara del equipo: un ▶ en su minuto
   entre los marcadores («Vídeo del móvil · 0:18»); un clic lleva ahí y lo
   abre. Un clip HEVC en un PC sin «HEVC Video Extensions» se abre igual y se
   dice qué falta (aunque, con `codecs` del §6, el móvil ya lo habrá grabado
   en H.264).

## 5. Seguridad y límites

- Solo aparatos del **mismo grupo** (el canal cifrado con la clave del
  código; una persona de un chat compartido recibe «No sé qué es»).
- Antes del «vale» el PC niega: sin `grabacion`, una grabación que no
  conoce, la cámara apagada en la barra, un `mime` que no sea
  `image/jpeg|png|webp` o `video/mp4`, una foto de más de **64 MB** o un clip
  de más de **256 MB**. Al llegar mira los primeros bytes (JPEG, PNG,
  WebP o `ftyp` de MP4 que **no** sea HEIC/AVIF) y tira lo que no cuadra.
- **HEIC no**: el móvil pasa la foto a JPEG antes (manteniendo el EXIF).

## 6. Clips de vídeo: códec y peso

- **Códec**: `HEVC (H.265)` solo si el anuncio trae `"hevc"` en `codecs`
  (el PC tiene «HEVC Video Extensions»; lo mira con Media Foundation,
  `pixpin_pin::video::sabe_hevc`); si no, **H.264** (Windows lo trae de
  serie). Sin `codecs`, el PC no recibe clips: el botón de vídeo no sale.
- **Audio** AAC 96-128 kbit/s; **imagen** 1080p hasta 30 fps, **VBR
  4-6 Mbit/s**: un clip de 2 min pesa unos 60-90 MB y se lee bien la
  pizarra.
- **Lo más largo**: `clipMaximoMs` (2 min); al llegar, la grabación se para
  sola.
- **Reanudar**: cada toma con su `clave` (un UUID fijo de esa toma). Si la
  red corta a medias, el PC guarda lo llegado en un `.parcial` (si pasa de
  1 MB) y el siguiente `suelto` con la misma `clave` y los mismos `bytes`
  recibe `{"desdeByte":N}`: se mandan solo los bytes de `N` al final, con la
  cola del resumen **de lo mandado** (como `pon` con `reanudar1`).

## 7. La pantalla del móvil (flujo)

1. **Aviso**: con PixPin a la vista, si el anuncio dice `activa`, una tira
   arriba: «**Cámara para la reunión del PC: «Clase 27-10 10h05»**» con
   «Abrir cámara». Funciona igual en todos los modos de grabación del PC
   (Pantalla y sonido, Audio y capturas, Presencial…): al móvil solo le
   importa el anuncio.
2. **Cámara a pantalla completa**: vista previa, un **disparador grande**
   abajo, a su lado el de **clip** (si `codecs` no está vacío) y arriba
   «● 12:34 grabando en el PC» (el reloj del §3). En pausa: el disparador
   sigue (la foto entra en el minuto en que se pausó).
3. **Cada disparo** se apunta en una cola persistente (Room) con su `ms`,
   `tomada_en`, `reloj` y `clave`, y **se sube en segundo plano** con
   reintentos (espera 2, 4, 8… s hasta 1 min); el visor sigue libre para la
   siguiente. Una miniatura con ✓ cuando el PC dice `listo`.
4. Al acabar la grabación (`activa:false`), la pantalla lo dice y **sigue
   subiendo lo pendiente**; cerrarla no lo tira.

## 8. Pistas para implementarlo en Android

- **Foto**: CameraX `ImageCapture` con
  `setCaptureMode(CAPTURE_MODE_MAXIMIZE_QUALITY)` y la resolución más alta
  (`ResolutionSelector` con `ResolutionStrategy.HIGHEST_AVAILABLE_STRATEGY`);
  guardar con `OutputFileOptions` a fichero (JPEG con EXIF). Si el aparato
  da HEIC, pasarlo a JPEG copiando el EXIF (`ExifInterface`).
- **Clip**: CameraX `VideoCapture<Recorder>` con
  `QualitySelector.from(Quality.FHD)`, `setTargetVideoEncodingBitRate(5_000_000)`
  y el códec del anuncio (`MediaRecorder.VideoEncoder.HEVC` o `H264`;
  con `Recorder` de CameraX ≥1.4, `VideoSpec`/`setVideoCapabilitiesSource`);
  audio AAC. Cortar a `clipMaximoMs`. Contenedor MP4.
- **Subir**: un **servicio en primer plano** (`foregroundServiceType =
  dataSync`, notificación «Subiendo 3 fotos al PC…») o `WorkManager` con
  `setExpedited`; la conexión es la de «Enviar a tus aparatos»
  (`sincro/EnviarActivity` → `Conexion.abrir`), una por tanda, `adios` al
  final.
- **El reloj**: `SystemClock.elapsedRealtime()` para el tiempo pasado desde
  el anuncio (no cambia si el usuario toca la hora); `System.currentTimeMillis()`
  para `tomada_en` y `t0/t2`.
- **Preguntar**: cada 3-5 s con la pantalla de cámara abierta; cada 15-30 s
  con PixPin a la vista (para la tira del §7.1). Responder al aviso del PC
  en el `Respondedor` (`Presencia.kt`): `camara_remota` → `{"t":"camara_remota","reloj":<ahora>}`
  y refrescar el reloj con lo anunciado.
- **Tipos desconocidos**: contestar siempre `{"error":"No sé qué es «…»"}` y
  seguir; nunca cerrar la conexión.

## 9. Lista de tareas

### Android

- [ ] `camara1` en `Protocolo.CAPACIDADES` y leer las del PC.
- [ ] `camara_remota`: preguntar (§2.1), atender el aviso, medir el desfase.
- [ ] El reloj de la grabación (§3) y la pantalla de cámara (§7).
- [ ] `suelto` con `destino:"grabacion"`, `ms`, `tomada_en`, `reloj`,
      `clave` (y `duracionMs` el clip); seguir desde `desdeByte`.
- [ ] La cola persistente y la subida en segundo plano con reintentos.
- [ ] Clips: códec según `codecs`, 1080p/30, 4-6 Mbit/s, AAC, 2 min.
- [ ] Al recibir una clase por la sincronización: `resto.clips` y
      `resto.fuente` (`"movil"` o `"camara"`) se conservan (y, si se quiere,
      se enseñan). Lo de la reunión presencial grabada en el propio móvil
      (novedades §3.3) usa este mismo formato con `fuente: "camara"`.

### Mac, iPad e iPhone (`apple/`)

- [ ] El núcleo ya tiene el cable (`apple/nucleo/pixpin-sincro`, igual que
      `crates/`). Para hacer de PC: `Camaras` de `pixpin-proyecto` y decir
      `CAPACIDADES_CON_CAMARA`.

## 10. Dónde está probado

- `crates/pixpin-sincro/src/camara.rs`: el reloj con pausas, el desfase
  NTP, el minuto con y sin `ms`, lo que llega tarde, los clips, los tipos y
  los bytes de cabecera.
- `crates/pixpin-proyecto/tests/movil_como_camara.rs`: por TCP y con el
  `Respondedor` de verdad, un móvil 7 s adelantado manda dos fotos y un clip
  durante una grabación con una pausa (una foto sin `ms`), luego una tarde;
  el PC avisa al móvil y mide su reloj; un clip cortado sigue donde se
  quedó; un PC de antes dice «No sé qué es» y sigue.
