# Lienzo desde el pin y tinta a escala de Windows — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que la tinta del editor salga del tamaño de Excalidraw (escala de Windows), que el usuario pueda medir en su equipo por qué el lienzo va lento, y que un pin de imagen se abra en el lienzo infinito con la imagen de fondo, se dibuje encima y vuelva guardado al pin.

**Architecture:** La cámara del editor pasa a píxeles lógicos: una función pura `vista_efectiva` multiplica su zoom por la escala del monitor y es la única cámara con la que se pinta y se traduce el ratón. Un medidor puro acumula tiempos por fotograma y, con `[rendimiento] medir_fotogramas`, los registra cada 60. Para la Parte 1, un `Navegador` puro traduce rueda, espacio y botón central a desplazamiento o zoom con las constantes de Excalidraw; `pixpin-render` gana bitmaps premultiplicados y modo de interpolación; un `FondoLienzo` sube la imagen una vez y la pinta dentro de la capa congelada; el gestor de pines prepara un `PedidoLienzo`, `main.rs` abre el editor con él y se lo devuelve al gestor, que guarda en el `.pixpin2d` y repinta el pin.

**Tech Stack:** Rust 1.97 (workspace), crate `windows` 0.62 (Direct2D, Win32), `image` 0.25 (solo en `pixpin-codec`), serde/toml, `tracing`.

**Spec:** `docs/superpowers/specs/2026-09-13-lienzo-desde-pin-design.md`

## Global Constraints

- Rama `lienzo-desde-pin`. No se fusiona `main` aquí.
- Suite completa: `cargo test --workspace --no-fail-fast -- --test-threads=1` (sin `--test-threads=1` `crates/pixpin-motor2d/tests/asignaciones.rs` da rojos falsos; con `pixpinmax.exe` abierto falla `la_segunda_adquisicion_falla_mientras_viva_la_primera`, que no cuenta).
- `cargo` puede no estar en el PATH: anteponer `%USERPROFILE%\.cargo\bin`.
- Antes de cada commit: `cargo fmt --all` y `cargo clippy --workspace --all-targets -- -D warnings` limpios.
- Estilo `.rs`: español **sin tildes**, comentarios que explican el POR QUÉ, pruebas con nombre de frase y con casos negativos. `unsafe` solo con `// SAFETY:` (y nunca en `apps/pixpin`, que es `#![forbid(unsafe_code)]`).
- Regla de capas (`apps/pixpin/tests/capas.rs`): L0 `pixpin-geom`, `pixpin-model`, `pixpin-nivel`; L1 `pixpin-shell`, `pixpin-render`, `pixpin-gpu`, `pixpin-codec`, `pixpin-motor2d`; L2 `pixpin-capture`, `pixpin-pin`, `pixpin-pdf`, `pixpin-ocr`, `pixpin-record`, `pixpin-store`, `pixpin-proyecto`; L3 `pixpin-ui`, `pixpin-flow`, `pixpin-plugin`; L4 `pixpin`. Un crate solo depende de capas inferiores. `pixpin-render` y `pixpin-motor2d` son L1 y **no se conocen entre sí**; L2 puede depender de L1. Ninguna dependencia nueva entre crates en este plan.
- Constantes exactas: margen de encuadre 48 px lógicos (D135); `ZOOM_STEP = 0.1`, `MIN_ZOOM = 0.1`, `MAX_ZOOM = 30` de Excalidraw (`packages/common/src/constants.ts:349-351` @afa3a65); 60 fotogramas por línea de medición (D129); reserva 800×600 (errores); `WHEEL_DELTA = 120`; 100 px CSS por muesca.
- `EventoOverlay` sigue siendo `Copy + Eq`.
- `crates/pixpin-motor2d/tests/asignaciones.rs` sigue en 0 asignaciones en el camino caliente (este plan no toca el motor 2D).
- El agente **no** ejecuta la app ni sintetiza entrada (regla del usuario). Las pruebas manuales las hace el usuario.
- Cada commit termina con una línea en blanco y `Claude-Session: https://claude.ai/code/session_01FJbWm4jCHQtHwwndRDpLZJ`.

## Ajustes respecto a la spec

- **D141:** no hay un enum `Nitidez` en la aplicación además del de `pixpin-render`: `fondo_lienzo::modo_nitidez` devuelve directamente `pixpin_render::Interpolacion { Vecino, Lineal, Cubica }`. Dos enums iguales con una traducción entre ellos serían dos sitios que hay que tocar a la vez.
- **D143:** mientras el lienzo está abierto se conservan también los píxeles en CPU (la imagen ya reducida si hizo falta). Es lo que permite «volver a subir el bitmap» tras un dispositivo perdido (tabla de errores); se suelta todo al cerrar.
- **D136:** Windows da 120 por muesca y Excalidraw lee `deltaY` del navegador; se toma la equivalencia de Chromium en Windows (100 px CSS por muesca). La rueda horizontal (`WM_MOUSEHWHEEL`) y el pellizco del panel táctil quedan fuera.
- **D136:** `EventoOverlay::Rueda(i32)` no se cambia (lo usan la capa y los ajustes); los modificadores de la rueda se leen con `pixpin_shell::entrada::modificadores_pulsados()`, solo para ese evento. El foco del zoom es el último `RatonMovido` visto.
- **D144:** el editor tapa el área de trabajo del monitor principal. Un pin en otro monitor sigue pudiéndose cerrar con su propio botón mientras el lienzo está abierto; no se bloquea (haría falta tocar el WndProc del pin). Guardar no depende de que el pin siga vivo: la ruta sale del almacén.
- **D147:** para que el pin no muestre lo dibujado fuera de la imagen, `pintar_anotaciones` pasa a ir recortada a la tarjeta del contenido (hoy se sale por el margen de sombra).
- **D129:** `Instant::now()` se toma siempre (nanosegundos); solo el acumulado y el registro dependen de la opción. El tiempo de espera que sigue a un fotograma se atribuye a ese fotograma.
- **`abrir`:** gana dos parámetros sueltos (`medir_fotogramas: bool` en la Tarea 2, `fondo: Option<ImagenRgba>` en la Tarea 8) en vez de una función `abrir_con_fondo`; la bandeja pasa `None`.
- Fuera de alcance: el marco y los tiradores de la selección siguen midiéndose en píxeles físicos; el encuadre inicial no descuenta la caja de herramientas.

## Mapa de ficheros

| Fichero | Qué | Tarea |
|---|---|---|
| `apps/pixpin/src/navegacion.rs` (nuevo) | `escala_de`, `vista_efectiva`; `Navegador`, `zoom_de_rueda`, `aplicar` | 1, 5 |
| `apps/pixpin/src/medir_fotogramas.rs` (nuevo) | `MedidorFotogramas`, `Vuelta`, `Resumen` | 2 |
| `apps/pixpin/src/ventana_editor.rs` | Cámara lógica, CambioDpi, medición, navegación, fondo | 1, 2, 4, 5, 8 |
| `apps/pixpin/src/main.rs` | `mod`s nuevos, llamada de la bandeja, bucle del lienzo, textos | 1, 2, 5, 7, 8, 9, 10 |
| `crates/pixpin-store/src/ajustes.rs` | `Rendimiento::medir_fotogramas` | 2 |
| `crates/pixpin-shell/src/overlay.rs` | Botón central, `traer_encima` | 4 |
| `apps/pixpin/src/overlay.rs` | `match` exhaustivo con las variantes nuevas | 4 |
| `crates/pixpin-render/src/motor.rs` | `premultiplicar`, `bitmap_desde_pixeles_premultiplicado`, `lado_maximo_bitmap` | 6 |
| `crates/pixpin-render/src/lienzo.rs`, `lib.rs` | `Interpolacion`, `Pintor::bitmap_con` | 6 |
| `crates/pixpin-codec/src/imagen.rs`, `lib.rs` | `redimensionar` | 7 |
| `apps/pixpin/src/fondo_lienzo.rs` (nuevo) | `FondoLienzo`, `encuadre_inicial`, `modo_nitidez`, `lado_de_subida`, `se_ve`, `recuadro_gris` | 7 |
| `crates/pixpin-pin/src/menu.rs`, `ventana.rs` | `CMD_ABRIR_LIENZO`, `CambioPin::AbrirLienzoPedido`, recorte de anotaciones | 9, 10 |
| `crates/pixpin-store/i18n/{es-ES,en-US}/main.ftl` | `pin-abrir-en-lienzo` | 9 |
| `apps/pixpin/src/pines.rs` | `PedidoLienzo`, `tomar_lienzo`, `terminar_lienzo` y sus piezas puras | 10 |

---

# Parte 0a — tamaño y medición

### Task 1: La escala de Windows en la cámara del editor (D127, D128)

**Files:**
- Create: `apps/pixpin/src/navegacion.rs`
- Modify: `apps/pixpin/src/main.rs:52-63` (añadir `mod navegacion;` entre `mod grabador;` y `mod overlay;`)
- Modify: `apps/pixpin/src/ventana_editor.rs` (`use` :32-46; `abrir` :351-696, en concreto :380-397 inicialización, :406-411 cabeza del bucle, :475-485 traducción y tolerancia, :492-513 zona sucia, :518-565 capa congelada, :574-597 calibrar, :631-647 pintar, :667-680 zoom; pruebas :1486-1508)
- D128: `apps/pixpin/src/capa.rs` y `apps/pixpin/src/pines.rs` **no** se tocan.

**Interfaces:**
- Consumes: `pixpin_motor2d::camara::Camara { x, y, zoom }`, `Camara::{a_pantalla, a_mundo}`, `Monitor::escala_por_cien`, `pixpin_capture::enumerar_monitores`.
- Produces:
  - `pub fn escala_de(escala_por_cien: u32) -> f32`
  - `pub fn vista_efectiva(camara: &Camara, escala_por_cien: u32) -> Camara`

- [ ] **Step 1: Pruebas puras (fallan: no existe el módulo)**

`apps/pixpin/src/navegacion.rs`:

```rust
//! Por donde mira el editor.
//!
//! La camara del usuario (`camara`) trabaja en pixeles LOGICOS, los de
//! Excalidraw: zoom 1 es un pixel CSS, que Windows multiplica por su escala.
//! Con la pantalla al 150 % y la camara en pixeles fisicos, el mismo
//! `strokeWidth` salia un tercio mas fino que en excalidraw.com (D127).
//!
//! Puro: sin ventana ni GPU, para probarlo todo sin escritorio.

use pixpin_motor2d::camara::Camara;

/// La escala del monitor como factor. Un monitor que dijera 0 (no pasa, pero
/// un DPI mal leido no puede dejar la camara con zoom cero y dividir por el)
/// cuenta como 100 %.
pub fn escala_de(escala_por_cien: u32) -> f32 {
    if escala_por_cien == 0 {
        1.0
    } else {
        escala_por_cien as f32 / 100.0
    }
}

/// La camara con la que se pinta y se traduce el raton: la del usuario con
/// su zoom multiplicado por la escala del monitor.
///
/// Es la UNICA que ve el pintado y `a_evento`. Los ficheros no cambian: el
/// mundo sigue en las mismas unidades, solo cambia cuantos pixeles fisicos
/// ocupa cada una.
pub fn vista_efectiva(camara: &Camara, escala_por_cien: u32) -> Camara {
    Camara {
        x: camara.x,
        y: camara.y,
        zoom: camara.zoom * escala_de(escala_por_cien),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::vector::Punto2;

    #[test]
    fn a_escala_cien_la_vista_es_la_camara_del_usuario() {
        let c = Camara {
            x: 12.0,
            y: -7.0,
            zoom: 2.5,
        };
        assert_eq!(vista_efectiva(&c, 100), c);
    }

    #[test]
    fn ida_y_vuelta_de_pantalla_a_mundo_con_escala_150_y_zoom_arbitrario() {
        let c = Camara {
            x: -340.25,
            y: 118.5,
            zoom: 0.37,
        };
        let v = vista_efectiva(&c, 150);
        for p in [
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(123.5, -44.25),
            Punto2::nuevo(-900.0, 2048.0),
        ] {
            let vuelta = v.a_mundo(v.a_pantalla(p));
            assert!(
                (vuelta.x - p.x).abs() < 1e-2 && (vuelta.y - p.y).abs() < 1e-2,
                "{p:?} volvio como {vuelta:?}"
            );
        }
    }

    #[test]
    fn un_grosor_de_uno_a_escala_150_ocupa_uno_y_medio_lo_de_escala_100() {
        let c = Camara::nueva();
        let ancho = |escala| {
            let v = vista_efectiva(&c, escala);
            v.a_pantalla(Punto2::nuevo(1.0, 0.0)).x - v.a_pantalla(Punto2::nuevo(0.0, 0.0)).x
        };
        assert!((ancho(100) - 1.0).abs() < 1e-6);
        assert!((ancho(150) - 1.5).abs() < 1e-6);
        assert!((ancho(150) / ancho(100) - 1.5).abs() < 1e-6);
    }

    #[test]
    fn una_escala_de_cero_no_deja_la_camara_sin_zoom() {
        // Caso negativo: con zoom 0, `a_mundo` dividiria por cero y el raton
        // caeria en el infinito.
        let v = vista_efectiva(&Camara::nueva(), 0);
        assert_eq!(v.zoom, 1.0);
    }
}
```

En `ventana_editor.rs`, `mod pruebas`, junto a `el_raton_llega_al_motor_en_coordenadas_del_mundo` (:1486):

```rust
    #[test]
    fn a_escala_150_el_raton_cae_en_el_mundo_logico() {
        // D127: un clic 150 px fisicos a la derecha del origen es 100 px
        // logicos, que es donde Excalidraw pondria el punto.
        let efectiva = crate::navegacion::vista_efectiva(&Camara::nueva(), 150);
        let ev = EventoOverlay::BotonPulsado(Punto { x: 150, y: 75 });
        let Some(EventoGesto::Pulsar { p, .. }) = a_evento(&ev, &efectiva, Punto { x: 0, y: 0 })
        else {
            panic!("un boton pulsado es un Pulsar");
        };
        assert_eq!((p.x, p.y), (100.0, 50.0));
    }
```

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p pixpin navegacion -- --test-threads=1`
Expected: FAIL de compilación: `failed to resolve: could not find navegacion in the crate root` (en `a_escala_150_el_raton_cae_en_el_mundo_logico`), porque el `mod` aún no está declarado.

- [ ] **Step 3: Implementación**

1. `main.rs`: `mod navegacion;` entre `mod grabador;` y `mod overlay;`.
2. `ventana_editor.rs`, `use`: añadir `use crate::navegacion::vista_efectiva;`.
3. En `abrir`, sustituir :382 y :389 y :397 por:

```rust
    let camara = Camara::nueva();
    // D127: la camara del usuario va en pixeles logicos; `efectiva` es la
    // que pinta y traduce el raton. Se recalcula si cambia la escala.
    let mut escala_por_cien = monitor.escala_por_cien;
    let mut efectiva = vista_efectiva(&camara, escala_por_cien);
```

```rust
    let mut zoom_visto = efectiva.zoom;
```

```rust
    let mut caja = CajaHerramientas::colocar(area, area, escala_por_cien, &BOTONES_EDITOR);
