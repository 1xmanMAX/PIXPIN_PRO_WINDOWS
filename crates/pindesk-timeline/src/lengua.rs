//! **Las fechas en cada idioma de la interfaz.**
//!
//! Este crate es cimiento y no lee catalogos de Fluent: los nombres de los
//! meses y de los dias, y como se escribe una fecha, van en una tabla por
//! idioma, con los datos de CLDR (la forma «de calendario» del mes donde va
//! suelto y, en ruso, el genitivo dentro de una fecha: «5 октября»).
//!
//! Los catorce idiomas son los de `pindesk_store::Idioma`, por su codigo ISO
//! (`Lengua::de_codigo`). Un `bool` sigue valiendo donde antes se pedia
//! `ingles: bool` (`true` ingles, `false` español): las fechas de español e
//! ingles no cambian ni una letra.

/// El idioma de una fecha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Lengua {
    #[default]
    Es,
    En,
    Zh,
    Hi,
    Ar,
    Pt,
    Fr,
    De,
    Ja,
    Ru,
    Ko,
    It,
    Id,
    Tr,
}

impl From<bool> for Lengua {
    /// El `ingles: bool` de antes.
    fn from(ingles: bool) -> Self {
        if ingles { Lengua::En } else { Lengua::Es }
    }
}

impl Lengua {
    /// La de un codigo ISO 639-1 (`es`, `ja`…) o una etiqueta (`pt-BR`).
    /// Sin parecido, ingles.
    pub fn de_codigo(codigo: &str) -> Lengua {
        let base = codigo
            .split(['-', '_'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        match base.as_str() {
            "es" => Lengua::Es,
            "zh" => Lengua::Zh,
            "hi" => Lengua::Hi,
            "ar" => Lengua::Ar,
            "pt" => Lengua::Pt,
            "fr" => Lengua::Fr,
            "de" => Lengua::De,
            "ja" => Lengua::Ja,
            "ru" => Lengua::Ru,
            "ko" => Lengua::Ko,
            "it" => Lengua::It,
            "id" => Lengua::Id,
            "tr" => Lengua::Tr,
            _ => Lengua::En,
        }
    }

    /// El codigo ISO 639-1: el `lang` de la pagina exportada.
    pub fn codigo(self) -> &'static str {
        match self {
            Lengua::Es => "es",
            Lengua::En => "en",
            Lengua::Zh => "zh",
            Lengua::Hi => "hi",
            Lengua::Ar => "ar",
            Lengua::Pt => "pt",
            Lengua::Fr => "fr",
            Lengua::De => "de",
            Lengua::Ja => "ja",
            Lengua::Ru => "ru",
            Lengua::Ko => "ko",
            Lengua::It => "it",
            Lengua::Id => "id",
            Lengua::Tr => "tr",
        }
    }

    pub(crate) fn tabla(self) -> &'static Tabla {
        match self {
            Lengua::Es => &ES,
            Lengua::En => &EN,
            Lengua::Zh => &ZH,
            Lengua::Hi => &HI,
            Lengua::Ar => &AR,
            Lengua::Pt => &PT,
            Lengua::Fr => &FR,
            Lengua::De => &DE,
            Lengua::Ja => &JA,
            Lengua::Ru => &RU,
            Lengua::Ko => &KO,
            Lengua::It => &IT,
            Lengua::Id => &ID,
            Lengua::Tr => &TR,
        }
    }
}

