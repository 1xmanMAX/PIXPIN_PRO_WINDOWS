//! **Anotar encima de la zona sin abrir otra ventana** (v2-captura).
//!
//! Es el mismo nucleo de dibujo que el lienzo, el pin y el anotador de
//! pantalla: una `Escena` y un `Gesto` de `pixpin-motor2d`, pintados con
//! `dibujo::pintar`. Aqui solo se ofrecen las cinco de la maqueta (lapiz,
//! flecha, rectangulo, texto y mosaico), cinco colores y tres grosores.
//!
//! La escena vive en coordenadas de la zona, con un aumento igual a la
//! escala del monitor: el grosor «medio» mide lo mismo a ojo en un monitor
//! al 100 % que en uno al 150 %, como en el resto de la app.
//!
//! **El mosaico tapa de verdad**: en pantalla, la foto de la zona se baja a
//! la CPU la primera vez que hace falta y se tapa con la misma receta que el
//! lienzo (`mosaico::tapar_rgba`); al salir, la captura se tapa igual antes
//! de pintar encima lo demas. Mientras se arrastra un mosaico se ve su banda
//! maciza, que tapa igual y no cuesta rehacer la foto en cada movimiento.

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_geom::Punto;
use pixpin_motor2d::cache::Cache;
use pixpin_motor2d::elemento::ColorRgba;
use pixpin_motor2d::estilo::NivelGrosor;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::mosaico::{self, Tapado};
use pixpin_motor2d::texto::TeclaTexto;
use pixpin_motor2d::vector::Punto2;
use pixpin_motor2d::{Escena, Figura};
use pixpin_render::{MotorRender, Pintor, RectF};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

use super::disposicion::{COLORES, Util};
use crate::imagenes_lienzo::ImagenesLienzo;

/// Teclas virtuales que se traducen a edicion de texto.
const VK_BACK: u32 = 0x08;
const VK_RETURN: u32 = 0x0D;
const VK_END: u32 = 0x23;
const VK_HOME: u32 = 0x24;
const VK_LEFT: u32 = 0x25;
const VK_RIGHT: u32 = 0x27;
const VK_DELETE: u32 = 0x2E;

pub struct Anotacion {
    pub escena: Escena,
    pub gesto: Gesto,
    pub util: Option<Util>,
    pub color: usize,
    pub grosor: usize,
    /// Esquina de la zona en el escritorio virtual.
    origen: Punto,
    /// Pixeles fisicos por unidad del dibujo: la escala del monitor.
    zoom: f32,
    cache: Cache,
    cache_tinta: pixpin_render::CacheTinta,
    imagenes: ImagenesLienzo,
    /// La foto de la zona en CPU, para tapar. Se baja la primera vez que
    /// hay un mosaico.
    foto: Option<ImagenRgba>,
    /// La foto ya tapada en la GPU, con los tapados que lleva.
    tapada: Option<(Vec<Tapado>, ID2D1Bitmap1)>,
}

pub fn herramienta_de(u: Util) -> Herramienta {
    match u {
        Util::Lapiz => Herramienta::Lapiz,
        Util::Flecha => Herramienta::Flecha,
        Util::Rectangulo => Herramienta::Rectangulo,
        Util::Texto => Herramienta::Texto,
        Util::Mosaico => Herramienta::Mosaico,
    }
}

