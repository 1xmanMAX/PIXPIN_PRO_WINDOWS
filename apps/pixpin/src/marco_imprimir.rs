//! **El marco para imprimir** (19-oct-2026), el mismo en todos los visores.
//!
//! El usuario: «el lapiz, el marcador y el frame sean universales, de forma
//! que con el frame tambien pueda imprimir rapido lo que necesito». Pulsar
//! el marco congela lo que se ve del visor (una foto de su sitio en la
//! pantalla), se arrastra un recuadro encima y lo encerrado va al dialogo
//! de imprimir de Windows. Como trabaja sobre la foto y no sobre el
//! documento, vale igual para un PDF, un Word, una hoja, una pagina web, un
//! plano o un modelo 3D: cada visor solo pone el boton.
//!
//! Corre en el hilo del visor que lo pide (su bucle sigue atendiendo el
//! dialogo de imprimir despues). Mientras se elige, ese visor espera.

use anyhow::{Context, Result};
use pixpin_codec::imagen::ImagenRgba;
use pixpin_geom::{Punto, Rect};
use pixpin_render::{Color, MotorRender, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Catalogo;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

const VK_ESCAPE: u32 = 0x1B;
const VK_RETURN: u32 = 0x0D;
/// Lo minimo que tiene que medir el recuadro (pixeles): menos es un clic.
const MINIMO: i32 = 8;
const AZUL: Color = Color { r: 0.29, g: 0.56, b: 1.0, a: 1.0 };
const VELO: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.45 };
const CRISTAL: Color = Color { r: 0.08, g: 0.08, b: 0.09, a: 0.90 };
const TEXTO: Color = Color { r: 0.95, g: 0.95, b: 0.96, a: 1.0 };

/// **Todo el gesto**: elegir dentro de `area` (pixeles de pantalla, el sitio
/// del visor) y mandarlo a imprimir con `ventana` de duena. Nada si se sale
/// con Esc.
pub fn imprimir_zona(ventana: isize, area: Rect, titulo: &str, textos: &Catalogo) {
    match elegir(area, textos) {
        Ok(Some(foto)) => {
            if let Err(e) = imprimir(ventana, foto, titulo) {
                tracing::warn!(?e, "marco: no se pudo imprimir");
            }
        }
        Ok(None) => {}
        Err(e) => tracing::warn!(?e, "marco: no se pudo elegir la zona"),
    }
}

/// **El boton del marco de un visor**: la zona es lo de dentro de su
/// ventana (`ventana`, o la activa de este hilo si no se sabe cual).
pub fn de_la_ventana(ventana: Option<isize>, titulo: &str, textos: &Catalogo) {
    let Some((hwnd, area)) = pixpin_shell::overlay::area_cliente(ventana) else { return };
    imprimir_zona(hwnd, area, titulo, textos);
}

/// La foto de `area` y el recuadro elegido encima, recortado. `None` con Esc.
pub fn elegir(area: Rect, textos: &Catalogo) -> Result<Option<ImagenRgba>> {
    let d = pixpin_capture::Dispositivo::nuevo().context("sin dispositivo")?;
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let centro = Punto { x: area.x + area.ancho as i32 / 2, y: area.y + area.alto as i32 / 2 };
    let m = monitores
        .monitores()
        .iter()
        .find(|m| m.area.contiene(centro))
        .or_else(|| monitores.monitores().first())
        .context("sin monitor")?
        .clone();
    let area = area.interseccion(m.area).context("el visor no esta en ningun monitor")?;
    let foto = pixpin_capture::capturar_monitor(&d, m.id, m.area).context("sin captura")?;
    let recorte = foto.recortar(&d, area).context("sin recorte")?;
    let img = pixpin_capture::a_imagen(&d, &recorte).context("sin imagen")?;
    let escala = m.escala_por_cien as f32 / 100.0;

    let motor = MotorRender::nuevo(d.d3d()).context("sin motor")?;
    let ventana = VentanaOverlay::nueva(area).context("sin ventana")?;
    let superficie = Superficie::nueva(&motor, d.d3d(), ventana.handle(), area.ancho, area.alto).context("sin superficie")?;
    let fondo = motor.bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles).context("sin fondo")?;
    ventana.mostrar();
    ventana.enfocar();
    ventana.poner_cursor(pixpin_shell::overlay::FormaCursorWin::Cruz);

    let pista = textos.t("marco-imprimir-pista");
    let mut desde: Option<Punto> = None;
    let mut hasta = Punto { x: 0, y: 0 };
    let mut elegido: Option<Rect> = None;
    let mut sucio = true;
    let a_local = |p: Punto| Punto { x: p.x - area.x, y: p.y - area.y };
    'bucle: loop {
        if sucio {
            pintar(&motor, &superficie, &fondo, area, desde.map(|a| caja(a, hasta)), &pista, escala);
            sucio = false;
        }
        pixpin_shell::overlay::esperar_eventos(Some(100));
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match ev {
                EventoOverlay::BotonPulsado(p) => {
                    desde = Some(a_local(p));
                    hasta = a_local(p);
                    ventana.capturar_raton();
                    sucio = true;
                }
                EventoOverlay::RatonMovido(p) if desde.is_some() => {
                    hasta = a_local(p);
                    sucio = true;
                }
                EventoOverlay::BotonSoltado(p) => {
                    ventana.soltar_raton();
                    if let Some(a) = desde.take() {
                        let r = caja(a, a_local(p));
                        if r.ancho as i32 >= MINIMO && r.alto as i32 >= MINIMO {
                            elegido = Some(r);
                            break 'bucle;
                        }
                    }
                    sucio = true;
                }
                // Enter sin recuadro: todo lo que se ve.
                EventoOverlay::Tecla { vk: VK_RETURN, .. } => {
                    elegido = Some(Rect { x: 0, y: 0, ancho: img.ancho, alto: img.alto });
                    break 'bucle;
                }
                EventoOverlay::Tecla { vk: VK_ESCAPE, .. } | EventoOverlay::Cerrar => break 'bucle,
                EventoOverlay::BotonDerechoPulsado(_) => break 'bucle,
                EventoOverlay::Pintar => sucio = true,
                _ => {}
            }
        }
    }
    ventana.ocultar();
    Ok(elegido.map(|r| recortar(&img, r)))
}

