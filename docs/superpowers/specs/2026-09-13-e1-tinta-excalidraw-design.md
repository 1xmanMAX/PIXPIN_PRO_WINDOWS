# E1 — La tinta de Excalidraw, nativa

**Fecha:** 2026-09-13
**Estado:** diseño aprobado por el usuario, sección a sección
**Rama:** `e1-tinta-excalidraw` (sale de `medir`, que aún no está en `main`)
**Alcance:** entrada, geometría y pintado del trazo a mano. No las formas, ni
las flechas, ni la interfaz (E2–E5).

---

## 1. Por qué

El usuario abrió el editor 2D y la tinta se ve «pésima». Pidió construir sobre
Excalidraw y el Excalidraw de Obsidian. Se analizaron los tres repos y se
decidió **no** meter un navegador: se porta Excalidraw a código nativo.

### La decisión web contra nativo

| | Excalidraw en WebView2 | Excalidraw nativo (elegido) |
|---|---|---|
| RAM con el editor abierto | ~150–250 MB | ~25–40 MB (a medir) |
| Abrir el editor | 0,5–1,5 s | < 100 ms |
| Tinta | 1 punto por fotograma | todos los puntos y la presión |
| Anotar rápido y editor avanzado | dos motores | el mismo motor |

El «Excalidraw local» de Obsidian **no es nativo**: es el mismo React dentro
del Chromium de Electron, con un fork (`@zsviczian/excalidraw`) que añade
plumas y modo lápiz. Su calidad sale de técnicas y parámetros, que se pueden
copiar.

### Licencias, comprobadas

| Pieza | Licencia | Uso |
|---|---|---|
| `excalidraw/excalidraw` | MIT | Se porta código |
| `zsviczian/excalidraw` (el motor de Obsidian) | MIT | Se porta lo que añade |
| `steveruizok/perfect-freehand` | MIT | Se porta código |
| `zsviczian/obsidian-excalidraw-plugin` | **AGPL-3.0** | Solo ideas, nunca código |

### Por qué se ve mal hoy (diagnóstico verificado en el código)

| # | Causa | Evidencia |
|---|---|---|
| 1 | Solo `WM_MOUSEMOVE`, enteros, `sleep(5 ms)` y repintado completo entre lecturas: ~40–60 puntos/s | `pixpin-shell/src/overlay.rs:510`, `apps/pixpin/src/ventana_editor.rs:409` |
| 2 | Trazos de menos de 48 px se pintan como polilínea de grosor fijo, sin tapas | `pixpin-motor2d/src/pintado.rs:409-458` |
| 3 | Relleno `ALTERNATE`: agujeros donde el contorno se cruza | `pixpin-render` `lienzo.rs:715-738` sin `SetFillMode` |
| 4 | Contorno con `AddLines`, sin curvas | `lienzo.rs:732` |
| 5 | `size = grosor` (3) en vez de `grosor × 4.25`; afilado forzado; sin tapa inicial; presión inicial fija 0,25; presión del lápiz nunca leída | `pintado.rs:118-121`, `trazo.rs:89,150,164-167,222-229`, `gesto.rs:439` |
| 6 | Cada movimiento invalida la ventana entera y recalcula todo; pincel y geometría nuevos por primitiva; `Present(0,0)` | `ventana_editor.rs:306`, `cache.rs:93-111`, `lienzo.rs:186-195` |

Descartado: antialias (es `PER_PRIMITIVE` por defecto) y borrosidad por DPI.

---

## 2. El programa «Excalidraw nativo»

| # | Subproyecto | Entrega |
|---|---|---|
| **E1** | **Tinta** | Este documento |
| E2 | Formas a mano | Rectángulo, rombo, elipse, línea, flecha (rough.js fiel), texto |
| E3 | Enlaces y marcos | Flechas enganchadas a formas, frames |
| E4 | Editor avanzado | El plan del 2026-09-06: selección múltiple, transformar, grupos, deshacer total |
| E5 | Interfaz | Barra, panel de estilo, biblioteca, exportar |

### Reglas comunes

