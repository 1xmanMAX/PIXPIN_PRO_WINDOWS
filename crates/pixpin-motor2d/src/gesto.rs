//! Todo lo que pasa entre pulsar y soltar.
//!
//! En el Android son 3.482 lineas (`DrawController.kt`) porque atiende a
//! dos, tres y cuatro dedos, al lapiz y al canto de la mano. Aqui hay un
//! raton y un teclado, asi que donde el Android pregunta «hay segundo
//! dedo?», esto pregunta «esta Shift?».
//!
//! # Que toca y que devuelve (D23)
//!
//! **La escena si la toca.** Es un `Vec` de datos, se prueba sin pantalla, y
//! mantener aparte un registro de ordenes pendientes seria una segunda
//! verdad sobre el mismo dibujo.
//!
//! **Lo que devuelve es lo que la ventana tiene que hacer**: que region
//! redibujar y que cursor poner. De Direct2D no sabe nada. Por eso esto vive
//! en el motor y no en la interfaz: una prueba puede decir «pulsar aqui,
//! mover cuarenta pixeles con Shift, soltar» y comprobar el resultado exacto
//! sin abrir una ventana.
//!
//! # El orden de decision al pulsar
//!
//! Es la parte que hay que leer despacio:
//!
//! 1. **Un tirador manda sobre lo que haya debajo.** Sin esto, un tirador
//!    encima de un trazo es inalcanzable.
//! 2. **Lo ya seleccionado manda sobre lo de encima.** Sin esto, mover un
//!    grupo se convierte en seleccionar por accidente lo que estaba encima.
//! 3. Lo que haya bajo el cursor.
//! 4. Vacio: marquesina si la herramienta es la mano, dibujar si no.

use pixpin_geom::Tirador;

use crate::elemento::{Elemento, Figura};
use crate::escena::Escena;
use crate::impacto::{dentro_de, elemento_en};
use crate::seleccion::Seleccion;
use crate::tiradores::{Agarre, Tiradores};
use crate::transformar::{self, a_saltos, angulo_hacia};
use crate::vector::Punto2;

/// Puntos que se reservan de una vez para el trazo en curso.
///
/// 512 porque el trazo mas largo del fichero del movil tiene 492. Reservar
/// de una vez es lo que permite que mover el raton dibujando no asigne
/// memoria — la regla del camino caliente.
pub const PUNTOS_RESERVADOS: usize = 512;

/// Con que se dibuja. Vino de `pixpin-ui/anotador.rs` en el paso 1: quien
/// decide que hace un clic tiene que saber que herramienta hay puesta, y esa
/// decision es logica pura, no interfaz.
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
    /// Si necesita un arrastre de verdad para producir algo. El lapiz no:
    /// un clic deja un punto de tinta, que es lo que espera cualquiera que
    /// haya usado un rotulador.
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventoGesto {
    Pulsar { p: Punto2, shift: bool, alt: bool },
    Mover { p: Punto2, shift: bool, alt: bool },
    Soltar { p: Punto2 },
    Escape,
    Suprimir,
    Deshacer,
    Rehacer,
    SeleccionarTodo,
}

