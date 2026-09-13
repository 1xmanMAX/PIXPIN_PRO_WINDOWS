# Lienzo desde el pin, y la tinta del tamaño y la fluidez de Excalidraw

**Fecha:** 2026-09-13
**Estado:** diseño aprobado por el usuario, sección a sección
**Rama:** `lienzo-desde-pin` (sale de `medir`, que ya incluye E1)
**Programa:** es la continuación de E1 (`2026-09-13-e1-tinta-excalidraw-design.md`). La interfaz tipo
Excalidraw con iconos es el subproyecto siguiente, con su propio documento.

---

## 1. Por qué

El usuario probó E1 y dijo dos cosas:

1. Quiere **entrar al lienzo infinito desde un pin de imagen**, con la imagen centrada, y dibujar encima.
2. La tinta **se ve pequeña y «barata»** y, en el lienzo, **va lenta y poco fluida** al dibujar y al mover
   líneas.

Hoy el editor (verificado en el código):

| Hecho | Evidencia |
|---|---|
| No pinta imágenes: `Orden::Imagen` se ignora | `apps/pixpin/src/ventana_editor.rs:1125-1128` |
| La cámara es inmutable: sin zoom ni desplazamiento | `ventana_editor.rs:382` (`let camara = Camara::nueva()`) |
| La rueda no llega al motor | `a_evento` la descarta (`_ => None`, `ventana_editor.rs:134`) |
| Al cerrar, la escena se descarta | `apps/pixpin/src/main.rs:722-724` |
| El pin guarda sus anotaciones en un `.pixpin2d` junto al objeto | `apps/pixpin/src/pines.rs:890-912, 1033` |
| `bitmap_desde_pixeles` ignora el alfa | `crates/pixpin-render/src/motor.rs:234` (`D2D1_ALPHA_MODE_IGNORE`) |

### Por qué la tinta se ve pequeña (causa confirmada)

Excalidraw dibuja en píxeles CSS, que el navegador multiplica por la escala de Windows. El editor dibuja con
zoom 1 = un píxel físico y no aplica la escala al lienzo (sí a la barra, `escala_por_cien`). Con la pantalla
al 150 %, el mismo `strokeWidth` sale un tercio más fino que en Excalidraw, y un trazo fino se ve dentado.
El grosor por defecto sí coincide: Excalidraw usa `medium` → `FREEDRAW_STROKE_WIDTH.medium = 1`.

### Por qué va lenta (sin confirmar)

Tres sospechas, que se miden antes de tocar nada:

| # | Sospecha | Mecanismo |
|---|---|---|
| S1 | Vsync | `Present1(1, …)` bloquea el hilo hasta el refresco; mientras, no se leen puntos ni se pinta |
| S2 | Repintado que no llega | `WM_PAINT` solo se genera con la cola de mensajes vacía; con un ratón de 1000 Hz casi nunca lo está |
| S3 | Recalcular el trazo entero | `contorno_de_lapiz` sobre todos los puntos en cada fotograma: crece con el largo del trazo |

---

## 2. Orden

| Parte | Qué | Condición para empezar |
|---|---|---|
| **0a** | Escala de Windows en el lienzo + medición de fotogramas | Ya |
| **0b** | Arreglo de la fluidez | Tener el registro del usuario con la medición de 0a |
| **1** | Lienzo desde el pin | Ya (independiente de 0b) |

---

## 3. Parte 0a — tamaño y medición

| # | Decisión | Elección | Razón |
|---|---|---|---|
| D127 | Escala del lienzo | La cámara del editor trabaja en píxeles lógicos: zoom 1 = `escala_por_cien / 100` píxeles físicos. `camara.zoom` sigue siendo el zoom del usuario; la matriz de vista y `a_mundo`/`a_pantalla` multiplican por la escala del monitor | Mismo tamaño que Excalidraw al mismo `strokeWidth`; los ficheros no cambian (mundo) |
| D128 | Capa y pin | No cambian en 0a: allí la tinta va sobre los píxeles de una captura, y su escala es la de la imagen | Cambiarla alteraría anotaciones ya hechas |
| D129 | Medición | `[rendimiento] medir_fotogramas = true` en `pixpinmax.toml` (por defecto `false`). Con ella activa, el editor registra con `tracing::info!` una línea cada 60 fotogramas con: puntos recibidos, ms en vaciar la cola, ms en pintar, ms en presentar, ms de espera, y el máximo de cada uno | Diagnóstico en la máquina del usuario sin entrada sintetizada; apagado no cuesta nada |
| D130 | Entrega de 0a | Binario + instrucciones: activar la opción, dibujar 10 s en el lienzo con ratón (y con lápiz si hay), pasar el registro | Regla del usuario: el agente no usa su PC |

