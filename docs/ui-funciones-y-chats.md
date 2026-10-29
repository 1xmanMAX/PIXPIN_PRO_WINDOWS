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

**Pestañas por tipo, abrir lo encontrado e ir al mensaje** (27-oct-2026, lo
de PixPin Android 8815b492 y 80e76094).

- **Pestañas** encima de la seccion, mientras se busca (el buscador con el
  teclado, algo escrito o una pestaña puesta): Todo, Fotos, Vídeos, Archivos,
  Enlaces, Voz y Dibujos, con cuantos hay en cada una («Fotos 3»). Que
  mensaje sale en cual es la regla de la ficha del chat
  (`cuaderno::indices_de_seccion`): un video no es foto ni archivo, lo del
  buzon solo sale en «Todo». Se filtra antes del tope de 20. Pildoras de
  28 pt repartidas en lineas si no caben; la puesta, del color de enviar.
  Pulsar la puesta vuelve a «Todo»; Escape en el buscador, tambien.
- **Con una pestaña puesta**, los chats de la lista no salen, y **sin
  escribir nada** salen todos los de esa clase de todos los chats. Sin
  escribir y en «Todo», nada (para eso esta la lista de chats).
- **Cada fila** lleva delante la cosa: la miniatura de la foto, el icono del
  formato de un archivo (el de su fila en el chat) o el de su tipo (video,
  voz, lienzo, pagina, carpeta, enlace); una nota, el avatar de su chat como
  antes. Arriba «en <chat>» con la foto del chat y la fecha; debajo el trozo
  (o su nombre, si no hay nada escrito y no tiene texto).
- **Pulsar** la abre en su visor, lo mismo que tocar su burbuja (se lleva el
  chat a ella y se toca); una nota con enlace abre el enlace. Una nota sin
  enlace, una nota de voz, un fichero que aun no llego al equipo o lo que no
  apunta a nada, al mensaje (`buscar_en_chats::al_pulsar`). **La flechita**
  de la derecha o **el clic derecho** llevan siempre al mensaje.
- **Lo de dentro de una carpeta** no sale suelto en el chat: el salto va a la
  tarjeta de su carpeta (`adonde_saltar`), venga de la busqueda o del
  universo.
- Clave nueva (los cuatro `main.ftl`): `chat-buscar-en` («en { $chat }» /
  «in { $chat }»); los nombres de las pestañas son los `info-*` de la ficha.

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
tanto los resultados de buscar chats), la pildora de la cabecera y la
cabecera de la isla. La chapa verde de chat compartido sigue
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
- **El karaoke** (la pantalla de la letra de una canción): **sin la tira de
  arriba** de las notas (la flecha con el nombre del fichero; 29-oct-2026, a
  petición del usuario). Arriba va la portada con el título y el artista de
  las etiquetas (sin etiquetas, nada: nunca el nombre del fichero) y, a la
  derecha, **A−**, **A+**, el reloj (sincronizar a toques), los subtítulos
  («Sincronizar sola»), el lápiz y una **✕** para volver; también vuelven
  Escape y la cabecera del chat, como en las demás pantallas de la letra.
  **Sin botón de copiar** (no lo quería); Ctrl+C sigue llevándola en LRC.
  La línea que suena va grande y en color a un tercio de la altura, las
  cantadas apagadas; un clic en una línea lleva la canción ahí.
- **Moverse por la letra**: la rueda, el panel táctil, **arrastrar** con el
  ratón y las teclas (Re Pág/Av Pág, flechas, Inicio, Fin), con y sin
  tiempos. Mientras suena, la vista sigue a la línea que suena; moverla a
  mano lo para **6 s** (o hasta pulsar una línea) y luego vuelve sola.
  Parada, se queda donde se dejó. *El fallo que había* (29-oct-2026): con
  tiempos, el karaoke calculaba la vista en cada fotograma desde la línea
  que suena y lo que la rueda escribía en `scroll` no se leía nunca; ahora
  se pinta desde `scroll` mientras dura lo movido a mano y se apunta lo
  pintado (`LetraAbierta::pintado`) para que mover empiece desde lo que se
  ve (`mover_la_letra`, `sigue_a_la_cancion` en `ventana_chat.rs`).
- **Sincronizar la letra** (el reloj): suena desde el principio y
  **Espacio** (o «Marcar», o un clic en la lista) al empezar cada línea le
  pone su minuto; **Retroceso** deshace y vuelve dos segundos antes de la
  anterior; **Intro** guarda en `[m:ss]` y **Escape** lo deja. Un toque
  nunca queda antes que el anterior. Sirve también para corregir lo que
  ponga «Sincronizar sola».
