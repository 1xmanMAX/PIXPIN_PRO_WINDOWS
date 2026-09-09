//! El iman: **un solo sitio que decide a que se pega el cursor**.
//!
//! Es lo que separa un esquema de un monton de rayas casi alineadas. Con el
//! raton se falla la esquina exacta de un rectangulo por dos o tres pixeles,
//! y esos dos o tres pixeles se ven.
//!
//! ## Por que hay una sola puerta
//!
//! En el Android que se porta, seis sitios distintos llamaban al buscador de
//! anclajes y cada uno decidia por su cuenta tres cosas: si engancha, con que
//! radio y que anclajes cuentan. El resultado previsible fue que cada
//! herramienta nueva habia que acordarse de anadirla a esa lista, y a alguna
//! se le olvidaba. Aqui se dice **que se esta haciendo** ([`Faena`]) y el
//! motor responde: una herramienta nueva encaja en una faena y hereda su
//! comportamiento entero, sin tocar nada de este fichero.
//!
//! Sin escritorio y sin estado: se le dan los elementos y un punto, y dice a
//! donde habria que pegarse. Asi se comprueba sin dispositivo que engancha
//! donde debe y -mas importante- que **no** engancha donde no debe.

use serde::{Deserialize, Serialize};

use crate::elemento::{Elemento, Figura};
use crate::vector::Punto2;

/// Que clase de punto notable es. Sirve para pintar la pista de otra forma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoAnclaje {
    /// Una de las cuatro esquinas de la caja de una figura.
    Esquina,
    /// El primer o el ultimo punto de una figura con puntos.
    Extremo,
    /// El medio de un lado, o el punto de en medio de una lista de puntos.
    Medio,
    /// El centro de la caja. En una elipse es el centro de la
    /// circunferencia, que es lo que se busca al trazar un radio.
    Centro,
}

/// Un punto al que merece la pena pegarse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anclaje {
    /// Ya en coordenadas del mundo, girado si el elemento lo estaba.
    pub punto: Punto2,
    pub tipo: TipoAnclaje,
    pub id: u64,
}

/// Que se esta haciendo cuando se pide sitio.
///
/// No es «que herramienta hay puesta», y es a proposito. Lo que decide a que
/// debe pegarse el cursor no es el boton que este pulsado sino **que esta
/// pasando**: afinar la punta de una flecha ya trazada y afinar la de una
/// recta son la misma faena aunque sean dos herramientas, y trazar a mano
/// alzada es otra aunque el lapiz y el marcador sean dos botones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Faena {
    /// Naciendo una figura: se arrastra y va creciendo.
    Trazando,
    /// A mano alzada. **No engancha a nada**: ver [`ajustes_para`].
    AMano,
    /// Recolocando un punto de algo ya dibujado.
    Afinando,
    /// Arrastrando algo entero de un sitio a otro.
    Moviendo,
}

/// Que se engancha. Es la parte **configurable**: cada clase de punto se
/// puede apagar por separado, y apagarlas todas deja el dibujo libre.
///
/// No es una preferencia de adorno. Un iman que tira cuando no quieres es
/// peor que no tenerlo, y cual estorba depende de lo que estes dibujando.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ajustes {
    pub activo: bool,
    /// Gobierna [`TipoAnclaje::Esquina`] **y** [`TipoAnclaje::Extremo`]:
    /// para quien dibuja son la misma idea -el vertice de algo- y dos
    /// casillas para eso serian dos casillas que nadie entiende por
    /// separado.
    pub esquinas: bool,
    pub medios: bool,
    pub centros: bool,
    /// Radio de captura **en pixeles de pantalla**, no de escena.
    ///
    /// En pantalla es donde ocurre el problema: el cursor apunta con la
    /// misma precision mires al zoom que mires. Si el radio fuese de
    /// escena, muy acercado engancharia a medio dibujo y muy alejado no
    /// engancharia a nada.
    pub radio_px: f32,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            activo: true,
            esquinas: true,
            medios: true,
            centros: true,
            radio_px: 14.0,
        }
    }
}

