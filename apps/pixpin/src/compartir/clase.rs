//! **Compartir una grabacion** (27-oct-2026, el usuario: «en las
//! grabaciones, que el menu de compartir sea el de toda la app, con los
//! mismos formatos; sobre todo el HTML, que se vea claro e intuitivo»).
//!
//! Una nota de voz sola (una clase, una reunion o una nota cualquiera) entra
//! en la hoja de compartir de siempre con lo de siempre —sus paginas para el
//! PDF y las imagenes, el original y el `.pixpin`— y con lo suyo:
//!
//! - **HTML**: la pagina de [`pixpin_proyecto::clase_exportar::html`], con
//!   el reproductor, la transcripcion que salta y se resalta, las capturas
//!   en su minuto y los marcadores. El audio va dentro hasta
//!   [`AUDIO_DENTRO`]; mas grande, al lado, como el video (siempre al lado:
//!   pesa demasiado para base64).
//! - **PDF** (y PNG, JPG, SVG de una pagina): el titulo, los marcadores y
//!   la transcripcion con las capturas en su minuto, en A4.
//! - **Word**, **Markdown** (con la carpeta de las capturas, en un ZIP) y
//!   **Texto** (la transcripcion con sus minutos).
//! - **Anki**: el fichero para «Archivo → Importar», una tarjeta por
//!   captura, como el «Enviar a Anki» del menu.
//! - **Originales**: el audio, el video, las capturas y lo adjunto.
//!
//! Lo que se lee de la grabacion es [`ClaseParaExportar`], que hoy sale de
//! la nota y sus enlaces y mañana puede salir de la carpeta de la grabacion.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use pixpin_motor2d::ColorRgba;
use pixpin_motor2d::exportar as ex;
use pixpin_motor2d::pintado::Orden;
use pixpin_proyecto::clase_exportar::{self as ce, ClaseParaExportar, Fila, Medio, Rotulos};
use pixpin_proyecto::cuaderno::Mensaje;
use pixpin_store::Catalogo;

use super::{
    ALTO_A4, ANCHO_A4, BLANCO, Entero, Extra, FuenteImagen, MARGEN_A4, Preparado, TINTA_SUAVE,
    TINTA_TEXTO, escribir, hoja_en_blanco, nombre_de_fichero, pagina,
};

/// El audio mas grande que va dentro de la pagina (en base64 pesa un
/// tercio mas, y un navegador abre bien hasta unas decenas de megas).
pub(crate) const AUDIO_DENTRO: u64 = 30 * 1024 * 1024;
/// Una captura que pesa mas que esto se vuelve a guardar en JPEG para la
/// pagina: cuarenta capturas de pantalla en PNG son cien megas.
const CAPTURA_LIGERA: u64 = 400 * 1024;
/// El lado mas largo de una captura dentro de la pagina.
const LADO_CAPTURA: u32 = 2000;

/// **Una grabacion lista para compartir**: lo leido y lo que dice la pagina.
pub(crate) struct Grabacion {
    pub datos: ClaseParaExportar,
    pub rotulos: Rotulos,
    /// El mazo de Anki (`PixPin::<proyecto>`) y la pregunta de sus tarjetas.
    mazo: String,
    pregunta: String,
}

/// La fecha de la grabacion en el idioma de la app.
fn fecha(cuando_utc: i64, t: &Catalogo) -> String {
    if cuando_utc <= 0 {
        return String::new();
    }
    let local = pixpin_shell::entorno::a_local(cuando_utc);
    let (ano, mes, dia) = crate::automatizaciones::fecha_civil(local.div_euclid(86_400_000));
    let min = local.rem_euclid(86_400_000) / 60_000;
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("dia", format!("{dia:02}"));
    args.set("mes", format!("{mes:02}"));
    args.set("ano", ano.to_string());
    args.set("hora", format!("{:02}:{:02}", min / 60, min % 60));
    t.t_args("grabacion-fecha", &args)
}

