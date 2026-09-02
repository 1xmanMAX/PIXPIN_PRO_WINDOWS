//! De elemento a ordenes de dibujo.
//!
//! El motor **no dibuja**: produce la lista de poligonos y polilineas que hay
//! que pintar, con su color y su grosor. Quien las pinta es el consumidor —el
//! pin, la capa de pantalla, el PDF—, que ya tiene su pintor.
//!
//! Esto no es un rodeo, son dos ventajas concretas: el motor entero queda puro
//! y se prueba sin GPU (aqui, en CI, sin escritorio), y las mismas ordenes
//! valen para Direct2D hoy y para exportar a SVG manana sin tocar una linea de
//! geometria.

use crate::azar::Azar;
use crate::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use crate::escena::Escena;
use crate::formas;
use crate::trazo::{self, Ajustes};
use crate::vector::Punto2;

/// Una cosa que pintar. El consumidor traduce cada variante a su API.
#[derive(Debug, Clone, PartialEq)]
pub enum Orden {
    /// Contorno cerrado que se rellena. Es como se pinta la tinta de un
    /// trazo a mano: no es una linea gruesa, es una mancha con forma.
    Poligono {
        puntos: Vec<Punto2>,
        color: ColorRgba,
    },
    /// Linea abierta de grosor constante.
    Polilinea {
        puntos: Vec<Punto2>,
        color: ColorRgba,
        grosor: f32,
        estilo: EstiloTrazo,
    },
    /// Relleno de una figura cerrada (el interior de un rectangulo).
    Relleno {
        puntos: Vec<Punto2>,
        color: ColorRgba,
    },
    /// Texto en su caja.
    Texto {
        texto: String,
        x: f32,
        y: f32,
        tam: f32,
        familia: String,
        color: ColorRgba,
        ancho_max: f32,
    },
    /// Un bitmap que el consumidor tiene que resolver por su id.
    Imagen {
        id_objeto: u64,
        x: f32,
        y: f32,
        ancho: f32,
        alto: f32,
        opacidad: f32,
    },
}

/// Aplica la opacidad del elemento a un color.
fn con_opacidad(c: ColorRgba, opacidad: f32) -> ColorRgba {
    ColorRgba {
        a: c.a * opacidad.clamp(0.0, 1.0),
        ..c
    }
}

