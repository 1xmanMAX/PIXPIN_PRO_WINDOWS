//! **Imprimir el PDF entero** (27-oct-2026, el usuario: «el clic derecho en
//! el boton del marco: imprimir los marcos o todo el PDF»): el dialogo de
//! imprimir de Windows con todas las hojas y lo anotado encima, como el de
//! un Word (`office::imprimir`).
//!
//! Cada hoja se dibuja al pintarla en el papel, a la resolucion que pide la
//! impresora (o la vista previa), con `Windows.Data.Pdf` como en la
//! ventana: asi un PDF de cien hojas no se queda entero en la memoria. El
//! PDF se abre en el hilo que pinta y se suelta alli mismo
//! (`pixpin_pdf::Documento` no cambia de hilo), y la ultima hoja dibujada
//! se guarda por si la vista previa la pide otra vez.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use pixpin_docs::vista;
use pixpin_render::imprimir_moderno::{self as moderno, Documento, Pagina};
use pixpin_render::{Color, MotorRender, Pintor, RectF};
use pixpin_store::Catalogo;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use super::{Estado, ahora_ms, guardar_capas, office::encajar_en_el_papel};
use crate::lector_tinta::{self, Capa, Tinta};

/// Lo que se imprime: las hojas del PDF (su medida en puntos) y su tinta.
struct PdfImpreso {
    ruta: PathBuf,
    medidas: Vec<(f32, f32)>,
    capas: Vec<Option<Capa>>,
    tinta: RefCell<Tinta>,
    /// La ultima hoja dibujada: su numero, su ancho en pixeles, el motor
    /// para el que se subio (su direccion) y el mapa.
    ultima: RefCell<Option<(usize, u32, usize, ID2D1Bitmap1)>>,
}

impl PdfImpreso {
    /// El dibujo de la hoja `i` a `ancho` pixeles, subido a `motor`.
    fn bitmap(&self, i: usize, ancho: u32, motor: &MotorRender) -> Option<ID2D1Bitmap1> {
        let clave = motor as *const MotorRender as usize;
        if let Some((j, w, m, b)) = self.ultima.borrow().as_ref()
            && (*j, *w, *m) == (i, ancho, clave)
        {
            return Some(b.clone());
        }
        // Abierto y soltado aqui: el PDF de WinRT vive en el hilo que pinta.
        let doc = pixpin_pdf::Documento::abrir(&self.ruta)
            .inspect_err(|err| tracing::warn!(?err, "pdf entero: no se abre para imprimir"))
            .ok()?;
        let img = doc
            .renderizar(i as u32, ancho)
            .inspect_err(|err| tracing::warn!(?err, hoja = i, "pdf entero: sin dibujo de la hoja"))
            .ok()?;
        let b = motor
            .bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles)
            .ok()?;
        *self.ultima.borrow_mut() = Some((i, ancho, clave, b.clone()));
        Some(b)
    }
}

impl Documento for PdfImpreso {
    fn paginar(&mut self, _motor: &MotorRender, _alcance: Option<&str>) -> usize {
        self.medidas.len()
    }

    fn pintar(&self, i: usize, hoja: Pagina, p: &Pintor) {
        let Some(&(pw, ph)) = self.medidas.get(i) else {
            return;
        };
        let (pw, ph) = (pw.max(1.0), ph.max(1.0));
        let (x, y, k) = encajar_en_el_papel((pw, ph), hoja.papel);
        // Los pixeles de la hoja en el papel, a la escala de quien pinta.
        let ancho = ((pw * k * hoja.escala).round() as u32).clamp(64, pixpin_pdf::ANCHO_MAXIMO);
        let destino = RectF {
            x: x * hoja.escala,
            y: y * hoja.escala,
            ancho: pw * k * hoja.escala,
            alto: ph * k * hoja.escala,
        };
        if let Some(b) = self.bitmap(i, ancho, p.motor()) {
            p.bitmap(&b, destino, None, false);
        }
        // La tinta, en las unidades del lector (la hoja mide `ANCHO_HOJA`).
        let Some(Some(capa)) = self.capas.get(i) else {
            return;
        };
        let unidades = k * pw / vista::ANCHO_HOJA * hoja.escala;
        let alto = vista::ANCHO_HOJA * ph / pw;
        p.poner_vista((destino.x, destino.y), unidades, (0.0, 0.0));
        p.con_recorte(
            RectF {
                x: 0.0,
                y: 0.0,
                ancho: vista::ANCHO_HOJA,
                alto,
            },
            |p| {
                self.tinta.borrow_mut().pintar_capa(
                    p,
                    i,
                    capa,
                    (0.0, 0.0, vista::ANCHO_HOJA, alto),
                    unidades,
                    lector_tinta::papel(Color::BLANCO),
                    false,
                );
            },
        );
    }
}

/// **El dialogo de imprimir con el PDF entero** y lo anotado (lo pendiente
/// se guarda antes). Si no se abre, el aviso.
pub(super) fn imprimir(e: &mut Estado, ruta: &Path, textos: &Catalogo) {
    if e.medidas.is_empty() {
        return;
    }
    guardar_capas(e, ruta);
    let capas = e
        .medidas
        .iter()
        .enumerate()
        .map(|(i, &(pw, ph))| {
            let alto = vista::ANCHO_HOJA * ph.max(1.0) / pw.max(1.0);
            e.donde
                .para_leer(ruta, i)
                .is_file()
                .then(|| e.donde.leer_capa(ruta, i, e.ajustes.espacios, alto))
                .filter(|c| !c.vacia())
        })
        .collect();
    let doc = PdfImpreso {
        ruta: ruta.to_path_buf(),
        medidas: e.medidas.clone(),
        capas,
        tinta: RefCell::new(Tinta::nueva()),
        ultima: RefCell::new(None),
    };
    let titulo = pixpin_docs::sin_extension(&pixpin_docs::nombre(ruta));
    let apaisada = e.medidas.first().is_some_and(|&(w, h)| w > h);
    let hecho = (|| -> Result<()> {
        let d = pixpin_capture::Dispositivo::nuevo().context("sin dispositivo para imprimir")?;
        let motor = MotorRender::nuevo(d.d3d()).context("sin motor para imprimir")?;
        let pedido = moderno::Pedido {
            titulo,
            apaisada,
            opcion: None,
            ppp_previa: 144.0,
        };
        moderno::mostrar(
            windows::Win32::Foundation::HWND(e.hwnd as *mut _),
            pedido,
            d.d3d().clone(),
            motor,
            Box::new(doc),
        )
        .context("el dialogo de imprimir no se abrio")
    })();
    if let Err(err) = hecho {
        tracing::warn!(?err, "pdf entero: no se pudo imprimir");
        e.aviso = Some((textos.t("marcos-menu-no-imprime"), ahora_ms() + 4000));
    }
}
