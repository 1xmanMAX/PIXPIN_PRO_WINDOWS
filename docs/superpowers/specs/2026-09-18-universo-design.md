# El Universo: los archivos del chat en un lienzo infinito

**Fecha:** 2026-09-18
**Estado:** diseño escrito a partir de la conversación con el usuario; pendiente de su revisión
**Rama:** `universo` (sale de la rama en curso cuando `pin-en-vivo` quede commiteada)
**Decisiones:** D200–D259 (se salta el hueco para no chocar con numeraciones de otras ramas)

---

## 1. Qué es y por qué

El usuario quiere un **visor de archivos mejorado**: un *file manager* que, en vez de carpetas y listas, pone
los archivos en un **lienzo infinito con lógica de universo**, donde se pueden agrupar, anotar, conectar con
líneas y abrir directamente.

Lo que dijo, en orden:

1. Vincular archivos a elementos que él crea (emojis, imágenes, figuras, composiciones agrupadas con
   información) y conectarlos con **vínculos visibles de varios tipos**.
2. Lógica de universo: **galaxias = proyectos**, **planetas = planes**, **exoplanetas** y más.
3. **Ctrl + clic** abre directamente el documento o la carpeta.
4. Alta fluidez en **portátiles con poca RAM y CPU pobre**.
5. Trackpad: mover en **cualquier dirección**, no solo arriba y abajo.
6. Zoom semántico en un solo lienzo (Q2 → A, con el doble clic para enfocar de la opción C).
7. Pertenencia **por posición** (Q3 → A).
8. Todo en **un solo fichero** (Q4 → A).
9. Última corrección, que manda sobre las anteriores: **solo se vincula con los archivos del chat de
   PixPin**. No hay rutas sueltas del disco: el universo es otra forma de ver y organizar lo que ya está en
   los proyectos del chat.

Por qué encaja en PixPin Max sin coste grande (verificado en el código):

| Ya existe | Dónde |
|---|---|
| Lienzo infinito con cámara, zoom 0,05–30 y enfoque | `crates/pixpin-motor2d/src/camara.rs:41` (`desplazar :109`, `acercar_en :123`, `encajar :141`) |
| Navegación pura: rueda, Mayús+rueda, Ctrl+rueda, espacio, botón central, desplazamiento lateral del panel táctil | `apps/pixpin/src/navegacion.rs` |
| Recorte a lo visible y rejilla espacial | `camara::recortar :190`, `pintado.rs:563`, `indice.rs:66` (`Rejilla`, D27) |
| Capa congelada durante el arrastre, repintado por zonas, 0 % de CPU en reposo | `pixpin-render/src/capa_estatica.rs`, `ventana_editor.rs:570-580` |
| Niveles de equipo Completo/Ligero | `crates/pixpin-nivel` |
| Pertenencia por posición (el marco arrastra lo que contiene) | `pixpin-motor2d/src/marco.rs:20,38`; `gesto.rs:1045` |
| Mensajes del chat con su fichero, su clase y sus tres códigos | `pixpin-proyecto/src/cuaderno.rs` (`Mensaje`, `codigo_unico`, `codigo_chat`) |
| Ruta real del fichero de un mensaje | `ventana_chat.rs:3218` (`ruta_del_mensaje`) |
| Abrir con Windows (fichero o carpeta) y mostrar en el Explorador | `pixpin-shell/src/abrir.rs:20,41` |
| Miniaturas reducidas y cargadas poco a poco | `apps/pixpin/src/miniaturas.rs` (164 px, 4 por fotograma) |
| Ficha de archivo con color por extensión | `ventana_chat.rs:5339,5411` |
| Meter ficheros en un proyecto | `ventana_chat.rs:3484` (`meter_ficheros`), `cuaderno::adjunto` |
| Todas las herramientas de dibujo | `pixpin-motor2d` (`Figura`, `gesto.rs`) |

Lo que **no** existe y construye este diseño: jerarquía anidada, conexiones atadas a elementos, zoom
semántico, elementos emoji, abrir con Ctrl+clic desde el lienzo y el propio documento del universo.

---

## 2. El modelo: qué es cada cosa

### 2.1 Los astros

| Astro | Qué representa | Quién lo crea | Puede contener |
|---|---|---|---|
| **Cosmos** | El universo entero: todo el fondo del lienzo | Existe siempre, uno solo | Galaxias, exoplanetas, anotaciones |
| **Galaxia** | **Un proyecto del chat.** La conversación general es la galaxia central, «Vía Láctea» | Automática: una por proyecto de `proyectos/indice.json` | Planetas, lunas de su proyecto, su nebulosa, anotaciones |
| **Planeta** | Un plan, un tema o una carpeta mental. Lo crea el usuario con nombre, emoji y color | El usuario | Lunas, anotaciones |
| **Exoplaneta** | Un planeta **fuera de toda galaxia**. Reúne archivos de proyectos distintos | El usuario (es un planeta soltado en el cosmos) | Lunas de **cualquier** proyecto, anotaciones |
| **Luna** | **Un archivo del chat**: un mensaje con fichero, o una nota o una mini-app | Automática: una por mensaje colocable | Nada |
| **Nebulosa** | Lo que aún no se ha colocado de una galaxia: una rejilla ordenada en su borde | Automática | Lunas sin colocar |

