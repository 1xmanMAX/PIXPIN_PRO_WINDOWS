//! **Los marcos numerados de los pines de imagen** (19-oct-2026), con la
//! logica del plano y de los demas visores (`marcos_doc`): el marco del riel
//! o la **I** entra y sale del modo, arrastrar pone uno con su numero, Supr
//! quita el ultimo y **Ctrl+P** los imprime en grupo, una hoja por marco.
//!
//! El pin los pinta y recoge el gesto (`pixpin_pin::PedidoMarco`); aqui se
//! llevan con `marcos_doc::Marcos`, que los guarda al lado de la imagen del
//! almacen (`<imagen>.pixpin-marcos`). Van en pixeles de esa imagen, sin
//! girar: se quedan en su sitio de la foto aunque el pin se agrande, se
//! acerque por dentro o se gire.
//!
//! Imprimir no fotografia la pantalla como los visores: cada marco se
//! recorta de la imagen entera del almacen (con lo anotado encima, horneado
//! a su tamano real por el pin), asi que sale con todos sus pixeles aunque
//! el pin se vea pequeno. Sin marcos, Ctrl+P hace lo de siempre (elegir una
//! zona del pin e imprimirla).

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_pin::PedidoMarco;

use super::{Pines, cargar, textos};
use crate::marcos_doc::Marcos;

impl Pines {
    /// La imagen del almacen de un pin de foto (la de resolucion completa):
    /// al lado van sus marcos.
    fn imagen_del_pin(&self, id: u64) -> Option<std::path::PathBuf> {
        let a = self.almacen.borrow();
        let e = a.entradas().iter().find(|e| e.id == id)?;
        (e.tipo == pixpin_store::TipoEntrada::Imagen && !e.objeto.is_empty()).then(|| a.ruta_objeto(e))
    }

    /// Al nacer el pin (o volver al arrancar): sus marcos, si tiene, ya
    /// pintados en su sitio.
    pub(super) fn cargar_marcos(&mut self, id: u64) {
        if !self.vivos.get(&id).is_some_and(|p| p.admite_marcos()) {
            return;
        }
        let Some(imagen) = self.imagen_del_pin(id) else {
            return;
        };
        let marcos = Marcos::de(Some(&imagen));
        if !marcos.lista.is_empty() {
            self.ensenar_marcos(id, &marcos);
        }
        self.marcos.insert(id, marcos);
    }

    /// Le pasa al pin la lista y el modo.
    fn ensenar_marcos(&self, id: u64, m: &Marcos) {
        if let Some(pin) = self.vivos.get(&id) {
            let lista: Vec<[f64; 4]> = m.lista.iter().map(|m| m.caja).collect();
            pin.poner_marcos(lista, m.activo, &crate::marcos_doc::pista(textos()));
        }
    }

    /// Sale del modo de los marcos de `id` (al entrar a anotar: los dos
    /// modos no van a la vez).
    pub(super) fn salir_de_marcos(&mut self, id: u64) {
        let Some(m) = self.marcos.get_mut(&id) else {
            return;
        };
        if m.activo {
            m.alternar();
            let m = m.clone();
            self.ensenar_marcos(id, &m);
        }
    }

    /// Lo que pidio el pin sobre sus marcos.
    pub(super) fn pedido_de_marcos(&mut self, id: u64, pedido: PedidoMarco) -> Result<()> {
        if !self.marcos.contains_key(&id) {
            // Un pin que nacio antes de tener imagen en el almacen, o uno
            // que no es una foto: se intenta una vez mas.
            self.cargar_marcos(id);
        }
        if pedido == PedidoMarco::Imprimir {
            return self.imprimir_marcos(id);
        }
        // Entrar en el modo saca de anotar: no van a la vez.
        let entrando = pedido == PedidoMarco::Alternar && self.marcos.get(&id).is_some_and(|m| !m.activo);
        if entrando && self.anotacion.as_ref().is_some_and(|a| a.id == id) {
            self.salir_de_anotar()?;
        }
        let Some(m) = self.marcos.get_mut(&id) else {
            return Ok(());
        };
        match pedido {
            PedidoMarco::Alternar => m.alternar(),
            PedidoMarco::QuitarUltimo => {
                m.quitar_ultimo();
            }
            PedidoMarco::Borrar => m.borrar(),
            PedidoMarco::Nuevo { a, b, px } => {
                // La misma regla que en el plano: un clic (8 px o menos de
                // ancho o de alto en la pantalla) no es un marco.
                if m.pulsar(a, 0) {
                    m.mover(b);
                    m.soltar(b, px);
                }
            }
            PedidoMarco::Imprimir => {}
        }
        let m = m.clone();
        self.ensenar_marcos(id, &m);
        Ok(())
    }

