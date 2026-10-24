//! **Las letras de los planos viajan con los aparatos propios** (14-oct-2026).
//!
//! El usuario tiene AutoCAD en el PC y no quiere que el movil le pida letras
//! al abrir planos. Las letras que **el mismo** pone en su carpeta
//! (`<raiz>/fuentes-shx`, el boton «Aa» del visor: ver
//! `pixpin_cad::letras`) se llevan a sus otros aparatos, nunca a personas:
//! PixPin no reparte letras de nadie (las de Autodesk y Microsoft no se
//! copian ni se suben a ningun sitio; solo viajan las que el usuario anadio).
//!
//! **Como viajan**: un mensaje ARCHIVO por letra en el chat «Asistente»
//! (que no se comparte), con id fijo [`id_de`] y ruta portatil
//! `pixpin:files/guardados/fuentes-shx/<nombre>` ([`ruta_portatil`]). Es lo
//! de `automatizaciones::senalar_fichero`, pero uno por fichero: asi la
//! sincronizacion manda solo lo nuevo o cambiado (compara cada fichero por su
//! resumen), dos aparatos que anaden letras a la vez no se pisan, y **borrar
//! el mensaje borra el fichero en el otro aparato** (lo de `guardados/` se
//! va con su mensaje: `adjuntosFuera` del movil, `borrar_adjuntos` aqui).
//! En el PC esa ruta del chat «Asistente» es la carpeta del usuario
//! (`vista::LETRAS_PORTATIL`): lo que llega se usa al momento y el visor
//! abierto relee el plano (mira la huella de las letras).
//!
//! Por que `guardados/fuentes-shx/` y no `fuentes-shx/` como la carpeta del
//! movil (`LectorDePlanos.carpetaShx`): `fuentes-shx/` no esta en
//! `Disco.CARPETAS` de ningun aparato (la sincronizacion la rechaza) y fuera
//! de `guardados/` borrar el mensaje no se lleva el fichero. El movil tiene
//! que leer sus letras de `files/guardados/fuentes-shx` (ver
//! `docs/investigacion/2026-10-13-para-android.md`).
//!
//! Topes: una letra de mas de [`TOPE_POR_LETRA`], o las que pasen de
//! [`TOPE_TOTAL`] entre todas, se quedan en este aparato. Lo que no es una
//! letra por dentro ([`pixpin_cad::letras::parece_letra`]) no se anade, y si
//! llega, se rechaza ([`limpiar`]).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::{self, Clase, Cuaderno, Mensaje, Sello};
use pixpin_proyecto::vista;

/// El principio del id del mensaje de cada letra.
pub const PREFIJO_ID: &str = "auto-letra-";
/// Una letra mas grande no viaja (las SHX pesan KB; una TrueType normal,
/// menos de 1 MB; las asiaticas enormes se quedan donde estan).
pub const TOPE_POR_LETRA: u64 = 10 * 1024 * 1024;
/// Entre todas las que viajan.
pub const TOPE_TOTAL: u64 = 64 * 1024 * 1024;
/// El texto del mensaje.
const TEXTO: &str = "🔤 Letra de los planos (boton «Aa» del visor; solo va a tus aparatos)";

/// El id fijo del mensaje de la letra `nombre` (sin mirar mayusculas).
pub fn id_de(nombre: &str) -> String {
    format!(
        "{PREFIJO_ID}{}",
        pixpin_sincro::disco::limpio(&nombre.to_lowercase())
    )
}

/// Donde la deja el otro aparato: `pixpin:files/guardados/fuentes-shx/<nombre>`.
pub fn ruta_portatil(nombre: &str) -> String {
    format!(
        "{}{}{nombre}",
        pixpin_sincro::disco::PORTATIL,
        vista::LETRAS_PORTATIL
    )
}

