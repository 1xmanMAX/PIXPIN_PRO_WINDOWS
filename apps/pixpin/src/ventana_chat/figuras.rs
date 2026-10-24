//! **La galeria de las figuras** (24-oct-2026, el usuario: «ademas de la
//! visualizacion en proyectos a su lado tambien este la visualizacion de las
//! figuras y que sea como una galeria donde yo pueda ver las figuras asi
//! realizadas agrupadas, de forma que sea todo intuitivo y muy rapido de
//! ver»).
//!
//! La tercera parte del interruptor de la barra de arriba (Chat · Proyectos ·
//! Figuras). Ocupa todo lo de debajo de la barra: la biblioteca es una sola
//! para todos los proyectos (`biblioteca_figuras`), asi que no tiene sentido
//! dejar al lado la lista de proyectos.
//!
//! - **Arriba, fichas por grupo**: «Todas», «Mis figuras» y las categorias
//!   (la categoria va en el nombre, «Diario · Gratitud», ver
//!   `figuras_de_serie::categoria_de`). Un clic deja solo ese grupo.
//! - **Debajo, cada grupo con su titulo y su cuenta** y sus figuras en
//!   rejilla, cada una dibujada de verdad (las mismas ordenes que la vista
//!   previa de un lienzo en el chat) con su nombre.
//! - **Un clic en una figura la ensena en grande**; ahi las del usuario se
//!   pueden borrar (dos clics, como todo lo que no se deshace). Escape o un
//!   clic fuera la cierra.
//!
//! Se lee al entrar y no en cada fotograma: son unas decenas de figuras.

use std::cell::RefCell;

use pixpin_geom::{Punto, Rect};
use pixpin_motor2d::biblioteca::{self, FiguraGuardada};
use pixpin_motor2d::figuras_de_serie::{CATEGORIAS, categoria_de, es_de_serie};
use pixpin_render::{Color, Pintor, RectF};
use pixpin_store::Catalogo;

use super::{LienzoVisto, Tema, con_alfa, pintar_lienzo, rf};

/// Lo que mide una ficha de la rejilla (logicos): el dibujo y su nombre.
const CELDA_ANCHO: u32 = 168;
const CELDA_DIBUJO: u32 = 118;
const CELDA_NOMBRE: u32 = 34;
const HUECO: u32 = 12;
const MARGEN: u32 = 20;
const ALTO_FICHAS: u32 = 34;
const ALTO_TITULO: u32 = 34;
/// El grupo de lo que no lleva categoria: lo del usuario.
pub(super) const MIS_FIGURAS: &str = "";

/// Una figura, ya lista para pintar.
struct Vista {
    id: String,
    /// El nombre sin la categoria delante.
    nombre: String,
    grupo: String,
    de_serie: bool,
    dibujo: Option<LienzoVisto>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Zona {
    Grupo(Option<String>),
    Figura(usize),
    Borrar,
    Cerrar,
    /// Dentro de la grande: no hace nada (que no la cierre).
    Grande,
}

/// Lo que pide la galeria al chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pedido {
    Nada,
    Repintar,
    /// Escape sin nada abierto: de vuelta al chat.
    Salir,
}

pub(super) struct VistaFiguras {
    activa: bool,
    figuras: Vec<Vista>,
    /// El grupo elegido arriba; `None`, todas.
    filtro: Option<String>,
    scroll: i32,
    /// La que se ensena en grande.
    grande: Option<usize>,
    /// El primer clic de borrar.
    borrar_pedido: bool,
    raton: Punto,
    zonas: RefCell<Vec<(Rect, Zona)>>,
    /// Lo que mide todo, apuntado al pintar (para no pasarse con la rueda).
    alto_total: std::cell::Cell<i32>,
}

impl Default for VistaFiguras {
    fn default() -> Self {
        VistaFiguras {
            activa: false,
            figuras: Vec::new(),
            filtro: None,
            scroll: 0,
            grande: None,
            borrar_pedido: false,
            raton: Punto { x: -1, y: -1 },
            zonas: RefCell::new(Vec::new()),
            alto_total: std::cell::Cell::new(0),
        }
    }
}

