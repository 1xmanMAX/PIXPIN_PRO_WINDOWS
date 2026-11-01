//! **La version de la Microsoft Store** (solo en la rama `microsoft-store`;
//! ver `docs/tienda.md` y `docs/ramas.md`).
//!
//! Todo lo que la Store obliga a cambiar pasa por aqui, para que unir `main`
//! en `microsoft-store` no choque: el resto del codigo solo pregunta
//! [`activa`] en los pocos sitios donde escribe en Windows.
//!
//! Esta activa si se compilo con `--features tienda` (lo hace
//! `.github/workflows/tienda.yml`) **o**, como red, si el proceso corre
//! dentro del paquete de la Store aunque se compilara sin la feature
//! (`pindesk_shell::tienda::dentro_del_paquete_tienda`). El paquete de
//! IDENTIDAD local (`PinDesk`, editor `CN=PinDesk Local`) no cuenta: ese corre
//! desde la carpeta instalada y sigue escribiendo en el registro.
//!
//! Lo que cambia con ella:
//!
//! - **Arrancar con Windows**: la tarea de inicio del paquete
//!   (`desktop:StartupTask`, `TaskId` `PinDeskInicio`, apagada de fabrica)
//!   en vez de `HKCU\...\Run`. La app no la enciende sola: solo cuando el
//!   usuario pulsa el interruptor de Ajustes.
//! - **Abrir con / tipos de fichero / protocolos / notificaciones**: los
//!   declara el manifiesto (`tienda/AppxManifest.xml`); la app no escribe
//!   nada en `HKCU\Software\Classes` (con la escritura en el registro sin
//!   virtualizar, lo escrito se quedaria al desinstalar).
//! - **Cortafuegos**: la app nunca ejecuta `herramientas/cortafuegos-sincro.ps1`
//!   (tampoco en `main`); en la Store, Ajustes explica que Windows puede
//!   preguntar si PinDesk puede usar la red.
//! - **Actualizarse**: no hay actualizador propio (tampoco en `main`); la
//!   Store actualiza el paquete. El `onnxruntime.dll` va dentro del paquete,
//!   junto al exe, asi que tampoco se baja ninguna DLL.

use pindesk_shell::tienda::{self as paquete, EstadoInicio};
use pindesk_store::ajustes::Ajustes;

/// Si se compilo para la Store.
pub const COMPILADA: bool = cfg!(feature = "tienda");

/// Si se comporta como la version de la Store (ver el modulo).
pub fn activa() -> bool {
    COMPILADA || paquete::dentro_del_paquete_tienda()
}

/// **Paso 5 del arranque en la Store**, en lugar de escribir en el
/// registro: el interruptor de «Arrancar con Windows» refleja la tarea de
/// inicio de verdad (de fabrica apagada, aunque el TOML diga `true`), y no
/// se toca nada mas. Devuelve `true` si se encargo (y `main.rs` no debe
/// hacer lo suyo).
pub fn al_arrancar(config: &mut Ajustes) -> bool {
    if !activa() {
        return false;
    }
    config.arranque_con_windows = match paquete::estado_inicio() {
        Ok(e) => {
            tracing::info!(?e, "tienda: tarea de inicio del paquete");
            e.encendida()
        }
        Err(e) => {
            // Compilada para la Store pero corriendo sin paquete (una
            // prueba a mano): no hay tarea, y el registro no se toca.
            tracing::info!(?e, "tienda: sin tarea de inicio (¿fuera del paquete?)");
            false
        }
    };
    tracing::info!(
        "tienda: «Abrir con», protocolos y notificaciones los da el manifiesto; no se escribe en el registro"
    );
    true
}

