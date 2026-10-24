# Un asistente que trabaja como Claude Code

*25-oct-2026 · sigue a [2026-10-23-asistente-que-sabe-usar-la-app.md](2026-10-23-asistente-que-sabe-usar-la-app.md)*

## El pedido

> «El agente todavía no es consistente en su resultado final y comete muchos errores; quiero que
> siga mejorándolo… investiga formas de que realmente sea un agente como Claude Code y que mejore
> mucho más su resultado en cualquier escenario.»

Dos fallos reales del usuario, con la nube:

1. **El cronograma.** En el lienzo estaba la plantilla de serie «Cronograma»; pidió *«agrándala a 30
   actividades y 30 semanas»*. El asistente la agrandó (mal numerada: ya arreglado en
   `pixpin-motor2d/src/bloques.rs`) y **además dibujó otros dos cronogramas al lado** con las
   herramientas de dibujar, torcidos e incompletos, **con actividades que nadie pidió**.
2. **El anuncio.** Tras la revisión final escribió *«todavía no está listo el cronograma de 19
   semanas y 30 actividades, quedó a medias y desordenado… ahora voy a limpiar el resto y hacerlo
   por completo, alineado y revisado»* y **ahí terminó el turno**, sin hacer nada más.

## Cómo trabajan los buenos agentes

