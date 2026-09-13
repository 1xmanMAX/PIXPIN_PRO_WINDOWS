# E1 — La tinta de Excalidraw, nativa — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que el trazo a mano de PixPin (editor, capa sobre pantalla y anotación del pin) sea geométricamente idéntico al de Excalidraw, lea todos los puntos del ratón y la presión del lápiz, y se pinte sin lag.

**Architecture:** Se portan a Rust, en f64 y línea a línea, `perfect-freehand@1.2.0` (pluma variable) y `@excalidraw/laser-pointer@1.3.1` (pluma constante), vigilados por un oráculo en Node que ejecuta los originales. El motor produce una orden nueva `Orden::Tinta` que `pixpin-render` pinta con curvas cuadráticas por puntos medios y relleno *winding*. `pixpin-shell` recupera los movimientos que Windows fusiona (`GetMouseMovePointsEx`) y lee el historial del lápiz (`GetPointerPenInfoHistory`). El editor espera mensajes sin `sleep`, congela la escena mientras se dibuja y presenta con vsync.

**Tech Stack:** Rust 1.97 (workspace), crate `windows` 0.62 (Direct2D, DXGI, Win32 input), serde/serde_json, Node 24 solo para el oráculo.

**Spec:** `docs/superpowers/specs/2026-09-13-e1-tinta-excalidraw-design.md`

## Global Constraints

- Rama `e1-tinta-excalidraw`. No se fusiona `main` aquí.
- Suite completa: `cargo test --workspace --no-fail-fast -- --test-threads=1` (sin `--test-threads=1` `asignaciones.rs` da rojos falsos; con `pixpinmax.exe` abierto falla `la_segunda_adquisicion_falla_mientras_viva_la_primera`, que no cuenta).
- `cargo` puede no estar en el PATH: anteponer `%USERPROFILE%\.cargo\bin`.
- Antes de cada commit: `cargo fmt --all` y `cargo clippy --workspace --all-targets -- -D warnings` limpios.
- Estilo `.rs`: español **sin tildes**, comentarios que explican el POR QUÉ, pruebas con nombre de frase y con casos negativos. `unsafe` solo con `// SAFETY:`.
- Regla de capas (`apps/pixpin/tests/capas.rs`): un crate solo depende de capas inferiores. `pixpin-render` y `pixpin-motor2d` son L1 y **no se conocen entre sí**. `pixpin-pin` (L2) puede depender de `pixpin-shell` (L1).
- Licencias: se porta código MIT de Excalidraw / perfect-freehand / laser-pointer. **Nunca** código de `obsidian-excalidraw-plugin` (AGPL).
- Constantes exactas (spec D109, D112, D116–D118): variable `size = strokeWidth × 4.25`, `thinning 0.6`, `smoothing 0.5`, easing `sin(t·π/2)`, `last: true` siempre; constante `size = strokeWidth × 1.4`, `simplify 0`, presión 1; `streamline` 0.5 ratón / 0.2 lápiz; grosores 0.5 / 1 / 2.
- El agente **no** ejecuta la app con entrada sintetizada (regla del usuario). Las pruebas manuales las hace el usuario.
- Cada commit termina con la línea `Claude-Session: https://claude.ai/code/session_01FJbWm4jCHQtHwwndRDpLZJ`.

## Ajustes respecto a la spec

- **D108:** la muestra no lleva `t_ms`. Nada lo consume (Excalidraw tampoco usa el tiempo); se añade el día que haga falta. Además va en punto fijo (1/16 px) porque viaja dentro de `EventoOverlay`, que es `Copy + Eq`.
- **Prueba manual 5** («guardar `.excalidraw` y abrirlo en excalidraw.com»): el editor aún no guarda (E4/E5). La cubre la prueba automática de ida y vuelta de la Tarea 4.
- **D115:** la puerta `de_lejos_el_dibujo_cuesta_una_decima_parte` dependía del umbral de 48 px y se reescribe (Tarea 6); la economía de lejos pasa a la capa congelada y a la caché de realizaciones.

## Mapa de ficheros

| Fichero | Qué | Tarea |
|---|---|---|
| `herramientas/oraculo-excalidraw/{package.json,casos.mjs,generar.mjs,.gitignore}` | Oráculo Node | 1 |
| `crates/pixpin-motor2d/tests/oraculo/tinta.json` | Referencias generadas | 1 |
| `crates/pixpin-motor2d/src/tinta/freehand.rs` | Porte perfect-freehand | 2 |
| `crates/pixpin-motor2d/src/tinta/laser.rs` | Porte LaserPointer | 3 |
| `crates/pixpin-motor2d/tests/oraculo_tinta.rs` | Rust contra el oráculo | 2, 3 |
| `crates/pixpin-motor2d/src/tinta/mod.rs` | Opciones de pluma y `contorno_de_lapiz` | 4 |
| `crates/pixpin-motor2d/src/{elemento,excalidraw,gesto,...}.rs` | Campo `opciones` | 4 |
| `crates/pixpin-render/src/tinta.rs` | `pasos_de_tinta`, `Pintor::tinta`, `CacheTinta` | 5, 10 |
| `crates/pixpin-render/src/{lienzo,motor,superficie}.rs` | Pinceles cacheados, `presentar_sincronizado` | 5, 9 |
| `crates/pixpin-motor2d/src/pintado.rs` | `Orden::Tinta`, umbral 2 px | 6 |
| `crates/pixpin-shell/src/puntero.rs` | Entrada fina ratón y lápiz | 7 |
| `crates/pixpin-shell/src/overlay.rs` | `EventoOverlay::Muestra`, `esperar_eventos` | 7, 9 |
| `crates/pixpin-motor2d/src/gesto.rs` | Presión, plumas, `trazo_en_curso` | 8 |
| `apps/pixpin/src/ventana_editor.rs` | Muestras, origen, bucle, capa congelada | 9, 10 |
| `crates/pixpin-ui/src/anotador.rs`, `apps/pixpin/src/{capa,pines}.rs`, `crates/pixpin-pin/src/ventana.rs` | Entrada fina en capa y pin | 11 |
| `docs/excalidraw/mapa.md`, `THIRD-PARTY-NOTICES.md`, `herramientas/novedades-excalidraw.mjs`, `medidas/` | Mapa, avisos, novedades, medidas | 12 |

---

### Task 1: El oráculo de Excalidraw

**Files:**
- Create: `herramientas/oraculo-excalidraw/package.json`
- Create: `herramientas/oraculo-excalidraw/.gitignore`
- Create: `herramientas/oraculo-excalidraw/casos.mjs`
- Create: `herramientas/oraculo-excalidraw/generar.mjs`
- Create (generado): `crates/pixpin-motor2d/tests/oraculo/tinta.json`

**Interfaces:**
- Produces: `tinta.json` con forma `{ "origen": {...}, "casos": [ { "nombre": str, "puntos": [[f64,f64]], "presiones": [f64], "grosor": f64, "variabilidad": "variable"|"constant", "streamline": f64, "contorno": [[f64,f64]], "svg": str } ] }`. `presiones` vacío = presión simulada.

- [ ] **Step 1: `package.json` y `.gitignore`**

```json
{
  "name": "oraculo-excalidraw",
  "private": true,
  "type": "module",
  "description": "Ejecuta la tinta real de Excalidraw y guarda referencias para las pruebas de Rust. No se distribuye.",
  "scripts": { "generar": "node generar.mjs" },
  "dependencies": {
    "perfect-freehand": "1.2.0",
    "@excalidraw/laser-pointer": "1.3.1"
  }
}
```

`.gitignore`:

```
node_modules/
```

- [ ] **Step 2: `casos.mjs` — trazos fijos y deterministas**

```js
// Trazos de prueba. Deterministas: nada de Math.random, para que regenerar
// dé el mismo fichero byte a byte si el original no cambió.
const rango = (n, f) => Array.from({ length: n }, (_, i) => f(i));

const trazos = {
  recta: rango(40, (i) => [i * 5, 0]),
  curva: rango(60, (i) => [i * 4, Math.sin(i / 6) * 40]),
  espiral: rango(120, (i) => {
    const a = i / 8;
    const r = 4 + i * 0.9;
    return [Math.cos(a) * r, Math.sin(a) * r];
  }),
  // Un ocho: el contorno se cruza consigo mismo (el caso de los agujeros).
  bucle: rango(80, (i) => {
    const a = (i / 80) * 2 * Math.PI * 1.2;
    return [Math.sin(a * 2) * 60, Math.sin(a) * 60];
  }),
  punto: [[10, 10]],
  dos_puntos: [[0, 0], [30, 12]],
  zigzag: rango(30, (i) => [i * 25, i % 2 ? 40 : 0]),
  lento_rapido: rango(50, (i) => [i * i * 0.4, i * 2]),
};

const presionCreciente = (n) => rango(n, (i) => i / (n - 1));

export const casos = [
  ...Object.entries(trazos).flatMap(([nombre, puntos]) => [
    { nombre: `${nombre}_variable_medio`, puntos, presiones: [], grosor: 1, variabilidad: "variable", streamline: 0.5 },
    { nombre: `${nombre}_constante_medio`, puntos, presiones: [], grosor: 1, variabilidad: "constant", streamline: 0.5 },
  ]),
  { nombre: "recta_variable_fino", puntos: trazos.recta, presiones: [], grosor: 0.5, variabilidad: "variable", streamline: 0.5 },
  { nombre: "recta_variable_grueso", puntos: trazos.recta, presiones: [], grosor: 2, variabilidad: "variable", streamline: 0.5 },
  { nombre: "curva_lapiz_presion", puntos: trazos.curva, presiones: presionCreciente(60), grosor: 1, variabilidad: "variable", streamline: 0.2 },
  { nombre: "espiral_lapiz_presion", puntos: trazos.espiral, presiones: presionCreciente(120), grosor: 2, variabilidad: "variable", streamline: 0.2 },
  { nombre: "zigzag_constante_lapiz", puntos: trazos.zigzag, presiones: [], grosor: 2, variabilidad: "constant", streamline: 0.2 },
];
```

- [ ] **Step 3: `generar.mjs` — las funciones reales**

```js
// Genera crates/pixpin-motor2d/tests/oraculo/tinta.json con la tinta REAL de
// Excalidraw. Las dos envolturas y getSvgPathFromStroke son copia literal de
// excalidraw/excalidraw packages/element/src/shape.ts @afa3a65 (MIT,
// Copyright (c) 2020 Excalidraw); getStroke y LaserPointer vienen de npm, en
// las versiones exactas que fija packages/excalidraw/package.json.
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { getStroke } from "perfect-freehand";
import { LaserPointer } from "@excalidraw/laser-pointer";
import { casos } from "./casos.mjs";

const aqui = dirname(fileURLToPath(import.meta.url));
const destino = join(aqui, "..", "..", "crates", "pixpin-motor2d", "tests", "oraculo", "tinta.json");

function variable(c) {
  const simulatePressure = c.presiones.length === 0;
  const input = simulatePressure
    ? c.puntos
    : c.puntos.map(([x, y], i) => [x, y, c.presiones[i]]);
  return getStroke(input, {
    simulatePressure,
    size: c.grosor * 4.25,
    thinning: 0.6,
    smoothing: 0.5,
    streamline: c.streamline,
    easing: (t) => Math.sin((t * Math.PI) / 2),
    last: true,
  });
}

function constante(c) {
  const lp = new LaserPointer({
    size: c.grosor * 1.4,
    streamline: c.streamline,
    simplify: 0,
    sizeMapping: (d) => Math.max(0.1, d.pressure),
  });
  c.puntos.forEach(([x, y]) => lp.addPoint([x, y, 1]));
  return lp.getStrokeOutline().map(([x, y]) => [x, y]);
}

const med = (A, B) => [(A[0] + B[0]) / 2, (A[1] + B[1]) / 2];
const TO_FIXED_PRECISION = /(\s?[A-Z]?,?-?[0-9]*\.[0-9]{0,2})(([0-9]|e|-)*)/g;
const getSvgPathFromStroke = (points) => {
  if (!points.length) return "";
  const max = points.length - 1;
  return points
    .reduce(
      (acc, point, i, arr) => {
        if (i === max) acc.push(point, med(point, arr[0]), "L", arr[0], "Z");
        else acc.push(point, med(point, arr[i + 1]));
        return acc;
      },
      ["M", points[0], "Q"],
    )
    .join(" ")
    .replace(TO_FIXED_PRECISION, "$1");
};

const salida = {
  origen: { excalidraw: "afa3a65", "perfect-freehand": "1.2.0", "@excalidraw/laser-pointer": "1.3.1" },
  casos: casos.map((c) => {
    const contorno = c.variabilidad === "constant" ? constante(c) : variable(c);
    return { ...c, contorno, svg: getSvgPathFromStroke(contorno) };
  }),
};

mkdirSync(dirname(destino), { recursive: true });
writeFileSync(destino, JSON.stringify(salida, null, 1) + "\n");
console.log(`${salida.casos.length} casos -> ${destino}`);
```

- [ ] **Step 4: Instalar y generar**

Run: `cd herramientas/oraculo-excalidraw && npm install && npm run generar`
Expected: `21 casos -> ...tinta.json`. Comprobar a mano que `punto_variable_medio` tiene 13 puntos de contorno y que ningún contorno contiene `null` (NaN serializado). Si `LaserPointer` no se exporta con ese nombre, mirar `node_modules/@excalidraw/laser-pointer/dist/types.d.ts` y ajustar solo el `import`.

- [ ] **Step 5: Commit**

```bash
git add herramientas/oraculo-excalidraw crates/pixpin-motor2d/tests/oraculo/tinta.json
git commit -m "El oraculo: la tinta real de Excalidraw, guardada para compararse con ella"
```

---

### Task 2: perfect-freehand fiel (pluma variable)

**Files:**
- Create: `crates/pixpin-motor2d/src/tinta/mod.rs` (solo `pub mod freehand;` por ahora)
- Create: `crates/pixpin-motor2d/src/tinta/freehand.rs`
- Modify: `crates/pixpin-motor2d/src/lib.rs` (añadir `pub mod tinta;` junto a `pub mod trazo;`)
- Create: `crates/pixpin-motor2d/tests/oraculo_tinta.rs`

**Interfaces:**
- Produces: `pixpin_motor2d::tinta::freehand::{Entrada, Opciones, contorno, seno}` con
  `pub struct Entrada { pub x: f64, pub y: f64, pub presion: Option<f64> }`,
  `pub struct Opciones { pub size: f64, pub thinning: f64, pub smoothing: f64, pub streamline: f64, pub simular_presion: bool, pub last: bool, pub easing: fn(f64) -> f64 }`,
  `pub fn contorno(entrada: &[Entrada], o: &Opciones) -> Vec<[f64; 2]>`, `pub fn seno(t: f64) -> f64`.

- [ ] **Step 1: Prueba del oráculo (falla: no existe el módulo)**

`crates/pixpin-motor2d/tests/oraculo_tinta.rs`:

```rust
//! La tinta de Rust contra la de Excalidraw ejecutada de verdad.
//!
//! `tests/oraculo/tinta.json` lo escribe `herramientas/oraculo-excalidraw`
//! con las funciones originales. Si esto se pone rojo tras regenerar, es que
//! Excalidraw cambio algo que aun no hemos portado; si se pone rojo sin
//! regenerar, es que el porte se rompio.

use pixpin_motor2d::tinta::freehand::{self, Entrada, Opciones};
use serde::Deserialize;

const TOLERANCIA: f64 = 0.01;

#[derive(Deserialize)]
struct Fichero {
    casos: Vec<Caso>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct Caso {
    nombre: String,
    puntos: Vec<[f64; 2]>,
    presiones: Vec<f64>,
    grosor: f64,
    variabilidad: String,
    streamline: f64,
    contorno: Vec<[f64; 2]>,
    svg: String,
}

fn casos(variabilidad: &str) -> Vec<Caso> {
    let texto = include_str!("oraculo/tinta.json");
    let f: Fichero = serde_json::from_str(texto).expect("tinta.json se lee");
    f.casos
        .into_iter()
        .filter(|c| c.variabilidad == variabilidad)
        .collect()
}

fn comparar(nombre: &str, esperado: &[[f64; 2]], obtenido: &[[f64; 2]]) {
    assert_eq!(
        esperado.len(),
        obtenido.len(),
        "{nombre}: {} puntos en Excalidraw, {} en Rust",
        esperado.len(),
        obtenido.len()
    );
    for (i, (e, o)) in esperado.iter().zip(obtenido).enumerate() {
        let d = (e[0] - o[0]).abs().max((e[1] - o[1]).abs());
        assert!(
            d <= TOLERANCIA,
            "{nombre}: el punto {i} difiere {d}: Excalidraw {e:?}, Rust {o:?}"
        );
    }
}

#[test]
fn la_pluma_variable_coincide_con_excalidraw_en_todos_los_casos() {
    let casos = casos("variable");
    assert!(casos.len() >= 10, "el oraculo tiene que traer casos variables");
    for c in casos {
        let simular = c.presiones.is_empty();
        let entrada: Vec<Entrada> = c
            .puntos
            .iter()
            .enumerate()
            .map(|(i, p)| Entrada {
                x: p[0],
                y: p[1],
                presion: if simular { None } else { Some(c.presiones[i]) },
            })
            .collect();
        let o = Opciones {
            size: c.grosor * 4.25,
            thinning: 0.6,
            smoothing: 0.5,
            streamline: c.streamline,
            simular_presion: simular,
            last: true,
            easing: freehand::seno,
        };
        comparar(&c.nombre, &c.contorno, &freehand::contorno(&entrada, &o));
    }
}
```

- [ ] **Step 2: Ejecutar y ver que falla**

Run: `cargo test -p pixpin-motor2d --test oraculo_tinta -- --test-threads=1`
Expected: FAIL de compilación, `could not find tinta in pixpin_motor2d`.

- [ ] **Step 3: El porte**

`crates/pixpin-motor2d/src/tinta/mod.rs`:

```rust
//! La tinta de Excalidraw, portada (E1). Ver `docs/excalidraw/mapa.md`.

pub mod freehand;
```

`crates/pixpin-motor2d/src/tinta/freehand.rs`:

