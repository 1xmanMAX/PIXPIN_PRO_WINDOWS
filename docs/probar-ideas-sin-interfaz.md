# Probar las ideas de Jev sin interfaz (29-oct-2026)

El usuario: «hacer programas pequeños sin interfaz para comprobar si **todas**
las ideas funcionarían, y luego implementar la misma lógica en la app».

Son **ejemplos de Cargo** de `crates/pixpin-typesafe` (no un paquete nuevo:
`apps/pixpin/tests/capas.rs` cuenta los paquetes). Corren en Linux, Mac y
Windows, sin ventana: leen un guion de reunión inventado, lo pasan frase a
frase en tiempo real simulado y escriben lo que Jev juzgó y lo que el código
decidió en cada paso, lo que tardó cada petición, los tokens y lo que cuesta.
Si el guion trae lo que se espera (`# espera: …`), lo comprueban y salen con
error si algo no pasa: cada guion es una **prueba de evaluación** contra la
API de verdad.

**La lógica no está en los ejemplos**: está en la biblioteca, la misma que
usará la app (ver «Dónde está la lógica»). Los ejemplos solo son el banco de
pruebas (`examples/comun/`). Llevar una idea a la app es cablear, no
reescribir.

## Cómo se corren

```sh
# De verdad (la clave en TYPESAFE_API_KEY o en un fichero, TYPESAFE_API_KEY_FILE):
TYPESAFE_API_KEY=… cargo run -q -p pixpin-typesafe --example temas_en_vivo -- coordinacion.txt --rapido

# Sin clave: escribe el JSON exacto de cada petición y contesta «no» a todo.
cargo run -q -p pixpin-typesafe --example tareas_dichas -- tareas.txt --rapido --en-seco

# Sin clave, con las respuestas de verdad grabadas el 29-oct-2026 (jev-1.14.0):
cargo run -q -p pixpin-typesafe --example tareas_dichas -- tareas.txt --rapido \
    --grabadas grabadas/tareas_dichas.tareas.jsonl

# De verdad y grabando para repetirlo luego sin clave:
TYPESAFE_API_KEY=… cargo run -q -p pixpin-typesafe --example decisiones -- coordinacion.txt --rapido \
    --grabar /tmp/decisiones.jsonl
```

- El guion se busca tal cual o en `crates/pixpin-typesafe/examples/guiones/`
  (basta el nombre); lo mismo `--grabadas`.
- `--rapido`: no espera entre frases (el reloj de la reunión avanza solo con
  lo que tarda cada petición). Sin él, las frases llegan a su hora de verdad.
- `--ver`: de verdad, y además escribe cada petición exacta.
- `--tiempo S`: lo que se espera por petición (10 s; la app usa 2-4 s).
- `--latencia MS`: en seco o con grabadas, la latencia simulada (300 ms).
- Al final: `RESUMEN <ejemplo> esperas=a/b peticiones=n errores=e tokens=t
  p50_ms=… p95_ms=…` (una línea, para juntar resultados).

**La clave**: en Linux sale por `curl`, con las cabeceras por la entrada
estándar (`-H @-`): nunca en la línea de órdenes (`ps`), ni en el registro,
ni en lo grabado (que solo guarda la huella del cuerpo, las respuestas y los
tokens). En Windows, por WinHTTP como la app. `curl` respeta `HTTPS_PROXY`.

**Lo grabado** (`--grabadas F`, JSONL, ver `Repetido` en `examples/comun/mod.rs`):

- `{"huella": …, "answers": …}` (lo que escribe `--grabar`): la respuesta a
  la petición con ese cuerpo exacto. Si se cambia una pregunta, la huella ya
  no casa: esa petición se contesta «no» a todo, se avisa
  `(sin grabar…)` y al final `quedan N respuestas grabadas sin usar`.
- `{"pide": "yo me encargo", "answers": {"compromiso": 0.92, "quien": "ana"}}`:
  escrito a mano, para la siguiente petición que contenga ese texto (un
  número es un Noul; un texto, la opción elegida de una Choice). Las demás,
  «no».
- `{"fallo": "sin red"}` o `{"http": 429}`: un fallo, para ver que sale
  como **error** y no como «no» (`grabadas/decisiones.fallos.jsonl`).

## Los prototipos

| ejemplo | qué comprueba | guiones | pregunta a Jev | decide el código |
|---|---|---|---|---|
| `temas_en_vivo` | los temas de la agenda se marcan solos mientras se habla; la hora del ✓ frente a la de la frase | `coordinacion`, `temas_prueba_ignorada` | 3 Nouls por tema pendiente sobre los últimos 40 s (`preguntas::temas`) | `temas`: `Rollo`, `Ritmo`, `Seguimiento` (lo de la app, tal cual) |
| `tareas_dichas` | «yo me encargo de mandar la cotización el viernes» → tarea {texto, responsable, fecha, prioridad} | `tareas`, `tareas_control`, `coordinacion` | ¿compromiso?, ¿dice quién?, ¿quién? (Choice con los participantes), ¿plazo?, ¿cuál de las fechas? (Choice), ¿prisa? | `fechas` (el día), `compromisos` (la tabla) |
| `urgencia_en_vivo` | «esto es para hoy» sube la prioridad de una tarea que ya existe | `coordinacion` (`# tarea:`) | al empezar, `urgencia` por tarea; luego ¿es de esta tarea?, ¿la apura?, ¿plazo? | `urgencia_viva` → la tabla de `urgencia` |
| `tema_olvidado` | queda poco y hay temas sin tratar; se cierra con temas pendientes | `tema_olvidado` | ¿es de la agenda?, ¿qué tema?, ¿se está cerrando? | `tiempo_de_temas`: reloj, tratado = 30 s, avisos |
| `tiempo_por_tema` | cada frase a un tema (o «otro») y los minutos de cada uno | `coordinacion` | ¿es de la agenda? (Noul), ¿qué tema? (Choice) | `tiempo_de_temas`: la duda sigue al tema anterior; cada frase cuenta hasta la siguiente (60 s como mucho) |
| `decisiones` | «se decidió X» frente a «podríamos X» | `coordinacion` | ¿anuncia una decisión? | `vivo::es_decision` (≥ 0.7) |
| `preguntas_pendientes` | preguntas que nadie contestó al final | `coordinacion` | ¿pregunta de verdad?, y por cada abierta, ¿esta frase la contesta? | `pendientes` |
| `fechas_al_horario` | «nos vemos el martes a las 3» → evento | `fechas`, `coordinacion` | ¿pone un momento a una cita o evento?, ¿cuál de las fechas? | `fechas`, `citas` |
| `repaso_oral` | qué puntos clave cubre, dice mal o faltan en la explicación de un alumno | `repaso` | por punto: ¿lo explica?, ¿lo contradice? | `repaso` |
| `dictado_a_chat` | «anota esto en el chat de la obra» → qué chat y qué texto | `dictado`, `dictado_control`, `coordinacion` | ¿pide anotar algo?, ¿nombra el chat?, ¿cuál? (Choice), ¿qué texto? (Choice entre los que saca el código) | `dictado` |
| `reunion_completa` | **todas** las de frase a frase en una sola petición por frase, con las esperas de todas | `coordinacion` | lo de las siete de arriba juntas (18-23 preguntas) | lo mismo |

