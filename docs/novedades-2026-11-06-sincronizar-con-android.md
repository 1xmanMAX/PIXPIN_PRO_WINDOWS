# Novedades del 6-nov-2026: lo nuevo de Android, también en el PC

El usuario: «hay nuevos cambios en la app de android, aplica estos cambios
aquí también para que todo sea también sincronizable correctamente».

Android v0.161.0 (4-nov) trajo la **revisión de urgencia que viaja**
(`docs/urgencia-que-viaja.md` de Android): lo que dijo Jev de cada tarea se
guarda junto a su lista y no se vuelve a preguntar en ningún aparato. Para
que el PC lo sincronice bien hacían falta cuatro cosas, y al mirarlo salieron
otras que tiene que arreglar Android (§3).

## 1. Lo que hace ahora el PC

### El fichero

`pins/draw/anot-<uid de la lista>.urgencia.json`, uno por lista de tareas
(el mensaje del chat), al lado de lo demás anotado de ese mensaje. En el PC,
`pindesk_proyecto::anotado::urgencia_de_la_lista(raiz, ficha, uid)`, en
`android/pins/draw/` de la carpeta del chat, como los comentarios.

```json
{
  "version": 1,
  "tareas": [
    {
      "texto": "Mandar la cotización (al cliente)",
      "consecuencia": [
        0.05,
        0.15,
        0.5,
        0.3
      ],
      "confianza": 0.6,
      "bloquea": 0.8,
      "plazo_ajeno": 0.9,
      "piden_ya": 0.2,
      "cuando": 1793582613265,
      "aparato": "K7Q2"
    }
  ]
}
```

`pindesk_sincro::urgencias` es el puerto de `UrgenciasGuardadas.kt`, probado
con los mismos casos que `UrgenciasGuardadasTest` y con el ejemplo del
manual de Android:

- **Leer**: una tarea sin `texto` (o en blanco) o sin `consecuencia` (o
  vacía) no cuenta; `confianza` es 0.6 si falta, `bloquea` y `plazo_ajeno`
  0; `piden_ya` falta si se preguntó sin mensajes del chat.
- **Juntar**: por `texto`; gana el `cuando` mayor y, a igualdad, el
  `aparato` mayor (comparado por UTF-16, como Kotlin). Conmutativo e
  idempotente.
- **Escribir**: claves en el orden de Android (`texto`, `consecuencia`,
  `confianza`, `bloquea`, `plazo_ajeno`, `piden_ya` si hay, `cuando`,
  `aparato` si no está vacío), las tareas por `texto`, los números a
  diezmilésimas como `Double.toString` (`1.0`, `1.0E-4`), sangría de dos
  espacios como kotlinx y `\n` al final (`canonico::a_texto_con_sangria`,
  `canonico::double_de_kotlin`). Lo desconocido de arriba se conserva y lo
  desconocido de dentro de una tarea no (Android tampoco). Un JSON roto no se
  pisa; si nada cambia, no se reescribe; temporal y renombrar.

### Viaja con su lista

- `.urgencia.json` está en `TERMINACIONES` (`pindesk_sincro::anotado`).
- **Lo anotado con el código de cualquier mensaje viaja con él**
  (`disco::alcance_de` y `prioridad::referencias`), como `alcanceCon` y
  `referencias` de Android desde el 4-nov. Antes solo lo de un adjunto o una
  nota (`lleva_anotado`), y lo de una lista de tareas, que es un `MINIAPP`,
  no salía nunca. **Lo que se borra con el mensaje no cambia**: sigue siendo
  solo lo de `lleva_anotado` (`vista.rs`, `disco_android.rs`), igual que en
  Android.

### Se junta al llegar, y lo juntado vuelve

- `disco::juntar_urgencia`, al escribir lo que llega (después de las
  lecciones y las pegatinas de comentario): si aquí también se revisaron
  tareas, se escribe lo de los dos.