```rust
//! Porte fiel de perfect-freehand 1.2.0 (MIT, Copyright (c) 2021 Stephen
//! Ruiz Ltd): `getStrokePoints` y `getStrokeOutlinePoints`.
//!
//! Origen: npm `perfect-freehand@1.2.0`, la version que fija Excalidraw
//! `afa3a65` en `packages/excalidraw/package.json`. No la del repositorio
//! de perfect-freehand, que va por delante.
//!
//! # Por que f64 y bucles que suman el paso
//!
//! La referencia es JavaScript: todo es f64 y los arcos se trazan con
//! `for (t = 0; t <= 1; t += 1/13)`. Sumar 1/13 trece veces no da 1 exacto,
//! y de eso depende si sale la ultima vuelta del bucle. Un porte "limpio"
//! con `0..=13` daria un punto de mas o de menos en algunas tapas y el
//! oraculo lo cazaria. Asi que se copia el bucle tal cual, en f64.
//!
//! # Que NO se porta
//!
//! El afilado (`taper`) y las tapas planas: Excalidraw no los usa. Con
//! `taper = 0` y `cap = true` sus ramas son codigo muerto.

pub type V = [f64; 2];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entrada {
    pub x: f64,
    pub y: f64,
    /// `None` equivale al `undefined` de JavaScript: se usa la presion por
    /// defecto (0,25 el primer punto, 0,5 el resto).
    pub presion: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub struct Opciones {
    pub size: f64,
    pub thinning: f64,
    pub smoothing: f64,
    pub streamline: f64,
    pub simular_presion: bool,
    pub last: bool,
    pub easing: fn(f64) -> f64,
}

/// easeOutSine, el easing que pasa Excalidraw.
pub fn seno(t: f64) -> f64 {
    (t * std::f64::consts::PI / 2.0).sin()
}

const RATE_OF_PRESSURE_CHANGE: f64 = 0.275;
const FIXED_PI: f64 = std::f64::consts::PI + 0.0001;
const START_CAP_SEGMENTS: f64 = 13.0;
const END_CAP_SEGMENTS: f64 = 29.0;
const CORNER_CAP_SEGMENTS: f64 = 13.0;
const END_NOISE_THRESHOLD: f64 = 3.0;
const MIN_STREAMLINE_T: f64 = 0.15;
const STREAMLINE_T_RANGE: f64 = 0.85;
const MIN_RADIUS: f64 = 0.01;
const DEFAULT_FIRST_PRESSURE: f64 = 0.25;
const DEFAULT_PRESSURE: f64 = 0.5;

fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1]]
}
fn mul(a: V, n: f64) -> V {
    [a[0] * n, a[1] * n]
}
fn neg(a: V) -> V {
    [-a[0], -a[1]]
}
fn per(a: V) -> V {
    [a[1], -a[0]]
}
fn dpr(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn uni(a: V) -> V {
    let l = a[0].hypot(a[1]);
    [a[0] / l, a[1] / l]
}
fn dist(a: V, b: V) -> f64 {
    (a[1] - b[1]).hypot(a[0] - b[0])
}
fn dist2(a: V, b: V) -> f64 {
    let d = sub(a, b);
    d[0] * d[0] + d[1] * d[1]
}
fn lrp(a: V, b: V, t: f64) -> V {
    add(a, mul(sub(b, a), t))
}
fn prj(a: V, b: V, c: f64) -> V {
    add(a, mul(b, c))
}
fn rot_around(a: V, c: V, r: f64) -> V {
    let (s, co) = (r.sin(), r.cos());
    let (px, py) = (a[0] - c[0], a[1] - c[1]);
    [px * co - py * s + c[0], px * s + py * co + c[1]]
}

#[derive(Debug, Clone, Copy)]
struct PuntoTrazo {
    point: V,
    pressure: f64,
    vector: V,
    distance: f64,
    running_length: f64,
}

/// `presion >= 0` en JavaScript: `undefined` y `NaN` no valen.
fn valida(p: Option<f64>) -> Option<f64> {
    p.filter(|v| *v >= 0.0)
}

fn puntos_del_trazo(entrada: &[Entrada], o: &Opciones) -> Vec<PuntoTrazo> {
    if entrada.is_empty() {
        return Vec::new();
    }
    let t = MIN_STREAMLINE_T + (1.0 - o.streamline) * STREAMLINE_T_RANGE;
    let mut pts: Vec<Entrada> = entrada.to_vec();

    // Dos puntos se convierten en cinco. `lrp` del original devuelve solo
    // x,y, asi que los interpolados PIERDEN la presion: se copia eso.
    if pts.len() == 2 {
        let ultimo = pts[1];
        pts.truncate(1);
        for i in 1..5 {
            let q = lrp([pts[0].x, pts[0].y], [ultimo.x, ultimo.y], i as f64 / 4.0);
            pts.push(Entrada {
                x: q[0],
                y: q[1],
                presion: None,
            });
        }
    }
    // Uno se duplica desplazado (1,1), este SI conserva la presion.
    if pts.len() == 1 {
        let p = pts[0];
        pts.push(Entrada {
            x: p.x + 1.0,
            y: p.y + 1.0,
            presion: p.presion,
        });
    }

    let mut salida = vec![PuntoTrazo {
        point: [pts[0].x, pts[0].y],
        pressure: valida(pts[0].presion).unwrap_or(DEFAULT_FIRST_PRESSURE),
        vector: [1.0, 1.0],
        distance: 0.0,
        running_length: 0.0,
    }];
    let mut alcanzado = false;
    let mut recorrido = 0.0;
    let mut prev = salida[0];
    let max = pts.len() - 1;

    for (i, p) in pts.iter().enumerate().skip(1) {
        let point = if o.last && i == max {
            [p.x, p.y]
        } else {
            lrp(prev.point, [p.x, p.y], t)
        };
        if prev.point == point {
            continue;
        }
        let distance = dist(point, prev.point);
        // El recorrido suma aunque el punto se descarte abajo: asi lo hace
        // el original, y cambia cuando se alcanza el largo minimo.
        recorrido += distance;
        if i < max && !alcanzado {
            if recorrido < o.size {
                continue;
            }
            alcanzado = true;
        }
        prev = PuntoTrazo {
            point,
            pressure: valida(p.presion).unwrap_or(DEFAULT_PRESSURE),
            vector: uni(sub(prev.point, point)),
            distance,
            running_length: recorrido,
        };
        salida.push(prev);
    }
    salida[0].vector = salida.get(1).map_or([0.0, 0.0], |s| s.vector);
    salida
}

fn simular_presion(anterior: f64, distancia: f64, size: f64) -> f64 {
    let sp = (distancia / size).min(1.0);
    let rp = (1.0 - sp).min(1.0);
    (anterior + (rp - anterior) * (sp * RATE_OF_PRESSURE_CHANGE)).min(1.0)
}

fn radio(size: f64, thinning: f64, presion: f64, easing: fn(f64) -> f64) -> f64 {
    size * easing(0.5 - thinning * (0.5 - presion))
}

fn contorno_de_puntos(puntos: &[PuntoTrazo], o: &Opciones) -> Vec<V> {
    if puntos.is_empty() || o.size <= 0.0 {
        return Vec::new();
    }
    let n = puntos.len();
    let total = puntos[n - 1].running_length;
    let min_distance = (o.size * o.smoothing).powi(2);
    let mut izquierda: Vec<V> = Vec::new();
    let mut derecha: Vec<V> = Vec::new();

    let mut prev_pressure = puntos.iter().take(10).fold(puntos[0].pressure, |acc, p| {
        let presion = if o.simular_presion {
            simular_presion(acc, p.distance, o.size)
        } else {
            p.pressure
        };
        (acc + presion) / 2.0
    });
    let mut radius = radio(o.size, o.thinning, puntos[n - 1].pressure, o.easing);
    let mut first_radius: Option<f64> = None;
    let mut prev_vector = puntos[0].vector;
    let mut prev_left = puntos[0].point;
    let mut prev_right = prev_left;
    let mut temp_left = prev_left;
    let mut temp_right = prev_right;
    let mut is_prev_sharp = false;

    for i in 0..n {
        let PuntoTrazo {
            point,
            vector,
            distance,
            running_length,
            mut pressure,
        } = puntos[i];
        let is_last = i == n - 1;
        if !is_last && total - running_length < END_NOISE_THRESHOLD {
            continue;
        }
        if o.thinning != 0.0 {
            if o.simular_presion {
                pressure = simular_presion(prev_pressure, distance, o.size);
            }
            radius = radio(o.size, o.thinning, pressure, o.easing);
        } else {
            radius = o.size / 2.0;
        }
        if first_radius.is_none() {
            first_radius = Some(radius);
        }
        // Sin afilado las dos fuerzas valen 1: queda solo el minimo.
        radius = radius.max(MIN_RADIUS);

        let next_vector = if is_last { vector } else { puntos[i + 1].vector };
        let next_dpr = if is_last { 1.0 } else { dpr(vector, next_vector) };
        let prev_dpr = dpr(vector, prev_vector);
        let is_point_sharp = prev_dpr < 0.0 && !is_prev_sharp;
        let is_next_sharp = next_dpr < 0.0;

        if is_point_sharp || is_next_sharp {
            let offset = mul(per(prev_vector), radius);
            let step = 1.0 / CORNER_CAP_SEGMENTS;
            let mut t = 0.0;
            while t <= 1.0 {
                temp_left = rot_around(sub(point, offset), point, FIXED_PI * t);
                izquierda.push(temp_left);
                temp_right = rot_around(add(point, offset), point, FIXED_PI * -t);
                derecha.push(temp_right);
                t += step;
            }
            prev_left = temp_left;
            prev_right = temp_right;
            if is_next_sharp {
                is_prev_sharp = true;
            }
            continue;
        }
        is_prev_sharp = false;

        if is_last {
            let offset = mul(per(vector), radius);
            izquierda.push(sub(point, offset));
            derecha.push(add(point, offset));
            continue;
        }

        let offset = mul(per(lrp(next_vector, vector, next_dpr)), radius);
        temp_left = sub(point, offset);
        if i <= 1 || dist2(prev_left, temp_left) > min_distance {
            izquierda.push(temp_left);
            prev_left = temp_left;
        }
        temp_right = add(point, offset);
        if i <= 1 || dist2(prev_right, temp_right) > min_distance {
            derecha.push(temp_right);
            prev_right = temp_right;
        }
        prev_pressure = pressure;
        prev_vector = vector;
    }

    let first_point = puntos[0].point;
    let last_point = if n > 1 {
        puntos[n - 1].point
    } else {
        add(puntos[0].point, [1.0, 1.0])
    };

    // Un solo punto: un circulo. `(firstRadius || radius)` en JS: un cero
    // cuenta como ausente.
    if n == 1 {
        let r = match first_radius {
            Some(r) if r != 0.0 => r,
            _ => radius,
        };
        let start = prj(first_point, uni(per(sub(first_point, last_point))), -r);
        let step = 1.0 / START_CAP_SEGMENTS;
        let mut punto = Vec::new();
        let mut t = step;
        while t <= 1.0 {
            punto.push(rot_around(start, first_point, FIXED_PI * 2.0 * t));
            t += step;
        }
        return punto;
    }

    let mut tapa_inicio = Vec::new();
    let step = 1.0 / START_CAP_SEGMENTS;
    let mut t = step;
    while t <= 1.0 {
        tapa_inicio.push(rot_around(derecha[0], first_point, FIXED_PI * t));
        t += step;
    }

    let direction = per(neg(puntos[n - 1].vector));
    let start = prj(last_point, direction, radius);
    let mut tapa_fin = Vec::new();
    let step = 1.0 / END_CAP_SEGMENTS;
    let mut t = step;
    while t < 1.0 {
        tapa_fin.push(rot_around(start, last_point, FIXED_PI * 3.0 * t));
        t += step;
    }

    izquierda.extend(tapa_fin);
    izquierda.extend(derecha.into_iter().rev());
    izquierda.extend(tapa_inicio);
    izquierda
}

/// `getStroke`: puntos de entrada a contorno cerrado, listo para rellenar.
pub fn contorno(entrada: &[Entrada], o: &Opciones) -> Vec<V> {
    contorno_de_puntos(&puntos_del_trazo(entrada, o), o)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn opciones() -> Opciones {
        Opciones {
            size: 4.25,
            thinning: 0.6,
            smoothing: 0.5,
            streamline: 0.5,
            simular_presion: true,
            last: true,
            easing: seno,
        }
    }

    fn e(x: f64, y: f64) -> Entrada {
        Entrada { x, y, presion: None }
    }

    #[test]
    fn una_lista_vacia_da_un_contorno_vacio_sin_panico() {
        assert!(contorno(&[], &opciones()).is_empty());
    }

    #[test]
    fn un_clic_sin_arrastrar_es_un_circulo_de_trece_puntos() {
        let c = contorno(&[e(10.0, 10.0)], &opciones());
        assert_eq!(c.len(), 13);
        for p in &c {
            let r = (p[0] - 10.5).hypot(p[1] - 10.5);
            assert!(r > 0.5 && r < 5.0, "radio fuera de rango: {r}");
        }
    }

    #[test]
    fn un_tamano_de_cero_no_dibuja_nada() {
        let o = Opciones { size: 0.0, ..opciones() };
        assert!(contorno(&[e(0.0, 0.0), e(10.0, 0.0)], &o).is_empty());
    }

    #[test]
    fn ningun_punto_del_contorno_es_nan_ni_infinito() {
        let entrada: Vec<Entrada> = (0..200)
            .map(|i| e(i as f64 * 3.0, ((i as f64) / 5.0).sin() * 30.0))
            .collect();
        assert!(contorno(&entrada, &opciones())
            .iter()
            .all(|p| p[0].is_finite() && p[1].is_finite()));
    }

    #[test]
    fn con_presion_real_apretar_engorda_el_trazo() {
        let recta = |p: f64| -> Vec<Entrada> {
            (0..30)
                .map(|i| Entrada { x: i as f64 * 4.0, y: 0.0, presion: Some(p) })
                .collect()
        };
        let o = Opciones { simular_presion: false, ..opciones() };
        let alto = |c: Vec<V>| {
            let (mn, mx) = c.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
            mx - mn
        };
        assert!(alto(contorno(&recta(1.0), &o)) > alto(contorno(&recta(0.1), &o)));
    }
}
```

- [ ] **Step 4: Ejecutar pruebas**

Run: `cargo test -p pixpin-motor2d --test oraculo_tinta -- --test-threads=1` y `cargo test -p pixpin-motor2d tinta::freehand -- --test-threads=1`
Expected: PASS. Si el oráculo falla en un caso, **no ajustar la tolerancia**: comparar línea a línea con `dist/esm/index.js` de `perfect-freehand@1.2.0` (bajarlo con `npm pack perfect-freehand@1.2.0`).

- [ ] **Step 5: Commit**

```bash
git add crates/pixpin-motor2d/src/tinta crates/pixpin-motor2d/src/lib.rs crates/pixpin-motor2d/tests/oraculo_tinta.rs
git commit -m "perfect-freehand 1.2.0 portado tal cual: la pluma variable ya coincide con Excalidraw"
```

---

### Task 3: LaserPointer fiel (pluma constante)

**Files:**
- Create: `crates/pixpin-motor2d/src/tinta/laser.rs`
- Modify: `crates/pixpin-motor2d/src/tinta/mod.rs` (añadir `pub mod laser;`)
- Modify: `crates/pixpin-motor2d/tests/oraculo_tinta.rs` (segunda prueba)

**Interfaces:**
- Produces: `pixpin_motor2d::tinta::laser::contorno(puntos: &[[f64; 2]], size: f64, streamline: f64) -> Vec<[f64; 2]>`

- [ ] **Step 1: Prueba del oráculo (falla)**

Añadir a `tests/oraculo_tinta.rs`:

```rust
#[test]
fn la_pluma_constante_coincide_con_excalidraw_en_todos_los_casos() {
    let casos = casos("constant");
    assert!(casos.len() >= 8, "el oraculo tiene que traer casos constantes");
    for c in casos {
        let obtenido =
            pixpin_motor2d::tinta::laser::contorno(&c.puntos, c.grosor * 1.4, c.streamline);
        comparar(&c.nombre, &c.contorno, &obtenido);
    }
}
```

- [ ] **Step 2: Ejecutar y ver que falla**

Run: `cargo test -p pixpin-motor2d --test oraculo_tinta -- --test-threads=1`
Expected: FAIL, `could not find laser`.

- [ ] **Step 3: El porte**

`crates/pixpin-motor2d/src/tinta/laser.rs`:

```rust
//! Porte fiel de `LaserPointer` de Excalidraw (MIT, Copyright (c) 2020
//! Excalidraw), `packages/laser-pointer/src/state.ts` y `math.ts` @afa3a65,
//! publicado como `@excalidraw/laser-pointer@1.3.1`.
//!
//! Es la pluma de grosor constante: Excalidraw la usa con `simplify: 0`,
//! presion fijada a 1 y `sizeMapping = max(0.1, presion)`, asi que el
//! tamano es siempre `size`. Solo se porta ese camino: la simplificacion,
//! `keepHead` y la cola estable/inestable no cambian la salida con esas
//! opciones (la cola solo agrupa puntos, el contorno se hace con todos).
//!
//! Los bucles de angulo suman `PI / 16` en f64, como el original: ver
//! `freehand.rs` para por que eso importa.

use std::f64::consts::PI;

type V = [f64; 2];

const CORNER_DETECTION_MAX_ANGLE: f64 = 75.0;

fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1]]
}
fn smul(a: V, s: f64) -> V {
    [a[0] * s, a[1] * s]
}
fn norm(a: V) -> V {
    let m = (a[0].powi(2) + a[1].powi(2)).sqrt();
    [a[0] / m, a[1] / m]
}
fn rot(a: V, rad: f64) -> V {
    [
        rad.cos() * a[0] - rad.sin() * a[1],
        rad.sin() * a[0] + rad.cos() * a[1],
    ]
}
fn plerp(a: V, b: V, t: f64) -> V {
    add(a, smul(sub(b, a), t))
}
fn angle(p: V, p1: V, p2: V) -> f64 {
    (p2[1] - p[1]).atan2(p2[0] - p[0]) - (p1[1] - p[1]).atan2(p1[0] - p[0])
}
fn norm_angle(a: f64) -> f64 {
    a.sin().atan2(a.cos())
}
fn mag(a: V) -> f64 {
    (a[0].powi(2) + a[1].powi(2)).sqrt()
}
fn dist(a: V, b: V) -> f64 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
}

/// `addPoint` repetido: descarta duplicados exactos del punto ORIGINAL y
/// suaviza contra el ultimo punto YA suavizado.
fn suavizar(puntos: &[V], streamline: f64) -> Vec<V> {
    let mut originales: Vec<V> = Vec::new();
    let mut salida: Vec<V> = Vec::new();
    for &p in puntos {
        if originales.last() == Some(&p) {
            continue;
        }
        originales.push(p);
        let q = match salida.last() {
            Some(&ultimo) if streamline > 0.0 => plerp(ultimo, p, 1.0 - streamline),
            _ => p,
        };
        salida.push(q);
    }
    salida
}

pub fn contorno(puntos: &[V], size: f64, streamline: f64) -> Vec<V> {
    let points = suavizar(puntos, streamline);
    let len = points.len();
    if len == 0 {
        return Vec::new();
    }
    let paso = PI / 16.0;

    if len == 1 {
        let c = points[0];
        if size < 0.5 {
            return Vec::new();
        }
        let mut ps = Vec::new();
        let mut theta = 0.0;
        while theta <= PI * 2.0 {
            ps.push(add(c, smul(rot([1.0, 0.0], theta), size)));
            theta += paso;
        }
        ps.push(add(c, smul([1.0, 0.0], size)));
        return ps;
    }

    if len == 2 {
        let (c, n) = (points[0], points[1]);
        if size < 0.5 {
            return Vec::new();
        }
        let mut ps = Vec::new();
        let p_angle = angle(c, [c[0], c[1] - 100.0], n);
        let mut theta = p_angle;
        while theta <= PI + p_angle {
            ps.push(add(c, smul(rot([1.0, 0.0], theta), size)));
            theta += paso;
        }
        let mut theta = PI + p_angle;
        while theta <= PI * 2.0 + p_angle {
            ps.push(add(n, smul(rot([1.0, 0.0], theta), size)));
            theta += paso;
        }
        ps.push(ps[0]);
        return ps;
    }

    let mut forward: Vec<V> = Vec::new();
    let mut backward: Vec<V> = Vec::new();
    let mut prev_speed = 0.0;

    for i in 1..len - 1 {
        let (p, c, n) = (points[i - 1], points[i], points[i + 1]);
        let d = dist(p, c);
        let speed = prev_speed + (d - prev_speed) * 0.2;
        let c_size = size;

        let dir_pc = norm(sub(p, c));
        let dir_nc = norm(sub(n, c));
        let p1dir_pc = rot(dir_pc, PI / 2.0);
        let p2dir_pc = rot(dir_pc, -PI / 2.0);
        let p1dir_nc = rot(dir_nc, PI / 2.0);
        let p2dir_nc = rot(dir_nc, -PI / 2.0);
        let p1_pc = add(c, smul(p1dir_pc, c_size));
        let p2_pc = add(c, smul(p2dir_pc, c_size));
        let p1_nc = add(c, smul(p1dir_nc, c_size));
        let p2_nc = add(c, smul(p2dir_nc, c_size));
        let ftdir = add(p1dir_pc, p2dir_nc);
        let btdir = add(p2dir_pc, p1dir_nc);
        let pa_pc = add(
            c,
            smul(if mag(ftdir) == 0.0 { dir_pc } else { norm(ftdir) }, c_size),
        );
        let pa_nc = add(
            c,
            smul(if mag(btdir) == 0.0 { dir_nc } else { norm(btdir) }, c_size),
        );

        let c_angle = norm_angle(angle(c, p, n));
        let variance = if speed > 35.0 { 0.5 } else { 1.0 };
        let d_angle = (CORNER_DETECTION_MAX_ANGLE / 180.0) * PI * variance;

        if c_angle.abs() < d_angle {
            let t_angle = norm_angle(PI - c_angle).abs();
            if t_angle == 0.0 {
                // El original hace `continue` ANTES de `prevSpeed = speed`.
                continue;
            }
            if c_angle < 0.0 {
                backward.push(p2_pc);
                backward.push(pa_nc);
                let mut theta = 0.0;
                while theta <= t_angle {
                    forward.push(add(c, rot(smul(p1dir_pc, c_size), theta)));
                    theta += t_angle / 4.0;
                }
                let mut theta = t_angle;
                while theta >= 0.0 {
                    backward.push(add(c, rot(smul(p1dir_pc, c_size), theta)));
                    theta -= t_angle / 4.0;
                }
                backward.push(pa_nc);
                backward.push(p1_nc);
            } else {
                forward.push(p1_pc);
                forward.push(pa_pc);
                let mut theta = 0.0;
                while theta <= t_angle {
                    backward.push(add(c, rot(smul(p1dir_pc, -c_size), -theta)));
                    theta += t_angle / 4.0;
                }
                let mut theta = t_angle;
                while theta >= 0.0 {
                    forward.push(add(c, rot(smul(p1dir_pc, -c_size), -theta)));
                    theta -= t_angle / 4.0;
                }
                forward.push(pa_pc);
                forward.push(p2_nc);
            }
        } else {
            forward.push(pa_pc);
            backward.push(pa_nc);
        }
        prev_speed = speed;
    }

    let first = points[0];
    let second = points[1];
    let penultimate = points[len - 2];
    let ultimate = points[len - 1];
    let ppdir_fs = rot(norm(sub(second, first)), -PI / 2.0);
    let ppdir_pu = rot(norm(sub(penultimate, ultimate)), PI / 2.0);

    let mut start_cap: Vec<V> = Vec::new();
    if size > 0.1 {
        let mut theta = 0.0;
        while theta <= PI {
            start_cap.insert(0, add(first, rot(smul(ppdir_fs, size), -theta)));
            theta += paso;
        }
        start_cap.insert(0, add(first, smul(ppdir_fs, -size)));
    } else {
        start_cap.push(first);
    }

    let mut end_cap: Vec<V> = Vec::new();
    let mut theta = 0.0;
    while theta <= PI * 3.0 {
        end_cap.push(add(ultimate, rot(smul(ppdir_pu, -size), -theta)));
        theta += paso;
    }

    let mut salida = start_cap.clone();
    salida.extend(forward);
    salida.extend(end_cap.into_iter().rev());
    salida.extend(backward.into_iter().rev());
    if let Some(&primero) = start_cap.first() {
        salida.push(primero);
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sin_puntos_no_hay_contorno() {
        assert!(contorno(&[], 1.4, 0.5).is_empty());
    }

    #[test]
    fn un_punto_con_tamano_menor_de_medio_pixel_no_se_dibuja() {
        assert!(contorno(&[[0.0, 0.0]], 0.4, 0.5).is_empty());
    }

    #[test]
    fn los_puntos_repetidos_no_cambian_el_contorno() {
        let a = contorno(&[[0.0, 0.0], [10.0, 0.0], [20.0, 5.0]], 1.4, 0.5);
        let b = contorno(&[[0.0, 0.0], [10.0, 0.0], [10.0, 0.0], [20.0, 5.0]], 1.4, 0.5);
        assert_eq!(a, b);
    }

    #[test]
    fn el_grosor_de_una_recta_larga_es_constante() {
        let recta: Vec<V> = (0..50).map(|i| [i as f64 * 5.0, 0.0]).collect();
        let c = contorno(&recta, 2.0, 0.5);
        // Lejos de las tapas, todos los puntos quedan a `size` de la linea.
        let medios: Vec<&V> = c.iter().filter(|p| p[0] > 20.0 && p[0] < 200.0).collect();
        assert!(!medios.is_empty());
        assert!(medios.iter().all(|p| (p[1].abs() - 2.0).abs() < 1e-6));
    }
}
```