/// Un nombre que puede viajar tal cual: el plano pide la letra por su
/// nombre, asi que no se cambia; uno que corta la ruta portatil en un texto
/// (comillas, parentesis...) o que sale de la carpeta se queda aqui.
fn nombre_que_viaja(nombre: &str) -> bool {
    !nombre.trim().is_empty()
        && !nombre.starts_with('.')
        && !nombre.chars().any(|c| {
            matches!(
                c,
                '/' | '\\' | '"' | '\'' | '(' | ')' | '<' | '>' | '\n' | '\r'
            ) || c.is_control()
        })
        && !nombre.contains("..")
}

/// La carpeta de letras del usuario en esta raiz.
pub fn carpeta_en(raiz: &Path) -> PathBuf {
    pixpin_cad::letras::carpeta_en(raiz)
}

/// El chat «Asistente», si ya esta (sin crearlo).
fn asistente(raiz: &Path) -> Option<almacen::Ficha> {
    Indice::leer(raiz)
        .proyectos
        .into_iter()
        .find(|f| f.es_asistente())
}

/// **Pone el mensaje de cada letra de la carpeta que no lo tenga**, dentro
/// de los topes. Si ya esta, no se toca: el fichero cambia, el mensaje no
/// (la sincronizacion mira el fichero). Devuelve cuantos puso.
pub fn senalar(raiz: &Path, aparato: &str, ahora: i64) -> std::io::Result<usize> {
    let letras: Vec<PathBuf> = pixpin_cad::letras::letras_en(&carpeta_en(raiz))
        .into_iter()
        .filter(|p| pixpin_cad::letras::parece_letra(p))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(nombre_que_viaja)
        })
        .collect();
    if letras.is_empty() {
        return Ok(0);
    }
    let (proyecto, carpeta) = crate::automatizaciones::chat_del_asistente(raiz, aparato, ahora)?;
    let c = match Cuaderno::leer_de(&carpeta) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => return Err(e),
    };
    let ya: BTreeSet<&str> = c
        .mensajes
        .iter()
        .map(|m| m.id.as_str())
        .filter(|i| i.starts_with(PREFIJO_ID))
        .collect();
    let peso = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let nombre = |p: &Path| {
        p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let mut total: u64 = letras
        .iter()
        .filter(|p| ya.contains(id_de(&nombre(p)).as_str()))
        .map(|p| peso(p))
        .sum();
    let mut numero = c.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let mut puestos = 0;
    let mut vistos = BTreeSet::new();
    for l in &letras {
        let n = nombre(l);
        let id = id_de(&n);
        if ya.contains(id.as_str()) || !vistos.insert(id.clone()) {
            continue;
        }
        let bytes = peso(l);
        if bytes > TOPE_POR_LETRA || total + bytes > TOPE_TOTAL {
            tracing::info!(letra = %n, bytes, "letras: demasiado grande para viajar, se queda en este equipo");
            continue;
        }
        let mut m = Mensaje::adjunto(
            Clase::Archivo,
            &n,
            &ruta_portatil(&n),
            bytes as i64,
            &Sello {
                cuando: ahora,
                numero,
                aparato: aparato.to_string(),
                proyecto: proyecto.clone(),
            },
        );
        m.id = id;
        m.texto = TEXTO.into();
        cuaderno::anadir(&carpeta, &m)?;
        total += bytes;
        numero += 1;
        puestos += 1;
    }
    Ok(puestos)
}

/// Quita los mensajes de letra cuyo nombre cumpla `cuales`, **con su marca
/// de borrado** (asi el otro aparato los quita, y con ellos sus ficheros).
fn quitar_mensajes(
    raiz: &Path,
    ahora: i64,
    cuales: impl Fn(&Mensaje) -> bool,
) -> std::io::Result<Vec<Mensaje>> {
    let Some(f) = asistente(raiz) else {
        return Ok(Vec::new());
    };
    let carpeta = almacen::carpeta(raiz, &f.id);
    let Ok(c) = Cuaderno::leer_de(&carpeta) else {
        return Ok(Vec::new());
    };
    let ids: BTreeSet<String> = c
        .mensajes
        .iter()
        .filter(|m| m.id.starts_with(PREFIJO_ID) && cuales(m))
        .map(|m| m.id.clone())
        .collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let quitados = cuaderno::quitar(&carpeta, &ids)?;
    vista::anotar_borrados(raiz, &f.id, &quitados, ahora)?;
    Ok(quitados)
}

