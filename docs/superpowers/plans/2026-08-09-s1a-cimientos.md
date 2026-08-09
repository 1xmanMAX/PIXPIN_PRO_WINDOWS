# PixPin Max · S1-A Cimientos — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dejar en pie una aplicación que arranca, vive en la bandeja del sistema, resuelve dónde guardar sus ajustes, los lee y escribe en TOML, habla español e inglés, registra atajos globales reasignables e impide que se ejecute dos veces a la vez.

**Architecture:** Workspace `cargo` con 15 crates de librería en cinco capas más el ejecutable. La regla de capas (una capa sólo depende de capas inferiores) la verifica un test, no la disciplina. Toda la lógica pura —resolución de rutas, ajustes, combinaciones de teclas, elección de idioma— vive en crates con `#![forbid(unsafe_code)]` y se prueba sin Windows. El `unsafe` queda confinado a `pixpin-shell`, que es lo único que habla con Win32 en este plan.

**Tech Stack:** Rust estable (edición 2024) · crate `windows` para Win32 · `serde` + `toml` para ajustes · `fluent-bundle` para idiomas · `tracing` para registro · `cargo-deny` como puerta de licencias.

**Documento de diseño:** [`docs/superpowers/specs/2026-08-09-s1-cimientos-captura-design.md`](../specs/2026-08-09-s1-cimientos-captura-design.md)
**Documento maestro:** [`docs/superpowers/specs/2026-08-09-pixpin-pc-master-design.md`](../specs/2026-08-09-pixpin-pc-master-design.md)

## Global Constraints

Estas reglas aplican a **todas** las tareas de este plan. No se repiten en cada una.

- **Rust estable, edición 2024. Nada de `nightly`.**
- **Windows mínimo: Windows 10 21H2 (compilación 19044).**
- **Licencia MIT.** `cargo-deny` debe rechazar cualquier dependencia GPL o AGPL. Es la regla que protege la decisión D2/D3 del documento maestro.
- **Nombre del ejecutable: `pixpinmax.exe`.** Nombre del producto: **PixPin Max**.
- **`#![forbid(unsafe_code)]` obligatorio** en `pixpin-geom`, `pixpin-model`, `pixpin-flow`, `pixpin-store`, `pixpin-ui`, `pixpin-plugin`.
- **`unsafe` permitido pero documentado con `// SAFETY:`** en `pixpin-shell`, `pixpin-render`, `pixpin-gpu`, `pixpin-codec`, `pixpin-capture`, `pixpin-record`, `pixpin-pin`, `pixpin-ocr`, `pixpin-pdf`. Cada bloque `unsafe` lleva su comentario explicando por qué es correcto.
- **Regla de capas:** un crate sólo puede depender de crates de una capa estrictamente inferior. L0 `geom, model` · L1 `shell, render, gpu, codec` · L2 `capture, pin, pdf, ocr, record, store` · L3 `ui, flow, plugin` · L4 `pixpin` (el ejecutable).
- **Cero telemetría, cero analíticas, cero red.** Ninguna tarea de este plan puede abrir un socket.
- **Idiomas:** `es-ES` y `en-US`. Todo texto visible al usuario pasa por el catálogo Fluent, nunca literal en el código.
- **Comentarios y nombres en español**, como en el proyecto Android. Los identificadores públicos van en español (`Ajustes`, `resolver`, `Ubicacion`).
- **Commits frecuentes.** Cada tarea termina en commit.

---

## Estructura de ficheros

Lo que este plan crea. Cada fichero tiene una responsabilidad y sólo una.

```
Cargo.toml                       workspace: miembros, dependencias compartidas, perfil release
rust-toolchain.toml              fija canal estable
deny.toml                        puerta de licencias (rechaza GPL/AGPL)
.github/workflows/ci.yml         clippy, tests, cargo-deny
LICENSE                          MIT
README.md                        con la nota de marca de DepthPixel

crates/
  pixpin-geom/src/lib.rs         L0 esqueleto
  pixpin-model/src/lib.rs        L0 esqueleto
  pixpin-render/src/lib.rs       L1 esqueleto
  pixpin-gpu/src/lib.rs          L1 esqueleto
  pixpin-codec/src/lib.rs        L1 esqueleto
  pixpin-shell/
    src/lib.rs                   reexporta los modulos
    src/atajo.rs                 tipo Atajo: parseo y formato de "Ctrl+Alt+X"  [logica pura]
    src/instancia.rs             mutex con nombre para instancia unica
    src/ventana.rs               ventana solo-mensajes y bucle dirigido por eventos
    src/atajos.rs                registro Win32 de atajos globales
    src/bandeja.rs               icono y menu de la bandeja del sistema
    src/arranque.rs              arranque con Windows (registro)
    src/entorno.rs               locale del sistema, ruta del ejecutable, APPDATA
  pixpin-capture/src/lib.rs      L2 esqueleto
  pixpin-pin/src/lib.rs          L2 esqueleto
  pixpin-pdf/src/lib.rs          L2 esqueleto
  pixpin-ocr/src/lib.rs          L2 esqueleto
  pixpin-record/src/lib.rs       L2 esqueleto
  pixpin-store/
    src/lib.rs                   reexporta los modulos
    src/rutas.rs                 portable vs instalado
    src/ajustes.rs               struct Ajustes + TOML
    src/idioma.rs                catalogo Fluent y resolucion de idioma
    i18n/es-ES/main.ftl          textos en español
    i18n/en-US/main.ftl          textos en ingles
  pixpin-ui/src/lib.rs           L3 esqueleto
  pixpin-flow/src/lib.rs         L3 esqueleto
  pixpin-plugin/src/lib.rs       L3 esqueleto

apps/pixpin/
  Cargo.toml                     produce pixpinmax.exe
  build.rs                       incrusta el manifiesto (DPI, compatibilidad)
  pixpinmax.manifest             PerMonitorV2, supportedOS Windows 10, asInvoker
  src/main.rs                    ensambla todo
  tests/capas.rs                 verifica la regla de capas
```

**Por qué `Atajo` vive en `pixpin-shell` y no en `pixpin-store`:** `pixpin-shell` es L1 y `pixpin-store` es L2, así que shell **no puede** depender de store. Como `RegisterHotKey` vive en shell y los ajustes necesitan serializar atajos, el tipo tiene que estar en la capa de abajo. Store depende de shell (L2 → L1, permitido) y así ambos lo usan sin romper la regla.

---

## Task 1: Workspace, regla de capas y CI

Crea el esqueleto completo y, sobre todo, **el test que impide que la arquitectura se degrade**. Se hace primero porque cualquier tarea posterior que rompa las capas debe fallar de inmediato.

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `deny.toml`, `LICENSE`, `README.md`
- Create: `.github/workflows/ci.yml`
- Create: `crates/<cada uno de los 15>/Cargo.toml` y `crates/<cada uno>/src/lib.rs`
- Create: `apps/pixpin/Cargo.toml`, `apps/pixpin/src/main.rs`
- Test: `apps/pixpin/tests/capas.rs`

**Interfaces:**
- Consumes: nada.
- Produces: el workspace. Los crates de esqueleto exponen sólo su atributo de política de `unsafe`. El ejecutable se llama `pixpinmax`.

- [ ] **Step 1: Escribir el test de capas (fallará)**

Crear `apps/pixpin/tests/capas.rs`:

```rust
//! Verifica la regla de capas del documento maestro: un crate solo puede
//! depender de crates de una capa estrictamente inferior.
//!
//! Se lee el Cargo.toml de cada miembro del workspace en vez de inspeccionar
//! el grafo de cargo porque asi el test no necesita compilar nada y corre en
//! milisegundos, tambien en CI sin Windows.

use std::fs;
use std::path::{Path, PathBuf};

/// Capa de cada crate. Numero menor = mas abajo en la arquitectura.
fn capa(nombre: &str) -> Option<u8> {
    Some(match nombre {
        "pixpin-geom" | "pixpin-model" => 0,
        "pixpin-shell" | "pixpin-render" | "pixpin-gpu" | "pixpin-codec" => 1,
        "pixpin-capture" | "pixpin-pin" | "pixpin-pdf" | "pixpin-ocr" | "pixpin-record"
        | "pixpin-store" => 2,
        "pixpin-ui" | "pixpin-flow" | "pixpin-plugin" => 3,
        "pixpin" => 4,
        _ => return None,
    })
}

/// Raiz del workspace, subiendo desde el directorio de este paquete.
fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("el paquete debe estar en <raiz>/apps/pixpin")
        .to_path_buf()
}

/// Devuelve (nombre, ruta del Cargo.toml) de cada crate de PixPin.
fn manifiestos() -> Vec<(String, PathBuf)> {
    let mut salida = Vec::new();
    for dir in [raiz().join("crates"), raiz().join("apps")] {
        for entrada in fs::read_dir(&dir).expect("falta el directorio del workspace") {
            let ruta = entrada.unwrap().path().join("Cargo.toml");
            if ruta.is_file() {
                let texto = fs::read_to_string(&ruta).unwrap();
                let doc: toml::Value = texto.parse().unwrap();
                let nombre = doc["package"]["name"].as_str().unwrap().to_string();
                salida.push((nombre, ruta));
            }
        }
    }
    salida
}

#[test]
fn estan_los_dieciseis_paquetes() {
    let encontrados = manifiestos();
    assert_eq!(
        encontrados.len(),
        16,
        "se esperan 15 crates de libreria mas el ejecutable, encontrados: {:?}",
        encontrados.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );
}

#[test]
fn todo_paquete_tiene_capa_asignada() {
    for (nombre, _) in manifiestos() {
        assert!(
            capa(&nombre).is_some(),
            "el paquete `{nombre}` no tiene capa asignada en el test; \
             añadelo a la funcion `capa` o revisa como se llama"
        );
    }
}

#[test]
fn ninguna_dependencia_sube_de_capa() {
    for (nombre, ruta) in manifiestos() {
        let mia = capa(&nombre).unwrap();
        let texto = fs::read_to_string(&ruta).unwrap();
        let doc: toml::Value = texto.parse().unwrap();

        for seccion in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(tabla) = doc.get(seccion).and_then(|v| v.as_table()) else {
                continue;
            };
            for dep in tabla.keys() {
                let Some(suya) = capa(dep) else { continue };
                assert!(
                    suya < mia,
                    "`{nombre}` (capa {mia}) depende de `{dep}` (capa {suya}) en [{seccion}]. \
                     Una capa solo puede depender de capas estrictamente inferiores."
                );
            }
        }
    }
}
```

- [ ] **Step 2: Ejecutar el test y comprobar que falla**

Run: `cargo test -p pixpin --test capas`
Expected: FAIL — no existe todavía ni el workspace ni el paquete `pixpin`.

- [ ] **Step 3: Crear el workspace raíz**

Crear `Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = ["crates/*", "apps/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
license = "MIT"
authors = ["Max Anthony Mamani Maquera"]
repository = "https://github.com/1xmanMAX/pixpin-max"

[workspace.dependencies]
# Internas
pixpin-geom = { path = "crates/pixpin-geom" }
pixpin-model = { path = "crates/pixpin-model" }
pixpin-shell = { path = "crates/pixpin-shell" }
pixpin-render = { path = "crates/pixpin-render" }
pixpin-gpu = { path = "crates/pixpin-gpu" }
pixpin-codec = { path = "crates/pixpin-codec" }
pixpin-capture = { path = "crates/pixpin-capture" }
pixpin-pin = { path = "crates/pixpin-pin" }
pixpin-pdf = { path = "crates/pixpin-pdf" }
pixpin-ocr = { path = "crates/pixpin-ocr" }
pixpin-record = { path = "crates/pixpin-record" }
pixpin-store = { path = "crates/pixpin-store" }
pixpin-ui = { path = "crates/pixpin-ui" }
pixpin-flow = { path = "crates/pixpin-flow" }
pixpin-plugin = { path = "crates/pixpin-plugin" }

[profile.release]
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
opt-level = 3
```

Crear `rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["clippy", "rustfmt"]
targets = ["x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"]
```

- [ ] **Step 4: Crear los 15 crates de librería**

Para **cada** crate de la lista, crear `crates/<nombre>/Cargo.toml`:

```toml
[package]
name = "<nombre>"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
```

Y `crates/<nombre>/src/lib.rs`. Para los **seis** crates sin `unsafe` (`pixpin-geom`, `pixpin-model`, `pixpin-store`, `pixpin-ui`, `pixpin-flow`, `pixpin-plugin`):

```rust
//! <nombre> — ver docs/superpowers/specs/2026-08-09-pixpin-pc-master-design.md
//!
//! Este crate no puede contener `unsafe`. Si alguna vez lo necesitara, seria
//! señal de que la frontera de capas se ha roto y hay que arreglar el diseño,
//! no relajar la regla.
#![forbid(unsafe_code)]
```

Para los **nueve** que hablan con el sistema (`pixpin-shell`, `pixpin-render`, `pixpin-gpu`, `pixpin-codec`, `pixpin-capture`, `pixpin-pin`, `pixpin-pdf`, `pixpin-ocr`, `pixpin-record`):

```rust
//! <nombre> — ver docs/superpowers/specs/2026-08-09-pixpin-pc-master-design.md
//!
//! Este crate habla con el sistema operativo o con librerias C. El `unsafe`
//! esta permitido, pero cada bloque lleva su comentario `// SAFETY:`.
#![deny(clippy::undocumented_unsafe_blocks)]
```

- [ ] **Step 5: Crear el ejecutable**

Crear `apps/pixpin/Cargo.toml`:

```toml
[package]
name = "pixpin"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[[bin]]
name = "pixpinmax"
path = "src/main.rs"

