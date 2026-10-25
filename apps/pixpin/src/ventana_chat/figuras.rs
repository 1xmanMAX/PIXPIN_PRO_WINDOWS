//! **La pestaña Figuras de la ventana del chat** (24-oct-2026, el usuario:
//! «la visualizacion de las figuras … que sea como una galeria donde yo pueda
//! ver las figuras asi realizadas agrupadas, de forma que sea todo intuitivo
//! y muy rapido de ver»; 26-oct: «la pestaña de figuras tiene que tener la
//! misma ui que las demas pestañas»).
//!
//! Es la tercera parte del interruptor de la barra (Chat · Proyectos ·
//! **Figuras**) y se queda en la misma ventana, con la misma cara que
//! Proyectos (`proyectos.rs`):
//!
//! - **A la izquierda, la lista de grupos** en el sitio de la de proyectos,
//!   con sus filas: el circulo de color con las iniciales, el nombre y
//!   cuantas figuras tiene. «Todas» arriba, luego «Mis figuras» y las
//!   categorias (la categoria va en el nombre, «Diario · Gratitud», ver
//!   `figuras_de_serie::categoria_de`). El buscador de arriba filtra por
//!   nombre, grupo y textos.
//! - **A la derecha, la tarjeta del grupo** como la de un proyecto: su
//!   nombre grande, cuantas tiene, la elegida en grande (la portada, con lo
//!   que mide y de que esta hecha) y la tira con todas debajo; el boton de
//!   rejilla las ensena todas a la vez. Las del usuario se pueden borrar
//!   (la papelera, dos clics).
//!
//! Estrecha, un solo panel: la lista, o la tarjeta con el circulo de volver.
//! Teclado: escribir busca, ←/→ recorren las figuras, ↑/↓ los grupos y
//! Escape borra lo buscado.

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant, SystemTime};

use pixpin_geom::{Punto, Rect};
use pixpin_motor2d::biblioteca::{self, FiguraGuardada};
use pixpin_motor2d::figuras_de_serie::{CATEGORIAS, categoria_de, es_de_serie};
use pixpin_render::icono::{Icono, Pintura, TrazoIcono};
use pixpin_render::letras::Letra;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_store::Catalogo;
use pixpin_ui::chat::{self, Disposicion};
use pixpin_ui::proyectos as ui;

use super::proyectos::{CLARA, OSCURA, Paleta};
use super::{
    LienzoVisto, Tema, color_avatar, colores_de_caja, con_alfa, encoger, giro_de_rueda, iniciales,
    mi, pildora, pintar_lienzo, redaccion, rf,
};
use crate::caja_dibujo::hex;

/// El grupo de lo que no lleva categoria: lo del usuario.
pub(super) const MIS_FIGURAS: &str = "";

const VK_ESCAPE: u32 = 0x1B;
const VK_IZQUIERDA: u32 = 0x25;
const VK_ARRIBA: u32 = 0x26;
const VK_DERECHA: u32 = 0x27;
const VK_ABAJO: u32 = 0x28;

/// El papel de las figuras: blanco, el del lienzo de fabrica.
const PAPEL: Color = hex(0xffffff);
/// El rojo de borrar.
const ROJO: Color = hex(0xd9534f);

/// `view_carousel` (`src/action/view_carousel`), el mismo que el de la
/// tarjeta de Proyectos: vuelve de la rejilla a la portada.
const VIEW_CAROUSEL: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[TrazoIcono {
        d: "M7 19h10V4H7v15zm-5-2h4V6H2v11zM18 6v11h4V6h-4z",
        relleno: Pintura::Actual,
        trazo: Pintura::Nada,
        grosor: 0.0,
        extremo_redondo: false,
        union_redonda: false,
        opacidad: 1.0,
        par_impar: false,
        matriz: None,
        mascara: None,
    }],
};

// --- Lo de cada figura -------------------------------------------------------

/// De que esta hecha una figura, para el previsualizador.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Piezas {
    pub total: usize,
    pub trazos: usize,
    pub lineas: usize,
    pub formas: usize,
    pub textos: usize,
    pub otras: usize,
}

/// Una figura, ya lista para pintar.
struct Vista {
    id: String,
    /// El nombre sin la categoria delante.
    nombre: String,
    grupo: String,
    de_serie: bool,
    tamano: (f32, f32),
    piezas: Piezas,
    /// Los textos que lleva, en orden de lectura (arriba abajo).
    letras: Vec<String>,
    dibujo: Option<LienzoVisto>,
}

pub(super) fn piezas_de(f: &FiguraGuardada) -> Piezas {
    use pixpin_motor2d::Figura as F;
    let mut p = Piezas::default();
    for e in f.elementos.iter().filter(|e| !e.borrado) {
        p.total += 1;
        match &e.figura {
            F::Lapiz { .. } | F::Resaltador { .. } => p.trazos += 1,
            F::Linea { .. } | F::Flecha { .. } | F::Arco { .. } | F::Cota { .. } => p.lineas += 1,
            F::Rectangulo | F::Rombo | F::Elipse | F::Region { .. } => p.formas += 1,
            F::Texto { .. } => p.textos += 1,
            _ => p.otras += 1,
        }
    }
    p
}