    /// **Ctrl+P**: los marcos en grupo, una hoja por marco y en su orden,
    /// recortados de la imagen entera; sin marcos, la zona de siempre.
    fn imprimir_marcos(&self, id: u64) -> Result<()> {
        let cajas: Vec<[f64; 4]> = self
            .marcos
            .get(&id)
            .map(|m| m.lista.iter().map(|m| m.caja).collect())
            .unwrap_or_default();
        if cajas.is_empty() {
            return self.imprimir_zona_de_foto(id);
        }
        let pin = self.vivos.get(&id).context("el pin no esta en pantalla")?;
        // Con los filtros que el usuario le puso y lo anotado; sin modo noche ni
        // negativo de pantalla: los colores de verdad.
        let imagen = match pin.imagen_para_imprimir() {
            Some(horneada) => horneada,
            None => {
                let ruta = self.imagen_del_pin(id).context("el pin no tiene imagen en el almacen")?;
                cargar(&ruta).context("no se pudo leer la imagen del almacen")?
            }
        };
        let fotos: Vec<ImagenRgba> = cajas.iter().filter_map(|c| recortar(&imagen, *c)).collect();
        if fotos.is_empty() {
            return Ok(());
        }
        let titulo = self
            .imagen_del_pin(id)
            .and_then(|r| r.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "PixPin".to_string());
        crate::marco_imprimir::imprimir_fotos(pin.hwnd().0 as isize, fotos, &titulo)
    }
}

impl Pines {
    /// Sin marcos, lo de siempre: elegir una zona del pin e imprimirla. Pero
    /// una foto va **tal cual** a `imprimir_fotos`: `imprimir_zona` aclara lo
    /// que parece modo noche (para una nota o un visor oscuros), y una foto
    /// oscura de verdad salia invertida y sin sus colores.
    fn imprimir_zona_de_foto(&self, id: u64) -> Result<()> {
        let Some(pin) = self.vivos.get(&id) else {
            return Ok(());
        };
        let (hwnd, area) = (pin.hwnd(), pin.rect_contenido());
        if area.ancho == 0 || area.alto == 0 {
            return Ok(());
        }
        let Some(foto) = crate::marco_imprimir::elegir(area, textos())? else {
            return Ok(());
        };
        let titulo = self
            .imagen_del_pin(id)
            .and_then(|r| r.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "PixPin".to_string());
        crate::marco_imprimir::imprimir_fotos(hwnd.0 as isize, vec![foto], &titulo)
    }
}

