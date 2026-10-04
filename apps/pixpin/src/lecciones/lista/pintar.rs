//! Lo que se ve de la ventana de lecciones v2: la cabecera con la barra
//! rapida, las tres columnas, el menu desplegado y el aviso de abajo.

use pixpin_geom::Rect;
use pixpin_lecciones::Leccion;
use pixpin_render::icono::material as mi;
use pixpin_render::{Color, Pintor, RectF};

use super::super::ui::{self, Hace, color_de, color_de_gravedad, con_alfa, v2};
use super::repaso::{self, nombre_de_gravedad};
use super::{Accion, Estado, Filtro, Foco, Menu, columnas, t1, t2};
use crate::caja_dibujo::hex;
use crate::ventanita::dentro;

const ALTO_FILA: f32 = 60.0;

pub(super) fn todo(e: &mut Estado, p: &Pintor, marco: Rect, s: f32) {
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    p.limpiar(v2::FONDO);
    e.botones.vaciar();
    e.botones.zona(RectF { x: 0.0, y: 0.0, ancho: w, alto: h }, Accion::Fondo);
    if e.repaso.is_some() {
        repaso::pintar(e, p, w, h, s);
    } else {
        let arriba = cabecera(e, p, w, s);
        let (lista, ficha, datos) = columnas(w, h, arriba, s);
        columna_lista(e, p, lista, s);
        if e.comprobacion {
            comprobacion(e, p, RectF { ancho: ficha.ancho + datos.ancho, ..ficha }, s);
        } else if e.elegida().is_some() {
            columna_ficha(e, p, ficha, s);
            columna_datos(e, p, datos, s);
        } else {
            vacia(e, p, RectF { ancho: ficha.ancho + datos.ancho, ..ficha }, s);
        }
        menu(e, p, w, h, s);
    }
    aviso(e, p, w, h, s);
}

// ------------------------------------------------------------ cabecera

/// La cabecera: el titulo, la barra «¿Que aprendiste?» y lo rellenado solo.
/// Devuelve su alto.
fn cabecera(e: &mut Estado, p: &Pintor, w: f32, s: f32) -> f32 {
    let tx = e.textos.clone();
    let alto = 112.0 * s;
    e.botones.zona(RectF { x: 0.0, y: 0.0, ancho: w, alto }, Accion::Mover);
    p.rellenar(RectF { x: 0.0, y: alto - 1.0 * s, ancho: w, alto: 1.0 * s }, v2::LINEA);
    let m = 18.0 * s;
    ui::negrita(p, &tx.t("chat-lecciones"), m, 25.0 * s, 20.0 * s, 150.0 * s, v2::TEXTO);

    // Cerrar (la ventana no tiene marco de Windows).
    let cerrar = RectF { x: w - m - 40.0 * s, y: 18.0 * s, ancho: 40.0 * s, alto: 40.0 * s };
    boton_icono(e, p, cerrar, Accion::Cerrar, &mi::CLOSE, None, s);

    // La barra.
    let bx = m + 162.0 * s;
    let barra = RectF { x: bx, y: 14.0 * s, ancho: cerrar.x - 10.0 * s - bx, alto: 48.0 * s };
    let foco = e.foco == Some(Foco::Rapida);
    if foco {
        p.rellenar_redondeado(ui::encoger(barra, -2.0 * s), 16.0 * s, v2::ELEGIDO);
    }
    p.rellenar_redondeado(barra, 14.0 * s, v2::CAJA);
    let cy = barra.y + barra.alto / 2.0;
    p.icono(&mi::LIGHTBULB, RectF { x: barra.x + 12.0 * s, y: cy - 10.0 * s, ancho: 20.0 * s, alto: 20.0 * s }, v2::AMARILLO);
    // De derecha a izquierda: Guardar, imagen, microfono, fotos.
    let guardar_rot = tx.t("lec2-guardar");
    let gw = ui::ancho_de_boton(p, false, &guardar_rot, Some("Enter"), s);
    let guardar = RectF { x: barra.x + barra.ancho - gw - 5.0 * s, y: barra.y + 5.0 * s, ancho: gw, alto: 38.0 * s };
    let vale = !e.rapida.campo.texto.trim().is_empty();
    ui::boton_v2(
        p,
        &mut e.botones,
        guardar,
        Accion::Guardar,
        None,
        &guardar_rot,
        Some("Enter"),
        Some(if vale { v2::AZUL } else { hex(0x3a3a3c) }),
        if vale { v2::BLANCO } else { v2::APAGADO },
        s,
    );
    let img = RectF { x: guardar.x - 44.0 * s, y: cy - 20.0 * s, ancho: 40.0 * s, alto: 40.0 * s };
    boton_icono(e, p, img, Accion::ImagenRapida, &mi::IMAGE, None, s);
    let mic = RectF { x: img.x - 42.0 * s, ..img };
    let grabando = matches!(e.dictado, super::super::dictar::Dictado::Grabando(_)) && e.dictando == Foco::Rapida;
    boton_icono(e, p, mic, Accion::DictarRapida, &mi::MIC, grabando.then_some(v2::ROJO), s);
    let mut fx = mic.x - 8.0 * s;
    let n_fotos = e.rapida.fotos.len();
    for i in (0..n_fotos).rev() {
        let f = RectF { x: fx - 44.0 * s, y: cy - 15.0 * s, ancho: 38.0 * s, alto: 30.0 * s };
        p.rellenar_redondeado(f, 6.0 * s, hex(0x8c7a5b));
        p.icono(&mi::IMAGE, ui::encoger(f, 7.0 * s), v2::OSCURO);
        let x = RectF { x: f.x + f.ancho - 10.0 * s, y: f.y - 6.0 * s, ancho: 16.0 * s, alto: 16.0 * s };
        p.circulo((x.x + 8.0 * s, x.y + 8.0 * s), 8.0 * s, hex(0x48484a));
        p.icono(&mi::CLOSE, ui::encoger(x, 3.0 * s), v2::TEXTO);
        e.botones.zona(ui::encoger(x, -4.0 * s), Accion::QuitarFoto(i));
        fx = f.x - 6.0 * s;
    }
    let tx0 = barra.x + 42.0 * s;
    let campo = RectF { x: tx0, y: barra.y, ancho: (fx - tx0 - 6.0 * s).max(40.0 * s), alto: barra.alto };
    e.botones.zona(campo, Accion::FocoRapida);
    let pista = tx.t("lec2-que-aprendiste");
    let rapida = &e.rapida.campo;
    p.con_recorte(campo, |p| {
        let (tw, _) = p.medir_texto(&rapida.texto, 15.0 * s);
        // Lo ultimo escrito siempre a la vista.
        let corre = (tw - campo.ancho + 12.0 * s).max(0.0);
        rapida.pintar_texto(p, tx0 - corre, cy - 10.0 * s, 100_000.0, 15.0 * s, foco, &pista, s);
    });

    // Lo rellenado solo.
    let y = 72.0 * s;
    let fila_cy = y + 13.0 * s;
    let mut x = bx;
    let estado = if e.dictando == Foco::Rapida { e.dictado.estado() } else { None };
    if let Some(t) = estado {
        p.texto_linea(&t, x, fila_cy - 9.0 * s, 13.0 * s, w - x - 200.0 * s, if grabando { v2::ROJO } else { v2::APAGADO });
    } else if e.rapida.campo.texto.trim().is_empty() {
        p.texto_linea(&tx.t("lec2-rellena-solo-vacio"), x, fila_cy - 9.0 * s, 13.0 * s, w - x - 200.0 * s, v2::APAGADO);
    } else {
        p.texto_color("✨", x, fila_cy - 9.0 * s, 13.0 * s, v2::CIAN);
        x += 22.0 * s;
        let t = tx.t("lec2-rellena-solo");
        p.texto(&t, x, fila_cy - 9.0 * s, 13.0 * s, v2::SUAVE);
        x += p.medir_texto(&t, 13.0 * s).0 + 10.0 * s;
        let area = e.area_rapida().unwrap_or_else(|| tx.t("lec2-sin-area"));
        let rot = t1(&tx, "lec2-area-de", "area", area);
        x = etiqueta(e, p, x, fila_cy, &rot, None, true, Accion::AbrirMenu(Menu::AreaRapida), s) + 6.0 * s;
        if e.menu == Some(Menu::AreaRapida) {
            e.ancla = RectF { x: x - 6.0 * s - p.medir_texto(&rot, 13.0 * s).0 - 40.0 * s, y: fila_cy - 13.0 * s, ancho: 200.0 * s, alto: 26.0 * s };
        }
        let proyecto = e.proyecto_rapida().and_then(|f| e.nombre_de(&f)).unwrap_or_else(|| tx.t("lec2-sin-proyecto"));
        let rot = t1(&tx, "lec2-proyecto-de", "proyecto", proyecto);
        let x0 = x;
        x = etiqueta(e, p, x, fila_cy, &rot, None, true, Accion::AbrirMenu(Menu::ProyectoRapida), s) + 6.0 * s;
        if e.menu == Some(Menu::ProyectoRapida) {
            e.ancla = RectF { x: x0, y: fila_cy - 13.0 * s, ancho: (x - x0).max(220.0 * s), alto: 26.0 * s };
        }
        let g = e.gravedad_rapida();
        x = etiqueta(e, p, x, fila_cy, &tx.t(nombre_de_gravedad(g)), Some(color_de_gravedad(g)), false, Accion::CicloGravedad, s) + 10.0 * s;
        if let Some(l) = e.rapida.parecida.clone() {
            let rot = t1(&tx, "lec2-ya-la-tienes", "titulo", ui::corto(&l.titulo, 34));
            let fin = barra.x + barra.ancho - 200.0 * s;
            if x < fin - 120.0 * s {
                let ancho = (p.medir_texto(&rot, 13.0 * s).0 + 40.0 * s).min(fin - x);
                let caja = RectF { x, y: fila_cy - 13.0 * s, ancho, alto: 26.0 * s };
                let encima = dentro(caja, e.botones.raton);
                p.rellenar_redondeado(caja, 8.0 * s, if encima { con_alfa(v2::AMARILLO, 0.26) } else { con_alfa(v2::AMARILLO, 0.16) });
                p.icono(&mi::REPLAY, RectF { x: x + 8.0 * s, y: fila_cy - 7.0 * s, ancho: 14.0 * s, alto: 14.0 * s }, v2::AMARILLO);
                p.texto_linea(&rot, x + 28.0 * s, fila_cy - 9.0 * s, 13.0 * s, ancho - 34.0 * s, v2::AMARILLO);
                e.botones.zona(caja, Accion::YaLaTengo);
            }
        }
    }
    // A la derecha: Ctrl V pega capturas.
    let pega = tx.t("lec2-pega-capturas");
    let pw = p.medir_texto(&pega, 13.0 * s).0;
    let px = barra.x + barra.ancho - pw;
    let cw = ui::ancho_de_chapa(p, "Ctrl V", s);
    // Solo si cabe junto a lo rellenado.
    if px - cw - 6.0 * s > x + 10.0 * s {
        p.texto(&pega, px, fila_cy - 9.0 * s, 13.0 * s, v2::APAGADO);
        ui::chapa(p, "Ctrl V", px - cw - 6.0 * s, fila_cy, v2::SUAVE, hex(0x2a2a2d), s);
    }
    alto
}

