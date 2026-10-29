//! **Toda grabacion se pasa a texto, aunque PixPin se cierre antes**
//! (29-oct-2026, el punto 47 de la lista del usuario en el movil: «toda
//! grabacion se transcribe siempre»; Android lo hizo con una cola en el
//! disco, `ColaDeTexto`).
//!
//! Hasta ahora, cerrar PixPin (o «Salir» de la bandeja, que termina sin
//! pasar a texto) a mitad de la transcripcion dejaba la nota sin texto, y
//! nada la retomaba. Ahora cada nota de una clase guardada se apunta en
//! `voz/textos-pendientes/<id>.json` (`{proyecto, id, intentos}`) **antes**
//! de empezar a pasarla a texto, y se borra al escribir el texto o si el
//! usuario pulsa «Dejarlo sin texto». **Al arrancar** se retoma lo que
//! quedo ([`retomar`]): la nota que ya tiene texto (se paso a mano desde el
//! chat) o que ya no esta, fuera; la que no se pudo pasar (sin reconocedor
//! todavia), se queda para la siguiente vez; tras [`INTENTOS`] fallos del
//! reconocedor con ella, se deja (siempre se puede pedir «Pasar a texto» en
//! la nota).

use std::path::{Path, PathBuf};
use std::time::Duration;

use pixpin_proyecto::{almacen, cuaderno};
use serde::{Deserialize, Serialize};

use super::{Fase, Pedido, empezar_el_texto, escribir_el_texto};

/// Los fallos del reconocedor con una nota antes de dejarla.
const INTENTOS: u32 = 3;

/// Una nota que espera su texto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Pendiente {
    pub proyecto: String,
    pub id: String,
    #[serde(default)]
    pub intentos: u32,
}

fn carpeta(raiz: &Path) -> PathBuf {
    raiz.join("voz").join("textos-pendientes")
}

/// El fichero de la nota `id` (su id va tal cual en el nombre, sin lo que
/// no puede ir en un nombre de fichero).
fn fichero(raiz: &Path, id: &str) -> PathBuf {
    carpeta(raiz).join(format!("{}.json", almacen::nombre_seguro(id)))
}

/// **Apunta la nota** para pasarla a texto (si ya estaba, se queda como
/// estaba: no se pierden sus intentos).
pub(super) fn apuntar(raiz: &Path, proyecto: &str, id: &str) {
    let f = fichero(raiz, id);
    if f.is_file() {
        return;
    }
    let p = Pendiente {
        proyecto: proyecto.to_string(),
        id: id.to_string(),
        intentos: 0,
    };
    escribir(&f, &p);
}

fn escribir(f: &Path, p: &Pendiente) {
    if let Some(d) = f.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    match serde_json::to_vec(p) {
        Ok(b) => {
            if let Err(e) = std::fs::write(f, b) {
                tracing::warn!(?e, id = %p.id, "la nota no se apunto para pasarla a texto");
            }
        }
        Err(e) => tracing::warn!(?e, "la nota no se apunto para pasarla a texto"),
    }
}

/// **Ya tiene su texto** (o el usuario no lo quiere): fuera de la cola.
pub(super) fn quitar(raiz: &Path, id: &str) {
    let _ = std::fs::remove_file(fichero(raiz, id));
}

/// **El reconocedor fallo con ella**: un intento mas; tras [`INTENTOS`],
/// fuera. Devuelve si sigue en la cola.
pub(super) fn fallo(raiz: &Path, id: &str) -> bool {
    let f = fichero(raiz, id);
    let Some(mut p) = leer(&f) else {
        return false;
    };
    p.intentos += 1;
    if p.intentos >= INTENTOS {
        tracing::info!(id, "la nota se deja sin texto tras varios intentos");
        let _ = std::fs::remove_file(&f);
        return false;
    }
    escribir(&f, &p);
    true
}

fn leer(f: &Path) -> Option<Pendiente> {
    serde_json::from_slice(&std::fs::read(f).ok()?).ok()
}

