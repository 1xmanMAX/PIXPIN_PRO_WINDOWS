# Medición S1-B2 (overlay) — equipo de desarrollo — 2026-09-02

**Equipo:** el mismo del informe de S1-B1 (16 GB, 4 núcleos físicos, GPU integrada 128 MB VRAM,
monitor 3000×2000 al 150 %). **Binario:** `target/release/pixpinmax.exe`, rama `s1b2-overlay`.
**Método:** entrada sintetizada (`keybd_event`/`mouse_event`), tiempos del propio log
(`overlay visible` con desglose), RAM/CPU con `Get-Process`.

| Métrica | Objetivo | Medido |
|---|---|---|
| **Atajo → overlay visible (caliente)** | **< 50 ms** (sagrado) | **27-44 ms** en 5 aperturas seguidas (peor: 44) ✓ |
| Atajo → overlay visible (primera vez) | el camino lento que 5.3 autoriza una única vez | 89 ms (creación perezosa de dispositivo+ventanas) |
| CPU con overlay abierto, congelado y quieto | 0 % | **0 ms de CPU en 5 s** ✓ |
| RAM privada con overlay abierto (1 monitor 3000×2000) | anotar (primera medición) | **7,4 MB privados** — las texturas viven en memoria de GPU, fuera de los bytes privados; en gráficos integrados salen igualmente de la RAM del sistema (D18) pero el SO las atribuye al driver |
| Fotogramas en vivo, `Completo` | > que Ligero | 121 aceptados en ~2 s ≈ **60 fps** (sin tope) ✓ |
| Fotogramas en vivo, `Ligero` forzado | ≈ 30/s | 86 aceptados en ~2,8 s ≈ **30 fps** ✓ — primer consumidor real del nivel funcionando |
| Puerta de los bytes (`overlay_no_muta`) | bytes idénticos | ✓ en verde con GPU real |
| Selección arrastrada | sin tirones perceptibles | subjetivo: fluida en las pruebas sintetizadas; pendiente de mano humana |

## Cómo se bajó de 200 ms a 27: tres persistencias, medidas y no adivinadas

El desglose del log (`captura_ms`/`pintar_ms`/`mostrar_ms`) señaló tres costes por invocación:

1. **Crear el dispositivo D3D11: ~90 ms** → `Recursos` lo retiene entre capturas.
2. **El primer fotograma de WGC: 80-100 ms** → congelación por **Desktop Duplication** persistente
   (pull, cero coste en reposo, caché del último fotograma), con WGC de respaldo si el duplicador
   está frío o pierde el acceso.
3. **Ventana + DComp + swapchain: ~90 ms** → las ventanas se **ocultan** en vez de destruirse, y
   ~25-40 ms más salieron de mostrar **sin activar** (la activación de `ShowWindow` sincroniza con
   el shell; el foco lo toma `enfocar()` después, fuera del camino de «visible»).

## Incidencias reales cazadas ejecutando (no leyendo)

- El primer `AcquireNextFrame` de un duplicador recién creado con pantalla quieta entrega una
  textura **vacía** marcada con `LastPresentTime=0`; copiarla daba la captura en negro. Lo cazó el
  test de píxeles reales.
- `SetForegroundWindow` a secas es no determinista (funcionó a las 21:49, falló a las 21:52 con el
  mismo binario): el overlay quedaba **sordo al teclado**. Remedio: `AttachThreadInput` temporal.
- Con `panic = "abort"` y sin consola, un pánico moría **mudo**. Ahora hay hook que escribe al log.

## Pendiente de mano del usuario

- El recorrido visual completo (lupa nítida, snap sin sombra, tiradores) sobre uso real.
- Repetir este informe en la **máquina suelo** (i3 3.ª gen, 4 GB) cuando esté disponible.
