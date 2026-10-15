//! **Cambiar el nombre de un archivo del chat**, tambien de los audios
//! (`guardados/Renombrar.kt` del movil, 4-oct-2026).
//!
//! Cambia `Mensaje::nombre`, que es lo que se ensena y lo que encuentra el
//! buscador; el archivo en disco no se toca, porque la sincronizacion
//! empareja por su ruta. Un lienzo sigue su camino de siempre (su nombre es
//! tambien el de su hoja), asi que aqui solo se decide que se puede
//! renombrar y como queda el nombre.

use pixpin_proyecto::cuaderno::{Clase, Mensaje};

/// Lo mas largo que se guarda, como en el movil.
const TOPE: usize = 120;

/// Si a `m` se le puede poner nombre: un archivo con fichero o con hoja, que
/// no sea una leccion ni algo del buzon.
pub(crate) fn se_puede(m: &Mensaje) -> bool {
    matches!(
        m.clase,
        Some(Clase::Imagen | Clase::Archivo | Clase::Voz | Clase::Dibujo | Clase::Pagina)
    ) && !m.en_buzon
        && (m.ruta.is_some() || m.referencia.is_some())
        && !crate::lecciones::almacen::es_leccion(m)
}

/// `nuevo` limpio y **con la extension que tenia** `viejo` si no se la puso:
/// «informe.pdf» renombrado a «Memoria» queda «Memoria.pdf», y sigue
/// saliendo con su icono y abriendose con su visor. Vacio si no queda nada.
pub(crate) fn con_su_extension(viejo: &str, nuevo: &str) -> String {
    let limpio: String = nuevo
        .replace('\n', " ")
        .replace('/', "-")
        .trim()
        .chars()
        .take(TOPE)
        .collect();
    if limpio.is_empty() {
        return String::new();
    }
    let Some(ext) = extension(viejo) else {
        return limpio;
    };
    if limpio.to_lowercase().ends_with(&ext.to_lowercase()) {
        limpio
    } else {
        limpio + ext
    }
}

/// La extension de `nombre` con su punto: de uno a cinco letras o cifras al
/// final (`\.[A-Za-z0-9]{1,5}$` en el movil).
fn extension(nombre: &str) -> Option<&str> {
    let punto = nombre.rfind('.')?;
    let ext = &nombre[punto + 1..];
    (1..=5)
        .contains(&ext.len())
        .then_some(())
        .filter(|_| ext.chars().all(|c| c.is_ascii_alphanumeric()))
        .map(|_| &nombre[punto..])
}

/// El nombre que se le puso a una nota de voz, sin su «.m4a», para ensenarlo
/// en la burbuja. `None` si no tiene o es el de serie (`voz_<hora>.m4a` del
/// PC, `voz-<hora>.m4a` de lo que llega): esos no le dicen nada a nadie.
pub(crate) fn nombre_propio_de_voz(m: &Mensaje) -> Option<String> {
    let n = m.nombre.trim();
    let base = n.strip_suffix(".m4a").unwrap_or(n);
    let de_serie = ["voz_", "voz-"].iter().any(|p| {
        base.strip_prefix(p)
            .is_some_and(|resto| !resto.is_empty() && resto.chars().all(|c| c.is_ascii_digit()))
    });
    (!base.is_empty() && !de_serie).then(|| base.to_string())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_voz_ensena_el_nombre_que_se_le_puso_y_no_el_de_serie() {
        let voz = |nombre: &str| Mensaje {
            clase: Some(Clase::Voz),
            nombre: nombre.into(),
            ..Default::default()
        };
        assert_eq!(
            nombre_propio_de_voz(&voz("Clase de física.m4a")).as_deref(),
            Some("Clase de física")
        );
        // Casos negativos: sin nombre y con el de serie.
        assert_eq!(nombre_propio_de_voz(&voz("")), None);
        assert_eq!(nombre_propio_de_voz(&voz("voz_1758123.m4a")), None);
        assert_eq!(nombre_propio_de_voz(&voz("voz-1758123.m4a")), None);
    }

    #[test]
    fn conserva_la_extension() {
        assert_eq!(
            con_su_extension("informe (1).pdf", "  Memoria "),
            "Memoria.pdf"
        );
        assert_eq!(
            con_su_extension("informe.pdf", "Memoria.PDF"),
            "Memoria.PDF"
        );
        assert_eq!(
            con_su_extension("voz_123.m4a", "Clase de física"),
            "Clase de física.m4a"
        );
        assert_eq!(con_su_extension("Nota de voz 14:32", "Clase 3"), "Clase 3");
        assert_eq!(con_su_extension("a.pdf", "   "), "");
        assert_eq!(con_su_extension("x.pdf", "a/b"), "a-b.pdf");
    }

    #[test]
    fn que_se_puede_renombrar() {
        let m = |c: Clase, ruta: Option<&str>| Mensaje {
            id: "1".into(),
            clase: Some(c),
            ruta: ruta.map(str::to_string),
            ..Default::default()
        };
        assert!(se_puede(&m(Clase::Voz, Some("archivos/x.m4a"))));
        assert!(se_puede(&m(Clase::Imagen, Some("archivos/x.png"))));
        assert!(se_puede(&m(Clase::Archivo, Some("archivos/x.pdf"))));
        // Casos negativos: una nota sin fichero y una leccion.
        assert!(!se_puede(&m(Clase::Nota, None)));
        let mut leccion = m(
            Clase::Archivo,
            Some("pixpin:files/guardados/lecciones/a.leccion"),
        );
        leccion.id = "lec-a".into();
        assert!(!se_puede(&leccion));
    }
}