[dependencies]

[dev-dependencies]
toml = "0.8"
```

Crear `apps/pixpin/src/main.rs`:

```rust
//! PixPin Max — punto de entrada.
//!
//! PixPin es marca de DepthPixel. Este proyecto es una implementacion
//! personal e independiente.

fn main() {
    println!("PixPin Max — esqueleto");
}
```

- [ ] **Step 6: Ejecutar el test y comprobar que pasa**

Run: `cargo test -p pixpin --test capas`
Expected: PASS — los tres tests en verde.

- [ ] **Step 7: Añadir la puerta de licencias**

Crear `deny.toml`:

```toml
# Puerta de licencias. Protege la decision D2/D3 del documento maestro:
# PixPin Max es MIT y no puede heredar copyleft de ninguna dependencia.
# QuickView (GPL-3.0) es referencia de tecnicas, nunca de codigo.

[licenses]
# Solo licencias permisivas compatibles con MIT.
allow = [
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "Unicode-3.0",
    "CC0-1.0",
    "MPL-2.0",
]
confidence-threshold = 0.9

[bans]
# Cualquier copyleft fuerte es un fallo de build, no un aviso.
deny = []
multiple-versions = "warn"

[advisories]
yanked = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

- [ ] **Step 8: Comprobar que la puerta de licencias funciona**

Run: `cargo install cargo-deny --locked` (una sola vez) y después `cargo deny check licenses`
Expected: PASS. Si alguna dependencia futura fuese GPL, este comando fallaría.

- [ ] **Step 9: Añadir CI**

Crear `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-D warnings"

jobs:
  # Todo lo que vive en L0/L2 puro corre aqui: rapido y sin GPU.
  pruebas:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2
      - name: Formato
        run: cargo fmt --all --check
      - name: Clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
      - name: Tests
        run: cargo test --workspace

  licencias:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: EmbarkStudios/cargo-deny-action@v2
        with:
          command: check licenses bans sources
```

- [ ] **Step 10: Añadir LICENSE y README**

Crear `LICENSE` con el texto estándar de MIT, año 2026, titular «Max Anthony Mamani Maquera».

Crear `README.md`:

```markdown
# PixPin Max

Herramienta de captura, anotación y pines flotantes para Windows 10 21H2 o superior.
Escrita en Rust. Sin cuentas, sin red, sin telemetría.

> El nombre **PixPin** pertenece a DepthPixel. Este proyecto es una implementación
> personal e independiente, igual que su versión Android.

## Estado

En construcción. Ver `docs/superpowers/specs/` para el diseño y
`docs/superpowers/plans/` para los planes de implementación.

## Compilar

```
cargo build --release
```

El binario sale en `target/release/pixpinmax.exe`.

## Licencia

MIT.
```

- [ ] **Step 11: Comprobar que todo el workspace está sano**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: los tres comandos en verde.

- [ ] **Step 12: Commit**

```bash
git add -A
git commit -m "Workspace de PixPin Max con la regla de capas verificada por test

Crea los 15 crates de libreria mas el ejecutable, repartidos en las cinco
capas del documento maestro, y un test que lee los Cargo.toml y falla si
algun crate depende de una capa igual o superior. Se hace antes que nada
porque una arquitectura por capas que solo vive en un documento se degrada
en semanas.

cargo-deny queda configurado para rechazar copyleft: es lo que impide que
la licencia MIT se pierda por descuido al añadir una dependencia."
```

---

## Task 2: Resolución de rutas — portable frente a instalado

**Files:**
- Create: `crates/pixpin-store/src/rutas.rs`
- Modify: `crates/pixpin-store/src/lib.rs`
- Modify: `crates/pixpin-store/Cargo.toml`

**Interfaces:**
- Consumes: nada.
- Produces:
  - `pub enum Ubicacion { Portable { raiz: PathBuf }, Instalado { raiz: PathBuf } }`
  - `pub const NOMBRE_AJUSTES: &str = "pixpinmax.toml"`
  - `pub fn resolver(dir_exe: &Path, appdata: &Path) -> Ubicacion`
  - `impl Ubicacion { pub fn fichero_ajustes(&self) -> PathBuf; pub fn es_portable(&self) -> bool }`

- [ ] **Step 1: Escribir los tests que fallan**

Crear `crates/pixpin-store/src/rutas.rs` con sólo los tests al final del fichero:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    /// Crea un directorio temporal propio del test, sin dependencias externas.
    fn temporal(etiqueta: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pixpin-pruebas-{etiqueta}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn con_fichero_junto_al_exe_es_portable() {
        let exe = temporal("portable");
        fs::write(exe.join(NOMBRE_AJUSTES), "").unwrap();

        let u = resolver(&exe, Path::new(r"C:\NoUsado"));

        assert_eq!(u, Ubicacion::Portable { raiz: exe.clone() });
        assert!(u.es_portable());
        assert_eq!(u.fichero_ajustes(), exe.join(NOMBRE_AJUSTES));
    }

    #[test]
    fn sin_fichero_junto_al_exe_es_instalado() {
        let exe = temporal("instalado");
        let appdata = temporal("appdata");

        let u = resolver(&exe, &appdata);

        assert_eq!(u, Ubicacion::Instalado { raiz: appdata.join("PixPinMax") });
        assert!(!u.es_portable());
        assert_eq!(
            u.fichero_ajustes(),
            appdata.join("PixPinMax").join(NOMBRE_AJUSTES)
        );
    }

    #[test]
    fn un_directorio_con_ese_nombre_no_cuenta_como_portable() {
        // Si alguien crea por error una CARPETA llamada pixpinmax.toml, no debe
        // activarse el modo portable: se buscaria un fichero de ajustes que no
        // existe y la app arrancaria con un estado incoherente.
        let exe = temporal("directorio");
        fs::create_dir(exe.join(NOMBRE_AJUSTES)).unwrap();
        let appdata = temporal("appdata2");

        let u = resolver(&exe, &appdata);

        assert!(!u.es_portable(), "una carpeta no debe activar el modo portable");
    }
}
```

- [ ] **Step 2: Ejecutar y comprobar que falla**

Run: `cargo test -p pixpin-store rutas`
Expected: FAIL — no existen `Ubicacion`, `resolver`, `NOMBRE_AJUSTES`.

- [ ] **Step 3: Implementar**

Añadir al principio de `crates/pixpin-store/src/rutas.rs`:

```rust
//! Donde viven los ajustes.
//!
//! El modo portable no se activa con una bandera ni con un instalador
//! distinto: **lo decide la presencia del fichero de ajustes junto al
//! ejecutable**. Asi copiar la carpeta a un USB y llevarsela funciona sin que
//! el usuario tenga que saber nada, y una instalacion normal nunca escribe
//! junto al .exe (donde probablemente no tendria permisos).

use std::path::{Path, PathBuf};

/// Nombre del fichero de ajustes, igual en los dos modos.
pub const NOMBRE_AJUSTES: &str = "pixpinmax.toml";

/// Carpeta dentro de APPDATA en el modo instalado.
const CARPETA_APPDATA: &str = "PixPinMax";

/// Donde estan los datos de la aplicacion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ubicacion {
    /// Todo junto al ejecutable. Cero rastro en el registro.
    Portable { raiz: PathBuf },
    /// Bajo `%APPDATA%\PixPinMax`.
    Instalado { raiz: PathBuf },
}

impl Ubicacion {
    /// Raiz de datos, sea cual sea el modo.
    pub fn raiz(&self) -> &Path {
        match self {
            Ubicacion::Portable { raiz } | Ubicacion::Instalado { raiz } => raiz,
        }
    }

    /// Ruta completa del fichero de ajustes.
    pub fn fichero_ajustes(&self) -> PathBuf {
        self.raiz().join(NOMBRE_AJUSTES)
    }

    pub fn es_portable(&self) -> bool {
        matches!(self, Ubicacion::Portable { .. })
    }
}

/// Decide el modo a partir del directorio del ejecutable y de APPDATA.
///
/// Ambas rutas se pasan como parametro en vez de consultarlas aqui para que
/// esto sea logica pura y se pueda probar sin tocar el entorno real.
pub fn resolver(dir_exe: &Path, appdata: &Path) -> Ubicacion {
    if dir_exe.join(NOMBRE_AJUSTES).is_file() {
        Ubicacion::Portable { raiz: dir_exe.to_path_buf() }
    } else {
        Ubicacion::Instalado { raiz: appdata.join(CARPETA_APPDATA) }
    }
}
```

Añadir a `crates/pixpin-store/src/lib.rs`:

```rust
pub mod rutas;

pub use rutas::{NOMBRE_AJUSTES, Ubicacion, resolver};
```

- [ ] **Step 4: Ejecutar y comprobar que pasa**

Run: `cargo test -p pixpin-store rutas`
Expected: PASS — los tres tests en verde.

- [ ] **Step 5: Commit**

```bash
git add crates/pixpin-store
git commit -m "Resolucion de rutas: el modo portable lo decide la presencia del fichero

pixpinmax.toml junto al .exe activa el modo portable; si no esta, se usa
%APPDATA%\\PixPinMax. Sin banderas ni instaladores distintos: copiar la
carpeta a un USB simplemente funciona.

resolver() recibe el directorio del exe y APPDATA como parametros en vez de
consultarlos, para que sea logica pura y testeable sin tocar el entorno. Se
comprueba tambien que una CARPETA con ese nombre no active el modo portable
por error."
```

---

## Task 3: Combinaciones de teclas

Vive en `pixpin-shell` (L1) porque `pixpin-store` (L2) necesita serializarlas y una capa no puede depender de otra igual o superior. El parseo en sí es lógica pura.

**Files:**
- Create: `crates/pixpin-shell/src/atajo.rs`
- Modify: `crates/pixpin-shell/src/lib.rs`
- Modify: `crates/pixpin-shell/Cargo.toml`

**Interfaces:**
- Consumes: nada.
- Produces:
  - `pub struct Modificadores { pub ctrl: bool, pub alt: bool, pub shift: bool, pub win: bool }`
  - `pub struct Atajo { pub modificadores: Modificadores, pub tecla: Tecla }`
  - `pub enum Tecla { Letra(char), Digito(u8), Funcion(u8), Imprimir, Insertar }`
  - `impl FromStr for Atajo` (error `ErrorAtajo`), `impl Display for Atajo`
  - `impl Atajo { pub fn modificadores_win32(&self) -> u32; pub fn tecla_win32(&self) -> u32 }`
  - Serde vía `#[serde(try_from = "String", into = "String")]`

- [ ] **Step 1: Añadir dependencias**

Run:
```bash
cargo add serde --features derive -p pixpin-shell
cargo add thiserror -p pixpin-shell
```

- [ ] **Step 2: Escribir los tests que fallan**

Crear `crates/pixpin-shell/src/atajo.rs` con sólo los tests:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn parsea_la_combinacion_por_defecto() {
        let a: Atajo = "Ctrl+Alt+X".parse().unwrap();
        assert!(a.modificadores.ctrl);
        assert!(a.modificadores.alt);
        assert!(!a.modificadores.shift);
        assert!(!a.modificadores.win);
        assert_eq!(a.tecla, Tecla::Letra('X'));
    }

    #[test]
    fn el_formato_es_reversible() {
        // Ojo: se escriben ya en forma canonica (Ctrl, Alt, Shift, Win). Ese
        // orden fijo es lo que hace estable la ida y vuelta.
        for texto in ["Ctrl+Alt+X", "Ctrl+Shift+F12", "Alt+Win+9", "Ctrl+Impr"] {
            let a: Atajo = texto.parse().unwrap();
            assert_eq!(a.to_string(), texto, "no sobrevive la ida y vuelta");
        }
    }

    #[test]
    fn no_distingue_mayusculas_al_leer() {
        let a: Atajo = "ctrl+alt+x".parse().unwrap();
        let b: Atajo = "CTRL+ALT+X".parse().unwrap();
        assert_eq!(a, b);
        // Pero al escribir siempre sale en la forma canonica.
        assert_eq!(a.to_string(), "Ctrl+Alt+X");
    }

    #[test]
    fn rechaza_combinaciones_invalidas() {
        // Sin tecla final.
        assert!("Ctrl+".parse::<Atajo>().is_err());
        // Sin ningun modificador: RegisterHotKey lo aceptaria, pero secuestrar
        // una tecla suelta a nivel global rompe la escritura en cualquier app.
        assert!("X".parse::<Atajo>().is_err());
        // Tecla desconocida.
        assert!("Ctrl+Alt+Inexistente".parse::<Atajo>().is_err());
        // Modificador repetido.
        assert!("Ctrl+Ctrl+X".parse::<Atajo>().is_err());
        // Vacio.
        assert!("".parse::<Atajo>().is_err());
    }

    #[test]
    fn traduce_a_los_codigos_de_win32() {
        let a: Atajo = "Ctrl+Alt+X".parse().unwrap();
        // MOD_ALT = 0x0001, MOD_CONTROL = 0x0002
        assert_eq!(a.modificadores_win32(), 0x0001 | 0x0002);
        // El codigo virtual de una letra es su mayuscula ASCII.
        assert_eq!(a.tecla_win32(), 'X' as u32);

        let f: Atajo = "Ctrl+Shift+F12".parse().unwrap();
        // VK_F1 = 0x70, luego F12 = 0x7B
        assert_eq!(f.tecla_win32(), 0x7B);
    }

    #[test]
    fn se_serializa_como_texto_plano() {
        let a: Atajo = "Ctrl+Alt+X".parse().unwrap();
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, "\"Ctrl+Alt+X\"");
        let vuelta: Atajo = serde_json::from_str(&json).unwrap();
        assert_eq!(a, vuelta);
    }
}
```

Añadir `serde_json` como dependencia de desarrollo:

```bash
cargo add serde_json --dev -p pixpin-shell
```

- [ ] **Step 3: Ejecutar y comprobar que falla**

Run: `cargo test -p pixpin-shell atajo`
Expected: FAIL — no existen `Atajo`, `Tecla`, `Modificadores`.

- [ ] **Step 4: Implementar**

Añadir al principio de `crates/pixpin-shell/src/atajo.rs`:

```rust
//! Combinaciones de teclas globales.
//!
//! Vive en `pixpin-shell` y no en `pixpin-store` por la regla de capas: shell
//! es L1 y store es L2, asi que el tipo tiene que estar abajo para que ambos
//! puedan usarlo. `RegisterHotKey` tambien esta aqui, asi que es su sitio
//! natural.
//!
//! Se serializa como texto ("Ctrl+Alt+X") en vez de como estructura para que
//! el fichero TOML sea legible y editable a mano.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