**D200 — Qué mensajes son lunas.** Las clases `Imagen`, `Archivo`, `Voz`, `Dibujo`, `Pagina`, `MiniApp`
y `Proyecto`. Las `Nota` del chat **no** entran por defecto porque llenarían el cielo de charla. Cada
galaxia tiene el interruptor «Incluir notas del chat» (apagado). `Otra(_)` entra como archivo genérico.
Los mensajes del buzón (`en_buzon`) no entran, porque caducan a los siete días.

**D201 — Una luna está en un solo sitio.** Un archivo aparece una vez en todo el universo. Para
relacionarlo con otro lugar se usa una **conexión**. Dos copias del mismo archivo en dos sitios harían dudar
de cuál es la buena y obligarían a sincronizarlas.

**D202 — Dónde puede ir una luna.**
- En su propia galaxia: suelta, dentro de un planeta de esa galaxia o en su nebulosa.
- En un exoplaneta.

Soltarla dentro de **otra** galaxia no se permite: vuelve a su sitio con un destello rojo de 300 ms. Un
archivo del proyecto A dentro de la galaxia B mentiría sobre dónde vive.

**D203 — Planeta dentro de planeta, no.** La jerarquía es fija: Cosmos › Galaxia › Planeta › Luna. Un
planeta soltado dentro de otro se queda **encima** sin ser su hijo, y se avisa con el mismo destello.
Una profundidad fija permite saber qué pintar en cada nivel de zoom sin recorrer árboles.

### 2.2 Las anotaciones

Son los elementos del motor 2D que ya existen: texto, lápiz, rectángulo, elipse, flecha libre, marco,
etc. A ellos se suman **dos piezas nuevas**:

| Pieza nueva | Qué es |
|---|---|
| **Emoji** | Un emoji o icono suelto como elemento, de tamaño libre. Nueva variante `Figura::Emoji { caracter }`. Se pinta con DirectWrite en color (`D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT`) |
| **Nota de astro** | Texto largo pegado a un astro. Se edita en el inspector (§4.4) y se ve bajo el astro al acercarse |

**D204 — Las anotaciones pertenecen por posición, calculada al vuelo.** Igual que el marco
(`marco::contenidos`): cuando empieza a arrastrarse un astro, sus anotaciones son las que caben enteras
dentro de su círculo. No se guarda el padre de una anotación. Es la convención que ya eligió el proyecto, y
así borrar o mover una anotación no deja referencias colgando.

**D205 — Los astros sí guardan su padre** (`padre: Option<IdAstro>`). Se recalcula solo al **soltar**:
el padre es el contenedor válido más pequeño cuyo círculo contiene el centro del astro. El zoom semántico
necesita saber «qué hay dentro de esta galaxia» en cada fotograma sin buscar geometría.

### 2.3 Las conexiones

Líneas **atadas** a dos astros. Siguen a sus extremos cuando se mueven y salen del borde del círculo, no del
centro.

| Tipo | Aspecto | Significado |
|---|---|---|
| **Relación** | Línea continua fina, gris | «Tiene que ver con» |
| **Depende** | Flecha continua, color de acento | «B necesita a A» |
| **Referencia** | Línea de puntos | «Mira también» |
| **Secuencia** | Flecha discontinua con número en medio | «Primero esto, luego aquello» |

Cada conexión tiene además un **rótulo** opcional (texto corto en el centro) y un **color** opcional.

**D206 — Se atan astros, no anotaciones.** Una conexión une dos astros (galaxia, planeta, exoplaneta o
luna). Para unir anotaciones sigue estando la flecha libre del motor 2D. Atar flechas a figuras libres
obligaría a cambiar el motor entero (`Linea`/`Flecha` no tienen enlaces, §3 del mapa) y el usuario pidió
conectar **archivos y lo que los agrupa**.

**D207 — Conexión entre galaxias.** Se permite y se pinta siempre. A zoom lejano une los discos de las
galaxias; así se ve la relación entre proyectos desde arriba.

**D208 — Una conexión con un extremo que ya no existe se borra al cargar.** Queda una línea en el registro.
Una línea que no va a ninguna parte no aporta nada.

---

## 3. Cómo se entra

| # | Entrada | Qué hace |
|---|---|---|
| D210 | **Botón «Universo»** en la cabecera de la lista de proyectos del chat, a la izquierda del buscador. Icono de órbita (anillo con punto) | Abre el universo encuadrando **todo el cosmos** |
| D211 | **Botón «Ver en el universo»** en la cabecera del proyecto abierto, junto a su nombre | Abre el universo **enfocando esa galaxia** |
| D212 | **«Mostrar en el universo»** en el menú del clic derecho de una foto (`menu_de_foto`) y de una ficha de archivo | Abre el universo, enfoca la galaxia y **resalta esa luna** con un pulso de 600 ms. Si estaba en la nebulosa, la nebulosa se abre en esa página |
| D213 | Entrada **«Universo»** en el menú de la bandeja, bajo «Chat» | Igual que D210 |
| D214 | **Ctrl+U** con la ventana de chat enfocada | Igual que D211 si hay proyecto abierto, si no igual que D210 |

**D215 — La ventana.** Es el **editor de lienzo existente** (`ventana_editor`) en un **modo nuevo,
`Modo::Universo`**. Gana así, sin reescribirlos, las herramientas, deshacer, pantalla completa (F11), la
escala de Windows (D127), la navegación (D136) y la medición (D129). El universo vive en **su propio hilo**,
como el chat, así que el chat sigue abierto y usable detrás. Si el universo ya está abierto, lo que el chat
pida sobre él (D211–D212) se deja en un pedido compartido y despierta su ventana (`MSG_DESPIERTA`): **mueve la
cámara del universo ya abierto**, sin abrir otro. El universo deja alejarse más que un dibujo (zoom mínimo
0,002 en vez de 0,05) para ver todas las galaxias a la vez.

