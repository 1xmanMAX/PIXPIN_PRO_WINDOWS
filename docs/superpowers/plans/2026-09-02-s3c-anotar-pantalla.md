# S3-C · Anotar la pantalla — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** que `Ctrl+Alt+A` abra una capa transparente sobre la pantalla viva y `Ctrl+Alt+Shift+A` una sobre la pantalla congelada, las dos con la misma caja de herramientas que el pin, con foco, lupa y texto in situ, y que lo dibujado pueda quedarse como pin; y que el pin gane la caja de herramientas visible, el foco, la lupa y el texto que S3-B dejó pendientes.

**Architecture:** la máquina pura `pixpin-ui::anotador` gana texto in situ; el motor 2D gana la figura `Foco` y la orden `Velo`; `pixpin-render` gana la primitiva de velo con hueco. El ejecutable tiene un único módulo `capa.rs` que sirve para los dos modos (vivo y congelado: solo cambia si hay un bitmap de fondo). El pin recibe una **paleta flotante** (`pixpin-pin::Paleta`, una ventana pequeña sin activación) que el gestor pinta con el mismo código que la caja de la capa, y el pin pinta el velo y la lupa sobre su propio bitmap.

**Tech Stack:** Rust 1.97, crate `windows` 0.62 (Direct2D/DirectComposition/Dwm/Imm), `pixpin-motor2d` puro, `pixpin-ui` puro.

**Spec:** `docs/superpowers/specs/2026-09-02-s3bc-anotacion-design.md` (D46–D55). Decisiones nuevas de esta fase, auto-aprobadas (recomendadas) bajo la autorización del 2026-09-01:

| # | Decisión | Elección |
|---|---|---|
| D56 | Cómo se entra al modo congelado | Un segundo atajo, `Ctrl+Alt+Shift+A` (ajuste `anotar_congelada`). Misma capa, con la captura del monitor como fondo. En congelado el modo pasante no existe (no hay nada vivo debajo). |
| D57 | Texto in situ | Vive en la máquina pura: clic con la herramienta Texto abre un texto en curso, cada carácter lo alarga, `Enter` lo confirma, `Escape` lo cancela, `Retroceso` borra. El IME del sistema compone en su ventana por defecto, colocada junto al punto de escritura. |
| D58 | Caja de herramientas del pin | Una **ventana paleta** aparte (`WS_EX_NOACTIVATE`, sin cromo) colocada por `CajaHerramientas::colocar` junto al pin, viva solo mientras se anota. El pin no la conoce: la crea y la pinta el gestor. |
| D59 | Captura final de la capa | Se captura la pantalla **con la capa visible** pero sin caja ni lupa (se repinta, se espera dos vueltas de composición con `DwmFlush`, se captura). Después se destruye la capa y se pregunta si guardar (D54). |
| D60 | Lupa sobre pantalla viva | Cada movimiento con la lupa activa captura el monitor (duplicador persistente, ≤ 60 muestras/s) y la lupa se coloca **fuera** de su propia región fuente para no ampliarse a sí misma. Sobre congelado y sobre el pin, muestrea el bitmap que ya hay. |
| D61 | Imágenes incrustadas | Fuera de S3-C. No están en los criterios de aceptación de la spec y necesitan un almacén de bitmaps por anotación que llega con el PDF (S6). |

## Global Constraints

- Máquina suelo: i3 3.ª gen, 4 GB, HD 4000. Baseline `x86-64`; sin `target-cpu=native`.
- `apps/pixpin` y `pixpin-ui` llevan `#![forbid(unsafe_code)]`. El Win32 vive en `pixpin-shell` (L1) y `pixpin-pin` (L2).
- Regla de capas (`apps/pixpin/tests/capas.rs`): L1 `shell/render/motor2d/codec`, L2 `capture/pin/store`, L3 `ui`. Un crate solo depende de capas inferiores. `pixpin-pin` NO puede ver `pixpin-ui`.
- `cargo` no está en el PATH: `export PATH="$USERPROFILE/.cargo/bin:$PATH"` (Bash) al principio de cada comando.
- Puerta estándar por tarea: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace -- --test-threads=1`; la suite `--ignored` cuando la tarea toca escritorio/GPU.
- Un commit por tarea, mensaje en español, terminado con `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>` y `Claude-Session: https://claude.ai/code/session_016eoazzUbmckpE1eLfVoX36`.
- Los textos visibles van al catálogo Fluent (`crates/pixpin-store/i18n/{es-ES,en-US}/main.ftl`), nunca literales en el código.
- Comentarios y nombres en español sin tildes en identificadores; los comentarios explican el **porqué** (estilo del repo).
- Rama: `s3c-anotar-pantalla` (ya existe con trabajo sin commit).

---

### Task 1: Puerta y commit de la base de la capa viva

El trabajo sin commit (capa viva, atajo `Ctrl+Alt+A`, modo pasante, rueda) compila. Hay que pasarle la puerta y cerrarlo antes de construir encima.

**Files:**
- Ya modificados: `apps/pixpin/src/main.rs`, `apps/pixpin/src/overlay.rs`, `apps/pixpin/src/capa.rs`, `crates/pixpin-shell/src/{atajos,lib,overlay}.rs`, `crates/pixpin-store/src/ajustes.rs`

- [ ] **Step 1: Puerta**

```bash
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -cE "^error"; cargo test --workspace -- --test-threads=1 2>&1 | grep -E "^test result|FAILED|panicked"
```
Esperado: `0` errores de clippy, todos los `test result: ok`.

- [ ] **Step 2: Suite de escritorio de los tests nuevos**

```bash
cargo test -p pixpin-shell --lib -- --ignored --test-threads=1 pasante
```
Esperado: `el_modo_pasante_se_activa_y_se_quita` y `poner_pasante_no_borra_los_demas_estilos` en `ok`.

- [ ] **Step 3: Commit**

```bash
git add apps/pixpin/src/capa.rs apps/pixpin/src/main.rs apps/pixpin/src/overlay.rs crates/pixpin-shell/src/atajos.rs crates/pixpin-shell/src/lib.rs crates/pixpin-shell/src/overlay.rs crates/pixpin-store/src/ajustes.rs
git commit -m "La capa viva nace: dibujar sobre la pantalla en movimiento y dejar pasar los clics

Ctrl+Alt+A abre una ventana transparente a pantalla completa sobre la
que se dibuja con la misma maquina que el pin. Espacio alterna entre
recoger el raton y dejarlo pasar (WS_EX_TRANSPARENT, D50): el dibujo se
sigue viendo y los clics llegan a la aplicacion de abajo. Al salir con
algo dibujado, lo dibujado se queda como pin.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_016eoazzUbmckpE1eLfVoX36"
```

---

### Task 2: El foco como figura propia del motor 2D

D51: el foco oscurece **todo menos** una zona. Hoy el anotador lo construye como un rectángulo con relleno oscuro, que oscurece justo lo contrario. El motor no conoce el tamaño del lienzo, así que produce el **hueco** y el consumidor pinta el velo alrededor.

**Files:**
- Modify: `crates/pixpin-motor2d/src/elemento.rs` (enum `Figura`, ~línea 42)
- Modify: `crates/pixpin-motor2d/src/pintado.rs` (enum `Orden` ~línea 21, `fn ordenes` ~línea 70)
- Modify: `crates/pixpin-motor2d/src/impacto.rs` (`fn toca`, ~línea 20)

**Interfaces:**
- Produces: `Figura::Foco { elipse: bool }`; `Orden::Velo { hueco: Vec<Punto2>, color: ColorRgba }`. El velo cubre TODO el lienzo del consumidor menos el polígono `hueco`. Tras el velo va una `Orden::Polilinea` con el borde del hueco en `e.trazo`.

- [ ] **Step 1: Tests que fallan (pintado.rs, dentro de `mod pruebas` existente)**