pub(super) fn letras_de(f: &FiguraGuardada) -> Vec<String> {
    let mut v: Vec<(f32, f32, String)> = f
        .elementos
        .iter()
        .filter(|e| !e.borrado)
        .filter_map(|e| match &e.figura {
            pixpin_motor2d::Figura::Texto { texto, .. } if !texto.trim().is_empty() => {
                Some((e.y, e.x, texto.trim().replace('\n', " ")))
            }
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    v.into_iter().map(|(_, _, t)| t).collect()
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
        tamano: f.tamano(),
        piezas: piezas_de(f),
        letras: letras_de(f),
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
        (640.0 / lado).min(1.0),
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

/// Si la figura casa con lo buscado (sin mayusculas ni tildes, en su
/// nombre, su grupo o sus textos).
fn casa(f: &Vista, consulta: &str) -> bool {
    let q = plegar(consulta.trim());
    q.is_empty()
        || plegar(&f.nombre).contains(&q)
        || plegar(&f.grupo).contains(&q)
        || f.letras.iter().any(|l| plegar(l).contains(&q))
}

pub(super) fn plegar(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            otra => otra,
        })
        .collect()
}


// --- La pestaña ------------------------------------------------------------------

/// Lo que se puede pulsar, apuntado al pintar: lo que no se ve no se pulsa.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Zona {
    /// Una fila de la lista: su grupo (`None`, todas).
    Fila(Option<String>),
    /// El circulo de volver a la lista, con la ventana estrecha.
    Volver,
    Rejilla,
    Borrar,
    Portada,
    /// El hueco de la tira o de la rejilla: ahi la rueda la corre de lado.
    Tira,
    /// Una miniatura: la figura, por su puesto en `figuras`.
    Figura(usize),
}

/// Una fila de la lista: el grupo (`None`, todas), su nombre y cuantas.
struct Fila {
    grupo: Option<String>,
    nombre: String,
    cuantas: usize,
}

/// La pestaña Figuras: lo leido de la biblioteca y lo que se esta mirando.
pub(super) struct VistaFiguras {
    figuras: Vec<Vista>,
    /// El grupo de la tarjeta; `None`, todas.
    grupo: Option<String>,
    /// El buscador de la lista, la caja de una linea comun.
    pub(super) busqueda: redaccion::CajaLinea,
    pub(super) buscando: bool,
    /// La figura de la portada, por su puesto en `figuras`.
    en_foco: Option<usize>,
    rejilla: bool,
    /// Lo corrida que esta la tira, y si tiene que ensenar la del foco.
    corrida: Cell<i32>,
    ver: Cell<bool>,
    /// Lo bajada que esta la lista.
    scroll: i32,
    /// La fila bajo el raton, por su grupo.
    sobre: Option<Option<String>>,
    borrar_pedido: bool,
    /// Con la ventana estrecha: la tarjeta en vez de la lista.
    en_tarjeta: bool,
    zonas: RefCell<Vec<(Rect, Zona)>>,
    /// Donde quedo la tira en el ultimo fotograma, para la rueda.
    tira: Cell<Option<(Rect, ui::Mosaico)>>,
    /// Cuando se leyo la biblioteca, para releerla si cambia por fuera (una
    /// figura guardada en un lienzo, llegada del movil).
    leida: Option<SystemTime>,
    mirado: Instant,
}

fn modificada() -> Option<SystemTime> {
    std::fs::metadata(crate::biblioteca_figuras::ruta())
        .and_then(|m| m.modified())
        .ok()
}

impl VistaFiguras {
    /// La pestaña con la biblioteca de las figuras guardadas.
    pub(super) fn nueva() -> VistaFiguras {
        let mut v = VistaFiguras::con(todas());
        v.leida = modificada();
        v
    }

    /// La pestaña con estas figuras (las pruebas no miran el disco).
    pub(super) fn con(figuras: Vec<FiguraGuardada>) -> VistaFiguras {
        let mut v = VistaFiguras {
            figuras: Vec::new(),
            grupo: None,
            busqueda: redaccion::CajaLinea::nueva(),
            buscando: false,
            en_foco: None,
            rejilla: false,
            corrida: Cell::new(0),
            ver: Cell::new(false),
            scroll: 0,
            sobre: None,
            borrar_pedido: false,
            en_tarjeta: false,
            zonas: RefCell::new(Vec::new()),
            tira: Cell::new(None),
            leida: None,
            mirado: Instant::now(),
        };
        v.poner(figuras);
        v
    }

    fn poner(&mut self, figuras: Vec<FiguraGuardada>) {
        let antes = self
            .en_foco
            .and_then(|i| self.figuras.get(i))
            .map(|f| f.id.clone());
        self.figuras = figuras.iter().map(vista_de).collect();
        // Las del usuario primero y despues las categorias en su orden.
        self.figuras.sort_by_key(|f| orden_del_grupo(&f.grupo));
        if let Some(g) = &self.grupo
            && !self.figuras.iter().any(|f| &f.grupo == g)
        {
            self.grupo = None;
        }
        self.en_foco = antes.and_then(|id| self.figuras.iter().position(|f| f.id == id));
    }

    /// Si la biblioteca cambio por fuera, se relee (mirandolo cada dos
    /// segundos como mucho). Devuelve si hay que repintar.
    pub(super) fn latido(&mut self) -> bool {
        if self.mirado.elapsed() < Duration::from_secs(2) {
            return false;
        }
        self.mirado = Instant::now();
        let ahora = modificada();
        if ahora == self.leida {
            return false;
        }
        self.leida = ahora;
        self.poner(todas());
        true
    }

    /// Lo que ocupa la ventana estrecha: la lista o la tarjeta.
    pub(super) fn vista(&self) -> chat::Vista {
        if self.en_tarjeta {
            chat::Vista::SoloChat
        } else {
            chat::Vista::SoloLista
        }
    }

    fn consulta(&self) -> &str {
        self.busqueda.texto()
    }