/// El recuadro entre dos esquinas cualesquiera.
fn caja(a: Punto, b: Punto) -> Rect {
    Rect {
        x: a.x.min(b.x),
        y: a.y.min(b.y),
        ancho: (a.x - b.x).unsigned_abs(),
        alto: (a.y - b.y).unsigned_abs(),
    }
}

/// Lo de dentro de `r` (pixeles de la foto), recortado a la foto.
fn recortar(img: &ImagenRgba, r: Rect) -> ImagenRgba {
    let x0 = r.x.clamp(0, img.ancho as i32) as u32;
    let y0 = r.y.clamp(0, img.alto as i32) as u32;
    let x1 = (r.x + r.ancho as i32).clamp(0, img.ancho as i32) as u32;
    let y1 = (r.y + r.alto as i32).clamp(0, img.alto as i32) as u32;
    let (w, h) = (x1.saturating_sub(x0).max(1), y1.saturating_sub(y0).max(1));
    let mut pixeles = Vec::with_capacity((w * h * 4) as usize);
    for y in y0..y0 + h {
        let fila = ((y.min(img.alto - 1) * img.ancho + x0) * 4) as usize;
        pixeles.extend_from_slice(&img.pixeles[fila..fila + (w * 4) as usize]);
    }
    ImagenRgba { ancho: w, alto: h, pixeles }
}

fn pintar(motor: &MotorRender, s: &Superficie, fondo: &ID2D1Bitmap1, area: Rect, sel: Option<Rect>, pista: &str, e: f32) {
    let Ok(destino) = s.empezar(motor) else { return };
    let todo = RectF { x: 0.0, y: 0.0, ancho: area.ancho as f32, alto: area.alto as f32 };
    let _ = motor.dibujar(&destino, |p| {
        p.bitmap(fondo, todo, None, true);
        match sel {
            Some(r) => {
                let r = RectF { x: r.x as f32, y: r.y as f32, ancho: r.ancho as f32, alto: r.alto as f32 };
                // Lo de fuera, a oscuras; el recuadro, con su raya.
                p.velo(todo, &[(r.x, r.y), (r.x + r.ancho, r.y), (r.x + r.ancho, r.y + r.alto), (r.x, r.y + r.alto)], VELO);
                p.trazar(r, 2.0 * e, AZUL);
            }
            None => p.rellenar(todo, Color { a: 0.18, ..VELO }),
        }
        pista_arriba(p, pista, todo.ancho, e);
    });
    let _ = s.presentar();
}

fn pista_arriba(p: &Pintor, pista: &str, ancho: f32, e: f32) {
    let tam = 13.0 * e;
    let (w, h) = p.medir_texto(pista, tam);
    let caja = RectF { x: (ancho - w) / 2.0 - 14.0 * e, y: 14.0 * e, ancho: w + 28.0 * e, alto: h + 14.0 * e };
    p.rellenar_redondeado(caja, caja.alto / 2.0, CRISTAL);
    p.texto(pista, caja.x + 14.0 * e, caja.y + 7.0 * e, tam, TEXTO);
}

/// Lo que se imprime: la zona, encajada en el papel.
struct ZonaImpresa {
    foto: ImagenRgba,
    bitmap: Option<ID2D1Bitmap1>,
}

