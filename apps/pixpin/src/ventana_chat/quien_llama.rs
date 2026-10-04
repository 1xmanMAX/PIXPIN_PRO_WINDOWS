//! **«Quien llama»** (B11, v0.98.6 del movil, pedido por el usuario): al
//! programar la llamada secreta de una nota de voz se elige tambien el
//! nombre que saldra en la pantalla de la llamada; por defecto, el de la
//! nota.
//!
//! En el movil es un campo de texto dentro del dialogo de la hora. Aqui las
//! horas son un menu, y un menu no tiene donde teclear: la primera linea del
//! menu de una nota de voz dice «Quien llama: Recado» y abre una barra
//! encima de la caja de escribir (la misma que «Elegir la hora…»); Intro
//! guarda el nombre y vuelve a sacar las horas, Escape la deja.
//!
//! El nombre se guarda como en el movil, fuera del mensaje y en el aparato
//! (`llamada::poner_quien_llama`), y se apunta al poner la hora aunque no se
//! haya tocado: `ponerQuienLlama(quien)` al elegir.

use std::path::Path;

use pixpin_geom::Rect;
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_render::{Pintor, RectF};
use pixpin_store::Catalogo;

use super::{Accion, EntradaMenu, Tema, entrada, icono_centrado, mi};

/// Lo mas largo que se deja teclear: un nombre, no una frase.
pub(super) const MAXIMO: usize = 40;

/// Si poner hora a `m` programa una llamada secreta: una nota de voz
/// (`cual.clase == Clase.VOZ`).
pub(super) fn es_llamada(m: &Mensaje) -> bool {
    m.clase == Some(Clase::Voz)
}

/// El nombre que se ensena ahora: el apuntado o el de la nota.
pub(super) fn nombre_actual(carpeta: &Path, m: &Mensaje) -> String {
    crate::llamada::quien_llama_en(carpeta, m)
}

/// La primera linea del menu de las horas de una nota de voz; `None` en lo
/// demas.
pub(super) fn entrada_del_menu(
    m: &Mensaje,
    i: usize,
    carpeta: &Path,
    textos: &Catalogo,
) -> Option<EntradaMenu> {
    if !es_llamada(m) {
        return None;
    }
    let mut nombre = nombre_actual(carpeta, m);
    if nombre.trim().is_empty() {
        nombre = textos.t("llamada-titulo");
    }
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("nombre", nombre);
    Some(entrada(
        Some(&mi::PERSON),
        textos.t_args("chat-quien-llama", &args),
        Accion::QuienLlama(i),
    ))
}

/// El menu de las horas de `m` (el `i` del chat): el de siempre y, si es una
/// nota de voz, con «Quien llama: …» delante.
pub(super) fn menu_de_las_horas(
    m: Option<&Mensaje>,
    i: usize,
    carpeta: &Path,
    textos: &Catalogo,
) -> Vec<EntradaMenu> {
    let mut v = super::menu_de_recordatorio(i, textos);
    if let Some(q) = m.and_then(|m| entrada_del_menu(m, i, carpeta, textos)) {
        v.insert(0, q);
    }
    v
}

/// **Intro en la barra**: guarda el nombre, la cierra y devuelve el menu de
/// las horas colgado de donde estaba la barra, para seguir eligiendo la hora
/// como en el dialogo del movil. `None` si la nota ya no esta (la borro una
/// sincronizacion): la barra se cierra sin mas.
pub(super) fn al_pulsar_intro(
    a: &mut super::Abierto,
    raiz: &Path,
    textos: &Catalogo,
) -> Option<super::MenuAbierto> {
    let (id, escrito) = a.quien_llama.take()?;
    let i = a.mensajes.iter().position(|m| m.id == id)?;
    if let Err(e) = crate::llamada::poner_quien_llama(raiz, &id, &escrito) {
        tracing::warn!(?e, "no se pudo apuntar quien llama");
    }
    // El menu cuelga del aspa de la barra, que es lo ultimo que se pinto.
    let ancla = a
        .zonas
        .borrow()
        .iter()
        .find(|(_, z)| *z == super::Zona::CerrarQuien)
        .map(|(r, _)| pixpin_geom::Punto {
            x: r.derecha(),
            y: r.y,
        })?;
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &a.ficha.id);
    Some(super::MenuAbierto::nuevo(
        ancla,
        menu_de_las_horas(a.mensajes.get(i), i, &carpeta, textos),
    ))
}

/// Al ponerle la hora a una nota de voz, se apunta quien llama (el que se
/// ve, aunque no se haya tocado), como el movil al elegir.
pub(super) fn al_poner_la_hora(raiz: &Path, carpeta: &Path, m: &Mensaje) {
    if !es_llamada(m) {
        return;
    }
    let quien = nombre_actual(carpeta, m);
    if let Err(e) = crate::llamada::poner_quien_llama(raiz, &m.id, &quien) {
        tracing::warn!(?e, "no se pudo apuntar quien llama");
    }
}

/// Una letra tecleada en la barra: cualquiera que se escriba, sin pasar de
/// [`MAXIMO`].
pub(super) fn teclear(escrito: &mut String, c: char) {
    if !c.is_control() && escrito.chars().count() < MAXIMO {
        escrito.push(c);
    }
}

