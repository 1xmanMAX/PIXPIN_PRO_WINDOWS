//! **La pasada de tapado del editor**: donde el mosaico deja de ser una
//! figura y pasa a ser pixeles.
//!
//! Es deliberadamente corto. Toda la geometria —que caja, que cuadro, en que
//! orden— la decide `pixpin_motor2d::mosaico`, que es puro y se prueba sin
//! GPU; y todo el Direct2D lo hace `pixpin_render::capa_estatica::tapar`.
//! Aqui solo se juntan los dos, que es lo unico que no puede vivir en
//! ninguno de ellos.
//!
//! # Como se engancha, y por que asi
//!
//! El mosaico tapa **lo que hay debajo**, y debajo es lo que se pinto antes
//! que el. Asi que no vale con pasar al final y tapar: una flecha dibujada
//! despues, senalando el dato tapado, acabaria pixelada tambien.
//!
//! El orden del fotograma queda asi:
//!
//! 1. Se pinta la escena **saltandose los mosaicos** ([`es_mosaico`]).
//! 2. Se cierra el fotograma, porque leer lo ya pintado necesita que el
//!    destino este escrito de verdad.
//! 3. Se llama a [`pasar`] una vez, que tapa cada mosaico con lo que hay
//!    debajo de el.
//!
//! El paso 1 es el que hay que recordar: si el mosaico se pintara ademas con
//! su banda opaca de `pintado.rs`, lo que la pasada leeria seria la banda, y
//! el resultado seria un rectangulo gris uniforme —lo de antes, con mas
//! trabajo—.

use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::mosaico;
use pixpin_render::MotorRender;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

/// Si este elemento lo pinta la pasada de tapado y no el camino normal.
pub fn es_mosaico(e: &Elemento) -> bool {
    matches!(e.figura, Figura::Mosaico { .. })
}

/// **Tapa todos los mosaicos que se ven.**
///
/// Devuelve cuantos tapo, que es lo que una prueba de humo puede mirar sin
/// GPU. Un fallo de uno no detiene a los demas: que un mosaico no llegue a
/// taparse es malo, pero que por su culpa no se tapen los otros es peor.
pub fn pasar(
    motor: &mut MotorRender,
    destino: &ID2D1Bitmap1,
    elementos: &[Elemento],
    camara: &Camara,
    ancho_px: u32,
    alto_px: u32,
) -> usize {
    let mut tapados = 0;
    for m in mosaico::plan_en_pantalla(elementos, camara, ancho_px, alto_px) {
        match pixpin_render::capa_estatica::tapar(motor, destino, m.zona, m.lado, m.desenfoque) {
            Ok(()) => tapados += 1,
            Err(e) => tracing::warn!(?e, id = m.id, "no se pudo tapar un mosaico"),
        }
    }
    tapados
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn mosaico_en(x: f32) -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Mosaico { desenfoque: false },
            x,
            y: 0.0,
            ancho: 40.0,
            alto: 20.0,
            grosor: 2.0,
            ..Default::default()
        }
    }

    #[test]
    fn solo_los_mosaicos_se_saltan_del_pintado_normal() {
        assert!(es_mosaico(&mosaico_en(0.0)));
        assert!(!es_mosaico(&Elemento::default()), "un rectangulo se pinta");
    }

    /// El caso normal y el que mas veces ocurre: ni un mosaico. La pasada no
    /// tiene entonces nada que hacer, y sobre todo no puede inventarse una
    /// zona: tapar de mas es tapar el dibujo.
    #[test]
    fn sin_mosaicos_a_la_vista_la_pasada_no_tiene_nada_que_hacer() {
        let c = Camara::nueva();
        let vacio = |es: &[Elemento]| mosaico::plan_en_pantalla(es, &c, 800, 600).is_empty();
        assert!(vacio(&[]));
        assert!(vacio(&[Elemento::default()]), "un rectangulo no es mosaico");
        assert!(
            vacio(&[mosaico_en(10_000.0)]),
            "uno fuera de la pantalla tampoco cuenta"
        );
        assert!(!vacio(&[mosaico_en(10.0)]), "uno a la vista si");
    }
}
