# El comentario avanzado: la pegatina con su minichat (3-nov-2026)

Manual para hacer lo mismo en Android, Mac e iPad y que lo de un aparato se
vea y se junte en el otro. Lo pidió el usuario:

> «Un emoticono de comentario, cuatro tipos (tres de urgencia y uno de
> comentario normal), que pego donde sea como una pegatina; una vez puesto
> funciona como atajo a una foto, un texto o un elemento como una figura que
> cree, como un MINICHAT: mando una imagen y se abre en el lienzo, y también
> puedo anotar sobre su comentario.»

## 1. Qué es

- Una **pegatina** pegada en un punto de un documento: un redondel del color
  de su tipo con un piquito que baja hasta el punto.
- Cuatro **tipos**: `urgente` (rojo `#E53935`, «!!!»), `alta` (naranja
  `#FB8C00`, «!!»), `media` (amarillo `#FDD835`, «!», signo casi negro
  `#3E2F00`) y `nota` (azul `#5C6BC0`, el bocadillo). Donde no se pinta la
  pegatina se usa su emoticono: 🔴 🟠 🟡 💬.
- Su **hilo** (el minichat): entradas con autor, hora, texto y adjuntos
  (un fichero, una imagen, un atajo a un mensaje del chat o a un elemento de
  un lienzo, un lienzo propio). Una imagen se abre en el lienzo para anotar
  encima; lo anotado se guarda y viaja, unido a esa entrada.
- Se **resuelve** (y se reabre); de una urgencia se **crea una tarea** (en
  el Inbox, unida a la pegatina); se **quita** (queda su lápida).

## 2. El fichero (uno por documento, el de siempre)

Las pegatinas van en el **mismo `.comentarios.json`** que los comentarios de
las notas (`pindesk_docs::md_comentarios`), en la clave **`pegatinas`**. Lo
demás del fichero no se toca: quien escribe pegatinas conserva `comentarios`
y cualquier otra clave; quien escribe comentarios de una nota conserva
`pegatinas` (va en `resto`).

| Documento | Fichero | Adjuntos | Lienzos de las entradas |
|---|---|---|---|
| Adjunto del chat (PDF, Word, Excel, imagen, página, plano), nota | `pins/draw/anot-<uid>.comentarios.json` | `pins/draw/anot-<uid>.a<huella>.adjunto` | `pins/draw/anot-<uid>.c<id>.excalidraw.gz` |
| Lienzo del chat (`pins/draw/<d>.excalidraw.gz`) | `pins/draw/<d>.comentarios.json` | `pins/draw/<d>.a<huella>.adjunto` | `pins/draw/<d>.c<id>.excalidraw.gz` |
| Fichero suelto (no viaja) | `<fichero>.pixpin-comentarios.json` | `<fichero>.pixpin-anotado/comentarios/<huella>.<ext>` | `<fichero>.pixpin-anotado/comentarios/c<id>.excalidraw` |

```json
{
  "version": 1,
  "comentarios": [],
  "pegatinas": [
    {
      "id": "pc1-18f2a3b4c5d-0",
      "tipo": "urgente",
      "ancla": {"sitio": "pagina", "pagina": 2, "x": 120.5, "y": 700},
      "autor": "PC de Max",
      "aparato": "pc1",
      "creado": 1793582613265,
      "cambiado": 1793582613265,
      "resuelto": false,
      "tarea": "K7Q2M9X4PA",
      "hilo": [
        {
          "id": "pc1-18f2a3b4c5e-0",
          "autor": "PC de Max",
          "aparato": "pc1",
          "cuando": 1793582614000,
          "cambiado": 1793582614000,
          "texto": "La viga del eje B no cuadra",
          "adjuntos": [
            {"tipo": "imagen", "ruta": "pins/draw/anot-ABCDE23456.a9f1c2d3e4f5a6b7c.adjunto",
             "nombre": "foto.jpg", "tinta": "pins/draw/anot-ABCDE23456.cpc1-18f2a3b4c5e-0-0.excalidraw.gz"},
            {"tipo": "archivo", "ruta": "pins/draw/anot-ABCDE23456.a0123456789abcdef.adjunto", "nombre": "estructuras.pdf"},
            {"tipo": "mensaje", "chat": "<id del proyecto>", "mensaje": "<Codigos.unico>", "rotulo": "Nota del lunes"},
            {"tipo": "elemento", "lienzo": "d1730000000", "elemento": "<id>", "punto": [320.5, 1040], "rotulo": "Viga B"},
            {"tipo": "lienzo", "tinta": "pins/draw/anot-ABCDE23456.cpc1-18f2a3b4c5f-0.excalidraw.gz"}
          ]
        }
      ]
    }
  ]
}
```

