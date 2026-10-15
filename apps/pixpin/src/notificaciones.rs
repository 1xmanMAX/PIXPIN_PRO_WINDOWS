//! **Lo que dicen las notificaciones de Windows de la app** (15-oct-2026):
//! los recordatorios de mensajes y tareas («Hecha», «+10 min», «Abrir») y
//! las capturas que se van a ir («Conservar», «Abrir»). El asistente arma la
//! suya en `automatizaciones::cola`.
//!
//! Cada boton es un **pedido** (`docs/protocolo-pedidos.md`), el mismo que
//! mandaria Flow: Windows lo devuelve a PixPin por el protocolo
//! `pixpinmax:` (ver `pixpin_shell::notificacion`). Aqui solo se arma el
//! toast: es puro y se prueba sin Windows; quien avisa lo saca con
//! `notificacion::mostrar_o` y cae a su aviso de siempre si no sale.

use std::path::{Path, PathBuf};

use pixpin_shell::notificacion::{Boton, Toast};
use pixpin_store::Catalogo;
use serde_json::{Value, json};

/// Lo que se pospone con «+10 min».
pub const POSPONER_MIN: u32 = 10;

fn abrir_mensaje(proyecto: &str, codigo: &str) -> Value {
    json!({ "pixpin": 1, "accion": "abrir", "que": { "tipo": "mensaje", "proyecto": proyecto, "codigo": codigo } })
}

/// **La de un recordatorio que sono.** `proyecto`: la carpeta del chat;
/// `id`: el de la agenda (el del mensaje, o `tarea:<lista>:<hora>`);
/// `tarea`: la lista y el numero de la tarea, si es de una tarea.
pub fn de_recordatorio(t: &Catalogo, proyecto: &str, id: &str, tarea: Option<(&str, usize)>, texto: &str) -> Toast {
    let mut botones = Vec::new();
    let abrir = match tarea {
        Some((lista, indice)) => {
            botones.push(Boton::nuevo(
                t.t("notif-hecha"),
                json!({ "pixpin": 1, "accion": "marcar_tarea", "proyecto": proyecto, "codigo": lista, "indice": indice, "hecha": true }),
            ));
            botones.push(Boton::nuevo(
                t.t("notif-mas-10"),
                json!({ "pixpin": 1, "accion": "posponer", "proyecto": proyecto, "mensaje": lista, "indice": indice, "minutos": POSPONER_MIN }),
            ));
            abrir_mensaje(proyecto, lista)
        }
        None => {
            botones.push(Boton::nuevo(
                t.t("notif-mas-10"),
                json!({ "pixpin": 1, "accion": "posponer", "proyecto": proyecto, "mensaje": id, "minutos": POSPONER_MIN }),
            ));
            abrir_mensaje(proyecto, id)
        }
    };
    botones.push(Boton::nuevo(t.t("notif-abrir"), abrir.clone()));
    Toast {
        titulo: t.t("chat-recordatorio-titulo"),
        texto: texto.to_string(),
        imagen: None,
        al_pulsar: Some(abrir),
        botones,
    }
}

/// El proyecto (la carpeta) de la carpeta de un chat.
pub fn proyecto_de(carpeta: &Path) -> Option<String> {
    carpeta.file_name()?.to_str().map(str::to_string)
}

