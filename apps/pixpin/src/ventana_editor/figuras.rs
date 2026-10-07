//! **Las figuras del lienzo** (F12, F14): la biblioteca de la barra, la
//! grafica de una funcion con su ecuacion tipografiada, la tabla en blanco,
//! pegar una tabla de Excel, meter una imagen desde un fichero y la foto de
//! la Zona.
//!
//! Es la mitad de ventana de lo que en el motor es puro (`grafica`,
//! `ecuacion`, `tabla_dibujada`, `biblioteca`, `zona`): el menu, el cajetin
//! de la formula, el fichero donde viven las figuras guardadas y medir las
//! letras con DirectWrite. En el movil es `PanelDeFiguras` +
//! `DialogoDeGrafica` (`DrawFiguras.kt`) y `EditorDeTablaPegada`; aqui el
//! panel es el menu nativo de Windows (sabe de teclado y de bordes de
//! pantalla, y una lista corta no pide mas), y el dialogo, un cajetin
//! dibujado sobre el lienzo como el de calibrar.

use super::{FueraDeLaEscena, dibujar_orden, pintar};
use crate::fondo_lienzo::FondoLienzo;
use crate::imagenes_lienzo::ImagenesLienzo;
use pixpin_motor2d::biblioteca::{self, FiguraGuardada};
use pixpin_motor2d::cache::Cache;
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::ecuacion::{self, Estilo};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::Gesto;
use pixpin_motor2d::grafica::{self, Fallo, Peticion};
use pixpin_motor2d::indice::Rejilla;
use pixpin_motor2d::vector::Punto2;
use pixpin_motor2d::{ColorRgba, Elemento};
use pixpin_render::{CapaEstatica, Color, MotorRender, Pintor, RectF, Superficie};
use pixpin_shell::exportar::EntradaMenu;
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Catalogo;
use pixpin_ui::CajaHerramientas;
use std::path::PathBuf;
use windows::Win32::Foundation::HWND;

/// Lo que mide una letra con DirectWrite, que es con lo que se pinta: asi
/// las piezas de una ecuacion quedan pegadas donde toca. Si DirectWrite no
/// contesta, la cuenta a ojo del motor.
///
/// Mide con `familia`, la MISMA letra con la que se pintaran los textos: el
/// pincel nace en Excalifont, bastante mas ancha que Segoe UI, y medido en
/// Segoe el texto se salia de su celda y las piezas de una ecuacion se
/// montaban unas sobre otras.
pub(super) fn medidor<'a>(
    motor: &'a MotorRender,
    familia: &str,
) -> impl Fn(&str, f32) -> (f32, f32) + 'a {
    let familia = familia.to_string();
    move |texto, tam| {
        if pixpin_motor2d::texto::hay_medidor() {
            let estilo = pixpin_motor2d::texto::EstiloDeTexto::default();
            return pixpin_motor2d::texto::medida(texto, tam, &familia, estilo);
        }
        let (w, h) = motor.medir_texto(texto, tam, f32::MAX);
        if w > 0.0 && h > 0.0 {
            (w, h)
        } else {
            (
                pixpin_motor2d::texto::ancho_de_renglon(texto, tam),
                tam * pixpin_motor2d::texto::INTERLINEADO,
            )
        }
    }
}

/// El pincel de ahora como estilo de figura: el color, la letra y la
/// opacidad que el panel dejo como «actuales», como hace el movil con su
/// `scene.style`.
pub(super) fn estilo_del_pincel(g: &Gesto) -> Estilo {
    let tam = if g.estilo.tamano_letra > 0.0 {
        g.estilo.tamano_letra
    } else {
        pixpin_motor2d::texto::TAM_POR_DEFECTO
    };
    Estilo {
        color: g.estilo.trazo,
        tam,
        opacidad: g.estilo.opacidad,
        familia: pixpin_motor2d::texto::nombre_de_familia(g.estilo.familia).to_string(),
    }
}

/// El centro de lo que se ve, en el dibujo (`centroDeLaVista` del movil).
pub(super) fn centro_de_la_vista(camara: &Camara, ancho_px: f32, alto_px: f32) -> Punto2 {
    let (x0, y0, x1, y1) = camara.ventana(ancho_px, alto_px);
    Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0)
}

/// **Estampa unos elementos en el medio de la vista**, agrupados, en un paso
/// de deshacer, y los deja elegidos para llevarlos a su sitio de un tiron.
pub(super) fn estampar_en_la_vista(
    escena: &mut Escena,
    gesto: &mut Gesto,
    centro: Punto2,
    nombre: &str,
    elementos: &[Elemento],
) -> bool {
    if elementos.is_empty() {
        return false;
    }
    let figura = FiguraGuardada {
        nombre: nombre.to_string(),
        elementos: biblioteca::normalizar(elementos),
    };
    let ids = biblioteca::estampar_en_escena(escena, &figura, centro);
    let hubo = !ids.is_empty();
    if hubo {
        gesto.seleccion.poner_todos(ids);
    }
    hubo
}

/// **Pega la tabla que trae el portapapeles** (Ctrl+V con unas celdas de
/// Excel copiadas). `false` si el texto no es una tabla: entonces Ctrl+V
/// sigue su camino de siempre.
///
/// Si ademas viene el `HTML Format` de Sheets o de Excel, manda el: trae las
/// celdas combinadas y la negrita, que el texto con tabuladores pierde. Solo
/// se mira cuando el texto ya es una tabla, para que copiar un parrafo de una
/// pagina web maquetada con tablas no acabe pegando una rejilla.
pub(super) fn pegar_tabla(
    html: Option<&str>,
    texto: &str,
    escena: &mut Escena,
    gesto: &mut Gesto,
    centro: Punto2,
    medir: ecuacion::Medir<'_>,
) -> bool {
    use pixpin_motor2d::tabla_dibujada::{self, Celda};
    let Some(filas) = tabla_dibujada::rejilla_de_texto(texto) else {
        return false;
    };
    let estilo = estilo_del_pincel(gesto);
    let origen = Punto2::nuevo(0.0, 0.0);
    let vista = html.and_then(pixpin_proyecto::portapapeles_tabla::vista_de_html);
    let v = match vista {
        // Como en la hoja: la cabecera se distingue por su negrita, sin el
        // fondo gris de la tabla que viene solo de texto.
        Some(vista) => {
            let celdas: Vec<Vec<Celda>> = vista
                .into_iter()
                .map(|f| {
                    f.into_iter()
                        .map(|c| Celda {
                            texto: c.texto,
                            negrita: c.negrita,
                            filas: c.filas,
                            columnas: c.columnas,
                            tapada: c.tapada,
                        })
                        .collect()
                })
                .collect();
            tabla_dibujada::elementos_de_tabla_con_juntas(&celdas, &estilo, origen, medir, false)
        }
        None => tabla_dibujada::elementos_de_tabla(&filas, &estilo, origen, medir, true),
    };
    estampar_en_la_vista(escena, gesto, centro, "tabla", &v)
}

// ---------------------------------------------------------------------------
// La biblioteca en disco
// ---------------------------------------------------------------------------

/// Donde viven las figuras guardadas: **un fichero aparte y no dentro de
/// cada dibujo** (`BibliotecaStore` del movil), para que una figura guardada
/// en un croquis este puesta al abrir el siguiente.
pub(super) fn ruta_biblioteca() -> PathBuf {
    let dir_exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_default();
    let appdata = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_default();
    pixpin_store::resolver(&dir_exe, &appdata)
        .raiz()
        .join("figuras.json")
}

/// Las figuras guardadas, o ninguna. Un fichero ilegible es como si no
/// hubiera: perderlas es malo, no poder abrir el lienzo por ellas es peor.
pub(super) fn cargar_biblioteca(ruta: &std::path::Path) -> Vec<FiguraGuardada> {
    std::fs::read(ruta)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Escribe la lista entera, a un temporal y renombrando: un corte a medias
/// no puede dejar la biblioteca rota.
pub(super) fn guardar_biblioteca(ruta: &std::path::Path, figuras: &[FiguraGuardada]) -> bool {
    let Ok(datos) = serde_json::to_vec(figuras) else {
        return false;
    };
    if let Some(dir) = ruta.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let temporal = ruta.with_extension("json.tmp");
    std::fs::write(&temporal, datos).is_ok() && std::fs::rename(&temporal, ruta).is_ok()
}

// ---------------------------------------------------------------------------
// El menu
// ---------------------------------------------------------------------------

/// Lo que se elige en el menu de las figuras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Eleccion {
    Grafica,
    TablaEnBlanco,
    PegarTabla,
    GuardarSeleccion,
    Estampar(usize),
    Quitar(usize),
    /// Abrir el cajetin de la tabla elegida (filas, columnas, celdas).
    EditarTabla,
    /// Meter en su celda lo elegido que esta encima de la tabla.
    MeterEnCelda,
}

/// Que tabla hay en lo elegido, para ofrecer lo suyo en el menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TablaElegida {
    Ninguna,
    Sola,
    /// La tabla y algo encima de alguna de sus celdas.
    ConFiguras,
}

impl TablaElegida {
    pub fn de(escena: &Escena, gesto: &Gesto) -> TablaElegida {
        let elegidos: Vec<&Elemento> = escena
            .visibles()
            .filter(|e| gesto.seleccion.contiene(e.id))
            .collect();
        match pixpin_motor2d::tabla_dibujada::leer::leer_tabla(&elegidos) {
            None => TablaElegida::Ninguna,
            Some(t) if t.figuras.is_empty() => TablaElegida::Sola,
            Some(_) => TablaElegida::ConFiguras,
        }
    }
}

const ID_GRAFICA: u32 = 1;
const ID_TABLA: u32 = 2;
const ID_PEGAR: u32 = 3;
const ID_GUARDAR: u32 = 4;
const ID_EDITAR_TABLA: u32 = 5;
const ID_METER: u32 = 6;
const ID_FIGURA: u32 = 100;
const ID_QUITAR: u32 = 1000;

/// Las entradas del menu, puras (se prueban sin ventana). Con una tabla
/// elegida, lo suyo arriba (editarla, meter en una celda lo que tiene
/// encima); luego las de fabrica —la grafica, la tabla en blanco, pegar—,
/// las propias, las nuevas arriba (`BibliotecaStore.anadir`), y al final
/// guardar y quitar.
pub(super) fn entradas(
    t: &Catalogo,
    figuras: &[FiguraGuardada],
    hay_seleccion: bool,
    tabla: TablaElegida,
) -> Vec<EntradaMenu> {
    let mut v = Vec::new();
    if tabla != TablaElegida::Ninguna {
        v.push(EntradaMenu {
            id: ID_EDITAR_TABLA,
            texto: t.t("figuras-editar-tabla"),
        });
        if tabla == TablaElegida::ConFiguras {
            v.push(EntradaMenu {
                id: ID_METER,
                texto: t.t("figuras-meter-en-celda"),
            });
        }
        v.push(EntradaMenu {
            id: 0,
            texto: String::new(),
        });
    }
    v.extend([
        EntradaMenu {
            id: ID_GRAFICA,
            texto: t.t("figuras-grafica"),
        },
        EntradaMenu {
            id: ID_TABLA,
            texto: t.t("figuras-tabla-en-blanco"),
        },
        EntradaMenu {
            id: ID_PEGAR,
            texto: t.t("figuras-pegar-tabla"),
        },
    ]);
    if !figuras.is_empty() {
        v.push(EntradaMenu {
            id: 0,
            texto: String::new(),
        });
        for (i, f) in figuras.iter().enumerate() {
            v.push(EntradaMenu {
                id: ID_FIGURA + i as u32,
                texto: f.nombre.clone(),
            });
        }
    }
    v.push(EntradaMenu {
        id: 0,
        texto: String::new(),
    });
    if hay_seleccion {
        v.push(EntradaMenu {
            id: ID_GUARDAR,
            texto: t.t("figuras-guardar-seleccion"),
        });
    }
    for (i, f) in figuras.iter().enumerate() {
        v.push(EntradaMenu {
            id: ID_QUITAR + i as u32,
            texto: format!("{} «{}»", t.t("figuras-quitar"), f.nombre),
        });
    }
    // Sin separador colgando al final.
    if v.last().is_some_and(|e| e.id == 0) {
        v.pop();
    }
    v
}

/// De la entrada elegida a lo que significa.
pub(super) fn eleccion_de(id: u32, figuras: usize) -> Option<Eleccion> {
    Some(match id {
        ID_GRAFICA => Eleccion::Grafica,
        ID_TABLA => Eleccion::TablaEnBlanco,
        ID_PEGAR => Eleccion::PegarTabla,
        ID_GUARDAR => Eleccion::GuardarSeleccion,
        ID_EDITAR_TABLA => Eleccion::EditarTabla,
        ID_METER => Eleccion::MeterEnCelda,
        i if (ID_QUITAR..ID_QUITAR + figuras as u32).contains(&i) => {
            Eleccion::Quitar((i - ID_QUITAR) as usize)
        }
        i if (ID_FIGURA..ID_FIGURA + figuras as u32).contains(&i) => {
            Eleccion::Estampar((i - ID_FIGURA) as usize)
        }
        _ => return None,
    })
}

