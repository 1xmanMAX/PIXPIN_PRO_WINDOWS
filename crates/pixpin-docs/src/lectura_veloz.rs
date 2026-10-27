//! **Lectura Veloz** (27-oct-2026): el puente, en los dos sentidos, con la
//! app de lectura rapida del usuario (Tauri, esquema `lecturaveloz://`).
//! El protocolo entero, con ejemplos y como probarlo, esta en
//! `docs/integracion-lectura-veloz.md`; aqui va lo puro, para probarlo sin
//! ventanas:
//!
//! - **De PixPin a Lectura Veloz** ([`Salida`]): «⚡ Lectura veloz» de los
//!   lectores arma `lecturaveloz://pixpin/abrir?v=1&doc=…&ruta=…` con el
//!   documento y por donde se va (el 🟢 si lo hay).
//! - **De Lectura Veloz a PixPin** ([`Progreso`]): el verbo
//!   `pixpinmax://lectura-veloz?v=1&doc=…&ruta=…&tipo=…` mueve el marcador
//!   verde a donde se va leyendo alli. Llega de fuera, asi que aqui se mira
//!   **todo** antes de tocar nada: una ruta local con extension de
//!   documento, numeros finitos y acotados, textos cortos. Que la ruta
//!   exista y sea un fichero lo mira quien escribe (esto no toca el disco).
//!
//! Los nombres de los parametros son los de la version 1 del protocolo y
//! no se cambian: Lectura Veloz los usa tal cual.

use std::collections::BTreeMap;
use std::path::Path;

use crate::lectura::Ajustes;
use crate::voz_alta;

/// La version del protocolo (`v=`).
pub const VERSION: u32 = 1;

/// El esquema de Lectura Veloz, sin `://`.
pub const ESQUEMA: &str = "lecturaveloz";

/// El prefijo del `doc` de un fichero que no es un adjunto del chat: el
/// resto es una huella estable de su ruta ([`id_de_ruta`]).
pub const PREFIJO_RUTA: &str = "ruta:";

/// Lo mas largo que se acepta en `doc` (un uid del chat son unas decenas de
/// letras; `ruta:` mas la huella, 21).
pub const TOPE_DOC: usize = 200;

/// Lo mas largo de una ruta que se acepta (el `MAX_PATH` largo de Windows).
pub const TOPE_RUTA: usize = 32_767;

/// La pagina mas alta que se cree: un PDF de cien mil hojas ya es raro.
pub const TOPE_PAGINA: u32 = 100_000;

/// **Las extensiones de los documentos que PixPin lee** y cuyo sitio de
/// lectura se deja mover desde fuera. Solo documentos: nada de codigo,
/// ajustes ni ejecutables, aunque el visor los abra como texto.
pub const EXTENSIONES: [&str; 20] = [
    "pdf", "epub", "docx", "docm", "dotx", "dotm", "doc", "pptx", "pptm", "ppsx", "ppsm", "potx",
    "potm", "html", "htm", "xhtml", "md", "markdown", "txt", "rtf",
];

/// Que lee Lectura Veloz, y con que cuenta va la posicion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    /// Por hojas: `pagina` (desde 0) y `frac`, la parte de su alto.
    Pdf,
    /// De corrido: `fraccion` del documento entero.
    Texto,
}

impl Tipo {
    pub fn como_texto(self) -> &'static str {
        match self {
            Tipo::Pdf => "pdf",
            Tipo::Texto => "texto",
        }
    }

    pub fn de_texto(t: &str) -> Option<Tipo> {
        match t.trim().to_ascii_lowercase().as_str() {
            "pdf" => Some(Tipo::Pdf),
            "texto" => Some(Tipo::Texto),
            _ => None,
        }
    }
}

/// La extension de una ruta en minusculas («» si no tiene).
fn extension_de(ruta: &str) -> String {
    let nombre = ruta.rsplit(['\\', '/']).next().unwrap_or(ruta);
    match nombre.rsplit_once('.') {
        Some((antes, e)) if !antes.is_empty() => e.to_ascii_lowercase(),
        _ => String::new(),
    }
}

/// Si la ruta es de un documento que PixPin lee (por su extension).
pub fn es_documento(ruta: &Path) -> bool {
    EXTENSIONES.contains(&extension_de(&ruta.to_string_lossy()).as_str())
}

