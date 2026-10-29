# Probar ideas sin interfaz

Programas pequenos, sin ventanas, para ver si una idea funciona antes de
meterla en la app, y luego llevar **la misma logica** a la app (peticion del
usuario, 29-oct-2026). Viven como `examples/` de cada crate (no son paquetes
del workspace: `apps/pixpin/tests/capas.rs` los cuenta) y corren en la
maquina de desarrollo, que es Linux.

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