pub(crate) fn rotulos(t: &Catalogo) -> Rotulos {
    Rotulos {
        idioma: t.t("grabacion-idioma"),
        transcripcion: t.t("grabacion-transcripcion"),
        capturas: t.t("grabacion-capturas"),
        captura: t.t("grabacion-captura"),
        marcadores: t.t("grabacion-marcadores"),
        marcador: t.t("grabacion-marcador"),
        adjuntos: t.t("compartir-adjuntos"),
        duracion: t.t("grabacion-duracion"),
        sin_texto: t.t("grabacion-sin-texto"),
        buscar: t.t("grabacion-buscar"),
        seguir: t.t("grabacion-seguir"),
        reproducir: t.t("grabacion-reproducir"),
        velocidad: t.t("grabacion-velocidad"),
        ir_al_minuto: t.t("grabacion-ir-al-minuto"),
        cerrar: t.t("grabacion-cerrar"),
        audio: t.t("grabacion-audio"),
        video: t.t("grabacion-video"),
    }
}

/// Los trozos de la transcripcion, como los lee la pantalla de la letra.
fn lineas_de(voz: &Mensaje) -> Vec<(Option<i64>, String)> {
    crate::voz::transcripcion_de(voz)
        .map(crate::voz::trozos)
        .unwrap_or_default()
        .into_iter()
        .map(|t| (t.ms, crate::voz::sin_marcas(&t.texto)))
        .filter(|(_, t)| !t.is_empty())
        .collect()
}

/// **Lee la grabacion de la nota `voz`** del proyecto: el chat entero, para
/// encontrar sus capturas y su video. `None` si no es una nota de voz.
pub(crate) fn leer(raiz: &Path, proyecto: &str, voz: &Mensaje, t: &Catalogo) -> Option<Grabacion> {
    if !ce::es_grabacion(voz) {
        return None;
    }
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, proyecto);
    let mensajes = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta)
        .map(|c| c.mensajes)
        .unwrap_or_default();
    // La nota como esta en el disco (la del chat puede venir sin lo ultimo).
    let voz = mensajes.iter().find(|m| m.id == voz.id).unwrap_or(voz);
    let datos = ClaseParaExportar::de_la_nota(
        voz,
        &mensajes,
        lineas_de(voz),
        fecha(voz.cuando, t),
        &|m: &Mensaje| super::ruta_de(raiz, proyecto, m),
    );
    let nombre = pixpin_proyecto::almacen::Indice::leer(raiz)
        .buscar(proyecto)
        .map(|f| f.nombre.clone())
        .unwrap_or_default();
    Some(Grabacion {
        datos,
        rotulos: rotulos(t),
        mazo: format!("PixPin::{}", nombre.trim().replace("::", ":")),
        pregunta: t.t("anki-pregunta-clase"),
    })
}

// ---------------------------------------------------------------------------
// Preparar

/// **Pone la grabacion en la hoja**: sus paginas, sus formatos propios y,
/// en `p.clase`, lo que hace su pagina web. Devuelve los originales.
pub(crate) fn anadir(p: &mut Preparado, g: Grabacion, t: &Catalogo) -> Vec<PathBuf> {
    if !g.datos.titulo.trim().is_empty() {
        p.titulo = g.datos.titulo.clone();
    }
    let hojas = hojas_de(p, &g.datos, &g.rotulos);
    for (i, h) in hojas.into_iter().enumerate() {
        let (clave, nombre, detalle, nivel) = if i == 0 {
            (
                "grabacion".to_string(),
                p.titulo.clone(),
                t.t("compartir-tipo-grabacion"),
                0,
            )
        } else {
            (
                format!("grabacion|p{}", i + 1),
                format!("{} {}", t.t("compartir-pagina"), i + 1),
                String::new(),
                1,
            )
        };
        p.pieza(pagina(&clave, nombre, detalle, nivel), h, BLANCO, false);
        p.marcar(&clave);
    }
    let g = Arc::new(g);
    let base = nombre_de_fichero(&p.titulo);
    let propio = |id: &'static str, clave: &'static str, hacer: super::exportable::Hacer| Extra {
        id,
        clave,
        que: Entero::Funcion(hacer),
    };
    let (g1, b1) = (g.clone(), base.clone());
    p.extras.push(propio(
        "word",
        "compartir-word",
        Arc::new(move |c: &Path| word(&g1, &b1, c)),
    ));
    let (g2, b2) = (g.clone(), base.clone());
    p.extras.push(propio(
        "md",
        "compartir-md",
        Arc::new(move |c: &Path| markdown(&g2, &b2, c)),
    ));
    let (g3, b3) = (g.clone(), base.clone());
    p.extras.push(propio(
        "texto",
        "compartir-texto",
        Arc::new(move |c: &Path| {
            let r = c.join(format!("{b3}.txt"));
            escribir(&r, ce::texto(&g3.datos, &g3.rotulos).as_bytes())?;
            Ok(vec![r])
        }),
    ));
    if !g.datos.capturas.is_empty() {
        let (g4, b4) = (g.clone(), base.clone());
        p.extras.push(propio(
            "anki",
            "compartir-anki",
            Arc::new(move |c: &Path| anki(&g4, &b4, c)),
        ));
    }
    let originales = g.datos.ficheros();
    p.clase = Some(g);
    originales
}

