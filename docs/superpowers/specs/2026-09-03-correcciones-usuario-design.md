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
