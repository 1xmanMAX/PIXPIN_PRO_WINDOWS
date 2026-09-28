//! **El PDF de lo compartido, con las paginas del PDF como paginas de
//! verdad.** Puerto de lo que hace `ExportarProyecto.aArchivo` del movil
//! con `PdfUnion.soloPaginas`: la pagina uno de un proyecto y las siete a
//! diez de otro salen del documento **tal cual** —vectoriales, con su texto
//! que se busca y se copia—, no como la foto de 1400 px que se pintaba para
//! ponerla en la hoja. Lo demas (lienzos, notas, tablas, paginas con algo
//! dibujado encima) se escribe como siempre con `pixpin_pdf::escribir`.
//!
//! **En el orden de la hoja de compartir**, que es una mejora sobre el
//! movil: alli las paginas del PDF iban todas delante y los dibujos detras;
//! aqui se escriben por tramos seguidos y se pegan en orden. Si algo no se
//! deja pegar (un PDF cifrado), el documento entero sale como antes, con
//! las paginas pintadas: mejor pesado que sin paginas.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use pixpin_motor2d::exportar::Hoja;
use pixpin_motor2d::pintado::Orden;

use super::{BLANCO, FuenteImagen, Lector, Pieza, rect};

/// Si la pieza es **una pagina del PDF y nada mas**: el papel solo, sin
/// nada dibujado encima, blanco y sin marcos. Esa se copia del documento;
/// con algo encima hay que componerla, y eso lo hace el escritor.
pub(super) fn pagina_sola(pieza: &Pieza, fuentes: &HashMap<u64, FuenteImagen>) -> Option<(PathBuf, u32)> {
    let h = &pieza.hoja;
    if pieza.fondo != BLANCO || h.ordenes.len() != 1 || !h.granos.is_empty() || !h.grafitos.is_empty() || !h.marcos.is_empty() {
        return None;
    }
    let Orden::Imagen { id_objeto, .. } = &h.ordenes[0] else {
        return None;
    };
    match fuentes.get(id_objeto)? {
        FuenteImagen::PaginaPdf { pdf, pagina, .. } => Some((pdf.clone(), *pagina)),
        _ => None,
    }
}

/// Un tramo seguido de lo que se entrega.
enum Tramo<'a> {
    /// Paginas de un mismo PDF, desde 0 y en orden.
    Paginas(PathBuf, Vec<usize>),
    /// Hojas que se escriben.
    Hojas(Vec<&'a Pieza>),
}

fn tramos<'a>(piezas: &[&'a Pieza], fuentes: &HashMap<u64, FuenteImagen>) -> Vec<Tramo<'a>> {
    let mut salida: Vec<Tramo<'a>> = Vec::new();
    for &x in piezas {
        match (pagina_sola(x, fuentes), salida.last_mut()) {
            (Some((pdf, n)), Some(Tramo::Paginas(ultimo, lista))) if *ultimo == pdf => lista.push(n as usize),
            (Some((pdf, n)), _) => salida.push(Tramo::Paginas(pdf, vec![n as usize])),
            (None, Some(Tramo::Hojas(lista))) => lista.push(x),
            (None, _) => salida.push(Tramo::Hojas(vec![x])),
        }
    }
    salida
}

/// **El PDF de estas piezas**, en su orden.
pub(super) fn de_piezas(piezas: &[&Pieza], lector: &Lector) -> Result<Vec<u8>> {
    match por_tramos(piezas, lector) {
        Some(b) => Ok(b),
        None => de_hojas(piezas, lector),
    }
}

/// Por tramos, o `None` si algun tramo de paginas no se deja copiar.
fn por_tramos(piezas: &[&Pieza], lector: &Lector) -> Option<Vec<u8>> {
    let tramos = tramos(piezas, lector.fuentes);
    if !tramos.iter().any(|t| matches!(t, Tramo::Paginas(..))) {
        return None;
    }
    let mut leidos: HashMap<PathBuf, Vec<u8>> = HashMap::new();
    let mut documento: Option<Vec<u8>> = None;
    for t in tramos {
        let trozo = match t {
            Tramo::Paginas(pdf, lista) => {
                if !leidos.contains_key(&pdf) {
                    let bytes = std::fs::read(&pdf)
                        .inspect_err(|e| tracing::info!(?e, pdf = %pdf.display(), "PDF que no se lee para compartir"))
                        .ok()?;
                    leidos.insert(pdf.clone(), bytes);
                }
                pixpin_pdf::union::solo_paginas(&leidos[&pdf], &lista)?
            }
            Tramo::Hojas(lista) => de_hojas(&lista, lector).ok()?,
        };
        documento = Some(match documento {
            None => trozo,
            Some(d) => pixpin_pdf::union::anadir_paginas(&d, &trozo)?,
        });
    }
    documento
}

/// Las hojas escritas con `pixpin_pdf::escribir`, como siempre: una pagina
/// del PDF va pintada, como foto.
fn de_hojas(piezas: &[&Pieza], lector: &Lector) -> Result<Vec<u8>> {
    // La letra de la pantalla, para que el PDF parta las lineas donde ella.
    // Sin ella, Helvetica.
    let segoe = pixpin_pdf::letra::del_sistema("Segoe UI");
    let hojas: Vec<Hoja> = piezas.iter().map(|x| con_su_papel(x)).collect();
    pixpin_pdf::escribir::de_hojas_con_letra(
        &hojas,
        Some(BLANCO),
        &|id| {
            lector.leer(id).map(|i| pixpin_pdf::escribir::Pixeles {
                ancho: i.ancho,
                alto: i.alto,
                rgba: i.pixeles.clone(),
            })
        },
        segoe.as_ref(),
    )
    .context("sin paginas")
}

/// El papel de cada pagina, debajo de todo: el escritor pone uno solo para
/// todas y un Word va en oscuro.
fn con_su_papel(x: &Pieza) -> Hoja {
    let mut h = x.hoja.clone();
    if x.fondo != BLANCO {
        let (a, b, c, d) = h.caja;
        h.ordenes.insert(
            0,
            Orden::Relleno {
                puntos: rect(a, b, c, d),
                color: x.fondo,
            },
        );
        for g in h.granos.iter_mut() {
            g.0 += 1;
        }
        for g in h.grafitos.iter_mut() {
            g.antes_de += 1;
        }
    }
    h
}

#[cfg(test)]
#[path = "pdf/pruebas.rs"]
mod pruebas;