    /// Las filas de la lista: «Todas» y los grupos con algo que case con lo
    /// buscado, con cuantas tienen.
    fn filas(&self, textos: &Catalogo) -> Vec<Fila> {
        let q = self.consulta();
        let mut grupos: Vec<(String, usize)> = Vec::new();
        let mut total = 0;
        for f in self.figuras.iter().filter(|f| casa(f, q)) {
            total += 1;
            match grupos.iter_mut().find(|(g, _)| *g == f.grupo) {
                Some((_, n)) => *n += 1,
                None => grupos.push((f.grupo.clone(), 1)),
            }
        }
        let mut v = vec![Fila {
            grupo: None,
            nombre: textos.t("figuras-todas"),
            cuantas: total,
        }];
        v.extend(grupos.into_iter().map(|(g, n)| Fila {
            nombre: rotulo_del_grupo(&g, textos),
            grupo: Some(g),
            cuantas: n,
        }));
        v
    }

    /// Los grupos de la lista en su orden (`None`, todas), sin componer
    /// nombres: para ↑ y ↓.
    fn grupos(&self) -> Vec<Option<String>> {
        let q = self.consulta();
        let mut v = vec![None];
        for f in self.figuras.iter().filter(|f| casa(f, q)) {
            let g = Some(f.grupo.clone());
            if !v.contains(&g) {
                v.push(g);
            }
        }
        v
    }

    /// Las figuras de la tarjeta: las del grupo elegido que casan con lo
    /// buscado, por su puesto en `figuras`.
    fn de_la_tarjeta(&self) -> Vec<usize> {
        let q = self.consulta();
        (0..self.figuras.len())
            .filter(|&i| {
                let f = &self.figuras[i];
                self.grupo.as_ref().is_none_or(|g| &f.grupo == g) && casa(f, q)
            })
            .collect()
    }

    /// La de la portada: la elegida si esta en la tarjeta, si no la primera.
    fn foco_en(&self, de: &[usize]) -> Option<usize> {
        self.en_foco
            .filter(|i| de.contains(i))
            .or(de.first().copied())
    }

    // --- Pintar ----------------------------------------------------------------

    /// La lista de grupos en el sitio de la de proyectos, su buscador, y la
    /// tarjeta del grupo en el panel derecho. Va encima de lo que pinta el
    /// chat: la barra, el interruptor y los botones redondos son los suyos.
    pub(super) fn pintar(
        &self,
        p: &Pintor,
        d: &Disposicion,
        tema: &Tema,
        claro: bool,
        escala: u32,
        textos: &Catalogo,
    ) {
        self.zonas.borrow_mut().clear();
        self.tira.set(None);
        let filas = self.filas(textos);
        self.pintar_lista(p, d, tema, escala, textos, &filas);
        let pal = if claro { &CLARA } else { &OSCURA };
        self.pintar_tarjeta(p, d, tema, pal, escala, textos, &filas);
    }

    fn zona(&self, r: Rect, dentro: Rect, z: Zona) {
        if let Some(r) = r.interseccion(dentro) {
            self.zonas.borrow_mut().push((r, z));
        }
    }

    fn pintar_lista(
        &self,
        p: &Pintor,
        d: &Disposicion,
        tema: &Tema,
        escala: u32,
        textos: &Catalogo,
        filas: &[Fila],
    ) {
        let e = escala as f32 / 100.0;
        // El buscador: el mismo sitio y la misma caja que el de proyectos.
        let caja = d.buscador(escala);
        if caja.ancho > 0 {
            p.rellenar_redondeado(rf(caja), chat::BUSCADOR_RADIO as f32 * e, tema.buscador);
            let margen = chat::BUSCADOR_TEXTO_X as f32 * e;
            self.busqueda.pintar(
                p,
                rf(caja),
                RectF {
                    x: caja.x as f32 + margen,
                    ancho: (caja.ancho as f32 - 2.0 * margen).max(0.0),
                    ..rf(caja)
                },
                chat::BUSCADOR_TAM * e,
                &textos.t("figuras-buscar"),
                &colores_de_caja(tema, tema.texto, tema.apagado),
                self.buscando,
                Instant::now(),
            );
        }
        if d.filas.ancho == 0 || d.filas.alto == 0 {
            return;
        }
        // Las filas de proyectos (y su «+») quedan debajo: se tapan enteras.
        crate::tema_cosmos::rellenar_o_cielo(p, rf(d.filas), tema.lista);
        let linea = (1.0 * e).max(1.0);
        p.rellenar(
            RectF {
                x: d.lista.derecha() as f32 - linea,
                y: d.filas.y as f32,
                ancho: linea,
                alto: d.filas.alto as f32,
            },
            tema.separador,
        );
        if filas[0].cuantas == 0 {
            let vacio = if self.figuras.is_empty() {
                textos.t("figuras-vacia")
            } else {
                textos.t("figuras-sin-resultados")
            };
            let ancho = d.filas.ancho as f32 - 32.0 * e;
            let (w, _) = p.medir_texto_ajustado(&vacio, 13.0 * e, ancho);
            p.texto_ajustado(
                &vacio,
                d.filas.x as f32 + (d.filas.ancho as f32 - w) / 2.0,
                d.filas.y as f32 + 40.0 * e,
                13.0 * e,
                ancho,
                tema.apagado,
            );
            return;
        }
        p.empujar_recorte(rf(d.filas));
        let (primera, cuantas) = d.visibles(self.scroll, filas.len(), escala);
        for (i, f) in filas.iter().enumerate().skip(primera).take(cuantas) {
            let r = d.fila(i, self.scroll, escala);
            let elegida = self.grupo == f.grupo;
            if elegida {
                p.rellenar(rf(r), tema.fila_elegida);
            } else if self.sobre.as_ref() == Some(&f.grupo) {
                p.rellenar(rf(r), tema.fila_sobre);
            }
            self.zona(r, d.filas, Zona::Fila(f.grupo.clone()));
            let partes = chat::partes_fila(r, d.plegada, escala);
            // El avatar: un circulo de su color con las iniciales, como el
            // de un proyecto sin logo.
            let a = rf(partes.avatar);
            p.rellenar_redondeado(a, a.ancho / 2.0, color_avatar(&f.nombre));
            let letras = iniciales(&f.nombre);
            let tam = a.alto * 0.4;
            let (w, h) = p.medir_texto(&letras, tam);
            p.texto(
                &letras,
                a.x + (a.ancho - w) / 2.0,
                a.y + (a.alto - h) / 2.0,
                tam,
                Color::BLANCO,
            );
            if partes.ancho_texto == 0 {
                continue;
            }
            let (nombre_color, cuenta_color) = if elegida {
                (tema.texto_elegido, tema.texto_elegido)
            } else {
                (tema.texto, tema.apagado)
            };
            p.texto_linea(
                &f.nombre,
                partes.nombre.x as f32,
                partes.nombre.y as f32,
                chat::TEXTO_TAM * e,
                partes.ancho_texto as f32,
                nombre_color,
            );
            p.texto_linea(
                &cuantas_figuras(f.cuantas, textos),
                partes.resumen.x as f32,
                partes.resumen.y as f32,
                chat::TEXTO_TAM * e,
                partes.ancho_texto as f32,
                cuenta_color,
            );
        }
        p.soltar_recorte();
    }

