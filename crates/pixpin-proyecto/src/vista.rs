//! El almacen del PC visto como la carpeta `files` de PixPin Android.
//!
//! Es el `Disco` de sincronizar para este equipo. El movil guarda todos los
//! chats en un `guardados.jsonl`, los proyectos en `proyectos/proyectos.json`
//! y los lienzos en `pins/draw/`; el PC guarda un chat por proyecto, cada uno
//! en su carpeta. Aqui se traduce entre las dos cosas **sin cambiar como
//! guarda el PC**: el chat del PC sigue leyendo lo mismo de los mismos
//! ficheros, y el movil ve lo que esperaria ver de otro movil. El diseno,
//! con cada regla y su por que, esta en
//! `docs/superpowers/specs/2026-09-19-sincronizar-chats-design.md`.
//!
//! Las reglas, en corto:
//!
//! - **Chat ↔ ficha.** «Mensajes guardados» es la conversacion general del
//!   movil (`general`). Cada otra ficha es el chat de su proyecto, con el id
//!   de `resto["chat"]`, o el de su `proyecto.json` (lo que trajo un
//!   `.pixpin` del movil), o el suyo.
//! - **Rutas.** `pins/draw/<d>.excalidraw.gz` es `lienzos/<d>.excalidraw` de
//!   su carpeta; lo nacido aqui sale como `guardados/pc/<chat>/<ruta>`; lo
//!   demas del movil se guarda en `android/<ruta>` de la carpeta del chat.
//! - **Texto.** Lo que llega se guarda tal cual, con `pixpin:files/` dentro:
//!   asi sale igual que entro y su resumen no cambia. Al salir solo se
//!   reescribe lo nacido aqui con ruta relativa.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

use pixpin_sincro::canonico::Json;
use pixpin_sincro::disco::{
    self, Cambio, Disco, GENERAL, Identidad, NOMBRE_GENERAL, PORTATIL, escribir_atomico, limpio,
};
use pixpin_sincro::kotlin;
use pixpin_sincro::mensajes::{Aparato, Chat};

use crate::almacen::{self, Ficha, Indice};

/// La marca de una ficha que ya se sincroniza: `completar_hojas` no le
/// inventa mensajes, que el movil no tiene y tomaria por nuevos.
pub const MARCA_SINCRO: &str = "sincro";
/// Donde se guarda el id de chat de una ficha cuando no es el suyo.
pub const MARCA_CHAT: &str = "chat";

/// El `Disco` de este equipo.
pub struct DiscoPc {
    raiz: PathBuf,
    avisos: Option<Box<dyn Fn(Cambio) + Send + Sync>>,
    /// El mapa de chats, por la firma del indice (tamano y fecha): cada ruta
    /// de archivo lo necesita, y rehacerlo es leer el indice y el
    /// `proyecto.json` de cada ficha.
    mapa: std::sync::Mutex<Option<(Firma, Mapa)>>,
}

type Firma = (u64, Option<std::time::SystemTime>);
/// Cada chat con su ficha.
type Mapa = Vec<(String, Ficha)>;

impl DiscoPc {
    pub fn nuevo(raiz: &Path) -> DiscoPc {
        DiscoPc {
            raiz: raiz.to_path_buf(),
            avisos: None,
            mapa: std::sync::Mutex::new(None),
        }
    }

    /// Con quien avisar de lo que se escribe (para recargar la pantalla).
    pub fn con_avisos(mut self, f: impl Fn(Cambio) + Send + Sync + 'static) -> DiscoPc {
        self.avisos = Some(Box::new(f));
        self
    }

