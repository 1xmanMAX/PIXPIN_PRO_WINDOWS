//! **Juntar lo de dos aparatos.**
//!
//! Las reglas, las mismas en todos:
//!
//! - Las pegatinas se juntan **por id**: la que solo tiene un lado entra.
//! - Lo de la pegatina (tipo, sitio, resuelto, quitada, tarea): **gana el
//!   ultimo cambio** (`cambiado`). A igualdad de hora, la de texto JSON mayor
//!   (asi los dos aparatos eligen la misma sin hablar).
//! - El hilo se **suma entrada a entrada** por id; si los dos tocaron la
//!   misma entrada, gana la de `cambiado` mayor (con el mismo desempate). Se
//!   ordena por `cuando` (y por id).
//! - **Lo nuevo gana a lo quitado**: una pegatina quitada en un lado vuelve
//!   si en el otro se le escribio despues de quitarla (solo con lo escrito
//!   despues: lo de antes ya se habia quitado).
//!
//! Como todo lleva su hora y nada se borra (lapidas), juntar no necesita la
//! version comun de antes: es conmutativo, asociativo e idempotente.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::fichero;
use crate::modelo::{Comentario, Entrada, Pegatinas};

fn json<T: serde::Serialize>(t: &T) -> String {
    serde_json::to_string(t).unwrap_or_default()
}

/// Si `a` gana a `b` (mas nueva, o a igualdad la de JSON mayor).
fn gana<T: serde::Serialize>(a: &T, hora_a: i64, b: &T, hora_b: i64) -> bool {
    hora_a > hora_b || (hora_a == hora_b && json(a) >= json(b))
}

/// La cabeza de una pegatina (sin su hilo), para desempatar.
fn cabeza(c: &Comentario) -> Comentario {
    Comentario {
        hilo: Vec::new(),
        ..c.clone()
    }
}

fn juntar_hilos(a: &[Entrada], b: &[Entrada]) -> Vec<Entrada> {
    let mut por_id: BTreeMap<&str, &Entrada> = BTreeMap::new();
    for e in a.iter().chain(b) {
        match por_id.get(e.id.as_str()) {
            Some(otra) if !gana(e, e.cambiado, *otra, otra.cambiado) => {}
            _ => {
                por_id.insert(e.id.as_str(), e);
            }
        }
    }
    let mut v: Vec<Entrada> = por_id.into_values().cloned().collect();
    v.sort_by(|x, y| x.cuando.cmp(&y.cuando).then_with(|| x.id.cmp(&y.id)));
    v
}

fn juntar_una(a: &Comentario, b: &Comentario) -> Comentario {
    let (ca, cb) = (cabeza(a), cabeza(b));
    let (ganadora, otra) = if gana(&ca, a.cambiado, &cb, b.cambiado) {
        (a, b)
    } else {
        (b, a)
    };
    let mut c = Comentario {
        hilo: juntar_hilos(&a.hilo, &b.hilo),
        ..cabeza(ganadora)
    };
    // Lo nuevo gana a lo quitado: algo escrito en el otro despues de
    // quitarla la devuelve.
    if c.borrado
        && !otra.borrado
        && otra
            .hilo
            .iter()
            .any(|e| !e.borrado && e.cuando > ganadora.cambiado)
    {
        c.borrado = false;
        c.cambiado = c.cambiado.max(otra.ultimo());
    }
    c
}

/// **Junta dos listas de pegatinas.** El orden: el de creacion (y el id).
pub fn juntar(a: &Pegatinas, b: &Pegatinas) -> Pegatinas {
    let mut por_id: BTreeMap<&str, Comentario> = BTreeMap::new();
    for c in a.lista.iter().chain(&b.lista) {
        let junta = match por_id.get(c.id.as_str()) {
            Some(otra) => juntar_una(otra, c),
            None => c.clone(),
        };
        por_id.insert(c.id.as_str(), junta);
    }
    let mut lista: Vec<Comentario> = por_id.into_values().collect();
    lista.sort_by(|x, y| x.creado.cmp(&y.creado).then_with(|| x.id.cmp(&y.id)));
    Pegatinas { lista }
}

/// **Lo que llega por la sincronizacion, junto con lo de aqui** (los dos
/// ficheros `.comentarios.json` enteros). `None` si no hace falta cambiar
/// lo que llega (ya lo trae todo, o alguno no se entiende: entonces manda
/// lo de siempre, gana el mas nuevo). Lo que no son pegatinas (los
/// comentarios de una nota) es lo que llega, como hasta ahora.
pub fn al_llegar(aqui: &str, llega: &str) -> Option<String> {
    let (Ok(mio), Ok(suyo)) = (fichero::leer(aqui), fichero::leer(llega)) else {
        return None;
    };
    let junta = juntar(&mio, &suyo);
    let mismo =
        serde_json::to_value(&junta.lista).ok()? == serde_json::to_value(&suyo.lista).ok()?;
    if mismo {
        return None;
    }
    fichero::escribir(llega, &junta).ok()
}