/// Que hay que volver a pintar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Region {
    Nada,
    /// Solo esta caja del mundo. Es lo que se usa en el camino caliente.
    Caja(f32, f32, f32, f32),
    Todo,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormaCursor {
    Flecha,
    Cruz,
    Mover,
    Texto,
    Giro,
    /// Escalar, con el tirador (para saber a que lado apunta) y el angulo
    /// del elemento (para girar esa direccion con el).
    Escalar {
        tirador: Tirador,
        angulo: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Respuesta {
    pub region: Region,
    pub cursor: FormaCursor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Estado {
    Reposo,
    Dibujando { id: u64 },
    Moviendo { anterior: Punto2 },
    Escalando { tirador: Tirador },
    Girando { anterior: f32 },
    Marquesina { origen: Punto2, hasta: Punto2 },
}

pub struct Gesto {
    estado: Estado,
    pub seleccion: Seleccion,
    pub herramienta: Herramienta,
    /// El trazo en curso. Se reserva una vez y se reutiliza con `clear()`.
    trazo: Vec<Punto2>,
}

impl Default for Gesto {
    fn default() -> Self {
        Self {
            estado: Estado::Reposo,
            seleccion: Seleccion::nueva(),
            herramienta: Herramienta::Lapiz,
            trazo: Vec::with_capacity(PUNTOS_RESERVADOS),
        }
    }
}

impl Gesto {
    pub fn nuevo() -> Self {
        Self::default()
    }

    pub fn en_reposo(&self) -> bool {
        matches!(self.estado, Estado::Reposo)
    }

    /// La marquesina en curso, para que la ventana la pinte.
    pub fn marquesina(&self) -> Option<(f32, f32, f32, f32)> {
        match self.estado {
            Estado::Marquesina { origen, hasta } => Some((origen.x, origen.y, hasta.x, hasta.y)),
            _ => None,
        }
    }

    /// Para la prueba de que el buffer se reutiliza.
    pub fn capacidad_del_trazo(&self) -> usize {
        self.trazo.capacity()
    }

    pub fn evento(&mut self, ev: EventoGesto, escena: &mut Escena, escala: f32) -> Respuesta {
        match ev {
            EventoGesto::Pulsar { p, shift, alt } => self.pulsar(p, shift, alt, escena, escala),
            EventoGesto::Mover { p, shift, alt } => self.mover(p, shift, alt, escena, escala),
            EventoGesto::Soltar { p } => self.soltar(p, escena),
            EventoGesto::Escape => {
                escena.cancelar_paso();
                self.estado = Estado::Reposo;
                self.seleccion.limpiar();
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                }
            }
            EventoGesto::Suprimir => {
                escena.abrir_paso();
                for &id in self.seleccion.ids() {
                    // `borrar_apuntando` y no `borrar`: el primero apunta el
                    // cambio en el paso, el segundo no. Con `borrar`, la
                    // prueba `suprimir_borra_lo_seleccionado_de_una_vez`
                    // falla en su segunda mitad.
                    escena.borrar_apuntando(id);
                }
                escena.cerrar_paso();
                self.seleccion.limpiar();
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                }
            }
            EventoGesto::Deshacer => {
                escena.deshacer();
                self.seleccion.limpiar();
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                }
            }
            EventoGesto::Rehacer => {
                escena.rehacer();
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                }
            }
            EventoGesto::SeleccionarTodo => {
                let vivos: Vec<u64> = escena.visibles().map(|e| e.id).collect();
                self.seleccion.poner_todos(vivos);
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                }
            }
        }
    }

    /// Los tiradores de la seleccion, si hay algo elegido.
    fn tiradores(&self, escena: &Escena, escala: f32) -> Option<Tiradores> {
        let caja = self.seleccion.caja(escena)?;
        // Con un solo elemento, el marco lleva su angulo. Con varios, la
        // caja es paralela a los ejes y cada uno conserva el suyo.
        let angulo = match self.seleccion.ids() {
            [uno] => escena.buscar(*uno).map_or(0.0, |e| e.angulo),
            _ => 0.0,
        };
        Some(Tiradores::de_caja(caja, angulo, escala))
    }

    /// El elemento nuevo que empieza esta herramienta en este punto.
    ///
    /// Los puntos se reservan de una vez, igual que el buffer del gesto: es
    /// lo que hace que mover el raton dibujando **no asigne memoria**. Si
    /// este `Vec` empezara vacio, crecer de 4 a 8 a 16... asignaria una
    /// docena de veces por trazo, en el unico camino del programa con un
    /// plazo sagrado. La tarea 15 lo comprueba contando asignaciones.
    fn nuevo_elemento(&self, p: Punto2) -> Elemento {
        let reservados = || {
            let mut v = Vec::with_capacity(PUNTOS_RESERVADOS);
            v.push(p);
            v
        };
        let figura = match self.herramienta {
            Herramienta::Lapiz => Figura::Lapiz {
                puntos: reservados(),
                presiones: Vec::new(),
            },
            Herramienta::Resaltador => Figura::Resaltador {
                puntos: reservados(),
            },
            Herramienta::Linea => Figura::Linea { puntos: vec![p, p] },
            Herramienta::Flecha => Figura::Flecha {
                puntos: vec![p, p],
                punta_inicio: false,
                punta_fin: true,
            },
            Herramienta::Elipse => Figura::Elipse,
            Herramienta::Foco => Figura::Foco { elipse: false },
            // Rectangulo y todo lo demas que deje rastro.
            _ => Figura::Rectangulo,
        };
        Elemento {
            id: 0, // lo pone `Escena::anadir`
            figura,
            x: p.x,
            y: p.y,
            ancho: 0.0,
            alto: 0.0,
            angulo: 0.0,
            trazo: crate::elemento::ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            grosor: 3.0,
            estilo: crate::elemento::EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
        }
    }

    /// Que cursor toca en este punto, estando en reposo.
    fn cursor_en(&self, p: Punto2, escena: &Escena, escala: f32) -> FormaCursor {
        if let Some(ts) = self.tiradores(escena, escala) {
            match ts.en(p, escala) {
                // Van los dos: cual, para saber a que lado apunta; y el
                // angulo, para girar esa direccion con el elemento.
                Some(Agarre::Tamano(t)) => {
                    return FormaCursor::Escalar {
                        tirador: t,
                        angulo: ts.angulo,
                    };
                }
                Some(Agarre::Giro) => return FormaCursor::Giro,
                None => {}
            }
        }
        match self.herramienta {
            Herramienta::Mano => {
                if elemento_en(&escena.elementos, p).is_some() {
                    FormaCursor::Mover
                } else {
                    FormaCursor::Flecha
                }
            }
            Herramienta::Texto => FormaCursor::Texto,
            _ => FormaCursor::Cruz,
        }
    }

    fn pulsar(
        &mut self,
        p: Punto2,
        shift: bool,
        _alt: bool,
        escena: &mut Escena,
        escala: f32,
    ) -> Respuesta {
        // Un segundo pulsar sin su soltar: la ventana puede recibirlo si el
        // usuario solto fuera. Se ignora en vez de perder el gesto entero.
        if !self.en_reposo() {
            return Respuesta {
                region: Region::Nada,
                cursor: self.cursor_en(p, escena, escala),
            };
        }
        escena.abrir_paso();

        // 1. Un tirador manda sobre lo que haya debajo.
        if let Some(ts) = self.tiradores(escena, escala) {
            match ts.en(p, escala) {
                Some(Agarre::Tamano(t)) => {
                    self.estado = Estado::Escalando { tirador: t };
                    return Respuesta {
                        region: Region::Nada,
                        cursor: FormaCursor::Escalar {
                            tirador: t,
                            angulo: ts.angulo,
                        },
                    };
                }
                Some(Agarre::Giro) => {
                    self.estado = Estado::Girando {
                        anterior: angulo_hacia(ts.centro, p),
                    };
                    return Respuesta {
                        region: Region::Nada,
                        cursor: FormaCursor::Giro,
                    };
                }
                None => {}
            }
        }

        // 2. Lo ya seleccionado manda sobre lo de encima.
        let sobre_lo_elegido = self
            .seleccion
            .ids()
            .iter()
            .filter_map(|id| escena.buscar(*id))
            .any(|e| crate::impacto::toca(e, p));
        if sobre_lo_elegido && !shift {
            self.estado = Estado::Moviendo { anterior: p };
            return Respuesta {
                region: Region::Nada,
                cursor: FormaCursor::Mover,
            };
        }

        // 3. Lo que haya bajo el cursor.
        if let Some(id) = elemento_en(&escena.elementos, p) {
            if shift {
                self.seleccion.alternar(id);
            } else {
                self.seleccion.poner(id);
            }
            self.estado = Estado::Moviendo { anterior: p };
            return Respuesta {
                region: Region::Todo,
                cursor: FormaCursor::Mover,
            };
        }

        // 4. Vacio.
        if self.herramienta == Herramienta::Mano {
            if !shift {
                self.seleccion.limpiar();
            }
            self.estado = Estado::Marquesina {
                origen: p,
                hasta: p,
            };
            return Respuesta {
                region: Region::Todo,
                cursor: FormaCursor::Flecha,
            };
        }
        // El texto no entra en esta entrega (ver arriba): con la herramienta
        // de texto puesta, un clic en vacio no hace nada en vez de dejar un
        // rectangulo, que es lo que pasaria al caer en el `_` de
        // `nuevo_elemento`.
        if self.herramienta.deja_rastro() && self.herramienta != Herramienta::Texto {
            self.seleccion.limpiar();
            self.trazo.clear();
            self.trazo.push(p);
            let id = escena.anadir(self.nuevo_elemento(p));
            self.estado = Estado::Dibujando { id };
            return Respuesta {
                region: Region::Todo,
                cursor: FormaCursor::Cruz,
            };
        }
        Respuesta {
            region: Region::Nada,
            cursor: FormaCursor::Cruz,
        }
    }

    fn mover(
        &mut self,
        p: Punto2,
        shift: bool,
        alt: bool,
        escena: &mut Escena,
        escala: f32,
    ) -> Respuesta {
        match self.estado {
            Estado::Reposo => Respuesta {
                region: Region::Nada,
                cursor: self.cursor_en(p, escena, escala),
            },

            Estado::Dibujando { id } => {
                // El camino caliente. Ni una asignacion: el buffer ya esta
                // reservado, y se ensucia el tramo y no la pantalla.
                let anterior = *self.trazo.last().unwrap_or(&p);
                if self.trazo.len() < PUNTOS_RESERVADOS {
                    self.trazo.push(p);
                }
                let grosor = escena.buscar(id).map_or(3.0, |e| e.grosor);
                if let Some(e) = escena.buscar_mut(id) {
                    match &mut e.figura {
                        Figura::Lapiz { puntos, .. } | Figura::Resaltador { puntos } => {
                            puntos.push(p);
                        }
                        Figura::Linea { puntos } | Figura::Flecha { puntos, .. } => {
                            // Linea y flecha son dos puntos: el segundo sigue
                            // al cursor en vez de acumularse.
                            if let Some(ultimo) = puntos.last_mut() {
                                *ultimo = p;
                            }
                        }
                        _ => {
                            // Las figuras de caja crecen desde donde se pulso.
                            let o = *self.trazo.first().unwrap_or(&p);
                            e.x = o.x.min(p.x);
                            e.y = o.y.min(p.y);
                            e.ancho = (p.x - o.x).abs();
                            e.alto = (p.y - o.y).abs();
                        }
                    }
                    e.tocar();
                }
                let m = grosor / 2.0 + 1.0;
                Respuesta {
                    region: Region::Caja(
                        anterior.x.min(p.x) - m,
                        anterior.y.min(p.y) - m,
                        anterior.x.max(p.x) + m,
                        anterior.y.max(p.y) + m,
                    ),
                    cursor: FormaCursor::Cruz,
                }
            }

            Estado::Moviendo { anterior } => {
                let (dx, dy) = (p.x - anterior.x, p.y - anterior.y);
                for &id in self.seleccion.ids() {
                    // Sin esto el paso queda vacio y no hay nada que
                    // deshacer. Es el error mas facil de cometer aqui.
                    escena.apuntar_edicion(id);
                    if let Some(e) = escena.buscar_mut(id) {
                        e.mover(dx, dy);
                    }
                }
                self.estado = Estado::Moviendo { anterior: p };
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Mover,
                }
            }

            Estado::Escalando { tirador } => {
                for &id in self.seleccion.ids() {
                    escena.apuntar_edicion(id);
                    if let Some(e) = escena.buscar_mut(id) {
                        transformar::escalar(e, tirador, p, shift, alt);
                    }
                }
                let angulo = self.tiradores(escena, escala).map_or(0.0, |t| t.angulo);
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Escalar { tirador, angulo },
                }
            }

            Estado::Girando { anterior } => {
                let Some(centro) = self.seleccion.centro(escena) else {
                    return Respuesta {
                        region: Region::Nada,
                        cursor: FormaCursor::Giro,
                    };
                };
                let ahora = angulo_hacia(centro, p);
                let ahora = if shift { a_saltos(ahora) } else { ahora };
                let delta = ahora - anterior;
                for &id in self.seleccion.ids() {
                    escena.apuntar_edicion(id);
                    if let Some(e) = escena.buscar_mut(id) {
                        transformar::girar(e, centro, delta);
                    }
                }
                self.estado = Estado::Girando { anterior: ahora };
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Giro,
                }
            }

            Estado::Marquesina { origen, .. } => {
                self.estado = Estado::Marquesina { origen, hasta: p };
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                }
            }
        }
    }

    fn soltar(&mut self, p: Punto2, escena: &mut Escena) -> Respuesta {
        if let Estado::Marquesina { origen, .. } = self.estado {
            let caja = (origen.x, origen.y, p.x, p.y);
            let cogidos = dentro_de(&escena.elementos, caja);
            self.seleccion.poner_todos(cogidos);
        }
        // Un paso sin cambios no entra en el historial, asi que hacer clic
        // sin arrastrar no consume un Ctrl+Z. De eso se encarga cerrar_paso.
        escena.cerrar_paso();
        self.estado = Estado::Reposo;
        Respuesta {
            region: Region::Todo,
            cursor: FormaCursor::Flecha,
        }
    }
}

