//! El panel «Pines abiertos» (rediseno v2): la lista de los pines que hay,
//! agrupados por su color, para encontrar los que quedaron tapados o se
//! ocultaron.
//!
//! Clic en una fila: trae el pin a la vista y lo resalta. El ojo lo oculta o
//! lo vuelve a ensenar; la cruz lo cierra. Abajo, mostrar, ocultar y cerrar
//! todos (cerrar se puede deshacer).
//!
//! Como la paleta, este crate solo pinta y dice donde se pulso: que hace cada
//! cosa lo decide el gestor (`apps/pixpin/src/pines/abiertos.rs`), que
//! conoce el almacen. La disposicion es pura y se prueba aparte.

use std::rc::Rc;
use std::sync::Once;

use pixpin_geom::Rect;
use pixpin_render::{Color, MotorRender, Pintor, RectF, Superficie};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::Gdi::ValidateRect;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::menu::TextosV2;
use crate::ventana::ErrorPin;

/// Que es cada pin, para su miniatura y su detalle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoFila {
    Foto,
    Nota,
    Pdf,
    Video,
    Archivo,
    Vivo,
    Herramienta,
}

/// Una fila del panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilaPanel {
    pub id: u64,
    pub nombre: String,
    pub detalle: String,
    pub tipo: TipoFila,
    /// El color de su grupo (0-7 en la paleta), o sin grupo.
    pub grupo: Option<u8>,
    pub oculto: bool,
}

/// Lo que se pide desde el panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccionPanel {
    /// Traer el pin a la vista y resaltarlo.
    Traer(u64),
    /// Ocultarlo o volver a ensenarlo.
    Alternar(u64),
    Cerrar(u64),
    MostrarTodos,
    OcultarTodos,
    CerrarTodos,
    /// Devolver los que cerro «Cerrar todos».
    Deshacer,
    CerrarPanel,
}

/// Que hay bajo un punto del panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zona {
    Fila(u64),
    Ojo(u64),
    Cruz(u64),
    Boton(AccionPanel),
    /// La cabecera: se agarra para mover el panel.
    Cabecera,
}

/// Medidas, en pixeles logicos.
const ANCHO_LOGICO: f32 = 316.0;
const MARGEN: f32 = 14.0;
const ALTO_TITULO: f32 = 40.0;
const ALTO_BUSCAR: f32 = 40.0;
const ALTO_GRUPO: f32 = 32.0;
const ALTO_FILA: f32 = 52.0;
const LADO_BOTON: f32 = 40.0;
const ALTO_PIE_BOTON: f32 = 40.0;
const HUECO_PIE: f32 = 8.0;

/// El ancho del panel a una escala.
pub fn ancho_panel(escala: f32) -> u32 {
    (ANCHO_LOGICO * escala).ceil() as u32
}

/// Las filas que casan con la busqueda (sin distinguir mayusculas), en su
/// orden. Vacia, todas.
pub fn filtrar<'a>(filas: &'a [FilaPanel], busqueda: &str) -> Vec<&'a FilaPanel> {
    let b = busqueda.trim().to_lowercase();
    filas
        .iter()
        .filter(|f| {
            b.is_empty()
                || f.nombre.to_lowercase().contains(&b)
                || f.detalle.to_lowercase().contains(&b)
        })
        .collect()
}

/// Las filas por grupo: los colores en el orden de la paleta y «sin grupo»
/// al final, cada grupo con sus filas en el orden de llegada.
pub fn agrupar<'a>(filas: &[&'a FilaPanel]) -> Vec<(Option<u8>, Vec<&'a FilaPanel>)> {
    let mut grupos: Vec<(Option<u8>, Vec<&FilaPanel>)> = Vec::new();
    let mut claves: Vec<Option<u8>> = filas.iter().map(|f| f.grupo).collect();
    claves.sort_by_key(|g| g.map_or(u8::MAX as u16 + 1, |c| c as u16));
    claves.dedup();
    for g in claves {
        grupos.push((g, filas.iter().copied().filter(|f| f.grupo == g).collect()));
    }
    grupos
}

/// Una cosa pintada del panel.
#[derive(Debug, Clone, PartialEq)]
pub enum Pieza {
    Grupo {
        color: Option<u8>,
        cuantos: usize,
    },
    Fila {
        id: u64,
    },
}

/// El panel dispuesto: donde va cada cosa.
#[derive(Debug, Clone, PartialEq)]
pub struct Disposicion {
    pub ancho: f32,
    pub alto: f32,
    pub titulo: RectF,
    pub cruz_panel: RectF,
    pub buscar: RectF,
    /// La zona de la lista (recorta lo que se desplaza).
    pub lista: RectF,
    /// Lo que va en la lista, ya desplazado.
    pub piezas: Vec<(RectF, Pieza)>,
    /// Cuanto mide la lista entera, para no desplazarse de mas.
    pub alto_lista: f32,
    /// Los tres botones del pie, en orden.
    pub pie: [(RectF, AccionPanel); 3],
}

