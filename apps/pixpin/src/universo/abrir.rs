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
        _ => match fichero_de(raiz, f) {
            Some(r) => Apertura::Fichero(r),
            None => Apertura::Nada(Motivo::NoEstaEnEquipo),
        },
    }
}

/// Lo que se hace con cada apertura. `Hoja` no: la abre quien llama al
/// editor (ver `Sesion::hoja_pedida`), porque hay que cerrar el universo
/// antes. `Chat` se enchufa cuando el chat sepa ir a un mensaje (Tarea 18).
pub fn ejecutar(a: &Apertura) {
    let abrir = |r: &Path| {
        if let Err(e) = pixpin_shell::abrir(r) {
            tracing::warn!(?e, ruta = %r.display(), "no se pudo abrir desde el universo");
        }
    };
    match a {
        Apertura::Fichero(r) | Apertura::Carpeta(r) => abrir(r),
        Apertura::Varios(v) => v.iter().for_each(|r| abrir(r)),
        Apertura::Chat { proyecto, codigo } => {
            tracing::info!(%proyecto, ?codigo, "ir al chat desde el universo (pendiente)");
        }
        Apertura::Hoja { .. } | Apertura::Nada(_) => {}
    }
}

/// El ancho con el que se dibuja una pagina de PDF de fondo: el del chat.
const ANCHO_PAGINA: u32 = 1600;
/// Cuantas hojas enlazadas se siguen antes de parar.
const SALTOS_MAXIMOS: usize = 32;

/// Abre una hoja de un proyecto en el lienzo y la guarda al cerrar,
/// siguiendo los enlaces a otras hojas. Es lo que hace el chat con
/// `abrir_dibujo`; va aqui aparte porque el chat no se puede tocar ahora
/// mismo (ver el informe de la Tarea 14).
pub fn abrir_hoja(
    raiz: &Path,
    proyecto: &str,
    referencia: &str,
    opciones: crate::ventana_chat::OpcionesLienzo,
) {
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, proyecto);
    let mensajes = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta)
        .map(|c| c.mensajes)
        .unwrap_or_default();
    let mut actual = referencia.to_string();
    for _ in 0..SALTOS_MAXIMOS {
        let ruta = pixpin_proyecto::almacen::lienzo(raiz, proyecto, &actual);
        let lienzo = match std::fs::read_to_string(&ruta)
            .map_err(|e| e.to_string())
            .and_then(|t| pixpin_motor2d::excalidraw::leer(&t).map_err(|e| e.to_string()))
        {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!(%e, ruta = %ruta.display(), "no se pudo abrir la hoja");
                return;
            }
        };
        let escena = pixpin_motor2d::excalidraw::a_escena(&lienzo);
        let fotos: Vec<(u64, PathBuf)> = pixpin_motor2d::excalidraw::ficheros(&lienzo)
            .into_iter()
            .map(|(id, rel)| (id, carpeta.join(rel)))
            .collect();
        let pagina = mensajes
            .iter()
            .find(|m| m.referencia.as_deref() == Some(actual.as_str()))
            .and_then(|m| m.pagina);
        let fondo = pagina.and_then(|pagina| {
            pixpin_pdf::Documento::abrir(&carpeta.join("documento.pdf"))
                .and_then(|d| d.renderizar(pagina, ANCHO_PAGINA))
                .map_err(|e| tracing::warn!(?e, pagina, "no se pudo dibujar la pagina"))
                .ok()
        });
        let (escena, destino) = match crate::ventana_editor::abrir(
            escena,
            opciones.enganche,
            opciones.nivel,
            opciones.medir_fotogramas,
            fondo,
            &fotos,
        ) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(?e, "no se pudo abrir el lienzo de la hoja");
                return;
            }
        };
        guardar_hoja(&ruta, &lienzo, &escena);
        match destino {
            Some(d) => actual = d,
            None => return,
        }
    }
}

/// Lo mismo que `guardar_hoja_dibujada` del chat: a un temporal y
/// renombrar, y solo si algo cambio.
fn guardar_hoja(
    ruta: &Path,
    lienzo: &pixpin_motor2d::excalidraw::Lienzo,
    escena: &pixpin_motor2d::Escena,
) {
    use pixpin_motor2d::excalidraw::{con_escena, escribir};
    let despues = escribir(&con_escena(lienzo, escena));
    if escribir(lienzo) == despues {
        return;
    }
    let temporal = ruta.with_extension("excalidraw.tmp");
    if let Err(e) =
        std::fs::write(&temporal, despues).and_then(|()| std::fs::rename(&temporal, ruta))
    {
        tracing::error!(?e, ruta = %ruta.display(), "no se pudo guardar la hoja");
    }
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
