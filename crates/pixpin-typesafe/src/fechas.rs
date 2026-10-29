//! **Fechas y horas dichas en voz alta**, resueltas en codigo (29-oct-2026,
//! `docs/probar-ideas-sin-interfaz.md`).
//!
//! La regla de TypeSafe: las fechas no se le preguntan a Jev. El codigo
//! busca en la frase **todas** las expresiones que podrian ser un dia o una
//! hora ([`buscar`]: «el viernes», «mañana a las 9», «el martes a las 3»,
//! «el 15 de noviembre», «la semana que viene», «a fin de mes»...) y las
//! resuelve contra la fecha de la reunion. Jev solo elige **cual** es el
//! plazo o la cita (una Choice sobre estas expresiones, con un Noul de «hay
//! alguna»): «la cotización **del viernes**» nombra un tema, no un plazo.
//!
//! Lo que decide el codigo, sin preguntar a nadie:
//!
//! - «el viernes» (o «el próximo viernes», «el viernes que viene»): el
//!   siguiente viernes despues de hoy; dicho un viernes, el de dentro de una
//!   semana. «este viernes»: el de esta semana (hoy, si es viernes).
//! - «el martes de la semana que viene»: el martes de la semana siguiente.
//!   «el lunes pasado»: el anterior.
//! - «la semana que viene»: el lunes de la semana siguiente, **aproximada**.
//! - «a fin de mes»: el ultimo dia del mes.
//! - «el 15»: el proximo dia 15 (este mes si no paso, si no el siguiente);
//!   «el 15 de noviembre»: ese dia (el año que viene si ya paso).
//! - «a las 3» sin «de la mañana/tarde»: de 1 a 7, por la tarde (15:00);
//!   de 8 a 12, tal cual. Es una reunion de trabajo o de clase.
//! - Una hora sola («a las 5»): hoy a esa hora.
//! - «mañana» es un dia salvo en «por la mañana», «esta mañana»...

use std::fmt;

/// Un dia del calendario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fecha {
    pub anio: i32,
    pub mes: u32,
    pub dia: u32,
}

const DIAS_SEMANA: [&str; 7] = [
    "lunes",
    "martes",
    "miercoles",
    "jueves",
    "viernes",
    "sabado",
    "domingo",
];

const DIAS_CORTOS: [&str; 7] = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];

const MESES: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

fn dias_del_mes(anio: i32, mes: u32) -> u32 {
    match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (anio % 4 == 0 && anio % 100 != 0) || anio % 400 == 0 => 29,
        _ => 28,
    }
}

impl Fecha {
    /// Una fecha que existe, o `None`.
    pub fn nueva(anio: i32, mes: u32, dia: u32) -> Option<Fecha> {
        ((1..=12).contains(&mes) && dia >= 1 && dia <= dias_del_mes(anio, mes)).then_some(Fecha {
            anio,
            mes,
            dia,
        })
    }

    /// De «2026-10-26».
    pub fn de_texto(s: &str) -> Option<Fecha> {
        let mut p = s.trim().splitn(3, '-');
        let anio = p.next()?.parse().ok()?;
        let mes = p.next()?.parse().ok()?;
        let dia = p.next()?.parse().ok()?;
        Fecha::nueva(anio, mes, dia)
    }

    /// Los dias desde el 1-ene-1970 (el algoritmo de H. Hinnant).
    pub fn dias(self) -> i64 {
        let y = i64::from(self.anio) - i64::from(self.mes <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.mes);
        let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(self.dia) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// La fecha de tantos dias desde el 1-ene-1970.
    pub fn de_dias(z: i64) -> Fecha {
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let dia = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let mes = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let anio = (yoe + era * 400 + i64::from(mes <= 2)) as i32;
        Fecha { anio, mes, dia }
    }

    pub fn mas(self, n: i64) -> Fecha {
        Fecha::de_dias(self.dias() + n)
    }

    /// 0 = lunes ... 6 = domingo.
    pub fn dia_semana(self) -> u32 {
        // El 1-ene-1970 fue jueves (3).
        (self.dias() + 3).rem_euclid(7) as u32
    }

    /// Los dias de `self` a `otra` (negativo si `otra` es antes).
    pub fn hasta(self, otra: Fecha) -> i64 {
        otra.dias() - self.dias()
    }

    pub fn ultimo_del_mes(self) -> Fecha {
        Fecha {
            dia: dias_del_mes(self.anio, self.mes),
            ..self
        }
    }
}

impl fmt::Display for Fecha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.anio, self.mes, self.dia)
    }
}

