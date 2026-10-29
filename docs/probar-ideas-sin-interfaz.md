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
