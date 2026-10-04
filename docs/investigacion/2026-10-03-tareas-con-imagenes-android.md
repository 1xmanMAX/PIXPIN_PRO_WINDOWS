# Tareas con imagenes — cómo hacerlo igual en Android

3-oct-2026. Lo pidió el usuario: «que una tarea pueda llevar imágenes, como cuando pego una
imagen en la terminal de Claude Code». En el PC ya está (ventana de Tareas, panel de la
mini-app en el chat y el pedido `anadir_tarea` del plugin de Flow Launcher). Este documento
dice **el formato exacto**, cómo pintarlo y cómo **adjuntar imágenes desde dentro de la
aplicación** en Android. Android de hoy no hay que tocarlo para que nada se rompa (ver §2);
lo de §3 y §4 es para verlo y usarlo igual que en el PC.

> En Android **no** se adjunta desde el buscador del teléfono ni desde el lanzador: solo
> dentro de la aplicación (pantalla de la lista de tareas). Lo dijo el usuario.

## 1. El formato

Como la fecha de creación (`2026-10-02-tareas-con-fecha.md`), las imágenes van **dentro del
texto de la casilla**, como imágenes de Markdown, **delante** de la marca de la fecha:

```
# Inbox

- [ ] comprar yeso ![img 01](pixpin:files/guardados/pc/general/archivos/tarea-1759500000000-01.png) ➕ 2026-10-03
- [ ] ![img 01](pixpin:files/guardados/pc/general/archivos/tarea-1759500000000-02.png) ➕ 2026-10-03
- [x] grieta ![img 01](pixpin:files/guardados/grieta.jpg) en el muro ![img 02](pixpin:files/guardados/pc/obra/archivos/tarea-1759500000000-03.png) ➕ 2026-10-01
```

Reglas (las mismas que `mini::imagenes_de` / `mini::fichas_a_imagenes` del PC):

- Una imagen es `![alt](enlace)`: el `alt` no lleva `]`; el enlace no va vacío y **no lleva
  blancos ni `)`** (la sincronización corta ahí, `Disco.enTexto`). Expresión equivalente:
  `!\[([^\]]*)]\(([^)\s]+)\)`.
- El `alt` es `img 01`, `img 02`… (dos cifras, desde 1, en el orden en que se adjuntaron). Es
  solo un rótulo: **el lector no lo mira**, puede decir cualquier cosa.
- Pueden ir en cualquier sitio del texto, pero **siempre antes de `➕ AAAA-MM-DD`**: así
  `Tareas.partir` (regex `CREADA`, «la fecha al final») sigue encontrando la fecha sin cambios.
- Una tarea puede ser solo imágenes (texto vacío): vale.
- Lo que se lee de la tarea es el texto **sin** las imágenes, con los blancos que dejan
  juntados en uno: `grieta ![..](..) en el muro` se lee «grieta en el muro».
- Lo que no cumple las reglas (`[x](y)` sin `!`, `![a]()`, `![a](mi foto.png)`) **es texto** y
  se enseña tal cual.

### El enlace

Es la ruta portátil de siempre, `pixpin:files/<ruta dentro de files/>`, la misma que usan las
notas (`2026-10-01-incrustados-de-notas-android.md`):

- **Nacida en el PC**: `pixpin:files/guardados/pc/<chat>/archivos/tarea-<ms>-<nn>.<ext>`. El
  PC copia la imagen a `archivos/` del chat de la lista, **sin mensaje propio en el chat**
  (como un documento metido en una nota: el chat no se llena de fotos sueltas). En Android
  ese fichero llega a `files/guardados/pc/<chat>/archivos/…` como cualquier adjunto del PC.
- **Nacida en Android**: guardar la copia en la carpeta de adjuntos del chat
  (`MensajesStore.carpetaDeAdjuntos()` = `files/guardados/`), con nombre `tarea-<ms>-<nn>.<ext>` (sin
  blancos ni paréntesis), y escribir en el texto **la ruta absoluta** de ese fichero, igual
  que hace hoy un mensaje con adjunto: `Disco.aPortatil` la cambia a `pixpin:files/guardados/
  tarea-….jpg` al salir y `aLocal` la vuelve absoluta al entrar. En el PC eso se guarda en
  `<chat>/android/guardados/tarea-….jpg` y se encuentra solo.
- **Por qué viaja**: `alcance_de` (PC) y `Disco.alcance` (Android) ya mandan **cualquier
  fichero que un mensaje nombre** con `pixpin:files/` dentro de su texto, y el documento de
  la lista ES el texto del mensaje `MINIAPP`. No hace falta nada nuevo en la sincronización.
