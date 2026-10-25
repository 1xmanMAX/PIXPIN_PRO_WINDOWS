# Novedades del 25-oct-2026: lo que tienen que hacer los otros aparatos

Lo que entro en `main` el 25-oct-2026 en PixPin Max (Windows) y lo que
necesitan PixPin Android y la version de Mac, iPad e iPhone (`apple/`) para
leerlo y escribirlo igual. Todo viaja por la sincronizacion de siempre: no hay
mensajes nuevos en el protocolo, solo ficheros y campos de `resto`.

**Manual paso a paso** (ficheros, formatos, choques y lista de tareas por
aparato): [manual-sincronizar-lo-nuevo.md](manual-sincronizar-lo-nuevo.md).
Al sincronizar, ahora se juntan sin perder nada los marcadores de una clase,
las paginas de una carpeta y la biblioteca de figuras cambiados en los dos
aparatos (§7 del manual).

## 1. Vista de cada marcador del lienzo (como las vistas de AutoCAD)

- Al poner un marcador se guarda lo que se veia (centro y cuanto mundo cabia);
  al ir a el, se vuelve a esa vista. Mayus+clic (o Ctrl+Mayus+1…9) la
  actualiza.
- Fichero `pins/draw/<d>.vistas.json`, al lado de `pins/draw/<d>.marcas`
  (la linea de `.marcas` no cambia). Objeto `{"<id de la marca>": "x y ancho alto"}`.
- Vaciar = escribir `{}` (la sincronizacion no lleva borrados).
- Formato, formulas para guardar y restaurar y la lista de tareas de Android:
  [sincronizar-marcos-vistas-y-tinta.md §3.8](sincronizar-marcos-vistas-y-tinta.md).
- **Android**: meter `<d>.vistas.json` en el alcance de cada dibujo, junto a
  `<d>.marcas`, y guardar/restaurar la vista de cada marca.
- **apple/**: el nucleo ya lo tiene (`apple/nucleo`, misma logica que `crates/`).

## 2. Nota rica en el chat (texto con formato e imagenes en una burbuja)

- Sigue siendo una `NOTA` cuyo `texto` es Markdown; las imagenes van como
  `![…](pixpin:files/guardados/pc/<chat>/notas/<ms>-….png)`. Sin campos nuevos.
- Varias imagenes seguidas se ensenan como album (filas de 2 o 3).
- Detalle: [nota-rica-del-chat.md](nota-rica-del-chat.md).
- **Android / apple**: que la burbuja de una `NOTA` pinte ese Markdown y las
  imagenes en su sitio, nunca la ruta como texto.

## 3. Grabar clase o reunion (audio, capturas y ahora video)

Al terminar una grabacion entran en el chat, con numeros seguidos:

| Mensaje | `resto` |
|---|---|
| La nota de voz (`VOZ`) | `grabacion: "clase"`, `capturas: [{ms, id}]` (siempre, aunque este vacia), `video: {id}` si se grabo video |
| El video (`ARCHIVO`, `.mp4`, H.264 + AAC) | `enGrabacion: {de: <id de la voz>, ms: 0}` |
| Cada captura (`IMAGEN`) | `enGrabacion: {de: <id de la voz>, ms}` |

- La transcripcion sale de la nota de voz, como antes.
- **Android / apple**: conservar esos campos (ya viajan en `resto`) y, si se
  quiere, ensenar la clase como en el PC: el texto a un lado y la imagen del
  minuto que suena al otro.

### 3.1 Dos modos al grabar y la ventana «Estudiar la clase»

- Al preparar se elige el modo: **Pantalla y sonido** (video de la pantalla
  con el sonido del equipo y/o el microfono) o **Audio y capturas** (solo
  audio; «Captura» guarda la pantalla entera donde esta la barra, al
  instante). Lo que se guarda es lo mismo de la tabla de arriba: no hay
  campos nuevos por el modo.
- **Marcadores con nota**, campo nuevo en la nota de voz:

  | Mensaje | `resto` |
  |---|---|
  | La nota de voz (`VOZ`) | `marcadores: [{ms, texto}]` en orden de `ms`; uno por milisegundo (poner otro en el mismo cambia la nota); `texto` de 200 caracteres como mucho, sin saltos de linea, puede ir vacio. **Sin marcadores la clave se quita** (no `[]`), como `marcas`. |

  Son distintos de las banderitas (`marcas: [ms]`), que siguen igual; la
  ventana del PC ensena las dos cosas juntas y quita cada una de su campo.
  Las cuentas (y su formato) estan en `crates/pixpin-proyecto/src/clase.rs`.
- **Android / apple**: conservar `marcadores` (viaja en `resto`) y, si se
  quiere, ensenarlos en la letra de la nota como fichas con su minuto que
  llevan el audio ahi.

### 3.2 La grabacion entra en su carpeta

- Al terminar, ademas de los mensajes de la tabla de arriba (que no
  cambian), entra **una carpeta** (`MINIAPP` «carpeta») con el nombre de
  la grabacion y, en su documento, `tipo: "grabacion"`, `voz`, `video`,
  `duracionMs` y cada pieza con su `ms`. Lo que se adjunta luego a la
  clase lleva `adjunto: true` en su pagina y no es de lo grabado.
- En el PC la grabacion se ve como **una sola tarjeta** (no la nota, el
  video y las capturas sueltos) y, en general, **lo que esta en una
  carpeta ya no sale suelto en el chat** (tambien las paginas de un
  cuaderno): se ve dentro de su carpeta, en la galeria y con la lupa.
- Formato, ejemplo y tareas:
  [manual-sincronizar-lo-nuevo.md §5.1](manual-sincronizar-lo-nuevo.md).
- **Android / apple**: conservar el `texto` de la carpeta tal cual (ya se
  hace con las mini-apps). Opcional: la tarjeta de la grabacion y no
  ensenar sueltos los mensajes de una carpeta.

## 4. La ficha del chat (la isla al pulsar el nombre)

- Pestanas nuevas en `Seccion` (`crates/pixpin-proyecto/src/cuaderno.rs`):
  **Videos** (imagen o archivo con extension de video, `es_video`) y
  **Enlaces** (mensajes con una direccion web, `tiene_enlace`). Fotos y
  Archivos ya no cuentan los videos. Copiado en `apple/nucleo`.
- Lo que se pulsa se ve dentro de la isla; solo la ✕, Esc o un clic fuera la
  cierran.

## 5. Lo que se quito

- La vista **Proyectos** del chat (el interruptor queda en Chat · Figuras).
- Del menu ⋮ del chat: Proyectos, Conversaciones, Comenzar, Escala de
  interfaz, Grupos de ventanas y Ajustes. La escala y los grupos de ventanas
  estan ahora en Ajustes → General → Ventanas.
- Adjuntar una figura abre la ventana Figuras con un boton «Adjuntar».

No cambia nada de lo que se guarda en disco por esto: un aparato que aun
tenga la vista Proyectos sigue leyendo y escribiendo los mismos chats.
