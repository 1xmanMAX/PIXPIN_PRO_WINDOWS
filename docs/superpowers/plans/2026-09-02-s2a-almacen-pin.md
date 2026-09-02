# PixPin Max · S2-A Almacén + pin de imagen — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que un atajo nuevo abra el overlay y, al confirmar, el recorte quede **flotando 1:1 en su sitio** como pin sin bordes; que el pin se mueva agarrándolo desde cualquier punto, se redimensione proporcional desde las esquinas, se cierre con `Esc` sin perder nada, y que al reiniciar la app los pines reaparezcan donde estaban — porque todo vive primero en el almacén.

**Architecture:** El almacén (ficheros reales + `indice.json`, D27) vive en `pixpin-store` como módulo puro de `std::fs`. La interacción del pin es una máquina de estados pura en `pixpin-pin/src/estado.rs`; la ventana del pin (`PixPinPin`, WndProc propio) es **autocontenida**: guarda su estado tras `GWLP_USERDATA` y notifica al ejecutable por un callback (`CambioPin`) — no puede tocar el almacén porque `pixpin-pin` y `pixpin-store` son ambos L2. El dibujo (tarjeta redondeada + sombra) pasa por el pintor seguro de `pixpin-render`, que gana `bitmap_desde_pixeles` para subir una imagen de CPU sin depender de ningún crate de la misma capa.

**Tech Stack:** todo lo ya construido: `pixpin-render` (D2D/DComp), `pixpin-capture` (overlay + Recursos), `pixpin-codec` (`ImagenRgba`, PNG), `serde_json` para el índice.

**Spec:** [`../specs/2026-09-01-s2-pines-almacen-design.md`](../specs/2026-09-01-s2-pines-almacen-design.md) — decisiones D20-D30; esta subfase es el «S2-A» de su §10.
**Rendimiento:** [`../specs/2026-08-31-rendimiento-equipos-modestos-design.md`](../specs/2026-08-31-rendimiento-equipos-modestos-design.md) — puertas de §7 de la spec de S2.
**Lecciones:** [`../retrospectivas/2026-08-09-s1a-lecciones.md`](../retrospectivas/2026-08-09-s1a-lecciones.md) — y las de S1-B2 recogidas en `medidas/2026-09-02-equipo-desarrollo-s1b2.md`.

## Global Constraints

- **Rust estable, edición 2024. Baseline `x86-64` (D17), vigilado por `baseline.rs`.**
- **`#![forbid(unsafe_code)]`** en `pixpin-geom`, `pixpin-model`, `pixpin-nivel`, `pixpin-store`, `pixpin-ui`, `pixpin-flow`, `pixpin-plugin` y `apps/pixpin`. **`unsafe` con `// SAFETY:`** en el resto; una obligación del llamante se escribe como obligación.
- **Regla de capas:** L0 `geom, model, nivel` · L1 `shell, render, gpu, codec` · L2 `capture, pin, pdf, ocr, record, store` · L3 `ui, flow, plugin` · L4 `pixpin`. **`pixpin-pin` NO puede depender de `pixpin-store` ni de `pixpin-capture` (misma capa)**: el almacén lo toca el ejecutable, y el dispositivo D3D llega como `&ID3D11Device` (tipo externo).
- **D21, el principio rector:** el pin es una vista de una entrada del almacén. Cerrar un pin NUNCA borra su contenido.
- **D23:** mover agarrando desde cualquier punto; esquinas (12 px lógicos) redimensionan **siempre proporcional** ancladas a la opuesta; `Esc` cierra el enfocado; cero cromo. Doble clic: 100 % ↔ ajustado.
- **D30:** esquinas redondeadas (8 px lógicos × escala) y sombra difusa dibujada (negra suave; el color por grupo llega en S2-B). La ventana lleva margen transparente para la sombra.
- **Escritura segura del índice:** temporal + `rename`. En S2-A el «retardo de 300 ms» de la spec se implementa como **escritura al soltar el gesto** (nunca por fotograma): misma garantía, sin temporizador. Queda anotado como equivalencia consciente.
- Identificadores ASCII en español; texto de usuario por Fluent; tests con `-- --test-threads=1`; los de escritorio/GPU `#[ignore]`. Cero red. Cada tarea termina en commit. **Todo test de invariante trae su caso negativo; «verificado» = «lo ejecuté».**
- **La puerta de cada tarea cuenta errores, no mira la última línea** (lección de S1-B2): `E=$(cargo clippy ... 2>&1 | grep -cE "^error")` y fallar si no es 0.

### Estado del que partes

`main` con S1 completo. Piezas que esta fase consume tal cual:

| De | Qué |
|---|---|
| `pixpin_geom` | `Punto`, `Rect`, `Monitor`, `DisposicionMonitores`, `Seleccion`/`Tirador` |
| `pixpin_render` | `MotorRender` (`dibujar`, `bitmap_desde_textura`), `Pintor` (`rellenar_redondeado`, `bitmap`, …), `Superficie`, `Color`, `RectF` |
| `pixpin_codec` | `ImagenRgba`, `guardar`, `FormatoImagen` |
| `apps/pixpin/src/overlay.rs` | `Recursos`, `ejecutar_overlay`, `ModoConfirmacion`, `AccionFinal`, la barra |
| `pixpin_shell` | `atajos::{registrar, ID_*}`, patrón de ventana con WndProc propio (en `overlay.rs` del crate) |
| `pixpin_store` | `Ubicacion` (`raiz()`), `Ajustes`/`Atajos`, catálogos Fluent |

**Entorno:** `cargo` no está en el PATH (`export PATH="$USERPROFILE/.cargo/bin:$PATH"`). Monitor de desarrollo 3000×2000 al 150 %.

---

## Estructura de ficheros

```
crates/pixpin-geom/src/pin_geometria.rs   redimension proporcional anclada + recolocacion (Task 1)
crates/pixpin-store/src/almacen.rs        indice.json + objetos/, temporal+rename (Task 2)
crates/pixpin-codec/src/imagen.rs         gana cargar(ruta) -> ImagenRgba (Task 3)
crates/pixpin-render/src/motor.rs         gana bitmap_desde_pixeles(ancho, alto, &[u8]) (Task 3)
crates/pixpin-pin/src/estado.rs           maquina de interaccion del pin, PURA (Task 4)
crates/pixpin-pin/src/ventana.rs          PixPinPin autocontenida: WndProc + dibujo + callback (Task 5)
crates/pixpin-pin/src/lib.rs              reexporta
apps/pixpin/src/pines.rs                  gestor: crear desde imagen, restaurar, persistir cambios (Task 7)
apps/pixpin/src/overlay.rs                AccionFinal::Pinear y ModoConfirmacion::Pinear (Task 6)
apps/pixpin/src/main.rs                   atajo Ctrl+Alt+F, cableado, restauracion al arrancar (Tasks 6-7)
crates/pixpin-store/src/ajustes.rs        Atajos gana `pin` (Task 6)
crates/pixpin-shell/src/atajos.rs         ID_PIN (Task 6)
```

**Fuera de S2-A** (van en S2-B, spec §10): notas, fichas de archivo, portapapeles con atajo, menú contextual, grupos, imán de bordes, `Ctrl+C` sobre el pin. El pin v1 de esta fase: imagen, mover, proporcional, doble clic, `Esc`, persistencia y restauración.

---

## Task 1: Geometría del pin, en puro

**Files:**
- Create: `crates/pixpin-geom/src/pin_geometria.rs`
- Modify: `crates/pixpin-geom/src/lib.rs`

**Interfaces:**
- Consumes: `Punto`, `Rect`.
- Produces:
  - `pub enum Esquina { Noroeste, Noreste, Sureste, Suroeste }`
  - `pub fn esquina_en(rect_local: Rect, p: Punto, zona: u32) -> Option<Esquina>` — `p` en coordenadas LOCALES de la ventana; `zona` en px físicos (12 lógicos × escala, lo aplica el llamante).
  - `pub fn redimension_proporcional(original: Rect, esquina: Esquina, cursor: Punto, minimo: u32) -> Rect` — ancla la esquina OPUESTA, conserva la proporción de `original` (D23), nunca baja de `minimo` en ninguna dimensión. `original` y `cursor` en coordenadas del escritorio virtual.
  - `pub fn recolocar_en_area(rect: Rect, area_trabajo: Rect) -> Rect` — desliza el rect dentro del área SIN cambiar su tamaño; si es más grande que el área, lo alinea a la esquina superior izquierda. Para restaurar pines cuyo monitor desapareció (spec §5.2).

- [ ] **Step 1: Escribir los tests que fallan**