// Constantes de Win32, repetidas aqui para que el parseo sea logica pura y no
// arrastre el crate `windows` a los tests.
const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const MOD_WIN: u32 = 0x0008;
const VK_F1: u32 = 0x70;
const VK_SNAPSHOT: u32 = 0x2C;
const VK_INSERT: u32 = 0x2D;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modificadores {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

impl Modificadores {
    fn alguno(&self) -> bool {
        self.ctrl || self.alt || self.shift || self.win
    }
}

/// La tecla final de la combinacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tecla {
    /// Siempre en mayuscula.
    Letra(char),
    /// 0..=9
    Digito(u8),
    /// 1..=24
    Funcion(u8),
    /// Impr Pant
    Imprimir,
    Insertar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Atajo {
    pub modificadores: Modificadores,
    pub tecla: Tecla,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ErrorAtajo {
    #[error("la combinacion esta vacia")]
    Vacia,
    #[error("falta la tecla final")]
    SinTecla,
    #[error(
        "hace falta al menos un modificador: una tecla suelta a nivel global \
         impediria escribirla en cualquier otra aplicacion"
    )]
    SinModificador,
    #[error("modificador repetido: `{0}`")]
    ModificadorRepetido(String),
    #[error("tecla desconocida: `{0}`")]
    TeclaDesconocida(String),
}

impl Atajo {
    /// Mascara de modificadores tal como la espera `RegisterHotKey`.
    pub fn modificadores_win32(&self) -> u32 {
        let m = self.modificadores;
        (if m.alt { MOD_ALT } else { 0 })
            | (if m.ctrl { MOD_CONTROL } else { 0 })
            | (if m.shift { MOD_SHIFT } else { 0 })
            | (if m.win { MOD_WIN } else { 0 })
    }

    /// Codigo de tecla virtual tal como lo espera `RegisterHotKey`.
    pub fn tecla_win32(&self) -> u32 {
        match self.tecla {
            Tecla::Letra(c) => c as u32,
            Tecla::Digito(d) => u32::from(b'0' + d),
            Tecla::Funcion(n) => VK_F1 + u32::from(n - 1),
            Tecla::Imprimir => VK_SNAPSHOT,
            Tecla::Insertar => VK_INSERT,
        }
    }
}

impl FromStr for Atajo {
    type Err = ErrorAtajo;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        let texto = texto.trim();
        if texto.is_empty() {
            return Err(ErrorAtajo::Vacia);
        }

        let partes: Vec<&str> = texto.split('+').map(str::trim).collect();
        if partes.iter().any(|p| p.is_empty()) {
            return Err(ErrorAtajo::SinTecla);
        }

        let (ultima, previas) = partes.split_last().expect("nunca vacio tras el trim");

        let mut modificadores = Modificadores::default();
        for parte in previas {
            let campo = match parte.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => &mut modificadores.ctrl,
                "alt" => &mut modificadores.alt,
                "shift" | "mayus" => &mut modificadores.shift,
                "win" => &mut modificadores.win,
                _ => return Err(ErrorAtajo::TeclaDesconocida((*parte).to_string())),
            };
            if *campo {
                return Err(ErrorAtajo::ModificadorRepetido((*parte).to_string()));
            }
            *campo = true;
        }

        if !modificadores.alguno() {
            return Err(ErrorAtajo::SinModificador);
        }

        let tecla = parsear_tecla(ultima)?;
        Ok(Atajo { modificadores, tecla })
    }
}

fn parsear_tecla(texto: &str) -> Result<Tecla, ErrorAtajo> {
    let arriba = texto.to_ascii_uppercase();

    if arriba.len() == 1 {
        let c = arriba.chars().next().expect("longitud comprobada");
        if c.is_ascii_alphabetic() {
            return Ok(Tecla::Letra(c));
        }
        if let Some(d) = c.to_digit(10) {
            return Ok(Tecla::Digito(d as u8));
        }
    }

    if let Some(resto) = arriba.strip_prefix('F') {
        if let Ok(n) = resto.parse::<u8>() {
            if (1..=24).contains(&n) {
                return Ok(Tecla::Funcion(n));
            }
        }
    }

    match arriba.as_str() {
        "IMPR" | "IMPRPANT" | "PRTSC" => Ok(Tecla::Imprimir),
        "INS" | "INSERT" | "INSERTAR" => Ok(Tecla::Insertar),
        _ => Err(ErrorAtajo::TeclaDesconocida(texto.to_string())),
    }
}

impl fmt::Display for Atajo {
    /// Forma canonica: siempre en el orden Ctrl, Alt, Shift, Win.
    ///
    /// El orden fijo es lo que hace que la ida y vuelta sea estable y que dos
    /// ficheros de ajustes equivalentes se vean iguales.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.modificadores;
        if m.ctrl {
            write!(f, "Ctrl+")?;
        }
        if m.alt {
            write!(f, "Alt+")?;
        }
        if m.shift {
            write!(f, "Shift+")?;
        }
        if m.win {
            write!(f, "Win+")?;
        }
        match self.tecla {
            Tecla::Letra(c) => write!(f, "{c}"),
            Tecla::Digito(d) => write!(f, "{d}"),
            Tecla::Funcion(n) => write!(f, "F{n}"),
            Tecla::Imprimir => write!(f, "Impr"),
            Tecla::Insertar => write!(f, "Ins"),
        }
    }
}

// Serde va por texto para que el TOML quede legible y editable a mano.
impl Serialize for Atajo {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Atajo {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)?;
        texto.parse().map_err(serde::de::Error::custom)
    }
}
```

Añadir a `crates/pixpin-shell/src/lib.rs`:

```rust
pub mod atajo;

pub use atajo::{Atajo, ErrorAtajo, Modificadores, Tecla};
```

- [ ] **Step 5: Ejecutar y comprobar que pasa**

Run: `cargo test -p pixpin-shell atajo`
Expected: PASS — los seis tests en verde.

- [ ] **Step 6: Commit**

```bash
git add crates/pixpin-shell
git commit -m "Tipo Atajo con parseo, forma canonica y traduccion a Win32

Se serializa como texto ('Ctrl+Alt+X') para que el TOML sea editable a mano,
y siempre se escribe en el orden Ctrl, Alt, Shift, Win: el orden fijo es lo
que hace estable la ida y vuelta y evita que dos ajustes equivalentes se vean
distintos.

Rechaza deliberadamente las teclas sueltas sin modificador. RegisterHotKey
las aceptaria, pero secuestrar una tecla a nivel global impide escribirla en
cualquier otra aplicacion.

Vive en pixpin-shell (L1) y no en pixpin-store (L2) por la regla de capas:
store necesita serializar atajos, asi que el tipo tiene que estar debajo."
```

---

## Task 4: Modelo de ajustes en TOML

**Files:**
- Create: `crates/pixpin-store/src/ajustes.rs`
- Modify: `crates/pixpin-store/src/lib.rs`
- Modify: `crates/pixpin-store/Cargo.toml`

**Interfaces:**
- Consumes: `pixpin_shell::Atajo` (Task 3), `Ubicacion` (Task 2).
- Produces:
  - `pub struct Ajustes { pub idioma: PreferenciaIdioma, pub atajos: Atajos, pub carpeta_capturas: Option<PathBuf>, pub formato_color: FormatoColor, pub arranque_con_windows: bool, pub limite_scroll_px: u32 }`
  - `pub struct Atajos { pub region: Atajo, pub copiar: Atajo, pub scroll: Atajo, pub cuentagotas: Atajo }`
  - `pub enum PreferenciaIdioma { Sistema, Español, Ingles }`
  - `pub enum FormatoColor { Hex, Rgb, Hsl }`
  - `pub fn cargar(ubicacion: &Ubicacion) -> Result<Ajustes, ErrorAjustes>`
  - `pub fn guardar(ubicacion: &Ubicacion, ajustes: &Ajustes) -> Result<(), ErrorAjustes>`

- [ ] **Step 1: Añadir dependencias**

Run:
```bash
cargo add serde --features derive -p pixpin-store
cargo add toml -p pixpin-store
cargo add thiserror -p pixpin-store
cargo add pixpin-shell --path crates/pixpin-shell -p pixpin-store
```

Verifica que el test de capas sigue pasando: `pixpin-store` (L2) → `pixpin-shell` (L1) es correcto.

Run: `cargo test -p pixpin --test capas`
Expected: PASS.

- [ ] **Step 2: Escribir los tests que fallan**

Crear `crates/pixpin-store/src/ajustes.rs` con sólo los tests:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    fn temporal(etiqueta: &str) -> Ubicacion {
        let dir = std::env::temp_dir().join(format!("pixpin-ajustes-{etiqueta}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Ubicacion::Instalado { raiz: dir }
    }

    #[test]
    fn los_valores_por_defecto_son_los_del_diseño() {
        let a = Ajustes::default();
        assert_eq!(a.atajos.region.to_string(), "Ctrl+Alt+X");
        assert_eq!(a.atajos.copiar.to_string(), "Ctrl+Alt+C");
        assert_eq!(a.atajos.scroll.to_string(), "Ctrl+Alt+S");
        assert_eq!(a.atajos.cuentagotas.to_string(), "Ctrl+Alt+D");
        assert_eq!(a.idioma, PreferenciaIdioma::Sistema);
        assert_eq!(a.formato_color, FormatoColor::Hex);
        assert!(!a.arranque_con_windows);
    }

    #[test]
    fn sobrevive_la_ida_y_vuelta_por_toml() {
        let mut original = Ajustes::default();
        original.idioma = PreferenciaIdioma::Ingles;
        original.arranque_con_windows = true;
        original.atajos.region = "Ctrl+Shift+F1".parse().unwrap();

        let texto = toml::to_string_pretty(&original).unwrap();
        let vuelta: Ajustes = toml::from_str(&texto).unwrap();

        assert_eq!(original, vuelta);
    }

    #[test]
    fn si_no_hay_fichero_se_usan_los_valores_por_defecto() {
        let u = temporal("sin-fichero");
        let a = cargar(&u).unwrap();
        assert_eq!(a, Ajustes::default());
    }

    #[test]
    fn un_fichero_a_medias_completa_con_los_valores_por_defecto() {
        // Es el caso real de un usuario que edita el TOML a mano y solo
        // escribe lo que quiere cambiar. No debe romper nada.
        let u = temporal("parcial");
        fs::write(u.fichero_ajustes(), "arranque_con_windows = true\n").unwrap();

        let a = cargar(&u).unwrap();

        assert!(a.arranque_con_windows);
        assert_eq!(a.atajos.region.to_string(), "Ctrl+Alt+X");
    }

    #[test]
    fn las_claves_desconocidas_se_ignoran() {
        // Compatibilidad hacia atras: un fichero escrito por una version mas
        // nueva no debe impedir que arranque una version mas vieja.
        let u = temporal("desconocidas");
        fs::write(
            u.fichero_ajustes(),
            "arranque_con_windows = true\nfuncion_del_futuro = 42\n",
        )
        .unwrap();

        let a = cargar(&u).unwrap();

        assert!(a.arranque_con_windows);
    }

    #[test]
    fn un_atajo_invalido_da_error_con_mensaje_util() {
        let u = temporal("atajo-malo");
        fs::write(u.fichero_ajustes(), "[atajos]\nregion = \"NoEsUnAtajo\"\n").unwrap();

        let e = cargar(&u).unwrap_err();

        assert!(
            e.to_string().contains("NoEsUnAtajo"),
            "el error debe decir que valor concreto esta mal, dijo: {e}"
        );
    }

    #[test]
    fn guardar_crea_el_directorio_si_no_existe() {
        let dir = std::env::temp_dir().join("pixpin-ajustes-crear/anidado");
        let _ = fs::remove_dir_all(dir.parent().unwrap());
        let u = Ubicacion::Instalado { raiz: dir.clone() };

        guardar(&u, &Ajustes::default()).unwrap();

        assert!(u.fichero_ajustes().is_file());
        assert_eq!(cargar(&u).unwrap(), Ajustes::default());
    }
}
```

- [ ] **Step 3: Ejecutar y comprobar que falla**

Run: `cargo test -p pixpin-store ajustes`
Expected: FAIL — no existe `Ajustes`.

- [ ] **Step 4: Implementar**

Añadir al principio de `crates/pixpin-store/src/ajustes.rs`:

```rust
//! Los ajustes de la aplicacion, en TOML.
//!
//! Dos reglas gobiernan el formato, y las dos existen para que el fichero se
//! pueda editar a mano sin miedo:
//!
//! - **Todo campo que falte se rellena con su valor por defecto.** Un usuario
//!   que solo quiere cambiar un atajo escribe dos lineas, no el fichero entero.
//! - **Las claves desconocidas se ignoran.** Un fichero escrito por una version
//!   mas nueva no impide arrancar a una mas vieja.

use std::fs;
use std::path::PathBuf;

use pixpin_shell::Atajo;
use serde::{Deserialize, Serialize};

use crate::rutas::Ubicacion;

#[derive(Debug, thiserror::Error)]
pub enum ErrorAjustes {
    #[error("no se pudo leer {ruta}: {fuente}")]
    Lectura { ruta: PathBuf, #[source] fuente: std::io::Error },
    #[error("no se pudo escribir {ruta}: {fuente}")]
    Escritura { ruta: PathBuf, #[source] fuente: std::io::Error },
    #[error("el fichero de ajustes tiene un error: {0}")]
    Formato(#[from] toml::de::Error),
    #[error("no se pudieron serializar los ajustes: {0}")]
    Serializacion(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PreferenciaIdioma {
    /// Se toma del idioma de Windows.
    #[default]
    Sistema,
    #[serde(rename = "es")]
    Español,
    #[serde(rename = "en")]
    Ingles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FormatoColor {
    #[default]
    Hex,
    Rgb,
    Hsl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Atajos {
    /// Capturar region y mostrar la barra de resultado.
    pub region: Atajo,
    /// Capturar region y copiar directo al portapapeles, sin confirmacion.
    pub copiar: Atajo,
    /// Captura larga con scroll.
    pub scroll: Atajo,
    /// Cuentagotas global.
    pub cuentagotas: Atajo,
}

impl Default for Atajos {
    fn default() -> Self {
        // `expect` es correcto aqui: si una constante del propio codigo no
        // parsea, es un fallo de programacion y debe verse en el primer test.
        Self {
            region: "Ctrl+Alt+X".parse().expect("atajo por defecto valido"),
            copiar: "Ctrl+Alt+C".parse().expect("atajo por defecto valido"),
            scroll: "Ctrl+Alt+S".parse().expect("atajo por defecto valido"),
            cuentagotas: "Ctrl+Alt+D".parse().expect("atajo por defecto valido"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ajustes {
    pub idioma: PreferenciaIdioma,
    pub atajos: Atajos,
    /// Si es `None` se usa la carpeta Imagenes del usuario.
    pub carpeta_capturas: Option<PathBuf>,
    pub formato_color: FormatoColor,
    pub arranque_con_windows: bool,
    /// Tope de altura de la captura con scroll. Sin el, una pagina infinita
    /// capturaria hasta agotar la memoria.
    pub limite_scroll_px: u32,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            idioma: PreferenciaIdioma::default(),
            atajos: Atajos::default(),
            carpeta_capturas: None,
            formato_color: FormatoColor::default(),
            arranque_con_windows: false,
            limite_scroll_px: 30_000,
        }
    }
}

/// Lee los ajustes. Si el fichero no existe, devuelve los valores por defecto.
pub fn cargar(ubicacion: &Ubicacion) -> Result<Ajustes, ErrorAjustes> {
    let ruta = ubicacion.fichero_ajustes();
    let texto = match fs::read_to_string(&ruta) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Ajustes::default()),
        Err(fuente) => return Err(ErrorAjustes::Lectura { ruta, fuente }),
    };
    Ok(toml::from_str(&texto)?)
}

/// Escribe los ajustes, creando el directorio si hace falta.
pub fn guardar(ubicacion: &Ubicacion, ajustes: &Ajustes) -> Result<(), ErrorAjustes> {
    let ruta = ubicacion.fichero_ajustes();
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre)
            .map_err(|fuente| ErrorAjustes::Escritura { ruta: padre.to_path_buf(), fuente })?;
    }
    let texto = toml::to_string_pretty(ajustes)?;
    fs::write(&ruta, texto).map_err(|fuente| ErrorAjustes::Escritura { ruta, fuente })
}
```

Añadir a `crates/pixpin-store/src/lib.rs`:

```rust
pub mod ajustes;

pub use ajustes::{Ajustes, Atajos, ErrorAjustes, FormatoColor, PreferenciaIdioma, cargar, guardar};
```

- [ ] **Step 5: Ejecutar y comprobar que pasa**

Run: `cargo test -p pixpin-store`
Expected: PASS — los siete tests de ajustes más los tres de rutas.

- [ ] **Step 6: Commit**

```bash
git add crates/pixpin-store
git commit -m "Ajustes en TOML pensados para editarse a mano

Dos reglas que existen para que el usuario pueda abrir el fichero sin miedo:
todo campo que falte se rellena con su valor por defecto, y las claves
desconocidas se ignoran. Lo primero permite escribir solo lo que se quiere
cambiar; lo segundo evita que un fichero de una version mas nueva impida
arrancar a una mas vieja.

Los cuatro atajos por defecto (Ctrl+Alt+X/C/S/D) quedan fijados aqui y
verificados por test contra el documento de diseño de S1."
```

---

## Task 5: Idiomas con Fluent

**Files:**
- Create: `crates/pixpin-store/src/idioma.rs`
- Create: `crates/pixpin-store/i18n/es-ES/main.ftl`
- Create: `crates/pixpin-store/i18n/en-US/main.ftl`
- Modify: `crates/pixpin-store/src/lib.rs`
- Modify: `crates/pixpin-store/Cargo.toml`

**Interfaces:**
- Consumes: `PreferenciaIdioma` (Task 4).
- Produces:
  - `pub enum Idioma { Español, Ingles }`
  - `pub fn resolver_idioma(locale_sistema: &str, preferencia: PreferenciaIdioma) -> Idioma`
  - `pub struct Catalogo`
  - `impl Catalogo { pub fn nuevo(idioma: Idioma) -> Self; pub fn t(&self, clave: &str) -> String; pub fn t_args(&self, clave: &str, args: &FluentArgs) -> String }`

- [ ] **Step 1: Añadir dependencias**

Run:
```bash
cargo add fluent-bundle -p pixpin-store
cargo add unic-langid -p pixpin-store
```

- [ ] **Step 2: Escribir los catálogos**

Crear `crates/pixpin-store/i18n/es-ES/main.ftl`:

```ftl
# PixPin Max — textos en español.
# Toda cadena visible al usuario vive aqui, nunca literal en el codigo.

app-nombre = PixPin Max

bandeja-capturar = Capturar
bandeja-ajustes = Ajustes
bandeja-salir = Salir

resultado-copiar = Copiar
resultado-guardar-como = Guardar como…
resultado-guardar = Guardar
resultado-descartar = Descartar

error-otra-instancia = PixPin Max ya se esta ejecutando.
error-atajo-ocupado = No se pudo registrar { $atajo }: otra aplicacion lo esta usando.
```

Crear `crates/pixpin-store/i18n/en-US/main.ftl`:

```ftl
# PixPin Max — English strings.

app-nombre = PixPin Max

bandeja-capturar = Capture
bandeja-ajustes = Settings
bandeja-salir = Exit

resultado-copiar = Copy
resultado-guardar-como = Save as…
resultado-guardar = Save
resultado-descartar = Discard

error-otra-instancia = PixPin Max is already running.
error-atajo-ocupado = Could not register { $atajo }: another application is using it.
```

- [ ] **Step 3: Escribir los tests que fallan**

Crear `crates/pixpin-store/src/idioma.rs` con sólo los tests:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_preferencia_explicita_gana_al_sistema() {
        assert_eq!(resolver_idioma("es-ES", PreferenciaIdioma::Ingles), Idioma::Ingles);
        assert_eq!(resolver_idioma("en-US", PreferenciaIdioma::Español), Idioma::Español);
    }

    #[test]
    fn en_modo_sistema_se_mira_el_locale() {
        assert_eq!(resolver_idioma("es-ES", PreferenciaIdioma::Sistema), Idioma::Español);
        assert_eq!(resolver_idioma("es-MX", PreferenciaIdioma::Sistema), Idioma::Español);
        assert_eq!(resolver_idioma("en-GB", PreferenciaIdioma::Sistema), Idioma::Ingles);
    }

    #[test]
    fn un_idioma_no_soportado_cae_a_ingles() {
        // Ingles y no español: es el reparto habitual para un idioma que no
        // conocemos, y quien tenga el sistema en aleman entendera antes ingles.
        assert_eq!(resolver_idioma("de-DE", PreferenciaIdioma::Sistema), Idioma::Ingles);
        assert_eq!(resolver_idioma("", PreferenciaIdioma::Sistema), Idioma::Ingles);
        assert_eq!(resolver_idioma("basura", PreferenciaIdioma::Sistema), Idioma::Ingles);
    }

    #[test]
    fn los_dos_catalogos_devuelven_texto_traducido() {
        let es = Catalogo::nuevo(Idioma::Español);
        let en = Catalogo::nuevo(Idioma::Ingles);

        assert_eq!(es.t("bandeja-salir"), "Salir");
        assert_eq!(en.t("bandeja-salir"), "Exit");
        // El nombre del producto no se traduce.
        assert_eq!(es.t("app-nombre"), "PixPin Max");
        assert_eq!(en.t("app-nombre"), "PixPin Max");
    }

    #[test]
    fn una_clave_que_falta_devuelve_la_propia_clave() {
        // Devolver la clave en vez de entrar en panico: un texto que falta es
        // feo, pero una aplicacion que se cierra por eso es inaceptable.
        let es = Catalogo::nuevo(Idioma::Español);
        assert_eq!(es.t("clave-que-no-existe"), "clave-que-no-existe");
    }

    #[test]
    fn los_dos_catalogos_tienen_exactamente_las_mismas_claves() {
        // Este test es la red que impide que una traduccion se quede atras.
        let claves_es = claves_de(include_str!("../i18n/es-ES/main.ftl"));
        let claves_en = claves_de(include_str!("../i18n/en-US/main.ftl"));

        let solo_es: Vec<_> = claves_es.difference(&claves_en).collect();
        let solo_en: Vec<_> = claves_en.difference(&claves_es).collect();

        assert!(
            solo_es.is_empty() && solo_en.is_empty(),
            "catalogos desincronizados. Solo en es-ES: {solo_es:?}. Solo en en-US: {solo_en:?}"
        );
    }

    /// Extrae los identificadores de un fichero .ftl sin usar el parser de
    /// Fluent: basta con las lineas que empiezan por identificador y `=`.
    fn claves_de(ftl: &str) -> std::collections::BTreeSet<String> {
        ftl.lines()
            .filter(|l| !l.trim_start().starts_with('#') && !l.starts_with(char::is_whitespace))
            .filter_map(|l| l.split_once('=' ))
            .map(|(clave, _)| clave.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect()
    }
}
```

- [ ] **Step 4: Ejecutar y comprobar que falla**

Run: `cargo test -p pixpin-store idioma`
Expected: FAIL — no existen `Idioma`, `resolver_idioma`, `Catalogo`.

- [ ] **Step 5: Implementar**

Añadir al principio de `crates/pixpin-store/src/idioma.rs`:

```rust
//! Idiomas de la interfaz, con Fluent.
//!
//! Se elige Fluent y no un mapa de cadenas porque maneja bien plurales y
//! genero, que en español hacen falta en cuanto aparece un contador.
//!
//! `resolver_idioma` recibe el locale como texto en vez de consultarlo al
//! sistema: este crate tiene `#![forbid(unsafe_code)]` y no puede llamar a
//! Win32. El ejecutable se lo pasa desde `pixpin_shell::entorno`.

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use unic_langid::LanguageIdentifier;

use crate::ajustes::PreferenciaIdioma;

const FTL_ES: &str = include_str!("../i18n/es-ES/main.ftl");
const FTL_EN: &str = include_str!("../i18n/en-US/main.ftl");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Idioma {
    Español,
    Ingles,
}

impl Idioma {
    fn etiqueta(self) -> &'static str {
        match self {
            Idioma::Español => "es-ES",
            Idioma::Ingles => "en-US",
        }
    }

    fn fuente(self) -> &'static str {
        match self {
            Idioma::Español => FTL_ES,
            Idioma::Ingles => FTL_EN,
        }
    }
}

/// Decide el idioma final. La preferencia explicita del usuario siempre gana.
pub fn resolver_idioma(locale_sistema: &str, preferencia: PreferenciaIdioma) -> Idioma {
    match preferencia {
        PreferenciaIdioma::Español => Idioma::Español,
        PreferenciaIdioma::Ingles => Idioma::Ingles,
        // Solo miramos la parte de idioma: es-MX y es-ES comparten catalogo.
        PreferenciaIdioma::Sistema => {
            if locale_sistema.split(['-', '_']).next().unwrap_or("") == "es" {
                Idioma::Español
            } else {
                Idioma::Ingles
            }
        }
    }
}

/// Catalogo de textos ya cargado.
pub struct Catalogo {
    bundle: FluentBundle<FluentResource>,
}

impl Catalogo {
    pub fn nuevo(idioma: Idioma) -> Self {
        let lang: LanguageIdentifier = idioma
            .etiqueta()
            .parse()
            .expect("las etiquetas de idioma son constantes del propio codigo");

        let recurso = FluentResource::try_new(idioma.fuente().to_string())
            .expect("los catalogos se validan por test en tiempo de compilacion");

        let mut bundle = FluentBundle::new_concurrent(vec![lang]);
        // Fluent inserta marcas de direccion invisibles alrededor de los
        // argumentos. Utiles en arabe o hebreo; aqui solo ensucian el texto y
        // rompen las comparaciones exactas de los tests.
        bundle.set_use_isolating(false);
        bundle
            .add_resource(recurso)
            .expect("no puede haber claves duplicadas en un unico recurso");

        Self { bundle }
    }

