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

pub mod adjuntos;
pub mod comentarios;
pub mod comentarios_web;
pub mod guardar;
pub mod incrustados;
pub mod lector_web;
pub mod paginas_vivas;

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

/// Donde se recuerda la letra y el tamano de las notas (H12): al lado del
/// almacen y sin viajar, como `vista-del-chat.txt`; es de este equipo.
pub(crate) const FICHERO_VISTA: &str = "vista-de-notas.txt";

/// Como se llama una nota en ese fichero: por su codigo unico (el que no
/// cambia aunque se le cambie el titulo) o por su ruta.
pub(crate) fn clave_de_vista(d: &Destino) -> String {
    match d {
        Destino::Mensaje { codigo, .. } => format!("nota:{codigo}"),
        Destino::Nueva { proyecto } => format!("nueva:{proyecto}"),
        Destino::Fichero { ruta } => format!("fichero:{}", ruta.display()),
    }
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
        compartir: t.t("nota-md-compartir"),
        tabla: t.t("nota-md-tabla"),
        imagen: t.t("nota-md-imagen"),
        fecha: t.t("nota-md-fecha"),
        pista_barra: t.t("nota-md-pista-barra"),
        cambiar_titulo: t.t("nota-md-cambiar-titulo"),
        fila_encima: t.t("nota-md-fila-encima"),
        fila_debajo: t.t("nota-md-fila-debajo"),
        quitar_fila: t.t("nota-md-quitar-fila"),
        columna_izquierda: t.t("nota-md-columna-izquierda"),
        columna_derecha: t.t("nota-md-columna-derecha"),
        quitar_columna: t.t("nota-md-quitar-columna"),
        quitar_tabla: t.t("nota-md-quitar-tabla"),
        boton_fila_mas: t.t("nota-md-boton-fila-mas"),
        boton_fila_menos: t.t("nota-md-boton-fila-menos"),
        boton_columna_mas: t.t("nota-md-boton-columna-mas"),
        boton_columna_menos: t.t("nota-md-boton-columna-menos"),
        boton_combinar: t.t("nota-md-boton-combinar"),
        boton_color: t.t("nota-md-boton-color"),
        combinar_celdas: t.t("nota-md-combinar-celdas"),
        separar_celdas: t.t("nota-md-separar-celdas"),
        color_fondo: t.t("nota-md-color-fondo"),
        color_letra: t.t("nota-md-color-letra"),
        aplicar_a: t.t("nota-md-aplicar-a"),
        nombres_fondo: t.t("nota-md-nombres-fondo"),
        nombres_letra: t.t("nota-md-nombres-letra"),
        pista_color: t.t("nota-md-pista-color"),
        meses: t.t("nota-md-meses"),
        imagen_no_copiada: t.t("nota-md-imagen-no-copiada"),
        fotos: paginas_vivas::rotulos(t),
        incrustados: incrustados::rotulos(t),
        pagina_viva: t.t("nota-md-pagina-viva"),
        enlace_hoja: t.t("nota-md-enlace-hoja"),
        comentarios: comentarios::rotulos(t),
        exportar_word: t.t("nota-md-exportar-word"),
        tipo_word: t.t("nota-md-tipo-word"),
        barra: pixpin_notas::barra_flotante::RotulosBarra {
            texto_normal: t.t("nota-md-texto-normal"),
            quitar_formato: t.t("nota-md-quitar-formato"),
            enlace_pista: t.t("nota-md-enlace-pista"),
            emojis: t.t("nota-md-emojis"),
            letra_texto: t.t("nota-md-letra-texto"),
            letra_titulos: t.t("nota-md-letra-titulos"),
            tamano: t.t("nota-md-tamano"),
            tamanos: t.t("nota-md-tamanos"),
            solo_esta_nota: t.t("nota-md-solo-esta-nota"),
            pista_tamano: t.t("nota-md-pista-tamano"),
        },
    }
}

/// Abre la nota en su ventana, en su propio hilo (como el lector): quien la
/// pide sigue a lo suyo.
pub fn abrir(idioma: pixpin_store::Idioma, ubicacion: Ubicacion, destino: Destino) {
    tracing::info!(?destino, "abrir nota");
    if let Ok(v) = ABIERTAS.lock()
        && let Some((_, h)) = v.iter().find(|(d, _)| *d == destino)
    {
        tracing::info!(hwnd = *h, "la nota ya estaba abierta: al frente");
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
            correr(&textos, idioma, &ubicacion, destino);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de la nota");
    }
}

