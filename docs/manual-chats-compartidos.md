# Chats compartidos en el PC: el «+», la chapa y cada uno en su lado (25-oct-2026)

El usuario: «haz que los chats compartidos sean una opción especial: al darle al «+» en los chats,
que primero salgan dos opciones, un chat normal o crear un chat COMPARTIDO. Un chat compartido
lleva una insignia para que se vea que lo es; uno normal se ve normal. Y compartido, que el chat se
vuelva como un chat de verdad: mis burbujas a la derecha y las de la otra persona al otro lado. Se
puede compartir con varias personas.»

Esto es la cara de los chats compartidos de Android (`docs/chats-compartidos.md` del repositorio
de Android, 13-oct-2026). **No cambia nada de lo que viaja**: ni `compartidos.json`, ni
`persona.json`, ni las firmas, ni el cable (`PXC1`/`PXI1`). Un PC con esto y un móvil de antes se
entienden igual que antes; lo nuevo es solo cómo se ve y por dónde se crea.

## Lo que se ve

### El «+» de la lista de chats

El clic izquierdo en el «+» ya no crea el chat directamente: abre un menú pequeño (el de la v2 del
chat) con dos filas, cada una con su explicación de una línea:

| Fila | Explicación | Qué hace |
|---|---|---|
| **Chat normal** | Solo tú y tus aparatos | Lo de siempre: el chat nace sin nombre y con la caja del nombre abierta. |
| **Chat compartido** | Con otras personas, cada una en su lado | Igual, y además nace ya **compartido** (en `compartidos.json`, con uno mismo dentro como creador). Al terminar de escribir su nombre (Intro o Esc) se abren sus **Personas** en Sincronizar, donde «Añadir» enseña el código y el QR para invitar. |

Se invita a **varias personas** desde ahí, una tras otra, con el mismo mecanismo de siempre: cada
entrada la firma quien invita y la lista se junta sin servidor. El protocolo ya admitía varias; no
hizo falta tocarlo.

El clic derecho en el «+» sigue siendo el de antes (crear en una carpeta elegida, la papelera,
«Entrar en un chat compartido»).

### La chapa

Un chat compartido en el que **estoy dentro** lleva:

- en la lista, un **circulito verde con dos personas** abajo a la derecha del avatar (con un aro del
  color de la fila, como el punto de «en línea» de Telegram), y delante del nombre el mismo icono
  en verde con **cuántas personas** son (si son más de una);
- en la cabecera, la misma chapa sobre el disco del título y, delante del subtítulo,
  **«Compartido · 3 personas»**. Pulsar la chapa o ese texto abre sus Personas (invitar, quitar,
  salir).

Un chat normal no lleva nada de eso. Un chat del que salí o del que me quitaron vuelve a verse
normal (sigue en la lista, pero ya no estoy dentro).

«Personas» cuenta **personas, no aparatos**: mi PC y mi móvil son una (`nombres_dentro`).

### Cada uno en su lado

Solo en los chats compartidos, el historial pasa a ser una conversación:

- **lo mío**, a la derecha y con mi burbuja de siempre (la azul);
- **lo de cada otra persona**, a la izquierda, con la burbuja neutra (la blanca de Telegram) y,
  encima de la **primera burbuja de cada tanda**, su **nombre** con un círculo con su **inicial**,
  los dos de su color;
- las tandas se agrupan como en Telegram: dos seguidas de **la misma persona**, del mismo día y del
  mismo rato (5 minutos) van pegadas y sin repetir el nombre; si cambia la persona, aunque las dos
  vayan a la izquierda, se separan y la nueva lleva su nombre;
- las respuestas del asistente, si las hay, van a la izquierda sin nombre.

Un chat normal sigue como siempre: todo a un lado (es un cuaderno propio), y en el chat del
asistente, cada uno a su lado como ya era.

## La regla de «mío»

De quién es cada mensaje sale de **`Mensaje.aparato`**, el código fijo del aparato donde nació
(`K7Q2`, cuatro signos del SHA-256 del id del aparato), que ya viajaba. Si un mensaje lleva el id
entero del aparato en vez del código (los hay de antes), se saca el código corto por el mismo
camino que la chapa `#47·K7Q2`.

**Es mío** un mensaje cuyo código sea el de:

1. este aparato;
2. cualquier aparato de mi **grupo de sincronizar** (`identidad.json`: `yo` y `miembros`; mi móvil,
   mi tableta);
3. cualquier aparato de la **lista del chat** cuyo `id` sea uno de los anteriores (mi móvil entra
   solo en mis chats compartidos al sincronizar el grupo; es mío aunque esté en la lista con mi
   nombre).

**Es de otra persona** solo el que lleva el código de un aparato de la lista del chat que **no** es
mío; se le pone el `nombre` de esa fila de la lista (sin nombre, su código). Sus aparatos (su
móvil, su PC) son la misma persona si se llaman igual.

