//! Lo del visor de hojas que no pinta ni toca Windows: que se abre, la
//! seleccion y las teclas, el zoom, el desplazamiento y sus barras, copiar
//! como texto con tabuladores, los enlaces y la configuracion regional.
//! Todo con pruebas.

use std::borrow::Cow;
use std::path::Path;

use pixpin_hojas::{Rango, Regional};

// ---------------------------------------------------------------------------
// Que se abre

/// Las extensiones de los libros que lee `pixpin-hojas`: los de ahora
/// (`.xlsx`...) y los de Excel 97-2003 (`.xls`, `.xlt`), que se leen sin
/// Office. El `.xlsb` es binario de otra clase: sigue yendo a su programa.
pub fn se_abre(nombre: &str) -> bool {
    matches!(
        pixpin_docs::extension(nombre).as_str(),
        "xlsx" | "xlsm" | "xltx" | "xltm" | "xls" | "xlt"
    )
}

/// Como [`se_abre`], mirando tambien dentro del fichero: un ZIP (un libro de
/// ahora) o un `.xls` que el lector de `pixpin-hojas` abre de verdad. Un
/// fichero roto, cifrado o de Excel 5 se queda con su camino de antes (la
/// vista previa de Office); y un `.xlsx` que en realidad es un `.xls`
/// renombrado se abre aqui si se entiende.
pub fn se_abre_fichero(ruta: &Path) -> bool {
    se_abre(&pixpin_docs::nombre(ruta)) && (empieza_como_zip(ruta) || xls_legible(ruta))
}

fn empieza_como_zip(ruta: &Path) -> bool {
    use std::io::Read;
    let mut cabeza = [0u8; 4];
    std::fs::File::open(ruta)
        .and_then(|mut f| f.read_exact(&mut cabeza))
        .is_ok()
        && cabeza == *b"PK\x03\x04"
}

/// Si el fichero es un `.xls` BIFF8 que `pixpin_hojas` abre bien. Lee y
/// entiende el libro de verdad (globales y hoja activa), asi que se acuerda
/// del ultimo resultado por ruta, tamano y fecha: quien pregunta varias
/// veces seguidas no lo repite.
fn xls_legible(ruta: &Path) -> bool {
    use std::io::Read;
    static ULTIMO: std::sync::Mutex<
        Option<(std::path::PathBuf, u64, Option<std::time::SystemTime>, bool)>,
    > = std::sync::Mutex::new(None);
    let Ok(meta) = std::fs::metadata(ruta) else {
        return false;
    };
    // Una cabecera de contenedor compuesto: si no, ni se lee.
    let mut cabeza = [0u8; 8];
    if std::fs::File::open(ruta)
        .and_then(|mut f| f.read_exact(&mut cabeza))
        .is_err()
        || !pixpin_hojas::cfb::es_cfb(&cabeza)
    {
        return false;
    }
    let clave = (ruta.to_path_buf(), meta.len(), meta.modified().ok());
    if let Ok(g) = ULTIMO.lock() {
        if let Some((r, n, t, ok)) = g.as_ref() {
            if (r, *n, *t) == (&clave.0, clave.1, clave.2) {
                return *ok;
            }
        }
    }
    let ok = std::fs::read(ruta)
        .ok()
        .is_some_and(|b| pixpin_hojas::xls::XlsAbierto::abrir(&b).is_ok());
    if let Ok(mut g) = ULTIMO.lock() {
        *g = Some((clave.0, clave.1, clave.2, ok));
    }
    ok
}

// ---------------------------------------------------------------------------
// Seleccion y teclas

/// La seleccion: la celda de donde se empezo (ancla) y la activa (la que
/// se mueve con Mayus+flechas). Fila y columna, base 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Seleccion {
    pub ancla: (u32, u32),
    pub activa: (u32, u32),
}

impl Seleccion {
    pub fn en(fila: u32, col: u32) -> Seleccion {
        Seleccion {
            ancla: (fila, col),
            activa: (fila, col),
        }
    }

    pub fn rango(&self) -> Rango {
        let (a, b) = (self.ancla, self.activa);
        Rango {
            fila0: a.0.min(b.0),
            col0: a.1.min(b.1),
            fila1: a.0.max(b.0),
            col1: a.1.max(b.1),
        }
    }
}

