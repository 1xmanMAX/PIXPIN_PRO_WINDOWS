# La nube de los chats compartidos (PinDesk Windows)

> 1-nov-2026. Lo pidió el usuario: «Los chats personales siguen por la red
> local (gratis, sin servidores). Los chats **compartidos** ahora pueden ir
> por la red local o por la nube: al crear uno elijo Local o Nube. Si es
> Nube, los ficheros van a mi servidor (que también los enseña) y se bajan
> en los aparatos de los demás. Subir **lo menos posible**: si dibujo una
> raya en un plano, sube esa raya, no el plano otra vez. Y un botón En
> línea / Sin conexión en los documentos.»

El servidor es la web de la obra (repositorio `agente-2.0-constrution`,
manual `docs/pixpin-nube.md`, §3–§4 el protocolo 1 y §10 la capacidad
`deltas1`). Aquí está lo del PC; lo de dentro, sin ventanas y probado, en
`crates/pindesk-sincro/src/nube/`.

## Qué va por dónde

| Chat | Red local (mDNS, protocolo 4) | Nube |
|---|---|---|
| Personal (el general y los proyectos propios) | sí, como siempre | no (el manual lo permitiría como espejo; no se ofrece aún) |
| Compartido en **Local** | sí, como siempre | no |
| Compartido en **Nube** | sí, si estáis en la misma Wi-Fi | sí: sube y baja desde donde estés |

Un chat compartido en la nube **sigue** sincronizándose por la red local
cuando toca: las dos vías juntan con la misma regla y no se pisan.

## Ponerlo en marcha

1. En la web: **PinDesk nube → dar de alta este aparato**. Sale una llave
   `pxn_…` (se ve una sola vez).
2. En el PC: **Sincronizar → Nube**: la dirección de la web (`https://…`;
   `http://` solo hacia `127.0.0.1`/`localhost`, para pruebas), la llave, y
   **Guardar**. **Probar** dice el espacio y la persona.
   - La dirección se guarda en `<datos>/sincro/nube/ajustes.json` (no
     viaja: `sincro/` no está en las carpetas que viajan).
   - La llave, **cifrada con DPAPI** en
     `%LOCALAPPDATA%\PinDesk\claves\nube.clave`, como las de la IA. Nunca
     en el TOML de ajustes ni en `tracing`.
3. El aparato se presenta solo (`PUT /yo`, firmado con su clave P‑256 de
   `compartidos.rs`): así la nube sabe en qué chats compartidos está.

## Crear o pasar un chat a la nube

- El «+» de la lista tiene ahora tres: **Chat**, **Chat compartido** (red
  local, lo de siempre) y **Chat compartido en la nube**.
- Un compartido que ya existe: **clic derecho → Compartir con personas**;
  arriba, «Por dónde va»: **Red local / Nube**.
- Quien está dentro de un compartido que otro puso en la nube lo ve
  entrar solo (`GET chats`, `novedades`): se baja entero la primera vez. Si
  lo pasa a «Red local» a mano, no vuelve solo.
- En la lista, los chats de la nube llevan una **nubecita** arriba a la
  derecha del avatar: azul subiendo, verde al día, gris sin red, roja si
  la llave no vale.

La lista de chats de la nube vive en `sincro/nube/chats.json`
(`ChatsEnLaNube`: `chats`, los puestos a mano en `locales`, y el `seq` de
las novedades). **No** va en `compartidos.json`, que es byte a byte el de
Android.

## Cómo sube y baja (el hilo `nube`)

- Un hilo mira cada 3 s la firma barata (número de ficheros, tamaños,
  última fecha) de la carpeta de cada chat de la nube; cuando cambió y lleva
  **4 s quieta**, hace una vuelta (`pindesk_sincro::nube::vuelta`). Otro hilo
  espera las novedades con la conexión abierta (`novedades?espera=25`) y
  despierta al primero en cuanto otro sube. Sin red, espera creciente
  (`reintentos::espera_tras_fallo`). Nada de esto corre en el hilo de la
  ventana.
- **Lo sabido de cada chat** (`sincro/nube/chats/<chat>.json`,
  `EstadoDeChat`): la última revisión, el resumen de cada mensaje (por su
  `id`), el del proyecto y la lista firmada, y de cada fichero su SHA‑256,
  sus deltas y la versión de cada trazo que la nube ya tiene. La diferencia
  se saca contra eso.
- **Con `deltas1`** (si `hola.capacidades` lo dice): `GET cambios` desde la
  última revisión y `PATCH` con solo lo cambiado: los mensajes enteros que
  cambiaron, `quitar` por `id`, el proyecto si cambió, la lista firmada si
  cambió, y de cada fichero su línea. Un 409 baja los cambios de otro, junta
  y vuelve a sacar el parche (hasta 4 veces). No se manda un segundo PATCH
  para confirmar (subiría la revisión); los `faltan` se suben con `PUT
  objetos`. Un parche de más de 4 MB va como inventario entero (`PUT`).