```rust
    fn foco(elipse: bool) -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Foco { elipse },
            x: 10.0,
            y: 20.0,
            ancho: 100.0,
            alto: 50.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(1.0, 1.0, 1.0),
            relleno: Some(ColorRgba { r: 0.0, g: 0.0, b: 0.0, a: 0.6 }),
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 0.0,
            opacidad: 1.0,
            semilla: 7,
            version: 0,
            borrado: false,
        }
    }

    #[test]
    fn el_foco_produce_un_velo_con_hueco_rectangular_y_su_borde() {
        // D51: el motor no sabe cuanto mide el lienzo, asi que entrega el
        // HUECO y el consumidor oscurece todo lo demas.
        let o = ordenes(&foco(false));
        let Orden::Velo { hueco, color } = &o[0] else {
            panic!("la primera orden del foco debe ser el velo, fue {:?}", o[0]);
        };
        assert_eq!(hueco.len(), 4);
        assert_eq!(hueco[0], Punto2::nuevo(10.0, 20.0));
        assert_eq!(hueco[2], Punto2::nuevo(110.0, 70.0));
        assert!((color.a - 0.6).abs() < 1e-6);
        assert!(
            matches!(o[1], Orden::Polilinea { .. }),
            "tras el velo va el borde del hueco"
        );
    }

    #[test]
    fn el_foco_eliptico_tiene_un_hueco_redondo() {
        let o = ordenes(&foco(true));
        let Orden::Velo { hueco, .. } = &o[0] else {
            panic!("velo esperado");
        };
        // Una elipse lisa tiene muchos mas vertices que un rectangulo.
        assert!(hueco.len() > 16, "hueco con {} puntos", hueco.len());
    }

    #[test]
    fn el_foco_sin_relleno_oscurece_al_sesenta_por_ciento() {
        // Caso negativo: un fichero antiguo o un consumidor descuidado que
        // no ponga relleno no puede dejar el velo transparente.
        let mut e = foco(false);
        e.relleno = None;
        let Orden::Velo { color, .. } = &ordenes(&e)[0] else {
            panic!("velo esperado");
        };
        assert!((color.a - 0.6).abs() < 1e-6);
    }
```

Y en `impacto.rs`, en su `mod pruebas`:

```rust
    #[test]
    fn el_foco_se_agarra_por_dentro_del_hueco() {
        // Lo que se ve es el hueco: es lo que el usuario intenta mover.
        let mut e = super::pruebas::elemento_base(); // si no existe un
        // constructor asi en este modulo, construir el Elemento a mano
        // igual que en pintado.rs con figura Figura::Foco { elipse: false },
        // x 10, y 20, ancho 100, alto 50.
        e.figura = Figura::Foco { elipse: false };
        assert!(toca(&e, Punto2::nuevo(50.0, 40.0)));
        assert!(!toca(&e, Punto2::nuevo(500.0, 400.0)));
    }
```

- [ ] **Step 2: Comprobar que fallan**

```bash
cargo test -p pixpin-motor2d 2>&1 | grep -E "^error|no variant" | head -5
```
Esperado: error de compilación `no variant named Foco`.

- [ ] **Step 3: Implementar**

`elemento.rs`, en `Figura`, tras `Elipse`:

```rust
    /// Oscurece todo menos su caja (D51). El motor entrega el hueco; quien
    /// pinta sabe cuanto mide el lienzo y oscurece el resto.
    Foco {
        #[serde(default)]
        elipse: bool,
    },
```

Revisar `Elemento::puntos` y cualquier `match` exhaustivo sobre `Figura` (el compilador los lista): `Foco` va con `Rectangulo`/`Elipse` (sin puntos).

`pintado.rs`, en `Orden`, tras `Relleno`:

```rust
    /// Oscurece TODO el lienzo salvo el poligono `hueco` (D51). El motor
    /// no conoce el tamano del lienzo; el consumidor si.
    Velo {
        hueco: Vec<Punto2>,
        color: ColorRgba,
    },
```

En `ordenes`, brazo nuevo tras `Figura::Elipse`:

```rust
        Figura::Foco { elipse } => {
            // Hueco LISO siempre: un velo tembloroso deja rendijas por las
            // que se cuela el fondo oscurecido.
            let mut lisa = Azar::nuevo(e.semilla);
            let hueco = if *elipse {
                formas::elipse(e.x, e.y, e.ancho, e.alto, 0.0, &mut lisa)
                    .into_iter()
                    .next()
                    .unwrap_or_default()
            } else {
                vec![
                    Punto2::nuevo(e.x, e.y),
                    Punto2::nuevo(e.x + e.ancho, e.y),
                    Punto2::nuevo(e.x + e.ancho, e.y + e.alto),
                    Punto2::nuevo(e.x, e.y + e.alto),
                ]
            };
            let oscuridad = e.relleno.unwrap_or(ColorRgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.6,
            });
            salida.push(Orden::Velo {
                hueco: hueco.clone(),
                color: con_opacidad(oscuridad, e.opacidad),
            });
            // El borde del hueco, cerrado: ayuda a ver donde acaba el foco
            // sobre fondos ya oscuros.
            let mut borde = hueco;
            if let Some(primero) = borde.first().copied() {
                borde.push(primero);
            }
            salida.push(Orden::Polilinea {
                puntos: borde,
                color,
                grosor: e.grosor,
                estilo: EstiloTrazo::Solido,
            });
        }
```

`impacto.rs`, en el `match &e.figura` de `toca`:

```rust
        Figura::Foco { .. } => dentro_de_la_caja(p, e, margen),
```

- [ ] **Step 4: Tests en verde**

