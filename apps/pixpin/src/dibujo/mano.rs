//! **La mano**: lo que hace cada clic y cada tecla con las herramientas de
//! dibujo, en cualquier anfitrion.
//!
//! Era el centro del bucle de `ventana_editor.rs` (la parte que no es de su
//! ventana) y se movio aqui tal cual el 2026-09-24: el panel y la caja,
//! escribir un texto, la tabla de atajos del motor, las letras de las
//! herramientas y de la pluma, copiar y pegar, agrupar, el lazo, el
//! cuentagotas de estilo, la goma y, al final, el `Gesto` del motor con las
//! cuatro herramientas de construir. Con esto el lector, el pin y el
//! anotador de pantalla responden EXACTAMENTE igual que el lienzo.
//!
//! **Traduce y dice que cambio; no pinta.** Cada anfitrion tiene su forma de
//! repintar —el lienzo con sus capas, su horneado y sus zonas; el lector en
//! la misma pasada que el texto— y esa decision sigue siendo suya. Por eso
//! devuelve `Atendido` en vez de repintar.
//!
//! Lo que es de un solo anfitrion no esta aqui: navegar (cada uno se mueve a
//! su manera), el universo, las marcas, los enlaces entre hojas, F11 y
//! exportar son del lienzo.

use super::permitidas::{self, Anfitrion};
use super::teclas::{
    CambioPluma, a_evento, aplicar_orden, con_modificadores, elegir_herramienta, pulsar_boton,
    tecla_a_pluma, tecla_del_motor,
};
use crate::imagenes_lienzo::ImagenesLienzo;
use pixpin_geom::{Punto, Rect};
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta, Respuesta};
use pixpin_motor2d::seleccion::OrdenEditor;
use pixpin_motor2d::vector::Punto2;
use pixpin_motor2d::Elemento;
use pixpin_shell::overlay::EventoOverlay;
use pixpin_ui::{BotonCaja, CajaHerramientas, DestinoClic};

/// Cuanto tiene que quedarse quieto el cursor dibujando a mano para que el
/// trazo se convierta en figura (el gesto de pararse). Es el numero del
/// movil, que vive en el motor: el mismo gesto a distinto ritmo en dos
/// aparatos serian dos gestos que aprender.
pub const PAUSA_FORMA: std::time::Duration =
    std::time::Duration::from_millis(pixpin_motor2d::forma_rapida::ESPERA_PARA_LA_FORMA_MS);

/// Desde donde se mira: la camara (documento→pantalla, ya con la escala
/// del monitor), la ventana en el escritorio virtual y su escala.
#[derive(Clone, Copy)]
pub struct Vista<'a> {
    pub camara: &'a Camara,
    /// La ventana en coordenadas del escritorio virtual: los eventos traen
    /// esas coordenadas y el documento empieza en su esquina.
    pub area: Rect,
    pub escala_por_cien: u32,
    /// Donde van la barra y el panel (coordenadas del escritorio). Casi
    /// siempre la ventana; en el anotador de pantalla, que cubre todos los
    /// monitores, el principal.
    pub interfaz: Rect,
}

impl Vista<'_> {
    fn origen(&self) -> Punto {
        Punto {
            x: self.area.x,
            y: self.area.y,
        }
    }

    /// Un punto de la ventana (coordenadas del escritorio) en el documento.
    pub fn al_documento(&self, p: Punto) -> Punto2 {
        self.camara.a_mundo(Punto2::nuevo(
            (p.x - self.area.x) as f32,
            (p.y - self.area.y) as f32,
        ))
    }
}

/// Que hay que repintar tras un evento. `todo` es el fotograma entero;
/// `contenido`, ademas, que cambio el DIBUJO y no solo lo que hay encima
/// (el lienzo lo necesita para decidir si puede mover el visual en vez de
/// repintar, ver A3 en `ventana_editor.rs`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Repinte {
    pub todo: bool,
    pub contenido: bool,
}

impl Repinte {
    pub const NADA: Repinte = Repinte {
        todo: false,
        contenido: false,
    };
    pub const DIBUJO: Repinte = Repinte {
        todo: true,
        contenido: true,
    };
    pub const ENCIMA: Repinte = Repinte {
        todo: true,
        contenido: false,
    };

    fn si(cambio: bool) -> Repinte {
        if cambio { Repinte::DIBUJO } else { Repinte::NADA }
    }

    pub fn algo(self) -> bool {
        self.todo || self.contenido
    }
}

/// Lo que paso al pasarle el evento al `Gesto` del motor.
pub struct Paso {
    pub r: Respuesta,
    /// Donde se pulso, si fue un pulsar (en el documento).
    pub pulsado: Option<Punto2>,
    /// Una de las cuatro de construir hizo algo al pulsar.
    pub construido: bool,
    pub en_reposo_antes: bool,
    /// Si habia algo elegido antes de este aviso (su marco se quedaria
    /// pintado).
    pub habia_eleccion: bool,
}

/// Lo que hizo la mano con un evento.
#[derive(Default)]
pub struct Atendido {
    /// El evento era suyo: el anfitrion no debe hacer nada mas con el.
    pub consumido: bool,
    pub repinte: Repinte,
    /// Se pulso «Salir» en la caja.
    pub salir: bool,
    /// Se eligio una herramienta de la caja (el universo suelta la suya).
    pub eligio: bool,
    /// Paso por el motor. Solo si `consumido`.
    pub paso: Option<Paso>,
    /// Algo que tiene que hacer el anfitrion porque la mano no sabe: abrir
    /// el selector de una imagen o el menu de las figuras.
    pub pedido: Option<Pedido>,
}

/// Lo que la mano pide al anfitrion (ver `Atendido::pedido`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pedido {
    /// Se pulso «Imagen» en la barra: elegir un fichero y ponerlo.
    Imagen,
    /// Se pulso «Figuras»: el menu, bajo el boton (coordenadas del
    /// escritorio, las de los clics).
    Figuras(Punto),
    /// Se pulso «Imprimir» (F11): el dialogo de imprimir con vista previa.
    Imprimir,
    /// Se pulso «Compartir»: la hoja de compartir, como `Ctrl+Mayus+S`.
    Compartir,
    /// Se pulso el clic a traves (solo en el anotador de pantalla viva).
    Atravesar,
}