- [ ] **Step 4: Ejecutar pruebas**

Run: `cargo test -p pixpin-motor2d --test oraculo_tinta -- --test-threads=1` y `cargo test -p pixpin-motor2d tinta::laser -- --test-threads=1`
Expected: PASS. Si difiere, comparar con `node_modules/@excalidraw/laser-pointer/dist/esm.js`.

- [ ] **Step 5: Commit**

```bash
git add crates/pixpin-motor2d/src/tinta crates/pixpin-motor2d/tests/oraculo_tinta.rs
git commit -m "LaserPointer portado: la pluma constante de Excalidraw, con sus esquinas a 75 grados"
```

---

### Task 4: Las opciones de pluma en el modelo y en el fichero

**Files:**
- Modify: `crates/pixpin-motor2d/src/tinta/mod.rs`
- Modify: `crates/pixpin-motor2d/src/elemento.rs:47-51` (`Figura::Lapiz`) y `:248` (el `match` que la reconstruye)
- Modify: `crates/pixpin-motor2d/src/excalidraw.rs:273-285` (leer) y `:405-413` (escribir)
- Modify (añadir el campo en cada literal `Figura::Lapiz { .. }` que no compile): `cache.rs:156`, `elemento.rs:270,347`, `escena.rs:872,934`, `formato.rs:73`, `gesto.rs:437`, `impacto.rs:337`, `transformar.rs:481,541,670,854`, `crates/pixpin-ui/src/anotador.rs:511`, `crates/pixpin-motor2d/tests/puertas.rs:187`

**Interfaces:**
- Consumes: `tinta::freehand::{contorno, Entrada, Opciones, seno}`, `tinta::laser::contorno` (Tareas 2–3).
- Produces:
  - `pub enum Variabilidad { Variable, Constante }` (serde `"variable"` / `"constant"`, por defecto `Variable`)
  - `pub struct OpcionesTinta { pub variabilidad: Variabilidad, pub streamline: f32 }` (Default `Variable` + `STREAMLINE_RATON`; serde con los nombres de Excalidraw `variability` / `streamline`)
  - `pub const STREAMLINE_RATON: f32 = 0.5; STREAMLINE_LAPIZ: f32 = 0.2; FACTOR_VARIABLE: f32 = 4.25; FACTOR_CONSTANTE: f32 = 1.4; GROSOR_FINO: f32 = 0.5; GROSOR_MEDIO: f32 = 1.0; GROSOR_GRUESO: f32 = 2.0;`
  - `pub fn contorno_de_lapiz(puntos: &[Punto2], presiones: &[f32], grosor: f32, opciones: Option<OpcionesTinta>) -> Vec<Punto2>`
  - `pub fn contorno_de_resaltador(puntos: &[Punto2], grosor: f32) -> Vec<Punto2>`
  - `Figura::Lapiz { puntos, presiones, #[serde(default)] opciones: Option<OpcionesTinta> }`. **`None` = trazo legado** (anterior a E1): su `grosor` era el tamaño en píxeles, así que se pinta con `grosor / FACTOR_VARIABLE` para que un `.pixpin2d` viejo no engorde 4,25 veces.

- [ ] **Step 1: Pruebas (fallan)**

Al final de `tinta/mod.rs`:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;

    fn recta() -> Vec<Punto2> {
        (0..40).map(|i| Punto2::nuevo(i as f32 * 5.0, 0.0)).collect()
    }

    fn alto(c: &[Punto2]) -> f32 {
        let (mn, mx) = c
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        mx - mn
    }

    #[test]
    fn un_trazo_legado_no_engorda_al_abrirlo_con_la_tinta_nueva() {
        // Grosor 4,25 sin opciones = el mismo trazo que grosor 1 con opciones.
        let legado = contorno_de_lapiz(&recta(), &[], 4.25, None);
        let nuevo = contorno_de_lapiz(&recta(), &[], 1.0, Some(OpcionesTinta::default()));
        assert_eq!(legado, nuevo);
    }

    #[test]
    fn la_pluma_constante_da_otra_forma_que_la_variable() {
        let v = contorno_de_lapiz(&recta(), &[], 1.0, Some(OpcionesTinta::default()));
        let constante = OpcionesTinta { variabilidad: Variabilidad::Constante, ..Default::default() };
        let c = contorno_de_lapiz(&recta(), &[], 1.0, Some(constante));
        assert_ne!(v, c);
    }

    #[test]
    fn presiones_de_otra_longitud_se_tratan_como_simuladas() {
        let p = recta();
        let simulada = contorno_de_lapiz(&p, &[], 1.0, Some(OpcionesTinta::default()));
        let rota = contorno_de_lapiz(&p, &[0.9, 0.9], 1.0, Some(OpcionesTinta::default()));
        assert_eq!(simulada, rota);
    }

    #[test]
    fn mas_grosor_es_mas_ancho() {
        let o = Some(OpcionesTinta::default());
        assert!(
            alto(&contorno_de_lapiz(&recta(), &[], GROSOR_GRUESO, o))
                > alto(&contorno_de_lapiz(&recta(), &[], GROSOR_FINO, o))
        );
    }

    #[test]
    fn las_opciones_viajan_con_los_nombres_de_excalidraw() {
        let o = OpcionesTinta { variabilidad: Variabilidad::Constante, streamline: 0.2 };
        let json = serde_json::to_string(&o).unwrap();
        assert!(json.contains("\"variability\":\"constant\""), "{json}");
    }
}
```

En `excalidraw.rs`, dentro de su `mod pruebas` (usar las funciones de lectura/escritura que ya usan sus pruebas de la línea ~606; aquí se escriben `leer` y `escribir`):

```rust
#[test]
fn un_freedraw_de_excalidraw_trae_sus_opciones_de_pluma_y_vuelve_igual() {
    use crate::tinta::{OpcionesTinta, Variabilidad};
    let texto = r##"{"type":"excalidraw","version":2,"elements":[{"id":"a","type":"freedraw",
        "x":0,"y":0,"width":10,"height":10,"angle":0,"strokeColor":"#1e1e1e",
        "backgroundColor":"transparent","strokeWidth":2,"seed":1,
        "points":[[0,0],[5,5],[10,10]],"pressures":[0.2,0.5,0.9],"simulatePressure":false,
        "strokeOptions":{"variability":"constant","streamline":0.2}}]}"##;
    let escena = leer(texto).expect("se lee");
    let Figura::Lapiz { presiones, opciones, .. } = &escena.elementos()[0].figura else {
        panic!("tendria que ser un lapiz")
    };
    assert_eq!(presiones.len(), 3);
    assert_eq!(
        *opciones,
        Some(OpcionesTinta { variabilidad: Variabilidad::Constante, streamline: 0.2 })
    );
    let vuelta = escribir(&escena);
    assert!(vuelta.contains("\"variability\":\"constant\""), "{vuelta}");
    assert!(vuelta.contains("\"simulatePressure\":false"), "{vuelta}");
}

#[test]
fn con_simulate_pressure_verdadero_se_ignoran_las_presiones_guardadas() {
    let texto = r##"{"type":"excalidraw","version":2,"elements":[{"id":"a","type":"freedraw",
        "x":0,"y":0,"width":10,"height":10,"strokeWidth":1,"seed":1,
        "points":[[0,0],[5,5]],"pressures":[0.2,0.5],"simulatePressure":true}]}"##;
    let escena = leer(texto).expect("se lee");
    let Figura::Lapiz { presiones, opciones, .. } = &escena.elementos()[0].figura else {
        panic!()
    };
    assert!(presiones.is_empty());
    assert_eq!(*opciones, Some(crate::tinta::OpcionesTinta::default()));
}
```

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p pixpin-motor2d -- --test-threads=1`
Expected: FAIL de compilación (`OpcionesTinta` no existe, `Figura::Lapiz` no tiene `opciones`).

- [ ] **Step 3: Implementación**

`tinta/mod.rs` completo (encima de las pruebas):

```rust
//! La tinta de Excalidraw, portada (E1). Ver `docs/excalidraw/mapa.md`.
//!
//! `freehand` y `laser` son portes literales y trabajan en f64. Este modulo
//! es la envoltura de Excalidraw (`getFreedrawOutlinePoints` en
//! `packages/element/src/shape.ts` @afa3a65): elige pluma, aplica los
//! factores de tamano y pasa a `Punto2`.

pub mod freehand;
pub mod laser;

use serde::{Deserialize, Serialize};

use crate::vector::Punto2;

pub const STREAMLINE_RATON: f32 = 0.5;
pub const STREAMLINE_LAPIZ: f32 = 0.2;
/// `VARIABLE_WIDTH_FREEDRAW.SIZE_FACTOR`: afinado a ojo por Excalidraw.
pub const FACTOR_VARIABLE: f32 = 4.25;
/// `CONSTANT_WIDTH_FREEDRAW.SIZE_FACTOR`.
pub const FACTOR_CONSTANTE: f32 = 1.4;
/// `FREEDRAW_STROKE_WIDTH`: la mitad que el resto de figuras (esquema 2.0).
pub const GROSOR_FINO: f32 = 0.5;
pub const GROSOR_MEDIO: f32 = 1.0;
pub const GROSOR_GRUESO: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Variabilidad {
    #[default]
    #[serde(rename = "variable")]
    Variable,
    #[serde(rename = "constant")]
    Constante,
}

/// `strokeOptions` de Excalidraw, con sus mismos nombres en el fichero.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OpcionesTinta {
    #[serde(rename = "variability", default)]
    pub variabilidad: Variabilidad,
    #[serde(default = "streamline_raton")]
    pub streamline: f32,
}

fn streamline_raton() -> f32 {
    STREAMLINE_RATON
}

impl Default for OpcionesTinta {
    fn default() -> Self {
        Self {
            variabilidad: Variabilidad::Variable,
            streamline: STREAMLINE_RATON,
        }
    }
}

fn a_punto(v: [f64; 2]) -> Punto2 {
    Punto2::nuevo(v[0] as f32, v[1] as f32)
}

/// El contorno de un trazo de lapiz, como lo pinta Excalidraw.
///
/// `opciones == None` es un trazo de antes de E1: su `grosor` era el tamano
/// en pixeles y no un `strokeWidth`, asi que se divide por el factor para
/// que un dibujo viejo no se abra cuatro veces mas gordo.
pub fn contorno_de_lapiz(
    puntos: &[Punto2],
    presiones: &[f32],
    grosor: f32,
    opciones: Option<OpcionesTinta>,
) -> Vec<Punto2> {
    let (o, grosor) = match opciones {
        Some(o) => (o, grosor),
        None => (OpcionesTinta::default(), grosor / FACTOR_VARIABLE),
    };
    match o.variabilidad {
        Variabilidad::Constante => {
            let pts: Vec<[f64; 2]> = puntos.iter().map(|p| [p.x as f64, p.y as f64]).collect();
            laser::contorno(&pts, (grosor * FACTOR_CONSTANTE) as f64, o.streamline as f64)
                .into_iter()
                .map(a_punto)
                .collect()
        }
        Variabilidad::Variable => {
            // Excalidraw guarda presiones solo si son reales; una lista de
            // otra longitud no puede venir de un lapiz y se trata como
            // simulada en vez de leer fuera de rango.
            let simular = presiones.len() != puntos.len();
            let entrada: Vec<freehand::Entrada> = puntos
                .iter()
                .enumerate()
                .map(|(i, p)| freehand::Entrada {
                    x: p.x as f64,
                    y: p.y as f64,
                    presion: if simular { None } else { Some(presiones[i] as f64) },
                })
                .collect();
            let op = freehand::Opciones {
                size: (grosor * FACTOR_VARIABLE) as f64,
                thinning: 0.6,
                smoothing: 0.5,
                streamline: o.streamline as f64,
                simular_presion: simular,
                last: true,
                easing: freehand::seno,
            };
            freehand::contorno(&entrada, &op).into_iter().map(a_punto).collect()
        }
    }
}

/// El resaltador (D45): grueso, sin adelgazar y translucido. No existe en
/// Excalidraw; se hace con la misma maquina con `thinning = 0`, que da
/// grosor constante y tapas redondas. `grosor * 3` conserva su tamano de
/// antes de E1.
pub fn contorno_de_resaltador(puntos: &[Punto2], grosor: f32) -> Vec<Punto2> {
    let entrada: Vec<freehand::Entrada> = puntos
        .iter()
        .map(|p| freehand::Entrada { x: p.x as f64, y: p.y as f64, presion: None })
        .collect();
    let op = freehand::Opciones {
        size: (grosor * 3.0) as f64,
        thinning: 0.0,
        smoothing: 0.5,
        streamline: STREAMLINE_RATON as f64,
        simular_presion: true,
        last: true,
        easing: freehand::seno,
    };
    freehand::contorno(&entrada, &op).into_iter().map(a_punto).collect()
}
```

`elemento.rs`, `Figura::Lapiz`:

```rust
    Lapiz {
        puntos: Vec<Punto2>,
        #[serde(default)]
        presiones: Vec<f32>,
        /// La pluma de Excalidraw (`strokeOptions`). `None` = trazo de antes
        /// de E1, cuyo grosor eran pixeles: ver `tinta::contorno_de_lapiz`.
        #[serde(default)]
        opciones: Option<crate::tinta::OpcionesTinta>,
    },
```

En `elemento.rs:248` añadir `opciones` al patrón y al literal que reconstruye.

`excalidraw.rs`, rama `"freedraw"` al leer:

```rust
        "freedraw" => {
            // `simulatePressure: true` manda sobre `pressures`: Excalidraw las
            // ignora al pintar aunque vengan en el fichero.
            let simulada = v.get("simulatePressure").and_then(|b| b.as_bool()) == Some(true);
            Figura::Lapiz {
                puntos: puntos_desde(v, x, y),
                presiones: if simulada {
                    Vec::new()
                } else {
                    v.get("pressures")
                        .and_then(|p| p.as_array())
                        .map(|l| l.iter().filter_map(|p| p.as_f64()).map(|p| p as f32).collect())
                        .unwrap_or_default()
                },
                // Todo freedraw de Excalidraw lleva el grosor en `strokeWidth`,
                // con o sin `strokeOptions`: nunca es legado.
                opciones: Some(
                    v.get("strokeOptions")
                        .and_then(|s| serde_json::from_value(s.clone()).ok())
                        .unwrap_or_default(),
                ),
            }
        }
```

Rama `Figura::Lapiz` al escribir:

```rust
        Figura::Lapiz { puntos, presiones, opciones } => {
            mapa.insert("points".into(), puntos_hacia(puntos, e.x, e.y));
            mapa.insert(
                "pressures".into(),
                Value::Array(presiones.iter().map(|p| Value::from(*p as f64)).collect()),
            );
            mapa.insert("simulatePressure".into(), Value::Bool(presiones.is_empty()));
            let o = opciones.unwrap_or_default();
            mapa.insert("strokeOptions".into(), serde_json::to_value(o).unwrap_or(Value::Null));
            // Un trazo legado se exporta con su grosor ya convertido, para
            // que Excalidraw y el movil lo vean del mismo ancho que aqui.
            if opciones.is_none() {
                mapa.insert(
                    "strokeWidth".into(),
                    Value::from((e.grosor / crate::tinta::FACTOR_VARIABLE) as f64),
                );
            }
        }
```

> `strokeWidth` se inserta en la línea 378, antes de este `match`, así que la inserción legada lo sobreescribe. Si el orden fuera otro, mover la inserción legada detrás.

Resto de literales que no compilen: en código que crea trazos nuevos (`gesto.rs:437`, `anotador.rs:511`) poner `opciones: Some(crate::tinta::OpcionesTinta::default())` (las Tareas 8 y 11 lo afinan); en pruebas y en `formato.rs`, `escena.rs`, `impacto.rs`, `transformar.rs`, `cache.rs`, `puertas.rs` poner `opciones: None`. Los patrones `Figura::Lapiz { puntos, .. }` no cambian.

- [ ] **Step 4: Ejecutar pruebas**

Run: `cargo test -p pixpin-motor2d -p pixpin-ui -- --test-threads=1`
Expected: PASS, incluidos `formato::pruebas::un_fichero_de_una_version_futura_sigue_abriendo` y los de ida y vuelta del puente.

- [ ] **Step 5: Commit**

```bash
git add -A crates/pixpin-motor2d crates/pixpin-ui
git commit -m "La pluma de Excalidraw se guarda con el trazo, y un dibujo viejo no engorda al abrirlo"
```

---

### Task 5: `Pintor::tinta` — cuadráticas por puntos medios, relleno winding y pinceles cacheados

**Files:**
- Create: `crates/pixpin-render/src/tinta.rs`
- Modify: `crates/pixpin-render/src/lib.rs` (`pub mod tinta;` y `pub use tinta::{PasoTrayecto, pasos_de_tinta};`)
- Modify: `crates/pixpin-render/src/motor.rs:111-140` (caché de pinceles)
- Modify: `crates/pixpin-render/src/lienzo.rs:186-195` (`Pintor::pincel` delega) y junto a `poligono` (`:514`)
- Modify: `crates/pixpin-render/Cargo.toml` (`[dev-dependencies] serde_json = "1.0"`)

**Interfaces:**
- Consumes: `crates/pixpin-motor2d/tests/oraculo/tinta.json` (solo lectura en pruebas, por ruta; no hay dependencia entre crates).
- Produces:
  - `pub enum PasoTrayecto { Mover((f32, f32)), Cuadratica { control: (f32, f32), fin: (f32, f32) }, Linea((f32, f32)), Cerrar }`
  - `pub fn pasos_de_tinta(contorno: &[(f32, f32)]) -> Vec<PasoTrayecto>`
  - `impl Pintor { pub fn tinta(&self, contorno: &[(f32, f32)], color: Color); pub(crate) fn geometria_tinta(&self, contorno: &[(f32, f32)]) -> Option<ID2D1PathGeometry1> }`
  - `impl MotorRender { pub(crate) fn pincel(&self, color: Color) -> Option<ID2D1SolidColorBrush> }`

- [ ] **Step 1: Pruebas puras (fallan)**

