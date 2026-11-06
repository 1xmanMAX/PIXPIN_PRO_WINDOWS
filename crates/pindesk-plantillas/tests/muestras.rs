//! **Las muestras**: se pintan de verdad (PNG, PDF, SVG y el lienzo de
//! PinDesk) sin ventana ni GPU, y se miran.
//!
//! - `PLANTILLAS_LETRAS`: carpeta con `fraunces-600.ttf`, `work-sans-400.ttf`
//!   y `caveat-700.ttf` (las de PinDesk desempaquetadas); sin ella, las del
//!   sistema que haya (DejaVu en Linux, Segoe UI en Windows).
//! - `PLANTILLAS_MUESTRAS`: carpeta donde dejar los PNG y PDF; sin ella, en
//!   el directorio temporal.

use std::path::{Path, PathBuf};

use pindesk_plantillas::exportar::{self, letras::Letras, pdf, png, svg};
use pindesk_plantillas::fecha::Fecha;
use pindesk_plantillas::formato;
use pindesk_plantillas::galeria;
use pindesk_plantillas::ia::{self, ClienteFalso};
use pindesk_plantillas::tema::{CAVEAT, FRAUNCES, WORK_SANS};
use pindesk_plantillas::textos::Textos;

fn hoy() -> Fecha {
    Fecha::nueva(2026, 11, 6).unwrap()
}

fn leer(p: &Path) -> Option<Vec<u8>> {
    std::fs::read(p).ok()
}

fn letras() -> Letras {
    let mut l = Letras::nuevas();
    let dir = std::env::var_os("PLANTILLAS_LETRAS").map(PathBuf::from);
    let sistema = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ];
    let sistema_negrita = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "C:\\Windows\\Fonts\\segoeuib.ttf",
    ];
    let propia = |n: &str| dir.as_ref().and_then(|d| leer(&d.join(n)));
    let primera = |v: &[&str]| v.iter().find_map(|p| leer(Path::new(p)));
    if let Some(b) = propia("work-sans-400.ttf").or_else(|| primera(&sistema)) {
        l.poner(WORK_SANS, false, b);
    }
    if let Some(b) = primera(&sistema_negrita) {
        l.poner(WORK_SANS, true, b);
    }
    if let Some(b) = propia("fraunces-600.ttf")
        .or_else(|| primera(&["/usr/share/fonts/truetype/dejavu/DejaVuSerif-Bold.ttf"]))
    {
        l.poner(FRAUNCES, true, b.clone());
        l.poner(FRAUNCES, false, b);
    }
    if let Some(b) = propia("caveat-700.ttf") {
        l.poner(CAVEAT, true, b.clone());
        l.poner(CAVEAT, false, b);
    }
    for r in [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\simhei.ttf",
    ] {
        if let Some(b) = leer(Path::new(r)) {
            l.reserva(b);
        }
    }
    l
}

