//! **Donde cae cada cosa del panel** y que hay bajo el raton. Puro: medidas
//! de la maqueta (`Bandeja2.dc.html`) por la escala del monitor, sin
//! ventana ni GPU, para que las pruebas lo comprueben.
//!
//! Filas de 44 y botones de 40 como poco (los principios del rediseno).

use pixpin_render::RectF;

/// Ancho del panel, en pixeles logicos (el de la maqueta).
pub const ANCHO: f32 = 440.0;
const MARGEN: f32 = 14.0;
const HUECO: f32 = 8.0;
pub const CABECERA: f32 = 40.0;
pub const FILA: f32 = 44.0;
const CAPTURAR: f32 = 54.0;
const MODO: f32 = 48.0;
const FAVORITO: f32 = 62.0;
const MINIATURA: f32 = 56.0;
/// Cuantos modos de captura hay en la fila.
pub const MODOS: usize = 6;
/// Cuantas capturas se ensenan.
pub const MINIATURAS: usize = 4;
/// Cuantas ventanas hay en la rejilla de abajo.
pub const VENTANAS: usize = 4;

/// Lo que se puede pulsar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zona {
    Sincronizar,
    Ajustes,
    Buscador,
    Capturar,
    Modo(usize),
    EditarFavoritos,
    Favorito(usize),
    Anadir,
    /// 0 ocultar los pines, 1 dejar pasar el clic, 2 silenciar los atajos.
    Interruptor(usize),
    VerGaleria,
    Miniatura(usize),
    AccionMiniatura(usize, AccionMiniatura),
    Ventana(usize),
    Salir,
    /// Una fila de la lista: un resultado del buscador, o una candidata al
    /// elegir favoritos.
    Fila(usize),
    /// Terminar de elegir favoritos.
    Hecho,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccionMiniatura {
    Pinear,
    Copiar,
    Borrar,
}

pub const ACCIONES_MINIATURA: [AccionMiniatura; 3] = [
    AccionMiniatura::Pinear,
    AccionMiniatura::Copiar,
    AccionMiniatura::Borrar,
];

/// Que se ensena debajo del buscador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// El panel entero.
    Normal,
    /// Se esta escribiendo: la lista de lo que se encontro.
    Busqueda { resultados: usize },
    /// Eligiendo favoritos: todas las acciones (o las que se buscan), con
    /// su marca.
    Editar { candidatas: usize },
}

/// Lo que la disposicion necesita saber de lo que hay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vista {
    /// Pixeles fisicos por logico.
    pub escala: f32,
    /// Lo desplazado hacia abajo, en fisicos.
    pub scroll: f32,
    pub modo: Modo,
    pub favoritos: usize,
    /// Si sale la ficha «Anadir» (no se ha llegado al tope).
    pub con_anadir: bool,
    /// El ancho del boton «Sincronizar ahora», ya medido, en logicos.
    pub ancho_sincronizar: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Disposicion {
    pub escala: f32,
    pub ancho: f32,
    /// Lo que mide todo lo de dentro, sin desplazar (fisicos).
    pub alto_total: f32,
    pub logo: RectF,
    pub sincronizar: RectF,
    pub ajustes: RectF,
    pub buscador: RectF,
    /// Normal: la tarjeta de capturar y lo que lleva.
    pub tarjeta_captura: RectF,
    pub capturar: RectF,
    pub modos: Vec<RectF>,
    pub etiqueta_favoritos: RectF,
    pub editar: RectF,
    pub favoritos: Vec<RectF>,
    pub anadir: Option<RectF>,
    pub tarjeta_interruptores: RectF,
    pub etiqueta_pines: RectF,
    pub interruptores: Vec<RectF>,
    pub raya: RectF,
    pub etiqueta_atajos: RectF,
    pub etiqueta_ultimas: RectF,
    pub ver_galeria: RectF,
    pub miniaturas: Vec<RectF>,
    pub ventanas: Vec<RectF>,
    pub raya_salir: RectF,
    pub salir: RectF,
    /// Busqueda y editar: la cabecera (solo al editar) y las filas.
    pub cabecera_lista: Option<RectF>,
    pub hecho: Option<RectF>,
    pub filas: Vec<RectF>,
    zonas: Vec<(RectF, Zona)>,
}

fn r(x: f32, y: f32, ancho: f32, alto: f32) -> RectF {
    RectF { x, y, ancho, alto }
}

const NADA: RectF = RectF {
    x: 0.0,
    y: 0.0,
    ancho: 0.0,
    alto: 0.0,
};