```bash
cargo test -p pixpin-motor2d 2>&1 | grep -E "^test result|FAILED"
```
Esperado: `test result: ok` (72 + 4 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/pixpin-motor2d
git commit -m "El foco es una figura del motor: entrega el hueco y el consumidor oscurece el resto

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_016eoazzUbmckpE1eLfVoX36"
```

---

### Task 3: La primitiva de velo con hueco en `pixpin-render`

**Files:**
- Modify: `crates/pixpin-render/src/lienzo.rs` (impl `Pintor`, junto a `poligono`, ~línea 282)

**Interfaces:**
- Produces: `Pintor::velo(&self, marco: RectF, hueco: &[(f32, f32)], color: Color)`. Rellena `marco` con `color` dejando `hueco` sin pintar.

- [ ] **Step 1: Implementar**

Direct2D rellena con regla alternada por defecto: una geometría con dos figuras cerradas (el marco y el hueco) deja el hueco vacío.

```rust
    /// Rellena `marco` dejando sin pintar el poligono `hueco`: es el foco
    /// de D51. Dos figuras cerradas en una misma geometria y la regla de
    /// relleno alternada de Direct2D hacen el agujero sin recortes ni
    /// capas.
    pub fn velo(&self, marco: RectF, hueco: &[(f32, f32)], color: Color) {
        use windows::Win32::Graphics::Direct2D::Common::{
            D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED, D2D1_FILL_MODE_ALTERNATE,
        };
        if hueco.len() < 3 {
            self.rellenar(marco, color);
            return;
        }
        let esquinas = [
            (marco.x, marco.y),
            (marco.x + marco.ancho, marco.y),
            (marco.x + marco.ancho, marco.y + marco.alto),
            (marco.x, marco.y + marco.alto),
        ];
        // SAFETY: igual que `geometria`: crear, rellenar entre Open/Close y
        // descartar si algo falla a mitad.
        let geometria = unsafe {
            let Ok(geometria) = self.motor.fabrica().CreatePathGeometry() else {
                return;
            };
            let Ok(sumidero) = geometria.Open() else {
                return;
            };
            sumidero.SetFillMode(D2D1_FILL_MODE_ALTERNATE);
            for figura in [&esquinas[..], hueco] {
                sumidero.BeginFigure(
                    Vector2 {
                        X: figura[0].0,
                        Y: figura[0].1,
                    },
                    D2D1_FIGURE_BEGIN_FILLED,
                );
                let resto: Vec<Vector2> = figura[1..]
                    .iter()
                    .map(|(x, y)| Vector2 { X: *x, Y: *y })
                    .collect();
                sumidero.AddLines(&resto);
                sumidero.EndFigure(D2D1_FIGURE_END_CLOSED);
            }
            if sumidero.Close().is_err() {
                return;
            }
            geometria
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; geometria y pincel vivos.
            unsafe { self.motor.contexto().FillGeometry(&geometria, &p, None) };
        }
    }
```

Si `Vector2` no está ya importado en `lienzo.rs`, es `windows_numerics::Vector2` (mirar cómo lo importa `geometria`).

- [ ] **Step 2: Compila y clippy**

```bash
cargo clippy -p pixpin-render --all-targets -- -D warnings 2>&1 | grep -cE "^error"
```
Esperado: `0`.

- [ ] **Step 3: Prueba de escritorio**

Si `lienzo.rs` o `superficie.rs` tienen tests `#[ignore]` que pintan sobre una superficie real, añadir uno igual que llame a `p.velo(...)` con un hueco de 4 puntos y compruebe que `dibujar` devuelve `Ok`. Si no existe esa infraestructura, la verificación del velo es visual en Task 14 y se anota aquí.

- [ ] **Step 4: Commit**

```bash
git add crates/pixpin-render
git commit -m "Pintor::velo: rellenar todo menos un hueco con la regla alternada de Direct2D

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_016eoazzUbmckpE1eLfVoX36"
```

---

### Task 4: El anotador construye `Figura::Foco`

**Files:**
- Modify: `crates/pixpin-ui/src/anotador.rs` (`fn construir`, ~línea 300)

- [ ] **Step 1: Test que falla (en `mod pruebas` de anotador.rs)**

```rust
    #[test]
    fn el_foco_es_una_figura_propia_y_no_un_rectangulo_relleno() {
        // D51: si fuera un rectangulo con relleno oscuro oscureceria justo
        // lo que se quiere ensenar.
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Foco));
        a.procesar(EventoAnotador::Pulsar(Punto2::nuevo(10.0, 10.0)));
        a.procesar(EventoAnotador::Mover(Punto2::nuevo(60.0, 40.0)));
        let EfectoAnotador::Terminado(e) = a.procesar(EventoAnotador::Soltar(Punto2::nuevo(60.0, 40.0))) else {
            panic!("un arrastre con el foco termina un elemento");
        };
        assert_eq!(e.figura, Figura::Foco { elipse: false });
        assert_eq!(e.relleno.map(|c| c.a), Some(0.6));
    }
```

Si algún test existente afirma que el foco es `Figura::Rectangulo`, cambiarlo por `Figura::Foco { elipse: false }`.

- [ ] **Step 2: Falla**

```bash
cargo test -p pixpin-ui foco 2>&1 | grep -E "panicked|FAILED|error" | head
```

- [ ] **Step 3: Implementar** — en `construir`, separar el brazo:

```rust
            Herramienta::Rectangulo => Figura::Rectangulo,
            Herramienta::Foco => Figura::Foco { elipse: false },
```

El bloque `(relleno, color, opacidad)` que ya distingue `Foco` se queda igual: el relleno oscuro pasa a ser el color del velo.

- [ ] **Step 4: Verde**

```bash
cargo test -p pixpin-ui 2>&1 | grep -E "^test result|FAILED"
```

- [ ] **Step 5: Commit**

```bash
git add crates/pixpin-ui
git commit -m "El anotador dibuja el foco como figura propia

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_016eoazzUbmckpE1eLfVoX36"
```

---

### Task 5: Pintar el velo en el pin y en la capa

**Files:**
- Modify: `crates/pixpin-pin/src/ventana.rs` (`fn pintar_anotaciones`, ~línea 550)
- Modify: `apps/pixpin/src/capa.rs` (`fn pintar_ordenes`, ~línea 310)

- [ ] **Step 1: Pin** — `pintar_anotaciones` recibe hoy `(p, i, margen)`. Añadir el brazo (el marco es el contenido del pin, desplazado por el margen de sombra):

```rust
            Orden::Velo { hueco, color: c } => {
                let r = i.estado.rect();
                let marco = RectF {
                    x: margen,
                    y: margen,
                    ancho: r.ancho as f32,
                    alto: r.alto as f32,
                };
                let v: Vec<(f32, f32)> = hueco.iter().map(mover).collect();
                p.velo(marco, &v, color(*c));
            }
```

- [ ] **Step 2: Capa** — `pintar_ordenes(p, ordenes)` pasa a `pintar_ordenes(p, ordenes, marco: RectF)` y el llamador le da el monitor entero (`RectF { x: 0.0, y: 0.0, ancho: self.area.ancho as f32, alto: self.area.alto as f32 }`):

```rust
            Orden::Velo { hueco, color: c } => {
                let v: Vec<(f32, f32)> = hueco.iter().map(|q| (q.x, q.y)).collect();
                p.velo(marco, &v, color(*c));
            }
```

- [ ] **Step 3: Puerta y commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -cE "^error"; cargo test --workspace -- --test-threads=1 2>&1 | grep -E "FAILED|^test result: F"
git add crates/pixpin-pin apps/pixpin/src/capa.rs
git commit -m "El pin y la capa pintan el velo del foco

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_016eoazzUbmckpE1eLfVoX36"
```

---

### Task 6: La lupa de anotación: aumento fraccionario y colocación fuera de su fuente

D52/D60. La `Lupa` del overlay tiene factor entero 8. La de anotación va de 1,5× a 8× en pasos de ×1,25, y sobre pantalla viva tiene que colocarse fuera de la región que amplía.

**Files:**
- Modify: `crates/pixpin-ui/src/lupa.rs`
- Modify: `apps/pixpin/src/overlay.rs` (donde use `Lupa { factor: 8, ... }` o `.factor`; el grep de la sesión no encontró usos fuera de `por_defecto`, pero comprobar con `cargo build`)

**Interfaces:**
- Produces: `Lupa { factor: f32, diametro: u32 }`; `Lupa::con_aumento(escala_por_cien: u32, factor: f32) -> Lupa` (diámetro 240 px lógicos); `Lupa::colocar_fuera(&self, cursor: Punto, monitor: Rect) -> Punto` cuya caja `diametro×diametro` no interseca `region_fuente(cursor, monitor)`.

- [ ] **Step 1: Tests que fallan**

```rust
    #[test]
    fn la_lupa_de_anotacion_tiene_aumento_fraccionario() {
        let l = Lupa::con_aumento(100, 2.0);
        assert_eq!(l.diametro, 240);
        let f = l.region_fuente(Punto { x: 500, y: 500 }, Rect { x: 0, y: 0, ancho: 1920, alto: 1080 });
        assert_eq!(f.ancho, 120);
        let l = Lupa::con_aumento(100, 2.5);
        assert_eq!(l.region_fuente(Punto { x: 500, y: 500 }, Rect { x: 0, y: 0, ancho: 1920, alto: 1080 }).ancho, 96);
    }

    #[test]
    fn colocar_fuera_nunca_pisa_la_region_que_amplia() {
        // D60: sobre pantalla viva la lupa muestrea la pantalla CON la lupa
        // dibujada; si se pisara a si misma se ampliaria en bucle.
        let monitor = Rect { x: 0, y: 0, ancho: 1920, alto: 1080 };
        let l = Lupa::con_aumento(100, 1.5);
        for cursor in [
            Punto { x: 0, y: 0 },
            Punto { x: 1919, y: 1079 },
            Punto { x: 960, y: 540 },
            Punto { x: 1919, y: 0 },
            Punto { x: 0, y: 1079 },
            Punto { x: 100, y: 540 },
        ] {
            let fuente = l.region_fuente(cursor, monitor);
            let pos = l.colocar_fuera(cursor, monitor);
            let destino = Rect { x: pos.x, y: pos.y, ancho: l.diametro, alto: l.diametro };
            assert!(
                destino.interseccion(fuente).is_none() || destino.interseccion(fuente).unwrap().esta_vacio(),
                "cursor {cursor:?}: destino {destino:?} pisa fuente {fuente:?}"
            );
            assert!(monitor.contiene(pos), "{pos:?} fuera del monitor");
        }
    }
```

(Si `Rect` no tiene `interseccion`, usar la que exista en `pixpin-geom::rect` — comprobar con `grep -n "pub fn" crates/pixpin-geom/src/rect.rs` — o comparar bordes a mano: `destino.derecha() <= fuente.izquierda() || destino.izquierda() >= fuente.derecha() || destino.abajo() <= fuente.arriba() || destino.arriba() >= fuente.abajo()`.)

- [ ] **Step 2: Fallan** — `cargo test -p pixpin-ui lupa 2>&1 | grep -E "^error" | head -3`.

- [ ] **Step 3: Implementar**

```rust
pub struct Lupa {
    /// Aumento: pixeles dibujados por cada pixel real.
    pub factor: f32,
    pub diametro: u32,
}

impl Lupa {
    pub fn por_defecto(escala_por_cien: u32) -> Lupa {
        Lupa { factor: 8.0, diametro: 176 * escala_por_cien / 100 }
    }

    /// La lupa de anotacion (D52): mas grande y con el aumento que pida
    /// la rueda.
    pub fn con_aumento(escala_por_cien: u32, factor: f32) -> Lupa {
        Lupa { factor: factor.max(1.0), diametro: 240 * escala_por_cien / 100 }
    }

    pub fn region_fuente(&self, cursor: Punto, monitor: Rect) -> Rect {
        let lado = ((self.diametro as f32 / self.factor).round() as i32).max(1);
        // ... resto igual, usando `lado`
    }

    /// Donde dibujarla para que NO pise su propia region fuente (D60):
    /// prueba los cuatro cuadrantes a una distancia que garantiza la
    /// separacion y se queda con el primero que cabe en el monitor.
    pub fn colocar_fuera(&self, cursor: Punto, monitor: Rect) -> Punto {
        let d = self.diametro as i32;
        let fuente = self.region_fuente(cursor, monitor);
        let candidatos = [
            Punto { x: fuente.derecha() + MARGEN_CURSOR, y: fuente.abajo() + MARGEN_CURSOR },
            Punto { x: fuente.izquierda() - MARGEN_CURSOR - d, y: fuente.abajo() + MARGEN_CURSOR },
            Punto { x: fuente.derecha() + MARGEN_CURSOR, y: fuente.arriba() - MARGEN_CURSOR - d },
            Punto { x: fuente.izquierda() - MARGEN_CURSOR - d, y: fuente.arriba() - MARGEN_CURSOR - d },
            // Si ninguna diagonal cabe (monitor minusculo), a un lado.
            Punto { x: fuente.derecha() + MARGEN_CURSOR, y: fuente.arriba() },
            Punto { x: fuente.izquierda() - MARGEN_CURSOR - d, y: fuente.arriba() },
        ];
        for c in candidatos {
            let cabe = c.x >= monitor.izquierda()
                && c.y >= monitor.arriba()
                && c.x + d <= monitor.derecha()
                && c.y + d <= monitor.abajo();
            if cabe {
                return c;
            }
        }
        // Monitor mas pequeno que fuente+lupa: se acepta el solape.
        self.colocar(cursor, monitor)
    }
}
```

Actualizar los tests existentes que escriben `factor: 8` a `factor: 8.0`.

- [ ] **Step 4: Verde y compilación del ejecutable**

```bash
cargo test -p pixpin-ui 2>&1 | grep -E "^test result|FAILED"; cargo build -p pixpin 2>&1 | grep -cE "^error"
```

- [ ] **Step 5: Commit**

```bash
git add crates/pixpin-ui apps/pixpin/src/overlay.rs
git commit -m "La lupa de anotacion: aumento fraccionario y colocacion fuera de su propia fuente

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_016eoazzUbmckpE1eLfVoX36"
```

---

### Task 7: La lupa en la capa viva

**Files:**
- Modify: `apps/pixpin/src/capa.rs`

**Interfaces:**
- Produces: `CapaViva::quiere_muestra(&self) -> bool`; `CapaViva::poner_muestra(&mut self, inst: Instantanea)`; `CapaViva::monitor(&self) -> Monitor`.

- [ ] **Step 1: Estado nuevo en `CapaViva`**

```rust
    /// Lo ultimo que se vio debajo de la capa, para la lupa (D60). Se
    /// guarda la instantanea porque el bitmap ES su textura.
    muestra: Option<(pixpin_capture::Instantanea, windows::Win32::Graphics::Direct2D::ID2D1Bitmap1)>,
    ultima_muestra: std::time::Instant,
    monitor: Monitor,
```

Inicializar `muestra: None`, `ultima_muestra: Instant::now()`, `monitor: *monitor`.

- [ ] **Step 2: Métodos**

```rust
    pub fn monitor(&self) -> Monitor {
        self.monitor
    }

    /// Si hace falta una captura fresca para la lupa: solo con la lupa
    /// activa, recogiendo el raton, y como mucho 60 veces por segundo.
    pub fn quiere_muestra(&self) -> bool {
        self.anotador.herramienta() == Herramienta::Lupa
            && !self.ventana.es_pasante()
            && self.ultima_muestra.elapsed() >= std::time::Duration::from_millis(16)
    }

    pub fn poner_muestra(&mut self, inst: pixpin_capture::Instantanea) {
        if let Ok(b) = self.motor.bitmap_desde_textura(inst.textura()) {
            self.muestra = Some((inst, b));
        }
        self.ultima_muestra = std::time::Instant::now();
        self.pintar();
    }
```

- [ ] **Step 3: Pintar la lupa** (en `pintar`, tras las órdenes y antes de la caja; nunca en pasante):

```rust
            if !pasante && self.anotador.herramienta() == Herramienta::Lupa {
                if let Some((_, fuente_bitmap)) = &self.muestra {
                    let lupa = pixpin_ui::Lupa::con_aumento(self.escala_por_cien, self.anotador.lupa());
                    let local = Rect { x: 0, y: 0, ancho: self.area.ancho, alto: self.area.alto };
                    let fuente = lupa.region_fuente(self.cursor, local);
                    let pos = lupa.colocar_fuera(self.cursor, local);
                    let d = lupa.diametro as f32;
                    let destino = RectF { x: pos.x as f32, y: pos.y as f32, ancho: d, alto: d };
                    p.bitmap(
                        fuente_bitmap,
                        destino,
                        Some(RectF { x: fuente.x as f32, y: fuente.y as f32, ancho: fuente.ancho as f32, alto: fuente.alto as f32 }),
                        true,
                    );
                    p.trazar(destino, 2.0 * e, Color::ACENTO);
                }
            }
```

(`e` es la escala `self.escala_por_cien as f32 / 100.0`, calcularla al principio de `pintar`.)

- [ ] **Step 4: Cablear en `ejecutar_capa_viva`** — tras `capa.raton(EventoRaton::Mover(p))`:

```rust
            EventoOverlay::RatonMovido(p) => {
                let seguir = capa.raton(EventoRaton::Mover(p));
                if seguir && capa.quiere_muestra() {
                    match recursos.congelar_monitor(&capa.monitor()) {
                        Ok(inst) => capa.poner_muestra(inst),
                        Err(e) => tracing::debug!(?e, "sin muestra para la lupa"),
                    }
                }
                seguir
            }
```

El cierre de `bucle_modal` captura `capa` y `recursos` por préstamo mutable: son dos locales distintas, así que el borrow checker lo acepta.

- [ ] **Step 5: Puerta y commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -cE "^error"
git add apps/pixpin/src/capa.rs
git commit -m "La lupa sobre la pantalla viva: muestrea el monitor y se coloca fuera de su fuente

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_016eoazzUbmckpE1eLfVoX36"
```

---

### Task 8: El modo congelado (D56)

**Files:**
- Modify: `crates/pixpin-store/src/ajustes.rs` (struct `Atajos` ~línea 96, `Default` ~línea 110, test ~línea 189)
- Modify: `crates/pixpin-shell/src/atajos.rs` (`ID_ANOTAR_CONGELADA`), `crates/pixpin-shell/src/lib.rs` (reexport)
- Modify: `apps/pixpin/src/capa.rs`, `apps/pixpin/src/main.rs`

**Interfaces:**
- Produces: `pub enum ModoCapa { Viva, Congelada }`; `capa::ejecutar_capa(recursos: &mut Recursos, modo: ModoCapa) -> Result<Option<ImagenRgba>>` (sustituye a `ejecutar_capa_viva`); `pixpin_shell::ID_ANOTAR_CONGELADA: u32 = 8`; `Atajos::anotar_congelada` por defecto `Ctrl+Alt+Shift+A`.

- [ ] **Step 1: Test de ajustes que falla** — en el test de valores por defecto:

```rust
        assert_eq!(a.atajos.anotar_congelada.to_string(), "Ctrl+Alt+Shift+A");
```

- [ ] **Step 2: Implementar ajustes y atajo**

```rust
    /// Anotar sobre una captura estatica de la pantalla (S3-C, D56).
    pub anotar_congelada: Atajo,
// Default:
            anotar_congelada: "Ctrl+Alt+Shift+A".parse().expect("atajo por defecto valido"),
```

`atajos.rs`: `pub const ID_ANOTAR_CONGELADA: u32 = 8;` y reexport en `lib.rs`.

- [ ] **Step 3: La capa con fondo** — en `capa.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoCapa {
    /// Transparente: la pantalla sigue viva debajo (D49).
    Viva,
    /// Con la captura del monitor como fondo (D49/D56).
    Congelada,
}
```

`CapaViva` gana `fondo: Option<(Instantanea, ID2D1Bitmap1)>` y `nueva(...)` recibe `fondo: Option<Instantanea>` y lo envuelve con `motor.bitmap_desde_textura`. En `pintar`, tras `limpiar_transparente()`:

```rust
            if let Some((_, f)) = &self.fondo {
                p.bitmap(f, todo, None, false);
            }
```

`alternar_pasante` no hace nada en congelado (devuelve `false`): no hay nada vivo debajo. `quiere_muestra` devuelve `false` si hay fondo, y la lupa usa `self.fondo` como bitmap fuente cuando lo hay (`self.fondo.as_ref().or(self.muestra.as_ref())`).

`ejecutar_capa(recursos, modo)`: si `modo == Congelada`, `let fondo = Some(recursos.congelar_monitor(&monitor)?)` antes de crear la capa.

- [ ] **Step 4: main.rs** — registrar `(atajos::ID_ANOTAR_CONGELADA, config.atajos.anotar_congelada)` y convertir el brazo de `ID_ANOTAR` en uno que atienda ambos:

```rust
            Evento::Atajo(id) if id == atajos::ID_ANOTAR || id == atajos::ID_ANOTAR_CONGELADA => {
                let modo = if id == atajos::ID_ANOTAR { capa::ModoCapa::Viva } else { capa::ModoCapa::Congelada };
                // ... el cuerpo actual, llamando a capa::ejecutar_capa(r, modo)
```

- [ ] **Step 5: Puerta y commit**

```bash
cargo test --workspace -- --test-threads=1 2>&1 | grep -E "FAILED|^test result: F"; cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -cE "^error"
git add crates/pixpin-store crates/pixpin-shell apps/pixpin
git commit -m "Ctrl+Alt+Shift+A: anotar sobre la pantalla congelada con la misma capa

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_016eoazzUbmckpE1eLfVoX36"
```

---

### Task 9: Captura limpia al salir y la pregunta de guardar (D54, D59)

**Files:**
- Modify: `crates/pixpin-shell/src/overlay.rs` (fn libre `esperar_composicion`), `crates/pixpin-shell/src/dialogo.rs` (`preguntar`), `crates/pixpin-shell/src/lib.rs`
- Modify: `crates/pixpin-store/i18n/es-ES/main.ftl`, `crates/pixpin-store/i18n/en-US/main.ftl`
- Modify: `apps/pixpin/src/capa.rs`, `apps/pixpin/src/main.rs`

**Interfaces:**
- Produces: `pixpin_shell::esperar_composicion()` (dos `DwmFlush`); `pixpin_shell::preguntar(propietaria: HWND, titulo: &str, mensaje: &str) -> bool` (Sí/No, por defecto Sí); claves Fluent `capa-guardar-titulo`, `capa-guardar-pregunta`.

- [ ] **Step 1: Shell**

```rust
/// Espera a que el compositor haya presentado lo ultimo que se dibujo.
/// Dos vueltas: la primera cierra el fotograma en curso, la segunda
/// garantiza que el nuestro ya esta en pantalla y por tanto en la
/// captura (D59).
pub fn esperar_composicion() {
    use windows::Win32::Graphics::Dwm::DwmFlush;
    // SAFETY: sin precondiciones; un fallo (sin DWM) solo significa no
    // esperar.
    unsafe {
        let _ = DwmFlush();
        let _ = DwmFlush();
    }
}
```

`dialogo.rs`:

```rust
/// Pregunta de si/no con Si por defecto: para "¿guardar lo dibujado?".
pub fn preguntar(propietaria: HWND, titulo: &str, mensaje: &str) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{IDYES, MB_ICONQUESTION, MB_YESNO, MessageBoxW};
    let mensaje = HSTRING::from(mensaje);
    let titulo = HSTRING::from(titulo);
    // SAFETY: HSTRING propias vivas durante la llamada; propietaria del
    // llamante, viva mientras el cuadro es modal.
    unsafe { MessageBoxW(Some(propietaria), &mensaje, &titulo, MB_YESNO | MB_ICONQUESTION) == IDYES }
}
```

- [ ] **Step 2: Catálogo** — es-ES:

```
capa-guardar-titulo = Anotación de pantalla
capa-guardar-pregunta = ¿Guardar lo dibujado como un pin?
```
en-US:
```
capa-guardar-titulo = Screen annotation
capa-guardar-pregunta = Keep the drawing as a pin?
```

- [ ] **Step 3: Capa** — `pintar` pasa a `pintar_con(&self, cromo: bool)`; `pintar()` llama `pintar_con(true)`. Con `cromo == false` no se pinta ni la caja ni la lupa. En `ejecutar_capa`, la salida:

```rust
    if !capa.tiene_dibujo() {
        return Ok(None);
    }
    // Sin caja ni lupa: lo que se pinea es el dibujo sobre lo que habia
    // debajo, no la interfaz (D59).
    capa.pintar_con(false);
    pixpin_shell::esperar_composicion();
    let imagen = capturar_con_dibujo(recursos, &monitor)?;
    drop(capa);
    Ok(Some(imagen))
```

(`capturar_con_dibujo` deja de recibir `&CapaViva`.)

- [ ] **Step 4: main.rs** — al recibir `Ok(Some(imagen))`, preguntar antes de pinear:

```rust
                    Ok(Some(imagen)) => {
                        // D54: cerrar sin avisar tirando cinco minutos de
                        // anotaciones es el peor fallo posible aqui.
                        let guardar = pixpin_shell::preguntar(
                            hwnd,
                            &textos.t("capa-guardar-titulo"),
                            &textos.t("capa-guardar-pregunta"),
                        );
                        if guardar { /* el pineado actual */ } else { tracing::info!("anotacion descartada por el usuario"); }
                    }
```

(Comprobar el nombre real del método de traducción en `Catalogo`: el código usa `textos.t_args(...)`; el de sin argumentos es `textos.t(...)` si existe, si no `t_args` con `FluentArgs::new()`.)

- [ ] **Step 5: Puerta y commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -cE "^error"; cargo test --workspace -- --test-threads=1 2>&1 | grep -E "FAILED|^test result: F"
git add crates/pixpin-shell crates/pixpin-store apps/pixpin
git commit -m "Al salir de la capa: captura sin interfaz y pregunta antes de guardar como pin

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_016eoazzUbmckpE1eLfVoX36"
```

---

### Task 10: Texto in situ en la máquina pura, y los caracteres en el overlay (D57)

**Files:**
- Modify: `crates/pixpin-ui/src/anotador.rs`
- Modify: `crates/pixpin-shell/src/overlay.rs` (`EventoOverlay`, WndProc, `VentanaOverlay::poner_posicion_ime`), `crates/pixpin-shell/Cargo.toml` (feature `Win32_UI_Input_Ime`)
- Modify: `apps/pixpin/src/capa.rs`, `apps/pixpin/src/pines.rs` (quitar el brazo `PedirTexto`)

**Interfaces:**
- Produces: `EventoAnotador::Caracter(char)`; `TeclaAnotador::{Enter, Retroceso}`; `EfectoAnotador::PedirTexto` desaparece; `Anotador::editando_texto(&self) -> Option<Punto2>`; `EventoOverlay::Caracter(char)`; `VentanaOverlay::poner_posicion_ime(&self, p: Punto)`.

- [ ] **Step 1: Tests que fallan (anotador.rs)**

```rust
    fn con_texto() -> Anotador {
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Texto));
        a.procesar(EventoAnotador::Pulsar(Punto2::nuevo(30.0, 40.0)));
        a
    }

    #[test]
    fn escribir_y_enter_dejan_un_elemento_de_texto_donde_se_pulso() {
        let mut a = con_texto();
        for c in "Hola".chars() {
            let efecto = a.procesar(EventoAnotador::Caracter(c));
            assert!(matches!(efecto, EfectoAnotador::EnCurso(_)), "cada letra se ve al momento");
        }
        let EfectoAnotador::Terminado(e) = a.procesar(EventoAnotador::Tecla(TeclaAnotador::Enter)) else {
            panic!("Enter confirma");
        };
        assert_eq!(e.figura, Figura::Texto { texto: "Hola".into(), tam: 20.0, familia: "Segoe UI".into() });
        assert_eq!((e.x, e.y), (30.0, 40.0));
        assert!(a.editando_texto().is_none());
    }

    #[test]
    fn retroceso_borra_y_un_texto_vacio_no_deja_nada() {
        // Caso negativo: un clic con Texto y Enter sin escribir no puede
        // dejar un elemento invisible que luego estorbe al seleccionar.
        let mut a = con_texto();
        a.procesar(EventoAnotador::Caracter('a'));
        a.procesar(EventoAnotador::Tecla(TeclaAnotador::Retroceso));
        assert_eq!(a.procesar(EventoAnotador::Tecla(TeclaAnotador::Enter)), EfectoAnotador::Repintar);
    }

    #[test]
    fn escape_cancela_el_texto_sin_salir() {
        let mut a = con_texto();
        a.procesar(EventoAnotador::Caracter('x'));
        assert_eq!(a.procesar(EventoAnotador::Tecla(TeclaAnotador::Escape)), EfectoAnotador::Repintar);
        assert!(a.editando_texto().is_none());
        assert_eq!(a.procesar(EventoAnotador::Tecla(TeclaAnotador::Escape)), EfectoAnotador::Salir);
    }

    #[test]
    fn cambiar_de_herramienta_confirma_el_texto_escrito() {
        // Perder lo escrito por pulsar el lapiz seria un fallo de datos.
        let mut a = con_texto();
        a.procesar(EventoAnotador::Caracter('y'));
        assert!(matches!(
            a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Lapiz)),
            EfectoAnotador::Terminado(_)
        ));
        assert_eq!(a.herramienta(), Herramienta::Lapiz);
    }

    #[test]
    fn los_caracteres_de_control_no_entran_en_el_texto() {
        let mut a = con_texto();
        assert_eq!(a.procesar(EventoAnotador::Caracter('\r')), EfectoAnotador::Nada);
        assert_eq!(a.procesar(EventoAnotador::Caracter('\u{8}')), EfectoAnotador::Nada);
    }

    #[test]
    fn el_texto_de_la_previsualizacion_lleva_cursor() {
        let mut a = con_texto();
        let EfectoAnotador::EnCurso(e) = a.procesar(EventoAnotador::Caracter('a')) else { panic!() };
        assert_eq!(e.figura, Figura::Texto { texto: "a|".into(), tam: 20.0, familia: "Segoe UI".into() });
    }