/// Una hora del dia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hora {
    pub h: u32,
    pub m: u32,
}

impl fmt::Display for Hora {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.h, self.m)
    }
}

/// **Un momento dicho**: el dia, la hora si se dijo, y si es aproximado
/// («la semana que viene»).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Momento {
    pub fecha: Fecha,
    pub hora: Option<Hora>,
    pub aproximado: bool,
}

impl fmt::Display for Momento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}",
            DIAS_CORTOS[self.fecha.dia_semana() as usize],
            self.fecha
        )?;
        if let Some(h) = self.hora {
            write!(f, " {h}")?;
        }
        if self.aproximado {
            write!(f, " (aprox.)")?;
        }
        Ok(())
    }
}

/// **Una expresion de la frase** que podria ser un dia o una hora: las
/// palabras tal como se dijeron y lo que significan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expresion {
    pub texto: String,
    pub momento: Momento,
}

/// Una palabra: donde esta (bytes del original) y como se compara (sin
/// mayusculas ni tildes).
#[derive(Debug, Clone)]
struct Pal {
    ini: usize,
    fin: usize,
    n: String,
}

fn sin_tilde(c: char) -> char {
    match c {
        'á' | 'à' => 'a',
        'é' | 'è' => 'e',
        'í' | 'ì' => 'i',
        'ó' | 'ò' => 'o',
        'ú' | 'ù' | 'ü' => 'u',
        'ñ' => 'n',
        otro => otro,
    }
}

/// Las palabras de la frase. «10:30» y «15/11» quedan en una.
fn palabras(s: &str) -> Vec<Pal> {
    let cs: Vec<(usize, char)> = s.char_indices().collect();
    let mut v = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        if !cs[i].1.is_alphanumeric() {
            i += 1;
            continue;
        }
        let ini = cs[i].0;
        let mut j = i;
        while j < cs.len() {
            let c = cs[j].1;
            let union = (c == ':' || c == '/')
                && j > i
                && cs[j - 1].1.is_ascii_digit()
                && cs.get(j + 1).is_some_and(|x| x.1.is_ascii_digit());
            if c.is_alphanumeric() || union {
                j += 1;
            } else {
                break;
            }
        }
        let fin = cs.get(j).map_or(s.len(), |x| x.0);
        let n: String = s[ini..fin]
            .chars()
            .flat_map(char::to_lowercase)
            .map(sin_tilde)
            .collect();
        v.push(Pal { ini, fin, n });
        i = j;
    }
    v
}

fn es(p: &[Pal], i: usize, w: &str) -> bool {
    p.get(i).is_some_and(|x| x.n == w)
}

fn sigue(p: &[Pal], i: usize, ws: &[&str]) -> bool {
    ws.iter().enumerate().all(|(k, w)| es(p, i + k, w))
}

fn numero(w: &str) -> Option<u32> {
    if let Ok(n) = w.parse() {
        return Some(n);
    }
    Some(match w {
        "una" | "uno" | "primero" => 1,
        "dos" => 2,
        "tres" => 3,
        "cuatro" => 4,
        "cinco" => 5,
        "seis" => 6,
        "siete" => 7,
        "ocho" => 8,
        "nueve" => 9,
        "diez" => 10,
        "once" => 11,
        "doce" => 12,
        _ => return None,
    })
}

fn dia_semana(w: &str) -> Option<u32> {
    DIAS_SEMANA.iter().position(|d| *d == w).map(|i| i as u32)
}

fn mes(w: &str) -> Option<u32> {
    MESES.iter().position(|m| *m == w).map(|i| i as u32 + 1)
}

