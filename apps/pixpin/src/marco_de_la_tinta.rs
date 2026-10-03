//! **La pasada del marco de la tinta** (K21, 29-sep-2026: «asegurate de
//! actualizar los archivos que ya estan anotados usando esta forma de
//! marco»).
//!
//! Lo anotado antes del marco no tiene `.hoja`: cada lector lo lee con la
//! regla vieja (`pixpin_docs::vista::capa_del_movil(espacios)` en un PDF del
//! chat, la columna corrida `izq` en un Word o un libro). Mientras quede una
//! sola tinta asi, la regla vieja no se puede retirar. Esta pasada le pone a
//! cada tinta `anot-<uid>[-p<n>]` de cada chat **el marco que dice justo como
//! se lee hoy**, en las dos lineas de Android, y **sin tocar la tinta**: nada
//! cambia de sitio, y desde ahi el marco manda en los dos aparatos.
//!
//! Cuando corre:
//!
//! - **Al arrancar, una vez**, en un hilo aparte (no frena el arranque), con
//!   una marca en el almacen (`sincro/marcos-de-la-tinta-v2.hecho`). Esa vez,
//!   antes, lo que el PC anoto junto al documento (`<doc>.pixpin-anotado/`)
//!   y aun no paso a su mensaje pasa como pasa al abrirlo (ver
//!   [`pasar_lo_de_antes`]).
//! - **Tras cada sincronizacion**, para lo que llegue sin marco (lo que
//!   anota un movil anterior a v0.98.0, que aun no lo escribe). Tambien
//!   deja en dos lineas los marcos que el PC escribio el 29-sep con la
//!   huella de su tinta: Android v0.98.0 no entiende uno de tres.
//!
//! Escribir lo mismo no toca el fichero: la pasada repetida no cambia
//! resumenes ni provoca envios. Cada `.hoja` nueva viaja una vez.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use pixpin_proyecto::anotado::{self as an, Pasada, QueTinta, TintaSinMarco};
use pixpin_proyecto::vista::DiscoPc;
use pixpin_sincro::anotado::MarcoDeLaHoja;

use crate::lector_tinta::Unidades;

/// La marca de que la pasada de una vez ya se hizo en este almacen.
///
/// Con version en el nombre: la del 29-sep escribio marcos de tres lineas
/// (con la huella) que Android 0.98 descarta; la `v2` vuelve a pasar una vez
/// para dejarlos en las dos lineas del movil.
pub fn marca(raiz: &Path) -> PathBuf {
    raiz.join("sincro").join("marcos-de-la-tinta-v2.hecho")
}

