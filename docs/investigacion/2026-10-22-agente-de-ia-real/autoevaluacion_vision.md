# Autoevaluación con visión y refinamiento iterativo para el agente del lienzo de PixPin

Alcance: qué métodos de autocrítica y refinamiento mejoran de verdad los resultados, cuándo fallan, y cómo diseñar el evaluador, la rúbrica, los criterios de parada y el presupuesto. Aplicado a un agente que dibuja en un lienzo tipo Excalidraw, puede renderizarlo (`ver_lienzo`) y usa Claude Haiku, con Sonnet cuando haga falta.

## 1. Self-Refine, Reflexion, CRITIC, LATS y evaluador-optimizador: ganancias medidas, fallos y cuándo hace falta feedback externo

### Takeaway
El refinamiento iterativo ayuda mucho en tareas abiertas y de varios aspectos: estilo, restricciones como "incluye todos los conceptos" o legibilidad. En razonamiento casi no ayuda, o empeora, si el modelo se juzga solo, sin una señal externa (herramienta, test, verificador). Lo que más pesa es que el feedback sea concreto y esté anclado en una verificación externa, más que el número de vueltas. La mayor parte de la ganancia llega en las 1–3 primeras rondas.

### Cited Findings
- **Self-Refine.** Mejora ~20 puntos absolutos de media en 7 tareas frente a la generación en un solo paso con el mismo LLM (GPT-3.5, ChatGPT, GPT-4) — [Self-Refine, arXiv 2303.17651](https://arxiv.org/abs/2303.17651)
- **Self-Refine con GPT-4, por tarea (base → refinado).** Dialogue Response 25.4→74.6; Constrained Generation 15.0→45.0; Code Readability 27.4→56.2; Acronym 30.4→56.0; Code Optimization 27.3→36.0; **Math Reasoning 92.9→93.1 (+0.2)**. Con feedback de oráculo, GPT-4 solo gana +0.7 en matemáticas — [Self-Refine HTML](https://arxiv.org/html/2303.17651)
- **Rendimientos decrecientes en Self-Refine (media de 3 modelos).** Constrained Generation 29.0→40.3→46.7→49.7 (+11.3, +6.4, +3.0); Code Optimization 22.0→27.0→27.9→28.8 (+5.0, +0.9, +0.9). En tareas de varios aspectos, como Acronym, la mejora no es monótona. Máximo de 4 iteraciones, o parar con un indicador de parada que se extrae del propio feedback — [Self-Refine HTML](https://arxiv.org/html/2303.17651)
- **Feedback específico frente a genérico (ablación de Self-Refine).** Sentiment Reversal: 43.2 con feedback específico, 31.2 con genérico y 0 sin feedback. Code Optimization: 27.5 / 26.0 / 24.8 — [Self-Refine HTML](https://arxiv.org/html/2303.17651)
- **Modelos débiles.** Vicuna-13B no lograba producir feedback en el formato pedido: repetía su salida o alucinaba. En un esquema mixto (Vicuna genera, ChatGPT critica y refina) Math pasó de 24.18% a 40.5%. Es decir, un crítico más fuerte rescata a un generador débil — [Self-Refine HTML](https://arxiv.org/html/2303.17651)
- **Huang et al., autocorrección intrínseca sin etiquetas.** Empeora el razonamiento:
  - GPT-4: GSM8K 95.5→91.5→89.0; HotpotQA 49.0→49.0→43.0.
  - GPT-3.5: CommonSenseQA 75.8→38.1.
  - Llama-2: GSM8K 62.0→43.5→36.5.

  Con etiquetas de oráculo sí "mejora" (GPT-3.5 GSM8K 75.9→84.3), pero los autores dicen que eso mide al oráculo, no la autocorrección — [Huang et al. 2023, arXiv 2310.01798 (HTML)](https://arxiv.org/html/2310.01798v2)
- **Huang et al., cambios de respuesta.** De las respuestas que cambia, GPT-3.5 convierte más correctas en incorrectas que al revés — [Huang et al. HTML](https://arxiv.org/html/2310.01798v2)
- **Huang et al., el prompt inicial puede inflar la ganancia.** En CommonGen-Hard, el prompt original de Self-Refine daba 44.0 (67.0 tras autocorregir). Con un prompt inicial mejorado que exige explícitamente "TODOS" los conceptos, la primera respuesta ya da 81.8, y autocorregir encima la *baja* a 75.1. Parte de la ganancia atribuida al refinamiento era un prompt inicial mal especificado — [Huang et al. HTML](https://arxiv.org/html/2310.01798v2)
- **Huang et al., coste igualado.** En GSM8K, el debate multiagente (83.0–83.2) rinde menos que la autoconsistencia por mayoría con el mismo número de respuestas (85.3 con 6, 88.2 con 9). Recomiendan usar feedback externo (ejecución de código, herramientas, verificadores) y comparar siempre contra líneas base de igual coste — [Huang et al. HTML](https://arxiv.org/html/2310.01798v2)
- **Kamoi et al. 2024 (revisión crítica).** No hay trabajos previos que muestren autocorrección exitosa solo con feedback de LLMs con prompt, salvo en tareas excepcionalmente adecuadas. Funciona con feedback externo fiable o con fine-tuning a gran escala. Muchos estudios sobrevaloran la autocorrección — [Kamoi et al., arXiv 2406.01297](https://arxiv.org/abs/2406.01297)
- **CRITIC (crítica verificada con herramientas, ChatGPT).**
  - QA, en EM/F1: HotpotQA 40.3/52.9 con herramientas frente a 33.1/46.1 sin ellas (CoT 32.7/42.8); AmbigNQ 62.0/74.9 frente a 55.2/67.3.
  - Matemáticas: SVAMP +1.3 con herramienta y **+0.0 sin herramienta**.
  - Toxicidad: la probabilidad baja de 0.192 a 0.040 con herramienta y *sube* a 0.223 sin ella.
  - Límite de 3–4 correcciones; 2–3 rondas capturan la mayor parte del beneficio.

  [CRITIC, arXiv 2305.11738 (HTML)](https://arxiv.org/html/2305.11738)
- **Reflexion.** HumanEval pass@1 de 91.0 con GPT-4 frente a 80.1 de base, usando tests unitarios autogenerados (máximo 6 por problema) como señal externa. Una réplica independiente obtuvo 89.4 frente a 81.7 — [Reflexion, arXiv 2303.11366](https://arxiv.org/pdf/2303.11366); réplica citada en [MAR, arXiv 2512.20845](https://arxiv.org/pdf/2512.20845)
- **LATS (búsqueda en árbol con reflexión).** 92.7% en HumanEval con GPT-4, según fuentes secundarias. No verifiqué la tabla original — [blog beancount sobre LATS](https://beancount.io/zh/bean-labs/research-logs/2026/05/10/lats-language-agent-tree-search-reasoning-acting-planning)
- **Evaluador-optimizador de Anthropic.** Un LLM genera y otro evalúa y da feedback en bucle. Encaja cuando (1) hay criterios de evaluación claros, (2) el feedback humano mejora demostrablemente la respuesta y (3) el LLM puede dar ese tipo de feedback. Los agentes deben obtener "ground truth" del entorno en cada paso (resultados de herramientas, ejecución) e incluir condiciones de parada como un máximo de iteraciones — [Anthropic, Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)

### Inferences
- Para PixPin, la parte "objetiva" del rubro (solapes, texto que se sale, ortografía y acentos, que estén todos los elementos pedidos) debe verificarse con código o herramientas, al estilo CRITIC. La visión del modelo queda para lo que no se puede medir: coherencia con el tema y aspecto general.
- Antes de añadir rondas, conviene mejorar el prompt inicial: lista explícita de lo pedido y reglas de maquetación. Huang et al. muestran que eso a veces da más que el refinamiento.
- LATS y la búsqueda en árbol multiplican las llamadas. No encajan con un presupuesto de créditos en un editor interactivo.

### Gaps
- No encontré números de LATS en la fuente primaria (arXiv 2310.04406), ni su coste en tokens.
- No hay estudios de Self-Refine o CRITIC hechos con Claude Haiku. Los datos son de GPT-3.5/4 y Llama, de 2023.

## 2. VLM como juez y autoverificación visual de gráficos, diagramas, UI, SVG y diapositivas

### Takeaway
El bucle "renderizar → mirar → criticar → corregir" funciona con modelos frontera, y hay datos directos con Claude Haiku 4.5: +5.7% tras 3 ciclos y +9.8% quedándose con la mejor versión, usando Sonnet 4.5 como crítico visual. En modelos pequeños el refinamiento visual suele empeorar la validez de la salida. El punto débil común es traducir un defecto visto en la imagen a una edición precisa del código o las coordenadas. Una rúbrica multidimensional con bandas definidas coincide mucho más con las personas que una puntuación única.

### Cited Findings
- **CITL (refinamiento de código frontend guiado por visión).** Generadores DeepSeek-R1-Distill-Qwen-14B, **Claude Haiku 4.5** y Claude Sonnet 4.5. Crítico visual fijo: **Sonnet 4.5**. Presupuesto fijo de T=3 ciclos — [Vision-Guided Iterative Refinement, arXiv 2604.05839](https://arxiv.org/html/2604.05839)
- **CITL, ganancia acumulada sobre el código inicial.**

  | Generador | Inicial | Ciclo 1 | Ciclo 2 | Ciclo 3 | Mejor de los ciclos |
  |---|---|---|---|---|---|
  | Haiku 4.5 | 7.436 | +3.6% | +5.2% | +5.7% | 8.160 (+9.8%) |
  | Sonnet 4.5 | 7.584 | +4.9% | +5.8% | +6.1% | +10.8% |
  | Distill-Qwen-14B | 5.236 | +6.2% | +7.8% | +8.7% | +17.8% |

  Quedarse con la mejor versión casi duplica la ganancia de quedarse con la última — [arXiv 2604.05839](https://arxiv.org/html/2604.05839)
- **CITL, otros resultados.**
  - Refinar *sin* crítico da solo +1.5% (Qwen-14B).
  - En el 86% de las tareas la mejor solución sale del ciclo 1 o posterior (configuración con Sonnet).
  - La calidad del código *baja* en cada ciclo con Qwen-14B (−1.7%, −2.8%, −3.2%): hay regresiones si no se guarda la mejor versión.
  - Tokens: 2,571 la salida inicial, 45,808 acumulados en el ciclo 3, 23,662 hasta la mejor versión.

  [arXiv 2604.05839](https://arxiv.org/html/2604.05839)
- **CITL, rúbrica del juez.** 4 dimensiones de 1 a 10: tarea visual y estética (consulta + captura), tarea en código y calidad del código (consulta + código). Bandas: 1–2 fallos graves, 3–4 carencias importantes, 5–6 problemas menores, 7–8 faltan detalles pequeños, 9–10 cumplido del todo. Acuerdo con preferencias humanas: **69.5% multidimensional frente a 48.5% con una sola dimensión** (200 tareas) — [arXiv 2604.05839](https://arxiv.org/html/2604.05839)
- **Advertencia con VLMs pequeños (SVG, Qwen3.5 9B/27B).** El refinamiento iterativo con feedback visual tuvo un efecto inconsistente en la calidad y a menudo *bajó* la tasa de compilación. Text→SVG sin restricción: 0.837 en un solo turno frente a 0.475 (no estructurado) y 0.180 (semiestructurado). Los tokens suben de 1.4k a 4.8k–10.7k. La variante "estructurada" fue la que menos degradó (0.785) y la que menos tokens usó. Los autores lo atribuyen a que los modelos pequeños no logran ligar el problema visual con el código — [Constrained Iterative Refinement for SVG, arXiv 2608.28678](https://arxiv.org/html/2608.28678)
- **Figuras científicas como programas.** El crítico de render ve el fragmento SVG *y* su render, porque algunos defectos, como el texto desbordado, solo se ven renderizados — [Figures as Programs, arXiv 2609.01006](https://arxiv.org/pdf/2609.01006)
- **Otras líneas de 2026 con bucle de render.** Se apoyan en entrenamiento (DPO/RL), no solo en prompts — [IntroSVG, arXiv 2603.09312](https://arxiv.org/abs/2603.09312); [RefineSVG, arXiv 2607.27699](https://arxiv.org/abs/2607.27699v1); [Render-in-the-Loop, arXiv 2604.20730](https://arxiv.org/html/2604.20730v3); [VFLM, maquetación de texto, arXiv 2603.22187](https://arxiv.org/html/2603.22187v1)
  - IntroSVG: un solo VLM hace de generador y de crítico.
  - RefineSVG: devuelve al modelo un mapa de diferencias como señal.
- **Agent starter kit de tldraw.** El agente recibe una *captura de su vista* más los datos estructurados de las formas, en tres formatos simplificados:
  - BlurryShape: límites, id, tipo y texto de lo que hay en la vista.
  - FocusedShape: casi todas las propiedades; es también el formato con que el agente crea formas.
  - PeripheralShapeCluster: lo que está fuera de pantalla, agrupado.

  Las coordenadas se envían relativas y redondeadas. La revisión es un modo programable (`schedule()`, `interrupt()` en modo `reviewing`, "Review the new area thoroughly for any mistakes"). Además hay **canvas lints** que señalan problemas de las formas y entran en la entrada del agente, y `sanitizeAction()` corrige errores como ids inválidos. Recomiendan Anthropic — [tldraw Agent starter kit](https://tldraw.dev/starter-kits/agent)
- **tldraw sobre la revisión visual.** Los agentes "no pueden señalar". Para revisar, el agente hacía captura, dejaba comentarios anclados a elementos, cambiaba el código y volvía a capturar — [tldraw blog, Agents can't point](https://tldraw.dev/blog/agents-cant-point)
- **Límites de visión que documenta Anthropic.** Las coordenadas y la localización de Claude son aproximadas, conviene verificarlas. El conteo es aproximado con muchos objetos pequeños. Hay errores con imágenes pequeñas (<200 px) o de baja calidad. La compresión JPEG fuerte hace ilegible el texto. Mejor imagen antes que texto en el prompt — [Claude Vision docs](https://platform.claude.com/docs/en/build-with-claude/vision)

### Inferences
- **Diseño recomendado para PixPin:**
  - Haiku genera.
  - Las comprobaciones deterministas corren primero, como los "lints" de tldraw.
  - El juez visual recibe la captura *y* la lista estructurada de formas, con ids, bbox y texto, para que pueda nombrar el elemento exacto a corregir en lugar de "señalar" en la imagen.
  - Con todo, que pida correcciones por id y no por coordenadas leídas de la imagen.
- **Crítico visual.** El CITL usó Sonnet como crítico de Haiku y logró +9.8%. Una opción razonable: Haiku como juez barato por defecto y Sonnet solo si Haiku suspende dos veces o si la puntuación de coherencia es dudosa. No encontré datos de Haiku criticándose a sí mismo con visión.
- **No dar por hecho que Haiku localiza bien los solapes en la imagen.** Los solapes y la alineación deben salir de la geometría del modelo de escena, no de la visión.

### Gaps
- El CITL no nombra el modelo juez ni da costes en dólares.
- No encontré cifras publicadas de tldraw "make real" ni del agent kit sobre cuánto mejora la revisión.
- No localicé números de Design2Code en esta búsqueda.

## 3. Diseño de rúbrica, escalas, crítico distinto o el mismo, número de rondas y prevención de sobreedición y regresiones

### Takeaway
Conviene una rúbrica multidimensional con bandas descritas, feedback concreto y accionable, y un máximo de 2–3 rondas. Hay que guardar siempre la mejor versión y aceptar un cambio solo si mejora la puntuación sin romper las comprobaciones duras. Un juez que evalúa su propia salida tiende a favorecerla, así que es preferible un crítico distinto, o al menos un prompt de crítico separado sin el razonamiento del generador.

### Cited Findings
- La rúbrica multidimensional (4 ejes de 1 a 10 con bandas) coincide con humanos el 69.5% de las veces, frente al 48.5% de una dimensión — [arXiv 2604.05839](https://arxiv.org/html/2604.05839)
- El feedback específico supera al genérico (43.2 frente a 31.2 en Sentiment Reversal) — [Self-Refine HTML](https://arxiv.org/html/2303.17651)
- Ganancias por ronda decrecientes: Self-Refine +11.3, +6.4, +3.0 — [Self-Refine HTML](https://arxiv.org/html/2303.17651). CRITIC: 2–3 rondas capturan casi todo — [CRITIC HTML](https://arxiv.org/html/2305.11738). CITL con Haiku: +3.6, +1.6, +0.5 puntos porcentuales por ciclo — [arXiv 2604.05839](https://arxiv.org/html/2604.05839)
- Guardar la mejor versión funciona: en CITL la mejor de los ciclos da +9.8% frente a +5.7% del último ciclo (Haiku). Además evita la caída de calidad del código, −3.2% en el ciclo 3 con Qwen-14B — [arXiv 2604.05839](https://arxiv.org/html/2604.05839)
- Huang et al.: GPT-3.5 cambia más respuestas correctas a incorrectas que al revés. La autocorrección sin señal externa destruye respuestas buenas, que es justo la sobreedición — [Huang et al. HTML](https://arxiv.org/html/2310.01798v2)
- **Sesgo de autopreferencia.** Los LLM reconocen sus propias salidas con precisión "no trivial" y las puntúan más alto aunque los humanos las juzguen iguales. Tras fine-tuning, la autorreconocimiento y la autopreferencia correlacionan linealmente — [Panickssery et al., arXiv 2404.13076](https://arxiv.org/abs/2404.13076)
- **Por pares o puntuación absoluta.** Un estudio de 2025 halla que las preferencias por pares se invierten en ~35% de los casos ante distractores, frente a ~9% de las puntuaciones absolutas — [arXiv 2504.14716 (vía fugumt)](https://fugumt.com/fugumt/paper_check/2504.14716v2_enmode). El trabajo UDA señala sesgo de preferencia, incluida la autopreferencia, en el juicio por pares. Con GPT-4o, por pares 90.5% frente a puntual 88.0%; los modelos pequeños van en sentido contrario — [UDA, arXiv 2508.09724](https://arxiv.org/html/2508.09724v1)
- **Crítico más fuerte que el generador.** Rescata salidas: Vicuna generando con ChatGPT criticando pasó de 24.18% a 40.5% — [Self-Refine HTML](https://arxiv.org/html/2303.17651)

### Inferences
- **Rúbrica propuesta para PixPin.** Cada eje de 0 a 10 con bandas como las de CITL y salida en JSON:
  - (a) completitud frente a la petición, con una lista de requisitos extraída al inicio, cada uno sí/no;
  - (b) ortografía y acentos en español;
  - (c) orden, alineación, sin solapes ni texto desbordado;
  - (d) coherencia lógica con el tema;
  - (e) legibilidad (contraste, tamaño).

  Los ejes (b) y (c) los puntúa sobre todo el código, y el VLM solo los complementa.
- **Regla de aceptación.** Cada versión se puntúa. Se acepta la nueva solo si (1) no rompe ninguna comprobación dura que antes pasaba y (2) su puntuación total ≥ la mejor + un margen pequeño. Si no, se revierte a la mejor. Paradas:
  - todas las comprobaciones duras pasan y cada eje ≥ 8;
  - o mejora < 0.3 puntos;
  - o se llega a 2 rondas con Haiku (3 como máximo absoluto).
- **Por pares o absoluta.** Para "¿es mejor la v2 que la v1?" se puede usar una comparación por pares, presentando las dos en orden aleatorio. Para la parada, una puntuación absoluta por ejes, que es más estable ante distractores.
- **Contra la sobreedición.** El crítico debe devolver una lista acotada (≤5) de problemas con el id del elemento y la acción concreta. El generador solo toca esos elementos, en un diff, no rehace todo.

### Gaps
- No encontré un estudio que compare directamente Haiku criticando a Haiku frente a Sonnet criticando a Haiku con imágenes.
- La evidencia sobre puntuación por pares frente a absoluta es mixta y en parte de blogs.
- No encontré estudios específicos sobre escalas binarias o checklists frente a escalas Likert en jueces VLM.

## 4. Comprobaciones deterministas combinadas con el juez VLM ("verificadores programáticos primero")

### Takeaway
Toda la evidencia (CRITIC, Reflexion, Huang, Kamoi, Anthropic) apunta a que la señal externa es lo que hace funcionar la autocorrección. En un lienzo con modelo de escena se pueden calcular de forma exacta y gratis los solapes, el texto desbordado, la alineación, la ortografía y el idioma. El VLM queda para lo que no se puede medir.

### Cited Findings
- **CRITIC.** Sin herramientas, la ganancia se reduce o desaparece (SVAMP +0.0; la toxicidad empeora de 0.192 a 0.223). Con herramientas, la toxicidad baja a 0.040 y HotpotQA F1 sube de 46.1 a 52.9 — [CRITIC HTML](https://arxiv.org/html/2305.11738)
- **Reflexion.** Usa la ejecución de tests (señal externa) para su 91% en HumanEval — [Reflexion](https://arxiv.org/pdf/2303.11366)
- **tldraw.** Mete "canvas lints" (problemas detectados en las formas) en la entrada del agente y sanea sus acciones antes de aplicarlas — [tldraw Agent starter kit](https://tldraw.dev/starter-kits/agent)
- **Figures as Programs.** El texto desbordado se ve en el render, no en el código. El crítico recibe las dos cosas — [arXiv 2609.01006](https://arxiv.org/pdf/2609.01006)
- **Anthropic.** Los agentes necesitan "ground truth" del entorno en cada paso — [Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)
- **Vision de Claude.** Localización aproximada y conteo impreciso. No conviene fiar a la imagen las comprobaciones geométricas — [Claude Vision docs](https://platform.claude.com/docs/en/build-with-claude/vision)

### Inferences
- **Comprobaciones duras recomendadas, en Rust, antes de cualquier llamada al VLM:**
  - intersección de bbox entre formas que no sean contenedor/contenido;
  - texto cuyo ancho medido supera el de su forma o del lienzo visible;
  - elementos fuera de la vista;
  - alineación: desviación de bordes o centros respecto a una rejilla o a sus vecinos por debajo de N px;
  - flechas cuyo extremo no toca ninguna forma;
  - ortografía y acentos en español con un diccionario Hunspell es_ES o es_419 o similar;
  - detección de idioma del texto;
  - completitud: cada requisito extraído de la petición debe tener al menos una forma o etiqueta que lo cubra (correspondencia léxica).
- **Uso del resultado.** El informe de estas comprobaciones se pasa al generador como feedback concreto, con ids. Si todo pasa, se llama al VLM solo para la coherencia y el aspecto general. Así muchas tareas terminan sin ninguna llamada de visión.

### Gaps
- No encontré mediciones publicadas de cuánto mejora un juez VLM al recibir la salida de lints junto a la imagen. Es una inferencia basada en CRITIC y en tldraw.

## 5. Control de coste y latencia

### Takeaway
El refinamiento multiplica los tokens: ×9 a ×18 en CITL hasta el ciclo 3, y ×3 a ×6 en SVG. Las palancas principales son:
- comprobaciones deterministas primero;
- parada temprana;
- quedarse con la mejor versión, que en CITL cuesta la mitad de tokens que llegar al ciclo 3;
- capturas reducidas;
- escalar a Sonnet solo cuando hace falta.

### Cited Findings
- **Tokens en CITL.** 2,571 en la salida inicial, 45,808 acumulados en el ciclo 3, 23,662 hasta la mejor solución — [arXiv 2604.05839](https://arxiv.org/html/2604.05839)
- **Tokens en SVG.** Refinar cuesta 4.8k–10.7k tokens frente a 1.4k–1.7k de un solo turno. La variante estructurada es la más barata (2.4k–3.3k) — [arXiv 2608.28678](https://arxiv.org/html/2608.28678)
- **Coste de una imagen en Claude.** ⌈ancho/28⌉ × ⌈alto/28⌉ tokens visuales. 1000×1000 son 1,296 tokens. En el nivel estándar (Haiku y otros), una imagen de 1920×1080 se reduce a 1456×819 y cuesta 1,560 tokens. Con Haiku 4.5, a 1 USD por millón de tokens de entrada, una imagen de 1000×1000 cuesta ~1.30 USD por cada mil imágenes. Las imágenes base64 se reenvían en cada turno, así que conviene la Files API o no acumular capturas viejas en el historial — [Claude Vision docs](https://platform.claude.com/docs/en/build-with-claude/vision)
- **Huang et al.** Recomiendan comparar el refinamiento con líneas base de igual coste, como la autoconsistencia, e informar del coste — [Huang et al. HTML](https://arxiv.org/html/2310.01798v2)
- **Anthropic.** Recomienda un máximo de iteraciones como condición de parada — [Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)
- **Destilación en CITL.** Un estudiante LoRA recupera el 24.7% de la mejor ganancia con 2,583 tokens, frente a los 23,662 de la mejor versión — [arXiv 2604.05839](https://arxiv.org/html/2604.05839)

### Inferences
- **Presupuesto concreto por petición:**
  - Ronda 0: generación y lints, sin coste de visión.
  - Ronda 1: si los lints pasan, una llamada a Haiku con una captura de ~1000 px de lado largo (~1.3k tokens) para la rúbrica.
  - Ronda 2: corrección solo si algún eje < 8.
  - Escalar a Sonnet como crítico solo si Haiku no supera la parada en 2 rondas o si la petición es compleja (muchos elementos).
  - Tope absoluto de tokens o dinero por petición, y en cada turno quitar del historial las capturas anteriores menos la mejor.
- **Capturas.** Recortar al área tocada, como el `reviewing` con límites de tldraw. Sale más barato y se lee mejor que el lienzo entero. El texto debe seguir legible: evitar JPEG fuerte (ver la guía de Claude).
- **Latencia.** Los lints son instantáneos. La llamada de visión añade un viaje de ida y vuelta. Se puede mostrar el resultado ya y aplicar las correcciones después, mostrando el progreso.

### Gaps
- No encontré mediciones de latencia por ronda con Haiku ni con Sonnet en bucles de visión.
- No encontré datos publicados del coste en dólares de los pipelines CITL o tldraw.