Crear `crates/pixpin-geom/src/pin_geometria.rs` con sólo las pruebas:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::punto::Punto;
    use crate::rect::Rect;

    #[test]
    fn las_cuatro_esquinas_se_detectan_y_el_centro_no() {
        let r = Rect { x: 0, y: 0, ancho: 400, alto: 300 };
        assert_eq!(esquina_en(r, Punto { x: 5, y: 5 }, 12), Some(Esquina::Noroeste));
        assert_eq!(esquina_en(r, Punto { x: 395, y: 5 }, 12), Some(Esquina::Noreste));
        assert_eq!(esquina_en(r, Punto { x: 395, y: 295 }, 12), Some(Esquina::Sureste));
        assert_eq!(esquina_en(r, Punto { x: 5, y: 295 }, 12), Some(Esquina::Suroeste));
        // Caso negativo: el centro y los BORDES no son esquinas — en el pin,
        // el borde mueve como cualquier otro punto (D23: cero cromo).
        assert_eq!(esquina_en(r, Punto { x: 200, y: 150 }, 12), None);
        assert_eq!(esquina_en(r, Punto { x: 200, y: 5 }, 12), None);
        assert_eq!(esquina_en(r, Punto { x: 13, y: 13 }, 12), None);
    }

    #[test]
    fn redimensionar_conserva_la_proporcion_y_ancla_la_opuesta() {
        // 400x300 = 4:3. Arrastrar la sureste a un cursor "ancho" debe dar
        // un rect 4:3 con la noroeste clavada en (100, 100).
        let r = Rect { x: 100, y: 100, ancho: 400, alto: 300 };
        let nuevo = redimension_proporcional(r, Esquina::Sureste, Punto { x: 900, y: 400 }, 48);
        assert_eq!((nuevo.x, nuevo.y), (100, 100), "la esquina opuesta no se mueve");
        let prop_original = 400.0 / 300.0;
        let prop_nueva = nuevo.ancho as f64 / nuevo.alto as f64;
        assert!(
            (prop_nueva - prop_original).abs() < 0.02,
            "proporcion {prop_nueva} != {prop_original}"
        );
        assert!(nuevo.ancho > 400, "arrastrar hacia fuera agranda");
    }

    #[test]
    fn redimensionar_por_la_noroeste_ancla_la_sureste() {
        let r = Rect { x: 100, y: 100, ancho: 400, alto: 300 };
        let nuevo = redimension_proporcional(r, Esquina::Noroeste, Punto { x: 300, y: 250 }, 48);
        assert_eq!(
            (nuevo.derecha(), nuevo.abajo()),
            (500, 400),
            "la sureste queda clavada"
        );
        assert!(nuevo.ancho < 400, "arrastrar hacia dentro encoge");
    }

    #[test]
    fn el_minimo_impide_desaparecer() {
        // Caso negativo: cruzar el ancla no puede dar 0 ni voltear.
        let r = Rect { x: 100, y: 100, ancho: 400, alto: 300 };
        let nuevo = redimension_proporcional(r, Esquina::Sureste, Punto { x: 90, y: 90 }, 48);
        assert!(nuevo.ancho >= 48 && nuevo.alto >= 48, "quedo {nuevo:?}");
        assert_eq!((nuevo.x, nuevo.y), (100, 100));
    }

    #[test]
    fn recolocar_desliza_sin_cambiar_tamano() {
        let area = Rect { x: 0, y: 0, ancho: 1920, alto: 1040 };
        let fuera = Rect { x: 1800, y: -50, ancho: 300, alto: 200 };
        let dentro = recolocar_en_area(fuera, area);
        assert_eq!((dentro.ancho, dentro.alto), (300, 200), "el tamano es sagrado");
        assert_eq!(area.interseccion(dentro), Some(dentro), "queda entero dentro");
    }

    #[test]
    fn recolocar_un_gigante_lo_alinea_arriba_izquierda() {
        // Caso negativo del clamp: un pin mas grande que el area no puede
        // hacer entrar en panico a un clamp con min > max.
        let area = Rect { x: 0, y: 0, ancho: 800, alto: 600 };
        let gigante = Rect { x: 500, y: 500, ancho: 2000, alto: 1500 };
        let puesto = recolocar_en_area(gigante, area);
        assert_eq!((puesto.x, puesto.y), (0, 0));
        assert_eq!((puesto.ancho, puesto.alto), (2000, 1500));
    }
}
```

- [ ] **Step 2: Ejecutar y comprobar que falla**

Run: `cargo test -p pixpin-geom pin_geometria -- --test-threads=1`
Expected: FAIL — no existe `Esquina`.

- [ ] **Step 3: Implementar**

Añadir encima de las pruebas:

```rust
//! Geometria del pin flotante: esquinas proporcionales y recolocacion.
//!
//! D23: el pin se agarra desde cualquier punto y SOLO las esquinas
//! redimensionan, siempre en proporcion, ancladas a la opuesta. La
//! recolocacion restaura pines cuyo monitor desaparecio sin cambiarles el
//! tamano ni dejarlos fuera de pantalla.

use crate::punto::Punto;
use crate::rect::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Esquina {
    Noroeste,
    Noreste,
    Sureste,
    Suroeste,
}

impl Esquina {
    /// El punto que NO se mueve al redimensionar por esta esquina.
    fn ancla(self, r: Rect) -> Punto {
        match self {
            Esquina::Noroeste => Punto { x: r.derecha(), y: r.abajo() },
            Esquina::Noreste => Punto { x: r.izquierda(), y: r.abajo() },
            Esquina::Sureste => Punto { x: r.izquierda(), y: r.arriba() },
            Esquina::Suroeste => Punto { x: r.derecha(), y: r.arriba() },
        }
    }
}

/// Que esquina hay bajo el punto local, con zona cuadrada de `zona` px.
pub fn esquina_en(rect_local: Rect, p: Punto, zona: u32) -> Option<Esquina> {
    let z = zona as i32;
    let cerca_izq = p.x < rect_local.izquierda() + z;
    let cerca_der = p.x >= rect_local.derecha() - z;
    let cerca_arr = p.y < rect_local.arriba() + z;
    let cerca_aba = p.y >= rect_local.abajo() - z;
    match (cerca_izq, cerca_der, cerca_arr, cerca_aba) {
        (true, false, true, false) => Some(Esquina::Noroeste),
        (false, true, true, false) => Some(Esquina::Noreste),
        (false, true, false, true) => Some(Esquina::Sureste),
        (true, false, false, true) => Some(Esquina::Suroeste),
        _ => None,
    }
}

/// Redimension proporcional: el rect nuevo tiene la proporcion de
/// `original`, la esquina opuesta clavada, y su tamano lo dicta la
/// distancia del cursor al ancla (domina el eje mayor relativo).
pub fn redimension_proporcional(
    original: Rect,
    esquina: Esquina,
    cursor: Punto,
    minimo: u32,
) -> Rect {
    if original.esta_vacio() {
        return original;
    }
    let ancla = esquina.ancla(original);
    let dx = (cursor.x - ancla.x).abs().max(1) as f64;
    let dy = (cursor.y - ancla.y).abs().max(1) as f64;
    let proporcion = original.ancho as f64 / original.alto as f64;

    // Domina el eje que pide mas tamano relativo: asi el rect siempre
    // "alcanza" al cursor por un lado y lo recorta por el otro.
    let (ancho, alto) = if dx / proporcion >= dy {
        (dx, dx / proporcion)
    } else {
        (dy * proporcion, dy)
    };
    let minimo = minimo.max(1) as f64;
    let (ancho, alto) = if ancho < minimo || alto < minimo {
        if proporcion >= 1.0 {
            (minimo * proporcion, minimo)
        } else {
            (minimo, minimo / proporcion)
        }
    } else {
        (ancho, alto)
    };
    let (ancho, alto) = (ancho.round() as u32, alto.round() as u32);

    // Reconstruir desde el ancla hacia el lado de la esquina activa.
    let (x, y) = match esquina {
        Esquina::Sureste => (ancla.x, ancla.y),
        Esquina::Noroeste => (ancla.x - ancho as i32, ancla.y - alto as i32),
        Esquina::Noreste => (ancla.x, ancla.y - alto as i32),
        Esquina::Suroeste => (ancla.x - ancho as i32, ancla.y),
    };
    Rect { x, y, ancho, alto }
}

/// Desliza el rect al interior del area sin cambiar su tamano. Un rect mas
/// grande que el area queda alineado a la esquina superior izquierda.
pub fn recolocar_en_area(rect: Rect, area_trabajo: Rect) -> Rect {
    let max_x = (area_trabajo.derecha() - rect.ancho as i32).max(area_trabajo.izquierda());
    let max_y = (area_trabajo.abajo() - rect.alto as i32).max(area_trabajo.arriba());
    Rect {
        x: rect.x.clamp(area_trabajo.izquierda(), max_x),
        y: rect.y.clamp(area_trabajo.arriba(), max_y),
        ancho: rect.ancho,
        alto: rect.alto,
    }
}
```

Añadir a `crates/pixpin-geom/src/lib.rs`, en orden alfabético con los módulos existentes:

```rust
pub mod pin_geometria;

pub use pin_geometria::{Esquina, esquina_en, recolocar_en_area, redimension_proporcional};
```

- [ ] **Step 4: Ejecutar y comprobar que pasa**

Run: `cargo test -p pixpin-geom -- --test-threads=1`
Expected: PASS — los 6 nuevos más los 33 existentes.

- [ ] **Step 5: Puerta y commit**

Run: `cargo fmt --all --check && cargo test --workspace -- --test-threads=1` y `E=$(cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -cE "^error"); [ "$E" = "0" ]`

```bash
git add crates/pixpin-geom
git commit -m "Geometria del pin: esquinas proporcionales y recolocacion

