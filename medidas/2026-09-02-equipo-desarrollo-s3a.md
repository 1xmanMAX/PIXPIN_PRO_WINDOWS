# Medición S3-A (motor 2D) — equipo de desarrollo — 2026-09-02

**Equipo:** el de siempre (16 GB, 4 núcleos físicos, monitor 3000×2000 al
150 %). **Método:** `cargo test -p pixpin-motor2d --test puertas --release`.
Los tests viven en `crates/pixpin-motor2d/tests/puertas.rs` y **corren en CI**
en cada cambio, así que estos números no son una foto: son una guardia.

## Las cuatro puertas de §6 de la spec

| Métrica | Objetivo | Medido | Margen |
|---|---|---|---|
| Trazo de 500 puntos → polígono | < 2 ms | **38 µs** | 52× |
| Órdenes de 100 elementos (sin caché) | ≤ 1 fotograma (16,6 ms) | **< 1 ms** | > 16× |
| Escena de 1000 elementos en memoria | < 20 MB | **144 KB** (144 B por elemento) | 145× |
| Abrir un `.pixpin2d` de 1000 elementos | < 150 ms | **18 ms** | 8× |

Y la que de verdad importa, que no es de velocidad:

| Invariante | Comprobación |
|---|---|
| **El dibujo es idéntico al reabrirlo** | Se guarda una escena de 31 elementos (30 figuras + un trazo de 80 puntos), se lee y se comparan las órdenes de dibujo punto por punto: iguales ✓ |

## Lo que estos márgenes permiten decidir

Los 38 µs por trazo cambian una decisión del diseño: **la caché de geometrías
(D43) deja de ser urgente**. Un dibujo de 100 elementos se regenera entero en
menos de un milisegundo, así que en S3-B se puede empezar sin caché y añadirla
solo si el dibujo sobre pantalla en vivo (S3-C, 60 fps sostenidos) la pide. Es
la misma disciplina que retiró la Tarea 11 de S2-B: complejidad solo con los
números delante.

Los 144 bytes por elemento salen del tamaño de la estructura, no del
asignador: un elemento con puntos añade su `Vec`. Un trazo típico de 200
puntos son 1,6 KB más, así que mil trazos reales rondarían 1,7 MB — sigue muy
lejos del tope.

## Corrección de diseño encontrada al implementar

La spec decía que el motor dependería de `pixpin-render` para dibujar. **No
podía**: los dos son L1, y la regla del proyecto prohíbe que una capa dependa
de sí misma (`apps/pixpin/tests/capas.rs` lo habría rechazado en la primera
compilación). El motor pasa a producir **órdenes de dibujo** —polígonos y
polilíneas con su color— y las pinta el consumidor, que ya tiene pintor.

El resultado es mejor que lo planeado: el motor entero queda **puro**, sus 72
tests corren en CI sin GPU ni escritorio, y las mismas órdenes servirán para
exportar a SVG en S4 sin tocar una línea de geometría. `pixpin-render` ganó
`Pintor::poligono` y `Pintor::polilinea`, que reciben pares de `f32` y por eso
no atan el renderizador a este crate.

## Notas honestas

- Los topes se relajan ×20 en depuración: el coma flotante de Rust sin
  optimizar va varias veces más lento y medir ahí no diría nada. Lo que se
  comprueba siempre es el orden de magnitud.
- El aspecto del trazo (que parezca tinta y no un cable) **no está verificado
  por ojo humano todavía**: el motor no tiene con qué dibujarse hasta S3-B.
  Los tests garantizan la geometría, no la belleza.
- El suelo real (i3 3.ª gen, 4 GB) sigue pendiente. Con 52× de margen en la
  operación más cara, no se espera problema, pero eso se dice cuando se mide.
