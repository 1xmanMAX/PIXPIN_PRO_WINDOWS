# PixPin Max — Plan maestro de ejecución autónoma

> **Protocolo de reanudación** (para cualquier sesión que retome esto):
> 1. Lee este fichero y `git log --oneline -10`.
> 2. La fase en curso es la primera sin `[x]`. Abre su plan detallado (columna
>    «Plan») y continúa en la primera tarea sin marcar.
> 3. Disciplina invariable: puerta estándar por tarea (fmt, clippy contando
>    errores con `grep -cE "^error"`, `cargo test --workspace -- --test-threads=1`,
>    suite `--ignored` cuando la tarea toca escritorio/GPU) + commit por tarea;
>    por fase: PR → CI verde → merge a `main`.
> 4. **Autorización vigente:** el 2026-09-01 el usuario ordenó ejecutar todo el
>    roadmap de principio a fin y aprobar en su nombre la opción recomendada
>    cuando algo pida aprobación. Las decisiones así tomadas se anotan en el
>    plan de la fase con la etiqueta «(auto-aprobada: recomendada)».
> 5. Restricción innegociable: QuickView es GPL-3.0 — técnicas sí, código jamás.
>    Excalidraw es MIT — su código sí puede leerse y adaptarse.

**Objetivo:** completar PixPin Max desde el estado actual (v0.2 a medias) hasta
v1.0 siguiendo el roadmap del documento maestro
(`docs/superpowers/specs/2026-08-09-pixpin-pc-master-design.md`, sección 3, con
el reorden D20).

**Máquina suelo normativa:** i3 3.ª gen, 4 GB, HD 4000 — baseline `x86-64`,
niveles Completo/Ligero de `pixpin-nivel`. Los tres intocables: atajo→overlay
< 50 ms; latencia ≤ 1 fotograma real; bytes guardados idénticos entre niveles.

## Fases

| Estado | Fase | Entrega | Spec | Plan |
|---|---|---|---|---|
| [x] | S1-A cimientos | bandeja, ajustes, idiomas, atajos | 2026-08-09-s1 | 2026-08-09-s1a-cimientos.md |
| [x] | S1-B1 captura | Ctrl+Alt+X → PNG + portapapeles | 2026-08-09-s1 | 2026-08-09-s1b1-captura.md |
| [x] | S1-B2 overlay | overlay < 50 ms, lupa, imán, vivo | 2026-08-09-s1 | 2026-09-01-s1b2-overlay.md |
| [x] | S2-A almacén + pin imagen | Ctrl+Alt+F → pin 1:1 persistente | 2026-09-01-s2 | 2026-09-02-s2a-almacen-pin.md |
| [x] | S2-B tipos y grupos | nota, ficha, Ctrl+Alt+V, menú, grupos, imán | 2026-09-01-s2 §10 | 2026-09-02-s2b-tipos-grupos.md |
| [x] | S2-C pin pro | rueda=zoom (S3-B), lupa y foco en el pin (S3-C), **vídeo con Media Foundation y documentos con miniatura de la Shell** | 2026-09-02-s2c-pin-pro-design.md (D62–D72) | 2026-09-02-s2c-pin-pro.md |
| [ ] | S1-B3 scroll + cuentagotas | Ctrl+Alt+S y Ctrl+Alt+D activos | 2026-08-09-s1 | (escribir al llegar) |
| [x] | S3-A motor 2D | `pixpin-motor2d`: elementos, trazo a mano, rugosidad determinista, texto, hit-test, `.pixpin2d` | (escribir spec desde `docs/investigacion/2026-09-02-excalidraw-analisis.md`) | (escribir al llegar) |
| [x] | S3-B anotar el pin | doble clic → editar dentro del pin: lápiz, resaltador, líneas, formas, texto, imágenes | (spec S3) | (escribir al llegar) |
| [x] | S3-C anotar la pantalla | dos modos: congelada (captura estática) y **capa viva** sobre la pantalla en movimiento, con paso de clics conmutable; foco, lupa, texto in situ, paleta del pin | 2026-09-02-s3bc-anotacion-design.md (D56–D61) | 2026-09-02-s3c-anotar-pantalla.md |
| [ ] | S3-D ajustes visuales | ventana de configuración: atajos reasignables, tema, carpetas, rendimiento, arranque | (spec S3) | (escribir al llegar) |
| [ ] | S3-E chat | feed tipo mensajería que organiza el almacén | (spec S3) | (escribir al llegar) |
| [ ] | S4 salidas + automatización | guardado configurable, OLE drag-out | maestro §S4 | (escribir al llegar) |
| [ ] | S5 OCR + traducción + grabación | lo que no existía en Android | maestro §S5 | (escribir al llegar) |
| [ ] | S6 visor pro + PDF | v1.0 | maestro §S6 | (escribir al llegar) |

## Alcance ampliado por el usuario (2026-09-02)

Orden literal del usuario, desglosada en las fases de arriba para que nada se pierda:

1. **Pines de todo tipo**: imágenes, vídeos, texto, archivos, carpetas y documentos (S2-B lo básico, S2-C vídeo y documentos).
2. **Ventana de ajustes visual** — reasignar atajos y todo lo demás sin editar el TOML a mano (S3-D).
3. **Rueda del ratón = zoom** sobre el pin: agrandar y achicar (S2-C).
4. **Anotar dentro del pin**: doble clic entra en modo edición y se dibuja encima con lápiz, resaltador y líneas (S3-B).
5. **Foco y lupa** como funciones de la caja de herramientas (S2-C para la lupa del pin; el foco vive con las herramientas de S3).
6. **Anotar sobre la pantalla en cualquier momento**, con **dos modos** (S3-C):
   - **Congelada**: captura la pantalla entera y se dibuja sobre esa imagen estática.
   - **Capa viva**: una capa transparente encima de la pantalla que sigue moviéndose debajo; se entra y se sale de la capa, y con ella inactiva los clics pasan a la aplicación de abajo.
7. **Motor de edición avanzada 2D propio** (`pixpin-motor2d`, S3-A): herramientas de trazo analizadas e importadas de **Excalidraw** (MIT — se lee y se adapta, no se copia una extensión ajena). El análisis del original vive en `docs/investigacion/2026-09-02-excalidraw-analisis.md`; el motor es implementación nueva en Rust sobre Direct2D, compartida por el pin (S3-B), la pantalla (S3-C) y el PDF (S6).

Cada fase sin spec propia pasa antes por diseño (secciones del maestro +
decisiones nuevas numeradas), con las elecciones recomendadas auto-aprobadas y
anotadas. Cada fase produce software usable y fusionado antes de abrir la
siguiente.

## Estado del vigilante

- Monitor de sesión (latido cada 2 h) armado el 2026-09-02. Vive mientras viva
  la sesión; si la sesión muere del todo, este fichero ES el vigilante: el
  protocolo de arriba reanuda desde cualquier punto con una sola orden.
