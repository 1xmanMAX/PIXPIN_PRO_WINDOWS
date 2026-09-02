//! La capa viva: dibujar sobre la pantalla mientras la pantalla sigue viva.
//!
//! Es el segundo de los dos modos que pidio el usuario. El primero —congelar
//! la pantalla y anotar sobre la foto— lo cubre el overlay de S1-B2. Este es
//! el otro: una ventana transparente a pantalla completa, siempre encima,
//! sobre la que se dibuja mientras debajo sigue reproduciendose el video,
//! avanzando la presentacion o compilando el codigo.
//!
//! Lo que lo hace usable de verdad es **poder salirse de en medio** (D50): la
//! capa alterna entre recoger el raton (dibujas) y dejarlo pasar (el dibujo
//! se ve, pero los clics llegan a la aplicacion de abajo). Sin eso, una capa
//! a pantalla completa secuestra el escritorio y solo sirve para un garabato
//! rapido.

use anyhow::{Context, Result};
use pixpin_capture::Instantanea;
use pixpin_geom::{Monitor, Punto, Rect};
use pixpin_motor2d::{Escena, Orden, Punto2};
use pixpin_render::{Color, MotorRender, RectF, Superficie};
use pixpin_shell::overlay::VentanaOverlay;
use pixpin_ui::{
    Anotador, BotonCaja, CajaHerramientas, EfectoAnotador, EventoAnotador, Herramienta, Lupa,
    TeclaAnotador,
};
use std::rc::Rc;
use std::time::{Duration, Instant};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

/// Una captura envuelta como bitmap. La instantanea viaja con el bitmap
/// porque el bitmap ES su textura: soltarla lo dejaria colgando.
struct Fondo {
    _instantanea: Instantanea,
    bitmap: ID2D1Bitmap1,
}

/// Los dos modos que pidio el usuario (D49).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoCapa {
    /// Transparente: la pantalla sigue viva debajo.
    Viva,
    /// Con la captura del monitor como fondo (D56). Sin modo pasante: no
    /// hay nada vivo debajo a lo que dejar pasar los clics.
    Congelada,
}

/// Una capa de anotacion cubriendo un monitor.
pub struct CapaViva {
    ventana: VentanaOverlay,
    superficie: Superficie,
    motor: Rc<MotorRender>,
    area: Rect,
    monitor: Monitor,
    /// La captura de fondo del modo congelado; `None` en el modo vivo.
    fondo: Option<Fondo>,
    escala_por_cien: u32,
    escena: Escena,
    anotador: Anotador,
    en_curso: Option<pixpin_motor2d::Elemento>,
    caja: CajaHerramientas,
    /// Ultima posicion conocida del cursor: la lupa la necesita para saber
    /// que ampliar sin esperar a un clic.
    cursor: Punto,
    /// Lo ultimo que se vio debajo de la capa, para la lupa (D60).
    muestra: Option<Fondo>,
    ultima_muestra: Instant,
}

impl CapaViva {
    pub fn nueva(
        d3d: &ID3D11Device,
        motor: Rc<MotorRender>,
        monitor: &Monitor,
        semilla: u32,
        fondo: Option<Instantanea>,
    ) -> Result<CapaViva> {
        let ventana =
            VentanaOverlay::nueva(monitor.area).context("no se pudo crear la capa viva")?;
        let fondo = match fondo {
            Some(instantanea) => Some(Fondo {
                bitmap: motor
                    .bitmap_desde_textura(instantanea.textura())
                    .context("no se pudo envolver la captura de fondo")?,
                _instantanea: instantanea,
            }),
            None => None,
        };
        let superficie = Superficie::nueva(
            &motor,
            d3d,
            ventana.handle(),
            monitor.area.ancho,
            monitor.area.alto,
        )
        .context("no se pudo preparar el dibujo de la capa")?;

        // La caja se coloca respecto al area de trabajo entera, no respecto
        // a un contenido: aqui el "contenido" es la pantalla.
        let caja = CajaHerramientas::colocar(
            monitor.area_trabajo,
            monitor.area_trabajo,
            monitor.escala_por_cien,
        );

        Ok(CapaViva {
            ventana,
            superficie,
            motor,
            area: monitor.area,
            monitor: *monitor,
            fondo,
            escala_por_cien: monitor.escala_por_cien,
            escena: Escena::nueva(),
            anotador: Anotador::nuevo(semilla | 1),
            en_curso: None,
            caja,
            cursor: Punto { x: 0, y: 0 },
            muestra: None,
            ultima_muestra: Instant::now(),
        })
    }