fn salida() -> PathBuf {
    let d = std::env::var_os("PLANTILLAS_MUESTRAS")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("pindesk-plantillas-muestras"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Cada una de la galeria a PNG (su primera pagina), y el fichero de ida y
/// vuelta sin perder nada.
#[test]
fn la_galeria_entera_a_png_y_de_ida_y_vuelta() {
    let l = letras();
    let dir = salida();
    for idioma in ["es-ES", "en-US", "ja-JP"] {
        let t = Textos::nuevos(idioma);
        for clave in galeria::CLAVES {
            let d = galeria::por_clave(clave, &t, hoy()).unwrap();
            // El fichero: escribir y leer da lo mismo.
            let texto = formato::escribir(&d);
            assert!(texto.contains("\"formato\": \"pindesk-plantilla\""));
            let (otra, avisos) = formato::leer(&texto).unwrap();
            assert_eq!(otra, d, "{clave}: ida y vuelta");
            assert!(avisos.is_empty(), "{clave}: {avisos:?}");
            let p = exportar::pagina(&d, hoy(), 0);
            let px = png::pintar(&p, 110.0, &l).unwrap();
            assert!(px.width() > 800);
            // No es una hoja en blanco: hay tinta.
            let tinta = px.pixels().iter().filter(|c| c.red() < 200).count();
            assert!(tinta > 2000, "{clave}: casi en blanco ({tinta})");
            if idioma == "es-ES" || clave == "mensual" {
                px.save_png(dir.join(format!("{clave}-{idioma}.png")))
                    .unwrap();
            }
        }
    }
}

/// El ejemplo del dueño, hecho por la «IA» (un cliente falso que contesta
/// lo que contestaria el modelo) y sin modelo, a PNG.
#[test]
fn el_ejemplo_de_la_ia_a_png() {
    let l = letras();
    let frase = "planner semanal con horario de 7 a 22, lista de tareas y hábitos";
    let respuesta = include_str!("respuesta_ia.json");
    let c = ClienteFalso::con(&[respuesta]);
    let p = ia::describir(&c, frase, "es", hoy()).unwrap();
    assert!(p.avisos.is_empty(), "{:?}", p.avisos);
    let pag = exportar::pagina(&p.doc, hoy(), 0);
    png::pintar(&pag, 110.0, &l)
        .unwrap()
        .save_png(salida().join("ia-planner-semanal.png"))
        .unwrap();
    let sin = ia::sin_modelo(frase, "es", &Textos::nuevos("es"), hoy());
    let pag = exportar::pagina(&sin, hoy(), 0);
    png::pintar(&pag, 110.0, &l)
        .unwrap()
        .save_png(salida().join("ia-sin-modelo.png"))
        .unwrap();
}

/// El diario de un año a PDF: 365 paginas de vectores con su texto.
#[test]
fn un_ano_de_diario_a_pdf_svg_y_lienzo() {
    let l = letras();
    let d = galeria::por_clave("diario", &Textos::nuevos("es"), hoy()).unwrap();
    let paginas = exportar::paginas(&d, hoy());
    assert_eq!(paginas.len(), 365);
    let bytes = pdf::a_pdf(&paginas, &l, &d.nombre);
    assert!(bytes.starts_with(b"%PDF-1.7"));
    assert!(bytes.ends_with(b"%%EOF\n"));
    let texto = String::from_utf8_lossy(&bytes);
    assert!(texto.contains("/Count 365"));
    assert!(bytes.len() < 12_000_000, "{} bytes", bytes.len());
    std::fs::write(salida().join("diario-2027.pdf"), &bytes).unwrap();
    // Sin letras tambien sale (Helvetica).
    let sin = pdf::a_pdf(&paginas[..2], &Letras::nuevas(), "x");
    assert!(String::from_utf8_lossy(&sin).contains("/Helvetica"));
    // SVG de una pagina.
    let s = svg::a_svg(&paginas[0], &d.tema, &l.vista(&paginas[0].familias));
    assert!(s.starts_with("<svg") && s.contains("<text") && s.trim_end().ends_with("</svg>"));
    // El lienzo de PinDesk lo lee como un .excalidraw del movil.
    let ex = exportar::figura::excalidraw(&paginas[..1], &d.tema);
    let lienzo = pindesk_motor2d::excalidraw::leer(&ex).unwrap();
    assert!(lienzo.entradas.len() > 50);
}

/// Una pagina en cada idioma de las fechas: los meses y dias salen en el
/// suyo (se miran en las muestras).
#[test]
fn el_calendario_en_varios_idiomas() {
    let l = letras();
    for idioma in ["de-DE", "ru-RU", "zh-CN", "ko-KR"] {
        let t = Textos::nuevos(idioma);
        let d = galeria::por_clave("mensual", &t, hoy()).unwrap();
        let p = exportar::pagina(&d, hoy(), 0);
        let textos: Vec<String> = p
            .prims
            .iter()
            .filter_map(|x| match x {
                pindesk_plantillas::prim::Prim::Texto { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect();
        let esperado = match idioma {
            "de-DE" => "Januar 2027",
            "ru-RU" => "Январь 2027",
            "zh-CN" => "2027年1月",
            _ => "2027년 1월",
        };
        assert!(textos.iter().any(|x| x == esperado), "{idioma}: {textos:?}");
        png::pintar(&p, 80.0, &l)
            .unwrap()
            .save_png(salida().join(format!("mensual-{idioma}.png")))
            .unwrap();
    }
}
