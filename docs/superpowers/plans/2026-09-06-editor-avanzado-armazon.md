# Armazón del editor avanzado — Plan de implementación

> **Para quien ejecute esto:** SUB-SKILL OBLIGATORIA — usa
> `superpowers:subagent-driven-development` (recomendada) o
> `superpowers:executing-plans` para ejecutar tarea a tarea. Los pasos usan
> casillas (`- [ ]`) para ir marcando.

**Objetivo:** dar a PixPin Max la máquina que le falta para ser un editor —
selección múltiple, redimensionar, girar, deshacer de verdad— sin perder el
0 % de CPU en reposo ni los 50 ms del atajo global.

**Arquitectura:** la geometría se porta del Android (`Transform.kt`,
`TransformHandles.kt`, `Collision.kt`, `History.kt`, `Organize.kt`) porque es
verdad matemática y vale igual aquí; el gesto se rediseña para ratón y
teclado. Todo lo nuevo vive en `pixpin-motor2d`, que es puro y se prueba sin
escritorio; la ventana solo traduce.

**Pila:** Rust 2024, `serde`, Direct2D vía `pixpin-render`, Win32 vía
`pixpin-shell`. **Ninguna dependencia nueva.**

**Diseño:** `docs/superpowers/specs/2026-09-06-editor-avanzado-design.md`

---

## Restricciones globales

Valen para **todas** las tareas.

- **El código va en español SIN tildes; la documentación, CON tildes.** Mira
  `crates/pixpin-motor2d/src/escena.rs`: «Deshacer es logico», no «lógico».
  Es la convención de todo el proyecto y no se negocia por fichero.
- **`pixpin-motor2d` lleva `#![forbid(unsafe_code)]`.** No se toca.
- **Ninguna dependencia nueva** en ningún `Cargo.toml` (D28).
- **Baseline del procesador: `x86-64` explícito.** Jamás `target-cpu=native`:
  el i3 de la máquina suelo no tiene AVX2 y el binario moriría al arrancar,
  sin mensaje útil (D17).
- **Reutiliza `pixpin_geom::Tirador`**, que ya tiene los ocho nombres
  (`NoroesteEsquina`, `NorteBorde`, `NoresteEsquina`, `EsteBorde`,
  `SuresteEsquina`, `SurBorde`, `SuroesteEsquina`, `OesteBorde`). No crees
  otro enum con los mismos valores: `pixpin-motor2d` ya depende de
  `pixpin-geom`.
- **Las pruebas van en `mod pruebas` al final del fichero**, con nombres que
  son frases: `fn el_marco_de_seleccion_rodea_la_caja_con_holgura()`.
- **Convenio de `escala`:** en este motor `escala` significa *unidades de
  mundo por píxel de pantalla*, o sea `1.0 / zoom`. Así lo usa ya
  `pintado::marco_de_seleccion`. No lo inviertas.
- **Camino caliente = mover el ratón mientras se dibuja.** Cero asignaciones,
  y se invalida la región del último tramo, nunca la pantalla.
- **Techo del historial: 8 MB** (D25).
- **Puerta completa antes de fusionar**, los tres comandos:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace -- --test-threads=1`
- **Una rama y una solicitud de cambios por tarea**, con el porqué escrito.

---

## Estructura de ficheros

| Fichero | Responsabilidad | Tarea |
|---|---|---|
| `pixpin-motor2d/src/escena.rs` | Historial de transacciones | 1 |
| `pixpin-motor2d/src/seleccion.rs` | **Nuevo.** Selección múltiple | 2 |
| `pixpin-motor2d/src/elemento.rs` + `excalidraw.rs` | Campo `grupos` y su puente | 3 |
| `pixpin-motor2d/src/transformar.rs` | **Nuevo.** Escalar y girar con ancla | 4 |
| `pixpin-motor2d/src/tiradores.rs` | **Nuevo.** Dónde caen los 8 + el de giro | 5 |
| `pixpin-motor2d/src/impacto.rs` | + picado múltiple y marquesina | 6 |
| `pixpin-motor2d/src/indice.rs` | **Nuevo.** Rejilla espacial | 7 |
| `pixpin-motor2d/src/cache.rs` | **Nuevo.** Caché de geometría por `version` | 8 |
| `pixpin-motor2d/src/gesto.rs` | **Nuevo.** La máquina de estados | 9 |
| `pixpin-shell/src/overlay.rs` | + `alt` en `Tecla`, + cursor de giro | 10 |
| `apps/pixpin/src/ventana_editor.rs` | **Nuevo.** La ventana | 11 |
| `pixpin-motor2d/src/organizar.rs` | **Nuevo.** Orden, grupos, alinear | 12 |
| `pixpin-ui/src/propiedades.rs` | **Nuevo.** Qué se ajusta de qué | 13 |
| `pixpin-render/src/lienzo.rs` | + capa estática cacheada | 14 |
| `pixpin-motor2d/tests/asignaciones.rs` | **Nuevo.** El asignador que cuenta | 15 |

**Hito:** al acabar la tarea 11 hay un editor que se abre, dibuja, selecciona,
redimensiona, gira y deshace. De la 12 a la 15 es acabado y demostración.

**Orden obligado:** 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11. De la 12 a
la 15 pueden ir en cualquier orden entre ellas.

---

## Tarea 1: El historial de transacciones

Hoy `Cambio` tiene tres variantes y no cubre redimensionar, girar ni cambiar
de color. Y cada movimiento es su propio paso, así que arrastrar cuarenta
elementos serían cuarenta `Ctrl+Z`.

**Ficheros:**
- Modificar: `crates/pixpin-motor2d/src/escena.rs` — el `enum Cambio` de la
  línea 39 y `deshacer`/`rehacer`/`invertir` de las líneas 166-212
- Modificar: `crates/pixpin-motor2d/src/elemento.rs` — añadir `bytes()`
- Modificar: `apps/pixpin/src/capa.rs`, `apps/pixpin/src/pines.rs` — llamadas
- Prueba: en `escena.rs`, dentro de `mod pruebas`

**Interfaces:**
- Consume: `Elemento` y `Escena` tal como están hoy.
- Produce:
  ```rust
  impl Escena {
      pub fn abrir_paso(&mut self);
      pub fn cerrar_paso(&mut self);
      pub fn cancelar_paso(&mut self);
      pub fn apuntar_edicion(&mut self, id: u64);
      pub fn deshacer(&mut self) -> bool;   // OJO: era Option<u64>
      pub fn rehacer(&mut self) -> bool;    // OJO: era Option<u64>
      pub fn bytes_de_historial(&self) -> usize;
  }
  impl Elemento {
      pub fn bytes(&self) -> usize;
  }
  ```

- [ ] **Paso 1: Escribe las pruebas que fallan**

En `escena.rs`, dentro de `mod pruebas`:

```rust
#[test]
fn un_paso_agrupa_todos_los_cambios_del_gesto() {
    let mut escena = Escena::nueva();
    let a = escena.anadir(base());
    let b = escena.anadir(base());
    let c = escena.anadir(base());

    escena.abrir_paso();
    for id in [a, b, c] {
        escena.apuntar_edicion(id);
        escena.buscar_mut(id).unwrap().mover(10.0, 0.0);
    }
    escena.cerrar_paso();

    assert!(escena.deshacer(), "tiene que haber algo que deshacer");
    assert_eq!(escena.buscar(a).unwrap().x, 0.0, "el primero vuelve");
    assert_eq!(escena.buscar(c).unwrap().x, 0.0, "y el tercero tambien");
    assert!(escena.deshacer(), "queda deshacer el haber anadido");
}

#[test]
fn treinta_ciclos_de_deshacer_y_rehacer_no_deforman_el_dibujo() {
    // El motivo de D24. Con la operacion inversa en vez del estado
    // anterior, escalar por 1,5 y dividir por 1,5 acumularia error en coma
    // flotante hasta que el dibujo se nota torcido.
    let mut escena = Escena::nueva();
    let id = escena.anadir(base());
    let partida = escena.buscar(id).unwrap().clone();

    for _ in 0..30 {
        escena.abrir_paso();
        escena.apuntar_edicion(id);
        let e = escena.buscar_mut(id).unwrap();
        e.ancho *= 1.5;
        e.alto *= 1.5;
        e.angulo += 0.37;
        e.tocar();
        escena.cerrar_paso();

        assert!(escena.deshacer());
    }

    assert_eq!(
        escena.buscar(id).unwrap(),
        &partida,
        "identico bit a bit, no aproximado"
    );
}

#[test]
fn cancelar_un_paso_deja_todo_como_estaba_y_no_ensucia_el_historial() {
    let mut escena = Escena::nueva();
    let id = escena.anadir(base());

    escena.abrir_paso();
    escena.apuntar_edicion(id);
    escena.buscar_mut(id).unwrap().mover(500.0, 500.0);
    escena.cancelar_paso();

    assert_eq!(escena.buscar(id).unwrap().x, 0.0, "vuelve a su sitio");
    assert!(escena.deshacer(), "queda el paso de haberlo anadido");
    assert!(!escena.deshacer(), "y nada mas: el arrastre cancelado no cuenta");
}

#[test]
fn un_gesto_sin_cambios_no_consume_un_ctrl_zeta() {
    let mut escena = Escena::nueva();
    escena.anadir(base());
    escena.abrir_paso();
    escena.cerrar_paso();

    assert!(escena.deshacer(), "el anadido");
    assert!(!escena.deshacer(), "el clic sin arrastrar no dejo paso");
}

#[test]
fn apuntar_dos_veces_el_mismo_elemento_guarda_solo_el_estado_original() {
    // Mover el raton produce cien avisos por gesto. Si cada uno guardara
    // una instantanea, arrastrar un trazo largo se comeria el techo de
    // memoria en un solo arrastre.
    let mut escena = Escena::nueva();
    let id = escena.anadir(base());

    escena.abrir_paso();
    for _ in 0..100 {
        escena.apuntar_edicion(id);
        escena.buscar_mut(id).unwrap().mover(1.0, 0.0);
    }
    escena.cerrar_paso();

    assert!(escena.deshacer());
    assert_eq!(escena.buscar(id).unwrap().x, 0.0, "vuelve al origen entero");
}

#[test]
fn el_historial_no_pasa_de_ocho_megas() {
    // D25: el techo va en memoria, no en numero de pasos.
    let mut escena = Escena::nueva();
    let puntos: Vec<Punto2> = (0..492)
        .map(|i| Punto2::nuevo(i as f32, (i * 2) as f32))
        .collect();
    let id = escena.anadir(Elemento {
        figura: Figura::Lapiz { puntos, presiones: Vec::new() },
        ..base()
    });

    for _ in 0..500 {
        escena.abrir_paso();
        escena.apuntar_edicion(id);
        escena.buscar_mut(id).unwrap().mover(1.0, 0.0);
        escena.cerrar_paso();
    }

    assert!(
        escena.bytes_de_historial() <= 8 * 1024 * 1024,
        "el historial ocupa {} bytes",
        escena.bytes_de_historial()
    );
    assert!(escena.deshacer(), "y aun asi se deshace lo reciente");
}
```

Si `mod pruebas` de `escena.rs` no tiene ya un `fn base()`, añádelo (copia el
de `pintado.rs`, que construye un `Elemento` con `Figura::Rectangulo`,
100×50, semilla 1).

- [ ] **Paso 2: Comprueba que fallan**

```
cargo test -p pixpin-motor2d escena -- --test-threads=1
```

Esperado: no compila — `no method named 'abrir_paso' found for struct 'Escena'`.

- [ ] **Paso 3: Cambia el modelo del historial**

En `escena.rs`, sustituye el `enum Cambio` de la línea 39:

```rust
/// Un cambio suelto. Cada uno sabe invertirse.
#[derive(Debug, Clone, PartialEq)]
enum Cambio {
    Anadido(u64),
    Borrado(u64),
    /// El elemento entero **antes** del cambio.
    ///
    /// Guardar el estado anterior y no la operacion inversa es D24, y no es
    /// preferencia de estilo: en coma flotante `(a * 1.5) / 1.5` no siempre
    /// devuelve `a`. Con la operacion inversa, treinta ciclos de deshacer y
    /// rehacer deforman el dibujo poco a poco — un fallo que aparece en
    /// casa del usuario y no en las pruebas.
    Editado { id: u64, antes: Box<Elemento> },
}

/// Todo lo que hizo un gesto. Un arrastre que mueve cuarenta elementos es
/// **un** paso de deshacer, no cuarenta.
#[derive(Debug, Clone, PartialEq, Default)]
struct Paso {
    cambios: Vec<Cambio>,
}

impl Cambio {
    /// Lo que ocupa de cara al techo del historial.
    fn bytes(&self) -> usize {
        match self {
            Cambio::Anadido(_) | Cambio::Borrado(_) => size_of::<Cambio>(),
            Cambio::Editado { antes, .. } => size_of::<Cambio>() + antes.bytes(),
        }
    }
}

/// Techo del historial (D25). Ocho megas son quinientos arrastres de un
/// trazo de 492 puntos —mas de lo que nadie deshace de una sentada— y una
/// fraccion asumible de los ~1,5 GB que Windows 10 deja libres en la
/// maquina suelo de 4 GB.
const TECHO_HISTORIAL: usize = 8 * 1024 * 1024;

fn bytes_de(paso: &Paso) -> usize {
    paso.cambios.iter().map(Cambio::bytes).sum()
}
```

Y cambia los campos de `Escena`:

```rust
    #[serde(skip)]
    historia: Vec<Paso>,
    #[serde(skip)]
    rehacer: Vec<Paso>,
    /// El paso que se esta construyendo, entre `abrir_paso` y `cerrar_paso`.
    #[serde(skip)]
    en_curso: Option<Paso>,
    /// Lo que ocupan `historia` y `rehacer`, para no tener que recorrerlos.
    #[serde(skip)]
    bytes_historial: usize,
```

Añade `en_curso: None` y `bytes_historial: 0` a `impl Default for Escena`.

- [ ] **Paso 4: Añade `bytes()` a `Elemento`**

En `elemento.rs`, dentro de `impl Elemento`:

```rust
    /// Lo que ocupa de verdad, contando lo que hay al otro lado de los
    /// punteros. `size_of` solo cuenta la cabecera, y un trazo de 492
    /// puntos son cuatro kilobytes que no apareceran en el techo.
    pub fn bytes(&self) -> usize {
        let dentro = match &self.figura {
            Figura::Lapiz { puntos, presiones } => {
                puntos.len() * size_of::<Punto2>() + presiones.len() * size_of::<f32>()
            }
            Figura::Resaltador { puntos }
            | Figura::Linea { puntos }
            | Figura::Flecha { puntos, .. } => puntos.len() * size_of::<Punto2>(),
            Figura::Texto { texto, familia, .. } => texto.len() + familia.len(),
            _ => 0,
        };
        size_of::<Elemento>() + dentro
    }
```

(En la tarea 3, cuando exista `grupos`, se le suma
`self.grupos.iter().map(String::len).sum::<usize>()`.)

- [ ] **Paso 5: Escribe las transacciones**

Sustituye `deshacer`, `rehacer` e `invertir` (líneas 166-212) por:

```rust
    /// Empieza un gesto. Todo lo que pase hasta `cerrar_paso` sera un solo
    /// paso de deshacer.
    ///
    /// Abrir dos veces sin cerrar no es error: el segundo abrir no hace
    /// nada. La ventana puede recibir un `WM_LBUTTONDOWN` sin su
    /// `WM_LBUTTONUP` si el usuario suelta fuera, y perder el gesto entero
    /// por eso seria peor que ignorarlo.
    pub fn abrir_paso(&mut self) {
        if self.en_curso.is_none() {
            self.en_curso = Some(Paso::default());
        }
    }

    /// Guarda el estado actual de un elemento para poder volver a el.
    ///
    /// Apuntarlo dos veces dentro del mismo paso guarda **solo la primera**:
    /// mover el raton produce cien avisos por gesto, y cien instantaneas de
    /// un trazo largo se comerian el techo en un solo arrastre.
    pub fn apuntar_edicion(&mut self, id: u64) {
        let Some(paso) = &self.en_curso else { return };
        let ya_esta = paso
            .cambios
            .iter()
            .any(|c| matches!(c, Cambio::Editado { id: i, .. } if *i == id));
        if ya_esta {
            return;
        }
        let Some(e) = self.buscar(id) else { return };
        let cambio = Cambio::Editado { id, antes: Box::new(e.clone()) };
        if let Some(paso) = &mut self.en_curso {
            paso.cambios.push(cambio);
        }
    }

    /// Cierra el gesto. Un paso sin cambios no entra en el historial: hacer
    /// clic sin arrastrar no debe consumir un `Ctrl+Z`.
    pub fn cerrar_paso(&mut self) {
        let Some(paso) = self.en_curso.take() else { return };
        if paso.cambios.is_empty() {
            return;
        }
        self.bytes_historial += bytes_de(&paso);
        self.historia.push(paso);
        for p in self.rehacer.drain(..) {
            self.bytes_historial -= bytes_de(&p);
        }
        self.podar();
    }

    /// Deshace el gesto en curso sin apuntarlo. Es lo que hace `Escape` a
    /// mitad de un arrastre, y sale gratis porque el paso ya guarda el
    /// estado anterior de lo que se estaba tocando.
    pub fn cancelar_paso(&mut self) {
        let Some(paso) = self.en_curso.take() else { return };
        self.aplicar_inverso(&paso);
    }

    /// Deshace el ultimo gesto. Devuelve si habia algo que deshacer.
    pub fn deshacer(&mut self) -> bool {
        let Some(paso) = self.historia.pop() else { return false };
        self.bytes_historial -= bytes_de(&paso);
        let inverso = self.aplicar_inverso(&paso);
        self.bytes_historial += bytes_de(&inverso);
        self.rehacer.push(inverso);
        true
    }

    /// Rehace el ultimo gesto deshecho.
    pub fn rehacer(&mut self) -> bool {
        let Some(paso) = self.rehacer.pop() else { return false };
        self.bytes_historial -= bytes_de(&paso);
        let inverso = self.aplicar_inverso(&paso);
        self.bytes_historial += bytes_de(&inverso);
        self.historia.push(inverso);
        true
    }

    /// Aplica el paso al reves y devuelve el paso que lo desharia.
    ///
    /// Que devuelva su propio inverso es lo que hace que deshacer y rehacer
    /// sean la misma funcion. Con dos funciones distintas, acabarian
    /// discrepando en cuanto se anadiera una variante a `Cambio`.
    fn aplicar_inverso(&mut self, paso: &Paso) -> Paso {
        let mut inverso = Paso::default();
        // Al reves: si un gesto borro y luego anadio, deshacerlo tiene que
        // quitar lo anadido antes de restaurar lo borrado.
        for c in paso.cambios.iter().rev() {
            match c {
                Cambio::Anadido(id) => {
                    self.borrar(*id);
                    inverso.cambios.push(Cambio::Borrado(*id));
                }
                Cambio::Borrado(id) => {
                    self.restaurar(*id);
                    inverso.cambios.push(Cambio::Anadido(*id));
                }
                Cambio::Editado { id, antes } => {
                    if let Some(i) = self.elementos.iter().position(|e| e.id == *id) {
                        let ahora = self.elementos[i].clone();
                        self.elementos[i] = (**antes).clone();
                        inverso.cambios.push(Cambio::Editado {
                            id: *id,
                            antes: Box::new(ahora),
                        });
                    }
                }
            }
        }
        inverso
    }

    /// Tira los pasos mas antiguos hasta caber en el techo (D25).
    ///
    /// El techo va en memoria y no en numero de pasos porque quinientos
    /// pasos son ocho kilobytes o doscientos megas segun lo que se haya
    /// tocado, y en un equipo de 4 GB eso no se puede prometer.
    fn podar(&mut self) {
        while self.bytes_historial > TECHO_HISTORIAL && self.historia.len() > 1 {
            let viejo = self.historia.remove(0);
            self.bytes_historial -= bytes_de(&viejo);
        }
    }

    /// Lo que ocupa el historial ahora mismo.
    pub fn bytes_de_historial(&self) -> usize {
        self.bytes_historial
    }

    /// Si hay algo que deshacer en esta sesion.
    pub fn hay_que_deshacer(&self) -> bool {
        !self.historia.is_empty()
    }
```

**Ojo con `anadir()`:** hoy mete `Cambio::Anadido` directamente en
`self.historia`. Ahora tiene que envolverlo en un `Paso` de un solo cambio si
no hay paso abierto, y meterlo en el paso en curso si lo hay:

```rust
    // Dentro de `anadir`, donde hoy hace `self.historia.push(Cambio::Anadido(id))`:
    self.apuntar_anadido(id);

    // Y un ayudante privado nuevo:
    fn apuntar_anadido(&mut self, id: u64) {
        let cambio = Cambio::Anadido(id);
        match &mut self.en_curso {
            Some(paso) => paso.cambios.push(cambio),
            None => {
                let paso = Paso { cambios: vec![cambio] };
                self.bytes_historial += bytes_de(&paso);
                self.historia.push(paso);
                self.rehacer.clear();
            }
        }
    }
```

Haz lo mismo con `borrar_apuntando` y `apuntar_movimiento`: donde antes
metían un `Cambio` suelto, ahora va al paso en curso o a un paso propio.

- [ ] **Paso 6: Arregla a quien llamaba a lo viejo**

`deshacer()` y `rehacer()` devolvían `Option<u64>`. Encuentra a quién afecta:

```
cargo build --workspace 2>&1 | grep -B2 -A4 "deshacer\|rehacer" | head -40
```

Los sitios conocidos son `apps/pixpin/src/capa.rs` y `apps/pixpin/src/pines.rs`.
Ninguno usa el `u64` para otra cosa que redibujar, así que el cambio es
mecánico: `if let Some(_) = escena.deshacer()` pasa a `if escena.deshacer()`.

- [ ] **Paso 7: Comprueba que pasan y que no rompiste nada**

```
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Esperado: todo verde, y las pruebas que ya había siguen pasando.

- [ ] **Paso 8: Commit**

```bash
git add crates/pixpin-motor2d/src/escena.rs crates/pixpin-motor2d/src/elemento.rs apps/pixpin/src/capa.rs apps/pixpin/src/pines.rs
git commit -m "El historial pasa de tres variantes a transacciones

Cambio solo sabia de anadir, borrar y mover: redimensionar, girar y
cambiar de color no se podian deshacer. Y cada movimiento era su propio
paso, asi que arrastrar cuarenta elementos habrian sido cuarenta Ctrl+Z.

Ahora un gesto es un paso (abrir_paso/cerrar_paso) y Cambio::Editado
guarda el elemento entero de antes. Guardar el estado anterior y no la
operacion inversa es D24, y no es preferencia: en coma flotante
(a * 1.5) / 1.5 no siempre devuelve a, y treinta ciclos de deshacer
deforman el dibujo. La prueba lo exige identico bit a bit.

El techo va en memoria y no en numero de pasos (D25): quinientos pasos
son ocho kilobytes o doscientos megas segun lo que se haya tocado, y en
un equipo de 4 GB eso no se puede prometer.

Sale gratis cancelar_paso, que es lo que hace Escape a mitad de un
arrastre: el paso ya guarda el estado anterior, asi que cancelar es
aplicarlo y tirar el paso."
```

---

## Tarea 2: Selección múltiple

Hoy `elemento_en()` devuelve un `Option<u64>` y `marco_de_seleccion()` dibuja
el marco de uno. No existe seleccionar varios, y sin eso no hay editor: no se
puede mover un grupo, ni alinear, ni borrar de una vez.

**Ficheros:**
- Crear: `crates/pixpin-motor2d/src/seleccion.rs`
- Modificar: `crates/pixpin-motor2d/src/lib.rs` — declarar y reexportar
- Prueba: en `seleccion.rs`, `mod pruebas`

**Interfaces:**
- Consume: `Escena`, `Elemento`, `Punto2` de la tarea 1.
- Produce:
  ```rust
  pub struct Seleccion { /* campos privados */ }
  impl Seleccion {
      pub fn nueva() -> Self;
      pub fn ids(&self) -> &[u64];
      pub fn esta_vacia(&self) -> bool;
      pub fn cuantos(&self) -> usize;
      pub fn contiene(&self, id: u64) -> bool;
      pub fn poner(&mut self, id: u64);
      pub fn alternar(&mut self, id: u64);
      pub fn poner_todos(&mut self, ids: impl IntoIterator<Item = u64>);
      pub fn limpiar(&mut self);
      pub fn capacidad(&self) -> usize;
      pub fn caja(&self, escena: &Escena) -> Option<(f32, f32, f32, f32)>;
      pub fn centro(&self, escena: &Escena) -> Option<Punto2>;
  }
  ```

- [ ] **Paso 1: Escribe las pruebas que fallan**

Crea `crates/pixpin-motor2d/src/seleccion.rs` con solo esto:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};

    fn rect(x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho,
            alto,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
        }
    }

    #[test]
    fn poner_sustituye_y_alternar_anade_o_quita() {
        let mut s = Seleccion::nueva();
        s.poner(1);
        assert_eq!(s.ids(), &[1]);

        // Clic normal sustituye.
        s.poner(2);
        assert_eq!(s.ids(), &[2]);

        // Shift+clic anade.
        s.alternar(3);
        assert_eq!(s.ids(), &[2, 3]);

        // Shift+clic sobre lo ya elegido lo quita.
        s.alternar(2);
        assert_eq!(s.ids(), &[3]);
    }

    #[test]
    fn poner_dos_veces_el_mismo_no_lo_duplica() {
        let mut s = Seleccion::nueva();
        s.alternar(7);
        s.poner(7);
        assert_eq!(s.ids(), &[7], "una sola vez");
    }

    #[test]
    fn la_caja_de_varios_abarca_a_todos() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = escena.anadir(rect(100.0, 50.0, 20.0, 20.0));

        let mut s = Seleccion::nueva();
        s.poner_todos([a, b]);

        let (x0, y0, x1, y1) = s.caja(&escena).unwrap();
        assert_eq!((x0, y0), (0.0, 0.0));
        assert_eq!((x1, y1), (120.0, 70.0));
        assert_eq!(s.centro(&escena).unwrap(), Punto2::nuevo(60.0, 35.0));
    }

    #[test]
    fn un_elemento_borrado_no_cuenta_para_la_caja() {
        // Deshacer es borrado logico: el elemento sigue en la lista. Si la
        // caja lo contara, el marco abarcaria cosas que el usuario no ve.
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = escena.anadir(rect(500.0, 500.0, 10.0, 10.0));
        escena.borrar(b);

        let mut s = Seleccion::nueva();
        s.poner_todos([a, b]);

        let (_, _, x1, y1) = s.caja(&escena).unwrap();
        assert_eq!((x1, y1), (10.0, 10.0), "el borrado no estira la caja");
    }

    #[test]
    fn una_seleccion_de_solo_borrados_no_tiene_caja() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        escena.borrar(a);

        let mut s = Seleccion::nueva();
        s.poner(a);
        assert!(s.caja(&escena).is_none());
    }

    #[test]
    fn limpiar_conserva_la_capacidad_para_no_reasignar() {
        // D26: arrastrar una marquesina es camino caliente. El Vec se
        // reutiliza con clear(), que no devuelve la memoria al sistema.
        let mut s = Seleccion::nueva();
        s.poner_todos(1..=50);
        let capacidad = s.capacidad();
        s.limpiar();
        assert!(s.esta_vacia());
        assert_eq!(s.capacidad(), capacidad, "clear() no reasigna");
    }
}
```

- [ ] **Paso 2: Comprueba que falla**

Declara el módulo en `lib.rs` (`pub mod seleccion;`) y corre:

```
cargo test -p pixpin-motor2d seleccion -- --test-threads=1
```

Esperado: `cannot find type 'Seleccion' in this scope`.

- [ ] **Paso 3: Escribe la implementación**

Encima del `mod pruebas`, en el mismo fichero:

```rust
//! Que elementos estan elegidos.
//!
//! # Por que un `Vec` y no un `HashSet` (D26)
//!
//! Una seleccion de trabajo son de uno a veinte elementos. Buscar
//! linealmente en veinte es mas rapido que calcular un hash, y sobre todo:
//! el `Vec` se reutiliza con `clear()`, que conserva la memoria ya pedida.
//! Arrastrar una marquesina reconstruye la seleccion en cada aviso del
//! raton —camino caliente— y ahi la regla es cero asignaciones.
//!
//! El orden de los ids no significa nada, pero es estable a proposito: un
//! `HashSet` lo cambiaria de un recorrido a otro, y con el cambiaria el
//! orden en que se pintan los marcos.

