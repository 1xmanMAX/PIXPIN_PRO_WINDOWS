//! **Mandar fotos y texto a un lienzo por su nombre** (17-oct-2026).
//!
//! El usuario: «poder ingresar o mandar directamente fotos y texto a un canvas
//! con solo saber su nombre, que sea igual que mandarle a un chat, y que se
//! vayan apilando para que luego pueda organizarlos». Es el pedido `al_lienzo`
//! (`pedidos.rs`): lo mandan Flow Launcher (`p lienzo <nombre> > texto`), la
//! consola (`pixpinmax al-lienzo <nombre> …`) y cualquier script.
//!
//! - **Se apilan**: cada cosa va debajo de la anterior que llego, en una
//!   columna a la derecha de lo que ya habia ([`sitio_en_la_pila`]), y nunca
//!   encima de nada: si el usuario puso algo en la columna, se baja hasta el
//!   primer hueco. Lo apilado se reconoce por su id de fichero (`pila-…`), que
//!   viaja con el `.excalidraw`: la pila sigue igual tras cerrar y abrir.
//! - **En vivo si esta abierto**: el editor que tiene ese lienzo se apunta
//!   ([`abierto`]) y recoge lo suyo en su bucle ([`atender`]); si no esta
//!   abierto, se escribe en su fichero ([`meter_en_disco`]). Las dos cosas van
//!   bajo el mismo cerrojo que el apunte: o lo recoge el editor, o el editor
//!   ya lee el fichero con ello dentro. Lo que el editor no llegara a recoger
//!   se escribe al cerrarse, despues de que el guarde.
//! - **Las mismas piezas de siempre**: el texto nace con `Gesto::pegar_texto`
//!   (como pegar un texto en el lienzo) y la foto con `elemento_imagen`, en el
//!   almacen de imagenes del lienzo; en disco, las fotos van a `imagenes/` con
//!   `guardar_pegadas_en_hoja`, como las pegadas a mano.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pixpin_codec::ImagenRgba;
use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::Elemento;
use pixpin_motor2d::vector::Punto2;

use crate::imagenes_lienzo::ImagenesLienzo;

/// El principio del id de fichero de lo apilado.
pub const PREFIJO: &str = "pila-";
/// Lo mas ancho que entra una foto o un texto en la pila.
pub const ANCHO_COLUMNA: f32 = 480.0;
/// Entre dos cosas de la pila.
pub const HUECO: f32 = 24.0;
/// Entre lo que ya habia y la columna nueva.
pub const AIRE_COLUMNA: f32 = 80.0;

/// Lo que se manda: un texto y/o unas fotos.
#[derive(Debug, Clone, Default)]
pub struct Envio {
    pub texto: Option<String>,
    pub imagenes: Vec<ImagenRgba>,
}

impl Envio {
    pub fn vacio(&self) -> bool {
        self.texto.as_deref().is_none_or(|t| t.trim().is_empty()) && self.imagenes.is_empty()
    }
}

fn es_de_la_pila(e: &Elemento) -> bool {
    e.extras.id_de_fichero.as_deref().is_some_and(|i| i.starts_with(PREFIJO))
}

fn finita(c: (f32, f32, f32, f32)) -> bool {
    [c.0, c.1, c.2, c.3].iter().all(|v| v.is_finite())
}

/// **Donde va lo siguiente de la pila** (su esquina de arriba a la
/// izquierda), de tamano `tam`: debajo de lo ultimo apilado o, la primera
/// vez, en una columna a la derecha de todo lo que hay; bajando hasta que no
/// pise nada. `excepto`: un elemento que no cuenta (el que se esta colocando).
pub fn sitio_en_la_pila(escena: &Escena, tam: (f32, f32), excepto: Option<u64>) -> (f32, f32) {
    let (w, h) = (tam.0.max(1.0), tam.1.max(1.0));
    let cajas: Vec<(f32, f32, f32, f32)> = escena
        .visibles()
        .filter(|e| Some(e.id) != excepto)
        .map(Elemento::caja)
        .filter(|c| finita(*c))
        .collect();
    let ultima = escena
        .visibles()
        .filter(|e| Some(e.id) != excepto && es_de_la_pila(e))
        .last()
        .map(Elemento::caja)
        .filter(|c| finita(*c));
    let (x, mut y) = match ultima {
        Some(u) => (u.0, u.3 + HUECO),
        None => match cajas.iter().copied().reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))) {
            Some(t) => (t.2 + AIRE_COLUMNA, t.1),
            None => (0.0, 0.0),
        },
    };
    // Sin pisar nada: se baja por debajo de lo que estorbe.
    for _ in 0..10_000 {
        let estorba = cajas
            .iter()
            .filter(|c| c.0 < x + w + HUECO / 2.0 && c.2 > x - HUECO / 2.0 && c.1 < y + h + HUECO / 2.0 && c.3 > y - HUECO / 2.0)
            .map(|c| c.3)
            .reduce(f32::max);
        match estorba {
            Some(abajo) => y = abajo + HUECO,
            None => break,
        }
    }
    (x, y)
}

