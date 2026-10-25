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