    pub fn mostrar(&self) {
        self.ventana.mostrar();
        self.ventana.enfocar();
        self.pintar();
    }

    /// Alterna entre dibujar y dejar pasar los clics (D50). En el modo
    /// congelado no hace nada: debajo solo hay una foto (D56).
    pub fn alternar_pasante(&self) -> bool {
        if self.fondo.is_some() {
            return false;
        }
        let ahora = !self.ventana.es_pasante();
        self.ventana.poner_pasante(ahora);
        // Repintar para que la caja de herramientas se atenue: si no, el
        // usuario no sabria en cual de los dos estados esta.
        self.pintar();
        ahora
    }

    pub fn tiene_dibujo(&self) -> bool {
        self.escena.cuantos_visibles() > 0
    }

    pub fn monitor(&self) -> Monitor {
        self.monitor
    }

    /// Si hace falta una captura fresca para la lupa (D60): solo con la
    /// lupa activa, recogiendo el raton, y como mucho 60 veces por segundo.
    pub fn quiere_muestra(&self) -> bool {
        // Con fondo congelado la lupa amplia el fondo: no hay nada nuevo
        // que muestrear.
        self.fondo.is_none()
            && self.anotador.herramienta() == Herramienta::Lupa
            && !self.ventana.es_pasante()
            && self.ultima_muestra.elapsed() >= Duration::from_millis(16)
    }

    /// La pantalla de ahora mismo, para que la lupa la amplie.
    pub fn poner_muestra(&mut self, instantanea: Instantanea) {
        if let Ok(bitmap) = self.motor.bitmap_desde_textura(instantanea.textura()) {
            self.muestra = Some(Fondo {
                _instantanea: instantanea,
                bitmap,
            });
        }
        self.ultima_muestra = Instant::now();
        self.pintar();
    }

    /// Un evento del raton en coordenadas del escritorio virtual. Devuelve
    /// `false` si la capa pide cerrarse.
    pub fn raton(&mut self, evento: EventoRaton) -> bool {
        // Local al monitor: el documento de la capa vive en coordenadas de
        // esta pantalla, no del escritorio entero.
        let local = |p: Punto| Punto {
            x: p.x - self.area.x,
            y: p.y - self.area.y,
        };

        match evento {
            EventoRaton::Pulsar(p) => {
                let l = local(p);
                self.cursor = l;
                // Un clic en la caja ELIGE, no dibuja: sin esta comprobacion,
                // pulsar el lapiz dejaria un punto de tinta bajo el boton.
                if let Some(boton) = self.caja.boton_en(l) {
                    return self.pulsar_boton(boton);
                }
                if self.caja.contiene(l) {
                    return true;
                }
                self.anotar(EventoAnotador::Pulsar(a_punto2(l)));
            }
            EventoRaton::Mover(p) => {
                let l = local(p);
                self.cursor = l;
                self.anotar(EventoAnotador::Mover(a_punto2(l)));
            }
            EventoRaton::Soltar(p) => {
                let l = local(p);
                self.cursor = l;
                if !self.caja.contiene(l) {
                    self.anotar(EventoAnotador::Soltar(a_punto2(l)));
                }
            }
            EventoRaton::Rueda(delta) => self.anotar(EventoAnotador::Rueda(delta)),
        }
        true
    }

    /// Una tecla. Devuelve `false` si la capa pide cerrarse.
    pub fn tecla(&mut self, tecla: TeclaAnotador) -> bool {
        let efecto = self.anotador.procesar(EventoAnotador::Tecla(tecla));
        self.aplicar(efecto)
    }

    fn pulsar_boton(&mut self, boton: BotonCaja) -> bool {
        match boton {
            BotonCaja::Elegir(h) => {
                self.anotar(EventoAnotador::CambiarHerramienta(h));
                true
            }
            BotonCaja::Deshacer => {
                self.escena.deshacer();
                self.pintar();
                true
            }
            BotonCaja::Rehacer => {
                self.escena.rehacer();
                self.pintar();
                true
            }
            BotonCaja::Color => true,
            BotonCaja::Salir => false,
        }
    }

    fn anotar(&mut self, evento: EventoAnotador) {
        let efecto = self.anotador.procesar(evento);
        self.aplicar(efecto);
    }

