//! **Los comentarios de la nota** (H12, 30-sep-2026): de donde los lee y
//! adonde los escribe el editor. La ventana y el panel son de
//! `pixpin_notas::editor::comentarios`; el formato, de
//! `pixpin_docs::md_comentarios`; el sitio, de
//! `pixpin_proyecto::comentarios_de_notas` (junto a lo anotado del chat, que
//! es lo que los hace viajar con la nota y borrarse con ella).

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use pixpin_proyecto::comentarios_de_notas as sitio;
use pixpin_store::Catalogo;

use super::Destino;

/// El fichero de comentarios de la nota. Una nota nueva aun no tiene: sus
/// comentarios se escriben al guardarse ella por primera vez.
pub fn ruta(raiz: &Path, destino: &Destino) -> Option<PathBuf> {
    match destino {
        Destino::Mensaje { proyecto, codigo } => sitio::de_la_nota(raiz, proyecto, codigo),
        Destino::Nueva { .. } => None,
        Destino::Fichero { ruta } => Some(sitio::del_md(raiz, ruta)),
    }
}

/// Lo que necesita el editor: quien comenta (el nombre de este aparato) y
/// leer y escribir el fichero de la nota que este abierta (`actual` cambia
/// cuando una nota nueva se guarda).
pub fn de(
    raiz: &Path,
    actual: &Rc<RefCell<Destino>>,
    aparato: &str,
) -> pixpin_notas::DeComentarios {
    let autor = pixpin_proyecto::identidad::Identidad::leer_o_crear(raiz, "PC")
        .map(|i| i.yo.nombre)
        .unwrap_or_else(|_| "PC".into());
    let (r1, r2) = (raiz.to_path_buf(), raiz.to_path_buf());
    let (d1, d2) = (actual.clone(), actual.clone());
    pixpin_notas::DeComentarios {
        autor,
        aparato: aparato.to_string(),
        leer: Some(Box::new(move || match ruta(&r1, &d1.borrow()) {
            Some(f) => sitio::leer(&f),
            None => Some(String::new()),
        })),
        guardar: Some(Box::new(move |texto: &str| {
            let Some(f) = ruta(&r2, &d2.borrow()) else {
                return false;
            };
            match sitio::escribir(&f, texto) {
                Ok(()) => true,
                Err(e) => {
                    tracing::warn!(?e, ruta = %f.display(), "no se pudieron guardar los comentarios");
                    false
                }
            }
        })),
    }
}

/// Los textos del panel.
pub fn rotulos(t: &Catalogo) -> pixpin_notas::RotulosComentarios {
    pixpin_notas::RotulosComentarios {
        comentarios: t.t("nota-md-com-titulo"),
        comentar: t.t("nota-md-com-comentar"),
        sin_comentarios: t.t("nota-md-com-ninguno"),
        pista: t.t("nota-md-com-pista"),
        sin_ancla: t.t("nota-md-com-sin-ancla"),
        responder_pista: t.t("nota-md-com-responder-pista"),
        responder: t.t("nota-md-com-responder"),
        guardar: t.t("nota-md-com-guardar"),
        cancelar: t.t("nota-md-com-cancelar"),
        editar: t.t("nota-md-com-editar"),
        borrar: t.t("nota-md-com-borrar"),
        resuelto: t.t("nota-md-com-resuelto"),
        ver_resueltos: t.t("nota-md-com-ver-resueltos"),
        ocultar_resueltos: t.t("nota-md-com-ocultar-resueltos"),
        editado: t.t("nota-md-com-editado"),
        borrar_hilo: t.t("nota-md-com-borrar-hilo"),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_nota_nueva_aun_no_tiene_fichero_de_comentarios() {
        let raiz = std::env::temp_dir().join(format!("pixpin-com-app-{}", std::process::id()));
        assert_eq!(
            ruta(
                &raiz,
                &Destino::Nueva {
                    proyecto: "p1".into()
                }
            ),
            None
        );
        // Y una de un proyecto que no esta, tampoco (no se inventa sitio).
        let d = Destino::Mensaje {
            proyecto: "no-esta".into(),
            codigo: "ABCDE23456".into(),
        };
        assert_eq!(ruta(&raiz, &d), None);
    }

    #[test]
    fn un_md_suelto_los_lleva_a_su_lado() {
        let raiz = std::env::temp_dir().join(format!("pixpin-com-app-md-{}", std::process::id()));
        let md = raiz.join("fuera").join("apuntes.md");
        assert_eq!(
            ruta(&raiz, &Destino::Fichero { ruta: md }),
            Some(raiz.join("fuera").join("apuntes.comentarios.json"))
        );
    }

    #[test]
    fn todos_los_textos_del_panel_existen_en_los_dos_idiomas() {
        for idioma in [pixpin_store::Idioma::Espanol, pixpin_store::Idioma::Ingles] {
            let r = rotulos(&Catalogo::nuevo(idioma));
            for t in [
                &r.comentarios,
                &r.comentar,
                &r.sin_comentarios,
                &r.pista,
                &r.borrar_hilo,
                &r.ver_resueltos,
            ] {
                assert!(
                    !t.is_empty() && !t.starts_with("nota-md-com"),
                    "falta un texto: {t:?}"
                );
            }
        }
    }
}