impl Atendido {
    fn suyo(repinte: Repinte) -> Atendido {
        Atendido {
            consumido: true,
            repinte,
            ..Atendido::default()
        }
    }
}

/// El estado de la mano que no es del motor.
pub struct Mano {
    pub anfitrion: Anfitrion,
    /// Lo copiado con Ctrl+C. Vive con el anfitrion: cerrarlo se lo lleva,
    /// que es lo que espera cualquiera.
    pub portapapeles: Vec<Elemento>,
    /// La goma esta pulsada: borra lo que vaya tocando hasta soltar.
    pub borrando: bool,
    /// Forma rapida: desde cuando esta quieto el cursor dibujando a mano, y
    /// donde. Quieto `PAUSA_FORMA` convierte el trazo en figura.
    pub quieto: Option<(std::time::Instant, Punto2)>,
    /// La punta predicha del trazo en curso (ver `tinta::prediccion`).
    pub predictor: pixpin_motor2d::tinta::prediccion::Predictor,
    /// C1, apagado de fabrica: el filtro de 1 euro de `pixpin-tinta`.
    filtro: pixpin_tinta::FiltroUnEuro,
    suavizado_natural: bool,
    reloj: std::time::Instant,
    /// **La zona que se acaba de soltar** (F8), en el documento, a la espera
    /// de que el anfitrion saque su foto. La mano no sabe pintar: lo pide.
    pub zona_pedida: Option<(f32, f32, f32, f32)>,
    /// Con la Zona en la mano se esta arrastrando una copia ya sacada: la
    /// lleva la mano de siempre hasta soltar, y luego vuelve la Zona.
    zona_moviendo: bool,
    /// La lupa del lienzo (`dibujo::lupa`): la varita y apuntar lo que mira.
    pub lupa: super::lupa::ManoDeLupa,
    /// El grupo de la barra que tiene sus hermanas a la vista
    /// (`dibujo::grupos`). Vive aqui y no en la caja porque la caja se
    /// rehace (al cambiar el tamano, y en el lector con cada evento); el
    /// anfitrion la pinta con `caja.con_desplegado(mano.desplegado)`.
    pub desplegado: Option<pixpin_ui::GrupoBarra>,
    /// El ultimo pulsar fue de la caja: su soltar tambien lo es.
    pulso_en_caja: bool,
}

impl Mano {
    /// Una mano con los ajustes de tinta de la aplicacion.
    pub fn nueva(anfitrion: Anfitrion) -> Mano {
        Mano::con_tinta(anfitrion, crate::ventana_editor::ajustes_tinta_de_la_app())
    }

    pub fn con_tinta(anfitrion: Anfitrion, tinta: pixpin_store::Tinta) -> Mano {
        Mano {
            anfitrion,
            portapapeles: Vec::new(),
            borrando: false,
            quieto: None,
            predictor: pixpin_motor2d::tinta::prediccion::Predictor::nuevo(),
            filtro: pixpin_tinta::FiltroUnEuro::nuevo(mandos_de(tinta)),
            suavizado_natural: tinta.suavizado == pixpin_store::Suavizado::Natural,
            reloj: std::time::Instant::now(),
            zona_pedida: None,
            zona_moviendo: false,
            lupa: Default::default(),
            desplegado: None,
            pulso_en_caja: false,
        }
    }

    /// Los milisegundos del reloj de la mano (el del predictor y el filtro).
    pub fn ms(&self) -> f64 {
        self.reloj.elapsed().as_secs_f64() * 1000.0
    }

    /// Si la herramienta sale en este anfitrion con los ajustes de ahora.
    pub fn permitida(&self, h: Herramienta) -> bool {
        permitidas::permitida(self.anfitrion, h)
    }