```

El tamaño del texto es `(grosor * 5.0).clamp(14.0, 120.0)` → con el grosor por defecto 4,0 sale 20,0.

- [ ] **Step 2: Fallan** — `cargo test -p pixpin-ui 2>&1 | grep -E "^error" | head -3`.

- [ ] **Step 3: Implementar**

```rust
pub enum TeclaAnotador { Escape, Deshacer, Rehacer, Suprimir, Enter, Retroceso }

pub enum EventoAnotador { /* ... */ Caracter(char), /* ... */ }
// quitar EfectoAnotador::PedirTexto

#[derive(Debug, Clone)]
struct TextoEnCurso {
    origen: Punto2,
    contenido: String,
}
// campo nuevo en Anotador: texto: Option<TextoEnCurso>

    /// Donde se esta escribiendo, si se esta escribiendo: para colocar la
    /// ventana del IME.
    pub fn editando_texto(&self) -> Option<Punto2> {
        self.texto.as_ref().map(|t| t.origen)
    }

    fn tam_texto(&self) -> f32 {
        (self.grosor * 5.0).clamp(14.0, 120.0)
    }

    fn elemento_texto(&self, t: &TextoEnCurso, cursor: bool) -> Elemento {
        let tam = self.tam_texto();
        let mut texto = t.contenido.clone();
        if cursor {
            texto.push('|');
        }
        let ancho = (t.contenido.chars().count().max(1) as f32) * tam * 0.6;
        Elemento {
            id: 0,
            figura: Figura::Texto { texto, tam, familia: "Segoe UI".into() },
            x: t.origen.x,
            y: t.origen.y,
            ancho,
            alto: tam * 1.3,
            angulo: 0.0,
            trazo: self.color,
            relleno: None,
            grosor: self.grosor,
            estilo: EstiloTrazo::Solido,
            rugosidad: 0.0,
            opacidad: 1.0,
            semilla: self.semilla,
            version: 0,
            borrado: false,
        }
    }

    /// Confirma el texto en curso: `Terminado` si hay algo, `Repintar` si
    /// estaba vacio (un texto vacio es basura invisible).
    fn confirmar_texto(&mut self) -> EfectoAnotador {
        let Some(t) = self.texto.take() else {
            return EfectoAnotador::Nada;
        };
        if t.contenido.is_empty() {
            return EfectoAnotador::Repintar;
        }
        let e = self.elemento_texto(&t, false);
        self.semilla = (self.semilla.wrapping_mul(48271) & 0x7FFF_FFFF).max(1);
        EfectoAnotador::Terminado(Box::new(e))
    }
