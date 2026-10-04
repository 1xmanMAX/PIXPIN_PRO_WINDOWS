//! **La nota dentro de la aplicacion**: lo que el editor le pide a PixPin
//! para las fotos y las hojas de los proyectos, sin saber nada de
//! proyectos (eso es de la aplicacion, `apps/pixpin/src/notas_md/`).
//!
//! - **Paginas vivas**: una hoja de un proyecto (lienzo, pagina de PDF
//!   anotada, tabla, otra nota) metida como imagen que se actualiza. La
//!   aplicacion la pinta en `notas/vivo-<hoja>.png` y da el renglon; el
//!   editor la ensena como cualquier foto y le pregunta cada poco si alguna
//!   se volvio a pintar (`vigilar`) o perdio su hoja.
//! - **Enlaces a hojas** (`pixpin:hoja=`): Ctrl+clic los abre.
//! - **Abrir**: doble clic en una foto la ensena en grande; en una pagina
//!   viva, abre su hoja en su editor.
//! - **Pendientes**: lo que se mando a esta nota desde fuera («Insertar en
//!   una nota» del chat).
//! - **Medios**: documentos, mensajes del chat, hojas enlazadas y audios
//!   con su transcripcion, pintados como en el chat (`incrustados`).
//!
//! Todo es opcional: sin aplicacion detras (las pruebas) el editor sigue
//! funcionando y esas entradas no hacen nada.

use std::path::PathBuf;

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::{DragFinish, DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PCWSTR};

/// Los textos de las fotos y las paginas vivas.
#[derive(Debug, Clone, Default)]
pub struct RotulosFotos {
    /// El nombre de una imagen pegada («Captura»).
    pub captura: String,
    pub ver_grande: String,
    pub abrir_hoja: String,
    pub pequena: String,
    pub mediana: String,
    pub grande: String,
    /// «Al ancho de la columna»: quita el ancho puesto.
    pub columna: String,
    pub quitar: String,
    /// El aviso de una pagina viva cuya hoja ya no esta.
    pub hoja_borrada: String,
    pub otros_proyectos: String,
    pub sin_hojas: String,
    pub solo_imagenes: String,
}

/// Una hoja que se puede meter: la clave con que la aplicacion la
/// reconoce y su nombre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HojaElegible {
    pub clave: String,
    pub nombre: String,
}

/// Las hojas de un proyecto, para el selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrupoDeHojas {
    pub proyecto: String,
    /// El proyecto de la nota: sus hojas van arriba, sin submenu.
    pub propio: bool,
    pub hojas: Vec<HojaElegible>,
}

/// Lo que cambio en una pagina viva desde la ultima vez que se pregunto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Viva {
    /// Su PNG se volvio a pintar (o aparecio): hay que releerlo.
    Renovada,
    /// Su hoja ya no esta: se queda la ultima copia, con un aviso.
    SinHoja,
}

/// Lo que la aplicacion pone para que la nota este integrada.
// Cada cierre se lee mejor con su firma a la vista que tras un alias.
#[allow(clippy::type_complexity)]
#[derive(Default)]
pub struct Integracion {
    /// Las hojas que se pueden meter, por proyecto.
    pub hojas: Option<Box<dyn FnMut() -> Vec<GrupoDeHojas>>>,
    /// Mete la hoja `clave`: da el renglon de la pagina viva, o el enlace a
    /// ella si el segundo es `true`.
    pub insertar_hoja: Option<Box<dyn FnMut(&str, bool) -> Option<String>>>,
    /// Le pasa las rutas de las paginas vivas de la nota y devuelve lo que
    /// cambio desde la ultima vez. No se queda esperando: lo pesado (pintar
    /// las hojas) lo hace la aplicacion en otro hilo.
    pub vigilar: Option<Box<dyn FnMut(&[String]) -> Vec<(String, Viva)>>>,
    /// Abre una foto en grande (por su ruta del Markdown) o una hoja (por
    /// la ruta de su pagina viva o por un enlace `pixpin:hoja=`).
    pub abrir: Option<Box<dyn FnMut(&str)>>,
    /// Renglones que se mandaron a esta nota desde fuera.
    pub pendientes: Option<Box<dyn FnMut() -> Vec<String>>>,
    /// El fichero local de la letra y el tamano con que se leen las notas
    /// (ver `vista`): no va en el `.md`. `None`: se elige pero no se guarda.
    pub ajustes_vista: Option<std::path::PathBuf>,
    /// Como se llama esta nota en ese fichero (para «solo en esta nota»);
    /// se pregunta al guardar, que una nota nueva cambia de sitio.
    pub clave_vista: Option<Box<dyn Fn() -> String>>,
    /// Documentos, mensajes del chat, hojas enlazadas y audios (ver
    /// `incrustados`). Sin ellos, esos renglones se quedan como texto.
    pub medios: Option<Box<dyn crate::incrustados::Medios>>,
}

/// El toque que la aplicacion le manda a una nota abierta para que mire ya
/// sus paginas vivas y lo que se le mando (`PostMessage` a su ventana).
pub const WM_VIVAS: u32 = WM_APP + 0x31;

/// Da el toque [`WM_VIVAS`] a la nota de la ventana `hwnd` (de otro hilo):
/// que mire ya lo que se le mando. Si la ventana ya no existe, no pasa nada.
pub fn tocar(hwnd: isize) {
    // SAFETY: un mensaje encolado a una ventana; si ya no existe, falla sin
    // dano.
    unsafe {
        let _ = PostMessageW(
            Some(HWND(hwnd as *mut _)),
            WM_VIVAS,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        );
    }
}

/// El primer id de las hojas en el menu del selector.
pub const ID_HOJA: u32 = 0x4000;