/// Un boton redondo con solo icono (40 x 40).
fn boton_icono(e: &mut Estado, p: &Pintor, caja: RectF, que: Accion, icono: &pixpin_render::icono::Icono, fondo: Option<Color>, s: f32) {
    let encima = dentro(caja, e.botones.raton);
    if let Some(c) = fondo {
        p.rellenar_redondeado(caja, 10.0 * s, c);
    } else if encima {
        p.rellenar_redondeado(caja, 10.0 * s, con_alfa(v2::BLANCO, 0.10));
    }
    p.icono(icono, ui::encoger(caja, 10.0 * s), if encima || fondo.is_some() { v2::TEXTO } else { v2::SUAVE });
    e.botones.zona(caja, que);
}

/// Una etiqueta de las de «rellenado solo» (alto 26): con punto de color o
/// flecha de menu. Devuelve la x de su final.
#[allow(clippy::too_many_arguments)] // estado, pintor, sitio, rotulo, punto, flecha, que y escala
fn etiqueta(e: &mut Estado, p: &Pintor, x: f32, cy: f32, rotulo: &str, punto: Option<Color>, flecha: bool, que: Accion, s: f32) -> f32 {
    let tw = p.medir_texto(rotulo, 13.0 * s).0;
    let ancho = 20.0 * s + tw + if punto.is_some() { 14.0 * s } else { 0.0 } + if flecha { 18.0 * s } else { 0.0 };
    let caja = RectF { x, y: cy - 13.0 * s, ancho, alto: 26.0 * s };
    let encima = dentro(caja, e.botones.raton);
    p.rellenar_redondeado(caja, 8.0 * s, if encima { v2::ENCIMA } else { v2::CAJA });
    let mut xx = x + 10.0 * s;
    if let Some(c) = punto {
        p.circulo((xx + 4.0 * s, cy), 4.0 * s, c);
        xx += 14.0 * s;
    }
    p.texto(rotulo, xx, cy - 9.0 * s, 13.0 * s, hex(0xe5e5ea));
    if flecha {
        p.icono(&mi::KEYBOARD_ARROW_DOWN, RectF { x: xx + tw + 2.0 * s, y: cy - 7.0 * s, ancho: 14.0 * s, alto: 14.0 * s }, v2::APAGADO);
    }
    e.botones.zona(caja, que);
    x + ancho
}

// --------------------------------------------------------------- lista