- **Sin `deltas1`** (una nube del protocolo 1): el inventario entero con
  `PUT` y `base`, los objetos que falten, y otra vez el `PUT`. El 409 baja el
  inventario, junta y vuelve.
- La primera vez de un chat (revisión 0) se baja antes de subir.

### Los trazos: solo la raya nueva

Un lienzo o una tinta (`*.excalidraw.gz`) sube su **original** (gzip, sin
fecha: el mismo SHA cada vez) y después **deltas**: un objeto JSON con
gzip `{pixpinDelta:1, tipo:"excalidraw", rel, base, elementos, borrados,
archivos?, appState?}` con solo los elementos nuevos o que **ganan** a lo
que la nube tiene, las fotos nuevas y el `appState` si cambió.

- **La regla** (`nube::trazos::gana`, la de Excalidraw): por cada `id`, la
  `version` mayor; a igualdad, el `versionNonce` menor; si empatan, se
  queda el que estaba. Lo borrado es una **lápida** (`isDeleted: true`, con
  la versión subida) y viaja en `elementos`. `borrados` solo lo aplica
  (lápida, `version + 1`): el PC nunca quita un elemento de su lista, y lo
  que falta suele ser lo que aún no llegó, así que no lo manda.
- **Al bajar**: si la línea de la nube es la misma foto con más deltas, se
  bajan solo los nuevos y se ponen encima del fichero de aquí con la regla
  (dos personas que pintan a la vez suman sus trazos). Si cambió la foto
  (alguien compactó), foto + todos los deltas, y se junta con lo de aquí
  elemento a elemento.
- **Compactar**: con más de 20 deltas o cuando los deltas pesan más de la
  mitad de la foto, se sube una foto nueva con `deltas: []`.
- Lo demás (`.marcos.json`, `.vistas.json`, `.marcas`, `.hoja`, tablas,
  PDF…) se sube entero cuando cambia su SHA. Un JSON que cambió aquí y allí
  se junta con `fusion::json`; lo demás que cambió aquí se queda y sube en
  la vuelta siguiente.
- Lo que llega **nunca escribe fuera de su chat**: `disco::permitida`,
  `compartidos::salta_de_carpeta` y la valla de los ficheros de los demás
  chats, como lo de una persona por la red local.

### Cuánto sube una raya

| | Protocolo 1 (antes) | `deltas1` |
|---|---|---|
| Chat de 201 mensajes y un lienzo de 300 trazos; **una raya de 30 puntos y un mensaje cambiado** (nube falsa, `nube::pruebas`) | 139 974 B (inventario + lienzo entero) | **842 B** de parche + **518 B** de delta |
| Una raya de 30 puntos contra la **nube de referencia de verdad** (`nube::de_verdad`) | — | **383 B** de parche + **329 B** de delta |

## «En línea / Sin conexión»

En un visor de un documento de un chat de la nube, un botón dice **En
línea** (nube verde) o **Sin conexión** (nube tachada, dorada). Por
omisión, en línea. Fuera de un chat de la nube no sale.

- **Sin conexión** (`nube::sin_conexion::SinConexion::retener`): se copia
  aparte lo anotado del documento (`anot-<uid>…`, o el lienzo
  `pins/draw/<referencia>.excalidraw.gz` y lo de su lado; en
  `sincro/nube/sin-conexion/`) y la vuelta **no sube** esos ficheros.
- **Lo de los demás se sigue viendo**: lo que llega se escribe en el
  fichero de verdad como siempre (el visor lo relee y lo junta *debajo* de
  lo suyo, `lector_pdf::al_dia`) **y también en la copia**, que es siempre
  «lo de la nube sin lo mío sin conexión».
- **Volver a en línea**: se tira la copia y la vuelta siguiente sube lo
  anotado como delta (solo los trazos nuevos).
- **Salir del documento sin conexión**: se pregunta. **Sí** (por omisión,
  como pidió el usuario): descartar lo anotado sin conexión (la copia vuelve
  a su sitio, lo nacido sin conexión se borra; lo de los demás se queda).
  **No**: volver a en línea y subirlo. **Cancelar**: dejarlo sin conexión
  para la próxima vez que se abra.

### Dónde está

