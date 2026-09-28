# El chat de PixPin Android, pieza a pieza, y cómo está en Windows

Fecha: 2026-09-18. Fuente: repo `1xmanMAX/PIXPIN_PRO_ANDROID` (v0.51.0), ficheros
`guardados/MensajesActivity.kt`, `CabeceraFlotante.kt`, `ColoresDelChat.kt`,
`EtiquetaDeDia.kt`, `Mensajes.kt`, `HojasDelProyecto.kt`, `ui/HojaDeCompartir.kt`,
`res/values/strings.xml`; y las 6 capturas del móvil del usuario (tema de noche
con colores dinámicos). Los colores se midieron en píxeles de esas capturas.

Estado: **sí** = hecho y funciona sobre el almacén; **a medias** = existe con
otra forma o con un límite dicho abajo; **aviso** = el icono y la entrada están,
pero la función no existe en Windows y al pulsarla sale un aviso honesto
(«… todavía no existe en Windows»); **no** = no está.

Líneas: `apps/pixpin/src/ventana_chat.rs` salvo que se diga otra cosa.

## Iconos

Todos son Material Icons (relleno, 24 px) del repo `google/material-design-icons`
(Apache-2.0), copiados tal cual en `crates/pixpin-render/src/icono/material.rs`
(46 constantes, `pixpin_render::icono::material::*`). Los `AutoMirrored` de
Compose son el mismo dibujo.

| Compose | Constante | Dónde se usa en el chat |
|---|---|---|
| `AutoMirrored.Filled.ArrowBack` | `ARROW_BACK` | pastilla de volver |
| `Filled.Search` | `SEARCH` | lupa de la cabecera |
| `Filled.MoreVert` | `MORE_VERT` | tres puntos |
| `Filled.Folder` / `Filled.BookmarkBorder` | `FOLDER` / `BOOKMARK_BORDER` | disco del título (proyecto / «Mensajes guardados»); fila de un proyecto adjunto; «Guardar esto» |
| `Filled.KeyboardArrowDown` | `KEYBOARD_ARROW_DOWN` | flechita del título |
| `Filled.Close` | `CLOSE` | salir de la selección, quitar la respuesta, quitar etiqueta |
| `Filled.AttachFile` | `ATTACH_FILE` | clip |
| `Filled.Mic` | `MIC` | micrófono |
| `AutoMirrored.Filled.Send` | `SEND` | enviar; reenviar en la barra de selección |
| `Filled.Stop` | `STOP` | (copiado; grabar no existe) |
| `Filled.OpenInNew` | `OPEN_IN_NEW` | pastilla «abrir fuera» de la fila y esquina de la foto; «Sacar a la pantalla» |
| `Filled.Link` | `LINK` | tarjeta «Viene de» |
| `AutoMirrored.Filled.ArrowForward` | `ARROW_FORWARD` | tarjeta «Viene de» |
| `Filled.Description` | `DESCRIPTION` | fila de archivo; «Un archivo» |
| `Filled.Draw` | `DRAW` | fila de lienzo; «Lienzo nuevo», «Abrir en lienzo» |
| `Filled.TableChart` | `TABLE_CHART` | fila de tabla |
| `AutoMirrored.Filled.MenuBook` | `MENU_BOOK` | fila de página; «Una página de un proyecto» |
| `Filled.ViewInAr` | `VIEW_IN_AR` | (copiado; croquis no existe en Windows) |
| `AutoMirrored.Filled.Reply` | `REPLY` | «Comentar esto», barra de respuesta, hilo |
| `Filled.ContentCopy` | `CONTENT_COPY` | «Copiar» |
| `Filled.PushPin` | `PUSH_PIN` | fijado junto a la hora; fijar en bloque |
| `Filled.IosShare` (`IconoDeCompartir`) | `IOS_SHARE` | «Compartir» |
| `Filled.Launch` | `LAUNCH` | «Abrir con otra app» |
| `Filled.Compress` | `COMPRESS` | «Aligerar el PDF» |
| `Filled.Edit` | `EDIT` | «Cambiar el nombre», «Dibujar aquí mismo» |
| `Filled.Alarm` | `ALARM` | «Recordármelo» |
| `Filled.Crop` | `CROP` | «Ver solo la foto» |
| `Filled.Forum` | `FORUM` | «Ver comentarios» |
| `Filled.EmojiEmotions` | `EMOJI_EMOTIONS` | «Poner una etiqueta» |
| `AutoMirrored.Filled.Forward` | `FORWARD` | «Reenviar» |
| `Filled.LibraryAdd` | `LIBRARY_ADD` | «Unir al proyecto» |
| `Filled.Subtitles` | `SUBTITLES` | «Pasar a texto», «Teleprónter» |
| `Filled.Lyrics` | `LYRICS` | «Letra o texto» |
| `Filled.CheckBox` / `CheckBoxOutlineBlank` | `CHECK_BOX` / `CHECK_BOX_OUTLINE_BLANK` | «Elegir», casillas de la selección, papel elegido |
| `Filled.Delete` | `DELETE` | «Borrar» (en rojo) |
| `Filled.Palette` | `PALETTE` | «Fondo del chat» |
| `Filled.LibraryMusic` | `LIBRARY_MUSIC` | «Biblioteca de audio» |
| `AutoMirrored.Filled.List` | `LIST` | (copiado; barra de fijados con lista no hecha) |
| `Filled.Checklist` | `CHECKLIST` | «Una mini-app» |
| `Filled.RecordVoiceOver` | `RECORD_VOICE_OVER` | «Una conversación» |
| `Filled.Hearing` | `HEARING` | «Pronunciar» |
| `Filled.PlayArrow` | `PLAY_ARROW` | fila de una nota de voz |
| `Filled.Image` | `IMAGE` | «Foto o vídeo» (entrada solo de Windows) |
| `Filled.Wifi` | `WIFI` | «Traer del móvil» (entrada solo de Windows) |