```

4. Primera cosa dentro del `for` de eventos, tras `if hwnd != ventana.handle() { continue; }` (:409-411):

```rust
            // D127: con el editor abierto se puede cambiar la escala de
            // Windows. La tinta tiene que seguir del tamano de Excalidraw, y
            // la caja de herramientas, del de sus botones.
            if matches!(ev, EventoOverlay::CambioDpi) {
                if let Some(m) = pixpin_capture::enumerar_monitores()
                    .ok()
                    .and_then(|d| d.principal().copied())
                {
                    escala_por_cien = m.escala_por_cien;
                    efectiva = vista_efectiva(&camara, escala_por_cien);
                    caja =
                        CajaHerramientas::colocar(area, area, escala_por_cien, &BOTONES_EDITOR);
                    // La capa congelada se horneo a la escala vieja.
                    capa.soltar();
                    todo_sucio = true;
                    ventana.invalidar();
                }
                continue;
            }
```

5. En el resto de `abrir`, `camara` → `efectiva` en cada uso de conversión o pintado, y `monitor.escala_por_cien` → `escala_por_cien`:
   - :475-482 `a_evento(&ev, &efectiva, Punto { .. })`
   - :485 `gesto.evento(g, &mut escena, 1.0 / efectiva.zoom)`
   - :495-496 `efectiva.a_pantalla(..)`
   - :522 `let vista = efectiva.ventana(ancho_px, alto_px);`
   - :525 `camara: (efectiva.x, efectiva.y, efectiva.zoom),`
   - :531-532 `let origen = efectiva.a_pantalla(..); p.poner_vista((0.0, 0.0), efectiva.zoom, (origen.x, origen.y));`
   - :552 `efectiva.zoom,`
   - :576-592 `pedir_medida(.., &efectiva, .., &caja, escala_por_cien, ..)`
   - :631-646 `pintar(.., &efectiva, .., &caja, escala_por_cien, ..)`
   - :667-678 `decidir_zoom(efectiva.zoom, ..)` y `cache_tinta.fijar_escala(efectiva.zoom);`
   `pintar`, `pedir_medida` y `a_evento` no cambian de firma: reciben la cámara efectiva por su parámetro `camara`.

- [ ] **Step 4: Ejecutar**

Run: `cargo test -p pixpin -- --test-threads=1`
Expected: PASS, incluidas las 4 de `navegacion::pruebas` y `a_escala_150_el_raton_cae_en_el_mundo_logico`.

- [ ] **Step 5: fmt, clippy, suite y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --no-fail-fast -- --test-threads=1`
Expected: sin avisos; suite en verde.

```bash
git add apps/pixpin/src/navegacion.rs apps/pixpin/src/ventana_editor.rs apps/pixpin/src/main.rs
git commit -m "D127: la camara del editor en pixeles logicos, la tinta del tamano de Excalidraw" -m "Claude-Session: https://claude.ai/code/session_01FJbWm4jCHQtHwwndRDpLZJ"
```

---

### Task 2: Medir los fotogramas del editor (D129)

**Files:**
- Modify: `crates/pixpin-store/src/ajustes.rs` (`Rendimiento` :64-72; pruebas: `todo_cambiado` :340-377, nueva prueba junto a :593)
- Create: `apps/pixpin/src/medir_fotogramas.rs`
- Modify: `apps/pixpin/src/main.rs` (`mod medir_fotogramas;` entre `mod grabador;` y `mod navegacion;`; llamada del editor :716-728)
- Modify: `apps/pixpin/src/ventana_editor.rs` (`abrir` firma :351-355, bucle :406-691; `pintar` :762-897; prueba `abrir_y_cerrar_el_editor_no_revienta` :1632-1644)

**Interfaces:**
- Consumes: `vista_efectiva` (Tarea 1).
- Produces:
  - `pixpin_store::ajustes::Rendimiento { pub nivel: PreferenciaNivel, pub medir_fotogramas: bool }`
  - `pub const FOTOGRAMAS_POR_LINEA: u32 = 60`
  - `pub struct Pintado { pub pintar: Duration, pub presentar: Duration }`
  - `pub struct Vuelta { pub puntos: u32, pub vaciar: Duration, pub pintado: Option<Pintado>, pub esperar: Duration }`
  - `pub struct Campo { pub media: f32, pub maximo: f32 }`
  - `pub struct Resumen { pub fotogramas: u32, pub puntos: Campo, pub vaciar_ms: Campo, pub pintar_ms: Campo, pub presentar_ms: Campo, pub esperar_ms: Campo }` con `pub fn registrar(&self)`
  - `pub struct MedidorFotogramas` con `pub fn nuevo(activo: bool) -> Self` y `pub fn anotar(&mut self, v: Vuelta) -> Option<Resumen>`
  - `pub fn abrir(escena: Escena, ajustes_iman: enganche::Ajustes, nivel: pixpin_nivel::Nivel, medir_fotogramas: bool) -> Result<Escena>`
  - `fn pintar(..) -> Option<std::time::Duration>` (`None` = no se pintó; `Some(d)` = se presentó y presentar tardó `d`)

- [ ] **Step 1: Pruebas (fallan)**

En `ajustes.rs`, `mod pruebas`, tras `un_nivel_desconocido_da_error_en_vez_de_adivinar`:

```rust
    #[test]
    fn medir_fotogramas_se_lee_y_por_defecto_esta_apagado() {
        let a: Ajustes = toml::from_str("[rendimiento]\nmedir_fotogramas = true").unwrap();
        assert!(a.rendimiento.medir_fotogramas);
        assert_eq!(
            a.rendimiento.nivel,
            PreferenciaNivel::Auto,
            "lo que no se nombra conserva su valor"
        );
        // Caso negativo: un fichero de antes no tiene la clave y no mide nada.
        let viejo: Ajustes = toml::from_str("[rendimiento]\nnivel = \"ligero\"").unwrap();
        assert!(!viejo.rendimiento.medir_fotogramas);
    }
```

y en `todo_cambiado`, antes de `..Ajustes::default()`:

```rust
            rendimiento: Rendimiento {
                nivel: PreferenciaNivel::Ligero,
                medir_fotogramas: true,
            },
```

`apps/pixpin/src/medir_fotogramas.rs`, solo el módulo de pruebas por ahora (encima irá la implementación):

```rust
#[cfg(test)]
mod pruebas {
    use super::*;

    fn vuelta(puntos: u32, pintar_ms: u64) -> Vuelta {
        Vuelta {
            puntos,
            vaciar: Duration::from_millis(1),
            pintado: Some(Pintado {
                pintar: Duration::from_millis(pintar_ms),
                presentar: Duration::from_millis(10),
            }),
            esperar: Duration::from_millis(3),
        }
    }

    #[test]
    fn apagado_no_registra_nada_por_muchos_fotogramas_que_pasen() {
        let mut m = MedidorFotogramas::nuevo(false);
        for i in 0..600 {
            assert_eq!(m.anotar(vuelta(5, 2)), None, "vuelta {i}");
        }
    }

    #[test]
    fn encendido_da_una_linea_cada_sesenta_fotogramas_con_medias_y_maximos() {
        let mut m = MedidorFotogramas::nuevo(true);
        for i in 0..59 {
            let pintar = if i == 7 { 8 } else { 2 };
            assert_eq!(m.anotar(vuelta(5, pintar)), None, "fotograma {i}");
        }
        let r = m
            .anotar(vuelta(5, 2))
            .expect("el fotograma 60 cierra la linea");
        assert_eq!(r.fotogramas, FOTOGRAMAS_POR_LINEA);
        assert!((r.puntos.media - 5.0).abs() < 1e-3);
        assert_eq!(r.puntos.maximo, 5.0);
        // (59 x 2 + 8) / 60 = 2,1
        assert!((r.pintar_ms.media - 2.1).abs() < 1e-2, "{:?}", r.pintar_ms);
        assert!((r.pintar_ms.maximo - 8.0).abs() < 1e-3);
        assert!((r.presentar_ms.media - 10.0).abs() < 1e-3);
        assert!((r.vaciar_ms.media - 1.0).abs() < 1e-3);
        assert!((r.esperar_ms.media - 3.0).abs() < 1e-3);
        // Y vuelve a empezar de cero: la segunda linea no arrastra el maximo.
        for _ in 0..59 {
            assert_eq!(m.anotar(vuelta(1, 1)), None);
        }
        let r2 = m.anotar(vuelta(1, 1)).expect("segunda linea");
        assert!((r2.pintar_ms.maximo - 1.0).abs() < 1e-3);
    }

    #[test]
    fn las_vueltas_sin_fotograma_suman_al_siguiente_fotograma() {
        // Con un raton de 1000 Hz hay vueltas que vacian la cola y no pintan:
        // sus puntos y su espera son del fotograma que por fin se pinta.
        let mut m = MedidorFotogramas::nuevo(true);
        let sin_pintar = Vuelta {
            puntos: 7,
            vaciar: Duration::from_millis(1),
            pintado: None,
            esperar: Duration::from_millis(3),
        };
        assert_eq!(m.anotar(sin_pintar), None);
        assert_eq!(m.anotar(sin_pintar), None);
        for _ in 0..59 {
            let _ = m.anotar(vuelta(0, 2));
        }
        let r = m.anotar(vuelta(0, 2)).expect("sesenta fotogramas");
        assert_eq!(r.puntos.maximo, 14.0);
        assert!((r.esperar_ms.maximo - 9.0).abs() < 1e-3);
    }
}
```

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p pixpin-store ajustes -- --test-threads=1` y `cargo test -p pixpin medir_fotogramas -- --test-threads=1`
Expected: FAIL de compilación (`Rendimiento` no tiene `medir_fotogramas`; `MedidorFotogramas` no existe / módulo no declarado).

- [ ] **Step 3: Implementación**

`ajustes.rs`, `Rendimiento`:

```rust
pub struct Rendimiento {
    /// `auto` deja decidir a los hechos; `completo` y `ligero` fuerzan el
    /// nivel. Forzar `ligero` en una maquina potente es legitimo y util:
    /// es como se prueba la ruta ligera sin tener hardware flojo delante.
    pub nivel: PreferenciaNivel,
    /// Registrar cada 60 fotogramas del editor cuanto se tarda en vaciar la
    /// cola, pintar, presentar y esperar (D129). Es para diagnosticar en el
    /// equipo del usuario sin entrada sintetizada; apagado no registra nada.
    pub medir_fotogramas: bool,
}
```

`medir_fotogramas.rs`, encima de las pruebas:

```rust
//! Cuanto cuesta cada fotograma del editor, para medirlo en el equipo del
//! usuario (D129).
//!
//! La spec tiene tres sospechas de por que el lienzo va lento (vsync, un
//! `WM_PAINT` que no llega, recalcular el trazo entero) y cada una deja una
//! huella distinta en estos numeros. Se decide con ellos, no a ojo.
//!
//! Sin memoria dinamica: son sumas y maximos en campos fijos, y el registro
//! es una linea de `tracing` cada 60 fotogramas.

use std::time::Duration;

/// Cada cuantos fotogramas se escribe una linea.
pub const FOTOGRAMAS_POR_LINEA: u32 = 60;

/// Lo que costo un fotograma que si se pinto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pintado {
    /// Desde empezar hasta justo antes de presentar.
    pub pintar: Duration,
    /// `presentar_sincronizado`: con vsync, aqui se nota la espera al refresco.
    pub presentar: Duration,
}

/// Una vuelta del bucle del editor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vuelta {
    /// `RatonMovido` y `Muestra` recibidos en la vuelta.
    pub puntos: u32,
    /// Bombear mensajes y pasar la cola al gesto.
    pub vaciar: Duration,
    /// `None` si en esta vuelta no se pinto.
    pub pintado: Option<Pintado>,
    /// Dentro de `esperar_eventos`.
    pub esperar: Duration,
}

/// Media y maximo de una magnitud en una linea.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Campo {
    pub media: f32,
    pub maximo: f32,
}

/// Una linea del registro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resumen {
    pub fotogramas: u32,
    pub puntos: Campo,
    pub vaciar_ms: Campo,
    pub pintar_ms: Campo,
    pub presentar_ms: Campo,
    pub esperar_ms: Campo,
}

