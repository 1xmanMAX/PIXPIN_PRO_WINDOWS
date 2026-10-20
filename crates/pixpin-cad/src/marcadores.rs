//! **Los marcadores de los planos y de los modelos** (19-oct-2026). El
//! usuario: «los marcadores deben existir en CAD y en todos los formatos,
//! para ir a una parte concreta (un muro concreto), guardando no solo la
//! posicion sino tambien el zoom. En BIM, al poner un marcador puedo hacer
//! anotaciones, y solo se ven en esa vista, a ese zoom, en esa posicion y en
//! ese estado (si estaba en corte o algo): las anotaciones van con todo el
//! estado; asi se guarda informacion de esos puntos en un modelo 3D».
//!
//! - **En el plano** ([`Marcador2d`]): el centro y lo que se ve de ancho y
//!   de alto (la posicion y el zoom), en coordenadas del dibujo.
//! - **En el modelo** ([`Estado3d`]): la camara (objetivo, rumbo, altura y
//!   distancia: el zoom), la caja de seccion, lo aislado, las plantas y
//!   categorias ocultas y, para volver igual, el tema y las aristas.
//!   Lo anotado sobre un marcador 3D es tinta en la pantalla y solo se ve
//!   mientras la vista **es** la del marcador ([`coincide`]): al girar se
//!   va y al volver vuelve.
//! - **Los marcos para imprimir del modelo** ([`Marco3d`]), como los del
//!   plano (I, numerados 1, 2, 3…, Ctrl+P en grupo): cada uno es un recuadro
//!   de la pantalla con el estado en que se hizo; se ve cuando la vista es
//!   esa, y al imprimir se vuelve a cada uno y se fotografia.
//!
//! La pantalla se cuenta **desde el centro y en altos de ventana**
//! ([`a_normal`]): la camara 3D tiene el campo de vision vertical fijo, asi
//! que un punto del modelo cae en el mismo `(x, y)` normalizado aunque la
//! ventana cambie de tamano, y la tinta y los marcos no se separan de el.
//!
//! Todo va en un fichero JSON que da quien abre la ventana ([`Guardados`]).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::modelo::Constructor;
use crate::texto::Textos;

/// Un marcador del plano: el centro (coordenadas del dibujo, absolutas) y lo
/// que se ve de ancho y de alto alli.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marcador2d {
    pub id: String,
    #[serde(default)]
    pub nombre: String,
    pub x: f64,
    pub y: f64,
    pub ancho: f64,
    pub alto: f64,
}

impl Marcador2d {
    /// Unidades del dibujo por pixel para ver lo mismo en una ventana de
    /// `w` x `h`: cabe todo lo que se veia (el lado que mas aprieta).
    pub fn px_en(&self, w: u32, h: u32) -> f64 {
        let px = (self.ancho / w.max(1) as f64).max(self.alto / h.max(1) as f64);
        if px.is_finite() && px > 0.0 { px } else { 1.0 }
    }
}

/// **El estado entero de la vista de un modelo**: lo que hace falta para
/// volver justo ahi. Coordenadas del mundo absolutas (con el origen del
/// modelo sumado), asi no dependen de como se leyo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Estado3d {
    pub objetivo: [f64; 3],
    pub rumbo: f64,
    pub altura: f64,
    pub dist: f64,
    /// La caja de seccion `[x0, y0, z0, x1, y1, z1]`, si estaba puesta.
    #[serde(default)]
    pub seccion: Option<[f64; 6]>,
    /// El elemento aislado (su numero en el modelo).
    #[serde(default)]
    pub aislado: Option<u32>,
    /// Las plantas y categorias ocultas (ver `arbol`), ordenadas.
    #[serde(default)]
    pub ocultos: Vec<String>,
    #[serde(default)]
    pub claro: bool,
    #[serde(default = "si")]
    pub aristas: bool,
}

fn si() -> bool {
    true
}

/// **Si dos estados son la misma vista**: la misma camara (con una
/// milesima de margen, lo que se pierde al guardar y al animar), la misma
/// caja de seccion, lo mismo aislado y lo mismo oculto. El tema y las
/// aristas no cuentan: cambian como se ve, no lo que se ve.
pub fn coincide(a: &Estado3d, b: &Estado3d) -> bool {
    let d = (0..3).map(|k| (a.objetivo[k] - b.objetivo[k]).powi(2)).sum::<f64>().sqrt();
    let escala = b.dist.abs().max(1e-6);
    let mut dr = (a.rumbo - b.rumbo) % std::f64::consts::TAU;
    if dr > std::f64::consts::PI {
        dr -= std::f64::consts::TAU;
    } else if dr < -std::f64::consts::PI {
        dr += std::f64::consts::TAU;
    }
    let camara = d < escala * 2e-3 && dr.abs() < 2e-3 && (a.altura - b.altura).abs() < 2e-3 && (a.dist / escala - 1.0).abs() < 2e-3;
    let seccion = match (a.seccion, b.seccion) {
        (None, None) => true,
        (Some(p), Some(q)) => p.iter().zip(q.iter()).all(|(x, y)| (x - y).abs() <= escala * 1e-3 + 1e-6),
        _ => false,
    };
    camara && seccion && a.aislado == b.aislado && a.ocultos == b.ocultos
}

