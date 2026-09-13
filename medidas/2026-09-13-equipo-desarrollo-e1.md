# Medición E1 — equipo de desarrollo — 2026-09-13

**Equipo:** portátil de desarrollo, CPU x86_64, GPU Intel(R) UHD Graphics integrada +
NVIDIA GeForce MX250 discreta (la puerta de pintado usa el adaptador `D3D_DRIVER_TYPE_HARDWARE`
por defecto del sistema, que en esta máquina no es la GPU más rápida disponible).
**Binario:** `target/release/pixpinmax.exe`, rama `e1-tinta-excalidraw` tras el commit de esta tarea.
**Método:** `cargo test --release` con las puertas escritas en los Steps 1 y 2 de la Tarea 12.
El agente no mide con entrada sintetizada (regla del usuario): estos números salen de las
puertas automáticas, no de manejar el ratón ni el lápiz.

## Puerta de geometría (Step 1, CPU)

`cargo test -p pixpin-motor2d --release --test puertas -- --test-threads=1`

| Prueba | Tope (spec E1 §4) | Medido |
|---|---|---|
| `un_trazo_de_cinco_mil_puntos_se_calcula_en_menos_de_dos_milisegundos` | < 2 000 µs | **903,7 µs** |
| `un_trazo_de_500_puntos_se_convierte_en_contorno_en_menos_de_2_ms` (puerta ya existente, renombrada en esta tarea) | ≤ 2 000 µs | 45 µs |

Las 11 pruebas de `puertas.rs` en release: **PASS** (0 fallos, ejecutadas en 0,05 s en total).

## Puerta de fotograma con 1.000 trazos (Step 2, GPU)

`cargo test -p pixpin-render --release --test puerta_tinta -- --ignored --test-threads=1`

La primera versión de esta puerta (Tarea 12, antes de la revisión) medía repintar los 1.000
trazos completos y le puso el tope de 3 ms de la spec §5. Eso mide el escenario equivocado: la
spec habla del fotograma **mientras se dibuja** (D120), donde los 1.000 trazos ya realizados
viven en `CapaEstatica` y el fotograma es un volcado (blit) más pintar SOLO el trazo en curso, sin
cache. La revisión partió esa puerta en dos:

| Prueba | Escenario | Tope | Medido (5 corridas en release) |
|---|---|---|---|
| `un_fotograma_dibujando_con_mil_trazos_en_escena_cuesta_menos_de_tres_milisegundos` | mientras se dibuja (D120): `CapaEstatica::volcar` + `Pintor::tinta` del trazo en curso, sin cache | spec §5, < 3 000 µs | **2,41 – 2,70 ms** en 4 de 5 corridas; **3,05 ms** en una corrida suelta (falló esa vez, justo en el borde) |
| `repintar_mil_trazos_realizados_cabe_en_un_fotograma_de_60_hz` (la puerta original, renombrada) | repintar los 1.000 trazos completos (p.ej. al reconstruir la capa estatica) — la spec no le pone 3 ms | 60 Hz, < 16 666 µs | **3,13 – 4,61 ms** en las mismas 5 corridas |

**Nota honesta:** la puerta del escenario correcto (mientras se dibuja) queda cerca del tope de
3 ms en esta máquina — pasa la mayoría de las veces (2,4-2,7 ms) pero se vio una corrida en 3,05 ms
que la hace fallar. No se ha tocado el tope de 3 ms (es el de la spec) ni el código de pintado
para maquillar el número. El repintado completo tampoco tiene margen sobrado frente a los 16,6 ms
de un fotograma a 60 Hz (hasta 4,61 ms de los 16,6 ms, con la misma GPU débil de esta máquina).
Ambas puertas quedan escritas y marcadas `#[ignore]`; hay que repetirlas en el equipo del usuario
o en hardware con una GPU dedicada real antes de dar cualquiera de las dos por buena de verdad —
el escenario que manda (mientras se dibuja) sigue siendo el más ajustado.

## Binario (Step 6)

| Momento | Tamaño |
|---|---|
| Antes de esta tarea | no medido en esta rama antes: el único `pixpinmax.exe` que quedaba en `target/release/` es de una compilación anterior sin fecha de commit fiable (2 986 KiB / 3 003 392 bytes, 2026-09-09) — no es una línea base controlada para esta tarea, porque no se sabe con qué commit exacto se generó |
| Después de esta tarea (`cargo build --release -p pixpin`) | **2 993 KiB / 3 066 880 bytes** |

Esta tarea no toca código de la app (`apps/pixpin`) ni de los cristales que se enlazan en el
binario más allá de un comentario de documentación (`laser.rs`) y pruebas de integración, así que
no se espera que el tamaño cambie por esto: la diferencia frente al build de 2026-09-09 es de
trabajo de tareas anteriores ya fusionado en la rama, no de esta tarea.

## Para que el usuario rellene con uso real

Medir con el Administrador de tareas mientras usa el editor de verdad (dibujar con el lápiz o el
ratón, dejarlo en reposo, abrir un dibujo grande). El agente no sintetiza esta parte.

| Métrica | Objetivo (spec) | Medido |
|---|---|---|
| CPU dibujando sin parar (columna CPU del proceso) | ≤ 10 % de un núcleo | |
| CPU en reposo con el editor abierto | 0 % | |
| RAM privada antes de abrir el editor | — | |
| RAM privada con un dibujo grande abierto | — | |
| Diferencia de RAM privada (con el dibujo abierto − antes de abrir el editor) | ≤ 40 MB | |