| # | Decisión | Elección |
|---|---|---|
| D100 | Fuente | Excalidraw oficial como principal; fork de zsviczian solo para lo que añade. Clonados en `proyectos de referencia/` (fuera de git), cada uno en un commit fijado |
| D101 | Mapa de traducción | `docs/excalidraw/mapa.md`: fichero origen → commit → módulo Rust. Cabecera de cada módulo portado con su origen. Aviso MIT en `THIRD-PARTY-NOTICES.md` |
| D102 | Oráculo | `herramientas/oraculo-excalidraw/` (Node) ejecuta las funciones reales y escribe ficheros de referencia JSON en `crates/pixpin-motor2d/tests/oraculo/`. Rust compara con tolerancia 0,01. Se regeneran solo al portar una actualización: la CI normal no necesita Node |
| D103 | Novedades | `herramientas/novedades-excalidraw` lista los commits de ambos repos que tocan ficheros del mapa desde el último commit portado |
| D104 | Consumo | Se conservan las puertas actuales (CPU en reposo 0 %) y cada subproyecto añade las suyas |

Commit de partida de Excalidraw oficial: `afa3a65` (2026-09-10).

---

## 3. Entrada

| # | Decisión | Elección | Razón |
|---|---|---|---|
| D105 | Ratón | `WM_MOUSEMOVE` + `GetMouseMovePointsEx` (hasta 64 puntos con marca de tiempo) para recuperar los fusionados | Todos los puntos de un ratón de 1000 Hz |
| D106 | No `EnableMouseInPointer` | Descartado | Es de proceso entero: pondría en riesgo overlay, pines y gestos |
| D107 | Lápiz | `WM_POINTER*` solo en las ventanas de dibujo; `GetPointerPenInfoHistory` con presión (0–1024 → 0–1) y posición HIMETRIC subpíxel. La goma se detecta y se ignora hasta E4 | Presión real y fidelidad |
| D108 | Modelo | `PuntoEntrada { x: f32, y: f32, presion: Option<f32>, t_ms: u32 }` en coordenadas del **lienzo** | Corrige el desfase de sumar el origen de pantalla |
| D109 | Presión y suavizado | Ratón: presión simulada, `streamline 0.5`. Lápiz: presión real, `streamline 0.2`. Solo se descartan duplicados exactos | Igual que Excalidraw (`App.tsx`, `DEFAULT_STROKE_STREAMLINE[_PRECISE]`) |
| D110 | Bucle | Fuera `sleep`; `MsgWaitForMultipleObjectsEx`; se vacía la cola entera y se pinta **un** fotograma sincronizado con el refresco | No perder puntos y no pintar lo que no se ve |

Lectura de Windows → `pixpin-shell`. `PuntoEntrada` y la elección de presión →
`pixpin-motor2d` (puro, probado).

---

## 4. Geometría

| # | Decisión | Elección |
|---|---|---|
| D111 | perfect-freehand fiel | Rehacer `trazo.rs` siguiendo el original: `t = 0.15 + (1 − streamline)·0.85`; presión inicial = media de los 10 primeros puntos; `RATE_OF_PRESSURE_CHANGE 0.275`; ruido final 3 px; normal interpolada por producto escalar; interpolación en trazos cortos; tapas de 13 (inicio) y 29 (fin) segmentos; `FIXED_PI`; **sin afilado** |
| D112 | `last` | Siempre `true`, también durante el trazo (así lo hace `getVariableWidthFreedrawOutline`) |
| D113 | Contorno → forma | Cuadráticas por puntos medios (`getSvgPathFromStroke`). `Pintor` gana `trayecto` (mover / cuadrática / cerrar) |
| D114 | Relleno | `D2D1_FILL_MODE_WINDING` |
| D115 | Nivel de detalle | Se elimina `TINTA_MINIMA_PX = 48`. Solo se simplifica un trazo cuya caja mide < 2 px en pantalla |
| D116 | Pluma variable (por defecto) | `size = strokeWidth × 4.25`, `thinning 0.6`, `smoothing 0.5`, easing `sin(t·π/2)` |
| D117 | Pluma constante | Porte del `LaserPointer` de Excalidraw: `size = strokeWidth × 1.4`, `simplify 0`, presión 1, `sizeMapping max(0.1, p)` |
| D118 | Grosores | `FREEDRAW_STROKE_WIDTH`: fino 0.5, medio 1, grueso 2. Color y opacidad como su panel |
| D119 | Fichero | El elemento guarda `pressures`, `simulatePressure` y `strokeOptions { variability, streamline }` en `.excalidraw` y `.pixpin2d`; sin esos campos, variable y presión simulada |