/// Lo que va en el selector, en orden: las hojas del proyecto propio y
/// luego cada proyecto con las suyas. La posicion de cada hoja en esta
/// lista es su id menos [`ID_HOJA`].
pub fn claves_del_selector(grupos: &[GrupoDeHojas]) -> Vec<&str> {
    let propios = grupos.iter().filter(|g| g.propio);
    let otros = grupos.iter().filter(|g| !g.propio);
    propios
        .chain(otros)
        .flat_map(|g| g.hojas.iter().map(|h| h.clave.as_str()))
        .collect()
}

/// Ensena el selector de hojas en `punto` (pantalla) y da la clave de la
/// elegida. Un menu de Windows con un submenu por proyecto: no hace falta
/// mas para elegir entre unas decenas de hojas, y se maneja con el teclado.
pub fn elegir_hoja(
    marco: HWND,
    grupos: &[GrupoDeHojas],
    r: &RotulosFotos,
    punto: POINT,
) -> Option<String> {
    let claves = claves_del_selector(grupos);
    if claves.is_empty() {
        return None;
    }
    // SAFETY: los menus se crean y se destruyen aqui (el principal destruye
    // sus submenus); las cadenas viven durante cada llamada; ventana propia.
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        let mut id = ID_HOJA;
        let poner = |m: HMENU, g: &GrupoDeHojas, id: &mut u32| {
            for h in &g.hojas {
                let _ = AppendMenuW(
                    m,
                    MF_STRING,
                    *id as usize,
                    &HSTRING::from(h.nombre.as_str()),
                );
                *id += 1;
            }
        };
        for g in grupos.iter().filter(|g| g.propio) {
            poner(menu, g, &mut id);
        }
        let otros: Vec<&GrupoDeHojas> = grupos
            .iter()
            .filter(|g| !g.propio && !g.hojas.is_empty())
            .collect();
        if !otros.is_empty() {
            if id > ID_HOJA {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            }
            let sub = CreatePopupMenu().ok()?;
            for g in otros {
                let deste = CreatePopupMenu().ok()?;
                poner(deste, g, &mut id);
                let _ = AppendMenuW(
                    sub,
                    MF_POPUP,
                    deste.0 as usize,
                    &HSTRING::from(g.proyecto.as_str()),
                );
            }
            let _ = AppendMenuW(
                menu,
                MF_POPUP,
                sub.0 as usize,
                &HSTRING::from(r.otros_proyectos.as_str()),
            );
        }
        let elegido = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            punto.x,
            punto.y,
            None,
            marco,
            None,
        );
        let _ = DestroyMenu(menu);
        let n = (elegido.0 as u32).checked_sub(ID_HOJA)? as usize;
        claves.get(n).map(|c| c.to_string())
    }
}

/// Los ficheros de un arrastre (`WM_DROPFILES` o `EN_DROPFILES`).
/// `soltar` lo libera: con `WM_DROPFILES` es de quien lo recibe.
pub fn rutas_de_hdrop(h: HDROP, soltar: bool) -> Vec<PathBuf> {
    let mut v = Vec::new();
    // SAFETY: `h` es el arrastre que dio Windows; cada nombre se lee en un
    // bufer del tamano que dice el propio arrastre.
    unsafe {
        let cuantos = DragQueryFileW(h, u32::MAX, None);
        for i in 0..cuantos {
            let largo = DragQueryFileW(h, i, None) as usize;
            let mut b = vec![0u16; largo + 1];
            let n = DragQueryFileW(h, i, Some(&mut b)) as usize;
            v.push(PathBuf::from(String::from_utf16_lossy(&b[..n.min(largo)])));
        }
        if soltar {
            DragFinish(h);
        }
    }
    v
}

/// Una imagen del portapapeles escrita en un PNG temporal con `nombre`,
/// para meterla como cualquier foto (se copia junto a la nota y el
/// temporal sobra).
pub fn png_temporal(img: &pixpin_codec::ImagenRgba, nombre: &str) -> Option<PathBuf> {
    let carpeta = std::env::temp_dir()
        .join("PixPin")
        .join("notas")
        .join(format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
    std::fs::create_dir_all(&carpeta).ok()?;
    let ruta = carpeta.join(format!("{nombre}.png"));
    let png = pixpin_codec::imagen::codificar_png(img).ok()?;
    std::fs::write(&ruta, png).ok()?;
    Some(ruta)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn grupo(p: &str, propio: bool, hojas: &[&str]) -> GrupoDeHojas {
        GrupoDeHojas {
            proyecto: p.into(),
            propio,
            hojas: hojas
                .iter()
                .map(|h| HojaElegible {
                    clave: format!("{p}/{h}"),
                    nombre: h.to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn el_selector_pone_primero_las_hojas_del_proyecto_de_la_nota() {
        let g = vec![
            grupo("Otra", false, &["a"]),
            grupo("Casa", true, &["planta", "alzado"]),
        ];
        assert_eq!(
            claves_del_selector(&g),
            vec!["Casa/planta", "Casa/alzado", "Otra/a"]
        );
        // Caso negativo: sin hojas, nada que elegir.
        assert!(claves_del_selector(&[grupo("Vacia", true, &[])]).is_empty());
    }

    #[test]
    fn una_imagen_pegada_se_escribe_en_un_png_que_se_puede_leer() {
        let img = pixpin_codec::ImagenRgba {
            ancho: 3,
            alto: 2,
            pixeles: vec![200; 24],
        };
        let r = png_temporal(&img, "Captura").unwrap();
        assert_eq!(r.file_name().unwrap(), "Captura.png");
        let leida = pixpin_codec::imagen::cargar(&r).unwrap();
        assert_eq!((leida.ancho, leida.alto), (3, 2));
        let _ = std::fs::remove_dir_all(r.parent().unwrap());
    }
}