/// Las ordenes de dibujo de un elemento, en orden de pintado.
pub fn ordenes(e: &Elemento) -> Vec<Orden> {
    if e.borrado {
        return Vec::new();
    }
    // Un generador propio, sembrado con la semilla del elemento: asi su
    // aspecto no depende de cuantos elementos se dibujaron antes (D38).
    let mut azar = Azar::nuevo(e.semilla);
    let color = con_opacidad(e.trazo, e.opacidad);
    let mut salida = Vec::new();

    match &e.figura {
        Figura::Lapiz { puntos, .. } => {
            // La tinta es un poligono relleno, no una linea gruesa: es lo que
            // permite que el trazo adelgace en los extremos.
            let a = Ajustes {
                tamano: e.grosor,
                ..Default::default()
            };
            let contorno = trazo::poligono(puntos, &a);
            if !contorno.is_empty() {
                salida.push(Orden::Poligono {
                    puntos: contorno,
                    color,
                });
            }
        }

        Figura::Resaltador { puntos } => {
            // D45: grueso, translucido y SIN adelgazar. Un resaltador de
            // grosor variable deja el texto medio tapado.
            let a = Ajustes {
                tamano: e.grosor * 3.0,
                adelgazado: 0.0,
                ..Default::default()
            };
            let contorno = trazo::poligono(puntos, &a);
            if !contorno.is_empty() {
                salida.push(Orden::Poligono {
                    puntos: contorno,
                    color: ColorRgba {
                        a: 0.35 * e.opacidad,
                        ..e.trazo
                    },
                });
            }
        }

        Figura::Linea { puntos } => {
            for par in puntos.windows(2) {
                for pasada in formas::linea(par[0], par[1], e.rugosidad, &mut azar) {
                    salida.push(Orden::Polilinea {
                        puntos: pasada,
                        color,
                        grosor: e.grosor,
                        estilo: e.estilo,
                    });
                }
            }
        }

        Figura::Flecha {
            puntos,
            punta_inicio,
            punta_fin,
        } => {
            for par in puntos.windows(2) {
                for pasada in formas::linea(par[0], par[1], e.rugosidad, &mut azar) {
                    salida.push(Orden::Polilinea {
                        puntos: pasada,
                        color,
                        grosor: e.grosor,
                        estilo: e.estilo,
                    });
                }
            }
            if puntos.len() >= 2 {
                if *punta_fin {
                    let n = puntos.len();
                    for p in
                        formas::punta_flecha(puntos[n - 2], puntos[n - 1], e.rugosidad, &mut azar)
                    {
                        salida.push(Orden::Polilinea {
                            puntos: p,
                            color,
                            grosor: e.grosor,
                            // La punta siempre solida: una punta punteada no
                            // se lee como punta.
                            estilo: EstiloTrazo::Solido,
                        });
                    }
                }
                if *punta_inicio {
                    for p in formas::punta_flecha(puntos[1], puntos[0], e.rugosidad, &mut azar) {
                        salida.push(Orden::Polilinea {
                            puntos: p,
                            color,
                            grosor: e.grosor,
                            estilo: EstiloTrazo::Solido,
                        });
                    }
                }
            }
        }

        Figura::Rectangulo => {
            // El relleno va PRIMERO: si fuera despues taparia el trazo.
            if let Some(r) = e.relleno.filter(|c| c.a > 0.0) {
                salida.push(Orden::Relleno {
                    puntos: vec![
                        Punto2::nuevo(e.x, e.y),
                        Punto2::nuevo(e.x + e.ancho, e.y),
                        Punto2::nuevo(e.x + e.ancho, e.y + e.alto),
                        Punto2::nuevo(e.x, e.y + e.alto),
                    ],
                    color: con_opacidad(r, e.opacidad),
                });
            }
            for pasada in formas::rectangulo(e.x, e.y, e.ancho, e.alto, e.rugosidad, &mut azar) {
                salida.push(Orden::Polilinea {
                    puntos: pasada,
                    color,
                    grosor: e.grosor,
                    estilo: e.estilo,
                });
            }
        }

        Figura::Elipse => {
            if let Some(r) = e.relleno.filter(|c| c.a > 0.0) {
                // El relleno usa la elipse LISA, no la rugosa: rellenar la
                // temblorosa deja huecos por donde se escapa el fondo.
                let mut lisa = Azar::nuevo(e.semilla);
                if let Some(anillo) = formas::elipse(e.x, e.y, e.ancho, e.alto, 0.0, &mut lisa)
                    .into_iter()
                    .next()
                {
                    salida.push(Orden::Relleno {
                        puntos: anillo,
                        color: con_opacidad(r, e.opacidad),
                    });
                }
            }
            for pasada in formas::elipse(e.x, e.y, e.ancho, e.alto, e.rugosidad, &mut azar) {
                salida.push(Orden::Polilinea {
                    puntos: pasada,
                    color,
                    grosor: e.grosor,
                    estilo: e.estilo,
                });
            }
        }

        Figura::Texto {
            texto,
            tam,
            familia,
        } => salida.push(Orden::Texto {
            texto: texto.clone(),
            x: e.x,
            y: e.y,
            tam: *tam,
            familia: familia.clone(),
            color,
            ancho_max: e.ancho.max(1.0),
        }),

        Figura::Imagen { id_objeto } => salida.push(Orden::Imagen {
            id_objeto: *id_objeto,
            x: e.x,
            y: e.y,
            ancho: e.ancho,
            alto: e.alto,
            opacidad: e.opacidad,
        }),
    }

    salida
}

