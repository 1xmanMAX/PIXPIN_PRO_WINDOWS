//! **Volver a pintar cuando la GPU se pierde**, sin reiniciar PixPin.
//!
//! El 4-oct-2026 el registro del usuario dijo `HRESULT(0x887A0005) The GPU
//! device instance has been suspended` y, desde ahi, ningun pin nuevo ni
//! ninguna captura funcionaron hasta reiniciar la app: todo lo creado sobre
//! un dispositivo perdido falla para siempre. Se detecta en un solo sitio
//! (`pixpin_render::perdida`, por donde pasa todo error de pintar o
//! presentar) y aqui se arregla:
//!
//! - **Hilo principal** ([`rehacer_si_se_perdio`]): dispositivo y motor
//!   nuevos (`Recursos`), cada pin rehace su superficie y su bitmap en su
//!   sitio y con su zoom y sus anotaciones, y el icono de la pila renace.
//!   Se mira en cada vuelta del bucle (es una consulta barata) y ademas el
//!   primer fallo despierta al bucle, que si no podria estar dormido.
//! - **Ventanas con su propio hilo y su propio dispositivo** (galeria,
//!   tareas, lecciones, chat...): [`con_recursos`] las abre; si el
//!   dispositivo se pierde, sus ventanas reciben `WM_CLOSE` (se guardan
//!   como al cerrarlas a mano) y se vuelven a abrir sobre uno nuevo.

use std::time::Instant;

use anyhow::Result;
use pixpin_store::Catalogo;
use windows::core::HRESULT;

use crate::overlay::Recursos;
use crate::pila_capturas::PilaCapturas;
use crate::pines::Pines;

/// Cuantas veces seguidas se vuelve a abrir una ventana cuyo dispositivo se
/// perdio. Si la GPU sigue cayendose (un driver roto), mas vueltas solo
/// harian parpadear la ventana: a la tercera se queda cerrada.
const TOPE_REAPERTURAS: u32 = 3;

/// Si hay que rehacer los recursos del hilo principal: solo si SU
/// dispositivo esta perdido. Un aviso de otro hilo (la galeria tiene el
/// suyo) no basta: rehacer todos los pines por eso seria un parpadeo para
/// nada.
pub fn hay_que_rehacer(motivo_del_principal: Option<HRESULT>) -> bool {
    motivo_del_principal.is_some()
}

/// Si una ventana que acaba de cerrarse se vuelve a abrir: solo si se cerro
/// porque su dispositivo se perdio, y no mas de [`TOPE_REAPERTURAS`] veces.
pub fn reabrir_la_ventana(motivo: Option<HRESULT>, reaperturas: u32) -> bool {
    motivo.is_some() && reaperturas < TOPE_REAPERTURAS
}

/// El motivo como se escribe en el registro: nombre y codigo.
pub fn describir(hr: HRESULT) -> String {
    format!(
        "{} (0x{:08X})",
        pixpin_render::perdida::nombre(hr),
        hr.0 as u32
    )
}

/// Instala el aviso: el primer fallo por dispositivo perdido, en el hilo
/// que sea, despierta al bucle principal (`hwnd`), que lo rehace.
pub fn instalar_aviso(hwnd: windows::Win32::Foundation::HWND) {
    // Un HWND no cruza hilos; su valor si, y PostMessage vale desde
    // cualquiera.
    let crudo = hwnd.0 as isize;
    pixpin_render::perdida::al_perder(move || {
        pixpin_shell::despertar(windows::Win32::Foundation::HWND(crudo as *mut _));
    });
}

/// Si el dispositivo del hilo principal se perdio, lo rehace todo encima de
/// uno nuevo. Devuelve si lo hizo.
pub fn rehacer_si_se_perdio(
    recursos: &mut Option<Recursos>,
    pines: &mut Option<Pines>,
    pila: &mut PilaCapturas,
    textos: &Catalogo,
) -> bool {
    // El aviso se recoge siempre: si era de otro hilo, ya esta atendido
    // (ese hilo se rehace solo) y el siguiente fallo volvera a avisar.
    let _ = pixpin_render::perdida::tomar_aviso();
    let motivo = recursos.as_ref().and_then(Recursos::perdido);
    if !hay_que_rehacer(motivo) {
        return false;
    }
    let motivo = motivo.expect("comprobado arriba");
    let t0 = Instant::now();
    // El nuevo ANTES de soltar el viejo: si la GPU aun no esta (el driver a
    // medio instalar), se queda el viejo y se reintenta en la siguiente
    // vuelta, en vez de quedarse sin nada.
    let nuevos = match Recursos::nuevos() {
        Ok(n) => n,
        Err(e) => {
            tracing::error!(
                motivo = %describir(motivo),
                ?e,
                "dispositivo grafico perdido y todavia no se puede crear otro"
            );
            return false;
        }
    };
    let rehechos = pines
        .as_mut()
        .map(|p| p.cambiar_dispositivo(nuevos.dispositivo(), nuevos.motor()))
        .unwrap_or_default();
    pila.cambiar_dispositivo(&nuevos, textos);
    let grafica = nuevos.dispositivo().adaptador();
    *recursos = Some(nuevos);
    let ms = t0.elapsed().as_millis() as u64;
    tracing::warn!(
        motivo = %describir(motivo),
        %grafica,
        pines = rehechos.pines,
        fallidos = rehechos.fallidos,
        ms,
        "dispositivo grafico perdido; recreado en {ms} ms"
    );
    true
}

