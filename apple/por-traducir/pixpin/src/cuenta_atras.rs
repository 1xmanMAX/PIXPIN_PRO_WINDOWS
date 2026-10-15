//! La cuenta atras de la captura con retardo (P2.1).
//!
//! Sirve para capturar lo que se esconde cuando pierdes el foco: un menu
//! abierto, un desplegable, un globo de ayuda. Se lanza, se cuenta hasta
//! cero, y en ese momento se abre la captura — con el menu todavia
//! desplegado, porque el foco no se ha movido de ahi.
//!
//! El plan pedia la cuenta en el rotulo de la bandeja. No vale: ese rotulo
//! solo se ve pasando el raton por el icono, y ahi es donde precisamente NO
//! esta el raton mientras preparas lo que vas a capturar. Va en un cartel
//! propio, en la esquina de abajo a la derecha y pasante a los clics, para
//! que no estorbe ni salga en lo que se captura.

use std::time::{Duration, Instant};

use pixpin_geom::Rect;
use pixpin_render::{Color, RectF, Superficie};
use pixpin_shell::overlay::VentanaOverlay;
use pixpin_store::Catalogo;

use crate::overlay::Recursos;

/// Tamano del cartel en pixeles logicos.
const ANCHO: u32 = 108;
const ALTO: u32 = 108;
/// Separacion del borde de la pantalla.
const MARGEN: i32 = 24;
/// Cada cuanto se mira si hay que cancelar. Corto para que Escape responda
/// al momento y no al segundo siguiente.
const LATIDO: Duration = Duration::from_millis(16);

const FONDO: Color = Color {
    r: 0.08,
    g: 0.08,
    b: 0.09,
    a: 0.90,
};
const TINTA: Color = Color {
    r: 0.95,
    g: 0.95,
    b: 0.96,
    a: 1.0,
};
const TENUE: Color = Color {
    r: 0.62,
    g: 0.63,
    b: 0.66,
    a: 1.0,
};

/// Los segundos que quedan, para el numero grande del cartel.
///
/// Aparte y pura porque el redondeo tiene truco: con `as_secs()` a secas,
/// una cuenta de tres segundos ensena «2» durante casi todo el primer
/// segundo y no ensena el tres nunca, y nadie cuenta asi. Se redondea hacia
/// arriba, que es lo que hace que empiece en «3» y acabe en «1».
pub fn quedan(total: Duration, transcurrido: Duration) -> u64 {
    let restante = total.saturating_sub(transcurrido);
    if restante.is_zero() {
        return 0;
    }
    restante.as_secs() + u64::from(restante.subsec_nanos() > 0)
}

/// Cuenta hasta cero ensenando el cartel. Devuelve `false` si se cancelo.
///
/// Cancelar es con Escape, como en todo lo demas que ocupa la pantalla.
pub fn esperar(recursos: &Recursos, segundos: u32, textos: &Catalogo) -> bool {
    if segundos == 0 {
        return true;
    }
    let total = Duration::from_secs(segundos as u64);
    let cartel = Cartel::nuevo(recursos);
    let inicio = Instant::now();
    let mut pintado = u64::MAX;

    loop {
        // Sin esto, Windows da la aplicacion por colgada durante la cuenta
        // y el cartel ni siquiera llega a pintarse.
        pixpin_shell::overlay::bombear_pendientes();
        if pixpin_shell::escape_pulsado() {
            tracing::info!("cuenta atras cancelada");
            return false;
        }
        let transcurrido = inicio.elapsed();
        if transcurrido >= total {
            return true;
        }
        let restan = quedan(total, transcurrido);
        if restan != pintado {
            pintado = restan;
            if let Some(c) = &cartel {
                c.pintar(restan, textos);
            }
        }
        std::thread::sleep(LATIDO);
    }
}

/// El cartel con el numero. `None` si no se pudo crear: perder la cuenta
/// visible es malo, pero mucho mejor que no poder capturar con retardo.
struct Cartel {
    ventana: VentanaOverlay,
    superficie: Superficie,
    motor: std::rc::Rc<pixpin_render::MotorRender>,
    escala: f32,
}