    /// Texto sin argumentos. Si la clave falta, devuelve la propia clave.
    pub fn t(&self, clave: &str) -> String {
        self.con_argumentos(clave, None)
    }

    /// Texto con argumentos, por ejemplo `{ $atajo }`.
    pub fn t_args(&self, clave: &str, args: &FluentArgs) -> String {
        self.con_argumentos(clave, Some(args))
    }

    fn con_argumentos(&self, clave: &str, args: Option<&FluentArgs>) -> String {
        let Some(mensaje) = self.bundle.get_message(clave) else {
            return clave.to_string();
        };
        let Some(patron) = mensaje.value() else {
            return clave.to_string();
        };
        let mut errores = Vec::new();
        let texto = self.bundle.format_pattern(patron, args, &mut errores);
        if !errores.is_empty() {
            tracing::warn!(clave, ?errores, "error al formatear un texto");
        }
        texto.into_owned()
    }
}
```

Añadir `tracing`:

```bash
cargo add tracing -p pixpin-store
```

Añadir a `crates/pixpin-store/src/lib.rs`:

```rust
pub mod idioma;

pub use idioma::{Catalogo, Idioma, resolver_idioma};
```

- [ ] **Step 6: Ejecutar y comprobar que pasa**

Run: `cargo test -p pixpin-store idioma`
Expected: PASS — los seis tests en verde, incluido el que compara las claves de los dos catálogos.

- [ ] **Step 7: Commit**

```bash
git add crates/pixpin-store
git commit -m "Idiomas con Fluent, y un test que impide que una traduccion se quede atras

Fluent en vez de un mapa de cadenas porque maneja plurales y genero, que en
español hacen falta en cuanto aparece un contador.

El test que de verdad importa es el que compara los identificadores de los dos
catalogos y falla si divergen: sin el, añadir un texto en español y olvidarlo
en ingles no se nota hasta que un usuario ve la clave cruda en pantalla.

Una clave que falta devuelve la propia clave en vez de entrar en panico. Un
texto sin traducir es feo; una aplicacion que se cierra por eso, inaceptable.

resolver_idioma recibe el locale como texto porque pixpin-store tiene
forbid(unsafe_code) y no puede llamar a Win32."
```

---

## Task 6: Entorno e instancia única

Primeras llamadas a Win32 del proyecto.

**Files:**
- Create: `crates/pixpin-shell/src/entorno.rs`
- Create: `crates/pixpin-shell/src/instancia.rs`
- Modify: `crates/pixpin-shell/src/lib.rs`
- Modify: `crates/pixpin-shell/Cargo.toml`

**Interfaces:**
- Consumes: nada.
- Produces:
  - `pub fn directorio_del_ejecutable() -> std::io::Result<PathBuf>`
  - `pub fn appdata() -> std::io::Result<PathBuf>`
  - `pub fn locale_del_sistema() -> String`
  - `pub struct InstanciaUnica` (libera el mutex en `Drop`)
  - `pub fn adquirir_instancia_unica() -> Result<InstanciaUnica, YaHayOtraInstancia>`

- [ ] **Step 1: Añadir el crate `windows`**

Run:
```bash
cargo add windows -p pixpin-shell --features \
  Win32_Foundation,Win32_System_Threading,Win32_Globalization,Win32_UI_Shell,Win32_System_Com
```

- [ ] **Step 2: Escribir los tests que fallan**

Crear `crates/pixpin-shell/src/instancia.rs` con sólo los tests:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_segunda_adquisicion_falla_mientras_viva_la_primera() {
        let primera = adquirir_instancia_unica().expect("la primera debe funcionar");

        assert!(
            adquirir_instancia_unica().is_err(),
            "la segunda instancia debe ser rechazada"
        );

        drop(primera);

        // Tras soltar la primera, el nombre queda libre otra vez.
        assert!(
            adquirir_instancia_unica().is_ok(),
            "al liberarse la primera, el nombre debe quedar disponible"
        );
    }
}
```

Crear `crates/pixpin-shell/src/entorno.rs` con sólo los tests:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_directorio_del_ejecutable_existe() {
        let dir = directorio_del_ejecutable().unwrap();
        assert!(dir.is_dir(), "{dir:?} deberia ser un directorio");
    }

    #[test]
    fn appdata_existe() {
        let dir = appdata().unwrap();
        assert!(dir.is_dir(), "{dir:?} deberia ser un directorio");
    }

    #[test]
    fn el_locale_tiene_forma_de_etiqueta_de_idioma() {
        let l = locale_del_sistema();
        assert!(!l.is_empty(), "el locale no puede venir vacio");
        assert!(
            l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "locale con forma rara: {l}"
        );
    }
}
```

- [ ] **Step 3: Ejecutar y comprobar que falla**

Run: `cargo test -p pixpin-shell entorno instancia`
Expected: FAIL — no existen las funciones.

- [ ] **Step 4: Implementar el entorno**

Añadir al principio de `crates/pixpin-shell/src/entorno.rs`:

```rust
//! Lo que hay que preguntarle a Windows antes de arrancar.
//!
//! Este modulo existe para que los crates con `forbid(unsafe_code)` no tengan
//! que llamar a Win32: reciben estos valores ya resueltos como parametros.

use std::path::PathBuf;

use windows::Win32::Globalization::GetUserDefaultLocaleName;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
use windows::core::PWSTR;

/// Directorio donde vive `pixpinmax.exe`.
pub fn directorio_del_ejecutable() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    exe.parent()
        .map(PathBuf::from)
        .ok_or_else(|| std::io::Error::other("el ejecutable no tiene directorio padre"))
}

/// `%APPDATA%` (la carpeta itinerante del usuario).
///
/// Se usa `SHGetKnownFolderPath` y no la variable de entorno porque la
/// variable se puede manipular y no siempre esta presente en sesiones de
/// servicio.
pub fn appdata() -> std::io::Result<PathBuf> {
    // SAFETY: SHGetKnownFolderPath escribe un puntero a cadena UTF-16
    // terminada en cero que debemos liberar con CoTaskMemFree. Se lee antes de
    // liberar y no se guarda ninguna referencia despues.
    unsafe {
        let ruta: PWSTR = SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None)
            .map_err(|e| std::io::Error::other(format!("SHGetKnownFolderPath fallo: {e}")))?;
        let texto = ruta.to_string().map_err(std::io::Error::other)?;
        CoTaskMemFree(Some(ruta.0 as *const _));
        Ok(PathBuf::from(texto))
    }
}

/// Etiqueta de idioma del usuario, por ejemplo `es-ES`.
///
/// Si Windows no la devuelve se asume `en-US`: es preferible una interfaz en
/// ingles a no arrancar.
pub fn locale_del_sistema() -> String {
    const MAX: usize = 85; // LOCALE_NAME_MAX_LENGTH
    let mut buffer = [0u16; MAX];

    // SAFETY: se pasa un buffer propio de tamaño conocido y la funcion
    // devuelve cuantos u16 escribio, incluido el cero final.
    let escritos = unsafe { GetUserDefaultLocaleName(&mut buffer) };

    if escritos <= 1 {
        return "en-US".to_string();
    }
    String::from_utf16_lossy(&buffer[..(escritos as usize - 1)])
}
```

- [ ] **Step 5: Implementar la instancia única**

Añadir al principio de `crates/pixpin-shell/src/instancia.rs`:

```rust
//! Una sola PixPin Max a la vez.
//!
//! Sin esto, dos copias pelearian por los mismos atajos globales: la segunda
//! fallaria al registrarlos y el usuario veria una aplicacion que "a veces no
//! responde al atajo", que es de los fallos mas dificiles de diagnosticar.

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

/// Devuelto cuando ya hay otra copia en marcha.
#[derive(Debug, thiserror::Error)]
#[error("ya hay otra instancia de PixPin Max en marcha")]
pub struct YaHayOtraInstancia;

/// Mientras este valor viva, ninguna otra copia puede arrancar.
pub struct InstanciaUnica {
    handle: HANDLE,
}

impl Drop for InstanciaUnica {
    fn drop(&mut self) {
        // SAFETY: `handle` viene de CreateMutexW, no se ha cerrado antes, y
        // este tipo no es Clone ni Copy, asi que se cierra exactamente una vez.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

pub fn adquirir_instancia_unica() -> Result<InstanciaUnica, YaHayOtraInstancia> {
    // El prefijo Local\ limita el ambito a la sesion del usuario: dos usuarios
    // distintos en el mismo equipo si pueden tener cada uno su PixPin Max.
    let nombre = w!(r"Local\PixPinMax-instancia-unica");

    // SAFETY: `nombre` es un literal UTF-16 estatico terminado en cero.
    // CreateMutexW devuelve un handle valido o un error; GetLastError se
    // consulta inmediatamente despues, antes de cualquier otra llamada.
    let (handle, ya_existia) = unsafe {
        let handle = CreateMutexW(None, true, nombre)
            .map_err(|_| YaHayOtraInstancia)?;
        let ya_existia = GetLastError() == ERROR_ALREADY_EXISTS;
        (handle, ya_existia)
    };

    if ya_existia {
        // SAFETY: handle recien creado y valido; se cierra una sola vez.
        unsafe {
            let _ = CloseHandle(handle);
        }
        return Err(YaHayOtraInstancia);
    }

    Ok(InstanciaUnica { handle })
}
```

Añadir a `crates/pixpin-shell/src/lib.rs`:

```rust
pub mod entorno;
pub mod instancia;

pub use entorno::{appdata, directorio_del_ejecutable, locale_del_sistema};
pub use instancia::{InstanciaUnica, YaHayOtraInstancia, adquirir_instancia_unica};
```

- [ ] **Step 6: Ejecutar y comprobar que pasa**

Run: `cargo test -p pixpin-shell -- --test-threads=1`
Expected: PASS.

**Por qué `--test-threads=1`:** el test de instancia única compite consigo mismo si dos tests corren a la vez, porque el nombre del mutex es global. Añade esta nota como comentario en el módulo de pruebas para que nadie lo descubra por las malas.

- [ ] **Step 7: Commit**

```bash
git add crates/pixpin-shell
git commit -m "Entorno e instancia unica: primeras llamadas a Win32

El mutex con nombre impide que arranquen dos copias. Sin el, la segunda
fallaria al registrar los atajos globales y el usuario veria una aplicacion
que 'a veces no responde al atajo': de los fallos mas dificiles de
diagnosticar. El prefijo Local\\ deja que dos usuarios distintos del mismo
equipo tengan cada uno la suya.

entorno.rs existe para que los crates con forbid(unsafe_code) reciban el
locale y las rutas ya resueltos en vez de tener que llamar a Win32. APPDATA
se pide con SHGetKnownFolderPath y no por variable de entorno, que se puede
manipular y falta en sesiones de servicio.

Cada bloque unsafe lleva su comentario SAFETY."
```

---

## Task 7: Ventana sólo-mensajes y bucle dirigido por eventos

El corazón del proceso. Una ventana invisible que recibe los atajos y los mensajes de la bandeja, y un bucle que **duerme** cuando no hay nada que hacer — de ahí sale el 0% de CPU en reposo del presupuesto.

**Files:**
- Create: `crates/pixpin-shell/src/ventana.rs`
- Modify: `crates/pixpin-shell/src/lib.rs`
- Modify: `crates/pixpin-shell/Cargo.toml`

**Interfaces:**
- Consumes: nada.
- Produces:
  - `pub enum Evento { Atajo(u32), MenuCapturar, MenuAjustes, MenuSalir, IconoPulsado }`
  - `pub struct VentanaMensajes`
  - `impl VentanaMensajes { pub fn nueva() -> windows::core::Result<Self>; pub fn handle(&self) -> HWND; pub fn ejecutar(self, al_recibir: impl FnMut(Evento) -> Continuar) }`
  - `pub enum Continuar { Si, No }`

- [ ] **Step 1: Ampliar las características del crate `windows`**

Run:
```bash
cargo add windows -p pixpin-shell --features \
  Win32_Foundation,Win32_System_Threading,Win32_Globalization,Win32_UI_Shell,Win32_System_Com,Win32_UI_WindowsAndMessaging,Win32_System_LibraryLoader
```

- [ ] **Step 2: Escribir el test que falla**

Crear `crates/pixpin-shell/src/ventana.rs` con sólo los tests:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_crea_y_se_destruye_sin_fugas() {
        let v = VentanaMensajes::nueva().expect("deberia poder crearse");
        assert!(!v.handle().is_invalid(), "el handle no puede ser invalido");
        drop(v);

        // Crear una segunda tras destruir la primera comprueba que la clase de
        // ventana se registra de forma reentrante y que no queda basura.
        let otra = VentanaMensajes::nueva().expect("la segunda tambien");
        assert!(!otra.handle().is_invalid());
    }
}
```

- [ ] **Step 3: Ejecutar y comprobar que falla**

Run: `cargo test -p pixpin-shell ventana -- --test-threads=1`
Expected: FAIL — no existe `VentanaMensajes`.

- [ ] **Step 4: Implementar**

Añadir al principio de `crates/pixpin-shell/src/ventana.rs`:

```rust
//! La ventana invisible que recibe todo y el bucle que duerme.
//!
//! Es una ventana `HWND_MESSAGE`: no se dibuja, no sale en la barra de tareas
//! y no aparece con Alt+Tab, pero recibe mensajes. Es donde llegan `WM_HOTKEY`
//! y las notificaciones de la bandeja.
//!
//! **El bucle usa `GetMessageW`, que bloquea el hilo hasta que llega algo.**
//! Esa eleccion es la que cumple el objetivo de 0% de CPU en reposo del
//! presupuesto de rendimiento. Un bucle con `PeekMessageW` giraria sin parar y
//! se comeria la bateria de un portatil sin hacer nada.

use std::cell::RefCell;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    HWND_MESSAGE, MSG, PostQuitMessage, RegisterClassExW, TranslateMessage, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_APP, WM_HOTKEY, WNDCLASSEXW,
};
use windows::core::{Result as WinResult, w};

/// Mensaje propio para las notificaciones del icono de bandeja.
pub const WM_BANDEJA: u32 = WM_APP + 1;

/// Identificadores de los elementos del menu de la bandeja.
pub const ID_MENU_CAPTURAR: u32 = 1;
pub const ID_MENU_AJUSTES: u32 = 2;
pub const ID_MENU_SALIR: u32 = 3;

/// Lo que le puede pasar a la aplicacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evento {
    /// Se pulso un atajo global. El numero es el identificador con el que se
    /// registro (ver `atajos.rs`).
    Atajo(u32),
    MenuCapturar,
    MenuAjustes,
    MenuSalir,
    /// Clic izquierdo en el icono de la bandeja.
    IconoPulsado,
}

/// Que hacer despues de atender un evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Continuar {
    Si,
    No,
}

thread_local! {
    /// Cola de eventos traducidos por el `WndProc`.
    ///
    /// Se usa un `thread_local` en vez de guardar un puntero al callback en
    /// los datos de la ventana porque el `WndProc` es una funcion `extern
    /// "system"` que no puede capturar entorno, y porque toda la interaccion
    /// con ventanas ocurre en el hilo de interfaz por exigencia de Win32.
    static PENDIENTES: RefCell<Vec<Evento>> = const { RefCell::new(Vec::new()) };
}

pub struct VentanaMensajes {
    hwnd: HWND,
}

impl VentanaMensajes {
    pub fn nueva() -> WinResult<Self> {
        const CLASE: windows::core::PCWSTR = w!("PixPinMaxVentanaMensajes");

        // SAFETY: GetModuleHandleW(None) devuelve el modulo del proceso
        // actual, que siempre existe mientras el proceso vive.
        let instancia = unsafe { GetModuleHandleW(None)? };

        let clase = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(procedimiento),
            hInstance: instancia.into(),
            lpszClassName: CLASE,
            ..Default::default()
        };