pub fn dentro(c: RectF, p: (f32, f32)) -> bool {
    p.0 >= c.x && p.0 < c.x + c.ancho && p.1 >= c.y && p.1 < c.y + c.alto
}

/// Reparte `n` columnas iguales en `ancho` con `hueco` entre ellas.
fn columnas(x: f32, y: f32, ancho: f32, alto: f32, n: usize, hueco: f32) -> Vec<RectF> {
    let w = (ancho - hueco * (n as f32 - 1.0)) / n as f32;
    (0..n)
        .map(|i| r(x + i as f32 * (w + hueco), y, w, alto))
        .collect()
}

pub fn disponer(v: &Vista) -> Disposicion {
    let e = v.escala;
    let ancho = ANCHO * e;
    let m = MARGEN * e;
    let h = HUECO * e;
    let util = ancho - 2.0 * m;
    let mut zonas: Vec<(RectF, Zona)> = Vec::new();
    let mut y = m - v.scroll;

    // Cabecera: logo, nombre y estado, «Sincronizar ahora» y Ajustes.
    let logo = r(m, y + 5.0 * e, 30.0 * e, 30.0 * e);
    let ajustes = r(m + util - CABECERA * e, y, CABECERA * e, CABECERA * e);
    let ancho_s = v.ancho_sincronizar.max(40.0) * e;
    let sincronizar = r(ajustes.x - 6.0 * e - ancho_s, y, ancho_s, CABECERA * e);
    zonas.push((sincronizar, Zona::Sincronizar));
    zonas.push((ajustes, Zona::Ajustes));
    y += CABECERA * e + h;

    let buscador = r(m, y, util, FILA * e);
    zonas.push((buscador, Zona::Buscador));
    y += FILA * e + h;

    let mut d = Disposicion {
        escala: e,
        ancho,
        alto_total: 0.0,
        logo,
        sincronizar,
        ajustes,
        buscador,
        tarjeta_captura: NADA,
        capturar: NADA,
        modos: Vec::new(),
        etiqueta_favoritos: NADA,
        editar: NADA,
        favoritos: Vec::new(),
        anadir: None,
        tarjeta_interruptores: NADA,
        etiqueta_pines: NADA,
        interruptores: Vec::new(),
        raya: NADA,
        etiqueta_atajos: NADA,
        etiqueta_ultimas: NADA,
        ver_galeria: NADA,
        miniaturas: Vec::new(),
        ventanas: Vec::new(),
        raya_salir: NADA,
        salir: NADA,
        cabecera_lista: None,
        hecho: None,
        filas: Vec::new(),
        zonas: Vec::new(),
    };

    match v.modo {
        Modo::Busqueda { resultados } => {
            for i in 0..resultados {
                let f = r(m, y, util, FILA * e);
                zonas.push((f, Zona::Fila(i)));
                d.filas.push(f);
                y += FILA * e;
            }
            if resultados == 0 {
                // El sitio del «nada se llama asi».
                y += FILA * e;
            }
        }
        Modo::Editar { candidatas } => {
            let cab = r(m, y, util, FILA * e);
            let hecho = r(m + util - 96.0 * e, y + 2.0 * e, 96.0 * e, 40.0 * e);
            zonas.push((hecho, Zona::Hecho));
            d.cabecera_lista = Some(cab);
            d.hecho = Some(hecho);
            y += FILA * e;
            for i in 0..candidatas {
                let f = r(m, y, util, FILA * e);
                zonas.push((f, Zona::Fila(i)));
                d.filas.push(f);
                y += FILA * e;
            }
        }
        Modo::Normal => {
            // Capturar, con sus modos debajo.
            let pad = 8.0 * e;
            let alto_t = pad + CAPTURAR * e + pad + MODO * e + pad;
            d.tarjeta_captura = r(m, y, util, alto_t);
            d.capturar = r(m + pad, y + pad, util - 2.0 * pad, CAPTURAR * e);
            zonas.push((d.capturar, Zona::Capturar));
            d.modos = columnas(
                m + pad,
                y + pad + CAPTURAR * e + pad,
                util - 2.0 * pad,
                MODO * e,
                MODOS,
                6.0 * e,
            );
            for (i, c) in d.modos.iter().enumerate() {
                zonas.push((*c, Zona::Modo(i)));
            }
            y += alto_t + h;

            // Favoritos.
            d.etiqueta_favoritos = r(m, y, util, 28.0 * e);
            d.editar = r(m + util - 84.0 * e, y - 6.0 * e, 84.0 * e, 40.0 * e);
            zonas.push((d.editar, Zona::EditarFavoritos));
            y += 28.0 * e + h;
            let celdas = v.favoritos + usize::from(v.con_anadir);
            let filas = celdas.div_ceil(4).max(1);
            for fila in 0..filas {
                let fila_y = y + fila as f32 * (FAVORITO * e + h);
                for (col, c) in columnas(m, fila_y, util, FAVORITO * e, 4, h)
                    .into_iter()
                    .enumerate()
                {
                    let i = fila * 4 + col;
                    if i < v.favoritos {
                        zonas.push((c, Zona::Favorito(i)));
                        d.favoritos.push(c);
                    } else if i == v.favoritos && v.con_anadir {
                        zonas.push((c, Zona::Anadir));
                        d.anadir = Some(c);
                    }
                }
            }
            y += filas as f32 * (FAVORITO * e + h);

            // Interruptores: «Pines» con dos y «Atajos» con uno.
            let pad = 4.0 * e;
            let etiqueta = 22.0 * e;
            let raya = 9.0 * e;
            let alto_t = pad + etiqueta + 2.0 * FILA * e + raya + etiqueta + FILA * e + pad;
            d.tarjeta_interruptores = r(m, y, util, alto_t);
            let xi = m + pad;
            let wi = util - 2.0 * pad;
            let mut yi = y + pad;
            d.etiqueta_pines = r(xi + 12.0 * e, yi, wi, etiqueta);
            yi += etiqueta;
            for i in 0..2 {
                let f = r(xi, yi, wi, FILA * e);
                zonas.push((f, Zona::Interruptor(i)));
                d.interruptores.push(f);
                yi += FILA * e;
            }
            d.raya = r(xi + 12.0 * e, yi + raya / 2.0, wi - 24.0 * e, e.max(1.0));
            yi += raya;
            d.etiqueta_atajos = r(xi + 12.0 * e, yi, wi, etiqueta);
            yi += etiqueta;
            let f = r(xi, yi, wi, FILA * e);
            zonas.push((f, Zona::Interruptor(2)));
            d.interruptores.push(f);
            y += alto_t + h;

            // Ultimas capturas.
            d.etiqueta_ultimas = r(m, y, util, 24.0 * e);
            d.ver_galeria = r(m + util - 130.0 * e, y - 8.0 * e, 130.0 * e, 40.0 * e);
            zonas.push((d.ver_galeria, Zona::VerGaleria));
            y += 24.0 * e + h;
            d.miniaturas = columnas(m, y, util, MINIATURA * e, MINIATURAS, h);
            for (i, c) in d.miniaturas.iter().enumerate() {
                zonas.push((*c, Zona::Miniatura(i)));
            }
            y += MINIATURA * e + h;

            // Las ventanas, en dos columnas.
            for _ in 0..VENTANAS.div_ceil(2) {
                for c in columnas(m, y, util, FILA * e, 2, h) {
                    let i = d.ventanas.len();
                    if i < VENTANAS {
                        zonas.push((c, Zona::Ventana(i)));
                        d.ventanas.push(c);
                    }
                }
                y += FILA * e + h;
            }

            // Salir, apartado tras una raya.
            d.raya_salir = r(m, y, util, e.max(1.0));
            y += e.max(1.0);
            d.salir = r(m, y, util, FILA * e);
            zonas.push((d.salir, Zona::Salir));
            y += FILA * e;
        }
    }
    d.alto_total = y + v.scroll + m;
    d.zonas = zonas;
    d
}