/// Topes de Excel.
pub const ULTIMA_FILA: u32 = 1_048_575;
pub const ULTIMA_COLUMNA: u32 = 16_383;

/// A donde se mueve la celda activa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mov {
    Arriba,
    Abajo,
    Izquierda,
    Derecha,
    /// Ctrl+flecha: al borde del bloque con datos, como Excel.
    BordeArriba,
    BordeAbajo,
    BordeIzquierda,
    BordeDerecha,
    RePag,
    AvPag,
    /// Inicio: a la columna A (o a la primera que se desplaza).
    Inicio,
    /// Ctrl+Inicio: a la primera celda que se desplaza.
    InicioHoja,
    /// Ctrl+Fin: a la ultima celda usada.
    FinHoja,
}

/// Lo que hace una tecla en el visor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccionTecla {
    /// Mover la celda activa; `extender` con Mayus (crece la seleccion).
    Mover(Mov, bool),
    HojaAnterior,
    HojaSiguiente,
    Copiar,
    SeleccionarTodo,
    ZoomInicial,
    Acercar,
    Alejar,
    PantallaCompleta,
    /// Abrir el enlace de la celda activa (Ctrl+Intro).
    AbrirEnlace,
    Escape,
}

pub const VK_INTRO: u32 = 0x0D;
pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_REPAG: u32 = 0x21;
pub const VK_AVPAG: u32 = 0x22;
pub const VK_FIN: u32 = 0x23;
pub const VK_INICIO: u32 = 0x24;
pub const VK_IZQUIERDA: u32 = 0x25;
pub const VK_ARRIBA: u32 = 0x26;
pub const VK_DERECHA: u32 = 0x27;
pub const VK_ABAJO: u32 = 0x28;
pub const VK_0: u32 = 0x30;
pub const VK_A: u32 = 0x41;
pub const VK_C: u32 = 0x43;
pub const VK_TAB: u32 = 0x09;
pub const VK_F11: u32 = 0x7A;
pub const VK_MAS: u32 = 0xBB;
pub const VK_MENOS: u32 = 0xBD;
pub const VK_MAS_NUM: u32 = 0x6B;
pub const VK_MENOS_NUM: u32 = 0x6D;
pub const VK_0_NUM: u32 = 0x60;

/// La tecla pulsada, en lo que hace. `None`: no es del visor.
pub fn accion_de_tecla(vk: u32, shift: bool, ctrl: bool, alt: bool) -> Option<AccionTecla> {
    use AccionTecla as A;
    if alt {
        return None;
    }
    let mov =
        |normal: Mov, con_ctrl: Mov| Some(A::Mover(if ctrl { con_ctrl } else { normal }, shift));
    match vk {
        VK_ARRIBA => mov(Mov::Arriba, Mov::BordeArriba),
        VK_ABAJO => mov(Mov::Abajo, Mov::BordeAbajo),
        VK_IZQUIERDA => mov(Mov::Izquierda, Mov::BordeIzquierda),
        VK_DERECHA => mov(Mov::Derecha, Mov::BordeDerecha),
        VK_TAB if !ctrl => Some(A::Mover(
            if shift { Mov::Izquierda } else { Mov::Derecha },
            false,
        )),
        VK_INTRO if ctrl => Some(A::AbrirEnlace),
        VK_INTRO => Some(A::Mover(
            if shift { Mov::Arriba } else { Mov::Abajo },
            false,
        )),
        VK_REPAG if ctrl => Some(A::HojaAnterior),
        VK_AVPAG if ctrl => Some(A::HojaSiguiente),
        VK_REPAG => Some(A::Mover(Mov::RePag, shift)),
        VK_AVPAG => Some(A::Mover(Mov::AvPag, shift)),
        VK_INICIO => mov(Mov::Inicio, Mov::InicioHoja),
        VK_FIN if ctrl => Some(A::Mover(Mov::FinHoja, shift)),
        VK_C if ctrl => Some(A::Copiar),
        VK_A if ctrl => Some(A::SeleccionarTodo),
        VK_0 | VK_0_NUM if ctrl => Some(A::ZoomInicial),
        VK_MAS | VK_MAS_NUM if ctrl => Some(A::Acercar),
        VK_MENOS | VK_MENOS_NUM if ctrl => Some(A::Alejar),
        VK_F11 => Some(A::PantallaCompleta),
        VK_ESCAPE => Some(A::Escape),
        _ => None,
    }
}

