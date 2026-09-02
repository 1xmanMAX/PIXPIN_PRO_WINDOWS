//! La maquina de anotar: que pasa cuando arrastras con cada herramienta.
//!
//! Es la misma pieza para las dos cosas que pidio el usuario —anotar dentro de
//! un pin y anotar sobre la pantalla—, porque el comportamiento es identico y
//! lo unico que cambia es el fondo. Aqui no hay ventanas ni Direct2D: entran
//! eventos, salen efectos, y todo se prueba en CI sin escritorio.
//!
//! La distincion que mas se nota al usarlo: **un clic no es un arrastre**. Con
//! el lapiz, un clic deja un punto de tinta; con el rectangulo, un clic no
//! deja nada, porque un rectangulo de cero por cero es basura invisible que
//! luego estorba al seleccionar.

use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use pixpin_motor2d::vector::Punto2;

/// Cuanto hay que moverse para que deje de ser un clic (px logicos).
pub const UMBRAL_ARRASTRE: f32 = 4.0;
/// Grosor por defecto y sus topes al girar la rueda.
pub const GROSOR_POR_DEFECTO: f32 = 4.0;
pub const GROSOR_MINIMO: f32 = 1.0;
pub const GROSOR_MAXIMO: f32 = 48.0;
/// Aumento de la lupa y sus topes.
pub const LUPA_POR_DEFECTO: f32 = 2.0;
pub const LUPA_MINIMA: f32 = 1.5;
pub const LUPA_MAXIMA: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Herramienta {
    /// Seleccionar y mover lo ya dibujado.
    Mano,
    Lapiz,
    Resaltador,
    Linea,
    Flecha,
    Rectangulo,
    Elipse,
    Texto,
    /// Oscurece todo menos una zona (D51).
    Foco,
    /// Amplia alrededor del cursor. No deja rastro: es una vista (D52).
    Lupa,
    Borrador,
}

impl Herramienta {
    /// Si la herramienta necesita un arrastre de verdad para producir algo.
    /// El lapiz no: un clic deja un punto de tinta, que es lo que espera
    /// cualquiera que haya usado un rotulador.
    pub fn necesita_arrastre(self) -> bool {
        !matches!(self, Herramienta::Lapiz | Herramienta::Texto)
    }

