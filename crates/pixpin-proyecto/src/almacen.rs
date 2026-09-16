//! Los proyectos que hay en este equipo: la lista de la ventana de chat.
//!
//! Un indice pequeno (`proyectos/indice.json`) con lo justo para pintar la
//! lista sin abrir nada: nombre, ultima linea, cuando se toco y los tres
//! codigos. El contenido de cada proyecto sigue en su `.pixpin`.
//!
//! Aqui vive tambien la regla de recibir de PixPin Android: **solo se pone
//! al dia un proyecto si coinciden sus tres codigos**; si no, entra como uno
//! nuevo con codigo unico nuevo. Ante la duda, duplicar y no pisar.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::codigos;

/// La ficha de un proyecto en la lista.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ficha {
    pub id: String,
    pub nombre: String,
    /// Los tres codigos (ver `codigos`).
    pub uid: Option<String>,
    pub creado: i64,
    pub aparato: Option<String>,
    /// Ultima vez que se toco, en milisegundos desde 1970. Ordena la lista.
    pub tocado: i64,
    /// Cuantas hojas tiene: lo que se ensena bajo el nombre mientras no
    /// haya mensajes. El texto se compone arriba, que es quien traduce.
    pub hojas: u32,
    /// La ultima linea, si ya hay uno.
    pub resumen: String,
    pub sin_leer: u32,
    /// El fichero del que salio, si vino de uno.
    pub paquete: Option<String>,
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

impl Ficha {
    /// La ficha de un proyecto recien abierto, con sus tres codigos tal como
    /// vienen: son los que deciden si esto ya estaba aqui.
    pub fn de_proyecto(p: &crate::Proyecto, paquete: Option<&std::path::Path>) -> Ficha {
        Ficha {
            id: p.id.clone(),
            nombre: p.nombre.clone(),
            uid: p.uid.clone(),
            creado: p.creado,
            aparato: p.aparato.clone(),
            tocado: p.tocado,
            hojas: p.hojas.len() as u32,
            resumen: String::new(),
            sin_leer: 0,
            paquete: paquete.map(|r| r.display().to_string()),
            resto: Default::default(),
        }
    }

    pub fn codigo_unico(&self) -> String {
        codigos::unico(self.uid.as_deref(), "p:", &self.id)
    }

    /// Los tres codigos iguales: es la misma cosa, se puede poner al dia.
    pub fn misma_que(&self, otra: &Ficha) -> bool {
        self.codigo_unico() == otra.codigo_unico()
            && self.creado == otra.creado
            && self.aparato == otra.aparato
    }
}