/// Lo que hace falta saber de la hoja para mover la seleccion.
pub struct Marco<'a> {
    /// La ultima fila y columna usadas.
    pub ultima: (u32, u32),
    /// Filas y columnas inmovilizadas.
    pub fijas: (u32, u32),
    /// Cuantas filas entran en una pantalla (RePag/AvPag).
    pub filas_por_pagina: u32,
    /// Si la celda tiene algo (para Ctrl+flecha).
    pub lleno: &'a dyn Fn(u32, u32) -> bool,
}

/// Ctrl+flecha en una linea: desde `pos` hacia `paso` (+1 o -1), hasta
/// `tope`. Si la celda y la siguiente tienen algo, al final del bloque; si
/// no, a la siguiente con algo; si no hay, al tope.
fn salto(pos: u32, adelante: bool, tope: u32, lleno: impl Fn(u32) -> bool) -> u32 {
    let siguiente = |p: u32| {
        if adelante {
            (p < tope).then(|| p + 1)
        } else {
            p.checked_sub(1)
        }
    };
    let Some(n) = siguiente(pos) else { return pos };
    if lleno(pos) && lleno(n) {
        let mut p = n;
        while let Some(q) = siguiente(p) {
            if !lleno(q) {
                break;
            }
            p = q;
        }
        return p;
    }
    let mut p = n;
    loop {
        if lleno(p) {
            return p;
        }
        match siguiente(p) {
            Some(q) => p = q,
            None => return p,
        }
    }
}

/// La seleccion despues de mover. Ctrl+flecha no pasa de lo usado (en un
/// visor no tiene sentido saltar a la fila un millon).
pub fn mover(sel: Seleccion, m: Mov, extender: bool, h: &Marco) -> Seleccion {
    let (f, c) = sel.activa;
    let (uf, uc) = (h.ultima.0.max(f), h.ultima.1.max(c));
    let pagina = h.filas_por_pagina.max(1);
    let nueva = match m {
        Mov::Arriba => (f.saturating_sub(1), c),
        Mov::Abajo => ((f + 1).min(ULTIMA_FILA), c),
        Mov::Izquierda => (f, c.saturating_sub(1)),
        Mov::Derecha => (f, (c + 1).min(ULTIMA_COLUMNA)),
        Mov::RePag => (f.saturating_sub(pagina), c),
        Mov::AvPag => ((f + pagina).min(ULTIMA_FILA), c),
        Mov::Inicio => (f, if c > h.fijas.1 { h.fijas.1 } else { 0 }),
        Mov::InicioHoja => (h.fijas.0, h.fijas.1),
        Mov::FinHoja => h.ultima,
        Mov::BordeArriba => (salto(f, false, uf, |x| (h.lleno)(x, c)), c),
        Mov::BordeAbajo => (salto(f, true, uf, |x| (h.lleno)(x, c)), c),
        Mov::BordeIzquierda => (f, salto(c, false, uc, |x| (h.lleno)(f, x))),
        Mov::BordeDerecha => (f, salto(c, true, uc, |x| (h.lleno)(f, x))),
    };
    if extender {
        Seleccion {
            ancla: sel.ancla,
            activa: nueva,
        }
    } else {
        Seleccion::en(nueva.0, nueva.1)
    }
}

// ---------------------------------------------------------------------------
// Zoom y desplazamiento

pub const ZOOM_MINIMO: f64 = 0.10;
pub const ZOOM_MAXIMO: f64 = 4.0;

pub fn acotar_zoom(z: f64) -> f64 {
    if z.is_finite() {
        z.clamp(ZOOM_MINIMO, ZOOM_MAXIMO)
    } else {
        1.0
    }
}

/// El zoom tras girar la rueda con Ctrl: un 10 % por muesca.
pub fn zoom_con_rueda(z: f64, muescas: f32) -> f64 {
    acotar_zoom(z * 1.1f64.powf(muescas as f64))
}

/// El zoom de la hoja tal como se guardo (`zoomScale`, en %).
pub fn zoom_guardado(porcentaje: u32) -> f64 {
    acotar_zoom(if porcentaje == 0 {
        1.0
    } else {
        porcentaje as f64 / 100.0
    })
}