- **Lo acordado es lo que llegó, no lo juntado** (`resumenAcordadoDe` de
  Android): en la red local, `traer_archivo` devuelve el resumen de lo que
  llegó por el cable para lo que se junta al llegar
  (`disco::se_junta_al_llegar`: comentarios, urgencia y lecciones). Así el
  fichero juntado cuenta como cambiado aquí y vuelve al otro en la vuelta
  siguiente; al cabo de dos vueltas los dos quedan iguales byte a byte. De
  paso, `juntar_leccion` hace por fin lo que decía su comentario («si la
  fusión difiere de lo que tiene el otro, la vuelta siguiente se la manda»).
- **Por la nube**, la urgencia se junta tarea a tarea (antes habría pasado
  por la fusión genérica de JSON, que no sabe de `cuando` ni de `aparato`), y
  si lo escrito no es lo que bajó, el fichero queda «sin subir» y sube en la
  misma vuelta.

### Jev no pregunta lo que ya está revisado

`jev::urgencia` (la app):

- `tareas::Lista` lleva el código único de su mensaje (`uid`).
- **Antes de preguntar** se mira lo sabido en memoria y luego el fichero de
  la lista: si su `texto` está, se usan esas señales
  (`Senales::de_partes`, con el valor esperado Σ i·p como lo rehace Android)
  y no se pregunta, venga de este equipo o de otro.
- **Lo que responde Jev** se añade al fichero de su lista, con su hora y el
  código de este equipo (`Identidad::leer_o_crear(raiz, "PC").yo.codigo()`).
- **Cuándo se pregunta**: las listas nacidas en este equipo (o sin aparato),
  al cambiar los mensajes, pasados 5 s, sin abrir Tareas: un hilo
  (`urgencia::vigilar`, arrancado en `main.rs`) mira la firma de las tareas
  cada 2 s, y también una vez al arrancar. Las de otro aparato, solo con
  Tareas abierta y si su revisión aún no ha llegado.
- Igual que antes: los temas de una reunión (`temasLista`) ni se preguntan ni
  se guardan; tras un fallo, esa tarea no se vuelve a preguntar en 10 min; lo
  que apura una reunión en vivo se queda en memoria.

### Probado

- `pindesk-sincro`: el formato, el juntar y el escribir (unidad); dos
  «móviles» por la red local con el protocolo de verdad (la parte de la
  urgencia de `SincronizarDeVerdadTest`), con una comprobación de que la
  prueba falla sin el arreglo de lo acordado; y dos aparatos por la nube
  falsa con el 409.
- `pindesk-proyecto` (`tests/sincronizar_con_el_movil.rs`): el PC y el móvil;
  la urgencia de una lista del PC llega tal cual, los dos revisan otra tarea
  a la vez y quedan iguales, dirigiendo el PC y dirigiendo el móvil.
- La app (`jev::urgencia`): solo compilado y con Clippy para Windows desde
  Linux; sus pruebas no corren aquí. Sin clave de Jev no hace nada.

### Apple

`apple/nucleo/pindesk-sincro` ya iba por detrás (le falta, por ejemplo,
`.adjunto`). Se le trajo solo lo puro, que compila y pasa sus pruebas allí:
`.urgencia.json` en `TERMINACIONES`, el módulo `urgencias` y lo que necesita
de `canonico`. Que viaje con cualquier mensaje y se junte al llegar queda
para cuando se traiga el resto de `disco` y `protocolo`.

## 2. Lo que no cambia

- El protocolo: los ficheros que se juntan siguen viajando como binarios (no
  son `es_texto`), y nada nuevo se pregunta al otro aparato.
- Un aparato que aún no conoce `.urgencia.json` no lo manda ni lo pide; uno
  de antes del 4-nov en Android tampoco. No se rompe nada: cada uno pregunta
  a Jev como siempre.

## 3. Lo que tiene que arreglar Android

### 3.1 `Mensaje.asignatura` se pierde (y `Proyecto.asignaturas`)

