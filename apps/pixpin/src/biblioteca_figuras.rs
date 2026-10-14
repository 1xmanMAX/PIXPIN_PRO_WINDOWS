//! **La biblioteca de figuras viaja con los aparatos propios** (14-oct).
//!
//! `figuras.json` (formato del movil, `BibliotecaStore.kt`: ver
//! `pixpin_motor2d::biblioteca`) vive en la carpeta del chat «Asistente»,
//! `proyectos/<asistente>/android/pins/draw/figuras.json`, que para el movil
//! es `pixpin:files/pins/draw/figuras.json`: **justo donde
//! `BibliotecaStore` lo lee y lo escribe** (`files/pins/draw/figuras.json`).
//! Un mensaje ARCHIVO fijo de ese chat ([`ID_MENSAJE`]) lo senala y la
//! sincronizacion del grupo lo lleva como a las automatizaciones
//! (`automatizaciones::senalar_fichero`): solo a los aparatos propios, nunca
//! a las personas de un chat compartido (el chat «Asistente» no se comparte).
//!
//! La biblioteca de antes (`<raiz>/figuras.json`, con el formato interno del
//! PC) se pasa aqui la primera vez y se deja donde estaba.

use std::path::{Path, PathBuf};

use crate::automatizaciones::{FicheroQueViaja, chat_del_asistente, senalar_fichero};

/// El id del mensaje que la senala.
pub const ID_MENSAJE: &str = "auto-figuras";
/// Donde la deja el movil.
pub const RUTA_PORTATIL: &str = "pixpin:files/pins/draw/figuras.json";
/// Dentro de la carpeta del chat «Asistente».
pub const RUTA_EN_EL_CHAT: &str = "android/pins/draw/figuras.json";

const FICHERO: FicheroQueViaja = FicheroQueViaja {
    id_mensaje: ID_MENSAJE,
    nombre: "figuras.json",
    ruta_portatil: RUTA_PORTATIL,
    en_el_chat: RUTA_EN_EL_CHAT,
    texto: "🧩 Las figuras guardadas del lienzo (se usan desde el menu de figuras)",
};

/// **La ruta de la biblioteca** en esta raiz (creando el chat «Asistente» si
/// no estaba y pasando la de antes si la hay).
pub fn ruta_en(raiz: &Path, aparato: &str, ahora: i64) -> std::io::Result<PathBuf> {
    let (_, carpeta) = chat_del_asistente(raiz, aparato, ahora)?;
    let ruta = carpeta.join(RUTA_EN_EL_CHAT);
    let vieja = raiz.join("figuras.json");
    if !ruta.exists() && vieja.exists() {
        let figuras = pixpin_motor2d::biblioteca::cargar(&vieja);
        if !figuras.is_empty() {
            pixpin_motor2d::biblioteca::guardar(&ruta, &figuras);
            senalar(&ruta, aparato, ahora)?;
        }
    }
    Ok(ruta)
}

/// La raiz de los datos de este PC y el codigo del equipo.
fn raiz_y_aparato() -> (PathBuf, String) {
    let dir_exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default();
    let raiz = pixpin_store::resolver(&dir_exe, &appdata).raiz().to_path_buf();
    let aparato = pixpin_proyecto::identidad::Identidad::leer_o_crear(&raiz, "PC")
        .map(|i| i.yo.codigo())
        .unwrap_or_default();
    (raiz, aparato)
}

/// La ruta de la biblioteca de este PC. Si el chat «Asistente» no se puede
/// crear, la de antes (sin sincronizar, pero sin perder nada).
pub fn ruta() -> PathBuf {
    let (raiz, aparato) = raiz_y_aparato();
    ruta_en(&raiz, &aparato, pixpin_shell::entorno::ahora_utc_ms()).unwrap_or_else(|e| {
        tracing::warn!(%e, "figuras: sin chat del asistente, la biblioteca no se sincroniza");
        raiz.join("figuras.json")
    })
}