/// Los tres botones que salen encima de una miniatura (pinear, copiar,
/// borrar), centrados. 28 en la maqueta; aqui 32 para que se acierten.
pub fn acciones_de_miniatura(c: RectF, escala: f32) -> [RectF; 3] {
    let lado = (32.0 * escala)
        .min(c.alto - 4.0 * escala)
        .min((c.ancho - 8.0 * escala) / 3.0);
    let hueco = 4.0 * escala;
    let total = 3.0 * lado + 2.0 * hueco;
    let x0 = c.x + (c.ancho - total) / 2.0;
    let y = c.y + (c.alto - lado) / 2.0;
    [0, 1, 2].map(|i| r(x0 + i as f32 * (lado + hueco), y, lado, lado))
}

impl Disposicion {
    /// Que hay bajo `p`. Con el raton sobre una miniatura con captura
    /// (`miniatura_viva`), sus botones le ganan a ella.
    pub fn zona_en(&self, p: (f32, f32), miniatura_viva: Option<usize>) -> Option<Zona> {
        if let Some(i) = miniatura_viva
            && let Some(c) = self.miniaturas.get(i)
            && dentro(*c, p)
        {
            for (b, a) in acciones_de_miniatura(*c, self.escala)
                .iter()
                .zip(ACCIONES_MINIATURA)
            {
                if dentro(*b, p) {
                    return Some(Zona::AccionMiniatura(i, a));
                }
            }
        }
        // La ultima apuntada gana: «Editar» y «Ver la galeria» se apuntan
        // despues de lo que pisan.
        self.zonas
            .iter()
            .rev()
            .find(|(c, _)| dentro(*c, p))
            .map(|(_, z)| *z)
    }
}