    /// **El panel, la caja y el texto que se esta escribiendo.** Va antes
    /// que lo propio del anfitrion que lea teclas (F11, exportar...): un
    /// clic en la caja ELIGE y no puede llegar ademas al gesto como si fuera
    /// un trazo, y mientras se escribe, una «r» es una erre y no el
    /// resaltador.
    ///
    /// `caja` es `None` si el anfitrion no la ensena ahora (el pin la tiene
    /// en otra ventana; la pantalla pasante la esconde). `panel` dice si el
    /// anfitrion ensena el panel lateral.
    pub fn interfaz(
        &mut self,
        ev: &EventoOverlay,
        gesto: &mut Gesto,
        escena: &mut Escena,
        caja: Option<&CajaHerramientas>,
        panel: bool,
        vista: Vista<'_>,
    ) -> Atendido {
        let mut sigue = Atendido::default();
        if panel && let EventoOverlay::BotonPulsado(p) | EventoOverlay::BotonSoltado(p) = *ev {
            // Todo el camino del clic esta en `panel_dibujo::atender_clic`,
            // que es por donde las pruebas pulsan cada boton del panel.
            use crate::panel_dibujo::ClicPanel;
            match crate::panel_dibujo::atender_clic(
                p,
                matches!(ev, EventoOverlay::BotonPulsado(_)),
                gesto,
                escena,
                vista.interfaz,
                vista.escala_por_cien,
            ) {
                ClicPanel::Suyo { repintar } => return Atendido::suyo(Repinte::si(repintar)),
                ClicPanel::Fuera { cerro } => {
                    if cerro {
                        sigue.repinte = Repinte::ENCIMA;
                    }
                }
            }
        }
        // Escape cierra primero las hermanas de un grupo: es lo ultimo que
        // se abrio (el «atras por capas» del editor del movil).
        const VK_ESCAPE: u32 = 0x1B;
        if self.desplegado.is_some()
            && let EventoOverlay::Tecla { vk: VK_ESCAPE, .. } = *ev
        {
            self.desplegado = None;
            return Atendido::suyo(Repinte::ENCIMA);
        }
        if let Some(caja) = caja {
            // La caja que ve el usuario: con las hermanas del grupo abierto.
            // Si esta caja ya no lo puede abrir (lo apagaron), se olvida.
            let caja = &caja.con_desplegado(self.desplegado);
            self.desplegado = caja.desplegado();
            if let EventoOverlay::BotonPulsado(p) = *ev {
                let destino = caja.destino(p);
                // Se apunta para que el soltar sea tambien de la caja aunque
                // para entonces ya no haya caja debajo: la fila de una
                // hermana desaparece al pulsarla, y su soltar caeria al
                // lienzo como el final de un trazo que nunca empezo.
                self.pulso_en_caja = !matches!(destino, DestinoClic::Lienzo);
                // Cualquier clic cierra las hermanas: elegir una, pulsar otro
                // boton o ponerse a dibujar («dibujar cierra las hermanas»,
                // `DrawEditorActivity` del movil). El hueco de la caja no.
                let habia = self.desplegado.take();
                if habia.is_some() {
                    sigue.repinte = Repinte::ENCIMA;
                }
                match destino {
                    // **Un grupo**: coge su cara y ensena las hermanas en el
                    // mismo clic (`dibujo::grupos::pulsar`).
                    DestinoClic::Boton(BotonCaja::Grupo(g)) => {
                        let r = super::grupos::pulsar(caja, g, gesto.herramienta, habia);
                        self.desplegado = r.desplegado;
                        let mut a = Atendido::suyo(Repinte::ENCIMA);
                        if let Some(h) = r.elegir.filter(|h| self.permitida(*h)) {
                            elegir_herramienta(gesto, h);
                            a.repinte = Repinte::DIBUJO;
                            a.eligio = true;
                        }
                        return a;
                    }
                    DestinoClic::Boton(boton) => {
                        // Una herramienta que no sale no se puede elegir ni
                        // aunque su boton se colara en la caja.
                        if let BotonCaja::Elegir(h) = boton
                            && !self.permitida(h)
                        {
                            return Atendido::suyo(Repinte::NADA);
                        }
                        // La imagen, las figuras e imprimir las hace el
                        // anfitrion.
                        if matches!(
                            boton,
                            BotonCaja::Imagen
                                | BotonCaja::Figuras
                                | BotonCaja::Imprimir
                                | BotonCaja::Compartir
                                | BotonCaja::Atravesar
                        ) {
                            let mut a = Atendido::suyo(sigue.repinte);
                            if permitidas::boton_permitido(self.anfitrion, boton) {
                                // Salida de un desplegable: queda de cara.
                                super::grupos::usado(boton);
                                a.pedido = Some(match boton {
                                    BotonCaja::Imagen => Pedido::Imagen,
                                    BotonCaja::Imprimir => Pedido::Imprimir,
                                    BotonCaja::Compartir => Pedido::Compartir,
                                    BotonCaja::Atravesar => Pedido::Atravesar,
                                    _ => {
                                        // El menu de las figuras, colgado
                                        // del boton de su grupo: la fila de
                                        // la que sale se cierra ahora.
                                        let r = pixpin_ui::grupo_de_boton(boton)
                                            .and_then(|g| caja.rect_de_boton(BotonCaja::Grupo(g)))
                                            .or_else(|| caja.rect_de_boton(boton));
                                        Pedido::Figuras(r.map_or(p, |r| Punto {
                                            x: r.x,
                                            y: r.abajo(),
                                        }))
                                    }
                                });
                            }
                            return a;
                        }
                        let mut a = Atendido::suyo(Repinte::DIBUJO);
                        if !pulsar_boton(boton, gesto, escena) {
                            a.salir = true;
                        }
                        a.eligio = matches!(boton, BotonCaja::Elegir(_));
                        return a;
                    }
                    // El hueco entre botones: de la caja, pero no un boton.
                    // Las hermanas se quedan (un clic en el borde de su isla
                    // no es cerrarla).
                    DestinoClic::Caja => {
                        self.desplegado = habia;
                        return Atendido::suyo(sigue.repinte);
                    }
                    DestinoClic::Lienzo => {}
                }
            }
            if let EventoOverlay::BotonSoltado(p) = *ev
                && (std::mem::take(&mut self.pulso_en_caja)
                    || !matches!(caja.destino(p), DestinoClic::Lienzo))
            {
                return Atendido::suyo(sigue.repinte);
            }
        }
        // Escribiendo, las teclas son del texto: una «r» es una erre y no la
        // herramienta rectangulo.
        if gesto.esta_escribiendo() {
            use pixpin_motor2d::texto::TeclaTexto;
            const VK_IZQUIERDA: u32 = 0x25;
            const VK_DERECHA: u32 = 0x27;
            const VK_INICIO: u32 = 0x24;
            const VK_FIN: u32 = 0x23;
            const VK_RETROCESO: u32 = 0x08;
            const VK_SUPRIMIR: u32 = 0x2E;
            const VK_ESCAPE_TEXTO: u32 = 0x1B;
            const VK_ENTRAR: u32 = 0x0D;
            let atendido = match *ev {
                // Los mandos llegan tambien como caracter; se atienden por
                // tecla, que es donde se distinguen bien.
                EventoOverlay::Caracter(c) if c >= ' ' => gesto.escribir(c, escena),
                EventoOverlay::Tecla { vk, .. } => match vk {
                    // Con el teclado, como en Excalidraw: el texto se queda
                    // elegido y ensena su marco, ya a la medida de lo escrito.
                    VK_ESCAPE_TEXTO => gesto.cerrar_texto_con_teclado(escena),
                    VK_IZQUIERDA => gesto.tecla_de_texto(TeclaTexto::Izquierda, escena),
                    VK_DERECHA => gesto.tecla_de_texto(TeclaTexto::Derecha, escena),
                    VK_INICIO => gesto.tecla_de_texto(TeclaTexto::Inicio, escena),
                    VK_FIN => gesto.tecla_de_texto(TeclaTexto::Fin, escena),
                    VK_RETROCESO => gesto.tecla_de_texto(TeclaTexto::Retroceso, escena),
                    VK_SUPRIMIR => gesto.tecla_de_texto(TeclaTexto::Suprimir, escena),
                    VK_ENTRAR => gesto.tecla_de_texto(TeclaTexto::Entrar, escena),
                    _ => false,
                },
                _ => false,
            };
            if atendido {
                sigue.repinte = Repinte::DIBUJO;
            }
            // Las teclas se consumen aunque no hagan nada; el raton no, que
            // es como se sale a pulsar en otro sitio.
            if matches!(ev, EventoOverlay::Caracter(_) | EventoOverlay::Tecla { .. }) {
                sigue.consumido = true;
            }
        }
        sigue
    }

