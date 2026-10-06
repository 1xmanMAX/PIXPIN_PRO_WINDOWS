//! La pestana **«Lecciones»** del timeline y el boton «💡 Leccion» de los
//! momentos (5-oct-2026).
//!
//! El usuario: «se me ocurre que timeline y lecciones aprendidas las juntes:
//! que cada tarjeta de timeline se pueda poner como leccion aprendida
//! facilmente con solo un boton, se quede marcado como leccion, pero todo
//! se muestre en el mismo lugar». Y de las lecciones: «quita la barra
//! lateral; emoticones y color a la vez; lo de "me volvio a pasar" esta
//! bien, mantenlo; que este en formato de tarjetas».
//!
//! Las lecciones siguen viviendo donde siempre (`lecciones::almacen`, un
//! archivo por leccion y su mensaje en el chat): asi viajan al movil y
//! entran en el repaso. Aqui solo se leen, se pintan en tarjetas y se crean
//! desde un momento, que guarda el id de la suya.
//!
//! Desmarcar un momento NO borra su leccion: solo la desvincula. Es lo mas
//! seguro (borrar seria perder lo escrito despues en la ficha, o en el
//! movil) y el aviso lo dice.

use std::collections::HashMap;

use super::*;
use crate::lecciones::almacen::{self as lal, Donde, Entrada};
use crate::timeline::tarjetas;
use pixpin_lecciones::etiquetador;
use pixpin_lecciones::{Leccion, Repaso};
use pixpin_proyecto::cuaderno::{self, Clase as ClaseMensaje, Mensaje};

/// Una leccion con sus fotos y su nota de voz ya buscadas en el chat.
#[derive(Debug, Clone)]
pub(super) struct TarjetaLeccion {
    pub entrada: Entrada,
    pub fotos: Vec<PathBuf>,
    pub voz: Option<(PathBuf, i64)>,
}

/// Todas las lecciones, con sus adjuntos. Los adjuntos son mensajes del
/// chat que responden a la leccion: se busca su fichero como lo busca el
/// chat (lo que aun no llego sincronizando no sale).
pub(super) fn cargar(raiz: &std::path::Path) -> Vec<TarjetaLeccion> {
    let mut cuadernos: HashMap<String, Vec<Mensaje>> = HashMap::new();
    lal::listar(raiz)
        .into_iter()
        .map(|entrada| {
            let mut fotos = Vec::new();
            let mut voz = None;
            if !entrada.leccion.adjuntos.is_empty() {
                let mensajes = cuadernos.entry(entrada.ficha.clone()).or_insert_with(|| {
                    cuaderno::Cuaderno::leer_de(&pixpin_proyecto::almacen::carpeta(
                        raiz,
                        &entrada.ficha,
                    ))
                    .map(|c| c.mensajes)
                    .unwrap_or_default()
                });
                for id in &entrada.leccion.adjuntos {
                    let Some(m) = mensajes.iter().find(|m| m.id == *id) else {
                        continue;
                    };
                    let Some(ruta) = m
                        .ruta
                        .as_deref()
                        .and_then(|r| pixpin_proyecto::vista::ruta_real(raiz, &entrada.ficha, r))
                        .filter(|r| r.is_file())
                    else {
                        continue;
                    };
                    match m.clase {
                        Some(ClaseMensaje::Imagen) => fotos.push(ruta),
                        Some(ClaseMensaje::Voz) if voz.is_none() => {
                            voz = Some((ruta, m.duracion_ms))
                        }
                        _ => {}
                    }
                }
            }
            TarjetaLeccion {
                entrada,
                fotos,
                voz,
            }
        })
        .collect()
}

/// Las tarjetas que se ven (indices en `Estado::lecciones`): todas, o las
/// que casan con lo buscado y con el filtro de gravedad, en su orden.
pub(super) fn visibles(e: &Estado) -> Vec<usize> {
    let todas: Vec<&Leccion> = e.lecciones.iter().map(|t| &t.entrada.leccion).collect();
    tarjetas::filtrar(
        &todas,
        &e.busqueda.texto,
        e.filtro_gravedad,
        e.orden_lecciones,
    )
}