Solo las esquinas redimensionan (D23: los bordes mueven como cualquier
otro punto), siempre en la proporcion original y ancladas a la opuesta;
el minimo impide desaparecer o voltear al cruzar el ancla. recolocar_en_area
desliza sin cambiar el tamano y el caso del pin mas grande que el area
esta cubierto: un clamp ingenuo con min > max entra en panico."
```

---
## Task 2: El almacén

**Files:**
- Create: `crates/pixpin-store/src/almacen.rs`
- Modify: `crates/pixpin-store/src/lib.rs`, `crates/pixpin-store/Cargo.toml`

**Interfaces:**
- Consumes: nada interno nuevo (`serde`, `serde_json`, `std::fs`).
- Produces:
  - `pub struct PinGuardado { pub x: i32, pub y: i32, pub ancho: u32, pub alto: u32, pub escala_por_cien: u32 }`
  - `pub enum TipoEntrada { Imagen }` (serde en minúsculas; nota y archivo llegan en S2-B)
  - `pub struct Entrada { pub id: u64, pub tipo: TipoEntrada, pub creado: String, pub origen: String, pub objeto: String, pub grupo: Option<u32>, pub pin: Option<PinGuardado> }`
  - `pub struct Almacen { … }` con:
    - `pub fn abrir(raiz: &Path) -> Result<Almacen, ErrorAlmacen>` — crea `almacen/objetos/` e `indice.json` si no existen; tolera claves desconocidas.
    - `pub fn guardar_imagen(&mut self, png: &[u8], origen: &str, pin: Option<PinGuardado>) -> Result<u64, ErrorAlmacen>` — escribe `objetos/AAAA/MM/NNNNNN.png` (contador propio, nunca reescribe), añade la entrada, persiste el índice. Devuelve el id.
    - `pub fn actualizar_pin(&mut self, id: u64, pin: Option<PinGuardado>) -> Result<(), ErrorAlmacen>` — persiste. `None` = cerrado (la entrada QUEDA: D21/D25).
    - `pub fn entradas(&self) -> &[Entrada]` y `pub fn ruta_objeto(&self, e: &Entrada) -> PathBuf`.
  - `pub enum ErrorAlmacen { Io(std::io::Error, PathBuf), Indice(serde_json::Error), NoExiste(u64) }` (con `thiserror`).
- **Persistencia atómica:** el índice SIEMPRE se escribe a `indice.json.tmp` + `rename`. Los objetos nunca se reescriben.

- [ ] **Step 1: Dependencia y tests que fallan**

```bash
cargo add serde_json -p pixpin-store
```

Crear `crates/pixpin-store/src/almacen.rs` con sólo las pruebas:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    fn raiz(etiqueta: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("pixpin-almacen-{etiqueta}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Un PNG minimo valido no hace falta: el almacen guarda BYTES y no
    /// valida el formato (eso es del codec). Cuatro bytes bastan.
    const BYTES: &[u8] = &[0x89, b'P', b'N', b'G'];

    fn pin() -> PinGuardado {
        PinGuardado { x: 100, y: 200, ancho: 300, alto: 150, escala_por_cien: 150 }
    }

    #[test]
    fn guardar_crea_objeto_e_indice_y_sobrevive_a_reabrir() {
        let r = raiz("basico");
        let id = {
            let mut a = Almacen::abrir(&r).unwrap();
            a.guardar_imagen(BYTES, "recorte", Some(pin())).unwrap()
        };
        // Reabrir desde disco: nada vive solo en memoria (D25).
        let a = Almacen::abrir(&r).unwrap();
        let e = a.entradas().iter().find(|e| e.id == id).expect("la entrada persiste");
        assert_eq!(e.tipo, TipoEntrada::Imagen);
        assert_eq!(e.origen, "recorte");
        assert_eq!(e.pin, Some(pin()));
        // El objeto es un fichero real navegable (D27).
        assert_eq!(fs::read(a.ruta_objeto(e)).unwrap(), BYTES);
    }

    #[test]
    fn cerrar_un_pin_no_borra_nada() {
        // D21 como test: actualizar a None conserva entrada y objeto.
        let r = raiz("cerrar");
        let mut a = Almacen::abrir(&r).unwrap();
        let id = a.guardar_imagen(BYTES, "recorte", Some(pin())).unwrap();
        a.actualizar_pin(id, None).unwrap();

        let a2 = Almacen::abrir(&r).unwrap();
        let e = a2.entradas().iter().find(|e| e.id == id).unwrap();
        assert_eq!(e.pin, None, "cerrado");
        assert!(a2.ruta_objeto(e).is_file(), "el contenido sigue ahi");
    }

    #[test]
    fn los_ids_y_los_objetos_nunca_se_reutilizan() {
        // Caso negativo del contador: dos guardados dan ids y rutas
        // distintos aunque el primero se "cierre" entre medias.
        let r = raiz("contador");
        let mut a = Almacen::abrir(&r).unwrap();
        let id1 = a.guardar_imagen(BYTES, "recorte", None).unwrap();
        a.actualizar_pin(id1, None).unwrap();
        let id2 = a.guardar_imagen(BYTES, "recorte", None).unwrap();
        assert_ne!(id1, id2);
        let e1 = a.entradas().iter().find(|e| e.id == id1).unwrap().objeto.clone();
        let e2 = a.entradas().iter().find(|e| e.id == id2).unwrap().objeto.clone();
        assert_ne!(e1, e2, "cada objeto tiene su fichero");
    }

    #[test]
    fn actualizar_un_id_inexistente_da_error() {
        let r = raiz("no-existe");
        let mut a = Almacen::abrir(&r).unwrap();
        assert!(matches!(a.actualizar_pin(999, None), Err(ErrorAlmacen::NoExiste(999))));
    }

    #[test]
    fn un_indice_con_claves_desconocidas_abre_igual() {
        // La regla de compatibilidad de los ajustes, aplicada al indice: un
        // fichero escrito por una version futura no impide arrancar.
        let r = raiz("futuro");
        {
            let mut a = Almacen::abrir(&r).unwrap();
            a.guardar_imagen(BYTES, "recorte", None).unwrap();
        }
        let ruta = r.join("almacen").join("indice.json");
        let texto = fs::read_to_string(&ruta).unwrap().replacen(
            "\"version\":",
            "\"funcion_del_futuro\": 42, \"version\":",
            1,
        );
        fs::write(&ruta, texto).unwrap();
        assert!(Almacen::abrir(&r).is_ok());
    }

    #[test]
    fn el_indice_se_escribe_por_temporal_mas_rename() {
        // No queda ningun .tmp tras operar: si quedara, la escritura no fue
        // atomica o el rename fallo en silencio.
        let r = raiz("atomico");
        let mut a = Almacen::abrir(&r).unwrap();
        a.guardar_imagen(BYTES, "recorte", Some(pin())).unwrap();
        let sobras: Vec<_> = fs::read_dir(r.join("almacen"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(sobras.is_empty(), "quedo un temporal: {sobras:?}");
    }
}
```

- [ ] **Step 2: Ejecutar y comprobar que falla**

Run: `cargo test -p pixpin-store almacen -- --test-threads=1`
Expected: FAIL — no existe `Almacen`.

- [ ] **Step 3: Implementar**

Añadir encima de las pruebas:

```rust
//! El almacen: la verdad de todo lo pineado (D21, D25, D27).
//!
//! Ficheros reales navegables con el Explorador mas un indice JSON que es
//! lo UNICO que se reescribe — siempre a temporal + rename, la disciplina
//! que el ExcalidrawStore del Android aprendio a golpes. Los objetos se
//! crean con contador y no se tocan jamas; solo "Eliminar del almacen"
//! (S2-B) los borrara.
//!
//! Este modulo es std::fs puro: se prueba entero con directorios
//! temporales, sin Windows.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ErrorAlmacen {
    #[error("no se pudo acceder a {1}: {0}")]
    Io(#[source] std::io::Error, PathBuf),
    #[error("el indice del almacen tiene un error: {0}")]
    Indice(#[from] serde_json::Error),
    #[error("no existe ninguna entrada con id {0}")]
    NoExiste(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinGuardado {
    pub x: i32,
    pub y: i32,
    pub ancho: u32,
    pub alto: u32,
    /// DPI del monitor donde vivia, para restaurar con sentido (spec 5.2).
    pub escala_por_cien: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoEntrada {
    Imagen,
    // Nota y Archivo llegan en S2-B; el serde tolerante ya los aguantara.
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entrada {
    pub id: u64,
    pub tipo: TipoEntrada,
    /// ISO-8601 UTC, texto: el indice se lee con un editor.
    pub creado: String,
    pub origen: String,
    /// Ruta relativa a `almacen/`.
    pub objeto: String,
    pub grupo: Option<u32>,
    pub pin: Option<PinGuardado>,
}

/// El fichero indice.json entero. `#[serde(default)]` en todo: un indice de
/// una version futura abre igual (misma regla que los ajustes).
#[derive(Debug, Default, Serialize, Deserialize)]
struct Indice {
    #[serde(default = "version_uno")]
    version: u32,
    #[serde(default)]
    siguiente_id: u64,
    #[serde(default)]
    entradas: Vec<Entrada>,
}

fn version_uno() -> u32 {
    1
}

pub struct Almacen {
    dir: PathBuf,
    indice: Indice,
}

impl Almacen {
    pub fn abrir(raiz: &Path) -> Result<Almacen, ErrorAlmacen> {
        let dir = raiz.join("almacen");
        fs::create_dir_all(dir.join("objetos")).map_err(|e| ErrorAlmacen::Io(e, dir.clone()))?;
        let ruta = dir.join("indice.json");
        let indice = match fs::read_to_string(&ruta) {
            Ok(texto) => serde_json::from_str(&texto)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Indice {
                version: 1,
                siguiente_id: 1,
                entradas: Vec::new(),
            },
            Err(e) => return Err(ErrorAlmacen::Io(e, ruta)),
        };
        Ok(Almacen { dir, indice })
    }

    pub fn entradas(&self) -> &[Entrada] {
        &self.indice.entradas
    }

    pub fn ruta_objeto(&self, e: &Entrada) -> PathBuf {
        self.dir.join(&e.objeto)
    }

    pub fn guardar_imagen(
        &mut self,
        png: &[u8],
        origen: &str,
        pin: Option<PinGuardado>,
    ) -> Result<u64, ErrorAlmacen> {
        let id = self.indice.siguiente_id.max(1);
        self.indice.siguiente_id = id + 1;

        let (anio, mes) = anio_mes_utc();
        let relativa = format!("objetos/{anio:04}/{mes:02}/{id:06}.png");
        let ruta = self.dir.join(&relativa);
        if let Some(padre) = ruta.parent() {
            fs::create_dir_all(padre).map_err(|e| ErrorAlmacen::Io(e, padre.to_path_buf()))?;
        }
        // El objeto se escribe UNA vez y no se toca mas.
        fs::write(&ruta, png).map_err(|e| ErrorAlmacen::Io(e, ruta.clone()))?;

        self.indice.entradas.push(Entrada {
            id,
            tipo: TipoEntrada::Imagen,
            creado: ahora_iso(),
            origen: origen.to_string(),
            objeto: relativa,
            grupo: None,
            pin,
        });
        self.persistir()?;
        Ok(id)
    }

    pub fn actualizar_pin(&mut self, id: u64, pin: Option<PinGuardado>) -> Result<(), ErrorAlmacen> {
        let entrada = self
            .indice
            .entradas
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or(ErrorAlmacen::NoExiste(id))?;
        entrada.pin = pin;
        self.persistir()
    }

    /// Temporal + rename: un proceso que muera a mitad deja el indice
    /// anterior intacto, nunca uno a medias que no abre.
    fn persistir(&self) -> Result<(), ErrorAlmacen> {
        let definitivo = self.dir.join("indice.json");
        let temporal = self.dir.join("indice.json.tmp");
        let texto = serde_json::to_string_pretty(&self.indice)?;
        fs::write(&temporal, texto).map_err(|e| ErrorAlmacen::Io(e, temporal.clone()))?;
        fs::rename(&temporal, &definitivo).map_err(|e| ErrorAlmacen::Io(e, definitivo))?;
        Ok(())
    }
}

/// (año, mes) actuales en UTC, sin dependencias: dias desde epoch con el
/// algoritmo civil de Howard Hinnant, suficiente y determinista.
fn anio_mes_utc() -> (i64, u32) {
    let segundos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let dias = segundos.div_euclid(86_400);
    let (anio, mes, _dia) = civil_desde_dias(dias);
    (anio, mes)
}

fn ahora_iso() -> String {
    let segundos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let dias = segundos.div_euclid(86_400);
    let resto = segundos.rem_euclid(86_400);
    let (a, m, d) = civil_desde_dias(dias);
    format!(
        "{a:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        resto / 3600,
        (resto % 3600) / 60,
        resto % 60
    )
}

