//! **Las recetas del asistente** (13-oct-2026): los pedidos que el usuario
//! repite («Resume lo último en 5 puntos», «¿Qué pone en esta foto?»),
//! guardados como botones encima de la caja de escribir del chat
//! «Asistente». Son del usuario y viajan **solo entre sus aparatos**, como
//! las listas de los chats compartidos (`compartidos`): nunca a una persona
//! de un chat compartido (`protocolo::solo_del_grupo`).
//!
//! ## El fichero (`asistente-recetas.json`, en la raiz de los datos)
//!
//! ```json
//! {"version":1,"recetas":[
//!   {"id":"r-mgp3k2-7f","nombre":"Resumir","icono":"📝",
//!    "pedido":"Resume lo último en 5 puntos","orden":0,"cambiada":1760390000000},
//!   {"id":"r-mgp3k9-02","nombre":"","icono":"","pedido":"","orden":1,
//!    "cambiada":1760390100000,"borrada":true}
//! ]}
//! ```
//!
//! - `id`: unico y para siempre; `cambiada`: ms UTC de la ultima vez que se
//!   toco (nombre, icono, pedido, orden o borrarla).
//! - `orden`: de menor a mayor, de izquierda a derecha.
//! - **Borrar deja una lapida** (`borrada: true`, sin textos): asi el otro
//!   aparato se entera en vez de devolverla. Las lapidas de mas de
//!   [`LAPIDA_DIAS`] dias se olvidan.
//!
//! ## Juntar ([`juntar`])
//!
//! Por `id`, **gana la cambiada mas tarde**; a igual hora, la lapida (borrar
//! es lo ultimo que se quiso), y si aun empatan, la de texto mayor (las dos
//! partes eligen la misma sin hablar). Es conmutativo e idempotente: da
//! igual quien empiece la vuelta o cuantas veces se repita.
//!
//! ## En el cable (sin subir la version del protocolo)
//!
//! Una peticion `recetas` con las mias en `parche` (CADENA con el JSON de
//! la lista); el otro junta, guarda y contesta lo juntado en `parche`, que
//! guardo yo. Un aparato de antes contesta «No sé qué es «recetas»» y la
//! vuelta sigue sin ellas.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::disco::escribir_atomico;

/// El fichero, en la raiz de los datos (junto a `galeria-compartida.json`).
pub const FICHERO: &str = "asistente-recetas.json";
/// La peticion del protocolo.
pub const PETICION: &str = "recetas";
/// La version del formato del fichero.
pub const VERSION: u32 = 1;
/// Cuanto se recuerda una receta borrada.
pub const LAPIDA_DIAS: i64 = 180;
/// Lo mas largo que se guarda de cada campo (un pedido es una frase, no un
/// documento).
pub const TOPE_NOMBRE: usize = 40;
pub const TOPE_PEDIDO: usize = 2000;
/// Cuantas recetas vivas como mucho (es una fila de botones).
pub const MAXIMO: usize = 40;

const DIA_MS: i64 = 86_400_000;

fn es_falso(b: &bool) -> bool {
    !*b
}

/// Una receta.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receta {
    pub id: String,
    #[serde(default)]
    pub nombre: String,
    /// Un emoji (o vacio).
    #[serde(default)]
    pub icono: String,
    /// Lo que se le pide al asistente al pulsarla.
    #[serde(default)]
    pub pedido: String,
    #[serde(default)]
    pub orden: i64,
    /// Ms UTC de la ultima vez que se toco.
    #[serde(default)]
    pub cambiada: i64,
    #[serde(default, skip_serializing_if = "es_falso")]
    pub borrada: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Recetario {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    recetas: Vec<Receta>,
}

pub fn ruta(raiz: &Path) -> PathBuf {
    raiz.join(FICHERO)
}