    #[allow(clippy::too_many_arguments)]
    fn pintar_tarjeta(
        &self,
        p: &Pintor,
        d: &Disposicion,
        tema: &Tema,
        pal: &Paleta,
        escala: u32,
        textos: &Catalogo,
        filas: &[Fila],
    ) {
        let area = d.chat;
        if area.ancho == 0 || area.alto == 0 {
            return;
        }
        let e = escala as f32 / 100.0;
        p.rellenar(rf(area), pal.fondo);
        let panel = ui::panel(area, d.cabecera_chat.alto, escala, 0);
        // Estrecha, el circulo de volver del chat lleva a la lista.
        if d.lista.ancho == 0 {
            let volver = d.pildoras(escala).volver;
            pildora(p, tema, rf(volver), e);
            icono_centrado(p, &mi::ARROW_BACK, volver, 24.0 * e, tema.pildora_texto);
            self.zona(volver, area, Zona::Volver);
        }
        // Sin barra de acciones: la tarjeta se queda tambien con su sitio,
        // y el mismo margen que a la izquierda.
        let margen = ui::MARGEN * escala / 100;
        let visible = Rect {
            ancho: (area.derecha() - margen as i32 - panel.hueco.x).max(0) as u32,
            alto: (area.abajo() - margen as i32 - panel.hueco.y).max(0) as u32,
            ..panel.hueco
        };
        let r = visible;
        p.empujar_recorte(rf(visible));
        p.rellenar_redondeado(rf(r), ui::RADIO_TARJETA as f32 * e, pal.tarjeta);
        let nombre = filas
            .iter()
            .find(|f| f.grupo == self.grupo)
            .map(|f| f.nombre.clone())
            .unwrap_or_else(|| textos.t("figuras-todas"));
        let de = self.de_la_tarjeta();
        let hay = !de.is_empty();
        let foco = self.foco_en(&de);
        let tuya = foco
            .and_then(|i| self.figuras.get(i))
            .is_some_and(|f| !f.de_serie);

        // El nombre grande, en dos lineas como mucho, y cuantas debajo.
        let provisional = ui::tarjeta(r, escala, 0, hay, self.rejilla);
        let letra = Letra::de("Segoe UI Semibold");
        let tam = ui::NOMBRE_TAM * e;
        let (_, alto) =
            p.medir_con_letra(&nombre, tam, provisional.texto.ancho.max(1) as f32, &letra);
        let (_, linea) = p.medir_con_letra("Ag", tam, f32::MAX, &letra);
        let alto_nombre = alto.min(2.0 * linea).max(linea);
        let detalle = if self.borrar_pedido && tuya {
            textos.t("figuras-borrar-seguro")
        } else {
            cuantas_figuras(de.len(), textos)
        };
        let (_, alto_det) = p.medir_texto(&detalle, ui::DETALLE_TAM * e);
        let t = ui::tarjeta(
            r,
            escala,
            (alto_nombre + alto_det).ceil() as u32,
            hay,
            self.rejilla,
        );
        let texto = rf(t.texto);
        p.empujar_recorte(RectF {
            alto: alto_nombre,
            ..texto
        });
        p.texto_con_letra(
            &nombre,
            texto.x,
            texto.y,
            tam,
            texto.ancho.max(1.0),
            &letra,
            pal.texto,
        );
        p.soltar_recorte();
        p.texto_linea(
            &detalle,
            texto.x,
            texto.y + alto_nombre,
            ui::DETALLE_TAM * e,
            texto.ancho,
            if self.borrar_pedido && tuya {
                ROJO
            } else {
                pal.texto_suave
            },
        );
        // La rejilla o una a una, y la papelera si la de la portada es suya:
        // iconos de 20 donde la de un proyecto lleva los tres puntos.
        if hay {
            let icono = if self.rejilla {
                &VIEW_CAROUSEL
            } else {
                &mi::GRID_VIEW
            };
            icono_centrado(p, icono, t.rejilla, 20.0 * e, pal.texto);
            self.zona(t.rejilla, visible, Zona::Rejilla);
        }
        if tuya {
            let color = if self.borrar_pedido { ROJO } else { pal.texto };
            icono_centrado(p, &mi::DELETE, t.menu, 20.0 * e, color);
            self.zona(t.menu, visible, Zona::Borrar);
        }
        match foco {
            None => {
                let vacio = if self.figuras.is_empty() {
                    textos.t("figuras-vacia")
                } else {
                    textos.t("figuras-sin-resultados")
                };
                let ancho = t.hojas.ancho as f32 - 32.0 * e;
                let (w, h) = p.medir_texto_ajustado(&vacio, ui::DETALLE_TAM * e, ancho);
                p.texto_ajustado(
                    &vacio,
                    t.hojas.x as f32 + (t.hojas.ancho as f32 - w) / 2.0,
                    t.hojas.y as f32 + (t.hojas.alto as f32 - h) / 2.0,
                    ui::DETALLE_TAM * e,
                    ancho,
                    pal.texto_suave,
                );
            }
            Some(foco) if self.rejilla => {
                let m = ui::rejilla(t.hojas.alto, de.len(), escala);
                self.pintar_tira(p, pal, escala, &de, t.hojas, m, Some(foco), visible);
            }
            Some(foco) => {
                self.pintar_portada(p, pal, e, textos, foco, t.hojas, visible);
                let m = if t.apaisada {
                    ui::tira_en_filas(t.tira.alto, de.len(), escala)
                } else {
                    ui::tira(de.len())
                };
                self.pintar_tira(p, pal, escala, &de, t.tira, m, Some(foco), visible);
            }
        }
        p.soltar_recorte();
    }

