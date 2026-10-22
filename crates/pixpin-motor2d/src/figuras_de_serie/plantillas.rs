//! **Las plantillas de pagina entera**: planificacion, academico, creativo y
//! diario. Una hoja A4 (794 × 1123, o apaisada) con su banda de color, el
//! titulo en Fraunces y las secciones con su rotulo y su raya de acento.
//!
//! Todas salen de las mismas piezas ([`Pag`]: seccion, renglones, tabla,
//! casillas, caja...) con el color de su categoria ([`Tema`]): cambiar el
//! aire de todas es tocar aqui una vez.

use super::{Al, CAVEAT, FRAUNCES, Lapiz, WORK_SANS, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{ColorRgba, EstiloTrazo};

/// A4 vertical a 96 ppp.
pub(super) const W: f32 = 794.0;
pub(super) const H: f32 = 1123.0;
/// El margen.
pub(super) const M: f32 = 48.0;
/// Donde empieza lo de debajo del titulo.
const Y0: f32 = 128.0;

pub(super) const TINTA: ColorRgba = hex(0x22304a);
pub(super) const ROTULO: ColorRgba = hex(0x5d6b82);
pub(super) const RAYA: ColorRgba = hex(0xcfd7e3);
pub(super) const BORDE: ColorRgba = hex(0xb4c0d3);
pub(super) const BLANCO: ColorRgba = hex(0xffffff);

/// El color de una categoria: el acento (rotulos, banda, rayas fuertes) y
/// su version suave (cabeceras, fondos).
#[derive(Clone, Copy)]
pub(super) struct Tema {
    pub categoria: &'static str,
    pub acento: ColorRgba,
    pub suave: ColorRgba,
    pub titulo: &'static str,
}

pub(super) const PLANIFICACION: Tema =
    Tema { categoria: "Planificación", acento: hex(0x3f6db3), suave: hex(0xe8eff9), titulo: FRAUNCES };
pub(super) const ACADEMICO: Tema =
    Tema { categoria: "Académico", acento: hex(0x2c8a7f), suave: hex(0xe3f3f0), titulo: FRAUNCES };
pub(super) const CREATIVO: Tema =
    Tema { categoria: "Creativo", acento: hex(0xc0692b), suave: hex(0xfbeee3), titulo: FRAUNCES };
pub(super) const DIARIO: Tema =
    Tema { categoria: "Diario", acento: hex(0xbf5878), suave: hex(0xfbe9ef), titulo: CAVEAT };

const DIAS: [&str; 7] = ["LUN", "MAR", "MIÉ", "JUE", "VIE", "SÁB", "DOM"];
const LETRAS: [&str; 7] = ["L", "M", "X", "J", "V", "S", "D"];

/// Una hoja a medio hacer.
pub(super) struct Pag {
    pub l: Lapiz,
    pub t: Tema,
    pub w: f32,
    pub h: f32,
}

impl Pag {
    /// La hoja: papel blanco, banda de color arriba, el titulo y los campos
    /// de la derecha («Semana», «Mes»...), y la raya de acento debajo.
    pub fn nueva(t: Tema, titulo: &str, campos: &[&str]) -> Pag {
        Pag::de_tamano(t, W, H, titulo, campos)
    }

    pub fn apaisada(t: Tema, titulo: &str, campos: &[&str]) -> Pag {
        Pag::de_tamano(t, H, W, titulo, campos)
    }

    fn de_tamano(t: Tema, w: f32, h: f32, titulo: &str, campos: &[&str]) -> Pag {
        let mut p = Pag { l: Lapiz::default(), t, w, h };
        p.l.rect(0.0, 0.0, w, h, Some(hex(0xdde3ec)), Some(BLANCO), 1.0, false);
        if t.titulo == CAVEAT {
            p.adornos();
        }
        p.l.rect(0.0, 0.0, w, 10.0, None, Some(t.acento), 1.0, false);
        if t.titulo == CAVEAT {
            p.l.texto(M, 34.0, titulo, 50.0, CAVEAT, t.acento, true, Al::Izq);
        } else {
            p.l.texto(M, 42.0, titulo, 34.0, t.titulo, TINTA, true, Al::Izq);
        }
        let mut x = w - M;
        for c in campos.iter().rev() {
            let ancho = 150.0;
            p.l.texto(x - ancho, 62.0, &c.to_uppercase(), 10.0, WORK_SANS, ROTULO, true, Al::Izq);
            p.l.linea(x - ancho, 82.0, x, 82.0, BORDE, 1.0);
            x -= ancho + 22.0;
        }
        p.l.linea(M, 104.0, w - M, 104.0, t.acento, 2.0);
        p
    }

    /// Las manchas suaves de las hojas del diario, detras de todo.
    fn adornos(&mut self) {
        let s = self.t.suave;
        let (w, h) = (self.w, self.h);
        // Dentro del papel: una mancha que se sale agranda la figura.
        self.l.circulo(w - 130.0, 200.0, 84.0, None, Some(s), 1.0);
        self.l.circulo(w - 62.0, 300.0, 34.0, None, Some(s), 1.0);
        self.l.circulo(132.0, h - 132.0, 100.0, None, Some(s), 1.0);
        self.l.circulo(262.0, h - 56.0, 40.0, None, Some(s), 1.0);
    }

    /// El rotulo de una seccion y su raya de acento. Devuelve donde empieza
    /// lo de debajo.
    pub fn seccion(&mut self, x: f32, y: f32, w: f32, s: &str) -> f32 {
        self.l.texto(x, y, &s.to_uppercase(), 11.0, WORK_SANS, self.t.acento, true, Al::Izq);
        self.l.linea(x, y + 20.0, x + w, y + 20.0, self.t.acento, 1.25);
        y + 20.0
    }

    /// `n` renglones cada `paso`, empezando un paso por debajo de `y`.
    pub fn renglones(&mut self, x: f32, y: f32, w: f32, n: usize, paso: f32) -> f32 {
        for i in 1..=n {
            let yy = y + paso * i as f32;
            self.l.linea(x, yy, x + w, yy, RAYA, 1.0);
        }
        y + paso * n as f32
    }

    /// `01`, `02`... con su renglon.
    pub fn numerados(&mut self, x: f32, y: f32, w: f32, n: usize, paso: f32) -> f32 {
        for i in 1..=n {
            let yy = y + paso * i as f32;
            self.l.texto(x + 2.0, yy - 17.0, &format!("{i:02}"), 10.0, WORK_SANS, self.t.acento, true, Al::Izq);
            self.l.linea(x + 26.0, yy, x + w, yy, RAYA, 1.0);
        }
        y + paso * n as f32
    }

    /// Casillas para marcar, cada una con su renglon (y su texto si lo hay).
    pub fn casillas(&mut self, x: f32, y: f32, w: f32, n: usize, paso: f32, textos: &[&str]) -> f32 {
        for i in 1..=n {
            let yy = y + paso * i as f32;
            self.l.rect(x + 2.0, yy - 15.0, 12.0, 12.0, Some(BORDE), Some(BLANCO), 1.25, true);
            if let Some(s) = textos.get(i - 1) {
                self.l.texto(x + 22.0, yy - 17.0, s, 11.0, WORK_SANS, ROTULO, false, Al::Izq);
            }
            self.l.linea(x + 22.0, yy, x + w, yy, RAYA, 1.0);
        }
        y + paso * n as f32
    }

    /// Un recuadro redondeado con su rotulo dentro, arriba a la izquierda.
    pub fn caja(&mut self, x: f32, y: f32, w: f32, h: f32, s: &str) {
        self.l.rect(x, y, w, h, Some(BORDE), None, 1.25, true);
        if !s.is_empty() {
            self.l.texto(x + 12.0, y + 10.0, &s.to_uppercase(), 10.0, WORK_SANS, ROTULO, true, Al::Izq);
        }
    }

    /// Una caja con el fondo suave.
    pub fn caja_suave(&mut self, x: f32, y: f32, w: f32, h: f32, s: &str) {
        self.l.rect(x, y, w, h, None, Some(self.t.suave), 1.0, true);
        if !s.is_empty() {
            self.l.texto(x + 12.0, y + 10.0, &s.to_uppercase(), 10.0, WORK_SANS, self.t.acento, true, Al::Izq);
        }
    }

    /// **Una tabla**: cabecera con fondo suave, columnas por fraccion del
    /// ancho, filas alternas apenas sombreadas y el borde de fuera. Devuelve
    /// donde acaba.
    pub fn tabla(&mut self, x: f32, y: f32, w: f32, cols: &[(&str, f32)], filas: usize, fh: f32) -> f32 {
        let total: f32 = cols.iter().map(|(_, f)| f).sum();
        let cab = if cols.iter().any(|(s, _)| !s.is_empty()) { fh.max(26.0) } else { 0.0 };
        let alto = cab + fh * filas as f32;
        if cab > 0.0 {
            self.l.rect(x, y, w, cab, None, Some(self.t.suave), 1.0, false);
        }
        for f in (1..filas).step_by(2) {
            self.l.rect(x, y + cab + fh * f as f32, w, fh, None, Some(hex(0xf7f9fc)), 1.0, false);
        }
        let mut cx = x;
        for (i, (s, f)) in cols.iter().enumerate() {
            let cw = w * f / total;
            if !s.is_empty() {
                self.l.texto(cx + cw / 2.0, y + (cab - 14.0) / 2.0, &s.to_uppercase(), 9.5, WORK_SANS, self.t.acento, true, Al::Centro);
            }
            if i > 0 {
                self.l.linea(cx, y, cx, y + alto, RAYA, 1.0);
            }
            cx += cw;
        }
        for f in 0..filas {
            let yy = y + cab + fh * f as f32;
            if f > 0 || cab > 0.0 {
                self.l.linea(x, yy, x + w, yy, RAYA, 1.0);
            }
        }
        self.l.rect(x, y, w, alto, Some(BORDE), None, 1.25, false);
        y + alto
    }

    /// Una lista de «Etiqueta: ____» (datos del vuelo, del curso...).
    pub fn datos(&mut self, x: f32, y: f32, w: f32, etiquetas: &[&str], paso: f32) -> f32 {
        for (i, s) in etiquetas.iter().enumerate() {
            let yy = y + paso * (i + 1) as f32;
            self.l.texto(x + 2.0, yy - 18.0, s, 11.0, WORK_SANS, ROTULO, false, Al::Izq);
            self.l.linea(x, yy, x + w, yy, RAYA, 1.0);
        }
        y + paso * etiquetas.len() as f32
    }

    /// Los circulitos de los dias (L M X J V S D), para marcar el de hoy.
    pub fn dias_en_circulos(&mut self, xd: f32, y: f32) {
        for (i, d) in LETRAS.iter().enumerate() {
            let cx = xd - (6 - i) as f32 * 22.0;
            self.l.circulo(cx, y, 9.0, Some(BORDE), None, 1.0);
            self.l.texto(cx, y - 6.5, d, 9.0, WORK_SANS, ROTULO, true, Al::Centro);
        }
    }

    pub fn fin(self, clave: &str, nombre: &str) -> FiguraGuardada {
        let cat = self.t.categoria;
        self.l.figura(clave, cat, nombre)
    }
}

// ---------------------------------------------------------------------------

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    v.extend([
        cronograma(),
        diario(),
        semanal(),
        mensual(),
        tareas(),
        metas(),
        habitos(),
        comidas(),
        finanzas(),
        itinerario(),
        viaje(),
        diario_de_viaje(),
        semestre(),
        curso(),
        estudio(),
        entrega(),
        mapa_mental(),
        storyboard(4),
        storyboard(6),
        receta(),
        pentagramas(),
        diario_libre(),
        autocuidado(),
        gratitud(),
        metas_del_dia(),
        sueno(),
    ]);
}

