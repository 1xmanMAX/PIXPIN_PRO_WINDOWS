//! Una vuelta con otro aparato, sin pantalla.
//!
//! Lo que hace `SincronizarActivity.unaVuelta` entre que se conecta y que se
//! despide: resolver los proyectos borrados, elegir (o no) que chats, y para
//! cada uno los dos pasos de la [Sesion] contando lo que va pasando. La
//! pantalla de cada aparato solo pone la cara: lo que dice y como se decide
//! es esto, igual en el movil y en el PC.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{Read, Write};

use crate::disco::Disco;
use crate::mensajes::{Aparato, Chat};
use crate::protocolo::{Hecho, Resultado, Sesion};

/// Un chat en la lista de elegir: donde esta y cuando se toco en cada lado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fila {
    pub id: String,
    pub nombre: String,
    pub aqui: Option<Chat>,
    pub alli: Option<Chat>,
    pub marcado: bool,
}

impl Fila {
    /// Lo que se ensena debajo del nombre en la pestana de un lado
    /// (`esMio`): si esta en el otro y cual es mas reciente.
    pub fn estado(&self, es_mio: bool, otro: &str) -> (String, bool) {
        let (suyo, del_otro) = if es_mio {
            (&self.aqui, &self.alli)
        } else {
            (&self.alli, &self.aqui)
        };
        let t = suyo.as_ref().map_or(0, |c| c.tocado);
        match del_otro {
            None => (format!("Solo aquí · se copiará a {otro}"), false),
            Some(o) if t > o.tocado => ("Más reciente".into(), true),
            Some(o) if t < o.tocado => ("Anterior".into(), false),
            Some(_) => ("Igual en los dos".into(), false),
        }
    }
}

/// Lo que se ensena mientras trabaja (`Fase.Trabajando`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trabajo {
    pub texto: String,
    pub hechos: u64,
    pub total: u64,
    pub velocidad: String,
}

impl Trabajo {
    pub fn solo(texto: String) -> Trabajo {
        Trabajo {
            texto,
            ..Default::default()
        }
    }
}

/// Lo que salio de una vuelta (`Vuelta`).
#[derive(Debug, Clone, Default)]
pub struct Vuelta {
    pub nombre: String,
    pub bytes: u64,
    pub segundos: f64,
    pub avisos: Vec<String>,
    pub cancelada: bool,
}

/// Que chats: con `Some`, se pregunta (la pantalla de «¿Que sincronizar?»);
/// devuelve None si se cancela. Sin ella se va con lo elegido la ultima vez
/// con ese aparato, o con todo la primera vez.
pub type Elegir<'a> = &'a mut dyn FnMut(&Aparato, &[Fila]) -> Option<BTreeSet<String>>;

/// **Los proyectos borrados, antes de sincronizar nada** (`resolverBorrados`).
/// Devuelve los chats que quedan borrados, que son los que no hay que
/// sincronizar: hacerlo los resucitaria por sus mensajes.
pub fn resolver_borrados<D: Disco + ?Sized, F: Read + Write>(
    s: &mut Sesion<'_, D, F>,
    mios: &HashMap<String, Chat>,
    suyos: &HashMap<String, Chat>,
    ahora: i64,
    progreso: &mut dyn FnMut(Trabajo),
) -> Resultado<HashSet<String>> {
    let disco = s.disco();
    let mis: Vec<_> = disco.lapidas();
    let sus = s.lapidas()?;
    let mut fuera = HashSet::new();
    if mis.is_empty() && sus.is_empty() {
        return Ok(fuera);
    }
    // El reloj del otro puede ir desviado: sus horas se pasan a la de aqui
    // restando el desfase antes de comparar.
    let desfase = s.desfase;
    for l in &mis {
        let Some(suyo) = suyos.get(&l.chat) else {
            fuera.insert(l.chat.clone());
            continue;
        };
        if suyo.tocado - desfase > l.cuando {
            disco.quitar_lapida(&l.chat)?;
        } else {
            progreso(Trabajo::solo(format!(
                "Borrando «{}» en {}…",
                suyo.nombre, s.otro.nombre
            )));
            s.borrar_alla(&l.chat)?;
            fuera.insert(l.chat.clone());
        }
    }
    for l in &sus {
        if mis.iter().any(|m| m.chat == l.chat) {
            fuera.insert(l.chat.clone());
            continue;
        }
        let Some(mio) = mios.get(&l.chat) else {
            continue;
        };
        if mio.tocado > l.cuando - desfase {
            continue;
        }
        progreso(Trabajo::solo(format!(
            "Borrando «{}», borrado en {}…",
            mio.nombre, s.otro.nombre
        )));
        disco.borrar_chat(
            &l.chat,
            &format!("Antes de borrarlo, borrado en {}", s.otro.nombre),
            ahora,
            &s.otro.id,
        )?;
        fuera.insert(l.chat.clone());
    }
    Ok(fuera)
}