**Todo lo demás es mío**: sin `aparato`, o con un código que nadie de la lista reclama. El chat era
mi cuaderno antes de compartirlo y lo de antes lo escribí yo. Quien salió del chat sigue en la
lista (con `fuera`), así que lo que escribió sigue saliendo con su nombre.

Las decisiones puras (de quién es, a qué lado va, qué número de autor le toca, dónde va el nombre)
están en `crates/pixpin-ui/src/conversacion.rs` y `historial.rs` (`Entrada::autor`,
`Entrada::alto_nombre`, `Puesto::nombre`), con sus pruebas. La ventana solo mide y pinta.

## Versiones vinculadas

*27-oct-2026 · la regla de Android v0.145.0 (`docs/versiones-en-chats-compartidos.md` y
`sincro/VersionesVinculadas.kt` de allí), con los mismos campos y los mismos nombres de archivo.*

El usuario: «en los chats compartidos yo edito lo que alguien me pasó y esa persona edita lo que yo
le pasé, y al sincronizar hay incongruencias». La fusión de tres vías junta bien dos cambios **míos**
(el móvil y el PC van hacia lo mismo), pero dos cambios de dos personas sobre el mismo lienzo dan
una tercera cosa que nadie hizo. Así que, **solo en un chat compartido**:

1. **Lo mío se edita en su sitio** (la regla de «mío» de arriba).
2. **Lo de otra persona no se toca nunca.** Al primer cambio se crea **mi versión**: un mensaje
   nuevo, mío, que contesta al original y lleva su propia copia de todo.
3. Lo que siga cambiando va a mi versión, que ya es mía. Volver a abrir el original y cambiarlo
   otra vez hace **otra** versión (la 2); para seguir con la mía, se abre la mía.
4. Si otra persona cambia lo mío, ella hace su versión; a mí me llega como una respuesta.

En un chat propio, y entre mis aparatos, todo sigue como siempre.

### Qué se versiona en el PC

| Qué | Por dónde | Cuándo nace la versión |
|---|---|---|
| Lienzo, página de PDF con dibujo | tocar la burbuja | al volver del editor, si algo cambió (borrador) |
| Foto (no vídeo) | tocar la burbuja, «Abrir en el lienzo» y «Dibujar aquí» (este va al lienzo grande: en la burbuja se pintaría encima de la suya) | ídem; la foto de fondo que pone el lienzo al estrenarse no cuenta |
| Documento (pdf, doc/docx/odt/rtf, ppt/pptx/odp, epub, html) | tocar la burbuja: el lector, el visor de HTML… abren **la copia** | ídem: la tinta del lector va con el código de la copia (`anotado::adjunto_de` la encuentra por su ruta) |
| Nota | el editor de notas | al primer guardado que cambia el texto; desde ahí el editor sigue en la mía |
| Descripción de un archivo | «Describir» | al momento |
| Mini-aplicación (gastos, alarma…) | su panel en el chat | al primer cambio, con el cambio hecho; el panel sigue sobre la mía |

**No** se versionan, igual que en Android: las listas de tareas (tachar en una compartida es lo que
se quiere), las tablas del chat, audios, vídeos, croquis, proyectos y lo que se abre en otra
aplicación. Fijar, poner una etiqueta o renombrar tampoco: son de la organización del chat.

Lo que el PC hace distinto, sin efecto en lo que viaja:

- **Mini-aplicaciones al momento y no provisionales.** En Android el editor de una mini-app busca
  su mensaje por el id, así que la copia entra en el chat al abrir y se quita si no cambió. En el PC
  el panel vive dentro del chat y sabe cuándo cambia algo: la versión nace con el primer cambio. Lo
  que se ve en el chat es lo mismo.
- **Hojas de cálculo (xlsx, ods, csv) sin versión.** Su pin no escribe nunca en el archivo: lo
  editado sale como una tabla del chat nueva, que ya es mía. No hay nada que proteger.
- **La huella de «si cambió» cuenta también el adjunto** (su tamaño y su fecha), porque el visor de
  HTML escribe en el fichero y no en lo anotado. La huella no viaja.
- La carpeta de páginas todavía se edita en su sitio (su ventana va por el id del mensaje; haría
  falta la copia provisional de Android).

### Lo que lleva una versión

- `id` nuevo (`<hora>-<código>`), `uid` nuevo, `aparato` = el de este PC, `numero` nuevo en el chat
  (uno más que el mayor, nunca menos que cuántos hay), `cuando` = cuando nace;
- **`respondeA`** = el `id` del original: cualquier PixPin, también uno de antes, la enseña como una
  respuesta que lo cita;
- **`versionDe`** = el código único del original (`uid`, o el que sale del id), el vínculo que no
  cambia entre aparatos; **`versionN`** = 1 + cuántas versiones de ese original había ya en el chat.
  Es solo una etiqueta: dos aparatos sin conexión pueden dar el mismo número y no pasa nada;
- **sin** `fijado`, `emoji`, `recuerdaEn`, `enBuzon`, `unido`, `letra`, `origen`, `recibidoDe`,
  `hojaDelTexto`; lo demás (también lo que el PC no entiende, en `resto`) se copia;