/// Dispone el panel. `desplazamiento` es lo bajado en la lista (>= 0).
pub fn disponer(
    filas: &[FilaPanel],
    busqueda: &str,
    alto: f32,
    escala: f32,
    desplazamiento: f32,
    deshacer: bool,
) -> Disposicion {
    let e = escala.max(0.5);
    let ancho = ANCHO_LOGICO * e;
    let m = MARGEN * e;
    let titulo = RectF {
        x: m,
        y: 10.0 * e,
        ancho: ancho - 2.0 * m,
        alto: ALTO_TITULO * e,
    };
    let cruz_panel = RectF {
        x: ancho - m - LADO_BOTON * e + 6.0 * e,
        y: titulo.y,
        ancho: LADO_BOTON * e,
        alto: LADO_BOTON * e,
    };
    let buscar = RectF {
        x: m,
        y: titulo.y + titulo.alto + 4.0 * e,
        ancho: ancho - 2.0 * m,
        alto: ALTO_BUSCAR * e,
    };
    let alto_pie = 3.0 * ALTO_PIE_BOTON * e + 2.0 * HUECO_PIE * e + 2.0 * m;
    let lista = RectF {
        x: 8.0 * e,
        y: buscar.y + buscar.alto + 6.0 * e,
        ancho: ancho - 16.0 * e,
        alto: (alto - alto_pie - (buscar.y + buscar.alto + 6.0 * e)).max(0.0),
    };
    let mut y = lista.y - desplazamiento;
    let mut piezas = Vec::new();
    let visibles = filtrar(filas, busqueda);
    for (color, del) in agrupar(&visibles) {
        piezas.push((
            RectF {
                x: lista.x,
                y,
                ancho: lista.ancho,
                alto: ALTO_GRUPO * e,
            },
            Pieza::Grupo {
                color,
                cuantos: del.len(),
            },
        ));
        y += ALTO_GRUPO * e;
        for f in del {
            piezas.push((
                RectF {
                    x: lista.x,
                    y,
                    ancho: lista.ancho,
                    alto: ALTO_FILA * e,
                },
                Pieza::Fila { id: f.id },
            ));
            y += ALTO_FILA * e;
        }
    }
    let alto_lista = y + desplazamiento - lista.y;
    let mut yb = alto - m - ALTO_PIE_BOTON * e * 3.0 - HUECO_PIE * e * 2.0;
    let mut boton = |a| {
        let r = RectF {
            x: m,
            y: yb,
            ancho: ancho - 2.0 * m,
            alto: ALTO_PIE_BOTON * e,
        };
        yb += (ALTO_PIE_BOTON + HUECO_PIE) * e;
        (r, a)
    };
    let pie = [
        boton(AccionPanel::MostrarTodos),
        boton(AccionPanel::OcultarTodos),
        boton(if deshacer {
            AccionPanel::Deshacer
        } else {
            AccionPanel::CerrarTodos
        }),
    ];
    Disposicion {
        ancho,
        alto,
        titulo,
        cruz_panel,
        buscar,
        lista,
        piezas,
        alto_lista,
        pie,
    }
}

fn dentro(r: RectF, x: f32, y: f32) -> bool {
    x >= r.x && x < r.x + r.ancho && y >= r.y && y < r.y + r.alto
}

impl Disposicion {
    /// El ojo y la cruz de una fila, a su derecha.
    pub fn botones_de_fila(&self, fila: RectF, escala: f32) -> (RectF, RectF) {
        let l = LADO_BOTON * escala;
        let y = fila.y + (fila.alto - l) / 2.0;
        let cruz = RectF {
            x: fila.x + fila.ancho - l - 2.0 * escala,
            y,
            ancho: l,
            alto: l,
        };
        let ojo = RectF {
            x: cruz.x - l,
            ..cruz
        };
        (ojo, cruz)
    }

    /// Lo que hay bajo un punto.
    pub fn zona_en(&self, x: f32, y: f32, escala: f32) -> Option<Zona> {
        if dentro(self.cruz_panel, x, y) {
            return Some(Zona::Boton(AccionPanel::CerrarPanel));
        }
        if dentro(self.titulo, x, y) {
            return Some(Zona::Cabecera);
        }
        for (r, a) in &self.pie {
            if dentro(*r, x, y) {
                return Some(Zona::Boton(*a));
            }
        }
        // La lista solo dentro de su recorte: lo desplazado fuera no se ve
        // y no se puede pulsar.
        if !dentro(self.lista, x, y) {
            return None;
        }
        for (r, p) in &self.piezas {
            if let Pieza::Fila { id } = p
                && dentro(*r, x, y)
            {
                let (ojo, cruz) = self.botones_de_fila(*r, escala);
                return Some(if dentro(cruz, x, y) {
                    Zona::Cruz(*id)
                } else if dentro(ojo, x, y) {
                    Zona::Ojo(*id)
                } else {
                    Zona::Fila(*id)
                });
            }
        }
        None
    }

    /// El desplazamiento dentro de lo que se puede bajar.
    pub fn limitar(&self, desplazamiento: f32) -> f32 {
        desplazamiento.clamp(0.0, (self.alto_lista - self.lista.alto).max(0.0))
    }
}

