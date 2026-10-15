//! La pestana **«Estado»**: como la primera captura de WeChat que mando el
//! usuario. Una tarjeta por mes; a la izquierda el numero del mes en
//! grande, su nombre corto y lo que mas se repitio («🍕 ×6»); a la derecha
//! sus dias, del mas reciente al 1. Un dia con algo lleva sus emoticonos
//! como los estados de WeChat: hasta tres circulos pastel apilados
//! (`emoticonos::pila_de_estados`, los mismos de las tareas) y «+N»; sin
//! emoticonos, la primera foto en un circulo, o un circulo pastel con la
//! inicial. Un dia sin nada es solo un puntito, y una fila entera sin nada
//! es bajita.
//!
//! Sustituye al calendario: clic en un dia lo abre y Ctrl+clic lo elige
//! para exportar varios juntos.
//!
//! Donde va cada cosa lo dice `disposicion::estado`, lo mismo para pintar
//! que para el clic.

use std::collections::BTreeMap;

use super::*;
use pixpin_timeline::archivo as arch;
use pixpin_timeline::emoticonos::{self as emo, pila_de_estados};
use pixpin_timeline::resumen::{LoMas, lo_mas_con_cuenta};

const TARJETA_MES: Color = hex(0x262628);
/// Lo que reserva abajo la barra flotante de dias elegidos.
const BARRA_ELEGIDOS: f32 = 76.0;

/// Lo que va en los circulos de un dia.
enum Circulo {
    /// Sus emoticonos distintos, en orden.
    Emoticonos(Vec<String>),
    Foto(PathBuf),
    /// Ni emoticono ni foto: la inicial de su primer titulo.
    Inicial(String),
}

/// Los momentos de cada dia (indices en `Estado::momentos`, en orden).
fn por_dia(e: &Estado) -> BTreeMap<Dia, Vec<usize>> {
    let mut m: BTreeMap<Dia, Vec<usize>> = BTreeMap::new();
    for (i, x) in e.momentos.iter().enumerate() {
        m.entry(Dia::de_instante(x.cuando, e.desfase))
            .or_default()
            .push(i);
    }
    m
}

fn circulo(e: &Estado, idx: &[usize]) -> Circulo {
    let mut emos: Vec<String> = Vec::new();
    for &i in idx {
        let m = &e.momentos[i];
        for t in [&m.titulo, &m.descripcion] {
            for x in emo::emoticonos(t) {
                if !emos.iter().any(|y| emo::clave(y) == emo::clave(x)) {
                    emos.push(x.to_string());
                }
            }
        }
    }
    if !emos.is_empty() {
        return Circulo::Emoticonos(emos);
    }
    for &i in idx {
        if let Some(f) = e.momentos[i].fotos.first() {
            return Circulo::Foto(e.almacen.ruta(f));
        }
    }
    let inicial = idx
        .first()
        .and_then(|&i| e.momentos[i].titulo.chars().find(|c| c.is_alphanumeric()))
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default();
    Circulo::Inicial(inicial)
}

/// Los meses y sus dias, al reves, con si tienen algo, como se ven.
fn meses(e: &Estado, pd: &BTreeMap<Dia, Vec<usize>>) -> Vec<(Mes, Vec<(Dia, bool)>)> {
    let hoy = e.hoy();
    arch::meses_del_estado(&e.momentos, e.desfase, hoy)
        .into_iter()
        .map(|m| {
            let dias = arch::dias_al_reves(m, hoy)
                .into_iter()
                .map(|d| (d, pd.contains_key(&d)))
                .collect();
            (m, dias)
        })
        .collect()
}

/// Lo que se deja libre al final: el alto de la barra de elegidos.
fn abajo(e: &Estado, s: f32) -> f32 {
    if e.elegidos.is_empty() {
        0.0
    } else {
        BARRA_ELEGIDOS * s
    }
}