- **archivos propios**, copiados y no enlazados (borrar el original se lleva los suyos): el adjunto
  en `guardados/<hora>_v<n>_<nombre>` (el nombre sin la hora de delante), el dibujo en
  `dib-<id>` (lienzo, página) o `foto-<id>` (foto), con sus `.marcas` y `.vistas.json`, y lo anotado
  `anot-<uid del original>*` copiado a `anot-<uid de la versión>*`.

Todo se nombra en rutas **portátiles** (las del móvil) y se pone donde `vista::DiscoPc::ruta` dice:
el adjunto acaba en `android/guardados/…` de la carpeta del chat y la versión lo apunta como
`pixpin:files/guardados/…`; el dibujo en `lienzos/dib-<id>.excalidraw`. Así viaja como el archivo
de cualquier mensaje, con el mismo nombre que le daría Android. `versionDe` y `versionN` son campos
del `Mensaje` del PC (también en el de Apple) con esos nombres y **no se escriben si no hay**: un
mensaje de siempre sale igual. Reenviar una versión a otro chat le quita el vínculo.

### Borradores y sin conexión

Nada de esto pide red: la versión se hace en este equipo y viaja en la siguiente vuelta de
sincronizar, como cualquier mensaje. Lo que se abre y aún no ha cambiado es un **borrador**,
apuntado en `<raíz>/sincro/versiones-en-curso.json` (no viaja: `sincro/` no es carpeta que se
mande). Al volver al chat (al cerrar el lienzo o el lector, o cuando el chat se relee) se mira cada
borrador: si cambió, entra en el chat como versión, abajo del todo; si no, se guarda por si su
editor sigue abierto y a las 12 horas se tira con sus copias. Abrir para mirar no deja nada en el
chat. Abrir otra vez lo mismo sin haberlo tocado sigue con el mismo borrador.

### En pantalla

- Encima de una versión: **«✎ Versión de Ana»** («Versión 2 de Ana», «Versión de lo tuyo» si el
  original es mío, «Versión de otro mensaje» si el original no está); tocarlo lleva al original.
  Debajo va la cita del original, por `respondeA`.
- En el original: **«✎ 2 versiones ▾»**, que abre la lista «Versión 1 · tuya», «Versión 2 · Ana»…;
  cada una lleva a la suya.
- Al abrir algo ajeno: «Es de Ana: si lo cambias, se guarda como tu versión y lo suyo no se toca».
  Al volver con cambios: «Se guardó como tu versión; el original no se ha tocado».

La regla, sin ventanas y con las pruebas de `VersionesVinculadasTest.kt`, está en
`crates/pixpin-proyecto/src/versiones.rs` (`tests/versiones_vinculadas.rs`); la ventana, en
`apps/pixpin/src/ventana_chat/versiones.rs`.

## Para Android y el Mac

- **No hay formato nuevo.** Lo que hace el «Chat compartido» del PC es exactamente
  `Compartidos.crear(chat, nombre, yo, ahora)` sobre el chat recién creado, lo mismo que «Compartir»
  en `ChatCompartidoActivity`. El `nombre` del `ChatCompartido` nace vacío y se rellena **en local**
  cuando se le pone nombre al chat, antes de invitar a nadie (no va firmado; `juntar` se queda con
  el que ya había, así que no se intenta propagar después).
- **Para verse igual**, en `MensajesActivity` habría que hacer lo mismo en los chats compartidos:
  lo mío a la derecha y lo de los demás a la izquierda, con el nombre encima de la primera de cada
  tanda (hoy Android ya pone el nombre, `autorDe`, pero todas las burbujas van al mismo lado). La
  regla de «mío» de arriba es la que hay que copiar; ojo con el punto 3: hoy `autorDe` solo deja
  fuera el código de **este** aparato, así que lo que escribo desde mi tableta sale con mi propio
  nombre encima. En el PC también le pasaba a la chapa `#47·K7Q2 · Max`; ya no.
- **El color de cada persona** es `tono(nombre)`: FNV-1a de 32 bits sobre los bytes UTF-8 del
  nombre sin espacios a los lados, módulo 7, sobre los siete colores de avatar de Telegram
  (`0xe17076, 0xfaa774, 0xa695e7, 0x7bc862, 0x6ec9cb, 0x65aadd, 0xee7aae`). Si los otros aparatos
  usan la misma cuenta, Ana tiene el mismo color en todos.
- **La chapa** es el icono `group` de Material sobre un círculo verde `#2e9e62`; en la lista, con el
  número de personas (`nombresDentro(c).size`) delante del nombre si son más de una.
- El «+» con dos opciones es solo del PC por ahora; en el móvil, el sitio natural es el FAB de
  la lista de proyectos.

## Sin probar todavía

- En un Windows de verdad con dos personas: las pruebas son de la lógica (`pixpin-ui`, y
  `en_el_chat` en `sincronizar/compartidos_ui.rs`) y el resto compila y pasa clippy.
