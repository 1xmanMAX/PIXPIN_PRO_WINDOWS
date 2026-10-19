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
    #[cfg(test)]
    pruebas::VENTANA.store(ventana.handle().0 as isize, std::sync::atomic::Ordering::SeqCst);
    ventana.mostrar();
    ventana.enfocar();
    ventana.poner_cursor(pixpin_shell::overlay::FormaCursorWin::Cruz);

    let pista = textos.t("marco-imprimir-pista");
    let mut desde: Option<Punto> = None;
    let mut hasta = Punto { x: 0, y: 0 };
    let mut elegido: Option<Rect> = None;
    let mut sucio = true;
    let a_local = |p: Punto| Punto { x: p.x - area.x, y: p.y - area.y };
    let mut quieto = std::time::Instant::now();
    'bucle: loop {
        if sucio {
            pintar(&motor, &superficie, &fondo, area, desde.map(|a| caja(a, hasta)), &pista, escala);
            sucio = false;
        }
        pixpin_shell::overlay::esperar_eventos(Some(100));
        // Sin bombear, el raton nunca llega a la ventana del marco.
        pixpin_shell::overlay::bombear_pendientes();
        // Olvidado abierto (o su ventana perdio el raton): a los 3 minutos sin
        // nada, se cierra solo y el visor vuelve a atender.
        if quieto.elapsed().as_secs() >= 180 {
            tracing::info!("marco: 3 minutos sin nada, se cierra");
            break 'bucle;
        }
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            quieto = std::time::Instant::now();
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

/// Lo que se imprime: una hoja por foto (un marco cada una, en su orden),
/// cada una encajada en el papel.
struct ZonaImpresa {
    fotos: Vec<ImagenRgba>,
    bitmaps: Vec<Option<ID2D1Bitmap1>>,
}

impl ZonaImpresa {
    fn de(fotos: Vec<ImagenRgba>) -> ZonaImpresa {
        ZonaImpresa { fotos, bitmaps: Vec::new() }
    }
}

impl pixpin_render::imprimir_moderno::Documento for ZonaImpresa {
    fn paginar(&mut self, motor: &MotorRender, _alcance: Option<&str>) -> usize {
        if self.bitmaps.len() != self.fotos.len() {
            self.bitmaps = self.fotos.iter().map(|f| motor.bitmap_desde_pixeles(f.ancho, f.alto, &f.pixeles).ok()).collect();
        }
        self.fotos.len()
    }

