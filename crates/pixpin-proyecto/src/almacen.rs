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
    /// Un proyecto creado AQUI, vacio y con sus tres codigos recien puestos.
    ///
    /// Se le pone `aparato` y `uid` propios, no los de nadie: es lo que hace
    /// que, cuando este proyecto llegue al movil, se reconozca como el mismo
    /// si vuelve, en vez de duplicarse en cada viaje (ver `recibir`).
    ///
    /// No crea la carpeta: eso lo hace quien escriba el primer mensaje, y
    /// asi un proyecto que se crea y se descarta no deja nada en el disco.
    pub fn nueva(nombre: &str, cuando: i64, aparato: &str) -> Ficha {
        Ficha {
            id: codigos::nuevo(),
            nombre: nombre.to_string(),
            uid: Some(codigos::nuevo()),
            creado: cuando,
            aparato: Some(aparato.to_string()),
            tocado: cuando,
            hojas: 0,
            resumen: String::new(),
            sin_leer: 0,
            paquete: None,
            resto: Default::default(),
        }
    }

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

/// El fichero de un lienzo del proyecto, con el mismo nombre que dentro del
/// `.pixpin` (`lienzos/<dibujo>.excalidraw`): asi un proyecto abierto y uno
/// empaquetado se leen igual.
pub fn lienzo(raiz: &Path, id: &str, dibujo: &str) -> PathBuf {
    carpeta(raiz, id)
        .join("lienzos")
        .join(format!("{dibujo}.excalidraw"))
}

/// Deja un nombre de fichero en algo que se pueda escribir en cualquier
/// disco: sin separadores, sin los signos que Windows prohibe y sin los
/// nombres reservados. Un nombre que venga de fuera no puede decidir donde
/// se escribe.
pub fn nombre_seguro(nombre: &str) -> String {
    const PROHIBIDOS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let limpio: String = nombre
        .chars()
        .map(|c| {
            if PROHIBIDOS.contains(&c) || (c as u32) < 0x20 {
                '_'
            } else {
                c
            }
        })
        .collect();
    // Windows tampoco admite que acabe en punto o espacio.
    let limpio = limpio.trim().trim_end_matches('.').trim();
    if limpio.is_empty() {
        return "archivo".to_string();
    }
    // Los nombres reservados de DOS siguen vivos y no se pueden crear.
    const RESERVADOS: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let raiz = limpio.split('.').next().unwrap_or(limpio);
    if RESERVADOS.iter().any(|r| raiz.eq_ignore_ascii_case(r)) {
        return format!("_{limpio}");
    }
    limpio.to_string()
}

