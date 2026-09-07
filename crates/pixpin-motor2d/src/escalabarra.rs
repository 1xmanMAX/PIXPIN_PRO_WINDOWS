//! La reglita a cuadros de los planos.
//!
//! Puerto de `EscalaGrafica.kt`. Puro.
//!
//! # Por que una barra y no un numero
//!
//! Un numero —«escala 1:50»— solo vale mientras nadie toque el papel. En
//! cuanto el plano se fotocopia al 80 %, se manda por un chat o se recorta,
//! el numero miente y nadie se entera. La barra a cuadros no puede mentir: se
//! encoge y se estira **con el dibujo**, asi que quien reciba la imagen puede
//! medir sobre ella con una regla y acertar.
//!
//! # Por que el paso es redondo
//!
//! Dado lo ancha que es la barra y cuanto vale un pixel, hay que elegir
//! cuanto mide cada cuadro. No vale repartir el ancho a partes iguales:
//! saldrian cuadros de «3,7 m» y una barra que hay que leer con calculadora.
//! Se elige un paso redondo —1, 2 o 5 por una potencia de diez— y se ponen
//! los cuadros que quepan. Es lo que hace un plano de verdad.

use crate::medida::Escala;

/// Como queda repartida una barra.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Barra {
    /// Cuanto mide cada cuadro, en unidades de mundo.
    pub paso: f32,
    /// Cuantos cuadros caben.
    pub cuadros: u32,
    /// Lo que ocupa de verdad. Nunca mas de lo pedido: la barra se dibuja
    /// dentro de su caja, y pasarse la sacaria del elemento y del recorte.
    pub ancho_usado_px: f32,
}

/// El mayor 1, 2 o 5 por una potencia de diez que no pase de `valor`.
///
/// Un valor absurdo —cero, negativo, NaN, infinito— devuelve 1, que es un
/// paso valido y evita propagar el disparate al reparto.
pub fn paso_redondo(valor: f32) -> f32 {
    if !valor.is_finite() || valor <= 0.0 {
        return 1.0;
    }
    let exp = valor.log10().floor();
    let base = 10f32.powf(exp);
    // De mayor a menor: se coge el primero que quepa.
    for m in [5.0, 2.0, 1.0] {
        let paso = m * base;
        if paso <= valor {
            return paso;
        }
    }
    // Solo se llega aqui por redondeo del logaritmo; una decada menos cabe
    // siempre.
    base / 2.0
}

/// Como repartir una barra de `ancho_px` con esa escala.
///
/// `None` cuando no se puede: sin escala valida la barra no podria decir
/// cuanto mide cada cuadro, y una barra que no lo dice es un adorno que
/// engana.
pub fn repartir(ancho_px: f32, escala: &Escala, cuadros_deseados: u32) -> Option<Barra> {
    if !escala.valida() || !ancho_px.is_finite() || ancho_px <= f32::EPSILON {
        return None;
    }
    let deseados = cuadros_deseados.max(1);
    let total_mundo = ancho_px * escala.unidades_por_pixel;
    let paso = paso_redondo(total_mundo / deseados as f32);
    if paso <= 0.0 || !paso.is_finite() {
        return None;
    }
    // Los que quepan enteros, y al menos uno: una barra sin cuadros no es
    // una barra.
    let cuadros = ((total_mundo / paso).floor() as u32).max(1);
    let ancho_usado_px = (cuadros as f32 * paso) / escala.unidades_por_pixel;
    Some(Barra {
        paso,
        cuadros,
        ancho_usado_px,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn metros(por_pixel: f32) -> Escala {
        Escala {
            unidades_por_pixel: por_pixel,
            unidad: "m".to_string(),
            decimales: 2,
        }
    }

    #[test]
    fn el_paso_siempre_es_uno_dos_o_cinco_por_una_potencia_de_diez() {
        // El motivo entero de esta tarea: repartir el ancho a partes iguales
        // daria cuadros de «3,7 m» y una barra que hay que leer con
        // calculadora.
        assert_eq!(paso_redondo(1.0), 1.0);
        assert_eq!(paso_redondo(3.7), 2.0);
        assert_eq!(paso_redondo(7.0), 5.0);
        assert_eq!(paso_redondo(12.0), 10.0);
        assert_eq!(paso_redondo(99.0), 50.0);
        assert!(
            (paso_redondo(0.37) - 0.2).abs() < 1e-6,
            "tambien por debajo de uno"
        );
        assert!((paso_redondo(0.07) - 0.05).abs() < 1e-6);
    }

    #[test]
    fn el_paso_de_un_valor_absurdo_no_revienta() {
        assert!(paso_redondo(0.0).is_finite());
        assert!(paso_redondo(-5.0).is_finite());
        assert!(paso_redondo(f32::NAN).is_finite());
        assert!(paso_redondo(f32::INFINITY).is_finite());
    }

    #[test]
    fn una_barra_de_cuatro_metros_con_cuatro_cuadros_da_pasos_de_un_metro() {
        // 400 px a 0,01 m/px son 4 m. Cuatro cuadros piden 1 m cada uno, que
        // ya es redondo.
        let b = repartir(400.0, &metros(0.01), 4).expect("reparto valido");
        assert!((b.paso - 1.0).abs() < 1e-5);
        assert_eq!(b.cuadros, 4);
        assert!((b.ancho_usado_px - 400.0).abs() < 1e-3);
    }

    #[test]
    fn el_ancho_usado_nunca_pasa_del_pedido() {
        // La barra se dibuja dentro de su caja: si se pasara, se saldria del
        // elemento y del recorte.
        for ancho in [37.0, 123.0, 456.0, 999.0] {
            for upp in [0.001, 0.01, 0.5, 3.0] {
                let b = repartir(ancho, &metros(upp), 4).expect("reparto valido");
                assert!(
                    b.ancho_usado_px <= ancho + 1e-3,
                    "{ancho} px a {upp} m/px se paso: {}",
                    b.ancho_usado_px
                );
            }
        }
    }

    #[test]
    fn los_cuadros_miden_todos_lo_mismo() {
        let b = repartir(500.0, &metros(0.02), 5).expect("reparto valido");
        let por_cuadro = b.ancho_usado_px / b.cuadros as f32;
        let en_mundo = por_cuadro * 0.02;
        assert!((en_mundo - b.paso).abs() < 1e-4, "cada cuadro mide el paso");
    }

    #[test]
    fn siempre_cabe_al_menos_un_cuadro() {
        // Una barra sin cuadros no es una barra. Si el ancho es ridiculo, se
        // pone uno y se acepta que sea pequeno.
        let b = repartir(10.0, &metros(0.01), 4).expect("reparto valido");
        assert!(b.cuadros >= 1);
    }

    #[test]
    fn sin_escala_valida_no_hay_barra() {
        // Una barra sin escala no puede decir cuanto mide cada cuadro, y una
        // barra que no dice cuanto mide es un adorno que engana.
        let mala = Escala {
            unidades_por_pixel: 0.0,
            unidad: "m".to_string(),
            decimales: 2,
        };
        assert!(repartir(400.0, &mala, 4).is_none());
    }

    #[test]
    fn un_ancho_imposible_no_da_barra() {
        assert!(repartir(0.0, &metros(0.01), 4).is_none());
        assert!(repartir(-100.0, &metros(0.01), 4).is_none());
        assert!(repartir(f32::NAN, &metros(0.01), 4).is_none());
    }

    #[test]
    fn pedir_cero_cuadros_no_divide_por_cero() {
        let b = repartir(400.0, &metros(0.01), 0);
        assert!(b.is_none() || b.unwrap().cuadros >= 1);
    }
}
