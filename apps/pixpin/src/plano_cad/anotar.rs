//! **Anotar sobre un plano con el motor del lienzo**: el mismo anfitrion que
//! usan los lectores de Word, libro y PDF (`lector_tinta::Tinta`), con el
//! mismo gesto, la misma barra, el mismo panel y las mismas teclas que el
//! editor, y la tinta optimizada de siempre. Como PixPin Android v0.114
//! («anotar en el visor de planos usa las herramientas del motor del
//! lienzo, sin herramientas propias») y como pidio el usuario el 9-oct-2026:
//! no otra tinta, la que ya hay.
//!
//! El visor de planos (`pixpin_cad::anotado::Anotador`) le pasa sus eventos
//! y le deja pintar con Direct2D sobre cada fotograma, en su mismo
//! dispositivo: la tinta va en la misma pasada que el plano y no se separa
//! de el al mover o acercar.
//!
//! **La capa** es la `anot-<uid>` del plano, como la de un Word anotado: la
//! lee y la guarda [`Capa`], que deja intacto lo que puso el movil, y viaja
//! al sincronizar. Un plano que no es de ningun chat (abierto desde el
//! disco) guarda su capa en la cache, solo en este PC. Sus unidades son las
//! de Android: la milesima del lado mayor del plano, con la y hacia abajo.

use std::path::{Path, PathBuf};

use pixpin_cad::anotado::{Anotador, EventoPlano, Respuesta, VistaPlano};
use pixpin_geom::{Punto, Rect};
use pixpin_motor2d::camara::Camara;
use pixpin_render::{Color, MotorRender};
use pixpin_shell::overlay::EventoOverlay;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::core::Interface;

use crate::lector_tinta::{self, Capa, Tinta};

/// Donde vive la capa de `plano`.
pub fn ruta_de_la_capa(raiz: &Path, plano: &Path) -> PathBuf {
    use pixpin_proyecto::anotado as an;
    if let Some(r) = an::raiz_de(plano)
        && let Some(b) = an::base_del_documento(&r, plano)
    {
        return b.tinta();
    }
    let clave = super::clave(plano).unwrap_or_else(|| "plano".into());
    raiz.join("cache").join("cad").join(format!("{clave}.anot.excalidraw"))
}

/// El papel del visor (el fondo claro u oscuro del plano): sobre el oscuro,
/// la tinta oscura se aclara (`dibujo::tema`), como en los lectores.
fn papel(claro: bool) -> pixpin_motor2d::ColorRgba {
    let c = if claro { Color { r: 0.99, g: 0.988, b: 0.976, a: 1.0 } } else { Color { r: 0.13, g: 0.15, b: 0.19, a: 1.0 } };
    lector_tinta::papel(c)
}

fn camara_de(v: &VistaPlano) -> Camara {
    let (x, y, zoom) = v.camara();
    Camara { x: x as f32, y: y as f32, zoom: zoom as f32 }
}

fn area_de(v: &VistaPlano) -> Rect {
    Rect { x: v.origen_pantalla.0, y: v.origen_pantalla.1, ancho: v.ancho, alto: v.alto }
}

/// El motor del lienzo sobre un plano.
pub struct MotorDelPlano {
    ruta: PathBuf,
    capa: Capa,
    tinta: Tinta,
    /// El de Direct2D, sobre el dispositivo del visor (se rehace si cambia).
    motor: Option<(usize, MotorRender)>,
    /// Lo ultimo que se vio de cada elemento (su grosor y, si es texto, su
    /// letra): asi se sabe que nacio o que se le cambio el tamano desde el
    /// panel, y solo eso se pone a la escala del plano.
    visto: std::collections::HashMap<u64, (f32, Option<f32>)>,
}

/// Los grosores que pone el motor (fino, medio y grueso de las formas, del
/// lapiz y del resaltador), al nacer algo o al elegirlos en el panel: solo
/// eso se pone a la escala del plano; lo pegado o duplicado ya la trae.
fn grosor_de_fabrica(g: f32) -> bool {
    use pixpin_motor2d::estilo::NivelGrosor as N;
    [N::Fino, N::Medio, N::Grueso].iter().any(|n| [n.de_forma(), n.de_tinta(), n.de_resaltador()].iter().any(|v| (v - g).abs() < 1e-4))
}

