//! **«Abrir con…»** en toda la app (13-oct-2026, el usuario: «que el boton
//! sea uno que diga "Abrir con…" y me de la opcion de abrir con cualquier
//! app que pueda abrir el formato»): el boton ↗ de los pines-visor, el menu
//! del chat, el buscador, las diapositivas sin PowerPoint... todos llaman
//! aqui, que llama al cuadro «Abrir con» de Windows
//! (`pixpin_shell::abrir_con_otra`) y, si no sale, lo dice con palabras:
//! «no se encuentra el archivo X» en vez de un «no se pudo, mira el
//! registro» que no explica nada.

use std::path::Path;

use pixpin_shell::abrir_con_otra::ErrorAbrirCon;
use pixpin_store::Catalogo;
use windows::Win32::Foundation::HWND;

/// El aviso para el usuario de un «Abrir con…» que no salio.
pub fn aviso(textos: &Catalogo, e: &ErrorAbrirCon) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    match e {
        ErrorAbrirCon::NoExiste(r) => {
            args.set("nombre", pixpin_docs::nombre(r));
            textos.t_args("abrir-con-no-existe", &args)
        }
        ErrorAbrirCon::Windows(_) => textos.t("abrir-con-fallo"),
    }
}

/// Abre el cuadro para `ruta`. `Ok(true)` si se abrio con algo,
/// `Ok(false)` si se cerro sin elegir y, si no salio, el aviso ya escrito
/// (y apuntado en el registro).
pub fn abrir_con(padre: HWND, ruta: &Path, textos: &Catalogo) -> Result<bool, String> {
    pixpin_shell::abrir_con_otra::abrir_con_otra(padre, ruta).map_err(|e| {
        tracing::warn!(%e, "«Abrir con…» no salio");
        aviso(textos, &e)
    })
}

/// Lo mismo para quien no tiene donde ensenar un aviso (un pin-visor): si
/// no sale, un cuadro con el motivo sobre `padre`.
pub fn abrir_con_o_avisar(padre: HWND, ruta: &Path, textos: &Catalogo) -> bool {
    match abrir_con(padre, ruta, textos) {
        Ok(abierto) => abierto,
        Err(aviso) => {
            pixpin_shell::exportar::informar(padre, &textos.t("abrir-con"), &aviso);
            false
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_store::Idioma;

    #[test]
    fn un_archivo_que_falta_se_dice_con_su_nombre_y_no_con_el_registro() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let falta = std::env::temp_dir().join("pixpin-abrir-con-app").join("Presupuesto 2026.xlsx");
        let aviso = abrir_con(HWND::default(), &falta, &textos).unwrap_err();
        assert!(aviso.contains("Presupuesto 2026.xlsx"), "{aviso}");
        // Caso negativo: nunca el mensaje de antes.
        assert_ne!(aviso, textos.t("chat-no-se-pudo"));
        assert!(!aviso.to_lowercase().contains("registro"), "{aviso}");
    }

    #[test]
    fn el_rotulo_es_abrir_con_en_los_dos_idiomas() {
        assert_eq!(Catalogo::nuevo(Idioma::Espanol).t("chat-abrir-con"), "Abrir con…");
        assert_eq!(Catalogo::nuevo(Idioma::Ingles).t("chat-abrir-con"), "Open with…");
        assert_eq!(Catalogo::nuevo(Idioma::Espanol).t("abrir-con"), "Abrir con…");
        assert_eq!(Catalogo::nuevo(Idioma::Ingles).t("abrir-con"), "Open with…");
    }

    #[test]
    fn el_aviso_de_windows_no_es_el_del_registro() {
        let textos = Catalogo::nuevo(Idioma::Ingles);
        let e = ErrorAbrirCon::Windows(windows::core::Error::from(windows::Win32::Foundation::E_INVALIDARG));
        let a = aviso(&textos, &e);
        assert!(!a.is_empty() && a != textos.t("chat-no-se-pudo"), "{a}");
    }
}