/// El tipo con que se le pasa a Lectura Veloz un documento: un PDF por
/// hojas; lo demas, de corrido. `None` si no es un documento.
pub fn tipo_de(ruta: &Path) -> Option<Tipo> {
    match extension_de(&ruta.to_string_lossy()).as_str() {
        "pdf" => Some(Tipo::Pdf),
        e if EXTENSIONES.contains(&e) => Some(Tipo::Texto),
        _ => None,
    }
}

/// **Una ruta absoluta de una unidad de este equipo** (`C:\…`): ni de red
/// (`\\servidor\…`), ni de dispositivo (`\\?\`, `\\.\`), ni relativa, ni
/// con un nulo dentro. La misma regla que `pixpin_shell::orden::es_ruta_local`
/// (este crate no depende de Windows); ademas, sin `..` por el camino.
pub fn es_ruta_local(r: &str) -> bool {
    let b = r.as_bytes();
    b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b[2] == b'\\' || b[2] == b'/')
        && !r.contains('\0')
        && !r.split(['\\', '/']).any(|t| t == "..")
}

/// Todo lo que no sea letra, cifra o `-_.~`, en `%XX` (los bytes de UTF-8).
/// Lo mismo que `pixpin_shell::orden::codificar`.
pub fn codificar(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    for b in t.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

/// **El `doc` de un fichero suelto**: `ruta:` y la huella FNV-1a de 64 bits
/// de su ruta canonica, en minusculas y con `\` (Windows no distingue
/// mayusculas, y `\\?\` es la misma ruta). Estable entre arranques y entre
/// versiones: Lectura Veloz la guarda.
pub fn id_de_ruta(canonica: &str) -> String {
    let limpia = canonica
        .strip_prefix(r"\\?\")
        .unwrap_or(canonica)
        .replace('/', "\\")
        .to_lowercase();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in limpia.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{PREFIJO_RUTA}{h:016x}")
}

/// Una fraccion de 0 a 1 (lo que no es un numero, 0).
fn acotada(f: f64) -> f32 {
    if f.is_finite() {
        f.clamp(0.0, 1.0) as f32
    } else {
        0.0
    }
}

/// **Un sitio del lector de PDF** (`pagina` mas la parte de su alto, la
/// cuenta de `Ajustes::pagina` y de las marcas) en los tres numeros del
/// protocolo: la pagina desde 0, la parte de ella y la del documento entero
/// (0 si no se sabe cuantas hojas hay).
pub fn posicion_de_hojas(sitio: f64, paginas: u32) -> (u32, f32, f32) {
    let sitio = if sitio.is_finite() {
        sitio.max(0.0)
    } else {
        0.0
    };
    let mut pagina = (sitio.floor() as u64).min(u64::from(TOPE_PAGINA)) as u32;
    let mut frac = acotada(sitio - sitio.floor());
    if paginas > 0 && pagina >= paginas {
        // Mas alla de la ultima: al final de la ultima.
        pagina = paginas - 1;
        frac = 1.0;
    }
    let fraccion = if paginas > 0 {
        acotada((f64::from(pagina) + f64::from(frac)) / f64::from(paginas))
    } else {
        0.0
    };
    (pagina, frac, fraccion)
}

// ------------------------------------------------------------ de PixPin a alli

/// Lo que se le manda a Lectura Veloz al pulsar «⚡ Lectura veloz».
#[derive(Debug, Clone, PartialEq)]
pub struct Salida {
    /// El uid del adjunto del chat, o `ruta:<huella>` ([`id_de_ruta`]).
    pub doc: String,
    /// La ruta absoluta del fichero.
    pub ruta: String,
    /// Su nombre, con la extension.
    pub nombre: String,
    pub tipo: Tipo,
    /// La pagina, desde 0 (0 de corrido).
    pub pagina: u32,
    /// La parte de esa pagina, de 0 a 1 (0 de corrido).
    pub frac: f32,
    /// La parte del documento entero, de 0 a 1.
    pub fraccion: f32,
    /// Cuantas hojas tiene (0 si no se sabe o es de corrido).
    pub paginas: u32,
    /// Cuando cambio por ultima vez lo leido (el 🟢 o el sitio), en
    /// milisegundos desde 1970; 0 si no se sabe.
    pub t: u64,
}

impl Salida {
    /// **El enlace**: `lecturaveloz://pixpin/abrir?v=1&…`, con cada valor
    /// codificado y los numeros acotados.
    pub fn enlace(&self) -> String {
        let num = |f: f32| {
            let f = if f.is_finite() {
                f.clamp(0.0, 1.0)
            } else {
                0.0
            };
            // Seis decimales sobran, y asi no sale `1e-7`.
            let t = format!("{f:.6}");
            let t = t.trim_end_matches('0').trim_end_matches('.');
            if t.is_empty() {
                "0".to_string()
            } else {
                t.to_string()
            }
        };
        let campos: [(&str, String); 10] = [
            ("v", VERSION.to_string()),
            ("doc", self.doc.clone()),
            ("ruta", self.ruta.clone()),
            ("nombre", self.nombre.clone()),
            ("tipo", self.tipo.como_texto().to_string()),
            ("pagina", self.pagina.min(TOPE_PAGINA).to_string()),
            ("frac", num(self.frac)),
            ("fraccion", num(self.fraccion)),
            ("paginas", self.paginas.min(TOPE_PAGINA).to_string()),
            ("t", self.t.to_string()),
        ];
        let mut s = format!("{ESQUEMA}://pixpin/abrir");
        for (i, (k, v)) in campos.iter().enumerate() {
            s.push(if i == 0 { '?' } else { '&' });
            s.push_str(k);
            s.push('=');
            s.push_str(&codificar(v));
        }
        s
    }
}

// ------------------------------------------------------------ de alli a PixPin

/// Lo que llega de Lectura Veloz, ya mirado ([`Progreso::de_campos`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Progreso {
    pub doc: String,
    /// Ruta absoluta y local, con extension de documento.
    pub ruta: String,
    pub tipo: Tipo,
    pub pagina: u32,
    pub frac: f32,
    pub fraccion: f32,
    pub t: u64,
}

/// **Donde se apunta lo leido**: por hojas (la pagina y el 🟢 de las marcas
/// del lector de PDF) o de corrido (el verde de la voz y el sitio del
/// lector de documentos).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destino {
    Hojas,
    Corrido,
}