// ---------------------------------------------------------------------------
// Las paginas (PDF, PNG, JPG, SVG)

const ACENTO: ColorRgba = ColorRgba::opaco(0.18, 0.44, 0.87);
const MARCA: ColorRgba = ColorRgba::opaco(0.72, 0.43, 0.0);
const LETRA: f32 = 12.0;
const LETRA_TITULO: f32 = 22.0;
const LETRA_SECCION: f32 = 13.0;
/// Lo que se aparta el texto de su minuto.
const SANGRIA: f32 = 58.0;
/// Lo mas alta que sale una captura en el A4.
const ALTO_CAPTURA: f32 = 430.0;

fn texto(texto: String, x: f32, y: f32, tam: f32, color: ColorRgba, negrita: bool) -> Orden {
    Orden::Texto {
        texto,
        x,
        y,
        tam,
        familia: "Segoe UI".into(),
        color,
        ancho_max: (ANCHO_A4 - x - MARGEN_A4) * 1.5,
        negrita,
        cursiva: false,
    }
}

/// Las hojas A4 que se van llenando de arriba abajo.
struct Paginas {
    hojas: Vec<pixpin_motor2d::exportar::Hoja>,
    ordenes: Vec<Orden>,
    y: f32,
}

impl Paginas {
    fn cabe(&mut self, alto: f32) {
        if self.y + alto > ALTO_A4 - MARGEN_A4 && !self.ordenes.is_empty() {
            self.hojas.push(hoja_en_blanco(
                (0.0, 0.0, ANCHO_A4, ALTO_A4),
                std::mem::take(&mut self.ordenes),
            ));
            self.y = MARGEN_A4;
        }
    }

    /// Un parrafo partido en renglones, con su minuto delante si lo lleva.
    fn parrafo(&mut self, minuto: Option<(String, ColorRgba)>, cuerpo: &str, color: ColorRgba) {
        let x = MARGEN_A4 + if minuto.is_some() { SANGRIA } else { 0.0 };
        let ancho = ANCHO_A4 - MARGEN_A4 - x;
        let alto = LETRA * ex::INTERLINEA;
        let renglones = ex::partir_texto(cuerpo, LETRA, ancho);
        // El minuto no se queda solo al pie de una hoja.
        self.cabe(alto * renglones.len().clamp(1, 2) as f32);
        if let Some((m, c)) = minuto {
            self.ordenes
                .push(texto(m, MARGEN_A4, self.y, LETRA, c, true));
        }
        for r in renglones {
            self.cabe(alto);
            if !r.trim().is_empty() {
                self.ordenes.push(texto(r, x, self.y, LETRA, color, false));
            }
            self.y += alto;
        }
        self.y += LETRA * 0.5;
    }

    fn seccion(&mut self, nombre: &str) {
        self.y += LETRA_SECCION * 0.6;
        self.cabe(LETRA_SECCION * ex::INTERLINEA * 3.0);
        self.ordenes.push(texto(
            nombre.to_uppercase(),
            MARGEN_A4,
            self.y,
            LETRA_SECCION,
            TINTA_SUAVE,
            true,
        ));
        self.y += LETRA_SECCION * ex::INTERLINEA + 6.0;
    }
}

