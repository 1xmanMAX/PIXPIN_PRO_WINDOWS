# Plan: chat como Telegram, lienzo como Excalidraw, y las tablas

Fecha: 16-sep-2026. Encargo del usuario, en sus palabras: la pantalla de
información del chat con sus pestañas («solo vídeos, solo archivos»), el menú
de adjuntar, copiar y pegar, crear un lienzo desde el chat, las tablas, la
visualización de proyectos, y las funciones que le faltan al editor (marco,
relleno, «texto mejorado» y más).

Este documento existe para que el orden no lo decida el entusiasmo. Cada fase
dice **qué se entrega**, **cómo se comprueba** y **qué NO incluye**, porque la
mitad del riesgo de una lista tan larga es creer que algo está hecho cuando
está a medias.

## Reglas que no cambian

1. **Telegram Desktop es GPL-3.0.** De ahí salen medidas, colores y
   comportamientos; nunca código ni recursos. PixPin Max es MIT y `deny.toml`
   rechaza copyleft: copiar su código obligaría a publicar todo el motor bajo
   GPL.
2. **PixPin Android es del usuario y es la referencia.** Ahí sí se lee y se
   cita el código. Cuando Android y Telegram discrepan (las fechas «Hoy» y
   «Ayer», por ejemplo), manda Android: esta app es su puerto.
3. **Lo que no se entiende se conserva.** Todo modelo lleva su `resto` con
   `#[serde(flatten)]`: abrir y volver a guardar desde Windows no puede tirar
   campos de una versión más nueva del móvil.
4. **La geometría va en `pixpin-ui`, pura y probada.** Medir texto necesita
   las fuentes del sistema, así que mide quien pinta y pasa el alto ya hecho.
5. **Nada de datos de ejemplo en la carpeta del usuario sin borrarlos después.**

---

## Fase 1 · Las tablas (empezada)

**Hecho**: `pixpin_proyecto::tabla` — el mismo `tablas/<id>.json` de Android,
con referencias `B7` ↔ (columna, fila), y cinco pruebas. Es un **mapa de celda
a texto**, no una matriz: una tabla con una celda en `A1` y otra en `Z900`
ocupa dos entradas, y la fusión celda a celda sale gratis porque un mapa se
junta clave por clave.

**Falta**:

1. **Fórmulas.** Hoy se guarda `=SUMA(B1:B6)` y se enseña tal cual. Calcular
   pide: analizador de expresiones (números, referencias, rangos, `+ - * /`,
   paréntesis), las funciones que use Android (`SUMA`, `PROMEDIO`, `MIN`,
   `MAX`, `CONTAR`), y **detección de ciclos** — sin ella, `A1 = B1` y
   `B1 = A1` cuelgan la aplicación. Puro y con pruebas.
2. **La rejilla en pantalla**: `pixpin_ui::rejilla` con alto de fila, ancho de
   columna, cabeceras `A B C` y `1 2 3`, y **solo se pintan las celdas que se
   ven** (igual que las filas del chat).
3. **Editar una celda**: escribir, `Entrar` baja, `Tab` avanza, `Escape`
   cancela. La barra de fórmula arriba.
4. **Una tabla como mensaje** del chat (`MINIAPP` con su palabra), y abrirla
   desde ahí.

**Comprobación**: `AA1` es la columna 27 y no la 26; `A0` no es una celda;
borrar una celda la quita del mapa en vez de dejar una cadena vacía que
viajaría por la red como si fuera un cambio.

---

## Fase 2 · La pantalla de información del chat, con sus pestañas

Es lo que el usuario pidió primero: pulsar el nombre en la cabecera y ver el
proyecto separado por tipos.

1. **`pixpin_ui::info`** — geometría pura: la columna de la derecha (ancho
   mínimo/máximo, y a partir de qué ancho de ventana pasa a pantalla
   completa), la cabecera con su botón de volver, la ficha con el avatar
   grande, y la **barra de pestañas** con su rayita indicadora.
2. **Las pestañas**, con los nombres de Android, no los de Telegram: `TODO`,
   `FOTOS`, `ARCHIVOS`, `VOZ`, `DIBUJOS`, `FIJADOS`, `BUZÓN`. Son las de
   `Seccion` del móvil, y usarlas hace que las dos apps se sientan la misma.
3. **La cuadrícula de fotos**: columnas según el ancho, separación fija,
   celdas cuadradas. Solo se pintan las que se ven.
4. **La lista de archivos**: icono por extensión, nombre, tamaño y fecha.
5. **Filtrar el historial** por sección: `Cuaderno::de_seccion`, puro y
   probado, para no repetir el criterio en la pantalla.