/// Como se escribe una fecha: las piezas son `{s}` dia de la semana, `{d}`
/// dia del mes, `{m}` mes (en la forma de dentro de una fecha), `{c}` mes
/// corto, `{a}` año. Se sustituyen a mano: son cinco y siempre las mismas.
pub(crate) struct Tabla {
    /// El mes suelto («octubre», «Октябрь»): rotulos y calendario.
    pub meses: [&'static str; 12],
    /// El mes dentro de una fecha: igual salvo en ruso (genitivo).
    pub meses_en_fecha: [&'static str; 12],
    pub meses_cortos: [&'static str; 12],
    /// De lunes a domingo.
    pub semana: [&'static str; 7],
    pub semana_corta: [&'static str; 7],
    pub iniciales: [&'static str; 7],
    /// «octubre de 2026»: `{m}` y `{a}`.
    pub mes_y_anio: &'static str,
    /// «Lunes 5 de octubre de 2026»: `{s}`, `{d}`, `{m}`, `{a}`.
    pub dia_entero: &'static str,
    /// Si la fecha entera empieza por el dia de la semana con mayuscula
    /// (los idiomas que lo escriben en minuscula: «lunes» → «Lunes»).
    pub mayuscula: bool,
    /// «5 oct 2026»: `{d}`, `{c}`, `{a}`.
    pub fecha_corta: &'static str,
}

impl Tabla {
    pub(crate) fn rellenar(
        patron: &str,
        semana: &str,
        dia: u8,
        mes: &str,
        corto: &str,
        anio: i32,
    ) -> String {
        patron
            .replace("{s}", semana)
            .replace("{d}", &dia.to_string())
            .replace("{m}", mes)
            .replace("{c}", corto)
            .replace("{a}", &anio.to_string())
    }
}

const NUMEROS_CJK: [&str; 12] = [
    "1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月",
];
const NUMEROS_KO: [&str; 12] = [
    "1월", "2월", "3월", "4월", "5월", "6월", "7월", "8월", "9월", "10월", "11월", "12월",
];

const ES_MESES: [&str; 12] = [
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
const EN_MESES: [&str; 12] = [
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
];
const HI_MESES: [&str; 12] = [
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
];
const AR_MESES: [&str; 12] = [
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
];
const AR_SEMANA: [&str; 7] = [
    "الاثنين",
    "الثلاثاء",
    "الأربعاء",
    "الخميس",
    "الجمعة",
    "السبت",
    "الأحد",
];
const PT_MESES: [&str; 12] = [
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
];
const FR_MESES: [&str; 12] = [
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
];
const DE_MESES: [&str; 12] = [
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
];
const IT_MESES: [&str; 12] = [
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
];
const ID_MESES: [&str; 12] = [
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
];
const TR_MESES: [&str; 12] = [
    "Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim",
    "Kasım", "Aralık",
];

static ES: Tabla = Tabla {
    meses: ES_MESES,
    meses_en_fecha: ES_MESES,
    meses_cortos: [
        "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic",
    ],
    semana: [
        "lunes",
        "martes",
        "miércoles",
        "jueves",
        "viernes",
        "sábado",
        "domingo",
    ],
    semana_corta: ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"],
    iniciales: ["L", "M", "X", "J", "V", "S", "D"],
    mes_y_anio: "{m} de {a}",
    dia_entero: "{s} {d} de {m} de {a}",
    mayuscula: true,
    fecha_corta: "{d} {c} {a}",
};

static EN: Tabla = Tabla {
    meses: EN_MESES,
    meses_en_fecha: EN_MESES,
    meses_cortos: [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ],
    semana: [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ],
    semana_corta: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
    iniciales: ["M", "T", "W", "T", "F", "S", "S"],
    mes_y_anio: "{m} {a}",
    dia_entero: "{s}, {m} {d}, {a}",
    mayuscula: false,
    fecha_corta: "{c} {d}, {a}",
};

static ZH: Tabla = Tabla {
    meses: NUMEROS_CJK,
    meses_en_fecha: NUMEROS_CJK,
    meses_cortos: NUMEROS_CJK,
    semana: [
        "星期一",
        "星期二",
        "星期三",
        "星期四",
        "星期五",
        "星期六",
        "星期日",
    ],
    semana_corta: ["周一", "周二", "周三", "周四", "周五", "周六", "周日"],
    iniciales: ["一", "二", "三", "四", "五", "六", "日"],
    mes_y_anio: "{a}年{m}",
    dia_entero: "{a}年{m}{d}日{s}",
    mayuscula: false,
    fecha_corta: "{a}年{c}{d}日",
};

static JA: Tabla = Tabla {
    meses: NUMEROS_CJK,
    meses_en_fecha: NUMEROS_CJK,
    meses_cortos: NUMEROS_CJK,
    semana: [
        "月曜日",
        "火曜日",
        "水曜日",
        "木曜日",
        "金曜日",
        "土曜日",
        "日曜日",
    ],
    semana_corta: ["月", "火", "水", "木", "金", "土", "日"],
    iniciales: ["月", "火", "水", "木", "金", "土", "日"],
    mes_y_anio: "{a}年{m}",
    dia_entero: "{a}年{m}{d}日{s}",
    mayuscula: false,
    fecha_corta: "{a}年{c}{d}日",
};

static KO: Tabla = Tabla {
    meses: NUMEROS_KO,
    meses_en_fecha: NUMEROS_KO,
    meses_cortos: NUMEROS_KO,
    semana: [
        "월요일",
        "화요일",
        "수요일",
        "목요일",
        "금요일",
        "토요일",
        "일요일",
    ],
    semana_corta: ["월", "화", "수", "목", "금", "토", "일"],
    iniciales: ["월", "화", "수", "목", "금", "토", "일"],
    mes_y_anio: "{a}년 {m}",
    dia_entero: "{a}년 {m} {d}일 {s}",
    mayuscula: false,
    fecha_corta: "{a}년 {c} {d}일",
};

static HI: Tabla = Tabla {
    meses: HI_MESES,
    meses_en_fecha: HI_MESES,
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
    semana: [
        "सोमवार",
        "मंगलवार",
        "बुधवार",
        "गुरुवार",
        "शुक्रवार",
        "शनिवार",
        "रविवार",
    ],
    semana_corta: ["सोम", "मंगल", "बुध", "गुरु", "शुक्र", "शनि", "रवि"],
    iniciales: ["सो", "मं", "बु", "गु", "शु", "श", "र"],
    mes_y_anio: "{m} {a}",
    dia_entero: "{s}, {d} {m} {a}",
    mayuscula: false,
    fecha_corta: "{d} {c} {a}",
};

static AR: Tabla = Tabla {
    meses: AR_MESES,
    meses_en_fecha: AR_MESES,
    // En arabe el mes corto es el mismo nombre (CLDR).
    meses_cortos: AR_MESES,
    semana: AR_SEMANA,
    semana_corta: AR_SEMANA,
    iniciales: ["ن", "ث", "ر", "خ", "ج", "س", "ح"],
    mes_y_anio: "{m} {a}",
    dia_entero: "{s}، {d} {m} {a}",
    mayuscula: false,
    fecha_corta: "{d} {c} {a}",
};

static PT: Tabla = Tabla {
    meses: PT_MESES,
    meses_en_fecha: PT_MESES,
    meses_cortos: [
        "jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez",
    ],
    semana: [
        "segunda-feira",
        "terça-feira",
        "quarta-feira",
        "quinta-feira",
        "sexta-feira",
        "sábado",
        "domingo",
    ],
    semana_corta: ["seg", "ter", "qua", "qui", "sex", "sáb", "dom"],
    iniciales: ["S", "T", "Q", "Q", "S", "S", "D"],
    mes_y_anio: "{m} de {a}",
    dia_entero: "{s}, {d} de {m} de {a}",
    mayuscula: true,
    fecha_corta: "{d} de {c} de {a}",
};

static FR: Tabla = Tabla {
    meses: FR_MESES,
    meses_en_fecha: FR_MESES,
    meses_cortos: [
        "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
        "déc.",
    ],
    semana: [
        "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche",
    ],
    semana_corta: ["lun.", "mar.", "mer.", "jeu.", "ven.", "sam.", "dim."],
    iniciales: ["L", "M", "M", "J", "V", "S", "D"],
    mes_y_anio: "{m} {a}",
    dia_entero: "{s} {d} {m} {a}",
    mayuscula: true,
    fecha_corta: "{d} {c} {a}",
};

static DE: Tabla = Tabla {
    meses: DE_MESES,
    meses_en_fecha: DE_MESES,
    meses_cortos: [
        "Jan.", "Feb.", "März", "Apr.", "Mai", "Juni", "Juli", "Aug.", "Sept.", "Okt.", "Nov.",
        "Dez.",
    ],
    semana: [
        "Montag",
        "Dienstag",
        "Mittwoch",
        "Donnerstag",
        "Freitag",
        "Samstag",
        "Sonntag",
    ],
    semana_corta: ["Mo.", "Di.", "Mi.", "Do.", "Fr.", "Sa.", "So."],
    iniciales: ["M", "D", "M", "D", "F", "S", "S"],
    mes_y_anio: "{m} {a}",
    dia_entero: "{s}, {d}. {m} {a}",
    mayuscula: false,
    fecha_corta: "{d}. {c} {a}",
};

static RU: Tabla = Tabla {
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
    meses_en_fecha: [
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
    ],
    meses_cortos: [
        "янв.",
        "февр.",
        "мар.",
        "апр.",
        "мая",
        "июн.",
        "июл.",
        "авг.",
        "сент.",
        "окт.",
        "нояб.",
        "дек.",
    ],
    semana: [
        "понедельник",
        "вторник",
        "среда",
        "четверг",
        "пятница",
        "суббота",
        "воскресенье",
    ],
    semana_corta: ["пн", "вт", "ср", "чт", "пт", "сб", "вс"],
    iniciales: ["П", "В", "С", "Ч", "П", "С", "В"],
    mes_y_anio: "{m} {a} г.",
    dia_entero: "{s}, {d} {m} {a} г.",
    mayuscula: true,
    fecha_corta: "{d} {c} {a} г.",
};

static IT: Tabla = Tabla {
    meses: IT_MESES,
    meses_en_fecha: IT_MESES,
    meses_cortos: [
        "gen", "feb", "mar", "apr", "mag", "giu", "lug", "ago", "set", "ott", "nov", "dic",
    ],
    semana: [
        "lunedì",
        "martedì",
        "mercoledì",
        "giovedì",
        "venerdì",
        "sabato",
        "domenica",
    ],
    semana_corta: ["lun", "mar", "mer", "gio", "ven", "sab", "dom"],
    iniciales: ["L", "M", "M", "G", "V", "S", "D"],
    mes_y_anio: "{m} {a}",
    dia_entero: "{s} {d} {m} {a}",
    mayuscula: true,
    fecha_corta: "{d} {c} {a}",
};

static ID: Tabla = Tabla {
    meses: ID_MESES,
    meses_en_fecha: ID_MESES,
    meses_cortos: [
        "Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des",
    ],
    semana: [
        "Senin", "Selasa", "Rabu", "Kamis", "Jumat", "Sabtu", "Minggu",
    ],
    semana_corta: ["Sen", "Sel", "Rab", "Kam", "Jum", "Sab", "Min"],
    iniciales: ["S", "S", "R", "K", "J", "S", "M"],
    mes_y_anio: "{m} {a}",
    dia_entero: "{s}, {d} {m} {a}",
    mayuscula: false,
    fecha_corta: "{d} {c} {a}",
};

static TR: Tabla = Tabla {
    meses: TR_MESES,
    meses_en_fecha: TR_MESES,
    meses_cortos: [
        "Oca", "Şub", "Mar", "Nis", "May", "Haz", "Tem", "Ağu", "Eyl", "Eki", "Kas", "Ara",
    ],
    semana: [
        "Pazartesi",
        "Salı",
        "Çarşamba",
        "Perşembe",
        "Cuma",
        "Cumartesi",
        "Pazar",
    ],
    semana_corta: ["Pzt", "Sal", "Çar", "Per", "Cum", "Cmt", "Paz"],
    iniciales: ["P", "S", "Ç", "P", "C", "C", "P"],
    mes_y_anio: "{m} {a}",
    dia_entero: "{d} {m} {a} {s}",
    mayuscula: false,
    fecha_corta: "{d} {c} {a}",
};

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_bool_es_el_ingles_o_el_espanol_de_antes() {
        assert_eq!(Lengua::from(true), Lengua::En);
        assert_eq!(Lengua::from(false), Lengua::Es);
        assert_eq!(Lengua::de_codigo("pt-BR"), Lengua::Pt);
        assert_eq!(Lengua::de_codigo("zh"), Lengua::Zh);
        assert_eq!(Lengua::de_codigo("sw"), Lengua::En);
        for l in [
            Lengua::Es,
            Lengua::En,
            Lengua::Zh,
            Lengua::Hi,
            Lengua::Ar,
            Lengua::Pt,
            Lengua::Fr,
            Lengua::De,
            Lengua::Ja,
            Lengua::Ru,
            Lengua::Ko,
            Lengua::It,
            Lengua::Id,
            Lengua::Tr,
        ] {
            assert_eq!(Lengua::de_codigo(l.codigo()), l);
            let t = l.tabla();
            assert!(t.meses.iter().chain(t.semana.iter()).all(|n| !n.is_empty()));
        }
    }
}
