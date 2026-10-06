//! **Exportar el timeline** a HTML o a PDF, con el mismo aspecto de linea
//! de tiempo (la pagina la hace `pixpin_timeline::html`).
//!
//! El PDF es esa misma pagina impresa por Microsoft Edge sin ventana
//! (`--headless --print-to-pdf`). Edge viene con Windows 10 y 11, imprime el
//! HTML tal cual se ve (fotos, colores, sin partir un momento entre dos
//! hojas) y asi no hay que mantener un segundo dibujante de PDF que se
//! pareciera a la pagina sin serlo. Si un equipo no tuviera Edge, se dice y
//! se puede exportar en HTML.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use pixpin_timeline::almacen::Almacen;
use pixpin_timeline::html::{self, Opciones, Seccion};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    Html,
    Pdf,
}

impl Formato {
    pub fn extension(self) -> &'static str {
        match self {
            Formato::Html => "html",
            Formato::Pdf => "pdf",
        }
    }
}

/// La pagina, con las fotos y los audios metidos dentro.
pub fn pagina(almacen: &Almacen, op: &Opciones, secciones: &[Seccion<'_>]) -> String {
    let dentro = |nombre: &str| {
        let bytes = std::fs::read(almacen.ruta(nombre)).ok()?;
        Some(html::data_uri(nombre, &bytes))
    };
    html::pagina(op, secciones, &dentro, &dentro)
}

/// A partir de cuanto avisa la app de que la pagina pesa mucho (fotos y
/// audios van dentro): mandarla por mensajeria o abrirla en el movil cuesta.
pub const PESA_MUCHO: usize = 50 * 1024 * 1024;

/// Donde esta Edge, si esta.
fn edge() -> Option<PathBuf> {
    let mut sitios = Vec::new();
    for var in ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"] {
        if let Some(d) = std::env::var_os(var) {
            sitios.push(PathBuf::from(d).join(r"Microsoft\Edge\Application\msedge.exe"));
        }
    }
    sitios.into_iter().find(|p| p.is_file())
}

/// **Imprime `pagina` a PDF en `destino`.** Tarda un par de segundos: se
/// llama desde un hilo aparte, nunca desde el de la ventana. Devuelve el
/// motivo si no se pudo.
pub fn a_pdf(pagina: &str, destino: &Path) -> Result<(), String> {
    let edge = edge().ok_or_else(|| "sin-edge".to_string())?;
    let base = std::env::temp_dir().join(format!("pixpin-timeline-pdf-{}", std::process::id()));
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let html = base.join("timeline.html");
    std::fs::write(&html, pagina).map_err(|e| e.to_string())?;
    // Un perfil propio y vacio: con el del usuario, Edge abierto se quedaria
    // con la orden y no imprimiria nada.
    let perfil = base.join("perfil");
    let _ = std::fs::remove_file(destino);
    let mut orden = std::process::Command::new(&edge);
    orden
        .arg("--headless=new")
        .arg("--disable-gpu")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--no-pdf-header-footer")
        .arg(format!("--user-data-dir={}", perfil.display()))
        .arg(format!("--print-to-pdf={}", destino.display()))
        .arg(url_de(&html));
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: sin consola que se abra y se cierre.
        orden.creation_flags(0x0800_0000);
    }
    let mut hijo = orden.spawn().map_err(|e| e.to_string())?;
    let empezo = Instant::now();
    let resultado = loop {
        match hijo.try_wait() {
            Ok(Some(_)) => break Ok(()),
            Ok(None) if empezo.elapsed() > Duration::from_secs(90) => {
                let _ = hijo.kill();
                break Err("tiempo".to_string());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => break Err(e.to_string()),
        }
    };
    let _ = std::fs::remove_dir_all(&base);
    resultado?;
    match std::fs::metadata(destino) {
        Ok(m) if m.len() > 0 => Ok(()),
        _ => Err("vacio".to_string()),
    }
}

