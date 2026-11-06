//! **La ventana del editor**: la paleta de bloques a la izquierda, la
//! pagina en medio y las propiedades a la derecha, con la galeria y la IA.
//!
//! Va como las ventanas de PinDesk (la de Figuras sobre todo): una
//! `VentanaOverlay` normal que pinta con Direct2D, la cabecera del movil con
//! sus pastillas, el ☀/☾ y los colores de dia y de noche del chat. Lo que no
//! es pintar ni repartir clics (insertar, mover, deshacer, las guias de
//! soltar, las propiedades, la IA) lo hace el nucleo y esta probado alli.
//!
//! Raton: arrastrar una pieza de la paleta a la pagina la inserta donde
//! dice la guia (entre, al lado o dentro); arrastrar un bloque lo mueve; el
//! asa de su borde le cambia el tamaño. La rueda lleva la pagina, Ctrl +
//! rueda (o pellizcar en el panel) acerca hacia el raton, dos dedos la
//! llevan. Teclado: Supr borra, Ctrl+D duplica, Ctrl+Z / Ctrl+Y deshacen y
//! rehacen, Ctrl+S guarda, Ctrl+O abre, Ctrl+N nueva, Ctrl+E exporta a PDF,
//! Alt+↑/↓ sube y baja, RePag/AvPag cambian de pagina, Ctrl + «+»/«−»/«0»
//! acercan, alejan y encajan.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use nucleo::edicion::{Destino, Editor, Guia, asa_en, destino_en};
use nucleo::enlace::Accion;
use nucleo::exportar;
use nucleo::fecha::Fecha;
use nucleo::formato;
use nucleo::galeria;
use nucleo::ia;
use nucleo::modelo::{Bloque, Cada, Documento, Eje, Repeticion, Tamano};
use nucleo::paleta::{self, Valor};
use nucleo::tema::Tema;
use nucleo::textos::Textos;
use pindesk_geom::{Punto, Rect};
use pindesk_render::{Color, MotorRender, Pintor, RectF, Superficie};
use pindesk_shell::overlay::{EventoOverlay, FormaCursorWin, VentanaOverlay};

use windows::Win32::Foundation::HWND;

use crate::pintar::{self, Maqueta, Vista};

// --- Medidas (pixeles logicos) ---------------------------------------------------

const BARRA: f32 = 56.0;
const PALETA: f32 = 212.0;
const PANEL: f32 = 312.0;
const FILA: f32 = 30.0;
const MARGEN: f32 = 14.0;
const PASTILLA: f32 = 30.0;
const DURA_AVISO: Duration = Duration::from_millis(3200);

// --- Teclas ----------------------------------------------------------------------

const VK_RETROCESO: u32 = 0x08;
const VK_INTRO: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_REPAG: u32 = 0x21;
const VK_AVPAG: u32 = 0x22;
const VK_ARRIBA: u32 = 0x26;
const VK_ABAJO: u32 = 0x28;
const VK_SUPR: u32 = 0x2E;

/// Hoy (UTC; lo que importa es el dia).
pub fn hoy() -> Fecha {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    Fecha::de_dias((s / 86_400) as i64)
}

/// **Exporta un documento** a `ruta` en el formato de su extension. La
/// pagina `n` para lo que es de una pagina (PNG, SVG, lienzo).
pub fn exportar_a(
    doc: &Documento,
    hoy: Fecha,
    n: usize,
    formato: &str,
    ruta: &Path,
) -> Result<(), String> {
    let datos: Vec<u8> = match formato {
        "pdf" => {
            let paginas = exportar::paginas(doc, hoy);
            exportar::pdf::a_pdf(&paginas, &crate::rutas::letras(), &doc.nombre)
        }
        "png" => {
            let p = exportar::pagina(doc, hoy, n);
            exportar::png::a_png(&p, 200.0, &crate::rutas::letras()).ok_or("png")?
        }
        "svg" => {
            let p = exportar::pagina(doc, hoy, n);
            let l = crate::rutas::letras();
            exportar::svg::a_svg(&p, &doc.tema, &l.vista(&p.familias)).into_bytes()
        }
        "excalidraw" => {
            let p = exportar::pagina(doc, hoy, n);
            exportar::figura::excalidraw(&[p], &doc.tema).into_bytes()
        }
        otro => return Err(format!("formato «{otro}»")),
    };
    std::fs::write(ruta, datos).map_err(|e| e.to_string())
}

// --- Colores de la interfaz (los del chat de PinDesk, de dia y de noche) ---------