/// El tamano con que entra una foto: el suyo, o reducida a lo ancho de la
/// columna.
pub fn tamano_en_la_pila(ancho: u32, alto: u32) -> (f32, f32) {
    let (w, h) = (ancho.max(1) as f32, alto.max(1) as f32);
    let f = (ANCHO_COLUMNA / w).min(1.0);
    (w * f, h * f)
}

/// **Apila lo enviado en la escena**: las fotos una debajo de otra y el texto
/// debajo (como el pie de una foto en el chat), todo en un paso de deshacer.
/// Devuelve los ids de lo que entro.
pub fn apilar(escena: &mut Escena, envio: &Envio, imagenes: &mut ImagenesLienzo, ahora: i64) -> Vec<u64> {
    let antes = escena.pasos_cerrados();
    let mut ids = Vec::new();
    let mut n = 0u32;
    let mut id_nuevo = || {
        n += 1;
        format!("{PREFIJO}{ahora:x}-{n}")
    };
    for img in &envio.imagenes {
        let (w, h) = tamano_en_la_pila(img.ancho, img.alto);
        let Some(objeto) = imagenes.guardar(img.clone()) else { continue };
        let (x, y) = sitio_en_la_pila(escena, (w, h), None);
        let mut e = crate::imagenes_lienzo::elemento_imagen(objeto, x, y, w, h);
        e.extras.id_de_fichero = Some(id_nuevo());
        escena.abrir_paso();
        ids.push(escena.anadir(e));
        escena.cerrar_paso();
    }
    if let Some(t) = envio.texto.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        // Como pegar un texto en el lienzo, con un gesto de paso (el estilo
        // de fabrica): nace en cualquier sitio y luego se lleva a la pila.
        let mut g = pixpin_motor2d::gesto::Gesto::nuevo();
        if let Some(id) = g.pegar_texto(escena, t, Punto2::nuevo(0.0, 0.0), ANCHO_COLUMNA) {
            let c = escena.buscar(id).map(Elemento::caja).unwrap_or((0.0, 0.0, 1.0, 1.0));
            let (x, y) = sitio_en_la_pila(escena, (c.2 - c.0, c.3 - c.1), Some(id));
            escena.abrir_paso();
            escena.apuntar_edicion(id);
            let marca = id_nuevo();
            if let Some(e) = escena.buscar_mut(id) {
                e.x += x - c.0;
                e.y += y - c.1;
                e.extras.id_de_fichero = Some(marca);
                e.tocar();
            }
            escena.cerrar_paso();
            ids.push(id);
        }
    }
    if escena.pasos_cerrados().saturating_sub(antes) >= 2 {
        escena.juntar_pasos(antes);
    }
    ids
}