const ANCHO: f32 = W - 2.0 * M;

// ------------------------------------------------------------- planificacion

/// **El cronograma** (el que le hizo falta al usuario para un boceto
/// preliminar): actividades, responsable y doce semanas en tres meses.
fn cronograma() -> FiguraGuardada {
    let mut p = Pag::apaisada(PLANIFICACION, "Cronograma", &["Proyecto", "Inicio"]);
    let (x, w) = (M, H - 2.0 * M);
    let y = 132.0;
    let fijas = [("#", 0.6), ("Actividad", 5.2), ("Resp.", 1.6)];
    let semanas: Vec<String> = (1..=12).map(|s| format!("S{s}")).collect();
    let mut cols: Vec<(&str, f32)> = fijas.to_vec();
    cols.extend(semanas.iter().map(|s| (s.as_str(), 0.9)));
    let total: f32 = cols.iter().map(|(_, f)| f).sum();
    let ancho_fijo = w * 7.4 / total;
    // Los meses encima de las semanas.
    let ancho_semana = w * 0.9 / total;
    for m in 0..3 {
        let mx = x + ancho_fijo + ancho_semana * 4.0 * m as f32;
        p.l.rect(mx, y - 26.0, ancho_semana * 4.0, 24.0, None, Some(p.t.acento), 1.0, false);
        p.l.texto(mx + ancho_semana * 2.0, y - 22.0, &format!("MES {}", m + 1), 10.0, WORK_SANS, BLANCO, true, Al::Centro);
    }
    let fin = p.tabla(x, y, w, &cols, 18, 28.0);
    for f in 0..18 {
        let yy = y + 28.0 + 28.0 * f as f32;
        p.l.texto(x + w * 0.3 / total, yy + 7.0, &format!("{}", f + 1), 10.0, WORK_SANS, ROTULO, false, Al::Centro);
    }
    // La leyenda.
    let ly = fin + 18.0;
    p.l.rect(x, ly, 34.0, 12.0, None, Some(p.t.acento), 1.0, true);
    p.l.texto(x + 42.0, ly - 2.0, "Avance", 11.0, WORK_SANS, ROTULO, false, Al::Izq);
    p.l.rect(x + 120.0, ly, 34.0, 12.0, None, Some(p.t.suave), 1.0, true);
    p.l.texto(x + 162.0, ly - 2.0, "Planeado", 11.0, WORK_SANS, ROTULO, false, Al::Izq);
    let r = x + 260.0;
    p.l.poli(&[(r + 7.0, ly - 1.0), (r + 14.0, ly + 6.0), (r + 7.0, ly + 13.0), (r, ly + 6.0), (r + 7.0, ly - 1.0)], hex(0xd9822b), 1.5, Some(hex(0xf0a04b)));
    p.l.texto(r + 22.0, ly - 2.0, "Hito", 11.0, WORK_SANS, ROTULO, false, Al::Izq);
    p.fin("cronograma", "Cronograma")
}