/// Cuantas lecciones casan con lo buscado (para la franja de la busqueda de
/// momentos). Cero sin busqueda.
pub(super) fn cuantas_buscadas(e: &Estado) -> usize {
    if e.busqueda.texto.trim().is_empty() {
        return 0;
    }
    let todas: Vec<&Leccion> = e.lecciones.iter().map(|t| &t.entrada.leccion).collect();
    tarjetas::buscar(&todas, &e.busqueda.texto).len()
}

/// Las portadas que se ven, con la `y` (desde lo alto del contenido) de su
/// tarjeta: el bucle carga solo las cercanas.
pub(super) fn portadas(e: &Estado, w: f32, s: f32) -> Vec<(f32, PathBuf)> {
    let v = visibles(e);
    let (cajas, _) = tarjetas::rejilla(v.len(), w, s);
    v.iter()
        .zip(cajas)
        .filter_map(|(&k, r)| e.lecciones[k].fotos.first().map(|f| (r.y, f.clone())))
        .collect()
}

// ------------------------------------------------------------ acciones

/// Que hacer al pulsar la bombilla de un momento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AlMarcar {
    /// Era leccion: se desvincula (la leccion se queda).
    Desmarcar,
    /// Tuvo una leccion que sigue existiendo: se vuelve a enlazar esa.
    Reusar(String),
    /// Una nueva.
    Crear,
}

/// Lo que toca hacer con `m`, sabiendo que lecciones existen. Pura para
/// probarla: marcar, desmarcar y marcar no debe dejar dos lecciones.
pub(super) fn al_marcar(m: &Momento, existe: &dyn Fn(&str) -> bool) -> AlMarcar {
    if m.leccion.is_some() {
        return AlMarcar::Desmarcar;
    }
    match &m.leccion_previa {
        Some(id) if existe(id) => AlMarcar::Reusar(id.clone()),
        _ => AlMarcar::Crear,
    }
}

/// **El boton «💡 Leccion»** de un momento: lo marca creando su leccion (o
/// reenlazando la que ya tuvo), o lo desmarca (sin borrar la leccion).
pub(super) fn alternar(e: &mut Estado, i: usize, textos: &Catalogo) {
    let Some(m) = e.momentos.get(i).cloned() else {
        return;
    };
    let existe = |id: &str| e.lecciones.iter().any(|t| t.entrada.leccion.id == id);
    match al_marcar(&m, &existe) {
        AlMarcar::Desmarcar => {
            let x = &mut e.momentos[i];
            x.leccion_previa = x.leccion.take();
            e.guardar_todos(textos);
            e.decir(textos.t("timeline-leccion-quitada"));
        }
        AlMarcar::Reusar(id) => {
            let x = &mut e.momentos[i];
            x.leccion = Some(id);
            x.leccion_previa = None;
            e.guardar_todos(textos);
            e.decir(textos.t("timeline-leccion-hecha"));
        }
        AlMarcar::Crear => match crear(e, &m) {
            Ok(id) => {
                if let Some(x) = e.momentos.iter_mut().find(|x| x.id == m.id) {
                    x.leccion = Some(id);
                    x.leccion_previa = None;
                }
                e.guardar_todos(textos);
                e.releer_lecciones();
                e.decir(textos.t("timeline-leccion-hecha"));
            }
            Err(err) => {
                tracing::warn!(?err, "timeline: no se pudo hacer la leccion");
                e.decir(textos.t("timeline-leccion-fallo"));
            }
        },
    }
}

