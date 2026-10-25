# Benchmark ASR en español: Whisper vs Qwen3-ASR vs Mega-ASR (CPU)

*Fecha: 2026-10-25. Máquina: Xeon 2.1 GHz, 4 vCPU (AVX-512 + AMX-BF16), 15 GB RAM, sin GPU.*

## TL;DR / veredicto

- **Mega-ASR no mejora a Qwen3-ASR-1.7B base en español; en audio degradado es peor.** Con el router oficial, en audio limpio y en "reunión" queda igual que la base, porque el router casi nunca activa la LoRA (0/44 clips limpios y 3/44 de reunión). En las dos condiciones de "aula" la LoRA se activa en el 100 % de los clips y el WER *sube*: de 28.0 → 33.0 % en aula cercana y de 42.2 → 77.6 % en aula lejana. La subida de aula lejana viene de 2 clips donde el modelo entra en un bucle de repetición ("…para ir a la escuela de lenguas para ir a la escuela de lenguas…"). Aun quitando esos clips, la base gana (465 errores frente a 549) y gana clip a clip 23–11. En aula cercana Mega-ASR sí baja la mediana por frase (18 % frente a 22 %), pero falla peor en los casos difíciles y en el total pierde (diferencia pareada +5.1 pp, IC95 [+0.1, +10.8]).
- **Mega-ASR también cambia el formato de salida.** Con la LoRA activa escribe los números con letras ("mil novecientos sesenta y tres") y en audio ruidoso suele quitar mayúsculas y puntuación. Para una app de notas es un empeoramiento, aunque la métrica normalizada lo perdone.
- **Qwen3-ASR (cualquier tamaño) supera claramente a Whisper.** Qwen3-ASR-0.6B frente a Whisper base, la diferencia en WER es de −6 pp en limpio, −14 pp en reunión y −44/−57 pp en aula. Todas son significativas. El 0.6B también le gana a Whisper small en las cuatro condiciones (en limpio por poco).
- **Qwen3-ASR-0.6B es el mejor compromiso** si hace falta un modelo ligero (~2.9 GB de RAM pico en bf16, RTF ≈ 0.35 en 4 núcleos). El 1.7B es ~1–1.5 pp mejor en limpio y en reunión, y ~10–12 pp mejor en aula, pero necesita ~5.4 GB y es ~2× más lento (RTF ≈ 0.75).
- **Whisper base (el modelo actual de la app) se hunde en reverberación + ruido.** Llega a ~95–98 % de WER en aula, con muchas salidas vacías o alucinadas (a veces en portugués o inglés). En reunión (Opus + babble a 10 dB) tiene un 22.5 % de WER, frente al 8.3 % del Qwen 0.6B.

## Setup

| Modelo | Implementación | Precisión | Decodificación |
|---|---|---|---|
| Whisper base | faster-whisper 1.x (CTranslate2), `Systran/faster-whisper-base` | **int8** | `language="es"`, beam 5, sin VAD |
| Whisper small | ídem, `Systran/faster-whisper-small` | **int8** | ídem |
| Qwen3-ASR-0.6B | paquete oficial `qwen-asr` 0.0.6 (backend transformers 4.57.6) | bf16 | `language="Spanish"`, greedy, max_new_tokens 512 |
| Qwen3-ASR-1.7B | `qwen-asr`, pesos `Qwen3-ASR-1.7B/` del repo HF `zhifeixie/Mega-ASR` (no se descargó dos veces) | bf16 | ídem |
| Mega-ASR (LoRA forzada) | wrapper oficial `MegaASR` de github.com/xzf-thu/Mega-ASR, `routing_enabled=False` (= `--no-routing`) | bf16 | ídem |
| Mega-ASR (router) | router oficial `audio_quality_router` (umbral 0.5) aplicado a cada clip: si `degraded` → salida LoRA, si no → salida base | bf16 | ídem |

- torch 2.14.1+cpu, 4 hilos, batch 1, Python 3.13 en venv. Se eligió bf16 para Qwen y Mega porque en esta CPU con AMX es tan rápido como fp32 o más. Además, el 1.7B en fp32 no cabía con margen en la RAM compartida. Ojo: en una prueba con 3 clips, el 0.6B en fp32 y en bf16 dio salidas ligeramente distintas.
- **Cómo se evaluó Mega-ASR.** El wrapper oficial monta y desmonta la LoRA sumando y restando deltas densos sobre los pesos (`weight.add_(±delta)`). En bf16 ese ida y vuelta no es exacto: alternar base y LoRA en un mismo proceso iría corrompiendo poco a poco los pesos base. Por eso:
  - (a) la base se ejecutó en un proceso aparte sin LoRA;
  - (b) la LoRA se ejecutó con el wrapper oficial y `routing_enabled=False`, así que se aplica una sola vez al cargar;
  - (c) el modo router combina (a) y (b) según la decisión del router en cada clip. El resultado coincide con lo que haría el pipeline oficial, sin la deriva numérica.
