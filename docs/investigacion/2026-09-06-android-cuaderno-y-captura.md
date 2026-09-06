# El PixPin de Android por dentro: el cuaderno, los pines y la captura

**Fecha:** 2026-09-06 · **Material:** el clon de solo lectura que el usuario dejó en
`proyectos de referencia/PIXPIN_PRO_ANDROID` (Kotlin + Jetpack Compose, `minSdk 29`,
`targetSdk 36`, versión 0.28.3, `versionCode` 81). No se ha modificado nada de ese
directorio.

## 0. Qué se ha mirado, y cómo

Se han leído los documentos propios del proyecto (`docs/formato-pixpin.md`,
`docs/motor.md`, los seis diseños de `docs/superpowers/specs/`), el manifiesto, el
`build.gradle.kts` y el código de los módulos encargados. Las líneas están **contadas**
con `find` y `wc -l`, no estimadas.

Lo que **no** se ha hecho: ejecutar la aplicación ni comprobar en un teléfono real que
los ficheros estén donde el código dice que los escribe. Todas las rutas de este informe
salen del código.

De los ficheros muy grandes (`MensajesActivity.kt`, 5.367 líneas;
`PinWindowController.kt`, 4.572) se ha leído la estructura completa —todas las funciones
y composables por firma, más el KDoc, que en este proyecto es largo y explica el porqué
de cada decisión— y en detalle los bloques de persistencia, gestos y export. No línea a
línea.

### El tamaño real, medido

| Módulo | Ficheros | Líneas | Qué es |
|---|---:|---:|---|
| `motor/` | 103 | 55.385 | El editor de lienzo (Excalidraw propio), PDF, SVG, exportación HTML |
| `croquis3d/` | 23 | 21.024 | El croquis del espacio en 3D |
| `guardados/` | 26 | 10.865 | **El cuaderno**: el chat con uno mismo |
| `pin/` | 18 | 8.140 | Los pines flotantes |
| `motormd/` | 22 | 6.964 | El motor de Markdown |
| `ui/` | 7 | 4.097 | Proyectos, editor Markdown, tema |
| `capture/` | 12 | 2.435 | La captura de pantalla |
| `mini/` | 7 | 2.031 | Las mini-aplicaciones |
| `data/` | 5 | 1.058 | Repositorios y ajustes |
| `floating/` | 4 | 786 | La bola flotante y el servicio |
| `clipboard/` | 6 | 567 | El portapapeles y las palabras mágicas |
| `capa/` | 1 | 533 | Dibujar sobre la pantalla viva |
| `annotate/` | 1 | 181 | El lector de trazos |
| raíz | 2 | 1.583 | `MainActivity`, `PixPinApp` |
| `com/k2fsa/sherpa/onnx/` | 5 | 1.579 | Enlace con sherpa-onnx (código de terceros, copiado dentro) |

Los dos módulos grandes —`motor` y `croquis3d`— **no** son objeto de este informe; el
encargo cubre el resto. Se mencionan solo cuando algo de lo inventariado depende de
ellos, que es a menudo.

---

## 1. Las dos preguntas que había que contestar

### 1.1 Las notas de voz: ¿con qué se transcriben?

**Todo ocurre en el propio teléfono. No hay ningún servidor de inferencia, ni de OpenAI
ni de nadie.** La red se usa solo para bajar el modelo la primera vez.

Hay **tres motores**, elegibles en Ajustes (clave `motor_de_voz` del DataStore):

| Motor | Fichero | Qué es de verdad | Red | Peso |
|---|---|---|---|---|
| **Vosk** (por defecto) | `guardados/MotorVosk.kt` | Kaldi vía `com.alphacephei:vosk-android:0.3.47` + JNA. Clases `org.vosk.{LibVosk, Model, Recognizer}` | Solo para bajar el modelo de `alphacephei.com/vosk/models/<modelo>.zip` | ~40 MB por idioma |
| **Whisper** | `guardados/MotorWhisper.kt` | **No es OpenAI por red**: es Whisper convertido a ONNX corriendo con **sherpa-onnx** (k2-fsa, Apache-2) en local | Solo para bajar los `.onnx` de Hugging Face | tiny 104 MB · base 161 MB · small 375 MB |
| **Google** | `guardados/MotorGoogle.kt` | `android.speech.SpeechRecognizer` **en el dispositivo** (`createOnDeviceSpeechRecognizer`, `EXTRA_PREFER_OFFLINE=true`) | No para reconocer; sí puede pedir al sistema que baje un paquete de idioma | — |

Detalles que importan para portar:

- **Vosk es el que manda.** `SettingsRepository.kt:152` fija `motorDeVoz = MOTOR_VOSK`.
  Los modelos están mapeados por idioma: `es`, `en`, `pt`, `fr`, `de`, `it`, `ca`
  (`vosk-model-small-*`), con vuelta a español si no hay. Se descomprimen a
  `filesDir/vosk/<modelo>/` con un centinela `.listo` y una guardia anti *zip-slip*.
- **Whisper solo funciona en `arm64-v8a`.** `MotorWhisper.soportado()` lo comprueba y
  avisa. Las bibliotecas nativas están **copiadas dentro del repositorio**:
  `app/src/main/jniLibs/arm64-v8a/libonnxruntime.so` (21 MB) y `libsherpa-onnx-jni.so`
  (4,6 MB), con el enlace Kotlin en `app/src/main/java/com/k2fsa/sherpa/onnx/`. Por eso
  **no aparece en `build.gradle.kts`**: no es una dependencia Gradle, es código y
  binarios vendorizados.
- **Google no es el camino principal por una razón concreta y documentada**: exige
  Android 13 (antes no existe `RecognizerIntent.EXTRA_AUDIO_SOURCE`, que es lo que
  permite darle un fichero en vez del micrófono) y falla al casar etiquetas de idioma
  —pide `es-PE`, el teléfono tiene `es-419`— diciendo «idioma no disponible» con el
  idioma ya descargado.
- **El preproceso es común y no trivial.** `guardados/Transcriptor.kt` decodifica
  cualquier formato (`.m4a`, `.ogg` de WhatsApp, `.mp3`, `.wav`, `.amr`) a PCM 16 bits
  mono 16 kHz con `MediaExtractor` + `MediaCodec` y un remuestreador lineal propio, a
  `cacheDir/pcm-<nanoTime>.raw`. Si el códec del fabricante falla, reintenta forzando el
  software (`c2.android.*` / `OMX.google.*`). Encima hay un VAD por energía RMS
  (`cortes`), agrupación en tandas (`agrupar`) y un filtro anti-alucinaciones
  (`creible`: descarta alfabetos ajenos, «Subtítulos realizados por… amara.org» y
  palabras repetidas ocho veces o más).

**Qué significa para Windows.** Los tres tienen equivalente de escritorio directo:

- **Vosk** publica binarios para Windows y tiene enlace de Rust (`vosk` en crates.io
  sobre `libvosk`). Los mismos modelos `vosk-model-small-*` valen tal cual. Es la ruta
  de menor fricción.
- **sherpa-onnx** tiene compilación oficial para Windows x64 y enlace de Rust
  (`sherpa-rs`). Los ficheros ONNX descargados en el móvil son **los mismos**: se pueden
  copiar y funcionan. La restricción `arm64-v8a` es de Android, no del modelo.
- **whisper.cpp** con `whisper-rs` es la otra opción obvia si no se quiere ONNX Runtime.
- El reconocimiento de voz **propio de Windows** (`Windows.Media.SpeechRecognition`, o
  el más nuevo de Windows 11) resolvería el caso Google, pero heredaría el mismo
  problema de idiomas y ataría el resultado a lo que el usuario tenga instalado. No
  merece la pena si Vosk ya da texto **con tiempo por palabra**, que es lo que aquí se
  usa para el formato `[1:23] lo que se dijo` y para saltar en el audio.