Al final de `crates/pixpin-render/src/tinta.rs`:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_contorno_vacio_no_da_ningun_paso() {
        assert!(pasos_de_tinta(&[]).is_empty());
    }

    #[test]
    fn cada_vertice_es_el_control_de_una_cuadratica_que_acaba_en_el_punto_medio() {
        let c = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)];
        assert_eq!(
            pasos_de_tinta(&c),
            vec![
                PasoTrayecto::Mover((0.0, 0.0)),
                PasoTrayecto::Cuadratica { control: (0.0, 0.0), fin: (5.0, 0.0) },
                PasoTrayecto::Cuadratica { control: (10.0, 0.0), fin: (10.0, 5.0) },
                PasoTrayecto::Cuadratica { control: (10.0, 10.0), fin: (5.0, 5.0) },
                PasoTrayecto::Linea((0.0, 0.0)),
                PasoTrayecto::Cerrar,
            ]
        );
    }

    /// Los numeros del SVG de Excalidraw, en orden, sin las letras.
    fn numeros_svg(svg: &str) -> Vec<f32> {
        svg.split_whitespace()
            .filter(|t| !t.chars().all(|c| c.is_ascii_alphabetic()))
            .flat_map(|t| {
                t.split(',')
                    .map(|n| n.parse::<f32>().expect("numero"))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn numeros_pasos(p: &[PasoTrayecto]) -> Vec<f32> {
        p.iter()
            .flat_map(|paso| match *paso {
                PasoTrayecto::Mover(a) | PasoTrayecto::Linea(a) => vec![a.0, a.1],
                PasoTrayecto::Cuadratica { control, fin } => {
                    vec![control.0, control.1, fin.0, fin.1]
                }
                PasoTrayecto::Cerrar => vec![],
            })
            .collect()
    }

    #[test]
    fn el_trayecto_coincide_con_get_svg_path_from_stroke_de_excalidraw() {
        let ruta = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../pixpin-motor2d/tests/oraculo/tinta.json"
        );
        let texto = std::fs::read_to_string(ruta).expect("falta el oraculo (Tarea 1)");
        let v: serde_json::Value = serde_json::from_str(&texto).unwrap();
        for caso in v["casos"].as_array().unwrap() {
            let contorno: Vec<(f32, f32)> = caso["contorno"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| (p[0].as_f64().unwrap() as f32, p[1].as_f64().unwrap() as f32))
                .collect();
            let esperado = numeros_svg(caso["svg"].as_str().unwrap());
            let obtenido = numeros_pasos(&pasos_de_tinta(&contorno));
            assert_eq!(esperado.len(), obtenido.len(), "{}", caso["nombre"]);
            for (e, o) in esperado.iter().zip(&obtenido) {
                // Excalidraw TRUNCA a dos decimales: la diferencia llega a 0,01.
                assert!((e - o).abs() <= 0.011, "{}: {e} contra {o}", caso["nombre"]);
            }
        }
    }
}
```

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p pixpin-render tinta -- --test-threads=1`
Expected: FAIL de compilación (`pasos_de_tinta` no existe).

- [ ] **Step 3: Implementación**

`tinta.rs` (encima de las pruebas):

```rust
//! Pintar la tinta como Excalidraw: contorno cerrado, curvas cuadraticas
//! por puntos medios y relleno *winding*.
//!
//! `pasos_de_tinta` es `getSvgPathFromStroke` de Excalidraw
//! (`packages/element/src/shape.ts` @afa3a65, MIT) sin el recorte a dos
//! decimales, que solo existe para acortar el SVG. Es pura: se prueba sin
//! GPU contra el oraculo.
//!
//! Por que *winding* y no la regla por defecto de Direct2D (alternada): el
//! contorno de perfect-freehand se cruza consigo mismo en bucles, retrocesos
//! y en los arcos de las esquinas. Con la alternada cada cruce abre un
//! agujero; Canvas2D rellena con *nonzero*, que es *winding*. `Pintor::velo`
//! SI necesita la alternada para su hueco, por eso esto es una geometria
//! aparte y no un cambio en `Pintor::geometria`.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PasoTrayecto {
    Mover((f32, f32)),
    Cuadratica { control: (f32, f32), fin: (f32, f32) },
    Linea((f32, f32)),
    Cerrar,
}

/// `M p0 Q p0 m01 p1 m12 ... pN mN0 L p0 Z`.
pub fn pasos_de_tinta(contorno: &[(f32, f32)]) -> Vec<PasoTrayecto> {
    let Some(&primero) = contorno.first() else {
        return Vec::new();
    };
    let medio = |a: (f32, f32), b: (f32, f32)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let max = contorno.len() - 1;
    let mut pasos = Vec::with_capacity(contorno.len() + 3);
    pasos.push(PasoTrayecto::Mover(primero));
    for (i, &p) in contorno.iter().enumerate() {
        let siguiente = if i == max { primero } else { contorno[i + 1] };
        pasos.push(PasoTrayecto::Cuadratica {
            control: p,
            fin: medio(p, siguiente),
        });
    }
    pasos.push(PasoTrayecto::Linea(primero));
    pasos.push(PasoTrayecto::Cerrar);
    pasos
}
```

`lienzo.rs`, dentro de `impl Pintor<'_>` junto a `poligono`:

```rust
    /// Rellena el contorno de un trazo de tinta (ver `crate::tinta`).
    pub fn tinta(&self, contorno: &[(f32, f32)], color: Color) {
        if contorno.len() < 3 {
            return;
        }
        let Some(geometria) = self.geometria_tinta(contorno) else {
            return;
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; geometria y pincel vivos.
            unsafe { self.motor.contexto().FillGeometry(&geometria, &p, None) };
        }
    }

    pub(crate) fn geometria_tinta(&self, contorno: &[(f32, f32)]) -> Option<ID2D1PathGeometry1> {
        use crate::tinta::{PasoTrayecto, pasos_de_tinta};
        use windows::Win32::Graphics::Direct2D::Common::{
            D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED, D2D1_FILL_MODE_WINDING,
        };
        use windows::Win32::Graphics::Direct2D::D2D1_QUADRATIC_BEZIER_SEGMENT;
        let v = |p: (f32, f32)| Vector2 { X: p.0, Y: p.1 };

        // SAFETY: la geometria se abre, se rellena y se cierra aqui mismo;
        // si algo falla a mitad se descarta sin dibujarla.
        unsafe {
            let geometria = self.motor.fabrica().CreatePathGeometry().ok()?;
            let sumidero = geometria.Open().ok()?;
            sumidero.SetFillMode(D2D1_FILL_MODE_WINDING);
            // Las cuadraticas seguidas se mandan en bloque: una llamada COM
            // por tramo seria buena parte del coste de un trazo largo.
            let mut tanda: Vec<D2D1_QUADRATIC_BEZIER_SEGMENT> = Vec::new();
            for paso in pasos_de_tinta(contorno) {
                if !matches!(paso, PasoTrayecto::Cuadratica { .. }) && !tanda.is_empty() {
                    sumidero.AddQuadraticBeziers(&tanda);
                    tanda.clear();
                }
                match paso {
                    PasoTrayecto::Mover(p) => sumidero.BeginFigure(v(p), D2D1_FIGURE_BEGIN_FILLED),
                    PasoTrayecto::Cuadratica { control, fin } => {
                        tanda.push(D2D1_QUADRATIC_BEZIER_SEGMENT {
                            point1: v(control),
                            point2: v(fin),
                        })
                    }
                    PasoTrayecto::Linea(p) => sumidero.AddLine(v(p)),
                    PasoTrayecto::Cerrar => sumidero.EndFigure(D2D1_FIGURE_END_CLOSED),
                }
            }
            sumidero.Close().ok()?;
            Some(geometria)
        }
    }
```

> Si en `windows` 0.62 `D2D1_FILL_MODE_WINDING` o `D2D1_QUADRATIC_BEZIER_SEGMENT` viven en otro módulo, localizarlos con `rg -n "pub const D2D1_FILL_MODE_WINDING|pub struct D2D1_QUADRATIC_BEZIER_SEGMENT" "$USERPROFILE/.cargo/registry/src"` y corregir solo el `use`.

Pinceles. En `motor.rs`, campo nuevo en `MotorRender` (inicializado con `pinceles: std::cell::RefCell::new(Vec::new())` en `nuevo`):

```rust
    /// Pinceles ya creados, por color. Crear uno por primitiva costaba casi
    /// tanto como la geometria (diagnostico de E1). Uno por color y no uno
    /// solo con `SetColor`: hay primitivas que piden dos pinceles a la vez y
    /// el segundo pisaria el color del primero.
    pinceles: std::cell::RefCell<Vec<([u32; 4], ID2D1SolidColorBrush)>>,
```

y en `impl MotorRender`:

```rust
    /// Tope pequeno: un dibujo real usa pocos colores, y el tope evita que un
    /// degradado pintado a mano llene la memoria de pinceles.
    const MAX_PINCELES: usize = 32;

    pub(crate) fn pincel(&self, color: Color) -> Option<ID2D1SolidColorBrush> {
        let clave = [
            color.r.to_bits(),
            color.g.to_bits(),
            color.b.to_bits(),
            color.a.to_bits(),
        ];
        let mut lista = self.pinceles.borrow_mut();
        if let Some((_, p)) = lista.iter().find(|(c, _)| *c == clave) {
            return Some(p.clone());
        }
        // SAFETY: crear un pincel sobre el contexto vivo no tiene mas
        // precondiciones.
        let p = unsafe { self.contexto.CreateSolidColorBrush(&color.a_d2d(), None).ok()? };
        if lista.len() >= Self::MAX_PINCELES {
            lista.remove(0);
        }
        lista.push((clave, p.clone()));
        Some(p)
    }
```

En `lienzo.rs`, el cuerpo de `Pintor::pincel` pasa a ser `self.motor.pincel(color)`.

- [ ] **Step 4: Ejecutar pruebas**

Run: `cargo test -p pixpin-render -- --test-threads=1` y, con sesión de escritorio, `cargo test -p pixpin-render -- --ignored --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/pixpin-render
git commit -m "Pintor::tinta: curvas por puntos medios y relleno winding, sin agujeros al cruzarse"
```

---

### Task 6: `Orden::Tinta` y el umbral de 2 px

**Files:**
- Modify: `crates/pixpin-motor2d/src/pintado.rs` (enum `Orden`, `girar_orden`, ramas `Lapiz`/`Resaltador` de `ordenes`, `TINTA_MINIMA_PX`)
- Delete: `crates/pixpin-motor2d/src/trazo.rs`; Modify `crates/pixpin-motor2d/src/lib.rs:38,61`
- Modify: `crates/pixpin-motor2d/tests/puertas.rs` (imports, `puntos_de`, pruebas del umbral)
- Modify: `apps/pixpin/src/ventana_editor.rs:740` (`dibujar_orden`), `apps/pixpin/src/capa.rs:499` (`pintar_ordenes`), `crates/pixpin-pin/src/ventana.rs:1930`

**Interfaces:**
- Consumes: `tinta::{contorno_de_lapiz, contorno_de_resaltador}` (Tarea 4), `Pintor::tinta` (Tarea 5).
- Produces: `Orden::Tinta { contorno: Vec<Punto2>, color: ColorRgba }`; `pub const TINTA_MINIMA_PX: f32 = 2.0`.

- [ ] **Step 1: Pruebas (fallan)**

En `pintado.rs`, `mod pruebas` (si el ayudante que construye un `Elemento` de prueba tiene otro nombre, usar ese en vez de `elemento_de_prueba`):

```rust
#[test]
fn un_lapiz_se_pinta_como_tinta_y_no_como_poligono() {
    let e = Elemento {
        figura: Figura::Lapiz {
            puntos: (0..20).map(|i| Punto2::nuevo(i as f32 * 4.0, 0.0)).collect(),
            presiones: Vec::new(),
            opciones: Some(crate::tinta::OpcionesTinta::default()),
        },
        ..elemento_de_prueba()
    };
    assert!(matches!(ordenes(&e).first(), Some(Orden::Tinta { .. })));
}

#[test]
fn una_firma_de_cuarenta_pixeles_sigue_siendo_tinta() {
    // Antes de E1, por debajo de 48 px salia una raya de grosor fijo: las
    // letras y las firmas pequenas nunca se veian como tinta.
    let e = Elemento {
        figura: Figura::Lapiz {
            puntos: (0..20)
                .map(|i| Punto2::nuevo(i as f32 * 2.0, (i as f32).sin() * 5.0))
                .collect(),
            presiones: Vec::new(),
            opciones: Some(crate::tinta::OpcionesTinta::default()),
        },
        ..elemento_de_prueba()
    };
    assert!(matches!(ordenes_a_distancia(&e, 1.0).first(), Some(Orden::Tinta { .. })));
}
```

En `tests/puertas.rs`, sustituir `un_trazo_diminuto_se_dibuja_como_una_raya` por:

```rust
#[test]
fn solo_lo_que_mide_menos_de_dos_pixeles_en_pantalla_se_dibuja_como_raya() {
    let escena = dibujo_como_el_del_movil();
    let e = escena.visibles().next().expect("el dibujo tiene elementos");
    let (x0, y0, x1, y1) = e.caja();
    let lado = (x1 - x0).max(y1 - y0);
    assert!(matches!(
        pixpin_motor2d::ordenes_a_distancia(e, 1.5 / lado).first(),
        Some(pixpin_motor2d::Orden::Polilinea { .. })
    ));
    assert!(matches!(
        pixpin_motor2d::ordenes_a_distancia(e, 10.0 / lado).first(),
        Some(pixpin_motor2d::Orden::Tinta { .. })
    ));
}
```

Y sustituir `de_lejos_el_dibujo_cuesta_una_decima_parte` por esta versión. El umbral pasó de 48 a 2 px por decisión D115 (aprobada): la economía de lejos la dan ahora la rejilla, la capa congelada y la caché de realizaciones (Tareas 9–10), no quitar tinta.

```rust
#[test]
fn de_lejos_nunca_cuesta_mas_que_el_dibujo_entero() {
    // D115: la tinta solo se sustituye por debajo de 2 px. Esta puerta ya
    // no promete una decima parte; promete que alejarse no fabrica MAS
    // geometria, y que al 5 % algo se sigue viendo.
    let escena = dibujo_como_el_del_movil();
    let entero = puntos_de(&ordenes_de_escena(&escena));
    let c = pixpin_motor2d::Camara { x: 100.0, y: 150.0, zoom: 0.05 };
    let lejos = puntos_de(&pixpin_motor2d::ordenes_de_escena_vista(&escena, &c, 1920.0, 1080.0));
    assert!(lejos > 0, "al 5 % no se pinto nada");
    assert!(lejos <= entero, "de lejos no puede costar mas que entero: {lejos} > {entero}");
}
```

`puntos_de` cuenta también la tinta:

```rust
            pixpin_motor2d::Orden::Poligono { puntos, .. }
            | pixpin_motor2d::Orden::Tinta { contorno: puntos, .. }
            | pixpin_motor2d::Orden::Polilinea { puntos, .. }
            | pixpin_motor2d::Orden::Relleno { puntos, .. } => puntos.len(),
```

En `de_cerca_se_dibuja_la_tinta_entera` cambiar `Orden::Poligono` por `Orden::Tinta`. Quitar `Ajustes` y `poligono` del `use` de `puertas.rs`; las puertas que medían `poligono(&trazo_largo(n), &Ajustes { .. })` pasan a `pixpin_motor2d::tinta::contorno_de_lapiz(&trazo_largo(n), &[], 1.0, Some(Default::default()))` con los mismos topes de tiempo.

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p pixpin-motor2d -- --test-threads=1`
Expected: FAIL (`Orden::Tinta` no existe).

- [ ] **Step 3: Implementación**

`pintado.rs`, en `enum Orden` tras `Poligono`:

```rust
    /// El contorno cerrado de un trazo a mano (E1). Se rellena con curvas
    /// por puntos medios y regla *winding* (`pixpin_render::tinta`), no
    /// como un poligono de rectas: es lo que hace que se vea como en
    /// Excalidraw.
    Tinta {
        contorno: Vec<Punto2>,
        color: ColorRgba,
    },
```

`girar_orden`: añadir `| Orden::Tinta { contorno: puntos, .. }` al primer brazo.

Ramas de `ordenes` (sustituyen las de `Lapiz` y `Resaltador`):

```rust
        Figura::Lapiz { puntos, presiones, opciones } => {
            let contorno = crate::tinta::contorno_de_lapiz(puntos, presiones, e.grosor, *opciones);
            if !contorno.is_empty() {
                salida.push(Orden::Tinta { contorno, color });
            }
        }

        Figura::Resaltador { puntos } => {
            // D45: grueso, translucido y SIN adelgazar. Un resaltador de
            // grosor variable deja el texto medio tapado.
            let contorno = crate::tinta::contorno_de_resaltador(puntos, e.grosor);
            if !contorno.is_empty() {
                salida.push(Orden::Tinta {
                    contorno,
                    color: ColorRgba { a: 0.35 * e.opacidad, ..e.trazo },
                });
            }
        }
```

Quitar `use crate::trazo::{self, Ajustes};`. Sustituir la constante y su comentario:

```rust
/// Por debajo de este tamano en pantalla un trazo de tinta se pinta como
/// una raya.
///
/// Era 48 px y era la economia del lienzo infinito, pero borraba la tinta
/// de todo lo pequeno: letras, tics y firmas salian como rayas de grosor
/// fijo y puntas cortadas (diagnostico de E1). Excalidraw no simplifica
/// nunca. Aqui se queda en 2 px (D115), donde de verdad no se distingue, y
/// la economia de lejos pasa a la rejilla, la capa congelada y la cache de
/// realizaciones de Direct2D.
pub const TINTA_MINIMA_PX: f32 = 2.0;
```

Borrar `crates/pixpin-motor2d/src/trazo.rs` y sus dos líneas en `lib.rs`. `rg -n "trazo::|PuntoTrazo|linea_central|Ajustes \{" crates/pixpin-motor2d apps` debe quedar sin resultados del trazo viejo (el `Ajustes` de `enganche` es otro y se queda).

Consumidores, un brazo nuevo en cada `match`:

`apps/pixpin/src/ventana_editor.rs`, `dibujar_orden`:

```rust
        Orden::Tinta { contorno, color } => p.tinta(&a_tuplas(contorno), a_color(*color)),
```

`apps/pixpin/src/capa.rs`, `pintar_ordenes`:

```rust
            Orden::Tinta { contorno, color: c } => {
                let v: Vec<(f32, f32)> = contorno.iter().map(|q| (q.x, q.y)).collect();
                p.tinta(&v, color(*c));
            }
```

`crates/pixpin-pin/src/ventana.rs:1930`:

```rust
            Orden::Tinta { contorno, color: c } => {
                let v: Vec<(f32, f32)> = contorno.iter().map(mover).collect();
                p.tinta(&v, color(*c));
            }
```

- [ ] **Step 4: Ejecutar pruebas**

Run: `cargo build --workspace --all-targets` y `cargo test --workspace --no-fail-fast -- --test-threads=1`
Expected: PASS, salvo el fallo conocido de instancia única si la app está abierta.

- [ ] **Step 5: Commit**

```bash
git add -A crates apps
git commit -m "El motor entrega tinta y los tres pintores la pintan como Excalidraw; adios al umbral de 48 px"
```

---

### Task 7: Entrada fina — todos los puntos del ratón y del lápiz

**Files:**
- Create: `crates/pixpin-shell/src/puntero.rs`
- Modify: `crates/pixpin-shell/src/lib.rs` (`pub mod puntero;`)
- Modify: `crates/pixpin-shell/Cargo.toml` (features `"Win32_UI_Input_Pointer"`, `"Win32_UI_Controls"`)
- Modify: `crates/pixpin-shell/src/overlay.rs:52-84` (variante), `:113-123` (thread_local), `:127-350` (`pedir_entrada_fina`), `:510-513` (`WM_MOUSEMOVE`), y un brazo `WM_POINTERUPDATE`

**Interfaces:**
- Produces:
  - `pub struct Muestra { x16: i32, y16: i32, presion: Option<u16> }` — `Copy + Eq` (por eso en punto fijo: `EventoOverlay` deriva `Copy, Eq`), con `pub fn nueva(x: f32, y: f32, presion: Option<f32>) -> Muestra`, `pub fn x(&self) -> f32`, `pub fn y(&self) -> f32`, `pub fn presion(&self) -> Option<f32>`. Coordenadas del **escritorio virtual**, en píxeles físicos.
  - `pub fn perdidos_entre(recientes: &[(i32, i32, u32)], anterior: (i32, i32, u32)) -> Vec<(i32, i32)>`
  - `pub fn himetric_a_pixel(him: (i32, i32), dispositivo: (i32, i32, i32, i32), pantalla: (i32, i32, i32, i32)) -> (f32, f32)`
  - `pub struct HistorialRaton` con `pub const fn nuevo() -> Self`, `pub fn olvidar(&mut self)`, `pub fn recuperar(&mut self, x: i32, y: i32, tiempo: u32) -> Vec<Muestra>`
  - `pub fn muestras_de_lapiz(wparam: WPARAM) -> Option<Vec<Muestra>>`
  - `pub fn es_raton_de_lapiz() -> bool`
  - `EventoOverlay::Muestra(Muestra)` y `VentanaOverlay::pedir_entrada_fina(&self)`

Por qué `D106`: **no** se llama a `EnableMouseInPointer`; convertiría el ratón en punteros para todo el proceso y el overlay, los pines y los gestos se reescribirían sin pruebas.

- [ ] **Step 1: Pruebas puras (fallan)**

Al final de `puntero.rs`:

```rust
#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_recuperan_los_puntos_fusionados_del_mas_viejo_al_mas_nuevo() {
        // Como lo da Windows: el mas nuevo primero, y el [0] es el del mensaje.
        let recientes = [(40, 0, 104), (30, 0, 103), (20, 0, 102), (10, 0, 101), (0, 0, 100)];
        let anterior = (10, 0, 101);
        assert_eq!(perdidos_entre(&recientes, anterior), vec![(20, 0), (30, 0)]);
    }

    #[test]
    fn sin_nada_fusionado_no_se_recupera_nada() {
        let recientes = [(40, 0, 104), (30, 0, 103)];
        assert!(perdidos_entre(&recientes, (30, 0, 103)).is_empty());
    }

    #[test]
    fn no_se_cuela_nada_mas_viejo_que_el_ultimo_entregado() {
        // El anterior ya salio del historial (mas de 64 movimientos): se para
        // en el primero con tiempo menor, no se devuelve historia vieja.
        let recientes = [(40, 0, 204), (30, 0, 203), (20, 0, 150), (10, 0, 90)];
        assert_eq!(perdidos_entre(&recientes, (0, 0, 180)), vec![(30, 0)]);
    }

    #[test]
    fn himetric_se_reparte_proporcional_sobre_la_pantalla() {
        let (x, y) = himetric_a_pixel((5000, 2500), (0, 0, 10000, 5000), (100, 50, 2020, 1130));
        assert!((x - 1060.0).abs() < 0.01 && (y - 590.0).abs() < 0.01, "{x},{y}");
    }

    #[test]
    fn un_dispositivo_de_tamano_cero_no_divide_por_cero() {
        let (x, y) = himetric_a_pixel((5, 5), (0, 0, 0, 0), (0, 0, 100, 100));
        assert!(x.is_finite() && y.is_finite());
    }

    #[test]
    fn la_muestra_guarda_subpixel_y_presion_redondeados() {
        let m = Muestra::nueva(10.53, -3.25, Some(0.5));
        assert!((m.x() - 10.5625).abs() < 1e-4, "{}", m.x());
        assert!((m.y() + 3.25).abs() < 1e-4);
        assert_eq!(m.presion(), Some(512.0 / 1024.0));
        assert_eq!(Muestra::nueva(0.0, 0.0, None).presion(), None);
    }
}
```

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p pixpin-shell puntero -- --test-threads=1`
Expected: FAIL de compilación (no existe `puntero`).

