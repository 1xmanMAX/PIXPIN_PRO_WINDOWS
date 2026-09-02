# Medición S2-C (pin pro: vídeo y documentos) — equipo de desarrollo — 2026-09-02

**Equipo:** el de siempre (16 GB, 4 núcleos físicos, Intel UHD + MX250, monitor
3000×2000 al 150 %, nivel `Completo`). **Binario:** `target/release/pixpinmax.exe`,
rama `s2c-pin-pro`, modo portable. **Método:** archivos puestos en el portapapeles
con `Set-Clipboard -Path` y `Ctrl+Alt+V` sintetizado; huellas MD5 de una región del
pin separadas 600 ms para detectar movimiento; CPU y RAM con `Get-Process`; tiempos
del log; capturas revisadas. **Vídeo de prueba:** un MKV 1920×1080 de `Videos\`
(grabación de pantalla). **Documento:** un PNG de 3000×2000 por referencia.

**Aviso sobre estas medidas:** durante toda la sesión de pruebas el usuario estaba
usando el equipo (ratón y foco): el log registró un clic físico y 70 movimientos
de ratón 1,1 s después de crear un pin, que lo arrastraron. Los pasos que dependen
del foco del teclado (`Espacio`) y los que comparan huellas quedaron contaminados
en varias vueltas y se marcan como tales.

## Lo verificado

1. **Vídeo por `Ctrl+Alt+V`** → `archivo pineado tipo="video" ms=174–247`; el pin
   nace a 960 px con la proporción de la miniatura de la Shell y **se mueve**
   (huellas distintas a 600 ms) ✓
2. **Doble clic** → pausa: huellas iguales y **94 ms de CPU en 5 s** (vuelta sin
   interferencia) ✓; en otra vuelta 390 ms con el ratón del usuario encima
3. **Menú del clic derecho → Sonido** → `sonido del video alternado silenciado=false` ✓
4. **PNG por referencia** → `tipo="documento"`: miniatura de la Shell con el nombre
   en su franja ✓ (554 ms la primera vez, 99 ms con la miniatura ya en caché)
5. **PDF** → `tipo="ficha"` (758 ms): en este equipo **no hay proveedor de
   miniaturas para PDF**, y D62 lo prevé: cae a ficha sin más
6. **`falso.mp4`** (un texto renombrado) → nace como vídeo, Media Foundation avisa,
   `video no reproducible; se ensena como documento o ficha` y aparece la ficha ✓
7. **Reiniciar con vídeo + documento + ficha** → `restaurados=3 ms=307`; el vídeo
   vuelve reproduciéndose (≈13 % de CPU, fotograma visible en la captura), el
   documento con su miniatura, la ficha como ficha ✓
8. **`Espacio` para reanudar**: sin verificar por automatización (el foco se lo
   llevaba el usuario). El camino de código es el mismo que el del doble clic.

## Rendimiento

| Métrica | Objetivo (spec §5) | Medido |
|---|---|---|
| Vídeo 1080p reproduciéndose, `Completo` | ≤ 15 % de un núcleo | **14–16 %** (1437–1640 ms de CPU en 10 s): en el límite; la decodificación es por hardware y el coste es el repintado a 60 Hz del pin de 960×540 |
| Vídeo en pausa | 0 % | **≈2 %** (94 ms en 5 s): el temporizador se para; queda el motor de Media Foundation en reposo |
| RAM con un vídeo 1080p | ≤ +60 MB | 87–88 MB privados totales con el vídeo (la base con recursos de dibujo ronda 64–85 MB): **≈ +5 a +25 MB** ✓ |
| Documento: pinear → visible | ≤ 300 ms con un PDF de 10 MB | PNG de 3,7 MB: 554 ms en frío, 99 ms en caché. **No cumplido en frío**: la miniatura pasa a un hilo en la siguiente iteración (la spec ya lo preveía) |
| Restauración con vídeo, documento y ficha | < 500 ms | **307 ms** ✓ (era 954 ms hasta dejar de pedir la miniatura del vídeo al restaurar) |

## Lo que se corrigió durante la prueba

- **Restaurar pedía la miniatura del vídeo** solo para saber su proporción, aunque
  el rect ya estaba en el índice: 954 → 307 ms.
- **`Cargo.lock`** no se había añadido a los commits que trajeron `windows-core`
  y `tracing` a `pixpin-pin`.

## Notas honestas

- El 14–16 % de CPU reproduciendo es el repintado de la ventana del pin a cada
  fotograma (60 Hz en `Completo`), no la decodificación. Bajarlo pasa por
  repintar solo cuando `OnVideoStreamTick` da fotograma (ya es así) y por el tope
  de 30 fps en `Ligero`, que se aplica al temporizador pero no se midió aquí.
- La miniatura de la Shell se pide en el hilo de interfaz (D62 lo decía):
  554 ms de bloqueo la primera vez. Es la primera mejora pendiente de la fase.
- Sin proveedor de miniaturas de PDF en la máquina de pruebas, el caso
  «documento PDF» no se pudo ver; el caso general (cualquier archivo con
  miniatura) sí, con un PNG.
- El suelo real (i3 3.ª gen, 4 GB) sigue pendiente.