/// Lo que espera, en el orden de sus nombres (lo que no se entiende, fuera).
pub(super) fn pendientes(raiz: &Path) -> Vec<Pendiente> {
    let Ok(dir) = std::fs::read_dir(carpeta(raiz)) else {
        return Vec::new();
    };
    let mut v: Vec<(PathBuf, Pendiente)> = Vec::new();
    for e in dir.flatten() {
        let ruta = e.path();
        if ruta.extension().is_none_or(|x| x != "json") {
            continue;
        }
        match leer(&ruta) {
            Some(p) => v.push((ruta, p)),
            None => {
                let _ = std::fs::remove_file(&ruta);
            }
        }
    }
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v.into_iter().map(|(_, p)| p).collect()
}

/// **Retoma lo que quedo** (en el hilo de quien llama: el de recuperar al
/// arrancar). `antes`: lo que habia antes de recuperar las grabaciones
/// cortadas (esas ya se pasan a texto solas).
pub(super) fn retomar(pedido: &Pedido, antes: Vec<Pendiente>) {
    let raiz = pedido.ubicacion.raiz().to_path_buf();
    for p in antes {
        let voz = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&raiz, &p.proyecto))
            .ok()
            .and_then(|c| c.mensajes.into_iter().find(|m| m.id == p.id));
        let Some(voz) = voz else {
            // La nota ya no esta (se borro, el chat se fue).
            quitar(&raiz, &p.id);
            continue;
        };
        if voz
            .transcripcion
            .as_deref()
            .is_some_and(|t| !t.trim().is_empty())
        {
            quitar(&raiz, &p.id);
            continue;
        }
        let Some(Fase::Texto { id, mut en_marcha }) = empezar_el_texto(pedido, &p.proyecto, &voz)
        else {
            // Sin reconocedor (o sin el audio todavia): a la proxima.
            tracing::info!(id = %p.id, "la nota sigue esperando su texto");
            continue;
        };
        tracing::info!(id = %p.id, "se retoma el texto de una clase");
        let salida = loop {
            if let Some(s) = en_marcha.recoger() {
                break s;
            }
            std::thread::sleep(Duration::from_millis(250));
        };
        match salida
            .map_err(|e| anyhow::anyhow!("{e:?}"))
            .and_then(|t| escribir_el_texto(pedido, &p.proyecto, &id, &t))
        {
            Ok(()) => quitar(&raiz, &p.id),
            Err(e) => {
                tracing::info!(?e, id = %p.id, "el texto retomado no salio");
                fallo(&raiz, &p.id);
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn raiz(nombre: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!(
            "pixpin-cola-de-texto-{nombre}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        r
    }

    #[test]
    fn se_apunta_se_quita_y_tras_tres_fallos_se_deja() {
        let r = raiz("apuntar");
        assert!(pendientes(&r).is_empty());
        apuntar(&r, "p1", "v-2");
        apuntar(&r, "p1", "v-1");
        let v = pendientes(&r);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].id, "v-1");
        // Apuntarla otra vez no le borra los intentos.
        assert!(fallo(&r, "v-1"));
        apuntar(&r, "p1", "v-1");
        assert_eq!(pendientes(&r)[0].intentos, 1);
        assert!(fallo(&r, "v-1"));
        // Caso negativo: al tercer fallo ya no sigue.
        assert!(!fallo(&r, "v-1"));
        assert_eq!(pendientes(&r).len(), 1);
        quitar(&r, "v-2");
        assert!(pendientes(&r).is_empty());
        // Fallar con una que no esta no la crea.
        assert!(!fallo(&r, "nada"));
        assert!(pendientes(&r).is_empty());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn lo_que_no_se_entiende_se_tira() {
        let r = raiz("roto");
        std::fs::create_dir_all(carpeta(&r)).unwrap();
        std::fs::write(carpeta(&r).join("x.json"), b"no es json").unwrap();
        std::fs::write(carpeta(&r).join("otro.txt"), b"{}").unwrap();
        assert!(pendientes(&r).is_empty());
        assert!(!carpeta(&r).join("x.json").exists());
        // Lo que no es de la cola no se toca.
        assert!(carpeta(&r).join("otro.txt").exists());
        let _ = std::fs::remove_dir_all(&r);
    }
}