    /// La figura que se esta mirando, en grande sobre su papel, con su
    /// nombre, si es de serie o tuya, lo que mide y de que esta hecha.
    #[allow(clippy::too_many_arguments)]
    fn pintar_portada(
        &self,
        p: &Pintor,
        pal: &Paleta,
        e: f32,
        textos: &Catalogo,
        n: usize,
        caja: Rect,
        visible: Rect,
    ) {
        if caja.ancho == 0 || caja.alto == 0 {
            return;
        }
        p.rellenar_redondeado(rf(caja), ui::RADIO_PORTADA as f32 * e, pal.variante);
        self.zona(caja, visible, Zona::Portada);
        let Some(f) = self.figuras.get(n) else { return };
        p.empujar_recorte(rf(caja));
        let papel = encoger(rf(caja), 10.0 * e);
        p.rellenar_redondeado(papel, ui::RADIO_HOJA as f32 * e, PAPEL);
        // La etiqueta va abajo: el dibujo deja su sitio.
        let tam = ui::PORTADA_ETIQUETA_TAM * e;
        let origen = if f.de_serie {
            textos.t("figuras-de-serie")
        } else {
            textos.t("figuras-tuya")
        };
        let arriba = format!("{} · {}", f.nombre, origen);
        let abajo = format!(
            "{:.0} × {:.0} · {}",
            f.tamano.0,
            f.tamano.1,
            resumen_de_piezas(&f.piezas, textos)
        );
        let (w1, h1) = p.medir_texto(&arriba, tam);
        let (w2, h2) = p.medir_texto(&abajo, tam);
        let alto_etiqueta = h1 + h2 + 8.0 * e;
        if let Some(dibujo) = &f.dibujo {
            let mut sitio = encoger(papel, 14.0 * e);
            sitio.alto = (sitio.alto - alto_etiqueta).max(1.0);
            pintar_lienzo(p, dibujo, sitio, None);
        }
        let w = w1.max(w2).min(caja.ancho as f32 - 32.0 * e).max(0.0);
        let etiqueta = RectF {
            x: caja.x as f32 + 8.0 * e,
            y: caja.abajo() as f32 - 8.0 * e - alto_etiqueta,
            ancho: w + 16.0 * e,
            alto: alto_etiqueta,
        };
        p.rellenar_redondeado(etiqueta, 6.0 * e, con_alfa(pal.fondo, 0.85));
        p.texto_linea(
            &arriba,
            etiqueta.x + 8.0 * e,
            etiqueta.y + 4.0 * e,
            tam,
            w + 1.0,
            pal.texto,
        );
        p.texto_linea(
            &abajo,
            etiqueta.x + 8.0 * e,
            etiqueta.y + 4.0 * e + h1,
            tam,
            w + 1.0,
            pal.texto_suave,
        );
        p.soltar_recorte();
    }

    /// La tira (o la rejilla): las figuras de la tarjeta en miniatura, en su
    /// hueco, que se corre de lado con la rueda.
    #[allow(clippy::too_many_arguments)]
    fn pintar_tira(
        &self,
        p: &Pintor,
        pal: &Paleta,
        escala: u32,
        de: &[usize],
        area: Rect,
        m: ui::Mosaico,
        foco: Option<usize>,
        visible: Rect,
    ) {
        if area.ancho == 0 || area.alto == 0 {
            return;
        }
        let maximo = m.desplazamiento_maximo(area.ancho, escala).max(0);
        let mut corrida = self.corrida.get();
        if self.ver.replace(false)
            && let Some(pos) = foco.and_then(|f| de.iter().position(|i| *i == f))
        {
            corrida = m.para_ver(pos, area.ancho, corrida, escala);
        }
        let corrida = corrida.clamp(0, maximo);
        self.corrida.set(corrida);
        self.tira.set(Some((area, m)));
        let dentro = area.interseccion(visible);
        if let Some(r) = dentro {
            self.zonas.borrow_mut().push((r, Zona::Tira));
        }
        let e = escala as f32 / 100.0;
        p.empujar_recorte(rf(area));
        for x in m.columnas_visibles(area.ancho, corrida, escala) {
            for fila in 0..m.filas {
                let Some(k) = m.indice(x, fila) else { continue };
                let n = de[k];
                let celda = m.celda(area, x, fila, corrida, escala);
                self.miniatura(p, pal, e, n, celda, foco == Some(n));
                if let Some(dentro) = dentro {
                    self.zona(celda, dentro, Zona::Figura(n));
                }
            }
        }
        p.soltar_recorte();
    }

