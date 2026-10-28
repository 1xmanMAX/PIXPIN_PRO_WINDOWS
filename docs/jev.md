# Jev de TypeSafe en PixPin (28-oct-2026)

## Que es Jev

**Jev** es el modelo de **TypeSafe** (`typesafe.ai`), el primer modelo
«System One» (o «de decisiones»): no escribe texto, **contesta preguntas
tipadas** sobre un estado que se le da —texto, JSON con campos con nombre e
imagenes— con **probabilidades calibradas**, en 70-500 ms. Hay cuatro tipos
de pregunta:

| Tipo | Que contesta | Ejemplo en PixPin |
|---|---|---|
| **Noul** | la probabilidad de que algo sea verdad (0-1) | «¿este renglon encarga una tarea?» |
| **Choice** | una opcion de varias, con la probabilidad de cada una | (el cliente la sabe; ninguna funcion la usa todavia) |
| **Score** | un nivel de una escala descrita nivel a nivel, con la probabilidad de cada nivel | «¿que pasa si esta tarea se hace tarde?» (0-3) |
| **Box** | donde esta algo en una imagen (sus cuatro bordes en fracciones) | (el cliente la sabe; ninguna funcion la usa todavia) |

La regla de PixPin, la de la guia de TypeSafe: **los juicios de Jev, las
decisiones en codigo**. Jev dice «este renglon encarga una tarea: 0.94»;
que se marca, que tarjeta va a Anki, que prioridad tiene una tarea o que
modelo contesta al asistente lo decide una **tabla escrita en codigo**, con
sus pruebas. Una pregunta pide un solo juicio, el que una persona haria de
un vistazo; las fechas, las cuentas y lo que el usuario marco a mano los
lleva el codigo.

**Sin clave de TypeSafe, todo sigue exactamente como antes.** Y un fallo
(sin red, 401, 429, una respuesta rota) **es un error, nunca un «no»**: la
funcion hace lo de siempre y lo apunta en el registro.

## Donde esta

- `crates/pixpin-typesafe` (capa 1, sin crates de PixPin; portable):
  - `api.rs`: la peticion y la respuesta segun el OpenAPI
    (`https://api.typesafe.ai/openapi.json`, version 0.2.0).
  - `cliente.rs`: el POST con 2 s de tiempo, el registro del cuerpo exacto
    (`tracing`, nivel `debug`, objetivo `jev`; las imagenes cambiadas por su
    tamano; la clave nunca), el **cache** de peticiones identicas (por la
    huella SHA-256 del cuerpo, 2048 respuestas por proceso) y un reintento
    con 429/529.
  - `transporte.rs`: WinHTTP en Windows (lo mismo que usa el asistente para
    Claude y DeepSeek); un transporte grabado en las pruebas.
  - `claves.rs`: la clave cifrada con DPAPI.
  - **`preguntas.rs`: todo el texto de todas las preguntas, sus criterios,
    los niveles de cada Score y los umbrales.** Es lo que se revisa y se
    cambia; nada de texto de preguntas fuera de ahi.
  - `reunion.rs`, `anki.rs`, `urgencia.rs`, `modelo.rs`: el estado de cada
    funcion, la lectura de las respuestas y su **tabla de decision**.
- `apps/pixpin/src/jev.rs` y `jev/`: la clave y los interruptores de
  Ajustes, y donde cada funcion se engancha a la app.

## La API que se usa

`POST https://api.typesafe.ai/v1/systemone` con `Authorization: Bearer
<clave>` y JSON:

```json
{
  "model": "jev-latest",
  "state": {"clase": "Cálculo I", "antes": ["…"], "linea": "Para el lunes resuelvan del 1 al 5.", "despues": ["…"]},
  "questions": {
    "tarea": {"type": "noul", "instructions": "Does `linea` contain an action item…?",
              "criteria": {"true": "Someone is given a task…", "false": "The speaker explains…"}},
    "valor": {"type": "score", "instructions": "…", "criteria": ["nivel 0", "nivel 1", "nivel 2", "nivel 3"]}
  }
}
```

