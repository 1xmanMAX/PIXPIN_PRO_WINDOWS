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
| 5 | `horario` | Horario / Timetable | Tus clases de la semana, con aviso antes | `calendar_month` | `#F76B15` | Abre Tareas en su vista Horario (el horario vive dentro de Tareas desde el 25-oct). |
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

## Buscar en todos los chats y sincronizar

Dos cosas de la cabecera de la lista de chats (25-oct-2026, el usuario:
«que el buscador busque tambien dentro de todos los chats» y «mientras se
sincroniza, que el boton gire»).

### El buscador busca tambien dentro de los chats

**Que hace.** Lo escrito en el buscador de la lista sigue filtrando los chats
por su nombre y su ultima linea, y ademas, **debajo de esos chats**, sale una
seccion **«Mensajes · N»** con los mensajes de *cualquier* chat que llevan lo
escrito, como en Telegram. N es cuantos hay en total.

- **Que se busca** de cada mensaje: su texto (en una nota rica, el texto sin
  las rutas de sus fotos: el pie `alt` si cuenta, `pixpin:files/…` no), la
  transcripcion de una nota de voz y el nombre de su fichero. El pie de una
  foto es su texto. No hay OCR guardado por mensaje, asi que no se busca.
- **No salen** los mensajes que no tienen burbuja en su chat: las paginas
  sueltas de un PDF unido (`soloGaleria`), las lecciones y los de sistema
  (`auto-*`). Pulsarlos no llevaria a ninguna parte.
- **Como compara**: sin mayusculas ni tildes, con la misma tabla que la lupa
  de dentro del chat (`a/á/à/â/ä/ã/å → a`, `ñ → n`, `ç → c`…, de un
  caracter a uno), y la frase entera en cualquier sitio. Los saltos de linea y
  tabuladores cuentan como un espacio («hola mundo» encuentra «hola⏎mundo»).
  El filtro de los chats por nombre tambien deja de mirar tildes.
- **Orden**: del mas nuevo al mas viejo (fecha del mensaje), y a igual fecha
  por chat y codigo, para que la misma busqueda de siempre lo mismo.
- **Cada fila** mide lo que una fila de chat (62 pt): el avatar del chat (su
  logo o su circulo de color con iniciales), el nombre del chat, la fecha del
  mensaje a la derecha (como la hora de la lista: hoy la hora, si no la fecha)
  y debajo **un trozo en una linea**: empieza como mucho 22 caracteres antes
  de lo encontrado, por el principio de una palabra y con «…» delante si se
  corta; 160 caracteres como mucho, con «…» detras. Lo encontrado va en
  negrita y con un fondo del color de enviar al 28 %.
- **La cabecera** «Mensajes · N»: franja de 30 pt del color de la cabecera,
  texto de 12 pt apagado.
- **Ver mas**: se ensenan 20; si hay mas, una fila de 40 pt «Ver N más»
  centrada, en el color de enviar, que anade otros 20 cada vez.
- **Pulsar un mensaje** abre su chat llevado a el y resaltado (el mismo camino
  que «Ver en el chat»). Si es el chat abierto y tenia su lupa o un chip de
  etiqueta puestos, se quitan: el mensaje podria estar escondido.
- **Sin chats que casen por nombre pero con mensajes**, la lista no dice
  «Nada con eso»: ensena solo la seccion. Con la caja vacia (o Escape), la
  seccion se va al momento.
- La lista se desplaza entera, chats y seccion, con la rueda.

**Rapido con muchos datos.**

- El indice se hace **fuera del hilo de la ventana**, la primera vez que se
  busca, y despues solo se relee el `guardados.jsonl` de un chat si cambio su
  fecha o su tamano (y se sueltan los chats que ya no estan). Cada texto se
  guarda tal cual y **aplanado** (minusculas, sin tildes), asi buscar es un
  `contains` sobre lo ya aplanado; los tramos a resaltar se calculan solo de
  los que se ensenan.
- **Espera de 180 ms** desde la ultima tecla antes de buscar (el
  «debounce»); si llegan varios pedidos al hilo, solo se atiende el ultimo, y
  lo que conteste a uno viejo se tira.
- Cuando llega algo por sincronizar (o cambia el indice de chats) con una
  busqueda puesta, se repite sola.