/// El trozo `caja` (pixeles de la imagen, `[x0, y0, x1, y1]`) de `img`,
/// recortado a ella. `None` si no queda nada dentro.
pub(crate) fn recortar(img: &ImagenRgba, caja: [f64; 4]) -> Option<ImagenRgba> {
    let (w, h) = (img.ancho as f64, img.alto as f64);
    let x0 = caja[0].clamp(0.0, w).floor() as u32;
    let y0 = caja[1].clamp(0.0, h).floor() as u32;
    let x1 = caja[2].clamp(0.0, w).ceil() as u32;
    let y1 = caja[3].clamp(0.0, h).ceil() as u32;
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (ancho, alto) = (x1 - x0, y1 - y0);
    let mut pixeles = Vec::with_capacity((ancho * alto * 4) as usize);
    for y in y0..y1 {
        let fila = ((y * img.ancho + x0) * 4) as usize;
        pixeles.extend_from_slice(&img.pixeles[fila..fila + (ancho * 4) as usize]);
    }
    Some(ImagenRgba { ancho, alto, pixeles })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn foto(w: u32, h: u32) -> ImagenRgba {
        let mut pixeles = Vec::new();
        for y in 0..h {
            for x in 0..w {
                pixeles.extend_from_slice(&[x as u8, y as u8, 0, 255]);
            }
        }
        ImagenRgba { ancho: w, alto: h, pixeles }
    }

    #[test]
    fn marcos_el_recorte_sale_de_los_pixeles_de_la_imagen() {
        let img = foto(40, 30);
        let r = recortar(&img, [10.0, 5.0, 20.0, 25.0]).unwrap();
        assert_eq!((r.ancho, r.alto), (10, 20));
        // La primera fila empieza en el pixel (10, 5) de la foto.
        assert_eq!(&r.pixeles[..4], &[10, 5, 0, 255]);
        let ultimo = r.pixeles.len() - 4;
        assert_eq!(&r.pixeles[ultimo..], &[19, 24, 0, 255]);
    }

    #[test]
    fn marcos_caso_negativo_fuera_de_la_imagen_no_hay_hoja() {
        let img = foto(40, 30);
        assert!(recortar(&img, [50.0, 0.0, 80.0, 10.0]).is_none());
        // A medias: se queda con lo que cae dentro.
        let r = recortar(&img, [-10.0, -10.0, 5.0, 5.0]).unwrap();
        assert_eq!((r.ancho, r.alto), (5, 5));
    }

    #[test]
    fn marcos_de_un_pin_se_guardan_al_lado_de_su_imagen_y_vuelven() {
        let d = std::env::temp_dir().join(format!("pixpin-marcos-pin-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let imagen = d.join("captura.png");
        let mut m = Marcos::de(Some(&imagen));
        m.alternar();
        // Lo que manda el pin al soltar: dos esquinas y los px por pixel.
        for (a, b) in [([10.0, 10.0], [110.0, 60.0]), ([200.0, 20.0], [150.0, 90.0])] {
            assert!(m.pulsar(a, 0));
            m.mover(b);
            assert!(m.soltar(b, 0.5));
        }
        // A media escala, 10 pixeles son 5 de pantalla: un clic.
        m.pulsar([0.0, 0.0], 0);
        assert!(!m.soltar([10.0, 10.0], 0.5));
        let otra = Marcos::de(Some(&imagen));
        assert_eq!(otra.lista.len(), 2);
        assert_eq!(otra.lista[1].caja, [150.0, 20.0, 200.0, 90.0], "ordenado arrastre como arrastre");
        m.borrar();
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **Los colores impresos son los de la foto**: una foto con franjas
    /// saturadas y una mitad oscura de verdad, dos marcos recortados de sus
    /// pixeles y a un PDF por la impresora PDF de Windows, sin dialogo. Hay
    /// que darle `PIXPIN_IMPRIMIR_A_PDF=<fichero.pdf>` y mirar el PDF: la
    /// hoja 1 con rojo, verde y azul vivos; la 2, oscura (no invertida).
    #[test]
    #[ignore = "necesita la impresora PDF; PIXPIN_IMPRIMIR_A_PDF y mirar el PDF"]
    fn marcos_de_una_foto_impresos_a_pdf_con_sus_colores() {
        assert!(std::env::var_os("PIXPIN_IMPRIMIR_A_PDF").is_some(), "falta PIXPIN_IMPRIMIR_A_PDF");
        let (w, h) = (600u32, 300u32);
        let mut pixeles = Vec::new();
        for _y in 0..h {
            for x in 0..w {
                let p = if x < 300 {
                    match x / 100 {
                        0 => [230, 20, 30, 255],
                        1 => [20, 190, 60, 255],
                        _ => [30, 60, 230, 255],
                    }
                } else {
                    // Una noche: azul muy oscuro con un brillo amarillo.
                    if (400..440).contains(&x) { [250, 200, 40, 255] } else { [12, 16, 40, 255] }
                };
                pixeles.extend_from_slice(&p);
            }
        }
        let img = ImagenRgba { ancho: w, alto: h, pixeles };
        let fotos: Vec<_> = [[0.0, 0.0, 300.0, 300.0], [300.0, 0.0, 600.0, 300.0]]
            .iter()
            .filter_map(|c| recortar(&img, *c))
            .collect();
        assert_eq!(fotos.len(), 2);
        // Lo que va al papel es el recorte tal cual: ni un pixel cambiado.
        assert_eq!(&fotos[1].pixeles[..4], &[12, 16, 40, 255]);
        crate::marco_imprimir::imprimir_fotos(0, fotos, "marcos del pin").unwrap();
    }
}
