# Novedades del 1-nov-2026: cada asignatura con su lista de tareas

El usuario: «en la ventana de Tareas, en el Horario, pongo mis asignaturas y
mi horario. Quiero que cada asignatura se cree a la vez como asignatura y como
grupo de tareas; que al abrir Tareas vea mis asignaturas entre las listas y
pueda añadirles tareas (ahora no puedo), y que se puedan buscar desde Flow
Launcher».

Lo que entra en PinDesk (Windows) y lo que necesitan Android y `apple/`.
**No hay campos nuevos en lo que se sincroniza**: la lista de una asignatura
es una lista de tareas como cualquier otra (mensaje `MINIAPP` con
`miniapp:"tareas"`) en el chat «Asignaturas».

## 1. El modelo

- **Asignatura** = una linea `Asignatura <nombre> [@aula] [#RRGGBB] lista:<id>`
  de `horario.md` (solo de este equipo; el horario no viaja).
- **Su lista** = el mensaje `<id>` del chat «Asignaturas» (el chat con
  `"asignaturas": true` en su ficha del indice; si no la tiene, el que se llama
  asi). Su titulo (`# <nombre>`) es el nombre de la asignatura.
- `lista:<id>` es **el enlace**: con el, renombrar la asignatura renombra la
  lista (el titulo del documento y `nombre` del mensaje) y sus tareas siguen
  con ella. Sin el (horarios de antes del 1-nov, o escritos a mano), la lista
  se busca por el nombre sin mirar mayusculas ni tildes, y el enlace se apunta
  solo.
- Manda el nombre del horario: si la lista enlazada se llama de otra forma (se
  renombro en el movil), se le vuelve a poner el de la asignatura.
- **Nunca se borran tareas**: quitar una asignatura del horario deja su lista
  en el chat con sus tareas (deja de salir con color en Tareas y, vacia, deja
  de verse alli; en el chat sigue). Si la lista enlazada ya no esta (se borro
  en el chat), se crea otra vacia.

## 2. Quien lo hace (Windows)

- Al añadir una asignatura en la vista Horario se crea su lista **a la vez**
  y su linea nace con `lista:<id>`.
- La ficha de una asignatura (doble clic) tiene ahora **Nombre** ademas de
  Aula: renombrarla renombra sus clases y su lista.
- El vigia del horario, al arrancar y cada vez que `horario.md` cambia
  (`horario::asignaturas::enlazar`): crea las listas que faltan (tambien las
  de las asignaturas que solo nombra una clase), apunta los enlaces (solo toca
  el final de las lineas `Asignatura …`; lo demas queda letra a letra),
  renombra y deja `cache/asignaturas.json` para Flow.

## 3. La ventana de Tareas

- Las listas de las asignaturas salen **con el color de la asignatura** y
  **aunque esten vacias** (Tarjetas y Kanban); buscando, si su nombre coincide.
- Cada encabezado de lista tiene un **«＋»**: lo que se escriba en la caja
  va a esa lista (vale para todas las listas, no solo el Inbox).
- El libro de la caja («Lista o asignatura de la tarea») elige entre el Inbox,
  las asignaturas (📘) y todas las demas listas de todos los chats.

## 4. Flow Launcher y la linea de ordenes

- `p geom` → la asignatura, con su proxima clase («Próxima clase: mañana 8:00
  · A-12», «En clase hasta 10:00») y sus pendientes. Intro abre su lista en
  Tareas; Tab deja su nombre escrito.
- `p geometría resolver la hoja 3` → «Añadir tarea a Geometría: resolver la
  hoja 3» (con las mismas #etiquetas, urgencia y hora que `t …`).
- Pedidos (`docs/protocolo-pedidos.md`): `anadir_tarea` con `asignatura`, y
  `abrir_lista` (`asignatura` o `proyecto`+`codigo`).
- `pindesk tarea "hoja 3" --asignatura Geometría`, `pindesk tareas --asignatura Física`.

## 5. Lo que necesitan los otros aparatos

- **Android**: nada obligatorio; las listas llegan como siempre al chat
  «Asignaturas». Recomendado: no ofrecer renombrar esas listas desde el movil
  (el PC les vuelve a poner el nombre de la asignatura) o, si se renombran,
  saber que el PC manda.
- **apple/**: lo mismo. Si algun dia tiene horario, que escriba y lea el mismo
  `lista:<id>` en las lineas `Asignatura …`.
