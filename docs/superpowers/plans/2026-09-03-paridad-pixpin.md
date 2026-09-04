# Paridad con PixPin (solo lo local) y mejoras — Plan de implementación

> **Para quien ejecute:** las fases van en orden. Cada tarea acaba con
> `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
> y `cargo test --workspace -- --test-threads=1` en verde, y se fusiona antes
> de abrir la siguiente. Los pasos llevan casilla para ir marcándolos.

**Objetivo:** tener todas las funciones **locales** del PixPin original,
mejor implementadas, sin nada de cuenta, licencia, red, actualizaciones ni
telemetría.

**Guías:** `docs/investigacion/2026-09-03-pixpin-original-estructura.md` (qué
tiene el original y sobre qué está construido) y
`docs/investigacion/2026-09-03-quickview-y-excalidraw-guia.md` (cómo se hace
que no dé tirones y cómo se anota bien).

**Restricciones de todo el plan**

- Rust, licencia MIT. `cargo-deny` rechaza GPL y AGPL: nada de FFmpeg, x264
  ni código de QuickView, Excalidraw-plugin o PixPin.
- Nada sale del equipo. Sin cuenta, sin red, sin telemetría.
- Capas: L0 `nivel`/`geom`, L1 `shell`/`render`/`motor2d`/`codec`,
  L2 `capture`/`pin`/`store`, L3 `ui`. `apps/pixpin` y `pixpin-ui` son
  `forbid(unsafe_code)`. Lo hace cumplir `apps/pixpin/tests/capas.rs`.
- Equipo suelo: Core i3 de 3.ª generación, 4 GB, gráficos integrados.
  Baseline `x86-64`, sin AVX2.
- Toda máquina de estado va pura y con pruebas en CI; el Win32 se cablea
  aparte y se verifica a mano sobre el binario.

---

## Mapa: qué tiene el original y qué tenemos nosotros

Los 27 comandos públicos del original, más lo observado. **Fuera de alcance
por decisión del usuario:** cuenta, licencia, red, actualizaciones,
telemetría y el tutorial incrustado.

| Función del original | Estado | Fase |
|---|---|---|
| Capturar región con barra | tenemos | — |
| Capturar y copiar directo | tenemos | — |
| Captura larga con scroll | tenemos | — |
| Cuentagotas | tenemos | — |
| Región bajo el cursor | tenemos (UI Automation) | — |
| Pinear recorte / portapapeles | tenemos | — |
| Grupos de pines y cambiar de grupo | tenemos | — |
| Anotar (pin y pantalla) | tenemos | — |
| Vídeo y documentos pineados | tenemos | — |
| **Registro de comandos y atajos reasignables** | no | **P0** |
| Restaurar el último pin cerrado | no | P1 |
| Cerrar todos los pines | no | P1 |
| Ocultar y mostrar todos los pines | parcial (por grupo) | P1 |
| Paso de clics en un pin | parcial (solo capa viva) | P1 |
| Poner encima la ventana bajo el ratón | no | P1 |
| Pinear lo seleccionado en el Explorador | no | P1 |
| Silenciar los atajos un rato | no | P1 |
| Lista de programas a ignorar | no | P1 |
| Captura con retardo | no | P2 |
| Captura directa sin overlay a región fija | no | P2 |
| Regiones guardadas | no | P2 |
| Capturar y entrar directo a anotar | no | P2 |
| Teclas 100 %, ajustar, rellenar | no | P3 |
| Rotar y voltear | no | P3 |
| Zoom con la ventana bloqueada | no | P3 |
| Reconocer texto (OCR) | no | P4 |
| Reconocer párrafos | no | P4 |
| Fórmula a LaTeX | no | aplazado |
| Traducir el portapapeles | **fuera**: necesita red | — |
| Grabar GIF | no | P5 |
| Ventana de ajustes | no | P6 |
| Menú contextual del Explorador | no | P7 |
| Avisos en pantalla | no | P7 |

---

## Fase P0 — Los cimientos (desbloquea todo lo demás)

### Tarea P0.1 · Registro de comandos

Hoy cada atajo es una constante y una rama del `match` del bucle principal.
Añadir una función cuesta tocar cuatro sitios. El original lo resuelve con un
registro: cada acción es una fila con nombre estable, título traducido, atajo
opcional y marca de si sale en la bandeja. Copiamos ese **diseño**.

**Ficheros**
- Crear: `crates/pixpin-store/src/comandos.rs`
- Modificar: `crates/pixpin-store/src/lib.rs`, `ajustes.rs`
- Modificar: `apps/pixpin/src/main.rs` (registro de atajos y menú de bandeja)
- Modificar: `crates/pixpin-store/i18n/*/main.ftl`

**Interfaz que produce**
- `Comando` (enum de nombres estables) y `Descriptor { comando, nombre, clave_titulo, atajo_por_defecto, en_bandeja }`
- `CATALOGO: &[Descriptor]`
- `Enlaces` con `atajo_de(Comando) -> Option<Atajo>` y `de_ajustes(...)`
- El TOML gana `[comandos]` con `nombre = "Ctrl+Alt+X"`; `""` = sin atajo.

**Pasos**
- [ ] Prueba: el catálogo cubre todos los `Comando`, sin nombres repetidos ni atajos por defecto repetidos.
- [ ] Prueba: ida y vuelta por TOML; un nombre desconocido se ignora sin romper; `""` deja el comando sin atajo.
- [ ] Prueba: migración — un TOML con la tabla vieja `[atajos]` sigue funcionando y produce los mismos enlaces.
- [ ] Implementar `comandos.rs` hasta que pasen.
- [ ] Cablear `main.rs`: registrar los atajos desde `Enlaces`, y generar el menú de bandeja desde `en_bandeja`.
- [ ] Puerta completa y fusión.

**Aceptación:** los atajos actuales siguen funcionando igual, el TOML viejo
se sigue leyendo, y añadir un comando nuevo es añadir una fila.

### Tarea P0.2 · Núcleo del zoom de QuickView

De `docs/investigacion/2026-09-03-quickview-y-excalidraw-guia.md` §1.

**Ficheros**
- Crear: `crates/pixpin-pin/src/zoom.rs` (puro, con pruebas)
- Modificar: `crates/pixpin-pin/src/ventana.rs`, `crates/pixpin-render/src/superficie.rs`
- Modificar: `apps/pixpin/src/pines.rs`

**Pasos**
- [x] Prueba: escalar anclado a un punto deja ese punto quieto.
- [x] Prueba: el controlador exponencial converge y es independiente de los fotogramas (mismo resultado con pasos de 8 ms y de 33 ms, dentro de tolerancia).
- [x] Prueba: el paso se acota entre 1/240 s y 50 ms.
- [x] Implementar el controlador puro (`pixpin-pin::zoom`).
- [x] Sustituir `Animacion` por el controlador; el destino se actualiza sin reiniciar.
- [x] La rueda pasa a anclar en el cursor (`WM_MOUSEWHEEL` trae coordenadas de pantalla).
- [x] Bandera de interacción con antirrebote de 150 ms: mientras dura, textura estirada y filtro barato; al parar, un repintado nítido.
- [ ] Filtro por escala: vecino más cercano en 1:1 exacto y en ampliación grande de imagen pequeña.
- [ ] Puerta y fusión.

**Aceptación:** al girar la rueda el punto bajo el cursor no se mueve, y el
zoom encadenado no da saltos ni en el equipo suelo.

---

## Fase P1 — Comandos de pines que faltan

Todos son filas nuevas del registro de P0.1 más una acción pequeña.

- [x] **P1.1 Restaurar el último pin cerrado.** El almacén ya conserva la entrada al cerrar (borrado lógico). Pila de cerrados en `Pines`; el comando reabre el último con su rect y su grupo. Prueba pura de la pila (tope, vaciado, no revive lo eliminado del almacén).
- [x] **P1.2 Cerrar todos los pines** y **P1.3 ocultar y mostrar todos**, sobre lo que ya existe por grupos.
- [ ] **P1.4 Paso de clics en un pin.** `WS_EX_TRANSPARENT | WS_EX_LAYERED` sobre la ventana del pin, igual que en la capa viva. Alterna por comando y por el menú del pin.
- [x] **P1.5 Poner encima la ventana bajo el ratón.** `WindowFromPoint` + `SetWindowPos(HWND_TOPMOST)`, con aviso de qué ventana se fijó. Vive en `pixpin-shell`.
- [ ] **P1.6 Pinear lo seleccionado en el Explorador.** Por automatización de la Shell, sin tocar el portapapeles del usuario.
- [ ] **P1.7 Silenciar los atajos un rato.** Estado en el bucle; los atajos globales se desregistran y se vuelven a registrar. Icono de bandeja distinto mientras dure.
- [ ] **P1.8 Lista de programas a ignorar.** En el TOML; si la ventana en primer plano es de esa lista, los atajos no actúan.

## Fase P2 — Capturas que faltan

- [ ] **P2.1 Captura con retardo** (3 s por defecto, configurable), con cuenta atrás en la bandeja.
- [ ] **P2.2 Captura directa sin overlay** a la última región usada.
- [ ] **P2.3 Regiones guardadas** con nombre en el TOML, y comando por región.
- [ ] **P2.4 Capturar y entrar directo a anotar**, saltándose la barra.

## Fase P3 — Zoom y vista del pin (lo que eligió el usuario)

- [x] **P3.1 Teclas de zoom**: 100 % exacto (escala clavada en 1.0 y filtro nítido), ajustar al monitor, rellenar.
- [x] **P3.2 Rotar 90° a izquierda y derecha, voltear en horizontal y vertical.** El `Elemento` ya tiene `angulo`; para el pin es una transformada en el pintado y un campo persistido.
- [ ] **P3.3 Arrastre vertical con el botón derecho = zoom.** Un clic derecho sin arrastrar sigue abriendo el menú.
- [x] **P3.4 `Ctrl` + rueda: zoom con la ventana bloqueada** y desplazamiento del contenido dentro. Es lo que el usuario pidió para las notas.

## Fase P4 — Reconocimiento de texto, sin distribuir modelos

El original trae 34 MB de modelos cifrados. Nosotros usamos
**`Windows.Media.Ocr`**, que ya viene en Windows 10 y 11, funciona sin
conexión y no añade un solo byte al ejecutable.

- [ ] **P4.1 Envoltorio** `pixpin-shell::ocr`, con la lista de idiomas instalados y degradación limpia si no hay ninguno.
- [ ] **P4.2 Comando «reconocer texto»** sobre un pin o sobre una selección: copia el texto al portapapeles.
- [x] **P4.3 Agrupar en párrafos** por geometría de las cajas devueltas (puro y con pruebas), para que el texto copiado tenga saltos de línea con sentido.

## Fase P5 — Grabar GIF

Sin FFmpeg ni x264: un GIF es paleta de 256 colores más compresión LZW.

- [x] **P5.1 Codificador GIF** en `pixpin-codec` (puro, con pruebas): cuantización por octree o mediana, LZW, fotogramas con retardo y bucle.
- [ ] **P5.2 Grabación de una región** con el bucle de captura que ya existe, tope de tiempo y de tamaño, y aviso de cuánto lleva.
- [ ] **P5.3 Pausa y reanudación**, como el original.

## Fase P6 — Ventana de ajustes (la S3-D del plan maestro)

Ahora es mucho más barata: la lista de comandos y sus atajos sale del
registro de P0.1, no de una lista escrita a mano.

- [ ] **P6.1 Ventana con pestañas**: general, atajos, captura, pines, rendimiento.
- [ ] **P6.2 Reasignar atajos** capturando la combinación, con detección de choques.
- [ ] **P6.3 El resto de ajustes** que hoy solo están en el TOML.

## Fase P7 — Integración con el sistema

- [ ] **P7.1 Avisos en pantalla** propios, sin robar el foco.
- [ ] **P7.2 Menú contextual del Explorador** para pinear un fichero.

---

## Mejoras sobre el original que van dentro del plan

| Dónde | Mejora |
|---|---|
| Dibujo | Zoom por transformada de composición y sombra solo en el anillo (ya hechas, D89–D91). El original va por Qt sobre ANGLE, dos capas de traducción más. |
| Dibujo | Pendiente: repintado por capas sucias y pirámide de teselas para capturas gigantes. |
| Anotación | Selección, modificadores y deshacer real (ya hechos, D92–D96). Pendiente: estilos recordados, bloquear elemento y bloqueo de herramienta. |
| Peso | 2,2 MB contra 150 MB, sin dependencias nativas. |
| OCR | `Windows.Media.Ocr` en vez de 34 MB de modelos. |
| Vídeo | Media Foundation en vez de FFmpeg y x264, evitando la GPL y usando el codificador por hardware. |
| Región | UI Automation en vez de un modelo de visión. |
| Privacidad | Sin cuenta, sin red, sin telemetría y sin informes de fallo remotos. |

## Qué conviene repartir entre varios agentes, y qué no

**Sí, en paralelo.** Piezas puras y autocontenidas, cada una en su fichero
nuevo, que se prueban solas en CI y no tocan el Win32 compartido:

- P5.1 codificador GIF (`pixpin-codec`, ninguna dependencia del resto)
- P4.3 agrupar en párrafos (puro)
- P0.2 el controlador de zoom, la parte pura
- Investigaciones y revisiones de una rama ya terminada

**No, en secuencia.** Todo lo que toca `ventana.rs`, `main.rs` o `pines.rs`,
que son el punto de encuentro de casi todo: dos agentes editándolos a la vez
se pisan y el conflicto cuesta más que el trabajo. También va en secuencia
cualquier tarea que cambie el TOML o el catálogo de comandos, porque el
resto se apoya en ellos.

**Regla:** primero se cierra P0.1 en secuencia, porque todas las fases
siguientes añaden filas a ese catálogo. A partir de ahí, las piezas puras se
pueden repartir; el cableado, no.