/// El interruptor de Ajustes, en la Store: enciende o apaga la tarea de
/// inicio. Si el usuario la apago en Configuracion, la app ya no puede
/// encenderla: se abre esa pagina para que lo haga el.
pub fn fijar_arranque(encender: bool) {
    match paquete::fijar_inicio(encender) {
        Ok(EstadoInicio::ApagadaPorElUsuario) if encender => {
            tracing::info!("tienda: la tarea de inicio la apago el usuario; abro Configuracion");
            if let Err(e) =
                pindesk_shell::abrir::abrir(std::path::Path::new("ms-settings:startupapps"))
            {
                tracing::warn!(?e, "tienda: no se pudo abrir Configuracion › Inicio");
            }
        }
        Ok(e) => tracing::info!(?e, encender, "tienda: tarea de inicio"),
        Err(e) => tracing::warn!(
            ?e,
            encender,
            "tienda: no se pudo cambiar la tarea de inicio"
        ),
    }
}

/// «Hacerla predeterminada» de Ajustes, en la Store: la pagina de
/// Aplicaciones predeterminadas de la app del paquete, sin inscribir nada en
/// el registro. `false` fuera de la Store (y entonces lo hace `main`).
pub fn abrir_predeterminadas() -> bool {
    if !activa() {
        return false;
    }
    let uri = format!(
        "ms-settings:defaultapps?registeredAUMID={}!PinDesk",
        paquete::FAMILIA_TIENDA
    );
    if let Err(e) = pindesk_shell::abrir::abrir(std::path::Path::new(&uri)) {
        tracing::warn!(?e, "tienda: no se pudo abrir Aplicaciones predeterminadas");
    }
    true
}

/// La ayuda de una fila de Ajustes: en la Store, las que hablan del registro
/// o de la red dicen lo que pasa de verdad dentro del paquete.
pub fn ayuda(clave: &'static str) -> &'static str {
    if !activa() {
        return clave;
    }
    ayuda_en_la_tienda(clave)
}

fn ayuda_en_la_tienda(clave: &'static str) -> &'static str {
    match clave {
        "ajustes2-ayuda-arranque" => "tienda-ayuda-arranque",
        "ajustes2-ayuda-abrir-con" => "tienda-ayuda-abrir-con",
        "ajustes2-ayuda-presencia" => "tienda-ayuda-presencia",
        otra => otra,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn en_la_tienda_las_ayudas_del_registro_y_la_red_cambian() {
        assert_eq!(
            ayuda_en_la_tienda("ajustes2-ayuda-arranque"),
            "tienda-ayuda-arranque"
        );
        assert_eq!(
            ayuda_en_la_tienda("ajustes2-ayuda-abrir-con"),
            "tienda-ayuda-abrir-con"
        );
        assert_eq!(
            ayuda_en_la_tienda("ajustes2-ayuda-presencia"),
            "tienda-ayuda-presencia"
        );
        // Las demas, tal cual.
        assert_eq!(
            ayuda_en_la_tienda("ajustes2-ayuda-idioma"),
            "ajustes2-ayuda-idioma"
        );
    }

    #[test]
    fn las_ayudas_de_la_tienda_estan_en_los_dos_idiomas() {
        let es = pindesk_store::Catalogo::nuevo(pindesk_store::Idioma::Espanol);
        let en = pindesk_store::Catalogo::nuevo(pindesk_store::Idioma::Ingles);
        for clave in [
            "tienda-ayuda-arranque",
            "tienda-ayuda-abrir-con",
            "tienda-ayuda-presencia",
        ] {
            assert!(es.traducida(clave), "falta {clave} en es-ES");
            assert!(en.traducida(clave), "falta {clave} en en-US");
        }
    }

    #[test]
    fn fuera_de_la_tienda_no_cambia_nada() {
        // `cargo test` no corre empaquetado: solo cuenta la feature.
        assert_eq!(activa(), COMPILADA);
        let mut a = Ajustes {
            arranque_con_windows: true,
            ..Ajustes::default()
        };
        if !COMPILADA {
            assert!(!al_arrancar(&mut a));
            assert!(a.arranque_con_windows, "sin la Store el ajuste no se toca");
            assert_eq!(ayuda("ajustes2-ayuda-arranque"), "ajustes2-ayuda-arranque");
        } else {
            // Compilada para la Store y sin paquete: no hay tarea, apagado.
            assert!(al_arrancar(&mut a));
            assert!(!a.arranque_con_windows);
        }
    }
}