fn es_pdf(doc: &Path) -> bool {
    doc.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// **Lo que el PC anoto junto al documento, a su mensaje**, como se hace al
/// abrirlo en el lector (`anotado_del_adjunto::leer_capa` en un Word o un
/// libro, `hoja_del_pdf` en cada hoja de un PDF): se copia si el mensaje aun
/// no tiene tinta, o se junta con la que ya mando el movil, una sola vez
/// (cada uno deja su marca `.pasada`). Lo de junto al documento no se borra.
/// Devuelve cuantos documentos tenian algo de antes.
pub fn pasar_lo_de_antes(raiz: &Path) -> usize {
    let mut n = 0;
    for (chat, ficha) in DiscoPc::nuevo(raiz).mapa() {
        for (x, doc) in an::adjuntos_del_chat(raiz, &chat, &ficha.id) {
            if !doc.is_file() {
                continue;
            }
            if es_pdf(&doc) {
                if pixpin_proyecto::capas_del_pdf::de_este_pdf(raiz, &doc).is_some() {
                    continue;
                }
                let hojas = hojas_de_antes(&doc);
                if hojas.is_empty() {
                    continue;
                }
                n += 1;
                let adjunto = Some((raiz.to_path_buf(), x));
                for i in hojas {
                    crate::anotado_del_adjunto::hoja_del_pdf(adjunto.as_ref(), &doc, i);
                }
            } else if crate::lector_tinta::ruta_de_capa(&doc).is_file() {
                n += 1;
                let _ = crate::anotado_del_adjunto::leer_capa(&doc);
            }
        }
    }
    n
}

/// Las hojas (desde 0) con tinta de antes junto a un PDF (`hoja-<n>.excalidraw`).
fn hojas_de_antes(pdf: &Path) -> Vec<usize> {
    let Ok(dir) = std::fs::read_dir(crate::lector_tinta::carpeta_de(pdf)) else {
        return Vec::new();
    };
    let mut hojas: Vec<usize> = dir
        .flatten()
        .filter_map(|f| {
            let n = f.file_name().to_string_lossy().to_string();
            n.strip_prefix("hoja-")?.strip_suffix(".excalidraw")?.parse::<usize>().ok()?.checked_sub(1)
        })
        .collect();
    hojas.sort_unstable();
    hojas
}

/// El alto de cada hoja de un PDF en unidades del lector
/// (`pixpin_docs::vista::Hojas`), una vez por PDF y pasada.
fn altos_de(pdf: &Path, ya: &mut HashMap<PathBuf, Option<Vec<f32>>>) -> Option<Vec<f32>> {
    ya.entry(pdf.to_path_buf())
        .or_insert_with(|| match pixpin_pdf::Documento::abrir(pdf) {
            Ok(d) => Some(pixpin_docs::vista::Hojas::colocar(&d.medidas()).altos),
            Err(e) => {
                tracing::warn!(?e, pdf = %pdf.display(), "PDF que no se abre: su tinta se queda sin marco");
                None
            }
        })
        .clone()
}

/// **El marco que dice como se lee hoy una tinta sin marco** (la regla
/// vieja), o `None` si no se puede saber. En una hoja de PDF, antes, lo
/// que el PC dibujo ahi antes de saber las unidades del movil pasa a ellas
/// (`lo_del_pc_a_la_capa_del_movil`, una vez por hoja, como al abrirla): con
/// el marco puesto ya no se haria nunca, y ese trazo quedaria encogido.
pub fn marco_de_antes(
    raiz: &Path,
    t: &TintaSinMarco,
    altos: &mut HashMap<PathBuf, Option<Vec<f32>>>,
) -> Option<MarcoDeLaHoja> {
    let doc = t.doc.as_deref()?;
    match t.que {
        QueTinta::Documento if !es_pdf(doc) => crate::anotado_del_adjunto::marco_de_antes_del_documento(doc, &t.base),
        QueTinta::Pagina(i) if es_pdf(doc) => {
            // El PDF de un proyecto lleva la tinta en sus hojas (sin marco).
            if !doc.is_file() || pixpin_proyecto::capas_del_pdf::de_este_pdf(raiz, doc).is_some() {
                return None;
            }
            let i = usize::try_from(i).ok()?;
            let alto = *altos_de(doc, altos)?.get(i)?;
            let u = Unidades::de_la_capa_del_movil(crate::anotado_del_adjunto::espacios_de_ahora(doc));
            crate::anotado_del_adjunto::lo_del_pc_a_la_capa_del_movil(doc, i, &t.base.tinta(), u);
            Some(u.marco_de(&crate::lector_pdf_proyecto::hoja_propia(alto)))
        }
        _ => None,
    }
}

/// **Una pasada**: con `lo_de_antes`, primero [`pasar_lo_de_antes`]; luego
/// el marco de cada tinta que no lo tiene ([`an::poner_marcos`]).
pub fn pasada(raiz: &Path, lo_de_antes: bool) -> Pasada {
    if lo_de_antes {
        let n = pasar_lo_de_antes(raiz);
        if n > 0 {
            tracing::info!(documentos = n, "tinta de junto a los documentos pasada a sus mensajes");
        }
    }
    let mut altos = HashMap::new();
    let p = an::poner_marcos(raiz, &mut |t| marco_de_antes(raiz, t, &mut altos));
    for a in &p.avisos {
        tracing::warn!(aviso = %a, "pasada del marco de la tinta");
    }
    if p.escritos + p.sin_huella > 0 {
        tracing::info!(
            escritos = p.escritos,
            sin_huella = p.sin_huella,
            ya_estaban = p.ya_estaban,
            sin_calcular = p.sin_calcular,
            "marco de la tinta puesto a lo ya anotado"
        );
    }
    p
}

/// Si hay una pasada en marcha, y si se pidio otra mientras.
static EN_MARCHA: AtomicBool = AtomicBool::new(false);
static OTRA_VEZ: AtomicBool = AtomicBool::new(false);

/// Corre la pasada en un hilo aparte; si ya hay una, se deja pedida otra al
/// acabar (una sincronizacion puede terminar mientras corre la del arranque).
fn en_segundo_plano(raiz: PathBuf, lo_de_antes: bool) {
    if EN_MARCHA.swap(true, Ordering::SeqCst) {
        OTRA_VEZ.store(true, Ordering::SeqCst);
        return;
    }
    let hilo = std::thread::Builder::new().name("marco-de-la-tinta".into()).spawn(move || {
        let mut lo_de_antes = lo_de_antes;
        loop {
            OTRA_VEZ.store(false, Ordering::SeqCst);
            pasada(&raiz, lo_de_antes);
            if lo_de_antes {
                let m = marca(&raiz);
                let puesta = m.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&m, b""));
                if let Err(e) = puesta {
                    tracing::warn!(?e, "no se pudo dejar la marca de la pasada del marco");
                }
                lo_de_antes = false;
            }
            if !OTRA_VEZ.load(Ordering::SeqCst) {
                break;
            }
        }
        EN_MARCHA.store(false, Ordering::SeqCst);
    });
    if let Err(e) = hilo {
        EN_MARCHA.store(false, Ordering::SeqCst);
        tracing::warn!(?e, "no se pudo lanzar la pasada del marco de la tinta");
    }
}