/// Lo mismo con la letra: los tamanos del panel y el de fabrica.
fn letra_de_fabrica(t: f32, la_elegida: f32) -> bool {
    (t - la_elegida).abs() < 1e-4 || pixpin_ui::panel_lateral::TAMANOS_DE_LETRA.iter().any(|v| (v - t).abs() < 1e-4)
}

/// El grosor y la letra de un elemento, para ver si cambiaron.
fn medidas_de(e: &pixpin_motor2d::Elemento) -> (f32, Option<f32>) {
    let letra = match &e.figura {
        pixpin_motor2d::Figura::Texto { tam, .. } => Some(*tam),
        _ => None,
    };
    (e.grosor, letra)
}

impl MotorDelPlano {
    pub fn nuevo(ruta: PathBuf) -> MotorDelPlano {
        let capa = Capa::leer(&ruta);
        let mut tinta = Tinta::nueva();
        tinta.hoja = Some(0);
        let visto = capa.escena.elementos.iter().map(|e| (e.id, medidas_de(e))).collect();
        MotorDelPlano { ruta, capa, tinta, motor: None, visto }
    }

    /// **Los tamanos del lienzo, a la escala del plano** (lo pidio el usuario
    /// el 9-oct-2026: «la tinta es muy grande para el plano»). Los grosores y
    /// letras del motor son de una hoja; en un plano, la unidad de la capa
    /// es la milesima del lado entero y salian enormes. Lo que nace se pone a
    /// la escala de lo que se ve al dibujarlo (uno entre el zoom): un trazo «fino» se
    /// ve fino en la pantalla, como al anotar en AutoCAD, y se puede acercar
    /// para detallar.
    ///
    /// Vale tambien al **elegir otro tamano en el panel** (10-oct-2026, el
    /// usuario: «al seleccionar el tamano de la letra o el lapiz con las
    /// opciones disponibles se agrandan grandemente como antes y ya no hay
    /// forma de volver a ese tamano pequeno»): el panel pone el valor de una
    /// hoja tal cual, tanto a lo nuevo como a lo elegido; ahora, todo grosor
    /// o letra del panel que aparezca (al nacer o al cambiar) se pasa a la
    /// escala del plano. Lo que no viene del panel (estirar un texto, pegar,
    /// deshacer) se deja como esta.
    fn a_la_escala_del_plano(&mut self, zoom: f32) {
        let letra = self.tinta.gesto.estilo.tamano_letra;
        poner_a_escala(&mut self.capa.escena.elementos, &mut self.visto, 1.0 / zoom.max(1e-6), letra);
    }
}

/// Ver [`MotorDelPlano::a_la_escala_del_plano`]: `k` es uno entre el zoom y
/// `letra`, la letra elegida ahora en el motor.
fn poner_a_escala(elementos: &mut [pixpin_motor2d::Elemento], visto: &mut std::collections::HashMap<u64, (f32, Option<f32>)>, k: f32, letra: f32) {
    for e in elementos.iter_mut() {
        let antes = visto.get(&e.id).copied();
        let (g, t) = medidas_de(e);
        let grosor_nuevo = antes.is_none_or(|(g0, _)| (g0 - g).abs() > 1e-6);
        if grosor_nuevo && grosor_de_fabrica(g) {
            e.grosor *= k;
        }
        let letra_nueva = antes.is_none_or(|(_, t0)| t0.is_none_or(|t0| t.is_some_and(|t| (t0 - t).abs() > 1e-6)));
        if letra_nueva
            && let pixpin_motor2d::Figura::Texto { tam, .. } = &mut e.figura
            && letra_de_fabrica(*tam, letra)
        {
            *tam *= k;
            e.ancho *= k;
            e.alto *= k;
        }
        visto.insert(e.id, medidas_de(e));
    }
}

