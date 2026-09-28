//! La ventana de ajustes (P6).
//!
//! Hasta ahora todo se tocaba editando el TOML a mano y reiniciando. Esto
//! ensena lo mismo en cinco pestanas —atajos, general, captura, dibujo y
//! las herramientas de dibujo que salen—
//! y lo guarda al cerrar SIN borrar los comentarios del fichero, que era la
//! condicion para poder escribirlo desde el programa.
//!
//! La logica de donde cae cada cosa y que pasa al pulsarla vive en
//! `pixpin_ui::ajustes`, pura y probada. Aqui esta lo que necesita una
//! pantalla: la ventana, el pintado, y traducir cada golpe a un cambio en
//! la copia de trabajo de los ajustes.
//!
//! Lo que no cabe en un clic o en una tecla —la carpeta de capturas, la
//! lista de programas a ignorar, las regiones— sigue en el fichero, y hay
//! un boton que lo abre. Una caja de texto propia para tres ajustes que se
//! cambian una vez en la vida no compensa lo que cuesta hacerla bien.

use anyhow::{Context, Result};
use pixpin_geom::{Punto, Rect};
use pixpin_render::{Color, MotorRender, RectF, Superficie};
use pixpin_shell::Atajo;
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::ajustes::{
    Ajustes, FormatoColor, ModoDeIdiomas, NivelPdf, PreferenciaIdioma, PreferenciaNivel,
};
use pixpin_store::comandos::{CATALOGO, Comando, Enlaces};
use pixpin_store::{Catalogo, Ubicacion};
use pixpin_ui::ajustes::{
    BOTON_LADO, Control, Estado, FILA_ALTO, Fila, Golpe, MARGEN, PESTANAS_ALTO, PIE_ALTO, Recta,
    alto_de_lista, botones_de_numero, botones_del_pie, cajas_de_opcion, golpe_en,
    limitar_desplazamiento, numero_tras, pestanas, rect_de_fila, zona_de_control,
};

/// Medidas de la ventana en pixeles logicos.
const ANCHO: u32 = 660;
const ALTO: u32 = 540;
/// Cuantos pixeles baja la lista por cada muesca de la rueda.
const PASO_RUEDA: i32 = 48;
/// La pestana de las herramientas de dibujo (la quinta).
const PESTANA_HERRAMIENTAS: usize = 4;

const FONDO: Color = Color {
    r: 0.11,
    g: 0.11,
    b: 0.12,
    a: 0.98,
};
const FONDO_BARRA: Color = Color {
    r: 0.08,
    g: 0.08,
    b: 0.09,
    a: 1.0,
};
const TINTA: Color = Color {
    r: 0.95,
    g: 0.95,
    b: 0.96,
    a: 1.0,
};
const TENUE: Color = Color {
    r: 0.58,
    g: 0.59,
    b: 0.63,
    a: 1.0,
};
const CAJA: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.08,
};
const CAJA_ACTIVA: Color = Color {
    r: 0.16,
    g: 0.51,
    b: 0.96,
    a: 1.0,
};
const AVISO: Color = Color {
    r: 0.90,
    g: 0.30,
    b: 0.25,
    a: 1.0,
};
const SEPARADOR: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.06,
};

/// Que ajuste hay detras de cada fila.
///
/// La fila solo sabe que control ensena; esto es lo que dice a que campo
/// de `Ajustes` corresponde. Va aparte para que anadir un ajuste sea
/// anadir una variante aqui y una fila en `filas_de`, y que el compilador
/// avise si falta el `match` de aplicarlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Clave {
    Comando(Comando),
    Idioma,
    Arranque,
    TemaCosmos,
    Color,
    Nivel,
    PdfAligerar,
    PdfNivel,
    VozSegundo,
    VozModo,
    RetardoCaptura,
    LimiteScroll,
    GifRitmo,
    GifRetardo,
    ImanActivo,
    ImanEsquinas,
    ImanMedios,
    ImanCentros,
    ImanRadio,
    /// Una herramienta de dibujo, por su nombre estable del TOML
    /// (`pixpin_store::herramientas::NOMBRES`).
    Herramienta(&'static str),
    /// Un titulo de seccion: no hay nada detras.
    Seccion,
}