/// Conversion dias-desde-epoch -> fecha civil (Hinnant, dominio ±millones
/// de años; aqui solo se usa con fechas reales).
fn civil_desde_dias(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
```

Añadir a `crates/pixpin-store/src/lib.rs`:

```rust
pub mod almacen;

pub use almacen::{Almacen, Entrada, ErrorAlmacen, PinGuardado, TipoEntrada};
```

- [ ] **Step 4: Ejecutar y comprobar que pasa**

Run: `cargo test -p pixpin-store -- --test-threads=1`
Expected: PASS — los 6 del almacén más los existentes.

- [ ] **Step 5: Puerta y commit**

Run: la puerta estándar (fmt + clippy contando errores + tests workspace).

```bash
git add crates/pixpin-store Cargo.lock
git commit -m "El almacen: ficheros reales, indice atomico, cerrar no borra

D21 convertido en test: actualizar el pin a None conserva entrada y
objeto. Los objetos se escriben una vez con contador y no se tocan; el
indice va SIEMPRE por temporal + rename (hay un test que exige que no
queden .tmp), y las claves desconocidas se ignoran: un indice de una
version futura abre igual, la misma regla que los ajustes. La fecha sale
de una conversion civil propia (Hinnant): cero dependencias nuevas por
un ano y un mes."
```

---

## Task 3: Subir píxeles a la GPU y cargar PNG del disco

Las dos primitivas que el pin necesita y que hoy no existen: `pixpin-codec` sabe guardar pero no cargar, y `pixpin-render` sabe envolver texturas pero no crear un bitmap desde memoria.

**Files:**
- Modify: `crates/pixpin-codec/src/imagen.rs`, `crates/pixpin-codec/src/lib.rs`
- Modify: `crates/pixpin-render/src/motor.rs`, `crates/pixpin-render/src/lib.rs`

**Interfaces:**
- Produces:
  - `pixpin_codec::cargar(ruta: &Path) -> Result<ImagenRgba, ErrorCodec>` (variante nueva `ErrorCodec::Lectura { ruta, fuente: image::ImageError }`)
  - `MotorRender::bitmap_desde_pixeles(&self, ancho: u32, alto: u32, rgba: &[u8]) -> Result<ID2D1Bitmap1, ErrorRender>` — convierte RGBA→BGRA premultiplicado... no: D2D acepta `DXGI_FORMAT_R8G8B8A8_UNORM` con alfa ignorado, así que se sube TAL CUAL sin conversión. Variante nueva `ErrorRender::TamanoIncoherente`.

- [ ] **Step 1: Tests que fallan**

Añadir al módulo de pruebas de `crates/pixpin-codec/src/imagen.rs`:

```rust
    #[test]
    fn cargar_devuelve_lo_guardado() {
        let dir = temporal("cargar");
        let ruta = dir.join("ida-vuelta.png");
        let original = imagen_de_prueba();
        guardar(&original, &ruta, FormatoImagen::Png).unwrap();
        let leida = cargar(&ruta).unwrap();
        assert_eq!(leida, original, "PNG es sin perdida: la vuelta es identica");
    }

    #[test]
    fn cargar_una_ruta_inexistente_da_error_con_la_ruta() {
        let e = cargar(std::path::Path::new("Z:/no/existe.png")).unwrap_err();
        assert!(e.to_string().contains("existe.png"), "el error debe decir cual: {e}");
    }
```

Y a `crates/pixpin-render/src/motor.rs` (módulo de pruebas):

```rust
    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn un_bitmap_desde_pixeles_dibuja_esos_pixeles() {
        let (d3d, ctx) = dispositivo_de_prueba();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        // 2x1: rojo, verde (RGBA).
        let bitmap = motor
            .bitmap_desde_pixeles(2, 1, &[255, 0, 0, 255, 0, 255, 0, 255])
            .expect("deberia subir");
        let destino_tex = textura(&d3d, 2, 1);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();
        motor
            .dibujar(&destino, |p| {
                p.bitmap(
                    &bitmap,
                    crate::lienzo::RectF { x: 0.0, y: 0.0, ancho: 2.0, alto: 1.0 },
                    None,
                    true,
                );
            })
            .unwrap();
        // El destino es BGRA: rojo = [0,0,255,255].
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 0, 0), [0, 0, 255, 255]);
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 1, 0), [0, 255, 0, 255]);
    }

    #[test]
    fn un_buffer_incoherente_no_llega_a_la_gpu() {
        // Caso negativo y ademas corre SIN GPU: la validacion es previa.
        let r = std::panic::catch_unwind(|| {
            // Sin dispositivo no se puede crear motor; la validacion del
            // tamano vive en una funcion pura que se prueba directa.
            validar_tamano_rgba(2, 2, 7)
        });
        assert!(matches!(r, Ok(Err(_))), "7 bytes no son 2x2x4");
        assert!(validar_tamano_rgba(2, 2, 16).is_ok());
    }
```

- [ ] **Step 2: Comprobar que falla** — `cargo test -p pixpin-codec cargar -- --test-threads=1` y `cargo build -p pixpin-render`: FAIL/no compila.

- [ ] **Step 3: Implementar**

En `pixpin-codec/src/imagen.rs`:

```rust
/// Lee una imagen del disco a RGBA. La pareja de `guardar`.
pub fn cargar(ruta: &Path) -> Result<ImagenRgba, ErrorCodec> {
    let dinamica = image::open(ruta).map_err(|fuente| ErrorCodec::Lectura {
        ruta: ruta.to_path_buf(),
        fuente,
    })?;
    let rgba = dinamica.to_rgba8();
    Ok(ImagenRgba {
        ancho: rgba.width(),
        alto: rgba.height(),
        pixeles: rgba.into_raw(),
    })
}
```

con la variante en `ErrorCodec`:

```rust
    #[error("no se pudo leer {ruta}: {fuente}")]
    Lectura {
        ruta: std::path::PathBuf,
        #[source]
        fuente: image::ImageError,
    },
```

y el reexport `cargar` en `lib.rs`. En `pixpin-render/src/motor.rs`:

```rust
/// Validacion pura del buffer, separada para probarse sin GPU.
pub fn validar_tamano_rgba(ancho: u32, alto: u32, bytes: usize) -> Result<(), ErrorRender> {
    let espera = ancho as usize * alto as usize * 4;
    if ancho == 0 || alto == 0 || bytes != espera {
        return Err(ErrorRender::TamanoIncoherente { ancho, alto, tiene: bytes, espera });
    }
    Ok(())
}

impl MotorRender {
    /// Sube pixeles RGBA de CPU como bitmap D2D. Para los pines: la imagen
    /// viene del almacen (PNG en disco), no de una textura de captura.
    pub fn bitmap_desde_pixeles(
        &self,
        ancho: u32,
        alto: u32,
        rgba: &[u8],
    ) -> Result<ID2D1Bitmap1, ErrorRender> {
        validar_tamano_rgba(ancho, alto, rgba.len())?;
        let propiedades = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_R8G8B8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            colorContext: std::mem::ManuallyDrop::new(None),
        };
        // SAFETY: el puntero y el paso describen exactamente `rgba`, que
        // vive durante la llamada; D2D copia los datos al crear el bitmap.
        let bitmap = unsafe {
            self.contexto.CreateBitmap(
                windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_U {
                    width: ancho,
                    height: alto,
                },
                Some(rgba.as_ptr() as *const _),
                ancho * 4,
                &propiedades,
            )?
        };
        Ok(bitmap)
    }
}
```

con la variante y los imports (`DXGI_FORMAT_R8G8B8A8_UNORM`, `D2D_SIZE_U`):

```rust
    #[error("el buffer tiene {tiene} bytes pero {ancho}x{alto} necesita {espera}")]
    TamanoIncoherente { ancho: u32, alto: u32, tiene: usize, espera: usize },
```

y reexport `validar_tamano_rgba` en `lib.rs`. **Si `CreateBitmap` difiere en windows 0.62** (orden de argumentos o `Option`), ajusta mecánicamente y repórtalo.

- [ ] **Step 4: Ejecutar y comprobar que pasa**

Run: `cargo test -p pixpin-codec -- --test-threads=1` y `cargo test -p pixpin-render --lib -- --test-threads=1 --ignored`
Expected: PASS ambos.

- [ ] **Step 5: Puerta y commit**

```bash
git add crates/pixpin-codec crates/pixpin-render
git commit -m "cargar() en el codec y bitmap_desde_pixeles en el motor

Las dos primitivas del pin: leer el PNG del almacen (pareja exacta de
guardar, con test de ida y vuelta identica) y subir RGBA de CPU a un
bitmap D2D con R8G8B8A8+IGNORE, sin conversion de canales. La validacion
del buffer es una funcion pura probada sin GPU; el test con GPU dibuja
dos pixeles conocidos y los lee de vuelta en BGRA."
```

---
## Task 4: La máquina de interacción del pin, en puro

**Files:**
- Create: `crates/pixpin-pin/src/estado.rs`
- Modify: `crates/pixpin-pin/src/lib.rs`, `crates/pixpin-pin/Cargo.toml`
- Modify: `apps/pixpin/tests/capas.rs` — nada que cambiar (el conteo no varía; `pixpin-pin` ya existe); sólo comprobar que sigue verde con la dependencia nueva.

**Interfaces:**
- Consumes: `pixpin_geom::{Punto, Rect, Esquina, esquina_en, redimension_proporcional}` (Task 1).
- Produces:
  - `pub const ZONA_ESQUINA_LOGICA: u32 = 12;` y `pub const MINIMO_LOGICO: u32 = 48;` (D23 y spec §3.2)
  - `pub enum EventoPin { BotonPulsado(Punto), RatonMovido(Punto), BotonSoltado, DobleClic, Escape }` — los `Punto` en coordenadas del ESCRITORIO (la ventana los traduce)
  - `pub enum EfectoPin { Nada, Mover(Rect), Redimensionar(Rect), AlternarTamano, Cerrar, GestoTerminado(Rect) }` — `Mover`/`Redimensionar` piden recolocar la ventana YA; `GestoTerminado` es la señal de persistir (la escritura-al-soltar del plan)
  - `pub struct EstadoPin { … }` con `pub fn nuevo(rect: Rect, escala_por_cien: u32) -> Self; pub fn procesar(&mut self, e: EventoPin) -> EfectoPin; pub fn rect(&self) -> Rect; pub fn sobre_esquina(&self, p: Punto) -> bool` (para el cursor diagonal)

- [ ] **Step 1: Escribir los tests que fallan**

Crear `crates/pixpin-pin/src/estado.rs` con sólo las pruebas:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_geom::{Punto, Rect};

    fn pin() -> EstadoPin {
        EstadoPin::nuevo(Rect { x: 100, y: 100, ancho: 400, alto: 300 }, 100)
    }

    #[test]
    fn agarrar_por_el_centro_mueve_como_un_objeto() {
        let mut e = pin();
        e.procesar(EventoPin::BotonPulsado(Punto { x: 300, y: 250 }));
        let ef = e.procesar(EventoPin::RatonMovido(Punto { x: 330, y: 270 }));
        assert_eq!(
            ef,
            EfectoPin::Mover(Rect { x: 130, y: 120, ancho: 400, alto: 300 }),
            "se desplaza lo que el raton, sin cambiar tamano"
        );
        // Soltar persiste: la senal de escritura-al-soltar.
        assert_eq!(
            e.procesar(EventoPin::BotonSoltado),
            EfectoPin::GestoTerminado(Rect { x: 130, y: 120, ancho: 400, alto: 300 })
        );
    }

    #[test]
    fn agarrar_por_un_borde_tambien_mueve() {
        // Caso negativo de D23: el borde NO es un tirador; solo las esquinas.
        let mut e = pin();
        e.procesar(EventoPin::BotonPulsado(Punto { x: 300, y: 102 }));
        let ef = e.procesar(EventoPin::RatonMovido(Punto { x: 300, y: 132 }));
        assert!(matches!(ef, EfectoPin::Mover(_)), "el borde mueve, no redimensiona: {ef:?}");
    }

    #[test]
    fn la_esquina_redimensiona_en_proporcion() {
        let mut e = pin();
        // Sureste del rect (100,100,400x300): (499, 399) esta en la zona.
        e.procesar(EventoPin::BotonPulsado(Punto { x: 497, y: 397 }));
        let ef = e.procesar(EventoPin::RatonMovido(Punto { x: 700, y: 500 }));
        let EfectoPin::Redimensionar(r) = ef else {
            panic!("la esquina debe redimensionar: {ef:?}");
        };
        assert_eq!((r.x, r.y), (100, 100), "ancla noroeste clavada");
        let prop = r.ancho as f64 / r.alto as f64;
        assert!((prop - 4.0 / 3.0).abs() < 0.02, "proporcion rota: {prop}");
        assert!(r.ancho > 400);
    }

    #[test]
    fn escape_cierra_y_doble_clic_alterna() {
        let mut e = pin();
        assert_eq!(e.procesar(EventoPin::Escape), EfectoPin::Cerrar);
        assert_eq!(e.procesar(EventoPin::DobleClic), EfectoPin::AlternarTamano);
    }

    #[test]
    fn mover_sin_boton_no_hace_nada() {
        // Caso negativo: el hover puro no arrastra.
        let mut e = pin();
        assert_eq!(
            e.procesar(EventoPin::RatonMovido(Punto { x: 300, y: 250 })),
            EfectoPin::Nada
        );
    }

    #[test]
    fn sobre_esquina_guia_el_cursor_con_la_escala() {
        // A 200% la zona logica de 12 son 24 fisicos.
        let e = EstadoPin::nuevo(Rect { x: 0, y: 0, ancho: 400, alto: 300 }, 200);
        assert!(e.sobre_esquina(Punto { x: 380, y: 280 }), "dentro de 24 px");
        assert!(!e.sobre_esquina(Punto { x: 360, y: 260 }), "fuera de 24 px");
    }
}
```