- **RAM de Mega-ASR.** El wrapper oficial guarda en memoria todos los deltas densos (655 matrices) para poder conmutar, y llega a **~11.5 GB de pico** (medido en una prueba). Con la máquina compartida eso provocó OOM-kills. Para la LoRA forzada parcheé `LoRADeltaSwitch` para que aplique cada delta en cuanto lo calcula y luego lo descarte. Es la misma operación `weight.add_(delta)`, y la salida coincidió con la del wrapper sin parchear en los 6 clips comparados. Con el parche, el pico baja a ~5.2 GB, igual que la base. En producción se haría igual: fusionar la LoRA una vez, como hace su ruta vLLM.

## Datos

- **Origen:** FLEURS `es_419`, split *test* (google/fleurs; solo se descargaron `test.tsv` y `audio/test.tar.gz`; los audios se leyeron en memoria con soundfile, sin extraer ni ejecutar nada del dataset).
- **44 frases**, una por `id` de frase (sin frases repetidas), con 9–21 s cada una. En total son **9.3 min por condición** (37 min de audio evaluado por modelo). Hay 30 voces masculinas y 14 femeninas, todas de lectura. La selección es determinista (seed 1234). Para el babble se usaron 12 clips distintos de los evaluados.
- **Referencia:** la columna `raw_transcription` (con mayúsculas, puntuación y cifras).
- **Condiciones** (16 kHz mono, scripts en `scripts/prep.py` y `scripts/prep_aula_cerca.py`):
  1. **clean**: la señal original, normalizada en nivel.
  2. **reunion**: ruido de oficina/babble (70 % babble de 5 hablantes FLEURS + 30 % ruido rosa) a **SNR 10 dB**, después un ida y vuelta **Opus 24 kbps** (ffmpeg libopus, `-application voip`).
  3. **aula_cerca**: RIR sintética de ruido con decaimiento exponencial, **RT60 0.6 s**, con **DRR ≈ 0 dB** (micrófono a ~1–1.5 m), más babble y ruido rosa a **SNR 5 dB**.
  4. **aula** (o "aula lejana"): misma RT60 de 0.6 s, pero la RIR tiene **DRR ≈ −20 dB**, lo que equivale a un portátil al fondo del aula y muy lejos del profesor. Más babble y ruido rosa a SNR 5 dB. Fue la primera versión de "aula" y resultó muy dura. Se mantiene como caso extremo y se añadió `aula_cerca` como caso realista.
- **Métrica:** WER a nivel corpus con jiwer, después de aplicar `BasicTextNormalizer` de whisper_normalizer (minúsculas, sin puntuación). Se reporta:
  - **principal:** con tildes y con **números normalizados** (las cifras se pasan a palabras en español con num2words y se unen "6.000" / "333 000"; se aplica igual a referencias e hipótesis de todos los modelos);
  - sin tildes (`remove_diacritics=True`);
  - sin normalizar números ("crudo").

  Los IC95 salen de un bootstrap por frase (2000 réplicas). Las comparaciones entre modelos usan bootstrap pareado.

## Resultados

### WER (%) principal: tildes conservadas, números normalizados

| Modelo | clean | reunion | aula_cerca | aula (lejana) |
|---|---|---|---|---|
| whisper-base (int8) | 11.8 | 22.5 | 94.6 | 98.3 |
| whisper-small (int8) | 7.2 | 11.0 | 60.7 | 78.0 |
| qwen3-asr-0.6b | 5.8 | 8.3 | 37.7 | 54.6 |
| **qwen3-asr-1.7b** | **5.2** | 7.0 | **28.0** | **42.2** |
| mega-asr (LoRA forzada) | 5.3 | 7.4 | 33.0 | 77.6 |
| mega-asr (router) | **5.2** | **6.9** | 33.0 | 77.6 |

IC95 bootstrap (WER %):