- **Ids**: `<aparato>-<hora en hexadecimal>-<n>`, únicos en el fichero (el
  aparato los hace únicos entre aparatos). Los de las entradas, igual.
- **Horas**: milisegundos UTC. `cambiado` de la pegatina es la de lo último
  que se cambió de **ella** (tipo, ancla, resuelto, borrado, tarea); el de
  una entrada, lo último de **esa entrada**. Al cambiar algo, `cambiado`
  avanza siempre (`max(ahora, cambiado + 1)`), aunque el reloj vaya para atrás.
- **Nada se borra**: quitar una pegatina pone `"borrado": true`, vacía el
  texto y los adjuntos de sus entradas y las marca `borrado`; quitar una
  entrada, lo mismo con ella. Una lista vacía se escribe `[]`.
- **Adjuntos**: `ruta` y `tinta` son rutas **portátiles** (`pins/draw/…`,
  sin subcarpetas); fuera de un chat, rutas del disco. El fichero de un
  adjunto se guarda tal cual en `<base>.a<huella>.adjunto`, con `huella` =
  FNV-1a de 64 bits de su contenido en 16 cifras hexadecimales (el mismo
  fichero desde dos aparatos es uno; no cambia nunca). Su nombre y su
  extensión van en `nombre`. `tipo` es `archivo`, `imagen` (por la extensión
  del nombre: png, jpg, jpeg, gif, bmp, webp, tif, tiff), `mensaje`,
  `elemento` o `lienzo`; uno que no se conoce se conserva y no se abre.
- **El lienzo de una imagen** (`tinta`): lo anotado encima, un Excalidraw en
  píxeles de la imagen (la imagen va de fondo en `(0,0)-(ancho,alto)`). Se
  crea la primera vez que se abre para anotar (`<base>.c<entrada>-<k>`). El
  **lienzo propio** (`tipo: lienzo`) es un Excalidraw sin fondo.
- **Atajo a un elemento**: `elemento` es su id y `punto` su centro en
  coordenadas del mundo al hacer el atajo; ir a él centra la cámara en
  `punto` (el id se usa si se encuentra).
- Lo que no se conoce, en cualquier nivel, **se conserva** al reescribir.
- Un JSON roto **no se escribe encima** (se perdería lo que hubiera).

## 3. El ancla (`ancla`)

| `sitio` | Campos | Visores |
|---|---|---|
| `pagina` | `pagina` (desde 0), `x`, `y` | PDF y Office paginado: **puntos** de la página (sus medidas, y hacia abajo desde arriba a la izquierda). Hoja de cálculo: `pagina` = hoja del libro, píxeles de la hoja al 100 % (96 ppp), los de sus marcos. Imagen: píxeles de la imagen sin girar, `pagina` 0. |
| `escena` | `x`, `y` | Lienzo: coordenadas del mundo (las de los elementos Excalidraw, y hacia abajo). |
| `texto` | `sobre`, `cita`, `posicion` | Lo que se recoloca: `sobre` = `documento` (el lector de Word, libros y texto: sus párrafos unidos con `\n`), `markdown` (el `.md` tal cual) o `html` (los nodos de texto de `<body>`, como las marcas de texto). |

- `cita` = `{"type":"TextQuoteSelector","exact","prefix","suffix"}` y
  `posicion` = `{"type":"TextPositionSelector","start","end"}`: el mismo
  JSON que las marcas de texto (`docs/sincronizar-marcos-vistas-y-tinta.md`
  §3.10), posiciones en **unidades UTF-16**, hasta 32 de contexto por lado,
  sin blancos en los bordes. Un clic sin elegir cita la palabra del clic.
