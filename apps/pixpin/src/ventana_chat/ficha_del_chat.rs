//! **La ficha del chat**: la isla flotante que sale al pulsar el nombre en
//! la cabecera (25-oct-2026, el usuario: «su UI es mala. Mira Telegram: es
//! una isla flotante que muestra el chat, fotos, videos, archivos, enlaces…
//! muy bien presentado, y hacer click dentro no debe salir de esa zona»).
//!
//! - **Arriba**, el avatar del chat, su nombre y cuantas cosas tiene, con la
//!   lupa y el aspa.
//! - **Las pestanas**, un selector segmentado con la cuenta de cada una. Solo
//!   salen las que tienen algo (como en Telegram): un chat sin videos no
//!   ensena «Videos 0».
//! - **El contenido**: lo que se mira (fotos, videos, dibujos, hojas) en una
//!   rejilla de miniaturas cuadradas que pone mas columnas cuanto mas ancha
//!   es la ventana; lo que se lee (archivos, enlaces, voz) en filas.
//! - **Lo que se pulsa se ve DENTRO**: la previa de la cosa en grande, con
//!   las flechas de la anterior y la siguiente (o ← y →), «Abrir» y «Ver en
//!   el chat». La flecha de arriba (o Escape) vuelve a la rejilla. Un enlace
//!   se abre directamente en el navegador, y la isla se queda.
//!
//! Solo el aspa, Escape (desde la rejilla) o un clic FUERA la cierran.
//!
//! Pintar y repartir los clics salen de las mismas cuentas
//! (`pixpin_ui::info::Isla`, [`colocar`] y [`zona_en`]): cuando eran dos,
//! se veian botones que no respondian.

use pixpin_geom::{Punto, Rect};
use pixpin_proyecto::cuaderno::{Clase, Mensaje, Seccion};
use pixpin_render::icono::Icono;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_ui::chat::{self, Disposicion};
use pixpin_ui::info::{self as ui, Isla};

use super::{
    Abierto, Ojeada, Pinta, color_avatar, colores_de_caja, con_alfa, encoger,
    indices_de_la_seccion, iniciales, logo, mi, negritas, papel_de_vista, pintar_lienzo,
    pintar_ojeada_tabla, pintar_tarjeta_galeria, rf, ruta_del_mensaje, tarjetas,
};

/// En que orden salen las pestanas: primero lo que se mira, luego lo que se
/// lee, y «Todo» y el buzon al final.
pub(super) const ORDEN: [Seccion; 10] = [
    Seccion::Galeria,
    Seccion::Fotos,
    Seccion::Videos,
    Seccion::Archivos,
    Seccion::Enlaces,
    Seccion::Voz,
    Seccion::Dibujos,
    Seccion::Fijados,
    Seccion::Todo,
    Seccion::Buzon,
];

/// La ventana entera, que es lo que tapa el velo.
fn ventana_de(d: &Disposicion) -> Rect {
    Rect {
        x: 0,
        y: 0,
        ancho: d.barra.ancho,
        alto: d.chat.abajo().max(d.lista.abajo()).max(0) as u32,
    }
}

/// **Donde cae la isla**, para pintarla y para repartir sus clics: centrada
/// bajo la barra de titulo, que sigue siendo de la ventana (moverla,
/// cerrarla) y no se tapa.
pub(super) fn disposicion(d: &Disposicion, escala: u32) -> Isla {
    let todo = ventana_de(d);
    let bajo_la_barra = Rect {
        y: d.barra.abajo(),
        alto: (todo.abajo() - d.barra.abajo()).max(0) as u32,
        ..todo
    };
    let (caja, completa) = ui::isla_en(bajo_la_barra, escala);
    Isla::calcular(caja, completa, escala)
}

/// Las pestanas que salen, con cuantas cosas tiene cada una: las que tienen
/// algo y, aunque este vacia, la que esta puesta.
pub(super) fn pestanas(a: &Abierto) -> Vec<(Seccion, usize)> {
    ORDEN
        .iter()
        .map(|s| (*s, indices_de_la_seccion(a, *s).len()))
        .filter(|(s, n)| *n > 0 || a.info == Some(*s))
        .collect()
}

/// La pestana con la que se abre: la primera que tenga algo; en un chat
/// vacio, «Todo», que dice que no hay nada.
pub(super) fn primera(a: &Abierto) -> Seccion {
    ORDEN
        .iter()
        .copied()
        .find(|s| !indices_de_la_seccion(a, *s).is_empty())
        .unwrap_or(Seccion::Todo)
}

