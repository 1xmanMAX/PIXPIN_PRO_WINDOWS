//! **Lo comun de las ventanitas de la voz** (Pronunciar, la conversacion por
//! turnos y la llamada secreta): los colores de «papel oscuro» del
//! telepronter, un boton pintado que se apunta para el raton y el marco
//! centrado en el monitor.
//!
//! El telepronter lleva su propia copia de esto desde antes; las tres
//! nuevas comparten esta para no repetirla tres veces mas.

#![forbid(unsafe_code)]

use pixpin_geom::Rect;
use pixpin_render::{Color, Pintor, RectF};

use crate::caja_dibujo::hex;

pub const FONDO: Color = hex(0x121316);
pub const TEXTO: Color = hex(0xe4e2e6);
pub const APAGADO: Color = hex(0x9a9aa2);
pub const CRISTAL: Color = hex(0x26262c);
pub const ROJO: Color = hex(0xe05a4f);
pub const VERDE: Color = hex(0x2fb344);
pub const AZUL: Color = hex(0x1e88e5);
pub const DORADO: Color = hex(0xe8c06a);
const BOTON: Color = hex(0x2c2c34);
const BOTON_ENCIMA: Color = hex(0x36363e);

/// Los botones pintados en este fotograma, para saber cual hay bajo el
/// raton. Se vacian al empezar cada fotograma.
pub struct Botones<A: Copy> {
    v: Vec<(RectF, A)>,
    pub raton: (f32, f32),
}

impl<A: Copy> Default for Botones<A> {
    fn default() -> Self {
        Botones {
            v: Vec::new(),
            raton: (-1.0, -1.0),
        }
    }
}

pub fn dentro(r: RectF, p: (f32, f32)) -> bool {
    p.0 >= r.x && p.0 <= r.x + r.ancho && p.1 >= r.y && p.1 <= r.y + r.alto
}

impl<A: Copy> Botones<A> {
    pub fn vaciar(&mut self) {
        self.v.clear();
    }

    /// Apunta una zona pulsable sin pintarla (un circulo, una ficha).
    pub fn zona(&mut self, caja: RectF, que: A) {
        self.v.push((caja, que));
    }

    /// Pinta un boton con su rotulo centrado y lo apunta. `color` es el
    /// fondo si esta encendido (rojo de grabar, verde de contestar…).
    pub fn boton(
        &mut self,
        p: &Pintor,
        caja: RectF,
        que: A,
        rotulo: &str,
        color: Option<Color>,
        escala: f32,
    ) {
        let fondo = match color {
            Some(c) => c,
            None if dentro(caja, self.raton) => BOTON_ENCIMA,
            None => BOTON,
        };
        p.rellenar_redondeado(caja, 8.0 * escala, fondo);
        let tam = 15.0 * escala;
        let (w, h) = p.medir_texto(rotulo, tam);
        p.texto(
            rotulo,
            caja.x + (caja.ancho - w) / 2.0,
            caja.y + (caja.alto - h) / 2.0,
            tam,
            TEXTO,
        );
        self.v.push((caja, que));
    }

    /// La ultima zona apuntada bajo el raton (las de encima se apuntan
    /// despues).
    pub fn bajo_el_raton(&self) -> Option<A> {
        self.v
            .iter()
            .rev()
            .find(|(r, _)| dentro(*r, self.raton))
            .map(|(_, a)| *a)
    }
}

/// Un marco de `ancho` x `alto` (logicos) centrado en `area`, sin salirse.
pub fn centrado(area: Rect, ancho: u32, alto: u32, escala_por_cien: u32) -> Rect {
    let w = (ancho * escala_por_cien / 100).min(area.ancho);
    let h = (alto * escala_por_cien / 100).min(area.alto);
    Rect {
        x: area.x + (area.ancho - w) as i32 / 2,
        y: area.y + (area.alto - h) as i32 / 2,
        ancho: w,
        alto: h,
    }
}