/// Si `t` lleva pegatinas (para no tocar los comentarios de una nota que
/// no las tiene).
pub fn lleva_pegatinas(t: &str) -> bool {
    serde_json::from_str::<Value>(t)
        .ok()
        .and_then(|v| {
            v.get(fichero::CLAVE)
                .and_then(Value::as_array)
                .map(|a| !a.is_empty())
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::ancla::Ancla;
    use crate::modelo::{Quien, Tipo};

    fn q(a: &str) -> Quien {
        Quien {
            autor: a.into(),
            aparato: a.into(),
        }
    }

    fn base() -> (Pegatinas, String) {
        let mut p = Pegatinas::default();
        let id = p.poner(Tipo::Media, Ancla::pagina(0, 10.0, 10.0), &q("pc"), 100);
        p.escribir(&id, &q("pc"), 110, "hola", vec![]);
        (p, id)
    }

    #[test]
    fn dos_aparatos_escriben_a_la_vez_y_quedan_las_dos_entradas() {
        let (b, id) = base();
        let mut pc = b.clone();
        let mut movil = b.clone();
        pc.escribir(&id, &q("pc"), 200, "desde el PC", vec![]);
        movil.escribir(&id, &q("and"), 150, "desde el movil", vec![]);
        // Y cada uno cambia algo de la pegatina: gana el ultimo.
        movil.cambiar_tipo(&id, Tipo::Urgente, 300);
        pc.mover(&id, Ancla::pagina(0, 50.0, 50.0), 250);
        let j = juntar(&pc, &movil);
        let c = j.get(&id).unwrap();
        let textos: Vec<&str> = c.entradas().map(|e| e.texto.as_str()).collect();
        assert_eq!(textos, ["hola", "desde el movil", "desde el PC"]);
        assert_eq!(c.tipo, Tipo::Urgente);
        // El sitio es de la cabeza: la del movil (300) gana entera.
        assert_eq!(c.ancla, Ancla::pagina(0, 10.0, 10.0));
        // Conmutativa e idempotente.
        assert_eq!(juntar(&movil, &pc), j);
        assert_eq!(juntar(&j, &j), j);
        assert_eq!(juntar(&j, &pc), j);
    }

    #[test]
    fn lo_quitado_no_vuelve_salvo_que_se_escriba_despues() {
        let (b, id) = base();
        let mut pc = b.clone();
        pc.quitar(&id, 500);
        // El movil no se entero y sigue igual: queda quitada.
        let j = juntar(&pc, &b);
        assert!(j.get(&id).is_none());
        assert_eq!(j.lista.len(), 1, "la lapida se queda");
        // El movil escribio despues de quitarla: vuelve, solo con eso.
        let mut movil = b.clone();
        movil.escribir(&id, &q("and"), 600, "aun no!", vec![]);
        let j = juntar(&pc, &movil);
        let c = j.get(&id).expect("vuelve");
        let textos: Vec<&str> = c.entradas().map(|e| e.texto.as_str()).collect();
        assert_eq!(textos, ["aun no!"]);
        assert_eq!(juntar(&movil, &pc), j);
        // Caso negativo: lo escrito ANTES de quitarla no la devuelve.
        let mut viejo = b.clone();
        viejo.escribir(&id, &q("and"), 400, "antes", vec![]);
        assert!(juntar(&pc, &viejo).get(&id).is_none());
    }

    #[test]
    fn cada_lado_pone_las_suyas() {
        let mut a = Pegatinas::default();
        let mut b = Pegatinas::default();
        a.poner(Tipo::Nota, Ancla::escena(0.0, 0.0), &q("pc"), 1);
        b.poner(Tipo::Alta, Ancla::escena(5.0, 5.0), &q("and"), 2);
        let j = juntar(&a, &b);
        assert_eq!(j.vivas().count(), 2);
        assert_eq!(j.lista[0].aparato, "pc");
    }

    #[test]
    fn al_llegar_solo_reescribe_si_aqui_habia_algo_que_no_trae() {
        let (b, id) = base();
        let llega = fichero::escribir(r#"{"comentarios":[{"id":"n1"}]}"#, &b).unwrap();
        let mut aqui = b.clone();
        // Lo de aqui ya lo trae: no se toca.
        assert_eq!(
            al_llegar(&fichero::escribir("", &aqui).unwrap(), &llega),
            None
        );
        aqui.escribir(&id, &q("pc"), 900, "solo aqui", vec![]);
        let t = al_llegar(&fichero::escribir("", &aqui).unwrap(), &llega).unwrap();
        let p = fichero::leer(&t).unwrap();
        assert_eq!(p.get(&id).unwrap().entradas().count(), 2);
        // Los comentarios de la nota son los que llegan.
        assert!(t.contains("\"n1\""));
        // Caso negativo: lo roto no se junta.
        assert_eq!(al_llegar("{roto", &llega), None);
        assert_eq!(al_llegar(&llega, "{roto"), None);
        assert!(lleva_pegatinas(&llega));
        assert!(!lleva_pegatinas(r#"{"comentarios":[]}"#));
    }
}
