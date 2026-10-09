//! **La ventana de un modelo 3D (BIM), como un pin**: la misma que la de los
//! planos (sin bordes, siempre encima o no, la barra fuera que aparece y se
//! va), con lo que pide un modelo: girarlo, acercarse y cortarlo. Lo pidio el
//! usuario el 9-oct-2026: «un visor sencillo como un pin, que se pueda ver
//! facilmente cualquier modelo».
//!
//! - Arrastrar (izquierdo): girar alrededor del punto que se toco.
//! - Arrastrar (derecho o central), o Mayus + izquierdo: mover.
//! - Rueda: acercar y alejar hacia el cursor.
//! - Clic: el elemento tocado se resalta y se nombra (Muro · nombre).
//! - Doble clic, F o Inicio: el modelo entero.
//! - Barra: asa, tema, corte (un plano horizontal que se sube y se baja con
//!   el deslizador de la derecha o la rueda encima de el) y el pin.
//! - Teclas: Esc (suelta lo elegido; si no hay, cierra), B tema, C corte,
//!   A aristas, T encima, P planta, flechas para girar.

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, ReleaseCapture, VK_CONTROL, VK_SHIFT};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use crate::convertir::rgba;
use crate::gpu::{Gpu, PlanoGpu};
use crate::gpu3d::{ModeloGpu, Vista3d};
use crate::modelo::{Constructor, Modelo};
use crate::modelo3d::{Modelo3d, tipo_legible};
use crate::texto::Textos;
use crate::ventana::{
    ALTO_BARRA, CAPA_ENCIMA, De, ESTADO, Ev, ancho_barra, aviso, barra, botones_barra, colocar_barra, crear_barra, crear_ventana, poner_encima,
    rect, rect_en, seguir_barra, vista_pantalla,
};