- Buscarla: (1) la cita, y si sale más de una vez la de más contexto igual y
  a igualdad la más cercana a `start`; (2) lo que haya entre `prefix` y
  `suffix` (no más del doble de la cita + 64). Si nada, la pegatina queda
  **sin sitio**: no se pinta, sale en la lista y no se borra.
- La pegatina se pinta con el piquito en el ancla (en un texto, al principio
  de la cita).
- Los números van a diezmilésimas.

## 4. Juntar dos aparatos

Al llegar un `.comentarios.json` que también cambió aquí
(`pindesk_sincro::disco::juntar_comentarios`, al lado de las lecciones), no
gana el fichero más nuevo: se **juntan las pegatinas**
(`pindesk_comentarios::fusion`):

1. Por `id`: la que solo tiene un lado entra.
2. Lo de la pegatina (todo menos el hilo): gana la de `cambiado` mayor; a
   igualdad, la de JSON (sin el hilo, compacto) mayor — así los dos eligen la
   misma sin hablar.
3. El hilo se suma entrada a entrada por `id`; la misma entrada en los dos,
   gana la de `cambiado` mayor (mismo desempate). Orden: `cuando` y luego `id`.
4. **Lo nuevo gana a lo quitado**: una pegatina quitada en un lado vuelve si
   en el otro tiene una entrada no borrada con `cuando` posterior al
   `cambiado` de la lápida (solo con eso: lo de antes ya se había quitado).
5. Lo que no son pegatinas (los `comentarios` de una nota) es lo que llega,
   como hasta ahora. Si lo que llega ya lo trae todo, no se reescribe.

Es conmutativo, asociativo e idempotente: no necesita la versión común.
Quien guarda también junta con lo que hay en el disco antes de escribir.

## 5. El espejo del lienzo (para los móviles sin pegatinas)

Cada pegatina de un lienzo lleva en su dibujo un **espejo**: un texto
Excalidraw con su emoticono, para que un móvil que aún no las conoce vea dónde
está (`pindesk_comentarios::espejo`):

```json
{"id": "comentario-<id>", "type": "text", "text": "🔴", "originalText": "🔴",
 "x": <cx-18>, "y": <cy-18>, "width": 36, "height": 36, "fontSize": 28.8,
 "locked": true, "isDeleted": false, "updated": <cambiado>, "version": <cambiado/1000>,
 "opacity": 100,
 "customData": {"pindesk": "emoji", "pixpin": "emoji",
                "comentario": {"id": "<id>", "tipo": "urgente", "resuelto": false}}}
```

- Quien conoce las pegatinas **no lo pinta ni lo deja coger** (en el PC es
  un elemento ajeno del lienzo) y lo **reescribe al guardar** el dibujo desde
  el fichero de comentarios. Resuelta: `opacity` 50. Quitada: `isDeleted`.
- Al abrir, un espejo **más nuevo** que su pegatina (`updated` > `cambiado`:
  lo movió o lo borró un móvil) se aplica: la mueve (al centro del espejo),
  le pone su tipo y resuelto, o la quita. Un espejo sin pegatina se vuelve
  pegatina sin hilo.

## 6. Lo que viaja (sincronizar, nube y «sin conexión»)

- `TERMINACIONES` (`crates/pindesk-sincro/src/anotado.rs`) lleva ya
  `.comentarios.json` y ahora **`.adjunto`**: los `anot-<uid>.a….adjunto` y
  `anot-<uid>.c….excalidraw.gz` viajan y se borran con su mensaje.
- Las de un lienzo van con su dibujo por `alcance` (como `<d>.marcas` y
  `<d>.vistas.json`): `comentarios_junto_a` y lo adjuntado que nombra el
  propio fichero (`rutas_de_comentarios`).
- «Sin conexión» (`nube::sin_conexion::pertenece`): la clave de un adjunto
  retiene `anot-<uid>…` (también los `.adjunto` y los lienzos `c…`); la de un
  lienzo, `pins/draw/<d>.*` (sus pegatinas, adjuntos y lienzos).
- **Android tiene que añadir `.adjunto` a su `TERMINACIONES`** y, para los
  lienzos, `<d>.comentarios.json` y sus adjuntos al alcance del dibujo.

## 7. La interfaz (igual en todos los visores)