```

En `procesar`:
- `CambiarHerramienta(h)`: `let confirmado = self.confirmar_texto(); self.gesto = None; self.herramienta = h; if confirmado != Nada { return confirmado } else { Repintar }`.
- `Caracter(c)`: si `c.is_control()` → `Nada`; si no hay texto en curso → `Nada`; si lo hay, push y `EnCurso(elemento_texto(t, true))`.
- `Tecla(Enter)`: `confirmar_texto()` (si no había texto → `Nada`).
- `Tecla(Retroceso)`: pop y `EnCurso`; sin texto → `Nada`.
- `Tecla(Escape)`: si hay texto → `self.texto = None; Repintar`; luego las reglas actuales del gesto.
- `Pulsar(p)` con `Herramienta::Texto`: si hay texto en curso → `confirmar_texto()`; si no → `self.texto = Some(TextoEnCurso { origen: p, contenido: String::new() }); EnCurso(elemento_texto(.., true))`.
- `Tecla(Suprimir)` se queda como estaba.

- [ ] **Step 4: Verde en pixpin-ui** — `cargo test -p pixpin-ui 2>&1 | grep -E "^test result|FAILED"`.

- [ ] **Step 5: Shell** — `Cargo.toml`: añadir `"Win32_UI_Input_Ime"` a las features. `EventoOverlay::Caracter(char)`. En el WndProc:

```rust
        WM_CHAR => {
            // WM_CHAR trae unidades UTF-16: un emoji llega en dos mensajes.
            // La mitad alta se guarda hasta que llega la baja.
            let unidad = wparam.0 as u16;
            let caracter = MITAD_ALTA.with(|alta| {
                if (0xD800..0xDC00).contains(&unidad) {
                    alta.set(Some(unidad));
                    None
                } else if (0xDC00..0xE000).contains(&unidad) {
                    let a = alta.take()?;
                    char::decode_utf16([a, unidad]).next()?.ok()
                } else {
                    alta.set(None);
                    char::from_u32(unidad as u32)
                }
            });
            if let Some(c) = caracter {
                encolar(EventoOverlay::Caracter(c));
            }
            LRESULT(0)
        }
