//! Mover, escalar y girar. Puerto de `Transform.kt` del Android.
//!
//! # El problema del ancla
//!
//! Redimensionar tirando de una esquina significa que **la de enfrente no
//! se mueve**. Sin giro es aritmetica de colegio. Con el elemento girado,
//! la forma evidente esta mal:
//!
//! > Cambio `ancho` y `alto` -> el centro se desplaza -> el elemento se
//! > dibuja girado **alrededor de un centro nuevo** -> la esquina anclada
//! > se va.
//!
//! El usuario lo ve como que la figura «se escapa» al redimensionarla.
//!
//! # La formula
//!
//! Un elemento se dibuja girado `ang` alrededor de su centro, asi que un
//! punto del mundo `q` es el punto local `q.girar(centro, -ang)`. Escalar
//! es multiplicar el desplazamiento respecto al ancla y devolverlo:
//!
//! ```text
//! nuevo(q) = ancla_mundo + R(ang) . ( (q_local - ancla_local) . (sx, sy) )
//! ```
//!
//! Si `q` es el ancla, el desplazamiento es cero y el resultado es el ancla:
//! **queda quieta por construccion, no por cuidado**.
//!
//! # La regla de los puntos
//!
//! `Elemento::caja()` calcula la caja **de los puntos** para lapiz,
//! resaltador, linea y flecha. Por eso: toda transformacion que toque una
//! figura con puntos, toca los puntos. Si solo se cambiara `ancho`, el
//! trazo no escalaria y el marco se despegaria del dibujo. Es el mismo
//! motivo por el que `Elemento::mover` ya mueve los puntos.

use pixpin_geom::Tirador;

use crate::elemento::{Elemento, Figura};
use crate::vector::Punto2;

/// Por debajo de esto no se aplasta: se voltea (D30).
pub const MINIMO: f32 = 1.0;

/// Quince grados, el salto de giro con `Shift`.
pub const SALTO_GIRO: f32 = std::f32::consts::FRAC_PI_2 / 6.0;

/// El centro de la caja de un elemento.
fn centro_de(e: &Elemento) -> Punto2 {
    let (x0, y0, x1, y1) = e.caja();
    Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0)
}

/// Que ejes mueve cada tirador, y donde queda su ancla dentro de la caja
/// en tantos por uno: `(0,0)` es la esquina noroeste, `(1,1)` la sureste.
fn ancla_y_ejes(t: Tirador) -> (f32, f32, bool, bool) {
    match t {
        Tirador::NoroesteEsquina => (1.0, 1.0, true, true),
        Tirador::NorteBorde => (0.5, 1.0, false, true),
        Tirador::NoresteEsquina => (0.0, 1.0, true, true),
        Tirador::EsteBorde => (0.0, 0.5, true, false),
        Tirador::SuresteEsquina => (0.0, 0.0, true, true),
        Tirador::SurBorde => (0.5, 0.0, false, true),
        Tirador::SuroesteEsquina => (1.0, 0.0, true, true),
        Tirador::OesteBorde => (1.0, 0.5, true, false),
    }
}