Respuesta: `{"model": "jev-1.14.0", "answers": {"tarea": {"type": "noul",
"noul": 0.94}, "valor": {"type": "score", "score": 2.6, "confidence": 0.6,
"legend": {…}, "probabilities": {"0": 0.0, "1": 0.1, "2": 0.2, "3": 0.7}}},
"usage": {"input_tokens": 180, "output_tokens": 9}}`. La Choice manda
`criteria: {nombre: descripcion}` (en el orden escrito: el orden pesa) y
contesta `choice`, `confidence` y `probabilities`; el Box contesta `box:
{top, bottom, left, right}`. Con imagenes (`assets: {ref: {type: "image",
format, data_base64}}`) o un Box se manda la cabecera
`TypeSafe-Danger-Zone-Images: true`. La prueba de la clave de Ajustes es
`GET /v1/models` (no gasta tokens).

Lo que se sigue de la guia: **una peticion por estado, con todas sus
preguntas juntas** (el estado se lee y se paga una vez); **preguntas
especulativas** (quien y cuando de una tarea van en la misma peticion y
solo se leen si hay tarea); **umbrales 0.7 / 0.3** con la franja de en
medio como «sin decidir» (tiene su fila en cada tabla); los campos vacios
no se mandan (una lista vacia se leeria como senal); las preguntas en
ingles (donde Jev acierta mas) sobre el estado tal cual, en espanol.

## Las cuatro funciones

Cada una tiene su interruptor en **Ajustes › Asistente › TypeSafe (Jev)**
(`[jev]` del TOML: `reunion`, `anki`, `urgencia`, `modelo`; encendidas de
fabrica, pero sin clave no hacen nada).

### 1. Partes importantes de la clase o reunion

**Cuando**: al acabar de pasar a texto una nota de voz (desde el chat o al
acabar de grabar una clase), en un hilo, cuatro peticiones a la vez; y al
abrir «Estudiar la clase» si la nota aun no se miro.

**Estado** (uno por renglon con minuto y dos palabras o mas): `clase` (el
titulo), `linea` (el renglon), `antes` y `despues` (hasta dos renglones de
cada lado).

**Preguntas** (Nouls): `tarea` (¿encarga algo que alguien tiene que hacer
despues?), `tarea_quien`, `tarea_cuando` (especulativas), `decision`
(¿anuncia algo ya decidido?), `pregunta` (¿alguien pregunta algo que espera
respuesta?), `examen` (¿dice que entra en un examen o una nota?), `dato`
(¿una definicion, formula, regla o dato clave?), `charla` (¿charla ajena al
tema?).

**Tabla** (`reunion::decidir`), con SI = 0.7:

| charla ≥ SI | tarea ≥ SI | decision ≥ SI | pregunta ≥ SI | examen o dato ≥ SI | se marca |
|---|---|---|---|---|---|
| no | si | — | — | — | tarea |
| no | — | si | — | — | decision |
| no | — | — | si | — | pregunta |
| no | — | — | — | si | examen |
| si | si | — | — | — | solo tarea («traigan la calculadora» es logistica y es tarea) |
| si | no | * | * | * | nada |
| (cualquiera entre 0.3 y 0.7) | | | | | no se marca |

**Datos**: `resto["importantes"] = [{ms, tipo, p}]` en la nota de voz (y
`quien`, `cuando`, `enTareas` en las tareas), `resto["importantesDe"]` el
modelo. Sin clave `importantes`: no se miro; `[]`: se miro y no hay nada.
Ver [manual-sincronizar-lo-nuevo.md §14](manual-sincronizar-lo-nuevo.md).

**Se ve**: en «Estudiar la clase», un punto de color por tipo bajo el
minuto de cada renglon y, bajo el titulo de la transcripcion, el filtro
(«Todo · Tarea 3 · Decisión 1 · Pregunta 5 · Examen 2»); mientras se busca,
«Buscando lo importante…». «+N tareas a Tareas» apunta las tareas en el
Inbox con su nota `💬 🎙 <clase> ⏱ m:ss` (el minuto al que volver). La
pagina compartida (HTML) lleva las mismas chapitas y el filtro.

**Sin clave**: nada cambia (ni marcas ni filtro).

### 2. Tarjetas de Anki que valen la pena

**Cuando**: «Enviar a Anki» de una clase grabada (una tarjeta por captura,
con lo dicho en su tramo detras).

**Estado** (uno por tarjeta con tres palabras dichas o mas): `clase`,
`minuto`, `dicho`.

**Preguntas**: `valor` (Score 0-3: nada que estudiar · un ejemplo que solo
se entiende viendo la clase · explica parte del tema · una definicion,
formula, regla o idea clave que memorizar) y `examen` (Noul).