- **Sincronizar sola** (los subtítulos; 29-oct-2026): Whisper oye la
  canción en su hilo y la letra pegada **se alinea** con lo oído; cada línea
  se lleva un minuto aproximado y se guarda en el mismo `[m:ss] línea` (el
  móvil la canta sin cambiar nada). **El texto guardado es siempre el
  pegado**, nunca lo que oyó Whisper. Arranca sola al pegar (lápiz) una
  letra sin tiempos si el reconocedor ya está bajado; si no, el aviso dice
  que su botón lo baja (una vez, unos 160 MB). Se ve el avance en una
  pastilla bajo la cabecera («Sincronizando la letra… 40 %»); pulsar otra
  vez el botón lo para, y cerrar la conversación también. Al acabar, el
  aviso dice cuántas líneas se oyeron («12 de 30»); con menos de un tercio,
  que va a ojo. Si la letra se edita mientras, no se guarda. El idioma con
  el que se oye es el de la letra si se nota (`idioma_de_la_letra`: una
  canción en inglés se oye en inglés con PixPin en castellano) y si no, el
  de Ajustes con su segundo idioma.

### Cómo se alinea la letra con lo oído (`pixpin_voz::alinear_letra`)

Puro, sin sonido ni ventanas, con sus pruebas (errores del reconocedor,
estribillos repetidos, líneas que no se oyen, coros de fondo y «yeah»,
huecos instrumentales, rótulos, el final de la canción).

**Entra**: las líneas de la letra (texto, en orden: las mismas que
sincronizar a toques, un renglón por línea) y las palabras oídas (texto y
milisegundo en que empiezan), más lo que dura la canción si se sabe.
**Sale**: por línea, un milisegundo, una confianza de 0 a 1 y si es un
ancla (sale de palabras oídas) o se repartió; o nada si ninguna línea se
pudo anclar.

1. **Lo oído.** Whisper (`whisper::oir_cancion`) oye la canción en tandas
   de frases de hasta 8 s (cortadas en los silencios, `cortes.rs`); cada
   tanda trae su texto con principio y fin, y dentro se reparte el tiempo
   de cada palabra según lo larga que es (`palabras_oidas`).
2. **Normalizar** las dos partes: minúsculas, sin tildes ni puntuación, el
   apóstrofo junta (`can't` → `cant`), los «x2» no son palabras. Una línea
   que es solo un rótulo entre corchetes (`[Coro]`) no se canta.
3. **Needleman–Wunsch palabra a palabra**, monótono: una pareja suma
   `1 + parecido` si se parecen (Levenshtein, desde 0,65; las palabras de
   una o dos letras solo exactas y valen 0,5), una pareja distinta resta
   0,9, una palabra de la letra sin pareja resta 0,6 y una oída sin pareja
   0,4. Lo oído antes de la primera palabra y después de la última no
   cuesta. Un estribillo repetido casa con su vez porque el camino nunca
   vuelve atrás.
4. **Anclas**: confianza de una línea = suma de los parecidos de sus
   palabras casadas / sus palabras; ancla con ≥ 0,4 y al menos dos casadas
   (una si la línea tiene una). Su minuto es el de su primera palabra
   casada menos lo que se tarda en cantar las de delante (al ritmo de la
   canción), nunca antes que el ancla anterior.
5. **El ritmo** (ms por letra): la mediana de lo medido dentro de las
   líneas y entre dos líneas seguidas ancladas (hasta 20 s); 70 si no hay,
   entre 35 y 300.
6. **Las líneas sin ancla**: entre dos anclas, repartidas por letras pero
   nunca más despacio que el ritmo (así un solo de guitarra queda antes de
   la estrofa siguiente y no estira la anterior); antes de la primera,
   hacia atrás al ritmo; después de la última, al ritmo sin pasar del final.
   Un rótulo va con la línea que lo sigue. Todo en orden.

**Para Android**: es el mismo cálculo, sin dependencias, y se porta tal
cual a `guardados/LetraDeCancion.kt` (las constantes están arriba de
`alinear_letra.rs`); las palabras oídas pueden salir de su `MotorWhisper`
por tandas igual que aquí. El resultado se guarda en el formato de siempre,
así que un PC y un móvil pueden sincronizar cada uno a su manera y leer lo
del otro.

El auricular de llamadas del móvil es solo de allí: es del aparato y no
viaja.

## Imprimir desde el botón de marcos

El botón del marco del riel de los visores (el de los marcos numerados)
sigue igual al pulsarlo: entra y sale de poner marcos. **El clic derecho**
(en Android y en el Mac, la **pulsación larga**) saca un menú pequeño con
dos cosas:

- **«Imprimir los marcos (N)»**: lo mismo que Ctrl+P con marcos, una hoja
  por marco. Sin ningún marco sale en gris, con la pista «aún no hay: pulsa
  el marco y arrastra».
- **«Imprimir el … entero»**, con el nombre de lo que es: «el PDF entero»,
  «el Word entero», «el PowerPoint entero», «la hoja entera» (una hoja de
  cálculo), «la página entera» (una página web), «el documento entero» (un
  libro, unas notas, un formato viejo de Office), «el plano entero» y, en
  un modelo 3D, «la vista del modelo».

En Windows se quedan además los atajos y botones de imprimir de siempre
(Ctrl+P, «Imprimir» del menú ⋯); en Android solo queda este botón. Los
textos van en los cuatro catálogos (`marcos-menu-*`).

