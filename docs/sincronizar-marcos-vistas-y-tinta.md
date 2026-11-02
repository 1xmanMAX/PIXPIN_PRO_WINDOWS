# Marcos, marcadores con vista y tinta: qué viaja y cómo (20-oct-2026)

Manual para seguir desarrollando en otros dispositivos (otro PC, el móvil
Android) y que lo hecho aquí se sincronice con ellos. Cubre lo añadido entre
el 19 y el 20 de octubre de 2026 en PinDesk (PC):

- los **marcos numerados** para imprimir (como los del plano: I, arrastrar,
  Supr, Ctrl+P en grupo) en todos los visores;
- los **marcadores con vista** (sitio **y zoom**; en un modelo 3D, toda la
  cámara y el estado);
- la **tinta** de Excel, de las páginas web y de las notas;
- los **marcadores de los planos (DWG) y de los modelos (BIM)**, con la tinta
  de cada marcador 3D;
- la interfaz común (riel lateral, fichas de los marcos, globos con atajo,
  teclas), que no viaja como datos pero tiene que verse igual en todos.

## 1. Cómo sincroniza PinDesk (recordatorio)

- Solo **LAN**: mDNS `_pindesk._tcp.` en el puerto 47474, canal TCP cifrado
  AES-256-GCM con el código del grupo (que nunca viaja). Mensajes JSON al
  estilo kotlinx. `VERSION = 4` (estricta).
- La unidad es **un chat** (el general y uno por proyecto). Viajan sus
  mensajes, el `proyecto.json` y **los ficheros de su alcance**.
- Lo que acompaña a un adjunto viaja **solo** si se llama
  `pins/draw/anot-<uid><terminación>` (carpeta plana, sin subcarpetas) y la
  terminación está en la lista blanca `TERMINACIONES`
  (`crates/pindesk-sincro/src/anotado.rs`). Se borra con su mensaje.
- `<uid>`:
  - un adjunto: el `Codigos.unico` de su mensaje;
  - el PDF de un proyecto: el uid del proyecto;
  - un PDF leído como texto: `<uid>-texto`;
  - una nota o una hoja `nota`: su `codigo_unico`.
- `uid_del_nombre` corta el uid en el primer `-` o `.`, así que
  `anot-<uid>-h1…` y `anot-<uid>-m<id>…` son del mismo mensaje.
- **No hay lápidas de ficheros**: un fichero que solo tiene un lado se envía
  o se pide siempre. Por eso **vaciar nunca es borrar**: se escribe `[]`,
  `{}`, `""` o una capa con todo `isDeleted`. Si se borrase, volvería.
- Los ficheros JSON que no son `.excalidraw.gz` ni `tablas/*.json` se
  resuelven entero, gana el más nuevo (con la deriva de reloj corregida).
  Las capas `.excalidraw.gz` se mezclan figura a figura.

## 2. Terminaciones nuevas

Se añadieron a `TERMINACIONES` (PC):

| Terminación | Constante | Qué es |
|---|---|---|
| `.marcos.json` | `MARCOS` | los marcos numerados de un documento |
| `.vistas.json` | `VISTAS` | lo que guarda cada marcador además de su sitio |
| `.marcas-texto.json` | `MARCAS_TEXTO` | las marcas de texto de una nota o una página web (2-nov, §3.10) |

**Android tiene que añadir las mismas a su `TERMINACIONES`** (Kotlin).
Sin eso las guarda al llegar pero no las lista: el PC las reenvía en cada
ronda, Android no devuelve sus cambios y no las borra con el mensaje (ya pasa
con `.asistente.json`).

## 3. Los ficheros, uno por uno

En el PC, `C` es la carpeta del chat (`almacen::carpeta(raiz, ficha)`): lo de
texto va en `C/android/pins/draw/<nombre>` y la tinta en
`C/lienzos/<base>.excalidraw` (JSON sin comprimir en el PC; `.excalidraw.gz`
en el nombre portátil). En Android, todo en `files/pins/draw/`.

