//! **Donde escribe su tinta el lector de PDF**, como `CapasDelPdf` del
//! movil: en la hoja del proyecto si el PDF es de uno (y asi viaja al
//! sincronizar), o junto al PDF si no (`lector_tinta::ruta_de_hoja`, que en
//! el movil tampoco viaja). La decision esta en
//! `pixpin_proyecto::capas_del_pdf`; aqui solo se le da al lector la ruta de
//! cada capa.

use std::path::{Path, PathBuf};

use pixpin_proyecto::capas_del_pdf::{self, DelProyecto};

/// Donde va lo anotado de un PDF abierto en el lector.
#[derive(Debug, Clone)]
pub struct DondeVa {
    raiz: PathBuf,
    /// De que proyecto es, si es de uno.
    proyecto: Option<DelProyecto>,
    huella: String,
    /// Si es un adjunto del chat (y no de un proyecto): su tinta va con el
    /// codigo de su mensaje (`anot-<uid>-p<n>`, v0.96). Se busca una vez.
    adjunto: Option<(PathBuf, pixpin_proyecto::anotado::Adjunto)>,
}

impl DondeVa {
    /// Mira si `pdf` es de un proyecto y, si lo es, adopta en sus hojas lo
    /// que se anoto antes junto al PDF (cuando el PC aun no lo hacia asi).
    pub fn de(raiz: &Path, pdf: &Path, ahora: i64) -> DondeVa {
        let proyecto = capas_del_pdf::de_este_pdf(raiz, pdf);
        let adjunto = if proyecto.is_none() { crate::anotado_del_adjunto::adjunto_del_pdf(pdf) } else { None };
        let d = DondeVa {
            raiz: raiz.to_path_buf(),
            huella: capas_del_pdf::huella(&pdf.to_string_lossy()),
            proyecto,
            adjunto,
        };
        if let Some(p) = &d.proyecto {
            d.adoptar(&p.ficha, pdf, ahora);
        }
        d
    }

    /// Para quien solo lee lo anotado (compartir, la pagina web) y no sabe
    /// donde esta el almacen: se busca subiendo desde el PDF
    /// (`<raiz>/proyectos/<ficha>/…`). No adopta ni escribe nada.
    pub fn solo_leer(pdf: &Path) -> DondeVa {
        let raiz = pdf
            .ancestors()
            .find(|a| {
                a.file_name().is_some_and(|n| n == "proyectos") && a.join("indice.json").is_file()
            })
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        let proyecto = raiz.as_deref().and_then(|r| capas_del_pdf::de_este_pdf(r, pdf));
        let adjunto = if proyecto.is_none() { crate::anotado_del_adjunto::adjunto_del_pdf(pdf) } else { None };
        DondeVa {
            proyecto,
            raiz: raiz.unwrap_or_default(),
            huella: capas_del_pdf::huella(&pdf.to_string_lossy()),
            adjunto,
        }
    }

    fn adoptar(&self, ficha: &str, pdf: &Path, ahora: i64) {
        let Ok(dir) = std::fs::read_dir(crate::lector_tinta::carpeta_de(pdf)) else {
            return;
        };
        for f in dir.flatten() {
            let nombre = f.file_name().to_string_lossy().to_string();
            // `hoja-<n>.excalidraw`, con `n` desde 1.
            let Some(n) = nombre
                .strip_prefix("hoja-")
                .and_then(|x| x.strip_suffix(".excalidraw"))
                .and_then(|x| x.parse::<u32>().ok())
                .filter(|n| *n >= 1)
            else {
                continue;
            };
            match capas_del_pdf::adoptar(&self.raiz, ficha, n - 1, &self.huella, &f.path(), ahora) {
                Ok(true) => tracing::info!(%ficha, hoja = n, "tinta del lector adoptada en la hoja del proyecto"),
                Ok(false) => {}
                Err(e) => tracing::warn!(?e, %ficha, hoja = n, "no se pudo adoptar la tinta del lector"),
            }
        }
    }