/// Lo que ensena la seccion, con lo buscado con la lupa ya filtrado.
pub(super) fn de_la_seccion(a: &Abierto, seccion: Seccion) -> Vec<usize> {
    let mut suyos = indices_de_la_seccion(a, seccion);
    if !a.busqueda_info.trim().is_empty() {
        suyos.retain(|n| {
            let m = &a.mensajes[*n];
            pixpin_ui::resaltado::hay_coincidencia(&m.resumen(), &a.busqueda_info)
                || pixpin_ui::resaltado::hay_coincidencia(&m.nombre, &a.busqueda_info)
        });
    }
    suyos
}

/// Como queda el contenido de una seccion: lo que se ve y donde.
pub(super) struct Colocado {
    /// Todo lo de la seccion, por su posicion en `a.mensajes`.
    pub suyos: Vec<usize>,
    /// Lo que se ve: `(posicion en a.mensajes, su celda o su fila)`.
    pub cosas: Vec<(usize, Rect)>,
    /// Lo que mide todo junto (para el tope de la rueda).
    pub alto: u32,
    pub cuadricula: bool,
}

/// **Coloca el contenido** de la seccion en la isla: la misma cuenta para
/// pintar, para el clic, para el resalte del raton y para cargar las fotos.
pub(super) fn colocar(a: &Abierto, seccion: Seccion, i: &Isla, escala: u32) -> Colocado {
    let suyos = de_la_seccion(a, seccion);
    let area = i.contenido;
    if seccion.es_cuadricula() {
        let r = ui::rejilla_isla(area.ancho, escala);
        let arriba = Rect {
            y: area.y + r.hueco as i32,
            ..area
        };
        let (primera, cuantas) = r.visibles(arriba, suyos.len(), a.scroll_info, escala);
        let cosas = (primera..primera + cuantas)
            .map(|n| (suyos[n], r.celda(n, arriba, a.scroll_info, escala)))
            .collect();
        Colocado {
            alto: r.alto_total(suyos.len(), escala) + 2 * r.hueco,
            suyos,
            cosas,
            cuadricula: true,
        }
    } else {
        let (primera, cuantas) = ui::filas_visibles(area, suyos.len(), a.scroll_info, escala);
        let cosas = (primera..primera + cuantas)
            .map(|n| (suyos[n], ui::fila_isla(n, area, a.scroll_info, escala)))
            .collect();
        Colocado {
            alto: ui::alto_de_las_filas(suyos.len(), escala),
            suyos,
            cosas,
            cuadricula: false,
        }
    }
}

/// La anterior (`paso` -1) o la siguiente (+1) a la que se esta viendo en
/// la previa, dentro de su seccion.
pub(super) fn vecina(a: &Abierto, paso: i32) -> Option<usize> {
    let (seccion, actual) = (a.info?, a.info_abierta?);
    let suyos = de_la_seccion(a, seccion);
    let k = suyos.iter().position(|n| *n == actual)? as i32 + paso;
    (k >= 0).then(|| suyos.get(k as usize).copied()).flatten()
}

/// Lo que hay bajo el raton en la isla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ZonaInfo {
    /// Fuera de la isla, en el velo: cierra.
    Fuera,
    Cerrar,
    Buscar,
    CajaBuscar,
    Pestana(Seccion),
    /// Una cosa de la rejilla o de las filas (su posicion en `a.mensajes`).
    Cosa(usize),
    /// En la previa: la flecha de volver a la rejilla, las de los lados y
    /// los dos botones del pie.
    Volver,
    Anterior,
    Siguiente,
    Abrir,
    EnElChat,
    /// Dentro de la isla, sin nada que hacer: no cierra.
    Dentro,
}

/// **Que hay en `l`**. Las mismas cuentas que el pintado.
pub(super) fn zona_en(a: &Abierto, l: Punto, d: &Disposicion, escala: u32) -> ZonaInfo {
    let Some(seccion) = a.info else {
        return ZonaInfo::Fuera;
    };
    let i = disposicion(d, escala);
    if !i.panel.contiene(l) {
        return ZonaInfo::Fuera;
    }
    if i.cerrar.contiene(l) {
        return ZonaInfo::Cerrar;
    }
    if a.info_abierta.is_some() {
        let p = ui::previa(i.hueco_de_la_previa(), escala);
        return if i.avatar.contiene(l) {
            ZonaInfo::Volver
        } else if p.anterior.contiene(l) && vecina(a, -1).is_some() {
            ZonaInfo::Anterior
        } else if p.siguiente.contiene(l) && vecina(a, 1).is_some() {
            ZonaInfo::Siguiente
        } else if p.abrir.contiene(l) {
            ZonaInfo::Abrir
        } else if p.en_el_chat.contiene(l) {
            ZonaInfo::EnElChat
        } else {
            ZonaInfo::Dentro
        };
    }
    if i.buscar.contiene(l) {
        return ZonaInfo::Buscar;
    }
    if a.buscando_info && i.caja_buscar.contiene(l) {
        return ZonaInfo::CajaBuscar;
    }
    let lista = pestanas(a);
    let anchos = a.anchos_pestanas.borrow().clone();
    if anchos.len() == lista.len()
        && let Some(n) = i
            .pestanas(&anchos, escala)
            .iter()
            .position(|r| r.contiene(l))
    {
        return ZonaInfo::Pestana(lista[n].0);
    }
    if i.contenido.contiene(l)
        && let Some((n, _)) = colocar(a, seccion, &i, escala)
            .cosas
            .iter()
            .find(|(_, r)| r.contiene(l))
    {
        return ZonaInfo::Cosa(*n);
    }
    ZonaInfo::Dentro
}