fn columna_lista(e: &mut Estado, p: &Pintor, r: RectF, s: f32) {
    let tx = e.textos.clone();
    p.rellenar(r, v2::COLUMNA);
    p.rellenar(RectF { x: r.x + r.ancho - 1.0 * s, ancho: 1.0 * s, ..r }, v2::LINEA);
    let m = 14.0 * s;
    let x = r.x + m;
    let ancho = r.ancho - 2.0 * m;
    let mut y = r.y + 14.0 * s;

    // Buscador y lista de comprobacion.
    let caja = RectF { x, y, ancho: ancho - 46.0 * s, alto: 40.0 * s };
    let foco = e.foco == Some(Foco::Buscar);
    if foco {
        p.rellenar_redondeado(ui::encoger(caja, -2.0 * s), 12.0 * s, v2::ELEGIDO);
        p.rellenar_redondeado(caja, 10.0 * s, v2::CAJA);
    } else {
        p.rellenar_redondeado(caja, 10.0 * s, con_alfa(v2::BLANCO, 0.07));
    }
    p.icono(&mi::SEARCH, RectF { x: x + 12.0 * s, y: y + 12.0 * s, ancho: 16.0 * s, alto: 16.0 * s }, v2::APAGADO);
    e.botones.zona(caja, Accion::FocoBuscar);
    let derecha = if e.consulta.texto.is_empty() {
        let cw = ui::ancho_de_chapa(p, "Ctrl F", s);
        ui::chapa(p, "Ctrl F", caja.x + caja.ancho - cw - 10.0 * s, y + 20.0 * s, v2::SUAVE, hex(0x2a2a2d), s);
        cw + 16.0 * s
    } else {
        let b = RectF { x: caja.x + caja.ancho - 36.0 * s, y: y + 4.0 * s, ancho: 32.0 * s, alto: 32.0 * s };
        p.icono(&mi::CLOSE, ui::encoger(b, 8.0 * s), v2::APAGADO);
        e.botones.zona(b, Accion::BorrarConsulta);
        40.0 * s
    };
    let campo = RectF { x: x + 36.0 * s, y, ancho: caja.ancho - 36.0 * s - derecha, alto: 40.0 * s };
    let pista = tx.t("lec2-buscar");
    let consulta = &e.consulta;
    p.con_recorte(campo, |p| consulta.pintar_texto(p, campo.x, y + 11.0 * s, 100_000.0, 14.0 * s, foco, &pista, s));
    let lista = RectF { x: x + ancho - 40.0 * s, y, ancho: 40.0 * s, alto: 40.0 * s };
    let puesta = e.comprobacion;
    boton_icono(e, p, lista, Accion::Comprobacion, &mi::CHECKLIST, puesta.then_some(con_alfa(v2::AMARILLO, 0.22)), s);
    y += 50.0 * s;

    // Filtros con su cuenta (y el proyecto, si se abrio desde uno).
    let mut fx = x;
    let alto_ficha = 32.0 * s;
    let mut fichas: Vec<(String, Option<Color>, bool, Accion, Option<String>)> = Vec::new();
    if let Some(pr) = &e.proyecto {
        let nombre = e.nombre_de(pr).unwrap_or_else(|| pr.clone());
        fichas.push((nombre, None, true, Accion::QuitarProyecto, Some("×".into())));
    }
    for (i, (f, n)) in e.filtros().into_iter().enumerate() {
        let (rot, punto) = match &f {
            Filtro::Todas => (tx.t("lec2-todas"), None),
            Filtro::Repetidas => (tx.t("lec2-repetidas"), None),
            Filtro::Graves => (tx.t("lec2-graves"), Some(v2::ROJO)),
            Filtro::Area(a) => (a.clone(), None),
        };
        fichas.push((rot, punto, e.filtro == f, Accion::Filtro(i), Some(n.to_string())));
    }
    for (rot, punto, puesta, que, cuenta) in fichas {
        let tw = p.medir_texto(&rot, 13.0 * s).0;
        let cuenta_w = cuenta.as_ref().map_or(0.0, |c| p.medir_texto(c, 12.0 * s).0 + 6.0 * s);
        let w = 24.0 * s + tw + cuenta_w + if punto.is_some() { 14.0 * s } else { 0.0 };
        if fx > x && fx + w > x + ancho {
            fx = x;
            y += alto_ficha + 6.0 * s;
        }
        let caja = RectF { x: fx, y, ancho: w, alto: alto_ficha };
        let encima = dentro(caja, e.botones.raton);
        let fondo = if puesta { v2::TEXTO } else if encima { con_alfa(v2::BLANCO, 0.12) } else { con_alfa(v2::BLANCO, 0.07) };
        p.rellenar_redondeado(caja, 16.0 * s, fondo);
        let mut xx = fx + 12.0 * s;
        if let Some(c) = punto {
            p.circulo((xx + 4.0 * s, y + 16.0 * s), 4.0 * s, c);
            xx += 14.0 * s;
        }
        p.texto(&rot, xx, y + 7.0 * s, 13.0 * s, if puesta { v2::OSCURO } else { hex(0xe5e5ea) });
        if let Some(c) = cuenta {
            ui::negrita(p, &c, xx + tw + 6.0 * s, y + 8.0 * s, 12.0 * s, 100.0 * s, if puesta { hex(0x48484a) } else { v2::APAGADO });
        }
        e.botones.zona(caja, que);
        fx += w + 6.0 * s;
    }
    y += alto_ficha + 10.0 * s;

    // Para repasar hoy.
    if !e.hoy.is_empty() {
        let caja = RectF { x, y, ancho, alto: 64.0 * s };
        p.rellenar_redondeado(caja, 12.0 * s, ui::mezclar(v2::COLUMNA, v2::AMARILLO, 0.15));
        ui::negrita(p, &t1(&tx, "lec2-para-repasar", "n", e.hoy.len() as i64), x + 14.0 * s, y + 13.0 * s, 14.0 * s, ancho - 150.0 * s, v2::TEXTO);
        p.texto_linea(&tx.t("lec2-que-harias-si"), x + 14.0 * s, y + 35.0 * s, 12.0 * s, ancho - 150.0 * s, v2::SUAVE);
        let rot = tx.t("lec2-repasar");
        let bw = ui::ancho_de_boton(p, false, &rot, Some("R"), s);
        let b = RectF { x: x + ancho - bw - 12.0 * s, y: y + 12.0 * s, ancho: bw, alto: 40.0 * s };
        let encima = dentro(b, e.botones.raton);
        p.rellenar_redondeado(b, 10.0 * s, if encima { ui::aclarar(v2::AMARILLO, 0.08) } else { v2::AMARILLO });
        let (rw, rh) = ui::medir_negrita(p, &rot, 14.0 * s, 300.0 * s);
        ui::negrita(p, &rot, b.x + 14.0 * s, b.y + (b.alto - rh) / 2.0, 14.0 * s, 300.0 * s, v2::OSCURO);
        ui::chapa(p, "R", b.x + 22.0 * s + rw, b.y + 20.0 * s, v2::OSCURO, ui::mezclar(v2::AMARILLO, v2::OSCURO, 0.12), s);
        e.botones.zona(b, Accion::Repasar);
        y += 64.0 * s + 8.0 * s;
    }

    // Las filas, desplazables, hasta el pie.
    let pie = 44.0 * s;
    let vista = RectF { x: r.x, y, ancho: r.ancho, alto: (r.y + r.alto - pie - y).max(0.0) };
    e.zona_lista = vista;
    e.scroll_lista = e.scroll_lista.clamp(0.0, (e.alto_lista - vista.alto).max(0.0));
    let filas = e.visibles.clone();
    // Que se vea la elegida al moverse con las flechas.
    if e.seguir_sel {
        e.seguir_sel = false;
        if let Some(pos) = e.sel.as_ref().and_then(|id| filas.iter().position(|i| e.todas[*i].leccion.id == *id)) {
            let grupos_antes = 1.0 + if pos > 0 && cambia_de_grupo(e, &filas, pos) { 1.0 } else { 0.0 };
            let fy = pos as f32 * ALTO_FILA * s + grupos_antes * 30.0 * s;
            if fy < e.scroll_lista {
                e.scroll_lista = (fy - 30.0 * s).max(0.0);
            } else if fy + ALTO_FILA * s > e.scroll_lista + vista.alto {
                e.scroll_lista = fy + ALTO_FILA * s - vista.alto + 8.0 * s;
            }
        }
    }
    let mut total = 0.0;
    p.con_recorte(vista, |p| {
        let y0 = vista.y - e.scroll_lista;
        let mut yy = y0;
        if filas.is_empty() {
            let t = if e.todas.is_empty() {
                tx.t("lec2-lista-vacia")
            } else if e.consulta.texto.trim().is_empty() {
                tx.t("lec2-nada-con-filtro")
            } else {
                t1(&tx, "lec2-nada-con", "q", e.consulta.texto.trim().to_string())
            };
            p.texto_ajustado(&t, x + 4.0 * s, yy + 10.0 * s, 14.0 * s, ancho - 8.0 * s, v2::APAGADO);
            yy += 60.0 * s;
        }
        let mut grupo: Option<bool> = None;
        for i in filas.iter().copied() {
            let este_mes = ui::mismo_mes(e.todas[i].leccion.tocada, e.ahora);
            if grupo != Some(este_mes) {
                grupo = Some(este_mes);
                let rot = if este_mes { tx.t("lec2-este-mes") } else { tx.t("lec2-antes") };
                ui::negrita(p, &rot, x + 6.0 * s, yy + 12.0 * s, 12.0 * s, ancho, v2::APAGADO);
                yy += 30.0 * s;
            }
            let caja = RectF { x: r.x + 8.0 * s, y: yy, ancho: r.ancho - 16.0 * s, alto: ALTO_FILA * s - 4.0 * s };
            if caja.y + caja.alto >= vista.y && caja.y <= vista.y + vista.alto {
                fila(e, p, i, caja, vista, s);
            }
            yy += ALTO_FILA * s;
        }
        total = yy - y0 + 8.0 * s;
    });
    e.alto_lista = total;

    // El pie: la leyenda de los colores y las flechas.
    let py = r.y + r.alto - pie;
    p.rellenar(RectF { x: r.x, y: py, ancho: r.ancho - 1.0 * s, alto: 1.0 * s }, v2::LINEA);
    let cy = py + pie / 2.0;
    let mut lx = x;
    for g in 1..=3 {
        p.circulo((lx + 5.0 * s, cy), 5.0 * s, color_de_gravedad(g));
        let t = tx.t(nombre_de_gravedad(g));
        p.texto(&t, lx + 14.0 * s, cy - 8.0 * s, 12.0 * s, v2::APAGADO);
        lx += 14.0 * s + p.medir_texto(&t, 12.0 * s).0 + 12.0 * s;
    }
    let mover = tx.t("lec2-moverse");
    let mw = p.medir_texto(&mover, 12.0 * s).0;
    let mx = x + ancho - mw;
    if mx > lx + 60.0 * s {
        p.texto(&mover, mx, cy - 8.0 * s, 12.0 * s, v2::APAGADO);
        let w2 = ui::ancho_de_chapa(p, "↓", s);
        ui::chapa(p, "↓", mx - w2 - 6.0 * s, cy, v2::SUAVE, hex(0x2a2a2d), s);
        ui::chapa(p, "↑", mx - 2.0 * w2 - 10.0 * s, cy, v2::SUAVE, hex(0x2a2a2d), s);
    }
}

/// Si la fila `pos` empieza otro grupo (este mes / antes).
fn cambia_de_grupo(e: &Estado, filas: &[usize], pos: usize) -> bool {
    filas[..=pos].windows(2).any(|w| {
        ui::mismo_mes(e.todas[w[0]].leccion.tocada, e.ahora) != ui::mismo_mes(e.todas[w[1]].leccion.tocada, e.ahora)
    })
}

