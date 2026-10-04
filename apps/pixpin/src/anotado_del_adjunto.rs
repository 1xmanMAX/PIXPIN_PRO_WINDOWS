//! **Lo anotado en los lectores sobre un adjunto del chat, con los nombres
//! del movil** (v0.96, `sincro/AnotacionesDelAdjunto.kt`,
//! `ui/ExportarDocumentoAnotado.kt`, `pdf/LectorPdfActivity.kt`).
//!
//! Los lectores del PC (Word/libro en `visor`, PDF en `lector_pdf`) leen y
//! guardan por aqui. Si el documento es un adjunto de un chat del almacen,
//! lo que viaja va a los ficheros `anot-<uid>…` del mensaje (ver
//! `pixpin_proyecto::anotado`, donde esta el porque de una sola verdad):
//!
//! | Que | Word / libro | PDF suelto | PDF de un proyecto |
//! |---|---|---|---|
//! | tinta | `anot-<uid>` (corrida `izq`) | `anot-<uid>-p<n>` | sus hojas (ya viajaban) |
//! | marcadores | `.marcas` (`id:fraccion:emoji`) | `.marcas` (`id:x:y:emoji`) | `anot-<uid del proyecto>.marcas` |
//! | espacios | `.maqueta` (`izq`, `der`) | `.espacios` | `anot-<uid del proyecto>.espacios` |
//! | columna y letra | `.maqueta` | — | — |
//! | por donde iba | `.sitio` | — | — |
//! | el verde de la voz | `.voz` (`parrafo:fraccion`) | el 🟢 de `.marcas` | el 🟢 de sus marcas |
//!
//! Lo demas (el aumento, la pagina del PDF) sigue en `<doc>.pixpin-lectura`,
//! que es de esta pantalla, como en el movil siguen en sus preferencias. Lo
//! que el PC no tiene (lo de un PDF leido como texto) no
//! se toca: se queda en su fichero y se vuelve a mandar tal cual.
//!
//! **La primera vez** lo de antes (junto al documento) pasa a los ficheros
//! del mensaje si estos no existen aun, y no se borra nada (`migrar`).
//!
//! # La columna del movil y la del PC
//!
//! La maqueta es `columna,izq,der,tamano,grosor,tipo` con los numeros del
//! movil. La columna y el tamano valen tal cual (pixeles de lectura y tanto
//! por ciento). El grosor y el tipo son indices de sus cuatro pesos (300,
//! 400, 600, 800) y sus cuatro letras (serif, sans, mono, cursiva), **los
//! mismos del PC desde K16** (`lectura::TIPOS`/`GROSORES`): con otra letra
//! el texto se parte en otros renglones y la tinta cae en otra palabra, asi
//! que viajan tal cual. La tinta del movil cuenta desde el borde de la pagina (la
//! columna empieza en `izq`) y la del PC desde el borde de la columna: se
//! corre `izq` al leer y se devuelve al guardar. Una maqueta que el PC no
//! cambia no se reescribe: sus `izq`, `der`, grosor y tipo del movil quedan.

use std::path::{Path, PathBuf};

use pixpin_docs::lectura::{self, Ajustes};
use pixpin_docs::{vista, voz_alta};
use pixpin_proyecto::anotado::{self as an, Base};
use pixpin_sincro::anotado::{
    CIFRAS_DE_LA_HUELLA, Maqueta, MarcoDeLaHoja, huella_coincide, margen_de,
};

use crate::lector_tinta::{self, Capa, Unidades};

