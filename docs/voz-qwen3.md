# Qwen3-ASR-0.6B: pasar grabaciones a texto mejor que con Whisper

*2026-10-25. Código: `crates/pixpin-voz/src/qwen3/`, `apps/pixpin/src/voz_motor.rs`.*

## Qué es y para qué

Un motor **opcional** de voz a texto para notas de voz y grabaciones de
clases o reuniones. Whisper *base* sigue siendo **el de fábrica y el de
reserva**. Qwen3-ASR-0.6B (Qwen/Alibaba, **Apache-2.0**) se elige en
Ajustes › Voz › «Pasar a texto con», y solo entonces se baja (unos 988 MB).

Por qué: en el banco de `docs/investigacion/2026-10-25-voz-a-texto-en-espanol.md`
(FLEURS es_419, PyTorch bf16) Qwen3-ASR-0.6B tiene un WER de 5,8 / 8,3 /
37,7 % en limpio / reunión / aula cercana, frente a 11,8 / 22,5 / 94,6 % de
Whisper base. La mitad de errores o menos, y en el aula pasa de inútil a
utilizable.

## Decisión de runtime: ONNX int8 de sherpa-onnx sobre el ONNX Runtime que ya hay

Se miraron tres caminos:

| Opción | Qué haría falta | Veredicto |
|---|---|---|
| **(a) ONNX + el `ort.rs` propio** | Una exportación (encoder + decoder con caché KV) y el decodificador voraz en Rust | **Elegida.** Ya hay una exportación int8 hecha y mantenida por sherpa-onnx (k2-fsa, los mismos de nuestro Whisper), con su decodificador C++ (Apache-2.0) como referencia. Cero dependencias nuevas, el mismo runtime que Whisper e Indra. |
| (b) GGUF + transcribe.cpp/ggml | Bajar una DLL de C++ y un FFI nuevo | Descartada: otra biblioteca nativa que mantener y firmar, y nada que ganar si (a) cabe. |
| (c) Exportar a mano con optimum/torch.onnx | Exportar y cuantizar aquí | Innecesaria: (a) ya existe y está probada. |

En Hugging Face hay una docena de exportaciones (andrewleech, OpenVoiceOS,
Daumee…). Se eligió **`csukuangfj2/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25`**
(exportación de Wasser1462/Qwen3-ASR-onnx, publicada por el autor de
sherpa-onnx y usada por su `offline-recognizer-qwen3-asr-impl.cc`), fijada al
commit `68818b23…`:

| Fichero | Bytes | sha256 |
|---|---|---|
| `tokenizer/tokenizer_config.json` | 12 487 | `4942d005…` |
| `tokenizer/merges.txt` | 1 671 853 | `8831e4f1…` |
| `tokenizer/vocab.json` | 2 776 833 | `ca10d7e9…` |
| `conv_frontend.onnx` (fp32) | 44 148 281 | `d22dc442…` |
| `encoder.int8.onnx` | 182 491 662 | `60748d3e…` |
| `decoder.int8.onnx` | 755 914 231 | `4f6885be…` |

Los sha256 completos están en `qwen3/catalogo.rs`. Es ONNX IR 9 / opset 17
con `MatMulInteger` + `DynamicQuantizeLinear` (cuantización dinámica
estándar, sin operadores de Microsoft): vale el `onnxruntime.dll` de
`System32` (1.17+) o el oficial 1.31.0 que ya baja Indra en los Windows que
no lo traen.

**Licencia**: los pesos de Qwen3-ASR-0.6B son Apache-2.0; la exportación es
una conversión de esos mismos pesos. El repositorio espejo de Hugging Face
no trae fichero de licencia propio (riesgo menor: se avisa con un enlace a
la ficha oficial). El decodificador de Rust está escrito a partir del de
sherpa-onnx (Apache-2.0), no copiado.

## Cómo corre (`crates/pixpin-voz/src/qwen3/`)

1. **Tandas** (`guion::tandas`): los mismos cortes por silencios que Whisper
   (`cortes.rs`, el puerto del móvil), frases juntas hasta 20 s y ninguna de
   más de 30, sin solapar; cada tanda sabe su milisegundo → `[m:ss] texto`.
   Las tandas calladas no se le dan al modelo.
