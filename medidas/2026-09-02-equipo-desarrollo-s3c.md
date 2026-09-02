# Medición S3-C (anotar la pantalla) — equipo de desarrollo — 2026-09-02

**Equipo:** el de siempre (16 GB, 4 núcleos físicos, Intel UHD + MX250, monitor
3000×2000 al 150 %, nivel `Completo`). **Binario:** `target/release/pixpinmax.exe`,
rama `s3c-anotar-pantalla`, modo portable. **Método:** entrada sintetizada desde un
proceso DPI-aware (`keybd_event`/`mouse_event`/`SetCursorPos` en píxeles físicos),
tiempos del propio log, estado leído del almacén, CPU y RAM con `Get-Process`,
capturas de pantalla revisadas una a una.

## Los recorridos, punto a punto

**Capa viva (`Ctrl+Alt+A`)**
1. Atajo → `capa de anotacion abierta modo=Viva`; la caja de herramientas a la
   izquierda con el lápiz activo; la pantalla sigue viva debajo ✓
2. Lápiz: trazo rojo con la tinta del motor ✓
3. Foco (F): todo oscuro salvo el rectángulo arrastrado, con borde blanco ✓
4. Lupa (Q): amplía en vivo lo de debajo, la rueda cambia el aumento, y se
   coloca fuera de su propia fuente (no se amplía a sí misma) ✓
5. Texto (T): clic, «hola» con cursor mientras se escribe, `Enter` confirma ✓
6. `Espacio` → pasante; un clic sobre el terminal lo activa (primer plano
   `CASCADIA_HOSTING_WINDOW_CLASS`); la caja desaparece y el dibujo se ve ✓
7. `Ctrl+Alt+A` con la capa abierta → alterna el pasante **y devuelve el foco**
   a la capa; sigue habiendo una sola capa ✓
8. `Escape` → «¿Guardar lo dibujado como un pin?» → Sí → pin con la captura
   anotada, **sin caja ni lupa en la imagen** ✓

**Capa congelada (`Ctrl+Alt+Shift+A`)**: abre con la foto de fondo, lápiz y
flecha, `Espacio` no cambia nada, lupa sobre la foto, `Escape` → No →
`anotacion de pantalla descartada por el usuario` ✓

**Pin**: doble clic → `modo anotacion id=5 ms=10` y la paleta a su izquierda;
lápiz, foco y lupa (sobre el bitmap del pin) desde la paleta; texto «pin»;
`Escape` → `anotacion guardada id=5 elementos=3` (la lupa no es elemento),
`000005.pixpin2d` de 3576 B junto al objeto, la paleta destruida; reiniciar →
el pin vuelve con trazo, foco y texto ✓

## Los tres fallos que encontró esta prueba

Los tres son de **cableado entre mitades que funcionan por separado**, como en
S2-B y S3-B, y ninguna prueba unitaria podía verlos:

1. **El pasante no pasaba.** `WS_EX_TRANSPARENT` se leía como puesto y la
   prueba unitaria lo daba por bueno, pero los clics seguían llegando a la
   capa: un clic «pasante» con la herramienta de texto abría un texto in
   situ (se vio en la captura como un cursor huérfano). Ese estilo solo deja
   pasar el ratón en una ventana `WS_EX_LAYERED`. Arreglado poniendo los dos.
2. **El atajo reabría la capa al cerrarla.** Pulsado con la capa abierta, se
   encolaba en la ventana principal y se atendía al salir del bucle modal.
   Ahora el bucle modal se lo entrega al overlay (`EventoOverlay::Atajo`) y la
   capa lo usa para alternar el pasante, que es lo que pide D50 y la única vía
   cuando la aplicación de abajo ya tiene el foco.
3. **La lupa viva costaba un 28 % de CPU y 230 MB.** Muestreaba con una
   captura completa por movimiento; en este equipo el duplicador está tomado
   por el escritorio remoto y cada muestra caía a WGC. Ahora es una
   `SesionViva` con tope de fps que solo existe mientras la lupa está activa.

Y dos de paso: el zoom con la rueda guardaba escala 100 (el pin volvía 1,5×
más grande tras reiniciar) y una `Z` suelta deshacía mientras se escribía.

## Rendimiento

| Métrica | Objetivo (spec §4) | Medido |
|---|---|---|
| Atajo → capa visible | — | **68 ms** en frío (primera del día), **26 ms** en caliente |
| CPU de la capa viva en reposo | 0 % | **0 ms de CPU en 10 s** ✓ |
| Trazo siguiendo al ratón | ≤ 1 fotograma | sin retraso perceptible en las capturas; 188 ms de CPU por 5 trazos de 24 puntos |
| Capa viva sobre pantalla en movimiento | 60 fps `Completo` / 30 `Ligero` | la capa solo repinta cuando hay algo nuevo; con la lupa activa, **392 fotogramas en 8,3 s (≈47 fps)** a un 20 % de un núcleo |
| Entrar en modo anotación en un pin | < 50 ms | **10 ms** ✓ |
| RAM con la capa abierta | — | 64 MB privados; 67 MB con la lupa viva |

El tope de 30 fps en `Ligero` reutiliza exactamente el mecanismo verificado en
S1-B2 (`SesionViva` con `tope`), así que no se volvió a medir aquí.

## Notas honestas

- La entrada sintetizada **se descarta en silencio** cuando el primer plano
  es una ventana de un proceso elevado (`GameInputServiceWindow` tras matar la
  app). Costó media hora atribuírselo a la app; ahora el guion trae al frente
  el terminal antes de empezar. Queda anotado para las siguientes fases.
- Los procesos que sintetizan entrada tienen que ser DPI-aware
  (`SetProcessDPIAware`) para trabajar en píxeles físicos, que son los del log.
- El IME se coloca junto al texto, pero no se probó con un IME real (japonés
  o chino): la entrada sintetizada solo produce caracteres directos.
- El suelo real (i3 3.ª gen, 4 GB) sigue pendiente, como en todas las fases.