/// Escala `e` arrastrando `tirador` hasta el punto `p` del mundo.
///
/// - `proporcional` (`Shift`): conserva la razon entre ancho y alto.
/// - `desde_centro` (`Alt`): el ancla pasa a ser el centro.
pub fn escalar(
    e: &mut Elemento,
    tirador: Tirador,
    p: Punto2,
    proporcional: bool,
    desde_centro: bool,
) {
    let (x0, y0, x1, y1) = e.caja();
    let (ancho, alto) = (x1 - x0, y1 - y0);
    // Una caja degenerada no se puede escalar: dividir por cero daria NaN,
    // y un NaN en la geometria borra el elemento de la pantalla sin error.
    if ancho.abs() < f32::EPSILON || alto.abs() < f32::EPSILON {
        return;
    }
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let ang = e.angulo;

    let (ax, ay, mueve_x, mueve_y) = ancla_y_ejes(tirador);
    let ancla_local = if desde_centro {
        centro
    } else {
        Punto2::nuevo(x0 + ancho * ax, y0 + alto * ay)
    };

    // El cursor, en el marco propio del elemento.
    let p_local = p.girar(centro, -ang);

    // Cuanto se estira cada eje. El tirador que no mueve un eje lo deja a 1.
    //
    // Dos sutilezas. Con `desde_centro`, la referencia es media caja y no la
    // caja entera: el cursor se aleja del centro, no del borde de enfrente.
    // Y el `signo` existe porque el ancla puede estar a la derecha o abajo
    // (tiradores del oeste y del norte): ahi, alejarse del ancla es ir hacia
    // los negativos.
    let mut sx = if mueve_x {
        let ancla_a_borde = if desde_centro { ancho / 2.0 } else { ancho };
        let signo = if ax > 0.5 { -1.0 } else { 1.0 };
        (p_local.x - ancla_local.x) * signo / ancla_a_borde
    } else {
        1.0
    };
    let mut sy = if mueve_y {
        let ancla_a_borde = if desde_centro { alto / 2.0 } else { alto };
        let signo = if ay > 0.5 { -1.0 } else { 1.0 };
        (p_local.y - ancla_local.y) * signo / ancla_a_borde
    } else {
        1.0
    };

    if proporcional && mueve_x && mueve_y {
        // El que mas se ha movido manda, para que la figura siga al cursor
        // por el eje en que el usuario esta tirando de verdad.
        let k = if sx.abs() > sy.abs() {
            sx.abs()
        } else {
            sy.abs()
        };
        sx = k * sx.signum();
        sy = k * sy.signum();
    }

    // No se aplasta: se voltea (D30). El minimo se aplica al tamano final,
    // conservando el signo, que es lo que produce el volteo.
    let tope = |s: f32, largo: f32| -> f32 {
        if (s * largo).abs() < MINIMO {
            MINIMO / largo * if s < 0.0 { -1.0 } else { 1.0 }
        } else {
            s
        }
    };
    let sx = tope(sx, ancho);
    let sy = tope(sy, alto);

    let ancla_mundo = ancla_local.girar(centro, ang);
    let origen = Punto2::nuevo(0.0, 0.0);

    // La formula del encabezado, en una sola funcion.
    let mapear = |q: Punto2| -> Punto2 {
        let ql = q.girar(centro, -ang);
        let d = Punto2::nuevo((ql.x - ancla_local.x) * sx, (ql.y - ancla_local.y) * sy);
        ancla_mundo.sumar(d.girar(origen, ang))
    };

    // Las figuras con puntos escalan sus puntos: su caja sale de ellos.
    match &mut e.figura {
        Figura::Lapiz { puntos, .. }
        | Figura::Resaltador { puntos }
        | Figura::Linea { puntos }
        | Figura::Flecha { puntos, .. } => {
            for q in puntos.iter_mut() {
                *q = mapear(*q);
            }
        }
        _ => {}
    }

    // Y todas, incluidas esas, actualizan su caja: `x`/`y` se usan para las
    // figuras sin puntos, y para las que los tienen es informacion
    // coherente que no debe quedarse vieja.
    let centro_nuevo = mapear(centro);
    e.ancho = (ancho * sx).abs();
    e.alto = (alto * sy).abs();
    e.x = centro_nuevo.x - e.ancho / 2.0;
    e.y = centro_nuevo.y - e.alto / 2.0;
    e.tocar();
}

/// Gira `e` en `delta` radianes alrededor de `centro`.
///
/// Si `centro` es el suyo, gira sobre si mismo. Si es ajeno -el centro de
/// una seleccion de varios- ademas orbita: es lo que hace que girar cinco
/// elementos a la vez se vea como girar el conjunto.
pub fn girar(e: &mut Elemento, centro: Punto2, delta: f32) {
    let propio = centro_de(e);

    match &mut e.figura {
        Figura::Lapiz { puntos, .. }
        | Figura::Resaltador { puntos }
        | Figura::Linea { puntos }
        | Figura::Flecha { puntos, .. } => {
            for q in puntos.iter_mut() {
                *q = q.girar(centro, delta);
            }
        }
        _ => {}
    }

    let nuevo = propio.girar(centro, delta);
    e.x += nuevo.x - propio.x;
    e.y += nuevo.y - propio.y;
    e.angulo += delta;
    e.tocar();
}

