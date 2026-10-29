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

**Hecho para la reunion** (29-oct-2026): los puntos 1, 2 y 4 para las
siete ideas de la reunion (tareas dichas, urgencia en vivo, tema olvidado,
tiempo por tema, decisiones, preguntas pendientes y fechas al horario).
La mesa de `reunion_completa` es ahora biblioteca
(`pixpin_typesafe::en_la_reunion`) y la usa la escucha de «Grabar clase»
(hilo `reunion-jev`, una peticion por frase); las fichas de la barra, el
acta de la nota (`pixpin_proyecto::acta_viva`) y lo que se ve en «Estudiar
la clase» y en la pagina compartida estan en `docs/jev.md` §6. El dictado
a chat y el repaso oral van aparte.

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
hablar → texto) y RTF. `--salida` escribe el guion `[m:ss] Quien: frase`
que leen los prototipos de Jev (`crates/pixpin-typesafe/examples/`); aqui no
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