/// Guarda un fichero dentro del proyecto y devuelve su ruta **relativa a la
/// carpeta del proyecto**, que es lo que se apunta en el mensaje.
///
/// Se copia en vez de apuntar al original a proposito: el original se mueve,
/// se renombra o se borra, y un proyecto que viaja no puede depender de una
/// ruta del escritorio de nadie.
///
/// Si ya hay uno con ese nombre se le pone un numero, en vez de pisarlo: dos
/// `captura.png` de dos sitios distintos son dos ficheros distintos.
pub fn guardar_adjunto(
    raiz: &Path,
    id: &str,
    nombre: &str,
    bytes: &[u8],
) -> std::io::Result<String> {
    let carpeta = carpeta(raiz, id).join("archivos");
    std::fs::create_dir_all(&carpeta)?;
    let seguro = nombre_seguro(nombre);
    let (tronco, extension) = match seguro.rsplit_once('.') {
        Some((t, e)) if !t.is_empty() => (t.to_string(), format!(".{e}")),
        _ => (seguro.clone(), String::new()),
    };
    let mut intento = seguro.clone();
    let mut n = 1;
    while carpeta.join(&intento).exists() {
        intento = format!("{tronco} ({n}){extension}");
        n += 1;
        if n > 9999 {
            return Err(std::io::Error::other("demasiados ficheros con ese nombre"));
        }
    }
    std::fs::write(carpeta.join(&intento), bytes)?;
    Ok(format!("archivos/{intento}"))
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

    fn carpeta_temporal(etiqueta: &str) -> PathBuf {
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
    fn un_proyecto_creado_aqui_nace_con_sus_tres_codigos_y_no_se_duplica() {
        let f = Ficha::nueva("Obra nueva", 1_000, "K7Q2");
        assert_eq!(f.nombre, "Obra nueva");
        assert_eq!(f.aparato.as_deref(), Some("K7Q2"));
        assert_eq!((f.creado, f.tocado), (1_000, 1_000));
        assert!(f.uid.is_some(), "sin uid no se reconoceria al volver");
        assert_eq!(f.paquete, None, "no salio de ningun fichero");

        // Dos creados seguidos son cosas distintas: si compartieran codigos,
        // el segundo pisaria al primero al viajar.
        let otro = Ficha::nueva("Obra nueva", 1_000, "K7Q2");
        assert_ne!(f.id, otro.id);
        assert_ne!(f.uid, otro.uid);
        assert!(!f.misma_que(&otro), "mismo nombre no es la misma cosa");

        // Y el que vuelve del movil se reconoce en vez de duplicarse.
        let mut i = Indice::default();
        i.proyectos.push(f.clone());
        let (que, _) = i.recibir(f);
        assert_eq!(que, Recibido::Actualizado);
        assert_eq!(i.proyectos.len(), 1);
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
        let raiz = carpeta_temporal("disco");
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

    #[test]
    fn un_nombre_de_fuera_no_puede_decidir_donde_se_escribe() {
        // Lo importante: nada que salga de la carpeta.
        for malo in ["../../pasa.txt", r"c:\windows\system32\a.dll", "a/b/c.png"] {
            let s = nombre_seguro(malo);
            assert!(!s.contains('/') && !s.contains('\\'), "{malo} -> {s}");
            assert!(!s.contains(':'), "{malo} -> {s}");
        }
        // Los nombres que Windows no deja crear.
        assert_eq!(nombre_seguro("CON"), "_CON");
        assert_eq!(nombre_seguro("nul.txt"), "_nul.txt");
        assert_eq!(nombre_seguro("fin."), "fin");
        assert_eq!(nombre_seguro("   "), "archivo");
        // Y uno normal se queda como esta, acentos incluidos.
        assert_eq!(nombre_seguro("Plano fachada ñ.pdf"), "Plano fachada ñ.pdf");
    }

    #[test]
    fn dos_ficheros_con_el_mismo_nombre_no_se_pisan() {
        let raiz = carpeta_temporal("adjuntos");
        let a = guardar_adjunto(&raiz, "pr-1", "captura.png", b"primero").unwrap();
        let b = guardar_adjunto(&raiz, "pr-1", "captura.png", b"segundo").unwrap();
        assert_eq!(a, "archivos/captura.png");
        assert_eq!(
            b, "archivos/captura (1).png",
            "el segundo no pisa al primero"
        );
        let base = carpeta(&raiz, "pr-1");
        assert_eq!(std::fs::read(base.join(&a)).unwrap(), b"primero");
        assert_eq!(std::fs::read(base.join(&b)).unwrap(), b"segundo");
        // Y cada proyecto tiene los suyos.
        let c = guardar_adjunto(&raiz, "pr-2", "captura.png", b"de otro").unwrap();
        assert_eq!(c, "archivos/captura.png");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_nombre_sin_extension_tambien_se_numera_bien() {
        let raiz = carpeta_temporal("sin-extension");
        assert_eq!(
            guardar_adjunto(&raiz, "p", "LEEME", b"1").unwrap(),
            "archivos/LEEME"
        );
        assert_eq!(
            guardar_adjunto(&raiz, "p", "LEEME", b"2").unwrap(),
            "archivos/LEEME (1)"
        );
        // Caso negativo: uno que empieza por punto es todo extension.
        assert_eq!(
            guardar_adjunto(&raiz, "p", ".gitignore", b"3").unwrap(),
            "archivos/.gitignore"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