| Idea | Fuente | Qué tiene PixPin |
|---|---|---|
| El bucle es **reunir contexto → actuar → verificar → repetir**; verificar con reglas (lint, pruebas) antes que con otro modelo. | Anthropic, [Building agents with the Claude Agent SDK](https://www.anthropic.com/engineering/building-agents-with-the-claude-agent-sdk) | Bucle `vueltas` + lint (`lint.rs`) + revisión con rúbrica (`ojos`). **Falta**: las comprobaciones deterministas antes de ejecutar (que una llamada no contradiga el pedido). |
| Empezar por lo más simple; **orquestador-trabajadores** solo cuando las subtareas no se pueden prever, y **evaluador-optimizador** cuando hay un criterio claro. Herramientas «a prueba de errores» (poka-yoke): que el uso equivocado no sea posible. | Anthropic, [Building effective agents](https://www.anthropic.com/engineering/building-effective-agents) | Hay jefe (`orquesta.rs`) y evaluador (`revisar`). **Pero el jefe entraba casi siempre** (ver diagnóstico) y no había poka-yoke: con el cronograma puesto, dibujar otro era tan fácil como alargarlo. |
| **Lista de tareas explícita** (`TodoWrite`) que el propio agente escribe al empezar algo de varios pasos y va marcando; un solo paso en curso; no acaba con pasos abiertos. | Claude Code (herramienta `TodoWrite`, [mejores prácticas](https://www.anthropic.com/engineering/claude-code-best-practices)) | Solo la lista del jefe (`Evento::Tareas`). El turno normal no tenía ninguna. |
| **Leer antes de editar** (la herramienta de editar falla si no se leyó el fichero) y **editar en vez de reescribir**. | Claude Code (herramientas `Read`/`Edit`) | El «Lienzo destino» se lee al empezar; las instrucciones piden editar (rama del 24-oct). **Falta** que sea una regla dura, no solo un consejo. |
| **El turno no se acaba porque el modelo lo anuncie**: el agente solo para cuando contesta sin herramientas, así que «ahora voy a…» sin llamada es un fallo, y se corrige devolviéndoselo. Los *hooks* de parada (`Stop`) permiten no dejarle acabar si algo no está. | Claude Code ([hooks](https://docs.claude.com/en/docs/claude-code/hooks)) | Nada: un texto sin llamadas cerraba el turno, dijera lo que dijera. |
| **Contexto mínimo y relevante**: lo justo, en el sitio justo; separar lo que es del usuario de lo que añade el sistema. | Anthropic, [Effective context engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents) | El resumen del lienzo y las notas (hora, archivos) se pegan **al mismo texto del pedido**, y las reglas que leen «el pedido» los leían también. |
| **Subagentes con contexto limpio** van bien para buscar o investigar en paralelo, pero pierden el hilo en tareas acopladas; dan peor resultado si cada uno no ve lo que hizo el resto. | Anthropic, [How we built our multi-agent research system](https://www.anthropic.com/engineering/multi-agent-research-system) | Los trabajadores del jefe empiezan de cero con «haz SOLO la tuya»: para *agrandar una plantilla* (una sola cosa acoplada) eso es lo peor. |
| **Evaluaciones con casos reales** del uso, con comprobación automática del resultado (no del texto), y casos negativos. | Anthropic, [Demystifying evals for AI agents](https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents) | Banco `tests/banco.rs` (~100 casos). **Faltaban** los dos casos del usuario. |
| **Preguntar ante la ambigüedad** que cambia el resultado; si no, asumir y decirlo. | Claude Code (`AskUserQuestion`); CLAM/AskBench | `preguntar_usuario` (22-oct) y la regla del 24-oct («¿cambiar este o hacer otro?»). |

## Diagnóstico: por qué falla (el cronograma, paso a paso)

1. **El pedido «crecía» con el lienzo y entraba el jefe.** La app pega al pedido el resumen del
   lienzo (`edicion::anadir_resumen`: «[Lienzo destino: 120 elementos…»), la hora y los archivos. El
   bucle decidía con ese texto entero: `orquesta::conviene` → `pedido_compuesto` cuenta «varias
   partes» si pasa de 220 letras, y con el resumen **siempre pasaba**. Con la nube el jefe está
   siempre (`nube::jefe`), así que *«agrándala a 30 actividades y 30 semanas»* —una sola cosa— se
   partía en tareas («30 actividades», «30 semanas», «revisar»…). Cada **trabajador empieza con un
   contexto limpio** y la orden «haz SOLO la tuya»: no ve que otro ya agrandó la plantilla y hace lo
   suyo… dibujando un cronograma nuevo. Eso explica *dos* cronogramas de más (dos tareas). El mismo
   texto alimentaba los colores (`colores_del_pedido`) y el repaso (`REPASO_COMPLETO` siempre).
2. **Nada impedía dibujar otro al lado.** El 24-oct se quitaron `dibujar/cronograma/svg` del
   catálogo cuando el pedido es agrandar, pero (a) la vuelta de planear puede volver a añadirlas
   (`Opciones::ampliar` trae el catálogo entero), (b) con un pedido que no case la heurística no se
   quitan, y (c) es un consejo en el catálogo, no una comprobación al ejecutar. Hacía falta un
   poka-yoke en el bucle.
3. **Inventar contenido**: el trabajador recibe «Tarea: agrandar a 30 actividades. Se da por hecha si:
   hay 30 actividades», y el camino corto para cumplir eso es *escribir 30 actividades*. Las reglas
   de no inventar (24-oct) ayudan; la guarda del punto 2 corta el caso de dibujarlas.
4. **Acabar anunciando.** El jefe tenía **una sola** ronda de «lo que falta» (`ronda_final: bool`);
   si la segunda revisión aún veía cosas a medias, su `respuesta` («ahora voy a limpiar el resto…»)
   se daba al usuario y el turno acababa. Igual en el turno normal: cualquier texto sin llamadas
   cerraba, aunque dijera «voy a…».
5. **Pocas vueltas con la nube.** `VUELTAS = 4` (+2 reintentos) se puso por el modelo local de 2B,
   que se queda en bucle; con Claude o DeepSeek, agrandar + numerar + revisar no cabe en 6 llamadas y
   acaba en «Hecho en parte» o anunciando lo que falta.
6. **Sin lista propia del turno.** Fuera del jefe, el modelo no escribe ni marca sus pasos: nada
   comprueba que hizo todo lo pedido (y solo lo pedido) antes de contestar.

## Plan priorizado

| # | Mejora | Impacto | Riesgo | Estado |
|---|---|---|---|---|
| 1 | **Pedido sin anexos** para decidir (jefe, repaso, colores): `guardas::sin_anexos`, `agente::pedido_limpio`. | Alto: deja de mandar al jefe lo simple con el lienzo abierto (la causa de los duplicados). | Bajo. | **Hecho** |
| 2 | **Guarda anti-duplicado** determinista: pedido de cambiar lo que hay + lienzo con algo + llamada de crear desde cero (dibujar, cronograma, SVG, poner otra plantilla) → `ERROR` que explica qué usar (`alargar`, `reorganizar_hoja`, `editar_texto`) o que pregunte. Vale también para los trabajadores del jefe. | Alto. | Bajo: no salta si pide «otro», «al lado», «nuevo», o «ponle…» (añadir piezas). | **Hecho** |
| 3 | **No acabar anunciando**: una respuesta sin llamadas que anuncia trabajo o dice que quedó a medias recibe un empujón («hazlo ahora o di exactamente qué falta») y el turno sigue, sin gastar vueltas, hasta 3 veces (repaso y revisión incluidos). En el jefe: hasta 2 rondas finales, el anuncio cuenta como «falta», y sin rondas la respuesta dice que no quedó terminado en vez de prometer. | Alto (el caso 2). | Bajo: las ofertas («si quieres, …») y las preguntas no cuentan; solo con la nube. | **Hecho** |
| 4 | **Lista de pasos del turno** (`pasos_del_pedido`, como `TodoWrite`): el modelo la escribe en pedidos de 3+ pasos y la marca; se ve en la burbuja (la lista del jefe); no puede acabar con pasos abiertos (empujón con cuáles). Apuntar no gasta vuelta. | Medio-alto. | Bajo: herramienta nueva solo con la nube y el repaso; los trabajadores del jefe no la tienen. | **Hecho** |
| 5 | **Más vueltas con la nube** (`VUELTAS_NUBE = 12`): el tope es de seguridad, no el que decide cuándo acabar. | Medio. | Bajo (coste: se paga lo que se usa). | **Hecho** |
| 6 | **Casos del banco**: `editar-agrandar-plantilla-sin-duplicar` (no dibuja, usa `alargar` hasta 30) y `editar-agrandar-sin-inventar` (no añade ni escribe actividades). | Medir. | Nulo. | **Hecho** (falta correrlos con la nube) |
| 7 | **Verificar contra el pedido** al acabar: sacar los números pedidos («30 actividades», «30 semanas») y compararlos con lo que hay en el lienzo (filas de la plantilla tras `alargar`, que el resumen diga «Cronograma: 30 actividades × 30 semanas»). Si no cuadra, empujón con la diferencia. | Alto. | Medio: necesita que `figuras_de_serie` cuente filas y semanas de una plantilla puesta. | Pendiente |
| 8 | **El jefe solo para lo que se reparte de verdad**: no partir en tareas lo que toca un mismo objeto (una plantilla, una tabla) y pasar a cada trabajador lo hecho *en el lienzo* (el resumen actualizado), no solo el informe. | Alto. | Medio. | Pendiente |
| 9 | **Leer antes de editar como regla dura**: si una llamada de editar nombra un `#id` que el modelo no ha visto en este turno (ni en el resumen ni en un resultado), ERROR «léelo primero». | Medio. | Bajo. | Pendiente |
| 10 | **Hooks de parada configurables**: que cada comprobación (lint, números, pasos) sea un «hook» que puede impedir acabar, con el mismo tope de empujones. | Medio. | Medio. | Pendiente |
| 11 | Llevar el diario (`diario.rs`) a métricas por caso: empujones, guardas que saltaron, vueltas usadas; y casos del banco salidos del diario. | Medir. | Bajo. | Pendiente |

## Lo que se hizo (25-oct)

- `crates/pixpin-ia/src/guardas.rs` (nuevo): `sin_anexos`, `lo_que_pidio` (el pedido dentro del
  encargo de un trabajador), `duplicaria` (la guarda), `anuncia_pendiente` y `EMPUJON_ANUNCIO`.
- `crates/pixpin-ia/src/pasos.rs` (nuevo): la herramienta `pasos_del_pedido`, su lectura, el
  resultado para el modelo y el empujón con los pasos abiertos.
- `crates/pixpin-ia/src/agente.rs`: `pedido_limpio` (para el jefe, el repaso, los colores y el
  crítico); `pedido_del_usuario` ya no confunde los avisos del turno con el pedido
  (`MARCA_DEL_TURNO`); la guarda antes de ejecutar; el empujón al acabar (`Estado::empujones`);
  `pasos_del_pedido` lo contesta el turno y se ve como `Evento::Tareas`; `VUELTAS_NUBE`; dos líneas
  más en las instrucciones (la lista de pasos; contestar sin herramientas acaba el turno).
- `crates/pixpin-ia/src/orquesta.rs`: `RONDAS_FINALES = 2`, `faltan_del_cierre` (el anuncio cuenta
  como lo que falta) y `sin_promesas`.
- Pruebas puras con motor guionado (anuncia → empujón → llamada → acaba; respuesta normal no se
  empuja; tope; el local no se toca; agrandar no dibuja otro y dibujar algo nuevo sí; los pasos
  abiertos se empujan y sin repaso no se ofrece; el jefe hace lo anunciado en otra ronda).

## Cómo medirlo

```text
cd crates/pixpin-ia
PIXPIN_IA_MOTOR=claude PIXPIN_IA_SOLO="editar-agrandar" cargo test --test banco -- --ignored --nocapture --exact banco
```

Y en el diario del asistente: cuántos turnos acaban en «Hecho en parte», cuántos empujones y
cuántas guardas saltan por semana (deberían bajar los primeros y ser raros los segundos).