/// Abre la ventana y no vuelve hasta que se cierra.
///
/// Devuelve los ajustes nuevos si algo cambio (ya guardados en el
/// fichero), o `None` si se cerro sin tocar nada. Quien llama decide que
/// hacer con ellos: volver a registrar los atajos, sobre todo.
pub fn abrir(
    recursos: &crate::overlay::Recursos,
    actual: &Ajustes,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
) -> Result<Option<Ajustes>> {
    let disposicion = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = disposicion
        .principal()
        .context("sin monitor principal")?
        .to_owned();
    let e = monitor.escala_por_cien as f32 / 100.0;
    let (fisico_ancho, fisico_alto) = ((ANCHO as f32 * e) as u32, (ALTO as f32 * e) as u32);
    let marco = Rect {
        x: monitor.area.x + (monitor.area.ancho as i32 - fisico_ancho as i32) / 2,
        y: monitor.area.y + (monitor.area.alto as i32 - fisico_alto as i32) / 2,
        ancho: fisico_ancho,
        alto: fisico_alto,
    };
    let ventana = VentanaOverlay::nueva(marco).context("no se pudo abrir la ventana de ajustes")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        fisico_ancho,
        fisico_alto,
    )
    .context("sin superficie para los ajustes")?;
    ventana.mostrar();
    ventana.enfocar();

    let nombres_pestanas = [
        textos.t("ajustes-pestana-atajos"),
        textos.t("ajustes-pestana-general"),
        textos.t("ajustes-pestana-captura"),
        textos.t("ajustes-pestana-dibujo"),
        textos.t("ajustes-pestana-herramientas"),
    ];
    // Copia de trabajo: se toca esta y se guarda al cerrar. Tocar los
    // ajustes de verdad en cada clic obligaria a deshacer a mano si se
    // cierra sin querer.
    let mut ajustes = actual.clone();
    let (mut enlaces, _) = Enlaces::de_ajustes(&ajustes);
    let mut estado = Estado::default();
    let mut cambiado = false;
    let mut resaltado: Option<Golpe> = None;
    let mut pintado: Option<(Estado, Option<Golpe>, u64)> = None;
    let mut version = 0u64;

    loop {
        pixpin_shell::overlay::bombear_pendientes();
        let mut filas = filas_de(estado.pestana, &ajustes, &enlaces, textos);
        let mut cerrar = false;

        for (hwnd, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            let local = |p: Punto| Punto {
                x: ((p.x - marco.x) as f32 / e) as i32,
                y: ((p.y - marco.y) as f32 / e) as i32,
            };
            let solo_filas: Vec<Fila> = filas.iter().map(|(_, f)| f.clone()).collect();
            match evento {
                EventoOverlay::BotonPulsado(p) => {
                    let golpe = golpe_en(
                        local(p),
                        ANCHO as f32,
                        ALTO as f32,
                        nombres_pestanas.len(),
                        &solo_filas,
                        estado.desplazamiento,
                    );
                    // Cualquier clic termina una captura en curso: si se
                    // estaba esperando una tecla y se pulsa otra cosa, es
                    // que se cambio de idea.
                    estado.capturando = None;
                    match golpe {
                        Some(Golpe::Pestana(i)) => {
                            estado.pestana = i;
                            estado.desplazamiento = 0;
                        }
                        Some(Golpe::CapturarAtajo(i)) => estado.capturando = Some(i),
                        Some(Golpe::Alternar(i)) => {
                            if let Some((clave, _)) = filas.get(i) {
                                aplicar_interruptor(&mut ajustes, *clave);
                                cambiado = true;
                            }
                        }
                        Some(Golpe::Elegir { fila, cual }) => {
                            if let Some((clave, _)) = filas.get(fila) {
                                aplicar_opcion(&mut ajustes, *clave, cual);
                                cambiado = true;
                            }
                        }
                        Some(Golpe::Menos(i)) | Some(Golpe::Mas(i)) => {
                            if let Some((clave, f)) = filas.get(i) {
                                let subir = matches!(golpe, Some(Golpe::Mas(_)));
                                if let Some(n) = numero_tras(&f.control, subir) {
                                    aplicar_numero(&mut ajustes, *clave, n);
                                    cambiado = true;
                                }
                            }
                        }
                        Some(Golpe::Cerrar) => cerrar = true,
                        Some(Golpe::AbrirFichero) => {
                            if let Err(err) =
                                pixpin_shell::abrir::abrir(&ubicacion.fichero_ajustes())
                            {
                                tracing::warn!(?err, "no se pudo abrir el fichero de ajustes");
                            }
                        }
                        None => {}
                    }
                    version += 1;
                }
                EventoOverlay::RatonMovido(p) => {
                    let ahora = golpe_en(
                        local(p),
                        ANCHO as f32,
                        ALTO as f32,
                        nombres_pestanas.len(),
                        &solo_filas,
                        estado.desplazamiento,
                    );
                    if ahora != resaltado {
                        resaltado = ahora;
                        version += 1;
                    }
                }
                EventoOverlay::Rueda(delta) => {
                    // Rueda hacia arriba es delta positivo, y subir la
                    // lista es RESTAR desplazamiento.
                    let muescas = delta / 120;
                    estado.desplazamiento = limitar_desplazamiento(
                        estado.desplazamiento - muescas * PASO_RUEDA,
                        filas.len(),
                        ALTO as f32,
                    );
                    version += 1;
                }
                EventoOverlay::Tecla { vk, .. } => {
                    if let Some(i) = estado.capturando {
                        match vk {
                            // Escape cancela la captura, NO cierra la
                            // ventana: mientras se graba, Escape es «dejalo
                            // como estaba».
                            0x1B => estado.capturando = None,
                            // Suprimir o Retroceso: quitar el atajo.
                            0x2E | 0x08 => {
                                if let Some((Clave::Comando(c), _)) = filas.get(i) {
                                    enlaces.poner(*c, None);
                                    ajustes.comandos = enlaces.a_tabla();
                                    cambiado = true;
                                }
                                estado.capturando = None;
                            }
                            // Los modificadores solos no cuentan: se espera
                            // a la tecla final.
                            0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0..=0xA5 => {}
                            _ => {
                                let modificadores = pixpin_shell::modificadores_pulsados();
                                if let Some(a) = Atajo::desde_teclado(vk, modificadores) {
                                    if let Some((Clave::Comando(c), _)) = filas.get(i) {
                                        enlaces.poner(*c, Some(a));
                                        ajustes.comandos = enlaces.a_tabla();
                                        cambiado = true;
                                    }
                                    estado.capturando = None;
                                }
                                // Una tecla que no vale como atajo (Intro,
                                // una flecha, una letra sin modificador) se
                                // ignora y se sigue esperando: el usuario ve
                                // que no ha entrado y lo intenta de otra
                                // forma.
                            }
                        }
                    } else if vk == 0x1B {
                        cerrar = true;
                    }
                    version += 1;
                }
                EventoOverlay::Cerrar => cerrar = true,
                _ => {}
            }
            // Las filas pueden haber cambiado con el golpe: se rehacen
            // para el siguiente evento de esta misma vuelta.
            filas = filas_de(estado.pestana, &ajustes, &enlaces, textos);
        }
        if cerrar {
            break;
        }

        let clave_pintado = (estado.clone(), resaltado, version);
        if pintado.as_ref() != Some(&clave_pintado) {
            pintado = Some(clave_pintado);
            pintar(
                &motor,
                &superficie,
                e,
                &nombres_pestanas,
                &filas,
                &estado,
                resaltado,
                textos,
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(8));
    }

    ventana.ocultar();
    pixpin_shell::overlay::bombear_pendientes();

    if !cambiado {
        return Ok(None);
    }
    // Conservando los comentarios: es la razon de que exista
    // `guardar_conservando`, y aqui es donde se juega.
    pixpin_store::ajustes::guardar_conservando(ubicacion, &ajustes)
        .context("no se pudieron guardar los ajustes")?;
    tracing::info!("ajustes guardados desde la ventana");
    Ok(Some(ajustes))
}