const INFINITE: u32 = u32::MAX;
const BOTONES: [&str; 4] = ["asa", "tema", "corte", "fijar"];
const FOV: f64 = 0.785_398; // 45 grados

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn suma(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn por(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn punto(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cruz(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = punto(a, a).sqrt().max(1e-30);
    por(a, 1.0 / l)
}

/// La camara: mira a `objetivo` desde `dist`, girada `rumbo` (alrededor de
/// Z) y `altura` (sobre el horizonte), en radianes. Z es arriba.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orbita {
    pub objetivo: [f64; 3],
    pub rumbo: f64,
    pub altura: f64,
    pub dist: f64,
}

const ALTURA_MAX: f64 = 1.5697; // casi 90 grados: la planta

impl Orbita {
    /// Desde el ojo hacia el objetivo, unitario.
    fn adelante(&self) -> [f64; 3] {
        let (c, s) = (self.altura.cos(), self.altura.sin());
        [-c * self.rumbo.cos(), -c * self.rumbo.sin(), -s]
    }

    pub fn ojo(&self) -> [f64; 3] {
        sub(self.objetivo, por(self.adelante(), self.dist))
    }

    /// Derecha, arriba y adelante de la pantalla.
    fn ejes(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let f = self.adelante();
        let mut r = cruz(f, [0.0, 0.0, 1.0]);
        if punto(r, r) < 1e-12 {
            r = [-self.rumbo.sin(), self.rumbo.cos(), 0.0];
        }
        let r = unit(r);
        let u = cruz(r, f);
        (r, u, f)
    }

    /// El modelo entero (su caja) en la ventana, visto desde el sureste y
    /// algo de arriba.
    pub fn encuadrar(caja: [f32; 6], ancho: u32, alto: u32) -> Orbita {
        let c = [(caja[0] + caja[3]) as f64 / 2.0, (caja[1] + caja[4]) as f64 / 2.0, (caja[2] + caja[5]) as f64 / 2.0];
        let d = [(caja[3] - caja[0]) as f64, (caja[4] - caja[1]) as f64, (caja[5] - caja[2]) as f64];
        let mut o = Orbita { objetivo: c, rumbo: -0.8, altura: 0.5, dist: 1.0 };
        o.dist = o.distancia_para(caja, ancho, alto).max(punto(d, d).sqrt() * 1e-3).max(1e-3);
        o
    }

    /// Lo que hay que alejarse para que las 8 esquinas de la caja quepan.
    fn distancia_para(&self, caja: [f32; 6], ancho: u32, alto: u32) -> f64 {
        let (r, u, f) = self.ejes();
        let ty = (FOV / 2.0).tan() * 0.94;
        let tx = ty * ancho.max(1) as f64 / alto.max(1) as f64;
        let mut d: f64 = 0.0;
        for i in 0..8 {
            let p = [
                caja[if i & 1 == 0 { 0 } else { 3 }] as f64,
                caja[if i & 2 == 0 { 1 } else { 4 }] as f64,
                caja[if i & 4 == 0 { 2 } else { 5 }] as f64,
            ];
            let q = sub(p, self.objetivo);
            let z = punto(q, f);
            d = d.max(punto(q, r).abs() / tx - z).max(punto(q, u).abs() / ty - z);
        }
        d
    }

    /// De mundo (relativo al origen del modelo) a recorte, en filas (p * M).
    pub fn matriz(&self, ancho: u32, alto: u32, radio: f64) -> [[f32; 4]; 4] {
        let (r, u, f) = self.ejes();
        let e = self.ojo();
        let cerca = (self.dist * 0.01).max(radio * 1e-5).max(1e-4);
        let lejos = self.dist + radio * 4.0 + 1.0;
        let ys = 1.0 / (FOV / 2.0).tan();
        let xs = ys * alto.max(1) as f64 / ancho.max(1) as f64;
        let q = lejos / (lejos - cerca);
        // Vista (filas): x = r·(p-e), y = u·(p-e), z = f·(p-e).
        let v = [
            [r[0], u[0], f[0], 0.0],
            [r[1], u[1], f[1], 0.0],
            [r[2], u[2], f[2], 0.0],
            [-punto(r, e), -punto(u, e), -punto(f, e), 1.0],
        ];
        let p = [[xs, 0.0, 0.0, 0.0], [0.0, ys, 0.0, 0.0], [0.0, 0.0, q, 1.0], [0.0, 0.0, -cerca * q, 0.0]];
        let mut m = [[0f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                m[i][j] = (0..4).map(|k| v[i][k] * p[k][j]).sum::<f64>() as f32;
            }
        }
        m
    }

    /// La direccion (unitaria) del rayo que sale del ojo por el pixel (x, y).
    pub fn rayo(&self, x: f64, y: f64, ancho: u32, alto: u32) -> [f64; 3] {
        let (r, u, f) = self.ejes();
        let t = (FOV / 2.0).tan();
        let nx = (x / ancho.max(1) as f64 * 2.0 - 1.0) * t * ancho.max(1) as f64 / alto.max(1) as f64;
        let ny = (1.0 - y / alto.max(1) as f64 * 2.0) * t;
        unit(suma(f, suma(por(r, nx), por(u, ny))))
    }

    /// Lo que mide un pixel a la distancia del objetivo.
    fn por_pixel(&self, alto: u32) -> f64 {
        2.0 * self.dist * (FOV / 2.0).tan() / alto.max(1) as f64
    }

    /// Acercar `factor` (<1 acerca) dejando quieto `fijo` en la pantalla.
    pub fn zoom(&self, factor: f64, fijo: [f64; 3]) -> Orbita {
        let factor = factor.clamp(1e-3, 1e3);
        let mut o = *self;
        o.objetivo = suma(fijo, por(sub(self.objetivo, fijo), factor));
        o.dist = (self.dist * factor).max(1e-4);
        o
    }

    /// Girar alrededor de `pivote` (rumbo y altura en radianes).
    pub fn girar(&self, d_rumbo: f64, d_altura: f64, pivote: [f64; 3]) -> Orbita {
        let altura = (self.altura + d_altura).clamp(-ALTURA_MAX, ALTURA_MAX);
        let d_altura = altura - self.altura;
        let ojo = self.ojo();
        // Rumbo: alrededor de la vertical del pivote.
        let rz = |p: [f64; 3]| {
            let q = sub(p, pivote);
            let (c, s) = (d_rumbo.cos(), d_rumbo.sin());
            suma(pivote, [q[0] * c - q[1] * s, q[0] * s + q[1] * c, q[2]])
        };
        let (ojo, obj) = (rz(ojo), rz(self.objetivo));
        let mut o = Orbita { objetivo: obj, rumbo: self.rumbo + d_rumbo, altura: self.altura, dist: self.dist };
        // Altura: alrededor del eje derecho de la camara, por el pivote.
        let (r, _, _) = o.ejes();
        let rot = |p: [f64; 3]| {
            let q = sub(p, pivote);
            let (c, s) = (d_altura.cos(), d_altura.sin());
            // Rodrigues, con el signo que sube el ojo al subir la altura.
            let k = por(r, -1.0);
            let v = suma(suma(por(q, c), por(cruz(k, q), s)), por(k, punto(k, q) * (1.0 - c)));
            suma(pivote, v)
        };
        let (ojo, obj) = (rot(ojo), rot(o.objetivo));
        o.objetivo = obj;
        o.altura = altura;
        // Que el objetivo quede a `dist` delante del ojo nuevo.
        o.objetivo = suma(ojo, por(o.adelante(), self.dist));
        o
    }

    /// Mover la camara `dx`, `dy` pixeles (el modelo sigue al raton).
    pub fn mover(&self, dx: f64, dy: f64, alto: u32) -> Orbita {
        let (r, u, _) = self.ejes();
        let k = self.por_pixel(alto);
        let mut o = *self;
        o.objetivo = suma(self.objetivo, suma(por(r, -dx * k), por(u, dy * k)));
        o
    }

    fn hacia(&self, destino: &Orbita, k: f64) -> Orbita {
        let mut dr = destino.rumbo - self.rumbo;
        while dr > std::f64::consts::PI {
            dr -= std::f64::consts::TAU;
        }
        while dr < -std::f64::consts::PI {
            dr += std::f64::consts::TAU;
        }
        Orbita {
            objetivo: suma(self.objetivo, por(sub(destino.objetivo, self.objetivo), k)),
            rumbo: self.rumbo + dr * k,
            altura: self.altura + (destino.altura - self.altura) * k,
            dist: (self.dist.ln() + (destino.dist.ln() - self.dist.ln()) * k).exp(),
        }
    }

    fn cerca_de(&self, o: &Orbita) -> bool {
        let d = sub(self.objetivo, o.objetivo);
        punto(d, d).sqrt() < o.dist * 1e-3
            && (self.rumbo - o.rumbo).abs() < 1e-3
            && (self.altura - o.altura).abs() < 1e-3
            && (self.dist / o.dist - 1.0).abs() < 1e-3
    }
}

/// Los textos de la ventana, en el idioma de la app.
#[derive(Debug, Clone)]
pub struct TextosUi3d {
    pub abriendo: String,
    pub vacio: String,
    pub error: String,
    pub todo: String,
    pub tema: String,
    pub corte: String,
    pub aristas: String,
    pub planta: String,
    pub encima: String,
    pub cerrar: String,
    pub altura_corte: String,
    pub pista: String,
}

impl Default for TextosUi3d {
    fn default() -> Self {
        Self {
            abriendo: "Abriendo el modelo…".into(),
            vacio: "El modelo no tiene nada que dibujar".into(),
            error: "No se pudo abrir el modelo".into(),
            todo: "Ver todo el modelo\tF".into(),
            tema: "Fondo claro u oscuro\tB".into(),
            corte: "Cortar el modelo\tC".into(),
            aristas: "Aristas\tA".into(),
            planta: "Vista en planta\tP".into(),
            encima: "Siempre encima\tT".into(),
            cerrar: "Cerrar\tEsc".into(),
            altura_corte: "Corte".into(),
            pista: "Arrastrar: girar · Derecho: mover · Rueda: acercar · Clic: ver que es".into(),
        }
    }
}

const M_TODO: u32 = 1;
const M_TEMA: u32 = 2;
const M_CORTE: u32 = 3;
const M_ARISTAS: u32 = 4;
const M_PLANTA: u32 = 5;
const M_ENCIMA: u32 = 6;
const M_CERRAR: u32 = 7;

fn menu(hwnd: HWND, t: &TextosUi3d, corte: bool, aristas: bool, fijada: bool) {
    // SAFETY: menu propio que se destruye antes de salir; cadenas vivas.
    unsafe {
        let Ok(m) = CreatePopupMenu() else { return };
        let poner = |id: u32, texto: &str, marca: bool| {
            let v: Vec<u16> = texto.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = AppendMenuW(m, if marca { MF_STRING | MF_CHECKED } else { MF_STRING }, id as usize, PCWSTR(v.as_ptr()));
        };
        poner(M_TODO, &t.todo, false);
        poner(M_PLANTA, &t.planta, false);
        poner(M_TEMA, &t.tema, false);
        poner(M_CORTE, &t.corte, corte);
        poner(M_ARISTAS, &t.aristas, aristas);
        let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
        poner(M_ENCIMA, &t.encima, fijada);
        poner(M_CERRAR, &t.cerrar, false);
        let mut p = windows::Win32::Foundation::POINT::default();
        let _ = GetCursorPos(&mut p);
        let _ = TrackPopupMenu(m, TPM_RIGHTBUTTON, p.x, p.y, Some(0), hwnd, None);
        let _ = DestroyMenu(m);
    }
}

/// Lo que se escribe tras una cota: metros, pies o nada (un DWG no lo dice).
fn unidad_de(metros: f32) -> &'static str {
    if (metros - 1.0).abs() < 1e-6 {
        " m"
    } else if (metros - 0.3048).abs() < 0.001 {
        " ft"
    } else {
        ""
    }
}

/// El deslizador del corte: donde esta (x0, y0, x1, y1) en la ventana.
fn carril(w: u32, h: u32, e: f64) -> (f64, f64, f64, f64) {
    let x1 = w as f64 - 14.0 * e;
    (x1 - 8.0 * e, 40.0 * e, x1, h as f64 - 40.0 * e)
}

struct Encima<'a> {
    elegido: Option<&'a crate::modelo3d::Elemento>,
    corte: Option<(f64, f64)>,
    unidad: &'a str,
    pista: Option<&'a str>,
}