### Oráculo de E1

Unos 20 trazos fijos: recta, curva, espiral, bucle que se cruza, punto suelto,
dos puntos, zigzag rápido, lápiz con presión creciente, cada uno en variable y
constante. Se generan con `getStroke`, `LaserPointer` y `getSvgPathFromStroke`
reales. Rust debe coincidir en cada punto con tolerancia 0,01.

### Puerta de geometría

Mientras se dibuja, el trazo se recalcula entero cada fotograma (como
Excalidraw). Solo si la puerta falla se recalcula la cola.
**Trazo de 5.000 puntos < 2 ms** en el equipo de desarrollo (a apuntar en el i3).

---

## 5. Pintado

| # | Decisión | Elección | Razón |
|---|---|---|---|
| D120 | Escena congelada | Al pulsar, la escena se pinta una vez en `CapaEstatica`; cada fotograma = copiar capa + trazo en curso; al soltar, el trazo entra en la capa | El coste deja de crecer con el dibujo |
| D121 | Caché por elemento | `ID2D1GeometryRealization` por versión del elemento; no un bitmap por elemento | En el i3 la memoria de vídeo es compartida |
| D122 | Zoom | Durante el gesto se estira la realización; tras 300 ms quieto (500 ms en `Ligero`) se rehace nítida | Como `shouldCacheIgnoreZoom` de Excalidraw |
| D123 | Pinceles | Caché por color y opacidad | Hoy uno por primitiva |
| D124 | Recorte | Solo se pintan elementos visibles | |
| D125 | Presentar | `Present1` con el rectángulo sucio de la cola, con vsync; sin cambios no se pinta | 0 % CPU en reposo |
| D126 | Compartido | En `pixpin-render`; lo usan el editor, la capa de anotar pantalla y la anotación del pin | La tinta buena llega a los tres |

### Puertas de pintado

| Puerta | Umbral |
|---|---|
| Movimiento → tinta en pantalla | ≤ 1 fotograma (≤ 17 ms a 60 Hz) |
| Fotograma con 1.000 trazos en escena mientras se dibuja | ≤ 3 ms |
| CPU dibujando sin parar | ≤ 10 % de un núcleo |
| CPU en reposo con el editor abierto | 0 % |
| RAM añadida al abrir el editor con 1.000 trazos | ≤ 40 MB |

---

## 6. Errores y degradación

| Situación | Comportamiento |
|---|---|
| `GetMouseMovePointsEx` o el historial del lápiz fallan | Solo el punto del mensaje; un aviso en el registro, una vez |
| `GeometryRealization` falla | Se pinta la geometría directamente |
| Dispositivo perdido | Se reconstruyen capa y cachés, como el pin |
| Nivel `Ligero` | Mismos algoritmos; retardo del redibujado nítido 500 ms |
| Fichero sin presiones ni modo | Variable, presión simulada |

---

## 7. Pruebas

- **Oráculo** (sección 4), en `pixpin-motor2d`.
- **Puras:** entrada → trazo, elección de presión, descarte de duplicados,
  contorno → cuadráticas, lectura y escritura de `strokeOptions`.
- **Puertas** de las secciones 4 y 5, en `pixpin-render` y `apps/pixpin/tests`.
- Las pruebas existentes siguen en verde, lanzadas como indica la memoria
  del proyecto (no `cargo test` a secas).

### Prueba manual del usuario

El agente no usa la PC del usuario (regla del 2026-09-03). Se entrega el
binario en su `portable\` con esta lista:

1. En el editor, escribir el nombre rápido con el ratón: letras redondas, sin
   esquinas ni agujeros.
2. Espiral rápida, bucle que se cruza, punto suelto.
3. Con el lápiz: apretar y aflojar; el grosor cambia.
4. Variable y constante; los tres grosores.
5. Guardar `.excalidraw` y abrirlo en excalidraw.com: se ve igual.
6. Lo mismo en la anotación del pin y en la capa sobre pantalla.

---

## 8. Fuera de E1

- Plumas extra del fork de Obsidian (subrayador, estilográfica…): el usuario
  pidió las de Excalidraw.
- Goma del lápiz, borrador, lazo: E4.
- Recalcular solo la cola del trazo: solo si falla la puerta.
- Interfaz de selección de pluma más allá de lo que ya tiene el editor: E5.