/// Las filas de una pestana, con la clave de lo que hay detras de cada
/// una.
/// El segundo idioma de las notas de voz que se ofrece (B6): ninguno y los
/// tres que el usuario mezcla. El codigo va al TOML; el rotulo, del catalogo.
/// Un codigo escrito a mano en el TOML que no este aqui sale como «ninguno»
/// en la ventana pero se respeta mientras no se toque.
const SEGUNDOS_IDIOMAS: [(&str, &str); 4] = [
    ("", "ajustes-voz-ninguno"),
    ("en", "ajustes-voz-ingles"),
    ("es", "ajustes-voz-espanol"),
    ("pt", "ajustes-voz-portugues"),
];

fn filas_de(pestana: usize, a: &Ajustes, enlaces: &Enlaces, t: &Catalogo) -> Vec<(Clave, Fila)> {
    match pestana {
        0 => CATALOGO
            .iter()
            .map(|d| {
                let atajo = enlaces.atajo_de(d.comando);
                // Choca si otro comando tiene exactamente el mismo atajo.
                // Se mira aqui, con todos delante, porque una fila sola no
                // puede saberlo.
                let choca = atajo.is_some_and(|mio| {
                    CATALOGO
                        .iter()
                        .any(|o| o.comando != d.comando && enlaces.atajo_de(o.comando) == Some(mio))
                });
                (
                    Clave::Comando(d.comando),
                    Fila {
                        etiqueta: t.t(d.clave_titulo),
                        control: Control::Atajo {
                            texto: atajo
                                .map(|x| x.to_string())
                                .unwrap_or_else(|| t.t("ajustes-sin-atajo")),
                            choca,
                        },
                    },
                )
            })
            .collect(),
        1 => vec![
            (
                Clave::Idioma,
                Fila {
                    etiqueta: t.t("ajustes-idioma"),
                    control: Control::Opcion {
                        opciones: vec![
                            t.t("ajustes-idioma-sistema"),
                            "Español".to_string(),
                            "English".to_string(),
                        ],
                        elegida: match a.idioma {
                            PreferenciaIdioma::Sistema => 0,
                            PreferenciaIdioma::Espanol => 1,
                            PreferenciaIdioma::Ingles => 2,
                        },
                    },
                },
            ),
            (
                Clave::Arranque,
                Fila {
                    etiqueta: t.t("ajustes-arranque"),
                    control: Control::Interruptor(a.arranque_con_windows),
                },
            ),
            // M1: el tema Cosmos del movil (`tema_cosmos.rs`).
            (
                Clave::TemaCosmos,
                Fila {
                    etiqueta: t.t("ajustes-tema-cosmos"),
                    control: Control::Interruptor(a.tema_cosmos),
                },
            ),
            (
                Clave::Color,
                Fila {
                    etiqueta: t.t("ajustes-color"),
                    control: Control::Opcion {
                        opciones: vec!["Hex".into(), "RGB".into(), "HSL".into()],
                        elegida: match a.formato_color {
                            FormatoColor::Hex => 0,
                            FormatoColor::Rgb => 1,
                            FormatoColor::Hsl => 2,
                        },
                    },
                },
            ),
            (
                Clave::Nivel,
                Fila {
                    etiqueta: t.t("ajustes-nivel"),
                    control: Control::Opcion {
                        opciones: vec![
                            t.t("ajustes-nivel-auto"),
                            t.t("ajustes-nivel-completo"),
                            t.t("ajustes-nivel-ligero"),
                        ],
                        elegida: match a.rendimiento.nivel {
                            PreferenciaNivel::Auto => 0,
                            PreferenciaNivel::Completo => 1,
                            PreferenciaNivel::Ligero => 2,
                        },
                    },
                },
            ),
            // Aligerar los PDF al meterlos al chat (`crate::aligerar`). Dos
            // filas y no una con «No» entre los niveles, como el movil: cinco
            // opciones no caben en la zona de control.
            (
                Clave::PdfAligerar,
                Fila {
                    etiqueta: t.t("ajustes-pdf-aligerar"),
                    control: Control::Interruptor(a.pdf.aligerar_al_entrar),
                },
            ),
            (
                Clave::PdfNivel,
                Fila {
                    etiqueta: t.t("ajustes-pdf-nivel"),
                    control: Control::Opcion {
                        opciones: vec![
                            t.t("ajustes-pdf-nivel-sin-perdida"),
                            t.t("ajustes-pdf-nivel-equilibrado"),
                            t.t("ajustes-pdf-nivel-pequeno"),
                            t.t("ajustes-pdf-nivel-extremo"),
                        ],
                        elegida: match a.pdf.nivel {
                            NivelPdf::SinPerdida => 0,
                            NivelPdf::Equilibrado => 1,
                            NivelPdf::Pequeno => 2,
                            NivelPdf::Extremo => 3,
                        },
                    },
                },
            ),
            // Voz (B6): el segundo idioma de las notas y que hace Whisper
            // con los dos, como `segundoIdiomaDeVoz` y `modoDeIdiomas`.
            (
                Clave::VozSegundo,
                Fila {
                    etiqueta: t.t("ajustes-voz-segundo"),
                    control: Control::Opcion {
                        opciones: SEGUNDOS_IDIOMAS.iter().map(|(_, k)| t.t(k)).collect(),
                        elegida: SEGUNDOS_IDIOMAS
                            .iter()
                            .position(|(c, _)| *c == a.voz.segundo_idioma)
                            .unwrap_or(0),
                    },
                },
            ),
            (
                Clave::VozModo,
                Fila {
                    etiqueta: t.t("ajustes-voz-modo"),
                    control: Control::Opcion {
                        opciones: vec![
                            t.t("ajustes-voz-modo-cada-uno"),
                            t.t("ajustes-voz-modo-todo-en-uno"),
                        ],
                        elegida: match a.voz.modo_de_idiomas {
                            ModoDeIdiomas::CadaUno => 0,
                            ModoDeIdiomas::TodoEnUno => 1,
                        },
                    },
                },
            ),
        ],
        2 => vec![
            (
                Clave::RetardoCaptura,
                Fila {
                    etiqueta: t.t("ajustes-retardo-captura"),
                    control: Control::Numero {
                        valor: a.retardo_captura_s,
                        minimo: 0,
                        maximo: 30,
                        paso: 1,
                    },
                },
            ),
            (
                Clave::LimiteScroll,
                Fila {
                    etiqueta: t.t("ajustes-limite-scroll"),
                    control: Control::Numero {
                        valor: a.limite_scroll_px,
                        minimo: 2_000,
                        maximo: 100_000,
                        paso: 2_000,
                    },
                },
            ),
            (
                Clave::GifRitmo,
                Fila {
                    etiqueta: t.t("ajustes-gif-ritmo"),
                    control: Control::Numero {
                        valor: a.gif.por_segundo,
                        minimo: 5,
                        maximo: 30,
                        paso: 5,
                    },
                },
            ),
            (
                Clave::GifRetardo,
                Fila {
                    etiqueta: t.t("ajustes-gif-retardo"),
                    control: Control::Numero {
                        valor: a.gif.retardo_s,
                        minimo: 0,
                        maximo: 10,
                        paso: 1,
                    },
                },
            ),
        ],
        3 => vec![
            (
                Clave::ImanActivo,
                Fila {
                    etiqueta: t.t("ajustes-iman"),
                    control: Control::Interruptor(a.enganche.activo),
                },
            ),
            (
                Clave::ImanEsquinas,
                Fila {
                    etiqueta: t.t("ajustes-iman-esquinas"),
                    control: Control::Interruptor(a.enganche.esquinas),
                },
            ),
            (
                Clave::ImanMedios,
                Fila {
                    etiqueta: t.t("ajustes-iman-medios"),
                    control: Control::Interruptor(a.enganche.medios),
                },
            ),
            (
                Clave::ImanCentros,
                Fila {
                    etiqueta: t.t("ajustes-iman-centros"),
                    control: Control::Interruptor(a.enganche.centros),
                },
            ),
            (
                Clave::ImanRadio,
                Fila {
                    etiqueta: t.t("ajustes-iman-radio"),
                    control: Control::Numero {
                        valor: a.enganche.radio_px as u32,
                        // Por debajo de 6 px el iman no llega a nada antes
                        // de que el cursor este ya encima; por encima de 40
                        // agarra cosas que no estabas mirando.
                        minimo: 6,
                        maximo: 40,
                        paso: 2,
                    },
                },
            ),
        ],
        // Las herramientas de dibujo que salen en el lienzo, el lector, los
        // pines y el anotador de pantalla: una casilla cada una.
        // Agrupadas como en la barra (`permitidas::secciones_de_ajustes`):
        // un titulo por grupo y sus herramientas debajo. Un grupo con todas
        // apagadas desaparece de la barra.
        PESTANA_HERRAMIENTAS => {
            let mut v = Vec::new();
            for (grupo, nombres) in crate::dibujo::permitidas::secciones_de_ajustes() {
                let titulo = match grupo {
                    Some(g) => t.t(&format!("barra-grupo-{}", g.nombre())),
                    None if v.is_empty() => t.t("ajustes-herramientas-sueltas"),
                    None => String::new(),
                };
                if !titulo.is_empty() {
                    v.push((
                        Clave::Seccion,
                        Fila {
                            etiqueta: titulo,
                            control: Control::Seccion,
                        },
                    ));
                }
                for n in nombres {
                    v.push((
                        Clave::Herramienta(n),
                        Fila {
                            etiqueta: t.t(&format!("herramientas-{n}")),
                            control: Control::Interruptor(a.herramientas.activa(n)),
                        },
                    ));
                }
            }
            v
        }
        _ => vec![],
    }
}

