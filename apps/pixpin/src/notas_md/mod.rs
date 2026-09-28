//! **El editor de notas Markdown** (H12), hermano del lienzo: el puerto del
//! `MarkdownEditorActivity` del movil con su `EditorVivo`.
//!
//! Lo que hace igual que el movil: lo escrito se ve formateado mientras se
//! escribe (titulos grandes, negrita, cursiva, tachado, codigo, citas,
//! listas con vineta, casillas que se marcan con un clic, enlaces, formulas
//! entre dolares), Intro sigue la lista, y lo guardado es Markdown llano que
//! el movil abre igual en su editor. Crear y abrir, desde el chat del
//! proyecto como las demas hojas.
//!
//! Lo que pone Windows gratis: el control de texto es el `RichEdit` del
//! sistema (`msftedit.dll`), con el IME, el corrector ortografico, deshacer,
//! el zoom con Ctrl+rueda y la accesibilidad de cualquier editor de Windows.
//! Nada que cargar: la DLL viene con Windows.
//!
//! - [`guardar`]: de donde sale el texto y adonde vuelve (puro, probado).
//! - La ventana, en el crate `pixpin-notas` (necesita `unsafe`).
//! - Las cuentas del Markdown, en `pixpin_docs::md_vivo`.

pub mod guardar;

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use pixpin_store::{Catalogo, Ubicacion};

/// Que nota se edita.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "lowercase")]
pub enum Destino {
    /// Una nota de un proyecto, por su codigo unico: un mensaje `NOTA` del
    /// chat o una hoja `nota` del proyecto (tienen el mismo codigo). El
    /// codigo y no el id, porque las hojas del movil que el cuaderno no
    /// tiene se ensenan con un id que se calcula cada vez.
    Mensaje { proyecto: String, codigo: String },
    /// Una nota que aun no existe: se crea en ese proyecto al guardar.
    Nueva { proyecto: String },
    /// Un `.md` suelto (un adjunto del chat): se escribe en su sitio.
    Fichero { ruta: PathBuf },
}

/// Las notas abiertas, con su ventana: pedir otra vez la misma la trae al
/// frente en vez de abrir dos editores que se pisarian al guardar.
static ABIERTAS: Mutex<Vec<(Destino, isize)>> = Mutex::new(Vec::new());

/// Si un mensaje del chat se abre en el editor de notas: una nota escrita
/// (no una mini-app) o un `.md` adjunto.
pub fn se_edita(m: &pixpin_proyecto::cuaderno::Mensaje, ruta: Option<&std::path::Path>) -> bool {
    use pixpin_proyecto::cuaderno::Clase;
    match m.clase {
        Some(Clase::Nota) => m.miniapp.is_none(),
        _ => ruta.is_some_and(es_markdown),
    }
}

pub fn es_markdown(ruta: &std::path::Path) -> bool {
    matches!(
        pixpin_docs::extension(&pixpin_docs::nombre(ruta)).as_str(),
        "md" | "markdown"
    )
}

/// Los textos de la ventana, del catalogo.
fn rotulos(t: &Catalogo) -> pixpin_notas::Rotulos {
    pixpin_notas::Rotulos {
        nueva: t.t("nota-md-nueva"),
        sufijo: t.t("nota-md-sufijo"),
        negrita: t.t("nota-md-negrita"),
        cursiva: t.t("nota-md-cursiva"),
        tachado: t.t("nota-md-tachado"),
        codigo: t.t("nota-md-codigo"),
        enlace: t.t("nota-md-enlace"),
        titulo1: t.t("nota-md-titulo1"),
        titulo2: t.t("nota-md-titulo2"),
        titulo3: t.t("nota-md-titulo3"),
        lista: t.t("nota-md-lista"),
        numerada: t.t("nota-md-numerada"),
        casilla: t.t("nota-md-casilla"),
        marcar: t.t("nota-md-marcar"),
        cita: t.t("nota-md-cita"),
        bloque: t.t("nota-md-bloque"),
        raya: t.t("nota-md-raya"),
        formula: t.t("nota-md-formula"),
        cortar: t.t("nota-md-cortar"),
        copiar: t.t("nota-md-copiar"),
        pegar: t.t("nota-md-pegar"),
        guardar: t.t("nota-md-guardar"),
        copia_md: t.t("nota-md-copia"),
        tipo_md: t.t("nota-md-tipo"),
        no_guardada: t.t("nota-md-no-guardada"),
        mayus: t.t("nota-md-mayus"),
        intro: t.t("nota-md-intro"),
    }
}