| Modelo | clean | reunion | aula_cerca | aula |
|---|---|---|---|---|
| whisper-base | 9–15 | 17–29 | 84–105 | 95–102 |
| whisper-small | 4–10 | 7–16 | 51–71 | 70–85 |
| qwen3-asr-0.6b | 4–8 | 5–13 | 30–46 | 47–63 |
| qwen3-asr-1.7b | 3–8 | 4–10 | 21–36 | 35–50 |
| mega-asr-lora | 3–8 | 4–11 | 23–43 | 41–142 |
| mega-asr-router | 3–8 | 4–10 | 24–44 | 40–141 |

### WER sin tildes (números normalizados)

| Modelo | clean | reunion | aula_cerca | aula |
|---|---|---|---|---|
| whisper-base | 11.3 | 21.9 | 94.2 | 98.2 |
| whisper-small | 6.6 | 10.3 | 60.1 | 77.6 |
| qwen3-asr-0.6b | 5.5 | 8.1 | 37.3 | 54.3 |
| qwen3-asr-1.7b | 5.0 | 6.7 | 27.7 | 42.1 |
| mega-asr-lora | 5.2 | 7.2 | 32.5 | 77.3 |
| mega-asr-router | 5.0 | 6.6 | 32.5 | 77.3 |

Quitar las tildes cambia muy poco (≤ 0.7 pp): casi ningún modelo falla en tildes.

### WER "crudo" (tildes conservadas, números sin normalizar)

| Modelo | clean | reunion | aula_cerca | aula |
|---|---|---|---|---|
| whisper-base | 12.2 | 23.1 | 94.9 | 98.3 |
| whisper-small | 7.6 | 11.5 | 60.5 | 78.3 |
| qwen3-asr-0.6b | 6.3 | 10.8 | 38.5 | 54.9 |
| qwen3-asr-1.7b | 5.7 | 8.6 | 28.0 | 42.2 |
| mega-asr-lora | 8.5 | 10.5 | 34.2 | 79.6 |
| mega-asr-router | 5.7 | 8.5 | 34.2 | 79.6 |

Sin normalizar números, la LoRA de Mega-ASR sale penalizada (8.5 % frente a 5.7 % en limpio) porque escribe las cifras con letras.

### WER mediano por frase (%) y nº de frases en bucle o alucinación (hipótesis > 1.5× la longitud de la referencia)

| Modelo | clean | reunion | aula_cerca | aula |
|---|---|---|---|---|
| whisper-base | 9.1 (0) | 21.4 (0) | 100.0 (8) | 100.0 (2) |
| whisper-small | 4.1 (0) | 6.7 (0) | 49.7 (3) | 81.7 (0) |
| qwen3-asr-0.6b | 4.0 (0) | 4.3 (0) | 28.6 (0) | 53.9 (0) |
| qwen3-asr-1.7b | 3.5 (0) | 3.9 (0) | 22.2 (0) | 38.3 (0) |
| mega-asr-lora | 3.4 (0) | 3.6 (0) | **18.2** (1) | 40.0 (2) |
| mega-asr-router | 3.5 (0) | 3.9 (0) | 18.2 (1) | 40.0 (2) |

Interpretación: en aula cercana, la frase *típica* sale algo mejor con Mega-ASR (la mediana baja de 22 a 18 %). Pero tiene más errores catastróficos: más sustituciones "inventadas" (292 frente a 233) y un bucle de repetición. El total del corpus termina peor. Es lo que se espera de un modelo que, ante audio sucio, prefiere "adivinar" frases plausibles antes que borrar palabras (eliminaciones 36 frente a 60).

### Comparaciones pareadas (Δ WER en pp, A − B, IC95 por bootstrap pareado)

| Comparación | clean | reunion | aula_cerca | aula |
|---|---|---|---|---|
| Mega-LoRA − Qwen3-1.7B | +0.1 [−0.5, +0.7] | +0.4 [−0.6, +1.5] | **+5.1 [+0.1, +10.8]** | **+35.4 [+2.5, +94]** |
| Mega-router − Qwen3-1.7B | 0.0 | −0.1 [−0.3, 0.0] | +5.1 | +35.4 |
| Qwen3-0.6B − Whisper base | **−5.9** [−7.9, −4.0] | **−14.2** [−17.5, −10.9] | **−56.9** | **−43.7** |
| Qwen3-0.6B − Whisper small | −1.4 [−2.9, 0.0] | **−2.7** [−4.4, −1.1] | **−23.0** | **−23.4** |
| Qwen3-0.6B − Qwen3-1.7B | +0.6 [+0.1, +1.1] | +1.4 [0.0, +3.1] | +9.7 [+5.6, +14.1] | +12.4 [+7.9, +17.3] |
| Qwen3-1.7B − Whisper small | −2.0 | −4.0 | −32.7 | −35.8 |