use crate::elemento::Elemento;
use crate::escena::Escena;
use crate::vector::Punto2;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Seleccion {
    ids: Vec<u64>,
}

impl Seleccion {
    pub fn nueva() -> Self {
        Self::default()
    }

    pub fn ids(&self) -> &[u64] {
        &self.ids
    }

    pub fn esta_vacia(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn cuantos(&self) -> usize {
        self.ids.len()
    }

    pub fn contiene(&self, id: u64) -> bool {
        self.ids.contains(&id)
    }

    /// Lo que hace un clic normal: sustituye la seleccion entera.
    pub fn poner(&mut self, id: u64) {
        self.ids.clear();
        self.ids.push(id);
    }

    /// Lo que hace `Shift` + clic: lo anade si no estaba, lo quita si estaba.
    pub fn alternar(&mut self, id: u64) {
        match self.ids.iter().position(|x| *x == id) {
            Some(i) => {
                self.ids.remove(i);
            }
            None => self.ids.push(id),
        }
    }

    /// Lo que hace la marquesina o `Ctrl+A`: sustituye por todos estos.
    pub fn poner_todos(&mut self, ids: impl IntoIterator<Item = u64>) {
        self.ids.clear();
        for id in ids {
            if !self.ids.contains(&id) {
                self.ids.push(id);
            }
        }
    }

    /// Vacia la seleccion **conservando la memoria pedida** (D26).
    pub fn limpiar(&mut self) {
        self.ids.clear();
    }

    /// Para la prueba de que `limpiar` no reasigna.
    pub fn capacidad(&self) -> usize {
        self.ids.capacity()
    }

    /// Los elementos elegidos que siguen vivos.
    ///
    /// Filtra los borrados porque deshacer es borrado logico: el elemento
    /// sigue en la lista, y si contara, el marco abarcaria cosas que el
    /// usuario no ve en la pantalla.
    fn vivos<'a>(&'a self, escena: &'a Escena) -> impl Iterator<Item = &'a Elemento> + 'a {
        self.ids
            .iter()
            .filter_map(move |id| escena.buscar(*id))
            .filter(|e| !e.borrado)
    }

    /// La caja que abarca todo lo elegido, paralela a los ejes.
    ///
    /// Paralela a los ejes aunque los elementos esten girados: es lo que
    /// hacen Excalidraw y el Android. Cada elemento conserva su propio
    /// angulo; la caja es solo el marco desde el que se tira.
    pub fn caja(&self, escena: &Escena) -> Option<(f32, f32, f32, f32)> {
        let mut caja: Option<(f32, f32, f32, f32)> = None;
        for e in self.vivos(escena) {
            let (x0, y0, x1, y1) = e.caja();
            caja = Some(match caja {
                None => (x0, y0, x1, y1),
                Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            });
        }
        caja
    }

    /// El centro de la caja: alrededor de el se gira y se escala.
    pub fn centro(&self, escena: &Escena) -> Option<Punto2> {
        let (x0, y0, x1, y1) = self.caja(escena)?;
        Some(Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0))
    }
}
```

- [ ] **Paso 4: Declara y reexporta**

En `crates/pixpin-motor2d/src/lib.rs`, manteniendo el orden alfabético que ya
tiene:

```rust
pub mod seleccion;
pub use seleccion::Seleccion;
```

- [ ] **Paso 5: Comprueba que pasan**

```
cargo test -p pixpin-motor2d seleccion -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

- [ ] **Paso 6: Commit**

```bash
git add crates/pixpin-motor2d/src/seleccion.rs crates/pixpin-motor2d/src/lib.rs
git commit -m "Seleccion multiple

elemento_en() devuelve un Option<u64> y marco_de_seleccion() dibuja el
marco de uno. Sin seleccionar varios no hay editor: no se puede mover un
grupo, ni alinear, ni borrar de una vez.

Un Vec y no un HashSet (D26). Una seleccion de trabajo son de uno a
veinte elementos: buscar linealmente en veinte gana a calcular un hash, y
clear() conserva la memoria pedida. Arrastrar una marquesina reconstruye
la seleccion en cada aviso del raton, que es camino caliente, y ahi la
regla es cero asignaciones. Hay una prueba que lo exige.

La caja de varios es paralela a los ejes aunque los elementos esten
girados, como en Excalidraw y en el Android: cada uno conserva su angulo
y la caja es solo el marco desde el que se tira. Y filtra los borrados,
porque deshacer es borrado logico y si no el marco abarcaria cosas que el
usuario no ve."
```

---

## Tarea 3: Grupos, y que sobrevivan el viaje al móvil

El puente ya conserva los `groupIds` del móvil dentro del JSON original —
`elemento_hacia` escribe encima del `Value` de partida en vez de construirlo
de cero (línea 266 de `excalidraw.rs`)— pero Windows no puede **leerlos**. Un
plano hecho en el teléfono se abre aquí como un montón de piezas sueltas.

**Ficheros:**
- Modificar: `crates/pixpin-motor2d/src/elemento.rs` — campo `grupos`, y
  sumarlo en `bytes()`
- Modificar: `crates/pixpin-motor2d/src/excalidraw.rs` — leerlo en
  `elemento_desde` (línea 183), escribirlo en `elemento_hacia` (línea 269)
- Prueba: en `excalidraw.rs`, `mod pruebas`

**Interfaces:**
- Consume: `Elemento` de la tarea 1, con su `bytes()`.
- Produce: `Elemento { pub grupos: Vec<String>, .. }`

- [ ] **Paso 1: Escribe las pruebas que fallan**

En `mod pruebas` de `excalidraw.rs`. **Ajusta los nombres de las funciones de
carga y guardado a los reales del fichero** — mira las pruebas que ya tiene y
copia de ahí la forma de llamarlas y de llegar a la escena:

```rust
#[test]
fn los_grupos_del_movil_llegan_al_escritorio() {
    let json = r#"{
        "type": "excalidraw",
        "elements": [
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "strokeColor":"#000000","seed":1,"groupIds":["g1","g2"]},
            {"type":"rectangle","x":20,"y":0,"width":10,"height":10,
             "strokeColor":"#000000","seed":2,"groupIds":[]}
        ]
    }"#;
    let lienzo = cargar(json).unwrap();
    assert_eq!(lienzo.escena.elementos[0].grupos, vec!["g1", "g2"]);
    assert!(lienzo.escena.elementos[1].grupos.is_empty());
}

#[test]
fn un_elemento_sin_grupos_se_lee_igual() {
    // Compatibilidad hacia atras: los ficheros que ya guardamos no llevan
    // el campo y tienen que seguir abriendo.
    let json = r#"{"type":"excalidraw","elements":[
        {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
         "strokeColor":"#000000","seed":1}
    ]}"#;
    let lienzo = cargar(json).unwrap();
    assert!(lienzo.escena.elementos[0].grupos.is_empty());
}

#[test]
fn los_grupos_sobreviven_la_ida_y_la_vuelta() {
    let json = r#"{"type":"excalidraw","elements":[
        {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
         "strokeColor":"#000000","seed":1,"groupIds":["g1"]}
    ]}"#;
    let lienzo = cargar(json).unwrap();
    let otra_vez = cargar(&guardar(&lienzo)).unwrap();
    assert_eq!(otra_vez.escena.elementos[0].grupos, vec!["g1"]);
}

#[test]
fn agrupar_en_windows_se_ve_en_el_movil() {
    // Lo que hace util esta tarea: no solo conservar los grupos del
    // telefono, sino que los que se hagan aqui vuelvan alla.
    let json = r#"{"type":"excalidraw","elements":[
        {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
         "strokeColor":"#000000","seed":1}
    ]}"#;
    let mut lienzo = cargar(json).unwrap();
    lienzo.escena.elementos[0].grupos = vec!["nuevo".to_string()];

    let vuelta = guardar(&lienzo);
    assert!(
        vuelta.contains("\"groupIds\""),
        "el JSON tiene que llevar groupIds: {vuelta}"
    );
    assert_eq!(cargar(&vuelta).unwrap().escena.elementos[0].grupos, vec!["nuevo"]);
}

#[test]
fn desagrupar_en_windows_no_deja_los_grupos_viejos() {
    // Si groupIds solo se escribiera cuando hay grupos, desagrupar aqui
    // dejaria intactos los del JSON original y el movil los seguiria
    // viendo agrupados. Es el motivo de escribirlo siempre.
    let json = r#"{"type":"excalidraw","elements":[
        {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
         "strokeColor":"#000000","seed":1,"groupIds":["viejo"]}
    ]}"#;
    let mut lienzo = cargar(json).unwrap();
    lienzo.escena.elementos[0].grupos.clear();

    let otra_vez = cargar(&guardar(&lienzo)).unwrap();
    assert!(
        otra_vez.escena.elementos[0].grupos.is_empty(),
        "desagrupado aqui, desagrupado alla"
    );
}
```

- [ ] **Paso 2: Comprueba que fallan**

```
cargo test -p pixpin-motor2d excalidraw -- --test-threads=1
```

Esperado: `no field 'grupos' on type 'Elemento'`.

- [ ] **Paso 3: Añade el campo**

En `elemento.rs`, dentro de `struct Elemento`, junto a los demás
`#[serde(default)]`:

```rust
    /// Los grupos a los que pertenece, con los identificadores del movil
    /// (`groupIds` de Excalidraw).
    ///
    /// Cadenas y no numeros porque el movil las genera como cadenas y esto
    /// viaja de ida y vuelta sin tocarlas. Inventar aqui un `u64`
    /// obligaria a mantener una tabla de traduccion, que es una segunda
    /// verdad sobre lo mismo.
    #[serde(default)]
    pub grupos: Vec<String>,
```

Y en `bytes()`, que la tarea 1 dejó preparado:

```rust
        let grupos: usize = self.grupos.iter().map(String::len).sum();
        size_of::<Elemento>() + dentro + grupos
```

- [ ] **Paso 4: Léelo y escríbelo en el puente**

En `elemento_desde` (línea 183), donde se construye el `Elemento`:

```rust
        grupos: v
            .get("groupIds")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|g| g.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
```

En `elemento_hacia` (línea 269), junto a los demás `mapa.insert`:

```rust
    // Se escribe siempre, tambien vacio: si solo se escribiera cuando hay
    // grupos, desagrupar en Windows dejaria los groupIds viejos del
    // original y el movil los volveria a ver agrupados.
    mapa.insert(
        "groupIds".into(),
        Value::Array(e.grupos.iter().map(|g| Value::String(g.clone())).collect()),
    );
```

- [ ] **Paso 5: Arregla los constructores literales**

El campo es obligatorio en las construcciones literales de `Elemento`.
Encuéntralas:

```
cargo build --workspace --all-targets 2>&1 | grep -A3 "missing field" | head -40
```

En cada una añade `grupos: Vec::new(),`. Están sobre todo en los `mod pruebas`
y en `formas.rs`.

- [ ] **Paso 6: Comprueba que pasan**

```
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Vigila especialmente que **las pruebas de ida y vuelta del `.pixpin` sigan
verdes**: son las que garantizan que un elemento que Windows no entiende
sobrevive al viaje, y esta tarea toca justo ese camino.

- [ ] **Paso 7: Commit**

```bash
git add crates/pixpin-motor2d/src/elemento.rs crates/pixpin-motor2d/src/excalidraw.rs crates/pixpin-motor2d/src/formas.rs
git commit -m "Los grupos del movil dejan de ser invisibles en Windows

El puente ya conservaba los groupIds dentro del JSON original, porque
elemento_hacia escribe encima del Value de partida en vez de construirlo
de cero. Pero Windows no podia leerlos: un plano hecho en el telefono se
abria aqui como un monton de piezas sueltas.

Elemento gana grupos: Vec<String>. Cadenas y no numeros porque el movil
las genera asi y esto viaja de ida y vuelta sin tocarlas; inventar un u64
obligaria a una tabla de traduccion, que es una segunda verdad sobre lo
mismo.

groupIds se escribe siempre, tambien vacio. Si solo se escribiera cuando
hay grupos, desagrupar en Windows dejaria los del original intactos y el
movil los seguiria viendo agrupados. Hay una prueba para eso."
```

---

## Tarea 4: Transformar — escalar y girar con ancla

La tarea más delicada del plan. En el motor no existe **ni una línea** de
redimensionado de elementos: lo único parecido es `escalar_anclado` de
`pixpin-pin`, que redimensiona la *ventana* del pin. Esto se escribe de cero.

**Ficheros:**
- Crear: `crates/pixpin-motor2d/src/transformar.rs`
- Modificar: `crates/pixpin-motor2d/src/lib.rs`
- Modificar: `crates/pixpin-motor2d/Cargo.toml` — ya depende de `pixpin-geom`;
  comprueba que sí antes de nada
- Prueba: en `transformar.rs`, `mod pruebas`

**Interfaces:**
- Consume: `Elemento`, `Punto2`, `pixpin_geom::Tirador`.
- Produce:
  ```rust
  pub const MINIMO: f32 = 1.0;
  pub const SALTO_GIRO: f32 = std::f32::consts::FRAC_PI_2 / 6.0; // 15 grados

  pub fn escalar(
      e: &mut Elemento,
      tirador: Tirador,
      p: Punto2,
      proporcional: bool,
      desde_centro: bool,
  );
  pub fn girar(e: &mut Elemento, centro: Punto2, delta: f32);
  pub fn angulo_hacia(centro: Punto2, p: Punto2) -> f32;
  pub fn a_saltos(angulo: f32) -> f32;
  ```

### La idea, antes del código

Todo sale de una sola fórmula. El elemento se dibuja **girado `ang` alrededor
de su centro**, así que un punto del mundo `q` corresponde al punto local
`q.girar(centro, -ang)`. Escalar es multiplicar el desplazamiento respecto al
ancla, y devolverlo al mundo:

```
nuevo(q) = ancla_en_el_mundo + R(ang) · ( (q_local − ancla_local) · (sx, sy) )
```

Comprueba tú mismo que el ancla no se mueve: si `q` es el ancla, `q_local`
es `ancla_local`, el desplazamiento es cero, y el resultado es
`ancla_en_el_mundo`. **Por construcción, no por cuidado.**

La forma evidente —cambiar `ancho` y `alto` y volver a girar— falla porque el
centro se desplaza y el elemento acaba girado alrededor de un centro nuevo. El
usuario ve que la figura «se escapa» al redimensionarla.

- [ ] **Paso 1: Escribe las pruebas que fallan**

Crea `crates/pixpin-motor2d/src/transformar.rs` con solo el `mod pruebas`:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};
    use std::f32::consts::{FRAC_PI_2, PI};

    /// Un rectangulo de 100x50 con la esquina en el origen.
    fn rect() -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Rectangulo,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 50.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
        }
    }

    /// La esquina superior izquierda, ya en el mundo (girada).
    fn esquina_no(e: &Elemento) -> Punto2 {
        let (x0, y0, x1, y1) = e.caja();
        let c = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        Punto2::nuevo(x0, y0).girar(c, e.angulo)
    }

    fn cerca(a: Punto2, b: Punto2, que: &str) {
        assert!(
            (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3,
            "{que}: esperaba ({}, {}), es ({}, {})",
            b.x,
            b.y,
            a.x,
            a.y
        );
    }

    #[test]
    fn escalar_por_una_esquina_deja_quieta_la_de_enfrente() {
        let mut e = rect();
        let ancla = esquina_no(&e);

        escalar(&mut e, Tirador::SuresteEsquina, Punto2::nuevo(200.0, 100.0), false, false);

        assert_eq!(e.ancho, 200.0);
        assert_eq!(e.alto, 100.0);
        cerca(esquina_no(&e), ancla, "la esquina anclada");
    }

    #[test]
    fn con_el_elemento_girado_la_esquina_anclada_sigue_sin_moverse() {
        // ESTA es la prueba que justifica la tarea entera. La forma
        // evidente —cambiar ancho/alto y volver a girar— la falla, porque
        // el centro se desplaza y el elemento acaba girado alrededor de un
        // centro nuevo.
        for angulo in [0.0, 0.5236, FRAC_PI_2, PI, 2.6] {
            let mut e = rect();
            e.angulo = angulo;
            let ancla = esquina_no(&e);

            escalar(&mut e, Tirador::SuresteEsquina, Punto2::nuevo(180.0, 90.0), false, false);

            cerca(esquina_no(&e), ancla, &format!("a {angulo} radianes"));
        }
    }

    #[test]
    fn escalar_y_devolver_deja_el_elemento_donde_estaba() {
        for angulo in [0.0, 0.5236, FRAC_PI_2, PI] {
            let mut e = rect();
            e.angulo = angulo;
            let (x0, y0, x1, y1) = e.caja();
            let c = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            let se_original = Punto2::nuevo(x1, y1).girar(c, angulo);

            escalar(&mut e, Tirador::SuresteEsquina, Punto2::nuevo(300.0, 300.0), false, false);
            escalar(&mut e, Tirador::SuresteEsquina, se_original, false, false);

            assert!((e.ancho - 100.0).abs() < 1e-2, "ancho a {angulo}: {}", e.ancho);
            assert!((e.alto - 50.0).abs() < 1e-2, "alto a {angulo}: {}", e.alto);
        }
    }

    #[test]
    fn un_tirador_de_lado_solo_mueve_su_eje() {
        let mut e = rect();
        escalar(&mut e, Tirador::EsteBorde, Punto2::nuevo(300.0, 999.0), false, false);
        assert_eq!(e.ancho, 300.0);
        assert_eq!(e.alto, 50.0, "el lado este no toca el alto");
    }

    #[test]
    fn escalar_un_trazo_mueve_sus_puntos() {
        // Elemento::caja() calcula la caja DE LOS PUNTOS para las figuras
        // con puntos. Si solo se cambiara ancho/alto, el trazo no escalaria
        // y el marco de seleccion se despegaria del dibujo.
        let mut e = Elemento {
            figura: Figura::Lapiz {
                puntos: vec![
                    Punto2::nuevo(0.0, 0.0),
                    Punto2::nuevo(50.0, 25.0),
                    Punto2::nuevo(100.0, 50.0),
                ],
                presiones: Vec::new(),
            },
            grosor: 0.0, // sin margen, para que la caja sean los puntos
            ..rect()
        };

        escalar(&mut e, Tirador::SuresteEsquina, Punto2::nuevo(200.0, 100.0), false, false);

        let Figura::Lapiz { puntos, .. } = &e.figura else {
            panic!("sigue siendo un lapiz");
        };
        cerca(puntos[0], Punto2::nuevo(0.0, 0.0), "el primero es el ancla");
        cerca(puntos[2], Punto2::nuevo(200.0, 100.0), "el ultimo va al cursor");
        cerca(puntos[1], Punto2::nuevo(100.0, 50.0), "el de en medio, a escala");
    }

    #[test]
    fn con_shift_se_conserva_la_proporcion() {
        let mut e = rect();
        escalar(&mut e, Tirador::SuresteEsquina, Punto2::nuevo(200.0, 999.0), true, false);
        assert!(
            (e.ancho / e.alto - 2.0).abs() < 1e-3,
            "100x50 es 2:1, y es {}x{}",
            e.ancho,
            e.alto
        );
    }

    #[test]
    fn con_alt_se_escala_desde_el_centro() {
        let mut e = rect();
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);

        escalar(&mut e, Tirador::SuresteEsquina, Punto2::nuevo(150.0, 75.0), false, true);

        let (x0, y0, x1, y1) = e.caja();
        cerca(
            Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0),
            centro,
            "el centro no se mueve",
        );
    }

    #[test]
    fn cruzar_el_ancla_voltea_en_vez_de_aplastar() {
        // D30: aplastar a cero pierde informacion sin remedio; voltear es
        // reversible y es lo que espera quien cruzo el raton al otro lado.
        let mut e = rect();
        escalar(&mut e, Tirador::SuresteEsquina, Punto2::nuevo(-100.0, 50.0), false, false);

        assert!(e.ancho >= MINIMO, "no se aplasta: {}", e.ancho);
        let (x0, _, x1, _) = e.caja();
        assert!(x0 < 0.0 && x1 <= 0.0 + 1e-3, "quedo al otro lado del ancla");
    }

    #[test]
    fn girar_alrededor_de_su_centro_suma_el_angulo_y_no_lo_mueve() {
        let mut e = rect();
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);

        girar(&mut e, centro, FRAC_PI_2);

        assert!((e.angulo - FRAC_PI_2).abs() < 1e-6);
        let (x0, y0, x1, y1) = e.caja();
        cerca(
            Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0),
            centro,
            "girar sobre si mismo no lo mueve",
        );
    }

    #[test]
    fn girar_alrededor_de_un_centro_ajeno_lo_mueve_en_orbita() {
        // Es lo que pasa al girar una seleccion de varios: cada uno gira
        // sobre si mismo Y orbita el centro comun.
        let mut e = rect();
        let ajeno = Punto2::nuevo(0.0, 0.0);
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);

        girar(&mut e, ajeno, FRAC_PI_2);

        let (x0, y0, x1, y1) = e.caja();
        let nuevo = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        cerca(nuevo, centro.girar(ajeno, FRAC_PI_2), "orbito el centro comun");
    }

    #[test]
    fn girar_un_trazo_gira_sus_puntos() {
        let mut e = Elemento {
            figura: Figura::Lapiz {
                puntos: vec![Punto2::nuevo(10.0, 0.0), Punto2::nuevo(20.0, 0.0)],
                presiones: Vec::new(),
            },
            grosor: 0.0,
            ..rect()
        };
        girar(&mut e, Punto2::nuevo(0.0, 0.0), FRAC_PI_2);

        let Figura::Lapiz { puntos, .. } = &e.figura else { panic!() };
        cerca(puntos[0], Punto2::nuevo(0.0, 10.0), "el primero");
        cerca(puntos[1], Punto2::nuevo(0.0, 20.0), "el segundo");
    }

    #[test]
    fn los_saltos_de_giro_son_de_quince_grados() {
        assert!((a_saltos(0.20) - 0.0).abs() < 1e-6, "0,20 rad baja a 0");
        assert!((a_saltos(0.30) - SALTO_GIRO).abs() < 1e-6, "0,30 rad sube a 15");
        assert!((a_saltos(-0.30) + SALTO_GIRO).abs() < 1e-6, "y en negativo");
    }

    #[test]
    fn el_angulo_hacia_arriba_es_cero() {
        // El tirador de giro esta encima del elemento; con el cursor ahi
        // mismo, el giro tiene que ser cero y no un cuarto de vuelta.
        let c = Punto2::nuevo(0.0, 0.0);
        assert!(angulo_hacia(c, Punto2::nuevo(0.0, -10.0)).abs() < 1e-6);
        let derecha = angulo_hacia(c, Punto2::nuevo(10.0, 0.0));
        assert!((derecha - FRAC_PI_2).abs() < 1e-6, "a la derecha, +90: {derecha}");
    }

    #[test]
    fn transformar_sube_la_version_para_invalidar_la_cache() {
        let mut e = rect();
        let antes = e.version;
        escalar(&mut e, Tirador::SuresteEsquina, Punto2::nuevo(200.0, 100.0), false, false);
        assert_ne!(e.version, antes, "sin esto la cache pintaria lo viejo");
    }
}
```

- [ ] **Paso 2: Comprueba que fallan**

Declara `pub mod transformar;` en `lib.rs` y corre:

```
cargo test -p pixpin-motor2d transformar -- --test-threads=1
```

Esperado: `cannot find function 'escalar' in this scope`.

- [ ] **Paso 3: Escribe la implementación**

Encima del `mod pruebas`:

```rust
//! Mover, escalar y girar. Puerto de `Transform.kt` del Android.
//!
//! # El problema del ancla
//!
//! Redimensionar tirando de una esquina significa que **la de enfrente no
//! se mueve**. Sin giro es aritmetica de colegio. Con el elemento girado,
//! la forma evidente esta mal:
//!
//! > Cambio `ancho` y `alto` -> el centro se desplaza -> el elemento se
//! > dibuja girado **alrededor de un centro nuevo** -> la esquina anclada
//! > se va.
//!
//! El usuario lo ve como que la figura «se escapa» al redimensionarla.
//!
//! # La formula
//!
//! Un elemento se dibuja girado `ang` alrededor de su centro, asi que un
//! punto del mundo `q` es el punto local `q.girar(centro, -ang)`. Escalar
//! es multiplicar el desplazamiento respecto al ancla y devolverlo:
//!
//! ```text
//! nuevo(q) = ancla_mundo + R(ang) · ( (q_local − ancla_local) · (sx, sy) )
//! ```
//!
//! Si `q` es el ancla, el desplazamiento es cero y el resultado es el ancla:
//! **queda quieta por construccion, no por cuidado**.
//!
//! # La regla de los puntos
//!
//! `Elemento::caja()` calcula la caja **de los puntos** para lapiz,
//! resaltador, linea y flecha. Por eso: toda transformacion que toque una
//! figura con puntos, toca los puntos. Si solo se cambiara `ancho`, el
//! trazo no escalaria y el marco se despegaria del dibujo. Es el mismo
//! motivo por el que `Elemento::mover` ya mueve los puntos.

use pixpin_geom::Tirador;

use crate::elemento::{Elemento, Figura};
use crate::vector::Punto2;

/// Por debajo de esto no se aplasta: se voltea (D30).
pub const MINIMO: f32 = 1.0;

/// Quince grados, el salto de giro con `Shift`.
pub const SALTO_GIRO: f32 = std::f32::consts::FRAC_PI_2 / 6.0;

/// El centro de la caja de un elemento.
fn centro_de(e: &Elemento) -> Punto2 {
    let (x0, y0, x1, y1) = e.caja();
    Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0)
}

/// Que ejes mueve cada tirador, y donde queda su ancla dentro de la caja
/// en tantos por uno: `(0,0)` es la esquina noroeste, `(1,1)` la sureste.
fn ancla_y_ejes(t: Tirador) -> (f32, f32, bool, bool) {
    match t {
        Tirador::NoroesteEsquina => (1.0, 1.0, true, true),
        Tirador::NorteBorde => (0.5, 1.0, false, true),
        Tirador::NoresteEsquina => (0.0, 1.0, true, true),
        Tirador::EsteBorde => (0.0, 0.5, true, false),
        Tirador::SuresteEsquina => (0.0, 0.0, true, true),
        Tirador::SurBorde => (0.5, 0.0, false, true),
        Tirador::SuroesteEsquina => (1.0, 0.0, true, true),
        Tirador::OesteBorde => (1.0, 0.5, true, false),
    }
}