- [ ] **Step 2: Ejecutar y comprobar que falla**

```bash
cargo add pixpin-geom --path crates/pixpin-geom -p pixpin-pin
```

Run: `cargo test -p pixpin-pin estado -- --test-threads=1`
Expected: FAIL — no existe `EstadoPin`. Y `cargo test -p pixpin --test capas -- --test-threads=1` PASS (L2→L0).

- [ ] **Step 3: Implementar**

```rust
//! La interaccion del pin como maquina pura (D23).
//!
//! Todo el pin se agarra y se mueve como un objeto fisico; SOLO las cuatro
//! esquinas redimensionan, siempre en proporcion. La ventana traduce
//! mensajes Win32 a EventoPin y ejecuta los EfectoPin; asi el
//! comportamiento entero se prueba sin abrir una ventana, igual que el
//! overlay de S1-B2.

use pixpin_geom::{Esquina, Punto, Rect, esquina_en, redimension_proporcional};

/// Zona de esquina en pixeles LOGICOS (D23); la escala la aplica el estado.
pub const ZONA_ESQUINA_LOGICA: u32 = 12;
/// Lado minimo del pin en pixeles logicos (spec 3.2).
pub const MINIMO_LOGICO: u32 = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventoPin {
    BotonPulsado(Punto),
    RatonMovido(Punto),
    BotonSoltado,
    DobleClic,
    Escape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EfectoPin {
    Nada,
    /// Recolocar la ventana ya, sin repintar contenido.
    Mover(Rect),
    /// Cambiar tamano de ventana y repintar.
    Redimensionar(Rect),
    /// Doble clic: 100% <-> ajustado (lo resuelve el dueno, que sabe el
    /// tamano nativo de la imagen).
    AlternarTamano,
    Cerrar,
    /// El gesto acabo: persistir la posicion (escritura-al-soltar).
    GestoTerminado(Rect),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gesto {
    Ninguno,
    Moviendo { agarre: Punto, origen: Rect },
    Redimensionando { esquina: Esquina, origen: Rect },
}

#[derive(Debug)]
pub struct EstadoPin {
    rect: Rect,
    escala_por_cien: u32,
    gesto: Gesto,
}

impl EstadoPin {
    pub fn nuevo(rect: Rect, escala_por_cien: u32) -> Self {
        Self { rect, escala_por_cien: escala_por_cien.max(100), gesto: Gesto::Ninguno }
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    fn zona(&self) -> u32 {
        ZONA_ESQUINA_LOGICA * self.escala_por_cien / 100
    }

    fn minimo(&self) -> u32 {
        MINIMO_LOGICO * self.escala_por_cien / 100
    }

    /// Para el cursor diagonal: el unico feedback de las esquinas (D23).
    pub fn sobre_esquina(&self, p: Punto) -> bool {
        esquina_en(self.rect, p, self.zona()).is_some()
    }

    pub fn procesar(&mut self, evento: EventoPin) -> EfectoPin {
        match evento {
            EventoPin::Escape => EfectoPin::Cerrar,
            EventoPin::DobleClic => EfectoPin::AlternarTamano,
            EventoPin::BotonPulsado(p) => {
                self.gesto = match esquina_en(self.rect, p, self.zona()) {
                    Some(esquina) => Gesto::Redimensionando { esquina, origen: self.rect },
                    None => Gesto::Moviendo { agarre: p, origen: self.rect },
                };
                EfectoPin::Nada
            }
            EventoPin::RatonMovido(p) => match self.gesto {
                Gesto::Ninguno => EfectoPin::Nada,
                Gesto::Moviendo { agarre, origen } => {
                    self.rect = Rect {
                        x: origen.x + (p.x - agarre.x),
                        y: origen.y + (p.y - agarre.y),
                        ancho: origen.ancho,
                        alto: origen.alto,
                    };
                    EfectoPin::Mover(self.rect)
                }
                Gesto::Redimensionando { esquina, origen } => {
                    self.rect = redimension_proporcional(origen, esquina, p, self.minimo());
                    EfectoPin::Redimensionar(self.rect)
                }
            },
            EventoPin::BotonSoltado => {
                if self.gesto == Gesto::Ninguno {
                    return EfectoPin::Nada;
                }
                self.gesto = Gesto::Ninguno;
                EfectoPin::GestoTerminado(self.rect)
            }
        }
    }
}
```

Reemplazar `crates/pixpin-pin/src/lib.rs`:

```rust
//! pixpin-pin — los pines flotantes: el alma de PixPin (spec S2).
//!
//! Este crate habla con Win32; `unsafe` permitido con `// SAFETY:` por
//! bloque. La interaccion vive en `estado`, que es puro y se prueba sin
//! escritorio. La regla de capas prohibe depender de pixpin-store y de
//! pixpin-capture (misma capa L2): el almacen lo toca el ejecutable via
//! el callback CambioPin, y el dispositivo llega como &ID3D11Device.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod estado;

pub use estado::{EfectoPin, EstadoPin, EventoPin, MINIMO_LOGICO, ZONA_ESQUINA_LOGICA};
```

- [ ] **Step 4: Ejecutar y comprobar que pasa** — `cargo test -p pixpin-pin -- --test-threads=1`: los 6 en verde.

- [ ] **Step 5: Puerta y commit**

```bash
git add crates/pixpin-pin Cargo.lock
git commit -m "La interaccion del pin como maquina pura