Conclusión: **la transcripción se porta entera**, y sin depender de nada de Android. Lo
único que hay que rehacer es el decodificador de audio a PCM: `MediaCodec` no existe en
Windows; en Rust se hace con `symphonia` (decodificación) más `rubato` o un
remuestreador propio, que es sensiblemente más simple que lo que hay aquí.

### 1.2 El formato `.pixpin`: ¿es portable?

**Sí, y está pensado justamente para esto.** El documento `docs/formato-pixpin.md` y el
código de `motor/PaquetePixpin.kt` (216 líneas) coinciden: no hay discrepancia entre lo
que se documenta y lo que se hace.

Un `.pixpin` es **un ZIP normal**, sin cifrado ni binario propio, con:

| Entrada | Qué es |
|---|---|
| `manifest.json` | `{"formato":"pixpin","version":1,"aplicacion":"pixpin-android","escrito":<ms>,"proyecto":"<nombre>"}` |
| `proyecto.json` | El `Proyecto` serializado con kotlinx.serialization, con `pdfOrigen` y `pdfLimpio` puestos a `null` |
| `lienzos/<id>.excalidraw` | Un lienzo por hoja, en **JSON de Excalidraw plano** |
| `imagenes/<id>` | Cada foto tal cual (JPEG, PNG, WEBP…); el tipo va en `files` del lienzo |
| `croquis/<id>.json` | Cada croquis del espacio, en JSON |
| `notas/<id-de-hoja>.md` | Cada nota, en Markdown (además va inline en `proyecto.json`) |
| `documento.pdf` | El PDF del proyecto, la copia limpia sin anotar |

Tres decisiones lo hacen realmente portable, y las tres están escritas en el código:

1. **Nada de rutas del teléfono.** Cada foto va dentro con su identificador y en el JSON
   del lienzo `files[<id>].path` pasa a ser `imagenes/<id>`, no una ruta absoluta. Al
   importar se le pone la ruta del aparato nuevo.
2. **Sin comprimir dos veces.** En el teléfono los lienzos viven en gzip
   (`filesDir/pins/draw/<id>.excalidraw.gz`) y los croquis también
   (`filesDir/croquis3d/<id>.croquis.gz`), pero dentro del ZIP van en **JSON plano**,
   con el argumento explícito de que «un editor de escritorio los lee sin más».
3. **Los lectores tienen que ignorar claves desconocidas.** `version` sube solo cuando
   cambia algo que un lector viejo no pueda saltarse.

`proyecto.json` es sencillo: `id`, `nombre`, `hojas[]` (con `id`, `nombre`, y uno de
`dibujo` / `nota` / `croquis`+`vista`, más `pagina` para las páginas de PDF),
`archivado`, `tocado`, `pdfOrigen`, `pdfLimpio`, `croquis[]`.

**Verdicto:** un `.pixpin` hecho en el móvil se puede abrir en Windows con una
biblioteca ZIP y `serde_json`, sin ingeniería inversa. El único trabajo real es
**entender el JSON de Excalidraw**, que es lo que ya se investigó en
`docs/investigacion/2026-09-02-excalidraw-analisis.md`, y el JSON del croquis 3D, que es
propio y no está documentado.

Un aviso honesto: el `.pixpin` es el formato de **intercambio de proyectos**. No lleva
dentro el cuaderno (`guardados.jsonl`), ni los pines (`pins.json`), ni los ajustes. Para
mover un teléfono entero a Windows haría falta más que este fichero.

---

## 2. `guardados` — el cuaderno (10.865 líneas, 26 ficheros)

Es la parte que la aplicación de Windows **no tiene en absoluto**, así que va con
detalle.

### 2.1 Qué es

Un **chat con uno mismo** que hace de cajón de documentos. El modelo es
declaradamente el *Saved Messages* de Telegram: el código cita por nombre y línea
`ChatActivity.java`, `ChatMessageCell.java`, `ChatAttachAlert.java`, `SeekBarWaveform`
y `DialogsActivity`.

Se deja algo ahí —una nota, una foto, un PDF que te mandaron, un audio— y se vuelve a
buscarlo después. Todo se ordena por día, se busca, se etiqueta, se fija arriba, se
reenvía. Es el sitio al que se llega desde la bola flotante, y desde donde se llega a
todo lo demás.

Hay **varias conversaciones**: la general y **una por cada proyecto**. La misma pantalla
sirve para todas; se conmuta desde el menú «Chats». La lista de conversaciones se ordena
por el último mensaje, como Telegram, con la general siempre arriba.

**Lo que se puede dejar ahí** (el enum `Clase`):

| Clase | Qué es | Al tocarlo |
|---|---|---|
| `NOTA` | Texto suelto. Si trae un enlace, se pinta una tarjeta | Abre el enlace en el navegador |
| `IMAGEN` | Una foto; si ya se anotó, el dibujo se compone encima | Abre el editor de lienzo, no un visor |
| `ARCHIVO` | PDF o cualquier otra cosa, con icono y tamaño | Se abre fuera, con la aplicación que toque |
| `VOZ` | Nota de voz con onda de barritas y su transcripción plegable | Reproduce |
| `DIBUJO` | Un lienzo del editor | Abre el editor |
| `PAGINA` | Una página concreta de un PDF de un proyecto, con sus anotaciones | Abre esa página para anotar |
| `PROYECTO` | Un **acceso directo** a un proyecto (no una copia, a propósito) | Va al proyecto |
| `MINIAPP` | Una lista de tareas, unos gastos, un contador… | Abre la mini-aplicación |

Y lo que la pantalla hace además: agrupación de burbujas por «rato» (ventana de 5
minutos, la misma de Telegram), separadores de día (HOY / AYER / `d MMMM` / con año),
búsqueda con resaltado, fichas por sección (TODO, FOTOS, ARCHIVOS, VOZ, DIBUJOS,
FIJADOS, BUZÓN), fijados con paseo cíclico, selección múltiple por arrastre, deslizar
para responder, hilos de comentarios con salto a la cita y botón de volver, etiquetas
emoji, y un buzón donde caducan cosas a los 7 días.

Además hay cinco pantallas alrededor que son casi aplicaciones aparte:

| Fichero | Qué es |
|---|---|
| `ConversacionActivity.kt` (406) | Una conversación por turnos: N micrófonos, uno por persona. Une los `.m4a` sin recodificar con `MediaMuxer` y produce `turnos` para que la transcripción salga como diálogo con nombres |
| `TelepronterActivity.kt` (417) | Teleprónter: el texto baja solo mientras se lee en voz alta y se graba. **No usa reconocedor**: deduce el minuto de cada párrafo de la velocidad del scroll |
| `PronunciarActivity.kt` (406) | Practicar pronunciación: mantener pulsado, hablar, soltar, oírse. Se transcribe en el idioma que se practica, no en el de Ajustes |
| `LetraActivity.kt` (240) | La letra o el texto de un audio a pantalla completa, con párrafos `[1:23]` que saltan y se resaltan |
| `BibliotecaDeAudioActivity.kt` (153) | Toda la música y todas las notas de voz de todos los chats en una lista |

Un rasgo de arquitectura que conviene copiar: **toda la lógica que decide algo vive en
ficheros puros sin Android** (`Mensajes.kt`, `Agrupacion.kt`, `Onda.kt`, `Enlaces.kt`,
`Resaltado.kt`, `EtiquetaDeDia.kt`, `Conversaciones.kt`) para poder probarla en JVM sin
teléfono. Las Activities solo pintan. El proyecto tiene 151 ficheros de test bajo
`app/src/test/`.

### 2.2 Sus piezas