/// **La grabacion en hojas A4**: el titulo con la fecha y la duracion, los
/// marcadores, y la transcripcion con las capturas en su minuto.
fn hojas_de(
    p: &mut Preparado,
    c: &ClaseParaExportar,
    r: &Rotulos,
) -> Vec<pixpin_motor2d::exportar::Hoja> {
    let ancho = ANCHO_A4 - 2.0 * MARGEN_A4;
    let mut h = Paginas {
        hojas: Vec::new(),
        ordenes: Vec::new(),
        y: MARGEN_A4,
    };
    for l in ex::partir_texto(c.titulo.trim(), LETRA_TITULO, ancho) {
        h.ordenes
            .push(texto(l, MARGEN_A4, h.y, LETRA_TITULO, TINTA_TEXTO, true));
        h.y += LETRA_TITULO * ex::INTERLINEA;
    }
    let mut meta = Vec::new();
    if !c.fecha.trim().is_empty() {
        meta.push(c.fecha.trim().to_string());
    }
    if c.duracion() > 0 {
        meta.push(format!("{} {}", r.duracion, ce::minuto(c.duracion())));
    }
    if !meta.is_empty() {
        h.y += 2.0;
        h.ordenes.push(texto(
            meta.join("  ·  "),
            MARGEN_A4,
            h.y,
            LETRA,
            TINTA_SUAVE,
            false,
        ));
        h.y += LETRA * ex::INTERLINEA;
    }
    h.y += LETRA;
    if !c.marcadores.is_empty() {
        h.seccion(&r.marcadores);
        for m in &c.marcadores {
            let nota = if m.texto.trim().is_empty() {
                r.marcador.clone()
            } else {
                m.texto.trim().to_string()
            };
            h.parrafo(Some((ce::minuto(m.ms), MARCA)), &nota, TINTA_TEXTO);
        }
    }
    h.seccion(&r.transcripcion);
    if c.lineas.is_empty() {
        h.parrafo(None, &r.sin_texto, TINTA_SUAVE);
    }
    for f in ce::filas(c) {
        match f {
            Fila::Linea(i) => {
                let (ms, dicho) = &c.lineas[i];
                h.parrafo(ms.map(|ms| (ce::minuto(ms), ACENTO)), dicho, TINTA_TEXTO);
            }
            Fila::Marcador(i) => {
                let m = &c.marcadores[i];
                let nota = if m.texto.trim().is_empty() {
                    r.marcador.clone()
                } else {
                    format!("{}: {}", r.marcador, m.texto.trim())
                };
                h.parrafo(Some((ce::minuto(m.ms), MARCA)), &nota, MARCA);
            }
            Fila::Captura(i) => {
                let (ms, ruta) = &c.capturas[i];
                let Ok((w, alto)) = pixpin_codec::imagen::medidas(ruta) else {
                    continue;
                };
                let (w, alto) = (w.max(1) as f32, alto.max(1) as f32);
                let k = (ancho / w).min(ALTO_CAPTURA / alto);
                let (iw, ih) = (w * k, alto * k);
                let pie = LETRA * ex::INTERLINEA;
                h.y += 4.0;
                h.cabe(ih + pie + 8.0);
                let id = p.imagen(FuenteImagen::Fichero(ruta.clone()));
                h.ordenes.push(Orden::Imagen {
                    id_objeto: id,
                    x: MARGEN_A4 + (ancho - iw) / 2.0,
                    y: h.y,
                    ancho: iw,
                    alto: ih,
                    opacidad: 1.0,
                    recorte: None,
                    angulo: 0.0,
                });
                h.y += ih + 4.0;
                h.ordenes.push(texto(
                    format!("{} {} · {}", r.captura, i + 1, ce::minuto(*ms)),
                    MARGEN_A4 + (ancho - iw) / 2.0,
                    h.y,
                    LETRA * 0.9,
                    TINTA_SUAVE,
                    false,
                ));
                h.y += pie + LETRA;
            }
        }
    }
    if !h.ordenes.is_empty() || h.hojas.is_empty() {
        h.hojas.push(hoja_en_blanco(
            (0.0, 0.0, ANCHO_A4, ALTO_A4),
            std::mem::take(&mut h.ordenes),
        ));
    }
    h.hojas
}

// ---------------------------------------------------------------------------
// Los formatos

/// Copia (o enlaza, que es instantaneo) un fichero grande junto a la pagina.
fn al_lado(origen: &Path, destino: &Path) -> Result<()> {
    if destino.is_file() {
        let _ = std::fs::remove_file(destino);
    }
    if std::fs::hard_link(origen, destino).is_err() {
        std::fs::copy(origen, destino)
            .with_context(|| format!("no se pudo copiar {}", origen.display()))?;
    }
    Ok(())
}

