# La interfaz del chat: funciones y chats

Lo que se ve y se toca en la ventana del chat, seccion por seccion. Cada
seccion es de una funcion y se puede leer sola.

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