Sustituidos: el clip de Tabler (`CLIP`) por `ATTACH_FILE`; el triángulo de enviar
dibujado a mano por `SEND`; el `PIN_ICON` de Excalidraw de la esquina de la foto
por `OPEN_IN_NEW`; los iconos de Excalidraw del menú del clip por los de Material.

## Colores y medidas (medidos en las capturas y en el Kotlin)

| Pieza | Móvil | Windows |
|---|---|---|
| Papel | degradado `#151E27→#10161D` («Mar» de noche) y 6 papeles (`FondosDelChat`) | igual, los 6, elegibles (`FONDOS`, l. 118) |
| Burbuja | `#3E618A`, radio 17, todas a la izquierda | igual (`RADIO_BURBUJA`) |
| Contorno | 2 dp del color del lienzo: `HojasDelProyecto.colorDe` = `hashCode` de Java de `referencia` en módulo 6 sobre azul `#4C7DF0`, verde `#2FA84F`, naranja `#E0803A`, morado `#9B59D0`, rojo `#D64B6A`, turquesa `#12A5A5` | misma cuenta (`pixpin_ui::chat::color_del_borde`, probada contra Java) |
| Botón redondo de la fila | 44, `#493A11`, icono crema `#FEE8B7` | igual |
| Punto de proyecto | 12, halo blanco 2, verde `#2E9E4F` / rojo `#D24B3E` | igual; rojo en «Mensajes guardados» (el general) |
| Pastilla «abrir fuera» | 30×24, radio 8, color de la hora al 15,6 %, icono 16 | igual |
| Chapa del código | letra 10, fondo blanco al 20 % (`#6481A1` sobre la burbuja), radio 6 | igual (antes usaba el color del separador) |
| Hora | 11, `#A8CCE8` | 11 |
| Foto sola | sin relleno, redondeada con la burbuja; chapa+hora en pastilla negra 60 %, radio 8, abajo a la derecha; botón 30 negro 42 % arriba a la derecha; punto arriba a la izquierda | igual |
| «Viene de» | pastilla `#493A11` radio 12, enlace 18, «Viene de» 10 `#D0BE90`, texto 13 seminegrita `#FEE8B7`, flecha 16 | igual (sin seminegrita: el pintor no tiene pesos) |
| Separador de día | pastilla `#242B55`, radio 11, letra 14 lavanda `#C5C9EA`, «Hoy»/«Ayer»/«13 de septiembre» | igual |
| Cabecera | tres pastillas de 46, aire 6, fondo `#161B31`, filete `#44495C`; disco 28 `#FFD179` con carpeta; nombre + flecha; «N cosas · tamaño» 12 | igual (`Disposicion::pildoras`) |
| Isla de escribir | margen 7, 9 abajo, radio 22, fondo `#14192D`; campo `#242B55` radio 22 fila 44; «Escribe algo…» `#B4B9D7`; clip dentro, micrófono fuera | igual (`Disposicion::isla/campo`); letra 15 en vez de 18, en proporción a las burbujas de 13 |
| Tamaños | `Formatter.formatShortFileSize`: «949 kB», «1.1 MB» | `pixpin_ui::chat::tamano_corto` |