### Qué va junto (una petición por frase)

Todo lo que mira **la frase, quién la dice y las dos de antes** comparte el
estado y va en **una sola petición** (`vivo`): tareas dichas, urgencia en
vivo, decisiones, preguntas pendientes, citas, dictado, tiempo por tema y
tema olvidado. Cada idea pone sus preguntas solo si está encendida
(`vivo::Partes`), y `participantes` y `agenda` solo van al estado si van las
preguntas que los nombran. `reunion_completa` lo comprueba: 30/30 esperas
con las ocho ideas juntas, igual que sueltas.

Van **aparte**: los temas en vivo (su estado es la ventana de 40 s y su
ritmo, una petición cada 1,5 s como mucho) y el repaso oral (otro estado: lo
explicado entero y los puntos de la clase).

### Las fechas, en código

Jev no calcula fechas. `fechas::buscar` encuentra en la frase todas las
expresiones de día u hora («el viernes», «mañana a las 9», «el martes a las
3», «el 15 de noviembre», «la semana que viene», «a fin de mes», «el lunes
pasado») y las resuelve contra el día de la reunión (`# fecha:`). Jev solo
elige cuál es el plazo o la cita («la cotización **del viernes**» nombra un
tema, no un plazo). Sin ninguna expresión no se pregunta por plazos ni
citas; con una sola, basta el Noul. Las reglas (dicho un viernes, «el
viernes» es el de dentro de una semana; «a las 3» sin más es por la tarde)
están en la cabecera de `fechas.rs`.

## Resultados con la API de verdad (29-oct-2026, jev-1.14.0, desde Linux)

Guiones inventados, sin datos de nadie. Latencia medida de punta a punta
(curl por el proxy de este contenedor). Antes = las preguntas como se
escribieron, sin probar; después = tras ajustar el texto en `preguntas.rs`.

| ejemplo · guion | esperas antes | después | peticiones | p50 / p95 ms | tokens de entrada |
|---|---|---|---|---|---|
| temas_en_vivo · coordinacion | 4/4 | 4/4 | 19 | 175 / 369 | 26 056 |
| temas_en_vivo · temas_prueba_ignorada | 4/4 | 4/4 | 11 | 176 / 242 | 18 493 |
| tareas_dichas · tareas | 9/10 | 10/10 | 11 | 172 / 219 | 9 908 |
| tareas_dichas · tareas_control (no usado para ajustar) | 6/7 | 7/7 | 8 | 170 / 217 | 6 867 |
| tareas_dichas · coordinacion | 5/5 | 5/5 | 19 | 189 / 216 | 16 391 |
| urgencia_en_vivo · coordinacion | 4/4 | 4/4 | 22 | 181 / 237 | 19 559 |
| tema_olvidado · tema_olvidado (no usado para ajustar) | 6/6 | 6/6 | 13 | 169 / 189 | 9 585 |
| tiempo_por_tema · coordinacion | 5/5 (+3 nuevas: 6/8) | 8/8 | 19 | 193 / 361 | 13 014 |
| decisiones · coordinacion | 5/5 | 5/5 | 19 | 172 / 206 | 8 656 |
| preguntas_pendientes · coordinacion | 4/4 | 4/4 | 19 | 169 / 228 | 9 846 |
| fechas_al_horario · fechas | 6/6 | 6/6 | 6 | 191 / 244 | 2 924 |
| fechas_al_horario · coordinacion | 2/2 | 2/2 | 8 | 173 / 205 | 3 885 |
| repaso_oral · repaso | 5/5 | 5/5 | 5 | 173 / 233 | 6 794 |
| dictado_a_chat · dictado | 5/6 | 6/6 | 7 | 164 / 218 | 4 609 |
| dictado_a_chat · dictado_control (no usado para ajustar) | 4/5 | 5/5 | 6 | 186 / 191 | 4 001 |
| dictado_a_chat · coordinacion | 2/2 | 2/2 | 19 | 177 / 203 | 12 796 |
| reunion_completa · coordinacion | 27/27 | 30/30 | 22 | 186 / 206 | 48 337 |

(La primera vuelta de `decisiones` y `reunion_completa` dio menos por dos
fallos del banco de pruebas, no de Jev: la hora delante del texto de la
decisión y las esperas de una idea comparadas con lo de otra. Arreglado;
repitiendo las respuestas grabadas de entonces salen 5/5 y 27/27.)

**Lo que se ajustó** (solo el texto de las preguntas o el estado, nunca los
umbrales):

1. `compromiso`: «hay que revisar los planos antes del 15» daba 0.39 (la
   pregunta decía «alguien se compromete»). Ahora pregunta por una
   **tarea** («action item»), con o sin persona; quién la hace lo dice otra
   pregunta. 0.39 → tarea de «nadie» el 15-nov.
2. `dictado`: «anótalo, que la grúa se va el viernes» no contaba (la
   pregunta decía «en un chat»). Ahora pregunta si pide **anotar** algo; el
   chat lo dice `dictado_chat_dicho`. La nota sale sin chat («¿qué chat?»).
3. `de_la_agenda`: «lo de los proyectores lo vemos al final» contaba como
   hablar del tema (0.99). El «no» de los criterios nombra ese caso: dejarlo
   para luego no es tratarlo. → 0.10.
4. `tema`: «de acuerdo, queda decidido: adelantamos el pedido al lunes» (por
   el paro) iba a «Compra de proyectores» (pedido ≈ compra). No se tocó la
   redacción: el estado lleva `tema_anterior`, el tema que el código ya
   sabe de la frase anterior. Va al paro; los cambios de tema («ahora, la
   excursión al museo») siguen saliendo a la primera.

**Latencia**: 160-200 ms la mediana, 190-370 ms el p95, con 1 a 23
preguntas por petición: juntar preguntas no la sube. En los temas en vivo,
el ✓ sale **0,2 s** después de acabar la frase que lo decide.

**Coste**: 0.042 $ por millón de tokens de entrada. Todo lo de esta tabla,
233 peticiones y ~222 000 tokens ≈ **0,009 $**; la sesión entera de
pruebas (cuatro vueltas, ~770 peticiones) ≈ 0,03 $. Una reunión de una hora
con todo encendido: ~2 200 tokens por frase en `reunion_completa`, unas 1 000
frases → ~2,2 M tokens ≈ **0,09 $**, más los temas en vivo (≤ 0,15 $,
`docs/jev.md`).

**Lo que falta probar** (antes de fiarse del todo): reuniones largas y
reales pasadas por Whisper (con sus errores y sin «Quién:»: los temas en
vivo ya van sin quién habla, las demás lo usan), más casos de control por
idea (estos guiones son cortos: no hay que ajustar a ellos), y ver si
`tema_anterior` arrastra un tema de más cuando el cambio no se anuncia.

## Dónde está la lógica

Todo en `crates/pixpin-typesafe/src/`, con sus pruebas (`cargo test -p
pixpin-typesafe`: cada tabla fila a fila y respuestas grabadas):