/// Una captura para dentro de la pagina: tal cual si es ligera; si no, en
/// JPEG y sin pasar de [`LADO_CAPTURA`].
fn captura_ligera(ruta: &Path) -> Option<Medio> {
    let tal_cual = || {
        Some(Medio::Incrustado {
            mime: ce::mime_de(ruta).to_string(),
            bytes: std::fs::read(ruta).ok()?,
        })
    };
    if std::fs::metadata(ruta).ok()?.len() <= CAPTURA_LIGERA {
        return tal_cual();
    }
    let Ok(mut img) = pixpin_codec::cargar(ruta) else {
        return tal_cual();
    };
    let lado = img.ancho.max(img.alto);
    if lado > LADO_CAPTURA {
        let k = LADO_CAPTURA as f32 / lado as f32;
        let (w, h) = (
            ((img.ancho as f32 * k).round() as u32).max(1),
            ((img.alto as f32 * k).round() as u32).max(1),
        );
        if let Ok(r) = pixpin_codec::imagen::redimensionar(img.clone(), w, h) {
            img = r;
        }
    }
    let opaca = img.pixeles.chunks_exact(4).all(|p| p[3] == 255);
    let bytes = if opaca {
        pixpin_codec::imagen::codificar_jpg(&img, 85).ok()
    } else {
        pixpin_codec::codificar_png(&img).ok()
    };
    match bytes {
        Some(b) => Some(Medio::Incrustado {
            mime: if opaca { "image/jpeg" } else { "image/png" }.into(),
            bytes: b,
        }),
        None => tal_cual(),
    }
}

/// **La pagina web de la grabacion** en `carpeta`: la pagina y, al lado, lo
/// que no cabe dentro (el video, un audio muy largo, los adjuntos).
pub(crate) fn web(g: &Grabacion, base: &str, carpeta: &Path) -> Result<Vec<PathBuf>> {
    let c = &g.datos;
    let mut ficheros = Vec::new();
    let mut al_lado_de: Vec<(PathBuf, String)> = Vec::new();
    let mut poner_al_lado = |origen: &Path, que: &str| -> Option<String> {
        let ext = origen
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        let nombre = format!("{base} - {que}{ext}");
        let destino = carpeta.join(&nombre);
        al_lado(origen, &destino)
            .inspect_err(|e| {
                tracing::warn!(?e, "un fichero de la grabacion no se pudo poner al lado")
            })
            .ok()?;
        ficheros.push(destino);
        al_lado_de.push((origen.to_path_buf(), nombre.clone()));
        Some(nombre)
    };
    if let Some(a) = &c.audio
        && std::fs::metadata(a).is_ok_and(|m| m.len() > AUDIO_DENTRO)
    {
        poner_al_lado(a, &nombre_de_fichero(&g.rotulos.audio.to_lowercase()));
    }
    if let Some(v) = &c.video {
        poner_al_lado(v, &nombre_de_fichero(&g.rotulos.video.to_lowercase()));
    }
    for a in &c.adjuntos {
        let tronco = a
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        poner_al_lado(a, &nombre_de_fichero(&tronco));
    }
    let medio = |ruta: &Path| -> Option<Medio> {
        if let Some((_, n)) = al_lado_de.iter().find(|(o, _)| o == ruta) {
            return Some(Medio::AlLado(n.clone()));
        }
        if c.capturas.iter().any(|(_, r)| r == ruta) {
            return captura_ligera(ruta);
        }
        Some(Medio::Incrustado {
            mime: ce::mime_de(ruta).to_string(),
            bytes: std::fs::read(ruta).ok()?,
        })
    };
    let pagina = ce::html(c, &g.rotulos, &medio);
    let ruta = carpeta.join(format!("{base}.html"));
    escribir(&ruta, pagina.as_bytes())?;
    ficheros.insert(0, ruta);
    Ok(ficheros)
}