### 3.1 Marcos numerados — `anot-<uid>.marcos.json`

Para PDF, Word, Excel, páginas web, notas e imágenes adjuntas.

```json
[
  { "caja": [x0, y0, x1, y1], "parte": 0 },
  { "caja": [x0, y0, x1, y1], "parte": 2, "vista": "…" }
]
```

- El orden de la lista **es** el número (1, 2, 3…). Quitar uno renumera.
- `caja` va ordenada (x0 ≤ x1, y0 ≤ y1), en las **unidades del documento** de
  cada visor (y hacia abajo):
  - PDF y Word paginado: unidades del lector (`ANCHO_HOJA` de ancho por hoja,
    y = todas las hojas seguidas);
  - Word en modo lectura: unidades del documento reflujado;
  - Excel: píxeles de la hoja al 100 % (96 ppp), con `parte` = hoja (desde 0);
  - web y notas: píxeles CSS del documento;
  - imagen: píxeles de la imagen guardada, sin girar.
- `parte` vale 0 si no hay partes. `vista` solo si el visor la necesita.
- Vacío: `[]`.
- Lógica (igual que el plano): botón del marco o **I** entra y sale; arrastrar
  más de 8 px pone uno; **Supr** quita el último; **Ctrl+P** imprime todos,
  una hoja por marco, acercándose a cada uno hasta llenar la ventana y con
  sus colores de verdad (sin modo noche). Fichas 1, 2, 3… abajo a la
  izquierda: pulsar una va a ese marco.

### 3.2 Vistas de los marcadores — `anot-<uid>.vistas.json`

En los documentos (PDF, Word, Excel, web, notas): un objeto de id del
marcador a texto.

```json
{ "1792…": "1.75 120 3400.5" }
```

- `"zoom x y"`: el zoom y la esquina de arriba a la izquierda en unidades del
  documento (`zoom_a_texto` / `zoom_de_texto` en `marcos_doc.rs`).
- El marcador en sí sigue donde estaba: `anot-<uid>.marcas` (Word, web,
  notas: `id:fraccion:emoji|…`; PDF y Excel: `id:x:y:emoji|…`). Un marcador
  sin vista vuelve solo a su sitio, como antes.
- Vacío: `{}`.

En **planos y modelos** (DWG/DXF, IFC/RVT…) el mismo nombre lleva otro
formato (se distingue por el tipo del adjunto):

```json
{ "version": 1,
  "plano":  [ {"id":"m…","nombre":"","x":0,"y":0,"ancho":0,"alto":0} ],
  "modelo": [ {"id":"m…","nombre":"","estado":{ … Estado3d … }} ] }
```

- `plano` (2D): `x`, `y` = centro de la vista y `ancho`, `alto` = lo que se
  ve, en coordenadas absolutas del dibujo (y hacia arriba). Al volver:
  `max(ancho/w, alto/h)` unidades por píxel.
- `Estado3d`:

```json
{"objetivo":[x,y,z],"rumbo":r,"altura":a,"dist":d,
 "seccion":[x0,y0,z0,x1,y1,z1] | null,"aislado":n | null,
 "aislado_clave":"<clave>",
 "ocultos":["n:=<clave del nivel>","n:-","t:<Tipo>"],"claro":false,"aristas":true}
```

  - `objetivo` en coordenadas absolutas del mundo, Z arriba; `rumbo` y
    `altura` en radianes (|altura| ≤ 1,5697); `dist` en unidades del modelo.
    El ojo: `objetivo + dist·[cos a·cos r, cos a·sin r, sin a]`, campo
    vertical de 45°.
  - `seccion`: la caja del corte, coordenadas absolutas.
  - **Claves estables (20-oct-2026)**, iguales en todos los aparatos
    (`Modelo3d::clave_de_elemento` / `clave_de_nivel` en `pindesk-cad`):
    - elemento: su `GlobalId` del IFC (un Revit se pasa a IFC); sin él,
      `~<tipo IFC>|<nombre>|<k>`, con `k` = cuántos elementos sin `GlobalId`
      del mismo tipo y nombre van antes en el fichero;
    - nivel: el `GlobalId` del `IfcBuildingStorey`; sin él,
      `<nombre>@<cota en mm, redondeada>`.
  - `aislado_clave` es lo que manda; `aislado` (el número en el modelo del
    PC que lo guardó) es solo una pista y Android puede ignorarlo. Un
    marcador de antes sin `aislado_clave` se resuelve por `aislado`.
  - En `ocultos`, `n:=<clave>` es un nivel por su clave; `n:<i>` (lo de antes
    del 20-oct) es su número y se sigue entendiendo. Al leer, una clave que
    no está en el modelo se ignora.
  - **Android**: al guardar, escribir `aislado_clave` y `n:=<clave>`; al
    abrir, buscar el elemento y el nivel por esas claves con las mismas
    reglas.
  - Una vista «coincide» con un marcador si objetivo, distancia y ángulos
    difieren menos de ~2e-3 (relativo), el corte menos de 1e-3·dist, y
    lo aislado (por `aislado_clave` si los dos la traen; si no, por
    `aislado`) y `ocultos` son iguales (`claro` y `aristas` no cuentan).

### 3.3 Marcos 3D — `anot-<uid>.marcos.json` (de un modelo)

```json
{ "version": 1, "marcos": [ {"id":"m…","caja":[x0,y0,x1,y1],"estado":{ … Estado3d … }} ] }
```

- `caja` en la pantalla: desde el centro de la ventana, en unidades de su
  alto, x a la derecha, y hacia abajo (`px = x·h + w/2`, `py = y·h + h/2`).
- Se ve solo con la vista de su `estado`; Ctrl+P aplica cada estado, pinta
  sin interfaz y lo fotografía.

### 3.4 Tinta de cada marcador 3D — `anot-<uid>-m<id>.excalidraw.gz` + `.hoja`

- Lo anotado sobre un marcador del modelo: elementos Excalidraw en la
  pantalla, desde el centro de la ventana, en milésimas de su alto, y hacia
  abajo (`x = (px − w/2)·1000/h`).
- `.hoja`: `-2500,-2500,2500,2500` y la línea de versión del marco.
- Solo se ve cuando la vista coincide con el marcador (y quieta).
- `<id>` tal cual (los del PC empiezan por `m`: `anot-<uid>-mm18f…`),
  saneado a `[A-Za-z0-9_-]`.
- Quitar un marcador: su capa se guarda con todo `isDeleted`; no se borra.

### 3.5 Tinta de Excel — `anot-<uid>-h<i>.excalidraw.gz`

- Una capa por hoja (`i` desde 0), en píxeles de la hoja al 100 %, con la
  rejilla del propio libro (igual en todos). **Sin `.hoja`**.

### 3.6 Tinta de páginas web y notas — `anot-<uid>.excalidraw.gz`

- En píxeles CSS del documento, sin desplazamiento ni `.hoja`.
- **Ojo Android**: es el mismo nombre que la tinta de un Word; a un adjunto
  html o md **no** se le aplica la regla de columnas del Word (el `izq` de la
  `.maqueta`).
- Los marcadores de la página: `anot-<uid>.marcas` (`id:fraccion:emoji`).

### 3.7 Planos 2D

- Su tinta, cotas y marcos para imprimir siguen en la capa del plano
  (`anot-<uid>.excalidraw.gz` + `.hoja`), como antes. Los marcadores 2D, en
  `.vistas.json` (§3.2).

### 3.8 Vista de cada marcador de un lienzo — `pins/draw/<d>.vistas.json` (25-oct-2026)

Como las vistas con nombre de AutoCAD: al plantar un marcador en un lienzo
se guarda **lo que se ve en pantalla** (encuadre y aumento), y al ir a él se
vuelve a ver eso mismo.