- `preguntas.rs` — **todo el texto** de las preguntas: `vivo` (las de frase
  a frase) y `repaso`, junto a las que ya había.
- `vivo.rs` — el estado de una frase, qué preguntas van según las partes
  encendidas (`Partes`), una petición, y leer las respuestas (`Senales`).
  Un fallo es un error de la frase entera, nunca un «no».
- `fechas.rs` — las expresiones de día y hora y su resolución.
- `compromisos.rs`, `urgencia_viva.rs`, `pendientes.rs`, `citas.rs`,
  `dictado.rs`, `tiempo_de_temas.rs`, `repaso.rs` — una tabla de decisión
  cada uno, en la cabecera del módulo.
- `temas.rs` y `urgencia.rs` — los de antes, sin cambios.

## Cómo se lleva a la app (el cableado)

Lo que ya existe: `pixpin-voz` `en_vivo` da las frases mientras se graba y
`pixpin_proyecto::temas` cablea los temas en vivo (ver `docs/jev.md` §5).
Para lo nuevo:

1. **Un hilo por reunión** junto al de los temas: por cada frase de
   `en_vivo` (con ≥ 3 palabras), `vivo::preguntar` con las partes que el
   usuario tenga encendidas (interruptores en `[jev]`, como los de ahora) y
   el contexto: título, participantes (si se conocen; si no, sin
   `quien`), la agenda, las dos frases anteriores y el día de la nota. Una
   en vuelo como mucho; lo que llega mientras tanto se junta en la
   siguiente (lo mismo que `temas::Ritmo`). Tiempo: 2-4 s.
2. **Lo decidido, como propuesta** (nunca hecho solo): tareas propuestas
   (`compromisos::Propuesta`) a la lista de tareas con un botón de aceptar;
   los cambios de prioridad (`urgencia_viva::Cambio`) como aviso sobre la
   tarea; las decisiones y las preguntas sin respuesta al acta de la nota
   (como `importantes`); los eventos (`citas::Evento`) al calendario con
   confirmar; las notas de dictado (`dictado::Nota`) al chat elegido, o
   preguntando cuál si `chat` es `None`; los avisos de tema olvidado
   (`tiempo_de_temas::Aviso`) como un aviso discreto; los minutos por tema
   en la página de la reunión.
3. **El repaso oral**, desde el modo estudio: los puntos clave salen de las
   partes importantes de la clase (`reunion`); lo que dice el alumno, de
   `en_vivo`; `repaso::preguntar` por frase.
4. **Sin clave o si Jev falla**, nada de esto aparece y todo sigue como
   antes (los fallos son errores, no «no»).
5. Antes de encender cada idea por defecto, correr su guion de verdad y
   añadir un guion de control con casos de la vida real (inventados, sin
   datos de nadie).

### Android

Las mismas preguntas y tablas, copiando `preguntas.rs` (`vivo`, `repaso`) y
las funciones `decidir`/`proponer`/`actualizar` de cada módulo, y
`fechas.rs` (puro cálculo, sin dependencias). La API es un POST con JSON;
la clave en el Keystore, nunca en lo que se sincroniza. Lo decidido viaja
como hoy viajan las tareas, los eventos y la nota de voz (`importantes`,
`temas`); las propuestas sin aceptar no viajan. Los guiones y las
respuestas grabadas sirven también para probar la copia del móvil: la misma
petición tiene que dar la misma huella.

## Audio (pixpin-voz)

Dos prototipos en `crates/pixpin-voz/examples/`, con lo comun (leer el
audio, encontrar el modelo) en `examples/comun/mod.rs`:

| Prototipo | Lo que prueba | El camino de la app |
|---|---|---|
| `alinear_cancion` | «Sincronizar sola»: el minuto de cada linea de una letra pegada | `ventana_chat/musica.rs` → `Whisper::oir_cancion_pcm` → `alinear_letra::palabras_oidas` → `alinear_letra::alinear` |
| `frases_en_vivo` | La escucha en vivo de una reunion: cortar frases y pasarlas a texto | `grabar_clase/escucha.rs` → `en_vivo::Frasero` + `Cola` + `Freno` → `Whisper::cargar_ligero` + `transcribir_pcm` |

### Que corre en Linux

- **Whisper base, el mismo de la app**: el modulo `whisper` compila ahora
  tambien fuera de Windows (bajar el modelo, con WinHTTP, y decodificar, con
  Media Foundation, siguen siendo solo de Windows). `ort.rs` ya cargaba
  `libonnxruntime.so` con `dlopen`; vale el que trae el paquete de Python
  `onnxruntime` (probado con **1.29.0**).
- **El audio**: un WAV PCM de 16 bits se lee a mano (cualquier muestreo y
  canales, a 16 kHz con `en_vivo::a_16k`); MP3, OGG, M4A… se le piden a
  `ffmpeg` si esta en el `PATH` (`ffmpeg -i x -ac 1 -ar 16000 -f s16le -`).
  Sin `ffmpeg`, convertir antes a WAV.
- SAPI (el reconocedor de Windows) no existe en Linux: en vivo se mide con
  Whisper base, que es el que usa la app cuando el de Windows falla o no
  esta.

### Preparar (una vez; nada de esto va al repositorio)

```sh
D=/tmp/pixpin-voz            # cualquier carpeta fuera del repo
mkdir -p $D/raiz/whisper/base $D/ort $D/audio
# El modelo, los mismos tres ficheros que baja la app (unos 160 MB).
for f in base-tokens.txt base-encoder.int8.onnx base-decoder.int8.onnx; do
  curl -L -o $D/raiz/whisper/base/$f \
    https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base/resolve/main/$f
done
# ONNX Runtime: el del paquete de Python vale.
pip install onnxruntime
ln -s "$(python3 -c 'import onnxruntime,os;print(os.path.dirname(onnxruntime.__file__))')"/capi/libonnxruntime.so.* \
  $D/ort/libonnxruntime.so
```

Audio de prueba, todo de **dominio publico** (National Jukebox de la
Library of Congress, grabaciones de 1908-1920, y LibriVox):

```sh
cd $D/audio
IA=https://archive.org/download
curl -L -o ballgame.mp3  $IA/loc-jukebox-730655-take-me-out-to-the-ball-game/jukebox-730655_take-me-out-to-the-ball-game.mp3  # 1908, ingles
curl -L -o cielito.mp3   $IA/loc-jukebox-32729-cielito-lindo/jukebox-32729_cielito-lindo.mp3                                    # 1919, castellano
curl -L -o mananitas.mp3 $IA/loc-jukebox-35389-las-ma-anitas/jukebox-35389_las-ma-anitas.mp3                                    # 1920, castellano
# Una «reunion» de 3 min con cuatro lectores (Cuentos de la selva, Quiroga):
# 45 s de cada capitulo, saltando la entradilla, con 2 s de silencio entre ellos.
for c in 01 02 03 04; do
  curl -L -o cap$c.ogg $IA/cuentosdelaselva_1603_librivox/cuentosdelaselva_${c}_quiroga.ogg
  ffmpeg -ss 25 -t 45 -i cap$c.ogg -ac 1 -ar 16000 -c:a pcm_s16le trozo$c.wav
done
ffmpeg -f lavfi -t 2 -i anullsrc=r=16000:cl=mono -c:a pcm_s16le silencio.wav
printf "file '%s'\n" trozo01.wav silencio.wav trozo02.wav silencio.wav \
  trozo03.wav silencio.wav trozo04.wav > lista.txt
ffmpeg -f concat -safe 0 -i lista.txt -c:a pcm_s16le reunion.wav
```