`docs/novedades-2026-11-06-un-chat-para-las-asignaturas.md` decía que
Android guardaba `asignatura` «en `resto`». **No es así**: el `Mensaje` de
Android (`guardados/Mensajes.kt`) no tiene dónde guardar lo que no conoce y
se lee con `ignoreUnknownKeys = true`, así que el campo **se tira** al
recibir el mensaje. Lo mismo `"asignaturas": true` del `Proyecto`
(`motor/Proyectos.kt`).

No es solo que el móvil no lo vea: el resumen de un mensaje se calcula sobre
todos sus campos, y el PC cuenta `asignatura` (`kotlin::normalizar_mensaje`
conserva lo desconocido para que cuadre con un móvil más nuevo). El móvil,
sin el campo, saca otro resumen del mismo mensaje y lo puede tomar por
cambiado; si su versión vuelve al PC, la asignatura se pierde también allí.

Hace falta, como con `asistente`:

```kotlin
// guardados/Mensajes.kt, en Mensaje
val asignatura: String? = null,
// motor/Proyectos.kt, en Proyecto
val asignaturas: Boolean? = null,
```

Y conservarlos al copiar o editar (`copy(...)` ya lo hace si están en la
clase).

### 3.2 Las pegatinas de un dibujo no viajan desde el móvil

En `Disco.alcanceCon`, `poner` añade a cada lienzo sus marcadores
(`marcasJuntoA`) y sus vistas (`vistasJuntoA`), pero **no**:

- `<dibujo>.comentarios.json`, las pegatinas del lienzo;
- lo adjuntado a ellas, `<dibujo>.a<huella>.adjunto`, ni el lienzo propio de
  una entrada, `<dibujo>.c<id>.excalidraw.gz`.

Esos nombres no empiezan por `anot-`, así que `porUid` tampoco los recoge.
El PC sí los manda (`anotado::comentarios_junto_a` y
`anotado::rutas_de_comentarios`, desde el 3-nov): lo del PC llega al móvil,
pero lo puesto en el móvil sobre un dibujo no llega al PC. Falta en `poner`
lo equivalente: el `.comentarios.json` junto a las marcas y, leyéndolo, las
rutas de sus adjuntos y lienzos (solo de `pins/draw/`, sin subcarpetas).

### 3.3 Los chats «pr-17…»

Chats que salen con un id `pr-<milisegundos>` en vez de un nombre. Esos ids
los pone Android al crear un proyecto, y en particular `Asistente.asegurarChat`
al crear un chat «Asistente» cuando `ChatDelAsistente.buscar` no encuentra
ninguno. La causa probable:

1. Un móvil y un PC crean cada uno su «Asistente». El PC los junta
   (`vista/asistente_unico.rs`): **pasa primero los mensajes** del que sobra
   al más antiguo, con sus adjuntos, y luego lo borra como cualquier chat
   (copia, marcas de sus mensajes y lápida). Eso ya era así; no había nada
   que adelantar.
2. En el móvil, la **sincronización automática se salta todo chat con lápida
   en cualquiera de los dos lados** (`SincronizacionAutomatica.con`:
   «eso lo decide quien mire»), y solo la pantalla de sincronizar aplica las
   lápidas, si el chat está marcado. Además, si el móvil lo tocó después de
   la lápida, `planDeBorrados` se lo queda sin levantar la lápida del PC.
   Así el chat borrado sigue en el móvil, fuera de toda sincronización, y si
   el asistente del móvil sigue escribiendo en él la conversación vuelve a
   quedar partida.
3. Si el «Asistente» del móvil se borra pero el bueno aún no ha llegado,
   `asegurarChat` crea otro `pr-<ms>` y el ciclo se repite; un chat cuyos
   mensajes quedan sin su `Proyecto` se ve con su id.

Lo que ayudaría en Android: aplicar la lápida de un chat del asistente
también en la automática (sus mensajes ya están en el que queda), o no
crear un «Asistente» nuevo mientras haya mensajes con los ids fijos del PC
(`IDS_DEL_PC`) o un chat del asistente con lápida pendiente. En el PC no se
cambió nada: el orden ya era el correcto.
