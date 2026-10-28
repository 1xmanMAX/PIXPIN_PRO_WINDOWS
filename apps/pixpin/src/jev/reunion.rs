//! **Lo importante de una clase o reunion** al acabar de pasarla a texto
//! (`docs/jev.md`): cada renglon de la transcripcion va a Jev con sus
//! vecinos y el titulo, en segundo plano y varios a la vez
//! (`pixpin_typesafe::reunion`); lo hallado se escribe en la nota de voz,
//! `resto["importantes"]` (`pixpin_proyecto::clase`), que viaja al movil.
//! «Estudiar la clase» y la pagina compartida lo ensenan con sus chapitas y
//! su filtro; las tareas se pueden apuntar en Tareas ([`a_tareas`]).
//!
//! Sin clave, con la funcion apagada o si algo falla, la nota se queda como
//! estaba (sin `importantes`): un fallo no se guarda como «no hay nada».

use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use pixpin_proyecto::clase::{self, Importante, TipoImportante};
use pixpin_proyecto::{almacen, cuaderno};
use pixpin_store::Ubicacion;
use pixpin_typesafe::reunion::{self as jr, Renglon};

use super::Funcion;

/// Cuantas preguntas van a la vez (Jev contesta en 70-500 ms; una clase de
/// una hora son unos 150 renglones).
const A_LA_VEZ: usize = 4;

/// Las notas que se estan mirando ahora (por id).
static EN_MARCHA: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn con_en_marcha<R>(f: impl FnOnce(&mut HashSet<String>) -> R) -> R {
    let mut g = EN_MARCHA.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(HashSet::new))
}

/// Si se esta buscando lo importante de esta nota.
pub fn en_marcha(id: &str) -> bool {
    con_en_marcha(|s| s.contains(id))
}

/// Los renglones de la transcripcion de una nota de voz, como los lee la
/// pantalla de la letra (sin las fotos metidas en el texto).
pub fn renglones_de(m: &cuaderno::Mensaje) -> Vec<Renglon> {
    crate::voz::transcripcion_de(m)
        .map(crate::voz::trozos)
        .unwrap_or_default()
        .into_iter()
        .map(|t| Renglon {
            ms: t.ms,
            texto: crate::voz::sin_marcas(&t.texto),
        })
        .filter(|r| !r.texto.trim().is_empty())
        .collect()
}

/// El color de cada tipo (el mismo en la pagina compartida, `pagina.css`):
/// azul la tarea, verde la decision, morado la pregunta y rojo el examen.
pub fn color_de(t: TipoImportante) -> u32 {
    match t {
        TipoImportante::Tarea => 0x3b82f6,
        TipoImportante::Decision => 0x22a06b,
        TipoImportante::Pregunta => 0x8b5cf6,
        TipoImportante::Examen => 0xe5534b,
    }
}

/// El texto de su chapita.
pub fn clave_de(t: TipoImportante) -> &'static str {
    match t {
        TipoImportante::Tarea => "clase-importante-tarea",
        TipoImportante::Decision => "clase-importante-decision",
        TipoImportante::Pregunta => "clase-importante-pregunta",
        TipoImportante::Examen => "clase-importante-examen",
    }
}

/// De lo hallado por Jev a lo que se guarda.
fn a_guardar(h: &jr::Hallazgo) -> Importante {
    Importante {
        ms: h.ms,
        tipo: match h.tipo {
            jr::Tipo::Tarea => TipoImportante::Tarea,
            jr::Tipo::Decision => TipoImportante::Decision,
            jr::Tipo::Pregunta => TipoImportante::Pregunta,
            jr::Tipo::Examen => TipoImportante::Examen,
        },
        p: h.p,
        quien: h.quien,
        cuando: h.cuando,
        en_tareas: false,
    }
}

/// **Busca lo importante de la nota `id`** en su hilo, si hay clave y la
/// funcion esta encendida y la nota aun no se miro (`forzar`: aunque ya se
/// mirara, p. ej. tras volver a pasarla a texto). Vuelve en seguida.
pub fn lanzar(u: &Ubicacion, proyecto: &str, id: &str, forzar: bool) {
    let Some(cliente) = super::para(u, Funcion::Reunion) else {
        return;
    };
    if !con_en_marcha(|s| s.insert(id.to_string())) {
        return;
    }
    let raiz = u.raiz().to_path_buf();
    let (proyecto, id) = (proyecto.to_string(), id.to_string());
    let id_hilo = id.clone();
    let lanzado = std::thread::Builder::new()
        .name("jev-reunion".into())
        .spawn(move || {
            let empezo = std::time::Instant::now();
            match buscar(&cliente, &raiz, &proyecto, &id_hilo, forzar) {
                Ok(Some(n)) => {
                    tracing::info!(
                        n,
                        ms = empezo.elapsed().as_millis() as u64,
                        "jev: lo importante de la clase, guardado"
                    );
                    crate::ventana_chat::refrescar();
                }
                Ok(None) => {}
                Err(e) => tracing::warn!("jev: no se busco lo importante de la clase: {e}"),
            }
            con_en_marcha(|s| s.remove(&id_hilo));
        });
    if lanzado.is_err() {
        con_en_marcha(|s| s.remove(&id));
    }
}

