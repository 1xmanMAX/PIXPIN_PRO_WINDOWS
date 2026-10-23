# Un asistente que sabe usar PixPin y revisa su trabajo

*23-oct-2026 · rama `ccr-729cc4a5-rc7s9p` · une y termina `respaldo/agente-{prompt,herramientas,memoria,revision}`*

## El pedido

> «No le digas lo que puede hacer: enséñale a usar la app y qué herramientas tiene, de forma que
> cuando le pida algo no diga que no sabe hacer eso, que pueda hacerlo y bien, y que supervise su
> propio trabajo.»

Tres fallos concretos detrás de eso:

1. **Se rinde**: «no tengo una herramienta para traducir la figura», cuando sí puede (leer los
   textos, traducirlos él y reescribirlos con `editar_texto`).
2. **Lo hace mal**: textos encimados o que se salen de su caja, flechas sueltas, partes del pedido
   sin hacer.
3. **No lo revisa**: contesta «hecho» sin mirar el resultado ni comprobar que cubrió todo.

## Lo que dice la investigación

| Idea | Fuente | Qué se toma |
|---|---|---|
| El sistema a la «altura justa»: ni una lista de si-entonces (frágil) ni algo vago. Empezar mínimo y añadir según los fallos observados. | Anthropic, [Effective context engineering for AI agents](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents) | Principios + método + mapa del entorno, no «si pide X usa Y». |
| Pocos ejemplos **canónicos y variados** en vez de una regla por caso. | Ídem | Cuatro ejemplos distintos en las instrucciones. |
| Describir cada herramienta «como a alguien nuevo en el equipo»: el contexto implícito, la jerga y **las relaciones entre recursos**. Pequeños cambios en las descripciones dan mejoras grandes. | Anthropic, [Writing effective tools for agents](https://www.anthropic.com/engineering/writing-tools-for-agents) | El «cuándo usarla» va en la descripción de cada herramienta; el mapa de la app explica cómo se relacionan los sitios (#id, «Lienzo destino», adjuntos). |
| Errores que dicen qué arreglar, no códigos. | Ídem | Los `ERROR:` de las herramientas y el lint ya nombran el `#id` y el arreglo. |
| Cada prueba con un resultado verificable; tareas realistas de varios pasos; no castigar diferencias de forma. | Ídem | El banco de pedidos (abajo). |
| **Que el mismo modelo relea su borrador puede empeorar** el resultado (muchos falsos negativos del verificador). | [ICLR 2025](https://proceedings.iclr.cc/paper_files/paper/2025/file/f3c5e56274140e0420baa3916c529210-Paper-Conference.pdf) | El repaso no es «¿está bien?» en el aire: se ancla en los resultados de las herramientas, los `ERROR` y el lint. |
| Un monitor que verifica contra la salida estructurada de las herramientas y da directivas de corrección (no bloqueos) sube la recuperación y baja la invención. | [When Tools Fail, Agents Fabricate (NeurIPS 2026)](https://neurips.cc/virtual/2026/173038) | El lint va pegado a cada resultado; el repaso corrige, no bloquea. |
| Comprobar si una acción ya se hizo antes de reintentarla mejora la fiabilidad sin tocar el modelo. | [Verified Tool Calls (arXiv 2608.02645)](https://arxiv.org/pdf/2608.02645) | La revisión guarda versiones y vuelve a la mejor en vez de rehacer. |
| Quedarse con la **mejor** versión casi duplica la ganancia de la autocorrección; del ciclo 2 al 3 apenas mejora. | CITL, arXiv 2604.05839 (citado en `respaldo/agente-revision`) | Dos rondas de serie, con copia y vuelta atrás. |
| El fallo más medido hoy es **preguntar de menos**; pero preguntar de más también cuesta. | CLAM, AskBench (citado en `respaldo/agente-prompt`) | `preguntar_usuario` estrecha: una pregunta, 2-4 opciones, solo si equivocarse cuesta. |

## El diagnóstico en PixPin

- **La nube ya ve todas las herramientas** (`turno.rs`: el catálogo entero, ~90, porque eligen bien
  y el prefijo se cachea). El «no tengo herramienta» de la nube **no** era que le faltaran: era no
  saber cómo se relacionan los sitios de la app y no ver la combinación. Al modelo local sí se le dan
  solo las que vienen a cuento (`seleccion.rs`), y sigue congelado por decisión del usuario.
- Las instrucciones de antes eran una lista de rutas («si pide dibujar usa…»). El 22-oct
  (`respaldo/agente-prompt`) pasaron a principios; faltaba el mapa del entorno.
- No había ningún paso de **comprobar antes de contestar** salvo la revisión visual del lienzo, y esa
  se quedaba en una ronda sin red (sin volver atrás si empeoraba).

## Lo que se hizo (23-oct)

**1. Enseñar en vez de prohibir**
- `agente::instrucciones` (nube): quién eres y dónde trabajas; qué haces tú (traducir, redactar,
  razonar) y qué hacen las herramientas (leer y escribir en la app); el método (entender → mirar →
  planear → actuar → comprobar → contar); el nivel de calidad; cuándo preguntar; cuatro ejemplos.
- **`manual.rs`, el mapa de la app** (nuevo): 15 sitios (chats y notas, lienzo —elegir hoja, crear,
  encontrar y cambiar, desde una imagen—, mirar, documentos, tareas, capturas, voz, lecciones,
  ventanas, móvil y conexiones, saber y recordar, hablar con el usuario), qué es cada uno, cómo se
  nombra lo de dentro y **qué herramientas lo leen o lo escriben**. Más los cuatro patrones de
  combinación (leer → pensar → escribir; encontrar → cambiar; crear → mirar → corregir; buscar lo
  que no ves) y qué hacer antes de decir que no.
  - **Se genera de las tablas de verdad** (`herramientas::catalogo`, `acciones::todas`,
    `edicion::filas`): no se queda viejo. Una prueba falla si se añade una herramienta sin sitio o si
    el mapa nombra una que ya no existe.
  - Fijo para cada versión: va en el prefijo cacheado (~1.300 tokens más, frente a los ~13.000 del
    catálogo).
- El «cuándo usarla» de cada herramienta vive en su descripción (`herramientas::para_la_nube`), no
  en las instrucciones.

**2. Que pueda hacerlo**
- `editar_texto`: traducir o corregir todos los textos de una figura en una llamada.
- `preguntar_usuario` con su **tarjeta en la burbuja** (lo que faltaba de la rama): la pregunta, un
  botón por opción (la recomendada primero), «Otro…» para escribir lo suyo; lo que se escribe
  mientras espera es la respuesta; a los 5 minutos sigue con la recomendada.
- **Memoria de largo plazo** (`memoria.rs`): el perfil y los temas del usuario viajan a sus
  aparatos; al pulsar «Conversación nueva» se guarda lo duradero de la que se cierra.

**3. Que supervise su trabajo**
- **Lint** (`lint.rs`): lo que se mide sin modelo (textos encimados o que se salen, flechas
  sueltas, casi-alineados, sin traducir, tildes) va pegado al resultado de cada herramienta que dibuja
  o edita.
- **Revisión con rúbrica** (`ojos/rubrica.rs`): tras dibujar, mira una captura y la puntúa;
  **guarda una copia** antes de cada corrección y **vuelve a la mejor** si la corrección empeora (en
  la app: `al_lienzo::Trabajo::{Copia, VolverA, Comprobar}`, un solo Ctrl+Z para todo el turno). Dos
  rondas con la nube. Si Haiku no da una nota legible o suspende dos veces, puntúa **Sonnet** (el
  crítico de respaldo, con la misma clave).
- **Repaso antes de contestar** (`agente::repasar`, nuevo, solo nube):
  - si iba a **rendirse** («no puedo», «no tengo herramienta»…), una vuelta para buscar la
    combinación en el mapa y hacerla, o hacer la parte posible y decir qué falta;
  - si hizo algo y el pedido era **de varias partes** o **algo falló**, una vuelta para repasar el
    pedido punto por punto **contra los resultados de las herramientas** y terminar lo que falte.
  - Si estaba todo bien contesta «LISTO» y no queda rastro (ni en la burbuja ni en la conversación);
    si encontró algo, lo hace y lo cuenta.

## Cómo medirlo

El banco que ya hay (`apps/pixpin/src/asistente/banco_documentos.rs`, `ojos/banco.rs`, pruebas
`#[ignore]` con la nube y una clave) se amplía con pedidos de resultado **verificable**:

| Pedido | Se comprueba |
|---|---|
| «Traduce esta figura al inglés» | 0 textos en español en la figura; ninguno se sale de su forma (lint). |
| «Corrige las tildes del lienzo» | El lint de ortografía queda vacío. |
| «Haz un diagrama del proceso de compra con 6 pasos y una decisión» | 7 formas; 1 rombo; flechas unidas; 0 solapes. |
| «Saca las tareas de este PDF y ponlas para el viernes» | N tareas en una lista con fecha. |
| «Resume cada sección del PDF al margen» | Una nota por sección, sin solapar el texto. |
| «Pon los títulos en azul y más grandes» | Solo cambian los títulos. |
| «Exporta esto a Word y mándalo al móvil» (dos partes) | Las dos acciones hechas. |
| Pedidos imposibles («llama por teléfono a Juan») | Dice qué no se puede en una línea; no inventa. |

Métricas por pedido: hecho / hecho en parte / no hecho; **rendiciones falsas** (dijo «no puedo» y
se podía); fallos duros del lint al final; vueltas, tokens y segundos. El repaso y la revisión se
encienden y apagan (`Opciones::{repasar, revisar}`) para medir lo que aportan y lo que cuestan.

## Lo que cuesta y los riesgos

- **Tokens**: el mapa, ~1.300 tokens cacheados. El repaso, una vuelta más solo en pedidos de varias
  partes, con fallos o rendiciones. La revisión, hasta tres capturas puntuadas (~1.200 tokens cada
  una) solo tras dibujar.
- **Repaso complaciente**: el mismo modelo puede decir «LISTO» sin estarlo. Por eso el repaso se
  ancla en los resultados y el lint, y la revisión visual usa un crítico más fuerte.
- **El modelo local no cambia**: congelado por decisión del usuario (15-oct); ni mapa, ni repaso,
  ni revisión.

## Lo siguiente

1. Correr el banco ampliado con la nube y apuntar las métricas antes y después.
2. Llevar el lint más allá del lienzo: documentos anotados (notas que se pisan) y tareas
   (duplicadas, sin fecha cuando la había).
3. Que las descripciones de las 47 acciones de la app digan también qué devuelven y con qué se
   combinan (la parte de «relaciones entre recursos» de Anthropic).
4. Llevar `manual.rs` y el repaso a la copia del núcleo de Mac (`PIXPIN_PRO_MAC`).