/// Lo que va plano encima del modelo: el nombre de lo elegido, el
/// deslizador del corte y la pista.
fn encima_del_modelo(textos: &mut Textos, a: &Encima, ui: &TextosUi3d, w: u32, h: u32, e: f64, claro: bool) -> Modelo {
    let mut c = Constructor::nuevo();
    let (pildora, letra, tenue) =
        if claro { (0xEE1C1C1Eu32, rgba(255, 255, 255), rgba(0xC7, 0xC7, 0xCC)) } else { (0xEEF5F5F7u32, rgba(0x1C, 0x1C, 0x1E), rgba(0x6E, 0x6E, 0x73)) };
    let tam = 13.0 * e;
    let pastilla = |textos: &mut Textos, c: &mut Constructor, t: &str, x: f64, y: f64| {
        let ancho = textos.medir_pantalla(t, tam);
        rect(c, x, y, x + ancho + 20.0 * e, y + 26.0 * e, pildora);
        textos.en_pantalla(c, t, x + 10.0 * e, y + 17.5 * e, tam, letra, x + ancho + 40.0 * e);
    };
    if let Some(el) = a.elegido {
        let tipo = tipo_legible(&el.tipo);
        let t = if el.nombre.is_empty() { tipo } else { format!("{tipo} · {}", el.nombre) };
        pastilla(textos, &mut c, &t, 12.0 * e, h as f64 - 38.0 * e);
    } else if let Some(p) = a.pista {
        pastilla(textos, &mut c, p, 12.0 * e, h as f64 - 38.0 * e);
    }
    if let Some((frac, z)) = a.corte {
        let (x0, y0, x1, y1) = carril(w, h, e);
        let azul = rgba(0x00, 0x60, 0xDF);
        rect_en(&mut c, x0 + 2.5 * e, y0, x1 - 2.5 * e, y1, tenue, CAPA_ENCIMA * 10.0);
        let y = y1 - (y1 - y0) * frac;
        rect_en(&mut c, x0 + 2.5 * e, y, x1 - 2.5 * e, y1, azul, CAPA_ENCIMA * 5.0);
        rect(&mut c, x0 - 5.0 * e, y - 4.0 * e, x1 + 5.0 * e, y + 4.0 * e, azul);
        let t = format!("{} {:.2}{}", ui.altura_corte, z, a.unidad);
        let ancho = textos.medir_pantalla(&t, tam);
        pastilla(textos, &mut c, &t, x0 - ancho - 38.0 * e, y - 13.0 * e);
    }
    c.terminar()
}

