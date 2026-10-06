# Timeline — diseño (5-oct-2026)

## Lo que pidió el usuario

> «Registrar todo lo que pasa día a día, minuto a minuto, de forma rápida y sencilla […] como un chat que se llena súper rápido, con una UI diferente al chat; que no se puedan enviar archivos, tareas ni canvas: solo voz, texto e imágenes […] UI de timeline vertical; que se muestren solo las últimas 24 horas, lo demás se archive y se pueda buscar viendo el calendario y dándole clic a lo que pasó ese día; exportar las últimas 24 horas o los días que seleccione en HTML o PDF, con el mismo formato de timeline.»

Al dictar: «con esto pasó…» abre el **título** y «y así te lo cuento…» abre la **descripción**. Se le propusieron las frases y siguió adelante con ellas.

Android no tiene nada igual (buscado en el Kotlin actual el 5-oct). Por eso es un diseño nuevo y no un puerto.

## Decisiones

| # | Decisión | Por qué |
|---|---|---|
| T1 | Vive aparte del chat: `<datos>/timeline/momentos.jsonl` (un momento por línea) y `timeline/medios/` | No llena el cuaderno que se sincroniza con notas del día. El JSON de una línea es fácil de llevar al móvil cuando allí exista. |
| T2 | Crate `pixpin-timeline` (capa 0): formato, fichero, frases, días y página HTML, con pruebas | Igual que `pixpin-lecciones`: la lógica, probada sin ventanas. |
| T3 | Las frases se buscan sin mayúsculas, tildes ni signos, y como palabras enteras | Whisper escribe la misma frase de muchas maneras. |
| T4 | Sin frases, la primera línea (al escribir) o la primera oración (al dictar) es el título | Un momento nunca queda sin título. |
| T5 | El dictado guarda el audio, y el momento lleva la hora en que se pulsó el micrófono | Oírlo dice más que la transcripción y la salva si se entendió mal. «En ese mismo momento». |
| T6 | Con la caja vacía, lo dictado se apunta solo; con algo escrito, se suma a la caja y espera a Intro | Lo rápido por defecto, sin perder lo que se estaba escribiendo. |
| T7 | Tres vistas: últimas 24 h (con caja), un día (solo lectura), calendario del mes | Lo pidió así: 24 h a la vista, lo demás por calendario. |
| T8 | Exportar = lo que se ve (24 h, el día abierto o los días elegidos en el calendario) | Una sola acción, sin un formulario de rangos. |
| T9 | PDF = la misma página HTML impresa por Edge sin ventana | Mismo aspecto exacto y sin un segundo dibujante de PDF. Edge viene con Windows 10 y 11; si falta, se dice. |
| T10 | Borrar no se lleva las fotos ni el audio | Así «Deshacer» lo devuelve entero. |

## Entradas

Bandeja «🕒 Timeline…» (id 908) y botón redondo en la cabecera de la lista del chat (`Extra::Timeline`), el primero en retirarse en una lista estrecha.

## Teclado

| Tecla | Qué hace |
|---|---|
| Intro | Apunta |
| Mayús+Intro | Salto de línea |
| Ctrl+M | Dicta |
| Ctrl+V | Pega texto o una imagen |
| ↑ con la caja vacía | Corrige el último momento |
| Ctrl+Z | Deshace un borrado mientras dura su aviso |

Esc va por capas: menú, corrección, caja, vista, cerrar.

## Pendiente

- Llevarlo al móvil: el formato ya está pensado para eso.
- Una entrada `p timeline` en Flow Launcher.

## Rediseño del 5-oct (tarde): barra común, pestañas como WeChat y lecciones juntas

Tres pedidos del usuario el mismo día:

1. «En estos 4 apartados (galería, lecciones, tareas, timeline) uniformiza la UI: que todos en la parte superior mantengan la barra de búsqueda y las funciones únicas de cada parte».
2. Tres capturas de «Mis publicaciones» de WeChat: «algo así, mejora la parte de timeline».
3. «Mejora la parte de lecciones aprendidas […] quita la barra lateral; usa más los emoticones […] emoticones y color a la vez, grave rojo pero el emoticón pintado de rojo; lo de "me volvió a pasar" está bien, mantenlo; formato de tarjetas […] que se pueda compartir como imagen cada tarjeta; […] que timeline y lecciones aprendidas las juntes: que cada tarjeta de timeline se pueda poner como lección aprendida con solo un botón, se quede marcado como lección, pero todo se muestre en el mismo lugar».