**Claves de idioma** (los cuatro `main.ftl`): `chat-buscar-mensajes` (con
`$n`: «Mensajes · { $n }» / «Messages · { $n }») y `chat-buscar-ver-mas`
(«Ver { $n } más» / «Show { $n } more»).

**En el movil y el Mac**: el mismo buscador de la lista de chats, la misma
seccion debajo y lo mismo al tocar (abrir el chat en ese mensaje,
resaltado). En el movil, el indice en una corrutina de fondo
(`Dispatchers.Default`) y la espera de 180 ms con `debounce`; en el Mac/iPad,
una `Task` de fondo. Las reglas de comparar y del trozo estan en
`crates/pixpin-ui/src/buscar_en_chats.rs` con sus pruebas: copiarlas tal cual
(o llamar al nucleo Rust en Apple) para que den lo mismo en todos los
aparatos.

### El boton de sincronizar gira mientras se sincroniza

- **Gira** (una vuelta por segundo, en el sentido de las agujas del reloj,
  sobre su centro) mientras haya **cualquier vuelta en marcha**: la pulsada en
  la pantalla de Sincronizar, la automatica, la de las tareas al instante, la
  de los chats compartidos con otras personas y la que pide el otro aparato al
  llamar a este. Las sondas de «¿estas ahi?» no cuentan (duran un instante).
  Al acabar se para y vuelve a su posicion.
- **Si la ultima vuelta fallo** (no conecto o se corto; en «con todos», si no
  quedo al dia con ninguno), el boton lleva una **marca roja con «!»** arriba
  a la derecha (circulo `#E53935` con aro del color de la cabecera), hasta que
  se pulsa el boton (la pantalla de Sincronizar dice que paso) o empieza otra
  vuelta. Cancelar no cuenta como fallo.
- **Sin gasto en reposo**: se repinta 30 veces por segundo solo mientras
  gira; quieto no hay temporizador. El hilo de la vuelta despierta la ventana
  al empezar y al acabar.
- En el movil: el icono del boton con una `rotate` infinita
  (`rememberInfiniteTransition`, 1000 ms lineal) mientras el servicio de
  sincronizar diga que hay una vuelta en marcha, y la chapa roja con «!» con
  el ultimo resultado. En el Mac, lo mismo con `rotationEffect` y
  `withAnimation(.linear(duration: 1).repeatForever(autoreverses: false))`.

**Donde esta en el codigo (Windows)**

- `crates/pixpin-ui/src/buscar_en_chats.rs`: aplanar, buscar con tope y
  cuenta, el trozo, la espera y como se reparte la seccion; con pruebas.
- `crates/pixpin-ui/src/giro.rs`: el angulo y cuando repintar; con pruebas.
- `apps/pixpin/src/ventana_chat/buscar_en_chats.rs`: el hilo del indice y
  pintar y pulsar la seccion.
- `apps/pixpin/src/sincronizar.rs` (`EnCurso`, `ultima_fallo`,
  `olvidar_fallo`): cuantas vueltas hay y como acabo la ultima.

## Foto del chat

*25-oct-2026.* El usuario: «permitir poner un fondo en las esferas de los
chats para que parezca mas una app de chat: dame la opcion de poner emojis
como su perfil, o poner una imagen yo».

**Donde se abre.** Clic derecho sobre un chat de la lista → «Cambiar foto
del chat…» (sustituye a «Poner/Quitar imagen de logo»), o pulsar el avatar
de arriba en la isla del chat (la ficha que sale al pulsar el nombre; con
el raton encima sale un aro azul).

**La hoja** (`apps/pixpin/src/ventana_chat/foto_del_chat.rs`), pequena,
siempre encima, en el tema claro u oscuro del chat:

- El circulo grande con lo que quedara. Con una imagen, **arrastrar** la
  mueve y la **rueda** acerca hacia el raton; lo que se ve en el circulo es
  lo que se guarda (`pixpin_ui::foto_del_chat::Recorte`: el cuadrado nunca
  se sale de la foto).
- **Elegir imagen…** (el dialogo de Windows), **Pegar imagen** (o Ctrl+V:
  una imagen copiada o el fichero de una imagen copiado en el Explorador) y
  **Como siempre** (las iniciales en su color de siempre).
- **Fondo**: doce degradados (`FONDOS`, los siete de los avatares de
  Telegram y amarillo, marron, indigo, gris y grafito), detras del emoji o
  de las iniciales. Elegir uno con una imagen puesta vuelve al ultimo emoji
  (o a las iniciales).