/// Una fila de la lista: punto, titulo, area · proyecto · cuando, y «2×».
fn fila(e: &mut Estado, p: &Pintor, i: usize, caja: RectF, vista: RectF, s: f32) {
    let tx = e.textos.clone();
    let x = &e.todas[i];
    let l = &x.leccion;
    let elegida = e.sel.as_deref() == Some(l.id.as_str());
    let encima = dentro(caja, e.botones.raton) && dentro(vista, e.botones.raton);
    if elegida {
        p.rellenar_redondeado(caja, 12.0 * s, con_alfa(v2::ELEGIDO, 0.16));
        p.rellenar(RectF { x: caja.x, y: caja.y + 6.0 * s, ancho: 3.0 * s, alto: caja.alto - 12.0 * s }, v2::ELEGIDO);
    } else if encima {
        p.rellenar_redondeado(caja, 12.0 * s, con_alfa(v2::BLANCO, 0.05));
    }
    p.circulo((caja.x + 17.0 * s, caja.y + 19.0 * s), 5.0 * s, color_de_gravedad(l.gravedad));
    let tx0 = caja.x + 32.0 * s;
    let mut derecha = 12.0 * s;
    if !l.repeticiones.is_empty() {
        let t = format!("{}×", l.veces_que_paso());
        let tw = p.medir_texto(&t, 12.0 * s).0 + 16.0 * s;
        let b = RectF { x: caja.x + caja.ancho - tw - 10.0 * s, y: caja.y + 10.0 * s, ancho: tw, alto: 24.0 * s };
        p.rellenar_redondeado(b, 8.0 * s, con_alfa(v2::AMARILLO, 0.12));
        p.texto(&t, b.x + 8.0 * s, b.y + 4.0 * s, 12.0 * s, v2::AMARILLO);
        derecha += tw + 6.0 * s;
    }
    let ancho = caja.ancho - (tx0 - caja.x) - derecha;
    if elegida {
        let titulo = ui::corto(&l.titulo, 200);
        p.con_recorte(RectF { x: tx0, y: caja.y, ancho, alto: caja.alto }, |p| {
            ui::negrita(p, &titulo, tx0, caja.y + 9.0 * s, 14.0 * s, 100_000.0, v2::TEXTO);
        });
    } else {
        p.texto_linea(&l.titulo.replace('\n', " "), tx0, caja.y + 9.0 * s, 14.0 * s, ancho, v2::TEXTO);
    }
    let mut sub: Vec<String> = Vec::new();
    sub.push(if l.area.trim().is_empty() { tx.t("lec2-sin-area") } else { l.area.clone() });
    if !x.general {
        sub.push(x.nombre_chat.clone());
    }
    sub.push(hace_texto(&tx, ui::hace(l.tocada, e.ahora)));
    p.texto_linea(&sub.join(" · "), tx0, caja.y + 31.0 * s, 12.0 * s, ancho, v2::APAGADO);
    e.botones.zona(caja, Accion::Fila(i));
}

pub fn hace_texto(tx: &pixpin_store::Catalogo, h: Hace) -> String {
    match h {
        Hace::Hoy => tx.t("lec2-hoy"),
        Hace::Ayer => tx.t("lec2-ayer"),
        Hace::Dias(n) => t1(tx, "lec2-hace-dias", "n", n),
        Hace::Semanas(n) => t1(tx, "lec2-hace-semanas", "n", n),
        Hace::Meses(n) => t1(tx, "lec2-hace-meses", "n", n),
        Hace::Anios(n) => t1(tx, "lec2-hace-anios", "n", n),
    }
}

/// «28 sep»: el dia y el mes corto.
pub fn fecha_corta(tx: &pixpin_store::Catalogo, ms: i64) -> String {
    let (_, m, d) = ui::fecha_de(ms);
    let meses = tx.t("lec2-meses");
    let mes = meses.split_whitespace().nth(m as usize - 1).unwrap_or("").to_string();
    format!("{d} {mes}")
}

// --------------------------------------------------------------- ficha

/// Un bloque de la ficha: su rotulo de color, su texto (o la caja para
/// editarlo) y, en «Que paso», las fotos y notas de voz.
struct Bloque {
    foco: Foco,
    rotulo: String,
    color: Color,
    icono: Option<&'static pixpin_render::icono::Icono>,
    texto: String,
    vacio: String,
    verde: bool,
}

fn columna_ficha(e: &mut Estado, p: &Pintor, r: RectF, s: f32) {
    let tx = e.textos.clone();
    let Some(x) = e.elegida().cloned() else {
        return;
    };
    let l = x.leccion.clone();
    let pad = 24.0 * s;
    let ix = r.x + pad;
    let iw = r.ancho - 2.0 * pad;
    let acciones = 40.0 * s + 20.0 * s;
    let vista = RectF { x: r.x, y: r.y, ancho: r.ancho, alto: (r.alto - acciones - 8.0 * s).max(0.0) };
    e.zona_ficha = vista;
    e.scroll_ficha = e.scroll_ficha.clamp(0.0, (e.alto_ficha - vista.alto).max(0.0));
    let mut total = 0.0;
    let raiz = e.raiz();
    let adjuntos = e.adjuntos.de(&raiz, &x, e.motor.as_deref()).clone();
    p.con_recorte(vista, |p| {
        let y0 = r.y + 20.0 * s - e.scroll_ficha;
        let mut y = y0;
        // La pista de arriba y «Mas campos».
        p.texto(&tx.t("lec2-clic-para-editar"), ix, y, 12.0 * s, v2::APAGADO);
        let mas = tx.t("lec2-mas-campos");
        let mw = p.medir_texto(&mas, 12.0 * s).0;
        let mc = RectF { x: ix + iw - mw - 8.0 * s, y: y - 8.0 * s, ancho: mw + 8.0 * s, alto: 32.0 * s };
        p.texto(&mas, mc.x + 4.0 * s, y, 12.0 * s, if dentro(mc, e.botones.raton) { hex(0xa0e4ff) } else { v2::CIAN });
        e.botones.zona(mc, Accion::MasCampos);
        y += 22.0 * s;

        // El titulo.
        if e.foco == Some(Foco::Titulo) {
            y = caja_de_edicion(e, p, ix - 8.0 * s, y, iw + 16.0 * s, 24.0 * s, Foco::Titulo, s);
        } else {
            let (tw, th) = ui::medir_negrita(p, &l.titulo, 26.0 * s, iw - 30.0 * s);
            ui::negrita(p, &l.titulo, ix, y, 26.0 * s, iw - 30.0 * s, v2::TEXTO);
            let zona = RectF { x: ix - 6.0 * s, y: y - 4.0 * s, ancho: iw + 12.0 * s, alto: th + 8.0 * s };
            if dentro(zona, e.botones.raton) {
                p.rellenar_redondeado(zona, 8.0 * s, con_alfa(v2::BLANCO, 0.04));
            }
            // El lapiz detras de la ultima linea.
            let ultima = tw.min(iw - 30.0 * s);
            p.icono(&mi::EDIT, RectF { x: ix + ultima + 10.0 * s, y: y + th - 26.0 * s, ancho: 16.0 * s, alto: 16.0 * s }, v2::APAGADO);
            e.botones.zona(zona, Accion::Editar(Foco::Titulo));
            y += th + 14.0 * s;
        }

        let bloques = [
            Bloque {
                foco: Foco::Paso,
                rotulo: tx.t("lec2-que-paso"),
                color: v2::CIAN,
                icono: Some(&mi::ALARM),
                texto: l.que_paso.clone(),
                vacio: tx.t("lec2-que-paso-vacio"),
                verde: false,
            },
            Bloque {
                foco: Foco::PorQue,
                rotulo: tx.t("lec2-por-que"),
                color: v2::NARANJA,
                icono: None,
                texto: l.por_que.clone(),
                vacio: tx.t("lec2-por-que-vacio"),
                verde: false,
            },
            Bloque {
                foco: Foco::Proxima,
                rotulo: tx.t("lec2-proxima"),
                color: v2::VERDE,
                icono: Some(&mi::CHECK_CIRCLE),
                texto: l.proxima.clone(),
                vacio: tx.t("lec2-proxima-vacio"),
                verde: true,
            },
        ];
        for b in bloques {
            let con_adjuntos = b.foco == Foco::Paso;
            y = bloque(e, p, &b, ix, y, iw, if con_adjuntos { Some(&adjuntos) } else { None }, s) + 12.0 * s;
        }
        total = y - y0 + 10.0 * s;
    });
    e.alto_ficha = total;

    // Las acciones, siempre abajo.
    let ay = r.y + r.alto - acciones + 4.0 * s;
    p.rellenar(RectF { x: r.x, y: ay - 12.0 * s, ancho: r.ancho, alto: 1.0 * s }, v2::LINEA);
    let ver = tx.t("lec2-ver-en-chat");
    let pin = tx.t("lec2-pinear");
    let w2 = ui::ancho_de_boton(p, true, &pin, Some("P"), s);
    let borrar = tx.t("lec2-borrar");
    let w3 = ui::ancho_de_boton(p, true, &borrar, Some("Supr"), s);
    // Estrecha: «Ver en el chat» se queda en su icono para que quepa Borrar.
    let mut w1 = ui::ancho_de_boton(p, true, &ver, None, s);
    let solo_icono = w1 + w2 + w3 + 24.0 * s > iw;
    if solo_icono {
        w1 = 40.0 * s;
    }
    let b1 = RectF { x: ix, y: ay, ancho: w1, alto: 40.0 * s };
    if solo_icono {
        boton_icono(e, p, b1, Accion::VerEnChat, &mi::FORUM, Some(con_alfa(v2::BLANCO, 0.08)), s);
    } else {
        ui::boton_v2(p, &mut e.botones, b1, Accion::VerEnChat, Some(&mi::FORUM), &ver, None, Some(con_alfa(v2::BLANCO, 0.08)), v2::TEXTO, s);
    }
    let b2 = RectF { x: b1.x + w1 + 8.0 * s, y: ay, ancho: w2, alto: 40.0 * s };
    ui::boton_v2(p, &mut e.botones, b2, Accion::Pinear, Some(&mi::PUSH_PIN), &pin, Some("P"), Some(con_alfa(v2::BLANCO, 0.08)), v2::TEXTO, s);
    let b3 = RectF { x: (ix + iw - w3).max(b2.x + w2 + 8.0 * s), y: ay, ancho: w3, alto: 40.0 * s };
    ui::boton_v2(p, &mut e.botones, b3, Accion::Borrar, Some(&mi::DELETE), &borrar, Some("Supr"), None, v2::ROJO_TEXTO, s);
}