### `alinear_cancion`

```sh
cargo run -p pixpin-voz --release --example alinear_cancion -- \
  $D/audio/cielito.mp3 letra.lrc --raiz $D/raiz --ort $D/ort [--idioma es] \
  [--solo-tiempos] [--oido] [--lrc salida.lrc]
```

Sale cada linea con su `[m:ss]` (lo que guardaria la app), la confianza y si
es **ancla** (sale de lo oido) o **repartida**. Si la letra ya trae tiempos
(LRC o `[m:ss]`), se le quitan para alinearla y se usan como verdad: sale el
error de cada linea y la media, el maximo y cuantas caen a 1 y 2 s.
`--solo-tiempos` no imprime el texto de las lineas (para compartir lo medido
sin copiar la letra).

**La verdad** de cada linea es el inicio de su primera palabra segun
faster-whisper *large-v3* con `word_timestamps` (unos ±0,3 s), casada con la
letra en orden y revisada linea a linea donde la primera palabra no caso. La
letra de la prueba en ingles es la de 1908 con las estrofas como se cantan
en esa grabacion; las dos en castellano son las lineas que oye *large-v3*
(una linea por frase cantada). Las letras y la verdad no se guardan aqui:
se rehacen con las grabaciones de arriba.

```sh
pip install faster-whisper   # solo para la verdad
python3 - <<'EOF'
from faster_whisper import WhisperModel
import wave, numpy as np
w = wave.open("cielito.wav"); pcm = np.frombuffer(w.readframes(w.getnframes()), np.int16) / 32768
for s in WhisperModel("large-v3", compute_type="int8").transcribe(
        pcm.astype("float32"), language="es", word_timestamps=True)[0]:
    print(s.words[0].start, s.text)
EOF
```

#### Medido (29-oct-2026, Xeon 2,1 GHz, 4 nucleos, ONNX Runtime 1.29)

Whisper base oye la cancion entera en 15-20 s: **RTF 0,08-0,13** con todos
los hilos (`Whisper::cargar`, como la app). Alinear tarda menos de 10 ms.

Error por linea (|minuto dado − verdad|), media / maximo / lineas a ≤1 s:

| Paso | Take me out… (1908, en, 40 lineas) | Cielito lindo (1919, es, 24) | Las mañanitas (1920, es, 19) |
|---|---|---|---|
| Antes | 0,91 s / 3,7 s / 60 % | 2,81 s / 14,8 s / 50 % | 3,34 s / 7,2 s / 26 % |
| + sin anotaciones | 0,71 s / 3,6 s / 70 % | 2,64 s / 13,9 s / 50 % | 3,34 s / 7,5 s / 21 % |
| + repartir sobre la voz (sin ventana) | 0,89 s / **12,7 s** / 75 % | 3,00 s / 11,9 s / 54 % | 3,81 s / **30,0 s** / 32 % |
| + gemelas + ventana delante (×2) | 0,61 s / 2,1 s / 75 % | 1,36 s / 4,2 s / 58 % | 3,18 s / 18,5 s / 32 % |
| **Ahora** (ventana ×1,5) | **0,59 s / 2,1 s / 78 %** | **1,36 s / 4,2 s / 58 %** | **2,40 s / 8,2 s / 37 %** |

Las anclas solas, ahora: 0,55 s / 1,4 s (15 de 40), 0,90 s / 3,0 s (13 de
24), 0,96 s / 1,1 s (3 de 19). Lo flojo es *Las mañanitas*: Whisper base
solo ancla 3 de 19 lineas (tres voces y orquesta de 1920), y el resto se
reparte.

Lo que se cambio, y por que:

1. **Sin anotaciones** (`whisper::Fichas::sin_habla`, solo en
   `oir_cancion_pcm`): Whisper base escribia «[Music]» o «♪» en vez de la
   letra en 9 de 25, 5 de 30 y 13 de 29 tandas. Se suprimen los tokens de
   `non_speech_tokens` de `openai/whisper`; las notas de voz no cambian.
2. **Repartir sobre la voz** (`alinear_letra::Voz`): las lineas sin ancla se
   reparten sobre los tramos con palabras oidas (casen o no con la letra), y
   no al ritmo detras de la primera ancla: una estrofa que Whisper oye mal
   pero oye va donde se canto, y un solo sigue sin estirar nada (alli no se
   oyen palabras). Detras de la ultima ancla, hasta lo ultimo que se oye.
   Sola, empeoraba el principio: lo que Whisper se inventa en la intro
   cuenta como voz (la primera linea, 30 s antes). De ahi la ventana:
3. **Ventana delante de la primera ancla**: solo lo cantado en 1,5 veces el
   ritmo de la cancion antes de ella (`LENTO`).
4. **Gemelas**: una linea repetida (un estribillo cantado dos veces
   seguidas) casaba con la vez equivocada (12 s antes en *Cielito lindo*);
   si su gemela cae mucho mas cerca de donde toca al ritmo desde las anclas
   de alrededor, el ancla pasa a la gemela.

Probado y **descartado**: filtrar la banda de la voz (200-3 500 Hz) antes de
Whisper no mejora (0,93 / 1,38 / 2,37 s de media: estas grabaciones ya son
de banda estrecha); tandas de 12 s en vez de 8 empeoran (1,01 / 3,65 /
3,64 s) y tardan el doble; de 5 s salen las mismas tandas que de 8 (los
cortes en silencio mandan).

Pruebas nuevas en `alinear_letra`: `la_voz_son_los_tramos_con_palabras_oidas`,
`una_estrofa_mal_oida_se_reparte_por_donde_se_canto`,
`lo_que_whisper_se_invente_en_la_intro_no_adelanta_la_primera_linea`,
`el_estribillo_repetido_que_se_oyo_una_vez_va_a_la_vez_que_toca`; en
`whisper`: `las_anotaciones_y_las_notas_no_son_habla_y_las_palabras_si` y
`en_una_cancion_las_anotaciones_no_se_eligen_y_gana_lo_siguiente`.

### `frases_en_vivo`

```sh
cargo run -p pixpin-voz --release --example frases_en_vivo -- \
  $D/audio/reunion.wav --raiz $D/raiz --ort $D/ort [--rapido] \
  [--medida whisper|windows] [--salida frases.txt] [--quien Nombre]
```

El audio entra en el `Frasero` como lo da la mezcla de la grabacion (48 kHz
estereo, bloques de 10 ms) y cada frase se pasa a texto con Whisper base
cargado como en la app (`cargar_ligero`: el reparto del modo cuidadoso, uno
o dos hilos), con la misma `Cola` (3, tira lo mas viejo) y el mismo `Freno`.
Sin `--rapido`, en tiempo real con dos hilos como la app; con `--rapido`,
tiempo real simulado (el reloj de la grabacion cuenta cuando saldria cada
frase y cuando estaria su texto con un solo reconocedor).