La escala del monitor la da `monitor.escala_por_cien`, que el editor ya recibe. Un cambio de DPI con el editor
abierto (`EventoOverlay::CambioDpi`) actualiza la escala y repinta.

---

## 4. Parte 0b — fluidez

Se decide con los números de D129. Cada rama tiene su arreglo y su prueba:

| Si la medición muestra | Arreglo | Prueba |
|---|---|---|
| S1: presentar domina (> 8 ms de media) | Presentar sin bloquear (`Present1(0, DXGI_PRESENT_DO_NOT_WAIT)` o el objeto de espera de latencia de la swapchain) y pintar como mucho una vez por refresco con temporizador | Puerta: pintar + presentar < 4 ms de media |
| S2: muchos puntos por fotograma pero pocos fotogramas por segundo | Pintar al terminar de vaciar la cola si pasó ≥ 1 refresco desde el último fotograma, sin esperar a `WM_PAINT` | Pura: el bucle decide pintar con la cola no vacía tras un refresco |
| S3: pintar crece con el largo del trazo | Recalcular solo la cola: el contorno de los puntos estables se guarda y se recalculan los últimos N (N = 32) | Oráculo: el contorno por cola coincide con el entero (tolerancia 0,01); puerta: 5.000 puntos < 0,5 ms por fotograma |

Si la medición no apunta a ninguna, se para y se vuelve a hablar con el usuario antes de tocar código.

---

## 5. Parte 1 — lienzo desde el pin

### Flujo

| # | Decisión | Elección |
|---|---|---|
| D131 | Entrada | Menú del clic derecho del pin → **«Abrir en lienzo»**, solo en pines de imagen. El doble clic sigue anotando dentro del pin |
| D132 | Qué se abre | `ventana_editor::abrir` con la escena del `.pixpin2d` del pin (o vacía) y la imagen del pin como **fondo fijo** en el mundo (0,0)–(ancho, alto): las coordenadas que ya usan sus anotaciones |
| D133 | Editar | Dibujar encima con todas las herramientas. La imagen no es un elemento: no se selecciona, no se mueve, no se borra |
| D134 | Si el pin está anotando | Primero guarda y sale de la anotación, luego abre el lienzo |
| D135 | Encuadre inicial | Imagen centrada; zoom 1 si cabe en el área de trabajo con 48 px lógicos de margen, si no el zoom que la hace caber |
| D136 | Moverse (convención de Excalidraw) | Rueda: desplazar vertical. `Shift+rueda`: horizontal. `Ctrl+rueda`: zoom hacia el cursor (`Camara::acercar_en`). Espacio mantenido + arrastrar, o botón central + arrastrar: desplazar |
| D137 | Editor de la bandeja | Sin cambios de comportamiento: sin fondo, sin guardar. Sí gana D127 y D136 |

### Pintar la imagen

| # | Decisión | Elección | Razón |
|---|---|---|---|
| D138 | Carga | `pixpin_codec::cargar` del objeto del almacén; subida una vez con una variante de `bitmap_desde_pixeles` con **alfa premultiplicado** | Un PNG transparente se ve sobre el blanco del lienzo, no sobre negro |
| D139 | Imagen gigante | Si supera `ID2D1DeviceContext::GetMaximumBitmapSize`, se reduce al cargar (lado mayor = máximo) y se pinta estirada a su tamaño real | La HD 4000 no admite texturas enormes |
| D140 | Orden de pintado | La imagen se pinta primero, dentro de la vista del mundo, y entra en la capa congelada | Coste cero por fotograma mientras se dibuja |
| D141 | Nitidez | `Pintor::bitmap` gana un modo: vecino más cercano si el zoom efectivo es exactamente 1 o ≥ 3; cúbica de calidad si es < 1; lineal entre medias | Píxeles nítidos al ampliar capturas; reducción sin dientes |
| D142 | Fuera de la vista | No se pinta | |
| D143 | Memoria | Una copia en GPU mientras el lienzo está abierto; se suelta al cerrar | ~8 MB una captura 1080p, ~32 MB una 4K |