/// La leccion de un momento, en el almacen de siempre: en «Mensajes
/// guardados», con sus fotos y su audio. Devuelve su id.
fn crear(e: &Estado, m: &Momento) -> std::io::Result<String> {
    let ahora = Estado::ahora();
    let ficha = pixpin_proyecto::almacen::asegurar_guardados(&e.raiz, ahora, &e.aparato)?.id;
    let texto = format!("{}\n{}", m.titulo, m.descripcion);
    let propuesta = etiquetador::proponer(&texto, &etiquetador::Aprendido::default(), &[]);
    // La gravedad que proponga el etiquetador; si no dice nada, importante
    // (si se marco como leccion, algo enseno).
    let gravedad = etiquetador::proponer_gravedad(&texto, propuesta.tipo.as_deref()).max(2);
    let l = Leccion {
        titulo: m.titulo.clone(),
        que_paso: m.descripcion.clone(),
        tipo: propuesta
            .tipo
            .clone()
            .unwrap_or_else(|| pixpin_lecciones::leccion::TIPO_LECCION.to_string()),
        area: propuesta.area.clone().unwrap_or_default(),
        gravedad,
        etiquetas_auto: propuesta.etiquetas.clone(),
        causas: propuesta.causas.clone(),
        ..Leccion::nueva(&pixpin_lecciones::leccion::nuevo_id(ahora), ahora, "")
    };
    let mut fotos: Vec<lal::Foto> = Vec::new();
    for f in &m.fotos {
        match std::fs::read(e.almacen.ruta(f)) {
            Ok(b) => fotos.push((f.clone(), b)),
            Err(err) => tracing::warn!(?err, foto = %f, "timeline: foto sin pasar a la leccion"),
        }
    }
    let voz = m.audio.as_ref().and_then(|a| {
        std::fs::read(e.almacen.ruta(a))
            .ok()
            .map(|b| (a.clone(), b, m.duracion_ms))
    });
    lal::guardar_con_adjuntos(
        &e.raiz,
        &l,
        &Donde::Nueva { ficha },
        &e.aparato,
        &fotos,
        voz.as_ref(),
    )?;
    Ok(l.id)
}

/// **«Me volvio a pasar»**: como en la lista de siempre.
pub(super) fn otra_vez(e: &mut Estado, k: usize, textos: &Catalogo) {
    let Some(t) = e.lecciones.get(k) else {
        return;
    };
    let nueva = Repaso::repetida(&t.entrada.leccion, Estado::ahora());
    let veces = nueva.veces_que_paso();
    guardar(e, k, &nueva, textos);
    let mut a = fluent_bundle::FluentArgs::new();
    a.set("n", veces as i64);
    e.decir(textos.t_args("timeline-otra-vez-hecho", &a));
}

/// Un clic en la gravedad le da la vuelta.
pub(super) fn cambiar_gravedad(e: &mut Estado, k: usize, textos: &Catalogo) {
    let Some(t) = e.lecciones.get(k) else {
        return;
    };
    let l = &t.entrada.leccion;
    let nueva = Leccion {
        gravedad: tarjetas::siguiente_gravedad(l.gravedad),
        tocada: Estado::ahora(),
        ..l.clone()
    };
    guardar(e, k, &nueva, textos);
}

fn guardar(e: &mut Estado, k: usize, l: &Leccion, textos: &Catalogo) {
    let donde = Donde::de(&e.lecciones[k].entrada);
    match lal::guardar(&e.raiz, l, &donde, &e.aparato) {
        Ok(_) => e.releer_lecciones(),
        Err(err) => {
            tracing::warn!(?err, "timeline: no se pudo guardar la leccion");
            e.decir(textos.t("timeline-leccion-fallo"));
        }
    }
}

// ------------------------------------------------------------- pintado