/// La caja donde se edita un texto de la ficha, con su borde azul. Devuelve
/// la y de debajo.
#[allow(clippy::too_many_arguments)] // estado, pintor, sitio, ancho, tamano, foco y escala
fn caja_de_edicion(e: &mut Estado, p: &Pintor, x: f32, y: f32, ancho: f32, tam: f32, f: Foco, s: f32) -> f32 {
    let pad = 10.0 * s;
    let (_, linea) = p.medir_texto("Ag", tam);
    let t = if e.editando.texto.ends_with('\n') { format!("{}.", e.editando.texto) } else { e.editando.texto.clone() };
    let (_, th) = if t.is_empty() { (0.0, linea) } else { p.medir_texto_ajustado(&t, tam, ancho - 2.0 * pad) };
    let caja = RectF { x, y, ancho, alto: th.max(linea) + 2.0 * pad };
    p.rellenar_redondeado(ui::encoger(caja, -2.0 * s), 12.0 * s, v2::ELEGIDO);
    p.rellenar_redondeado(caja, 10.0 * s, v2::CAJA);
    let pista = e.textos.t("lec2-escribe-aqui");
    e.editando.pintar_texto(p, x + pad, y + pad, ancho - 2.0 * pad, tam, true, &pista, s);
    e.botones.zona(caja, Accion::Editar(f));
    // Como se guarda.
    let ayuda = if f.multilinea() { e.textos.t("lec2-ayuda-bloque") } else { e.textos.t("lec2-ayuda-linea") };
    p.texto(&ayuda, x + 4.0 * s, caja.y + caja.alto + 4.0 * s, 11.5 * s, v2::APAGADO);
    caja.y + caja.alto + 22.0 * s
}

/// Un bloque de la ficha. Devuelve la y de debajo.
#[allow(clippy::too_many_arguments)] // estado, pintor, bloque, sitio, ancho, adjuntos y escala
fn bloque(e: &mut Estado, p: &Pintor, b: &Bloque, x: f32, y: f32, ancho: f32, adjuntos: Option<&Vec<super::adjuntos::Adjunto>>, s: f32) -> f32 {
    let tx = e.textos.clone();
    let pad_x = 16.0 * s;
    let iw = ancho - 2.0 * pad_x;
    let editando = e.foco == Some(b.foco);
    let tam = if b.verde { 16.0 * s } else { 15.0 * s };
    // Lo que mide, antes de pintar el fondo.
    let texto = if b.texto.trim().is_empty() { &b.vacio } else { &b.texto };
    let alto_texto = if editando {
        let (_, linea) = p.medir_texto("Ag", tam);
        let t = if e.editando.texto.ends_with('\n') { format!("{}.", e.editando.texto) } else { e.editando.texto.clone() };
        let (_, th) = if t.is_empty() { (0.0, linea) } else { p.medir_texto_ajustado(&t, tam, iw - 20.0 * s) };
        th.max(linea) + 20.0 * s + 20.0 * s
    } else {
        p.medir_texto_ajustado(texto, tam, iw).1
    };
    let n_adj = adjuntos.map_or(0, Vec::len);
    let con_fila = adjuntos.is_some();
    let alto_adj = if con_fila { 12.0 * s + 72.0 * s } else { 0.0 };
    let alto = 14.0 * s + 26.0 * s + alto_texto + alto_adj + 14.0 * s;
    let caja = RectF { x, y, ancho, alto };
    let encima = dentro(caja, e.botones.raton);
    if b.verde {
        p.rellenar_redondeado(caja, 14.0 * s, ui::mezclar(v2::FONDO, v2::VERDE, 0.10));
        p.rellenar(RectF { x, y: y + 8.0 * s, ancho: 3.0 * s, alto: alto - 16.0 * s }, v2::VERDE);
    } else {
        p.rellenar_redondeado(caja, 14.0 * s, if encima && !editando { hex(0x29292c) } else { v2::COLUMNA });
    }
    if !editando {
        e.botones.zona(caja, Accion::Editar(b.foco));
    }
    // El rotulo.
    let mut rx = x + pad_x;
    let ry = y + 14.0 * s;
    match b.icono {
        Some(i) => p.icono(i, RectF { x: rx, y: ry, ancho: 15.0 * s, alto: 15.0 * s }, b.color),
        None => {
            // «?» en un circulo: no hay icono de ayuda en el juego de iconos.
            let c = (rx + 7.5 * s, ry + 7.5 * s);
            p.anillo(c, 6.8 * s, 1.6 * s, b.color);
            let (qw, qh) = ui::medir_negrita(p, "?", 10.0 * s, 50.0 * s);
            ui::negrita(p, "?", c.0 - qw / 2.0, c.1 - qh / 2.0, 10.0 * s, 50.0 * s, b.color);
        }
    }
    rx += 23.0 * s;
    ui::negrita(p, &b.rotulo.to_uppercase(), rx, ry + 0.5 * s, 12.0 * s, iw, b.color);
    if encima && !editando {
        let rot = tx.t("lec2-editar");
        let ew = p.medir_texto(&rot, 12.0 * s).0 + 36.0 * s;
        let eb = RectF { x: x + ancho - pad_x - ew, y: y + 8.0 * s, ancho: ew, alto: 28.0 * s };
        p.rellenar_redondeado(eb, 8.0 * s, con_alfa(v2::BLANCO, 0.08));
        p.icono(&mi::EDIT, RectF { x: eb.x + 10.0 * s, y: eb.y + 8.0 * s, ancho: 12.0 * s, alto: 12.0 * s }, hex(0xe5e5ea));
        p.texto(&rot, eb.x + 28.0 * s, eb.y + 6.0 * s, 12.0 * s, hex(0xe5e5ea));
    }
    let ty = y + 14.0 * s + 26.0 * s;
    if editando {
        caja_de_edicion(e, p, x + pad_x - 6.0 * s, ty - 4.0 * s, iw + 12.0 * s, tam, b.foco, s);
    } else {
        let color = if b.texto.trim().is_empty() { v2::APAGADO } else { v2::CUERPO };
        p.texto_ajustado(texto, x + pad_x, ty, tam, iw, color);
    }
    // Las fotos y notas de voz.
    if let Some(adj) = adjuntos {
        let mut ax = x + pad_x;
        let ay = ty + alto_texto + 12.0 * s;
        for (k, a) in adj.iter().enumerate() {
            match a.clase {
                super::adjuntos::Clase::Foto => {
                    let celda = RectF { x: ax, y: ay, ancho: 112.0 * s, alto: 72.0 * s };
                    if ax + celda.ancho > x + ancho - 60.0 * s {
                        break;
                    }
                    p.rellenar_redondeado(celda, 10.0 * s, v2::CAJA);
                    if let Some(bm) = &a.bitmap {
                        let fuente = super::adjuntos::recorte(a.ancho, a.alto, celda);
                        p.con_recorte(celda, |p| p.bitmap(bm, celda, Some(fuente), false));
                    } else {
                        p.icono(&mi::IMAGE, ui::encoger(celda, 24.0 * s), v2::APAGADO);
                    }
                    if dentro(celda, e.botones.raton) {
                        p.rellenar_redondeado(celda, 10.0 * s, con_alfa(v2::BLANCO, 0.10));
                        p.icono(&mi::PUSH_PIN, RectF { x: celda.x + celda.ancho - 24.0 * s, y: celda.y + 6.0 * s, ancho: 18.0 * s, alto: 18.0 * s }, v2::TEXTO);
                    }
                    e.botones.zona(celda, Accion::Adjunto(k));
                    ax += celda.ancho + 10.0 * s;
                }
                super::adjuntos::Clase::Voz => {
                    let celda = RectF { x: ax, y: ay + 8.0 * s, ancho: 200.0 * s, alto: 56.0 * s };
                    if ax + celda.ancho > x + ancho - 60.0 * s {
                        break;
                    }
                    p.rellenar_redondeado(celda, 12.0 * s, if dentro(celda, e.botones.raton) { v2::ENCIMA } else { v2::CAJA });
                    let c = (celda.x + 30.0 * s, celda.y + 28.0 * s);
                    p.circulo(c, 20.0 * s, v2::AZUL);
                    p.icono(&mi::PLAY_ARROW, RectF { x: c.0 - 9.0 * s, y: c.1 - 9.0 * s, ancho: 18.0 * s, alto: 18.0 * s }, v2::BLANCO);
                    p.texto(&tx.t("lec2-nota-de-voz"), celda.x + 60.0 * s, celda.y + 18.0 * s, 13.0 * s, v2::TEXTO);
                    e.botones.zona(celda, Accion::Adjunto(k));
                    ax += celda.ancho + 10.0 * s;
                }
            }
        }
        // Anadir una foto.
        let mas = RectF { x: ax, y: ay + 8.0 * s, ancho: 48.0 * s, alto: 56.0 * s };
        let encima = dentro(mas, e.botones.raton);
        if encima {
            p.rellenar_redondeado(mas, 12.0 * s, con_alfa(v2::BLANCO, 0.06));
        }
        p.trazar_discontinuo(mas, 1.0 * s, con_alfa(v2::BLANCO, 0.30));
        p.linea((mas.x + 24.0 * s, mas.y + 20.0 * s), (mas.x + 24.0 * s, mas.y + 36.0 * s), 2.0 * s, v2::SUAVE);
        p.linea((mas.x + 16.0 * s, mas.y + 28.0 * s), (mas.x + 32.0 * s, mas.y + 28.0 * s), 2.0 * s, v2::SUAVE);
        e.botones.zona(mas, Accion::AnadirAdjunto);
        if n_adj == 0 {
            p.texto(&tx.t("lec2-anadir-foto"), mas.x + 60.0 * s, mas.y + 19.0 * s, 13.0 * s, v2::APAGADO);
        }
    }
    y + alto
}