```

con `thread_local! { static MITAD_ALTA: Cell<Option<u16>> = const { Cell::new(None) }; }`.

```rust
    /// Coloca la ventana de composicion del IME donde se escribe: sin esto
    /// el japones o el chino se componen en la esquina de la pantalla.
    pub fn poner_posicion_ime(&self, p: Punto) {
        use windows::Win32::UI::Input::Ime::{
            CFS_POINT, COMPOSITIONFORM, ImmGetContext, ImmReleaseContext, ImmSetCompositionWindow,
        };
        // SAFETY: contexto del IME de una ventana propia, tomado y
        // devuelto en la misma llamada.
        unsafe {
            let ctx = ImmGetContext(self.hwnd);
            if ctx.is_invalid() {
                return;
            }
            let forma = COMPOSITIONFORM {
                dwStyle: CFS_POINT,
                ptCurrentPos: windows::Win32::Foundation::POINT { x: p.x, y: p.y },
                ..Default::default()
            };
            let _ = ImmSetCompositionWindow(ctx, &forma);
            let _ = ImmReleaseContext(self.hwnd, ctx);
        }
    }
```

(Si `ImmGetContext` devuelve `HIMC` sin `is_invalid`, comparar con `HIMC::default()`.)

- [ ] **Step 6: Capa** — en el bucle: `EventoOverlay::Caracter(c) => capa.caracter(c)`, `VK_RETURN (0x0D) => capa.tecla(TeclaAnotador::Enter)`, `VK_BACK (0x08) => capa.tecla(TeclaAnotador::Retroceso)`. Y `CapaViva::caracter(&mut self, c: char) -> bool { self.anotar(EventoAnotador::Caracter(c)); true }`. Tras cada efecto, si `self.anotador.editando_texto()` es `Some(p)`, `self.ventana.poner_posicion_ime(Punto { x: p.x as i32, y: p.y as i32 })`. Quitar el brazo `PedirTexto` en `aplicar` y en `pines.rs::anotar`. Al procesar `Tecla { vk: VK_ESCAPE }` el orden ya está bien: la máquina decide.

- [ ] **Step 7: Puerta y commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -cE "^error"; cargo test --workspace -- --test-threads=1 2>&1 | grep -E "FAILED|^test result: F"
git add crates/pixpin-ui crates/pixpin-shell apps/pixpin
git commit -m "Texto in situ: se escribe donde se pulsa, Enter confirma y el IME compone al lado

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_016eoazzUbmckpE1eLfVoX36"
```