## Opciones del chat

### Cabecera

| Opción (móvil) | Icono | Windows | Dónde |
|---|---|---|---|
| Volver (sale del proyecto) | ArrowBack | **sí**: cierra el proyecto (guarda borrador, hoja y lienzo vivo) | `Zona::Volver`, l. 713 |
| Tocar el título: fichas de secciones | — | **a medias**: abre el panel de información con sus pestañas (Todo, Fotos, …), que es lo mismo por secciones | `Zona::Titulo`, l. 720 |
| Mantener el título: buscar | — | **no** (no hay pulsación larga); la lupa hace lo mismo | — |
| Lupa: buscar en la conversación | Search | **sí**: filtra las burbujas; con la lupa encendida salen debajo los chips de las etiquetas que de verdad hay puestas, y pulsar uno filtra por ese emoji (pulsarlo otra vez lo quita). Apagar la lupa se lleva las dos cosas, al revés que el móvil, que deja el filtro de emoji puesto sin enseñarlo | `Zona::Buscar` y `Zona::Chip`, `chips_de`, `pixpin_ui::chat::fila_de_chips` |
| ⋮ Biblioteca de audio (solo general) | LibraryMusic | **aviso** | `menu_de_cabecera`, l. 6758 |
| ⋮ Conversaciones de proyectos (solo general) | — | **a medias** (26-sep): comparte acción con «Proyectos» y lleva a la pantalla de Proyectos (H13); los chats siguen en la lista de la izquierda | `proyectos.rs` |
| ⋮ Proyectos | — | **sí** (26-sep): abre la pantalla de Proyectos del móvil (H13), sin cambiar lo que recuerda el interruptor de la barra | `ventana_chat/proyectos.rs` |
| ⋮ Comenzar (arrancar la bola) | — | **aviso**: PixPin ya está en marcha | — |
| ⋮ Fondo del chat | Palette | **sí**: 6 papeles, se guarda en `fondo-del-chat.txt` junto al almacén | l. 654, `fondo_guardado` l. 7396 |
| ⋮ Ajustes | — | **aviso**: se abren desde la bandeja (abrirlos desde el chat necesita `main.rs`) | — |

### Menú de un mensaje (móvil: mantener pulsado; Windows: clic derecho)

`menu_de_mensaje`, l. 6852; acciones en `ejecutar`, l. 7018. Mismo orden que el móvil.