impl VistaFiguras {
    pub(super) fn activa(&self) -> bool {
        self.activa
    }

    /// Entra (y lee la biblioteca) o sale.
    pub(super) fn poner(&mut self, si: bool) {
        self.activa = si;
        self.grande = None;
        self.borrar_pedido = false;
        if si {
            self.leer();
        } else {
            // Lo dibujado se suelta al salir: el chat no lo usa.
            self.figuras.clear();
        }
    }

    fn leer(&mut self) {
        let figuras = biblioteca::cargar(&crate::biblioteca_figuras::ruta());
        self.figuras = figuras.iter().map(vista_de).collect();
        // Las del usuario primero y despues las categorias en su orden.
        self.figuras.sort_by_key(|f| orden_del_grupo(&f.grupo));
        if let Some(g) = &self.filtro
            && !self.figuras.iter().any(|f| &f.grupo == g)
        {
            self.filtro = None;
        }
        self.scroll = 0;
    }

    /// Los grupos que hay, en orden, con cuantas figuras tiene cada uno.
    fn grupos(&self) -> Vec<(String, usize)> {
        let mut v: Vec<(String, usize)> = Vec::new();
        for f in &self.figuras {
            match v.iter_mut().find(|(g, _)| *g == f.grupo) {
                Some((_, n)) => *n += 1,
                None => v.push((f.grupo.clone(), 1)),
            }
        }
        v
    }

    pub(super) fn mover(&mut self, l: Punto) -> bool {
        let antes = self.zona_en(self.raton);
        self.raton = l;
        antes != self.zona_en(l)
    }

    fn zona_en(&self, l: Punto) -> Option<Zona> {
        self.zonas
            .borrow()
            .iter()
            .rev()
            .find(|(r, _)| r.contiene(l))
            .map(|(_, z)| z.clone())
    }

    pub(super) fn rueda(&mut self, delta: i32, escala: u32) -> bool {
        if self.grande.is_some() {
            return false;
        }
        let paso = (CELDA_DIBUJO * escala / 100) as i32;
        let nuevo = (self.scroll - delta.signum() * paso).clamp(0, self.alto_total.get().max(0));
        let cambio = nuevo != self.scroll;
        self.scroll = nuevo;
        cambio
    }

    pub(super) fn pulsar(&mut self, l: Punto) -> Pedido {
        let zona = self.zona_en(l);
        if self.grande.is_some() {
            match zona {
                Some(Zona::Borrar) => {
                    if self.borrar_pedido {
                        self.borrar();
                    } else {
                        self.borrar_pedido = true;
                    }
                }
                Some(Zona::Grande) => {}
                _ => {
                    self.grande = None;
                    self.borrar_pedido = false;
                }
            }
            return Pedido::Repintar;
        }
        match zona {
            Some(Zona::Grupo(g)) => {
                self.filtro = g;
                self.scroll = 0;
                Pedido::Repintar
            }
            Some(Zona::Figura(i)) => {
                self.grande = Some(i);
                Pedido::Repintar
            }
            _ => Pedido::Nada,
        }
    }

    pub(super) fn escape(&mut self) -> Pedido {
        if self.grande.take().is_some() {
            self.borrar_pedido = false;
            Pedido::Repintar
        } else {
            Pedido::Salir
        }
    }

    /// Quita de la biblioteca la que se ve en grande (solo las del usuario).
    fn borrar(&mut self) {
        let Some(i) = self.grande.take() else {
            return;
        };
        self.borrar_pedido = false;
        let Some(f) = self.figuras.get(i).filter(|f| !f.de_serie) else {
            return;
        };
        let ruta = crate::biblioteca_figuras::ruta();
        let mut todas = biblioteca::cargar(&ruta);
        let antes = todas.len();
        todas.retain(|t| t.id != f.id);
        if todas.len() != antes && crate::biblioteca_figuras::guardar(&ruta, &todas) {
            tracing::info!(nombre = %f.nombre, "figura borrada desde la galeria");
        }
        self.leer();
    }
}