/// El angulo del centro al punto, con **cero apuntando hacia arriba**.
///
/// Hacia arriba y no hacia la derecha porque el tirador de giro esta encima
/// del elemento: con el cursor ahi mismo, el giro tiene que ser cero.
pub fn angulo_hacia(centro: Punto2, p: Punto2) -> f32 {
    let d = p.restar(centro);
    d.x.atan2(-d.y)
}

/// Redondea a saltos de quince grados. Es lo que hace `Shift` al girar.
pub fn a_saltos(angulo: f32) -> f32 {
    (angulo / SALTO_GIRO).round() * SALTO_GIRO
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_6, PI};

    /// Un rectangulo de 100x50 con la esquina en el origen.
    fn rect() -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Rectangulo,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 50.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
        }
    }

    /// La esquina superior izquierda, ya en el mundo (girada).
    fn esquina_no(e: &Elemento) -> Punto2 {
        let (x0, y0, x1, y1) = e.caja();
        let c = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        Punto2::nuevo(x0, y0).girar(c, e.angulo)
    }

    fn cerca(a: Punto2, b: Punto2, que: &str) {
        assert!(
            (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3,
            "{que}: esperaba ({}, {}), es ({}, {})",
            b.x,
            b.y,
            a.x,
            a.y
        );
    }

    #[test]
    fn escalar_por_una_esquina_deja_quieta_la_de_enfrente() {
        let mut e = rect();
        let ancla = esquina_no(&e);

        escalar(
            &mut e,
            Tirador::SuresteEsquina,
            Punto2::nuevo(200.0, 100.0),
            false,
            false,
        );

        assert_eq!(e.ancho, 200.0);
        assert_eq!(e.alto, 100.0);
        cerca(esquina_no(&e), ancla, "la esquina anclada");
    }

    /// El punto del mundo al que hay que arrastrar para hacer, en un
    /// elemento girado `angulo`, **el mismo gesto** que `local` haria en uno
    /// sin girar.
    ///
    /// Sin esto, las pruebas de abajo arrastrarian al mismo punto del mundo
    /// para todos los angulos, que no es el mismo gesto: a 90 grados ese
    /// punto cae al otro lado del ancla y lo que sale es un volteo, no un
    /// escalado. Y tras un volteo la esquina que se queda quieta ya no es la
    /// noroeste, asi que la comprobacion dejaria de tener sentido.
    fn arrastre_equivalente(local: Punto2, centro: Punto2, angulo: f32) -> Punto2 {
        local.girar(centro, angulo)
    }

    #[test]
    fn con_el_elemento_girado_la_esquina_anclada_sigue_sin_moverse() {
        // ESTA es la prueba que justifica la tarea entera. La forma
        // evidente —cambiar ancho/alto y volver a girar— la falla, porque
        // el centro se desplaza y el elemento acaba girado alrededor de un
        // centro nuevo.
        for angulo in [0.0, FRAC_PI_6, FRAC_PI_2, PI, 2.6] {
            let mut e = rect();
            e.angulo = angulo;
            let centro = Punto2::nuevo(50.0, 25.0);
            let ancla = esquina_no(&e);
            let destino = arrastre_equivalente(Punto2::nuevo(180.0, 90.0), centro, angulo);

            escalar(&mut e, Tirador::SuresteEsquina, destino, false, false);

            cerca(esquina_no(&e), ancla, &format!("a {angulo} radianes"));
            assert!(
                (e.ancho - 180.0).abs() < 1e-2,
                "ancho a {angulo}: {}",
                e.ancho
            );
            assert!((e.alto - 90.0).abs() < 1e-2, "alto a {angulo}: {}", e.alto);
        }
    }

    #[test]
    fn escalar_y_devolver_deja_el_elemento_donde_estaba() {
        for angulo in [0.0, FRAC_PI_6, FRAC_PI_2, PI] {
            let mut e = rect();
            e.angulo = angulo;
            let centro = Punto2::nuevo(50.0, 25.0);
            let se_original = arrastre_equivalente(Punto2::nuevo(100.0, 50.0), centro, angulo);
            let lejos = arrastre_equivalente(Punto2::nuevo(300.0, 150.0), centro, angulo);

            escalar(&mut e, Tirador::SuresteEsquina, lejos, false, false);
            escalar(&mut e, Tirador::SuresteEsquina, se_original, false, false);

            assert!(
                (e.ancho - 100.0).abs() < 1e-2,
                "ancho a {angulo}: {}",
                e.ancho
            );
            assert!((e.alto - 50.0).abs() < 1e-2, "alto a {angulo}: {}", e.alto);
        }
    }

    #[test]
    fn cruzar_el_ancla_con_el_elemento_girado_voltea_sin_degenerar() {
        // El caso que las dos pruebas de arriba evitan a proposito, aqui
        // comprobado de frente: arrastrar al otro lado del ancla en un
        // elemento girado voltea. Lo que se exige es que el resultado siga
        // siendo un elemento valido —sin cero, sin NaN— y no que la esquina
        // noroeste siga quieta, porque tras un volteo esa ya no es el ancla.
        for angulo in [0.0, FRAC_PI_2, 2.6] {
            let mut e = rect();
            e.angulo = angulo;
            let centro = Punto2::nuevo(50.0, 25.0);
            let detras = arrastre_equivalente(Punto2::nuevo(-120.0, 90.0), centro, angulo);

            escalar(&mut e, Tirador::SuresteEsquina, detras, false, false);

            assert!(e.ancho >= MINIMO, "no se aplasta a {angulo}: {}", e.ancho);
            assert!(e.alto >= MINIMO, "ni de alto a {angulo}: {}", e.alto);
            assert!(e.x.is_finite() && e.y.is_finite(), "sin NaN a {angulo}");
        }
    }

    #[test]
    fn un_tirador_de_lado_solo_mueve_su_eje() {
        let mut e = rect();
        escalar(
            &mut e,
            Tirador::EsteBorde,
            Punto2::nuevo(300.0, 999.0),
            false,
            false,
        );
        assert_eq!(e.ancho, 300.0);
        assert_eq!(e.alto, 50.0, "el lado este no toca el alto");
    }

    #[test]
    fn escalar_un_trazo_mueve_sus_puntos() {
        // Elemento::caja() calcula la caja DE LOS PUNTOS para las figuras
        // con puntos. Si solo se cambiara ancho/alto, el trazo no escalaria
        // y el marco de seleccion se despegaria del dibujo.
        let mut e = Elemento {
            figura: Figura::Lapiz {
                puntos: vec![
                    Punto2::nuevo(0.0, 0.0),
                    Punto2::nuevo(50.0, 25.0),
                    Punto2::nuevo(100.0, 50.0),
                ],
                presiones: Vec::new(),
            },
            grosor: 0.0, // sin margen, para que la caja sean los puntos
            ..rect()
        };

        escalar(
            &mut e,
            Tirador::SuresteEsquina,
            Punto2::nuevo(200.0, 100.0),
            false,
            false,
        );

        let Figura::Lapiz { puntos, .. } = &e.figura else {
            panic!("sigue siendo un lapiz");
        };
        cerca(puntos[0], Punto2::nuevo(0.0, 0.0), "el primero es el ancla");
        cerca(
            puntos[2],
            Punto2::nuevo(200.0, 100.0),
            "el ultimo va al cursor",
        );
        cerca(
            puntos[1],
            Punto2::nuevo(100.0, 50.0),
            "el de en medio, a escala",
        );
    }

    #[test]
    fn con_shift_se_conserva_la_proporcion() {
        let mut e = rect();
        escalar(
            &mut e,
            Tirador::SuresteEsquina,
            Punto2::nuevo(200.0, 999.0),
            true,
            false,
        );
        assert!(
            (e.ancho / e.alto - 2.0).abs() < 1e-3,
            "100x50 es 2:1, y es {}x{}",
            e.ancho,
            e.alto
        );
    }

    #[test]
    fn con_alt_se_escala_desde_el_centro() {
        let mut e = rect();
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);

        escalar(
            &mut e,
            Tirador::SuresteEsquina,
            Punto2::nuevo(150.0, 75.0),
            false,
            true,
        );

        let (x0, y0, x1, y1) = e.caja();
        cerca(
            Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0),
            centro,
            "el centro no se mueve",
        );
    }

    #[test]
    fn cruzar_el_ancla_voltea_en_vez_de_aplastar() {
        // D30: aplastar a cero pierde informacion sin remedio; voltear es
        // reversible y es lo que espera quien cruzo el raton al otro lado.
        let mut e = rect();
        escalar(
            &mut e,
            Tirador::SuresteEsquina,
            Punto2::nuevo(-100.0, 50.0),
            false,
            false,
        );

        assert!(e.ancho >= MINIMO, "no se aplasta: {}", e.ancho);
        let (x0, _, x1, _) = e.caja();
        assert!(x0 < 0.0 && x1 <= 0.0 + 1e-3, "quedo al otro lado del ancla");
    }

    #[test]
    fn girar_alrededor_de_su_centro_suma_el_angulo_y_no_lo_mueve() {
        let mut e = rect();
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);

        girar(&mut e, centro, FRAC_PI_2);

        assert!((e.angulo - FRAC_PI_2).abs() < 1e-6);
        let (x0, y0, x1, y1) = e.caja();
        cerca(
            Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0),
            centro,
            "girar sobre si mismo no lo mueve",
        );
    }

    #[test]
    fn girar_alrededor_de_un_centro_ajeno_lo_mueve_en_orbita() {
        // Es lo que pasa al girar una seleccion de varios: cada uno gira
        // sobre si mismo Y orbita el centro comun.
        let mut e = rect();
        let ajeno = Punto2::nuevo(0.0, 0.0);
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);

        girar(&mut e, ajeno, FRAC_PI_2);

        let (x0, y0, x1, y1) = e.caja();
        let nuevo = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        cerca(
            nuevo,
            centro.girar(ajeno, FRAC_PI_2),
            "orbito el centro comun",
        );
    }

    #[test]
    fn girar_un_trazo_gira_sus_puntos() {
        let mut e = Elemento {
            figura: Figura::Lapiz {
                puntos: vec![Punto2::nuevo(10.0, 0.0), Punto2::nuevo(20.0, 0.0)],
                presiones: Vec::new(),
            },
            grosor: 0.0,
            ..rect()
        };
        girar(&mut e, Punto2::nuevo(0.0, 0.0), FRAC_PI_2);

        let Figura::Lapiz { puntos, .. } = &e.figura else {
            panic!()
        };
        cerca(puntos[0], Punto2::nuevo(0.0, 10.0), "el primero");
        cerca(puntos[1], Punto2::nuevo(0.0, 20.0), "el segundo");
    }

    #[test]
    fn los_saltos_de_giro_son_de_quince_grados() {
        // La media division son 7,5 grados = 0,1309 rad: por debajo se baja
        // al salto anterior y por encima se sube al siguiente.
        assert!(
            (a_saltos(0.10) - 0.0).abs() < 1e-6,
            "0,10 rad (5,7 grados) baja a 0"
        );
        assert!(
            (a_saltos(0.20) - SALTO_GIRO).abs() < 1e-6,
            "0,20 rad (11,5) ya sube a 15"
        );
        assert!(
            (a_saltos(0.30) - SALTO_GIRO).abs() < 1e-6,
            "0,30 rad (17,2) tambien"
        );
        assert!((a_saltos(-0.30) + SALTO_GIRO).abs() < 1e-6, "y en negativo");
    }

    #[test]
    fn el_angulo_hacia_arriba_es_cero() {
        // El tirador de giro esta encima del elemento; con el cursor ahi
        // mismo, el giro tiene que ser cero y no un cuarto de vuelta.
        let c = Punto2::nuevo(0.0, 0.0);
        assert!(angulo_hacia(c, Punto2::nuevo(0.0, -10.0)).abs() < 1e-6);
        let derecha = angulo_hacia(c, Punto2::nuevo(10.0, 0.0));
        assert!(
            (derecha - FRAC_PI_2).abs() < 1e-6,
            "a la derecha, +90: {derecha}"
        );
    }

    #[test]
    fn transformar_sube_la_version_para_invalidar_la_cache() {
        let mut e = rect();
        let antes = e.version;
        escalar(
            &mut e,
            Tirador::SuresteEsquina,
            Punto2::nuevo(200.0, 100.0),
            false,
            false,
        );
        assert_ne!(e.version, antes, "sin esto la cache pintaria lo viejo");
    }
}
