//! **Las pegatinas de comentario en la pantalla** (3-nov-2026), como logica
//! pura: donde va cada cosa y que hay bajo el raton. Es la parte comun de
//! todos los visores (lienzo, PDF, Word, hojas, imagenes, paginas...): cada
//! uno solo dice donde cae cada pegatina en su ventana y de que punto de su
//! documento es un punto de la ventana. El modelo esta en
//! `pindesk_comentarios`; el pintado y el minichat, en la app.
//!
//! Piezas:
//!
//! - **La pegatina**: un redondel del color de su tipo con su signo (`!!!`,
//!   `!!`, `!` o el bocadillo), con un piquito que baja hasta el punto donde
//!   esta pegada (el ancla). Se coge por el redondel.
//! - **El boton de comentar**, abajo a la derecha, con el numero de las
//!   abiertas. Abre **la tira** con los cuatro tipos y la lista.
//! - **El menu** del clic derecho: los cuatro tipos, resolver o reabrir,
//!   crear tarea (solo las de urgencia) y quitar.
//! - **La lista** «Comentarios» del documento, encima del boton.
//! - **El minichat**: su sitio junto a la pegatina y sus partes.
//!
//! Todo en pixeles de la VENTANA; `e` es la escala de la pantalla (1,0 =
//! 96 ppp).

use pindesk_comentarios::{Comentario, Tipo};

/// Un rectangulo en pixeles de la ventana.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Caja {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

impl Caja {
    pub fn dentro(&self, (x, y): (f32, f32)) -> bool {
        x >= self.x && x < self.x + self.ancho && y >= self.y && y < self.y + self.alto
    }

    pub fn centro(&self) -> (f32, f32) {
        (self.x + self.ancho / 2.0, self.y + self.alto / 2.0)
    }

    pub fn abajo(&self) -> f32 {
        self.y + self.alto
    }

    pub fn derecha(&self) -> f32 {
        self.x + self.ancho
    }
}

// ------------------------------------------------------------ la pegatina

/// El radio del redondel, en pixeles logicos.
pub const RADIO: f32 = 13.0;
/// Lo que mide el piquito, del borde del redondel al ancla.
pub const PIQUITO: f32 = 7.0;
/// Cuanto hay que mover el raton con el boton pulsado para que sea
/// arrastrar y no un clic.
pub const UMBRAL_ARRASTRE: f32 = 4.0;

/// El centro del redondel de una pegatina cuyo ancla cae en `ancla`.
pub fn centro(ancla: (f32, f32), e: f32) -> (f32, f32) {
    (ancla.0, ancla.1 - (RADIO + PIQUITO) * e.max(0.5))
}

/// Y al reves: donde queda el ancla si el redondel se suelta en `c`.
pub fn ancla_de_centro(c: (f32, f32), e: f32) -> (f32, f32) {
    (c.0, c.1 + (RADIO + PIQUITO) * e.max(0.5))
}

/// El signo de cada tipo: tres, dos o un admiracion; la nota lleva el
/// dibujo del bocadillo (`None`).
pub fn signo(t: Tipo) -> Option<&'static str> {
    match t {
        Tipo::Urgente => Some("!!!"),
        Tipo::Alta => Some("!!"),
        Tipo::Media => Some("!"),
        Tipo::Nota => None,
    }
}

/// Una pegatina tal como se ve: su id y donde cae su ancla.
#[derive(Debug, Clone, PartialEq)]
pub struct EnPantalla {
    pub id: String,
    pub tipo: Tipo,
    pub ancla: (f32, f32),
    pub resuelto: bool,
    /// Cuantas entradas tiene (el numerito).
    pub entradas: usize,
}

/// **La pegatina bajo el raton**: la de mas arriba (la ultima de la lista,
/// que es la que se pinta encima) cuyo redondel o piquito contiene el punto.
pub fn bajo(lista: &[EnPantalla], p: (f32, f32), e: f32) -> Option<usize> {
    let r = (RADIO + 3.0) * e.max(0.5);
    lista.iter().rposition(|x| {
        let c = centro(x.ancla, e);
        let (dx, dy) = (p.0 - c.0, p.1 - c.1);
        let en_redondel = dx * dx + dy * dy <= r * r;
        let en_piquito =
            (p.0 - x.ancla.0).abs() <= 5.0 * e && p.1 >= c.1 && p.1 <= x.ancla.1 + 2.0 * e;
        en_redondel || en_piquito
    })
}