- Va al lado del dibujo y de sus marcas: `pins/draw/<d>.excalidraw.gz`,
  `pins/draw/<d>.marcas` (`id:x:y:emoji|…`, sin cambios) y
  `pins/draw/<d>.vistas.json`. No lleva `anot-`: viaja con el dibujo por
  `alcance` (`vistas_junto_a`, al lado de `marcas_junto_a`), no por
  `TERMINACIONES`.
- Un objeto de id del marcador (el mismo `id` de la línea de `.marcas`, en
  texto) a `"x y ancho alto"`:

```json
{ "1792000000000": "320.5 1040 800 450" }
```

  - `x`, `y`: el **centro** de lo que se veía, en coordenadas del mundo del
    lienzo (las de los elementos Excalidraw, y hacia abajo).
  - `ancho`, `alto`: cuánto mundo se veía (> 0), en las mismas unidades.
  - Números separados por espacios, con punto decimal; el PC los escribe a
    diezmilésimas y sin `.0` si son enteros.
- **Al volver** en un hueco de `w`×`h` píxeles (los de la cámara, sin la
  escala del sistema): `zoom = min(w/ancho, h/alto)` acotado a los topes
  del zoom de quien mira (en el editor del PC los de Excalidraw, `[0.1, 30]`:
  por debajo del mínimo de la rueda, la primera muesca de alejar acercaría),
  y la esquina de arriba a la izquierda en
  `(x − w/(2·zoom), y − h/(2·zoom))`. En la misma ventana da la cámara
  exacta de antes; en otra (el móvil de pie), todo lo guardado, entero y
  centrado. En Excalidraw: `scrollX = w/(2·zoom) − x`,
  `scrollY = h/(2·zoom) − y`.
- **Al guardar** desde una cámara (esquina `cx, cy`, `zoom`) en un hueco de
  `w`×`h`: `ancho = w/zoom`, `alto = h/zoom`, `x = cx + ancho/2`,
  `y = cy + alto/2`.
- Un marcador **sin** vista (los de antes, o un id que no está) vuelve como
  siempre: centrado a lo ancho, arriba, con el aumento de ahora. Una entrada
  que no se entiende se ignora.
- Quitar un marcador quita su entrada; sin ninguna se escribe `{}` (no se
  borra). Lo resuelve la sincronización entero, gana el más nuevo. **Se
  sabe y se acepta**: si dos aparatos cambian vistas del mismo lienzo entre
  dos sincronizaciones (el PC fija la de 🏠 y el móvil la de ⭐), queda solo
  el fichero del más nuevo y la otra vista vuelve a la de antes (la marca no
  se pierde: va en `.marcas`). Mezclar por id pediría saber cuándo se quitó
  cada una, y no compensa para algo que se vuelve a fijar con un Mayús+clic.
- En el PC: **Mayús+clic** en el marcador o en su punto del riel (o
  **Ctrl+Mayús+1…9**) cambia su vista por la de ahora. El viaje hasta la
  vista acerca o aleja por múltiplos persiguiendo el centro.
- Fuera de un chat (un `.excalidraw` suelto), la misma vista en
  `<dibujo>.pixpin-vistas` al lado del fichero, que no viaja.
- Lógica: `VistaDeMarca` en `crates/pindesk-motor2d/src/marcas.rs`; ficheros
  y teclas en `apps/pindesk/src/ventana_editor/marcas.rs`.

### 3.9 Marcas sobre el texto de un PDF, Word o PowerPoint (1-nov-2026)

Leyendo (no con el lápiz), lo elegido con el ratón se puede **resaltar,
subrayar con raya ondulada, subrayar, tachar o tapar**: la misma herramienta
que el texto reconocido de los pines. **No hay fichero nuevo**: cada marca es
dibujo normal de la capa de su hoja (`<doc>.pixpin-anotado/hoja-<n>.excalidraw`
junto al fichero, o `anot-<uid>-p<n>.excalidraw.gz` + `.hoja` de un adjunto),
en las mismas unidades y con el mismo marco que la tinta (§3.4–3.5), así que
viaja, se exporta y se imprime como cualquier trazo. Android, si no hace nada,
ya las ve y las puede borrar como dibujo.