fn diario() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Planificador diario", &["Fecha"]);
    p.dias_en_circulos(W - M - 9.0 - 172.0, 72.0);
    let izq = 330.0;
    let y = p.seccion(M, Y0, izq, "Horario");
    for (i, h) in (6..=22).enumerate() {
        let yy = y + 10.0 + i as f32 * 54.0;
        p.l.texto(M, yy + 4.0, &format!("{h:02}"), 11.0, WORK_SANS, p.t.acento, true, Al::Izq);
        p.l.linea(M + 30.0, yy + 18.0, M + izq, yy + 18.0, RAYA, 1.0);
        p.l.linea_con(M + 30.0, yy + 45.0, M + izq, yy + 45.0, RAYA, 1.0, EstiloTrazo::Punteado);
    }
    let x2 = M + izq + 30.0;
    let w2 = W - M - x2;
    let y = p.seccion(x2, Y0, w2, "Meta del día");
    p.caja_suave(x2, y + 10.0, w2, 70.0, "");
    let y = p.seccion(x2, y + 100.0, w2, "Prioridades");
    let y = p.numerados(x2, y, w2, 3, 34.0);
    let y = p.seccion(x2, y + 26.0, w2, "Otras tareas");
    let y = p.casillas(x2, y, w2, 10, 32.0, &[]);
    let y = p.seccion(x2, y + 26.0, w2, "Notas");
    p.caja(x2, y + 10.0, w2, H - M - y - 10.0, "");
    p.fin("planificador-diario", "Planificador diario")
}

fn semanal() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Planificador semanal", &["Semana", "Mes"]);
    let izq = 360.0;
    let y = p.seccion(M, Y0, izq, "Horario");
    let alto = (H - M - y - 12.0) / 7.0;
    for (i, d) in DIAS.iter().enumerate() {
        let yy = y + 12.0 + alto * i as f32;
        p.l.rect(M, yy, 38.0, alto - 8.0, None, Some(p.t.suave), 1.0, true);
        p.l.texto(M + 19.0, yy + 8.0, d, 10.0, WORK_SANS, p.t.acento, true, Al::Centro);
        p.l.rect(M, yy, izq, alto - 8.0, Some(BORDE), None, 1.25, true);
        p.renglones(M + 50.0, yy + 2.0, izq - 62.0, 3, (alto - 14.0) / 3.4);
    }
    let x2 = M + izq + 28.0;
    let w2 = W - M - x2;
    let y = p.seccion(x2, Y0, w2, "Objetivo de la semana");
    p.caja_suave(x2, y + 10.0, w2, 80.0, "");
    let y = p.seccion(x2, y + 110.0, w2, "Prioridades");
    let y = p.numerados(x2, y, w2, 4, 32.0);
    let y = p.seccion(x2, y + 24.0, w2, "Otras tareas");
    let y = p.casillas(x2, y, w2, 9, 30.0, &[]);
    let y = p.seccion(x2, y + 24.0, w2, "Hábitos");
    let celda = 22.0;
    let x0 = x2 + w2 - 7.0 * celda;
    for (i, d) in LETRAS.iter().enumerate() {
        p.l.texto(x0 + celda * i as f32 + celda / 2.0, y + 6.0, d, 9.0, WORK_SANS, ROTULO, true, Al::Centro);
    }
    for f in 0..6 {
        let yy = y + 26.0 + f as f32 * 28.0;
        p.l.linea(x2, yy + 20.0, x0 - 8.0, yy + 20.0, RAYA, 1.0);
        for i in 0..7 {
            p.l.circulo(x0 + celda * i as f32 + celda / 2.0, yy + 11.0, 7.5, Some(BORDE), None, 1.0);
        }
    }
    p.fin("planificador-semanal", "Planificador semanal")
}

fn mensual() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Planificador mensual", &["Mes", "Año"]);
    let y = Y0;
    let cw = ANCHO / 7.0;
    p.l.rect(M, y, ANCHO, 28.0, None, Some(p.t.acento), 1.0, true);
    for (i, d) in DIAS.iter().enumerate() {
        p.l.texto(M + cw * i as f32 + cw / 2.0, y + 7.0, d, 10.0, WORK_SANS, BLANCO, true, Al::Centro);
    }
    let fh = 96.0;
    let y1 = y + 36.0;
    for f in 0..6 {
        for c in 0..7 {
            let (cx, cy) = (M + cw * c as f32, y1 + fh * f as f32);
            let fondo = if c >= 5 { Some(hex(0xf6f8fc)) } else { None };
            p.l.rect(cx + 2.0, cy + 2.0, cw - 4.0, fh - 4.0, Some(RAYA), fondo, 1.0, true);
            p.l.circulo(cx + cw - 19.0, cy + 18.0, 8.0, Some(RAYA), None, 1.0);
        }
    }
    let y = y1 + fh * 6.0 + 22.0;
    let mitad = ANCHO / 2.0 - 14.0;
    let ya = p.seccion(M, y, mitad, "Objetivo del mes");
    p.renglones(M, ya, mitad, 2, 30.0);
    let yb = p.seccion(M, ya + 80.0, mitad, "Prioridades");
    p.numerados(M, yb, mitad, 4, 30.0);
    let x2 = M + mitad + 28.0;
    let yc = p.seccion(x2, y, mitad, "Tareas");
    p.casillas(x2, yc, mitad, 7, 30.0, &[]);
    p.fin("planificador-mensual", "Planificador mensual")
}

fn tareas() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Lista de tareas", &["Fecha"]);
    let y = p.seccion(M, Y0, ANCHO, "Lo más importante");
    p.caja_suave(M, y + 10.0, ANCHO, 116.0, "");
    p.numerados(M + 14.0, y + 12.0, ANCHO - 28.0, 3, 34.0);
    let y = y + 150.0;
    let mitad = ANCHO / 2.0 - 14.0;
    for (k, x) in [M, M + mitad + 28.0].into_iter().enumerate() {
        let ys = p.seccion(x, y, mitad, if k == 0 { "Pendientes" } else { "Más tareas" });
        for i in 1..=21 {
            let yy = ys + 34.0 * i as f32;
            p.l.rect(x + 2.0, yy - 17.0, 14.0, 14.0, Some(BORDE), None, 1.25, true);
            p.l.linea(x + 26.0, yy, x + mitad - 60.0, yy, RAYA, 1.0);
            p.l.texto(x + mitad - 26.0, yy - 17.0, "/", 11.0, WORK_SANS, RAYA, false, Al::Centro);
            p.l.linea(x + mitad - 50.0, yy, x + mitad, yy, RAYA, 1.0);
        }
    }
    p.fin("lista-de-tareas", "Lista de tareas")
}