// ------------------------------------------------- el arrastre o el clic

/// Lo que va haciendo el boton pulsado sobre una pegatina.
#[derive(Debug, Clone, PartialEq)]
pub struct Arrastre {
    pub id: String,
    /// Donde se pulso y donde esta el ancla de la pegatina en ese momento.
    pub desde: (f32, f32),
    pub ancla_al_pulsar: (f32, f32),
    /// Donde esta ahora el raton.
    pub ahora: (f32, f32),
    /// Si ya paso el umbral: desde entonces es arrastrar, no un clic.
    pub movido: bool,
}

impl Arrastre {
    pub fn nuevo(id: &str, p: (f32, f32), ancla: (f32, f32)) -> Arrastre {
        Arrastre {
            id: id.into(),
            desde: p,
            ancla_al_pulsar: ancla,
            ahora: p,
            movido: false,
        }
    }

    /// El raton se movio. Devuelve si ahora es arrastrar.
    pub fn mover(&mut self, p: (f32, f32), e: f32) -> bool {
        self.ahora = p;
        let (dx, dy) = (p.0 - self.desde.0, p.1 - self.desde.1);
        if !self.movido && dx * dx + dy * dy >= (UMBRAL_ARRASTRE * e).powi(2) {
            self.movido = true;
        }
        self.movido
    }

    /// Donde va el ancla mientras se arrastra.
    pub fn ancla(&self) -> (f32, f32) {
        (
            self.ancla_al_pulsar.0 + self.ahora.0 - self.desde.0,
            self.ancla_al_pulsar.1 + self.ahora.1 - self.desde.1,
        )
    }
}

// ------------------------------------------------------ el boton y la tira

/// Lado del boton de comentar y de cada celda de la tira.
const LADO_BOTON: f32 = 40.0;
const CELDA: f32 = 40.0;
const BORDE: f32 = 16.0;
const RELLENO: f32 = 4.0;

/// Lo que hay en la tira y el boton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinoBoton {
    /// El boton de comentar (abre y cierra la tira).
    Comentar,
    /// Un tipo de la tira: pegar una de ese tipo.
    Tipo(Tipo),
    /// La lista «Comentarios».
    Lista,
    /// Dentro de la tira, entre botones.
    Hueco,
}

/// El boton de comentar y, si esta abierta, la tira.
#[derive(Debug, Clone, PartialEq)]
pub struct Boton {
    pub boton: Caja,
    /// La tira, a la izquierda del boton.
    pub tira: Option<Caja>,
    pub celdas: Vec<(Caja, DestinoBoton)>,
}

impl Boton {
    /// Coloca el boton en una ventana de `ancho` x `alto`, abajo a la
    /// derecha. `reserva_abajo` es lo que ocupa abajo lo del propio visor
    /// (una barra de estado, unas fichas): el boton va encima.
    pub fn colocar(ancho: f32, alto: f32, e: f32, abierta: bool, reserva_abajo: f32) -> Boton {
        let e = e.max(0.5);
        let lado = LADO_BOTON * e;
        let boton = Caja {
            x: ancho - (BORDE * e + lado),
            y: alto - (BORDE * e + lado) - reserva_abajo,
            ancho: lado,
            alto: lado,
        };
        let (tira, celdas) = if abierta {
            let destinos: Vec<DestinoBoton> = Tipo::TODOS
                .iter()
                .map(|t| DestinoBoton::Tipo(*t))
                .chain(std::iter::once(DestinoBoton::Lista))
                .collect();
            let celda = CELDA * e;
            let relleno = RELLENO * e;
            let ancho_tira = destinos.len() as f32 * celda + 2.0 * relleno;
            let tira = Caja {
                x: boton.x - 8.0 * e - ancho_tira,
                y: boton.y + (lado - (celda + 2.0 * relleno)) / 2.0,
                ancho: ancho_tira,
                alto: celda + 2.0 * relleno,
            };
            let celdas = destinos
                .into_iter()
                .enumerate()
                .map(|(i, d)| {
                    (
                        Caja {
                            x: tira.x + relleno + i as f32 * celda,
                            y: tira.y + relleno,
                            ancho: celda,
                            alto: celda,
                        },
                        d,
                    )
                })
                .collect();
            (Some(tira), celdas)
        } else {
            (None, Vec::new())
        };
        Boton {
            boton,
            tira,
            celdas,
        }
    }