| Opción | Icono | Windows | Dónde |
|---|---|---|---|
| Comentar esto | Reply | **sí**: barra de respuesta sobre la isla con aspa; la nota enviada lleva `respondeA`; la burbuja enseña la cita (barra de acento, `#n nombre` y resumen) y pulsarla lleva al original | l. 7049, `guardar_nota` l. 3729, `pintar_cita` l. 3515 |
| Copiar | ContentCopy | **sí** (portapapeles) | l. 7053 |
| Fijar arriba / Quitar de arriba | — (el móvil no lleva icono) | **sí**: reescribe `fijado`; chincheta junto a la hora; barra del fijado, que al pulsarla lleva al mensaje | l. 7062 |
| Compartir | IosShare | **a medias**: no hay hoja de compartir en Windows; deja el fichero (o el texto) en el portapapeles y lo dice | l. 7071 |
| Abrir con otra app | Launch | **a medias**: abre con la aplicación de Windows por omisión (no el diálogo «Abrir con»). Delante va **Abrir aquí**, que solo sale si el visor de PixPin sabe leer ese archivo y lo abre en su propia ventana | `Accion::AbrirAqui` y `Accion::AbrirCon`, `visor::se_abre` |
| Aligerar el PDF (solo PDF) | Compress | **aviso** | — |
| Cambiar el nombre (lienzo) | Edit | **sí**: se escribe en la propia fila, Entrar guarda, Escape deja como estaba (solo el nombre del mensaje, no el de la hoja del proyecto) | l. 7100, `guardar_nombre_de_mensaje` l. 7544 |
| Sacar a la pantalla | OpenInNew | **sí**: manda el fichero a la ventana principal, que lo hace pin (imagen, vídeo o ficha); sin fichero, aviso | l. 7093 |
| Recordármelo | Alarm | **aviso** | — |
| Guardar esto (buzón) | BookmarkBorder | **sí** (quita `enBuzon`) | l. 7063 |
| Ver solo la foto / Ver el dibujo entero | Crop | **aviso** | — |
| Ver comentarios (si tiene) | Forum | **sí**: menú con los comentarios; elegir uno lleva a él | l. 7106 |
| Poner una etiqueta | EmojiEmotions | **sí**: ⭐ ✅ ⏳ 💡 💰 📍 y quitarla; se ve junto a la hora | l. 7065 |
| Reenviar | Forward | **sí**: menú de proyectos; copia el mensaje (fichero y lienzo incluidos) al cuaderno del otro con su número y códigos | l. 7129, `reenviar` l. 7314 |
| Unir al proyecto (general) | LibraryAdd | **aviso**: en Windows cada chat ya es un proyecto | — |
| Volver a añadir al proyecto | LibraryAdd | **no** (depende de las hojas del proyecto del móvil) | — |
| Pasar a texto (voz) | Subtitles | **aviso** | — |
| Letra o texto (audio) | Lyrics | **aviso** | — |
| Elegir | CheckBox | **sí**: modo de varios (ver abajo) | l. 7123 |
| Borrar | Delete (rojo) | **sí**: pregunta; quita la línea del cuaderno sin tocar lo que no entiende y borra el adjunto (no lienzos ni páginas, que son hojas) | l. 7148, `borrar_mensajes` l. 7269 |
| (solo Windows) Abrir en lienzo / Dibujar aquí mismo, en fotos | Draw / Edit | **sí** (lo de antes) | `menu_de_mensaje` |

### Selección de varios

| Opción | Icono | Windows | Dónde |
|---|---|---|---|
| Entrar: «Elegir», o Ctrl+clic en una burbuja | — | **sí**; pulsar marca/desmarca; tinte de lado a lado y casilla | l. 868 |
| Barra de arriba: salir | Close | **sí** (también Escape) | `Zona::SelCerrar` |
| Copiar en bloque (con línea en blanco entre medias) | ContentCopy | **sí** | l. 747, `copiar_marcados` l. 7493 |
| Fijar en bloque / soltar si ya lo estaban todos | PushPin | **sí** | `fijar_marcados` l. 7514 |
| Reenviar en bloque | Send | **sí** | `Zona::SelReenviar` |
| Unir en bloque | LibraryAdd | **no** (ver «Unir») | — |
| Borrar en bloque | Delete | **sí** | `Zona::SelBorrar` |
| Arrastrar para marcar varias | — | **no** | — |

### Burbujas