    /// **Las herramientas**: atajos, letras, portapapeles, lazo, cuentagotas,
    /// goma y el gesto del motor, en ese orden (el del lienzo de siempre).
    ///
    /// `imagenes` es el almacen de imagenes pegadas si el anfitrion las
    /// sabe pintar; sin el, Ctrl+V solo pega lo copiado dentro. `ancho_px`
    /// y `alto_px` son los de la ventana, para centrar una imagen pegada.
    #[allow(clippy::too_many_arguments)]
    pub fn herramienta(
        &mut self,
        ev: &EventoOverlay,
        gesto: &mut Gesto,
        escena: &mut Escena,
        imagenes: Option<&mut ImagenesLienzo>,
        vista: Vista<'_>,
        ancho_px: f32,
        alto_px: f32,
    ) -> Atendido {
        // **La tabla de atajos del motor, antes que las letras sueltas.** Va
        // primero porque sus atajos llevan modificadores y los de
        // herramienta no: dejandola detras, un `Mayus+V` se habria comido ya
        // la `V` de la pluma. La unica tecla pelada que reclama —`S` del
        // lazo, `K` del cuentagotas— no la usa ninguna de las otras dos
        // tablas, y hay una prueba que lo vigila.
        if let EventoOverlay::Tecla {
            vk,
            ctrl,
            shift,
            alt,
            ..
        } = *ev
            && let Some(tecla) = tecla_del_motor(vk)
            && let Some(orden) = pixpin_motor2d::seleccion::atajo_de(tecla, ctrl, shift, alt)
        {
            // Las dos que eligen herramienta, solo si esa herramienta sale.
            let apagada = match orden {
                OrdenEditor::Lazo => !self.permitida(Herramienta::Lazo),
                OrdenEditor::CopiarEstilo => !self.permitida(Herramienta::CopiarEstilo),
                _ => false,
            };
            // Consumida aunque no haya hecho nada: un `Ctrl+Alt+V` sin
            // estilo tomado no puede caer en el pegar del portapapeles.
            let hecho = !apagada && aplicar_orden(orden, gesto, escena);
            return Atendido::suyo(Repinte::si(hecho));
        }
        if let EventoOverlay::Caracter(c) = *ev {
            if let Some(h) = permitidas::herramienta_de_letra(self.anfitrion, c) {
                elegir_herramienta(gesto, h);
                return Atendido::suyo(Repinte::DIBUJO);
            }
            if let Some(cambio) = tecla_a_pluma(c) {
                match cambio {
                    CambioPluma::Grosor(g) => gesto.grosor_tinta = g,
                    CambioPluma::AlternarVariabilidad => {
                        use pixpin_motor2d::tinta::Variabilidad;
                        gesto.variabilidad = match gesto.variabilidad {
                            Variabilidad::Variable => Variabilidad::Constante,
                            Variabilidad::Constante => Variabilidad::Variable,
                        };
                    }
                }
                tracing::info!(grosor = gesto.grosor_tinta, variabilidad = ?gesto.variabilidad, "pluma");
                return Atendido::suyo(Repinte::NADA);
            }
        }
        // Los atajos de portapapeles y grupo. No son gestos: no tocan la
        // maquina de estados, operan sobre lo que hay elegido.
        if let EventoOverlay::Tecla {
            vk, ctrl, shift, ..
        } = *ev
            && ctrl
            && matches!(vk, v if v == b'C' as u32
                || v == b'X' as u32
                || v == b'V' as u32
                || v == b'D' as u32
                || v == b'G' as u32)
        {
            let hecho = self.portapapeles_y_grupos(vk, shift, gesto, escena, imagenes, vista, ancho_px, alto_px);
            return Atendido::suyo(Repinte::si(hecho));
        }

        // **El lazo.** Ni la maquina de estados ni `construir` lo tocan: no
        // `deja_rastro()`, asi que no nace ningun elemento. Lo lleva entero
        // quien tiene la escena — trazar mientras se arrastra y volcar lo
        // atrapado en la seleccion al soltar.
        if gesto.herramienta == Herramienta::Lazo {
            let mut cambio = false;
            match *ev {
                EventoOverlay::BotonPulsado(p) => {
                    gesto.lazo = Some(pixpin_motor2d::lazo::Lazo::empezar(vista.al_documento(p)));
                    cambio = true;
                }
                EventoOverlay::RatonMovido(p) => {
                    if let Some(l) = gesto.lazo.as_mut() {
                        // `mover` ya criba los puntos pegados: ver
                        // `PASO_MINIMO`. Solo se repinta si de verdad crecio
                        // el contorno.
                        cambio = l.mover(vista.al_documento(p));
                    }
                }
                EventoOverlay::BotonSoltado(_) => {
                    if let Some(l) = gesto.lazo.take() {
                        // Encerrar y no rozar: rodear algo es una peticion
                        // precisa (ver `ModoLazo`). Un lazo de dos puntos
                        // —un clic suelto— no atrapa nada y deja la seleccion
                        // vacia, que es lo mismo que pulsar en el aire.
                        let cogidos =
                            l.atrapados(&escena.elementos, pixpin_motor2d::lazo::ModoLazo::Encerrar);
                        gesto.seleccion.poner_todos(cogidos);
                        cambio = true;
                    }
                }
                _ => {}
            }
            if matches!(
                ev,
                EventoOverlay::BotonPulsado(_)
                    | EventoOverlay::BotonSoltado(_)
                    | EventoOverlay::RatonMovido(_)
            ) {
                return Atendido::suyo(Repinte::si(cambio));
            }
        }

        // **La bolita** (`Tool.BOLITA` del movil): como el lazo, la lleva
        // quien tiene la escena. Mientras se arrastra, lo que toca entra o
        // sale de la seleccion, una vez por barrido (`bolita.rs`).
        if gesto.herramienta == Herramienta::Bolita {
            let radio = pixpin_motor2d::bolita::RADIO_DE_LA_BOLITA / vista.camara.zoom.max(0.0001);
            // Sin barrido, mover el raton no repinta nada (la HD 4000).
            let repintar = gesto.bolita.centro.is_some()
                || matches!(ev, EventoOverlay::BotonPulsado(_) | EventoOverlay::BotonSoltado(_));
            match *ev {
                EventoOverlay::BotonPulsado(p) => {
                    gesto.bolita.empezar();
                    let q = vista.al_documento(p);
                    gesto.bolita.pasar(q, radio, escena, &mut gesto.seleccion);
                }
                EventoOverlay::RatonMovido(p) if gesto.bolita.centro.is_some() => {
                    let q = vista.al_documento(p);
                    gesto.bolita.pasar(q, radio, escena, &mut gesto.seleccion);
                }
                EventoOverlay::BotonSoltado(_) => gesto.bolita.terminar(),
                _ => {}
            }
            if matches!(
                ev,
                EventoOverlay::BotonPulsado(_)
                    | EventoOverlay::BotonSoltado(_)
                    | EventoOverlay::RatonMovido(_)
            ) {
                // Durante el barrido el circulo sigue al raton.
                return Atendido::suyo(Repinte::si(repintar));
            }
        }

        // **El cuentagotas de estilo.** Un clic toma el estilo de lo que hay
        // debajo; los siguientes lo pegan. Es el orden natural —se apunta al
        // modelo y luego a los destinos— y el que hace que la herramienta se
        // pueda usar seguida sin volver al teclado.
        if gesto.herramienta == Herramienta::CopiarEstilo
            && let EventoOverlay::BotonPulsado(p) = *ev
        {
            let q = vista.al_documento(p);
            let mut hecho = false;
            if let Some(id) = pixpin_motor2d::impacto::elemento_en(&escena.elementos, q) {
                hecho = match &gesto.estilo_tomado {
                    None => {
                        if let Some(e) = escena.buscar(id) {
                            gesto.estilo_tomado = Some(pixpin_motor2d::estilo::copiar(e));
                        }
                        true
                    }
                    Some(copiado) => {
                        let mut uno = pixpin_motor2d::seleccion::Seleccion::nueva();
                        uno.poner(id);
                        pixpin_motor2d::estilo::pegar_a(escena, &uno, copiado) > 0
                    }
                };
            }
            return Atendido::suyo(Repinte::si(hecho));
        }

        // La goma. El motor la tiene en su lista de herramientas pero no hace
        // nada con ella, asi que elegirla y arrastrar no borraba nada. Borra
        // lo que toca al pulsar y mientras se arrastra, que es como se usa
        // una goma; cada toque es un paso que se puede deshacer.
        if gesto.herramienta == Herramienta::Borrador {
            match *ev {
                EventoOverlay::BotonPulsado(_) => self.borrando = true,
                EventoOverlay::BotonSoltado(_) => self.borrando = false,
                _ => {}
            }
            let punto = match *ev {
                EventoOverlay::BotonPulsado(p) | EventoOverlay::RatonMovido(p) if self.borrando => {
                    Some(p)
                }
                _ => None,
            };
            let mut hecho = false;
            if let Some(p) = punto {
                let q = vista.al_documento(p);
                if let Some(id) = pixpin_motor2d::impacto::elemento_en(&escena.elementos, q) {
                    escena.abrir_paso();
                    hecho = escena.borrar_apuntando(id);
                    escena.cerrar_paso();
                }
            }
            if matches!(
                ev,
                EventoOverlay::BotonPulsado(_)
                    | EventoOverlay::BotonSoltado(_)
                    | EventoOverlay::RatonMovido(_)
            ) {
                return Atendido::suyo(Repinte::si(hecho));
            }
        }

        // **El laser** (F14): la estela sigue al raton pulsado y se apaga
        // sola. No toca la escena: se repinta lo de encima. Va como
        // `DIBUJO` y no como `ENCIMA` para que el lienzo no se ahorre el
        // fotograma moviendo el visual de la escena (ver A3).
        if gesto.herramienta == Herramienta::Laser {
            let cambio = match *ev {
                EventoOverlay::BotonPulsado(p) => {
                    gesto.laser.pulsar(vista.al_documento(p));
                    true
                }
                EventoOverlay::RatonMovido(p) => gesto.laser.mover(vista.al_documento(p)),
                EventoOverlay::Muestra(m) => gesto.laser.mover(vista.camara.a_mundo(Punto2::nuevo(
                    m.x() - vista.area.x as f32,
                    m.y() - vista.area.y as f32,
                ))),
                EventoOverlay::BotonSoltado(_) => {
                    gesto.laser.soltar();
                    true
                }
                _ => false,
            };
            if matches!(
                ev,
                EventoOverlay::BotonPulsado(_)
                    | EventoOverlay::BotonSoltado(_)
                    | EventoOverlay::RatonMovido(_)
                    | EventoOverlay::Muestra(_)
            ) {
                return Atendido::suyo(Repinte::si(cambio));
            }
        }

        // **La lupa** (`Tool.LUPA`): la varita que convierte una figura
        // cerrada, y con la mano, coger lo que mira una lupa elegida para
        // apuntarla a otro sitio. Solo en el lienzo, que es quien pinta lo
        // de dentro; en la pantalla la Lupa es la lupa viva.
        if let Some(cambio) = self.lupa.atender(
            ev,
            gesto,
            escena,
            |p| vista.al_documento(p),
            vista.camara.zoom,
            self.anfitrion == Anfitrion::Lienzo,
        ) {
            return Atendido::suyo(Repinte::si(cambio));
        }

        // **La zona** (F8): se arrastra el rectangulo y al soltar se pide la
        // foto. Una copia ya sacada se arrastra con la Zona puesta, como en
        // el movil: la lleva la mano de siempre hasta soltar.
        if gesto.herramienta == Herramienta::Zona && !self.zona_moviendo {
            match *ev {
                EventoOverlay::BotonPulsado(p) => {
                    let q = vista.al_documento(p);
                    let copia = pixpin_motor2d::impacto::elemento_en(&escena.elementos, q)
                        .and_then(|id| escena.buscar(id))
                        .is_some_and(pixpin_motor2d::zona::es_copia);
                    if copia {
                        self.zona_moviendo = true;
                        gesto.herramienta = Herramienta::Mano;
                    } else {
                        gesto.seleccion.limpiar();
                        gesto.zona = Some((q, q));
                        return Atendido::suyo(Repinte::DIBUJO);
                    }
                }
                EventoOverlay::RatonMovido(p) => {
                    if let Some((a, _)) = gesto.zona {
                        gesto.zona = Some((a, vista.al_documento(p)));
                        return Atendido::suyo(Repinte::DIBUJO);
                    }
                    return Atendido::suyo(Repinte::NADA);
                }
                EventoOverlay::Muestra(_) => return Atendido::suyo(Repinte::NADA),
                EventoOverlay::BotonSoltado(_) => {
                    if let Some((a, b)) = gesto.zona.take() {
                        if pixpin_motor2d::zona::cuenta(a, b, vista.camara.zoom) {
                            self.zona_pedida = Some(pixpin_motor2d::zona::caja(a, b));
                        }
                        return Atendido::suyo(Repinte::DIBUJO);
                    }
                    return Atendido::suyo(Repinte::NADA);
                }
                _ => {}
            }
        }

        // Traducir y, si le toca al motor, pasarselo.
        let Some(g) = a_evento(ev, vista.camara, vista.origen()) else {
            return Atendido::default();
        };
        let paso = self.al_motor(con_modificadores(g), gesto, escena, vista.camara);
        // La copia de zona ya se solto: vuelve la Zona a la mano.
        if self.zona_moviendo && matches!(ev, EventoOverlay::BotonSoltado(_)) {
            self.zona_moviendo = false;
            gesto.herramienta = Herramienta::Zona;
        }
        let repinte = Repinte::si(paso.construido);
        Atendido {
            consumido: true,
            repinte,
            paso: Some(paso),
            ..Atendido::default()
        }
    }