// --------------------------------------------------------------- datos

fn columna_datos(e: &mut Estado, p: &Pintor, r: RectF, s: f32) {
    let tx = e.textos.clone();
    let Some(x) = e.elegida().cloned() else {
        return;
    };
    let l = x.leccion.clone();
    p.rellenar(RectF { x: r.x, y: r.y, ancho: 1.0 * s, alto: r.alto }, v2::LINEA);
    let pad = 18.0 * s;
    let ix = r.x + pad;
    let iw = r.ancho - 2.0 * pad;
    let mut y = r.y + 20.0 * s;

    // Cuantas veces paso.
    let caja = RectF { x: ix, y, ancho: iw, alto: 128.0 * s };
    p.rellenar_redondeado(caja, 14.0 * s, v2::CAJA);
    let n = l.veces_que_paso();
    let num = n.to_string();
    let (nw, _) = ui::medir_negrita(p, &num, 30.0 * s, 200.0 * s);
    ui::negrita(p, &num, ix + 14.0 * s, y + 8.0 * s, 30.0 * s, 200.0 * s, v2::AMARILLO);
    p.texto_linea(&t1(&tx, "lec2-veces", "n", n as i64), ix + 22.0 * s + nw, y + 22.0 * s, 14.0 * s, iw - nw - 30.0 * s, v2::TEXTO);
    let ultima = l.repeticiones.iter().copied().max().unwrap_or(l.creada);
    let donde = if x.general { String::new() } else { format!(" · {}", x.nombre_chat) };
    p.texto_linea(&format!("{}{donde}", t1(&tx, "lec2-la-ultima", "fecha", fecha_corta(&tx, ultima))), ix + 14.0 * s, y + 50.0 * s, 12.0 * s, iw - 28.0 * s, v2::APAGADO);
    let b = RectF { x: ix + 14.0 * s, y: y + 74.0 * s, ancho: iw - 28.0 * s, alto: 40.0 * s };
    let encima = dentro(b, e.botones.raton);
    p.rellenar_redondeado(b, 10.0 * s, con_alfa(v2::AMARILLO, if encima { 0.24 } else { 0.16 }));
    let rot = tx.t("lec2-me-volvio");
    let rw = ui::medir_negrita(p, &rot, 14.0 * s, 400.0 * s).0;
    let total_w = 26.0 * s + rw + 8.0 * s + ui::ancho_de_chapa(p, "+1", s);
    let mut bx = b.x + ((b.ancho - total_w) / 2.0).max(6.0 * s);
    p.icono(&mi::REPLAY, RectF { x: bx, y: b.y + 11.0 * s, ancho: 18.0 * s, alto: 18.0 * s }, v2::AMARILLO);
    bx += 26.0 * s;
    ui::negrita(p, &rot, bx, b.y + 11.0 * s, 14.0 * s, b.ancho, v2::AMARILLO);
    if total_w + 12.0 * s <= b.ancho {
        ui::chapa(p, "+1", bx + rw + 8.0 * s, b.y + 20.0 * s, v2::AMARILLO, ui::mezclar(v2::CAJA, v2::AMARILLO, 0.2), s);
    }
    e.botones.zona(b, Accion::OtraVez);
    y += caja.alto + 16.0 * s;

    // Gravedad.
    p.texto(&tx.t("lec2-gravedad"), ix, y, 12.0 * s, v2::APAGADO);
    y += 20.0 * s;
    let seg = RectF { x: ix, y, ancho: iw, alto: 42.0 * s };
    p.rellenar_redondeado(seg, 10.0 * s, v2::CAJA);
    let bw = (iw - 6.0 * s - 8.0 * s) / 3.0;
    for g in 1..=3 {
        let b = RectF { x: ix + 3.0 * s + (g - 1) as f32 * (bw + 4.0 * s), y: y + 3.0 * s, ancho: bw, alto: 36.0 * s };
        let puesta = l.gravedad.clamp(1, 3) == g;
        if puesta {
            p.rellenar_redondeado(b, 8.0 * s, color_de_gravedad(g));
        } else if dentro(b, e.botones.raton) {
            p.rellenar_redondeado(b, 8.0 * s, con_alfa(v2::BLANCO, 0.08));
        }
        let t = tx.t(nombre_de_gravedad(g));
        // La letra se encoge si el rotulo no cabe (columna estrecha).
        let mut tam = 13.0 * s;
        while tam > 10.0 * s && ui::medir_negrita(p, &t, tam, 10_000.0).0 > bw - 6.0 * s {
            tam -= 0.5 * s;
        }
        let (tw, th) = if puesta { ui::medir_negrita(p, &t, tam, 10_000.0) } else { p.medir_texto(&t, tam) };
        let (tx0, ty0) = (b.x + (b.ancho - tw) / 2.0, b.y + (b.alto - th) / 2.0);
        if puesta {
            ui::negrita(p, &t, tx0, ty0, tam, 10_000.0, v2::OSCURO);
        } else {
            p.texto(&t, tx0, ty0, tam, v2::TEXTO);
        }
        e.botones.zona(b, Accion::Gravedad(g));
    }
    y += 42.0 * s + 14.0 * s;

    // Area.
    p.texto(&tx.t("lec2-area"), ix, y, 12.0 * s, v2::APAGADO);
    y += 20.0 * s;
    let pill = RectF { x: ix, y, ancho: iw, alto: 40.0 * s };
    let encima = dentro(pill, e.botones.raton) || e.menu == Some(Menu::AreaFicha);
    p.rellenar_redondeado(pill, 10.0 * s, if encima { v2::ENCIMA } else { v2::CAJA });
    let area = if l.area.trim().is_empty() { tx.t("lec2-sin-area") } else { l.area.clone() };
    p.circulo((ix + 17.0 * s, y + 20.0 * s), 5.0 * s, color_de(&l.area));
    p.texto_linea(&area, ix + 30.0 * s, y + 11.0 * s, 14.0 * s, iw - 60.0 * s, v2::TEXTO);
    p.icono(&mi::KEYBOARD_ARROW_DOWN, RectF { x: ix + iw - 28.0 * s, y: y + 12.0 * s, ancho: 16.0 * s, alto: 16.0 * s }, v2::APAGADO);
    e.botones.zona(pill, Accion::AbrirMenu(Menu::AreaFicha));
    if e.menu == Some(Menu::AreaFicha) {
        e.ancla = pill;
    }
    y += 40.0 * s + 14.0 * s;

    // Proyecto (el chat donde vive: no se cambia desde aqui, como en el movil).
    p.texto(&tx.t("lec2-proyecto"), ix, y, 12.0 * s, v2::APAGADO);
    y += 20.0 * s;
    let pill = RectF { x: ix, y, ancho: iw, alto: 40.0 * s };
    p.rellenar_redondeado(pill, 10.0 * s, v2::CAJA);
    let nombre = if x.general { tx.t("lec2-sin-proyecto") } else { x.nombre_chat.clone() };
    p.rellenar_redondeado(RectF { x: ix + 12.0 * s, y: y + 15.0 * s, ancho: 10.0 * s, alto: 10.0 * s }, 3.0 * s, if x.general { v2::APAGADO } else { color_de(&x.nombre_chat) });
    p.texto_linea(&nombre, ix + 30.0 * s, y + 11.0 * s, 14.0 * s, iw - 40.0 * s, v2::TEXTO);
    y += 40.0 * s + 14.0 * s;

    // Etiquetas.
    p.texto(&tx.t("lec2-etiquetas"), ix, y, 12.0 * s, v2::APAGADO);
    y += 20.0 * s;
    let mut tx_ = ix;
    for (i, t) in l.todas_las_etiquetas().iter().enumerate() {
        let auto = !l.etiquetas.contains(t);
        let rot = if auto { format!("✨ {t}") } else { t.clone() };
        let w = p.medir_texto(&rot, 13.0 * s).0 + 20.0 * s + 16.0 * s;
        if tx_ > ix && tx_ + w > ix + iw {
            tx_ = ix;
            y += 36.0 * s;
        }
        let c = RectF { x: tx_, y, ancho: w, alto: 30.0 * s };
        let encima = dentro(c, e.botones.raton);
        p.rellenar_redondeado(c, 8.0 * s, if encima { v2::ENCIMA } else { v2::CAJA });
        p.texto_color(&rot, tx_ + 10.0 * s, y + 6.0 * s, 13.0 * s, hex(0xe5e5ea));
        let xx = c.x + c.ancho - 16.0 * s;
        let col = if encima { v2::TEXTO } else { v2::APAGADO };
        p.linea((xx - 3.5 * s, y + 11.5 * s), (xx + 3.5 * s, y + 18.5 * s), 1.4 * s, col);
        p.linea((xx - 3.5 * s, y + 18.5 * s), (xx + 3.5 * s, y + 11.5 * s), 1.4 * s, col);
        e.botones.zona(c, Accion::QuitarEtiqueta(i));
        tx_ += w + 6.0 * s;
    }
    if e.foco == Some(Foco::Etiqueta) {
        if tx_ > ix && tx_ + 140.0 * s > ix + iw {
            tx_ = ix;
            y += 36.0 * s;
        }
        let w = (ix + iw - tx_).max(120.0 * s);
        let c = RectF { x: tx_, y, ancho: w, alto: 30.0 * s };
        p.rellenar_redondeado(ui::encoger(c, -2.0 * s), 9.0 * s, v2::ELEGIDO);
        p.rellenar_redondeado(c, 8.0 * s, v2::CAJA);
        let pista = tx.t("lec2-nueva-etiqueta");
        let ed = &e.editando;
        p.con_recorte(c, |p| ed.pintar_texto(p, c.x + 10.0 * s, y + 6.0 * s, 100_000.0, 13.0 * s, true, &pista, s));
        e.botones.zona(c, Accion::Editar(Foco::Etiqueta));
    } else {
        let rot = tx.t("lec2-anadir");
        let w = p.medir_texto(&rot, 13.0 * s).0 + 20.0 * s;
        if tx_ > ix && tx_ + w > ix + iw {
            tx_ = ix;
            y += 36.0 * s;
        }
        let c = RectF { x: tx_, y, ancho: w, alto: 30.0 * s };
        if dentro(c, e.botones.raton) {
            p.rellenar_redondeado(c, 8.0 * s, con_alfa(v2::BLANCO, 0.06));
        }
        p.trazar_discontinuo(c, 1.0 * s, con_alfa(v2::BLANCO, 0.25));
        p.texto(&rot, tx_ + 10.0 * s, y + 6.0 * s, 13.0 * s, v2::SUAVE);
        e.botones.zona(c, Accion::Editar(Foco::Etiqueta));
    }
    y += 30.0 * s + 14.0 * s;

    // Relacionadas.
    let rel = e.relacionadas();
    let pie = r.y + r.alto - 40.0 * s;
    if !rel.is_empty() && y + 64.0 * s < pie {
        p.texto(&tx.t("lec2-relacionadas"), ix, y, 12.0 * s, v2::APAGADO);
        y += 20.0 * s;
        for (i, o) in rel.iter().enumerate() {
            if y + 44.0 * s > pie {
                break;
            }
            let c = RectF { x: ix, y, ancho: iw, alto: 44.0 * s };
            let encima = dentro(c, e.botones.raton);
            p.rellenar_redondeado(c, 10.0 * s, if encima { v2::ENCIMA } else { v2::CAJA });
            p.circulo((ix + 15.0 * s, y + 22.0 * s), 5.0 * s, color_de_gravedad(o.gravedad));
            p.texto_linea(&o.titulo.replace('\n', " "), ix + 30.0 * s, y + 13.0 * s, 13.0 * s, iw - 56.0 * s, v2::TEXTO);
            p.icono(&mi::ARROW_FORWARD, RectF { x: ix + iw - 24.0 * s, y: y + 15.0 * s, ancho: 14.0 * s, alto: 14.0 * s }, v2::APAGADO);
            e.botones.zona(c, Accion::Relacionada(i));
            y += 50.0 * s;
        }
    }

    // El proximo repaso.
    p.icono(&mi::ALARM, RectF { x: ix, y: pie + 12.0 * s, ancho: 14.0 * s, alto: 14.0 * s }, v2::APAGADO);
    p.texto_linea(&proximo_repaso(&tx, &l, e.ahora), ix + 20.0 * s, pie + 11.0 * s, 12.0 * s, iw - 20.0 * s, v2::APAGADO);
}