2. **Mel** (`mel.rs`): el `WhisperFeatureExtractor` de 128 bandas sin
   rellenar a 30 s, en Rust. Probado contra `transformers` (diferencia
   < 2·10⁻³ en todos los puntos de la prueba).
3. **Frente convolucional → encoder**: ~13 vectores de 1024 por segundo; los
   finales a cero se quitan.
4. **Decoder** (el LLM de 0,6B, 28 capas, caché KV `[1, L, 8, 128]` por capa
   que se rellena en su sitio con los `key_delta`/`value_delta` de cada
   paso). Guion de chat con el idioma forzado:
   `…<|audio_start|><|audio_pad|>×n<|audio_end|><|im_end|>\n<|im_start|>assistant\nlanguage Spanish<asr_text>`.
   Voraz; para en `<|im_end|>`, en el tope (10 tokens/s + 16) o en un bucle.
5. **Tokens a texto** (`fichas.rs`): BPE a nivel de byte de Qwen escrito a
   mano (pretroceado de Qwen2, fusiones solo para el guion). Los números
   del guion coinciden con `tokenizers` (prueba con los ficheros de verdad).
6. `creible` como con Whisper. Con dos idiomas («cada trozo como se dijo»)
   el modelo adivina el de cada tanda y, si no es uno de los dos, se rehace
   con el primero. «Todo en el primero» (traducir) sigue con Whisper.

**Un hallazgo que cambia el WER**: el decoder int8 cuantiza cada activación
con una sola escala por llamada. Si el guion entero (~280 tokens con el
audio) entra de una vez, el texto pierde precisión. **Se le da en trozos de
64 tokens** (con la caché ya rellena), lo que de paso baja los logits de la
entrada de ~170 MB a ~40 MB:

| Entrada del decoder | WER limpio (44 frases) |
|---|---|
| De una vez (como sherpa-onnx) | 7,6 % |
| Trozos de 64 | **6,5 %** |
| Trozos de 32 | 6,5 % |
| Trozos de 16 | 6,4 % (más lento) |
| Referencia PyTorch bf16 | 5,8 % |

## Medidas (Xeon 2,8 GHz, 4 vCPU con AVX-512/VNNI, 15 GB)

Las 44 frases limpias del banco (9,3 min, las mismas de la investigación,
regeneradas de FLEURS es_419 con la misma semilla), WER con la misma
normalización (tildes, cifras a palabras). La referencia PyTorch son las
hipótesis guardadas del banco.

| Canal | WER | Δ frente a PyTorch (IC95 pareado) | RTF | Pico de RAM |
|---|---|---|---|---|
| Whisper base int8 (banco) | 11,8 % | | 0,11 | 0,7 GB |
| Qwen3-ASR PyTorch bf16 (banco) | 5,8 % | — | 0,35 | 2,9 GB |
| ONNX int8, prototipo numpy (trozos de 64) | 6,5 % | +0,7 [0,0, +1,5] | 0,53 | 1,66 GB* |
| **ONNX int8, Rust (`qwen3::motor`)** | **6,6 %** | **+0,8 [−0,2, +1,9]** | **0,54** | **1,56 GB** |
| ONNX int8, Rust, modo cuidadoso | 6,9 % | +1,0 [0,0, +2,4] | 0,84 | 1,47 GB |

\* con el intérprete de Python. El Rust es el canal entero (tandas de
20 s incluidas), con ONNX Runtime 1.29 y 4 hilos; el modo cuidadoso usa 3
hilos y no preempaqueta los pesos. Paridad: la diferencia con PyTorch no es
significativa (el IC incluye el cero) y la mayoría de frases salen
idénticas tras normalizar. Carga de los modelos: ~7 s.

### En el equipo de referencia (i3 de 3.ª gen., 4 GB)

No se pudo medir en el aparato. Estimación:

- **Memoria**: en modo cuidadoso (4 GB) sin preempaquetar, pico de
  ~1,47 GB del proceso (medido en Linux con la misma biblioteca). Cabe con Windows 10/11 y PixPin al lado si
  no hay otra cosa pesada; `turno_pesado` ya impide que coincida con otra
  transcripción o con aligerar un PDF. Por eso **se ofrece desde ~3,5 GB**
  de memoria física, no se exige 8 GB.