**D216 — Salir.** `Esc` sin selección, o el botón cerrar. Guarda (D240) y cierra. El chat no cambia.

---

## 4. Cómo se ve

### 4.1 Pantalla completa

```
┌─ PixPin · Universo ─────────────────────────────────────────── ─ □ ✕ ┐
│ ⌂ Cosmos › 🌀 Casa nueva › 🪐 Planos                     🔍 Buscar… (Ctrl+F) │  ← barra de ruta (D220)
├──┬──────────────────────────────────────────────────────┬────────────┤
│▣ │  ·    ✦        ·           ·        ✦     ·         │ 🪐 Planos   │
│✥ │        .-~~~~~~~~-.                                 │ Planeta     │
│◯ │     .~   🪐 Planos  ~.     ·   ┄┄┄ Referencia ┄┄┄→   │ 12 lunas    │
│😀│    (  ○plano.pdf       )                     .-~~-.  │─────────────│
│📝│    (   ○cocina.jpg  ○  )━━━ Depende ━━━━━━▶ ( 🪐 ) │ Nota        │
│🔗│     `~  ○ ○ medidas ~´                       `-~~-´  │ «Revisar    │
│──│        `-~~~~~~~~-´      ·                          │  con el     │
│✏ │   ░░ Nebulosa ░░  ▦▦▦▦▦▦ +38 más                     │  arquitecto»│
│▭ │                                                      │─────────────│
│◯ │         ·          ✦            ·                    │ Conexiones  │
│T │                                                      │ → Presupuesto│
│  │                                                      │─────────────│
│  │                                        ┌──────────┐  │ [Abrir]     │
│  │                                        │ minimapa │  │ [Ir al chat]│
│  │                                        └──────────┘  │             │
└──┴──────────────────────────────────────────────────────┴────────────┘
  ↑ caja de herramientas (D221)                          ↑ inspector (D223)
```

### 4.2 Paleta

**D217 — El universo es oscuro siempre**, sea cual sea el tema de Windows. Es la metáfora (espacio) y además
ahorra batería en pantallas OLED. Los tokens:

| Token | Valor | Uso |
|---|---|---|
| `espacio` | `#0B1020` | Fondo |
| `estrella` | `#FFFFFF` a 20–70 % | Estrellas del fondo |
| `texto` | `#E8ECF5` | Nombres |
| `texto_suave` | `#8A93A8` | Detalles, códigos, fechas |
| `acento` | `#40A7E3` | Selección, «Depende», el azul del chat |
| `peligro` | `#E5484D` | Destello de «aquí no se puede» |
| `panel` | `#141A2E` a 92 % | Inspector, barra de ruta, caja de herramientas |
| `borde_panel` | `#FFFFFF` a 8 % | Separadores |

El color de cada galaxia es el del avatar de su proyecto (`color_avatar`, `ventana_chat.rs:1738`): así el
usuario reconoce el proyecto por el mismo color que ve en la lista del chat.

### 4.3 Qué se pinta según lo cerca que se esté (zoom semántico)

**D218 — El nivel no depende del zoom sino del tamaño en pantalla de cada astro** (su radio aparente, en
píxeles lógicos). Así una galaxia grande y una pequeña al mismo zoom se ven cada una como les corresponde.

Cada tipo mira **su propio** radio aparente:

| Astro | Radio aparente → qué se pinta |
|---|---|
| **Galaxia** | < 6 px: **punto** de luz de su color · 6–120 px: **disco** con brillo radial (2 círculos suaves) + nombre + pastilla con el nº de lunas; **su contenido no se carga** · ≥ 120 px: **abierta**, con borde tenue (anillo al 15 %) y su nombre arriba; dentro se ven sus hijos |
| **Planeta / exoplaneta** | < 6 px: no se pinta · 6–40 px: **disco** liso con su color, sin lunas · ≥ 40 px: **icono**: círculo + emoji centrado + nombre debajo + anillo de órbita; dentro se ven sus lunas |
| **Luna** | < 4 px: no se pinta · 4–16 px: **punto** · 16–48 px: **icono** de su clase · 48–120 px: **ficha** (icono + nombre + tamaño) · ≥ 120 px: **vista previa** (miniatura de foto, primeras líneas de nota o mini-app, página de PDF) + código de chat (`47·K7Q2`) |

Los hijos solo se pintan si su padre está «abierto»: la galaxia a ≥ 120 px y el planeta a ≥ 40 px.

La **histéresis** es del 15 %: se sube de nivel al pasar el umbral y se baja al quedar un 15 % por debajo.
Así un zoom que roza un umbral no hace parpadear ni recargar.

**D219 — Aspecto de cada luna por clase:**