- **`Mensajes.kt` (511)** — el tipo central y las funciones puras que lo manejan.
- **`MensajesStore.kt` (281)** — el almacén.
- **`MensajesActivity.kt` (5.367)** — la pantalla. Enorme, y es deuda reconocible.
- **`Conversaciones.kt` (87)** — la lista de chats, en un solo recorrido.
- **`Agrupacion.kt` (166)** — cuándo dos burbujas van pegadas y con qué radios.
- **`Transcriptor.kt` (543)** + los tres motores — véase §1.1.
- **`Reproductor.kt` (142)**, **`Onda.kt` (155)**, **`BarraDelReproductor.kt` (113)** — audio.
- **`UnirAlProyecto.kt` (276)** — convierte mensajes en hojas de un proyecto.
- **`PaginaAnotada.kt` (152)** — compone página de PDF + anotaciones, con caché LRU.
- **`Enlaces.kt` (93)**, **`Resaltado.kt` (161)**, **`EtiquetaDeDia.kt` (77)** — lógica pura de presentación.
- **`Compartir.kt` (64)**, **`GuardarCompartidoActivity.kt` (228)**, **`NuevoProyectoCompartidoActivity.kt` (142)** — entrada y salida por el menú de compartir del sistema.

Los tipos exactos:

```kotlin
@Serializable enum class Clase { NOTA, IMAGEN, ARCHIVO, VOZ, DIBUJO, PAGINA, PROYECTO, MINIAPP }

@Serializable data class Mensaje(
    val id: String,
    val cuando: Long,                    // epoch ms
    val clase: Clase,
    val texto: String = "",
    val ruta: String? = null,            // el fichero, si lo hay
    val nombre: String = "",
    val bytes: Long = 0,
    val referencia: String? = null,      // id de dibujo / hoja / proyecto — NO es un fichero
    val pagina: Int? = null,             // página del PDF, para Clase.PAGINA
    val duracionMs: Int = 0,
    val picos: List<Int> = emptyList(),  // la onda de la nota de voz
    val miniapp: String? = null,         // la palabra, no el ordinal del enum
    val proyecto: String? = null,        // null = conversación general
    val emoji: String? = null,
    val soloLaFoto: Boolean = true,
    val fijado: Boolean = false,
    val respondeA: String? = null,
    val enBuzon: Boolean = false,
    val unido: Boolean = false,          // ya volcado a hojas del proyecto
    val transcripcion: String? = null,
    val estadoDelTexto: String? = null,  // "bien" | "aviso" | "mal" | "letra"
    val hojaDelTexto: String? = null,
    val marcas: List<Int> = emptyList(), // banderitas, en ms
    val turnos: List<TurnoDeVoz> = emptyList()
)

@Serializable data class TurnoDeVoz(val quien: String, val desdeMs: Int, val hastaMs: Int)

data class Conversacion(val proyecto: String?, val nombre: String, val ultimo: Mensaje?, val cuantos: Int)
data class Tramo(val dia: Long, val mensajes: List<Mensaje>)
enum class Seccion { TODO, FOTOS, ARCHIVOS, VOZ, DIBUJOS, FIJADOS, BUZON }
```

### 2.3 Cómo guarda los datos

**No hay Room, ni SQLite, ni ORM.** Se comprobó buscando `@Entity`, `@Dao`,
`@Database`, `RoomDatabase`, `androidx.room` y `SQLiteOpenHelper` en todo el árbol: cero
resultados, y ninguna dependencia de Room en `build.gradle.kts`.

Esto **contradice el documento de diseño original**. `docs/superpowers/specs/2026-07-26-pixpin-android-design.md`,
sección 4, punto 7, dice: «Persistencia — Room (pines, historial), DataStore (ajustes),
imágenes en almacenamiento privado». Room nunca se usó. `data/PinRepository.kt` lo
explica en una línea: «Persistencia de pines e historial en JSON (dataset pequeño: no
hace falta Room)». **Manda el código.**

Lo que hay:

| Qué | Dónde | Formato |
|---|---|---|
| **Los mensajes** | `filesDir/guardados.jsonl` | **JSON Lines**: una línea = un `Mensaje`. Se añade al final; solo se reescribe entero al borrar o editar |
| Temporal de reescritura | `filesDir/guardados.jsonl.nuevo` | Se sustituye con `renameTo` (atómico), con recurso a `copyTo` |
| **Los adjuntos** | `filesDir/guardados/` | Copias propias, nombradas `<millis>_<nombre con extensión>` |
| Notas de voz del chat | `filesDir/guardados/voz_<epoch>.m4a` | AAC en contenedor MPEG-4 |
| Notas de voz de los pines | `filesDir/voz/voz_<millis>.m4a` | Idem |
| Modelos de voz | `filesDir/vosk/<modelo>/` (+ centinela `.listo`) y `filesDir/whisper/<tiny\|base\|small>/` | ZIP descomprimido / ficheros `.onnx` |
| Temporales | `cacheDir/pcm-<nanoTime>.raw`, `cacheDir/compartir/`, `cacheDir/share/` | |

El serializador es `Json { ignoreUnknownKeys = true; encodeDefaults = true }`.

Dos decisiones de diseño están razonadas en el KDoc y merecen conservarse:

1. **Una línea por mensaje, no un JSON único.** Añadir es escribir al final. Con un JSON
   único, guardar el mensaje doscientos costaría reescribir los doscientos, y aquí
   dentro va texto largo.
2. **Una línea rota no se lleva por delante el resto.** `leer()` hace
   `runCatching { decodeFromString<Mensaje>(linea) }.getOrNull()` por línea. Con un JSON
   único, una línea corrupta dejaría **todo** ilegible.

La reactividad son dos `MutableStateFlow` estáticos: `MensajesStore.cambios` (contador
que sube en cada escritura; la pantalla recarga) y `MensajesStore.avances`
(`Map<idMensaje, Float>`, el progreso de cada transcripción en curso).

### 2.4 De qué depende de Android

| Cosa | Para qué |
|---|---|
| `RECORD_AUDIO` | Notas de voz, teleprónter, pronunciar, conversación por turnos. Se pide **en caliente**, al grabar la primera |
| `INTERNET` | Solo para bajar los modelos de voz (Vosk de alphacephei, Whisper de Hugging Face) |
| `MediaRecorder` (MIC, MPEG_4, AAC, 22.050 Hz, 32 kbps) | Grabar. `getMaxAmplitude()` cada 50 ms alimenta la onda **mientras se graba**, porque sacarla del AAC después exigiría decodificar cada fila de la lista |
| `MediaPlayer` + `PlaybackParams` | Reproducir, con velocidades 1×, 1,25×, 1,5×, 2×, 0,75× |
| `MediaExtractor` / `MediaCodec` / `MediaCodecList` | Decodificar cualquier audio a PCM 16 kHz para transcribir |
| `MediaMuxer` | Unir los `.m4a` de una conversación por turnos sin recodificar |
| `MediaMetadataRetriever` | Ver si un audio es música (ARTIST/ALBUM/GENRE): si lo es, no se transcribe, se marca «letra» |
| `SpeechRecognizer` + `ParcelFileDescriptor.createPipe()` | El motor Google, dándole un fichero por una tubería |
| SAF (`ActivityResultContracts.OpenDocument`) | Adjuntar. **No hay `READ_EXTERNAL_STORAGE`**: todo entra por SAF y se copia a `filesDir` |
| `ContentResolver` (`openInputStream`, `getType`, `query` con `OpenableColumns`) | Leer lo compartido, clasificarlo y saber su nombre |
| `FileProvider` propio (`data.ProveedorDeArchivos`, authority `${applicationId}.fileprovider`) | Compartir y abrir hacia fuera. Es propio porque el de serie deduce el tipo por extensión y no conoce `svg` |
| `ACTION_SEND` / `ACTION_SEND_MULTIPLE` (dos Activities exportadas) | Ser destino del menú de compartir del sistema, con dos entradas distintas: «poner un pin» y «guardar aquí» |
| `ACTION_VIEW` con `pathPattern=".*\\.pixpin"` | Abrir un paquete de proyecto tocándolo en cualquier gestor de ficheros |
| `PdfDocument`, `LruCache`, `ClipboardManager`, `HapticFeedback` | Compartir como PDF, cachear páginas anotadas, copiar, vibrar |