/// Las fotos que salen en los circulos, con la `y` (desde lo alto del
/// contenido) de su circulo: el bucle carga solo las que caen cerca.
pub(super) fn fotos_de_los_circulos(e: &Estado, w: f32, s: f32) -> Vec<(f32, PathBuf)> {
    let pd = por_dia(e);
    let (tarjetas, _) = dis::estado(&meses(e, &pd), w, s, abajo(e, s));
    let mut v = Vec::new();
    for t in &tarjetas {
        for c in &t.celdas {
            if let Some(idx) = pd.get(&c.dia)
                && let Circulo::Foto(r) = circulo(e, idx)
            {
                v.push((c.centro.1, r));
            }
        }
    }
    v
}

fn oscurecer(c: Color, k: f32) -> Color {
    Color {
        r: c.r * (1.0 - k),
        g: c.g * (1.0 - k),
        b: c.b * (1.0 - k),
        a: c.a,
    }
}

/// La izquierda de una tarjeta: el numero del mes, su nombre corto (y el
/// ano si no es este) y lo que mas hubo, en una chapa «🍕 ×6».
fn columna_del_mes(p: &Pintor, e: &Estado, pd: &BTreeMap<Dia, Vec<usize>>, mes: Mes, caja: RectF, s: f32) {
    let x = caja.x + 18.0 * s;
    let mut y = caja.y + 12.0 * s;
    let num = mes.mes.to_string();
    let (_, nh) = ui::medir_negrita(p, &num, 44.0 * s, 100.0 * s);
    ui::negrita(p, &num, x, y - 4.0 * s, 44.0 * s, 100.0 * s, TEXTO_V);
    y += nh - 4.0 * s;
    let mut nombre = pixpin_timeline::estilo::mes_corto(mes.mes, e.ingles).to_string();
    if mes.anio != e.hoy().anio {
        nombre.push_str(&format!(" {}", mes.anio));
    }
    p.texto(&nombre, x + 2.0 * s, y, 13.5 * s, GRIS);
    y += 26.0 * s;
    let del_mes: Vec<&Momento> = pd
        .iter()
        .filter(|(d, _)| Mes::de(**d) == mes)
        .flat_map(|(_, v)| v.iter().map(|&i| &e.momentos[i]))
        .collect();
    let Some((lo, n)) = lo_mas_con_cuenta(&del_mes) else {
        return;
    };
    let txt = format!("{} ×{n}", lo.texto());
    let tam = if matches!(lo, LoMas::Emoticono(_)) {
        15.0 * s
    } else {
        12.5 * s
    };
    let tw = super::historia::medir_trozos(p, &txt, tam, false).min((dis::COLUMNA_MES - 30.0) * s);
    let chapa = RectF {
        x,
        y,
        ancho: tw + 20.0 * s,
        alto: 28.0 * s,
    };
    p.rellenar_redondeado(chapa, chapa.alto / 2.0, hex(0x323235));
    p.con_recorte(chapa, |p| {
        super::historia::pintar_trozos(
            p,
            &txt,
            chapa.x + 10.0 * s,
            chapa.y + (chapa.alto - tam * 1.33) / 2.0,
            tam,
            false,
            CUERPO,
            false,
        );
    });
}

