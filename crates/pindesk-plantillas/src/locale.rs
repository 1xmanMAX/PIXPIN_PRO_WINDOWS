//! **Las fechas dichas en cada idioma**: meses, dias de la semana, el primer
//! dia de la semana y el orden de dia, mes y año. Son datos del calendario,
//! no textos de la interfaz (esos van en `i18n/`): una plantilla en japones
//! dice «2027年1月4日» aunque la app este en español.
//!
//! Los catorce idiomas de PinDesk (`Idioma::TODOS` de pindesk-store), con los
//! nombres de CLDR. Un idioma desconocido cae al ingles.

use crate::fecha::{DiaSemana, Fecha};

/// Como se escribe una fecha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orden {
    /// 6/11/2026, «6 de noviembre de 2026».
    DiaMesAno,
    /// 11/6/2026, «November 6, 2026».
    MesDiaAno,
    /// 2026/11/6, «2026年11月6日».
    AnoMesDia,
}

/// Lo de un idioma.
#[derive(Debug)]
pub struct Locale {
    pub etiqueta: &'static str,
    pub meses: [&'static str; 12],
    /// Los meses dentro de una fecha, si cambian (el genitivo ruso).
    meses_fecha: Option<[&'static str; 12]>,
    pub meses_cortos: [&'static str; 12],
    /// De lunes a domingo.
    pub dias: [&'static str; 7],
    pub dias_cortos: [&'static str; 7],
    pub dias_letra: [&'static str; 7],
    /// 0 lunes, 6 domingo.
    pub primer_dia: DiaSemana,
    orden: Orden,
    /// El separador de la fecha corta.
    separador: &'static str,
    /// «{d} de {m} de {a}»: `{d}`, `{m}`, `{a}` y, con dia de la semana,
    /// `{s}`.
    largo: &'static str,
    largo_con_dia: &'static str,
    /// «Semana {n}».
    semana: &'static str,
    /// Escribir de derecha a izquierda.
    pub rtl: bool,
}

const ES: Locale = Locale {
    etiqueta: "es-ES",
    meses: [
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
    ],
    meses_fecha: None,
    meses_cortos: [
        "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic",
    ],
    dias: [
        "lunes",
        "martes",
        "miércoles",
        "jueves",
        "viernes",
        "sábado",
        "domingo",
    ],
    dias_cortos: ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"],
    dias_letra: ["L", "M", "X", "J", "V", "S", "D"],
    primer_dia: 0,
    orden: Orden::DiaMesAno,
    separador: "/",
    largo: "{d} de {m} de {a}",
    largo_con_dia: "{s}, {d} de {m} de {a}",
    semana: "Semana {n}",
    rtl: false,
};

const EN: Locale = Locale {
    etiqueta: "en-US",
    meses: [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ],
    meses_fecha: None,
    meses_cortos: [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ],
    dias: [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ],
    dias_cortos: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
    dias_letra: ["M", "T", "W", "T", "F", "S", "S"],
    primer_dia: 6,
    orden: Orden::MesDiaAno,
    separador: "/",
    largo: "{m} {d}, {a}",
    largo_con_dia: "{s}, {m} {d}, {a}",
    semana: "Week {n}",
    rtl: false,
};

const ZH: Locale = Locale {
    etiqueta: "zh-CN",
    meses: [
        "一月",
        "二月",
        "三月",
        "四月",
        "五月",
        "六月",
        "七月",
        "八月",
        "九月",
        "十月",
        "十一月",
        "十二月",
    ],
    meses_fecha: Some([
        "1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月",
    ]),
    meses_cortos: [
        "1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月",
    ],
    dias: [
        "星期一",
        "星期二",
        "星期三",
        "星期四",
        "星期五",
        "星期六",
        "星期日",
    ],
    dias_cortos: ["周一", "周二", "周三", "周四", "周五", "周六", "周日"],
    dias_letra: ["一", "二", "三", "四", "五", "六", "日"],
    primer_dia: 0,
    orden: Orden::AnoMesDia,
    separador: "/",
    largo: "{a}年{m}{d}日",
    largo_con_dia: "{a}年{m}{d}日 {s}",
    semana: "第{n}周",
    rtl: false,
};

const HI: Locale = Locale {
    etiqueta: "hi-IN",
    meses: [
        "जनवरी",
        "फ़रवरी",
        "मार्च",
        "अप्रैल",
        "मई",
        "जून",
        "जुलाई",
        "अगस्त",
        "सितंबर",
        "अक्तूबर",
        "नवंबर",
        "दिसंबर",
    ],
    meses_fecha: None,
    meses_cortos: [
        "जन॰",
        "फ़र॰",
        "मार्च",
        "अप्रैल",
        "मई",
        "जून",
        "जुल॰",
        "अग॰",
        "सित॰",
        "अक्तू॰",
        "नव॰",
        "दिस॰",
    ],
    dias: [
        "सोमवार",
        "मंगलवार",
        "बुधवार",
        "गुरुवार",
        "शुक्रवार",
        "शनिवार",
        "रविवार",
    ],
    dias_cortos: ["सोम", "मंगल", "बुध", "गुरु", "शुक्र", "शनि", "रवि"],
    dias_letra: ["सो", "मं", "बु", "गु", "शु", "श", "र"],
    primer_dia: 6,
    orden: Orden::DiaMesAno,
    separador: "/",
    largo: "{d} {m} {a}",
    largo_con_dia: "{s}, {d} {m} {a}",
    semana: "सप्ताह {n}",
    rtl: false,
};

const AR: Locale = Locale {
    etiqueta: "ar-SA",
    meses: [
        "يناير",
        "فبراير",
        "مارس",
        "أبريل",
        "مايو",
        "يونيو",
        "يوليو",
        "أغسطس",
        "سبتمبر",
        "أكتوبر",
        "نوفمبر",
        "ديسمبر",
    ],
    meses_fecha: None,
    meses_cortos: [
        "يناير",
        "فبراير",
        "مارس",
        "أبريل",
        "مايو",
        "يونيو",
        "يوليو",
        "أغسطس",
        "سبتمبر",
        "أكتوبر",
        "نوفمبر",
        "ديسمبر",
    ],
    dias: [
        "الاثنين",
        "الثلاثاء",
        "الأربعاء",
        "الخميس",
        "الجمعة",
        "السبت",
        "الأحد",
    ],
    dias_cortos: [
        "الاثنين",
        "الثلاثاء",
        "الأربعاء",
        "الخميس",
        "الجمعة",
        "السبت",
        "الأحد",
    ],
    dias_letra: ["ن", "ث", "ر", "خ", "ج", "س", "ح"],
    primer_dia: 6,
    orden: Orden::DiaMesAno,
    separador: "/",
    largo: "{d} {m} {a}",
    largo_con_dia: "{s}، {d} {m} {a}",
    semana: "الأسبوع {n}",
    rtl: true,
};

const PT: Locale = Locale {
    etiqueta: "pt-BR",
    meses: [
        "janeiro",
        "fevereiro",
        "março",
        "abril",
        "maio",
        "junho",
        "julho",
        "agosto",
        "setembro",
        "outubro",
        "novembro",
        "dezembro",
    ],
    meses_fecha: None,
    meses_cortos: [
        "jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez",
    ],
    dias: [
        "segunda-feira",
        "terça-feira",
        "quarta-feira",
        "quinta-feira",
        "sexta-feira",
        "sábado",
        "domingo",
    ],
    dias_cortos: ["seg", "ter", "qua", "qui", "sex", "sáb", "dom"],
    dias_letra: ["S", "T", "Q", "Q", "S", "S", "D"],
    primer_dia: 6,
    orden: Orden::DiaMesAno,
    separador: "/",
    largo: "{d} de {m} de {a}",
    largo_con_dia: "{s}, {d} de {m} de {a}",
    semana: "Semana {n}",
    rtl: false,
};

const FR: Locale = Locale {
    etiqueta: "fr-FR",
    meses: [
        "janvier",
        "février",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "août",
        "septembre",
        "octobre",
        "novembre",
        "décembre",
    ],
    meses_fecha: None,
    meses_cortos: [
        "janv", "févr", "mars", "avr", "mai", "juin", "juil", "août", "sept", "oct", "nov", "déc",
    ],
    dias: [
        "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche",
    ],
    dias_cortos: ["lun", "mar", "mer", "jeu", "ven", "sam", "dim"],
    dias_letra: ["L", "M", "M", "J", "V", "S", "D"],
    primer_dia: 0,
    orden: Orden::DiaMesAno,
    separador: "/",
    largo: "{d} {m} {a}",
    largo_con_dia: "{s} {d} {m} {a}",
    semana: "Semaine {n}",
    rtl: false,
};

const DE: Locale = Locale {
    etiqueta: "de-DE",
    meses: [
        "Januar",
        "Februar",
        "März",
        "April",
        "Mai",
        "Juni",
        "Juli",
        "August",
        "September",
        "Oktober",
        "November",
        "Dezember",
    ],
    meses_fecha: None,
    meses_cortos: [
        "Jan", "Feb", "März", "Apr", "Mai", "Juni", "Juli", "Aug", "Sept", "Okt", "Nov", "Dez",
    ],
    dias: [
        "Montag",
        "Dienstag",
        "Mittwoch",
        "Donnerstag",
        "Freitag",
        "Samstag",
        "Sonntag",
    ],
    dias_cortos: ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"],
    dias_letra: ["M", "D", "M", "D", "F", "S", "S"],
    primer_dia: 0,
    orden: Orden::DiaMesAno,
    separador: ".",
    largo: "{d}. {m} {a}",
    largo_con_dia: "{s}, {d}. {m} {a}",
    semana: "Woche {n}",
    rtl: false,
};

const JA: Locale = Locale {
    etiqueta: "ja-JP",
    meses: [
        "1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月",
    ],
    meses_fecha: None,
    meses_cortos: [
        "1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月",
    ],
    dias: [
        "月曜日",
        "火曜日",
        "水曜日",
        "木曜日",
        "金曜日",
        "土曜日",
        "日曜日",
    ],
    dias_cortos: ["月", "火", "水", "木", "金", "土", "日"],
    dias_letra: ["月", "火", "水", "木", "金", "土", "日"],
    primer_dia: 6,
    orden: Orden::AnoMesDia,
    separador: "/",
    largo: "{a}年{m}{d}日",
    largo_con_dia: "{a}年{m}{d}日 {s}",
    semana: "第{n}週",
    rtl: false,
};

const RU: Locale = Locale {
    etiqueta: "ru-RU",
    meses: [
        "январь",
        "февраль",
        "март",
        "апрель",
        "май",
        "июнь",
        "июль",
        "август",
        "сентябрь",
        "октябрь",
        "ноябрь",
        "декабрь",
    ],
    meses_fecha: Some([
        "января",
        "февраля",
        "марта",
        "апреля",
        "мая",
        "июня",
        "июля",
        "августа",
        "сентября",
        "октября",
        "ноября",
        "декабря",
    ]),
    meses_cortos: [
        "янв", "фев", "мар", "апр", "май", "июн", "июл", "авг", "сен", "окт", "ноя", "дек",
    ],
    dias: [
        "понедельник",
        "вторник",
        "среда",
        "четверг",
        "пятница",
        "суббота",
        "воскресенье",
    ],
    dias_cortos: ["пн", "вт", "ср", "чт", "пт", "сб", "вс"],
    dias_letra: ["П", "В", "С", "Ч", "П", "С", "В"],
    primer_dia: 0,
    orden: Orden::DiaMesAno,
    separador: ".",
    largo: "{d} {m} {a} г.",
    largo_con_dia: "{s}, {d} {m} {a} г.",
    semana: "Неделя {n}",
    rtl: false,
};

const KO: Locale = Locale {
    etiqueta: "ko-KR",
    meses: [
        "1월", "2월", "3월", "4월", "5월", "6월", "7월", "8월", "9월", "10월", "11월", "12월",
    ],
    meses_fecha: None,
    meses_cortos: [
        "1월", "2월", "3월", "4월", "5월", "6월", "7월", "8월", "9월", "10월", "11월", "12월",
    ],
    dias: [
        "월요일",
        "화요일",
        "수요일",
        "목요일",
        "금요일",
        "토요일",
        "일요일",
    ],
    dias_cortos: ["월", "화", "수", "목", "금", "토", "일"],
    dias_letra: ["월", "화", "수", "목", "금", "토", "일"],
    primer_dia: 6,
    orden: Orden::AnoMesDia,
    separador: ". ",
    largo: "{a}년 {m} {d}일",
    largo_con_dia: "{a}년 {m} {d}일 {s}",
    semana: "{n}주차",
    rtl: false,
};

const IT: Locale = Locale {
    etiqueta: "it-IT",
    meses: [
        "gennaio",
        "febbraio",
        "marzo",
        "aprile",
        "maggio",
        "giugno",
        "luglio",
        "agosto",
        "settembre",
        "ottobre",
        "novembre",
        "dicembre",
    ],
    meses_fecha: None,
    meses_cortos: [
        "gen", "feb", "mar", "apr", "mag", "giu", "lug", "ago", "set", "ott", "nov", "dic",
    ],
    dias: [
        "lunedì",
        "martedì",
        "mercoledì",
        "giovedì",
        "venerdì",
        "sabato",
        "domenica",
    ],
    dias_cortos: ["lun", "mar", "mer", "gio", "ven", "sab", "dom"],
    dias_letra: ["L", "M", "M", "G", "V", "S", "D"],
    primer_dia: 0,
    orden: Orden::DiaMesAno,
    separador: "/",
    largo: "{d} {m} {a}",
    largo_con_dia: "{s} {d} {m} {a}",
    semana: "Settimana {n}",
    rtl: false,
};

const ID: Locale = Locale {
    etiqueta: "id-ID",
    meses: [
        "Januari",
        "Februari",
        "Maret",
        "April",
        "Mei",
        "Juni",
        "Juli",
        "Agustus",
        "September",
        "Oktober",
        "November",
        "Desember",
    ],
    meses_fecha: None,
    meses_cortos: [
        "Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des",
    ],
    dias: [
        "Senin", "Selasa", "Rabu", "Kamis", "Jumat", "Sabtu", "Minggu",
    ],
    dias_cortos: ["Sen", "Sel", "Rab", "Kam", "Jum", "Sab", "Min"],
    dias_letra: ["S", "S", "R", "K", "J", "S", "M"],
    primer_dia: 6,
    orden: Orden::DiaMesAno,
    separador: "/",
    largo: "{d} {m} {a}",
    largo_con_dia: "{s}, {d} {m} {a}",
    semana: "Minggu ke-{n}",
    rtl: false,
};

const TR: Locale = Locale {
    etiqueta: "tr-TR",
    meses: [
        "Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim",
        "Kasım", "Aralık",
    ],
    meses_fecha: None,
    meses_cortos: [
        "Oca", "Şub", "Mar", "Nis", "May", "Haz", "Tem", "Ağu", "Eyl", "Eki", "Kas", "Ara",
    ],
    dias: [
        "Pazartesi",
        "Salı",
        "Çarşamba",
        "Perşembe",
        "Cuma",
        "Cumartesi",
        "Pazar",
    ],
    dias_cortos: ["Pzt", "Sal", "Çar", "Per", "Cum", "Cmt", "Paz"],
    dias_letra: ["P", "S", "Ç", "P", "C", "C", "P"],
    primer_dia: 0,
    orden: Orden::DiaMesAno,
    separador: ".",
    largo: "{d} {m} {a}",
    largo_con_dia: "{d} {m} {a} {s}",
    semana: "{n}. hafta",
    rtl: false,
};

/// Los catorce, en el orden de `Idioma::TODOS`.
pub const TODOS: [&Locale; 14] = [
    &ES, &EN, &ZH, &HI, &AR, &PT, &FR, &DE, &JA, &RU, &KO, &IT, &ID, &TR,
];

/// **El de una etiqueta** (`es`, `es-MX`, `pt_PT`, `zh-Hans`…), por su parte
/// de idioma; sin parecido, ingles.
pub fn de(etiqueta: &str) -> &'static Locale {
    let idioma = etiqueta
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    TODOS
        .iter()
        .copied()
        .find(|l| l.etiqueta.split('-').next() == Some(idioma.as_str()))
        .unwrap_or(&EN)
}

/// Como se pinta una fecha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatoFecha {
    /// 06/11/2026 (en el orden del idioma).
    #[default]
    Corto,
    /// 6 de noviembre de 2026.
    Largo,
    /// viernes, 6 de noviembre de 2026.
    Completo,
    /// 2026-11-06.
    Iso,
    /// Solo el numero (dia, semana, mes, año).
    Numero,
    /// El nombre abreviado (dia de la semana, mes).
    Abreviado,
    /// La inicial (dia de la semana).
    Letra,
}

impl Locale {
    pub fn mes(&self, mes: u32) -> &'static str {
        self.meses[(mes.clamp(1, 12) - 1) as usize]
    }

    fn mes_en_fecha(&self, mes: u32) -> &'static str {
        let i = (mes.clamp(1, 12) - 1) as usize;
        self.meses_fecha.map_or(self.meses[i], |m| m[i])
    }

    pub fn dia(&self, d: DiaSemana) -> &'static str {
        self.dias[(d % 7) as usize]
    }

    /// Los siete dias en el orden de la semana de este idioma, con su
    /// numero (0 lunes).
    pub fn semana_ordenada(&self) -> [DiaSemana; 7] {
        let mut v = [0; 7];
        for (i, x) in v.iter_mut().enumerate() {
            *x = (self.primer_dia + i as u32) % 7;
        }
        v
    }

    /// **Una fecha** en su formato.
    pub fn fecha(&self, f: Fecha, formato: FormatoFecha) -> String {
        match formato {
            FormatoFecha::Iso => f.to_string(),
            FormatoFecha::Numero => f.dia.to_string(),
            FormatoFecha::Abreviado => {
                format!("{} {}", f.dia, self.meses_cortos[f.mes as usize - 1])
            }
            FormatoFecha::Letra => self.dias_letra[f.dia_semana() as usize].to_string(),
            FormatoFecha::Corto => {
                let s = self.separador;
                match self.orden {
                    Orden::DiaMesAno => format!("{:02}{s}{:02}{s}{}", f.dia, f.mes, f.ano),
                    Orden::MesDiaAno => format!("{}{s}{}{s}{}", f.mes, f.dia, f.ano),
                    Orden::AnoMesDia => format!("{}{s}{:02}{s}{:02}", f.ano, f.mes, f.dia),
                }
            }
            FormatoFecha::Largo | FormatoFecha::Completo => {
                let patron = if formato == FormatoFecha::Largo {
                    self.largo
                } else {
                    self.largo_con_dia
                };
                patron
                    .replace("{s}", self.dia(f.dia_semana()))
                    .replace("{m}", self.mes_en_fecha(f.mes))
                    .replace("{d}", &f.dia.to_string())
                    .replace("{a}", &f.ano.to_string())
            }
        }
    }

    /// El dia de la semana.
    pub fn dia_semana(&self, f: Fecha, formato: FormatoFecha) -> String {
        let d = f.dia_semana() as usize;
        match formato {
            FormatoFecha::Abreviado | FormatoFecha::Corto => self.dias_cortos[d].to_string(),
            FormatoFecha::Letra => self.dias_letra[d].to_string(),
            FormatoFecha::Numero => (d + 1).to_string(),
            _ => self.dias[d].to_string(),
        }
    }

    /// El mes.
    pub fn mes_de(&self, f: Fecha, formato: FormatoFecha) -> String {
        match formato {
            FormatoFecha::Numero => f.mes.to_string(),
            FormatoFecha::Abreviado | FormatoFecha::Letra => {
                self.meses_cortos[f.mes as usize - 1].to_string()
            }
            FormatoFecha::Completo | FormatoFecha::Largo => {
                format!("{} {}", self.mes(f.mes), f.ano)
            }
            _ => self.mes(f.mes).to_string(),
        }
    }

    /// **El titulo de un mes** («Enero 2027», «2027年1月», «2027년 1월»).
    pub fn titulo_mes(&self, f: Fecha) -> String {
        let m = self.mes_en_fecha(f.mes);
        match self.orden {
            Orden::AnoMesDia if self.etiqueta.starts_with("ko") => format!("{}년 {m}", f.ano),
            Orden::AnoMesDia => format!("{}年{m}", f.ano),
            _ => {
                let mut c = self.mes(f.mes).chars();
                let capital: String = c
                    .next()
                    .map(|p| p.to_uppercase().chain(c).collect())
                    .unwrap_or_default();
                format!("{capital} {}", f.ano)
            }
        }
    }

    /// «Semana 45».
    pub fn semana(&self, n: u32) -> String {
        self.semana.replace("{n}", &n.to_string())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn f(a: i32, m: u32, d: u32) -> Fecha {
        Fecha::nueva(a, m, d).unwrap()
    }

    #[test]
    fn la_misma_fecha_en_varios_idiomas() {
        let v = f(2026, 11, 6);
        assert_eq!(
            de("es").fecha(v, FormatoFecha::Largo),
            "6 de noviembre de 2026"
        );
        assert_eq!(
            de("es-MX").fecha(v, FormatoFecha::Completo),
            "viernes, 6 de noviembre de 2026"
        );
        assert_eq!(
            de("en-US").fecha(v, FormatoFecha::Largo),
            "November 6, 2026"
        );
        assert_eq!(de("en").fecha(v, FormatoFecha::Corto), "11/6/2026");
        assert_eq!(de("es").fecha(v, FormatoFecha::Corto), "06/11/2026");
        assert_eq!(de("de").fecha(v, FormatoFecha::Corto), "06.11.2026");
        assert_eq!(de("de").fecha(v, FormatoFecha::Largo), "6. November 2026");
        assert_eq!(de("ja").fecha(v, FormatoFecha::Largo), "2026年11月6日");
        assert_eq!(de("zh-Hans").fecha(v, FormatoFecha::Largo), "2026年11月6日");
        assert_eq!(de("ru").fecha(v, FormatoFecha::Largo), "6 ноября 2026 г.");
        assert_eq!(de("ko").fecha(v, FormatoFecha::Largo), "2026년 11월 6일");
        assert_eq!(
            de("fr").fecha(v, FormatoFecha::Completo),
            "vendredi 6 novembre 2026"
        );
        assert_eq!(
            de("pt_PT").dia_semana(v, FormatoFecha::Largo),
            "sexta-feira"
        );
        assert_eq!(de("tr").mes_de(v, FormatoFecha::Largo), "Kasım 2026");
        assert_eq!(de("xx").etiqueta, "en-US", "lo desconocido, en ingles");
        assert_eq!(de("es").fecha(v, FormatoFecha::Iso), "2026-11-06");
    }

    #[test]
    fn la_semana_empieza_donde_toca() {
        assert_eq!(de("es").semana_ordenada()[0], 0);
        assert_eq!(de("en-US").semana_ordenada()[0], 6);
        assert_eq!(de("en-US").semana_ordenada()[1], 0);
        assert_eq!(de("ja").dias_cortos[6], "日");
        assert_eq!(de("es").semana(45), "Semana 45");
        assert_eq!(de("ko").semana(3), "3주차");
        assert!(de("ar").rtl && !de("he").rtl);
    }

    #[test]
    fn cada_idioma_tiene_todo() {
        for l in TODOS {
            assert!(l.meses.iter().all(|m| !m.is_empty()), "{}", l.etiqueta);
            assert!(l.dias.iter().all(|m| !m.is_empty()), "{}", l.etiqueta);
            assert!(l.largo.contains("{d}") && l.largo.contains("{a}"));
            assert!(l.largo_con_dia.contains("{s}"));
            assert!(l.semana.contains("{n}"));
            assert!(l.primer_dia == 0 || l.primer_dia == 6);
        }
    }
}