**Lo que no usa, y sorprende:** el módulo **no declara ni arranca ningún servicio** y
**no publica ninguna notificación**. La transcripción va en `Thread {}` crudos y en
`Dispatchers.IO`; el aviso al usuario es un `Toast` más el `StateFlow` de avances.
Consecuencia real: una transcripción larga **no sobrevive** a que el sistema mate el
proceso.

### 2.5 Valoración

**Se porta casi entero, y es lo más valioso del inventario.** Es un cuaderno personal
sobre ficheros locales; nada de lo que hace es de teléfono.

| Qué | Cómo queda en Windows |
|---|---|
| El modelo `Mensaje` y `Clase` | Directo. `serde` con `#[serde(default)]` reproduce `encodeDefaults` + `ignoreUnknownKeys` |
| `guardados.jsonl` | **Se lee tal cual.** JSON Lines es JSON Lines. El escritor de escritorio puede usar el mismo fichero y el mismo temporal-más-rename |
| Toda la lógica pura (agrupación, días, resaltado, enlaces, onda, conversaciones) | Se traduce línea a línea a Rust. Son ~1.100 líneas sin Android y con tests ya escritos que sirven de especificación |
| Transcripción | Se porta con Vosk o sherpa-onnx (§1.1). El decodificador de audio hay que rehacerlo con `symphonia` |
| Reproductor y onda | Rehacer. `MediaPlayer` no existe; en Windows es `IMFMediaEngine`, o `rodio`/`symphonia` en Rust. La onda ya está guardada en `Mensaje.picos`, así que solo hay que pintarla |
| Grabación | Rehacer. `MediaRecorder` → WASAPI (o `cpal`), y elegir contenedor: Windows no trae codificador AAC tan a mano; podría guardarse en Opus u Ogg |
| Adjuntar, compartir, abrir | **Se simplifica mucho.** SAF, `FileProvider`, `ContentResolver` y `ACTION_SEND` son andamiaje de Android para algo que en Windows es abrir un diálogo de fichero, copiarlo y llamar a `ShellExecute` |
| Ser destino de «compartir» | No tiene equivalente. En Windows lo sustituyen: arrastrar y soltar sobre la ventana, pegar del portapapeles, y asociar la extensión `.pixpin` |
| `ConversacionActivity` (unir `.m4a` sin recodificar) | Rehacer. `MediaMuxer` no existe; es remultiplexado, que en Rust hay que montarlo o delegar en FFmpeg — y este proyecto ha decidido no llevar dependencias nativas de terceros |
| Teleprónter, pronunciar, letra, biblioteca | Se portan; son interfaz sobre el reproductor y el almacén |
| Que no haya servicio para la transcripción | **Arreglarlo al portar.** En Windows un hilo de trabajo con estado persistido es trivial y no hay nada que lo mate |

Lo que hay que **rehacer por diseño**, no por plataforma: `MensajesActivity.kt` con 5.367
líneas es un fichero que no debería repetirse. La pantalla tiene ocho o diez subsistemas
casi independientes (burbujas, búsqueda, selección múltiple, hilos, adjuntar, grabar,
compartir, fijados) que en el puerto conviene separar desde el principio.

---

## 3. `pin` — los pines flotantes (8.140 líneas, 18 ficheros)

### 3.1 Qué hace

Ventanas que se quedan encima de todo y no dependen de ninguna aplicación. Cada pin es
**una ventana independiente**: moverlo no recompone los demás.

Todo se maneja por gestos, sin menús: arrastrar mueve (soltarlo sobre la bola lo
minimiza a burbuja), pellizcar escala, dos dedos arriba/abajo cambia la opacidad, doble
toque minimiza, un toque copia o abre, y una pulsación larga saca una barra de cuatro
acciones.

**Trece tipos de pin** (`PinType`): `IMAGE, TEXT, COLOR, FILE, TIMER, CHECKLIST,
COUNTER, LEDGER, TABLE, CROQUIS, DRAW, RULETA, VOZ`. `CROQUIS` es histórico —el editor
se retiró— pero el valor sigue en el enum a propósito, porque los JSON guardados llevan
el literal `"CROQUIS"` y quitarlo haría fallar la deserialización de la lista entera; se
filtra al leer. La «pizarra» no es un tipo: es un `IMAGE` con fondo liso generado.

Además: grupos (los del mismo grupo se mueven, ocultan y cierran juntos, con borde de
color por hash del id), historial de cerrados, pines «guardados» que sobreviven al
cierre, ocultar-todo, modo atravesable, restauración al reiniciar el teléfono, y
recordatorios con hora.

### 3.2 Sus piezas

| Fichero | Líneas | Qué es |
|---|---:|---|
| `PinWindowController.kt` | 4.572 | Un pin. Ventana, gestos, modos de edición/anotación/ventana, visor de PDF, pizarra, mini-apps, voz, export |
| `OverlayManager.kt` | 1.425 | Todos los pines. Fábrica, visibilidad, historial, grupos, lista de pines, persistencia, recordatorios, buzón |
| `OverlayTouch.kt` | 366 | `View.OnTouchListener` con modos `NONE/DRAG/SCALE/OPACITY/RESIZE/SCROLL` |
| `MiniApps.kt` | 342 | Cuerpos y lógica de los pines-herramienta (`Ledger`, temporizador, casillas, tabla) |
| `PinModels.kt` | 213 | `PinType`, `PinState`, `WidgetState`, `esperaDelReloj` |
| `Voz.kt` | 180 | Grabar y medir picos |
| `ImageStore.kt` | 174 | Imágenes de pin, pizarras generadas |
| `GrabadoraActivity.kt` | 168 | La pantalla de grabar |
| `OverlayWindowFactory.kt` | 131 | `OverlayComposeWindow`: Compose dentro de una ventana sin Activity |
| `PinZoom.kt` | 112 | La matemática del pellizco |
| `FileStore.kt` | 86 | Ficheros pineados |
| `PinGroups.kt` | 82 | Lógica pura de grupos |
| `Recordatorios.kt` | 68 | `AlarmManager` |
| `PinChrome.kt` | 57 | Cuánto mide la ventana de más que el recuadro visible (sombra, pegatina) |
| `HoraDelRecordatorioActivity.kt` | 53 | El selector de hora del sistema |
| `RecordatorioReceiver.kt` | 49 | Lo que despierta al pin |
| `TextBoxSize.kt` | 32 | Límites del cuadro de texto |
| `BootReceiver.kt` | 30 | Relanzar el servicio tras reiniciar |

`PinState` completo, que es lo que se persiste:

```kotlin
@Serializable data class PinState(
    val id: String, val type: PinType,
    val text: String? = null, val imagePath: String? = null, val colorArgb: Int? = null,
    val filePath: String? = null, val fileName: String? = null, val mimeType: String? = null,
    val x: Int = 100, val y: Int = 200, val scale: Float = 1f, val alpha: Float = 1f,
    val clickThrough: Boolean = false, val minimized: Boolean = false,
    val groupId: String? = null, val isPinned: Boolean = false,
    val textBoxWidth: Int = 330, val textBoxHeight: Int? = null,
    val priority: Boolean = false, val emoji: String? = null,
    val ventana: Boolean = false,                       // el pin-imagen como marco
    val encuadreX: Float = 0f, val encuadreY: Float = 0f,
    val encuadreZoom: Float = 1f, val encuadreGiro: Float = 0f,
    val audioPath: String? = null, val recordarA: Long? = null,
    val ventanaAncho: Int? = null, val ventanaAlto: Int? = null,
    val widget: WidgetState = WidgetState(),
    val croquisPath: String? = null, val drawPath: String? = null
)
```