**Tabla** (`anki::veredicto`), con `util` = P(nivel 2 o 3):

| examen ≥ 0.7 | util ≥ 0.7 | util ≤ 0.3 | veredicto |
|---|---|---|---|
| si | * | * | queda |
| no | si | — | queda |
| no | — | si | sobra |
| no | entre 0.3 y 0.7 | | dudosa |

Y `anki::elegir`: quedan las que «quedan» y las que no tienen nada dicho
(la imagen es lo que se repasa); las dudosas, de mas a menos util, solo
hasta llegar a 3; si aun asi no quedara ninguna, la mas util. El aviso dice
cuantas dejo fuera.

**Sin clave o si falla**: todas, como siempre.

### 3. Urgencia de las tareas segun el contexto

**Cuando**: al leer las listas en la ventana de Tareas, en un hilo (hasta 60
tareas por tanda, cuatro a la vez); al acabar, Tareas relee. Lo sabido se
queda en memoria mientras la app esta abierta (nada va al disco ni viaja);
tras un fallo, esa tarea no se vuelve a preguntar en 10 minutos.

**Estado**: `tarea` (su texto sin marcas, con su nota), `lista`, `chat` y,
si hay, `mensajes_recientes` (los cinco ultimos mensajes escritos del
chat). **La fecha no va**: cuanto falta para su hora (`⏰`) lo cuenta el
codigo.

**Preguntas**: `consecuencia` (Score 0-3: nada · una pequena molestia ·
afecta al trabajo o los estudios · dano serio), `bloquea` (¿otro la
espera?), `plazo_ajeno` (¿el plazo lo puso otro: un profesor, una oficina,
un evento?) y `piden_ya` (¿en los mensajes recientes la piden pronto? solo
con mensajes).

**Tabla** (`urgencia::decidir`): **lo marcado por el usuario manda** (⏫ →
urgente, 🔼 → alta). Si no, puntos:

| senal | puntos |
|---|---|
| consecuencia (valor esperado del Score) | 0 a 3 |
| bloquea ≥ 0.7 | +1 |
| plazo_ajeno ≥ 0.7 | +1 |
| piden_ya ≥ 0.7 | +1 |
| su hora es hoy o ya paso | +2 |
| manana | +1.5 |
| en 2-3 dias | +1 |
| en 4-7 dias | +0.5 |

≥ 5 urgente; ≥ 3 alta; **baja** si P(consecuencia 2-3) ≤ 0.3, bloquea y
plazo_ajeno ≤ 0.3, nadie la pide y no tiene hora en 7 dias; lo demas
normal.

**Se ve**: una chapita «✦ Urgente», «✦ Alta» o «✦ Baja» (la chispa dice
que es de Jev, no una marca del usuario) en Tarjetas y Kanban, y lo
pendiente se ordena por prioridad (urgente, alta, normal, baja) antes que
por fecha. Una tarea marcada a mano ensena solo su marca.

**Sin clave**: ni chapitas ni otro orden; todo como siempre.

### 4. Elegir el modelo segun el pedido

**Cuando**: cada pedido al asistente que **no** es una orden del camino
rapido (esas se hacen en este PC sin modelo, como siempre). La pregunta se
hace en el hilo del turno, al pedir la primera respuesta: el chat no espera
al enviar.

**Estado**: `pedido` y, si hay, `conversacion_reciente` (los tres ultimos
mensajes con quien los dijo).

**Preguntas**: `dificultad` (Score 0-3: una accion directa · una respuesta
corta o un dibujo simple · una explicacion o trabajo con varias partes ·
razonar con cuidado sobre mucho material), `orden_simple`, `razonar` e
`importa` (¿un error tendria consecuencias: una nota, dinero, salud, un
plazo?).

**Tabla** (`modelo::decidir`), por orden; `dificil` = P(nivel 2-3),
`muy_dificil` = P(nivel 3):

| condicion | modelo |
|---|---|
| orden_simple ≥ 0.7 y dificil ≤ 0.3 | rapido |
| razonar ≥ 0.7 | fuerte |
| muy_dificil ≥ 0.5 | fuerte |
| importa ≥ 0.7 y dificil ≥ 0.5 | fuerte |
| dificil ≤ 0.3 | rapido |
| lo demas (sin decidir) | rapido (lo de siempre) |