Por frase: inicio, fin de la voz, **corte** (se dejo de hablar → sale la
frase), espera en la cola, lo que tarda Whisper, **retraso** (se dejo de
hablar → texto) y RTF. `--salida` escribe el guion oido
`[inicio fin_voz texto] Quien: frase` que leen los prototipos de Jev
(`crates/pixpin-typesafe/examples/`, ver «De punta a punta»); aqui no
se sabe quien habla, asi que `Quien` es `--quien` (por omision «Voz»).

#### Medido (la «reunion» de 186 s, cuatro lectores)

| Medida | Frases | Duracion media | Corte medio / max | Whisper medio / max | Retraso medio / max | RTF |
|---|---|---|---|---|---|---|
| `PARA_WHISPER`, tiempo real | 25 | 7,0 s | 0,66 / 1,70 s | 1,93 / 2,94 s | **2,59 / 4,30 s** | 0,275 |
| `PARA_WHISPER`, simulado | 25 | 7,0 s | 0,66 / 1,70 s | 1,98 / 3,29 s | 2,64 / 4,43 s | 0,282 |
| `PARA_WINDOWS`, simulado | 43 | 4,0 s | 0,49 / 1,98 s | 1,39 / 2,24 s | 1,92 / 3,59 s | 0,346 |

Ninguna frase espero en la cola con `PARA_WHISPER` ni se salto (el freno
salta la siguiente si una tarda mas del 80 % de lo que dura; aqui ninguna
paso del 40 %). Con `PARA_WINDOWS` el freno salto una (`SaltarRetraso`,
tras una frase corta que costo casi lo que duraba): con Whisper cada
llamada cuesta casi lo mismo corta que larga, por eso su medida junta 5 s.
Y se entiende peor: contra lo que oye *large-v3* (450 palabras), el texto
junto tiene un WER de **8,9 %** con `PARA_WHISPER` y de 14,4 % con
`PARA_WINDOWS` (con la frase saltada dentro): a frase mas corta, menos
contexto para Whisper.

El simulado da lo mismo que el tiempo real (±0,05 s): para
probar medidas basta `--rapido`, que tarda 50 s en vez de 3 min. Casi todas
cortan a los 0,5 s de callarse; las de 1,5-1,7 s son dos frases de menos de
5 s que esperan a `SILENCIO_LARGO_MS` y dos largas (10 s) cuyo silencio tarda
en contar (seguramente ruido de sala por encima del umbral de voz, RMS 300).

## Más casos (29-oct-2026)

Lo que faltaba por probar con Jev fuera de la reunión frase a frase: los
cuatro módulos que ya usaba la app y nunca se habían probado de verdad, y
siete ideas nuevas. Mismo banco de pruebas (`examples/comun/`), con un
guion principal y uno **de control** por idea (no se usó para ajustar).

### Guiones de casos sueltos

Una captura, una nota, una ráfaga de fotos o un aviso no son frases de una
reunión: van como **casos** en la cabecera del guion (`comun/casos.rs`).
`# caso: c01` empieza uno y las claves que siguen son suyas; las de antes
del primero son de todos; `\n` en un valor es un salto de línea. Cada
prototipo apunta «id → lo decidido» y las esperas lo buscan como siempre:

```text
# chat: Obra Calle Mayor | reforma del piso: planos, presupuestos...
# caso: c01
# ventana: Presupuesto_reforma.pdf - Visor de PDF
# texto: Partida 3: alicatado del baño ..... 312,00 €
# espera: c01 → obra calle mayor
```

Las **fotos** son inventadas, dibujadas con Pillow por
`guiones/fotos/hacer_fotos.py` (personas de dibujo, pizarras, hojas,
pantallas, una tabla, letra «a mano» letra a letra): ninguna foto de nadie.
El mismo script escribe `fotos/verdad.txt`, la caja de verdad de cada
pizarra para medir el recorte. Pillow 13.0; con otra versión pueden salir
otros bytes y las respuestas grabadas ya no casarían.

```sh
cargo run -q -p pixpin-typesafe --example mejor_foto -- rafagas.txt --rapido \
    --grabadas grabadas/mejor_foto.rafagas.jsonl
```

### Los prototipos

| ejemplo | módulo | guiones | pregunta a Jev | decide el código |
|---|---|---|---|---|
| `partes_importantes` | `reunion` (ya en la app: `importantes`) | `importantes`, `importantes_control` | 8 Nouls por renglón | `reunion::decidir` |
| `tarjetas_anki` | `anki` | `anki`, `anki_control` | Score de utilidad + Noul de examen | `anki::veredicto`, `anki::elegir` |
| `urgencia_suelta` | `urgencia` (la lista de tareas) | `urgencia`, `urgencia_control` | Score de consecuencia + 3 Nouls | puntos de `urgencia::decidir` (los días los cuenta el código) |
| `elegir_modelo` | `modelo` (lo de `jev::modelo::segun_jev`) | `modelo`, `modelo_control` | Score de dificultad + 3 Nouls | `modelo::decidir` |
| `captura_a_chat` | `captura_a_chat` (nuevo) | `capturas`, `capturas_control` | ¿es de algún chat? (Noul) y ¿de cuál? (Choice con nombre y de qué trata) | guardar / sugerir / ninguno |
| `etiquetas_solas` | `etiquetas` (nuevo) | `etiquetas`, `etiquetas_control` | un Noul por etiqueta del usuario | ≥ 0.7 se pone (3 como mucho); 0.5-0.7 se sugiere si no hay ninguna |
| `mejor_foto` | `mejor_foto` (nuevo, imágenes) | `rafagas`, `rafagas_control` | por foto: nítida, ojos cerrados, oscura, quemada, torcida; y ¿cuál se quedaría? (Choice) | descarta por motivo, elige la mejor candidata |
| `recortar_pizarra` | `pizarra` (nuevo, Box) | `pizarras`, `pizarras_control` | ¿hay pizarra u hoja? (Noul), ¿dónde? (Box), ¿se ve entera? (Noul) | recorte con 2 % de margen; «falta un trozo» |
| `elegir_ocr` | `motor_ocr` (nuevo, imágenes) | `ocr`, `ocr_control` | ¿hay texto?, ¿de qué clase? (captura, impreso, tabla, manuscrito) y, con dos lecturas, ¿cuál copia mejor? | Windows o Indra (`docs/ocr.md`) |
| `avisos_listos` | `alertas` (nuevo) | `avisos`, `avisos_control` | ¿automático?, ¿pide algo ya?, ¿esperar una hora haría daño?, ¿es de la reunión? | interrumpir / discreto / mostrar / para después / bandeja según lo que hace el usuario (lo sabe la app) |
| `citas_del_chat` | `horario_de_chat` (nuevo) | `chat_citas`, `chat_citas_control` | ¿pone un momento a una cita?, ¿cuál de las fechas?, ¿acordada o propuesta?, ¿acepta lo pendiente? | pendiente → evento cuando otro lo acepta |