    /// Si lo que dibuja se guarda en el documento.
    pub fn deja_rastro(self) -> bool {
        !matches!(
            self,
            Herramienta::Mano | Herramienta::Lupa | Herramienta::Borrador
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeclaAnotador {
    Escape,
    Deshacer,
    Rehacer,
    Suprimir,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventoAnotador {
    Pulsar(Punto2),
    Mover(Punto2),
    Soltar(Punto2),
    /// Positivo hacia arriba, como manda Windows.
    Rueda(i32),
    Tecla(TeclaAnotador),
    CambiarHerramienta(Herramienta),
    CambiarColor(ColorRgba),
}

#[derive(Debug, Clone, PartialEq)]
pub enum EfectoAnotador {
    Nada,
    /// Cambio algo que se ve pero no la escena (la lupa, el grosor).
    Repintar,
    /// Previsualizacion mientras se arrastra: NO se anade a la escena.
    EnCurso(Box<Elemento>),
    /// Terminado: el consumidor lo anade a la escena.
    Terminado(Box<Elemento>),
    BorrarEn(Punto2),
    Deshacer,
    Rehacer,
    /// El consumidor abre su editor de texto en ese punto.
    PedirTexto(Punto2),
    Salir,
}

/// El estado de la anotacion en curso.
#[derive(Debug)]
pub struct Anotador {
    herramienta: Herramienta,
    color: ColorRgba,
    grosor: f32,
    lupa: f32,
    /// Semilla del elemento que se esta dibujando; sube al terminar cada uno
    /// para que dos figuras seguidas no salgan calcadas.
    semilla: u32,
    gesto: Option<Gesto>,
}

#[derive(Debug, Clone)]
struct Gesto {
    inicio: Punto2,
    puntos: Vec<Punto2>,
    /// Si ya se paso del umbral: un clic y un arrastre no son lo mismo.
    arrastrando: bool,
}

impl Anotador {
    pub fn nuevo(semilla_inicial: u32) -> Self {
        Self {
            herramienta: Herramienta::Lapiz,
            color: ColorRgba::opaco(0.90, 0.15, 0.15),
            grosor: GROSOR_POR_DEFECTO,
            lupa: LUPA_POR_DEFECTO,
            semilla: semilla_inicial.max(1),
            gesto: None,
        }
    }

    pub fn herramienta(&self) -> Herramienta {
        self.herramienta
    }

    pub fn color(&self) -> ColorRgba {
        self.color
    }

    pub fn grosor(&self) -> f32 {
        self.grosor
    }

    pub fn lupa(&self) -> f32 {
        self.lupa
    }

    /// Si hay un gesto en curso. Mientras lo haya, el pin no se puede mover.
    pub fn dibujando(&self) -> bool {
        self.gesto.is_some()
    }

    pub fn procesar(&mut self, evento: EventoAnotador) -> EfectoAnotador {
        match evento {
            EventoAnotador::CambiarHerramienta(h) => {
                // Cambiar de herramienta a media raya ABANDONA el trazo: si
                // se terminara, saldria medio lapiz y medio rectangulo.
                self.gesto = None;
                self.herramienta = h;
                EfectoAnotador::Repintar
            }

            EventoAnotador::CambiarColor(c) => {
                self.color = c;
                EfectoAnotador::Repintar
            }

            EventoAnotador::Rueda(delta) => {
                // Cada modo le da a la rueda lo mas util de ese modo (D55).
                if self.herramienta == Herramienta::Lupa {
                    let paso = if delta > 0 { 1.25 } else { 0.8 };
                    self.lupa = (self.lupa * paso).clamp(LUPA_MINIMA, LUPA_MAXIMA);
                } else {
                    let paso = if delta > 0 { 1.0 } else { -1.0 };
                    self.grosor = (self.grosor + paso).clamp(GROSOR_MINIMO, GROSOR_MAXIMO);
                }
                EfectoAnotador::Repintar
            }

            EventoAnotador::Tecla(t) => match t {
                TeclaAnotador::Escape if self.gesto.is_some() => {
                    // El primer Escape abandona el trazo en curso; el
                    // segundo sale. Asi no se pierde el dibujo por un
                    // Escape de mas.
                    self.gesto = None;
                    EfectoAnotador::Repintar
                }
                TeclaAnotador::Escape => EfectoAnotador::Salir,
                TeclaAnotador::Deshacer => EfectoAnotador::Deshacer,
                TeclaAnotador::Rehacer => EfectoAnotador::Rehacer,
                TeclaAnotador::Suprimir => EfectoAnotador::Deshacer,
            },

            EventoAnotador::Pulsar(p) => {
                match self.herramienta {
                    Herramienta::Borrador => return EfectoAnotador::BorrarEn(p),
                    Herramienta::Texto => return EfectoAnotador::PedirTexto(p),
                    // La mano y la lupa no dibujan: las gestiona el
                    // consumidor, que es quien sabe que hay debajo.
                    Herramienta::Mano | Herramienta::Lupa => return EfectoAnotador::Nada,
                    _ => {}
                }
                self.gesto = Some(Gesto {
                    inicio: p,
                    puntos: vec![p],
                    arrastrando: false,
                });
                EfectoAnotador::Nada
            }

            EventoAnotador::Mover(p) => {
                let Some(g) = self.gesto.as_mut() else {
                    // Sin boton pulsado, mover solo importa para la lupa.
                    return if self.herramienta == Herramienta::Lupa {
                        EfectoAnotador::Repintar
                    } else {
                        EfectoAnotador::Nada
                    };
                };
                if p.distancia(g.inicio) > UMBRAL_ARRASTRE {
                    g.arrastrando = true;
                }
                g.puntos.push(p);
                match self.construir(false) {
                    Some(e) => EfectoAnotador::EnCurso(Box::new(e)),
                    None => EfectoAnotador::Nada,
                }
            }

            EventoAnotador::Soltar(p) => {
                let Some(g) = self.gesto.as_mut() else {
                    return EfectoAnotador::Nada;
                };
                g.puntos.push(p);
                if p.distancia(g.inicio) > UMBRAL_ARRASTRE {
                    g.arrastrando = true;
                }
                let arrastrado = g.arrastrando;
                let elemento = self.construir(true);
                self.gesto = None;

                // Un clic con el rectangulo no deja nada: una figura de cero
                // por cero es basura invisible que luego estorba al
                // seleccionar. Con el lapiz SI, que es un punto de tinta.
                if !arrastrado && self.herramienta.necesita_arrastre() {
                    return EfectoAnotador::Repintar;
                }
                match elemento {
                    Some(e) => {
                        // Semilla nueva para el siguiente: dos rectangulos
                        // seguidos no pueden salir calcados.
                        self.semilla = self.semilla.wrapping_mul(48271) & 0x7FFF_FFFF;
                        self.semilla = self.semilla.max(1);
                        EfectoAnotador::Terminado(Box::new(e))
                    }
                    None => EfectoAnotador::Repintar,
                }
            }
        }
    }

    /// El elemento que corresponde al gesto actual. `None` si todavia no hay
    /// nada que ensenar.
    fn construir(&self, _terminado: bool) -> Option<Elemento> {
        let g = self.gesto.as_ref()?;
        if !self.herramienta.deja_rastro() {
            return None;
        }
        let ultimo = *g.puntos.last()?;
        let (x0, y0) = (g.inicio.x.min(ultimo.x), g.inicio.y.min(ultimo.y));
        let (x1, y1) = (g.inicio.x.max(ultimo.x), g.inicio.y.max(ultimo.y));

        let figura = match self.herramienta {
            Herramienta::Lapiz => Figura::Lapiz {
                puntos: g.puntos.clone(),
                presiones: Vec::new(),
            },
            Herramienta::Resaltador => Figura::Resaltador {
                puntos: g.puntos.clone(),
            },
            Herramienta::Linea => Figura::Linea {
                puntos: vec![g.inicio, ultimo],
            },
            Herramienta::Flecha => Figura::Flecha {
                puntos: vec![g.inicio, ultimo],
                punta_inicio: false,
                punta_fin: true,
            },
            Herramienta::Rectangulo | Herramienta::Foco => Figura::Rectangulo,
            Herramienta::Elipse => Figura::Elipse,
            _ => return None,
        };

        // El foco es un rectangulo con relleno oscuro: oscurece lo de fuera
        // desde el punto de vista del ojo, aunque tecnicamente pinte dentro.
        let (relleno, color, opacidad) = if self.herramienta == Herramienta::Foco {
            (
                Some(ColorRgba {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.6,
                }),
                ColorRgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.9,
                },
                1.0,
            )
        } else {
            (None, self.color, 1.0)
        };

        Some(Elemento {
            id: 0, // lo asigna la escena
            figura,
            x: x0,
            y: y0,
            ancho: x1 - x0,
            alto: y1 - y0,
            angulo: 0.0,
            trazo: color,
            relleno,
            grosor: self.grosor,
            estilo: EstiloTrazo::Solido,
            // El resaltador nunca tiembla: sobre texto se leeria peor (D45).
            rugosidad: if self.herramienta == Herramienta::Resaltador {
                0.0
            } else {
                1.0
            },
            opacidad,
            semilla: self.semilla,
            version: 0,
            borrado: false,
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn p(x: f32, y: f32) -> Punto2 {
        Punto2::nuevo(x, y)
    }

    fn arrastrar(a: &mut Anotador, desde: Punto2, hasta: Punto2) -> EfectoAnotador {
        a.procesar(EventoAnotador::Pulsar(desde));
        a.procesar(EventoAnotador::Mover(hasta));
        a.procesar(EventoAnotador::Soltar(hasta))
    }

    #[test]
    fn arrastrar_con_el_lapiz_produce_un_trazo_terminado() {
        let mut a = Anotador::nuevo(1);
        match arrastrar(&mut a, p(0.0, 0.0), p(100.0, 50.0)) {
            EfectoAnotador::Terminado(e) => {
                assert!(matches!(e.figura, Figura::Lapiz { .. }));
                assert_eq!(e.grosor, GROSOR_POR_DEFECTO);
            }
            otro => panic!("se esperaba un trazo terminado, llego {otro:?}"),
        }
    }

    #[test]
    fn un_clic_con_el_lapiz_deja_un_punto_de_tinta() {
        // Como un rotulador de verdad: tocar el papel mancha.
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Lapiz));
        a.procesar(EventoAnotador::Pulsar(p(10.0, 10.0)));
        let efecto = a.procesar(EventoAnotador::Soltar(p(11.0, 10.0)));
        assert!(matches!(efecto, EfectoAnotador::Terminado(_)));
    }

    #[test]
    fn un_clic_con_el_rectangulo_no_deja_nada() {
        // Caso negativo del anterior: un rectangulo de cero por cero es
        // basura invisible que luego estorba al seleccionar.
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Rectangulo));
        a.procesar(EventoAnotador::Pulsar(p(10.0, 10.0)));
        let efecto = a.procesar(EventoAnotador::Soltar(p(11.0, 10.0)));
        assert!(
            matches!(efecto, EfectoAnotador::Repintar),
            "un clic no puede crear un rectangulo vacio, llego {efecto:?}"
        );
    }

    #[test]
    fn mientras_se_arrastra_hay_previsualizacion_y_no_elemento() {
        // Si `Mover` devolviera Terminado, cada movimiento del raton anadiria
        // un elemento a la escena y un solo trazo dejaria cien.
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Elipse));
        a.procesar(EventoAnotador::Pulsar(p(0.0, 0.0)));
        for i in 1..10 {
            let e = a.procesar(EventoAnotador::Mover(p(i as f32 * 10.0, 20.0)));
            assert!(matches!(e, EfectoAnotador::EnCurso(_)), "llego {e:?}");
        }
        assert!(matches!(
            a.procesar(EventoAnotador::Soltar(p(100.0, 20.0))),
            EfectoAnotador::Terminado(_)
        ));
    }