// ---------------------------------------------------------------------------
// La ventana.
// ---------------------------------------------------------------------------

struct Interno {
    motor: Rc<MotorRender>,
    superficie: Superficie,
    escala: f32,
    filas: Vec<FilaPanel>,
    busqueda: String,
    desplazamiento: f32,
    deshacer: bool,
    encima: Option<Zona>,
    textos: TextosV2,
    colores: [String; 8],
    tema_claro: bool,
    al_pedir: Box<dyn Fn(AccionPanel)>,
}

/// La ventana del panel. Se destruye al soltarla.
pub struct PanelAbiertos {
    hwnd: HWND,
}

static REGISTRO: Once = Once::new();

fn interno_de<'a>(hwnd: HWND) -> Option<&'a mut Interno> {
    // SAFETY: el puntero lo pone `PanelAbiertos::nuevo` y solo WM_NCDESTROY
    // lo retira; entre ambos es un Box valido. Todo en el hilo de interfaz.
    unsafe {
        let crudo = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Interno;
        crudo.as_mut()
    }
}

fn registrar_clase() {
    // SAFETY: registro unico (Once) de una clase con WndProc propio.
    unsafe {
        let clase = WNDCLASSW {
            lpfnWndProc: Some(procedimiento),
            hInstance: GetModuleHandleW(None).expect("modulo propio").into(),
            lpszClassName: w!("PixPinPanelPines"),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&clase);
    }
}

impl PanelAbiertos {
    /// Abre el panel en `rect` (pixeles fisicos) y lo activa: se escribe en
    /// su buscador y Esc lo cierra.
    #[allow(clippy::too_many_arguments)] // lo que el panel recibe al nacer
    pub fn nuevo(
        d3d: &ID3D11Device,
        motor: Rc<MotorRender>,
        rect: Rect,
        escala_por_cien: u32,
        textos: TextosV2,
        colores: [String; 8],
        tema_claro: bool,
        al_pedir: Box<dyn Fn(AccionPanel)>,
    ) -> Result<PanelAbiertos, ErrorPin> {
        REGISTRO.call_once(registrar_clase);
        // SAFETY: clase registrada; estilos documentados; modulo propio.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW,
                w!("PixPinPanelPines"),
                &windows::core::HSTRING::from(textos.panel_titulo.as_str()),
                WS_POPUP,
                rect.x,
                rect.y,
                rect.ancho as i32,
                rect.alto as i32,
                None,
                None,
                Some(GetModuleHandleW(None).map_err(ErrorPin::Creacion)?.into()),
                None,
            )
            .map_err(ErrorPin::Creacion)?
        };
        let superficie = Superficie::nueva(&motor, d3d, hwnd, rect.ancho, rect.alto)?;
        let interno = Box::new(Interno {
            motor,
            superficie,
            escala: escala_por_cien as f32 / 100.0,
            filas: Vec::new(),
            busqueda: String::new(),
            desplazamiento: 0.0,
            deshacer: false,
            encima: None,
            textos,
            colores,
            tema_claro,
            al_pedir,
        });
        // SAFETY: el Box se cede al USERDATA y se recupera en WM_NCDESTROY.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(interno) as isize);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
        }
        Ok(PanelAbiertos { hwnd })
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Donde esta, en pixeles fisicos (se puede mover por la cabecera).
    pub fn rect(&self) -> Rect {
        let mut r = RECT::default();
        // SAFETY: GetWindowRect sobre la ventana propia.
        unsafe {
            let _ = GetWindowRect(self.hwnd, &mut r);
        }
        Rect {
            x: r.left,
            y: r.top,
            ancho: (r.right - r.left).max(0) as u32,
            alto: (r.bottom - r.top).max(0) as u32,
        }
    }

    /// Cambia lo que ensena y repinta. `deshacer`: el ultimo «Cerrar todos»
    /// aun se puede deshacer.
    pub fn poner_filas(&self, filas: Vec<FilaPanel>, deshacer: bool) {
        if let Some(i) = interno_de(self.hwnd) {
            i.filas = filas;
            i.deshacer = deshacer;
            pintar(self.hwnd, i);
        }
    }

    /// Lo pone delante otra vez (se pidio abrirlo y ya estaba abierto).
    pub fn traer(&self) {
        // SAFETY: ventana propia y viva.
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = SetForegroundWindow(self.hwnd);
        }
    }
}

impl Drop for PanelAbiertos {
    fn drop(&mut self) {
        // SAFETY: destruir una ventana propia; WM_NCDESTROY libera el Box.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

fn alto_ventana(hwnd: HWND) -> f32 {
    let mut r = RECT::default();
    // SAFETY: GetClientRect sobre la ventana propia.
    unsafe {
        let _ = GetClientRect(hwnd, &mut r);
    }
    (r.bottom - r.top).max(1) as f32
}

fn disposicion_de(hwnd: HWND, i: &Interno) -> Disposicion {
    disponer(
        &i.filas,
        &i.busqueda,
        alto_ventana(hwnd),
        i.escala,
        i.desplazamiento,
        i.deshacer,
    )
}

const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a,
    }
}