impl MotorDelPlano {
    fn guardar(&mut self) {
        if let Some(d) = self.ruta.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Err(e) = self.capa.guardar(&self.ruta) {
            tracing::warn!(?e, ruta = %self.ruta.display(), "no se pudo guardar lo anotado en el plano");
        }
    }
}

impl Anotador for MotorDelPlano {
    fn evento(&mut self, ev: EventoPlano, v: &VistaPlano) -> Respuesta {
        let p = |x: i32, y: i32| Punto { x, y };
        let ev = match ev {
            EventoPlano::Mover(x, y) => EventoOverlay::RatonMovido(p(x, y)),
            EventoPlano::Bajar(x, y) => EventoOverlay::BotonPulsado(p(x, y)),
            EventoPlano::Subir(x, y) => EventoOverlay::BotonSoltado(p(x, y)),
            EventoPlano::BajarCentral(x, y) => EventoOverlay::BotonCentralPulsado(p(x, y)),
            EventoPlano::SubirCentral(x, y) => EventoOverlay::BotonCentralSoltado(p(x, y)),
            EventoPlano::Rueda(d) => EventoOverlay::Rueda(d),
            EventoPlano::Tecla { vk, shift, ctrl, alt } => EventoOverlay::Tecla { vk, shift, ctrl, alt },
            EventoPlano::TeclaSoltada(vk) => EventoOverlay::TeclaSoltada(vk),
            EventoPlano::Caracter(c) => EventoOverlay::Caracter(c),
        };
        self.tinta.hoja = Some(0);
        let cam = camara_de(v);
        let h = self.tinta.evento(&ev, &mut self.capa, &cam, area_de(v), v.escala_por_cien);
        self.a_la_escala_del_plano(cam.zoom);
        if h.cambio {
            self.capa.sucia = true;
        }
        Respuesta { consumido: h.consumido, salir: h.salir, arrastra: h.arrastra }
    }

    fn pintar(&mut self, d3d: &ID3D11Device, destino: &ID3D11Texture2D, v: &VistaPlano, anotando: bool) {
        if self.capa.vacia() && !anotando {
            return;
        }
        let clave = d3d.as_raw() as usize;
        if self.motor.as_ref().is_none_or(|(k, _)| *k != clave) {
            match MotorRender::nuevo(d3d) {
                Ok(m) => self.motor = Some((clave, m)),
                Err(e) => {
                    tracing::warn!(?e, "sin Direct2D para la tinta del plano");
                    return;
                }
            }
        }
        let Some((_, motor)) = &self.motor else { return };
        let Ok(bitmap) = motor.destino_backbuffer(destino) else { return };
        let cam = camara_de(v);
        let (capa, tinta) = (&self.capa, &mut self.tinta);
        let area = area_de(v);
        let r = motor.dibujar(&bitmap, |p| {
            // Lo anotado, en las unidades de la capa con la camara del plano.
            p.poner_vista((0.0, 0.0), cam.zoom, (-cam.x * cam.zoom, -cam.y * cam.zoom));
            let vista = (cam.x, cam.y, cam.x + v.ancho as f32 / cam.zoom, cam.y + v.alto as f32 / cam.zoom);
            tinta.pintar_capa(p, 0, capa, vista, cam.zoom, papel(v.claro), anotando);
            // La barra y el panel del lienzo, en pixeles de la ventana.
            p.desplazar(0.0, 0.0);
            if anotando {
                tinta.pintar_interfaz(p, Some(capa), area, v.escala_por_cien);
            }
        });
        if let Err(e) = r {
            tracing::warn!(?e, "no se pudo pintar la tinta del plano");
        }
    }

    fn terminar(&mut self) {
        // Lo que se escribia se queda escrito, y lo elegido se suelta.
        if self.tinta.gesto.esta_escribiendo() {
            self.tinta.gesto.cerrar_texto(&mut self.capa.escena);
            self.capa.sucia = true;
        }
        self.tinta.gesto.seleccion.limpiar();
        self.tinta.gesto.lazo = None;
        self.guardar();
    }

