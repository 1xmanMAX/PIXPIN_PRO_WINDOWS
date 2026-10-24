//! **Lo que cambia en el disco, a la vista** (14-oct-2026). El lector de
//! PDF y Word guarda en memoria la capa de cada hoja que ya leyo; lo que
//! escribia otro en esa capa con el lector abierto (el asistente al anotar,
//! `asistente::documentos_acciones`, o la sincronizacion con el movil) no
//! se veia hasta reabrir, y al guardar lo de aqui se pisaba. El visor de
//! planos ya lo hacia bien (`plano_cad::anotar`, `al_dia`); esto es lo
//! mismo para el lector:
//!
//! - **Mirar barato**: como mucho cada [`MIRAR_EL_DISCO`], solo la firma
//!   (fecha y tamano) de las capas ya leidas. Si cambio y aqui no hay nada
//!   a medias (capa sin guardar o un trazo en curso), se relee.
//! - **Sin pisar**: con algo a medias no se relee; al guardar, si el disco
//!   cambio desde que se leyo, lo de aqui se escribe aparte y se junta con
//!   lo del disco con la regla de la sincronizacion
//!   (`plano_cad::anotar::juntar_con_el_disco`), y se sigue anotando sobre
//!   lo junto.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use super::{Estado, alto_de};

/// Cada cuanto, como mucho, se mira si el disco cambio (lo del plano).
pub(super) const MIRAR_EL_DISCO: Duration = Duration::from_millis(700);

/// La firma de un fichero: su fecha y su tamano (`None` si no esta).
type Sello = Option<(SystemTime, u64)>;

fn sello_de(ruta: &Path) -> Sello {
    std::fs::metadata(ruta)
        .ok()
        .and_then(|m| Some((m.modified().ok()?, m.len())))
}

/// Como estaba en el disco la capa de una hoja la ultima vez que se leyo o
/// se escribio aqui.
struct Visto {
    ruta: PathBuf,
    sello: Sello,
    /// El texto de entonces: la base para juntar.
    base: Option<String>,
}

/// La firma antes que el texto: si cambia entre medias, la proxima vez se
/// vuelve a leer (de mas, nunca de menos).
fn ver(ruta: PathBuf) -> Visto {
    let sello = sello_de(&ruta);
    let base = std::fs::read_to_string(&ruta).ok();
    Visto { ruta, sello, base }
}

/// Lo que se sabe del disco de cada capa abierta.
#[derive(Default)]
pub(super) struct AlDia {
    mirado: Option<Instant>,
    vistos: HashMap<usize, Visto>,
    /// El ultimo aviso del asistente que se vio (`lector_tinta::turnos`).
    aviso: u64,
}

impl AlDia {
    /// Las capas se van a leer de otro sitio (el PDF paso a proyecto).
    pub(super) fn olvidar(&mut self) {
        self.vistos.clear();
    }
}

/// **Apunta como esta en el disco** cada capa ya leida que aun no se
/// miraba, y olvida las que ya no estan. Se llama al pintar: barato si no
/// hay nada nuevo.
pub(super) fn apuntar(e: &mut Estado, pdf: &Path) {
    let nuevas: Vec<usize> = e
        .capas
        .keys()
        .filter(|i| !e.al_dia.vistos.contains_key(i))
        .copied()
        .collect();
    for i in nuevas {
        let ruta = e.donde.para_leer(pdf, i);
        e.al_dia.vistos.insert(i, ver(ruta));
    }
    if e.al_dia.vistos.len() > e.capas.len() {
        let capas = &e.capas;
        e.al_dia.vistos.retain(|i, _| capas.contains_key(i));
    }
}

/// [`mirar_ya`], sin mirar el disco mas de una vez cada [`MIRAR_EL_DISCO`].
/// Si el asistente acaba de escribir en alguna capa, se mira ya, sin
/// esperar a los 0,7 s: lo primero que anota se ve al momento.
pub(super) fn mirar(e: &mut Estado, pdf: &Path) -> bool {
    let avisado = crate::lector_tinta::turnos::hay_aviso(&mut e.al_dia.aviso);
    if !avisado
        && e.al_dia
            .mirado
            .is_some_and(|t| t.elapsed() < MIRAR_EL_DISCO)
    {
        return false;
    }
    e.al_dia.mirado = Some(Instant::now());
    mirar_ya(e, pdf)
}