/// **Lo apila en el `.excalidraw` del lienzo cerrado**, con sus fotos en
/// `imagenes/` del proyecto. Devuelve cuantas cosas entraron.
pub fn meter_en_disco(raiz: &Path, proyecto: &str, dibujo: &str, envio: &Envio, ahora: i64) -> Result<usize, String> {
    let ruta = pixpin_proyecto::almacen::lienzo(raiz, proyecto, dibujo);
    let texto = std::fs::read_to_string(&ruta).map_err(|e| format!("no se pudo leer el lienzo: {e}"))?;
    let lienzo = pixpin_motor2d::excalidraw::leer(&texto).map_err(|e| format!("el lienzo no se entiende: {e}"))?;
    let mut escena = pixpin_motor2d::excalidraw::a_escena(&lienzo);
    let mut imagenes = ImagenesLienzo::nuevo(16_384);
    let ids = apilar(&mut escena, envio, &mut imagenes, ahora);
    if ids.is_empty() {
        return Ok(0);
    }
    let nuevas = imagenes.tomar_nuevas();
    let lienzo = crate::imagenes_lienzo::guardar_pegadas_en_hoja(
        &pixpin_proyecto::almacen::carpeta(raiz, proyecto),
        &lienzo,
        &escena,
        &nuevas,
        ahora,
    )
    .map_err(|e| format!("no se pudieron guardar las fotos: {e}"))?;
    if !crate::ventana_chat::guardar_hoja_dibujada(&ruta, &lienzo, &escena) {
        return Err("no se pudo guardar el lienzo".into());
    }
    Ok(ids.len())
}

// ---------------------------------------------------------------------------
// Los lienzos abiertos y lo que espera a que lo recojan

struct Abierto {
    n: u64,
    raiz: PathBuf,
    proyecto: String,
    dibujo: String,
    hwnd: isize,
}

struct Entrega {
    proyecto: String,
    dibujo: String,
    envio: Envio,
}

#[derive(Default)]
struct Estado {
    siguiente: u64,
    abiertos: Vec<Abierto>,
    entregas: Vec<Entrega>,
}

static ESTADO: Mutex<Estado> = Mutex::new(Estado { siguiente: 0, abiertos: Vec::new(), entregas: Vec::new() });

/// Mientras vive, ese lienzo esta abierto en un editor. Se suelta despues de
/// guardarlo: lo que quedara sin recoger va entonces a su fichero.
pub struct Apunte(u64);

/// Apunta que el lienzo `dibujo` del proyecto se va a abrir en este hilo.
/// Hay que pedirlo ANTES de leer su fichero.
pub fn abierto(raiz: &Path, proyecto: &str, dibujo: &str) -> Apunte {
    let mut g = ESTADO.lock().unwrap_or_else(|e| e.into_inner());
    g.siguiente += 1;
    let n = g.siguiente;
    g.abiertos.push(Abierto { n, raiz: raiz.to_path_buf(), proyecto: proyecto.into(), dibujo: dibujo.into(), hwnd: 0 });
    Apunte(n)
}

impl Drop for Apunte {
    fn drop(&mut self) {
        let mut g = ESTADO.lock().unwrap_or_else(|e| e.into_inner());
        let Some(i) = g.abiertos.iter().position(|a| a.n == self.0) else { return };
        let a = g.abiertos.remove(i);
        // Si hay otro editor con el mismo lienzo, lo suyo sigue esperandole.
        if g.abiertos.iter().any(|b| b.proyecto == a.proyecto && b.dibujo == a.dibujo) {
            return;
        }
        let (suyas, otras): (Vec<Entrega>, Vec<Entrega>) =
            std::mem::take(&mut g.entregas).into_iter().partition(|e| e.proyecto == a.proyecto && e.dibujo == a.dibujo);
        g.entregas = otras;
        for e in suyas {
            if let Err(motivo) = meter_en_disco(&a.raiz, &a.proyecto, &a.dibujo, &e.envio, pixpin_shell::entorno::ahora_utc_ms()) {
                tracing::warn!(%motivo, "lo mandado al lienzo no se pudo escribir al cerrarlo");
            }
        }
    }
}

/// Adonde fue lo mandado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Donde {
    /// Al lienzo abierto, que lo recoge en su bucle.
    EnVivo,
    /// A su fichero: `n` cosas.
    EnDisco(usize),
}

/// **Manda lo enviado al lienzo**: en vivo si esta abierto, si no a su
/// fichero (bajo el mismo cerrojo, ver la cabecera).
pub fn mandar(raiz: &Path, proyecto: &str, dibujo: &str, envio: Envio, ahora: i64) -> Result<Donde, String> {
    let mut g = ESTADO.lock().unwrap_or_else(|e| e.into_inner());
    let hwnds: Vec<isize> = g.abiertos.iter().filter(|a| a.proyecto == proyecto && a.dibujo == dibujo).map(|a| a.hwnd).collect();
    if hwnds.is_empty() {
        return meter_en_disco(raiz, proyecto, dibujo, &envio, ahora).map(Donde::EnDisco);
    }
    g.entregas.push(Entrega { proyecto: proyecto.into(), dibujo: dibujo.into(), envio });
    drop(g);
    for h in hwnds.into_iter().filter(|h| *h != 0) {
        pixpin_shell::overlay::despertar(h);
    }
    Ok(Donde::EnVivo)
}