/// **Anade las letras elegidas** (boton «Aa» → «Añadir letras…») y las
/// hace viajar. Lo que no es una letra por dentro se salta. Devuelve
/// cuantas se copiaron.
pub fn anadir(raiz: &Path, aparato: &str, ahora: i64, rutas: &[PathBuf]) -> usize {
    let buenas: Vec<PathBuf> = rutas
        .iter()
        .filter(|r| pixpin_cad::letras::parece_letra(r))
        .cloned()
        .collect();
    let n = pixpin_cad::letras::anadir(&carpeta_en(raiz), &buenas);
    if n > 0
        && let Err(e) = senalar(raiz, aparato, ahora)
    {
        tracing::warn!(%e, "letras: no se pudieron senalar para la sincro");
    }
    n
}

/// **«Quitar las mias»**: fuera las letras de la carpeta y sus mensajes,
/// con marca: en los otros aparatos se van tambien. Devuelve cuantas.
pub fn quitar_todas(raiz: &Path, ahora: i64) -> usize {
    let n = pixpin_cad::letras::quitar_todas(&carpeta_en(raiz));
    if let Err(e) = quitar_mensajes(raiz, ahora, |_| true) {
        tracing::warn!(%e, "letras: no se pudieron quitar sus mensajes");
    }
    n
}

/// **Rechaza lo que llego y no es una letra**: un fichero con nombre de
/// letra que por dentro no lo es, o un mensaje de letra que senala otra
/// cosa. Fuera el fichero y su mensaje (con marca, para que el otro
/// aparato lo quite tambien). Devuelve cuantos.
pub fn limpiar(raiz: &Path, ahora: i64) -> usize {
    let carpeta = carpeta_en(raiz);
    let malos: BTreeSet<String> = std::fs::read_dir(&carpeta)
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && pixpin_cad::letras::es_letra(p)
                        && !pixpin_cad::letras::parece_letra(p)
                })
                .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .collect()
        })
        .unwrap_or_default();
    let no_es_letra = |m: &Mensaje| {
        let n = m.nombre.as_str();
        malos.contains(n)
            || !pixpin_cad::letras::es_letra(Path::new(n))
            || m.ruta.as_deref() != Some(ruta_portatil(n).as_str())
    };
    let quitados = quitar_mensajes(raiz, ahora, no_es_letra).unwrap_or_else(|e| {
        tracing::warn!(%e, "letras: no se pudo rechazar un mensaje");
        Vec::new()
    });
    let mut fuera: BTreeSet<String> = malos;
    let prefijo = ruta_portatil("");
    for m in &quitados {
        // El fichero al que lleva su ruta, y solo un nombre suelto: nada
        // que salga de la carpeta.
        if let Some(n) = m
            .ruta
            .as_deref()
            .and_then(|r| r.strip_prefix(prefijo.as_str()))
            && !n.is_empty()
            && !n.contains(['/', '\\'])
            && !n.contains("..")
        {
            fuera.insert(n.to_string());
        }
    }
    let borrados = fuera
        .iter()
        .filter(|n| std::fs::remove_file(carpeta.join(n)).is_ok())
        .count();
    borrados.max(quitados.len())
}

/// **Al abrir un plano**: rechazar lo que no vale y hacer viajar las letras
/// que el usuario ya tenia (las de antes de esto, o las dejadas a mano).
pub fn al_abrir(raiz: &Path) {
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    limpiar(raiz, ahora);
    if let Err(e) = senalar(raiz, &aparato(raiz), ahora) {
        tracing::warn!(%e, "letras: no se pudieron senalar para la sincro");
    }
}

/// El codigo de este equipo (el sello de los mensajes).
pub fn aparato(raiz: &Path) -> String {
    pixpin_proyecto::identidad::Identidad::leer_o_crear(raiz, "PC")
        .map(|i| i.yo.codigo())
        .unwrap_or_default()
}

#[cfg(test)]
mod pruebas;