| Clase | Icono | Vista previa (≥ 120 px) |
|---|---|---|
| Imagen | Círculo recortado con la miniatura | Miniatura cuadrada 164 px (`miniaturas.rs`) |
| Archivo | Pastilla con la extensión, en el color de `color_de_extension` | La ficha de archivo del chat (`ficha_de_archivo`) |
| Voz | Onda | Duración + primera línea de la transcripción |
| Dibujo | Lápiz | La ojeada del lienzo que ya pinta el chat (`leer_vista`, `pintar_lienzo`) |
| Pagina | Hoja con esquina doblada | Página del PDF ya renderizada si existe; si no, el icono grande |
| MiniApp | Cuadrícula | Las primeras filas (`pintar_ojeada_tabla`) |
| Proyecto | Mini galaxia | Nombre del proyecto referenciado |
| Nota (si se incluyen) | Nota adhesiva | Primeras 4 líneas |

Una luna cuyo fichero **no está en este equipo** (ruta absoluta del móvil, o fichero borrado) se pinta al
40 % de opacidad con un ⚠ pequeño. Sigue pudiéndose mover, anotar y conectar, pero no abrir.

**El fondo de estrellas.** 3 capas de puntos generadas con una semilla fija y guardadas en una baldosa
de 512×512 px que se repite. Tienen paralaje: la capa lejana se mueve al 20 % de la cámara, la media al
50 % y la cercana al 100 %. **En Ligero hay una sola capa sin paralaje.** Coste por fotograma: 1 a 3
bitmaps dibujados en mosaico.

### 4.4 Paneles

**D220 — Barra de ruta** (arriba, 36 px lógicos). Muestra `⌂ Cosmos › galaxia › planeta` según el astro
enfocado o el que está bajo el centro de la pantalla. Cada tramo se puede pulsar y vuela a ese astro. A la
derecha está el buscador (D226).

**D221 — Caja de herramientas** (izquierda). Arriba, las herramientas del universo; debajo, separadas, las
que ya existen:

| Tecla | Herramienta | Qué hace |
|---|---|---|
| `V` | Seleccionar | Mover, seleccionar, marco de selección (la actual) |
| `P` | Planeta | Clic: crea un planeta de radio 400 (mundo) y abre el inspector con el nombre en edición |
| `E` | Emoji | Abre el selector de emojis (§4.5) y lo coloca en el clic |
| `N` | Nota | Texto (la herramienta de texto actual) |
| `C` | Conectar | Arrastrar de un astro a otro crea una conexión del último tipo usado. Soltar fuera, nada |
| resto | Las del editor | Lápiz, rectángulo, elipse, flecha, texto, marco… sin cambios |

**D222 — Minimapa** (abajo a la derecha, 180×120 px lógicos, se oculta con `M`). Dibuja solo los discos de
las galaxias y un rectángulo con la vista actual. Pulsar o arrastrar sobre él mueve la cámara. Es barato
porque solo pinta galaxias (decenas) y se redibuja únicamente cuando cambia la cámara.

**D223 — Inspector** (derecha, 280 px lógicos). Aparece con **un** astro seleccionado y se oculta sin
selección, así que el lienzo queda entero para mirar.

- **Cabecera:** emoji, nombre (editable en planetas y exoplanetas; en galaxias muestra el nombre del
  proyecto sin editarlo aquí) y la clase.
- **Luna:** vista previa grande, nombre del fichero, tamaño, fecha, código de chat, proyecto.
- **Nota del astro:** caja de texto de varias líneas. Se guarda al salir de la caja.
- **Conexiones:** lista de las conexiones, con su tipo; pulsar una vuela al otro extremo, `Supr` la borra.
- **Botones:**
  - `Abrir` (lo mismo que Ctrl+clic)
  - `Ir al chat` (abre la ventana de chat en ese proyecto y resalta el mensaje)
  - `Mostrar en carpeta` (`abrir_ubicacion`)
  - `Devolver a la nebulosa` (solo lunas)
- **Planeta:** color (8 muestras), emoji, tamaño (S 250, M 400, L 700 de radio de mundo).
- **Galaxia:** interruptor «Incluir notas del chat» (D200), «Ordenar planetas» (D232).

Con **varios** seleccionados, el inspector enseña solo «N elementos» y los botones Conectar entre sí,
Agrupar en planeta nuevo y Devolver a la nebulosa.

### 4.5 Selector de emojis

Rejilla de 8 columnas con unos 200 emojis en 6 pestañas: caras, objetos, naturaleza, símbolos, espacio y
recientes. Lleva un buscador por nombre (en español e inglés, tabla fija en `pixpin-ui`). Se pinta con
DirectWrite con fuente de color (Segoe UI Emoji, que viene con Windows). No usa imágenes empaquetadas.

---

## 5. Cómo se usa

### 5.1 Ratón, teclado y trackpad

| Gesto | Resultado |
|---|---|
| Dos dedos en el panel táctil, en cualquier dirección | Desplaza la vista (ya resuelto en `navegacion.rs` y los commits `06fa333`–`c06c4cb`) |
| Pellizco / Ctrl+rueda | Zoom hacia el cursor |
| Espacio + arrastrar, botón central | Desplazar |
| Clic | Selecciona |
| Arrastrar astro | Lo mueve con todo lo que contiene (hijos + anotaciones dentro, D204) |
| **Ctrl + clic** | **Abre** (D224) |
| Doble clic en galaxia o planeta | **Enfoca** (D225) |
| Doble clic en luna | Abre el inspector con el nombre en foco. No abre el fichero: abrir es Ctrl+clic, para que un doble clic torpe no lance programas |
| `Inicio` | Encaja todo el cosmos |
| `F` | Encaja la selección |
| `Supr` | Borra del universo (D230) |
| `Ctrl+F` | Buscar (D226) |
| `Ctrl+Z / Ctrl+Y` | Deshacer / rehacer, astros y anotaciones en la misma pila (D234) |
| Arrastrar ficheros desde el Explorador | Los mete en el proyecto de la galaxia bajo el cursor y los coloca donde se sueltan (D227) |
| `Ctrl+V` con una imagen o ficheros | Igual que soltarlos, en el centro de la vista |