/// Un dia que empieza en la palabra `i`: hasta donde llega (exclusivo), el
/// dia y si es aproximado.
fn fecha_en(p: &[Pal], i: usize, hoy: Fecha) -> Option<(usize, Fecha, bool)> {
    let w = p.get(i)?.n.as_str();
    if w == "hoy" {
        return Some((i + 1, hoy, false));
    }
    if sigue(p, i, &["pasado", "manana"]) {
        return Some((i + 2, hoy.mas(2), false));
    }
    if w == "manana" {
        let antes = i.checked_sub(1).map(|k| p[k].n.as_str());
        if !matches!(antes, Some("la" | "esta" | "por" | "de" | "en" | "cada")) {
            return Some((i + 1, hoy.mas(1), false));
        }
        return None;
    }
    // «la semana que viene», «la próxima semana».
    let j = if w == "la" { i + 1 } else { i };
    for f in [
        &["semana", "que", "viene"][..],
        &["proxima", "semana"],
        &["semana", "proxima"],
    ] {
        if sigue(p, j, f) {
            let lunes = hoy.mas(7 - i64::from(hoy.dia_semana()));
            return Some((j + f.len(), lunes, true));
        }
    }
    // «a fin de mes», «a final de mes».
    let j = if w == "a" { i + 1 } else { i };
    if sigue(p, j, &["fin", "de", "mes"]) || sigue(p, j, &["final", "de", "mes"]) {
        return Some((j + 3, hoy.ultimo_del_mes(), false));
    }
    // «el (próximo) viernes (que viene)», «este viernes».
    let mut j = i;
    let mut este = false;
    if matches!(w, "el" | "este" | "esta") {
        este = w != "el";
        j += 1;
    }
    if es(p, j, "proximo") {
        j += 1;
    }
    if let Some(d) = p.get(j).and_then(|x| dia_semana(&x.n)) {
        j += 1;
        let hd = hoy.dia_semana();
        if sigue(p, j, &["de", "la", "semana", "que", "viene"])
            || sigue(p, j, &["de", "la", "proxima", "semana"])
        {
            let lunes = hoy.mas(7 - i64::from(hd));
            let n = if es(p, j + 2, "semana") { 5 } else { 4 };
            return Some((j + n, lunes.mas(i64::from(d)), false));
        }
        if es(p, j, "pasado") {
            // «el lunes pasado»: el anterior (dicho un lunes, el de hace una
            // semana).
            let mut atras = (i64::from(hd) - i64::from(d)).rem_euclid(7);
            if atras == 0 {
                atras = 7;
            }
            return Some((j + 1, hoy.mas(-atras), false));
        }
        if sigue(p, j, &["que", "viene"]) {
            j += 2;
        }
        let mut delta = (i64::from(d) - i64::from(hd)).rem_euclid(7);
        if delta == 0 && !este {
            delta = 7;
        }
        return Some((j, hoy.mas(delta), false));
    }
    // «15/11».
    if let Some((d, m)) = w.split_once('/') {
        let (d, m) = (d.parse().ok()?, m.parse().ok()?);
        return proximo_dia(hoy, d, Some(m)).map(|f| (i + 1, f, false));
    }
    // «el 15», «el 15 de noviembre», «15 de noviembre».
    let (j, con_el) = if w == "el" { (i + 1, true) } else { (i, false) };
    let n = p.get(j).and_then(|x| {
        (x.n.chars().all(|c| c.is_ascii_digit()) || x.n == "primero")
            .then(|| numero(&x.n))
            .flatten()
    })?;
    if !(1..=31).contains(&n) {
        return None;
    }
    if es(p, j + 1, "de")
        && let Some(m) = p.get(j + 2).and_then(|x| mes(&x.n))
    {
        return proximo_dia(hoy, n, Some(m)).map(|f| (j + 3, f, false));
    }
    if con_el {
        return proximo_dia(hoy, n, None).map(|f| (j + 1, f, false));
    }
    None
}