- [ ] **Step 3: Implementación**

`puntero.rs` (encima de las pruebas):

```rust
//! Entrada fina para dibujar: todos los puntos del raton y del lapiz (E1).
//!
//! Windows fusiona los `WM_MOUSEMOVE` pendientes en uno: con un raton de
//! 1000 Hz y un fotograma ocupado, la aplicacion ve 40-60 puntos por
//! segundo y un trazo rapido sale como un poligono de pocos lados. Los
//! puntos no se pierden: `GetMouseMovePointsEx` guarda los 64 ultimos. Y el
//! lapiz trae los suyos, con presion y subpixel, en su historial de puntero.
//!
//! Excalidraw no hace nada de esto (se queda con un punto por fotograma);
//! aqui se puede hacer mejor que el original.

use std::cell::Cell;

use windows::Win32::Foundation::{RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GMMP_USE_DISPLAY_POINTS, GetMouseMovePointsEx, MOUSEMOVEPOINT,
};
use windows::Win32::UI::Input::Pointer::{
    GetPointerPenInfoHistory, GetPointerType, POINTER_FLAG_INCONTACT, POINTER_PEN_INFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageExtraInfo, GetPointerDeviceRects, PEN_MASK_PRESSURE, POINTER_INPUT_TYPE, PT_PEN,
};

/// Una muestra del puntero en el escritorio virtual.
///
/// En punto fijo (1/16 px y presion 0-1024) y no en `f32` porque viaja
/// dentro de `EventoOverlay`, que es `Copy + Eq`: un `f32` no es `Eq`.
/// 1/16 de pixel es mas fino que cualquier digitalizador de consumo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Muestra {
    x16: i32,
    y16: i32,
    presion: Option<u16>,
}

impl Muestra {
    pub fn nueva(x: f32, y: f32, presion: Option<f32>) -> Muestra {
        Muestra {
            x16: (x * 16.0).round() as i32,
            y16: (y * 16.0).round() as i32,
            presion: presion.map(|p| (p.clamp(0.0, 1.0) * 1024.0).round() as u16),
        }
    }
    pub fn x(&self) -> f32 {
        self.x16 as f32 / 16.0
    }
    pub fn y(&self) -> f32 {
        self.y16 as f32 / 16.0
    }
    pub fn presion(&self) -> Option<f32> {
        self.presion.map(|p| p as f32 / 1024.0)
    }
}

/// Los puntos que Windows fusiono entre el ultimo entregado y el actual.
///
/// `recientes` viene como lo devuelve `GetMouseMovePointsEx`: el mas nuevo
/// primero, y en `[0]` el punto del propio mensaje, que ya llega por su
/// cuenta y no se repite. Se para al llegar al anterior o a algo mas viejo
/// que el: si el anterior ya salio del historial, no se devuelve historia
/// de antes de pulsar. (El contador de tiempo da la vuelta cada 49 dias:
/// ese mensaje se queda sin recuperar, nada mas.)
pub fn perdidos_entre(recientes: &[(i32, i32, u32)], anterior: (i32, i32, u32)) -> Vec<(i32, i32)> {
    let mut v = Vec::new();
    for &(x, y, t) in recientes.iter().skip(1) {
        if (x, y, t) == anterior || t < anterior.2 {
            break;
        }
        v.push((x, y));
    }
    v.reverse();
    v
}

/// HIMETRIC del digitalizador a pixeles del escritorio, por proporcion
/// entre los dos rectangulos que da `GetPointerDeviceRects`.
pub fn himetric_a_pixel(
    him: (i32, i32),
    dispositivo: (i32, i32, i32, i32),
    pantalla: (i32, i32, i32, i32),
) -> (f32, f32) {
    let (dl, dt, dr, db) = dispositivo;
    let (pl, pt, pr, pb) = pantalla;
    let ancho_d = ((dr - dl) as f32).max(1.0);
    let alto_d = ((db - dt) as f32).max(1.0);
    (
        pl as f32 + (him.0 - dl) as f32 * (pr - pl) as f32 / ancho_d,
        pt as f32 + (him.1 - dt) as f32 * (pb - pt) as f32 / alto_d,
    )
}

/// Con varios monitores, `GMMP_USE_DISPLAY_POINTS` devuelve las
/// coordenadas negativas como valores de 16 bits sin signo.
fn con_signo(c: i32) -> i32 {
    if c > 32767 { c - 65536 } else { c }
}

thread_local! {
    static AVISADO: Cell<bool> = const { Cell::new(false) };
}

fn avisar_una_vez(que: &str) {
    if !AVISADO.with(|a| a.replace(true)) {
        tracing::warn!(que, "sin historial de puntero: se usa solo el punto del mensaje");
    }
}

/// Lo que hace falta recordar entre dos `WM_MOUSEMOVE` de una ventana.
#[derive(Debug, Default)]
pub struct HistorialRaton {
    anterior: Option<(i32, i32, u32)>,
}

impl HistorialRaton {
    pub const fn nuevo() -> Self {
        Self { anterior: None }
    }

    /// Al soltar el boton: el siguiente trazo no recupera puntos de este.
    pub fn olvidar(&mut self) {
        self.anterior = None;
    }

    /// Los puntos fusionados antes de `(x, y)` (escritorio virtual) y
    /// `tiempo` (`GetMessageTime`). El primer movimiento de un trazo no
    /// recupera nada: lo anterior es de antes de pulsar.
    pub fn recuperar(&mut self, x: i32, y: i32, tiempo: u32) -> Vec<Muestra> {
        let Some(anterior) = self.anterior.replace((x, y, tiempo)) else {
            return Vec::new();
        };
        let entrada = MOUSEMOVEPOINT {
            x: x & 0xFFFF,
            y: y & 0xFFFF,
            time: tiempo,
            dwExtraInfo: 0,
        };
        let mut buf = [MOUSEMOVEPOINT::default(); 64];
        // SAFETY: estructura de entrada local y bufer local de 64, el maximo
        // que documenta la API; el tamano pasado es el de la estructura.
        let n = unsafe {
            GetMouseMovePointsEx(
                std::mem::size_of::<MOUSEMOVEPOINT>() as u32,
                &entrada,
                &mut buf,
                GMMP_USE_DISPLAY_POINTS,
            )
        };
        if n <= 0 {
            avisar_una_vez("GetMouseMovePointsEx");
            return Vec::new();
        }
        let recientes: Vec<(i32, i32, u32)> = buf[..n as usize]
            .iter()
            .map(|m| (con_signo(m.x), con_signo(m.y), m.time))
            .collect();
        perdidos_entre(&recientes, anterior)
            .into_iter()
            .map(|(x, y)| Muestra::nueva(x as f32, y as f32, None))
            .collect()
    }
}

/// Si el `WM_MOUSEMOVE` que se esta atendiendo lo sintetizo Windows a partir
/// de un lapiz o del tacto (firma documentada `0xFF515700`). Sus muestras
/// buenas llegan por `WM_POINTERUPDATE`; este se descarta para no mezclar
/// puntos enteros sin presion en medio del trazo.
pub fn es_raton_de_lapiz() -> bool {
    // SAFETY: lee un valor del mensaje actual del hilo; sin precondiciones.
    let extra = unsafe { GetMessageExtraInfo() }.0 as usize;
    extra & 0xFFFF_FF00 == 0xFF51_5700
}

/// Las muestras de un `WM_POINTERUPDATE` de lapiz en contacto, de la mas
/// vieja a la mas nueva. `None` si no es un lapiz o si falla la API.
pub fn muestras_de_lapiz(wparam: WPARAM) -> Option<Vec<Muestra>> {
    let id = (wparam.0 & 0xFFFF) as u32;
    let mut tipo = POINTER_INPUT_TYPE::default();
    // SAFETY: id del mensaje actual y salida local.
    unsafe { GetPointerType(id, &mut tipo).ok()? };
    if tipo != PT_PEN {
        return None;
    }
    let mut cuantos = 0u32;
    // SAFETY: primera llamada solo para saber cuantas entradas hay.
    if unsafe { GetPointerPenInfoHistory(id, &mut cuantos, None) }.is_err() {
        avisar_una_vez("GetPointerPenInfoHistory");
        return None;
    }
    let mut buf = vec![POINTER_PEN_INFO::default(); cuantos.max(1) as usize];
    // SAFETY: bufer local del tamano pedido; `cuantos` sale con lo escrito.
    unsafe { GetPointerPenInfoHistory(id, &mut cuantos, Some(buf.as_mut_ptr())).ok()? };
    buf.truncate(cuantos as usize);
    let dispositivo = buf.first()?.pointerInfo.sourceDevice;
    let (mut rd, mut rp) = (RECT::default(), RECT::default());
    // SAFETY: dispositivo del propio mensaje; salidas locales.
    let con_rects = unsafe { GetPointerDeviceRects(dispositivo, &mut rd, &mut rp) }.is_ok();
    Some(
        buf.iter()
            .rev()
            .filter(|i| i.pointerInfo.pointerFlags.contains(POINTER_FLAG_INCONTACT))
            .map(|i| {
                let (x, y) = if con_rects {
                    himetric_a_pixel(
                        (i.pointerInfo.ptHimetricLocation.x, i.pointerInfo.ptHimetricLocation.y),
                        (rd.left, rd.top, rd.right, rd.bottom),
                        (rp.left, rp.top, rp.right, rp.bottom),
                    )
                } else {
                    (i.pointerInfo.ptPixelLocation.x as f32, i.pointerInfo.ptPixelLocation.y as f32)
                };
                let presion = i
                    .penMask
                    .contains(PEN_MASK_PRESSURE)
                    .then(|| i.pressure as f32 / 1024.0);
                Muestra::nueva(x, y, presion)
            })
            .collect(),
    )
}
```

> Las rutas de `use` de `windows` 0.62 para `GetPointerDeviceRects`, `PEN_MASK_PRESSURE`, `PT_PEN` y `POINTER_FLAG_INCONTACT` pueden estar en `Win32::UI::Input::Pointer` o `Win32::UI::WindowsAndMessaging`/`Controls`: localizarlas con `rg -n "pub unsafe fn GetPointerDeviceRects|pub const PEN_MASK_PRESSURE|pub const PT_PEN" "$USERPROFILE/.cargo/registry/src"` y corregir solo los `use`. Si `GetMouseMovePointsEx` pide `*const MOUSEMOVEPOINT` en vez de `&`, pasar `&entrada as *const _`. Si `pixpin-shell` no depende aún de `tracing`, añadirlo a su `Cargo.toml` con la misma versión que ya fija `Cargo.lock` para `apps/pixpin`.

`overlay.rs`:

1. Variante al final de `EventoOverlay`:

```rust
    /// Un punto del trazo que no es el del `WM_MOUSEMOVE`: uno que Windows
    /// fusiono, o uno del lapiz con presion. Llega ANTES del `RatonMovido`
    /// al que precede, y solo a ventanas que llamaron a
    /// `pedir_entrada_fina`. Coordenadas del escritorio virtual.
    Muestra(crate::puntero::Muestra),
```

2. En el `thread_local!` de la línea 113:

```rust
    /// Ventanas que quieren todos los puntos, con su historial de raton.
    static ENTRADA_FINA: RefCell<Vec<(HWND, crate::puntero::HistorialRaton)>> =
        const { RefCell::new(Vec::new()) };
```

3. En `impl VentanaOverlay`:

```rust
    /// Pide todos los puntos del trazo (`EventoOverlay::Muestra`). Solo para
    /// las ventanas de dibujo: el resto sigue recibiendo un movimiento por
    /// mensaje, que es lo que esperan.
    pub fn pedir_entrada_fina(&self) {
        ENTRADA_FINA.with(|e| {
            let mut e = e.borrow_mut();
            if !e.iter().any(|(h, _)| *h == self.hwnd) {
                e.push((self.hwnd, crate::puntero::HistorialRaton::nuevo()));
            }
        });
    }
```

(Si el campo del `HWND` en `VentanaOverlay` no se llama `hwnd`, usar `self.handle()`.) En el `Drop` de `VentanaOverlay` (o donde se destruye la ventana), quitar su entrada con `ENTRADA_FINA.with(|e| e.borrow_mut().retain(|(h, _)| *h != hwnd))`.

4. `WM_MOUSEMOVE` pasa a:

```rust
        WM_MOUSEMOVE => {
            let p = punto(lparam);
            const MK_LBUTTON: usize = 0x0001;
            let boton = wparam.0 & MK_LBUTTON != 0;
            let fina = ENTRADA_FINA.with(|e| e.borrow().iter().any(|(h, _)| *h == hwnd));
            if fina {
                if boton && crate::puntero::es_raton_de_lapiz() {
                    // Con el lapiz apoyado, sus muestras llegan por
                    // WM_POINTERUPDATE; este movimiento sintetizado se tira.
                    return LRESULT(0);
                }
                // SAFETY: tiempo del mensaje que se esta atendiendo.
                let tiempo = unsafe { GetMessageTime() } as u32;
                let perdidos = ENTRADA_FINA.with(|e| {
                    let mut e = e.borrow_mut();
                    let Some((_, h)) = e.iter_mut().find(|(w, _)| *w == hwnd) else {
                        return Vec::new();
                    };
                    if boton {
                        h.recuperar(p.x, p.y, tiempo)
                    } else {
                        h.olvidar();
                        Vec::new()
                    }
                });
                for m in perdidos {
                    encolar(EventoOverlay::Muestra(m));
                }
            }
            encolar(EventoOverlay::RatonMovido(p));
            LRESULT(0)
        }
```

5. Brazo nuevo, antes del `_ =>` final:

```rust
        WM_POINTERUPDATE
            if ENTRADA_FINA.with(|e| e.borrow().iter().any(|(h, _)| *h == hwnd)) =>
        {
            if let Some(muestras) = crate::puntero::muestras_de_lapiz(wparam) {
                for m in muestras {
                    encolar(EventoOverlay::Muestra(m));
                }
            }
            // Se deja pasar a DefWindowProc: asi Windows sigue sintetizando
            // los clics del lapiz, que es como se pulsa y se suelta.
            // SAFETY: reenvio estandar del mensaje recibido.
            unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
        }
```

Añadir a los `use` de `overlay.rs`: `GetMessageTime`, `WM_POINTERUPDATE` (de `Win32::UI::WindowsAndMessaging`).

6. Los `match` sobre `EventoOverlay` que no compilen por la variante nueva (`apps/pixpin/src/{capa,overlay,editor,ventana_ajustes}.rs`, `ventana_editor.rs`): si tienen `_ =>` no hay que tocar nada; si son exhaustivos, añadir `EventoOverlay::Muestra(_) => {}` (o `=> true` donde el brazo devuelve `bool`). Las Tareas 9 y 11 le dan uso en el editor y en la capa.

- [ ] **Step 4: Ejecutar pruebas**

Run: `cargo build --workspace --all-targets` y `cargo test -p pixpin-shell -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates/pixpin-shell apps
git commit -m "Entrada fina: los puntos que Windows fusiona y el historial del lapiz con su presion"
```

---

### Task 8: El gesto lleva la presión y la pluma

**Files:**
- Modify: `crates/pixpin-motor2d/src/gesto.rs:120-129` (`EventoGesto`), `:197-223` (`Gesto` y su `Default`), `:430-480` (`nuevo_elemento`), `:~643` (donde `Pulsar` crea el elemento y pasa a `Estado::Dibujando`), `:672-714` (`Mover` en `Estado::Dibujando`)
- Modify: todos los constructores `EventoGesto::Pulsar { .. }` / `EventoGesto::Mover { .. }` (22 sitios: `gesto.rs`, `tests/asignaciones.rs`, `apps/pixpin/src/ventana_editor.rs`)
- Modify: `crates/pixpin-motor2d/tests/asignaciones.rs` (caso con presión)

**Interfaces:**
- Consumes: `tinta::{OpcionesTinta, Variabilidad, STREAMLINE_RATON, STREAMLINE_LAPIZ, GROSOR_MEDIO}` (Tarea 4).
- Produces:
  - `EventoGesto::Pulsar { p: Punto2, shift: bool, alt: bool, presion: Option<f32> }`
  - `EventoGesto::Mover { p: Punto2, shift: bool, alt: bool, presion: Option<f32> }`
  - `Gesto` gana `pub grosor_tinta: f32` (por defecto `GROSOR_MEDIO`) y `pub variabilidad: Variabilidad` (por defecto `Variable`)
  - `pub fn trazo_en_curso(&self) -> Option<u64>` — el id del lápiz o resaltador que se está dibujando

- [ ] **Step 1: Pruebas (fallan)**

En `gesto.rs`, `mod pruebas` (usar los ayudantes de escena que ya usan las pruebas de la línea ~1117):

