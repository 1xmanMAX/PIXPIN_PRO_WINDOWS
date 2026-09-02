# Medición S2-A (almacén + pin de imagen) — equipo de desarrollo — 2026-09-02

**Equipo:** el mismo de los informes anteriores (16 GB, 4 núcleos físicos, GPU integrada
128 MB VRAM, monitor 3000×2000 al 150 %). **Binario:** `target/release/pixpinmax.exe`,
rama `s2a-pines`, modo portable en carpeta de pruebas. **Método:** entrada sintetizada
(`keybd_event`/`mouse_event`), tiempos del log (`pin creado`, `pines restaurados al
arrancar`), estado en `indice.json`, RAM/CPU con `Get-Process`, ventanas por
`FindWindowExW("PixPinPin")`.

## Las puertas de la spec §7 aplicables a S2-A

| Métrica | Objetivo | Medido |
|---|---|---|
| CPU con 3 pines quietos | 0 % | **0,00 % — 0 ms de CPU en 10 s** ✓ (composición: nada que hacer en reposo) |
| RAM privada con 3 pines ~450×300 | anotar (el tope de 100/60 MB se audita con 10 pines en S2-B) | **53,3 MB privados** (84 MB de working set) con los recursos persistentes del overlay ya creados (dispositivo, duplicadores, ventanas) |
| Primer pin restaurado visible | < 200 ms | **150 ms los TRES pines** (Completo); 119 ms en Ligero. La línea del log cubre abrir almacén + crear dispositivo y motor + cargar 3 PNG + crear 3 ventanas ✓ |
| Mover un pin | sin repintados | verificado por inspección: `EfectoPin::Mover` es solo `SetWindowPos` con `SWP_NOSIZE` — `pintar` no aparece en esa ruta; la composición desplaza el visual entero. Solo `Redimensionar` repinta, obligado porque la superficie cambia de tamaño |

## El flujo completo, comprobado punto a punto (plan Task 8, Step 1)

1. **Ctrl+Alt+F → arrastre → Enter**: pin flotante 1:1 exactamente en la región
   (drag ×1,5 de DPI = 450×300 físicos en (300,300); `indice.json` guarda ese rect
   exacto). El foco vuelve a la aplicación anterior (Chrome en la prueba) ✓.
2. **Mover agarrando el cuerpo**: se desplaza pegado al ratón y el rect se persiste
   **al soltar** (write-on-release) ✓.
3. **Esquina proporcional**: 1451×1088 → 1082×811 (proporción 1,334 conservada al
   redondeo) con la esquina opuesta clavada en el mismo píxel (SE fijo en 3050,2416) ✓.
4. **Doble clic**: alterna al tamaño nativo de la imagen (600×450) y de vuelta ✓.
5. **Clic + Esc**: cierra el pin enfocado; la entrada **queda** en el almacén con
   `pin: null` (D21: cerrar nunca borra) ✓.
6. **Matar el proceso y relanzar**: los 3 pines reaparecen donde estaban
   (`restaurados=3 ms=150`), verificados por ventana y por captura de pantalla ✓.
7. **Almacén navegable**: `almacen/objetos/2026/09/00000N.png` abre con cualquier
   visor; `indice.json` legible a mano ✓.
8. **`nivel = "ligero"` forzado**: restauración (119 ms) y creación de pin funcionan
   igual; el nivel no cambia el pin en S2-A, como manda el diseño ✓.

## Notas honestas

- El overlay caliente siguió en **22-25 ms** con pines vivos; tras un reinicio el
  primer atajo pagó 79 ms (camino frío que 5.3 autoriza una vez).
- Las medidas se tomaron con la máquina **en uso real** (el usuario navegando):
  dos pines de prueba tempranos se cerraron con Esc desde fuera del guion, lo que
  de paso confirmó el cierre por Esc y el `pin: null` sin pérdida de datos.
- La restauración es secuencial (decisión del plan): con 150 ms para 3 pines no hay
  motivo para paralelizar todavía; se revisará con 10 pines en la auditoría de S2-B.
- Pendiente de ojo humano: la calidad visual de la sombra y el redondeo (el plan de
  S2-A dibuja la imagen sin recorte redondeado a conciencia; queda para S2-B).
- El suelo real (i3 3.ª gen, 4 GB) sigue pendiente de tener la máquina delante.