/// **Relee las capas que cambiaron en el disco** y no tienen nada a medias
/// aqui. Dice si releyo alguna (hay que repintar).
pub(super) fn mirar_ya(e: &mut Estado, pdf: &Path) -> bool {
    apuntar(e, pdf);
    let trazando_en = if e.tinta.trazando() {
        e.tinta.hoja
    } else {
        None
    };
    let mut releidas = false;
    let hojas: Vec<usize> = e.al_dia.vistos.keys().copied().collect();
    for i in hojas {
        let Some(v) = e.al_dia.vistos.get(&i) else {
            continue;
        };
        if sello_de(&v.ruta) == v.sello {
            continue;
        }
        // Lo de aqui a medias no se pisa: se junta al guardar.
        if e.capas.get(&i).is_none_or(|c| c.sucia) || trazando_en == Some(i) {
            continue;
        }
        let visto = ver(v.ruta.clone());
        let capa = e
            .donde
            .leer_capa(pdf, i, e.ajustes.espacios, alto_de(&e.hojas, i));
        tracing::info!(hoja = i, ruta = %visto.ruta.display(), "lector: la capa cambio en el disco, releida");
        e.capas.insert(i, capa);
        e.al_dia.vistos.insert(i, visto);
        // Los pasos de antes eran de la capa de antes: deshacerlos en la
        // releida quitaria lo leido. Lo que trae del asistente entra como
        // su propio paso (`apuntar_turnos`).
        e.hechos.retain(|k| *k != i);
        e.deshechos.retain(|k| *k != i);
        releidas = true;
    }
    if releidas {
        // Lo calculado y lo elegido eran de la capa de antes.
        e.tinta.olvidar_lo_calculado();
        e.tinta.gesto.seleccion.limpiar();
    }
    apuntar_turnos(e);
    releidas
}

/// **Lo que puso el asistente, en el orden de deshacer**: cada hoja leida
/// con su turno marcado (`lector_tinta::turnos`) entra en `hechos`, para
/// que Ctrl+Z lo quite (de todas sus hojas a la vez, `lector_pdf::deshacer`).
pub(super) fn apuntar_turnos(e: &mut Estado) {
    let mut nuevas: Vec<usize> = e
        .capas
        .iter_mut()
        .filter_map(|(i, c)| {
            let t = c.turno.as_mut().filter(|t| t.sin_apuntar)?;
            t.sin_apuntar = false;
            Some(*i)
        })
        .collect();
    nuevas.sort_unstable();
    e.hechos.extend(nuevas);
}

/// **Guarda las capas con cambios**. La que cambio en el disco desde que se
/// leyo se junta con el ([`juntar`]); las demas, como siempre.
pub(super) fn guardar(e: &mut Estado, pdf: &Path) {
    let sucias: Vec<usize> = e
        .capas
        .iter()
        .filter(|(_, c)| c.sucia)
        .map(|(i, _)| *i)
        .collect();
    for i in sucias {
        let alto = alto_de(&e.hojas, i);
        let cambiada = e
            .al_dia
            .vistos
            .get(&i)
            .is_some_and(|v| v.ruta.is_file() && sello_de(&v.ruta) != v.sello);
        let hecho = if cambiada {
            juntar(e, pdf, i, alto)
        } else {
            match e.capas.get_mut(&i) {
                // La tinta y, al lado, su marco (`anot-<uid>-p<i>.hoja`).
                Some(c) => e.donde.guardar_capa(pdf, i, c, alto),
                None => Ok(()),
            }
        };
        match hecho {
            // Lo escrito es lo que hay ahora: no se vuelve a leer.
            Ok(()) => {
                let ruta = e.donde.para_leer(pdf, i);
                e.al_dia.vistos.insert(i, ver(ruta));
            }
            Err(err) => tracing::warn!(?err, hoja = i, "no se pudo guardar lo anotado en la hoja"),
        }
    }
}