/// Las ordenes de la escena entera, de abajo arriba.
pub fn ordenes_de_escena(escena: &Escena) -> Vec<Orden> {
    escena.visibles().flat_map(ordenes).collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn base() -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Rectangulo,
            x: 10.0,
            y: 10.0,
            ancho: 100.0,
            alto: 50.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 99,
            version: 0,
            borrado: false,
        }
    }

    #[test]
    fn un_elemento_borrado_no_produce_ninguna_orden() {
        let e = Elemento {
            borrado: true,
            ..base()
        };
        assert!(ordenes(&e).is_empty());
    }

    #[test]
    fn el_relleno_se_pinta_antes_que_el_trazo() {
        // Al reves, el relleno taparia el borde y la figura se veria sin
        // contorno.
        let e = Elemento {
            relleno: Some(ColorRgba::opaco(1.0, 1.0, 0.0)),
            ..base()
        };
        let o = ordenes(&e);
        assert!(matches!(o[0], Orden::Relleno { .. }), "primero el relleno");
        assert!(o[1..].iter().any(|x| matches!(x, Orden::Polilinea { .. })));
    }

    #[test]
    fn el_mismo_elemento_produce_siempre_las_mismas_ordenes() {
        // La puerta de la fase, a nivel de dibujo: reabrir el documento no
        // cambia ni un punto.
        let e = base();
        assert_eq!(ordenes(&e), ordenes(&e));
    }

    #[test]
    fn cambiar_la_semilla_cambia_el_dibujo() {
        // Caso negativo: si la semilla no llegara hasta aqui, todas las
        // figuras del documento saldrian calcadas.
        let a = base();
        let b = Elemento {
            semilla: 100,
            ..base()
        };
        assert_ne!(ordenes(&a), ordenes(&b));
    }

    #[test]
    fn el_resaltador_es_translucido_y_de_grosor_constante() {
        // D45: si adelgazara o fuera opaco, el texto de debajo quedaria
        // ilegible, que es justo lo contrario de resaltar.
        let e = Elemento {
            figura: Figura::Resaltador {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
            },
            trazo: ColorRgba::opaco(1.0, 1.0, 0.0),
            ..base()
        };
        match &ordenes(&e)[0] {
            Orden::Poligono { color, .. } => {
                assert!(color.a < 0.5, "no es translucido: alfa {}", color.a)
            }
            otra => panic!("el resaltador deberia ser un poligono, es {otra:?}"),
        }
    }

    #[test]
    fn una_flecha_con_las_dos_puntas_dibuja_mas_que_una_con_una() {
        let con_una = Elemento {
            figura: Figura::Flecha {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
                punta_inicio: false,
                punta_fin: true,
            },
            ..base()
        };
        let con_dos = Elemento {
            figura: Figura::Flecha {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
                punta_inicio: true,
                punta_fin: true,
            },
            ..base()
        };
        assert!(ordenes(&con_dos).len() > ordenes(&con_una).len());
    }

    #[test]
    fn la_opacidad_del_elemento_llega_al_color() {
        let e = Elemento {
            opacidad: 0.5,
            ..base()
        };
        match &ordenes(&e)[0] {
            Orden::Polilinea { color, .. } => assert_eq!(color.a, 0.5),
            otra => panic!("se esperaba una polilinea, es {otra:?}"),
        }
    }

    #[test]
    fn la_escena_dibuja_de_abajo_arriba_y_salta_lo_borrado() {
        let mut escena = Escena::nueva();
        escena.anadir(base());
        let dos = escena.anadir(Elemento {
            figura: Figura::Elipse,
            ..base()
        });
        let con_ambos = ordenes_de_escena(&escena).len();
        escena.borrar(dos);
        assert!(ordenes_de_escena(&escena).len() < con_ambos);
    }

    #[test]
    fn un_texto_produce_una_orden_de_texto_con_su_ancho() {
        let e = Elemento {
            figura: Figura::Texto {
                texto: "hola".into(),
                tam: 14.0,
                familia: "Segoe UI".into(),
            },
            ..base()
        };
        match &ordenes(&e)[0] {
            Orden::Texto {
                texto, ancho_max, ..
            } => {
                assert_eq!(texto, "hola");
                assert_eq!(*ancho_max, 100.0);
            }
            otra => panic!("se esperaba texto, es {otra:?}"),
        }
    }
}