pub fn color_de(i: usize) -> ColorRgba {
    let [r, g, b] = COLORES[i.min(COLORES.len() - 1)];
    ColorRgba::opaco(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

impl Anotacion {
    pub fn nueva(origen: Punto, escala_por_cien: u32) -> Anotacion {
        let mut a = Anotacion {
            escena: Escena::nueva(),
            gesto: Gesto::nuevo(),
            util: None,
            color: 0,
            grosor: 1,
            origen,
            zoom: escala_por_cien.max(100) as f32 / 100.0,
            cache: Cache::nueva(),
            cache_tinta: pixpin_render::CacheTinta::nueva(),
            imagenes: ImagenesLienzo::nuevo(4096),
            foto: None,
            tapada: None,
        };
        // Trazo limpio de captura de pantalla: sin el temblor «artista» de
        // Excalidraw, que sobre una foto parece un error.
        a.gesto.estilo.rugosidad = 0.0;
        // Sin herramienta hasta que se coja una: el gesto nace con el lapiz.
        a.gesto.tomar_herramienta(Herramienta::Mano);
        a.aplicar_estilo();
        a
    }

    pub fn origen(&self) -> Punto {
        self.origen
    }

    /// Si no se ha dibujado nada (o se deshizo todo).
    pub fn vacia(&self) -> bool {
        self.escena.visibles().next().is_none()
    }

    pub fn escribiendo(&self) -> bool {
        self.gesto.esta_escribiendo()
    }

    /// Coge una herramienta; la misma otra vez la suelta.
    pub fn tomar(&mut self, u: Util) {
        if self.gesto.esta_escribiendo() {
            self.gesto.cerrar_texto(&mut self.escena);
        }
        if self.util == Some(u) {
            self.util = None;
            self.gesto.tomar_herramienta(Herramienta::Mano);
        } else {
            self.util = Some(u);
            self.gesto.tomar_herramienta(herramienta_de(u));
        }
        self.gesto.seleccion.limpiar();
    }

    pub fn poner_color(&mut self, i: usize) {
        self.color = i.min(COLORES.len() - 1);
        self.aplicar_estilo();
    }

    pub fn poner_grosor(&mut self, i: usize) {
        self.grosor = i.min(2);
        self.aplicar_estilo();
    }

    fn aplicar_estilo(&mut self) {
        self.gesto.estilo.trazo = color_de(self.color);
        let (nivel, tinta) = match self.grosor {
            0 => (NivelGrosor::Fino, pixpin_motor2d::tinta::GROSOR_FINO),
            1 => (NivelGrosor::Medio, pixpin_motor2d::tinta::GROSOR_MEDIO),
            _ => (NivelGrosor::Grueso, pixpin_motor2d::tinta::GROSOR_GRUESO),
        };
        self.gesto.estilo.grosor = nivel;
        self.gesto.grosor_tinta = tinta;
        // El texto crece con el grosor, como un rotulador mas gordo.
        self.gesto.estilo.tamano_letra = match self.grosor {
            0 => 16.0,
            1 => 20.0,
            _ => 28.0,
        };
    }

    fn al_dibujo(&self, p: Punto) -> Punto2 {
        Punto2::nuevo(
            (p.x - self.origen.x) as f32 / self.zoom,
            (p.y - self.origen.y) as f32 / self.zoom,
        )
    }

    fn evento(&mut self, ev: EventoGesto) {
        let ev = crate::dibujo::teclas::con_modificadores(ev);
        let _ = self.gesto.evento(ev, &mut self.escena, 1.0 / self.zoom);
    }

    pub fn pulsar(&mut self, p: Punto) {
        let q = self.al_dibujo(p);
        self.evento(EventoGesto::Pulsar {
            p: q,
            shift: false,
            alt: false,
            presion: None,
        });
    }

    pub fn mover(&mut self, p: Punto) {
        let q = self.al_dibujo(p);
        self.evento(EventoGesto::Mover {
            p: q,
            shift: false,
            alt: false,
            presion: None,
        });
    }

    pub fn soltar(&mut self, p: Punto) {
        let q = self.al_dibujo(p);
        self.evento(EventoGesto::Soltar { p: q });
    }

    pub fn deshacer(&mut self) {
        if self.gesto.esta_escribiendo() {
            self.gesto.cerrar_texto(&mut self.escena);
        }
        self.evento(EventoGesto::Deshacer);
    }

    pub fn rehacer(&mut self) {
        self.evento(EventoGesto::Rehacer);
    }

    /// Borra lo elegido (Supr), si no se esta escribiendo.
    pub fn suprimir(&mut self) {
        if !self.gesto.esta_escribiendo() {
            self.evento(EventoGesto::Suprimir);
        }
    }

    /// Una letra escrita en el texto que se esta escribiendo.
    pub fn caracter(&mut self, c: char) -> bool {
        if c.is_control() {
            return false;
        }
        self.gesto.escribir(c, &mut self.escena)
    }

    /// Las teclas de edicion mientras se escribe. Devuelve si la uso.
    pub fn tecla_de_texto(&mut self, vk: u32) -> bool {
        if !self.gesto.esta_escribiendo() {
            return false;
        }
        let t = match vk {
            VK_BACK => TeclaTexto::Retroceso,
            VK_DELETE => TeclaTexto::Suprimir,
            VK_RETURN => TeclaTexto::Entrar,
            VK_LEFT => TeclaTexto::Izquierda,
            VK_RIGHT => TeclaTexto::Derecha,
            VK_HOME => TeclaTexto::Inicio,
            VK_END => TeclaTexto::Fin,
            _ => return false,
        };
        self.gesto.tecla_de_texto(t, &mut self.escena);
        true
    }

    /// Termina el texto que se escribe (Esc). Devuelve si habia uno.
    pub fn cerrar_texto(&mut self) -> bool {
        self.gesto.cerrar_texto_con_teclado(&mut self.escena)
    }

    fn tapados(&self) -> Vec<Tapado> {
        mosaico::tapados(&self.escena.elementos)
            .into_iter()
            .map(|(_, t)| t)
            .collect()
    }

    /// Antes de pintar: si los mosaicos cambiaron y no se esta arrastrando,
    /// se rehace la foto tapada. `bajar_foto` trae la foto de la zona la
    /// primera vez.
    pub fn preparar(
        &mut self,
        motor: &MotorRender,
        bajar_foto: impl FnOnce() -> Option<ImagenRgba>,
    ) {
        self.imagenes.asegurar(motor);
        let tapados = self.tapados();
        if tapados.is_empty() {
            self.tapada = None;
            return;
        }
        if !self.gesto.en_reposo() {
            return;
        }
        if self.tapada.as_ref().is_some_and(|(t, _)| *t == tapados) {
            return;
        }
        if self.foto.is_none() {
            self.foto = bajar_foto();
        }
        let Some(foto) = &self.foto else {
            return;
        };
        let mut px = foto.pixeles.clone();
        tapar(&mut px, foto.ancho, foto.alto, &tapados, self.zoom);
        match motor.bitmap_desde_pixeles(foto.ancho, foto.alto, &px) {
            Ok(b) => self.tapada = Some((tapados, b)),
            Err(e) => tracing::warn!(?e, "no se pudo subir la foto tapada"),
        }
    }

    /// Pinta lo anotado sobre la zona. `zona` es la zona en coordenadas de
    /// la ventana que pinta.
    pub fn pintar(&mut self, p: &Pintor<'_>, zona: RectF) {
        p.con_recorte(zona, |p| {
            let tapados = self.tapados();
            let cocidos: &[Tapado] = match &self.tapada {
                Some((t, b)) => {
                    p.bitmap(b, zona, None, true);
                    t
                }
                None => &[],
            };
            p.poner_vista((zona.x, zona.y), self.zoom, (0.0, 0.0));
            // Los mosaicos que aun no estan en la foto (el que se arrastra):
            // su banda maciza, que tapa igual.
            for t in tapados.iter().filter(|t| !cocidos.contains(t)) {
                let (x0, y0, x1, y1) = t.caja;
                p.rellenar(
                    RectF {
                        x: x0,
                        y: y0,
                        ancho: x1 - x0,
                        alto: y1 - y0,
                    },
                    crate::dibujo::pintar::a_color(mosaico::TAPA_MACIZA),
                );
            }
            let vista = (0.0, 0.0, zona.ancho / self.zoom, zona.alto / self.zoom);
            crate::dibujo::pintar::pintar_escena(
                p,
                &self.escena,
                &mut self.cache,
                &mut self.cache_tinta,
                &self.imagenes,
                vista,
                self.zoom,
                |_| false,
            );
            crate::dibujo::pintar::pintar_encima(
                p,
                &self.gesto,
                &self.escena,
                vista,
                self.zoom,
                &self.imagenes,
                false,
                false,
            );
            p.desplazar(0.0, 0.0);
        });
    }

    /// **Lo anotado, dentro de la captura**: los mosaicos tapan la foto y lo
    /// demas se pinta encima, con la GPU y a la misma escala que se veia.
    pub fn hornear(
        &mut self,
        imagen: &mut ImagenRgba,
        motor: &MotorRender,
        d3d: &ID3D11Device,
    ) -> Result<()> {
        if self.gesto.esta_escribiendo() {
            self.gesto.cerrar_texto(&mut self.escena);
        }
        let tapados = self.tapados();
        tapar(
            &mut imagen.pixeles,
            imagen.ancho,
            imagen.alto,
            &tapados,
            self.zoom,
        );
        let hay_trazos = self
            .escena
            .visibles()
            .any(|e| !matches!(e.figura, Figura::Mosaico { .. }));
        if !hay_trazos {
            return Ok(());
        }
        let (w, h) = (imagen.ancho, imagen.alto);
        let fondo = motor
            .bitmap_desde_pixeles(w, h, &imagen.pixeles)
            .context("no se pudo subir la captura para anotarla")?;
        let fuera = pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(motor, d3d, w, h)
            .context("sin superficie para hornear lo anotado")?;
        self.imagenes.asegurar(motor);
        let zoom = self.zoom;
        let vista = (0.0, 0.0, w as f32 / zoom, h as f32 / zoom);
        let (escena, cache, cache_tinta, imagenes) = (
            &self.escena,
            &mut self.cache,
            &mut self.cache_tinta,
            &self.imagenes,
        );
        motor
            .dibujar(&fuera.destino, |p| {
                p.bitmap(
                    &fondo,
                    RectF {
                        x: 0.0,
                        y: 0.0,
                        ancho: w as f32,
                        alto: h as f32,
                    },
                    None,
                    true,
                );
                p.poner_vista((0.0, 0.0), zoom, (0.0, 0.0));
                crate::dibujo::pintar::pintar_escena(
                    p,
                    escena,
                    cache,
                    cache_tinta,
                    imagenes,
                    vista,
                    zoom,
                    |_| true,
                );
            })
            .context("no se pudo pintar lo anotado")?;
        fuera.esperar_gpu().context("la GPU no acabo")?;
        let (_, _, px) = fuera.leer_rgba().context("no se pudo leer lo anotado")?;
        if px.len() == imagen.pixeles.len() {
            imagen.pixeles = px;
        }
        Ok(())
    }

    /// Cuanto se ha dibujado (para las pruebas).
    #[cfg(test)]
    pub fn cuantos(&self) -> usize {
        self.escena.visibles().count()
    }
}

/// Tapa en el sitio cada mosaico (en unidades del dibujo) sobre unos
/// pixeles fisicos a `zoom` pixeles por unidad.
fn tapar(px: &mut [u8], ancho: u32, alto: u32, tapados: &[Tapado], zoom: f32) {
    for t in tapados {
        let (x0, y0, x1, y1) = t.caja;
        let caja = (x0 * zoom, y0 * zoom, x1 * zoom, y1 * zoom);
        let lado = mosaico::lado_en_pantalla(t.grano, zoom);
        mosaico::tapar_rgba(px, ancho, alto, caja, lado, t.desenfoque);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn coger_la_misma_herramienta_dos_veces_la_suelta() {
        let mut a = Anotacion::nueva(Punto { x: 100, y: 100 }, 100);
        a.tomar(Util::Flecha);
        assert_eq!(a.util, Some(Util::Flecha));
        assert_eq!(a.gesto.herramienta, Herramienta::Flecha);
        a.tomar(Util::Flecha);
        assert_eq!(a.util, None, "caso negativo: la segunda vez la suelta");
        assert_eq!(a.gesto.herramienta, Herramienta::Mano);
    }

    #[test]
    fn un_rectangulo_se_dibuja_en_coordenadas_de_la_zona_y_se_deshace() {
        let mut a = Anotacion::nueva(Punto { x: 100, y: 200 }, 150);
        a.tomar(Util::Rectangulo);
        a.poner_color(3);
        a.pulsar(Punto { x: 130, y: 230 });
        a.mover(Punto { x: 280, y: 380 });
        a.soltar(Punto { x: 280, y: 380 });
        assert_eq!(a.cuantos(), 1);
        let e = a.escena.visibles().next().unwrap();
        // 30 px fisicos al 150 % son 20 unidades del dibujo.
        assert!(
            (e.x - 20.0).abs() < 0.01 && (e.y - 20.0).abs() < 0.01,
            "{} {}",
            e.x,
            e.y
        );
        assert!((e.ancho - 100.0).abs() < 0.01);
        assert_eq!(e.trazo, color_de(3));
        assert!(!a.vacia());
        a.deshacer();
        assert!(a.vacia(), "deshacer lo quita");
        a.rehacer();
        assert!(!a.vacia(), "rehacer lo devuelve");
    }

    #[test]
    fn sin_herramienta_el_clic_no_dibuja() {
        // Caso negativo: sin coger nada, arrastrar no deja rastro.
        let mut a = Anotacion::nueva(Punto { x: 0, y: 0 }, 100);
        a.pulsar(Punto { x: 10, y: 10 });
        a.mover(Punto { x: 90, y: 90 });
        a.soltar(Punto { x: 90, y: 90 });
        assert!(a.vacia());
    }

    #[test]
    fn el_texto_se_escribe_y_las_teclas_de_texto_solo_valen_escribiendo() {
        let mut a = Anotacion::nueva(Punto { x: 0, y: 0 }, 100);
        assert!(
            !a.tecla_de_texto(VK_BACK),
            "caso negativo: sin texto abierto"
        );
        a.tomar(Util::Texto);
        a.pulsar(Punto { x: 40, y: 40 });
        a.soltar(Punto { x: 40, y: 40 });
        assert!(a.escribiendo());
        for c in "Revisar".chars() {
            a.caracter(c);
        }
        assert!(a.tecla_de_texto(VK_BACK));
        assert!(a.cerrar_texto());
        assert!(!a.escribiendo());
        let t = a
            .escena
            .visibles()
            .find_map(|e| match &e.figura {
                Figura::Texto { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(t, "Revisa");
    }

    #[test]
    fn el_mosaico_tapa_los_pixeles_de_su_caja_y_no_los_de_fuera() {
        let (w, h) = (64u32, 32u32);
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let v = ((x * 7 + y * 13) % 256) as u8;
                px.extend_from_slice(&[v, 255 - v, v / 2, 255]);
            }
        }
        let original = px.clone();
        let t = Tapado {
            caja: (0.0, 0.0, 16.0, 16.0),
            grano: 8.0,
            desenfoque: false,
        };
        tapar(&mut px, w, h, &[t], 1.0);
        let i = |x: u32, y: u32| ((y * w + x) * 4) as usize;
        // Dentro: un cuadro de 8 es del mismo color entero.
        assert_eq!(px[i(0, 0)..i(0, 0) + 4], px[i(7, 7)..i(7, 7) + 4]);
        assert_ne!(px[i(0, 0)..i(0, 0) + 4], original[i(0, 0)..i(0, 0) + 4]);
        // Caso negativo: fuera de la caja nada cambia.
        assert_eq!(
            px[i(40, 20)..i(40, 20) + 4],
            original[i(40, 20)..i(40, 20) + 4]
        );
    }
}