    /// Si el PDF es de un proyecto.
    pub fn es_de_un_proyecto(&self) -> bool {
        self.proyecto.is_some()
    }

    /// Lo que se pinta: la copia limpia del proyecto si la hay (el documento
    /// que viene del movil lleva lo anotado cocido dentro y aqui va ademas
    /// encima), o el propio PDF.
    pub fn documento(&self, pdf: &Path) -> PathBuf {
        self.proyecto
            .as_ref()
            .and_then(|p| p.limpio.clone())
            .unwrap_or_else(|| pdf.to_path_buf())
    }

    /// De donde se lee la capa de la hoja `i` (desde 0). No cambia nada.
    pub fn para_leer(&self, pdf: &Path, i: usize) -> PathBuf {
        self.ruta(pdf, i, false)
    }

    /// Donde se escribe la capa de la hoja `i`: en un proyecto, la hoja
    /// recibe su dibujo si aun no tenia (y el proyecto lo apunta).
    pub fn para_escribir(&self, pdf: &Path, i: usize) -> PathBuf {
        self.ruta(pdf, i, true)
    }

    fn ruta(&self, pdf: &Path, i: usize, escribir: bool) -> PathBuf {
        if let Some(p) = &self.proyecto
            && let Ok(pagina) = u32::try_from(i)
            && let Some(d) = capas_del_pdf::dibujo_de_la_pagina(
                &self.raiz,
                &p.ficha,
                pagina,
                &self.huella,
                escribir,
                pixpin_shell::entorno::ahora_utc_ms(),
            )
        {
            return capas_del_pdf::fichero(&self.raiz, &p.ficha, &d);
        }
        // Un adjunto del chat, con el codigo de su mensaje; si no, junto al PDF.
        crate::anotado_del_adjunto::hoja_del_pdf(self.adjunto.as_ref(), pdf, i)
            .unwrap_or_else(|| crate::lector_tinta::ruta_de_hoja(pdf, i))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::almacen::{self, Ficha, Indice};
    use pixpin_proyecto::{Hoja, Proyecto};

    fn raiz(etiqueta: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("pixpin-lector-proyecto-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    fn proyecto_con_pdf(r: &Path) -> (Ficha, PathBuf) {
        let ficha = Ficha::nueva("Plano", 5, "PC01");
        let mut i = Indice::default();
        i.proyectos.push(ficha.clone());
        i.guardar(r).unwrap();
        let carpeta = almacen::carpeta(r, &ficha.id);
        std::fs::create_dir_all(carpeta.join("archivos")).unwrap();
        let doc = carpeta.join("archivos/doc-1.pdf");
        std::fs::write(&doc, b"%PDF-1.4").unwrap();
        let p = Proyecto {
            id: ficha.id.clone(),
            hojas: (0..2)
                .map(|n| Hoja {
                    id: format!("h{n}"),
                    pagina: Some(n),
                    ..Default::default()
                })
                .collect(),
            pdf_origen: Some("archivos/doc-1.pdf".into()),
            ..Default::default()
        };
        std::fs::write(carpeta.join("proyecto.json"), serde_json::to_string(&p).unwrap()).unwrap();
        (ficha, doc)
    }

    #[test]
    fn la_tinta_del_pdf_de_un_proyecto_va_al_dibujo_de_su_hoja_y_viaja_con_el() {
        let r = raiz("proyecto");
        let (ficha, doc) = proyecto_con_pdf(&r);
        // Lo que se anoto antes junto al PDF se adopta al abrir.
        let hermana = crate::lector_tinta::carpeta_de(&doc);
        std::fs::create_dir_all(&hermana).unwrap();
        std::fs::write(
            hermana.join("hoja-1.excalidraw"),
            r#"{"elements":[{"id":"a","type":"freedraw","x":0,"y":0,"points":[[0,0],[1,1]]}]}"#,
        )
        .unwrap();
        let d = DondeVa::de(&r, &doc, 100);
        assert!(d.es_de_un_proyecto());
        let h = capas_del_pdf::huella(&doc.to_string_lossy());
        let hoja0 = almacen::lienzo(&r, &ficha.id, &format!("pdf-{h}-p0"));
        assert_eq!(d.para_leer(&doc, 0), hoja0);
        assert!(hoja0.is_file(), "lo de antes ya esta en la hoja");
        // La hoja 1 aun no tiene dibujo: leer no se lo pone, escribir si.
        let hoja1 = almacen::lienzo(&r, &ficha.id, &format!("pdf-{h}-p1"));
        assert_eq!(d.para_leer(&doc, 1), hoja1);
        let json = || std::fs::read_to_string(almacen::carpeta(&r, &ficha.id).join("proyecto.json")).unwrap();
        assert!(!json().contains(&format!("pdf-{h}-p1")));
        assert_eq!(d.para_escribir(&doc, 1), hoja1);
        assert!(json().contains(&format!("pdf-{h}-p1")));
        // Y lo escrito ahi es lo que el sincronizar manda con el proyecto.
        std::fs::write(&hoja1, r#"{"elements":[]}"#).unwrap();
        use pixpin_sincro::disco::Disco;
        let disco = pixpin_proyecto::vista::DiscoPc::nuevo(&r);
        let chat = pixpin_proyecto::vista::chat_de_ficha(&r, &ficha.id).unwrap();
        let alcance = disco.alcance(&chat).unwrap();
        assert!(alcance.iter().any(|(rel, _)| *rel == format!("pins/draw/pdf-{h}-p1.excalidraw.gz")));
        assert!(alcance.iter().any(|(rel, _)| *rel == format!("pins/draw/pdf-{h}-p0.excalidraw.gz")));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_tinta_de_un_pdf_suelto_sigue_junto_al_pdf() {
        // Caso negativo: un PDF que no es de ningun proyecto no toca ninguno.
        let r = raiz("suelto");
        let (ficha, _) = proyecto_con_pdf(&r);
        let suelto = r.join("informe.pdf");
        std::fs::write(&suelto, b"%PDF-1.4").unwrap();
        let d = DondeVa::de(&r, &suelto, 1);
        assert!(!d.es_de_un_proyecto());
        assert_eq!(d.para_escribir(&suelto, 3), crate::lector_tinta::ruta_de_hoja(&suelto, 3));
        assert_eq!(d.documento(&suelto), suelto);
        let json = std::fs::read_to_string(almacen::carpeta(&r, &ficha.id).join("proyecto.json")).unwrap();
        assert!(!json.contains("pdf-"));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn compartir_encuentra_la_tinta_del_proyecto_sin_saber_donde_esta_el_almacen() {
        let r = raiz("solo-leer");
        let (ficha, doc) = proyecto_con_pdf(&r);
        let h = capas_del_pdf::huella(&doc.to_string_lossy());
        DondeVa::de(&r, &doc, 1).para_escribir(&doc, 0);
        let d = DondeVa::solo_leer(&doc);
        assert!(d.es_de_un_proyecto());
        assert_eq!(d.para_leer(&doc, 0), almacen::lienzo(&r, &ficha.id, &format!("pdf-{h}-p0")));
        // Caso negativo: un PDF fuera de cualquier almacen lee junto a el.
        let fuera = std::env::temp_dir().join("pixpin-fuera-de-todo.pdf");
        let d = DondeVa::solo_leer(&fuera);
        assert!(!d.es_de_un_proyecto());
        assert_eq!(d.para_leer(&fuera, 2), crate::lector_tinta::ruta_de_hoja(&fuera, 2));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn una_pagina_que_el_proyecto_no_tiene_se_anota_junto_al_pdf() {
        // Caso negativo: el proyecto tiene las hojas 0 y 1; la 7 es suelta.
        let r = raiz("sin-hoja");
        let (_, doc) = proyecto_con_pdf(&r);
        let d = DondeVa::de(&r, &doc, 1);
        assert_eq!(d.para_escribir(&doc, 7), crate::lector_tinta::ruta_de_hoja(&doc, 7));
        let _ = std::fs::remove_dir_all(&r);
    }
}
