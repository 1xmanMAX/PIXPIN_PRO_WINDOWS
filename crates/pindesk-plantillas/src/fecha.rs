//! **Las fechas**, a mano y sin dependencias: dias desde una epoca, dia de la
//! semana, semana ISO, sumar dias y meses. Calendario gregoriano proleptico
//! (el de los calendarios de papel); lo que se pinta va de 1900 a 2200.

use std::fmt;

/// Un dia del calendario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fecha {
    pub ano: i32,
    /// 1-12.
    pub mes: u32,
    /// 1-31.
    pub dia: u32,
}

/// El dia de la semana, de lunes (0) a domingo (6).
pub type DiaSemana = u32;

pub const ANO_MIN: i32 = 1900;
pub const ANO_MAX: i32 = 2200;

impl Fecha {
    /// Una fecha valida, o `None` (el 30 de febrero no existe).
    pub fn nueva(ano: i32, mes: u32, dia: u32) -> Option<Fecha> {
        if !(ANO_MIN..=ANO_MAX).contains(&ano) || !(1..=12).contains(&mes) {
            return None;
        }
        (dia >= 1 && dia <= dias_del_mes(ano, mes)).then_some(Fecha { ano, mes, dia })
    }

    /// `2027-01-31`. Tambien acepta `2027-1-31` y `2027/01/31`.
    pub fn leer(s: &str) -> Option<Fecha> {
        let mut p = s.trim().split(['-', '/']);
        let ano = p.next()?.trim().parse().ok()?;
        let mes = p.next()?.trim().parse().ok()?;
        let dia = p.next()?.trim().parse().ok()?;
        if p.next().is_some() {
            return None;
        }
        Fecha::nueva(ano, mes, dia)
    }

    /// Dias desde el 1-ene-1970 (negativos antes). Algoritmo de Howard
    /// Hinnant (`days_from_civil`).
    pub fn a_dias(self) -> i64 {
        let y = i64::from(self.ano) - i64::from(self.mes <= 2);
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let m = i64::from(self.mes);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.dia) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// La inversa de [`Fecha::a_dias`]. Fuera de [`ANO_MIN`]..[`ANO_MAX`] se
    /// queda en el borde.
    pub fn de_dias(z: i64) -> Fecha {
        let z = z + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let y = (y + i64::from(m <= 2)) as i32;
        if y < ANO_MIN {
            return Fecha {
                ano: ANO_MIN,
                mes: 1,
                dia: 1,
            };
        }
        if y > ANO_MAX {
            return Fecha {
                ano: ANO_MAX,
                mes: 12,
                dia: 31,
            };
        }
        Fecha {
            ano: y,
            mes: m,
            dia: d,
        }
    }

    pub fn mas_dias(self, n: i64) -> Fecha {
        Fecha::de_dias(self.a_dias() + n)
    }

    /// El mismo dia `n` meses despues (el 31 cae al ultimo del mes).
    pub fn mas_meses(self, n: i32) -> Fecha {
        let total = self.ano * 12 + self.mes as i32 - 1 + n;
        let ano = total.div_euclid(12).clamp(ANO_MIN, ANO_MAX);
        let mes = total.rem_euclid(12) as u32 + 1;
        Fecha {
            ano,
            mes,
            dia: self.dia.min(dias_del_mes(ano, mes)),
        }
    }

    /// Lunes 0 … domingo 6. El 1-ene-1970 fue jueves.
    pub fn dia_semana(self) -> DiaSemana {
        (self.a_dias() + 3).rem_euclid(7) as u32
    }

    pub fn primero_del_mes(self) -> Fecha {
        Fecha { dia: 1, ..self }
    }

    /// El primer dia de su semana, si la semana empieza en `primero`
    /// (0 lunes, 6 domingo).
    pub fn inicio_de_semana(self, primero: DiaSemana) -> Fecha {
        let atras = (self.dia_semana() + 7 - primero % 7) % 7;
        self.mas_dias(-i64::from(atras))
    }

    /// **La semana ISO 8601** y su año (el 1-ene-2027 es de la semana 53
    /// de 2026).
    pub fn semana_iso(self) -> (i32, u32) {
        // El jueves de su semana decide el año.
        let jueves = self.mas_dias(3 - i64::from(self.dia_semana()));
        let primero = Fecha {
            ano: jueves.ano,
            mes: 1,
            dia: 1,
        };
        let semana = (jueves.a_dias() - primero.a_dias()) / 7 + 1;
        (jueves.ano, semana as u32)
    }

    /// El dia del año (1-366).
    pub fn dia_del_ano(self) -> u32 {
        (self.a_dias()
            - Fecha {
                ano: self.ano,
                mes: 1,
                dia: 1,
            }
            .a_dias()
            + 1) as u32
    }
}

impl fmt::Display for Fecha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.ano, self.mes, self.dia)
    }
}

pub fn es_bisiesto(ano: i32) -> bool {
    (ano % 4 == 0 && ano % 100 != 0) || ano % 400 == 0
}