- **Grupo**: todos los elementos de una marca (uno por renglón, y en todas
  las hojas por las que pasa) llevan en `groupIds` el mismo
  `marca-texto-<ms>` (milisegundos UTC de cuando se hizo). Lo que empieza por
  `marca-texto` es una marca de texto; el resto es tinta a mano.
- **Formas** (caja del renglón `x, y, w, h` en unidades de la hoja):
  - Resaltar: `rectangle` en `x − 0,12h, y − 0,12h`, de `w + 0,24h` por
    `h + 0,24h`, relleno sólido del color, sin borde (`strokeColor`
    transparente), `opacity` 40.
  - Tapar: lo mismo con `opacity` 100 (no deja leer; al exportar a PDF la
    hoja va como imagen, así que lo tapado no queda en el texto).
  - Subrayar: `line` de dos puntos a `y + h + g`, con `g = max(0,09h, 1,5)`
    de grosor.
  - Tachar: `line` de dos puntos a `y + 0,55h`, grosor `g`.
  - Ondulada: `line` de muchos puntos (una ola cada cuatro pasos de
    `max(0,09h, 1,5)`, amplitud igual al paso) a `y + h + g`, grosor `0,8g`.
  - `roughness` 0 en todas.
- **Colores** (los de la barra del pin): `#F47286`, `#B99CF5`, `#5BA4F5`,
  `#3FC88C`, `#F5C218` (el amarillo de serie).
- **El lector de Word, libros y páginas** (el que reajusta el texto: Word en
  modo lectura, EPUB, Markdown, HTML, texto): lo mismo, en su capa
  (`capa.excalidraw` junto al fichero o `anot-<uid>.excalidraw.gz`, §3.6), en
  las unidades del documento. La primera marca **fija la columna y la letra**
  como la primera vez que se anota, así que el texto ya no se vuelve a partir
  y la marca queda sobre su palabra; con la maqueta del móvil cae en la misma
  palabra allí.
- **Hojas escaneadas**: el texto lo pone el OCR del aparato la primera vez que
  se pasa el ratón (o con **T**); no se guarda ni viaja, solo las marcas.
- **Código**: `crates/pindesk-motor2d/src/marcas_texto.rs` (formas, colores,
  grupo), `crates/pindesk-ui/src/barra_marcar.rs` (la barrita),
  `apps/pindesk/src/lector_pdf/marcar.rs` (el lector de PDF y Office),
  `apps/pindesk/src/visor/marcar.rs` (el de Word, libros y páginas).

- **Hojas de cálculo** (2-nov-2026): leyendo, con dos celdas o más elegidas
  con el ratón sale la misma barrita y **se marca la celda entera** (no un
  trozo de su texto: casi siempre es una cifra, y el texto se recorta o se
  desborda según el ancho de la columna, que cambia de un aparato a otro).
  Resaltar y tapar: **un** rectángulo que cubre el rango; subrayar, tachar y
  ondulada: una raya **por fila**, de lado a lado del rango. Van en la capa
  de la hoja (`anot-<uid>-h<i>`, §3.5) en sus unidades (la rejilla de
  `rejilla_de_la_capa`), con el mismo grupo `marca-texto-<ms>` y las mismas
  formas. Código: `apps/pindesk/src/visor_hoja/marcar.rs`.

### 3.10 Marcas de texto de las notas y las páginas web — `anot-<uid>.marcas-texto.json` (2-nov-2026)

En una nota Markdown (el lector de notas) y en una página web (el visor de
HTML) el texto **se recoloca** con el ancho de la ventana, así que una caja no
sirve: cada marca guarda **su cita**, como los selectores de la W3C (*Web
Annotation Data Model*: `TextQuoteSelector` y `TextPositionSelector`), y al
abrir se vuelve a buscar. Es un **fichero nuevo**, uno por documento:

- De un adjunto del chat o de una nota (mensaje `NOTA` u hoja `nota`):
  `pins/draw/anot-<uid>.marcas-texto.json` — viaja y se borra con su mensaje
  (terminación `MARCAS_TEXTO`). Vacío se escribe `{"marcas":[]}`, no se borra.
- Un fichero suelto: `<fichero>.pixpin-marcas-texto.json` a su lado (no
  viaja; sin marcas se quita). Al abrirlo como adjunto pasa una vez al
  mensaje (`del_mensaje_o_junto`).

```json
{
  "version": 1,
  "sobre": "markdown",
  "marcas": [
    {
      "id": "marca-texto-1793582613265",
      "marca": "ondulada",
      "color": "#5BA4F5",
      "cuando": 1793582613265,
      "cita": {"type": "TextQuoteSelector", "exact": "el curado empieza",
               "prefix": "… ya está **hormigonada** y ", "suffix": " mañana.\n\nOtra…"},
      "posicion": {"type": "TextPositionSelector", "start": 44, "end": 61}
    }
  ]
}
```

- `sobre`: en qué texto se cuentan las posiciones. `markdown`: el `.md` tal
  cual (las mismas posiciones que los comentarios, `.comentarios.json`).
  `html`: los nodos de texto de `<body>` en orden, sin `<script>`, `<style>`,
  `<noscript>`, `<template>`, `<textarea>`, `<select>`/`<option>` ni lo que
  pone PinDesk (`#pixpin-riel-capa`, `#pixpin-marcas`). Es el
  `textContent` de un `TreeWalker` de texto: cualquier `WebView` lo cuenta
  igual.
- Posiciones en **unidades UTF-16** (las de JavaScript y Kotlin). `prefix` y
  `suffix`: hasta 32 de cada lado (sin partir un par sustituto). La cita va
  sin blancos en los bordes.
- `marca`: `resaltar`, `ondulada`, `subrayar`, `tachar` o `tapar`; `color`:
  uno de los cinco de §3.9. Lo que no se conozca se conserva al reescribir.
- **Volver a encontrarla** (en este orden): (1) la cita tal cual en su
  `start` con el mismo contexto; (2) la aparición de la cita con más contexto
  igual y, si empatan, la más cercana a `start`; (3) lo que haya entre su
  `prefix` y su `suffix` (también con solo 16 u 8 letras pegadas a la cita;
  no más del doble de la cita + 64); (4) la **parecida**: el trozo con menos
  cambios (como mucho uno por cada cinco letras; citas de menos de 5 letras
  no) buscado alrededor de los trozos de la cita que siguen tal cual. Si
  nada, la marca queda **sin sitio**: no se pinta, no se borra (vuelve si el
  texto vuelve). Las cuentas, con pruebas: `herramientas/lector-md/src/marcas-texto.mjs`.
- **Pintarlas**: resaltar = fondo del color al 40 % (`#RRGGBB66`); tapar =
  fondo y letra del color (no deja leer); subrayar y tachar = raya de 2 px;
  ondulada = raya ondulada de 1,5 px. En una página web el PC usa
  `CSS.highlights` (no toca el DOM); en la nota, decoraciones de CodeMirror.
- **Juntar** dos versiones (la página abierta y lo que trajo la
  sincronización): por `id`, lo nuevo de cada lado entra, lo quitado en un
  lado se va si el otro no lo cambió; si los dos cambiaron la misma, gana la
  de quien guarda (`pindesk_docs::ancla_texto::fusionar`). Entre aparatos el
  fichero entero gana el más nuevo, como los demás JSON (§1).
- **La interfaz**: los mismos botones que la barrita del PDF. En la nota van
  en la misma burbuja que «Comentar», solo leyendo; un clic en una marca la
  elige (otra marca la cambia, un color la pinta, el círculo la quita);
  Ctrl+Z / Ctrl+Y dentro de la página.
- **Código**: `crates/pindesk-docs/src/ancla_texto.rs` (fichero y fusión),
  `apps/pindesk/src/marcas_web.rs` (dónde va, leer y guardar),
  `herramientas/lector-md/src/marcas-texto.mjs` (anclar y buscar),
  `marcas-nota.js` (la nota), `marcas-pagina.js` (cualquier página;
  construido en `apps/pindesk/recursos/marcas-pagina.js`), `barra-marcas.js`.

