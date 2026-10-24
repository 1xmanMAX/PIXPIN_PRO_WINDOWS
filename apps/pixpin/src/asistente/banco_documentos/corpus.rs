//! **El corpus del banco de documentos** (15-oct-2026): 16 documentos de
//! todo tipo hechos aqui, sin internet, con los escritores de la propia app
//! (el PDF del lienzo `pixpin_pdf::escribir`, el Word de las notas
//! `pixpin_docs::exportar_docx`, el Excel de las tablas
//! `pixpin_proyecto::tabla_xlsx`) y fotos libres de Windows
//! (`C:\Windows\Web\Screen`). El PowerPoint se escribe a mano (no hay
//! escritor en la app): un ZIP sin comprimir con su XML minimo, que lee el
//! mismo `pixpin_office` que el lector.
//!
//! Cada documento sabe lo que lleva ([`Doc::datos`]): las comprobaciones de
//! los casos miran esos datos en lo que contesto el asistente.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pixpin_codec::ImagenRgba;
use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura};
use pixpin_motor2d::vector::Punto2;

/// Donde quedan los documentos (fuera del repo).
pub fn carpeta() -> PathBuf {
    std::env::var("PIXPIN_BANCO_DOCS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(r"F:\pixpin-banco-docs"))
}

/// Un documento del corpus.
#[derive(Debug, Clone)]
pub struct Doc {
    pub nombre: &'static str,
    /// Que tipo de contenido es (para la tabla).
    pub tipo: &'static str,
}

/// Todos, en orden.
pub const DOCS: &[Doc] = &[
    Doc {
        nombre: "01-comunicado-reunion.pdf",
        tipo: "solo texto",
    },
    Doc {
        nombre: "02-informe-drenaje.docx",
        tipo: "parrafos largos, titulos y listas",
    },
    Doc {
        nombre: "03-formulario-fisica.pdf",
        tipo: "formulas matematicas y fisicas",
    },
    Doc {
        nombre: "04-pizarra-formulas.png",
        tipo: "formulas en imagen",
    },
    Doc {
        nombre: "05-presupuesto-obra.xlsx",
        tipo: "tabla (Excel)",
    },
    Doc {
        nombre: "06-notas-curso.pdf",
        tipo: "tabla en PDF",
    },
    Doc {
        nombre: "07-stock-materiales.png",
        tipo: "tabla en foto",
    },
    Doc {
        nombre: "08-articulo-con-fotos.docx",
        tipo: "texto con fotos",
    },
    Doc {
        nombre: "09-boletin-dos-columnas.pdf",
        tipo: "mixto de dos columnas con foto",
    },
    Doc {
        nombre: "10-grafica-produccion.png",
        tipo: "una grafica",
    },
    Doc {
        nombre: "11-diagrama-flujo.png",
        tipo: "diagrama de flujo",
    },
    Doc {
        nombre: "12-geometria-circunferencia.png",
        tipo: "geometria",
    },
    Doc {
        nombre: "13-guia-escaneada.png",
        tipo: "hoja escaneada (inclinada, con ruido)",
    },
    Doc {
        nombre: "14-charla-seguridad.pptx",
        tipo: "presentacion",
    },
    Doc {
        nombre: "15-metrados-con-error.xlsx",
        tipo: "tabla con un error de calculo",
    },
    Doc {
        nombre: "16-examen-mixto.pdf",
        tipo: "examen: formulas, figura y alternativas",
    },
];

pub fn ruta(nombre: &str) -> PathBuf {
    carpeta().join(nombre)
}

// ---------------------------------------------------------------------------
// Una hoja de elementos del lienzo, para el PDF y el PNG

fn negro() -> ColorRgba {
    ColorRgba::opaco(0.08, 0.08, 0.10)
}

/// Una hoja (en puntos) con sus elementos y sus fotos.
pub struct Hoja {
    pub w: f32,
    pub h: f32,
    pub els: Vec<Elemento>,
    pub fotos: HashMap<u64, ImagenRgba>,
}

impl Hoja {
    pub fn nueva(w: f32, h: f32) -> Hoja {
        Hoja {
            w,
            h,
            els: Vec::new(),
            fotos: HashMap::new(),
        }
    }

    fn base(figura: Figura, x: f32, y: f32, ancho: f32, alto: f32, trazo: ColorRgba) -> Elemento {
        Elemento {
            figura,
            x,
            y,
            ancho,
            alto,
            trazo,
            grosor: 1.5,
            rugosidad: 0.0,
            opacidad: 1.0,
            ..Default::default()
        }
    }

    /// Un renglon de texto; devuelve su alto.
    pub fn texto(&mut self, t: &str, x: f32, y: f32, tam: f32, negrita: bool) -> f32 {
        self.texto_color(t, x, y, tam, negrita, negro())
    }

    pub fn texto_color(
        &mut self,
        t: &str,
        x: f32,
        y: f32,
        tam: f32,
        negrita: bool,
        c: ColorRgba,
    ) -> f32 {
        let ancho =
            pixpin_motor2d::exportar::ancho_de_texto(t, tam) * if negrita { 1.06 } else { 1.0 };
        let mut e = Hoja::base(
            Figura::Texto {
                texto: t.into(),
                tam,
                familia: "Segoe UI".into(),
            },
            x,
            y,
            ancho,
            tam * 1.25,
            c,
        );
        e.extras.negrita = negrita;
        self.els.push(e);
        tam * 1.25
    }

    /// Un texto centrado en `cx`.
    pub fn centrado(&mut self, t: &str, cx: f32, y: f32, tam: f32, negrita: bool) -> f32 {
        let w = pixpin_motor2d::exportar::ancho_de_texto(t, tam);
        self.texto(t, cx - w / 2.0, y, tam, negrita)
    }

    /// Un texto rico (formulas en linea entre `$...$`); devuelve su alto.
    pub fn rico(&mut self, fuente: &str, x: f32, y: f32, tam: f32) -> f32 {
        let mut e = Hoja::base(
            Figura::Texto {
                texto: String::new(),
                tam,
                familia: "Segoe UI".into(),
            },
            x,
            y,
            10.0,
            tam,
            negro(),
        );
        pixpin_motor2d::texto_rico::poner_fuente(&mut e, fuente);
        let alto = e.alto;
        self.els.push(e);
        alto
    }

    /// Un parrafo partido a `ancho`; devuelve donde acaba.
    pub fn parrafo(&mut self, t: &str, x: f32, mut y: f32, ancho: f32, tam: f32) -> f32 {
        let mut renglon = String::new();
        for p in t.split_whitespace() {
            let probar = if renglon.is_empty() {
                p.to_string()
            } else {
                format!("{renglon} {p}")
            };
            if pixpin_motor2d::exportar::ancho_de_texto(&probar, tam) > ancho && !renglon.is_empty()
            {
                y += self.texto(&renglon, x, y, tam, false) * 1.12;
                renglon = p.to_string();
            } else {
                renglon = probar;
            }
        }
        if !renglon.is_empty() {
            y += self.texto(&renglon, x, y, tam, false) * 1.12;
        }
        y
    }

    pub fn linea(&mut self, puntos: &[(f32, f32)], grosor: f32, c: ColorRgba) {
        let pts: Vec<Punto2> = puntos.iter().map(|&(x, y)| Punto2 { x, y }).collect();
        let (x0, y0) = (
            pts.iter().map(|p| p.x).fold(f32::MAX, f32::min),
            pts.iter().map(|p| p.y).fold(f32::MAX, f32::min),
        );
        let (x1, y1) = (
            pts.iter().map(|p| p.x).fold(f32::MIN, f32::max),
            pts.iter().map(|p| p.y).fold(f32::MIN, f32::max),
        );
        // Los puntos van en coordenadas de la escena (como `ecuacion::elemento_linea`).
        let rel = pts.clone();
        let mut e = Hoja::base(
            Figura::Linea { puntos: rel },
            pts[0].x,
            pts[0].y,
            (x1 - x0).max(0.1),
            (y1 - y0).max(0.1),
            c,
        );
        e.grosor = grosor;
        self.els.push(e);
    }

    pub fn flecha(&mut self, a: (f32, f32), b: (f32, f32)) {
        let rel = vec![Punto2 { x: a.0, y: a.1 }, Punto2 { x: b.0, y: b.1 }];
        let mut e = Hoja::base(
            Figura::Flecha {
                puntos: rel,
                punta_inicio: Default::default(),
                punta_fin: pixpin_motor2d::formas::TipoPunta::Flecha,
                codos: false,
            },
            a.0,
            a.1,
            (b.0 - a.0).abs().max(0.1),
            (b.1 - a.1).abs().max(0.1),
            negro(),
        );
        e.grosor = 2.0;
        self.els.push(e);
    }

    pub fn caja(
        &mut self,
        figura: Figura,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        trazo: ColorRgba,
        relleno: Option<ColorRgba>,
    ) {
        let mut e = Hoja::base(figura, x, y, w, h, trazo);
        e.relleno = relleno;
        e.estilo_relleno = pixpin_motor2d::EstiloRelleno::Solido;
        e.grosor = 2.0;
        self.els.push(e);
    }

    /// Una foto de disco, reducida, en su caja.
    pub fn foto(&mut self, ruta: &Path, x: f32, y: f32, w: f32, h: f32) {
        let Ok(img) = pixpin_codec::imagen::cargar(ruta) else {
            return;
        };
        let img =
            pixpin_codec::imagen::redimensionar(img, (w * 1.6) as u32, (h * 1.6) as u32).unwrap();
        let id = 7000 + self.fotos.len() as u64;
        self.fotos.insert(id, img);
        self.els
            .push(crate::imagenes_lienzo::elemento_imagen(id, x, y, w, h));
    }

    /// Una tabla con rejilla; devuelve donde acaba.
    pub fn tabla(
        &mut self,
        filas: &[&[&str]],
        x: f32,
        y: f32,
        anchos: &[f32],
        alto_fila: f32,
        tam: f32,
        rejilla: bool,
    ) -> f32 {
        let total: f32 = anchos.iter().sum();
        let gris = ColorRgba::opaco(0.35, 0.35, 0.38);
        for (i, f) in filas.iter().enumerate() {
            let fy = y + i as f32 * alto_fila;
            if i == 0 && rejilla {
                self.caja(
                    Figura::Rectangulo,
                    x,
                    fy,
                    total,
                    alto_fila,
                    gris,
                    Some(ColorRgba::opaco(0.88, 0.91, 0.96)),
                );
            }
            let mut cx = x;
            for (j, c) in f.iter().enumerate() {
                self.texto(
                    c,
                    cx + 6.0,
                    fy + (alto_fila - tam * 1.25) / 2.0,
                    tam,
                    i == 0,
                );
                cx += anchos[j];
            }
        }
        let fin = y + filas.len() as f32 * alto_fila;
        for i in 0..=filas.len() {
            if rejilla || i == 0 || i == 1 || i == filas.len() {
                let fy = y + i as f32 * alto_fila;
                self.linea(&[(x, fy), (x + total, fy)], 1.0, gris);
            }
        }
        if rejilla {
            let mut cx = x;
            for a in anchos.iter().chain([&0.0]) {
                self.linea(&[(cx, y), (cx, fin)], 1.0, gris);
                cx += a;
            }
        }
        fin
    }

    fn escena(&self) -> pixpin_motor2d::Escena {
        let mut s = pixpin_motor2d::Escena::nueva();
        // El papel blanco entero (que el PNG no se recorte a lo dibujado).
        s.anadir(Elemento {
            x: 0.0,
            y: 0.0,
            ancho: self.w,
            alto: self.h,
            trazo: ColorRgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.0,
            },
            relleno: Some(ColorRgba::opaco(1.0, 1.0, 1.0)),
            estilo_relleno: pixpin_motor2d::EstiloRelleno::Solido,
            grosor: 0.0,
            rugosidad: 0.0,
            ..Default::default()
        });
        for e in &self.els {
            s.anadir(e.clone());
        }
        s
    }
}