fn aplicar_interruptor(a: &mut Ajustes, clave: Clave) {
    match clave {
        Clave::Arranque => a.arranque_con_windows = !a.arranque_con_windows,
        Clave::TemaCosmos => a.tema_cosmos = !a.tema_cosmos,
        Clave::PdfAligerar => a.pdf.aligerar_al_entrar = !a.pdf.aligerar_al_entrar,
        Clave::ImanActivo => a.enganche.activo = !a.enganche.activo,
        Clave::ImanEsquinas => a.enganche.esquinas = !a.enganche.esquinas,
        Clave::ImanMedios => a.enganche.medios = !a.enganche.medios,
        Clave::ImanCentros => a.enganche.centros = !a.enganche.centros,
        Clave::Herramienta(n) => {
            let activa = a.herramientas.activa(n);
            a.herramientas.poner(n, !activa);
        }
        _ => {}
    }
}

fn aplicar_opcion(a: &mut Ajustes, clave: Clave, cual: usize) {
    match clave {
        Clave::Idioma => {
            a.idioma = match cual {
                1 => PreferenciaIdioma::Espanol,
                2 => PreferenciaIdioma::Ingles,
                _ => PreferenciaIdioma::Sistema,
            }
        }
        Clave::Color => {
            a.formato_color = match cual {
                1 => FormatoColor::Rgb,
                2 => FormatoColor::Hsl,
                _ => FormatoColor::Hex,
            }
        }
        Clave::Nivel => {
            a.rendimiento.nivel = match cual {
                1 => PreferenciaNivel::Completo,
                2 => PreferenciaNivel::Ligero,
                _ => PreferenciaNivel::Auto,
            }
        }
        Clave::VozSegundo => {
            a.voz.segundo_idioma = SEGUNDOS_IDIOMAS.get(cual).map(|(c, _)| c.to_string()).unwrap_or_default();
        }
        Clave::VozModo => {
            a.voz.modo_de_idiomas = if cual == 1 {
                ModoDeIdiomas::TodoEnUno
            } else {
                ModoDeIdiomas::CadaUno
            };
        }
        Clave::PdfNivel => {
            a.pdf.nivel = match cual {
                0 => NivelPdf::SinPerdida,
                2 => NivelPdf::Pequeno,
                3 => NivelPdf::Extremo,
                _ => NivelPdf::Equilibrado,
            }
        }
        _ => {}
    }
}

