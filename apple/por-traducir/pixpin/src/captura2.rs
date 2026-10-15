//! **La captura v2** (maquetas `Captura2-elegir` y `Captura2-despues`).
//!
//! Lo que el overlay de siempre (`overlay.rs`) gana con el rediseno:
//!
//! - **Al elegir**: la barra grande de abajo con los siete modos y su letra
//!   (Zona Z, Ventana V, Pantalla P, Con scroll S, Grabar GIF G, Pin en vivo
//!   L, Texto T), las medidas que se pueden teclear, las proporciones 16:9,
//!   4:3 y 1:1, «Repetir la ultima zona» (R), la ventana bajo el raton
//!   resaltada con su nombre, la fila de los gestos con Alt y la lupa con el
//!   codigo de color (C lo copia, Mayus cambia HEX/RGB).
//! - **Despues de elegir**: «Copiar» primero (Enter), Pinear (Ctrl P), Al
//!   chat con el selector de proyecto en un paso (Ctrl M), Texto por OCR
//!   (Ctrl T) y Guardar (Ctrl S); y la barra de anotar encima de la zona,
//!   sin abrir otra ventana: lapiz, flecha, rectangulo, texto y mosaico
//!   (1-5), cinco colores, tres grosores, deshacer y rehacer.
//!
//! Aqui viven el estado de la sesion y lo que decide cada tecla; donde cae
//! cada cosa esta en `disposicion` (puro y probado), el dibujo en `pintar`,
//! lo anotado en `anotar` y el envio al chat en `al_chat`.

pub mod al_chat;
pub mod anotar;
pub mod disposicion;
#[cfg(test)]
mod muestras;
pub mod pintar;

use pixpin_geom::{Punto, Rect};
use pixpin_shell::ventanas_visibles::VentanaVisible;
use pixpin_store::Catalogo;
use pixpin_ui::FormatoColorLupa;

use crate::overlay::ModoConfirmacion;
use anotar::Anotacion;
use disposicion::{Cifras, Modo, Proporcion, Util, filtrar};

/// Los rotulos, ya traducidos: el overlay no conoce el catalogo.
#[derive(Debug, Clone, Default)]
pub struct Textos {
    pub modos: [String; 7],
    pub libre: String,
    pub repetir: String,
    pub ancho: String,
    pub alto: String,
    pub sin_abrir: String,
    pub gestos: [(String, String); 4],
    pub salir: String,
    pub pista_ventana: String,
    pub pista_zona: String,
    pub copiar_color: String,
    pub cambiar_formato: String,
    pub acciones: [String; 6],
    pub buscar_proyecto: String,
    pub recientes: String,
    pub el_ultimo: String,
    pub comentario: String,
    pub enviar: String,
    pub sin_proyectos: String,
}

impl Textos {
    pub fn de(t: &Catalogo) -> Textos {
        let s = |k: &str| t.t(k);
        Textos {
            modos: [
                s("captura2-zona"),
                s("captura2-ventana"),
                s("captura2-pantalla"),
                s("captura2-scroll"),
                s("captura2-gif"),
                s("captura2-pin-vivo"),
                s("captura2-texto"),
            ],
            libre: s("captura2-libre"),
            repetir: s("captura2-repetir"),
            ancho: s("captura2-ancho"),
            alto: s("captura2-alto"),
            sin_abrir: s("captura2-sin-abrir"),
            gestos: [
                (s("captura2-arrastrar-izq"), s("captura2-gesto-copiar")),
                (s("captura2-arrastrar-der"), s("captura2-gesto-pinear")),
                (s("captura2-arrastrar-centro"), s("captura2-gesto-pin-vivo")),
                (s("captura2-doble-centro"), s("captura2-gesto-anotar")),
            ],
            salir: s("captura2-salir"),
            pista_ventana: s("captura2-pista-ventana"),
            pista_zona: s("captura2-pista-zona"),
            copiar_color: s("captura2-copiar-color"),
            cambiar_formato: s("captura2-cambiar-formato"),
            acciones: [
                s("captura2-copiar"),
                s("captura2-pinear"),
                s("captura2-al-chat"),
                s("captura2-ocr"),
                s("captura2-guardar"),
                s("captura2-descartar"),
            ],
            buscar_proyecto: s("captura2-buscar-proyecto"),
            recientes: s("captura2-recientes"),
            el_ultimo: s("captura2-el-ultimo"),
            comentario: s("captura2-comentario"),
            enviar: s("captura2-enviar"),
            sin_proyectos: s("captura2-sin-proyectos"),
        }
    }