    /// Cada chat con su ficha, en el orden del indice. El primero que pide un
    /// id de chat se lo queda; uno repetido (el mismo `.pixpin` importado
    /// dos veces) se queda con el id de su ficha.
    pub fn mapa(&self) -> Vec<(String, Ficha)> {
        let firma = std::fs::metadata(almacen::ruta(&self.raiz))
            .map(|m| (m.len(), m.modified().ok()))
            .unwrap_or((0, None));
        if let Ok(g) = self.mapa.lock()
            && let Some((f, m)) = g.as_ref()
            && *f == firma
        {
            return m.clone();
        }
        let m = mapa_de(&self.raiz, &Indice::leer(&self.raiz));
        if let Ok(mut g) = self.mapa.lock() {
            *g = Some((firma, m.clone()));
        }
        m
    }

    /// Despues de escribir el indice o un `proyecto.json`: la firma podria no
    /// cambiar si dos escrituras caen en el mismo instante con el mismo largo.
    fn olvidar_mapa(&self) {
        if let Ok(mut g) = self.mapa.lock() {
            *g = None;
        }
    }

    pub fn ficha_de(&self, chat: &str) -> Option<Ficha> {
        self.mapa()
            .into_iter()
            .find(|(c, _)| c == chat)
            .map(|(_, f)| f)
    }

    fn carpeta_de(&self, chat: &str) -> PathBuf {
        match self.ficha_de(chat) {
            Some(f) => almacen::carpeta(&self.raiz, &f.id),
            // Todavia sin ficha: donde la creara `asegurar_ficha` si puede.
            None => almacen::carpeta(&self.raiz, &limpio(chat)),
        }
    }

    /// La ficha de un chat, creandola si no esta. «Mensajes guardados» se
    /// crea como siempre; otra, con el id del chat si es un nombre de
    /// carpeta valido y libre, para que su carpeta se reconozca a simple
    /// vista.
    fn asegurar_ficha(&self, chat: &str, nombre: &str) -> io::Result<Ficha> {
        if let Some(f) = self.ficha_de(chat) {
            return Ok(f);
        }
        let aparato = self.identidad()?.yo;
        let aparato = crate::codigos::de_aparato(&aparato.id);
        if chat == GENERAL {
            let f = almacen::asegurar_guardados(&self.raiz, ahora(), &aparato);
            self.olvidar_mapa();
            return f;
        }
        let mut indice = Indice::leer(&self.raiz);
        let valido = !chat.is_empty()
            && chat.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && indice.buscar(chat).is_none()
            && !almacen::carpeta(&self.raiz, chat).exists();
        let mut ficha = Ficha {
            id: if valido {
                chat.to_string()
            } else {
                crate::codigos::nuevo()
            },
            nombre: nombre.to_string(),
            ..Default::default()
        };
        if !valido {
            ficha
                .resto
                .insert(MARCA_CHAT.into(), serde_json::Value::String(chat.into()));
        }
        ficha
            .resto
            .insert(MARCA_SINCRO.into(), serde_json::Value::Bool(true));
        indice.proyectos.push(ficha.clone());
        indice.guardar(&self.raiz)?;
        self.olvidar_mapa();
        Ok(ficha)
    }

    /// Las lineas de un chat: las que se entienden como objeto JSON (con el
    /// `proyecto` del chat puesto, que es por lo que se reconocen) y las que
    /// no, tal cual, para no perderlas al reescribir.
    fn lineas(&self, chat: &str) -> (Vec<Json>, Vec<String>) {
        let texto = std::fs::read_to_string(self.carpeta_de(chat).join("guardados.jsonl"))
            .unwrap_or_default();
        let mut buenas = Vec::new();
        let mut crudas = Vec::new();
        for l in texto.lines().filter(|l| !l.trim().is_empty()) {
            match Json::analizar(l) {
                Ok(mut j @ Json::Objeto(_)) => {
                    kotlin::poner(&mut j, "proyecto", proyecto_de(chat));
                    buenas.push(j)
                }
                _ => crudas.push(l.to_string()),
            }
        }
        (buenas, crudas)
    }

