//! **El modo oscuro de los visores, uno para toda la app** (12-oct-2026, el
//! usuario: «todos los visores con modo oscuro, con un boton luna/sol en la
//! barra de arriba del pin. Se recuerda»).
//!
//! Se cambia con el boton luna/sol de la barra de los pines-lector
//! (`lector_pin`) y vale para todos a la vez: los que se abran despues y
//! los que ya estan abiertos, que miran [`version`] en cada vuelta de su
//! bucle (despiertan como mucho cada 50-100 ms) y se repintan si cambio.
//!
//! Se guarda en `estado.toml` (`pixpin_store::estado`, lo que el programa
//! recuerda solo). Si nunca se toco, manda el **tema de Windows** para las
//! aplicaciones: el mismo criterio que Android, donde el lector sigue el
//! modo oscuro del telefono.
//!
//! Como se pinta cada visor en oscuro esta en `pixpin_render::oscuro`.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use pixpin_store::Ubicacion;

static ACTIVO: AtomicBool = AtomicBool::new(false);
/// Sube cada vez que cambia: los visores abiertos la comparan con la suya.
static VERSION: AtomicU32 = AtomicU32::new(0);
static UBICACION: OnceLock<Ubicacion> = OnceLock::new();

/// Lo que vale al arrancar: lo guardado o, si nunca se toco, lo contrario
/// del tema claro de Windows.
pub fn decidir(guardado: Option<bool>, windows_en_claro: bool) -> bool {
    guardado.unwrap_or(!windows_en_claro)
}

/// Lee la preferencia (una vez, al arrancar la app).
pub fn iniciar(ubicacion: &Ubicacion) {
    let guardado = pixpin_store::estado::cargar(ubicacion).lectores_oscuros;
    ACTIVO.store(
        decidir(guardado, pixpin_shell::entorno::tema_claro()),
        Ordering::Relaxed,
    );
    let _ = UBICACION.set(ubicacion.clone());
}

/// Donde estan los datos de la app (la que se fijo al arrancar), para
/// quien no la recibe (el «Exportar…» de un pin).
pub fn ubicacion() -> Option<Ubicacion> {
    UBICACION.get().cloned()
}

/// Si los visores van en oscuro.
pub fn activo() -> bool {
    ACTIVO.load(Ordering::Relaxed)
}

/// Cuantas veces ha cambiado: quien pinta guarda la que vio y repinta si
/// ya no es esa.
pub fn version() -> u32 {
    VERSION.load(Ordering::Relaxed)
}

/// **Luna/sol**: lo cambia para todos y lo guarda. Devuelve como queda.
pub fn alternar() -> bool {
    let nuevo = !ACTIVO.fetch_xor(true, Ordering::Relaxed);
    VERSION.fetch_add(1, Ordering::Relaxed);
    if let Some(u) = UBICACION.get()
        && let Err(e) = guardar(u, nuevo)
    {
        // Solo se pierde que se recuerde: el cambio ya se ve.
        tracing::warn!(?e, "no se pudo guardar el modo oscuro de los visores");
    }
    nuevo
}

/// Guarda la preferencia en `estado.toml` sin tocar lo demas que hay.
pub fn guardar(ubicacion: &Ubicacion, oscuro: bool) -> std::io::Result<()> {
    let mut estado = pixpin_store::estado::cargar(ubicacion);
    estado.lectores_oscuros = Some(oscuro);
    pixpin_store::estado::guardar(ubicacion, &estado)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal(etiqueta: &str) -> Ubicacion {
        let dir =
            std::env::temp_dir().join(format!("pixpin-oscuro-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Ubicacion::Instalado { raiz: dir }
    }

    #[test]
    fn sin_tocar_manda_windows_y_lo_guardado_manda_siempre() {
        assert!(decidir(None, false), "Windows en oscuro: visores en oscuro");
        assert!(!decidir(None, true), "Windows en claro: visores en claro");
        // Caso negativo: lo elegido en el boton gana al tema de Windows.
        assert!(!decidir(Some(false), false));
        assert!(decidir(Some(true), true));
    }

    #[test]
    fn la_preferencia_se_guarda_y_no_pisa_lo_demas() {
        let u = temporal("guardar");
        // Algo que ya estaba recordado.
        let antes = pixpin_store::estado::Estado {
            escala_interfaz: Some(125),
            ..Default::default()
        };
        pixpin_store::estado::guardar(&u, &antes).unwrap();
        guardar(&u, true).unwrap();
        let leido = pixpin_store::estado::cargar(&u);
        assert_eq!(leido.lectores_oscuros, Some(true));
        assert_eq!(leido.escala_interfaz, Some(125), "lo demas sigue");
        guardar(&u, false).unwrap();
        assert_eq!(
            pixpin_store::estado::cargar(&u).lectores_oscuros,
            Some(false)
        );
        let _ = std::fs::remove_dir_all(u.raiz());
    }

    #[test]
    fn alternar_cambia_el_valor_y_la_version() {
        // Sin ubicacion puesta (no se guarda nada en las pruebas).
        let (v0, a0) = (version(), activo());
        assert_eq!(alternar(), !a0);
        assert_eq!(activo(), !a0);
        assert!(version() != v0);
        alternar();
        assert_eq!(activo(), a0);
    }
}
