//! **Los campos que se llenan solos** y las paginas de una repeticion.
//!
//! Cada pagina tiene su [`Contexto`]: su fecha, su numero y el idioma. Un
//! bloque [`Tipo::Campo`](crate::modelo::Tipo::Campo) pinta uno de sus datos,
//! y cualquier texto puede llevarlos entre llaves:
//!
//! | En el texto | Sale (6-nov-2026, es) |
//! |---|---|
//! | `{fecha}` | 06/11/2026 |
//! | `{fecha:largo}` | 6 de noviembre de 2026 |
//! | `{fecha:completo}` | viernes, 6 de noviembre de 2026 |
//! | `{fecha:iso}` | 2026-11-06 |
//! | `{dia}` | 6 |
//! | `{dia_semana}` / `{dia_semana:corto}` | viernes / vie |
//! | `{semana}` | 45 |
//! | `{mes}` / `{mes:numero}` | noviembre / 11 |
//! | `{ano}` | 2026 |
//! | `{pagina}` / `{paginas}` | 3 / 365 |
//!
//! Lo que no es un campo se deja como esta (`{hola}` sale `{hola}`).

use crate::fecha::Fecha;
use crate::locale::{self, FormatoFecha, Locale};
use crate::modelo::{Cada, QueCampo, Repeticion};

/// Lo mas largo que puede salir una repeticion (un año de dias son 366).
pub const MAX_PAGINAS: usize = 800;

/// **Lo de una pagina.**
#[derive(Debug, Clone, Copy)]
pub struct Contexto {
    pub fecha: Fecha,
    /// 1, 2, 3…
    pub pagina: usize,
    pub paginas: usize,
    pub locale: &'static Locale,
}

impl Contexto {
    pub fn nuevo(fecha: Fecha, idioma: &str) -> Contexto {
        Contexto {
            fecha,
            pagina: 1,
            paginas: 1,
            locale: locale::de(idioma),
        }
    }

    /// **El valor de un campo.**
    pub fn valor(&self, campo: QueCampo, formato: FormatoFecha) -> String {
        let l = self.locale;
        let f = self.fecha;
        match campo {
            QueCampo::Fecha => l.fecha(f, formato),
            QueCampo::Dia => f.dia.to_string(),
            QueCampo::DiaSemana => l.dia_semana(f, formato),
            QueCampo::Semana => match formato {
                FormatoFecha::Largo | FormatoFecha::Completo => l.semana(f.semana_iso().1),
                _ => f.semana_iso().1.to_string(),
            },
            QueCampo::Mes => l.mes_de(f, formato),
            QueCampo::Ano => f.ano.to_string(),
            QueCampo::Pagina => match formato {
                FormatoFecha::Largo | FormatoFecha::Completo => {
                    format!("{} / {}", self.pagina, self.paginas)
                }
                _ => self.pagina.to_string(),
            },
        }
    }

    /// **Pone los campos de un texto** (`{fecha:largo}`, `{semana}`…).
    pub fn rellenar(&self, texto: &str) -> String {
        if !texto.contains('{') {
            return texto.to_string();
        }
        let mut sal = String::with_capacity(texto.len() + 16);
        let mut resto = texto;
        while let Some(i) = resto.find('{') {
            sal.push_str(&resto[..i]);
            let tras = &resto[i + 1..];
            match tras.find('}') {
                Some(j) if j <= 24 => {
                    let dentro = &tras[..j];
                    match self.de_marca(dentro) {
                        Some(v) => sal.push_str(&v),
                        None => {
                            sal.push('{');
                            sal.push_str(dentro);
                            sal.push('}');
                        }
                    }
                    resto = &tras[j + 1..];
                }
                _ => {
                    sal.push('{');
                    resto = tras;
                }
            }
        }
        sal.push_str(resto);
        sal
    }

    fn de_marca(&self, marca: &str) -> Option<String> {
        let (nombre, formato) = match marca.split_once(':') {
            Some((n, f)) => (n.trim(), Some(f.trim())),
            None => (marca.trim(), None),
        };
        if nombre == "paginas" {
            return Some(self.paginas.to_string());
        }
        let campo = QueCampo::TODOS.into_iter().find(|c| c.clave() == nombre)?;
        let formato = match formato {
            None => match campo {
                QueCampo::DiaSemana | QueCampo::Mes => FormatoFecha::Largo,
                _ => FormatoFecha::Corto,
            },
            Some(f) => formato_de(f)?,
        };
        Some(match (campo, formato) {
            // `{mes}` es el nombre solo; `{mes:largo}` lleva el año.
            (QueCampo::Mes, FormatoFecha::Largo) if marca.trim() == "mes" => {
                self.locale.mes(self.fecha.mes).to_string()
            }
            _ => self.valor(campo, formato),
        })
    }
}

/// El formato por su palabra (`corto`, `largo`, `completo`, `iso`,
/// `numero`, `abreviado`, `letra`).
pub fn formato_de(s: &str) -> Option<FormatoFecha> {
    Some(match s {
        "corto" => FormatoFecha::Corto,
        "largo" => FormatoFecha::Largo,
        "completo" => FormatoFecha::Completo,
        "iso" => FormatoFecha::Iso,
        "numero" => FormatoFecha::Numero,
        "abreviado" => FormatoFecha::Abreviado,
        "letra" => FormatoFecha::Letra,
        _ => return None,
    })
}

