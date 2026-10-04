//! La barra del pin (rediseno v2), sin nada de pantalla: que botones lleva
//! cada tipo de pin, donde cae cada uno y donde se pone la barra.
//!
//! **La misma barra en todos los pines.** Primero el control propio del
//! tipo (el zoom de una foto, las paginas de un PDF, el reproductor de un
//! video, «EN VIVO» y congelar en un pin en vivo, abrir en una ficha), luego
//! lo comun —Copiar, Anotar, Mas— y Cerrar aparte, siempre a la derecha. El
//! menu del clic derecho sigue el mismo orden (`menu.rs`).
//!
//! La barra va FUERA del pin, encima (o debajo si arriba no cabe): asi no
//! tapa lo que se esta mirando, y su ancho no depende del del pin. Por eso
//! el video ya no lleva su franja de mandos dentro de la imagen: sus mandos
//! (`mandos_video`) son el control propio de esta barra.

use pixpin_geom::Rect;
use pixpin_render::RectF;

/// Alto de la barra y lado de sus botones, en pixeles logicos (objetivos de
/// 40 px, como pide el rediseno).
pub const ALTO_LOGICO: f32 = 48.0;
pub const BOTON_LOGICO: f32 = 40.0;
/// Lo que queda entre la barra y el pin.
pub const SEPARACION_LOGICA: f32 = 8.0;
/// Lo que mide el aviso de un boton («Copiar  Ctrl C») debajo de la barra.
pub const AVISO_LOGICO: f32 = 34.0;
const RELLENO: f32 = 4.0;
const HUECO: f32 = 2.0;
const SEPARADOR: f32 = 9.0;
const ROTULO_ZOOM: f32 = 56.0;
const ROTULO_PAGINA: f32 = 64.0;
const ROTULO_TIEMPO: f32 = 48.0;
const ROTULO_EN_VIVO: f32 = 84.0;
const RECORRIDO: f32 = 132.0;

/// Lo que hace un boton de la barra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccionBarra {
    Alejar,
    Acercar,
    PaginaAnterior,
    PaginaSiguiente,
    /// Pinear la pagina que se ve.
    PinearPagina,
    /// Reproducir o pausar (video).
    Reproducir,
    /// Saltar en el video: la linea de tiempo.
    Saltar,
    /// Silenciar o dar sonido (video). La rueda encima cambia el volumen.
    Sonido,
    Congelar,
    /// Abrir el fichero de una ficha con su aplicacion.
    Abrir,
    Copiar,
    Anotar,
    Mas,
    Cerrar,
}

/// Un texto fijo de la barra: no se pulsa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotulo {
    /// «100 %».
    Zoom,
    /// «3 / 48».
    Pagina,
    /// El tiempo visto y el total de un video.
    Tiempo,
    Duracion,
    /// La chapa roja de un pin en vivo.
    EnVivo,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pieza {
    Boton(AccionBarra),
    Rotulo(Rotulo),
    Separador,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Elemento {
    pub pieza: Pieza,
    /// En coordenadas de la barra: (0, 0) es su esquina.
    pub rect: RectF,
}

/// El control propio de cada tipo de pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Propio {
    /// Una herramienta: lo suyo ya esta dentro.
    Ninguno,
    /// Foto o nota: alejar, el zoom y acercar.
    Zoom,
    /// Un PDF: pasar de pagina (si hay mas de una) y pinear la que se ve.
    Paginas {
        con_paso: bool,
    },
    Video,
    Vivo,
    /// Una ficha o un documento que no es PDF.
    Abrir,
}

/// Que barra lleva un pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TipoBarra {
    pub propio: Propio,
    /// Anotar encima: fotos y notas (es lo que ya hace el doble clic).
    pub anotable: bool,
    /// Copiar: lo que se ve (en un pin en vivo, el fotograma de ahora).
    pub copiable: bool,
}

/// La barra ya dispuesta.
#[derive(Debug, Clone, PartialEq)]
pub struct Barra {
    pub ancho: f32,
    pub alto: f32,
    pub elementos: Vec<Elemento>,
}

fn dentro(r: RectF, x: f32, y: f32) -> bool {
    x >= r.x && x < r.x + r.ancho && y >= r.y && y < r.y + r.alto
}

