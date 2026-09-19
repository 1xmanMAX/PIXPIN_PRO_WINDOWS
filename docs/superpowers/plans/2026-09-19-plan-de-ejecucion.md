# Plan de ejecución: ponerse al día con PixPin Android y pulir el rendimiento

**Fecha:** 2026-09-19 · **Rama:** `pin-en-vivo` · **Petición del usuario:** ejecutar todo lo pendiente
sin ir preguntando, en paralelo donde sea seguro, con un agente revisor.

## Regla de reparto (para que los agentes no se pisen)

Cada tanda asigna a cada agente un **conjunto cerrado de ficheros**. Dos agentes nunca comparten
fichero. Lo que toca `main.rs`, `ventana_chat.rs` o `ventana_editor.rs` va con dueño único por tanda.

| Zona | Ficheros |
|---|---|
| **Tinta y lienzo** | `crates/pixpin-tinta/*` (nuevo), `crates/pixpin-render/*`, `crates/pixpin-motor2d/*`, `apps/pixpin/src/ventana_editor.rs`, `apps/pixpin/src/editor.rs` |
| **Sincronizar** | `crates/pixpin-sincro/*`, `crates/pixpin-proyecto/*`, `apps/pixpin/src/sincronizar*`, `apps/pixpin/src/recibir.rs` |
| **Chat** | `apps/pixpin/src/ventana_chat.rs`, `crates/pixpin-ui/src/chat.rs` |
| **Universo** | `apps/pixpin/src/universo/*`, `crates/pixpin-universo/*` |
| **Bandeja y arranque** | `apps/pixpin/src/main.rs`, `crates/pixpin-store/src/comandos.rs` |
| **Textos** | `crates/pixpin-store/i18n/*` (varios agentes: solo AÑADIR claves al final) |

## Puerta común (todos, antes de cada commit)

`cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings`;
`cargo test --workspace --no-fail-fast -- --test-threads=1`.
`la_segunda_adquisicion_falla_mientras_viva_la_primera` falla con la app abierta y no cuenta.
Nadie ejecuta la aplicación ni sintetiza ratón o teclado. Nadie compila en `target/entrega`.

---

## Tanda 1 (en marcha)

| # | Trabajo | Zona |
|---|---|---|
| 1.1 | Arnés de medida del editor, A1 (nada de objetos D2D por fotograma), B1/D1 (pintar justo antes del plazo de DWM), A2/A3 (capa de escena en su visual), **motor de tinta aislado** `pixpin-tinta`, C1 suavizado | Tinta y lienzo |
| 1.2 | Recibir: «Actualizar el que tengo / Crear como nuevo»; pantalla de copias de seguridad; atender al móvil con la ventana cerrada (presencia mientras la app vive) | Sincronizar + Bandeja |
| 1.3 | Chat: «Unir al proyecto» / «Volver a añadir», pulsación larga en el título, arrastrar para seleccionar, deslizar para comentar, la lupa filtra burbujas, miniatura de página de PDF y onda de nota de voz | Chat |
| 1.4 | Inventario de todo lo que Android añadió de la v0.52 a la v0.66 y no está en el PC, con plan por valor y esfuerzo | Solo un documento |

## Tanda 2 (tras la 1)

| # | Trabajo | Zona |
|---|---|---|
| 2.1 | Lo que salga del inventario 1.4, por valor: visores (HTML, DOCX, EPUB), «Abrir con» con tres opciones, marcador y engranaje, lectura a gusto, multitarea de lienzos | Según el inventario |
| 2.2 | Universo: organizador manual con subespacios, varios universos, barra del «+» | Universo |
| 2.3 | Lo que el revisor devuelva de la tanda 1 | Donde toque |

## Revisor (tras cada tanda)

Un agente aparte que **no escribe código de producto**: lee los commits de la tanda, los compara
con el Kotlin del móvil y con la puerta común, ejecuta la suite y el lint, busca cosas rotas o a
medias, y devuelve una lista de correcciones ordenada por gravedad. Lo que encuentre entra en la
tanda siguiente.

## Entregas al usuario

Tras cada tanda: compilar desde el worktree limpio `.claude/worktrees/entrega`, sustituir
`target\entrega\release\pixpinmax.exe`, reiniciar la aplicación, abrir por el menú de la bandeja lo
que se pueda (chat 22, recibir 24, sincronizar 26, universo 901) y mandar capturas al usuario con
lo que cambió y qué probar a mano.

## Lo que queda fuera a propósito

- **Restaurar los diez proyectos de la papelera:** los borró el usuario a mano el 19-sep a las
  16:44. Se quedan en `almacen\papelera\` hasta que él lo pida.
- **La física de resortes del universo del móvil:** pelea con miles de archivos que llegan solos
  del chat. Si el usuario la quiere, se estudia aparte con su medición.
- **Controladores propios de Windows para la tinta:** exigen firma, un fallo tumba el equipo y no
  ganan latencia; el equivalente es el motor aislado de 1.1.