**D224 — Ctrl + clic abre.**

| Sobre | Abre |
|---|---|
| Luna con fichero en el equipo | El fichero con su programa (`pixpin_shell::abrir::abrir`) |
| Luna Dibujo / Pagina | El editor 2D de esa hoja, igual que desde el chat (`abrir_una_hoja`); al cerrarlo se vuelve al universo |
| Luna Nota / MiniApp | El chat, en ese mensaje |
| Luna sin fichero en el equipo | Nada; destello rojo y un globo «Este archivo no está en este equipo» durante 2 s |
| Galaxia | La conversación del proyecto en el chat |
| Planeta / exoplaneta | La **carpeta** de su proyecto (en un exoplaneta, la de la luna más reciente); `Ctrl+Mayús+clic` abre **todas sus lunas** hasta un tope de 10 (si hay más, pregunta) |

Un Ctrl+clic con cualquier herramienta activa abre igual: se comprueba antes del gesto, como hoy el
`enlace` (`ventana_editor.rs:~720-746`).

**D225 — Enfocar.** La cámara vuela en 250 ms (curva de salida) hasta encajar el astro con un 10 % de
margen. Lo que queda fuera del astro enfocado se pinta al 35 % de opacidad. `Esc` o un doble clic en el
vacío deshace el enfoque. **En Ligero el salto es instantáneo**, sin animación ni atenuado; el atenuado se
cambia por un velo plano, que cuesta un rectángulo.

**D226 — Buscar.** Al escribir filtra por nombre del astro, nombre del fichero, texto de la nota del astro
y, en notas y mini-apps del chat, su texto. Muestra hasta 20 resultados con su ruta (`Galaxia › Planeta`).
`Entrar` vuela al primero y lo resalta. Busca en un índice en memoria de cadenas ya en minúsculas, que se
construye al cargar cada galaxia (D236), así que no toca el disco al teclear.

**D227 — Meter archivos desde fuera.** Soltar ficheros sobre una galaxia llama a la misma lógica que el chat
(`meter_ficheros` + `cuaderno::adjunto`): el fichero se copia al proyecto, nace el mensaje y su luna aparece
en el punto de soltar, dentro del planeta si cae en uno. Si se sueltan en el cosmos vacío o en un
exoplaneta, se pregunta a qué proyecto van (menú con la lista de proyectos). **Así se cumple la regla del
usuario: el universo solo apunta a archivos del chat.**

### 5.2 Organizar

**D228 — Nebulosa.** Cada galaxia pinta, en su parte baja y dentro de su círculo, una rejilla de lunas sin
colocar: 10 columnas × 6 filas que empiezan en `centro + 0,55·radio`. Van ordenadas de la más nueva a la más
vieja, en celdas de 120×120 mundo y páginas de 60. El chip «+N más»
pasa de página. Arrastrar una luna fuera de la nebulosa la coloca. Una luna nueva que llega por el chat va a
la nebulosa, y una pastilla «3 nuevas» sobre la galaxia lo avisa hasta que se mira.

**D229 — Colocación inicial de las galaxias.** Las que no tienen posición se ponen en una espiral de ángulo
áureo alrededor del origen. La Vía Láctea va en (0, 0), con una separación de 6.000 mundo y un radio de
galaxia de 2.000. Un proyecto nuevo ocupa el siguiente hueco de la espiral que no choque con una galaxia
movida a mano.

**D230 — Borrar nunca borra archivos.**

| Se borra | Qué pasa |
|---|---|
| Luna | Vuelve a la nebulosa. **El fichero y el mensaje del chat quedan intactos** |
| Planeta | Sus lunas vuelven a la nebulosa. Sus anotaciones se borran, con aviso de confirmación si hay más de 5 |
| Exoplaneta | Cada luna vuelve a la nebulosa de su galaxia |
| Galaxia | No se puede borrar desde aquí: es un proyecto, se borra desde el chat. `Supr` no hace nada |
| Conexión / anotación | Se borra |

**D231 — Un proyecto borrado en el chat** hace desaparecer su galaxia. Sus lunas que estaban en
exoplanetas desaparecen también, y sus conexiones con ellas (D208). El registro dice cuántas.

**D232 — Ordenar planetas** (botón de la galaxia). Reparte los planetas de la galaxia en un anillo, y las
lunas de cada planeta en su órbita a partes iguales. Es opcional: nada se reordena solo, porque el usuario
organiza a mano y un auto-ordenado le desharía el trabajo.

**D233 — Crear un planeta a partir de una selección.** Con varias lunas seleccionadas, «Agrupar en planeta
nuevo» crea un planeta en su centro, con el radio justo para contenerlas más un 20 %, y se las asigna.

**D234 — Deshacer.** Una pila única para astros y anotaciones, con tope de 200 pasos. Cada paso es un
`Cambio` pequeño (mover N astros con su delta, crear, borrar con su copia, cambiar un campo). Las
anotaciones siguen usando su deshacer lógico (`borrado`); la pila del universo guarda qué pila tocar en
cada paso.

---

## 6. Lo que pesa poco (rendimiento)