/// Un marcador del modelo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marcador3d {
    pub id: String,
    #[serde(default)]
    pub nombre: String,
    pub estado: Estado3d,
}

/// Un marco para imprimir del modelo: su recuadro en la pantalla
/// (normalizado, [`a_normal`]: `[x0, y0, x1, y1]`) y el estado de la vista.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marco3d {
    pub caja: [f64; 4],
    pub estado: Estado3d,
}

/// Del pixel de la ventana al punto normalizado: desde el centro y en altos
/// de ventana (x a la derecha, y hacia abajo).
pub fn a_normal(x: f64, y: f64, w: u32, h: u32) -> [f64; 2] {
    let h = h.max(1) as f64;
    [(x - w as f64 / 2.0) / h, (y - h / 2.0) / h]
}

/// Y de vuelta, al pixel.
pub fn de_normal(p: [f64; 2], w: u32, h: u32) -> [f64; 2] {
    let hf = h.max(1) as f64;
    [p[0] * hf + w as f64 / 2.0, p[1] * hf + hf / 2.0]
}

/// La caja de un marco, en pixeles de la ventana `[x0, y0, x1, y1]`.
pub fn caja_en_pixeles(caja: [f64; 4], w: u32, h: u32) -> [f64; 4] {
    let a = de_normal([caja[0], caja[1]], w, h);
    let b = de_normal([caja[2], caja[3]], w, h);
    [a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])]
}

/// **Un id nuevo** para un marcador: la hora en nanosegundos y un contador
/// (dos en el mismo instante no se pisan).
pub fn nuevo_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CUENTA: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    format!("m{:x}{:x}", t, CUENTA.fetch_add(1, Ordering::Relaxed))
}

/// Lo que se guarda de un plano o un modelo.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Contenido {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plano: Vec<Marcador2d>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modelo: Vec<Marcador3d>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marcos: Vec<Marco3d>,
}

/// **Los marcadores (y los marcos del modelo) con su fichero.** Sin fichero
/// duran lo que la ventana.
#[derive(Debug, Clone, Default)]
pub struct Guardados {
    pub c: Contenido,
    fichero: Option<PathBuf>,
}

impl Guardados {
    pub fn de(fichero: Option<&Path>) -> Guardados {
        let c = fichero
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|t| serde_json::from_str::<Contenido>(&t).ok())
            .unwrap_or_default();
        Guardados { c, fichero: fichero.map(Path::to_path_buf) }
    }

    /// Lo escribe (sin nada, quita el fichero).
    pub fn guardar(&self) {
        let Some(f) = &self.fichero else { return };
        if self.c == Contenido::default() {
            let _ = std::fs::remove_file(f);
            return;
        }
        let hecho = serde_json::to_string_pretty(&self.c).map_err(std::io::Error::other).and_then(|t| {
            if let Some(d) = f.parent() {
                std::fs::create_dir_all(d)?;
            }
            let tmp = f.with_extension("tmp");
            std::fs::write(&tmp, t)?;
            std::fs::rename(&tmp, f)
        });
        if let Err(e) = hecho {
            tracing::warn!(?e, fichero = %f.display(), "marcadores: no se pudieron guardar");
        }
    }
}

// ---------------------------------------------------------------------------
// Los puntos bajo el riel

/// Lo que ocupa cada punto a lo alto y su diametro (en reposo y con el
/// raton encima): los del riel de los lectores (`pixpin_ui::riel_marcas`).
const PASO: f64 = 30.0;
const PUNTO: f64 = 20.0;
const PUNTO_CERCA: f64 = 26.0;

/// El centro de cada punto, debajo de la isla del riel (`isla`, en pixeles
/// de la ventana), apretados si no caben en `alto`.
pub fn puntos(isla: [f64; 4], n: usize, alto: u32, e: f64) -> Vec<[f64; 2]> {
    if n == 0 {
        return Vec::new();
    }
    let arriba = isla[3] + 12.0 * e;
    let hueco = (alto as f64 - arriba - 16.0 * e).max(0.0);
    let paso = (PASO * e).min(hueco / n as f64).max(1.0);
    let x = (isla[0] + isla[2]) / 2.0;
    (0..n).map(|i| [x, arriba + paso * i as f64 + paso / 2.0]).collect()
}

