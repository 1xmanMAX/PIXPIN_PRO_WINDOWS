# Telegram Desktop: estructura, medidas, colores y técnicas de fluidez (para la ventana de chat de PixPin)

Fecha: 15-sep-2026. Estudio de `telegramdesktop/tdesktop`, commit `272f6f5c2d29d8cdb3aec15907d616b87451a3ca`
(10-sep-2026), y su submódulo `desktop-app/lib_ui` en el commit fijado por tdesktop,
`09f35036fd4f2d2f67422d294740f9a92c38827f` («Lend the kept paragraphs out instead of sharing them.»).
Clon disperso en el scratchpad de la sesión.

> **Licencia.** Telegram Desktop y lib_ui son GPL-3.0. PixPin Max es MIT. De aquí solo se toman **medidas,
> colores, estructura y técnicas explicadas con palabras propias**. No se copia código, ni iconos, ni
> imágenes, ni sonidos, ni el nombre o la marca. Las medidas (números de píxeles, hex de colores) son hechos
> de diseño, no expresión protegida; aun así conviene no calcar la paleta completa sino elegir la nuestra
> con estos valores como referencia.

Rutas abreviadas:
- `tg:` = `tdesktop/Telegram/SourceFiles/`
- `ui:` = `lib_ui/ui/`
- `night:` = `colors.tdesktop-theme` dentro de `tdesktop/Telegram/Resources/night.tdesktop-theme` (zip)

Unidades: en los `.style` las medidas van en `px` lógicos a escala 100 % (Telegram multiplica por la escala
del sistema). Fuente base: Open Sans en Telegram; en Windows cae a Segoe UI si se configura la del sistema.

## Índice

