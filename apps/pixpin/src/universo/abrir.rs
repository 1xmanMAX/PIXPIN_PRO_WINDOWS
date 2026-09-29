//! Ctrl+clic: que abre cada astro (D224), y abrirlo.
//!
//! `que_abre` es pura para poder probar la tabla entera sin tocar el
//! Explorador; `ejecutar` es la que habla con Windows.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pixpin_universo::ficha::{ClaseLuna, FichaLuna};
use pixpin_universo::{Clase, IdAstro, Universo};

/// Cuantos ficheros abre como mucho Ctrl+Mayus+clic en un planeta. Mas que
/// esto es llenar la pantalla de ventanas por un despiste.
pub const TOPE_ABRIR_VARIOS: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Motivo {
    /// El fichero es del movil o se borro (D219).
    NoEstaEnEquipo,
    /// Un planeta sin lunas con fichero.
    SinArchivos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Apertura {
    Fichero(PathBuf),
    /// Un dibujo o una pagina: se abre en el lienzo, no en otro programa.
    Hoja {
        proyecto: String,
        referencia: String,
    },
    /// La conversacion del proyecto, y el mensaje si hay uno.
    Chat {
        proyecto: String,
        codigo: Option<String>,
    },
    Carpeta(PathBuf),
    Varios(Vec<PathBuf>),
    Nada(Motivo),
}

fn fichero_de(raiz: &Path, f: &FichaLuna) -> Option<PathBuf> {
    if !f.en_equipo {
        return None;
    }
    let r = f.ruta.as_deref().filter(|r| !r.is_empty())?;
    Some(pixpin_proyecto::almacen::carpeta(raiz, &f.proyecto).join(r))
}

/// La tabla D224. `todas` es Ctrl+Mayus: en un planeta, abrir sus ficheros
/// en vez de su carpeta.
pub fn que_abre(
    u: &Universo,
    id: IdAstro,
    fichas: &HashMap<String, FichaLuna>,
    raiz: &Path,
    todas: bool,
) -> Apertura {
    let Some(a) = u.astro(id) else {
        return Apertura::Nada(Motivo::SinArchivos);
    };
    match &a.clase {
        Clase::Galaxia { proyecto } => Apertura::Chat {
            proyecto: proyecto.clone(),
            codigo: None,
        },
        Clase::Luna { codigo, proyecto } => {
            let Some(f) = fichas.get(codigo) else {
                // Sin ficha todavia no se sabe que es: al chat, que si lo sabe.
                return Apertura::Chat {
                    proyecto: proyecto.clone(),
                    codigo: Some(codigo.clone()),
                };
            };
            que_abre_ficha(f, raiz)
        }
        Clase::Planeta => {
            let lunas: Vec<&FichaLuna> = u
                .hijos(id)
                .filter_map(|l| l.codigo().and_then(|c| fichas.get(c)))
                .collect();
            if todas {
                let v: Vec<PathBuf> = lunas
                    .iter()
                    .filter_map(|f| fichero_de(raiz, f))
                    .take(TOPE_ABRIR_VARIOS)
                    .collect();
                return if v.is_empty() {
                    Apertura::Nada(Motivo::SinArchivos)
                } else {
                    Apertura::Varios(v)
                };
            }
            // En una galaxia, la carpeta es la de su proyecto; un
            // exoplaneta no tiene proyecto, y se toma el de su luna mas
            // reciente, que es lo que el usuario acaba de meter ahi.
            let proyecto = a
                .padre
                .and_then(|g| u.astro(g))
                .and_then(|g| g.proyecto().map(str::to_string))
                .or_else(|| {
                    lunas
                        .iter()
                        .max_by_key(|f| f.cuando)
                        .map(|f| f.proyecto.clone())
                });
            match proyecto {
                Some(p) => Apertura::Carpeta(pixpin_proyecto::almacen::carpeta(raiz, &p)),
                None => Apertura::Nada(Motivo::SinArchivos),
            }
        }
    }
}

/// Lo que abre un archivo, este en el cielo o todavia en la nebulosa: la
/// misma regla para los dos, porque es el mismo archivo.
pub fn que_abre_ficha(f: &FichaLuna, raiz: &Path) -> Apertura {
    match f.clase {
        ClaseLuna::Nota | ClaseLuna::MiniApp => Apertura::Chat {
            proyecto: f.proyecto.clone(),
            codigo: Some(f.codigo.clone()),
        },
        ClaseLuna::Dibujo | ClaseLuna::Pagina if f.referencia.is_some() => Apertura::Hoja {
            proyecto: f.proyecto.clone(),
            referencia: f.referencia.clone().unwrap_or_default(),
        },
        // Una hoja-pagina del documento sin dibujo todavia: tambien al
        // lienzo, con la pagina de fondo, como en el movil. Va por su codigo
        // (no tiene otra sena); `abrir_hoja` la encuentra y el chat le
        // estrena el dibujo.
        ClaseLuna::Pagina if f.ruta.as_deref().is_none_or(str::is_empty) => Apertura::Hoja {
            proyecto: f.proyecto.clone(),
            referencia: f.codigo.clone(),
        },
        _ => match fichero_de(raiz, f) {
            Some(r) => Apertura::Fichero(r),
            None => Apertura::Nada(Motivo::NoEstaEnEquipo),
        },
    }
}

/// Lo que hace falta para abrir el chat desde el universo, que vive en
/// otro hilo y puede estar cerrado.
#[derive(Clone)]
pub struct AlChat {
    pub idioma: pixpin_store::Idioma,
    pub ubicacion: pixpin_store::Ubicacion,
    pub opciones: crate::ventana_chat::OpcionesLienzo,
}

/// Lo que se hace con cada apertura. `Hoja` no: la abre quien llama al
/// editor (ver `Sesion::hoja_pedida`), porque hay que cerrar el universo
/// antes. `Chat` necesita `chat`; sin el (en las pruebas) no hace nada.
pub fn ejecutar(a: &Apertura, chat: Option<&AlChat>) {
    let abrir = |r: &Path| {
        if let Err(e) = pixpin_shell::abrir(r) {
            tracing::warn!(?e, ruta = %r.display(), "no se pudo abrir desde el universo");
        }
    };
    match a {
        Apertura::Fichero(r) | Apertura::Carpeta(r) => abrir(r),
        Apertura::Varios(v) => v.iter().for_each(|r| abrir(r)),
        Apertura::Chat { proyecto, codigo } => match chat {
            Some(c) => crate::ventana_chat::ir_a(
                c.idioma,
                c.ubicacion.clone(),
                c.opciones,
                proyecto.clone(),
                codigo.clone(),
            ),
            None => tracing::warn!(%proyecto, "ir al chat sin saber como abrirlo"),
        },
        Apertura::Hoja { .. } | Apertura::Nada(_) => {}
    }
}

/// Abre una hoja de un proyecto en el lienzo y la guarda al cerrar,
/// siguiendo los enlaces a otras hojas: lo mismo que pulsarla en el chat,
/// con la misma funcion (`ventana_chat::abrir_hojas`). Los mensajes se leen
/// aqui porque el universo no tiene el proyecto abierto como el chat.
pub fn abrir_hoja(
    raiz: &Path,
    proyecto: &str,
    referencia: &str,
    opciones: crate::ventana_chat::OpcionesLienzo,
) {
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, proyecto);
    let mensajes = match pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta) {
        Ok(c) => c.mensajes,
        Err(e) => {
            tracing::warn!(?e, %proyecto, "no se pudo leer el cuaderno de la hoja");
            return;
        }
    };
    match indice_de_referencia(&mensajes, referencia) {
        Some(i) => {
            crate::ventana_chat::abrir_hojas(raiz, proyecto, &mensajes, i, opciones);
        }
        // Una hoja que no esta en el chat (un sublienzo que solo esta en
        // `proyecto.json`, uno con el nombre de antes de importarse) se busca
        // como la busca una zona vinculada.
        None => match crate::salto_por_enlace::hoja_del_enlace(raiz, proyecto, &mensajes, referencia) {
            Some(crate::salto_por_enlace::HojaDelEnlace::EnLaLista(i)) => {
                crate::ventana_chat::abrir_hojas(raiz, proyecto, &mensajes, i, opciones);
            }
            Some(crate::salto_por_enlace::HojaDelEnlace::Leida(m)) => {
                let mut todos = mensajes;
                todos.push(m);
                let i = todos.len() - 1;
                crate::ventana_chat::abrir_hojas(raiz, proyecto, &todos, i, opciones);
            }
            None => tracing::warn!(%referencia, %proyecto, "la hoja no esta en el chat"),
        },
    }
}