D23 completo y probado sin ventanas: todo el pin mueve como un objeto
(incluidos los BORDES, que es el caso negativo que distingue al pin del
overlay), solo las esquinas redimensionan y siempre en proporcion, Esc
cierra, doble clic alterna, y soltar emite GestoTerminado: la senal de
escritura-al-soltar con la que el ejecutable persiste sin escribir 60
veces por segundo."
```

---

## Task 5: La ventana del pin, autocontenida

**Files:**
- Create: `crates/pixpin-pin/src/ventana.rs`
- Modify: `crates/pixpin-pin/src/lib.rs`, `crates/pixpin-pin/Cargo.toml`

**Interfaces:**
- Consumes: `EstadoPin`/`EventoPin`/`EfectoPin` (Task 4); `pixpin_render::{MotorRender, Superficie, Color, RectF}`; `pixpin_codec::ImagenRgba` (para el tamaño nativo); `&ID3D11Device` externo.
- Produces:
  - `pub const MARGEN_SOMBRA_LOGICO: u32 = 24;` — la ventana es el rect del contenido MÁS este margen a cada lado (D30)
  - `pub enum CambioPin { Movido(Rect), Redimensionado(Rect), Cerrado }` — `Rect` = contenido en escritorio virtual
  - `pub struct Pin { … }` con:
    - `pub fn nuevo(d3d: &ID3D11Device, motor: Rc<MotorRender>, imagen: &ImagenRgba, rect_contenido: Rect, escala_por_cien: u32, al_cambiar: Box<dyn Fn(CambioPin)>) -> Result<Pin, ErrorPin>` — crea la ventana (`PixPinPin`, WndProc propio), sube la imagen, pinta y muestra `SW_SHOWNOACTIVATE` (spec §4.4: no roba el foco)
    - `pub fn rect_contenido(&self) -> Rect`
  - `Drop` destruye la ventana. `Rc<MotorRender>`: mismo hilo, el motor es de la app y de todos los pines.

**Decisiones de implementación que el ejecutor respeta:**

- Estilos: `WS_POPUP` + `WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW`. Clase `PixPinPin` registrada con `Once`, **WndProc propio que JAMÁS llama a `PostQuitMessage`** (la mina de S1-A, tercera vez).
- **Autocontenida:** el estado interno (`Box<PinInterno>` con estado puro, superficie, bitmap, motor, callback) cuelga de `GWLP_USERDATA`; el WndProc lo recupera y ejecuta los efectos ahí mismo. Los mensajes llegan por el bucle principal de la app (`VentanaMensajes::ejecutar` bombea sin filtro), sin bucle propio ni colas.
- `WM_NCDESTROY` recupera el `Box` y lo suelta (es el punto documentado para liberar USERDATA).
- Efectos: `Mover` → `SetWindowPos` (sin repintar: DComp mueve el visual); `Redimensionar` → `SetWindowPos` + recrear superficie al tamaño nuevo + repintar; `Cerrar` → callback `Cerrado` + `DestroyWindow`; `AlternarTamano` → si el contenido está al 100 % pasa a ajustado (80 % del área de trabajo del monitor actual, spec §3.2) y si no, al 100 % (tamaño nativo de la imagen), con callback `Redimensionado`; `GestoTerminado` → callback `Movido`.
- Dibujo del fotograma (via `motor.dibujar` + `Pintor`): limpiar transparente → **sombra**: 6 rectángulos redondeados concéntricos negros con alfa decreciente (0.10 → 0.02) desplazados 2 px lógicos hacia abajo — difusa sin desenfoque real; el cacheado por bitmap de la spec §6 queda anotado para cuando muchos pines redimensionen a la vez → tarjeta: `rellenar_redondeado` blanco (irrelevante: la imagen la cubre) → la imagen con `p.bitmap(..., nitido=false)` en el rect interior → nada más. Cero cromo.
- **Esquinas redondeadas de la imagen:** D2D recorta con `PushLayer`+geometría redondeada… para S2-A se dibuja la imagen SIN redondear sobre la tarjeta redondeada — el borde redondeado sólo se aprecia en la sombra. Anotado como simplificación consciente; el `PushLayer` llega con el pulido visual de S2-B. *(Si el ejecutor ve trivial el layer redondeado con la API del crate, puede hacerlo ya y reportarlo.)*
- Cursor: `WM_SETCURSOR` pone la diagonal (`IDC_SIZENWSE`/`IDC_SIZENESW`) si `estado.sobre_esquina`, `IDC_SIZEALL` si no.
- Teclado: el pin recibe `WM_KEYDOWN` sólo si tiene foco (clic previo). `Esc` → efecto `Cerrar`. Exactamente lo que pide D23: `Esc` cierra **el enfocado**.

- [ ] **Step 1: Tests que fallan**

Crear `crates/pixpin-pin/src/ventana.rs` con sólo las pruebas:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_codec::ImagenRgba;
    use pixpin_geom::Rect;
    use pixpin_render::MotorRender;
    use std::cell::RefCell;
    use std::rc::Rc;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    };

    fn d3d() -> ID3D11Device {
        let mut d = None;
        // SAFETY: salidas locales, constantes documentadas (patron de
        // pixpin-render).
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d),
                None,
                None,
            )
            .expect("GPU real");
        }
        d.unwrap()
    }

    fn imagen_2x2() -> ImagenRgba {
        ImagenRgba { ancho: 2, alto: 2, pixeles: vec![255; 16] }
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn el_pin_se_crea_visible_y_al_destruirse_no_mata_nada() {
        let d3d = d3d();
        let motor = Rc::new(MotorRender::nuevo(&d3d).unwrap());
        let cambios: Rc<RefCell<Vec<CambioPin>>> = Rc::new(RefCell::new(Vec::new()));
        let c = Rc::clone(&cambios);
        let pin = Pin::nuevo(
            &d3d,
            Rc::clone(&motor),
            &imagen_2x2(),
            Rect { x: 100, y: 100, ancho: 200, alto: 150 },
            100,
            Box::new(move |cambio| c.borrow_mut().push(cambio)),
        )
        .expect("el pin deberia crearse");

        // SAFETY: IsWindowVisible es consulta pura sobre handle vivo.
        let visible = unsafe {
            windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(pin.hwnd()).as_bool()
        };
        assert!(visible, "el pin nace visible (sin robar el foco)");
        assert_eq!(pin.rect_contenido(), Rect { x: 100, y: 100, ancho: 200, alto: 150 });

        // Dos pines: destruir uno no toca al otro (la mina de S1-A).
        let c2 = Rc::clone(&cambios);
        let pin2 = Pin::nuevo(
            &d3d,
            motor,
            &imagen_2x2(),
            Rect { x: 400, y: 100, ancho: 200, alto: 150 },
            100,
            Box::new(move |cambio| c2.borrow_mut().push(cambio)),
        )
        .unwrap();
        let hwnd2 = pin2.hwnd();
        drop(pin);
        // SAFETY: IsWindow consulta pura.
        let vivo2 = unsafe {
            windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(hwnd2)).as_bool()
        };
        assert!(vivo2, "destruir un pin no puede llevarse a los demas");
    }

    #[test]
    fn la_ventana_es_mayor_que_el_contenido_por_el_margen() {
        // Puro: la conversion contenido <-> ventana con margen de sombra.
        let contenido = Rect { x: 100, y: 100, ancho: 200, alto: 150 };
        let v = rect_ventana(contenido, 150);
        let margen = (MARGEN_SOMBRA_LOGICO * 150 / 100) as i32;
        assert_eq!(v.x, 100 - margen);
        assert_eq!(v.ancho, 200 + 2 * margen as u32);
        assert_eq!(contenido_desde_ventana(v, 150), contenido, "ida y vuelta exacta");
    }
}
```

- [ ] **Step 2: Comprobar que falla** — `cargo test -p pixpin-pin ventana -- --test-threads=1`: no compila.

- [ ] **Step 3: Implementar**

```bash
cargo add pixpin-render --path crates/pixpin-render -p pixpin-pin
cargo add pixpin-codec --path crates/pixpin-codec -p pixpin-pin
cargo add thiserror -p pixpin-pin
cargo add windows -p pixpin-pin --features Win32_Foundation,Win32_Graphics_Direct3D11,Win32_Graphics_Gdi,Win32_UI_WindowsAndMessaging,Win32_UI_Input_KeyboardAndMouse,Win32_System_LibraryLoader
```

El esqueleto (los patrones de clase/WndProc/cola son los de `pixpin-shell/src/overlay.rs`, ya revisados — cópialos, adaptando lo señalado):

```rust
//! La ventana del pin: PixPinPin, autocontenida tras GWLP_USERDATA.
//!
//! El pin vive en el bucle principal de la app (VentanaMensajes::ejecutar
//! bombea todos los mensajes del hilo), asi que su WndProc ejecuta los
//! efectos ahi mismo: mover con SetWindowPos, redimensionar recreando la
//! superficie, cerrar destruyendo. El ejecutable se entera por el callback
//! CambioPin (unica via: pixpin-pin no puede tocar el almacen, misma capa).
//!
//! Este WndProc JAMAS llama a PostQuitMessage — tercera vez que la mina de
//! S1-A esta a punto de pisarse, tercera vez que el comentario lo impide.

use std::rc::Rc;

use pixpin_codec::ImagenRgba;
use pixpin_geom::{Punto, Rect};
use pixpin_render::{Color, MotorRender, RectF, Superficie};
// ... imports Win32 (CreateWindowExW, DefWindowProcW, SetWindowLongPtrW,
//     GWLP_USERDATA, WM_NCDESTROY, SetWindowPos, etc.)

pub const MARGEN_SOMBRA_LOGICO: u32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CambioPin {
    Movido(Rect),
    Redimensionado(Rect),
    Cerrado,
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorPin {
    #[error("no se pudo crear la ventana del pin: {0}")]
    Creacion(#[source] windows::core::Error),
    #[error("no se pudo preparar el dibujo del pin: {0}")]
    Dibujo(#[from] pixpin_render::ErrorRender),
}

/// Rect de VENTANA para un contenido: el margen de sombra a cada lado.
pub fn rect_ventana(contenido: Rect, escala_por_cien: u32) -> Rect {
    let m = (MARGEN_SOMBRA_LOGICO * escala_por_cien / 100) as i32;
    Rect {
        x: contenido.x - m,
        y: contenido.y - m,
        ancho: contenido.ancho + 2 * m as u32,
        alto: contenido.alto + 2 * m as u32,
    }
}

/// La inversa exacta de `rect_ventana`.
pub fn contenido_desde_ventana(ventana: Rect, escala_por_cien: u32) -> Rect {
    let m = (MARGEN_SOMBRA_LOGICO * escala_por_cien / 100) as i32;
    Rect {
        x: ventana.x + m,
        y: ventana.y + m,
        ancho: ventana.ancho - 2 * m as u32,
        alto: ventana.alto - 2 * m as u32,
    }
}

struct PinInterno {
    estado: crate::estado::EstadoPin,
    escala_por_cien: u32,
    motor: Rc<MotorRender>,
    d3d: windows::Win32::Graphics::Direct3D11::ID3D11Device,
    superficie: Superficie,
    imagen_nativa: (u32, u32),
    bitmap: windows::Win32::Graphics::Direct2D::ID2D1Bitmap1,
    al_cambiar: Box<dyn Fn(CambioPin)>,
}

pub struct Pin {
    hwnd: windows::Win32::Foundation::HWND,
}

// impl Pin { nuevo, hwnd, rect_contenido } + registrar_clase (Once) +
// procedimiento_pin: traduce WM_LBUTTONDOWN (SetCapture) / WM_MOUSEMOVE /
// WM_LBUTTONUP (ReleaseCapture) / WM_LBUTTONDBLCLK (la clase se registra
// con CS_DBLCLKS) / WM_KEYDOWN(VK_ESCAPE) / WM_SETCURSOR / WM_PAINT
// (ValidateRect + repintar) / WM_NCDESTROY (recuperar y soltar el Box).
// Las coordenadas de raton llegan en cliente y se convierten a escritorio
// sumando la esquina de la ventana (GetWindowRect), igual que el overlay.
// pintar(interno): motor.dibujar(destino de superficie.empezar) con:
//   limpiar_transparente; 6 aros de sombra (rellenar_redondeado con alfa
//   0.10, 0.08, 0.06, 0.045, 0.03, 0.02, cada uno 2 px logicos mas grande,
//   desplazados 2 px logicos hacia abajo); la imagen con p.bitmap en el
//   rect interior (margen..tamano-margen); presentar.
```

**Este esqueleto es contrato, no entregable:** la tarea no está hecha mientras quede un comentario `// impl ...` sin cuerpo. Los cuerpos replican los patrones existentes citados.

Añadir a `lib.rs`: `pub mod ventana;` y `pub use ventana::{CambioPin, ErrorPin, MARGEN_SOMBRA_LOGICO, Pin, contenido_desde_ventana, rect_ventana};`

- [ ] **Step 4: Ejecutar y comprobar que pasa**

Run: `cargo test -p pixpin-pin -- --test-threads=1` (el puro del margen) y `cargo test -p pixpin-pin -- --test-threads=1 --ignored` (el de escritorio).
Expected: PASS ambos.

- [ ] **Step 5: Puerta y commit**