const fn hex(c: u32) -> Color {
    Color {
        r: ((c >> 16) & 0xff) as f32 / 255.0,
        g: ((c >> 8) & 0xff) as f32 / 255.0,
        b: (c & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

#[derive(Clone, Copy)]
struct Colores {
    barra: Color,
    panel: Color,
    lienzo: Color,
    separador: Color,
    texto: Color,
    apagado: Color,
    pastilla: Color,
    pastilla_sobre: Color,
    acento: Color,
    texto_acento: Color,
    campo: Color,
    peligro: Color,
}

const CLARO: Colores = Colores {
    barra: hex(0xf1f1f1),
    panel: hex(0xffffff),
    lienzo: hex(0xe9ecf1),
    separador: hex(0xe0e0e0),
    texto: hex(0x222222),
    apagado: hex(0x8a8f98),
    pastilla: hex(0xe8e8ea),
    pastilla_sobre: hex(0xdcdce0),
    acento: hex(0x419fd9),
    texto_acento: hex(0xffffff),
    campo: hex(0xf3f4f6),
    peligro: hex(0xe5484d),
};

const OSCURO: Colores = Colores {
    barra: hex(0x242f3d),
    panel: hex(0x17212b),
    lienzo: hex(0x0e1621),
    separador: hex(0x101921),
    texto: hex(0xf5f5f5),
    apagado: hex(0x7f91a4),
    pastilla: hex(0x2c3847),
    pastilla_sobre: hex(0x34445a),
    acento: hex(0x2b82c9),
    texto_acento: hex(0xffffff),
    campo: hex(0x202b36),
    peligro: hex(0xe92539),
};

fn con_alfa(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

// --- El estado ---------------------------------------------------------------------

/// Lo que hace un boton.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Orden {
    Nuevo,
    Abrir,
    Guardar,
    MenuExportar,
    Exportar(&'static str),
    Enviar,
    Galeria,
    Deshacer,
    Rehacer,
    DiaNoche,
    Cerrar,
    PaginaAntes,
    PaginaDespues,
    Encajar,
    Duplicar,
    Borrar,
    Subir,
    Bajar,
    EnFila,
    EnColumna,
    IaCrear,
    IaAjustar,
    IaSugerir,
    IaAplicar,
    IaDescartar,
    CerrarGaleria,
}

/// Que se escribe en un campo.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Objetivo {
    Nombre,
    Describir,
    Ajustar,
    Prop(u64, &'static str),
    Desde,
    Hasta,
}

/// Un cambio del documento desde el panel (sin bloque elegido).
#[derive(Debug, Clone, Copy, PartialEq)]
enum DocCampo {
    Papel,
    Horizontal,
    Margen(i32),
    Tema,
    Idioma,
    Cada,
    SinFinde,
}

/// Lo que hay bajo el raton.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Zona {
    Barra,
    Boton(Orden),
    Pieza(&'static str),
    Campo(Objetivo),
    /// Una propiedad: su clave y el paso (−1, +1) o 0 (alternar / siguiente).
    Prop(&'static str, i32),
    Doc(DocCampo),
    Tarjeta(usize),
    Lienzo,
    Paleta,
    Panel,
}

/// Lo que se arrastra.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Arrastre {
    Nuevo {
        pieza: &'static str,
        movido: bool,
    },
    Mover {
        id: u64,
        desde: Punto,
        movido: bool,
    },
    Asa {
        id: u64,
        eje: Eje,
        inicio: f32,
    },
    Llevar {
        desde: Punto,
        origen: (f32, f32),
    },
    Borde {
        borde: pindesk_ui::chat::Borde,
        marco: Rect,
    },
}

/// Lo que se esta escribiendo.
struct Foco {
    objetivo: Objetivo,
    texto: String,
}

/// Que hace «Aplicar» con la propuesta de la IA.
enum Aplicar {
    Todo,
    Bloque(u64, Bloque),
}

struct Propuesta {
    doc: Documento,
    aplicar: Aplicar,
    maqueta: Maqueta,
}

/// Lo que se le pide a la IA.
#[derive(Debug, Clone, Copy, PartialEq)]
enum QueIa {
    Describir,
    Ajustar,
    Sugerir,
}

/// Lo que devuelve el hilo de la IA.
enum Hecho {
    Doc(Result<Box<ia::Propuesta>, String>),
    Bloque(u64, Result<Bloque, String>),
    SinClave,
}

struct App {
    ed: Editor,
    t: Textos,
    ruta: Option<PathBuf>,
    hoy: Fecha,
    pagina: usize,
    paginas: usize,
    vista: Vista,
    encajada: bool,
    maqueta: Option<Maqueta>,
    propuesta: Option<Propuesta>,
    galeria: bool,
    tarjetas: Vec<(Documento, Maqueta)>,
    arrastre: Option<Arrastre>,
    guia: Option<(Destino, Guia)>,
    foco: Option<Foco>,
    raton: Punto,
    zonas: RefCell<Vec<(RectF, Zona)>>,
    aviso: Option<(String, Instant)>,
    noche: bool,
    ia_pendiente: Option<Receiver<Hecho>>,
    ia_que: Option<QueIa>,
    describir: String,
    ajustar: String,
    menu_exportar: bool,
    scroll_paleta: f32,
    scroll_panel: f32,
    /// Pixeles por pixel logico.
    e: f32,
    escala_por_cien: u32,
    ancho: f32,
    alto: f32,
    hwnd: isize,
}

impl App {
    fn nueva(t: Textos, e: f32, escala_por_cien: u32) -> App {
        let hoy = hoy();
        let doc = galeria::por_clave("semanal", &t, hoy).unwrap_or_else(|| Documento::nuevo(""));
        let mut a = App {
            ed: Editor::nuevo(doc),
            t,
            ruta: None,
            hoy,
            pagina: 0,
            paginas: 1,
            vista: Vista {
                ox: 0.0,
                oy: 0.0,
                escala: 3.0,
            },
            encajada: true,
            maqueta: None,
            propuesta: None,
            galeria: true,
            tarjetas: Vec::new(),
            arrastre: None,
            guia: None,
            foco: None,
            raton: Punto { x: -1, y: -1 },
            zonas: RefCell::new(Vec::new()),
            aviso: None,
            noche: crate::rutas::noche_elegida().unwrap_or(!pindesk_shell::tema_claro()),
            ia_pendiente: None,
            ia_que: None,
            describir: String::new(),
            ajustar: String::new(),
            menu_exportar: false,
            scroll_paleta: 0.0,
            scroll_panel: 0.0,
            e,
            escala_por_cien,
            ancho: 0.0,
            alto: 0.0,
            hwnd: 0,
        };
        a.ed.cambiado = false;
        a
    }

    fn colores(&self) -> Colores {
        if self.noche { OSCURO } else { CLARO }
    }

    fn l(&self, x: f32) -> f32 {
        x * self.e
    }

    fn lienzo(&self) -> RectF {
        RectF {
            x: self.l(PALETA),
            y: self.l(BARRA),
            ancho: (self.ancho - self.l(PALETA + PANEL)).max(50.0),
            alto: (self.alto - self.l(BARRA)).max(50.0),
        }
    }

    /// Que hay que volver a maquetar.
    fn tocado(&mut self) {
        self.maqueta = None;
    }

    fn maquetar(&mut self) {
        if self.maqueta.is_some() {
            return;
        }
        self.paginas = exportar::contextos(&self.ed.doc, self.hoy).len();
        self.pagina = self.pagina.min(self.paginas.saturating_sub(1));
        let p = exportar::pagina(&self.ed.doc, self.hoy, self.pagina);
        self.maqueta = Some(Maqueta::nueva(p));
        if self.encajada {
            self.encajar();
        }
    }

    fn encajar(&mut self) {
        let (w, h) = self.ed.doc.pagina.medidas();
        let l = self.lienzo();
        let m = self.l(36.0);
        let s = ((l.ancho - 2.0 * m) / w)
            .min((l.alto - 2.0 * m - self.l(30.0)) / h)
            .max(0.2);
        self.vista = Vista {
            ox: l.x + (l.ancho - w * s) / 2.0,
            oy: l.y + m,
            escala: s,
        };
        self.encajada = true;
    }

    fn acercar(&mut self, f: f32, en: (f32, f32)) {
        let (w, _) = self.ed.doc.pagina.medidas();
        let min = (self.lienzo().ancho * 0.15 / w).max(0.2);
        let nueva = (self.vista.escala * f).clamp(min, 40.0 * self.e);
        let (mx, my) = self.vista.a_mm(en.0, en.1);
        self.vista.escala = nueva;
        self.vista.ox = en.0 - mx * nueva;
        self.vista.oy = en.1 - my * nueva;
        self.encajada = false;
    }

    fn llevar(&mut self, dx: f32, dy: f32) {
        self.vista.ox += dx;
        self.vista.oy += dy;
        self.encajada = false;
    }

    fn avisar(&mut self, s: String) {
        self.aviso = Some((s, Instant::now()));
    }

    fn zona_en(&self, p: Punto) -> Option<Zona> {
        let (x, y) = (p.x as f32, p.y as f32);
        self.zonas
            .borrow()
            .iter()
            .rev()
            .find(|(r, _)| x >= r.x && x < r.x + r.ancho && y >= r.y && y < r.y + r.alto)
            .map(|(_, z)| *z)
    }

    fn sobre_lienzo(&self, p: Punto) -> bool {
        matches!(self.zona_en(p), Some(Zona::Lienzo))
    }

    /// El punto de la ventana en mm de la pagina.
    fn mm(&self, p: Punto) -> (f32, f32) {
        self.vista.a_mm(p.x as f32, p.y as f32)
    }

    // --- Ficheros -------------------------------------------------------------

    fn titulo_fichero(&self) -> String {
        if self.ed.doc.nombre.trim().is_empty() {
            self.t.t("plt-sin-titulo")
        } else {
            self.ed.doc.nombre.clone()
        }
    }

    fn poner_doc(&mut self, doc: Documento, ruta: Option<PathBuf>) {
        self.ed = Editor::nuevo(doc);
        self.ruta = ruta;
        self.pagina = 0;
        self.propuesta = None;
        self.galeria = false;
        self.encajada = true;
        self.tocado();
    }

    fn abrir(&mut self, ruta: &Path) {
        match std::fs::read_to_string(ruta)
            .map_err(|e| e.to_string())
            .and_then(|t| formato::leer(&t).map_err(|e| e.to_string()))
        {
            Ok((doc, _)) => self.poner_doc(doc, Some(ruta.to_path_buf())),
            Err(_) => {
                let s = self
                    .t
                    .con("plt-error-abrir", &[("ruta", ruta.display().to_string())]);
                self.avisar(s);
            }
        }
    }

    fn guardar(&mut self, como: bool) -> bool {
        let ruta = match (&self.ruta, como) {
            (Some(r), false) => Some(r.clone()),
            _ => pindesk_shell::guardar::pedir_ruta_para(
                hwnd(self.hwnd),
                &self.titulo_fichero(),
                &self.t.t("plt-filtro"),
                formato::EXTENSION,
            ),
        };
        let Some(ruta) = ruta else {
            return false;
        };
        match std::fs::write(&ruta, formato::escribir(&self.ed.doc)) {
            Ok(()) => {
                self.ruta = Some(ruta);
                self.ed.cambiado = false;
                let s = self.t.t("plt-guardado");
                self.avisar(s);
                true
            }
            Err(e) => {
                let s = self
                    .t
                    .con("plt-error-guardar", &[("motivo", e.to_string())]);
                self.avisar(s);
                false
            }
        }
    }

    /// Antes de dejar el documento: si hay cambios, pregunta si guardar.
    fn puede_dejar(&mut self) -> bool {
        if !self.ed.cambiado {
            return true;
        }
        if pindesk_shell::preguntar(
            hwnd(self.hwnd),
            nucleo::NOMBRE_PRODUCTO,
            &self.t.t("plt-cambios-sin-guardar"),
        ) {
            return self.guardar(false);
        }
        true
    }

    fn exportar(&mut self, formato: &'static str) {
        self.menu_exportar = false;
        let (tipo, ext) = match formato {
            "pdf" => ("PDF", "pdf"),
            "png" => ("PNG", "png"),
            "svg" => ("SVG", "svg"),
            _ => ("Excalidraw", "excalidraw"),
        };
        let Some(ruta) = pindesk_shell::guardar::pedir_ruta_para(
            hwnd(self.hwnd),
            &self.titulo_fichero(),
            tipo,
            ext,
        ) else {
            return;
        };
        let s = match exportar_a(&self.ed.doc, self.hoy, self.pagina, formato, &ruta) {
            Ok(()) => self
                .t
                .con("plt-exportado", &[("ruta", ruta.display().to_string())]),
            Err(e) => self.t.con("plt-error-guardar", &[("motivo", e)]),
        };
        self.avisar(s);
    }

    fn enviar_a_pindesk(&mut self) {
        let s = match nucleo::carpeta::guardar_en(&crate::rutas::carpeta_compartida(), &self.ed.doc)
        {
            Ok(_) => self.t.t("plt-enviada"),
            Err(e) => self
                .t
                .con("plt-error-guardar", &[("motivo", e.to_string())]),
        };
        self.avisar(s);
    }

    // --- La IA ----------------------------------------------------------------

    fn lanzar_ia(&mut self, que: QueIa) {
        if self.ia_pendiente.is_some() {
            return;
        }
        let (tx, rx) = channel();
        let doc = self.ed.doc.clone();
        let hoy = self.hoy;
        let idioma = self.t.etiqueta().to_string();
        let hwnd = self.hwnd;
        let frase = self.describir.clone();
        let orden = self.ajustar.clone();
        let sel = self.ed.seleccion;
        match que {
            QueIa::Describir if frase.trim().is_empty() => return,
            QueIa::Ajustar if orden.trim().is_empty() || sel.is_none() => return,
            QueIa::Sugerir if sel.is_none() => return,
            _ => {}
        }
        let lanzado = std::thread::Builder::new()
            .name("plantillas-ia".into())
            .spawn(move || {
                let hecho = match crate::nube::Nube::de_pindesk() {
                    None => Hecho::SinClave,
                    Some(n) => match (que, sel) {
                        (QueIa::Describir, _) => Hecho::Doc(
                            ia::describir(&n, &frase, &idioma, hoy)
                                .map(Box::new)
                                .map_err(|e| e.to_string()),
                        ),
                        (QueIa::Sugerir, Some(id)) => Hecho::Bloque(
                            id,
                            ia::sugerir(&n, &doc, id, hoy)
                                .map(|(b, _)| b)
                                .map_err(|e| e.to_string()),
                        ),
                        (QueIa::Ajustar, Some(id)) => Hecho::Bloque(
                            id,
                            ia::ajustar(&n, &doc, id, &orden, hoy)
                                .map(|(b, _)| b)
                                .map_err(|e| e.to_string()),
                        ),
                        _ => Hecho::SinClave,
                    },
                };
                let _ = tx.send(hecho);
                pindesk_shell::overlay::despertar(hwnd);
            });
        if lanzado.is_ok() {
            self.ia_pendiente = Some(rx);
            self.ia_que = Some(que);
        }
    }

    fn mirar_ia(&mut self) -> bool {
        let Some(rx) = &self.ia_pendiente else {
            return false;
        };
        let Ok(hecho) = rx.try_recv() else {
            return false;
        };
        self.ia_pendiente = None;
        let que = self.ia_que.take();
        match hecho {
            Hecho::Doc(Ok(p)) => {
                if !p.avisos.is_empty() {
                    let s = self
                        .t
                        .con("plt-ia-avisos", &[("n", p.avisos.len().to_string())]);
                    self.avisar(s);
                }
                self.proponer(p.doc, Aplicar::Todo);
            }
            Hecho::Bloque(id, Ok(b)) => {
                let mut doc = self.ed.doc.clone();
                if let Some(x) = doc.buscar_mut(id) {
                    let tam = x.tam;
                    *x = b.clone();
                    x.id = id;
                    if x.tam == nucleo::modelo::Tam::Auto {
                        x.tam = tam;
                    }
                }
                doc.numerar();
                self.proponer(doc, Aplicar::Bloque(id, b));
            }
            Hecho::Doc(Err(e)) | Hecho::Bloque(_, Err(e)) => {
                let s = self.t.con("plt-ia-error", &[("motivo", e)]);
                self.avisar(s);
            }
            Hecho::SinClave => {
                // Sin clave: lo que se pueda sin modelo, y se dice.
                let s = self.t.t("plt-ia-sin-clave");
                self.avisar(s);
                match que {
                    Some(QueIa::Describir) => {
                        let d =
                            ia::sin_modelo(&self.describir, self.t.etiqueta(), &self.t, self.hoy);
                        self.proponer(d, Aplicar::Todo);
                    }
                    Some(QueIa::Ajustar) => {
                        if let Some(id) = self.ed.seleccion
                            && let Some(b) = self
                                .ed
                                .doc
                                .buscar(id)
                                .and_then(|b| ia::ajustar_sin_modelo(b, &self.ajustar))
                        {
                            let mut doc = self.ed.doc.clone();
                            if let Some(x) = doc.buscar_mut(id) {
                                *x = b.clone();
                                x.id = id;
                            }
                            doc.numerar();
                            self.proponer(doc, Aplicar::Bloque(id, b));
                        }
                    }
                    _ => {}
                }
            }
        }
        true
    }

    fn proponer(&mut self, doc: Documento, aplicar: Aplicar) {
        let p = exportar::pagina(&doc, self.hoy, 0);
        self.propuesta = Some(Propuesta {
            doc,
            aplicar,
            maqueta: Maqueta::nueva(p),
        });
        self.galeria = false;
    }

    fn aplicar_propuesta(&mut self) {
        let Some(p) = self.propuesta.take() else {
            return;
        };
        match p.aplicar {
            Aplicar::Todo => {
                self.ed.poner_documento(p.doc);
                self.pagina = 0;
                self.encajada = true;
            }
            Aplicar::Bloque(id, b) => {
                self.ed.reemplazar(id, b);
            }
        }
        self.tocado();
    }

    // --- Las ordenes -----------------------------------------------------------

    fn orden(&mut self, o: Orden, vivo: &mut bool) {
        if o != Orden::MenuExportar && !matches!(o, Orden::Exportar(_)) {
            self.menu_exportar = false;
        }
        let sel = self.ed.seleccion;
        match o {
            Orden::Nuevo => {
                if self.puede_dejar() {
                    self.galeria = true;
                }
            }
            Orden::Galeria => self.galeria = !self.galeria,
            Orden::CerrarGaleria => self.galeria = false,
            Orden::Abrir => {
                if self.puede_dejar()
                    && let Some(r) = pindesk_shell::elegir::pedir_ficheros(hwnd(self.hwnd))
                        .into_iter()
                        .next()
                {
                    self.abrir(&r);
                }
            }
            Orden::Guardar => {
                self.guardar(false);
            }
            Orden::MenuExportar => self.menu_exportar = !self.menu_exportar,
            Orden::Exportar(f) => self.exportar(f),
            Orden::Enviar => self.enviar_a_pindesk(),
            Orden::Deshacer => {
                if self.ed.deshacer() {
                    self.tocado();
                }
            }
            Orden::Rehacer => {
                if self.ed.rehacer() {
                    self.tocado();
                }
            }
            Orden::DiaNoche => {
                self.noche = !self.noche;
                crate::rutas::recordar_noche(self.noche);
            }
            Orden::Cerrar => {
                if self.puede_dejar() {
                    *vivo = false;
                }
            }
            Orden::PaginaAntes => {
                if self.pagina > 0 {
                    self.pagina -= 1;
                    self.tocado();
                }
            }
            Orden::PaginaDespues => {
                if self.pagina + 1 < self.paginas {
                    self.pagina += 1;
                    self.tocado();
                }
            }
            Orden::Encajar => self.encajar(),
            Orden::Duplicar => {
                if let Some(id) = sel
                    && self.ed.duplicar(id).is_some()
                {
                    self.tocado();
                }
            }
            Orden::Borrar => {
                if let Some(id) = sel
                    && self.ed.borrar(id)
                {
                    self.tocado();
                }
            }
            Orden::Subir | Orden::Bajar => {
                if let Some(id) = sel
                    && self.ed.correr(id, if o == Orden::Subir { -1 } else { 1 })
                {
                    self.tocado();
                }
            }
            Orden::EnFila | Orden::EnColumna => {
                if let Some(id) = sel
                    && self
                        .ed
                        .envolver(
                            id,
                            if o == Orden::EnFila {
                                "fila"
                            } else {
                                "columna"
                            },
                        )
                        .is_some()
                {
                    self.tocado();
                }
            }
            Orden::IaCrear => {
                self.confirmar(false);
                self.lanzar_ia(QueIa::Describir);
            }
            Orden::IaAjustar => {
                self.confirmar(false);
                self.lanzar_ia(QueIa::Ajustar);
            }
            Orden::IaSugerir => self.lanzar_ia(QueIa::Sugerir),
            Orden::IaAplicar => self.aplicar_propuesta(),
            Orden::IaDescartar => self.propuesta = None,
        }
    }

    /// Una propiedad del bloque elegido: `paso` −1/+1 para los numeros, 0
    /// para alternar o pasar a la siguiente opcion.
    fn tocar_prop(&mut self, clave: &'static str, paso: i32) {
        let Some(id) = self.ed.seleccion else { return };
        let Some(b) = self.ed.doc.buscar(id) else {
            return;
        };
        let Some(p) = paleta::de(b).into_iter().find(|p| p.clave == clave) else {
            return;
        };
        let nuevo = match p.valor {
            Valor::Numero {
                valor,
                min,
                max,
                paso: d,
            } => Valor::Numero {
                valor: (valor + d * f64::from(paso)).clamp(min, max),
                min,
                max,
                paso: d,
            },
            Valor::Si(s) => Valor::Si(!s),
            Valor::Opcion { valor, opciones } => {
                let i = opciones.iter().position(|o| *o == valor).unwrap_or(0);
                let n = opciones.len().max(1);
                let j = if paso < 0 {
                    (i + n - 1) % n
                } else {
                    (i + 1) % n
                };
                Valor::Opcion {
                    valor: opciones.get(j).copied().unwrap_or("").to_string(),
                    opciones,
                }
            }
            otro => otro,
        };
        if self.ed.cambiar(id, |b| paleta::poner(b, clave, &nuevo)) {
            self.tocado();
        }
    }

    fn tocar_doc(&mut self, c: DocCampo, paso: i32) {
        let t_claves: Vec<&str> = Tema::claves().collect();
        let idiomas = nucleo::textos::IDIOMAS;
        let hoy = self.hoy;
        let cambio = self.ed.cambiar_documento(|d| match c {
            DocCampo::Papel => {
                let lista = Tamano::DE_SERIE;
                let i = lista
                    .iter()
                    .position(|t| *t == d.pagina.tamano)
                    .unwrap_or(0);
                d.pagina.tamano = lista[(i + 1) % lista.len()];
            }
            DocCampo::Horizontal => d.pagina.horizontal = !d.pagina.horizontal,
            DocCampo::Margen(s) => {
                let m = (d.pagina.margenes.arriba + 2.0 * s as f32).clamp(0.0, 40.0);
                d.pagina.margenes = nucleo::modelo::Margenes::iguales(m);
            }
            DocCampo::Tema => {
                let i = t_claves
                    .iter()
                    .position(|k| *k == d.tema.base)
                    .map_or(0, |i| i + 1);
                d.tema = Tema::de_serie(t_claves[i % t_claves.len()]).unwrap_or_default();
            }
            DocCampo::Idioma => {
                let i = idiomas
                    .iter()
                    .position(|k| *k == d.idioma)
                    .map_or(0, |i| i + 1);
                d.idioma = idiomas[i % idiomas.len()].to_string();
            }
            DocCampo::Cada => {
                d.repeticion = match d.repeticion.as_ref().map(|r| r.cada) {
                    None => Some(Repeticion {
                        cada: Cada::Dia,
                        desde: Fecha::nueva(hoy.ano + 1, 1, 1).unwrap_or(hoy).to_string(),
                        hasta: Fecha::nueva(hoy.ano + 1, 12, 31).unwrap_or(hoy).to_string(),
                        sin_fines_de_semana: false,
                    }),
                    Some(Cada::Dia) => d.repeticion.clone().map(|r| Repeticion {
                        cada: Cada::Semana,
                        ..r
                    }),
                    Some(Cada::Semana) => d.repeticion.clone().map(|r| Repeticion {
                        cada: Cada::Mes,
                        ..r
                    }),
                    Some(Cada::Mes) => None,
                };
            }
            DocCampo::SinFinde => {
                if let Some(r) = d.repeticion.as_mut() {
                    r.sin_fines_de_semana = !r.sin_fines_de_semana;
                }
            }
        });
        let _ = paso;
        if cambio {
            if matches!(c, DocCampo::Papel | DocCampo::Horizontal) {
                self.encajada = true;
            }
            self.tocado();
        }
    }

    // --- Escribir ---------------------------------------------------------------

    fn empezar_a_escribir(&mut self, o: Objetivo) {
        let texto = match o {
            Objetivo::Nombre => self.ed.doc.nombre.clone(),
            Objetivo::Describir => self.describir.clone(),
            Objetivo::Ajustar => self.ajustar.clone(),
            Objetivo::Desde => self
                .ed
                .doc
                .repeticion
                .as_ref()
                .map(|r| r.desde.clone())
                .unwrap_or_default(),
            Objetivo::Hasta => self
                .ed
                .doc
                .repeticion
                .as_ref()
                .map(|r| r.hasta.clone())
                .unwrap_or_default(),
            Objetivo::Prop(id, clave) => self
                .ed
                .doc
                .buscar(id)
                .and_then(|b| paleta::de(b).into_iter().find(|p| p.clave == clave))
                .map(|p| match p.valor {
                    Valor::Texto(s) => s,
                    Valor::Lista(l) => l.join(", "),
                    Valor::Numero { valor, .. } => format!("{}", (valor * 100.0).round() / 100.0),
                    Valor::Opcion { valor, .. } => valor,
                    Valor::Si(s) => s.to_string(),
                })
                .unwrap_or_default(),
        };
        self.foco = Some(Foco { objetivo: o, texto });
    }

    /// Escribe lo del foco en su sitio. Los campos de la IA se quedan con el
    /// texto (y con Intro lanzan la IA).
    fn confirmar(&mut self, intro: bool) {
        let Some(f) = self.foco.take() else { return };
        match f.objetivo {
            Objetivo::Nombre => {
                if self
                    .ed
                    .cambiar_documento(|d| d.nombre = f.texto.trim().to_string())
                {
                    self.tocado();
                }
            }
            Objetivo::Describir => {
                self.describir = f.texto;
                if intro {
                    self.lanzar_ia(QueIa::Describir);
                }
            }
            Objetivo::Ajustar => {
                self.ajustar = f.texto;
                if intro {
                    self.lanzar_ia(QueIa::Ajustar);
                }
            }
            Objetivo::Desde | Objetivo::Hasta => {
                if Fecha::leer(&f.texto).is_some() {
                    let desde = f.objetivo == Objetivo::Desde;
                    let t = f.texto.trim().to_string();
                    if self.ed.cambiar_documento(|d| {
                        if let Some(r) = d.repeticion.as_mut() {
                            if desde {
                                r.desde = t;
                            } else {
                                r.hasta = t;
                            }
                        }
                    }) {
                        self.tocado();
                    }
                }
            }
            Objetivo::Prop(id, clave) => {
                let Some(p) = self
                    .ed
                    .doc
                    .buscar(id)
                    .and_then(|b| paleta::de(b).into_iter().find(|p| p.clave == clave))
                else {
                    return;
                };
                let nuevo = match p.valor {
                    Valor::Numero { min, max, paso, .. } => {
                        match f.texto.trim().replace(',', ".").parse::<f64>() {
                            Ok(v) => Valor::Numero {
                                valor: v.clamp(min, max),
                                min,
                                max,
                                paso,
                            },
                            Err(_) => return,
                        }
                    }
                    Valor::Lista(_) => Valor::Lista(
                        f.texto
                            .split([',', ';'])
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect(),
                    ),
                    _ => Valor::Texto(f.texto),
                };
                if self.ed.cambiar(id, |b| paleta::poner(b, clave, &nuevo)) {
                    self.tocado();
                }
            }
        }
    }
}

fn hwnd(h: isize) -> HWND {
    HWND(h as *mut _)
}

// --- El bucle ----------------------------------------------------------------------

/// **Abre la ventana** y la atiende hasta que se cierra.
pub fn correr(accion: Accion) -> Result<(), String> {
    let t = Textos::nuevos(&crate::rutas::idioma());
    let monitores = pindesk_capture::enumerar_monitores().map_err(|e| e.to_string())?;
    let monitor = monitores
        .monitor_en(pindesk_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .ok_or("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien.max(100);
    let e = escala as f32 / 100.0;
    let area = monitor.area_trabajo;
    let w = ((1440.0 * e) as u32).min(area.ancho);
    let h = ((900.0 * e) as u32).min(area.alto);
    let mut marco = Rect {
        x: area.x + (area.ancho - w) as i32 / 2,
        y: area.y + (area.alto - h) as i32 / 2,
        ancho: w,
        alto: h,
    };
    let mut ventana =
        VentanaOverlay::nueva_normal(marco, &t.t("plt-producto")).map_err(|e| e.to_string())?;
    let dispositivo = pindesk_capture::Dispositivo::nuevo().map_err(|e| e.to_string())?;
    let motor = MotorRender::nuevo(dispositivo.d3d()).map_err(|e| e.to_string())?;
    let superficie = Superficie::nueva(
        &motor,
        dispositivo.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .map_err(|e| e.to_string())?;
    ventana.pedir_gestos_tactiles();
    ventana.aceptar_ficheros(true);
    ventana.mostrar();
    ventana.enfocar();

    let mut a = App::nueva(t, e, escala);
    a.hwnd = ventana.handle().0 as isize;
    a.ancho = marco.ancho as f32;
    a.alto = marco.alto as f32;
    match accion {
        Accion::Abrir(r) => a.abrir(&r),
        Accion::Figura(id) => {
            if let Some(r) =
                nucleo::carpeta::ruta_de_figura(&crate::rutas::carpeta_compartida(), &id)
            {
                a.abrir(&r);
            }
        }
        Accion::Galeria(c) => {
            if let Some(d) = galeria::por_clave(&c, &a.t, a.hoy) {
                a.poner_doc(d, None);
            }
        }
        Accion::Describir(s) => {
            a.describir = s;
            a.galeria = false;
            a.empezar_a_escribir(Objetivo::Describir);
        }
        _ => {}
    }

    let mut vivo = true;
    let mut pintar = true;
    while vivo {
        pindesk_shell::overlay::bombear_pendientes();
        if let Some(real) = ventana.rect_del_sistema()
            && real != marco
        {
            let tam = (real.ancho, real.alto) != (marco.ancho, marco.alto);
            marco = real;
            if tam {
                let _ = superficie.redimensionar(marco.ancho, marco.alto);
                a.ancho = marco.ancho as f32;
                a.alto = marco.alto as f32;
                if a.encajada {
                    a.encajar();
                }
            }
            pintar = true;
        }
        for (hw, ev) in pindesk_shell::overlay::tomar_eventos_pendientes() {
            if hw != ventana.handle() {
                continue;
            }
            pintar |= a.evento(ev, &mut ventana, &mut marco, &superficie, &mut vivo);
        }
        if !vivo {
            break;
        }
        if a.mirar_ia() {
            pintar = true;
        }
        if a.aviso
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed() >= DURA_AVISO)
        {
            a.aviso = None;
            pintar = true;
        }
        if pintar {
            pintar = false;
            a.maquetar();
            if a.galeria && a.tarjetas.is_empty() {
                a.tarjetas = galeria::CLAVES
                    .iter()
                    .filter_map(|c| galeria::por_clave(c, &a.t, a.hoy))
                    .map(|d| {
                        let p = exportar::pagina(&d, a.hoy, 0);
                        (d, Maqueta::nueva(p))
                    })
                    .collect();
            }
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| a.pintar_todo(p));
                let _ = superficie.presentar();
            }
            // Lo que no es boton de la barra arrastra la ventana.
            let (barras, botones) = a.zonas_de_arrastre();
            ventana.poner_arrastre(barras, botones);
        }
        let espera = if a.ia_pendiente.is_some() {
            120
        } else if a.aviso.is_some() {
            250
        } else {
            1_000
        };
        pindesk_shell::overlay::esperar_eventos(Some(espera));
    }
    ventana.ocultar();
    Ok(())
}

fn rect_de(r: RectF) -> Rect {
    Rect {
        x: r.x.round() as i32,
        y: r.y.round() as i32,
        ancho: r.ancho.max(0.0).round() as u32,
        alto: r.alto.max(0.0).round() as u32,
    }
}

impl App {
    fn zonas_de_arrastre(&self) -> (Vec<Rect>, Vec<Rect>) {
        let zonas = self.zonas.borrow();
        let barras = zonas
            .iter()
            .filter(|(_, z)| *z == Zona::Barra)
            .map(|(r, _)| rect_de(*r))
            .collect();
        let botones = zonas
            .iter()
            .filter(|(r, z)| *z != Zona::Barra && r.y < self.l(BARRA))
            .map(|(r, _)| rect_de(*r))
            .collect();
        (barras, botones)
    }

    /// **Un evento.** Devuelve si hay que pintar.
    fn evento(
        &mut self,
        ev: EventoOverlay,
        ventana: &mut VentanaOverlay,
        marco: &mut Rect,
        superficie: &Superficie,
        vivo: &mut bool,
    ) -> bool {
        let local = |p: Punto, m: &Rect| Punto {
            x: p.x - m.x,
            y: p.y - m.y,
        };
        match ev {
            EventoOverlay::Cerrar => {
                if self.puede_dejar() {
                    *vivo = false;
                }
                false
            }
            EventoOverlay::RatonMovido(p) => {
                let l = local(p, marco);
                let antes = self.raton;
                self.raton = l;
                self.mover_raton(l, p, ventana, marco, superficie);
                let cursor = match self.arrastre {
                    Some(Arrastre::Asa {
                        eje: Eje::Vertical, ..
                    }) => FormaCursorWin::RedimNS,
                    Some(Arrastre::Asa {
                        eje: Eje::Horizontal,
                        ..
                    }) => FormaCursorWin::RedimEO,
                    Some(Arrastre::Nuevo { .. } | Arrastre::Mover { movido: true, .. }) => {
                        FormaCursorWin::Mover
                    }
                    _ => match pindesk_ui::chat::borde_en(
                        l,
                        marco.ancho,
                        marco.alto,
                        self.escala_por_cien,
                    ) {
                        Some(
                            pindesk_ui::chat::Borde::Izquierda | pindesk_ui::chat::Borde::Derecha,
                        ) => FormaCursorWin::RedimEO,
                        Some(pindesk_ui::chat::Borde::Arriba | pindesk_ui::chat::Borde::Abajo) => {
                            FormaCursorWin::RedimNS
                        }
                        Some(
                            pindesk_ui::chat::Borde::ArribaIzquierda
                            | pindesk_ui::chat::Borde::AbajoDerecha,
                        ) => FormaCursorWin::RedimNoSe,
                        Some(_) => FormaCursorWin::RedimNeSo,
                        None => match self.asa_bajo(l) {
                            Some(Eje::Vertical) => FormaCursorWin::RedimNS,
                            Some(Eje::Horizontal) => FormaCursorWin::RedimEO,
                            None => FormaCursorWin::Flecha,
                        },
                    },
                };
                ventana.poner_cursor(cursor);
                self.arrastre.is_some()
                    || self.zona_en(antes) != self.zona_en(l)
                    || self.sobre_lienzo(l)
            }
            EventoOverlay::BotonPulsado(p) => {
                let l = local(p, marco);
                self.raton = l;
                self.pulsar(l, &*ventana, marco, vivo);
                true
            }
            EventoOverlay::BotonSoltado(p) => {
                let l = local(p, marco);
                self.soltar(l);
                ventana.soltar_raton();
                true
            }
            EventoOverlay::BotonDerechoPulsado(p) => {
                // Atras en una opcion o un numero.
                let l = local(p, marco);
                if let Some(Zona::Prop(c, 0)) = self.zona_en(l) {
                    self.tocar_prop(c, -1);
                }
                true
            }
            EventoOverlay::Rueda(d) => {
                self.rueda(d, false, pindesk_shell::entrada::tecla_pulsada_ahora(0x11))
            }
            EventoOverlay::RuedaHorizontal(d) => self.rueda(d, true, false),
            EventoOverlay::RuedaFina(r) => self.rueda(r.delta, r.horizontal, r.ctrl),
            EventoOverlay::DeslizTactil(d) => {
                if self.sobre_lienzo(self.raton) {
                    self.llevar(d.dx(), d.dy());
                    true
                } else {
                    false
                }
            }
            EventoOverlay::PellizcoTactil(pz) => {
                let l = local(pz.en, marco);
                self.acercar(pz.factor(), (l.x as f32, l.y as f32));
                true
            }
            EventoOverlay::FicherosSoltados => {
                if let Some(r) = pindesk_shell::overlay::ficheros_soltados()
                    .into_iter()
                    .find(|r| r.extension().and_then(|e| e.to_str()) == Some(formato::EXTENSION))
                    && self.puede_dejar()
                {
                    self.abrir(&r);
                }
                true
            }
            EventoOverlay::Caracter(c) => {
                if let Some(f) = self.foco.as_mut()
                    && !c.is_control()
                    && f.texto.chars().count() < 600
                {
                    f.texto.push(c);
                    return true;
                }
                false
            }
            EventoOverlay::Tecla {
                vk,
                ctrl,
                shift,
                alt,
            } => self.tecla(vk, ctrl, shift, alt, vivo),
            EventoOverlay::CambioDpi | EventoOverlay::Pintar | EventoOverlay::Despierta => true,
            _ => false,
        }
    }

    fn rueda(&mut self, delta: i32, horizontal: bool, ctrl: bool) -> bool {
        let muescas = delta as f32 / 120.0;
        match self.zona_en(self.raton) {
            Some(Zona::Lienzo) | Some(Zona::Tarjeta(_)) if !self.galeria || ctrl => {
                if ctrl && !horizontal {
                    let p = (self.raton.x as f32, self.raton.y as f32);
                    self.acercar(1.12f32.powf(muescas), p);
                } else if horizontal {
                    self.llevar(-muescas * self.l(60.0), 0.0);
                } else {
                    self.llevar(0.0, muescas * self.l(60.0));
                }
                true
            }
            Some(Zona::Pieza(_) | Zona::Paleta) => {
                self.scroll_paleta = (self.scroll_paleta - muescas * self.l(60.0)).max(0.0);
                true
            }
            Some(Zona::Panel | Zona::Prop(..) | Zona::Doc(_) | Zona::Campo(_)) => {
                self.scroll_panel = (self.scroll_panel - muescas * self.l(60.0)).max(0.0);
                true
            }
            _ => false,
        }
    }

    fn tecla(&mut self, vk: u32, ctrl: bool, shift: bool, alt: bool, vivo: &mut bool) -> bool {
        // Escribiendo: Intro pone, Escape deja, Retroceso borra.
        if self.foco.is_some() {
            match vk {
                VK_INTRO => self.confirmar(true),
                VK_ESCAPE => self.foco = None,
                VK_RETROCESO => {
                    if let Some(f) = self.foco.as_mut() {
                        f.texto.pop();
                    }
                }
                _ => return false,
            }
            return true;
        }
        let sel = self.ed.seleccion;
        match (vk, ctrl, shift) {
            (0x5A, true, false) => self.orden(Orden::Deshacer, vivo),
            (0x5A, true, true) | (0x59, true, _) => self.orden(Orden::Rehacer, vivo),
            (0x53, true, _) => {
                self.guardar(shift);
            }
            (0x4F, true, _) => self.orden(Orden::Abrir, vivo),
            (0x4E, true, _) => self.orden(Orden::Nuevo, vivo),
            (0x45, true, _) => self.exportar("pdf"),
            (0x44, true, _) => self.orden(Orden::Duplicar, vivo),
            (0xBB | 0x6B, true, _) => {
                let c = self.centro_lienzo();
                self.acercar(1.2, c);
            }
            (0xBD | 0x6D, true, _) => {
                let c = self.centro_lienzo();
                self.acercar(1.0 / 1.2, c);
            }
            (0x30 | 0x60, true, _) => self.encajar(),
            (VK_SUPR, ..) if sel.is_some() => self.orden(Orden::Borrar, vivo),
            (VK_ARRIBA, false, _) if alt => self.orden(Orden::Subir, vivo),
            (VK_ABAJO, false, _) if alt => self.orden(Orden::Bajar, vivo),
            (VK_REPAG, ..) => self.orden(Orden::PaginaAntes, vivo),
            (VK_AVPAG, ..) => self.orden(Orden::PaginaDespues, vivo),
            (VK_ESCAPE, ..) => {
                if self.arrastre.take().is_some() {
                    self.guia = None;
                } else if self.menu_exportar {
                    self.menu_exportar = false;
                } else if self.propuesta.is_some() {
                    self.propuesta = None;
                } else if self.galeria {
                    self.galeria = false;
                } else {
                    self.ed.seleccion = None;
                }
            }
            _ => return false,
        }
        true
    }

    fn centro_lienzo(&self) -> (f32, f32) {
        let l = self.lienzo();
        (l.x + l.ancho / 2.0, l.y + l.alto / 2.0)
    }

    /// El asa del elegido bajo el raton, si la hay.
    fn asa_bajo(&self, l: Punto) -> Option<Eje> {
        let id = self.ed.seleccion?;
        if self.propuesta.is_some() || self.galeria || !self.sobre_lienzo(l) {
            return None;
        }
        let m = self.maqueta.as_ref()?;
        let (x, y) = self.mm(l);
        // El asa tiene unos pixeles de holgura a cualquier aumento.
        let holgura = (self.l(5.0) / self.vista.escala).max(1.0);
        [
            (0.0, 0.0),
            (holgura, 0.0),
            (-holgura, 0.0),
            (0.0, holgura),
            (0.0, -holgura),
        ]
        .iter()
        .find_map(|(dx, dy)| asa_en(&m.pagina, id, x + dx, y + dy))
    }

    fn pulsar(&mut self, l: Punto, ventana: &VentanaOverlay, marco: &Rect, vivo: &mut bool) {
        // Los bordes de la ventana la redimensionan.
        if let Some(b) =
            pindesk_ui::chat::borde_en(l, marco.ancho, marco.alto, self.escala_por_cien)
        {
            self.arrastre = Some(Arrastre::Borde {
                borde: b,
                marco: *marco,
            });
            ventana.capturar_raton();
            return;
        }
        let zona = self.zona_en(l);
        // Lo que se escribia se pone al pulsar en otro sitio.
        if !matches!(zona, Some(Zona::Campo(_))) {
            self.confirmar(false);
        }
        if !matches!(
            zona,
            Some(Zona::Boton(Orden::MenuExportar | Orden::Exportar(_)))
        ) {
            self.menu_exportar = false;
        }
        match zona {
            Some(Zona::Boton(o)) => self.orden(o, vivo),
            Some(Zona::Pieza(pieza)) => {
                self.arrastre = Some(Arrastre::Nuevo {
                    pieza,
                    movido: false,
                });
                ventana.capturar_raton();
            }
            Some(Zona::Campo(o)) => {
                if self.foco.as_ref().map(|f| f.objetivo) != Some(o) {
                    self.confirmar(false);
                    self.empezar_a_escribir(o);
                }
            }
            Some(Zona::Prop(c, paso)) => self.tocar_prop(c, paso),
            Some(Zona::Doc(c)) => self.tocar_doc(c, 1),
            Some(Zona::Tarjeta(i)) => {
                let d = self
                    .tarjetas
                    .get(i)
                    .map(|(d, _)| d.clone())
                    .unwrap_or_else(|| {
                        let mut d = Documento::nuevo(&self.t.t("plt-sin-titulo"));
                        d.idioma = self.t.etiqueta().to_string();
                        d
                    });
                if self.puede_dejar() {
                    self.poner_doc(d, None);
                }
            }
            Some(Zona::Lienzo) if self.propuesta.is_none() && !self.galeria => {
                if let Some(eje) = self.asa_bajo(l)
                    && let Some(id) = self.ed.seleccion
                {
                    let inicio = self
                        .maqueta
                        .as_ref()
                        .and_then(|m| m.pagina.caja_de(id))
                        .map(|c| {
                            if eje == Eje::Vertical {
                                c.caja.y
                            } else {
                                c.caja.x
                            }
                        })
                        .unwrap_or(0.0);
                    self.ed.paso();
                    self.arrastre = Some(Arrastre::Asa { id, eje, inicio });
                    ventana.capturar_raton();
                    return;
                }
                let (x, y) = self.mm(l);
                let bajo = self
                    .maqueta
                    .as_ref()
                    .and_then(|m| m.pagina.bloque_en(x, y))
                    .map(|c| c.id);
                match bajo {
                    Some(id) => {
                        self.ed.seleccion = Some(id);
                        self.scroll_panel = 0.0;
                        if id != self.ed.doc.raiz.id {
                            self.arrastre = Some(Arrastre::Mover {
                                id,
                                desde: l,
                                movido: false,
                            });
                        }
                    }
                    None => {
                        self.ed.seleccion = None;
                        self.arrastre = Some(Arrastre::Llevar {
                            desde: l,
                            origen: (self.vista.ox, self.vista.oy),
                        });
                    }
                }
                ventana.capturar_raton();
            }
            _ => {}
        }
    }

    fn mover_raton(
        &mut self,
        l: Punto,
        p: Punto,
        ventana: &mut VentanaOverlay,
        marco: &mut Rect,
        superficie: &Superficie,
    ) {
        match self.arrastre {
            Some(Arrastre::Borde { borde, marco: m0 }) => {
                let nuevo = pindesk_ui::chat::redimensionar(m0, borde, p, self.escala_por_cien);
                if nuevo != *marco {
                    *marco = nuevo;
                    ventana.mover(nuevo);
                    let _ = superficie.redimensionar(nuevo.ancho, nuevo.alto);
                    self.ancho = nuevo.ancho as f32;
                    self.alto = nuevo.alto as f32;
                    if self.encajada {
                        self.encajar();
                    }
                }
            }
            Some(Arrastre::Llevar { desde, origen }) => {
                self.vista.ox = origen.0 + (l.x - desde.x) as f32;
                self.vista.oy = origen.1 + (l.y - desde.y) as f32;
                self.encajada = false;
            }
            Some(Arrastre::Asa { id, eje, inicio }) => {
                let (x, y) = self.mm(l);
                let mm = if eje == Eje::Vertical {
                    y - inicio
                } else {
                    x - inicio
                };
                self.ed.redimensionar_sin_paso(id, (mm / 1.0).round());
                self.tocado();
                self.maquetar();
            }
            Some(Arrastre::Nuevo { pieza, .. }) => {
                self.arrastre = Some(Arrastre::Nuevo {
                    pieza,
                    movido: true,
                });
                self.guia = self.guia_en(l, None);
            }
            Some(Arrastre::Mover { id, desde, movido }) => {
                let lejos = (l.x - desde.x).abs() + (l.y - desde.y).abs() > self.l(5.0) as i32;
                if movido || lejos {
                    self.arrastre = Some(Arrastre::Mover {
                        id,
                        desde,
                        movido: true,
                    });
                    self.guia = self.guia_en(l, Some(id));
                }
            }
            None => {}
        }
    }

    fn guia_en(&self, l: Punto, arrastrado: Option<u64>) -> Option<(Destino, Guia)> {
        if !self.sobre_lienzo(l) || self.galeria || self.propuesta.is_some() {
            return None;
        }
        let m = self.maqueta.as_ref()?;
        let (x, y) = self.mm(l);
        destino_en(&m.pagina, &self.ed.doc, x, y, arrastrado)
    }

    fn soltar(&mut self, l: Punto) {
        let guia = self.guia.take();
        match self.arrastre.take() {
            Some(Arrastre::Nuevo { pieza, movido }) => {
                let Some(b) = paleta::nuevo(pieza, &self.t) else {
                    return;
                };
                if movido {
                    if let Some((d, _)) = guia.or_else(|| self.guia_en(l, None)) {
                        self.ed.insertar(b, d);
                        self.tocado();
                    }
                } else {
                    // Un clic en la pieza: al final del contenedor elegido, o
                    // de la pagina.
                    let padre = self
                        .ed
                        .seleccion
                        .and_then(|s| self.ed.doc.buscar(s))
                        .filter(|b| b.tipo.es_contenedor())
                        .map_or(self.ed.doc.raiz.id, |b| b.id);
                    let n = self
                        .ed
                        .doc
                        .buscar(padre)
                        .and_then(|p| p.tipo.hijos())
                        .map_or(0, Vec::len);
                    self.ed.insertar(b, Destino::Dentro { padre, indice: n });
                    self.galeria = false;
                    self.propuesta = None;
                    self.tocado();
                }
            }
            Some(Arrastre::Mover {
                id, movido: true, ..
            }) => {
                if let Some((d, _)) = guia
                    && self.ed.mover(id, d)
                {
                    self.tocado();
                }
            }
            _ => {}
        }
    }

    // --- Pintar -------------------------------------------------------------------

    fn zona(&self, r: RectF, z: Zona) {
        self.zonas.borrow_mut().push((r, z));
    }

    fn sobre(&self, r: RectF) -> bool {
        let (x, y) = (self.raton.x as f32, self.raton.y as f32);
        x >= r.x && x < r.x + r.ancho && y >= r.y && y < r.y + r.alto
    }

    /// Una pastilla con su texto. Devuelve su caja.
    #[allow(clippy::too_many_arguments)]
    fn pastilla(
        &self,
        p: &Pintor,
        x: f32,
        y: f32,
        texto: &str,
        fuerte: bool,
        z: Zona,
        c: &Colores,
    ) -> RectF {
        let tam = self.l(13.0);
        let (w, _) = p.medir_texto(texto, tam);
        let r = RectF {
            x,
            y,
            ancho: w + self.l(26.0),
            alto: self.l(PASTILLA),
        };
        let fondo = match (fuerte, self.sobre(r)) {
            (true, false) => c.acento,
            (true, true) => con_alfa(c.acento, 0.85),
            (false, true) => c.pastilla_sobre,
            (false, false) => c.pastilla,
        };
        p.rellenar_redondeado(r, r.alto / 2.0, fondo);
        let color = if fuerte { c.texto_acento } else { c.texto };
        let (_, h) = p.medir_texto(texto, tam);
        p.texto(
            texto,
            r.x + self.l(13.0),
            r.y + (r.alto - h) / 2.0,
            tam,
            color,
        );
        self.zona(r, z);
        r
    }

    fn pintar_todo(&self, p: &Pintor) {
        let c = self.colores();
        self.zonas.borrow_mut().clear();
        let todo = RectF {
            x: 0.0,
            y: 0.0,
            ancho: self.ancho,
            alto: self.alto,
        };
        p.rellenar(todo, c.lienzo);
        let lienzo = self.lienzo();
        self.zona(lienzo, Zona::Lienzo);
        self.pintar_lienzo(p, &c, lienzo);
        self.pintar_paleta(p, &c);
        self.pintar_panel(p, &c);
        self.pintar_barra(p, &c);
        if self.galeria {
            self.pintar_galeria(p, &c, lienzo);
        }
        if let Some((s, _)) = &self.aviso {
            let tam = self.l(13.0);
            let (w, h) = p.medir_texto(s, tam);
            let r = RectF {
                x: lienzo.x + (lienzo.ancho - w - self.l(32.0)) / 2.0,
                y: self.alto - self.l(64.0),
                ancho: w + self.l(32.0),
                alto: h + self.l(18.0),
            };
            p.rellenar_redondeado(r, r.alto / 2.0, con_alfa(hex(0x1d2733), 0.94));
            p.texto(s, r.x + self.l(16.0), r.y + self.l(9.0), tam, hex(0xffffff));
        }
        // El fantasma de la pieza que se arrastra.
        if let Some(Arrastre::Nuevo {
            pieza,
            movido: true,
        }) = self.arrastre
        {
            let s = self.t.t(&format!("plt-pieza-{pieza}"));
            let x = self.raton.x as f32 + self.l(14.0);
            let y = self.raton.y as f32 + self.l(10.0);
            self.pastilla(p, x, y, &s, true, Zona::Lienzo, &c);
        }
    }

    fn pintar_barra(&self, p: &Pintor, c: &Colores) {
        let barra = RectF {
            x: 0.0,
            y: 0.0,
            ancho: self.ancho,
            alto: self.l(BARRA),
        };
        p.rellenar(barra, c.barra);
        p.rellenar(
            RectF {
                y: barra.alto - 1.0,
                alto: 1.0,
                ..barra
            },
            c.separador,
        );
        self.zona(barra, Zona::Barra);
        let y = (barra.alto - self.l(PASTILLA)) / 2.0;
        // A la izquierda, el producto y el nombre (se escribe al pulsar).
        let tam = self.l(15.0);
        let producto = self.t.t("plt-producto");
        p.texto(&producto, self.l(MARGEN), y + self.l(5.0), tam, c.texto);
        let (wp, _) = p.medir_texto(&producto, tam);
        let x_nombre = self.l(MARGEN) + wp + self.l(14.0);
        let nombre = match &self.foco {
            Some(f) if f.objetivo == Objetivo::Nombre => format!("{}|", f.texto),
            _ => format!(
                "{}{}",
                self.titulo_fichero(),
                if self.ed.cambiado { " •" } else { "" }
            ),
        };
        let r_nombre = RectF {
            x: x_nombre,
            y,
            ancho: self.l(220.0),
            alto: self.l(PASTILLA),
        };
        if self.sobre(r_nombre) || matches!(&self.foco, Some(f) if f.objetivo == Objetivo::Nombre) {
            p.rellenar_redondeado(r_nombre, self.l(8.0), c.campo);
        }
        p.texto_linea(
            &nombre,
            x_nombre + self.l(8.0),
            y + self.l(6.0),
            self.l(13.0),
            r_nombre.ancho - self.l(16.0),
            c.apagado,
        );
        self.zona(r_nombre, Zona::Campo(Objetivo::Nombre));
        // A la derecha, de derecha a izquierda.
        let mut x = self.ancho - self.l(MARGEN);
        let cerrar = RectF {
            x: x - self.l(PASTILLA),
            y,
            ancho: self.l(PASTILLA),
            alto: self.l(PASTILLA),
        };
        p.rellenar_redondeado(
            cerrar,
            cerrar.alto / 2.0,
            if self.sobre(cerrar) {
                c.peligro
            } else {
                c.pastilla
            },
        );
        let cx = cerrar.x + cerrar.ancho / 2.0;
        let cy = cerrar.y + cerrar.alto / 2.0;
        let k = self.l(5.0);
        let color_x = if self.sobre(cerrar) {
            hex(0xffffff)
        } else {
            c.texto
        };
        p.linea((cx - k, cy - k), (cx + k, cy + k), self.l(1.6), color_x);
        p.linea((cx - k, cy + k), (cx + k, cy - k), self.l(1.6), color_x);
        self.zona(cerrar, Zona::Boton(Orden::Cerrar));
        x = cerrar.x - self.l(8.0);
        // El ☀/☾.
        let dn = RectF {
            x: x - self.l(PASTILLA),
            ..cerrar
        };
        p.rellenar_redondeado(
            dn,
            dn.alto / 2.0,
            if self.sobre(dn) {
                c.pastilla_sobre
            } else {
                c.pastilla
            },
        );
        let (dcx, dcy) = (dn.x + dn.ancho / 2.0, dn.y + dn.alto / 2.0);
        if self.noche {
            p.circulo((dcx, dcy), self.l(7.0), c.texto);
            p.circulo(
                (dcx + self.l(3.5), dcy - self.l(2.5)),
                self.l(6.0),
                if self.sobre(dn) {
                    c.pastilla_sobre
                } else {
                    c.pastilla
                },
            );
        } else {
            p.circulo((dcx, dcy), self.l(4.5), c.texto);
            for i in 0..8 {
                let a = (i as f32 * 45.0f32).to_radians();
                let (r0, r1) = (self.l(6.5), self.l(9.0));
                p.linea(
                    (dcx + r0 * a.cos(), dcy + r0 * a.sin()),
                    (dcx + r1 * a.cos(), dcy + r1 * a.sin()),
                    self.l(1.5),
                    c.texto,
                );
            }
        }
        self.zona(dn, Zona::Boton(Orden::DiaNoche));
        x = dn.x - self.l(12.0);
        let botones: [(&str, Orden, bool); 4] = [
            ("plt-enviar-pindesk", Orden::Enviar, true),
            ("plt-exportar", Orden::MenuExportar, false),
            ("plt-guardar", Orden::Guardar, false),
            ("plt-abrir", Orden::Abrir, false),
        ];
        let mut r_exportar = None;
        for (clave, o, fuerte) in botones {
            let s = self.t.t(clave);
            let (w, _) = p.medir_texto(&s, self.l(13.0));
            x -= w + self.l(26.0);
            let r = self.pastilla(p, x, y, &s, fuerte, Zona::Boton(o), c);
            if o == Orden::MenuExportar {
                r_exportar = Some(r);
            }
            x -= self.l(8.0);
        }
        // En medio-izquierda: galeria, nueva, deshacer y rehacer.
        let mut xi = x_nombre + r_nombre.ancho + self.l(12.0);
        for (clave, o) in [
            ("plt-galeria", Orden::Galeria),
            ("plt-nuevo", Orden::Nuevo),
            ("plt-deshacer", Orden::Deshacer),
            ("plt-rehacer", Orden::Rehacer),
        ] {
            let s = match o {
                Orden::Deshacer => "↶".to_string(),
                Orden::Rehacer => "↷".to_string(),
                _ => self.t.t(clave),
            };
            let (w, _) = p.medir_texto(&s, self.l(13.0));
            if xi + w + self.l(26.0) > x {
                break;
            }
            let r = self.pastilla(
                p,
                xi,
                y,
                &s,
                o == Orden::Galeria && self.galeria,
                Zona::Boton(o),
                c,
            );
            xi = r.x + r.ancho + self.l(8.0);
        }
        // El menu de exportar, debajo de su boton.
        if self.menu_exportar
            && let Some(r) = r_exportar
        {
            let opciones = [
                ("plt-exportar-pdf", "pdf"),
                ("plt-exportar-png", "png"),
                ("plt-exportar-svg", "svg"),
                ("plt-exportar-lienzo", "excalidraw"),
            ];
            let ancho = self.l(210.0);
            let menu = RectF {
                x: (r.x + r.ancho - ancho).max(0.0),
                y: barra.alto + self.l(4.0),
                ancho,
                alto: self.l(FILA) * opciones.len() as f32 + self.l(12.0),
            };
            p.rellenar_redondeado(menu, self.l(12.0), c.panel);
            p.polilinea(
                &pintar::contorno_redondo(menu, self.l(12.0)),
                1.0,
                c.separador,
            );
            for (i, (clave, f)) in opciones.iter().enumerate() {
                let fila = RectF {
                    x: menu.x + self.l(6.0),
                    y: menu.y + self.l(6.0) + self.l(FILA) * i as f32,
                    ancho: menu.ancho - self.l(12.0),
                    alto: self.l(FILA),
                };
                if self.sobre(fila) {
                    p.rellenar_redondeado(fila, self.l(8.0), c.campo);
                }
                p.texto(
                    &self.t.t(clave),
                    fila.x + self.l(10.0),
                    fila.y + self.l(7.0),
                    self.l(13.0),
                    c.texto,
                );
                self.zona(fila, Zona::Boton(Orden::Exportar(f)));
            }
        }
    }

    fn pintar_paleta(&self, p: &Pintor, c: &Colores) {
        let r = RectF {
            x: 0.0,
            y: self.l(BARRA),
            ancho: self.l(PALETA),
            alto: self.alto - self.l(BARRA),
        };
        p.rellenar(r, c.panel);
        p.rellenar(
            RectF {
                x: r.ancho - 1.0,
                ancho: 1.0,
                ..r
            },
            c.separador,
        );
        self.zona(r, Zona::Paleta);
        p.con_recorte(r, |p| {
            let mut y = r.y + self.l(12.0) - self.scroll_paleta;
            p.texto(
                &self.t.t("plt-bloques"),
                self.l(MARGEN),
                y,
                self.l(16.0),
                c.texto,
            );
            y += self.l(24.0);
            p.texto(
                &self.t.t("plt-arrastra"),
                self.l(MARGEN),
                y,
                self.l(11.5),
                c.apagado,
            );
            y += self.l(26.0);
            for (grupo, piezas) in paleta::PALETA {
                p.texto(
                    &self.t.t(&format!("plt-grupo-{grupo}")).to_uppercase(),
                    self.l(MARGEN),
                    y,
                    self.l(10.5),
                    c.apagado,
                );
                y += self.l(20.0);
                for pieza in piezas {
                    let fila = RectF {
                        x: self.l(8.0),
                        y,
                        ancho: r.ancho - self.l(16.0),
                        alto: self.l(FILA - 2.0),
                    };
                    if self.sobre(fila) {
                        p.rellenar_redondeado(fila, self.l(8.0), c.campo);
                    }
                    p.circulo(
                        (fila.x + self.l(12.0), fila.y + fila.alto / 2.0),
                        self.l(4.0),
                        color_de_grupo(grupo),
                    );
                    p.texto_linea(
                        &self.t.t(&format!("plt-pieza-{pieza}")),
                        fila.x + self.l(24.0),
                        fila.y + self.l(6.0),
                        self.l(13.0),
                        fila.ancho - self.l(28.0),
                        c.texto,
                    );
                    if fila.y + fila.alto > r.y && fila.y < r.y + r.alto {
                        self.zona(fila, Zona::Pieza(pieza));
                    }
                    y += self.l(FILA);
                }
                y += self.l(10.0);
            }
        });
    }

    fn pintar_lienzo(&self, p: &Pintor, c: &Colores, l: RectF) {
        p.con_recorte(l, |p| {
            let (m, es_propuesta) = match &self.propuesta {
                Some(pr) => (Some(&pr.maqueta), true),
                None => (self.maqueta.as_ref(), false),
            };
            let Some(m) = m else { return };
            let mut v = self.vista;
            if es_propuesta {
                // La propuesta, encajada en el lienzo.
                let (w, h) = (m.pagina.ancho, m.pagina.alto);
                let s = ((l.ancho - self.l(72.0)) / w)
                    .min((l.alto - self.l(130.0)) / h)
                    .max(0.2);
                v = Vista {
                    ox: l.x + (l.ancho - w * s) / 2.0,
                    oy: l.y + self.l(70.0),
                    escala: s,
                };
            }
            pintar::pagina(p, m, v, true);
            if !es_propuesta {
                self.pintar_seleccion(p, m, v, c);
            }
            // La guia de soltar.
            if let Some((_, g)) = &self.guia {
                let azul = hex(0x2f7cf6);
                match g {
                    Guia::Raya { a, b } => {
                        p.linea(v.p(*a), v.p(*b), self.l(3.0), azul);
                        p.circulo(v.p(*a), self.l(4.0), azul);
                        p.circulo(v.p(*b), self.l(4.0), azul);
                    }
                    Guia::Zona(caja) => {
                        let r = v.caja(*caja);
                        p.rellenar_redondeado(r, self.l(6.0), con_alfa(azul, 0.12));
                        p.trazar_discontinuo(r, self.l(2.0), azul);
                    }
                }
            }
            // Abajo: la pagina y el aumento.
            let y = l.y + l.alto - self.l(44.0);
            let mut x = l.x + self.l(16.0);
            if !es_propuesta && self.paginas > 1 {
                let r = self.pastilla(p, x, y, "‹", false, Zona::Boton(Orden::PaginaAntes), c);
                x = r.x + r.ancho + self.l(6.0);
                let s = self.t.con(
                    "plt-pagina-de",
                    &[
                        ("n", (self.pagina + 1).to_string()),
                        ("total", self.paginas.to_string()),
                    ],
                );
                let r = self.pastilla(p, x, y, &s, false, Zona::Lienzo, c);
                x = r.x + r.ancho + self.l(6.0);
                let r = self.pastilla(p, x, y, "›", false, Zona::Boton(Orden::PaginaDespues), c);
                x = r.x + r.ancho + self.l(6.0);
            }
            if !es_propuesta {
                let zoom = format!(
                    "{} %",
                    (self.vista.escala / self.e * 25.4 / 96.0 * 100.0).round()
                );
                let r = self.pastilla(p, x, y, &zoom, false, Zona::Boton(Orden::Encajar), c);
                let _ = r;
            }
            // La propuesta de la IA, con su barra de aplicar.
            if es_propuesta {
                let barra = RectF {
                    x: l.x + self.l(16.0),
                    y: l.y + self.l(14.0),
                    ancho: l.ancho - self.l(32.0),
                    alto: self.l(44.0),
                };
                p.rellenar_redondeado(barra, self.l(14.0), c.panel);
                p.texto(
                    &format!("✦ {}", self.t.t("plt-ia-vista-previa")),
                    barra.x + self.l(16.0),
                    barra.y + self.l(13.0),
                    self.l(14.0),
                    c.texto,
                );
                let s_ap = self.t.t("plt-ia-aplicar");
                let s_de = self.t.t("plt-ia-descartar");
                let (w1, _) = p.medir_texto(&s_ap, self.l(13.0));
                let (w2, _) = p.medir_texto(&s_de, self.l(13.0));
                let x1 = barra.x + barra.ancho - self.l(8.0) - w1 - self.l(26.0);
                let yb = barra.y + (barra.alto - self.l(PASTILLA)) / 2.0;
                self.pastilla(p, x1, yb, &s_ap, true, Zona::Boton(Orden::IaAplicar), c);
                self.pastilla(
                    p,
                    x1 - self.l(8.0) - w2 - self.l(26.0),
                    yb,
                    &s_de,
                    false,
                    Zona::Boton(Orden::IaDescartar),
                    c,
                );
            }
        });
    }

    fn pintar_seleccion(&self, p: &Pintor, m: &Maqueta, v: Vista, _c: &Colores) {
        let azul = hex(0x2f7cf6);
        // Lo de debajo del raton, suave.
        if self.arrastre.is_none() && self.sobre_lienzo(self.raton) {
            let (x, y) = self.mm(self.raton);
            if let Some(b) = m.pagina.bloque_en(x, y)
                && Some(b.id) != self.ed.seleccion
            {
                p.trazar(v.caja(b.caja), 1.0, con_alfa(azul, 0.45));
            }
        }
        let Some(id) = self.ed.seleccion else { return };
        let Some(cb) = m.pagina.caja_de(id) else {
            return;
        };
        let r = v.caja(cb.caja);
        p.trazar(r, self.l(1.6), azul);
        // El rotulo de lo elegido.
        let tipo = self.ed.doc.buscar(id).map(|b| b.tipo.clave()).unwrap_or("");
        let s = self.t.t(&format!("plt-pieza-{tipo}"));
        let tam = self.l(11.0);
        let (w, h) = p.medir_texto(&s, tam);
        let et = RectF {
            x: r.x,
            y: r.y - h - self.l(6.0),
            ancho: w + self.l(12.0),
            alto: h + self.l(5.0),
        };
        p.rellenar_redondeado(et, self.l(5.0), azul);
        p.texto(
            &s,
            et.x + self.l(6.0),
            et.y + self.l(2.5),
            tam,
            hex(0xffffff),
        );
        // El asa del tamaño, en su borde de detras.
        if let Some(padre) = cb.padre.and_then(|pid| m.pagina.caja_de(pid))
            && let Some(eje) = padre.eje
        {
            let (cx, cy, w, h) = match eje {
                Eje::Vertical => (r.x + r.ancho / 2.0, r.y + r.alto, self.l(28.0), self.l(7.0)),
                Eje::Horizontal => (r.x + r.ancho, r.y + r.alto / 2.0, self.l(7.0), self.l(28.0)),
            };
            let asa = RectF {
                x: cx - w / 2.0,
                y: cy - h / 2.0,
                ancho: w,
                alto: h,
            };
            p.rellenar_redondeado(asa, w.min(h) / 2.0, hex(0xffffff));
            p.polilinea(
                &pintar::contorno_redondo(asa, w.min(h) / 2.0),
                self.l(1.4),
                azul,
            );
        }
    }

    fn pintar_galeria(&self, p: &Pintor, c: &Colores, l: RectF) {
        p.rellenar(l, con_alfa(c.lienzo, 0.97));
        p.con_recorte(l, |p| {
            let mut y = l.y + self.l(24.0);
            p.texto(
                &self.t.t("plt-galeria-titulo"),
                l.x + self.l(28.0),
                y,
                self.l(22.0),
                c.texto,
            );
            let s_cerrar = self.t.t("plt-cerrar");
            let (wc, _) = p.medir_texto(&s_cerrar, self.l(13.0));
            self.pastilla(
                p,
                l.x + l.ancho - self.l(28.0) - wc - self.l(26.0),
                y,
                &s_cerrar,
                false,
                Zona::Boton(Orden::CerrarGaleria),
                c,
            );
            y += self.l(46.0);
            let ancho_t = self.l(170.0);
            let alto_t = self.l(230.0);
            let hueco = self.l(18.0);
            let por_fila = (((l.ancho - self.l(56.0)) + hueco) / (ancho_t + hueco))
                .floor()
                .max(1.0) as usize;
            let total = self.tarjetas.len() + 1;
            for i in 0..total {
                let (col, fila) = (i % por_fila, i / por_fila);
                let r = RectF {
                    x: l.x + self.l(28.0) + (ancho_t + hueco) * col as f32,
                    y: y + (alto_t + hueco) * fila as f32,
                    ancho: ancho_t,
                    alto: alto_t,
                };
                let encima = self.sobre(r);
                p.rellenar_redondeado(
                    r,
                    self.l(14.0),
                    if encima { c.pastilla_sobre } else { c.panel },
                );
                let dibujo = RectF {
                    x: r.x + self.l(12.0),
                    y: r.y + self.l(12.0),
                    ancho: r.ancho - self.l(24.0),
                    alto: r.alto - self.l(52.0),
                };
                let nombre = if i < self.tarjetas.len() {
                    let (d, m) = &self.tarjetas[i];
                    let (w, h) = (m.pagina.ancho, m.pagina.alto);
                    let s = (dibujo.ancho / w).min(dibujo.alto / h);
                    let v = Vista {
                        ox: dibujo.x + (dibujo.ancho - w * s) / 2.0,
                        oy: dibujo.y + (dibujo.alto - h * s) / 2.0,
                        escala: s,
                    };
                    pintar::pagina(p, m, v, true);
                    self.zona(r, Zona::Tarjeta(i));
                    d.nombre.clone()
                } else {
                    // En blanco.
                    let hoja = RectF {
                        x: dibujo.x + dibujo.ancho * 0.18,
                        y: dibujo.y + self.l(6.0),
                        ancho: dibujo.ancho * 0.64,
                        alto: dibujo.alto - self.l(12.0),
                    };
                    p.rellenar(hoja, hex(0xffffff));
                    let (cx, cy) = (hoja.x + hoja.ancho / 2.0, hoja.y + hoja.alto / 2.0);
                    p.linea(
                        (cx - self.l(12.0), cy),
                        (cx + self.l(12.0), cy),
                        self.l(2.0),
                        c.acento,
                    );
                    p.linea(
                        (cx, cy - self.l(12.0)),
                        (cx, cy + self.l(12.0)),
                        self.l(2.0),
                        c.acento,
                    );
                    self.zona(r, Zona::Tarjeta(usize::MAX));
                    self.t.t("plt-galeria-vacia")
                };
                p.texto_linea(
                    &nombre,
                    r.x + self.l(12.0),
                    r.y + r.alto - self.l(32.0),
                    self.l(13.0),
                    r.ancho - self.l(24.0),
                    c.texto,
                );
            }
        });
    }

    fn pintar_panel(&self, p: &Pintor, c: &Colores) {
        let r = RectF {
            x: self.ancho - self.l(PANEL),
            y: self.l(BARRA),
            ancho: self.l(PANEL),
            alto: self.alto - self.l(BARRA),
        };
        p.rellenar(r, c.panel);
        p.rellenar(RectF { ancho: 1.0, ..r }, c.separador);
        self.zona(r, Zona::Panel);
        p.con_recorte(r, |p| {
            let x = r.x + self.l(MARGEN);
            let w = r.ancho - self.l(2.0 * MARGEN);
            let mut y = r.y + self.l(14.0) - self.scroll_panel;
            // «Descríbelo».
            p.texto(
                &format!("✦ {}", self.t.t("plt-ia-describir")),
                x,
                y,
                self.l(15.0),
                c.texto,
            );
            y += self.l(26.0);
            y = self.campo_texto(
                p,
                c,
                x,
                y,
                w,
                Objetivo::Describir,
                &self.describir,
                &self.t.t("plt-ia-pista"),
                3,
            );
            let pensando = self.ia_pendiente.is_some();
            let s = if pensando {
                self.t.t("plt-ia-pensando")
            } else {
                self.t.t("plt-ia-crear")
            };
            self.pastilla(p, x, y, &s, true, Zona::Boton(Orden::IaCrear), c);
            y += self.l(PASTILLA + 18.0);
            p.rellenar(
                RectF {
                    x,
                    y,
                    ancho: w,
                    alto: 1.0,
                },
                c.separador,
            );
            y += self.l(14.0);
            match self.ed.seleccion.and_then(|id| self.ed.doc.buscar(id)) {
                Some(b) => self.panel_bloque(p, c, b, x, y, w),
                None => self.panel_documento(p, c, x, y, w),
            }
        });
    }

    /// Un campo de texto de `lineas` renglones. Devuelve la y de debajo.
    #[allow(clippy::too_many_arguments)]
    fn campo_texto(
        &self,
        p: &Pintor,
        c: &Colores,
        x: f32,
        y: f32,
        w: f32,
        o: Objetivo,
        valor: &str,
        pista: &str,
        lineas: u32,
    ) -> f32 {
        let alto = self.l(12.0) + self.l(17.0) * lineas as f32;
        let r = RectF {
            x,
            y,
            ancho: w,
            alto,
        };
        let activo = self.foco.as_ref().is_some_and(|f| f.objetivo == o);
        p.rellenar_redondeado(r, self.l(10.0), c.campo);
        if activo {
            p.polilinea(
                &pintar::contorno_redondo(r, self.l(10.0)),
                self.l(1.5),
                c.acento,
            );
        }
        let (texto, color) = match &self.foco {
            Some(f) if f.objetivo == o => (format!("{}|", f.texto), c.texto),
            _ if valor.is_empty() => (pista.to_string(), c.apagado),
            _ => (valor.to_string(), c.texto),
        };
        p.con_recorte(r, |p| {
            if lineas > 1 {
                p.parrafo(
                    &texto,
                    x + self.l(10.0),
                    y + self.l(6.0),
                    self.l(13.0),
                    w - self.l(20.0),
                    &[],
                    color,
                );
            } else {
                p.texto_linea(
                    &texto,
                    x + self.l(10.0),
                    y + self.l(6.0),
                    self.l(13.0),
                    w - self.l(20.0),
                    color,
                );
            }
        });
        self.zona(r, Zona::Campo(o));
        y + alto + self.l(10.0)
    }

    /// Una fila del panel: el rotulo a la izquierda y el control a la
    /// derecha. Devuelve la caja del control.
    fn fila_panel(&self, p: &Pintor, c: &Colores, x: f32, y: f32, w: f32, rotulo: &str) -> RectF {
        p.texto_linea(
            rotulo,
            x,
            y + self.l(8.0),
            self.l(12.5),
            w * 0.42,
            c.apagado,
        );
        RectF {
            x: x + w * 0.44,
            y,
            ancho: w * 0.56,
            alto: self.l(FILA),
        }
    }

    fn control_opcion(&self, p: &Pintor, c: &Colores, r: RectF, texto: &str, z: Zona) {
        p.rellenar_redondeado(
            r,
            self.l(8.0),
            if self.sobre(r) {
                c.pastilla_sobre
            } else {
                c.campo
            },
        );
        p.texto_linea(
            texto,
            r.x + self.l(10.0),
            r.y + self.l(7.0),
            self.l(12.5),
            r.ancho - self.l(28.0),
            c.texto,
        );
        p.texto(
            "›",
            r.x + r.ancho - self.l(16.0),
            r.y + self.l(6.0),
            self.l(13.0),
            c.apagado,
        );
        self.zona(r, z);
    }

    fn control_si(&self, p: &Pintor, c: &Colores, r: RectF, si: bool, z: Zona) {
        let iw = self.l(42.0);
        let ih = self.l(24.0);
        let i = RectF {
            x: r.x + r.ancho - iw,
            y: r.y + (r.alto - ih) / 2.0,
            ancho: iw,
            alto: ih,
        };
        p.rellenar_redondeado(i, ih / 2.0, if si { hex(0x34c759) } else { c.pastilla });
        let cx = if si {
            i.x + iw - ih / 2.0
        } else {
            i.x + ih / 2.0
        };
        p.circulo((cx, i.y + ih / 2.0), ih / 2.0 - self.l(2.5), hex(0xffffff));
        self.zona(i, z);
    }

    #[allow(clippy::too_many_arguments)]
    fn control_numero(
        &self,
        p: &Pintor,
        c: &Colores,
        r: RectF,
        valor: &str,
        menos: Zona,
        mas: Zona,
        escribir: Option<Zona>,
    ) {
        let b = self.l(FILA);
        let r_menos = RectF { ancho: b, ..r };
        let r_mas = RectF {
            x: r.x + r.ancho - b,
            ancho: b,
            ..r
        };
        let r_valor = RectF {
            x: r.x + b + self.l(4.0),
            ancho: r.ancho - 2.0 * b - self.l(8.0),
            ..r
        };
        for (rr, s, z) in [(r_menos, "−", menos), (r_mas, "+", mas)] {
            p.rellenar_redondeado(
                rr,
                self.l(8.0),
                if self.sobre(rr) {
                    c.pastilla_sobre
                } else {
                    c.pastilla
                },
            );
            let (w, h) = p.medir_texto(s, self.l(15.0));
            p.texto(
                s,
                rr.x + (rr.ancho - w) / 2.0,
                rr.y + (rr.alto - h) / 2.0,
                self.l(15.0),
                c.texto,
            );
            self.zona(rr, z);
        }
        p.rellenar_redondeado(r_valor, self.l(8.0), c.campo);
        let (w, h) = p.medir_texto(valor, self.l(13.0));
        p.texto(
            valor,
            r_valor.x + (r_valor.ancho - w) / 2.0,
            r_valor.y + (r_valor.alto - h) / 2.0,
            self.l(13.0),
            c.texto,
        );
        if let Some(z) = escribir {
            self.zona(r_valor, z);
        }
    }

    fn panel_bloque(&self, p: &Pintor, c: &Colores, b: &Bloque, x: f32, mut y: f32, w: f32) {
        p.texto(
            &self.t.t(&format!("plt-pieza-{}", b.tipo.clave())),
            x,
            y,
            self.l(17.0),
            c.texto,
        );
        y += self.l(32.0);
        // Las acciones, en pastillas que se reparten en renglones.
        let mut xa = x;
        for (clave, o) in [
            ("plt-duplicar", Orden::Duplicar),
            ("plt-borrar", Orden::Borrar),
            ("plt-subir", Orden::Subir),
            ("plt-bajar", Orden::Bajar),
            ("plt-envolver-fila", Orden::EnFila),
            ("plt-envolver-columna", Orden::EnColumna),
        ] {
            let s = self.t.t(clave);
            let (ws, _) = p.medir_texto(&s, self.l(13.0));
            if xa + ws + self.l(26.0) > x + w {
                xa = x;
                y += self.l(PASTILLA + 6.0);
            }
            let r = self.pastilla(p, xa, y, &s, false, Zona::Boton(o), c);
            xa = r.x + r.ancho + self.l(6.0);
        }
        y += self.l(PASTILLA + 16.0);
        p.texto(
            &self.t.t("plt-propiedades").to_uppercase(),
            x,
            y,
            self.l(10.5),
            c.apagado,
        );
        y += self.l(22.0);
        for prop in paleta::de(b) {
            let rotulo = self.t.t(&format!("plt-prop-{}", prop.clave));
            match &prop.valor {
                Valor::Texto(s) => {
                    p.texto_linea(&rotulo, x, y, self.l(12.5), w, c.apagado);
                    y += self.l(20.0);
                    y = self.campo_texto(p, c, x, y, w, Objetivo::Prop(b.id, prop.clave), s, "", 1);
                }
                Valor::Lista(l) => {
                    p.texto_linea(&rotulo, x, y, self.l(12.5), w, c.apagado);
                    y += self.l(20.0);
                    y = self.campo_texto(
                        p,
                        c,
                        x,
                        y,
                        w,
                        Objetivo::Prop(b.id, prop.clave),
                        &l.join(", "),
                        "",
                        2,
                    );
                }
                Valor::Numero { valor, .. } => {
                    let r = self.fila_panel(p, c, x, y, w, &rotulo);
                    let s = match &self.foco {
                        Some(f) if f.objetivo == Objetivo::Prop(b.id, prop.clave) => {
                            format!("{}|", f.texto)
                        }
                        _ => format!("{}", (valor * 100.0).round() / 100.0),
                    };
                    self.control_numero(
                        p,
                        c,
                        r,
                        &s,
                        Zona::Prop(prop.clave, -1),
                        Zona::Prop(prop.clave, 1),
                        Some(Zona::Campo(Objetivo::Prop(b.id, prop.clave))),
                    );
                    y += self.l(FILA + 8.0);
                }
                Valor::Opcion { valor, .. } => {
                    let r = self.fila_panel(p, c, x, y, w, &rotulo);
                    let s = if valor.is_empty() {
                        "—".to_string()
                    } else {
                        self.t.t(&format!("plt-opcion-{valor}"))
                    };
                    self.control_opcion(p, c, r, &s, Zona::Prop(prop.clave, 0));
                    y += self.l(FILA + 8.0);
                }
                Valor::Si(si) => {
                    let r = self.fila_panel(p, c, x, y, w, &rotulo);
                    self.control_si(p, c, r, *si, Zona::Prop(prop.clave, 0));
                    y += self.l(FILA + 8.0);
                }
            }
        }
        // La IA sobre lo elegido.
        y += self.l(8.0);
        p.rellenar(
            RectF {
                x,
                y,
                ancho: w,
                alto: 1.0,
            },
            c.separador,
        );
        y += self.l(14.0);
        p.texto(
            &format!("✦ {}", self.t.t("plt-ia-ajustar")),
            x,
            y,
            self.l(14.0),
            c.texto,
        );
        y += self.l(24.0);
        y = self.campo_texto(
            p,
            c,
            x,
            y,
            w,
            Objetivo::Ajustar,
            &self.ajustar,
            &self.t.t("plt-ia-ajustar-pista"),
            2,
        );
        let r = self.pastilla(
            p,
            x,
            y,
            &self.t.t("plt-ia-ajustar"),
            true,
            Zona::Boton(Orden::IaAjustar),
            c,
        );
        self.pastilla(
            p,
            r.x + r.ancho + self.l(8.0),
            y,
            &self.t.t("plt-ia-sugerir"),
            false,
            Zona::Boton(Orden::IaSugerir),
            c,
        );
    }

    fn panel_documento(&self, p: &Pintor, c: &Colores, x: f32, mut y: f32, w: f32) {
        let d = &self.ed.doc;
        p.texto(&self.t.t("plt-documento"), x, y, self.l(17.0), c.texto);
        y += self.l(28.0);
        p.texto_linea(
            &self.t.t("plt-sin-seleccion"),
            x,
            y,
            self.l(12.0),
            w,
            c.apagado,
        );
        y += self.l(28.0);
        let fila = |y: &mut f32, rotulo: &str| {
            let r = self.fila_panel(p, c, x, *y, w, rotulo);
            *y += self.l(FILA + 8.0);
            r
        };
        let r = fila(&mut y, &self.t.t("plt-papel"));
        self.control_opcion(
            p,
            c,
            r,
            &self.t.t(&format!("plt-papel-{}", d.pagina.tamano.clave())),
            Zona::Doc(DocCampo::Papel),
        );
        let r = fila(&mut y, &self.t.t("plt-horizontal"));
        self.control_si(
            p,
            c,
            r,
            d.pagina.horizontal,
            Zona::Doc(DocCampo::Horizontal),
        );
        let r = fila(&mut y, &self.t.t("plt-margenes"));
        self.control_numero(
            p,
            c,
            r,
            &format!("{}", d.pagina.margenes.arriba.round()),
            Zona::Doc(DocCampo::Margen(-1)),
            Zona::Doc(DocCampo::Margen(1)),
            None,
        );
        let r = fila(&mut y, &self.t.t("plt-tema"));
        self.control_opcion(
            p,
            c,
            r,
            &self.t.t(&format!("plt-tema-{}", d.tema.base)),
            Zona::Doc(DocCampo::Tema),
        );
        // Las muestras de color del tema.
        let mut xm = x;
        for col in [d.tema.acento, d.tema.suave, d.tema.tinta, d.tema.raya] {
            p.circulo(
                (xm + self.l(9.0), y + self.l(6.0)),
                self.l(7.0),
                pintar::color(col),
            );
            xm += self.l(22.0);
        }
        y += self.l(24.0);
        let r = fila(&mut y, &self.t.t("plt-idioma-fechas"));
        let nombre = nucleo::locale::de(&d.idioma).titulo_mes(self.hoy);
        self.control_opcion(
            p,
            c,
            r,
            &format!("{} · {}", d.idioma, nombre),
            Zona::Doc(DocCampo::Idioma),
        );
        let r = fila(&mut y, &self.t.t("plt-repetir"));
        let cada = match d.repeticion.as_ref().map(|r| r.cada) {
            None => "ninguna",
            Some(Cada::Dia) => "dia",
            Some(Cada::Semana) => "semana",
            Some(Cada::Mes) => "mes",
        };
        self.control_opcion(
            p,
            c,
            r,
            &self.t.t(&format!("plt-cada-{cada}")),
            Zona::Doc(DocCampo::Cada),
        );
        if let Some(rep) = &d.repeticion {
            for (o, clave, valor) in [
                (Objetivo::Desde, "plt-desde", &rep.desde),
                (Objetivo::Hasta, "plt-hasta", &rep.hasta),
            ] {
                p.texto_linea(&self.t.t(clave), x, y, self.l(12.5), w, c.apagado);
                y += self.l(20.0);
                y = self.campo_texto(p, c, x, y, w, o, valor, "AAAA-MM-DD", 1);
            }
            let r = fila(&mut y, &self.t.t("plt-sin-finde"));
            self.control_si(
                p,
                c,
                r,
                rep.sin_fines_de_semana,
                Zona::Doc(DocCampo::SinFinde),
            );
            p.texto(
                &self
                    .t
                    .con("plt-paginas", &[("n", self.paginas.to_string())]),
                x,
                y,
                self.l(12.5),
                c.apagado,
            );
        }
    }
}

/// El color de cada grupo de la paleta.
fn color_de_grupo(g: &str) -> Color {
    match g {
        "estructura" => hex(0x8e8e93),
        "texto" => hex(0x3f6db3),
        "listas" => hex(0x2c8a7f),
        "fechas" => hex(0xc0692b),
        _ => hex(0xbf5878),
    }
}