/// Los ocho colores de grupo, en el orden de la paleta (`rgb_de` del gestor).
const COLORES_GRUPO: [Color; 8] = [
    rgba(219, 51, 46, 1.0),
    rgba(242, 115, 26, 1.0),
    rgba(242, 173, 26, 1.0),
    rgba(51, 168, 84, 1.0),
    rgba(26, 168, 173, 1.0),
    rgba(41, 112, 219, 1.0),
    rgba(122, 71, 199, 1.0),
    rgba(222, 71, 153, 1.0),
];

/// Un ojo: dos arcos y la pupila; tachado si esta oculto.
fn ojo(p: &Pintor, r: RectF, color: Color, tachado: bool, e: f32) {
    let (cx, cy) = (r.x + r.ancho / 2.0, r.y + r.alto / 2.0);
    let (a, b) = (9.0 * e, 5.5 * e);
    let arco = |signo: f32| -> Vec<(f32, f32)> {
        (0..=12)
            .map(|k| {
                let t = std::f32::consts::PI * k as f32 / 12.0;
                (cx - a * t.cos(), cy + signo * b * t.sin())
            })
            .collect()
    };
    p.polilinea(&arco(1.0), 1.8 * e, color);
    p.polilinea(&arco(-1.0), 1.8 * e, color);
    p.anillo((cx, cy), 2.6 * e, 1.8 * e, color);
    if tachado {
        p.linea(
            (cx - 8.0 * e, cy - 8.0 * e),
            (cx + 8.0 * e, cy + 8.0 * e),
            1.8 * e,
            color,
        );
    }
}

fn pintar(hwnd: HWND, i: &Interno) {
    let Ok(destino) = i.superficie.empezar(&i.motor) else {
        return;
    };
    let d = disposicion_de(hwnd, i);
    let _ = i.motor.dibujar(&destino, |p| {
        p.limpiar_transparente();
        pintar_panel(p, i, &d);
    });
    let _ = i.superficie.presentar();
}