### Router de Mega-ASR (fracción de clips enviados a la LoRA)

| | clean | reunion | aula_cerca | aula |
|---|---|---|---|---|
| clips con LoRA | 0/44 | 3/44 | 44/44 | 44/44 |
| P(degradado) media | 0.04 | 0.11 | 0.96 | 0.96 |

El router no considera degradado el audio de tipo reunión (Opus 24 kbps + babble a 10 dB). En la práctica, Mega-ASR con router *es* Qwen3-ASR-1.7B en ese escenario. Justo en los clips reverberantes, donde sí activa la LoRA, la LoRA empeora el resultado en español.

### Velocidad y memoria (CPU 4 hilos, batch 1)

| Modelo | RTF mediano (s cómputo / s audio) | RAM pico del proceso | Disco (pesos) |
|---|---|---|---|
| whisper-base int8 | **0.11** | 0.7 GB | 142 MB |
| whisper-small int8 | 0.32 | 1.5 GB | 464 MB |
| qwen3-asr-0.6b bf16 | 0.35 | 2.9 GB | 1.9 GB |
| qwen3-asr-1.7b bf16 | 0.77 | 5.4 GB | 4.7 GB |
| mega-asr bf16 (LoRA fusionada) | ≈0.6–0.8 (misma arquitectura que 1.7B) | 5.2 GB (con parche low-mem) / **~11.5 GB con el wrapper oficial** | 4.7 GB + 92 MB LoRA + 5 MB router |
| router de calidad | ~0.001 | despreciable | 5 MB |

Los RTF son medianas sobre los mismos 19 clips limpios, medidos con la máquina sin carga externa. **La máquina se compartió con compilaciones de Rust** durante buena parte de las ejecuciones (load average 4–10). Por eso los RTF totales y los p90 están inflados de forma desigual: el 1.7B y Mega llegaron a tener picos de RTF 5–20 por contención y no por el modelo. Los RTF absolutos son orientativos; lo fiable es el orden relativo: Whisper base ≈ 3× más rápido que Qwen 0.6B, que a su vez es ≈ 2× más rápido que el 1.7B. La carga del modelo tarda 7–10 s en Qwen y ~1–2 s en Whisper con caché caliente.

## Ejemplos (transcripciones lado a lado)

Selección automática por condición: el clip donde Mega-ASR más gana a la base 1.7B, el clip donde más pierde y el de mayor diferencia entre Whisper base y Qwen 0.6B. "Errores" = S+D+I tras la normalización. Las transcripciones largas están truncadas.

#### aula / u15 — Mega-ASR mejor que 1.7B base

| Modelo | Errores | Transcripción |
|---|---|---|
| **Referencia** | | Viajaron de vuelta hacia París en un carruaje rodeados por una muchedumbre que gritaban y lanzaban amenazas contra el rey y la reina. |
| whisper-base | 23 | *(vacío)* |
| whisper-small | 23 | *(vacío)* |
| qwen3-asr-0.6b | 9 | viajaron de vuelta a París en un gran crucero deidas por una monarquía que llevaban y gastaban millones contra el país y la reina. |
| qwen3-asr-1.7b | 11 | Viajaron de vuelta hacia París y fueron rodeados por una muchedumbre que estaban yacía con ganas de comprarle la reina. |
| mega-asr-lora | 4 | viajaron de vuelta hacia paris y encontraron rodeados por una muchedumbre que gritaban y lanzaban amenazas contra el rey y la reina. |

#### aula / u42 — Mega-ASR peor que 1.7B base

| Modelo | Errores | Transcripción |
|---|---|---|
| **Referencia** | | Debajo del puente hay un espacio libre de 15 metros y, si bien su construcción fue terminada en agosto de 2011, no se habilitó para el tránsito sino hasta marzo de 2017. |
| whisper-base | 36 | *(vacío)* |
| whisper-small | 36 | *(vacío)* |
| qwen3-asr-0.6b | 29 | debajo del fuente hay una superficie de cemento y si vemos que se han podido arrojar los mismos huecos por el borde de la gráfica en el tabaco de los edificios |
| qwen3-asr-1.7b | 27 | debajo del puente hay un pasillo de cemento y subiendo por el puente se puede mirar por el puente hacia el litoral del Ártico y el espacio de los edificios |
| mega-asr-lora | 355 | de ahí fue corriendo a una cafetería de internet y de allí fue a la estación de autobuses para ir a la escuela de lenguas para ir a la escuela de lenguas para ir a la escuela de lenguas para ir a la escuela de lenguas para ir a la escuela de lenguas para ir a  […] |