El equipo de referencia es el de `2026-08-31-rendimiento-equipos-modestos-design.md`: portátil con Intel HD
4000 y 4 GB de RAM. Las reglas:

**D235 — Nada se anima en reposo.** Sin órbitas girando, estrellas que titilan ni brillos pulsantes. Con
el universo quieto la CPU está al 0 %, como el editor hoy. Las únicas animaciones son el vuelo al enfocar
(250 ms), el destello de rechazo (300 ms) y el pulso de «mostrar en el universo» (600 ms), y en Ligero
solo queda el destello.

**D236 — Se lee lo que se ve.**

| Dato | Cuándo se lee | Cuándo se suelta |
|---|---|---|
| `proyectos/indice.json` | Al abrir el universo | Al cerrar |
| `universo.json` (D240) | Al abrir | Al cerrar |
| `guardados.jsonl` de un proyecto | Cuando su galaxia pasa a ≥ 120 px de radio aparente (entra su contenido) | Cuando lleva 60 s por debajo de 6 px **y** hay más de 4 galaxias cargadas (se suelta la más antigua) |
| Miniaturas | Al pintar una luna en nivel vista previa. Se reutiliza `miniaturas.rs` (4 por fotograma) | LRU: 300 en Completo (~32 MB), 120 en Ligero (~13 MB) |
| Vista previa de dibujos | Igual que el chat: al pintar a ≥ 120 px | LRU de 20 |

La lectura del cuaderno ocurre **en un hilo aparte**. Mientras tanto la galaxia muestra sus planetas (que
están en `universo.json`) y un anillo de carga en vez de las lunas.

**D237 — De cada mensaje se guarda solo lo que se pinta.** Un `Mensaje` lleva texto entero, `resto` y
demás. En memoria se guarda una `FichaLuna`: código único, clase, nombre, ruta relativa, bytes, fecha,
código de chat, las primeras 200 letras del texto y su versión en minúsculas para buscar. Son unos 400
bytes por luna: 10.000 lunas son unos 4 MB.

**D238 — Pintar solo lo necesario.**
1. Los astros van en una rejilla propia (`RejillaAstros`, celda 1.000 mundo; la de `indice.rs` está atada a
   `Escena`), que se rehace solo cuando cambia el universo. Se piden los candidatos
   de la vista y se descartan los de nivel «no se pinta» antes de generar órdenes.
2. Un astro que no se pinta no pinta a sus hijos. Una galaxia lejana es 1 disco, aunque tenga 3.000 lunas.
3. **Tope de 600 lunas con ficha o vista previa por fotograma.** Si en la vista cupieran más, las más
   pequeñas bajan a nivel icono o punto. Así un zoom intermedio sobre una galaxia enorme no hunde el
   fotograma.
4. Los rótulos se miden una vez y se guardan en la caché por (texto, tamaño). DirectWrite es lo caro, no el
   rectángulo.
5. Al arrastrar se repinta todo en la primera entrega. Si la medición (D239) no cumple, se pasa a la capa
   congelada que ya existe: lo que no se mueve queda en un bitmap y solo se repintan los astros arrastrados y
   sus conexiones.
6. Las conexiones se recortan contra la vista con su caja envolvente antes de calcular la intersección con
   los círculos.

**D239 — Presupuesto** (medido con `medir_fotogramas`, D129):

| Medida | Completo | Ligero (HD 4000) |
|---|---|---|
| Abrir el universo (20 proyectos, 5.000 mensajes) hasta el primer fotograma | < 250 ms | < 400 ms |
| Fotograma desplazando sobre el cosmos | < 4 ms | < 8 ms |
| Fotograma con 600 lunas con ficha en vista | < 8 ms | < 14 ms |
| Memoria añadida con el universo abierto (sin miniaturas) | < 40 MB | < 30 MB |
| CPU en reposo | 0 % | 0 % |

---

## 7. Dónde se guarda

**D240 — Un solo fichero: `<raiz>/universo/universo.json`** (Q4 → A).

- **Qué contiene:** la cámara, las galaxias (proyecto → posición, radio, notas-del-chat sí/no, nota del
  astro), los planetas y exoplanetas, la colocación de las lunas (código único + proyecto → posición y
  padre; **nunca rutas**), las conexiones y la escena de anotaciones del motor 2D, embebida con el mismo
  serde que `.pixpin2d`.
- **Cómo se escribe:** JSON con `version: 1`, a un temporal y renombrado, como `formato.rs` (un corte de
  luz deja el anterior entero).
- **Cuándo se guarda:** 2 s después del último cambio y al cerrar.
- **Campos desconocidos:** todos llevan `serde(default)` y un `resto` aplanado, así que no se pierde lo que
  añada una versión futura (la misma regla que el cuaderno).
- **Tamaño:** unos 150 bytes por luna colocada. 5.000 lunas ocupan unos 750 KB y se escriben en menos de
  20 ms. Las que siguen en la nebulosa **no se guardan**: estar en la nebulosa es no tener entrada.

**D241 — Por qué no dentro de cada proyecto.** El universo mezcla proyectos (exoplanetas, conexiones entre
galaxias). Repartirlo en N ficheros haría que guardar una conexión tocara dos. El móvil no conoce el
universo: el `.pixpin` que viaja **no cambia**, así que la sincronización con Android no se toca.

