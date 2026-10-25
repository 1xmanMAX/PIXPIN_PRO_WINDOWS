//! **Cuando la pantalla que se graba se va**: un monitor que se desconecta
//! (el proyector del aula, el portatil que se cierra con un monitor
//! externo) o cambia de manejador al volver.
//!
//! Lo que hace OBS (`plugins/win-capture/duplicator-monitor-capture.c`):
//! si la captura se cae, lo intenta otra vez cada tres segundos
//! (`RESET_INTERVAL_SEC`) y antes vuelve a buscar el monitor **por su
//! nombre de dispositivo** (`update_monitor_handle`), porque el `HMONITOR`
//! cambia al desconectarlo y conectarlo. Aqui igual, y si ese monitor ya
//! no esta se pasa al principal: en una clase es mejor seguir grabando
//! otra pantalla que una imagen congelada durante una hora. Mientras, el
//! video repite la ultima imagen y el sonido no se corta.
//!
//! Una **ventana** que se cierra no vuelve (su `HWND` ya no existe): el
//! video sigue con su ultima imagen hasta que se termine.

use crate::reloj::UNIDADES_POR_SEGUNDO;

/// Lo que se espera entre dos intentos.
pub const CADA: i64 = 3 * UNIDADES_POR_SEGUNDO;

/// Cuando toca volver a intentar abrir la fuente.
#[derive(Debug, Clone, Default)]
pub struct Reintento {
    proximo: Option<i64>,
}

impl Reintento {
    /// Si toca a la hora `ahora` (el primero, enseguida); si toca, apunta el
    /// siguiente.
    pub fn toca(&mut self, ahora: i64) -> bool {
        match self.proximo {
            Some(p) if ahora < p => false,
            _ => {
                self.proximo = Some(ahora + CADA);
                true
            }
        }
    }

    /// La fuente volvio: el proximo fallo se intenta enseguida.
    pub fn olvidar(&mut self) {
        self.proximo = None;
    }
}

/// **Que monitor grabar ahora**: el que tiene el nombre de dispositivo del
/// de antes (`\\.\DISPLAY2`), o el principal si ya no esta. `disponibles`
/// son los monitores conectados, con su manejador y su nombre.
pub fn elegir_monitor(nombre: &str, disponibles: &[(isize, String)], principal: isize) -> isize {
    disponibles
        .iter()
        .find(|(_, n)| !nombre.is_empty() && n == nombre)
        .map_or(principal, |(h, _)| *h)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const S: i64 = UNIDADES_POR_SEGUNDO;

    #[test]
    fn se_reintenta_cada_tres_segundos() {
        let mut r = Reintento::default();
        assert!(r.toca(10 * S), "el primero, enseguida");
        // Caso negativo: no antes de tres segundos.
        assert!(!r.toca(11 * S));
        assert!(!r.toca(13 * S - 1));
        assert!(r.toca(13 * S));
        r.olvidar();
        assert!(r.toca(13 * S + 1));
    }

    #[test]
    fn el_monitor_se_busca_por_su_nombre_y_si_no_el_principal() {
        let v = vec![
            (11, r"\\.\DISPLAY1".to_string()),
            (77, r"\\.\DISPLAY2".to_string()),
        ];
        // Volvio con otro manejador: el nuevo.
        assert_eq!(elegir_monitor(r"\\.\DISPLAY2", &v, 11), 77);
        // Caso negativo: ya no esta (o no se supo su nombre): el principal.
        assert_eq!(elegir_monitor(r"\\.\DISPLAY3", &v, 11), 11);
        assert_eq!(elegir_monitor("", &v, 11), 11);
    }
}