---

### Task 11: Texto in situ dentro del pin

**Files:**
- Modify: `crates/pixpin-pin/src/ventana.rs` (`CambioPin`, WndProc, `Pin::poner_posicion_ime`), `crates/pixpin-pin/Cargo.toml` (feature `Win32_UI_Input_Ime`)
- Modify: `apps/pixpin/src/pines.rs` (`atender`, `anotar`)

**Interfaces:**
- Produces: `CambioPin::{CaracterAnotando(char), EnterAnotando, RetrocesoAnotando}`; `Pin::poner_posicion_ime(&self, p: Punto)` (p en coordenadas del contenido).

- [ ] **Step 1: Pin** — `WM_CHAR` cuando `i.anotando` (mismo manejo de subrogados que en la Task 10, con su propio `thread_local`), `WM_KEYDOWN` con `VK_RETURN` → `EnterAnotando` y `VK_BACK` → `RetrocesoAnotando`, ambos solo si `i.anotando`. `poner_posicion_ime` suma el margen de sombra (`MARGEN_SOMBRA_LOGICO * escala / 100`) al punto del contenido y hace lo mismo que en el overlay.

- [ ] **Step 2: Gestor** — en `atender`:

```rust
            CambioPin::CaracterAnotando(c) => self.anotar(id, EventoAnotador::Caracter(c)),
            CambioPin::EnterAnotando => self.anotar(id, EventoAnotador::Tecla(TeclaAnotador::Enter)),
            CambioPin::RetrocesoAnotando => self.anotar(id, EventoAnotador::Tecla(TeclaAnotador::Retroceso)),
```

En `anotar`, tras aplicar el efecto y antes de repintar: `if let Some(p) = a.anotador.editando_texto() { pin.poner_posicion_ime(Punto { x: p.x as i32, y: p.y as i32 }) }`.

- [ ] **Step 3: Puerta y commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -cE "^error"; cargo test --workspace -- --test-threads=1 2>&1 | grep -E "FAILED|^test result: F"
git add crates/pixpin-pin apps/pixpin/src/pines.rs
git commit -m "El pin tambien escribe: caracteres, Enter y Retroceso llegan al anotador

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_016eoazzUbmckpE1eLfVoX36"
```

---

### Task 12: La paleta flotante del pin (D58)

El pin es L2 y la caja (`pixpin-ui`) es L3: el pin no puede pintarla. La paleta es una ventana pequeña de `pixpin-pin` que solo sabe **dónde se pulsa** y **pintar lo que le digan**; el gestor la coloca con `CajaHerramientas::colocar` y la pinta con el mismo código que la capa.

**Files:**
- Create: `crates/pixpin-pin/src/paleta.rs`
- Modify: `crates/pixpin-pin/src/lib.rs` (`pub mod paleta; pub use paleta::Paleta;`), `crates/pixpin-pin/src/ventana.rs` (`CambioPin::PaletaPulsada(Punto)`)
- Create: `apps/pixpin/src/caja_dibujo.rs` (el `pintar_caja` y `etiqueta` que hoy viven en `capa.rs`, como `pub fn pintar_caja(p: &Pintor, caja: &CajaHerramientas, activa: Herramienta, escala_por_cien: u32)`)
- Modify: `apps/pixpin/src/capa.rs` (usar `caja_dibujo`), `apps/pixpin/src/main.rs` (`mod caja_dibujo;`), `apps/pixpin/src/pines.rs`

**Interfaces:**
- Produces:
```rust
pub struct Paleta { hwnd: HWND }
impl Paleta {
    /// `rect` en pixeles fisicos del escritorio virtual. `al_pulsar` recibe
    /// el punto LOCAL a la paleta. No roba el foco (WS_EX_NOACTIVATE): el
    /// teclado sigue en el pin.
    pub fn nueva(d3d: &ID3D11Device, motor: Rc<MotorRender>, rect: Rect, al_pulsar: Box<dyn Fn(Punto)>) -> Result<Paleta, ErrorPin>;
    /// Cambia como se pinta y repinta ya. Tambien se usa en WM_PAINT.
    pub fn poner_pintor(&self, pintor: Box<dyn Fn(&Pintor)>);
}
impl Drop for Paleta { /* DestroyWindow */ }
```

- [ ] **Step 1: paleta.rs** — seguir el patrón exacto de `ventana.rs`: `Once` + `registrar_clase` con `w!("PixPinPaleta")`, `CreateWindowExW(WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, ..., WS_POPUP, ...)`, `Superficie::nueva`, un `PaletaInterno { motor, superficie, ancho, alto, pintor: Option<Box<dyn Fn(&Pintor)>>, al_pulsar }` en `GWLP_USERDATA`, `ShowWindow(SW_SHOWNOACTIVATE)`. WndProc:
  - `WM_MOUSEACTIVATE` → `LRESULT(MA_NOACTIVATE as isize)` (así ni el clic activa).
  - `WM_LBUTTONUP` → `(i.al_pulsar)(punto(lparam))`.
  - `WM_PAINT` → `ValidateRect` y `pintar(i)`.
  - `WM_NCDESTROY` → recuperar el `Box` (igual que el pin).
  `fn pintar(i)`: `superficie.empezar` → `motor.dibujar(&destino, |p| { p.limpiar_transparente(); if let Some(f) = &i.pintor { f(p) } })` → `presentar`.

- [ ] **Step 2: `CambioPin::PaletaPulsada(Punto)`** con doc: «un clic en la paleta del pin, en coordenadas de la paleta; lo produce el gestor, no la ventana del pin, pero viaja por la misma cola».

- [ ] **Step 3: caja_dibujo.rs** — mover `pintar_caja`/`etiqueta` de `capa.rs` a una función libre:

```rust
pub fn pintar_caja(p: &Pintor, caja: &CajaHerramientas, activa: Herramienta, escala_por_cien: u32, origen: Punto)
```
donde `origen` se resta a cada rect (la capa pasa `Punto { x: 0, y: 0 }` porque su documento ya es local al monitor; la paleta pasa `caja.marco` para que el marco caiga en `(0,0)` de su ventana).

- [ ] **Step 4: Gestor** — `Anotacion` gana `caja: CajaHerramientas`, `paleta: Paleta`, `ultimo_cursor: Punto`. En `entrar_a_anotar`:

```rust
        let disposicion = pixpin_capture::enumerar_monitores().context("sin monitores")?;
        let contenido = pin.rect_contenido();
        let monitor = disposicion
            .monitores()
            .iter()
            .find(|m| m.area.contiene(Punto { x: contenido.x, y: contenido.y }))
            .or_else(|| disposicion.principal())
            .copied()
            .context("sin monitor para la paleta")?;
        let caja = CajaHerramientas::colocar(contenido, monitor.area_trabajo, monitor.escala_por_cien);
        let pedidos = Rc::clone(&self.pedidos);
        let hwnd_app = self.hwnd_app;
        let paleta = Paleta::nueva(&self.d3d, Rc::clone(&self.motor), caja.marco, Box::new(move |p| {
            pedidos.borrow_mut().push((id, CambioPin::PaletaPulsada(p)));
            pixpin_shell::despertar(hwnd_app);
        }))?;
```

y un método `fn repintar_paleta(&self)` que hace `paleta.poner_pintor(Box::new(move |p| caja_dibujo::pintar_caja(p, &caja, activa, escala, caja.marco.origen())))` (capturando copias: `CajaHerramientas` es `Copy`). Llamarlo al entrar y cada vez que cambie la herramienta.

En `atender`: `CambioPin::PaletaPulsada(p)` → convertir a global `Punto { x: p.x + caja.marco.x, y: p.y + caja.marco.y }`, `caja.boton_en(global)` y resolver como en `CapaViva::pulsar_boton` (`Elegir(h)` → `anotar(id, CambiarHerramienta(h))` + repintar paleta; `Deshacer`/`Rehacer` → escena + `poner_anotaciones`; `Salir` → `salir_de_anotar()`; `Color` → nada por ahora).

`salir_de_anotar` ya hace `self.anotacion.take()`: la paleta se destruye con el `Drop`.

- [ ] **Step 5: Puerta y commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -cE "^error"; cargo test --workspace -- --test-threads=1 2>&1 | grep -E "FAILED|^test result: F"
git add crates/pixpin-pin apps/pixpin
git commit -m "La paleta flotante del pin: la caja de herramientas junto al pin mientras se anota

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_016eoazzUbmckpE1eLfVoX36"
```