    fn escribir_lineas(&self, chat: &str, buenas: &[Json], crudas: &[String]) -> io::Result<()> {
        let mut texto = String::new();
        for j in buenas {
            texto.push_str(&j.a_texto());
            texto.push('\n');
        }
        for l in crudas {
            texto.push_str(l);
            texto.push('\n');
        }
        escribir_atomico(
            &self.carpeta_de(chat).join("guardados.jsonl"),
            texto.as_bytes(),
        )
    }

    /// Un mensaje guardado aqui, a como viaja.
    fn portatil_de(&self, chat: &str, m: &Json) -> Option<Json> {
        let mut o = m.clone();
        kotlin::poner(&mut o, "proyecto", proyecto_de(chat));
        // El `Mensaje` del PC puede no llevar clase: una nota, para el movil.
        if kotlin::cadena(&o, "clase").is_none() {
            kotlin::poner(&mut o, "clase", Json::cadena("NOTA"));
        }
        if let Some(r) = kotlin::cadena(&o, "ruta").filter(|r| es_relativa(r)) {
            let v = format!("{PORTATIL}{}", virtual_de(chat, r));
            kotlin::poner(&mut o, "ruta", Json::cadena(v));
        }
        kotlin::normalizar_mensaje(&o)
    }

    /// El proyecto de una ficha tal como lo tiene aqui, antes de hacerlo
    /// portatil.
    fn proyecto_local(&self, chat: &str, f: &Ficha) -> Json {
        let dir = almacen::carpeta(&self.raiz, &f.id);
        let mut p = std::fs::read_to_string(dir.join("proyecto.json"))
            .ok()
            .and_then(|t| Json::analizar(&t).ok())
            .filter(|j| j.como_objeto().is_some())
            .unwrap_or_else(|| {
                // Nacido aqui: se le hace uno con lo que dice la ficha. Con la
                // fecha de creacion de tocado, que no se mueve con cada
                // mensaje: si no, cada nota escrita aqui haria viajar el
                // proyecto entero.
                Json::de_valor(&serde_json::json!({"tocado": f.creado.max(0)}))
            });
        kotlin::poner(&mut p, "id", Json::cadena(chat));
        kotlin::poner(&mut p, "nombre", Json::cadena(f.nombre.clone()));
        if let Some(u) = &f.uid {
            kotlin::poner(&mut p, "uid", Json::cadena(u.clone()));
        }
        if f.creado > 0 {
            kotlin::poner(&mut p, "creado", Json::numero(f.creado));
        }
        if let Some(a) = &f.aparato {
            kotlin::poner(&mut p, "aparato", Json::cadena(a.clone()));
        }
        p
    }
}

/// El `proyecto` de un mensaje de ese chat: nulo en la general.
fn proyecto_de(chat: &str) -> Json {
    if chat == GENERAL {
        Json::Nulo
    } else {
        Json::cadena(chat)
    }
}

/// Una ruta relativa de este equipo (`archivos/x.png`), no una del movil ni
/// una absoluta.
fn es_relativa(r: &str) -> bool {
    !r.is_empty()
        && !r.starts_with(PORTATIL)
        && !r.starts_with('/')
        && !r.contains(':')
        && !r.contains('\\')
}

/// La ruta con que viaja un fichero nacido aqui, relativo a su carpeta.
pub fn virtual_de(chat: &str, local: &str) -> String {
    if let Some(d) = local
        .strip_prefix("lienzos/")
        .and_then(|x| x.strip_suffix(".excalidraw"))
        .filter(|d| !d.contains('/'))
    {
        return format!("pins/draw/{d}.excalidraw.gz");
    }
    if let Some(x) = local.strip_prefix("android/") {
        return x.to_string();
    }
    format!("guardados/pc/{}/{local}", limpio(chat))
}