/// El proximo dia `d` (del mes `m` si se dice), desde hoy incluido.
fn proximo_dia(hoy: Fecha, d: u32, m: Option<u32>) -> Option<Fecha> {
    match m {
        Some(m) => {
            let f = Fecha::nueva(hoy.anio, m, d)?;
            if f >= hoy {
                Some(f)
            } else {
                Fecha::nueva(hoy.anio + 1, m, d)
            }
        }
        None => {
            let mut a = hoy.anio;
            let mut mm = hoy.mes;
            for _ in 0..3 {
                if let Some(f) = Fecha::nueva(a, mm, d).filter(|f| *f >= hoy) {
                    return Some(f);
                }
                mm += 1;
                if mm > 12 {
                    mm = 1;
                    a += 1;
                }
            }
            None
        }
    }
}

/// Una hora que empieza en la palabra `i`: hasta donde llega y la hora.
fn hora_en(p: &[Pal], i: usize) -> Option<(usize, Hora)> {
    if sigue(p, i, &["al", "mediodia"]) || sigue(p, i, &["a", "mediodia"]) {
        return Some((i + 2, Hora { h: 12, m: 0 }));
    }
    let j = if sigue(p, i, &["a", "las"]) || sigue(p, i, &["a", "la"]) {
        i + 2
    } else {
        return None;
    };
    let w = p.get(j)?.n.as_str();
    let (mut h, mut m) = match w.split_once(':') {
        Some((a, b)) => (a.parse().ok()?, b.parse().ok()?),
        None => (numero(w)?, 0),
    };
    if h > 23 || m > 59 {
        return None;
    }
    let mut k = j + 1;
    if sigue(p, k, &["y", "media"]) {
        m = 30;
        k += 2;
    } else if sigue(p, k, &["y", "cuarto"]) {
        m = 15;
        k += 2;
    } else if sigue(p, k, &["menos", "cuarto"]) {
        h = if h == 1 { 12 } else { h - 1 };
        m = 45;
        k += 2;
    }
    if sigue(p, k, &["de", "la", "manana"]) {
        k += 3;
    } else if sigue(p, k, &["de", "la", "tarde"]) || sigue(p, k, &["de", "la", "noche"]) {
        if h < 12 {
            h += 12;
        }
        k += 3;
    } else {
        if sigue(p, k, &["en", "punto"]) {
            k += 2;
        }
        if (1..=7).contains(&h) {
            h += 12;
        }
    }
    Some((k, Hora { h, m }))
}

