//! El PDF de lo compartido con las paginas del documento copiadas tal cual
//! (`PdfUnion.soloPaginas` del movil) y en el orden de la hoja.

use super::super::*;
use super::{de_hojas, pagina_sola, por_tramos};
use pixpin_motor2d::{Elemento, Figura};
use pixpin_store::Idioma;

fn carpeta(nombre: &str) -> PathBuf {
    let c = std::env::temp_dir().join(format!(
        "pixpin-compartir-pdf-{nombre}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&c);
    std::fs::create_dir_all(&c).unwrap();
    c
}

fn trazo() -> Elemento {
    Elemento {
        figura: Figura::Lapiz {
            puntos: (0..20)
                .map(|i| Punto2::nuevo(100.0 + i as f32 * 6.0, 200.0 + (i % 3) as f32 * 4.0))
                .collect(),
            presiones: Vec::new(),
            opciones: Some(Default::default()),
        },
        grosor: 3.0,
        trazo: ColorRgba::opaco(0.9, 0.1, 0.1),
        ..Elemento::default()
    }
}

/// Un PDF de texto de tres paginas, vectorial, escrito aqui.
fn documento(dir: &Path) -> PathBuf {
    let pdf = dir.join("memoria.pdf");
    let hojas = hojas_de_texto(
        Some("Memoria"),
        &"Parrafo de prueba con letras. ".repeat(900),
    );
    assert!(hojas.len() >= 3);
    std::fs::write(
        &pdf,
        pixpin_pdf::escribir::de_hojas(&hojas[..3], Some(BLANCO), &|_| None).unwrap(),
    )
    .unwrap();
    pdf
}

fn papel(pdf: &Path, pagina: u32) -> Option<(FuenteImagen, f32, f32)> {
    Some((
        FuenteImagen::PaginaPdf {
            pdf: pdf.to_path_buf(),
            pagina,
            ancho: 1400,
        },
        1400.0,
        1980.0,
    ))
}

fn lineas(bytes: &[u8], pagina: usize) -> bool {
    pixpin_pdf::plano::de_bytes(bytes, pagina).is_some_and(|p| p.se_manda_como_lineas())
}

#[test]
fn las_paginas_del_pdf_salen_tal_cual_y_en_el_orden_de_la_hoja() {
    let dir = carpeta("orden");
    let pdf = documento(&dir);
    let t = Catalogo::nuevo(Idioma::Espanol);
    let mut p = Preparado::nuevo("Entrega");
    let mut con_trazo = Escena::nueva();
    con_trazo.anadir(trazo());
    assert!(anadir_lienzo(
        &mut p,
        "a",
        "",
        &Escena::nueva(),
        papel(&pdf, 0),
        &|_| None,
        &t
    ));
    assert!(anadir_lienzo(
        &mut p,
        "b",
        "",
        &con_trazo,
        None,
        &|_| None,
        &t
    ));
    assert!(anadir_lienzo(
        &mut p,
        "c",
        "",
        &con_trazo,
        papel(&pdf, 1),
        &|_| None,
        &t
    ));
    assert!(anadir_lienzo(
        &mut p,
        "d",
        "",
        &Escena::nueva(),
        papel(&pdf, 2),
        &|_| None,
        &t
    ));
    let claves: Vec<String> = ["a", "b", "c", "d"].map(String::from).to_vec();
    let salida = generar(&p, PDF, &claves, &dir).unwrap();
    let bytes = std::fs::read(&salida.ficheros[0]).unwrap();
    assert_eq!(pixpin_pdf::union::contar_paginas(&bytes), Some(4));
    // La primera y la ultima son las del documento, copiadas: su texto sigue
    // siendo texto.
    let primera = pixpin_pdf::plano::de_bytes(&bytes, 0).unwrap();
    // (Se cuentan letras y no rotulos: los trozos de un renglon van juntos.)
    let letras: usize = primera.textos.iter().map(|t| t.texto.chars().count()).sum();
    assert!(
        lineas(&bytes, 0) && letras > 40,
        "la pagina copiada lleva su texto: {letras}"
    );
    assert!(lineas(&bytes, 3));
    // La que lleva algo dibujado encima se compone como siempre: la pagina
    // pintada debajo (una foto) y el trazo encima.
    let tercera = pixpin_pdf::plano::de_bytes(&bytes, 2).unwrap();
    assert!(
        tercera.fotos.len() == 1 && tercera.textos.is_empty(),
        "con algo encima la pagina va pintada"
    );
    // Y todo el documento pesa menos que pintando las tres paginas.
    let todo_pintado = de_hojas(
        &claves
            .iter()
            .map(|k| p.pieza_de(k).unwrap())
            .collect::<Vec<_>>(),
        &Lector {
            fuentes: &p.imagenes,
            pdfs: Default::default(),
        },
    )
    .unwrap();
    assert!(
        bytes.len() < todo_pintado.len(),
        "{} >= {}",
        bytes.len(),
        todo_pintado.len()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sin_paginas_sueltas_del_pdf_se_escribe_como_siempre() {
    let dir = carpeta("sin");
    let pdf = documento(&dir);
    let t = Catalogo::nuevo(Idioma::Espanol);
    let mut p = Preparado::nuevo("Entrega");
    let mut con_trazo = Escena::nueva();
    con_trazo.anadir(trazo());
    assert!(anadir_lienzo(
        &mut p,
        "c",
        "",
        &con_trazo,
        papel(&pdf, 1),
        &|_| None,
        &t
    ));
    let pieza = p.pieza_de("c").unwrap();
    assert!(
        pagina_sola(pieza, &p.imagenes).is_none(),
        "una pagina con algo encima no es una pagina sola"
    );
    let lector = Lector {
        fuentes: &p.imagenes,
        pdfs: Default::default(),
    };
    assert!(por_tramos(&[pieza], &lector).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn si_el_pdf_ya_no_esta_se_cae_a_pintar_y_no_falla() {
    let dir = carpeta("perdido");
    let t = Catalogo::nuevo(Idioma::Espanol);
    let mut p = Preparado::nuevo("Entrega");
    let perdido = dir.join("no-esta.pdf");
    let mut con_trazo = Escena::nueva();
    con_trazo.anadir(trazo());
    assert!(anadir_lienzo(
        &mut p,
        "a",
        "",
        &Escena::nueva(),
        papel(&perdido, 0),
        &|_| None,
        &t
    ));
    assert!(anadir_lienzo(
        &mut p,
        "b",
        "",
        &con_trazo,
        None,
        &|_| None,
        &t
    ));
    let claves: Vec<String> = ["a", "b"].map(String::from).to_vec();
    let salida = generar(&p, PDF, &claves, &dir).unwrap();
    let bytes = std::fs::read(&salida.ficheros[0]).unwrap();
    assert!(pixpin_pdf::union::contar_paginas(&bytes).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}