    /// El evento ya traducido al motor: suavizado, forma rapida, predictor,
    /// `Gesto::evento` y las cuatro de construir. Aparte para que las
    /// pruebas lo alimenten sin pasar por Windows (`con_modificadores`
    /// pregunta el teclado de verdad).
    pub fn al_motor(
        &mut self,
        mut g: EventoGesto,
        gesto: &mut Gesto,
        escena: &mut Escena,
        camara: &Camara,
    ) -> Paso {
        let ms = self.ms();
        // C1: con `[tinta] suavizado = "natural"`, la posicion pasa por el
        // filtro de 1 euro antes de llegar al gesto. Se filtra en PIXELES DE
        // PANTALLA y no en unidades del documento: los hercios del filtro
        // describen el temblor de la mano, que no cambia con el aumento. Solo
        // mientras se traza a mano: filtrar un arrastre de la seleccion se
        // sentiria como que la aplicacion va pegajosa.
        if self.suavizado_natural
            && matches!(
                gesto.herramienta,
                Herramienta::Lapiz | Herramienta::Resaltador | Herramienta::Grafito
            )
        {
            match &mut g {
                EventoGesto::Pulsar { p, .. } => {
                    self.filtro.reiniciar();
                    let s = camara.a_pantalla(*p);
                    let (x, y) = self.filtro.filtrar(s.x, s.y, ms);
                    *p = camara.a_mundo(Punto2::nuevo(x, y));
                }
                EventoGesto::Mover { p, .. } if gesto.elemento_en_curso().is_some() => {
                    let s = camara.a_pantalla(*p);
                    let (x, y) = self.filtro.filtrar(s.x, s.y, ms);
                    *p = camara.a_mundo(Punto2::nuevo(x, y));
                }
                _ => {}
            }
        }
        // Donde se pulso, para las cuatro de construir: `g` se consume en
        // `gesto.evento` y el punto hace falta DESPUES, cuando el punto
        // etiquetado ya ha nacido.
        let mut pulsado = None;
        match g {
            EventoGesto::Pulsar { p, .. } => {
                self.predictor.reiniciar();
                self.predictor.anotar(p, ms);
                self.quieto = Some((std::time::Instant::now(), p));
                pulsado = Some(p);
            }
            EventoGesto::Mover { p, .. } => {
                self.predictor.anotar(p, ms);
                // Moverse mas del temblor del movil (8 px de pantalla)
                // reinicia la pausa: la mano parada nunca esta quieta del
                // todo, y con menos la cuenta no se cumplia.
                let lejos = self.quieto.is_none_or(|(_, q)| {
                    q.distancia(p) * camara.zoom > pixpin_motor2d::forma_rapida::TEMBLOR_PX
                });
                if lejos {
                    self.quieto = Some((std::time::Instant::now(), p));
                }
            }
            EventoGesto::Soltar { .. } => self.quieto = None,
            _ => {}
        }
        let en_reposo_antes = gesto.en_reposo();
        let habia_eleccion = !gesto.seleccion.ids().is_empty();
        let r = gesto.evento(g, escena, 1.0 / camara.zoom);
        // **Las cuatro de construir.** Van DESPUES del gesto y solo al
        // pulsar: tres de ellas no dibujan nada -miran lo que ya hay y lo
        // cambian- y la cuarta remata el punto que el gesto acaba de hacer
        // nacer, asi que antes no existiria.
        let construido = pulsado
            .is_some_and(|p| super::construir::al_pulsar(escena, gesto, p, camara.zoom));
        Paso {
            r,
            pulsado,
            construido,
            en_reposo_antes,
            habia_eleccion,
        }
    }