/// **Zoom alrededor del raton** en un eje: donde queda el desplazamiento
/// (`pos`, px de la hoja al 100 %) para que el punto bajo el raton siga
/// bajo el raton. `raton`: px de pantalla desde el borde de las celdas;
/// `fijo`: lo que miden las filas o columnas inmovilizadas (px al 100 %).
/// Encima de lo inmovilizado no se mueve.
pub fn con_foco(pos: f64, raton: f64, fijo: f64, e0: f64, e1: f64) -> f64 {
    if raton < fijo * e0 || e0 <= 0.0 || e1 <= 0.0 {
        return pos;
    }
    let punto = pos + (raton - fijo * e0) / e0;
    punto - (raton - fijo * e1) / e1
}

/// Hasta donde se puede desplazar en un eje: (minimo, maximo). `fijo`: lo
/// inmovilizado (px al 100 %); `usado`: hasta donde llega lo usado;
/// `visible`: px de pantalla de la zona de celdas; `margen`: px de hoja de
/// mas al final, para que lo ultimo no quede pegado al borde.
pub fn limites(fijo: f64, usado: f64, visible: f64, escala: f64, margen: f64) -> (f64, f64) {
    let e = escala.max(0.01);
    let ve = (visible / e - fijo).max(0.0);
    (fijo, (usado + margen - ve).max(fijo))
}

/// El pulgar de una barra: (desde, largo) en px de la pista. `visible`: lo
/// que se ve (en las mismas unidades que `min` y `max`).
pub fn pulgar(pista: f32, pos: f64, min: f64, max: f64, visible: f64, minimo: f32) -> (f32, f32) {
    let total = (max - min).max(0.0) + visible.max(1e-6);
    let largo = ((pista as f64 * visible / total) as f32).clamp(minimo.min(pista), pista.max(0.0));
    let desde = if max > min {
        ((pista - largo) as f64 * ((pos - min) / (max - min)).clamp(0.0, 1.0)) as f32
    } else {
        0.0
    };
    (desde, largo)
}

/// Lo contrario de [`pulgar`]: el desplazamiento con el pulgar en `desde`.
pub fn pos_de_pulgar(pista: f32, largo: f32, desde: f32, min: f64, max: f64) -> f64 {
    let hueco = (pista - largo) as f64;
    if hueco <= 0.0 {
        return min;
    }
    min + (max - min) * (desde as f64 / hueco).clamp(0.0, 1.0)
}

/// Lo que hay que mover `pos` en un eje para que `[a, b)` (px de hoja) se
/// vea entero en `[pos, pos + ve)`.
pub fn para_ver(pos: f64, ve: f64, a: f64, b: f64) -> f64 {
    if a < pos {
        a
    } else if b > pos + ve {
        (b - ve).min(a)
    } else {
        pos
    }
}

// ---------------------------------------------------------------------------
// Copiar

/// Un campo de texto con tabuladores como lo pone Excel: entre comillas si
/// lleva tabuladores, saltos o comillas (y las comillas dobladas).
fn campo(s: &str) -> Cow<'_, str> {
    if s.contains(['\t', '\n', '\r', '"']) {
        Cow::Owned(format!("\"{}\"", s.replace('"', "\"\"")))
    } else {
        Cow::Borrowed(s)
    }
}

/// La seleccion como texto con tabuladores (lo que pega Excel o un correo):
/// filas con `\r\n`, tambien al final, como Excel.
pub fn tsv(r: Rango, mut valor: impl FnMut(u32, u32) -> String) -> String {
    let mut s = String::new();
    for f in r.fila0..=r.fila1 {
        for c in r.col0..=r.col1 {
            if c > r.col0 {
                s.push('\t');
            }
            let v = valor(f, c);
            s.push_str(&campo(&v));
        }
        s.push_str("\r\n");
    }
    s
}

/// Lo mas que se copia de una vez (celdas): un millon es ya un texto de
/// decenas de MB en el portapapeles.
pub const MAXIMO_A_COPIAR: u64 = 2_000_000;

pub fn celdas_de(r: Rango) -> u64 {
    (r.fila1 - r.fila0 + 1) as u64 * (r.col1 - r.col0 + 1) as u64
}

// ---------------------------------------------------------------------------
// Enlaces

