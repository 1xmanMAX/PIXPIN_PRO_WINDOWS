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
    /// Confirma el texto en curso (D57).
    Enter,
    /// Borra el ultimo caracter del texto en curso.
    Retroceso,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventoAnotador {
    Pulsar(Punto2),
    Mover(Punto2),
    Soltar(Punto2),
    /// Positivo hacia arriba, como manda Windows.
    Rueda(i32),
    Tecla(TeclaAnotador),
    /// Un caracter escrito, ya compuesto (el IME entrega el resultado).
    Caracter(char),
    CambiarHerramienta(Herramienta),
    CambiarColor(ColorRgba),
}

#[derive(Debug, Clone, PartialEq)]
pub enum EfectoAnotador {
    Nada,
    /// Cambio algo que se ve pero no la escena (la lupa, el grosor).
    Repintar,
    /// Con la mano: mira que hay bajo el punto y dilo con `poner_seleccion`.
    /// El anotador no ve la escena (vive en otra capa), asi que pregunta.
    SeleccionarEn(Punto2),
    /// Arrastrando lo seleccionado: desplazalo esto mas. Es el paso de un
    /// fotograma, no el total.
    MoverSeleccion {
        dx: f32,
        dy: f32,
    },
    /// Se solto tras arrastrar lo seleccionado: apunta ESTE total como un
    /// solo paso deshacible.
    MovimientoTerminado {
        dx: f32,
        dy: f32,
    },
    /// Alt + arrastrar: haz una copia de lo seleccionado y selecciona la
    /// copia; el arrastre siguiente movera la copia y dejara el original.
    DuplicarSeleccion,
    /// Suprimir con algo seleccionado.
    BorrarSeleccion,
    /// Previsualizacion mientras se arrastra: NO se anade a la escena.
    EnCurso(Box<Elemento>),
    /// Terminado: el consumidor lo anade a la escena.
    Terminado(Box<Elemento>),
    BorrarEn(Punto2),
    Deshacer,
    Rehacer,
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
    /// El texto que se esta escribiendo (D57). No esta en la escena hasta
    /// que se confirma: asi Escape lo tira sin tocar el deshacer.
    texto: Option<TextoEnCurso>,
    /// Lo seleccionado con la mano. El id lo pone el consumidor, que es
    /// quien tiene la escena; aqui solo se sabe si hay algo o no.
    seleccion: Option<u64>,
    /// Mayusculas: restringe angulos y proporciones. Alt: duplica al
    /// arrastrar. Los pone el consumidor antes de cada evento de puntero.
    shift: bool,
    alt: bool,
}

/// Pasos de angulo a los que se enganchan lineas y flechas con mayusculas:
/// doce por media vuelta, o sea cada 15 grados. Salen la horizontal, la
/// vertical y las diagonales, que es lo que se busca al senalar algo.
const PASOS_DE_ANGULO: f32 = 12.0;

/// El punto final ya restringido por la tecla de mayusculas: las lineas y
/// flechas se enganchan a angulos redondos, y los rectangulos y elipses
/// salen cuadrados y circulos. Lo demas se queda como esta.
fn restringir(inicio: Punto2, fin: Punto2, herramienta: Herramienta) -> Punto2 {
    let (dx, dy) = (fin.x - inicio.x, fin.y - inicio.y);
    match herramienta {
        Herramienta::Linea | Herramienta::Flecha => {
            let radio = (dx * dx + dy * dy).sqrt();
            if radio == 0.0 {
                return fin;
            }
            let paso = std::f32::consts::PI / PASOS_DE_ANGULO;
            let angulo = (dy.atan2(dx) / paso).round() * paso;
            Punto2 {
                x: inicio.x + radio * angulo.cos(),
                y: inicio.y + radio * angulo.sin(),
            }
        }
        Herramienta::Rectangulo | Herramienta::Elipse | Herramienta::Foco => {
            // El lado manda el eje mas largo, y se conserva hacia donde se
            // arrastraba: si no, la figura saltaria al cuadrante de enfrente.
            let lado = dx.abs().max(dy.abs());
            Punto2 {
                x: inicio.x + lado.copysign(if dx == 0.0 { 1.0 } else { dx }),
                y: inicio.y + lado.copysign(if dy == 0.0 { 1.0 } else { dy }),
            }
        }
        _ => fin,
    }
}