    /// **El gesto de pararse**: el trazo a mano quieto con el boton pulsado
    /// se convierte en compas, rectangulo, linea o elipse, y lo que se
    /// arrastre despues la ajusta. `true` si convirtio (hay que repintar el
    /// dibujo). Lo llama el anfitrion en cada vuelta de su bucle.
    pub fn forma_rapida(&mut self, gesto: &mut Gesto, escena: &mut Escena, zoom: f32) -> bool {
        let Some((desde, p)) = self.quieto else {
            return false;
        };
        let dibujando_a_mano = matches!(
            gesto.herramienta,
            Herramienta::Lapiz | Herramienta::Grafito | Herramienta::Resaltador
        ) && gesto.trazo_en_curso().is_some();
        if !dibujando_a_mano {
            self.quieto = None;
            return false;
        }
        if desde.elapsed() < PAUSA_FORMA {
            return false;
        }
        // Una sola vez por pausa: si no parece nada, se sigue dibujando y la
        // siguiente pausa lo vuelve a mirar.
        self.quieto = None;
        if gesto.convertir_en_forma(escena, p, 1.0 / zoom).is_some() {
            self.predictor.reiniciar();
            return true;
        }
        false
    }

    /// Cuanto puede dormir el anfitrion sin perderse la forma rapida: con la
    /// mano quieta no llegan eventos.
    pub fn tope_forma_ms(&self) -> Option<u32> {
        self.quieto
            .map(|(desde, _)| PAUSA_FORMA.saturating_sub(desde.elapsed()).as_millis() as u32 + 1)
    }