| Visor | Dónde va el botón | Qué retiene (la clave) |
|---|---|---|
| Lector de PDF, y de Word/PowerPoint convertidos (`lector_pdf`) | arriba a la izquierda | `anot-<uid>…` del adjunto |
| Lector de PDF con **el PDF de un proyecto** (su `pdfOrigen` o su copia limpia) | arriba a la izquierda | `anot-<unico del proyecto>…` (marcadores, espacios) **y el lienzo de cada hoja con pagina** (`pins/draw/<dibujo>.excalidraw.gz` y su `.marcas`; `pdf-<huella>-p<i>` si aun no tenia) |
| Lector de Word, libros y páginas (`visor`) | arriba a la izquierda | `anot-<uid>…` (también `-texto`) |
| Visor de hojas, un Excel o una tabla del chat (`visor_hoja`) | arriba a la izquierda de la hoja, bajo la barra de fórmulas | `anot-<uid>-h<i>…` |
| Plano CAD (`plano_cad`, `pindesk_cad::ventana`) | un botón de su barra, antes de luna/sol | `anot-<uid>…` (marcadores, marcos, lo de cada marcador) |
| Modelo BIM / 3D (`modelo_bim`, `pindesk_cad::ventana3d`) | igual, en su barra | `anot-<uid>-m<id>…` y sus marcadores |
| Lienzo de una hoja DIBUJO o PAGINA, y el de una foto del chat (`ventana_chat::abrir_una_hoja` → `ventana_editor`) | arriba a la izquierda (no presentando) | la **referencia** del lienzo |
| Visor de HTML, una pagina web del chat (`visor_html`) | arriba a la izquierda, en su ventanita encima del WebView (`visor_html::riel`, como el riel) | `anot-<uid>…`: tinta, marcas y marcos del riel, vistas y las **marcas de texto** (`anot-<uid>.marcas-texto.json`) |
| Lector de notas Markdown (`notas_md::lector_web`), una nota de un proyecto o un `.md` del chat | igual, en su ventanita | `anot-<codigo>…` de la nota (lo mismo, y sus comentarios) |

La clave la saca `en_linea::Clave` con `sincronizar::nube::documento`: de
un documento, su adjunto (`pindesk_proyecto::anotado::adjunto_de`) o, si no
es un adjunto, la base `anot-<codigo>` de lo suyo
(`anotado_del_adjunto::base_de`: una nota de un proyecto abierta en el
lector de notas, `cache/riel-notas/<ficha>-<codigo>.md`); de un lienzo, la
ficha y su referencia (`en_linea::referencia_de_la_hoja`).

**El PDF de un proyecto** escribe en dos sitios: sus marcadores en
`anot-<unico del proyecto>…` (`anotado::base_del_pdf`) y la tinta de cada
hoja en el lienzo de **la hoja del proyecto con esa pagina**
(`capas_del_pdf::dibujo_de_la_pagina`). Su clave lleva las dos cosas:
`clave_con_lienzos(codigo, dibujos)` = `codigo|dibujo0|dibujo1…` (`|` no
cabe en un nombre de fichero), con los dibujos de
`capas_del_pdf::dibujos_de_las_paginas` (el de cada hoja con `pagina`, o el
`pdf-<huella>-p<i>` que tendra al escribir, con la misma huella del lector).
Dos claves con el mismo codigo son el mismo documento (`misma_clave`): si
mientras esta sin conexion el movil le pone dibujo a una hoja, se sigue
viendo «Sin conexión» y descartar mira las dos. Apuntar el dibujo nuevo en
`proyecto.json` no se retiene (es el proyecto, no la tinta): sin su lienzo
la hoja queda vacia.

Las pruebas de que cada clave retiene lo que escribe su visor, y nada más:
`nube::sin_conexion::pruebas::la_clave_de_cada_visor_retiene_lo_que_ese_visor_escribe`,
`la_clave_del_pdf_de_un_proyecto_retiene_sus_hojas_y_nada_mas`,
`dos_claves_con_el_mismo_codigo_son_el_mismo_documento` y
`retener_el_pdf_de_un_proyecto_copia_sus_hojas_y_descartar_las_devuelve`.

Los **visores web** (HTML y notas) pintan sus mandos en ventanas de
DirectComposition encima del WebView (el riel y la tinta,
`visor_html::riel`), así que el botón va igual: una tercera ventanita del
tamaño justo del botón, arriba a la izquierda de la vista, que no tapa la
página. Al cerrar, como los demás: ya guardado lo del riel y escondida la
ventana, si quedó sin conexión se pregunta.

No lo llevan:

- El croquis 3D: no tiene tinta.
- El pin de Office (`pin_office`): su tinta y sus marcos son solo mientras
  está abierto (la vista previa de Office no dice por dónde va); no se
  guarda nada que pueda subir.