#[derive(Debug, Clone)]
struct Gesto {
    inicio: Punto2,
    puntos: Vec<Punto2>,
    /// Si ya se paso del umbral: un clic y un arrastre no son lo mismo.
    arrastrando: bool,
    /// El ultimo punto entregado, para dar el desplazamiento de este paso y
    /// no el acumulado, al arrastrar lo seleccionado.
    ultimo: Punto2,
    /// Con Alt ya se hizo la copia: solo una por arrastre.
    duplicado: bool,
}

#[derive(Debug, Clone)]
struct TextoEnCurso {
    origen: Punto2,
    contenido: String,
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
            texto: None,
            seleccion: None,
            shift: false,
            alt: false,
        }
    }

    /// Que modificadores estan pulsados. Lo dice el consumidor antes de cada
    /// evento de puntero: el anotador no lee el teclado.
    pub fn poner_modificadores(&mut self, shift: bool, alt: bool) {
        self.shift = shift;
        self.alt = alt;
    }

    /// La respuesta a `SeleccionarEn`: que elemento quedo seleccionado, o
    /// `None` si se pulso en vacio.
    pub fn poner_seleccion(&mut self, id: Option<u64>) {
        self.seleccion = id;
    }

    pub fn seleccion(&self) -> Option<u64> {
        self.seleccion
    }

    /// Donde se esta escribiendo, si se esta escribiendo: para colocar la
    /// ventana de composicion del IME al lado.
    pub fn editando_texto(&self) -> Option<Punto2> {
        self.texto.as_ref().map(|t| t.origen)
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
                // se terminara, saldria medio lapiz y medio rectangulo. El
                // texto, en cambio, se CONFIRMA: perder lo escrito por
                // pulsar el lapiz seria un fallo de datos.
                self.gesto = None;
                let confirmado = self.confirmar_texto();
                // Al coger una herramienta de dibujo se suelta lo
                // seleccionado: si no, Suprimir borraria algo elegido hace
                // tres trazos en vez de deshacer.
                if h != Herramienta::Mano {
                    self.seleccion = None;
                }
                self.herramienta = h;
                match confirmado {
                    EfectoAnotador::Terminado(e) => EfectoAnotador::Terminado(e),
                    _ => EfectoAnotador::Repintar,
                }
            }

            EventoAnotador::Caracter(c) => {
                // Los de control (Enter, Retroceso, Escape) llegan tambien
                // por WM_CHAR y ya tienen su tecla: aqui no son texto.
                if c.is_control() {
                    return EfectoAnotador::Nada;
                }
                let Some(t) = self.texto.as_mut() else {
                    return EfectoAnotador::Nada;
                };
                t.contenido.push(c);
                let t = t.clone();
                EfectoAnotador::EnCurso(Box::new(self.elemento_texto(&t, true)))
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
                TeclaAnotador::Escape if self.texto.is_some() => {
                    // Escape mientras se escribe tira el texto, no la
                    // sesion: es lo que hace cualquier editor.
                    self.texto = None;
                    EfectoAnotador::Repintar
                }
                TeclaAnotador::Escape if self.gesto.is_some() => {
                    // El primer Escape abandona el trazo en curso; el
                    // segundo sale. Asi no se pierde el dibujo por un
                    // Escape de mas.
                    self.gesto = None;
                    EfectoAnotador::Repintar
                }
                TeclaAnotador::Escape => EfectoAnotador::Salir,
                TeclaAnotador::Enter => self.confirmar_texto(),
                TeclaAnotador::Retroceso => match self.texto.as_mut() {
                    Some(t) => {
                        t.contenido.pop();
                        let t = t.clone();
                        EfectoAnotador::EnCurso(Box::new(self.elemento_texto(&t, true)))
                    }
                    None => EfectoAnotador::Nada,
                },
                TeclaAnotador::Deshacer => EfectoAnotador::Deshacer,
                TeclaAnotador::Rehacer => EfectoAnotador::Rehacer,
                // Suprimir con algo elegido lo borra; sin nada elegido
                // vuelve a ser el deshacer de siempre.
                TeclaAnotador::Suprimir => match self.seleccion {
                    Some(_) => EfectoAnotador::BorrarSeleccion,
                    None => EfectoAnotador::Deshacer,
                },
            },

            EventoAnotador::Pulsar(p) => {
                match self.herramienta {
                    Herramienta::Borrador => return EfectoAnotador::BorrarEn(p),
                    Herramienta::Texto => {
                        // Escribiendo, un clic confirma lo escrito; si no,
                        // abre un texto nuevo donde se pulso (D57).
                        if self.texto.is_some() {
                            return self.confirmar_texto();
                        }
                        let t = TextoEnCurso {
                            origen: p,
                            contenido: String::new(),
                        };
                        let previa = self.elemento_texto(&t, true);
                        self.texto = Some(t);
                        return EfectoAnotador::EnCurso(Box::new(previa));
                    }
                    // La lupa no dibuja ni selecciona: es una vista (D52).
                    Herramienta::Lupa => return EfectoAnotador::Nada,
                    // La mano selecciona lo que haya debajo. Quien mira la
                    // escena es el consumidor; aqui se abre el gesto para
                    // poder arrastrar despues.
                    Herramienta::Mano => {
                        self.gesto = Some(Gesto {
                            inicio: p,
                            puntos: vec![p],
                            arrastrando: false,
                            ultimo: p,
                            duplicado: false,
                        });
                        return EfectoAnotador::SeleccionarEn(p);
                    }
                    _ => {}
                }
                self.gesto = Some(Gesto {
                    inicio: p,
                    puntos: vec![p],
                    arrastrando: false,
                    ultimo: p,
                    duplicado: false,
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
                // Con la mano, arrastrar mueve lo seleccionado en vez de
                // dibujar. Alt hace primero una copia y mueve la copia.
                if self.herramienta == Herramienta::Mano {
                    if !g.arrastrando || self.seleccion.is_none() {
                        return EfectoAnotador::Nada;
                    }
                    if self.alt && !g.duplicado {
                        g.duplicado = true;
                        // El punto no avanza: el desplazamiento de este paso
                        // se lo lleva la copia en el evento siguiente.
                        return EfectoAnotador::DuplicarSeleccion;
                    }
                    let (dx, dy) = (p.x - g.ultimo.x, p.y - g.ultimo.y);
                    g.ultimo = p;
                    return EfectoAnotador::MoverSeleccion { dx, dy };
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
                if p.distancia(g.inicio) > UMBRAL_ARRASTRE {
                    g.arrastrando = true;
                }
                // La mano no deja elemento: cierra el arrastre diciendo
                // cuanto se movio en total, que es lo que se deshace.
                if self.herramienta == Herramienta::Mano {
                    let (inicio, arrastrado) = (g.inicio, g.arrastrando);
                    self.gesto = None;
                    if !arrastrado || self.seleccion.is_none() {
                        return EfectoAnotador::Nada;
                    }
                    return EfectoAnotador::MovimientoTerminado {
                        dx: p.x - inicio.x,
                        dy: p.y - inicio.y,
                    };
                }
                g.puntos.push(p);
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

    /// Tamano del texto: crece con el grosor, que es lo que la rueda cambia.
    fn tam_texto(&self) -> f32 {
        (self.grosor * 5.0).clamp(14.0, 120.0)
    }

    /// El elemento de un texto en curso o confirmado. Con `cursor`, lleva
    /// una barra al final para que se vea donde se escribe.
    fn elemento_texto(&self, t: &TextoEnCurso, cursor: bool) -> Elemento {
        let tam = self.tam_texto();
        let mut texto = t.contenido.clone();
        if cursor {
            texto.push('|');
        }
        // Ancho estimado, no medido: el motor no tiene DirectWrite. Sirve
        // para el hit-test y para que el consumidor no parta la linea.
        let ancho = (t.contenido.chars().count().max(1) as f32) * tam * 0.6;
        Elemento {
            id: 0,
            figura: Figura::Texto {
                texto,
                tam,
                familia: "Segoe UI".into(),
            },
            x: t.origen.x,
            y: t.origen.y,
            ancho,
            alto: tam * 1.3,
            angulo: 0.0,
            trazo: self.color,
            relleno: None,
            grosor: self.grosor,
            estilo: EstiloTrazo::Solido,
            rugosidad: 0.0,
            opacidad: 1.0,
            semilla: self.semilla,
            version: 0,
            borrado: false,
        }
    }

    /// Confirma el texto en curso: `Terminado` si hay algo, `Repintar` si
    /// estaba vacio (un texto vacio es basura invisible que estorba al
    /// seleccionar), `Nada` si no se estaba escribiendo.
    fn confirmar_texto(&mut self) -> EfectoAnotador {
        let Some(t) = self.texto.take() else {
            return EfectoAnotador::Nada;
        };
        if t.contenido.is_empty() {
            return EfectoAnotador::Repintar;
        }
        let e = self.elemento_texto(&t, false);
        self.semilla = (self.semilla.wrapping_mul(48271) & 0x7FFF_FFFF).max(1);
        EfectoAnotador::Terminado(Box::new(e))
    }

    /// El elemento que corresponde al gesto actual. `None` si todavia no hay
    /// nada que ensenar.
    fn construir(&self, _terminado: bool) -> Option<Elemento> {
        let g = self.gesto.as_ref()?;
        if !self.herramienta.deja_rastro() {
            return None;
        }
        let ultimo = *g.puntos.last()?;
        let ultimo = if self.shift {
            restringir(g.inicio, ultimo, self.herramienta)
        } else {
            ultimo
        };
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
            Herramienta::Rectangulo => Figura::Rectangulo,
            Herramienta::Foco => Figura::Foco { elipse: false },
            Herramienta::Elipse => Figura::Elipse,
            _ => return None,
        };

        // En el foco el "relleno" es el color del velo que oscurece TODO
        // menos su hueco (D51); el motor lo traduce a una orden de velo.
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

    /// Un anotador con la mano puesta y algo ya seleccionado, como quedaria
    /// tras pulsar sobre un elemento.
    fn con_mano_y_seleccion() -> Anotador {
        let mut a = Anotador::nuevo(7);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Mano));
        a
    }

    #[test]
    fn la_mano_pregunta_que_hay_debajo_y_luego_arrastra_lo_elegido() {
        // Antes la mano no hacia nada: estaba en la paleta y no se podia
        // ni seleccionar ni mover lo dibujado.
        let mut a = con_mano_y_seleccion();
        assert_eq!(
            a.procesar(EventoAnotador::Pulsar(p(10.0, 10.0))),
            EfectoAnotador::SeleccionarEn(p(10.0, 10.0))
        );
        // El consumidor mira la escena y contesta.
        a.poner_seleccion(Some(42));
        assert_eq!(a.seleccion(), Some(42));

        // Cada paso da SU desplazamiento, no el acumulado.
        assert_eq!(
            a.procesar(EventoAnotador::Mover(p(30.0, 10.0))),
            EfectoAnotador::MoverSeleccion { dx: 20.0, dy: 0.0 }
        );
        assert_eq!(
            a.procesar(EventoAnotador::Mover(p(35.0, 15.0))),
            EfectoAnotador::MoverSeleccion { dx: 5.0, dy: 5.0 }
        );
        // Y al soltar se cierra con el total, que es lo que se deshace.
        assert_eq!(
            a.procesar(EventoAnotador::Soltar(p(35.0, 15.0))),
            EfectoAnotador::MovimientoTerminado { dx: 25.0, dy: 5.0 }
        );
    }

    #[test]
    fn un_clic_en_vacio_con_la_mano_no_mueve_nada() {
        // Caso negativo: sin seleccion, arrastrar con la mano no debe
        // producir movimientos huerfanos.
        let mut a = con_mano_y_seleccion();
        a.procesar(EventoAnotador::Pulsar(p(10.0, 10.0)));
        a.poner_seleccion(None);
        assert_eq!(
            a.procesar(EventoAnotador::Mover(p(60.0, 60.0))),
            EfectoAnotador::Nada
        );
        assert_eq!(
            a.procesar(EventoAnotador::Soltar(p(60.0, 60.0))),
            EfectoAnotador::Nada
        );
    }

    #[test]
    fn alt_duplica_una_sola_vez_por_arrastre() {
        let mut a = con_mano_y_seleccion();
        a.poner_modificadores(false, true);
        a.procesar(EventoAnotador::Pulsar(p(10.0, 10.0)));
        a.poner_seleccion(Some(1));
        assert_eq!(
            a.procesar(EventoAnotador::Mover(p(40.0, 10.0))),
            EfectoAnotador::DuplicarSeleccion
        );
        // El consumidor selecciona la copia y a partir de ahi se mueve ella.
        a.poner_seleccion(Some(2));
        assert_eq!(
            a.procesar(EventoAnotador::Mover(p(50.0, 10.0))),
            EfectoAnotador::MoverSeleccion { dx: 40.0, dy: 0.0 }
        );
        assert!(matches!(
            a.procesar(EventoAnotador::Mover(p(60.0, 10.0))),
            EfectoAnotador::MoverSeleccion { .. }
        ));
    }

    #[test]
    fn suprimir_borra_lo_elegido_y_si_no_hay_nada_deshace() {
        let mut a = con_mano_y_seleccion();
        assert_eq!(
            a.procesar(EventoAnotador::Tecla(TeclaAnotador::Suprimir)),
            EfectoAnotador::Deshacer,
            "sin nada elegido, Suprimir sigue siendo deshacer"
        );
        a.poner_seleccion(Some(3));
        assert_eq!(
            a.procesar(EventoAnotador::Tecla(TeclaAnotador::Suprimir)),
            EfectoAnotador::BorrarSeleccion
        );
    }

    #[test]
    fn coger_una_herramienta_de_dibujo_suelta_la_seleccion() {
        let mut a = con_mano_y_seleccion();
        a.poner_seleccion(Some(9));
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Lapiz));
        assert_eq!(a.seleccion(), None);
    }

    #[test]
    fn mayusculas_engancha_la_flecha_a_angulos_redondos() {
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Flecha));
        a.poner_modificadores(true, false);
        // Casi horizontal (5 grados) se endereza del todo.
        let ef = arrastrar(&mut a, p(0.0, 0.0), p(100.0, 8.75));
        let EfectoAnotador::Terminado(e) = ef else {
            panic!("{ef:?}");
        };
        let Figura::Flecha { puntos, .. } = &e.figura else {
            panic!("{:?}", e.figura);
        };
        assert!(
            (puntos[1].y - puntos[0].y).abs() < 0.01,
            "queda horizontal: {puntos:?}"
        );
        assert!((puntos[1].x - 100.38).abs() < 1.0, "y conserva el largo");
    }

    #[test]
    fn mayusculas_hace_cuadrado_el_rectangulo_sin_saltar_de_cuadrante() {
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Rectangulo));
        a.poner_modificadores(true, false);
        // Arrastrando hacia arriba a la izquierda: el cuadrado tiene que
        // quedarse ahi, no rebotar al otro lado del punto de partida.
        let ef = arrastrar(&mut a, p(100.0, 100.0), p(40.0, 80.0));
        let EfectoAnotador::Terminado(e) = ef else {
            panic!("{ef:?}");
        };
        assert_eq!((e.ancho, e.alto), (60.0, 60.0), "lados iguales");
        assert_eq!((e.x, e.y), (40.0, 40.0), "y sigue arriba a la izquierda");
    }

    #[test]
    fn sin_mayusculas_no_se_restringe_nada() {
        // Caso negativo del anterior: si restringiera siempre, no se podria
        // dibujar un rectangulo normal.
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Rectangulo));
        let ef = arrastrar(&mut a, p(0.0, 0.0), p(100.0, 30.0));
        let EfectoAnotador::Terminado(e) = ef else {
            panic!("{ef:?}");
        };
        assert_eq!((e.ancho, e.alto), (100.0, 30.0));
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
    fn el_clic_con_texto_abre_un_texto_en_curso_donde_se_pulso() {
        // D57: el texto se escribe in situ, en la propia maquina; no hay
        // editor aparte que abrir.
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Texto));
        let e = a.procesar(EventoAnotador::Pulsar(p(5.0, 6.0)));
        assert!(matches!(e, EfectoAnotador::EnCurso(_)), "llego {e:?}");
        assert_eq!(a.editando_texto(), Some(p(5.0, 6.0)));
        assert!(!a.dibujando(), "escribir no es un gesto de arrastre");
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
    fn el_foco_es_una_figura_propia_y_no_un_rectangulo_relleno() {
        // D51: si fuera un rectangulo con relleno oscuro oscureceria justo
        // lo que se quiere ensenar.
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Foco));
        let EfectoAnotador::Terminado(e) = arrastrar(&mut a, p(10.0, 10.0), p(60.0, 40.0)) else {
            panic!("un arrastre con el foco termina un elemento");
        };
        assert_eq!(e.figura, Figura::Foco { elipse: false });
        assert_eq!(e.relleno.map(|c| c.a), Some(0.6));
    }

    fn con_texto() -> Anotador {
        let mut a = Anotador::nuevo(1);
        a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Texto));
        a.procesar(EventoAnotador::Pulsar(p(30.0, 40.0)));
        a
    }

    #[test]
    fn escribir_y_enter_dejan_un_elemento_de_texto_donde_se_pulso() {
        let mut a = con_texto();
        for c in "Hola".chars() {
            let efecto = a.procesar(EventoAnotador::Caracter(c));
            assert!(
                matches!(efecto, EfectoAnotador::EnCurso(_)),
                "cada letra se ve al momento, llego {efecto:?}"
            );
        }
        let EfectoAnotador::Terminado(e) = a.procesar(EventoAnotador::Tecla(TeclaAnotador::Enter))
        else {
            panic!("Enter confirma");
        };
        assert_eq!(
            e.figura,
            Figura::Texto {
                texto: "Hola".into(),
                tam: 20.0,
                familia: "Segoe UI".into()
            }
        );
        assert_eq!((e.x, e.y), (30.0, 40.0));
        assert!(a.editando_texto().is_none());
    }

    #[test]
    fn retroceso_borra_y_un_texto_vacio_no_deja_nada() {
        // Caso negativo: un clic con Texto y Enter sin escribir no puede
        // dejar un elemento invisible que luego estorbe al seleccionar.
        let mut a = con_texto();
        a.procesar(EventoAnotador::Caracter('a'));
        a.procesar(EventoAnotador::Tecla(TeclaAnotador::Retroceso));
        assert_eq!(
            a.procesar(EventoAnotador::Tecla(TeclaAnotador::Enter)),
            EfectoAnotador::Repintar
        );
    }

    #[test]
    fn escape_cancela_el_texto_sin_salir() {
        let mut a = con_texto();
        a.procesar(EventoAnotador::Caracter('x'));
        assert_eq!(
            a.procesar(EventoAnotador::Tecla(TeclaAnotador::Escape)),
            EfectoAnotador::Repintar
        );
        assert!(a.editando_texto().is_none());
        assert_eq!(
            a.procesar(EventoAnotador::Tecla(TeclaAnotador::Escape)),
            EfectoAnotador::Salir
        );
    }

    #[test]
    fn cambiar_de_herramienta_confirma_el_texto_escrito() {
        // Perder lo escrito por pulsar el lapiz seria un fallo de datos.
        let mut a = con_texto();
        a.procesar(EventoAnotador::Caracter('y'));
        assert!(matches!(
            a.procesar(EventoAnotador::CambiarHerramienta(Herramienta::Lapiz)),
            EfectoAnotador::Terminado(_)
        ));
        assert_eq!(a.herramienta(), Herramienta::Lapiz);
    }

    #[test]
    fn los_caracteres_de_control_no_entran_en_el_texto() {
        let mut a = con_texto();
        assert_eq!(
            a.procesar(EventoAnotador::Caracter('\r')),
            EfectoAnotador::Nada
        );
        assert_eq!(
            a.procesar(EventoAnotador::Caracter('\u{8}')),
            EfectoAnotador::Nada
        );
    }

    #[test]
    fn la_previsualizacion_del_texto_lleva_cursor_y_el_final_no() {
        let mut a = con_texto();
        let EfectoAnotador::EnCurso(e) = a.procesar(EventoAnotador::Caracter('a')) else {
            panic!("se esperaba previsualizacion");
        };
        assert!(matches!(e.figura, Figura::Texto { ref texto, .. } if texto == "a|"));
        // Un segundo clic con Texto confirma, sin cursor.
        let EfectoAnotador::Terminado(e) = a.procesar(EventoAnotador::Pulsar(p(0.0, 0.0))) else {
            panic!("el clic confirma lo escrito");
        };
        assert!(matches!(e.figura, Figura::Texto { ref texto, .. } if texto == "a"));
    }

    #[test]
    fn escribir_sin_haber_pulsado_no_hace_nada() {
        // Caso negativo: las teclas que llegan con otra herramienta no
        // pueden inventar un texto de la nada.
        let mut a = Anotador::nuevo(1);
        assert_eq!(
            a.procesar(EventoAnotador::Caracter('z')),
            EfectoAnotador::Nada
        );
        assert_eq!(
            a.procesar(EventoAnotador::Tecla(TeclaAnotador::Enter)),
            EfectoAnotador::Nada
        );
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