```rust
fn pulsar(g: &mut Gesto, e: &mut Escena, x: f32, y: f32, presion: Option<f32>) {
    g.evento(EventoGesto::Pulsar { p: Punto2::nuevo(x, y), shift: false, alt: false, presion }, e, 1.0);
}
fn mover(g: &mut Gesto, e: &mut Escena, x: f32, y: f32, presion: Option<f32>) {
    g.evento(EventoGesto::Mover { p: Punto2::nuevo(x, y), shift: false, alt: false, presion }, e, 1.0);
}
fn lapiz_de(e: &Escena) -> (Vec<Punto2>, Vec<f32>, Option<crate::tinta::OpcionesTinta>, f32) {
    let el = e.visibles().last().expect("hay un trazo");
    let Figura::Lapiz { puntos, presiones, opciones } = &el.figura else { panic!("no es lapiz") };
    (puntos.clone(), presiones.clone(), *opciones, el.grosor)
}

#[test]
fn con_raton_el_trazo_nace_con_la_pluma_elegida_y_sin_presiones() {
    let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
    g.herramienta = Herramienta::Lapiz;
    g.variabilidad = crate::tinta::Variabilidad::Constante;
    g.grosor_tinta = crate::tinta::GROSOR_GRUESO;
    pulsar(&mut g, &mut e, 0.0, 0.0, None);
    mover(&mut g, &mut e, 5.0, 0.0, None);
    let (_, presiones, opciones, grosor) = lapiz_de(&e);
    assert!(presiones.is_empty());
    assert_eq!(grosor, crate::tinta::GROSOR_GRUESO);
    let o = opciones.expect("un trazo nuevo nunca es legado");
    assert_eq!(o.variabilidad, crate::tinta::Variabilidad::Constante);
    assert_eq!(o.streamline, crate::tinta::STREAMLINE_RATON);
}

#[test]
fn la_primera_presion_real_rellena_hacia_atras_y_afina_el_suavizado() {
    // El boton lo pulsa un mensaje de raton sin presion; la presion llega
    // con la primera muestra del lapiz.
    let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
    g.herramienta = Herramienta::Lapiz;
    pulsar(&mut g, &mut e, 0.0, 0.0, None);
    mover(&mut g, &mut e, 5.0, 0.0, Some(0.7));
    mover(&mut g, &mut e, 9.0, 0.0, Some(0.9));
    let (puntos, presiones, opciones, _) = lapiz_de(&e);
    assert_eq!(puntos.len(), presiones.len());
    assert_eq!(presiones, vec![0.7, 0.7, 0.9]);
    assert_eq!(opciones.unwrap().streamline, crate::tinta::STREAMLINE_LAPIZ);
}

#[test]
fn un_punto_repetido_exacto_no_se_guarda() {
    let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
    g.herramienta = Herramienta::Lapiz;
    pulsar(&mut g, &mut e, 0.0, 0.0, None);
    mover(&mut g, &mut e, 5.0, 0.0, None);
    mover(&mut g, &mut e, 5.0, 0.0, None);
    assert_eq!(lapiz_de(&e).0.len(), 2);
}

#[test]
fn trazo_en_curso_solo_existe_mientras_se_dibuja_a_mano() {
    let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
    g.herramienta = Herramienta::Lapiz;
    assert_eq!(g.trazo_en_curso(), None);
    pulsar(&mut g, &mut e, 0.0, 0.0, None);
    assert!(g.trazo_en_curso().is_some());
    g.evento(EventoGesto::Soltar { p: Punto2::nuevo(1.0, 0.0) }, &mut e, 1.0);
    assert_eq!(g.trazo_en_curso(), None);
}
```

En `tests/asignaciones.rs`, junto al caso existente de mover dibujando, uno igual pero con `presion: Some(0.5)` en cada `Mover`, con la misma exigencia de 0 asignaciones.

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p pixpin-motor2d -- --test-threads=1`
Expected: FAIL de compilación (`presion` no es campo de `EventoGesto::Pulsar`).

- [ ] **Step 3: Implementación**

`EventoGesto`:

```rust
pub enum EventoGesto {
    /// `presion` es la del lapiz (0-1) o `None` con raton.
    Pulsar { p: Punto2, shift: bool, alt: bool, presion: Option<f32> },
    Mover { p: Punto2, shift: bool, alt: bool, presion: Option<f32> },
    Soltar { p: Punto2 },
    Escape,
    Suprimir,
    Deshacer,
    Rehacer,
    SeleccionarTodo,
}
```

`Gesto`: dos campos públicos nuevos, y en su `Default`:

```rust
    /// El `strokeWidth` de Excalidraw para el lapiz: 0,5 / 1 / 2.
    pub grosor_tinta: f32,
    /// Pluma variable o constante para los trazos nuevos.
    pub variabilidad: crate::tinta::Variabilidad,
```

```rust
            grosor_tinta: crate::tinta::GROSOR_MEDIO,
            variabilidad: crate::tinta::Variabilidad::Variable,
```

`nuevo_elemento`: la rama del lápiz y el grosor:

```rust
            Herramienta::Lapiz => Figura::Lapiz {
                puntos: reservados(),
                // Reservadas igual que los puntos: la primera presion real no
                // puede pedir memoria en el camino caliente.
                presiones: Vec::with_capacity(PUNTOS_RESERVADOS),
                opciones: Some(crate::tinta::OpcionesTinta {
                    variabilidad: self.variabilidad,
                    streamline: crate::tinta::STREAMLINE_RATON,
                }),
            },
```

```rust
            grosor: if self.herramienta == Herramienta::Lapiz {
                self.grosor_tinta
            } else {
                3.0
            },
```

Una función libre en `gesto.rs` que hace el trabajo de un punto de lápiz, compartida por `Pulsar` y `Mover`:

```rust
/// Anade un punto a un trazo de lapiz respetando sus presiones.
///
/// - Solo se descartan duplicados exactos (D109), como Excalidraw.
/// - La primera presion real rellena hacia atras los puntos que llegaron sin
///   ella (el clic lo entrega un mensaje de raton, la presion llega despues)
///   y cambia el suavizado al del lapiz.
/// - Un punto sin presion en un trazo que ya la tiene repite la ultima, para
///   que las dos listas no se desalineen.
fn anadir_a_lapiz(
    puntos: &mut Vec<Punto2>,
    presiones: &mut Vec<f32>,
    opciones: &mut Option<crate::tinta::OpcionesTinta>,
    p: Punto2,
    presion: Option<f32>,
) {
    if puntos.last() == Some(&p) {
        return;
    }
    puntos.push(p);
    match presion {
        Some(pr) => {
            if presiones.is_empty() {
                presiones.resize(puntos.len() - 1, pr);
                if let Some(o) = opciones {
                    o.streamline = crate::tinta::STREAMLINE_LAPIZ;
                }
            }
            presiones.push(pr);
        }
        None => {
            if let Some(&ultima) = presiones.last() {
                presiones.push(ultima);
            }
        }
    }
}
```

En `Estado::Dibujando` de `Mover`, separar el brazo compartido:

```rust
                        Figura::Lapiz { puntos, presiones, opciones } => {
                            anadir_a_lapiz(puntos, presiones, opciones, p, presion);
                        }
                        Figura::Resaltador { puntos } => {
                            if puntos.last() != Some(&p) {
                                puntos.push(p);
                            }
                        }
```

(`presion` sale del patrón `EventoGesto::Mover { p, shift, alt, presion }` donde se destruye el evento; seguir el mismo camino por el que hoy llega `p`.)

Donde `Pulsar` crea el elemento y pone `Estado::Dibujando { id }`, justo antes de `escena.anadir(e)`:

```rust
            if let (Some(pr), Figura::Lapiz { presiones, opciones, .. }) = (presion, &mut e.figura) {
                presiones.push(pr);
                if let Some(o) = opciones {
                    o.streamline = crate::tinta::STREAMLINE_LAPIZ;
                }
            }
```

`trazo_en_curso`, junto a `en_reposo`:

```rust
    /// El trazo a mano que se esta dibujando ahora. La ventana lo excluye de
    /// la capa congelada (D120): es lo unico que cambia en cada fotograma.
    pub fn trazo_en_curso(&self) -> Option<u64> {
        match self.estado {
            Estado::Dibujando { id }
                if matches!(self.herramienta, Herramienta::Lapiz | Herramienta::Resaltador) =>
            {
                Some(id)
            }
            _ => None,
        }
    }
```

Resto de constructores de `EventoGesto::Pulsar`/`Mover` que no compilen: añadir `presion: None`. Los patrones que destruyen con `..` no cambian.

- [ ] **Step 4: Ejecutar pruebas**

Run: `cargo test -p pixpin-motor2d -- --test-threads=1` y `cargo build --workspace --all-targets`
Expected: PASS, incluidas las 4+1 de `asignaciones.rs` con 0 asignaciones.

- [ ] **Step 5: Commit**

```bash
git add -A crates/pixpin-motor2d apps/pixpin/src/ventana_editor.rs
git commit -m "El gesto guarda la presion del lapiz y nace con la pluma y el grosor elegidos"
```

---

### Task 9: El editor — muestras, origen, bucle sin `sleep`, vsync y escena congelada

**Files:**
- Modify: `apps/pixpin/src/ventana_editor.rs` (`a_evento` :87, `tecla_a_herramienta` :142, `abrir` :206-420, `pintar` :470-579, pruebas :1082-1129)
- Modify: `apps/pixpin/src/main.rs:717` (pasa el nivel)
- Modify: `crates/pixpin-shell/src/overlay.rs` (`esperar_eventos`)
- Modify: `crates/pixpin-render/src/superficie.rs:229-235` (`presentar_sincronizado`)
- Modify: `crates/pixpin-render/Cargo.toml` (feature `"Win32_Foundation"` ya está; comprobar `DXGI_PRESENT_PARAMETERS` en `Win32_Graphics_Dxgi`)

**Interfaces:**
- Consumes: `EventoOverlay::Muestra`, `Muestra::{x,y,presion}`, `VentanaOverlay::pedir_entrada_fina` (Tarea 7); `EventoGesto::{Pulsar,Mover}` con `presion`, `Gesto::{grosor_tinta, variabilidad, trazo_en_curso}` (Tarea 8).
- Produces:
  - `pub fn a_evento(ev: &EventoOverlay, camara: &Camara, origen: Punto) -> Option<EventoGesto>` (ahora resta el origen de la ventana y traduce `Muestra`)
  - `fn tecla_a_pluma(c: char) -> Option<CambioPluma>` con `enum CambioPluma { Grosor(f32), AlternarVariabilidad }`
  - `fn excluidos_de(gesto: &Gesto) -> Vec<u64>`
  - `pub fn abrir(escena: Escena, ajustes_iman: enganche::Ajustes, nivel: pixpin_nivel::Nivel) -> Result<Escena>`
  - `pixpin_shell::overlay::esperar_eventos(tope_ms: Option<u32>)`
  - `Superficie::presentar_sincronizado(&self, sucio: Option<(i32, i32, i32, i32)>) -> Result<(), ErrorRender>`

- [ ] **Step 1: Pruebas puras (fallan)**

En `ventana_editor.rs`, `mod pruebas` (las llamadas existentes a `a_evento(&ev, &camara)` pasan a `a_evento(&ev, &camara, Punto { x: 0, y: 0 })`):

```rust
#[test]
fn el_origen_de_la_ventana_se_resta_antes_de_pasar_al_mundo() {
    // Con la barra de tareas arriba o a la izquierda, el area de trabajo no
    // empieza en (0,0) y la tinta salia desplazada del cursor.
    let camara = Camara::nueva();
    let ev = EventoOverlay::BotonPulsado(Punto { x: 110, y: 60 });
    let Some(EventoGesto::Pulsar { p, presion, .. }) = a_evento(&ev, &camara, Punto { x: 100, y: 50 }) else {
        panic!("tendria que ser Pulsar")
    };
    assert_eq!((p.x, p.y), (10.0, 10.0));
    assert_eq!(presion, None);
}

#[test]
fn una_muestra_del_lapiz_llega_al_gesto_con_su_presion_y_su_subpixel() {
    let camara = Camara::nueva();
    let m = pixpin_shell::puntero::Muestra::nueva(12.5, 7.25, Some(0.75));
    let Some(EventoGesto::Mover { p, presion, .. }) =
        a_evento(&EventoOverlay::Muestra(m), &camara, Punto { x: 0, y: 0 })
    else {
        panic!("tendria que ser Mover")
    };
    assert_eq!((p.x, p.y), (12.5, 7.25));
    assert_eq!(presion, Some(0.75));
}

#[test]
fn las_teclas_uno_dos_tres_eligen_grosor_y_v_alterna_la_pluma() {
    assert_eq!(tecla_a_pluma('1'), Some(CambioPluma::Grosor(pixpin_motor2d::tinta::GROSOR_FINO)));
    assert_eq!(tecla_a_pluma('2'), Some(CambioPluma::Grosor(pixpin_motor2d::tinta::GROSOR_MEDIO)));
    assert_eq!(tecla_a_pluma('3'), Some(CambioPluma::Grosor(pixpin_motor2d::tinta::GROSOR_GRUESO)));
    assert_eq!(tecla_a_pluma('v'), Some(CambioPluma::AlternarVariabilidad));
    assert_eq!(tecla_a_pluma('L'), None, "L sigue siendo el lapiz");
    // Ninguna de las nuevas pisa una herramienta.
    for c in ['1', '2', '3', 'V'] {
        assert_eq!(tecla_a_herramienta(c), None, "{c}");
    }
}

#[test]
fn dibujando_se_excluye_el_trazo_y_moviendo_la_seleccion() {
    let mut escena = Escena::nueva();
    let mut g = Gesto::nuevo();
    g.herramienta = Herramienta::Lapiz;
    assert!(excluidos_de(&g).is_empty());
    g.evento(
        EventoGesto::Pulsar { p: Punto2::nuevo(1.0, 1.0), shift: false, alt: false, presion: None },
        &mut escena,
        1.0,
    );
    assert_eq!(excluidos_de(&g), vec![g.trazo_en_curso().unwrap()]);
}
```

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p pixpin ventana_editor -- --test-threads=1`
Expected: FAIL de compilación.

- [ ] **Step 3: Implementación**

`a_evento` con origen y muestras:

```rust
pub fn a_evento(ev: &EventoOverlay, camara: &Camara, origen: Punto) -> Option<EventoGesto> {
    // Los mensajes traen coordenadas del escritorio virtual; el lienzo
    // empieza en la esquina de la ventana. Restar el origen es lo que hace
    // que la tinta caiga bajo el cursor aunque el area de trabajo no empiece
    // en (0,0).
    let (ox, oy) = (origen.x as f32, origen.y as f32);
    let al_mundo = |x: f32, y: f32| camara.a_mundo(Punto2::nuevo(x - ox, y - oy));
    let entero = |p: &Punto| al_mundo(p.x as f32, p.y as f32);
    match ev {
        EventoOverlay::BotonPulsado(p) => Some(EventoGesto::Pulsar {
            p: entero(p),
            shift: false,
            alt: false,
            presion: None,
        }),
        EventoOverlay::RatonMovido(p) => Some(EventoGesto::Mover {
            p: entero(p),
            shift: false,
            alt: false,
            presion: None,
        }),
        EventoOverlay::Muestra(m) => Some(EventoGesto::Mover {
            p: al_mundo(m.x(), m.y()),
            shift: false,
            alt: false,
            presion: m.presion(),
        }),
        EventoOverlay::BotonSoltado(p) => Some(EventoGesto::Soltar { p: entero(p) }),
        // ... resto sin cambios (teclas) ...
        _ => None,
    }
}
```

Teclas de pluma, junto a `tecla_a_herramienta`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum CambioPluma {
    Grosor(f32),
    AlternarVariabilidad,
}

/// Las plumas de Excalidraw sin interfaz nueva (la barra es E5): 1, 2 y 3
/// son sus tres grosores; V alterna variable y constante. Ninguna choca con
/// `tecla_a_herramienta`, y la prueba lo vigila.
fn tecla_a_pluma(c: char) -> Option<CambioPluma> {
    use pixpin_motor2d::tinta::{GROSOR_FINO, GROSOR_GRUESO, GROSOR_MEDIO};
    match c.to_ascii_uppercase() {
        '1' => Some(CambioPluma::Grosor(GROSOR_FINO)),
        '2' => Some(CambioPluma::Grosor(GROSOR_MEDIO)),
        '3' => Some(CambioPluma::Grosor(GROSOR_GRUESO)),
        'V' => Some(CambioPluma::AlternarVariabilidad),
        _ => None,
    }
}
```

y en el bucle, junto al tratamiento de `EventoOverlay::Caracter`:

```rust
            if let EventoOverlay::Caracter(c) = ev {
                if let Some(cambio) = tecla_a_pluma(c) {
                    match cambio {
                        CambioPluma::Grosor(g) => gesto.grosor_tinta = g,
                        CambioPluma::AlternarVariabilidad => {
                            use pixpin_motor2d::tinta::Variabilidad;
                            gesto.variabilidad = match gesto.variabilidad {
                                Variabilidad::Variable => Variabilidad::Constante,
                                Variabilidad::Constante => Variabilidad::Variable,
                            };
                        }
                    }
                    tracing::info!(grosor = gesto.grosor_tinta, variabilidad = ?gesto.variabilidad, "pluma del editor");
                    continue;
                }
            }
```

Excluidos, función libre:

```rust
/// Lo que NO entra en la capa congelada: lo que cambia en cada fotograma.
/// Una sola funcion para `abrir` y `pintar`: si cada una calculara su lista,
/// la capa se daria por invalida en cada fotograma (la `Estampa` no casaria)
/// o, peor, valdria sin contener lo que hay que pintar encima.
fn excluidos_de(gesto: &Gesto) -> Vec<u64> {
    match gesto.trazo_en_curso() {
        Some(id) => vec![id],
        None => gesto.seleccion.ids().to_vec(),
    }
}
```

`abrir` gana `nivel: pixpin_nivel::Nivel` y en `main.rs:717` se llama `ventana_editor::abrir(pixpin_motor2d::Escena::nueva(), config.enganche, decision.nivel)` (misma variable que ya usan las líneas 782 y 1014). Tras `ventana.enfocar();`: `ventana.pedir_entrada_fina();`. `nivel` se guarda para la Tarea 10 (`let _ = nivel;` hasta entonces no hace falta: la Tarea 10 lo usa; si clippy se queja entre tareas, nombrarlo `_nivel`).

Dentro del bucle, cambios:

1. `a_evento(&ev, &camara)` → `a_evento(&ev, &camara, Punto { x: area.x, y: area.y })`.
2. Condición de la capa congelada:

```rust
                // La capa congelada vive mientras algo cambia en cada
                // fotograma: mover, escalar o girar la seleccion (como antes)
                // y, desde E1, dibujar a mano (D120). Lo excluido se repinta
                // encima; lo demas se copia de la capa.
                let excluidos = excluidos_de(&gesto);
                let activo_ahora = !gesto.en_reposo() && !excluidos.is_empty();
```

y en el bloque `if activo_ahora && en_reposo_antes`, usar esa `excluidos` (quitar la línea `let excluidos = gesto.seleccion.ids().to_vec();`).

3. Zona sucia acumulada. Antes del `'bucle: loop`: `let mut hay_que_pintar = false; let mut sucio: Option<(f32, f32, f32, f32)> = None; let mut todo_sucio = false;`. En el `match r.region`:

```rust
                match r.region {
                    Region::Nada => {}
                    Region::Caja(x0, y0, x1, y1) => {
                        let a = camara.a_pantalla(Punto2::nuevo(x0, y0));
                        let b = camara.a_pantalla(Punto2::nuevo(x1, y1));
                        let caja = (a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
                        sucio = Some(match sucio {
                            None => caja,
                            Some(s) => (s.0.min(caja.0), s.1.min(caja.1), s.2.max(caja.2), s.3.max(caja.3)),
                        });
                        ventana.invalidar();
                    }
                    Region::Todo => {
                        todo_sucio = true;
                        ventana.invalidar();
                    }
                }
```

Cualquier otro `ventana.invalidar()` del bucle (botones, teclas, calibrar) va acompañado de `todo_sucio = true;`.

4. `EventoOverlay::Pintar` ya no pinta: marca `hay_que_pintar = true;`. Tras vaciar la cola de eventos, y **en lugar de** `std::thread::sleep(..)`:

```rust
        // Un solo fotograma por vuelta, despues de haber pasado TODOS los
        // puntos al gesto: pintar a mitad de la cola era lo que hacia perder
        // puntos. Presentar con vsync bloquea hasta el refresco; mientras,
        // Windows guarda los movimientos y la Tarea 7 los recupera.
        if hay_que_pintar {
            rejilla.sincronizar(&escena);
            let zona = if todo_sucio || !capa.lista() {
                None
            } else {
                sucio.map(|(x0, y0, x1, y1)| {
                    (x0.floor() as i32 - 2, y0.floor() as i32 - 2, x1.ceil() as i32 + 2, y1.ceil() as i32 + 2)
                })
            };
            pintar(
                &mut motor, &superficie, &escena, &camara, &gesto, &mut cache, &rejilla, &capa,
                &caja, monitor.escala_por_cien, ancho_px, alto_px, zona, |_| {},
            );
            hay_que_pintar = false;
            sucio = None;
            todo_sucio = false;
        }
        // Dormir hasta que llegue algo. Sin `sleep` fijo: con el `sleep` de
        // 5 ms (15,6 ms reales sin `timeBeginPeriod`) el bucle perdia la
        // mitad de los puntos de un trazo rapido, y en reposo no gana nada.
        pixpin_shell::overlay::esperar_eventos(None);
```