/// Escala `e` arrastrando `tirador` hasta el punto `p` del mundo.
///
/// - `proporcional` (`Shift`): conserva la razon entre ancho y alto.
/// - `desde_centro` (`Alt`): el ancla pasa a ser el centro.
pub fn escalar(
    e: &mut Elemento,
    tirador: Tirador,
    p: Punto2,
    proporcional: bool,
    desde_centro: bool,
) {
    let (x0, y0, x1, y1) = e.caja();
    let (ancho, alto) = (x1 - x0, y1 - y0);
    // Una caja degenerada no se puede escalar: dividir por cero daria NaN,
    // y un NaN en la geometria borra el elemento de la pantalla sin error.
    if ancho.abs() < f32::EPSILON || alto.abs() < f32::EPSILON {
        return;
    }
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let ang = e.angulo;

    let (ax, ay, mueve_x, mueve_y) = ancla_y_ejes(tirador);
    let ancla_local = if desde_centro {
        centro
    } else {
        Punto2::nuevo(x0 + ancho * ax, y0 + alto * ay)
    };

    // El cursor, en el marco propio del elemento.
    let p_local = p.girar(centro, -ang);

    // Cuanto se estira cada eje. El tirador que no mueve un eje lo deja a 1.
    //
    // Dos sutilezas. Con `desde_centro`, la referencia es media caja y no la
    // caja entera: el cursor se aleja del centro, no del borde de enfrente.
    // Y el `signo` existe porque el ancla puede estar a la derecha o abajo
    // (tiradores del oeste y del norte): ahi, alejarse del ancla es ir hacia
    // los negativos.
    let mut sx = if mueve_x {
        let ancla_a_borde = if desde_centro { ancho / 2.0 } else { ancho };
        let signo = if ax > 0.5 { -1.0 } else { 1.0 };
        (p_local.x - ancla_local.x) * signo / ancla_a_borde
    } else {
        1.0
    };
    let mut sy = if mueve_y {
        let ancla_a_borde = if desde_centro { alto / 2.0 } else { alto };
        let signo = if ay > 0.5 { -1.0 } else { 1.0 };
        (p_local.y - ancla_local.y) * signo / ancla_a_borde
    } else {
        1.0
    };

    if proporcional && mueve_x && mueve_y {
        // El que mas se ha movido manda, para que la figura siga al cursor
        // por el eje en que el usuario esta tirando de verdad.
        let k = if sx.abs() > sy.abs() { sx.abs() } else { sy.abs() };
        sx = k * sx.signum();
        sy = k * sy.signum();
    }

    // No se aplasta: se voltea (D30). El minimo se aplica al tamano final,
    // conservando el signo, que es lo que produce el volteo.
    let tope = |s: f32, largo: f32| -> f32 {
        if (s * largo).abs() < MINIMO {
            MINIMO / largo * if s < 0.0 { -1.0 } else { 1.0 }
        } else {
            s
        }
    };
    let sx = tope(sx, ancho);
    let sy = tope(sy, alto);

    let ancla_mundo = ancla_local.girar(centro, ang);
    let origen = Punto2::nuevo(0.0, 0.0);

    // La formula del encabezado, en una sola funcion.
    let mapear = |q: Punto2| -> Punto2 {
        let ql = q.girar(centro, -ang);
        let d = Punto2::nuevo((ql.x - ancla_local.x) * sx, (ql.y - ancla_local.y) * sy);
        ancla_mundo.sumar(d.girar(origen, ang))
    };

    // Las figuras con puntos escalan sus puntos: su caja sale de ellos.
    match &mut e.figura {
        Figura::Lapiz { puntos, .. }
        | Figura::Resaltador { puntos }
        | Figura::Linea { puntos }
        | Figura::Flecha { puntos, .. } => {
            for q in puntos.iter_mut() {
                *q = mapear(*q);
            }
        }
        _ => {}
    }

    // Y todas, incluidas esas, actualizan su caja: `x`/`y` se usan para las
    // figuras sin puntos, y para las que los tienen es informacion
    // coherente que no debe quedarse vieja.
    let centro_nuevo = mapear(centro);
    e.ancho = (ancho * sx).abs();
    e.alto = (alto * sy).abs();
    e.x = centro_nuevo.x - e.ancho / 2.0;
    e.y = centro_nuevo.y - e.alto / 2.0;
    e.tocar();
}

/// Gira `e` en `delta` radianes alrededor de `centro`.
///
/// Si `centro` es el suyo, gira sobre si mismo. Si es ajeno —el centro de
/// una seleccion de varios— ademas orbita: es lo que hace que girar cinco
/// elementos a la vez se vea como girar el conjunto.
pub fn girar(e: &mut Elemento, centro: Punto2, delta: f32) {
    let propio = centro_de(e);

    match &mut e.figura {
        Figura::Lapiz { puntos, .. }
        | Figura::Resaltador { puntos }
        | Figura::Linea { puntos }
        | Figura::Flecha { puntos, .. } => {
            for q in puntos.iter_mut() {
                *q = q.girar(centro, delta);
            }
        }
        _ => {}
    }

    let nuevo = propio.girar(centro, delta);
    e.x += nuevo.x - propio.x;
    e.y += nuevo.y - propio.y;
    e.angulo += delta;
    e.tocar();
}

/// El angulo del centro al punto, con **cero apuntando hacia arriba**.
///
/// Hacia arriba y no hacia la derecha porque el tirador de giro esta encima
/// del elemento: con el cursor ahi mismo, el giro tiene que ser cero.
pub fn angulo_hacia(centro: Punto2, p: Punto2) -> f32 {
    let d = p.restar(centro);
    d.x.atan2(-d.y)
}

/// Redondea a saltos de quince grados. Es lo que hace `Shift` al girar.
pub fn a_saltos(angulo: f32) -> f32 {
    (angulo / SALTO_GIRO).round() * SALTO_GIRO
}
```

- [ ] **Paso 4: Comprueba que pasan**

```
cargo test -p pixpin-motor2d transformar -- --test-threads=1
```

Si falla `con_el_elemento_girado...`, el fallo casi seguro está en `mapear`:
el desplazamiento se gira alrededor **del origen**, no del centro, porque ya
es un vector y no un punto.

Si falla `cruzar_el_ancla_voltea...`, revisa `tope`: tiene que conservar el
signo, que es justo lo que produce el volteo.

- [ ] **Paso 5: La puerta entera**

```
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

- [ ] **Paso 6: Commit**

```bash
git add crates/pixpin-motor2d/src/transformar.rs crates/pixpin-motor2d/src/lib.rs
git commit -m "Escalar y girar, con el ancla quieta

En el motor no habia ni una linea de redimensionado de elementos: lo
unico parecido era escalar_anclado de pixpin-pin, que redimensiona la
ventana del pin.

Lo dificil es el ancla. Redimensionar tirando de una esquina significa
que la de enfrente no se mueve, y con el elemento girado la forma
evidente falla: cambiar ancho y alto desplaza el centro, el elemento
acaba girado alrededor de un centro nuevo, y la esquina anclada se va.
El usuario lo ve como que la figura se escapa.

La formula es una sola: nuevo(q) = ancla_mundo + R(ang) · ((q_local −
ancla_local) · (sx, sy)). Si q es el ancla, el desplazamiento es cero y
sale el ancla. Queda quieta por construccion, no por cuidado. La prueba
lo comprueba a 0, 30, 90, 180 y 149 grados.

Y una regla que se escapa facil: Elemento::caja() calcula la caja DE LOS
PUNTOS para lapiz, resaltador, linea y flecha, asi que escalar un trazo
tiene que escalar sus 492 puntos. Si solo se cambiara ancho, el trazo no
escalaria y el marco se despegaria del dibujo.

Por debajo del minimo se voltea en vez de aplastar (D30): aplastar a cero
pierde informacion sin remedio, voltear es reversible y es lo que espera
quien cruzo el raton al otro lado."
```

---

## Tarea 5: Dónde caen los tiradores

Ocho de tamaño más uno de giro. **Viven en el mundo pero miden en pantalla**:
un tirador tiene 8 píxeles al 20 % y al 500 %.

**Ficheros:**
- Crear: `crates/pixpin-motor2d/src/tiradores.rs`
- Modificar: `crates/pixpin-motor2d/src/lib.rs`
- Prueba: en `tiradores.rs`, `mod pruebas`

**Interfaces:**
- Consume: `Elemento`, `Punto2`, `Orden`, `pixpin_geom::Tirador`,
  `transformar::MINIMO`.
- Produce:
  ```rust
  pub const LADO: f32 = 8.0;              // pixeles de pantalla
  pub const ZONA: f32 = 10.0;             // pixeles de pantalla
  pub const SEPARACION_GIRO: f32 = 24.0;  // pixeles de pantalla

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum Agarre { Tamano(Tirador), Giro }

  pub struct Tiradores {
      pub tamano: [(Tirador, Punto2); 8],
      pub giro: Punto2,
      pub centro: Punto2,
      pub angulo: f32,
  }

  impl Tiradores {
      pub fn de_caja(caja: (f32, f32, f32, f32), angulo: f32, escala: f32) -> Self;
      pub fn de_elemento(e: &Elemento, escala: f32) -> Self;
      pub fn en(&self, p: Punto2, escala: f32) -> Option<Agarre>;
      pub fn ordenes(&self, escala: f32) -> Vec<Orden>;
  }
  ```

Recuerda el convenio: **`escala` son unidades de mundo por píxel de
pantalla**, o sea `1.0 / zoom`, igual que en `pintado::marco_de_seleccion`.

- [ ] **Paso 1: Escribe las pruebas que fallan**

Crea `crates/pixpin-motor2d/src/tiradores.rs` con solo el `mod pruebas`:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    /// Una caja de 100x50 en el origen.
    const CAJA: (f32, f32, f32, f32) = (0.0, 0.0, 100.0, 50.0);

    fn busca(t: &Tiradores, cual: Tirador) -> Punto2 {
        t.tamano.iter().find(|(c, _)| *c == cual).unwrap().1
    }

    fn cerca(a: Punto2, b: Punto2, que: &str) {
        assert!(
            (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3,
            "{que}: esperaba ({}, {}), es ({}, {})",
            b.x, b.y, a.x, a.y
        );
    }

    #[test]
    fn los_ocho_caen_en_las_esquinas_y_en_los_medios() {
        let t = Tiradores::de_caja(CAJA, 0.0, 1.0);
        cerca(busca(&t, Tirador::NoroesteEsquina), Punto2::nuevo(0.0, 0.0), "NO");
        cerca(busca(&t, Tirador::NorteBorde), Punto2::nuevo(50.0, 0.0), "N");
        cerca(busca(&t, Tirador::NoresteEsquina), Punto2::nuevo(100.0, 0.0), "NE");
        cerca(busca(&t, Tirador::EsteBorde), Punto2::nuevo(100.0, 25.0), "E");
        cerca(busca(&t, Tirador::SuresteEsquina), Punto2::nuevo(100.0, 50.0), "SE");
        cerca(busca(&t, Tirador::SurBorde), Punto2::nuevo(50.0, 50.0), "S");
        cerca(busca(&t, Tirador::SuroesteEsquina), Punto2::nuevo(0.0, 50.0), "SO");
        cerca(busca(&t, Tirador::OesteBorde), Punto2::nuevo(0.0, 25.0), "O");
    }

    #[test]
    fn con_la_caja_girada_los_tiradores_giran_con_ella() {
        let t = Tiradores::de_caja(CAJA, FRAC_PI_2, 1.0);
        let centro = Punto2::nuevo(50.0, 25.0);
        // La esquina noroeste, girada un cuarto de vuelta sobre el centro.
        cerca(
            busca(&t, Tirador::NoroesteEsquina),
            Punto2::nuevo(0.0, 0.0).girar(centro, FRAC_PI_2),
            "NO girada",
        );
    }

    #[test]
    fn el_de_giro_queda_separado_por_encima_y_gira_tambien() {
        // Encima del borde norte, a SEPARACION_GIRO pixeles de pantalla.
        let t = Tiradores::de_caja(CAJA, 0.0, 1.0);
        cerca(t.giro, Punto2::nuevo(50.0, -SEPARACION_GIRO), "sin girar");

        let g = Tiradores::de_caja(CAJA, FRAC_PI_2, 1.0);
        let centro = Punto2::nuevo(50.0, 25.0);
        cerca(
            g.giro,
            Punto2::nuevo(50.0, -SEPARACION_GIRO).girar(centro, FRAC_PI_2),
            "girado",
        );
    }

    #[test]
    fn la_separacion_del_giro_se_mide_en_pixeles_de_pantalla() {
        // Al 20 % de aumento (escala 5.0) el tirador se separa cinco veces
        // mas en el mundo, para verse igual de separado en la pantalla.
        let t = Tiradores::de_caja(CAJA, 0.0, 5.0);
        cerca(t.giro, Punto2::nuevo(50.0, -SEPARACION_GIRO * 5.0), "al 20 %");
    }

    #[test]
    fn picar_un_tirador_lo_encuentra_por_su_zona_y_no_por_su_dibujo() {
        let t = Tiradores::de_caja(CAJA, 0.0, 1.0);

        // Justo encima: lo encuentra.
        assert_eq!(
            t.en(Punto2::nuevo(100.0, 50.0), 1.0),
            Some(Agarre::Tamano(Tirador::SuresteEsquina))
        );
        // A nueve pixeles: dentro de la zona de diez, aunque el dibujo mida
        // ocho. Nadie acierta un cuadradito de ocho pixeles al primer
        // intento.
        assert_eq!(
            t.en(Punto2::nuevo(100.0 + 9.0, 50.0), 1.0),
            Some(Agarre::Tamano(Tirador::SuresteEsquina))
        );
        // A veinte: fuera.
        assert_eq!(t.en(Punto2::nuevo(100.0 + 20.0, 50.0), 1.0), None);
    }

    #[test]
    fn la_zona_de_picado_tambien_se_mide_en_pixeles_de_pantalla() {
        // Al 20 % (escala 5.0), diez pixeles de pantalla son cincuenta del
        // mundo. Si no, al alejarse los tiradores serian inalcanzables.
        let t = Tiradores::de_caja(CAJA, 0.0, 5.0);
        assert_eq!(
            t.en(Punto2::nuevo(100.0 + 45.0, 50.0), 5.0),
            Some(Agarre::Tamano(Tirador::SuresteEsquina))
        );
    }

    #[test]
    fn el_de_giro_manda_sobre_el_de_tamano_si_se_solapan() {
        // Con una caja muy pequena, el de giro puede caer encima del borde
        // norte. Girar es el gesto mas dificil de acertar de los dos, asi
        // que gana.
        let minuscula = (0.0, 0.0, 2.0, 2.0);
        let t = Tiradores::de_caja(minuscula, 0.0, 1.0);
        assert_eq!(t.en(t.giro, 1.0), Some(Agarre::Giro));
    }

    #[test]
    fn picar_lejos_de_todo_no_encuentra_nada() {
        let t = Tiradores::de_caja(CAJA, 0.0, 1.0);
        assert_eq!(t.en(Punto2::nuevo(500.0, 500.0), 1.0), None);
    }

    #[test]
    fn cada_tirador_se_pinta_con_relleno_y_borde() {
        // Nueve tiradores por dos ordenes cada uno. El borde no es adorno:
        // un cuadrado blanco sin el es invisible sobre fondo claro.
        let t = Tiradores::de_caja(CAJA, 0.0, 1.0);
        assert_eq!(t.ordenes(1.0).len(), 18);
    }

    #[test]
    fn los_cuadraditos_giran_con_el_elemento() {
        // En una figura a 45 grados, los cuadraditos van a 45 grados y no
        // de canto. Cuesta una linea y es de lo que separa un editor que
        // se siente bien de uno que no.
        let t = Tiradores::de_caja(CAJA, FRAC_PI_2, 1.0);
        let Orden::Relleno { puntos, .. } = &t.ordenes(1.0)[0] else {
            panic!("la primera orden es el relleno del primer tirador");
        };
        let lado = puntos[0].distancia(puntos[1]);
        assert!((lado - LADO).abs() < 1e-3, "sigue siendo cuadrado: {lado}");
        // Girado un cuarto de vuelta, el lado que iba en x ahora va en y.
        assert!(
            (puntos[0].x - puntos[1].x).abs() < 1e-3,
            "el primer lado quedo vertical"
        );
    }
}
```

- [ ] **Paso 2: Comprueba que falla**

Declara `pub mod tiradores;` en `lib.rs` y corre:

```
cargo test -p pixpin-motor2d tiradores -- --test-threads=1
```

Esperado: `cannot find type 'Tiradores' in this scope`.

- [ ] **Paso 3: Escribe la implementación**

```rust
//! Donde caen los nueve puntos de agarre y cual esta bajo el cursor.
//!
//! # Viven en el mundo, miden en pantalla
//!
//! Un tirador tiene ocho pixeles siempre: al 500 % no se convierte en un
//! ladrillo y al 20 % no desaparece. En unidades del mundo eso es
//! `LADO * escala`, donde `escala` es —como en todo este motor— unidades de
//! mundo por pixel de pantalla, o sea `1.0 / zoom`.
//!
//! # Por que la zona de picado es mas grande que el dibujo
//!
//! Diez pixeles contra ocho. Nadie acierta un cuadradito de ocho pixeles al
//! primer intento, y fallar un tirador no es un fallo pequeno: el clic cae
//! en el elemento de debajo y lo que el usuario queria redimensionar acaba
//! movido.

use pixpin_geom::Tirador;

use crate::elemento::{ColorRgba, Elemento, EstiloTrazo};
use crate::pintado::Orden;
use crate::vector::Punto2;

/// Lado del cuadradito que se dibuja, en pixeles de pantalla.
pub const LADO: f32 = 8.0;

/// Radio de la zona que responde al cursor, en pixeles de pantalla.
pub const ZONA: f32 = 10.0;

/// Cuanto se separa el tirador de giro del borde de arriba, en pixeles.
pub const SEPARACION_GIRO: f32 = 24.0;

const RELLENO: ColorRgba = ColorRgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
const BORDE: ColorRgba = ColorRgba { r: 0.36, g: 0.42, b: 0.95, a: 1.0 };

/// De que se ha agarrado el cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agarre {
    Tamano(Tirador),
    Giro,
}

/// Los nueve puntos de agarre, ya en coordenadas del mundo.
#[derive(Debug, Clone, PartialEq)]
pub struct Tiradores {
    pub tamano: [(Tirador, Punto2); 8],
    pub giro: Punto2,
    pub centro: Punto2,
    pub angulo: f32,
}

impl Tiradores {
    /// Los tiradores de una caja paralela a los ejes, girada `angulo`
    /// alrededor de su centro.
    pub fn de_caja(caja: (f32, f32, f32, f32), angulo: f32, escala: f32) -> Self {
        let (x0, y0, x1, y1) = caja;
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (mx, my) = (centro.x, centro.y);

        // Sin girar primero, y luego se gira todo de una vez: es una
        // operacion por punto en vez de una formula distinta por esquina.
        let sitios = [
            (Tirador::NoroesteEsquina, Punto2::nuevo(x0, y0)),
            (Tirador::NorteBorde, Punto2::nuevo(mx, y0)),
            (Tirador::NoresteEsquina, Punto2::nuevo(x1, y0)),
            (Tirador::EsteBorde, Punto2::nuevo(x1, my)),
            (Tirador::SuresteEsquina, Punto2::nuevo(x1, y1)),
            (Tirador::SurBorde, Punto2::nuevo(mx, y1)),
            (Tirador::SuroesteEsquina, Punto2::nuevo(x0, y1)),
            (Tirador::OesteBorde, Punto2::nuevo(x0, my)),
        ];
        let tamano = sitios.map(|(c, p)| (c, p.girar(centro, angulo)));

        // El de giro, separado por encima del borde norte. La separacion va
        // en pixeles de pantalla, como todo lo demas de aqui.
        let giro = Punto2::nuevo(mx, y0 - SEPARACION_GIRO * escala).girar(centro, angulo);

        Self { tamano, giro, centro, angulo }
    }

    /// Los tiradores de un elemento.
    pub fn de_elemento(e: &Elemento, escala: f32) -> Self {
        Self::de_caja(e.caja(), e.angulo, escala)
    }

    /// De que hay agarre bajo el punto, si de alguno.
    ///
    /// El de giro se mira primero: con una caja muy pequena puede caer
    /// encima del borde norte, y girar es el gesto mas dificil de acertar
    /// de los dos.
    pub fn en(&self, p: Punto2, escala: f32) -> Option<Agarre> {
        let radio = ZONA * escala;
        if p.distancia(self.giro) <= radio {
            return Some(Agarre::Giro);
        }
        self.tamano
            .iter()
            .find(|(_, q)| p.distancia(*q) <= radio)
            .map(|(c, _)| Agarre::Tamano(*c))
    }

    /// Como se pintan: un cuadradito blanco con borde por cada uno.
    ///
    /// Girados con el elemento, para que en una figura a 45 grados los
    /// cuadraditos vayan a 45 grados y no de canto.
    pub fn ordenes(&self, escala: f32) -> Vec<Orden> {
        let mitad = LADO * escala / 2.0;
        let mut fuera = Vec::with_capacity(9);
        let sitios = self.tamano.iter().map(|(_, p)| p).chain([&self.giro]);
        for p in sitios {
            let esquinas = [
                Punto2::nuevo(p.x - mitad, p.y - mitad),
                Punto2::nuevo(p.x + mitad, p.y - mitad),
                Punto2::nuevo(p.x + mitad, p.y + mitad),
                Punto2::nuevo(p.x - mitad, p.y + mitad),
            ];
            let puntos: Vec<Punto2> = esquinas
                .iter()
                .map(|q| q.girar(*p, self.angulo))
                .collect();
            fuera.push(Orden::Relleno { puntos: puntos.clone(), color: RELLENO });
            let mut cerrado = puntos;
            cerrado.push(cerrado[0]);
            fuera.push(Orden::Polilinea {
                puntos: cerrado,
                color: BORDE,
                grosor: (1.0 * escala).max(0.5),
                estilo: EstiloTrazo::Solido,
            });
        }
        fuera
    }
}
```

- [ ] **Paso 4: Comprueba que pasan y cierra**

```
cargo test -p pixpin-motor2d tiradores -- --test-threads=1
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

- [ ] **Paso 5: Commit**

```bash
git add crates/pixpin-motor2d/src/tiradores.rs crates/pixpin-motor2d/src/lib.rs
git commit -m "Los nueve tiradores: donde caen y cual esta bajo el cursor

Ocho de tamano y uno de giro. Viven en el mundo pero miden en pantalla:
un tirador tiene ocho pixeles al 20 % y al 500 %, que en unidades del
mundo es LADO * escala. Si midieran en el mundo, al alejarse serian
invisibles y al acercarse, ladrillos.

La zona que responde son diez pixeles y el dibujo ocho, a proposito.
Nadie acierta un cuadradito de ocho pixeles al primer intento, y fallar
un tirador no es un fallo pequeno: el clic cae en el elemento de debajo y
lo que se queria redimensionar acaba movido.

El de giro se mira antes que los de tamano. Con una caja muy pequena cae
encima del borde norte, y de los dos gestos girar es el mas dificil de
acertar."
```

---

## Tarea 6: Picado múltiple y marquesina

`impacto.rs` sabe decir qué elemento hay bajo un punto. Le faltan dos cosas:
**todos** los que hay bajo un punto, y todos los que caen dentro de una caja.

**Ficheros:**
- Modificar: `crates/pixpin-motor2d/src/impacto.rs`
- Modificar: `crates/pixpin-motor2d/src/lib.rs` — reexportar lo nuevo
- Prueba: en `impacto.rs`, `mod pruebas`

**Interfaces:**
- Consume: `Elemento`, `Punto2`, `toca()` y `elemento_en()` que ya existen.
- Produce:
  ```rust
  pub fn elementos_en(elementos: &[Elemento], p: Punto2) -> Vec<u64>;
  pub fn dentro_de(elementos: &[Elemento], caja: (f32, f32, f32, f32)) -> Vec<u64>;
  pub fn esquinas_giradas(e: &Elemento) -> [Punto2; 4];
  ```

- [ ] **Paso 1: Escribe las pruebas que fallan**

En `mod pruebas` de `impacto.rs` (reutiliza el `fn base()` o el constructor
que ya tenga el fichero):

```rust
#[test]
fn la_marquesina_coge_lo_que_esta_entero_dentro_y_no_lo_que_toca() {
    // D29. Con trazos largos, «lo que toque» selecciona cosas que el
    // usuario no ve venir: un trazo de dos metros que cruza la pantalla
    // entraria en cualquier marquesina que roce su camino.
    let dentro = Elemento { x: 10.0, y: 10.0, ancho: 20.0, alto: 20.0, ..base() };
    let a_medias = Elemento { x: 90.0, y: 10.0, ancho: 40.0, alto: 20.0, ..base() };
    let fuera = Elemento { x: 500.0, y: 500.0, ancho: 10.0, alto: 10.0, ..base() };
    let mut lista = vec![dentro, a_medias, fuera];
    for (i, e) in lista.iter_mut().enumerate() {
        e.id = i as u64 + 1;
    }

    let cogidos = dentro_de(&lista, (0.0, 0.0, 100.0, 100.0));
    assert_eq!(cogidos, vec![1], "solo el que cabe entero");
}

#[test]
fn un_elemento_girado_cuenta_por_sus_esquinas_giradas() {
    // Un cuadrado de 100 girado 45 grados mide 141 en diagonal: cabe en su
    // caja sin girar pero NO en una marquesina justa.
    use std::f32::consts::FRAC_PI_4;
    let e = Elemento {
        id: 1,
        x: 0.0,
        y: 0.0,
        ancho: 100.0,
        alto: 100.0,
        angulo: FRAC_PI_4,
        ..base()
    };
    let lista = vec![e];

    assert!(
        dentro_de(&lista, (0.0, 0.0, 100.0, 100.0)).is_empty(),
        "girado, se sale de su propia caja"
    );
    assert_eq!(
        dentro_de(&lista, (-30.0, -30.0, 130.0, 130.0)),
        vec![1],
        "con sitio de sobra, si"
    );
}

#[test]
fn la_marquesina_no_coge_los_borrados() {
    let mut a = Elemento { id: 1, x: 0.0, y: 0.0, ancho: 10.0, alto: 10.0, ..base() };
    a.borrado = true;
    let lista = vec![a];
    assert!(dentro_de(&lista, (-100.0, -100.0, 100.0, 100.0)).is_empty());
}

#[test]
fn elementos_en_los_devuelve_de_arriba_abajo() {
    // El orden de la lista ES el orden de pintado: el ultimo se pinta
    // encima. Al picar, el de encima va primero, que es lo que el usuario
    // cree que esta tocando.
    let a = Elemento { id: 1, x: 0.0, y: 0.0, ancho: 100.0, alto: 100.0, relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)), ..base() };
    let b = Elemento { id: 2, ..a.clone() };
    let lista = vec![a, b];

    assert_eq!(elementos_en(&lista, Punto2::nuevo(50.0, 50.0)), vec![2, 1]);
}

#[test]
fn elementos_en_y_elemento_en_estan_de_acuerdo() {
    let a = Elemento { id: 1, x: 0.0, y: 0.0, ancho: 100.0, alto: 100.0, relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)), ..base() };
    let b = Elemento { id: 2, x: 20.0, y: 20.0, ..a.clone() };
    let lista = vec![a, b];
    let p = Punto2::nuevo(50.0, 50.0);

    assert_eq!(
        elemento_en(&lista, p),
        elementos_en(&lista, p).first().copied(),
        "el primero de la lista es el que devuelve elemento_en"
    );
}

#[test]
fn las_esquinas_giradas_de_un_elemento_sin_giro_son_su_caja() {
    let e = Elemento { x: 10.0, y: 20.0, ancho: 30.0, alto: 40.0, angulo: 0.0, ..base() };
    let c = esquinas_giradas(&e);
    let (x0, y0, x1, y1) = e.caja();
    assert_eq!(c[0], Punto2::nuevo(x0, y0));
    assert_eq!(c[2], Punto2::nuevo(x1, y1));
}
```

- [ ] **Paso 2: Comprueba que fallan**

```
cargo test -p pixpin-motor2d impacto -- --test-threads=1
```

Esperado: `cannot find function 'dentro_de' in this scope`.

- [ ] **Paso 3: Escribe la implementación**