/// El menu, bajo el boton de las figuras.
pub(super) fn menu(
    propietaria: HWND,
    x: i32,
    y: i32,
    t: &Catalogo,
    figuras: &[FiguraGuardada],
    hay_seleccion: bool,
    tabla: TablaElegida,
) -> Option<Eleccion> {
    let v = entradas(t, figuras, hay_seleccion, tabla);
    let id = pixpin_shell::exportar::menu_emergente(propietaria, x, y, &v)?;
    eleccion_de(id, figuras.len())
}

// ---------------------------------------------------------------------------
// El cajetin: la grafica, la tabla, el cronograma y el nombre de una figura
// ---------------------------------------------------------------------------

/// Un campo del cajetin.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct Campo {
    pub etiqueta: String,
    pub texto: String,
    /// Admite varios renglones (las formulas: una curva por renglon).
    pub renglones: bool,
    /// Va a medio ancho: dos seguidos comparten fila, como los limites del
    /// dialogo de la grafica del movil (`x desde | x hasta`).
    pub media: bool,
    /// Lo que se lee en gris mientras esta vacio (el `placeholder`).
    pub pista: String,
}

impl Campo {
    pub fn nuevo(etiqueta: impl Into<String>, texto: impl Into<String>) -> Campo {
        Campo {
            etiqueta: etiqueta.into(),
            texto: texto.into(),
            ..Default::default()
        }
    }

    pub fn de_renglones(mut self) -> Campo {
        self.renglones = true;
        self
    }

    pub fn a_medias(mut self) -> Campo {
        self.media = true;
        self
    }

    pub fn con_pista(mut self, pista: impl Into<String>) -> Campo {
        self.pista = pista.into();
        self
    }
}

/// Los botones de la tabla, los del editor de tabla del movil
/// (`EditorDeTablaPegada`: pegar, fila, columna, quitar fila y la casilla de
/// cabecera) mas quitar columna, que alli no hay y aqui se pidio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Accion {
    MasFila,
    MenosFila,
    MasColumna,
    MenosColumna,
    Cabecera,
    Pegar,
}

pub(super) const ACCIONES: [Accion; 6] = [
    Accion::MasFila,
    Accion::MenosFila,
    Accion::MasColumna,
    Accion::MenosColumna,
    Accion::Cabecera,
    Accion::Pegar,
];

/// Una tabla dentro del cajetin: sus celdas son los campos, fila a fila.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TablaEnCajetin {
    pub columnas: usize,
    pub cabecera: bool,
}

/// Lo que se teclea en el cajetin: los campos y cual tiene el cursor.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Formulario {
    pub titulo: String,
    pub campos: Vec<Campo>,
    pub activo: usize,
    pub aviso: Option<String>,
    pub ayuda: String,
    /// El rotulo del boton que acepta («Insertar», «Aplicar»...).
    pub aceptar: String,
    pub cancelar: String,
    /// Con el teclado de formulas del movil (`TecladoDeFormulas`), que
    /// escribe en el primer campo.
    pub teclado: bool,
    /// Si es una tabla: los campos son sus celdas.
    pub rejilla: Option<TablaEnCajetin>,
    /// Los rotulos de `ACCIONES`, en su orden (solo con rejilla).
    pub rotulos_de_acciones: Vec<String>,
}

/// Lo que hace una tecla o un clic en el cajetin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tecla {
    Sigue,
    Aceptar,
    Cancelar,
    /// Un boton de la tabla que necesita a quien llama (pegar lee el
    /// portapapeles); los demas los resuelve el propio formulario.
    Accion(Accion),
}

/// **El teclado de formulas del movil** (`TecladoDeFormulas`), fila a fila:
/// lo que ensena cada tecla y lo que escribe. Aqui se escribe siempre al
/// final del campo, asi que las flechas del movil (mover el cursor) no
/// hacen falta y su hueco es «otra curva» (un renglon nuevo). `\u{8}` borra.
pub(super) const TECLADO: [&[(&str, &str)]; 7] = [
    &[
        ("7", "7"),
        ("8", "8"),
        ("9", "9"),
        ("÷", "/"),
        ("(", "("),
        (")", ")"),
    ],
    &[
        ("4", "4"),
        ("5", "5"),
        ("6", "6"),
        ("×", "*"),
        ("^", "^"),
        ("x²", "^2"),
    ],
    &[
        ("1", "1"),
        ("2", "2"),
        ("3", "3"),
        ("−", "-"),
        ("√", "sqrt("),
        ("π", "pi"),
    ],
    &[
        ("0", "0"),
        (".", "."),
        (",", ","),
        ("+", "+"),
        ("x", "x"),
        ("e", "e"),
    ],
    &[
        ("sin", "sin("),
        ("cos", "cos("),
        ("tan", "tan("),
        ("ln", "ln("),
        ("log", "log("),
        ("exp", "exp("),
    ],
    &[
        ("abs", "abs("),
        ("|x|", "|"),
        ("<", "<"),
        (">", ">"),
        ("≤", "<="),
        ("≥", ">="),
    ],
    &[("si", " si "), (";", "; "), ("↵", "\n"), ("⌫", "\u{8}")],
];

/// Las teclas «fuertes» del movil (funciones, la x y borrar), que se pintan
/// de otro tono para encontrarlas de un vistazo.
fn tecla_fuerte(rotulo: &str) -> bool {
    (rotulo.len() > 1 && rotulo.chars().all(|c| c.is_ascii_alphabetic()))
        || matches!(rotulo, "x" | "√" | "⌫" | "↵")
}

impl Formulario {
    pub fn nuevo(
        titulo: String,
        campos: Vec<Campo>,
        ayuda: String,
        aceptar: String,
        cancelar: String,
    ) -> Formulario {
        Formulario {
            titulo,
            campos,
            activo: 0,
            aviso: None,
            ayuda,
            aceptar,
            cancelar,
            teclado: false,
            rejilla: None,
            rotulos_de_acciones: Vec::new(),
        }
    }

    /// **Una tecla del cajetin**, pura: Intro acepta, Mayus+Intro abre otro
    /// renglon en las formulas (otra curva), Tab y Mayus+Tab cambian de
    /// campo (de celda en una tabla), Retroceso borra y Escape cancela.
    pub fn tecla(&mut self, c: char, mayus: bool) -> Tecla {
        if self.campos.is_empty() {
            return match c {
                '\u{1b}' => Tecla::Cancelar,
                '\r' | '\n' => Tecla::Aceptar,
                _ => Tecla::Sigue,
            };
        }
        match c {
            '\r' | '\n' if mayus && self.campos[self.activo].renglones => {
                self.campos[self.activo].texto.push('\n');
                Tecla::Sigue
            }
            '\r' | '\n' => Tecla::Aceptar,
            '\u{1b}' => Tecla::Cancelar,
            '\t' => {
                let n = self.campos.len();
                self.activo = if mayus {
                    (self.activo + n - 1) % n
                } else {
                    (self.activo + 1) % n
                };
                Tecla::Sigue
            }
            '\u{8}' => {
                self.campos[self.activo].texto.pop();
                self.aviso = None;
                Tecla::Sigue
            }
            c if c >= ' ' => {
                self.campos[self.activo].texto.push(c);
                self.aviso = None;
                Tecla::Sigue
            }
            _ => Tecla::Sigue,
        }
    }

    /// Las flechas en una tabla: de celda en celda, como en una hoja.
    pub fn flecha(&mut self, vk: u32) {
        let Some(r) = self.rejilla else { return };
        let n = self.campos.len();
        let cols = r.columnas.max(1);
        let (f, c) = (self.activo / cols, self.activo % cols);
        let filas = n / cols;
        let (f, c) = match vk {
            0x25 => (f, c.saturating_sub(1)),
            0x27 => (f, (c + 1).min(cols - 1)),
            0x26 => (f.saturating_sub(1), c),
            0x28 => ((f + 1).min(filas.saturating_sub(1)), c),
            _ => return,
        };
        self.activo = (f * cols + c).min(n.saturating_sub(1));
    }

    /// Pega texto en el campo activo; en uno de un renglon, sin saltos.
    pub fn pegar(&mut self, texto: &str) {
        let Some(campo) = self.campos.get_mut(self.activo) else {
            return;
        };
        if campo.renglones {
            campo.texto.push_str(&texto.replace("\r\n", "\n"));
        } else {
            // Una celda copiada de Excel trae su salto de linea detras: sobra.
            let t = texto.replace("\r\n", "\n");
            campo
                .texto
                .push_str(&t.trim_end_matches('\n').replace('\n', " "));
        }
        self.aviso = None;
    }

    /// Una tecla del teclado de formulas: escribe en las formulas y las deja
    /// con el cursor.
    pub fn teclear(&mut self, escribe: &str) {
        let Some(campo) = self.campos.first_mut() else {
            return;
        };
        if escribe == "\u{8}" {
            campo.texto.pop();
        } else {
            campo.texto.push_str(escribe);
        }
        self.activo = 0;
        self.aviso = None;
    }

    /// Las celdas de la tabla, fila a fila.
    pub fn celdas(&self) -> Vec<Vec<String>> {
        let cols = self.rejilla.map_or(1, |r| r.columnas.max(1));
        self.campos
            .chunks(cols)
            .map(|f| f.iter().map(|c| c.texto.clone()).collect())
            .collect()
    }

    /// Pone otras celdas en la tabla (lo que trae «pegar»). Una rejilla vacia
    /// no cambia nada: el movil tampoco tira lo tecleado si el portapapeles
    /// no trae una tabla.
    pub fn poner_celdas(&mut self, filas: &[Vec<String>]) -> bool {
        let cols = filas.iter().map(Vec::len).max().unwrap_or(0);
        let Some(r) = self.rejilla.as_mut() else {
            return false;
        };
        if filas.is_empty() || cols == 0 {
            return false;
        }
        r.columnas = cols;
        self.campos = filas
            .iter()
            .flat_map(|f| {
                (0..cols).map(|i| Campo::nuevo("", f.get(i).cloned().unwrap_or_default()))
            })
            .collect();
        self.activo = 0;
        true
    }

    /// **Un boton de la tabla.** Anadir fila o columna las pone al final,
    /// como el movil; quitar quita la de la celda activa, y nunca la ultima
    /// que queda (una tabla sin filas no se puede volver a llenar). Pegar lo
    /// resuelve quien llama, que es quien lee el portapapeles.
    pub fn accion(&mut self, a: Accion) -> Tecla {
        let Some(r) = self.rejilla else {
            return Tecla::Sigue;
        };
        let cols = r.columnas.max(1);
        let filas = self.campos.len() / cols;
        let (f, c) = (self.activo / cols, self.activo % cols);
        let mut celdas = self.celdas();
        match a {
            Accion::MasFila => {
                celdas.push(vec![String::new(); cols]);
                self.poner_celdas(&celdas);
                self.activo = filas * cols;
            }
            Accion::MenosFila if filas > 1 => {
                celdas.remove(f);
                self.poner_celdas(&celdas);
                self.activo = (f.min(filas - 2)) * cols + c;
            }
            Accion::MasColumna => {
                for fila in &mut celdas {
                    fila.push(String::new());
                }
                self.poner_celdas(&celdas);
                self.activo = f * (cols + 1) + cols;
            }
            Accion::MenosColumna if cols > 1 => {
                for fila in &mut celdas {
                    fila.remove(c);
                }
                self.poner_celdas(&celdas);
                self.activo = f * (cols - 1) + c.min(cols - 2);
            }
            Accion::Cabecera => {
                if let Some(r) = self.rejilla.as_mut() {
                    r.cabecera = !r.cabecera;
                }
            }
            Accion::Pegar => return Tecla::Accion(Accion::Pegar),
            _ => {}
        }
        self.aviso = None;
        Tecla::Sigue
    }

    /// **Un clic en el cajetin**, con su disposicion: activa el campo pinchado,
    /// pulsa la tecla o el boton que haya debajo. Fuera de todo, nada.
    pub fn clic(&mut self, x: f32, y: f32, d: &Disposicion, valido: bool) -> Tecla {
        let dentro = |r: &RectF| x >= r.x && x <= r.x + r.ancho && y >= r.y && y <= r.y + r.alto;
        if dentro(&d.cerrar) || dentro(&d.cancelar) {
            return Tecla::Cancelar;
        }
        if dentro(&d.aceptar) {
            return if valido { Tecla::Aceptar } else { Tecla::Sigue };
        }
        if let Some(i) = d.campos.iter().position(dentro) {
            self.activo = i;
            return Tecla::Sigue;
        }
        if let Some(&(_, fila, col)) = d.teclas.iter().find(|(r, _, _)| dentro(r)) {
            self.teclear(TECLADO[fila][col].1);
            return Tecla::Sigue;
        }
        if let Some(&(_, a)) = d.acciones.iter().find(|(r, _)| dentro(r)) {
            return self.accion(a);
        }
        Tecla::Sigue
    }
}