/// Hacia donde apunta un tirador, en un elemento girado `angulo`.
///
/// Cero es hacia arriba y crece en el sentido de las agujas, igual que
/// `transformar::angulo_hacia`. La ventana usa esto para elegir entre las
/// cuatro flechas que trae Windows.
pub fn direccion_del_tirador(t: Tirador, angulo: f32) -> f32 {
    use std::f32::consts::PI;
    let base = match t {
        Tirador::NorteBorde => 0.0,
        Tirador::NoresteEsquina => PI / 4.0,
        Tirador::EsteBorde => PI / 2.0,
        Tirador::SuresteEsquina => 3.0 * PI / 4.0,
        Tirador::SurBorde => PI,
        Tirador::SuroesteEsquina => 5.0 * PI / 4.0,
        Tirador::OesteBorde => 3.0 * PI / 2.0,
        Tirador::NoroesteEsquina => 7.0 * PI / 4.0,
    };
    base + angulo
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};
    use pixpin_geom::Tirador;

    fn rect(x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho,
            alto,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
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

    fn pulsar(p: Punto2) -> EventoGesto {
        EventoGesto::Pulsar {
            p,
            shift: false,
            alt: false,
        }
    }
    fn mover(p: Punto2) -> EventoGesto {
        EventoGesto::Mover {
            p,
            shift: false,
            alt: false,
        }
    }

    /// Un arrastre entero: pulsar, mover y soltar.
    fn arrastrar(g: &mut Gesto, e: &mut Escena, de: Punto2, a: Punto2) {
        g.evento(pulsar(de), e, 1.0);
        g.evento(mover(a), e, 1.0);
        g.evento(EventoGesto::Soltar { p: a }, e, 1.0);
    }

    #[test]
    fn un_arrastre_entero_deja_exactamente_un_paso_de_deshacer() {
        // El invariante que hace util el historial de la tarea 1.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        arrastrar(
            &mut g,
            &mut escena,
            Punto2::nuevo(50.0, 50.0),
            Punto2::nuevo(150.0, 50.0),
        );
        assert_eq!(escena.buscar(id).unwrap().x, 100.0, "se movio");

        assert!(escena.deshacer());
        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "de una vez");
    }

    #[test]
    fn escape_a_mitad_de_un_arrastre_lo_cancela() {
        // Sale gratis: el paso ya guarda el estado anterior, asi que
        // cancelar es aplicarlo y tirar el paso.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(500.0, 500.0)), &mut escena, 1.0);
        g.evento(EventoGesto::Escape, &mut escena, 1.0);

        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "volvio a su sitio");
        assert!(g.en_reposo());
    }

    #[test]
    fn un_tirador_manda_sobre_el_elemento_que_haya_debajo() {
        // Sin esta prioridad, un tirador encima de otro elemento es
        // inalcanzable: el clic cae en el de debajo.
        let mut escena = Escena::nueva();
        let grande = escena.anadir(rect(0.0, 0.0, 200.0, 200.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(grande);

        // La esquina sureste del grande, que cae DENTRO del propio grande.
        let ts = Tiradores::de_elemento(escena.buscar(grande).unwrap(), 1.0);
        let se = ts
            .tamano
            .iter()
            .find(|(c, _)| *c == Tirador::SuresteEsquina)
            .unwrap()
            .1;

        g.evento(pulsar(se), &mut escena, 1.0);
        g.evento(
            mover(Punto2::nuevo(se.x + 100.0, se.y + 100.0)),
            &mut escena,
            1.0,
        );
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(se.x + 100.0, se.y + 100.0),
            },
            &mut escena,
            1.0,
        );

        let e = escena.buscar(grande).unwrap();
        assert!(e.ancho > 250.0, "escalo en vez de moverse: {}", e.ancho);
        assert_eq!(e.x, 0.0, "y no se movio");
    }

    #[test]
    fn lo_ya_seleccionado_manda_sobre_lo_que_haya_encima() {
        // Sin esta regla, mover un grupo se convierte en seleccionar por
        // accidente lo que estaba encima.
        let mut escena = Escena::nueva();
        let abajo = escena.anadir(rect(0.0, 0.0, 200.0, 200.0));
        let encima = escena.anadir(rect(50.0, 50.0, 50.0, 50.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(abajo);

        // Pulsar donde estan los dos: gana el ya elegido.
        arrastrar(
            &mut g,
            &mut escena,
            Punto2::nuevo(75.0, 75.0),
            Punto2::nuevo(85.0, 75.0),
        );

        assert_eq!(g.seleccion.ids(), &[abajo], "sigue el de antes");
        assert_eq!(
            escena.buscar(abajo).unwrap().x,
            10.0,
            "se movio el de antes"
        );
        assert_eq!(
            escena.buscar(encima).unwrap().x,
            50.0,
            "el de encima, quieto"
        );
    }

    #[test]
    fn arrastrar_en_vacio_con_la_mano_hace_marquesina() {
        let mut escena = Escena::nueva();
        let dentro = escena.anadir(rect(20.0, 20.0, 30.0, 30.0));
        let fuera = escena.anadir(rect(500.0, 500.0, 30.0, 30.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        assert!(g.marquesina().is_some(), "hay marquesina en curso");
        g.evento(mover(Punto2::nuevo(200.0, 200.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(200.0, 200.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(g.seleccion.ids(), &[dentro]);
        assert!(!g.seleccion.contiene(fuera));
        assert!(g.marquesina().is_none(), "y se acabo al soltar");
    }

    #[test]
    fn shift_mas_clic_anade_a_la_seleccion() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 50.0, 50.0));
        let b = escena.anadir(rect(100.0, 0.0, 50.0, 50.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;

        g.evento(pulsar(Punto2::nuevo(25.0, 25.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(25.0, 25.0),
            },
            &mut escena,
            1.0,
        );
        g.evento(
            EventoGesto::Pulsar {
                p: Punto2::nuevo(125.0, 25.0),
                shift: true,
                alt: false,
            },
            &mut escena,
            1.0,
        );
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(125.0, 25.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(g.seleccion.ids(), &[a, b]);
    }

    #[test]
    fn dibujar_un_trazo_lo_anade_y_lo_deja_deshacible() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        for i in 1..20 {
            g.evento(mover(Punto2::nuevo(i as f32 * 5.0, 0.0)), &mut escena, 1.0);
        }
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(95.0, 0.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(escena.cuantos_visibles(), 1);
        assert!(escena.deshacer());
        assert_eq!(escena.cuantos_visibles(), 0, "un trazo, un Ctrl+Z");
    }

    #[test]
    fn dibujando_se_ensucia_el_tramo_y_no_la_pantalla() {
        // En 1080p es la diferencia entre dos millones de pixeles por
        // fotograma y unos cientos.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        let r = g.evento(mover(Punto2::nuevo(10.0, 0.0)), &mut escena, 1.0);

        let Region::Caja(x0, y0, x1, y1) = r.region else {
            panic!("tiene que ser una caja, es {:?}", r.region);
        };
        assert!(x1 - x0 < 40.0 && y1 - y0 < 40.0, "el tramo, no la pantalla");
    }

    #[test]
    fn dibujar_no_asigna_memoria_por_cada_aviso_del_raton() {
        // El buffer del trazo en curso se reserva de una vez y se reutiliza
        // entre trazos con clear(). La prueba de verdad —contar
        // asignaciones— es la tarea 15; aqui se comprueba lo que se puede
        // observar desde dentro.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        for i in 1..400 {
            g.evento(mover(Punto2::nuevo(i as f32, 0.0)), &mut escena, 1.0);
        }
        assert!(
            g.capacidad_del_trazo() >= PUNTOS_RESERVADOS,
            "se reservo de una vez"
        );
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(400.0, 0.0),
            },
            &mut escena,
            1.0,
        );

        let tras_el_primero = g.capacidad_del_trazo();
        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        assert_eq!(
            g.capacidad_del_trazo(),
            tras_el_primero,
            "el segundo trazo reutiliza el buffer del primero"
        );
    }

    #[test]
    fn suprimir_borra_lo_seleccionado_de_una_vez() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = escena.anadir(rect(20.0, 0.0, 10.0, 10.0));
        let mut g = Gesto::nuevo();
        g.seleccion.poner_todos([a, b]);

        g.evento(EventoGesto::Suprimir, &mut escena, 1.0);
        assert_eq!(escena.cuantos_visibles(), 0);

        assert!(escena.deshacer());
        assert_eq!(escena.cuantos_visibles(), 2, "los dos vuelven de una vez");
    }

    #[test]
    fn seleccionar_todo_no_coge_los_borrados() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = escena.anadir(rect(20.0, 0.0, 10.0, 10.0));
        escena.borrar(b);
        let mut g = Gesto::nuevo();

        g.evento(EventoGesto::SeleccionarTodo, &mut escena, 1.0);
        assert_eq!(g.seleccion.ids(), &[a]);
    }

    #[test]
    fn pulsar_dos_veces_sin_soltar_no_pierde_el_gesto() {
        // La ventana puede recibir un WM_LBUTTONDOWN sin su WM_LBUTTONUP si
        // el usuario suelta fuera. Perder el gesto entero por eso seria
        // peor que ignorar el segundo pulsar.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(150.0, 50.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(150.0, 50.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(escena.buscar(id).unwrap().x, 100.0);
        assert!(escena.deshacer());
        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "sigue siendo un paso");
    }

    #[test]
    fn el_cursor_de_escalar_va_girado_con_el_elemento() {
        // En una figura a 45 grados, el tirador de la esquina ensena la
        // flecha que de verdad apunta hacia donde va a crecer.
        use std::f32::consts::FRAC_PI_4;
        let mut escena = Escena::nueva();
        let mut e = rect(0.0, 0.0, 100.0, 100.0);
        e.angulo = FRAC_PI_4;
        let id = escena.anadir(e);
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        let ts = Tiradores::de_elemento(escena.buscar(id).unwrap(), 1.0);
        let se = ts
            .tamano
            .iter()
            .find(|(c, _)| *c == Tirador::SuresteEsquina)
            .unwrap()
            .1;
        let r = g.evento(mover(se), &mut escena, 1.0);

        let FormaCursor::Escalar { tirador, angulo } = r.cursor else {
            panic!("sobre un tirador toca cursor de escalar, es {:?}", r.cursor);
        };
        assert_eq!(
            tirador,
            Tirador::SuresteEsquina,
            "cual, para saber la direccion"
        );
        assert!(
            (angulo - FRAC_PI_4).abs() < 1e-3,
            "y el angulo del elemento, para girarla: {angulo}"
        );
    }

    #[test]
    fn la_direccion_de_un_tirador_gira_con_el_elemento() {
        // Sin girar, la esquina sureste apunta a 135 grados (abajo y a la
        // derecha). Girado un cuarto de vuelta, apunta a 225.
        use std::f32::consts::{FRAC_PI_2, PI};
        let recta = direccion_del_tirador(Tirador::SuresteEsquina, 0.0);
        assert!((recta - 3.0 * PI / 4.0).abs() < 1e-3, "sin girar: {recta}");

        let girada = direccion_del_tirador(Tirador::SuresteEsquina, FRAC_PI_2);
        assert!((girada - 5.0 * PI / 4.0).abs() < 1e-3, "girada: {girada}");
    }
}
