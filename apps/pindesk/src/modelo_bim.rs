//! **Ver modelos BIM (IFC y Revit)** (9-oct-2026): el usuario pidio «un visor
//! sencillo como un pin, que se pueda ver facilmente cualquier modelo». Se
//! lee con `pindesk-bim` y se ve en `pindesk_cad::ventana3d`; aqui, como se
//! abre desde PinDesk. Igual que los planos (`plano_cad`): el modelo se lee en
//! otro proceso (`pindesk.exe --bim-convertir <modelo> <cache>`), asi un
//! fichero roto solo tumba a ese proceso, y lo leido queda en
//! `<raiz>/cache/bim/`.

use std::path::{Path, PathBuf};

use pindesk_store::{Idioma, Ubicacion};

pub const ORDEN: &str = "--bim-convertir";
const EN_CACHE: usize = 16;

pub fn se_abre(nombre: &str) -> bool {
    pindesk_bim::se_abre(&pindesk_docs::extension(nombre))
}

/// Si PinDesk se lanzo para leer un modelo, lo lee y dice con que codigo
/// salir. `None`: es un arranque normal.
pub fn convertir_si_toca() -> Option<i32> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.get(1).is_none_or(|a| a != ORDEN) {
        return None;
    }
    let (Some(entrada), Some(salida)) = (args.get(2), args.get(3)) else {
        return Some(2);
    };
    let (entrada, salida) = (PathBuf::from(entrada), PathBuf::from(salida));
    Some(match pindesk_bim::convertir_fichero(&entrada) {
        Ok(modelo) => {
            let temporal = salida.with_extension("tmp");
            let escrito = std::fs::write(&temporal, modelo.a_bytes())
                .and_then(|()| std::fs::rename(&temporal, &salida));
            if escrito.is_ok() { 0 } else { 3 }
        }
        Err(e) => {
            let _ = std::fs::write(salida.with_extension("error"), e);
            1
        }
    })
}

fn podar(carpeta: &Path) {
    let Ok(d) = std::fs::read_dir(carpeta) else {
        return;
    };
    let mut v: Vec<(std::time::SystemTime, PathBuf)> = d
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.0));
    for (_, p) in v.into_iter().skip(EN_CACHE) {
        let _ = std::fs::remove_file(p);
    }
}