/// Dispone la barra de un tipo de pin a una escala. Siempre cabe entera: va
/// fuera del pin, y su ancho no depende del suyo.
pub fn disponer(tipo: TipoBarra, escala: f32) -> Barra {
    let e = escala.max(0.5);
    let alto = ALTO_LOGICO * e;
    let lado = BOTON_LOGICO * e;
    let y = (alto - lado) / 2.0;
    let mut x = RELLENO * e;
    let mut v: Vec<Elemento> = Vec::new();
    let mut poner = |v: &mut Vec<Elemento>, pieza: Pieza, ancho: f32| {
        let (ry, ralto) = match pieza {
            Pieza::Separador => (alto * 0.25, alto * 0.5),
            _ => (y, lado),
        };
        v.push(Elemento {
            pieza,
            rect: RectF {
                x,
                y: ry,
                ancho,
                alto: ralto,
            },
        });
        x += ancho + HUECO * e;
    };
    let boton = |a| Pieza::Boton(a);
    match tipo.propio {
        Propio::Ninguno => {}
        Propio::Zoom => {
            poner(&mut v, boton(AccionBarra::Alejar), lado);
            poner(&mut v, Pieza::Rotulo(Rotulo::Zoom), ROTULO_ZOOM * e);
            poner(&mut v, boton(AccionBarra::Acercar), lado);
        }
        Propio::Paginas { con_paso } => {
            if con_paso {
                poner(&mut v, boton(AccionBarra::PaginaAnterior), lado);
                poner(&mut v, Pieza::Rotulo(Rotulo::Pagina), ROTULO_PAGINA * e);
                poner(&mut v, boton(AccionBarra::PaginaSiguiente), lado);
            }
            poner(&mut v, boton(AccionBarra::PinearPagina), lado);
        }
        Propio::Video => {
            poner(&mut v, boton(AccionBarra::Reproducir), lado);
            poner(&mut v, Pieza::Rotulo(Rotulo::Tiempo), ROTULO_TIEMPO * e);
            poner(&mut v, boton(AccionBarra::Saltar), RECORRIDO * e);
            poner(&mut v, Pieza::Rotulo(Rotulo::Duracion), ROTULO_TIEMPO * e);
            poner(&mut v, boton(AccionBarra::Sonido), lado);
        }
        Propio::Vivo => {
            poner(&mut v, Pieza::Rotulo(Rotulo::EnVivo), ROTULO_EN_VIVO * e);
            poner(&mut v, boton(AccionBarra::Congelar), lado);
        }
        Propio::Abrir => {
            poner(&mut v, boton(AccionBarra::Abrir), lado);
        }
    }
    if !v.is_empty() {
        poner(&mut v, Pieza::Separador, SEPARADOR * e);
    }
    if tipo.copiable {
        poner(&mut v, boton(AccionBarra::Copiar), lado);
    }
    if tipo.anotable {
        poner(&mut v, boton(AccionBarra::Anotar), lado);
    }
    poner(&mut v, boton(AccionBarra::Mas), lado);
    poner(&mut v, Pieza::Separador, SEPARADOR * e);
    poner(&mut v, boton(AccionBarra::Cerrar), lado);
    // El ultimo `poner` dejo un hueco de mas detras.
    let ancho = x - HUECO * e + RELLENO * e;
    Barra {
        ancho,
        alto,
        elementos: v,
    }
}

impl Barra {
    /// El boton bajo un punto de la barra. Un rotulo, un separador o el
    /// relleno no son de nadie.
    pub fn accion_en(&self, x: f32, y: f32) -> Option<AccionBarra> {
        self.elementos.iter().find_map(|e| match e.pieza {
            Pieza::Boton(a) if dentro(e.rect, x, y) => Some(a),
            _ => None,
        })
    }

    /// Donde esta un boton, si esta barra lo lleva.
    pub fn rect_de(&self, accion: AccionBarra) -> Option<RectF> {
        self.elementos.iter().find_map(|e| match e.pieza {
            Pieza::Boton(a) if a == accion => Some(e.rect),
            _ => None,
        })
    }

    pub fn rect_de_rotulo(&self, rotulo: Rotulo) -> Option<RectF> {
        self.elementos.iter().find_map(|e| match e.pieza {
            Pieza::Rotulo(r) if r == rotulo => Some(e.rect),
            _ => None,
        })
    }