/// **Tras guardar en `ruta`**: si es la del chat «Asistente», pone (una
/// vez) el mensaje que la hace viajar. Una ruta cualquiera (pruebas, la de
/// antes) no se toca.
pub fn senalar(ruta: &Path, aparato: &str, ahora: i64) -> std::io::Result<()> {
    let partes = Path::new(RUTA_EN_EL_CHAT).components().count();
    let Some(carpeta) = ruta.ancestors().nth(partes) else { return Ok(()) };
    if carpeta.join(RUTA_EN_EL_CHAT) != ruta {
        return Ok(());
    }
    let Some(proyecto) = carpeta.file_name().and_then(|n| n.to_str()) else { return Ok(()) };
    if carpeta.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()) != Some("proyectos") {
        return Ok(());
    }
    senalar_fichero(carpeta, proyecto, aparato, ahora, &FICHERO)
}

/// Guarda la lista entera y la hace viajar.
pub fn guardar(ruta: &Path, figuras: &[pixpin_motor2d::biblioteca::FiguraGuardada]) -> bool {
    let ok = pixpin_motor2d::biblioteca::guardar(ruta, figuras);
    if ok {
        avisar(ruta);
    }
    ok
}

/// Anade una figura (la del mismo nombre la sustituye) y la hace viajar.
pub fn anadir(ruta: &Path, figura: pixpin_motor2d::biblioteca::FiguraGuardada) -> bool {
    let ok = pixpin_motor2d::biblioteca::anadir(ruta, figura);
    if ok {
        avisar(ruta);
    }
    ok
}