        // SAFETY: `clase` esta completamente inicializada y su `lpszClassName`
        // apunta a un literal estatico. Registrar la misma clase dos veces
        // devuelve 0 y pone ERROR_CLASS_ALREADY_EXISTS, que aqui es benigno:
        // significa que otra VentanaMensajes ya la registro.
        unsafe {
            RegisterClassExW(&clase);
        }

        // SAFETY: la clase esta registrada y HWND_MESSAGE es el padre valido
        // para una ventana solo-mensajes.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                CLASE,
                w!("PixPin Max"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instancia.into()),
                None,
            )?
        };

        Ok(Self { hwnd })
    }

    pub fn handle(&self) -> HWND {
        self.hwnd
    }

    /// Bucle principal. Bloquea el hilo hasta que `al_recibir` devuelve
    /// `Continuar::No` o llega `WM_QUIT`.
    pub fn ejecutar(self, mut al_recibir: impl FnMut(Evento) -> Continuar) {
        let mut mensaje = MSG::default();
        loop {
            // SAFETY: `mensaje` es una estructura propia y valida. GetMessageW
            // devuelve 0 al recibir WM_QUIT y -1 si hay error.
            let resultado = unsafe { GetMessageW(&mut mensaje, None, 0, 0) };
            if resultado.0 <= 0 {
                break;
            }

            // SAFETY: `mensaje` viene de GetMessageW y es valido.
            unsafe {
                let _ = TranslateMessage(&mensaje);
                DispatchMessageW(&mensaje);
            }

            let eventos: Vec<Evento> = PENDIENTES.with(|p| p.borrow_mut().drain(..).collect());
            for evento in eventos {
                if al_recibir(evento) == Continuar::No {
                    return;
                }
            }
        }
    }
}

impl Drop for VentanaMensajes {
    fn drop(&mut self) {
        // SAFETY: `hwnd` viene de CreateWindowExW y no se ha destruido antes;
        // este tipo no es Clone ni Copy.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Traduce mensajes de Win32 a `Evento` y los deja en la cola del hilo.
///
/// No hace trabajo real: cuanto antes vuelva, mas fluido va todo lo demas.
extern "system" fn procedimiento(
    hwnd: HWND,
    mensaje: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{WM_COMMAND, WM_DESTROY, WM_LBUTTONUP};

    let evento = match mensaje {
        WM_HOTKEY => Some(Evento::Atajo(wparam.0 as u32)),
        WM_COMMAND => match (wparam.0 & 0xFFFF) as u32 {
            ID_MENU_CAPTURAR => Some(Evento::MenuCapturar),
            ID_MENU_AJUSTES => Some(Evento::MenuAjustes),
            ID_MENU_SALIR => Some(Evento::MenuSalir),
            _ => None,
        },
        WM_BANDEJA if (lparam.0 as u32) == WM_LBUTTONUP => Some(Evento::IconoPulsado),
        WM_DESTROY => {
            // SAFETY: llamada sin argumentos que solo encola WM_QUIT.
            unsafe { PostQuitMessage(0) };
            None
        }
        _ => None,
    };

    if let Some(evento) = evento {
        PENDIENTES.with(|p| p.borrow_mut().push(evento));
        return LRESULT(0);
    }

    // SAFETY: delegar en el procedimiento por defecto siempre es valido para
    // los mensajes que no tratamos.
    unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
}
```

Añadir a `crates/pixpin-shell/src/lib.rs`:

```rust
pub mod ventana;

pub use ventana::{
    Continuar, Evento, ID_MENU_AJUSTES, ID_MENU_CAPTURAR, ID_MENU_SALIR, VentanaMensajes,
    WM_BANDEJA,
};
```

- [ ] **Step 5: Ejecutar y comprobar que pasa**

Run: `cargo test -p pixpin-shell -- --test-threads=1`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/pixpin-shell
git commit -m "Ventana solo-mensajes y bucle que duerme

Una ventana HWND_MESSAGE: invisible, fuera de la barra de tareas y de
Alt+Tab, pero recibe WM_HOTKEY y las notificaciones de la bandeja.

El bucle usa GetMessageW, que bloquea el hilo hasta que llega algo. Esa
eleccion es la que cumple el 0% de CPU en reposo del presupuesto: un bucle
con PeekMessageW giraria sin parar y se comeria la bateria de un portatil
sin hacer nada.

El WndProc solo traduce mensajes a Evento y los encola; el trabajo real lo
hace el bucle. Cuanto antes vuelva el WndProc, mas fluido va todo lo demas."
```

---

## Task 8: Registro de atajos globales

**Files:**
- Create: `crates/pixpin-shell/src/atajos.rs`
- Modify: `crates/pixpin-shell/src/lib.rs`

**Interfaces:**
- Consumes: `Atajo` (Task 3), `VentanaMensajes` (Task 7).
- Produces:
  - `pub const ID_REGION: u32 = 1; ID_COPIAR: u32 = 2; ID_SCROLL: u32 = 3; ID_CUENTAGOTAS: u32 = 4;`
  - `pub struct AtajosRegistrados` (los libera en `Drop`)
  - `pub fn registrar(hwnd: HWND, peticiones: &[(u32, Atajo)]) -> (AtajosRegistrados, Vec<(u32, Atajo)>)` — devuelve los registrados y **la lista de los que fallaron**

- [ ] **Step 1: Escribir el test que falla**

Crear `crates/pixpin-shell/src/atajos.rs` con sólo los tests:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::ventana::VentanaMensajes;

    #[test]
    fn registra_y_libera_una_combinacion_poco_usada() {
        let v = VentanaMensajes::nueva().unwrap();
        // Combinacion rara a proposito para no chocar con nada real.
        let raro: Atajo = "Ctrl+Alt+Shift+F24".parse().unwrap();

        let (registrados, fallidos) = registrar(v.handle(), &[(ID_REGION, raro)]);

        assert!(fallidos.is_empty(), "no deberia fallar: {fallidos:?}");
        drop(registrados);

        // Tras liberar, la misma combinacion debe poder registrarse otra vez.
        let (otros, fallidos) = registrar(v.handle(), &[(ID_REGION, raro)]);
        assert!(fallidos.is_empty(), "tras liberar deberia poder repetirse");
        drop(otros);
    }

    #[test]
    fn un_atajo_ocupado_se_informa_en_vez_de_abortar() {
        // Registrar dos veces la misma combinacion: la segunda choca.
        let v = VentanaMensajes::nueva().unwrap();
        let raro: Atajo = "Ctrl+Alt+Shift+F23".parse().unwrap();

        let (primeros, _) = registrar(v.handle(), &[(ID_REGION, raro)]);
        let (segundos, fallidos) = registrar(v.handle(), &[(ID_COPIAR, raro)]);

        assert_eq!(fallidos.len(), 1, "el choque debe reportarse");
        assert_eq!(fallidos[0].0, ID_COPIAR);

        drop(segundos);
        drop(primeros);
    }
}
```

- [ ] **Step 2: Ejecutar y comprobar que falla**

Run: `cargo test -p pixpin-shell atajos -- --test-threads=1`
Expected: FAIL — no existe `registrar`.

- [ ] **Step 3: Implementar**

Añadir al principio de `crates/pixpin-shell/src/atajos.rs`:

```rust
//! Registro de los atajos globales.
//!
//! **Un atajo ocupado no es un error fatal.** Otra aplicacion puede tener ya
//! Ctrl+Alt+X, y cerrarse por eso seria desproporcionado: se registra todo lo
//! que se pueda, se devuelve la lista de los que fallaron, y el ejecutable
//! avisa al usuario para que elija otro.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey,
};

use crate::atajo::Atajo;

pub const ID_REGION: u32 = 1;
pub const ID_COPIAR: u32 = 2;
pub const ID_SCROLL: u32 = 3;
pub const ID_CUENTAGOTAS: u32 = 4;

/// Mientras esto viva, los atajos siguen registrados.
pub struct AtajosRegistrados {
    hwnd: HWND,
    ids: Vec<u32>,
}

impl Drop for AtajosRegistrados {
    fn drop(&mut self) {
        for id in &self.ids {
            // SAFETY: cada id se registro con exito sobre este mismo hwnd y no
            // se ha liberado antes; este tipo no es Clone ni Copy.
            unsafe {
                let _ = UnregisterHotKey(Some(self.hwnd), *id as i32);
            }
        }
    }
}

/// Registra todas las peticiones que pueda.
///
/// Devuelve el guardia con las que funcionaron y la lista de las que no.
pub fn registrar(hwnd: HWND, peticiones: &[(u32, Atajo)]) -> (AtajosRegistrados, Vec<(u32, Atajo)>) {
    let mut ids = Vec::new();
    let mut fallidos = Vec::new();

    for (id, atajo) in peticiones {
        // SAFETY: `hwnd` es una ventana valida de este hilo; los codigos vienen
        // de `Atajo`, que solo produce combinaciones bien formadas.
        let ok = unsafe {
            RegisterHotKey(
                Some(hwnd),
                *id as i32,
                HOT_KEY_MODIFIERS(atajo.modificadores_win32()),
                atajo.tecla_win32(),
            )
        };

        if ok.is_ok() {
            ids.push(*id);
        } else {
            fallidos.push((*id, *atajo));
        }
    }

    (AtajosRegistrados { hwnd, ids }, fallidos)
}
```

Ampliar las características del crate `windows`:

```bash
cargo add windows -p pixpin-shell --features \
  Win32_Foundation,Win32_System_Threading,Win32_Globalization,Win32_UI_Shell,Win32_System_Com,Win32_UI_WindowsAndMessaging,Win32_System_LibraryLoader,Win32_UI_Input_KeyboardAndMouse
```

Añadir a `crates/pixpin-shell/src/lib.rs`:

```rust
pub mod atajos;

pub use atajos::{
    AtajosRegistrados, ID_COPIAR, ID_CUENTAGOTAS, ID_REGION, ID_SCROLL, registrar,
};
```

- [ ] **Step 4: Ejecutar y comprobar que pasa**

Run: `cargo test -p pixpin-shell -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/pixpin-shell
git commit -m "Atajos globales: un choque se informa, no aborta

Otra aplicacion puede tener ya Ctrl+Alt+X. Cerrarse por eso seria
desproporcionado, asi que registrar() devuelve los que funcionaron y la lista
de los que fallaron, para que el ejecutable avise al usuario y le deje elegir
otra combinacion.