/// **Una vuelta con un aparato ya conectado.** `rotulo` va delante de lo que
/// se ensena mientras trabaja, para saber por donde va una ronda.
pub fn una<D: Disco + ?Sized, F: Read + Write>(
    s: &mut Sesion<'_, D, F>,
    elegir: Option<Elegir<'_>>,
    hecho: &mut Hecho,
    rotulo: &str,
    ahora: &dyn Fn() -> i64,
    progreso: &mut dyn FnMut(Trabajo),
) -> Resultado<Vuelta> {
    let disco = s.disco();
    let otro = s.otro.clone();
    let mios: HashMap<String, Chat> = disco
        .chats()?
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();
    let orden_mio: Vec<String> = disco.chats()?.into_iter().map(|c| c.id).collect();
    let catalogo = s.catalogo()?;
    let suyos: HashMap<String, Chat> = catalogo.iter().map(|c| (c.id.clone(), c.clone())).collect();
    let antes = disco.elegidos(&otro.id);
    let borrados = resolver_borrados(s, &mios, &suyos, ahora(), progreso)?;
    let mut ids: Vec<String> = Vec::new();
    for id in orden_mio.iter().chain(catalogo.iter().map(|c| &c.id)) {
        if !ids.contains(id) && !borrados.contains(id) {
            ids.push(id.clone());
        }
    }
    let filas: Vec<Fila> = ids
        .iter()
        .map(|id| {
            let m = mios.get(id).cloned();
            let su = suyos.get(id).cloned();
            Fila {
                id: id.clone(),
                nombre: m
                    .as_ref()
                    .or(su.as_ref())
                    .map(|c| c.nombre.clone())
                    .unwrap_or_default(),
                aqui: m,
                alli: su,
                marcado: antes.as_ref().is_none_or(|a| a.contains(id)),
            }
        })
        .collect();
    let elegidos: BTreeSet<String> = match elegir {
        Some(e) => match e(&otro, &filas) {
            None => {
                s.adios();
                return Ok(Vuelta {
                    nombre: otro.nombre,
                    cancelada: true,
                    ..Default::default()
                });
            }
            Some(puesto) => {
                disco.guardar_elegidos(&otro.id, &puesto)?;
                puesto
            }
        },
        None => antes.unwrap_or_else(|| ids.iter().cloned().collect()),
    };

    let empezo = std::time::Instant::now();
    for chat in ids.iter().filter(|c| elegidos.contains(*c)) {
        let nombre = filas
            .iter()
            .find(|f| f.id == *chat)
            .map(|f| f.nombre.clone())
            .unwrap_or_default();
        progreso(Trabajo::solo(format!(
            "{rotulo}Comparando «{nombre}» con {}…",
            otro.nombre
        )));
        let prep = s.preparar(chat)?;
        progreso(Trabajo::solo(format!("{rotulo}Juntando «{nombre}»…")));
        s.aplicar(&prep, hecho)?;
        let archivos = s.preparar_archivos(&prep)?;
        let total = archivos.bytes();
        let mut hechos = 0u64;
        let t0 = std::time::Instant::now();
        let mut ultimo = std::time::Instant::now() - std::time::Duration::from_secs(1);
        s.aplicar_archivos(&archivos, hecho, &mut |n| {
            hechos += n;
            if ultimo.elapsed().as_millis() > 150 {
                ultimo = std::time::Instant::now();
                let seg = t0.elapsed().as_secs_f64().max(0.001);
                progreso(Trabajo {
                    texto: format!("{rotulo}Pasando archivos de «{nombre}»"),
                    hechos,
                    total,
                    velocidad: format!("{}/s", tamano_legible((hechos as f64 / seg) as i64)),
                });
            }
        })?;
        progreso(Trabajo::solo(format!("{rotulo}Terminando «{nombre}»…")));
        s.cerrar(&prep, ahora())?;
    }
    s.adios();
    let mut avisos = Vec::new();
    if !hecho.saltados.is_empty() {
        let mut vistos = Vec::new();
        for n in &hecho.saltados {
            if !vistos.contains(n) {
                vistos.push(n.clone());
            }
        }
        let lista: Vec<String> = vistos.iter().map(|n| format!("«{n}»")).collect();
        avisos.push(format!(
            "No se pasó {} porque se estaba guardando en ese momento. Vuelve a sincronizar.",
            lista.join(", ")
        ));
    }
    if let Some(a) = aviso_del_reloj(&otro, s.desfase) {
        avisos.push(a);
    }
    Ok(Vuelta {
        nombre: otro.nombre,
        bytes: s.enviados() + s.recibidos(),
        segundos: empezo.elapsed().as_secs_f64(),
        avisos,
        cancelada: false,
    })
}

