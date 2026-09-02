# S1-B3 · Captura con scroll y cuentagotas

**Fecha:** 2026-09-02 · **Estado:** aprobado (decisiones auto-aprobadas bajo la
autorización del usuario del 2026-09-01) · **Documento padre:**
[`2026-08-09-s1-cimientos-captura-design.md`](2026-08-09-s1-cimientos-captura-design.md)
§4 (captura con scroll) y §5.1 (atajos `Ctrl+Alt+S` y `Ctrl+Alt+D`).

Los dos atajos se registran desde S1-A y hoy solo dejan «atajo pulsado, todavía
sin accion» en el log. Esta subfase los activa.

## 1. Decisiones

| # | Decisión | Elección | Razón |
|---|---|---|---|
| D73 | **Dónde vive la matemática del cosido** | `pixpin-codec::cosido`, puro: firmas de fila, encaje con rechazo de ambigüedad, franjas fijas, plan y cosedor. Es el puerto de `ScrollMatcher`/`ScrollPlan`/`ScrollStitcher` del Android, con sus mismas constantes (`TAIL_ROWS` 48, `SAMPLE_STEP` 5, `TOLERANCE` 40, `MIN_VARIATION` 300) | `ImagenRgba` vive en el códec; el algoritmo solo mira números por fila y se prueba en CI con páginas sintéticas, como las 9 pruebas del Android |
| D74 | **Pies y cabeceras fijos** | Además del Android: entre dos fotogramas consecutivos que sí se han desplazado, las filas idénticas por arriba son cabecera y por abajo pie (tope: 40 % del alto). La cabecera entra una vez (con el primer fotograma); el pie se excluye de cada tira y se añade una sola vez al final | El Android cosía el pie en cada paso; la spec de S1 §4 lo llama por su nombre y exige resolverlo en el diseño |
| D75 | **Cómo se hace scroll** | Tras confirmar la región, el overlay se **oculta**, el cursor va al centro de la región y se envía la rueda (`SendInput`, 3 muescas) a la ventana de debajo; se espera a que dos capturas seguidas sean idénticas (asentado, ≤ 1 s) y se captura la región desde el duplicador o WGC | Windows entrega la rueda a la ventana bajo el cursor sin activarla (ajuste por defecto desde Windows 10). Ocultar el overlay es lo que deja ver la ventana real |
| D76 | **Cuándo parar** | Tres pasos seguidos sin contenido nuevo, 20 000 px de alto, 30 s de reloj, o `Esc` (sondeado con `GetAsyncKeyState` porque el overlay está oculto) | Los tres de la spec más un tope de tiempo: una página que cambia sola (un reloj, un vídeo) nunca daría «sin contenido nuevo» |
| D77 | **Qué pasa con el resultado** | La imagen cosida **se copia al portapapeles y se queda como pin** centrado (80 % del área si no cabe). Desde el pin: guardar como, anotar, copiar | La spec de S1 decía «igual que `Ctrl+Alt+X`, con barra», pero la barra vive dentro del overlay con la pantalla congelada y una imagen de 20 000 px no cabe ahí. El pin ya existe (S2) y es mejor barra: se ve entera, se anota y se guarda |
| D78 | **Cuentagotas** | `Ctrl+Alt+D` abre el mismo overlay en modo cuentagotas: sin recuadro de selección, con la lupa y el color; un clic (o `Enter`) copia el color bajo el cursor en el formato configurado (`formato_color` del TOML) y cierra; `Esc` cancela | Es la lupa de S1-B2 con una salida distinta: mismo código, dos funciones, como decía la spec |

## 2. Arquitectura

```
pixpin-codec (L1)   cosido.rs   firmas · encontrar_desplazamiento · es_lisa · franjas_fijas
                                Plan { plan(firmas, filas) -> Orden } · Cosedor { anadir(marco) · terminar() }
pixpin-shell (L1)   entrada.rs  rueda_en(punto, muescas) · escape_pulsado()
apps/pixpin         scroll.rs   ejecutar_scroll(recursos, region) -> Option<ImagenRgba>
                    overlay.rs  ModoConfirmacion::{Scroll, Cuentagotas}; AccionFinal::Scroll { region }
                    main.rs     ID_SCROLL → overlay Scroll → ejecutar_scroll → copiar + pinear
                                ID_CUENTAGOTAS → overlay Cuentagotas → copiar texto del color
```

## 3. Pruebas

- **CI (puras):** las 9 del Android portadas (encaje, ruido, banda lisa, patrón
  repetido, contenido distinto, marco corto, firma de fila) más las nuevas:
  página sintética con textura recorrida en pasos de 120 px → la imagen cosida
  es igual a la página; con cabecera y pie fijos → aparecen una sola vez;
  parada por tres «sin movimiento».
- **E2E:** una ventana del Bloc de notas con 400 líneas numeradas; `Ctrl+Alt+S`,
  seleccionar el área de texto, `Enter` → pin con la página cosida y las líneas
  consecutivas sin repetir ni saltar. `Ctrl+Alt+D` sobre un color conocido →
  el portapapeles contiene `#RRGGBB` (o el formato configurado).

## 4. Criterios de aceptación

- [ ] `Ctrl+Alt+S` cose una página del Bloc de notas sin filas repetidas ni saltadas y la deja como pin y en el portapapeles
- [ ] Se para sola al llegar al final; `Esc` la para a mano
- [ ] `Ctrl+Alt+D` copia el color en el formato configurado
- [ ] Las pruebas puras del cosido corren en CI