    pub fn modo(&self, m: Modo) -> &str {
        let i = Modo::TODOS.iter().position(|x| *x == m).unwrap_or(0);
        &self.modos[i]
    }

    pub fn accion(&self, a: disposicion::Accion) -> &str {
        let i = disposicion::Accion::TODAS
            .iter()
            .position(|x| *x == a)
            .unwrap_or(0);
        &self.acciones[i]
    }
}

/// Lo que el ejecutable sabe y el overlay no.
#[derive(Debug, Clone, Default)]
pub struct Contexto {
    pub textos: Textos,
    /// La ultima zona capturada en esta sesion de la app, para «Repetir».
    pub ultima_region: Option<Rect>,
    /// La carpeta de datos, para leer los proyectos de «Al chat» cuando se
    /// abre el selector (no antes: abrir la captura no lee nada de disco).
    pub raiz: Option<std::path::PathBuf>,
    /// El codigo de este equipo, por si hay que crear «Mensajes guardados».
    pub aparato: String,
    /// Si el gancho de los gestos con Alt esta puesto: sin el, la fila de
    /// pistas mentiria.
    pub gestos: bool,
}

/// Que campo de texto tiene el teclado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo {
    Ancho,
    Alto,
}

/// El selector de proyecto abierto.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EstadoSelector {
    pub busqueda: String,
    pub comentario: String,
    /// La fila elegida (con las flechas o el raton), entre las filtradas.
    pub elegida: usize,
    /// Si se escribe en el comentario (si no, en la busqueda).
    pub en_comentario: bool,
}

impl EstadoSelector {
    /// Los proyectos que se ven ahora, como mucho `FILAS_SELECTOR`.
    pub fn visibles<'a>(&self, proyectos: &'a [(String, String)]) -> Vec<&'a (String, String)> {
        let mut v = filtrar(proyectos, &self.busqueda);
        v.truncate(disposicion::FILAS_SELECTOR);
        v
    }

    /// El id del proyecto al que iria la captura ahora.
    pub fn destino(&self, proyectos: &[(String, String)]) -> Option<String> {
        let v = self.visibles(proyectos);
        v.get(self.elegida.min(v.len().saturating_sub(1)))
            .map(|(id, _)| id.clone())
    }

    pub fn escribir(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        if self.en_comentario {
            if self.comentario.chars().count() < 500 {
                self.comentario.push(c);
            }
        } else if self.busqueda.chars().count() < 60 {
            self.busqueda.push(c);
            self.elegida = 0;
        }
    }

    pub fn borrar(&mut self) {
        if self.en_comentario {
            self.comentario.pop();
        } else {
            self.busqueda.pop();
            self.elegida = 0;
        }
    }

    pub fn mover(&mut self, delta: i32, cuantas: usize) {
        if cuantas == 0 {
            self.elegida = 0;
            return;
        }
        let n = cuantas as i32;
        self.elegida = (self.elegida as i32 + delta).rem_euclid(n) as usize;
    }
}

