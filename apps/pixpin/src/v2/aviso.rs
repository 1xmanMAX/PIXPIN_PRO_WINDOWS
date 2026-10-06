//! **El aviso de abajo** («Copiada», «2 llevadas a la papelera · Deshacer»):
//! el mismo en todas las ventanas v2.
//!
//! POR QUE comun: la galeria lo tenia gris con radio 10 y el enlace en cian
//! suelto; tareas, negro con radio 14. El revisor: «parecen de apps
//! distintas». Este es el de la maqueta: negro casi opaco (se lee sobre
//! cualquier miniatura), radio de flotante, centrado a 18 del borde de abajo
//! y la accion como un boton de verdad, con su chapita de atajo.

#![forbid(unsafe_code)]

use std::time::{Duration, Instant};

use pixpin_render::icono::Icono;
use pixpin_render::icono::material as mi;
use pixpin_render::{Pintor, RectF};

use super::color::{blanco, negro};
use crate::ventanita::Botones;

/// Lo que dura un aviso que solo informa.
pub const DURA: Duration = Duration::from_millis(2_500);
/// Lo que dura uno con accion: hay que leerlo, decidir y llegar al boton.
pub const DURA_CON_ACCION: Duration = Duration::from_secs(6);

/// El boton de un aviso («Deshacer»).
#[derive(Debug, Clone)]
pub struct AccionAviso<A> {
    pub rotulo: String,
    pub icono: &'static Icono,
    /// La chapita de su atajo («Ctrl Z»).
    pub chapa: Option<&'static str>,
    pub accion: A,
}

/// Un aviso en pantalla.
#[derive(Debug, Clone)]
pub struct Aviso<A> {
    pub texto: String,
    pub desde: Instant,
    pub accion: Option<AccionAviso<A>>,
}

impl<A: Copy> Aviso<A> {
    /// Un aviso que solo informa.
    pub fn nuevo(texto: impl Into<String>) -> Self {
        Aviso {
            texto: texto.into(),
            desde: Instant::now(),
            accion: None,
        }
    }

    /// Un aviso con «Deshacer» (icono de deshacer y chapita «Ctrl Z»).
    pub fn con_deshacer(texto: impl Into<String>, rotulo: impl Into<String>, accion: A) -> Self {
        Aviso {
            texto: texto.into(),
            desde: Instant::now(),
            accion: Some(AccionAviso {
                rotulo: rotulo.into(),
                icono: &mi::UNDO,
                chapa: Some("Ctrl Z"),
                accion,
            }),
        }
    }

    /// Lo que dura este aviso.
    pub fn dura(&self) -> Duration {
        if self.accion.is_some() {
            DURA_CON_ACCION
        } else {
            DURA
        }
    }

    /// Si ya se tiene que ir, `pasado` despues de salir. Aparte de
    /// [`caducado`](Self::caducado) para probarlo sin esperar.
    pub fn caducado_tras(&self, pasado: Duration) -> bool {
        pasado > self.dura()
    }

    /// Si ya se tiene que ir.
    pub fn caducado(&self) -> bool {
        self.caducado_tras(self.desde.elapsed())
    }
}

/// Alto de la caja del aviso, en logicos: un boton de 40 y 4 de aire.
const ALTO: f32 = 48.0;

/// **Pinta el aviso** centrado a 18 del borde de abajo de `zona` y apunta su
/// accion. Toda la caja se apunta antes como `A::default()` (el «nada» de
/// cada ventana): un clic que cae en el aviso fuera del boton no debe
/// atravesarlo y elegir la miniatura de debajo. Devuelve la caja pintada.
pub fn pintar<A: Copy + Default>(
    aviso: &Aviso<A>,
    p: &Pintor,
    botones: &mut Botones<A>,
    zona: RectF,
    s: f32,
) -> RectF {
    let tam = super::LETRA_CUERPO * s;
    let (tw, th) = p.medir_texto(&aviso.texto, tam);
    let ancho_boton = aviso.accion.as_ref().map_or(0.0, |a| {
        crate::lecciones::ui::ancho_de_boton(p, true, &a.rotulo, a.chapa, s)
    });
    let pad = super::MARGEN * s;
    let derecha = if aviso.accion.is_some() {
        4.0 * s
    } else {
        pad
    };
    let hueco = if aviso.accion.is_some() {
        super::HUECO * s
    } else {
        0.0
    };
    let alto = ALTO * s;
    let ancho = (pad + tw + hueco + ancho_boton + derecha).min((zona.ancho - 32.0 * s).max(0.0));
    let caja = RectF {
        x: zona.x + (zona.ancho - ancho) / 2.0,
        y: zona.y + zona.alto - 18.0 * s - alto,
        ancho,
        alto,
    };
    // Un borde de blanco al 10 % (el recuadro un poco mayor por debajo): el
    // aviso casi negro sobre el fondo oscuro de la ventana no tendria canto.
    p.rellenar_redondeado(
        super::geom::encoger(caja, -1.0 * s),
        (super::RADIO_FLOTANTE + 1.0) * s,
        blanco(0.10),
    );
    p.rellenar_redondeado(caja, super::RADIO_FLOTANTE * s, negro(0.92));
    botones.zona(caja, A::default());
    let ancho_texto = (ancho - pad - hueco - ancho_boton - derecha).max(0.0);
    p.texto_linea(
        &aviso.texto,
        caja.x + pad,
        caja.y + (alto - th) / 2.0,
        tam,
        ancho_texto,
        super::TEXTO,
    );
    if let Some(a) = &aviso.accion {
        let b = RectF {
            x: caja.x + caja.ancho - derecha - ancho_boton,
            y: caja.y + (alto - super::BOTON * s) / 2.0,
            ancho: ancho_boton,
            alto: super::BOTON * s,
        };
        crate::lecciones::ui::boton_v2(
            p,
            botones,
            b,
            a.accion,
            Some(a.icono),
            &a.rotulo,
            a.chapa,
            None,
            super::ENLACE,
            s,
        );
    }
    caja
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_aviso_normal_se_va_a_los_dos_segundos_y_medio() {
        let a: Aviso<u8> = Aviso::nuevo("Copiada");
        assert!(!a.caducado_tras(Duration::from_millis(2_400)));
        assert!(a.caducado_tras(Duration::from_millis(2_600)));
        // Caso negativo: recien salido no esta caducado.
        assert!(!a.caducado());
    }

    #[test]
    fn con_accion_dura_seis_segundos_para_dar_tiempo_a_pulsarla() {
        let a = Aviso::con_deshacer("Borrada", "Deshacer", 7u8);
        assert_eq!(a.accion.as_ref().map(|x| x.accion), Some(7));
        assert_eq!(a.accion.as_ref().and_then(|x| x.chapa), Some("Ctrl Z"));
        // Caso negativo: a los 3 s uno normal ya se fue; este no.
        assert!(!a.caducado_tras(Duration::from_secs(3)));
        assert!(a.caducado_tras(Duration::from_millis(6_100)));
    }
}