### Guardar y volver

| # | Decisión | Elección |
|---|---|---|
| D144 | Modalidad | Como hoy: mientras el lienzo está abierto, las órdenes de otros pines y de la bandeja esperan en cola. El pin no puede cerrarse ni borrarse durante el lienzo |
| D145 | Por encima | La ventana del lienzo queda por encima de los pines |
| D146 | Al cerrar | Compactar; guardar en el `.pixpin2d` del pin con `pixpin_motor2d::guardar`; si la escena queda vacía y no había fichero, no crear ninguno; el pin recarga y repinta; soltar el bitmap; atender la cola |
| D147 | Lo de fuera | Lo dibujado fuera de la imagen se guarda; el pin no lo muestra; reabrir el lienzo lo muestra |

### Errores

| Situación | Comportamiento |
|---|---|
| El PNG no se puede leer | El lienzo abre con las anotaciones sobre un recuadro gris del tamaño de la entrada (si se conoce; si no, 800×600) y un aviso en el registro |
| El `.pixpin2d` está corrupto | El lienzo no se abre (no se pisa el fichero); error en el registro |
| Fallo al guardar | El fichero anterior queda intacto; error en el registro; el pin sigue con lo de antes |
| Dispositivo perdido | Se vuelve a subir el bitmap, como la caché de tinta |

Fuera de esta entrega: guardado automático periódico, deshacer entre sesiones, avisos en pantalla (P7.1).

---

## 6. Pruebas

**Puras:**
- D135: encuadre inicial (cabe → zoom 1 centrada; no cabe → zoom que la hace caber con margen).
- D141: modo de interpolación según zoom efectivo (1 exacto, 2, 3, 0,5).
- D136: rueda / `Shift` / `Ctrl` / espacio+arrastrar / central → desplazamiento o zoom correctos.
- D127: `a_mundo(a_pantalla(p)) == p` con escala 150 % y zoom arbitrario; un `strokeWidth` de 1 a zoom 1 y
  escala 150 % ocupa 1,5 veces lo que a escala 100 %.
- D146: escena vacía sin fichero previo no crea fichero; con fichero previo lo guarda vacío.
- Error: fichero corrupto no se sobrescribe.
- D131: el menú de un pin de imagen tiene «Abrir en lienzo»; el de una nota no.
- D129: con la opción apagada no se registra nada; encendida, una línea por cada 60 fotogramas con los campos.

**Con GPU (`--ignored`):**
- D138: un PNG con transparencia conserva el alfa al pintarse.
- D139: una imagen mayor que el máximo se reduce y se pinta a su tamaño lógico.

**Ida y vuelta:** abrir la escena de un pin, añadir un trazo, guardar, recargar con `pixpin_motor2d::cargar`:
mismo trazo en las mismas coordenadas.

**Suite:** las 944 pruebas actuales siguen en verde.

### Prueba manual del usuario

Parte 0a:
1. Activar `medir_fotogramas`, abrir el Editor, dibujar 10 s rápido con ratón (y con lápiz), cerrar, pasar el
   último fichero de `registros\`.
2. La tinta al grosor medio se ve del mismo tamaño que en excalidraw.com con la pantalla al mismo zoom.

Parte 1:
1. Clic derecho en un pin de imagen → «Abrir en lienzo»: imagen centrada, lo anotado antes aparece encima.
2. Rueda, `Shift+rueda`, `Ctrl+rueda`, espacio+arrastrar: fluido; al acercar mucho se ven píxeles nítidos.
3. Dibujar dentro y fuera de la imagen; `Esc`.
4. El pin muestra lo de dentro; reabrir el lienzo muestra también lo de fuera.
5. Una captura con transparencia no sale con fondo negro.