`pintar` gana el parámetro `zona: Option<(i32, i32, i32, i32)>` antes de `encima`. Dentro, `ahora.excluidos` pasa a `excluidos_de(gesto)`, el filtro `if capa_vale && !gesto.seleccion.contiene(id)` pasa a `if capa_vale && !ahora.excluidos.contains(&id)`, y la última línea `let _ = superficie.presentar();` pasa a `let _ = superficie.presentar_sincronizado(zona);`. `pedir_medida` llama a `pintar(..., None, ...)`.

`overlay.rs`:

```rust
/// Duerme el hilo hasta que llegue un mensaje o pase `tope_ms`.
///
/// Es el sustituto de un `sleep` fijo en los bucles que bombean a mano:
/// en reposo cuesta 0 % de CPU y, en cuanto llega un movimiento, vuelve sin
/// esperar a que termine ningun turno. `MWMO_INPUTAVAILABLE` hace que vuelva
/// tambien si ya habia mensajes en la cola que `bombear_pendientes` dejo sin
/// sacar (tope de 64 por vuelta).
pub fn esperar_eventos(tope_ms: Option<u32>) {
    use windows::Win32::System::Threading::INFINITE;
    use windows::Win32::UI::WindowsAndMessaging::{
        MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, QS_ALLINPUT,
    };
    // SAFETY: sin handles; solo espera a la cola de mensajes del hilo.
    unsafe {
        MsgWaitForMultipleObjectsEx(None, tope_ms.unwrap_or(INFINITE), QS_ALLINPUT, MWMO_INPUTAVAILABLE);
    }
}
```

`superficie.rs`:

```rust
    /// Presenta sincronizado con el refresco y, si se sabe, diciendo que
    /// rectangulo cambio (D125). Solo lo usan las ventanas de dibujo: el pin
    /// sigue con `presentar`, que no bloquea su hilo.
    ///
    /// La zona sucia no ahorra pintar (el fotograma se pinta entero sobre la
    /// capa congelada), ahorra componer: DWM solo recompone ese trozo.
    pub fn presentar_sincronizado(
        &self,
        sucio: Option<(i32, i32, i32, i32)>,
    ) -> Result<(), ErrorRender> {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Dxgi::{DXGI_PRESENT, DXGI_PRESENT_PARAMETERS};
        let (ancho, alto) = self.asignado.get();
        let mut rect = sucio.map(|(l, t, r, b)| RECT {
            left: l.clamp(0, ancho as i32),
            top: t.clamp(0, alto as i32),
            right: r.clamp(0, ancho as i32),
            bottom: b.clamp(0, alto as i32),
        });
        // Un rectangulo vacio tras recortar no es "nada cambio", es "no se
        // sabe": se presenta entero.
        if rect.is_some_and(|r| r.right <= r.left || r.bottom <= r.top) {
            rect = None;
        }
        let parametros = DXGI_PRESENT_PARAMETERS {
            DirtyRectsCount: u32::from(rect.is_some()),
            pDirtyRects: rect.as_mut().map_or(std::ptr::null_mut(), |r| r as *mut RECT),
            pScrollRect: std::ptr::null_mut(),
            pScrollOffset: std::ptr::null_mut(),
        };
        // SAFETY: `parametros` y el rectangulo viven hasta el final de la
        // llamada; la swapchain es de modelo flip, que admite zonas sucias.
        unsafe { self.swapchain.Present1(1, DXGI_PRESENT(0), &parametros).ok()? };
        Ok(())
    }
```

- [ ] **Step 4: Ejecutar pruebas**

Run: `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo test --workspace --no-fail-fast -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A apps crates/pixpin-shell crates/pixpin-render
git commit -m "El editor ya no duerme 5 ms: pasa todos los puntos, pinta un fotograma con vsync y congela la escena al dibujar"
```

---

### Task 10: Caché de realizaciones y zoom con retardo

**Files:**
- Modify: `crates/pixpin-render/src/tinta.rs` (`CacheTinta`, `retardo_nitido`)
- Modify: `crates/pixpin-render/src/lienzo.rs` (`Pintor::tinta_cacheada`)
- Modify: `crates/pixpin-render/src/lib.rs` (`pub use tinta::{CacheTinta, retardo_nitido};`)
- Modify: `crates/pixpin-render/Cargo.toml` (`pixpin-nivel` como dependencia: L0, permitido)
- Modify: `apps/pixpin/src/ventana_editor.rs` (`abrir`, `pintar`, `dibujar_orden`)

**Interfaces:**
- Consumes: `Pintor::geometria_tinta` (Tarea 5), `pintar(.., zona, ..)` y `nivel` en `abrir` (Tarea 9).
- Produces:
  - `pub struct CacheTinta` con `pub fn nueva() -> Self`, `pub fn vaciar(&mut self)`, `pub fn escala(&self) -> f32`, `pub fn fijar_escala(&mut self, escala: f32)`, `pub fn cuantas(&self) -> usize`
  - `pub fn retardo_nitido(nivel: pixpin_nivel::Nivel) -> std::time::Duration` — 300 ms `Completo`, 500 ms `Ligero` (D122)
  - `impl Pintor { pub fn tinta_cacheada(&self, cache: &mut CacheTinta, clave: (u64, u32, u32), contorno: &[(f32, f32)], color: Color) }` — `clave = (id, version, indice de la orden)`

Por qué realizaciones y no un bitmap por elemento (D121): en el i3 con HD 4000 la memoria de vídeo es la RAM; mil bitmaps de trazo la agotan, mil teselaciones no.

- [ ] **Step 1: Pruebas (fallan)**

En `tinta.rs`, `mod pruebas`:

```rust
#[test]
fn el_retardo_nitido_es_mas_largo_en_ligero() {
    use pixpin_nivel::Nivel;
    assert_eq!(retardo_nitido(Nivel::Completo).as_millis(), 300);
    assert_eq!(retardo_nitido(Nivel::Ligero).as_millis(), 500);
}

#[test]
fn fijar_otra_escala_tira_lo_realizado_y_la_misma_no() {
    let mut c = CacheTinta::nueva();
    assert_eq!(c.escala(), 1.0);
    c.fijar_escala(1.0);
    assert_eq!(c.escala(), 1.0);
    c.fijar_escala(2.0);
    assert_eq!(c.escala(), 2.0);
    assert_eq!(c.cuantas(), 0);
}
```

Prueba con GPU (`#[ignore]`, como las demás de `pixpin-render` que necesitan dispositivo). El ayudante crea un D3D11 propio porque `pixpin-capture::Dispositivo` es L2 y no se puede usar desde aquí:

```rust
/// Motor y un destino de `ancho x alto` sobre un D3D11 hardware propio.
fn motor_y_destino_de_prueba(ancho: u32, alto: u32) -> (crate::MotorRender, windows::Win32::Graphics::Direct2D::ID2D1Bitmap1) {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11CreateDevice,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
    let mut d3d = None;
    // SAFETY: salidas locales; sin adaptador concreto ni capas de depuracion.
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut d3d),
            None,
            None,
        )
        .expect("sin D3D11 hardware");
    }
    let d3d = d3d.expect("dispositivo");
    let motor = crate::MotorRender::nuevo(&d3d).expect("motor");
    let desc = D3D11_TEXTURE2D_DESC {
        Width: ancho,
        Height: alto,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
        ..Default::default()
    };
    let mut textura = None;
    // SAFETY: descripcion local valida; salida local.
    unsafe { d3d.CreateTexture2D(&desc, None, Some(&mut textura)).expect("textura") };
    let destino = motor.destino_desde_textura(&textura.expect("textura")).expect("destino");
    (motor, destino)
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn pintar_dos_veces_la_misma_clave_realiza_una_sola_vez() {
    let (motor, destino) = motor_y_destino_de_prueba(64, 64);
    let mut cache = CacheTinta::nueva();
    let contorno = [(10.0, 10.0), (50.0, 10.0), (50.0, 50.0), (10.0, 50.0)];
    for _ in 0..2 {
        motor
            .dibujar(&destino, |p| p.tinta_cacheada(&mut cache, (7, 1, 0), &contorno, Color::NEGRO))
            .unwrap();
    }
    assert_eq!(cache.cuantas(), 1);
    motor
        .dibujar(&destino, |p| p.tinta_cacheada(&mut cache, (7, 2, 0), &contorno, Color::NEGRO))
        .unwrap();
    assert_eq!(cache.cuantas(), 1, "una version nueva sustituye, no acumula");
}
```

> Si `Color::NEGRO` no existe, usar `Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }`. Si los `use` de D3D11 no casan con `windows` 0.62, copiar los de las pruebas `#[ignore]` que ya crean dispositivo en `crates/pixpin-render/src/superficie.rs`.

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p pixpin-render tinta -- --test-threads=1`
Expected: FAIL de compilación.

- [ ] **Step 3: Implementación**

`tinta.rs`:

```rust
use std::collections::HashMap;
use std::time::Duration;

use windows::Win32::Graphics::Direct2D::ID2D1GeometryRealization;

/// Cuanto tiene que estar quieto el zoom para rehacer la tinta nitida (D122,
/// el `shouldCacheIgnoreZoom` de Excalidraw). Mientras tanto se estira lo
/// realizado: algo borroso y casi gratis.
pub fn retardo_nitido(nivel: pixpin_nivel::Nivel) -> Duration {
    match nivel {
        pixpin_nivel::Nivel::Ligero => Duration::from_millis(500),
        _ => Duration::from_millis(300),
    }
}

struct Realizada {
    version: u32,
    realizacion: ID2D1GeometryRealization,
}

/// La tinta ya teselada por Direct2D, por elemento y orden.
///
/// Una realizacion es la geometria convertida en triangulos a una escala:
/// pintarla no recalcula nada en el procesador. Se rehace solo si cambia la
/// version del elemento o si se fija otra escala.
pub struct CacheTinta {
    mapa: HashMap<(u64, u32), Realizada>,
    escala: f32,
}

impl CacheTinta {
    pub fn nueva() -> Self {
        Self { mapa: HashMap::new(), escala: 1.0 }
    }

    /// Dispositivo perdido o documento nuevo: las realizaciones son del
    /// dispositivo viejo y no valen.
    pub fn vaciar(&mut self) {
        self.mapa.clear();
    }

    pub fn escala(&self) -> f32 {
        self.escala
    }

    /// Se llama cuando el zoom lleva `retardo_nitido` quieto.
    pub fn fijar_escala(&mut self, escala: f32) {
        if escala != self.escala {
            self.escala = escala;
            self.mapa.clear();
        }
    }

    pub fn cuantas(&self) -> usize {
        self.mapa.len()
    }
}

impl Default for CacheTinta {
    fn default() -> Self {
        Self::nueva()
    }
}
```

`lienzo.rs`, en `impl Pintor<'_>`:

```rust
    /// Como `tinta`, pero reutilizando la teselacion de fotogramas
    /// anteriores. `clave` = (id del elemento, version, indice de la orden).
    /// Si la realizacion no se puede crear (contexto sin D2D 1.1 o
    /// dispositivo raro), se pinta sin cache: se ve igual, cuesta mas.
    pub fn tinta_cacheada(
        &self,
        cache: &mut crate::tinta::CacheTinta,
        clave: (u64, u32, u32),
        contorno: &[(f32, f32)],
        color: Color,
    ) {
        use windows::Win32::Graphics::Direct2D::{
            D2D1_DEFAULT_FLATTENING_TOLERANCE, ID2D1DeviceContext1,
        };
        use windows::core::Interface;
        if contorno.len() < 3 {
            return;
        }
        let Ok(ctx1) = self.motor.contexto().cast::<ID2D1DeviceContext1>() else {
            return self.tinta(contorno, color);
        };
        let (id, version, indice) = clave;
        let vale = cache
            .mapa
            .get(&(id, indice))
            .is_some_and(|r| r.version == version);
        if !vale {
            let Some(geometria) = self.geometria_tinta(contorno) else {
                return;
            };
            // La tolerancia se divide por la escala: a 200 % hacen falta
            // el doble de triangulos para que la curva no se vea poligonal.
            let tolerancia = D2D1_DEFAULT_FLATTENING_TOLERANCE / cache.escala.max(0.01);
            // SAFETY: geometria recien creada y contexto vivo.
            let Ok(r) = (unsafe { ctx1.CreateFilledGeometryRealization(&geometria, tolerancia) }) else {
                return self.tinta(contorno, color);
            };
            cache.mapa.insert((id, indice), crate::tinta::Realizada { version, realizacion: r });
        }
        let Some(pincel) = self.pincel(color) else {
            return;
        };
        let r = &cache.mapa[&(id, indice)].realizacion;
        // SAFETY: dentro del fotograma; realizacion y pincel vivos.
        unsafe { ctx1.DrawGeometryRealization(r, &pincel) };
    }
```

Para que `lienzo.rs` acceda a `mapa`, `escala` y `Realizada`, declararlos `pub(crate)` en `tinta.rs`.

`ventana_editor.rs`:

1. En `abrir`, junto a `let mut cache = Cache::nueva();`:

```rust
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    let retardo = pixpin_render::retardo_nitido(nivel);
    // Cuando cambio el zoom por ultima vez. La camara del editor aun no hace
    // zoom (E4), pero el mecanismo queda puesto y probado.
    let mut zoom_cambiado: Option<std::time::Instant> = None;
```

2. Antes de `esperar_eventos`, sustituir `esperar_eventos(None)` por:

```rust
        if camara.zoom != cache_tinta.escala() {
            let desde = *zoom_cambiado.get_or_insert_with(std::time::Instant::now);
            if desde.elapsed() >= retardo {
                cache_tinta.fijar_escala(camara.zoom);
                capa.soltar();
                zoom_cambiado = None;
                ventana.invalidar();
            }
        }
        let tope = zoom_cambiado.map(|d| retardo.saturating_sub(d.elapsed()).as_millis() as u32);
        pixpin_shell::overlay::esperar_eventos(tope);
```

3. `pintar` y el cierre de `capa.preparar` reciben `&mut cache_tinta`. El bucle de elementos cuenta el índice de orden y pasa la clave:

```rust
            let mut indice = 0u32;
            por_cada_orden(cache, e, camara.zoom, escena.escala.as_ref(), |orden| {
                dibujar_orden(p, orden, vista, Some((&mut *cache_tinta, (e.id, e.version, indice))));
                indice += 1;
            });
```

(`cache` se pasa como argumento y `cache_tinta` se captura por `&mut` en el cierre: son variables distintas y el préstamo es legal. No copiar las órdenes a un `Vec`: `por_cada_orden` existe precisamente para no copiar geometría en cada fotograma.)

4. `dibujar_orden` gana `tinta: Option<(&mut pixpin_render::CacheTinta, (u64, u32, u32))>` y su brazo de tinta:

```rust
        Orden::Tinta { contorno, color } => match tinta {
            Some((c, clave)) => p.tinta_cacheada(c, clave, &a_tuplas(contorno), a_color(*color)),
            None => p.tinta(&a_tuplas(contorno), a_color(*color)),
        },
```

Las llamadas que no son de elementos de la escena (tiradores, pista del imán, cajetín) pasan `None`.

5. Si `motor.dibujar` devuelve `Err` en `pintar` (dispositivo perdido), `cache_tinta.vaciar()`; el `let _ = motor.dibujar(..)` pasa a `if motor.dibujar(..).is_err() { cache_tinta.vaciar(); }`.

- [ ] **Step 4: Ejecutar pruebas**

Run: `cargo test --workspace --no-fail-fast -- --test-threads=1` y `cargo test -p pixpin-render -- --ignored --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A apps crates/pixpin-render
git commit -m "La tinta terminada se tesela una vez y se reutiliza; el zoom la rehace nitida al quedarse quieto"
```

---

### Task 11: Capa sobre pantalla y anotación del pin — muestras y presión

**Files:**
- Modify: `crates/pixpin-ui/src/anotador.rs:45-56` (`EventoAnotador`), `:149-160` (`struct Gesto`), `:358-420` (`Mover`/`Soltar`), `:~300` (`Pulsar`), `:496-530` (`construir`)
- Modify: `apps/pixpin/src/capa.rs` (`CapaViva::muestra`, `ejecutar_capa` :600-662, `pedir_entrada_fina`, `anotar` :280-290)
- Modify: `crates/pixpin-pin/Cargo.toml` (`pixpin-shell = { version = "0.1.0", path = "../pixpin-shell" }`)
- Modify: `crates/pixpin-pin/src/ventana.rs` (`CambioPin::MuestraPuntero`, `WM_MOUSEMOVE` :2392-2415, brazo `WM_POINTERUPDATE`, `WM_LBUTTONUP` olvida el historial)
- Modify: `apps/pixpin/src/pines.rs:829` y `:1014-1026`

**Interfaces:**
- Consumes: `pixpin_shell::puntero::{Muestra, HistorialRaton, muestras_de_lapiz, es_raton_de_lapiz}`, `EventoOverlay::Muestra`, `VentanaOverlay::pedir_entrada_fina` (Tarea 7); `tinta::{OpcionesTinta, FACTOR_VARIABLE, STREAMLINE_LAPIZ}` (Tarea 4).
- Produces:
  - `EventoAnotador::Muestra { p: Punto2, presion: Option<f32> }` — igual que `Mover`, con presión
  - `CambioPin::MuestraPuntero { x: f32, y: f32, presion: Option<f32> }` — en coordenadas de contenido, como `PunteroMovido`
  - `impl CapaViva { pub fn muestra(&mut self, m: Muestra) }`

- [ ] **Step 1: Pruebas del anotador (fallan)**

En `anotador.rs`, `mod pruebas` (usar el patrón de las pruebas existentes de la línea ~690 para crear el anotador y leer el `Terminado`):

```rust
#[test]
fn las_muestras_con_presion_llegan_al_trazo_terminado() {
    let mut a = Anotador::nuevo();
    a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Lapiz));
    a.procesar(EventoAnotador::Pulsar(Punto2::nuevo(0.0, 0.0)));
    a.procesar(EventoAnotador::Muestra { p: Punto2::nuevo(10.0, 0.0), presion: Some(0.4) });
    a.procesar(EventoAnotador::Muestra { p: Punto2::nuevo(20.0, 0.0), presion: Some(0.8) });
    let EfectoAnotador::Terminado(e) = a.procesar(EventoAnotador::Soltar(Punto2::nuevo(20.0, 0.0))) else {
        panic!("tendria que terminar")
    };
    let Figura::Lapiz { puntos, presiones, opciones } = &e.figura else { panic!() };
    assert_eq!(puntos.len(), presiones.len());
    let o = opciones.expect("un trazo nuevo nunca es legado");
    assert_eq!(o.streamline, pixpin_motor2d::tinta::STREAMLINE_LAPIZ);
}

#[test]
fn con_raton_el_trazo_conserva_el_grosor_visual_de_antes() {
    // La rueda de la capa sigue moviendo `grosor` en pixeles; el elemento
    // guarda un strokeWidth, asi que se divide por el factor.
    let mut a = Anotador::nuevo();
    a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Lapiz));
    a.procesar(EventoAnotador::Pulsar(Punto2::nuevo(0.0, 0.0)));
    a.procesar(EventoAnotador::Mover(Punto2::nuevo(30.0, 0.0)));
    let EfectoAnotador::Terminado(e) = a.procesar(EventoAnotador::Soltar(Punto2::nuevo(30.0, 0.0))) else {
        panic!()
    };
    assert!((e.grosor - GROSOR_POR_DEFECTO / pixpin_motor2d::tinta::FACTOR_VARIABLE).abs() < 1e-6);
    let Figura::Lapiz { presiones, .. } = &e.figura else { panic!() };
    assert!(presiones.is_empty());
}
```

> Si `Anotador::nuevo()` pide argumentos o `Soltar` devuelve el elemento por otro efecto, seguir el patrón de `anotador.rs:740-750`.

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p pixpin-ui anotador -- --test-threads=1`
Expected: FAIL de compilación (`EventoAnotador::Muestra` no existe).

- [ ] **Step 3: Implementación**

`anotador.rs`:

1. Variante:

```rust
    /// Como `Mover`, pero con un punto de la entrada fina: uno que Windows
    /// fusiono o uno del lapiz con presion (E1).
    Muestra { p: Punto2, presion: Option<f32> },
```

2. `struct Gesto` gana `presiones: Vec<f32>` (inicializado `Vec::new()` donde se crea el `Gesto` en `Pulsar`).

3. Al principio de `procesar`, convertir la muestra en el mismo camino de `Mover` guardando la presión:

```rust
        let (evento, presion) = match evento {
            EventoAnotador::Muestra { p, presion } => (EventoAnotador::Mover(p), presion),
            otro => (otro, None),
        };
