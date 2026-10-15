//! **El PDF de un documento que se lee, con lo anotado o limpio** (29-sep).
//! Puerto de `ExportarPdfAnotado.formatoPdf` y `PdfConAnotaciones` del
//! movil (v0.97.0).
//!
//! Antes el «Exportar» del lector y el «PDF» de la hoja de compartir pintaban
//! cada hoja como fotografia: un PDF de cien hojas y 3 MB salia de 14 o 15,
//! sin texto que buscar y borroso al ampliar (queja del usuario). Ahora es
//! **el PDF de siempre** con una revision al final: la tinta en vectores en
//! su capa, los margenes del lector y los marcadores en el indice
//! (`pixpin_pdf::con_anotaciones`). Con el interruptor «Con anotaciones»
//! quitado, el PDF limpio tal cual.
//!
//! Lo usan el lector (con la tinta que tiene en memoria, [`con_tinta`]) y la
//! hoja de compartir, venga del lector o del chat ([`de_documento`], que lo
//! lee todo de disco como el `DelChat` del movil).

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use pixpin_docs::vista;
use pixpin_motor2d::marcas::{self, Marca};
use pixpin_motor2d::pintado::{self, Orden};
use pixpin_pdf::con_anotaciones::{self, Anotaciones, Marcador};

/// **Los marcadores del lector como los del indice**: «⭐ Hoja 3», en el
/// orden del documento (`PdfConAnotaciones.marcadoresDe`).
pub(crate) fn marcadores_de(lista: &[Marca]) -> Vec<Marcador> {
    let mut ordenadas = lista.to_vec();
    marcas::en_orden(&mut ordenadas);
    ordenadas
        .iter()
        .map(|m| {
            let hoja = marcas::pagina_de(m) as usize;
            Marcador {
                titulo: format!("{} Hoja {}", m.emoji, hoja + 1),
                pagina: hoja,
                alto: marcas::alto_en_la_pagina(m),
            }
        })
        .collect()
}

/// **El PDF sin nada anotado dentro**: en un proyecto, su copia limpia; y si
/// aun asi trae capas de PixPin cocidas por el movil, cortado antes de
/// ellas. Si no, lo anotado saldria dos veces.
pub(crate) fn base_limpia(ruta: &Path) -> Result<Vec<u8>> {
    let documento = crate::lector_pdf_proyecto::DondeVa::solo_leer(ruta).documento(ruta);
    let mut bytes = std::fs::read(&documento)
        .with_context(|| format!("no se pudo leer {}", documento.display()))?;
    if let Some(n) = pixpin_pdf::cocido::largo_sin_lo_cocido(&bytes) {
        bytes.truncate(n);
    }
    Ok(bytes)
}

/// **`base` con esta tinta** (por hoja, en unidades del lector), estos
/// espacios para anotar (`vista::espacios_del_pdf`) y estos marcadores.
/// `None` si el PDF no se deja (cifrado, roto): quien llama cae a las fotos.
pub(crate) fn con_tinta(
    base: &[u8],
    tinta: &HashMap<usize, Vec<Orden>>,
    (izquierda, derecha): (f32, f32),
    marcadores: &[Marcador],
) -> Option<Vec<u8>> {
    // La letra de la pantalla, para que un texto de la tinta se vea igual.
    let segoe = pixpin_pdf::letra::del_sistema("Segoe UI");
    let de_la_hoja = |i: usize| tinta.get(&i).cloned();
    con_anotaciones::hacer(
        base,
        &Anotaciones {
            tinta: &de_la_hoja,
            izquierda,
            derecha,
            marcadores,
            imagenes: &|_| None,
            letra: segoe.as_ref(),
        },
    )
}

/// **El PDF de las hojas `paginas`** (desde 0; `None`, todas) de este
/// documento, con lo anotado que tenga en disco o limpio. Unas cuantas
/// hojas salen como paginas de verdad del original (`union::solo_paginas`),
/// no como fotos.
pub(crate) fn de_documento(
    ruta: &Path,
    paginas: Option<&[usize]>,
    con_anotaciones: bool,
) -> Result<Vec<u8>> {
    let original = base_limpia(ruta)?;
    let total = pixpin_pdf::union::contar_paginas(&original).unwrap_or(0) as usize;
    let todas: Vec<usize> = (0..total).collect();
    let cuales: Vec<usize> = paginas.map_or_else(
        || todas.clone(),
        |p| p.iter().copied().filter(|i| *i < total).collect(),
    );
    anyhow::ensure!(
        !cuales.is_empty() || total == 0,
        "el PDF no tiene esas hojas"
    );
    let enteras = cuales == todas || total == 0;
    let base = if enteras {
        original
    } else {
        pixpin_pdf::union::solo_paginas(&original, &cuales)
            .context("esas hojas no se dejan copiar")?
    };
    if !con_anotaciones {
        return Ok(base);
    }

    // Lo anotado, leido como lo lee el lector (la tinta de un PDF de
    // proyecto esta en sus hojas).
    let ajustes = crate::anotado_del_adjunto::leer(ruta);
    let doc = pixpin_pdf::Documento::abrir(ruta).map_err(|e| anyhow::anyhow!("{e}"))?;
    let hojas = vista::Hojas::colocar(&doc.medidas());
    let donde = crate::lector_pdf_proyecto::DondeVa::solo_leer(ruta);
    let mut tinta = HashMap::new();
    for (j, &i) in cuales.iter().enumerate() {
        if !donde.para_leer(ruta, i).is_file() {
            continue;
        }
        let alto = hojas.altos.get(i).copied().unwrap_or(0.0);
        let ordenes =
            pintado::ordenes_de_escena(&donde.leer_capa(ruta, i, ajustes.espacios, alto).escena);
        if !ordenes.is_empty() {
            tinta.insert(j, ordenes);
        }
    }
    // Los marcadores de las hojas que van, con su numero nuevo.
    let marcadores: Vec<Marcador> = marcadores_de(&marcas::de_texto(&ajustes.marcas))
        .into_iter()
        .filter_map(|mut m| {
            let j = cuales.iter().position(|&i| i == m.pagina)?;
            m.pagina = j;
            Some(m)
        })
        .collect();
    con_tinta(
        &base,
        &tinta,
        vista::espacios_del_pdf(ajustes.espacios),
        &marcadores,
    )
    .context("el PDF no se deja anotar (cifrado o roto)")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_marcadores_van_en_orden_con_su_emoticono_y_su_hoja() {
        let lista = marcas::de_texto("2:0.5:3.25:⭐|1:0.5:0.5:🔖");
        let m = marcadores_de(&lista);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].titulo, "🔖 Hoja 1");
        assert_eq!((m[1].pagina, m[1].alto), (3, 0.25));
        assert!(marcadores_de(&[]).is_empty());
    }
}