El guardia AtajosRegistrados los libera en Drop, de forma que salir de la
aplicacion nunca deja combinaciones secuestradas."
```

---

## Task 9: Icono y menú de bandeja

**Files:**
- Create: `crates/pixpin-shell/src/bandeja.rs`
- Create: `apps/pixpin/recursos/pixpinmax.ico`
- Modify: `crates/pixpin-shell/src/lib.rs`

**Interfaces:**
- Consumes: `VentanaMensajes` (Task 7), `WM_BANDEJA`, los `ID_MENU_*` (Task 7).
- Produces:
  - `pub struct Bandeja` (retira el icono en `Drop`)
  - `impl Bandeja { pub fn nueva(hwnd: HWND, titulo: &str) -> WinResult<Self>; pub fn mostrar_menu(&self, hwnd: HWND, etiquetas: &EtiquetasMenu) -> WinResult<()> }`
  - `pub struct EtiquetasMenu { pub capturar: String, pub ajustes: String, pub salir: String }`

- [ ] **Step 1: Crear el icono**

Genera un `.ico` de 256×256 con los tamaños 16, 32, 48, 256 incrustados y guárdalo en `apps/pixpin/recursos/pixpinmax.ico`. Un cuadrado con una chincheta basta como marcador; el diseño definitivo no bloquea este plan.

- [ ] **Step 2: Escribir el test que falla**

Crear `crates/pixpin-shell/src/bandeja.rs` con sólo los tests:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::ventana::VentanaMensajes;

    #[test]
    fn se_añade_y_se_retira_el_icono() {
        let v = VentanaMensajes::nueva().unwrap();
        let b = Bandeja::nueva(v.handle(), "PixPin Max — prueba").expect("deberia añadirse");
        drop(b);

        // Poder añadir un segundo icono tras retirar el primero demuestra que
        // el Drop hizo su trabajo y no quedo un fantasma en la bandeja.
        let otra = Bandeja::nueva(v.handle(), "PixPin Max — prueba 2").expect("y otra vez");
        drop(otra);
    }
}
```

- [ ] **Step 3: Ejecutar y comprobar que falla**

Run: `cargo test -p pixpin-shell bandeja -- --test-threads=1`
Expected: FAIL — no existe `Bandeja`.

- [ ] **Step 4: Implementar**

Añadir al principio de `crates/pixpin-shell/src/bandeja.rs`:

```rust
//! Icono de la bandeja del sistema y su menu contextual.
//!
//! El icono es la unica presencia visible de PixPin Max cuando no estas
//! capturando. Se retira en `Drop` para que cerrar la aplicacion no deje un
//! icono fantasma que solo desaparece al pasar el raton por encima.

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, IDI_APPLICATION, LoadIconW,
    MF_SEPARATOR, MF_STRING, SetForegroundWindow, TPM_RIGHTBUTTON, TrackPopupMenu,
};
use windows::core::{HSTRING, Result as WinResult};

use crate::ventana::{ID_MENU_AJUSTES, ID_MENU_CAPTURAR, ID_MENU_SALIR, WM_BANDEJA};

/// Identificador del icono dentro de nuestra propia ventana. Solo hay uno.
const ID_ICONO: u32 = 1;

/// Textos del menu, ya traducidos por el catalogo Fluent.
pub struct EtiquetasMenu {
    pub capturar: String,
    pub ajustes: String,
    pub salir: String,
}

pub struct Bandeja {
    datos: NOTIFYICONDATAW,
}

impl Bandeja {
    pub fn nueva(hwnd: HWND, titulo: &str) -> WinResult<Self> {
        // SAFETY: IDI_APPLICATION es un icono del sistema siempre disponible;
        // pasar None como instancia indica que es predefinido.
        let icono = unsafe { LoadIconW(None, IDI_APPLICATION)? };

        let mut datos = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: ID_ICONO,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_BANDEJA,
            hIcon: icono,
            ..Default::default()
        };

        // szTip es un array de 128 u16 terminado en cero. Se trunca a 127 para
        // dejar sitio al cero final.
        for (destino, origen) in datos.szTip.iter_mut().zip(titulo.encode_utf16().take(127)) {
            *destino = origen;
        }

        // SAFETY: `datos` esta completamente inicializada, su cbSize es
        // correcto y hWnd es una ventana valida de este proceso.
        unsafe {
            Shell_NotifyIconW(NIM_ADD, &datos).ok()?;
        }

        Ok(Self { datos })
    }

    /// Muestra el menu contextual donde este el raton.
    pub fn mostrar_menu(&self, hwnd: HWND, etiquetas: &EtiquetasMenu) -> WinResult<()> {
        // SAFETY: todas estas llamadas operan sobre un menu recien creado por
        // nosotros y sobre una ventana valida. El menu se destruye siempre
        // antes de salir de la funcion.
        unsafe {
            let menu = CreatePopupMenu()?;
            AppendMenuW(
                menu,
                MF_STRING,
                ID_MENU_CAPTURAR as usize,
                &HSTRING::from(etiquetas.capturar.as_str()),
            )?;
            AppendMenuW(
                menu,
                MF_STRING,
                ID_MENU_AJUSTES as usize,
                &HSTRING::from(etiquetas.ajustes.as_str()),
            )?;
            AppendMenuW(menu, MF_SEPARATOR, 0, None)?;
            AppendMenuW(
                menu,
                MF_STRING,
                ID_MENU_SALIR as usize,
                &HSTRING::from(etiquetas.salir.as_str()),
            )?;

            let mut punto = POINT::default();
            GetCursorPos(&mut punto)?;

            // Sin esta llamada el menu no se cierra al hacer clic fuera. Es un
            // requisito documentado de TrackPopupMenu que se olvida a menudo.
            let _ = SetForegroundWindow(hwnd);

            let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON, punto.x, punto.y, None, hwnd, None);
            let _ = DestroyMenu(menu);
        }
        Ok(())
    }
}

impl Drop for Bandeja {
    fn drop(&mut self) {
        // SAFETY: `datos` describe un icono añadido por nosotros con NIM_ADD y
        // aun no retirado; este tipo no es Clone ni Copy.
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.datos);
        }
    }
}
```

Ampliar las características del crate `windows` con `Win32_UI_Shell` (ya está) y comprobar que `NOTIFYICONDATAW` resuelve.

Añadir a `crates/pixpin-shell/src/lib.rs`:

```rust
pub mod bandeja;

pub use bandeja::{Bandeja, EtiquetasMenu};
```

- [ ] **Step 5: Ejecutar y comprobar que pasa**

Run: `cargo test -p pixpin-shell -- --test-threads=1`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/pixpin-shell apps/pixpin/recursos
git commit -m "Icono de bandeja con menu contextual traducible

El icono se retira en Drop para que cerrar la aplicacion no deje un icono
fantasma que solo desaparece cuando pasas el raton por encima.

mostrar_menu llama a SetForegroundWindow antes de TrackPopupMenu: es un
requisito documentado que se olvida a menudo, y sin el el menu no se cierra
al hacer clic fuera.

Las etiquetas llegan ya traducidas desde el catalogo Fluent; este modulo no
contiene ni un texto literal."
```

---

## Task 10: Arranque con Windows

**Files:**
- Create: `crates/pixpin-shell/src/arranque.rs`
- Modify: `crates/pixpin-shell/src/lib.rs`

**Interfaces:**
- Consumes: nada.
- Produces:
  - `pub fn permitido(es_portable: bool) -> bool`
  - `pub fn establecer(activo: bool, es_portable: bool, ruta_exe: &Path) -> Result<(), ErrorArranque>`
  - `pub fn esta_activo() -> bool`
  - `pub enum ErrorArranque { ModoPortable, Registro(std::io::Error) }`

- [ ] **Step 1: Escribir los tests que fallan**

Crear `crates/pixpin-shell/src/arranque.rs` con sólo los tests:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;
    use std::path::Path;

    #[test]
    fn el_modo_portable_no_permite_tocar_el_registro() {
        // La promesa del modo portable es "cero rastro en el equipo". Escribir
        // en HKCU\...\Run la romperia, y ademas dejaria una entrada apuntando
        // a un USB que manaña no esta.
        assert!(!permitido(true));
        assert!(permitido(false));
    }

    #[test]
    fn establecer_en_modo_portable_da_error_claro() {
        let e = establecer(true, true, Path::new(r"C:\x\pixpinmax.exe")).unwrap_err();
        assert!(matches!(e, ErrorArranque::ModoPortable));
    }

    #[test]
    fn desactivar_en_modo_portable_no_es_error() {
        // Desactivar algo que no puede estar activo es una operacion vacia,
        // no un fallo: asi el codigo que aplica ajustes no necesita ramas.
        assert!(establecer(false, true, Path::new(r"C:\x\pixpinmax.exe")).is_ok());
    }

    #[test]
    fn en_modo_instalado_activar_y_desactivar_es_reversible() {
        let exe = Path::new(r"C:\NoExiste\pixpinmax.exe");

        establecer(true, false, exe).unwrap();
        assert!(esta_activo(), "tras activar deberia estar activo");

        establecer(false, false, exe).unwrap();
        assert!(!esta_activo(), "tras desactivar no deberia estar activo");
    }
}
```

- [ ] **Step 2: Ejecutar y comprobar que falla**

Run: `cargo test -p pixpin-shell arranque -- --test-threads=1`
Expected: FAIL — no existen las funciones.

- [ ] **Step 3: Implementar**

Añadir al principio de `crates/pixpin-shell/src/arranque.rs`:

```rust
//! Arranque con Windows.
//!
//! **En modo portable esto no se toca nunca.** La promesa del modo portable es
//! cero rastro en el equipo, y una entrada en `HKCU\...\Run` la romperia. Ademas
//! apuntaria a una ruta de USB que mañana puede no estar, dejando un error de
//! arranque al usuario cada vez que enciende el ordenador.

use std::path::Path;

use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, RegCloseKey, RegDeleteValueW,
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::{HSTRING, w};

const VALOR: windows::core::PCWSTR = w!("PixPinMax");

#[derive(Debug, thiserror::Error)]
pub enum ErrorArranque {
    #[error("en modo portable no se escribe en el registro, por diseño")]
    ModoPortable,
    #[error("error del registro de Windows: {0}")]
    Registro(#[from] windows::core::Error),
}

/// Si se puede ofrecer la opcion al usuario.
pub fn permitido(es_portable: bool) -> bool {
    !es_portable
}

fn abrir(acceso: windows::Win32::System::Registry::REG_SAM_FLAGS) -> windows::core::Result<HKEY> {
    let mut clave = HKEY::default();
    // SAFETY: la ruta es un literal estatico terminado en cero y `clave` es una
    // variable propia que se rellena en caso de exito.
    unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Run"),
            Some(0),
            acceso,
            &mut clave,
        )
        .ok()?;
    }
    Ok(clave)
}

/// Activa o desactiva el arranque automatico.
pub fn establecer(activo: bool, es_portable: bool, ruta_exe: &Path) -> Result<(), ErrorArranque> {
    if es_portable {
        // Desactivar es una operacion vacia, no un fallo: asi quien aplica los
        // ajustes no necesita ramificar por modo.
        return if activo { Err(ErrorArranque::ModoPortable) } else { Ok(()) };
    }

    let clave = abrir(KEY_WRITE)?;

    let resultado = if activo {
        // Las comillas son obligatorias: sin ellas una ruta con espacios
        // (C:\Program Files\...) se interpreta como varios argumentos.
        let linea = HSTRING::from(format!("\"{}\"", ruta_exe.display()));
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                linea.as_ptr() as *const u8,
                (linea.len() + 1) * size_of::<u16>(),
            )
        };
        // SAFETY: `bytes` describe exactamente el buffer UTF-16 de `linea`,
        // incluido su cero final, y `linea` sigue viva durante la llamada.
        unsafe { RegSetValueExW(clave, VALOR, Some(0), REG_SZ, Some(bytes)).ok() }
    } else {
        // SAFETY: `clave` es un handle valido abierto justo arriba.
        unsafe { RegDeleteValueW(clave, VALOR).ok() }
    };

    // SAFETY: `clave` viene de RegOpenKeyExW y no se ha cerrado antes.
    unsafe {
        let _ = RegCloseKey(clave);
    }

    // Borrar un valor que no existe no es un fallo desde el punto de vista del
    // usuario: el resultado que pedia (que no arranque solo) ya se cumple.
    match resultado {
        Ok(()) => Ok(()),
        Err(_) if !activo => Ok(()),
        Err(e) => Err(ErrorArranque::Registro(e)),
    }
}

/// Si ahora mismo hay una entrada de arranque automatico.
pub fn esta_activo() -> bool {
    let Ok(clave) = abrir(KEY_READ) else {
        return false;
    };
    // SAFETY: `clave` es valida; se piden solo los metadatos del valor pasando
    // None como buffer, que es el uso documentado para comprobar existencia.
    let existe = unsafe { RegQueryValueExW(clave, VALOR, None, None, None, None).is_ok() };
    // SAFETY: `clave` viene de RegOpenKeyExW y no se ha cerrado antes.
    unsafe {
        let _ = RegCloseKey(clave);
    }
    existe
}
```

Ampliar las características del crate `windows` con `Win32_System_Registry`:

```bash
cargo add windows -p pixpin-shell --features \
  Win32_Foundation,Win32_System_Threading,Win32_Globalization,Win32_UI_Shell,Win32_System_Com,Win32_UI_WindowsAndMessaging,Win32_System_LibraryLoader,Win32_UI_Input_KeyboardAndMouse,Win32_System_Registry
```

Añadir a `crates/pixpin-shell/src/lib.rs`:

```rust
pub mod arranque;

pub use arranque::ErrorArranque;
```

- [ ] **Step 4: Ejecutar y comprobar que pasa**

Run: `cargo test -p pixpin-shell -- --test-threads=1`
Expected: PASS. El test de ida y vuelta escribe y borra una entrada real en `HKCU`, y la deja limpia.

- [ ] **Step 5: Commit**

