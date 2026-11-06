//! **Adonde va lo apuntado en la caja de Tareas** (1-nov-2026): lo puro del
//! desplegable del libro de la caja.
//!
//! El usuario: «en las listas de tareas quiero ver tambien mis asignaturas y
//! poder añadirles tareas; ahora no puedo». Antes el libro solo ofrecia las
//! asignaturas y, sin ninguna, decia que no habia; las demas listas solo se
//! alcanzaban abriendo la ventana desde el chat. Ahora el desplegable es el
//! de **todas** las listas: el Inbox (lo de siempre, sin elegir nada), las
//! asignaturas del horario (con su lista, aunque aun no exista: se crea al
//! apuntar) y detras las demas listas de todos los chats. Y cada lista tiene
//! ademas su «＋» en el encabezado, que la deja elegida.

#![forbid(unsafe_code)]

use pindesk_ia::acciones::normalizar;

use super::Lista;

/// Lo que se puede elegir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destino {
    /// El Inbox de «Mensajes guardados» (sin elegir nada).
    Inbox,
    /// La lista de la asignatura de ese nombre.
    Asignatura(String),
    /// Una lista cualquiera, por su clave (`Lista::clave`).
    Lista(String),
}

/// **Las entradas del desplegable**, en orden, con su rotulo: el Inbox
/// (`inbox`, ya traducido), cada asignatura de `asignaturas` (con su libro
/// delante) y despues las demas listas de `listas` que no son ni el Inbox ni
/// la de una asignatura, cada una con su chat. `elegido` lleva la marca ✓.
pub fn opciones(
    inbox: &str,
    asignaturas: &[String],
    listas: &[Lista],
    elegido: &Destino,
) -> Vec<(Destino, String)> {
    let marca = |d: &Destino| if d == elegido { "✓ " } else { "" };
    let mut v = vec![(Destino::Inbox, format!("{}{inbox}", marca(&Destino::Inbox)))];
    for n in asignaturas {
        let d = Destino::Asignatura(n.clone());
        v.push((d.clone(), format!("{}📘 {n}", marca(&d))));
    }
    let de_asignatura = |l: &Lista| {
        l.asignatura.is_some()
            || asignaturas
                .iter()
                .any(|n| l.chat == crate::horario::asignaturas::NOMBRE_CHAT && mismo(n, &l.titulo))
    };
    for l in listas
        .iter()
        .filter(|l| !super::es_inbox(l) && !de_asignatura(l))
    {
        let d = Destino::Lista(l.clave());
        let rotulo = format!("{}{} · {}", marca(&d), l.titulo, l.chat);
        v.push((d, rotulo));
    }
    v
}

fn mismo(a: &str, b: &str) -> bool {
    normalizar(a.trim()) == normalizar(b.trim())
}

/// Lo que esta elegido ahora en la caja: la asignatura si hay una, si no la
/// lista, y si no el Inbox.
pub fn elegido(asignatura: Option<&str>, lista: Option<&Lista>) -> Destino {
    match (asignatura, lista) {
        (Some(a), _) => Destino::Asignatura(a.to_string()),
        (None, Some(l)) => Destino::Lista(l.clave()),
        (None, None) => Destino::Inbox,
    }
}

/// **El «＋» del encabezado de la lista `l`**: lo que queda elegido en la
/// caja. La de una asignatura, como su asignatura (asi lo apuntado sigue a
/// su enlace aunque la lista se renombre); las demas, como esa lista.
pub fn de_la_lista(l: &Lista, asignaturas: &[String]) -> Destino {
    if l.chat == crate::horario::asignaturas::NOMBRE_CHAT
        && let Some(n) = asignaturas.iter().find(|n| mismo(n, &l.titulo))
    {
        return Destino::Asignatura(n.clone());
    }
    if super::es_inbox(l) {
        return Destino::Inbox;
    }
    Destino::Lista(l.clave())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::tareas::filas_de;

    fn lista(chat: &str, titulo: &str, guardados: bool) -> Lista {
        Lista {
            proyecto: chat.to_lowercase(),
            chat: chat.into(),
            guardados,
            codigo: format!("m-{}", titulo.to_lowercase()),
            titulo: titulo.into(),
            cuando: 1,
            filas: filas_de(""),
            aparato: None,
            asignatura: None,
            uid: None,
        }
    }

    fn muestra() -> Vec<Lista> {
        let mut geo = lista("Asignaturas", "Geometría", false);
        geo.asignatura = Some(0x0A84FF);
        vec![
            lista("Mensajes guardados", "Inbox", true),
            geo,
            lista("Asignaturas", "Física", false),
            lista("Casa", "Compra", false),
        ]
    }

    #[test]
    fn todas_las_listas_se_pueden_elegir_y_las_asignaturas_primero() {
        let ls = muestra();
        let asig = vec![
            "Geometría".to_string(),
            "Física".to_string(),
            "Arte".to_string(),
        ];
        let v = opciones("Inbox", &asig, &ls, &Destino::Inbox);
        let destinos: Vec<&Destino> = v.iter().map(|(d, _)| d).collect();
        assert_eq!(
            destinos,
            [
                &Destino::Inbox,
                &Destino::Asignatura("Geometría".into()),
                &Destino::Asignatura("Física".into()),
                // Sin lista aun: se crea al apuntar.
                &Destino::Asignatura("Arte".into()),
                &Destino::Lista("casa/m-compra".into()),
            ]
        );
        assert_eq!(v[0].1, "✓ Inbox");
        assert_eq!(v[4].1, "Compra · Casa");
        // La elegida lleva la marca.
        let v = opciones("Inbox", &asig, &ls, &Destino::Lista("casa/m-compra".into()));
        assert_eq!(v[0].1, "Inbox");
        assert_eq!(v[4].1, "✓ Compra · Casa");
    }

    #[test]
    fn el_mas_de_una_lista_elige_su_asignatura_o_esa_lista() {
        let ls = muestra();
        let asig = vec!["geometria".to_string(), "Física".to_string()];
        assert_eq!(
            de_la_lista(&ls[1], &asig),
            Destino::Asignatura("geometria".into())
        );
        assert_eq!(de_la_lista(&ls[0], &asig), Destino::Inbox);
        assert_eq!(
            de_la_lista(&ls[3], &asig),
            Destino::Lista("casa/m-compra".into())
        );
        // Caso negativo: una lista del chat «Asignaturas» que ya no es de
        // ninguna asignatura es una lista mas.
        assert_eq!(
            de_la_lista(&ls[2], &["Geometría".to_string()]),
            Destino::Lista("asignaturas/m-física".into())
        );
        assert_eq!(elegido(None, None), Destino::Inbox);
        assert_eq!(
            elegido(Some("Física"), Some(&ls[3])),
            Destino::Asignatura("Física".into())
        );
    }
}