    /// Que hay bajo el punto: `None` si nada del boton (le toca al visor).
    pub fn destino(&self, p: (f32, f32)) -> Option<DestinoBoton> {
        if self.boton.dentro(p) {
            return Some(DestinoBoton::Comentar);
        }
        if let Some((_, d)) = self.celdas.iter().find(|(c, _)| c.dentro(p)) {
            return Some(*d);
        }
        self.tira
            .filter(|t| t.dentro(p))
            .map(|_| DestinoBoton::Hueco)
    }
}

// ------------------------------------------------------------- el menu

/// Lo que se puede hacer con una pegatina desde su menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcion {
    Tipo(Tipo),
    Abrir,
    Resolver,
    Reabrir,
    CrearTarea,
    Quitar,
}

/// Las opciones del menu de una pegatina, en su orden.
pub fn opciones(c: &Comentario) -> Vec<Opcion> {
    let mut v: Vec<Opcion> = vec![Opcion::Abrir];
    v.extend(Tipo::TODOS.iter().map(|t| Opcion::Tipo(*t)));
    v.push(if c.resuelto {
        Opcion::Reabrir
    } else {
        Opcion::Resolver
    });
    if pindesk_comentarios::lista::se_puede_crear_tarea(c) {
        v.push(Opcion::CrearTarea);
    }
    v.push(Opcion::Quitar);
    v
}

const FILA_MENU: f32 = 32.0;
const ANCHO_MENU: f32 = 210.0;

/// El menu ya colocado.
#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    pub id: String,
    pub caja: Caja,
    pub filas: Vec<(Caja, Opcion)>,
}

impl Menu {
    /// Junto al punto del clic, sin salirse de la ventana.
    pub fn colocar(
        id: &str,
        opciones: &[Opcion],
        p: (f32, f32),
        ventana: (f32, f32),
        e: f32,
    ) -> Menu {
        let e = e.max(0.5);
        let (w, h) = (
            ANCHO_MENU * e,
            opciones.len() as f32 * FILA_MENU * e + 8.0 * e,
        );
        let x = if p.0 + w > ventana.0 {
            (p.0 - w).max(0.0)
        } else {
            p.0
        };
        let y = if p.1 + h > ventana.1 {
            (p.1 - h).max(0.0)
        } else {
            p.1
        };
        let caja = Caja {
            x,
            y,
            ancho: w,
            alto: h,
        };
        let filas = opciones
            .iter()
            .enumerate()
            .map(|(i, o)| {
                (
                    Caja {
                        x,
                        y: y + 4.0 * e + i as f32 * FILA_MENU * e,
                        ancho: w,
                        alto: FILA_MENU * e,
                    },
                    *o,
                )
            })
            .collect();
        Menu {
            id: id.into(),
            caja,
            filas,
        }
    }

    pub fn opcion_en(&self, p: (f32, f32)) -> Option<Opcion> {
        self.filas
            .iter()
            .find(|(c, _)| c.dentro(p))
            .map(|(_, o)| *o)
    }
}

// ------------------------------------------------------------- la lista

const ANCHO_LISTA: f32 = 320.0;
const FILA_LISTA: f32 = 52.0;
const CABECERA_LISTA: f32 = 40.0;

/// La lista «Comentarios», encima del boton.
#[derive(Debug, Clone, PartialEq)]
pub struct Lista {
    pub caja: Caja,
    pub cabecera: Caja,
    pub cerrar: Caja,
    /// Las filas que se ven (indice en la lista y su caja).
    pub filas: Vec<(usize, Caja)>,
    /// Cuantas filas caben.
    pub caben: usize,
}

impl Lista {
    /// `n` filas, empezando por `desde` (lo desplazado), encima de `boton`.
    pub fn colocar(n: usize, desde: usize, boton: &Caja, ventana: (f32, f32), e: f32) -> Lista {
        let e = e.max(0.5);
        let ancho = (ANCHO_LISTA * e)
            .min(ventana.0 - 2.0 * BORDE * e)
            .max(120.0 * e);
        let alto_libre = (boton.y - 8.0 * e - BORDE * e).max(CABECERA_LISTA * e + FILA_LISTA * e);
        let caben =
            (((alto_libre - CABECERA_LISTA * e) / (FILA_LISTA * e)).floor() as usize).max(1);
        let visibles = n.saturating_sub(desde).min(caben).max(1);
        let alto = CABECERA_LISTA * e + visibles as f32 * FILA_LISTA * e;
        let caja = Caja {
            x: (boton.derecha() - ancho).max(0.0),
            y: boton.y - 8.0 * e - alto,
            ancho,
            alto,
        };
        let cabecera = Caja {
            alto: CABECERA_LISTA * e,
            ..caja
        };
        let cerrar = Caja {
            x: caja.derecha() - 36.0 * e,
            y: caja.y + 4.0 * e,
            ancho: 32.0 * e,
            alto: 32.0 * e,
        };
        let filas = (desde..n)
            .take(caben)
            .enumerate()
            .map(|(k, i)| {
                (
                    i,
                    Caja {
                        x: caja.x,
                        y: cabecera.abajo() + k as f32 * FILA_LISTA * e,
                        ancho: caja.ancho,
                        alto: FILA_LISTA * e,
                    },
                )
            })
            .collect();
        Lista {
            caja,
            cabecera,
            cerrar,
            filas,
            caben,
        }
    }

