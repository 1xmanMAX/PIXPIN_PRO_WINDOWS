//! La pestana **«Momentos»**: el archivo como la segunda captura de WeChat
//! que mando el usuario. El ano en grande, el mes en pequeno y, por dia, su
//! numero grande (con el dia de la semana debajo) y en fila las miniaturas
//! de lo que paso. Un momento con foto ensena su primera foto; sin foto, un
//! cuadrado pastel con su titulo.
//!
//! Donde va cada cosa lo dice `disposicion::archivo`, lo mismo para pintar
//! que para el clic. Se calcula **una vez por cambio** (de momentos o de
//! ancho) y se guarda: antes se rehacia tres veces por fotograma.

use std::rc::Rc;

use super::*;
use pixpin_timeline::archivo as arch;

/// La disposicion de «Momentos» ya hecha, con su clave.
pub(super) struct CacheArchivo {
    clave: (u64, u32, u32),
    pub piezas: Vec<dis::PiezaArchivo>,
    pub alto: f32,
    /// Cada miniatura con su momento (indice en `Estado::momentos`), en `y`
    /// contada desde lo alto del contenido.
    pub minis: Vec<(usize, RectF)>,
}

/// Lo que cambia la disposicion: los ids y las horas de los momentos.
fn huella(e: &Estado) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for m in &e.momentos {
        for b in m.id.bytes().chain(m.cuando.to_le_bytes()) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01B3);
        }
    }
    h
}

/// La disposicion de «Momentos» para un ancho, de la cache si nada cambio.
pub(super) fn disposicion(e: &Estado, w: f32, s: f32) -> Rc<CacheArchivo> {
    let clave = (huella(e), w.to_bits(), s.to_bits());
    if let Some(c) = e.cache_archivo.borrow().as_ref()
        && c.clave == clave
    {
        return c.clone();
    }
    let anios = arch::agrupar(&e.momentos, e.desfase);
    let (piezas, alto) = dis::archivo(&anios, w, s);
    let indice: std::collections::HashMap<&str, usize> = e
        .momentos
        .iter()
        .enumerate()
        .map(|(i, m)| (m.id.as_str(), i))
        .collect();
    let dias = anios.iter().flat_map(|a| &a.meses).flat_map(|m| &m.dias);
    let cajas = piezas.iter().filter_map(|p| match p {
        dis::PiezaArchivo::Dia { minis, .. } => Some(minis),
        _ => None,
    });
    let mut minis = Vec::new();
    for (d, rs) in dias.zip(cajas) {
        for (m, r) in d.momentos.iter().zip(rs) {
            if let Some(&i) = indice.get(m.id.as_str()) {
                minis.push((i, *r));
            }
        }
    }
    let c = Rc::new(CacheArchivo {
        clave,
        piezas,
        alto,
        minis,
    });
    *e.cache_archivo.borrow_mut() = Some(c.clone());
    c
}

/// Un tono pastel de los de los estados (`emoticonos::TONOS`) para un
/// momento sin foto: siempre el mismo para el mismo momento.
pub(super) fn color_suave(id: &str) -> Color {
    hex(pixpin_timeline::emoticonos::tono_de(id))
}