```

y en los dos sitios donde se hace `g.puntos.push(p)` (líneas 386 y 413), sustituir por `anadir_punto(g, p, presion)` con:

```rust
/// Mismo contrato que `gesto::anadir_a_lapiz` del motor: duplicados exactos
/// fuera, relleno hacia atras con la primera presion real, y listas siempre
/// de la misma longitud.
fn anadir_punto(g: &mut Gesto, p: Punto2, presion: Option<f32>) {
    if g.puntos.last() == Some(&p) {
        return;
    }
    g.puntos.push(p);
    match presion {
        Some(pr) => {
            if g.presiones.is_empty() {
                g.presiones.resize(g.puntos.len() - 1, pr);
            }
            g.presiones.push(pr);
        }
        None => {
            if let Some(&u) = g.presiones.last() {
                g.presiones.push(u);
            }
        }
    }
}
```

(En `Soltar`, `presion` es `None`.)

4. `construir`, rama del lápiz y grosor:

```rust
            Herramienta::Lapiz => Figura::Lapiz {
                puntos: g.puntos.clone(),
                presiones: if g.presiones.len() == g.puntos.len() {
                    g.presiones.clone()
                } else {
                    Vec::new()
                },
                opciones: Some(pixpin_motor2d::tinta::OpcionesTinta {
                    variabilidad: pixpin_motor2d::tinta::Variabilidad::Variable,
                    streamline: if g.presiones.is_empty() {
                        pixpin_motor2d::tinta::STREAMLINE_RATON
                    } else {
                        pixpin_motor2d::tinta::STREAMLINE_LAPIZ
                    },
                }),
            },
```

y donde el elemento toma `grosor: self.grosor` (líneas 468 y 564), para el lápiz:

```rust
            // La capa y el pin miden el grosor en pixeles (la rueda lo
            // cambia asi); la tinta de Excalidraw lo quiere como strokeWidth.
            grosor: if self.herramienta == Herramienta::Lapiz {
                self.grosor / pixpin_motor2d::tinta::FACTOR_VARIABLE
            } else {
                self.grosor
            },
```

Los `matches!(evento, EventoAnotador::Pulsar(_) | EventoAnotador::Mover(_) | EventoAnotador::Soltar(_))` de `capa.rs:286` y `pines.rs:1024` ganan `| EventoAnotador::Muestra { .. }`; el `if let` de `pines.rs:1014` gana `| EventoAnotador::Muestra { p, .. }`.

`capa.rs`:

1. En `CapaViva::nueva`, tras crear la ventana: `ventana.pedir_entrada_fina();` (usar el nombre del campo de la ventana en `CapaViva`).
2. Método:

```rust
    /// Un punto de la entrada fina, en coordenadas del escritorio virtual.
    pub fn muestra(&mut self, m: pixpin_shell::puntero::Muestra) {
        let (x, y) = (m.x() - self.area.x as f32, m.y() - self.area.y as f32);
        if self.caja.contiene(Punto { x: x as i32, y: y as i32 }) {
            return;
        }
        self.anotar(EventoAnotador::Muestra { p: Punto2::nuevo(x, y), presion: m.presion() });
    }
```

3. En `ejecutar_capa`, junto a `RatonMovido`: `EventoOverlay::Muestra(m) => { capa.muestra(m); true }`.

`crates/pixpin-pin/src/ventana.rs`:

1. `CambioPin` gana:

```rust
    /// Un punto de la entrada fina mientras se anota (E1): uno que Windows
    /// fusiono o uno del lapiz. En coordenadas del contenido, con subpixel.
    MuestraPuntero { x: f32, y: f32, presion: Option<f32> },
```

2. Un `thread_local!` en el módulo:

```rust
thread_local! {
    /// Historial del raton del pin que se esta anotando. Uno basta: solo se
    /// anota un pin a la vez.
    static HISTORIAL: std::cell::RefCell<pixpin_shell::puntero::HistorialRaton> =
        const { std::cell::RefCell::new(pixpin_shell::puntero::HistorialRaton::nuevo()) };
}
```

3. Conversión de pantalla a contenido, junto a `punto_contenido`:

```rust
    /// Como `punto_contenido`, desde coordenadas del escritorio virtual y
    /// sin redondear: las muestras traen subpixel.
    fn muestra_a_contenido(i: &PinInterno, m: pixpin_shell::puntero::Muestra) -> (f32, f32) {
        let mut r = RECT::default();
        // SAFETY: GetWindowRect sobre la ventana del propio pin.
        unsafe {
            let _ = GetWindowRect(i.hwnd, &mut r);
        }
        let (ox, oy) = origen_contenido(i.hwnd, i.estado.rect());
        (m.x() - r.left as f32 - ox as f32, m.y() - r.top as f32 - oy as f32)
    }
```

4. En `WM_MOUSEMOVE`, la rama `if i.anotando { .. }` pasa a:

```rust
                if i.anotando {
                    const MK_LBUTTON: usize = 0x0001;
                    if wparam.0 & MK_LBUTTON != 0 {
                        if pixpin_shell::puntero::es_raton_de_lapiz() {
                            // Sus muestras llegan por WM_POINTERUPDATE.
                            return LRESULT(0);
                        }
                        let c = punto_contenido(i, lparam);
                        let mut r = RECT::default();
                        // SAFETY: GetWindowRect y GetMessageTime sobre el mensaje actual.
                        let (x, y, t) = unsafe {
                            let _ = GetWindowRect(hwnd, &mut r);
                            let (ox, oy) = origen_contenido(i.hwnd, i.estado.rect());
                            (r.left + ox + c.x, r.top + oy + c.y, GetMessageTime() as u32)
                        };
                        for m in HISTORIAL.with(|h| h.borrow_mut().recuperar(x, y, t)) {
                            let (cx, cy) = muestra_a_contenido(i, m);
                            (i.al_cambiar)(CambioPin::MuestraPuntero { x: cx, y: cy, presion: None });
                        }
                    }
                    (i.al_cambiar)(CambioPin::PunteroMovido(punto_contenido(i, lparam)));
                }
```

5. En el `WM_LBUTTONUP` de anotación (línea ~2435): `HISTORIAL.with(|h| h.borrow_mut().olvidar());` antes de `PunteroSoltado`.

6. Brazo nuevo:

```rust
        WM_POINTERUPDATE if interno_de(hwnd).is_some_and(|i| i.anotando) => {
            if let (Some(i), Some(muestras)) =
                (interno_de(hwnd), pixpin_shell::puntero::muestras_de_lapiz(wparam))
            {
                for m in muestras {
                    let (x, y) = muestra_a_contenido(i, m);
                    (i.al_cambiar)(CambioPin::MuestraPuntero { x, y, presion: m.presion() });
                }
            }
            // SAFETY: reenvio estandar; Windows sigue sintetizando los clics.
            unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
        }
```

Añadir los `use` que falten (`GetMessageTime`, `WM_POINTERUPDATE`, `GetWindowRect`, `RECT`).

`apps/pixpin/src/pines.rs:829`, brazo nuevo:

```rust
            CambioPin::MuestraPuntero { x, y, presion } => self.anotar(
                id,
                EventoAnotador::Muestra { p: pixpin_motor2d::Punto2::nuevo(x, y), presion },
            ),
```

Cualquier otro `match` exhaustivo sobre `CambioPin` que no compile: `CambioPin::MuestraPuntero { .. } => {}`.

- [ ] **Step 4: Ejecutar pruebas**

Run: `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --no-fail-fast -- --test-threads=1`
Expected: PASS (incluida `capas.rs`: `pixpin-pin` L2 → `pixpin-shell` L1 es legal).

- [ ] **Step 5: Commit**

```bash
git add -A apps crates/pixpin-ui crates/pixpin-pin
git commit -m "La capa y el pin reciben los mismos puntos y la misma presion que el editor"
```

---

### Task 12: Puertas, mapa, avisos de licencia, novedades y entrega

**Files:**
- Modify: `crates/pixpin-motor2d/tests/puertas.rs` (puerta de geometría)
- Create: `crates/pixpin-render/tests/puerta_tinta.rs` (puerta de fotograma, `#[ignore]`)
- Create: `docs/excalidraw/mapa.md`
- Create: `THIRD-PARTY-NOTICES.md`
- Create: `herramientas/novedades-excalidraw.mjs`
- Create: `medidas/2026-09-13-equipo-desarrollo-e1.md`

**Interfaces:**
- Consumes: todo lo anterior.
- Produces: puertas automáticas de D-geometría y D-pintado; el mapa de traducción (D101); el guion de novedades (D103).

- [ ] **Step 1: Puerta de geometría**

En `tests/puertas.rs`:

```rust
#[test]
fn un_trazo_de_cinco_mil_puntos_se_calcula_en_menos_de_dos_milisegundos() {
    // Spec E1 §4: mientras se dibuja se recalcula el trazo entero en cada
    // fotograma (como Excalidraw). Si esto se pone rojo, el arreglo es
    // recalcular solo la cola, no subir el tope.
    let trazo = trazo_largo(5_000);
    let o = Some(pixpin_motor2d::tinta::OpcionesTinta::default());
    // Calentar: la primera vuelta paga paginas y cache del procesador.
    let _ = pixpin_motor2d::tinta::contorno_de_lapiz(&trazo, &[], 1.0, o);
    let mut mejor = std::time::Duration::MAX;
    for _ in 0..5 {
        let t = Instant::now();
        let c = pixpin_motor2d::tinta::contorno_de_lapiz(&trazo, &[], 1.0, o);
        mejor = mejor.min(t.elapsed());
        assert!(!c.is_empty());
    }
    assert!(
        mejor.as_micros() < 2_000 * FACTOR as u128,
        "5.000 puntos en {mejor:?}"
    );
}
```

Run: `cargo test -p pixpin-motor2d --release --test puertas -- --test-threads=1`
Expected: PASS. Apuntar el tiempo real en la hoja de medidas.

- [ ] **Step 2: Puerta de fotograma con 1.000 trazos (GPU)**

`crates/pixpin-render/tests/puerta_tinta.rs`:

```rust
//! Puerta de pintado de E1 (spec §5): un fotograma con 1.000 trazos ya
//! realizados en menos de 3 ms. Necesita GPU: `--ignored`.

use std::time::Instant;

use pixpin_render::{CacheTinta, Color, MotorRender};

fn contorno(i: usize) -> Vec<(f32, f32)> {
    let (bx, by) = ((i % 40) as f32 * 45.0, (i / 40) as f32 * 40.0);
    (0..60)
        .map(|k| {
            let a = k as f32 / 60.0 * std::f32::consts::TAU;
            (bx + 20.0 + a.cos() * 15.0, by + 20.0 + a.sin() * 8.0)
        })
        .collect()
}

/// Las mismas lineas que `motor_y_destino_de_prueba` de `src/tinta.rs`
/// (Tarea 10): una prueba de integracion no ve los ayudantes `#[cfg(test)]`
/// del crate, asi que se repiten aqui.
fn motor_y_destino(ancho: u32, alto: u32) -> (MotorRender, windows::Win32::Graphics::Direct2D::ID2D1Bitmap1) {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11CreateDevice,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
    let mut d3d = None;
    // SAFETY: salidas locales; sin adaptador concreto ni capas de depuracion.
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut d3d),
            None,
            None,
        )
        .expect("sin D3D11 hardware");
    }
    let d3d = d3d.expect("dispositivo");
    let motor = MotorRender::nuevo(&d3d).expect("motor");
    let desc = D3D11_TEXTURE2D_DESC {
        Width: ancho,
        Height: alto,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
        ..Default::default()
    };
    let mut textura = None;
    // SAFETY: descripcion local valida; salida local.
    unsafe { d3d.CreateTexture2D(&desc, None, Some(&mut textura)).expect("textura") };
    let destino = motor.destino_desde_textura(&textura.expect("textura")).expect("destino");
    (motor, destino)
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn mil_trazos_realizados_se_pintan_en_menos_de_tres_milisegundos() {
    let (motor, destino) = motor_y_destino(1920, 1080);
    let contornos: Vec<_> = (0..1_000).map(contorno).collect();
    let mut cache = CacheTinta::nueva();
    let negro = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    let fotograma = |cache: &mut CacheTinta| {
        motor
            .dibujar(&destino, |p| {
                for (i, c) in contornos.iter().enumerate() {
                    p.tinta_cacheada(cache, (i as u64, 1, 0), c, negro);
                }
            })
            .unwrap();
    };
    fotograma(&mut cache); // realiza
    let mut mejor = std::time::Duration::MAX;
    for _ in 0..10 {
        let t = Instant::now();
        fotograma(&mut cache);
        mejor = mejor.min(t.elapsed());
    }
    assert!(mejor.as_micros() < 3_000, "1.000 trazos en {mejor:?}");
}
```

> `pixpin-render` ya depende de `windows` con las features de Direct3D11 y DXGI, así que la prueba de integración compila sin dependencias nuevas.

Run: `cargo test -p pixpin-render --release --test puerta_tinta -- --ignored --test-threads=1`
Expected: PASS; apuntar el tiempo.

- [ ] **Step 3: Mapa de traducción (D101)**

`docs/excalidraw/mapa.md`:

```markdown
# Mapa de traducción de Excalidraw a PixPin

Cada fila es un fichero del original que tiene porte en Rust. `novedades-excalidraw.mjs`
lee la columna **Origen** y la **Revisión** para decir qué ha cambiado desde el último porte.

| Origen (repo:ruta) | Revisión portada | Módulo Rust | Notas |
|---|---|---|---|
| npm:perfect-freehand@1.2.0 `getStrokePoints`, `getStrokeOutlinePoints` | 1.2.0 | `pixpin-motor2d::tinta::freehand` | sin taper ni tapas planas |
| excalidraw:packages/laser-pointer/src/state.ts | afa3a65 | `pixpin-motor2d::tinta::laser` | solo `simplify 0`, presión 1 |
| excalidraw:packages/laser-pointer/src/math.ts | afa3a65 | `pixpin-motor2d::tinta::laser` | |
| excalidraw:packages/element/src/shape.ts (`getFreedrawOutlinePoints`, factores) | afa3a65 | `pixpin-motor2d::tinta` | |
| excalidraw:packages/element/src/shape.ts (`getSvgPathFromStroke`) | afa3a65 | `pixpin-render::tinta::pasos_de_tinta` | sin recorte a 2 decimales |
| excalidraw:packages/common/src/constants.ts (`FREEDRAW_STROKE_WIDTH`, `DEFAULT_STROKE_STREAMLINE*`) | afa3a65 | `pixpin-motor2d::tinta` | |
| excalidraw:packages/element/src/types.ts (`StrokeOptions`, freedraw) | afa3a65 | `pixpin-motor2d::{tinta, excalidraw}` | formato |

## Cómo portar una novedad

1. `node herramientas/novedades-excalidraw.mjs` lista los commits que tocan estas rutas.
2. Actualizar `herramientas/oraculo-excalidraw` (versiones npm si cambiaron) y `npm run generar`.
3. `cargo test -p pixpin-motor2d --test oraculo_tinta`: lo rojo es lo que hay que portar.
4. Portar, dejar verde, actualizar la columna **Revisión** de esta tabla.

Fork `zsviczian/excalidraw` (MIT): solo se mira lo que añade sobre el oficial. El plugin
`obsidian-excalidraw-plugin` es **AGPL-3.0**: nunca se porta código.
```

- [ ] **Step 4: Avisos de licencia**

`THIRD-PARTY-NOTICES.md`:

```markdown
# Avisos de terceros

PixPin Max incluye código traducido a Rust de los siguientes proyectos, bajo sus licencias.

## Excalidraw

Módulos: `crates/pixpin-motor2d/src/tinta/{mod.rs,laser.rs}`, `crates/pixpin-render/src/tinta.rs`.

MIT License

Copyright (c) 2020 Excalidraw

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## perfect-freehand

Módulo: `crates/pixpin-motor2d/src/tinta/freehand.rs`.

MIT License

Copyright (c) 2021 Stephen Ruiz Ltd

(mismo texto de la licencia MIT de arriba)
```

> Copiar el texto exacto de `LICENSE` de cada repo (están en el scratchpad `referencias/excalidraw/LICENSE` y `referencias/perfect-freehand/LICENSE`, o en `node_modules/*/LICENSE`) en vez de «mismo texto»: el aviso debe ser literal.

- [ ] **Step 5: Guion de novedades (D103)**

`herramientas/novedades-excalidraw.mjs`:

```js
// Lista los commits de Excalidraw (oficial y fork de zsviczian) que tocan
// ficheros del mapa desde la revision portada. Uso:
//   node herramientas/novedades-excalidraw.mjs [ruta-clon-oficial] [ruta-clon-zsviczian]
// Por defecto usa "proyectos de referencia/excalidraw" y ".../excalidraw-zsviczian".
import { execFileSync } from "node:child_process";
import { readFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const raiz = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const mapa = readFileSync(join(raiz, "docs", "excalidraw", "mapa.md"), "utf8");
const filas = [...mapa.matchAll(/^\| excalidraw:(\S+)[^|]*\| (\w+) \|/gm)].map((m) => ({
  ruta: m[1],
  revision: m[2],
}));

const clones = [
  process.argv[2] ?? join(raiz, "proyectos de referencia", "excalidraw"),
  process.argv[3] ?? join(raiz, "proyectos de referencia", "excalidraw-zsviczian"),
];

for (const clon of clones) {
  if (!existsSync(join(clon, ".git"))) {
    console.log(`(no hay clon en ${clon}: git clone https://github.com/excalidraw/excalidraw)`);
    continue;
  }
  execFileSync("git", ["-C", clon, "fetch", "--quiet", "origin"]);
  console.log(`\n== ${clon}`);
  for (const { ruta, revision } of filas) {
    let log = "";
    try {
      log = execFileSync("git", ["-C", clon, "log", "--oneline", `${revision}..origin/HEAD`, "--", ruta], {
        encoding: "utf8",
      }).trim();
    } catch {
      log = `(la revision ${revision} no existe en este clon)`;
    }
    console.log(`\n${ruta} desde ${revision}:\n${log || "  sin cambios"}`);
  }
}
console.log("\nperfect-freehand: comparar la version de packages/excalidraw/package.json con 1.2.0.");
```

Run: `git clone --depth 200 https://github.com/excalidraw/excalidraw "proyectos de referencia/excalidraw"` (la carpeta está en `.gitignore`) y `node herramientas/novedades-excalidraw.mjs`
Expected: lista por ruta; con `afa3a65` recién portado, «sin cambios» o solo commits posteriores al 2026-09-10.

- [ ] **Step 6: Suite completa, binario y medidas**

Run: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --no-fail-fast -- --test-threads=1`, `cargo build --release -p pixpin`
Expected: todo verde (salvo el fallo conocido de instancia única con la app abierta).

Copiar `target/release/pixpinmax.exe` a la carpeta `portable\` que usa el usuario (scratchpad de la sesión `26afd105…`), **sin tocar su `almacen\`**; cerrar antes su `pixpinmax.exe` si está abierto, avisándole.

`medidas/2026-09-13-equipo-desarrollo-e1.md`: tiempos de las puertas de los Steps 1 y 2, tamaño del binario antes/después, y una tabla vacía para que el usuario rellene en su uso real: CPU dibujando sin parar (Administrador de tareas, columna CPU del proceso, tope ≤ 10 % de un núcleo), CPU en reposo con el editor abierto (0 %), RAM privada antes y después de abrir el editor con un dibujo grande (≤ 40 MB de diferencia). **El agente no mide con entrada sintetizada** (regla del usuario).

- [ ] **Step 7: Commit**

```bash
git add -A docs/excalidraw THIRD-PARTY-NOTICES.md herramientas medidas crates
git commit -m "E1 cerrada: puertas de tinta, mapa de traduccion, avisos MIT y guion de novedades"
```

- [ ] **Step 8: Lista para el usuario**

Entregar en el chat (no probar por él):

1. En el editor (bandeja → Editor), escribir el nombre rápido con el ratón: letras redondas, sin esquinas ni agujeros.
2. Espiral rápida, un ocho que se cruce y un clic suelto (debe salir un punto redondo).
3. Con el lápiz: apretar y aflojar; el grosor cambia.
4. Teclas `1`, `2`, `3` (fino, medio, grueso) y `V` (variable ↔ constante).
5. Lo mismo en la capa de anotar pantalla y dentro de un pin (doble clic para anotar).
6. Abrir un pin anotado **antes** de esta versión: sus trazos no deben verse más gordos.

El punto 5 de la spec («guardar `.excalidraw` y abrirlo en excalidraw.com») **no se puede probar a mano todavía**: el editor no tiene guardar (E4/E5). Lo cubre la prueba automática de ida y vuelta de la Tarea 4; decírselo al usuario.