    #[test]
    fn dos_figuras_seguidas_no_salen_calcadas() {
        // La semilla avanza al terminar cada elemento; si no, dos
        // rectangulos dibujados a mano saldrian con el MISMO temblor y se
        // notaria al instante.
        let mut a = Anotador::nuevo(7);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Rectangulo));
        let una = arrastrar(&mut a, p(0.0, 0.0), p(100.0, 50.0));
        let otra = arrastrar(&mut a, p(0.0, 0.0), p(100.0, 50.0));
        let semilla = |e: &EfectoAnotador| match e {
            EfectoAnotador::Terminado(x) => x.semilla,
            _ => panic!("se esperaba un elemento terminado"),
        };
        assert_ne!(semilla(&una), semilla(&otra));
    }

    #[test]
    fn cambiar_de_herramienta_a_media_raya_abandona_el_trazo() {
        // Si se terminara, saldria medio lapiz y medio rectangulo.
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::Pulsar(p(0.0, 0.0)));
        a.procesar(EventoAnotador::Mover(p(50.0, 50.0)));
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Linea));
        assert!(!a.dibujando());
        let e = a.procesar(EventoAnotador::Soltar(p(80.0, 80.0)));
        assert!(matches!(e, EfectoAnotador::Nada), "llego {e:?}");
    }

    #[test]
    fn el_primer_escape_abandona_el_trazo_y_el_segundo_sale() {
        // Salir a la primera perderia el dibujo entero por un Escape de mas.
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::Pulsar(p(0.0, 0.0)));
        a.procesar(EventoAnotador::Mover(p(50.0, 50.0)));
        assert!(matches!(
            a.procesar(EventoAnotador::Tecla(TeclaAnotador::Escape)),
            EfectoAnotador::Repintar
        ));
        assert!(matches!(
            a.procesar(EventoAnotador::Tecla(TeclaAnotador::Escape)),
            EfectoAnotador::Salir
        ));
    }

    #[test]
    fn la_rueda_cambia_el_grosor_dibujando_y_el_aumento_con_la_lupa() {
        let mut a = Anotador::nuevo(1);
        let antes = a.grosor();
        a.procesar(EventoAnotador::Rueda(120));
        assert!(a.grosor() > antes);
        assert_eq!(a.lupa(), LUPA_POR_DEFECTO, "la lupa no se toca dibujando");

        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Lupa));
        let grosor = a.grosor();
        a.procesar(EventoAnotador::Rueda(120));
        assert!(a.lupa() > LUPA_POR_DEFECTO);
        assert_eq!(a.grosor(), grosor, "con la lupa no se toca el grosor");
    }

    #[test]
    fn el_grosor_y_el_aumento_tienen_topes() {
        // Sin topes, girar la rueda un rato deja un pincel de mil pixeles o
        // uno de cero, y en ambos casos no se puede dibujar.
        let mut a = Anotador::nuevo(1);
        for _ in 0..500 {
            a.procesar(EventoAnotador::Rueda(120));
        }
        assert_eq!(a.grosor(), GROSOR_MAXIMO);
        for _ in 0..500 {
            a.procesar(EventoAnotador::Rueda(-120));
        }
        assert_eq!(a.grosor(), GROSOR_MINIMO);
    }

    #[test]
    fn el_resaltador_no_tiembla() {
        // D45: sobre texto, un resaltador rugoso lo deja peor de leer.
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Resaltador));
        match arrastrar(&mut a, p(0.0, 0.0), p(100.0, 0.0)) {
            EfectoAnotador::Terminado(e) => assert_eq!(e.rugosidad, 0.0),
            otro => panic!("llego {otro:?}"),
        }
    }

    #[test]
    fn el_borrador_pide_borrar_donde_se_pulsa_y_no_dibuja() {
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Borrador));
        let e = a.procesar(EventoAnotador::Pulsar(p(30.0, 40.0)));
        assert_eq!(e, EfectoAnotador::BorrarEn(p(30.0, 40.0)));
        assert!(!a.dibujando(), "el borrador no abre ningun gesto");
    }

    #[test]
    fn el_texto_pide_al_consumidor_que_abra_su_editor() {
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Texto));
        assert_eq!(
            a.procesar(EventoAnotador::Pulsar(p(5.0, 6.0))),
            EfectoAnotador::PedirTexto(p(5.0, 6.0))
        );
    }

    #[test]
    fn el_foco_nace_con_relleno_oscuro() {
        // D51: lo que hace foco es el oscurecido, no el borde.
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Foco));
        match arrastrar(&mut a, p(0.0, 0.0), p(200.0, 100.0)) {
            EfectoAnotador::Terminado(e) => {
                let r = e.relleno.expect("el foco necesita relleno");
                assert!(r.a > 0.3, "el oscurecido es demasiado sutil: {}", r.a);
            }
            otro => panic!("llego {otro:?}"),
        }
    }

    #[test]
    fn la_mano_y_la_lupa_no_dejan_rastro() {
        for h in [Herramienta::Mano, Herramienta::Lupa] {
            let mut a = Anotador::nuevo(1);
            a.procesar(EventoAnotador::CambiarHerramienta(h));
            let e = arrastrar(&mut a, p(0.0, 0.0), p(100.0, 100.0));
            assert!(
                !matches!(e, EfectoAnotador::Terminado(_)),
                "{h:?} no puede crear elementos, llego {e:?}"
            );
        }
    }

    #[test]
    fn soltar_sin_haber_pulsado_no_entra_en_panico() {
        // Pasa de verdad: se pulsa fuera de la ventana y se suelta dentro.
        let mut a = Anotador::nuevo(1);
        assert_eq!(
            a.procesar(EventoAnotador::Soltar(p(1.0, 1.0))),
            EfectoAnotador::Nada
        );
    }
}