/// Abre una ventana de hilo propio con sus recursos (`abrir` es su bucle) y,
/// si se cierra porque el dispositivo se perdio, la vuelve a abrir sobre uno
/// nuevo. `que` sale en el registro.
pub fn con_recursos(que: &str, mut abrir: impl FnMut(&Recursos) -> Result<()>) -> Result<()> {
    let mut reaperturas = 0;
    loop {
        // Que un dispositivo perdido cierre las ventanas de este hilo: es lo
        // que hace volver a `abrir`. Se rearma en cada vuelta.
        pixpin_render::perdida::cerrar_ventanas_al_perder(true);
        let recursos = Recursos::nuevos()?;
        let hecho = abrir(&recursos);
        let motivo = recursos.perdido();
        if !reabrir_la_ventana(motivo, reaperturas) {
            pixpin_render::perdida::cerrar_ventanas_al_perder(false);
            if let Some(m) = motivo {
                tracing::warn!(que, motivo = %describir(m), "dispositivo grafico perdido otra vez; la ventana se queda cerrada");
            }
            return hecho;
        }
        reaperturas += 1;
        tracing::warn!(
            que,
            motivo = %describir(motivo.expect("comprobado arriba")),
            reaperturas,
            "dispositivo grafico perdido; la ventana se vuelve a abrir"
        );
        drop(recursos);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_render::perdida::{DEVICE_HUNG, DEVICE_REMOVED};

    #[test]
    fn se_rehace_solo_si_el_dispositivo_del_hilo_principal_esta_perdido() {
        assert!(hay_que_rehacer(Some(DEVICE_REMOVED)));
        assert!(hay_que_rehacer(Some(DEVICE_HUNG)));
    }

    #[test]
    fn caso_negativo_con_el_dispositivo_sano_no_se_rehace_nada() {
        // Aunque otro hilo haya avisado de una perdida en el suyo.
        assert!(!hay_que_rehacer(None));
    }

    #[test]
    fn una_ventana_se_reabre_tras_una_perdida_y_hasta_tres_veces() {
        assert!(reabrir_la_ventana(Some(DEVICE_REMOVED), 0));
        assert!(reabrir_la_ventana(Some(DEVICE_REMOVED), 2));
        assert!(!reabrir_la_ventana(Some(DEVICE_REMOVED), 3), "tope");
    }

    #[test]
    fn caso_negativo_cerrar_a_mano_no_reabre_la_ventana() {
        assert!(!reabrir_la_ventana(None, 0));
    }

    /// De punta a punta, como en el bucle principal: un pin abierto, el
    /// dispositivo de los recursos se «pierde» (fallo inyectado), el pin deja
    /// de pintar, y `rehacer_si_se_perdio` lo deja pintando sobre uno nuevo.
    #[test]
    #[ignore = "abre un pin de verdad en el escritorio; necesita GPU"]
    fn el_bucle_principal_rehace_los_pines_tras_perder_la_gpu() {
        use pixpin_render::perdida;
        let dir = std::env::temp_dir().join("pixpin-perdida-gpu");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ubicacion = pixpin_store::rutas::resolver(&dir, &dir.join("appdata"));
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let hwnd = windows::Win32::Foundation::HWND::default();
        let mut recursos = Some(Recursos::nuevos().unwrap());
        let r = recursos.as_ref().unwrap();
        let mut pines = Some(
            Pines::nuevos(
                ubicacion.raiz(),
                r.d3d(),
                r.motor(),
                String::new(),
                crate::textos_del_pin(&textos),
                String::new(),
                String::new(),
                hwnd,
                Some(16),
            )
            .unwrap(),
        );
        let mut pila = PilaCapturas::nueva(&pixpin_store::ajustes::Capturas::default(), hwnd);
        let monitores = pixpin_capture::enumerar_monitores().unwrap();
        let monitor = monitores.principal().unwrap().to_owned();
        let imagen = pixpin_codec::ImagenRgba {
            ancho: 320,
            alto: 200,
            pixeles: [30u8, 160, 90, 255].repeat(320 * 200),
        };
        let p = pines.as_mut().unwrap();
        p.pinear_imagen_centrada(&imagen, &monitor).unwrap();
        assert!(p.todos_pintan_bien());

        // Caso negativo: con la GPU sana no se rehace nada.
        assert!(!rehacer_si_se_perdio(
            &mut recursos,
            &mut pines,
            &mut pila,
            &textos
        ));

        let viejo = recursos.as_ref().unwrap().d3d();
        perdida::inyectar_perdida(&viejo, Some(perdida::DEVICE_REMOVED));
        pines.as_ref().unwrap().repintar_todos();
        assert!(
            !pines.as_ref().unwrap().todos_pintan_bien(),
            "sin GPU no pinta"
        );
        assert!(perdida::tomar_aviso(), "el fallo avisa al bucle");
        // `tomar_aviso` lo recogio; rehacer mira el dispositivo, no el aviso.
        assert!(rehacer_si_se_perdio(
            &mut recursos,
            &mut pines,
            &mut pila,
            &textos
        ));
        perdida::inyectar_perdida(&viejo, None);
        assert!(
            pines.as_ref().unwrap().todos_pintan_bien(),
            "el pin vuelve a pintar sobre el dispositivo nuevo"
        );
        assert!(recursos.as_ref().unwrap().perdido().is_none());
        drop(pines);
    }

    #[test]
    fn el_motivo_sale_con_su_nombre_y_su_codigo() {
        assert_eq!(describir(DEVICE_REMOVED), "DEVICE_REMOVED (0x887A0005)");
        assert_eq!(
            describir(windows::Win32::Foundation::E_FAIL),
            "otro (0x80004005)"
        );
    }
}