/// El orden de los grupos: lo del usuario, y las categorias de serie en el
/// orden del menu del lienzo; una categoria inventada por el usuario, detras.
fn orden_del_grupo(g: &str) -> usize {
    if g == MIS_FIGURAS {
        0
    } else {
        CATEGORIAS
            .iter()
            .position(|c| *c == g)
            .map_or(CATEGORIAS.len() + 1, |i| i + 1)
    }
}

fn vista_de(f: &FiguraGuardada) -> Vista {
    let (grupo, nombre) = match categoria_de(&f.nombre) {
        Some((c, n)) => (c.to_string(), n.to_string()),
        None => (MIS_FIGURAS.to_string(), f.nombre.clone()),
    };
    Vista {
        id: f.id.clone(),
        nombre,
        grupo,
        de_serie: es_de_serie(f),
        dibujo: dibujo_de(f),
    }
}

/// La figura como la vista previa de un lienzo: sus ordenes y su caja.
fn dibujo_de(f: &FiguraGuardada) -> Option<LienzoVisto> {
    let mut escena = pixpin_motor2d::escena::Escena::nueva();
    for e in &f.elementos {
        escena.anadir(e.clone());
    }
    let caja = escena.caja()?;
    let lado = (caja.2 - caja.0).max(caja.3 - caja.1).max(1.0);
    let (ordenes, grafitos) = pixpin_motor2d::tinta::grafito::ordenes_con_grafito(
        &escena,
        (512.0 / lado).min(1.0),
        1 << 21,
    );
    let ordenes = crate::dibujo::tema::ordenes_como_en_el_lienzo(ordenes, escena.fondo);
    Some(LienzoVisto {
        ordenes,
        grafitos,
        caja,
        fondo: None,
        giro_del_fondo: 0.0,
        papel: None,
    })
}

/// El nombre de un grupo para ensenarlo.
fn rotulo_del_grupo(g: &str, textos: &Catalogo) -> String {
    if g == MIS_FIGURAS {
        textos.t("figuras-mias")
    } else {
        g.to_string()
    }
}