Cada tabla de decisión está en la cabecera de su módulo, con sus pruebas
(`cargo test -p pixpin-typesafe`), y el texto de las preguntas al final de
`preguntas.rs` (`captura_a_chat`, `etiquetas`, `mejor_foto`, `pizarra`,
`ocr`, `alertas`, `horario_de_chat`).

### Resultados con la API de verdad (jev-1.14.0, desde Linux)

Antes = las preguntas como se escribieron; después = tras ajustar (solo con
el guion principal). Latencia y tokens, los de la última vuelta.

| ejemplo · guion | esperas antes | después | peticiones | p50 / p95 ms | tokens | tokens por petición |
|---|---|---|---|---|---|---|
| partes_importantes · importantes | 12/12 | 12/12 | 14 | 167 / 216 | 15 221 | 1 087 |
| partes_importantes · control | 9/9 | 9/9 | 9 | 167 / 179 | 9 571 | 1 063 |
| tarjetas_anki · anki | 14/14 | 14/14 | 9 | 182 / 269 | 4 796 | 532 |
| tarjetas_anki · control | 8/8 | 8/8 | 6 | 174 / 180 | 3 184 | 530 |
| urgencia_suelta · urgencia | 8/8 | 8/8 | 8 | 186 / 320 | 5 069 | 633 |
| urgencia_suelta · control | 6/6 | 6/6 | 5 | 179 / 244 | 3 140 | 628 |
| elegir_modelo · modelo | 11/12 | 12/12 (sin cambiar nada, ver abajo) | 12 | 189 / 362 | 8 880 | 740 |
| elegir_modelo · control | 5/6 | 5/6 | 6 | 198 / 384 | 4 386 | 731 |
| captura_a_chat · capturas | 11/11 | 11/11 | 11 | 166 / 223 | 7 563 | 687 |
| captura_a_chat · control | 6/6 | 6/6 | 6 | 176 / 226 | 3 743 | 623 |
| etiquetas_solas · etiquetas | 11/13 | 13/13 | 10 | 166 / 217 | 11 449 | 1 144 |
| etiquetas_solas · control | 5/5 | 5/5 | 5 | 166 / 206 | 4 544 | 908 |
| mejor_foto · rafagas (16 fotos) | 13/14 | 14/14 | 5 | 230 / 329 | 15 365 | 3 073 |
| mejor_foto · control (8 fotos) | 8/8 | 8/8 | 3 | 216 / 228 | 8 301 | 2 767 |
| recortar_pizarra · pizarras | 7/7 | 7/7 | 6 | 238 / 248 | 6 093 | 1 015 |
| recortar_pizarra · control | 4/4 | 4/4 | 4 | 328 / 435 | 4 061 | 1 015 |
| elegir_ocr · ocr | 13/15 | 15/15 | 6 | 196 / 202 | 7 080 | 1 180 |
| elegir_ocr · control | 11/11 | 11/11 | 4 | 193 / 223 | 4 947 | 1 236 |
| avisos_listos · avisos | 10/11 | 11/11 | 11 | 176 / 203 | 7 072 | 642 |
| avisos_listos · control | 5/6 | 6/6 (*) | 6 | 168 / 171 | 3 787 | 631 |
| citas_del_chat · chat_citas | 8/8 | 8/8 | 8 | 175 / 229 | 4 776 | 597 |
| citas_del_chat · control | 6/7 | 7/7 (*) | 7 | 171 / 178 | 4 033 | 576 |

(*) Lo que falló en estos dos controles se arregló en el código, no en las
preguntas, y no son ya del todo casos «no vistos»: ver 4 y 6.

**El recorte de la pizarra**: la caja de Jev casa con la verdad con un IoU
de **0,92-0,98** (media 0,95 en las ocho fotos con pizarra u hoja, también
torcidas, cortadas por el borde o una hoja pequeña en una esquina); el
recorte, con su margen del 2 %, 0,86-0,87 de media. «¿Se ve entera?» acierta
las dos del guion principal que el borde corta (0,07 y 0,16) y no se
confunde con el cuadro ni con la ventana (`hay_pizarra` 0,07 y 0,08).

**Lo que se ajustó** (solo con el guion principal):

1. `etiquetas`: «¿es «Universidad» uno de los temas principales de la
   nota?» daba 0,10 con unos apuntes de cálculo y 0,24 con un horario de
   clases (el tema es el cálculo, no la universidad). Ahora pregunta si el
   usuario **archivaría** la nota en esa etiqueta, «también cuando la
   etiqueta nombra una parte de la vida y no el tema» (con un ejemplo que no
   es de ningún guion: la factura del veterinario en «Mascotas»). → 0,76 y
   0,74; las demás igual.
2. `ocr::tipo`: una hoja impresa fotografiada salía «captura de pantalla»
   (0,75 y 0,85). Las opciones dicen ahora lo que se ve: ventanas, menús,
   botones o burbujas de chat; una hoja de papel con párrafos sobre una
   mesa. → impreso 0,87 y 0,68.
3. `mejor_foto`: «¿está bien expuesta? (ni oscura ni quemada)» daba 0,50
   con la foto quemada: dos preguntas, `oscura` y `quemada` (la regla de
   «cuidado con la "o"»). → quemada 0,89.
4. `alertas` (la tabla, en código): «nuevo inicio de sesión desde Moscú, ¿has
   sido tú? cambia la contraseña ahora» es automático (0,98) y se iba a la
   bandeja. **Automático no es sin importancia**: lo automático que pide
   algo ya (`pide_ya` ≥ 0,7) sigue por las filas de abajo. El control lo
   había destapado antes (la tarjeta bloqueada del banco: automático 0,88,
   grave 0,69 → bandeja); se añadió al principal un caso parecido, no el
   mismo, y el control pasó de 5/6 a 6/6.
5. `modelo::razonar` (redacción que ya existía): «¿cuánto es el 20 % de
   150?» salió una vez «razonar» 0,70 (al modelo fuerte) y otra 0,67. Se
   probó añadir «una sola cuenta no es razonar»: el principal salía bien,
   pero la dosis de jarabe del control bajaba de 0,89 a 0,55 y se iba al
   modelo rápido. **Se dejó como estaba.** El fallo que queda en el
   control es la lista de la compra de una paella: «importa» 0,68-0,72
   según la vuelta y dificultad en el borde → fuerte por «importante».
6. `horario_de_chat` (código): «el domingo comemos en casa de la abuela a
   las 2» salía como dos expresiones de `fechas::buscar` (el domingo sin
   hora; las 2 de hoy). `horario_de_chat::juntar`: un solo día sin hora y
   una sola hora suelta en el mismo mensaje son el mismo momento. Lo
   destapó el control (6/7 → 7/7). Valdría también para `fechas` en la
   reunión; aquí no se tocó ese módulo.

**Lo que se ve en las respuestas**:

- **Un mismo cuerpo no siempre da el mismo número**: entre dos vueltas,
  ±0,03-0,05 (razonar 0,70 / 0,67; importa 0,72 / 0,68 / 0,70). Lo que cae
  justo en 0,7 cambia de fila de una vuelta a otra. Las tablas con una
  franja dudosa (sugerir, dejar para después) lo aguantan; las de dos
  salidas (rápido o fuerte) no.
