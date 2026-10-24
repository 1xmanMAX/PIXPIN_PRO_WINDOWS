//! **Mirar y editar** (24-oct-2026, el usuario: «cuando entro al canvas
//! siempre entre en modo visualizacion y con el lapiz de la barra lateral
//! pueda activar el modo edicion; en el modo edicion que todos los botones
//! desaparezcan y que sea pantalla completa a excepcion de la barra
//! lateral»).
//!
//! - El lienzo abre siempre **mirando**: arrastrar mueve el dibujo y nada se
//!   elige ni se edita. El riel de la derecha sigue a la vista, con las
//!   marcas y el lapiz.
//! - El **lapiz del riel** (o Alt+R) pasa a editar: el lienzo se pone a
//!   pantalla completa y la barra del pin (nombre, fijar, pantalla completa,
//!   cerrar) se va. Quedan las herramientas para dibujar, el riel y el
//!   panel de propiedades, que se quita con su boton.
//! - El lapiz otra vez (o Escape) vuelve a mirar y, si estaba en ventana
//!   antes de editar, a la ventana.
//!
//! Aqui solo lo que se decide sin ventana, para probarlo.

/// Lo que se acuerda del modo de la ventana al ponerse a editar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct AlEditar {
    /// Estaba en ventana al empezar a editar: al volver a mirar, vuelve a
    /// ella.
    volver_a_ventana: bool,
}

impl AlEditar {
    /// Al pasar a editar (`editando`) o a mirar: si hay que alternar la
    /// pantalla completa (sin recordarlo como preferencia del usuario).
    pub fn al_cambiar(&mut self, editando: bool, pantalla_completa: bool) -> bool {
        if editando {
            self.volver_a_ventana = !pantalla_completa;
            !pantalla_completa
        } else {
            std::mem::take(&mut self.volver_a_ventana) && pantalla_completa
        }
    }

    /// El usuario cambio el modo el mismo (F11, el boton de la barra): lo
    /// que eligio manda y ya no se le devuelve a la ventana.
    pub fn a_mano(&mut self) {
        self.volver_a_ventana = false;
    }
}

/// **Escape editando** vuelve a mirar si no hay nada a medias (un texto o
/// un arrastre los suelta antes el motor) ni se presenta; en el anotador de
/// pantalla no, que tiene su regla.
pub(super) fn escape_vuelve_a_mirar(
    editando: bool,
    es_lienzo: bool,
    sin_presentar: bool,
    gesto_en_reposo: bool,
) -> bool {
    editando && es_lienzo && sin_presentar && gesto_en_reposo
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn editar_pone_pantalla_completa_y_mirar_devuelve_a_la_ventana() {
        let mut m = AlEditar::default();
        // En ventana: editar la pone a pantalla completa...
        assert!(m.al_cambiar(true, false));
        // ...y volver a mirar, ya a pantalla completa, la devuelve.
        assert!(m.al_cambiar(false, true));
        // Caso negativo: ya a pantalla completa, ni al editar ni al volver
        // se toca nada (es lo que eligio el usuario).
        assert!(!m.al_cambiar(true, true));
        assert!(!m.al_cambiar(false, true));
        // Editando paso a ventana a mano (F11): al volver a mirar se queda.
        assert!(m.al_cambiar(true, false));
        m.a_mano();
        assert!(!m.al_cambiar(false, false));
    }

    #[test]
    fn escape_editando_vuelve_a_mirar_solo_sin_nada_a_medias() {
        assert!(escape_vuelve_a_mirar(true, true, true, true));
        assert!(
            !escape_vuelve_a_mirar(false, true, true, true),
            "mirando sigue lo de siempre"
        );
        assert!(
            !escape_vuelve_a_mirar(true, true, true, false),
            "un arrastre a medias lo cancela el motor"
        );
        assert!(
            !escape_vuelve_a_mirar(true, true, false, true),
            "presentando, deja de presentar"
        );
        assert!(
            !escape_vuelve_a_mirar(true, false, true, true),
            "el anotador de pantalla tiene su regla"
        );
    }
}