/// **Abre la ventana del modelo** y no vuelve hasta que se cierra.
pub fn ver(titulo: &str, cargando: Receiver<Result<Modelo3d, String>>, ui: TextosUi3d) -> Result<(), String> {
    let (hwnd, mut w, mut h) = crear_ventana(w!("PixPinModelo3d"), &format!("{titulo} — PixPin")).map_err(|e| e.to_string())?;
    let mut gpu = match Gpu::nueva(hwnd, w, h) {
        Ok(g) => g,
        Err(e) => {
            // SAFETY: ventana propia.
            let _ = unsafe { DestroyWindow(hwnd) };
            return Err(format!("sin Direct3D: {e}"));
        }
    };
    // SAFETY: ventana propia.
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let e = dpi as f64 / 96.0;
    let (bw, bh) = (ancho_barra(e, BOTONES.len()).ceil() as i32, (ALTO_BARRA * e).ceil() as i32);
    let hbarra = crear_barra(hwnd, bw, bh).map_err(|e| e.to_string())?;
    let mut gpu_barra = Gpu::nueva(hbarra, bw as u32, bh as u32).map_err(|e| e.to_string())?;
    ESTADO.with(|s| s.borrow_mut().barra = Some((hbarra, (8.0 * e) as i32)));
    let mut textos = Textos::nuevo();
    let mut modelo: Option<(Modelo3d, ModeloGpu)> = None;
    let mut mensaje = ui.abriendo.clone();
    let mut cam = Orbita { objetivo: [0.0; 3], rumbo: -0.8, altura: 0.5, dist: 10.0 };
    let mut destino = cam;
    let mut radio = 10.0;
    // Lo que se encuadra y se corta: la caja sin los puntos sueltos lejanos.
    let mut caja_vista = [0f32; 6];
    let mut claro = false;
    let mut fijada = true;
    let mut aristas = true;
    let mut corte: Option<f64> = None; // fraccion de la altura del modelo
    let mut elegido: Option<u32> = None;
    let mut pista_hasta = Some(Instant::now() + Duration::from_secs(6));
    let (mut dentro_plano, mut dentro_barra) = (false, false);
    let mut fuera_desde: Option<Instant> = None;
    let mut barra_visible = false;
    // Arrastre: desde donde, la camara al empezar, que boton, el pivote.
    let mut arrastre: Option<(i32, i32, Orbita, u8, [f64; 3], bool)> = None;
    let mut arrastre_corte = false;
    let mut boton_encima: Option<usize> = None;
    let mut sucio = true;
    let mut barra_sucia = true;
    // El ultimo punto tocado por la rueda: se reusa mientras se sigue girando.
    let mut rueda: Option<(Instant, i32, i32, [f64; 3])> = None;
    let mut aviso_ui: Option<(Modelo, PlanoGpu)> = None;
    let vista = |cam: &Orbita, w: u32, h: u32, radio: f64, elegido: Option<u32>, corte: Option<f64>, caja: [f32; 6], claro: bool| {
        let (r, u, f) = cam.ejes();
        let luz = unit(suma(suma(por(f, -0.5), por(u, 0.75)), por(r, 0.35)));
        let ojo = cam.ojo();
        Vista3d {
            mvp: cam.matriz(w, h, radio),
            luz: [luz[0] as f32, luz[1] as f32, luz[2] as f32, 0.0],
            ojo: [ojo[0] as f32, ojo[1] as f32, ojo[2] as f32, 1.0],
            corte: match corte {
                Some(k) => [1.0, caja[2] + (caja[5] - caja[2]) * k as f32, (4.5 * e) as f32, 0.0],
                None => [0.0, 0.0, (4.5 * e) as f32, 0.0],
            },
            elegido: elegido.map_or(0, |e| e + 1),
            claro: if claro { 1.0 } else { 0.0 },
            pantalla: [w as f32, h as f32],
        }
    };
    'bucle: loop {
        let animando = cam != destino;
        // SAFETY: bucle de mensajes de este hilo.
        unsafe {
            if !animando && !sucio && !barra_sucia {
                let espera = if modelo.is_none() || barra_visible || fuera_desde.is_some() || pista_hasta.is_some() { 50 } else { INFINITE };
                let _ = MsgWaitForMultipleObjectsEx(None, espera, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            }
            let mut m = MSG::default();
            while PeekMessageW(&mut m, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&m);
                DispatchMessageW(&m);
            }
        }
        if modelo.is_none() && mensaje == ui.abriendo {
            let llegado = match cargando.try_recv() {
                Ok(r) => Some(r),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(String::new())),
            };
            if let Some(r) = llegado {
                match r {
                    Ok(m) if m.vacio() => mensaje = ui.vacio.clone(),
                    Ok(m) => match gpu.subir_3d(&m) {
                        Ok(g) => {
                            let c = m.caja;
                            radio = ((c[3] - c[0]).hypot(c[4] - c[1]).hypot(c[5] - c[2]) as f64 / 2.0).max(1e-3);
                            caja_vista = m.caja_util();
                            cam = Orbita::encuadrar(caja_vista, w, h);
                            destino = cam;
                            modelo = Some((m, g));
                            mensaje.clear();
                        }
                        Err(err) => mensaje = format!("{}: {err}", ui.error),
                    },
                    Err(err) if err.is_empty() => mensaje = ui.error.clone(),
                    Err(err) => mensaje = format!("{}: {err}", ui.error),
                }
                aviso_ui = None;
                sucio = true;
            }
        }
        let eventos: Vec<(De, Ev)> = ESTADO.with(|s| s.borrow_mut().eventos.drain(..).collect());
        let b_barra = botones_barra(e, BOTONES.len());
        let boton_en = |x: i32| -> Option<usize> { b_barra.iter().position(|(a, b)| (x as f64) >= *a && (x as f64) < *b) };
        let caja = caja_vista;
        let (mut cambiar_tema, mut cambiar_corte, mut cambiar_encima, mut encuadrar, mut planta) = (false, false, false, false, false);
        let en_carril = |x: i32, y: i32, w: u32, h: u32| {
            let (x0, y0, x1, y1) = carril(w, h, e);
            (x as f64) >= x0 - 10.0 * e && (x as f64) <= x1 + 10.0 * e && (y as f64) >= y0 - 10.0 * e && (y as f64) <= y1 + 10.0 * e
        };
        let frac_de = |y: i32, w: u32, h: u32| {
            let (_, y0, _, y1) = carril(w, h, e);
            ((y1 - y as f64) / (y1 - y0)).clamp(0.0, 1.0)
        };
        for (de, ev) in eventos {
            match (de, ev) {
                (_, Ev::Cerrar) => break 'bucle,
                (De::Barra, Ev::Mover(x, _)) => {
                    dentro_barra = true;
                    let b = boton_en(x);
                    if b != boton_encima {
                        boton_encima = b;
                        barra_sucia = true;
                    }
                }
                (De::Barra, Ev::Salir) => {
                    dentro_barra = false;
                    boton_encima = None;
                    barra_sucia = true;
                }
                (De::Barra, Ev::Bajar(x, _, 0)) => match boton_en(x) {
                    Some(0) => {
                        // SAFETY: ventanas propias.
                        unsafe {
                            let _ = ReleaseCapture();
                            SendMessageW(hwnd, WM_NCLBUTTONDOWN, Some(WPARAM(HTCAPTION as usize)), Some(LPARAM(0)));
                        }
                    }
                    Some(1) => cambiar_tema = true,
                    Some(2) => cambiar_corte = true,
                    Some(3) => cambiar_encima = true,
                    _ => {}
                },
                (De::Barra, _) => {}
                (De::Plano, Ev::Tamano(nw, nh)) => {
                    if (nw, nh) != (w, h) {
                        w = nw;
                        h = nh;
                        let _ = gpu.redimensionar(w, h);
                        aviso_ui = None;
                        sucio = true;
                    }
                }
                (De::Plano, Ev::Mover(x, y)) => {
                    dentro_plano = true;
                    if arrastre_corte {
                        corte = Some(frac_de(y, w, h));
                        sucio = true;
                    } else if let Some((x0, y0, c0, b, pivote, _)) = arrastre {
                        let (dx, dy) = ((x - x0) as f64, (y - y0) as f64);
                        if dx.abs() + dy.abs() > 3.0 {
                            if let Some(a) = &mut arrastre {
                                a.5 = true;
                            }
                            // SAFETY: lee el estado de Mayus.
                            let mayus = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                            cam = if b == 0 && !mayus {
                                c0.girar(-dx * 0.008, dy * 0.008, pivote)
                            } else {
                                c0.mover(dx, dy, h)
                            };
                            destino = cam;
                            sucio = true;
                        }
                    }
                }
                (De::Plano, Ev::Salir) => dentro_plano = false,
                (De::Plano, Ev::Bajar(x, y, b)) => {
                    pista_hasta = None;
                    // SAFETY: lee el estado de Ctrl.
                    let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
                    if b == 0 && ctrl {
                        // SAFETY: ventana propia.
                        unsafe {
                            let _ = ReleaseCapture();
                            SendMessageW(hwnd, WM_NCLBUTTONDOWN, Some(WPARAM(HTCAPTION as usize)), Some(LPARAM(0)));
                        }
                    } else if b == 0 && corte.is_some() && en_carril(x, y, w, h) {
                        arrastre_corte = true;
                        corte = Some(frac_de(y, w, h));
                        sucio = true;
                    } else if let Some((_, g)) = &modelo {
                        // Girar alrededor de lo que se toca (o del objetivo).
                        let v = vista(&cam, w, h, radio, elegido, corte, caja, claro);
                        let pivote = if b == 0 { gpu.elegir(g, &v, x, y).ok().flatten().map(|(_, p)| p.map(|c| c as f64)) } else { None };
                        cam = destino;
                        arrastre = Some((x, y, cam, b, pivote.unwrap_or(cam.objetivo), false));
                    }
                }
                (De::Plano, Ev::Subir(x, y, b)) => {
                    arrastre_corte = false;
                    let movido = arrastre.is_some_and(|a| a.5);
                    arrastre = None;
                    if !movido && b == 2 {
                        menu(hwnd, &ui, corte.is_some(), aristas, fijada);
                    } else if !movido && b == 0
                        && let Some((_, g)) = &modelo
                    {
                        let v = vista(&cam, w, h, radio, elegido, corte, caja, claro);
                        let nuevo = gpu.elegir(g, &v, x, y).ok().flatten().map(|(e, _)| e);
                        if nuevo != elegido {
                            elegido = nuevo;
                            sucio = true;
                        }
                    }
                }
                (De::Plano, Ev::Doble(..)) => encuadrar = true,
                (De::Plano, Ev::Rueda(x, y, d)) => {
                    pista_hasta = None;
                    if corte.is_some() && en_carril(x, y, w, h) {
                        let k = corte.unwrap_or(0.5) + d as f64 / 120.0 * 0.02;
                        corte = Some(k.clamp(0.0, 1.0));
                        sucio = true;
                        continue;
                    }
                    let factor = 1.2f64.powf(-(d as f64) / 120.0);
                    // El punto bajo el cursor: el tocado (si se sigue girando
                    // la rueda en el mismo sitio, el de antes).
                    let fijo = match rueda {
                        Some((t, rx, ry, p)) if t.elapsed() < Duration::from_millis(400) && (rx - x).abs() + (ry - y).abs() < 4 => Some(p),
                        _ => None,
                    };
                    let fijo = fijo.or_else(|| {
                        let (_, g) = modelo.as_ref()?;
                        let v = vista(&destino, w, h, radio, elegido, corte, caja, claro);
                        gpu.elegir(g, &v, x, y).ok().flatten().map(|(_, p)| p.map(|c| c as f64))
                    });
                    // Sin nada debajo: el punto del rayo a la distancia del objetivo.
                    let fijo = fijo.unwrap_or_else(|| suma(destino.ojo(), por(destino.rayo(x as f64, y as f64, w, h), destino.dist)));
                    rueda = Some((Instant::now(), x, y, fijo));
                    destino = destino.zoom(factor, fijo);
                }
                (De::Plano, Ev::Menu(id)) => match id {
                    M_TODO => encuadrar = true,
                    M_TEMA => cambiar_tema = true,
                    M_CORTE => cambiar_corte = true,
                    M_ARISTAS => {
                        aristas = !aristas;
                        sucio = true;
                    }
                    M_PLANTA => planta = true,
                    M_ENCIMA => cambiar_encima = true,
                    M_CERRAR => break 'bucle,
                    _ => {}
                },
                (De::Plano, Ev::Tecla(vk)) => match vk {
                    0x1B => {
                        if elegido.is_some() {
                            elegido = None;
                            sucio = true;
                        } else {
                            break 'bucle;
                        }
                    }
                    0x46 | 0x24 => encuadrar = true,
                    0x42 => cambiar_tema = true,
                    0x43 => cambiar_corte = true,
                    0x41 => {
                        aristas = !aristas;
                        sucio = true;
                    }
                    0x50 => planta = true,
                    0x54 => cambiar_encima = true,
                    0xBB | 0x6B => destino = destino.zoom(1.0 / 1.4, destino.objetivo),
                    0xBD | 0x6D => destino = destino.zoom(1.4, destino.objetivo),
                    0x25 => destino = destino.girar(0.26, 0.0, destino.objetivo),
                    0x27 => destino = destino.girar(-0.26, 0.0, destino.objetivo),
                    0x26 => destino = destino.girar(0.0, 0.2, destino.objetivo),
                    0x28 => destino = destino.girar(0.0, -0.2, destino.objetivo),
                    _ => {}
                },
            }
        }
        if encuadrar && modelo.is_some() {
            destino = Orbita::encuadrar(caja, w, h);
        }
        if planta && modelo.is_some() {
            let mut o = Orbita::encuadrar(caja, w, h);
            o.rumbo = -std::f64::consts::FRAC_PI_2;
            o.altura = ALTURA_MAX;
            o.dist = o.distancia_para(caja, w, h).max(1e-3);
            destino = o;
        }
        if cambiar_tema {
            claro = !claro;
            barra_sucia = true;
            aviso_ui = None;
            sucio = true;
        }
        if cambiar_corte {
            corte = if corte.is_some() { None } else { Some(0.5) };
            barra_sucia = true;
            sucio = true;
        }
        if cambiar_encima {
            fijada = !fijada;
            poner_encima(hwnd, fijada);
            barra_sucia = true;
        }
        if pista_hasta.is_some_and(|t| Instant::now() > t) {
            pista_hasta = None;
            sucio = true;
        }
        // La barra, como en los planos.
        let quiere = dentro_plano || dentro_barra || arrastre.is_some();
        if quiere {
            fuera_desde = None;
        } else if barra_visible && fuera_desde.is_none() {
            fuera_desde = Some(Instant::now());
        }
        let mostrar = quiere || fuera_desde.is_some_and(|t| t.elapsed() < Duration::from_millis(600));
        if !mostrar {
            fuera_desde = None;
        }
        if mostrar && modelo.is_some() {
            if barra_visible {
                seguir_barra(hwnd);
            } else {
                let mut r = RECT::default();
                // SAFETY: ventanas propias; estructuras locales.
                unsafe {
                    let _ = GetWindowRect(hwnd, &mut r);
                    let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
                    let _ = GetMonitorInfoW(mon, &mut info);
                    let (x, y) = colocar_barra(r, info.rcWork, bw, bh, (8.0 * e) as i32);
                    let _ = SetWindowPos(hbarra, Some(HWND_TOPMOST), x, y, bw, bh, SWP_NOACTIVATE | SWP_SHOWWINDOW);
                }
                barra_visible = true;
                barra_sucia = true;
            }
        } else if barra_visible {
            barra_visible = false;
            // SAFETY: ventana propia.
            let _ = unsafe { ShowWindow(hbarra, SW_HIDE) };
        }
        if cam != destino {
            let c = cam.hacia(&destino, 0.35);
            cam = if c.cerca_de(&destino) { destino } else { c };
            sucio = true;
        }
        if barra_sucia && barra_visible {
            let m = barra(e, &BOTONES, boton_encima, claro, &[false, false, corte.is_some(), fijada]);
            if let Ok(p) = gpu_barra.subir(&m) {
                let (gw, gh) = gpu_barra.tamano();
                let fondo = if claro { [0.976, 0.976, 0.984, 1.0] } else { [0.118, 0.118, 0.125, 1.0] };
                let _ = gpu_barra.dibujar(fondo, &[(&p, vista_pantalla(&m, gw, gh))]);
            }
            barra_sucia = false;
        }
        if aviso_ui.is_none() && !mensaje.is_empty() {
            let m = aviso(&mut textos, &mensaje, w, h, e, claro);
            aviso_ui = gpu.subir(&m).ok().map(|p| (m, p));
            sucio = true;
        }
        if sucio {
            let fondo = if claro { [0.955, 0.958, 0.965, 1.0] } else { [0.13, 0.15, 0.19, 1.0] };
            let mut capas: Vec<(&PlanoGpu, crate::gpu::Vista)> = Vec::new();
            if mensaje.is_empty() {
                aviso_ui = None;
            } else if let Some((m, p)) = &aviso_ui {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            let datos = Encima {
                elegido: modelo.as_ref().and_then(|(m, _)| m.elementos.get(elegido? as usize)),
                corte: corte.map(|k| (k, modelo.as_ref().map_or(0.0, |(m, _)| m.origen[2] + (caja[2] + (caja[5] - caja[2]) * k as f32) as f64))),
                unidad: modelo.as_ref().map_or("", |(m, _)| unidad_de(m.metros)),
                pista: pista_hasta.is_some().then_some(ui.pista.as_str()),
            };
            let encima = if modelo.is_some() {
                let m = encima_del_modelo(&mut textos, &datos, &ui, w, h, e, claro);
                gpu.subir(&m).ok().map(|p| (m, p))
            } else {
                None
            };
            if let Some((m, p)) = &encima {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            let v = vista(&cam, w, h, radio, elegido, corte, caja, claro);
            let r = gpu.dibujar_3d(fondo, modelo.as_ref().map(|(_, g)| (g, v)), aristas, &capas);
            if let Err(err) = r {
                tracing::warn!(error = %err, "no se pudo dibujar el modelo");
            }
            sucio = false;
        }
    }
    // SAFETY: ventanas propias.
    unsafe {
        let _ = DestroyWindow(hbarra);
        let _ = DestroyWindow(hwnd);
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn proyectar(m: &[[f32; 4]; 4], p: [f64; 3], w: u32, h: u32) -> [f64; 2] {
        let v = [p[0] as f32, p[1] as f32, p[2] as f32, 1.0];
        let c: Vec<f32> = (0..4).map(|j| (0..4).map(|i| v[i] * m[i][j]).sum()).collect();
        [((c[0] / c[3]) as f64 + 1.0) / 2.0 * w as f64, (1.0 - (c[1] / c[3]) as f64) / 2.0 * h as f64]
    }

    #[test]
    fn el_objetivo_cae_en_el_centro_y_la_derecha_a_la_derecha() {
        let o = Orbita { objetivo: [1.0, 2.0, 3.0], rumbo: -0.8, altura: 0.5, dist: 20.0 };
        let m = o.matriz(800, 600, 10.0);
        let c = proyectar(&m, o.objetivo, 800, 600);
        assert!((c[0] - 400.0).abs() < 0.01 && (c[1] - 300.0).abs() < 0.01);
        // Lo que esta mas alto sale mas arriba (y menor en pantalla).
        let arriba = proyectar(&m, [1.0, 2.0, 5.0], 800, 600);
        assert!(arriba[1] < 300.0);
        // Sin espejo: visto desde el sur (rumbo -90°), +X sale a la derecha.
        let s = Orbita { objetivo: [0.0; 3], rumbo: -std::f64::consts::FRAC_PI_2, altura: 0.2, dist: 10.0 };
        let p = proyectar(&s.matriz(800, 600, 10.0), [1.0, 0.0, 0.0], 800, 600);
        assert!(p[0] > 400.0, "{p:?}");
    }

    #[test]
    fn el_zoom_deja_quieto_el_punto_fijo() {
        let o = Orbita { objetivo: [0.0; 3], rumbo: 0.3, altura: 0.4, dist: 50.0 };
        let fijo = [5.0, -3.0, 2.0];
        let antes = proyectar(&o.matriz(800, 600, 30.0), fijo, 800, 600);
        let z = o.zoom(0.5, fijo);
        let despues = proyectar(&z.matriz(800, 600, 30.0), fijo, 800, 600);
        assert!((antes[0] - despues[0]).abs() < 0.05 && (antes[1] - despues[1]).abs() < 0.05);
        assert!((z.dist - 25.0).abs() < 1e-9);
    }

    #[test]
    fn girar_alrededor_de_un_punto_lo_deja_en_su_sitio_de_pantalla_si_es_el_objetivo() {
        let o = Orbita { objetivo: [3.0, 1.0, 0.0], rumbo: 0.3, altura: 0.4, dist: 50.0 };
        let g = o.girar(0.5, 0.2, o.objetivo);
        assert!((g.rumbo - 0.8).abs() < 1e-9 && (g.altura - 0.6).abs() < 1e-9);
        let d = sub(g.objetivo, o.objetivo);
        assert!(punto(d, d).sqrt() < 1e-6, "{:?}", g.objetivo);
        // La altura no pasa de la planta.
        assert!(o.girar(0.0, 5.0, o.objetivo).altura <= ALTURA_MAX);
    }

    #[test]
    fn encuadrar_mete_la_caja_entera() {
        let caja = [0.0, 0.0, 0.0, 40.0, 10.0, 12.0];
        let o = Orbita::encuadrar(caja, 800, 600);
        let m = o.matriz(800, 600, 25.0);
        for i in 0..8 {
            let p = [caja[if i & 1 == 0 { 0 } else { 3 }] as f64, caja[if i & 2 == 0 { 1 } else { 4 }] as f64, caja[if i & 4 == 0 { 2 } else { 5 }] as f64];
            let q = proyectar(&m, p, 800, 600);
            assert!(q[0] >= 0.0 && q[0] <= 800.0 && q[1] >= 0.0 && q[1] <= 600.0, "{q:?}");
        }
    }
}
