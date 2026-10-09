//! **Anotar encima del plano** con el motor del lienzo (como PixPin Android
//! v0.114: «anotar en el visor de planos usa las herramientas del motor del
//! lienzo, sin herramientas propias»; y lo que el usuario recordo el
//! 9-oct-2026: el motor de edicion tiene su desarrollo para la maxima
//! fluidez, asi que se integra el que hay, no se hace otra tinta).
//!
//! El visor no sabe nada del motor: le pasa los eventos de su ventana y le
//! deja pintar encima de cada fotograma, en el mismo dispositivo y la misma
//! pasada (asi la tinta no se separa del plano al mover o acercar). Quien lo
//! implementa es la aplicacion (`plano_cad::anotar`), con el anfitrion del
//! lector (`lector_tinta::Tinta`) y la capa `anot-<uid>`.
//!
//! **Las coordenadas de la capa** son las de Android: una unidad es la
//! milesima del lado mayor del plano y la y baja. Salen del propio plano,
//! asi que la capa cae en su sitio en cualquier aparato sin guardar nada mas.

use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};

use crate::modelo::Modelo;

/// La milesima del lado mayor del plano (unidades del plano por unidad de la capa).
pub fn unidad_de_la_capa(m: &Modelo) -> f64 {
    let u = ((m.caja[2] - m.caja[0]).max(m.caja[3] - m.caja[1]) as f64) / 1000.0;
    if u > 0.0 && u.is_finite() { u } else { 1.0 }
}

/// Del lienzo de la capa (y baja) al plano (y sube), y al reves.
pub fn de_la_capa(p: [f64; 2], u: f64) -> [f64; 2] {
    [p[0] * u, -p[1] * u]
}

pub fn a_la_capa(p: [f64; 2], u: f64) -> [f64; 2] {
    [p[0] / u, -p[1] / u]
}

/// Lo que ve la ventana del plano, para el motor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VistaPlano {
    /// El punto del plano (respecto a su origen) en el centro de la ventana.
    pub centro: [f64; 2],
    /// Unidades del plano por pixel.
    pub px: f64,
    pub ancho: u32,
    pub alto: u32,
    /// Donde empieza el area de la ventana en el escritorio (los eventos van
    /// en coordenadas del escritorio, como los de las ventanas del motor).
    pub origen_pantalla: (i32, i32),
    /// 100 a 100 %, 150 a 150 %...
    pub escala_por_cien: u32,
    /// [`unidad_de_la_capa`].
    pub unidad: f64,
    pub claro: bool,
}

impl VistaPlano {
    /// La camara del motor: el punto de la capa en la esquina de arriba a
    /// la izquierda y los pixeles por unidad de la capa.
    pub fn camara(&self) -> (f64, f64, f64) {
        let x = (self.centro[0] - self.ancho as f64 / 2.0 * self.px) / self.unidad;
        let y = -(self.centro[1] + self.alto as f64 / 2.0 * self.px) / self.unidad;
        (x, y, self.unidad / self.px)
    }
}

/// Un evento de la ventana del plano, en coordenadas del escritorio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventoPlano {
    Mover(i32, i32),
    Bajar(i32, i32),
    Subir(i32, i32),
    BajarCentral(i32, i32),
    SubirCentral(i32, i32),
    Rueda(i32),
    Tecla { vk: u32, shift: bool, ctrl: bool, alt: bool },
    TeclaSoltada(u32),
    Caracter(char),
}

/// Lo que dice el motor de un evento.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Respuesta {
    /// Era suyo: el visor no hace nada mas con el.
    pub consumido: bool,
    /// Se pulso «Salir» en su barra: dejar de anotar.
    pub salir: bool,
    /// Empezo un arrastre: capturar el raton.
    pub arrastra: bool,
}

/// **El motor del lienzo, visto desde el visor.**
pub trait Anotador {
    /// Un evento mientras se anota.
    fn evento(&mut self, ev: EventoPlano, vista: &VistaPlano) -> Respuesta;
    /// Pinta lo anotado (siempre) y, si `anotando`, la barra y el panel del
    /// motor, encima del fotograma ya hecho (`destino`, el bufer de la
    /// ventana) y antes de presentarlo.
    fn pintar(&mut self, dispositivo: &ID3D11Device, destino: &ID3D11Texture2D, vista: &VistaPlano, anotando: bool);
    /// Dejar de anotar: cerrar lo que se escribia y guardar la capa.
    fn terminar(&mut self);
    /// Si quiere el Esc (cerrar un texto, soltar lo elegido) antes que el visor.
    fn quiere_escape(&self) -> bool;
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_capa_y_el_plano_van_y_vuelven() {
        let u = 0.25;
        let p = [12.0, -7.5];
        let q = a_la_capa(p, u);
        // En la capa la y baja.
        assert_eq!(q, [48.0, 30.0]);
        assert_eq!(de_la_capa(q, u), p);
    }

    #[test]
    fn la_unidad_es_la_milesima_del_lado_mayor() {
        let m = Modelo { caja: [0.0, 0.0, 500.0, 2000.0], ..Default::default() };
        assert!((unidad_de_la_capa(&m) - 2.0).abs() < 1e-9);
        // Caso negativo: un plano sin caja no da una unidad absurda.
        assert_eq!(unidad_de_la_capa(&Modelo::default()), 1.0);
    }

    #[test]
    fn la_camara_del_motor_pone_el_mismo_punto_en_el_mismo_pixel() {
        let v = VistaPlano { centro: [100.0, 50.0], px: 0.5, ancho: 800, alto: 600, origen_pantalla: (0, 0), escala_por_cien: 100, unidad: 2.0, claro: false };
        let (x, y, zoom) = v.camara();
        // Un punto del plano: su pixel por la camara del plano y por la del motor.
        let p = [130.0, 20.0];
        let px_plano = [(p[0] - v.centro[0]) / v.px + 400.0, 300.0 - (p[1] - v.centro[1]) / v.px];
        let q = a_la_capa(p, v.unidad);
        let px_motor = [(q[0] - x) * zoom, (q[1] - y) * zoom];
        assert!((px_plano[0] - px_motor[0]).abs() < 1e-9 && (px_plano[1] - px_motor[1]).abs() < 1e-9);
    }
}