/// **Al arrancar**: la pasada de una vez, si este almacen aun no la tiene.
pub fn al_arrancar(raiz: PathBuf) {
    if !marca(&raiz).exists() {
        en_segundo_plano(raiz, true);
    }
}

/// **Tras una sincronizacion**: el marco de lo que haya llegado sin el.
pub fn tras_sincronizar(raiz: &Path) {
    en_segundo_plano(raiz.to_path_buf(), false);
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::anotado_del_adjunto::pruebas::{con_adjunto, raiz};
    use crate::lector_pdf_proyecto::DondeVa;

    /// Tinta del movil sin espacios (unidades x2,5 con el cero en -1050).
    const TINTA: &str = r#"{"elements":[{"id":"jDNyxAFkaxm2qZ-KsluKG","type":"freedraw","x":-800,"y":1000,"width":250,"height":0,"strokeWidth":1,"version":1,"points":[{"x":0.0,"y":0.0},{"x":250.0,"y":0.0}]}]}"#;

    #[test]
    fn una_tinta_de_word_sin_marco_recibe_el_de_la_regla_vieja_y_se_lee_igual() {
        let r = raiz("pasada-word");
        let doc = con_adjunto(&r, "acta.docx", b"PK");
        let x = an::adjunto_de(&r, &doc).unwrap();
        let b = an::base_del_documento(&r, &doc).unwrap();
        an::escribir(&b.fichero(".maqueta"), "420,280,280,100,1,0").unwrap();
        std::fs::create_dir_all(b.tinta().parent().unwrap()).unwrap();
        let tinta = r#"{"elements":[{"id":"w1","type":"freedraw","x":580,"y":100,"width":200,"height":0,"strokeWidth":2,"version":1,"points":[{"x":0.0,"y":0.0},{"x":200.0,"y":0.0}]}],"files":{}}"#;
        std::fs::write(b.tinta(), tinta).unwrap();
        let antes = crate::anotado_del_adjunto::leer_capa(&doc);
        let caja_antes = antes.escena.caja();
        let p = pasada(&r, false);
        assert_eq!(p.escritos, 1, "{p:?}");
        assert_eq!(an::leer(&b.marco()).as_deref(), Some("280,0,700,420\nv1\n"));
        assert_eq!(std::fs::read_to_string(b.tinta()).unwrap(), tinta, "la tinta, byte a byte");
        assert_eq!(crate::anotado_del_adjunto::leer_capa(&doc).escena.caja(), caja_antes, "en el mismo sitio");
        assert_eq!(x.uid, b.base.trim_start_matches("anot-"));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_pasada_es_idempotente_y_la_hoja_de_un_pdf_que_no_se_abre_se_queda_sin_marco() {
        // Caso negativo: el «PDF» de la prueba no es un PDF de verdad, asi
        // que no hay medidas y no se inventa ningun marco.
        let r = raiz("pasada-pdf-roto");
        let pdf = con_adjunto(&r, "plano.pdf", b"%PDF-1.4");
        let d = DondeVa::de(&r, &pdf, 1);
        let tinta = d.para_escribir(&pdf, 0);
        std::fs::create_dir_all(tinta.parent().unwrap()).unwrap();
        std::fs::write(&tinta, TINTA).unwrap();
        let p = pasada(&r, true);
        assert_eq!((p.escritos, p.sin_calcular), (0, 1), "{p:?}");
        let b = crate::anotado_del_adjunto::base_de_la_hoja_del_pdf(crate::anotado_del_adjunto::adjunto_del_pdf(&pdf).as_ref(), 0).unwrap();
        assert!(!b.marco().exists());
        assert_eq!(std::fs::read_to_string(&tinta).unwrap(), TINTA);
        let _ = std::fs::remove_dir_all(&r);
    }

    /// Con un PDF de verdad (Windows.Data.Pdf): la hoja sin marco lo recibe
    /// y la tinta cae donde caia con la regla vieja, con los espacios del
    /// mensaje. Luego nada cambia en otra pasada.
    #[test]
    fn una_hoja_de_pdf_sin_marco_recibe_el_suyo_y_cae_donde_caia() {
        let bytes = pdf_de_muestra().expect("un PDF de una hoja");
        let r = raiz("pasada-pdf");
        let pdf = con_adjunto(&r, "plano.pdf", &bytes);
        let medidas = pixpin_pdf::Documento::abrir(&pdf).unwrap().medidas();
        let alto = pixpin_docs::vista::Hojas::colocar(&medidas).altos[0];
        let b = crate::anotado_del_adjunto::base_de_la_hoja_del_pdf(crate::anotado_del_adjunto::adjunto_del_pdf(&pdf).as_ref(), 0).unwrap();
        an::escribir(&an::base_del_pdf(&r, &pdf, None).unwrap().fichero(".espacios"), "1").unwrap();
        std::fs::create_dir_all(b.tinta().parent().unwrap()).unwrap();
        std::fs::write(b.tinta(), TINTA).unwrap();
        let d = DondeVa::de(&r, &pdf, 1);
        // Leida sin escribir (como compartir): el lector ya le apuntaria el marco.
        let antes = DondeVa::solo_leer(&pdf).leer_capa(&pdf, 0, 1, alto).escena.caja().unwrap();
        let p = pasada(&r, true);
        assert_eq!(p.escritos, 1, "{p:?}");
        let (m, h) = MarcoDeLaHoja::de_texto_con_huella(&an::leer(&b.marco()).unwrap()).unwrap();
        assert_eq!(h, None, "dos lineas, como Android");
        assert_eq!(m, Unidades::de_la_capa_del_movil(1).marco_de(&crate::lector_pdf_proyecto::hoja_propia(alto)));
        assert_eq!(std::fs::read_to_string(b.tinta()).unwrap(), TINTA, "la tinta, byte a byte");
        // Ya con marco, aunque cambien los espacios cae en el mismo sitio.
        let despues = d.leer_capa(&pdf, 0, 3, alto).escena.caja().unwrap();
        let cerca = |a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)| {
            [(a.0, b.0), (a.1, b.1), (a.2, b.2), (a.3, b.3)].iter().all(|(x, y)| (x - y).abs() < 0.01)
        };
        assert!(cerca(antes, despues), "{antes:?} {despues:?}");
        let fecha = std::fs::metadata(b.marco()).unwrap().modified().unwrap();
        let p = pasada(&r, true);
        assert_eq!((p.escritos, p.ya_estaban), (0, 1));
        assert_eq!(std::fs::metadata(b.marco()).unwrap().modified().unwrap(), fecha);
        let _ = std::fs::remove_dir_all(&r);
    }

    /// Un PDF de verdad de una hoja de 20 x 30 (Windows lo abre y lo mide).
    fn pdf_de_muestra() -> Option<Vec<u8>> {
        let img = pixpin_codec::ImagenRgba { ancho: 20, alto: 30, pixeles: [7, 90, 200, 255].repeat(600) };
        pixpin_pdf::union::de_imagenes(&[img])
    }

    #[test]
    fn la_pasada_de_una_vez_pasa_la_tinta_de_junto_al_documento_y_le_pone_su_marco() {
        let r = raiz("pasada-antes");
        let doc = con_adjunto(&r, "notas.docx", b"PK");
        pixpin_docs::lectura::escribir(&doc, &pixpin_docs::lectura::Ajustes { columna: 600, ..Default::default() }).unwrap();
        let vieja = crate::lector_tinta::ruta_de_capa(&doc);
        std::fs::create_dir_all(vieja.parent().unwrap()).unwrap();
        std::fs::write(&vieja, r#"{"type":"excalidraw","elements":[{"id":"pc1","type":"freedraw","x":10,"y":40,"width":2,"height":2,"points":[[0,0],[2,2]]}]}"#).unwrap();
        let b = an::base_del_documento(&r, &doc).unwrap();
        // Caso negativo: la de tras sincronizar no pasa lo de antes.
        let p = pasada(&r, false);
        assert_eq!(p.escritos, 0);
        assert!(!b.tinta().exists());
        let p = pasada(&r, true);
        assert!(b.tinta().is_file(), "pasada a su mensaje");
        // `leer_capa` ya le puso su marco al pasarla; la pasada lo encuentra.
        assert_eq!(p.ya_estaban + p.escritos, 1, "{p:?}");
        let (m, h) = MarcoDeLaHoja::de_texto_con_huella(&an::leer(&b.marco()).unwrap()).unwrap();
        assert_eq!(m, MarcoDeLaHoja::nuevo(400.0, 0.0, 1000.0, 600.0));
        assert_eq!(h, None, "dos lineas, como Android");
        assert!(vieja.is_file(), "lo de antes no se borra");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_marca_va_en_la_carpeta_de_sincronizar_del_almacen() {
        assert_eq!(marca(Path::new("C:/a")), Path::new("C:/a").join("sincro").join("marcos-de-la-tinta-v2.hecho"));
    }
}

/// **Con una copia de los datos del usuario** (nunca los de verdad): tras
/// la pasada, cada tinta de un mensaje tiene su `.hoja` y todo cae donde
/// caia. Se lanza a mano:
/// `PIXPIN_DATOS_COPIA=<copia> cargo test -p pixpin --bin pixpinmax pasada_con_los_datos_copiados -- --ignored --nocapture`.
#[cfg(test)]
mod con_datos_reales {
    use super::*;

    /// La caja de cada tinta de cada mensaje, como la lee el lector ahora.
    /// `como_al_abrir` hace lo que hace el lector al abrir cada hoja (la
    /// migracion de lo del PC de antes del 29-sep), para comparar despues
    /// solo lo que cambia el marco.
    fn cajas(raiz: &Path, como_al_abrir: bool) -> Vec<(String, Option<(f32, f32, f32, f32)>)> {
        let mut salida = Vec::new();
        for (chat, ficha) in DiscoPc::nuevo(raiz).mapa() {
            for (x, doc) in an::adjuntos_del_chat(raiz, &chat, &ficha.id) {
                if !doc.is_file() {
                    continue;
                }
                if es_pdf(&doc) {
                    let Ok(d) = pixpin_pdf::Documento::abrir(&doc) else { continue };
                    let altos = pixpin_docs::vista::Hojas::colocar(&d.medidas()).altos;
                    let donde = if como_al_abrir {
                        crate::lector_pdf_proyecto::DondeVa::de(raiz, &doc, 1)
                    } else {
                        crate::lector_pdf_proyecto::DondeVa::solo_leer(&doc)
                    };
                    let espacios = crate::anotado_del_adjunto::espacios_de_ahora(&doc);
                    for (i, alto) in altos.iter().enumerate() {
                        if donde.para_leer(&doc, i).is_file() {
                            let c = donde.leer_capa(&doc, i, espacios, *alto);
                            salida.push((format!("{}-p{i}", x.uid), c.escena.caja()));
                        }
                    }
                } else if an::base_del_documento(raiz, &doc).is_some_and(|b| b.tinta().is_file()) {
                    salida.push((x.uid.clone(), crate::anotado_del_adjunto::leer_capa(&doc).escena.caja()));
                }
            }
        }
        salida
    }

    #[test]
    #[ignore = "necesita una copia de los datos del usuario"]
    fn pasada_con_los_datos_copiados_cada_tinta_tiene_su_marco_y_cae_donde_caia() {
        let raiz = PathBuf::from(std::env::var("PIXPIN_DATOS_COPIA").expect("PIXPIN_DATOS_COPIA"));
        // Primero lo de antes (como la pasada de una vez), para comparar
        // despues solo el efecto del marco.
        let n = pasar_lo_de_antes(&raiz);
        println!("documentos con tinta de antes: {n}");
        let antes = cajas(&raiz, true);
        let tintas_antes: Vec<(PathBuf, Vec<u8>)> = tintas(&raiz).into_iter().map(|t| (t.clone(), std::fs::read(&t).unwrap())).collect();
        let p = pasada(&raiz, true);
        println!("{p:?}");
        let despues = cajas(&raiz, false);
        for ((a, ca), (_, cd)) in antes.iter().zip(&despues) {
            println!("{a}: antes {ca:?} despues {cd:?}");
        }
        assert_eq!(antes.len(), despues.len());
        for ((a, ca), (_, cd)) in antes.iter().zip(&despues) {
            let (Some(ca), Some(cd)) = (ca, cd) else {
                assert_eq!(ca, cd, "{a}");
                continue;
            };
            let d = [(ca.0 - cd.0), (ca.1 - cd.1), (ca.2 - cd.2), (ca.3 - cd.3)].map(f32::abs);
            assert!(d.iter().all(|x| *x < 0.01), "{a}: {ca:?} -> {cd:?}");
        }
        for (t, bytes) in &tintas_antes {
            assert_eq!(&std::fs::read(t).unwrap(), bytes, "tinta tocada: {}", t.display());
        }
        // Cada tinta de un mensaje (menos las de un PDF leido como texto)
        // tiene su `.hoja` en dos lineas.
        use pixpin_sincro::disco::Disco;
        let d = DiscoPc::nuevo(&raiz);
        let mut sin = Vec::new();
        for (chat, _) in d.mapa() {
            for rel in d.anotado(&chat) {
                let Some(base) = rel.strip_suffix(".excalidraw.gz") else { continue };
                if base.ends_with("-texto") {
                    continue;
                }
                let hoja = d.ruta(&chat, &format!("{base}.hoja"));
                match an::leer(&hoja).and_then(|t| MarcoDeLaHoja::de_texto_con_huella(&t)) {
                    Some((_, None)) => {}
                    otro => sin.push(format!("{rel}: {otro:?}")),
                }
            }
        }
        assert!(sin.is_empty(), "sin marco: {sin:#?}");
        // Y otra pasada no hace nada.
        let otra = pasada(&raiz, true);
        assert_eq!((otra.escritos, otra.sin_huella), (0, 0), "{otra:?}");
    }

    /// Las tintas de mensajes del almacen (rutas en el disco del PC).
    fn tintas(raiz: &Path) -> Vec<PathBuf> {
        use pixpin_sincro::disco::Disco;
        let d = DiscoPc::nuevo(raiz);
        let mut v = Vec::new();
        for (chat, _) in d.mapa() {
            for rel in d.anotado(&chat) {
                if rel.ends_with(".excalidraw.gz") {
                    v.push(d.ruta(&chat, &rel));
                }
            }
        }
        v
    }
}