fn pintar_panel(p: &Pintor, i: &Interno, d: &Disposicion) {
    use pixpin_render::icono::material::{CLOSE, SEARCH};
    let e = i.escala;
    let claro = i.tema_claro;
    let fondo = if claro {
        rgba(0xF9, 0xF9, 0xFB, 1.0)
    } else {
        rgba(0x23, 0x23, 0x26, 1.0)
    };
    let borde = if claro {
        rgba(0, 0, 0, 0.12)
    } else {
        rgba(255, 255, 255, 0.10)
    };
    let tinta = if claro {
        rgba(0x1C, 0x1C, 0x1E, 1.0)
    } else {
        rgba(0xF5, 0xF5, 0xF7, 1.0)
    };
    let tenue = if claro {
        rgba(0x6E, 0x6E, 0x73, 1.0)
    } else {
        rgba(0x98, 0x98, 0x9D, 1.0)
    };
    let suave = if claro {
        rgba(0, 0, 0, 0.06)
    } else {
        rgba(255, 255, 255, 0.07)
    };
    let naranja = rgba(0xFF, 0x9F, 0x0A, 1.0);
    let rojo = rgba(0xFF, 0x69, 0x61, 1.0);
    let azul = rgba(0x00, 0x60, 0xDF, 1.0);

    // El marco.
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: d.ancho,
        alto: d.alto,
    };
    p.rellenar_redondeado(todo, 14.0 * e, borde);
    p.rellenar_redondeado(
        RectF {
            x: 1.0,
            y: 1.0,
            ancho: d.ancho - 2.0,
            alto: d.alto - 2.0,
        },
        13.0 * e,
        fondo,
    );

    // El titulo, cuantos hay y la cruz.
    let t = &i.textos;
    let tam_titulo = 16.0 * e;
    let (_, ht) = p.medir_texto(&t.panel_titulo, tam_titulo);
    p.texto_linea(
        &t.panel_titulo,
        d.titulo.x,
        d.titulo.y + (d.titulo.alto - ht) / 2.0,
        tam_titulo,
        d.titulo.ancho - 90.0 * e,
        tinta,
    );
    let cuantos = i.filas.len().to_string();
    let (wc, hc) = p.medir_texto(&cuantos, 13.0 * e);
    p.texto_linea(
        &cuantos,
        d.cruz_panel.x - wc - 6.0 * e,
        d.titulo.y + (d.titulo.alto - hc) / 2.0,
        13.0 * e,
        wc + 2.0,
        tenue,
    );
    if i.encima == Some(Zona::Boton(AccionPanel::CerrarPanel)) {
        p.rellenar_redondeado(d.cruz_panel, 8.0 * e, suave);
    }
    let icono = |r: RectF, lado: f32| RectF {
        x: r.x + (r.ancho - lado) / 2.0,
        y: r.y + (r.alto - lado) / 2.0,
        ancho: lado,
        alto: lado,
    };
    p.icono(&CLOSE, icono(d.cruz_panel, 18.0 * e), tenue);

    // El buscador: lo que se escribe va siempre aqui.
    p.rellenar_redondeado(d.buscar, 10.0 * e, suave);
    p.icono(
        &SEARCH,
        RectF {
            x: d.buscar.x + 12.0 * e,
            y: d.buscar.y + (d.buscar.alto - 16.0 * e) / 2.0,
            ancho: 16.0 * e,
            alto: 16.0 * e,
        },
        tenue,
    );
    let tam = 14.0 * e;
    let (texto_buscar, color_buscar) = if i.busqueda.is_empty() {
        (t.panel_buscar.as_str(), tenue)
    } else {
        (i.busqueda.as_str(), tinta)
    };
    let (wb, hb) = p.medir_texto(texto_buscar, tam);
    let xb = d.buscar.x + 36.0 * e;
    p.texto_linea(
        texto_buscar,
        xb,
        d.buscar.y + (d.buscar.alto - hb) / 2.0,
        tam,
        d.buscar.ancho - 48.0 * e,
        color_buscar,
    );
    if !i.busqueda.is_empty() {
        // El cursor de escribir, tras lo escrito.
        let xc = (xb + wb + 1.0 * e).min(d.buscar.x + d.buscar.ancho - 10.0 * e);
        p.rellenar(
            RectF {
                x: xc,
                y: d.buscar.y + 11.0 * e,
                ancho: 1.5 * e,
                alto: d.buscar.alto - 22.0 * e,
            },
            Color::ACENTO,
        );
    }

    // La lista.
    p.con_recorte(d.lista, |p| {
        if d.piezas.is_empty() {
            let vacio = if i.filas.is_empty() {
                &t.panel_vacio
            } else {
                &t.panel_buscar
            };
            p.texto_ajustado(
                vacio,
                d.lista.x + 8.0 * e,
                d.lista.y + 16.0 * e,
                13.0 * e,
                d.lista.ancho - 16.0 * e,
                tenue,
            );
        }
        for (r, pieza) in &d.piezas {
            if r.y + r.alto < d.lista.y || r.y > d.lista.y + d.lista.alto {
                continue;
            }
            match pieza {
                Pieza::Grupo { color, cuantos } => {
                    let c = color.map_or(rgba(0x63, 0x63, 0x66, 1.0), |c| {
                        COLORES_GRUPO[c as usize % 8]
                    });
                    let cy = r.y + r.alto - 12.0 * e;
                    p.circulo((r.x + 9.0 * e, cy), 5.0 * e, c);
                    let nombre = match color {
                        Some(c) => i.colores[*c as usize % 8].to_uppercase(),
                        None => t.panel_sin_grupo.to_uppercase(),
                    };
                    let tam = 12.0 * e;
                    let (_, h) = p.medir_texto(&nombre, tam);
                    p.texto_linea(&nombre, r.x + 20.0 * e, cy - h / 2.0, tam, r.ancho - 60.0 * e, tenue);
                    let n = cuantos.to_string();
                    let (w, _) = p.medir_texto(&n, tam);
                    p.texto_linea(&n, r.x + r.ancho - w - 6.0 * e, cy - h / 2.0, tam, w + 2.0, tenue);
                }
                Pieza::Fila { id } => {
                    let Some(f) = i.filas.iter().find(|f| f.id == *id) else {
                        continue;
                    };
                    let encima = matches!(
                        i.encima,
                        Some(Zona::Fila(x) | Zona::Ojo(x) | Zona::Cruz(x)) if x == *id
                    );
                    if encima {
                        p.rellenar_redondeado(*r, 10.0 * e, suave);
                    }
                    let atenuar = |c: Color| if f.oculto { Color { a: c.a * 0.6, ..c } } else { c };
                    // La miniatura: el tipo, como en la maqueta.
                    let th = RectF {
                        x: r.x + 8.0 * e,
                        y: r.y + (r.alto - 36.0 * e) / 2.0,
                        ancho: 44.0 * e,
                        alto: 36.0 * e,
                    };
                    let (fondo_th, rotulo) = match f.tipo {
                        TipoFila::Foto => (rgba(0x3D, 0x5C, 0x80, 1.0), "IMG"),
                        TipoFila::Nota => (rgba(0x3A, 0x3A, 0x3C, 1.0), "TXT"),
                        TipoFila::Pdf => (rgba(0xE5, 0xE5, 0xEA, 1.0), "PDF"),
                        TipoFila::Video => (rgba(0x0B, 0x0B, 0x0D, 1.0), "VID"),
                        TipoFila::Archivo => (rgba(0x48, 0x48, 0x4A, 1.0), "DOC"),
                        TipoFila::Vivo => (rgba(0x24, 0x40, 0x2C, 1.0), "LIVE"),
                        TipoFila::Herramienta => (rgba(0x1E, 0x7A, 0x4A, 1.0), "APP"),
                    };
                    p.rellenar_redondeado(th, 7.0 * e, atenuar(fondo_th));
                    let color_rotulo = if f.tipo == TipoFila::Pdf {
                        rgba(0x3A, 0x3A, 0x3C, 1.0)
                    } else {
                        rgba(255, 255, 255, 0.8)
                    };
                    let tam_r = 9.5 * e;
                    let (wr, hr) = p.medir_texto(rotulo, tam_r);
                    p.texto_linea(
                        rotulo,
                        th.x + (th.ancho - wr) / 2.0,
                        th.y + (th.alto - hr) / 2.0,
                        tam_r,
                        th.ancho,
                        atenuar(color_rotulo),
                    );
                    if f.tipo == TipoFila::Vivo {
                        p.circulo((th.x + 7.0 * e, th.y + 7.0 * e), 3.0 * e, rgba(0xFF, 0x45, 0x3A, 1.0));
                    }
                    // Nombre y detalle.
                    let (ojo_r, cruz_r) = d.botones_de_fila(*r, e);
                    let xt = th.x + th.ancho + 10.0 * e;
                    let ancho_t = (ojo_r.x - xt - 4.0 * e).max(1.0);
                    let tam_n = 14.0 * e;
                    p.texto_linea(&f.nombre, xt, r.y + 8.0 * e, tam_n, ancho_t, atenuar(tinta));
                    let (detalle, color_detalle) = if f.oculto {
                        (t.panel_oculto.as_str(), naranja)
                    } else {
                        (f.detalle.as_str(), tenue)
                    };
                    p.texto_linea(detalle, xt, r.y + 29.0 * e, 12.0 * e, ancho_t, color_detalle);
                    if i.encima == Some(Zona::Ojo(*id)) {
                        p.rellenar_redondeado(ojo_r, 8.0 * e, suave);
                    }
                    if i.encima == Some(Zona::Cruz(*id)) {
                        p.rellenar_redondeado(cruz_r, 8.0 * e, suave);
                    }
                    ojo(p, ojo_r, if f.oculto { tinta } else { tenue }, f.oculto, e);
                    p.icono(&CLOSE, icono(cruz_r, 16.0 * e), tenue);
                }
            }
        }
        // La ayuda, debajo de la ultima fila.
        if !d.piezas.is_empty() {
            let y = d.lista.y + d.alto_lista - i.desplazamiento + 10.0 * e;
            p.texto_ajustado(
                &t.panel_ayuda,
                d.lista.x + 6.0 * e,
                y,
                12.0 * e,
                d.lista.ancho - 12.0 * e,
                tenue,
            );
        }
    });

    // El pie: mostrar (azul), ocultar (con su atajo) y cerrar (rojo).
    p.rellenar(
        RectF {
            x: 0.0,
            y: d.pie[0].0.y - MARGEN * e,
            ancho: d.ancho,
            alto: 1.0,
        },
        borde,
    );
    for (r, a) in &d.pie {
        let encima = i.encima == Some(Zona::Boton(*a));
        let (texto, atajo, fondo_b, color_t) = match a {
            AccionPanel::MostrarTodos => (&t.panel_mostrar_todos, None, azul, Color::BLANCO),
            AccionPanel::OcultarTodos => (&t.panel_ocultar_todos, Some("Ctrl 2"), suave, tinta),
            AccionPanel::Deshacer => (&t.panel_deshacer, Some("Ctrl Z"), suave, tinta),
            _ => (&t.panel_cerrar_todos, None, Color { a: 0.0, ..rojo }, rojo),
        };
        let fondo_b = if encima {
            Color {
                a: (fondo_b.a + 0.08).min(1.0),
                ..fondo_b
            }
        } else {
            fondo_b
        };
        p.rellenar_redondeado(*r, 10.0 * e, fondo_b);
        if *a == AccionPanel::CerrarTodos {
            p.trazar(*r, 1.0 * e, Color { a: 0.4, ..rojo });
        }
        let tam = 14.0 * e;
        let (w, h) = p.medir_texto(texto, tam);
        let wa = atajo.map_or(0.0, |s| p.medir_texto(s, 11.0 * e).0 + 12.0 * e + 8.0 * e);
        let x = r.x + (r.ancho - w - wa) / 2.0;
        p.texto_linea(texto, x, r.y + (r.alto - h) / 2.0, tam, w + 2.0, color_t);
        if let Some(s) = atajo {
            let (ws, hs) = p.medir_texto(s, 11.0 * e);
            let chapa = RectF {
                x: x + w + 8.0 * e,
                y: r.y + (r.alto - 20.0 * e) / 2.0,
                ancho: ws + 12.0 * e,
                alto: 20.0 * e,
            };
            p.rellenar_redondeado(chapa, 5.0 * e, suave);
            p.texto_linea(s, chapa.x + 6.0 * e, chapa.y + (20.0 * e - hs) / 2.0, 11.0 * e, ws + 2.0, tenue);
        }
    }
}