/// El punto bajo (x, y): dentro de la columna, el mas cercano a lo alto.
pub fn punto_en(isla: [f64; 4], n: usize, alto: u32, e: f64, x: i32, y: i32) -> Option<usize> {
    let p = puntos(isla, n, alto, e);
    let (Some(a), Some(b)) = (p.first(), p.last()) else { return None };
    let medio = (p.get(1).map_or(PASO * e, |q| q[1] - a[1])) / 2.0;
    let (x, y) = (x as f64, y as f64);
    if x < isla[0] || x >= isla[2] || y < a[1] - medio || y >= b[1] + medio {
        return None;
    }
    p.iter().enumerate().min_by(|(_, u), (_, v)| (u[1] - y).abs().total_cmp(&(v[1] - y).abs())).map(|(i, _)| i)
}

fn disco_en(c: &mut Constructor, centro: [f64; 2], r: f64, color: u32, capa: f64) {
    let n = 24;
    let mut t = Vec::with_capacity(n * 3);
    for i in 0..n {
        let a0 = i as f64 / n as f64 * std::f64::consts::TAU;
        let a1 = (i + 1) as f64 / n as f64 * std::f64::consts::TAU;
        t.extend_from_slice(&[centro, [centro[0] + r * a0.cos(), centro[1] + r * a0.sin()], [centro[0] + r * a1.cos(), centro[1] + r * a1.sin()]]);
    }
    c.triangulos(&t, color, Some(capa));
}