/// Pinta la galeria en `area` (todo lo de debajo de la barra).
pub(super) fn pintar(
    p: &Pintor,
    v: &VistaFiguras,
    area: Rect,
    tema: &Tema,
    escala: u32,
    textos: &Catalogo,
) {
    let e = escala as f32 / 100.0;
    let ei = |x: u32| x * escala / 100;
    let mut zonas = v.zonas.borrow_mut();
    zonas.clear();
    p.rellenar(rf(area), tema.chat);
    let margen = ei(MARGEN) as i32;

    // Las fichas de los grupos.
    let mut x = area.x + margen;
    let y_fichas = area.y + ei(12) as i32;
    let grupos = v.grupos();
    let tam = 13.0 * e;
    let mut fichas: Vec<(Option<String>, String)> = vec![(
        None,
        format!("{} · {}", textos.t("figuras-todas"), v.figuras.len()),
    )];
    fichas.extend(grupos.iter().map(|(g, n)| {
        (
            Some(g.clone()),
            format!("{} · {n}", rotulo_del_grupo(g, textos)),
        )
    }));
    for (g, rotulo) in &fichas {
        let (w, h) = p.medir_texto(rotulo, tam);
        let caja = Rect {
            x,
            y: y_fichas,
            ancho: (w + 24.0 * e) as u32,
            alto: ei(ALTO_FICHAS) - ei(6),
        };
        if caja.derecha() > area.derecha() - margen {
            break;
        }
        let elegida = &v.filtro == g;
        let encima = caja.contiene(v.raton);
        p.rellenar_redondeado(
            rf(caja),
            caja.alto as f32 / 2.0,
            if elegida {
                tema.fila_elegida
            } else if encima {
                tema.fila_sobre
            } else {
                tema.boton_sobre
            },
        );
        p.texto(
            rotulo,
            caja.x as f32 + 12.0 * e,
            caja.y as f32 + (caja.alto as f32 - h) / 2.0,
            tam,
            if elegida {
                tema.texto_elegido
            } else {
                tema.texto
            },
        );
        zonas.push((caja, Zona::Grupo(g.clone())));
        x = caja.derecha() + ei(8) as i32;
    }

    // La rejilla, por grupos.
    let arriba = y_fichas + ei(ALTO_FICHAS) as i32 + ei(6) as i32;
    let dentro = Rect {
        x: area.x,
        y: arriba,
        ancho: area.ancho,
        alto: (area.abajo() - arriba).max(0) as u32,
    };
    let ancho_util = area.ancho.saturating_sub(2 * ei(MARGEN));
    let paso = ei(CELDA_ANCHO + HUECO);
    let columnas = ((ancho_util + ei(HUECO)) / paso.max(1)).max(1) as usize;
    let alto_celda = ei(CELDA_DIBUJO + CELDA_NOMBRE);
    p.empujar_recorte(rf(dentro));
    let mut y = arriba - v.scroll;
    if v.figuras.is_empty() {
        let vacio = textos.t("figuras-vacia");
        let (w, _) = p.medir_texto(&vacio, 14.0 * e);
        p.texto(
            &vacio,
            area.x as f32 + (area.ancho as f32 - w) / 2.0,
            arriba as f32 + 40.0 * e,
            14.0 * e,
            tema.apagado,
        );
    }
    for (g, n) in grupos
        .iter()
        .filter(|(g, _)| v.filtro.as_ref().is_none_or(|f| f == g))
    {
        y += ei(10) as i32;
        p.texto(
            &format!("{}  ({n})", rotulo_del_grupo(g, textos)),
            (area.x + margen) as f32,
            y as f32 + 6.0 * e,
            15.0 * e,
            tema.texto,
        );
        y += ei(ALTO_TITULO) as i32;
        let de_este: Vec<usize> = (0..v.figuras.len())
            .filter(|i| &v.figuras[*i].grupo == g)
            .collect();
        for (k, i) in de_este.iter().enumerate() {
            let (col, fila) = (k % columnas, k / columnas);
            let celda = Rect {
                x: area.x + margen + (col as u32 * paso) as i32,
                y: y + (fila as u32 * (alto_celda + ei(HUECO))) as i32,
                ancho: ei(CELDA_ANCHO),
                alto: alto_celda,
            };
            if celda.abajo() < dentro.y || celda.y > dentro.abajo() {
                continue;
            }
            pintar_celda(p, &v.figuras[*i], celda, tema, e, celda.contiene(v.raton));
            if celda.y >= dentro.y || celda.abajo() > dentro.y {
                zonas.push((celda, Zona::Figura(*i)));
            }
        }
        let filas = de_este.len().div_ceil(columnas) as u32;
        y += (filas * (alto_celda + ei(HUECO))) as i32;
    }
    p.soltar_recorte();
    v.alto_total
        .set((y + v.scroll - arriba - dentro.alto as i32 + ei(MARGEN) as i32).max(0));

    // La grande, encima de todo.
    if let Some(f) = v.grande.and_then(|i| v.figuras.get(i)) {
        p.rellenar(rf(area), con_alfa(Color::NEGRO, 0.55));
        let w = (area.ancho as f32 * 0.7).min(900.0 * e);
        let h = (area.alto as f32 * 0.8).min(680.0 * e);
        let caja = RectF {
            x: area.x as f32 + (area.ancho as f32 - w) / 2.0,
            y: area.y as f32 + (area.alto as f32 - h) / 2.0,
            ancho: w,
            alto: h,
        };
        p.rellenar_redondeado(caja, 14.0 * e, tema.papel);
        if let Some(d) = &f.dibujo {
            pintar_lienzo(
                p,
                d,
                RectF {
                    x: caja.x + 24.0 * e,
                    y: caja.y + 56.0 * e,
                    ancho: caja.ancho - 48.0 * e,
                    alto: caja.alto - 120.0 * e,
                },
                None,
            );
        }
        p.texto_linea(
            &f.nombre,
            caja.x + 24.0 * e,
            caja.y + 18.0 * e,
            17.0 * e,
            caja.ancho - 48.0 * e,
            tema.texto_papel,
        );
        let grande = Rect {
            x: caja.x as i32,
            y: caja.y as i32,
            ancho: caja.ancho as u32,
            alto: caja.alto as u32,
        };
        zonas.push((grande, Zona::Grande));
        // Abajo: cerrar y, si es del usuario, borrar.
        let boton = |x: f32, ancho: f32| Rect {
            x: x as i32,
            y: (caja.y + caja.alto - 52.0 * e) as i32,
            ancho: ancho as u32,
            alto: (36.0 * e) as u32,
        };
        let cerrar = boton(caja.x + caja.ancho - 24.0 * e - 120.0 * e, 120.0 * e);
        let mut botones = vec![(
            cerrar,
            Zona::Cerrar,
            textos.t("figuras-cerrar"),
            tema.enviar,
        )];
        if !f.de_serie {
            let borrar = boton(cerrar.x as f32 - 12.0 * e - 170.0 * e, 170.0 * e);
            let rotulo = if v.borrar_pedido {
                textos.t("figuras-borrar-seguro")
            } else {
                textos.t("figuras-borrar")
            };
            botones.push((
                borrar,
                Zona::Borrar,
                rotulo,
                crate::caja_dibujo::hex(0xd9534f),
            ));
        }
        for (r, z, rotulo, color) in botones {
            p.rellenar_redondeado(rf(r), 10.0 * e, color);
            let (tw, th) = p.medir_texto(&rotulo, 14.0 * e);
            p.texto(
                &rotulo,
                r.x as f32 + (r.ancho as f32 - tw) / 2.0,
                r.y as f32 + (r.alto as f32 - th) / 2.0,
                14.0 * e,
                Color::BLANCO,
            );
            zonas.push((r, z));
        }
    }
}