**Dónde no hay marcas de texto** (2-nov-2026), y por qué:

- **El pin de Office** (Visio, correos `.msg`, un Word/PowerPoint que no se
  deja convertir): lo pinta el previsualizador de Office en su propia
  ventana; no da el texto ni sus posiciones (ni su desplazamiento: allí
  tampoco hay marcadores). Lo que se convierte (`.doc`, `.ppt`, RTF,
  OpenDocument) ya se lee en el visor propio, con marcas.
- **Los mensajes del chat y la línea de tiempo**: son conversación y
  rótulos cortos, no un documento que se lee; una marca ahí sería de todos
  los del chat. Para marcar lo de un mensaje se abre como nota.
- **La transcripción de una clase grabada** (`estudiar_clase`): ya resalta
  lo que suena y lleva sus marcadores por minuto; marcar su texto queda
  para más adelante (su texto se rehace al transcribir otra vez).

## 4. Lo que NO viaja (a propósito)

- Lo de un documento que **no es un adjunto** de un chat: queda al lado del
  fichero (`<doc>.pixpin-marcos`, `<doc>.pixpin-vistas`,
  `<doc>.pixpin-anotado/…`) o en `<raíz>/marcadores/<hash>/` para planos y
  modelos. Al abrirlo como adjunto se pasa una vez a `anot-<uid>…`.
- Los **pines del escritorio** (las imágenes flotantes): no son de ningún
  chat. Si la imagen se manda a un chat, sus marcos viajan con el mensaje.
- Las notas nuevas sin guardar.
- El pin de la vista previa de Office (no dice su scroll), que ya solo queda
  para Visio, los correos `.msg` y un Word o PowerPoint que no se deja
  convertir. Un `.doc`/`.ppt`/RTF/OpenDocument se convierte una vez con
  Office a `.docx`/`.pptx` (`office_convertido.rs`, copia en
  `<raíz>/cache/office/`) y se lee en el visor propio **en nombre del
  original** (`pindesk_proyecto::anotado::suplir`): lo anotado va a los
  `anot-<uid>…` del adjunto original y viaja como lo de un `.docx`. Android:
  si abre esos formatos por su cuenta, que use el mismo `uid` y las mismas
  terminaciones que para el `.docx`.

## 4 bis. El `.pindesk` exportado lleva todo

- De un chat: la carpeta entera, con `lienzos/anot-…` y
  `android/pins/draw/anot-…`. «Crear como nuevo» da un código nuevo a cada
  mensaje que tiene algo anotado y renombra sus `anot-<uid>…` con él.
- De un fichero suelto: un proyecto nuevo con el fichero como mensaje (con
  `uid`), lo que cuelga de su nombre (`archivos/<nombre>.pixpin-*`, que el
  visor pasa a `anot-<uid>…` al abrirlo) y lo de un plano o un modelo 3D ya
  como `anot-<uid>…`.
- Lo editable de un lienzo es el `.pindesk` (el `.excalidraw` suelto ya no se
  ofrece al compartir); los `.excalidraw` se siguen abriendo e importando.

## 5. La interfaz igual en todos (para reproducirla en otro dispositivo)

- **Riel** a la derecha, arriba: marcador (`BOOKMARK_ADD`), lápiz (`EDIT`),
  marco (`CROP_FREE`), botones de 36 px; pulsado = azul `#0060DF` con el
  icono blanco. Al dejar el ratón encima, un globo con nombre y tecla
  («Marcador (Ctrl+M)», «Anotar (A)», «Marcos para imprimir (I)»).
- **Barra** de arriba: tipo y nombre, botones propios, luna/sol, pantalla
  completa (F11), fijar, cerrar (Esc); menú **⋮** vertical (G) sin repetir lo
  que ya tiene botón. Editar el texto usa `EDIT_NOTE`; el lápiz es solo
  anotar.