/// Hasta donde baja la rueda en la seccion abierta.
pub(super) fn tope(a: &Abierto, d: &Disposicion, escala: u32) -> i32 {
    let i = disposicion(d, escala);
    (a.alto_info.get() as i32 - i.contenido.alto as i32).max(0)
}

/// Las fotos que hay que tener leidas para el proximo fotograma: las
/// miniaturas de lo que se ve y, en la previa, la grande de la abierta. Y los
/// fondos de los lienzos que se ven (paginas de PDF), de las vistas previas.
pub(super) fn a_cargar(
    a: &Abierto,
    d: &Disposicion,
    escala: u32,
) -> (Vec<std::path::PathBuf>, Vec<std::path::PathBuf>) {
    let Some(seccion) = a.info else {
        return (Vec::new(), Vec::new());
    };
    let foto = |n: usize| {
        let m = a.mensajes.get(n)?;
        (m.clase == Some(Clase::Imagen) && !pixpin_proyecto::cuaderno::es_video(m))
            .then(|| ruta_del_mensaje(&a.raiz, &a.ficha.id, m))
            .flatten()
    };
    let fondo = |n: usize| match a.vistas.get(n) {
        Some(Some(Ojeada::Lienzo(l))) => l.fondo.as_ref().map(|(r, _)| r.clone()),
        _ => None,
    };
    if let Some(n) = a.info_abierta {
        let grandes = foto(n).into_iter().chain(fondo(n)).collect();
        return (Vec::new(), grandes);
    }
    let i = disposicion(d, escala);
    let cosas = colocar(a, seccion, &i, escala).cosas;
    let miniaturas = cosas.iter().filter_map(|(n, _)| foto(*n)).collect();
    let fondos = cosas.iter().filter_map(|(n, _)| fondo(*n)).collect();
    (miniaturas, fondos)
}

// --- Pintar ------------------------------------------------------------------

/// **Pinta la isla** encima de todo, con el velo detras.
pub(super) fn pintar(p: &Pintor, d: &Disposicion, c: &Pinta, a: &Abierto, seccion: Seccion) {
    let (tema, escala) = (c.tema, c.escala);
    let e = escala as f32 / 100.0;
    // El velo, bajo la barra de titulo: dice que lo de debajo espera.
    let todo = ventana_de(d);
    p.rellenar(
        rf(Rect {
            y: d.barra.abajo(),
            alto: (todo.abajo() - d.barra.abajo()).max(0) as u32,
            ..todo
        }),
        tema.velo,
    );
    let i = disposicion(d, escala);
    let sobre = zona_en(a, a.raton_info, d, escala);
    let radio = if i.pantalla_completa {
        0.0
    } else {
        ui::ISLA_RADIO as f32 * e
    };
    // La sombra y el filete, como la hoja del «+»: se despega del velo.
    let caja = rf(i.panel);
    if !i.pantalla_completa {
        p.rellenar_redondeado(
            RectF {
                y: caja.y + 10.0 * e,
                ..encoger(caja, -4.0 * e)
            },
            radio + 4.0 * e,
            con_alfa(Color::NEGRO, 0.22),
        );
        p.rellenar_redondeado(caja, radio, tema.pildora_borde);
    }
    p.rellenar_redondeado(encoger(caja, 1.0), (radio - 1.0).max(0.0), tema.lista);

    pintar_cabecera(p, c, a, &i, sobre);
    // La raya bajo la cabecera.
    p.rellenar(
        RectF {
            x: i.cabecera.x as f32 + radio / 2.0,
            y: i.cabecera.abajo() as f32 - 1.0,
            ancho: i.cabecera.ancho as f32 - radio,
            alto: 1.0,
        },
        tema.separador,
    );

    match a.info_abierta {
        Some(n) => pintar_previa(p, c, a, &i, n, sobre),
        None => {
            pintar_pestanas(p, c, a, &i, seccion, sobre);
            pintar_contenido(p, c, a, &i, seccion, sobre);
        }
    }
}