/// La fila de arriba: chips de filtro por gravedad («Todas · 😡 2 · 😟 2 ·
/// 😌 1») y, a la derecha, el orden («Recientes / Mas repetidas»).
fn filtros(e: &Estado, p: &Pintor, area: RectF, y: f32, s: f32, textos: &Catalogo) -> Vec<(RectF, Accion)> {
    let mut zonas = Vec::new();
    let todas: Vec<&Leccion> = e.lecciones.iter().map(|t| &t.entrada.leccion).collect();
    let n = tarjetas::cuantas_por_gravedad(&todas);
    let chips: Vec<(String, Option<i64>)> = std::iter::once((textos.t("timeline-filtro-todas"), None))
        .chain(
            [(3i64, n[2]), (2, n[1]), (1, n[0])]
                .into_iter()
                .filter(|(_, c)| *c > 0)
                .map(|(g, c)| (format!("{} {c}", tarjetas::gravedad(g).emoticono), Some(g))),
        )
        .collect();
    let tam = 13.5 * s;
    let alto = 32.0 * s;
    let mut x = area.x + tarjetas::MARGEN * s;
    for (t, g) in chips {
        let w = super::historia::medir_trozos(p, &t, tam, false) + 24.0 * s;
        let r = RectF {
            x,
            y,
            ancho: w,
            alto,
        };
        let activo = e.filtro_gravedad == g;
        let fondo = if activo {
            ACENTO
        } else if dentro(r, e.botones.raton) {
            hex(0x3A3A3D)
        } else {
            hex(0x2C2C2E)
        };
        p.rellenar_redondeado(r, alto / 2.0, fondo);
        super::historia::pintar_trozos(
            p,
            &t,
            r.x + 12.0 * s,
            r.y + (alto - tam * 1.33) / 2.0,
            tam,
            false,
            TEXTO_V,
            false,
        );
        zonas.push((r, Accion::FiltroGravedad(g)));
        x += w + 8.0 * s;
    }
    // El orden, a la derecha, como un pequeno control segmentado.
    let rot = [
        (textos.t("timeline-orden-recientes"), tarjetas::Orden::Recientes),
        (textos.t("timeline-orden-repetidas"), tarjetas::Orden::MasRepetidas),
    ];
    let anchos: Vec<f32> = rot
        .iter()
        .map(|(t, _)| p.medir_texto(t, tam).0 + 24.0 * s)
        .collect();
    let total: f32 = anchos.iter().sum::<f32>() + 6.0 * s;
    let caja = RectF {
        x: area.x + area.ancho - tarjetas::MARGEN * s - total,
        y,
        ancho: total,
        alto,
    };
    if caja.x > x {
        p.rellenar_redondeado(caja, alto / 2.0, hex(0x2C2C2E));
        let mut ox = caja.x + 3.0 * s;
        for ((t, o), w) in rot.iter().zip(anchos) {
            let r = RectF {
                x: ox,
                y: y + 3.0 * s,
                ancho: w,
                alto: alto - 6.0 * s,
            };
            if e.orden_lecciones == *o {
                p.rellenar_redondeado(r, r.alto / 2.0, hex(0x48484A));
            }
            let (tw, th) = p.medir_texto(t, tam);
            p.texto(t, r.x + (w - tw) / 2.0, r.y + (r.alto - th) / 2.0, tam, TEXTO_V);
            zonas.push((r, Accion::OrdenLecciones(*o == tarjetas::Orden::MasRepetidas)));
            ox += w;
        }
    }
    zonas
}

pub(super) fn pintar(e: &mut Estado, p: &Pintor, area: RectF, s: f32, textos: &Catalogo) {
    let v = visibles(e);
    let (cajas, alto) = tarjetas::rejilla(v.len(), area.ancho, s);
    e.alto_contenido = alto;
    let tope = (alto - area.alto).max(0.0);
    e.scroll = e.scroll.clamp(0.0, tope);
    let y0 = area.y - e.scroll;
    let raton = e.botones.raton;
    let rot_otra = textos.t("timeline-otra-vez-pildora");
    let ancho_otra = ui::ancho_de_boton(p, false, &rot_otra, None, s);
    let mut zonas: Vec<(RectF, Accion)> = Vec::new();
    if !e.lecciones.is_empty() {
        p.con_recorte(area, |p| {
            zonas = filtros(e, p, area, y0 + 12.0 * s, s, textos);
        });
    }
    if v.is_empty() {
        let clave = if e.lecciones.is_empty() {
            "timeline-lecciones-vacio"
        } else {
            "timeline-lecciones-sin-resultados"
        };
        archivo::vacio(p, area, &textos.t(clave), s);
        for (r, a) in zonas {
            zona_en(&mut e.botones, area, r, a);
        }
        return;
    }
    p.con_recorte(area, |p| {
        for (&k, r) in v.iter().zip(&cajas) {
            let celda = RectF { y: y0 + r.y, ..*r };
            if celda.y > area.y + area.alto || celda.y + celda.alto + 10.0 * s < area.y {
                continue;
            }
            let t = &e.lecciones[k];
            let pt = tarjetas::partes(celda, t.fotos.len(), ancho_otra, s);
            let encima = dentro(pt.tarjeta, raton) && dentro(area, raton);
            tarjeta(e, p, t, &pt, encima, &rot_otra, raton, s, textos);
            // La tarjeta abre su historia; sus botones, apuntados despues,
            // le ganan.
            let c = clave(&t.entrada.leccion.id);
            zonas.push((pt.tarjeta, Accion::AbrirLeccion(c)));
            zonas.push((pt.otra_vez, Accion::OtraVez(c)));
            zonas.push((pt.compartir, Accion::MenuCompartirLeccion(c)));
        }
    });
    for (r, a) in zonas {
        zona_en(&mut e.botones, area, r, a);
    }
}