    pub fn fila_en(&self, p: (f32, f32)) -> Option<usize> {
        self.filas
            .iter()
            .find(|(_, c)| c.dentro(p))
            .map(|(i, _)| *i)
    }
}

// ------------------------------------------------------------ el minichat

/// Lo que mide el minichat, en pixeles logicos.
pub const ANCHO_MINICHAT: f32 = 380.0;
pub const ALTO_MINICHAT: f32 = 460.0;

/// **Donde se abre el minichat**: a la derecha de la pegatina si cabe en el
/// monitor; si no, a su izquierda; y siempre dentro del area de trabajo
/// (`monitor`: x, y, ancho, alto, en pixeles del escritorio). `pegatina` es
/// el centro del redondel en el escritorio.
pub fn sitio_del_minichat(pegatina: (f32, f32), monitor: Caja, e: f32) -> Caja {
    let e = e.max(0.5);
    let (w, h) = (ANCHO_MINICHAT * e, ALTO_MINICHAT * e);
    let aire = (RADIO + 10.0) * e;
    let x = if pegatina.0 + aire + w <= monitor.derecha() {
        pegatina.0 + aire
    } else {
        pegatina.0 - aire - w
    };
    let y = pegatina.1 - 40.0 * e;
    Caja {
        x: x.clamp(monitor.x, (monitor.derecha() - w).max(monitor.x)),
        y: y.clamp(monitor.y, (monitor.abajo() - h).max(monitor.y)),
        ancho: w,
        alto: h,
    }
}

/// Los botones del minichat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotonChat {
    /// Resolver o reabrir.
    Resolver,
    /// Crear la tarea (solo urgencias).
    Tarea,
    Cerrar,
    /// Adjuntar un fichero (el cuadro de abrir de Windows).
    Adjuntar,
    /// Pegar del portapapeles (una imagen, un fichero copiado).
    Pegar,
    /// Un atajo a un elemento del lienzo o a un mensaje del chat.
    Atajo,
    /// Un lienzo propio, en blanco.
    Lienzo,
    Enviar,
}

/// Las partes del minichat, en pixeles de su ventana.
#[derive(Debug, Clone, PartialEq)]
pub struct MiniChat {
    pub cabecera: Caja,
    /// El redondel del tipo, en la cabecera.
    pub pegatina: Caja,
    /// Donde va el hilo (con su desplazamiento).
    pub hilo: Caja,
    /// La barra de abajo: los botones y la caja de escribir.
    pub barra: Caja,
    pub caja_texto: Caja,
    pub botones: Vec<(Caja, BotonChat)>,
}