    #[allow(clippy::too_many_arguments)]
    fn portapapeles_y_grupos(
        &mut self,
        vk: u32,
        shift: bool,
        gesto: &mut Gesto,
        escena: &mut Escena,
        imagenes: Option<&mut ImagenesLienzo>,
        vista: Vista<'_>,
        ancho_px: f32,
        alto_px: f32,
    ) -> bool {
        use pixpin_motor2d::portapapeles as pp;
        use pixpin_ui::panel_lateral::AccionPanel;
        match vk {
            v if v == b'C' as u32 => {
                self.portapapeles = pp::copiar(escena, &gesto.seleccion);
                false
            }
            v if v == b'X' as u32 => {
                self.portapapeles = pp::copiar(escena, &gesto.seleccion);
                !self.portapapeles.is_empty()
                    && crate::panel_dibujo::aplicar(AccionPanel::Borrar, gesto, escena)
            }
            v if v == b'V' as u32 => {
                use crate::imagenes_lienzo as img;
                // El portapapeles del sistema solo se abre si no hay nada
                // copiado dentro, y solo si el anfitrion sabe pintar
                // imagenes: ver `decidir_pegado`.
                let del_sistema = if self.portapapeles.is_empty() && imagenes.is_some() {
                    pixpin_codec::portapapeles::leer()
                } else {
                    None
                };
                let nuevos = match img::decidir_pegado(!self.portapapeles.is_empty(), del_sistema) {
                    img::Pegado::Elementos => pp::pegar(
                        escena,
                        &self.portapapeles,
                        pp::DESPLAZAMIENTO,
                        pp::DESPLAZAMIENTO,
                    ),
                    img::Pegado::Imagen(bruta) => match imagenes {
                        None => Vec::new(),
                        Some(imagenes) => {
                            let v = vista.camara.ventana(ancho_px, alto_px);
                            match imagenes.guardar(bruta) {
                                None => Vec::new(),
                                Some(id_objeto) => {
                                    // El tamano se toma de lo que de verdad
                                    // se subio: si la GPU obligo a reducir,
                                    // la caja tiene que seguir a los pixeles
                                    // o la imagen saldria estirada.
                                    let (w, h) =
                                        imagenes.tamano(id_objeto).expect("recien guardada");
                                    let (ancho, alto) =
                                        img::tamano_al_pegar(w, h, v.2 - v.0, v.3 - v.1);
                                    let (x, y) = img::esquina_centrada(v, ancho, alto);
                                    vec![escena.anadir(img::elemento_imagen(
                                        id_objeto, x, y, ancho, alto,
                                    ))]
                                }
                            }
                        }
                    },
                    img::Pegado::Nada => Vec::new(),
                };
                let hubo = !nuevos.is_empty();
                if hubo {
                    // Queda elegido lo pegado, como en Excalidraw: asi se
                    // puede llevar a su sitio de un tiron.
                    gesto.seleccion.poner_todos(nuevos);
                }
                hubo
            }
            v if v == b'D' as u32 => {
                crate::panel_dibujo::aplicar(AccionPanel::Duplicar, gesto, escena)
            }
            // Ctrl+Shift+L bloquea y desbloquea, como en Excalidraw.
            v if v == b'L' as u32 && shift => {
                pixpin_motor2d::organizar::bloquear(escena, &mut gesto.seleccion)
            }
            _ => {
                // Ctrl+G agrupa; con mayusculas, desagrupa.
                if shift {
                    pixpin_motor2d::organizar::desagrupar(escena, &gesto.seleccion);
                    true
                } else {
                    pixpin_motor2d::organizar::agrupar(escena, &gesto.seleccion).is_some()
                }
            }
        }
    }
}