/// Las hojas a un PDF (con texto de verdad: se selecciona y se busca).
pub fn a_pdf(hojas: &[Hoja]) -> Vec<u8> {
    let mut todas = Vec::new();
    let mut medidas = Vec::new();
    let mut fotos: HashMap<u64, ImagenRgba> = HashMap::new();
    for h in hojas {
        let mut v = pixpin_motor2d::exportar::hojas(
            &h.escena(),
            pixpin_motor2d::exportar::Alcance::Todo,
            &[],
            None,
        );
        let mut hoja = v.remove(0);
        hoja.caja = (0.0, 0.0, h.w, h.h);
        todas.push(hoja);
        medidas.push((h.w, h.h));
        fotos.extend(h.fotos.iter().map(|(k, v)| (*k, v.clone())));
    }
    let de = |id: u64| {
        fotos.get(&id).map(|i| pixpin_pdf::escribir::Pixeles {
            ancho: i.ancho,
            alto: i.alto,
            rgba: i.pixeles.clone(),
        })
    };
    pixpin_pdf::escribir::de_hojas_a_medida(
        &todas,
        &medidas,
        &de,
        crate::asistente::hoja_a_lienzo::letra_segoe(),
    )
    .expect("no salio el PDF")
}

/// La hoja `i` de un PDF a pixeles.
pub fn pdf_a_png(pdf: &Path, i: u32, ancho: u32) -> ImagenRgba {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    pixpin_pdf::Documento::abrir(pdf)
        .unwrap()
        .renderizar(i, ancho)
        .unwrap()
}

/// Una hoja a pixeles (por su PDF).
pub fn hoja_a_png(h: Hoja, ancho: u32) -> ImagenRgba {
    let tmp = std::env::temp_dir().join(format!(
        "pixpin-corpus-{}-{}.pdf",
        std::process::id(),
        h.els.len()
    ));
    std::fs::write(&tmp, a_pdf(&[h])).unwrap();
    let img = pdf_a_png(&tmp, 0, ancho);
    let _ = std::fs::remove_file(&tmp);
    img
}

fn guardar_png(nombre: &str, img: &ImagenRgba) {
    std::fs::write(
        ruta(nombre),
        pixpin_codec::imagen::codificar_png(img).unwrap(),
    )
    .unwrap();
}