/// Un numero de un campo, con coma decimal admitida (`numeroDelCampo`).
pub(super) fn numero_del_campo(t: &str) -> Option<f64> {
    t.trim()
        .replace(',', ".")
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite())
}

/// **El cajetin de la grafica**, como el `DialogoDeGrafica` del movil: las
/// formulas con su ejemplo en gris, la ecuacion compuesta debajo, su teclado
/// de formulas, los limites de dos en dos y la escala; con lo de fabrica ya
/// puesto (`sin(x)`, la `x` de -5 a 5, la `y` de -3 a 3 y 40 px la unidad).
pub(super) fn formulario_de_grafica(t: &Catalogo) -> Formulario {
    let mut f = Formulario::nuevo(
        t.t("grafica-titulo"),
        vec![
            Campo::nuevo(t.t("grafica-formulas"), "sin(x)")
                .de_renglones()
                .con_pista("x^2 - 2x · sin(x)/x · x^2 si x<0; 2x si x>=0"),
            Campo::nuevo(t.t("grafica-x-desde"), "-5").a_medias(),
            Campo::nuevo(t.t("grafica-x-hasta"), "5").a_medias(),
            Campo::nuevo(t.t("grafica-y-desde"), "-3").a_medias(),
            Campo::nuevo(t.t("grafica-y-hasta"), "3").a_medias(),
            Campo::nuevo(t.t("grafica-escala"), "40"),
        ],
        t.t("grafica-ayuda"),
        t.t("grafica-insertar"),
        t.t("cajetin-cancelar"),
    );
    f.teclado = true;
    f
}

/// **El cajetin de una tabla** (`EditorDeTablaPegada` del movil): una celda
/// por campo, los botones de fila, columna, cabecera y pegar, e insertar.
pub(super) fn formulario_de_tabla(
    t: &Catalogo,
    filas: &[Vec<String>],
    cabecera: bool,
    titulo: &str,
    aceptar: &str,
) -> Formulario {
    let mut f = Formulario::nuevo(
        t.t(titulo),
        Vec::new(),
        t.t("tabla-ayuda"),
        t.t(aceptar),
        t.t("cajetin-cancelar"),
    );
    f.rejilla = Some(TablaEnCajetin {
        columnas: 1,
        cabecera,
    });
    f.rotulos_de_acciones = [
        "tabla-mas-fila",
        "tabla-menos-fila",
        "tabla-mas-columna",
        "tabla-menos-columna",
        "tabla-cabecera",
        "tabla-pegar",
    ]
    .iter()
    .map(|k| t.t(k))
    .collect();
    if !f.poner_celdas(filas) {
        f.poner_celdas(&vec![vec![String::new(); 3]; 4]);
    }
    f
}

/// La peticion que dice el cajetin, o el aviso de lo que falla.
// `!(b > a)` es a proposito: tambien rechaza los NaN.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub(super) fn peticion_de(f: &Formulario, t: &Catalogo) -> Result<Peticion, String> {
    let formulas: Vec<String> = f.campos[0]
        .texto
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let n = |i: usize| numero_del_campo(&f.campos[i].texto);
    let (Some(a), Some(b), Some(c), Some(d), Some(k)) = (n(1), n(2), n(3), n(4), n(5)) else {
        return Err(t.t("grafica-limites-mal"));
    };
    let p = Peticion {
        formulas,
        x_desde: a,
        x_hasta: b,
        y_desde: c,
        y_hasta: d,
        escala: k,
    };
    match grafica::compilar(&p) {
        Err(Fallo::SinFormula) => Err(t.t("grafica-sin-formula")),
        Err(Fallo::NoSeEntiende(s)) => Err(format!("{} {s}", t.t("grafica-formula-mal"))),
        Err(Fallo::OtraVariable(s)) => Err(format!("{} {s}", t.t("grafica-solo-x"))),
        Err(Fallo::Limites) => Err(t.t("grafica-limites-mal")),
        Ok(_) if !(b > a) || !(d > c) || !(k > 0.0) => Err(t.t("grafica-limites-mal")),
        Ok(_) => Ok(p),
    }
}

/// Lo que se necesita del lienzo para pintar el cajetin encima.
pub(super) struct Lienzo<'a> {
    pub ventana: &'a VentanaOverlay,
    pub motor: &'a mut MotorRender,
    pub superficie: &'a Superficie,
    pub escena: &'a Escena,
    pub camara: &'a Camara,
    pub gesto: &'a Gesto,
    pub cache: &'a mut Cache,
    pub cache_tinta: &'a mut pixpin_render::CacheTinta,
    pub rejilla: &'a Rejilla,
    pub capa: &'a CapaEstatica,
    pub fondo: &'a mut Option<FondoLienzo>,
    pub imagenes: &'a mut ImagenesLienzo,
    pub caja: &'a CajaHerramientas,
    pub corrimiento_ui: (f32, f32),
    pub escala_por_cien: u32,
    pub ancho_px: f32,
    pub alto_px: f32,
}

