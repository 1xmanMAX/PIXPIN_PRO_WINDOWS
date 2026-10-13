# Para Android: lo que cambió en el PC (11 al 13-oct-2026)

Punto de entrada único para quien trabaje en PixPin Android
(`1xmanMAX/PIXPIN_PRO_ANDROID`). Todo está en `main` de
`1xmanMAX/PIXPIN_PRO_WINDOWS` (6fb6b4f y siguientes). Cada punto remite a su
guía detallada en esta misma carpeta.

## Qué hay que hacer en Android (por orden)

1. **Guardar el formato nuevo de las celdas** (A1 de
   `2026-10-12-que-se-sincroniza.md`). El PC añade a `EstiloDeCelda` las claves
   `k` (cursiva), `s` (subrayado), `t` (tachado), `c` (color de letra), `w`
   (ajustar texto) y `nf` (`numero` | `moneda` | `porcentaje` | `fecha`), junto
   a las de siempre (`n`, `a`, `f`, `e`). Android ya las ignora al leer
   (`ignoreUnknownKeys`), pero **las pierde al volver a guardar**: hay que
   añadirlas con esos mismos nombres, pintarlas y, si se puede, ofrecer los
   mismos botones (negrita, cursiva, subrayado, tachado, resaltar, color,
   alinear, formato de número, ajustar, borrar formato).
2. **Abrir la mini-app `tabla` creada en el PC** (A2): el `texto` del mensaje es
   exactamente el JSON de `TablaDeCalculo`. Hoy Android lo enseña como texto.
3. **Anotaciones de los planos CAD** (`2026-10-12-anotaciones-cad-android.md`):
   el PC ya usa los mismos ficheros que Android (`anot-<uid>.excalidraw.gz` +
   `anot-<uid>.hoja`, unidad = letra más común × 1,4 / 20 como v0.115) y guarda
   cotas y marcos en los grupos `plano-cota` / `plano-marco`. En Android:
   que `CapaDelPlano.con` junte también **por forma** (no solo por id) para no
   borrar y recrear cotas llegadas del PC, y un orden explícito de los marcos.
4. **Fórmulas** (`2026-10-11-formulas-android.md`): el motor del PC tiene ~90
   funciones con su tabla de pruebas; copiarlo para que una hoja calcule igual
   en los dos aparatos.
5. **Hojas editadas de un Excel del chat** (A3): hoy Android usa
   `tablas/libro-<msg>-<i>.json` y el PC mensajes mini-app con
   `referencia = libro-<msg>-<i>`; ninguno ve las ediciones del otro.
   **Pendiente de decisión del usuario.** Recomendación: que Android lea
   también los mensajes del PC. No implementar hasta que el usuario elija.
6. **A4** (logo de proyecto, pines, ajustes): necesitan diseño antes de tocar
   nada. Ojo: Android reescribe el proyecto sin los campos que no conoce, así
   que el PC no puede añadirle campos hasta que Android los conserve.

## Lo que el PC ya cambió y Android ya hacía (no hay que tocar nada)

- Archivar un proyecto nacido en el PC ahora viaja (marca `archivado`).
- Un adjunto borrado en un aparato se borra también del disco del otro
  (`adjuntosFuera`); el PC solo borra lo de `guardados/` que nadie más usa.
- Las tablas del móvil (`TABLA`, `tablas/<ref>.json`) se abren y editan en el
  PC y vuelven con el mismo JSON, conservando lo que el PC no conoce.
- Word en modo texto: imágenes dentro de celdas de tabla contadas como en
  Android (antes el PC desfasaba las siguientes).
- Modo oscuro de los visores: el PDF usa la misma matriz que «Invertir
  colores» (`MATRIZ_INVERTIDA`). Es una preferencia de cada aparato; no viaja.

## Cómo comprobarlo

Las pruebas del PC con un «móvil» simulado que habla el protocolo de verdad
están en `crates/pixpin-proyecto/tests/sincronizar_con_el_movil.rs` y
`apps/pixpin/src/plano_cad/anotar/pruebas_con_el_movil.rs`: describen, caso a
caso, qué espera recibir y mandar cada lado.