/// Lo de [`lanzar`], en el hilo. `Ok(None)`: no habia nada que hacer.
fn buscar(
    cliente: &pixpin_typesafe::Cliente,
    raiz: &Path,
    proyecto: &str,
    id: &str,
    forzar: bool,
) -> Result<Option<usize>, String> {
    let carpeta = almacen::carpeta(raiz, proyecto);
    let m = cuaderno::Cuaderno::leer_de(&carpeta)
        .map_err(|e| e.to_string())?
        .mensajes
        .into_iter()
        .find(|m| m.id == id)
        .ok_or("la nota ya no esta")?;
    if !forzar && clase::importantes_de(&m.resto).is_some() {
        return Ok(None);
    }
    let renglones = renglones_de(&m);
    if renglones.is_empty() {
        return Ok(None);
    }
    let titulo = crate::biblioteca_audio::titulo_de_audio(&m);
    let (hallado, modelo) =
        jr::buscar(cliente, &titulo, &renglones, A_LA_VEZ).map_err(|e| e.to_string())?;
    let v: Vec<Importante> = hallado.iter().map(a_guardar).collect();
    let n = v.len();
    let transcripcion = m.transcripcion.clone();
    // Se escribe sobre la nota de ahora (el chat o el movil pudieron
    // tocarla), y solo si el texto sigue siendo el que se miro.
    let cambiada = cuaderno::cambiar(&carpeta, id, |m| {
        if m.transcripcion != transcripcion {
            return false;
        }
        // Lo ya apuntado en Tareas sigue apuntado.
        let antes: Vec<i64> = clase::importantes_de(&m.resto)
            .unwrap_or_default()
            .into_iter()
            .filter(|i| i.en_tareas)
            .map(|i| i.ms)
            .collect();
        clase::poner_importantes(&mut m.resto, &v, &modelo);
        clase::marcar_en_tareas(&mut m.resto, &antes);
        true
    })
    .map_err(|e| e.to_string())?;
    Ok(cambiada.map(|_| n))
}

/// El texto de la tarea apuntada desde la clase: lo dicho (recortado) y, en
/// su nota (`💬`), de que clase y en que minuto, para volver a el.
pub fn texto_de_tarea(dicho: &str, clase: &str, ms: i64) -> String {
    let dicho: String = dicho.split_whitespace().collect::<Vec<_>>().join(" ");
    let corto = if dicho.chars().count() > 140 {
        format!("{}…", dicho.chars().take(140).collect::<String>())
    } else {
        dicho
    };
    format!(
        "{corto} {} 🎙 {} ⏱ {}",
        crate::tareas::frase::NOTA,
        clase.trim(),
        pixpin_voz::marca_de_tiempo(ms)
    )
}

/// **Apunta en el Inbox de Tareas** las tareas halladas en la nota que aun
/// no se apuntaron, y las marca. Devuelve cuantas.
pub fn a_tareas(raiz: &Path, proyecto: &str, id: &str) -> Result<usize, String> {
    let carpeta = almacen::carpeta(raiz, proyecto);
    let m = cuaderno::Cuaderno::leer_de(&carpeta)
        .map_err(|e| e.to_string())?
        .mensajes
        .into_iter()
        .find(|m| m.id == id)
        .ok_or("la nota ya no esta")?;
    let pendientes: Vec<i64> = clase::importantes_de(&m.resto)
        .unwrap_or_default()
        .into_iter()
        .filter(|i| i.tipo == TipoImportante::Tarea && !i.en_tareas)
        .map(|i| i.ms)
        .collect();
    if pendientes.is_empty() {
        return Ok(0);
    }
    let renglones = renglones_de(&m);
    let titulo = crate::biblioteca_audio::titulo_de_audio(&m);
    let aparato = crate::letras_que_viajan::aparato(raiz);
    let mut hechas = Vec::new();
    for ms in pendientes {
        let Some(r) = renglones.iter().find(|r| r.ms == Some(ms)) else {
            continue;
        };
        match crate::tareas::apuntar(raiz, &aparato, &texto_de_tarea(&r.texto, &titulo, ms)) {
            Ok(_) => hechas.push(ms),
            Err(e) => tracing::warn!(?e, "jev: no se pudo apuntar una tarea de la clase"),
        }
    }
    if !hechas.is_empty() {
        cuaderno::cambiar(&carpeta, id, |m| {
            clase::marcar_en_tareas(&mut m.resto, &hechas)
        })
        .map_err(|e| e.to_string())?;
        crate::ventana_chat::refrescar();
        crate::tareas::refrescar();
    }
    Ok(hechas.len())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_tarea_lleva_de_que_clase_y_que_minuto() {
        let t = texto_de_tarea(
            "Para el lunes   resuelvan los ejercicios del 1 al 5.",
            "Cálculo I",
            65_000,
        );
        assert_eq!(
            t,
            format!(
                "Para el lunes resuelvan los ejercicios del 1 al 5. {} 🎙 Cálculo I ⏱ 1:05",
                crate::tareas::frase::NOTA
            )
        );
        let largo = texto_de_tarea(&"palabra ".repeat(60), "x", 0);
        assert!(largo.contains('…'));
    }

    #[test]
    fn sin_clave_no_se_lanza_nada() {
        let u = Ubicacion::Instalado {
            raiz: std::env::temp_dir().join("pixpin-jev-reunion-sin-clave"),
        };
        lanzar(&u, "p", "n1", false);
        assert!(!en_marcha("n1"));
    }
}