/// `avisoDelReloj`: con la hora mal, lo que llega se coloca en el chat donde
/// no toca.
pub fn aviso_del_reloj(otro: &Aparato, desfase: i64) -> Option<String> {
    let minutos = desfase.abs() / 60_000;
    if minutos < 2 {
        return None;
    }
    let sentido = if desfase > 0 {
        "adelantado"
    } else {
        "atrasado"
    };
    Some(format!(
        "El reloj de {} va {minutos} min {sentido}. Pon «fecha y hora automáticas» en los dos: con la hora mal, lo que llega se coloca en el chat donde no toca.",
        otro.nombre
    ))
}

/// `avisosDeLoHecho`: lo rescatado y lo que no hizo falta mandar.
pub fn avisos_de_lo_hecho(h: &Hecho) -> Vec<String> {
    let mut v = Vec::new();
    if h.rescatados > 0 {
        v.push(if h.rescatados == 1 {
            "1 cosa borrada en un aparato se quedó porque en el otro se había cambiado después."
                .to_string()
        } else {
            format!(
                "{} cosas borradas en un aparato se quedaron porque en el otro se había cambiado después.",
                h.rescatados
            )
        });
    }
    if h.ahorrados > 64_000 {
        v.push(format!(
            "Solo viajaron los cambios: {} que no hizo falta mandar.",
            tamano_legible(h.ahorrados)
        ));
    }
    v
}

/// `contarLoHecho`: lo que se le cuenta al usuario al acabar.
pub fn contar_lo_hecho(h: &Hecho, bytes: u64, segundos: f64) -> String {
    let mut partes = Vec::new();
    if h.traidos > 0 {
        partes.push(format!("{} traídos", h.traidos));
    }
    if h.enviados > 0 {
        partes.push(format!("{} enviados", h.enviados));
    }
    if h.borrados > 0 {
        partes.push(format!("{} borrados", h.borrados));
    }
    if h.archivos > 0 {
        partes.push(format!("{} archivos", h.archivos));
    }
    if h.fusionados > 0 {
        partes.push(format!("{} juntados de los dos", h.fusionados));
    }
    let primera = if partes.is_empty() {
        "Ya estaban iguales.".to_string()
    } else {
        format!("{}.", partes.join(" · "))
    };
    let tam = tamano_legible(bytes as i64);
    format!(
        "{primera}\n{} en {} s.",
        if tam.is_empty() { "0 B".into() } else { tam },
        decimal(segundos)
    )
}

/// `tamanoLegible`: en kilobytes de mil, con coma decimal como el movil en
/// espanol.
pub fn tamano_legible(bytes: i64) -> String {
    match bytes {
        b if b <= 0 => String::new(),
        b if b < 1_000 => format!("{b} B"),
        b if b < 1_000_000 => format!("{} kB", b / 1_000),
        b if b < 1_000_000_000 => format!("{} MB", decimal(b as f64 / 1_000_000.0)),
        b => format!("{} GB", decimal(b as f64 / 1_000_000_000.0)),
    }
}

fn decimal(x: f64) -> String {
    format!("{x:.1}").replace('.', ",")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_tamano_se_escribe_como_en_el_movil() {
        assert_eq!(tamano_legible(0), "");
        assert_eq!(tamano_legible(999), "999 B");
        assert_eq!(tamano_legible(4_500), "4 kB");
        assert_eq!(tamano_legible(2_300_000), "2,3 MB");
    }

    #[test]
    fn lo_hecho_se_cuenta_como_en_el_movil() {
        let h = Hecho {
            traidos: 2,
            archivos: 1,
            ..Default::default()
        };
        assert_eq!(
            contar_lo_hecho(&h, 2_300_000, 1.25),
            "2 traídos · 1 archivos.\n2,3 MB en 1,2 s."
        );
        assert_eq!(
            contar_lo_hecho(&Hecho::default(), 0, 0.0),
            "Ya estaban iguales.\n0 B en 0,0 s."
        );
    }

    #[test]
    fn cada_fila_dice_cual_es_mas_reciente() {
        let c = |t| Chat {
            id: "p".into(),
            nombre: "P".into(),
            mensajes: 0,
            tocado: t,
        };
        let f = Fila {
            id: "p".into(),
            nombre: "P".into(),
            aqui: Some(c(5)),
            alli: Some(c(3)),
            marcado: true,
        };
        assert_eq!(f.estado(true, "Móvil"), ("Más reciente".into(), true));
        assert_eq!(f.estado(false, "PC"), ("Anterior".into(), false));
        let solo = Fila { alli: None, ..f };
        assert_eq!(
            solo.estado(true, "Móvil").0,
            "Solo aquí · se copiará a Móvil"
        );
    }
}
