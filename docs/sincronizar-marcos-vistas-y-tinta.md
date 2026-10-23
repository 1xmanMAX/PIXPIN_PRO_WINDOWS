# Marcos, marcadores con vista y tinta: qué viaja y cómo (20-oct-2026)

Manual para seguir desarrollando en otros dispositivos (otro PC, el móvil
Android) y que lo hecho aquí se sincronice con ellos. Cubre lo añadido entre
el 19 y el 20 de octubre de 2026 en PixPin Max (PC):

- los **marcos numerados** para imprimir (como los del plano: I, arrastrar,
  Supr, Ctrl+P en grupo) en todos los visores;
- los **marcadores con vista** (sitio **y zoom**; en un modelo 3D, toda la
  cámara y el estado);
- la **tinta** de Excel, de las páginas web y de las notas;
- los **marcadores de los planos (DWG) y de los modelos (BIM)**, con la tinta
  de cada marcador 3D;
- la interfaz común (riel lateral, fichas de los marcos, globos con atajo,
  teclas), que no viaja como datos pero tiene que verse igual en todos.

## 1. Cómo sincroniza PixPin (recordatorio)

- Solo **LAN**: mDNS `_pixpin._tcp.` en el puerto 47474, canal TCP cifrado
  AES-256-GCM con el código del grupo (que nunca viaja). Mensajes JSON al
  estilo kotlinx. `VERSION = 4` (estricta).
- La unidad es **un chat** (el general y uno por proyecto). Viajan sus
  mensajes, el `proyecto.json` y **los ficheros de su alcance**.
- Lo que acompaña a un adjunto viaja **solo** si se llama
  `pins/draw/anot-<uid><terminación>` (carpeta plana, sin subcarpetas) y la
  terminación está en la lista blanca `TERMINACIONES`
  (`crates/pixpin-sincro/src/anotado.rs`). Se borra con su mensaje.
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

**Android tiene que añadir las mismas dos a su `TERMINACIONES`** (Kotlin).
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
    (`Modelo3d::clave_de_elemento` / `clave_de_nivel` en `pixpin-cad`):
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
  original** (`pixpin_proyecto::anotado::suplir`): lo anotado va a los
  `anot-<uid>…` del adjunto original y viaja como lo de un `.docx`. Android:
  si abre esos formatos por su cuenta, que use el mismo `uid` y las mismas
  terminaciones que para el `.docx`.

## 4 bis. El `.pixpin` exportado lleva todo

- De un chat: la carpeta entera, con `lienzos/anot-…` y
  `android/pins/draw/anot-…`. «Crear como nuevo» da un código nuevo a cada
  mensaje que tiene algo anotado y renombra sus `anot-<uid>…` con él.
- De un fichero suelto: un proyecto nuevo con el fichero como mensaje (con
  `uid`), lo que cuelga de su nombre (`archivos/<nombre>.pixpin-*`, que el
  visor pasa a `anot-<uid>…` al abrirlo) y lo de un plano o un modelo 3D ya
  como `anot-<uid>…`.
- Lo editable de un lienzo es el `.pixpin` (el `.excalidraw` suelto ya no se
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
- **Panel táctil** en planos y modelos: dos dedos mueven; pellizco = zoom.
- **Audio**: lo grabado bajito se reproduce realzado (pico al −1 dBFS, hasta
  ×16, limitador suave), `crates/pixpin-audio/src/volumen.rs`.

## 6. Dónde está el código (PC)

| Pieza | Fichero |
|---|---|
| Lista blanca | `crates/pixpin-sincro/src/anotado.rs` (`MARCOS`, `VISTAS`) |
| Marcos y vistas de documentos | `apps/pixpin/src/marcos_doc.rs` |
| A qué fichero va cada cosa | `apps/pixpin/src/anotado_del_adjunto.rs` (`del_mensaje_o_junto`, `tinta_de_la_hoja_del_libro`, `tinta_de_la_pagina`) |
| Planos y modelos | `crates/pixpin-cad/src/marcadores.rs`, `apps/pixpin/src/plano_cad.rs` (`SitioDeMarcadores`) |
| Imprimir en grupo | `apps/pixpin/src/marco_imprimir.rs` (`imprimir_fotos`) |
| Prueba de dos PC | `crates/pixpin-proyecto/tests/marcos_vistas_y_tinta_entre_dos_pc.rs` |

## 7. Para Android, en orden

1. Añadir `.marcos.json` y `.vistas.json` a `TERMINACIONES`.
2. Leer y escribir esos ficheros con los formatos de §3, escribiendo `[]`/`{}`
   en vez de borrar y de forma atómica.
3. Excel: la tinta en `anot-<uid>-h<i>.excalidraw.gz`; web y notas en
   `anot-<uid>.excalidraw.gz` sin regla de columnas.
4. Marcadores con vista: al ponerlo, guardar el zoom y el sitio en
   `.vistas.json`; al ir, restaurarlos.
5. La interfaz y las teclas de §5.