/// **Lo recoge el editor `hwnd`** en su bucle: lo mandado a la hoja que tiene
/// (`zona_al_chat::hoja_abierta`), apilado en su escena. Devuelve los ids.
pub fn atender(hwnd: isize, escena: &mut Escena, imagenes: &mut ImagenesLienzo) -> Vec<u64> {
    let Some(h) = crate::zona_al_chat::hoja_abierta() else { return Vec::new() };
    atender_hoja(&h.proyecto, &h.dibujo, hwnd, escena, imagenes)
}

/// Lo mismo, diciendo de que hoja es (las pruebas).
pub fn atender_hoja(proyecto: &str, dibujo: &str, hwnd: isize, escena: &mut Escena, imagenes: &mut ImagenesLienzo) -> Vec<u64> {
    let suyas: Vec<Entrega> = {
        let mut g = ESTADO.lock().unwrap_or_else(|e| e.into_inner());
        for a in g.abiertos.iter_mut().filter(|a| a.proyecto == proyecto && a.dibujo == dibujo && a.hwnd == 0) {
            a.hwnd = hwnd;
        }
        if g.entregas.is_empty() {
            return Vec::new();
        }
        let (suyas, otras): (Vec<Entrega>, Vec<Entrega>) =
            std::mem::take(&mut g.entregas).into_iter().partition(|e| e.proyecto == proyecto && e.dibujo == dibujo);
        g.entregas = otras;
        suyas
    };
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    suyas.iter().flat_map(|e| apilar(escena, &e.envio, imagenes, ahora)).collect()
}

/// La caja de unos elementos, para llevar la vista hasta ellos.
pub fn caja_de(escena: &Escena, ids: &[u64]) -> Option<(f32, f32, f32, f32)> {
    ids.iter()
        .filter_map(|id| escena.buscar(*id))
        .map(Elemento::caja)
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
}

// ---------------------------------------------------------------------------
// El lienzo por su nombre

/// **El lienzo que se llama `nombre`**: en el proyecto dicho o, sin el, en
/// todos (el tocado mas tarde si hay varios). `(proyecto, referencia,
/// nombre)`. Sin ninguno que se llame asi, `None`.
pub fn buscar(raiz: &Path, proyecto: Option<&str>, nombre: &str) -> Option<(String, String, String)> {
    use pixpin_proyecto::{almacen, cuaderno};
    let ids: Vec<String> = match proyecto.map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => vec![p.to_string()],
        None => almacen::Indice::leer_si_esta(raiz).ok()?.proyectos.into_iter().map(|f| f.id).collect(),
    };
    let mut mejor: Option<(i64, bool, String, String, String)> = None;
    for id in ids {
        let Ok(c) = cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, &id)) else { continue };
        let Some(i) = crate::asistente::chat::lienzo_por_nombre(&c.mensajes, nombre) else { continue };
        let m = &c.mensajes[i];
        let nombre_suyo = crate::asistente::chat::nombre_de_lienzo(m);
        // Mejor el que se llama justo asi que el que solo lo contiene.
        let exacto = crate::asistente::chat::lienzo_por_nombre(std::slice::from_ref(m), nombre).is_some()
            && normal(&nombre_suyo) == normal(nombre);
        let Some(r) = m.referencia.clone() else { continue };
        let gana = mejor.as_ref().is_none_or(|(cuando, ex, ..)| (exacto, m.cuando) > (*ex, *cuando));
        if gana {
            mejor = Some((m.cuando, exacto, id, r, nombre_suyo));
        }
    }
    mejor.map(|(_, _, p, r, n)| (p, r, n))
}

fn normal(s: &str) -> String {
    crate::asistente::chat::nombre_normal(s)
}

#[cfg(test)]
mod pruebas;