/// La barra de teclear quien llama, en `barra` (encima de la isla). Devuelve
/// el aspa de cerrarla, para su zona.
pub(super) fn pintar_barra(
    p: &Pintor,
    barra: Rect,
    tema: &Tema,
    textos: &Catalogo,
    escrito: &str,
    e: f32,
) -> Rect {
    let caja = RectF {
        x: barra.x as f32,
        y: barra.y as f32,
        ancho: barra.ancho as f32,
        alto: (barra.alto as f32 - 6.0 * e).max(1.0),
    };
    p.rellenar_redondeado(caja, 16.0 * e, tema.campo);
    let icono = Rect {
        x: (caja.x + 8.0 * e) as i32,
        y: caja.y as i32,
        ancho: (26.0 * e) as u32,
        alto: caja.alto as u32,
    };
    icono_centrado(p, &mi::PERSON, icono, 18.0 * e, tema.enviar);
    let cerrar = Rect {
        x: (caja.x + caja.ancho - caja.alto) as i32,
        y: caja.y as i32,
        ancho: caja.alto as u32,
        alto: caja.alto as u32,
    };
    icono_centrado(p, &mi::CLOSE, cerrar, 20.0 * e, tema.campo_apagado);
    let tam = 12.0 * e;
    let pista = textos.t("chat-quien-llama-teclas");
    let (w_pista, alto_r) = p.medir_texto(&pista, tam);
    let y = caja.y + (caja.alto - alto_r) / 2.0;
    let x_pista = (cerrar.x as f32 - w_pista - 6.0 * e).max(caja.x);
    p.texto(&pista, x_pista, y, tam, tema.campo_apagado);
    let rotulo = format!("{} {escrito}|", textos.t("chat-quien-llama-barra"));
    let x = caja.x + 40.0 * e;
    p.texto_linea(
        &rotulo,
        x,
        y,
        13.0 * e,
        (x_pista - x - 8.0 * e).max(0.0),
        tema.texto,
    );
    cerrar
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn raiz(etiqueta: &str) -> std::path::PathBuf {
        let r = std::env::temp_dir().join(format!(
            "pixpin-quien-llama-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(r.join("proyectos").join("p1")).unwrap();
        r
    }

    fn mensaje(clase: Clase, nombre: &str) -> Mensaje {
        Mensaje {
            id: "m1".into(),
            clase: Some(clase),
            nombre: nombre.into(),
            ..Mensaje::default()
        }
    }

    #[test]
    fn el_menu_de_una_nota_de_voz_empieza_por_quien_llama_y_el_de_lo_demas_no() {
        let r = raiz("menu");
        let carpeta = r.join("proyectos").join("p1");
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let e = entrada_del_menu(&mensaje(Clase::Voz, "recado.m4a"), 4, &carpeta, &textos)
            .expect("una nota de voz es una llamada");
        assert!(e.texto.contains("recado"), "{}", e.texto);
        assert!(matches!(e.accion, Accion::QuienLlama(4)));
        // Casos negativos: una nota escrita o un archivo no llaman.
        assert!(entrada_del_menu(&mensaje(Clase::Nota, "x"), 4, &carpeta, &textos).is_none());
        assert!(
            entrada_del_menu(&mensaje(Clase::Archivo, "a.pdf"), 4, &carpeta, &textos).is_none()
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn poner_la_hora_a_una_nota_de_voz_apunta_quien_llama_como_el_movil() {
        let r = raiz("hora");
        let carpeta = r.join("proyectos").join("p1");
        let voz = mensaje(Clase::Voz, "recado.m4a");
        al_poner_la_hora(&r, &carpeta, &voz);
        assert_eq!(
            crate::llamada::quien_llama(&r, "m1").as_deref(),
            Some("recado")
        );
        // Lo ya puesto se respeta: no se pisa con el nombre de la nota.
        crate::llamada::poner_quien_llama(&r, "m1", "Mama").unwrap();
        al_poner_la_hora(&r, &carpeta, &voz);
        assert_eq!(nombre_actual(&carpeta, &voz), "Mama");
        // Caso negativo: a una nota escrita no se le apunta nada.
        let nota = Mensaje {
            id: "m2".into(),
            ..mensaje(Clase::Nota, "hola")
        };
        al_poner_la_hora(&r, &carpeta, &nota);
        assert_eq!(crate::llamada::quien_llama(&r, "m2"), None);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn en_la_barra_se_teclea_un_nombre_y_no_mas_de_cuarenta_letras() {
        let mut s = String::new();
        for c in "Mama Rosa".chars() {
            teclear(&mut s, c);
        }
        assert_eq!(s, "Mama Rosa");
        // Casos negativos: un salto o un tabulador no son letras.
        teclear(&mut s, '\r');
        teclear(&mut s, '\t');
        assert_eq!(s, "Mama Rosa");
        for _ in 0..100 {
            teclear(&mut s, 'a');
        }
        assert_eq!(s.chars().count(), MAXIMO);
    }
}