/// Papel gris, grano y un poco de borroso: como sale del escaner.
pub fn escanear(img: &mut ImagenRgba) {
    let (w, h) = (img.ancho as usize, img.alto as usize);
    let orig = img.pixeles.clone();
    let mut s: u32 = 77;
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                let mut suma = 0u32;
                let mut n = 0u32;
                for yy in y.saturating_sub(1)..(y + 2).min(h) {
                    for xx in x.saturating_sub(1)..(x + 2).min(w) {
                        suma += orig[(yy * w + xx) * 4 + c] as u32;
                        n += 1;
                    }
                }
                s = s.wrapping_mul(1103515245).wrapping_add(12345);
                let ruido = ((s >> 16) % 34) as f32 - 17.0;
                // Mas oscuro hacia la derecha (la luz del escaner).
                let sombra = 18.0 * x as f32 / w as f32;
                let v = (suma / n) as f32 * 0.86 + ruido + 12.0 - sombra;
                img.pixeles[(y * w + x) * 4 + c] = v.clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// Girada `grados` alrededor del centro (lo de fuera, papel gris).
pub fn inclinar(img: &ImagenRgba, grados: f32) -> ImagenRgba {
    let (w, h) = (img.ancho as i64, img.alto as i64);
    let (s, c) = grados.to_radians().sin_cos();
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let mut px = vec![0u8; img.pixeles.len()];
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f32 - cx, y as f32 - cy);
            let (sx, sy) = (c * dx + s * dy + cx, -s * dx + c * dy + cy);
            let (x0, y0) = (sx.floor() as i64, sy.floor() as i64);
            let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
            let i = ((y * w + x) * 4) as usize;
            for k in 0..3 {
                let p = |xx: i64, yy: i64| {
                    if xx < 0 || yy < 0 || xx >= w || yy >= h {
                        205.0
                    } else {
                        img.pixeles[((yy * w + xx) * 4) as usize + k] as f32
                    }
                };
                let v = p(x0, y0) * (1.0 - fx) * (1.0 - fy)
                    + p(x0 + 1, y0) * fx * (1.0 - fy)
                    + p(x0, y0 + 1) * (1.0 - fx) * fy
                    + p(x0 + 1, y0 + 1) * fx * fy;
                px[i + k] = v as u8;
            }
            px[i + 3] = 255;
        }
    }
    ImagenRgba {
        ancho: img.ancho,
        alto: img.alto,
        pixeles: px,
    }
}

// ---------------------------------------------------------------------------
// ZIP sin comprimir (el PowerPoint)