    /// Una miniatura de la tira: la figura sobre su papel, el marco del
    /// primario si es la de la portada y su nombre debajo.
    fn miniatura(&self, p: &Pintor, pal: &Paleta, e: f32, n: usize, celda: Rect, en_foco: bool) {
        let Some(f) = self.figuras.get(n) else { return };
        let r = rf(celda);
        p.rellenar_redondeado(r, ui::RADIO_HOJA as f32 * e, PAPEL);
        if let Some(d) = &f.dibujo {
            pintar_lienzo(p, d, encoger(r, 6.0 * e), None);
        }
        if en_foco {
            p.trazar(encoger(r, 1.0 * e), 2.0 * e, pal.primario);
        } else {
            p.trazar(encoger(r, 0.5 * e), 1.0 * e, pal.filo);
        }
        let tam = ui::ETIQUETA_TAM * e;
        let (w, _) = p.medir_texto(&f.nombre, tam);
        p.texto_linea(
            &f.nombre,
            r.x + ((r.ancho - w) / 2.0).max(0.0),
            r.y + r.alto + 2.0 * e,
            tam,
            r.ancho,
            pal.texto,
        );
    }

    // --- Lo que hace el usuario --------------------------------------------------

    fn zona_en(&self, l: Punto) -> Option<Zona> {
        self.zonas
            .borrow()
            .iter()
            .rev()
            .find(|(r, _)| r.contiene(l))
            .map(|(_, z)| z.clone())
    }

    /// Un clic en la lista o en la tarjeta. Devuelve si hay que repintar.
    pub(super) fn pulsar(&mut self, l: Punto) -> bool {
        let zona = self.zona_en(l);
        if zona != Some(Zona::Borrar) {
            self.borrar_pedido = false;
        }
        match zona {
            Some(Zona::Fila(g)) => {
                if self.grupo != g {
                    self.grupo = g;
                    self.en_foco = None;
                    self.corrida.set(0);
                }
                self.en_tarjeta = true;
            }
            Some(Zona::Volver) => self.en_tarjeta = false,
            Some(Zona::Rejilla) => {
                self.rejilla = !self.rejilla;
                self.ver.set(true);
            }
            Some(Zona::Borrar) => {
                if self.borrar_pedido {
                    self.borrar();
                } else {
                    self.borrar_pedido = true;
                }
            }
            // En la rejilla, elegir una la sube a la portada, como en
            // Proyectos.
            Some(Zona::Figura(i)) => {
                self.en_foco = Some(i);
                if self.rejilla {
                    self.rejilla = false;
                }
                self.ver.set(true);
            }
            Some(Zona::Portada | Zona::Tira) | None => return false,
        }
        true
    }

    /// El raton se mueve: el resalte de la fila de debajo.
    pub(super) fn mover(&mut self, l: Punto) -> bool {
        let sobre = match self.zona_en(l) {
            Some(Zona::Fila(g)) => Some(g),
            _ => None,
        };
        let cambia = sobre != self.sobre;
        self.sobre = sobre;
        cambia
    }

    /// La rueda: sobre la lista la baja, sobre la tira la corre de lado y
    /// sobre la portada pasa a la figura de al lado.
    pub(super) fn rueda(
        &mut self,
        l: Punto,
        delta: i32,
        d: &Disposicion,
        escala: u32,
        textos: &Catalogo,
    ) -> bool {
        if d.filas.contiene(l) {
            let paso = (chat::FILA * 3 / 2 * escala / 100) as i32;
            let n = self.filas(textos).len();
            let nuevo = d.scroll_ajustado(self.scroll - giro_de_rueda(delta, paso), n, escala);
            let cambia = nuevo != self.scroll;
            self.scroll = nuevo;
            return cambia;
        }
        match self.zona_en(l) {
            Some(Zona::Tira | Zona::Figura(_)) => {
                let Some((area, m)) = self.tira.get() else {
                    return false;
                };
                let paso = (ui::ANCHO_COLUMNA * escala / 100) as i32;
                let maximo = m.desplazamiento_maximo(area.ancho, escala).max(0);
                let nueva = (self.corrida.get() - giro_de_rueda(delta, paso)).clamp(0, maximo);
                let cambia = nueva != self.corrida.get();
                self.corrida.set(nueva);
                cambia
            }
            Some(Zona::Portada) => self.paso(if delta < 0 { 1 } else { -1 }),
            _ => false,
        }
    }

    /// Las teclas de la pestaña (sin el buscador): devuelve si era suya.
    pub(super) fn tecla(&mut self, vk: u32) -> bool {
        match vk {
            VK_IZQUIERDA => {
                self.paso(-1);
                true
            }
            VK_DERECHA => {
                self.paso(1);
                true
            }
            VK_ARRIBA => {
                self.grupo_vecino(-1);
                true
            }
            VK_ABAJO => {
                self.grupo_vecino(1);
                true
            }
            VK_ESCAPE if !self.consulta().is_empty() => {
                self.busqueda.vaciar();
                self.buscado();
                true
            }
            VK_ESCAPE if self.en_tarjeta => {
                self.en_tarjeta = false;
                true
            }
            _ => false,
        }
    }

    /// Pone la pestaña como la dejaria el usuario, para las muestras.
    #[cfg(test)]
    pub(super) fn como(&mut self, grupo: Option<&str>, rejilla: bool, buscar: &str, en_tarjeta: bool) {
        self.busqueda.poner_texto(buscar);
        self.buscando = !buscar.is_empty();
        self.grupo = grupo.map(String::from);
        self.rejilla = rejilla;
        self.en_tarjeta = en_tarjeta;
        self.en_foco = None;
        self.corrida.set(0);
    }

    /// Lo buscado cambio: la lista y la tarjeta vuelven a «Todas».
    pub(super) fn buscado(&mut self) {
        self.grupo = None;
        self.en_foco = None;
        self.scroll = 0;
        self.corrida.set(0);
    }