impl MiniChat {
    /// `alto_texto` es lo que pide la caja de escribir (crece con los
    /// renglones, hasta un tope). `con_tarea` dice si va el boton de tarea.
    pub fn colocar(ancho: f32, alto: f32, e: f32, alto_texto: f32, con_tarea: bool) -> MiniChat {
        let e = e.max(0.5);
        let b = 32.0 * e;
        let cab = 48.0 * e;
        let cabecera = Caja {
            x: 0.0,
            y: 0.0,
            ancho,
            alto: cab,
        };
        let pegatina = Caja {
            x: 12.0 * e,
            y: (cab - 28.0 * e) / 2.0,
            ancho: 28.0 * e,
            alto: 28.0 * e,
        };
        let mut botones = Vec::new();
        let mut x = ancho - 8.0 * e - b;
        let y = (cab - b) / 2.0;
        for bt in [BotonChat::Cerrar, BotonChat::Resolver]
            .into_iter()
            .chain(con_tarea.then_some(BotonChat::Tarea))
        {
            botones.push((
                Caja {
                    x,
                    y,
                    ancho: b,
                    alto: b,
                },
                bt,
            ));
            x -= b + 4.0 * e;
        }
        let alto_texto = alto_texto.clamp(24.0 * e, 120.0 * e);
        let alto_barra = alto_texto + 16.0 * e + b + 6.0 * e;
        let barra = Caja {
            x: 0.0,
            y: alto - alto_barra,
            ancho,
            alto: alto_barra,
        };
        let enviar = Caja {
            x: ancho - 8.0 * e - b,
            y: barra.y + 8.0 * e + (alto_texto - b).max(0.0) / 2.0,
            ancho: b,
            alto: b,
        };
        let caja_texto = Caja {
            x: 10.0 * e,
            y: barra.y + 8.0 * e,
            ancho: enviar.x - 18.0 * e,
            alto: alto_texto,
        };
        botones.push((enviar, BotonChat::Enviar));
        let fila = barra.y + 8.0 * e + alto_texto + 6.0 * e;
        for (i, bt) in [
            BotonChat::Adjuntar,
            BotonChat::Pegar,
            BotonChat::Atajo,
            BotonChat::Lienzo,
        ]
        .into_iter()
        .enumerate()
        {
            botones.push((
                Caja {
                    x: 8.0 * e + i as f32 * (b + 4.0 * e),
                    y: fila,
                    ancho: b,
                    alto: b,
                },
                bt,
            ));
        }
        let hilo = Caja {
            x: 0.0,
            y: cab,
            ancho,
            alto: (barra.y - cab).max(0.0),
        };
        MiniChat {
            cabecera,
            pegatina,
            hilo,
            barra,
            caja_texto,
            botones,
        }
    }

    pub fn boton_en(&self, p: (f32, f32)) -> Option<BotonChat> {
        self.botones
            .iter()
            .find(|(c, _)| c.dentro(p))
            .map(|(_, b)| *b)
    }
}