/// Que paso al recibir un proyecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recibido {
    /// Ya estaba: se puso al dia el que habia.
    Actualizado,
    /// No estaba (o no es el mismo): entro como proyecto nuevo.
    Creado,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Indice {
    pub proyectos: Vec<Ficha>,
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

pub fn ruta(raiz: &Path) -> PathBuf {
    raiz.join("proyectos").join("indice.json")
}

/// La carpeta de un proyecto, donde vive su cuaderno (`guardados.jsonl`) y
/// lo que traiga consigo. Por `id` y no por codigo unico porque el codigo
/// puede cambiar al recibir, y una carpeta que se renombra sola pierde lo
/// que hubiera dentro.
pub fn carpeta(raiz: &Path, id: &str) -> PathBuf {
    raiz.join("proyectos").join(id)
}

impl Indice {
    /// Lee el indice. Un fichero ilegible da la lista vacia y no impide
    /// abrir la ventana: es una lista, no los datos.
    pub fn leer(raiz: &Path) -> Indice {
        std::fs::read_to_string(ruta(raiz))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Temporal y renombrado, como el resto del almacen: un corte a mitad no
    /// deja la lista a medias.
    pub fn guardar(&self, raiz: &Path) -> std::io::Result<()> {
        let fichero = ruta(raiz);
        let carpeta = fichero.parent().unwrap_or(raiz);
        std::fs::create_dir_all(carpeta)?;
        let tmp = carpeta.join("indice.json.tmp");
        let texto = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, texto)?;
        std::fs::rename(&tmp, &fichero)
    }

    /// Las fichas en el orden de la lista: lo ultimo tocado arriba, y a
    /// igualdad de fecha, por nombre, para que no bailen entre arranques.
    pub fn ordenadas(&self) -> Vec<&Ficha> {
        let mut v: Vec<&Ficha> = self.proyectos.iter().collect();
        v.sort_by(|a, b| {
            b.tocado
                .cmp(&a.tocado)
                .then_with(|| a.nombre.cmp(&b.nombre))
        });
        v
    }

    pub fn buscar(&self, id: &str) -> Option<&Ficha> {
        self.proyectos.iter().find(|f| f.id == id)
    }

    /// Mete lo que llega: pone al dia el que ya esta si coinciden los tres
    /// codigos, y si no lo anade como nuevo, con codigo unico nuevo para que
    /// los dos puedan convivir y volver a viajar sin pisarse.
    pub fn recibir(&mut self, llega: Ficha) -> (Recibido, String) {
        if let Some(sitio) = self.proyectos.iter().position(|f| f.misma_que(&llega)) {
            let id = self.proyectos[sitio].id.clone();
            let sin_leer = self.proyectos[sitio].sin_leer;
            // El id de aqui manda: lo de fuera puede venir con otro y hay
            // ficheros que ya lo usan.
            self.proyectos[sitio] = Ficha {
                id: id.clone(),
                sin_leer,
                ..llega
            };
            return (Recibido::Actualizado, id);
        }
        let mut nueva = llega;
        if self.proyectos.iter().any(|f| f.id == nueva.id) {
            // Mismo id pero otra cosa: se le da uno propio para no mezclar.
            nueva.id = format!("{}-{}", nueva.id, codigos::nuevo());
        }
        // El codigo que trae se respeta: es lo que hace que la proxima vez se
        // reconozca y se ponga al dia en vez de duplicarse. Solo se estrena
        // uno si aqui ya hay otro proyecto con ese mismo codigo, que si no
        // quedarian dos cosas distintas llamandose igual.
        let suyo = nueva.codigo_unico();
        if nueva.uid.is_none() || self.proyectos.iter().any(|f| f.codigo_unico() == suyo) {
            nueva.uid = Some(codigos::nuevo());
        }
        let id = nueva.id.clone();
        self.proyectos.push(nueva);
        (Recibido::Creado, id)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ficha(id: &str, nombre: &str, tocado: i64) -> Ficha {
        Ficha {
            id: id.into(),
            nombre: nombre.into(),
            uid: Some("VVT587BFCA".into()),
            creado: 1_757_939_357_123,
            aparato: Some("K7Q2".into()),
            tocado,
            ..Default::default()
        }
    }

    fn carpeta(etiqueta: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-proyectos-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn la_lista_ensena_arriba_lo_ultimo_tocado() {
        let mut i = Indice::default();
        i.proyectos.push(ficha("a", "Casa", 10));
        i.proyectos.push(ficha("b", "Obra", 30));
        i.proyectos.push(ficha("c", "Taller", 20));
        let nombres: Vec<&str> = i.ordenadas().iter().map(|f| f.nombre.as_str()).collect();
        assert_eq!(nombres, ["Obra", "Taller", "Casa"]);
    }

    #[test]
    fn recibir_lo_mismo_lo_pone_al_dia_en_vez_de_duplicarlo() {
        let mut i = Indice::default();
        i.recibir(ficha("a", "Casa", 10));
        let mut otra_vez = ficha("id-de-otro-aparato", "Casa (con la cocina)", 50);
        otra_vez.resumen = "3 hojas".into();
        let (que, id) = i.recibir(otra_vez);
        assert_eq!(que, Recibido::Actualizado);
        assert_eq!(id, "a", "conserva el id de aqui");
        assert_eq!(i.proyectos.len(), 1);
        assert_eq!(i.buscar("a").unwrap().nombre, "Casa (con la cocina)");
        assert_eq!(i.buscar("a").unwrap().resumen, "3 hojas");
    }

    #[test]
    fn si_no_coinciden_los_tres_codigos_entra_como_nuevo_y_no_pisa() {
        let mut i = Indice::default();
        i.recibir(ficha("a", "Casa", 10));
        // Mismo codigo unico y misma fecha, pero otro aparato: no es lo mismo.
        let mut otra = ficha("a", "Casa de otro", 20);
        otra.aparato = Some("9FMQ".into());
        let (que, id) = i.recibir(otra);
        assert_eq!(que, Recibido::Creado);
        assert_ne!(id, "a", "no puede compartir id con el que ya estaba");
        assert_eq!(i.proyectos.len(), 2);
        assert_eq!(i.buscar("a").unwrap().nombre, "Casa", "el de aqui, intacto");
        // Y el nuevo estrena codigo unico, para poder viajar sin pisar.
        let nuevo = i.buscar(&id).unwrap();
        assert_ne!(nuevo.uid.as_deref(), Some("VVT587BFCA"));
        assert_eq!(nuevo.uid.as_deref().map(str::len), Some(codigos::LARGO));
    }

    #[test]
    fn el_indice_va_y_vuelve_del_disco_sin_perder_campos() {
        let raiz = carpeta("disco");
        let mut i = Indice::default();
        let mut f = ficha("a", "Casa", 10);
        f.resto
            .insert("futuro".into(), serde_json::Value::Bool(true));
        i.proyectos.push(f);
        i.guardar(&raiz).unwrap();
        let vuelta = Indice::leer(&raiz);
        assert_eq!(vuelta, i);
        assert!(vuelta.proyectos[0].resto.contains_key("futuro"));
        // Caso negativo: sin fichero, lista vacia y ningun error.
        let _ = std::fs::remove_dir_all(&raiz);
        assert_eq!(Indice::leer(&raiz), Indice::default());
    }
}
