//! **La descripcion de una foto** (el usuario, 3-oct: «cuando envio un texto
//! y una imagen pasa que solo se envia la imagen y el texto desaparece»).
//!
//! Se guarda como en PixPin Android: en `Mensaje::texto` del propio mensaje
//! IMAGEN (o ARCHIVO), no en una nota aparte. El movil lo ensena debajo de la
//! foto, dentro de la burbuja (`alPie` de `MensajesActivity.kt`: para lo que
//! no es una nota ni una mini-app, `m.texto` si no esta en blanco), y una
//! foto con texto deja de comerse la burbuja (`soloFoto` pide `texto` vacio).
//! Aqui `texto_de` ya hacia lo mismo; lo que faltaba era escribirlo.

use pixpin_proyecto::cuaderno::{Clase, Mensaje};

/// Si a `m` se le puede poner o cambiar la descripcion: una foto o un
/// archivo con su fichero. Una leccion no, que su `texto` es su resumen, ni
/// lo del buzon, que caduca.
pub(crate) fn se_puede(m: &Mensaje) -> bool {
    matches!(m.clase, Some(Clase::Imagen | Clase::Archivo))
        && !m.en_buzon
        && m.ruta.is_some()
        && !crate::lecciones::almacen::es_leccion(m)
}

/// `m` con `texto` de descripcion, o `None` si no cambia nada. Vacio la
/// quita: es la forma de borrarla.
pub(crate) fn con_descripcion(m: &Mensaje, texto: &str) -> Option<Mensaje> {
    let limpio = texto.trim();
    if limpio == m.texto.trim() {
        return None;
    }
    let mut nuevo = m.clone();
    nuevo.texto = limpio.to_string();
    Some(nuevo)
}

/// Lo escrito vuelve a la caja al cancelar el cuadro de confirmar: se saco
/// de ahi como pie (`Pendientes`), y cancelar no puede ser perderlo. Si la
/// caja ya tiene algo (no deberia, el cuadro es modal) no se pisa.
pub(crate) fn pie_de_vuelta(borrador: &mut String, pie: String) {
    if borrador.trim().is_empty() && !pie.trim().is_empty() {
        *borrador = pie;
    }
}

/// La hora de un mensaje nuevo dado el ultimo repartido: la de ahora, o uno
/// mas que la ultima si ya se dio esta. El `id` de un mensaje es su hora en
/// milisegundos (`Mensaje::adjunto`, `Mensaje::nota`), y dos mensajes en el
/// mismo milisegundo —dos fotos soltadas juntas, una foto y su nota— salian
/// con el MISMO id: cambiar uno reescribia los dos y el movil se quedaba con
/// uno solo.
pub(crate) fn siguiente_hora(ahora: i64, ultima: i64) -> i64 {
    ahora.max(ultima + 1)
}

/// [`siguiente_hora`] con el reloj de verdad y la ultima de todo el proceso
/// (la caja de soltar y los pedidos adjuntan desde otros hilos).
pub(crate) fn hora_sin_repetir() -> i64 {
    use std::sync::atomic::{AtomicI64, Ordering};
    static ULTIMA: AtomicI64 = AtomicI64::new(0);
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let mut ultima = ULTIMA.load(Ordering::Relaxed);
    loop {
        let toca = siguiente_hora(ahora, ultima);
        match ULTIMA.compare_exchange_weak(ultima, toca, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return toca,
            Err(otra) => ultima = otra,
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn foto(texto: &str) -> Mensaje {
        Mensaje {
            id: "1".into(),
            clase: Some(Clase::Imagen),
            ruta: Some("archivos/foto.png".into()),
            texto: texto.into(),
            ..Default::default()
        }
    }

    #[test]
    fn se_describe_una_foto_y_un_archivo() {
        assert!(se_puede(&foto("")));
        let mut archivo = foto("");
        archivo.clase = Some(Clase::Archivo);
        assert!(se_puede(&archivo));
        // Casos negativos: una nota, una foto del buzon y una sin fichero.
        let mut nota = foto("hola");
        nota.clase = Some(Clase::Nota);
        assert!(!se_puede(&nota));
        let mut buzon = foto("");
        buzon.en_buzon = true;
        assert!(!se_puede(&buzon));
        let mut sin_fichero = foto("");
        sin_fichero.ruta = None;
        assert!(!se_puede(&sin_fichero));
    }

    #[test]
    fn la_descripcion_se_pone_se_cambia_y_se_quita() {
        let puesta = con_descripcion(&foto(""), "  La pared norte  ").unwrap();
        assert_eq!(puesta.texto, "La pared norte");
        assert_eq!(puesta.id, "1", "es el mismo mensaje, no uno nuevo");
        let quitada = con_descripcion(&puesta, "   ").unwrap();
        assert!(quitada.texto.is_empty());
        // Caso negativo: lo mismo de antes no reescribe nada.
        assert!(con_descripcion(&puesta, "La pared norte ").is_none());
        assert!(con_descripcion(&foto(""), "").is_none());
    }

    #[test]
    fn cancelar_devuelve_el_pie_a_la_caja() {
        let mut borrador = String::new();
        pie_de_vuelta(&mut borrador, "mira esto".into());
        assert_eq!(borrador, "mira esto");
        // Casos negativos: no pisa lo que ya hay ni pone espacios.
        let mut lleno = String::from("otra cosa");
        pie_de_vuelta(&mut lleno, "mira esto".into());
        assert_eq!(lleno, "otra cosa");
        let mut vacio = String::new();
        pie_de_vuelta(&mut vacio, "  ".into());
        assert!(vacio.is_empty());
    }

    #[test]
    fn dos_mensajes_seguidos_nunca_comparten_hora() {
        assert_eq!(siguiente_hora(1000, 0), 1000);
        assert_eq!(
            siguiente_hora(1000, 1000),
            1001,
            "mismo milisegundo, uno mas"
        );
        // Caso negativo: un reloj que va hacia atras tampoco repite.
        assert_eq!(siguiente_hora(900, 1001), 1002);
        let a = hora_sin_repetir();
        let b = hora_sin_repetir();
        assert!(b > a);
    }
}