| Pieza | Windows | Dónde |
|---|---|---|
| Todas a la izquierda, `#3E618A`, contorno del color del lienzo | **sí** | `pintar_historial` l. 2555 |
| Fila: botón 44 marrón + punto + nombre (+ «949 kB PDF») + «abrir fuera» | **sí** (archivo, lienzo, tabla, página, proyecto, voz) | `fila_de` l. 6125 |
| Lienzo: vista previa arriba y fila «Lienzo» debajo | **sí** (el `<id>.excalidraw` hecho aquí se llama «Lienzo») | `nombre_de_la_fila` l. 6179 |
| Tabla: rejilla y fila «Tabla» | **sí** | — |
| Foto sola sin relleno, hora y chapa encima en pastilla, botón en la esquina, punto | **sí** | l. ~2740, `pintar_hora_sobre_foto` l. 3389 |
| Chapa `#4·Z5WA` + hora abajo a la izquierda | **sí** (ya estaba; color de la chapa corregido) | — |
| Tarjeta «Viene de» (campo `vieneDe`) que lleva al lienzo de origen | **sí**, si ese lienzo es un mensaje de este proyecto; si no, aviso | `pintar_viene_de` l. 3445, `abrir_el_origen` l. 7562 |
| Proyecto adjunto: pulsarlo abre ese proyecto | **sí** | l. 884 |
| Separadores de día en pastilla | **sí** | — |
| Página de PDF: hoja con lo anotado y «Página N» encima | **a medias**: sale como fila con el libro | — |
| Nota de voz: onda, reproducir, velocidad | **a medias**: fila con ▶ y duración; pulsar la abre con el reproductor de Windows | — |
| Mini-app de tareas/gastos | **no** (solo la tabla) | — |
| Tarjeta de un enlace en una nota | **no** | — |
| «Recibido de …» | **no** | — |
| Franja y fechas del buzón | **no** | — |
| Deslizar la burbuja para comentar | **no** (en escritorio es el menú) | — |
| Destello al saltar a un mensaje | **no** (salta sin destello) | — |

### Caja de escribir y clip

| Opción | Icono | Windows | Dónde |
|---|---|---|---|
| Isla flotante, campo en pastilla, «Escribe algo…» | — | **sí** | `pintar_redaccion` l. 6389 |
| Clip dentro del campo; se retira al escribir | AttachFile | **sí** | l. ~1000 |
| Micrófono ↔ enviar en el mismo sitio | Mic / Send | **sí** el cambio; grabar: **aviso** | l. 1024 |
| Mantener para grabar, deslizar para cancelar, subir para fijar | Mic / Stop | **aviso** (no hay grabación en Windows) | — |
| Barra «respondiendo a…» con aspa | Reply / Close | **sí** | `pintar_redaccion` |
| Clip → Un archivo | Description | **sí** | l. 610 |
| Clip → Una página de un proyecto | MenuBook | **aviso** | `menu_del_clip` l. 6701 |
| Clip → Un proyecto entero | Folder | **sí**: menú de proyectos; deja un mensaje que abre ese proyecto | l. 7183 |
| Clip → Una conversación | RecordVoiceOver | **aviso** | — |
| Clip → Teleprónter | Subtitles | **aviso** | — |
| Clip → Pronunciar | Hearing | **aviso** | — |
| Clip → Una mini-app | Checklist | **a medias**: solo «Tabla» | — |
| (solo Windows) Foto o vídeo, Lienzo nuevo, Traer del móvil | Image / Draw / Wifi | **sí** (lo de antes) | — |
| Aviso tipo Toast | — | **sí**: pastilla oscura 2,5 s sobre la isla | `pintar_aviso` l. 6540 |

## Lo que queda sin hacer, y por qué

- Grabar voz, transcribir, letra, recordatorios, aligerar PDF, teleprónter,
  pronunciar, conversación, biblioteca de audio, «ver solo la foto»: no existen en
  Windows; salen con aviso.
- Buscar filtrando las burbujas (el móvil filtra la conversación y enseña chips de
  secciones y etiquetas): Windows lo hace en el panel de información.
- Unir al proyecto / volver a añadir: dependen del modelo de hojas del proyecto del
  móvil (`UnirAlProyecto`), que en Windows vive en `pixpin-proyecto` (fuera de lo
  que este trabajo podía tocar).
- Miniatura de página de PDF dentro de la burbuja, mini-apps de tareas y gastos,
  tarjeta de enlace, buzón con sus fechas, arrastrar para marcar, deslizar para
  comentar, destello al saltar.
- Abrir los ajustes desde el chat: hace falta `main.rs`.
- Atribución de Material Icons: está en la cabecera de `icono/material.rs`; falta la
  línea en `THIRD-PARTY-NOTICES.md` (no estaba entre los ficheros que se podían tocar).
