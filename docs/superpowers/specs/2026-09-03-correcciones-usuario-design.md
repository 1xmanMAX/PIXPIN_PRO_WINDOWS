# Correcciones tras la primera prueba del usuario

**Fecha:** 2026-09-03 · **Estado:** hecho · **Origen:** el usuario probó el
binario de S1-B3 y reportó siete cosas (2026-09-02).

## 1. Lo reportado y la causa

| # | Reporte | Causa | Arreglo |
|---|---|---|---|
| 1 | Al agrandar un pin, a partir de un tamaño se desplaza al lateral superior sin crecer | Windows limita el tamaño de una ventana al de la pantalla por defecto (`WM_GETMINMAXINFO`); `SetWindowPos` movía sin agrandar | El pin responde a `WM_GETMINMAXINFO` con un tope de 32 000 px |
| 2 | La sombra solo alrededor de dos vértices | La misma: la superficie se recreaba mayor que la ventana recortada y la sombra quedaba fuera por abajo y por la derecha | El mismo arreglo |
| 3 | Quitar los atajos `Ctrl+Alt+X`, `Ctrl+Alt+D`, `Ctrl+Alt+Shift+A` | Petición | D81: esos tres atajos son `Option` en el TOML y por defecto no se registran; la captura con barra sigue en la bandeja («Capturar») |
| 4 | Pinear directo con `Alt + clic derecho y arrastrar`; copiar directo con `Alt + clic izquierdo y arrastrar` | Petición | D81: gancho de ratón de bajo nivel (`pixpin-shell::gestos`) |
| 5 | La ficha de archivo y la nota no deben redimensionarse: al intentarlo «se queda en su posición y el resto se redimensiona» | La ficha estiraba a lo ancho y la nota en proporción; el contenido no acompaña | D82: `Contenido::redimensionable()` es falso para ficha y nota; el estado del pin nace `fijo` (esquinas = mover, sin doble clic, sin rueda, sin «Tamaño original» en el menú) |
| 6 | El vídeo pineado no se reproduce | El fichero del usuario es HEVC (`hvc1`) y el equipo no tiene la extensión «HEVC Video Extensions»; Media Foundation no lo decodifica y el pin se degradaba a ficha/documento sin decir por qué | El pin degradado lleva la coletilla «sin códec de vídeo» (`pin-sin-codec`). H.264 (mkv de OBS) se reproduce |
| 7 | Con una herramienta activa el puntero sigue siendo las cuatro flechas | El pin siempre ponía `IDC_SIZEALL`/`IDC_SIZENWSE`; la capa siempre la cruz | `CursorAnotacion` en el pin y `FormaCursorWin::{Texto, Flecha}` en el shell: cruz para dibujar, barra para texto, flecha para la mano; el gestor lo pone al cambiar de herramienta |

### 1.1 Segunda vuelta del reporte 1 (el usuario: «sigue pasando, con todo pin»)

El tope de `WM_GETMINMAXINFO` no bastaba. La captura de la prueba lo mostró: la
ventana sí crecía más allá de la pantalla, pero **el contenido dejaba de pintarse**
(la superficie de composición no se podía recrear a ese tamaño y el error se
tragaba en silencio). Como el zoom es desde el centro, la esquina de la ventana
se iba hacia arriba a la izquierda mientras la imagen no crecía y se «escondía».

Arreglo (D83): **la ventana del pin nunca es mayor que el escritorio virtual**.
`ventana_visible` recorta la ventana ideal al escritorio; el contenido se pinta
desplazado dentro de ella (`Pintor::desplazar`, transformación D2D que
`dibujar` reinicia en cada fotograma porque el contexto es compartido). Los
puntos del ratón, el IME y las anotaciones usan el origen real del contenido
(`origen_contenido`, preguntado a la ventana). Mover un pin mayor que la
pantalla cambia su parte visible: es una redimensión de ventana. La superficie
se redimensiona en sitio con `ResizeBuffers` (`Superficie::redimensionar`) y
solo se recrea si eso falla; ambos fallos quedan en el log. El tope pasa a
20 000 px por lado (`MAXIMO_FISICO`): el tamaño real ya no cuesta memoria.

**Zoom por arrastre (D84, idea del usuario):** `Ctrl + clic y arrastrar` sobre
el pin; hacia arriba agranda, hacia abajo encoge, exponencial (300 px = el
doble), desde el centro, con los mismos topes que la rueda. En la ficha y la
nota, `Ctrl + arrastrar` mueve como un arrastre normal. Sin modificador el clic
sigue moviendo, y `Alt` está reservado a los gestos globales.