fn avisar(ruta: &Path) {
    let (_, aparato) = raiz_y_aparato();
    if let Err(e) = senalar(ruta, &aparato, pixpin_shell::entorno::ahora_utc_ms()) {
        tracing::warn!(%e, "figuras: no se pudo senalar la biblioteca para la sincro");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::biblioteca::{self, FiguraGuardada};
    use pixpin_motor2d::elemento::{Elemento, Figura};
    use pixpin_proyecto::{almacen, cuaderno};

    fn raiz_de_prueba(nombre: &str) -> PathBuf {
        let raiz = std::env::temp_dir().join(format!("pixpin-figuras-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let obra = almacen::Ficha::nueva("Obra", 1, "PC01");
        almacen::Indice { proyectos: vec![obra], ..Default::default() }.guardar(&raiz).unwrap();
        raiz
    }

    fn figura(nombre: &str) -> FiguraGuardada {
        FiguraGuardada {
            id: format!("id-{nombre}"),
            nombre: nombre.into(),
            elementos: vec![Elemento { figura: Figura::Elipse, ancho: 40.0, alto: 20.0, ..Default::default() }],
        }
    }

    /// **Ida y vuelta con el movil**: el PC guarda, el fichero cae donde la
    /// sincro lo deja en el movil (`pixpin:files/pins/draw/figuras.json`, la
    /// ruta de `BibliotecaStore`), el movil lo lee, anade la suya como la
    /// escribe `BibliotecaStore.anadir` (al principio) y la vuelta llega al
    /// PC con las dos.
    #[test]
    fn la_biblioteca_va_al_movil_y_vuelve_por_el_chat_del_asistente() {
        let raiz = raiz_de_prueba("idavuelta");
        let ruta = ruta_en(&raiz, "PC01", 5).unwrap();
        assert!(ruta.ends_with(r"android\pins\draw\figuras.json"), "{ruta:?}");
        assert!(anadir(&ruta, figura("Arbol isometrico")));
        senalar(&ruta, "PC01", 6).unwrap();
        // Un solo mensaje ARCHIVO aunque se guarde dos veces, en el chat
        // «Asistente» (no en la obra, que podria compartirse).
        assert!(anadir(&ruta, figura("Norte")));
        senalar(&ruta, "PC01", 7).unwrap();
        let (asistente, carpeta) = chat_del_asistente(&raiz, "PC01", 8).unwrap();
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        let senal: Vec<_> = c.mensajes.iter().filter(|m| m.id == ID_MENSAJE).collect();
        assert_eq!(senal.len(), 1);
        assert_eq!(senal[0].ruta.as_deref(), Some(RUTA_PORTATIL));
        assert_eq!(senal[0].clase, Some(cuaderno::Clase::Archivo));
        let obra = almacen::Indice::leer(&raiz).proyectos.into_iter().find(|f| f.nombre == "Obra").unwrap();
        let en_la_obra = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&raiz, &obra.id)).map(|c| c.mensajes.len()).unwrap_or(0);
        assert_eq!(en_la_obra, 0, "nada en un chat que se puede compartir");
        // La ruta portatil es el fichero de verdad: asi lo lleva la sincro.
        let real = pixpin_proyecto::vista::ruta_real(&raiz, &asistente, RUTA_PORTATIL).unwrap();
        assert_eq!(real, ruta);
        // «El movil»: files/pins/draw/figuras.json con lo que llego.
        let movil = raiz.join("movil").join("files").join("pins").join("draw").join("figuras.json");
        std::fs::create_dir_all(movil.parent().unwrap()).unwrap();
        std::fs::copy(&ruta, &movil).unwrap();
        let mut lista: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(&movil).unwrap()).unwrap();
        // Lo que lee `ExcalidrawJson.decodeFromString<List<FiguraGuardada>>`.
        for f in &lista {
            assert!(f["id"].is_string() && f["nombre"].is_string() && f["propia"] == true);
            assert!(f["elementos"].as_array().unwrap().iter().all(|e| e["type"].is_string()));
        }
        lista.insert(0, serde_json::json!({"id": "and-1", "nombre": "Sello del movil", "propia": true, "elementos": [
            {"id": "m1", "type": "rectangle", "x": 0, "y": 0, "width": 30, "height": 10, "strokeColor": "#1e1e1e",
             "backgroundColor": "transparent", "seed": 2, "version": 1, "isDeleted": false}]}));
        std::fs::write(&movil, serde_json::to_string(&lista).unwrap()).unwrap();
        // La vuelta: la sincro deja el fichero del movil en la misma ruta.
        std::fs::copy(&movil, &ruta).unwrap();
        let pc = biblioteca::cargar(&ruta_en(&raiz, "PC01", 9).unwrap());
        assert_eq!(
            pc.iter().map(|f| f.nombre.as_str()).collect::<Vec<_>>(),
            ["Sello del movil", "Norte", "Arbol isometrico"]
        );
        assert_eq!(pc[1].id, "id-Norte", "los ids viajan intactos");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn la_biblioteca_de_antes_se_pasa_una_vez_y_una_ruta_cualquiera_no_se_senala() {
        let raiz = raiz_de_prueba("migrar");
        // La de antes, con el formato interno del PC.
        std::fs::write(raiz.join("figuras.json"), serde_json::to_string(&vec![figura("Vieja")]).unwrap()).unwrap();
        let ruta = ruta_en(&raiz, "PC01", 5).unwrap();
        assert_eq!(biblioteca::cargar(&ruta)[0].nombre, "Vieja");
        // Ya en el formato del movil.
        assert!(std::fs::read_to_string(&ruta).unwrap().contains("\"type\""));
        let (_, carpeta) = chat_del_asistente(&raiz, "PC01", 6).unwrap();
        assert!(cuaderno::Cuaderno::leer_de(&carpeta).unwrap().mensajes.iter().any(|m| m.id == ID_MENSAJE));
        // Caso negativo: un fichero fuera de un chat no crea mensajes.
        let suelto = raiz.join("otra").join("figuras.json");
        senalar(&suelto, "PC01", 7).unwrap();
        assert!(!raiz.join("otra").join("cuaderno.json").exists());
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