/// «Proximo repaso: hoy / manana / en 5 dias».
pub fn proximo_repaso(tx: &pixpin_store::Catalogo, l: &Leccion, ahora: i64) -> String {
    let dia = pixpin_lecciones::leccion::DIA;
    let dias = l.repasar.div_euclid(dia) - ahora.div_euclid(dia);
    let cuando = match dias {
        i64::MIN..=0 => tx.t("lec2-hoy"),
        1 => tx.t("lec2-manana"),
        n => t1(tx, "lec2-en-dias", "n", n),
    };
    t1(tx, "lec2-proximo-repaso", "cuando", cuando)
}

// --------------------------------------------------------- lo de encima

fn vacia(e: &mut Estado, p: &Pintor, r: RectF, s: f32) {
    let tx = e.textos.clone();
    let cx = r.x + r.ancho / 2.0;
    let mut y = r.y + r.alto * 0.28;
    let (bw, _) = p.medir_texto("💡", 48.0 * s);
    p.texto_color("💡", cx - bw / 2.0, y, 48.0 * s, v2::TEXTO);
    y += 80.0 * s;
    let t = tx.t("lec2-vacia-titulo");
    let (tw, _) = ui::medir_negrita(p, &t, 18.0 * s, r.ancho);
    ui::negrita(p, &t, cx - tw / 2.0, y, 18.0 * s, r.ancho, v2::TEXTO);
    y += 34.0 * s;
    let sub = tx.t("lec2-vacia-texto");
    let anch = (r.ancho - 80.0 * s).min(520.0 * s);
    p.texto_ajustado(&sub, cx - anch / 2.0, y, 14.0 * s, anch, v2::APAGADO);
}