/// `file:///C:/…` de una ruta, con lo que no puede ir en una URL escapado.
fn url_de(ruta: &Path) -> String {
    let mut s = String::from("file:///");
    for b in ruta.to_string_lossy().replace('\\', "/").bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => {
                s.push(b as char)
            }
            otro => s.push_str(&format!("%{otro:02X}")),
        }
    }
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_url_de_una_ruta_con_blancos_y_tildes() {
        assert_eq!(
            url_de(Path::new(r"C:\Users\Max Book\línea.html")),
            "file:///C:/Users/Max%20Book/l%C3%ADnea.html"
        );
    }

    /// Una foto de muestra: un degradado con una franja.
    fn foto_de_muestra(ruta: &Path, ancho: u32, alto: u32, a: [u8; 3], b: [u8; 3]) {
        let mut pixeles = Vec::with_capacity((ancho * alto * 4) as usize);
        for y in 0..alto {
            for x in 0..ancho {
                let t = (x + y) as f32 / (ancho + alto) as f32;
                let franja = ((y as f32 / alto as f32) - 0.62).abs() < 0.05;
                for c in 0..3 {
                    let v = a[c] as f32 * (1.0 - t) + b[c] as f32 * t;
                    pixeles.push(if franja { (v * 0.6) as u8 } else { v as u8 });
                }
                pixeles.push(255);
            }
        }
        let img = pixpin_codec::ImagenRgba {
            ancho,
            alto,
            pixeles,
        };
        pixpin_codec::guardar(&img, ruta, pixpin_codec::FormatoImagen::Png).unwrap();
    }

    /// Un audio de muestra: dos segundos de un tono suave, en WAV (el que
    /// dicta la app es m4a; el tipo de cada uno lo prueba `html`).
    fn audio_de_muestra(ruta: &Path) {
        let muestras = 16_000u32 * 2;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + muestras * 2).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&16_000u32.to_le_bytes());
        b.extend_from_slice(&32_000u32.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(muestras * 2).to_le_bytes());
        for n in 0..muestras {
            let t = n as f32 / 16_000.0;
            let v = (t * 440.0 * std::f32::consts::TAU).sin() * 6000.0 * (1.0 - t / 2.0);
            b.extend_from_slice(&(v as i16).to_le_bytes());
        }
        std::fs::write(ruta, b).unwrap();
    }

    /// Captura la pagina con Edge sin ventana, para mirarla.
    fn captura(html: &Path, png: &Path, ancla: &str, w: u32, h: u32) {
        let Some(edge) = edge() else { return };
        let perfil = std::env::temp_dir().join(format!("pixpin-tl-captura-{}", std::process::id()));
        let _ = std::process::Command::new(edge)
            .arg("--headless=new")
            .arg("--disable-gpu")
            .arg("--hide-scrollbars")
            .arg(format!("--user-data-dir={}", perfil.display()))
            .arg(format!("--window-size={w},{h}"))
            .arg(format!("--screenshot={}", png.display()))
            .arg(format!("{}{ancla}", url_de(html)))
            .status();
        let _ = std::fs::remove_dir_all(perfil);
    }

    /// Exporta un timeline de muestra en HTML y PDF a la carpeta de
    /// `PIXPIN_MUESTRAS` y saca capturas de la pagina con Edge. Ignorada:
    /// necesita Edge y tarda.
    #[test]
    #[ignore]
    fn muestra_exportada_en_html_y_pdf() {
        use pixpin_timeline::Momento;
        let Some(dir) = std::env::var_os("PIXPIN_MUESTRAS").map(PathBuf::from) else {
            return;
        };
        let raiz = dir.join("datos-export");
        let _ = std::fs::remove_dir_all(&raiz);
        let almacen = Almacen::en(&raiz);
        std::fs::create_dir_all(almacen.medios()).unwrap();
        foto_de_muestra(&almacen.ruta("f.png"), 1200, 800, [240, 150, 80], [90, 40, 120]);
        foto_de_muestra(&almacen.ruta("g.png"), 900, 1200, [60, 160, 200], [20, 60, 90]);
        audio_de_muestra(&almacen.ruta("c.wav"));
        let dia = pixpin_timeline::dias::Dia {
            anio: 2026,
            mes: 10,
            dia: 5,
        };
        let base = dia.numero() * pixpin_timeline::dias::DIA_MS;
        let h = |hh: i64, mm: i64| base + (hh * 60 + mm) * 60_000;
        let a = Momento::nuevo(
            "a".into(),
            h(9, 5),
            "Reunión con el equipo ☕".into(),
            "Revisamos el avance.\nTodo en orden.".into(),
        );
        let mut b = Momento::nuevo(
            "b".into(),
            h(12, 40),
            "Llegó el pedido".into(),
            "Venían dos galones abiertos. \"Ojo\" con </script> en el texto.".into(),
        );
        b.fotos.push("f.png".into());
        b.fotos.push("g.png".into());
        let mut c = Momento::nuevo(
            "c".into(),
            h(18, 2),
            "Se fue la luz".into(),
            "Volvió a los veinte minutos.".into(),
        );
        c.audio = Some("c.wav".into());
        c.duracion_ms = 2_000;
        let op = Opciones {
            titulo: "Timeline".into(),
            subtitulo: "Lunes 5 de octubre de 2026".into(),
            pie: "Exportado con PixPin Max".into(),
            nota_de_voz: "Nota de voz".into(),
            desfase: 0,
            ingles: false,
            pestanas: ["Línea".into(), "Momentos".into(), "Estado".into()],
            sobre_todo: "Sobre todo".into(),
        };
        let html = pagina(
            &almacen,
            &op,
            &[Seccion {
                rotulo: "Lunes 5 de octubre de 2026".into(),
                momentos: vec![&a, &b, &c],
            }],
        );
        let ruta = dir.join("timeline-exportado.html");
        std::fs::write(&ruta, &html).unwrap();
        captura(&ruta, &dir.join("timeline-exportado.png"), "", 900, 1100);
        captura(&ruta, &dir.join("timeline-exportado-historia.png"), "#m=b", 900, 1100);
        captura(&ruta, &dir.join("timeline-exportado-voz.png"), "#m=c", 420, 820);
        a_pdf(&html, &dir.join("timeline-exportado.pdf")).unwrap();
        assert!(std::fs::metadata(dir.join("timeline-exportado.pdf")).unwrap().len() > 1000);
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn cada_formato_con_su_extension() {
        assert_eq!(Formato::Html.extension(), "html");
        assert_eq!(Formato::Pdf.extension(), "pdf");
    }
}