fn es_pdf(doc: &Path) -> bool {
    doc.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// La base de lo anotado de `doc`, si es un adjunto del chat (o el PDF de un proyecto).
fn base_de(doc: &Path) -> Option<Base> {
    let raiz = an::raiz_de(doc)?;
    if es_pdf(doc) {
        let proyecto = pixpin_proyecto::capas_del_pdf::de_este_pdf(&raiz, doc).map(|p| p.ficha);
        an::base_del_pdf(&raiz, doc, proyecto.as_deref())
    } else {
        an::base_del_documento(&raiz, doc)
    }
}

// ------------------------------------------------ la letra del movil y la del PC

/// El grosor del PC como indice de los del movil: desde K16 son los mismos
/// cuatro (300, 400, 600, 800).
pub fn grosor_al_movil(g: u8) -> i32 {
    i32::from(g.min(lectura::GROSORES as u8 - 1))
}

/// Uno de los cuatro grosores del movil, el mismo en el PC (uno fuera de
/// rango, al mas cercano).
pub fn grosor_del_movil(g: i32) -> u8 {
    g.clamp(0, lectura::GROSORES as i32 - 1) as u8
}

/// La letra del PC como indice de las del movil: las mismas cuatro (serif,
/// sans, ancho fijo, cursiva).
pub fn tipo_al_movil(t: u8) -> i32 {
    i32::from(t.min(lectura::TIPOS as u8 - 1))
}

/// Una de las cuatro letras del movil, la misma en el PC.
pub fn tipo_del_movil(t: i32) -> u8 {
    t.clamp(0, lectura::TIPOS as i32 - 1) as u8
}

/// **El fichero de la tinta del mensaje** de un Word o un libro del chat
/// (`anot-<uid>.excalidraw`), exista o no; `None` si no es un adjunto. El
/// lector mira si ya estaba antes de leer la capa: la de ahi va en la
/// maqueta del movil, la de junto al documento puede ser de la vieja (K16).
pub fn tinta_del_mensaje(doc: &Path) -> Option<PathBuf> {
    base_de(doc).filter(|_| !es_pdf(doc)).map(|b| b.tinta())
}

/// La maqueta que corresponde a lo fijado en el PC, conservando del movil lo
/// que el PC no sabe decir si la columna no cambio. `None` si lo que hay ya
/// dice eso mismo (no se reescribe).
pub fn maqueta_para(a: &Ajustes, actual: Option<Maqueta>) -> Option<Maqueta> {
    let columna = a.columna as i32;
    if let Some(m) = actual
        && m.columna == columna
        && lectura::tamano_valido(m.tamano.max(0) as u32) == a.tamano
        && grosor_del_movil(m.grosor) == a.grosor
        && tipo_del_movil(m.tipo) == a.tipo
        && a.lados
            .is_none_or(|(i, d)| (i as i32, d as i32) == (m.izq, m.der))
    {
        return None;
    }
    // Los espacios de cada lado: los puestos con los mandos de abajo; si no,
    // los del movil con esta misma columna, o los dos tercios de siempre.
    let (izq, der) = match (a.lados, actual) {
        (Some((i, d)), _) => (i as i32, d as i32),
        (None, Some(m)) if m.columna == columna => (m.izq, m.der),
        _ => (margen_de(columna), margen_de(columna)),
    };
    Some(Maqueta {
        columna,
        izq,
        der,
        tamano: a.tamano as i32,
        grosor: grosor_al_movil(a.grosor),
        tipo: tipo_al_movil(a.tipo),
    })
}

/// Los marcadores de un Word como los escribe `Lectura.aTexto`.
fn marcadores_a_texto(a: &Ajustes) -> String {
    a.marcadores
        .iter()
        .map(|m| format!("{}:{}:{}", m.id, m.fraccion, m.emoji))
        .collect::<Vec<_>>()
        .join("|")
}

// ------------------------------------------------------------ leer y escribir

/// **Lo que se recuerda de un documento**: lo de junto al documento y, si
/// es un adjunto del chat, lo que viaja encima (manda lo de su mensaje).
pub fn leer(doc: &Path) -> Ajustes {
    let mut a = lectura::leer(doc);
    let Some(b) = base_de(doc) else {
        return a;
    };
    migrar(&b, &a, es_pdf(doc));
    poner_lo_del_mensaje(&b, &mut a, es_pdf(doc));
    a
}

/// Lo de antes a los ficheros del mensaje, si aun no los tiene (`migrar`).
fn migrar(b: &Base, a: &Ajustes, pdf: bool) {
    if pdf {
        an::poner_si_falta(
            &b.fichero(".marcas"),
            Some(&a.marcas)
                .filter(|m| !m.is_empty())
                .map(String::as_str),
        );
        an::poner_si_falta(
            &b.fichero(".espacios"),
            (a.espacios != 0).then(|| a.espacios.to_string()).as_deref(),
        );
        return;
    }
    let marcas = (!a.marcadores.is_empty()).then(|| marcadores_a_texto(a));
    an::poner_si_falta(&b.fichero(".marcas"), marcas.as_deref());
    let maqueta = (a.columna > 0)
        .then(|| maqueta_para(a, None).map(|m| m.a_texto()))
        .flatten();
    an::poner_si_falta(&b.fichero(".maqueta"), maqueta.as_deref());
    an::poner_si_falta(
        &b.fichero(".sitio"),
        (a.sitio > 0.0).then(|| a.sitio.to_string()).as_deref(),
    );
    an::poner_si_falta(
        &b.fichero(".voz"),
        a.voz.map(|(p, f)| voz_alta::voz_a_texto(p, f)).as_deref(),
    );
}

fn poner_lo_del_mensaje(b: &Base, a: &mut Ajustes, pdf: bool) {
    let leer = |t: &str| an::leer(&b.fichero(t));
    if pdf {
        if let Some(t) = leer(".marcas") {
            a.marcas = t.replace(['\n', '\r'], "").trim().to_string();
        }
        // Lo que no se entiende no pisa lo de aqui (`toIntOrNull() ?: prefs`).
        if let Some(e) = leer(".espacios").and_then(|t| t.trim().parse::<i64>().ok()) {
            a.espacios = (e & 3) as u8;
        }
        return;
    }
    if let Some(t) = leer(".marcas") {
        a.marcadores = lectura::marcadores_de_texto(t.trim());
    }
    if let Some(t) = leer(".maqueta") {
        match Maqueta::de_texto(&t) {
            Some(m) => {
                a.columna = m.columna.clamp(0, 20_000) as u32;
                a.tamano = lectura::tamano_valido(m.tamano.max(0) as u32);
                a.grosor = grosor_del_movil(m.grosor);
                a.tipo = tipo_del_movil(m.tipo);
                // Los espacios de cada lado, dentro de lo que cabe
                // (`Lectura.espacioValido` al leer la maqueta).
                a.lados = Some((
                    vista::espacio_valido(i64::from(m.izq), a.columna),
                    vista::espacio_valido(i64::from(m.der), a.columna),
                ));
            }
            // Vacia: la columna se solto (en el PC, al quitar toda la tinta).
            None => {
                a.columna = 0;
                a.lados = None;
            }
        }
    }
    // El marcador verde de la voz (`anot-<uid>.voz`): vacio es sin verde.
    if let Some(t) = leer(".voz") {
        a.voz = voz_alta::voz_de_texto(&t);
    }
    if let Some(s) = leer(".sitio")
        .and_then(|t| t.trim().parse::<f32>().ok())
        .filter(|s| s.is_finite())
    {
        a.sitio = s.clamp(0.0, 1.0);
    }
}

/// **Guarda lo que se recuerda de un documento**: junto a el, como
/// siempre, y lo que viaja en los ficheros de su mensaje. Solo se escribe
/// lo que cambia (cada fecha nueva es algo que mandar).
pub fn escribir(doc: &Path, a: &Ajustes) -> std::io::Result<()> {
    let junto = lectura::escribir(doc, a);
    if let Some(b) = base_de(doc) {
        escribir_lo_del_mensaje(&b, a, es_pdf(doc))?;
    }
    junto
}

fn escribir_lo_del_mensaje(b: &Base, a: &Ajustes, pdf: bool) -> std::io::Result<()> {
    // Lo que no esta y no dice nada no se crea: un fichero vacio mas en el
    // otro aparato por cada documento que solo se abrio.
    let poner = |t: &str, texto: String, nada: bool| -> std::io::Result<()> {
        let f = b.fichero(t);
        if nada && !f.exists() {
            return Ok(());
        }
        an::escribir(&f, &texto)
    };
    if pdf {
        let marcas = a.marcas.replace(['\n', '\r'], "");
        poner(".marcas", marcas.clone(), marcas.is_empty())?;
        return poner(".espacios", a.espacios.to_string(), a.espacios == 0);
    }
    let marcas = marcadores_a_texto(a);
    poner(".marcas", marcas.clone(), marcas.is_empty())?;
    let f = b.fichero(".maqueta");
    let actual = an::leer(&f).and_then(|t| Maqueta::de_texto(&t));
    if a.columna == 0 {
        // Sin columna, vacia y no borrada: borrada seria «no se sabe».
        if actual.is_some() {
            an::escribir(&f, "")?;
        }
    } else if let Some(m) = maqueta_para(a, actual) {
        an::escribir(&f, &m.a_texto())?;
    }
    let f = b.fichero(".sitio");
    let igual = an::leer(&f).and_then(|t| t.trim().parse::<f32>().ok()) == Some(a.sitio);
    if !igual {
        poner(".sitio", a.sitio.to_string(), a.sitio <= 0.0)?;
    }
    // El verde: lo de ahora, o vacio si se quito (no se borra, como el movil).
    let f = b.fichero(".voz");
    let ahora = a
        .voz
        .map(|(p, fr)| voz_alta::voz_a_texto(p, fr))
        .unwrap_or_default();
    if an::leer(&f).and_then(|t| voz_alta::voz_de_texto(&t)) != a.voz {
        poner(".voz", ahora, a.voz.is_none())?;
    }
    Ok(())
}

// ------------------------------------------------------------------ la tinta

/// Cuanto se corre la tinta del movil: su margen izquierdo (`izq`), o el
/// que el PC le pondria a esta columna si aun no hay maqueta.
fn corrida(b: &Base, a: &Ajustes) -> f32 {
    an::leer(&b.fichero(".maqueta"))
        .and_then(|t| Maqueta::de_texto(&t))
        .map(|m| m.izq)
        .unwrap_or_else(|| margen_de(a.columna as i32)) as f32
}

/// **La columna del PC como marco** (`MarcoDeLaHoja`): la banda de la
/// columna desde lo alto del documento, tan alta como ancha, en unidades
/// del lector (que cuentan desde el borde de la columna). `None` sin
/// columna fijada: sin ella no hay hoja con que encajar.
fn columna_propia(a: &Ajustes) -> Option<MarcoDeLaHoja> {
    let c = a.columna as f32;
    (c > 0.0).then(|| MarcoDeLaHoja::nuevo(0.0, 0.0, c, c))
}

/// **En que unidades esta la tinta del mensaje de un Word o un libro**: las
/// de su marco (`anot-<uid>.hoja`) si lo trae; si no, la regla de antes, la
/// columna corrida su `izq` ([`corrida`]).
fn unidades_del_documento(b: &Base, a: &Ajustes) -> Unidades {
    columna_propia(a)
        .and_then(|p| unidades_del_marco(b, &p))
        .unwrap_or_else(|| Unidades::corridas(corrida(b, a)))
}

/// **El marco que corresponde a como se lee hoy sin marco la tinta de un
/// Word o un libro** (la regla vieja: la columna corrida su `izq`), para la
/// pasada que se lo pone a lo ya anotado (`marco_de_la_tinta`). No escribe
/// nada: lee lo de junto al documento y lo del mensaje sin pasar lo uno a
/// lo otro. `None` sin columna fijada (sin ella no hay hoja con que encajar).
pub fn marco_de_antes_del_documento(doc: &Path, b: &Base) -> Option<MarcoDeLaHoja> {
    let mut a = lectura::leer(doc);
    poner_lo_del_mensaje(b, &mut a, false);
    let p = columna_propia(&a)?;
    Some(Unidades::corridas(corrida(b, &a)).marco_de(&p))
}

/// Si el mensaje de este documento del chat ya tiene su `.marcas` (aunque
/// vacio: se quitaron todas). Es el «ya se guardaron» de Android
/// (`archivoDeMarcas.exists()`). `false` si no es un adjunto.
pub fn hay_marcas_del_mensaje(doc: &Path) -> bool {
    base_de(doc).is_some_and(|b| b.fichero(".marcas").exists())
}

/// Los espacios de un PDF del chat como los lee el lector (los del mensaje
/// encima de los de aqui), sin escribir nada.
pub fn espacios_de_ahora(pdf: &Path) -> u8 {
    let mut a = lectura::leer(pdf);
    if let Some(b) = base_de(pdf) {
        poner_lo_del_mensaje(&b, &mut a, true);
    }
    a.espacios
}

/// **La capa de tinta de un Word o un libro.** De un adjunto del chat, la de
/// su mensaje (con lo de antes pasado la primera vez); si no, la de junto al
/// documento.
pub fn leer_capa(doc: &Path) -> Capa {
    let vieja = lector_tinta::ruta_de_capa(doc);
    let Some(b) = base_de(doc).filter(|_| !es_pdf(doc)) else {
        return Capa::leer(&vieja);
    };
    let a = leer(doc);
    let u = unidades_del_documento(&b, &a);
    let nueva = b.tinta();
    if !nueva.exists() && vieja.is_file() {
        // La de antes esta en las unidades del PC: se escribe corrida, con
        // su marco al lado.
        let mut c = Capa::leer(&vieja);
        c.sucia = true;
        match c.guardar_en(&nueva, u) {
            Ok(()) => {
                an::marcar_tinta_pasada(&vieja);
                if let Some(p) = columna_propia(&a)
                    && let Err(e) = escribir_marco(&b, u, &p)
                {
                    tracing::warn!(?e, "no se pudo escribir el marco de la tinta pasada");
                }
            }
            Err(e) => tracing::warn!(?e, "no se pudo pasar la tinta de antes al mensaje"),
        }
    } else {
        // Si el movil ya mando la suya, la de aqui se junta (una vez): con
        // solo copiar si falta, lo anotado aqui antes no viajaba nunca.
        // Y quien escribe la tinta escribe su marco: sin el, la huella
        // seria la de antes y el marco se ignoraria.
        if an::pasar_tinta_de_antes(&vieja, &nueva, u.dx as f64)
            && let Some(p) = columna_propia(&a)
            && let Err(e) = escribir_marco(&b, u, &p)
        {
            tracing::warn!(?e, "no se pudo escribir el marco de la tinta juntada");
        }
    }
    Capa::leer_en(&nueva, u)
}

/// Guarda la capa donde [leer_capa] la leyo y, si cambio, su marco al
/// lado (`anot-<uid>.hoja`): primero la tinta, luego el marco, cada uno por
/// un temporal (ver `DondeVa::guardar_capa`).
pub fn guardar_capa(doc: &Path, capa: &mut Capa) -> std::io::Result<()> {
    match base_de(doc).filter(|_| !es_pdf(doc)) {
        Some(b) => {
            let sucia = capa.sucia;
            let a = leer(doc);
            let u = unidades_del_documento(&b, &a);
            capa.guardar_en(&b.tinta(), u)?;
            match columna_propia(&a) {
                Some(p) if sucia => escribir_marco(&b, u, &p),
                _ => Ok(()),
            }
        }
        None => capa.guardar(&lector_tinta::ruta_de_capa(doc)),
    }
}

// ------------------------------------------------------- el marco de la tinta

/// **La base de la tinta de la hoja `i` de un PDF suelto del chat**
/// (`anot-<uid>-p<i>`: su marco es `Base::marco`), junto a lo demas de su
/// mensaje; `None` si no es un adjunto.
pub fn base_de_la_hoja_del_pdf(
    adjunto: Option<&(PathBuf, pixpin_proyecto::anotado::Adjunto)>,
    i: usize,
) -> Option<Base> {
    let (raiz, x) = adjunto?;
    Some(Base::de_pagina(raiz, x, u32::try_from(i).ok()?))
}

/// **El marco de la tinta de `b`, si esta, se entiende y es de esta
/// tinta.** Uno que no se entiende se ignora (se lee con la regla de antes)
/// y se dice. Uno con la huella de otra tinta tambien: lo escribio quien
/// escribio la tinta de entonces, y un movil que aun no sabe del marco
/// (v0.97) puede haberla reescrito despues en otras unidades sin tocarlo.
/// Uno sin huella (los del PC del 29-sep) vale como hasta ahora.
pub fn leer_marco(b: &Base) -> Option<MarcoDeLaHoja> {
    let f = b.marco();
    let texto = an::leer(&f)?;
    let Some((m, huella)) = MarcoDeLaHoja::de_texto_con_huella(&texto) else {
        tracing::warn!(fichero = %f.display(), texto = %texto.trim(), "marco de la tinta que no se entiende: se ignora");
        return None;
    };
    if let Some(h) = huella {
        let resumen = b.resumen_de_la_tinta();
        if !resumen.as_deref().is_some_and(|r| huella_coincide(&h, r)) {
            tracing::warn!(
                fichero = %f.display(),
                huella = %h,
                tinta = ?resumen.as_deref().map(|r| &r[..CIFRAS_DE_LA_HUELLA]),
                "marco de otra tinta (se reescribio sin el): se ignora y se lee con la regla de antes"
            );
            return None;
        }
    }
    Some(m)
}

/// Las unidades que dice el marco de `b` para una hoja que en el lector es
/// `propia` (ver `Unidades::del_marco`); `None` sin marco que valga.
pub fn unidades_del_marco(b: &Base, propia: &MarcoDeLaHoja) -> Option<Unidades> {
    Unidades::del_marco(propia, &leer_marco(b)?)
}

/// **Escribe el marco de una tinta recien guardada en las unidades `u`**:
/// donde cae en ellas la hoja `propia`, en las dos lineas de Android. Si el
/// que hay ya dice eso mismo (el del otro aparato con el que se leyo,
/// escrito con sus decimales) no se toca —salvo quitarle la huella que le
/// ponia el PC el 29-sep, que Android no entiende—: una ida y vuelta sin
/// cambios no reescribe nada, y no viaja de vuelta un fichero que solo
/// cambio en el ultimo decimal.
pub fn escribir_marco(b: &Base, u: Unidades, propia: &MarcoDeLaHoja) -> std::io::Result<()> {
    if !propia.valido() {
        return Ok(());
    }
    if let Some((ya, huella)) =
        an::leer(&b.marco()).and_then(|t| MarcoDeLaHoja::de_texto_con_huella(&t))
        && Unidades::del_marco(propia, &ya).is_some_and(|v| v.casi_iguales(&u))
    {
        return if huella.is_some() {
            b.quitar_la_huella()
        } else {
            Ok(())
        };
    }
    b.escribir_marco(&u.marco_de(propia))
}

/// **La tinta de la hoja `i` de un PDF suelto** que es un adjunto del chat
/// (`anot-<uid>-p<i>`), con la de antes copiada la primera vez (mismas
/// unidades que el movil: se copia tal cual). `None` si no es un adjunto.
pub fn hoja_del_pdf(
    adjunto: Option<&(PathBuf, pixpin_proyecto::anotado::Adjunto)>,
    pdf: &Path,
    i: usize,
) -> Option<PathBuf> {
    use pixpin_sincro::disco::Disco;
    let (raiz, x) = adjunto?;
    let pagina = u32::try_from(i).ok()?;
    let rel = pixpin_sincro::anotado::rel(
        &pixpin_sincro::anotado::de_pagina(&x.uid, pagina),
        ".excalidraw.gz",
    );
    let nueva = pixpin_proyecto::vista::DiscoPc::nuevo(raiz).ruta(&x.chat, &rel);
    let vieja = lector_tinta::ruta_de_hoja(pdf, i);
    if an::copiar_si_falta(&vieja, &nueva) {
        an::marcar_tinta_pasada(&vieja);
    } else {
        // La hoja que ya mando el movil se junta con la de aqui, una vez.
        an::pasar_tinta_de_antes(&vieja, &nueva, 0.0);
    }
    Some(nueva)
}

/// Si un id de fichero es de los que nacen en el PC (`enlace::id_de_texto`:
/// `pc` y el numero en hexadecimal). Los del movil son de 21 letras al azar
/// con mayusculas, `_` y `-`: que uno sea `pc` y solo hexadecimal no pasa.
fn nacido_en_el_pc(id: &str) -> bool {
    id.strip_prefix("pc").is_some_and(|h| {
        (1..=16).contains(&h.len())
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// **Lo que el PC escribio en la hoja del movil antes de saber sus
/// unidades, a ellas** (una vez por hoja y por PC). Hasta el 29-sep el PC
/// leia y escribia `anot-<uid>-p<n>` como si fuera la hoja a 1400 de ancho;
/// el movil la guarda en las de su capa (ver
/// `pixpin_docs::vista::capa_del_movil`), asi que lo dibujado aqui salia
/// alli encogido y, leyendolo ya bien, saldria encogido tambien aqui. Lo
/// nacido en el PC que ya esta en el fichero se pasa a las unidades del
/// movil; lo del movil no se toca. La marca (junto al PDF, que no viaja) se
/// pone antes de escribir: correrlo dos veces lo estropearia, y no hacerlo
/// solo lo deja como estaba.
pub fn lo_del_pc_a_la_capa_del_movil(pdf: &Path, i: usize, hoja: &Path, u: lector_tinta::Unidades) {
    let marca = lector_tinta::carpeta_de(pdf).join(format!("hoja-{}.unidades-del-movil", i + 1));
    if marca.exists() {
        return;
    }
    let puesta = std::fs::create_dir_all(marca.parent().unwrap_or(Path::new(".")))
        .and_then(|()| std::fs::write(&marca, b""));
    if puesta.is_err() || !hoja.is_file() || u == lector_tinta::Unidades::DEL_PC {
        return;
    }
    let mut c = Capa::leer(hoja);
    let movidos = c.pasar_a(u, |e| {
        e.extras
            .id_de_fichero
            .as_deref()
            .is_some_and(nacido_en_el_pc)
    });
    if movidos > 0 {
        match c.guardar(hoja) {
            Ok(()) => {
                tracing::info!(movidos, hoja = %hoja.display(), "tinta del PC llevada a las unidades del movil")
            }
            Err(e) => tracing::warn!(
                ?e,
                "no se pudo llevar la tinta del PC a las unidades del movil"
            ),
        }
    }
}

/// El adjunto del chat que es este PDF, una vez por lector abierto.
pub fn adjunto_del_pdf(pdf: &Path) -> Option<(PathBuf, pixpin_proyecto::anotado::Adjunto)> {
    let raiz = an::raiz_de(pdf)?;
    let x = an::adjunto_de(&raiz, pdf)?;
    Some((raiz, x))
}

/// **Los marcadores de un lienzo**: junto a su dibujo del movil si es un
/// lienzo de un chat (`<dibujo>.marcas`, que viaja con el), con los de antes
/// pasados la primera vez; si no, el fichero de siempre.
pub fn marcas_del_lienzo(lienzo: &Path, de_siempre: PathBuf) -> PathBuf {
    match an::marcas_del_lienzo(lienzo) {
        Some(nueva) => {
            an::copiar_si_falta(&de_siempre, &nueva);
            nueva
        }
        None => de_siempre,
    }
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;
    use pixpin_proyecto::almacen::{self, Ficha, Indice};
    use pixpin_proyecto::cuaderno::{self, Clase, Mensaje, Sello};

    pub(crate) fn raiz(etiqueta: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!(
            "pixpin-anotado-app-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    /// Un almacen con un adjunto `nombre` en el chat de «Tesis».
    pub(crate) fn con_adjunto(r: &Path, nombre: &str, bytes: &[u8]) -> PathBuf {
        let ficha = Ficha::nueva("Tesis", 5, "PC01");
        let mut i = Indice::default();
        i.proyectos.push(ficha.clone());
        i.guardar(r).unwrap();
        let ruta = almacen::guardar_adjunto(r, &ficha.id, nombre, bytes).unwrap();
        let sello = Sello {
            cuando: 10,
            numero: 1,
            aparato: "PC01".into(),
            proyecto: ficha.id.clone(),
        };
        let carpeta = almacen::carpeta(r, &ficha.id);
        cuaderno::anadir(
            &carpeta,
            &Mensaje::adjunto(Clase::Archivo, nombre, &ruta, 2, &sello),
        )
        .unwrap();
        carpeta.join(&ruta)
    }

    fn base(doc: &Path) -> Base {
        base_de(doc).expect("es un adjunto del chat")
    }

    #[test]
    fn la_letra_del_movil_es_la_misma_en_el_pc_y_vuelve_igual() {
        // K16: las cuatro letras y los cuatro grosores del movil, tal cual.
        for g in 0..4u8 {
            assert_eq!(grosor_del_movil(grosor_al_movil(g)), g);
            assert_eq!(tipo_del_movil(tipo_al_movil(g)), g);
        }
        assert_eq!([0, 1, 2, 3].map(grosor_del_movil), [0, 1, 2, 3]);
        assert_eq!([0, 1, 2, 3].map(tipo_del_movil), [0, 1, 2, 3]);
        // Caso negativo: lo que no existe cae en el extremo mas cercano.
        assert_eq!([-1, 9].map(tipo_del_movil), [0, 3]);
        assert_eq!([-5, 7].map(grosor_del_movil), [0, 3]);
    }

    #[test]
    fn los_espacios_de_cada_lado_y_el_verde_de_la_voz_viajan_como_en_el_movil() {
        let r = raiz("lados-y-voz");
        let doc = con_adjunto(&r, "libro.epub", b"PK");
        let b = base(&doc);
        // Del movil: un solo paso a la izquierda y dos a la derecha, y su verde.
        an::escribir(&b.fichero(".maqueta"), "420,140,280,100,1,0").unwrap();
        an::escribir(&b.fichero(".voz"), "12:0.25").unwrap();
        let mut a = leer(&doc);
        assert_eq!(a.lados, Some((140, 280)));
        assert_eq!(a.voz, Some((12, 0.25)));
        // Sin cambios no se reescribe nada.
        escribir(&doc, &a).unwrap();
        assert_eq!(
            an::leer(&b.fichero(".maqueta")).as_deref(),
            Some("420,140,280,100,1,0")
        );
        assert_eq!(an::leer(&b.fichero(".voz")).as_deref(), Some("12:0.25"));
        // El PC quita el paso de la izquierda y el verde avanza.
        a.lados = Some((0, 280));
        a.voz = Some((13, 0.3));
        escribir(&doc, &a).unwrap();
        assert_eq!(
            an::leer(&b.fichero(".maqueta")).as_deref(),
            Some("420,0,280,100,1,0")
        );
        assert_eq!(an::leer(&b.fichero(".voz")).as_deref(), Some("13:0.3"));
        // Acabado el documento: vacio, no borrado (borrado seria «no se sabe»).
        a.voz = None;
        escribir(&doc, &a).unwrap();
        assert_eq!(an::leer(&b.fichero(".voz")).as_deref(), Some(""));
        assert_eq!(leer(&doc).voz, None);
        // Un espacio fuera de lo que cabe se acota al leer.
        an::escribir(&b.fichero(".maqueta"), "420,900,280,100,1,0").unwrap();
        assert_eq!(leer(&doc).lados, Some((280, 280)));
    }

    #[test]
    fn un_documento_sin_verde_no_crea_su_fichero() {
        let r = raiz("sin-verde");
        let doc = con_adjunto(&r, "acta.docx", b"PK");
        let b = base(&doc);
        escribir(&doc, &leer(&doc)).unwrap();
        assert!(!b.fichero(".voz").exists());
    }

    #[test]
    fn lo_que_llega_del_movil_sobre_un_word_se_lee_encima_de_lo_de_aqui() {
        let r = raiz("leer-word");
        let doc = con_adjunto(&r, "tesis.docx", b"PK");
        lectura::escribir(
            &doc,
            &Ajustes {
                columna: 860,
                zoom: 0.8,
                marcadores: lectura::marcadores_de_texto("7:0.1:X"),
                ..Ajustes::default()
            },
        )
        .unwrap();
        let b = base(&doc);
        an::escribir(&b.fichero(".maqueta"), "384,256,256,115,3,2").unwrap();
        an::escribir(&b.fichero(".marcas"), "1790218080412:0.34022403:P").unwrap();
        an::escribir(&b.fichero(".sitio"), "0.7799783").unwrap();
        let a = leer(&doc);
        assert_eq!(
            (a.columna, a.tamano, a.grosor, a.tipo),
            (384, 115, 3, 2),
            "la columna y la letra del movil"
        );
        assert_eq!(a.marcadores.len(), 1);
        assert_eq!(a.marcadores[0].emoji, "P");
        assert!((a.sitio - 0.7799783).abs() < 1e-6);
        assert_eq!(a.zoom, 0.8, "lo de esta pantalla sigue siendo de aqui");
        // Guardar sin cambiar la letra no reescribe la maqueta del movil:
        // sus margenes, su peso negro y su letra de ancho fijo se quedan.
        escribir(&doc, &a).unwrap();
        assert_eq!(
            an::leer(&b.fichero(".maqueta")).as_deref(),
            Some("384,256,256,115,3,2")
        );
        assert_eq!(
            an::leer(&b.fichero(".marcas")).as_deref(),
            Some("1790218080412:0.34022403:P")
        );
        assert_eq!(an::leer(&b.fichero(".sitio")).as_deref(), Some("0.7799783"));
        // Cambiada en el PC, se reescribe con los margenes que tenia.
        escribir(
            &doc,
            &Ajustes {
                tamano: 130,
                ..a.clone()
            },
        )
        .unwrap();
        assert_eq!(
            an::leer(&b.fichero(".maqueta")).as_deref(),
            Some("384,256,256,130,3,2")
        );
        // Y sin columna (se quito toda la tinta), vacia, no borrada.
        escribir(&doc, &Ajustes { columna: 0, ..a }).unwrap();
        assert_eq!(an::leer(&b.fichero(".maqueta")).as_deref(), Some(""));
        assert_eq!(leer(&doc).columna, 0);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn lo_de_antes_de_un_word_pasa_una_vez_a_su_mensaje_y_no_se_borra() {
        let r = raiz("migrar-word");
        let doc = con_adjunto(&r, "acta.docx", b"PK");
        let antes = Ajustes {
            columna: 860,
            sitio: 0.6239229,
            marcadores: lectura::marcadores_de_texto("1790218080412:0.34022403:P"),
            ..Ajustes::default()
        };
        lectura::escribir(&doc, &antes).unwrap();
        // La tinta de antes, en las unidades del PC (desde el borde de la columna).
        let vieja = lector_tinta::ruta_de_capa(&doc);
        std::fs::create_dir_all(vieja.parent().unwrap()).unwrap();
        std::fs::write(
            &vieja,
            r#"{"type":"excalidraw","elements":[{"id":"pc1","type":"freedraw","x":10,"y":40,"width":2,"height":2,"points":[[0,0],[2,2]]}]}"#,
        )
        .unwrap();
        let capa = leer_capa(&doc);
        let b = base(&doc);
        assert_eq!(
            an::leer(&b.fichero(".maqueta")).as_deref(),
            Some("860,573,573,100,1,0")
        );
        assert_eq!(
            an::leer(&b.fichero(".marcas")).as_deref(),
            Some("1790218080412:0.34022403:P")
        );
        assert_eq!(an::leer(&b.fichero(".sitio")).as_deref(), Some("0.6239229"));
        // En el fichero del mensaje cuenta desde el borde de la pagina; en el
        // lector, igual que antes.
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(b.tinta()).unwrap()).unwrap();
        assert_eq!(v["elements"][0]["x"].as_f64(), Some(583.0));
        assert_eq!(capa.escena.visibles().next().unwrap().x, 10.0);
        assert!(
            vieja.is_file() && lectura::ruta_de_ajustes(&doc).is_file(),
            "lo viejo se queda"
        );
        // La segunda vez no se pisa lo que ya tiene el mensaje.
        std::fs::write(&vieja, r#"{"type":"excalidraw","elements":[]}"#).unwrap();
        assert_eq!(leer_capa(&doc).escena.cuantos_visibles(), 1);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn lo_anotado_aqui_antes_se_junta_con_lo_que_ya_mando_el_movil_en_un_word_y_en_un_pdf() {
        // Queja del 28-sep: «lo que se modifica aqui no se envia». El movil
        // mando su tinta antes de que el lector del PC abriera el documento
        // y lo anotado aqui antes se quedaba junto al documento para siempre.
        let r = raiz("juntar");
        let doc = con_adjunto(&r, "biblio.docx", b"PK");
        let b = base(&doc);
        an::escribir(&b.fichero(".maqueta"), "384,256,256,100,1,0").unwrap();
        std::fs::create_dir_all(b.tinta().parent().unwrap()).unwrap();
        std::fs::write(
            b.tinta(),
            r#"{"elements":[{"id":"m1","type":"freedraw","x":300,"y":10,"width":4,"height":4,"seed":1,"points":[{"x":0.0,"y":0.0},{"x":4.0,"y":4.0}]}],"files":{}}"#,
        )
        .unwrap();
        let vieja = lector_tinta::ruta_de_capa(&doc);
        std::fs::create_dir_all(vieja.parent().unwrap()).unwrap();
        std::fs::write(
            &vieja,
            r#"{"type":"excalidraw","elements":[{"id":"pc1","type":"freedraw","x":10,"y":40,"width":0,"height":0,"points":[[0,0],[2,2]]}]}"#,
        )
        .unwrap();
        let capa = leer_capa(&doc);
        assert_eq!(
            capa.escena.cuantos_visibles(),
            2,
            "lo del movil y lo de aqui"
        );
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(b.tinta()).unwrap()).unwrap();
        assert_eq!(
            v["elements"][1]["x"].as_f64(),
            Some(266.0),
            "lo de aqui, corrido a la pagina"
        );
        // Borrado despues, no vuelve al abrir otra vez.
        std::fs::write(b.tinta(), r#"{"elements":[]}"#).unwrap();
        assert_eq!(leer_capa(&doc).escena.cuantos_visibles(), 0);

        // Lo mismo con una hoja de un PDF, sin correr.
        let pdf = con_adjunto(&r, "plano.pdf", b"%PDF-1.4");
        let adjunto = adjunto_del_pdf(&pdf);
        let (raiz, x) = adjunto.clone().unwrap();
        let nueva = {
            use pixpin_sincro::disco::Disco;
            let rel = pixpin_sincro::anotado::rel(
                &pixpin_sincro::anotado::de_pagina(&x.uid, 2),
                ".excalidraw.gz",
            );
            pixpin_proyecto::vista::DiscoPc::nuevo(&raiz).ruta(&x.chat, &rel)
        };
        std::fs::create_dir_all(nueva.parent().unwrap()).unwrap();
        std::fs::write(&nueva, r#"{"elements":[{"id":"m2","type":"freedraw","x":5,"y":5,"width":1,"height":1,"points":[{"x":0.0,"y":0.0},{"x":1.0,"y":1.0}]}]}"#).unwrap();
        let hoja_vieja = lector_tinta::ruta_de_hoja(&pdf, 2);
        std::fs::create_dir_all(hoja_vieja.parent().unwrap()).unwrap();
        std::fs::write(
            &hoja_vieja,
            r#"{"elements":[{"id":"pc1","type":"freedraw","x":70,"y":80,"points":[[0,0],[9,9]]}]}"#,
        )
        .unwrap();
        let h = hoja_del_pdf(adjunto.as_ref(), &pdf, 2).unwrap();
        assert_eq!(h, nueva);
        assert_eq!(Capa::leer(&h).escena.cuantos_visibles(), 2);
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&h).unwrap()).unwrap();
        assert_eq!(
            v["elements"][1]["x"].as_f64(),
            Some(70.0),
            "en un PDF las unidades son las mismas"
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_tinta_del_movil_se_corre_su_margen_y_lo_que_no_se_toca_vuelve_a_su_numero_exacto() {
        let r = raiz("corrida");
        let doc = con_adjunto(&r, "libro.epub", b"PK");
        let b = base(&doc);
        an::escribir(&b.fichero(".maqueta"), "384,256,256,100,1,0").unwrap();
        std::fs::create_dir_all(b.tinta().parent().unwrap()).unwrap();
        std::fs::write(
            b.tinta(),
            r#"{"elements":[{"id":"m1","type":"freedraw","x":10.123456789,"y":10278.29,"width":4,"height":4,"seed":1,"version":1,"versionNonce":1,"updated":1,"points":[{"x":0.0,"y":0.0},{"x":4.0,"y":4.0}]}],"files":{}}"#,
        )
        .unwrap();
        let mut capa = leer_capa(&doc);
        let e = capa.escena.visibles().next().unwrap().clone();
        assert!((e.x - (10.123_457 - 256.0)).abs() < 1e-3, "{}", e.x);
        // Un trazo nuevo del PC, en el margen de la izquierda.
        let mut nuevo = e.clone();
        nuevo.x = -100.0;
        capa.escena.anadir(nuevo);
        capa.sucia = true;
        guardar_capa(&doc, &mut capa).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(b.tinta()).unwrap()).unwrap();
        let xs: Vec<f64> = v["elements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["x"].as_f64().unwrap())
            .collect();
        assert_eq!(xs.len(), 2);
        assert!(
            (xs[0] - 10.123456789).abs() < 1e-9,
            "el del movil, intacto: {xs:?}"
        );
        assert_eq!(xs[1], 156.0, "el nuevo, corrido al borde de la pagina");
        // Caso negativo: un documento fuera del almacen, junto a el y sin correr.
        let fuera = r.join("fuera.docx");
        std::fs::write(&fuera, b"PK").unwrap();
        assert!(base_de(&fuera).is_none());
        let mut c = leer_capa(&fuera);
        c.escena.anadir(e);
        c.sucia = true;
        guardar_capa(&fuera, &mut c).unwrap();
        let v: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(lector_tinta::ruta_de_capa(&fuera)).unwrap(),
        )
        .unwrap();
        assert!(v["elements"][0]["x"].as_f64().unwrap() < 0.0, "sin correr");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn un_pdf_del_chat_lee_y_guarda_sus_marcadores_y_espacios_con_el_codigo_del_mensaje() {
        let r = raiz("pdf");
        let pdf = con_adjunto(&r, "plano.pdf", b"%PDF-1.4");
        // Lo de antes: marcas y espacios junto al PDF, y la tinta de la hoja 4.
        lectura::escribir(
            &pdf,
            &Ajustes {
                espacios: 2,
                marcas: "1:0.5:3.61:B".into(),
                pagina: 4.5,
                ..Ajustes::default()
            },
        )
        .unwrap();
        let hoja_vieja = lector_tinta::ruta_de_hoja(&pdf, 3);
        std::fs::create_dir_all(hoja_vieja.parent().unwrap()).unwrap();
        std::fs::write(&hoja_vieja, r#"{"elements":[]}"#).unwrap();
        let a = leer(&pdf);
        let b = base(&pdf);
        assert_eq!(
            an::leer(&b.fichero(".marcas")).as_deref(),
            Some("1:0.5:3.61:B")
        );
        assert_eq!(an::leer(&b.fichero(".espacios")).as_deref(), Some("2"));
        assert_eq!(a.pagina, 4.5);
        // Lo que manda el movil gana.
        an::escribir(&b.fichero(".espacios"), "1").unwrap();
        an::escribir(&b.fichero(".marcas"), "m1:0.5:2.25:E").unwrap();
        let a = leer(&pdf);
        assert_eq!((a.espacios, a.marcas.as_str()), (1, "m1:0.5:2.25:E"));
        escribir(&pdf, &Ajustes { espacios: 3, ..a }).unwrap();
        assert_eq!(an::leer(&b.fichero(".espacios")).as_deref(), Some("3"));
        // Basura en los espacios no pisa lo de aqui.
        an::escribir(&b.fichero(".espacios"), "x").unwrap();
        assert_eq!(leer(&pdf).espacios, 3);
        // La tinta de cada hoja, con el codigo del mensaje y la de antes copiada.
        let adjunto = adjunto_del_pdf(&pdf);
        let nueva = hoja_del_pdf(adjunto.as_ref(), &pdf, 3).unwrap();
        assert!(
            nueva
                .to_string_lossy()
                .ends_with(&format!("{}-p3.excalidraw", b.base)),
            "{nueva:?}"
        );
        assert!(nueva.is_file() && hoja_vieja.is_file());
        assert_eq!(hoja_del_pdf(None, &pdf, 3), None);
        let _ = std::fs::remove_dir_all(&r);
    }

    /// El caso del usuario (Bastidas_CJ_.pdf, sin espacios en el movil): lo
    /// del movil se lee a la escala de la hoja, y lo que el PC dibujo antes
    /// de saberlo (en la hoja a 1400) pasa una vez a las unidades del movil,
    /// para que alli se vea donde se dibujo y aqui no encoja.
    #[test]
    fn la_hoja_de_un_pdf_del_chat_se_lee_en_las_unidades_del_lector_del_movil() {
        use crate::lector_pdf_proyecto::DondeVa;
        use lector_tinta::Unidades;
        let r = raiz("unidades-pdf");
        let pdf = con_adjunto(&r, "tesis.pdf", b"%PDF-1.4");
        let d = DondeVa::de(&r, &pdf, 1);
        assert_eq!(d.unidades(0), Unidades::de_la_capa_del_movil(0));
        assert_eq!(
            d.unidades(3),
            Unidades::DEL_PC,
            "con los dos espacios el movil guarda la hoja tal cual"
        );
        let h = d.para_leer(&pdf, 0);
        std::fs::create_dir_all(h.parent().unwrap()).unwrap();
        let fichero = r#"{"elements":[
            {"id":"jDNyxAFkaxm2qZ-KsluKG","type":"freedraw","x":-800,"y":1000,"width":250,"height":0,"strokeWidth":1,"version":1,"points":[{"x":0.0,"y":0.0},{"x":250.0,"y":0.0}]},
            {"id":"pc1f","type":"freedraw","x":100,"y":400,"width":100,"height":0,"strokeWidth":1,"version":2,"points":[[0,0],[100,0]]}]}"#;
        std::fs::write(&h, fichero).unwrap();
        let caja = |c: &Capa, n: usize| c.escena.visibles().nth(n).unwrap().caja();
        let c = d.leer_capa(&pdf, 0, 0, 1980.0);
        let (movil, pc) = (caja(&c, 0), caja(&c, 1));
        assert!(
            (movil.0 - 100.0).abs() < 3.0 && (movil.2 - movil.0 - 100.0).abs() < 5.0,
            "{movil:?}"
        );
        assert!(
            (pc.0 - 100.0).abs() < 3.0 && (pc.2 - pc.0 - 100.0).abs() < 5.0,
            "lo del PC donde se dibujo: {pc:?}"
        );
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&h).unwrap()).unwrap();
        assert_eq!(
            v["elements"][0]["x"].as_f64(),
            Some(-800.0),
            "lo del movil no se toca"
        );
        let x = v["elements"][1]["x"].as_f64().unwrap();
        assert!(
            (x + 800.0).abs() < 1.0,
            "lo del PC, en las unidades del movil: {x}"
        );
        assert!(
            v["elements"][1]["version"].as_u64() > Some(2),
            "y como un cambio, para que el movil lo tome"
        );
        // Una sola vez: abrirlo otra vez no lo vuelve a encoger.
        let otra = d.leer_capa(&pdf, 0, 0, 1980.0);
        assert!(
            (caja(&otra, 1).0 - 100.0).abs() < 3.0,
            "{:?}",
            caja(&otra, 1)
        );
        // Caso negativo: compartir solo lee, no escribe nada.
        let h2 = d.para_leer(&pdf, 1);
        std::fs::write(&h2, fichero).unwrap();
        let _ = DondeVa::solo_leer(&pdf).leer_capa(&pdf, 1, 0, 1980.0);
        assert_eq!(std::fs::read_to_string(&h2).unwrap(), fichero);
        // Caso negativo: un PDF que no es del chat sigue en las unidades del PC.
        let suelto = r.join("suelto.pdf");
        std::fs::write(&suelto, b"%PDF-1.4").unwrap();
        assert_eq!(DondeVa::de(&r, &suelto, 1).unidades(0), Unidades::DEL_PC);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn un_id_nacido_en_el_pc_se_reconoce_y_uno_del_movil_no() {
        assert!(
            nacido_en_el_pc("pc1f")
                && nacido_en_el_pc("pcc")
                && nacido_en_el_pc("pc0123456789abcdef")
        );
        assert!(!nacido_en_el_pc("jDNyxAFkaxm2qZ-KsluKG"));
        assert!(
            !nacido_en_el_pc("pcZ")
                && !nacido_en_el_pc("pc")
                && !nacido_en_el_pc("pc0123456789abcdef0")
        );
        assert!(!nacido_en_el_pc("PC1"));
    }

    #[test]
    fn un_pdf_que_no_es_del_chat_no_crea_nada_que_viaje() {
        let r = raiz("pdf-fuera");
        let del_chat = con_adjunto(&r, "otro.pdf", b"%PDF");
        let carpeta = del_chat.parent().unwrap().parent().unwrap().to_path_buf();
        // Dentro de la carpeta del chat pero sin mensaje que lo senale.
        let pdf = carpeta.join("archivos/suelto.pdf");
        std::fs::write(&pdf, b"%PDF").unwrap();
        escribir(
            &pdf,
            &Ajustes {
                espacios: 2,
                marcas: "1:0:0:E".into(),
                ..Ajustes::default()
            },
        )
        .unwrap();
        assert_eq!(leer(&pdf).espacios, 2);
        assert!(adjunto_del_pdf(&pdf).is_none());
        assert!(!carpeta.join("android").exists(), "nada con codigo");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn los_marcadores_de_un_lienzo_del_chat_pasan_junto_a_su_dibujo_una_vez() {
        let r = raiz("lienzo");
        let doc = con_adjunto(&r, "x.docx", b"PK");
        let carpeta = doc.parent().unwrap().parent().unwrap().to_path_buf();
        let lienzo = carpeta.join("lienzos/d9.excalidraw");
        std::fs::create_dir_all(lienzo.parent().unwrap()).unwrap();
        let vieja = carpeta.join("lienzos/d9.excalidraw.pixpin-marcas");
        std::fs::write(&vieja, "k:10.0:20.0:H").unwrap();
        let nueva = marcas_del_lienzo(&lienzo, vieja.clone());
        assert_eq!(nueva, carpeta.join("android/pins/draw/d9.marcas"));
        assert_eq!(an::leer(&nueva).as_deref(), Some("k:10.0:20.0:H"));
        std::fs::write(&vieja, "otra").unwrap();
        marcas_del_lienzo(&lienzo, vieja.clone());
        assert_eq!(
            an::leer(&nueva).as_deref(),
            Some("k:10.0:20.0:H"),
            "no se pisa"
        );
        // Caso negativo: fuera del almacen, el de siempre.
        let fuera = r.join("d.excalidraw");
        assert_eq!(
            marcas_del_lienzo(&fuera, r.join("d.excalidraw.pixpin-marcas")),
            r.join("d.excalidraw.pixpin-marcas")
        );
        let _ = std::fs::remove_dir_all(&r);
    }
}

/// **Con una copia de los datos del usuario** (nunca los de verdad): lo que
/// mando el movil v0.96 se ve en los lectores del PC. Se lanza a mano:
/// `PIXPIN_DATOS_COPIA=<copia> cargo test -p pixpin --bin pixpinmax con_los_datos_copiados -- --ignored --nocapture`.
#[cfg(test)]
mod con_datos_reales {
    use super::*;

    #[test]
    #[ignore = "necesita una copia de los datos del usuario"]
    fn con_los_datos_copiados_lo_del_movil_se_ve_en_los_lectores() {
        let raiz = PathBuf::from(std::env::var("PIXPIN_DATOS_COPIA").expect("PIXPIN_DATOS_COPIA"));
        let carpeta = raiz.join("proyectos/N5T7C46GZE/archivos");
        let docx = carpeta.join("Bibliografia_Papers_Tesis_Max.docx");
        let a = leer(&docx);
        let capa = leer_capa(&docx);
        let xs: Vec<f32> = capa.escena.visibles().map(|e| e.x).collect();
        println!("docx: base {:?}", base_de(&docx).map(|b| b.base));
        println!(
            "docx: columna {} tamano {} sitio {} marcadores {}",
            a.columna,
            a.tamano,
            a.sitio,
            a.marcadores.len()
        );
        println!("docx: {} trazos, x {:?}", xs.len(), xs);
        let pdf = carpeta.join("Transcripcion_Voz_260904_201352.pdf");
        let a = leer(&pdf);
        println!(
            "pdf: base {:?} espacios {} marcas {:?}",
            base_de(&pdf).map(|b| b.base),
            a.espacios,
            a.marcas
        );
        let adj = adjunto_del_pdf(&pdf);
        for i in 0..7 {
            let h = hoja_del_pdf(adj.as_ref(), &pdf, i).unwrap();
            if h.is_file() {
                println!(
                    "pdf hoja {i}: {} trazos ({})",
                    Capa::leer(&h).escena.cuantos_visibles(),
                    h.display()
                );
            }
        }
        let lienzo =
            raiz.join("proyectos/pr-1788574477233/lienzos/hoja-pr-1788574477233-0.excalidraw");
        let m = marcas_del_lienzo(&lienzo, lienzo.with_extension("excalidraw.pixpin-marcas"));
        println!(
            "lienzo: {} -> {:?}",
            m.display(),
            std::fs::read_to_string(&m).ok()
        );
        assert_eq!(base_de(&docx).unwrap().base, "anot-9KKUS3XF5W");
        assert_eq!(base_de(&pdf).unwrap().base, "anot-34FJGS8TT2");
    }
}

/// **El marco de la tinta** (`anot-….hoja`, 29-sep): con el, la tinta cae
/// sobre la hoja en cualquier escala en que la escribiera el otro aparato.
#[cfg(test)]
mod pruebas_del_marco {
    use super::pruebas::{con_adjunto, raiz};
    use super::*;
    use crate::lector_pdf_proyecto::DondeVa;
    use pixpin_motor2d::elemento::Figura;
    use pixpin_sincro::anotado::HOJA;

    /// Los puntos de cada trazo vivo, en unidades del lector.
    fn puntos(c: &Capa) -> Vec<Vec<(f32, f32)>> {
        c.escena
            .visibles()
            .map(|e| match &e.figura {
                Figura::Lapiz { puntos, .. } => puntos.iter().map(|p| (p.x, p.y)).collect(),
                otra => panic!("{otra:?}"),
            })
            .collect()
    }

    fn cerca(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01
    }

    /// Una hoja de 1400 x 1980 escrita por otro aparato con el cero en
    /// (500, 300) y tres veces mas grande: un trazo en las esquinas de la
    /// hoja y otro de (100, 200) a (400, 200), de grosor 6.
    const TINTA_A_OTRA_ESCALA: &str = r#"{"elements":[
        {"id":"esquinas","type":"freedraw","x":500,"y":300,"width":4200,"height":5940,"strokeWidth":3,"version":1,"points":[{"x":0.0,"y":0.0},{"x":4200.0,"y":5940.0}]},
        {"id":"raya","type":"freedraw","x":800,"y":900,"width":900,"height":0,"strokeWidth":6,"version":1,"points":[{"x":0.0,"y":0.0},{"x":900.0,"y":0.0}]}]}"#;
    /// Su marco, como lo escribiria Kotlin (`1400.0`): se conserva tal cual.
    const SU_MARCO: &str = "500.0,300.0,4700.0,6240.0\nv1\n";

    fn pdf_con_tinta(
        etiqueta: &str,
        marco: Option<&str>,
    ) -> (PathBuf, PathBuf, DondeVa, PathBuf, PathBuf) {
        let r = raiz(etiqueta);
        let pdf = con_adjunto(&r, "plano.pdf", b"%PDF-1.4");
        let d = DondeVa::de(&r, &pdf, 1);
        let tinta = d.para_escribir(&pdf, 0);
        std::fs::create_dir_all(tinta.parent().unwrap()).unwrap();
        std::fs::write(&tinta, TINTA_A_OTRA_ESCALA).unwrap();
        let (raiz_almacen, x) = adjunto_del_pdf(&pdf).unwrap();
        let f = base_de_la_hoja_del_pdf(Some(&(raiz_almacen, x)), 0)
            .unwrap()
            .marco();
        if let Some(m) = marco {
            an::escribir(&f, m).unwrap();
        }
        (r, pdf, d, tinta, f)
    }

    /// La huella de la tinta de la hoja 0 del PDF del chat, como ahora.
    fn huella_de_la_hoja(pdf: &Path) -> String {
        base_de_la_hoja_del_pdf(adjunto_del_pdf(pdf).as_ref(), 0)
            .unwrap()
            .huella()
            .unwrap()
    }

    #[test]
    fn un_marco_con_la_huella_de_otra_tinta_se_ignora_y_con_la_suya_manda() {
        // Con la huella de su tinta, el marco manda: la esquina de la hoja.
        let (r, pdf, d, tinta, f) = pdf_con_tinta("marco-huella", None);
        let h = huella_de_la_hoja(&pdf);
        an::escribir(&f, &format!("500.0,300.0,4700.0,6240.0\nv1\ntinta {h}\n")).unwrap();
        assert!(cerca(
            puntos(&d.leer_capa(&pdf, 0, 0, 1980.0))[0][0],
            (0.0, 0.0)
        ));
        // El movil de hoy reescribe la tinta (otro trazo) sin tocar el
        // marco: ya no es la suya, se lee con la regla vieja (x2,5, -1050).
        let reescrita = TINTA_A_OTRA_ESCALA.replace("\"raya\"", "\"raya2\"");
        std::fs::write(&tinta, &reescrita).unwrap();
        let p = puntos(&d.leer_capa(&pdf, 0, 0, 1980.0));
        assert!(cerca(p[1][0], (1850.0 / 2.5, 900.0 / 2.5)), "{:?}", p[1]);
        // Y como el marco mentia, el lector le apunta el de la regla vieja,
        // en las dos lineas de Android (lo que hace `tintaDeLaHoja`).
        assert_eq!(an::leer(&f).as_deref(), Some("-1050,0,2450,4950\nv1\n"));
        // Y una huella con una cifra cambiada, igual (caso negativo).
        let otra: String = h.chars().rev().collect();
        an::escribir(&f, &format!("500,300,4700,6240\nv1\ntinta {otra}\n")).unwrap();
        std::fs::write(&tinta, TINTA_A_OTRA_ESCALA).unwrap();
        assert_ne!(otra, h);
        let p = puntos(&d.leer_capa(&pdf, 0, 0, 1980.0));
        assert!(cerca(p[1][0], (1850.0 / 2.5, 900.0 / 2.5)), "{:?}", p[1]);
        // Sin huella (el formato de Android) manda.
        an::escribir(&f, SU_MARCO).unwrap();
        assert!(cerca(
            puntos(&d.leer_capa(&pdf, 0, 0, 1980.0))[0][0],
            (0.0, 0.0)
        ));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_tinta_escrita_con_su_marco_en_otra_escala_cae_exactamente_sobre_la_hoja() {
        let (r, pdf, d, _, f) = pdf_con_tinta("marco-escala", Some(SU_MARCO));
        assert_eq!(
            f.file_name().unwrap().to_string_lossy(),
            format!("{}-p0.hoja", base_de(&pdf).unwrap().base)
        );
        // Sin espacios la regla vieja lo leeria dos veces y media: el marco manda.
        let c = d.leer_capa(&pdf, 0, 0, 1980.0);
        let p = puntos(&c);
        assert!(
            cerca(p[0][0], (0.0, 0.0)) && cerca(p[0][1], (1400.0, 1980.0)),
            "{:?}",
            p[0]
        );
        assert!(
            cerca(p[1][0], (100.0, 200.0)) && cerca(p[1][1], (400.0, 200.0)),
            "{:?}",
            p[1]
        );
        let raya = c.escena.visibles().nth(1).unwrap();
        assert!(
            (raya.grosor - 2.0).abs() < 1e-4,
            "el grosor, a la misma escala: {}",
            raya.grosor
        );
        // Con cualquier espacio puesto, lo mismo: ya no depende de ellos.
        assert_eq!(puntos(&d.leer_capa(&pdf, 0, 3, 1980.0)), p);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn con_otra_proporcion_se_encaja_eje_por_eje_al_marco() {
        // El otro aparato midio la hoja un 10 % mas alta: la esquina de
        // abajo cae igual en la esquina de abajo de la hoja de aqui.
        let (r, pdf, d, _, _) = pdf_con_tinta("marco-proporcion", Some("500,300,4700,5940\n"));
        let p = puntos(&d.leer_capa(&pdf, 0, 0, 1800.0));
        assert!(cerca(p[0][0], (0.0, 0.0)), "{:?}", p[0]);
        assert!((p[0][1].0 - 1400.0).abs() < 0.01, "{:?}", p[0]);
        assert!(
            (p[0][1].1 - 1800.0 * 5940.0 / 5640.0).abs() < 0.05,
            "{:?}",
            p[0]
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn ida_y_vuelta_sin_cambios_no_escribe_y_al_guardar_la_tinta_pasa_a_la_hoja_como_en_android() {
        let (r, pdf, d, tinta, f) = pdf_con_tinta("marco-ida-y-vuelta", Some(SU_MARCO));
        let (antes_t, antes_m) = (std::fs::read(&tinta).unwrap(), std::fs::read(&f).unwrap());
        let mut c = d.leer_capa(&pdf, 0, 0, 1980.0);
        d.guardar_capa(&pdf, 0, &mut c, 1980.0).unwrap();
        assert_eq!(
            std::fs::read(&tinta).unwrap(),
            antes_t,
            "sin cambios no se escribe"
        );
        assert_eq!(std::fs::read(&f).unwrap(), antes_m, "ni el marco");
        // Guardada, como Android v0.98.0: la tinta en las unidades de la
        // hoja (0 a 1400) y el marco que lo dice.
        c.sucia = true;
        d.guardar_capa(&pdf, 0, &mut c, 1980.0).unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "0,0,1400,1980\nv1\n");
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&tinta).unwrap()).unwrap();
        let x = |v: &serde_json::Value| v.as_f64().unwrap();
        assert!(
            (x(&v["elements"][1]["x"]) - 100.0).abs() < 1e-3,
            "{}",
            v["elements"][1]
        );
        assert!((x(&v["elements"][1]["y"]) - 200.0).abs() < 1e-3);
        assert!((x(&v["elements"][1]["strokeWidth"]) - 2.0).abs() < 1e-4);
        let fin = &v["elements"][0]["points"][1];
        assert!(
            (fin["x"].as_f64().or(fin[0].as_f64()).unwrap() - 1400.0).abs() < 1e-3,
            "{fin}"
        );
        // Otra vez igual: ya no se toca nada.
        let fecha = std::fs::metadata(&f).unwrap().modified().unwrap();
        let mut c = d.leer_capa(&pdf, 0, 0, 1980.0);
        assert!(cerca(puntos(&c)[1][0], (100.0, 200.0)), "{:?}", puntos(&c));
        c.sucia = true;
        d.guardar_capa(&pdf, 0, &mut c, 1980.0).unwrap();
        assert_eq!(std::fs::metadata(&f).unwrap().modified().unwrap(), fecha);
        let _ = std::fs::remove_dir_all(&r);
    }

    /// Lo que escribe Android v0.98.0 (`LectorPdfActivity`): la tinta de la
    /// hoja en sus unidades, 0 a 1400, con `deHoja` al lado. Se lee sin
    /// escala y con cualquier espacio puesto en el PC.
    #[test]
    fn la_tinta_nueva_del_movil_con_su_marco_de_hoja_se_lee_tal_cual_con_cualquier_espacio() {
        let (r, pdf, d, tinta, f) =
            pdf_con_tinta("marco-android-nuevo", Some("0,0,1400,1979.899\nv1\n"));
        std::fs::write(
            &tinta,
            r#"{"elements":[{"id":"aNdRoId98xxxxxxxxxxxx","type":"freedraw","x":100,"y":400,"width":300,"height":0,"strokeWidth":2,"version":3,"points":[{"x":0.0,"y":0.0},{"x":300.0,"y":0.0}]}]}"#,
        )
        .unwrap();
        for espacios in 0..4u8 {
            let c = d.leer_capa(&pdf, 0, espacios, 1980.0);
            let p = puntos(&c);
            assert!(
                (p[0][0].0 - 100.0).abs() < 0.01 && (p[0][1].0 - 400.0).abs() < 0.01,
                "{espacios}: {p:?}"
            );
            assert!((p[0][0].1 - 400.0).abs() < 0.1, "{espacios}: {p:?}");
            assert!((c.escena.visibles().next().unwrap().grosor - 2.0).abs() < 1e-3);
        }
        assert_eq!(
            an::leer(&f).as_deref(),
            Some("0,0,1400,1979.899\nv1\n"),
            "leer no toca su marco"
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn sin_marco_se_lee_con_la_regla_de_antes_y_se_le_apunta_su_marco_sin_tocarla() {
        let (r, pdf, d, tinta, f) = pdf_con_tinta("marco-sin", None);
        let antes = std::fs::read(&tinta).unwrap();
        // La regla vieja, sin espacios (`capa_del_movil(0)`): x2,5 y -1050.
        let mut c = d.leer_capa(&pdf, 0, 0, 1980.0);
        let p = puntos(&c);
        assert!(cerca(p[1][0], (1850.0 / 2.5, 900.0 / 2.5)), "{:?}", p[1]);
        // Como Android (`migrar`): su marco al lado, y la tinta intacta.
        assert_eq!(an::leer(&f).as_deref(), Some("-1050,0,2450,4950\nv1\n"));
        assert_eq!(
            std::fs::read(&tinta).unwrap(),
            antes,
            "la tinta, byte a byte"
        );
        // Ya no depende de los espacios: con los dos, en el mismo sitio.
        assert_eq!(puntos(&d.leer_capa(&pdf, 0, 3, 1980.0)), p);
        // Guardada, a la hoja.
        c.sucia = true;
        d.guardar_capa(&pdf, 0, &mut c, 1980.0).unwrap();
        assert_eq!(an::leer(&f).as_deref(), Some("0,0,1400,1980\nv1\n"));
        assert_eq!(puntos(&d.leer_capa(&pdf, 0, 1, 1980.0)), p);
        // Caso negativo: quien solo lee (compartir) no escribe ningun marco.
        let (r2, pdf2, _, _, f2) = pdf_con_tinta("marco-sin-solo-leer", None);
        let _ = DondeVa::solo_leer(&pdf2).leer_capa(&pdf2, 0, 0, 1980.0);
        assert!(!f2.exists());
        let _ = std::fs::remove_dir_all(&r);
        let _ = std::fs::remove_dir_all(&r2);
    }

    /// `fijarLoViejo` de Android: antes de cambiar los espacios, lo que no
    /// tiene marco lo recibe con los de ahora; asi la tinta no se corre.
    #[test]
    fn antes_de_cambiar_los_espacios_la_tinta_sin_marco_recibe_el_suyo() {
        let (r, pdf, d, tinta, f) = pdf_con_tinta("marco-fijar", None);
        let antes = std::fs::read(&tinta).unwrap();
        assert_eq!(d.fijar_lo_viejo(&pdf, 2, &[1980.0, 1980.0]), 1);
        let k = 2.5 / 1.75;
        let m = MarcoDeLaHoja::de_texto(&an::leer(&f).unwrap()).unwrap();
        assert!(
            m.casi_igual(&MarcoDeLaHoja::nuevo(
                -1050.0,
                0.0,
                -1050.0 + 1400.0 * k,
                1980.0 * k
            )),
            "{m:?}"
        );
        assert_eq!(std::fs::read(&tinta).unwrap(), antes);
        // Otra vez, nada: ya lo tiene.
        assert_eq!(d.fijar_lo_viejo(&pdf, 0, &[1980.0]), 0);
        assert!(
            MarcoDeLaHoja::de_texto(&an::leer(&f).unwrap())
                .unwrap()
                .casi_igual(&m)
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn un_marco_mal_formado_se_ignora_y_se_lee_con_la_regla_de_antes() {
        for malo in [
            "500,300,4700",
            "a,b,c,d",
            "0,0,0,0",
            "500,300,4700,6240\nv9\n",
            "",
        ] {
            let (r, pdf, d, _, f) = pdf_con_tinta("marco-malo", Some(malo));
            let p = puntos(&d.leer_capa(&pdf, 0, 0, 1980.0));
            assert!(
                cerca(p[1][0], (1850.0 / 2.5, 900.0 / 2.5)),
                "{malo:?}: {:?}",
                p[1]
            );
            assert_eq!(an::leer(&f).as_deref(), Some(malo), "leer no lo toca");
            let _ = std::fs::remove_dir_all(&r);
        }
    }

    #[test]
    fn el_pdf_suelto_no_tiene_marco() {
        // Caso negativo: lo de junto al PDF solo lo lee el PC, no viaja.
        let r = raiz("marco-suelto");
        let suelto = r.join("suelto.pdf");
        std::fs::write(&suelto, b"%PDF-1.4").unwrap();
        let d = DondeVa::de(&r, &suelto, 1);
        assert_eq!(d.unidades_de(0, 0, 1980.0), Unidades::DEL_PC);
        let mut c = d.leer_capa(&suelto, 0, 0, 1980.0);
        c.sucia = true;
        d.guardar_capa(&suelto, 0, &mut c, 1980.0).unwrap();
        let nombres: Vec<String> = std::fs::read_dir(lector_tinta::carpeta_de(&suelto))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(nombres.iter().all(|n| !n.ends_with(".hoja")), "{nombres:?}");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_tinta_de_un_word_con_su_marco_cae_en_la_columna_aunque_la_maqueta_diga_otro_margen() {
        let r = raiz("marco-word");
        let doc = con_adjunto(&r, "acta.docx", b"PK");
        let b = base_de(&doc).unwrap();
        // La maqueta dice columna 420 con 280 de margen; el marco, que la
        // columna esta a 560 y al doble (otra escala de la capa).
        an::escribir(&b.fichero(".maqueta"), "420,280,280,100,1,0").unwrap();
        an::escribir(&b.fichero(HOJA), "560,0,1400,840\nv1\n").unwrap();
        std::fs::create_dir_all(b.tinta().parent().unwrap()).unwrap();
        let tinta = r#"{"elements":[{"id":"w1","type":"freedraw","x":580,"y":100,"width":200,"height":0,"strokeWidth":2,"version":1,"points":[{"x":0.0,"y":0.0},{"x":200.0,"y":0.0}]}],"files":{}}"#;
        std::fs::write(b.tinta(), tinta).unwrap();
        let mut c = leer_capa(&doc);
        let p = puntos(&c);
        assert!(
            cerca(p[0][0], (10.0, 50.0)) && cerca(p[0][1], (110.0, 50.0)),
            "{p:?}"
        );
        // Ida y vuelta: sin cambios, nada; guardada, el marco del otro se queda.
        guardar_capa(&doc, &mut c).unwrap();
        assert_eq!(std::fs::read_to_string(b.tinta()).unwrap(), tinta);
        c.sucia = true;
        guardar_capa(&doc, &mut c).unwrap();
        assert_eq!(
            an::leer(&b.fichero(HOJA)).as_deref(),
            Some("560,0,1400,840\nv1\n")
        );
        // Uno con la huella del PC del 29-sep que dice lo mismo: se le quita.
        let h = b.huella().unwrap();
        an::escribir(
            &b.fichero(HOJA),
            &format!("560,0,1400,840\nv1\ntinta {h}\n"),
        )
        .unwrap();
        let mut c = leer_capa(&doc);
        c.sucia = true;
        guardar_capa(&doc, &mut c).unwrap();
        assert_eq!(
            an::leer(&b.fichero(HOJA)).as_deref(),
            Some("560,0,1400,840\nv1\n")
        );
        // Caso negativo: sin marco, la regla de antes (la columna corrida `izq`).
        std::fs::remove_file(b.fichero(HOJA)).unwrap();
        std::fs::write(b.tinta(), tinta).unwrap();
        let mut c = leer_capa(&doc);
        assert!(cerca(puntos(&c)[0][0], (300.0, 100.0)), "{:?}", puntos(&c));
        // Y al guardar un cambio escribe el suyo: la columna a 280, a escala 1.
        c.sucia = true;
        guardar_capa(&doc, &mut c).unwrap();
        assert_eq!(
            an::leer(&b.fichero(HOJA)).as_deref(),
            Some("280,0,700,420\nv1\n")
        );
        let _ = std::fs::remove_dir_all(&r);
    }
}