/// **El desplazamiento del hilo**: con `total` de alto de contenido en un
/// hueco de `hueco`, lo mas que se puede bajar. El hilo se abre abajo del
/// todo (lo ultimo escrito), como un chat.
pub fn tope_del_hilo(total: f32, hueco: f32) -> f32 {
    (total - hueco).max(0.0)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pindesk_comentarios::{Ancla, Pegatinas, Quien};

    fn en(id: &str, x: f32, y: f32) -> EnPantalla {
        EnPantalla {
            id: id.into(),
            tipo: Tipo::Alta,
            ancla: (x, y),
            resuelto: false,
            entradas: 0,
        }
    }

    #[test]
    fn la_pegatina_se_coge_por_su_redondel_y_gana_la_de_encima() {
        let l = [en("a", 100.0, 100.0), en("b", 110.0, 100.0)];
        let c = centro((100.0, 100.0), 1.0);
        assert_eq!(c, (100.0, 80.0));
        assert_eq!(ancla_de_centro(c, 1.0), (100.0, 100.0));
        // En el solape, la de encima (la ultima).
        assert_eq!(bajo(&l, (105.0, 80.0), 1.0), Some(1));
        assert_eq!(bajo(&l, (90.0, 80.0), 1.0), Some(0));
        // El piquito tambien.
        assert_eq!(bajo(&l[..1], (100.0, 99.0), 1.0), Some(0));
        // Caso negativo: lejos, nada; y a escala 2 el redondel crece.
        assert_eq!(bajo(&l, (100.0, 140.0), 1.0), None);
        assert_eq!(bajo(&l[..1], (100.0, 40.0), 2.0), Some(0));
    }

    #[test]
    fn arrastrar_pide_pasar_el_umbral() {
        let mut a = Arrastre::nuevo("a", (10.0, 10.0), (10.0, 30.0));
        assert!(!a.mover((12.0, 11.0), 1.0));
        assert!(a.mover((20.0, 10.0), 1.0));
        assert_eq!(a.ancla(), (20.0, 30.0));
        // Ya movido, sigue siendo arrastre aunque vuelva.
        assert!(a.mover((10.0, 10.0), 1.0));
    }

    #[test]
    fn el_boton_y_la_tira() {
        let b = Boton::colocar(800.0, 600.0, 1.0, false, 0.0);
        assert_eq!(
            b.boton,
            Caja {
                x: 744.0,
                y: 544.0,
                ancho: 40.0,
                alto: 40.0
            }
        );
        assert_eq!(b.destino((760.0, 560.0)), Some(DestinoBoton::Comentar));
        assert_eq!(b.destino((400.0, 300.0)), None);
        let b = Boton::colocar(800.0, 600.0, 1.0, true, 30.0);
        assert_eq!(b.celdas.len(), 5);
        let (c, d) = b.celdas[0];
        assert_eq!(d, DestinoBoton::Tipo(Tipo::Urgente));
        assert_eq!(
            b.destino(c.centro()),
            Some(DestinoBoton::Tipo(Tipo::Urgente))
        );
        assert_eq!(b.destino(b.celdas[4].0.centro()), Some(DestinoBoton::Lista));
        assert!(b.tira.unwrap().derecha() < b.boton.x);
        assert_eq!(b.boton.y, 514.0);
    }

    #[test]
    fn el_menu_ofrece_lo_que_toca_y_no_se_sale() {
        let mut p = Pegatinas::default();
        let q = Quien::default();
        let u = p.poner(Tipo::Urgente, Ancla::escena(0.0, 0.0), &q, 1);
        let n = p.poner(Tipo::Nota, Ancla::escena(0.0, 0.0), &q, 2);
        let o = opciones(p.get(&u).unwrap());
        assert!(o.contains(&Opcion::CrearTarea) && o.contains(&Opcion::Resolver));
        let o = opciones(p.get(&n).unwrap());
        assert!(!o.contains(&Opcion::CrearTarea));
        p.resolver(&n, true, 3);
        assert!(opciones(p.get(&n).unwrap()).contains(&Opcion::Reabrir));
        let m = Menu::colocar(&u, &o, (790.0, 590.0), (800.0, 600.0), 1.0);
        assert!(m.caja.derecha() <= 800.0 && m.caja.abajo() <= 600.0);
        assert_eq!(m.opcion_en(m.filas[0].0.centro()), Some(Opcion::Abrir));
        assert_eq!(m.opcion_en((0.0, 0.0)), None);
    }

    #[test]
    fn la_lista_cabe_encima_del_boton() {
        let b = Boton::colocar(800.0, 600.0, 1.0, false, 0.0);
        let l = Lista::colocar(30, 0, &b.boton, (800.0, 600.0), 1.0);
        assert!(l.caja.y >= 0.0 && l.caja.abajo() <= b.boton.y);
        assert_eq!(l.filas.len(), l.caben);
        assert_eq!(l.fila_en(l.filas[2].1.centro()), Some(2));
        let l = Lista::colocar(30, 28, &b.boton, (800.0, 600.0), 1.0);
        assert_eq!(
            l.filas.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
            [28, 29]
        );
        // Sin ninguna, una fila vacia para el «no hay».
        let l = Lista::colocar(0, 0, &b.boton, (800.0, 600.0), 1.0);
        assert!(l.filas.is_empty() && l.caja.alto > l.cabecera.alto);
    }

    #[test]
    fn el_minichat_se_abre_junto_a_la_pegatina_dentro_del_monitor() {
        let m = Caja {
            x: 0.0,
            y: 0.0,
            ancho: 1920.0,
            alto: 1040.0,
        };
        let c = sitio_del_minichat((100.0, 100.0), m, 1.0);
        assert_eq!((c.x, c.y), (123.0, 60.0));
        // Pegada al borde derecho: a la izquierda.
        let c = sitio_del_minichat((1900.0, 1000.0), m, 1.0);
        assert!(c.derecha() <= 1900.0 && c.abajo() <= 1040.0);
        let d = MiniChat::colocar(380.0, 460.0, 1.0, 24.0, true);
        assert!(d.hilo.abajo() <= d.barra.y + 0.01);
        assert!(d.boton_en(d.caja_texto.centro()).is_none());
        for b in [
            BotonChat::Enviar,
            BotonChat::Adjuntar,
            BotonChat::Atajo,
            BotonChat::Tarea,
        ] {
            let caja = d.botones.iter().find(|(_, x)| *x == b).unwrap().0;
            assert_eq!(d.boton_en(caja.centro()), Some(b));
        }
        let sin = MiniChat::colocar(380.0, 460.0, 1.0, 500.0, false);
        assert!(sin.botones.iter().all(|(_, b)| *b != BotonChat::Tarea));
        assert!(sin.caja_texto.alto <= 120.0);
        assert_eq!(tope_del_hilo(100.0, 300.0), 0.0);
        assert_eq!(tope_del_hilo(500.0, 300.0), 200.0);
    }
}