pub fn dias_del_mes(ano: i32, mes: u32) -> u32 {
    match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if es_bisiesto(ano) => 29,
        2 => 28,
        _ => 0,
    }
}

/// **Las semanas de un mes** en filas de 7, empezando en `primero`: cada
/// casilla es la fecha, y `None` lo que cae fuera del mes.
pub fn rejilla_del_mes(ano: i32, mes: u32, primero: DiaSemana) -> Vec<[Option<Fecha>; 7]> {
    let Some(uno) = Fecha::nueva(ano, mes, 1) else {
        return Vec::new();
    };
    let hueco = (uno.dia_semana() + 7 - primero % 7) % 7;
    let total = dias_del_mes(ano, mes);
    let mut filas = Vec::new();
    let mut fila = [None; 7];
    let mut col = hueco as usize;
    for d in 1..=total {
        fila[col] = Some(Fecha { ano, mes, dia: d });
        col += 1;
        if col == 7 {
            filas.push(fila);
            fila = [None; 7];
            col = 0;
        }
    }
    if col > 0 {
        filas.push(fila);
    }
    filas
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn dias_ida_y_vuelta_y_dia_de_la_semana() {
        let f = Fecha::nueva(1970, 1, 1).unwrap();
        assert_eq!(f.a_dias(), 0);
        assert_eq!(f.dia_semana(), 3, "jueves");
        let hoy = Fecha::nueva(2026, 11, 6).unwrap();
        assert_eq!(hoy.dia_semana(), 4, "el 6-nov-2026 es viernes");
        for z in [-25_000, -1, 0, 1, 20_000, 60_000, 83_000] {
            assert_eq!(Fecha::de_dias(z).a_dias(), z);
        }
        assert_eq!(Fecha::nueva(2027, 1, 1).unwrap().dia_semana(), 4);
    }

    #[test]
    fn meses_bisiestos_y_fechas_que_no_existen() {
        assert!(Fecha::nueva(2028, 2, 29).is_some());
        assert!(Fecha::nueva(2027, 2, 29).is_none());
        assert!(Fecha::nueva(2100, 2, 29).is_none());
        assert!(Fecha::nueva(2000, 2, 29).is_some());
        assert!(Fecha::nueva(2027, 13, 1).is_none());
        assert!(
            Fecha::nueva(1500, 1, 1).is_none(),
            "fuera de lo que se pinta"
        );
        let enero31 = Fecha::nueva(2027, 1, 31).unwrap();
        assert_eq!(enero31.mas_meses(1), Fecha::nueva(2027, 2, 28).unwrap());
        assert_eq!(enero31.mas_meses(-1), Fecha::nueva(2026, 12, 31).unwrap());
        assert_eq!(Fecha::leer("2027/3/9"), Fecha::nueva(2027, 3, 9));
        assert_eq!(Fecha::leer("2027-02-30"), None);
        assert_eq!(Fecha::leer("hoy"), None);
    }

    #[test]
    fn semana_iso_en_los_bordes_del_ano() {
        assert_eq!(Fecha::nueva(2027, 1, 1).unwrap().semana_iso(), (2026, 53));
        assert_eq!(Fecha::nueva(2027, 1, 4).unwrap().semana_iso(), (2027, 1));
        assert_eq!(Fecha::nueva(2026, 12, 31).unwrap().semana_iso(), (2026, 53));
        assert_eq!(Fecha::nueva(2024, 12, 30).unwrap().semana_iso(), (2025, 1));
        assert_eq!(Fecha::nueva(2026, 11, 6).unwrap().semana_iso(), (2026, 45));
    }

    #[test]
    fn rejilla_de_febrero_y_de_un_mes_que_empieza_en_domingo() {
        // Febrero de 2027 empieza en lunes y tiene 28 dias: 4 filas justas.
        let f = rejilla_del_mes(2027, 2, 0);
        assert_eq!(f.len(), 4);
        assert_eq!(f[0][0].unwrap().dia, 1);
        // Empezando en domingo, el lunes 1 va en la segunda casilla.
        let d = rejilla_del_mes(2027, 2, 6);
        assert_eq!(d[0][0], None);
        assert_eq!(d[0][1].unwrap().dia, 1);
        // Agosto de 2027 empieza en domingo: con semana de lunes, 6 filas.
        assert_eq!(rejilla_del_mes(2027, 8, 0).len(), 6);
        assert!(rejilla_del_mes(2027, 0, 0).is_empty());
    }

    #[test]
    fn inicio_de_semana_segun_el_pais() {
        let vie = Fecha::nueva(2026, 11, 6).unwrap();
        assert_eq!(vie.inicio_de_semana(0), Fecha::nueva(2026, 11, 2).unwrap());
        assert_eq!(vie.inicio_de_semana(6), Fecha::nueva(2026, 11, 1).unwrap());
        assert_eq!(vie.dia_del_ano(), 310);
    }
}