    fn quiere_escape(&self) -> bool {
        self.tinta.quiere_escape()
    }
}

/// **Lo anotado, en una hoja impresa**: la capa con el motor (el mismo
/// pintado que en pantalla), en papel blanco.
pub struct ParaImprimir {
    capa: Capa,
    tinta: std::cell::RefCell<Tinta>,
    /// Unidades del plano por unidad de la capa.
    pub unidad: f64,
}

impl ParaImprimir {
    pub fn leer(ruta: &Path, m: &pixpin_cad::modelo::Modelo) -> Option<ParaImprimir> {
        let capa = Capa::leer(ruta);
        (!capa.vacia()).then(|| ParaImprimir {
            capa,
            tinta: std::cell::RefCell::new(Tinta::nueva()),
            unidad: pixpin_cad::anotado::unidad_de_la_capa(m),
        })
    }

    /// Pinta la capa a `escala` (unidades del destino por unidad de la capa)
    /// con su (0, 0) en `cero`. `visible`: lo que se ve, en la capa.
    pub fn pintar(&self, p: &pixpin_render::Pintor, escala: f32, cero: (f32, f32), visible: (f32, f32, f32, f32)) {
        p.poner_vista((0.0, 0.0), escala, cero);
        let blanco = lector_tinta::papel(Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 });
        self.tinta.borrow_mut().pintar_capa(p, 0, &self.capa, visible, escala, blanco, false);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::estilo::NivelGrosor;

    fn raya(id: u64, grosor: f32) -> pixpin_motor2d::Elemento {
        let mut e = pixpin_motor2d::zona::marca((0.0, 0.0, 10.0, 10.0), "x");
        e.id = id;
        e.grosor = grosor;
        e
    }

    #[test]
    fn lo_elegido_en_el_panel_se_pone_a_la_escala_del_plano() {
        let k = 0.01;
        let mut visto = std::collections::HashMap::new();
        // Nace con el grosor «grueso» del panel: a la escala del plano.
        let grueso = NivelGrosor::Grueso.de_tinta();
        let mut v = vec![raya(1, grueso)];
        poner_a_escala(&mut v, &mut visto, k, 20.0);
        assert!((v[0].grosor - grueso * k).abs() < 1e-6);
        // Una vuelta sin cambios no lo vuelve a achicar.
        poner_a_escala(&mut v, &mut visto, k, 20.0);
        assert!((v[0].grosor - grueso * k).abs() < 1e-6);
        // Se elige «fino» en el panel con la raya elegida: tambien a escala
        // (antes se quedaba con el valor de una hoja, enorme en el plano).
        v[0].grosor = NivelGrosor::Fino.de_tinta();
        poner_a_escala(&mut v, &mut visto, k, 20.0);
        assert!((v[0].grosor - NivelGrosor::Fino.de_tinta() * k).abs() < 1e-6);
        // Un texto con la letra de 36 del panel.
        let mut t = raya(2, NivelGrosor::Fino.de_forma());
        t.figura = pixpin_motor2d::Figura::Texto { texto: "a".into(), tam: 36.0, familia: "Segoe UI".into() };
        v.push(t);
        poner_a_escala(&mut v, &mut visto, k, 20.0);
        let pixpin_motor2d::Figura::Texto { tam, .. } = &v[1].figura else { panic!() };
        assert!((tam - 0.36).abs() < 1e-5, "{tam}");
        // Caso negativo: un cambio que no viene del panel (estirar el
        // texto, pegar algo ya a escala) se deja como esta.
        if let pixpin_motor2d::Figura::Texto { tam, .. } = &mut v[1].figura {
            *tam = 0.5;
        }
        v.push(raya(3, 0.0123));
        poner_a_escala(&mut v, &mut visto, k, 20.0);
        let pixpin_motor2d::Figura::Texto { tam, .. } = &v[1].figura else { panic!() };
        assert!((tam - 0.5).abs() < 1e-6);
        assert!((v[2].grosor - 0.0123).abs() < 1e-7);
    }
}