/// Lo que abre un hipervinculo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enlace {
    /// Una pagina web o un correo: con el programa del sistema.
    Web(String),
    /// Un sitio del libro: una hoja (por nombre) y una celda.
    Sitio {
        hoja: Option<String>,
        celda: Option<(u32, u32)>,
    },
}

/// El destino de un hipervinculo, si es de los que se abren. Solo web y
/// correo: una ruta a un fichero podria ser un ejecutable.
pub fn enlace_de(destino: &str, externo: bool) -> Option<Enlace> {
    let d = destino.trim();
    if d.is_empty() {
        return None;
    }
    if externo {
        let l = d.to_ascii_lowercase();
        return (l.starts_with("http://") || l.starts_with("https://") || l.starts_with("mailto:"))
            .then(|| Enlace::Web(d.to_string()));
    }
    let d = d.trim_start_matches('#');
    let (hoja, celda) = match d.rsplit_once('!') {
        Some((h, c)) => {
            let h = h.trim();
            let h = h
                .strip_prefix('\'')
                .and_then(|x| x.strip_suffix('\''))
                .map_or(h.to_string(), |x| x.replace("''", "'"));
            (Some(h), c)
        }
        None => (None, d),
    };
    let celda = Rango::leer(celda).map(|r| (r.fila0, r.col0));
    (hoja.is_some() || celda.is_some()).then_some(Enlace::Sitio { hoja, celda })
}

// ---------------------------------------------------------------------------
// Configuracion regional

/// El patron de fecha corta de Windows (`dd/MM/yyyy`) en codigo de Excel
/// (`dd/mm/yyyy`), con el separador de Windows cambiado por la `/` (que se
/// pinta con el separador). `None` si lleva algo que no se entiende.
pub fn fecha_corta_de_windows(patron: &str, separador: &str) -> Option<String> {
    if patron.is_empty() {
        return None;
    }
    let mut p = patron.to_string();
    if !separador.is_empty() && separador != "/" {
        p = p.replace(separador, "/");
    }
    if !p
        .chars()
        .all(|c| matches!(c, 'd' | 'M' | 'y' | '/' | ' ' | '-' | '.' | ','))
        || !p.contains('d')
        || !p.contains('M')
    {
        return None;
    }
    Some(p.replace('M', "m"))
}