fn aplicar_numero(a: &mut Ajustes, clave: Clave, n: u32) {
    match clave {
        Clave::RetardoCaptura => a.retardo_captura_s = n,
        Clave::LimiteScroll => a.limite_scroll_px = n,
        Clave::GifRitmo => a.gif.por_segundo = n,
        Clave::GifRetardo => a.gif.retardo_s = n,
        Clave::ImanRadio => a.enganche.radio_px = n as f32,
        _ => {}
    }
}

fn a_rectf(r: Recta, e: f32) -> RectF {
    RectF {
        x: r.x * e,
        y: r.y * e,
        ancho: r.ancho * e,
        alto: r.alto * e,
    }
}

#[allow(clippy::too_many_arguments)]
fn pintar(
    motor: &MotorRender,
    superficie: &Superficie,
    e: f32,
    nombres_pestanas: &[String],
    filas: &[(Clave, Fila)],
    estado: &Estado,
    resaltado: Option<Golpe>,
    textos: &Catalogo,
) {
    let Ok(destino) = superficie.empezar(motor) else {
        return;
    };
    let _ = motor.dibujar(&destino, |p| {
        dibujar(p, e, nombres_pestanas, filas, estado, resaltado, textos)
    });
    let _ = superficie.presentar();
}

/// Lo que se ve en la ventana, en un pintor cualquiera: el de la ventana o,
/// en las pruebas, uno fuera de pantalla para dejar la muestra en PNG.
fn dibujar(
    p: &pixpin_render::Pintor,
    e: f32,
    nombres_pestanas: &[String],
    filas: &[(Clave, Fila)],
    estado: &Estado,
    resaltado: Option<Golpe>,
    textos: &Catalogo,
) {
    let (ancho, alto) = (ANCHO as f32, ALTO as f32);
    let sin_atajo = textos.t("ajustes-sin-atajo");
    {
        p.limpiar_transparente();
        p.rellenar_redondeado(
            RectF {
                x: 0.0,
                y: 0.0,
                ancho: ancho * e,
                alto: alto * e,
            },
            10.0 * e,
            FONDO,
        );
        let centrar = |texto: &str, r: RectF, tam: f32, color: Color| {
            let (w, h) = p.medir_texto(texto, tam);
            p.texto(
                texto,
                r.x + (r.ancho - w) / 2.0,
                r.y + (r.alto - h) / 2.0,
                tam,
                color,
            );
        };
        let tam = 13.0 * e;

        // Las filas primero; las pestanas y el pie van DESPUES, con fondo
        // opaco, para tapar lo que se haya desplazado por debajo de ellos.
        let lista_arriba = PESTANAS_ALTO;
        let lista_abajo = PESTANAS_ALTO + alto_de_lista(alto);
        for (i, (clave, fila)) in filas.iter().enumerate() {
            let r = rect_de_fila(i, estado.desplazamiento, ancho);
            if r.y + r.alto < lista_arriba || r.y > lista_abajo {
                continue;
            }
            p.rellenar_redondeado(
                RectF {
                    x: MARGEN * e,
                    y: (r.y + r.alto - 1.0) * e,
                    ancho: (ancho - 2.0 * MARGEN) * e,
                    alto: 1.0 * e,
                },
                0.0,
                SEPARADOR,
            );
            let (_, h) = p.medir_texto(&fila.etiqueta, tam);
            // Un titulo de seccion va en el color de acento y lo que cuelga
            // de el, un poco metido (como el iman del movil).
            let (x_etiqueta, color_etiqueta) = match (&fila.control, clave) {
                (Control::Seccion, _) => (MARGEN, CAJA_ACTIVA),
                (_, Clave::Herramienta(_)) => (MARGEN + 12.0, TINTA),
                _ => (MARGEN, TINTA),
            };
            p.texto(
                &fila.etiqueta,
                x_etiqueta * e,
                r.y * e + (FILA_ALTO * e - h) / 2.0,
                tam,
                color_etiqueta,
            );
            let zona = zona_de_control(r);
            match &fila.control {
                Control::Seccion => {}
                Control::Atajo { texto, choca } => {
                    let capturando = estado.capturando == Some(i);
                    let caja = a_rectf(zona, e);
                    let fondo = if capturando { CAJA_ACTIVA } else { CAJA };
                    p.rellenar_redondeado(caja, 6.0 * e, fondo);
                    let (rotulo, color) = if capturando {
                        (textos.t("ajustes-pulsa-combinacion"), TINTA)
                    } else if *choca {
                        (format!("{texto}  ·  {}", textos.t("ajustes-choca")), AVISO)
                    } else if *texto == sin_atajo {
                        (texto.clone(), TENUE)
                    } else {
                        (texto.clone(), TINTA)
                    };
                    centrar(&rotulo, caja, tam, color);
                }
                Control::Interruptor(activo) => {
                    // Una pildora con la bolita a un lado u otro.
                    let pildora = Recta {
                        x: zona.x + zona.ancho - 46.0,
                        y: zona.y + 2.0,
                        ancho: 46.0,
                        alto: BOTON_LADO - 4.0,
                    };
                    let caja = a_rectf(pildora, e);
                    p.rellenar_redondeado(
                        caja,
                        caja.alto / 2.0,
                        if *activo { CAJA_ACTIVA } else { CAJA },
                    );
                    let bola = (BOTON_LADO - 8.0) * e;
                    let x = if *activo {
                        caja.x + caja.ancho - bola - 2.0 * e
                    } else {
                        caja.x + 2.0 * e
                    };
                    p.rellenar_redondeado(
                        RectF {
                            x,
                            y: caja.y + (caja.alto - bola) / 2.0,
                            ancho: bola,
                            alto: bola,
                        },
                        bola / 2.0,
                        TINTA,
                    );
                }
                Control::Opcion { opciones, elegida } => {
                    for (cual, caja) in cajas_de_opcion(zona, opciones.len())
                        .into_iter()
                        .enumerate()
                    {
                        let r = a_rectf(caja, e);
                        let es = cual == *elegida;
                        let sobre = resaltado == Some(Golpe::Elegir { fila: i, cual });
                        p.rellenar_redondeado(
                            r,
                            5.0 * e,
                            if es {
                                CAJA_ACTIVA
                            } else if sobre {
                                CAJA
                            } else {
                                Color { a: 0.0, ..CAJA }
                            },
                        );
                        centrar(&opciones[cual], r, tam, if es { TINTA } else { TENUE });
                    }
                }
                Control::Numero { valor, .. } => {
                    let (menos, mas) = botones_de_numero(zona);
                    for (caja, rotulo, golpe) in
                        [(menos, "-", Golpe::Menos(i)), (mas, "+", Golpe::Mas(i))]
                    {
                        let r = a_rectf(caja, e);
                        let sobre = resaltado == Some(golpe);
                        p.rellenar_redondeado(r, 5.0 * e, if sobre { CAJA_ACTIVA } else { CAJA });
                        centrar(rotulo, r, 16.0 * e, TINTA);
                    }
                    let numero = valor.to_string();
                    let (w, h) = p.medir_texto(&numero, tam);
                    p.texto(
                        &numero,
                        (menos.x - 10.0) * e - w,
                        zona.y * e + (zona.alto * e - h) / 2.0,
                        tam,
                        TINTA,
                    );
                }
            }
        }

        // Pestanas, con fondo opaco.
        p.rellenar_redondeado(
            RectF {
                x: 0.0,
                y: 0.0,
                ancho: ancho * e,
                alto: PESTANAS_ALTO * e,
            },
            10.0 * e,
            FONDO_BARRA,
        );
        for (i, caja) in pestanas(ancho, nombres_pestanas.len())
            .into_iter()
            .enumerate()
        {
            let r = a_rectf(caja, e);
            let activa = i == estado.pestana;
            if activa {
                // Una linea debajo, no un relleno: senala sin gritar.
                p.rellenar_redondeado(
                    RectF {
                        x: r.x + 12.0 * e,
                        y: r.y + r.alto - 3.0 * e,
                        ancho: r.ancho - 24.0 * e,
                        alto: 3.0 * e,
                    },
                    1.5 * e,
                    CAJA_ACTIVA,
                );
            }
            centrar(
                &nombres_pestanas[i],
                r,
                14.0 * e,
                if activa { TINTA } else { TENUE },
            );
        }

        // El pie, con fondo opaco.
        p.rellenar_redondeado(
            RectF {
                x: 0.0,
                y: (alto - PIE_ALTO) * e,
                ancho: ancho * e,
                alto: PIE_ALTO * e,
            },
            10.0 * e,
            FONDO_BARRA,
        );
        let (abrir, cerrar) = botones_del_pie(ancho, alto);
        for (caja, rotulo, golpe, primario) in [
            (
                abrir,
                textos.t("ajustes-abrir-fichero"),
                Golpe::AbrirFichero,
                false,
            ),
            (cerrar, textos.t("ajustes-cerrar"), Golpe::Cerrar, true),
        ] {
            let r = a_rectf(caja, e);
            let sobre = resaltado == Some(golpe);
            p.rellenar_redondeado(
                r,
                6.0 * e,
                if primario {
                    CAJA_ACTIVA
                } else if sobre {
                    CAJA
                } else {
                    Color { a: 0.0, ..CAJA }
                },
            );
            centrar(&rotulo, r, tam, if primario { TINTA } else { TENUE });
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn apagar_el_iman_desde_los_ajustes_lo_apaga_de_verdad() {
        let mut a = Ajustes::default();
        assert!(a.enganche.activo);
        aplicar_interruptor(&mut a, Clave::ImanActivo);
        assert!(!a.enganche.activo);

        assert!(a.enganche.centros);
        aplicar_interruptor(&mut a, Clave::ImanCentros);
        assert!(!a.enganche.centros);

        aplicar_numero(&mut a, Clave::ImanRadio, 22);
        assert_eq!(a.enganche.radio_px, 22.0);
    }

    #[test]
    fn el_arranque_sigue_funcionando_tras_pasar_el_if_a_match() {
        // `aplicar_interruptor` era un `if` de un solo caso y pasa a ser un
        // `match`: esta prueba es la red de esa conversion.
        let mut a = Ajustes::default();
        let antes = a.arranque_con_windows;
        aplicar_interruptor(&mut a, Clave::Arranque);
        assert_eq!(a.arranque_con_windows, !antes);
    }

    #[test]
    fn la_pestana_de_herramientas_trae_una_casilla_por_herramienta_y_apaga_de_verdad() {
        let mut a = Ajustes::default();
        let (enlaces, _) = Enlaces::de_ajustes(&a);
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let filas = filas_de(PESTANA_HERRAMIENTAS, &a, &enlaces, &textos);
        // Una casilla por herramienta, TODAS (las nuevas tambien), y un
        // titulo por grupo de la barra mas el de las sueltas.
        let casillas = filas
            .iter()
            .filter(|(c, _)| matches!(c, Clave::Herramienta(_)))
            .count();
        assert_eq!(casillas, pixpin_store::herramientas::NOMBRES.len());
        let titulos: Vec<&str> = filas
            .iter()
            .filter(|(c, _)| *c == Clave::Seccion)
            .map(|(_, f)| f.etiqueta.as_str())
            .collect();
        assert_eq!(titulos.len(), 1 + pixpin_ui::GrupoBarra::TODOS.len());
        assert_eq!(titulos[0], "Siempre a la vista");
        assert!(titulos.contains(&"Formas") && titulos.contains(&"Compartir e imprimir"));
        // Soldar va bajo «Arreglar lo trazado», no suelta al final.
        let i_arreglar = filas.iter().position(|(_, f)| f.etiqueta == "Arreglar lo trazado").unwrap();
        let i_nudo = filas.iter().position(|(c, _)| *c == Clave::Herramienta("nudo")).unwrap();
        assert!(i_nudo > i_arreglar && i_nudo <= i_arreglar + 4);
        // De fabrica, todas encendidas; los titulos no se pulsan.
        assert!(filas.iter().all(|(c, f)| match c {
            Clave::Seccion => f.control == pixpin_ui::ajustes::Control::Seccion,
            _ => f.control == pixpin_ui::ajustes::Control::Interruptor(true),
        }));
        // Cada una con su titulo traducido, no con la clave.
        for (_, f) in &filas {
            assert!(
                !f.etiqueta.starts_with("herramientas-") && !f.etiqueta.starts_with("barra-"),
                "{}",
                f.etiqueta
            );
        }
        let (lazo, _) = filas
            .iter()
            .find(|(c, _)| *c == Clave::Herramienta("lazo"))
            .expect("fila del lazo");
        aplicar_interruptor(&mut a, *lazo);
        assert!(!a.herramientas.activa("lazo"));
        // Caso negativo: las demas siguen.
        assert!(a.herramientas.activa("lapiz"));
        let filas = filas_de(PESTANA_HERRAMIENTAS, &a, &enlaces, &textos);
        let (_, f) = filas
            .iter()
            .find(|(c, _)| *c == Clave::Herramienta("lazo"))
            .unwrap();
        assert_eq!(f.control, pixpin_ui::ajustes::Control::Interruptor(false));
        // Y se vuelve a encender.
        aplicar_interruptor(&mut a, *lazo);
        assert!(a.herramientas.activa("lazo"));
    }

    #[test]
    fn la_pestana_de_dibujo_trae_las_cinco_filas_del_iman() {
        // `Catalogo::nuevo` no toca disco: el .ftl esta embebido con
        // `include_str!` en tiempo de compilacion, asi que construirlo aqui
        // es barato y esta prueba puede vigilar `filas_de` de verdad.
        let a = Ajustes::default();
        let (enlaces, _) = Enlaces::de_ajustes(&a);
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);

        let filas = filas_de(3, &a, &enlaces, &textos);
        let claves: Vec<Clave> = filas.iter().map(|(c, _)| *c).collect();
        assert_eq!(
            claves,
            vec![
                Clave::ImanActivo,
                Clave::ImanEsquinas,
                Clave::ImanMedios,
                Clave::ImanCentros,
                Clave::ImanRadio,
            ]
        );
    }

    /// **Muestra de la pestana de herramientas**, arriba y bajada hasta las
    /// nuevas, con dos apagadas (el lazo y compartir). Se deja en
    /// `PIXPIN_MUESTRAS` (o la carpeta temporal) para mirarla.
    #[test]
    fn muestra_de_la_pestana_de_herramientas_agrupada() {
        let mut a = Ajustes::default();
        a.herramientas.poner("lazo", false);
        a.herramientas.poner("compartir", false);
        let (enlaces, _) = Enlaces::de_ajustes(&a);
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let filas = filas_de(PESTANA_HERRAMIENTAS, &a, &enlaces, &textos);
        let nombres: Vec<String> = [
            "ajustes-pestana-atajos",
            "ajustes-pestana-general",
            "ajustes-pestana-captura",
            "ajustes-pestana-dibujo",
            "ajustes-pestana-herramientas",
        ]
        .iter()
        .map(|k| textos.t(k))
        .collect();
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let destino =
            pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, d.d3d(), ANCHO, ALTO)
                .expect("textura");
        let dir = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        std::fs::create_dir_all(&dir).expect("carpeta");
        let tope = pixpin_ui::ajustes::desplazamiento_maximo(filas.len(), ALTO as f32);
        for (nombre, bajado) in [("arriba", 0), ("mitad", tope / 2), ("abajo", tope)] {
            let estado = Estado {
                pestana: PESTANA_HERRAMIENTAS,
                desplazamiento: bajado,
                capturando: None,
            };
            motor
                .dibujar(&destino.destino, |p| {
                    dibujar(p, 1.0, &nombres, &filas, &estado, None, &textos)
                })
                .expect("pinta");
            let (ancho, alto, pixeles) = destino.leer_rgba().expect("lee");
            // Hay algo pintado (el fondo de la ventana no es transparente).
            assert!(pixeles.chunks(4).filter(|px| px[3] > 0).count() > 1000);
            let png = pixpin_codec::codificar_png(&pixpin_codec::ImagenRgba {
                ancho,
                alto,
                pixeles,
            })
            .expect("png");
            std::fs::write(dir.join(format!("ajustes-herramientas-{nombre}.png")), png)
                .expect("escribe");
        }
    }
}