fn metas() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Metas", &["Fecha límite"]);
    let mitad = ANCHO / 2.0 - 14.0;
    let y = p.seccion(M, Y0, mitad, "Mi meta");
    p.caja_suave(M, y + 10.0, mitad, 170.0, "");
    let y = p.seccion(M, y + 206.0, mitad, "¿Por qué?");
    let y = p.renglones(M, y, mitad, 6, 30.0);
    let y = p.seccion(M, y + 26.0, mitad, "Motivación");
    p.renglones(M, y, mitad, 6, 30.0);
    let x2 = M + mitad + 28.0;
    let y = p.seccion(x2, Y0, mitad, "Pasos");
    let y = p.numerados(x2, y, mitad, 12, 30.0);
    let y = p.seccion(x2, y + 26.0, mitad, "Hitos");
    p.tabla(x2, y + 10.0, mitad, &[("Hito", 4.0), ("Fecha", 1.6), ("✓", 0.7)], 6, 28.0);
    let y = H - M - 60.0;
    let y = p.seccion(M, y, ANCHO, "Progreso");
    let paso = ANCHO / 10.0;
    for i in 0..10 {
        p.l.rect(M + paso * i as f32 + 2.0, y + 12.0, paso - 4.0, 22.0, Some(BORDE), None, 1.0, true);
        p.l.texto(M + paso * (i as f32 + 0.5), y + 16.0, &format!("{}%", (i + 1) * 10), 9.0, WORK_SANS, ROTULO, false, Al::Centro);
    }
    p.fin("metas", "Metas")
}

fn habitos() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Hábitos del mes", &["Mes"]);
    let y = Y0 + 8.0;
    let nombre = 168.0;
    let celda = (ANCHO - nombre) / 31.0;
    p.l.texto(M, y + 4.0, "HÁBITO", 10.0, WORK_SANS, p.t.acento, true, Al::Izq);
    for d in 0..31 {
        p.l.texto(M + nombre + celda * (d as f32 + 0.5), y + 5.0, &format!("{}", d + 1), 8.5, WORK_SANS, ROTULO, true, Al::Centro);
    }
    let fh = 34.0;
    for f in 0..22 {
        let yy = y + 28.0 + fh * f as f32;
        if f % 2 == 0 {
            p.l.rect(M, yy, ANCHO, fh, None, Some(p.t.suave), 1.0, true);
        }
        for d in 0..31 {
            let cx = M + nombre + celda * (d as f32 + 0.5);
            p.l.rect(cx - 6.5, yy + fh / 2.0 - 6.5, 13.0, 13.0, Some(BORDE), Some(BLANCO), 1.0, true);
        }
    }
    let y = y + 28.0 + fh * 22.0 + 24.0;
    let y = p.seccion(M, y, ANCHO, "Notas del mes");
    p.renglones(M, y, ANCHO, 3, 30.0);
    p.fin("habitos", "Hábitos del mes")
}

fn comidas() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Comidas de la semana", &["Semana"]);
    let cols = [("", 0.9), ("Desayuno", 2.0), ("Almuerzo", 2.0), ("Cena", 2.0), ("Snack", 1.4)];
    let fh = 80.0;
    let fin = p.tabla(M, Y0, ANCHO, &cols, 7, fh);
    let total: f32 = cols.iter().map(|(_, f)| f).sum();
    for (i, d) in DIAS.iter().enumerate() {
        let yy = Y0 + 26.0 + fh * i as f32;
        p.l.rect(M, yy, ANCHO * 0.9 / total, fh, None, Some(p.t.suave), 1.0, false);
        p.l.texto(M + ANCHO * 0.45 / total, yy + fh / 2.0 - 8.0, d, 11.0, WORK_SANS, p.t.acento, true, Al::Centro);
    }
    let y = fin + 26.0;
    let tercio = (ANCHO - 56.0) / 3.0;
    let ys = p.seccion(M, y, tercio * 2.0 + 28.0, "Lista de compras");
    p.casillas(M, ys, tercio, 7, 30.0, &[]);
    p.casillas(M + tercio + 28.0, ys, tercio, 7, 30.0, &[]);
    let x3 = M + 2.0 * (tercio + 28.0);
    let yn = p.seccion(x3, y, tercio, "Notas");
    p.caja(x3, yn + 10.0, tercio, H - M - yn - 10.0, "");
    p.fin("comidas", "Comidas de la semana")
}

fn finanzas() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Finanzas del mes", &["Mes"]);
    let mitad = ANCHO / 2.0 - 14.0;
    let x2 = M + mitad + 28.0;
    let cols = [("Fecha", 1.0), ("Concepto", 2.6), ("Monto", 1.3)];
    let total = |p: &mut Pag, x: f32, y: f32| {
        p.l.rect(x, y, mitad, 28.0, None, Some(p.t.suave), 1.0, false);
        p.l.texto(x + 12.0, y + 7.0, "TOTAL", 10.0, WORK_SANS, p.t.acento, true, Al::Izq);
        y + 28.0
    };
    let y = p.seccion(M, Y0, mitad, "Gastos fijos");
    let y = p.tabla(M, y + 10.0, mitad, &cols, 10, 27.0);
    let y = total(&mut p, M, y);
    let y = p.seccion(M, y + 24.0, mitad, "Gastos variables");
    let y = p.tabla(M, y + 10.0, mitad, &cols, 16, 27.0);
    total(&mut p, M, y);
    let y = p.seccion(x2, Y0, mitad, "Ingresos");
    let y = p.tabla(x2, y + 10.0, mitad, &cols, 6, 27.0);
    let y = total(&mut p, x2, y);
    let y = p.seccion(x2, y + 24.0, mitad, "Ahorro");
    let y = p.tabla(x2, y + 10.0, mitad, &[("Para", 2.6), ("Monto", 1.3)], 4, 27.0);
    let y = total(&mut p, x2, y);
    let y = p.seccion(x2, y + 24.0, mitad, "Resumen");
    let filas = ["Ingresos", "Gastos fijos", "Gastos variables", "Ahorro", "Saldo"];
    let fin = p.tabla(x2, y + 10.0, mitad, &[("", 2.6), ("Monto", 1.3)], filas.len(), 30.0);
    for (i, s) in filas.iter().enumerate() {
        let yy = fin - 30.0 * (filas.len() - i) as f32;
        p.l.texto(x2 + 12.0, yy + 8.0, s, 11.0, WORK_SANS, if i == 4 { p.t.acento } else { ROTULO }, i == 4, Al::Izq);
    }
    p.fin("finanzas", "Finanzas del mes")
}