/// Los mandos del filtro de 1 euro: lo que diga el TOML y, para lo que no
/// diga, el valor conservador del propio motor.
pub fn mandos_de(t: pixpin_store::Tinta) -> pixpin_tinta::Mandos {
    let d = pixpin_tinta::Mandos::default();
    pixpin_tinta::Mandos {
        corte_minimo: t.corte_minimo.unwrap_or(d.corte_minimo),
        beta: t.beta.unwrap_or(d.beta),
        ..d
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_shell::overlay::EventoOverlay;

    fn vista_de(camara: &Camara) -> Vista<'_> {
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 1600,
            alto: 900,
        };
        Vista {
            camara,
            area,
            interfaz: area,
            escala_por_cien: 100,
        }
    }

    fn tecla(vk: u32) -> EventoOverlay {
        EventoOverlay::Tecla {
            vk,
            shift: false,
            ctrl: false,
            alt: false,
        }
    }

    #[test]
    fn en_el_lienzo_una_herramienta_apagada_no_responde_a_su_letra_ni_a_su_atajo() {
        permitidas::fijar(pixpin_store::herramientas::Herramientas {
            apagadas: vec!["lapiz".into(), "lazo".into()],
        });
        let camara = Camara::nueva();
        let mut mano = Mano::con_tinta(Anfitrion::Lienzo, Default::default());
        let mut gesto = Gesto::nuevo();
        gesto.tomar_herramienta(Herramienta::Mano);
        let mut escena = Escena::nueva();
        // La L del lapiz: consumida por nadie, y la herramienta no cambia.
        let a = mano.herramienta(
            &EventoOverlay::Caracter('l'),
            &mut gesto,
            &mut escena,
            None,
            vista_de(&camara),
            1600.0,
            900.0,
        );
        assert_eq!(gesto.herramienta, Herramienta::Mano);
        assert!(!a.consumido || a.paso.is_none());
        // La S del lazo es un atajo del motor: consumida, pero sin elegirlo.
        mano.herramienta(&tecla(b'S' as u32), &mut gesto, &mut escena, None, vista_de(&camara), 1600.0, 900.0);
        assert_eq!(gesto.herramienta, Herramienta::Mano);
        // Caso negativo: la R del resaltador sigue valiendo.
        mano.herramienta(
            &EventoOverlay::Caracter('r'),
            &mut gesto,
            &mut escena,
            None,
            vista_de(&camara),
            1600.0,
            900.0,
        );
        assert_eq!(gesto.herramienta, Herramienta::Resaltador);
        permitidas::fijar(Default::default());
        // Encendida otra vez, la S elige el lazo.
        mano.herramienta(&tecla(b'S' as u32), &mut gesto, &mut escena, None, vista_de(&camara), 1600.0, 900.0);
        assert_eq!(gesto.herramienta, Herramienta::Lazo);
    }

    #[test]
    fn un_boton_de_una_herramienta_apagada_no_la_elige_aunque_se_colara_en_la_caja() {
        permitidas::fijar(pixpin_store::herramientas::Herramientas {
            apagadas: vec!["rombo".into()],
        });
        let camara = Camara::nueva();
        let mut mano = Mano::con_tinta(Anfitrion::Lienzo, Default::default());
        let mut gesto = Gesto::nuevo();
        let mut escena = Escena::nueva();
        // Una caja vieja con todos los botones (hecha antes de apagarlo), con
        // las formas desplegadas: el rombo esta en su desplegable.
        let caja = CajaHerramientas::barra_superior(
            vista_de(&camara).area,
            100,
            &pixpin_ui::BOTONES_EDITOR,
        );
        mano.desplegado = Some(pixpin_ui::GrupoBarra::Formas);
        let r = caja
            .con_desplegado(mano.desplegado)
            .rect_de_boton(BotonCaja::Elegir(Herramienta::Rombo))
            .unwrap();
        let centro = Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        };
        let a = mano.interfaz(
            &EventoOverlay::BotonPulsado(centro),
            &mut gesto,
            &mut escena,
            Some(&caja),
            false,
            vista_de(&camara),
        );
        assert!(a.consumido, "el clic es de la caja, no un trazo");
        assert_ne!(gesto.herramienta, Herramienta::Rombo);
        permitidas::fijar(Default::default());
    }

    /// **Un clic en un grupo coge su herramienta y ensena las hermanas; un
    /// clic en una hermana la coge y las cierra**, sin dejar tinta en el
    /// lienzo ni con el soltar, que ya cae donde no hay caja. Escape y
    /// dibujar tambien las cierran.
    #[test]
    fn un_grupo_se_despliega_se_elige_una_hermana_y_se_cierra_sin_manchar_el_lienzo() {
        use pixpin_ui::GrupoBarra;
        let camara = Camara::nueva();
        let v = vista_de(&camara);
        let mut mano = Mano::con_tinta(Anfitrion::Lienzo, Default::default());
        let mut gesto = Gesto::nuevo();
        let mut escena = Escena::nueva();
        let caja = CajaHerramientas::barra_superior(v.area, 100, &pixpin_ui::BOTONES_EDITOR);
        let centro = |r: Rect| Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        };
        let clic = |mano: &mut Mano, gesto: &mut Gesto, escena: &mut Escena, p: Punto| {
            let a = mano.interfaz(&EventoOverlay::BotonPulsado(p), gesto, escena, Some(&caja), false, v);
            let b = mano.interfaz(&EventoOverlay::BotonSoltado(p), gesto, escena, Some(&caja), false, v);
            (a, b)
        };
        let flechas = centro(caja.rect_de_boton(BotonCaja::Grupo(GrupoBarra::Flechas)).unwrap());
        let (a, _) = clic(&mut mano, &mut gesto, &mut escena, flechas);
        assert!(a.consumido && a.eligio);
        assert_eq!(gesto.herramienta, Herramienta::Flecha, "la cara del grupo");
        assert_eq!(mano.desplegado, Some(GrupoBarra::Flechas));
        // La hermana: la linea.
        let linea = centro(
            caja.con_desplegado(mano.desplegado)
                .rect_de_boton(BotonCaja::Elegir(Herramienta::Linea))
                .unwrap(),
        );
        let (a, b) = clic(&mut mano, &mut gesto, &mut escena, linea);
        assert!(a.consumido && b.consumido, "ni el pulsar ni el soltar son del lienzo");
        assert_eq!(gesto.herramienta, Herramienta::Linea);
        assert_eq!(mano.desplegado, None, "elegir una hermana las cierra");
        assert_eq!(escena.cuantos_visibles(), 0);
        // Escape cierra antes que nada.
        clic(&mut mano, &mut gesto, &mut escena, flechas);
        assert_eq!(mano.desplegado, Some(GrupoBarra::Flechas));
        let a = mano.interfaz(&tecla(0x1B), &mut gesto, &mut escena, Some(&caja), false, v);
        assert!(a.consumido);
        assert_eq!(mano.desplegado, None);
        // Caso negativo: un clic en el lienzo las cierra pero NO es de la
        // caja: sigue hasta el gesto, que dibuja.
        clic(&mut mano, &mut gesto, &mut escena, flechas);
        let a = mano.interfaz(
            &EventoOverlay::BotonPulsado(Punto { x: 800, y: 600 }),
            &mut gesto,
            &mut escena,
            Some(&caja),
            false,
            v,
        );
        assert!(!a.consumido);
        assert_eq!(mano.desplegado, None);
    }

    #[test]
    fn el_trazo_con_la_mano_comun_nace_en_el_documento_y_se_deshace_de_una_vez() {
        let camara = Camara {
            x: 100.0,
            y: 50.0,
            zoom: 2.0,
        };
        let mut mano = Mano::con_tinta(Anfitrion::Lector, Default::default());
        let mut gesto = Gesto::nuevo();
        let mut escena = Escena::nueva();
        let v = vista_de(&camara);
        for ev in [
            EventoOverlay::BotonPulsado(Punto { x: 200, y: 200 }),
            EventoOverlay::RatonMovido(Punto { x: 260, y: 220 }),
            EventoOverlay::BotonSoltado(Punto { x: 260, y: 220 }),
        ] {
            let _ = mano.herramienta(&ev, &mut gesto, &mut escena, None, v, 1600.0, 900.0);
        }
        let e = escena.visibles().next().expect("trazo");
        // (200/2 + 100, 200/2 + 50): donde cae el raton en el documento.
        let (x0, y0, _, _) = e.caja();
        assert!((x0 - 200.0).abs() < 3.0 && (y0 - 150.0).abs() < 3.0, "{x0} {y0}");
        assert!(escena.deshacer());
        assert_eq!(escena.cuantos_visibles(), 0);
    }
}