fn cargar(raiz: &Path, ruta: &Path) -> Result<pindesk_cad::modelo3d::Modelo3d, String> {
    use pindesk_cad::modelo3d::Modelo3d;
    let carpeta = raiz.join("cache").join("bim");
    std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
    let clave = crate::plano_cad::clave(ruta).ok_or("no se puede leer el fichero")?;
    let destino = carpeta.join(format!("{clave}.px3d"));
    if let Ok(b) = std::fs::read(&destino)
        && let Some(m) = Modelo3d::de_bytes(&b)
    {
        let _ = std::fs::File::options().append(true).open(&destino);
        return Ok(m);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut orden = std::process::Command::new(exe);
    orden.arg(ORDEN).arg(ruta).arg(&destino);
    {
        use std::os::windows::process::CommandExt;
        orden.creation_flags(0x0800_0000);
    }
    let t = std::time::Instant::now();
    let estado = orden.status().map_err(|e| e.to_string())?;
    tracing::info!(ms = t.elapsed().as_millis() as u64, codigo = ?estado.code(), ruta = %ruta.display(), "modelo BIM leido aparte");
    if !estado.success() {
        let motivo = std::fs::read_to_string(destino.with_extension("error")).unwrap_or_default();
        let _ = std::fs::remove_file(destino.with_extension("error"));
        return Err(if motivo.is_empty() {
            "el modelo no se pudo leer (puede estar dañado)".into()
        } else {
            motivo
        });
    }
    podar(&carpeta);
    let b = std::fs::read(&destino).map_err(|e| e.to_string())?;
    Modelo3d::de_bytes(&b).ok_or_else(|| "lo leido no se entiende".to_string())
}

fn textos(idioma: Idioma) -> pindesk_cad::ventana3d::TextosUi3d {
    let t = pindesk_store::Catalogo::nuevo(idioma);
    let tecla = |clave: &str, atajo: &str| crate::plano_cad::con_tecla(&t, clave, atajo);
    pindesk_cad::ventana3d::TextosUi3d {
        abriendo: t.t("cad-modelo-abriendo"),
        vacio: t.t("cad-modelo-vacio"),
        error: t.t("cad-modelo-error"),
        todo: tecla("cad-modelo-todo", "F"),
        tema: tecla("cad-tema", "B"),
        corte: tecla("cad-corte", "C"),
        aislar: tecla("cad-aislar", "I"),
        panel: tecla("cad-capas", "L"),
        niveles: t.t("cad-niveles"),
        categorias: t.t("cad-categorias"),
        sin_nivel: t.t("cad-sin-nivel"),
        mostrar_todo: t.t("cad-mostrar-todo"),
        aristas: tecla("cad-aristas", "E"),
        planta: tecla("cad-planta", "P"),
        encima: tecla("cad-encima", "T"),
        cerrar: tecla("cad-cerrar", "Esc"),
        altura_corte: t.t("cad-altura-corte"),
        pista: t.t("cad-modelo-pista"),
    }
}

/// Los textos de los marcadores y los marcos del modelo.
fn textos_marcos(idioma: Idioma) -> pindesk_cad::ventana3d::TextosMarcos3d {
    let mut t = textos_marcos_sin_menu(idioma);
    // El clic derecho en el boton del marco, como en los demas visores.
    t.imprimir_menu =
        crate::marcos_doc::textos_de_imprimir(idioma, crate::marcos_doc::Entero::Modelo);
    t
}

fn textos_marcos_sin_menu(idioma: Idioma) -> pindesk_cad::ventana3d::TextosMarcos3d {
    let t = pindesk_store::Catalogo::nuevo(idioma);
    let tecla = |clave: &str, atajo: &str| crate::plano_cad::con_tecla(&t, clave, atajo);
    pindesk_cad::ventana3d::TextosMarcos3d {
        marcador: tecla("cad-modelo-marcador", "Ctrl+M"),
        marcos: tecla("cad-marcos", "I"),
        imprimir: tecla("cad-imprimir-marcos", "Ctrl+P"),
        borrar_marcos: t.t("cad-borrar-marcos"),
        pista_marcos: t.t("cad-modelo-pista-marcos"),
        pista_anotar: t.t("cad-modelo-pista-anotar"),
        anotar: tecla("cad-modelo-anotar", "A"),
        botones: crate::plano_cad::nombres_de_botones(&t),
        imprimir_menu: pindesk_cad::ventana::TextosImprimir::modelo(),
    }
}

/// **El riel del modelo** (19-oct-2026): los marcadores con todo el estado
/// de la vista, lo anotado sobre cada uno con el motor del lienzo (una capa
/// por marcador) y los marcos numerados que se imprimen en grupo: se
/// fotografia cada uno en su vista y van juntos al dialogo
/// (`marco_imprimir::imprimir_fotos`). De un adjunto del chat, todo en los
/// ficheros `anot-<uid>…` de su mensaje, que viajan
/// (`plano_cad::SitioDeMarcadores`).
fn barra_del_modelo(
    idioma: Idioma,
    raiz: &Path,
    ruta: &Path,
    nombre: &str,
) -> pindesk_cad::ventana3d::Barra3d {
    use pindesk_cad::ventana3d::Foto;
    let sitio = crate::plano_cad::SitioDeMarcadores::abrir(raiz, ruta);
    let (s1, s2) = (sitio.clone(), sitio.clone());
    let titulo = nombre.to_string();
    pindesk_cad::ventana3d::Barra3d {
        tipo: crate::plano_cad::tipo_de_archivo(ruta),
        marcadores: Some(sitio.vistas()),
        anotador: Some(Box::new(move |id: &str| {
            Box::new(crate::plano_cad::anotar::motor_de_marcador(&s1, id))
                as Box<dyn pindesk_cad::anotado::Anotador>
        })),
        // Sin borrar los ficheros: la capa con todo borrado (ver `vaciar_capa`).
        quitar_tinta: Some(Box::new(move |id: &str| {
            let r = crate::plano_cad::anotar::rutas_de_marcador(&s2, id);
            if let Err(e) = crate::plano_cad::anotar::vaciar_capa(&r) {
                tracing::warn!(?e, tinta = %r.tinta.display(), "no se pudo vaciar lo anotado del marcador");
            }
        })),
        foto: Some(Box::new(|r: [i32; 4]| {
            let area = pindesk_geom::Rect {
                x: r[0],
                y: r[1],
                ancho: r[2].max(0) as u32,
                alto: r[3].max(0) as u32,
            };
            crate::marco_imprimir::foto_de_pantalla(area).map(|i| (i.ancho, i.alto, i.pixeles))
        })),
        imprimir: Some(Box::new(move |v: isize, fotos: Vec<Foto>| {
            let fotos = fotos
                .into_iter()
                .map(|(ancho, alto, pixeles)| pindesk_codec::imagen::ImagenRgba {
                    ancho,
                    alto,
                    pixeles,
                })
                .collect();
            if let Err(e) = crate::marco_imprimir::imprimir_fotos(v, fotos, &titulo) {
                tracing::warn!(?e, "no se pudieron imprimir los marcos del modelo");
            }
        })),
        textos: textos_marcos(idioma),
        globo: Some(crate::plano_cad::globo_de_la_barra()),
        guardar: crate::plano_cad::guardar_de_paso(idioma, raiz, ruta),
    }
}

/// Abre el modelo en su ventana, en su propio hilo.
pub fn lanzar(idioma: Idioma, ubicacion: Ubicacion, ruta: &Path) {
    let ruta = ruta.to_path_buf();
    let raiz = ubicacion.raiz().to_path_buf();
    let lanzado = std::thread::Builder::new()
        .name("modelo-bim".into())
        .spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel();
            let (r2, raiz2) = (ruta.clone(), raiz.clone());
            let _ = std::thread::Builder::new()
                .name("modelo-bim-leer".into())
                .spawn(move || {
                    let _ = tx.send(cargar(&raiz2, &r2));
                });
            let titulo = ruta
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let extras = pindesk_cad::ventana3d::Extras {
                exportar: Some(crate::compartir::exportable::al_exportar_3d(
                    idioma,
                    titulo.clone(),
                    Some(ruta.clone()),
                    None,
                )),
                ..Default::default()
            };
            // En la barra, el tipo y el nombre, como en los demas pines.
            let nombre = crate::plano_cad::nombre_para_la_barra(&ruta);
            let barra = barra_del_modelo(idioma, &raiz, &ruta, &nombre);
            if let Err(e) =
                pindesk_cad::ventana3d::ver_con_barra(&nombre, barra, rx, textos(idioma), extras)
            {
                tracing::warn!(%e, "no se pudo abrir el visor de modelos");
                let _ = pindesk_shell::abrir(&ruta);
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el visor de modelos");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_abren_los_ifc_y_los_revit() {
        assert!(se_abre("Edificio.IFC") && se_abre("obra.rvt"));
        // Caso negativo: lo demas no (un DWG va al visor de planos).
        assert!(!se_abre("plano.dwg") && !se_abre("ifc"));
    }
}