/// **Donde se pone el panel**: junto al icono, pegado al borde del area de
/// trabajo que toca la barra de tareas. `trabajo` y `monitor` en fisicos;
/// `raton` es donde se hizo clic. Si no cabe de alto, se queda con el alto
/// del area (y por dentro se desplaza).
pub fn colocar(
    monitor: pixpin_geom::Rect,
    trabajo: pixpin_geom::Rect,
    raton: pixpin_geom::Punto,
    ancho: u32,
    alto: u32,
    margen: i32,
) -> pixpin_geom::Rect {
    let ancho = ancho.min(trabajo.ancho.saturating_sub(2 * margen as u32).max(1));
    let alto = alto.min(trabajo.alto.saturating_sub(2 * margen as u32).max(1));
    let (tx0, ty0) = (trabajo.x, trabajo.y);
    let (tx1, ty1) = (
        trabajo.x + trabajo.ancho as i32,
        trabajo.y + trabajo.alto as i32,
    );
    // La barra esta donde el area de trabajo se aparta del monitor.
    let arriba = trabajo.y > monitor.y;
    let izquierda = trabajo.x > monitor.x && trabajo.alto >= monitor.alto;
    let derecha = tx1 < monitor.x + monitor.ancho as i32 && trabajo.alto >= monitor.alto;
    let x = if izquierda {
        tx0 + margen
    } else if derecha {
        tx1 - margen - ancho as i32
    } else {
        // Abajo o arriba: centrado bajo el raton, sin salirse.
        (raton.x - ancho as i32 / 2).clamp(
            tx0 + margen,
            (tx1 - margen - ancho as i32).max(tx0 + margen),
        )
    };
    let y = if arriba {
        ty0 + margen
    } else if izquierda || derecha {
        (raton.y - alto as i32 / 2)
            .clamp(ty0 + margen, (ty1 - margen - alto as i32).max(ty0 + margen))
    } else {
        ty1 - margen - alto as i32
    };
    pixpin_geom::Rect { x, y, ancho, alto }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn vista(modo: Modo) -> Vista {
        Vista {
            escala: 1.0,
            scroll: 0.0,
            modo,
            favoritos: 3,
            con_anadir: true,
            ancho_sincronizar: 140.0,
        }
    }

    fn centro(c: RectF) -> (f32, f32) {
        (c.x + c.ancho / 2.0, c.y + c.alto / 2.0)
    }

    #[test]
    fn cada_cosa_esta_donde_se_pinta() {
        let d = disponer(&vista(Modo::Normal));
        assert_eq!(d.zona_en(centro(d.capturar), None), Some(Zona::Capturar));
        assert_eq!(d.zona_en(centro(d.modos[5]), None), Some(Zona::Modo(5)));
        assert_eq!(
            d.zona_en(centro(d.favoritos[2]), None),
            Some(Zona::Favorito(2))
        );
        assert_eq!(
            d.zona_en(centro(d.anadir.unwrap()), None),
            Some(Zona::Anadir)
        );
        assert_eq!(
            d.zona_en(centro(d.interruptores[2]), None),
            Some(Zona::Interruptor(2))
        );
        assert_eq!(
            d.zona_en(centro(d.ventanas[3]), None),
            Some(Zona::Ventana(3))
        );
        assert_eq!(d.zona_en(centro(d.salir), None), Some(Zona::Salir));
        assert_eq!(
            d.zona_en(centro(d.sincronizar), None),
            Some(Zona::Sincronizar)
        );
        assert_eq!(
            d.zona_en(centro(d.ver_galeria), None),
            Some(Zona::VerGaleria)
        );
        // Cabe en la maqueta (820 de alto) con una fila de favoritos.
        assert!(d.alto_total <= 820.0, "{}", d.alto_total);
    }

    #[test]
    fn caso_negativo_entre_botones_y_fuera_no_hay_nada() {
        let d = disponer(&vista(Modo::Normal));
        // El hueco entre el primer y el segundo modo.
        let a = d.modos[0];
        let entre = (a.x + a.ancho + 3.0, a.y + a.alto / 2.0);
        assert_eq!(d.zona_en(entre, None), None);
        assert_eq!(d.zona_en((-5.0, 100.0), None), None);
        assert_eq!(
            d.zona_en((5.0, 5.0), None),
            None,
            "el margen no es de nadie"
        );
    }

    #[test]
    fn los_objetivos_miden_al_menos_40_y_las_filas_44() {
        let d = disponer(&vista(Modo::Normal));
        let mut todos = vec![
            d.capturar,
            d.sincronizar,
            d.ajustes,
            d.editar,
            d.ver_galeria,
            d.salir,
        ];
        todos.extend(d.modos.iter().copied());
        todos.extend(d.favoritos.iter().copied());
        todos.extend(d.ventanas.iter().copied());
        for c in todos {
            assert!(c.alto >= 40.0 && c.ancho >= 40.0, "{c:?}");
        }
        for f in &d.interruptores {
            assert!(f.alto >= 44.0);
        }
    }

    #[test]
    fn los_botones_de_la_miniatura_solo_con_captura_debajo() {
        let d = disponer(&vista(Modo::Normal));
        let m = d.miniaturas[0];
        let [_, copiar, _] = acciones_de_miniatura(m, 1.0);
        assert_eq!(
            d.zona_en(centro(copiar), Some(0)),
            Some(Zona::AccionMiniatura(0, AccionMiniatura::Copiar))
        );
        assert_eq!(
            d.zona_en(centro(copiar), None),
            Some(Zona::Miniatura(0)),
            "caso negativo: sin captura no hay botones"
        );
    }

    #[test]
    fn al_buscar_solo_hay_filas_y_al_desplazar_se_mueven() {
        let d = disponer(&vista(Modo::Busqueda { resultados: 3 }));
        assert_eq!(d.filas.len(), 3);
        assert!(d.capturar.ancho == 0.0, "lo normal no se pinta al buscar");
        assert_eq!(d.zona_en(centro(d.filas[1]), None), Some(Zona::Fila(1)));
        let mut v = vista(Modo::Busqueda { resultados: 3 });
        v.scroll = 44.0;
        let d2 = disponer(&v);
        assert_eq!(d2.filas[1].y, d.filas[1].y - 44.0);
        assert_eq!(
            d2.alto_total, d.alto_total,
            "el alto no cambia al desplazar"
        );
    }

    #[test]
    fn al_editar_esta_hecho_y_las_candidatas() {
        let d = disponer(&vista(Modo::Editar { candidatas: 5 }));
        assert_eq!(d.zona_en(centro(d.hecho.unwrap()), None), Some(Zona::Hecho));
        assert_eq!(d.zona_en(centro(d.filas[4]), None), Some(Zona::Fila(4)));
    }

    #[test]
    fn a_doble_escala_todo_mide_el_doble() {
        let d1 = disponer(&vista(Modo::Normal));
        let mut v = vista(Modo::Normal);
        v.escala = 2.0;
        let d2 = disponer(&v);
        assert!((d2.alto_total - 2.0 * d1.alto_total).abs() < 0.01);
        assert!((d2.capturar.ancho - 2.0 * d1.capturar.ancho).abs() < 0.01);
    }

    #[test]
    fn el_panel_sale_junto_a_la_barra_de_tareas() {
        use pixpin_geom::{Punto, Rect};
        let monitor = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        };
        // Barra abajo: pegado abajo, bajo el raton y sin salirse a la derecha.
        let abajo = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1032,
        };
        let c = colocar(monitor, abajo, Punto { x: 1850, y: 1050 }, 440, 800, 12);
        assert_eq!((c.x, c.y), (1920 - 12 - 440, 1032 - 12 - 800));
        // Barra arriba: pegado arriba.
        let arriba = Rect {
            x: 0,
            y: 48,
            ancho: 1920,
            alto: 1032,
        };
        let c = colocar(monitor, arriba, Punto { x: 900, y: 20 }, 440, 800, 12);
        assert_eq!((c.x, c.y), (900 - 220, 60));
        // Caso negativo: un area mas baja que el panel no lo deja salirse;
        // se queda con lo que hay.
        let bajita = Rect {
            x: 0,
            y: 0,
            ancho: 1366,
            alto: 600,
        };
        let c = colocar(
            Rect {
                x: 0,
                y: 0,
                ancho: 1366,
                alto: 648,
            },
            bajita,
            Punto { x: 1300, y: 630 },
            440,
            800,
            12,
        );
        assert_eq!(c.alto, 600 - 24);
        assert!(c.y >= 12);
    }
}