/// **Guardar sin pisar lo que llego**: lo de aqui, aparte; junto con lo del
/// disco (la base es lo que se leyo); escrito de una vez; y releido, para
/// seguir anotando sobre lo junto. Si algo no se entiende (un disco danado),
/// lo de aqui, como antes.
fn juntar(e: &mut Estado, pdf: &Path, i: usize, alto: f32) -> std::io::Result<()> {
    let Some(v) = e.al_dia.vistos.get(&i) else {
        return Ok(());
    };
    let (ruta, base) = (v.ruta.clone(), v.base.clone());
    let Some(capa) = e.capas.get_mut(&i) else {
        return Ok(());
    };
    let suyo = std::fs::read_to_string(&ruta)?;
    let aparte = ruta.with_extension("excalidraw.mio");
    e.donde.escribir_aparte(i, capa, &aparte)?;
    let mio = std::fs::read_to_string(&aparte);
    let _ = std::fs::remove_file(&aparte);
    let mio = mio?;
    match crate::plano_cad::anotar::juntar_con_el_disco(base.as_deref(), &mio, &suyo) {
        Some(junto) => {
            pixpin_sincro::disco::escribir_atomico(&ruta, junto.as_bytes())?;
            e.donde.apuntar_marco(i, alto)?;
            tracing::info!(
                hoja = i,
                "lector: lo anotado se junto con lo que cambio en el disco"
            );
        }
        None => {
            tracing::warn!(ruta = %ruta.display(), "la capa del disco no se entiende: se escribe la de aqui");
            capa.sucia = true;
            e.donde.guardar_capa(pdf, i, capa, alto)?;
        }
    }
    let capa = e.donde.leer_capa(pdf, i, e.ajustes.espacios, alto);
    e.capas.insert(i, capa);
    e.tinta.olvidar_lo_calculado();
    e.tinta.gesto.seleccion.limpiar();
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::super::pruebas::estado;
    use super::*;
    use pixpin_motor2d::vector::Punto2;

    /// Un PDF suelto (su tinta, junto a el) en una carpeta propia.
    fn pdf(etiqueta: &str) -> (PathBuf, PathBuf) {
        let r =
            std::env::temp_dir().join(format!("pixpin-al-dia-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        let pdf = r.join("informe.pdf");
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        (r, pdf)
    }

    /// Lo que escribe el asistente (u otro aparato) en la capa de la hoja
    /// 0: un trazo con su id, junto a lo que ya hubiera.
    fn escribe_otro(e: &Estado, pdf: &Path, id: &str) {
        let ruta = e.donde.para_escribir(pdf, 0);
        let mut v: serde_json::Value = std::fs::read_to_string(&ruta)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(|| serde_json::json!({"type":"excalidraw","version":2,"elements":[]}));
        v["elements"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": id, "type": "freedraw", "x": 100, "y": 100, "width": 50, "height": 0,
                "strokeWidth": 2, "version": 1, "versionNonce": 7, "updated": 1,
                "points": [[0.0, 0.0], [50.0, 0.0]]
            }));
        std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
        // Otro tamano seguro: la firma cambia aunque la fecha no lo hiciera.
        std::fs::write(&ruta, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    }

    /// Lo que se ve en la capa de la hoja 0 (lo de cada uno se mira en el
    /// disco por su id: en memoria los ids son numeros).
    fn cuantos(e: &Estado) -> usize {
        e.capas[&0].escena.visibles().count()
    }

    fn abrir(pdf: &Path) -> Estado {
        let mut e = estado(1);
        e.donde = crate::lector_pdf_proyecto::DondeVa::de(Path::new(""), pdf, 1);
        let alto = alto_de(&e.hojas, 0);
        let capa = e.donde.leer_capa(pdf, 0, 0, alto);
        e.capas.insert(0, capa);
        apuntar(&mut e, pdf);
        e
    }

    #[test]
    fn lo_que_anota_el_asistente_se_ve_con_el_pdf_abierto() {
        let (r, pdf) = pdf("se-ve");
        let mut e = abrir(&pdf);
        assert!(e.capas[&0].vacia());
        // Sin cambios en el disco, no se relee nada.
        assert!(!mirar_ya(&mut e, &pdf));
        escribe_otro(&e, &pdf, "del-asistente");
        assert!(mirar_ya(&mut e, &pdf), "la capa cambio: se relee");
        assert_eq!(cuantos(&e), 1, "el trazo del asistente, a la vista");
        // Caso negativo: releida una vez, no se relee otra sin cambios.
        assert!(!mirar_ya(&mut e, &pdf));
        // Y lo guardado aqui no cuenta como cambio de fuera.
        let c = e.capas.get_mut(&0).unwrap();
        e.tinta
            .trazar(c, &[Punto2::nuevo(10.0, 10.0), Punto2::nuevo(60.0, 60.0)]);
        guardar(&mut e, &pdf);
        assert!(!mirar_ya(&mut e, &pdf), "lo propio no se relee");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn mirar_el_disco_es_barato_como_mucho_cada_700_ms() {
        let (r, pdf) = pdf("barato");
        let mut e = abrir(&pdf);
        assert!(!mirar(&mut e, &pdf));
        escribe_otro(&e, &pdf, "pronto");
        // Antes de los 0,7 s ni se mira.
        assert!(!mirar(&mut e, &pdf));
        e.al_dia.mirado = Some(Instant::now() - MIRAR_EL_DISCO);
        assert!(mirar(&mut e, &pdf));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn lo_que_anota_el_usuario_a_medias_no_se_pisa_y_se_junta_al_guardar() {
        let (r, pdf) = pdf("a-medias");
        let mut e = abrir(&pdf);
        escribe_otro(&e, &pdf, "antes");
        assert!(mirar_ya(&mut e, &pdf));
        // El usuario anota (sin guardar aun) y mientras tanto el asistente
        // escribe en la misma capa.
        let c = e.capas.get_mut(&0).unwrap();
        e.tinta
            .trazar(c, &[Punto2::nuevo(10.0, 10.0), Punto2::nuevo(60.0, 60.0)]);
        let mios = e.capas[&0].escena.visibles().count();
        assert!(e.capas[&0].sucia);
        escribe_otro(&e, &pdf, "mientras");
        // Caso negativo: con algo a medias no se relee (se perderia el trazo).
        assert!(!mirar_ya(&mut e, &pdf));
        assert_eq!(e.capas[&0].escena.visibles().count(), mios);
        // Al guardar se junta: queda lo de los dos, en el disco y aqui.
        guardar(&mut e, &pdf);
        assert_eq!(
            cuantos(&e),
            mios + 1,
            "lo del asistente entra y el trazo del usuario sigue"
        );
        assert!(!e.capas[&0].sucia);
        let en_disco = std::fs::read_to_string(e.donde.para_leer(&pdf, 0)).unwrap();
        assert!(en_disco.contains("mientras") && en_disco.contains("antes"));
        assert!(
            !e.donde
                .para_leer(&pdf, 0)
                .with_extension("excalidraw.mio")
                .exists(),
            "sin restos"
        );
        // Ya junto, nada nuevo que releer.
        assert!(!mirar_ya(&mut e, &pdf));
        let _ = std::fs::remove_dir_all(&r);
    }

    /// Lo que pone el asistente en un turno (en dos hojas) se quita con UN
    /// Ctrl+Z y vuelve con un Ctrl+Y; un trazo del usuario de antes sigue.
    #[test]
    fn el_turno_del_asistente_se_deshace_entero_en_todas_sus_hojas() {
        use crate::lector_tinta::{Capa, turnos};
        let (r, pdf) = pdf("turno");
        let mut e = estado(2);
        e.donde = crate::lector_pdf_proyecto::DondeVa::de(Path::new(""), &pdf, 1);
        let grupo = "asistente-anotado:prueba-turno";
        for i in 0..2 {
            let mut c = Capa::default();
            c.escena.anadir(pixpin_motor2d::Elemento {
                x: 10.0,
                y: 10.0,
                ancho: 40.0,
                alto: 20.0,
                grupos: vec![grupo.into()],
                ..Default::default()
            });
            c.sucia = true;
            let ruta = e.donde.para_escribir(&pdf, i);
            turnos::apuntar(&e.donde.para_leer(&pdf, i), grupo);
            std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
            c.guardar(&ruta).unwrap();
        }
        for i in 0..2 {
            let alto = alto_de(&e.hojas, i);
            let capa = e.donde.leer_capa(&pdf, i, 0, alto);
            e.capas.insert(i, capa);
        }
        apuntar_turnos(&mut e);
        assert_eq!(e.hechos.len(), 2);
        let visibles = |e: &Estado| {
            e.capas
                .values()
                .map(|c| c.escena.visibles().count())
                .sum::<usize>()
        };
        assert_eq!(visibles(&e), 2);
        super::super::deshacer(&mut e);
        assert_eq!(visibles(&e), 0, "un solo Ctrl+Z quita lo de las dos hojas");
        assert!(e.hechos.is_empty());
        super::super::rehacer(&mut e);
        assert_eq!(visibles(&e), 2, "y un Ctrl+Y lo devuelve");
        // Caso negativo: otra vez, sin turno nuevo, no se apunta de mas.
        apuntar_turnos(&mut e);
        assert_eq!(e.hechos.len(), 2);
        let _ = std::fs::remove_dir_all(&r);
    }
}