pub(super) fn pintar(e: &mut Estado, p: &Pintor, area: RectF, s: f32, textos: &Catalogo) {
    let d = disposicion(e, area.ancho, s);
    if d.piezas.is_empty() {
        vacio(p, area, &textos.t("timeline-momentos-vacio"), s);
        return;
    }
    e.alto_contenido = d.alto;
    let tope = (d.alto - area.alto).max(0.0);
    e.scroll = e.scroll.clamp(0.0, tope);
    let y0 = area.y - e.scroll;
    let raton = e.botones.raton;
    let mut zonas: Vec<(RectF, Accion)> = Vec::new();
    let m = dis::MARGEN * s;
    p.con_recorte(area, |p| {
        for pieza in &d.piezas {
            match pieza {
                dis::PiezaArchivo::Anio { anio, y } => {
                    let y = y0 + y;
                    if y > area.y + area.alto || y + 70.0 * s < area.y {
                        continue;
                    }
                    ui::negrita(p, &anio.to_string(), m, y, 40.0 * s, 300.0 * s, TEXTO_V);
                }
                dis::PiezaArchivo::Mes { mes, y } => {
                    let y = y0 + y;
                    if y > area.y + area.alto || y + 34.0 * s < area.y {
                        continue;
                    }
                    let nombre = dias::solo_el_mes(mes.mes, e.ingles);
                    p.texto(nombre, m, y + 6.0 * s, 14.0 * s, GRIS);
                }
                dis::PiezaArchivo::Dia {
                    dia,
                    numero,
                    minis: cajas,
                } => {
                    let numero = RectF {
                        y: y0 + numero.y,
                        ..*numero
                    };
                    let fondo = cajas
                        .last()
                        .map_or(numero.y + numero.alto, |r| y0 + r.y + r.alto);
                    if numero.y > area.y + area.alto || fondo < area.y {
                        continue;
                    }
                    let encima = dentro(numero, raton);
                    let color = if encima { ACENTO } else { TEXTO_V };
                    ui::negrita(
                        p,
                        &dia.dia.to_string(),
                        numero.x,
                        numero.y - 4.0 * s,
                        30.0 * s,
                        numero.ancho,
                        color,
                    );
                    p.texto(
                        pixpin_timeline::dias::dia_corto(*dia, e.ingles),
                        numero.x + 1.0 * s,
                        numero.y + 34.0 * s,
                        13.0 * s,
                        GRIS,
                    );
                    zonas.push((numero, Accion::AbrirDia(dia.numero())));
                }
                dis::PiezaArchivo::Fin { y } => {
                    let y = y0 + y + 20.0 * s;
                    let cx = area.ancho / 2.0;
                    p.linea((cx - 60.0 * s, y), (cx - 10.0 * s, y), 1.0 * s, LINEA);
                    p.linea((cx + 10.0 * s, y), (cx + 60.0 * s, y), 1.0 * s, LINEA);
                    p.circulo((cx, y), 2.5 * s, LINEA);
                }
            }
        }
        for (i, r) in &d.minis {
            let r = RectF { y: y0 + r.y, ..*r };
            if r.y > area.y + area.alto || r.y + r.alto < area.y {
                continue;
            }
            let encima = dentro(r, raton) && dentro(area, raton);
            let mo = &e.momentos[*i];
            mini(e, p, mo, r, encima, s);
            zonas.push((r, Accion::AbrirDetalle(clave(&mo.id))));
        }
    });
    for (r, a) in zonas {
        zona_en(&mut e.botones, area, r, a);
    }
}

/// Una miniatura: la primera foto, o el titulo sobre un tono pastel.
fn mini(e: &Estado, p: &Pintor, mo: &Momento, r: RectF, encima: bool, s: f32) {
    let radio = 6.0 * s;
    if let Some(f) = mo.fotos.first() {
        let ruta = e.almacen.ruta(f);
        p.rellenar_redondeado(r, radio, hex(0x2C2C2E));
        if let Some((b, w, h)) = e.minis.ya(&ruta) {
            if p.empujar_recorte_redondeado(r, radio) {
                crate::miniaturas::pintar_recortado(p, b, r, w, h);
                p.soltar_recorte_redondeado();
            } else {
                crate::miniaturas::pintar_recortado(p, b, r, w, h);
            }
        } else {
            p.icono(&mi::IMAGE, centrar(r, 28.0 * s), GRIS);
        }
        if mo.fotos.len() > 1 {
            // Cuantas lleva, en la esquina: la historia las pasa.
            let t = mo.fotos.len().to_string();
            let (tw, th) = p.medir_texto(&t, 11.5 * s);
            let c = RectF {
                x: r.x + r.ancho - tw - 16.0 * s,
                y: r.y + 6.0 * s,
                ancho: tw + 10.0 * s,
                alto: th + 4.0 * s,
            };
            p.rellenar_redondeado(c, c.alto / 2.0, Color { a: 0.6, ..Color::NEGRO });
            p.texto(&t, c.x + 5.0 * s, c.y + 2.0 * s, 11.5 * s, TEXTO_V);
        }
        if mo.audio.is_some() {
            let c = (r.x + r.ancho - 16.0 * s, r.y + r.alto - 16.0 * s);
            p.circulo(c, 11.0 * s, Color { a: 0.6, ..Color::NEGRO });
            p.icono(&mi::MIC, centrar_en(c, 14.0 * s), TEXTO_V);
        }
    } else {
        p.rellenar_redondeado(r, radio, color_suave(&mo.id));
        let pad = 10.0 * s;
        let caja = RectF {
            x: r.x + pad,
            y: r.y + pad,
            ancho: r.ancho - 2.0 * pad,
            // Cuatro renglones como mucho: lo demas se corta.
            alto: (4.0 * 19.0 * s).min(r.alto - 2.0 * pad),
        };
        p.con_recorte(caja, |p| {
            ui::negrita(p, &mo.titulo, caja.x, caja.y, 14.0 * s, caja.ancho, hex(0x1C1C1E));
        });
        if mo.audio.is_some() {
            p.icono(
                &mi::MIC,
                RectF {
                    x: r.x + r.ancho - pad - 16.0 * s,
                    y: r.y + r.alto - pad - 16.0 * s,
                    ancho: 16.0 * s,
                    alto: 16.0 * s,
                },
                hex(0x3A3A3D),
            );
        }
    }
    if mo.leccion.is_some() {
        marca_de_leccion(p, e, mo, (r.x + 17.0 * s, r.y + r.alto - 17.0 * s), s);
    }
    if encima {
        p.rellenar_redondeado(r, radio, blanco(0.08));
    }
}