impl Progreso {
    /// **Mira lo que llega** (los campos del enlace o del pedido, ya
    /// descodificados) y lo deja en su sitio. Lo que no se entiende es un
    /// error con el motivo, no algo que adivinar: este enlace escribe en el
    /// disco, y sin confirmar.
    ///
    /// - `v`: si viene, `1`.
    /// - `doc`: obligatorio, corto y sin caracteres de control.
    /// - `ruta`: obligatoria, absoluta de una unidad local, sin `..`, con
    ///   la extension de un documento ([`EXTENSIONES`]).
    /// - `tipo`: obligatorio, `pdf` o `texto`. Un `.pdf` va siempre por
    ///   hojas, y entonces `pagina` es obligatoria.
    /// - `pagina`, `frac`, `fraccion`, `t`: numeros finitos; las fracciones
    ///   se acotan a 0..1 y la pagina a [`TOPE_PAGINA`]. Si faltan, 0.
    pub fn de_campos(campos: &BTreeMap<String, String>) -> Result<Progreso, String> {
        let campo = |c: &str| campos.get(c).map(|v| v.trim()).filter(|v| !v.is_empty());
        if let Some(v) = campo("v")
            && v.parse::<u32>().ok() != Some(VERSION)
        {
            return Err(format!(
                "lectura-veloz: version {v} desconocida (se entiende la {VERSION})"
            ));
        }
        let doc = campo("doc").ok_or("lectura-veloz: falta doc")?;
        if doc.chars().count() > TOPE_DOC || doc.chars().any(char::is_control) {
            return Err("lectura-veloz: doc no valido".into());
        }
        let ruta = campo("ruta").ok_or("lectura-veloz: falta ruta")?;
        if ruta.len() > TOPE_RUTA || !es_ruta_local(ruta) {
            return Err("lectura-veloz: la ruta tiene que ser absoluta y de este equipo".into());
        }
        if !es_documento(Path::new(ruta)) {
            return Err("lectura-veloz: la ruta no es de un documento que PixPin lea".into());
        }
        let tipo = campo("tipo")
            .ok_or("lectura-veloz: falta tipo")
            .and_then(|t| Tipo::de_texto(t).ok_or("lectura-veloz: tipo es pdf o texto"))?;
        let numero = |c: &str| -> Result<Option<f64>, String> {
            match campo(c) {
                None => Ok(None),
                Some(v) => v
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite())
                    .map(Some)
                    .ok_or_else(|| format!("lectura-veloz: {c} no es un numero")),
            }
        };
        let pagina = numero("pagina")?;
        if pagina.is_none() && extension_de(ruta) == "pdf" {
            return Err("lectura-veloz: falta pagina (es un PDF)".into());
        }
        let pagina = pagina
            .unwrap_or(0.0)
            .floor()
            .clamp(0.0, f64::from(TOPE_PAGINA)) as u32;
        let frac = acotada(numero("frac")?.unwrap_or(0.0));
        let fraccion = acotada(numero("fraccion")?.unwrap_or(0.0));
        let t = match campo("t") {
            None => 0,
            Some(v) => v
                .parse::<u64>()
                .map_err(|_| "lectura-veloz: t no es un numero".to_string())?,
        };
        Ok(Progreso {
            doc: doc.to_string(),
            ruta: ruta.to_string(),
            tipo,
            pagina,
            frac,
            fraccion,
            t,
        })
    }

    /// Por hojas si es un PDF o si Lectura Veloz lo leyo por hojas (un Word
    /// o un PowerPoint en el lector de hojas); si no, de corrido.
    pub fn destino(&self) -> Destino {
        if self.tipo == Tipo::Pdf || extension_de(&self.ruta) == "pdf" {
            Destino::Hojas
        } else {
            Destino::Corrido
        }
    }

    /// La pagina mas la parte de su alto: la cuenta de `Ajustes::pagina` y
    /// de la `y` de las marcas del PDF.
    pub fn sitio_de_hojas(&self) -> f64 {
        f64::from(self.pagina) + f64::from(self.frac.clamp(0.0, 1.0))
    }

    /// Los campos como texto, para el pedido del protocolo (`lectura_veloz`).
    pub fn a_campos(&self) -> BTreeMap<String, String> {
        [
            ("v", VERSION.to_string()),
            ("doc", self.doc.clone()),
            ("ruta", self.ruta.clone()),
            ("tipo", self.tipo.como_texto().to_string()),
            ("pagina", self.pagina.to_string()),
            ("frac", self.frac.to_string()),
            ("fraccion", self.fraccion.to_string()),
            ("t", self.t.to_string()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
    }
}

/// **Lo que cambia en los ajustes de un documento** con lo leido en
/// Lectura Veloz. De corrido, el verde de la voz y el sitio; por hojas, la
/// pagina (el 🟢 de las marcas lo pone la app, que es quien entiende la
/// linea de marcas, `pixpin_motor2d::marcas`). Lo demas no se toca.
///
/// **El parrafo del verde no se sabe** sin medir el documento, asi que va
/// como el del movil cuando solo sabe la fraccion: `-1:<fraccion>`
/// ([`voz_alta::PARRAFO_DESCONOCIDO`]). Quien lo usa (el visor al escuchar o
/// al poner el 🟢, el movil) lo busca por la fraccion con
/// `voz_alta::parrafo_de_la_marca`; el visor abierto lo resuelve al llegar.
pub fn aplicar(a: &mut Ajustes, p: &Progreso) {
    match p.destino() {
        Destino::Hojas => a.pagina = p.sitio_de_hojas(),
        Destino::Corrido => {
            let f = p.fraccion.clamp(0.0, 1.0);
            a.voz = Some((voz_alta::PARRAFO_DESCONOCIDO, f));
            a.sitio = f;
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn campos(pares: &[(&str, &str)]) -> BTreeMap<String, String> {
        pares
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn pdf() -> BTreeMap<String, String> {
        campos(&[
            ("v", "1"),
            ("doc", "ruta:0123456789abcdef"),
            ("ruta", r"C:\Libros\Tesis final.pdf"),
            ("tipo", "pdf"),
            ("pagina", "4"),
            ("frac", "0.25"),
            ("fraccion", "0.3"),
            ("t", "1761562800000"),
        ])
    }

    #[test]
    fn el_enlace_de_salida_lleva_todo_codificado_y_en_orden() {
        let s = Salida {
            doc: "a1b2-c3".into(),
            ruta: r"C:\Users\Max\Mis libros\Año 1 & más.pdf".into(),
            nombre: "Año 1 & más.pdf".into(),
            tipo: Tipo::Pdf,
            pagina: 12,
            frac: 0.5,
            fraccion: 0.25,
            paginas: 50,
            t: 1_761_562_800_000,
        };
        assert_eq!(
            s.enlace(),
            "lecturaveloz://pixpin/abrir?v=1&doc=a1b2-c3\
             &ruta=C%3A%5CUsers%5CMax%5CMis%20libros%5CA%C3%B1o%201%20%26%20m%C3%A1s.pdf\
             &nombre=A%C3%B1o%201%20%26%20m%C3%A1s.pdf&tipo=pdf&pagina=12&frac=0.5\
             &fraccion=0.25&paginas=50&t=1761562800000"
        );
    }

    #[test]
    fn el_enlace_de_salida_acota_los_numeros() {
        let s = Salida {
            doc: "ruta:1".into(),
            ruta: r"C:\a.docx".into(),
            nombre: "a.docx".into(),
            tipo: Tipo::Texto,
            pagina: 0,
            frac: f32::NAN,
            fraccion: 1.7,
            paginas: 0,
            t: 0,
        };
        let e = s.enlace();
        assert!(
            e.contains("&tipo=texto&pagina=0&frac=0&fraccion=1&paginas=0&t=0"),
            "{e}"
        );
    }

    #[test]
    fn la_huella_de_la_ruta_es_estable_y_no_mira_mayusculas() {
        let a = id_de_ruta(r"C:\Libros\Tesis.pdf");
        assert_eq!(a, id_de_ruta(r"\\?\c:\libros\TESIS.pdf"));
        assert_eq!(a, id_de_ruta("C:/Libros/Tesis.pdf"));
        assert_ne!(a, id_de_ruta(r"C:\Libros\Tesis2.pdf"));
        assert!(a.starts_with("ruta:") && a.len() == 21, "{a}");
        // La huella de siempre (FNV-1a 64 de «c:\libros\tesis.pdf»): si
        // cambia, Lectura Veloz perderia lo guardado de cada fichero suelto.
        assert_eq!(a, "ruta:78954f1abb4b9635");
        assert_eq!(id_de_ruta(""), "ruta:cbf29ce484222325");
    }

    #[test]
    fn la_posicion_de_hojas_se_parte_en_pagina_y_fraccion() {
        assert_eq!(posicion_de_hojas(2.5, 10), (2, 0.5, 0.25));
        assert_eq!(posicion_de_hojas(0.0, 0), (0, 0.0, 0.0));
        // Mas alla de la ultima, al final de la ultima.
        assert_eq!(posicion_de_hojas(12.3, 10), (9, 1.0, 1.0));
        assert_eq!(posicion_de_hojas(f64::NAN, 4), (0, 0.0, 0.0));
        assert_eq!(posicion_de_hojas(-3.0, 4), (0, 0.0, 0.0));
    }

    #[test]
    fn un_progreso_bueno_se_lee_entero() {
        let p = Progreso::de_campos(&pdf()).unwrap();
        assert_eq!(
            p,
            Progreso {
                doc: "ruta:0123456789abcdef".into(),
                ruta: r"C:\Libros\Tesis final.pdf".into(),
                tipo: Tipo::Pdf,
                pagina: 4,
                frac: 0.25,
                fraccion: 0.3,
                t: 1_761_562_800_000,
            }
        );
        assert_eq!(p.destino(), Destino::Hojas);
        assert_eq!(p.sitio_de_hojas(), 4.25);
        // Ida y vuelta por el pedido.
        assert_eq!(Progreso::de_campos(&p.a_campos()).unwrap(), p);
    }

    #[test]
    fn los_numeros_se_acotan() {
        let mut c = pdf();
        c.insert("frac".into(), "7".into());
        c.insert("fraccion".into(), "-2".into());
        c.insert("pagina".into(), "1e12".into());
        let p = Progreso::de_campos(&c).unwrap();
        assert_eq!((p.pagina, p.frac, p.fraccion), (TOPE_PAGINA, 1.0, 0.0));
        c.insert("pagina".into(), "3.9".into());
        assert_eq!(Progreso::de_campos(&c).unwrap().pagina, 3);
    }

    #[test]
    fn caso_negativo_lo_que_no_es_valido_no_pasa() {
        let malos: Vec<(&str, Option<&str>)> = vec![
            ("v", Some("2")),
            ("doc", None),
            ("doc", Some("a\nb")),
            ("ruta", None),
            ("ruta", Some(r"\\servidor\libros\x.pdf")),
            ("ruta", Some(r"\\?\C:\x.pdf")),
            ("ruta", Some(r"libros\x.pdf")),
            ("ruta", Some(r"C:\libros\..\Windows\x.pdf")),
            ("ruta", Some(r"C:\Windows\System32\calc.exe")),
            ("ruta", Some(r"C:\scripts\borrar.ps1")),
            ("ruta", Some(r"C:\a\sin-extension")),
            ("tipo", None),
            ("tipo", Some("exe")),
            ("pagina", None),
            ("pagina", Some("NaN")),
            ("frac", Some("inf")),
            ("fraccion", Some("medio")),
            ("t", Some("-1")),
        ];
        for (k, v) in malos {
            let mut c = pdf();
            match v {
                Some(v) => c.insert(k.into(), v.into()),
                None => c.remove(k),
            };
            assert!(Progreso::de_campos(&c).is_err(), "{k}={v:?} paso");
        }
        let mut largo = pdf();
        largo.insert("doc".into(), "x".repeat(TOPE_DOC + 1));
        assert!(Progreso::de_campos(&largo).is_err());
    }

    #[test]
    fn de_corrido_la_pagina_no_hace_falta() {
        let c = campos(&[
            ("doc", "uid-9"),
            ("ruta", r"D:\libros\cuento.epub"),
            ("tipo", "texto"),
            ("fraccion", "0.42"),
        ]);
        let p = Progreso::de_campos(&c).unwrap();
        assert_eq!(p.destino(), Destino::Corrido);
        assert_eq!((p.pagina, p.frac, p.fraccion, p.t), (0, 0.0, 0.42, 0));
        // Un Word leido por hojas va por hojas.
        let mut c = c;
        c.insert("tipo".into(), "pdf".into());
        assert_eq!(Progreso::de_campos(&c).unwrap().destino(), Destino::Hojas);
    }

    #[test]
    fn un_pdf_va_por_hojas_aunque_diga_texto() {
        let mut c = pdf();
        c.insert("tipo".into(), "texto".into());
        assert_eq!(Progreso::de_campos(&c).unwrap().destino(), Destino::Hojas);
    }

    #[test]
    fn aplicar_mueve_solo_lo_suyo() {
        let mut a = Ajustes {
            sitio: 0.1,
            pagina: 1.0,
            marcas: "1:0.5:2.3:⭐".into(),
            voz: Some((40, 0.2)),
            tamano: 130,
            ..Ajustes::default()
        };
        let mut p = Progreso::de_campos(&pdf()).unwrap();
        aplicar(&mut a, &p);
        assert_eq!(a.pagina, 4.25);
        assert_eq!(
            (a.sitio, a.voz, a.marcas.as_str()),
            (0.1, Some((40, 0.2)), "1:0.5:2.3:⭐")
        );

        p.ruta = r"C:\Libros\Tesis.docx".into();
        p.tipo = Tipo::Texto;
        p.fraccion = 0.4;
        aplicar(&mut a, &p);
        assert_eq!(a.voz, Some((voz_alta::PARRAFO_DESCONOCIDO, 0.4)));
        assert_eq!(a.sitio, 0.4);
        assert_eq!((a.pagina, a.tamano), (4.25, 130));
        assert_eq!(a.marcas, "1:0.5:2.3:⭐");
    }

    #[test]
    fn el_verde_de_corrido_va_sin_parrafo_y_se_busca_por_la_fraccion() {
        let mut a = Ajustes::default();
        let c = campos(&[
            ("doc", "uid-1"),
            ("ruta", r"C:\a\b.epub"),
            ("tipo", "texto"),
            ("fraccion", "0.31"),
        ]);
        aplicar(&mut a, &Progreso::de_campos(&c).unwrap());
        assert_eq!(a.voz, Some((voz_alta::PARRAFO_DESCONOCIDO, 0.31)));
        // Se guarda como el movil, y vuelve igual.
        let texto = crate::lectura::a_texto(&a);
        assert!(texto.contains("voz -1:0.31\n"), "{texto}");
        assert_eq!(crate::lectura::de_texto(&texto).voz, a.voz);
        // Y el lector, con su documento medido, va por la fraccion.
        let fracciones = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5];
        assert_eq!(
            voz_alta::parrafo_de_la_marca(&fracciones, a.voz.unwrap()),
            Some(3)
        );
    }

    #[test]
    fn el_tipo_sale_de_la_extension() {
        assert_eq!(tipo_de(Path::new(r"C:\a\B.PDF")), Some(Tipo::Pdf));
        assert_eq!(tipo_de(Path::new(r"C:\a\b.epub")), Some(Tipo::Texto));
        assert_eq!(tipo_de(Path::new(r"C:\a\b.md")), Some(Tipo::Texto));
        assert_eq!(tipo_de(Path::new(r"C:\a\b.exe")), None);
        assert_eq!(tipo_de(Path::new(r"C:\a\.pdf")), None);
    }
}