/// **La de las capturas que se van en las proximas horas.** Una: con su
/// imagen, «Conservar» y «Abrir» (como pin). Varias: cuantas y la galeria.
/// `se_van`: la ruta y cuando se va (ms locales).
pub fn de_capturas(t: &Catalogo, se_van: &[(PathBuf, i64)], ahora_local: i64) -> Option<Toast> {
    let galeria = json!({ "pixpin": 1, "accion": "ventana", "cual": "galeria" });
    match se_van {
        [] => None,
        [(ruta, cuando)] => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("cuando", crate::recordatorios::cuando_legible(*cuando, ahora_local));
            Some(Toast {
                titulo: t.t("notif-capturas-titulo"),
                texto: t.t_args("notif-captura-se-va", &args),
                imagen: Some(ruta.clone()),
                al_pulsar: Some(galeria),
                botones: vec![
                    Boton::nuevo(t.t("notif-conservar"), json!({ "pixpin": 1, "accion": "conservar_captura", "ruta": ruta })),
                    Boton::nuevo(t.t("notif-abrir"), json!({ "pixpin": 1, "accion": "pinear", "ruta": ruta })),
                ],
            })
        }
        varias => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("n", varias.len());
            Some(Toast {
                titulo: t.t("notif-capturas-titulo"),
                texto: t.t_args("notif-capturas-se-van", &args),
                imagen: None,
                al_pulsar: Some(galeria.clone()),
                botones: vec![Boton::nuevo(t.t("notif-ver-galeria"), galeria)],
            })
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_shell::notificacion::{permitido, xml};
    use pixpin_store::Idioma;

    fn t() -> Catalogo {
        Catalogo::nuevo(Idioma::Espanol)
    }

    /// Cada boton tiene que poder volver por el protocolo, y el pedido tiene
    /// que entenderlo la app.
    fn botones_validos(toast: &Toast) {
        for b in &toast.botones {
            assert!(permitido(&b.pedido), "{}", b.pedido);
            assert!(crate::pedidos::leer(&b.pedido.to_string()).is_ok(), "{}", b.pedido);
        }
    }

    #[test]
    fn la_de_una_tarea_tiene_hecha_mas_10_y_abrir() {
        let toast = de_recordatorio(&t(), "pr-1", "tarea:L1:123", Some(("L1", 2)), "☑ Llamar al banco");
        let rotulos: Vec<_> = toast.botones.iter().map(|b| b.rotulo.as_str()).collect();
        assert_eq!(rotulos, ["Hecha", "+10 min", "Abrir"]);
        assert_eq!(toast.titulo, "Recordatorio");
        assert_eq!(toast.botones[0].pedido["accion"], "marcar_tarea");
        assert_eq!(toast.botones[0].pedido["indice"], 2);
        assert_eq!(toast.botones[1].pedido["mensaje"], "L1");
        assert_eq!(toast.botones[2].pedido["que"]["codigo"], "L1");
        botones_validos(&toast);
        assert!(xml(&toast).contains("☑ Llamar al banco"));
    }

    #[test]
    fn la_de_un_mensaje_no_tiene_hecha() {
        let toast = de_recordatorio(&t(), "pr-1", "m7", None, "comprar yeso");
        let rotulos: Vec<_> = toast.botones.iter().map(|b| b.rotulo.as_str()).collect();
        assert_eq!(rotulos, ["+10 min", "Abrir"]);
        assert_eq!(toast.botones[0].pedido["mensaje"], "m7");
        assert!(toast.botones[0].pedido.get("indice").is_none());
        botones_validos(&toast);
    }

    #[test]
    fn la_de_las_capturas() {
        let ahora = 1_000_000_000_000;
        assert!(de_capturas(&t(), &[], ahora).is_none(), "sin capturas, nada");
        let una = de_capturas(&t(), &[(PathBuf::from(r"C:\c\captura-0001.png"), ahora + 3_600_000)], ahora).unwrap();
        let rotulos: Vec<_> = una.botones.iter().map(|b| b.rotulo.as_str()).collect();
        assert_eq!(rotulos, ["Conservar", "Abrir"]);
        assert!(una.texto.starts_with("Esta captura se va"), "{}", una.texto);
        assert!(una.imagen.is_some());
        botones_validos(&una);
        let varias = de_capturas(
            &t(),
            &[(PathBuf::from("a.png"), ahora), (PathBuf::from("b.png"), ahora), (PathBuf::from("c.png"), ahora)],
            ahora,
        )
        .unwrap();
        assert!(varias.texto.contains('3'), "{}", varias.texto);
        assert_eq!(varias.botones.len(), 1);
        botones_validos(&varias);
    }

    #[test]
    fn el_proyecto_es_la_carpeta() {
        assert_eq!(proyecto_de(Path::new(r"C:\d\proyectos\pr-1")).as_deref(), Some("pr-1"));
        assert_eq!(proyecto_de(Path::new("")), None);
    }
}
