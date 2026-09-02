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
| [ ] | **S2-B tipos y grupos** | nota, ficha, Ctrl+Alt+V, menú, grupos, imán | 2026-09-01-s2 §10 | 2026-09-02-s2b-tipos-grupos.md |
| [ ] | S1-B3 scroll + cuentagotas | Ctrl+Alt+S y Ctrl+Alt+D activos | 2026-08-09-s1 | (escribir al llegar) |
| [ ] | S3 anotación + canvas + chat | canvas Excalidraw nativo + feed del almacén | (escribir spec) | (escribir al llegar) |
| [ ] | S4 salidas + automatización | guardado configurable, OLE drag-out | maestro §S4 | (escribir al llegar) |
| [ ] | S5 OCR + traducción + grabación | lo que no existía en Android | maestro §S5 | (escribir al llegar) |
| [ ] | S6 visor pro + PDF | v1.0 | maestro §S6 | (escribir al llegar) |

Cada fase sin spec propia pasa antes por diseño (secciones del maestro +
decisiones nuevas numeradas), con las elecciones recomendadas auto-aprobadas y
anotadas. Cada fase produce software usable y fusionado antes de abrir la
siguiente.

## Estado del vigilante

- Monitor de sesión (latido cada 2 h) armado el 2026-09-02. Vive mientras viva
  la sesión; si la sesión muere del todo, este fichero ES el vigilante: el
  protocolo de arriba reanuda desde cualquier punto con una sola orden.