Al final de `impacto.rs`, antes del `mod pruebas`:

```rust
/// Las cuatro esquinas de la caja del elemento, ya giradas.
///
/// En orden: noroeste, noreste, sureste, suroeste. Es lo que hace falta
/// para saber si cabe dentro de algo: la caja sin girar de un cuadrado de
/// 100 girado 45 grados mide 141 en diagonal, y usarla daria por dentro
/// cosas que se salen.
pub fn esquinas_giradas(e: &Elemento) -> [Punto2; 4] {
    let (x0, y0, x1, y1) = e.caja();
    let c = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    [
        Punto2::nuevo(x0, y0),
        Punto2::nuevo(x1, y0),
        Punto2::nuevo(x1, y1),
        Punto2::nuevo(x0, y1),
    ]
    .map(|p| if e.angulo == 0.0 { p } else { p.girar(c, e.angulo) })
}

/// Todos los que tocan el punto, **de arriba abajo**.
///
/// El orden de la lista es el de pintado —el ultimo se pinta encima—, asi
/// que se recorre al reves: el de encima va primero, que es el que el
/// usuario cree que esta tocando.
pub fn elementos_en(elementos: &[Elemento], p: Punto2) -> Vec<u64> {
    elementos
        .iter()
        .rev()
        .filter(|e| toca(e, p))
        .map(|e| e.id)
        .collect()
}

/// Los que caben **enteros** dentro de la caja. Es la marquesina (D29).
///
/// Enteros y no «los que toquen» a proposito: con trazos largos, tocar
/// selecciona cosas que el usuario no ve venir. Un trazo que cruza la
/// pantalla entraria en cualquier marquesina que roce su camino, y el
/// usuario acabaria moviendo medio dibujo sin saber por que.
pub fn dentro_de(elementos: &[Elemento], caja: (f32, f32, f32, f32)) -> Vec<u64> {
    let (mx0, my0, mx1, my1) = caja;
    let (mx0, mx1) = (mx0.min(mx1), mx0.max(mx1));
    let (my0, my1) = (my0.min(my1), my0.max(my1));
    elementos
        .iter()
        .filter(|e| !e.borrado)
        .filter(|e| {
            esquinas_giradas(e)
                .iter()
                .all(|q| q.x >= mx0 && q.x <= mx1 && q.y >= my0 && q.y <= my1)
        })
        .map(|e| e.id)
        .collect()
}
```

Normalizar la caja al principio no es adorno: la marquesina se arrastra en
cualquier dirección, y de derecha a izquierda llega con `x1 < x0`.

- [ ] **Paso 4: Reexporta y cierra**

En `lib.rs`, junto a lo que ya se reexporta de `impacto`:

```rust
pub use impacto::{TOLERANCIA, dentro_de, elemento_en, elementos_en, esquinas_giradas, toca};
```

```
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

- [ ] **Paso 5: Commit**

```bash
git add crates/pixpin-motor2d/src/impacto.rs crates/pixpin-motor2d/src/lib.rs
git commit -m "Picar varios y la marquesina

impacto.rs sabia decir que elemento hay bajo un punto. Le faltaban los
que hay bajo un punto (para poder ciclar entre lo apilado) y los que
caben dentro de una caja (la marquesina).

La marquesina coge lo que esta ENTERO dentro y no lo que toca (D29). Con
trazos largos, tocar selecciona cosas que el usuario no ve venir: un
trazo que cruza la pantalla entraria en cualquier marquesina que roce su
camino, y acabaria moviendo medio dibujo sin saber por que.

Y cuenta por las esquinas giradas, no por la caja sin girar. Un cuadrado
de 100 girado 45 grados mide 141 en diagonal: con la caja sin girar,
daria por dentro cosas que se salen."
```

---

## Tarea 7: La rejilla espacial

`elemento_en` y `recortar` recorren la lista entera. Con ocho mil elementos,
mover el ratón por encima cuesta ocho mil pruebas de impacto por aviso.

**Ficheros:**
- Crear: `crates/pixpin-motor2d/src/indice.rs`
- Modificar: `crates/pixpin-motor2d/src/lib.rs`
- Prueba: en `indice.rs`, `mod pruebas`

**Interfaces:**
- Consume: `Escena`, `Elemento`, `Azar` (que ya está en el crate, para las
  escenas al azar de las pruebas).
- Produce:
  ```rust
  pub const CELDA: f32 = 256.0;

  pub struct Rejilla { /* privado */ }
  impl Rejilla {
      pub fn nueva() -> Self;
      pub fn con_celda(lado: f32) -> Self;
      pub fn sincronizar(&mut self, escena: &Escena);
      pub fn candidatos(&self, caja: (f32, f32, f32, f32)) -> Vec<u64>;
      pub fn cuantas_celdas(&self) -> usize;
  }
  ```

- [ ] **Paso 1: Escribe las pruebas que fallan**

Crea `crates/pixpin-motor2d/src/indice.rs` con solo el `mod pruebas`:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::azar::Azar;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};

    fn rect(id: u64, x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho,
            alto,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
        }
    }

    /// Lo mismo que hace la rejilla, pero recorriendo todo. Es la verdad
    /// contra la que se compara.
    fn a_lo_bruto(escena: &Escena, caja: (f32, f32, f32, f32)) -> Vec<u64> {
        let (bx0, by0, bx1, by1) = caja;
        let mut fuera: Vec<u64> = escena
            .elementos
            .iter()
            .filter(|e| !e.borrado)
            .filter(|e| {
                let (x0, y0, x1, y1) = e.caja();
                x1 >= bx0 && x0 <= bx1 && y1 >= by0 && y0 <= by1
            })
            .map(|e| e.id)
            .collect();
        fuera.sort_unstable();
        fuera
    }

    #[test]
    fn la_rejilla_dice_lo_mismo_que_la_fuerza_bruta() {
        // La prueba que justifica la rejilla entera. Escenas al azar pero
        // reproducibles: Azar es el Lehmer que ya usa el motor para que un
        // dibujo tenga el mismo garabato en cada apertura.
        let mut azar = Azar::nuevo(20260906);
        let mut escena = Escena::nueva();
        for i in 0..800 {
            let x = azar.siguiente() * 4000.0 - 2000.0;
            let y = azar.siguiente() * 4000.0 - 2000.0;
            let ancho = 1.0 + azar.siguiente() * 300.0;
            let alto = 1.0 + azar.siguiente() * 300.0;
            escena.anadir(rect(i + 1, x, y, ancho, alto));
        }

        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(&escena);

        for _ in 0..200 {
            let x = azar.siguiente() * 4000.0 - 2000.0;
            let y = azar.siguiente() * 4000.0 - 2000.0;
            let caja = (x, y, x + azar.siguiente() * 900.0, y + azar.siguiente() * 900.0);

            let mut de_la_rejilla = rejilla.candidatos(caja);
            de_la_rejilla.sort_unstable();
            // La rejilla puede devolver de mas —quien la usa filtra— pero
            // JAMAS de menos: un elemento que no salga aqui es un elemento
            // que desaparece de la pantalla.
            for id in a_lo_bruto(&escena, caja) {
                assert!(
                    de_la_rejilla.contains(&id),
                    "la rejilla se dejo el {id} en la caja {caja:?}"
                );
            }
        }
    }

    #[test]
    fn un_elemento_que_cambia_de_sitio_cambia_de_celda() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(1, 0.0, 0.0, 10.0, 10.0));
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(&escena);
        assert_eq!(rejilla.candidatos((0.0, 0.0, 20.0, 20.0)), vec![id]);

        escena.buscar_mut(id).unwrap().mover(5000.0, 5000.0);
        rejilla.sincronizar(&escena);

        assert!(
            rejilla.candidatos((0.0, 0.0, 20.0, 20.0)).is_empty(),
            "ya no esta donde estaba"
        );
        assert_eq!(rejilla.candidatos((4990.0, 4990.0, 5020.0, 5020.0)), vec![id]);
    }

    #[test]
    fn sincronizar_sin_cambios_no_hace_nada() {
        // Se llama en cada fotograma. Si reconstruyera la rejilla entera
        // cada vez, seria mas cara que la fuerza bruta que viene a evitar.
        let mut escena = Escena::nueva();
        for i in 0..100 {
            escena.anadir(rect(i + 1, i as f32 * 10.0, 0.0, 5.0, 5.0));
        }
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(&escena);
        let celdas = rejilla.cuantas_celdas();

        rejilla.sincronizar(&escena);
        assert_eq!(rejilla.cuantas_celdas(), celdas, "ni una celda de mas");
    }

    #[test]
    fn un_elemento_borrado_sale_de_la_rejilla() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(1, 0.0, 0.0, 10.0, 10.0));
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(&escena);

        escena.borrar(id);
        rejilla.sincronizar(&escena);

        assert!(rejilla.candidatos((-100.0, -100.0, 100.0, 100.0)).is_empty());
    }

    #[test]
    fn un_elemento_enorme_ocupa_todas_las_celdas_que_cruza() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(1, 0.0, 0.0, 1000.0, 1000.0));
        let mut rejilla = Rejilla::con_celda(100.0);
        rejilla.sincronizar(&escena);

        // Se encuentra tocando cualquiera de sus rincones.
        assert_eq!(rejilla.candidatos((5.0, 5.0, 6.0, 6.0)), vec![id]);
        assert_eq!(rejilla.candidatos((995.0, 995.0, 996.0, 996.0)), vec![id]);
    }

    #[test]
    fn no_devuelve_el_mismo_id_dos_veces() {
        // Un elemento que cruza cuatro celdas y una consulta que abarca las
        // cuatro: si no se deduplicara, se pintaria cuatro veces.
        let mut escena = Escena::nueva();
        escena.anadir(rect(1, 90.0, 90.0, 20.0, 20.0));
        let mut rejilla = Rejilla::con_celda(100.0);
        rejilla.sincronizar(&escena);

        assert_eq!(rejilla.candidatos((0.0, 0.0, 300.0, 300.0)).len(), 1);
    }
}
```

- [ ] **Paso 2: Comprueba que falla**

Declara `pub mod indice;` en `lib.rs` y corre:

```
cargo test -p pixpin-motor2d indice -- --test-threads=1
```

Esperado: `cannot find type 'Rejilla' in this scope`.

- [ ] **Paso 3: Escribe la implementación**

```rust
//! Que elementos pueden estar en una zona, sin recorrerlos todos.
//!
//! `elemento_en` y `recortar` recorren la lista entera. Con ocho mil
//! elementos, mover el raton por encima cuesta ocho mil pruebas de impacto
//! por cada aviso del raton.
//!
//! # Por que una rejilla y no un arbol (D27)
//!
//! Un quadtree o un R-tree serian mas elegantes y escalarian mejor a
//! millones. Pero aqui hablamos de ocho mil, y a esa escala la rejilla ya
//! gana por goleada. Ademas:
//!
//! - **No reasigna al consultar.** Un arbol recorre nodos y va acumulando;
//!   la rejilla lee celdas contiguas.
//! - **Se demuestra correcta contra la fuerza bruta**, que es justo lo que
//!   hace la prueba principal. Un arbol mal equilibrado da resultados casi
//!   correctos, que es la peor clase de fallo.
//!
//! # La regla que no se puede romper
//!
//! La rejilla puede devolver **de mas** —quien la usa filtra despues— pero
//! jamas **de menos**. Un elemento que no salga de aqui es un elemento que
//! desaparece de la pantalla sin ningun error visible.

use std::collections::HashMap;

use crate::escena::Escena;

/// Lado de la celda, en unidades del mundo.
///
/// 256 sale de que un elemento tipico de trabajo mide entre 20 y 300: con
/// celdas mucho mas pequenas, cada elemento se apunta en decenas de ellas;
/// con celdas mucho mas grandes, cada consulta devuelve medio dibujo.
pub const CELDA: f32 = 256.0;

#[derive(Debug, Clone)]
pub struct Rejilla {
    lado: f32,
    celdas: HashMap<(i32, i32), Vec<u64>>,
    /// La `version` con la que se apunto cada elemento, para saber cual ha
    /// cambiado sin comparar elementos enteros.
    versiones: HashMap<u64, u32>,
}

impl Default for Rejilla {
    fn default() -> Self {
        Self::con_celda(CELDA)
    }
}

impl Rejilla {
    pub fn nueva() -> Self {
        Self::default()
    }

    pub fn con_celda(lado: f32) -> Self {
        Self {
            lado: lado.max(1.0),
            celdas: HashMap::new(),
            versiones: HashMap::new(),
        }
    }

    fn celda_de(&self, x: f32, y: f32) -> (i32, i32) {
        ((x / self.lado).floor() as i32, (y / self.lado).floor() as i32)
    }

    /// Pone la rejilla al dia con la escena.
    ///
    /// Se llama en cada fotograma, asi que **no reconstruye**: mira la
    /// `version` de cada elemento y solo reapunta los que han cambiado. Si
    /// reconstruyera, seria mas cara que la fuerza bruta que viene a
    /// evitar.
    pub fn sincronizar(&mut self, escena: &Escena) {
        let mut vistos: Vec<u64> = Vec::with_capacity(escena.elementos.len());

        for e in &escena.elementos {
            vistos.push(e.id);
            let cambio = match self.versiones.get(&e.id) {
                Some(v) => *v != e.version,
                None => true,
            };
            if !cambio {
                continue;
            }
            self.quitar(e.id);
            if !e.borrado {
                self.meter(e.id, e.caja());
            }
            self.versiones.insert(e.id, e.version);
        }

        // Los que ya no estan en la escena (compactar los saco de verdad).
        let sobrantes: Vec<u64> = self
            .versiones
            .keys()
            .copied()
            .filter(|id| !vistos.contains(id))
            .collect();
        for id in sobrantes {
            self.quitar(id);
            self.versiones.remove(&id);
        }
    }

    fn meter(&mut self, id: u64, caja: (f32, f32, f32, f32)) {
        let (x0, y0, x1, y1) = caja;
        let (cx0, cy0) = self.celda_de(x0, y0);
        let (cx1, cy1) = self.celda_de(x1, y1);
        for cx in cx0..=cx1 {
            for cy in cy0..=cy1 {
                self.celdas.entry((cx, cy)).or_default().push(id);
            }
        }
    }

    fn quitar(&mut self, id: u64) {
        // Recorrer las celdas es aceptable porque solo pasa cuando un
        // elemento cambia, y entonces esta en unas pocas.
        self.celdas.retain(|_, ids| {
            ids.retain(|x| *x != id);
            !ids.is_empty()
        });
    }

    /// Los que **pueden** estar en la caja. Puede devolver de mas, nunca de
    /// menos: quien lo use filtra con `toca` o con `dentro_de`.
    pub fn candidatos(&self, caja: (f32, f32, f32, f32)) -> Vec<u64> {
        let (x0, y0, x1, y1) = caja;
        let (x0, x1) = (x0.min(x1), x0.max(x1));
        let (y0, y1) = (y0.min(y1), y0.max(y1));
        let (cx0, cy0) = self.celda_de(x0, y0);
        let (cx1, cy1) = self.celda_de(x1, y1);

        let mut fuera: Vec<u64> = Vec::new();
        for cx in cx0..=cx1 {
            for cy in cy0..=cy1 {
                let Some(ids) = self.celdas.get(&(cx, cy)) else { continue };
                for id in ids {
                    if !fuera.contains(id) {
                        fuera.push(*id);
                    }
                }
            }
        }
        fuera
    }

    /// Cuantas celdas ocupadas hay. Para las pruebas.
    pub fn cuantas_celdas(&self) -> usize {
        self.celdas.len()
    }
}
```

**Aviso de rendimiento que hay que medir, no suponer:** `quitar` recorre
todas las celdas ocupadas, y `candidatos` usa `contains` sobre un `Vec`. Con
ocho mil elementos ambas cosas están bien; con doscientos mil, no. La tarea 15
mide con 8.000 y 20.000, y si ahí ya se nota, el arreglo es guardar en
`versiones` también las celdas que ocupa cada elemento. **No lo hagas antes de
medirlo:** es complejidad de más si no hace falta.

- [ ] **Paso 4: Cierra**

```
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

- [ ] **Paso 5: Commit**

```bash
git add crates/pixpin-motor2d/src/indice.rs crates/pixpin-motor2d/src/lib.rs
git commit -m "Rejilla espacial: dejar de recorrer ocho mil para encontrar uno

elemento_en y recortar recorren la lista entera. Con ocho mil elementos,
mover el raton por encima cuesta ocho mil pruebas de impacto por aviso.

Rejilla uniforme y no quadtree ni R-tree (D27). A ocho mil elementos la
rejilla ya gana; no reasigna al consultar; y se demuestra correcta contra
la fuerza bruta, que es lo que hace la prueba principal con ochocientos
elementos al azar y doscientas consultas. Un arbol mal equilibrado da
resultados casi correctos, que es la peor clase de fallo.

La regla que no se puede romper: puede devolver de mas, jamas de menos.
Un elemento que no salga de aqui desaparece de la pantalla sin ningun
error visible.

Sincronizar no reconstruye: mira la version de cada elemento y reapunta
solo los que cambiaron. Se llama en cada fotograma, asi que reconstruir
seria mas caro que la fuerza bruta que viene a evitar."
```

---

## Tarea 8: La caché de geometría

El arreglo de rendimiento más grande del plan y el que menos código cuesta.
Hoy `pintado::ordenes()` regenera el garabato de **cada elemento en cada
fotograma**. El campo `version` de `Elemento` existe justo para evitarlo —lo
dice su propio comentario— y **hoy no lo usa nadie**.

**Ficheros:**
- Crear: `crates/pixpin-motor2d/src/cache.rs`
- Modificar: `crates/pixpin-motor2d/src/lib.rs`
- Prueba: en `cache.rs`, `mod pruebas`

**Interfaces:**
- Consume: `Elemento`, `Orden`, `pintado::ordenes_a_distancia`, `Azar`,
  `camara::{ZOOM_MINIMO, ZOOM_MAXIMO}`.
- Produce:
  ```rust
  pub const NIVEL_MINIMO: i8 = -5;   // 5 % de aumento
  pub const NIVEL_MAXIMO: i8 = 4;    // 3.000 %

  pub fn nivel_de_detalle(zoom: f32) -> i8;

  pub struct Cache { /* privado */ }
  impl Cache {
      pub fn nueva() -> Self;
      pub fn ordenes(&mut self, e: &Elemento, zoom: f32) -> &[Orden];
      pub fn olvidar(&mut self, id: u64);
      pub fn vaciar(&mut self);
      pub fn cuantos(&self) -> usize;
      pub fn aciertos(&self) -> u64;
      pub fn fallos(&self) -> u64;
  }
  ```

- [ ] **Paso 1: Escribe las pruebas que fallan**

Crea `crates/pixpin-motor2d/src/cache.rs` con solo el `mod pruebas`:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::azar::Azar;
    use crate::camara::{ZOOM_MAXIMO, ZOOM_MINIMO};
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};
    use crate::pintado::ordenes_a_distancia;

    fn trazo(semilla: u32) -> Elemento {
        let mut azar = Azar::nuevo(semilla);
        let puntos: Vec<Punto2> = (0..40)
            .map(|i| Punto2::nuevo(i as f32 * 3.0, azar.siguiente() * 50.0))
            .collect();
        Elemento {
            id: semilla as u64,
            figura: Figura::Lapiz { puntos, presiones: Vec::new() },
            x: 0.0,
            y: 0.0,
            ancho: 120.0,
            alto: 50.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            grosor: 3.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
        }
    }

    #[test]
    fn lo_cacheado_es_igual_a_lo_recien_generado() {
        // La prueba que hace la cache digna de confianza. Si esto falla,
        // el dibujo cambia de aspecto segun si venia de la cache o no, que
        // es peor que no tener cache.
        let mut cache = Cache::nueva();
        for semilla in 1..60u32 {
            let e = trazo(semilla);
            for zoom in [0.1, 0.5, 1.0, 2.0, 7.0] {
                let esperado = ordenes_a_distancia(&e, zoom);
                assert_eq!(cache.ordenes(&e, zoom), esperado.as_slice(), "semilla {semilla} al {zoom}");
                // Y la segunda vez, que ya viene de la cache, tambien.
                assert_eq!(cache.ordenes(&e, zoom), esperado.as_slice(), "cacheado");
            }
        }
    }

    #[test]
    fn encuadrar_no_invalida_nada() {
        // Mover el lienzo no cambia el zoom, asi que la cache entera vale.
        // Es lo que hace que arrastrar el lienzo con ocho mil elementos no
        // cueste nada.
        let mut cache = Cache::nueva();
        let e = trazo(1);
        cache.ordenes(&e, 1.0);
        let fallos = cache.fallos();

        for _ in 0..100 {
            cache.ordenes(&e, 1.0);
        }
        assert_eq!(cache.fallos(), fallos, "ni un fallo mas");
        assert_eq!(cache.aciertos(), 100);
    }

    #[test]
    fn mover_la_rueda_un_poco_no_tira_la_cache() {
        // El motivo de que el nivel sea la octava y no el zoom en bruto: si
        // fuera el zoom, un grado de rueda tiraria los ocho mil elementos.
        let mut cache = Cache::nueva();
        let e = trazo(1);
        cache.ordenes(&e, 1.0);
        let fallos = cache.fallos();

        cache.ordenes(&e, 1.3);
        cache.ordenes(&e, 1.9);
        assert_eq!(cache.fallos(), fallos, "1,0 / 1,3 / 1,9 son la misma octava");
    }

    #[test]
    fn cruzar_la_octava_si_la_tira() {
        let mut cache = Cache::nueva();
        let e = trazo(1);
        cache.ordenes(&e, 1.9);
        let fallos = cache.fallos();

        cache.ordenes(&e, 2.1);
        assert_eq!(cache.fallos(), fallos + 1, "2,0 empieza otra octava");
    }

    #[test]
    fn tocar_el_elemento_invalida_su_entrada() {
        let mut cache = Cache::nueva();
        let mut e = trazo(1);
        cache.ordenes(&e, 1.0);
        let fallos = cache.fallos();

        e.tocar();
        cache.ordenes(&e, 1.0);
        assert_eq!(cache.fallos(), fallos + 1, "la version subio");
    }

    #[test]
    fn el_nivel_de_detalle_es_la_octava_del_aumento() {
        assert_eq!(nivel_de_detalle(1.0), 0);
        assert_eq!(nivel_de_detalle(1.9), 0);
        assert_eq!(nivel_de_detalle(2.0), 1);
        assert_eq!(nivel_de_detalle(4.0), 2);
        assert_eq!(nivel_de_detalle(0.5), -1);
        assert_eq!(nivel_de_detalle(0.25), -2);
    }

    #[test]
    fn hay_diez_niveles_entre_los_topes_de_la_camara() {
        // Del 5 % al 3.000 %, que son los topes que ya tiene la camara.
        assert_eq!(nivel_de_detalle(ZOOM_MINIMO), NIVEL_MINIMO);
        assert_eq!(nivel_de_detalle(ZOOM_MAXIMO), NIVEL_MAXIMO);
        assert_eq!((NIVEL_MAXIMO - NIVEL_MINIMO + 1), 10);
    }

    #[test]
    fn un_zoom_absurdo_no_produce_un_nivel_absurdo() {
        // log2(0) es menos infinito, y un i8 no lo aguanta. Que no reviente
        // ni produzca un nivel de mil.
        assert_eq!(nivel_de_detalle(0.0), NIVEL_MINIMO);
        assert_eq!(nivel_de_detalle(-3.0), NIVEL_MINIMO);
        assert_eq!(nivel_de_detalle(f32::INFINITY), NIVEL_MAXIMO);
        assert_eq!(nivel_de_detalle(f32::NAN), NIVEL_MINIMO);
    }

    #[test]
    fn olvidar_saca_el_elemento_y_vaciar_los_saca_todos() {
        let mut cache = Cache::nueva();
        let a = trazo(1);
        let b = trazo(2);
        cache.ordenes(&a, 1.0);
        cache.ordenes(&b, 1.0);
        assert_eq!(cache.cuantos(), 2);

        cache.olvidar(a.id);
        assert_eq!(cache.cuantos(), 1);
        cache.vaciar();
        assert_eq!(cache.cuantos(), 0);
    }
}
```

- [ ] **Paso 2: Comprueba que falla**

Declara `pub mod cache;` en `lib.rs` y corre:

```
cargo test -p pixpin-motor2d cache -- --test-threads=1
```

Esperado: `cannot find type 'Cache' in this scope`.

- [ ] **Paso 3: Escribe la implementación**

```rust
//! No volver a calcular el garabato de lo que no ha cambiado.
//!
//! `pintado::ordenes()` regenera la geometria de **cada elemento en cada
//! fotograma**. Un dibujo de trabajo son ocho mil elementos: ocho mil
//! generaciones de ruido, sesenta veces por segundo, para un dibujo que no
//! ha cambiado. El campo `version` de `Elemento` existe justo para esto —lo
//! dice su propio comentario— y no lo usaba nadie.
//!
//! # Por que la clave lleva el nivel y no el zoom
//!
//! La geometria depende del aumento: `ordenes_a_distancia` adelgaza los
//! trazos que a esa distancia no se ven. Si la clave llevara el zoom en
//! bruto, mover la rueda un grado invalidaria los ocho mil elementos y la
//! cache no serviria de nada justo cuando mas falta hace.
//!
//! Por eso la clave lleva la **octava**: `log2(zoom)` redondeado hacia
//! abajo. Entre los topes de la camara —5 % y 3.000 %— son diez valores, y
//! solo cambia al doblar o partir por la mitad el aumento. Encuadrar no lo
//! cambia nunca, asi que arrastrar el lienzo no invalida nada.

use std::collections::HashMap;

use crate::elemento::Elemento;
use crate::pintado::{Orden, ordenes_a_distancia};

/// El nivel del 5 % de aumento, el tope de alejarse de la camara.
pub const NIVEL_MINIMO: i8 = -5;
/// El nivel del 3.000 %, el tope de acercarse.
pub const NIVEL_MAXIMO: i8 = 4;

/// La octava del aumento: `log2(zoom)` redondeado hacia abajo, sujeto a los
/// topes de la camara.
///
/// Sujetarlo no es paranoia: `log2(0)` es menos infinito y `log2` de un
/// negativo es NaN, y convertir cualquiera de los dos a `i8` es
/// comportamiento que no queremos ni mirar.
pub fn nivel_de_detalle(zoom: f32) -> i8 {
    if !zoom.is_finite() {
        return if zoom > 0.0 { NIVEL_MAXIMO } else { NIVEL_MINIMO };
    }
    if zoom <= 0.0 {
        return NIVEL_MINIMO;
    }
    let n = zoom.log2().floor();
    if n < NIVEL_MINIMO as f32 {
        NIVEL_MINIMO
    } else if n > NIVEL_MAXIMO as f32 {
        NIVEL_MAXIMO
    } else {
        n as i8
    }
}

#[derive(Debug, Clone)]
struct Entrada {
    version: u32,
    nivel: i8,
    ordenes: Vec<Orden>,
}

#[derive(Debug, Clone, Default)]
pub struct Cache {
    mapa: HashMap<u64, Entrada>,
    aciertos: u64,
    fallos: u64,
}

impl Cache {
    pub fn nueva() -> Self {
        Self::default()
    }

    /// La geometria del elemento a ese aumento, calculandola solo si hace
    /// falta.
    pub fn ordenes(&mut self, e: &Elemento, zoom: f32) -> &[Orden] {
        let nivel = nivel_de_detalle(zoom);
        let vale = self
            .mapa
            .get(&e.id)
            .is_some_and(|x| x.version == e.version && x.nivel == nivel);

        if vale {
            self.aciertos += 1;
        } else {
            self.fallos += 1;
            let ordenes = ordenes_a_distancia(e, zoom);
            self.mapa.insert(e.id, Entrada { version: e.version, nivel, ordenes });
        }
        // El `unwrap` es seguro: o valia, o se acaba de meter.
        &self.mapa.get(&e.id).expect("recien puesto").ordenes
    }

    /// Se llama al borrar de verdad un elemento (`Escena::compactar`).
    pub fn olvidar(&mut self, id: u64) {
        self.mapa.remove(&id);
    }

    /// Se llama al abrir otro documento.
    pub fn vaciar(&mut self) {
        self.mapa.clear();
    }

    pub fn cuantos(&self) -> usize {
        self.mapa.len()
    }

    /// Cuantas veces valio lo cacheado. Para las pruebas y la medicion.
    pub fn aciertos(&self) -> u64 {
        self.aciertos
    }

    /// Cuantas veces hubo que calcular. Para las pruebas y la medicion.
    pub fn fallos(&self) -> u64 {
        self.fallos
    }
}
```