fn itinerario() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Itinerario", &["Destino"]);
    let mut y = Y0;
    for _ in 0..3 {
        p.l.texto(M, y, "DÍA", 11.0, WORK_SANS, p.t.acento, true, Al::Izq);
        p.l.linea(M + 34.0, y + 16.0, M + 200.0, y + 16.0, BORDE, 1.0);
        y = p.tabla(M, y + 28.0, ANCHO, &[("Hora", 1.0), ("Lugar", 2.4), ("Detalles", 3.2)], 7, 30.0) + 30.0;
    }
    p.fin("itinerario", "Itinerario")
}

fn viaje() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Planificador de viaje", &["Destino", "Fechas"]);
    let mitad = ANCHO / 2.0 - 14.0;
    let x2 = M + mitad + 28.0;
    let ficha = |p: &mut Pag, x: f32, y: f32, filas: &[&str]| {
        let fin = p.tabla(x, y, mitad, &[("", 1.4), ("", 2.2)], filas.len(), 30.0);
        for (i, s) in filas.iter().enumerate() {
            let yy = y + 30.0 * i as f32;
            p.l.rect(x, yy, mitad * 1.4 / 3.6, 30.0, None, Some(p.t.suave), 1.0, false);
            p.l.texto(x + 10.0, yy + 9.0, &s.to_uppercase(), 9.5, WORK_SANS, p.t.acento, true, Al::Izq);
        }
        p.l.rect(x, y, mitad, fin - y, Some(BORDE), None, 1.25, false);
        fin
    };
    let y = p.seccion(M, Y0, mitad, "Vuelo");
    let fin_izq = ficha(&mut p, M, y + 10.0, &["Aerolínea", "Reserva", "N.º de vuelo", "Salida", "Llegada", "N.º de vuelo", "Salida", "Llegada"]);
    let y = p.seccion(x2, Y0, mitad, "Alojamiento");
    let y = ficha(&mut p, x2, y + 10.0, &["Nombre", "Dirección", "Fechas", "Entrada / salida"]);
    let y = p.seccion(x2, y + 22.0, mitad, "Transporte");
    let fin_der = ficha(&mut p, x2, y + 10.0, &["Empresa", "Reserva", "Recoger / devolver"]);
    let y = fin_izq.max(fin_der) + 30.0;
    let ya = p.seccion(M, y, mitad, "Equipaje");
    p.casillas(M, ya, mitad, 16, 30.0, &[]);
    let yb = p.seccion(x2, y, mitad, "Antes de salir");
    p.casillas(x2, yb, mitad, 16, 30.0, &[]);
    p.fin("viaje", "Planificador de viaje")
}

fn diario_de_viaje() -> FiguraGuardada {
    let mut p = Pag::nueva(PLANIFICACION, "Diario de viaje", &["Lugar", "Fechas"]);
    let alto_foto = 380.0;
    p.caja_suave(M, Y0, ANCHO, alto_foto, "Foto o dibujo");
    let y = Y0 + alto_foto + 20.0;
    let paso = 22.0;
    let (cols, filas) = ((ANCHO / paso) as usize, ((H - M - y) / paso) as usize);
    for c in 0..=cols {
        let x = M + paso * c as f32;
        p.l.linea(x, y, x, y + paso * filas as f32, hex(0xe1e7f0), 1.0);
    }
    for f in 0..=filas {
        let yy = y + paso * f as f32;
        p.l.linea(M, yy, M + paso * cols as f32, yy, hex(0xe1e7f0), 1.0);
    }
    p.fin("diario-de-viaje", "Diario de viaje")
}

// ----------------------------------------------------------------- academico

fn semestre() -> FiguraGuardada {
    let mut p = Pag::nueva(ACADEMICO, "Horario del semestre", &["Inicio", "Fin"]);
    let mut cols = vec![("", 0.55)];
    cols.extend(DIAS.iter().map(|d| (*d, 1.0)));
    let fh = 38.0;
    let horas = 16;
    let fin = p.tabla(M, Y0, ANCHO, &cols, horas, fh);
    for h in 0..horas {
        let yy = Y0 + 26.0 + fh * h as f32;
        p.l.texto(M + 18.0, yy + 4.0, &format!("{:02}", 7 + h), 10.0, WORK_SANS, p.t.acento, true, Al::Centro);
    }
    let y = p.seccion(M, fin + 26.0, ANCHO, "Mis cursos");
    let mitad = ANCHO / 2.0 - 12.0;
    for i in 0..6 {
        let (x, yy) = (M + (mitad + 24.0) * (i % 2) as f32, y + 12.0 + 62.0 * (i / 2) as f32);
        p.l.rect(x, yy, mitad, 52.0, Some(BORDE), None, 1.25, true);
        p.l.rect(x, yy, 8.0, 52.0, None, Some(p.t.acento), 1.0, false);
        p.l.texto(x + 18.0, yy + 6.0, &format!("Curso {}:", i + 1), 11.0, WORK_SANS, ROTULO, true, Al::Izq);
        p.l.linea(x + 18.0, yy + 26.0, x + mitad - 12.0, yy + 26.0, RAYA, 1.0);
        p.l.texto(x + 18.0, yy + 30.0, "Aula:", 10.0, WORK_SANS, ROTULO, false, Al::Izq);
    }
    p.fin("semestre", "Horario del semestre")
}

fn curso() -> FiguraGuardada {
    let mut p = Pag::nueva(ACADEMICO, "Curso", &["Ciclo"]);
    let mitad = ANCHO / 2.0 - 14.0;
    let x2 = M + mitad + 28.0;
    let y = p.seccion(M, Y0, mitad, "Datos");
    let y = p.datos(M, y, mitad, &["Curso:", "Docente:", "Horario:", "Aula:", "Atención:", "Contacto:"], 32.0);
    let y = p.seccion(M, y + 24.0, mitad, "Libros y materiales");
    let y = p.tabla(M, y + 6.0, mitad, &[("", 5.0), ("", 1.0)], 7, 30.0);
    let y = p.seccion(M, y + 24.0, mitad, "Fechas importantes");
    p.tabla(M, y + 6.0, mitad, &[("Fecha", 1.0), ("Qué", 3.0)], 8, 30.0);
    let y = p.seccion(x2, Y0, mitad, "Entregas");
    let y = p.tabla(x2, y + 6.0, mitad, &[("Fecha", 1.0), ("Trabajo", 3.2), ("✓", 0.6)], 18, 30.0);
    let y = p.seccion(x2, y + 24.0, mitad, "Notas");
    p.caja(x2, y + 10.0, mitad, H - M - y - 10.0, "");
    p.fin("curso", "Curso")
}