/// **Pinta los puntos**: cada uno con su numero (1, 2, 3…), en el cristal
/// oscuro del riel; el que tiene el raton encima, mas grande, y el de la
/// vista de ahora en el azul de lo pulsado.
#[allow(clippy::too_many_arguments)]
pub fn pintar_puntos(c: &mut Constructor, textos: &mut Textos, isla: [f64; 4], n: usize, alto: u32, e: f64, encima: Option<usize>, actual: Option<usize>) {
    let cristal = (crate::convertir::rgba(0x26, 0x26, 0x2c) & 0x00ff_ffff) | 0xEB00_0000;
    let azul = crate::convertir::rgba(0x00, 0x60, 0xDF);
    let blanco = crate::convertir::rgba(255, 255, 255);
    let capa = crate::ventana::CAPA_RAYAS * 10.0;
    for (i, p) in puntos(isla, n, alto, e).into_iter().enumerate() {
        let d = if encima == Some(i) { PUNTO_CERCA } else { PUNTO } * e;
        disco_en(c, p, d / 2.0 + 1.0 * e, 0x60ff_ffff, capa * 2.0);
        disco_en(c, p, d / 2.0, if actual == Some(i) { azul } else { cristal }, capa);
        let t = (i + 1).to_string();
        let tam = 11.0 * e;
        let w = textos.medir_pantalla(&t, tam);
        textos.en_pantalla(c, &t, p[0] - w / 2.0, p[1] + tam * 0.36, tam, blanco, p[0] + d);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn estado() -> Estado3d {
        Estado3d {
            objetivo: [100.0, -20.0, 3.0],
            rumbo: -0.8,
            altura: 0.5,
            dist: 40.0,
            seccion: Some([0.0, 0.0, 0.0, 10.0, 10.0, 3.5]),
            aislado: None,
            ocultos: vec!["t:Muro".into()],
            claro: false,
            aristas: true,
        }
    }

    #[test]
    fn el_estado_se_guarda_y_vuelve_igual() {
        let d = std::env::temp_dir().join(format!("pixpin-marcadores-{}", std::process::id()));
        let f = d.join("m.json");
        let mut g = Guardados::de(Some(&f));
        assert_eq!(g.c, Contenido::default());
        g.c.modelo.push(Marcador3d { id: nuevo_id(), nombre: "Muro norte".into(), estado: estado() });
        g.c.plano.push(Marcador2d { id: nuevo_id(), nombre: String::new(), x: 1.5, y: -2.0, ancho: 300.0, alto: 200.0 });
        g.c.marcos.push(Marco3d { caja: [-0.2, -0.1, 0.3, 0.2], estado: estado() });
        g.guardar();
        let otra = Guardados::de(Some(&f));
        assert_eq!(otra.c, g.c);
        assert!(coincide(&otra.c.modelo[0].estado, &estado()));
        // Sin nada, no queda fichero.
        g.c = Contenido::default();
        g.guardar();
        assert!(!f.exists());
        // Caso negativo: un fichero roto no tumba nada, no hay marcadores.
        std::fs::write(&f, "{roto").unwrap();
        assert_eq!(Guardados::de(Some(&f)).c, Contenido::default());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_estado_viejo_sin_campos_nuevos_se_lee() {
        let t = r#"{"modelo":[{"id":"a","estado":{"objetivo":[0,0,0],"rumbo":0,"altura":0,"dist":1}}]}"#;
        let c: Contenido = serde_json::from_str(t).unwrap();
        assert!(c.modelo[0].estado.aristas && c.modelo[0].estado.seccion.is_none());
    }

    #[test]
    fn la_vista_es_la_del_marcador_solo_si_nada_cambio() {
        let a = estado();
        // Lo que se pierde al guardar y al animar: sigue siendo la misma.
        let mut casi = a.clone();
        casi.objetivo[0] += 1e-6;
        casi.rumbo += std::f64::consts::TAU;
        casi.claro = true;
        casi.aristas = false;
        assert!(coincide(&casi, &a), "el tema y las aristas no cuentan");
        // Casos negativos: girar, acercar, mover el corte, aislar u ocultar es otra.
        let cambios: Vec<Box<dyn Fn(&mut Estado3d)>> = vec![
            Box::new(|e| e.rumbo += 0.05),
            Box::new(|e| e.altura -= 0.05),
            Box::new(|e| e.dist *= 1.1),
            Box::new(|e| e.objetivo[2] += 1.0),
            Box::new(|e| e.seccion = None),
            Box::new(|e| e.seccion.as_mut().unwrap()[5] = 2.0),
            Box::new(|e| e.aislado = Some(7)),
            Box::new(|e| e.ocultos.clear()),
        ];
        for (i, c) in cambios.iter().enumerate() {
            let mut b = a.clone();
            c(&mut b);
            assert!(!coincide(&b, &a), "cambio {i}");
        }
    }

    #[test]
    fn la_pantalla_normalizada_no_depende_del_tamano() {
        let p = a_normal(700.0, 150.0, 1000, 600);
        assert!((p[0] - 200.0 / 600.0).abs() < 1e-12 && (p[1] + 0.25).abs() < 1e-12);
        let q = de_normal(p, 1000, 600);
        assert!((q[0] - 700.0).abs() < 1e-9 && (q[1] - 150.0).abs() < 1e-9);
        // El centro es el centro en cualquier ventana.
        assert_eq!(de_normal([0.0, 0.0], 800, 400), [400.0, 200.0]);
        let c = caja_en_pixeles([0.1, 0.1, -0.1, -0.1], 200, 100);
        assert_eq!(c, [90.0, 40.0, 110.0, 60.0]);
    }

    #[test]
    fn el_marcador_del_plano_ve_lo_mismo_en_otra_ventana() {
        let m = Marcador2d { id: "a".into(), nombre: String::new(), x: 0.0, y: 0.0, ancho: 1000.0, alto: 500.0 };
        assert_eq!(m.px_en(1000, 500), 1.0);
        // Mas estrecha: manda el ancho.
        assert_eq!(m.px_en(500, 500), 2.0);
        // Caso negativo: medidas imposibles no dan un zoom imposible.
        let roto = Marcador2d { ancho: f64::NAN, alto: 0.0, ..m };
        assert_eq!(roto.px_en(100, 100), 1.0);
    }

    #[test]
    fn los_puntos_van_bajo_la_isla_y_se_aprietan_si_no_caben() {
        let isla = [940.0, 16.0, 984.0, 124.0];
        let p = puntos(isla, 3, 800, 1.0);
        assert_eq!(p.len(), 3);
        assert!(p[0][1] > isla[3] && (p[1][1] - p[0][1] - 30.0).abs() < 1e-9);
        assert_eq!(punto_en(isla, 3, 800, 1.0, 962, p[1][1] as i32), Some(1));
        assert_eq!(punto_en(isla, 3, 800, 1.0, 962, p[2][1] as i32 + 4), Some(2));
        // Fuera de la columna, ninguno.
        assert_eq!(punto_en(isla, 3, 800, 1.0, 500, p[1][1] as i32), None);
        assert_eq!(punto_en(isla, 3, 800, 1.0, 962, 700), None);
        assert_eq!(punto_en(isla, 0, 800, 1.0, 962, 150), None);
        // 30 en una ventana baja: caben todos.
        let p = puntos(isla, 30, 400, 1.0);
        assert!(p.last().unwrap()[1] < 400.0);
    }
}