    /// Aplica un efecto de la maquina. Devuelve `false` si pide salir.
    fn aplicar(&mut self, efecto: EfectoAnotador) -> bool {
        match efecto {
            EfectoAnotador::Nada => return true,
            EfectoAnotador::Repintar => self.en_curso = None,
            EfectoAnotador::EnCurso(e) => self.en_curso = Some(*e),
            EfectoAnotador::Terminado(e) => {
                self.en_curso = None;
                self.escena.anadir(*e);
            }
            EfectoAnotador::BorrarEn(p) => {
                if let Some(v) = self.escena.elemento_en(p) {
                    self.escena.borrar(v);
                }
            }
            EfectoAnotador::Deshacer => {
                self.escena.deshacer();
            }
            EfectoAnotador::Rehacer => {
                self.escena.rehacer();
            }
            EfectoAnotador::PedirTexto(_) => {
                tracing::info!("el texto in situ llega con la entrada IME");
                return true;
            }
            EfectoAnotador::Salir => return false,
        }
        self.pintar();
        true
    }

    /// Repinta la capa entera: dibujo, trazo en curso y caja.
    pub fn pintar(&self) {
        let Ok(destino) = self.superficie.empezar(&self.motor) else {
            return;
        };
        let mut ordenes = pixpin_motor2d::ordenes_de_escena(&self.escena);
        if let Some(e) = &self.en_curso {
            ordenes.extend(pixpin_motor2d::ordenes(e));
        }
        let pasante = self.ventana.es_pasante();

        let _ = self.motor.dibujar(&destino, |p| {
            // Transparente de verdad: lo que hay debajo se ve y se sigue
            // moviendo. Esto es lo que separa la capa viva del modo
            // congelado, donde el fondo es una foto.
            p.limpiar_transparente();
            let todo = RectF {
                x: 0.0,
                y: 0.0,
                ancho: self.area.ancho as f32,
                alto: self.area.alto as f32,
            };
            // En congelado, la foto va debajo de todo (D56).
            if let Some(f) = &self.fondo {
                p.bitmap(&f.bitmap, todo, None, false);
            }
            pintar_ordenes(p, &ordenes, todo);
            // La caja y la lupa desaparecen en modo pasante: ahi la capa no
            // recoge el raton, asi que unos botones que no responden solo
            // estorban.
            if !pasante {
                self.pintar_lupa(p);
                self.pintar_caja(p);
            }
        });
        let _ = self.superficie.presentar();
    }

    /// La lupa (D52): amplia lo ultimo muestreado alrededor del cursor y se
    /// coloca fuera de su propia fuente (D60). No es un elemento: no se
    /// guarda ni se captura.
    fn pintar_lupa(&self, p: &pixpin_render::Pintor) {
        if self.anotador.herramienta() != Herramienta::Lupa {
            return;
        }
        // Congelada: amplia la foto. Viva: lo ultimo muestreado.
        let Some(fondo) = self.fondo.as_ref().or(self.muestra.as_ref()) else {
            return;
        };
        let lupa = Lupa::con_aumento(self.escala_por_cien, self.anotador.lupa());
        let local = Rect {
            x: 0,
            y: 0,
            ancho: self.area.ancho,
            alto: self.area.alto,
        };
        let fuente = lupa.region_fuente(self.cursor, local);
        let pos = lupa.colocar_fuera(self.cursor, local);
        let d = lupa.diametro as f32;
        let destino = RectF {
            x: pos.x as f32,
            y: pos.y as f32,
            ancho: d,
            alto: d,
        };
        p.bitmap(
            &fondo.bitmap,
            destino,
            Some(RectF {
                x: fuente.x as f32,
                y: fuente.y as f32,
                ancho: fuente.ancho as f32,
                alto: fuente.alto as f32,
            }),
            true,
        );
        p.trazar(
            destino,
            2.0 * self.escala_por_cien as f32 / 100.0,
            Color::ACENTO,
        );
    }

