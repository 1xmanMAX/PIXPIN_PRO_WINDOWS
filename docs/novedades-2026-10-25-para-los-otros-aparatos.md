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

### 3.3 Reunion presencial (27-oct-2026)

El usuario: «supon que estoy en una reunion PRESENCIAL: que siga grabando
aunque haya salido de la app, que aparezca un boton flotante para tomar
FOTOS de la reunion y, como las capturas, que todo se adjunte a la reunion;
y las fotos y las capturas a resolucion COMPLETA».

**En el PC** (hecho): tercer modo **Presencial** en «Grabar clase». Graba el
microfono (el sonido del equipo, apagado de salida) y la barra flotante
(siempre encima, pequena, se arrastra, no sale en las capturas) lleva
**«Foto»**: una foto con la camara del equipo (`MediaCapture`) a la mayor
resolucion que tenga, guardada tal cual en JPEG; y **«Video»**: un clip corto
con la camara (1080p30 como mucho, HEVC si la grafica lo codifica, si no
H.264, ~5 Mbps a ritmo variable, AAC 96 kbps). Con varias camaras se elige
en Preparar (la de atras primero: mira a la sala) y se recuerda. La barra
vive en su propio hilo: cerrar el chat o dejar PixPin en la bandeja no corta
nada; el icono de la bandeja dice «Grabando…» y «Salir» pregunta antes
(«Si» termina y guarda; «No» sigue grabando). Las capturas de pantalla de
los otros modos van en PNG a los pixeles de la pantalla, sin escalar; en el
chat entran los bytes tal cual y solo las miniaturas se reducen. «Estudiar
la clase» carga la imagen entera al acercarla.

**Lo que se guarda** (lo mismo de la tabla del §3, con dos campos nuevos):

| Mensaje | `resto` |
|---|---|
| La nota de voz (`VOZ`) | lo de siempre y, si hubo clips, `clips: [{ms, id, duracionMs}]` en orden de `ms` (**sin clips la clave se quita**, no `[]`) |
| Cada foto de la camara (`IMAGEN`, `foto-<hora>-<n>.jpg`) | `enGrabacion: {de, ms}` (el minuto en que se pulso) y **`fuente: "camara"`** |
| Cada clip (`ARCHIVO`, `.mp4`) | `enGrabacion: {de, ms, duracionMs}` (`ms`: cuando empezo) y `fuente: "camara"` |

En la carpeta de la grabacion (§3.2) las fotos van como las capturas («⏱
1:05») y cada clip con «▶ 1:05» en su minuto. `fuente` falta en las capturas
de pantalla (y en todo lo de antes): **falta = pantalla**. Las cuentas puras
(modos, que camara, que resolucion, nombres, `clips`) estan en
`crates/pixpin-proyecto/src/presencial.rs`.

**`fuente` y no `origen`** (27-oct-2026, al juntarlo con la camara del
movil): la primera version escribia `origen: "camara"` en `resto`, pero
`origen` ya es un campo del mensaje (el aparato de donde se copio): la linea
del cuaderno salia con `"origen"` dos veces y **no se volvia a leer** (la
foto desaparecia del chat). Ahora es `fuente: "camara"` (y `"movil"` lo de
la camara del movil, §3.4); el PC rescata al leer las lineas escritas con
`origen` repetido. **Android / apple**: escribir y leer `fuente`; nunca
`origen` en `resto`.

**En el movil es el uso natural** (por hacer en Android y iPhone):

- **Grabar la reunion entera con el propio telefono**, en segundo plano:
  servicio en primer plano (Android, `foregroundServiceType="microphone|camera"`)
  o sesion de audio/camara en segundo plano (iOS), con la notificacion
  permanente «Grabando…» y sus botones **Foto**, **Clip** y **Terminar**
  (y, si se quiere, la burbuja flotante encima de otras apps). Dos formas:
  - **Video de la reunion** (el movil apoyado mirando a la sala): HEVC
    (H.265) por hardware, si no H.264; **1080p30 a ~4–6 Mbps VBR**, o
    **720p a ~2,5 Mbps** para reuniones largas (se elige o se pasa solo a
    720p pasada la hora); sonido **AAC 96 kbps mono**. Entra como el video
    del §3 (`video: {id}` en la nota y `enGrabacion {de, ms: 0}` en el MP4).
  - **Solo audio** (el movil en la mesa): la nota de voz de siempre.
- **La nota de voz sale siempre** (con video tambien: la pista de audio a
  `.m4a`), porque **la transcripcion se hace despues de su audio**, como en
  el PC (Whisper o Qwen3 en el aparato, o en el PC al sincronizar): el texto
  de la nota en trozos de ~15 s, cada linea `[m:ss] texto`.
- **Fotos durante la reunion** a la resolucion entera de la camara de atras
  (sin reducir; JPEG de la camara, o HEVC/HEIC convertido a JPEG ~92 para
  que el PC lo lea), con `enGrabacion {de, ms}` y `fuente: "camara"`; y
  **clips** con su `duracionMs`, como en la tabla de arriba. Tocar la foto o
  el clip en la notificacion no saca la app delante.
- **Mismo formato, para que sincronice al PC sin tocar nada**: la nota con
  su transcripcion, el video, las fotos y los clips enlazados por `resto`, y
  la carpeta de la grabacion del §3.2. El PC los ensena en «Estudiar la
  clase» en su minuto (los clips, con ▶ en la lista de marcadores; un clic
  lleva al minuto y lo abre).
- **El movil como camara del PC** (otra cosa: el telefono manda su imagen a
  una grabacion que hace el PC) va en
  [movil-como-camara.md](movil-como-camara.md); no se repite aqui.

### 3.4 El movil como camara de la reunion del PC (27-oct-2026)

- Mientras el PC graba, **el movil hace las fotos y clips cortos** y entran
  en la clase en su minuto, como una captura mas. Va por el canal cifrado del
  grupo y el `suelto` de siempre, con la capacidad `camara1` en el `hola`:
  `camara_remota` (el PC le pasa al movil en que milisegundo va la grabacion
  y si esta en pausa) y `suelto` con `destino: "grabacion"` y `ms` (el
  minuto calculado en el movil).
- Mismo formato que la reunion presencial (§3.3): los clips en la misma
  `clips: [{ms, id, duracionMs}]` de la nota, cada pieza con `enGrabacion`
  (un clip, con `duracionMs`) y **`fuente: "movil"`** (la camara del equipo,
  `"camara"`); ademas `deAparato` y `tomadaEn`. «Estudiar la clase» los
  ensena por el mismo camino (▶ en su minuto, «Vídeo del móvil»).
- Guia completa (mensajes, JSON, flujo de la pantalla, CameraX, codecs):
  [movil-como-camara.md](movil-como-camara.md).
- **Android**: hacerla (§9 de la guia). **apple**: el nucleo ya tiene el
  cable.

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