```bash
git add crates/pixpin-pin Cargo.lock
git commit -m "La ventana del pin: PixPinPin autocontenida tras GWLP_USERDATA

Vive en el bucle principal de la app sin bucle propio: su WndProc ejecuta
los efectos de la maquina pura ahi mismo y avisa al ejecutable por el
callback CambioPin — la unica via legal, porque pixpin-pin no puede tocar
el almacen (misma capa L2). WM_NCDESTROY recupera y suelta el Box del
USERDATA; el WndProc jamas llama a PostQuitMessage, y el test destruye un
pin y comprueba que el otro sigue vivo: tercera vez que la mina de S1-A
queda desactivada con test propio.

La sombra difusa son seis aros redondeados concentricos de alfa
decreciente: sin desenfoque real y suficiente para el look de recorte
elevado; el cache por bitmap de la spec queda anotado para S2-B."
```

---
## Task 6: El atajo de pinear y `AccionFinal::Pinear`

**Files:**
- Modify: `crates/pixpin-shell/src/atajos.rs` (ID_PIN), `crates/pixpin-store/src/ajustes.rs` (`Atajos.pin`), `apps/pixpin/src/overlay.rs` (`ModoConfirmacion::Pinear`, `AccionFinal::Pinear`), `apps/pixpin/src/main.rs` (registro del atajo), catálogos `i18n` (texto de fallo de pin si hiciera falta: no — reutiliza `captura-fallo`).

**Interfaces:**
- Produces:
  - `pixpin_shell::atajos::ID_PIN: u32 = 5`
  - `Atajos.pin: Atajo` con valor por defecto `"Ctrl+Alt+F"` (spec §12)
  - `ModoConfirmacion::Pinear` — como `DirectoAlPortapapeles` pero la acción final lleva la **región** (el pin nace 1:1 en su sitio, D26)
  - `AccionFinal::Pinear { imagen: ImagenRgba, region: Rect }`

- [ ] **Step 1: Test del ajuste que falla**

En el módulo de pruebas de `ajustes.rs`, ampliar `los_valores_por_defecto_son_los_del_diseno` con:

```rust
        assert_eq!(a.atajos.pin.to_string(), "Ctrl+Alt+F");
```

Run: `cargo test -p pixpin-store ajustes -- --test-threads=1` → FAIL (no existe el campo).

- [ ] **Step 2: Implementar**

- `atajos.rs`: `pub const ID_PIN: u32 = 5;` (y al reexport de `lib.rs` del crate).
- `ajustes.rs`: campo `pub pin: Atajo` en `Atajos` con default `"Ctrl+Alt+F".parse().expect("atajo por defecto valido")` — cuidado: `Atajos` tiene `#[serde(default)]` por struct, así que el `impl Default` manual es el único sitio a tocar además del campo.
- `overlay.rs` del ejecutable:
  - `ModoConfirmacion::Pinear`
  - `AccionFinal::Pinear { imagen: ImagenRgba, region: Rect }`
  - `QueAccion::Pinear` y en `Efecto::Confirmar`: `Pinear` va directo como `DirectoAlPortapapeles` (sin barra): `PENDIENTE.poner(QueAccion::Pinear, region); Continuar::No`.
  - En la materialización final: `QueAccion::Pinear => AccionFinal::Pinear { imagen, region }`.
- `main.rs`: registrar `(atajos::ID_PIN, config.atajos.pin)` en `peticiones`, y en el `match` de atajos añadir `id == atajos::ID_PIN` con `ModoConfirmacion::Pinear`. El manejo de `AccionFinal::Pinear` lo cablea la Task 7 — hasta entonces, `ejecutar_accion` lo registra con `tracing::info!("pinear pendiente de la Task 7")` y devuelve `Ok(None)` para que este commit compile y funcione solo.

- [ ] **Step 3: Verificar y confirmar**

Run: `cargo test --workspace -- --test-threads=1` PASS · puerta estándar.

```bash
git add crates/pixpin-shell crates/pixpin-store apps/pixpin
git commit -m "Ctrl+Alt+F: el atajo de pinear, y AccionFinal::Pinear con region

El quinto atajo global (reasignable en TOML como los demas) abre el mismo
overlay con ModoConfirmacion::Pinear: confirmar no muestra barra y la
accion final lleva la REGION ademas de la imagen, porque el pin nace 1:1
exactamente donde se recorto (D26). El manejo real llega en la tarea del
gestor; de momento queda registrado en el log."
```

---

## Task 7: El gestor de pines: crear, restaurar, persistir

**Files:**
- Create: `apps/pixpin/src/pines.rs`
- Modify: `apps/pixpin/src/main.rs`, `apps/pixpin/Cargo.toml`

**Interfaces:**
- Consumes: `Almacen`/`PinGuardado`/`Entrada` (Task 2), `Pin`/`CambioPin` (Task 5), `pixpin_codec::{cargar, guardar? no: codificar}`… la imagen del overlay ya está en CPU (`ImagenRgba`); para el almacén hay que codificarla a PNG **en memoria**: `pixpin-codec` gana `pub fn codificar_png(imagen: &ImagenRgba) -> Result<Vec<u8>, ErrorCodec>` (parte de esta tarea; usa `image::write_to` con `Cursor`).
- Produces (interno del ejecutable):
  - `pub struct Pines { … }` con:
    - `pub fn nuevos(raiz: &Path, d3d: ID3D11Device, motor: Rc<MotorRender>) -> Result<Pines>` — abre el almacén.
    - `pub fn pinear(&mut self, imagen: &ImagenRgba, region: Rect, escala_por_cien: u32) -> Result<()>` — codifica PNG → `almacen.guardar_imagen` → crea el `Pin` visible.
    - `pub fn restaurar(&mut self, disposicion: &DisposicionMonitores) -> usize` — por cada entrada con `pin: Some`, carga el PNG, **recoloca** con `recolocar_en_area` si su monitor ya no existe (spec §5.2), crea el `Pin`. Devuelve cuántos restauró; los fallos individuales se registran y no tumban el resto.
  - Los callbacks `CambioPin` actualizan el almacén: `Movido`/`Redimensionado` → `actualizar_pin(id, Some(...))`; `Cerrado` → `actualizar_pin(id, None)` y saca el `Pin` del mapa. **Ojo al préstamo:** el callback no puede capturar `&mut Pines` (el `Pin` vive dentro). Patrón: `Rc<RefCell<Almacen>>` compartido entre `Pines` y cada callback, más una lista `Rc<RefCell<Vec<u64>>>` de cerrados que `Pines::purgar()` drena en el bucle principal (llamado tras cada evento de la bandeja… no: tras cada `Evento` del bucle, barato).

**Decisión de dueño de la textura:** la restauración carga el PNG y crea el pin **secuencialmente** en S2-A (la paralelización con el pool y la puerta de <200 ms del primer pin se miden en la Task 8 y, si no se cumple, se ataca ahí con los números delante — no antes).

- [ ] **Step 1: Test de `codificar_png` que falla**

En pruebas de `pixpin-codec/src/imagen.rs`:

```rust
    #[test]
    fn codificar_png_en_memoria_equivale_a_guardar() {
        let dir = temporal("codificar");
        let ruta = dir.join("disco.png");
        let original = imagen_de_prueba();
        guardar(&original, &ruta, FormatoImagen::Png).unwrap();
        let en_memoria = codificar_png(&original).unwrap();
        // No se exige igualdad byte a byte con el fichero (el encoder puede
        // variar entre rutas), sino la ida y vuelta: decodificar lo
        // codificado devuelve la imagen exacta.
        let vuelta = image::load_from_memory(&en_memoria).unwrap().to_rgba8();
        assert_eq!(vuelta.as_raw(), &original.pixeles);
    }

    #[test]
    fn codificar_una_imagen_vacia_da_error() {
        let vacia = ImagenRgba { ancho: 0, alto: 0, pixeles: vec![] };
        assert!(codificar_png(&vacia).is_err());
    }
```

`codificar_png` (en `imagen.rs`, con reexport):

```rust
/// PNG en memoria: para el almacen, que guarda bytes, no rutas.
pub fn codificar_png(imagen: &ImagenRgba) -> Result<Vec<u8>, ErrorCodec> {
    if imagen.ancho == 0 || imagen.alto == 0 {
        return Err(ErrorCodec::Vacia { ancho: imagen.ancho, alto: imagen.alto });
    }
    let espera = imagen.bytes_esperados();
    if imagen.pixeles.len() != espera {
        return Err(ErrorCodec::TamanoIncoherente {
            ancho: imagen.ancho,
            alto: imagen.alto,
            tiene: imagen.pixeles.len(),
            espera,
        });
    }
    let buffer: image::RgbaImage =
        image::ImageBuffer::from_raw(imagen.ancho, imagen.alto, imagen.pixeles.clone())
            .expect("el tamano se acaba de comprobar");
    let mut salida = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(buffer)
        .write_to(&mut salida, image::ImageFormat::Png)
        .map_err(|fuente| ErrorCodec::Escritura {
            ruta: std::path::PathBuf::from("<memoria>"),
            fuente,
        })?;
    Ok(salida.into_inner())
}
```

- [ ] **Step 2: Escribir `pines.rs`**

Estructura completa (es cableado de piezas probadas; la invariante con test propio es la recolocación, que ya la tiene la Task 1):