/// **El cajetin**: su propio bucle de eventos, como el de calibrar
/// (`pedir_medida`), porque tiene que seguir pintando el lienzo con el
/// dialogo encima mientras se teclea. Se maneja con el teclado y con el
/// raton (los campos, las teclas, los botones). `true` si se acepto con lo
/// tecleado valido (`validar` da `Ok`); con Escape o Cancelar, `false`.
pub(super) fn pedir(
    l: Lienzo<'_>,
    f: &mut Formulario,
    validar: impl Fn(&Formulario) -> Result<(), String>,
) -> bool {
    let Lienzo {
        ventana,
        motor,
        superficie,
        escena,
        camara,
        gesto,
        cache,
        cache_tinta,
        rejilla,
        capa,
        fondo,
        imagenes,
        caja,
        corrimiento_ui,
        escala_por_cien,
        ancho_px,
        alto_px,
    } = l;
    ventana.invalidar();
    // Las ecuaciones de la vista previa se miden con un DirectWrite propio:
    // el del motor esta prestado a `pintar` mientras se pinta.
    let sin_imagenes = ImagenesLienzo::nuevo(1);
    let k = escala_por_cien as f32 / 100.0;
    let mut previa = vista_previa(
        f,
        &medidor(motor, pixpin_motor2d::texto::FAMILIA_DEL_SISTEMA),
        escala_por_cien,
    );
    // Lo que hace una tecla o un clic, igual venga de donde venga.
    let atender = |f: &mut Formulario, tecla: Tecla| -> Option<bool> {
        match tecla {
            Tecla::Aceptar => match validar(f) {
                Ok(()) => return Some(true),
                Err(aviso) => f.aviso = Some(aviso),
            },
            Tecla::Cancelar => return Some(false),
            Tecla::Accion(Accion::Pegar) => {
                // «Pegar» de la tabla: lo que haya AHORA en el portapapeles,
                // como el boton del movil, sin cerrar nada.
                let texto = match pixpin_codec::portapapeles::leer() {
                    Some(pixpin_codec::portapapeles::ContenidoPortapapeles::Texto(t)) => t,
                    _ => String::new(),
                };
                let filas =
                    pixpin_motor2d::tabla_dibujada::rejilla_de_texto(&texto).unwrap_or_default();
                if !f.poner_celdas(&filas) {
                    f.aviso = Some(exportar_textos_sin_tabla());
                }
            }
            Tecla::Accion(_) | Tecla::Sigue => {}
        }
        None
    };
    loop {
        pixpin_shell::overlay::bombear_pendientes();
        let mut cambio = false;
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match ev {
                EventoOverlay::Caracter(c) => {
                    let mayus = pixpin_shell::entrada::modificadores_pulsados().shift;
                    let tecla = f.tecla(c, mayus);
                    if let Some(r) = atender(f, tecla) {
                        return r;
                    }
                    cambio = true;
                }
                EventoOverlay::Tecla { vk, ctrl: true, .. } if vk == b'V' as u32 => {
                    if let Some(pixpin_codec::portapapeles::ContenidoPortapapeles::Texto(t)) =
                        pixpin_codec::portapapeles::leer()
                    {
                        f.pegar(&t);
                        cambio = true;
                    }
                }
                EventoOverlay::Tecla {
                    vk, ctrl: false, ..
                } if (0x25..=0x28).contains(&vk) => {
                    f.flecha(vk);
                    cambio = true;
                }
                EventoOverlay::BotonPulsado(p) => {
                    let d = disponer(f, alto_de_la_previa(&previa, k), ancho_px, alto_px, k);
                    let valido = validar(f).is_ok();
                    let tecla = f.clic(p.x as f32, p.y as f32, &d, valido);
                    if let Some(r) = atender(f, tecla) {
                        return r;
                    }
                    cambio = true;
                }
                EventoOverlay::Cerrar => return false,
                EventoOverlay::Pintar => {
                    let valido = validar(f).is_ok();
                    pintar(
                        motor,
                        superficie,
                        escena,
                        camara,
                        gesto,
                        cache,
                        cache_tinta,
                        rejilla,
                        capa,
                        fondo,
                        imagenes,
                        caja,
                        None,
                        corrimiento_ui,
                        true,
                        escala_por_cien,
                        ancho_px,
                        alto_px,
                        None,
                        None,
                        None,
                        true,
                        FueraDeLaEscena::default(),
                        None,
                        |p, base| {
                            dibujar(
                                p,
                                base,
                                ancho_px,
                                alto_px,
                                escala_por_cien,
                                f,
                                &previa,
                                &sin_imagenes,
                                valido,
                            )
                        },
                    );
                }
                _ => {}
            }
        }
        if cambio {
            // Las piezas de la vista previa, medidas antes de pintar (dentro,
            // el motor esta prestado).
            previa = vista_previa(
                f,
                &medidor(motor, pixpin_motor2d::texto::FAMILIA_DEL_SISTEMA),
                escala_por_cien,
            );
            ventana.invalidar();
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// El aviso de «el portapapeles no trae una tabla», en el idioma de la app.
fn exportar_textos_sin_tabla() -> String {
    super::exportar::textos().t("figuras-sin-tabla")
}

/// La ecuacion de cada renglon de formulas, ya compuesta en blanco con su
/// alto; `None` donde el renglon no se entiende. Vacia si no hay teclado de
/// formulas (solo la grafica ensena su ecuacion).
type Previa = Vec<Option<(Vec<Elemento>, f32)>>;

fn vista_previa(f: &Formulario, medir: ecuacion::Medir<'_>, escala_por_cien: u32) -> Previa {
    if !f.teclado {
        return Vec::new();
    }
    let k = escala_por_cien as f32 / 100.0;
    let estilo = Estilo {
        color: ColorRgba::opaco(1.0, 1.0, 1.0),
        tam: 16.0 * k,
        opacidad: 1.0,
        familia: pixpin_motor2d::texto::FAMILIA_DEL_SISTEMA.to_string(),
    };
    f.campos
        .first()
        .map(|c| {
            c.texto
                .lines()
                .filter(|l| !l.trim().is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let c = pixpin_motor2d::formula::compilar(l)?;
            let color = if i == 0 {
                estilo.color
            } else {
                grafica::color_de(
                    grafica::COLORES_DE_CURVAS[(i - 1) % grafica::COLORES_DE_CURVAS.len()],
                )
            };
            let caja = ecuacion::caja_de_la_ecuacion("y", &c, estilo.tam, medir);
            Some((
                caja.elementos(0.0, 0.0, &estilo.con_color(color)),
                caja.alto,
            ))
        })
        .collect()
}

fn alto_de_la_previa(previa: &Previa, k: f32) -> f32 {
    previa
        .iter()
        .map(|x| x.as_ref().map_or(22.0 * k, |(_, h)| h + 6.0 * k))
        .sum()
}

/// **Donde va cada cosa del cajetin**, en pixeles de la ventana: lo usan el
/// pintar y el clic, para que lo que se ve y lo que se pincha no puedan
/// discrepar.
#[derive(Debug, Clone)]
pub(super) struct Disposicion {
    pub tarjeta: RectF,
    pub cerrar: RectF,
    pub titulo: (f32, f32),
    /// La caja de escribir de cada campo, en su orden.
    pub campos: Vec<RectF>,
    /// Donde va la etiqueta de cada campo (ninguna en las celdas).
    pub etiquetas: Vec<Option<(f32, f32)>>,
    /// Donde empieza la ecuacion compuesta, bajo las formulas.
    pub previa: (f32, f32),
    /// Cada tecla del teclado de formulas: su caja y su fila y columna.
    pub teclas: Vec<(RectF, usize, usize)>,
    pub acciones: Vec<(RectF, Accion)>,
    pub aviso: (f32, f32),
    pub ayuda: (f32, f32, f32),
    pub cancelar: RectF,
    pub aceptar: RectF,
}

fn rect(x: f32, y: f32, ancho: f32, alto: f32) -> RectF {
    RectF { x, y, ancho, alto }
}

/// La disposicion del cajetin `f`, centrado en una ventana de `ancho_px` x
/// `alto_px` a la escala `k` (1 = 100 %).
pub(super) fn disponer(
    f: &Formulario,
    alto_previa: f32,
    ancho_px: f32,
    alto_px: f32,
    k: f32,
) -> Disposicion {
    let (m, renglon, sep) = (18.0 * k, 26.0 * k, 10.0 * k);
    let t_etiqueta = 12.0 * k;
    let tecla = (44.0 * k, 30.0 * k, 4.0 * k);
    let ancho_teclado = 6.0 * tecla.0 + 5.0 * tecla.2;
    let columna_campos = 440.0 * k;
    // El teclado va a la derecha de los campos si cabe, y si no debajo.
    let teclado_al_lado =
        f.teclado && ancho_px >= columna_campos + ancho_teclado + 2.0 * m + 16.0 * k + 32.0 * k;
    let ancho = match f.rejilla {
        Some(r) => (2.0 * m + r.columnas as f32 * 130.0 * k)
            .clamp(520.0 * k, (ancho_px - 32.0 * k).max(520.0 * k)),
        None if teclado_al_lado => columna_campos + ancho_teclado + 2.0 * m + 16.0 * k,
        None => 520.0 * k,
    };
    let cero = rect(0.0, 0.0, 0.0, 0.0);
    let mut d = Disposicion {
        tarjeta: cero,
        cerrar: cero,
        titulo: (0.0, 0.0),
        campos: Vec::new(),
        etiquetas: Vec::new(),
        previa: (0.0, 0.0),
        teclas: Vec::new(),
        acciones: Vec::new(),
        aviso: (0.0, 0.0),
        ayuda: (0.0, 0.0, 0.0),
        cancelar: cero,
        aceptar: cero,
    };
    let x0 = m;
    let ancho_campos = if teclado_al_lado {
        columna_campos
    } else {
        ancho - 2.0 * m
    };
    d.titulo = (x0, m);
    d.cerrar = rect(ancho - m - 22.0 * k, m - 2.0 * k, 22.0 * k, 22.0 * k);
    let mut y = m + 34.0 * k;
    let arriba_de_los_campos = y;
    match f.rejilla {
        Some(r) => {
            let cols = r.columnas.max(1);
            let w = (ancho - 2.0 * m) / cols as f32;
            for (i, _) in f.campos.iter().enumerate() {
                let (fila, col) = (i / cols, i % cols);
                d.campos.push(rect(
                    x0 + col as f32 * w,
                    y + fila as f32 * renglon,
                    w,
                    renglon,
                ));
                d.etiquetas.push(None);
            }
            y += (f.campos.len().div_ceil(cols)) as f32 * renglon + sep;
            // Los botones de la tabla, en una fila bajo la rejilla.
            let n = ACCIONES.len() as f32;
            let w = (ancho - 2.0 * m - (n - 1.0) * 6.0 * k) / n;
            for (i, a) in ACCIONES.iter().enumerate() {
                d.acciones
                    .push((rect(x0 + i as f32 * (w + 6.0 * k), y, w, 28.0 * k), *a));
            }
            y += 28.0 * k + sep;
        }
        None => {
            let mut i = 0;
            while i < f.campos.len() {
                let c = &f.campos[i];
                let alto_caja = c.texto.split('\n').count().max(1) as f32 * renglon;
                let pareja = c.media && f.campos.get(i + 1).is_some_and(|s| s.media);
                let ancho_uno = if pareja {
                    (ancho_campos - 10.0 * k) / 2.0
                } else {
                    ancho_campos
                };
                for j in 0..if pareja { 2 } else { 1 } {
                    let x = x0 + j as f32 * (ancho_uno + 10.0 * k);
                    d.etiquetas.push(Some((x, y)));
                    d.campos
                        .push(rect(x, y + t_etiqueta + 5.0 * k, ancho_uno, alto_caja));
                }
                y += t_etiqueta + 5.0 * k + alto_caja + sep;
                if i == 0 && f.teclado {
                    d.previa = (x0, y);
                    y += alto_previa;
                    if !teclado_al_lado {
                        y += sep;
                        let y_teclado = y;
                        for (fila, teclas) in TECLADO.iter().enumerate() {
                            let w = (ancho_campos - (teclas.len() as f32 - 1.0) * tecla.2)
                                / teclas.len() as f32;
                            for col in 0..teclas.len() {
                                let r = rect(
                                    x0 + col as f32 * (w + tecla.2),
                                    y_teclado + fila as f32 * (tecla.1 + tecla.2),
                                    w,
                                    tecla.1,
                                );
                                d.teclas.push((r, fila, col));
                            }
                        }
                        y += TECLADO.len() as f32 * (tecla.1 + tecla.2) + sep;
                    }
                }
                i += if pareja { 2 } else { 1 };
            }
            if teclado_al_lado {
                let xt = x0 + columna_campos + 16.0 * k;
                for (fila, teclas) in TECLADO.iter().enumerate() {
                    let w = (ancho_teclado - (teclas.len() as f32 - 1.0) * tecla.2)
                        / teclas.len() as f32;
                    for col in 0..teclas.len() {
                        let r = rect(
                            xt + col as f32 * (w + tecla.2),
                            arriba_de_los_campos + fila as f32 * (tecla.1 + tecla.2),
                            w,
                            tecla.1,
                        );
                        d.teclas.push((r, fila, col));
                    }
                }
                y = y.max(arriba_de_los_campos + TECLADO.len() as f32 * (tecla.1 + tecla.2) + sep);
            }
        }
    }
    d.aviso = (x0, y);
    if f.aviso.is_some() {
        y += 20.0 * k;
    }
    d.ayuda = (x0, y, ancho - 2.0 * m);
    y += if f.ayuda.is_empty() { 0.0 } else { 34.0 * k };
    let (wb, hb) = (116.0 * k, 34.0 * k);
    d.aceptar = rect(ancho - m - wb, y, wb, hb);
    d.cancelar = rect(ancho - m - 2.0 * wb - 10.0 * k, y, wb, hb);
    y += hb + m;
    d.tarjeta = rect(0.0, 0.0, ancho, y);
    // Centrada en la ventana: todo se corre lo mismo.
    let (dx, dy) = (
        ((ancho_px - ancho) / 2.0).max(0.0),
        ((alto_px - y) / 2.0).max(0.0),
    );
    let correr = |r: &mut RectF| {
        r.x += dx;
        r.y += dy;
    };
    correr(&mut d.tarjeta);
    correr(&mut d.cerrar);
    correr(&mut d.aceptar);
    correr(&mut d.cancelar);
    d.campos.iter_mut().for_each(correr);
    d.teclas.iter_mut().for_each(|(r, _, _)| correr(r));
    d.acciones.iter_mut().for_each(|(r, _)| correr(r));
    for e in d.etiquetas.iter_mut().flatten() {
        (e.0, e.1) = (e.0 + dx, e.1 + dy);
    }
    d.titulo = (d.titulo.0 + dx, d.titulo.1 + dy);
    d.previa = (d.previa.0 + dx, d.previa.1 + dy);
    d.aviso = (d.aviso.0 + dx, d.aviso.1 + dy);
    d.ayuda = (d.ayuda.0 + dx, d.ayuda.1 + dy, d.ayuda.2);
    d
}

const fn gris(v: f32) -> Color {
    Color {
        r: v,
        g: v,
        b: v + 0.02,
        a: 1.0,
    }
}

/// Un rotulo centrado en una caja.
fn centrado(p: &Pintor<'_>, texto: &str, r: RectF, tam: f32, color: Color) {
    let (w, h) = p.medir_texto(texto, tam);
    p.texto(
        texto,
        r.x + (r.ancho - w) / 2.0,
        r.y + (r.alto - h) / 2.0,
        tam,
        color,
    );
}

/// La tarjeta del cajetin, centrada, en coordenadas de pantalla.
#[allow(clippy::too_many_arguments)]
fn dibujar(
    p: &Pintor<'_>,
    base: (f32, f32),
    ancho_px: f32,
    alto_px: f32,
    escala_por_cien: u32,
    f: &Formulario,
    previa: &Previa,
    sin_imagenes: &ImagenesLienzo,
    valido: bool,
) {
    p.desplazar(base.0, base.1);
    let k = escala_por_cien as f32 / 100.0;
    let d = disponer(f, alto_de_la_previa(previa, k), ancho_px, alto_px, k);
    let (t_etiqueta, t_texto) = (12.0 * k, 15.0 * k);
    let azul = Color {
        r: 0.41,
        g: 0.40,
        b: 0.84,
        a: 1.0,
    };
    let rojo = Color {
        r: 1.0,
        g: 0.45,
        b: 0.45,
        a: 1.0,
    };
    let apagado = gris(0.72);
    // La sombra y la tarjeta.
    let sombra = RectF {
        x: d.tarjeta.x + 3.0 * k,
        y: d.tarjeta.y + 5.0 * k,
        ..d.tarjeta
    };
    p.rellenar_redondeado(
        sombra,
        12.0 * k,
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.28,
        },
    );
    p.rellenar_redondeado(
        d.tarjeta,
        12.0 * k,
        Color {
            r: 0.13,
            g: 0.13,
            b: 0.15,
            a: 0.98,
        },
    );
    p.texto(&f.titulo, d.titulo.0, d.titulo.1, 17.0 * k, Color::BLANCO);
    centrado(p, "✕", d.cerrar, 14.0 * k, apagado);
    let cabecera = f.rejilla.is_some_and(|r| r.cabecera);
    let cols = f.rejilla.map_or(1, |r| r.columnas.max(1));
    for (i, c) in f.campos.iter().enumerate() {
        let r = d.campos[i];
        if let Some(Some((x, y))) = d.etiquetas.get(i) {
            p.texto(&c.etiqueta, *x, *y, t_etiqueta, apagado);
        }
        let celda = f.rejilla.is_some();
        let fondo = if celda && cabecera && i < cols {
            gris(0.27)
        } else {
            gris(0.2)
        };
        if celda {
            // Las celdas pegadas, como una hoja: un borde fino las separa.
            p.rellenar(r, fondo);
            p.trazar(r, 1.0, gris(0.34));
        } else {
            p.rellenar_redondeado(r, 5.0 * k, fondo);
        }
        if i == f.activo {
            p.trazar(r, 1.5 * k, azul);
        }
        let renglones: Vec<&str> = c.texto.split('\n').collect();
        let alto_renglon = r.alto / renglones.len().max(1) as f32;
        let aire = if celda { 5.0 * k } else { 8.0 * k };
        if c.texto.is_empty() && !c.pista.is_empty() {
            p.texto_linea(
                &c.pista,
                r.x + aire,
                r.y + (alto_renglon - t_texto * 1.3) / 2.0,
                t_texto,
                r.ancho - 2.0 * aire,
                gris(0.5),
            );
        }
        for (n, renglon) in renglones.iter().enumerate() {
            let s = if i == f.activo && n + 1 == renglones.len() {
                format!("{renglon}|")
            } else {
                renglon.to_string()
            };
            let y = r.y + n as f32 * alto_renglon + (alto_renglon - t_texto * 1.3) / 2.0;
            p.texto_linea(
                &s,
                r.x + aire,
                y,
                t_texto,
                r.ancho - 2.0 * aire,
                Color::BLANCO,
            );
        }
    }
    // La ecuacion como va a salir, una por curva y de su color.
    let (x, mut y) = d.previa;
    for pr in previa {
        match pr {
            Some((elementos, h)) => {
                let vista = (x, y, x + d.tarjeta.ancho, y + h);
                for e in elementos {
                    let mut e = e.clone();
                    e.mover(x, y);
                    for o in pixpin_motor2d::pintado::ordenes(&e) {
                        dibujar_orden(p, &o, vista, None, sin_imagenes, 1.0, None);
                    }
                }
                y += h + 6.0 * k;
            }
            None => {
                p.texto("?", x, y, t_texto, rojo);
                y += 22.0 * k;
            }
        }
    }
    for &(r, fila, col) in &d.teclas {
        let rotulo = TECLADO[fila][col].0;
        let tono = if tecla_fuerte(rotulo) {
            Color {
                r: 0.25,
                g: 0.25,
                b: 0.42,
                a: 1.0,
            }
        } else {
            gris(0.23)
        };
        p.rellenar_redondeado(r, 6.0 * k, tono);
        centrado(p, rotulo, r, 13.0 * k, Color::BLANCO);
    }
    for (i, &(r, a)) in d.acciones.iter().enumerate() {
        let encendido = a == Accion::Cabecera && cabecera;
        p.rellenar_redondeado(
            r,
            6.0 * k,
            if encendido {
                Color {
                    r: 0.25,
                    g: 0.25,
                    b: 0.42,
                    a: 1.0,
                }
            } else {
                gris(0.23)
            },
        );
        let rotulo = f
            .rotulos_de_acciones
            .get(i)
            .map(String::as_str)
            .unwrap_or("");
        let rotulo = if a == Accion::Cabecera {
            format!("{} {rotulo}", if cabecera { "☑" } else { "☐" })
        } else {
            rotulo.to_string()
        };
        centrado(p, &rotulo, r, 12.5 * k, Color::BLANCO);
    }
    if let Some(a) = &f.aviso {
        p.texto(a, d.aviso.0, d.aviso.1, 13.0 * k, rojo);
    }
    if !f.ayuda.is_empty() {
        p.texto_ajustado(&f.ayuda, d.ayuda.0, d.ayuda.1, 11.5 * k, d.ayuda.2, apagado);
    }
    p.rellenar_redondeado(d.cancelar, 7.0 * k, gris(0.23));
    centrado(p, &f.cancelar, d.cancelar, 14.0 * k, Color::BLANCO);
    // Aceptar no se enciende hasta que todo se entiende (el `enabled` del
    // boton del movil).
    p.rellenar_redondeado(d.aceptar, 7.0 * k, if valido { azul } else { gris(0.3) });
    centrado(
        p,
        &f.aceptar,
        d.aceptar,
        14.0 * k,
        if valido { Color::BLANCO } else { gris(0.55) },
    );
}

// ---------------------------------------------------------------------------
// La foto de la Zona
// ---------------------------------------------------------------------------

/// **La foto de la zona recortada en redondo** (F8), lista para el almacen:
/// el papel y lo dibujado dentro de `caja`, a la escala de `zona.rs`, con las
/// esquinas redondas y el filo que se lee sobre ese papel.
pub(super) fn foto_de_la_zona(
    lienzo: &super::exportar::Lienzo<'_>,
    caja: (f32, f32, f32, f32),
) -> Option<pixpin_codec::ImagenRgba> {
    let mut img = foto_cruda_de_la_zona(lienzo, caja)?;
    // Sobre una pagina de PDF o una foto (papel), el filo va negro: la hoja
    // es clara aunque el lienzo sea de noche. Como el movil.
    let de_noche = lienzo.papel.is_none() && crate::dibujo::tema::es_de_noche(lienzo.escena.fondo);
    pixpin_motor2d::zona::en_redondo_y_con_filo(
        &mut img.pixeles,
        img.ancho,
        img.alto,
        pixpin_motor2d::zona::color_del_filo(de_noche),
    );
    Some(img)
}

/// **La foto de la zona tal cual**, sin redondear: es la que va al chat
/// (`mandarLaZona` manda la foto de `fotoDeLaZona`; solo la copia que se
/// queda en el lienzo se recorta en redondo).
pub(super) fn foto_cruda_de_la_zona(
    lienzo: &super::exportar::Lienzo<'_>,
    caja: (f32, f32, f32, f32),
) -> Option<pixpin_codec::ImagenRgba> {
    // Lo pixelado sale pixelado en la foto tambien (`exportar::tapado`): lo
    // tapado no puede viajar en claro al chat por la zona.
    if let Some(t) = super::exportar::tapado::tapar_dentro(lienzo, Some(caja)) {
        let fotos = |id: u64| t.imagenes.get(&id).or_else(|| (lienzo.fotos)(id));
        let tapado = super::exportar::Lienzo {
            escena: &t.escena,
            seleccion: lienzo.seleccion,
            papel: lienzo.papel,
            fotos: &fotos,
            nombre: lienzo.nombre.clone(),
        };
        return foto_sin_mosaicos(&tapado, caja);
    }
    foto_sin_mosaicos(lienzo, caja)
}

fn foto_sin_mosaicos(
    lienzo: &super::exportar::Lienzo<'_>,
    caja: (f32, f32, f32, f32),
) -> Option<pixpin_codec::ImagenRgba> {
    let papel = lienzo.papel.map(|(_, w, h)| (w, h));
    // **La foto es lo que se ve**: sobre papel de noche la tinta se pinta
    // adaptada (`dibujo::tema`), y la foto tiene que salir igual —el movil la
    // saca con `Renderer(dark = esDeNoche(fondo))`—. Exportar no adapta (lo
    // que se exporta es lo que se eligio), asi que se adapta una copia de lo
    // dibujado, solo para la foto.
    let adaptada;
    let escena = if lienzo.papel.is_none() && crate::dibujo::tema::es_de_noche(lienzo.escena.fondo)
    {
        let fondo = lienzo.escena.fondo;
        let mut e = Escena::nueva();
        e.fondo = fondo;
        e.escala = lienzo.escena.escala.clone();
        e.elementos = lienzo
            .escena
            .elementos
            .iter()
            .map(|x| {
                let mut x = x.clone();
                x.trazo = crate::dibujo::tema::adaptar(x.trazo, fondo);
                x.relleno = x.relleno.map(|c| crate::dibujo::tema::adaptar(c, fondo));
                x
            })
            .collect();
        adaptada = e;
        &adaptada
    } else {
        lienzo.escena
    };
    let hoja = pixpin_motor2d::exportar::de_una_zona(escena, caja, papel)?;
    let escala = pixpin_motor2d::zona::escala_de_la_foto(caja);
    super::exportar::a_imagen(&hoja, escala, Some(lienzo.escena.fondo), lienzo)
        .map_err(|e| tracing::warn!(?e, "la foto de la zona no salio"))
        .ok()
}

/// Pone la foto de la zona encima, corrida `CORRIMIENTO_PX` de pantalla y
/// elegida, con su grupo de zona (`ponerCopiaDeZona` del movil). La
/// herramienta no cambia: se puede sacar otra o arrastrar esta.
pub(super) fn poner_copia_de_zona(
    escena: &mut Escena,
    gesto: &mut Gesto,
    imagenes: &mut ImagenesLienzo,
    foto: pixpin_codec::ImagenRgba,
    caja: (f32, f32, f32, f32),
    zoom: f32,
) -> bool {
    let Some(id_objeto) = imagenes.guardar(foto) else {
        return false;
    };
    let corrido = pixpin_motor2d::zona::CORRIMIENTO_PX / zoom.max(0.0001);
    let mut e = crate::imagenes_lienzo::elemento_imagen(
        id_objeto,
        caja.0 + corrido,
        caja.1 + corrido,
        caja.2 - caja.0,
        caja.3 - caja.1,
    );
    e.grupos.push(pixpin_motor2d::zona::grupo_nuevo(escena));
    escena.abrir_paso();
    let id = escena.anadir(e);
    escena.cerrar_paso();
    gesto.seleccion.limpiar();
    gesto.seleccion.poner(id);
    true
}

// ---------------------------------------------------------------------------
// Lo que hace el editor con los botones
// ---------------------------------------------------------------------------

/// El editor entero, prestado mientras se atiende un boton de la barra.
pub(super) struct Editor<'a> {
    pub ventana: &'a VentanaOverlay,
    pub motor: &'a mut MotorRender,
    pub superficie: &'a Superficie,
    pub escena: &'a mut Escena,
    pub camara: &'a Camara,
    pub gesto: &'a mut Gesto,
    pub cache: &'a mut Cache,
    pub cache_tinta: &'a mut pixpin_render::CacheTinta,
    pub rejilla: &'a mut Rejilla,
    pub capa: &'a CapaEstatica,
    pub fondo: &'a mut Option<FondoLienzo>,
    pub imagenes: &'a mut ImagenesLienzo,
    pub caja: &'a CajaHerramientas,
    pub corrimiento_ui: (f32, f32),
    pub escala_por_cien: u32,
    pub ancho_px: f32,
    pub alto_px: f32,
}

impl Editor<'_> {
    fn lienzo(&mut self) -> Lienzo<'_> {
        // El cajetin pinta la escena de debajo: la rejilla tiene que estar al
        // dia con lo ultimo que se dibujo.
        self.rejilla.sincronizar(self.escena);
        Lienzo {
            ventana: self.ventana,
            motor: self.motor,
            superficie: self.superficie,
            escena: self.escena,
            camara: self.camara,
            gesto: self.gesto,
            cache: self.cache,
            cache_tinta: self.cache_tinta,
            rejilla: self.rejilla,
            capa: self.capa,
            fondo: self.fondo,
            imagenes: self.imagenes,
            caja: self.caja,
            corrimiento_ui: self.corrimiento_ui,
            escala_por_cien: self.escala_por_cien,
            ancho_px: self.ancho_px,
            alto_px: self.alto_px,
        }
    }

    fn centro(&self) -> Punto2 {
        centro_de_la_vista(self.camara, self.ancho_px, self.alto_px)
    }
}