- **Emojis**: una rejilla de unos cien de uso comun, con buscador encima;
  se escribe sin pulsarlo, en castellano o en ingles («casa», «obra»,
  «book»), sin tildes y por el principio de las palabras. La rueda sobre la
  rejilla la desplaza. Un emoji sin fondo elegido se lleva el fondo propuesto
  del chat (uno fijo por codigo).
- **Cancelar** (o Escape) y **Guardar** (o Intro). Guardar sin haber tocado
  nada no escribe nada.

**Donde se pinta.** En todos los sitios del avatar del chat, con la misma
funcion (`ventana_chat/logo.rs`, `logo::pintar`): la fila de la lista (y por
tanto los resultados de buscar chats), la pildora de la cabecera, la isla y
las filas de su pestana Proyectos. La chapa verde de chat compartido sigue
encima. Una imagen que aun no ha llegado por la sincronizacion se pinta como
siempre y se vuelve a mirar cada 3 s.

**Como se guarda y viaja.** En la ficha del proyecto (`resto["avatar"]` del
indice) y, por la red, como el campo `avatar` del `Proyecto`
(`crates/pixpin-proyecto/src/avatar.rs`). La imagen, cuadrada, recortada y a
256 px como mucho, es `<proyecto>/avatar-<ms>.png` y viaja con el proyecto.
El `logo.png` de antes sigue saliendo y empieza a viajar como `tipo: imagen`
hasta que se cambie. Formato y reglas para Android y Apple:
[manual-sincronizar-lo-nuevo.md §10](manual-sincronizar-lo-nuevo.md#10-la-foto-del-chat-25-oct-2026).

## Música con letra

Lo pidió el usuario en el móvil (27-oct-2026): «Quiero guardar música, con su
letra adjunta, para poder cantar cuando quiera». El PC hace lo mismo con el
**mismo formato**, sin campos nuevos (`apps/pixpin/src/ventana_chat/musica.rs`).

**El formato.** Una canción es una nota de voz (`clase: "VOZ"`) con
`estadoDelTexto: "letra"` y la letra en `transcripcion`: `[m:ss] línea`, un
renglón en blanco entre líneas (`[h:mm:ss]` pasada la hora); una marca sin
texto es un silencio y un párrafo sin marca es texto suelto. Lo lee la
pantalla de la letra de los dos aparatos y viaja como cualquier nota.

- **Adjuntar → «Música»** (chip de la hoja del «+»): el diálogo de Windows
  con los audios (`mp3`, `m4a`, `flac`, `ogg`, `wav`…). Entran ya marcados
  como música; un `.lrc` o `.txt` con el mismo nombre al lado del audio es
  su letra. Con una sola canción se abre su karaoke.
- **Pegar un LRC** en el lápiz de la letra: se convierte solo
  (`pixpin_voz::letra_de_cancion`: centésimas, varias marcas en una línea,
  `[offset:]`, `[ti:]`/`[ar:]`, marcas por palabra `<mm:ss.xx>`), redondeado
  al segundo, y la nota queda como música. Lo que no es LRC se guarda tal
  cual.
- **La tarjeta** de la burbuja: la portada que trae el fichero (ID3 `APIC`,
  `covr` de MP4, `PICTURE` de FLAC; `pixpin_voz::etiquetas`) o un degradado
  con la nota, y el título y el artista de sus etiquetas en vez de la onda.
  Su pastilla (icono de letra) abre el karaoke; pulsar la burbuja la toca.
- **El karaoke** (la pantalla de la letra de una canción): portada, título y
  artista arriba; la línea que suena grande y en color a un tercio de la
  altura, las cantadas apagadas; un clic en una línea lleva la canción ahí.
  Ctrl+C (o copiar) la lleva en LRC.
- **Sincronizar la letra** (el reloj de la cabecera, en el sitio de la
  banderita): suena desde el principio y **Espacio** (o «Marcar», o un clic
  en la lista) al empezar cada línea le pone su minuto; **Retroceso** deshace
  y vuelve dos segundos antes de la anterior; **Intro** guarda en `[m:ss]` y
  **Escape** lo deja. Un toque nunca queda antes que el anterior.

El auricular de llamadas del móvil es solo de allí: es del aparato y no
viaja.