- **Siempre una copia propia**. Nunca enlazar el fichero original (galería, caché del
  lanzador, descargas): se borra o se mueve y la tarea se queda sin foto.

### Mover una tarea a otra lista (otro chat)

El PC (`tareas::mover`, «Mover a…» del Inbox) **copia** cada imagen al `archivos/` del chat
de destino y cambia el enlace en su sitio. Es lo que hay que hacer también en Android si un
día se mueven tareas entre listas: un enlace del móvil (`pixpin:files/guardados/x.jpg`) en el
PC se busca en la carpeta de **cada** chat (`<chat>/android/…`), así que en el chat de destino
no se encontraría; y aunque los del PC se encuentran desde cualquier chat, borrar el chat de
origen dejaría la tarea sin foto. Lo que todavía no ha llegado (sin fichero) se deja con su
enlace tal cual.

## 2. Compatibilidad hoy, sin tocar Android

- `Tareas.leer` lee la casilla y guarda el texto **crudo**, con las imágenes dentro; 
  `Tareas.escribir` lo vuelve a escribir igual. Marcar, mover, borrar otra tarea o renombrar
  **no pierden** las imágenes. ✔
- `Tareas.partir` encuentra la fecha porque las imágenes van antes. ✔
- La fila (`DeTareas`, `MiniActivity.kt`) enseña hoy el texto crudo: «comprar yeso
  ![img 01](/data/user/0/…/tarea-….png)». Se entiende pero es feo: §3 lo arregla.
- Al corregir una tarea (lápiz), el campo enseña el texto con los enlaces (el PC hace lo
  mismo en su panel): no se pierden al guardar.

## 3. Qué haría falta en Android para verlo igual que en el PC

1. **`Tareas.imagenes(texto)`** (junto a `partir`): devuelve el texto sin imágenes (blancos
   juntados, recortado) y la lista de enlaces, en orden. Se llama sobre lo que ya devuelve
   `partir(t.texto).first`.
2. **La fila**: el `Text` con el texto limpio y, **detrás del texto**, una miniatura por
   imagen de **28 dp** de lado, recortada al centro (`ContentScale.Crop`, esquinas de 4 dp),
   4 dp entre ellas y 8 dp tras el texto. El texto cede su sitio (se corta con «…» antes que
   las miniaturas). En una tarea hecha, las miniaturas con alfa 0,55, como el texto apagado.
3. **Sin fichero** (aún no ha llegado del PC): el hueco de 28 dp con fondo
   `surfaceVariant` y el icono `Icons.Filled.Image` en `onSurfaceVariant`. Tocarlo dice
   «Esa imagen aún no está en este equipo: llegará con la sincronización».
4. **Tocar una miniatura**: abre la imagen a pantalla completa con el visor de fotos del
   chat (el mismo que al tocar una foto de un mensaje), con pellizco para ampliar. En el PC el
   clic la saca a la pantalla como pin; el equivalente del teléfono es el visor.
5. **El resumen y la burbuja** del chat no cambian («3 de 7»). Si la burbuja enseña la
   primera tarea, que use el texto limpio.

## 4. Adjuntar imágenes desde la aplicación (solo dentro de la app)

En la línea de añadir de la lista (`LineaParaAnadir`, `MiniActivity.kt`) y en el campo de
corregir una tarea:

1. **Un botón de clip** (`Icons.Filled.AttachFile` o `AddPhotoAlternate`) a la izquierda del
   `+`. Abre el selector de fotos del sistema (`PickVisualMedia`, solo imágenes, varias a la
   vez) y, si se quiere, «Hacer foto» (`TakePicture`).
2. **Pegar**: si el portapapeles trae una imagen (pulsación larga → Pegar, o el teclado de
   Gboard con su portapapeles de imágenes: `contentReceiver` / `OnReceiveContentListener`
   con `image/*`), se adjunta igual que con el botón. El texto pegado sigue entrando como
   texto.
3. **Cada imagen adjunta** se copia en el acto a `files/guardados/tarea-<ms>-<nn>.<ext>`
   (como `MensajesStore` copia un adjunto; reducida si pasa de ~2000 px de lado, como las
   fotos del chat) y en el campo aparece una **chapa** `[img 01]` (`img 02`…) en el sitio del
   cursor: texto en `primary`, fondo `primary` con alfa 0,24, esquinas de 5 dp. Una chapa se
   borra entera con una sola pulsación de retroceso; mantenerla pulsada enseña la miniatura.
   (En Compose: `VisualTransformation` que pinta las fichas `[img NN]` como chapa, o una fila
   de miniaturas con aspa encima del campo si las chapas cuestan; lo que importa es el
   texto final.)