/// Abre la hoja de compartir con la nota ya guardada: como un mensaje
/// `NOTA` del proyecto (el del cuaderno si lo hay, o uno hecho con su texto
/// si es una hoja del movil o un `.md` suelto), que la hoja sabe sacar como
/// texto, PDF, imagen o pagina web.
pub(crate) fn compartir(
    idioma: pixpin_store::Idioma,
    ubicacion: &Ubicacion,
    destino: &Destino,
    aparato: &str,
) {
    use pixpin_proyecto::{almacen, cuaderno};
    let raiz = ubicacion.raiz().to_path_buf();
    let Some(texto) = guardar::leer_texto(&raiz, destino) else {
        return;
    };
    let (proyecto, codigo) = match destino {
        Destino::Mensaje { proyecto, codigo } => (proyecto.clone(), Some(codigo.clone())),
        Destino::Nueva { proyecto } => (proyecto.clone(), None),
        Destino::Fichero { .. } => (String::new(), None),
    };
    let del_cuaderno = codigo.as_ref().and_then(|c| {
        cuaderno::Cuaderno::leer_de(&almacen::carpeta(&raiz, &proyecto))
            .ok()?
            .mensajes
            .into_iter()
            .find(|m| m.codigo_unico() == *c)
    });
    let mensaje = del_cuaderno.unwrap_or_else(|| {
        cuaderno::Mensaje::nota(
            &texto,
            &cuaderno::Sello {
                cuando: pixpin_shell::entorno::ahora_utc_ms(),
                numero: 0,
                aparato: aparato.to_string(),
                proyecto: proyecto.clone(),
            },
        )
    });
    let titulo = match pixpin_docs::md_vivo::titulo(&texto) {
        t if t.is_empty() => Catalogo::nuevo(idioma).t("nota-md-nueva"),
        t => t,
    };
    crate::compartir::ventana::abrir(
        idioma,
        ubicacion.clone(),
        crate::compartir::Cosa::Mensajes {
            raiz,
            proyecto,
            titulo,
            mensajes: vec![mensaje],
        },
    );
}

/// Abre la nota en el lector nuevo (`lector_web`, 8-oct-2026); si este
/// equipo no tiene WebView2, o se pide desde el lector, en el de antes.
fn correr(
    textos: &Catalogo,
    idioma: pixpin_store::Idioma,
    ubicacion: &Ubicacion,
    destino: Destino,
) {
    use crate::grupos_ventanas::{self, Clase};
    use std::cell::RefCell;
    use std::rc::Rc;
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
    let inicial = destino.clone();
    let actual = Rc::new(RefCell::new(destino));
    let resultado = lector_web::correr(lector_web::Contexto {
        textos,
        idioma,
        ubicacion,
        aparato: &aparato,
        actual: actual.clone(),
        texto,
        colocacion,
        al_nacer: &mut |hwnd| {
            apunte.con_ventana(hwnd);
            if let Ok(mut v) = ABIERTAS.lock() {
                v.push((inicial.clone(), hwnd));
            }
        },
        al_cambiar_destino: &mut |antes, nuevo| {
            apunte.cambiar_clase(Clase::Nota {
                destino: nuevo.clone(),
            });
            if let Ok(mut v) = ABIERTAS.lock() {
                for (d, _) in v.iter_mut().filter(|(d, _)| d == antes) {
                    *d = nuevo.clone();
                }
            }
        },
    });
    let fin = actual.borrow().clone();
    if let Ok(mut v) = ABIERTAS.lock() {
        v.retain(|(d, _)| *d != fin && *d != inicial);
    }
    drop(apunte);
    match resultado {
        Ok(lector_web::Salida::Cerrada) => {}
        Ok(lector_web::Salida::EditorAnterior) => correr_anterior(textos, idioma, ubicacion, fin),
        Err(e) => {
            tracing::warn!(?e, "sin el lector nuevo; se abre el editor de antes");
            correr_anterior(textos, idioma, ubicacion, fin);
        }
    }
}