impl Resumen {
    /// Escribe la linea. Aparte de `anotar` para que las pruebas no dependan
    /// de un suscriptor de `tracing`.
    pub fn registrar(&self) {
        tracing::info!(
            fotogramas = self.fotogramas,
            puntos_media = self.puntos.media,
            puntos_max = self.puntos.maximo,
            vaciar_ms = self.vaciar_ms.media,
            vaciar_ms_max = self.vaciar_ms.maximo,
            pintar_ms = self.pintar_ms.media,
            pintar_ms_max = self.pintar_ms.maximo,
            presentar_ms = self.presentar_ms.media,
            presentar_ms_max = self.presentar_ms.maximo,
            esperar_ms = self.esperar_ms.media,
            esperar_ms_max = self.esperar_ms.maximo,
            "fotogramas del editor"
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Suma {
    total: f64,
    maximo: f32,
}

impl Suma {
    fn sumar(&mut self, v: f32) {
        self.total += v as f64;
        self.maximo = self.maximo.max(v);
    }

    fn campo(&self, n: u32) -> Campo {
        Campo {
            media: (self.total / n as f64) as f32,
            maximo: self.maximo,
        }
    }
}

fn ms(d: Duration) -> f32 {
    d.as_secs_f32() * 1000.0
}

/// El acumulador. Apagado, `anotar` vuelve enseguida y no guarda nada.
#[derive(Debug, Clone, Default)]
pub struct MedidorFotogramas {
    activo: bool,
    fotogramas: u32,
    // Lo que llevan las vueltas del fotograma que aun no se ha pintado.
    puntos_en_curso: u32,
    vaciar_en_curso: f32,
    esperar_en_curso: f32,
    puntos: Suma,
    vaciar: Suma,
    pintar: Suma,
    presentar: Suma,
    esperar: Suma,
}

impl MedidorFotogramas {
    pub fn nuevo(activo: bool) -> Self {
        Self {
            activo,
            ..Default::default()
        }
    }

    /// Apunta una vuelta. Devuelve la linea cuando se completan
    /// `FOTOGRAMAS_POR_LINEA` fotogramas pintados; entonces empieza de cero.
    ///
    /// Las vueltas que no pintan acumulan sus puntos, su vaciado y su espera
    /// en el fotograma siguiente: con un raton de 1000 Hz hay muchas, y
    /// contarlas como fotogramas escondería justo la sospecha S2.
    pub fn anotar(&mut self, v: Vuelta) -> Option<Resumen> {
        if !self.activo {
            return None;
        }
        self.puntos_en_curso = self.puntos_en_curso.saturating_add(v.puntos);
        self.vaciar_en_curso += ms(v.vaciar);
        self.esperar_en_curso += ms(v.esperar);
        let pintado = v.pintado?;
        self.puntos.sumar(self.puntos_en_curso as f32);
        self.vaciar.sumar(self.vaciar_en_curso);
        self.esperar.sumar(self.esperar_en_curso);
        self.pintar.sumar(ms(pintado.pintar));
        self.presentar.sumar(ms(pintado.presentar));
        self.puntos_en_curso = 0;
        self.vaciar_en_curso = 0.0;
        self.esperar_en_curso = 0.0;
        self.fotogramas += 1;
        if self.fotogramas < FOTOGRAMAS_POR_LINEA {
            return None;
        }
        let n = self.fotogramas;
        let r = Resumen {
            fotogramas: n,
            puntos: self.puntos.campo(n),
            vaciar_ms: self.vaciar.campo(n),
            pintar_ms: self.pintar.campo(n),
            presentar_ms: self.presentar.campo(n),
            esperar_ms: self.esperar.campo(n),
        };
        *self = Self::nuevo(true);
        Some(r)
    }
}
```

`main.rs`: `mod medir_fotogramas;` y la llamada de :717-721:

```rust
                match ventana_editor::abrir(
                    pixpin_motor2d::Escena::nueva(),
                    config.enganche,
                    decision.nivel,
                    config.rendimiento.medir_fotogramas,
                ) {
```

`ventana_editor.rs`:

1. `abrir` gana `medir_fotogramas: bool` tras `nivel` (y su doc: «`medir_fotogramas` enciende el registro de D129»).
2. Antes de `'bucle: loop {`: `let mut medidor = crate::medir_fotogramas::MedidorFotogramas::nuevo(medir_fotogramas);`
3. Cabeza del bucle:

```rust
    'bucle: loop {
        // D129: se mide siempre (unos nanosegundos por `Instant::now`); solo
        // se acumula y registra con la opcion encendida.
        let t_vuelta = std::time::Instant::now();
        let mut puntos = 0u32;
        pixpin_shell::overlay::bombear_pendientes();
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            if matches!(ev, EventoOverlay::RatonMovido(_) | EventoOverlay::Muestra(_)) {
                puntos += 1;
            }
```

(el bloque `CambioDpi` de la Tarea 1 va justo después).
4. Tras cerrar el `for` de eventos (antes de `if hay_que_pintar {`): `let vaciar = t_vuelta.elapsed();` y `let mut pintado_medido = None;`.
5. La llamada a `pintar` (:631-659) queda:

```rust
            let t_pintar = std::time::Instant::now();
            let pintado = pintar(
                &mut motor,
                &superficie,
                &escena,
                &efectiva,
                &gesto,
                &mut cache,
                &mut cache_tinta,
                &rejilla,
                &capa,
                &caja,
                escala_por_cien,
                ancho_px,
                alto_px,
                zona,
                |_| {},
            );
            match pintado {
                Some(presentar) => {
                    pintado_medido = Some(crate::medir_fotogramas::Pintado {
                        pintar: t_pintar.elapsed().saturating_sub(presentar),
                        presentar,
                    });
                    hay_que_pintar = false;
                    sucio = None;
                    todo_sucio = false;
                }
                None => {
                    // (comentario existente de :653-657 sin cambios)
                    todo_sucio = true;
                }
            }
```

6. Final del bucle (:690):

```rust
        let t_esperar = std::time::Instant::now();
        pixpin_shell::overlay::esperar_eventos(decision.tope_ms);
        if let Some(linea) = medidor.anotar(crate::medir_fotogramas::Vuelta {
            puntos,
            vaciar,
            pintado: pintado_medido,
            esperar: t_esperar.elapsed(),
        }) {
            linea.registrar();
        }
    }
```

7. `pintar`: devuelve `Option<std::time::Duration>`; `let Ok(destino) = superficie.empezar(motor) else { return None; };` y el final:

```rust
    let t_presentar = std::time::Instant::now();
    let _ = superficie.presentar_sincronizado(zona);
    Some(t_presentar.elapsed())
```

Actualizar su doc («Devuelve `None` si `superficie.empezar` falló; si pintó, cuánto tardó presentar»). `pedir_medida` ignora el valor devuelto como hasta ahora.
8. Prueba `abrir_y_cerrar_el_editor_no_revienta`: añadir `false` como cuarto argumento.

- [ ] **Step 4: Ejecutar**

Run: `cargo test -p pixpin-store -p pixpin -- --test-threads=1`
Expected: PASS (`medir_fotogramas_se_lee_y_por_defecto_esta_apagado`, `ningun_ajuste_se_queda_sin_guardar` con el campo nuevo, las 3 de `medir_fotogramas::pruebas`).

- [ ] **Step 5: fmt, clippy, suite y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --no-fail-fast -- --test-threads=1`
Expected: verde.

```bash
git add crates/pixpin-store/src/ajustes.rs apps/pixpin/src/medir_fotogramas.rs apps/pixpin/src/ventana_editor.rs apps/pixpin/src/main.rs
git commit -m "D129: medir_fotogramas registra cada 60 fotogramas lo que cuesta vaciar, pintar, presentar y esperar" -m "Claude-Session: https://claude.ai/code/session_01FJbWm4jCHQtHwwndRDpLZJ"
```

---

### Task 3: Entrega de la Parte 0a al usuario (D130)

**Files:**
- Ninguno (compilación y mensaje). **Sin commit.**

**Interfaces:**
- Consumes: Tareas 1 y 2.
- Produces: `target/release/pixpinmax.exe` y las instrucciones para el usuario.

- [ ] **Step 1: Suite, fmt y clippy**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --no-fail-fast -- --test-threads=1`
Expected: verde (la suite actual más las 9 pruebas nuevas de las Tareas 1 y 2).

- [ ] **Step 2: Binario**

Run: `cargo build --release -p pixpin`
Expected: `Finished release`, fichero `target/release/pixpinmax.exe` con fecha de ahora. **No** ejecutarlo.

- [ ] **Step 3: Mensaje al usuario (literal)**

```
Parte 0a lista en target\release\pixpinmax.exe (rama lienzo-desde-pin).

1. Cierra PixPin si esta abierto y abre pixpinmax.toml (junto al .exe en modo
   portable; si no, en %APPDATA%\PixPinMax). Añade o completa:

   [rendimiento]
   medir_fotogramas = true

2. Arranca el nuevo pixpinmax.exe, abre el Editor desde la bandeja y dibuja
   rapido durante 10 segundos con el raton (y otros 10 con el lapiz si tienes).
   Cierra con Esc.
3. Mandame el ultimo fichero de la carpeta registros\ (misma carpeta que el
   .toml). Las lineas que interesan dicen "fotogramas del editor".
4. Mira tambien si la tinta al grosor medio (tecla 2) se ve del mismo tamaño
   que en excalidraw.com con la pantalla al mismo zoom.

Con esos numeros decido la Parte 0b (vsync, repintado o recalculo del trazo).
```

- [ ] **Step 4: Parar y esperar**

La Parte 1 no depende de 0b y puede seguir (Tarea 4 en adelante); la Parte 0b no se toca sin el registro.

---

# Parte 1 — lienzo desde el pin

### Task 4: Botón central y ventana por encima en el overlay (D136, D145)

**Files:**
- Modify: `crates/pixpin-shell/src/overlay.rs` (`EventoOverlay` :51-88; `impl VentanaOverlay` tras `mover` :179-195; WndProc tras `WM_LBUTTONUP | WM_RBUTTONUP` :697-712; pruebas :857-1007)
- Modify: `apps/pixpin/src/overlay.rs:725-729` (brazo exhaustivo)
- Modify: `apps/pixpin/src/ventana_editor.rs:376-377` (`ventana.mostrar(); ventana.enfocar();`)

**Interfaces:**
- Produces:
  - `EventoOverlay::BotonCentralPulsado(Punto)`, `EventoOverlay::BotonCentralSoltado(Punto)` (escritorio virtual, como `BotonPulsado`)
  - `impl VentanaOverlay { pub fn traer_encima(&self) }`

- [ ] **Step 1: Pruebas (fallan)**

En `overlay.rs`, `mod pruebas`:

```rust
    #[test]
    fn el_boton_central_viaja_como_evento_copiable_y_comparable() {
        // `EventoOverlay` cruza colas por valor y se compara en las pruebas
        // del editor: las variantes nuevas no pueden quitarle Copy ni Eq.
        fn copiable<T: Copy + Eq>(_: T) {}
        let p = EventoOverlay::BotonCentralPulsado(Punto { x: 3, y: 4 });
        copiable(p);
        assert_ne!(p, EventoOverlay::BotonCentralSoltado(Punto { x: 3, y: 4 }));
        assert_ne!(p, EventoOverlay::BotonPulsado(Punto { x: 3, y: 4 }));
    }

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn traer_encima_pone_la_ventana_sobre_otra_topmost_creada_despues() {
        // D145: el lienzo tiene que quedar por encima de los pines, que
        // tambien son TOPMOST. Se crea `b` despues (queda encima) y se
        // comprueba que `traer_encima` pone `a` por delante.
        use windows::Win32::UI::WindowsAndMessaging::{GW_HWNDPREV, GetWindow};
        let rect = Rect {
            x: 0,
            y: 0,
            ancho: 100,
            alto: 100,
        };
        let a = VentanaOverlay::nueva(rect).unwrap();
        let b = VentanaOverlay::nueva(rect).unwrap();
        a.traer_encima();
        let mut actual = b.handle();
        let mut a_por_encima = false;
        for _ in 0..10_000 {
            // SAFETY: consulta del orden Z; si la cadena se corta, se para.
            match unsafe { GetWindow(actual, GW_HWNDPREV) } {
                Ok(h) if !h.is_invalid() => {
                    if h == a.handle() {
                        a_por_encima = true;
                        break;
                    }
                    actual = h;
                }
                _ => break,
            }
        }
        assert!(a_por_encima, "la ventana traida encima sigue debajo");
    }
```

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p pixpin-shell overlay -- --test-threads=1`
Expected: FAIL de compilación (`no variant named BotonCentralPulsado`, `no method named traer_encima`).

- [ ] **Step 3: Implementación**

Variantes, tras `BotonSoltado(Punto),`:

```rust
    /// Boton central (WM_MBUTTONDOWN / WM_MBUTTONUP). El editor lo usa para
    /// desplazar el lienzo como Excalidraw (D136). Escritorio virtual, como
    /// `BotonPulsado`.
    BotonCentralPulsado(Punto),
    BotonCentralSoltado(Punto),
```

Método, tras `mover`:

```rust
    /// La pone por delante de las demas ventanas TOPMOST sin activarla ni
    /// moverla (D145). Crearla despues suele bastar, pero un pin que se
    /// reordene (su paleta, un clic) quedaria encima del lienzo.
    pub fn traer_encima(&self) {
        // SAFETY: SetWindowPos sobre la ventana propia y viva; solo cambia el
        // orden Z (sin mover, sin redimensionar, sin robar el foco).
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
```

WndProc, tras el brazo `WM_LBUTTONUP | WM_RBUTTONUP`:

```rust
        WM_MBUTTONDOWN => {
            // SAFETY: SetCapture sobre ventana propia: arrastrar el lienzo
            // con el boton central no se pierde al salir del borde.
            unsafe { SetCapture(hwnd) };
            encolar(EventoOverlay::BotonCentralPulsado(punto(lparam)));
            LRESULT(0)
        }
        WM_MBUTTONUP => {
            // SAFETY: libera la captura tomada arriba.
            unsafe {
                let _ = ReleaseCapture();
            }
            encolar(EventoOverlay::BotonCentralSoltado(punto(lparam)));
            LRESULT(0)
        }
```

`apps/pixpin/src/overlay.rs:725-729`:

```rust
        EventoOverlay::Rueda(_)
        | EventoOverlay::Caracter(_)
        | EventoOverlay::TeclaSoltada(_)
        | EventoOverlay::Atajo(_)
        | EventoOverlay::Muestra(_)
        | EventoOverlay::BotonCentralPulsado(_)
        | EventoOverlay::BotonCentralSoltado(_) => Continuar::Si,
```

`ventana_editor.rs:376-377`:

```rust
    ventana.mostrar();
    // D145: el lienzo que se abre desde un pin tiene que taparlo.
    ventana.traer_encima();
    ventana.enfocar();
```

- [ ] **Step 4: Ejecutar**

Run: `cargo test -p pixpin-shell -p pixpin -- --test-threads=1`
Expected: PASS; la prueba `#[ignore]` no corre (la ejecuta el usuario con `--ignored` si quiere).

- [ ] **Step 5: fmt, clippy, suite y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --no-fail-fast -- --test-threads=1`

```bash
git add crates/pixpin-shell/src/overlay.rs apps/pixpin/src/overlay.rs apps/pixpin/src/ventana_editor.rs
git commit -m "Overlay: boton central y traer encima, para mover el lienzo y taparlo sobre los pines" -m "Claude-Session: https://claude.ai/code/session_01FJbWm4jCHQtHwwndRDpLZJ"
```

---

### Task 5: Moverse por el lienzo como en Excalidraw (D136, D137)

**Files:**
- Modify: `apps/pixpin/src/navegacion.rs` (añadir encima de `mod pruebas` y ampliar las pruebas)
- Modify: `apps/pixpin/src/ventana_editor.rs` (`use`; `abrir`: `let camara` de la Tarea 1 pasa a `let mut camara`; nuevo bloque al principio del `for` de eventos, después del de `CambioDpi`)

**Interfaces:**
- Consumes: `EventoOverlay::{Rueda, Tecla, TeclaSoltada, BotonPulsado, BotonSoltado, RatonMovido, Muestra, BotonCentralPulsado, BotonCentralSoltado}` (Tarea 4), `vista_efectiva`, `escala_de` (Tarea 1), `Camara::{desplazar, acercar_en}`, `pixpin_shell::entrada::modificadores_pulsados() -> pixpin_shell::Modificadores { ctrl, alt, shift, win }`.
- Produces:
  - `pub const VK_ESPACIO: u32 = 0x20`, `pub const MUESCA: i32 = 120`, `pub const PASO_ZOOM: f32 = 0.1`, `pub const ZOOM_MINIMO: f32 = 0.1`, `pub const ZOOM_MAXIMO: f32 = 30.0`, `pub const PX_POR_MUESCA: f32 = 100.0`
  - `pub fn zoom_de_rueda(zoom: f32, delta_rueda: i32) -> f32`
  - `pub struct Modificadores { pub ctrl: bool, pub shift: bool }`
  - `pub enum Accion { Desplazar { dx: f32, dy: f32 }, ZoomRueda { foco: Punto2, delta: i32 } }`
  - `pub struct Respuesta { pub consumido: bool, pub accion: Option<Accion> }`
  - `pub struct Navegador` con `pub fn nuevo() -> Self`, `pub fn evento(&mut self, ev: &EventoOverlay, origen: Punto, escala_por_cien: u32, m: Modificadores, en_reposo: bool) -> Respuesta`, `pub fn arrastrando(&self) -> bool`
  - `pub fn aplicar(camara: &mut Camara, accion: Accion) -> bool`

- [ ] **Step 1: Pruebas puras (fallan)**

En `navegacion.rs`, `mod pruebas` (mientras falla, añadir en el módulo de pruebas `use pixpin_geom::Punto;` y `use pixpin_shell::overlay::EventoOverlay;`; el Step 3 los sube al módulo):

```rust
    const ORIGEN: Punto = Punto { x: 0, y: 0 };
    const NADA: Modificadores = Modificadores {
        ctrl: false,
        shift: false,
    };

    #[test]
    fn el_paso_de_zoom_de_la_rueda_es_el_de_excalidraw() {
        // App.tsx:14024-14045 @afa3a65 con deltaY = -100 por muesca arriba.
        assert!((zoom_de_rueda(1.0, MUESCA) - 1.1).abs() < 1e-4);
        assert!((zoom_de_rueda(1.0, -MUESCA) - 0.9).abs() < 1e-4);
        // Por encima del 100 % el paso crece con log10(zoom).
        assert!((zoom_de_rueda(2.0, MUESCA) - 2.40103).abs() < 1e-4);
        // Topes: MIN_ZOOM y MAX_ZOOM.
        assert_eq!(zoom_de_rueda(30.0, MUESCA), ZOOM_MAXIMO);
        assert_eq!(zoom_de_rueda(0.1, -MUESCA), ZOOM_MINIMO);
        // Caso negativo: una rueda sin giro no cambia nada.
        assert_eq!(zoom_de_rueda(1.7, 0), 1.7);
    }

    #[test]
    fn la_rueda_sola_desplaza_en_vertical_como_excalidraw() {
        let mut n = Navegador::nuevo();
        let r = n.evento(&EventoOverlay::Rueda(-MUESCA), ORIGEN, 100, NADA, true);
        assert!(r.consumido);
        assert_eq!(r.accion, Some(Accion::Desplazar { dx: 0.0, dy: -100.0 }));
        let mut c = Camara::nueva();
        assert!(aplicar(&mut c, r.accion.unwrap()));
        assert_eq!((c.x, c.y), (0.0, 100.0), "rueda abajo = mirar mas abajo");
    }

    #[test]
    fn shift_y_rueda_desplaza_en_horizontal() {
        let mut n = Navegador::nuevo();
        let shift = Modificadores {
            ctrl: false,
            shift: true,
        };
        let r = n.evento(&EventoOverlay::Rueda(-MUESCA), ORIGEN, 100, shift, true);
        assert_eq!(r.accion, Some(Accion::Desplazar { dx: -100.0, dy: 0.0 }));
    }

    #[test]
    fn control_y_rueda_acerca_hacia_el_cursor() {
        let mut n = Navegador::nuevo();
        let _ = n.evento(
            &EventoOverlay::RatonMovido(Punto { x: 300, y: 200 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        let ctrl = Modificadores {
            ctrl: true,
            shift: false,
        };
        let r = n.evento(&EventoOverlay::Rueda(MUESCA), ORIGEN, 100, ctrl, true);
        let Some(accion @ Accion::ZoomRueda { foco, .. }) = r.accion else {
            panic!("Ctrl+rueda es zoom, dio {:?}", r.accion);
        };
        let mut c = Camara::nueva();
        let antes = c.a_mundo(foco);
        assert!(aplicar(&mut c, accion));
        assert!((c.zoom - 1.1).abs() < 1e-4);
        let despues = c.a_mundo(foco);
        assert!((antes.x - despues.x).abs() < 1e-3 && (antes.y - despues.y).abs() < 1e-3);
    }

    #[test]
    fn espacio_y_arrastrar_desplaza_y_no_llega_al_gesto() {
        let mut n = Navegador::nuevo();
        let espacio = EventoOverlay::Tecla {
            vk: VK_ESPACIO,
            shift: false,
            ctrl: false,
            alt: false,
        };
        assert!(n.evento(&espacio, ORIGEN, 100, NADA, true).consumido);
        let p = n.evento(
            &EventoOverlay::BotonPulsado(Punto { x: 100, y: 100 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        assert!(p.consumido && n.arrastrando());
        let m = n.evento(
            &EventoOverlay::RatonMovido(Punto { x: 130, y: 110 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        assert!(m.consumido);
        assert_eq!(m.accion, Some(Accion::Desplazar { dx: 30.0, dy: 10.0 }));
        let s = n.evento(
            &EventoOverlay::BotonSoltado(Punto { x: 130, y: 110 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        assert!(s.consumido && !n.arrastrando());
        let _ = n.evento(&EventoOverlay::TeclaSoltada(VK_ESPACIO), ORIGEN, 100, NADA, true);
        // Soltado el espacio, el siguiente clic vuelve a ser del gesto.
        let otro = n.evento(
            &EventoOverlay::BotonPulsado(Punto { x: 5, y: 5 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        assert!(!otro.consumido);
    }

    #[test]
    fn el_boton_central_arrastra_sin_espacio() {
        let mut n = Navegador::nuevo();
        let _ = n.evento(
            &EventoOverlay::BotonCentralPulsado(Punto { x: 10, y: 10 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        let m = n.evento(
            &EventoOverlay::RatonMovido(Punto { x: 4, y: 30 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        assert_eq!(m.accion, Some(Accion::Desplazar { dx: -6.0, dy: 20.0 }));
        let s = n.evento(
            &EventoOverlay::BotonCentralSoltado(Punto { x: 4, y: 30 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        assert!(s.consumido && !n.arrastrando());
    }

    #[test]
    fn sin_espacio_el_clic_y_el_movimiento_llegan_al_gesto() {
        let mut n = Navegador::nuevo();
        for ev in [
            EventoOverlay::BotonPulsado(Punto { x: 1, y: 1 }),
            EventoOverlay::RatonMovido(Punto { x: 9, y: 9 }),
            EventoOverlay::BotonSoltado(Punto { x: 9, y: 9 }),
        ] {
            let r = n.evento(&ev, ORIGEN, 100, NADA, true);
            assert!(!r.consumido, "{ev:?} no es del navegador");
            assert_eq!(r.accion, None);
        }
    }

    #[test]
    fn con_un_trazo_en_curso_el_espacio_no_roba_el_arrastre() {
        // Caso negativo: pulsar espacio a mitad de un trazo y seguir
        // moviendo tiene que seguir dibujando, no mover el lienzo.
        let mut n = Navegador::nuevo();
        let espacio = EventoOverlay::Tecla {
            vk: VK_ESPACIO,
            shift: false,
            ctrl: false,
            alt: false,
        };
        let _ = n.evento(&espacio, ORIGEN, 100, NADA, false);
        let p = n.evento(
            &EventoOverlay::BotonPulsado(Punto { x: 1, y: 1 }),
            ORIGEN,
            100,
            NADA,
            false,
        );
        assert!(!p.consumido && !n.arrastrando());
    }

    #[test]
    fn a_escala_150_el_lienzo_sigue_al_raton_pixel_a_pixel() {
        let mut n = Navegador::nuevo();
        let _ = n.evento(
            &EventoOverlay::BotonCentralPulsado(Punto { x: 0, y: 0 }),
            ORIGEN,
            150,
            NADA,
            true,
        );
        let m = n.evento(
            &EventoOverlay::RatonMovido(Punto { x: 150, y: 0 }),
            ORIGEN,
            150,
            NADA,
            true,
        );
        let mut c = Camara::nueva();
        let punto = Punto2::nuevo(40.0, 0.0);
        let antes = vista_efectiva(&c, 150).a_pantalla(punto);
        assert!(aplicar(&mut c, m.accion.unwrap()));
        let despues = vista_efectiva(&c, 150).a_pantalla(punto);
        assert!((despues.x - antes.x - 150.0).abs() < 1e-3);
    }
```

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p pixpin navegacion -- --test-threads=1`
Expected: FAIL de compilación (`cannot find function zoom_de_rueda`, `Navegador`, `Accion`...).

- [ ] **Step 3: Implementación**

`navegacion.rs`: arriba, junto al `use` de `Camara`, añadir `use pixpin_geom::Punto;`, `use pixpin_motor2d::vector::Punto2;` y `use pixpin_shell::overlay::EventoOverlay;` (en `mod pruebas` sobran entonces los `use` de `Punto2`, `Punto` y `EventoOverlay`: los trae `use super::*;`, así que se quitan). Tras `vista_efectiva`:

```rust
/// Codigo virtual de la barra espaciadora.
pub const VK_ESPACIO: u32 = 0x20;
/// `WHEEL_DELTA`: lo que Windows manda por muesca.
pub const MUESCA: i32 = 120;
/// `ZOOM_STEP` de Excalidraw (packages/common/src/constants.ts:349 @afa3a65).
pub const PASO_ZOOM: f32 = 0.1;
/// `MIN_ZOOM` y `MAX_ZOOM` (constants.ts:350-351).
pub const ZOOM_MINIMO: f32 = 0.1;
pub const ZOOM_MAXIMO: f32 = 30.0;
/// El `deltaY` que Chromium en Windows da por muesca con el ajuste de
/// fabrica (tres lineas): 100 px CSS. Excalidraw no ve la muesca, ve esto.
pub const PX_POR_MUESCA: f32 = 100.0;

/// El `deltaY` del navegador para un giro de Windows. Signo al reves:
/// Windows cuenta positivo alejando la rueda; el navegador, bajando la pagina.
fn delta_y_css(delta_rueda: i32) -> f32 {
    -(delta_rueda as f32) * PX_POR_MUESCA / MUESCA as f32
}

/// El zoom tras un giro con Ctrl. Porte de `handleWheel`
/// (packages/excalidraw/components/App.tsx:14024-14045 @afa3a65) y de
/// `getNormalizedZoom` (packages/excalidraw/scene/normalize.ts:7-9), MIT,
/// Copyright (c) 2020 Excalidraw.
pub fn zoom_de_rueda(zoom: f32, delta_rueda: i32) -> f32 {
    let delta_y = delta_y_css(delta_rueda);
    if delta_y == 0.0 {
        return zoom;
    }
    let signo = delta_y.signum();
    let paso_maximo = PASO_ZOOM * 100.0;
    let absoluto = delta_y.abs();
    let delta = if absoluto > paso_maximo {
        paso_maximo * signo
    } else {
        delta_y
    };
    let mut nuevo = zoom - delta / 100.0;
    // Mas acercado, pasos mayores (solo por encima del 100 %).
    nuevo += zoom.max(1.0).log10() * -signo * (absoluto / 20.0).min(1.0);
    nuevo = nuevo.max(ZOOM_MINIMO);
    ((nuevo * 1e6).round() / 1e6).clamp(ZOOM_MINIMO, ZOOM_MAXIMO)
}

/// Los modificadores que importan a la rueda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modificadores {
    pub ctrl: bool,
    pub shift: bool,
}

/// Lo que hay que hacerle a la camara del usuario.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Accion {
    /// Arrastrar el lienzo `dx`, `dy` pixeles LOGICOS, con el convenio de
    /// `Camara::desplazar` (el dibujo sigue a la mano).
    Desplazar { dx: f32, dy: f32 },
    /// Ctrl+rueda: el zoom lo decide `zoom_de_rueda` con el de la camara, y
    /// `foco` (pixeles logicos de la ventana) se queda quieto.
    ZoomRueda { foco: Punto2, delta: i32 },
}

/// Si el evento era del navegador y que hay que hacer.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Respuesta {
    /// `true`: el evento no puede llegar al gesto ni a la caja.
    pub consumido: bool,
    pub accion: Option<Accion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BotonArrastre {
    Principal,
    Central,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Arrastre {
    boton: BotonArrastre,
    /// Ultima posicion vista, en el escritorio virtual (pixeles fisicos).
    anterior: Punto,
}

/// Rueda, espacio + arrastrar y boton central, con la convencion de
/// Excalidraw (D136): `handleCanvasPanUsingWheelOrSpaceDrag`,
/// App.tsx:9075-9093, y la barra en App.tsx:5850-5853.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Navegador {
    espacio: bool,
    arrastre: Option<Arrastre>,
    /// Donde estaba el raton la ultima vez, en pixeles logicos de la
    /// ventana: el foco del zoom con Ctrl+rueda, que no trae posicion.
    ultimo: Punto2,
}

impl Navegador {
    pub fn nuevo() -> Self {
        Self::default()
    }

    pub fn arrastrando(&self) -> bool {
        self.arrastre.is_some()
    }

    /// `en_reposo` es `Gesto::en_reposo`: con un trazo o un arrastre en curso
    /// no se empieza a mover el lienzo (el clic es del gesto).
    pub fn evento(
        &mut self,
        ev: &EventoOverlay,
        origen: Punto,
        escala_por_cien: u32,
        m: Modificadores,
        en_reposo: bool,
    ) -> Respuesta {
        let escala = escala_de(escala_por_cien);
        let consumido = Respuesta {
            consumido: true,
            accion: None,
        };
        match *ev {
            EventoOverlay::Tecla { vk: VK_ESPACIO, .. } => {
                self.espacio = true;
                consumido
            }
            EventoOverlay::TeclaSoltada(VK_ESPACIO) => {
                self.espacio = false;
                consumido
            }
            EventoOverlay::BotonPulsado(p) if self.espacio && en_reposo => {
                self.arrastre = Some(Arrastre {
                    boton: BotonArrastre::Principal,
                    anterior: p,
                });
                consumido
            }
            EventoOverlay::BotonCentralPulsado(p) => {
                if en_reposo {
                    self.arrastre = Some(Arrastre {
                        boton: BotonArrastre::Central,
                        anterior: p,
                    });
                }
                consumido
            }
            EventoOverlay::RatonMovido(p) => {
                self.ultimo = Punto2::nuevo(
                    (p.x - origen.x) as f32 / escala,
                    (p.y - origen.y) as f32 / escala,
                );
                match &mut self.arrastre {
                    Some(a) => {
                        let dx = (p.x - a.anterior.x) as f32 / escala;
                        let dy = (p.y - a.anterior.y) as f32 / escala;
                        a.anterior = p;
                        Respuesta {
                            consumido: true,
                            accion: Some(Accion::Desplazar { dx, dy }),
                        }
                    }
                    None => Respuesta::default(),
                }
            }
            // Mientras se arrastra el lienzo, los puntos finos tampoco son
            // tinta.
            EventoOverlay::Muestra(_) if self.arrastre.is_some() => consumido,
            EventoOverlay::BotonSoltado(_)
                if matches!(
                    self.arrastre,
                    Some(Arrastre {
                        boton: BotonArrastre::Principal,
                        ..
                    })
                ) =>
            {
                self.arrastre = None;
                consumido
            }
            EventoOverlay::BotonCentralSoltado(_) => {
                if matches!(
                    self.arrastre,
                    Some(Arrastre {
                        boton: BotonArrastre::Central,
                        ..
                    })
                ) {
                    self.arrastre = None;
                }
                consumido
            }
            EventoOverlay::Rueda(delta) => {
                let accion = if m.ctrl {
                    Accion::ZoomRueda {
                        foco: self.ultimo,
                        delta,
                    }
                } else if m.shift {
                    // App.tsx:14067-14072: Shift lleva el giro a horizontal.
                    Accion::Desplazar {
                        dx: -delta_y_css(delta),
                        dy: 0.0,
                    }
                } else {
                    // App.tsx:14075-14078: scrollY - deltaY / zoom.
                    Accion::Desplazar {
                        dx: 0.0,
                        dy: -delta_y_css(delta),
                    }
                };
                Respuesta {
                    consumido: true,
                    accion: Some(accion),
                }
            }
            _ => Respuesta::default(),
        }
    }
}

/// Aplica la accion a la camara del usuario (logica). Devuelve si cambio.
pub fn aplicar(camara: &mut Camara, accion: Accion) -> bool {
    match accion {
        Accion::Desplazar { dx, dy } => {
            if dx == 0.0 && dy == 0.0 {
                return false;
            }
            camara.desplazar(dx, dy);
            true
        }
        Accion::ZoomRueda { foco, delta } => {
            let nuevo = zoom_de_rueda(camara.zoom, delta);
            camara.acercar_en(foco, nuevo / camara.zoom)
        }
    }
}
```

`ventana_editor.rs`:
1. `use crate::navegacion::{self, vista_efectiva};` (sustituye al de la Tarea 1).
2. `let camara = Camara::nueva();` → `let mut camara = Camara::nueva();` y, justo después de `let mut efectiva = ...`: `let mut navegador = navegacion::Navegador::nuevo();`
3. En el `for` de eventos, tras el bloque `CambioDpi` y antes del paso 0 (la caja):

```rust
            // D136: moverse por el lienzo va ANTES que la caja y que el
            // gesto. Un clic con el espacio pulsado arrastra el lienzo aunque
            // caiga sobre un boton, como en Excalidraw, y lo que el navegador
            // consume no puede empezar un trazo. Los modificadores solo se
            // leen para la rueda: sondearlos con cada muestra del lapiz seria
            // pagar cinco llamadas al sistema mil veces por segundo.
            let mods = if matches!(ev, EventoOverlay::Rueda(_)) {
                let m = pixpin_shell::entrada::modificadores_pulsados();
                navegacion::Modificadores {
                    ctrl: m.ctrl,
                    shift: m.shift,
                }
            } else {
                navegacion::Modificadores::default()
            };
            let nav = navegador.evento(
                &ev,
                Punto {
                    x: area.x,
                    y: area.y,
                },
                escala_por_cien,
                mods,
                gesto.en_reposo(),
            );
            if let Some(accion) = nav.accion {
                if navegacion::aplicar(&mut camara, accion) {
                    efectiva = vista_efectiva(&camara, escala_por_cien);
                    // La capa congelada se da por invalida sola: su Estampa
                    // lleva la camara. `decidir_zoom` rehace la tinta nitida
                    // cuando el zoom se quede quieto.
                    todo_sucio = true;
                    ventana.invalidar();
                }
            }
            if nav.consumido {
                if navegador.arrastrando() {
                    ventana.poner_cursor(FormaCursorWin::Mover);
                }
                continue;
            }
```

(D137: la bandeja usa el mismo `abrir`, así que gana esto sin más cambios.)

- [ ] **Step 4: Ejecutar**

Run: `cargo test -p pixpin navegacion -- --test-threads=1`
Expected: PASS (13 pruebas en `navegacion::pruebas`).

- [ ] **Step 5: fmt, clippy, suite y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --no-fail-fast -- --test-threads=1`

```bash
git add apps/pixpin/src/navegacion.rs apps/pixpin/src/ventana_editor.rs
git commit -m "D136: rueda, Shift, Ctrl, espacio y boton central mueven el lienzo como en Excalidraw" -m "Claude-Session: https://claude.ai/code/session_01FJbWm4jCHQtHwwndRDpLZJ"
```

---

### Task 6: Bitmaps con alfa, tamaño máximo e interpolación en `pixpin-render` (D138, D139, D141)

**Files:**
- Modify: `crates/pixpin-render/src/motor.rs` (`use` :12-30; `bitmap_desde_pixeles` :222-257; pruebas :404-511)
- Modify: `crates/pixpin-render/src/lienzo.rs` (`use` :8-13; `Pintor::bitmap` :404-426; pruebas desde :952)
- Modify: `crates/pixpin-render/src/lib.rs:11,20` (exportar `premultiplicar` e `Interpolacion`)

**Interfaces:**
- Produces:
  - `pub fn premultiplicar(rgba: &[u8]) -> Vec<u8>`
  - `impl MotorRender { pub fn bitmap_desde_pixeles_premultiplicado(&self, ancho: u32, alto: u32, rgba: &[u8]) -> Result<ID2D1Bitmap1, ErrorRender>; pub fn lado_maximo_bitmap(&self) -> u32 }`
  - `pub enum Interpolacion { Vecino, Lineal, Cubica }`
  - `impl Pintor { pub fn bitmap_con(&self, b: &ID2D1Bitmap1, destino: RectF, fuente: Option<RectF>, modo: Interpolacion) }`

- [ ] **Step 1: Pruebas (fallan)**

`motor.rs`, `mod pruebas`:

```rust
    #[test]
    fn premultiplicar_multiplica_el_color_por_el_alfa_y_no_toca_lo_opaco() {
        let v = premultiplicar(&[255, 0, 0, 128, 10, 20, 30, 0, 1, 2, 3, 255]);
        assert_eq!(&v[0..4], &[128, 0, 0, 128]);
        assert_eq!(&v[4..8], &[0, 0, 0, 0], "transparente total es negro cero");
        assert_eq!(&v[8..12], &[1, 2, 3, 255], "lo opaco queda igual");
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn un_png_transparente_se_pinta_sobre_el_blanco_y_no_sobre_negro() {
        // D138. Pixel 0: transparente total. Pixel 1: rojo al 50 %.
        let (d3d, ctx) = dispositivo_de_prueba();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let rgba = [0, 0, 0, 0, 255, 0, 0, 128];
        let con_alfa = motor
            .bitmap_desde_pixeles_premultiplicado(2, 1, &rgba)
            .expect("deberia subir");
        let destino_tex = textura(&d3d, 2, 1);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();
        let rect = crate::lienzo::RectF {
            x: 0.0,
            y: 0.0,
            ancho: 2.0,
            alto: 1.0,
        };
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color::BLANCO);
                p.bitmap_con(&con_alfa, rect, None, crate::lienzo::Interpolacion::Vecino);
            })
            .unwrap();
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 0, 0), [255, 255, 255, 255]);
        let [b, g, r, _] = pixel(&d3d, &ctx, &destino_tex, 1, 0);
        assert!(r >= 253, "rojo {r}");
        assert!((125..=130).contains(&g) && (125..=130).contains(&b), "{g} {b}");

        // Caso negativo: con el alfa ignorado (lo de antes) sale negro.
        let sin_alfa = motor.bitmap_desde_pixeles(2, 1, &rgba).unwrap();
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color::BLANCO);
                p.bitmap_con(&sin_alfa, rect, None, crate::lienzo::Interpolacion::Vecino);
            })
            .unwrap();
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 0, 0), [0, 0, 0, 255]);
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn un_bitmap_reducido_se_pinta_estirado_a_su_tamano_logico() {
        // D139: la imagen que no cabia se sube a menos pixeles y se pinta en
        // el rectangulo de su tamano real. 2x2 subido, 4x4 pintado.
        let (d3d, ctx) = dispositivo_de_prueba();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        assert!(
            motor.lado_maximo_bitmap() >= 2048,
            "ninguna GPU con Direct2D baja de 2048"
        );
        let rojo = [255u8, 0, 0, 255].repeat(4);
        let b = motor
            .bitmap_desde_pixeles_premultiplicado(2, 2, &rojo)
            .unwrap();
        let destino_tex = textura(&d3d, 8, 8);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color::BLANCO);
                p.bitmap_con(
                    &b,
                    crate::lienzo::RectF {
                        x: 0.0,
                        y: 0.0,
                        ancho: 4.0,
                        alto: 4.0,
                    },
                    None,
                    crate::lienzo::Interpolacion::Vecino,
                );
            })
            .unwrap();
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 3, 3), [0, 0, 255, 255]);
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 4, 4), [255, 255, 255, 255]);
    }
```

`lienzo.rs`, `mod pruebas`:

```rust
    #[test]
    fn cada_interpolacion_va_a_su_modo_de_direct2d() {
        use windows::Win32::Graphics::Direct2D::{
            D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC, D2D1_INTERPOLATION_MODE_LINEAR,
            D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
        };
        assert_eq!(
            Interpolacion::Vecino.a_d2d(),
            D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR
        );
        assert_eq!(Interpolacion::Lineal.a_d2d(), D2D1_INTERPOLATION_MODE_LINEAR);
        assert_eq!(
            Interpolacion::Cubica.a_d2d(),
            D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC
        );
    }
```

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p pixpin-render -- --test-threads=1`
Expected: FAIL de compilación (`premultiplicar`, `Interpolacion`, `bitmap_con` no existen).

- [ ] **Step 3: Implementación**

`motor.rs`: añadir `D2D1_ALPHA_MODE` al `use` de `Direct2D::Common`. Sustituir `bitmap_desde_pixeles` por:

```rust
/// RGBA recto (lo que da un PNG) a premultiplicado (lo que espera Direct2D
/// para componer con alfa). Pura, para probarla sin GPU.
pub fn premultiplicar(rgba: &[u8]) -> Vec<u8> {
    let mut v = rgba.to_vec();
    for p in v.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a == 255 {
            continue;
        }
        for c in &mut p[..3] {
            *c = ((*c as u32 * a + 127) / 255) as u8;
        }
    }
    v
}
```

(función libre, fuera de `impl MotorRender`), y dentro de `impl MotorRender`:

```rust
    /// Sube pixeles RGBA de CPU como bitmap D2D. Para los pines: la imagen
    /// viene del almacen (PNG en disco), no de una textura de captura. El
    /// alfa se IGNORA: el pin pinta su tarjeta debajo y una captura no trae
    /// alfa util.
    pub fn bitmap_desde_pixeles(
        &self,
        ancho: u32,
        alto: u32,
        rgba: &[u8],
    ) -> Result<ID2D1Bitmap1, ErrorRender> {
        validar_tamano_rgba(ancho, alto, rgba.len())?;
        self.crear_bitmap_rgba(ancho, alto, rgba, D2D1_ALPHA_MODE_IGNORE)
    }

    /// Como `bitmap_desde_pixeles`, pero respetando el alfa (D138): el fondo
    /// del lienzo es blanco, y un PNG transparente subido con el alfa
    /// ignorado salia con fondo negro.
    pub fn bitmap_desde_pixeles_premultiplicado(
        &self,
        ancho: u32,
        alto: u32,
        rgba: &[u8],
    ) -> Result<ID2D1Bitmap1, ErrorRender> {
        validar_tamano_rgba(ancho, alto, rgba.len())?;
        let pre = premultiplicar(rgba);
        self.crear_bitmap_rgba(ancho, alto, &pre, D2D1_ALPHA_MODE_PREMULTIPLIED)
    }

    /// El lado mayor que admite un bitmap en este dispositivo (D139). La
    /// HD 4000 no sube texturas enormes: lo que pase de aqui se reduce antes.
    pub fn lado_maximo_bitmap(&self) -> u32 {
        // SAFETY: consulta sin precondiciones sobre el contexto vivo.
        unsafe { self.contexto.GetMaximumBitmapSize() }
    }

    fn crear_bitmap_rgba(
        &self,
        ancho: u32,
        alto: u32,
        datos: &[u8],
        alfa: D2D1_ALPHA_MODE,
    ) -> Result<ID2D1Bitmap1, ErrorRender> {
        let propiedades = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_R8G8B8A8_UNORM,
                alphaMode: alfa,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            colorContext: std::mem::ManuallyDrop::new(None),
        };
        // SAFETY: el puntero y el paso describen exactamente `datos` (el
        // llamante ya valido ancho*alto*4), que vive durante la llamada; D2D
        // copia los datos al crear el bitmap.
        let bitmap = unsafe {
            self.contexto.CreateBitmap(
                D2D_SIZE_U {
                    width: ancho,
                    height: alto,
                },
                Some(datos.as_ptr() as *const _),
                ancho * 4,
                &propiedades,
            )?
        };
        Ok(bitmap)
    }
```

`lienzo.rs`: añadir `D2D1_INTERPOLATION_MODE` y `D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC` al `use` de `Direct2D`. Tras `RectF`:

```rust
/// Como se muestrea un bitmap al estirarlo o encogerlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolacion {
    /// Pixeles tal cual: la lupa, o una captura al 100 % o muy ampliada.
    Vecino,
    /// Suave y barata: ampliaciones intermedias.
    Lineal,
    /// Cubica de calidad: reducir sin dientes.
    Cubica,
}

impl Interpolacion {
    fn a_d2d(self) -> D2D1_INTERPOLATION_MODE {
        match self {
            Interpolacion::Vecino => D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
            Interpolacion::Lineal => D2D1_INTERPOLATION_MODE_LINEAR,
            Interpolacion::Cubica => D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC,
        }
    }
}
```

`Pintor::bitmap` y el nuevo:

```rust
    /// Dibuja un bitmap. `nitido` usa vecino mas cercano (la lupa: pixeles
    /// reales); si no, interpolacion lineal (reescalados suaves).
    pub fn bitmap(&self, b: &ID2D1Bitmap1, destino: RectF, fuente: Option<RectF>, nitido: bool) {
        let modo = if nitido {
            Interpolacion::Vecino
        } else {
            Interpolacion::Lineal
        };
        self.bitmap_con(b, destino, fuente, modo);
    }

    /// Dibuja un bitmap con el modo de interpolacion que se diga (D141). La
    /// transformacion activa cuenta: dentro de la vista del mundo, `destino`
    /// va en coordenadas del mundo.
    pub fn bitmap_con(
        &self,
        b: &ID2D1Bitmap1,
        destino: RectF,
        fuente: Option<RectF>,
        modo: Interpolacion,
    ) {
        let fuente_d2d = fuente.map(|f| f.a_d2d());
        // SAFETY: dentro del fotograma; bitmap del mismo dispositivo D2D
        // (obligacion del llamante: todos los bitmaps salen de este motor).
        unsafe {
            self.motor.contexto().DrawBitmap(
                b,
                Some(&destino.a_d2d()),
                1.0,
                modo.a_d2d(),
                fuente_d2d.as_ref().map(|f| f as *const _),
                None,
            )
        };
    }
```

`lib.rs`: `pub use motor::{Color, ErrorRender, MotorRender, premultiplicar, validar_tamano_rgba};` y `pub use lienzo::{EstiloTexto, Interpolacion, Pintor, RectF, Tramo};`.

- [ ] **Step 4: Ejecutar**

Run: `cargo test -p pixpin-render -- --test-threads=1`
Expected: PASS (`premultiplicar_...`, `cada_interpolacion_...`); las dos de GPU quedan `ignored`. Opcional si hay GPU en la máquina de trabajo (no abre ventanas ni usa la app): `cargo test -p pixpin-render -- --ignored --test-threads=1`.

- [ ] **Step 5: fmt, clippy, suite y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --no-fail-fast -- --test-threads=1`

```bash
git add crates/pixpin-render/src/motor.rs crates/pixpin-render/src/lienzo.rs crates/pixpin-render/src/lib.rs
git commit -m "Render: bitmaps con alfa premultiplicado, tamano maximo e interpolacion elegible" -m "Claude-Session: https://claude.ai/code/session_01FJbWm4jCHQtHwwndRDpLZJ"
```

---

### Task 7: El fondo del lienzo, sin GPU (D135, D139, D141, D142, D143)

**Files:**
- Modify: `crates/pixpin-codec/src/imagen.rs` (tras `codificar_png` :106; pruebas desde :176)
- Modify: `crates/pixpin-codec/src/lib.rs:18` (exportar `redimensionar`)
- Create: `apps/pixpin/src/fondo_lienzo.rs`
- Modify: `apps/pixpin/src/main.rs` (`mod fondo_lienzo;` entre `mod editor;` y `mod gif;`)

**Interfaces:**
- Consumes: `Interpolacion`, `MotorRender::bitmap_desde_pixeles_premultiplicado`, `Pintor::bitmap_con` (Tarea 6); `escala_de` (Tarea 1); `Camara::encajar`.
- Produces:
  - `pub fn pixpin_codec::redimensionar(imagen: ImagenRgba, ancho: u32, alto: u32) -> Result<ImagenRgba, ErrorCodec>`
  - `pub const MARGEN_ENCUADRE: f32 = 48.0`
  - `pub fn encuadre_inicial(ancho: f32, alto: f32, area_ancho_px: f32, area_alto_px: f32, escala_por_cien: u32) -> Camara` (cámara lógica)
  - `pub fn modo_nitidez(zoom_efectivo: f32) -> Interpolacion`
  - `pub fn lado_de_subida(ancho: u32, alto: u32, maximo: u32) -> Option<(u32, u32)>`
  - `pub fn se_ve(vista: (f32, f32, f32, f32), ancho: f32, alto: f32) -> bool`
  - `pub fn recuadro_gris(ancho: u32, alto: u32) -> ImagenRgba`
  - `pub struct FondoLienzo` con `pub fn nuevo(imagen: ImagenRgba, lado_maximo: u32) -> Self`, `pub fn ancho(&self) -> f32`, `pub fn alto(&self) -> f32`, `pub fn tamano_subido(&self) -> (u32, u32)`, `pub fn asegurar(&mut self, motor: &MotorRender)`, `pub fn pintar(&self, p: &Pintor<'_>, vista: (f32, f32, f32, f32), zoom_efectivo: f32)`, `pub fn soltar(&mut self)`

- [ ] **Step 1: Pruebas (fallan)**

`imagen.rs`, `mod pruebas`:

```rust
    #[test]
    fn redimensionar_da_el_tamano_pedido_con_sus_bytes() {
        let img = ImagenRgba {
            ancho: 4,
            alto: 2,
            pixeles: [10u8, 20, 30, 255].repeat(8),
        };
        let r = redimensionar(img, 2, 1).unwrap();
        assert_eq!((r.ancho, r.alto), (2, 1));
        assert_eq!(r.pixeles.len(), r.bytes_esperados());
        assert_eq!(&r.pixeles[0..4], &[10, 20, 30, 255], "un color liso sigue liso");
    }

    #[test]
    fn redimensionar_a_cero_o_con_bytes_de_menos_falla() {
        let buena = || ImagenRgba {
            ancho: 2,
            alto: 2,
            pixeles: vec![0; 16],
        };
        assert!(redimensionar(buena(), 0, 5).is_err());
        let corta = ImagenRgba {
            ancho: 2,
            alto: 2,
            pixeles: vec![0; 15],
        };
        assert!(redimensionar(corta, 1, 1).is_err());
    }
```

`fondo_lienzo.rs`, pruebas:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::navegacion::vista_efectiva;
    use pixpin_motor2d::vector::Punto2;

    #[test]
    fn una_imagen_que_cabe_se_abre_a_zoom_uno_y_centrada() {
        for escala in [100, 150] {
            let c = encuadre_inicial(800.0, 600.0, 1920.0, 1080.0, escala);
            assert_eq!(c.zoom, 1.0, "escala {escala}");
            let centro = vista_efectiva(&c, escala).a_pantalla(Punto2::nuevo(400.0, 300.0));
            assert!(
                (centro.x - 960.0).abs() < 0.5 && (centro.y - 540.0).abs() < 0.5,
                "escala {escala}: {centro:?}"
            );
        }
    }

    #[test]
    fn una_imagen_que_no_cabe_se_abre_al_zoom_que_la_hace_caber_con_margen() {
        let c = encuadre_inicial(4000.0, 3000.0, 1920.0, 1080.0, 100);
        assert!(c.zoom < 1.0);
        assert!(4000.0 * c.zoom + 2.0 * MARGEN_ENCUADRE <= 1920.0 + 0.5);
        assert!(3000.0 * c.zoom + 2.0 * MARGEN_ENCUADRE <= 1080.0 + 0.5);
        let centro = vista_efectiva(&c, 100).a_pantalla(Punto2::nuevo(2000.0, 1500.0));
        assert!((centro.x - 960.0).abs() < 0.5 && (centro.y - 540.0).abs() < 0.5);
    }

    #[test]
    fn un_pixel_mas_que_el_hueco_con_margen_ya_no_va_a_zoom_uno() {
        // Caso negativo del borde: 1825 + 2 x 48 = 1921 > 1920.
        assert!(encuadre_inicial(1825.0, 600.0, 1920.0, 1080.0, 100).zoom < 1.0);
        assert_eq!(encuadre_inicial(1824.0, 600.0, 1920.0, 1080.0, 100).zoom, 1.0);
    }

    #[test]
    fn la_nitidez_depende_del_zoom_efectivo() {
        assert_eq!(modo_nitidez(1.0), Interpolacion::Vecino);
        assert_eq!(modo_nitidez(2.0), Interpolacion::Lineal);
        assert_eq!(modo_nitidez(2.999), Interpolacion::Lineal);
        assert_eq!(modo_nitidez(3.0), Interpolacion::Vecino);
        assert_eq!(modo_nitidez(0.5), Interpolacion::Cubica);
        // Caso negativo: casi 1 no es 1; a 1,0001 ya se interpola.
        assert_eq!(modo_nitidez(1.0001), Interpolacion::Lineal);
    }

    #[test]
    fn solo_se_reduce_lo_que_pasa_del_maximo_y_se_conserva_la_proporcion() {
        assert_eq!(lado_de_subida(1920, 1080, 4096), None);
        assert_eq!(lado_de_subida(4096, 10, 4096), None);
        assert_eq!(lado_de_subida(5000, 10, 4096), Some((4096, 8)));
        assert_eq!(lado_de_subida(10, 9000, 4096), Some((5, 4096)));
        // Un maximo desconocido (0) no reduce a nada.
        assert_eq!(lado_de_subida(5000, 10, 0), None);
    }

    #[test]
    fn el_fondo_reducido_sigue_midiendo_lo_que_la_imagen_original() {
        let img = ImagenRgba {
            ancho: 5000,
            alto: 10,
            pixeles: vec![255; 5000 * 10 * 4],
        };
        let f = FondoLienzo::nuevo(img, 4096);
        assert_eq!(f.tamano_subido(), (4096, 8));
        assert_eq!((f.ancho(), f.alto()), (5000.0, 10.0));
        let pequena = ImagenRgba {
            ancho: 20,
            alto: 10,
            pixeles: vec![0; 800],
        };
        assert_eq!(FondoLienzo::nuevo(pequena, 4096).tamano_subido(), (20, 10));
    }

    #[test]
    fn fuera_de_la_vista_el_fondo_no_se_pinta() {
        assert!(se_ve((-10.0, -10.0, 100.0, 100.0), 800.0, 600.0));
        assert!(se_ve((799.0, 599.0, 900.0, 700.0), 800.0, 600.0));
        assert!(!se_ve((801.0, 0.0, 1000.0, 100.0), 800.0, 600.0));
        assert!(!se_ve((-500.0, -500.0, -1.0, -1.0), 800.0, 600.0));
    }

    #[test]
    fn el_recuadro_de_reserva_es_gris_opaco_y_nunca_de_cero() {
        let r = recuadro_gris(3, 2);
        assert_eq!((r.ancho, r.alto), (3, 2));
        assert_eq!(&r.pixeles[0..4], &[200, 200, 200, 255]);
        let minimo = recuadro_gris(0, 0);
        assert_eq!((minimo.ancho, minimo.alto), (1, 1));
    }
}
```

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p pixpin-codec imagen -- --test-threads=1` y `cargo test -p pixpin fondo_lienzo -- --test-threads=1`
Expected: FAIL de compilación (`redimensionar`, `encuadre_inicial`, `FondoLienzo`... no existen).

- [ ] **Step 3: Implementación**

`imagen.rs`, tras `codificar_png`:

```rust
/// La imagen a otro tamano, con filtro triangular (suave y rapido). Para el
/// fondo del lienzo cuando la GPU no admite su tamano (D139). Consume la
/// imagen para no duplicar sus bytes mientras se reduce.
pub fn redimensionar(imagen: ImagenRgba, ancho: u32, alto: u32) -> Result<ImagenRgba, ErrorCodec> {
    if imagen.ancho == 0 || imagen.alto == 0 || ancho == 0 || alto == 0 {
        return Err(ErrorCodec::Vacia { ancho, alto });
    }
    let (a0, h0) = (imagen.ancho, imagen.alto);
    let espera = imagen.bytes_esperados();
    let tiene = imagen.pixeles.len();
    let incoherente = ErrorCodec::TamanoIncoherente {
        ancho: a0,
        alto: h0,
        tiene,
        espera,
    };
    // `from_raw` acepta un buffer MAS largo de lo necesario: se exige el
    // tamano justo aqui, como `validar_tamano_rgba` en el render.
    if tiene != espera {
        return Err(incoherente);
    }
    let origen = image::RgbaImage::from_raw(a0, h0, imagen.pixeles).ok_or(incoherente)?;
    let r = image::imageops::resize(&origen, ancho, alto, image::imageops::FilterType::Triangle);
    Ok(ImagenRgba {
        ancho,
        alto,
        pixeles: r.into_raw(),
    })
}
```

`lib.rs`: `pub use imagen::{ErrorCodec, FormatoImagen, ImagenRgba, cargar, codificar_png, guardar, redimensionar};`

`fondo_lienzo.rs`, encima de las pruebas:

```rust
//! La imagen del pin como fondo fijo del lienzo (D132-D143).
//!
//! No es un elemento de la escena (D133): no pasa por `Escena` ni por la
//! rejilla, asi que el gesto no puede seleccionarla, moverla ni borrarla.
//! Vive en el mundo en (0,0)-(ancho, alto), las coordenadas en las que ya
//! estan las anotaciones del pin.

use pixpin_codec::ImagenRgba;
use pixpin_motor2d::camara::Camara;
use pixpin_render::{Interpolacion, MotorRender, Pintor, RectF};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

/// Margen alrededor de la imagen al abrir, en pixeles logicos (D135).
pub const MARGEN_ENCUADRE: f32 = 48.0;

/// La camara (logica) con la que se abre el lienzo: la imagen centrada, a
/// zoom 1 si cabe con margen y, si no, al zoom que la hace caber (D135).
pub fn encuadre_inicial(
    ancho: f32,
    alto: f32,
    area_ancho_px: f32,
    area_alto_px: f32,
    escala_por_cien: u32,
) -> Camara {
    let escala = crate::navegacion::escala_de(escala_por_cien);
    let (w, h) = (area_ancho_px / escala, area_alto_px / escala);
    let cabe = ancho + 2.0 * MARGEN_ENCUADRE <= w && alto + 2.0 * MARGEN_ENCUADRE <= h;
    if cabe {
        return Camara {
            x: ancho / 2.0 - w / 2.0,
            y: alto / 2.0 - h / 2.0,
            zoom: 1.0,
        };
    }
    Camara::encajar((0.0, 0.0, ancho, alto), w, h, MARGEN_ENCUADRE)
}

/// Como se muestrea la imagen segun cuantos pixeles fisicos ocupa cada
/// pixel suyo (D141): al 100 % exacto o muy ampliada, pixeles tal cual (una
/// captura se lee nitida); reducida, cubica (sin dientes); entre medias,
/// lineal.
pub fn modo_nitidez(zoom_efectivo: f32) -> Interpolacion {
    if zoom_efectivo == 1.0 || zoom_efectivo >= 3.0 {
        Interpolacion::Vecino
    } else if zoom_efectivo < 1.0 {
        Interpolacion::Cubica
    } else {
        Interpolacion::Lineal
    }
}

/// El tamano al que hay que subir una imagen para que su lado mayor no pase
/// de `maximo`, o `None` si ya cabe (D139).
pub fn lado_de_subida(ancho: u32, alto: u32, maximo: u32) -> Option<(u32, u32)> {
    let mayor = ancho.max(alto);
    if maximo == 0 || mayor <= maximo {
        return None;
    }
    let f = maximo as f64 / mayor as f64;
    Some((
        ((ancho as f64 * f).round() as u32).clamp(1, maximo),
        ((alto as f64 * f).round() as u32).clamp(1, maximo),
    ))
}

/// Si la imagen (0,0)-(ancho, alto) toca la caja del mundo que se ve (D142).
pub fn se_ve(vista: (f32, f32, f32, f32), ancho: f32, alto: f32) -> bool {
    vista.0 <= ancho && vista.2 >= 0.0 && vista.1 <= alto && vista.3 >= 0.0
}

/// El recuadro gris que sustituye a una imagen que no se pudo leer. Nunca de
/// cero pixeles: un bitmap vacio no se puede subir.
pub fn recuadro_gris(ancho: u32, alto: u32) -> ImagenRgba {
    let (ancho, alto) = (ancho.max(1), alto.max(1));
    ImagenRgba {
        ancho,
        alto,
        pixeles: [200u8, 200, 200, 255].repeat(ancho as usize * alto as usize),
    }
}

/// La imagen de fondo mientras el lienzo esta abierto.
///
/// Guarda los pixeles (ya reducidos si hizo falta) ademas del bitmap: con el
/// dispositivo perdido hay que volver a subirlo, igual que la cache de tinta
/// se rehace. Todo se suelta al cerrar el lienzo (D143).
pub struct FondoLienzo {
    /// Tamano en el mundo: el de la imagen original.
    ancho: f32,
    alto: f32,
    imagen: ImagenRgba,
    bitmap: Option<ID2D1Bitmap1>,
    /// Subir fallo: no se reintenta en cada fotograma hasta `soltar`.
    fallo: bool,
}

impl FondoLienzo {
    pub fn nuevo(imagen: ImagenRgba, lado_maximo: u32) -> Self {
        let (ancho, alto) = (imagen.ancho as f32, imagen.alto as f32);
        let imagen = match lado_de_subida(imagen.ancho, imagen.alto, lado_maximo) {
            None => imagen,
            Some((w, h)) => {
                tracing::info!(
                    original_ancho = imagen.ancho,
                    original_alto = imagen.alto,
                    ancho = w,
                    alto = h,
                    "imagen del lienzo reducida para la GPU"
                );
                pixpin_codec::redimensionar(imagen, w, h).unwrap_or_else(|e| {
                    tracing::warn!(?e, "no se pudo reducir la imagen del lienzo; sale sin fondo");
                    ImagenRgba {
                        ancho: 0,
                        alto: 0,
                        pixeles: Vec::new(),
                    }
                })
            }
        };
        Self {
            ancho,
            alto,
            imagen,
            bitmap: None,
            fallo: false,
        }
    }

    pub fn ancho(&self) -> f32 {
        self.ancho
    }

    pub fn alto(&self) -> f32 {
        self.alto
    }

    /// Cuantos pixeles se suben de verdad.
    pub fn tamano_subido(&self) -> (u32, u32) {
        (self.imagen.ancho, self.imagen.alto)
    }

    /// Sube el bitmap si aun no esta (una vez por lienzo, o tras `soltar`).
    pub fn asegurar(&mut self, motor: &MotorRender) {
        if self.bitmap.is_some() || self.fallo || self.imagen.ancho == 0 || self.imagen.alto == 0
        {
            return;
        }
        match motor.bitmap_desde_pixeles_premultiplicado(
            self.imagen.ancho,
            self.imagen.alto,
            &self.imagen.pixeles,
        ) {
            Ok(b) => self.bitmap = Some(b),
            Err(e) => {
                self.fallo = true;
                tracing::warn!(?e, "no se pudo subir la imagen del lienzo");
            }
        }
    }

    /// Pinta la imagen con la vista del mundo ya puesta. `vista` es la caja
    /// del mundo visible y `zoom_efectivo` el de la camara efectiva.
    pub fn pintar(&self, p: &Pintor<'_>, vista: (f32, f32, f32, f32), zoom_efectivo: f32) {
        let Some(b) = &self.bitmap else {
            return;
        };
        if !se_ve(vista, self.ancho, self.alto) {
            return;
        }
        p.bitmap_con(
            b,
            RectF {
                x: 0.0,
                y: 0.0,
                ancho: self.ancho,
                alto: self.alto,
            },
            None,
            modo_nitidez(zoom_efectivo),
        );
    }

    /// Olvida el bitmap (dispositivo perdido): el siguiente `asegurar` lo
    /// vuelve a subir desde los pixeles guardados.
    pub fn soltar(&mut self) {
        self.bitmap = None;
        self.fallo = false;
    }
}
```

`main.rs`: hasta la Tarea 8 nadie usa el módulo fuera de sus pruebas y clippy (`-D warnings`) daría `dead_code`; se declara así y la Tarea 8 quita el atributo:

```rust
// Lo usa el editor desde la Tarea 8 del plan del lienzo.
#[cfg_attr(not(test), allow(dead_code))]
mod fondo_lienzo;
```

- [ ] **Step 4: Ejecutar**

Run: `cargo test -p pixpin-codec -p pixpin -- --test-threads=1`
Expected: PASS (2 en `imagen::pruebas`, 8 en `fondo_lienzo::pruebas`).

- [ ] **Step 5: fmt, clippy, suite y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --no-fail-fast -- --test-threads=1`

```bash
git add crates/pixpin-codec/src/imagen.rs crates/pixpin-codec/src/lib.rs apps/pixpin/src/fondo_lienzo.rs apps/pixpin/src/main.rs
git commit -m "Fondo del lienzo: encuadre inicial, nitidez por zoom y reduccion a la GPU, probados sin ventana" -m "Claude-Session: https://claude.ai/code/session_01FJbWm4jCHQtHwwndRDpLZJ"
```

---

### Task 8: El editor pinta el fondo (D132, D133, D137, D140, D142, D143)

**Files:**
- Modify: `apps/pixpin/src/ventana_editor.rs` (`use`; `abrir` firma y cuerpo; bloque de la capa congelada; llamadas a `pedir_medida` y `pintar`; `pintar` :762-897; `pedir_medida` :926-1001; prueba `abrir_y_cerrar_el_editor_no_revienta`)
- Modify: `apps/pixpin/src/main.rs` (llamada de la bandeja; quitar el `cfg_attr` de `mod fondo_lienzo;`)

**Interfaces:**
- Consumes: `FondoLienzo`, `encuadre_inicial` (Tarea 7); `MotorRender::lado_maximo_bitmap` (Tarea 6).
- Produces: `pub fn abrir(escena: Escena, ajustes_iman: enganche::Ajustes, nivel: pixpin_nivel::Nivel, medir_fotogramas: bool, fondo: Option<pixpin_codec::ImagenRgba>) -> Result<Escena>`; `pintar(.., fondo: &mut Option<FondoLienzo>, ..)`; `pedir_medida(.., fondo: &mut Option<FondoLienzo>, ..)`.

- [ ] **Step 1: Prueba (falla)**

En `mod pruebas`, la prueba de GPU pasa a:

```rust
    #[test]
    #[ignore = "necesita sesion de escritorio con GPU; ejecutar con --ignored"]
    fn abrir_y_cerrar_el_editor_no_revienta() {
        // (comentario existente)
        let _ = abrir(
            Escena::nueva(),
            pixpin_motor2d::enganche::Ajustes::default(),
            pixpin_nivel::Nivel::Completo,
            false,
            None,
        );
    }
```

y una pura nueva:

```rust
    #[test]
    fn el_fondo_no_es_un_elemento_que_se_pueda_seleccionar() {
        // D133: la imagen vive fuera de la escena. Con un fondo abierto y
        // nada dibujado, Ctrl+A no elige nada y el borrador no tiene que
        // borrar.
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::nuevo();
        let _fondo = crate::fondo_lienzo::FondoLienzo::nuevo(
            crate::fondo_lienzo::recuadro_gris(800, 600),
            4096,
        );
        gesto.evento(EventoGesto::SeleccionarTodo, &mut escena, 1.0);
        assert!(gesto.seleccion.ids().is_empty());
        assert_eq!(escena.cuantos_visibles(), 0);
    }
```

- [ ] **Step 2: Ejecutar y ver que falla**

Run: `cargo test -p pixpin ventana_editor -- --test-threads=1`
Expected: FAIL de compilación (`abrir` recibe 5 argumentos, se esperaban 4).

- [ ] **Step 3: Implementación**

1. `use crate::fondo_lienzo::{FondoLienzo, encuadre_inicial};`
2. `abrir` gana `fondo: Option<pixpin_codec::ImagenRgba>` (doc: «`fondo` es la imagen del pin, fija en el mundo en (0,0)–(ancho, alto) (D132); la bandeja pasa `None` (D137)»). Tras crear `motor`:

```rust
    // D139: se reduce aqui, que es donde se sabe cuanto admite la GPU.
    let mut fondo = fondo.map(|img| FondoLienzo::nuevo(img, motor.lado_maximo_bitmap()));
```

3. La cámara de la Tarea 1/5:

```rust
    // D135: con fondo, la imagen centrada; sin fondo, el origen como antes.
    let mut camara = match &fondo {
        Some(f) => encuadre_inicial(
            f.ancho(),
            f.alto(),
            area.ancho as f32,
            area.alto as f32,
            monitor.escala_por_cien,
        ),
        None => Camara::nueva(),
    };
```

4. Bloque de la capa congelada (`if activo_ahora && en_reposo_antes {`): antes de `let _ = capa.preparar(`, `if let Some(f) = fondo.as_mut() { f.asegurar(&motor); }`; dentro del cierre, justo tras `p.poner_vista(..)`:

```rust
                        // D140: la imagen se pinta primera y entra en la capa:
                        // mientras se dibuja, no cuesta nada por fotograma.
                        if let Some(f) = &fondo {
                            f.pintar(p, vista, efectiva.zoom);
                        }
```

5. `pedir_medida(..)` y `pintar(..)` en `abrir`: añadir `&mut fondo,` tras `&capa,`.
6. Antes de `ventana.ocultar();` al final de `abrir`:

```rust
    // D143: la copia en GPU (y la de CPU) se suelta al cerrar, no al volver
    // al gestor de pines.
    drop(fondo);
```

7. `pintar`: nuevo parámetro `fondo: &mut Option<FondoLienzo>,` tras `capa: &CapaEstatica,`. Antes de `let Ok(destino) = superficie.empezar(motor)`:

```rust
    if let Some(f) = fondo.as_mut() {
        f.asegurar(motor);
    }
```

Antes de `let error = motor.dibujar(`: `let fondo_ref = fondo.as_ref();`. Dentro del cierre, sustituir

```rust
        if !capa_vale {
            p.limpiar(Color::BLANCO);
        }
        p.poner_vista((0.0, 0.0), camara.zoom, (origen.x, origen.y));
```

por

```rust
        if !capa_vale {
            p.limpiar(Color::BLANCO);
        }
        p.poner_vista((0.0, 0.0), camara.zoom, (origen.x, origen.y));
        // D140/D142: con la capa valida la imagen ya esta copiada; si no, va
        // la primera, debajo de todo, y solo si se ve.
        if !capa_vale {
            if let Some(f) = fondo_ref {
                f.pintar(p, vista, camara.zoom);
            }
        }
```

En `if error.is_err() {`, tras `cache_tinta.vaciar();`:

```rust
        // Dispositivo perdido: el bitmap del fondo tambien era del viejo.
        if let Some(f) = fondo.as_mut() {
            f.soltar();
        }
```

8. `pedir_medida`: parámetro `fondo: &mut Option<FondoLienzo>,` tras `capa: &CapaEstatica,` y pasarlo a su `pintar(..)` en la misma posición.
9. `main.rs`: la llamada de la bandeja gana `None` como quinto argumento; `mod fondo_lienzo;` sin atributo.

- [ ] **Step 4: Ejecutar**

Run: `cargo test -p pixpin -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: fmt, clippy, suite y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --no-fail-fast -- --test-threads=1`

```bash
git add apps/pixpin/src/ventana_editor.rs apps/pixpin/src/main.rs
git commit -m "D132/D140: el editor pinta la imagen de fondo en la capa congelada y la suelta al cerrar" -m "Claude-Session: https://claude.ai/code/session_01FJbWm4jCHQtHwwndRDpLZJ"
```

---

### Task 9: «Abrir en lienzo» en el menú del pin (D131)

**Files:**
- Modify: `crates/pixpin-pin/src/menu.rs` (constantes :15-38; `TextosPin` :42-70; `entradas_del_menu` :169-185; pruebas `textos()` :343-375 y nueva prueba)
- Modify: `crates/pixpin-pin/src/ventana.rs` (`CambioPin` :65-~140, tras `AnotarPedido`; `match cmd` :2723-2745)
- Modify: `crates/pixpin-store/i18n/es-ES/main.ftl` (tras `pin-copiar-texto` :139) y `crates/pixpin-store/i18n/en-US/main.ftl` (tras :138)
- Modify: `apps/pixpin/src/main.rs:1251` (`textos_del_pin`)
- Modify: `apps/pixpin/src/pines.rs:885-886` (brazo provisional)

**Interfaces:**
- Produces: `pub const CMD_ABRIR_LIENZO: u32 = 16`; `TextosPin::abrir_en_lienzo: String`; `CambioPin::AbrirLienzoPedido`; clave `pin-abrir-en-lienzo`.

- [ ] **Step 1: Prueba (falla)**

`menu.rs`, `mod pruebas`:

```rust
    #[test]
    fn abrir_en_lienzo_solo_esta_en_los_pines_de_imagen() {
        let estado = EstadoMenu {
            con_ocr: true,
            ..Default::default()
        };
        assert!(ids(&entradas_del_menu(&imagen(), estado, &textos())).contains(&CMD_ABRIR_LIENZO));
        // Casos negativos: una nota, una ficha, un video o un documento no
        // tienen pixeles propios que poner de fondo.
        let nota = Contenido::Nota { texto: "x".into() };
        for c in [
            nota,
            archivo(),
            video(),
            Contenido::Documento {
                nombre: "a.pdf".into(),
                vista: ImagenRgba {
                    ancho: 1,
                    alto: 1,
                    pixeles: vec![0; 4],
                },
            },
        ] {
            assert!(
                !ids(&entradas_del_menu(&c, estado, &textos())).contains(&CMD_ABRIR_LIENZO),
                "{c:?}"
            );
        }
    }
```

y en `textos()` añadir `abrir_en_lienzo: "Abrir en lienzo".into(),`.

- [ ] **Step 2: Ejecutar y ver que falla**

Run: `cargo test -p pixpin-pin menu -- --test-threads=1`
Expected: FAIL de compilación (`CMD_ABRIR_LIENZO` no existe; `TextosPin` no tiene `abrir_en_lienzo`).

- [ ] **Step 3: Implementación**

`menu.rs`:

```rust
/// Abrir el pin en el lienzo infinito del editor, con la imagen de fondo
/// (D131). Solo en pines de imagen.
pub const CMD_ABRIR_LIENZO: u32 = 16;
```

En `TextosPin`, tras `copiar_texto`:

```rust
    /// El de abrir la imagen en el lienzo del editor (D131).
    pub abrir_en_lienzo: String,
```

En `entradas_del_menu`, tras el bloque de `CMD_TEXTO` (:180-185):

```rust
    // Abrir en el lienzo va con las acciones de la imagen: dibujar sin el
    // borde del pin. El doble clic sigue anotando dentro del pin.
    if matches!(contenido, Contenido::Imagen(_)) {
        v.push(EntradaMenu::Accion {
            id: CMD_ABRIR_LIENZO,
            etiqueta: t.abrir_en_lienzo.clone(),
        });
    }
```

`ventana.rs`, en `CambioPin` tras `AnotarPedido,`:

```rust
    /// Menu de un pin de imagen: abrirlo en el lienzo del editor (D131). Lo
    /// resuelve el gestor, que conoce el almacen y el fichero de dibujo.
    AbrirLienzoPedido,
```

y en el `match cmd` (:2724), tras `CMD_TEXTO`:

```rust
                            crate::menu::CMD_ABRIR_LIENZO => Some(CambioPin::AbrirLienzoPedido),
```

`es-ES/main.ftl`, tras `pin-copiar-texto`:

```
# Abrir un pin de imagen en el lienzo del editor.
pin-abrir-en-lienzo = Abrir en lienzo
```

`en-US/main.ftl`, tras `pin-copiar-texto`:

```
# Opening an image pin in the editor canvas.
pin-abrir-en-lienzo = Open in canvas
```

`main.rs`, `textos_del_pin`, tras `copiar_texto`: `abrir_en_lienzo: textos.t("pin-abrir-en-lienzo"),`

`pines.rs`, `atender`: el pedido existe ya pero se atiende en la Tarea 10; hasta entonces, antes de `_ => Ok(()),`:

```rust
            CambioPin::AbrirLienzoPedido => {
                tracing::info!(id, "abrir en lienzo: aun sin atender");
                Ok(())
            }
```

- [ ] **Step 4: Ejecutar**

Run: `cargo test -p pixpin-pin -p pixpin-store -p pixpin -- --test-threads=1`
Expected: PASS, incluida `los_dos_catalogos_tienen_exactamente_las_mismas_claves`.

- [ ] **Step 5: fmt, clippy, suite y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --no-fail-fast -- --test-threads=1`

```bash
git add crates/pixpin-pin/src/menu.rs crates/pixpin-pin/src/ventana.rs crates/pixpin-store/i18n apps/pixpin/src/main.rs apps/pixpin/src/pines.rs
git commit -m "D131: Abrir en lienzo en el menu de los pines de imagen" -m "Claude-Session: https://claude.ai/code/session_01FJbWm4jCHQtHwwndRDpLZJ"
```

---

### Task 10: Del pin al lienzo y vuelta, y entrega de la Parte 1 (D134, D144, D146, D147, errores)

**Files:**
- Modify: `apps/pixpin/src/pines.rs` (`struct Pines` :161-208 y `nuevos` :252-283; `atender` :832-888; nuevas funciones tras `salir_de_anotar` :1023-1040; nuevas libres antes de `#[cfg(test)]` :1910; pruebas)
- Modify: `apps/pixpin/src/main.rs:1137-1156` (tras `p.purgar();`)
- Modify: `crates/pixpin-pin/src/ventana.rs:1814` (recorte de las anotaciones)

**Interfaces:**
- Consumes: `CambioPin::AbrirLienzoPedido` (Tarea 9); `ventana_editor::abrir(.., Some(fondo))` (Tarea 8); `fondo_lienzo::recuadro_gris` (Tarea 7); `pixpin_motor2d::{cargar, guardar, ErrorFormato, ordenes_de_escena}`; `Almacen::{entradas, ruta_objeto}`; `pixpin_codec::cargar`; `Pin::poner_anotaciones`; `Pintor::con_recorte`.
- Produces:
  - `pub struct PedidoLienzo { pub id: u64, pub ruta: PathBuf, pub habia_fichero: bool, pub escena: Escena, pub fondo: ImagenRgba }`
  - `impl Pines { pub fn tomar_lienzo(&mut self) -> Option<PedidoLienzo>; pub fn terminar_lienzo(&mut self, id: u64, ruta: &Path, habia_fichero: bool, resultado: anyhow::Result<Escena>); fn pedir_lienzo(&mut self, id: u64) -> Result<()> }`
  - `fn escena_para_lienzo(ruta: &Path) -> Result<(Escena, bool), pixpin_motor2d::ErrorFormato>`
  - `fn hay_que_guardar_lienzo(escena: &Escena, habia_fichero: bool) -> bool`
  - `fn guardar_lienzo(ruta: &Path, escena: &Escena, habia_fichero: bool) -> Result<bool, pixpin_motor2d::ErrorFormato>`
  - `fn tamano_de_reserva(guardado: Option<PinGuardado>) -> (u32, u32)` con `const RESERVA_LIENZO: (u32, u32) = (800, 600)`

- [ ] **Step 1: Pruebas puras (fallan)**

`pines.rs`, `mod pruebas` (añadir `use pixpin_motor2d::gesto::{EventoGesto, Gesto};`):

```rust
    fn dir_de_prueba(etiqueta: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-lienzo-{etiqueta}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn una_escena_vacia_sin_fichero_previo_no_crea_fichero() {
        let ruta = dir_de_prueba("vacia-sin-previo").join("objeto.pixpin2d");
        let (escena, habia) = escena_para_lienzo(&ruta).unwrap();
        assert!(!habia);
        assert!(!guardar_lienzo(&ruta, &escena, habia).unwrap());
        assert!(!ruta.exists(), "abrir y cerrar sin dibujar no deja ficheros");
    }

    #[test]
    fn con_fichero_previo_se_guarda_aunque_quede_vacia() {
        // Borrar todo en el lienzo tiene que borrarlo tambien del pin.
        let ruta = dir_de_prueba("vacia-con-previo").join("objeto.pixpin2d");
        let mut antes = Escena::nueva();
        antes.anadir(trazo(1.0, 2.0, 30.0, 40.0));
        pixpin_motor2d::guardar(&ruta, &antes).unwrap();
        let (_, habia) = escena_para_lienzo(&ruta).unwrap();
        assert!(habia);
        assert!(guardar_lienzo(&ruta, &Escena::nueva(), habia).unwrap());
        assert_eq!(pixpin_motor2d::cargar(&ruta).unwrap().cuantos_visibles(), 0);
    }

    #[test]
    fn un_fichero_corrupto_no_abre_el_lienzo_ni_se_sobrescribe() {
        let ruta = dir_de_prueba("corrupto").join("objeto.pixpin2d");
        std::fs::write(&ruta, "{ esto no es un dibujo").unwrap();
        assert!(escena_para_lienzo(&ruta).is_err());
        assert_eq!(
            std::fs::read_to_string(&ruta).unwrap(),
            "{ esto no es un dibujo"
        );
    }

    #[test]
    fn si_guardar_falla_el_fichero_anterior_queda_intacto() {
        let d = dir_de_prueba("guardar-falla");
        let ruta = d.join("objeto.pixpin2d");
        let mut antes = Escena::nueva();
        antes.anadir(trazo(1.0, 2.0, 30.0, 40.0));
        pixpin_motor2d::guardar(&ruta, &antes).unwrap();
        let original = std::fs::read_to_string(&ruta).unwrap();
        // El temporal de `guardar` no se puede escribir: hay un directorio
        // con su nombre.
        std::fs::create_dir_all(ruta.with_extension("pixpin2d.tmp")).unwrap();
        let mut nueva = Escena::nueva();
        nueva.anadir(trazo(5.0, 5.0, 9.0, 9.0));
        assert!(guardar_lienzo(&ruta, &nueva, true).is_err());
        assert_eq!(std::fs::read_to_string(&ruta).unwrap(), original);
    }

    #[test]
    fn lo_dibujado_en_el_lienzo_vuelve_en_las_mismas_coordenadas() {
        // Ida y vuelta (spec §6): la escena del pin, un trazo nuevo con el
        // gesto (fuera de la imagen incluso, D147), guardar y recargar.
        let ruta = dir_de_prueba("ida-y-vuelta").join("objeto.pixpin2d");
        let mut previa = Escena::nueva();
        previa.anadir(trazo(10.0, 10.0, 50.0, 50.0));
        pixpin_motor2d::guardar(&ruta, &previa).unwrap();

        let (mut escena, habia) = escena_para_lienzo(&ruta).unwrap();
        let mut gesto = Gesto::nuevo();
        gesto.evento(
            EventoGesto::Pulsar {
                p: Punto2::nuevo(-50.0, 10.0),
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        gesto.evento(
            EventoGesto::Mover {
                p: Punto2::nuevo(-20.0, 40.0),
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        gesto.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(-20.0, 40.0),
            },
            &mut escena,
            1.0,
        );
        escena.compactar();
        assert_eq!(escena.cuantos_visibles(), 2);
        assert!(guardar_lienzo(&ruta, &escena, habia).unwrap());

        let vuelta = pixpin_motor2d::cargar(&ruta).unwrap();
        let antes: Vec<Elemento> = escena.visibles().cloned().collect();
        let despues: Vec<Elemento> = vuelta.visibles().cloned().collect();
        assert_eq!(antes, despues);
    }

    #[test]
    fn sin_imagen_legible_el_recuadro_mide_lo_del_pin_o_800_por_600() {
        let g = Pines::guardado_desde(
            Rect {
                x: 0,
                y: 0,
                ancho: 320,
                alto: 200,
            },
            100,
            100,
        );
        assert_eq!(tamano_de_reserva(Some(g)), (320, 200));
        assert_eq!(tamano_de_reserva(None), (800, 600));
        let cero = PinGuardado {
            ancho: 0,
            ..g
        };
        assert_eq!(tamano_de_reserva(Some(cero)), (800, 600));
    }
```

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p pixpin pines -- --test-threads=1`
Expected: FAIL de compilación (`escena_para_lienzo`, `guardar_lienzo`, `tamano_de_reserva` no existen).

- [ ] **Step 3: Implementación en `pines.rs`**

Libres, antes de `#[cfg(test)]`:

```rust
/// El tamano del recuadro gris cuando la imagen del pin no se puede leer:
/// el del pin guardado si se conoce, si no 800 x 600 (tabla de errores).
const RESERVA_LIENZO: (u32, u32) = (800, 600);

fn tamano_de_reserva(guardado: Option<PinGuardado>) -> (u32, u32) {
    match guardado {
        Some(g) if g.ancho > 0 && g.alto > 0 => (g.ancho, g.alto),
        _ => RESERVA_LIENZO,
    }
}

/// La escena con la que se abre el lienzo y si ya habia fichero. Un fichero
/// corrupto es un error: abrir el lienzo con una escena vacia y guardarla al
/// cerrar pisaria lo que el usuario tenia (tabla de errores).
fn escena_para_lienzo(ruta: &Path) -> Result<(Escena, bool), pixpin_motor2d::ErrorFormato> {
    let habia = ruta.is_file();
    let escena = pixpin_motor2d::cargar(ruta)?;
    Ok((escena, habia))
}

/// D146: una escena vacia sin fichero previo no crea uno; con fichero previo
/// se guarda aunque quede vacia, o lo borrado volveria a salir en el pin.
fn hay_que_guardar_lienzo(escena: &Escena, habia_fichero: bool) -> bool {
    habia_fichero || escena.cuantos_visibles() > 0
}

/// Guarda lo del lienzo si toca. Devuelve si escribio. `guardar` escribe a
/// un temporal y renombra: si falla, el fichero anterior queda intacto.
fn guardar_lienzo(
    ruta: &Path,
    escena: &Escena,
    habia_fichero: bool,
) -> Result<bool, pixpin_motor2d::ErrorFormato> {
    if !hay_que_guardar_lienzo(escena, habia_fichero) {
        return Ok(false);
    }
    pixpin_motor2d::guardar(ruta, escena)?;
    Ok(true)
}
```

Tipo público, junto a `struct Anotacion`:

```rust
/// Lo que el gestor deja preparado para abrir un pin en el lienzo. El
/// gestor no abre el editor: el editor tiene su propio bucle y su propio
/// dispositivo, y lo abre `main.rs`, que se lo devuelve con `terminar_lienzo`.
pub struct PedidoLienzo {
    pub id: u64,
    /// El `.pixpin2d` del pin.
    pub ruta: PathBuf,
    pub habia_fichero: bool,
    pub escena: Escena,
    /// La imagen del pin, o el recuadro gris si no se pudo leer.
    pub fondo: ImagenRgba,
}
```

Campo en `Pines` (tras `anotacion`): `lienzo_pedido: Option<PedidoLienzo>,` y en `nuevos`: `lienzo_pedido: None,`.

`atender`: sustituir el brazo provisional de la Tarea 9 por `CambioPin::AbrirLienzoPedido => self.pedir_lienzo(id),`.

Métodos, tras `salir_de_anotar`:

```rust
    /// Prepara el lienzo de un pin (D132). Queda en `lienzo_pedido` hasta
    /// que el bucle principal lo recoja con `tomar_lienzo`.
    fn pedir_lienzo(&mut self, id: u64) -> Result<()> {
        if self.lienzo_pedido.is_some() {
            return Ok(());
        }
        // D134: si se esta anotando ESTE pin, primero se guarda y se sale;
        // si no, el lienzo abriria el fichero de antes de la anotacion.
        if self.anotacion.as_ref().is_some_and(|a| a.id == id) {
            self.salir_de_anotar()?;
        }
        let ruta = self
            .ruta_anotacion(id)
            .context("este pin no tiene contenido que abrir en el lienzo")?;
        let (escena, habia_fichero) = match escena_para_lienzo(&ruta) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(?e, id, ruta = %ruta.display(), "dibujo del pin corrupto; el lienzo no se abre para no pisarlo");
                return Ok(());
            }
        };
        let (ruta_imagen, guardado) = {
            let a = self.almacen.borrow();
            let e = a
                .entradas()
                .iter()
                .find(|e| e.id == id)
                .context("el pin no esta en el almacen")?;
            (a.ruta_objeto(e), e.pin)
        };
        let fondo = match cargar(&ruta_imagen) {
            Ok(imagen) => imagen,
            Err(e) => {
                let (ancho, alto) = tamano_de_reserva(guardado);
                tracing::warn!(?e, id, ancho, alto, "imagen del pin ilegible; el lienzo abre sobre un recuadro gris");
                crate::fondo_lienzo::recuadro_gris(ancho, alto)
            }
        };
        self.lienzo_pedido = Some(PedidoLienzo {
            id,
            ruta,
            habia_fichero,
            escena,
            fondo,
        });
        Ok(())
    }

    /// Lo que `purgar` dejo preparado para abrir en el lienzo, si hay algo.
    pub fn tomar_lienzo(&mut self) -> Option<PedidoLienzo> {
        self.lienzo_pedido.take()
    }

    /// Lo que devuelve el editor al cerrar (D146): guardar si toca y
    /// repintar el pin con lo guardado. Un fallo al guardar deja el fichero
    /// y el pin como estaban.
    pub fn terminar_lienzo(
        &mut self,
        id: u64,
        ruta: &Path,
        habia_fichero: bool,
        resultado: Result<Escena>,
    ) {
        let escena = match resultado {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(?e, id, "no se pudo abrir el lienzo");
                return;
            }
        };
        match guardar_lienzo(ruta, &escena, habia_fichero) {
            Err(e) => {
                tracing::error!(?e, id, "no se pudo guardar el lienzo; el pin sigue con lo de antes")
            }
            Ok(guardado) => {
                tracing::info!(id, guardado, elementos = escena.cuantos_visibles(), "lienzo cerrado");
                // Directamente con la escena y no con `recargar_anotaciones`:
                // esa no toca el pin si el fichero quedo vacio, y lo borrado
                // en el lienzo seguiria viendose en el pin.
                if let Some(pin) = self.vivos.get(&id) {
                    pin.poner_anotaciones(pixpin_motor2d::ordenes_de_escena(&escena));
                }
            }
        }
    }
```

- [ ] **Step 4: `main.rs` — abrir el editor con el pedido**

En :1139-1140:

```rust
        if let Some(p) = &mut pines {
            p.purgar();
            // D132/D144: el lienzo de un pin se abre aqui, fuera del gestor,
            // porque el editor tiene su propio bucle. Mientras dura, lo que
            // pidan otros pines espera en su cola; al cerrar se atiende con
            // otro `purgar` (que puede traer otro lienzo, de ahi el `while`).
            while let Some(pedido) = p.tomar_lienzo() {
                let crate::pines::PedidoLienzo {
                    id,
                    ruta,
                    habia_fichero,
                    escena,
                    fondo,
                } = pedido;
                let resultado = ventana_editor::abrir(
                    escena,
                    config.enganche,
                    decision.nivel,
                    config.rendimiento.medir_fotogramas,
                    Some(fondo),
                );
                p.terminar_lienzo(id, &ruta, habia_fichero, resultado);
                p.purgar();
            }
```

(el bloque de `tomar_paginas_extraidas` sigue igual detrás).

- [ ] **Step 5: `ventana.rs` — el pin no muestra lo de fuera (D147)**

En `pintar` del pin, :1814 (dentro del mismo cierre que `let caja = RectF { x: m, y: m, ancho: w, alto: h };` de :1579):

```rust
        // D147: lo dibujado en el lienzo fuera de la imagen se guarda, pero
        // el pin ensena solo su contenido: sin recorte se colaba por el
        // margen de la sombra.
        p.con_recorte(caja, |p| pintar_anotaciones(p, i, m));
```

- [ ] **Step 6: Ejecutar**

Run: `cargo test -p pixpin -p pixpin-pin -- --test-threads=1`
Expected: PASS (6 pruebas nuevas en `pines::pruebas`).

- [ ] **Step 7: fmt, clippy, suite, binario y commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --no-fail-fast -- --test-threads=1 && cargo build --release -p pixpin`
Expected: verde; `target/release/pixpinmax.exe` nuevo. **No** ejecutarlo.

```bash
git add apps/pixpin/src/pines.rs apps/pixpin/src/main.rs crates/pixpin-pin/src/ventana.rs
git commit -m "D146: el pin abre su imagen en el lienzo, guarda al cerrar y se repinta" -m "Claude-Session: https://claude.ai/code/session_01FJbWm4jCHQtHwwndRDpLZJ"
```

- [ ] **Step 8: Mensaje al usuario (literal)**

```
Parte 1 lista en target\release\pixpinmax.exe (rama lienzo-desde-pin).

1. Clic derecho en un pin de imagen -> "Abrir en lienzo": la imagen sale
   centrada y lo que ya habias anotado aparece encima.
2. Rueda (vertical), Shift+rueda (horizontal), Ctrl+rueda (zoom al cursor),
   espacio+arrastrar y boton central+arrastrar. Acerca mucho: los pixeles de
   la captura se ven nitidos.
3. Dibuja dentro y fuera de la imagen y cierra con Esc.
4. El pin muestra lo de dentro; vuelve a abrir el lienzo: sale tambien lo de
   fuera.
5. Prueba con una captura o PNG con transparencia: no debe salir fondo negro.
Si algo falla, pasame el ultimo fichero de registros\.
```

---

# Parte 0b — se planifica con el registro del usuario

No hay tareas hasta tener el registro de la Tarea 3 (líneas «fotogramas del editor»). Con esos números se escribe un plan aparte según la rama que indiquen (spec §4):

| Si la medición muestra | Arreglo | Prueba |
|---|---|---|
| S1: presentar domina (> 8 ms de media) | Presentar sin bloquear (`Present1(0, DXGI_PRESENT_DO_NOT_WAIT)` o el objeto de espera de latencia de la swapchain) y pintar como mucho una vez por refresco con temporizador | Puerta: pintar + presentar < 4 ms de media |
| S2: muchos puntos por fotograma pero pocos fotogramas por segundo | Pintar al terminar de vaciar la cola si pasó ≥ 1 refresco desde el último fotograma, sin esperar a `WM_PAINT` | Pura: el bucle decide pintar con la cola no vacía tras un refresco |
| S3: pintar crece con el largo del trazo | Recalcular solo la cola: el contorno de los puntos estables se guarda y se recalculan los últimos N (N = 32) | Oráculo: el contorno por cola coincide con el entero (tolerancia 0,01); puerta: 5.000 puntos < 0,5 ms por fotograma |

Si la medición no apunta a ninguna, se para y se vuelve a hablar con el usuario antes de tocar código.

## Cobertura de la spec

| Decisión / prueba | Tarea |
|---|---|
| D127 (y prueba ida y vuelta 150 %, grosor ×1,5) | 1 |
| D128 | 1 (no se tocan `capa.rs` ni el pin) |
| D129 (y prueba apagado/60 fotogramas) | 2 |
| D130 | 3 |
| D131 (y prueba imagen sí / nota no) | 9 |
| D132, D140, D143 | 7, 8 |
| D133 | 7 (módulo fuera de la escena), 8 (prueba) |
| D134, D144, D146 (y pruebas vacía/previa), D147 | 10 |
| D135 (y prueba encuadre) | 7, 8 |
| D136 (y prueba rueda/Shift/Ctrl/espacio/central), D137 | 4, 5, 8 |
| D138 (GPU), D139 (GPU y pura), D141 (y prueba 1/2/3/0,5) | 6, 7 |
| D142 | 7, 8 |
| D145 | 4 |
| Errores: PNG ilegible, corrupto, fallo al guardar, dispositivo perdido | 10, 10, 10, 8 |
| Ida y vuelta | 10 |