/// **Markdown**: solo el `.md` si no hay capturas; con ellas, el ZIP con
/// la carpeta de las capturas al lado.
fn markdown(g: &Grabacion, base: &str, carpeta: &Path) -> Result<Vec<PathBuf>> {
    if g.datos.capturas.is_empty() {
        let r = carpeta.join(format!("{base}.md"));
        escribir(
            &r,
            ce::markdown(&g.datos, &g.rotulos, &|_: &Path| None).as_bytes(),
        )?;
        return Ok(vec![r]);
    }
    let bytes = ce::paquete_markdown(&g.datos, &g.rotulos, base, &|r: &Path| {
        std::fs::read(r).ok()
    })
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    let r = carpeta.join(format!("{base} (Markdown).zip"));
    escribir(&r, &bytes)?;
    Ok(vec![r])
}

/// **Word**: el mismo Markdown por el escritor de `.docx` de las notas, con
/// las capturas dentro.
fn word(g: &Grabacion, base: &str, carpeta: &Path) -> Result<Vec<PathBuf>> {
    let c = &g.datos;
    let nombres: Vec<(PathBuf, String)> = c
        .capturas
        .iter()
        .enumerate()
        .map(|(i, (ms, r))| (r.clone(), ce::nombre_de_captura(i, *ms, r)))
        .collect();
    let md = ce::markdown(c, &g.rotulos, &|ruta: &Path| {
        nombres
            .iter()
            .find(|(r, _)| r == ruta)
            .map(|(_, n)| n.clone())
    });
    let resolver = |r: &str| {
        nombres
            .iter()
            .find(|(_, n)| n == r)
            .map(|(ruta, _)| ruta.clone())
    };
    let (bytes, _) = pixpin_notas::word::exportar(
        &md,
        &c.titulo,
        &pixpin_notas::vista::Vista::default(),
        None,
        &resolver,
        "",
        pixpin_shell::entorno::ahora_utc_ms(),
    )
    .context("no se pudo hacer el Word")?;
    let r = carpeta.join(format!("{base}.docx"));
    escribir(&r, &bytes)?;
    Ok(vec![r])
}

/// **Anki**: las tarjetas de la grabacion (una por captura, con lo dicho en
/// su tramo) en el fichero de «Archivo → Importar», y sus imagenes al lado.
/// Como el «Enviar a Anki» sin Anki abierto, las imagenes se dejan tambien
/// en la carpeta de medios de Anki, si esta: son las mismas con el mismo
/// nombre, asi que repetirlo no ensucia.
fn anki(g: &Grabacion, base: &str, carpeta: &Path) -> Result<Vec<PathBuf>> {
    let c = &g.datos;
    let trozos: Vec<(i64, String)> = c
        .lineas
        .iter()
        .filter_map(|(ms, t)| Some(((*ms)?, t.clone())))
        .collect();
    let capturas: Vec<(i64, pixpin_anki::Imagen)> = c
        .capturas
        .iter()
        .filter_map(|(ms, r)| {
            let ext = r
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_else(|| "png".into());
            let nombre = format!(
                "pixpin-{}-{}.{ext}",
                nombre_de_fichero(&c.titulo).replace(' ', "-"),
                ms
            );
            Some((
                *ms,
                pixpin_anki::Imagen {
                    nombre,
                    bytes: std::fs::read(r).ok()?,
                },
            ))
        })
        .collect();
    let mut tarjetas =
        pixpin_anki::tarjetas::de_la_clase(&c.titulo, &g.pregunta, &trozos, capturas);
    anyhow::ensure!(!tarjetas.is_empty(), "sin tarjetas");
    for t in &mut tarjetas {
        t.etiquetas = vec!["pixpin".to_string()];
    }
    let r = carpeta.join(format!("{base} (Anki).txt"));
    escribir(
        &r,
        pixpin_anki::fichero::texto(&g.mazo, &tarjetas).as_bytes(),
    )?;
    let mut v = vec![r];
    let medios = medios_de_anki();
    for i in tarjetas.iter().flat_map(|t| &t.imagenes) {
        let ruta = carpeta.join(&i.nombre);
        escribir(&ruta, &i.bytes)?;
        v.push(ruta);
        if let Some(m) = &medios {
            let _ = std::fs::write(m.join(&i.nombre), &i.bytes);
        }
    }
    Ok(v)
}