- **Botón de comentar** abajo a la derecha (isla blanca de 40 px, el
  bocadillo, con el número de las abiertas en rojo). Abre la **tira** a su
  izquierda: las cuatro pegatinas y la lista.
- Elegir un tipo y **pulsar en el documento la pega** ahí y abre su
  minichat. Arriba al centro dice «Haz clic donde quieras pegar el
  comentario (Esc para salir)».
- **Clic** en una pegatina: su minichat. **Arrastrar** (más de 4 px): moverla.
  **Clic derecho**: abrir, los cuatro tipos (con ✓ el de ahora), resolver o
  reabrir, crear tarea (solo urgencias abiertas sin tarea) y quitar.
- **El minichat**, junto a la pegatina (a su derecha si cabe): cabecera con
  el redondel (un clic pasa al tipo siguiente), el tipo y el documento,
  quitar, resolver y crear tarea; el hilo (lo último abajo, rueda para
  subir); la caja de escribir (**Intro** manda, Mayús+Intro parte el renglón)
  con adjuntar, pegar (**Ctrl+V** una imagen o ficheros), atajo (el elemento
  elegido del lienzo o un mensaje de los últimos del chat), lienzo nuevo y
  enviar. Soltar ficheros encima los adjunta. Un clic en una imagen la abre
  en el lienzo para anotar; en un fichero, lo abre el sistema; en un atajo,
  va a él. Clic derecho en una entrada propia, la quita. Escape o un clic
  fuera la cierra.
- **La lista «Comentarios (n)»**, encima del botón: las abiertas primero,
  por urgencia (urgente, alta, media, nota) y dentro lo más reciente arriba;
  las resueltas al final. Un clic en una fila lleva la vista a su pegatina.
- **Crear tarea**: «🔴 <lo primero que dice> — <documento>» en el Inbox de
  «Mensajes guardados»; la pegatina guarda el código del mensaje en `tarea`.

## 8. Dónde está (PC)

| Pieza | Fichero |
|---|---|
| Modelo, ancla, fichero, fusión, espejo, lista | `crates/pindesk-comentarios/` |
| Cajas de la pantalla (pegatina, botón, tira, menú, lista, minichat) | `crates/pindesk-ui/src/pegatinas.rs` |
| El componente de los visores, el minichat, el pintado | `apps/pindesk/src/comentarios.rs`, `apps/pindesk/src/comentarios/` |
| Juntar al llegar, alcance, `.adjunto` | `crates/pindesk-sincro/src/disco.rs`, `anotado.rs`, `prioridad.rs` |
| El espejo como elemento ajeno | `crates/pindesk-motor2d/src/excalidraw.rs` (`elemento_desde`) |
| Lienzo | `apps/pindesk/src/ventana_editor/marcas.rs` (con el riel) |
| PDF y Office | `apps/pindesk/src/lector_pdf/comentar.rs` |
| Word en lectura, libros, texto | `apps/pindesk/src/visor/comentar.rs` |
| Hojas de cálculo | `apps/pindesk/src/visor_hoja/comentar.rs` |
| Plano CAD | `apps/pindesk/src/plano_cad/anotar.rs` (`UbicarPlano`), con `Anotador::antes` de `crates/pindesk-cad/src/anotado.rs` |

## 9. Qué visores lo tienen (3-nov-2026)

- **Hecho**: lienzo (también el de una foto del chat: la foto va de fondo en
  sus píxeles, así que el ancla de escena son píxeles de la imagen), lector
  de PDF y Office (Word, PowerPoint), lector que reajusta el texto (Word en
  modo lectura, EPUB, texto), hojas de cálculo y plano CAD (ancla `pagina` 0
  en unidades del dibujo respecto a su origen, con y hacia **arriba**; se ven
  y se tocan también sin anotar).
- **Pendiente**: los pines de imagen del escritorio (`crates/pindesk-pin`:
  hace falta un pedido al gestor como el de los marcos), y el visor de HTML
  y el lector de notas Markdown (la página del WebView: el formato ya está,
  ancla `texto` con `sobre` `html` o `markdown` y las mismas cuentas que
  `marcas-texto.mjs`; falta el guion de la página y su mensaje al PC).