fn mapa_de(raiz: &Path, indice: &Indice) -> Vec<(String, Ficha)> {
    let mut salida: Vec<(String, Ficha)> = Vec::new();
    let mut usados = HashSet::new();
    for f in &indice.proyectos {
        let pedido = if f.es_guardados() {
            GENERAL.to_string()
        } else if let Some(c) = f.resto.get(MARCA_CHAT).and_then(|v| v.as_str()) {
            c.to_string()
        } else {
            std::fs::read_to_string(almacen::carpeta(raiz, &f.id).join("proyecto.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .and_then(|v| v.get("id")?.as_str().map(str::to_string))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| f.id.clone())
        };
        let chat = if usados.contains(&pedido) {
            f.id.clone()
        } else {
            pedido
        };
        if usados.insert(chat.clone()) {
            salida.push((chat, f.clone()));
        }
    }
    salida
}

/// El id de chat de una ficha, para quien borra o marca desde fuera.
pub fn chat_de_ficha(raiz: &Path, ficha_id: &str) -> Option<String> {
    mapa_de(raiz, &Indice::leer(raiz))
        .into_iter()
        .find(|(_, f)| f.id == ficha_id)
        .map(|(c, _)| c)
}

/// Donde esta en este equipo lo que un mensaje o un lienzo de `ficha_id`
/// senala: una ruta relativa a su carpeta, o una `pixpin:files/…` que llego
/// del movil. None si es de otro aparato (absoluta).
pub fn ruta_real(raiz: &Path, ficha_id: &str, ruta: &str) -> Option<PathBuf> {
    if ruta.is_empty() {
        return None;
    }
    if let Some(rel) = ruta.strip_prefix(PORTATIL) {
        if !disco::permitida(rel) {
            return None;
        }
        let chat = chat_de_ficha(raiz, ficha_id).unwrap_or_else(|| ficha_id.to_string());
        return Some(DiscoPc::nuevo(raiz).ruta(&chat, rel));
    }
    if Path::new(ruta).is_absolute() || ruta.starts_with('/') || ruta.contains(':') {
        return None;
    }
    Some(almacen::carpeta(raiz, ficha_id).join(ruta))
}

/// Deja marca de unos mensajes que se quitaron del chat de `ficha_id`: sin
/// ella, la vuelta siguiente los traeria otra vez del otro aparato.
pub fn anotar_borrados(
    raiz: &Path,
    ficha_id: &str,
    quitados: &[crate::cuaderno::Mensaje],
    cuando: i64,
) -> io::Result<()> {
    let Some(chat) = chat_de_ficha(raiz, ficha_id) else {
        return Ok(());
    };
    let d = DiscoPc::nuevo(raiz);
    let portatiles: Vec<Json> = quitados
        .iter()
        .filter_map(|m| serde_json::to_value(m).ok())
        .filter_map(|v| d.portatil_de(&chat, &Json::de_valor(&v)))
        .collect();
    disco::anotar_marcas_en(&d.sincro(), &disco::marcas_de(&chat, &portatiles, cuando))
}

fn ahora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn nombre_del_equipo() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PixPin Max".into())
}

fn al_cable(a: &crate::identidad::Aparato) -> Aparato {
    Aparato {
        id: a.id.clone(),
        nombre: a.nombre.clone(),
        letra: a.letra.clone(),
        desde: a.desde,
    }
}

impl Disco for DiscoPc {
    fn raiz(&self) -> PathBuf {
        self.raiz.clone()
    }

    fn identidad(&self) -> io::Result<Identidad> {
        let i = crate::identidad::Identidad::leer_o_crear(&self.raiz, &nombre_del_equipo())?;
        Ok(Identidad {
            yo: al_cable(&i.yo),
            codigo: i.codigo.clone(),
            miembros: i.miembros.iter().map(al_cable).collect(),
        })
    }

    /// Lo que no conoce el cable (el `resto` de la identidad y de este
    /// aparato) se conserva.
    fn guardar_identidad(&self, i: &Identidad) -> io::Result<()> {
        let actual = crate::identidad::Identidad::leer_o_crear(&self.raiz, &nombre_del_equipo())?;
        let del_cable = |a: &Aparato, resto: serde_json::Map<String, serde_json::Value>| {
            crate::identidad::Aparato {
                id: a.id.clone(),
                nombre: a.nombre.clone(),
                letra: a.letra.clone(),
                desde: a.desde,
                resto,
            }
        };
        crate::identidad::Identidad {
            yo: del_cable(&i.yo, actual.yo.resto.clone()),
            codigo: i.codigo.clone(),
            miembros: i
                .miembros
                .iter()
                .map(|m| {
                    let resto = actual
                        .miembros
                        .iter()
                        .find(|x| x.id == m.id)
                        .map(|x| x.resto.clone())
                        .unwrap_or_default();
                    del_cable(m, resto)
                })
                .collect(),
            resto: actual.resto,
        }
        .guardar(&self.raiz)
    }

    fn avisar(&self, c: Cambio) {
        if let Some(f) = &self.avisos {
            f(c);
        }
    }

    fn chats(&self) -> io::Result<Vec<Chat>> {
        let mapa = self.mapa();
        let resumen = |chat: &str| {
            let (l, _) = self.lineas(chat);
            let ultimo = l
                .iter()
                .filter_map(|m| kotlin::numero(m, "cuando"))
                .max()
                .unwrap_or(0)
                .max(0);
            (l.len() as u32, ultimo)
        };
        let (n, ultimo) = if mapa.iter().any(|(c, _)| c == GENERAL) {
            resumen(GENERAL)
        } else {
            (0, 0)
        };
        let mut v = vec![Chat {
            id: GENERAL.into(),
            nombre: NOMBRE_GENERAL.into(),
            mensajes: n,
            tocado: ultimo,
        }];
        for (chat, f) in mapa.iter().filter(|(c, _)| c != GENERAL) {
            let (n, ultimo) = resumen(chat);
            v.push(Chat {
                id: chat.clone(),
                nombre: f.nombre.clone(),
                mensajes: n,
                tocado: f.tocado.max(ultimo),
            });
        }
        Ok(v)
    }

    fn mensajes(&self, chat: &str) -> io::Result<Vec<Json>> {
        if self.ficha_de(chat).is_none() {
            return Ok(Vec::new());
        }
        let (l, _) = self.lineas(chat);
        Ok(l.iter().filter_map(|m| self.portatil_de(chat, m)).collect())
    }

    fn aplicar_mensajes(
        &self,
        chat: &str,
        poner: &[String],
        borrar: &[String],
        ahora: i64,
    ) -> io::Result<()> {
        if poner.is_empty() && borrar.is_empty() {
            return Ok(());
        }
        let quitar: HashSet<String> = borrar.iter().cloned().collect();
        // Lo que llega se guarda tal cual viene: con `pixpin:files/` dentro,
        // que es lo que hace que salga igual y su resumen no cambie.
        let llegan: Vec<Json> = poner
            .iter()
            .filter_map(|t| kotlin::mensaje_de_texto(t))
            .filter(|m| kotlin::chat_de(m) == chat)
            .collect();
        let ficha = self.asegurar_ficha(chat, chat)?;
        let (buenas, crudas) = self.lineas(chat);
        let (salida, quitados) = disco::aplicar_en_lista(buenas, chat, &llegan, &quitar);
        self.escribir_lineas(chat, &salida, &crudas)?;
        self.marcas_tras_aplicar(chat, &llegan, borrar, &quitados, ahora)?;
        // La ficha, al dia: la lista del chat ensena lo ultimo.
        let mut indice = Indice::leer(&self.raiz);
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == ficha.id) {
            let ultimo = salida
                .iter()
                .filter_map(|m| {
                    let c = kotlin::numero(m, "cuando")?;
                    let v = serde_json::from_str::<crate::cuaderno::Mensaje>(&m.a_texto()).ok()?;
                    Some((c, v.resumen()))
                })
                .max_by_key(|(c, _)| *c);
            if let Some((c, r)) = ultimo {
                f.tocado = f.tocado.max(c);
                f.resumen = r;
            }
            f.resto
                .insert(MARCA_SINCRO.into(), serde_json::Value::Bool(true));
            indice.guardar(&self.raiz)?;
            self.olvidar_mapa();
        }
        self.avisar(Cambio::Mensajes);
        Ok(())
    }

    fn sellar(&self) -> io::Result<()> {
        let aparato = crate::codigos::de_aparato(&self.identidad()?.yo.id);
        for (chat, _) in self.mapa() {
            let (mut buenas, crudas) = self.lineas(&chat);
            if !disco::sellar_chat(&mut buenas, &aparato).is_empty() {
                self.escribir_lineas(&chat, &buenas, &crudas)?;
            }
        }
        Ok(())
    }

    fn borrar_chat(&self, chat: &str, motivo: &str, cuando: i64, aparato: &str) -> io::Result<()> {
        // La conversacion general no es un proyecto: no se borra entera.
        if chat == GENERAL {
            return Ok(());
        }
        let Some(f) = self.ficha_de(chat) else {
            return self.anotar_lapida(chat, cuando, aparato);
        };
        self.hacer_copia(chat, motivo, cuando);
        // Marca de cada mensaje, como `aplicarMensajes`: sin ellas volverian
        // solos en la vuelta siguiente. El cuaderno no se vacia: va entero a
        // la papelera con su carpeta, por si hay que volver.
        let todos: Vec<(String, Json)> = self.mensajes_por_clave(chat)?;
        let claves: Vec<String> = todos.iter().map(|(k, _)| k.clone()).collect();
        let mensajes: Vec<Json> = todos.into_iter().map(|(_, m)| m).collect();
        self.marcas_tras_aplicar(chat, &[], &claves, &mensajes, cuando)?;
        almacen::borrar_proyectos(&self.raiz, std::slice::from_ref(&f.id), cuando)?;
        self.olvidar_mapa();
        // `borrar_proyectos` ya dejo lapida; esta dice ademas quien lo borro.
        self.anotar_lapida(chat, cuando, aparato)?;
        self.avisar(Cambio::Proyectos);
        Ok(())
    }

    fn proyecto_portatil(&self, chat: &str) -> io::Result<Option<Json>> {
        if chat == GENERAL {
            return Ok(None);
        }
        let Some(f) = self.ficha_de(chat) else {
            return Ok(None);
        };
        let mut p = self.proyecto_local(chat, &f);
        disco::sellar_proyecto(&mut p);
        Ok(kotlin::normalizar_proyecto(&p))
    }

    fn guardar_proyecto(&self, p: &Json) -> io::Result<()> {
        let Some(nuevo) = kotlin::normalizar_proyecto(p) else {
            return Ok(());
        };
        let chat = kotlin::cadena(&nuevo, "id").unwrap_or_default().to_string();
        if chat.is_empty() || chat == GENERAL {
            return Ok(());
        }
        let nombre = kotlin::cadena(&nuevo, "nombre")
            .unwrap_or_default()
            .to_string();
        let ficha = self.asegurar_ficha(&chat, &nombre)?;
        let antes = self.proyecto_portatil(&chat)?;
        let puesto = disco::actualizado(antes.as_ref(), &nuevo);
        let dir = almacen::carpeta(&self.raiz, &ficha.id);
        escribir_atomico(&dir.join("proyecto.json"), puesto.a_texto().as_bytes())?;
        self.olvidar_mapa();
        let mut indice = Indice::leer(&self.raiz);
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == ficha.id) {
            f.nombre = nombre;
            f.uid = kotlin::cadena(&puesto, "uid").map(str::to_string);
            f.creado = kotlin::numero(&puesto, "creado").unwrap_or(0);
            f.aparato = kotlin::cadena(&puesto, "aparato").map(str::to_string);
            f.tocado = f.tocado.max(kotlin::numero(&puesto, "tocado").unwrap_or(0));
            f.hojas = puesto
                .como_objeto()
                .and_then(|o| o.obtener("hojas"))
                .and_then(Json::como_lista)
                .map_or(0, |l| l.len() as u32);
            f.resto
                .insert(MARCA_SINCRO.into(), serde_json::Value::Bool(true));
            indice.guardar(&self.raiz)?;
            self.olvidar_mapa();
        }
        self.avisar(Cambio::Proyectos);
        Ok(())
    }

    fn ruta(&self, chat: &str, rel: &str) -> PathBuf {
        let p = self.carpeta_de(chat);
        if let Some(d) = rel
            .strip_prefix("pins/draw/")
            .and_then(|x| x.strip_suffix(".excalidraw.gz"))
            .filter(|d| !d.contains('/'))
        {
            return p.join("lienzos").join(format!("{d}.excalidraw"));
        }
        if let Some((c, resto)) = rel
            .strip_prefix("guardados/pc/")
            .and_then(|x| x.split_once('/'))
            && let Some((_, f)) = self.mapa().into_iter().find(|(ch, _)| limpio(ch) == c)
        {
            return almacen::carpeta(&self.raiz, &f.id).join(resto);
        }
        p.join("android").join(rel)
    }

    /// Lo que llega se guarda tal cual: `pixpin:files/` se queda dentro y
    /// quien lo lee aqui lo resuelve con [ruta_real].
    fn a_local(&self, _chat: &str, texto: String) -> String {
        texto
    }

    /// Un lienzo nacido aqui lleva sus fotos como `imagenes/<id>`, relativas
    /// a su carpeta: al salir se ponen como las veria el movil. Lo que ya
    /// venia del movil no se toca, ni se reescribe el texto si no hace falta.
    fn a_portatil(&self, chat: &str, rel: &str, texto: String) -> String {
        if !rel.ends_with(".excalidraw.gz") || !texto.contains("\"imagenes/") {
            return texto;
        }
        let Ok(mut j) = Json::analizar(&texto) else {
            return texto;
        };
        let Some(Json::Objeto(files)) = j.como_objeto().and_then(|o| o.obtener("files")).cloned()
        else {
            return texto;
        };
        let mut nuevos = files.clone();
        let mut cambio = false;
        for (k, v) in files.iter() {
            if let Some(ruta) = kotlin::cadena(v, "path").filter(|r| r.starts_with("imagenes/")) {
                let mut v = v.clone();
                let portatil = format!("{PORTATIL}{}", virtual_de(chat, ruta));
                kotlin::poner(&mut v, "path", Json::cadena(portatil));
                nuevos.poner(k, v);
                cambio = true;
            }
        }
        if !cambio {
            return texto;
        }
        kotlin::poner(&mut j, "files", Json::Objeto(nuevos));
        j.a_texto()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_rutas_nacidas_aqui_llevan_su_chat_y_los_lienzos_su_sitio_del_movil() {
        assert_eq!(
            virtual_de("pr-1", "lienzos/d9.excalidraw"),
            "pins/draw/d9.excalidraw.gz"
        );
        assert_eq!(
            virtual_de("pr-1", "archivos/captura.png"),
            "guardados/pc/pr-1/archivos/captura.png"
        );
        assert_eq!(
            virtual_de("general", "android/guardados/x.pdf"),
            "guardados/x.pdf"
        );
        // Y lo que no es relativo de aqui no se toca.
        assert!(!es_relativa("pixpin:files/guardados/x.pdf"));
        assert!(!es_relativa("/storage/emulated/0/x.pdf"));
        assert!(!es_relativa(r"C:\Users\x.pdf"));
        assert!(es_relativa("archivos/x.pdf"));
    }
}