    fn pintar_caja(&self, p: &pixpin_render::Pintor) {
        let e = self.escala_por_cien as f32 / 100.0;
        let m = self.caja.marco;
        p.rellenar_redondeado(
            RectF {
                x: m.x as f32,
                y: m.y as f32,
                ancho: m.ancho as f32,
                alto: m.alto as f32,
            },
            8.0 * e,
            Color {
                r: 0.12,
                g: 0.12,
                b: 0.14,
                a: 0.92,
            },
        );

        let activa = self.anotador.herramienta();
        for (i, boton) in pixpin_ui::BOTONES.iter().enumerate() {
            let r = self.caja.rect_de(i);
            let caja_boton = RectF {
                x: r.x as f32,
                y: r.y as f32,
                ancho: r.ancho as f32,
                alto: r.alto as f32,
            };
            if matches!(boton, BotonCaja::Elegir(h) if *h == activa) {
                p.rellenar_redondeado(
                    caja_boton,
                    6.0 * e,
                    Color {
                        r: 0.25,
                        g: 0.45,
                        b: 0.85,
                        a: 1.0,
                    },
                );
            }
            // Sin iconos todavia: una letra por herramienta, que es legible
            // y no bloquea el resto de la fase. Los iconos vectoriales
            // llegan cuando el motor dibuje sus propios simbolos.
            p.texto(
                etiqueta(*boton),
                caja_boton.x + 13.0 * e,
                caja_boton.y + 8.0 * e,
                16.0 * e,
                Color::BLANCO,
            );
        }
    }
}

/// La letra que representa cada boton mientras no haya iconos.
fn etiqueta(b: BotonCaja) -> &'static str {
    match b {
        BotonCaja::Elegir(Herramienta::Mano) => "M",
        BotonCaja::Elegir(Herramienta::Lapiz) => "L",
        BotonCaja::Elegir(Herramienta::Resaltador) => "R",
        BotonCaja::Elegir(Herramienta::Linea) => "/",
        BotonCaja::Elegir(Herramienta::Flecha) => ">",
        BotonCaja::Elegir(Herramienta::Rectangulo) => "□",
        BotonCaja::Elegir(Herramienta::Elipse) => "○",
        BotonCaja::Elegir(Herramienta::Texto) => "T",
        BotonCaja::Elegir(Herramienta::Foco) => "F",
        BotonCaja::Elegir(Herramienta::Lupa) => "Q",
        BotonCaja::Elegir(Herramienta::Borrador) => "B",
        BotonCaja::Deshacer => "↶",
        BotonCaja::Rehacer => "↷",
        BotonCaja::Color => "C",
        BotonCaja::Salir => "X",
    }
}

/// Pinta las ordenes del motor con el pintor. Igual que en el pin, pero sin
/// desplazamiento: aqui el origen del documento es el del monitor. `marco`
/// es el lienzo entero, que el velo del foco necesita conocer.
fn pintar_ordenes(p: &pixpin_render::Pintor, ordenes: &[Orden], marco: RectF) {
    let color = |c: pixpin_motor2d::ColorRgba| Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    };
    for orden in ordenes {
        match orden {
            Orden::Poligono { puntos, color: c } | Orden::Relleno { puntos, color: c } => {
                let v: Vec<(f32, f32)> = puntos.iter().map(|q| (q.x, q.y)).collect();
                p.poligono(&v, color(*c));
            }
            Orden::Polilinea {
                puntos,
                color: c,
                grosor,
                ..
            } => {
                let v: Vec<(f32, f32)> = puntos.iter().map(|q| (q.x, q.y)).collect();
                p.polilinea(&v, *grosor, color(*c));
            }
            Orden::Texto {
                texto,
                x,
                y,
                tam,
                color: c,
                ancho_max,
                ..
            } => p.texto_ajustado(texto, *x, *y, *tam, *ancho_max, color(*c)),
            Orden::Velo { hueco, color: c } => {
                let v: Vec<(f32, f32)> = hueco.iter().map(|q| (q.x, q.y)).collect();
                p.velo(marco, &v, color(*c));
            }
            // Las imagenes incrustadas quedan para S6 (D61).
            Orden::Imagen { .. } => {}
        }
    }
}

/// Lo que le pasa al raton sobre la capa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventoRaton {
    Pulsar(Punto),
    Mover(Punto),
    Soltar(Punto),
    Rueda(i32),
}

fn a_punto2(p: Punto) -> Punto2 {
    Punto2::nuevo(p.x as f32, p.y as f32)
}

/// Códigos de tecla que la capa entiende.
const VK_ESCAPE: u32 = 0x1B;
const VK_Z: u32 = 0x5A;
const VK_Y: u32 = 0x59;
/// Espacio alterna entre dibujar y dejar pasar los clics: es la tecla mas
/// grande del teclado y la unica que se acierta sin mirar mientras dibujas.
const VK_SPACE: u32 = 0x20;