impl pixpin_render::imprimir_moderno::Documento for ZonaImpresa {
    fn paginar(&mut self, motor: &MotorRender, _alcance: Option<&str>) -> usize {
        if self.bitmap.is_none() {
            self.bitmap = motor.bitmap_desde_pixeles(self.foto.ancho, self.foto.alto, &self.foto.pixeles).ok();
        }
        1
    }

    fn pintar(&self, _i: usize, hoja: pixpin_render::imprimir_moderno::Pagina, p: &Pintor) {
        let Some(b) = &self.bitmap else { return };
        let r = en_el_papel((self.foto.ancho as f32, self.foto.alto as f32), hoja.papel);
        let k = hoja.escala;
        p.bitmap(b, RectF { x: r.x * k, y: r.y * k, ancho: r.ancho * k, alto: r.alto * k }, None, false);
    }
}

/// Donde va la foto en el papel (DIP): con 1 cm de margen, lo mas grande
/// que quepa sin pasar del doble de como se veia (mas grande solo se
/// emborrona), y centrada arriba.
fn en_el_papel(foto: (f32, f32), papel: (f32, f32)) -> RectF {
    let margen = 96.0 / 2.54;
    let (aw, ah) = ((papel.0 - 2.0 * margen).max(1.0), (papel.1 - 2.0 * margen).max(1.0));
    let k = (aw / foto.0).min(ah / foto.1).min(2.0);
    let (w, h) = (foto.0 * k, foto.1 * k);
    RectF { x: (papel.0 - w) / 2.0, y: margen, ancho: w, alto: h }
}

/// El dialogo moderno de Windows con la vista previa; si no se abre, el
/// clasico.
fn imprimir(ventana: isize, foto: ImagenRgba, titulo: &str) -> Result<()> {
    let apaisada = foto.ancho > foto.alto;
    let d = pixpin_capture::Dispositivo::nuevo().context("sin dispositivo para imprimir")?;
    let motor = MotorRender::nuevo(d.d3d()).context("sin motor para imprimir")?;
    if !pixpin_render::imprimir_moderno::roto() {
        let pedido = pixpin_render::imprimir_moderno::Pedido { titulo: titulo.to_string(), apaisada, opcion: None, ppp_previa: 144.0 };
        let doc = ZonaImpresa { foto, bitmap: None };
        return pixpin_render::imprimir_moderno::mostrar(HWND(ventana as *mut _), pedido, d.d3d().clone(), motor, Box::new(doc))
            .context("el dialogo de imprimir no se abrio");
    }
    let Some(eleccion) = pixpin_shell::imprimir::pedir_impresion(HWND(ventana as *mut _), 1, false, apaisada) else {
        return Ok(());
    };
    let mut doc = ZonaImpresa { foto, bitmap: None };
    use pixpin_render::imprimir_moderno::Documento;
    doc.paginar(&motor, None);
    pixpin_render::imprimir::imprimir(
        &motor,
        &pixpin_render::imprimir::Trabajo {
            impresora: &eleccion.impresora,
            nombre: titulo,
            devmode: Some(&eleccion.devmode),
            paginas: &[eleccion.papel],
            fichero: None,
        },
        &mut |i, p| doc.pintar(i, pixpin_render::imprimir_moderno::Pagina { papel: eleccion.papel, escala: 1.0 }, p),
    )
    .context("la impresora no acepto el trabajo")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_recuadro_sale_igual_arrastre_hacia_donde_arrastre() {
        let r = caja(Punto { x: 50, y: 40 }, Punto { x: 10, y: 90 });
        assert_eq!(r, Rect { x: 10, y: 40, ancho: 40, alto: 50 });
    }

    #[test]
    fn el_recorte_no_se_sale_de_la_foto() {
        let img = ImagenRgba { ancho: 4, alto: 3, pixeles: (0..48).collect() };
        let r = recortar(&img, Rect { x: 2, y: 1, ancho: 10, alto: 10 });
        assert_eq!((r.ancho, r.alto), (2, 2));
        // La fila 1 desde la columna 2: los bytes 24..32.
        assert_eq!(&r.pixeles[..8], &(24u8..32).collect::<Vec<_>>()[..]);
    }

    #[test]
    fn en_el_papel_cabe_y_no_se_estira_de_mas() {
        let a4 = (793.7, 1122.5);
        // Una zona pequena: al doble, no a todo el papel.
        let r = en_el_papel((100.0, 50.0), a4);
        assert!((r.ancho - 200.0).abs() < 0.01);
        // Una enorme: reducida hasta caber entre los margenes.
        let r = en_el_papel((4000.0, 1000.0), a4);
        assert!(r.ancho <= a4.0 - 2.0 * 96.0 / 2.54 + 0.01);
    }
}