- **Las imágenes**: ~900 tokens una foto de 512 × 384; una ráfaga de 3-4
  fotos con 5 preguntas por foto, ~3 000-4 000 tokens y **230 ms** de
  mediana (frente a 170 ms con solo texto). El Box no tarda más que un
  Noul.
- Las fotos son **dibujos**: personas de dibujo, letra «a mano» hecha letra
  a letra. Que Jev acierte aquí dice que las preguntas están bien hechas,
  no que acierte con fotos de verdad: antes de encender `mejor_foto`,
  `pizarra` o `motor_ocr` en la app hay que probarlas con fotos reales
  (con permiso, o de dominio público) y con más casos de control.

**Coste**: la última vuelta de los 22 guiones, 161 peticiones y ~147 000
tokens ≈ **0,006 $**; toda la sesión (antes, ajustes y después, ~430
peticiones) ≈ 0,02 $. En la app: una captura a su chat o una nota a sus
etiquetas ≈ 0,00003 $; una ráfaga de cuatro fotos ≈ 0,00017 $.

### Cómo se llevaría a la app

- **Captura → chat**: al guardar una captura con texto (el OCR ya corre),
  `captura_a_chat::elegir` con los chats del usuario y una línea de qué
  trata cada uno (su descripción o los títulos de lo último). «Guardar»
  solo con deshacer; «sugerir», dos botones en el aviso de la captura.
- **Etiquetas**: al crear una nota o al terminar el OCR de una captura,
  `etiquetas::elegir` con las etiquetas que ya existen (nunca crea una).
- **Mejor foto**: en la cámara de PixPin y en lo que llega del móvil cuando
  hay varias fotos en pocos segundos; `mejor_foto::elegir`, las demás a
  una carpeta «descartadas» (nunca se borran solas).
- **Pizarra**: botón «recortar lo escrito» en la foto; `pizarra::recortar`
  y el recorte con su margen; «falta un trozo» como aviso.
- **OCR**: `motor_ocr::preguntar` antes de leer cuando el usuario no eligió
  motor, o para quedarse con la mejor de dos lecturas; sin Indra bajado,
  sugiere bajarlo con manuscritos y tablas.
- **Avisos**: lo que sincroniza y lo que llega a los chats pasa por
  `alertas::decidir_aviso` solo mientras se graba, se presenta o hay una
  reunión en el horario; libre, todo como siempre.
- **Citas del chat**: por mensaje nuevo con fechas o con una propuesta de
  otro pendiente, `horario_de_chat::preguntar`; el evento, propuesto al
  calendario con confirmar (como `citas` en la reunión).
- Lo de antes (`reunion`, `anki`, `urgencia`, `modelo`) ya está cableado;
  estos guiones quedan como prueba de evaluación para cuando cambie el
  modelo (`jev-latest`) o una pregunta.

## De punta a punta (audio → Jev)

Lo que faltaba: los prototipos de Jev se probaron con guiones escritos a
mano, con quien habla. Aqui se prueba la cadena entera, en vivo: audio de
una reunion en castellano → `pixpin_voz::en_vivo::Frasero` (con la `Cola`
y el `Freno`) → Whisper base como en la app (`frases_en_vivo`) → la logica
de Jev en vivo (`temas_en_vivo` y `reunion_completa`), con el texto de
Whisper y **sin quien habla** («Voz»).

### Las reuniones y el audio

Tres guiones inventados (sin datos de nadie) en
`crates/pixpin-typesafe/examples/guiones/`, con la misma agenda
(«Cotización del viernes», «Paro de transportistas», «Notas de los
estudiantes», «Compra de proyectores») tratada en otro orden, muletillas
(«eh», «o sea», «mmm»), interrupciones (un «—» delante: pisa 0,35 s la
frase anterior), una tarea con plazo, una decision, una pregunta que nadie
contesta y «nos vemos el martes a las 3»:

- `audio_coordinacion` (lunes): los proyectores «para el final» y no se
  llega; un dictado al chat de Compras.
- `audio_departamento` (miercoles): el paro sale sin nombrarlo («los
  camiones, que dicen que paran») ni anunciar el cambio; los proyectores se
  nombran y se dejan para otra reunion; «Marta se encarga… mañana».
- `audio_compras` (jueves): las notas se aplazan y los proyectores no se
  tratan; «yo mando el correo antes del lunes».

Las esperas son las de siempre (`# espera…`) y valen para el guion escrito
y para lo oido; donde Whisper cambia palabras se espera lo que importa
(«sin respuesta ¿Y quién avisa», no la frase entera).

El audio se hace con **Piper** (`pip install piper-tts`, sin red al
hablar), una voz por persona por orden de aparicion: `es_MX-claude-high`,
`es_ES-davefx-medium` y `es_ES-sharvard-medium` (hablante F), con un eco
corto (`aecho`) y ruido rosa de fondo (`anoisesrc`, amplitud 0,004) con
`ffmpeg`. Entre turnos, 0,25-0,8 s de silencio. Salen reuniones de 74-90 s
(las voces sinteticas hablan deprisa). Piper no da dos veces el mismo
audio: cada toma es otra grabacion.

### Como se corre

```sh
# Una vez (fuera del repo): lo de «Preparar» de arriba, y Piper con sus voces.
pip install piper-tts
V=https://huggingface.co/rhasspy/piper-voices/resolve/main/es
for v in es_MX/claude/high/es_MX-claude-high es_ES/davefx/medium/es_ES-davefx-medium \
         es_ES/sharvard/medium/es_ES-sharvard-medium; do
  curl -L -O $V/$v.onnx; curl -L -O $V/$v.onnx.json
done
# Cada reunion: hablar → oir → Jev (lo oido) → Jev (lo escrito, con y sin nombres).
PIXPIN_VOZ=$D VOCES=<carpeta de las voces> PYTHON=<python con piper> \
TYPESAFE_API_KEY_FILE=… SALIDA=/tmp/punta \
  crates/pixpin-typesafe/examples/guiones/de_punta_a_punta.sh audio_coordinacion.txt
```

`QUE="jev"` repite solo Jev sobre lo ya oido (`$SALIDA/<guion>.frases.txt`);
cada vuelta graba sus respuestas (`--grabar`) en `$SALIDA`.

**El guion oido**: `frases_en_vivo --salida` escribe
`[0:12.3 0:17.1 0:19.0] Voz: …` (empieza, se deja de hablar, llega el
texto) y el script le pone delante la cabecera y las esperas del guion
escrito. Los prototipos leen las dos formas (los guiones a mano siguen
igual). Con un guion oido, cada frase entra **cuando llega su texto** y el
reloj de la reunion avanza con lo que tarda Jev de verdad (`--rapido`):
corte + Whisper (lo mide `frases_en_vivo` en tiempo real simulado, ver
arriba) y luego Jev, con el `Ritmo` juntando lo que llega mientras hay una
peticion en vuelo. Al final sale lo que se tarda **de que se deja de
hablar a lo decidido** (`voz_p50_ms`, `voz_max_ms` en el `RESUMEN`).

### Medido (29-oct-2026, jev-1.14.0, Whisper base, Xeon 4 nucleos)