/// **La lista de comprobacion** (la del movil): lo de «la proxima vez…»
/// como casillas, para mirarla antes de empezar. No se guardan.
fn comprobacion(e: &mut Estado, p: &Pintor, r: RectF, s: f32) {
    let tx = e.textos.clone();
    let mut filas: Vec<usize> = e
        .visibles
        .iter()
        .copied()
        .filter(|i| {
            let l = &e.todas[*i].leccion;
            l.en_lista && (!l.proxima.trim().is_empty() || l.es_error())
        })
        .collect();
    filas.sort_by(|a, b| {
        let (la, lb) = (&e.todas[*a].leccion, &e.todas[*b].leccion);
        lb.gravedad.cmp(&la.gravedad).then(lb.repeticiones.len().cmp(&la.repeticiones.len()))
    });
    let pad = 24.0 * s;
    let x = r.x + pad;
    let ancho = r.ancho - 2.0 * pad;
    let hechas = filas.iter().filter(|i| e.marcadas.contains(&e.todas[**i].leccion.id)).count();
    let mut y = r.y + 20.0 * s;
    ui::negrita(p, &t2(&tx, "lec2-comprobacion", "hechas", hechas as i64, "total", filas.len() as i64), x, y, 18.0 * s, ancho, v2::TEXTO);
    y += 36.0 * s;
    if filas.is_empty() {
        p.texto_ajustado(&tx.t("lec2-comprobacion-vacia"), x, y, 14.0 * s, ancho, v2::APAGADO);
        return;
    }
    let vista = RectF { x: r.x, y, ancho: r.ancho, alto: r.y + r.alto - y };
    e.zona_ficha = vista;
    e.scroll_ficha = e.scroll_ficha.clamp(0.0, (e.alto_ficha - vista.alto).max(0.0));
    let mut total = 0.0;
    p.con_recorte(vista, |p| {
        let y0 = y - e.scroll_ficha;
        let mut yy = y0;
        for i in filas {
            let caja = RectF { x, y: yy, ancho, alto: 58.0 * s };
            let l = e.todas[i].leccion.clone();
            let marcada = e.marcadas.contains(&l.id);
            p.rellenar_redondeado(caja, 12.0 * s, if dentro(caja, e.botones.raton) { v2::ENCIMA } else { v2::CAJA });
            let icono = if marcada { &mi::CHECK_BOX } else { &mi::CHECK_BOX_OUTLINE_BLANK };
            p.icono(icono, RectF { x: x + 12.0 * s, y: yy + 15.0 * s, ancho: 28.0 * s, alto: 28.0 * s }, if marcada { v2::VERDE } else { v2::APAGADO });
            let principal = if l.proxima.trim().is_empty() { &l.titulo } else { &l.proxima };
            p.texto_linea(principal, x + 52.0 * s, yy + 9.0 * s, 15.0 * s, ancho - 130.0 * s, if marcada { v2::APAGADO } else { v2::TEXTO });
            p.texto_linea(&l.titulo, x + 52.0 * s, yy + 33.0 * s, 12.0 * s, ancho - 130.0 * s, v2::APAGADO);
            p.circulo((x + ancho - 20.0 * s, yy + 29.0 * s), 5.0 * s, color_de_gravedad(l.gravedad));
            e.botones.zona(caja, Accion::Marcar(i));
            yy += 64.0 * s;
        }
        total = yy - y0;
    });
    e.alto_ficha = total;
}

/// El menu desplegado (area o proyecto), colgando de su boton.
fn menu(e: &mut Estado, p: &Pintor, w: f32, h: f32, s: f32) {
    let Some(m) = e.menu else {
        return;
    };
    let opciones = e.opciones_del_menu(m);
    let actual = match m {
        Menu::AreaRapida => e.area_rapida().unwrap_or_default(),
        Menu::ProyectoRapida => e.proyecto_rapida().unwrap_or_default(),
        Menu::AreaFicha => e.elegida().map(|x| x.leccion.area.clone()).unwrap_or_default(),
    };
    let fila = 36.0 * s;
    let ancho = opciones
        .iter()
        .map(|(t, _)| p.medir_texto(t, 14.0 * s).0 + 56.0 * s)
        .fold(e.ancla.ancho.max(200.0 * s), f32::max)
        .min(360.0 * s);
    let alto = fila * opciones.len() as f32 + 12.0 * s;
    let mut x = e.ancla.x;
    let mut y = e.ancla.y + e.ancla.alto + 4.0 * s;
    if x + ancho > w - 8.0 * s {
        x = w - 8.0 * s - ancho;
    }
    if y + alto > h - 8.0 * s {
        y = (e.ancla.y - alto - 4.0 * s).max(8.0 * s);
    }
    let caja = RectF { x, y, ancho, alto };
    p.rellenar_redondeado(ui::encoger(caja, -1.0 * s), 13.0 * s, con_alfa(v2::BLANCO, 0.14));
    p.rellenar_redondeado(caja, 12.0 * s, hex(0x2a2a2d));
    e.botones.zona(caja, Accion::AbrirMenu(m));
    for (i, (t, valor)) in opciones.iter().enumerate() {
        let r = RectF { x: x + 6.0 * s, y: y + 6.0 * s + i as f32 * fila, ancho: ancho - 12.0 * s, alto: fila };
        if dentro(r, e.botones.raton) {
            p.rellenar_redondeado(r, 8.0 * s, con_alfa(v2::BLANCO, 0.10));
        }
        if *valor == actual {
            p.icono(&mi::CHECK_CIRCLE, RectF { x: r.x + 8.0 * s, y: r.y + 10.0 * s, ancho: 16.0 * s, alto: 16.0 * s }, v2::ELEGIDO);
        }
        let color = if valor.is_empty() { v2::APAGADO } else { v2::TEXTO };
        p.texto_linea(t, r.x + 32.0 * s, r.y + 9.0 * s, 14.0 * s, r.ancho - 40.0 * s, color);
        e.botones.zona(r, Accion::Opcion(i));
    }
}

/// El aviso de abajo; con «Deshacer» mientras se puede.
fn aviso(e: &mut Estado, p: &Pintor, w: f32, h: f32, s: f32) {
    let tx = e.textos.clone();
    let (texto, deshacer) = if e.borrando.is_some() {
        (tx.t("lec2-borrada"), true)
    } else if let Some((t, _)) = &e.aviso {
        (t.clone(), false)
    } else {
        return;
    };
    let tam = 14.0 * s;
    let (tw, th) = p.medir_texto_ajustado(&texto, tam, w * 0.5);
    let rot = tx.t("lec2-deshacer");
    let extra = if deshacer { 16.0 * s + ui::ancho_de_boton(p, false, &rot, Some("Ctrl Z"), s) } else { 0.0 };
    let alto = (th + 20.0 * s).max(if deshacer { 52.0 * s } else { 0.0 });
    let caja = RectF { x: (w - tw - extra) / 2.0 - 16.0 * s, y: h - alto - 84.0 * s, ancho: tw + extra + 32.0 * s, alto };
    p.rellenar_redondeado(caja, 12.0 * s, Color { a: 0.95, ..hex(0x0e0e10) });
    p.rellenar_redondeado(ui::encoger(caja, 1.0 * s), 11.0 * s, hex(0x2a2a2d));
    p.texto_ajustado(&texto, caja.x + 16.0 * s, caja.y + (alto - th) / 2.0, tam, tw + 2.0, v2::TEXTO);
    if deshacer {
        let bw = extra - 16.0 * s;
        let b = RectF { x: caja.x + caja.ancho - bw - 8.0 * s, y: caja.y + 6.0 * s, ancho: bw, alto: alto - 12.0 * s };
        ui::boton_v2(p, &mut e.botones, b, Accion::Deshacer, None, &rot, Some("Ctrl Z"), None, v2::CIAN, s);
    }
}
