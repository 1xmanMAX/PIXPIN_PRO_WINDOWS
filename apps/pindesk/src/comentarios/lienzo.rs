//! **Lo del lienzo**: las anclas de escena, el espejo Excalidraw de cada
//! pegatina (`pindesk_comentarios::espejo`) y abrir en el lienzo una imagen
//! de una entrada, o su lienzo propio, para anotar.

use std::path::{Path, PathBuf};

use pindesk_comentarios::{Ancla, Pegatinas, Sitio as SitioAncla, espejo};
use pindesk_motor2d::camara::Camara;
use pindesk_motor2d::excalidraw::{self, Entrada, Lienzo};
use pindesk_motor2d::vector::Punto2;
use serde_json::Value;

use super::{Pegador, Ubicar};

/// Las anclas de un lienzo: coordenadas del mundo con la camara que pinta
/// (la que ya lleva la escala de Windows).
pub(crate) struct UbicarLienzo<'a> {
    pub camara: &'a Camara,
}

impl Ubicar for UbicarLienzo<'_> {
    fn a_pantalla(&self, a: &Ancla) -> Option<(f32, f32)> {
        let SitioAncla::Escena { x, y } = a.sitio() else {
            return None;
        };
        let s = self.camara.a_pantalla(Punto2::nuevo(x as f32, y as f32));
        Some((s.x, s.y))
    }

    fn de_pantalla(&self, p: (f32, f32)) -> Option<Ancla> {
        let w = self.camara.a_mundo(Punto2::nuevo(p.0, p.1));
        Some(Ancla::escena(w.x as f64, w.y as f64))
    }
}

/// Un visor que no ubica nada (para pintar solo la interfaz).
pub(crate) struct SinUbicar;

impl Ubicar for SinUbicar {
    fn a_pantalla(&self, _: &Ancla) -> Option<(f32, f32)> {
        None
    }
    fn de_pantalla(&self, _: (f32, f32)) -> Option<Ancla> {
        None
    }
}

fn espejos_de(l: &Lienzo) -> Vec<Value> {
    l.entradas
        .iter()
        .filter_map(|e| match e {
            Entrada::Ajeno(v) if espejo::es_espejo(v) => Some((**v).clone()),
            _ => None,
        })
        .collect()
}

/// **Los espejos al dia** en un lienzo leido: los que ya estaban, en su
/// sitio del orden; los nuevos, al final. Devuelve si cambio algo.
pub(crate) fn poner_espejos(l: &mut Lienzo, p: &Pegatinas) -> bool {
    let mut espejos = espejos_de(l);
    if !espejo::poner(&mut espejos, p) {
        return false;
    }
    for e in &mut l.entradas {
        if let Entrada::Ajeno(v) = e
            && espejo::es_espejo(v)
            && let Some(n) = espejos.iter().find(|n| n.get("id") == v.get("id"))
        {
            **v = n.clone();
        }
    }
    for n in espejos {
        let ya = l
            .entradas
            .iter()
            .any(|e| matches!(e, Entrada::Ajeno(v) if v.get("id") == n.get("id")));
        if !ya {
            l.entradas.push(Entrada::Ajeno(Box::new(n)));
        }
    }
    true
}

/// **El lienzo con los espejos de sus pegatinas**, para guardarlo
/// (`ventana_chat::guardar_hoja_dibujada`). Sin pegatinas, tal cual.
pub(crate) fn con_espejos(dibujo: &Path, l: &Lienzo) -> Lienzo {
    let sitio = super::Sitio::del_lienzo(dibujo);
    let Ok(texto) = std::fs::read_to_string(&sitio.fichero) else {
        return l.clone();
    };
    let Ok(p) = pindesk_comentarios::fichero::leer(&texto) else {
        return l.clone();
    };
    let mut salida = l.clone();
    poner_espejos(&mut salida, &p);
    salida
}

/// **Las pegatinas de un lienzo**, con lo que hizo un movil con sus
/// espejos (moverlos, borrarlos) ya aplicado.
pub(crate) fn pegador_del_lienzo(dibujo: &Path) -> Pegador {
    let mut p = Pegador::del_lienzo(dibujo);
    let espejos = std::fs::read_to_string(dibujo)
        .ok()
        .and_then(|t| excalidraw::leer(&t).ok())
        .map(|l| espejo::leer(&espejos_de(&l)))
        .unwrap_or_default();
    if !espejos.is_empty() {
        let q = p.quien.clone();
        if espejo::aplicar(&mut p.pegatinas, &espejos, &q) {
            p.guardar();
        }
    }
    p
}