impl Ajustes {
    /// Todo apagado: dibujar a pulso, sin que nada tire del cursor.
    pub const NINGUNO: Self = Self {
        activo: false,
        esquinas: false,
        medios: false,
        centros: false,
        radio_px: 14.0,
    };
}

/// Cuanto tienen que diferenciarse dos distancias para que una gane por
/// cercania. Por debajo de esto es empate y manda la prioridad.
const EMPATE: f32 = 0.001;

/// Menor es mas prioritario.
fn prioridad(t: TipoAnclaje) -> u8 {
    match t {
        // Un vertice es lo mas intencionado que hay en un dibujo.
        TipoAnclaje::Esquina | TipoAnclaje::Extremo => 1,
        TipoAnclaje::Medio => 2,
        // El centro de una caja grande esta lejos de todo, y engancharse a
        // el por sorpresa desconcierta mas de lo que ayuda.
        TipoAnclaje::Centro => 3,
    }
}

/// Los ajustes que tocan para esta faena, partiendo de los del usuario.
///
/// **Lo que el usuario apaga se queda apagado**, pase lo que pase: la faena
/// puede quitar cosas pero nunca encender lo que se ha desactivado a mano.
/// Al reves seria un ajuste que no se obedece, que es peor que no tenerlo.
pub fn ajustes_para(faena: Faena, config: &Ajustes) -> Ajustes {
    match faena {
        Faena::Trazando | Faena::Afinando => *config,

        // Moviendo se busca un sitio, no se traza: se puede ser mas
        // generoso con el radio sin que estorbe, porque no hay un trazo en
        // curso al que dar un tiron.
        Faena::Moviendo => Ajustes {
            radio_px: config.radio_px * 1.5,
            ..*config
        },

        // A mano alzada, nada. Un trazo que salta a un vertice en mitad del
        // recorrido no se corrige, **se rompe**: el garabato pega un tiron y
        // sigue. Lo unico que el original permite aqui es el canto de una
        // guia -no es un punto al que ir, es una superficie sobre la que
        // resbalar-, y ni el canto ni las guias existen todavia.
        //
        // Esta rama devolviendo siempre nada NO es codigo muerto: es lo que
        // hace imposible que alguien cablee el lapiz a `Trazando` y se coma
        // el tiron. El dia que exista el ancla del canto, se enciende aqui y
        // el lapiz empieza a resbalar sin que nadie recuerde nada.
        Faena::AMano => Ajustes::NINGUNO,
    }
}

/// A que se pega el cursor, o `None` si nada tira de el.
///
/// Es **la unica puerta**. Quien quiera enganchar algo pregunta aqui y no
/// busca anclajes por su cuenta: es lo que hace que anadir una herramienta no
/// obligue a acordarse del iman.
///
/// `excluir` son los elementos que se estan dibujando o moviendo: sus propios
/// puntos no cuentan. Es una rebanada y no un solo id porque al mover una
/// seleccion de varios, excluir uno solo deja que el grupo se enganche a sus
/// propios miembros, que se arrastran con el.
///
/// No monta ninguna lista de anclajes: esto corre en cada movimiento del
/// raton, y el camino caliente tiene presupuesto de cero asignaciones.
pub fn sitio(
    elementos: &[Elemento],
    p: Punto2,
    zoom: f32,
    faena: Faena,
    config: &Ajustes,
    excluir: &[u64],
) -> Option<Anclaje> {
    let ajustes = ajustes_para(faena, config);
    if !ajustes.activo {
        return None;
    }
    // El radio se da en pixeles de pantalla y aqui se trabaja en escena.
    let radio = ajustes.radio_px / zoom.max(0.0001);

    let mut mejor: Option<Anclaje> = None;
    let mut mejor_d = f32::MAX;

    for e in elementos {
        if e.borrado || excluir.contains(&e.id) {
            continue;
        }
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);

        // El angulo se deshace sobre el punto, no sobre la figura -el mismo
        // truco que `impacto::toca`-. Asi la caja local vale de criba gire o
        // no el elemento, y no hace falta calcular ninguna caja girada: solo
        // el ancla que gane vuelve al mundo. Girar conserva distancias, asi
        // que comparar en local es identico a comparar en el mundo.
        let local = if e.angulo != 0.0 {
            p.girar(centro, -e.angulo)
        } else {
            p
        };

        // Todos los anclajes caen dentro de la caja: si el cursor esta a mas
        // de un radio de ella, no hay nada que mirar. La resta es lo que hace
        // soportable un plano importado -sin ella, cada movimiento montaria
        // los anclajes de miles de elementos para tirarlos-.
        if local.x < x0 - radio
            || local.x > x1 + radio
            || local.y < y0 - radio
            || local.y > y1 + radio
        {
            continue;
        }

        por_cada_anclaje(e, &ajustes, |punto_local, tipo| {
            let d = punto_local.distancia(local);
            if d > radio {
                return;
            }
            let gana = d < mejor_d - EMPATE
                || ((d - mejor_d).abs() <= EMPATE
                    && mejor.is_some_and(|m| prioridad(tipo) < prioridad(m.tipo)));
            if gana {
                let punto = if e.angulo != 0.0 {
                    punto_local.girar(centro, e.angulo)
                } else {
                    punto_local
                };
                mejor = Some(Anclaje {
                    punto,
                    tipo,
                    id: e.id,
                });
                mejor_d = d;
            }
        });
    }
    mejor
}