fn pintar_celda(p: &Pintor, f: &Vista, celda: Rect, tema: &Tema, e: f32, encima: bool) {
    let dibujo = RectF {
        x: celda.x as f32,
        y: celda.y as f32,
        ancho: celda.ancho as f32,
        alto: CELDA_DIBUJO as f32 * e,
    };
    if encima {
        p.rellenar_redondeado(
            RectF {
                x: dibujo.x - 4.0 * e,
                y: dibujo.y - 4.0 * e,
                ancho: dibujo.ancho + 8.0 * e,
                alto: celda.alto as f32 + 4.0 * e,
            },
            12.0 * e,
            tema.fila_sobre,
        );
    }
    p.rellenar_redondeado(dibujo, 10.0 * e, tema.papel);
    if let Some(d) = &f.dibujo {
        pintar_lienzo(
            p,
            d,
            RectF {
                x: dibujo.x + 10.0 * e,
                y: dibujo.y + 10.0 * e,
                ancho: dibujo.ancho - 20.0 * e,
                alto: dibujo.alto - 20.0 * e,
            },
            None,
        );
    }
    p.texto_linea(
        &f.nombre,
        dibujo.x + 2.0 * e,
        dibujo.y + dibujo.alto + 8.0 * e,
        12.5 * e,
        dibujo.ancho - 4.0 * e,
        tema.texto,
    );
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lo_del_usuario_va_primero_y_las_categorias_en_su_orden() {
        assert_eq!(orden_del_grupo(MIS_FIGURAS), 0);
        assert!(orden_del_grupo(CATEGORIAS[0]) < orden_del_grupo(CATEGORIAS[1]));
        // Caso negativo: una categoria inventada no se cuela delante de las
        // de serie.
        assert!(orden_del_grupo("Mi coleccion") > orden_del_grupo(CATEGORIAS[5]));
    }

    #[test]
    fn el_grupo_sale_del_nombre() {
        let f = FiguraGuardada {
            id: "x".into(),
            nombre: "Diario · Gratitud".into(),
            elementos: Vec::new(),
        };
        let v = vista_de(&f);
        assert_eq!(
            (v.grupo.as_str(), v.nombre.as_str()),
            ("Diario", "Gratitud")
        );
        assert!(v.dibujo.is_none(), "sin piezas no hay nada que dibujar");
        let mia = vista_de(&FiguraGuardada {
            nombre: "Mi árbol".into(),
            ..f
        });
        assert_eq!(mia.grupo, MIS_FIGURAS);
        assert!(!mia.de_serie);
    }
}