**Lo que oye Whisper**: WER 9-12 %, 15-19 % y 18-21 % (dos tomas de cada
reunion; 193-236 palabras). Ademas de palabras («A no tener chat de
compras» por «Anota en el chat de Compras», «¡Má, estás en carga!» por
«Marta se encarga»), **se come frases enteras** cuando el trozo junta
varios turnos (en `audio_compras`, «Nos llegaron tres presupuestos…»; en
la segunda toma de `audio_departamento`, el encargo a Marta) y **la
puntuacion**: «Sabemos si el paro afecta también a los envíos urgentes.»
sin «¿?», o una pregunta y su respuesta en una sola oracion («Cuando se
publican el viernes en cuanto al tutor de tercero la suba.»).

**Latencia**, por reunion (primera toma):

| tramo | medido |
|---|---|
| corte (se calla → sale la frase) | 0,45-0,62 s de media, 1,5 s max |
| Whisper base (`cargar_ligero`) | 1,8-1,9 s de media, 2,7 s max (3,1 / 4,8 s con la maquina ocupada compilando) |
| Jev | p50 175-195 ms, p95 200-440 ms |
| **se deja de hablar → ✓ del tema** | **p50 3,0 s, max 3,3 s** (segunda toma: max 3,9 s); de ello ~2,8 s son corte y Whisper y 0,2 s Jev |
| se deja de decir una oracion → lo decidido (`reunion_completa`, partida en oraciones) | p50 3,1-3,9 s, p95 9 s, max 10,8 s |

Con un guion escrito el ✓ sale a los 0,2 s; con audio, a los ~3 s: lo pone
Whisper, no Jev. Los trozos son de 5-12 s (`PARA_WHISPER`), asi que la
primera oracion de un trozo largo espera a que acabe el trozo: de ahi los
9-10 s del p95 por oracion. Ninguna frase espero en la cola ni se salto.

**Aciertos** (esperas cumplidas; «sin nombres» = el guion escrito con
«Voz» en vez de quien habla; «oido» = el texto de Whisper):

| | escrito | sin nombres | oido, antes | oido, despues |
|---|---|---|---|---|
| `temas_en_vivo` (3 reuniones) | 9/12 | — | 11/12 | 12/12 (2ª toma 11/12) |
| `reunion_completa` (3 reuniones) | 35/36 | 35/36 | 29/36 | **31/36** (2ª toma 31/36) |

Quitar quien habla **no cuesta nada** (35/36 en los dos). Lo que cuesta es
Whisper: 6-7 esperas de 36. Repetidos 3 veces los temas (mismo texto), lo
oido da 34/36 y lo escrito 29/36: los fallos son **todos el mismo**, abajo.

### Lo que fallo y lo que se cambio

1. **La escucha junta turnos.** Con `PARA_WHISPER` una «frase» trae dos o
   tres turnos: «Pues yo diría que la segunda. ¡Má, estás en carga de
   llamar…! ¡Echo, mañana la llamo!» salia como **una** tarea con «yo
   diría» dentro; «¿Y quién avisa…? mm, buena pregunta. Vale, pasamos a
   la cotización…» como pregunta 0.46 (no se abria). Arreglo, en codigo:
   **`vivo::partir`** parte lo de `en_vivo` en oraciones (tras `.?!` y un
   espacio, no en «2.400»; delante de «¿¡» salvo tras una coma; un trozo
   de menos de 3 palabras va con el siguiente) y reparte el milisegundo por
   letras. Los prototipos lo hacen con un guion oido (`--sin-partir` para
   comparar). 29/36 → 31/36; cuesta ~1,8 peticiones por trozo (unos 0,08 $
   la hora con todo encendido). **En la app**: el hilo de `vivo` del
   cableado (arriba, paso 1) tiene que partir asi cada frase de
   `en_vivo` antes de preguntar; los temas en vivo no lo necesitan (miran
   la ventana de 40 s).
2. **Sin saber quien habla, `quien` adivina.** «Yo me encargo…» con
   «Voz»: `quien_dicho` 0.98 (alguien se lo queda) y `quien` reparte entre
   los participantes (Ana 0.40, Luis 0.37): una tarea a nombre de quien no
   es. Arreglo, en codigo (`compromisos::QUIEN_MINIMO` = 0.5): con el
   elegido por debajo, «¿quién?». Cuando se nombra a alguien sale 0.97.
   Con nombres en el guion no cambia nada (las grabadas dan lo mismo).
3. **Lo que no se arreglo** (limites de Whisper base, no de Jev): el
   dictado con «A no tener chat de compras» (0.38); «¿Cuándo se publican?»
   pegada a su respuesta sin puntuacion; «Sabemos si el paro afecta…» sin
   «¿?» (pregunta 0.57-0.65, no llega a 0.7); la tarea de Marta si Whisper
   se come su nombre o la frase.

**Probado y descartado** (sin ganancia medible, se quito):

- Decir en el estado que es una transcripcion automatica (sin quien habla,
  sin puntuacion fiable): la pregunta sin «¿?» bajo de 0.65 a 0.57 y lo
  demas igual.
- **«Compra de proyectores» marcado sin tratarse** (el unico fallo de los
  temas, en lo escrito 1 de cada 4 vueltas por reunion): se marca cuando
  se cierra **otra** compra («hacemos el pedido mañana», aceptar la
  cotizacion de la imprenta) — tratado 0.52-0.86, cerrado 0.74-0.94. Se
  probo (3 vueltas × 8 guiones, con `coordinacion` y
  `temas_prueba_ignorada` de control): (B) nombrar el caso en el «no» de
  `se_trato` («also one that only shares a word or a kind of task with
  it»), (C) que la fila de «a medias y cerrado» pida `ahora` ≥ CASI, y
  (D) las dos. Dio 87, 87, 88 y 89 de 96 (sin cambio: 87): ruido. **No se
  cambio `preguntas.rs`.** Queda abierto; lo que si se ve:
- **Jev no es determinista**: la misma peticion, byte a byte, dio tratado
  0.59 y 0.78 en dos vueltas (hasta ±0.2). Un caso en el borde cambia de
  lado de una vuelta a otra: para ajustar, varias vueltas, no una.
- `tema_anterior` **si arrastra** (la duda de arriba): «nos vemos el martes
  para cerrar lo del pedido» fue a «Compra de proyectores», y el «lo
  dejamos aquí» siguiente (de la agenda 0.40) siguio ahi; el aviso de
  cierre ya no nombro los proyectores.
- Otros de Jev que salen igual en lo escrito: «el paro empieza el
  miércoles» y «empieza el martes» salen como **evento** (cita 0.77: «an
  event to take place»); «Sí, sí, queda decidido» no contesta «lo
  corregimos esta semana, ¿no?».
- Las reuniones sinteticas duran 1,5 min: con `MINIMO_TRATADO_MS` (30 s)
  casi todo tema queda «sin tratar» para el aviso de cierre; de ahi que
  sus esperas solo miren que salga el tema que toca.

**Coste** de todo esto: ~4,1 M tokens ≈ 0,17 $ (la mayor parte, las 96
vueltas de los temas).