/// **Abre en el lienzo** lo de una entrada: `imagen` de fondo (si es una
/// imagen) y la tinta de encima en `tinta` (el lienzo propio, sin fondo).
/// En su hilo, como los lienzos del chat: el visor que lo pide sigue vivo.
/// Al cerrar se guarda la tinta (JSON Excalidraw, el de los lienzos del
/// chat, que viaja como `.excalidraw.gz`).
pub(crate) fn abrir_tinta(imagen: Option<PathBuf>, tinta: PathBuf) {
    let lanzado = std::thread::Builder::new()
        .name("lienzo-del-comentario".into())
        .spawn(move || {
            let _com = pindesk_shell::ComDelHilo::iniciar();
            let opciones = crate::notas_md::paginas_vivas::opciones_del_lienzo();
            let fondo = imagen.as_deref().and_then(|i| match pindesk_codec::cargar(i) {
                Ok(img) => Some(img),
                Err(e) => {
                    tracing::warn!(?e, ruta = %i.display(), "imagen del comentario que no se pudo leer");
                    None
                }
            });
            let lienzo = match std::fs::read_to_string(&tinta) {
                Ok(t) => match excalidraw::leer(&t) {
                    Ok(l) => l,
                    Err(e) => {
                        // Abrir en blanco y guardar al cerrar pisaria lo que hubiera.
                        tracing::error!(?e, ruta = %tinta.display(), "lienzo del comentario roto; no se abre");
                        return;
                    }
                },
                Err(_) => Lienzo::vacio(),
            };
            let escena = excalidraw::a_escena(&lienzo);
            if let Some(p) = tinta.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            // Sus marcas y sus pegatinas, junto a el (se puede comentar
            // tambien sobre lo anotado).
            let _marcas = crate::ventana_editor::marcas::junto_a(&tinta);
            let mut pegadas = Vec::new();
            let resultado = crate::ventana_editor::abrir_con_pegadas(
                escena,
                opciones.enganche,
                opciones.nivel,
                opciones.medir_fotogramas,
                fondo,
                &[],
                &mut pegadas,
            );
            match resultado {
                Ok((escena, _)) => {
                    // Un lienzo nuevo sin nada no se crea: no hay que guardar.
                    if tinta.is_file() || escena.cuantos_visibles() > 0 {
                        crate::ventana_chat::guardar_hoja_dibujada(&tinta, &lienzo, &escena);
                    }
                    crate::ventana_chat::refrescar();
                }
                Err(e) => tracing::warn!(?e, "no se pudo abrir el lienzo del comentario"),
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el lienzo del comentario");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pindesk_comentarios::{Quien, Tipo};

    #[test]
    fn los_espejos_se_ponen_y_se_actualizan_en_su_sitio() {
        let mut l = excalidraw::leer(
            r#"{"type":"excalidraw","elements":[{"id":"r","type":"rectangle","x":0,"y":0,"width":5,"height":5}]}"#,
        )
        .unwrap();
        let mut p = Pegatinas::default();
        let id = p.poner(
            Tipo::Urgente,
            Ancla::escena(50.0, 50.0),
            &Quien::default(),
            1000,
        );
        assert!(poner_espejos(&mut l, &p));
        assert_eq!(l.entradas.len(), 2);
        // Se escribe como elemento y se vuelve a leer como ajeno.
        let t = excalidraw::escribir(&l);
        let l2 = excalidraw::leer(&t).unwrap();
        assert_eq!(
            l2.elementos().len(),
            1,
            "el espejo no es un elemento del PC"
        );
        assert_eq!(espejos_de(&l2).len(), 1);
        // Moverla lo cambia en su sitio, sin duplicar.
        p.mover(&id, Ancla::escena(10.0, 10.0), 2000);
        let mut l3 = l2.clone();
        assert!(poner_espejos(&mut l3, &p));
        assert_eq!(l3.entradas.len(), 2);
        assert_eq!(espejo::leer(&espejos_de(&l3))[0].x, 10.0);
        // Sin cambios, nada.
        assert!(!poner_espejos(&mut l3, &p));
    }

    #[test]
    fn la_camara_del_lienzo_va_y_vuelve() {
        let c = Camara {
            x: 100.0,
            y: 50.0,
            zoom: 2.0,
        };
        let u = UbicarLienzo { camara: &c };
        let a = Ancla::escena(110.0, 60.0);
        assert_eq!(u.a_pantalla(&a), Some((20.0, 20.0)));
        assert_eq!(u.de_pantalla((20.0, 20.0)), Some(a));
        assert_eq!(u.a_pantalla(&Ancla::pagina(0, 1.0, 1.0)), None);
    }
}