- **Marcos**: raya azul `#0078D4` de 2 px, número blanco en un cuadrito azul
  en la esquina de arriba a la izquierda; fichas de 28 px abajo a la
  izquierda con la pista a su derecha.
- **Teclas**: I marcos · Supr quita el último · Ctrl+P imprime · Ctrl+M
  marcador · A anotar · G menú ⋮ · Esc sale del modo antes de cerrar. En
  planos: D acotar; en 3D: E aristas.
- **Marcar texto** leyendo un PDF, Word o PowerPoint: al elegir texto sale
  encima una barrita con copiar, los cinco colores, resaltar, ondulada,
  subrayar, tachar, tapar y el círculo tachado de quitar (los dibujitos de la
  barra del pin). Tocar una marca la elige entera para cambiarla o quitarla;
  Ctrl+Z la deshace entera. **T** reconoce el texto de una hoja escaneada.
- **Panel táctil** en planos y modelos: dos dedos mueven; pellizco = zoom.
- **Audio**: lo grabado bajito se reproduce realzado (pico al −1 dBFS, hasta
  ×16, limitador suave), `crates/pindesk-audio/src/volumen.rs`.

## 6. Dónde está el código (PC)

| Pieza | Fichero |
|---|---|
| Lista blanca | `crates/pindesk-sincro/src/anotado.rs` (`MARCOS`, `VISTAS`, `MARCAS_TEXTO`) |
| Marcas de texto de notas y páginas | `crates/pindesk-docs/src/ancla_texto.rs`, `apps/pindesk/src/marcas_web.rs`, `herramientas/lector-md/src/marcas-*.js` |
| Marcos y vistas de documentos | `apps/pindesk/src/marcos_doc.rs` |
| A qué fichero va cada cosa | `apps/pindesk/src/anotado_del_adjunto.rs` (`del_mensaje_o_junto`, `tinta_de_la_hoja_del_libro`, `tinta_de_la_pagina`) |
| Planos y modelos | `crates/pindesk-cad/src/marcadores.rs`, `apps/pindesk/src/plano_cad.rs` (`SitioDeMarcadores`) |
| Imprimir en grupo | `apps/pindesk/src/marco_imprimir.rs` (`imprimir_fotos`) |
| Prueba de dos PC | `crates/pindesk-proyecto/tests/marcos_vistas_y_tinta_entre_dos_pc.rs` |

## 7. Para Android, en orden

1. Añadir `.marcos.json` y `.vistas.json` a `TERMINACIONES`.
2. Leer y escribir esos ficheros con los formatos de §3, escribiendo `[]`/`{}`
   en vez de borrar y de forma atómica.
3. Excel: la tinta en `anot-<uid>-h<i>.excalidraw.gz`; web y notas en
   `anot-<uid>.excalidraw.gz` sin regla de columnas.
4. Marcadores con vista: al ponerlo, guardar el zoom y el sitio en
   `.vistas.json`; al ir, restaurarlos.
5. Lienzos: añadir `pins/draw/<d>.vistas.json` al alcance de cada dibujo,
   junto a `<d>.marcas` (§3.8), y guardar/restaurar la vista de cada marca.
6. La interfaz y las teclas de §5.
7. Marcas de texto (§3.9): opcional; reconocer el grupo `marca-texto-*` para
   ofrecer la misma barrita al elegir texto en el lector de PDF.
8. Marcas de texto de notas y páginas web (§3.10): añadir
   `.marcas-texto.json` a `TERMINACIONES`; en el `WebView` del lector de
   notas y del visor de HTML, leer el fichero, buscar cada cita en el texto
   (el `.md` o los nodos de texto de `<body>`, según `sobre`) con el orden de
   §3.10 y pintarla; al marcar, escribir la lista entera (atómico, `[]` en
   vez de borrar). Se puede usar tal cual `marcas-texto.mjs` (sin
   dependencias). Excel: nada nuevo, son trazos de la capa de la hoja.