**Una decisión consciente:** la caché **no tiene techo de memoria**. Guarda la
geometría de los elementos que se han pintado, que son los que caben en la
pantalla más los que se hayan visto al pasar; con ocho mil elementos y unas
pocas órdenes por elemento es del orden de megas, no decenas. Si la medición
de la tarea 15 dice otra cosa, el arreglo es tirar las entradas de los
elementos que llevan N fotogramas sin pintarse. **No lo hagas antes de
medirlo.**

- [ ] **Paso 4: Cierra**

```
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

- [ ] **Paso 5: Commit**

```bash
git add crates/pixpin-motor2d/src/cache.rs crates/pixpin-motor2d/src/lib.rs
git commit -m "Dejar de regenerar ocho mil garabatos sesenta veces por segundo

pintado::ordenes() regeneraba la geometria de cada elemento en cada
fotograma. El campo version de Elemento existe justo para evitarlo —lo
dice su propio comentario— y no lo usaba nadie. Es el arreglo de
rendimiento mas grande del plan y el que menos codigo cuesta.

La clave lleva el nivel de detalle y no el zoom en bruto. La geometria
depende del aumento porque ordenes_a_distancia adelgaza lo que no se ve;
si la clave llevara el zoom, mover la rueda un grado invalidaria los ocho
mil y la cache no serviria justo cuando mas falta hace. El nivel es la
octava: log2(zoom) hacia abajo, diez valores entre los topes de la
camara, y encuadrar no lo cambia nunca.

La prueba que la hace digna de confianza compara lo cacheado con lo
recien generado para sesenta trazos al azar a cinco aumentos. Si eso
fallara, el dibujo cambiaria de aspecto segun de donde viniera, que es
peor que no tener cache."
```

---

## Tarea 9: La máquina del gesto

Todo lo que pasa entre pulsar y soltar. En el Android son 3.482 líneas porque
atiende a dos, tres y cuatro dedos, al lápiz y al canto de la mano. Aquí hay
un ratón y un teclado.

**Ficheros:**
- Crear: `crates/pixpin-motor2d/src/gesto.rs`
- Mover: `enum Herramienta` de `crates/pixpin-ui/src/anotador.rs:28` a
  `gesto.rs`, y reexportarlo desde `anotador.rs` para no romper al anotador
- Modificar: `crates/pixpin-motor2d/src/lib.rs`
- Prueba: en `gesto.rs`, `mod pruebas`

**Interfaces:**
- Consume: todo lo de las tareas 1-8.
- Produce:
  ```rust
  pub enum Herramienta { Mano, Lapiz, Resaltador, Linea, Flecha,
                         Rectangulo, Elipse, Texto, Foco, Lupa, Borrador }

  pub enum EventoGesto {
      Pulsar { p: Punto2, shift: bool, alt: bool },
      Mover { p: Punto2, shift: bool, alt: bool },
      Soltar { p: Punto2 },
      Escape,
      Suprimir,
      Deshacer,
      Rehacer,
      SeleccionarTodo,
  }

  pub enum Region { Nada, Caja(f32, f32, f32, f32), Todo }

  pub enum FormaCursor { Flecha, Cruz, Mover, Texto, Giro,
                         Escalar { tirador: Tirador, angulo: f32 } }

  pub struct Respuesta { pub region: Region, pub cursor: FormaCursor }

  pub struct Gesto { pub seleccion: Seleccion, pub herramienta: Herramienta, /* resto privado */ }
  impl Gesto {
      pub fn nuevo() -> Self;
      pub fn evento(&mut self, ev: EventoGesto, escena: &mut Escena, escala: f32) -> Respuesta;
      pub fn en_reposo(&self) -> bool;
      pub fn marquesina(&self) -> Option<(f32, f32, f32, f32)>;
  }
  ```

### Dos estados del diseño que esta tarea NO trae, y por qué

El diagrama de la §7 del diseño lleva dos estados más. Quedan fuera a
propósito, y conviene decirlo antes de que alguien los eche en falta:

- **`Encuadrando`** (botón central o `Espacio`). No entra porque **no es un
  estado del gesto**: mover el lienzo solo cambia la cámara, no toca la
  escena, no abre paso de deshacer y no produce nada que probar sin ventana.
  Vive en `ventana_editor.rs` (tarea 11), donde está la cámara.
- **`Escribiendo`.** El texto es un subsistema entero —cursor, IME,
  reajuste de líneas, edición dentro de la caja— y el anotador ya tiene el
  suyo en `pixpin-ui/anotador.rs`. Meterlo aquí duplicaría esa lógica antes
  de decidir cuál de las dos se queda. La herramienta de texto sigue
  funcionando en el anotador; en el editor llega con la segunda entrega,
  junto con el tacto del trazo.

### Por qué la máquina toca la escena

En la §1 del diseño dije «devuelve intenciones y no toca nada». Eso era
engañoso, y aquí queda afinado (D23):

- **La escena sí la toca.** Es un `Vec` de datos, se prueba sin pantalla, y
  mantener aparte un registro de órdenes pendientes sería una segunda verdad
  sobre el mismo dibujo.
- **Lo que devuelve es lo que la ventana tiene que hacer**: qué región
  redibujar y qué cursor poner. De Direct2D no sabe nada.

- [ ] **Paso 1: Mueve `Herramienta` al motor**

Corta el `enum Herramienta` y su `impl` (líneas 27-60 de
`crates/pixpin-ui/src/anotador.rs`) y pégalos en `gesto.rs`. En `anotador.rs`,
en su sitio:

```rust
// La herramienta vive en el motor desde que la maquina del gesto se mudo
// alli: quien decide que hace un clic tiene que saber que herramienta hay
// puesta, y esa decision es logica pura, no interfaz.
pub use pixpin_motor2d::gesto::Herramienta;
```

Comprueba que `pixpin-ui` sigue compilando antes de seguir:

```
cargo build -p pixpin-ui
```

- [ ] **Paso 2: Escribe las pruebas que fallan**

En `gesto.rs`:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};
    use pixpin_geom::Tirador;

    fn rect(x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho,
            alto,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
        }
    }

    fn pulsar(p: Punto2) -> EventoGesto {
        EventoGesto::Pulsar { p, shift: false, alt: false }
    }
    fn mover(p: Punto2) -> EventoGesto {
        EventoGesto::Mover { p, shift: false, alt: false }
    }

    /// Un arrastre entero: pulsar, mover y soltar.
    fn arrastrar(g: &mut Gesto, e: &mut Escena, de: Punto2, a: Punto2) {
        g.evento(pulsar(de), e, 1.0);
        g.evento(mover(a), e, 1.0);
        g.evento(EventoGesto::Soltar { p: a }, e, 1.0);
    }

    #[test]
    fn un_arrastre_entero_deja_exactamente_un_paso_de_deshacer() {
        // El invariante que hace util el historial de la tarea 1.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        arrastrar(&mut g, &mut escena, Punto2::nuevo(50.0, 50.0), Punto2::nuevo(150.0, 50.0));
        assert_eq!(escena.buscar(id).unwrap().x, 100.0, "se movio");

        assert!(escena.deshacer());
        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "de una vez");
    }

    #[test]
    fn escape_a_mitad_de_un_arrastre_lo_cancela() {
        // Sale gratis: el paso ya guarda el estado anterior, asi que
        // cancelar es aplicarlo y tirar el paso.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(500.0, 500.0)), &mut escena, 1.0);
        g.evento(EventoGesto::Escape, &mut escena, 1.0);

        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "volvio a su sitio");
        assert!(g.en_reposo());
    }

    #[test]
    fn un_tirador_manda_sobre_el_elemento_que_haya_debajo() {
        // Sin esta prioridad, un tirador encima de otro elemento es
        // inalcanzable: el clic cae en el de debajo.
        let mut escena = Escena::nueva();
        let grande = escena.anadir(rect(0.0, 0.0, 200.0, 200.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(grande);

        // La esquina sureste del grande, que cae DENTRO del propio grande.
        let ts = Tiradores::de_elemento(escena.buscar(grande).unwrap(), 1.0);
        let se = ts.tamano.iter().find(|(c, _)| *c == Tirador::SuresteEsquina).unwrap().1;

        g.evento(pulsar(se), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(se.x + 100.0, se.y + 100.0)), &mut escena, 1.0);
        g.evento(EventoGesto::Soltar { p: Punto2::nuevo(se.x + 100.0, se.y + 100.0) }, &mut escena, 1.0);

        let e = escena.buscar(grande).unwrap();
        assert!(e.ancho > 250.0, "escalo en vez de moverse: {}", e.ancho);
        assert_eq!(e.x, 0.0, "y no se movio");
    }

    #[test]
    fn lo_ya_seleccionado_manda_sobre_lo_que_haya_encima() {
        // Sin esta regla, mover un grupo se convierte en seleccionar por
        // accidente lo que estaba encima.
        let mut escena = Escena::nueva();
        let abajo = escena.anadir(rect(0.0, 0.0, 200.0, 200.0));
        let encima = escena.anadir(rect(50.0, 50.0, 50.0, 50.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(abajo);

        // Pulsar donde estan los dos: gana el ya elegido.
        arrastrar(&mut g, &mut escena, Punto2::nuevo(75.0, 75.0), Punto2::nuevo(85.0, 75.0));

        assert_eq!(g.seleccion.ids(), &[abajo], "sigue el de antes");
        assert_eq!(escena.buscar(abajo).unwrap().x, 10.0, "se movio el de antes");
        assert_eq!(escena.buscar(encima).unwrap().x, 50.0, "el de encima, quieto");
    }

    #[test]
    fn arrastrar_en_vacio_con_la_mano_hace_marquesina() {
        let mut escena = Escena::nueva();
        let dentro = escena.anadir(rect(20.0, 20.0, 30.0, 30.0));
        let fuera = escena.anadir(rect(500.0, 500.0, 30.0, 30.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        assert!(g.marquesina().is_some(), "hay marquesina en curso");
        g.evento(mover(Punto2::nuevo(200.0, 200.0)), &mut escena, 1.0);
        g.evento(EventoGesto::Soltar { p: Punto2::nuevo(200.0, 200.0) }, &mut escena, 1.0);

        assert_eq!(g.seleccion.ids(), &[dentro]);
        assert!(!g.seleccion.contiene(fuera));
        assert!(g.marquesina().is_none(), "y se acabo al soltar");
    }

    #[test]
    fn shift_mas_clic_anade_a_la_seleccion() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 50.0, 50.0));
        let b = escena.anadir(rect(100.0, 0.0, 50.0, 50.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;

        g.evento(pulsar(Punto2::nuevo(25.0, 25.0)), &mut escena, 1.0);
        g.evento(EventoGesto::Soltar { p: Punto2::nuevo(25.0, 25.0) }, &mut escena, 1.0);
        g.evento(
            EventoGesto::Pulsar { p: Punto2::nuevo(125.0, 25.0), shift: true, alt: false },
            &mut escena,
            1.0,
        );
        g.evento(EventoGesto::Soltar { p: Punto2::nuevo(125.0, 25.0) }, &mut escena, 1.0);

        assert_eq!(g.seleccion.ids(), &[a, b]);
    }

    #[test]
    fn dibujar_un_trazo_lo_anade_y_lo_deja_deshacible() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        for i in 1..20 {
            g.evento(mover(Punto2::nuevo(i as f32 * 5.0, 0.0)), &mut escena, 1.0);
        }
        g.evento(EventoGesto::Soltar { p: Punto2::nuevo(95.0, 0.0) }, &mut escena, 1.0);

        assert_eq!(escena.cuantos_visibles(), 1);
        assert!(escena.deshacer());
        assert_eq!(escena.cuantos_visibles(), 0, "un trazo, un Ctrl+Z");
    }

    #[test]
    fn dibujando_se_ensucia_el_tramo_y_no_la_pantalla() {
        // En 1080p es la diferencia entre dos millones de pixeles por
        // fotograma y unos cientos.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        let r = g.evento(mover(Punto2::nuevo(10.0, 0.0)), &mut escena, 1.0);

        let Region::Caja(x0, y0, x1, y1) = r.region else {
            panic!("tiene que ser una caja, es {:?}", r.region);
        };
        assert!(x1 - x0 < 40.0 && y1 - y0 < 40.0, "el tramo, no la pantalla");
    }

    #[test]
    fn dibujar_no_asigna_memoria_por_cada_aviso_del_raton() {
        // El buffer del trazo en curso se reserva de una vez y se reutiliza
        // entre trazos con clear(). La prueba de verdad —contar
        // asignaciones— es la tarea 15; aqui se comprueba lo que se puede
        // observar desde dentro.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        for i in 1..400 {
            g.evento(mover(Punto2::nuevo(i as f32, 0.0)), &mut escena, 1.0);
        }
        assert!(
            g.capacidad_del_trazo() >= PUNTOS_RESERVADOS,
            "se reservo de una vez"
        );
        g.evento(EventoGesto::Soltar { p: Punto2::nuevo(400.0, 0.0) }, &mut escena, 1.0);

        let tras_el_primero = g.capacidad_del_trazo();
        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        assert_eq!(
            g.capacidad_del_trazo(),
            tras_el_primero,
            "el segundo trazo reutiliza el buffer del primero"
        );
    }

    #[test]
    fn suprimir_borra_lo_seleccionado_de_una_vez() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = escena.anadir(rect(20.0, 0.0, 10.0, 10.0));
        let mut g = Gesto::nuevo();
        g.seleccion.poner_todos([a, b]);

        g.evento(EventoGesto::Suprimir, &mut escena, 1.0);
        assert_eq!(escena.cuantos_visibles(), 0);

        assert!(escena.deshacer());
        assert_eq!(escena.cuantos_visibles(), 2, "los dos vuelven de una vez");
    }

    #[test]
    fn seleccionar_todo_no_coge_los_borrados() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = escena.anadir(rect(20.0, 0.0, 10.0, 10.0));
        escena.borrar(b);
        let mut g = Gesto::nuevo();

        g.evento(EventoGesto::SeleccionarTodo, &mut escena, 1.0);
        assert_eq!(g.seleccion.ids(), &[a]);
    }

    #[test]
    fn pulsar_dos_veces_sin_soltar_no_pierde_el_gesto() {
        // La ventana puede recibir un WM_LBUTTONDOWN sin su WM_LBUTTONUP si
        // el usuario suelta fuera. Perder el gesto entero por eso seria
        // peor que ignorar el segundo pulsar.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(150.0, 50.0)), &mut escena, 1.0);
        g.evento(EventoGesto::Soltar { p: Punto2::nuevo(150.0, 50.0) }, &mut escena, 1.0);

        assert_eq!(escena.buscar(id).unwrap().x, 100.0);
        assert!(escena.deshacer());
        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "sigue siendo un paso");
    }

    #[test]
    fn el_cursor_de_escalar_va_girado_con_el_elemento() {
        // En una figura a 45 grados, el tirador de la esquina ensena la
        // flecha que de verdad apunta hacia donde va a crecer.
        use std::f32::consts::FRAC_PI_4;
        let mut escena = Escena::nueva();
        let mut e = rect(0.0, 0.0, 100.0, 100.0);
        e.angulo = FRAC_PI_4;
        let id = escena.anadir(e);
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        let ts = Tiradores::de_elemento(escena.buscar(id).unwrap(), 1.0);
        let se = ts.tamano.iter().find(|(c, _)| *c == Tirador::SuresteEsquina).unwrap().1;
        let r = g.evento(mover(se), &mut escena, 1.0);

        let FormaCursor::Escalar { tirador, angulo } = r.cursor else {
            panic!("sobre un tirador toca cursor de escalar, es {:?}", r.cursor);
        };
        assert_eq!(tirador, Tirador::SuresteEsquina, "cual, para saber la direccion");
        assert!(
            (angulo - FRAC_PI_4).abs() < 1e-3,
            "y el angulo del elemento, para girarla: {angulo}"
        );
    }

    #[test]
    fn la_direccion_de_un_tirador_gira_con_el_elemento() {
        // Sin girar, la esquina sureste apunta a 135 grados (abajo y a la
        // derecha). Girado un cuarto de vuelta, apunta a 225.
        use std::f32::consts::{FRAC_PI_2, PI};
        let recta = direccion_del_tirador(Tirador::SuresteEsquina, 0.0);
        assert!((recta - 3.0 * PI / 4.0).abs() < 1e-3, "sin girar: {recta}");

        let girada = direccion_del_tirador(Tirador::SuresteEsquina, FRAC_PI_2);
        assert!((girada - 5.0 * PI / 4.0).abs() < 1e-3, "girada: {girada}");
    }
}
```

- [ ] **Paso 3: Comprueba que fallan**

Declara `pub mod gesto;` en `lib.rs`:

```
cargo test -p pixpin-motor2d gesto -- --test-threads=1
```

- [ ] **Paso 4: Escribe la implementación**

Primero el tipo y el despacho de eventos; los tres métodos del gesto van en
el paso siguiente:

```rust
//! Todo lo que pasa entre pulsar y soltar.
//!
//! En el Android son 3.482 lineas (`DrawController.kt`) porque atiende a
//! dos, tres y cuatro dedos, al lapiz y al canto de la mano. Aqui hay un
//! raton y un teclado, asi que donde el Android pregunta «hay segundo
//! dedo?», esto pregunta «esta Shift?».
//!
//! # Que toca y que devuelve (D23)
//!
//! **La escena si la toca.** Es un `Vec` de datos, se prueba sin pantalla, y
//! mantener aparte un registro de ordenes pendientes seria una segunda
//! verdad sobre el mismo dibujo.
//!
//! **Lo que devuelve es lo que la ventana tiene que hacer**: que region
//! redibujar y que cursor poner. De Direct2D no sabe nada. Por eso esto vive
//! en el motor y no en la interfaz: una prueba puede decir «pulsar aqui,
//! mover cuarenta pixeles con Shift, soltar» y comprobar el resultado exacto
//! sin abrir una ventana.
//!
//! # El orden de decision al pulsar
//!
//! Es la parte que hay que leer despacio:
//!
//! 1. **Un tirador manda sobre lo que haya debajo.** Sin esto, un tirador
//!    encima de un trazo es inalcanzable.
//! 2. **Lo ya seleccionado manda sobre lo de encima.** Sin esto, mover un
//!    grupo se convierte en seleccionar por accidente lo que estaba encima.
//! 3. Lo que haya bajo el cursor.
//! 4. Vacio: marquesina si la herramienta es la mano, dibujar si no.

use pixpin_geom::Tirador;

use crate::elemento::{Elemento, Figura};
use crate::escena::Escena;
use crate::impacto::{dentro_de, elemento_en};
use crate::seleccion::Seleccion;
use crate::tiradores::{Agarre, Tiradores};
use crate::transformar::{self, a_saltos, angulo_hacia};
use crate::vector::Punto2;

/// Puntos que se reservan de una vez para el trazo en curso.
///
/// 512 porque el trazo mas largo del fichero del movil tiene 492. Reservar
/// de una vez es lo que permite que mover el raton dibujando no asigne
/// memoria — la regla del camino caliente.
pub const PUNTOS_RESERVADOS: usize = 512;

/// Con que se dibuja. Vino de `pixpin-ui/anotador.rs` en el paso 1: quien
/// decide que hace un clic tiene que saber que herramienta hay puesta, y esa
/// decision es logica pura, no interfaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Herramienta {
    /// Seleccionar y mover lo ya dibujado.
    Mano,
    Lapiz,
    Resaltador,
    Linea,
    Flecha,
    Rectangulo,
    Elipse,
    Texto,
    /// Oscurece todo menos una zona (D51).
    Foco,
    /// Amplia alrededor del cursor. No deja rastro: es una vista (D52).
    Lupa,
    Borrador,
}

impl Herramienta {
    /// Si necesita un arrastre de verdad para producir algo. El lapiz no:
    /// un clic deja un punto de tinta, que es lo que espera cualquiera que
    /// haya usado un rotulador.
    pub fn necesita_arrastre(self) -> bool {
        !matches!(self, Herramienta::Lapiz | Herramienta::Texto)
    }

    /// Si lo que dibuja se guarda en el documento.
    pub fn deja_rastro(self) -> bool {
        !matches!(
            self,
            Herramienta::Mano | Herramienta::Lupa | Herramienta::Borrador
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventoGesto {
    Pulsar { p: Punto2, shift: bool, alt: bool },
    Mover { p: Punto2, shift: bool, alt: bool },
    Soltar { p: Punto2 },
    Escape,
    Suprimir,
    Deshacer,
    Rehacer,
    SeleccionarTodo,
}

/// Que hay que volver a pintar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Region {
    Nada,
    /// Solo esta caja del mundo. Es lo que se usa en el camino caliente.
    Caja(f32, f32, f32, f32),
    Todo,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormaCursor {
    Flecha,
    Cruz,
    Mover,
    Texto,
    Giro,
    /// Escalar, con el angulo del elemento para que la flecha apunte a
    /// donde de verdad va a crecer.
    Escalar(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Respuesta {
    pub region: Region,
    pub cursor: FormaCursor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Estado {
    Reposo,
    Dibujando { id: u64 },
    Moviendo { anterior: Punto2 },
    Escalando { tirador: Tirador },
    Girando { anterior: f32 },
    Marquesina { origen: Punto2, hasta: Punto2 },
}

pub struct Gesto {
    estado: Estado,
    pub seleccion: Seleccion,
    pub herramienta: Herramienta,
    /// El trazo en curso. Se reserva una vez y se reutiliza con `clear()`.
    trazo: Vec<Punto2>,
}

impl Default for Gesto {
    fn default() -> Self {
        Self {
            estado: Estado::Reposo,
            seleccion: Seleccion::nueva(),
            herramienta: Herramienta::Lapiz,
            trazo: Vec::with_capacity(PUNTOS_RESERVADOS),
        }
    }
}

impl Gesto {
    pub fn nuevo() -> Self {
        Self::default()
    }

    pub fn en_reposo(&self) -> bool {
        matches!(self.estado, Estado::Reposo)
    }

    /// La marquesina en curso, para que la ventana la pinte.
    pub fn marquesina(&self) -> Option<(f32, f32, f32, f32)> {
        match self.estado {
            Estado::Marquesina { origen, hasta } => Some((origen.x, origen.y, hasta.x, hasta.y)),
            _ => None,
        }
    }

    /// Para la prueba de que el buffer se reutiliza.
    pub fn capacidad_del_trazo(&self) -> usize {
        self.trazo.capacity()
    }

    pub fn evento(&mut self, ev: EventoGesto, escena: &mut Escena, escala: f32) -> Respuesta {
        match ev {
            EventoGesto::Pulsar { p, shift, alt } => self.pulsar(p, shift, alt, escena, escala),
            EventoGesto::Mover { p, shift, alt } => self.mover(p, shift, alt, escena, escala),
            EventoGesto::Soltar { p } => self.soltar(p, escena),
            EventoGesto::Escape => {
                escena.cancelar_paso();
                self.estado = Estado::Reposo;
                self.seleccion.limpiar();
                Respuesta { region: Region::Todo, cursor: FormaCursor::Flecha }
            }
            EventoGesto::Suprimir => {
                escena.abrir_paso();
                for &id in self.seleccion.ids() {
                    // `borrar_apuntando` y no `borrar`: el primero apunta el
                    // cambio en el paso, el segundo no. Con `borrar`, la
                    // prueba `suprimir_borra_lo_seleccionado_de_una_vez`
                    // falla en su segunda mitad.
                    escena.borrar_apuntando(id);
                }
                escena.cerrar_paso();
                self.seleccion.limpiar();
                Respuesta { region: Region::Todo, cursor: FormaCursor::Flecha }
            }
            EventoGesto::Deshacer => {
                escena.deshacer();
                self.seleccion.limpiar();
                Respuesta { region: Region::Todo, cursor: FormaCursor::Flecha }
            }
            EventoGesto::Rehacer => {
                escena.rehacer();
                Respuesta { region: Region::Todo, cursor: FormaCursor::Flecha }
            }
            EventoGesto::SeleccionarTodo => {
                let vivos: Vec<u64> = escena.visibles().map(|e| e.id).collect();
                self.seleccion.poner_todos(vivos);
                Respuesta { region: Region::Todo, cursor: FormaCursor::Flecha }
            }
        }
    }
}
```

- [ ] **Paso 5: Escribe `pulsar`, `mover` y `soltar`**

En el mismo `impl Gesto`:

```rust
    /// Los tiradores de la seleccion, si hay algo elegido.
    fn tiradores(&self, escena: &Escena, escala: f32) -> Option<Tiradores> {
        let caja = self.seleccion.caja(escena)?;
        // Con un solo elemento, el marco lleva su angulo. Con varios, la
        // caja es paralela a los ejes y cada uno conserva el suyo.
        let angulo = match self.seleccion.ids() {
            [uno] => escena.buscar(*uno).map_or(0.0, |e| e.angulo),
            _ => 0.0,
        };
        Some(Tiradores::de_caja(caja, angulo, escala))
    }

    /// El elemento nuevo que empieza esta herramienta en este punto.
    ///
    /// Los puntos se reservan de una vez, igual que el buffer del gesto: es
    /// lo que hace que mover el raton dibujando **no asigne memoria**. Si
    /// este `Vec` empezara vacio, crecer de 4 a 8 a 16... asignaria una
    /// docena de veces por trazo, en el unico camino del programa con un
    /// plazo sagrado. La tarea 15 lo comprueba contando asignaciones.
    fn nuevo_elemento(&self, p: Punto2) -> Elemento {
        let reservados = || {
            let mut v = Vec::with_capacity(PUNTOS_RESERVADOS);
            v.push(p);
            v
        };
        let figura = match self.herramienta {
            Herramienta::Lapiz => Figura::Lapiz {
                puntos: reservados(),
                presiones: Vec::new(),
            },
            Herramienta::Resaltador => Figura::Resaltador { puntos: reservados() },
            Herramienta::Linea => Figura::Linea { puntos: vec![p, p] },
            Herramienta::Flecha => Figura::Flecha {
                puntos: vec![p, p],
                punta_inicio: false,
                punta_fin: true,
            },
            Herramienta::Elipse => Figura::Elipse,
            Herramienta::Foco => Figura::Foco { elipse: false },
            // Rectangulo y todo lo demas que deje rastro.
            _ => Figura::Rectangulo,
        };
        Elemento {
            id: 0, // lo pone `Escena::anadir`
            figura,
            x: p.x,
            y: p.y,
            ancho: 0.0,
            alto: 0.0,
            angulo: 0.0,
            trazo: crate::elemento::ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            grosor: 3.0,
            estilo: crate::elemento::EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
        }
    }

Y esta, **fuera del `impl Gesto`**, al nivel del módulo, porque la ventana la
llama sin tener un `Gesto` a mano:

```rust
/// Hacia donde apunta un tirador, en un elemento girado `angulo`.
///
/// Cero es hacia arriba y crece en el sentido de las agujas, igual que
/// `transformar::angulo_hacia`. La ventana usa esto para elegir entre las
/// cuatro flechas que trae Windows.
pub fn direccion_del_tirador(t: Tirador, angulo: f32) -> f32 {
        use std::f32::consts::PI;
        let base = match t {
            Tirador::NorteBorde => 0.0,
            Tirador::NoresteEsquina => PI / 4.0,
            Tirador::EsteBorde => PI / 2.0,
            Tirador::SuresteEsquina => 3.0 * PI / 4.0,
            Tirador::SurBorde => PI,
            Tirador::SuroesteEsquina => 5.0 * PI / 4.0,
            Tirador::OesteBorde => 3.0 * PI / 2.0,
        Tirador::NoroesteEsquina => 7.0 * PI / 4.0,
    };
    base + angulo
}
```

Y de vuelta dentro del `impl Gesto`:

```rust
    /// Que cursor toca en este punto, estando en reposo.
    fn cursor_en(&self, p: Punto2, escena: &Escena, escala: f32) -> FormaCursor {
        if let Some(ts) = self.tiradores(escena, escala) {
            match ts.en(p, escala) {
                // Van los dos: cual, para saber a que lado apunta; y el
                // angulo, para girar esa direccion con el elemento.
                Some(Agarre::Tamano(t)) => {
                    return FormaCursor::Escalar { tirador: t, angulo: ts.angulo };
                }
                Some(Agarre::Giro) => return FormaCursor::Giro,
                None => {}
            }
        }
        match self.herramienta {
            Herramienta::Mano => {
                if elemento_en(&escena.elementos, p).is_some() {
                    FormaCursor::Mover
                } else {
                    FormaCursor::Flecha
                }
            }
            Herramienta::Texto => FormaCursor::Texto,
            _ => FormaCursor::Cruz,
        }
    }

    fn pulsar(
        &mut self,
        p: Punto2,
        shift: bool,
        _alt: bool,
        escena: &mut Escena,
        escala: f32,
    ) -> Respuesta {
        // Un segundo pulsar sin su soltar: la ventana puede recibirlo si el
        // usuario solto fuera. Se ignora en vez de perder el gesto entero.
        if !self.en_reposo() {
            return Respuesta {
                region: Region::Nada,
                cursor: self.cursor_en(p, escena, escala),
            };
        }
        escena.abrir_paso();

        // 1. Un tirador manda sobre lo que haya debajo.
        if let Some(ts) = self.tiradores(escena, escala) {
            match ts.en(p, escala) {
                Some(Agarre::Tamano(t)) => {
                    self.estado = Estado::Escalando { tirador: t };
                    return Respuesta {
                        region: Region::Nada,
                        cursor: FormaCursor::Escalar { tirador: t, angulo: ts.angulo },
                    };
                }
                Some(Agarre::Giro) => {
                    self.estado = Estado::Girando {
                        anterior: angulo_hacia(ts.centro, p),
                    };
                    return Respuesta { region: Region::Nada, cursor: FormaCursor::Giro };
                }
                None => {}
            }
        }

        // 2. Lo ya seleccionado manda sobre lo de encima.
        let sobre_lo_elegido = self
            .seleccion
            .ids()
            .iter()
            .filter_map(|id| escena.buscar(*id))
            .any(|e| crate::impacto::toca(e, p));
        if sobre_lo_elegido && !shift {
            self.estado = Estado::Moviendo { anterior: p };
            return Respuesta { region: Region::Nada, cursor: FormaCursor::Mover };
        }

        // 3. Lo que haya bajo el cursor.
        if let Some(id) = elemento_en(&escena.elementos, p) {
            if shift {
                self.seleccion.alternar(id);
            } else {
                self.seleccion.poner(id);
            }
            self.estado = Estado::Moviendo { anterior: p };
            return Respuesta { region: Region::Todo, cursor: FormaCursor::Mover };
        }

        // 4. Vacio.
        if self.herramienta == Herramienta::Mano {
            if !shift {
                self.seleccion.limpiar();
            }
            self.estado = Estado::Marquesina { origen: p, hasta: p };
            return Respuesta { region: Region::Todo, cursor: FormaCursor::Flecha };
        }
        // El texto no entra en esta entrega (ver arriba): con la herramienta
        // de texto puesta, un clic en vacio no hace nada en vez de dejar un
        // rectangulo, que es lo que pasaria al caer en el `_` de
        // `nuevo_elemento`.
        if self.herramienta.deja_rastro() && self.herramienta != Herramienta::Texto {
            self.seleccion.limpiar();
            self.trazo.clear();
            self.trazo.push(p);
            let id = escena.anadir(self.nuevo_elemento(p));
            self.estado = Estado::Dibujando { id };
            return Respuesta { region: Region::Todo, cursor: FormaCursor::Cruz };
        }
        Respuesta { region: Region::Nada, cursor: FormaCursor::Cruz }
    }

    fn mover(
        &mut self,
        p: Punto2,
        shift: bool,
        alt: bool,
        escena: &mut Escena,
        escala: f32,
    ) -> Respuesta {
        match self.estado {
            Estado::Reposo => Respuesta {
                region: Region::Nada,
                cursor: self.cursor_en(p, escena, escala),
            },

            Estado::Dibujando { id } => {
                // El camino caliente. Ni una asignacion: el buffer ya esta
                // reservado, y se ensucia el tramo y no la pantalla.
                let anterior = *self.trazo.last().unwrap_or(&p);
                if self.trazo.len() < PUNTOS_RESERVADOS {
                    self.trazo.push(p);
                }
                let grosor = escena.buscar(id).map_or(3.0, |e| e.grosor);
                if let Some(e) = escena.buscar_mut(id) {
                    match &mut e.figura {
                        Figura::Lapiz { puntos, .. } | Figura::Resaltador { puntos } => {
                            puntos.push(p);
                        }
                        Figura::Linea { puntos } | Figura::Flecha { puntos, .. } => {
                            // Linea y flecha son dos puntos: el segundo sigue
                            // al cursor en vez de acumularse.
                            if let Some(ultimo) = puntos.last_mut() {
                                *ultimo = p;
                            }
                        }
                        _ => {
                            // Las figuras de caja crecen desde donde se pulso.
                            let o = *self.trazo.first().unwrap_or(&p);
                            e.x = o.x.min(p.x);
                            e.y = o.y.min(p.y);
                            e.ancho = (p.x - o.x).abs();
                            e.alto = (p.y - o.y).abs();
                        }
                    }
                    e.tocar();
                }
                let m = grosor / 2.0 + 1.0;
                Respuesta {
                    region: Region::Caja(
                        anterior.x.min(p.x) - m,
                        anterior.y.min(p.y) - m,
                        anterior.x.max(p.x) + m,
                        anterior.y.max(p.y) + m,
                    ),
                    cursor: FormaCursor::Cruz,
                }
            }

            Estado::Moviendo { anterior } => {
                let (dx, dy) = (p.x - anterior.x, p.y - anterior.y);
                for &id in self.seleccion.ids() {
                    // Sin esto el paso queda vacio y no hay nada que
                    // deshacer. Es el error mas facil de cometer aqui.
                    escena.apuntar_edicion(id);
                    if let Some(e) = escena.buscar_mut(id) {
                        e.mover(dx, dy);
                    }
                }
                self.estado = Estado::Moviendo { anterior: p };
                Respuesta { region: Region::Todo, cursor: FormaCursor::Mover }
            }

            Estado::Escalando { tirador } => {
                for &id in self.seleccion.ids() {
                    escena.apuntar_edicion(id);
                    if let Some(e) = escena.buscar_mut(id) {
                        transformar::escalar(e, tirador, p, shift, alt);
                    }
                }
                let angulo = self.tiradores(escena, escala).map_or(0.0, |t| t.angulo);
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Escalar { tirador, angulo },
                }
            }

            Estado::Girando { anterior } => {
                let Some(centro) = self.seleccion.centro(escena) else {
                    return Respuesta { region: Region::Nada, cursor: FormaCursor::Giro };
                };
                let ahora = angulo_hacia(centro, p);
                let ahora = if shift { a_saltos(ahora) } else { ahora };
                let delta = ahora - anterior;
                for &id in self.seleccion.ids() {
                    escena.apuntar_edicion(id);
                    if let Some(e) = escena.buscar_mut(id) {
                        transformar::girar(e, centro, delta);
                    }
                }
                self.estado = Estado::Girando { anterior: ahora };
                Respuesta { region: Region::Todo, cursor: FormaCursor::Giro }
            }

            Estado::Marquesina { origen, .. } => {
                self.estado = Estado::Marquesina { origen, hasta: p };
                Respuesta { region: Region::Todo, cursor: FormaCursor::Flecha }
            }
        }
    }

    fn soltar(&mut self, p: Punto2, escena: &mut Escena) -> Respuesta {
        if let Estado::Marquesina { origen, .. } = self.estado {
            let caja = (origen.x, origen.y, p.x, p.y);
            let cogidos = dentro_de(&escena.elementos, caja);
            self.seleccion.poner_todos(cogidos);
        }
        // Un paso sin cambios no entra en el historial, asi que hacer clic
        // sin arrastrar no consume un Ctrl+Z. De eso se encarga cerrar_paso.
        escena.cerrar_paso();
        self.estado = Estado::Reposo;
        Respuesta { region: Region::Todo, cursor: FormaCursor::Flecha }
    }
```

**Un aviso.** Fíjate en que los bucles sobre `self.seleccion.ids()` **no llevan
`.to_vec()`**, y no debe llevarlo. Parece que hiciera falta —dentro se toma
`&mut escena`— pero no: `escena` es un parámetro aparte, no un campo de
`self`, así que el préstamo inmutable de `self.seleccion` y el mutable de
`escena` no se pisan.

Es importante, no cosmético: `mover` se llama **en cada aviso del ratón**. Un
`.to_vec()` ahí asigna memoria en el camino caliente, sesenta veces por
segundo mientras se arrastra, y la prueba del asignador de la tarea 15 lo
caza.

- [ ] **Paso 6: Cierra**

```
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

- [ ] **Paso 7: Commit**

```bash
git add crates/pixpin-motor2d/src/gesto.rs crates/pixpin-motor2d/src/lib.rs crates/pixpin-ui/src/anotador.rs
git commit -m "La maquina del gesto, en el motor y no en la interfaz

En el Android son 3.482 lineas porque atiende a dos, tres y cuatro dedos,
al lapiz y al canto de la mano. Aqui hay un raton: donde el Android
pregunta si hay segundo dedo, esto pregunta si esta Shift.

Vive en el motor (D22) para que una prueba pueda decir «pulsar aqui,
mover cuarenta pixeles con Shift, soltar» y comprobar el resultado exacto
sin abrir una ventana. De paso, anotador.rs deja de tener 1.097 lineas
haciendo tres trabajos.

Afina lo que dije en la §1 del diseno (D23): la escena SI la toca —es un
Vec de datos y mantener un registro de ordenes en paralelo seria una
segunda verdad sobre el mismo dibujo—, y lo que devuelve es lo que la
ventana tiene que hacer: que region redibujar y que cursor poner.

El orden de decision al pulsar es la parte delicada. Un tirador manda
sobre lo que haya debajo, o un tirador encima de un trazo seria
inalcanzable. Y lo ya seleccionado manda sobre lo de encima, o mover un
grupo se convertiria en seleccionar por accidente lo que estaba encima."
```

---

## Tarea 10: `Alt` y el cursor de giro en la ventana

Dos huecos pequeños en `pixpin-shell`. `EventoOverlay::Tecla` trae `shift` y
`ctrl` pero no `alt`, y `Alt` es lo que escala desde el centro. Y no hay
cursor de giro.

**Ficheros:**
- Modificar: `crates/pixpin-shell/src/overlay.rs` — `EventoOverlay::Tecla`
  (línea 52), `FormaCursorWin` (línea 83), la lectura de teclas (línea 532) y
  el mapeo a `IDC_*` (línea 586)
- Modificar: quien construya o case `EventoOverlay::Tecla` —
  `apps/pixpin/src/capa.rs`, `apps/pixpin/src/overlay.rs`,
  `crates/pixpin-ui/src/anotador.rs`
- Prueba: en `overlay.rs`, `mod pruebas`

**Interfaces:**
- Produce:
  ```rust
  pub enum EventoOverlay {
      Tecla { vk: u32, shift: bool, ctrl: bool, alt: bool },  // + alt
      // ...el resto igual
  }
  pub enum FormaCursorWin { /* ...los ocho de hoy... */, Giro }
  ```

- [ ] **Paso 1: Escribe la prueba que falla**

`GetKeyState` necesita una sesión de escritorio, así que lo que se prueba aquí
es la parte pura: que el enum lleva el campo y que el mapeo de cursores es
total. En `mod pruebas` de `overlay.rs`:

```rust
#[test]
fn la_tecla_lleva_alt_ademas_de_shift_y_ctrl() {
    // Alt escala desde el centro. Sin este campo, el editor no puede
    // distinguir un arrastre normal de uno desde el centro.
    let t = EventoOverlay::Tecla { vk: 65, shift: true, ctrl: false, alt: true };
    let EventoOverlay::Tecla { alt, .. } = t else { panic!() };
    assert!(alt);
}

#[test]
fn hay_cursor_de_giro() {
    // Que exista la variante. Cual dibuja Windows se comprueba a mano: el
    // mapeo a IDC_* necesita una sesion de escritorio.
    let formas = [
        FormaCursorWin::Cruz,
        FormaCursorWin::Mover,
        FormaCursorWin::RedimNS,
        FormaCursorWin::RedimEO,
        FormaCursorWin::RedimNeSo,
        FormaCursorWin::RedimNoSe,
        FormaCursorWin::Texto,
        FormaCursorWin::Flecha,
        FormaCursorWin::Giro,
    ];
    assert_eq!(formas.len(), 9);
}
```

- [ ] **Paso 2: Añade el campo y la variante**

En `EventoOverlay::Tecla` (línea 52):

```rust
    Tecla {
        vk: u32,
        shift: bool,
        /// Ctrl mantenido: para `Ctrl+A` (seleccionar todo) y `Ctrl+Z`.
        ctrl: bool,
        /// Alt mantenido: en el editor, escalar desde el centro en vez de
        /// desde el ancla.
        alt: bool,
    },
```

En `FormaCursorWin` (línea 83):

```rust
    /// Girar lo seleccionado.
    ///
    /// Windows no trae cursor de giro: los ocho `IDC_*` estandar son
    /// flechas, cruz, barra de texto y poco mas. Se usa `IDC_HAND` porque
    /// al menos se distingue de los de redimension y no miente sobre lo
    /// que va a pasar. Un cursor de giro de verdad necesitaria un recurso
    /// propio, y eso no entra en esta entrega.
    Giro,
```

En la lectura de teclas (línea 532), junto a `shift` y `ctrl`:

```rust
            // SAFETY: GetKeyState es una consulta sin precondiciones.
            let alt = unsafe { GetKeyState(VK_MENU.0 as i32) } < 0;
```

Añade `VK_MENU` al `use` de la línea 23. En Win32, `VK_MENU` **es** la tecla
Alt; el nombre viene de que Alt abre los menús.

En el mapeo a `IDC_*` (línea 586):

```rust
                FormaCursorWin::Giro => IDC_HAND,
```

Añade `IDC_HAND` al `use` correspondiente.

- [ ] **Paso 3: Arregla a quien casaba el evento**

```
cargo build --workspace --all-targets 2>&1 | grep -B2 -A6 "Tecla" | head -40
```

En cada `EventoOverlay::Tecla { vk, shift, ctrl }` añade `alt` o `..`. Donde
el `alt` no importe todavía —el anotador de pantalla, la capa— usa `..`, que
es más honesto que ignorar una variable con nombre.

- [ ] **Paso 4: Cierra**

```
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

- [ ] **Paso 5: Commit**

```bash
git add crates/pixpin-shell/src/overlay.rs crates/pixpin-ui/src/anotador.rs apps/pixpin/src/capa.rs apps/pixpin/src/overlay.rs
git commit -m "Alt y el cursor de giro en la ventana

Dos huecos pequenos que el editor necesita. EventoOverlay::Tecla traia
shift y ctrl pero no alt, y Alt es lo que escala desde el centro en vez
de desde el ancla.

Y no habia cursor de giro. Windows no trae uno: los IDC_* estandar son
flechas, cruz y barra de texto. Se usa IDC_HAND porque al menos se
distingue de los de redimension y no miente sobre lo que va a pasar. Un
cursor de giro de verdad necesita un recurso propio y no entra aqui."
```

---

## Tarea 11: La ventana del editor

El hito. Al acabar esta tarea hay un editor que se abre, dibuja, selecciona,
redimensiona, gira y deshace.

**Ficheros:**
- Crear: `apps/pixpin/src/ventana_editor.rs`
- Modificar: `apps/pixpin/src/main.rs` — declarar el módulo y abrirlo
- Prueba: en `ventana_editor.rs`, `mod pruebas` (solo lo puro; lo demás,
  `#[ignore]` como manda la convención del proyecto para lo que necesita
  escritorio)

**Interfaces:**
- Consume: `Gesto`, `Escena`, `Cache`, `Rejilla`, `Seleccion`, `Tiradores`,
  `Camara` del motor; `VentanaOverlay`, `EventoOverlay`, `FormaCursorWin` de
  `pixpin-shell`; `MotorRender`, `Pintor` de `pixpin-render`.
- Produce:
  ```rust
  pub fn abrir(escena: Escena) -> anyhow::Result<Escena>;
  pub fn forma_de(cursor: FormaCursor) -> FormaCursorWin;
  pub fn a_evento(ev: &EventoOverlay, camara: &Camara) -> Option<EventoGesto>;
  ```

### El reparto

El fichero de la ventana **traduce y nada más**: convierte `EventoOverlay` en
`EventoGesto`, llama a la máquina, y hace lo que devuelve. Si crece más allá
de eso, es que se ha colado lógica que debería estar en el motor.

- [ ] **Paso 1: Escribe las pruebas que fallan**

Lo que se puede probar sin escritorio es justo la traducción, que es todo lo
que este fichero debería tener de propio:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_geom::Tirador;
    use pixpin_motor2d::camara::Camara;
    use std::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn el_cursor_de_escalar_elige_la_flecha_por_su_direccion() {
        // Windows solo tiene cuatro flechas de redimension. La direccion
        // del tirador, ya girada con el elemento, se reparte entre ellas.
        let se_recto = FormaCursor::Escalar {
            tirador: Tirador::SuresteEsquina,
            angulo: 0.0,
        };
        assert_eq!(forma_de(se_recto), FormaCursorWin::RedimNoSe);

        // El mismo tirador con el elemento girado un cuarto de vuelta
        // apunta a la otra diagonal.
        let se_girado = FormaCursor::Escalar {
            tirador: Tirador::SuresteEsquina,
            angulo: FRAC_PI_2,
        };
        assert_eq!(forma_de(se_girado), FormaCursorWin::RedimNeSo);
    }

    #[test]
    fn el_tirador_del_norte_es_la_flecha_vertical() {
        let n = FormaCursor::Escalar { tirador: Tirador::NorteBorde, angulo: 0.0 };
        assert_eq!(forma_de(n), FormaCursorWin::RedimNS);

        // Girado noventa grados, el borde norte apunta al este.
        let n = FormaCursor::Escalar { tirador: Tirador::NorteBorde, angulo: FRAC_PI_2 };
        assert_eq!(forma_de(n), FormaCursorWin::RedimEO);
    }

    #[test]
    fn media_vuelta_da_la_misma_flecha() {
        // Una flecha de redimension no tiene punta: norte y sur son la
        // misma. Sin esto, girar 180 grados cambiaria el cursor sin motivo.
        let n = FormaCursor::Escalar { tirador: Tirador::NorteBorde, angulo: 0.0 };
        let s = FormaCursor::Escalar { tirador: Tirador::SurBorde, angulo: 0.0 };
        assert_eq!(forma_de(n), forma_de(s));

        let girado = FormaCursor::Escalar { tirador: Tirador::NorteBorde, angulo: PI };
        assert_eq!(forma_de(n), forma_de(girado));
    }

    #[test]
    fn los_demas_cursores_se_traducen_uno_a_uno() {
        assert_eq!(forma_de(FormaCursor::Flecha), FormaCursorWin::Flecha);
        assert_eq!(forma_de(FormaCursor::Cruz), FormaCursorWin::Cruz);
        assert_eq!(forma_de(FormaCursor::Mover), FormaCursorWin::Mover);
        assert_eq!(forma_de(FormaCursor::Texto), FormaCursorWin::Texto);
        assert_eq!(forma_de(FormaCursor::Giro), FormaCursorWin::Giro);
    }

    #[test]
    fn el_raton_llega_al_motor_en_coordenadas_del_mundo() {
        // El motor trabaja en el mundo; la ventana recibe pixeles. Si esta
        // traduccion se olvidara, dibujar con el lienzo desplazado pintaria
        // en otro sitio.
        let camara = Camara { x: 100.0, y: 50.0, zoom: 2.0 };
        let ev = EventoOverlay::BotonPulsado(Punto { x: 20, y: 10 });

        let Some(EventoGesto::Pulsar { p, .. }) = a_evento(&ev, &camara) else {
            panic!("un boton pulsado es un Pulsar");
        };
        assert_eq!(p, camara.en_mundo(Punto2::nuevo(20.0, 10.0)));
    }

    #[test]
    fn control_zeta_es_deshacer_y_control_i_griega_rehacer() {
        let camara = Camara::nueva();
        let ctrl = |vk: u32| EventoOverlay::Tecla { vk, shift: false, ctrl: true, alt: false };

        assert_eq!(a_evento(&ctrl(b'Z' as u32), &camara), Some(EventoGesto::Deshacer));
        assert_eq!(a_evento(&ctrl(b'Y' as u32), &camara), Some(EventoGesto::Rehacer));
        assert_eq!(a_evento(&ctrl(b'A' as u32), &camara), Some(EventoGesto::SeleccionarTodo));
    }

    #[test]
    fn la_zeta_sin_control_no_deshace() {
        // Escribir una zeta en un texto no puede deshacer el dibujo.
        let camara = Camara::nueva();
        let sola = EventoOverlay::Tecla { vk: b'Z' as u32, shift: false, ctrl: false, alt: false };
        assert_ne!(a_evento(&sola, &camara), Some(EventoGesto::Deshacer));
    }

    #[test]
    fn lo_que_no_le_toca_al_motor_no_llega_al_motor() {
        let camara = Camara::nueva();
        assert_eq!(a_evento(&EventoOverlay::Pintar, &camara), None);
        assert_eq!(a_evento(&EventoOverlay::CambioDpi, &camara), None);
    }
}
```

Ajusta `Camara { x, y, zoom }` y `Camara::nueva()` a los nombres reales de
`camara.rs` — míralos antes de escribir la prueba.

- [ ] **Paso 2: Comprueba que fallan**

```
cargo test -p pixpin ventana_editor -- --test-threads=1
```

- [ ] **Paso 3: Escribe la traducción**

```rust
//! La ventana del editor avanzado.
//!
//! **Traduce y nada mas.** Convierte `EventoOverlay` en `EventoGesto`, llama
//! a la maquina del motor, y hace lo que devuelve: redibujar esta region,
//! poner este cursor. Si este fichero crece mas alla de eso, es que se ha
//! colado logica que deberia estar en el motor.
//!
//! No hace falta fonteneria nueva: `VentanaOverlay` ya sirve ventanas
//! completas —el editor de grabaciones la usa asi— y ya trae raton, teclas
//! con sus modificadores, caracteres con IME, cambio de DPI, cursores y el
//! bucle por eventos que da el 0 % de CPU en reposo.

use pixpin_geom::Tirador;
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::gesto::{EventoGesto, FormaCursor, direccion_del_tirador};
use pixpin_motor2d::vector::Punto2;
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin};

/// De la forma que pide el motor a la que entiende Windows.
///
/// Lo unico con sustancia es escalar: Windows solo trae cuatro flechas de
/// redimension, asi que la direccion del tirador —ya girada con el
/// elemento— se reparte entre ellas en cuartos de vuelta partidos por la
/// mitad. Y como una flecha no tiene punta, norte y sur son la misma: se
/// toma el angulo modulo media vuelta.
pub fn forma_de(cursor: FormaCursor) -> FormaCursorWin {
    use std::f32::consts::PI;
    match cursor {
        FormaCursor::Flecha => FormaCursorWin::Flecha,
        FormaCursor::Cruz => FormaCursorWin::Cruz,
        FormaCursor::Mover => FormaCursorWin::Mover,
        FormaCursor::Texto => FormaCursorWin::Texto,
        FormaCursor::Giro => FormaCursorWin::Giro,
        FormaCursor::Escalar { tirador, angulo } => {
            let d = direccion_del_tirador(tirador, angulo);
            // A media vuelta, y en octavos: cada flecha cubre 45 grados.
            let media = PI;
            let d = d.rem_euclid(media);
            let octavo = media / 4.0;
            match (d / octavo).round() as i32 % 4 {
                0 => FormaCursorWin::RedimNS,
                1 => FormaCursorWin::RedimNeSo,
                2 => FormaCursorWin::RedimEO,
                _ => FormaCursorWin::RedimNoSe,
            }
        }
    }
}