/// El editor de antes (`pixpin-notas`, sobre el `RichEdit`).
fn correr_anterior(
    textos: &Catalogo,
    idioma: pixpin_store::Idioma,
    ubicacion: &Ubicacion,
    destino: Destino,
) {
    use crate::grupos_ventanas::{self, Clase};
    use std::cell::RefCell;
    use std::rc::Rc;
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
    // Lo comparten guardar, las fotos y compartir: una nota nueva pasa a
    // ser su mensaje al guardarse la primera vez.
    let actual = Rc::new(RefCell::new(destino));
    let (a_resolver, a_adjuntar, a_compartir) = (actual.clone(), actual.clone(), actual.clone());
    let (r1, r2) = (raiz.clone(), raiz.clone());
    let ub = ubicacion.clone();
    let ap = aparato.clone();
    let resultado = pixpin_notas::correr(
        pixpin_notas::Pedido {
            texto,
            rotulos: rotulos(textos),
            nombre_de_fichero,
            colocacion,
            resolver: Box::new(move |ruta: &str| {
                adjuntos::resolver(&r1, &a_resolver.borrow(), ruta)
            }),
            adjuntar: Box::new(move |origen: &std::path::Path| {
                let ahora = pixpin_shell::entorno::ahora_utc_ms();
                adjuntos::adjuntar(&r2, &a_adjuntar.borrow(), origen, ahora)
                    .map_err(|e| tracing::warn!(?e, "no se pudo copiar la foto junto a la nota"))
                    .ok()
            }),
            compartir: Some(Box::new(move |_hwnd: isize| {
                compartir(idioma, &ub, &a_compartir.borrow(), &ap);
            })),
            integracion: {
                let mut i = paginas_vivas::integracion(idioma, ubicacion, &actual, &inicial);
                // La letra y el tamano con que se lee (no van en el `.md`).
                i.ajustes_vista = Some(raiz.join(FICHERO_VISTA));
                // Documentos, mensajes del chat y audios (`incrustados`).
                i.medios = Some(Box::new(incrustados::MediosDeLaNota::nuevo(
                    idioma, ubicacion, &actual,
                )));
                let a = actual.clone();
                i.clave_vista = Some(Box::new(move || clave_de_vista(&a.borrow())));
                i
            },
            comentarios: comentarios::de(&raiz, &actual, &aparato),
        },
        &mut |hwnd| {
            apunte.con_ventana(hwnd);
            if let Ok(mut v) = ABIERTAS.lock() {
                v.push((inicial.clone(), hwnd));
            }
        },
        &mut |texto: &str| {
            let ahora = pixpin_shell::entorno::ahora_utc_ms();
            let antes = actual.borrow().clone();
            match guardar::guardar(&raiz, &antes, texto, &aparato, ahora) {
                Ok(nuevo) => {
                    if nuevo != antes {
                        apunte.cambiar_clase(Clase::Nota {
                            destino: nuevo.clone(),
                        });
                        if let Ok(mut v) = ABIERTAS.lock() {
                            for (d, _) in v.iter_mut().filter(|(d, _)| *d == antes) {
                                *d = nuevo.clone();
                            }
                        }
                        *actual.borrow_mut() = nuevo;
                    }
                    // La burbuja del chat tiene que ensenar lo nuevo.
                    if !matches!(*actual.borrow(), Destino::Fichero { .. }) {
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
    let fin = actual.borrow().clone();
    if let Ok(mut v) = ABIERTAS.lock() {
        v.retain(|(d, _)| *d != fin && *d != inicial);
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
    fn la_letra_de_una_nota_se_recuerda_por_su_codigo_y_no_por_su_titulo() {
        let m = Destino::Mensaje {
            proyecto: "Casa Lima".into(),
            codigo: "K7Q2ABCDEF".into(),
        };
        assert_eq!(clave_de_vista(&m), "nota:K7Q2ABCDEF");
        let f = Destino::Fichero {
            ruta: std::path::PathBuf::from("C:\\notas\\plan.md"),
        };
        assert_eq!(clave_de_vista(&f), "fichero:C:\\notas\\plan.md");
        // Caso negativo: dos notas distintas no comparten clave.
        let n = Destino::Nueva {
            proyecto: "Casa Lima".into(),
        };
        assert_ne!(clave_de_vista(&n), clave_de_vista(&m));
    }

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
        assert!(se_edita(
            &archivo,
            Some(std::path::Path::new("C:\\a\\Apuntes.MD"))
        ));
        assert!(!se_edita(
            &archivo,
            Some(std::path::Path::new("C:\\a\\plano.pdf"))
        ));
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