/// **El menu de las figuras y lo que se elija en el.** `true` si cambio el
/// dibujo.
pub(super) fn atender_figuras(mut ed: Editor<'_>, en: pixpin_geom::Punto, t: &Catalogo) -> bool {
    let ruta = ruta_biblioteca();
    let mut biblio = cargar_biblioteca(&ruta);
    let hay_seleccion = !ed.gesto.seleccion.esta_vacia();
    let tabla = TablaElegida::de(ed.escena, ed.gesto);
    let Some(eleccion) = menu(
        ed.ventana.handle(),
        en.x,
        en.y,
        t,
        &biblio,
        hay_seleccion,
        tabla,
    ) else {
        return false;
    };
    let centro = ed.centro();
    match eleccion {
        Eleccion::Grafica => {
            let mut f = formulario_de_grafica(t);
            if !pedir(ed.lienzo(), &mut f, |f| peticion_de(f, t).map(|_| ())) {
                return false;
            }
            let Ok(p) = peticion_de(&f, t) else {
                return false;
            };
            let estilo = estilo_del_pincel(ed.gesto);
            let medir = medidor(ed.motor, &estilo.familia);
            let Ok(v) = grafica::elementos(&p, &estilo, &medir) else {
                return false;
            };
            estampar_en_la_vista(ed.escena, ed.gesto, centro, "grafica", &v)
        }
        // La tabla en blanco y la pegada pasan por su cajetin antes de
        // entrar, como en el movil (`EditorDeTablaPegada`): lo que llega casi
        // nunca es justo lo que se quiere dibujar.
        Eleccion::TablaEnBlanco => {
            let mut f = formulario_de_tabla(
                t,
                &vec![vec![String::new(); 3]; 4],
                true,
                "tabla-blanco-titulo",
                "tabla-insertar",
            );
            if !pedir(ed.lienzo(), &mut f, |_| Ok(())) {
                return false;
            }
            let estilo = estilo_del_pincel(ed.gesto);
            let v = tabla_de_celdas(&f, &estilo, &medidor(ed.motor, &estilo.familia));
            estampar_en_la_vista(ed.escena, ed.gesto, centro, "tabla", &v)
        }
        Eleccion::PegarTabla => {
            let (html, texto) = pixpin_codec::portapapeles::tabla::leer_tabla().unwrap_or_default();
            let texto = texto.unwrap_or_default();
            let pegadas =
                pixpin_motor2d::tabla_dibujada::rejilla_de_texto(&texto).unwrap_or_default();
            let mut f =
                formulario_de_tabla(t, &pegadas, true, "tabla-pegada-titulo", "tabla-insertar");
            if pegadas.is_empty() {
                f.aviso = Some(t.t("figuras-sin-tabla"));
            }
            if !pedir(ed.lienzo(), &mut f, |_| Ok(())) {
                return false;
            }
            let estilo = estilo_del_pincel(ed.gesto);
            let medir = medidor(ed.motor, &estilo.familia);
            // Sin tocar nada, manda lo del portapapeles tal cual: con su HTML
            // trae las celdas combinadas y la negrita, que el cajetin no sabe
            // ensenar.
            if !pegadas.is_empty() && f.celdas() == pegadas && f.rejilla.is_some_and(|r| r.cabecera)
            {
                return pegar_tabla(html.as_deref(), &texto, ed.escena, ed.gesto, centro, &medir);
            }
            let v = tabla_de_celdas(&f, &estilo, &medir);
            estampar_en_la_vista(ed.escena, ed.gesto, centro, "tabla", &v)
        }
        Eleccion::EditarTabla => editar_tabla(ed, t),
        Eleccion::MeterEnCelda => {
            let ids = ed.gesto.seleccion.ids().to_vec();
            pixpin_motor2d::tabla_dibujada::leer::meter_en_celdas(ed.escena, &ids)
        }
        Eleccion::GuardarSeleccion => {
            let mut f = Formulario::nuevo(
                t.t("figuras-nombre-titulo"),
                vec![Campo::nuevo(t.t("figuras-nombre"), "")],
                t.t("figuras-nombre-ayuda"),
                t.t("figuras-guardar"),
                t.t("cajetin-cancelar"),
            );
            if !pedir(ed.lienzo(), &mut f, |_| Ok(())) {
                return false;
            }
            let nombre = f.campos[0].texto.trim();
            let nombre = if nombre.is_empty() { "Figura" } else { nombre };
            if let Some(nueva) =
                biblioteca::de_la_seleccion(&ed.escena.elementos, ed.gesto.seleccion.ids(), nombre)
            {
                // Las nuevas, arriba: lo ultimo que se guarda es lo que se usa.
                biblio.insert(0, nueva);
                guardar_biblioteca(&ruta, &biblio);
            }
            false
        }
        Eleccion::Estampar(i) => match biblio.get(i) {
            Some(fig) => {
                let ids = biblioteca::estampar_en_escena(ed.escena, fig, centro);
                let hubo = !ids.is_empty();
                ed.gesto.seleccion.poner_todos(ids);
                hubo
            }
            None => false,
        },
        Eleccion::Quitar(i) => {
            if i < biblio.len() {
                biblio.remove(i);
                guardar_biblioteca(&ruta, &biblio);
            }
            false
        }
    }
}