fn estudio() -> FiguraGuardada {
    let mut p = Pag::nueva(ACADEMICO, "Sesión de estudio", &["Fecha"]);
    let mitad = ANCHO / 2.0 - 14.0;
    let x2 = M + mitad + 28.0;
    let y = p.seccion(M, Y0, mitad, "Tiempo");
    let y = p.datos(M, y, mitad, &["Inicio:", "Fin:", "Total:"], 30.0);
    let y = p.seccion(M, y + 24.0, mitad, "Objetivos");
    let y = p.numerados(M, y, mitad, 4, 30.0);
    let y = p.seccion(M, y + 24.0, mitad, "Temas / capítulos");
    let y = p.renglones(M, y, mitad, 10, 30.0);
    let y = p.seccion(M, y + 24.0, mitad, "Pausas");
    p.renglones(M, y, mitad, 4, 30.0);
    let y = p.seccion(x2, Y0, mitad, "Antes de empezar");
    let y = p.casillas(x2, y, mitad, 6, 28.0, &["Materiales listos", "Lugar tranquilo", "Sin distracciones", "Música de fondo", "Agua y algo de comer", "Meta clara"]);
    let y = p.seccion(x2, y + 24.0, mitad, "Bloques de tiempo");
    let fin = p.tabla(x2, y + 8.0, mitad, &[("", 1.3), ("15", 1.0), ("30", 1.0), ("45", 1.0), ("60", 1.0)], 8, 28.0);
    for i in 0..8 {
        p.l.texto(x2 + 10.0, y + 8.0 + 26.0 + 28.0 * i as f32 + 7.0, &format!("Hora {}", i + 1), 10.0, WORK_SANS, ROTULO, false, Al::Izq);
    }
    let y = p.seccion(x2, fin + 24.0, mitad, "Notas");
    let alto = H - M - y - 10.0;
    p.caja(x2, y + 10.0, mitad, alto, "");
    let (cx, cy) = (x2 + mitad - 56.0, y + 10.0 + alto - 56.0);
    p.l.circulo(cx, cy, 44.0, Some(p.t.acento), Some(p.t.suave), 1.5);
    p.l.texto(cx, cy - 8.0, "MI PREMIO", 10.0, WORK_SANS, p.t.acento, true, Al::Centro);
    p.fin("sesion-de-estudio", "Sesión de estudio")
}

fn entrega() -> FiguraGuardada {
    let mut p = Pag::nueva(ACADEMICO, "Planificador de tareas", &["Curso"]);
    let izq = 300.0;
    let y = p.seccion(M, Y0, izq, "Detalles");
    let y = p.datos(M, y, izq, &["Tarea:", "Inicio:", "Entrega:"], 32.0);
    let y = p.seccion(M, y + 24.0, izq, "Recursos");
    let y = p.renglones(M, y, izq, 5, 30.0);
    let y = p.seccion(M, y + 24.0, izq, "Pasos");
    p.casillas(M, y, izq, 15, 30.0, &[]);
    let x2 = M + izq + 28.0;
    let w2 = W - M - x2;
    let y = p.seccion(x2, Y0, w2, "Espacio libre (bocetos, notas, ideas)");
    let alto = H - M - y - 10.0;
    p.l.rect(x2, y + 10.0, w2, alto, Some(BORDE), None, 1.25, true);
    let paso = 24.0;
    let (nc, nf) = ((w2 / paso) as usize, (alto / paso) as usize);
    let (ox, oy) = (x2 + (w2 - paso * (nc - 1) as f32) / 2.0, y + 10.0 + (alto - paso * (nf - 1) as f32) / 2.0);
    for c in 0..nc {
        for f in 0..nf {
            p.l.circulo(ox + paso * c as f32, oy + paso * f as f32, 1.3, None, Some(hex(0xb7c2d2)), 1.0);
        }
    }
    p.fin("planificador-de-tareas", "Planificador de tareas")
}

fn mapa_mental() -> FiguraGuardada {
    let mut p = Pag::apaisada(ACADEMICO, "Mapa mental", &["Tema"]);
    let (cx, cy) = (H / 2.0, 120.0 + (W - 120.0) / 2.0);
    let acento = p.t.acento;
    // Las hojas de los bordes y las ramas.
    let tarjeta = |p: &mut Pag, x: f32, y: f32| {
        p.l.rect(x, y, 150.0, 86.0, Some(BORDE), Some(BLANCO), 1.25, true);
        p.l.rect(x, y, 150.0, 22.0, None, Some(p.t.suave), 1.0, true);
    };
    let sub = [(cx - 250.0, cy - 150.0), (cx + 100.0, cy - 150.0), (cx - 250.0, cy + 110.0), (cx + 100.0, cy + 110.0)];
    let hojas = [
        [(M, 140.0), (M, 270.0)],
        [(H - M - 150.0, 140.0), (H - M - 150.0, 270.0)],
        [(M, W - M - 216.0), (M, W - M - 86.0)],
        [(H - M - 150.0, W - M - 216.0), (H - M - 150.0, W - M - 86.0)],
    ];
    for (k, (sx, sy)) in sub.iter().enumerate() {
        let (scx, scy) = (sx + 75.0, sy + 20.0);
        p.l.poli(&[(cx, cy), (cx, scy), (scx, scy)], acento, 1.5, None);
        for (hx, hy) in hojas[k] {
            let (ex, ey) = if hx < cx { (hx + 150.0, hy + 43.0) } else { (hx, hy + 43.0) };
            let medio = if hx < cx { ex + 30.0 } else { ex - 30.0 };
            p.l.poli(&[(if hx < cx { *sx } else { sx + 150.0 }, scy), (medio, scy), (medio, ey), (ex, ey)], BORDE, 1.25, None);
            tarjeta(&mut p, hx, hy);
        }
    }
    for (sx, sy) in sub {
        p.l.rect(sx, sy, 150.0, 40.0, Some(acento), Some(p.t.suave), 1.5, true);
        p.l.texto(sx + 75.0, sy + 12.0, "SUBTEMA", 10.0, WORK_SANS, acento, true, Al::Centro);
    }
    p.l.rect(cx - 110.0, cy - 34.0, 220.0, 68.0, None, Some(acento), 1.0, true);
    p.l.texto(cx, cy - 9.0, "TEMA CENTRAL", 13.0, WORK_SANS, BLANCO, true, Al::Centro);
    p.fin("mapa-mental", "Mapa mental")
}

// ------------------------------------------------------------------ creativo