    fn pintar(&self, i: usize, hoja: pixpin_render::imprimir_moderno::Pagina, p: &Pintor) {
        let (Some(f), Some(Some(b))) = (self.fotos.get(i), self.bitmaps.get(i)) else { return };
        let r = en_el_papel((f.ancho as f32, f.alto as f32), hoja.papel);
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

/// **Lo congelado, listo para el papel**: si lo que se ve va en modo noche
/// (fondo oscuro), se le devuelve la luz (la misma inversion que conserva el
/// tono de `pixpin_render::oscuro`), para no imprimir una hoja negra con
/// letra blanca. Lo claro va tal cual.
fn para_papel(mut foto: ImagenRgba) -> ImagenRgba {
    let n = (foto.pixeles.len() / 4).max(1);
    let suma: u64 = foto
        .pixeles
        .chunks_exact(4)
        .map(|p| (p[0] as u64 * 299 + p[1] as u64 * 587 + p[2] as u64 * 114) / 1000)
        .sum();
    if suma / n as u64 >= 110 {
        return foto;
    }
    for p in foto.pixeles.chunks_exact_mut(4) {
        let (r, g, b) = pixpin_render::oscuro::invertir_luz_u8(p[0], p[1], p[2]);
        p[0] = r;
        p[1] = g;
        p[2] = b;
    }
    foto
}

/// El dialogo moderno de Windows con la vista previa; si no se abre (o ya
/// fallo en esta sesion), el clasico, que imprime lo mismo.
fn imprimir(ventana: isize, foto: ImagenRgba, titulo: &str) -> Result<()> {
    // Lo congelado de la pantalla puede venir en modo noche: a claro.
    imprimir_fotos(ventana, vec![para_papel(foto)], titulo)
}

/// **Imprime en grupo**: una hoja por foto, en su orden (los marcos 1, 2,
/// 3… de un documento, como las laminas del plano). El papel y su
/// orientacion los dice la primera, como en el plano y en el lienzo.
pub fn imprimir_fotos(ventana: isize, fotos: Vec<ImagenRgba>, titulo: &str) -> Result<()> {
    // Tal cual: cada visor las fotografia ya con sus colores de verdad (sin
    // modo noche). Aclarar aqui estropeaba los colores y una foto oscura de
    // verdad salia invertida.
    let Some(primera) = fotos.first() else { return Ok(()) };
    let apaisada = primera.ancho > primera.alto;
    let n = fotos.len() as u32;
    // Para las pruebas de los visores: sin dialogo, a un PDF con la impresora
    // PDF de Windows (`PIXPIN_IMPRIMIR_A_PDF=<fichero.pdf>`).
    if let Some(pdf) = std::env::var_os("PIXPIN_IMPRIMIR_A_PDF") {
        let d = pixpin_capture::Dispositivo::nuevo().context("sin dispositivo para imprimir")?;
        let motor = MotorRender::nuevo(d.d3d()).context("sin motor para imprimir")?;
        let papel = pixpin_render::imprimir_moderno::papel_por_omision(apaisada);
        tracing::info!(hojas = n, pdf = ?pdf, "marco: imprimiendo a un PDF (prueba)");
        return a_la_impresora(&motor, fotos, "Microsoft Print to PDF", None, papel, titulo, Some(std::path::Path::new(&pdf)));
    }
    let d = pixpin_capture::Dispositivo::nuevo().context("sin dispositivo para imprimir")?;
    if !pixpin_render::imprimir_moderno::roto() {
        let motor = MotorRender::nuevo(d.d3d()).context("sin motor para imprimir")?;
        let pedido = pixpin_render::imprimir_moderno::Pedido { titulo: titulo.to_string(), apaisada, opcion: None, ppp_previa: 144.0 };
        let doc = ZonaImpresa::de(fotos.clone());
        match pixpin_render::imprimir_moderno::mostrar(HWND(ventana as *mut _), pedido, d.d3d().clone(), motor, Box::new(doc)) {
            Ok(()) => return Ok(()),
            Err(e) => tracing::warn!(?e, "marco: sin dialogo moderno de imprimir, el clasico"),
        }
    }
    let Some(eleccion) = pixpin_shell::imprimir::pedir_impresion(HWND(ventana as *mut _), n, false, apaisada) else {
        return Ok(());
    };
    let motor = MotorRender::nuevo(d.d3d()).context("sin motor para imprimir")?;
    a_la_impresora(&motor, fotos, &eleccion.impresora, Some(&eleccion.devmode), eleccion.papel, titulo, None)
}

/// Imprime las fotos en `impresora`, una por hoja (o, con `fichero`,
/// escribe ahi lo que saldria por ella: las pruebas con «Microsoft Print to
/// PDF»).
fn a_la_impresora(
    motor: &MotorRender,
    fotos: Vec<ImagenRgba>,
    impresora: &str,
    devmode: Option<&[u8]>,
    papel: (f32, f32),
    titulo: &str,
    fichero: Option<&std::path::Path>,
) -> Result<()> {
    use pixpin_render::imprimir_moderno::Documento;
    let mut doc = ZonaImpresa::de(fotos);
    let n = doc.paginar(motor, None);
    let papeles = vec![papel; n];
    pixpin_render::imprimir::imprimir(
        motor,
        &pixpin_render::imprimir::Trabajo { impresora, nombre: titulo, devmode, paginas: &papeles, fichero },
        &mut |i, p| doc.pintar(i, pixpin_render::imprimir_moderno::Pagina { papel, escala: 1.0 }, p),
    )
    .context("la impresora no acepto el trabajo")
}

/// **La foto de un trozo de la pantalla** (pixeles de pantalla), tal cual:
/// lo que usa cada visor para imprimir sus marcos (se acerca al marco, se
/// pinta sin su interfaz y se fotografia).
pub fn foto_de_pantalla(area: Rect) -> Option<ImagenRgba> {
    let d = pixpin_capture::Dispositivo::nuevo().ok()?;
    let monitores = pixpin_capture::enumerar_monitores().ok()?;
    let centro = Punto { x: area.x + area.ancho as i32 / 2, y: area.y + area.alto as i32 / 2 };
    let m = monitores.monitores().iter().find(|m| m.area.contiene(centro)).or_else(|| monitores.monitores().first())?.clone();
    let area = area.interseccion(m.area)?;
    let foto = pixpin_capture::capturar_monitor(&d, m.id, m.area).ok()?;
    let recorte = foto.recortar(&d, area).ok()?;
    pixpin_capture::a_imagen(&d, &recorte).ok()
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

    /// La ventana del marco abierta ahora (para mandarle el raton).
    pub(super) static VENTANA: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

    #[test]
    fn lo_oscuro_se_aclara_para_el_papel_y_lo_claro_no_se_toca() {
        // Una hoja en noche: casi todo fondo negro y un poco de letra blanca.
        let mut pixeles = [20u8, 20, 22, 255].repeat(9);
        pixeles.extend([240, 240, 240, 255]);
        let oscura = ImagenRgba { ancho: 10, alto: 1, pixeles };
        let p = para_papel(oscura);
        assert!(p.pixeles[0] > 200, "el fondo negro pasa a claro: {:?}", &p.pixeles[..4]);
        assert!(p.pixeles[36] < 60, "la letra blanca pasa a oscura: {:?}", &p.pixeles[36..]);
        let clara = ImagenRgba { ancho: 2, alto: 1, pixeles: vec![250, 250, 250, 255, 10, 10, 10, 255] };
        assert_eq!(para_papel(clara.clone()), clara);
    }

    /// La vista previa del dialogo: la zona sale en el papel, arriba y al
    /// centro, y el resto del papel blanco.
    #[test]
    #[ignore = "necesita la GPU"]
    fn la_vista_previa_lleva_la_zona_en_el_papel() {
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let roja = ImagenRgba { ancho: 300, alto: 100, pixeles: [220u8, 30, 30, 255].repeat(300 * 100) };
        let a4 = pixpin_render::imprimir_moderno::A4;
        let (_, (w, h, px)) = pixpin_render::imprimir_moderno::miniatura_rgba(
            Box::new(ZonaImpresa::de(vec![roja])),
            motor,
            d.d3d().clone(),
            a4,
            None,
            0,
            a4.0,
            96.0,
        )
        .expect("miniatura");
        let en = |x: f32, y: f32| {
            let (x, y) = ((x * w as f32) as usize, (y * h as f32) as usize);
            let i = (y * w as usize + x) * 4;
            [px[i], px[i + 1], px[i + 2]]
        };
        let r = en_el_papel((300.0, 100.0), a4);
        let centro = en((r.x + r.ancho / 2.0) / a4.0, (r.y + r.alto / 2.0) / a4.1);
        assert!(centro[0] > 180 && centro[1] < 90, "la zona, en rojo: {centro:?}");
        let abajo = en(0.5, 0.9);
        assert!(abajo.iter().all(|c| *c > 240), "lo demas, papel blanco: {abajo:?}");
    }

    /// **El gesto entero**: congelar un trozo de la pantalla, arrastrar un
    /// recuadro (con el raton de mentira, a la ventana del marco) y que lo
    /// elegido salga por «Microsoft Print to PDF» a un fichero.
    #[test]
    #[ignore = "abre una ventana encima de la pantalla y usa la impresora PDF de Windows"]
    fn el_gesto_entero_imprime_lo_arrastrado() {
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let area = Rect { x: 200, y: 200, ancho: 600, alto: 400 };
        let raton = std::thread::spawn(|| {
            let t = std::time::Instant::now();
            let mut h = 0;
            while h == 0 && t.elapsed().as_secs() < 10 {
                std::thread::sleep(std::time::Duration::from_millis(50));
                h = VENTANA.load(std::sync::atomic::Ordering::SeqCst);
            }
            eprintln!("marco: ventana {h}");
            std::thread::sleep(std::time::Duration::from_millis(300));
            pixpin_shell::overlay::simular_raton(h, 0, 50, 40);
            pixpin_shell::overlay::simular_raton(h, 1, 150, 90);
            pixpin_shell::overlay::simular_raton(h, 1, 250, 140);
            pixpin_shell::overlay::simular_raton(h, 2, 250, 140);
        });
        eprintln!("marco: eligiendo");
        let foto = elegir(area, &textos).expect("elegir").expect("un recuadro");
        eprintln!("marco: elegido {}x{}", foto.ancho, foto.alto);
        raton.join().unwrap();
        assert_eq!((foto.ancho, foto.alto), (200, 100), "lo arrastrado, ni mas ni menos");
        let pdf = std::env::temp_dir().join(format!("pixpin-marco-{}.pdf", std::process::id()));
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        a_la_impresora(&motor, vec![para_papel(foto.clone()), para_papel(foto)], "Microsoft Print to PDF", None, pixpin_render::imprimir_moderno::A4, "marco", Some(&pdf)).expect("imprimir");
        eprintln!("marco: impreso");
        let bytes = std::fs::read(&pdf).expect("el pdf");
        assert!(bytes.starts_with(b"%PDF") && bytes.len() > 1000, "{} bytes", bytes.len());
        let _ = std::fs::remove_file(&pdf);
    }
}