`WidgetState` guarda el estado de las mini-apps con un campo por herramienta —y no una
jerarquía sellada— para evitar serialización polimórfica y migraciones. Guarda
**instantes absolutos** (`timerEndsAt`, `runningSince`) y no «lo que queda», para que la
cuenta siga siendo correcta aunque el pin se cierre o el móvil se reinicie.

### 3.3 Cómo guarda los datos

`data/PinRepository.kt`, JSON con kotlinx.serialization:

| Fichero | Qué lleva |
|---|---|
| `filesDir/pins/pins.json` | Los pines vivos: `List<PinState>` |
| `filesDir/pins/history.json` | Los cerrados |
| `filesDir/pins/saved.json` | Los marcados como guardados (`isPinned`) |
| `filesDir/pins/clip_<millis>.png` · `board_<millis>.png` | Imágenes importadas y pizarras generadas. **PNG al 100 %** |
| `filesDir/pins/files/<millis>_<base saneada>.<ext>` | Ficheros pineados, con la extensión original preservada |
| `filesDir/pins/draw/<id>.excalidraw.gz` | El lienzo del pin, **gzip** |
| `filesDir/pins/draw/files/<id>` | Las fotos incrustadas en ese lienzo |
| `filesDir/pins/croquis/` | Legado del editor retirado; no se borra |

Todas las escrituras van con temporal + `rename`; en `ExcalidrawStore` está razonado:
«en esta aplicación el proceso muere sin avisar más de lo normal, y un archivo a medias
no abre». `OverlayManager` agrupa las escrituras con un *debounce* de 800 ms.

Un riesgo conocido y comentado en el propio código: `read()` hace
`getOrDefault(emptyList())`, así que **si el JSON se corrompe se pierden todos los pines
en silencio**. Al portar conviene no repetirlo.

### 3.4 De qué depende de Android

| Cosa | Para qué |
|---|---|
| `SYSTEM_ALERT_WINDOW` + `TYPE_APPLICATION_OVERLAY` | El requisito de todo el módulo: poner ventanas encima de otras aplicaciones |
| `FLAG_LAYOUT_NO_LIMITS` + `FLAG_LAYOUT_IN_SCREEN` | Que un pin pueda cruzar el borde de la pantalla y tapar la barra de estado |
| `FLAG_NOT_TOUCH_MODAL` | Que lo de debajo siga recibiendo toques fuera del recuadro del pin |
| `FLAG_NOT_FOCUSABLE` | Por defecto sí, para no robar el teclado. **Se quita al editar texto**: sin foco no hay teclado |
| `FLAG_NOT_TOUCHABLE` | El modo atravesable. Obliga a una ventana-«tirador» de 32 dp aparte, porque un pin atravesable **no recibe ni un toque** y no habría forma de salir |
| Servicio en primer plano tipo `specialUse` (`floating/PinHostService`) | Mantener vivos bola y pines. Es `specialUse` **a propósito**: Android 15 prohíbe arrancar un servicio de tipo `mediaProjection` desde el arranque del teléfono, y este sí puede |
| `POST_NOTIFICATIONS` | La notificación persistente de ese servicio, con botones «capturar» y «pinear» |
| `RECEIVE_BOOT_COMPLETED` | `BootReceiver`: tras reiniciar, si no hay informe de fallo pendiente y el permiso de overlay está, relanza el servicio |
| `AlarmManager.setAlarmClock` + `USE_EXACT_ALARM` | Los recordatorios. `setAlarmClock` es la única que Android respeta con el ahorro de batería puesto. `USE_EXACT_ALARM` y no `SCHEDULE_EXACT_ALARM` porque «esto **es** un despertador» |
| `TimePickerDialog` en Activity translúcida | Elegir la hora. Es Activity y no diálogo dentro del pin porque un pin es una ventana **sin foco** y un selector sin foco no recibe toques |
| `RECORD_AUDIO` + `MediaRecorder` en `GrabadoraActivity` | Grabar. Es Activity y no ventana flotante porque **el micrófono no se puede abrir desde un servicio que arrancó en segundo plano** (Android 12+) y pedir el permiso es cosa de una Activity |
| `androidx.lifecycle` / `savedstate` sintéticos | `OverlayComposeWindow` monta un `LifecycleOwner`, `ViewModelStoreOwner` y `SavedStateRegistryOwner` a mano, porque no hay Activity detrás |
| `taskAffinity` propio en cada Activity lanzada desde el overlay | Que lanzarla no arrastre toda la tarea de PixPin al frente y el usuario acabe capturando la propia aplicación |

El recordatorio **no lanza notificación**, y está razonado: «una notificación es una fila
más en una lista que uno ya ignora, y lo que se pidió fue *que me lo recuerdes*». El pin
vuelve a la pantalla y suena.

Una cosa que **no se ha podido confirmar**: `BootReceiver` relanza el servicio pero no
se ha encontrado código que **reprograme las alarmas** tras el reinicio a partir de los
`recordarA` guardados. Las `setAlarmClock` se pierden al reiniciar. Puede ser
intencional o un fallo; no está documentado ni en un sentido ni en otro.

### 3.5 Valoración

El crate `pixpin-pin` de Windows ya tiene 7.603 líneas, así que este módulo es sobre
todo **material de contraste**, no de porte.

| Qué | Cómo queda en Windows |
|---|---|
| El modelo `PinState` / `WidgetState` | **Se lee para inspirarse, no se copia.** Windows ya tiene su `PinGuardado` en `pixpin-store/src/almacen.rs`, con campos propios (`escala_por_cien`, `zoom_por_cien`, DPI del monitor). Los conceptos que faltan y merecen mirarse: `groupId`, `emoji`, `priority`, `recordarA`, el modo «ventana» con encuadre, y `WidgetState` entero |
| Los trece tipos de pin | Los de mini-app (TIMER, CHECKLIST, COUNTER, LEDGER, TABLE, RULETA) son ideas portables tal cual y baratas |
| `PinZoom`, `PinGroups`, `PinChrome`, `TextBoxSize`, `esperaDelReloj`, `Ledger` | Lógica pura con tests ya escritos (`PinZoomTest`, `PinGroupsTest`, `PinChromeTest`, `RelojDelPinTest`…). Es la mejor especificación disponible de esos comportamientos |
| Las ventanas overlay | **No se porta nada.** `TYPE_APPLICATION_OVERLAY` y sus flags no existen; en Windows es `WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TRANSPARENT` con DirectComposition, que es lo que `pixpin-shell` ya hace |
| El «tirador» del modo atravesable | **No hace falta.** Existe solo porque en Android un pin atravesable pierde todo contacto con el dedo. En Windows hay teclado global y barra de tareas: el modo atravesable se sale con un atajo |
| El servicio en primer plano, `BootReceiver`, el arranque tras reiniciar | **No tiene sentido.** En Windows es un proceso normal y una entrada en «inicio» |
| `AlarmManager` + `TimePickerDialog` | Rehacer, y es más fácil: un temporizador en el propio proceso, o el Programador de tareas si se quiere que funcione con la aplicación cerrada |
| `GrabadoraActivity` como Activity aparte | **No hace falta.** La razón —no se puede abrir el micrófono desde un servicio de fondo— es una regla de Android |
| Los `LifecycleOwner` sintéticos de `OverlayComposeWindow` | No aplica: es andamiaje para meter Compose donde Compose no cabe |
| El fichero de 4.572 líneas | No repetirlo. Los visores de PDF y de páginas, la pizarra y el modo anotación son subsistemas casi independientes que ya tienen crate propio en Windows (`pixpin-pdf`, `pixpin-motor2d`) |