/// Del evento de la ventana al del motor. `None` es «esto no le toca al
/// motor»: pintar, el DPI, el despertar de otro hilo.
pub fn a_evento(ev: &EventoOverlay, camara: &Camara) -> Option<EventoGesto> {
    let al_mundo = |p: &pixpin_geom::Punto| {
        camara.en_mundo(Punto2::nuevo(p.x as f32, p.y as f32))
    };
    match ev {
        EventoOverlay::BotonPulsado(p) => Some(EventoGesto::Pulsar {
            p: al_mundo(p),
            shift: false,
            alt: false,
        }),
        EventoOverlay::RatonMovido(p) => Some(EventoGesto::Mover {
            p: al_mundo(p),
            shift: false,
            alt: false,
        }),
        EventoOverlay::BotonSoltado(p) => Some(EventoGesto::Soltar { p: al_mundo(p) }),
        EventoOverlay::Tecla { vk, ctrl, .. } => {
            const VK_ESCAPE: u32 = 0x1B;
            const VK_DELETE: u32 = 0x2E;
            match (*vk, *ctrl) {
                (VK_ESCAPE, _) => Some(EventoGesto::Escape),
                (VK_DELETE, _) => Some(EventoGesto::Suprimir),
                (v, true) if v == b'Z' as u32 => Some(EventoGesto::Deshacer),
                (v, true) if v == b'Y' as u32 => Some(EventoGesto::Rehacer),
                (v, true) if v == b'A' as u32 => Some(EventoGesto::SeleccionarTodo),
                _ => None,
            }
        }
        _ => None,
    }
}
```

**Ojo con `shift` y `alt` en el ratón:** `EventoOverlay::BotonPulsado` y
`RatonMovido` no los traen. La ventana los lee con `GetKeyState` en el momento
de traducir, igual que hace `overlay.rs` con las teclas. Es la misma llamada y
la misma justificación: preguntar por el estado de una tecla no tiene
precondiciones. Deja `a_evento` como está —puro y comprobable— y que quien la
llama sobrescriba los dos campos con lo que acaba de leer.

- [ ] **Paso 4: Escribe el bucle de la ventana**

Copia la forma de `apps/pixpin/src/editor.rs` (el editor de grabaciones), que
ya hace exactamente esto con `VentanaOverlay`. La estructura:

```rust
pub fn abrir(escena: Escena) -> anyhow::Result<Escena> {
    let mut escena = escena;
    let mut gesto = Gesto::nuevo();
    let mut camara = Camara::nueva();
    let mut cache = Cache::nueva();
    let mut rejilla = Rejilla::nueva();

    let ventana = VentanaOverlay::nueva(area_de_trabajo())?;
    let mut motor = MotorRender::nuevo(ventana.handle())?;

    ventana.ejecutar(|ev| {
        // 1. Traducir y, si le toca al motor, pasarselo.
        if let Some(g) = a_evento(&ev, &camara) {
            let g = con_modificadores(g);           // GetKeyState de Shift y Alt
            let r = gesto.evento(g, &mut escena, 1.0 / camara.zoom);
            ventana.poner_cursor(forma_de(r.cursor));
            match r.region {
                Region::Nada => {}
                Region::Caja(x0, y0, x1, y1) => {
                    ventana.invalidar(camara.en_pantalla_rect(x0, y0, x1, y1));
                }
                Region::Todo => ventana.invalidar_todo(),
            }
        }
        // 2. Pintar solo cuando lo pide la ventana.
        if matches!(ev, EventoOverlay::Pintar) {
            rejilla.sincronizar(&escena);
            pintar(&mut motor, &escena, &camara, &gesto, &mut cache, &rejilla);
        }
        if matches!(ev, EventoOverlay::Cerrar) {
            return Continuar::No;
        }
        Continuar::Si
    });

    escena.compactar();
    Ok(escena)
}
```

Y `pintar`, que es donde se junta todo lo de las tareas 7 y 8:

```rust
fn pintar(
    motor: &mut MotorRender,
    escena: &Escena,
    camara: &Camara,
    gesto: &Gesto,
    cache: &mut Cache,
    rejilla: &Rejilla,
) {
    // La rejilla dice que PUEDE verse; la camara filtra lo que de verdad se
    // ve. Sin la rejilla, esto recorreria los ocho mil elementos.
    let vista = camara.caja_visible();
    let candidatos = rejilla.candidatos(vista);

    let mut pintor = motor.empezar();
    pintor.transformar(camara.matriz());
    for id in candidatos {
        let Some(e) = escena.buscar(id) else { continue };
        if e.borrado {
            continue;
        }
        for orden in cache.ordenes(e, camara.zoom) {
            pintor.orden(orden);
        }
    }
    // Encima de todo: el marco de la seleccion, los tiradores y la
    // marquesina si la hay.
    let escala = 1.0 / camara.zoom;
    if let Some(caja) = gesto.seleccion.caja(escena) {
        pintor.marco(caja, escala);
        for orden in Tiradores::de_caja(caja, 0.0, escala).ordenes(escala) {
            pintor.orden(&orden);
        }
    }
    if let Some(m) = gesto.marquesina() {
        pintor.marquesina(m, escala);
    }
    pintor.terminar();
}
```

Los nombres exactos de `Pintor` (`transformar`, `orden`, `marco`,
`marquesina`, `empezar`, `terminar`) **no existen todavía tal cual**: mira
`crates/pixpin-render/src/lienzo.rs` y usa los que haya, o añade los que
falten siguiendo su estilo. Es el único sitio de esta tarea donde hay que
inventar API, y debe quedarse en `pixpin-render`, no aquí.

- [ ] **Paso 5: Ábrelo desde algún sitio**

En `apps/pixpin/src/main.rs`, declara `mod ventana_editor;` y añade la entrada
que lo abre. Lo mínimo para poder probarlo a mano: una opción en el menú de la
bandeja, «Editor», que llame a `ventana_editor::abrir(Escena::nueva())`.

Abrir un `.pixpin` desde ahí es la tarea siguiente del plan maestro, no ésta.

- [ ] **Paso 6: Pruébalo a mano**

Esto no lo cubre ninguna prueba automática y es lo que de verdad dice si la
tarea está hecha:

```
cargo run --release
```

1. Abre el editor desde la bandeja.
2. Dibuja tres trazos. `Ctrl+Z` tres veces: desaparecen de uno en uno.
3. `Ctrl+Y` tres veces: vuelven.
4. Cambia a la mano. Arrastra en vacío: aparece la marquesina y coge lo que
   queda entero dentro.
5. Con dos elementos elegidos, tira de una esquina: escalan los dos y la
   esquina de enfrente no se mueve.
6. Gira con el tirador de arriba. Con `Shift`, a saltos.
7. A mitad de un arrastre, `Escape`: vuelve a su sitio.
8. Un solo `Ctrl+Z` deshace el arrastre entero, no cada píxel.

Si alguno falla, es un fallo del motor y no de la ventana: vuelve a la tarea
correspondiente y añade la prueba que faltaba.

- [ ] **Paso 7: Cierra**

```
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

- [ ] **Paso 8: Commit**

```bash
git add apps/pixpin/src/ventana_editor.rs apps/pixpin/src/main.rs crates/pixpin-render/src/lienzo.rs
git commit -m "La ventana del editor: el hito

Ya se abre, dibuja, selecciona, redimensiona, gira y deshace.

No hizo falta fonteneria nueva. VentanaOverlay ya servia ventanas
completas —el editor de grabaciones la usa asi— y ya traia raton, teclas
con modificadores, caracteres con IME, cambio de DPI, cursores y el bucle
por eventos que da el 0 % de CPU en reposo. Lo unico que le faltaba eran
el campo alt y un cursor de giro, que fue la tarea anterior.

Este fichero traduce y nada mas: EventoOverlay a EventoGesto, llamar a la
maquina, y hacer lo que devuelve. Si crece mas alla de eso es que se ha
colado logica que deberia estar en el motor.

Lo unico con sustancia propia es el cursor de escalar. Windows solo trae
cuatro flechas de redimension, asi que la direccion del tirador —ya
girada con el elemento— se reparte entre ellas en octavos de vuelta. Y
como una flecha no tiene punta, norte y sur son la misma: el angulo va
modulo media vuelta, o girar 180 grados cambiaria el cursor sin motivo."
```

---

## Tarea 12: Organizar — orden, grupos, alinear, distribuir

Puerto de `Organize.kt` (310 líneas, puras). La tarea 3 trajo el campo
`grupos`; ésta lo hace servir para algo.

**Ficheros:**
- Crear: `crates/pixpin-motor2d/src/organizar.rs`
- Modificar: `crates/pixpin-motor2d/src/lib.rs`
- Prueba: en `organizar.rs`, `mod pruebas`

**Interfaces:**
- Consume: `Escena`, `Seleccion`, `Elemento`.
- Produce:
  ```rust
  pub enum Alineacion { Izquierda, CentroHorizontal, Derecha,
                        Arriba, CentroVertical, Abajo }
  pub enum Reparto { Horizontal, Vertical }

  pub fn agrupar(escena: &mut Escena, sel: &Seleccion) -> Option<String>;
  pub fn desagrupar(escena: &mut Escena, sel: &Seleccion);
  pub fn hermanos_de(escena: &Escena, id: u64) -> Vec<u64>;
  pub fn al_frente(escena: &mut Escena, sel: &Seleccion);
  pub fn al_fondo(escena: &mut Escena, sel: &Seleccion);
  pub fn alinear(escena: &mut Escena, sel: &Seleccion, como: Alineacion);
  pub fn repartir(escena: &mut Escena, sel: &Seleccion, como: Reparto);
  ```

- [ ] **Paso 1: Escribe las pruebas que fallan**

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};

    fn rect(x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id: 0, figura: Figura::Rectangulo, x, y, ancho, alto, angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0), relleno: None, grosor: 0.0,
            estilo: EstiloTrazo::Solido, rugosidad: 1.0, opacidad: 1.0,
            semilla: 1, version: 0, borrado: false, grupos: Vec::new(),
        }
    }

    fn con_tres() -> (Escena, u64, u64, u64) {
        let mut e = Escena::nueva();
        let a = e.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = e.anadir(rect(50.0, 30.0, 20.0, 20.0));
        let c = e.anadir(rect(100.0, 60.0, 10.0, 40.0));
        (e, a, b, c)
    }

    #[test]
    fn agrupar_pone_el_mismo_grupo_a_todos_y_desagrupar_lo_quita() {
        let (mut escena, a, b, _) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b]);

        let grupo = agrupar(&mut escena, &sel).unwrap();
        assert!(escena.buscar(a).unwrap().grupos.contains(&grupo));
        assert!(escena.buscar(b).unwrap().grupos.contains(&grupo));

        desagrupar(&mut escena, &sel);
        assert!(escena.buscar(a).unwrap().grupos.is_empty());
    }

    #[test]
    fn agrupar_uno_solo_no_crea_grupo() {
        // Un grupo de uno no es un grupo: seria basura en el fichero que
        // ademas viaja al movil.
        let (mut escena, a, _, _) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner(a);
        assert!(agrupar(&mut escena, &sel).is_none());
        assert!(escena.buscar(a).unwrap().grupos.is_empty());
    }

    #[test]
    fn desagrupar_solo_quita_el_grupo_de_dentro_y_respeta_los_de_fuera() {
        // Un elemento puede estar en varios grupos anidados, como en
        // Excalidraw. Desagrupar deshace el mas interno, no todos.
        let (mut escena, a, b, _) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b]);
        let fuera = agrupar(&mut escena, &sel).unwrap();
        let dentro = agrupar(&mut escena, &sel).unwrap();

        desagrupar(&mut escena, &sel);
        let g = &escena.buscar(a).unwrap().grupos;
        assert!(g.contains(&fuera), "el de fuera sigue");
        assert!(!g.contains(&dentro), "el de dentro se fue");
    }

    #[test]
    fn tocar_uno_de_un_grupo_los_trae_a_todos() {
        let (mut escena, a, b, c) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b]);
        agrupar(&mut escena, &sel);

        let hermanos = hermanos_de(&escena, a);
        assert!(hermanos.contains(&a) && hermanos.contains(&b));
        assert!(!hermanos.contains(&c));
    }

    #[test]
    fn un_elemento_sin_grupo_es_hermano_de_si_mismo_y_de_nadie_mas() {
        let (escena, a, _, _) = con_tres();
        assert_eq!(hermanos_de(&escena, a), vec![a]);
    }

    #[test]
    fn al_frente_y_al_fondo_cambian_el_orden_de_pintado() {
        // El orden de la lista ES el orden de pintado: el ultimo va encima.
        let (mut escena, a, _, _) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner(a);

        al_frente(&mut escena, &sel);
        assert_eq!(escena.elementos.last().unwrap().id, a);

        al_fondo(&mut escena, &sel);
        assert_eq!(escena.elementos.first().unwrap().id, a);
    }

    #[test]
    fn al_frente_conserva_el_orden_relativo_de_lo_que_sube() {
        let (mut escena, a, b, _) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b]);
        al_frente(&mut escena, &sel);

        let ids: Vec<u64> = escena.elementos.iter().map(|e| e.id).collect();
        let ia = ids.iter().position(|x| *x == a).unwrap();
        let ib = ids.iter().position(|x| *x == b).unwrap();
        assert!(ia < ib, "a estaba antes que b y lo sigue estando");
    }

    #[test]
    fn alinear_a_la_izquierda_los_lleva_al_borde_del_mas_a_la_izquierda() {
        let (mut escena, a, b, c) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b, c]);

        alinear(&mut escena, &sel, Alineacion::Izquierda);
        for id in [a, b, c] {
            assert_eq!(escena.buscar(id).unwrap().caja().0, 0.0);
        }
    }

    #[test]
    fn alinear_al_centro_usa_el_centro_del_conjunto() {
        let (mut escena, a, b, c) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b, c]);
        let centro = sel.centro(&escena).unwrap();

        alinear(&mut escena, &sel, Alineacion::CentroVertical);
        for id in [a, b, c] {
            let (_, y0, _, y1) = escena.buscar(id).unwrap().caja();
            assert!(((y0 + y1) / 2.0 - centro.y).abs() < 1e-3);
        }
    }

    #[test]
    fn repartir_deja_los_huecos_iguales() {
        let (mut escena, a, b, c) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b, c]);

        repartir(&mut escena, &sel, Reparto::Horizontal);

        let mut centros: Vec<f32> = [a, b, c]
            .iter()
            .map(|id| {
                let (x0, _, x1, _) = escena.buscar(*id).unwrap().caja();
                (x0 + x1) / 2.0
            })
            .collect();
        centros.sort_by(|p, q| p.partial_cmp(q).unwrap());
        let h1 = centros[1] - centros[0];
        let h2 = centros[2] - centros[1];
        assert!((h1 - h2).abs() < 1e-3, "huecos {h1} y {h2}");
    }

    #[test]
    fn repartir_no_mueve_los_dos_de_los_extremos() {
        // Repartir coloca lo de en medio; mover los extremos cambiaria el
        // sitio del conjunto, que no es lo que nadie espera.
        let (mut escena, a, b, c) = con_tres();
        let (x_a, x_c) = (
            escena.buscar(a).unwrap().caja().0,
            escena.buscar(c).unwrap().caja().0,
        );
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b, c]);

        repartir(&mut escena, &sel, Reparto::Horizontal);

        assert_eq!(escena.buscar(a).unwrap().caja().0, x_a);
        assert_eq!(escena.buscar(c).unwrap().caja().0, x_c);
    }

    #[test]
    fn alinear_y_repartir_dejan_un_solo_paso_de_deshacer() {
        let (mut escena, a, b, c) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b, c]);
        let antes = escena.buscar(b).unwrap().x;

        alinear(&mut escena, &sel, Alineacion::Izquierda);
        assert!(escena.deshacer());
        assert_eq!(escena.buscar(b).unwrap().x, antes, "los tres de una vez");
    }

    #[test]
    fn con_menos_de_tres_repartir_no_hace_nada() {
        let (mut escena, a, b, _) = con_tres();
        let antes = escena.buscar(b).unwrap().x;
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b]);

        repartir(&mut escena, &sel, Reparto::Horizontal);
        assert_eq!(escena.buscar(b).unwrap().x, antes, "no hay nada en medio");
    }
}
```

- [ ] **Paso 2: Comprueba que fallan**

```
cargo test -p pixpin-motor2d organizar -- --test-threads=1
```

- [ ] **Paso 3: Escribe la implementación**

```rust
//! Orden de pintado, grupos, alinear y repartir. Puerto de `Organize.kt`.
//!
//! # Los grupos son una lista y no un identificador
//!
//! `Elemento::grupos` es un `Vec<String>` porque un elemento puede estar en
//! varios grupos anidados, como en Excalidraw: el ultimo de la lista es el
//! mas interno. Desagrupar deshace **solo el mas interno**, que es lo que
//! espera quien agrupo dos veces.
//!
//! # Todo lo de aqui es un solo paso de deshacer
//!
//! Alinear cinco elementos es un `Ctrl+Z`, no cinco. Cada funcion publica
//! abre y cierra su paso.

use crate::escena::Escena;
use crate::seleccion::Seleccion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alineacion {
    Izquierda,
    CentroHorizontal,
    Derecha,
    Arriba,
    CentroVertical,
    Abajo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reparto {
    Horizontal,
    Vertical,
}

/// Mete lo elegido en un grupo nuevo. `None` si no hay al menos dos: un
/// grupo de uno no es un grupo, y ademas viajaria al movil como basura.
pub fn agrupar(escena: &mut Escena, sel: &Seleccion) -> Option<String> {
    if sel.cuantos() < 2 {
        return None;
    }
    // El nombre sale del contador de la escena, que ya garantiza que no se
    // repite dentro del documento. Con prefijo para no confundirlo con un
    // id de elemento al mirar el JSON a ojo.
    let grupo = format!("g{}", escena.siguiente_id);
    escena.siguiente_id += 1;

    escena.abrir_paso();
    for id in sel.ids() {
        escena.apuntar_edicion(*id);
        if let Some(e) = escena.buscar_mut(*id) {
            e.grupos.push(grupo.clone());
            e.tocar();
        }
    }
    escena.cerrar_paso();
    Some(grupo)
}

/// Deshace el grupo mas interno de lo elegido.
pub fn desagrupar(escena: &mut Escena, sel: &Seleccion) {
    escena.abrir_paso();
    for id in sel.ids() {
        escena.apuntar_edicion(*id);
        if let Some(e) = escena.buscar_mut(*id) {
            e.grupos.pop();
            e.tocar();
        }
    }
    escena.cerrar_paso();
}

/// Todos los que comparten el grupo mas interno de este. Si no tiene
/// grupo, solo el.
///
/// Es lo que convierte «pinche un elemento» en «cogi el grupo entero».
pub fn hermanos_de(escena: &Escena, id: u64) -> Vec<u64> {
    let Some(e) = escena.buscar(id) else { return Vec::new() };
    let Some(grupo) = e.grupos.last() else { return vec![id] };
    escena
        .visibles()
        .filter(|o| o.grupos.last() == Some(grupo))
        .map(|o| o.id)
        .collect()
}

/// Sube lo elegido al frente, conservando su orden relativo.
pub fn al_frente(escena: &mut Escena, sel: &Seleccion) {
    reordenar(escena, sel, true);
}

/// Baja lo elegido al fondo, conservando su orden relativo.
pub fn al_fondo(escena: &mut Escena, sel: &Seleccion) {
    reordenar(escena, sel, false);
}

fn reordenar(escena: &mut Escena, sel: &Seleccion, al_frente: bool) {
    // Particionar conserva el orden dentro de cada mitad, que es justo lo
    // que hace falta: subir dos elementos no debe intercambiarlos.
    let (movidos, quietos): (Vec<_>, Vec<_>) = escena
        .elementos
        .drain(..)
        .partition(|e| sel.contiene(e.id));
    escena.elementos = if al_frente {
        quietos.into_iter().chain(movidos).collect()
    } else {
        movidos.into_iter().chain(quietos).collect()
    };
}

/// Alinea lo elegido contra el borde o el centro del conjunto.
pub fn alinear(escena: &mut Escena, sel: &Seleccion, como: Alineacion) {
    let Some((cx0, cy0, cx1, cy1)) = sel.caja(escena) else { return };
    escena.abrir_paso();
    for id in sel.ids().to_vec() {
        let Some(e) = escena.buscar(id) else { continue };
        let (x0, y0, x1, y1) = e.caja();
        let (dx, dy) = match como {
            Alineacion::Izquierda => (cx0 - x0, 0.0),
            Alineacion::Derecha => (cx1 - x1, 0.0),
            Alineacion::CentroHorizontal => ((cx0 + cx1) / 2.0 - (x0 + x1) / 2.0, 0.0),
            Alineacion::Arriba => (0.0, cy0 - y0),
            Alineacion::Abajo => (0.0, cy1 - y1),
            Alineacion::CentroVertical => (0.0, (cy0 + cy1) / 2.0 - (y0 + y1) / 2.0),
        };
        if dx == 0.0 && dy == 0.0 {
            continue;
        }
        escena.apuntar_edicion(id);
        if let Some(e) = escena.buscar_mut(id) {
            e.mover(dx, dy);
        }
    }
    escena.cerrar_paso();
}

/// Deja huecos iguales entre los centros, sin mover los dos extremos.
///
/// No mover los extremos es lo que hace que repartir no cambie el sitio del
/// conjunto: se coloca lo de en medio, que es lo que se pide.
pub fn repartir(escena: &mut Escena, sel: &Seleccion, como: Reparto) {
    if sel.cuantos() < 3 {
        return;
    }
    let centro_de = |escena: &Escena, id: u64| -> Option<f32> {
        let (x0, y0, x1, y1) = escena.buscar(id)?.caja();
        Some(match como {
            Reparto::Horizontal => (x0 + x1) / 2.0,
            Reparto::Vertical => (y0 + y1) / 2.0,
        })
    };

    let mut orden: Vec<(u64, f32)> = sel
        .ids()
        .iter()
        .filter_map(|id| centro_de(escena, *id).map(|c| (*id, c)))
        .collect();
    orden.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let (primero, ultimo) = (orden[0].1, orden[orden.len() - 1].1);
    let hueco = (ultimo - primero) / (orden.len() - 1) as f32;

    escena.abrir_paso();
    for (i, (id, actual)) in orden.iter().enumerate().skip(1).take(orden.len() - 2) {
        let quiero = primero + hueco * i as f32;
        let d = quiero - actual;
        if d == 0.0 {
            continue;
        }
        escena.apuntar_edicion(*id);
        if let Some(e) = escena.buscar_mut(*id) {
            match como {
                Reparto::Horizontal => e.mover(d, 0.0),
                Reparto::Vertical => e.mover(0.0, d),
            }
        }
    }
    escena.cerrar_paso();
}
```

`siguiente_id` es `pub` en `Escena`, así que `agrupar` puede usarlo. Si te
chirría que un contador de elementos nombre grupos, la alternativa es un
contador propio — pero entonces hay que guardarlo en el fichero, y eso es un
campo más en el formato por muy poca cosa.

- [ ] **Paso 4: Cierra y commit**

```
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

```bash
git add crates/pixpin-motor2d/src/organizar.rs crates/pixpin-motor2d/src/lib.rs
git commit -m "Organizar: orden de pintado, grupos, alinear y repartir

Puerto de Organize.kt. La tarea de los grupos trajo el campo; esta lo
hace servir para algo: tocar un elemento de un grupo los trae a todos.

grupos es una lista y no un identificador porque un elemento puede estar
en varios grupos anidados, como en Excalidraw: el ultimo es el mas
interno, y desagrupar deshace solo ese. Agrupar uno solo no hace nada, un
grupo de uno no es un grupo y viajaria al movil como basura.

Repartir no mueve los dos extremos: coloca lo de en medio. Mover los
extremos cambiaria el sitio del conjunto, que no es lo que nadie espera
al pulsar «repartir».

Y todo lo de aqui es un solo paso de deshacer. Alinear cinco elementos es
un Ctrl+Z, no cinco."
```

---

## Tarea 13: El panel de propiedades

Lo que el Android resuelve en `PanelLateral.kt` con 2.185 líneas. Aquí, mucho
menos, por una decisión: **el panel enseña solo lo que tiene sentido para lo
seleccionado.**

**Ficheros:**
- Crear: `crates/pixpin-ui/src/propiedades.rs`
- Modificar: `crates/pixpin-ui/src/lib.rs`
- Modificar: `apps/pixpin/src/ventana_editor.rs` — pintarlo
- Prueba: en `propiedades.rs`, `mod pruebas`

**Interfaces:**
- Consume: `Elemento`, `Figura`, `Herramienta` del motor.
- Produce:
  ```rust
  pub enum Propiedad { ColorTrazo, Relleno, Grosor, Estilo, Rugosidad,
                       Opacidad, Fuente, TamanoTexto, PuntaFlecha }

  pub fn de_figura(f: &Figura) -> &'static [Propiedad];
  pub fn de_herramienta(h: Herramienta) -> &'static [Propiedad];
  pub fn comunes(elementos: &[&Elemento]) -> Vec<Propiedad>;
  ```

- [ ] **Paso 1: Escribe las pruebas que fallan**

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::elemento::Figura;

    #[test]
    fn un_trazo_a_mano_no_tiene_relleno() {
        // Una mancha de tinta no tiene interior que rellenar. Ensenar el
        // control en gris es ruido, y en un portatil viejo es ademas
        // espacio robado al lienzo.
        let p = de_figura(&Figura::Lapiz { puntos: Vec::new(), presiones: Vec::new() });
        assert!(!p.contains(&Propiedad::Relleno));
        assert!(p.contains(&Propiedad::Grosor));
    }

    #[test]
    fn un_texto_tiene_fuente_y_no_rugosidad() {
        let p = de_figura(&Figura::Texto {
            texto: String::new(),
            tam: 16.0,
            familia: "Segoe UI".to_string(),
        });
        assert!(p.contains(&Propiedad::Fuente));
        assert!(p.contains(&Propiedad::TamanoTexto));
        assert!(!p.contains(&Propiedad::Rugosidad), "las letras no son a mano");
    }

    #[test]
    fn un_rectangulo_si_tiene_relleno_y_rugosidad() {
        let p = de_figura(&Figura::Rectangulo);
        assert!(p.contains(&Propiedad::Relleno));
        assert!(p.contains(&Propiedad::Rugosidad));
    }

    #[test]
    fn una_flecha_tiene_puntas() {
        let p = de_figura(&Figura::Flecha {
            puntos: Vec::new(),
            punta_inicio: false,
            punta_fin: true,
        });
        assert!(p.contains(&Propiedad::PuntaFlecha));
    }

    #[test]
    fn el_resaltador_no_tiene_rugosidad_ni_estilo() {
        // Es grueso, translucido y liso a proposito: resaltar sobre texto
        // tiene que dejarlo legible.
        let p = de_figura(&Figura::Resaltador { puntos: Vec::new() });
        assert!(!p.contains(&Propiedad::Rugosidad));
        assert!(!p.contains(&Propiedad::Estilo));
    }

    #[test]
    fn con_varios_elegidos_solo_salen_las_propiedades_comunes() {
        // Un texto y un rectangulo comparten color y opacidad, nada mas.
        // Ensenar «rugosidad» con un texto elegido cambiaria algo que el
        // usuario no ve.
        let texto = elem(Figura::Texto {
            texto: String::new(),
            tam: 16.0,
            familia: "Segoe UI".to_string(),
        });
        let rect = elem(Figura::Rectangulo);
        let p = comunes(&[&texto, &rect]);

        assert!(p.contains(&Propiedad::ColorTrazo));
        assert!(p.contains(&Propiedad::Opacidad));
        assert!(!p.contains(&Propiedad::Rugosidad));
        assert!(!p.contains(&Propiedad::Fuente));
    }

    #[test]
    fn sin_nada_elegido_no_hay_panel() {
        assert!(comunes(&[]).is_empty());
    }

    #[test]
    fn las_comunes_salen_en_el_mismo_orden_siempre() {
        // El orden es el del enum. Si dependiera del orden de la
        // seleccion, los controles bailarian de sitio al elegir en
        // distinto orden, que es de las cosas mas molestas que puede hacer
        // una interfaz.
        let a = elem(Figura::Rectangulo);
        let b = elem(Figura::Elipse);
        assert_eq!(comunes(&[&a, &b]), comunes(&[&b, &a]));
    }

    #[test]
    fn la_herramienta_manda_cuando_no_hay_nada_elegido() {
        // Antes de dibujar tambien se eligen color y grosor.
        let p = de_herramienta(Herramienta::Lapiz);
        assert!(p.contains(&Propiedad::Grosor));
        assert!(!p.contains(&Propiedad::Relleno));
    }

    #[test]
    fn la_mano_y_la_lupa_no_ajustan_nada() {
        assert!(de_herramienta(Herramienta::Mano).is_empty());
        assert!(de_herramienta(Herramienta::Lupa).is_empty());
    }
}
```

Añade el ayudante `fn elem(figura: Figura) -> Elemento` construyendo un
`Elemento` como en las tareas anteriores.

- [ ] **Paso 2: Escribe la implementación**

La tabla es lo único con sustancia, y sale de `DrawProperties.kt`:

```rust
//! Que se puede ajustar de cada cosa.
//!
//! El panel ensena **solo lo que tiene sentido para lo seleccionado**: con
//! un trazo a mano no aparece «relleno», con un texto aparece la fuente y no
//! la rugosidad. Ensenar el control en gris es ruido, y en la pantalla de un
//! portatil viejo es ademas espacio robado al lienzo.
//!
//! El Android resuelve esto en `PanelLateral.kt` con 2.185 lineas; la tabla
//! en si esta en `DrawProperties.kt` (332 lineas, puras) y es lo que se
//! porta. Todo lo demas de aquellas 2.185 es Compose.

use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::gesto::Herramienta;

/// El orden de este enum es el orden en que salen los controles. No es
/// casual: si dependiera del orden de la seleccion, los controles bailarian
/// de sitio al elegir en distinto orden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Propiedad {
    ColorTrazo,
    Relleno,
    Grosor,
    Estilo,
    Rugosidad,
    Opacidad,
    Fuente,
    TamanoTexto,
    PuntaFlecha,
}

use Propiedad::*;