    /// La figura de al lado en la tarjeta.
    fn paso(&mut self, k: i32) -> bool {
        let de = self.de_la_tarjeta();
        let Some(actual) = self.foco_en(&de) else {
            return false;
        };
        let pos = de.iter().position(|i| *i == actual).unwrap_or(0) as i32;
        let nueva = de[(pos + k).clamp(0, de.len() as i32 - 1) as usize];
        let cambia = nueva != actual;
        self.en_foco = Some(nueva);
        self.ver.set(true);
        cambia
    }

    /// El grupo de al lado en la lista.
    fn grupo_vecino(&mut self, k: i32) {
        let grupos = self.grupos();
        let pos = grupos.iter().position(|g| *g == self.grupo).unwrap_or(0) as i32;
        let nuevo = grupos[(pos + k).clamp(0, grupos.len() as i32 - 1) as usize].clone();
        if nuevo != self.grupo {
            self.grupo = nuevo;
            self.en_foco = None;
            self.corrida.set(0);
        }
    }

    /// Quita de la biblioteca la de la portada (solo las del usuario).
    fn borrar(&mut self) {
        self.borrar_pedido = false;
        let de = self.de_la_tarjeta();
        let Some(f) = self
            .foco_en(&de)
            .and_then(|i| self.figuras.get(i))
            .filter(|f| !f.de_serie)
        else {
            return;
        };
        let ruta = crate::biblioteca_figuras::ruta();
        let mut todas = biblioteca::cargar(&ruta);
        let antes = todas.len();
        todas.retain(|t| t.id != f.id);
        if todas.len() != antes && crate::biblioteca_figuras::guardar(&ruta, &todas) {
            tracing::info!(nombre = %f.nombre, "figura borrada desde la pestaña de figuras");
        }
        self.en_foco = None;
        self.poner(todas);
        self.leida = modificada();
    }
}

/// «26 figuras».
fn cuantas_figuras(n: usize, textos: &Catalogo) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("n", n as i64);
    textos.t_args("figuras-cuantas", &args)
}