- **Velocidad**: el decoder lee ~600 MB de pesos int8 por token (unos 3,3
  tokens por segundo de audio en castellano) y en Ivy Bridge no hay AVX2 ni
  VNNI (MLAS usa sus núcleos SSE/AVX para int8). Con DDR3 y dos núcleos se
  espera un **RTF de ~1,5-3**: una clase de una hora tarda de 1,5 a 3 horas,
  en segundo plano. Para notas cortas o cuando importa la rapidez, Whisper.
  Ajustes lo dice: «tarda unas cuatro veces más que Whisper».

## La app

- `[voz] motor = "whisper" | "qwen3"` (`pixpin-store`), de fábrica Whisper.
- Ajustes › Voz: «Pasar a texto con» (Whisper base / Qwen3-ASR) con la línea
  de estado (MB a bajar, memoria, licencia; avance de la descarga; fallo),
  «Ficha de Qwen3-ASR» y «Borrar Qwen3-ASR». Elegir Qwen3-ASR empieza la
  descarga en un hilo (sha256 y tamaño de cada fichero; uno que no cuadra se
  borra); volver a Whisper la corta.
- `pixpin_voz::transcribir_con` usa Qwen3-ASR si está elegido y bajado; si
  falla (modelo roto, runtime viejo…) cae a Whisper y lo deja en el
  registro. Si Qwen3-ASR va a transcribir, «Pasar a texto» no baja Whisper.
- Avance por tanda y por token; cancelar se mira en cada token.

## Probar

```sh
# Lo puro, en Linux:
cargo test -p pixpin-voz
# Con el modelo de verdad (Linux o Windows):
PIXPIN_QWEN3=<carpeta con los 6 ficheros> PIXPIN_ORT=<carpeta con libonnxruntime.so|onnxruntime.dll> \
PIXPIN_CLIPS=<carpeta con uNN.wav y meta.json> PIXPIN_SALIDA=salida.json \
  cargo test -p pixpin-voz --release -- --ignored qwen3_de_verdad --nocapture
PIXPIN_QWEN3=<carpeta> cargo test -p pixpin-voz -- --ignored el_guion_da_los_numeros_de_tokenizers
```

`ort.rs` carga `libonnxruntime.so` con `dlopen` fuera de Windows solo para
estas pruebas.

## Android

Los mismos seis ficheros sirven tal cual con **onnxruntime-android**
(`com.microsoft.onnxruntime:onnxruntime-android`, la misma API en C/Java):

- Portar `mel.rs`, `fichas.rs` y `guion.rs` a Kotlin (son aritmética y texto;
  las pruebas de Rust traen los valores de referencia) y el bucle de
  `motor.rs` con `OrtSession.run`. Las mismas tres ideas: tandas de 20 s
  por silencios, guion en trozos de 64 tokens y caché KV del largo justo.
- Memoria: ~1,5 GB de pico; solo en móviles con 6 GB o más, y en un
  servicio en primer plano. El XNNPACK/NNAPI no ayuda con `MatMulInteger`:
  CPU con 4 hilos de los núcleos grandes (ARM con `sdot` va bien con int8).
- Descarga: los mismos URL fijados al commit y los mismos sha256 que
  `qwen3/catalogo.rs`; guardar en `filesDir/qwen3-asr/`.
- El texto sale igual en el PC y el móvil si se usa el mismo guion y la
  misma voracidad (el int8 de ARM y x86 puede diferir en algún token).

## Riesgos

- **Velocidad en el i3**: estimada, no medida (ver arriba).
- **Memoria en 4 GB**: ~1,5 GB de pico; con muchas cosas abiertas Windows
  tirará del fichero de paginación. El modo cuidadoso ya lo baja todo lo
  que se puede sin otra exportación.
- **Espejo sin licencia propia** en Hugging Face (pesos Apache-2.0 de Qwen).
- Marcas de tiempo por tanda (cada ~20 s), no por palabra. El
  Qwen3-ForcedAligner (español incluido) daría palabras, pero es otro modelo
  de ~0,6B: queda para después.
