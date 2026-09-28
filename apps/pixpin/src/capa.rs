//! **Anotar sobre la pantalla**: la entrada de los comandos `anotar` y
//! `anotar-congelada` (bandeja, o el atajo si el usuario se pone uno en el
//! TOML: de fabrica no tienen, D81).
//!
//! Hasta el 2026-09-24 aqui vivia una capa propia —`CapaViva`, sobre el
//! `Anotador` viejo de `pixpin-ui`, un monitor, once herramientas, sin capa
//! de tinta ni panel—. El usuario pidio que las herramientas de dibujo
//! fueran las mismas en todas partes y un anotador de pantalla de verdad,
//! asi que ahora **el anotador es el editor** con el anfitrion `Pantalla`
//! (ver `ventana_editor/pantalla.rs`): todos los monitores, viva o
//! congelada, las herramientas del lienzo (menos las apagadas en los
//! ajustes), su capa de tinta de baja latencia y su panel.
//!
//! Lo que queda aqui es lo que el editor no puede hacer solo: fotografiar la
//! pantalla. Antes de abrir (la foto de fondo de la congelada, sin la
//! ventana) y al salir (lo anotado sobre lo que habia debajo, que `main`
//! ofrece guardar y pinea, D54). Las dos van por los duplicadores de
//! `Recursos`, un monitor cada uno, y se unen en una del escritorio virtual.
//!
//! **Lo que se pierde respecto a la capa vieja**: alternar el pasante con el
//! mismo atajo global que abrio la capa (el bucle del editor solo ve los
//! eventos de su ventana). Espacio y Ctrl siguen haciendolo. La lupa en vivo
//! ya no se pierde: se porto el 2026-09-26 (`pantalla::LupaViva`).

use anyhow::{Context, Result};
use pixpin_geom::{Monitor, Rect};
use pixpin_nivel::Nivel;

/// Los dos modos que pidio el usuario (D49).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoCapa {
    /// Transparente: la pantalla sigue viva debajo.
    Viva,
    /// Con la captura del escritorio como fondo (D56). Sin modo pasante: no
    /// hay nada vivo debajo a lo que dejar pasar los clics.
    Congelada,
}

impl ModoCapa {
    fn a_modo(self) -> crate::ventana_editor::pantalla::Modo {
        match self {
            ModoCapa::Viva => crate::ventana_editor::pantalla::Modo::Viva,
            ModoCapa::Congelada => crate::ventana_editor::pantalla::Modo::Congelada,
        }
    }
}

/// Abre el anotador de pantalla y no vuelve hasta que se sale. Devuelve la
/// foto de lo anotado si se dibujo algo.
///
/// La foto final se hace **de la pantalla con lo anotado encima**: es lo
/// que el usuario ve, y es lo que espera que se quede como pin.
pub fn ejecutar_capa(
    recursos: &mut crate::overlay::Recursos,
    modo: ModoCapa,
    nivel: Nivel,
    enganche: pixpin_motor2d::enganche::Ajustes,
) -> Result<Option<pixpin_codec::ImagenRgba>> {
    let t0 = std::time::Instant::now();
    let disposicion =
        pixpin_capture::enumerar_monitores().context("no se pudieron enumerar los monitores")?;
    let principal = *disposicion.principal().context("sin monitor principal")?;
    let monitores: Vec<Monitor> = disposicion.monitores().to_vec();
    let escritorio = disposicion.escritorio_virtual();

    // Congelar ANTES de crear la ventana: asi la foto no lleva el anotador.
    let foto = match modo {
        ModoCapa::Viva => None,
        ModoCapa::Congelada => Some(
            foto_del_escritorio(recursos, &monitores, escritorio)
                .context("no se pudo congelar la pantalla")?,
        ),
    };
    let mut capturar = || match foto_del_escritorio(recursos, &monitores, escritorio) {
        Ok(img) => Some(img),
        Err(e) => {
            tracing::warn!(?e, "no se pudo fotografiar la pantalla anotada");
            None
        }
    };
    let mut pantalla = crate::ventana_editor::pantalla::Pantalla {
        modo: modo.a_modo(),
        escritorio,
        principal,
        foto,
        capturar: &mut capturar,
        resultado: None,
    };
    tracing::info!(
        ?modo,
        monitores = monitores.len(),
        ms = t0.elapsed().as_millis() as u64,
        "anotador de pantalla abierto"
    );
    crate::ventana_editor::abrir_pantalla(enganche, nivel, &mut pantalla)
}

/// El escritorio virtual entero AHORA: un monitor por duplicador, unidos.
fn foto_del_escritorio(
    recursos: &mut crate::overlay::Recursos,
    monitores: &[Monitor],
    escritorio: Rect,
) -> Result<pixpin_codec::ImagenRgba> {
    let mut fotos = Vec::with_capacity(monitores.len());
    for m in monitores {
        let instantanea = recursos
            .congelar_monitor(m)
            .with_context(|| format!("no se pudo capturar el monitor {}", m.id))?;
        let img = pixpin_capture::a_imagen(recursos.dispositivo(), &instantanea)
            .context("no se pudo bajar la captura a memoria")?;
        fotos.push((m.area, img));
    }
    Ok(crate::ventana_editor::pantalla::unir_fotos(escritorio, &fotos))
}
