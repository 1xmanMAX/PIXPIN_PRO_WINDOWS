# La ventana principal: Funciones y Chats

Como es la ventana principal de PixPin Max (la del chat) desde el
25-oct-2026, para que **Android y Mac** la repliquen igual que Windows. Cada
parte de la pantalla tiene su seccion; cada seccion la escribe quien la hizo y
no toca las demas (añadir una seccion nueva al final, con su propio titulo
`##`).

Arriba, en la barra de titulo, va el **interruptor Chat · Funciones**: dos
partes, una puesta. «Chat» ensena la lista de chats y la conversacion
abierta; «Funciones» cambia la lista por la rejilla de funciones. Las claves
de idioma son `proyectos-interruptor-chat` y
`proyectos-interruptor-funciones`.

## Funciones

**Que es.** La segunda parte del interruptor (antes «Figuras», que abria su
ventana aparte). El usuario: «que Figuras sea como Funciones, donde solo
aparezcan unas pocas funciones fuertes … y quitarlas de los chats». Elegida,
se queda puesta como una pestana: la **columna de la lista de chats** se
cambia por una rejilla de mosaicos grandes. La conversacion abierta sigue a
la derecha (en una ventana estrecha, o en el movil, la rejilla ocupa la
pantalla entera debajo de la barra). Pulsar «Chat» devuelve la lista.

**Lo que salio de la vista de chats.** La cabecera de la lista tenia, a la
izquierda de sincronizar, tres botones redondos: Tareas, Galeria de capturas
y Timeline. Se quitaron: ahora estan solo en Funciones, y la cabecera queda
con el buscador y sincronizar. Nada se quito de la aplicacion: lo que cuelga
de un chat (el «+» con Lienzo, Figura, Tareas como mini-app…) sigue en el
chat, y la bandeja no cambia.

### Los mosaicos, en su orden

El orden es el de uso, y es el mismo en todos los aparatos. El nombre estable
(`pixpin_ui::funciones::Funcion::nombre`) sirve para atajos, pruebas o
analitica.

| # | Nombre estable | Titulo (es / en) | Descripcion (es) | Icono (Material) | Color del circulo | Que hace |
|---|---|---|---|---|---|---|
| 1 | `grabar` | Grabar reunión / Record meeting | Clase o reunión, con capturas y transcripción | `mic` | `#E5484D` | La grabadora de clase o reunion (microfono + sonido del equipo + capturas de una zona, y luego el texto). Va al chat que se elija; sin chats, a «Guardados». |
| 2 | `lienzo` | Nuevo lienzo / New canvas | Un lienzo en blanco para dibujar o calcar | `draw` | `#8E4EC6` | Crea un lienzo vacio en el chat abierto (sale en su conversacion, como el «Lienzo» del «+») o, sin chat abierto, en «Guardados», y lo abre al momento. |
| 3 | `tareas` | Tareas / Tasks | Las listas de tareas de todos tus chats | `checklist` | `#1F7A3A` | La pantalla de Tareas (todas las listas juntas). |
| 4 | `timeline` | Timeline / Timeline | Tu día minuto a minuto, y las lecciones | `timeline` | `#0090FF` | El timeline, con la pestaña de Lecciones dentro. |
| 5 | `horario` | Horario / Timetable | Tus clases de la semana, con aviso antes | `calendar_month` | `#F76B15` | El horario de clases. Si el horario pasa a vivir dentro de Tareas, este mosaico abre Tareas en su vista Horario. |
| 6 | `galeria` | Capturas / Screenshots | Todas las capturas que has guardado | `photo_library` | `#12A594` | La galeria de capturas. |
| 7 | `figuras` | Figuras / Shapes | Tu biblioteca de figuras para los lienzos | `layers` | `#3E63DD` | La biblioteca de figuras (en Windows, su ventana con previsualizador). |

Claves de idioma: `funciones-titulo` (el titulo «Funciones» encima de la
rejilla) y, por mosaico, `funciones-<nombre>` y `funciones-<nombre>-detalle`
(en los cuatro `main.ftl`: Windows y Apple, es-ES y en-US). Android usa los
mismos textos.

### Como se ve

- **Titulo** «Funciones» arriba a la izquierda, 17 pt, color de texto normal,
  en una franja de 44 pt.
- **Rejilla** con 14 pt de margen y 10 pt entre mosaicos. Columnas segun el
  ancho: un mosaico mide al menos 140 pt, y nunca hay mas de 3 columnas (en la
  lista de siempre, 320 pt, salen 2). Se lee como un texto: de izquierda a
  derecha y de arriba abajo.
- **Mosaico**: rectangulo de esquinas redondeadas (radio 12 pt), 104 pt de
  alto, fondo el de la caja del buscador del tema (claro u oscuro). Dentro,
  con 12 pt de relleno: el circulo de color (34 pt) con el icono en blanco,
  debajo el titulo (13 pt) y debajo la descripcion en una linea (11 pt, color
  apagado, con puntos suspensivos si no cabe).
- **Ventana baja**: los mosaicos se encogen hasta 60 pt; por debajo de 88 pt
  se quita la descripcion y el icono pasa a la izquierda del titulo.
- **Lista plegada** (solo avatares): una columna de cuadrados con solo el
  circulo y el icono, sin titulo.
- **Raton**: al pasar, el mosaico toma el color de fila resaltada del tema.
- **Teclado** (al elegir la pestaña el teclado es de la rejilla hasta que se
  pulse fuera): flechas para moverse (no dan la vuelta; izquierda y derecha
  pasan de fila), Inicio y Fin, Tab da la vuelta, Intro o Espacio hacen el
  elegido, Escape vuelve a Chat. El elegido con el teclado se pinta con el
  color de fila elegida y el texto en su color.
- Claro y oscuro: solo colores del tema; los circulos de color son los mismos
  en los dos.

### Donde esta en el codigo (Windows)

- `crates/pixpin-ui/src/funciones.rs`: el orden, las claves, la rejilla
  (`rejilla`) y las flechas (`mover`), con pruebas.
- `crates/pixpin-ui/src/interruptor.rs` y
  `apps/pixpin/src/ventana_chat/interruptor.rs`: el interruptor.
- `apps/pixpin/src/ventana_chat/funciones.rs`: pintar, raton, teclado y lo que
  hace cada mosaico.