- **El lienzo de un pin o de una foto suelta (`.pixpin2d`)**: **nunca viaja
  a la nube**. Un pin vive en el almacén de pines (`almacen/objetos/…`,
  `pindesk_store`), fuera de cualquier chat, y su `.pixpin2d` está a su lado
  (`pines::ruta_anotacion`). La vuelta de la nube solo manda el `alcance` de
  un chat (`pindesk_sincro::disco::alcance_de`): las rutas que nombran sus
  mensajes, lo anotado `anot-<uid>…` de sus adjuntos y los lienzos de sus
  hojas y fotos; ningún mensaje nombra un `.pixpin2d`. El `.pixpin2d` que
  el PC dejaba junto a una **foto del chat** tampoco viaja: se adopta en el
  lienzo de la foto (`pindesk_proyecto::lienzo_de_la_foto`,
  `foto_anotada::asegurar_lienzo`), que sí viaja y ya lleva el botón (la
  referencia del lienzo). Sin nada que retener, no hace falta el botón en
  la barra de anotar del pin.

Para otro visor: guardar un `en_linea::EnLinea::de(doc)` (o `de_lienzo`),
pintarlo encima de lo suyo, pasarle los clics antes y llamar a `al_cerrar`
ya guardado lo suyo. Un visor que pinta su barra con otros medios
(`pindesk_cad`) recibe `en_linea::para_cad(doc)`: un `EnLinea` de la barra
(si está sin conexión y cambiar) y el mismo `EnLinea` para `al_cerrar`. El
lienzo no sabe de chats: quien lo abre pone `en_linea::en_lienzo` mientras
dura.

## Las marcas de texto (para la web de la nube)

Desde el 2-nov-2026 lo anotado de un documento trae un fichero más,
`anot-<uid>.marcas-texto.json` (terminación `MARCAS_TEXTO`): las marcas
(resaltar, ondulada, subrayar, tachar, tapar) que se hicieron leyendo una
**nota Markdown** o una **página web**. Sube y baja como los demás JSON de
lo anotado: entero, gana el más nuevo (no es un delta de trazos).

Para que la web de la nube (AGENTE) las enseñe, lo mismo que hace el PC:

- El formato, con un ejemplo, está en
  `docs/sincronizar-marcos-vistas-y-tinta.md` §3.10: cada marca lleva su
  cita como los selectores de la W3C (`TextQuoteSelector` con `exact`,
  `prefix`, `suffix`; `TextPositionSelector` con `start`, `end` en UTF-16).
- `sobre: "markdown"`: las posiciones cuentan sobre el `.md` tal cual (la
  web tiene que llevarlas al HTML que pinta: lo más sencillo es buscar la
  cita en el `.md`, y pintar sobre el texto renderizado buscando `exact` con
  su contexto, sin las marcas de Markdown). `sobre: "html"`: sobre los nodos
  de texto de `<body>` de la página, en orden, sin `script`/`style`/
  `noscript`/`template`/`textarea`/`select`.
- Buscar y pintar: `herramientas/lector-md/src/marcas-texto.mjs` es JS sin
  dependencias (`ubicar`, `reglasCss`) y sirve tal cual en el navegador;
  pintar con `CSS.highlights` no toca el DOM de la página.
- Una marca que no se encuentra no se pinta y **no se borra** al reescribir.
- Las celdas marcadas de una hoja de cálculo y lo marcado en un PDF o un
  Word no son este fichero: son trazos de su capa (`marca-texto-*` en
  `groupIds`), que la web ya pinta como la tinta.

## Pruebas

- `cargo test -p pindesk-sincro nube`: los deltas (regla, empates,
  lápidas, compactar), los cuerpos JSON, y dos aparatos contra una nube
  falsa en memoria (`nube::falsa`, con las reglas de §10): una raya, dos
  personas a la vez con 409, protocolo 1, compactar y sin conexión.
- **Contra la nube de referencia de verdad**:
  `PINDESK_NUBE_AGENTE=/ruta/a/agente-2.0-constrution cargo test -p
  pindesk-sincro nube::de_verdad` arranca su servidor
  (`crates/pindesk-sincro/pruebas/nube-de-referencia.mts`, con `tsx` de su
  `node_modules`) y le habla por HTTP con `nube::http::HttpLocal`: la firma
  de `PUT /yo`, un compartido que Max sube y Ana baja, una raya como delta y
  dos personas que pintan a la vez. Sin la variable, se salta.

## Para Android, Mac e iPad

Lo mismo, con el mismo formato: `docs/pixpin-nube.md` §10 del servidor y
`crates/pindesk-sincro/src/nube/` como ejemplo (es Rust portable: el Mac lo
puede usar tal cual con otro transporte). Lo que conviene copiar:

- el modo por chat **fuera** de `compartidos.json` (`sincro/nube/chats.json`);
- lo sabido por chat (revisión, resumen por `id` de mensaje, SHA y versión
  de cada trazo) para sacar el parche;
- `quitar` de mensajes por **`id`**, deltas con gzip, lápidas en
  `elementos`;
- «Sin conexión» como copia aparte + no subir, nunca como otra carpeta que
  el visor tenga que conocer.