#[allow(clippy::too_many_arguments)] // estado, pintor, tarjeta, partes, raton, escala y textos
fn tarjeta(
    e: &Estado,
    p: &Pintor,
    t: &TarjetaLeccion,
    pt: &tarjetas::Partes,
    encima: bool,
    rot_otra: &str,
    raton: (f32, f32),
    s: f32,
    textos: &Catalogo,
) {
    let l = &t.entrada.leccion;
    let gr = tarjetas::gravedad(l.gravedad);
    let radio = 14.0 * s;
    // Varias fotos: los bordes de detras, como un monton (dentro de la
    // celda: la tarjeta les deja sitio).
    for (n, c) in pt.pila.iter().enumerate() {
        let color = if n + 1 == pt.pila.len() {
            hex(0x48484A)
        } else {
            hex(0x3A3A3D)
        };
        p.rellenar_redondeado(*c, radio, color);
    }
    let caja = pt.tarjeta;
    p.rellenar_redondeado(caja, radio, if encima { hex(0x313134) } else { TARJETA });
    // La franja de arriba: la foto o el color de su gravedad con su cara.
    if p.empujar_recorte_redondeado(caja, radio) {
        match t.fotos.first().and_then(|f| e.minis.ya(f)) {
            Some((b, w, h)) => crate::miniaturas::pintar_recortado(p, b, pt.banda, w, h),
            None if !t.fotos.is_empty() => p.rellenar(pt.banda, hex(0x3A3A3D)),
            None => {
                p.rellenar(pt.banda, Color { a: 0.16, ..gr.color });
                let tam = 32.0 * s;
                let (tw, th) = p.medir_texto(gr.emoticono, tam);
                // En monocromo y del color de su gravedad: «grave rojo pero
                // el emoticon pintado de rojo».
                p.texto(
                    gr.emoticono,
                    pt.banda.x + pt.banda.ancho - tw - 18.0 * s,
                    pt.banda.y + (pt.banda.alto - th) / 2.0,
                    tam,
                    gr.color,
                );
            }
        }
        p.soltar_recorte_redondeado();
    }
    // La gravedad (sobre la foto, con un fondo oscuro debajo).
    let fondo = (!t.fotos.is_empty()).then_some(Color {
        a: 0.72,
        ..Color::NEGRO
    });
    super::compartir::chapa_de_gravedad(p, l.gravedad, pt.gravedad.x, pt.gravedad.y, fondo, s, textos);
    // El texto: el titulo (dos renglones) y «la proxima vez» si la hay; si
    // no, lo que paso.
    let tx = pt.texto;
    let tam_t = 16.0 * s;
    let renglones = dis::cortar_renglones(
        dis::renglones(&l.titulo, tx.ancho, &|x| {
            super::historia::medir_trozos(p, x, tam_t, true)
        }),
        2,
    );
    let lh = tam_t * 1.33;
    for (n, r) in renglones.iter().enumerate() {
        super::historia::pintar_trozos(p, r, tx.x, tx.y + n as f32 * lh, tam_t, true, TEXTO_V, false);
    }
    let th = renglones.len() as f32 * lh;
    let resto = RectF {
        y: tx.y + th + 6.0 * s,
        alto: (tx.alto - th - 6.0 * s).max(0.0),
        ..tx
    };
    let (texto, color) = if !l.proxima.trim().is_empty() {
        (
            format!("{}: {}", textos.t("timeline-leccion-proxima"), l.proxima.trim()),
            ui::v2::VERDE,
        )
    } else {
        (l.que_paso.trim().to_string(), CUERPO)
    };
    if resto.alto > 10.0 * s && !texto.is_empty() {
        p.con_recorte(resto, |p| {
            p.texto_ajustado(&texto, resto.x, resto.y, 13.5 * s, resto.ancho, color)
        });
    }
    // Abajo: «↻ +1», cuantas veces y la nota de voz.
    let b = pt.otra_vez;
    ui::boton_v2(
        p,
        &mut Botones::default(),
        b,
        Accion::Fondo,
        None,
        rot_otra,
        None,
        Some(if dentro(b, raton) {
            hex(0x3A3A3D)
        } else {
            hex(0x2F2F32)
        }),
        CUERPO,
        s,
    );
    let mut x = b.x + b.ancho + 10.0 * s;
    let veces = l.veces_que_paso();
    if veces > 1 {
        let t = format!("{veces}×");
        let (tw, tht) = ui::medir_negrita(p, &t, 15.0 * s, 80.0 * s);
        ui::negrita(p, &t, x, b.y + (b.alto - tht) / 2.0, 15.0 * s, tw + 2.0, gr.color);
        x += tw + 12.0 * s;
    }
    if t.voz.is_some() {
        p.icono(
            &mi::MIC,
            RectF {
                x,
                y: b.y + (b.alto - 18.0 * s) / 2.0,
                ancho: 18.0 * s,
                alto: 18.0 * s,
            },
            ui::v2::CIAN,
        );
    }
    let c = pt.compartir;
    if dentro(c, raton) {
        p.rellenar_redondeado(c, c.alto / 2.0, blanco(0.1));
    }
    p.icono(&mi::IOS_SHARE, encoger(c, 9.0 * s), if encima { CUERPO } else { GRIS });
}