fn local(lparam: LPARAM) -> (f32, f32) {
    (
        (lparam.0 & 0xFFFF) as i16 as f32,
        ((lparam.0 >> 16) & 0xFFFF) as i16 as f32,
    )
}

/// Pide algo al gestor sin prestamos vivos: puede cerrar este panel.
fn pedir(hwnd: HWND, a: AccionPanel) {
    let f: *const dyn Fn(AccionPanel) = match interno_de(hwnd) {
        Some(i) => &*i.al_pedir,
        None => return,
    };
    // SAFETY: el cierre vive en el Interno, que solo se libera en
    // WM_NCDESTROY. Si el gestor cierra el panel dentro de la llamada, lo
    // hace soltando `PanelAbiertos`, cuya destruccion se pide aqui mismo y
    // termina despues de que el cierre devuelva (DestroyWindow es sincrona,
    // pero el gestor no lo suelta hasta atender su cola, fuera de esto).
    unsafe { (*f)(a) };
}

extern "system" fn procedimiento(
    hwnd: HWND,
    mensaje: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    const WM_RATON_FUERA: u32 = 0x02A3;
    match mensaje {
        // La cabecera mueve el panel, como la barra de titulo de cualquier
        // ventana.
        WM_NCHITTEST => {
            // SAFETY: delegacion estandar para saber si cae dentro.
            let r = unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) };
            if r.0 as u32 == HTCLIENT
                && let Some(i) = interno_de(hwnd)
            {
                let mut w = RECT::default();
                // SAFETY: GetWindowRect sobre la ventana propia.
                unsafe {
                    let _ = GetWindowRect(hwnd, &mut w);
                }
                let (px, py) = local(lparam);
                let (x, y) = (px - w.left as f32, py - w.top as f32);
                if disposicion_de(hwnd, i).zona_en(x, y, i.escala) == Some(Zona::Cabecera) {
                    return LRESULT(HTCAPTION as isize);
                }
            }
            r
        }
        WM_MOUSEMOVE => {
            if let Some(i) = interno_de(hwnd) {
                let (x, y) = local(lparam);
                let z = disposicion_de(hwnd, i)
                    .zona_en(x, y, i.escala)
                    .filter(|z| *z != Zona::Cabecera);
                if z != i.encima {
                    i.encima = z;
                    pintar(hwnd, i);
                    let mut seguir = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    // SAFETY: estructura completa sobre la ventana propia.
                    unsafe {
                        let _ = TrackMouseEvent(&mut seguir);
                    }
                }
            }
            LRESULT(0)
        }
        m if m == WM_RATON_FUERA => {
            if let Some(i) = interno_de(hwnd)
                && i.encima.is_some()
            {
                i.encima = None;
                pintar(hwnd, i);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let Some(i) = interno_de(hwnd) else {
                return LRESULT(0);
            };
            let (x, y) = local(lparam);
            let accion = match disposicion_de(hwnd, i).zona_en(x, y, i.escala) {
                Some(Zona::Fila(id)) => Some(AccionPanel::Traer(id)),
                Some(Zona::Ojo(id)) => Some(AccionPanel::Alternar(id)),
                Some(Zona::Cruz(id)) => Some(AccionPanel::Cerrar(id)),
                Some(Zona::Boton(a)) => Some(a),
                Some(Zona::Cabecera) | None => None,
            };
            if let Some(a) = accion {
                pedir(hwnd, a);
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            if let Some(i) = interno_de(hwnd) {
                let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as f32;
                let d = disposicion_de(hwnd, i);
                i.desplazamiento = d.limitar(i.desplazamiento - delta / 120.0 * 3.0 * 52.0 * i.escala / 2.0);
                pintar(hwnd, i);
            }
            LRESULT(0)
        }
        WM_CHAR => {
            if let Some(i) = interno_de(hwnd) {
                let c = char::from_u32(wparam.0 as u32);
                match c {
                    // Retroceso borra la ultima letra.
                    Some('\u{8}') => {
                        i.busqueda.pop();
                    }
                    Some(c) if !c.is_control() => i.busqueda.push(c),
                    _ => return LRESULT(0),
                }
                i.desplazamiento = 0.0;
                pintar(hwnd, i);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = wparam.0 as u32;
            const VK_ESCAPE: u32 = 0x1B;
            const VK_Z: u32 = b'Z' as u32;
            let ctrl = {
                // SAFETY: consulta pura del teclado.
                let k = unsafe {
                    windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(0x11)
                };
                k < 0
            };
            if vk == VK_ESCAPE {
                // Esc primero borra la busqueda; con ella vacia, cierra.
                let vaciar = interno_de(hwnd).is_some_and(|i| !i.busqueda.is_empty());
                if vaciar {
                    if let Some(i) = interno_de(hwnd) {
                        i.busqueda.clear();
                        pintar(hwnd, i);
                    }
                } else {
                    pedir(hwnd, AccionPanel::CerrarPanel);
                }
            } else if vk == VK_Z && ctrl && interno_de(hwnd).is_some_and(|i| i.deshacer) {
                pedir(hwnd, AccionPanel::Deshacer);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            pedir(hwnd, AccionPanel::CerrarPanel);
            LRESULT(0)
        }
        WM_PAINT => {
            // SAFETY: ValidateRect marca la ventana como pintada; el dibujo
            // real lo hace el swapchain.
            unsafe {
                let _ = ValidateRect(Some(hwnd), None);
            }
            if let Some(i) = interno_de(hwnd) {
                pintar(hwnd, i);
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            // SAFETY: recupera el Box cedido en `nuevo` una sola vez.
            unsafe {
                let crudo = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut Interno;
                if !crudo.is_null() {
                    drop(Box::from_raw(crudo));
                }
            }
            LRESULT(0)
        }
        // NUNCA PostQuitMessage: cerrar el panel no apaga la aplicacion.
        // SAFETY: delegacion estandar.
        _ => unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) },
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn fila(id: u64, nombre: &str, grupo: Option<u8>) -> FilaPanel {
        FilaPanel {
            id,
            nombre: nombre.into(),
            detalle: "Foto".into(),
            tipo: TipoFila::Foto,
            grupo,
            oculto: false,
        }
    }

    #[test]
    fn los_grupos_van_en_el_orden_de_la_paleta_y_sin_grupo_al_final() {
        let filas = [
            fila(1, "a", None),
            fila(2, "b", Some(5)),
            fila(3, "c", Some(1)),
            fila(4, "d", Some(5)),
        ];
        let refs: Vec<&FilaPanel> = filas.iter().collect();
        let g = agrupar(&refs);
        let claves: Vec<Option<u8>> = g.iter().map(|(c, _)| *c).collect();
        assert_eq!(claves, vec![Some(1), Some(5), None]);
        let ids: Vec<u64> = g[1].1.iter().map(|f| f.id).collect();
        assert_eq!(ids, vec![2, 4], "en el orden de llegada");
    }

    #[test]
    fn buscar_no_distingue_mayusculas_y_caso_negativo_lo_que_no_casa_se_va() {
        let filas = [fila(1, "Plano Obra Miraflores", None), fila(2, "Tesis.pdf", None)];
        let v = filtrar(&filas, "obra");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, 1);
        assert!(filtrar(&filas, "zzz").is_empty());
        assert_eq!(filtrar(&filas, "  ").len(), 2, "vacia es todas");
    }

    #[test]
    fn el_clic_en_una_fila_distingue_fila_ojo_y_cruz() {
        let filas = [fila(7, "x", None)];
        let d = disponer(&filas, "", 800.0, 1.0, 0.0, false);
        let (r, _) = d
            .piezas
            .iter()
            .find(|(_, p)| matches!(p, Pieza::Fila { .. }))
            .cloned()
            .unwrap();
        let (ojo, cruz) = d.botones_de_fila(r, 1.0);
        assert_eq!(d.zona_en(r.x + 20.0, r.y + 20.0, 1.0), Some(Zona::Fila(7)));
        assert_eq!(d.zona_en(ojo.x + 5.0, ojo.y + 5.0, 1.0), Some(Zona::Ojo(7)));
        assert_eq!(d.zona_en(cruz.x + 5.0, cruz.y + 5.0, 1.0), Some(Zona::Cruz(7)));
        // Los botones de fila miden 40.
        assert!(ojo.ancho >= 40.0 && cruz.alto >= 40.0);
    }

    #[test]
    fn el_pie_lleva_mostrar_ocultar_y_cerrar_o_deshacer() {
        let d = disponer(&[], "", 800.0, 1.0, 0.0, false);
        let acciones: Vec<AccionPanel> = d.pie.iter().map(|(_, a)| *a).collect();
        assert_eq!(
            acciones,
            vec![
                AccionPanel::MostrarTodos,
                AccionPanel::OcultarTodos,
                AccionPanel::CerrarTodos
            ]
        );
        let d = disponer(&[], "", 800.0, 1.0, 0.0, true);
        assert_eq!(d.pie[2].1, AccionPanel::Deshacer, "tras cerrar todos, deshacer");
        // Todo el pie dentro del panel.
        assert!(d.pie[2].0.y + d.pie[2].0.alto <= 800.0);
    }

    #[test]
    fn caso_negativo_una_fila_desplazada_fuera_de_la_lista_no_se_pulsa() {
        let filas: Vec<FilaPanel> = (0..40).map(|i| fila(i, "x", None)).collect();
        let d0 = disponer(&filas, "", 600.0, 1.0, 0.0, false);
        let baja = d0.limitar(1.0e6);
        assert!(baja > 0.0 && baja < 1.0e6, "se limita a lo que hay");
        let d = disponer(&filas, "", 600.0, 1.0, baja, false);
        // Encima de la lista (en el buscador) no hay fila aunque se haya
        // desplazado una por debajo.
        assert!(!matches!(
            d.zona_en(d.buscar.x + 50.0, d.buscar.y + 5.0, 1.0),
            Some(Zona::Fila(_))
        ));
        // Ni una lista corta se desplaza.
        let corta = disponer(&filas[..2], "", 600.0, 1.0, 0.0, false);
        assert_eq!(corta.limitar(300.0), 0.0);
    }

    #[test]
    fn la_cabecera_agarra_y_la_cruz_cierra() {
        let d = disponer(&[], "", 600.0, 1.0, 0.0, false);
        assert_eq!(
            d.zona_en(d.titulo.x + 10.0, d.titulo.y + 10.0, 1.0),
            Some(Zona::Cabecera)
        );
        assert_eq!(
            d.zona_en(d.cruz_panel.x + 10.0, d.cruz_panel.y + 10.0, 1.0),
            Some(Zona::Boton(AccionPanel::CerrarPanel))
        );
    }
}