4. **Al añadir** (botón `+` o Intro): cada ficha `[img NN]` que siga en el texto se cambia por
   `![img NN](<ruta absoluta de su copia>)`; las imágenes cuya ficha se borró del campo se
   descartan (y su copia se borra); `Tareas.anadir` pone la fecha detrás como siempre.
   Resultado: `- [ ] comprar yeso ![img 01](/data/…/files/guardados/tarea-….jpg) ➕ 2026-10-03`,
   que sale hacia el PC como `pixpin:files/guardados/tarea-….jpg`.
5. **Cancelar** (vaciar el campo o salir sin añadir) borra las copias de las imágenes que no
   llegaron a ninguna tarea.

Y lo que **no** se hace: ni el buscador del teléfono, ni la tarea rápida desde fuera
(`TareaRapidaActivity`), ni un acceso directo adjuntan imágenes. Solo la pantalla de la lista.

## 5. Casos de prueba compartidos con el PC

Los mismos que `crates/pixpin-proyecto/src/mini.rs` (pruebas `las_imagenes_de_una_tarea_…`,
`una_ficha_sin_imagen_…`, `cambiar_enlaces_…`, `caso_negativo_lo_que_no_es_una_imagen_…`):

| Entrada (texto de la casilla) | Texto que se lee | Enlaces | Fecha |
|---|---|---|---|
| `comprar yeso ![img 01](pixpin:files/a.png) ➕ 2026-10-03` | `comprar yeso` | `pixpin:files/a.png` | 2026-10-03 |
| `yeso ![img 01](x.png) y arena` | `yeso y arena` | `x.png` | — |
| `![img 01](a.png) ➕ 2026-10-03` | (vacío) | `a.png` | 2026-10-03 |
| `ñandú ![img 01](ñ.png) ★` | `ñandú ★` | `ñ.png` | — |
| `un [enlace](x.png) normal` | igual | — | — |
| `![sin cierre](x.png` | igual | — | — |
| `![vacia]()` | igual | — | — |
| `![con blanco](mi foto.png)` | igual | — | — |
| `! [separada](x.png)` | igual | — | — |
| `pan` (tarea de antes) | `pan` | — | — |

Al **componer** (`fichas_a_imagenes`), con imágenes 1 → `a.png` y 2 → `b.png`:

| Texto del campo | Resultado |
|---|---|
| `comprar yeso [img 01]` (solo la 1) | `comprar yeso ![img 01](a.png)` |
| `[img 01] mira [img 03]` | `![img 01](a.png) mira [img 03] ![img 02](b.png)` (la 2 sin ficha va al final; la 3 sin imagen se queda como texto) |
| (vacío, solo la 1) | `![img 01](a.png)` |
| `pan ➕ 2026-10-01` (solo la 1) | `pan ![img 01](a.png) ➕ 2026-10-01` (la fecha sigue al final) |

Ida y vuelta: `Tareas.escribir(titulo, Tareas.leer(doc)) == doc` para un documento con estas
líneas, y marcar una tarea con imágenes cambia solo `[ ]` por `[x]`.

## 6. Dónde está en el PC

- `crates/pixpin-proyecto/src/mini.rs`: `imagenes_de`, `con_imagenes`, `fichas_a_imagenes`,
  `cambiar_enlaces`, `ficha_de_imagen`, `rotulo_de_imagen`, y sus pruebas.
- `apps/pixpin/src/tareas.rs`: guardar una imagen en el chat (`guardar_imagen`, un `.bmp` se
  guarda como `.png`), apuntar con imágenes (`apuntar_con`), resolver un enlace
  (`ruta_de_imagen`) y moverlas con la tarea (`mover`).
- `apps/pixpin/src/tareas/ventana.rs`: Ctrl+V con imagen en la caja (chapa `[img 01]`,
  retroceso, vista al pasar el ratón) y las miniaturas de las filas (clic → pin).
- `apps/pixpin/src/ventana_chat.rs` + `mini_panel.rs`: las miniaturas en el panel de la lista
  dentro del chat (clic → pin).
- `apps/pixpin/src/pedidos.rs` + `docs/protocolo-pedidos.md`: `anadir_tarea.imagenes`.
