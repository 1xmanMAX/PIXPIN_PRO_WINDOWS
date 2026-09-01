# Medición S1-B1 — equipo de desarrollo — 2026-09-01

**Equipo (de los `Hechos` reales del log):** RAM física 15,8 GiB · 4 núcleos físicos / 8 lógicos ·
VRAM dedicada 128 MB (integrada) · GPU hardware · nivel de característica 0xB000 (11_0) ·
monitor principal 3000×2000 físicos al 150 %.
**Binario:** `target/release/pixpinmax.exe`, commit `f2788c0` más el cableado de la Task 10.
**Método:** atajo global sintetizado con `keybd_event` desde PowerShell; tiempos con `date +%s%3N`
alrededor del lanzamiento del script (~200-400 ms de sobrecoste del arnés, anotado).

| Métrica | Objetivo | Medido |
|---|---|---|
| CPU en reposo | 0 % | **0 ms de CPU en 5 s de reposo = 0 %** |
| RAM en reposo | < 40 MB | **31 MB privados** (tras 5 capturas) |
| Atajo → PNG en disco (3000×2000) | < 400 ms | **~700-750 ms netos** (1003 ms brutos − ~260 ms de arnés) — **no cumple**, ver nota |
| Nivel decidido | coherente con el equipo | **`Completo`, sin razones** — coherente (16 GB, 4 núcleos, hardware) |
| `nivel = "ligero"` forzado | `ForzadoPorAjuste` en el log | **`Ligero, [ForzadoPorAjuste]`** ✓ |
| Primer atajo tras arrancar frente al segundo | el precalentamiento los acerca | primero ~1311 ms, segundo ~2220 ms — **el ruido del arnés y del PNG domina; no medible así** |
| PNG correcto | sin inclinación, colores buenos, píxeles físicos | ✓ verificado visualmente; IHDR = 3000×2000 |
| Tres capturas seguidas | sin sobrescribir | ✓ `captura-0001..0003.png` |
| Línea de éxito traducida | sí | ✓ «Captura guardada en …» |
| Modo portable | capturas junto al exe | ✓ `portable/capturas/captura-0001.png` |

## Notas honestas

- **El atajo→PNG no cumple el objetivo de 400 ms.** El andamio de S1-B1 crea un dispositivo D3D11
  **nuevo en cada captura** (~100-200 ms) y abre una sesión WGC desde cero; además el codificador PNG
  de `image` sobre 3000×2000 depende fuertemente del contenido. El overlay de S1-B2 retendrá el
  dispositivo y la instantánea, que es donde este número debe bajar. No se optimiza el andamio:
  se anota y se sigue.
- **El efecto del precalentamiento no es medible con este arnés** (~±500 ms de varianza entre
  lanzamientos de PowerShell y codificaciones PNG). Cuando el dispositivo se retenga (S1-B2), medirlo
  será trivial: será la diferencia entre tener textura o no tenerla.
- **Pendiente de mano del usuario:** pegar en Paint (`Ctrl+V`) y confirmar que la imagen sale del
  derecho. El test automático verificó que `CF_DIB` queda disponible y la inversión de filas tiene
  test propio, pero el ojo sobre Paint sigue siendo la prueba final.
- **Pendiente: repetir este informe en la máquina suelo** (i3 de 3.ª gen, 4 GB, HD 4000) cuando el
  usuario la tenga delante — `medidas/AAAA-MM-DD-maquina-suelo.md`.