/// Todo lo de la captura v2 que dura lo que dura un overlay.
pub struct Sesion {
    pub contexto: Contexto,
    pub modo: Modo,
    /// Con que se abrio el overlay: es lo que hacen Zona, Ventana y
    /// Pantalla al confirmar (preguntar, copiar directo o pinear).
    pub base: ModoConfirmacion,
    /// Si se ensena lo nuevo. No con un gesto (soltar ya confirma) ni con el
    /// cuentagotas (no hay zona que elegir).
    pub activa: bool,
    pub proporcion: Proporcion,
    pub campo: Option<Campo>,
    pub ancho: Cifras,
    pub alto: Cifras,
    pub ventanas: Vec<VentanaVisible>,
    pub formato: FormatoColorLupa,
    /// Mayus se pulso sola (sin otra tecla en medio): al soltarla cambia el
    /// formato del color.
    pub mayus_sola: bool,
    pub anotacion: Option<Anotacion>,
    /// Lo que se recuerda de las herramientas aunque la anotacion se rehaga
    /// (la zona se movio antes de dibujar nada).
    pub util: Option<Util>,
    pub color: usize,
    pub grosor: usize,
    pub selector: Option<EstadoSelector>,
    /// Los proyectos del selector, leidos al abrirlo por primera vez.
    pub proyectos: Vec<(String, String)>,
    /// El destino elegido en el selector, para cuando se cierre el overlay.
    pub al_chat: Option<(String, String)>,
    /// Un trazo de anotacion en curso.
    pub dibujando: bool,
    /// El ultimo pulsado cayo en un boton de lo nuevo: la soltada es suya.
    pub pulsado_ui: bool,
}

impl Sesion {
    pub fn nueva(
        contexto: Contexto,
        modo: ModoConfirmacion,
        gesto: bool,
        formato: FormatoColorLupa,
        ventanas: Vec<VentanaVisible>,
    ) -> Sesion {
        let (inicial, base) = match modo {
            ModoConfirmacion::Scroll => (Modo::Scroll, ModoConfirmacion::ConBarra),
            ModoConfirmacion::Gif => (Modo::Gif, ModoConfirmacion::ConBarra),
            ModoConfirmacion::PinEnVivo => (Modo::PinEnVivo, ModoConfirmacion::ConBarra),
            ModoConfirmacion::Texto => (Modo::Texto, ModoConfirmacion::ConBarra),
            otro => (Modo::Zona, otro),
        };
        Sesion {
            contexto,
            modo: inicial,
            base,
            activa: !gesto && modo != ModoConfirmacion::Cuentagotas,
            proporcion: Proporcion::Libre,
            campo: None,
            ancho: Cifras::default(),
            alto: Cifras::default(),
            ventanas,
            formato,
            mayus_sola: false,
            anotacion: None,
            util: None,
            color: 0,
            grosor: 1,
            selector: None,
            proyectos: Vec::new(),
            al_chat: None,
            dibujando: false,
            pulsado_ui: false,
        }
    }

    /// Lo que hace confirmar con el modo de ahora.
    pub fn confirmacion(&self, abierto_con: ModoConfirmacion) -> ModoConfirmacion {
        if !self.activa {
            return abierto_con;
        }
        match self.modo {
            Modo::Zona | Modo::Ventana | Modo::Pantalla => self.base,
            Modo::Scroll => ModoConfirmacion::Scroll,
            Modo::Gif => ModoConfirmacion::Gif,
            Modo::PinEnVivo => ModoConfirmacion::PinEnVivo,
            Modo::Texto => ModoConfirmacion::Texto,
        }
    }

    /// Si lo resaltado sale de UI Automation (los controles) o de aqui.
    pub fn usa_uia(&self) -> bool {
        !self.activa || !matches!(self.modo, Modo::Ventana | Modo::Pantalla)
    }

    /// Hay algo dibujado: la zona ya no se mueve ni cambia de modo.
    pub fn anotada(&self) -> bool {
        self.anotacion.as_ref().is_some_and(|a| !a.vacia())
    }