| # | Decisión | Por qué |
|---|---|---|
| T11 | Arriba, la barra común (`cabecera.rs`): título, buscador y, como botones propios, las pestañas **Hoy · Momentos · Estado · Lecciones** y **Exportar** (solo icono). Sin el botón Calendario | El pedido 1: la misma barra en las cuatro ventanas |
| T12 | El buscador busca en **todo** el timeline (título y descripción, sin tildes ni mayúsculas, todas las palabras). Con algo escrito, el contenido son los resultados por días; clic abre el detalle. Si también hay lecciones con eso, una franja amarilla lleva a ellas | Lo archivado es justo lo que no se ve |
| T13 | **Momentos** = la captura 2: año grande, mes pequeño, y por día su número grande y en fila las miniaturas (primera foto, o el título sobre un color suave). Número → el día; miniatura → el detalle | Lo pidió «algo así» |
| T14 | **Estado** = la captura 1 y sustituye al calendario: una tarjeta por mes (los que tienen algo y siempre el actual), días al revés desde hoy o el último del mes, círculo con el primer emoticono (o la primera foto, o un punto), apilado si hay varios, y «Sobre todo «X»» (el emoticono más repetido o, sin ninguno, la palabra de 4+ letras más repetida en los títulos). Clic abre el día; **Ctrl+clic** lo elige para exportar | Igual que WeChat; la elección de días del calendario se conserva |
| T15 | **Detalle** = la captura 3: foto entera sobre negro con ella misma oscurecida detrás, fecha «2026.10.5 17:11», ✕, texto sobre degradado, chapita de estado, puntitos y flechas. ←/→ fotos, ↑/↓ y RePag/AvPag el anterior/siguiente de la lista de la que se abrió. La foto se carga a tamaño de ventana en el bucle, una sola a la vez | Lo de la captura; una foto grande pesa decenas de megas |
| T16 | **Lecciones** son una pestaña del timeline, en tarjetas (sin barra lateral): gravedad con emoticono **y** color a la vez (😌 verde, 😟 naranja, 😡 rojo; el emoticono en monocromo, pintado de su color), portada con la primera foto (varias = bordes apilados), «Me volvió a pasar +1» con su «3×». Clic → el mismo detalle; ahí la gravedad da la vuelta con un clic | El pedido 3 |
| T17 | La bombilla de un momento lo hace **lección de verdad** en el almacén de siempre (`lecciones::almacen::guardar_con_adjuntos`, en «Mensajes guardados», con sus fotos y su audio; gravedad la del etiquetador o «importante») y el momento guarda su id (`Momento::leccion`). Desmarcar **solo desvincula**: la lección se queda | Así viaja al móvil y entra en el repaso. Borrar perdería lo escrito después en la ficha o en el móvil |
| T18 | Las entradas de Lecciones (bandeja 905, botón del chat, pedido `lecciones` sin proyecto ni consulta) abren el timeline en «Lecciones». «Nueva lección» (906) y el pedido con proyecto siguen con su ventana | «Todo en el mismo lugar»; la lista vieja sabe filtrar por proyecto |
| T19 | Cada tarjeta (momento o lección) se **comparte como imagen**: clic derecho o su botón → «Copiar como imagen» / «Guardar como imagen…». Se pinta sola fuera de pantalla al doble, con el mismo motor de la ventana | El pedido 3 |

Lo puro, con pruebas: `pixpin_timeline::{buscar, emoticonos, resumen, archivo}` y `dias::{fecha_con_hora, solo_el_mes}`; en la app, `timeline::disposicion` (Momentos, Estado, detalle: lo usan el pintado y el clic) y `timeline::tarjetas` (gravedad, rejilla y partes de una tarjeta de lección, búsqueda en lecciones).

### Teclado nuevo

| Tecla | Qué hace |
|---|---|
| Ctrl+F | Al buscador (Esc lo vacía, Intro le quita el foco) |
| Ctrl+1/2/3/4 | Hoy, Momentos, Estado, Lecciones |
| Escribir en Momentos, Estado o Lecciones | Empieza a buscar |
| ←/→ en el detalle | Foto anterior/siguiente |
| ↑/↓, RePag/AvPag en el detalle | Momento (o lección) anterior/siguiente |

Esc va por capas: menú, detalle, búsqueda, corrección, caja, día abierto, vuelta a «Hoy», cerrar.

### Pendiente de esta tanda

