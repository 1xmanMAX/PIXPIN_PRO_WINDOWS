//! **Exportar una leccion** (14-oct-2026): por el componente comun
//! «Exportar» de toda la app (`compartir::Cosa::Visor`), como cualquier
//! visor. El movil no exporta lecciones (solo las comparte como texto), asi
//! que aqui se sigue la regla del PC: todo lo que se ve sale en PDF, HTML,
//! `.pixpin`, imagen y su formato original.
//!
//! - **Las hojas**: el resumen de la leccion (el mismo texto que lleva su
//!   mensaje en el chat, `Leccion::resumen`) en paginas A4, y detras una
//!   hoja por cada foto suya. De ellas salen PDF, HTML, PNG, JPG y SVG.
//! - **Original**: su `<id>.leccion` tal cual (el JSON que lee el movil).
//! - **.pixpin**: el chat donde vive, entero (la leccion es un mensaje suyo
//!   con sus fotos y audios): asi se abre igual en otro PixPin o en el movil.
//! - **Propios**: el texto en `.txt` y `.md`.

use std::path::Path;

use pixpin_proyecto::cuaderno::Clase;
use pixpin_store::{Idioma, Ubicacion};

use super::almacen::{self, Entrada};
use crate::compartir::exportable::{self, Exportable};

/// Lo que ofrece una leccion al «Exportar…».
pub(crate) fn exportable(raiz: &Path, e: &Entrada) -> Exportable {
    let l = &e.leccion;
    let titulo = if l.titulo.trim().is_empty() {
        l.id.clone()
    } else {
        l.titulo.lines().next().unwrap_or(&l.titulo).trim().to_string()
    };
    let mut x = exportable::de_texto(&titulo, &l.resumen(), Some(&e.archivo));
    x.raiz = Some(raiz.to_path_buf());
    x.proyecto = Some(e.ficha.clone());
    // Sus fotos, detras del texto: una hoja cada una.
    let mut n = 0;
    for m in almacen::adjuntos_de(raiz, e) {
        if m.clase != Some(Clase::Imagen) {
            continue;
        }
        let Some(ruta) = m
            .ruta
            .as_deref()
            .and_then(|r| pixpin_proyecto::vista::ruta_real(raiz, &e.ficha, r))
            .filter(|r| r.is_file())
        else {
            continue;
        };
        if let Ok(foto) = exportable::de_imagen(&titulo, &ruta, None) {
            for mut h in foto.hojas {
                n += 1;
                h.nombre = format!("{titulo} · {n}");
                x.hojas.push(h);
            }
        }
    }
    x
}

/// Abre la hoja de «Exportar» con la leccion `id` tal como esta guardada.
/// Devuelve si la encontro.
pub(crate) fn abrir(ubicacion: &Ubicacion, idioma: Idioma, id: &str) -> bool {
    let raiz = ubicacion.raiz();
    let Some(e) = almacen::listar(raiz).into_iter().find(|e| e.leccion.id == id) else {
        tracing::warn!(%id, "exportar: esa leccion no esta");
        return false;
    };
    let x = exportable(raiz, &e);
    crate::compartir::ventana::abrir(idioma, ubicacion.clone(), crate::compartir::Cosa::Visor(x));
    true
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_lecciones::Leccion;

    #[test]
    fn una_leccion_ofrece_pdf_html_pixpin_imagen_y_su_leccion() {
        let d = std::env::temp_dir().join(format!("pixpin-leccion-exportar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let archivo = d.join("k1.leccion");
        let mut l = Leccion::nueva("k1", 1_700_000_000_000, "Revisar el encofrado antes de vaciar");
        l.que_paso = "Se abrio una junta en la losa".into();
        l.proxima = "Si hay vaciado, entonces revisar puntales".into();
        std::fs::write(&archivo, l.escribir()).unwrap();
        let e = Entrada { leccion: l, mensaje: Default::default(), ficha: "p1".into(), archivo: archivo.clone() };
        let x = exportable(&d, &e);
        let f = x.formatos();
        for id in ["pdf", "web", "png", "jpg", "svg", "original", "pixpin", "texto", "md"] {
            assert!(f.contains(&id), "falta {id}: {f:?}");
        }
        assert_eq!(x.original.as_deref(), Some(archivo.as_path()), "el original es su .leccion");
        assert_eq!(x.proyecto.as_deref(), Some("p1"), "el .pixpin es su chat");
        assert_eq!(x.titulo, "Revisar el encofrado antes de vaciar");
        // Y se generan de verdad (el .pixpin sin chat en el disco cae en uno
        // nuevo con el .leccion dentro).
        let t = pixpin_store::Catalogo::nuevo(Idioma::Espanol);
        let p = crate::compartir::preparar(crate::compartir::Cosa::Visor(x), &t).unwrap();
        let claves: Vec<String> = p.piezas.iter().map(|x| x.pagina.clave.clone()).collect();
        let salida = d.join("salida");
        for id in ["pdf", "web", "png", "original", "pixpin"] {
            let s = crate::compartir::generar(&p, id, &claves, &salida).unwrap_or_else(|e| panic!("{id}: {e:#}"));
            assert!(s.bytes > 0 && s.ficheros.iter().all(|f| f.is_file()), "{id}");
        }
        // Caso negativo: una leccion que no esta no abre nada.
        let u = Ubicacion::Portable { raiz: d.clone() };
        assert!(!abrir(&u, Idioma::Espanol, "no-esta"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