impl Cartel {
    fn nuevo(recursos: &Recursos) -> Option<Cartel> {
        let disposicion = pixpin_capture::enumerar_monitores().ok()?;
        let monitor = disposicion.principal()?.to_owned();
        let escala = monitor.escala_por_cien as f32 / 100.0;
        let (ancho, alto) = (
            (ANCHO as f32 * escala) as u32,
            (ALTO as f32 * escala) as u32,
        );
        let margen = (MARGEN as f32 * escala) as i32;
        // Abajo a la derecha: es la esquina donde menos suele haber algo
        // que mirar, y esta lejos de los menus, que salen donde se pulso.
        let marco = Rect {
            x: monitor.area.derecha() - ancho as i32 - margen,
            y: monitor.area.abajo() - alto as i32 - margen,
            ancho,
            alto,
        };
        let ventana = VentanaOverlay::nueva(marco).ok()?;
        let motor = recursos.motor();
        let superficie =
            Superficie::nueva(&motor, &recursos.d3d(), ventana.handle(), ancho, alto).ok()?;
        // Pasante: durante la cuenta el usuario sigue colocando lo que va a
        // capturar, y un cartel que se traga los clics lo estropearia.
        ventana.poner_pasante(true);
        ventana.mostrar();
        Some(Cartel {
            ventana,
            superficie,
            motor,
            escala,
        })
    }

    fn pintar(&self, restan: u64, textos: &Catalogo) {
        let Ok(destino) = self.superficie.empezar(&self.motor) else {
            return;
        };
        let e = self.escala;
        let lado = ANCHO as f32 * e;
        let _ = self.motor.dibujar(&destino, |p| {
            p.limpiar_transparente();
            p.rellenar_redondeado(
                RectF {
                    x: 0.0,
                    y: 0.0,
                    ancho: lado,
                    alto: lado,
                },
                16.0 * e,
                FONDO,
            );
            // El numero, grande y centrado: es lo unico que hay que leer.
            let numero = restan.to_string();
            let tam = 46.0 * e;
            let (w, h) = p.medir_texto(&numero, tam);
            p.texto(
                &numero,
                (lado - w) / 2.0,
                (lado - h) / 2.0 - 8.0 * e,
                tam,
                TINTA,
            );
            // Y debajo, como se para. Pequeno: se lee una vez y ya.
            let pie = textos.t("cuenta-atras-cancelar");
            let tam = 11.0 * e;
            let (w, _) = p.medir_texto(&pie, tam);
            p.texto(&pie, (lado - w) / 2.0, lado - 30.0 * e, tam, TENUE);
        });
        let _ = self.superficie.presentar();
    }
}

impl Drop for Cartel {
    fn drop(&mut self) {
        // Fuera ANTES de capturar: si se quedara en pantalla saldria en la
        // captura, que es justo lo que no puede pasar.
        self.ventana.ocultar();
        pixpin_shell::overlay::bombear_pendientes();
        // Y se espera a que el compositor lo haya presentado: ocultar solo
        // pide el cambio, no lo consuma. Sin esto, la captura puede salir
        // con el cartel todavia dentro (D59).
        pixpin_shell::overlay::esperar_composicion();
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_cuenta_empieza_en_el_total_y_acaba_en_uno() {
        // El fallo del redondeo hacia abajo: con `as_secs()` a secas, una
        // cuenta de tres ensena «2» durante casi todo el primer segundo y
        // no ensena el tres nunca. Nadie cuenta asi.
        let total = Duration::from_secs(3);
        assert_eq!(quedan(total, Duration::ZERO), 3);
        assert_eq!(quedan(total, Duration::from_millis(1)), 3);
        assert_eq!(quedan(total, Duration::from_millis(999)), 3);
        assert_eq!(quedan(total, Duration::from_millis(1000)), 2);
        assert_eq!(quedan(total, Duration::from_millis(2001)), 1);
        assert_eq!(quedan(total, Duration::from_millis(2999)), 1);
    }

    #[test]
    fn al_llegar_no_queda_nada() {
        let total = Duration::from_secs(3);
        assert_eq!(quedan(total, Duration::from_secs(3)), 0);
        // Caso negativo: pasarse de largo no puede dar un numero enorme por
        // haber restado por debajo de cero.
        assert_eq!(quedan(total, Duration::from_secs(9)), 0);
    }

    #[test]
    fn una_cuenta_de_un_segundo_ensena_el_uno() {
        let total = Duration::from_secs(1);
        assert_eq!(quedan(total, Duration::ZERO), 1);
        assert_eq!(quedan(total, Duration::from_millis(500)), 1);
        assert_eq!(quedan(total, Duration::from_millis(1000)), 0);
    }
}