/// Todas, lapidas incluidas. Sin fichero, ninguna. Un fichero roto es un
/// error (y no se pisa: lo que se guarde despues lo apartara antes).
pub fn leer(raiz: &Path) -> io::Result<Vec<Receta>> {
    match std::fs::read_to_string(ruta(raiz)) {
        Ok(t) => de_texto(&t).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "recetas que no se entienden")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// Guarda todas (escritura atomica). Si el fichero que habia estaba roto,
/// se aparta antes con `.roto-<ms>` para no perderlo del todo.
pub fn guardar(raiz: &Path, recetas: &[Receta]) -> io::Result<()> {
    let r = ruta(raiz);
    if let Ok(t) = std::fs::read_to_string(&r)
        && de_texto(&t).is_none()
    {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let _ = std::fs::rename(&r, raiz.join(format!("{FICHERO}.roto-{ms}")));
    }
    std::fs::create_dir_all(raiz)?;
    escribir_atomico(&r, a_texto(recetas).as_bytes())
}

/// El JSON del fichero (y del cable).
pub fn a_texto(recetas: &[Receta]) -> String {
    serde_json::to_string(&Recetario {
        version: VERSION,
        recetas: recetas.to_vec(),
    })
    .unwrap_or_else(|_| "{\"version\":1,\"recetas\":[]}".into())
}

/// Del JSON; `None` si no se entiende. Las recetas sin `id` se descartan.
pub fn de_texto(t: &str) -> Option<Vec<Receta>> {
    let v: serde_json::Value = serde_json::from_str(t).ok()?;
    // Solo el objeto del fichero (serde aceptaria tambien una lista suelta).
    if !v.is_object() {
        return None;
    }
    let r: Recetario = serde_json::from_value(v).ok()?;
    Some(r.recetas.into_iter().filter(|x| !x.id.trim().is_empty()).map(limpia).collect())
}

/// Recorta lo que venga demasiado largo (de otro aparato o a mano).
fn limpia(mut r: Receta) -> Receta {
    r.nombre = r.nombre.replace('\n', " ").trim().chars().take(TOPE_NOMBRE).collect();
    r.icono = r.icono.trim().chars().take(8).collect();
    r.pedido = r.pedido.trim().chars().take(TOPE_PEDIDO).collect();
    if r.borrada {
        r.nombre.clear();
        r.icono.clear();
        r.pedido.clear();
    }
    r
}

/// Las que se ven, en su orden.
pub fn visibles(recetas: &[Receta]) -> Vec<Receta> {
    let mut v: Vec<Receta> = recetas.iter().filter(|r| !r.borrada).cloned().collect();
    v.sort_by(|a, b| a.orden.cmp(&b.orden).then(a.cambiada.cmp(&b.cambiada)).then(a.id.cmp(&b.id)));
    v
}

/// Cual de dos versiones de la misma receta gana.
fn gana<'a>(a: &'a Receta, b: &'a Receta) -> &'a Receta {
    use std::cmp::Ordering::*;
    match a.cambiada.cmp(&b.cambiada).then(a.borrada.cmp(&b.borrada)) {
        Greater => a,
        Less => b,
        Equal => {
            // Desempate que las dos partes calculan igual.
            if a_texto(std::slice::from_ref(a)) >= a_texto(std::slice::from_ref(b)) {
                a
            } else {
                b
            }
        }
    }
}

/// **Junta las mias y las suyas** (ver la cabecera). `ahora` olvida las
/// lapidas viejas. Sale ordenado por `id`, igual en los dos aparatos.
pub fn juntar(mias: &[Receta], suyas: &[Receta], ahora: i64) -> Vec<Receta> {
    let mut por_id: std::collections::BTreeMap<String, Receta> = std::collections::BTreeMap::new();
    for r in mias.iter().chain(suyas) {
        let r = limpia(r.clone());
        match por_id.get(&r.id) {
            Some(otra) => {
                let g = gana(otra, &r).clone();
                por_id.insert(g.id.clone(), g);
            }
            None => {
                por_id.insert(r.id.clone(), r);
            }
        }
    }
    por_id
        .into_values()
        .filter(|r| !(r.borrada && ahora - r.cambiada > LAPIDA_DIAS * DIA_MS))
        .collect()
}

// ------------------------------------------------------------ cambiarlas

/// Un id nuevo: la hora en base 36 y unos bits al azar (dos aparatos que
/// crean a la vez no chocan).
fn id_nuevo(ahora: i64) -> String {
    let mut azar = [0u8; 4];
    let _ = getrandom_bytes(&mut azar);
    format!("r-{}-{:02x}{:02x}{:02x}{:02x}", base36(ahora.max(0) as u64), azar[0], azar[1], azar[2], azar[3])
}

fn getrandom_bytes(b: &mut [u8]) -> Result<(), ()> {
    use rand_core::{OsRng, RngCore};
    OsRng.try_fill_bytes(b).map_err(|_| ())
}

fn base36(mut n: u64) -> String {
    const D: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".into();
    }
    let mut s = Vec::new();
    while n > 0 {
        s.push(D[(n % 36) as usize]);
        n /= 36;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// Lo que puede salir mal al cambiarlas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoSePuede {
    /// Un pedido vacio no es una receta.
    Vacio,
    /// Ya hay [`MAXIMO`].
    Llenas,
    /// No hay ninguna con ese id (o esta borrada).
    NoEsta,
}

/// **Una receta nueva**, al final de la fila. Sin nombre, el principio del
/// pedido; sin icono, uno segun lo que pide ([`icono_para`]).
pub fn anadir(recetas: &mut Vec<Receta>, nombre: &str, icono: &str, pedido: &str, ahora: i64) -> Result<Receta, NoSePuede> {
    let pedido = pedido.trim();
    if pedido.is_empty() {
        return Err(NoSePuede::Vacio);
    }
    if visibles(recetas).len() >= MAXIMO {
        return Err(NoSePuede::Llenas);
    }
    let orden = recetas.iter().filter(|r| !r.borrada).map(|r| r.orden + 1).max().unwrap_or(0);
    let nombre = if nombre.trim().is_empty() { nombre_para(pedido) } else { nombre.trim().to_string() };
    let icono = if icono.trim().is_empty() { icono_para(pedido).to_string() } else { icono.trim().to_string() };
    let r = limpia(Receta {
        id: id_nuevo(ahora),
        nombre,
        icono,
        pedido: pedido.to_string(),
        orden,
        cambiada: ahora,
        borrada: false,
    });
    recetas.push(r.clone());
    Ok(r)
}

/// Cambia una receta viva y la sella con `ahora`.
pub fn cambiar(recetas: &mut [Receta], id: &str, ahora: i64, f: impl FnOnce(&mut Receta)) -> Result<(), NoSePuede> {
    let r = recetas.iter_mut().find(|r| r.id == id && !r.borrada).ok_or(NoSePuede::NoEsta)?;
    f(r);
    *r = limpia(std::mem::take(r));
    if r.pedido.is_empty() {
        return Err(NoSePuede::Vacio);
    }
    r.cambiada = ahora;
    Ok(())
}

/// La borra dejando su lapida.
pub fn borrar(recetas: &mut [Receta], id: &str, ahora: i64) -> Result<(), NoSePuede> {
    let r = recetas.iter_mut().find(|r| r.id == id && !r.borrada).ok_or(NoSePuede::NoEsta)?;
    r.borrada = true;
    r.cambiada = ahora;
    *r = limpia(std::mem::take(r));
    Ok(())
}

/// La mueve `paso` sitios en la fila (−1: a la izquierda). Renumera las
/// vivas de 0 en adelante y sella solo las que cambian de numero.
pub fn mover(recetas: &mut [Receta], id: &str, paso: i32, ahora: i64) -> Result<(), NoSePuede> {
    let mut fila: Vec<String> = visibles(recetas).into_iter().map(|r| r.id).collect();
    let i = fila.iter().position(|x| x == id).ok_or(NoSePuede::NoEsta)?;
    let j = (i as i64 + paso as i64).clamp(0, fila.len() as i64 - 1) as usize;
    let quien = fila.remove(i);
    fila.insert(j, quien);
    for (n, id) in fila.iter().enumerate() {
        if let Some(r) = recetas.iter_mut().find(|r| &r.id == id)
            && r.orden != n as i64
        {
            r.orden = n as i64;
            r.cambiada = ahora;
        }
    }
    Ok(())
}

/// El nombre de serie: las primeras palabras del pedido.
pub fn nombre_para(pedido: &str) -> String {
    let mut s = String::new();
    for p in pedido.split_whitespace() {
        if s.chars().count() + p.chars().count() + 1 > 22 {
            break;
        }
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(p.trim_matches(|c: char| !c.is_alphanumeric() && c != '¿' && c != '?'));
    }
    if s.is_empty() {
        s = pedido.chars().take(22).collect();
    }
    s
}

/// Los iconos que se ofrecen para elegir.
pub const ICONOS: [&str; 12] = ["⭐", "📝", "📊", "🎨", "🔎", "📄", "🖼️", "✅", "📅", "💡", "🌐", "🤖"];

/// El icono de serie, segun lo que se pide.
pub fn icono_para(pedido: &str) -> &'static str {
    let q = pedido.to_lowercase();
    let tiene = |v: &[&str]| v.iter().any(|p| q.contains(p));
    if tiene(&["resum", "apunt", "nota", "escrib", "redact"]) {
        "📝"
    } else if tiene(&["gráfic", "grafic", "barras", "tabla", "datos"]) {
        "📊"
    } else if tiene(&["dibuj", "lienzo", "pinta", "calca"]) {
        "🎨"
    } else if tiene(&["foto", "imagen", "describ", "qué pone", "que pone"]) {
        "🖼️"
    } else if tiene(&["lee", "pdf", "ocr", "texto"]) {
        "📄"
    } else if tiene(&["tarea", "lista", "pendiente"]) {
        "✅"
    } else if tiene(&["traduc", "idioma", "inglés", "ingles"]) {
        "🌐"
    } else {
        "⭐"
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn r(id: &str, nombre: &str, cambiada: i64) -> Receta {
        Receta {
            id: id.into(),
            nombre: nombre.into(),
            icono: "⭐".into(),
            pedido: format!("pedido de {nombre}"),
            orden: 0,
            cambiada,
            borrada: false,
        }
    }

    #[test]
    fn el_fichero_va_y_vuelve_igual() {
        let dir = std::env::temp_dir().join(format!("pixpin-recetas-fichero-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(leer(&dir).unwrap(), Vec::<Receta>::new(), "sin fichero, ninguna");
        let mut v = Vec::new();
        anadir(&mut v, "", "", "Resume lo último en 5 puntos", 1000).unwrap();
        anadir(&mut v, "Foto", "🖼️", "¿Qué pone en esta foto?", 2000).unwrap();
        let primera = v[0].id.clone();
        borrar(&mut v, &primera, 3000).unwrap();
        guardar(&dir, &v).unwrap();
        assert_eq!(leer(&dir).unwrap(), v);
        let t = std::fs::read_to_string(ruta(&dir)).unwrap();
        assert!(t.starts_with("{\"version\":1,\"recetas\":["), "{t}");
        assert!(t.contains("\"borrada\":true") && !t.contains("Resume"), "la lapida va sin texto: {t}");
        // Negativo: un fichero roto es error y no se pisa sin apartarlo.
        std::fs::write(ruta(&dir), "{roto").unwrap();
        assert!(leer(&dir).is_err());
        guardar(&dir, &v).unwrap();
        assert!(std::fs::read_dir(&dir).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().contains(".roto-")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn juntar_gana_lo_ultimo_y_da_igual_el_orden() {
        let mias = vec![r("a", "vieja", 10), r("b", "solo mia", 5)];
        let mut lapida = r("a", "", 20);
        lapida.borrada = true;
        let suyas = vec![r("a", "nueva", 15), r("c", "solo suya", 7)];
        let j1 = juntar(&mias, &suyas, 100);
        let j2 = juntar(&suyas, &mias, 100);
        assert_eq!(j1, j2, "conmutativo");
        assert_eq!(juntar(&j1, &j1, 100), j1, "idempotente");
        let nombres: Vec<&str> = j1.iter().map(|x| x.nombre.as_str()).collect();
        assert_eq!(nombres, ["nueva", "solo mia", "solo suya"]);
        // Borrar despues gana a cambiar antes, y viaja como lapida.
        let j = juntar(&j1, &[lapida.clone()], 100);
        assert!(j.iter().find(|x| x.id == "a").unwrap().borrada);
        assert_eq!(visibles(&j).len(), 2);
        // A la misma hora, la lapida gana.
        let mut empate = lapida.clone();
        empate.cambiada = 15;
        assert!(juntar(&suyas, &[empate], 100).iter().find(|x| x.id == "a").unwrap().borrada);
        // Las lapidas viejas se olvidan.
        assert!(juntar(&[lapida], &[], 20 + (LAPIDA_DIAS + 1) * DIA_MS).is_empty());
    }

    #[test]
    fn anadir_mover_y_cambiar() {
        let mut v = Vec::new();
        let a = anadir(&mut v, "", "", "Dibuja un plano de la casa", 1).unwrap();
        let b = anadir(&mut v, "B", "📊", "Gráfica de ventas", 2).unwrap();
        let c = anadir(&mut v, "C", "", "Lee este PDF", 3).unwrap();
        assert_eq!(a.icono, "🎨");
        assert_eq!(a.nombre, "Dibuja un plano de la");
        assert_eq!(c.icono, "📄");
        mover(&mut v, &c.id, -2, 10).unwrap();
        let fila: Vec<String> = visibles(&v).into_iter().map(|x| x.nombre).collect();
        assert_eq!(fila, ["C", "Dibuja un plano de la", "B"]);
        // Mover mas alla del borde se queda en el borde.
        mover(&mut v, &b.id, 5, 11).unwrap();
        assert_eq!(visibles(&v).last().unwrap().id, b.id);
        cambiar(&mut v, &b.id, 12, |r| r.nombre = "Ventas".into()).unwrap();
        assert_eq!(v.iter().find(|x| x.id == b.id).unwrap().cambiada, 12);
        // Negativos: vacio, una que no esta, y una borrada.
        assert_eq!(anadir(&mut v, "x", "", "   ", 13), Err(NoSePuede::Vacio));
        assert_eq!(cambiar(&mut v, &b.id, 14, |r| r.pedido.clear()), Err(NoSePuede::Vacio));
        assert_eq!(borrar(&mut v, "no-existe", 15), Err(NoSePuede::NoEsta));
        borrar(&mut v, &a.id, 16).unwrap();
        assert_eq!(mover(&mut v, &a.id, 1, 17), Err(NoSePuede::NoEsta));
        assert_eq!(cambiar(&mut v, &a.id, 18, |r| r.nombre = "z".into()), Err(NoSePuede::NoEsta));
        // Tope de recetas.
        let mut llenas = Vec::new();
        for i in 0..MAXIMO {
            anadir(&mut llenas, "", "", &format!("p{i}"), i as i64).unwrap();
        }
        assert_eq!(anadir(&mut llenas, "", "", "una mas", 99), Err(NoSePuede::Llenas));
    }

    /// **Ida y vuelta de verdad** entre dos aparatos del grupo (dos carpetas
    /// `files` hablando por un socket con el protocolo y el cifrado de
    /// verdad): una vuelta entera junta las recetas de los dos, lo borrado
    /// en uno se borra en el otro, y repetir no cambia nada.
    #[test]
    fn las_recetas_van_y_vuelven_entre_mis_aparatos() {
        use crate::disco::Disco;
        use crate::disco_android::DiscoAndroid;
        use crate::disco_android::prueba::{Reloj, conectado, crear_grupo, presentar};
        use crate::protocolo::Hecho;

        let dir = std::env::temp_dir().join(format!("pixpin-recetas-vuelta-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let tel = DiscoAndroid::nuevo(dir.join("telefono/files"), "tel");
        let pc = DiscoAndroid::nuevo(dir.join("pc/files"), "pc");
        presentar(&tel, "id-tel", "Teléfono");
        presentar(&pc, "id-pc", "PC");
        let reloj = Reloj::nuevo();
        crear_grupo(&tel, "ABCDE23456", reloj.ahora());
        conectado(&pc, &tel, true, Some("ABCDE23456"), &reloj, |_| {}).unwrap();

        let ahora = reloj.ahora();
        let mut en_tel = Vec::new();
        let resumir = anadir(&mut en_tel, "Resumir", "📝", "Resume lo último en 5 puntos", ahora).unwrap();
        guardar(&tel.raiz(), &en_tel).unwrap();
        let mut en_pc = Vec::new();
        let foto = anadir(&mut en_pc, "Foto", "🖼️", "¿Qué pone en esta foto?", ahora + 1).unwrap();
        guardar(&pc.raiz(), &en_pc).unwrap();

        // Una vuelta entera desde el PC, como la de la pantalla.
        let vuelta = |desde: &DiscoAndroid, hacia: &DiscoAndroid| {
            conectado(desde, hacia, false, None, &reloj, |s| {
                let mut hecho = Hecho::default();
                crate::vuelta::una(s, None, &mut hecho, "", &|| reloj.ahora(), &mut |_| {}).unwrap();
            })
            .unwrap();
        };
        vuelta(&pc, &tel);
        let ids = |d: &DiscoAndroid| -> Vec<String> { visibles(&leer(&d.raiz()).unwrap()).into_iter().map(|r| r.id).collect() };
        let mut esperado = vec![resumir.id.clone(), foto.id.clone()];
        esperado.sort();
        let mut en_los_dos = ids(&tel);
        en_los_dos.sort();
        assert_eq!(en_los_dos, esperado);
        assert_eq!(leer(&tel.raiz()).unwrap(), leer(&pc.raiz()).unwrap(), "los dos iguales");

        // Se borra en el telefono y se renombra en el PC otra: al juntar,
        // cada cosa en su sitio y la borrada no vuelve.
        let mut t = leer(&tel.raiz()).unwrap();
        borrar(&mut t, &resumir.id, reloj.ahora() + 10).unwrap();
        guardar(&tel.raiz(), &t).unwrap();
        let mut p = leer(&pc.raiz()).unwrap();
        cambiar(&mut p, &foto.id, reloj.ahora() + 11, |r| r.nombre = "Leer foto".into()).unwrap();
        guardar(&pc.raiz(), &p).unwrap();
        vuelta(&tel, &pc);
        for d in [&tel, &pc] {
            let v = visibles(&leer(&d.raiz()).unwrap());
            assert_eq!(v.len(), 1);
            assert_eq!(v[0].nombre, "Leer foto");
        }
        // Otra vuelta no cambia nada.
        let antes = std::fs::read_to_string(ruta(&pc.raiz())).unwrap();
        vuelta(&pc, &tel);
        assert_eq!(std::fs::read_to_string(ruta(&pc.raiz())).unwrap(), antes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Negativo: si el otro manda recetas que no se entienden, contesta con
    /// un error y no toca las suyas; y un aparato sin fichero no se rompe.
    #[test]
    fn unas_recetas_rotas_por_el_cable_no_pisan_nada() {
        use crate::disco::Disco;
        use crate::disco_android::DiscoAndroid;
        use crate::disco_android::prueba::{Reloj, conectado, crear_grupo, presentar};
        use crate::mensajes::Peticion;

        let dir = std::env::temp_dir().join(format!("pixpin-recetas-rotas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let tel = DiscoAndroid::nuevo(dir.join("telefono/files"), "tel");
        let pc = DiscoAndroid::nuevo(dir.join("pc/files"), "pc");
        presentar(&tel, "id-tel", "Teléfono");
        presentar(&pc, "id-pc", "PC");
        let reloj = Reloj::nuevo();
        crear_grupo(&tel, "ABCDE23456", reloj.ahora());
        conectado(&pc, &tel, true, Some("ABCDE23456"), &reloj, |_| {}).unwrap();
        let mut en_tel = Vec::new();
        anadir(&mut en_tel, "", "", "Hazme una gráfica", reloj.ahora()).unwrap();
        guardar(&tel.raiz(), &en_tel).unwrap();
        let antes = std::fs::read_to_string(ruta(&tel.raiz())).unwrap();
        let r = conectado(&pc, &tel, false, None, &reloj, |s| {
            s.pedir_de_prueba(&Peticion {
                parche: Some("{roto".into()),
                ..Peticion::de(PETICION)
            })
        })
        .unwrap()
        .unwrap();
        assert!(r.error.is_some(), "se queja");
        assert_eq!(std::fs::read_to_string(ruta(&tel.raiz())).unwrap(), antes, "no se toco");
        // El PC no tenia fichero: con una vuelta de recetas sale con la del tel.
        let hecho = conectado(&pc, &tel, false, None, &reloj, |s| s.recetas(reloj.ahora()).unwrap()).unwrap();
        assert!(hecho);
        assert_eq!(visibles(&leer(&pc.raiz()).unwrap()).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lo_que_llega_raro_se_limpia_o_se_descarta() {
        assert!(de_texto("no es json").is_none());
        assert!(de_texto("[]").is_none(), "una lista suelta no es el fichero");
        let largo = "x".repeat(5000);
        let t = format!(
            r#"{{"version":1,"recetas":[{{"id":"","nombre":"sin id","pedido":"p"}},{{"id":"z","nombre":"{largo}","pedido":"{largo}","futuro":true}}]}}"#
        );
        let v = de_texto(&t).unwrap();
        assert_eq!(v.len(), 1, "la de sin id se descarta");
        assert_eq!(v[0].nombre.chars().count(), TOPE_NOMBRE);
        assert_eq!(v[0].pedido.chars().count(), TOPE_PEDIDO);
        // Una version mas nueva se lee igual (campos de mas se ignoran).
        assert!(de_texto(r#"{"version":7,"recetas":[]}"#).unwrap().is_empty());
    }
}