/// Los puntos notables de `e`, **en su marco local**, uno a uno.
///
/// Entrega a un sumidero en vez de devolver un `Vec` porque corre por cada
/// elemento y cada movimiento del raton: montar una lista para tirarla es
/// justo lo que el presupuesto de cero asignaciones prohibe.
fn por_cada_anclaje(e: &Elemento, ajustes: &Ajustes, mut f: impl FnMut(Punto2, TipoAnclaje)) {
    match &e.figura {
        // Las figuras con puntos dan sus extremos y su medio; **su caja no
        // significa nada**: la caja de una diagonal tiene dos esquinas por
        // las que la linea no pasa.
        Figura::Lapiz { puntos, .. }
        | Figura::Resaltador { puntos }
        | Figura::Linea { puntos }
        | Figura::Flecha { puntos, .. }
        | Figura::Cota { puntos } => {
            if puntos.len() < 2 {
                return;
            }
            if ajustes.esquinas {
                f(puntos[0], TipoAnclaje::Extremo);
                f(puntos[puntos.len() - 1], TipoAnclaje::Extremo);
            }
            if ajustes.medios {
                f(puntos[puntos.len() / 2], TipoAnclaje::Medio);
            }
        }

        _ => {
            let (x0, y0, x1, y1) = e.caja();
            let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            if ajustes.esquinas {
                f(Punto2::nuevo(x0, y0), TipoAnclaje::Esquina);
                f(Punto2::nuevo(x1, y0), TipoAnclaje::Esquina);
                f(Punto2::nuevo(x1, y1), TipoAnclaje::Esquina);
                f(Punto2::nuevo(x0, y1), TipoAnclaje::Esquina);
            }
            if ajustes.medios {
                f(Punto2::nuevo(cx, y0), TipoAnclaje::Medio);
                f(Punto2::nuevo(x1, cy), TipoAnclaje::Medio);
                f(Punto2::nuevo(cx, y1), TipoAnclaje::Medio);
                f(Punto2::nuevo(x0, cy), TipoAnclaje::Medio);
            }
            if ajustes.centros {
                f(Punto2::nuevo(cx, cy), TipoAnclaje::Centro);
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo};

    /// Un rectangulo de 100x50 en (0,0), sin girar.
    fn rectangulo(id: u64) -> Elemento {
        Elemento {
            figura: Figura::Rectangulo,
            id,
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

    /// Una linea de (0,0) a (100,0), con un punto intermedio en (50,0).
    fn linea(id: u64) -> Elemento {
        Elemento {
            figura: Figura::Linea {
                puntos: vec![
                    Punto2::nuevo(0.0, 0.0),
                    Punto2::nuevo(50.0, 0.0),
                    Punto2::nuevo(100.0, 0.0),
                ],
            },
            ..rectangulo(id)
        }
    }

    fn todo() -> Ajustes {
        Ajustes::default()
    }

    #[test]
    fn engancha_a_la_esquina_de_un_rectangulo() {
        let es = [rectangulo(1)];
        // A 3 de escena de la esquina (0,0), con radio 14 px a zoom 1.
        let a = sitio(
            &es,
            Punto2::nuevo(3.0, 3.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        )
        .expect("tenia que enganchar");
        assert_eq!(a.tipo, TipoAnclaje::Esquina);
        assert_eq!(a.punto, Punto2::nuevo(0.0, 0.0));
        assert_eq!(a.id, 1);
    }

    #[test]
    fn engancha_al_medio_de_un_lado() {
        let es = [rectangulo(1)];
        // El medio del lado de arriba es (50,0). El centro es (50,25),
        // que esta a 25 y queda fuera del radio.
        let a = sitio(
            &es,
            Punto2::nuevo(50.0, 2.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        )
        .expect("tenia que enganchar");
        assert_eq!(a.tipo, TipoAnclaje::Medio);
        assert_eq!(a.punto, Punto2::nuevo(50.0, 0.0));
    }

    #[test]
    fn engancha_al_centro() {
        let es = [rectangulo(1)];
        let a = sitio(
            &es,
            Punto2::nuevo(51.0, 26.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        )
        .expect("tenia que enganchar");
        assert_eq!(a.tipo, TipoAnclaje::Centro);
        assert_eq!(a.punto, Punto2::nuevo(50.0, 25.0));
    }

    #[test]
    fn una_linea_da_extremos_y_medio_pero_no_esquinas_de_su_caja() {
        // La trampa 2 del diseno: `caja()` de una linea es la caja de sus
        // puntos, y sus esquinas son de un rectangulo por el que la linea
        // no pasa. Aqui la caja es (0,-1)-(100,1) por el grosor, asi que
        // su esquina superior izquierda seria (0,-1): no debe existir.
        let es = [linea(1)];
        let a = sitio(
            &es,
            Punto2::nuevo(0.5, -1.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        )
        .expect("tenia que enganchar al extremo");
        assert_eq!(a.tipo, TipoAnclaje::Extremo);
        assert_eq!(a.punto, Punto2::nuevo(0.0, 0.0));

        // Y el medio es el punto de en medio de la LISTA, no el centro de
        // la caja.
        let m = sitio(
            &es,
            Punto2::nuevo(50.0, 1.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        )
        .expect("tenia que enganchar al medio");
        assert_eq!(m.tipo, TipoAnclaje::Medio);
        assert_eq!(m.punto, Punto2::nuevo(50.0, 0.0));
    }

    #[test]
    fn un_rectangulo_girado_engancha_por_su_esquina_de_verdad() {
        // La trampa 1 del diseno. Girado 90 grados sobre su centro
        // (50,25), la esquina (0,0) acaba en (75,-25) -fuera de la caja
        // local, que sigue siendo (0,0)-(100,50)-. Si la criba usara la
        // caja sin girar, este enganche no ocurriria.
        let mut e = rectangulo(1);
        e.angulo = std::f32::consts::FRAC_PI_2;
        let es = [e];
        let a = sitio(
            &es,
            Punto2::nuevo(76.0, -24.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        )
        .expect("una esquina girada tambien engancha");
        assert_eq!(a.tipo, TipoAnclaje::Esquina);
        assert!(
            a.punto.distancia(Punto2::nuevo(75.0, -25.0)) < 0.01,
            "el ancla vuelve girada al mundo, no se queda en local: {:?}",
            a.punto
        );
    }

    #[test]
    fn el_elemento_excluido_no_se_engancha_a_si_mismo() {
        let es = [rectangulo(1)];
        assert!(
            sitio(
                &es,
                Punto2::nuevo(3.0, 3.0),
                1.0,
                Faena::Trazando,
                &todo(),
                &[1]
            )
            .is_none(),
            "una figura naciendo se pegaria a su propia esquina"
        );
    }

    #[test]
    fn se_excluye_una_seleccion_entera_no_solo_uno() {
        let es = [rectangulo(1), rectangulo(2)];
        assert!(
            sitio(
                &es,
                Punto2::nuevo(3.0, 3.0),
                1.0,
                Faena::Moviendo,
                &todo(),
                &[1, 2]
            )
            .is_none(),
            "un grupo que se mueve no puede pegarse a sus propios miembros"
        );
    }

    #[test]
    fn un_elemento_borrado_no_ofrece_anclajes() {
        let mut e = rectangulo(1);
        e.borrado = true;
        assert!(
            sitio(
                &[e],
                Punto2::nuevo(3.0, 3.0),
                1.0,
                Faena::Trazando,
                &todo(),
                &[]
            )
            .is_none()
        );
    }

    #[test]
    fn fuera_del_radio_no_engancha_nada() {
        let es = [rectangulo(1)];
        assert!(
            sitio(
                &es,
                Punto2::nuevo(-20.0, -20.0),
                1.0,
                Faena::Trazando,
                &todo(),
                &[]
            )
            .is_none()
        );
    }

    #[test]
    fn el_radio_es_de_pantalla_asi_que_el_zoom_lo_encoge() {
        let es = [rectangulo(1)];
        let p = Punto2::nuevo(10.0, 0.0);
        // A zoom 1 el radio de escena son 14: 10 entra.
        assert!(sitio(&es, p, 1.0, Faena::Trazando, &todo(), &[]).is_some());
        // A zoom 4 son 3,5: 10 ya no entra, a la MISMA distancia de escena.
        assert!(sitio(&es, p, 4.0, Faena::Trazando, &todo(), &[]).is_none());
    }

    #[test]
    fn a_mano_alzada_no_engancha_nunca() {
        let es = [rectangulo(1)];
        assert!(
            sitio(
                &es,
                Punto2::nuevo(0.0, 0.0),
                1.0,
                Faena::AMano,
                &todo(),
                &[]
            )
            .is_none(),
            "un trazo que salta a un vertice no se corrige, se rompe"
        );
    }

    #[test]
    fn lo_que_el_usuario_apaga_sigue_apagado_en_todas_las_faenas() {
        let es = [rectangulo(1)];
        let sin_esquinas = Ajustes {
            esquinas: false,
            ..Ajustes::default()
        };
        for faena in [Faena::Trazando, Faena::Afinando, Faena::Moviendo] {
            let a = sitio(&es, Punto2::nuevo(1.0, 1.0), 1.0, faena, &sin_esquinas, &[]);
            assert!(
                a.is_none_or(|a| a.tipo != TipoAnclaje::Esquina),
                "la faena {faena:?} encendio una esquina que el usuario apago"
            );
        }
    }

    #[test]
    fn apagar_el_iman_entero_lo_deja_libre() {
        let es = [rectangulo(1)];
        assert!(
            sitio(
                &es,
                Punto2::nuevo(0.0, 0.0),
                1.0,
                Faena::Trazando,
                &Ajustes::NINGUNO,
                &[]
            )
            .is_none()
        );
    }

    #[test]
    fn en_empate_de_distancia_gana_la_esquina_al_centro() {
        // Un cuadrado de 20x20: la esquina (0,0) y el centro (10,10)
        // estan los dos a la misma distancia del punto (5,5).
        let mut e = rectangulo(1);
        e.ancho = 20.0;
        e.alto = 20.0;
        let a = sitio(
            &[e],
            Punto2::nuevo(5.0, 5.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        )
        .expect("tenia que enganchar");
        assert_eq!(
            a.tipo,
            TipoAnclaje::Esquina,
            "el centro desconcierta; el vertice es lo intencionado"
        );
    }

    #[test]
    fn moviendo_engancha_mas_lejos_que_trazando() {
        let es = [rectangulo(1)];
        // 18 de escena: fuera del radio 14 de `Trazando`, dentro del 21
        // (14 x 1,5) de `Moviendo`.
        let p = Punto2::nuevo(-18.0, 0.0);
        assert!(sitio(&es, p, 1.0, Faena::Trazando, &todo(), &[]).is_none());
        assert!(sitio(&es, p, 1.0, Faena::Moviendo, &todo(), &[]).is_some());
    }
}