fn storyboard(n: usize) -> FiguraGuardada {
    let mut p = Pag::nueva(CREATIVO, "Storyboard", &["Proyecto", "Página"]);
    let acento = p.t.acento;
    let numero = |p: &mut Pag, x: f32, y: f32, i: usize| {
        p.l.circulo(x + 16.0, y + 16.0, 12.0, None, Some(acento), 1.0);
        p.l.texto(x + 16.0, y + 8.5, &format!("{i}"), 11.0, WORK_SANS, BLANCO, true, Al::Centro);
    };
    if n == 4 {
        let (fw, fh) = (340.0, 191.0);
        for i in 0..4 {
            let y = Y0 + 8.0 + i as f32 * (fh + 34.0);
            p.l.rect(M, y, fw, fh, Some(BORDE), Some(hex(0xfcfcfd)), 1.5, false);
            numero(&mut p, M + 8.0, y + 8.0, i + 1);
            let x2 = M + fw + 26.0;
            let w2 = W - M - x2;
            p.l.texto(x2, y, "ESCENA", 10.0, WORK_SANS, acento, true, Al::Izq);
            p.l.linea(x2 + 52.0, y + 15.0, x2 + 120.0, y + 15.0, BORDE, 1.0);
            p.l.texto(x2 + 140.0, y, "TOMA", 10.0, WORK_SANS, acento, true, Al::Izq);
            p.l.linea(x2 + 180.0, y + 15.0, x2 + w2, y + 15.0, BORDE, 1.0);
            p.renglones(x2, y + 18.0, w2, 6, 28.0);
        }
    } else {
        let fw = (ANCHO - 28.0) / 2.0;
        let fh = fw * 9.0 / 16.0;
        for i in 0..6 {
            let (x, y) = (M + (fw + 28.0) * (i % 2) as f32, Y0 + 8.0 + (fh + 110.0) * (i / 2) as f32);
            p.l.rect(x, y, fw, fh, Some(BORDE), Some(hex(0xfcfcfd)), 1.5, false);
            numero(&mut p, x + 8.0, y + 8.0, i + 1);
            p.renglones(x, y + fh, fw, 3, 28.0);
        }
    }
    p.fin(&format!("storyboard-{n}"), &format!("Storyboard de {n} viñetas"))
}

fn receta() -> FiguraGuardada {
    let mut p = Pag::nueva(CREATIVO, "Receta", &["Categoría"]);
    let mitad = ANCHO / 2.0 - 14.0;
    let x2 = M + mitad + 28.0;
    p.caja_suave(M, Y0, mitad, 230.0, "Foto");
    let y = p.datos(x2, Y0 - 6.0, mitad, &["Nombre:"], 34.0);
    let y = p.seccion(x2, y + 22.0, mitad, "Detalles");
    let fin = p.tabla(x2, y + 8.0, mitad, &[("", 1.4), ("", 2.0)], 3, 32.0);
    for (i, s) in ["Preparación", "Cocción", "Porciones"].iter().enumerate() {
        let yy = y + 8.0 + 32.0 * i as f32;
        p.l.rect(x2, yy, mitad * 1.4 / 3.4, 32.0, None, Some(p.t.suave), 1.0, false);
        p.l.texto(x2 + 10.0, yy + 10.0, &s.to_uppercase(), 9.5, WORK_SANS, p.t.acento, true, Al::Izq);
    }
    p.l.rect(x2, y + 8.0, mitad, fin - y - 8.0, Some(BORDE), None, 1.25, false);
    let y = Y0 + 260.0;
    let ya = p.seccion(M, y, mitad, "Ingredientes");
    p.casillas(M, ya, mitad, 16, 30.0, &[]);
    let yb = p.seccion(x2, y, mitad, "Preparación");
    p.numerados(x2, yb, mitad, 16, 30.0);
    let y = ya + 16.0 * 30.0 + 26.0;
    let y = p.seccion(M, y, ANCHO, "Notas");
    p.caja(M, y + 10.0, ANCHO, H - M - y - 10.0, "");
    p.fin("receta", "Receta")
}

fn pentagramas() -> FiguraGuardada {
    let mut p = Pag::nueva(CREATIVO, "Partitura", &["Título", "Autor"]);
    let mut y = Y0 + 20.0;
    while y + 40.0 < H - M {
        for k in 0..5 {
            let yy = y + 9.0 * k as f32;
            p.l.linea(M, yy, W - M, yy, hex(0x8f9bb0), 1.0);
        }
        p.l.linea(M, y, M, y + 36.0, hex(0x8f9bb0), 1.5);
        p.l.linea(W - M, y, W - M, y + 36.0, hex(0x8f9bb0), 1.5);
        y += 82.0;
    }
    p.fin("pentagramas", "Partitura")
}

// -------------------------------------------------------------------- diario

/// La fila del animo: cinco caritas para marcar una.
fn animo(p: &mut Pag, x: f32, y: f32) {
    for i in 0..5 {
        let cx = x + i as f32 * 30.0;
        p.l.circulo(cx, y, 11.0, Some(p.t.acento), None, 1.25);
        p.l.circulo(cx - 4.0, y - 3.0, 1.2, None, Some(p.t.acento), 1.0);
        p.l.circulo(cx + 4.0, y - 3.0, 1.2, None, Some(p.t.acento), 1.0);
        let curva = [-4.0, -2.0, 0.0, 2.0, 4.0][i];
        p.l.poli(&[(cx - 5.0, y + 4.0), (cx, y + 4.0 + curva * 0.8), (cx + 5.0, y + 4.0)], p.t.acento, 1.25, None);
    }
}

fn diario_libre() -> FiguraGuardada {
    let mut p = Pag::nueva(DIARIO, "Diario", &["Fecha"]);
    p.l.texto(M, Y0, "¿CÓMO ME SIENTO?", 10.0, WORK_SANS, p.t.acento, true, Al::Izq);
    animo(&mut p, M + 150.0, Y0 + 7.0);
    p.renglones(M, Y0 + 24.0, ANCHO, 30, 31.0);
    p.fin("diario", "Diario")
}