/// El mensaje que lleva esa hoja, que es por donde el chat la abre.
fn indice_de_referencia(
    mensajes: &[pixpin_proyecto::cuaderno::Mensaje],
    referencia: &str,
) -> Option<usize> {
    mensajes
        .iter()
        .position(|m| m.referencia.as_deref() == Some(referencia))
        // Una pagina sin dibujo se pide por su codigo unico.
        .or_else(|| {
            mensajes.iter().position(|m| {
                crate::pdf_en_chat::es_pagina_sin_dibujo(m) && m.codigo_unico() == referencia
            })
        })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_universo::ficha::{ClaseLuna, FichaLuna};
    use pixpin_universo::{Astro, IdAstro, Universo};

    fn preparar() -> (Universo, HashMap<String, FichaLuna>, std::path::PathBuf) {
        let raiz = std::env::temp_dir().join(format!("pixpin-abrir-{}", std::process::id()));
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, "p1");
        std::fs::create_dir_all(carpeta.join("archivos")).unwrap();
        std::fs::write(carpeta.join("archivos/a.pdf"), b"%PDF").unwrap();
        let mut u = Universo::nuevo();
        u.astros.push(Astro::galaxia(IdAstro(1), "p1", 0.0, 0.0));
        let mut pl = Astro::planeta(IdAstro(2), 0.0, 0.0, 400.0);
        pl.padre = Some(IdAstro(1));
        u.astros.push(pl);
        let mut l = Astro::luna(IdAstro(3), "m:a", "p1", 0.0, 0.0);
        l.padre = Some(IdAstro(2));
        u.astros.push(l);
        u.astros
            .push(Astro::luna(IdAstro(4), "m:nota", "p1", 0.0, 0.0));
        u.astros
            .push(Astro::luna(IdAstro(5), "m:fuera", "p1", 0.0, 0.0));
        u.astros
            .push(Astro::luna(IdAstro(6), "m:dibujo", "p1", 0.0, 0.0));
        let mut f = HashMap::new();
        f.insert(
            "m:a".into(),
            FichaLuna {
                codigo: "m:a".into(),
                proyecto: "p1".into(),
                clase: ClaseLuna::Archivo,
                ruta: Some("archivos/a.pdf".into()),
                en_equipo: true,
                ..Default::default()
            },
        );
        f.insert(
            "m:nota".into(),
            FichaLuna {
                codigo: "m:nota".into(),
                proyecto: "p1".into(),
                clase: ClaseLuna::Nota,
                en_equipo: true,
                ..Default::default()
            },
        );
        f.insert(
            "m:fuera".into(),
            FichaLuna {
                codigo: "m:fuera".into(),
                proyecto: "p1".into(),
                clase: ClaseLuna::Imagen,
                ruta: Some("/storage/x.jpg".into()),
                en_equipo: false,
                ..Default::default()
            },
        );
        f.insert(
            "m:dibujo".into(),
            FichaLuna {
                codigo: "m:dibujo".into(),
                proyecto: "p1".into(),
                clase: ClaseLuna::Dibujo,
                referencia: Some("d1".into()),
                en_equipo: true,
                ..Default::default()
            },
        );
        (u, f, raiz)
    }

    #[test]
    fn ctrl_clic_abre_cada_cosa_con_lo_suyo() {
        let (u, f, raiz) = preparar();
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, "p1");
        assert_eq!(
            que_abre(&u, IdAstro(3), &f, &raiz, false),
            Apertura::Fichero(carpeta.join("archivos/a.pdf"))
        );
        assert_eq!(
            que_abre(&u, IdAstro(4), &f, &raiz, false),
            Apertura::Chat {
                proyecto: "p1".into(),
                codigo: Some("m:nota".into())
            }
        );
        assert_eq!(
            que_abre(&u, IdAstro(5), &f, &raiz, false),
            Apertura::Nada(Motivo::NoEstaEnEquipo)
        );
        assert_eq!(
            que_abre(&u, IdAstro(6), &f, &raiz, false),
            Apertura::Hoja {
                proyecto: "p1".into(),
                referencia: "d1".into()
            }
        );
        assert_eq!(
            que_abre(&u, IdAstro(1), &f, &raiz, false),
            Apertura::Chat {
                proyecto: "p1".into(),
                codigo: None
            }
        );
        assert_eq!(
            que_abre(&u, IdAstro(2), &f, &raiz, false),
            Apertura::Carpeta(carpeta.clone())
        );
        assert_eq!(
            que_abre(&u, IdAstro(2), &f, &raiz, true),
            Apertura::Varios(vec![carpeta.join("archivos/a.pdf")])
        );
    }

    #[test]
    fn la_hoja_se_busca_por_su_referencia_y_una_que_no_esta_no_se_abre() {
        use pixpin_proyecto::cuaderno::Mensaje;
        let v = vec![
            Mensaje {
                id: "1".into(),
                ..Default::default()
            },
            Mensaje {
                id: "2".into(),
                referencia: Some("d1".into()),
                ..Default::default()
            },
        ];
        assert_eq!(indice_de_referencia(&v, "d1"), Some(1));
        assert_eq!(indice_de_referencia(&v, "otra"), None);
    }

    #[test]
    fn una_pagina_sin_dibujo_se_abre_en_el_lienzo_por_su_codigo() {
        use pixpin_proyecto::cuaderno::{Clase, Mensaje};
        let raiz = std::env::temp_dir();
        let mut pagina = FichaLuna {
            codigo: "u-pag-3".into(),
            proyecto: "p1".into(),
            clase: ClaseLuna::Pagina,
            ..Default::default()
        };
        assert_eq!(
            que_abre_ficha(&pagina, &raiz),
            Apertura::Hoja {
                proyecto: "p1".into(),
                referencia: "u-pag-3".into()
            }
        );
        let v = vec![Mensaje {
            id: "7".into(),
            uid: Some("u-pag-3".into()),
            clase: Some(Clase::Pagina),
            pagina: Some(3),
            ..Default::default()
        }];
        assert_eq!(indice_de_referencia(&v, "u-pag-3"), Some(0));
        // Casos negativos: una pagina extraida como PNG (con fichero) sigue
        // abriendose como fichero, y un codigo que no es de una pagina no
        // encuentra nada.
        pagina.ruta = Some("archivos/pagina-04.png".into());
        assert!(!matches!(que_abre_ficha(&pagina, &raiz), Apertura::Hoja { .. }));
        assert_eq!(indice_de_referencia(&v, "u-otra"), None);
    }

    #[test]
    fn un_exoplaneta_vacio_no_abre_nada() {
        let (mut u, f, raiz) = preparar();
        u.astros
            .push(Astro::planeta(IdAstro(9), 90_000.0, 0.0, 400.0));
        assert_eq!(
            que_abre(&u, IdAstro(9), &f, &raiz, false),
            Apertura::Nada(Motivo::SinArchivos)
        );
    }
}