---

## 4. `capture` — la captura de pantalla (2.435 líneas, 12 ficheros)

### 4.1 Qué hace

Toma un fotograma de la pantalla, deja elegir una región sobre la imagen congelada, deja
anotarla, y la fija como pin, la copia, la comparte o la guarda. Y hace **captura con
scroll**.

### 4.2 Sus piezas y cómo funciona

El orden es rígido, y está así por Android 14:

```
CaptureFlow.requestCapture
   ├─ ¿sin permiso de overlay? → aviso, fin
   ├─ ¿la sesión sigue viva? → capturar ya
   └─ si no → ConsentActivity → diálogo del sistema → CaptureService
                → startForeground(tipo mediaProjection) → ProjectionSession.start()
```

| Fichero | Líneas | Qué es |
|---|---:|---|
| `CaptureActivity.kt` | 830 | La pantalla de selección: región, lupa con cuentagotas, anotación, barra de 6 acciones |
| `ScrollCaptureController.kt` | 307 | El bucle de la captura larga |
| `Export.kt` | 292 | Guardar, copiar, compartir, y el «horneado» de las anotaciones |
| `ProjectionSession.kt` | 253 | El corazón: `MediaProjection` → `VirtualDisplay` → `ImageReader` → `Bitmap` |
| `CaptureService.kt` | 146 | El servicio en primer plano de tipo `mediaProjection` |
| `CaptureFlow.kt` | 128 | El único punto de entrada, venga de donde venga |
| `ScrollPlan.kt` | 125 | La **decisión** del cosido, sin tocar un píxel |
| `ScrollMatcher.kt` | 112 | La correlación de filas |
| `ScrollStitcher.kt` | 78 | Copiar píxeles y montar el resultado |
| `SelectionGeometry.kt` | 75 | Geometría pura del recorte |
| `ConsentActivity.kt` | 51 | El diálogo de consentimiento del sistema |
| `FrameHolder.kt` | 38 | Pasar un bitmap de 10 MB entre servicio y actividad sin meterlo en un `Intent` |

**Tres detalles técnicos que explican la forma del módulo**, todos documentados:

1. **Un token de `MediaProjection` sirve para UNA sola llamada a `createVirtualDisplay()`.**
   Pedir consentimiento y crear un display nuevo en cada captura falla a partir de la
   segunda. Por eso el `VirtualDisplay` se crea una vez y vive toda la sesión, y por eso
   existe un servicio en primer plano solo para mantener el token vivo.
2. **Un display espejo solo emite cuando la pantalla cambia.** Con la pantalla quieta,
   engancharse en el momento de capturar fallaba de forma intermitente. La solución es
   guardar siempre el último fotograma y un `grab()` en tres pasos: esperar un fotograma
   350 ms → coger el último si no llega nada → `virtualDisplay.resize()` para forzar
   repintado y esperar 2,5 s.
3. **El buffer de hardware tiene relleno de fila.** `Image.toBitmap` corrige
   `rowStride - pixelStride * width` creando el bitmap con ancho acolchado y recortando
   después.

**La captura con scroll no usa accesibilidad.** No hay ningún `AccessibilityService`. El
usuario **desplaza a mano** y PixPin va cosiendo:

- `ScrollMatcher` calcula una **firma por fila**: la luminancia entera de píxeles
  muestreados (`(r*77 + g*151 + b*28) shr 8`, que es 0,299/0,587/0,114 en escala de 256)
  sumada a lo largo de la fila. Es una correlación **1-D**, no de imagen entera.
- `findOffset` compara la cola contra todas las posiciones por suma de diferencias
  absolutas, y acepta solo si además: la media del error está bajo tolerancia, existe una
  segunda mejor opción a más de 4 filas, y esa segunda es al menos el doble de mala más
  un margen aditivo. Ese margen es lo que salva de los patrones repetidos (una cabecera
  cada N filas encaja perfecto en varios sitios; sin margen, «el doble de malo» se
  cumpliría con dos ceros). El KDoc lo resume: «lo importante no es acertar siempre, sino
  no acertar por casualidad».
- Si la cola es plana (`isFlat`), no sirve de referencia y se duplica su tamaño hasta 384
  filas.
- La barrita de progreso es su propia ventana overlay, y la región capturada se recorta
  por arriba para que la barra no salga repetida en cada tramo.

**La salida** (`Export.kt`): a la galería con **MediaStore** (no SAF), en
`Pictures/PixPin`, con nombre `PixPin_yyyyMMdd_HHmmss.<ext>` y formato elegido por el
usuario (PNG por defecto, o WEBP; JPEG si viene de ahí). Al portapapeles con
`ClipData.newUri`. Al compartir, un temporal en `cacheDir/share/` expuesto por el
`FileProvider` propio. Y ya **no hay botón de guardar**: si has hecho algo con la
captura, es que la querías, así que se guarda sola.

### 4.3 Valoración

`pixpin-capture` (1.620 líneas) y `apps/pixpin/src/scroll.rs` ya existen en Windows, y el
crate `pixpin-codec` ya tiene un `Cosedor`. Esto es contraste, no porte.

| Qué | Cómo queda en Windows |
|---|---|
| `MediaProjection`, `VirtualDisplay`, `ImageReader`, el consentimiento, el servicio, `START_NOT_STICKY`, `FrameHolder` | **Nada de esto existe ni hace falta.** En Windows es Desktop Duplication o `Windows.Graphics.Capture`, sin diálogo de permiso, sin token que caduque y sin servicio |
| La corrección de `rowStride` | Sí aplica, con otro nombre: los surfaces de D3D11 también tienen `RowPitch` |
| `ScrollMatcher` + `ScrollPlan` | **Portar la matemática tal cual.** Las firmas de fila y los tres filtros de confianza son independientes de la plataforma y tienen tests (`ScrollMatcherTest`, `ScrollStitcherTest`). Merece compararlos con lo que ya hace `pixpin_codec::cosido` |
| El bucle de scroll | **Se hace al revés.** En Android el usuario desplaza a mano porque no hay forma de hacerlo por él. En Windows sí la hay, y `apps/pixpin/src/scroll.rs` ya la usa: mandar la rueda a la ventana de debajo. Es mejor y no hay que copiar el bucle manual |
| `SelectionGeometry` | Geometría pura y portable, con test. La lección más útil: **el ancla de la esquina se calcula una sola vez al empezar el arrastre**; recalcularla en cada evento hace que persiga al dedo y la selección solo pueda crecer hacia abajo y a la derecha |
| `Export` a MediaStore | Rehacer: en Windows es escribir en `Imágenes\PixPin`, mucho más simple |
| El «horneado» (`horneaCaptura`) | La lección es de orden: **pintar la escena sobre la imagen entera y recortar después**, porque el mosaico saca sus píxeles del fondo en coordenadas de escena y sobre un bitmap ya recortado leería desplazado |
| No tener botón de guardar | Decisión de producto, no de plataforma. Vale igual en Windows |

---

## 5. Los módulos pequeños

### 5.1 `ui` (4.097 líneas, 7 ficheros)

**Qué hace.** `Proyectos.kt` (2.182) es la pantalla de proyectos que describe
`formato-pixpin.md`: un `VerticalPager` con una página por proyecto, y dentro las hojas,
que pueden ser página de PDF, lienzo, nota Markdown o croquis 3D. `abrirHoja` es el
despachador que decide qué editor abrir. Es también el **productor** del `.pixpin`: lo
escribe en `cacheDir/share/<nombre>.pixpin` y lo comparte con MIME `application/zip`.
Exporta además a PDF y a página web. Un arrastre lateral abre la conversación de ese
proyecto.