#### aula / u00 — Whisper base vs Qwen 0.6B

| Modelo | Errores | Transcripción |
|---|---|---|
| **Referencia** | | Entre las demás nominaciones se encuentran Mejor película, Mejor director, Mejor cinematografía, Mejor diseño de vestuario, Mejor edición de películas, Mejor partitura original, Mejor diseño de producción, Mejor edición de sonido, Mejor mezcla de sonido y Mejor guion original. |
| whisper-base | 39 | y informações que estáis en 1992, cooked a tunnel, |
| whisper-small | 12 | Entre las reglas, denominaciones, encuentran mejor película, mejor director, mejor cinematografía, mejor diseño del escuelo, mejor edición de películas, mejor arquitectura industrial, mejor diseño de producción, mejor edición de familiares, mejor lista de fami […] |
| qwen3-asr-0.6b | 8 | Entre las obras nominaciones se encuentran mejor película, mejor director, mejor cinematografía, mejor diseño de estuario, mejor edición de película, mejor película musical, mejor diseño de producción, mejor edición de sonido, mejor muestra de sonido, mejor gu […] |
| qwen3-asr-1.7b | 4 | Entre las premias nominaciones se encuentran mejor película, mejor director, mejor cinematografía, mejor diseño de vestuario, mejor edición de película, mejor transmisiones, mejor diseño de producción, mejor edición de sonido, mejor mezcla de sonido y mejor gu […] |
| mega-asr-lora | 4 | entre las premios nominaciones se encuentran mejor película, mejor director, mejor cinematografía, mejor diseño de vestuario, mejor edición de película, mejor maquillura, mejor diseño de producción, mejor edición de sonido, mejor mezcla de sonido y mejor guion […] |

#### aula_cerca / u11 — Mega-ASR mejor que 1.7B base

| Modelo | Errores | Transcripción |
|---|---|---|
| **Referencia** | | Liggins comenzó sus investigaciones sobre el parto prematuro en su tiempo libre, mientras trabajaba en el hospital. |
| whisper-base | 12 | Y les comenzó su financiación por el parro de natura en el tiempo de libre ciencia trabajaba en el latino. |
| whisper-small | 7 | Lillis comenzó su financiación sobre el parco de prematuras de tiempo libre mientras trabajaba en el hospital. |
| qwen3-asr-0.6b | 3 | Lilly comenzó sus investigaciones sobre el patrocinador en su tiempo libre mientras trabajaba en el hospital. |
| qwen3-asr-1.7b | 7 | Liz comenzó sus investigaciones sobre impactos prematuros y tiempo libre mientras trabajaban en el hospital. |
| mega-asr-lora | 1 | lilith comenzó sus investigaciones sobre el parto prematuro en su tiempo libre mientras trabajaba en el hospital |

#### aula_cerca / u42 — Mega-ASR peor que 1.7B base

| Modelo | Errores | Transcripción |
|---|---|---|
| **Referencia** | | Debajo del puente hay un espacio libre de 15 metros y, si bien su construcción fue terminada en agosto de 2011, no se habilitó para el tránsito sino hasta marzo de 2017. |
| whisper-base | 36 | *(vacío)* |
| whisper-small | 36 | *(vacío)* |
| qwen3-asr-0.6b | 29 | debajo del fuerte impacto debido a la intensa explosión fue terminado este vacío en octubre de 2017 |
| qwen3-asr-1.7b | 10 | debajo del puente hay un espacio libre de cinco metros y si bien su construcción fue terminada este verano no se realizó el tránsito hasta marzo de 2017 |
| mega-asr-lora | 32 | de bajo el cuadro de la infanta del rey es mencionado que si bien su actuación fue tan buena que su actuación no fue utilizada en cántico y no se notó en su debilidad |

#### aula_cerca / u16 — Whisper base vs Qwen 0.6B

| Modelo | Errores | Transcripción |
|---|---|---|
| **Referencia** | | Muchas personas no se los representan como dinosaurios, ya que tienen plumas y pueden volar. |
| whisper-base | 39 | Se las conmigo en el cerro de los dos, a ver las conmigo en las manos. Muchas los alzados. A ver la cosa está en el cerro de los alzados. La fecha es que uno más que uno más, que uno más. |
| whisper-small | 12 | Muchos personas no son representas como estos, ya que quieren tu lo más que pueda embajar. |
| qwen3-asr-0.6b | 2 | muchas personas no se representan como centauros ya que tienen plumas y pueden volar. |
| qwen3-asr-1.7b | 2 | Muchas personas no se representan como centauros, ya que tienen plumas y pueden volar. |
| mega-asr-lora | 5 | muchas personas no se representan como senadoras ya que tienen puro más y pueden votar |

