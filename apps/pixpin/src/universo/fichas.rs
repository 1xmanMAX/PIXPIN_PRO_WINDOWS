//! Del mensaje del chat a lo que el universo necesita (D237).

use std::path::Path;

use pixpin_proyecto::cuaderno::{Clase, Cuaderno, Mensaje};
use pixpin_universo::buscar::normalizar;
use pixpin_universo::ficha::{ClaseLuna, FichaLuna, es_colocable, extracto};

pub fn palabra_de(m: &Mensaje) -> &str {
    match &m.clase {
        Some(Clase::Nota) => "NOTA",
        Some(Clase::Imagen) => "IMAGEN",
        Some(Clase::Archivo) => "ARCHIVO",
        Some(Clase::Voz) => "VOZ",
        Some(Clase::Dibujo) => "DIBUJO",
        Some(Clase::Pagina) => "PAGINA",
        Some(Clase::Proyecto) => "PROYECTO",
        Some(Clase::MiniApp) => "MINIAPP",
        Some(Clase::Otra(p)) => p,
        None => "NOTA",
    }
}

pub fn de_mensaje(m: &Mensaje, raiz: &Path, proyecto: &str) -> FichaLuna {
    let clase = ClaseLuna::de_palabra(palabra_de(m));
    let texto = m.resumen();
    let ruta = m.ruta.clone().filter(|r| !r.is_empty());
    // Misma regla que `ventana_chat::ruta_del_mensaje`: una ruta absoluta es
    // de otro aparato y aqui no significa nada.
    let en_equipo = ruta.as_deref().is_some_and(|r| {
        !Path::new(r).is_absolute()
            && pixpin_proyecto::almacen::carpeta(raiz, proyecto)
                .join(r)
                .is_file()
    }) || matches!(clase, ClaseLuna::Nota | ClaseLuna::MiniApp);
    FichaLuna {
        codigo: m.codigo_unico(),
        proyecto: proyecto.to_string(),
        clase,
        nombre: if m.nombre.is_empty() {
            extracto(&texto)
        } else {
            m.nombre.clone()
        },
        ruta,
        bytes: m.bytes,
        cuando: m.cuando,
        codigo_chat: m.codigo_chat(),
        extracto: extracto(&texto),
        busqueda: normalizar(&format!("{} {}", m.nombre, extracto(&texto))),
        en_equipo,
        referencia: m.referencia.clone(),
    }
}

pub fn de_cuaderno(
    c: &Cuaderno,
    raiz: &Path,
    proyecto: &str,
    incluir_notas: bool,
) -> Vec<FichaLuna> {
    c.mensajes
        .iter()
        .filter(|m| {
            es_colocable(
                ClaseLuna::de_palabra(palabra_de(m)),
                m.en_buzon,
                incluir_notas,
            )
        })
        .map(|m| de_mensaje(m, raiz, proyecto))
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::cuaderno::{Clase, Mensaje};
    use pixpin_universo::ficha::ClaseLuna;

    fn mensaje(clase: Clase, ruta: Option<&str>) -> Mensaje {
        Mensaje {
            id: "1".into(),
            cuando: 5,
            clase: Some(clase),
            nombre: "Plano Cocina.pdf".into(),
            ruta: ruta.map(String::from),
            texto: "x".repeat(500),
            uid: Some("U1".into()),
            ..Default::default()
        }
    }

    #[test]
    fn una_ficha_lleva_el_codigo_unico_la_clase_y_un_extracto_de_doscientas() {
        let raiz = std::env::temp_dir();
        let f = de_mensaje(
            &mensaje(Clase::Archivo, Some("archivos/p.pdf")),
            &raiz,
            "p1",
        );
        assert_eq!(f.codigo, mensaje(Clase::Archivo, None).codigo_unico());
        assert_eq!(f.clase, ClaseLuna::Archivo);
        assert_eq!(f.extracto.chars().count(), 200);
        assert!(f.busqueda.contains("plano cocina"));
    }

    #[test]
    fn una_ruta_absoluta_del_movil_no_esta_en_este_equipo() {
        let raiz = std::env::temp_dir();
        let f = de_mensaje(
            &mensaje(Clase::Imagen, Some("/storage/emulated/0/a.jpg")),
            &raiz,
            "p1",
        );
        assert!(!f.en_equipo);
    }

    #[test]
    fn el_cuaderno_deja_fuera_el_buzon_y_las_notas_salvo_que_se_pidan() {
        let raiz = std::env::temp_dir();
        let mut buzon = mensaje(Clase::Imagen, None);
        buzon.en_buzon = true;
        buzon.id = "2".into();
        buzon.uid = Some("U2".into());
        let mut nota = mensaje(Clase::Nota, None);
        nota.id = "3".into();
        nota.uid = Some("U3".into());
        let c = pixpin_proyecto::cuaderno::Cuaderno {
            mensajes: vec![mensaje(Clase::Archivo, None), buzon, nota],
            lineas_rotas: 0,
        };
        assert_eq!(de_cuaderno(&c, &raiz, "p1", false).len(), 1);
        assert_eq!(de_cuaderno(&c, &raiz, "p1", true).len(), 2);
    }
}