- Los emoticonos dentro de los títulos de las tarjetas salen en monocromo (la negrita no usa la fuente en color); los círculos de «Estado» sí van en color.
- El fondo del detalle está oscurecido, no difuminado (el pintor no tiene desenfoque).
- La lección hecha desde un momento va siempre a «Mensajes guardados»; para cambiarla de proyecto, la ficha de siempre.

## 5-oct (noche): historias, tarjeta para compartir y exportar completo

El usuario: «Momentos y Estado son lo mismo, lo que los diferencia es la visualización. Quiero que cuando les dé clic pueda entrar a una interfaz como si fuera una aplicación, una red social: como Instagram, aparece el texto en el medio. Y eso se pueda compartir como una imagen, el texto en el medio y si hay una imagen que esté en el fondo […] como una tarjeta de las que dicen una frase. Y a la hora de exportar, que se exporte con su audio, la imagen, todo completo, con una interfaz muy similar».

| # | Decisión | Por qué |
|---|---|---|
| T20 | **La historia** sustituye al detalle (desde Hoy, Momentos, Estado, el día, la búsqueda y Lecciones): la foto llena la ventana bajo un velo con degradado de verdad (`Pintor::rect_degradado`, nuevo: sin franjas), o el degradado estable del momento (`estilo::degradado_de`, 8 degradados); título grande y descripción **centrados**, con sombra y los emoticonos en color; barritas de 3 px por foto; tercio izquierdo/derecho, ←/→ = foto anterior/siguiente (y al acabarse, momento); ↑/↓, RePag/AvPag y la rueda = momento. Abajo una sola chapita de estado (en una lección, su gravedad, que cambia con un clic, más «↻ Me volvió a pasar +1» y «3×») y la nota de voz grande | Lo pidió «como Instagram» |
| T21 | **Compartir** = la historia sin botones en **1080 × 1350** (4:5): foto o degradado, fecha «5 oct 2026 · 17:11», texto en el medio, «🎵 Nota de voz 0:42», estado y firma «PixPin Max · Timeline». La foto se carga a su tamaño solo para esa imagen | «Una tarjeta bonita, de las que dicen una frase» |
| T22 | **Exportar** lleva fotos y audios dentro (`data:`; `.m4a` como `audio/mp4`), cada fichero una sola vez; la página es oscura como la ventana, con pestañas Línea · Momentos · Estado (estas dos las arma su JavaScript), y cada tarjeta abre la historia en la página (barritas, flechas, teclado, Esc, reproductor propio). `#m=<id>` la abre al cargar. Los datos van en JSON sin ningún `<`. Al imprimir (PDF) solo sale la línea, en claro. Avisa si pasa de 50 MB. Las lecciones también se exportan igual | «Que se exporte con su audio, la imagen, todo completo» |
| T23 | Un solo módulo de emoticonos para toda la app: `pixpin_timeline::emoticonos` (criterio de Unicode, el de tareas: ⌚ ⏰ ☕ cuentan), con tonos pastel y la pila de estados; `tareas::emoticonos` solo reexporta | Había dos que discrepaban |
| T24 | Barra: columna de título de ancho fijo (200 px, subtítulo con «…») para que el buscador no salte; las pestañas como **control segmentado** (`cabecera::pintar_con_segmentos`); Exportar aparte con el icono de descarga, también en Lecciones; buscando, ninguna pestaña activa | Revisión de diseño |
| T25 | Estado: hasta tres emoticonos del día apilados en pastel con «+N» (encogen para no salirse de su celda); día sin nada = un puntito y fila sin nada bajita; hoy en píldora azul; bajo el mes, su nombre corto y «🍕 ×6». Los días elegidos solo valen en Estado | Revisión de diseño |
| T26 | Lecciones: todas las tarjetas con la misma franja de 72 px (foto, o el color de su gravedad con su cara pintada de ese color), pila de fotos dentro de la celda, «↻ +1» junto a «3×», «La próxima vez: …» cuando existe, chips «Todas · 😡 · 😟 · 😌» y orden «Recientes / Más repetidas» | Revisión de diseño |
| T27 | Marcar → desmarcar → marcar **reutiliza** la lección (el momento guarda `leccionPrevia`) | Creaba dos lecciones iguales |
| T28 | Las acciones llevan el **id** del momento o lección (un resumen de 64 bits), no su índice; la disposición de Momentos se guarda por huella y ancho | Tras releer el índice podía ser otro; y se recalculaba tres veces por fotograma |