---

### Task 13: La lupa dentro del pin

**Files:**
- Modify: `crates/pixpin-pin/src/ventana.rs` (`pub struct LupaPin`, `Pin::poner_lupa`, en `pintar` tras las anotaciones)
- Modify: `apps/pixpin/src/pines.rs`

**Interfaces:**
- Produces: `pub struct LupaPin { pub fuente: Rect, pub destino: Rect }` (coordenadas del contenido); `Pin::poner_lupa(&self, lupa: Option<LupaPin>)`.

- [ ] **Step 1: Pin** — campo `lupa: Option<LupaPin>` en `PinInterno`; `poner_lupa` lo guarda y repinta. En `pintar`, después de `pintar_anotaciones` y solo si `i.bitmap` existe:

```rust
        if let (Some(l), Some(b)) = (&i.lupa, &i.bitmap) {
            // La fuente esta en pixeles del contenido, que puede ir escalado
            // respecto al bitmap nativo: se convierte a pixeles del bitmap.
            let (nw, nh) = i.imagen_nativa;
            let fx = nw as f32 / w.max(1.0);
            let fy = nh as f32 / h.max(1.0);
            let fuente = RectF { x: l.fuente.x as f32 * fx, y: l.fuente.y as f32 * fy, ancho: l.fuente.ancho as f32 * fx, alto: l.fuente.alto as f32 * fy };
            let destino = RectF { x: l.destino.x as f32 + m, y: l.destino.y as f32 + m, ancho: l.destino.ancho as f32, alto: l.destino.alto as f32 };
            p.bitmap(b, destino, Some(fuente), true);
            p.trazar(destino, 2.0 * escala, Color::ACENTO);
        }
```

- [ ] **Step 2: Gestor** — en `anotar`, tras aplicar el efecto:

```rust
        if let EventoAnotador::Mover(p) | EventoAnotador::Pulsar(p) = &evento_copia { a.ultimo_cursor = Punto { x: p.x as i32, y: p.y as i32 }; }
        let lupa = if a.anotador.herramienta() == Herramienta::Lupa {
            let r = pin.rect_contenido();
            let local = Rect { x: 0, y: 0, ancho: r.ancho, alto: r.alto };
            let l = Lupa::con_aumento(escala, a.anotador.lupa());
            Some(LupaPin { fuente: l.region_fuente(a.ultimo_cursor, local), destino: { let p = l.colocar(a.ultimo_cursor, local); Rect { x: p.x, y: p.y, ancho: l.diametro, alto: l.diametro } } })
        } else { None };
        pin.poner_lupa(lupa);
```

(`evento_copia` es un `clone()` del evento antes de dárselo a la máquina; `escala` es la del pin, que `Pines` conoce por el monitor o por el guardado del pin — usar `100` si no hay otra a mano, igual que hace `zoom`.) Para un pin sin bitmap (nota) el pin ignora la lupa.

- [ ] **Step 3: Puerta y commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -cE "^error"; cargo test --workspace -- --test-threads=1 2>&1 | grep -E "FAILED|^test result: F"
git add crates/pixpin-pin apps/pixpin/src/pines.rs
git commit -m "La lupa dentro del pin amplia su propio bitmap

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_016eoazzUbmckpE1eLfVoX36"
```

---

### Task 14: Extremo a extremo sobre el binario release, medidas, documentos y PR

La lección de S2-B y S3-B: los fallos que quedan son de cableado y solo los caza la prueba E2E sobre el binario. Esta tarea no se salta.

**Files:**
- Create: `medidas/2026-09-02-equipo-desarrollo-s3c.md`
- Modify: `docs/superpowers/plans/2026-09-02-plan-maestro.md` (marcar S3-C), `docs/superpowers/specs/2026-09-02-s3bc-anotacion-design.md` (criterios cumplidos con `[x]`)

- [ ] **Step 1: Build release y arranque**

```bash
cargo build --release 2>&1 | grep -E "^error|Finished"
```
Arrancar `target/release/pixpinmax.exe` en modo portable (con `pixpinmax.toml` junto al exe, como en fases anteriores) y seguir el log.

- [ ] **Step 2: Recorrido de la capa viva** (entrada sintetizada por PowerShell con `SendInput`/`keybd_event` como en `medidas/…-s3b.md`, o a mano si el usuario está):
  1. `Ctrl+Alt+A` → log `capa viva abierta`; la pantalla sigue viva debajo (abrir un reloj o vídeo).
  2. Arrastrar con el lápiz → se ve el trazo.
  3. Pulsar `F` en la caja (foco), arrastrar → todo oscuro salvo el rectángulo.
  4. Pulsar `Q` (lupa), mover → la lupa amplía lo de debajo y no se amplía a sí misma; rueda cambia el aumento.
  5. Pulsar `T`, clic, escribir «hola», Enter → texto en su sitio.
  6. Espacio → `la capa cambia de modo pasante=true`; clic sobre la aplicación de abajo la activa (comprobar con `GetForegroundWindow`); Espacio de nuevo vuelve a dibujar.
  7. Escape → cuadro «¿Guardar lo dibujado como un pin?»; Sí → pin con la captura anotada, **sin caja ni lupa en la imagen**.
- [ ] **Step 3: Recorrido del modo congelado** — `Ctrl+Alt+Shift+A`, dibujar, Espacio no hace nada, Escape → No → sin pin y log `anotacion descartada`.
- [ ] **Step 4: Recorrido del pin** — doble clic en un pin de imagen → aparece la paleta a su lado; elegir foco, lupa, texto; Escape guarda y la paleta desaparece; reiniciar → la anotación con foco y texto vuelve.
- [ ] **Step 5: Medidas** — CPU de la capa viva en reposo (`Get-Process pixpinmax | select CPU` en dos instantes separados 10 s, esperado 0 %), RAM, tiempo atajo→capa visible desde el log (`tracing` con `Instant` alrededor de `CapaViva::nueva` + `mostrar`), y una nota honesta sobre lo que no se pudo sintetizar. Escribir `medidas/2026-09-02-equipo-desarrollo-s3c.md` con el mismo formato que el de S3-B.
- [ ] **Step 6: Documentos** — marcar `[x]` S3-C en el plan maestro y los criterios cumplidos en la spec; anotar D56–D61 en la spec (sección 2).
- [ ] **Step 7: Puerta final y PR**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -cE "^error"; cargo test --workspace -- --test-threads=1 2>&1 | grep -E "^test result|FAILED"
git add medidas docs
git commit -m "Cierra S3-C: recorrido E2E, medidas y decisiones D56-D61

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_016eoazzUbmckpE1eLfVoX36"
git push -u origin s3c-anotar-pantalla
gh pr create --title "S3-C: anotar la pantalla (capa viva, congelada, foco, lupa, texto)" --body "..."
```
Esperar CI verde y fusionar con `gh pr merge --merge`.

---

## Autorrevisión

- **Cobertura de la spec §6:** capa viva con atajo (T1), pasante (T1/T14), foco y lupa en los dos modos (T2–T7, T13), rueda = grosor/aumento (ya en la máquina; zoom del pin ya en S3-B), texto (T10–T11), guardado como pin con pregunta (T9), modo congelado (T8), puertas medidas (T14). Caja de herramientas del pin (T12). Imágenes incrustadas: fuera, D61.
- **Tipos:** `Orden::Velo { hueco, color }` se usa igual en T2, T5; `Lupa::con_aumento(u32, f32)` y `colocar_fuera` en T6, T7, T13; `TeclaAnotador::{Enter, Retroceso}` y `EventoAnotador::Caracter` en T10, T11; `CambioPin::PaletaPulsada(Punto)` en T12; `ModoCapa` y `ejecutar_capa` en T8, T9.
- **Sin marcadores de posición:** los pasos que dependen de nombres exactos que no se leyeron en la sesión (el método de traducción sin argumentos, `Rect::interseccion`, `HIMC::is_invalid`) dicen qué comprobar y las dos alternativas.