/// Los circulos de un dia con algo, centrados en `centro`.
/// La pila no se sale de su celda (`ancho`): con varios emoticonos, los
/// circulos encogen lo justo.
fn circulos_del_dia(p: &Pintor, e: &Estado, idx: &[usize], centro: (f32, f32), r: f32, ancho_celda: f32, s: f32) {
    let d = 2.0 * r;
    match circulo(e, idx) {
        Circulo::Emoticonos(emos) => {
            // La pila, centrada (lo que asoma va a la derecha) y dentro de
            // la celda.
            let n = emos.len().min(emo::VISIBLES) as f32;
            let d = d.min((ancho_celda - 6.0 * s) / (1.0 + 0.55 * (n - 1.0)));
            let r = d / 2.0;
            let ancho = emo::ancho_de_pila(d, emos.len());
            let (circulos, mas) = pila_de_estados(centro.0 - ancho / 2.0, d, emos.len());
            for (k, (cx, sombra)) in circulos.iter().enumerate().rev() {
                let tono = oscurecer(hex(emo::tono_de(&emos[k])), *sombra);
                p.circulo((*cx, centro.1), r + 1.5 * s, TARJETA_MES);
                p.circulo((*cx, centro.1), r, tono);
                if k == 0 {
                    let tam = r * 0.95;
                    let (tw, th) = p.medir_texto(&emos[0], tam);
                    p.texto_color(&emos[0], cx - tw / 2.0, centro.1 - th / 2.0, tam, TEXTO_V);
                }
            }
            if mas > 0 {
                let t = format!("+{mas}");
                let (tw, th) = p.medir_texto(&t, 11.0 * s);
                let b = (centro.0 + ancho / 2.0 - tw / 2.0, centro.1 + r - th / 2.0);
                p.rellenar_redondeado(
                    RectF {
                        x: b.0 - 4.0 * s,
                        y: b.1 - 1.0 * s,
                        ancho: tw + 8.0 * s,
                        alto: th + 2.0 * s,
                    },
                    (th + 2.0 * s) / 2.0,
                    hex(0x48484A),
                );
                p.texto(&t, b.0, b.1, 11.0 * s, TEXTO_V);
            }
        }
        Circulo::Foto(ruta) => {
            let caja = RectF {
                x: centro.0 - r,
                y: centro.1 - r,
                ancho: d,
                alto: d,
            };
            p.circulo(centro, r, hex(0x3A3A3D));
            if let Some((b, w, h)) = e.minis.ya(&ruta)
                && p.empujar_recorte_redondeado(caja, r)
            {
                crate::miniaturas::pintar_recortado(p, b, caja, w, h);
                p.soltar_recorte_redondeado();
            }
        }
        Circulo::Inicial(ini) => {
            p.circulo(centro, r, hex(emo::tono_de(&ini)));
            let tam = r * 0.85;
            let (tw, th) = ui::medir_negrita(p, &ini, tam, 100.0 * s);
            ui::negrita(
                p,
                &ini,
                centro.0 - tw / 2.0,
                centro.1 - th / 2.0,
                tam,
                tw + 2.0,
                hex(0x1C1C1E),
            );
        }
    }
}

pub(super) fn pintar(e: &mut Estado, p: &Pintor, area: RectF, s: f32, textos: &Catalogo) {
    let pd = por_dia(e);
    let lista = meses(e, &pd);
    let (tarjetas, alto) = dis::estado(&lista, area.ancho, s, abajo(e, s));
    e.alto_contenido = alto;
    let tope = (alto - area.alto).max(0.0);
    e.scroll = e.scroll.clamp(0.0, tope);
    let y0 = area.y - e.scroll;
    let raton = e.botones.raton;
    let hoy = e.hoy();
    let mut zonas: Vec<(RectF, Accion)> = Vec::new();
    p.con_recorte(area, |p| {
        for t in &tarjetas {
            let caja = RectF {
                y: y0 + t.caja.y,
                ..t.caja
            };
            if caja.y > area.y + area.alto || caja.y + caja.alto < area.y {
                continue;
            }
            p.rellenar_redondeado(caja, 16.0 * s, TARJETA_MES);
            columna_del_mes(p, e, &pd, t.mes, caja, s);
            // Las iniciales de la semana, encima de sus columnas.
            let letras = dias::iniciales(e.ingles);
            for (k, r) in t.semana.iter().enumerate() {
                let (lw, lh) = p.medir_texto(letras[k], 12.0 * s);
                p.texto(
                    letras[k],
                    r.x + (r.ancho - lw) / 2.0,
                    y0 + r.y + (r.alto - lh) / 2.0,
                    12.0 * s,
                    GRIS,
                );
            }
            for c in &t.celdas {
                let celda = RectF {
                    y: y0 + c.caja.y,
                    ..c.caja
                };
                let centro = (c.centro.0, y0 + c.centro.1);
                if !c.con_algo {
                    // Sin nada: su numero, apagado (es un calendario).
                    let num = c.dia.dia.to_string();
                    let (nw, nh) = p.medir_texto(&num, 13.0 * s);
                    p.texto(
                        &num,
                        celda.x + (celda.ancho - nw) / 2.0,
                        // En una semana con algo, a la altura de los otros
                        // numeros; en una vacia, en medio.
                        if celda.alto > 40.0 * s {
                            celda.y + 3.0 * s
                        } else {
                            celda.y + (celda.alto - nh) / 2.0
                        },
                        13.0 * s,
                        blanco(0.28),
                    );
                    continue;
                }
                let Some(idx) = pd.get(&c.dia) else { continue };
                let num = c.dia.dia.to_string();
                let (nw, nh) = p.medir_texto(&num, 14.0 * s);
                let ny = celda.y + 2.0 * s;
                if c.dia == hoy {
                    // Hoy: el numero en una pildora azul llena.
                    let r = RectF {
                        x: celda.x + (celda.ancho - nw) / 2.0 - 8.0 * s,
                        y: ny - 1.0 * s,
                        ancho: nw + 16.0 * s,
                        alto: nh + 2.0 * s,
                    };
                    p.rellenar_redondeado(r, r.alto / 2.0, ACENTO);
                }
                p.texto(&num, celda.x + (celda.ancho - nw) / 2.0, ny, 14.0 * s, TEXTO_V);
                circulos_del_dia(p, e, idx, centro, c.radio, celda.ancho, s);
                let r = c.radio;
                if e.elegidos.contains(&c.dia) {
                    p.anillo(centro, r + 3.5 * s, 2.5 * s, ACENTO);
                    let k = (centro.0 + r * 0.72, centro.1 - r * 0.72);
                    p.circulo(k, 9.0 * s, ACENTO);
                    p.icono(
                        &mi::CHECK_CIRCLE,
                        RectF {
                            x: k.0 - 9.0 * s,
                            y: k.1 - 9.0 * s,
                            ancho: 18.0 * s,
                            alto: 18.0 * s,
                        },
                        ui::v2::BLANCO,
                    );
                } else if dentro(celda, raton) && dentro(area, raton) {
                    p.anillo(centro, r + 3.0 * s, 2.0 * s, blanco(0.4));
                }
                zonas.push((celda, Accion::DiaDelEstado(c.dia.numero())));
            }
        }
    });
    for (r, a) in zonas {
        zona_en(&mut e.botones, area, r, a);
    }
    if !e.elegidos.is_empty() {
        barra_de_elegidos(e, p, area, s, textos);
    }
}