**No incluye**: buscar dentro de la sección (fase 3).

---

## Fase 3 · Buscar dentro de la conversación

El buscador de la izquierda ya filtra proyectos. Falta el de dentro:

1. `pixpin_ui::resaltado` — dónde cae cada coincidencia dentro de un texto,
   sin distinguir mayúsculas ni acentos. Puro y probado.
2. Pintar el resaltado en la burbuja.
3. Saltar de una coincidencia a otra, y que el historial se desplace hasta
   ella.

---

## Fase 4 · Adjuntar, copiar y pegar como Telegram

1. **El menú de adjuntar** al pulsar el clip: panel con sus entradas
   (imagen, archivo, **lienzo nuevo**, **tabla nueva**), con las medidas de
   Telegram.
2. **Crear un lienzo desde el chat**: entra como mensaje `DIBUJO` con su
   `referencia`, se escribe su `.excalidraw` vacío y se abre el editor.
3. **El diálogo de confirmación** al soltar o pegar: vista previa, nombre,
   tamaño y pie de foto. Hoy se guarda sin preguntar, que es rápido pero no
   deja poner un pie ni arrepentirse.
4. **Copiar desde el chat**: `Ctrl+C` sobre un mensaje copia su texto; sobre
   una imagen, la imagen.

---

## Fase 5 · El editor: lo que le falta a Excalidraw

Depende del inventario que está en marcha; el orden previsto, de más a menos
notable:

1. **Texto**. Hoy no se puede escribir en el lienzo. Hace falta entrada con
   cursor e IME, doble clic para editar, tamaño, familia y alineación. Es el
   hueco más grande: un croquis sin rótulos no sirve para una obra.
2. **Relleno**: sólido, rayado y cruzado, con los campos `backgroundColor` y
   `fillStyle` de Excalidraw, y el trazado a mano alzada que los hace parecer
   dibujados.
3. **Copiar y pegar dentro del lienzo**, y desde el sistema.
4. **Imágenes en el lienzo**: pegar una imagen y que quede como elemento, con
   su entrada en `files` del `.excalidraw` y en `imagenes/` del paquete.
5. **Marcos (frame)**: agrupan por contención; mover el marco mueve lo de
   dentro. Es lo que hace falta para presentar.
6. **Agrupar y bloquear**.
7. **Unión de flechas a figuras**: que la flecha siga a la figura al moverla.
8. **Enlaces entre lienzos** (`Element.enlace` en Android): pulsar una zona y
   saltar a otro lienzo.

**Regla dura de esta fase**: escribir de vuelta el `.excalidraw` **no puede
perder** los elementos que aquí no se entienden, ni el `appState`, ni la
escala. Antes de tocar el escritor hay que tener una prueba de ida y vuelta
que lo demuestre sobre un fichero real del móvil.

---

## Fase 6 · Visualización de proyectos

1. **Abrir un proyecto entero** desde el chat: sus hojas en la mesa, que es
   lo que ya hace `Pines::abrir_paquete`.
2. **Modo presentación**: una hoja a pantalla completa, avanzar y retroceder.
3. **Imprimir y exportar a PDF**, que en Android es de esta misma versión.

---

## Fase 7 · Terminar la sincronización por wifi

Lo que falta del crate `pixpin-sincro`, ya con el protocolo 4 portado:

1. **Descubrimiento mDNS** `_pixpin._tcp.` y la sonda `PING`/`PONG`.
2. **El bucle del diálogo**, los dos papeles.
3. **Diferencia y fusión** con sus parches.
4. **El envío puntual**, incluido el sentido invertido (`_pixpinrecibe._tcp.`):
   el ordenador enseña su código y el móvil le manda algo.

**Tres cosas que NO se deducen a ojo**, porque romperían la compatibilidad en
silencio y sin síntoma claro: la forma canónica exacta del JSON (de ella salen
todos los resúmenes), las rutas portátiles (`pixpin:files/`) y el orden de
claves del sello. Hay que leerlas del código del móvil o comprobarlas contra
un volcado real.

---

## Cómo se sabrá que cada fase está hecha

- Compila, `clippy` limpio, `cargo fmt` aplicado.
- Pruebas de lo puro, incluyendo **al menos un caso negativo** por pieza: lo
  que no debe pasar es tan importante como lo que sí.
- Probado en pantalla con datos de ejemplo, y **los datos de ejemplo
  borrados** de la carpeta del usuario al terminar.
- Un commit por fase, con el porqué de las decisiones que no son obvias.