    /// La anotacion lista para dibujar sobre `zona`. Si existia vacia y la
    /// zona se movio, se rehace en su sitio nuevo.
    pub fn anotacion_en(&mut self, zona: Rect, escala: u32) -> &mut Anotacion {
        let origen = Punto {
            x: zona.x,
            y: zona.y,
        };
        let rehacer = match &self.anotacion {
            None => true,
            Some(a) => a.vacia() && a.origen() != origen,
        };
        if rehacer {
            let mut a = Anotacion::nueva(origen, escala);
            a.poner_color(self.color);
            a.poner_grosor(self.grosor);
            if let Some(u) = self.util {
                a.tomar(u);
            }
            self.anotacion = Some(a);
        }
        self.anotacion.as_mut().expect("recien puesta")
    }

    /// Coge (o suelta) una herramienta de anotar.
    pub fn tomar(&mut self, u: Util, zona: Rect, escala: u32) {
        let a = self.anotacion_en(zona, escala);
        a.tomar(u);
        self.util = a.util;
    }

    pub fn poner_color(&mut self, i: usize, zona: Rect, escala: u32) {
        self.color = i;
        self.anotacion_en(zona, escala).poner_color(i);
    }

    pub fn poner_grosor(&mut self, i: usize, zona: Rect, escala: u32) {
        self.grosor = i;
        self.anotacion_en(zona, escala).poner_grosor(i);
    }

    /// La ventana de mas arriba bajo `p`.
    pub fn ventana_en(&self, p: Punto) -> Option<&VentanaVisible> {
        pixpin_shell::ventanas_visibles::ventana_en(&self.ventanas, p)
    }

    /// Abre el selector de proyecto, leyendo la lista la primera vez. Si
    /// no hay ningun proyecto, se crea «Mensajes guardados», que es donde
    /// van las cosas sin proyecto (como en el chat).
    pub fn abrir_selector(&mut self) {
        if self.proyectos.is_empty() {
            if let Some(raiz) = &self.contexto.raiz {
                self.proyectos = al_chat::proyectos(raiz);
                if self.proyectos.is_empty() {
                    let ahora = pixpin_shell::entorno::ahora_utc_ms();
                    match pixpin_proyecto::almacen::asegurar_guardados(
                        raiz,
                        ahora,
                        &self.contexto.aparato,
                    ) {
                        Ok(_) => self.proyectos = al_chat::proyectos(raiz),
                        Err(e) => tracing::warn!(?e, "no se pudo preparar Mensajes guardados"),
                    }
                }
            }
        }
        self.selector = Some(EstadoSelector::default());
    }