/// La `base` (nombres de meses y dias del idioma) con los separadores y la
/// fecha corta del usuario.
pub fn regional_de(mut base: Regional, s: &pixpin_shell::regional::RegionalDelSistema) -> Regional {
    let primero = |t: &str| t.chars().next();
    if let Some(d) = primero(&s.decimal) {
        base.decimal = d;
    }
    if let Some(m) = primero(&s.miles) {
        base.miles = m;
    }
    if base.miles == base.decimal {
        // Un sistema raro con los dos iguales: los numeros no se leerian.
        base.miles = if base.decimal == ',' { '.' } else { ',' };
    }
    if let Some(f) = primero(&s.separador_fecha) {
        base.separador_fecha = f;
    }
    if let Some(f) = fecha_corta_de_windows(&s.fecha_corta, &s.separador_fecha) {
        base.fecha_corta = f;
    }
    base
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_abren_los_libros_de_excel_modernos_y_no_los_binarios() {
        for n in [
            "a.xlsx",
            "B.XLSM",
            "plantilla.xltx",
            "macro.xltm",
            "viejo.xls",
            "Plantilla.XLT",
        ] {
            assert!(se_abre(n), "{n}");
        }
        // Caso negativo: el .xlsb es binario de otra clase; el .csv es texto.
        for n in ["b.xlsb", "c.csv", "xlsx", "d.ods", "xls"] {
            assert!(!se_abre(n), "{n}");
        }
    }

    #[test]
    fn un_xls_legible_se_abre_en_el_visor_y_uno_roto_se_queda_con_office() {
        let bueno = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/pixpin-hojas/tests/muestras/pequeno.xls");
        let datos = std::fs::read(&bueno).expect("la muestra de .xls");
        assert!(se_abre_fichero(&bueno));
        assert!(
            se_abre_fichero(&bueno),
            "la segunda vez sale de la memoria del ultimo resultado"
        );
        let dir =
            std::env::temp_dir().join(format!("pixpin-visor-hoja-xls-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        // Un .xls cortado (el contenedor no se entiende) se queda con Office.
        let cortado = dir.join("cortado.xls");
        std::fs::write(&cortado, &datos[..700]).unwrap();
        assert!(!se_abre_fichero(&cortado));
        // Con cabecera de contenedor pero sin libro dentro.
        let falso = dir.join("falso.xls");
        std::fs::write(
            &falso,
            [&pixpin_hojas::cfb::FIRMA[..], &[0u8; 600][..]].concat(),
        )
        .unwrap();
        assert!(!se_abre_fichero(&falso));
        // Un .xls que en verdad es un ZIP de ahora, y un .xlsx que en verdad es un .xls.
        let zip = dir.join("moderno.xls");
        std::fs::write(&zip, b"PK\x03\x04resto").unwrap();
        assert!(se_abre_fichero(&zip));
        let renombrado = dir.join("viejo.xlsx");
        std::fs::write(&renombrado, &datos).unwrap();
        assert!(se_abre_fichero(&renombrado));
        // No existe: no.
        assert!(!se_abre_fichero(&dir.join("no-esta.xls")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_xlsx_que_no_es_zip_no_se_abre_en_el_visor() {
        let dir = std::env::temp_dir().join(format!("pixpin-visor-hoja-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let bueno = dir.join("bueno.xlsx");
        std::fs::write(&bueno, b"PK\x03\x04resto").unwrap();
        let malo = dir.join("viejo.xlsx");
        std::fs::write(&malo, b"\xD0\xCF\x11\xE0 un xls renombrado").unwrap();
        assert!(se_abre_fichero(&bueno));
        assert!(!se_abre_fichero(&malo));
        assert!(!se_abre_fichero(&dir.join("no-existe.xlsx")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn las_teclas_de_excel() {
        use AccionTecla as A;
        assert_eq!(
            accion_de_tecla(VK_ABAJO, false, false, false),
            Some(A::Mover(Mov::Abajo, false))
        );
        assert_eq!(
            accion_de_tecla(VK_ABAJO, true, true, false),
            Some(A::Mover(Mov::BordeAbajo, true))
        );
        assert_eq!(
            accion_de_tecla(VK_INICIO, false, true, false),
            Some(A::Mover(Mov::InicioHoja, false))
        );
        assert_eq!(
            accion_de_tecla(VK_AVPAG, false, true, false),
            Some(A::HojaSiguiente)
        );
        assert_eq!(
            accion_de_tecla(VK_REPAG, false, false, false),
            Some(A::Mover(Mov::RePag, false))
        );
        assert_eq!(accion_de_tecla(VK_C, false, true, false), Some(A::Copiar));
        assert_eq!(
            accion_de_tecla(VK_0, false, true, false),
            Some(A::ZoomInicial)
        );
        assert_eq!(
            accion_de_tecla(VK_F11, false, false, false),
            Some(A::PantallaCompleta)
        );
        assert_eq!(
            accion_de_tecla(VK_ESCAPE, false, false, false),
            Some(A::Escape)
        );
        // Caso negativo: la C sola o con Alt no copia; el 0 solo no es zoom.
        assert_eq!(accion_de_tecla(VK_C, false, false, false), None);
        assert_eq!(accion_de_tecla(VK_C, false, true, true), None);
        assert_eq!(accion_de_tecla(VK_0, false, false, false), None);
    }

    fn marco<'a>(lleno: &'a dyn Fn(u32, u32) -> bool) -> Marco<'a> {
        Marco {
            ultima: (99, 9),
            fijas: (1, 0),
            filas_por_pagina: 20,
            lleno,
        }
    }

    #[test]
    fn mover_con_flechas_paginas_e_inicio() {
        let vacio = |_: u32, _: u32| false;
        let h = marco(&vacio);
        let s = Seleccion::en(5, 3);
        assert_eq!(mover(s, Mov::Abajo, false, &h), Seleccion::en(6, 3));
        assert_eq!(mover(s, Mov::AvPag, false, &h), Seleccion::en(25, 3));
        assert_eq!(mover(s, Mov::Inicio, false, &h), Seleccion::en(5, 0));
        assert_eq!(mover(s, Mov::InicioHoja, false, &h), Seleccion::en(1, 0));
        assert_eq!(mover(s, Mov::FinHoja, false, &h), Seleccion::en(99, 9));
        // Con Mayus el ancla se queda.
        let x = mover(s, Mov::Derecha, true, &h);
        assert_eq!(
            x.rango(),
            Rango {
                fila0: 5,
                col0: 3,
                fila1: 5,
                col1: 4
            }
        );
        // Caso negativo: arriba del todo y a la izquierda no se pasa de cero.
        assert_eq!(
            mover(Seleccion::en(0, 0), Mov::Arriba, false, &h),
            Seleccion::en(0, 0)
        );
        assert_eq!(
            mover(Seleccion::en(0, 0), Mov::RePag, false, &h),
            Seleccion::en(0, 0)
        );
    }

    #[test]
    fn ctrl_flecha_salta_como_excel() {
        // Columna 0: datos en filas 2..=5 y 10..=12.
        let lleno = |f: u32, c: u32| c == 0 && ((2..=5).contains(&f) || (10..=12).contains(&f));
        let h = marco(&lleno);
        // Desde un hueco, a la primera con algo.
        assert_eq!(
            mover(Seleccion::en(0, 0), Mov::BordeAbajo, false, &h).activa,
            (2, 0)
        );
        // Dentro de un bloque, a su final.
        assert_eq!(
            mover(Seleccion::en(2, 0), Mov::BordeAbajo, false, &h).activa,
            (5, 0)
        );
        // Al final de un bloque, al siguiente.
        assert_eq!(
            mover(Seleccion::en(5, 0), Mov::BordeAbajo, false, &h).activa,
            (10, 0)
        );
        assert_eq!(
            mover(Seleccion::en(12, 0), Mov::BordeArriba, false, &h).activa,
            (10, 0)
        );
        // Caso negativo: sin nada mas, al borde de lo usado (no a la fila un millon).
        assert_eq!(
            mover(Seleccion::en(12, 0), Mov::BordeAbajo, false, &h).activa,
            (99, 0)
        );
        assert_eq!(
            mover(Seleccion::en(0, 0), Mov::BordeArriba, false, &h).activa,
            (0, 0)
        );
    }

    #[test]
    fn el_zoom_se_acota_y_gira_alrededor_del_raton() {
        assert_eq!(acotar_zoom(10.0), ZOOM_MAXIMO);
        assert_eq!(acotar_zoom(0.0), ZOOM_MINIMO);
        assert_eq!(acotar_zoom(f64::NAN), 1.0);
        assert!((zoom_con_rueda(1.0, 1.0) - 1.1).abs() < 1e-9);
        assert_eq!(zoom_guardado(0), 1.0);
        assert_eq!(zoom_guardado(85), 0.85);
        // Sin paneles: el punto bajo el raton (pos + raton/e) se queda.
        let (pos, raton) = (100.0, 300.0);
        let nueva = con_foco(pos, raton, 0.0, 1.0, 2.0);
        assert!((nueva + raton / 2.0 - (pos + raton)).abs() < 1e-9);
        // Con 50 px fijos y el raton detras de ellos, igual.
        let n = con_foco(pos, raton, 50.0, 1.0, 1.5);
        let antes = pos + (raton - 50.0) / 1.0;
        let despues = n + (raton - 50.0 * 1.5) / 1.5;
        assert!((antes - despues).abs() < 1e-9);
        // Caso negativo: encima de lo inmovilizado no se mueve.
        assert_eq!(con_foco(pos, 20.0, 50.0, 1.0, 2.0), pos);
    }

    #[test]
    fn los_limites_y_el_pulgar_de_la_barra() {
        // 1000 px usados, 400 visibles al 100 %, 20 fijos, margen 0.
        let (min, max) = limites(20.0, 1000.0, 400.0, 1.0, 0.0);
        assert_eq!((min, max), (20.0, 620.0));
        // Caso negativo: si cabe todo, no se desplaza.
        assert_eq!(limites(0.0, 100.0, 400.0, 1.0, 0.0), (0.0, 0.0));
        let (d, l) = pulgar(200.0, 20.0, 20.0, 620.0, 380.0, 10.0);
        assert_eq!(d, 0.0);
        assert!((l - 200.0 * 380.0 / 980.0).abs() < 0.01);
        let (d, l2) = pulgar(200.0, 620.0, 20.0, 620.0, 380.0, 10.0);
        assert!((d + l2 - 200.0).abs() < 0.01);
        // Y de vuelta.
        assert!((pos_de_pulgar(200.0, l2, d, 20.0, 620.0) - 620.0).abs() < 1e-6);
        // Una hoja enorme: el pulgar no baja del minimo.
        assert_eq!(pulgar(200.0, 0.0, 0.0, 2_000_000.0, 500.0, 24.0).1, 24.0);
    }

    #[test]
    fn para_ver_una_celda_se_mueve_lo_justo() {
        assert_eq!(para_ver(100.0, 50.0, 120.0, 140.0), 100.0);
        assert_eq!(para_ver(100.0, 50.0, 80.0, 90.0), 80.0);
        assert_eq!(para_ver(100.0, 50.0, 160.0, 170.0), 120.0);
        // Una celda mas grande que la vista: se ve su principio.
        assert_eq!(para_ver(100.0, 50.0, 200.0, 400.0), 200.0);
    }

    #[test]
    fn copiar_como_texto_con_tabuladores() {
        let r = Rango {
            fila0: 0,
            col0: 0,
            fila1: 1,
            col1: 2,
        };
        let s = tsv(r, |f, c| match (f, c) {
            (0, 0) => "Nombre".into(),
            (0, 1) => "Con\ttab".into(),
            (0, 2) => "dijo \"hola\"".into(),
            (1, 0) => "1,234.50".into(),
            (1, 1) => String::new(),
            _ => "linea\nnueva".into(),
        });
        assert_eq!(
            s,
            "Nombre\t\"Con\ttab\"\t\"dijo \"\"hola\"\"\"\r\n1,234.50\t\t\"linea\nnueva\"\r\n"
        );
        assert_eq!(celdas_de(r), 6);
        // Caso negativo: una sola celda vacia es solo el salto final.
        assert_eq!(tsv(Rango::default(), |_, _| String::new()), "\r\n");
    }

    #[test]
    fn los_enlaces_web_y_los_del_libro() {
        assert_eq!(
            enlace_de("https://x.org/a", true),
            Some(Enlace::Web("https://x.org/a".into()))
        );
        assert_eq!(
            enlace_de("mailto:a@b.c", true),
            Some(Enlace::Web("mailto:a@b.c".into()))
        );
        assert_eq!(
            enlace_de("'Hoja de O''Neil'!B3", false),
            Some(Enlace::Sitio {
                hoja: Some("Hoja de O'Neil".into()),
                celda: Some((2, 1))
            })
        );
        assert_eq!(
            enlace_de("#Datos!A1:C4", false),
            Some(Enlace::Sitio {
                hoja: Some("Datos".into()),
                celda: Some((0, 0))
            })
        );
        assert_eq!(
            enlace_de("C7", false),
            Some(Enlace::Sitio {
                hoja: None,
                celda: Some((6, 2))
            })
        );
        // Caso negativo: un fichero o un programa no se abren desde una hoja.
        assert_eq!(
            enlace_de("file:///C:/Windows/system32/calc.exe", true),
            None
        );
        assert_eq!(enlace_de("..\\otro.xlsx", true), None);
        assert_eq!(enlace_de("", false), None);
    }

    #[test]
    fn la_configuracion_regional_de_windows() {
        use pixpin_shell::regional::RegionalDelSistema;
        let s = RegionalDelSistema {
            decimal: ",".into(),
            miles: ".".into(),
            separador_fecha: "-".into(),
            fecha_corta: "dd-MM-yyyy".into(),
        };
        let r = regional_de(Regional::es_pe(), &s);
        assert_eq!((r.decimal, r.miles, r.separador_fecha), (',', '.', '-'));
        assert_eq!(r.fecha_corta, "dd/mm/yyyy");
        assert_eq!(
            fecha_corta_de_windows("M/d/yyyy", "/").as_deref(),
            Some("m/d/yyyy")
        );
        // Caso negativo: lo que Windows no dijo se queda como estaba, y un
        // patron con era o texto no se usa.
        let r = regional_de(Regional::es_pe(), &RegionalDelSistema::default());
        assert_eq!(r, Regional::es_pe());
        assert_eq!(fecha_corta_de_windows("gg yyyy/MM/dd", "/"), None);
        // Dos separadores iguales no se dejan.
        let s = RegionalDelSistema {
            decimal: ",".into(),
            miles: ",".into(),
            ..Default::default()
        };
        let r = regional_de(Regional::es_pe(), &s);
        assert_ne!(r.decimal, r.miles);
    }
}