/// `%APPDATA%\Anki2\<perfil>\collection.media` del perfil tocado por ultima
/// vez, si Anki esta instalado (lo mismo que el «Enviar a Anki» del chat).
fn medios_de_anki() -> Option<PathBuf> {
    let anki = PathBuf::from(std::env::var_os("APPDATA")?).join("Anki2");
    std::fs::read_dir(anki)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path().join("collection.media"))
        .filter(|p| p.is_dir())
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::clase::Marcador;

    fn temporal(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-grabacion-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn grabacion(dir: &Path) -> Grabacion {
        let png = pixpin_codec::codificar_png(&pixpin_codec::ImagenRgba {
            ancho: 8,
            alto: 6,
            pixeles: vec![200; 8 * 6 * 4],
        })
        .unwrap();
        let captura = dir.join("c1.png");
        std::fs::write(&captura, png).unwrap();
        let audio = dir.join("clase.m4a");
        std::fs::write(&audio, b"aac").unwrap();
        let video = dir.join("clase.mp4");
        std::fs::write(&video, b"mp4").unwrap();
        Grabacion {
            datos: ClaseParaExportar {
                titulo: "Clase 1".into(),
                fecha: "hoy".into(),
                duracion_ms: 60_000,
                audio: Some(audio),
                video: Some(video),
                capturas: vec![(10_000, captura)],
                lineas: vec![(Some(0), "hola".into()), (Some(20_000), "adios".into())],
                marcadores: vec![Marcador {
                    ms: 5_000,
                    texto: "ojo".into(),
                }],
                adjuntos: vec![],
            },
            rotulos: Rotulos::default(),
            mazo: "PixPin::Fisica".into(),
            pregunta: "¿Qué?".into(),
        }
    }

    #[test]
    fn la_web_lleva_el_audio_dentro_y_el_video_al_lado() {
        let dir = temporal("web");
        let g = grabacion(&dir);
        let salida = dir.join("salida");
        std::fs::create_dir_all(&salida).unwrap();
        let v = web(&g, "Clase 1", &salida).unwrap();
        assert_eq!(v[0], salida.join("Clase 1.html"));
        assert_eq!(v[1], salida.join("Clase 1 - vídeo.mp4"));
        let h = std::fs::read_to_string(&v[0]).unwrap();
        assert!(h.contains("<audio id=\"audio\" src=\"data:audio/mp4;base64,"));
        assert!(h.contains("src=\"Clase%201%20-%20v%C3%ADdeo.mp4\""));
        assert!(h.contains("data:image/png;base64,"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_word_lleva_las_capturas_y_el_texto() {
        let dir = temporal("word");
        let g = grabacion(&dir);
        let v = word(&g, "Clase 1", &dir).unwrap();
        let bytes = std::fs::read(&v[0]).unwrap();
        let mut p = pixpin_docs::Paquete::de_bytes(&bytes).unwrap();
        let nombres = p.nombres();
        assert!(
            nombres.iter().any(|n| n.starts_with("word/media/")),
            "{nombres:?}"
        );
        let d = pixpin_docs::docx::de_paquete(&mut p, "x").unwrap();
        let todo: String = d.bloques.iter().map(|b| b.texto()).collect();
        assert!(todo.contains("adios"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn anki_deja_el_fichero_y_sus_imagenes() {
        let dir = temporal("anki");
        let g = grabacion(&dir);
        let v = anki(&g, "Clase 1", &dir).unwrap();
        assert_eq!(v.len(), 2);
        let t = std::fs::read_to_string(&v[0]).unwrap();
        assert!(t.contains("PixPin::Fisica"));
        assert!(v[1].is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn las_paginas_llevan_la_captura_y_el_texto() {
        let dir = temporal("hojas");
        let g = grabacion(&dir);
        let mut p = Preparado::nuevo("x");
        let hojas = hojas_de(&mut p, &g.datos, &g.rotulos);
        assert_eq!(hojas.len(), 1);
        let ordenes = &hojas[0].ordenes;
        assert!(ordenes.iter().any(|o| matches!(o, Orden::Imagen { .. })));
        let textos: Vec<&str> = ordenes
            .iter()
            .filter_map(|o| match o {
                Orden::Texto { texto, .. } => Some(texto.as_str()),
                _ => None,
            })
            .collect();
        let hola = textos.iter().position(|t| *t == "hola").unwrap();
        let adios = textos.iter().position(|t| *t == "adios").unwrap();
        let pie = textos
            .iter()
            .position(|t| t.starts_with("Captura 1"))
            .unwrap();
        assert!(hola < pie && pie < adios);
        assert_eq!(p.imagenes.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