    /// Mayus cambia entre HEX y RGB (y HSL si era el configurado).
    pub fn cambiar_formato(&mut self) {
        self.formato = match self.formato {
            FormatoColorLupa::Hex => FormatoColorLupa::Rgb,
            FormatoColorLupa::Rgb => FormatoColorLupa::Hex,
            FormatoColorLupa::Hsl => FormatoColorLupa::Hex,
        };
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn proyectos() -> Vec<(String, String)> {
        vec![
            ("g".into(), "Mensajes guardados".into()),
            ("t".into(), "Tesis".into()),
            ("o".into(), "Obra Miraflores".into()),
        ]
    }

    #[test]
    fn el_modo_de_apertura_elige_el_modo_de_la_barra() {
        let s = Sesion::nueva(
            Contexto::default(),
            ModoConfirmacion::Gif,
            false,
            FormatoColorLupa::Hex,
            vec![],
        );
        assert_eq!(s.modo, Modo::Gif);
        assert_eq!(s.confirmacion(ModoConfirmacion::Gif), ModoConfirmacion::Gif);
        let mut z = Sesion::nueva(
            Contexto::default(),
            ModoConfirmacion::Pinear,
            false,
            FormatoColorLupa::Hex,
            vec![],
        );
        assert_eq!(z.modo, Modo::Zona);
        // Zona con el atajo de pinear sigue pinando; Texto cambia a OCR.
        assert_eq!(
            z.confirmacion(ModoConfirmacion::Pinear),
            ModoConfirmacion::Pinear
        );
        z.modo = Modo::Texto;
        assert_eq!(
            z.confirmacion(ModoConfirmacion::Pinear),
            ModoConfirmacion::Texto
        );
    }

    #[test]
    fn con_un_gesto_o_el_cuentagotas_no_hay_barra_nueva() {
        // Caso negativo: el gesto confirma al soltar y manda su modo.
        let g = Sesion::nueva(
            Contexto::default(),
            ModoConfirmacion::DirectoAlPortapapeles,
            true,
            FormatoColorLupa::Hex,
            vec![],
        );
        assert!(!g.activa);
        assert_eq!(
            g.confirmacion(ModoConfirmacion::DirectoAlPortapapeles),
            ModoConfirmacion::DirectoAlPortapapeles
        );
        let c = Sesion::nueva(
            Contexto::default(),
            ModoConfirmacion::Cuentagotas,
            false,
            FormatoColorLupa::Hex,
            vec![],
        );
        assert!(!c.activa);
    }

    #[test]
    fn el_selector_filtra_mueve_y_da_el_destino() {
        let p = proyectos();
        let mut s = EstadoSelector::default();
        assert_eq!(s.destino(&p).as_deref(), Some("g"));
        s.mover(1, 3);
        assert_eq!(s.destino(&p).as_deref(), Some("t"));
        s.mover(-2, 3);
        assert_eq!(s.destino(&p).as_deref(), Some("o"), "da la vuelta");
        for c in "obra".chars() {
            s.escribir(c);
        }
        assert_eq!(s.elegida, 0, "buscar vuelve a la primera");
        assert_eq!(s.destino(&p).as_deref(), Some("o"));
        s.en_comentario = true;
        s.escribir('x');
        s.escribir('\u{8}');
        assert_eq!(s.comentario, "x", "un caracter de control no se escribe");
        s.en_comentario = false;
        for _ in 0..4 {
            s.borrar();
        }
        s.escribir('z');
        s.escribir('z');
        // Caso negativo: nada casa, no hay destino.
        assert_eq!(s.destino(&p), None);
    }

    #[test]
    fn la_anotacion_vacia_sigue_a_la_zona_y_la_llena_no() {
        let mut s = Sesion::nueva(
            Contexto::default(),
            ModoConfirmacion::ConBarra,
            false,
            FormatoColorLupa::Hex,
            vec![],
        );
        let z1 = Rect {
            x: 10,
            y: 10,
            ancho: 300,
            alto: 200,
        };
        s.tomar(Util::Rectangulo, z1, 100);
        assert_eq!(s.util, Some(Util::Rectangulo));
        let z2 = Rect { x: 50, ..z1 };
        assert_eq!(s.anotacion_en(z2, 100).origen().x, 50, "vacia: se muda");
        assert_eq!(s.anotacion.as_ref().unwrap().util, Some(Util::Rectangulo));
        let a = s.anotacion_en(z2, 100);
        a.pulsar(Punto { x: 60, y: 20 });
        a.mover(Punto { x: 160, y: 120 });
        a.soltar(Punto { x: 160, y: 120 });
        assert!(s.anotada());
        // Caso negativo: con algo dibujado no se muda.
        assert_eq!(s.anotacion_en(z1, 100).origen().x, 50);
    }

    #[test]
    fn mayus_alterna_hex_y_rgb() {
        let mut s = Sesion::nueva(
            Contexto::default(),
            ModoConfirmacion::ConBarra,
            false,
            FormatoColorLupa::Hex,
            vec![],
        );
        s.cambiar_formato();
        assert_eq!(s.formato, FormatoColorLupa::Rgb);
        s.cambiar_formato();
        assert_eq!(s.formato, FormatoColorLupa::Hex);
    }
}