Cómo imprime entero cada visor en Windows:

| Visor | Entero |
| --- | --- |
| PDF (`lector_pdf`) | El diálogo de imprimir con todas las hojas y su tinta (`lector_pdf/entero.rs`). |
| Word y PowerPoint (`lector_pdf`) | El imprimir de siempre (`office::imprimir`). |
| Word, libros y notas leídos como texto (`visor`) | El texto en trozos de papel, fotografiados como los marcos. |
| Hoja de cálculo (`visor_hoja`) | Lo usado de la hoja abierta, en trozos de papel (A4 tumbado). |
| Página web y notas (`visor_html`, `notas_md/lector_web`) | El imprimir de la página (`window.print()`), con lo anotado dentro. |
| Pin de Office (formatos viejos) | Su aplicación (el verbo «Imprimir» de Windows). |
| Plano (`pixpin-cad`) | El plano entero en una lámina, como Ctrl+P sin marcos. |
| Modelo 3D (`pixpin-cad`) | La vista de ahora. |

El menú lo hacen `marcos_doc::menu_de_imprimir` (las entradas en
`marcos_doc::entradas_de_imprimir`) y, en el plano y el modelo,
`pixpin_cad::ventana::menu_de_imprimir` con los mismos textos.

## El menú del mensaje según su tipo

*29-oct-2026, como el móvil (442eadf, `OpcionesDelMensaje.kt`).* El usuario
(checklist 45): «mantengo pulsado un PDF y me sale "recordatorio", "llamada
falsa"… es absurdo y solo ocupa espacio; quita lo que no aplica».

El orden del menú (clic derecho sobre una burbuja) ya era el del PC y el
móvil lo copió. Lo que cambia es **qué sale en cada tipo**; la decisión vive
en `apps/pixpin/src/ventana_chat/opciones_del_mensaje.rs` (`tipo_de`,
`recordar`, `leccion`), con pruebas:

| Tipo | Recordármelo / llamada secreta | Hacer lección |
| --- | --- | --- |
| Nota, mini-app (lista, tabla) | a la vista | nota: a la vista; mini-app: no |
| Enlace (nota con una dirección web) | a la vista | en «Más» |
| Nota de voz | a la vista (es la llamada secreta) | a la vista |
| Clase o reunión grabada | no | a la vista |
| Canción | no | no |
| Foto, hoja (lienzo, página, croquis) | en «Más» | en «Más» |
| Documento (PDF, Word, libro…), archivo, vídeo | no | no |
| Carpeta (cuaderno, grabación, proyecto) | no | no |
| Respuesta del asistente | no | en «Más» |

Con un recordatorio ya puesto (de antes o de otro aparato), «Quitar el
recordatorio» sale siempre, para que haya por dónde quitarlo.

**Una sola pantalla para una clase** (como el móvil 4e830ea): en la nota de
una clase grabada ya no sale «Letra» en «Más»; «Estudiar la clase» es su
pantalla (texto con sus minutos, capturas y marcadores). Una nota de voz y
una canción siguen con su letra (el karaoke del PC, ver «Música con letra»).

## Compartir un cuaderno

*29-oct-2026, como el móvil (201d327, `CompartirCarpeta.kt`).* El usuario
(checklist 51): «compartir un cuaderno con todos los formatos (PDF, HTML,
PixPin, fotos…), con la misma hoja de compartir de siempre».

Un cuaderno (una carpeta de páginas del chat) entra en la hoja de compartir
de toda la app con sus páginas, cada una con su título («Día 1», «Día 2»),
todas marcadas al abrir (`apps/pixpin/src/compartir/carpeta.rs`,
`preparar`):

- **PDF, página web, PNG, JPG y SVG**, con la regla del cuaderno: de un
  lienzo, lo que va sobre su plantilla o sus marcos (si no, todo); una foto,
  en una sola página.
- **Word, Markdown y Texto**, si alguna página lleva texto (una nota, la
  transcripción de una nota de voz), cada página bajo su título.
- **Original**: los ficheros de sus páginas tal cual.
- **PixPin (.pixpin)**: el chat entero, con la carpeta dentro, que se abre
  en otro PC y en el móvil con su carpeta. Y por Wi‑Fi, el botón de siempre.

Se llega desde tres sitios, y los tres dan la misma hoja:

1. «Compartir» en el menú del mensaje del cuaderno (antes mandaba el JSON
   de la carpeta como texto).
2. Lo marcado en el chat: una carpeta marcada junto a otros mensajes va con
   sus páginas (sin repetir las que ya estén marcadas).
3. El botón **«Compartir»** de la ventana del cuaderno, junto a «Exportar
   PDF» (que sigue: guarda el PDF con el índice de títulos sin pasar por la
   hoja).

La carpeta de una **grabación** se comparte como la grabación entera (la
página web con el reproductor, el PDF y el Word con las capturas en su
minuto…). No hay campos nuevos: nada cambia en lo que se sincroniza.
