//! La interaccion del pin como maquina pura (D23).
//!
//! Todo el pin se agarra y se mueve como un objeto fisico; SOLO las cuatro
//! esquinas redimensionan, siempre en proporcion. La ventana traduce
//! mensajes Win32 a EventoPin y ejecuta los EfectoPin; asi el
//! comportamiento entero se prueba sin abrir una ventana, igual que el
//! overlay de S1-B2.

use pixpin_geom::{Esquina, Punto, Rect, esquina_en, redimension_proporcional};

/// Zona de esquina en pixeles LOGICOS (D23); la escala la aplica el estado.
pub const ZONA_ESQUINA_LOGICA: u32 = 12;
/// Lado minimo del pin en pixeles logicos (spec 3.2).
pub const MINIMO_LOGICO: u32 = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventoPin {
    BotonPulsado(Punto),
    RatonMovido(Punto),
    BotonSoltado,
    DobleClic,
    Escape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EfectoPin {
    Nada,
    /// Recolocar la ventana ya, sin repintar contenido.
    Mover(Rect),
    /// Cambiar tamano de ventana y repintar.
    Redimensionar(Rect),
    /// Doble clic: 100% <-> ajustado (lo resuelve el dueno, que sabe el
    /// tamano nativo de la imagen).
    AlternarTamano,
    Cerrar,
    /// El gesto acabo: persistir la posicion (escritura-al-soltar).
    GestoTerminado(Rect),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gesto {
    Ninguno,
    Moviendo { agarre: Punto, origen: Rect },
    Redimensionando { esquina: Esquina, origen: Rect },
}

#[derive(Debug)]
pub struct EstadoPin {
    rect: Rect,
    escala_por_cien: u32,
    gesto: Gesto,
    /// La ficha de archivo solo estira a lo ancho (spec 4.1): su alto lo
    /// manda el contenido, no el raton.
    solo_ancho: bool,
    /// Sin redimension alguna: la ficha y la nota tienen el tamano que les
    /// da su contenido, y estirarlas solo dejaba el texto donde estaba con
    /// un hueco creciendo alrededor (lo encontro el usuario).
    fijo: bool,
}

impl EstadoPin {
    pub fn nuevo(rect: Rect, escala_por_cien: u32) -> Self {
        Self {
            rect,
            escala_por_cien: escala_por_cien.max(100),
            gesto: Gesto::Ninguno,
            solo_ancho: false,
            fijo: false,
        }
    }

    /// Como `nuevo`, pero para contenidos de alto fijo (la ficha).
    pub fn nuevo_solo_ancho(rect: Rect, escala_por_cien: u32) -> Self {
        Self {
            solo_ancho: true,
            ..Self::nuevo(rect, escala_por_cien)
        }
    }

    /// Como `nuevo`, pero sin redimension: solo se mueve.
    pub fn nuevo_fijo(rect: Rect, escala_por_cien: u32) -> Self {
        Self {
            fijo: true,
            ..Self::nuevo(rect, escala_por_cien)
        }
    }

    pub fn es_fijo(&self) -> bool {
        self.fijo
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// Coloca el rect sin tocar el resto del estado. Reconstruir el
    /// `EstadoPin` entero seria mas corto pero perderia `solo_ancho`, y una
    /// ficha empezaria a estirarse en vertical tras el primer doble clic.
    pub fn poner_rect(&mut self, rect: Rect) {
        self.rect = rect;
        self.gesto = Gesto::Ninguno;
    }

    fn zona(&self) -> u32 {
        ZONA_ESQUINA_LOGICA * self.escala_por_cien / 100
    }

    fn minimo(&self) -> u32 {
        MINIMO_LOGICO * self.escala_por_cien / 100
    }

    /// Para el cursor diagonal: el unico feedback de las esquinas (D23).
    pub fn sobre_esquina(&self, p: Punto) -> bool {
        !self.fijo && esquina_en(self.rect, p, self.zona()).is_some()
    }

    pub fn procesar(&mut self, evento: EventoPin) -> EfectoPin {
        match evento {
            EventoPin::Escape => EfectoPin::Cerrar,
            EventoPin::DobleClic => EfectoPin::AlternarTamano,
            EventoPin::BotonPulsado(p) => {
                let esquina = if self.fijo {
                    None
                } else {
                    esquina_en(self.rect, p, self.zona())
                };
                self.gesto = match esquina {
                    Some(esquina) => Gesto::Redimensionando {
                        esquina,
                        origen: self.rect,
                    },
                    None => Gesto::Moviendo {
                        agarre: p,
                        origen: self.rect,
                    },
                };
                EfectoPin::Nada
            }
            EventoPin::RatonMovido(p) => match self.gesto {
                Gesto::Ninguno => EfectoPin::Nada,
                Gesto::Moviendo { agarre, origen } => {
                    self.rect = Rect {
                        x: origen.x + (p.x - agarre.x),
                        y: origen.y + (p.y - agarre.y),
                        ancho: origen.ancho,
                        alto: origen.alto,
                    };
                    EfectoPin::Mover(self.rect)
                }
                Gesto::Redimensionando { esquina, origen } => {
                    let propuesto = redimension_proporcional(origen, esquina, p, self.minimo());
                    self.rect = if self.solo_ancho {
                        // El alto y la fila superior se quedan como estaban:
                        // una ficha estirada en vertical dejaria el icono y
                        // los dos textos flotando en un hueco vacio.
                        Rect {
                            x: propuesto.x,
                            y: origen.y,
                            ancho: propuesto.ancho,
                            alto: origen.alto,
                        }
                    } else {
                        propuesto
                    };
                    EfectoPin::Redimensionar(self.rect)
                }
            },
            EventoPin::BotonSoltado => {
                if self.gesto == Gesto::Ninguno {
                    return EfectoPin::Nada;
                }
                self.gesto = Gesto::Ninguno;
                EfectoPin::GestoTerminado(self.rect)
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_geom::{Punto, Rect};

    fn pin() -> EstadoPin {
        EstadoPin::nuevo(
            Rect {
                x: 100,
                y: 100,
                ancho: 400,
                alto: 300,
            },
            100,
        )
    }

    #[test]
    fn agarrar_por_el_centro_mueve_como_un_objeto() {
        let mut e = pin();
        e.procesar(EventoPin::BotonPulsado(Punto { x: 300, y: 250 }));
        let ef = e.procesar(EventoPin::RatonMovido(Punto { x: 330, y: 270 }));
        assert_eq!(
            ef,
            EfectoPin::Mover(Rect {
                x: 130,
                y: 120,
                ancho: 400,
                alto: 300
            }),
            "se desplaza lo que el raton, sin cambiar tamano"
        );
        // Soltar persiste: la senal de escritura-al-soltar.
        assert_eq!(
            e.procesar(EventoPin::BotonSoltado),
            EfectoPin::GestoTerminado(Rect {
                x: 130,
                y: 120,
                ancho: 400,
                alto: 300
            })
        );
    }

    #[test]
    fn agarrar_por_un_borde_tambien_mueve() {
        // Caso negativo de D23: el borde NO es un tirador; solo las esquinas.
        let mut e = pin();
        e.procesar(EventoPin::BotonPulsado(Punto { x: 300, y: 102 }));
        let ef = e.procesar(EventoPin::RatonMovido(Punto { x: 300, y: 132 }));
        assert!(
            matches!(ef, EfectoPin::Mover(_)),
            "el borde mueve, no redimensiona: {ef:?}"
        );
    }

    #[test]
    fn la_esquina_redimensiona_en_proporcion() {
        let mut e = pin();
        // Sureste del rect (100,100,400x300): (499, 399) esta en la zona.
        e.procesar(EventoPin::BotonPulsado(Punto { x: 497, y: 397 }));
        let ef = e.procesar(EventoPin::RatonMovido(Punto { x: 700, y: 500 }));
        let EfectoPin::Redimensionar(r) = ef else {
            panic!("la esquina debe redimensionar: {ef:?}");
        };
        assert_eq!((r.x, r.y), (100, 100), "ancla noroeste clavada");
        let prop = r.ancho as f64 / r.alto as f64;
        assert!((prop - 4.0 / 3.0).abs() < 0.02, "proporcion rota: {prop}");
        assert!(r.ancho > 400);
    }

    #[test]
    fn escape_cierra_y_doble_clic_alterna() {
        let mut e = pin();
        assert_eq!(e.procesar(EventoPin::Escape), EfectoPin::Cerrar);
        assert_eq!(e.procesar(EventoPin::DobleClic), EfectoPin::AlternarTamano);
    }

    #[test]
    fn mover_sin_boton_no_hace_nada() {
        // Caso negativo: el hover puro no arrastra.
        let mut e = pin();
        assert_eq!(
            e.procesar(EventoPin::RatonMovido(Punto { x: 300, y: 250 })),
            EfectoPin::Nada
        );
    }

    #[test]
    fn sobre_esquina_guia_el_cursor_con_la_escala() {
        // A 200% la zona logica de 12 son 24 fisicos.
        let e = EstadoPin::nuevo(
            Rect {
                x: 0,
                y: 0,
                ancho: 400,
                alto: 300,
            },
            200,
        );
        assert!(e.sobre_esquina(Punto { x: 380, y: 280 }), "dentro de 24 px");
        assert!(!e.sobre_esquina(Punto { x: 360, y: 260 }), "fuera de 24 px");
    }

    #[test]
    fn una_ficha_estira_a_lo_ancho_y_conserva_el_alto() {
        let inicial = Rect {
            x: 100,
            y: 100,
            ancho: 280,
            alto: 72,
        };
        let mut e = EstadoPin::nuevo_solo_ancho(inicial, 100);

        // Agarrar la esquina sureste y arrastrar en diagonal.
        e.procesar(EventoPin::BotonPulsado(Punto { x: 378, y: 170 }));
        e.procesar(EventoPin::RatonMovido(Punto { x: 500, y: 400 }));

        let r = e.rect();
        assert!(r.ancho > 280, "el ancho si crece: {}", r.ancho);
        assert_eq!(r.alto, 72, "el alto de una ficha no lo manda el raton");
        assert_eq!(r.y, 100, "y la fila superior tampoco se mueve");
    }

    #[test]
    fn un_pin_normal_si_cambia_de_alto() {
        // Caso negativo del anterior: si `solo_ancho` se aplicara a todos,
        // ninguna imagen podria escalarse.
        let inicial = Rect {
            x: 100,
            y: 100,
            ancho: 280,
            alto: 72,
        };
        let mut e = EstadoPin::nuevo(inicial, 100);
        e.procesar(EventoPin::BotonPulsado(Punto { x: 378, y: 170 }));
        e.procesar(EventoPin::RatonMovido(Punto { x: 500, y: 400 }));
        assert!(e.rect().alto > 72, "una imagen si crece en alto");
    }
}
