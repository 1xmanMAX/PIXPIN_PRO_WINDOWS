# Láminas de geometría: lo nuevo del lienzo del PC, para portarlo a Android

15-oct-2026. El PC dibuja ahora láminas de geometría exactas (semicircunferencias,
cuadrantes rellenos con degradado, arcos de color sobre una circunferencia, marcas
de ángulo, el cuadradito del ángulo recto, rectas punteadas con flecha, puntos con
letra). Todo viaja en el `.excalidraw` de siempre con **claves nuevas y reserva**:
un Android que todavía no las entienda sigue viendo algo razonable en el mismo
sitio, y al guardar las conserva (como cualquier clave que no conoce).

El código del PC: `crates/pixpin-motor2d/src/geometria_exacta.rs` (figuras, pintado,
lectura y escritura), `gesto/geometria.rs` (las herramientas) y `enganche.rs` (el imán).

## Ninguna figura nueva: tres variables del arco

Un sector, un cuadrante, una semicircunferencia y una marca de ángulo **son
`pixpin-arc`** (`arcStart` y `arcSweep` en radianes, como ya están). Lo nuevo son
tres claves opcionales del elemento:

| Clave | Valor | Qué hace | Reserva (Android sin portar) |
|---|---|---|---|
| `cierreArco` | `"sector"` / `"segmento"` | Cierra el arco por los dos radios (sector, cuadrante) o por la cuerda (segmento); se rellena con `backgroundColor` | Se ve el arco abierto |
| `marcaAngulo` | `{"arcos": 1-3, "recto": bool, "valor": "45°"}` | El arco es la marca de un ángulo: su centro es el vértice; se pintan `arcos` arquitos concéntricos (separados `max(3, 2.2·strokeWidth)`), el sector relleno translúcido debajo y `valor` fuera, sobre la bisectriz (`fontSize`). Con `recto`, el cuadradito de lado = radio en vez de los arcos | Se ve un arquito |
| `degradado` | `{"tipo": "lineal"/"radial", "desde": "#rrggbb", "hasta": "#rrggbb", "origen": [fx, fy], "destino": [fx, fy]}` | El relleno va de `desde` a `hasta`. `origen`/`destino` en **fracciones de la caja** de la figura (0–1), así se mueve y se estira con ella. Lineal: de origen a destino. Radial: centro en origen, radio = distancia origen→destino. Vale para rectángulo, rombo, elipse, arco cerrado y `pixpin-region` | `backgroundColor` = `desde`, `fillStyle` sólido (el PC lo escribe así si no había fondo) |

Y en el trazo:

| Clave | Valor | Reserva |
|---|---|---|
| `strokeStyle` + `trazoRayaPunto` | `"dashed"` + `true` | Raya-punto (ejes, líneas de centro). Patrón: raya `5·w+4`, hueco `2·w+2`, punto `w`, hueco `2·w+2` (w = strokeWidth). Sin portar se ve discontinuo |

`fontSize` en `pixpin-point` y en `pixpin-arc`: el tamaño de la letra del punto (ya
lo lee `Puntos.kt`, `e.fontSize ?: 20`) y del valor de una marca de ángulo.

Al escribir, el PC **quita** una de estas claves solo si la entendía: un valor de una
versión futura (`"cierreArco": "espiral"`) vuelve intacto. Haced lo mismo.

## Las herramientas (lo que ve el usuario)

- **Arco** (grupo Formas): en el panel, «Trazar el arco»:
  - *Guía* (de fábrica, como el móvil): sobre el borde de un óvalo de referencia o
    de una guía sin repasar, se arrastra y queda el arco repasado
    (`empezarArcoSobre` / `Gesture.Arcoing`). El PC lo hace también con el Óvalo.
  - *Centro*: clic en el centro, clic donde empieza (da el radio) y clic donde
    acaba; mientras, el arco sigue al cursor en el sentido en que se mueve
    (`barridoAcumulado`). Un solo deshacer se lo lleva.
  - *3 puntos*: inicio, uno por donde pasa, fin.
  - «Cierre del arco»: abierto / sector / segmento. Con relleno, «Relleno degradado».
- **Marca de ángulo** (nueva, grupo Formas): un clic dentro del ángulo, cerca de
  donde se juntan dos rectas. Se buscan **todas** las rectas que pasan por el vértice
  más cercano (a 60 px de pantalla) y la marca va entre las dos vecinas que encierran
  el clic; el radio es la distancia del clic al vértice (8–120). A 90° ± 0,5° pone el
  cuadradito. Relleno: el del panel, o el color del trazo al 30 %. «Arquitos»: 1, 2, 3.
- **Estilo del trazo**: sólido, discontinuo, punteado y **raya-punto**.
- **Relleno degradado** (rectángulo, rombo, óvalo, arco cerrado, bote): liso, lineal,
  radial. De fábrica va del color de relleno a uno 80 % más claro (`desde_relleno`).
- **Recortar** una circunferencia entre dos rectas deja un arco (ya estaba en
  `Recorte.kt`); un arco recortado pierde su cierre y su degradado.
- **Extender** un arco sigue **por su circunferencia** hasta lo primero que toca.

## El imán

Nuevos en el PC (Android tiene `Iman.kt`): 

- **Tangente** (`TipoAnclaje::Tangente`): mientras se traza una recta/flecha/cota,
  los dos puntos de tangencia desde su primer punto a cada circunferencia cercana
  (en un arco, solo si caen en su tramo). Marca: circulito con una raya que lo roza.
- **El arco** ofrece sus dos puntas (extremo), su punto medio y el centro; ya no las
  esquinas de su caja.
- Cruces, canto (puntos sobre la circunferencia) y tangentes encendidos en el editor.

## Para el asistente

`geometria_precisa` (crate `pixpin-ia`, `edicion.rs`) acepta `figura`:
`semicircunferencia` (`puntos` del diámetro, `lado`), `cuadrante`, `sector`, `segmento`
(`centro` + `radio` + `inicio`/`fin`/`sentido`, o `centro` + `puntos` de las puntas de
los radios), `arco` por tres `puntos`, `marca_angulo` (`vertice`, `puntos` de cada lado,
`arcos`, `valor`), `angulo_recto`, `punto` (`letra`, `letra_hacia`), y en todas `relleno`,
`degradado`, `relleno_hasta`, `trazo: raya_punto`, `flecha` (`fin`/`inicio`/`ambas`) con
`punta` (`rellena`/`abierta`) y `tam_letra`. Ángulos en grados de matemáticas (0 a la
derecha, 90 arriba).