`MarkdownEditorActivity.kt` (915) es el editor de notas, montado sobre el motor
`motormd`. `ZoomDeHoja.kt` es el gesto de dos dedos para «sacar» una hoja, con `Popup`
para escapar del recorte de la lista. `EditorDeBarra.kt` deja al usuario recolocar su
propia barra de herramientas arrastrando. `theme/AOscuras.kt` decide si es de noche de
verdad con el **sensor de luz** (histéresis de 10 a 40 lux) y la hora.

**Persistencia.** Ninguna propia: usa `ProyectosRepository` y el DataStore. Ojo:
`motormd/TextoStore` es un almacén **en memoria**, no en disco.

**De Android.** `PdfRenderer` (indirecto), `Intent`/`FileProvider` para compartir,
`SensorManager` con `TYPE_LIGHT` para el modo noche automático, `ClipboardManager`.

**Valoración.** El modelo de proyecto/hoja se porta directo y es lo que le falta a
Windows para leer un `.pixpin`. La pantalla en sí hay que rehacerla: un `VerticalPager`
a pantalla completa es una idea de móvil; en escritorio son ventanas y paneles. El
sensor de luz no existe en un PC de sobremesa: el modo noche va por el ajuste del
sistema. `ZoomDeHoja` no se porta —es un gesto táctil—; su equivalente es la rueda.

### 5.2 `mini` (2.031 líneas, 7 ficheros)

**Qué hace.** Mensajes de la conversación que *hacen algo*: lista de tareas, gastos,
cronómetro, temporizador, alarma, contador, ruleta. La burbuja muestra un resumen («3 de
7», «60,50 €»).

**Cómo guarda los datos.** Esta es la decisión más interesante del módulo y está
razonada: **el documento es Markdown dentro de `Mensaje.texto`**, no un fichero ni una
tabla aparte. Las tareas son casillas `- [ ]` / `- [x]`; los gastos, una tabla Markdown
`| Concepto | Importe (EUR) |` con la moneda en la cabecera; el contador, `- valor: N`.
El motivo: así se busca con el buscador de la conversación, se copia y se pega en
cualquier sitio, y sobrevive a que la aplicación cambie por dentro.

Cronómetro y temporizador guardan **instantes**, no números, por lo mismo que
`WidgetState`.

**De Android.** Solo `MiniActivity.kt` toca la plataforma: guarda a cada cambio (sin
botón de guardar) y programa avisos con `AlarmManager.setAlarmClock` a través de
`pin/Recordatorios`, con la clave prefijada `"mini:"`.

**Valoración.** Se porta entero y es barato. La lógica (`Tareas`, `Gastos`, `Tiempos`,
`Contador`, `Ruleta`) son ~1.000 líneas puras con tests. Lo único a rehacer son las
alarmas. La idea de guardar el estado como Markdown legible merece conservarse tal cual.

*Aviso de nomenclatura:* hay **dos enums distintos llamados `MiniApp`**:
`mini.MiniApp` (las del chat, con `id` persistido) y `clipboard.MiniApp` (las palabras
mágicas). No son el mismo tipo. Al portar conviene renombrar uno.

### 5.3 `data` (1.058 líneas, 5 ficheros)

**Qué hace.** Los repositorios y los ajustes.

**Cómo guarda los datos.** Es el mapa completo de la persistencia del proyecto:

| Fichero | Mecanismo | Dónde |
|---|---|---|
| `SettingsRepository.kt` (591) | **DataStore Preferences** | `datastore/pixpin_settings.preferences_pb` |
| `ProyectosRepository.kt` (203) | JSON | `filesDir/proyectos/proyectos.json`; PDF limpio en `filesDir/proyectos/limpio-<millis>.pdf` |
| `PinRepository.kt` (74) | JSON | `filesDir/pins/{pins,history,saved}.json` |
| `CrashLog.kt` (135) | Texto + SharedPreferences | `filesDir/crash.txt`; prefs `"crashlog"`, clave `ultima_muerte` |
| `ProveedorDeArchivos.kt` (59) | — | `FileProvider` con authority `${applicationId}.fileprovider` |

Los ajustes son ~44 claves. Algunas representativas: `default_pin_alpha`, `history_size`,
`ball_x`/`ball_y`, `capture_mode`, `copy_format`, `modo_noche`, `oled_negro`, `zurdo`,
`motor_de_voz`, `idioma_de_voz`, `segundo_idioma_de_voz`, `modelo_whisper`,
`modo_de_idiomas`, `palabras_magicas`, `pin_tools`/`capa_tools`/`editor_tools`
(conjuntos), `iman_*` (los imanes del editor).

Una convención que hay que respetar al portar: **`null` significa «no lo he tocado»** (y
mandan los valores de fábrica), mientras que un conjunto **vacío** significa «ninguna».
De ahí que los `reset*()` hagan `remove(key)` y no escriban un valor.

`CrashLog` hace algo que merece copiarse: además de enganchar
`Thread.setDefaultUncaughtExceptionHandler`, recoge las muertes que la aplicación no vio
(`ActivityManager.getHistoricalProcessExitReasons`, API 30+) para pillar crashes nativos,
ANR y OOM. Y sirve de **modo seguro**: si hay un informe pendiente, `OverlayManager` no
restaura nada al arrancar, para que la aplicación no quede imposible de abrir.

Ids con marca de tiempo en todas partes: `pr-<millis>` para proyecto, `h-<millis>-<n>`
para hoja, `c3d-<millis>` para croquis, `<id>-<ms>` como sufijo al importar un `.pixpin`.

**Valoración.** El esquema es directamente portable, y de hecho Windows ya hace lo mismo
por su cuenta: `pixpin-store` tiene `pixpinmax.toml` para ajustes e `indice.json` con
temporal + rename para el almacén. La equivalencia es limpia: DataStore → TOML,
`*.json` → `serde_json`, SharedPreferences → nada (es un caso suelto), `FileProvider` →
no existe ni hace falta. `CrashLog` y el modo seguro se portan y son buena idea.

### 5.4 `floating` (786 líneas, 4 ficheros)

**Qué hace.** La bola flotante —el orbe arrastrable que es el único disparador siempre a
mano—, el servicio que aloja todo, y el botón de ajustes rápidos.

- `FloatingBallController.kt` (472): ventana overlay de 48 dp con arrastre y ajuste al
  borde, menú con velo a pantalla completa, y cinco acciones: capturar, capa, pinear
  portapapeles, ocultar todo, lista de pines. Guarda su posición en `ball_x`/`ball_y`.
- `BallState.kt` (142): la lógica pura, sin Android y con test (`BallStateTest`):
  recuperación, retracción a medias en el borde (`MINIMO_VISIBLE = 0.25`), recolocación
  al girar la pantalla.
- `PinHostService.kt` (134): el servicio en primer plano de tipo `specialUse`, canal
  `"pixpin_ambient"`, `START_STICKY`, notificación persistente con dos botones.
- `CaptureTileService.kt` (38): un **botón de ajustes rápidos** (`TileService`) que
  captura al tocarlo.

**Valoración.** La bola existe porque Android **no tiene atajos de teclado globales**;
el propio documento de diseño lo dice en su tabla de adaptaciones: «Hotkeys globales
(`Ctrl+1`, `Ctrl+2`) → bola flotante, tile, notificación persistente». En Windows la
solución original vuelve a estar disponible, así que **la bola, el tile y la notificación
persistente no tienen sentido**. El servicio en primer plano tampoco: es un proceso
normal. Lo único que sobrevive es la idea de un menú rápido, que en escritorio es un
icono en la bandeja del sistema.

### 5.5 `clipboard` (567 líneas, 6 ficheros)

**Qué hace.** Convierte lo que hay en el portapapeles —o lo que llega por el menú de
compartir— en un pin del tipo que toque.