### 1.2 Tercera tanda (2026-09-03): fichas, notas, Markdown, cursor, zoom fluido, lag

| Reporte | Arreglo |
|---|---|
| El nombre largo de una ficha se sale de la tarjeta | La ficha crece a lo ancho con el nombre hasta 560 px lógicos (`FICHA_ANCHO_MAX_LOGICO`); más allá, nombre y detalle van en **una línea con puntos suspensivos** (`Pintor::texto_linea`, recorte DirectWrite) |
| Las notas deben poder redimensionarse y hacer zoom | D85: la nota tiene **redimensión libre** por las esquinas (cada eje por su lado, el texto se recoloca al ancho nuevo, `redimension_libre`) y **zoom proporcional** con la rueda o `Ctrl + arrastrar` (texto incluido, `zoom_texto`, persistido en `PinGuardado::zoom_por_cien`). Doble clic = «Tamaño original»: zoom 1 y tamaño natural |
| Markdown en las notas | D86: `pixpin-pin::markdown`, puro y probado: títulos `#`, listas `-`/`1.`, citas `>`, código con vallas, reglas `---`, `**negrita**`, `*cursiva*`, `` `código` ``, `[texto](enlace)`. Las líneas de un párrafo se conservan (una nota es texto pegado). El pintor gana texto con tramos de estilo (`Pintor::parrafo`, `EstiloTexto`, `Tramo`) y el motor `medir_parrafo` |
| El cursor sobre un pin es una «cruz» (las cuatro flechas) | Flecha normal sobre el pin; diagonal solo en las esquinas (el único aviso de que ahí se redimensiona) |
| El zoom con la rueda va a saltos | D87: cada muesca **anima** el pin hacia el destino (140 ms, salida cúbica, temporizador de 16 ms) y las muescas encadenan desde el destino en curso, no desde el fotograma intermedio; el giro fraccionado de una rueda fina da pasos proporcionales |
| Lag al capturar o mover | D88: la superficie del pin ya no reasigna memoria en cada fotograma: crece con un cuarto de margen (`Superficie::asegurar`) y se compacta al acabar el gesto (`compactar`). Mover un pin sin recorte sigue sin repintar. Los fotogramas de redimensión de más de 24 ms quedan en el registro («redimension lenta») para diagnosticar en el equipo del usuario. La captura en este equipo va por WGC porque el duplicador lo tiene tomado el escritorio remoto: 50-90 ms, no arreglable desde aquí |

## 2. Decisiones

| # | Decisión | Elección | Razón |
|---|---|---|---|
| D81 | **Gestos con Alt en lugar de tres atajos** | `WH_MOUSE_LL` en el hilo principal. Con Alt (y solo Alt) pulsado, el botón izquierdo o derecho se **traga** y llega a la ventana de mensajes como `WM_GESTO` con el punto; el bucle abre el overlay en modo copiar (izquierdo) o pinear (derecho) e inyecta el pulsado en ese punto, toma la captura del ratón y, al soltar con selección, confirma solo. La soltada **no** se traga: el overlay la necesita. El gancho se suspende mientras un overlay o la capa están abiertos. El estado «botón abajo» se lleva en el gancho: una pulsación tragada nunca llega a `GetAsyncKeyState` | Lo pidió el usuario. Un `Alt+clic` sin arrastre abre el overlay normal, para seleccionar a mano |
| D82 | **Ficha y nota fijas** | Solo se mueven. La rueda tampoco las escala | El tamaño lo da el contenido; estirarlas solo abría un hueco |

## 3. Verificación (E2E, entrada sintetizada, binario release)

- `Ctrl+Alt+X`, `Ctrl+Alt+D`, `Ctrl+Alt+Shift+A`: sin efecto; el log registra 5 atajos ✓
- `Alt+izquierdo` 1000,600 → 1400,900: el portapapeles tiene una imagen de 400×300 ✓
- `Alt+derecho` 1000,600 → 1600,1000: pin de 600×400 ✓; 24 muescas de rueda → 5989×4012 en (−1689, −1203): crece más allá de la pantalla ✓
- Ficha (`win.ini`) y nota: arrastrar la esquina las mueve 300,200 sin cambiar el tamaño; el cursor en la esquina es el de mover ✓
- Vídeo mkv H.264: se reproduce (dos huellas distintas a 700 ms) ✓; mp4 HEVC: ficha «5,1 MB · sin códec de vídeo» ✓
- Cursor en el pin anotando: cruz → texto → flecha según herramienta; al salir, mover ✓. En la capa viva, igual ✓
- `Alt+clic` sin arrastre: abre el overlay y `Esc` lo cierra sin copiar ✓