/// La gravedad de la leccion de un momento, si es leccion y ya se leyo.
fn gravedad_de_leccion(e: &Estado, mo: &Momento) -> Option<i64> {
    let id = mo.leccion.as_ref()?;
    e.lecciones
        .iter()
        .find(|t| &t.entrada.leccion.id == id)
        .map(|t| t.entrada.leccion.gravedad)
}

/// El emoticono de la marca de un momento que es leccion: el de su gravedad
/// (😌 😟 😡), o 💡 si aun no se leyo la leccion.
pub(super) fn emoticono_de_leccion(e: &Estado, mo: &Momento) -> Option<&'static str> {
    mo.leccion.as_ref()?;
    Some(gravedad_de_leccion(e, mo).map_or("💡", |g| super::super::tarjetas::gravedad(g).emoticono))
}

/// **La marca de «es una leccion»**, la misma en Hoy, Momentos y la
/// historia: el emoticono de su gravedad en color sobre un circulo oscuro
/// con un aro de ese color.
pub(super) fn marca_de_leccion(p: &Pintor, e: &Estado, mo: &Momento, c: (f32, f32), s: f32) {
    let Some(emo) = emoticono_de_leccion(e, mo) else {
        return;
    };
    let color = gravedad_de_leccion(e, mo).map_or(ui::v2::AMARILLO, |g| super::super::tarjetas::gravedad(g).color);
    p.circulo(c, 13.0 * s, Color { a: 0.85, ..Color::NEGRO });
    p.anillo(c, 13.0 * s, 1.5 * s, color);
    let tam = 14.0 * s;
    let (tw, th) = p.medir_texto(emo, tam);
    p.texto_color(emo, c.0 - tw / 2.0, c.1 - th / 2.0, tam, TEXTO_V);
}

fn centrar(r: RectF, lado: f32) -> RectF {
    centrar_en((r.x + r.ancho / 2.0, r.y + r.alto / 2.0), lado)
}

fn centrar_en(c: (f32, f32), lado: f32) -> RectF {
    RectF {
        x: c.0 - lado / 2.0,
        y: c.1 - lado / 2.0,
        ancho: lado,
        alto: lado,
    }
}

/// El aviso de «aqui no hay nada», centrado.
pub(super) fn vacio(p: &Pintor, area: RectF, t: &str, s: f32) {
    let ancho = (area.ancho - 96.0 * s).max(100.0);
    let (tw, th) = p.medir_texto_ajustado(t, 15.0 * s, ancho);
    p.icono(
        &mi::TIMELINE,
        RectF {
            x: area.x + (area.ancho - 48.0 * s) / 2.0,
            y: area.y + area.alto / 2.0 - th - 64.0 * s,
            ancho: 48.0 * s,
            alto: 48.0 * s,
        },
        blanco(0.25),
    );
    p.texto_ajustado(
        t,
        area.x + (area.ancho - tw) / 2.0,
        area.y + area.alto / 2.0 - th,
        15.0 * s,
        tw + 2.0,
        GRIS,
    );
}