**D242 — El enlace con el chat es el código único del mensaje** (`Mensaje::codigo_unico`). No se usa la
ruta ni la posición en la lista: el código sobrevive a renombrar el fichero, a reordenar el cuaderno y a
recibir el proyecto otra vez del móvil. Si el código no aparece en el cuaderno al cargar, la luna queda
**huérfana**: se pinta como fantasma con ⚠ y no se borra, porque puede que el mensaje llegue en la
siguiente sincronización. Hay un botón «Limpiar huérfanas» en el inspector de la galaxia.

**D243 — Cambios hechos fuera mientras el universo está abierto** (un mensaje nuevo en el chat, una
sincronización). No hay vigilante de ficheros, porque cuesta. Al recibir el foco la ventana (`WM_ACTIVATE`),
y cada vez que el chat manda un aviso por el mensajero al escribir, se compara la fecha y el tamaño de cada
`guardados.jsonl` cargado. Solo si cambió se vuelve a leer ese, en segundo plano.

**D244 — Imágenes pegadas.** Pegar una imagen en el universo **no** crea una imagen suelta en la escena:
la mete en el proyecto (D227) y nace como luna. Así se evita el hueco de hoy, en el que las imágenes pegadas
en el editor solo viven en memoria.

---

## 8. Errores

| Situación | Comportamiento |
|---|---|
| `universo.json` no existe | Se empieza vacío: galaxias en espiral y todo en nebulosas. No se crea el fichero hasta el primer cambio |
| `universo.json` corrupto | Se renombra a `universo.json.roto-<fecha>`, se empieza vacío y se enseña un aviso en la barra de ruta durante 6 s. **Nunca se pisa en silencio** |
| Un `guardados.jsonl` con líneas rotas | Se leen las buenas (`Cuaderno::leer`); la galaxia enseña «N mensajes ilegibles» en el inspector |
| Leer un cuaderno falla (E/S) | La galaxia se pinta con sus planetas y un ⚠. Se reintenta al siguiente foco de ventana |
| Abrir un fichero falla | Destello rojo en la luna y globo con el error de Windows 3 s; queda en el registro |
| Guardar falla | Se reintenta en el siguiente cambio; si falla al cerrar, se pregunta «No se pudo guardar el universo. ¿Cerrar igualmente?» |
| Dispositivo gráfico perdido | Se vuelven a subir la baldosa de estrellas y las miniaturas, como la caché de tinta |

---

## 9. Qué queda fuera

- Sincronizar el universo con el móvil.
- Vigilar carpetas del disco o vincular rutas sueltas (descartado por el usuario, punto 9 de §1).
- Animaciones de órbitas y estrellas vivas (D235).
- Anidar planetas (D203).
- Atar flechas del motor 2D a figuras libres (D206).
- Varios universos. Hay uno por instalación. Si hiciera falta, `universo.json` pasaría a una carpeta
  `universos/`, sin cambiar el formato.

---

## 10. Arquitectura

### 10.1 Piezas

| Pieza | Capa | Qué hace | Depende de |
|---|---|---|---|
| **`crates/pixpin-universo`** (nuevo) | L2 | Modelo puro: astros, conexiones, jerarquía, reglas D200–D208, nivel de detalle D218, nebulosa D228, espiral D229, deshacer D234, formato D240, búsqueda D226. **Sin Windows, sin Direct2D**, `forbid(unsafe)` | `pixpin-geom` (L0), `pixpin-motor2d` (L1: `Escena`, `Camara`, `Rejilla`), serde |
| `crates/pixpin-motor2d` | L1 | `Figura::Emoji` | — |
| `crates/pixpin-render` | L1 | Texto en color para emojis, degradado radial para el brillo de las galaxias, mosaico de bitmap | — |
| `crates/pixpin-ui` | L3 | Disposición pura de barra de ruta, inspector, minimapa y selector de emojis; botones nuevos de la caja | `pixpin-universo` |
| `apps/pixpin/src/universo/` (nuevo, módulo) | L4 | Pegamento: carga de cuadernos en hilo aparte, traducción `Mensaje → FichaLuna`, pintado de astros, Ctrl+clic, soltar ficheros, mensajes con el chat | todo lo anterior + `pixpin-proyecto`, `pixpin-shell` |
| `apps/pixpin/src/ventana_editor.rs` | L4 | `Modo::Universo`: engancha lo de arriba al bucle existente | — |
| `apps/pixpin/src/ventana_chat.rs` | L4 | Botones y menús D210–D212, Ctrl+U, avisos al universo | — |

**D245 — `pixpin-universo` no depende de `pixpin-proyecto`.** Las dos son L2 y la regla de capas lo
prohíbe. El universo recibe `FichaLuna` ya traducidas. La traducción `Mensaje → FichaLuna` vive en la app
(L4), que conoce a las dos. Hay que añadir la línea `"pixpin-universo" => 2` a `apps/pixpin/tests/capas.rs`.

### 10.2 Flujo de un fotograma

```
cámara ─▶ Rejilla de astros ─▶ candidatos ─▶ nivel_de(astro, cámara)  [pixpin-universo, puro]
                                               │
                                               ▼
                                 Vec<Visto { id, nivel, radio_px }> + conexiones visibles  [puro]
                                               │
                                               ▼
                        apps/pixpin/src/universo/pintar.rs → Pintor (Direct2D)
                                               │
      anotaciones (Escena) ──────── ordenes_de_escena_vista (existente) ──┘
```

Lo puro decide **qué se ve y a qué nivel**. La app lee cada astro y lo pinta. Así casi todo se prueba sin GPU.