#### clean / u01 — Mega-ASR mejor que 1.7B base

| Modelo | Errores | Transcripción |
|---|---|---|
| **Referencia** | | El Parque Nacional de los Lagos Plitvice es muy boscoso, tiene principalmente hayas, abetos y píceas, y presenta una vegetación entre alpina y mediterránea. |
| whisper-base | 7 | El Parque Nacional de los Lagos, Litfice es muy bosco, tiene principalmente hallas, avetos y picias y presento una vegetación entre alfina y mediterránea. |
| whisper-small | 4 | El Parque Nacional de los Lagos Splitfice es muy bocoso, tiene principalmente hallas, abetos y pizias y presenta una vegetación entre alpina y mediterránea. |
| qwen3-asr-0.6b | 4 | El Parque Nacional de los Lagos Platefize es muy boscoso, tiene principalmente allas, avetos y picias y presenta una vegetación entre alpina y mediterránea. |
| qwen3-asr-1.7b | 4 | El Parque Nacional de los Lagos Split-Visé es muy boscoso. Tiene principalmente haya, abetos y pícies y presenta una vegetación entre alpina y mediterránea. |
| mega-asr-lora | 3 | El parque nacional de los lagos splitfise es muy boscoso. Tiene principalmente ayas, abetos y pícies y presenta una vegetación entre alpina y mediterránea. |

#### clean / u33 — Mega-ASR peor que 1.7B base

| Modelo | Errores | Transcripción |
|---|---|---|
| **Referencia** | | De todas formas, el porcentaje de XDR-TB en el grupo total de enfermos de tuberculosis aún parece ser reducido: 6000 de un total de 333 000 individuos infectados en un momento particular en Sudáfrica. |
| whisper-base | 3 | De todas formas el porcentaje de XDR, TV, en el grupo total de enfermos de tuberculosis aún parece ser reducido, 6.000 de un total de 333.000 individuos infectados en un momento particular en su África. |
| whisper-small | 0 | De todas formas, el porcentaje de XDR-TB en el grupo total de enfermos de tuberculosis aún parece ser reducido, 6.000 de un total de 333.000 individuos infectados en un momento particular en Sudáfrica. |
| qwen3-asr-0.6b | 0 | De todas formas, el porcentaje de XDR-TB en el grupo total de enfermos de tuberculosis aún parece ser reducido: 6.000 de un total de 333.000 individuos infectados en un momento particular en Sudáfrica. |
| qwen3-asr-1.7b | 0 | De todas formas, el porcentaje de XDR-TB en el grupo total de enfermos de tuberculosis aún parece ser reducido: 6.000 de un total de 333.000 individuos infectados en un momento particular en Sudáfrica. |
| mega-asr-lora | 2 | De todas formas, el porcentaje de xdrtb en el grupo total de enfermos de tuberculosis aún parece ser reducido. Seis mil de un total de trescientos treinta y tres mil individuos infectados en un momento particular en sudáfrica. |

#### clean / u19 — Whisper base vs Qwen 0.6B

| Modelo | Errores | Transcripción |
|---|---|---|
| **Referencia** | | Expertos de la Universidad de Princeton, en EE. UU., y de la de Uppsala, en Suecia, dieron a conocer que la nueva especie logró evolucionar en tan solo dos generaciones, aun cuando se estimaba que demoraría mucho más, a causa de la reproducción entre un pinzón de Darwin endémico, Geospiza fortes, y un pinzón de cactus migrante, Geospiza conirostris. |
| whisper-base | 14 | Experts de la Universidad de Princeton en Estados Unidos y de la Uppsala en Suecia dieron a conocer que la nueva especie logró evolucionar en tan solo dos generaciones, aun cuando se estimaba que demoraría mucho más, a causa de la reproducción entre un pinzón  […] |
| whisper-small | 11 | Expertos de la Universidad de Princeton en Estados Unidos y de la Uppsala en Suecia dieron a conocer que la nueva especie logró evolucionar en tan solo dos generaciones, aún cuando se estimaba que demoraría mucho más, a causa de la reproducción entre un pinzón […] |
| qwen3-asr-0.6b | 6 | Expertos de la Universidad de Princeton en Estados Unidos y de la Uppsala en Suecia dieron a conocer que la nueva especie logró evolucionar en tan solo dos generaciones, aun cuando se estimaba que demoraría mucho más. A causa de la reproducción entre un pinzón […] |
| qwen3-asr-1.7b | 4 | Expertos de la Universidad de Princeton en Estados Unidos y de la Uppsala en Suecia dieron a conocer que la nueva especie logró evolucionar en tan solo dos generaciones, aun cuando se estimaba que demoraría mucho más, a causa de la reproducción entre un pinzón […] |
| mega-asr-lora | 4 | Expertos de la universidad de princeton en estados unidos y de la uppsala en suecia dieron a conocer que la nueva especie logró evolucionar en tan solo dos generaciones, aun cuando se estimaba que demoraría mucho más a causa de la reproducción entre un pinzón  […] |