pub fn de_figura(f: &Figura) -> &'static [Propiedad] {
    match f {
        Figura::Lapiz { .. } => &[ColorTrazo, Grosor, Opacidad],
        // Grueso, translucido y liso a proposito (D45): resaltar sobre
        // texto tiene que dejarlo legible.
        Figura::Resaltador { .. } => &[ColorTrazo, Grosor, Opacidad],
        Figura::Linea { .. } => &[ColorTrazo, Grosor, Estilo, Rugosidad, Opacidad],
        Figura::Flecha { .. } => {
            &[ColorTrazo, Grosor, Estilo, Rugosidad, Opacidad, PuntaFlecha]
        }
        Figura::Rectangulo | Figura::Elipse => {
            &[ColorTrazo, Relleno, Grosor, Estilo, Rugosidad, Opacidad]
        }
        Figura::Texto { .. } => &[ColorTrazo, Opacidad, Fuente, TamanoTexto],
        // El foco oscurece lo de alrededor: su color es el del velo.
        Figura::Foco { .. } => &[Opacidad],
        Figura::Imagen { .. } => &[Opacidad],
    }
}

/// Lo que se puede ajustar antes de dibujar, cuando no hay nada elegido.
pub fn de_herramienta(h: Herramienta) -> &'static [Propiedad] {
    match h {
        // No dejan rastro: no hay nada que ajustar.
        Herramienta::Mano | Herramienta::Lupa | Herramienta::Borrador => &[],
        Herramienta::Lapiz | Herramienta::Resaltador => &[ColorTrazo, Grosor, Opacidad],
        Herramienta::Linea => &[ColorTrazo, Grosor, Estilo, Rugosidad, Opacidad],
        Herramienta::Flecha => {
            &[ColorTrazo, Grosor, Estilo, Rugosidad, Opacidad, PuntaFlecha]
        }
        Herramienta::Rectangulo | Herramienta::Elipse => {
            &[ColorTrazo, Relleno, Grosor, Estilo, Rugosidad, Opacidad]
        }
        Herramienta::Texto => &[ColorTrazo, Opacidad, Fuente, TamanoTexto],
        Herramienta::Foco => &[Opacidad],
    }
}

/// Lo que se puede ajustar de todos a la vez.
///
/// Solo lo comun: ensenar «rugosidad» con un texto elegido cambiaria algo
/// que el usuario no ve cambiar.
pub fn comunes(elementos: &[&Elemento]) -> Vec<Propiedad> {
    let Some((primero, resto)) = elementos.split_first() else {
        return Vec::new();
    };
    let mut fuera: Vec<Propiedad> = de_figura(&primero.figura)
        .iter()
        .copied()
        .filter(|p| resto.iter().all(|e| de_figura(&e.figura).contains(p)))
        .collect();
    // El orden del enum, no el de la seleccion.
    fuera.sort_unstable();
    fuera
}
```

- [ ] **Paso 3: Píntalo en la ventana**

En `ventana_editor.rs`, el panel se dibuja a la derecha **solo si
`comunes(...)` no está vacío**. Con nada seleccionado, se usa
`de_herramienta(gesto.herramienta)`; si eso también está vacío —la mano, la
lupa—, no hay panel y el lienzo ocupa todo.

Sigue el estilo de `crates/pixpin-ui/src/panel.rs` (87 líneas) y de
`caja_herramientas.rs`, que ya resuelven esto para el anotador.

- [ ] **Paso 4: Cierra y commit**

```
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

```bash
git add crates/pixpin-ui/src/propiedades.rs crates/pixpin-ui/src/lib.rs apps/pixpin/src/ventana_editor.rs
git commit -m "El panel ensena solo lo que tiene sentido

Con un trazo a mano no aparece «relleno»; con un texto aparece la fuente
y no la rugosidad. Ensenar el control en gris es ruido, y en la pantalla
de un portatil viejo es ademas espacio robado al lienzo.

El Android resuelve esto en PanelLateral.kt con 2.185 lineas. La tabla en
si esta en DrawProperties.kt, 332 lineas puras, y es lo unico que se
porta: el resto es Compose.

Con varios elegidos salen solo las propiedades comunes, y en el orden del
enum y no en el de la seleccion. Si dependiera del orden de la seleccion,
los controles bailarian de sitio al elegir en distinto orden, que es de
las cosas mas molestas que puede hacer una interfaz.

Y sin nada elegido y con la mano puesta no hay panel: el lienzo ocupa
todo."
```

---

## Tarea 14: La capa estática

Mientras arrastras un elemento, los otros 7.999 no cambian. En una iGPU con
memoria compartida, esto es la diferencia entre arrastrar y **ver** arrastrar.

**Ficheros:**
- Crear: `crates/pixpin-render/src/capa_estatica.rs`
- Modificar: `crates/pixpin-render/src/lib.rs`
- Modificar: `apps/pixpin/src/ventana_editor.rs` — prepararla y soltarla
- Prueba: en `capa_estatica.rs`, `mod pruebas` (la política, que es pura)

### La decisión que simplifica todo

La tentación es cachear la capa **siempre** e ir invalidándola cuando algo
cambia. Eso obliga a saber qué ha cambiado en la escena en cada fotograma, y
acaba en un sistema de invalidación tan caro como lo que ahorra.

En su lugar: **la capa solo vive durante un gesto.** Al pulsar se prepara con
todo menos lo seleccionado; mientras dura el arrastre se copia y se pinta
encima lo que se mueve; al soltar se tira. Fuera de un gesto no hay nada que
invalidar, porque no hay capa.

Sale gratis por qué: durante un gesto, lo único que cambia es lo seleccionado,
y eso es exactamente lo que se excluye.

**Interfaces:**
- Produce:
  ```rust
  /// Con que se preparo la capa. Comparar dos dice si sigue valiendo.
  #[derive(Debug, Clone, PartialEq)]
  pub struct Estampa {
      pub camara: (f32, f32, f32),   // x, y, zoom
      pub tamano: (u32, u32),
      pub excluidos: Vec<u64>,
  }

  pub fn sigue_valiendo(preparada: &Estampa, ahora: &Estampa) -> bool;

  pub struct CapaEstatica { /* privado: el ID2D1Bitmap1 */ }
  impl CapaEstatica {
      pub fn nueva() -> Self;
      pub fn preparar(&mut self, motor: &mut MotorRender, e: Estampa) -> Result<(), ErrorRender>;
      pub fn volcar(&self, motor: &mut MotorRender, ahora: &Estampa) -> bool;
      pub fn soltar(&mut self);
      pub fn lista(&self) -> bool;
      pub fn bytes(&self) -> usize;
  }
  ```

- [ ] **Paso 1: Escribe las pruebas de la política**

Lo que se puede probar sin GPU es cuándo la capa vale y cuándo no, que es
donde están los fallos de verdad:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;

    fn estampa() -> Estampa {
        Estampa {
            camara: (0.0, 0.0, 1.0),
            tamano: (1920, 1080),
            excluidos: vec![7],
        }
    }

    #[test]
    fn la_misma_estampa_vale() {
        assert!(sigue_valiendo(&estampa(), &estampa()));
    }

    #[test]
    fn mover_la_camara_la_invalida() {
        // La capa esta pintada en coordenadas de pantalla: si el lienzo se
        // desplaza, lo pintado ya no cae donde toca.
        let mut ahora = estampa();
        ahora.camara.0 += 1.0;
        assert!(!sigue_valiendo(&estampa(), &ahora));
    }

    #[test]
    fn cambiar_el_aumento_la_invalida() {
        let mut ahora = estampa();
        ahora.camara.2 = 2.0;
        assert!(!sigue_valiendo(&estampa(), &ahora));
    }

    #[test]
    fn cambiar_el_tamano_de_la_ventana_la_invalida() {
        let mut ahora = estampa();
        ahora.tamano = (1280, 720);
        assert!(!sigue_valiendo(&estampa(), &ahora));
    }

    #[test]
    fn cambiar_lo_excluido_la_invalida() {
        // Si la seleccion cambia a mitad de gesto, la capa lleva pintado un
        // elemento que ahora se esta moviendo: se veria por duplicado, uno
        // quieto y otro siguiendo al raton.
        let mut ahora = estampa();
        ahora.excluidos = vec![8];
        assert!(!sigue_valiendo(&estampa(), &ahora));
    }

    #[test]
    fn el_orden_de_lo_excluido_no_importa() {
        // La seleccion es un Vec cuyo orden no significa nada. Invalidar
        // por reordenarlo tiraria la capa sin motivo.
        let preparada = Estampa { excluidos: vec![1, 2, 3], ..estampa() };
        let ahora = Estampa { excluidos: vec![3, 1, 2], ..estampa() };
        assert!(sigue_valiendo(&preparada, &ahora));
    }

    #[test]
    fn una_capa_recien_creada_no_esta_lista() {
        let c = CapaEstatica::nueva();
        assert!(!c.lista());
        assert_eq!(c.bytes(), 0, "sin gesto en curso no ocupa nada");
    }
}
```

- [ ] **Paso 2: Escribe la política**

```rust
//! Pintar una vez lo que no se mueve, y copiarlo en cada fotograma.
//!
//! Mientras se arrastra un elemento, los otros 7.999 no cambian. En una iGPU
//! con memoria compartida, pintarlos una vez a un mapa de bits y copiarlo es
//! la diferencia entre arrastrar y **ver** arrastrar.
//!
//! # Solo vive durante un gesto
//!
//! La tentacion es cachear siempre e ir invalidando cuando algo cambia. Eso
//! obliga a saber que ha cambiado en la escena en cada fotograma y acaba en
//! un sistema de invalidacion tan caro como lo que ahorra.
//!
//! Aqui la capa se prepara al pulsar —con todo menos lo seleccionado—, se
//! copia mientras dura el arrastre, y se tira al soltar. Fuera de un gesto
//! no hay nada que invalidar porque no hay capa. Y durante el gesto lo unico
//! que cambia es lo seleccionado, que es justo lo que se excluye.
//!
//! # Lo que cuesta
//!
//! En 1080p son unos 8 MB, y en la maquina suelo la memoria de video sale de
//! los mismos 4 GB. Es **una de las tres copias vivas** que el presupuesto
//! concede al nivel `Ligero`, y solo mientras dura un arrastre. Queda para
//! medir en la tarea 15, no para prometer.

/// Con que se preparo la capa.
#[derive(Debug, Clone, PartialEq)]
pub struct Estampa {
    /// `x`, `y` y aumento de la camara.
    pub camara: (f32, f32, f32),
    pub tamano: (u32, u32),
    /// Lo que NO esta en la capa porque se esta moviendo.
    pub excluidos: Vec<u64>,
}

/// Si la capa preparada con `preparada` sirve para pintar `ahora`.
///
/// La comparacion de la camara es exacta a proposito. Podria pensarse en una
/// tolerancia —«si se movio menos de un pixel, vale»— pero encuadrar mueve
/// la camara de verdad y la capa esta en coordenadas de pantalla: cualquier
/// desplazamiento la desalinea, y un dibujo medio pixel corrido se ve.
pub fn sigue_valiendo(preparada: &Estampa, ahora: &Estampa) -> bool {
    if preparada.camara != ahora.camara || preparada.tamano != ahora.tamano {
        return false;
    }
    // El orden de la seleccion no significa nada, asi que se comparan como
    // conjuntos: invalidar por reordenarla tiraria la capa sin motivo.
    if preparada.excluidos.len() != ahora.excluidos.len() {
        return false;
    }
    ahora
        .excluidos
        .iter()
        .all(|id| preparada.excluidos.contains(id))
}
```

- [ ] **Paso 3: Escribe la parte que toca la GPU**

Esto va en `pixpin-render`, donde `unsafe` está permitido con `// SAFETY:` en
cada bloque, como manda el encabezado del crate.

`CapaEstatica` guarda un `ID2D1Bitmap1` creado con
`CreateBitmap` y la bandera de destino de dibujo, del tamaño de la ventana.
`preparar` cambia el destino del contexto a ese mapa, pinta la escena sin los
excluidos y devuelve el destino a la pantalla. `volcar` hace un `DrawBitmap`
de ese mapa sobre el destino real.

Sigue el estilo de `superficie.rs` (331 líneas), que ya crea y gestiona
recursos de Direct2D con su `Drop`. **Dos reglas que no puedes saltarte:**

1. Cada bloque `unsafe` con su `// SAFETY:` explicando la precondición. El
   crate lleva `#![deny(clippy::undocumented_unsafe_blocks)]`.
2. `soltar()` tiene que dejar caer el bitmap de verdad (`= None`), no
   marcarlo como inválido. Si no, los 8 MB se quedan vivos entre gestos y el
   presupuesto de copias vivas se incumple en reposo, que es justo donde más
   duele.

Las pruebas de esta parte van marcadas `#[ignore]` con el motivo escrito,
como ya hacen las de bandeja y atajos: necesitan un dispositivo Direct3D y
`windows-latest` no lo ofrece de forma fiable.

- [ ] **Paso 4: Úsala desde la ventana**

En `ventana_editor.rs`:

```rust
// Al pulsar, si hay algo que se va a mover.
if !gesto.seleccion.esta_vacia() && !gesto.en_reposo() {
    capa.preparar(&mut motor, estampa_de(&camara, tamano, &gesto.seleccion))?;
}
// En cada fotograma del gesto.
if capa.volcar(&mut motor, &ahora) {
    pintar_solo(&mut motor, &escena, gesto.seleccion.ids());
} else {
    pintar(&mut motor, &escena, &camara, &gesto, &mut cache, &rejilla);
}
// Al soltar o al cancelar.
capa.soltar();
```

Fíjate en que `volcar` devuelve `bool`: si la capa ya no vale —porque se
encuadró a mitad del gesto—, se pinta todo como siempre. **Nunca** hay que
repintar la capa a mitad de un arrastre: eso sería pagar el precio completo en
el peor momento.

- [ ] **Paso 5: Cierra y commit**

```
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

```bash
git add crates/pixpin-render/src/capa_estatica.rs crates/pixpin-render/src/lib.rs apps/pixpin/src/ventana_editor.rs
git commit -m "La capa estatica: pintar una vez lo que no se mueve

Mientras se arrastra un elemento, los otros 7.999 no cambian. En una iGPU
con memoria compartida, pintarlos una vez a un mapa de bits y copiarlo es
la diferencia entre arrastrar y ver arrastrar.

La decision que lo simplifica todo: la capa solo vive durante un gesto.
Cachear siempre e ir invalidando obligaria a saber que ha cambiado en la
escena en cada fotograma, y acabaria costando lo que ahorra. Asi, fuera
de un gesto no hay nada que invalidar porque no hay capa, y durante el
gesto lo unico que cambia es lo seleccionado, que es lo que se excluye.

La politica —cuando vale la capa— es pura y se prueba sin GPU. Lo que
toca Direct2D va marcado #[ignore] como el resto de lo que necesita
escritorio.

soltar() deja caer el bitmap de verdad y no lo marca como invalido: si no,
los 8 MB se quedarian vivos entre gestos y el presupuesto de copias vivas
se incumpliria en reposo, que es donde mas duele."
```

---

## Tarea 15: Contar asignaciones y medir de verdad

La última, y la que decide si todo lo anterior sirvió de algo.

**Ficheros:**
- Crear: `crates/pixpin-motor2d/tests/asignaciones.rs`
- Crear: `medidas/2026-09-06-editor-avanzado.md`
- Prueba: el propio fichero de pruebas

### Por qué esto es una tarea y no un apéndice

Dos hechos encontrados al planificar:

1. **El asignador que cuenta no existe.** El documento de rendimiento pone
   «asignaciones en el camino caliente: 0, carril automático» en su tabla de
   presupuesto, y no hay nada que lo mida.
2. **La máquina suelo nunca se ha medido.** Las nueve mediciones del proyecto
   se llaman todas `equipo-desarrollo`.

- [ ] **Paso 1: Escribe el asignador que cuenta**

Va en `tests/` y no en `src/` a propósito: un `#[global_allocator]` afecta a
todo el binario, y no queremos uno en el programa de verdad.

```rust
//! Cuantas veces pide memoria el camino caliente.
//!
//! El documento de rendimiento pone «asignaciones en el camino caliente: 0»
//! en su tabla de presupuesto, y hasta ahora no habia nada que lo midiera.
//!
//! El camino caliente es uno solo: **mover el raton mientras se dibuja**.
//! Tiene el unico plazo sagrado del editor —un fotograma del refresco real,
//! 16 ms a 60 Hz— y es donde una asignacion se nota, porque el asignador
//! puede irse al sistema operativo en el peor momento.
//!
//! Esto vive en `tests/` y no en `src/` porque un `#[global_allocator]`
//! afecta a todo el binario: no queremos uno en el programa de verdad.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static VECES: AtomicUsize = AtomicUsize::new(0);
static CONTANDO: AtomicUsize = AtomicUsize::new(0);

struct Contador;

// SAFETY: se delega todo en `System`, que cumple el contrato de
// `GlobalAlloc`. Lo unico anadido es un contador atomico, que no toca la
// memoria devuelta ni cambia el puntero.
unsafe impl GlobalAlloc for Contador {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if CONTANDO.load(Ordering::Relaxed) == 1 {
            VECES.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: mismo `Layout` que nos han dado.
        unsafe { System.alloc(l) }
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: el puntero y el layout vienen de nuestro `alloc`.
        unsafe { System.dealloc(p, l) }
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, nuevo: usize) -> *mut u8 {
        // Reasignar tambien cuenta: es lo que hace un `Vec` al crecer, y es
        // justo lo que esta prueba viene a cazar.
        if CONTANDO.load(Ordering::Relaxed) == 1 {
            VECES.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: el puntero y el layout vienen de nuestro `alloc`.
        unsafe { System.realloc(p, l, nuevo) }
    }
}

#[global_allocator]
static ASIGNADOR: Contador = Contador;

/// Cuenta las asignaciones que hace `f`.
///
/// Un solo hilo: las pruebas de este proyecto corren con `--test-threads=1`
/// justamente porque varias toman recursos globales, y el contador es uno
/// mas.
fn contando<T>(f: impl FnOnce() -> T) -> (T, usize) {
    VECES.store(0, Ordering::Relaxed);
    CONTANDO.store(1, Ordering::Relaxed);
    let r = f();
    CONTANDO.store(0, Ordering::Relaxed);
    (r, VECES.load(Ordering::Relaxed))
}
```

- [ ] **Paso 2: Escribe las pruebas de presupuesto**

```rust
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::vector::Punto2;

#[test]
fn mover_el_raton_dibujando_no_asigna_memoria() {
    // La puerta que faltaba. Si esto falla, hay un `Vec` creciendo o un
    // `to_vec()` colado en el camino caliente.
    let mut escena = Escena::nueva();
    let mut gesto = Gesto::nuevo();
    gesto.herramienta = Herramienta::Lapiz;

    // Empezar el trazo SI asigna: crea el elemento y reserva sus puntos.
    // Eso pasa una vez por trazo, no una vez por aviso del raton.
    gesto.evento(
        EventoGesto::Pulsar { p: Punto2::nuevo(0.0, 0.0), shift: false, alt: false },
        &mut escena,
        1.0,
    );

    let (_, veces) = contando(|| {
        for i in 1..400 {
            gesto.evento(
                EventoGesto::Mover {
                    p: Punto2::nuevo(i as f32, (i % 7) as f32),
                    shift: false,
                    alt: false,
                },
                &mut escena,
                1.0,
            );
        }
    });

    assert_eq!(veces, 0, "el camino caliente asigno {veces} veces");
}

#[test]
fn arrastrar_una_seleccion_tampoco_asigna() {
    // Arrastrar es tan camino caliente como dibujar, y es donde un
    // `.to_vec()` sobre los ids de la seleccion se cuela con mas facilidad:
    // parece necesario por el prestamo y no lo es.
    use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};

    let mut escena = Escena::nueva();
    let mut ids = Vec::new();
    for i in 0..20 {
        ids.push(escena.anadir(Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x: i as f32 * 5.0,
            y: 0.0,
            ancho: 40.0,
            alto: 40.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
        }));
    }

    let mut gesto = Gesto::nuevo();
    gesto.herramienta = Herramienta::Mano;
    gesto.seleccion.poner_todos(ids);

    // Pulsar SI asigna: abre el paso y guarda una instantanea por elemento.
    // Eso pasa una vez por gesto.
    gesto.evento(
        EventoGesto::Pulsar { p: Punto2::nuevo(10.0, 10.0), shift: false, alt: false },
        &mut escena,
        1.0,
    );

    let (_, veces) = contando(|| {
        for i in 1..400 {
            gesto.evento(
                EventoGesto::Mover {
                    p: Punto2::nuevo(10.0 + i as f32, 10.0),
                    shift: false,
                    alt: false,
                },
                &mut escena,
                1.0,
            );
        }
    });

    assert_eq!(veces, 0, "arrastrar asigno {veces} veces");
}

#[test]
fn mover_el_raton_en_reposo_tampoco_asigna() {
    // Pasar el raton por encima calcula el cursor, que consulta tiradores y
    // picado. Es casi tan frecuente como dibujar.
    let mut escena = Escena::nueva();
    let mut gesto = Gesto::nuevo();
    gesto.herramienta = Herramienta::Mano;

    let (_, veces) = contando(|| {
        for i in 0..400 {
            gesto.evento(
                EventoGesto::Mover {
                    p: Punto2::nuevo(i as f32, 0.0),
                    shift: false,
                    alt: false,
                },
                &mut escena,
                1.0,
            );
        }
    });

    assert_eq!(veces, 0, "pasar el raton asigno {veces} veces");
}

#[test]
fn encuadrar_sesenta_fotogramas_solo_calcula_la_geometria_una_vez() {
    // No es una prueba de tiempo —eso depende de la maquina— sino de
    // comportamiento: encuadrar no cambia el aumento, asi que no debe
    // invalidar nada. Es lo que hace que arrastrar el lienzo con ocho mil
    // elementos no cueste nada.
    use pixpin_motor2d::cache::Cache;
    use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};

    const CUANTOS: u64 = 500;
    const FOTOGRAMAS: u64 = 60;

    let mut escena = Escena::nueva();
    for i in 0..CUANTOS {
        escena.anadir(Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x: i as f32 * 30.0,
            y: 0.0,
            ancho: 20.0,
            alto: 20.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: (i + 1) as u32,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
        });
    }

    let mut cache = Cache::nueva();
    for _ in 0..FOTOGRAMAS {
        // Encuadrar mueve la camara pero NO el aumento: el zoom que se le
        // pasa a la cache es el mismo en los sesenta fotogramas.
        for e in escena.visibles() {
            cache.ordenes(e, 1.0);
        }
    }

    assert_eq!(cache.fallos(), CUANTOS, "solo el primer fotograma calcula");
    assert_eq!(cache.aciertos(), CUANTOS * (FOTOGRAMAS - 1));
}
```

Si te sale un número distinto de 500 fallos, la caché se está invalidando por
algo que no debería: mira el nivel de detalle.

- [ ] **Paso 3: Si alguna falla, arréglalo donde toca**

Los sospechosos, por orden de probabilidad:

| Síntoma | Causa casi segura |
|---|---|
| Asigna una vez cada pocos avisos | Un `Vec` creciendo: falta un `with_capacity` |
| Asigna una vez por aviso | Un `to_vec()` o un `collect()` en el camino caliente |
| Asigna en reposo | `cursor_en` construyendo `Tiradores`, que tiene un array fijo pero llama a `Seleccion::caja` |

**El arreglo va en el motor, nunca en la prueba.** Si te ves tentado de subir
el número esperado de cero a «unas pocas», para y piensa: el presupuesto dice
cero, y cero es comprobable.

- [ ] **Paso 4: Mide, y escribe lo que salga**

Crea `medidas/2026-09-06-editor-avanzado.md`. Mide **cuatro cosas**, con la
escena de verdad —un `.pixpin` del usuario, no una inventada— y también con
carga sintética:

| Qué | Cómo |
|---|---|
| Latencia de trazo | Tiempo entre `WM_MOUSEMOVE` y el fin del `Present`, en el peor de 500 avisos |
| Fotogramas al arrastrar | Con 8.000 y con 20.000 elementos, con y sin capa estática |
| RAM en reposo | Con el editor abierto y sin tocar nada |
| RAM en pico | Arrastrando una selección de 500 elementos |

Y hazlo **dos veces**: con el nivel `Completo` y con **`Ligero` forzado** desde
`pixpinmax.toml`. La decisión D15 dice con estas palabras que esa anulación
«no es un lujo: es lo que permite ejercitar y medir la ruta ligera en
cualquier máquina».

**Y el informe tiene que decir lo que es.** Empieza el documento con esto,
literalmente:

> **Medido en el equipo de desarrollo** (Intel Core i7-10510U, 4 núcleos
> físicos, 15,8 GB de RAM, Intel UHD + NVIDIA MX250, Windows 11), **no en la
> máquina suelo.** El usuario no tiene el i3 de 2012 a mano. La carga
> sintética de 8.000 y 20.000 elementos busca acercarse, pero **no sustituye
> la medición del suelo, que sigue pendiente.**
>
> Un riesgo que este carril no puede cubrir: este equipo es un Comet Lake
> **con AVX2**. Un binario compilado con `target-cpu=native` funcionaría aquí
> perfectamente y moriría en el i3 con instrucción ilegal. Eso no se pilla
> midiendo, sino vigilando la configuración de compilación, y por eso D17
> fija el baseline en `x86-64` explícito.

Sigue el formato de los nueve informes que ya hay en `medidas/`.

- [ ] **Paso 5: Cierra y commit**

```
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

```bash
git add crates/pixpin-motor2d/tests/asignaciones.rs medidas/2026-09-06-editor-avanzado.md
git commit -m "Contar asignaciones, y medir diciendo donde se midio

Dos huecos encontrados al planificar. El documento de rendimiento pone
«asignaciones en el camino caliente: 0, carril automatico» en su tabla de
presupuesto y no habia nada que lo midiera. Y las nueve mediciones del
proyecto se llaman todas equipo-desarrollo: la maquina suelo nunca se ha
medido.

El asignador que cuenta vive en tests/ y no en src/ porque un
global_allocator afecta a todo el binario, y no queremos uno en el
programa de verdad. Cuenta tambien las reasignaciones, que es lo que hace
un Vec al crecer y justo lo que la prueba viene a cazar.

El usuario no tiene el i3 de 2012 a mano, asi que se mide aqui con Ligero
forzado y carga sintetica de 8.000 y 20.000 elementos. Y el informe lo
dice con todas las letras: medido en el equipo de desarrollo, no en el
suelo, y la del suelo sigue pendiente. Un numero medido en el sitio
equivocado y presentado como bueno es peor que no tenerlo."
```

---

## Cuando esté todo

**Lo que hay que poder hacer, y no se podía antes de empezar:**

1. Seleccionar varios elementos, a mano o con marquesina.
2. Redimensionarlos por cualquiera de los ocho tiradores, girado o no, sin
   que la esquina anclada se mueva.
3. Girarlos, sueltos o en conjunto, a saltos de 15° con `Shift`.
4. Deshacer un arrastre entero con un solo `Ctrl+Z`.
5. Cancelar un arrastre a mitad con `Escape`.
6. Agrupar, alinear y repartir.
7. Abrir un plano hecho en el móvil con sus grupos intactos, y devolverlo.

**Lo que queda fuera y dónde va:**

| Qué | Dónde |
|---|---|
| Filtro de un euro, pulso del trazo, espina Catmull-Rom | Segunda entrega del editor |
| Escribir texto **dentro del editor** (el anotador ya lo tiene) | Segunda entrega del editor |
| Las veinte herramientas que faltan | Fase B de `2026-09-06-android-a-windows.md` |
| Que el anotador y el pin hereden tiradores e historial | Decisión aparte |
| Abrir un `.pixpin` desde el editor | Fase A/C del plan maestro |
| El croquis 3D | Fase D |
| La sincronización por WiFi | Aplazada por el usuario |