/// Abre la nota en su ventana, en su propio hilo (como el lector): quien la
/// pide sigue a lo suyo.
pub fn abrir(idioma: pixpin_store::Idioma, ubicacion: Ubicacion, destino: Destino) {
    if let Ok(v) = ABIERTAS.lock()
        && let Some((_, h)) = v.iter().find(|(d, _)| *d == destino)
    {
        pixpin_shell::overlay::VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(
            *h as *mut _,
        ));
        return;
    }
    let lanzado = std::thread::Builder::new()
        .name("nota-md".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            correr(&textos, &ubicacion, destino);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de la nota");
    }
}

fn correr(textos: &Catalogo, ubicacion: &Ubicacion, destino: Destino) {
    use crate::grupos_ventanas::{self, Clase};
    let raiz = ubicacion.raiz().to_path_buf();
    let Some(texto) = guardar::leer_texto(&raiz, &destino) else {
        tracing::warn!(?destino, "la nota ya no esta; no se abre para no pisarla");
        return;
    };
    let aparato = pixpin_proyecto::identidad::Identidad::leer_o_crear(&raiz, "PC")
        .map(|i| i.yo.codigo())
        .unwrap_or_default();
    let clase = Clase::Nota {
        destino: destino.clone(),
    };
    let apunte = grupos_ventanas::apuntar(clase.clone());
    let colocacion = grupos_ventanas::tomar_colocacion(&clase);
    let nombre_de_fichero = match &destino {
        Destino::Fichero { ruta } => Some(pixpin_docs::nombre(ruta)),
        _ => None,
    };
    let inicial = destino.clone();
    let mut actual = destino;
    let resultado = pixpin_notas::correr(
        pixpin_notas::Pedido {
            texto,
            rotulos: rotulos(textos),
            nombre_de_fichero,
            colocacion,
        },
        &mut |hwnd| {
            apunte.con_ventana(hwnd);
            if let Ok(mut v) = ABIERTAS.lock() {
                v.push((inicial.clone(), hwnd));
            }
        },
        &mut |texto: &str| {
            let ahora = pixpin_shell::entorno::ahora_utc_ms();
            match guardar::guardar(&raiz, &actual, texto, &aparato, ahora) {
                Ok(nuevo) => {
                    if nuevo != actual {
                        apunte.cambiar_clase(Clase::Nota {
                            destino: nuevo.clone(),
                        });
                        if let Ok(mut v) = ABIERTAS.lock() {
                            for (d, _) in v.iter_mut().filter(|(d, _)| *d == actual) {
                                *d = nuevo.clone();
                            }
                        }
                        actual = nuevo;
                    }
                    // La burbuja del chat tiene que ensenar lo nuevo.
                    if !matches!(actual, Destino::Fichero { .. }) {
                        crate::ventana_chat::refrescar();
                    }
                    true
                }
                Err(e) => {
                    tracing::error!(?e, "no se pudo guardar la nota");
                    false
                }
            }
        },
    );
    if let Ok(mut v) = ABIERTAS.lock() {
        v.retain(|(d, _)| *d != actual && *d != inicial);
    }
    if let Err(e) = resultado {
        tracing::warn!(?e, "no se pudo abrir el editor de notas");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::cuaderno::{Clase, Mensaje};

    #[test]
    fn se_editan_las_notas_escritas_y_los_md_y_nada_mas() {
        let nota = Mensaje {
            clase: Some(Clase::Nota),
            ..Default::default()
        };
        assert!(se_edita(&nota, None));
        let mini = Mensaje {
            clase: Some(Clase::Nota),
            miniapp: Some("tabla".into()),
            ..Default::default()
        };
        assert!(!se_edita(&mini, None));
        let archivo = Mensaje::default();
        assert!(se_edita(&archivo, Some(std::path::Path::new("C:\\a\\Apuntes.MD"))));
        assert!(!se_edita(&archivo, Some(std::path::Path::new("C:\\a\\plano.pdf"))));
        assert!(!se_edita(&archivo, None));
    }

    #[test]
    fn el_destino_viaja_en_json_con_su_tipo() {
        let d = Destino::Mensaje {
            proyecto: "p1".into(),
            codigo: "U1".into(),
        };
        let t = serde_json::to_string(&d).unwrap();
        assert!(t.contains("\"tipo\":\"mensaje\""));
        assert_eq!(serde_json::from_str::<Destino>(&t).unwrap(), d);
    }
}