- `ClipboardPinActivity.kt` (43): una actividad translúcida y sin historial cuyo único
  fin es **dar foco a la aplicación**, porque desde Android 10 solo se puede leer el
  portapapeles con la ventana enfocada. La lectura ocurre en `onWindowFocusChanged`, no
  en `onCreate`.
- `ContentClassifier.kt` (106): puro. `PinContent` es `ColorPin` / `TextPin` /
  `MiniAppPin` / `TablePin` / `ImageUri` / `FileUri` / `Empty`. El orden importa: palabra
  mágica → color CSS (`#hex` de 3, 6 u 8, `rgb()`, `rgba()`, `r, g, b`, nombres CSS) →
  tabla → texto.
- `MagicWord.kt` (188): las **palabras mágicas**. Copiar la palabra «timer» —y nada más—
  y tocar la bola abre un temporizador en vez de crear un pin de texto. La regla de «tiene
  que ir sola» evita robarle a nadie el copiar y pegar de una palabra corriente; por eso
  se descartaron «nota» y «lista». Vienen unas por defecto («pomodoro», «crono»,
  «compras», «gastos», «contador»…) y el usuario las cambia; se guardan en la clave
  `palabras_magicas`.
- `TableData.kt` (104): detecta tablas pegadas de una hoja de cálculo probando tres
  separadores por orden de fiabilidad (tabulador, `|` de Markdown, dos o más espacios),
  exigiendo al menos 2 filas y que la mayoría coincida en número de columnas.
- `ShareReceiverActivity.kt` (78) y `ClipboardPinReader.kt` (48): entrada desde fuera.

**Valoración.** `ContentClassifier`, `MagicWord` y `TableData` son ~400 líneas puras con
tests (`ContentClassifierTest`) y **se portan tal cual**; son buenas ideas y no dependen
de nada. Lo que desaparece es todo el andamiaje: en Windows no hace falta una actividad
fantasma para poder leer el portapapeles, ni `ShareReceiverActivity` (su equivalente es
arrastrar y soltar).

### 5.6 `capa` (533 líneas) y `annotate` (181 líneas)

**`capa/CapaPantalla.kt`** es dibujar **encima de la pantalla viva**: una capa
transparente sobre lo que haya —otra aplicación, un vídeo, una presentación— donde se
dibuja mientras lo de debajo sigue funcionando. Su decisión central son **dos ventanas,
no una**: el lienzo, que recoge los toques mientras se dibuja y pasa a
`FLAG_NOT_TOUCHABLE` cuando se quiere que los clics lleguen abajo; y la barra, en ventana
aparte, porque dentro del lienzo se volvería intocable al atravesar. `copiar()` esconde
los overlays, espera 64 ms a que el compositor deje de dibujarlos, toma el fotograma y
hornea el dibujo encima.

**`annotate/StrokeTouchReader.kt`** son dos cosas: el **rechazo de palma** (`PalmGuard`:
mientras el lápiz esté en uso, o lo haya estado hace menos de 1,5 s, los toques de dedo
se ignoran) y un lector de trazos que lee `MotionEvent` en crudo. No usa gestos de
Compose por tres razones documentadas: `detectDragGestures` no emite nada hasta pasar el
umbral de arrastre, con lo que se perdía el arranque del trazo y los trazos cortos no
existían; un digitalizador muestrea a cientos de Hz y esas muestras viajan como
**históricos** dentro del evento (`getHistoricalX/Y/Pressure`), que Compose descartaba; y
las ventanas overlay interceptan los toques antes de que lleguen a Compose. Tiene además
un detalle bonito: **el segundo dedo hace de Shift** (círculo redondo, cuadrado
cuadrado).

**Valoración.** La capa **ya está portada**: `apps/pixpin/src/capa.rs` existe y su
comentario de cabecera describe exactamente el mismo problema y la misma solución
(alternar entre recoger el ratón y dejarlo pasar). Lo que aporta el Android es la lección
de las dos ventanas, que en Windows se resuelve distinto porque `WS_EX_TRANSPARENT` se
puede activar y desactivar sobre la misma ventana.

`StrokeTouchReader` **no se porta**: es un lector de eventos táctiles. En Windows su
equivalente es Windows Ink (`WM_POINTER`), y el problema que resuelve —muestras
históricas de alta frecuencia— existe igual: `GetPointerFrameInfo` devuelve varias
muestras por evento, y descartarlas empeora el trazo del mismo modo. El rechazo de palma
sí tiene sentido en una tableta con lápiz. Lo del segundo dedo como Shift, en escritorio
es la tecla Shift de verdad.

---

## 6. Resumen: qué se porta, qué se rehace, qué se tira

| Se porta casi tal cual | Hay que rehacerlo | No tiene sentido en escritorio |
|---|---|---|
| El formato `.pixpin` (ZIP + JSON) | El audio: grabar, reproducir, decodificar a PCM | La bola flotante, el tile de ajustes rápidos, la notificación persistente |
| `guardados.jsonl` y el modelo `Mensaje` | La captura: `MediaProjection` → Desktop Duplication | El servicio en primer plano y el arranque tras reiniciar |
| Toda la lógica pura con test (~3.000 líneas repartidas) | Los recordatorios: `AlarmManager` → temporizador propio | El consentimiento de captura y el token que caduca |
| La transcripción, con Vosk o sherpa-onnx | La salida a galería: MediaStore → `Imágenes\PixPin` | SAF, `FileProvider`, `ContentResolver`, `ACTION_SEND` |
| Las mini-aplicaciones y su Markdown legible | El bucle de scroll: a mano → mandar la rueda | El «tirador» del modo atravesable |
| Las palabras mágicas y el clasificador de portapapeles | Las pantallas grandes (`MensajesActivity`, `Proyectos`) | La actividad fantasma para leer el portapapeles |
| El modelo proyecto/hoja | El almacenamiento: `filesDir` → `%LOCALAPPDATA%` | `StrokeTouchReader` tal cual (pero sí su lección) |
| `CrashLog` y el modo seguro | | El sensor de luz para el modo noche |

**Lo que de verdad falta en Windows es `guardados`.** Los pines, la captura, la capa y el
scroll ya existen del lado de Rust, y este inventario sirve sobre todo para contrastar
detalles. El cuaderno no existe: son 10.865 líneas de las que la mitad larga —el modelo,
el almacén, la agrupación, la transcripción, el audio, la unión a proyectos— es
independiente de Android y se traduce; y la otra mitad es una pantalla de 5.367 líneas
que conviene no repetir.

## 7. Lo que no se ha podido comprobar

- **Si las alarmas de recordatorio se reprograman tras reiniciar el teléfono.**
  `BootReceiver` relanza el servicio; no se ha encontrado código que recorra `pins.json`
  y vuelva a llamar a `Recordatorios.poner` con los `recordarA` guardados. Puede ser
  intencional; no está documentado.
- **Nada se ha verificado en un dispositivo.** Todas las rutas y nombres de fichero
  salen del código.
- **No se han leído línea a línea** `MensajesActivity.kt` (5.367), `PinWindowController.kt`
  (4.572) ni `Proyectos.kt` (2.182). De los tres se ha leído la estructura completa, el
  KDoc y los bloques de persistencia, gestos y export.
- **`motor/` (55.385 líneas) y `croquis3d/` (21.024) están fuera de este encargo** y solo
  se han mirado en lo que otros módulos dependen de ellos. Son, con diferencia, la mayor
  parte del proyecto Android, y merecen su propio informe.
- **La discrepancia documentación/código está confirmada en un punto**: el diseño de
  2026-07-26 dice Room; el código usa JSON y JSONL, con la razón escrita. En el formato
  `.pixpin` no hay discrepancia: documento y código coinciden.