- [a. Disposición de la ventana principal](#a-disposición-de-la-ventana-principal)
- [b. Fila de la lista de chats](#b-fila-de-la-lista-de-chats)
- [c. Burbuja de mensaje](#c-burbuja-de-mensaje)
- [d. Paleta de colores (claro y oscuro)](#d-paleta-de-colores-claro-y-oscuro)
- [e. Tipografía](#e-tipografía)
- [f. Técnicas de fluidez](#f-técnicas-de-fluidez)
- [g. Modelo de datos conceptual](#g-modelo-de-datos-conceptual)
- [h. Traducción a PixPin](#h-traducción-a-pixpin)

## a. Disposición de la ventana principal

### Ventana

| Medida | Valor | Fuente |
|---|---|---|
| Ancho / alto mínimos de la ventana | 380 × 480 | `tg:window/window.style:13-14` |
| Tamaño por defecto | 800 × 600 (1024 × 768 en pantallas grandes) | `tg:window/window.style:15-18` |

### Columnas

Hay hasta cuatro franjas verticales, de izquierda a derecha:

1. **Barra de carpetas** (opcional, solo si el usuario tiene carpetas y elige la vista lateral): 72 px fijos
   (`windowFiltersWidth`, `tg:window/window.style:229`). Cada botón mide 62 px de alto mínimo, con icono arriba
   y texto de 11 px seminegrita debajo (`tg:window/window.style:230-245`). Se descuenta del ancho útil antes de
   repartir (`tg:window/window_session_controller.cpp:2626`, `:3287-3288`). La alternativa es una tira de
   pestañas horizontal encima de la lista (`chatsFiltersTabs`, `tg:dialogs/dialogs.style:780`).
2. **Lista de chats** (dialogs).
3. **Conversación** (history).
4. **Tercera columna** (info del chat o panel de emojis), opcional.

| Columna | Mínimo | Máximo | Fuente |
|---|---|---|---|
| Lista de chats | 260 px | 540 px (declarado; el tope está comentado en el código) | `tg:window/window.style:34-35`; `tg:window/window_session_controller.cpp:2681-2682` |
| Conversación | 380 px | — | `tg:window/window.style:36`; también `historyMinimalWidth: 380px` (`tg:ui/chat/chat.style:179`) |
| Tercera columna | 292 px | 392 px | `tg:window/window.style:37-38` |
| Lista «estrecha» (solo avatares) | `padding.left + photoSize + padding.left` = 10+46+10 = 66 px | — | `tg:window/window_session_controller.cpp:2610-2614` |
| Asa para arrastrar el borde | 6 px de ancho, cursor ↔ | `tg:ui/chat/chat.style:203`; `tg:ui/resize_area.h:14-17` |

### Cómo se reparte al redimensionar (`computeColumnLayout`, `tg:window/window_session_controller.cpp:2623-2671`)

- `bodyWidth` = ancho de la ventana − barra de carpetas.
- **Una columna** si `bodyWidth < 260 + 380 = 640` (`:2629-2636`): lista y chat ocupan todo el ancho y se
  alterna entre una y otra (como en móvil).
- **Normal (dos columnas)** si no cabe la tercera (`bodyWidth < 260+380+292 = 932`) o si está desactivada
  (`:2638-2648`): `anchoLista = bodyWidth × proporción`, con suelo de 260 y sin comerse los 380 del chat
  (`:2654-2656`); el chat se queda con el resto.
- **Tres columnas**: la lista por proporción, la tercera por su ancho guardado (acotado 292–392), y se encogen
  ambas si hace falta para que el chat conserve 380 (`:2657-2668`, `:2685-2714`).
- La **proporción** por defecto es `5/14 ≈ 0,357` del ancho (`0,275` si arranca con tercera columna)
  (`core/core_settings.h:1077-1078`, `core/core_settings.cpp:1775-1779`). Se guarda **por separado** con chat
  abierto y sin chat abierto (`nochat`) (`core/core_settings.cpp:161-162`).
- Al **arrastrar el asa** la nueva proporción es `ancho / anchoTotal`; si se arrastra por debajo de 130 px
  (la mitad del mínimo) la proporción pasa a 0 y la lista se pliega a la columna estrecha de avatares
  (`mainwidget.cpp:2885-2890`, en `ensureFirstColumnResizeAreaCreated`). El asa se coloca en el borde izquierdo
  de la conversación (`mainwidget.cpp:2841-2860`).
- Al abrir la tercera columna **intenta ensanchar la ventana** sin moverla antes que robar sitio al chat, y
  reescala la proporción para que la lista no cambie de ancho en píxeles
  (`tg:window/window_session_controller.cpp:2748-2776`); al cerrarla deshace lo mismo (`:2786-2815`).
- El cambio de ancho de la lista se anima (`dialogsWidthDuration: universalDuration`,
  `tg:dialogs/dialogs.style:214`; `mainwidget.cpp:2670-2677`).
- Umbral «chat ancho» (mensajes centrados con ancho máximo en vez de pegados): `adaptiveChatWideWidth: 880px`
  (`tg:window/window.style:40`).

### Cabecera de la lista y barra de búsqueda

- La franja superior de la lista tiene el **mismo alto que la cabecera del chat**: `filterAreaHeight =
  st::topBarHeight` = 54 px (`tg:dialogs/dialogs_widget.cpp:4540`; `tg:info/info.style:908`). Así las dos
  cabeceras forman una línea continua.
- Botón de menú (hamburguesa) a la izquierda: 40 × 40, con `dialogsFilterPadding` 7 px
  (`tg:dialogs/dialogs.style:92`, `:283-294`; colocación `tg:dialogs/dialogs_widget.cpp:4533-4561`).
- Campo de búsqueda centrado verticalmente en esos 54 px (`dialogs_widget.cpp:4549`): **pastilla** de radio
  18 px, alto mínimo 35 px, márgenes de texto 12/8/30/5 (el hueco derecho de 30 px es para la ✕), fondo
  `filterInputInactiveBg` → `filterInputActiveBg` al enfocarlo, borde de 2 px `windowBgRipple` activo
  (`tg:dialogs/dialogs.style:310-334`). El texto de ayuda se desliza 50 px al escribir (`placeholderShift`).
- Separación a la derecha del campo: `dialogsFilterSkip 4px + 7px` (`dialogs_widget.cpp:4537-4538`).
- Resultados agrupados con barras de título de 28 px (`searchedBarHeight`, `tg:dialogs/dialogs.style:804`).

### Cabecera de la conversación

- Alto 54 px (`topBarHeight`, `tg:info/info.style:908`). Botón atrás de 52 px de ancho en modo una columna
  (`tg:info/info.style:905-907`). Botones de acción (buscar, llamar, menú) de 40 × 54 con zona de onda de 40 px
  centrada (`topBarSearch`, `tg:info/info.style:917-927`). Avatar pequeño + nombre (negrita) + estado debajo.

### Campo de escribir (compose)

- Separación arriba y abajo del campo: `historySendPadding: 9px` (`tg:chat_helpers/chat_helpers.style:1302`).
- Campo sin borde, estilo de texto de los mensajes, alto de 36 px que crece hasta 72 px normal y
  **224 px como máximo** con desvanecido de 6 px al hacer scroll interno
  (`tg:chat_helpers/chat_helpers.style:1279-1300`); el máximo real es el menor entre 224 y lo que quepa
  (`tg:history/history_widget.cpp:8905-8909`).
- Botón enviar 44 × 46, margen derecho 2 px (`tg:chat_helpers/chat_helpers.style:1301`, `:1403`). A la
  izquierda el clip de adjuntar y a la derecha del campo el de emojis (`historyAttach`, `:1305`;
  `historyAttachEmoji`, `:1385`).
- Barra de «respondiendo a / editando» encima del campo: 49 px (`historyReplyHeight`, `:1128`;
  colocación `history_widget.cpp:6365`).
- El fondo del área de escribir es `historyComposeAreaBg` y la cuadrícula del chat encoge su alto en
  `alto del campo + 2 × 9` (`history_widget.cpp:8463`, `:11119-11120`).

## b. Fila de la lista de chats

### Geometría (`defaultDialogRow`, `tg:dialogs/dialogs.style:95-103`)

```
 ← 10 →┌──────┐← 12 →Nombre del chat (seminegrita)      [📌][✓✓] 14:32   ← 10 →
   ↑8  │avatar│      Último mensaje en una línea, cortado con…     ( 3 )
       │ 46px │                                                         
   ↓8  └──────┘                                                         
 alto total 62 px
```

| Elemento | Valor | Fuente |
|---|---|---|
| Alto de fila | **62 px** | `dialogsRowHeight`, `tg:dialogs/dialogs.style:91` |
| Márgenes | izq. 10, arriba 8, der. 10, abajo 8 | `defaultDialogRow.padding`, `:97` |
| Avatar | **46 px**, redondo, en (10, 8) | `photoSize`, `:98` |
| Nombre | x = 68, y = 10 (arriba de la línea) | `nameLeft/nameTop`, `:99-100` |
| Último mensaje | x = 68, y = 34 | `textLeft/textTop`, `:101-102` |
| Ancho útil del texto | `ancho − 68 − 10` | `tg:dialogs/ui/dialogs_layout.cpp:563` |
| Ancho mínimo del texto | 150 px | `dialogsTextWidthMin`, `tg:dialogs/dialogs.style:216` |
| Variante con etiquetas de carpeta | 72 px de alto, texto a y = 30, etiqueta a y = 52 | `taggedDialogRow`, `:114-118` |
| Variante foro (con temas) | 80 px | `forumDialogRow`, `:119-125` |

### Nombre, hora y marcas

- **Nombre** con la fuente seminegrita (`st::semiboldFont`), cortado con puntos suspensivos al ancho
  disponible (`tg:dialogs/ui/dialogs_layout.cpp:564-568`, `:918-929`). Colores `dialogsNameFg` /
  `dialogsNameFgOver` / `dialogsNameFgActive` según estado normal / ratón encima / seleccionado.
- **Hora** arriba a la derecha, fuente de 13 px (`dialogsDateFont`, `tg:dialogs/dialogs.style:88`), separada
  5 px del nombre (`dialogsDateSkip`, `:89`). Se pinta primero y **se resta del ancho del nombre**, de modo que el
  nombre nunca la pisa (`PaintRowTopRight`, `tg:dialogs/ui/dialogs_layout.cpp:87-106`). Su línea base se alinea
  con la del nombre (`:105`). Colores `dialogsDateFg` / `…Over` / `…Active`.
- **Estado del último mensaje enviado** (una o dos marcas) a la izquierda de la hora, reserva 20 px
  (`dialogsSendStateSkip`, `tg:dialogs/dialogs.style:400-410`).
- **Icono de tipo de chat** (grupo, canal, bot) antes del nombre, separación 3 px (`dialogsChatTypeSkip`, `:383`;
  `dialogs_layout.cpp:597-605`).

### Último mensaje

- Una sola línea con la fuente normal (`dialogsTextFont: normalFont`, `tg:dialogs/dialogs.style:86-87`).
- En grupos va precedido del autor («Ana: …») con el color de «servicio» `dialogsTextFgService`
  (`tg:dialogs/ui/dialogs_layout.cpp:691-694`); el resto en `dialogsTextFg` (`:356-359`).
- Si hay **borrador**, se muestra «Borrador: …» con `dialogsDraftFg` (`tg:dialogs/dialogs_inner_widget.cpp:1911`).
- Puede llevar una miniatura pequeña de la foto/vídeo del mensaje delante del texto
  (`dialogs/ui/dialogs_message_view.cpp`).

### Contador de no leídos

- **Pastilla**: alto 19 px, relleno lateral 5 px, texto 12 px negrita (`tg:dialogs/dialogs.style:78-80`).
  Ancho = `max(anchoTexto + 2×5, 19)` → con una cifra es un **círculo** de 19 px y con más se alarga
  (`tg:ui/unread_badge_paint.cpp:144-155`, `:157-179`).
- Se dibuja con **dos medios círculos precoloreados y cacheados** (izquierdo y derecho) más un rectángulo en
  medio; hay una caché por combinación silenciado × (normal/encima/seleccionado)
  (`tg:ui/unread_badge_paint.cpp:97-136`). Técnica barata: no se rasteriza una curva por fila.
- Posición: a la derecha de la **segunda línea**, con su centro alineado a la línea del texto
  (`PaintWideCounter`, `tg:dialogs/ui/dialogs_layout.cpp:322-343`). En la columna estrecha va sobre la esquina
  inferior derecha del avatar (`PaintNarrowCounter`, `:303-320`).
- Colores: `dialogsUnreadBg` (acento) si el chat suena, `dialogsUnreadBgMuted` (gris) si está silenciado; texto
  `dialogsUnreadFg` (blanco) (`dialogs_layout.cpp:208-219`; `unread_badge_paint.cpp:183-187`). Versiones
  `…Over` y `…Active` para los otros estados.
- «Marcado como no leído» sin número: un punto de diámetro `unreadMarkDiameter` (`dialogs_layout.cpp:197-222`).
- Menciones y reacciones: insignias aparte con su propio color (`dialogsUnreadMention`, `dialogsUnreadReaction`,
  `dialogs_layout.cpp:250-282`).
- Contador de más de N cifras se abrevia a «..NN» (`ComputeUnreadBadgeText`, `unread_badge_paint.cpp:89-94`).

### Estados de la fila

- Fondo: `dialogsBgActive` (seleccionado, color acento; el texto se vuelve blanco), `dialogsBgOver` (ratón encima)
  o el fondo normal (`tg:dialogs/ui/dialogs_layout.cpp:467-472`, `:486`).
- Al pulsar, **onda (ripple)** que crece desde el punto del clic con `dialogsRippleBg` o `dialogsRippleBgActive`
  (`dialogs_layout.cpp:487-492`; `dialogsRipple`, `tg:dialogs/dialogs.style:82-84`).
- No hay líneas separadoras entre filas: el ritmo lo dan los 62 px y el avatar.
- Indicador «en línea»: punto de 10 px con aro de 2 px en la esquina del avatar, aparece con 150 ms
  (`tg:dialogs/dialogs.style:185-188`).

### Fijados

- Los fijados van **arriba**, en el orden que decide el usuario (`PinnedList`, `tg:dialogs/dialogs_pinned_list.h:22-51`).
  Fijar coloca el chat primero (`:32-33`).
- En la fila, si no hay contador, se dibuja un **icono de chincheta** en el sitio del contador con el gris de
  silenciado (`dialogsPinnedIcon`, `tg:dialogs/dialogs.style:411-415`; `dialogs_layout.cpp:237-243`).
- **Reordenar arrastrando**: empieza tras mover 30 px (`kStartReorderThreshold`,
  `tg:dialogs/dialogs_inner_widget.cpp:110`, `:2674`); la fila arrastrada sigue al ratón y las demás se desplazan
  con una animación de 200 ms curva seno entrada-salida (`stickersRowDuration`,
  `tg:chat_helpers/chat_helpers.style:484`; `dialogs_inner_widget.cpp:2902-2915`).

## c. Burbuja de mensaje

### Márgenes y anchos (`tg:ui/chat/chat.style`)

| Medida | Valor | Fuente |
|---|---|---|
| Ancho máximo de burbuja | **430 px** | `msgMaxWidth`, `:14` |
| Ancho mínimo de burbuja | 160 px | `msgMinWidth`, `:22` |
| Relleno interior (texto ↔ borde) | izq. 11, arriba 8, der. 11, abajo 8 | `msgPadding`, `:25` |
| Margen exterior del mensaje | izq. 16, arriba 6, der. 56, abajo 2 | `msgMargin`, `:52` |
| Margen superior si va pegado al anterior | 0 | `msgMarginTopAttached`, `:53` |
| Sombra bajo la burbuja | 2 px (franja inferior, color `msgInShadow`/`msgOutShadow`) | `msgShadow`, `:54` |
| Avatar del autor (grupos) | 33 px; desplaza la burbuja 40 px | `msgPhotoSize/msgPhotoSkip`, `:23-24` |
| Radio grande / pequeño | **16 px** / 6 px (`roundRadiusLarge`) | `bubbleRadiusLarge/Small`, `:324-325`; `ui:basic.style:107` |
| Espacio bajo el último mensaje | 8 px | `historyPaddingBottom`, `:205` |
| Barra de scroll del historial | 12 px de zona, barra redondeada 3 px, se desvanece | `historyScroll`, `:187-201` |

- **Colocación horizontal** (`Message::countGeometry`, `tg:history/view/history_view_message.cpp:6455-6520`):
  el ancho disponible es `ancho − 16 − 56` (o `− 16 − 16` en modo estrecho, `:6463-6470`). La burbuja se encoge
  al ancho real del texto + relleno (`:6490-6500`) sin pasar de 430. Los **salientes se empujan a la derecha**
  (`contentLeft += disponible − ancho`, `:6502-6505`); los entrantes quedan a la izquierda. En modo «chat ancho»
  (≥ 880 px) la columna de mensajes se centra con ancho máximo `430 + 2×40 + 2×16`
  (`tg:history/view/history_view_element.cpp:787-794`).
- Es un **único cálculo de geometría** reutilizado para pintar, para detectar el ratón y para el menú
  (`countGeometry()` se llama desde muchos sitios, p. ej. `history_view_message.cpp:773`, `:1170`, `:3237`).

### Esquinas, cola y agrupación

- Cada esquina tiene uno de cuatro tipos: ninguna, **cola**, pequeña, grande
  (`BubbleCornerRounding`, `tg:ui/chat/message_bubble.h`).
- Regla (`Message::countMessageRounding`, `tg:history/view/history_view_message.cpp:6523-6551`):
  - Si el mensaje va **pegado al anterior**, la esquina superior del lado del autor es pequeña (6 px).
  - Si va **pegado al siguiente**, la esquina inferior de ese lado es pequeña.
  - Si es el **último del grupo**, la esquina inferior del lado del autor lleva la **cola** (un pico dibujado
    con un icono aparte del color de la burbuja, `historyBubbleTailInLeft`/`OutRight`,
    `tg:ui/chat/chat_style.style:216-219`), salvo que el medio lo impida (foto sin texto, botones).
  - El resto, grande (16 px).
- **Cuándo se agrupa** (`Element::computeIsAttachToPrevious`, `tg:history/view/history_view_element.cpp:2264-2348`):
  mismo autor (`:2333`), menos de **900 s (15 min)** entre ambos (`kAttachMessageToPreviousSecondsDelta`, `:99`,
  `:2309-2311`), ninguno es mensaje de servicio, y **no hay separador de fecha ni barra de no leídos** entre ellos
  (`:2277-2280`). Se recalcula solo para el vecino al insertar/quitar (`recountAttachToPreviousInBlocks`, `:2436-2452`).
- En grupos, el avatar del autor aparece solo junto al último mensaje del bloque y el nombre solo en el primero.

### Hora y marcas dentro de la burbuja

- Hora con fuente de 13 px (`msgDateFont`, `tg:ui/chat/chat.style:21`) en la esquina inferior derecha, a
  `(relleno − msgDateDelta)` = 9 px del borde derecho y 3 px del inferior (`msgDateDelta: point(2px, 5px)`, `:100`;
  `tg:history/view/history_view_message.cpp:5387-5388`).
- **Truco del «bloque de salto»**: al maquetar el texto se reserva al final de la última línea un hueco invisible
  del ancho de la hora (+ marcas). Si cabe en la última línea, la hora queda a su derecha; si no, el texto
  crece una línea (`skipBlockWidth/Height`, `tg:history/view/history_view_element.cpp:1509-1520`,
  `validateTextSkipBlock` `:2211-2219`). Así la burbuja nunca desperdicia una línea entera para la hora.
- Separación mínima texto ↔ hora 12 px (`msgDateSpace`, `:99`). Las marcas de enviado/leído (una o dos) ocupan
  24 px más a la derecha de la hora (`historySendStateSpace`, `:238`; `history_view_bottom_info.cpp:175`, `:293`).
- **Sobre una foto** la hora va en una pastilla oscura semitransparente, a 4 px del borde con relleno 8×2
  (`msgDateImgDelta/msgDateImgPadding`, `:102-103`; `history_view_message.cpp:5392-5393`), color `msgDateImgBg`.

### Separadores de fecha y barra de no leídos

- **Fecha**: una «píldora de servicio» centrada con texto seminegrita, relleno 12/3/12/4 y margen 10/10/10/2
  (`msgServicePadding/msgServiceMargin`, `tg:ui/chat/chat.style:95-96`). Su radio es la **mitad de su alto**
  (forma de cápsula) (`HistoryServiceMsgRadius`, `tg:ui/chat/chat_style.cpp:92-100`). Alto total
  = margen + relleno + fuente (`DateBadge::height`, `tg:history/view/history_view_element.cpp:823-829`).
  Fondo `msgServiceBg` (semitransparente sobre el fondo del chat), texto `msgServiceFg`.
- Se inserta como un «adorno» del **primer mensaje de cada día**, no como un elemento aparte (`DateBadge` es un
  componente del mensaje, `history_view_element.cpp:2417-2424`).
- **Fecha flotante**: al hacer scroll se muestra la fecha del mensaje de arriba pegada al borde superior; se
  oculta 1 s después de parar con un fundido de 200 ms (`historyScrollDateHideTimeout/historyDateFadeDuration`,
  `tg:ui/chat/chat.style:310-311`).
- **Barra «Mensajes no leídos»**: franja a todo el ancho de 32 px + 8 px de margen, texto centrado seminegrita,
  fondo `historyUnreadBarBg` y línea inferior `historyUnreadBarBorder`
  (`tg:ui/chat/chat.style:351-353`; `UnreadBar::paint`, `history_view_element.cpp:760-815`).

### Mensajes con imagen

- La foto se escala a lo ancho entre **100 y 430 px** (`minPhotoSize/maxMediaSize`, `tg:ui/chat/chat.style:168-169`),
  conservando proporción; con pie de foto la burbuja mide al menos 200 px (`historyPhotoBubbleMinWidth`, `:328`).
- Sin texto, la foto **es** la burbuja: sus esquinas siguen la misma regla de redondeo y no hay cola
  (`media->skipBubbleTail()`, `history_view_message.cpp:6531`).
- Álbum: rejilla de fotos con 4 px de separación (`historyGroupSkip`, `:508`), ancho 100–430.

### Mensajes con archivo (`msgFileLayout`, `tg:ui/chat/chat.style:378-392`)

| Variante | Relleno | Miniatura | Nombre (y) | Tamaño/estado (y) |
|---|---|---|---|---|
| Archivo sin vista previa | 12/8/10/8 | icono redondo 44 px, separado 11 px | 12 | 34 |
| Archivo con miniatura | 6/6/10/6 | miniatura 72 px, radio 12 (4 si pegado), separada 14 px | 11 | 31 (enlace «Abrir» a 55) |

- Ancho mínimo 268 px (`msgFileMinWidth`, `:421`); radios de miniatura 4 y 12 (`:376-377`).
- Nombre en seminegrita, cortado por el medio si no cabe; debajo «1,2 MB · PDF» en gris `msgInDateFg`/`msgOutDateFg`.
- El círculo de 44 px muestra el icono de descarga/abrir y, durante la descarga, un **arco de progreso** de 3 px
  (`msgFileRadialLine`, `:425`); el paso ratón-encima se anima 200 ms (`msgFileOverDuration`, `:424`).

### Respuesta citada (dentro de la burbuja)

- Bloque encima del texto, relleno 11/2/6/2 y 2 px arriba y abajo (`historyReplyPadding/Top/Bottom`,
  `tg:ui/chat/chat.style:63-73`).
- Estilo de cita: **barra vertical de 3 px** a la izquierda, radio 5 px, relleno 10/2/4/2 (`historyQuoteStyle`,
  `tg:chat_helpers/chat_helpers.style:1254-1260`); el fondo es el color del autor con poca opacidad y el nombre
  va en ese color.
- Si el mensaje citado tiene imagen, miniatura de 32 px con margen 7/4/4/4 (`historyReplyPreview/Margin`,
  `tg:ui/chat/chat.style:65-66`; uso en `tg:history/view/history_view_reply.cpp:662-666`).
- Dos líneas: nombre del autor + primera línea del texto citado (`history_view_reply.cpp:725-765`).
- Pulsar la cita salta al mensaje original y lo resalta brevemente.

## d. Paleta de colores (claro y oscuro)

**Cómo está hecho.** Hay un único fichero de paleta con ~700 nombres semánticos (`ui:colors.palette`), cada uno
con un valor por defecto que puede ser un hex (`#rrggbb` o `#rrggbbaa`) **o el nombre de otro color** (alias).
Los temas (`.tdesktop-theme`, un zip con `colors.tdesktop-theme` + `background`) solo sobrescriben los nombres que
cambian. Cambiar de tema = recargar la tabla y repintar; ningún widget tiene colores fijos. Técnica útil para
PixPin: **nombres por función, no por tono**, y alias para que un solo acento arrastre a todo.

- Tema claro («Clásico», el de por defecto): valores de `ui:colors.palette` (commit de lib_ui citado). El fondo del
  chat es un **fondo con patrón** (papel tapiz), no un color liso; los colores marcados «(adjusted)» en la paleta
  (`msgServiceBg`, `historyScrollBarBg`…) se recalculan a partir del color medio de ese fondo. Colores de muestra del
  esquema clásico: fondo `#9bd494`, burbuja saliente `#eaffdc`, acento `#40a7e3`
  (`tg:window/themes/window_themes_embedded.cpp:240-250`).
- Tema oscuro («Night» / «Tinted», azulado): `night:` = `colors.tdesktop-theme` dentro de
  `Telegram/Resources/night.tdesktop-theme`. Su fondo de chat es una imagen de 1×1 píxel de color **`#0e1621`**
  (`background.png` del mismo zip). Acento `#5288c1` (`window_themes_embedded.cpp:262-272`).
- Entre paréntesis, el alias al que apunta en claro.

### Base (ventana, texto, acento)

| Nombre | Claro | `ui:colors.palette` | Oscuro | `night:` |
|---|---|---|---|---|
| `windowBg` fondo general | `#ffffff` | :9 | `#17212b` | :1 |
| `windowFg` texto | `#000000` | :10 | `#f5f5f5` | :2 |
| `windowBoldFg` texto negrita | `#222222` | :16 | `#e9e8e8` | :8 |
| `windowSubTextFg` texto secundario | `#999999` | :14 | `#708499` | :6 |
| `windowBgOver` fondo con ratón | `#f1f1f1` | :11 | `#232e3c` | :3 |
| `windowBgRipple` onda | `#e5e5e5` | :12 | `#24303d` | :4 |
| `windowBgActive` **acento relleno** | `#40a7e3` | :18 | `#5288c1` | :10 |
| `windowActiveTextFg` texto acento/enlace | `#168acd` | :20 | `#6ab3f3` | :12 |
| `activeLineFg` línea activa | `#37a1de` | :39 | `#6396cb` | :27 |
| `shadowFg` sombras | `#00000018` | :24 | `#04080e56` | :15 |
| `menuIconFg` iconos | `#999999` | :56 | `#6c7883` | :45 |

### Lista de chats y búsqueda

| Nombre | Claro | `ui:` | Oscuro | `night:` |
|---|---|---|---|---|
| `dialogsBg` fondo lista | `#ffffff` (windowBg) | :183 | `#17212b` (windowBg) | — |
| `dialogsBgOver` fila con ratón | `#f1f1f1` (windowBgOver) | :201 | `#202b36` | :145 |
| `dialogsBgActive` **fila seleccionada** | `#419fd9` | :218 | `#2b5278` | :160 |
| `dialogsNameFg` nombre | `#222222` (windowBoldFg) | :184 | `#f5f5f5` | :132 |
| `dialogsTextFg` último mensaje | `#999999` (windowSubTextFg) | :187 | `#7f91a4` | :135 |
| `dialogsTextFgService` autor/tipo | `#168acd` | :188 | `#73b9f5` | :136 |
| `dialogsDraftFg` «Borrador:» | `#dd4b39` | :189 | `#ff525d` | :137 |
| `dialogsDateFg` hora | `#999999` | :186 | `#8696a8` | :134 |
| `dialogsUnreadBg` contador | `#40a7e3` (windowBgActive) | :194 | `#4082bc` | :142 |
| `dialogsUnreadBgMuted` contador silenciado | `#bbbbbb` | :195 | `#3e546a` | :143 |
| `dialogsSentIconFg` marcas de enviado | `#5dc452` | :193 | `#72bcfd` | :141 |
| `filterInputInactiveBg` búsqueda en reposo | `#f1f1f1` (windowBgOver) | :78 | `#242f3d` | :62 |
| `filterInputActiveBg` búsqueda enfocada | `#ffffff` (windowBg) | :77 | `#242f3d` | :451 |
| `placeholderFg` texto de ayuda | `#999999` | :73 | `#6d7883` | :58 |
| `sideBarBg` barra de carpetas | `#293a4c` | :645 | `#0e1621` | :457 |
| `sideBarBgActive` carpeta activa | `#17212b` | :646 | `#25303e` | :458 |

En la fila seleccionada nombre, texto, hora y contador pasan a blanco (`dialogsNameFgActive`,
`dialogsTextFgActive`, `dialogsDateFgActive` → `windowFgActive #ffffff`, `ui:` :219-222) y el contador se invierte
(fondo blanco, `dialogsUnreadBgActive`, `ui:` :229).

### Conversación

| Nombre | Claro | `ui:` | Oscuro | `night:` |
|---|---|---|---|---|
| fondo del chat | papel tapiz con patrón | — | `#0e1621` | `background.png` |
| `topBarBg` cabecera | `#ffffff` (windowBg) | :246 | `#17212b` | :181 |
| `historyComposeAreaBg` campo de escribir | `#ffffff` (msgInBg) | :458 | `#17212b` | :358 |
| `historySendIconFg` botón enviar | `#40a7e3` (windowBgActive) | :463 | `#5288c1` | :363 |
| `msgInBg` **burbuja entrante** | `#ffffff` (windowBg) | :349 | `#182533` | :256 |
| `msgOutBg` **burbuja saliente** | `#effdde` | :351 | `#2b5278` | :258 |
| `msgInBgSelected` / `msgOutBgSelected` | `#c2dcf2` / `#b7dbdb` | :350 / :352 | `#2e70a5` / `#2e70a5` | :257 / :259 |
| `msgInShadow` / `msgOutShadow` sombra 2 px | `#748ea229` / `#3ac3461d` | :359 / :361 | transparente | :266 / :268 |
| `msgInDateFg` / `msgOutDateFg` hora | `#a0acb6` / `#6db566` | :363 / :365 | `#6d7f8f` / `#7da8d3` | :270 / :272 |
| `historyTextOutFg` texto saliente | `#000000` (windowFg) | :262 | `#e4ecf2` | :191 |
| `historyOutIconFg` marcas ✓✓ | `#57b84c` | :272 | `#6bbfff` | :201 |
| `historyLinkInFg` enlaces | `#168acd` | :264 | `#70baf5` | :193 |
| `msgInReplyBarColor` / `msgOutReplyBarColor` barra de cita | `#37a1de` / `#5eb854` | :370 / :372 | `#429bdb` / `#65b9f4` | :277 / :279 |
| `msgServiceBg` píldora de fecha | `#517c417f` | :368 | `#213040d5` | :275 |
| `msgServiceFg` texto de la píldora | `#ffffff` | :367 | `#ffffff` | :274 |
| `msgDateImgBg` pastilla de hora sobre foto | `#00000054` | :380 | `#00000054` | :287 |
| `historyUnreadBarBg` / `…Fg` barra no leídos | `#fcfbfa` / `#538bb4` | :285 / :287 | `#182433` / `#ffffff` | :213 / :215 |
| `msgFileInBg` / `msgFileOutBg` círculo de archivo | `#40a7e3` / `#5fbe67` | :388 / :391 | `#3f96d0` / `#4c9ce2` | :294 / :297 |
| `historyScrollBarBg` barra de scroll | `#517c417a` | :344 | `#7f84897a` | :252 |

Observación: en claro la burbuja saliente es **verde muy pálido** y la entrante blanca sobre fondo con patrón; en
oscuro la saliente es **el mismo azul que la fila seleccionada** (`#2b5278`), lo que da coherencia entre lista y chat.

## e. Tipografía

- **Familia**: Open Sans (Regular, SemiBold e Italic) **incrustada en el ejecutable** (`lib_ui/fonts/OpenSans-*.ttf`),
  para que se vea igual en todos los sistemas. Si el usuario elige otra fuente, se busca el tamaño en píxeles que
  iguala la altura de las letras de Open Sans, para no romper las medidas (`ComputeMetrics`,
  `ui:style/style_core_font.cpp:187-240`; «basic = Open Sans», `:205`). Monoespaciada en Windows: Cascadia Mono →
  Consolas (`:148-163`).
- **Solo dos pesos** en toda la interfaz: normal y **seminegrita** (600). No se usa negrita 700 salvo en contadores.
- **Tamaño base 13 px** (`fsize`, `ui:basic.style:53`) para casi todo; 14 px en diálogos modales (`boxFontSize`, `:56`).
- Todo escala con la escala de la interfaz (100 %, 125 %, 150 %…): los `px` de los `.style` se multiplican.

| Uso | Fuente | Fuente (fichero) |
|---|---|---|
| Nombre en la fila de la lista | 13 px seminegrita | `semiboldFont`, `tg:dialogs/ui/dialogs_layout.cpp:918-929`; `ui:basic.style:55` |
| Último mensaje en la lista | 13 px normal | `dialogsTextFont: normalFont`, `tg:dialogs/dialogs.style:86` |
| Hora en la lista | 13 px normal | `dialogsDateFont: font(13px)`, `tg:dialogs/dialogs.style:88` |
| Contador de no leídos | 12 px **negrita** | `dialogsUnreadFont`, `tg:dialogs/dialogs.style:78` |
| Etiquetas de carpeta en la fila | 10 px | `dialogRowFilterTagStyle`, `tg:dialogs/dialogs.style` (tras `taggedDialogRow`) |
| Botones de la barra de carpetas | 11 px seminegrita | `tg:window/window.style:230-237` |
| Texto del mensaje | 13 px normal | `msgFont: font(fsize)`, `tg:ui/chat/chat.style:15`; `historyTextStyle`, `tg:chat_helpers/chat_helpers.style:1261` |
| Nombre del autor en la burbuja | 13 px seminegrita | `msgNameFont: semiboldFont`, `tg:ui/chat/chat.style:16` |
| Hora dentro de la burbuja | 13 px normal (en gris) | `msgDateFont: font(13px)`, `tg:ui/chat/chat.style:21` |
| Píldora de fecha / mensajes de servicio | 13 px seminegrita | `msgServiceFont: semiboldFont`, `tg:ui/chat/chat.style:18` |
| Barra «no leídos» | 13 px seminegrita | `historyUnreadBarFont`, `tg:ui/chat/chat.style:353` |
| Campo de escribir | 13 px normal (mismo estilo que el mensaje) | `historyComposeField.style: historyTextStyle`, `tg:chat_helpers/chat_helpers.style:1280` |
| Emoji en línea | 18 px | `emojiSize`, `ui:basic.style:59` |

Consecuencia de diseño: la jerarquía se consigue con **peso y color** (negro seminegrita vs. gris normal), no con
tamaños distintos. Para PixPin con DirectWrite: Segoe UI Variable/Segoe UI 13 px (≈ 9,75 pt) normal y seminegrita
reproduce el mismo ritmo sin incrustar fuentes ajenas.

## f. Técnicas de fluidez

Explicadas con mis palabras; se citan los sitios donde se ven en el código, pero **no se copia nada**.

### 1. Pintar solo lo visible, encontrando el primer elemento por búsqueda binaria

- Ambas listas guardan cada fila/mensaje con su **posición vertical acumulada** (`top()` + `height()`), de modo que
  el alto total es `último.top + último.height` (`tg:dialogs/dialogs_list.h:38-41`).
- Para empezar a pintar se hace una **búsqueda binaria** por la coordenada de scroll: `List::findByY` usa
  `lower_bound` sobre las filas (`tg:dialogs/dialogs_list.cpp:263-267`); el historial hace lo mismo sobre el vector
  de mensajes (`ListWidget::enumerateItems`, `tg:history/view/history_view_list_widget.cpp:250-283`; también
  `findViewForPinnedTracking`, `:2270-2296`). Nada de recorrer la lista entera.
- El `paintEvent` solo atiende el **rectángulo sucio** que manda el sistema (`e->rect()`,
  `tg:dialogs/dialogs_inner_widget.cpp:1009`), y recorre desde la primera fila visible hasta salirse de él
  (`:3626`). Los resultados de búsqueda, con filas de alto fijo, ni siquiera necesitan búsqueda: se calcula
  `desde = (y − salto) / 62` (`:1475-1476`).
- No hay «reciclado de widgets» al estilo Android: **las filas no son widgets**. Un solo widget grande se pinta
  entero con el pincel; cada fila es un objeto de datos con un método de pintado. Eso evita crear, medir y destruir
  controles al desplazarse — es la decisión de arquitectura que más ayuda en máquinas modestas.

### 2. Cachés: de altura, de maquetación de texto y de mapa de bits

- **Alturas**: cada mensaje calcula su ancho óptimo/mínimo una vez (`initDimensions` → `countOptimalSize`) y su
  alto solo cuando cambia el ancho de la columna (`resizeGetHeight` → `countCurrentSize`,
  `tg:history/view/history_view_object.h:18-24`). Si nada cambia, el alto ya está guardado. Al insertar o cambiar un
  mensaje se marca solo ese con «recalcular» (`setPendingResize`, `tg:history/view/history_view_element.cpp:1474`).
- **Texto**: el texto de cada mensaje se convierte una sola vez en una estructura con bloques y saltos de línea
  (`Ui::Text::String`); repintar solo la dibuja. El hueco para la hora se maneja con el «bloque de salto»
  (ver apartado c) sin rehacer el texto (`validateTextSkipBlock`, `:2211-2219`).
- **Imagen de fila mientras se arrastra el scroll**: si la lista está desplazándose, cada fila se pinta **una vez a
  una imagen** y después se copia tal cual; hay un mapa de hasta 256 imágenes o 32 MB que se vacía al parar
  (`Ui::RowsScrollCache`, `tg:ui/rows_scroll_cache.h:11-56`; uso en `tg:dialogs/dialogs_inner_widget.cpp:1080-1102`).
  Las filas activas, seleccionadas o animándose no se cachean.
- **Trozos precoloreados**: contadores hechos con dos medios círculos ya coloreados (apartado b), esquinas
  redondeadas y máscaras de onda precalculadas por color y radio (`tg:ui/chat/chat_style.cpp:745-820`), colas de
  burbuja como iconos coloreables. Rasterizar curvas es lo caro; hacerlo una vez y reutilizar es la regla.
- Los avatares se guardan como imagen ya redondeada y se tiran los de mensajes lejanos
  (`kClearUserpicsAfter = 50`, `tg:history/view/history_view_list_widget.cpp:116`).

### 3. Un solo reloj para todas las animaciones

- Hay un **gestor único** de animaciones: cada animación viva se apunta a una lista y el gestor las avanza todas de
  golpe, pidiendo después un repintado (`Ui::Animations::Manager`, `ui:effects/animations.cpp:74-200`).
- El ritmo se ata al ciclo de dibujo del sistema (`crl::on_main_update_requests`), con dos frenos: se ignoran
  avisos a menos de 4 ms (`kIgnoreUpdatesTimeout`, `:27`) y el paso nominal es `1000 / universalDuration` = **1000/120
  ≈ 8 ms** (`kAnimationTick`, `:26`). Si no hay animaciones vivas, **no hay temporizador ni repintados**.
- Duraciones y curvas típicas: 120 ms para cambios básicos (`universalDuration`, `ui:basic.style:135`), 150 ms para
  desplegar (`slideWrapDuration`, `:100`), 200 ms para fundidos (`fadeWrapDuration`, `:101`), 240 ms para deslizar
  una sección (`slideDuration`, `:96`), 150 ms para la aparición de un mensaje nuevo (`itemRevealDuration`,
  `tg:ui/chat/chat.style:58`), 200 ms para reordenar fijados con curva seno entrada-salida
  (`tg:dialogs/dialogs_inner_widget.cpp:2902-2915`). El catálogo de curvas está en `ui:effects/animation_value.h:34-44`
  (`linear`, `sineInOut`, `easeOutCirc`, `easeOutCubic`, `easeOutQuint`…).

### 4. Repintar lo mínimo

- Al cambiar el ratón de fila se repinta **solo el rectángulo de esa fila** (`update(0, y, ancho, 62)`,
  `tg:dialogs/dialogs_inner_widget.cpp:3764`), nunca la lista entera.
- Mientras se desplaza el historial se saltan repintados accesorios durante 100 ms
  (`kSkipRepaintWhileScrollMs`, `tg:history/history_widget.cpp:226`), y los GIF/vídeos se congelan si la ventana no
  está activa o hay un menú abierto (`p.setInactive(...isGifPaused...)`, `dialogs_inner_widget.cpp:1000-1001`).
- Si otra ventana o panel tapa el widget, se sale del `paintEvent` sin pintar (`contentOverlapped`, `:1003`).
- La fecha flotante se oculta sola 1 s después de parar el scroll, con fundido de 200 ms
  (`tg:ui/chat/chat.style:310-311`), así no se repinta mientras se lee.

### 5. Cargar por trozos y con margen

- El historial pide **30 mensajes** la primera vez y **50** en las siguientes tandas
  (`kMessagesPerPageFirst`/`kMessagesPerPage`, `tg:history/history_widget.cpp:222-223`).
- Mantiene en memoria ~**4 pantallas por arriba y 4 por abajo** y pide más cuando quedan menos de **2 pantallas** de
  margen (`kPreloadedScreensCount`, `kPreloadIfLessThanScreens`, `tg:history/view/history_view_list_widget.cpp:112-115`,
  `:2323-2336`). Es decir: se carga **antes** de llegar al borde, nunca al llegar.
- La lista de chats hace lo mismo: pide más filas hasta `visible + 3 × alto visible`
  (`PreloadHeightsCount`, `tg:dialogs/dialogs_inner_widget.cpp:4582-4583`, `:5484-5488`).
- Los borradores se guardan con retardo (1 s tras dejar de escribir, 5 s como tope) para no tocar disco en cada
  tecla (`kSaveDraftTimeout`, `tg:history/history_widget.cpp:230-231`).

### 6. Scroll

- El área de scroll es propia (no la de Qt): rueda, teclado, táctil e inercia con velocidad acotada
  (`ui:widgets/scroll_area.cpp:525-700`). La barra se dibuja fina (12 px de zona, 3 px de radio) y se desvanece
  cuando no se usa (`historyScroll`, `tg:ui/chat/chat.style:187-201`).
- El desplazamiento no anima el contenido: mueve el origen y repinta la banda nueva; la suavidad viene de que
  pintar una banda es barato (puntos 1 y 2).
- Al llegar mensajes nuevos arriba, se corrige la posición para que **lo que se está leyendo no salte**
  (se recalcula el alto total y se compensa, `tg:history/view/history_view_list_widget.cpp:2806-2850`).

## g. Modelo de datos a nivel conceptual

```
Session (dueña de todo)
 ├── peers:   quién es cada cual (persona, grupo, canal)  → PeerData
 ├── items:   mensajes por id                             → HistoryItem
 ├── histories: una conversación por peer                 → History (= Entry)
 └── chatsList(filtro): MainList
        ├── IndexedList  (orden + índice por letra para buscar)
        └── PinnedList   (orden manual de los fijados)
```

- **Chat / interlocutor**: se separa *quién* (`PeerData`: nombre, avatar, tipo) de *la conversación con él*
  (`History`, `tg:history/history.h:63`). Lo que la lista necesita no lo pide `History` directamente: está en una
  clase base **`Dialogs::Entry`** (`tg:dialogs/dialogs_entry.h:74`) con lo justo para pintar una fila: nombre para
  mostrar y para ordenar (`chatListName`, `chatListNameSortKey`, `:150-153`), fecha del último mensaje
  (`chatListTimeId`, `:171`), estado de no leídos (`chatListUnreadState`, `:146`) y **el nombre ya maquetado**
  (`_chatListNameText`, `:218`, con su número de versión para saber cuándo rehacerlo).
- **Clave de lista (`Dialogs::Key`, `tg:dialogs/dialogs_key.h:30-58`)**: un puntero envuelto que puede apuntar a un
  chat, una carpeta, un tema de foro… Todo lo que sabe la lista es «esto es una entrada»; así la misma lista sirve
  para cosas distintas. En PixPin equivaldría a `ClaveDeLista { Proyecto | Carpeta }`.
- **Orden**: cada entrada tiene una **clave de orden de 64 bits** (`_sortKeyInChatList`, `:213`):
  - normal: `(fecha << 32) | contador` — la fecha manda y el contador rompe empates manteniendo estable el orden de
    llegada (`DialogPosFromDate`, `tg:dialogs/dialogs_entry.cpp:37-42`);
  - fijado: `0xFFFFFFFF000000FF − índice` — números enormes, siempre por encima de cualquier fecha
    (`PinnedDialogPos`, `:48-50`);
  - anclado arriba del todo (chats de servicio): `0xFFFFFFFFFFFF000F − índice` (`:44-46`).
  Reordenar la lista = cambiar un número y volver a insertar, sin recorrerla entera
  (`computeSortPosition`, `:248-251`).
- **Fila de la lista (`Dialogs::Row`, `tg:dialogs/dialogs_row.h:84-220`)**: es la *vista* de una entrada; guarda su
  `top`, su `height` y su índice. Los datos viven en `Entry`; la fila solo sabe dónde se pinta.
- **Mensaje (`HistoryItem`)**: id, autor, fecha, texto con formato, medio adjunto (foto, archivo…), a quién responde,
  estado de envío y banderas (editado, saliente, silencioso…). Es **solo datos**: cómo se ve es
  `HistoryView::Element` (con su alto, su maquetación y su redondeo), de modo que un mismo mensaje puede pintarse en
  dos sitios a la vez. Los mensajes se guardan por bloques (`std::deque<HistoryBlock>`, `tg:history/history.h:490`).
- **Borrador (`Data::Draft`, `data/data_drafts.h:56-76`)**: texto con etiquetas, **posición del cursor**, a qué
  mensaje responde o qué mensaje se está editando, y fecha. Se guarda por chat, con retardo, y sobrevive al cierre.
- **No leídos**: no es un número suelto sino un **estado agregado** (mensajes, marcas, menciones, reacciones, y
  cuántos de ellos son de chats silenciados). Cada cambio en un chat se suma/resta al total de la lista sin recontar
  (`unreadStateChanged`, `tg:dialogs/dialogs_main_list.h:38-44`); la insignia se deduce de ese estado
  (`BadgesForUnread`, `tg:dialogs/dialogs_entry.h:52-55`).
- **Fijados**: lista aparte con orden propio y límite (`PinnedList`, `tg:dialogs/dialogs_pinned_list.h:22-51`);
  fijar/desfijar solo cambia la clave de orden.
- **Carpetas/filtros**: cada filtro tiene **su propia lista ordenada y su propio orden de fijados**
  (`MainList(filterId)`), no se recalcula al vuelo.

## h. Traducción a PixPin

### El mapeo

| Telegram | PixPin Max | Dónde vive hoy |
|---|---|---|
| Chat (`History` + `PeerData`) | **Proyecto** (`Proyecto`: `id`, `nombre`, `hojas`, `tocado`, `archivado`, `uid`) | `crates/pixpin-proyecto/src/lib.rs:119-145` |
| Entrada de la lista (`Dialogs::Entry`) | Vista ligera del proyecto: nombre, fecha `tocado`, nº de hojas, sin abrir el `.pixpin` | `lib.rs:120-144` |
| Mensaje (`HistoryItem`) | **Mensaje del cuaderno** (`Mensaje`: `id`, `cuando`, `clase`, `texto`, `ruta`, `nombre`, `bytes`, `referencia`, `pagina`, `responde_a`, `fijado`, `emoji`, `proyecto`) | `crates/pixpin-proyecto/src/cuaderno.rs:61-99` |
| Tipo de medio del mensaje | `Clase`: `Nota`, `Imagen`, `Archivo`, `Voz`, `Dibujo`, `Pagina`, `Proyecto`, `MiniApp`, `Otra(..)` | `cuaderno.rs:30-52` (Android añade `TABLA` y `CROQUIS`, `guardados/Mensajes.kt:36-72`) |
| Almacén de mensajes | `guardados.jsonl`, **una línea JSON por mensaje**, se añade al final; una línea rota se salta | `cuaderno.rs:1-22`, `:163-186` |
| Hilo/carpeta del chat | `Mensaje.proyecto`: `None` = conversación general, `Some(id)` = chat del proyecto | `cuaderno.rs:81`; `Mensajes.kt` («cada proyecto tiene la suya») |
| Respuesta citada | `Mensaje.responde_a` | `cuaderno.rs:94` |
| Fijados | `Mensaje.fijado` (mensaje) / proyectos fijados: falta, hay que añadirlo | `cuaderno.rs:84` |
| Etiquetas de carpeta (Telegram) | `Mensaje.emoji` (una etiqueta por mensaje, puesta después) | `cuaderno.rs:83` |
| «Reenviar / compartir» | El chat viaja en el `.pixpin`: `ChatQueViaja.preparar` / `.recibir` | `sincro/ChatQueViaja.kt:53-120` |
| Adjuntos | Entradas del ZIP del `.pixpin` (`lienzos/*.excalidraw`, `notas/*.md`, `documento.pdf`, `adjunto:*`) | `lib.rs:204-280`; `ChatQueViaja.kt:32-34` |

Es decir: **el modelo de datos ya existe y ya viene del móvil**. Lo que falta es la ventana.

### Qué se reutiliza de PixPin

- **Pintado**: `pixpin_render::Superficie` (cadena de intercambio DirectComposition, `asegurar`, `empezar`,
  `presentar`, `presentar_sincronizado`, `crates/pixpin-render/src/superficie.rs:65-330`) y
  `pixpin_render::Pintor` — ya tiene todo lo que pide una lista de chats: `rellenar_redondeado`,
  `con_recorte` / `empujar_recorte`, `bitmap`, `texto_ajustado`, `medir_texto`, `texto_con_fondo`, `parrafo`,
  `medir_parrafo` (`crates/pixpin-render/src/lienzo.rs:212-980`). Las burbujas son
  `rellenar_redondeado` + un trazado para la cola; los avatares, `bitmap` recortado.
- **Caché de capa quieta**: `CapaEstatica` + `Estampa` + `sigue_valiendo`
  (`crates/pixpin-render/src/capa_estatica.rs:38-72`) es justo el equivalente de la caché de filas de Telegram
  durante el scroll.
- **Iconos**: `pixpin_render::iconos_excalidraw` (constantes `Icono` con trazados vectoriales) para adjuntar,
  enviar, buscar, fijar. Nada de mapas de bits de terceros.
- **Lectura del proyecto**: `pixpin_proyecto::Paquete::abrir` / `lienzo_de` / `nota_de` para enseñar una hoja como
  mensaje (`crates/pixpin-proyecto/src/lib.rs:204-285`).
- **Cuaderno**: `Cuaderno::leer_de` y `de_conversacion(Some(id_proyecto))` ya devuelven los mensajes de un chat
  (`cuaderno.rs:182-200`).
- De `pixpin-ui` se aprovechan `panel_lateral.rs` y `propiedades.rs` como referencia de cómo se pintan listas y
  paneles con este `Pintor`.

### Medidas propuestas (calcadas del ritmo de Telegram, ajustadas a Segoe UI)

- Ventana: mínimo 380 × 480, por defecto 1024 × 700.
- Columnas: lista 260–540 (por defecto 36 % del ancho), conversación mínimo 380, asa de 6 px; por debajo de
  640 px de ancho, una sola columna.
- Fila de proyecto: 62 px de alto, avatar/miniatura 46 px en (10, 8), nombre a (68, 10), resumen a (68, 34),
  fecha arriba a la derecha, contador de 19 px.
- Cabeceras (lista y conversación): 54 px, campo de búsqueda en pastilla de 35 px con radio 18.
- Burbuja: máximo 430 px de ancho, relleno 11/8, radio 16 (6 al pegarse), margen 16/6/56/2, agrupación a 15 min.
- Campo de escribir: 36 px, hasta 224 px, con 9 px arriba y abajo; botón enviar 44 × 46.

### Plan por fases pequeñas

1. **Ventana y dos columnas.** Una ventana con `Superficie`, fondo, el reparto de columnas (proporción guardada,
   mínimos 260/380, plegado a columna de avatares) y el asa de 6 px para arrastrar. Sin datos: dos rectángulos.
   Prueba: redimensionar no parpadea y respeta mínimos.
2. **Lista de proyectos.** Índice de proyectos (nombre, `tocado`, nº de hojas) ordenado por una **clave de 64 bits**
   `(fecha << 32) | contador`, fijados por encima. Filas de 62 px pintadas solo en el rango visible (búsqueda
   binaria por Y), ratón encima y selección. Sin abrir ningún `.pixpin`.
3. **Historial con burbujas.** Leer `guardados.jsonl` del proyecto elegido (`Cuaderno::de_conversacion`), maquetar
   cada mensaje una vez (alto cacheado), pintar solo lo visible, separadores de fecha, agrupación por autor/tiempo,
   hora dentro de la burbuja con el truco del hueco final.
4. **Campo de escribir.** Alto elástico 36→224, borrador por proyecto guardado con retardo de 1 s, `Intro` envía y
   `Mayús+Intro` salta de línea; añadir la línea al `.jsonl` (escritura por anexión).
5. **Mensajes ricos.** `Clase::Dibujo` → miniatura del lienzo con `Paquete::lienzo_de`; `Clase::Pagina` → página del
   PDF; `Clase::Imagen` → foto 100–430 px; `Clase::Archivo` → tarjeta de 268 px con icono de 44 px, nombre y tamaño;
   `Clase::Nota` → texto; `responde_a` → cita con barra de 3 px. Clases desconocidas (`Otra`, `TABLA`, `CROQUIS`):
   tarjeta genérica que nunca se pierde el dato.
6. **Búsqueda y carpetas.** Campo en la cabecera de la lista que filtra por nombre de proyecto y por texto de
   mensaje; después, filtrado por `emoji` y por clase, y barra lateral de 72 px si hacen falta carpetas.
7. **Fluidez.** Solo cuando lo anterior funcione: caché de imagen de fila mientras se arrastra el scroll,
   precarga de dos pantallas, un único reloj de animaciones a ~8 ms con duraciones de 120/150/200 ms, y repintado
   por rectángulo de fila.

### Avisos de licencia para la implementación

- No se copia ni traduce código de Telegram Desktop ni de lib_ui. Este documento describe **qué** medidas y **qué**
  técnicas, no **cómo** están escritas.
- No se usan sus iconos, sonidos, fondos ni el nombre «Telegram» en la interfaz.
- La paleta debe ser la nuestra: úsense estos hex como referencia de contraste y de relación entre tonos, no como
  copia literal (en especial, conviene un acento propio en vez de `#40a7e3`/`#5288c1`).