/// **Las paginas de una repeticion**: una fecha por pagina. Sin repeticion
/// (o con fechas que no se entienden), una sola pagina en `hoy`.
pub fn paginas(rep: Option<&Repeticion>, hoy: Fecha, idioma: &str) -> Vec<Contexto> {
    let loc = locale::de(idioma);
    let mut fechas = Vec::new();
    if let Some(r) = rep
        && let (Some(desde), Some(hasta)) = (Fecha::leer(&r.desde), Fecha::leer(&r.hasta))
        && desde <= hasta
    {
        match r.cada {
            Cada::Dia => {
                let mut f = desde;
                while f <= hasta && fechas.len() < MAX_PAGINAS {
                    if !(r.sin_fines_de_semana && f.dia_semana() >= 5) {
                        fechas.push(f);
                    }
                    f = f.mas_dias(1);
                }
            }
            Cada::Semana => {
                let mut f = desde.inicio_de_semana(loc.primer_dia);
                while f <= hasta && fechas.len() < MAX_PAGINAS {
                    fechas.push(f);
                    f = f.mas_dias(7);
                }
            }
            Cada::Mes => {
                let mut f = desde.primero_del_mes();
                while f <= hasta && fechas.len() < MAX_PAGINAS {
                    fechas.push(f);
                    f = f.mas_meses(1);
                }
            }
        }
    }
    if fechas.is_empty() {
        fechas.push(hoy);
    }
    let total = fechas.len();
    fechas
        .into_iter()
        .enumerate()
        .map(|(i, fecha)| Contexto {
            fecha,
            pagina: i + 1,
            paginas: total,
            locale: loc,
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn f(a: i32, m: u32, d: u32) -> Fecha {
        Fecha::nueva(a, m, d).unwrap()
    }

    #[test]
    fn los_campos_de_un_texto_en_varios_idiomas() {
        let c = Contexto::nuevo(f(2026, 11, 6), "es-ES");
        assert_eq!(
            c.rellenar("Hoy es {dia_semana}, {fecha:largo} (semana {semana})"),
            "Hoy es viernes, 6 de noviembre de 2026 (semana 45)"
        );
        assert_eq!(c.rellenar("{mes} {ano}"), "noviembre 2026");
        assert_eq!(c.rellenar("{mes:numero}/{dia}"), "11/6");
        assert_eq!(c.rellenar("{hola} {fecha:raro} {"), "{hola} {fecha:raro} {");
        let en = Contexto::nuevo(f(2026, 11, 6), "en-US");
        assert_eq!(en.rellenar("{dia_semana:corto} {fecha}"), "Fri 11/6/2026");
        assert_eq!(en.valor(QueCampo::Semana, FormatoFecha::Largo), "Week 45");
        let ja = Contexto::nuevo(f(2027, 1, 4), "ja-JP");
        assert_eq!(ja.rellenar("{fecha:largo}"), "2027年1月4日");
        assert_eq!(ja.rellenar("{dia_semana}"), "月曜日");
        let de = Contexto::nuevo(f(2027, 1, 4), "de-DE");
        assert_eq!(de.rellenar("{fecha:completo}"), "Montag, 4. Januar 2027");
        let ru = Contexto::nuevo(f(2027, 3, 8), "ru");
        assert_eq!(ru.rellenar("{fecha:largo}"), "8 марта 2027 г.");
        assert_eq!(ru.rellenar("{mes}"), "март");
    }

    #[test]
    fn un_ano_de_dias_semanas_y_meses() {
        let rep = |cada, sin| Repeticion {
            cada,
            desde: "2027-01-01".into(),
            hasta: "2027-12-31".into(),
            sin_fines_de_semana: sin,
        };
        let hoy = f(2026, 11, 6);
        let dias = paginas(Some(&rep(Cada::Dia, false)), hoy, "es");
        assert_eq!(dias.len(), 365);
        assert_eq!(dias[0].fecha, f(2027, 1, 1));
        assert_eq!(dias[364].fecha, f(2027, 12, 31));
        assert_eq!(dias[41].paginas, 365);
        assert_eq!(dias[41].pagina, 42);
        assert_eq!(dias[41].rellenar("{pagina} de {paginas}"), "42 de 365");
        let laborables = paginas(Some(&rep(Cada::Dia, true)), hoy, "es");
        assert_eq!(laborables.len(), 261);
        assert!(laborables.iter().all(|c| c.fecha.dia_semana() < 5));
        // Las semanas empiezan en lunes en español y en domingo en ingles.
        let s_es = paginas(Some(&rep(Cada::Semana, false)), hoy, "es");
        assert_eq!(s_es[0].fecha, f(2026, 12, 28));
        assert_eq!(s_es.len(), 53);
        let s_en = paginas(Some(&rep(Cada::Semana, false)), hoy, "en-US");
        assert_eq!(s_en[0].fecha, f(2026, 12, 27));
        let meses = paginas(Some(&rep(Cada::Mes, false)), hoy, "fr");
        assert_eq!(meses.len(), 12);
        assert_eq!(meses[1].rellenar("{mes:largo}"), "février 2027");
    }

    #[test]
    fn sin_repeticion_o_con_fechas_malas_una_pagina_hoy() {
        let hoy = f(2026, 11, 6);
        assert_eq!(paginas(None, hoy, "es").len(), 1);
        let mala = Repeticion {
            cada: Cada::Dia,
            desde: "2027-12-31".into(),
            hasta: "2027-01-01".into(),
            sin_fines_de_semana: false,
        };
        let p = paginas(Some(&mala), hoy, "es");
        assert_eq!((p.len(), p[0].fecha), (1, hoy));
        let larga = Repeticion {
            cada: Cada::Dia,
            desde: "1990-01-01".into(),
            hasta: "2100-01-01".into(),
            sin_fines_de_semana: false,
        };
        assert_eq!(paginas(Some(&larga), hoy, "es").len(), MAX_PAGINAS);
    }
}