/// Abre la capa sobre el monitor principal, viva o congelada, y bombea sus
/// eventos hasta que el usuario la cierra. Devuelve la imagen del dibujo si
/// dibujó algo.
///
/// La captura final se hace **de la pantalla con la capa incluida**: es lo
/// que el usuario ve, y es lo que espera que se quede como pin.
pub fn ejecutar_capa(
    recursos: &mut crate::overlay::Recursos,
    modo: ModoCapa,
) -> Result<Option<pixpin_codec::ImagenRgba>> {
    use pixpin_shell::overlay::{EventoOverlay, bucle_modal};
    use pixpin_shell::ventana::Continuar;

    let disposicion =
        pixpin_capture::enumerar_monitores().context("no se pudieron enumerar los monitores")?;
    let monitor = *disposicion.principal().context("sin monitor principal")?;

    // Congelar ANTES de crear la ventana: asi la foto no lleva la capa.
    let fondo = match modo {
        ModoCapa::Viva => None,
        ModoCapa::Congelada => Some(
            recursos
                .congelar_monitor(&monitor)
                .context("no se pudo congelar la pantalla")?,
        ),
    };

    let semilla = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(1);
    let mut capa = CapaViva::nueva(&recursos.d3d(), recursos.motor(), &monitor, semilla, fondo)?;
    capa.mostrar();
    tracing::info!(?modo, "capa de anotacion abierta");

    let ventanas = [];
    bucle_modal(&ventanas, |_, evento| {
        let seguir = match evento {
            EventoOverlay::BotonPulsado(p) => capa.raton(EventoRaton::Pulsar(p)),
            EventoOverlay::RatonMovido(p) => {
                let seguir = capa.raton(EventoRaton::Mover(p));
                // La lupa sobre pantalla viva necesita ver lo de debajo
                // AHORA, no lo de cuando se abrio la capa (D60).
                if seguir && capa.quiere_muestra() {
                    match recursos.congelar_monitor(&capa.monitor()) {
                        Ok(inst) => capa.poner_muestra(inst),
                        Err(e) => tracing::debug!(?e, "sin muestra para la lupa"),
                    }
                }
                seguir
            }
            EventoOverlay::BotonSoltado(p) => capa.raton(EventoRaton::Soltar(p)),
            EventoOverlay::Tecla { vk, shift } => match vk {
                VK_SPACE => {
                    let pasante = capa.alternar_pasante();
                    tracing::info!(pasante, "la capa cambia de modo");
                    true
                }
                VK_ESCAPE => capa.tecla(TeclaAnotador::Escape),
                VK_Z => capa.tecla(if shift {
                    TeclaAnotador::Rehacer
                } else {
                    TeclaAnotador::Deshacer
                }),
                VK_Y => capa.tecla(TeclaAnotador::Rehacer),
                _ => true,
            },
            EventoOverlay::Rueda(delta) => capa.raton(EventoRaton::Rueda(delta)),
            EventoOverlay::Pintar => {
                capa.pintar();
                true
            }
            EventoOverlay::Cerrar => false,
            _ => true,
        };
        if seguir { Continuar::Si } else { Continuar::No }
    });

    if !capa.tiene_dibujo() {
        return Ok(None);
    }

    // La capa se oculta ANTES de capturar y la captura recoge la pantalla ya
    // sin ella: lo que se pinea es el dibujo compuesto sobre el fondo real,
    // que es lo que el usuario tenia delante.
    let imagen = capturar_con_dibujo(recursos, &monitor, &capa)?;
    Ok(Some(imagen))
}

/// Compone el dibujo de la capa sobre una captura del monitor.
fn capturar_con_dibujo(
    recursos: &mut crate::overlay::Recursos,
    monitor: &Monitor,
    capa: &CapaViva,
) -> Result<pixpin_codec::ImagenRgba> {
    // La capa sigue viva hasta que su `Drop` la destruya, asi que se captura
    // la pantalla CON el dibujo encima: es exactamente lo que se ve.
    let instantanea = recursos
        .congelar_monitor(monitor)
        .context("no se pudo capturar la pantalla anotada")?;
    let imagen = pixpin_capture::a_imagen(recursos.dispositivo(), &instantanea)
        .context("no se pudo bajar la captura a memoria")?;
    let _ = capa;
    Ok(imagen)
}
