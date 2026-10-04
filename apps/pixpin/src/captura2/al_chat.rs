//! **«Al chat»: la captura, al chat de un proyecto en un paso.**
//!
//! La foto entra como una imagen del chat, con el comentario como su pie
//! (el `texto` de la IMAGEN, como la deja el movil), y el proyecto sube en
//! la lista. Es lo mismo que soltar un fichero en la ventana del chat
//! (`ventana_chat::adjuntar_en_proyecto`), con el pie puesto antes de
//! escribir: un mensaje, una linea en el cuaderno.

use std::cell::RefCell;
use std::path::Path;

use pixpin_codec::ImagenRgba;
use pixpin_proyecto::{almacen, cuaderno};

thread_local! {
    /// El ultimo proyecto al que se mando una captura en esta sesion: sale
    /// el primero en el selector, con «el ultimo».
    static ULTIMO: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn ultimo() -> Option<String> {
    ULTIMO.with(|u| u.borrow().clone())
}

/// Los proyectos para el selector, `(id, nombre)`: el ultimo usado primero
/// y despues los de la lista en su orden (lo ultimo tocado arriba).
pub fn proyectos(raiz: &Path) -> Vec<(String, String)> {
    let indice = almacen::Indice::leer(raiz);
    ordenar(
        indice
            .ordenadas()
            .into_iter()
            .map(|f| (f.id.clone(), f.nombre.clone()))
            .collect(),
        ultimo().as_deref(),
    )
}

fn ordenar(mut v: Vec<(String, String)>, ultimo: Option<&str>) -> Vec<(String, String)> {
    if let Some(u) = ultimo {
        if let Some(i) = v.iter().position(|(id, _)| id == u) {
            let p = v.remove(i);
            v.insert(0, p);
        }
    }
    v
}

/// Manda la captura al chat de `proyecto`. Devuelve el nombre del proyecto.
pub fn mandar(
    raiz: &Path,
    proyecto: &str,
    aparato: &str,
    imagen: &ImagenRgba,
    comentario: &str,
) -> anyhow::Result<String> {
    let indice = almacen::Indice::leer(raiz);
    let ficha = indice
        .proyectos
        .iter()
        .find(|f| f.id == proyecto)
        .ok_or_else(|| anyhow::anyhow!("el proyecto {proyecto} ya no existe"))?
        .clone();
    let png = pixpin_codec::imagen::codificar_png(imagen)?;
    let cuando = pixpin_shell::entorno::ahora_utc_ms();
    let nombre = format!("captura-{cuando}.png");
    let ruta = almacen::guardar_adjunto(raiz, proyecto, &nombre, &png)?;
    let numero = crate::ventana_chat::siguiente_numero_en(raiz, proyecto)?;
    let mut mensaje = cuaderno::Mensaje::adjunto(
        cuaderno::clase_de_nombre(&nombre),
        &nombre,
        &ruta,
        png.len() as i64,
        &cuaderno::Sello {
            cuando,
            numero,
            aparato: aparato.to_string(),
            proyecto: proyecto.to_string(),
        },
    );
    mensaje.texto = comentario.trim().to_string();
    cuaderno::anadir(&almacen::carpeta(raiz, proyecto), &mensaje)?;
    crate::ventana_chat::subir_en_la_lista(raiz, proyecto, cuando, &mensaje.resumen())?;
    crate::ventana_chat::refrescar();
    ULTIMO.with(|u| *u.borrow_mut() = Some(proyecto.to_string()));
    Ok(ficha.nombre)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_ultimo_usado_sale_primero_y_lo_demas_no_se_mueve() {
        let v = vec![
            ("a".to_string(), "Guardados".to_string()),
            ("b".to_string(), "Obra".to_string()),
            ("c".to_string(), "Tesis".to_string()),
        ];
        let o = ordenar(v.clone(), Some("c"));
        assert_eq!(o[0].0, "c");
        assert_eq!(o[1].0, "a");
        assert_eq!(o[2].0, "b");
        // Casos negativos: sin ultimo, o con uno que ya no existe, igual.
        assert_eq!(ordenar(v.clone(), None), v);
        assert_eq!(ordenar(v.clone(), Some("zz")), v);
    }

    #[test]
    fn mandar_deja_la_foto_con_su_pie_en_el_chat_del_proyecto() {
        let raiz = std::env::temp_dir().join(format!(
            "pixpin-captura2-alchat-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        let f = almacen::Ficha::nueva("Tesis", 1, "PC01");
        let mut ind = almacen::Indice::default();
        ind.proyectos.push(f.clone());
        ind.guardar(&raiz).unwrap();
        let img = ImagenRgba {
            ancho: 2,
            alto: 2,
            pixeles: vec![255; 16],
        };
        let nombre = mandar(&raiz, &f.id, "PC01", &img, "  Revisar este monto ").unwrap();
        assert_eq!(nombre, "Tesis");
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&raiz, &f.id)).unwrap();
        assert_eq!(c.mensajes.len(), 1);
        assert_eq!(c.mensajes[0].texto, "Revisar este monto");
        assert_eq!(ultimo().as_deref(), Some(f.id.as_str()));
        // Caso negativo: a un proyecto que no existe no se manda nada.
        assert!(mandar(&raiz, "no-existe", "PC01", &img, "").is_err());
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