### 10.3 Tipos principales (`pixpin-universo`)

```rust
pub struct IdAstro(pub u64);

pub enum Clase { Galaxia { proyecto: String }, Planeta, Luna { codigo: String, proyecto: String } }
// Exoplaneta = Planeta con padre None. No es otra clase: es un planeta en el cosmos.

pub struct Astro {
    pub id: IdAstro,
    pub clase: Clase,
    pub centro: Punto,       // mundo
    pub radio: f32,          // mundo
    pub padre: Option<IdAstro>,
    pub nombre: String,      // planetas; galaxias y lunas lo toman del chat
    pub emoji: Option<String>,
    pub color: Option<u32>,
    pub nota: String,
    pub notas_del_chat: bool,        // solo galaxias (D200)
    #[serde(flatten)] pub resto: Map<String, Value>,
}

pub enum TipoConexion { Relacion, Depende, Referencia, Secuencia(u32) }
pub struct Conexion { pub id: u64, pub desde: IdAstro, pub hasta: IdAstro,
                      pub tipo: TipoConexion, pub rotulo: String, pub color: Option<u32> }

pub struct Universo { astros: Vec<Astro>, conexiones: Vec<Conexion>,
                      pub anotaciones: Escena, pub camara: Camara, /* índices en memoria */ }

pub struct FichaLuna { pub codigo: String, pub proyecto: String, pub clase: ClaseLuna,
                       pub nombre: String, pub ruta: Option<String>, pub bytes: i64,
                       pub cuando: i64, pub codigo_chat: Option<String>,
                       pub extracto: String, pub busqueda: String, pub en_equipo: bool }

pub enum Nivel { Oculto, Punto, Disco, Icono, Ficha, Vista }
```

---

## 11. Pruebas

**Puras (`pixpin-universo`):**
- D200: qué clases de mensaje son lunas; el interruptor de notas; buzón fuera.
- D201/D202: soltar una luna en su galaxia, en un planeta de su galaxia, en otra galaxia (rechazo) y en un
  exoplaneta (acepta de cualquier proyecto).
- D203: un planeta soltado dentro de otro no es su hijo.
- D205: el padre es el contenedor más pequeño que contiene el centro, y se recalcula solo al soltar.
- D204: mover un planeta mueve sus lunas, sus anotaciones enteras dentro y nada más.
- D208/D231: conexiones con extremo inexistente se descartan al cargar.
- D218: nivel por radio aparente en cada umbral, con histéresis (subir a 120, no bajar hasta 102).
- D228: nebulosa en páginas de 60, de la más nueva a la más vieja; colocar saca de la nebulosa.
- D229: la espiral no choca con galaxias movidas a mano; la Vía Láctea en (0, 0).
- D230: borrar luna, planeta y exoplaneta devuelve lunas a la nebulosa; una galaxia no se borra.
- D233: agrupar en planeta nuevo contiene a todas con un 20 % de margen.
- D234: deshacer y rehacer de cada `Cambio`; el tope de 200.
- D226: buscar por nombre, nota y extracto; sin distinguir mayúsculas ni tildes.
- D238: con 5.000 lunas visibles, no salen más de 600 en ficha o vista previa.
- D240: ida y vuelta de `universo.json`; los campos desconocidos sobreviven; un fichero corrupto no se
  pisa.
- D242: una luna huérfana se conserva y se marca.
- Conexión: el punto de salida está en el borde del círculo, en la dirección del otro extremo.

**App (`apps/pixpin`, puras):**
- `Mensaje → FichaLuna`: extracto de 200 letras, `en_equipo` falso con ruta absoluta.
- D224: la tabla de qué abre Ctrl+clic según el astro.
- D236: la política de soltar cuadernos (60 s, más de 4 cargadas, la más antigua).
- D210–D214: los menús tienen las entradas nuevas.
- Capas: `pixpin-universo` es L2 y no depende de L2.

**Rendimiento (`--ignored`, con GPU):** los presupuestos D239 sobre un universo sintético de 20 proyectos y
5.000 lunas.

**Suite:** todas las pruebas actuales siguen en verde; `asignaciones.rs` sigue en 0 en el camino caliente
del motor 2D.

### Prueba manual del usuario

1. Chat → botón Universo: se ven las galaxias de los proyectos, con sus colores, en espiral.
2. Dos dedos en el panel táctil en diagonal: la vista se mueve en diagonal. Pellizco: zoom.
3. Acercarse a una galaxia: aparecen su nebulosa y sus archivos. Acercarse más: nombres y miniaturas.
4. `P` y clic: planeta nuevo, se le pone nombre y emoji. Arrastrar tres archivos de la nebulosa dentro.
5. Mover el planeta: sus archivos van con él.
6. Ctrl+clic en un PDF: se abre en su programa. En una foto: se abre en el visor.
7. `C` y arrastrar de un planeta a otro: conexión. Cambiarla a «Depende» en el inspector.
8. Soltar el planeta fuera de la galaxia: pasa a exoplaneta. Meter en él un archivo de otro proyecto.
9. Arrastrar un archivo de un proyecto a la galaxia de otro: vuelve con destello rojo.
10. Soltar un fichero desde el Explorador sobre una galaxia: aparece como archivo nuevo en el chat del
    proyecto y en el universo.
11. Cerrar y volver a abrir: todo sigue donde estaba.
12. Con el universo quieto, el Administrador de tareas marca 0 % de CPU.