/// Un texto centrado a lo ancho de `caja`, con la y dada.
pub fn centrado_en(p: &Pintor, texto: &str, caja: RectF, y: f32, tam: f32, color: Color) {
    let (w, _) = p.medir_texto(texto, tam);
    p.texto(texto, caja.x + (caja.ancho - w) / 2.0, y, tam, color);
}

/// **Una pantalla pintada de verdad, en PNG**, para mirarla sin abrir la
/// ventana (necesita GPU). Deja el fichero en `PIXPIN_MUESTRAS` o en la
/// carpeta temporal y devuelve su ruta.
/// **Cierra una grabacion en otro hilo**: parar y escribir el final del
/// `.m4a` tarda 25-50 ms (medido), y en Pronunciar se hace cada vez que se
/// suelta la tecla.
pub fn cerrar_microfono(
    g: pixpin_audio::Grabadora,
) -> std::sync::mpsc::Receiver<Result<pixpin_audio::Grabacion, pixpin_audio::ErrorAudio>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let lanzado = std::thread::Builder::new()
        .name("cerrar-microfono".into())
        .spawn(move || {
            let _ = tx.send(g.parar());
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de cerrar el microfono");
    }
    rx
}

/// **Abre el microfono en otro hilo** y devuelve la grabadora por un canal.
///
/// Medido en este equipo: `Grabadora::empezar` tarda ~0,5 s la primera vez
/// (arranca Media Foundation, el codificador AAC y el microfono). En el hilo
/// de una ventana eso es medio segundo sin pintar ni atender teclas; aqui la
/// ventana sigue y recoge la grabadora cuando llega.
pub fn abrir_microfono(
    fichero: std::path::PathBuf,
) -> std::sync::mpsc::Receiver<Result<pixpin_audio::Grabadora, pixpin_audio::ErrorAudio>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let lanzado = std::thread::Builder::new()
        .name("abrir-microfono".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let _ = tx.send(pixpin_audio::Grabadora::empezar(&fichero));
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del microfono");
    }
    rx
}

#[cfg(test)]
pub fn muestra(
    nombre: &str,
    ancho: u32,
    alto: u32,
    pintar: impl FnOnce(&Pintor, &pixpin_render::MotorRender),
) -> std::path::PathBuf {
    use pixpin_render::MotorRender;
    use pixpin_render::fuera_de_pantalla::FueraDePantalla;
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ancho, alto).expect("superficie");
    motor
        .dibujar(&fuera.destino, |p| pintar(p, &motor))
        .expect("pintar");
    fuera.esperar_gpu().expect("esperar");
    let (_, _, pixeles) = fuera.leer_rgba().expect("leer");
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
        ancho,
        alto,
        pixeles,
    })
    .expect("codificar");
    let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let ruta = carpeta.join(format!("{nombre}.png"));
    std::fs::write(&ruta, png).expect("guardar");
    println!("{nombre}: {}", ruta.display());
    ruta
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_marco_centrado_no_se_sale_del_monitor() {
        let area = Rect {
            x: 1920,
            y: 0,
            ancho: 1280,
            alto: 720,
        };
        let m = centrado(area, 1000, 600, 100);
        assert_eq!((m.x, m.y, m.ancho, m.alto), (2060, 60, 1000, 600));
        let grande = centrado(area, 1000, 600, 200);
        assert_eq!(
            (grande.ancho, grande.alto),
            (1280, 720),
            "se recorta al monitor"
        );
        assert_eq!(grande.x, 1920);
    }

    #[test]
    fn gana_la_zona_de_encima_y_fuera_de_todas_no_hay_ninguna() {
        let mut b: Botones<u8> = Botones::default();
        let r = RectF {
            x: 0.0,
            y: 0.0,
            ancho: 10.0,
            alto: 10.0,
        };
        b.zona(r, 1);
        b.zona(r, 2);
        b.raton = (5.0, 5.0);
        assert_eq!(b.bajo_el_raton(), Some(2));
        b.raton = (50.0, 5.0);
        assert_eq!(b.bajo_el_raton(), None);
        b.vaciar();
        b.raton = (5.0, 5.0);
        assert_eq!(b.bajo_el_raton(), None);
    }
}