/// Un boton redondo con su icono, con el fondo puesto al pasar por encima.
fn boton_redondo(p: &Pintor, c: &Pinta, r: Rect, icono: &Icono, encima: bool) {
    let e = c.escala as f32 / 100.0;
    let caja = rf(r);
    if encima {
        p.rellenar_redondeado(caja, caja.ancho.min(caja.alto) / 2.0, c.tema.fila_sobre);
    }
    let lado = 22.0 * e;
    p.icono(
        icono,
        RectF {
            x: caja.x + (caja.ancho - lado) / 2.0,
            y: caja.y + (caja.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        c.tema.texto,
    );
}

fn pintar_cabecera(p: &Pintor, c: &Pinta, a: &Abierto, i: &Isla, sobre: ZonaInfo) {
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    boton_redondo(p, c, i.cerrar, &mi::CLOSE, sobre == ZonaInfo::Cerrar);

    // En la previa: la flecha de volver en el sitio del avatar, el nombre de
    // la cosa y «3 de 24 · fecha · tamano».
    if let Some(n) = a.info_abierta {
        boton_redondo(p, c, i.avatar, &mi::ARROW_BACK, sobre == ZonaInfo::Volver);
        let Some(m) = a.mensajes.get(n) else {
            return;
        };
        let suyos = a.info.map(|s| de_la_seccion(a, s)).unwrap_or_default();
        let k = suyos.iter().position(|x| *x == n).map_or(0, |k| k + 1);
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("k", k as i64);
        args.set("n", suyos.len() as i64);
        let mut sub = vec![
            textos.t_args("info-previa-posicion", &args),
            chat::etiqueta_hora(pixpin_shell::entorno::a_local(m.cuando), c.ahora),
        ];
        if m.bytes > 0 {
            sub.push(chat::tamano_corto(m.bytes as u64));
        }
        titulo_y_subtitulo(p, c, i, &nombre_de(m), &sub.join("  ·  "));
        return;
    }

    boton_redondo(
        p,
        c,
        i.buscar,
        &mi::SEARCH,
        sobre == ZonaInfo::Buscar || a.buscando_info,
    );
    // El avatar del chat, como en su fila de la lista: su logo, o el circulo
    // de su color con las iniciales.
    let av = rf(i.avatar);
    if !logo::pintar(p, &a.ficha.id, av) {
        p.rellenar_redondeado(av, av.ancho / 2.0, color_avatar(&a.ficha.codigo_unico()));
        let letras = iniciales(&a.ficha.nombre);
        let tam = av.alto * 0.4;
        let (w, h) = p.medir_texto(&letras, tam);
        p.texto(
            &letras,
            av.x + (av.ancho - w) / 2.0,
            av.y + (av.alto - h) / 2.0,
            tam,
            Color::BLANCO,
        );
    }
    if a.buscando_info {
        // Con la lupa encendida, la caja de buscar ocupa el sitio del nombre.
        let b = i.caja_buscar;
        p.rellenar_redondeado(rf(b), b.alto as f32 / 2.0, tema.buscador);
        let margen = ui::BUSCAR_TEXTO_X as f32 * e;
        a.busqueda_info.pintar(
            p,
            rf(b),
            RectF {
                x: b.x as f32 + margen,
                ancho: (b.ancho as f32 - 2.0 * margen).max(0.0),
                ..rf(b)
            },
            ui::BUSCAR_TAM * e,
            &textos.t("chat-buscar"),
            &colores_de_caja(tema, tema.texto, tema.apagado),
            true,
            std::time::Instant::now(),
        );
        return;
    }
    // Debajo del nombre, cuantas cosas tiene y el codigo unico del chat: es
    // el que hay que mirar para saber si lo de aqui y lo del movil son la
    // misma cosa (en el movil sale igual).
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("n", indices_de_la_seccion(a, Seccion::Todo).len() as i64);
    args.set("codigo", a.ficha.codigo_unico());
    titulo_y_subtitulo(
        p,
        c,
        i,
        &a.ficha.nombre,
        &textos.t_args("info-resumen", &args),
    );
}

fn titulo_y_subtitulo(p: &Pintor, c: &Pinta, i: &Isla, titulo: &str, subtitulo: &str) {
    let e = c.escala as f32 / 100.0;
    let (t, s) = (ui::ISLA_TITULO_TAM * e, ui::ISLA_SUBTITULO_TAM * e);
    let (_, th) = p.medir_texto("Ag", t);
    let (_, sh) = p.medir_texto("Ag", s);
    let hueco = 3.0 * e;
    let y = i.titulo.y as f32 + (i.titulo.alto as f32 - th - hueco - sh) / 2.0;
    let x = i.titulo.x as f32;
    let ancho = i.titulo.ancho as f32;
    p.texto_linea(titulo, x, y, t, ancho, c.tema.texto);
    p.texto_linea(subtitulo, x, y + th + hueco, s, ancho, c.tema.apagado);
}

/// El nombre con el que se ensena una cosa: el del fichero, o su texto.
fn nombre_de(m: &Mensaje) -> String {
    let r = m.resumen();
    if r.trim().is_empty() {
        m.nombre.clone()
    } else {
        r.lines().next().unwrap_or_default().to_string()
    }
}

fn pintar_pestanas(
    p: &Pintor,
    c: &Pinta,
    a: &Abierto,
    i: &Isla,
    seccion: Seccion,
    sobre: ZonaInfo,
) {
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let tam = ui::ISLA_PESTANA_TAM * e;
    let tam_n = (ui::ISLA_PESTANA_TAM - 2.0) * e;
    let hueco = 5.0 * e;
    // Cada pestana: su nombre y, mas apagada, su cuenta. Se mide aqui, que
    // es donde esta la fuente, y se apunta para que el clic sepa en cual cae.
    let lista: Vec<(Seccion, String, String)> = pestanas(a)
        .into_iter()
        .map(|(s, n)| (s, textos.t(s.clave()), n.to_string()))
        .collect();
    let anchos: Vec<f32> = lista
        .iter()
        .map(|(_, r, n)| p.medir_texto(r, tam).0 + hueco + p.medir_texto(n, tam_n).0)
        .collect();
    *a.anchos_pestanas.borrow_mut() = anchos.clone();
    let rects = i.pestanas(&anchos, escala);
    let fondo = i.fondo_de_la_tira(&rects);
    p.rellenar_redondeado(rf(fondo), fondo.alto as f32 / 2.0, tema.buscador);
    p.empujar_recorte(rf(fondo));
    for ((s, rotulo, cuenta), r) in lista.iter().zip(&rects) {
        let activa = *s == seccion;
        let pildora = i.pildora(*r, escala);
        if activa {
            p.rellenar_redondeado(rf(pildora), pildora.alto as f32 / 2.0, tema.pestana_activa);
        } else if sobre == ZonaInfo::Pestana(*s) {
            p.rellenar_redondeado(rf(pildora), pildora.alto as f32 / 2.0, tema.fila_sobre);
        }
        let (w, h) = p.medir_texto(rotulo, tam);
        let (wn, hn) = p.medir_texto(cuenta, tam_n);
        let x = r.x as f32 + ((r.ancho as f32 - w - hueco - wn) / 2.0).max(2.0 * e);
        let color = if activa {
            tema.texto_pestana_activa
        } else {
            tema.texto
        };
        p.texto_linea(
            rotulo,
            x,
            r.y as f32 + (r.alto as f32 - h) / 2.0,
            tam,
            (r.ancho as f32 - 4.0 * e).max(0.0),
            color,
        );
        if x + w + hueco + wn <= r.derecha() as f32 {
            p.texto(
                cuenta,
                x + w + hueco,
                r.y as f32 + (r.alto as f32 - hn) / 2.0,
                tam_n,
                if activa {
                    con_alfa(tema.texto_pestana_activa, 0.7)
                } else {
                    tema.apagado
                },
            );
        }
    }
    p.soltar_recorte();
}

fn pintar_contenido(
    p: &Pintor,
    c: &Pinta,
    a: &Abierto,
    i: &Isla,
    seccion: Seccion,
    sobre: ZonaInfo,
) {
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let colocado = colocar(a, seccion, i, escala);
    a.alto_info.set(colocado.alto);
    if colocado.suyos.is_empty() {
        // Vacio: un icono grande y una frase, en medio.
        let lado = 44.0 * e;
        let cx = i.contenido.x as f32 + i.contenido.ancho as f32 / 2.0;
        let cy = i.contenido.y as f32 + i.contenido.alto as f32 / 2.0;
        p.icono(
            icono_de_la_seccion(seccion),
            RectF {
                x: cx - lado / 2.0,
                y: cy - lado,
                ancho: lado,
                alto: lado,
            },
            con_alfa(tema.apagado, 0.6),
        );
        let vacio = if a.busqueda_info.trim().is_empty() {
            textos.t("info-vacia")
        } else {
            textos.t("chat-buscar-nada")
        };
        let (w, _) = p.medir_texto(&vacio, 13.5 * e);
        p.texto(&vacio, cx - w / 2.0, cy + 10.0 * e, 13.5 * e, tema.apagado);
        return;
    }
    p.empujar_recorte(rf(i.contenido));
    for (n, r) in &colocado.cosas {
        let encima = sobre == ZonaInfo::Cosa(*n);
        if colocado.cuadricula {
            pintar_celda(p, c, a, *n, *r, encima);
        } else {
            pintar_fila(p, c, a, *n, *r, encima);
        }
    }
    p.soltar_recorte();
}

/// El icono que representa una seccion (para el vacio).
fn icono_de_la_seccion(s: Seccion) -> &'static Icono {
    match s {
        Seccion::Fotos | Seccion::Galeria => &mi::PHOTO_LIBRARY,
        Seccion::Videos => &mi::PLAY_ARROW,
        Seccion::Enlaces => &mi::LINK,
        Seccion::Voz => &mi::MIC,
        Seccion::Dibujos => &mi::DRAW,
        Seccion::Fijados => &mi::PUSH_PIN,
        _ => &mi::DESCRIPTION,
    }
}

/// Una celda de la rejilla: la foto recortada al cuadrado, el dibujo sobre
/// su papel, o la tarjeta de lo que no tiene vista previa. Los videos llevan
/// el triangulo encima.
fn pintar_celda(p: &Pintor, c: &Pinta, a: &Abierto, n: usize, celda: Rect, encima: bool) {
    let (tema, e) = (c.tema, c.escala as f32 / 100.0);
    let Some(m) = a.mensajes.get(n) else {
        return;
    };
    let video = pixpin_proyecto::cuaderno::es_video(m);
    let foto = (!video)
        .then(|| ruta_del_mensaje(&a.raiz, &a.ficha.id, m))
        .flatten()
        .and_then(|ruta| c.miniaturas.ya(&ruta));
    if let Some((b, w, h)) = foto {
        p.empujar_recorte(rf(celda));
        crate::miniaturas::pintar_recortado(p, b, rf(celda), w, h);
        p.soltar_recorte();
    } else {
        match a
            .vistas
            .get(n)
            .and_then(|v| v.as_ref())
            .filter(|v| !matches!(v, Ojeada::Foto { .. }))
        {
            Some(vista) => pintar_ojeada(p, c, vista, rf(celda), 6.0 * e),
            None => pintar_tarjeta_galeria(p, c, m, celda),
        }
    }
    if video {
        insignia_de_video(
            p,
            rf(celda),
            (celda.ancho.min(celda.alto) as f32 / 5.0).max(14.0 * e),
        );
    }
    if encima {
        // Al pasar por encima se aclara y lleva su filete: se sabe que se
        // puede pulsar.
        p.rellenar(rf(celda), con_alfa(Color::BLANCO, 0.12));
        p.trazar(encoger(rf(celda), 1.0), (2.0 * e).max(1.0), tema.enviar);
    }
}

/// Una vista ya leida (dibujo, tabla, cuaderno) dentro de `caja`, sobre su
/// papel.
fn pintar_ojeada(p: &Pintor, c: &Pinta, vista: &Ojeada, caja: RectF, radio: f32) {
    let (tema, e) = (c.tema, c.escala as f32 / 100.0);
    let papel = match vista {
        Ojeada::Lienzo(l) => papel_de_vista(l, tema.papel),
        _ => tema.papel,
    };
    p.rellenar_redondeado(caja, radio, papel);
    p.empujar_recorte(caja);
    let dentro = encoger(caja, 6.0 * e);
    match vista {
        Ojeada::Lienzo(l) => pintar_lienzo(p, l, dentro, Some(c.previas)),
        Ojeada::Tabla(t) => pintar_ojeada_tabla(p, tema, c.escala, t, dentro),
        Ojeada::Carpeta(_) => {
            let lado = (dentro.ancho.min(dentro.alto) * 0.5).max(1.0);
            p.icono(
                &mi::MENU_BOOK,
                RectF {
                    x: dentro.x + (dentro.ancho - lado) / 2.0,
                    y: dentro.y + (dentro.alto - lado) / 2.0,
                    ancho: lado,
                    alto: lado,
                },
                tema.texto_papel,
            );
        }
        Ojeada::Foto { .. } => {}
    }
    p.soltar_recorte();
}

/// El circulo oscuro con el triangulo de «reproducir», en medio de `caja`.
fn insignia_de_video(p: &Pintor, caja: RectF, radio: f32) {
    let centro = (caja.x + caja.ancho / 2.0, caja.y + caja.alto / 2.0);
    p.circulo(centro, radio, con_alfa(Color::NEGRO, 0.55));
    let lado = radio * 1.2;
    p.icono(
        &mi::PLAY_ARROW,
        RectF {
            x: centro.0 - lado / 2.0,
            y: centro.1 - lado / 2.0,
            ancho: lado,
            alto: lado,
        },
        Color::BLANCO,
    );
}

/// Una fila: archivos, enlaces, voz, notas. A la izquierda su miniatura (la
/// foto, la chapa con la extension o un circulo con su icono), y dos lineas:
/// el nombre y, debajo, de que es, cuanto pesa y cuando (o la direccion, en
/// un enlace).
fn pintar_fila(p: &Pintor, c: &Pinta, a: &Abierto, n: usize, fila: Rect, encima: bool) {
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let Some(m) = a.mensajes.get(n) else {
        return;
    };
    if encima {
        p.rellenar_redondeado(rf(fila), 10.0 * e, tema.fila_sobre);
    }
    let f = ui::dentro_de_la_fila(fila, escala);
    let enlace = tarjetas::primer_enlace(&m.texto);
    let foto = ruta_del_mensaje(&a.raiz, &a.ficha.id, m).and_then(|r| c.miniaturas.ya(&r));
    let circulo = |icono: &Icono, color: Color| {
        let r = rf(f.miniatura);
        p.rellenar_redondeado(r, r.ancho / 2.0, color);
        let lado = r.ancho * 0.5;
        p.icono(
            icono,
            RectF {
                x: r.x + (r.ancho - lado) / 2.0,
                y: r.y + (r.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            Color::BLANCO,
        );
    };
    match (foto, &enlace) {
        (Some((b, w, h)), _) => {
            p.empujar_recorte(rf(f.miniatura));
            crate::miniaturas::pintar_recortado(p, b, rf(f.miniatura), w, h);
            p.soltar_recorte();
        }
        (None, Some(_)) if m.ruta.is_none() => circulo(&mi::LINK, tema.enviar),
        (None, _) if m.clase == Some(Clase::Voz) => circulo(&mi::PLAY_ARROW, tema.enviar),
        (None, _) if m.ruta.is_some() || !m.nombre.is_empty() => {
            pintar_tarjeta_galeria(p, c, m, f.miniatura)
        }
        (None, _) => circulo(&mi::DESCRIPTION, tema.apagado),
    }
    let nombre = nombre_de(m);
    p.parrafo(
        &nombre,
        f.nombre.x as f32,
        f.nombre.y as f32,
        13.5 * e,
        f.ancho_texto as f32,
        &negritas(&nombre, &a.busqueda_info),
        tema.texto,
    );
    // La segunda linea. En un enlace, su servidor y su camino, del color de
    // los enlaces: es lo que dice adonde lleva.
    let (segunda, color) = match &enlace {
        Some(en) if m.ruta.is_none() => {
            let donde = if en.de_que.is_empty() {
                en.donde.clone()
            } else {
                format!("{} / {}", en.donde, en.de_que)
            };
            (donde, tema.enlace)
        }
        _ => {
            let mut partes: Vec<String> = Vec::new();
            if let Some(clase) = super::clase_de(m, textos) {
                partes.push(clase);
            }
            if m.bytes > 0 {
                partes.push(chat::tamano_corto(m.bytes as u64));
            }
            partes.push(chat::etiqueta_hora(
                pixpin_shell::entorno::a_local(m.cuando),
                c.ahora,
            ));
            (partes.join("  ·  "), tema.apagado)
        }
    };
    p.texto_linea(
        &segunda,
        f.estado.x as f32,
        f.estado.y as f32,
        12.0 * e,
        f.ancho_texto as f32,
        color,
    );
}

/// **La previa de una cosa**, en grande dentro de la isla.
fn pintar_previa(p: &Pintor, c: &Pinta, a: &Abierto, i: &Isla, n: usize, sobre: ZonaInfo) {
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let Some(m) = a.mensajes.get(n) else {
        return;
    };
    let pr = ui::previa(i.hueco_de_la_previa(), escala);
    let vista = rf(pr.vista);
    let video = pixpin_proyecto::cuaderno::es_video(m);
    let ruta = ruta_del_mensaje(&a.raiz, &a.ficha.id, m);
    // La foto, entera y sin deformar: de las grandes si ya esta, si no de
    // las miniaturas (se vera borrosa un instante).
    let foto = (!video)
        .then_some(ruta.as_ref())
        .flatten()
        .filter(|_| m.clase == Some(Clase::Imagen))
        .and_then(|r| c.previas.ya(r).or_else(|| c.miniaturas.ya(r)));
    let ojeada = a
        .vistas
        .get(n)
        .and_then(|v| v.as_ref())
        .filter(|v| !matches!(v, Ojeada::Foto { .. }));
    if let Some((b, w, h)) = foto {
        p.bitmap(b, rf(ui::encajar(w, h, pr.vista)), None, false);
    } else if let Some(o) = ojeada {
        pintar_ojeada(p, c, o, vista, 12.0 * e);
    } else if matches!(m.clase, Some(Clase::Nota) | None) {
        // Una nota (o un enlace): su texto, en una tarjeta.
        p.rellenar_redondeado(vista, 12.0 * e, tema.burbuja_otra);
        let margen = 18.0 * e;
        p.empujar_recorte(vista);
        p.parrafo(
            &m.texto,
            vista.x + margen,
            vista.y + margen,
            15.0 * e,
            (vista.ancho - 2.0 * margen).max(0.0),
            &[],
            tema.texto,
        );
        p.soltar_recorte();
    } else {
        // Lo demas: su tarjeta (la chapa con la extension o el icono de su
        // clase) en un cuadrado, y debajo su nombre entero.
        let lado = (vista.ancho.min(vista.alto) * 0.55).max(1.0);
        let tarjeta = Rect {
            x: (vista.x + (vista.ancho - lado) / 2.0) as i32,
            y: (vista.y + (vista.alto - lado) / 2.0 - 18.0 * e) as i32,
            ancho: lado as u32,
            alto: lado as u32,
        };
        pintar_tarjeta_galeria(p, c, m, tarjeta);
        if video {
            insignia_de_video(p, rf(tarjeta), lado / 6.0);
        }
        let nombre = if m.nombre.is_empty() {
            nombre_de(m)
        } else {
            m.nombre.clone()
        };
        let (w, _) = p.medir_texto(&nombre, 14.0 * e);
        let w = w.min(vista.ancho);
        p.texto_linea(
            &nombre,
            vista.x + (vista.ancho - w) / 2.0,
            tarjeta.abajo() as f32 + 14.0 * e,
            14.0 * e,
            vista.ancho,
            tema.texto,
        );
    }

    // Las flechas de los lados, solo si hay a donde ir.
    for (r, paso, icono, zona) in [
        (pr.anterior, -1, &mi::CHEVRON_LEFT, ZonaInfo::Anterior),
        (pr.siguiente, 1, &mi::CHEVRON_RIGHT, ZonaInfo::Siguiente),
    ] {
        if vecina(a, paso).is_none() {
            continue;
        }
        let caja = rf(r);
        p.rellenar_redondeado(
            caja,
            caja.ancho / 2.0,
            if sobre == zona {
                tema.fila_elegida
            } else {
                tema.buscador
            },
        );
        let lado = caja.ancho * 0.6;
        p.icono(
            icono,
            RectF {
                x: caja.x + (caja.ancho - lado) / 2.0,
                y: caja.y + (caja.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            if sobre == zona {
                tema.texto_elegido
            } else {
                tema.texto
            },
        );
    }

    // El pie: «Ver en el chat» y «Abrir».
    let abrir = if tarjetas::primer_enlace(&m.texto).is_some() && m.ruta.is_none() {
        textos.t("info-abrir-enlace")
    } else if m.clase == Some(Clase::Voz) || video {
        textos.t("info-reproducir")
    } else {
        textos.t("info-abrir")
    };
    for (r, rotulo, icono, principal, zona) in [
        (
            pr.en_el_chat,
            textos.t("info-ver-en-el-chat"),
            &mi::ARROW_FORWARD,
            false,
            ZonaInfo::EnElChat,
        ),
        (pr.abrir, abrir, &mi::OPEN_IN_NEW, true, ZonaInfo::Abrir),
    ] {
        let caja = rf(r);
        let fondo = match (principal, sobre == zona) {
            (true, true) => tema.enviar,
            (true, false) => con_alfa(tema.enviar, 0.9),
            (false, true) => tema.fila_sobre,
            (false, false) => tema.buscador,
        };
        p.rellenar_redondeado(caja, caja.alto / 2.0, fondo);
        let tinta = if principal { Color::BLANCO } else { tema.texto };
        let lado = 18.0 * e;
        let tam = 13.5 * e;
        let (w, h) = p.medir_texto(&rotulo, tam);
        let w = w.min(caja.ancho - lado - 24.0 * e);
        let x0 = caja.x + (caja.ancho - lado - 6.0 * e - w) / 2.0;
        p.icono(
            icono,
            RectF {
                x: x0,
                y: caja.y + (caja.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            tinta,
        );
        p.texto_linea(
            &rotulo,
            x0 + lado + 6.0 * e,
            caja.y + (caja.alto - h) / 2.0,
            tam,
            w.max(0.0),
            tinta,
        );
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_orden_lleva_cada_seccion_una_vez() {
        for s in Seccion::TODAS {
            assert_eq!(
                ORDEN.iter().filter(|x| **x == s).count(),
                1,
                "{s:?} tiene que salir una vez"
            );
        }
        // Lo que se mira va antes que lo que se lee, y el buzon al final.
        let sitio = |s: Seccion| ORDEN.iter().position(|x| *x == s).unwrap();
        assert!(sitio(Seccion::Fotos) < sitio(Seccion::Archivos));
        assert!(sitio(Seccion::Videos) < sitio(Seccion::Enlaces));
        assert_eq!(ORDEN[ORDEN.len() - 1], Seccion::Buzon);
    }
}
