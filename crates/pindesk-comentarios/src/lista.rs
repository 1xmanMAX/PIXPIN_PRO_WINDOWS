//! **La lista «Comentarios» de un documento**: todas sus pegatinas, las
//! abiertas primero, por urgencia (urgente, alta, media, nota) y, dentro de
//! cada una, lo mas reciente arriba. Las resueltas, al final y en el mismo
//! orden. Un clic en una fila va a su pegatina.

use crate::modelo::{Comentario, Pegatinas, Tipo};

/// Una fila de la lista.
#[derive(Debug, Clone, PartialEq)]
pub struct Fila {
    pub id: String,
    pub tipo: Tipo,
    pub resumen: String,
    /// Lo ultimo que paso (milisegundos).
    pub cuando: i64,
    pub resuelto: bool,
    /// Cuantas entradas tiene el hilo.
    pub entradas: usize,
    /// Si tiene tarea creada.
    pub con_tarea: bool,
}

fn fila(c: &Comentario) -> Fila {
    Fila {
        id: c.id.clone(),
        tipo: c.tipo,
        resumen: c.resumen(),
        cuando: c.ultimo(),
        resuelto: c.resuelto,
        entradas: c.entradas().count(),
        con_tarea: c.tarea.is_some(),
    }
}

/// Las filas, en orden.
pub fn filas(p: &Pegatinas) -> Vec<Fila> {
    let mut v: Vec<Fila> = p.vivas().map(fila).collect();
    v.sort_by(|a, b| {
        a.resuelto
            .cmp(&b.resuelto)
            .then(a.tipo.orden().cmp(&b.tipo.orden()))
            .then(b.cuando.cmp(&a.cuando))
            .then(a.id.cmp(&b.id))
    });
    v
}

/// Si a una pegatina se le ofrece «Crear tarea»: las de urgencia, abiertas
/// y sin tarea aun.
pub fn se_puede_crear_tarea(c: &Comentario) -> bool {
    c.tipo.es_urgencia() && !c.resuelto && !c.borrado && c.tarea.is_none()
}

/// **El texto de la tarea** que sale de una pegatina: su emoticono, lo que
/// dice y de que documento es. `sin_texto` es lo que se pone si aun no
/// tiene nada escrito (el nombre del tipo, traducido).
pub fn texto_de_tarea(c: &Comentario, documento: &str, sin_texto: &str) -> String {
    let r = c.resumen();
    let r = if r.is_empty() {
        sin_texto.to_string()
    } else {
        r
    };
    if documento.trim().is_empty() {
        format!("{} {r}", c.tipo.emoji())
    } else {
        format!("{} {r} \u{2014} {}", c.tipo.emoji(), documento.trim())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::ancla::Ancla;
    use crate::modelo::Quien;

    #[test]
    fn por_urgencia_luego_por_fecha_y_las_resueltas_al_final() {
        let q = Quien {
            autor: "a".into(),
            aparato: "a".into(),
        };
        let mut p = Pegatinas::default();
        let nota = p.poner(Tipo::Nota, Ancla::escena(0.0, 0.0), &q, 10);
        let media_vieja = p.poner(Tipo::Media, Ancla::escena(0.0, 0.0), &q, 20);
        let urgente = p.poner(Tipo::Urgente, Ancla::escena(0.0, 0.0), &q, 30);
        let media_nueva = p.poner(Tipo::Media, Ancla::escena(0.0, 0.0), &q, 40);
        let resuelta = p.poner(Tipo::Urgente, Ancla::escena(0.0, 0.0), &q, 50);
        p.resolver(&resuelta, true, 60);
        let quitada = p.poner(Tipo::Urgente, Ancla::escena(0.0, 0.0), &q, 70);
        p.quitar(&quitada, 80);
        let ids: Vec<String> = filas(&p).into_iter().map(|f| f.id).collect();
        assert_eq!(ids, [urgente, media_nueva, media_vieja, nota, resuelta]);
    }

    #[test]
    fn la_tarea_solo_de_las_urgencias_abiertas() {
        let q = Quien::default();
        let mut p = Pegatinas::default();
        let u = p.poner(Tipo::Alta, Ancla::escena(0.0, 0.0), &q, 1);
        let n = p.poner(Tipo::Nota, Ancla::escena(0.0, 0.0), &q, 2);
        p.escribir(&u, &q, 3, "Cambiar la viga\nmas texto", vec![]);
        let c = p.get(&u).unwrap().clone();
        assert!(se_puede_crear_tarea(&c));
        assert!(!se_puede_crear_tarea(p.get(&n).unwrap()));
        assert_eq!(
            texto_de_tarea(&c, "plano.pdf", "Alta"),
            "\u{1F7E0} Cambiar la viga \u{2014} plano.pdf"
        );
        p.vincular_tarea(&u, "t1", 4);
        assert!(!se_puede_crear_tarea(p.get(&u).unwrap()));
        assert_eq!(
            texto_de_tarea(p.get(&n).unwrap(), "", "Nota"),
            "\u{1F4AC} Nota"
        );
    }
}