fn autocuidado() -> FiguraGuardada {
    let mut p = Pag::nueva(DIARIO, "Autocuidado", &["Fecha"]);
    let acento = p.t.acento;
    let cuarto = (ANCHO - 36.0) / 4.0;
    for (i, s) in ["Ánimo", "Clima", "Comidas", "Agua"].iter().enumerate() {
        let x = M + (cuarto + 12.0) * i as f32;
        p.caja_suave(x, Y0, cuarto, 74.0, s);
        let y = Y0 + 50.0;
        match i {
            0 => animo(&mut p, x + 24.0, y),
            1 => {
                p.l.circulo(x + 26.0, y, 9.0, Some(acento), None, 1.25);
                for k in 0..8 {
                    let a = (k as f32 * 45.0).to_radians();
                    p.l.linea(x + 26.0 + 12.0 * a.cos(), y + 12.0 * a.sin(), x + 26.0 + 16.0 * a.cos(), y + 16.0 * a.sin(), acento, 1.25);
                }
                p.l.arco(x + 70.0, y + 2.0, 9.0, 180.0, 360.0, acento, 1.25);
                p.l.arco(x + 82.0, y - 2.0, 11.0, 200.0, 360.0, acento, 1.25);
                p.l.linea(x + 61.0, y + 2.0, x + 93.0, y + 2.0, acento, 1.25);
                for k in 0..3 {
                    let dx = x + 118.0 + k as f32 * 8.0;
                    p.l.linea(dx, y - 6.0, dx - 4.0, y + 6.0, acento, 1.25);
                }
            }
            2 => {
                for k in 0..3 {
                    p.l.circulo(x + 26.0 + k as f32 * 34.0, y, 11.0, Some(acento), None, 1.25);
                }
            }
            _ => {
                for k in 0..8 {
                    let dx = x + 18.0 + k as f32 * 18.0;
                    p.l.poli(&[(dx, y - 10.0), (dx + 6.0, y), (dx + 4.0, y + 6.0), (dx, y + 8.0), (dx - 4.0, y + 6.0), (dx - 6.0, y), (dx, y - 10.0)], acento, 1.25, None);
                }
            }
        }
    }
    let mut y = Y0 + 104.0;
    let preguntas: [(&str, usize); 9] = [
        ("¿Hablé con alguien hoy?", 0),
        ("¿Cuánta energía social tengo?", 0),
        ("¿Cuánto tiempo pasé con el móvil?", 0),
        ("¿Hice ejercicio? ¿Cuánto?", 0),
        ("¿Dormí una siesta?", 0),
        ("¿Comí bien hoy?", 0),
        ("¿Qué comí?", 2),
        ("¿Cómo se siente mi cuerpo?", 3),
        ("¿Tomé mis medicinas?", 0),
    ];
    for (s, extra) in preguntas {
        p.l.circulo(M + 5.0, y + 8.0, 4.0, None, Some(acento), 1.0);
        p.l.texto(M + 16.0, y, s, 12.0, WORK_SANS, hex(0x4a3f48), true, Al::Izq);
        p.l.texto(W - M - 70.0, y + 1.0, "SÍ    NO", 10.0, WORK_SANS, acento, true, Al::Izq);
        p.l.linea(M + 16.0, y + 26.0, W - M - 90.0, y + 26.0, RAYA, 1.0);
        y += 40.0;
        if extra > 0 {
            y = p.renglones(M + 16.0, y - 14.0, ANCHO - 16.0, extra, 30.0) + 18.0;
        }
    }
    let y = p.seccion(M, y + 6.0, ANCHO, "Medicinas");
    p.tabla(M, y + 10.0, ANCHO, &[("", 1.0); 8], 2, 34.0);
    p.fin("autocuidado", "Autocuidado")
}

fn gratitud() -> FiguraGuardada {
    let mut p = Pag::nueva(DIARIO, "Gratitud", &["Fecha"]);
    let izq = 400.0;
    let x2 = M + izq + 24.0;
    let w2 = W - M - x2;
    let alto = (H - M - Y0 - 2.0 * 20.0) / 3.0;
    for (i, (s, d)) in [
        ("Agradezco", "Dibuja lo que te hace feliz"),
        ("Mis logros", "Dibuja un logro"),
        ("Lo mejor del día", "Dibuja algo que te hizo sonreír"),
    ]
    .iter()
    .enumerate()
    {
        let y = Y0 + (alto + 20.0) * i as f32;
        let ys = p.seccion(M, y, izq, s);
        p.renglones(M, ys, izq, ((alto - 30.0) / 29.0) as usize, 29.0);
        p.caja(x2, y, w2, alto, "");
        p.l.texto(x2 + w2 / 2.0, y + 10.0, d, 15.0, CAVEAT, p.t.acento, true, Al::Centro);
    }
    p.fin("gratitud", "Gratitud")
}

fn metas_del_dia() -> FiguraGuardada {
    let mut p = Pag::nueva(DIARIO, "Metas del día", &["Fecha"]);
    let alto = (H - M - Y0 - 2.0 * 18.0) / 3.0;
    for (i, s) in ["Intención de la mañana", "Afirmaciones del mediodía", "Reflexión de la noche"].iter().enumerate() {
        let y = Y0 + (alto + 18.0) * i as f32;
        p.l.rect(M, y, ANCHO, alto, Some(BORDE), Some(BLANCO), 1.25, true);
        p.l.rect(M, y, ANCHO, 34.0, None, Some(p.t.suave), 1.0, true);
        p.l.circulo(M + 20.0, y + 17.0, 7.0, None, Some(p.t.acento), 1.0);
        p.l.texto(M + 36.0, y + 9.0, &s.to_uppercase(), 11.0, WORK_SANS, p.t.acento, true, Al::Izq);
    }
    p.fin("metas-del-dia", "Metas del día")
}

fn sueno() -> FiguraGuardada {
    let mut p = Pag::nueva(DIARIO, "Sueño", &["Fecha"]);
    let acento = p.t.acento;
    let izq = ANCHO - 90.0;
    let y = p.datos(M, Y0 - 6.0, izq / 2.0 - 12.0, &["Me acosté:"], 32.0);
    p.datos(M + izq / 2.0 + 12.0, Y0 - 6.0, izq / 2.0 - 12.0, &["Me levanté:"], 32.0);
    let mut y = y + 20.0;
    let preguntas: [(&str, usize); 9] = [
        ("¿Seguí mi horario de siempre?", 0),
        ("¿Cómo dormí?", 0),
        ("¿Cómo fue el lugar? (ruido, luz, temperatura)", 2),
        ("¿Miré el móvil antes de dormir?", 0),
        ("¿Comí algo antes de acostarme?", 0),
        ("¿Me desperté en la noche? ¿Cuántas veces?", 0),
        ("¿Soñé? ¿Con qué?", 4),
        ("¿Me sentí descansado al despertar?", 0),
        ("Notas", 4),
    ];
    for (s, extra) in preguntas {
        p.l.poli(&[(M + 5.0, y + 3.0), (M + 9.0, y + 8.0), (M + 5.0, y + 13.0), (M + 1.0, y + 8.0), (M + 5.0, y + 3.0)], acento, 1.0, Some(acento));
        p.l.texto(M + 16.0, y, s, 12.0, WORK_SANS, hex(0x4a3f48), true, Al::Izq);
        p.l.linea(M + 16.0, y + 26.0, M + izq, y + 26.0, RAYA, 1.0);
        y += 40.0;
        if extra > 0 {
            y = p.renglones(M + 16.0, y - 14.0, izq - 16.0, extra, 30.0) + 18.0;
        }
    }
    // Las horas dormidas, en una columna a la derecha.
    let x = W - M - 40.0;
    p.l.rect(x - 10.0, Y0 - 6.0, 60.0, 500.0, None, Some(p.t.suave), 1.0, true);
    p.l.texto(x + 20.0, Y0 + 2.0, "HORAS", 9.5, WORK_SANS, acento, true, Al::Centro);
    for h in 0..12 {
        let cy = Y0 + 38.0 + h as f32 * 37.0;
        p.l.circulo(x + 20.0, cy, 12.0, Some(acento), Some(BLANCO), 1.25);
        p.l.texto(x + 20.0, cy - 7.0, &format!("{}", h + 1), 10.0, WORK_SANS, acento, true, Al::Centro);
    }
    p.fin("sueno", "Sueño")
}