Rapido es el de siempre (Claude Haiku 5.5 `claude-haiku-5-5`, o DeepSeek
Flash); fuerte, el del mismo proveedor y con la misma clave (Claude Sonnet
5.5 `claude-sonnet-5-5`, o DeepSeek V4 Pro `deepseek-v4-pro`), con el mismo
«pensar». La pastilla del modelo del chat «Asistente» dice lo elegido para
el ultimo pedido («Claude Sonnet · Jev») durante 15 minutos.

**Sin clave o si Jev falla**: el de siempre.

## Privacidad: que se manda

Solo con clave y con la funcion encendida, y solo a `api.typesafe.ai` por
HTTPS:

- **Reunion**: el titulo de la grabacion y su transcripcion, renglon a
  renglon con sus vecinos. Ni el audio ni el video ni las capturas.
- **Anki**: el titulo y lo dicho en el tramo de cada captura. Las imagenes
  no.
- **Urgencia**: el texto de cada tarea pendiente, el titulo de su lista,
  el nombre de su chat y los cinco ultimos mensajes escritos de ese chat.
- **Modelo**: el pedido al asistente y los tres ultimos mensajes de la
  conversacion.

La clave va cifrada con DPAPI en
`%LOCALAPPDATA%\PixPinMax\claves\typesafe.clave`: nunca en el TOML de
ajustes, nunca en la carpeta de datos que se sincroniza o se exporta. Lo
que se escribe en los datos (`importantes`) son minutos, tipos y
probabilidades, sin texto. TypeSafe dice que Jev no se entrena con lo que
se le manda (pagina de modelos, «Data handling»).

## Coste

Jev se paga por token de entrada: **0.042 $ por millon** (42 $ por mil
millones; los de salida, gratis), precio de Jev 1.14 el 28-oct-2026. Lo
que cuesta en PixPin, mas o menos:

- una clase de una hora (~150 renglones, ~350 tokens cada peticion con sus
  ocho preguntas): ~50 000 tokens ≈ **0.002 $**;
- mandar a Anki una clase de 20 capturas: ~8 000 tokens ≈ 0.0003 $;
- las prioridades de 60 tareas: ~25 000 tokens ≈ 0.001 $;
- elegir el modelo de un pedido: ~400 tokens ≈ 0.00002 $ (lo que se ahorra
  al no mandar a Sonnet lo facil, y al no mandar a Haiku lo dificil, es
  mucho mas).

Lo identico sale del cache del proceso y no se paga dos veces.

## Pruebas

- `cargo test -p pixpin-typesafe`: el JSON segun el OpenAPI, los errores
  (401, red, respuesta que falta o rota: errores, no «no»), el cache, el
  reintento, la cabecera de imagenes, cada tabla con sus filas y cada
  funcion con respuestas grabadas.
- `de_verdad_con_la_api` (`#[ignore]`): solo con `TYPESAFE_API_KEY` y en
  Windows, `cargo test -p pixpin-typesafe -- --ignored`.
- `pixpin-proyecto`: `clase` (el formato de `importantes`) y
  `clase_exportar` (las chapitas y el filtro de la pagina).
- `pixpin-anki`: lo dicho por captura y quedarse con algunas.

Las preguntas se escribieron sin probarlas contra la API (no habia clave el
28-oct-2026): antes de fiarse, probar con casos reales, dificiles y
negativos (la guia de TypeSafe, «Test with the live API») y ajustar el
texto en `preguntas.rs`, no los umbrales a unos pocos casos.

## Android

- **Lo que llega**: `importantes` en la nota de voz (ver
  [manual-sincronizar-lo-nuevo.md §14](manual-sincronizar-lo-nuevo.md)).
  El movil lo ensena igual (puntos de color, filtro, «+N tareas») sin
  necesitar clave ni red.
- **Lo que no viaja**: la clave, los interruptores de `[jev]` (son de este
  PC), las prioridades de las tareas (se calculan en cada aparato) y el
  modelo elegido.
- **Si Android quiere preguntar el mismo** (con su propia clave): las
  mismas preguntas y tablas, copiando `preguntas.rs` y las funciones
  `decidir` de cada modulo; la API es un POST con JSON (no hace falta SDK).
  Guardar la clave en el Keystore, nunca en lo que se sincroniza.