    /// Que fraccion del video es una `x` sobre la linea de tiempo, de 0 a 1
    /// (fuera de ella se pega al extremo: arrastrando se puede salir).
    pub fn fraccion(&self, x: f32) -> f64 {
        let Some(r) = self.rect_de(AccionBarra::Saltar) else {
            return 0.0;
        };
        let margen = r.ancho.min(16.0) / 2.0;
        (((x - r.x - margen) / (r.ancho - 2.0 * margen).max(1.0)) as f64).clamp(0.0, 1.0)
    }
}

/// De que lado del pin quedo la barra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lado {
    Encima,
    Debajo,
    /// Ni arriba ni abajo hay sitio (un pin que llena la pantalla): dentro,
    /// arriba, tapando un poco.
    Dentro,
}

/// Donde va la barra (su esquina, en pixeles fisicos) para un pin cuyo
/// contenido ocupa `pin` en un monitor cuya area de trabajo es `trabajo`.
/// Alineada con el borde izquierdo del pin y sin salirse del monitor.
pub fn colocar(
    pin: Rect,
    trabajo: Rect,
    ancho: u32,
    alto: u32,
    separacion: i32,
) -> (i32, i32, Lado) {
    let max_x = (trabajo.derecha() - ancho as i32).max(trabajo.x);
    let x = pin.x.clamp(trabajo.x, max_x);
    let arriba = pin.y - separacion - alto as i32;
    if arriba >= trabajo.y {
        return (x, arriba, Lado::Encima);
    }
    let abajo = pin.abajo() + separacion;
    if abajo + alto as i32 <= trabajo.abajo() {
        return (x, abajo, Lado::Debajo);
    }
    (x, pin.y.max(trabajo.y) + separacion, Lado::Dentro)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn tipo(propio: Propio) -> TipoBarra {
        TipoBarra {
            propio,
            anotable: true,
            copiable: true,
        }
    }

    fn acciones(b: &Barra) -> Vec<AccionBarra> {
        b.elementos
            .iter()
            .filter_map(|e| match e.pieza {
                Pieza::Boton(a) => Some(a),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn todas_las_barras_acaban_igual_copiar_anotar_mas_y_cerrar_aparte() {
        for propio in [
            Propio::Zoom,
            Propio::Paginas { con_paso: true },
            Propio::Video,
            Propio::Vivo,
            Propio::Abrir,
            Propio::Ninguno,
        ] {
            let b = disponer(tipo(propio), 1.0);
            let a = acciones(&b);
            let n = a.len();
            assert_eq!(
                &a[n - 4..],
                &[
                    AccionBarra::Copiar,
                    AccionBarra::Anotar,
                    AccionBarra::Mas,
                    AccionBarra::Cerrar
                ],
                "{propio:?}"
            );
            // Cerrar va aparte: un separador justo delante.
            let i = b
                .elementos
                .iter()
                .position(|e| e.pieza == Pieza::Boton(AccionBarra::Cerrar))
                .unwrap();
            assert_eq!(b.elementos[i - 1].pieza, Pieza::Separador);
        }
    }

    #[test]
    fn el_control_propio_va_primero() {
        let foto = acciones(&disponer(tipo(Propio::Zoom), 1.0));
        assert_eq!(&foto[..2], &[AccionBarra::Alejar, AccionBarra::Acercar]);
        let video = acciones(&disponer(tipo(Propio::Video), 1.0));
        assert_eq!(video[0], AccionBarra::Reproducir);
        let pdf = acciones(&disponer(tipo(Propio::Paginas { con_paso: true }), 1.0));
        assert_eq!(pdf[0], AccionBarra::PaginaAnterior);
    }

    #[test]
    fn caso_negativo_un_pdf_de_una_pagina_no_ofrece_pasarla() {
        let a = acciones(&disponer(tipo(Propio::Paginas { con_paso: false }), 1.0));
        assert!(!a.contains(&AccionBarra::PaginaSiguiente));
        assert!(!a.contains(&AccionBarra::PaginaAnterior));
        assert_eq!(a[0], AccionBarra::PinearPagina, "pinearla si se puede");
    }

    #[test]
    fn caso_negativo_lo_que_no_se_anota_ni_copia_no_lo_ofrece() {
        let b = disponer(
            TipoBarra {
                propio: Propio::Vivo,
                anotable: false,
                copiable: false,
            },
            1.0,
        );
        let a = acciones(&b);
        assert!(!a.contains(&AccionBarra::Anotar));
        assert!(!a.contains(&AccionBarra::Copiar));
        assert_eq!(
            a,
            vec![AccionBarra::Congelar, AccionBarra::Mas, AccionBarra::Cerrar]
        );
    }

    #[test]
    fn los_botones_miden_cuarenta_y_no_se_pisan() {
        let b = disponer(tipo(Propio::Video), 1.0);
        for e in &b.elementos {
            if let Pieza::Boton(a) = e.pieza {
                assert!(e.rect.alto >= 40.0, "{a:?}");
                assert!(e.rect.ancho >= 40.0, "{a:?}");
            }
        }
        for par in b.elementos.windows(2) {
            assert!(par[0].rect.x + par[0].rect.ancho <= par[1].rect.x);
        }
        let ultimo = b.elementos.last().unwrap().rect;
        assert!(ultimo.x + ultimo.ancho <= b.ancho, "todo dentro");
        // A doble escala, el doble.
        let b2 = disponer(tipo(Propio::Video), 2.0);
        assert!((b2.ancho - 2.0 * b.ancho).abs() < 0.01);
    }

    #[test]
    fn el_clic_encuentra_su_boton_y_caso_negativo_el_rotulo_no_es_boton() {
        let b = disponer(tipo(Propio::Zoom), 1.0);
        let copiar = b.rect_de(AccionBarra::Copiar).unwrap();
        assert_eq!(
            b.accion_en(copiar.x + 5.0, copiar.y + 5.0),
            Some(AccionBarra::Copiar)
        );
        let zoom = b.rect_de_rotulo(Rotulo::Zoom).unwrap();
        assert_eq!(b.accion_en(zoom.x + 5.0, zoom.y + 5.0), None);
        assert_eq!(b.accion_en(1.0, 1.0), None, "el relleno no es de nadie");
        assert_eq!(b.accion_en(-3.0, 20.0), None);
    }

    #[test]
    fn la_linea_de_tiempo_va_de_cero_a_uno() {
        let b = disponer(tipo(Propio::Video), 1.0);
        let r = b.rect_de(AccionBarra::Saltar).unwrap();
        assert_eq!(b.fraccion(r.x - 50.0), 0.0, "caso negativo: antes");
        assert_eq!(b.fraccion(r.x + r.ancho + 50.0), 1.0);
        assert!((b.fraccion(r.x + r.ancho / 2.0) - 0.5).abs() < 1e-6);
        // Sin video no hay linea: cero, sin panico.
        assert_eq!(disponer(tipo(Propio::Zoom), 1.0).fraccion(10.0), 0.0);
    }

    fn rect(x: i32, y: i32, ancho: u32, alto: u32) -> Rect {
        Rect { x, y, ancho, alto }
    }

    #[test]
    fn la_barra_va_encima_del_pin_alineada_a_su_izquierda() {
        let trabajo = rect(0, 0, 1920, 1040);
        let (x, y, lado) = colocar(rect(300, 400, 500, 300), trabajo, 420, 48, 8);
        assert_eq!((x, y, lado), (300, 400 - 8 - 48, Lado::Encima));
    }

    #[test]
    fn sin_sitio_arriba_va_debajo_y_sin_ninguno_dentro() {
        let trabajo = rect(0, 0, 1920, 1040);
        let (_, y, lado) = colocar(rect(300, 20, 500, 300), trabajo, 420, 48, 8);
        assert_eq!((y, lado), (328, Lado::Debajo));
        // Caso negativo: un pin que llena la pantalla no la echa fuera.
        let (_, y, lado) = colocar(rect(0, 0, 1920, 1040), trabajo, 420, 48, 8);
        assert_eq!(lado, Lado::Dentro);
        assert!(y >= 0 && y + 48 <= 1040);
    }

    #[test]
    fn caso_negativo_la_barra_no_se_sale_por_la_derecha() {
        let trabajo = rect(0, 0, 1920, 1040);
        let (x, _, _) = colocar(rect(1800, 400, 100, 100), trabajo, 420, 48, 8);
        assert_eq!(x, 1920 - 420);
        let (x, _, _) = colocar(rect(-50, 400, 100, 100), trabajo, 420, 48, 8);
        assert_eq!(x, 0);
    }
}