```bash
git add crates/pixpin-shell
git commit -m "Arranque con Windows, prohibido en modo portable

La promesa del modo portable es cero rastro en el equipo, y una entrada en
HKCU\\...\\Run la romperia. Peor aun: apuntaria a una ruta de USB que mañana
puede no estar, y el usuario veria un error de arranque cada vez que enciende
el ordenador. Por eso permitido() devuelve false en portable y establecer()
da un error explicito.

Desactivar en modo portable si es valido y no hace nada: quien aplica los
ajustes no deberia tener que ramificar por modo.

La ruta se escribe entre comillas porque sin ellas C:\\Program Files\\... se
interpreta como varios argumentos."
```

---

## Task 11: El ejecutable

Ensambla todo y deja una aplicación que se puede arrancar de verdad.

**Files:**
- Create: `apps/pixpin/build.rs`
- Create: `apps/pixpin/pixpinmax.manifest`
- Modify: `apps/pixpin/src/main.rs`
- Modify: `apps/pixpin/Cargo.toml`

**Interfaces:**
- Consumes: todo lo anterior.
- Produces: `pixpinmax.exe`.

- [ ] **Step 1: Añadir dependencias**

Run:
```bash
cargo add pixpin-shell --path crates/pixpin-shell -p pixpin
cargo add pixpin-store --path crates/pixpin-store -p pixpin
cargo add tracing tracing-subscriber tracing-appender anyhow -p pixpin
cargo add embed-resource --build -p pixpin
```

Comprueba que la regla de capas sigue en pie: `pixpin` es L4 y depende de L1 y L2, correcto.

Run: `cargo test -p pixpin --test capas`
Expected: PASS.

- [ ] **Step 2: Crear el manifiesto**

Crear `apps/pixpin/pixpinmax.manifest`:

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="PixPinMax" version="0.1.0.0"/>

  <!-- asInvoker: PixPin Max no necesita privilegios de administrador.
       Pedirlos rompería el arrastrar y soltar hacia aplicaciones normales
       por el aislamiento de privilegios de la interfaz de usuario. -->
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>

  <!-- PerMonitorV2 es imprescindible: sin esto, en un equipo con el portatil
       al 150% y un monitor externo al 100%, Windows escalaría nuestras
       ventanas y la lupa mostraría píxeles interpolados en vez de reales. -->
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
    </windowsSettings>
  </application>

  <!-- Declarar compatibilidad con Windows 10 evita que el sistema aplique
       capas de compatibilidad heredadas que falsean las APIs de versión. -->
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
</assembly>
```

Crear `apps/pixpin/pixpinmax.rc`:

```
#include <winuser.h>
1 24 "pixpinmax.manifest"
1 ICON "recursos/pixpinmax.ico"
```

Crear `apps/pixpin/build.rs`:

```rust
//! Incrusta el manifiesto y el icono en el ejecutable.

fn main() {
    embed_resource::compile("pixpinmax.rc", embed_resource::NONE)
        .manifest_required()
        .expect("el manifiesto es obligatorio: sin el no hay PerMonitorV2");
    println!("cargo:rerun-if-changed=pixpinmax.rc");
    println!("cargo:rerun-if-changed=pixpinmax.manifest");
}
```

- [ ] **Step 3: Escribir el ejecutable**

Reemplazar `apps/pixpin/src/main.rs`:

```rust
//! PixPin Max — punto de entrada.
//!
//! PixPin es marca de DepthPixel. Este proyecto es una implementacion
//! personal e independiente.
//!
//! El orden de arranque no es arbitrario:
//!
//! 1. Instancia unica primero, para no hacer trabajo que habra que deshacer.
//! 2. Rutas y ajustes, porque todo lo demas depende de ellos.
//! 3. Registro a fichero, para que cualquier fallo posterior quede escrito.
//! 4. Arranque con Windows, que solo depende de los ajustes.
//! 5. Idioma, antes de crear nada que muestre texto.
//! 6. Ventana, bandeja y atajos.
//! 7. Bucle, que duerme hasta que pasa algo.

// Sin consola: es una aplicacion de bandeja, no una herramienta de linea de
// comandos. Sin esto se abriria una ventana negra al arrancar.
#![windows_subsystem = "windows"]

use anyhow::{Context, Result};
use pixpin_shell::{
    Bandeja, Continuar, EtiquetasMenu, Evento, VentanaMensajes, adquirir_instancia_unica, arranque,
    atajos, entorno,
};
use pixpin_store::{Catalogo, Ubicacion, ajustes, idioma, rutas};

fn main() -> Result<()> {
    // 1. Una sola copia a la vez.
    let _instancia = match adquirir_instancia_unica() {
        Ok(i) => i,
        Err(_) => {
            // No se puede avisar con un dialogo traducido porque todavia no se
            // han leido los ajustes. Salir en silencio es el comportamiento
            // correcto: el usuario pulso el icono dos veces, nada mas.
            return Ok(());
        }
    };

    // 2. Donde vivimos y que nos han configurado.
    let dir_exe = entorno::directorio_del_ejecutable().context("no se pudo localizar el .exe")?;
    let appdata = entorno::appdata().context("no se pudo localizar APPDATA")?;
    let ubicacion = rutas::resolver(&dir_exe, &appdata);
    let config = ajustes::cargar(&ubicacion).context("no se pudieron leer los ajustes")?;

    // 3. Registro a fichero, antes de que pueda fallar nada mas.
    let _guardia_registro = iniciar_registro(&ubicacion);
    tracing::info!(
        portable = ubicacion.es_portable(),
        raiz = ?ubicacion.raiz(),
        "PixPin Max arrancando"
    );

    // 4. Reflejar en el registro de Windows lo que digan los ajustes.
    //
    // Se aplica en cada arranque y no solo al cambiar la casilla porque el
    // usuario puede haber editado el TOML a mano, o haber copiado su fichero
    // de ajustes a otro equipo. Asi el estado real y el declarado no divergen.
    let ruta_exe = dir_exe.join("pixpinmax.exe");
    match arranque::establecer(
        config.arranque_con_windows,
        ubicacion.es_portable(),
        &ruta_exe,
    ) {
        Ok(()) => {}
        Err(arranque::ErrorArranque::ModoPortable) => {
            // No es un fallo: es la regla del modo portable funcionando. Se
            // deja constancia para que nadie piense que la casilla esta rota.
            tracing::info!(
                "arranque con Windows ignorado: en modo portable no se toca el registro"
            );
        }
        Err(e) => tracing::warn!(?e, "no se pudo aplicar el arranque con Windows"),
    }

    // 5. Idioma, antes de crear nada con texto.
    let lengua = idioma::resolver_idioma(&entorno::locale_del_sistema(), config.idioma);
    let textos = Catalogo::nuevo(lengua);

    // 6. Ventana invisible, icono de bandeja y atajos.
    let ventana = VentanaMensajes::nueva().context("no se pudo crear la ventana de mensajes")?;
    let bandeja = Bandeja::nueva(ventana.handle(), &textos.t("app-nombre"))
        .context("no se pudo añadir el icono de bandeja")?;

    let peticiones = [
        (atajos::ID_REGION, config.atajos.region),
        (atajos::ID_COPIAR, config.atajos.copiar),
        (atajos::ID_SCROLL, config.atajos.scroll),
        (atajos::ID_CUENTAGOTAS, config.atajos.cuentagotas),
    ];
    let (_registrados, fallidos) = atajos::registrar(ventana.handle(), &peticiones);
    for (id, atajo) in &fallidos {
        // Se registra el problema pero no se aborta: otra aplicacion puede
        // tener ese atajo y el resto de PixPin Max sigue siendo util.
        tracing::warn!(id, %atajo, "no se pudo registrar el atajo; otra aplicacion lo tiene");
    }

    let etiquetas = EtiquetasMenu {
        capturar: textos.t("bandeja-capturar"),
        ajustes: textos.t("bandeja-ajustes"),
        salir: textos.t("bandeja-salir"),
    };

    // 7. A dormir hasta que pase algo.
    let hwnd = ventana.handle();
    ventana.ejecutar(|evento| match evento {
        Evento::MenuSalir => {
            tracing::info!("salida pedida por el usuario");
            Continuar::No
        }
        Evento::IconoPulsado => {
            if let Err(e) = bandeja.mostrar_menu(hwnd, &etiquetas) {
                tracing::warn!(?e, "no se pudo mostrar el menu de bandeja");
            }
            Continuar::Si
        }
        Evento::Atajo(id) => {
            // S1-B conecta esto con la captura. Por ahora solo se registra,
            // que ya permite comprobar de verdad que los atajos funcionan.
            tracing::info!(id, "atajo pulsado");
            Continuar::Si
        }
        Evento::MenuCapturar => {
            tracing::info!("capturar pedido desde el menu");
            Continuar::Si
        }
        Evento::MenuAjustes => {
            tracing::info!("ajustes pedidos desde el menu");
            Continuar::Si
        }
    });

    tracing::info!("PixPin Max terminado limpiamente");
    Ok(())
}

/// Registro rotativo diario junto a los ajustes. Nada sale del equipo.
fn iniciar_registro(ubicacion: &Ubicacion) -> tracing_appender::non_blocking::WorkerGuard {
    let dir = ubicacion.raiz().join("registros");
    let _ = std::fs::create_dir_all(&dir);
    let fichero = tracing_appender::rolling::daily(dir, "pixpinmax.log");
    let (escritor, guardia) = tracing_appender::non_blocking(fichero);
    tracing_subscriber::fmt()
        .with_writer(escritor)
        .with_ansi(false)
        .init();
    guardia
}
```

- [ ] **Step 4: Compilar y comprobar que arranca**

Run: `cargo build --release`
Expected: compila sin avisos y produce `target/release/pixpinmax.exe`.

Ejecuta `target/release/pixpinmax.exe`. Comprueba a mano:
1. Aparece el icono en la bandeja.
2. Clic izquierdo en el icono muestra el menú con los tres elementos, traducidos.
3. `Ctrl+Alt+X` escribe una línea «atajo pulsado» en `%APPDATA%\PixPinMax\registros\pixpinmax.log`.
4. Arrancar una segunda copia no hace nada y no deja un segundo icono.
5. «Salir» cierra la aplicación y **el icono desaparece de inmediato**, sin tener que pasar el ratón por encima.

- [ ] **Step 5: Comprobar el modo portable**

Copia `pixpinmax.exe` a una carpeta vacía y crea junto a él un `pixpinmax.toml` vacío. Ejecútalo.

Comprueba que los registros aparecen en `<esa carpeta>\registros\` y **no** en `%APPDATA%`.

Después escribe `arranque_con_windows = true` en ese `pixpinmax.toml` y vuelve a ejecutarlo. Comprueba dos cosas:
1. En el registro de la aplicación aparece la línea «arranque con Windows ignorado: en modo portable no se toca el registro».
2. En `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` **no** hay ningún valor `PixPinMax`.

Esa es la promesa del modo portable comprobada de extremo a extremo.

- [ ] **Step 6: Medir el presupuesto de rendimiento**

Con la aplicación en marcha y en reposo, en el Administrador de tareas:

| Métrica | Objetivo | Medido |
|---|---|---|
| CPU en reposo | 0% | |
| RAM en reposo | < 40 MB | |
| Tamaño de `pixpinmax.exe` | < 30 MB | |

Anota los valores reales. Si el CPU en reposo no es 0%, hay un bucle girando donde debería haber un `GetMessageW` bloqueante, y hay que encontrarlo antes de seguir con S1-B.

- [ ] **Step 7: Comprobar que todo el workspace sigue sano**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace -- --test-threads=1 && cargo deny check licenses`
Expected: los cuatro comandos en verde.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "PixPin Max arranca: bandeja, atajos globales, ajustes e idiomas

Primera version ejecutable. Vive en la bandeja, lee sus ajustes (portable o
en APPDATA segun donde este el .toml), responde a los cuatro atajos globales
y habla español o ingles segun el sistema.

El orden de arranque es deliberado: instancia unica primero para no hacer
trabajo que haya que deshacer, luego ajustes, luego registro a fichero para
que cualquier fallo posterior quede escrito, luego idioma antes de crear nada
que muestre texto.

El manifiesto declara PerMonitorV2, sin lo cual la lupa mostraria pixeles
interpolados en equipos con escalados mixtos, y asInvoker, porque pedir
privilegios de administrador romperia el arrastrar y soltar hacia
aplicaciones normales.

Los atajos que otra aplicacion ya tenga se registran como aviso y no impiden
arrancar."
```

---

## Definición de terminado para S1-A

- [ ] `cargo fmt --all --check` en verde
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` en verde
- [ ] `cargo test --workspace -- --test-threads=1` en verde
- [ ] `cargo deny check licenses bans sources` en verde
- [ ] El test de capas pasa y cubre los 16 paquetes
- [ ] Los dos catálogos de idioma tienen exactamente las mismas claves
- [ ] `pixpinmax.exe` arranca, vive en la bandeja y sale limpiamente
- [ ] Los cuatro atajos globales registran su pulsación en el fichero de registro
- [ ] El modo portable no escribe nada fuera de la carpeta del ejecutable, ni siquiera con `arranque_con_windows = true`
- [ ] En modo instalado, `arranque_con_windows` crea y borra la entrada de `HKCU\...\Run`
- [ ] Las tres métricas de rendimiento medibles en esta fase están anotadas y dentro del objetivo

**Lo siguiente:** plan **S1-B** — Direct2D, Windows.Graphics.Capture, overlay por monitor, lupa y ajuste automático con UI Automation.