/// En la busqueda de momentos, si lo buscado esta tambien en lecciones: una
/// franja arriba que lo dice y lleva a ellas.
pub(super) fn franja_buscadas(
    e: &mut Estado,
    p: &Pintor,
    zona: RectF,
    n: usize,
    s: f32,
    textos: &Catalogo,
) {
    let m = MARGEN * s;
    let caja = RectF {
        x: m,
        y: zona.y + 8.0 * s,
        ancho: zona.ancho - 2.0 * m,
        alto: zona.alto - 12.0 * s,
    };
    p.rellenar(zona, FONDO_V);
    let encima = dentro(caja, e.botones.raton);
    p.rellenar_redondeado(
        caja,
        10.0 * s,
        Color {
            a: if encima { 0.22 } else { 0.14 },
            ..ui::v2::AMARILLO
        },
    );
    p.icono(
        &mi::LIGHTBULB,
        RectF {
            x: caja.x + 12.0 * s,
            y: caja.y + (caja.alto - 20.0 * s) / 2.0,
            ancho: 20.0 * s,
            alto: 20.0 * s,
        },
        ui::v2::AMARILLO,
    );
    let t = con_n(textos, "timeline-tambien-lecciones", n);
    let (_, th) = p.medir_texto(&t, 14.0 * s);
    p.texto_linea(
        &t,
        caja.x + 42.0 * s,
        caja.y + (caja.alto - th) / 2.0,
        14.0 * s,
        caja.ancho - 60.0 * s,
        TEXTO_V,
    );
    e.botones.zona(caja, Accion::VerLeccionesBuscadas);
}