/// **Todas las expresiones de dia u hora de la frase**, en orden, resueltas
/// contra `hoy` (el dia de la reunion). Un dia seguido de una hora («el
/// martes a las 3») o una hora seguida de un dia («a las 3 el martes») es
/// una sola expresion.
pub fn buscar(frase: &str, hoy: Fecha) -> Vec<Expresion> {
    let p = palabras(frase);
    let mut v = Vec::new();
    let mut i = 0;
    while i < p.len() {
        let hallado = if let Some((j, f, aprox)) = fecha_en(&p, i, hoy) {
            // «mañana por la mañana a las 9»: lo de «por la mañana» se salta.
            let mut k = j;
            if sigue(&p, k, &["por", "la"]) {
                k += 3;
            }
            match hora_en(&p, k) {
                Some((k, h)) => Some((k, f, Some(h), aprox)),
                None => Some((j, f, None, aprox)),
            }
        } else if let Some((j, h)) = hora_en(&p, i) {
            match fecha_en(&p, j, hoy) {
                Some((k, f, aprox)) => Some((k, f, Some(h), aprox)),
                None => Some((j, hoy, Some(h), false)),
            }
        } else {
            None
        };
        match hallado {
            Some((fin, fecha, hora, aproximado)) => {
                v.push(Expresion {
                    texto: frase[p[i].ini..p[fin - 1].fin].to_string(),
                    momento: Momento {
                        fecha,
                        hora,
                        aproximado,
                    },
                });
                i = fin;
            }
            None => i += 1,
        }
    }
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Lunes 26-oct-2026.
    fn lunes() -> Fecha {
        Fecha::nueva(2026, 10, 26).unwrap()
    }

    fn una(frase: &str) -> (String, String) {
        let v = buscar(frase, lunes());
        assert_eq!(v.len(), 1, "{frase}: {v:?}");
        (v[0].texto.clone(), v[0].momento.to_string())
    }

    #[test]
    fn el_calendario() {
        let f = lunes();
        assert_eq!(f.dia_semana(), 0);
        assert_eq!(Fecha::de_dias(f.dias()), f);
        assert_eq!(f.mas(6).to_string(), "2026-11-01");
        assert_eq!(f.mas(-300).mas(300), f);
        assert_eq!(
            Fecha::nueva(2028, 2, 29).unwrap().mas(1).to_string(),
            "2028-03-01"
        );
        assert!(Fecha::nueva(2027, 2, 29).is_none());
        assert_eq!(Fecha::de_texto("2026-10-26"), Some(f));
        assert_eq!(f.ultimo_del_mes().to_string(), "2026-10-31");
        assert_eq!(f.hasta(f.mas(4)), 4);
        assert_eq!(Fecha::nueva(1970, 1, 1).unwrap().dias(), 0);
    }

    #[test]
    fn los_dias_de_la_semana() {
        assert_eq!(
            una("yo me encargo de mandar la cotización el viernes"),
            ("el viernes".into(), "vie 2026-10-30".into())
        );
        // Dicho un lunes, «el lunes» es el de la semana que viene.
        assert_eq!(una("lo vemos el lunes").1, "lun 2026-11-02");
        assert_eq!(una("este lunes sin falta").1, "lun 2026-10-26");
        assert_eq!(una("el próximo miércoles").1, "mié 2026-10-28");
        assert_eq!(una("el jueves que viene").0, "el jueves que viene");
        assert_eq!(
            una("el lunes pasado ya hablamos"),
            ("el lunes pasado".into(), "lun 2026-10-19".into())
        );
        assert_eq!(una("el viernes pasado").1, "vie 2026-10-23");
        assert_eq!(
            una("el martes de la semana que viene"),
            (
                "el martes de la semana que viene".into(),
                "mar 2026-11-03".into()
            )
        );
    }

    #[test]
    fn hoy_manana_y_lo_que_no_es_un_dia() {
        assert_eq!(una("esto es para hoy").1, "lun 2026-10-26");
        assert_eq!(una("mañana lo mando").1, "mar 2026-10-27");
        assert_eq!(una("pasado mañana").1, "mié 2026-10-28");
        assert!(buscar("lo hice esta mañana", lunes()).is_empty());
        assert_eq!(
            una("mañana por la mañana a las 9"),
            (
                "mañana por la mañana a las 9".into(),
                "mar 2026-10-27 09:00".into()
            )
        );
        assert!(buscar("compramos 3 proyectores", lunes()).is_empty());
        assert_eq!(una("la semana que viene").1, "lun 2026-11-02 (aprox.)");
        assert_eq!(una("a fin de mes").1, "sáb 2026-10-31");
    }

    #[test]
    fn las_fechas_con_numero() {
        assert_eq!(una("el 15 de noviembre").1, "dom 2026-11-15");
        assert_eq!(una("antes del 3 de enero").1, "dom 2027-01-03");
        assert_eq!(una("para el 5").1, "jue 2026-11-05");
        assert_eq!(una("para el 28").1, "mié 2026-10-28");
        assert_eq!(una("el 15/11").0, "15/11");
    }

    #[test]
    fn las_horas() {
        assert_eq!(
            una("nos vemos el martes a las 3"),
            ("el martes a las 3".into(), "mar 2026-10-27 15:00".into())
        );
        assert_eq!(
            una("a las tres y media el jueves").1,
            "jue 2026-10-29 15:30"
        );
        assert_eq!(una("a las 10:30").1, "lun 2026-10-26 10:30");
        assert_eq!(una("a las 9 de la mañana").1, "lun 2026-10-26 09:00");
        assert_eq!(una("a las 8 de la noche").1, "lun 2026-10-26 20:00");
        assert_eq!(una("a la una menos cuarto").1, "lun 2026-10-26 12:45");
        assert_eq!(una("al mediodía").1, "lun 2026-10-26 12:00");
    }

    #[test]
    fn varias_en_una_frase_en_orden() {
        let v = buscar(
            "la cotización del viernes la mando mañana a las 10",
            lunes(),
        );
        let t: Vec<&str> = v.iter().map(|e| e.texto.as_str()).collect();
        assert_eq!(t, ["viernes", "mañana a las 10"]);
    }
}