/// Abajo, flotando, mientras haya dias elegidos: cuantos, quitar y exportar.
fn barra_de_elegidos(e: &mut Estado, p: &Pintor, area: RectF, s: f32, textos: &Catalogo) {
    let rot = con_n(textos, "timeline-cal-elegidos", e.elegidos.len());
    let r1 = textos.t("timeline-cal-quitar");
    let w1 = ui::ancho_de_boton(p, false, &r1, None, s);
    let r2 = textos.t("timeline-exportar");
    let w2 = ui::ancho_de_boton(p, true, &r2, None, s);
    let (tw, _) = p.medir_texto(&rot, 14.0 * s);
    let ancho = tw + w1 + w2 + 56.0 * s;
    let caja = RectF {
        x: (area.ancho - ancho) / 2.0,
        y: area.y + area.alto - 64.0 * s,
        ancho,
        alto: 52.0 * s,
    };
    p.rellenar_redondeado(caja, 14.0 * s, blanco(0.14));
    p.rellenar_redondeado(encoger(caja, 1.0 * s), 13.0 * s, hex(0x2C2C2E));
    e.botones.zona(caja, Accion::Fondo);
    let (_, th) = p.medir_texto(&rot, 14.0 * s);
    p.texto(
        &rot,
        caja.x + 18.0 * s,
        caja.y + (caja.alto - th) / 2.0,
        14.0 * s,
        TEXTO_V,
    );
    let b2 = RectF {
        x: caja.x + caja.ancho - 6.0 * s - w2,
        y: caja.y + 6.0 * s,
        ancho: w2,
        alto: BOTON * s,
    };
    let b1 = RectF {
        x: b2.x - 6.0 * s - w1,
        ..b2
    };
    ui::boton_v2(
        p,
        &mut e.botones,
        b1,
        Accion::QuitarEleccion,
        None,
        &r1,
        None,
        None,
        CUERPO,
        s,
    );
    ui::boton_v2(
        p,
        &mut e.botones,
        b2,
        Accion::Exportar,
        Some(&mi::IOS_SHARE),
        &r2,
        None,
        Some(ui::v2::AZUL),
        ui::v2::BLANCO,
        s,
    );
}