/// La tabla que dice el cajetin, dibujada con la letra del pincel. Con las
/// celdas en blanco tambien: una tabla en blanco es una tabla donde escribir.
pub(super) fn tabla_de_celdas(
    f: &Formulario,
    estilo: &Estilo,
    medir: ecuacion::Medir<'_>,
) -> Vec<Elemento> {
    use pixpin_motor2d::tabla_dibujada::{Celda, elementos_de_tabla_con_juntas};
    let celdas: Vec<Vec<Celda>> = f
        .celdas()
        .into_iter()
        .map(|fila| fila.into_iter().map(Celda::de).collect())
        .collect();
    let cabecera = f.rejilla.is_some_and(|r| r.cabecera);
    elementos_de_tabla_con_juntas(&celdas, estilo, Punto2::nuevo(0.0, 0.0), medir, cabecera)
}

/// **La tabla elegida, a su cajetin y otra vez al lienzo** (Intro con una
/// tabla elegida, o «Editar la tabla» del menu de figuras): se lee de su
/// dibujo, se editan sus celdas, filas y columnas, y se vuelve a dibujar en
/// el mismo sitio y grupo, con sus figuras encajadas en su celda. `true` si
/// cambio el dibujo.
pub(super) fn editar_tabla(mut ed: Editor<'_>, t: &Catalogo) -> bool {
    use pixpin_motor2d::tabla_dibujada::{Celda, leer};
    let elegidos: Vec<&Elemento> = ed
        .escena
        .visibles()
        .filter(|e| ed.gesto.seleccion.contiene(e.id))
        .collect();
    let Some(vieja) = leer::leer_tabla(&elegidos) else {
        return false;
    };
    let textos: Vec<Vec<String>> = vieja
        .celdas
        .iter()
        .map(|f| f.iter().map(|c| c.texto.clone()).collect())
        .collect();
    let mut f = formulario_de_tabla(
        t,
        &textos,
        vieja.cabecera,
        "tabla-editar-titulo",
        "tabla-aplicar",
    );
    if !pedir(ed.lienzo(), &mut f, |_| Ok(())) {
        return false;
    }
    let cabecera = f.rejilla.is_some_and(|r| r.cabecera);
    if f.celdas() == textos && cabecera == vieja.cabecera {
        return false;
    }
    // Con la letra que ya tenia la tabla, no con la del pincel de ahora.
    let mut estilo = estilo_del_pincel(ed.gesto);
    if let Some(tam) = vieja.tam {
        estilo.tam = tam;
    }
    if let Some(familia) = &vieja.familia {
        estilo.familia = familia.clone();
    }
    if let Some(marco) = vieja
        .de_la_tabla
        .first()
        .and_then(|id| ed.escena.buscar(*id))
    {
        estilo.color = marco.trazo;
        estilo.opacidad = marco.opacidad;
    }
    // La negrita de cada celda se queda con su celda si sigue existiendo.
    let celdas: Vec<Vec<Celda>> = f
        .celdas()
        .into_iter()
        .enumerate()
        .map(|(i, fila)| {
            fila.into_iter()
                .enumerate()
                .map(|(j, texto)| Celda {
                    negrita: vieja
                        .celdas
                        .get(i)
                        .and_then(|f| f.get(j))
                        .is_some_and(|c| c.negrita),
                    ..Celda::de(texto)
                })
                .collect()
        })
        .collect();
    let medir = medidor(ed.motor, &estilo.familia);
    let ids = leer::rehacer_en_escena(ed.escena, &vieja, &celdas, cabecera, &estilo, &medir);
    if ids.is_empty() {
        return false;
    }
    ed.gesto.seleccion.limpiar();
    ed.gesto.seleccion.poner_todos(ids);
    true
}

/// El cajetin de un cronograma: cuantas filas, cuantas columnas y el nombre
/// de cada fila (`AjustesDelCronograma` del movil: «un campo por fila y no un
/// dialogo por nombre: con tres o cuatro tareas, abrir y cerrar una ventana
/// por cada una cuesta mas que teclearlos seguidos»). Filas y columnas de dos
/// en dos, como los limites de la grafica.
pub(super) fn formulario_de_cronograma(
    t: &Catalogo,
    tareas: &[pixpin_motor2d::cronograma::Tarea],
    periodos: u32,
) -> Formulario {
    let mut campos = vec![
        Campo::nuevo(t.t("cronograma-filas"), tareas.len().to_string()).a_medias(),
        Campo::nuevo(t.t("cronograma-columnas"), periodos.to_string()).a_medias(),
    ];
    for (i, tarea) in tareas.iter().enumerate() {
        campos.push(Campo::nuevo(
            format!("{} {}", t.t("cronograma-fila"), i + 1),
            tarea.nombre.clone(),
        ));
    }
    let mut f = Formulario::nuevo(
        t.t("cronograma-titulo"),
        campos,
        t.t("cronograma-ayuda"),
        t.t("tabla-aplicar"),
        t.t("cajetin-cancelar"),
    );
    f.activo = 2.min(tareas.len() + 1);
    f
}

/// Lo que dice el cajetin de un cronograma: filas, columnas y nombres. `Err`
/// con el aviso si las cuentas no son numeros de verdad.
pub(super) fn cronograma_de(
    f: &Formulario,
    t: &Catalogo,
) -> Result<(usize, u32, Vec<String>), String> {
    let entero = |i: usize| {
        f.campos
            .get(i)
            .and_then(|c| c.texto.trim().parse::<i64>().ok())
    };
    let (Some(filas), Some(columnas)) = (entero(0), entero(1)) else {
        return Err(t.t("cronograma-cuentas-mal"));
    };
    if !(0..=60).contains(&filas)
        || !(1..=pixpin_motor2d::cronograma::MAXIMO_DE_PERIODOS as i64).contains(&columnas)
    {
        return Err(t.t("cronograma-cuentas-mal"));
    }
    let nombres = f.campos[2..]
        .iter()
        .map(|c| c.texto.trim().to_string())
        .collect();
    Ok((filas as usize, columnas as u32, nombres))
}

/// Aplica lo del cajetin al cronograma `id`, en un paso de deshacer: los
/// nombres, luego las filas que sobran o faltan (las nuevas, detras de la
/// ultima, como el «+» del movil) y las columnas. `true` si cambio algo.
pub(super) fn aplicar_cronograma(
    escena: &mut Escena,
    id: u64,
    filas: usize,
    columnas: u32,
    nombres: &[String],
) -> bool {
    let Some(antes) = escena.buscar(id).cloned() else {
        return false;
    };
    escena.abrir_paso();
    escena.apuntar_edicion(id);
    if let Some(e) = escena.buscar_mut(id) {
        if let pixpin_motor2d::Figura::Cronograma { tareas, .. } = &mut e.figura {
            for (t, n) in tareas.iter_mut().zip(nombres) {
                t.nombre = n.clone();
            }
        }
        while let pixpin_motor2d::Figura::Cronograma { tareas, .. } = &e.figura {
            let n = tareas.len();
            if n < filas {
                pixpin_motor2d::cronograma::con_tarea_nueva(e, "");
            } else if n > filas {
                pixpin_motor2d::cronograma::sin_la_ultima_tarea(e);
            } else {
                break;
            }
        }
        pixpin_motor2d::cronograma::con_periodos(e, columnas as i64);
        // Un nombre que no cabe ensancha la figura hacia la derecha en vez
        // de salir recortado («Cimien…»): se acaba de teclear para leerlo.
        if let Some(ancho) = pixpin_motor2d::cronograma::ancho_para_los_nombres(e) {
            e.ancho = ancho;
        }
        e.tocar();
    }
    escena.cerrar_paso();
    escena.buscar(id).is_some_and(|e| e.figura != antes.figura)
}

/// **Enter con un cronograma elegido**: su cajetin (F12).
pub(super) fn editar_cronograma(mut ed: Editor<'_>, id: u64, t: &Catalogo) -> bool {
    let Some(pixpin_motor2d::Figura::Cronograma { tareas, periodos }) =
        ed.escena.buscar(id).map(|e| e.figura.clone())
    else {
        return false;
    };
    let mut f = formulario_de_cronograma(t, &tareas, periodos);
    if !pedir(ed.lienzo(), &mut f, |f| cronograma_de(f, t).map(|_| ())) {
        return false;
    }
    let Ok((filas, columnas, nombres)) = cronograma_de(&f, t) else {
        return false;
    };
    aplicar_cronograma(ed.escena, id, filas, columnas, &nombres)
}

/// **Meter una imagen desde un fichero** (el boton de imagen): el selector
/// de Windows, y la foto en el medio de la vista con el tamano con que se
/// pega (`imagenes_lienzo::tamano_al_pegar`), elegida.
pub(super) fn meter_imagen(ed: Editor<'_>) -> bool {
    let Some(ruta) = pixpin_shell::elegir::pedir_imagenes(ed.ventana.handle())
        .into_iter()
        .next()
    else {
        return false;
    };
    meter_imagen_de(
        &ruta,
        ed.imagenes,
        ed.escena,
        ed.gesto,
        ed.camara,
        ed.ancho_px,
        ed.alto_px,
    )
}