#### reunion / u27 — Mega-ASR mejor que 1.7B base

| Modelo | Errores | Transcripción |
|---|---|---|
| **Referencia** | | Con la construcción de la presa en 1963, se frenaron las inundaciones estacionales que dejaban sedimentos a lo largo del río. |
| whisper-base | 8 | Con la construcción de la presa en 1963 se frenaron las inundaciones especiales que dejaban en ese momento hasta el largo orden. |
| whisper-small | 2 | Con la construcción de la presa en 1963 se frenaron las inundaciones estacionales que dejaban sedimentos a lo largo de diez. |
| qwen3-asr-0.6b | 0 | Con la construcción de la presa en mil novecientos sesenta y tres se frenaron las inundaciones estacionales que dejaban sedimentos a lo largo del río. |
| qwen3-asr-1.7b | 2 | Con la construcción de la presa en 1963, se frenaron las inundaciones estacionales que dejaban sedimentos al largo del río. |
| mega-asr-lora | 0 | Con la construcción de la presa en mil novecientos sesenta y tres, se frenaron las inundaciones estacionales que dejaban sedimentos a lo largo del río. |

#### reunion / u43 — Mega-ASR peor que 1.7B base

| Modelo | Errores | Transcripción |
|---|---|---|
| **Referencia** | | Indica que la posición más efectiva donde colocar el objeto central es la intersección que se forma entre las líneas horizontales y verticales que dividen la imagen en tercios (ver ejemplo). |
| whisper-base | 4 | Indica que la posición más efectiva donde colocar el objeto central es la intersección que se forma entre las líneas horizontales y verticales que evidente al imagen en la tercera. |
| whisper-small | 1 | Indica que la posición más efectiva donde colocar el objeto central es la intersección que se forma entre las líneas horizontales y verticales que dividen la imagen en próspectas. |
| qwen3-asr-0.6b | 3 | Indica que la posición más efectiva donde colocar el objeto central es la intersección que se forma entre las líneas horizontales y verticales que dividen la imagen en partes. Ver ejemplo. |
| qwen3-asr-1.7b | 1 | Indica que la posición más efectiva donde colocar el objeto central es la intersección que se forma entre las líneas horizontales y verticales que dividen la imagen en perspectiva. |
| mega-asr-lora | 4 | Indica que la posición más efectiva donde colocar el objeto central es la intersección que se forma entre las líneas horizontales y verticales que dividen a la imagen en tercia. Ver ejemplo. |

#### reunion / u19 — Whisper base vs Qwen 0.6B

| Modelo | Errores | Transcripción |
|---|---|---|
| **Referencia** | | Expertos de la Universidad de Princeton, en EE. UU., y de la de Uppsala, en Suecia, dieron a conocer que la nueva especie logró evolucionar en tan solo dos generaciones, aun cuando se estimaba que demoraría mucho más, a causa de la reproducción entre un pinzón de Darwin endémico, Geospiza fortes, y un pinzón de cactus migrante, Geospiza conirostris. |
| whisper-base | 40 | Expertos de la Universidad de Princeton en Estados Unidos y de la Uxana de FESIA Quiero reconocer que la nueva FES se logró evolucionar en las nuevas generaciones. Aún para los estomados que llamarían mucho más, aconse a las reproducciones de impienzo de árbub […] |
| whisper-small | 28 | Expertos de la Universidad de Princeton, en Estados Unidos y de la Uppsala de Especia vieron a conocer que la nueva especie logró evolucionar entre los dos generaciones, aún cuando se estomaba que demoraría mucho más. A causa de las reproducciones de un piensó […] |
| qwen3-asr-0.6b | 24 | Expertos de la Universidad de Princeton en Estados Unidos y de la Upsala de Suecia vieron a conocer que la nueva especie logró evolucionar en tan solo dos generaciones, aunque se estimaba que demandaría mucho más. Acá vemos la reproducción en Princeton de Darw […] |
| qwen3-asr-1.7b | 16 | Expertos de la Universidad de Princeton en Estados Unidos y de la Uppsala en Suecia dieron a conocer que la nueva especie logró evolucionar en tan solo dos generaciones, aun cuando se estimaba que demoraría mucho más. A causa de la reproducción entre pinzones  […] |
| mega-asr-lora | 19 | expertos de la universidad de princeton en estados unidos y de la uppsala de suecia dieron a conocer que la nueva especie logró evolucionar en tan solo dos generaciones, aun cuando se estimaba que demoraría mucho más a causa de la reproducción en contención de […] |