```rust
//! El gestor de pines del ejecutable: la UNICA pieza que ve a la vez el
//! almacen (pixpin-store) y las ventanas (pixpin-pin), porque ambos son L2
//! y no pueden verse entre si. D21 en codigo: todo pasa por el almacen
//! primero; el Pin es la vista.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use anyhow::{Context, Result};
use pixpin_codec::{ImagenRgba, cargar, codificar_png};
use pixpin_geom::{DisposicionMonitores, Rect, recolocar_en_area};
use pixpin_pin::{CambioPin, Pin};
use pixpin_render::MotorRender;
use pixpin_store::{Almacen, PinGuardado};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

pub struct Pines {
    almacen: Rc<RefCell<Almacen>>,
    d3d: ID3D11Device,
    motor: Rc<MotorRender>,
    vivos: HashMap<u64, Pin>,
    /// Ids cerrados desde los callbacks; purgar() los drena en el bucle.
    cerrados: Rc<RefCell<Vec<u64>>>,
}

impl Pines {
    pub fn nuevos(raiz: &Path, d3d: ID3D11Device, motor: Rc<MotorRender>) -> Result<Pines> {
        let almacen = Almacen::abrir(raiz).context("no se pudo abrir el almacen")?;
        Ok(Pines {
            almacen: Rc::new(RefCell::new(almacen)),
            d3d,
            motor,
            vivos: HashMap::new(),
            cerrados: Rc::new(RefCell::new(Vec::new())),
        })
    }

    fn guardado_desde(region: Rect, escala: u32) -> PinGuardado {
        PinGuardado {
            x: region.x,
            y: region.y,
            ancho: region.ancho,
            alto: region.alto,
            escala_por_cien: escala,
        }
    }

    fn crear_ventana(
        &mut self,
        id: u64,
        imagen: &ImagenRgba,
        region: Rect,
        escala: u32,
    ) -> Result<()> {
        let almacen = Rc::clone(&self.almacen);
        let cerrados = Rc::clone(&self.cerrados);
        let pin = Pin::nuevo(
            &self.d3d,
            Rc::clone(&self.motor),
            imagen,
            region,
            escala,
            Box::new(move |cambio| {
                let resultado = match cambio {
                    CambioPin::Movido(r) | CambioPin::Redimensionado(r) => almacen
                        .borrow_mut()
                        .actualizar_pin(id, Some(Pines::guardado_desde(r, escala))),
                    CambioPin::Cerrado => {
                        cerrados.borrow_mut().push(id);
                        almacen.borrow_mut().actualizar_pin(id, None)
                    }
                };
                if let Err(e) = resultado {
                    // Perder una posicion no puede tumbar el pin: se
                    // registra y se sigue (el contenido ya esta a salvo).
                    tracing::warn!(?e, id, "no se pudo persistir el cambio del pin");
                }
            }),
        )
        .context("no se pudo crear la ventana del pin")?;
        self.vivos.insert(id, pin);
        Ok(())
    }

    /// D26: el recorte queda flotando 1:1 exactamente donde estaba.
    pub fn pinear(&mut self, imagen: &ImagenRgba, region: Rect, escala: u32) -> Result<()> {
        let png = codificar_png(imagen).context("no se pudo codificar el pin")?;
        let id = self
            .almacen
            .borrow_mut()
            .guardar_imagen(&png, "recorte", Some(Pines::guardado_desde(region, escala)))
            .context("no se pudo guardar en el almacen")?;
        self.crear_ventana(id, imagen, region, escala)
    }

    /// Restaura los pines abiertos del almacen. Los fallos individuales se
    /// registran y no tumban el resto; devuelve cuantos volvieron.
    pub fn restaurar(&mut self, disposicion: &DisposicionMonitores) -> usize {
        let pendientes: Vec<(u64, PinGuardado, std::path::PathBuf)> = {
            let a = self.almacen.borrow();
            a.entradas()
                .iter()
                .filter_map(|e| e.pin.map(|p| (e.id, p, a.ruta_objeto(e))))
                .collect()
        };
        let mut restaurados = 0;
        for (id, guardado, ruta) in pendientes {
            let imagen = match cargar(&ruta) {
                Ok(i) => i,
                Err(e) => {
                    tracing::warn!(?e, id, "pin sin objeto legible; queda solo en el almacen");
                    continue;
                }
            };
            let rect = Rect {
                x: guardado.x,
                y: guardado.y,
                ancho: guardado.ancho,
                alto: guardado.alto,
            };
            // Si el monitor de origen ya no existe (o el pin quedo fuera),
            // se desliza al area de trabajo mas razonable (spec 5.2).
            let (rect, escala) = match disposicion
                .monitores()
                .iter()
                .find(|m| m.area.interseccion(rect).is_some())
            {
                Some(m) => (recolocar_en_area(rect, m.area_trabajo), m.escala_por_cien),
                None => match disposicion.principal() {
                    Some(p) => (recolocar_en_area(rect, p.area_trabajo), p.escala_por_cien),
                    None => (rect, guardado.escala_por_cien),
                },
            };
            match self.crear_ventana(id, &imagen, rect, escala) {
                Ok(()) => restaurados += 1,
                Err(e) => tracing::warn!(?e, id, "no se pudo restaurar el pin"),
            }
        }
        restaurados
    }

    /// Saca de la lista los pines que se cerraron desde su propio WndProc.
    /// Llamar desde el bucle principal; barato (dos punteros si esta vacia).
    pub fn purgar(&mut self) {
        let cerrados: Vec<u64> = self.cerrados.borrow_mut().drain(..).collect();
        for id in cerrados {
            self.vivos.remove(&id);
        }
    }

    pub fn abiertos(&self) -> usize {
        self.vivos.len()
    }
}
```

- [ ] **Step 3: Cablear `main.rs`**

- `mod pines;` + `use pines::Pines;`.
- Tras crear `recursos_overlay`… los pines necesitan `d3d` y `motor`, que viven en `Recursos`. **Cambio pequeño en `overlay.rs`:** `Recursos` gana `pub fn d3d(&self) -> ID3D11Device` (clona la interfaz, es un puntero contado) y `pub fn motor(&self) -> Rc<MotorRender>`… `MotorRender` no está en `Rc` dentro de `Recursos`. Cambiar el campo `motor: MotorRender` a `motor: Rc<MotorRender>` en `Recursos` (los usos internos pasan de `&self.motor` a `&*self.motor` sólo donde el compilador lo pida; `Pieza`/`pintar` reciben `&MotorRender` igual).
- En el bucle: `Pines` se crea de forma perezosa junto a `Recursos` (mismo `Option`+`insert`), y `pines.restaurar(&disposicion)` se llama UNA vez al crearse — pero la restauración debe ocurrir **al arrancar**, no al primer atajo. Resolución: crear `Recursos` y `Pines` **antes del bucle** si el almacén tiene pines abiertos (`Almacen::abrir` + comprobar; si no hay ninguno, perezoso como hasta ahora). El coste (~150 ms tras la bandeja) sólo se paga cuando hay algo que restaurar, y la bandeja ya está visible: el presupuesto de arranque (<300 ms hasta bandeja) no se toca.
- `AccionFinal::Pinear { imagen, region }` en `ejecutar_accion`… `ejecutar_accion` no ve `Pines`. Mover el brazo: manejarlo en el `match` del bucle directamente (donde `pines` está a mano), llamando `pines.pinear(&imagen, region, escala_del_monitor_de_region)` — la escala sale de `enumerar_monitores` ya disponible en el flujo, o del monitor principal si la región no toca ninguno.
- Tras cada evento del bucle: `if let Some(p) = &mut pines { p.purgar(); }`.
- El log al restaurar: `tracing::info!(restaurados, ms = t.elapsed()...)` — alimenta la puerta de la Task 8.

- [ ] **Step 4: Verificación completa**

Run: workspace + `--ignored` + puerta estándar.

- [ ] **Step 5: Commit**

```bash
git add apps/pixpin crates/pixpin-codec Cargo.lock
git commit -m "El gestor de pines: pinear, restaurar y persistir al soltar

La unica pieza que ve a la vez el almacen y las ventanas (ambos L2, no
pueden verse entre si): todo pasa por el almacen primero y el Pin es la
vista (D21). Ctrl+Alt+F deja el recorte flotando 1:1 en su sitio; los
callbacks persisten al soltar el gesto; cerrar marca pin=None sin borrar
nada; y al arrancar se restauran los abiertos, recolocando al area de
trabajo si su monitor desaparecio. La restauracion solo se paga al
arrancar cuando hay algo que restaurar."
```

---

## Task 8: Verificación de extremo a extremo y puertas de la fase

**Files:**
- Create: `medidas/AAAA-MM-DD-<maquina>-s2a.md` (fecha real)
- Modify: el plan (casillas) y, si algún número se desmiente, la spec de S2.

- [ ] **Step 1: Comprobación manual del flujo completo**

Con el binario release (cada punto, si falla, se anota exactamente qué se vio):

1. `Ctrl+Alt+F` → seleccionar una región → `Enter`: el recorte queda **flotando 1:1 en su sitio**, con sombra suave y sin robar el foco de la selección… (el overlay tenía el foco; comprobar que al cerrar el overlay el pin está y el foco vuelve a la app anterior).
2. Arrastrar el pin desde el centro y desde un borde: se mueve pegado al ratón. Desde una esquina: redimensiona proporcional con cursor diagonal.
3. Doble clic: alterna 100 % ↔ ajustado. `Esc` con el pin enfocado (clic antes): se cierra.
4. `Ctrl+Alt+F` de nuevo, crear 2-3 pines más. Cerrar la app desde la bandeja. **Relanzar: los pines reaparecen donde estaban.**
5. Abrir `%APPDATA%\PixPinMax\almacen\` con el Explorador: los PNG están ahí, navegables; `indice.json` es legible.
6. `pixpinmax.toml` con `nivel = "ligero"`: todo lo anterior funciona igual (el nivel no cambia el pin en S2-A).

- [ ] **Step 2: Medir las puertas de la spec §7 aplicables a S2-A**

| Métrica | Objetivo | Cómo |
|---|---|---|
| CPU con 3 pines quietos | 0 % | `Get-Process`, 10 s |
| RAM privada con 3 pines ~400×300 | anotar (el tope de 100/60 MB se auditará con 10 en S2-B) | `Get-Process` |
| Primer pin restaurado visible | < 200 ms | la línea de `tracing` de la restauración |
| Mover un pin | sin repintados (verificar que `pintar` NO corre al mover) | contador temporal o inspección del log |

**Anota los valores reales aunque no cumplan.** Si el primer pin no baja de 200 ms, la paralelización con el pool (spec §7) se implementa entonces, con los números delante.

- [ ] **Step 3: Cerrar**

Run: suite completa + `--ignored` + `cargo deny` + push y PR.

```bash
git add medidas docs/superpowers/plans
git commit -m "Puertas de S2-A medidas y anotadas"
```

---

## Definición de terminado para S2-A

- [ ] Puerta estándar en verde (fmt, clippy contando errores, tests, `--ignored`, deny, capas, baseline)
- [ ] `Ctrl+Alt+F` deja el recorte flotando 1:1 en su sitio, con sombra y sin cromo
- [ ] Mover desde cualquier punto (bordes incluidos), esquinas proporcionales con cursor diagonal, doble clic alterna, `Esc` cierra el enfocado
- [ ] Cerrar un pin NO borra su entrada del almacén (test) y cerrar la app no pierde nada
- [ ] Al reiniciar, los pines reaparecen donde estaban; si su monitor no existe, recolocados en el principal
- [ ] El almacén es navegable con el Explorador; el índice se escribe por temporal+rename (test)
- [ ] Las métricas del Step 2 de la Task 8 anotadas con valores reales en `medidas/`

**Lo siguiente:** S2-B — notas, fichas de archivo, portapapeles con atajo (`Ctrl+Alt+V`), menú contextual completo, grupos con sombra de color y ocultar/mostrar, imán de bordes, `Ctrl+C`, y la auditoría de los topes de RAM con 10 pines.