/// Un icono de `lado` centrado en `caja`.
fn icono_centrado(p: &Pintor, icono: &Icono, caja: Rect, lado: f32, color: Color) {
    p.icono(
        icono,
        RectF {
            x: caja.x as f32 + (caja.ancho as f32 - lado) / 2.0,
            y: caja.y as f32 + (caja.alto as f32 - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        color,
    );
}

/// «23 · 8 líneas, 6 textos, 4 formas».
fn resumen_de_piezas(p: &Piezas, textos: &Catalogo) -> String {
    let mut partes = Vec::new();
    for (n, clave) in [
        (p.lineas, "figuras-lineas"),
        (p.formas, "figuras-formas"),
        (p.textos, "figuras-de-texto"),
        (p.trazos, "figuras-trazos"),
        (p.otras, "figuras-otras"),
    ] {
        if n > 0 {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("n", n as i64);
            partes.push(textos.t_args(clave, &args));
        }
    }
    if partes.is_empty() {
        p.total.to_string()
    } else {
        format!("{} · {}", p.total, partes.join(", "))
    }
}


// --- Adjuntar figuras al asistente (25-oct-2026) --------------------------------
//
// El usuario: «en la opcion de adjuntar del asistente … puedo adjuntar una
// figura para que la mejore, o decirle hazme una figura, y lo puedo anadir
// facilmente a la lista de figuras».

/// Las figuras de la biblioteca, en el orden de `biblioteca::cargar` (los
/// indices del menu de adjuntar apuntan aqui).
pub(super) fn todas() -> Vec<FiguraGuardada> {
    biblioteca::cargar(&crate::biblioteca_figuras::ruta())
}

/// Los grupos para el menu de adjuntar: `(grupo, [(indice, nombre)])`, lo del
/// usuario primero y las categorias en su orden.
pub(super) fn por_grupos(figuras: &[FiguraGuardada]) -> Vec<(String, Vec<(usize, String)>)> {
    let mut v: Vec<(String, Vec<(usize, String)>)> = Vec::new();
    for (i, f) in figuras.iter().enumerate() {
        let (g, n) = match categoria_de(&f.nombre) {
            Some((c, n)) => (c.to_string(), n.to_string()),
            None => (MIS_FIGURAS.to_string(), f.nombre.clone()),
        };
        match v.iter_mut().find(|(x, _)| *x == g) {
            Some((_, l)) => l.push((i, n)),
            None => v.push((g, vec![(i, n)])),
        }
    }
    v.sort_by_key(|(g, _)| orden_del_grupo(g));
    v
}

/// El nombre de un grupo en el menu.
pub(super) fn grupo_legible(g: &str, textos: &Catalogo) -> String {
    rotulo_del_grupo(g, textos)
}

/// **La escena de un lienzo con la figura dentro**, para adjuntarla al chat.
pub(super) fn escena_con(f: &FiguraGuardada) -> pixpin_motor2d::Escena {
    let mut escena = pixpin_motor2d::Escena::nueva();
    let (w, h) = f.tamano();
    biblioteca::estampar_en_escena(
        &mut escena,
        f,
        pixpin_motor2d::vector::Punto2::nuevo(w / 2.0, h / 2.0),
    );
    escena
}

/// **Guarda un lienzo del chat como figura** con `nombre` (la del mismo
/// nombre se sustituye). Lo que se guarda es lo que se ve en el lienzo, con
/// sus piezas tal cual.
pub(super) fn guardar_lienzo(escena: &pixpin_motor2d::Escena, nombre: &str) -> Result<(), String> {
    let piezas: Vec<pixpin_motor2d::Elemento> = escena.visibles().cloned().collect();
    if piezas.is_empty() {
        return Err("vacio".into());
    }
    let figura = FiguraGuardada {
        id: biblioteca::id_nuevo(),
        nombre: nombre.trim().chars().take(80).collect(),
        elementos: biblioteca::normalizar(&piezas),
    };
    if crate::biblioteca_figuras::anadir(&crate::biblioteca_figuras::ruta(), figura) {
        Ok(())
    } else {
        Err("no se pudo escribir".into())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_menu_de_adjuntar_agrupa_y_la_figura_entra_entera_en_su_lienzo() {
        let catalogo = pixpin_motor2d::figuras_de_serie::catalogo();
        let mut todas = vec![FiguraGuardada {
            id: "mia".into(),
            nombre: "Mi árbol".into(),
            elementos: catalogo[0].elementos.clone(),
        }];
        todas.extend(catalogo.iter().cloned());
        let g = por_grupos(&todas);
        assert_eq!(g[0].0, MIS_FIGURAS, "lo del usuario primero");
        assert_eq!(g[0].1, vec![(0, "Mi árbol".to_string())]);
        let total: usize = g.iter().map(|(_, l)| l.len()).sum();
        assert_eq!(total, todas.len(), "cada figura sale una vez");
        // La figura estampada es la misma, pieza a pieza.
        let e = escena_con(&catalogo[0]);
        assert_eq!(
            e.visibles().count(),
            catalogo[0].elementos.iter().filter(|x| !x.borrado).count()
        );
        let c = e.caja().unwrap();
        assert!(c.0 >= -0.5 && c.1 >= -0.5, "desde el origen: {c:?}");
    }

    #[test]
    fn lo_del_usuario_va_primero_y_las_categorias_en_su_orden() {
        assert_eq!(orden_del_grupo(MIS_FIGURAS), 0);
        assert!(orden_del_grupo(CATEGORIAS[0]) < orden_del_grupo(CATEGORIAS[1]));
        // Caso negativo: una categoria inventada no se cuela delante de las
        // de serie.
        assert!(
            orden_del_grupo("Mi coleccion") > orden_del_grupo(CATEGORIAS[CATEGORIAS.len() - 1])
        );
    }

    #[test]
    fn el_grupo_sale_del_nombre_y_se_busca_sin_tildes() {
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
        assert!(casa(&v, "GRATI") && casa(&v, "diario") && casa(&v, ""));
        assert!(!casa(&v, "árbol"), "lo que no esta no casa");
        let mia = vista_de(&FiguraGuardada {
            nombre: "Mi árbol".into(),
            ..f
        });
        assert_eq!(mia.grupo, MIS_FIGURAS);
        assert!(!mia.de_serie);
        assert!(casa(&mia, "arbol"), "sin tildes tambien");
    }

    #[test]
    fn el_previsualizador_cuenta_las_piezas_y_lee_los_textos() {
        let catalogo = pixpin_motor2d::figuras_de_serie::catalogo();
        let con_textos = catalogo
            .iter()
            .find(|f| piezas_de(f).textos > 1)
            .expect("alguna de serie lleva textos");
        let p = piezas_de(con_textos);
        assert_eq!(
            p.total,
            p.trazos + p.lineas + p.formas + p.textos + p.otras,
            "cada pieza cuenta una vez"
        );
        assert!(!letras_de(con_textos).is_empty());
        // Caso negativo: una figura sin textos no lista ninguno.
        let sin = FiguraGuardada {
            id: "y".into(),
            nombre: "Nada".into(),
            elementos: Vec::new(),
        };
        assert!(letras_de(&sin).is_empty());
        assert_eq!(piezas_de(&sin), Piezas::default());
    }

    #[test]
    fn la_pestana_agrupa_busca_y_se_recorre_con_el_teclado() {
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let mut v = VistaFiguras::con(pixpin_motor2d::figuras_de_serie::catalogo());
        let filas = v.filas(&textos);
        assert_eq!(filas[0].grupo, None, "«Todas» arriba");
        assert_eq!(filas[0].cuantas, v.figuras.len());
        let uml = filas.iter().find(|f| f.grupo.as_deref() == Some("UML"));
        assert!(uml.is_some_and(|f| f.cuantas > 0));
        // Buscar deja solo lo que casa, en la lista y en la tarjeta.
        v.busqueda.poner_texto("puerta");
        v.buscado();
        let de = v.de_la_tarjeta();
        assert!(!de.is_empty());
        assert!(de.iter().all(|i| casa(&v.figuras[*i], "puerta")));
        assert_eq!(v.filas(&textos)[0].cuantas, de.len());
        // → pasa a la siguiente y ↓ al grupo de debajo.
        let primera = v.foco_en(&de);
        assert!(v.tecla(VK_DERECHA));
        assert_ne!(v.foco_en(&v.de_la_tarjeta()), primera);
        assert!(v.tecla(VK_ABAJO));
        assert!(v.grupo.is_some());
        // Escape borra lo buscado y vuelve a «Todas».
        assert!(v.tecla(VK_ESCAPE));
        assert!(v.consulta().is_empty());
        assert_eq!(v.grupo, None);
        // Caso negativo: nada casa, nada en la tarjeta ni en «Todas».
        v.busqueda.poner_texto("zzzz-no-existe");
        v.buscado();
        assert!(v.de_la_tarjeta().is_empty());
        assert_eq!(v.filas(&textos)[0].cuantas, 0);
        // Y estrecha: elegir un grupo pasa a la tarjeta y Escape vuelve.
        v.busqueda.vaciar();
        v.en_tarjeta = true;
        assert_eq!(v.vista(), chat::Vista::SoloChat);
        assert!(v.tecla(VK_ESCAPE));
        assert_eq!(v.vista(), chat::Vista::SoloLista);
    }
}