## Limitaciones

- **Muestra pequeña:** 44 frases por condición (1 165 palabras de referencia), todas de un mismo corpus de *lectura* (FLEURS). Son frases de Wikipedia con muchos nombres propios, lejos del habla espontánea de una clase o reunión: sin disfluencias, sin solapamientos entre hablantes, sin frases largas. Los IC95 en limpio y reunión se solapan entre los Qwen y Mega. Solo las diferencias grandes (Qwen frente a Whisper base, y Mega frente a la base en aula) son robustas.
- **Degradaciones sintéticas:** la RIR es ruido con decaimiento exponencial y el babble sale de otras frases de FLEURS (otros hablantes leyendo). Opus va a 24 kbps sin pérdida de paquetes. Mega-ASR se entrenó con simulaciones acústicas "reales" de otro tipo, y en grabaciones reales de Meet/Zoom podría rendir distinto. Aun así, el router no reconoce el escenario "reunión" como degradado.
- **El "aula lejana"** (DRR ≈ −20 dB, SNR 5 dB) es extrema: ningún modelo baja del 40 %. Representa más bien el límite de lo que se puede rescatar.
- **Solo CPU, bf16 para Qwen y Mega, int8 para Whisper.** La app usa Whisper base en ONNX. Con faster-whisper int8 y beam 5, la calidad debería ser similar o algo mejor que un ONNX greedy, así que, si acaso, aquí se favorece a Whisper. En aula, las salidas vacías de Whisper vienen en parte de su umbral de no-voz y de compresión, con la configuración por defecto.
- **Clips cortos (≤ 21 s).** No se evaluó audio largo con chunking/VAD, que es el caso real de la app. Los bucles de repetición de Mega-ASR y las alucinaciones de Whisper suelen empeorar con audio largo.
- **Tiempos contaminados** por otras cargas de la máquina (ver arriba).
- Mega-ASR se evaluó en transformers con un parche de memoria que aplica la LoRA de una vez. Su salida se verificó idéntica a la del wrapper oficial en 6 clips. No se probó su ruta vLLM, que en CPU no aplica.

## Recomendación para la app de notas

1. **No adoptar Mega-ASR para español.** No aporta nada sobre Qwen3-ASR-1.7B en limpio ni en reunión (el router ni siquiera activa la LoRA). En reverberación empeora el total del corpus y produce bucles de repetición. Además pierde formato (números con letras, sin mayúsculas ni puntuación) y el wrapper oficial necesita ~11.5 GB de RAM para el modo router.
2. **Pasar de Whisper base a Qwen3-ASR-0.6B** es la mejora más clara por coste:
   - WER relativo: −50 % en limpio, −63 % en reunión, y aula utilizable (38 % frente a 95 %).
   - Coste: ~2.9 GB de RAM, ~2 GB de disco y ~3× más cómputo que Whisper base, aunque sigue unas 3 veces más rápido que el tiempo real en 4 núcleos.
   - Supera también a Whisper small en las cuatro condiciones.
3. **Qwen3-ASR-1.7B** solo compensa si la app apunta a máquinas con ≥ 8 GB libres y quiere el máximo en aulas o salas con reverberación (−10 pp frente al 0.6B en aula). En limpio y en reunión la ganancia es de ~1 pp.
4. Antes de decidir, conviene validar con 10–20 minutos de grabaciones *reales* de la app (Meet/Zoom/aula) transcritas a mano, con chunking y VAD como en producción.

## Archivos

- `informe.md`: este informe.
- `results.json`: WER por modelo × condición (las tres normalizaciones), IC, comparaciones pareadas, errores por frase, decisiones del router, RTF y RAM, y todas las hipótesis y referencias.
- `scripts/`: preparación de datos, ejecución y scoring (pequeños, para reproducir).