/// La foto de `ruta` en el medio de la vista, elegida: lo del boton de
/// imagen y lo que llega de otro aparato al lienzo abierto (`al_frente`).
pub(super) fn meter_imagen_de(
    ruta: &std::path::Path,
    imagenes: &mut ImagenesLienzo,
    escena: &mut Escena,
    gesto: &mut Gesto,
    camara: &Camara,
    ancho_px: f32,
    alto_px: f32,
) -> bool {
    let Ok(img) = pixpin_codec::cargar(ruta) else {
        tracing::warn!(ruta = %ruta.display(), "imagen que no se pudo abrir");
        return false;
    };
    let Some(id_objeto) = imagenes.guardar(img) else {
        return false;
    };
    let Some((w, h)) = imagenes.tamano(id_objeto) else {
        return false;
    };
    let v = camara.ventana(ancho_px, alto_px);
    let (ancho, alto) = crate::imagenes_lienzo::tamano_al_pegar(w, h, v.2 - v.0, v.3 - v.1);
    let (x, y) = crate::imagenes_lienzo::esquina_centrada(v, ancho, alto);
    escena.abrir_paso();
    let id = escena.anadir(crate::imagenes_lienzo::elemento_imagen(
        id_objeto, x, y, ancho, alto,
    ));
    escena.cerrar_paso();
    gesto.seleccion.limpiar();
    gesto.seleccion.poner(id);
    true
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn medir(t: &str, tam: f32) -> (f32, f32) {
        (t.chars().count() as f32 * tam * 0.5, tam * 1.2)
    }

    fn catalogo() -> Catalogo {
        Catalogo::nuevo(pixpin_store::Idioma::Espanol)
    }

    #[test]
    fn el_menu_ofrece_las_de_fabrica_y_las_propias_y_guardar_solo_con_algo_elegido() {
        let t = catalogo();
        let propia = FiguraGuardada {
            nombre: "Norte".into(),
            elementos: vec![Elemento::default()],
        };
        let sin = entradas(&t, &[], false, TablaElegida::Ninguna);
        assert_eq!(sin.iter().filter(|e| e.id != 0).count(), 3);
        assert!(
            sin.iter().all(|e| e.id != ID_GUARDAR),
            "sin seleccion no se guarda"
        );
        assert!(
            sin.last().is_some_and(|e| e.id != 0),
            "sin separador colgando"
        );
        let con = entradas(
            &t,
            std::slice::from_ref(&propia),
            true,
            TablaElegida::Ninguna,
        );
        assert!(con.iter().any(|e| e.id == ID_GUARDAR));
        assert!(con.iter().any(|e| e.id == ID_FIGURA && e.texto == "Norte"));
        assert!(
            con.iter()
                .any(|e| e.id == ID_QUITAR && e.texto.contains("Norte"))
        );
        assert_eq!(eleccion_de(ID_FIGURA, 1), Some(Eleccion::Estampar(0)));
        assert_eq!(eleccion_de(ID_QUITAR, 1), Some(Eleccion::Quitar(0)));
        // Caso negativo: una figura que ya no esta no se elige.
        assert_eq!(eleccion_de(ID_FIGURA + 1, 1), None);
        assert_eq!(eleccion_de(ID_GRAFICA, 0), Some(Eleccion::Grafica));
    }

    #[test]
    fn el_cajetin_acepta_con_intro_y_abre_otra_curva_con_mayus_intro() {
        let t = catalogo();
        let mut f = formulario_de_grafica(&t);
        assert_eq!(f.campos[0].texto, "sin(x)");
        assert_eq!(f.tecla('\r', true), Tecla::Sigue);
        for c in "x^2".chars() {
            f.tecla(c, false);
        }
        assert_eq!(f.campos[0].texto, "sin(x)\nx^2");
        // Tab al campo siguiente; en uno de un renglon, Mayus+Intro acepta.
        f.tecla('\t', false);
        assert_eq!(f.activo, 1);
        f.tecla('\t', true);
        assert_eq!(f.activo, 0);
        assert_eq!(f.tecla('\r', false), Tecla::Aceptar);
        assert_eq!(f.tecla('\u{1b}', false), Tecla::Cancelar);
        let p = peticion_de(&f, &t).unwrap();
        assert_eq!(p.formulas, vec!["sin(x)".to_string(), "x^2".into()]);
        assert_eq!(
            (p.x_desde, p.x_hasta, p.y_desde, p.y_hasta, p.escala),
            (-5.0, 5.0, -3.0, 3.0, 40.0)
        );
    }

    #[test]
    fn el_cajetin_dice_que_falla_y_no_deja_insertar() {
        let t = catalogo();
        let mut f = formulario_de_grafica(&t);
        f.campos[0].texto = "foo(x)".into();
        assert!(peticion_de(&f, &t).unwrap_err().contains("foo(x)"));
        f.campos[0].texto = "x + y".into();
        assert!(peticion_de(&f, &t).is_err());
        f.campos[0].texto = "x".into();
        f.campos[1].texto = "5".into();
        f.campos[2].texto = "-5".into();
        assert!(peticion_de(&f, &t).is_err(), "limites del reves");
        f.campos[1].texto = "-2,5".into();
        f.campos[2].texto = "2,5".into();
        assert_eq!(peticion_de(&f, &t).unwrap().x_desde, -2.5, "coma decimal");
    }

    #[test]
    fn pegar_en_un_campo_de_un_renglon_quita_los_saltos() {
        let mut f = formulario_de_grafica(&catalogo());
        f.activo = 1;
        f.campos[1].texto.clear();
        f.pegar("-10\r\n");
        assert_eq!(f.campos[1].texto, "-10");
        f.activo = 0;
        f.campos[0].texto.clear();
        f.pegar("x\r\n-x");
        assert_eq!(f.campos[0].texto, "x\n-x");
    }

    #[test]
    fn una_tabla_de_excel_pegada_sale_agrupada_elegida_y_centrada_en_la_vista() {
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::nuevo();
        let centro = Punto2::nuevo(500.0, 300.0);
        assert!(pegar_tabla(
            None,
            "a\tb\n1\t2\n",
            &mut escena,
            &mut gesto,
            centro,
            &medir
        ));
        let vivos: Vec<_> = escena.visibles().collect();
        assert!(!vivos.is_empty());
        let grupo = &vivos[0].grupos;
        assert_eq!(grupo.len(), 1);
        assert!(vivos.iter().all(|e| &e.grupos == grupo), "una sola pieza");
        assert_eq!(gesto.seleccion.ids().len(), vivos.len());
        let caja = biblioteca::caja_de(&escena.elementos).unwrap();
        assert!(((caja.0 + caja.2) / 2.0 - 500.0).abs() < 1.0);
        // Un paso de deshacer.
        escena.deshacer();
        assert_eq!(escena.visibles().count(), 0);
        // Caso negativo: un texto corriente no pega nada.
        assert!(!pegar_tabla(
            None,
            "hola",
            &mut escena,
            &mut gesto,
            centro,
            &medir
        ));
    }

    #[test]
    fn con_una_tabla_elegida_el_menu_ofrece_editarla_y_meter_en_la_celda_solo_si_hay_algo_encima() {
        let t = catalogo();
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::nuevo();
        assert_eq!(TablaElegida::de(&escena, &gesto), TablaElegida::Ninguna);
        assert!(pegar_tabla(
            None,
            "a\tb\n1\t2\n",
            &mut escena,
            &mut gesto,
            Punto2::nuevo(0.0, 0.0),
            &medir
        ));
        let tabla = TablaElegida::de(&escena, &gesto);
        assert_eq!(tabla, TablaElegida::Sola);
        let v = entradas(&t, &[], true, tabla);
        assert!(v.iter().any(|e| e.id == ID_EDITAR_TABLA));
        assert!(
            v.iter().all(|e| e.id != ID_METER),
            "sin nada encima no se ofrece meter"
        );
        assert_eq!(eleccion_de(ID_EDITAR_TABLA, 0), Some(Eleccion::EditarTabla));
        // Algo encima de una celda y elegido con ella: ahora si.
        let (x0, y0, x1, y1) = biblioteca::caja_de(&escena.elementos).unwrap();
        let punto = escena.anadir(Elemento {
            figura: pixpin_motor2d::Figura::Elipse,
            x: x0 + (x1 - x0) * 0.75 - 2.0,
            y: y0 + (y1 - y0) * 0.75 - 2.0,
            ancho: 4.0,
            alto: 4.0,
            ..Default::default()
        });
        let mut ids = gesto.seleccion.ids().to_vec();
        ids.push(punto);
        gesto.seleccion.poner_todos(ids);
        let tabla = TablaElegida::de(&escena, &gesto);
        assert_eq!(tabla, TablaElegida::ConFiguras);
        assert!(
            entradas(&t, &[], true, tabla)
                .iter()
                .any(|e| e.id == ID_METER)
        );
        assert_eq!(eleccion_de(ID_METER, 0), Some(Eleccion::MeterEnCelda));
    }

    #[test]
    fn la_tabla_en_blanco_del_cajetin_es_rejilla_sin_textos() {
        let f = formulario_de_tabla(
            &catalogo(),
            &vec![vec![String::new(); 3]; 4],
            true,
            "tabla-blanco-titulo",
            "tabla-insertar",
        );
        let e = tabla_de_celdas(&f, &estilo_del_pincel(&Gesto::nuevo()), &medir);
        assert!(
            e.iter()
                .all(|x| !matches!(x.figura, pixpin_motor2d::Figura::Texto { .. }))
        );
        // Marco, cabecera, dos rayas verticales y tres horizontales.
        assert_eq!(e.len(), 7);
    }

    #[test]
    fn en_el_cajetin_de_la_tabla_se_anaden_y_quitan_filas_y_columnas_como_en_el_movil() {
        let t = catalogo();
        let filas = vec![
            vec!["a".to_string(), "b".into()],
            vec!["c".into(), "d".into()],
        ];
        let mut f = formulario_de_tabla(&t, &filas, true, "tabla-editar-titulo", "tabla-aplicar");
        assert_eq!(f.campos.len(), 4);
        // Anadir fila y columna las pone al final (el movil), vacias.
        f.accion(Accion::MasFila);
        assert_eq!(
            f.celdas(),
            vec![vec!["a", "b"], vec!["c", "d"], vec!["", ""]]
        );
        assert_eq!(f.activo, 4, "el cursor va a la fila nueva");
        f.accion(Accion::MasColumna);
        assert_eq!(f.celdas()[0], vec!["a", "b", ""]);
        // Quitar quita la de la celda activa: la «b» (fila 0, columna 1).
        f.activo = 1;
        f.accion(Accion::MenosColumna);
        assert_eq!(f.celdas(), vec![vec!["a", ""], vec!["c", ""], vec!["", ""]]);
        f.activo = 2;
        f.accion(Accion::MenosFila);
        assert_eq!(f.celdas(), vec![vec!["a", ""], vec!["", ""]]);
        f.accion(Accion::Cabecera);
        assert_eq!(f.rejilla.map(|r| r.cabecera), Some(false));
        // Caso negativo: la ultima fila y la ultima columna no se quitan.
        let mut uno = formulario_de_tabla(
            &t,
            &[vec!["x".to_string()]],
            false,
            "tabla-editar-titulo",
            "tabla-aplicar",
        );
        uno.accion(Accion::MenosFila);
        uno.accion(Accion::MenosColumna);
        assert_eq!(uno.celdas(), vec![vec!["x"]]);
        // Pegar lo hace quien llama, que lee el portapapeles.
        assert_eq!(uno.accion(Accion::Pegar), Tecla::Accion(Accion::Pegar));
    }

    #[test]
    fn en_la_tabla_tab_y_las_flechas_van_de_celda_en_celda() {
        let filas = vec![
            vec!["a".to_string(), "b".into()],
            vec!["c".into(), "d".into()],
        ];
        let mut f = formulario_de_tabla(
            &catalogo(),
            &filas,
            true,
            "tabla-editar-titulo",
            "tabla-aplicar",
        );
        f.tecla('\t', false);
        assert_eq!(f.activo, 1);
        f.flecha(0x28); // abajo
        assert_eq!(f.activo, 3);
        f.flecha(0x25); // izquierda
        assert_eq!(f.activo, 2);
        f.flecha(0x28); // abajo, en la ultima fila: se queda
        assert_eq!(f.activo, 2);
        for c in "!".chars() {
            f.tecla(c, false);
        }
        assert_eq!(f.celdas()[1][0], "c!");
    }

    #[test]
    fn pegar_una_tabla_en_el_cajetin_la_cambia_entera_y_un_texto_suelto_no_toca_nada() {
        let mut f = formulario_de_tabla(
            &catalogo(),
            &[vec!["a".to_string()]],
            true,
            "tabla-pegada-titulo",
            "tabla-insertar",
        );
        assert!(f.poner_celdas(&[vec!["1".into(), "2".into(), "3".into()]]));
        assert_eq!(f.rejilla.map(|r| r.columnas), Some(3));
        assert!(!f.poner_celdas(&[]));
        assert_eq!(f.celdas(), vec![vec!["1", "2", "3"]]);
    }

    #[test]
    fn el_cajetin_se_maneja_con_el_raton_campos_teclas_y_botones() {
        let t = catalogo();
        let mut f = formulario_de_grafica(&t);
        let d = disponer(&f, 30.0, 1600.0, 1000.0, 1.0);
        // Los limites van de dos en dos, como en el movil.
        assert_eq!(
            d.campos[1].y, d.campos[2].y,
            "x desde y x hasta en la misma fila"
        );
        assert!(d.campos[2].x > d.campos[1].x);
        assert_eq!(d.campos[3].y, d.campos[4].y);
        // Con sitio, el teclado de formulas va al lado de los campos.
        assert_eq!(
            d.teclas.len(),
            TECLADO.iter().map(|f| f.len()).sum::<usize>()
        );
        let centro = |r: RectF| (r.x + r.ancho / 2.0, r.y + r.alto / 2.0);
        // Clic en un campo: se activa.
        let (x, y) = centro(d.campos[3]);
        assert_eq!(f.clic(x, y, &d, true), Tecla::Sigue);
        assert_eq!(f.activo, 3);
        // Clic en «sin» del teclado: escribe en las formulas.
        let (r, _, _) = *d
            .teclas
            .iter()
            .find(|(_, fi, co)| TECLADO[*fi][*co].0 == "sin")
            .unwrap();
        let (x, y) = centro(r);
        f.clic(x, y, &d, true);
        assert_eq!(f.campos[0].texto, "sin(x)sin(");
        assert_eq!(f.activo, 0);
        let (r, _, _) = *d
            .teclas
            .iter()
            .find(|(_, fi, co)| TECLADO[*fi][*co].0 == "⌫")
            .unwrap();
        let (x, y) = centro(r);
        f.clic(x, y, &d, true);
        assert_eq!(f.campos[0].texto, "sin(x)sin");
        // Los botones: aceptar solo si todo se entiende; cancelar y la cruz, siempre.
        let (x, y) = centro(d.aceptar);
        assert_eq!(
            f.clic(x, y, &d, false),
            Tecla::Sigue,
            "aceptar apagado no acepta"
        );
        assert_eq!(f.clic(x, y, &d, true), Tecla::Aceptar);
        let (x, y) = centro(d.cancelar);
        assert_eq!(f.clic(x, y, &d, true), Tecla::Cancelar);
        let (x, y) = centro(d.cerrar);
        assert_eq!(f.clic(x, y, &d, true), Tecla::Cancelar);
        // Caso negativo: fuera de la tarjeta no pasa nada.
        assert_eq!(f.clic(1.0, 1.0, &d, true), Tecla::Sigue);
        assert!(
            d.campos
                .iter()
                .chain([d.aceptar, d.cancelar].iter())
                .all(|r| r.x >= d.tarjeta.x
                    && r.y >= d.tarjeta.y
                    && r.x + r.ancho <= d.tarjeta.x + d.tarjeta.ancho + 0.01
                    && r.y + r.alto <= d.tarjeta.y + d.tarjeta.alto + 0.01)
        );
        // En una ventana estrecha el teclado baja debajo de las formulas.
        let estrecha = disponer(&f, 30.0, 700.0, 1000.0, 1.0);
        assert!(estrecha.teclas[0].0.y > estrecha.campos[0].y);
        assert!(estrecha.tarjeta.ancho <= 700.0);
    }

    #[test]
    fn la_biblioteca_va_y_vuelve_del_disco_y_un_fichero_roto_es_una_lista_vacia() {
        let dir = std::env::temp_dir().join(format!("pixpin-figuras-{}", std::process::id()));
        let ruta = dir.join("figuras.json");
        let f = FiguraGuardada {
            nombre: "Sello".into(),
            elementos: vec![Elemento::default()],
        };
        assert!(guardar_biblioteca(&ruta, std::slice::from_ref(&f)));
        assert_eq!(cargar_biblioteca(&ruta), vec![f]);
        std::fs::write(&ruta, b"{roto").unwrap();
        assert!(cargar_biblioteca(&ruta).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_cajetin_del_cronograma_pone_nombres_filas_y_columnas_en_un_paso() {
        use pixpin_motor2d::cronograma;
        let t = catalogo();
        let mut escena = Escena::nueva();
        let id = escena.anadir(Elemento {
            figura: pixpin_motor2d::Figura::Cronograma {
                tareas: cronograma::tareas_de_fabrica(),
                periodos: 6,
            },
            ancho: 600.0,
            alto: 300.0,
            ..Default::default()
        });
        let (tareas, periodos) = match &escena.buscar(id).unwrap().figura {
            pixpin_motor2d::Figura::Cronograma { tareas, periodos } => (tareas.clone(), *periodos),
            _ => unreachable!(),
        };
        let mut f = formulario_de_cronograma(&t, &tareas, periodos);
        assert_eq!(
            f.campos.len(),
            2 + 3,
            "filas, columnas y un nombre por fila"
        );
        assert_eq!(f.activo, 2, "empieza en el nombre de la primera");
        for c in "Obra".chars() {
            f.tecla(c, false);
        }
        f.campos[0].texto = "4".into();
        f.campos[1].texto = "9".into();
        let (filas, columnas, nombres) = cronograma_de(&f, &t).unwrap();
        assert!(aplicar_cronograma(
            &mut escena,
            id,
            filas,
            columnas,
            &nombres
        ));
        match &escena.buscar(id).unwrap().figura {
            pixpin_motor2d::Figura::Cronograma { tareas, periodos } => {
                assert_eq!(tareas.len(), 4);
                assert_eq!(tareas[0].nombre, "Obra");
                assert_eq!(*periodos, 9);
            }
            _ => unreachable!(),
        }
        escena.deshacer();
        assert!(matches!(
            &escena.buscar(id).unwrap().figura,
            pixpin_motor2d::Figura::Cronograma { tareas, periodos: 6 } if tareas.len() == 3
        ));
        // Caso negativo: una cuenta que no es un numero no se aplica.
        f.campos[1].texto = "muchas".into();
        assert!(cronograma_de(&f, &t).is_err());
        f.campos[1].texto = "0".into();
        assert!(cronograma_de(&f, &t).is_err());
    }

    #[test]
    fn un_nombre_largo_en_el_cajetin_ensancha_el_cronograma_en_vez_de_recortarse() {
        let mut escena = Escena::nueva();
        let mut e = Elemento {
            figura: pixpin_motor2d::Figura::Cronograma {
                tareas: pixpin_motor2d::cronograma::tareas_de_fabrica(),
                periodos: 6,
            },
            ancho: 300.0,
            alto: 200.0,
            ..Default::default()
        };
        e.extras.tam_letra = Some(20.0);
        let id = escena.anadir(e);
        let nombres = vec![
            "Estructura y muros de carga".to_string(),
            "B".into(),
            "C".into(),
        ];
        assert!(aplicar_cronograma(&mut escena, id, 3, 6, &nombres));
        let e = escena.buscar(id).unwrap();
        assert!(e.ancho > 300.0, "no se ensancho: {}", e.ancho);
        assert_eq!(e.x, 0.0, "se ensancha hacia la derecha");
        // Caso negativo: con nombres cortos el ancho no se toca.
        assert!(aplicar_cronograma(
            &mut escena,
            id,
            3,
            6,
            &["A".into(), "B".into(), "C".into()]
        ));
        assert!(
            escena.buscar(id).unwrap().ancho > 300.0,
            "no encoge lo que el usuario ya tenia"
        );
    }

    #[test]
    fn la_copia_de_zona_sale_corrida_elegida_y_con_su_grupo() {
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::nuevo();
        let mut imagenes = ImagenesLienzo::nuevo(4096);
        let foto = pixpin_codec::ImagenRgba {
            ancho: 40,
            alto: 20,
            pixeles: vec![255; 40 * 20 * 4],
        };
        assert!(poner_copia_de_zona(
            &mut escena,
            &mut gesto,
            &mut imagenes,
            foto,
            (10.0, 10.0, 50.0, 30.0),
            2.0
        ));
        let e = escena.visibles().next().unwrap();
        assert!(pixpin_motor2d::zona::es_copia(e));
        assert_eq!((e.x, e.y), (22.0, 22.0), "24 px de pantalla a zoom 2");
        assert_eq!((e.ancho, e.alto), (40.0, 20.0));
        assert_eq!(gesto.seleccion.ids(), &[e.id]);
    }

    /// **Las muestras para mirar a ojo** (lo pide la regla del proyecto: una
    /// muestra que nadie mira no prueba nada). Deja en `PIXPIN_MUESTRAS` (o
    /// en `target/muestras-lienzo`) la grafica con sus ecuaciones, la tabla
    /// pegada, la foto de una zona recortada en redondo y la presentacion con
    /// su pastilla y la estela del laser.
    ///
    /// `cargo test -p pixpin --bin pixpinmax muestras_del_lienzo -- --ignored
    /// --nocapture --test-threads=1`. Necesita GPU, por eso va ignorada.
    #[test]
    #[ignore]
    fn muestras_del_lienzo() {
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../target/muestras-lienzo")
            });
        std::fs::create_dir_all(&carpeta).unwrap();
        let guardar = |nombre: &str, img: &pixpin_codec::ImagenRgba| {
            let png = pixpin_codec::imagen::codificar_png(img).unwrap();
            let ruta = carpeta.join(format!("{nombre}.png"));
            std::fs::write(&ruta, png).unwrap();
            println!("{nombre}: {}", ruta.display());
        };
        let d = pixpin_capture::Dispositivo::nuevo().unwrap();
        let motor = MotorRender::nuevo(d.d3d()).unwrap();
        // El medidor de la aplicacion (lo pone `main` al arrancar): sin el,
        // la muestra mediria a ojo y no se veria lo que ve el usuario.
        pixpin_motor2d::texto::instalar_medidor(crate::dibujo::pintar::medir_para_el_motor);
        let medir = medidor(&motor, pixpin_motor2d::texto::nombre_de_familia(None));
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::nuevo();
        let estilo = estilo_del_pincel(&gesto);
        // La grafica: tres curvas, una por partes, con sus ecuaciones.
        let p = Peticion {
            formulas: vec![
                "sin(x)/x".into(),
                "sqrt(x+4) - 1".into(),
                "x^2/4 si x < 0; -x/2 si x >= 0".into(),
            ],
            x_desde: -6.0,
            x_hasta: 6.0,
            y_desde: -3.0,
            y_hasta: 3.0,
            escala: 50.0,
        };
        let v = grafica::elementos(&p, &estilo, &medir).unwrap();
        estampar_en_la_vista(
            &mut escena,
            &mut gesto,
            Punto2::nuevo(300.0, 150.0),
            "grafica",
            &v,
        );
        // Unas ecuaciones sueltas, las de la pizarra.
        let mut y = 380.0;
        for f in [
            "(x+1)^3/sqrt(2x)",
            "|x - 2| + e^(-x^2)",
            "log2(x) + cbrt(x) * pi",
        ] {
            let c = pixpin_motor2d::formula::compilar(f).unwrap();
            for e in ecuacion::elementos("f(x)", &c, 20.0, y, &estilo, &medir) {
                escena.anadir(e);
            }
            y += 70.0;
        }
        // La tabla pegada de Excel.
        pegar_tabla(
            None,
            "Material\tCantidad\tPrecio\nCemento\t12\t8,50\nArena fina\t3\t22,00\nLadrillo\t1200\t0,35\n",
            &mut escena,
            &mut gesto,
            Punto2::nuevo(900.0, 150.0),
            &medir,
        );
        // La de Sheets con celdas combinadas y negrita: su HTML manda.
        pegar_tabla(
            Some(
                "<table><tr><td style=\"font-weight:bold\">DESCRIPCION</td><td style=\"font-weight:bold\">MONTO</td></tr>\
                 <tr><td rowspan=\"3\">MENSUALIDAD RUBY</td><td>690</td></tr><tr><td>687.5</td></tr><tr><td>685.4</td></tr>\
                 <tr><td rowspan=\"2\">CELULAR</td><td>39.95</td></tr><tr><td>19.9</td></tr>\
                 <tr><td>INTERNET</td><td>64.99</td></tr><tr><td></td><td style=\"font-weight:bold\">2517.34</td></tr></table>",
            ),
            "DESCRIPCION\tMONTO\nMENSUALIDAD RUBY\t690\n\t687.5\n\t685.4\nCELULAR\t39.95\n\t19.9\nINTERNET\t64.99\n\t2517.34\n",
            &mut escena,
            &mut gesto,
            Punto2::nuevo(1250.0, 150.0),
            &medir,
        );
        // Un cronograma con nombres y una barra de otro color.
        let mut tareas = pixpin_motor2d::cronograma::tareas_de_fabrica();
        tareas[0].nombre = "Cimientos".into();
        tareas[1].nombre = "Estructura y muros de carga".into();
        tareas[1].cuanto = 2.5;
        tareas[2].desde = 3.5;
        tareas[2].color = Some(grafica::color_de(0x2f9e44));
        escena.anadir(Elemento {
            figura: pixpin_motor2d::Figura::Cronograma {
                tareas,
                periodos: 6,
            },
            x: 680.0,
            y: 330.0,
            ancho: 520.0,
            alto: 220.0,
            trazo: ColorRgba::opaco(0.12, 0.12, 0.12),
            relleno: Some(grafica::color_de(0x1971c2)),
            grosor: 1.0,
            ..Default::default()
        });
        let fotos = |_: u64| None;
        let lienzo = super::super::exportar::Lienzo {
            escena: &escena,
            seleccion: &[],
            papel: None,
            fotos: &fotos,
            nombre: "muestra".into(),
        };
        let hojas = pixpin_motor2d::exportar::hojas(
            &escena,
            pixpin_motor2d::exportar::Alcance::Todo,
            &[],
            None,
        );
        let img =
            super::super::exportar::a_imagen(&hojas[0], 1.0, Some(escena.fondo), &lienzo).unwrap();
        guardar("grafica-ecuaciones-y-tabla", &img);
        // La zona: el trozo de la grafica, recortado en redondo con su filo,
        // sobre papel claro y sobre la pizarra.
        let foto = foto_de_la_zona(&lienzo, (40.0, -120.0, 400.0, 200.0)).unwrap();
        guardar("zona-papel-claro", &foto);
        let mut noche = escena.clone();
        noche.fondo = ColorRgba::opaco(
            0x12 as f32 / 255.0,
            0x12 as f32 / 255.0,
            0x12 as f32 / 255.0,
        );
        let de_noche = super::super::exportar::Lienzo {
            escena: &noche,
            seleccion: &[],
            papel: None,
            fotos: &fotos,
            nombre: "muestra".into(),
        };
        guardar(
            "zona-pizarra",
            &foto_de_la_zona(&de_noche, (40.0, -120.0, 400.0, 200.0)).unwrap(),
        );
        // La presentacion: la pastilla y la estela del laser encima de un
        // fondo, a 1280 x 720.
        let (w, h) = (1280u32, 720u32);
        let destino =
            pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, d.d3d(), w, h)
                .unwrap();
        let pr = super::super::presentar::Presentacion {
            hojas: vec![(0.0, 0.0, 10.0, 10.0); 7],
            actual: 2,
            negro: false,
            modo: super::super::presentar::Modo::Laser,
            pastilla: true,
            antes: (Camara::nueva(), pixpin_motor2d::gesto::Herramienta::Lapiz),
        };
        let mut laser = pixpin_motor2d::puntero_laser::PunteroLaser::default();
        laser.pulsar_en(Punto2::nuevo(200.0, 400.0), 0.0);
        for i in 1..60 {
            let t = i as f32 / 60.0;
            laser.mover_en(
                Punto2::nuevo(200.0 + t * 800.0, 400.0 - (t * 9.0).sin() * 120.0),
                i as f64 * 12.0,
            );
        }
        let sin_imagenes = ImagenesLienzo::nuevo(1);
        motor
            .dibujar(&destino.destino, |p| {
                p.limpiar(Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                });
                for o in laser.ordenes_en(760.0, 1.0) {
                    dibujar_orden(
                        p,
                        &o,
                        (0.0, 0.0, w as f32, h as f32),
                        None,
                        &sin_imagenes,
                        1.0,
                        None,
                    );
                }
                super::super::presentar::pintar(
                    p,
                    (0.0, 0.0),
                    &pr,
                    w as f32,
                    h as f32,
                    100,
                    "← → Av Pág: pasar · B: negro · J: láser · L: lápiz · Esc: salir",
                );
            })
            .unwrap();
        let (ancho, alto, pixeles) = destino.leer_rgba().unwrap();
        guardar(
            "presentar-pastilla-y-laser",
            &pixpin_codec::ImagenRgba {
                ancho,
                alto,
                pixeles,
            },
        );
    }
}

#[cfg(test)]
mod muestras_figuras;