fn zip_guardado(entradas: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (nombre, datos) in entradas {
        let mut crc = flate2::Crc::new();
        crc.update(datos);
        let crc = crc.sum();
        let desde = out.len() as u32;
        let n = nombre.as_bytes();
        let cab = |firma: u32, central: bool| {
            let mut v = Vec::new();
            v.extend(firma.to_le_bytes());
            if central {
                v.extend(20u16.to_le_bytes());
            }
            v.extend(20u16.to_le_bytes()); // version
            v.extend(0u16.to_le_bytes()); // banderas
            v.extend(0u16.to_le_bytes()); // guardado
            v.extend(0u32.to_le_bytes()); // hora y fecha
            v.extend(crc.to_le_bytes());
            v.extend((datos.len() as u32).to_le_bytes());
            v.extend((datos.len() as u32).to_le_bytes());
            v.extend((n.len() as u16).to_le_bytes());
            v.extend(0u16.to_le_bytes());
            if central {
                v.extend(0u16.to_le_bytes()); // comentario
                v.extend(0u16.to_le_bytes()); // disco
                v.extend(0u16.to_le_bytes()); // atributos internos
                v.extend(0u32.to_le_bytes()); // externos
                v.extend(desde.to_le_bytes());
            }
            v.extend(n);
            v
        };
        out.extend(cab(0x0403_4b50, false));
        out.extend(datos);
        central.extend(cab(0x0201_4b50, true));
    }
    let inicio = out.len() as u32;
    let largo = central.len() as u32;
    out.extend(central);
    out.extend(0x0605_4b50u32.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend((entradas.len() as u16).to_le_bytes());
    out.extend((entradas.len() as u16).to_le_bytes());
    out.extend(largo.to_le_bytes());
    out.extend(inicio.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

const NS_P: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
const RELS: &str = r#"xmlns="http://schemas.openxmlformats.org/package/2006/relationships""#;
const T: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// Una caja de texto de PowerPoint (EMU), con sus parrafos.
fn caja_pptx(id: u32, x: u64, y: u64, w: u64, h: u64, parrafos: &[(&str, u32, bool)]) -> String {
    let ps: String = parrafos
        .iter()
        .map(|(t, sz, b)| {
            format!(
                r#"<a:p><a:r><a:rPr lang="es-PE" sz="{sz}" b="{}"/><a:t>{t}</a:t></a:r></a:p>"#,
                if *b { 1 } else { 0 }
            )
        })
        .collect();
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="T{id}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr wrap="square"/><a:lstStyle/>{ps}</p:txBody></p:sp>"#
    )
}

fn foto_pptx(id: u32, rid: &str, x: u64, y: u64, w: u64, h: u64) -> String {
    format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="Foto{id}"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#
    )
}

/// Un PowerPoint de diapositivas (`(formas, foto)`): el XML minimo.
fn pptx(diapositivas: &[(String, Option<Vec<u8>>)]) -> Vec<u8> {
    let mut e: Vec<(String, Vec<u8>)> = Vec::new();
    let n = diapositivas.len();
    let mut tipos = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="jpeg" ContentType="image/jpeg"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>"#,
    );
    for i in 1..=n {
        tipos.push_str(&format!(r#"<Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#));
    }
    tipos.push_str("</Types>");
    e.push(("[Content_Types].xml".into(), tipos.into_bytes()));
    e.push(("_rels/.rels".into(), format!(r#"<Relationships {RELS}><Relationship Id="r1" Type="{T}/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#).into_bytes()));
    let ids: String = (1..=n)
        .map(|i| format!(r#"<p:sldId id="{}" r:id="rS{i}"/>"#, 255 + i))
        .collect();
    e.push((
        "ppt/presentation.xml".into(),
        format!(r#"<p:presentation {NS_P}><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rM"/></p:sldMasterIdLst><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx="9144000" cy="5143500"/></p:presentation>"#).into_bytes(),
    ));
    let rels: String = (1..=n)
        .map(|i| {
            format!(r#"<Relationship Id="rS{i}" Type="{T}/slide" Target="slides/slide{i}.xml"/>"#)
        })
        .collect();
    e.push((
        "ppt/_rels/presentation.xml.rels".into(),
        format!(r#"<Relationships {RELS}>{rels}<Relationship Id="rM" Type="{T}/slideMaster" Target="slideMasters/slideMaster1.xml"/></Relationships>"#).into_bytes(),
    ));
    e.push((
        "ppt/slideLayouts/slideLayout1.xml".into(),
        format!(r#"<p:sldLayout {NS_P}><p:cSld><p:spTree></p:spTree></p:cSld></p:sldLayout>"#)
            .into_bytes(),
    ));
    e.push((
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(),
        format!(r#"<Relationships {RELS}><Relationship Id="rM" Type="{T}/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#).into_bytes(),
    ));
    e.push((
        "ppt/slideMasters/slideMaster1.xml".into(),
        format!(r#"<p:sldMaster {NS_P}><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></p:bgPr></p:bg><p:spTree></p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/></p:sldMaster>"#).into_bytes(),
    ));
    for (i, (formas, foto)) in diapositivas.iter().enumerate() {
        let k = i + 1;
        e.push((
            format!("ppt/slides/slide{k}.xml"),
            format!(r#"<p:sld {NS_P}><p:cSld><p:spTree>{formas}</p:spTree></p:cSld></p:sld>"#)
                .into_bytes(),
        ));
        let mut r = format!(
            r#"<Relationship Id="rL" Type="{T}/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>"#
        );
        if let Some(b) = foto {
            r.push_str(&format!(
                r#"<Relationship Id="rF" Type="{T}/image" Target="../media/foto{k}.jpeg"/>"#
            ));
            e.push((format!("ppt/media/foto{k}.jpeg"), b.clone()));
        }
        e.push((
            format!("ppt/slides/_rels/slide{k}.xml.rels"),
            format!(r#"<Relationships {RELS}>{r}</Relationships>"#).into_bytes(),
        ));
    }
    let refs: Vec<(&str, Vec<u8>)> = e.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
    zip_guardado(&refs)
}

// ---------------------------------------------------------------------------
// Excel y Word

fn xlsx(nombre_hoja: &str, filas: &[&[&str]]) -> Vec<u8> {
    let filas: Vec<Vec<String>> = filas
        .iter()
        .map(|f| f.iter().map(|c| c.to_string()).collect())
        .collect();
    let mut t = crate::pedidos::tabla_de_filas(nombre_hoja, &filas, true, 0);
    for (i, a) in [190u32, 70, 90, 90, 110].iter().enumerate() {
        t.anchos.insert(((b'A' + i as u8) as char).to_string(), *a);
    }
    pixpin_proyecto::tabla_xlsx::a_xlsx(
        &[pixpin_proyecto::tabla_xlsx::HojaParaVer {
            nombre: nombre_hoja,
            tabla: &t,
            valores: None,
        }],
        0,
    )
    .unwrap()
}

fn docx(markdown: &str, titulo: &str, fotos: &HashMap<String, Vec<u8>>) -> Vec<u8> {
    let imagen = |r: &str| fotos.get(r).cloned();
    pixpin_docs::exportar_docx::exportar(&pixpin_docs::exportar_docx::Nota {
        markdown,
        titulo,
        letra: Default::default(),
        comentarios: None,
        imagen: &imagen,
        autor: "Banco PixPin",
        ahora_ms: 1_792_000_000_000,
    })
    .unwrap()
}

/// Una foto libre de Windows, en JPEG reducido.
fn foto_libre(n: usize, ancho: u32) -> Option<(PathBuf, Vec<u8>)> {
    let dir = Path::new(r"C:\Windows\Web\Screen");
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jpg"))
        .collect();
    v.sort();
    let ruta = v.get(n % v.len().max(1))?.clone();
    let img = pixpin_codec::imagen::cargar(&ruta).ok()?;
    let alto = (img.alto as f32 * ancho as f32 / img.ancho as f32) as u32;
    let img = pixpin_codec::imagen::redimensionar(img, ancho, alto).ok()?;
    Some((ruta, pixpin_codec::imagen::codificar_jpg(&img, 82).ok()?))
}

// ---------------------------------------------------------------------------
// Los documentos

const A4: (f32, f32) = (800.0, 1100.0);

fn d01() {
    let mut h = Hoja::nueva(A4.0, A4.1);
    h.texto("CONSTRUCTORA ANDES S.A.C.", 60.0, 60.0, 15.0, true);
    h.texto("Lima, 15 de octubre de 2026", 560.0, 60.0, 13.0, false);
    h.texto("COMUNICADO N.° 07-2026-CA/OBRA", 60.0, 130.0, 20.0, true);
    h.texto(
        "Asunto: Reunión de coordinación de la obra Av. Florida – Tawa",
        60.0,
        175.0,
        14.0,
        true,
    );
    let mut y = 220.0;
    for p in [
        "Se convoca a todos los residentes, supervisores y jefes de cuadrilla a la reunión de coordinación semanal, que se realizará el jueves 22 de octubre de 2026 a las 9:00 a. m. en la caseta de obra de la Av. Florida, cuadra 3.",
        "En la reunión se revisará el avance del vaciado de la losa del bloque B, que lleva un retraso de cuatro días por la demora en la entrega del acero de 1/2 pulgada, y se acordará el nuevo cronograma de encofrado.",
        "También se tratará el cierre parcial del Jr. Antisuyo durante la instalación de la tubería de desagüe: la Municipalidad autorizó el cierre solo entre las 22:00 y las 5:00 horas, por lo que se necesita organizar un turno de noche.",
        "La asistencia es obligatoria. Quien no pueda asistir deberá enviar a un representante con poder de decisión y avisar antes del miércoles 21 al correo obra.florida@andes.pe.",
    ] {
        y = h.parrafo(p, 60.0, y, 680.0, 14.0) + 16.0;
    }
    h.texto("Atentamente,", 60.0, y + 20.0, 14.0, false);
    h.texto("Ing. Rosa Quispe Mamani", 60.0, y + 90.0, 14.0, true);
    h.texto(
        "Residente de obra – CIP 182455",
        60.0,
        y + 110.0,
        13.0,
        false,
    );
    std::fs::write(ruta(DOCS[0].nombre), a_pdf(&[h])).unwrap();
}

fn d02() {
    let md = r#"# Informe técnico: drenaje pluvial de la Av. Tawa

## 1. Antecedentes

La Av. Tawa, entre el Jr. Antisuyo y la Av. Florida, se inunda cada verano desde la ampliación de la calzada en 2023. Con las lluvias de febrero de 2026 el agua llegó a 25 cm sobre la vereda y entró a doce viviendas del lado norte. La Municipalidad encargó este informe para conocer las causas y proponer una solución que pueda ejecutarse antes de la próxima temporada de lluvias, sin cerrar la avenida más de dos semanas.

El tramo estudiado mide 640 m, tiene una pendiente media de 0,8 % hacia el este y recibe además el agua de tres calles transversales que no tienen cunetas. Se hizo el levantamiento topográfico, se inspeccionaron los buzones existentes con cámara y se revisaron los registros de lluvia de la estación Tacna de los últimos veinte años.

## 2. Diagnóstico

- Las cunetas actuales, de 0,30 m de ancho, están colmatadas en un 60 % con arena y basura.
- Los tres sumideros existentes están a más de 200 m entre sí, cuando la norma recomienda menos de 80 m.
- La tubería de descarga de 12 pulgadas trabaja a presión con lluvias de solo 15 mm/h.
- La calzada nueva quedó 8 cm más alta que el borde de las veredas del lado norte.

## 3. Propuesta

1. Ampliar las cunetas a 0,60 m de ancho con rejilla metálica en los cruces.
2. Construir seis sumideros nuevos, uno cada 75 m.
3. Cambiar la tubería de descarga por una de 24 pulgadas hasta el canal Uchusuma.
4. Rehacer el bombeo de la calzada para que el agua escurra hacia las cunetas.

El caudal de diseño, para un periodo de retorno de 10 años, es de 0,85 m³/s. Con la propuesta, el sistema lo evacúa con un tirante máximo de 12 cm en las cunetas.

## 4. Costo y plazo

El costo estimado de la obra es de S/ 184 500, incluido el IGV, y el plazo de ejecución es de 45 días calendario, trabajando por mitades de calzada para no cerrar la avenida.

## 5. Conclusiones

La inundación no se debe a lluvias extraordinarias sino a un sistema de drenaje pequeño y mal mantenido. La ampliación de las cunetas y los nuevos sumideros resuelven el problema; el cambio de la tubería de descarga es indispensable para que lo demás funcione.
"#;
    std::fs::write(
        ruta(DOCS[1].nombre),
        docx(md, "Informe de drenaje", &HashMap::new()),
    )
    .unwrap();
}

fn d03() {
    let mut h = Hoja::nueva(A4.0, A4.1);
    h.centrado(
        "Formulario de Física I – Cinemática y Dinámica",
        400.0,
        60.0,
        22.0,
        true,
    );
    h.centrado(
        "Universidad Nacional Jorge Basadre Grohmann – Escuela de Ingeniería Civil",
        400.0,
        95.0,
        12.0,
        false,
    );
    let mut y = 150.0;
    let lineas: &[(&str, &str)] = &[
        ("1. Movimiento rectilíneo uniformemente variado", ""),
        ("", "$v = v_0 + a t$"),
        ("", r"$x = x_0 + v_0 t + \frac{1}{2} a t^2$"),
        ("", r"$v^2 = v_0^2 + 2 a \Delta x$"),
        ("2. Caída libre y tiro parabólico", ""),
        (
            "",
            "$h_{max} = \\frac{v_0^2 \\sin^2\\theta}{2g}$     $R = \\frac{v_0^2 \\sin 2\\theta}{g}$",
        ),
        ("3. Dinámica y energía", ""),
        ("", r"$\sum \vec{F} = m \vec{a}$     $f_r = \mu N$"),
        ("", r"$E_c = \frac{1}{2} m v^2$     $E_p = m g h$"),
        ("4. Péndulo simple", ""),
        ("", "$T = 2\\pi\\sqrt{\\frac{L}{g}}$"),
        ("5. Matemática de apoyo", ""),
        ("", "$x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}$"),
        ("", r"$\int_0^{\pi} \sin x \, dx = 2$"),
    ];
    for (titulo, formula) in lineas {
        if !titulo.is_empty() {
            y += 14.0;
            y += h.texto(titulo, 60.0, y, 16.0, true) + 10.0;
        } else {
            y += h.rico(formula, 110.0, y, 22.0) + 18.0;
        }
    }
    h.texto("g = 9,81 m/s²", 60.0, y + 20.0, 14.0, false);
    std::fs::write(ruta(DOCS[2].nombre), a_pdf(&[h])).unwrap();
}

fn d04() {
    let mut h = Hoja::nueva(900.0, 600.0);
    // Una «pizarra» verde con tiza.
    h.caja(
        Figura::Rectangulo,
        0.0,
        0.0,
        900.0,
        600.0,
        ColorRgba::opaco(0.35, 0.25, 0.15),
        Some(ColorRgba::opaco(0.12, 0.30, 0.22)),
    );
    let tiza = ColorRgba::opaco(0.95, 0.95, 0.90);
    h.texto_color("Repaso para el parcial", 40.0, 30.0, 26.0, true, tiza);
    let formulas = [
        "Ley de Ohm:  $V = I R$",
        "Potencia:  $P = V I = I^2 R$",
        "Bernoulli:  $P_1 + \\frac{1}{2}\\rho v_1^2 + \\rho g h_1 = P_2 + \\frac{1}{2}\\rho v_2^2 + \\rho g h_2$",
        "Pitágoras:  $c = \\sqrt{a^2 + b^2}$",
        "Esfuerzo:  $\\sigma = \\frac{F}{A}$",
    ];
    let mut y = 100.0;
    for f in formulas {
        let i = h.els.len();
        y += h.rico(f, 50.0, y, 24.0) + 30.0;
        h.els[i].trazo = tiza;
    }
    let img = hoja_a_png(h, 1350);
    guardar_png(DOCS[3].nombre, &img);
}

const PRESUPUESTO: &[&[&str]] = &[
    &[
        "Partida",
        "Unidad",
        "Metrado",
        "P. unit. (S/)",
        "Parcial (S/)",
    ],
    &["Excavación de zanjas", "m3", "120", "38.50", "4620.00"],
    &["Concreto f'c=210 kg/cm2", "m3", "45", "420.00", "18900.00"],
    &[
        "Acero de refuerzo fy=4200",
        "kg",
        "3200",
        "5.80",
        "18560.00",
    ],
    &["Encofrado y desencofrado", "m2", "260", "48.00", "12480.00"],
    &["Tubería PVC 24\"", "m", "640", "95.00", "60800.00"],
    &["Sumideros con rejilla", "und", "6", "2150.00", "12900.00"],
    &["Total", "", "", "", "128260.00"],
];

fn d05() {
    std::fs::write(ruta(DOCS[4].nombre), xlsx("Presupuesto", PRESUPUESTO)).unwrap();
}

const NOTAS: &[&[&str]] = &[
    &[
        "N.°",
        "Alumno",
        "Parcial 1",
        "Parcial 2",
        "Final",
        "Promedio",
    ],
    &["1", "Apaza Choque, Luis", "14", "12", "15", "13.7"],
    &["2", "Condori Ticona, Ana", "17", "16", "18", "17.0"],
    &["3", "Flores Mamani, Jorge", "09", "11", "10", "10.0"],
    &["4", "Huanca Ramos, Rosa", "15", "18", "16", "16.3"],
    &["5", "Laura Quispe, Pedro", "12", "08", "11", "10.3"],
    &["6", "Mendoza Ccori, Elena", "19", "17", "18", "18.0"],
    &["7", "Pari Vargas, Miguel", "11", "13", "12", "12.0"],
    &["8", "Ticona Lupaca, Sofía", "16", "15", "17", "16.0"],
];

fn d06() {
    let mut h = Hoja::nueva(A4.0, A4.1);
    h.texto(
        "Registro de notas – Resistencia de Materiales I (2026-II)",
        60.0,
        60.0,
        18.0,
        true,
    );
    h.texto(
        "Docente: Ing. Carlos Vargas · Sección A · Nota aprobatoria: 10.5",
        60.0,
        95.0,
        13.0,
        false,
    );
    let fin = h.tabla(
        NOTAS,
        60.0,
        140.0,
        &[40.0, 240.0, 90.0, 90.0, 80.0, 100.0],
        34.0,
        13.0,
        true,
    );
    h.parrafo(
        "Observación: el promedio es la media de los dos parciales y el examen final. Los alumnos con promedio menor que 10.5 pueden rendir el examen sustitutorio el 30 de noviembre.",
        60.0,
        fin + 30.0,
        680.0,
        13.0,
    );
    std::fs::write(ruta(DOCS[5].nombre), a_pdf(&[h])).unwrap();
}

const STOCK: &[&[&str]] = &[
    &["Material", "Cantidad", "Unidad", "Proveedor"],
    &["Cemento Portland tipo I", "350", "bolsas", "Yura"],
    &[
        "Fierro corrugado 1/2\"",
        "120",
        "varillas",
        "Aceros Arequipa",
    ],
    &["Arena gruesa", "18", "m3", "Cantera Magollo"],
    &["Piedra chancada 1/2\"", "22", "m3", "Cantera Magollo"],
    &["Alambre negro N.° 16", "45", "kg", "Ferretería El Sol"],
    &["Clavos de 3\"", "25", "kg", "Ferretería El Sol"],
];

fn d07() {
    let mut h = Hoja::nueva(760.0, 360.0);
    h.texto(
        "Almacén de obra – stock al 14/10/2026",
        20.0,
        16.0,
        18.0,
        true,
    );
    h.tabla(
        STOCK,
        20.0,
        60.0,
        &[250.0, 100.0, 110.0, 260.0],
        38.0,
        15.0,
        true,
    );
    let img = hoja_a_png(h, 1300);
    guardar_png(DOCS[6].nombre, &img);
}

fn d08() {
    let mut fotos = HashMap::new();
    let mut md = String::from("# Tacna, entre el desierto y el mar\n\n");
    md.push_str("Tacna es la ciudad más austral del Perú. Su clima seco y soleado, con apenas 20 mm de lluvia al año, hace que el agua sea el recurso más valioso de la región, y que el turismo de naturaleza se concentre en los valles y en la costa.\n\n");
    if let Some((_, b)) = foto_libre(0, 900) {
        fotos.insert("foto-valle.jpg".to_string(), b);
        md.push_str("![El valle visto desde el mirador](foto-valle.jpg)\n\n");
    }
    md.push_str("## El valle y sus bodegas\n\nEn el valle de Tacna se cultivan olivos desde el siglo XVI. Hoy la región produce más del 60 % de la aceituna del país, y las bodegas de Pocollay y Calana reciben a miles de visitantes cada fin de semana. La ruta del pisco y del vino es la actividad turística que más creció en 2025, con 85 000 visitantes.\n\n");
    if let Some((_, b)) = foto_libre(3, 900) {
        fotos.insert("foto-costa.jpg".to_string(), b);
        md.push_str("![La costa de Boca del Río](foto-costa.jpg)\n\n");
    }
    md.push_str("## La costa\n\nLas playas de Boca del Río y Vila Vila son el destino de verano de los tacneños. En 2026 la Municipalidad terminó el malecón de 1,2 km y un estacionamiento para 300 vehículos, lo que duplicó las visitas de enero a marzo.\n\n");
    md.push_str("## Lo que falta\n\nLos operadores piden más señalización en las carreteras, servicios higiénicos en las playas y una oficina de información turística en el terminal terrestre, que recibe a los visitantes de Chile y de Bolivia.\n");
    std::fs::write(
        ruta(DOCS[7].nombre),
        docx(&md, "Tacna, entre el desierto y el mar", &fotos),
    )
    .unwrap();
}

fn d09() {
    let mut h = Hoja::nueva(A4.0, A4.1);
    h.caja(
        Figura::Rectangulo,
        0.0,
        0.0,
        800.0,
        110.0,
        ColorRgba::opaco(0.10, 0.25, 0.45),
        Some(ColorRgba::opaco(0.10, 0.25, 0.45)),
    );
    h.texto_color(
        "BOLETÍN DE OBRA",
        50.0,
        25.0,
        34.0,
        true,
        ColorRgba::opaco(1.0, 1.0, 1.0),
    );
    h.texto_color(
        "N.° 12 · Octubre de 2026 · Mejoramiento de la Av. Florida",
        50.0,
        75.0,
        14.0,
        false,
        ColorRgba::opaco(0.85, 0.90, 1.0),
    );
    // Dos columnas con una raya en medio.
    h.linea(
        &[(400.0, 140.0), (400.0, 1040.0)],
        1.0,
        ColorRgba::opaco(0.6, 0.6, 0.65),
    );
    let mut y = 140.0;
    y += h.texto("Avance general: 68 %", 50.0, y, 18.0, true) + 8.0;
    y = h.parrafo("Durante la primera quincena de octubre se terminó el vaciado de la losa del bloque A y se avanzó el 40 % del encofrado del bloque B. El avance físico de la obra llegó al 68 %, frente al 72 % programado.", 50.0, y, 320.0, 13.0) + 14.0;
    if let Some((r, _)) = foto_libre(1, 600) {
        h.foto(&r, 50.0, y, 320.0, 200.0);
        y += 206.0;
        y += h.texto("Vaciado de la losa del bloque A.", 50.0, y, 11.0, false) + 14.0;
    }
    y += h.texto("Seguridad", 50.0, y, 18.0, true) + 8.0;
    h.parrafo("Se cumplieron 120 días sin accidentes con tiempo perdido. En setiembre se dictaron seis charlas de cinco minutos sobre trabajos en altura y uso del arnés.", 50.0, y, 320.0, 13.0);
    let mut y = 140.0;
    y += h.texto("Próximas actividades", 430.0, y, 18.0, true) + 8.0;
    for item in [
        "• Vaciado de la losa del bloque B (27 de octubre).",
        "• Instalación de la tubería de desagüe del Jr. Antisuyo.",
        "• Pruebas de compresión de las probetas a 28 días.",
        "• Inicio del tarrajeo de los muros del primer piso.",
    ] {
        y = h.parrafo(item, 430.0, y, 320.0, 13.0) + 4.0;
    }
    y += 16.0;
    h.caja(
        Figura::Rectangulo,
        430.0,
        y,
        320.0,
        150.0,
        ColorRgba::opaco(0.85, 0.55, 0.10),
        Some(ColorRgba::opaco(1.0, 0.95, 0.85)),
    );
    h.texto("Dato del mes", 445.0, y + 12.0, 16.0, true);
    h.parrafo("Se usaron 410 m³ de concreto premezclado, con una resistencia media de 238 kg/cm² en las probetas a 7 días.", 445.0, y + 40.0, 290.0, 13.0);
    y += 175.0;
    y += h.texto("Contacto", 430.0, y, 18.0, true) + 8.0;
    h.parrafo("Oficina de obra: Av. Florida 345, Tacna. Teléfono 052-412233. Atención de lunes a viernes de 8:00 a 17:00.", 430.0, y, 320.0, 13.0);
    std::fs::write(ruta(DOCS[8].nombre), a_pdf(&[h])).unwrap();
}

/// Producción mensual de la grafica (mes, m3).
pub const PRODUCCION: &[(&str, f32)] = &[
    ("Ene", 120.0),
    ("Feb", 95.0),
    ("Mar", 140.0),
    ("Abr", 160.0),
    ("May", 130.0),
];

fn d10() {
    let mut h = Hoja::nueva(800.0, 560.0);
    h.centrado(
        "Producción mensual de concreto (m³) – 2026",
        400.0,
        20.0,
        22.0,
        true,
    );
    let (x0, y0, alto) = (110.0, 480.0, 380.0);
    let gris = ColorRgba::opaco(0.55, 0.55, 0.6);
    h.linea(&[(x0, y0), (x0 + 620.0, y0)], 2.0, negro());
    h.linea(&[(x0, y0), (x0, y0 - alto - 10.0)], 2.0, negro());
    for v in (0..=180).step_by(30) {
        let y = y0 - v as f32 / 180.0 * alto;
        h.linea(&[(x0 - 6.0, y), (x0 + 620.0, y)], 0.6, gris);
        h.texto(&v.to_string(), x0 - 45.0, y - 9.0, 14.0, false);
    }
    for (i, (mes, v)) in PRODUCCION.iter().enumerate() {
        let x = x0 + 40.0 + i as f32 * 115.0;
        let alto_b = v / 180.0 * alto;
        h.caja(
            Figura::Rectangulo,
            x,
            y0 - alto_b,
            70.0,
            alto_b,
            ColorRgba::opaco(0.15, 0.40, 0.75),
            Some(ColorRgba::opaco(0.25, 0.52, 0.86)),
        );
        h.centrado(
            &format!("{}", *v as i32),
            x + 35.0,
            y0 - alto_b - 24.0,
            14.0,
            true,
        );
        h.centrado(mes, x + 35.0, y0 + 10.0, 15.0, false);
    }
    h.texto("m³", 40.0, 80.0, 14.0, false);
    let img = hoja_a_png(h, 1200);
    guardar_png(DOCS[9].nombre, &img);
}

fn d11() {
    let mut h = Hoja::nueva(640.0, 820.0);
    let azul = ColorRgba::opaco(0.15, 0.35, 0.65);
    let claro = Some(ColorRgba::opaco(0.88, 0.93, 1.0));
    let cx = 320.0;
    h.caja(Figura::Elipse, cx - 80.0, 30.0, 160.0, 60.0, azul, claro);
    h.centrado("Inicio", cx, 48.0, 18.0, true);
    h.flecha((cx, 90.0), (cx, 140.0));
    // Leer N: paralelogramo
    let (px, py) = (cx - 105.0, 140.0);
    h.linea(
        &[
            (px + 30.0, py),
            (px + 210.0, py),
            (px + 180.0, py + 60.0),
            (px, py + 60.0),
            (px + 30.0, py),
        ],
        2.0,
        azul,
    );
    h.centrado("Leer N", cx, 158.0, 18.0, false);
    h.flecha((cx, 200.0), (cx, 250.0));
    h.caja(Figura::Rombo, cx - 120.0, 250.0, 240.0, 130.0, azul, claro);
    h.centrado("¿N mod 2 = 0?", cx, 304.0, 17.0, false);
    // Si a la izquierda, No a la derecha.
    h.flecha((cx - 120.0, 315.0), (110.0, 315.0));
    h.flecha((110.0, 315.0), (110.0, 450.0));
    h.texto("Sí", 150.0, 288.0, 16.0, true);
    h.flecha((cx + 120.0, 315.0), (530.0, 315.0));
    h.flecha((530.0, 315.0), (530.0, 450.0));
    h.texto("No", 465.0, 288.0, 16.0, true);
    h.caja(Figura::Rectangulo, 30.0, 450.0, 160.0, 64.0, azul, claro);
    h.centrado("Escribir \"Par\"", 110.0, 470.0, 16.0, false);
    h.caja(Figura::Rectangulo, 450.0, 450.0, 160.0, 64.0, azul, claro);
    h.centrado("Escribir \"Impar\"", 530.0, 470.0, 16.0, false);
    h.flecha((110.0, 514.0), (110.0, 600.0));
    h.flecha((110.0, 600.0), (cx - 80.0, 650.0));
    h.flecha((530.0, 514.0), (530.0, 600.0));
    h.flecha((530.0, 600.0), (cx + 80.0, 650.0));
    h.caja(Figura::Elipse, cx - 80.0, 640.0, 160.0, 60.0, azul, claro);
    h.centrado("Fin", cx, 658.0, 18.0, true);
    let img = hoja_a_png(h, 900);
    guardar_png(DOCS[10].nombre, &img);
}

fn d12() {
    let mut h = Hoja::nueva(760.0, 700.0);
    h.texto(
        "Problema 4. En la figura, AB es diámetro y m∢BAC = 35°. Halla x.",
        30.0,
        20.0,
        18.0,
        true,
    );
    let (cx, cy, r) = (380.0, 380.0, 230.0);
    h.caja(
        Figura::Elipse,
        cx - r,
        cy - r,
        2.0 * r,
        2.0 * r,
        negro(),
        None,
    );
    let a = (cx - r, cy);
    let b = (cx + r, cy);
    // C en la circunferencia, arriba a la izquierda: el angulo en A es 35°.
    let ang_c = (180.0f32 - 2.0 * 35.0).to_radians();
    let c = (cx + r * ang_c.cos(), cy - r * ang_c.sin());
    h.linea(&[a, b], 2.0, negro());
    h.linea(&[a, c, b], 2.0, negro());
    h.caja(
        Figura::Elipse,
        cx - 4.0,
        cy - 4.0,
        8.0,
        8.0,
        negro(),
        Some(negro()),
    );
    h.texto("O", cx - 6.0, cy + 8.0, 18.0, true);
    h.texto("A", a.0 - 28.0, a.1 - 12.0, 20.0, true);
    h.texto("B", b.0 + 10.0, b.1 - 12.0, 20.0, true);
    h.texto("C", c.0 - 6.0, c.1 - 32.0, 20.0, true);
    h.texto("35°", a.0 + 48.0, a.1 - 30.0, 17.0, false);
    h.texto("x", b.0 - 60.0, b.1 - 28.0, 19.0, true);
    // La marca del angulo recto en C.
    let (ux, uy) = ((a.0 - c.0) / 230.0, (a.1 - c.1) / 230.0);
    let (vx, vy) = ((b.0 - c.0) / 400.0, (b.1 - c.1) / 400.0);
    let m = 22.0;
    let n1 = (ux.hypot(uy), vx.hypot(vy));
    let p1 = (c.0 + ux / n1.0 * m, c.1 + uy / n1.0 * m);
    let p3 = (c.0 + vx / n1.1 * m, c.1 + vy / n1.1 * m);
    let p2 = (p1.0 + vx / n1.1 * m, p1.1 + vy / n1.1 * m);
    h.linea(&[p1, p2, p3], 1.5, negro());
    let img = hoja_a_png(h, 1100);
    guardar_png(DOCS[11].nombre, &img);
}

const ENSAYOS: &[&[&str]] = &[
    &[
        "Probeta",
        "Edad (días)",
        "Carga (kN)",
        "Resistencia (kg/cm²)",
    ],
    &["P-01", "7", "312", "176"],
    &["P-02", "7", "298", "168"],
    &["P-03", "28", "421", "238"],
    &["P-04", "28", "436", "246"],
];

fn d13() {
    let mut h = Hoja::nueva(A4.0, A4.1);
    h.centrado("GUÍA DE PRÁCTICA N.° 3", 400.0, 60.0, 22.0, true);
    h.centrado(
        "Tecnología del Concreto – Ensayo de compresión de probetas",
        400.0,
        95.0,
        15.0,
        false,
    );
    let mut y = 150.0;
    y = h.parrafo("Objetivo: determinar la resistencia a la compresión de probetas cilíndricas de concreto de 15 cm de diámetro y 30 cm de altura, según la norma NTP 339.034, y comparar el resultado con la resistencia de diseño f'c = 210 kg/cm².", 60.0, y, 680.0, 15.0) + 18.0;
    y += h.texto("Procedimiento", 60.0, y, 17.0, true) + 8.0;
    for p in [
        "1. Medir el diámetro de la probeta en dos direcciones perpendiculares.",
        "2. Colocar la probeta centrada en la prensa y aplicar la carga a velocidad constante.",
        "3. Anotar la carga máxima y el tipo de falla.",
        "4. Calcular la resistencia con la fórmula: f = P / A.",
    ] {
        y = h.parrafo(p, 60.0, y, 680.0, 15.0) + 4.0;
    }
    y += 20.0;
    y += h.texto("Resultados", 60.0, y, 17.0, true) + 10.0;
    let fin = h.tabla(
        ENSAYOS,
        60.0,
        y,
        &[120.0, 140.0, 140.0, 260.0],
        36.0,
        15.0,
        true,
    );
    h.parrafo("Conclusión: a los 28 días las probetas superan la resistencia de diseño; la media es 242 kg/cm², un 15 % más que f'c.", 60.0, fin + 26.0, 680.0, 15.0);
    let mut img = hoja_a_png(h, 1400);
    escanear(&mut img);
    let img = inclinar(&img, -2.4);
    guardar_png(DOCS[12].nombre, &img);
}

fn d14() {
    let emu = |px: f32| (px * 9525.0) as u64;
    let s1 = format!(
        "{}{}",
        caja_pptx(
            2,
            emu(60.0),
            emu(150.0),
            emu(840.0),
            emu(100.0),
            &[("Seguridad en obra: lo que no se negocia", 4000, true)]
        ),
        caja_pptx(
            3,
            emu(60.0),
            emu(280.0),
            emu(840.0),
            emu(60.0),
            &[(
                "Charla de cinco minutos – Obra Av. Florida – 15 de octubre de 2026",
                1800,
                false
            )]
        )
    );
    let s2 = format!(
        "{}{}",
        caja_pptx(
            2,
            emu(40.0),
            emu(30.0),
            emu(880.0),
            emu(70.0),
            &[("Equipo de protección personal (EPP)", 3200, true)]
        ),
        caja_pptx(
            3,
            emu(60.0),
            emu(120.0),
            emu(840.0),
            emu(380.0),
            &[
                (
                    "• Casco con barbiquejo en toda la obra, siempre.",
                    2000,
                    false
                ),
                (
                    "• Chaleco reflectivo, botas con puntera de acero y guantes.",
                    2000,
                    false
                ),
                (
                    "• Arnés con doble línea de vida por encima de 1,80 m.",
                    2000,
                    false
                ),
                (
                    "• Lentes de seguridad al cortar, esmerilar o picar concreto.",
                    2000,
                    false
                ),
                (
                    "• Nadie entra a una zanja de más de 1,50 m sin entibado.",
                    2000,
                    false
                ),
            ]
        )
    );
    let foto = foto_libre(2, 700).map(|(_, b)| b);
    let mut s3 = format!(
        "{}{}",
        caja_pptx(
            2,
            emu(40.0),
            emu(30.0),
            emu(880.0),
            emu(70.0),
            &[("Cómo vamos", 3200, true)]
        ),
        caja_pptx(
            3,
            emu(40.0),
            emu(120.0),
            emu(440.0),
            emu(360.0),
            &[
                (
                    "12 incidentes en 2025; 4 en lo que va de 2026.",
                    2000,
                    false
                ),
                ("120 días sin accidentes con tiempo perdido.", 2000, false),
                (
                    "La causa más común: caídas al mismo nivel por desorden.",
                    2000,
                    false
                ),
                ("Meta: cero accidentes al terminar la obra.", 2000, true),
            ]
        )
    );
    if foto.is_some() {
        s3.push_str(&foto_pptx(
            4,
            "rF",
            emu(500.0),
            emu(130.0),
            emu(420.0),
            emu(300.0),
        ));
    }
    let b = pptx(&[(s1, None), (s2, None), (s3, foto)]);
    std::fs::write(ruta(DOCS[13].nombre), b).unwrap();
}

/// Los metrados: la fila del tarrajeo esta mal (120 x 28.50 = 3420, no 3240).
pub const METRADOS: &[&[&str]] = &[
    &[
        "Partida",
        "Unidad",
        "Metrado",
        "P. unit. (S/)",
        "Parcial (S/)",
    ],
    &["Muro de ladrillo KK", "m2", "180", "62.00", "11160.00"],
    &["Tarrajeo de muros", "m2", "120", "28.50", "3240.00"],
    &["Contrapiso e=4cm", "m2", "95", "34.00", "3230.00"],
    &["Pintura látex 2 manos", "m2", "300", "12.50", "3750.00"],
    &["Puerta contraplacada", "und", "8", "380.00", "3040.00"],
    &["Total", "", "", "", "24420.00"],
];

fn d15() {
    std::fs::write(ruta(DOCS[14].nombre), xlsx("Metrados", METRADOS)).unwrap();
}

fn d16() {
    let mut h = Hoja::nueva(A4.0, A4.1);
    h.centrado("EXAMEN PARCIAL DE FÍSICA I", 400.0, 50.0, 22.0, true);
    h.texto(
        "Nombre: ______________________________     Fecha: 16/10/2026     Duración: 90 min",
        60.0,
        95.0,
        13.0,
        false,
    );
    let mut y = 140.0;
    y += h.texto("Problema 1 (6 puntos)", 60.0, y, 16.0, true) + 6.0;
    y = h.parrafo("Un bloque de 5 kg está sobre un plano inclinado 37° respecto de la horizontal. El coeficiente de rozamiento cinético entre el bloque y el plano es 0,2. Calcula la aceleración con que baja el bloque (g = 10 m/s²).", 60.0, y, 400.0, 14.0) + 10.0;
    y += h.rico("$a = g(\\sin\\theta - \\mu\\cos\\theta)$", 80.0, y, 20.0) + 20.0;
    // El plano inclinado a la derecha.
    let (px, py) = (500.0, 330.0);
    h.linea(
        &[
            (px, py),
            (px + 240.0, py),
            (px + 240.0, py - 180.0),
            (px, py),
        ],
        2.0,
        negro(),
    );
    h.texto("37°", px + 45.0, py - 30.0, 15.0, false);
    let ang = (180.0f32 / 240.0).atan();
    let (bx, by) = (px + 120.0, py - 120.0 * ang.tan());
    h.caja(
        Figura::Rectangulo,
        bx - 22.0,
        by - 44.0,
        44.0,
        40.0,
        ColorRgba::opaco(0.6, 0.2, 0.1),
        Some(ColorRgba::opaco(0.9, 0.6, 0.4)),
    );
    h.texto("5 kg", bx - 18.0, by - 36.0, 13.0, true);
    y = y.max(360.0);
    y += h.texto("Problema 2 (4 puntos)", 60.0, y, 16.0, true) + 6.0;
    y = h.parrafo("Un proyectil se lanza con una velocidad de 20 m/s y un ángulo de 30°. Halla su altura máxima.", 60.0, y, 680.0, 14.0) + 8.0;
    y += h.rico(
        "$h_{max} = \\frac{v_0^2 \\sin^2\\theta}{2g}$",
        80.0,
        y,
        20.0,
    ) + 24.0;
    y += h.texto(
        "Pregunta 3 (2 puntos). La unidad de la fuerza en el SI es:",
        60.0,
        y,
        16.0,
        true,
    ) + 10.0;
    for alt in ["a) joule", "b) newton", "c) watt", "d) pascal"] {
        y += h.texto(alt, 90.0, y, 14.0, false) + 6.0;
    }
    std::fs::write(ruta(DOCS[15].nombre), a_pdf(&[h])).unwrap();
}

/// **Hace el corpus entero** (si ya esta, lo rehace).
pub fn generar() {
    std::fs::create_dir_all(carpeta()).unwrap();
    let _com = pixpin_shell::ComDelHilo::iniciar();
    for f in [
        d01, d02, d03, d04, d05, d06, d07, d08, d09, d10, d11, d12, d13, d14, d15, d16,
    ] {
        f();
    }
}

/// La vista previa de un documento a PNG (la entrada de cada caso).
pub fn vista_previa(nombre: &str, ancho: u32) -> Option<ImagenRgba> {
    let r = ruta(nombre);
    let ext = pixpin_docs::extension(nombre);
    match ext.as_str() {
        "png" | "jpg" => pixpin_codec::imagen::cargar(&r).ok(),
        "pdf" => Some(pdf_a_png(&r, 0, ancho)),
        _ => None,
    }
}
