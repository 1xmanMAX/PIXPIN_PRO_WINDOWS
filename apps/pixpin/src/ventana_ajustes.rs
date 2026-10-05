//! La ventana de ajustes (v2, maqueta `Ajustes2`).
//!
//! A la izquierda, el buscador y las secciones; a la derecha, las opciones
//! de la seccion en tarjetas, cada una con su explicacion debajo. Un punto
//! azul marca lo que no esta como de fabrica, y al lado sale el boton de
//! volver a fabrica; arriba, «Restablecer <seccion>» hace lo mismo con
//! todas. Lo que se toca puede deshacerse con Ctrl+Z (o «Deshacer») sin
//! pedir confirmacion a nada.
//!
//! Se trabaja sobre una COPIA de los ajustes y se guarda al cerrar, sin
//! borrar los comentarios del fichero (`guardar_conservando`). Al cerrar se
//! aplica tambien lo que toca a Windows (arrancar con el, «Abrir con») y la
//! caducidad de las capturas; lo demas lo aplica quien llama
//! (`main.rs`: atajos, tema, herramientas…).
//!
//! La logica de donde cae cada cosa vive en `pixpin_ui::ajustes`, pura y
//! probada. Aqui estan las filas de cada seccion, que hace cada golpe y el
//! pintado.

use std::cell::Cell;

use anyhow::{Context, Result};
use fluent_bundle::FluentArgs;
use pixpin_geom::{Punto, Rect};
use pixpin_render::icono::material as mi;
use pixpin_render::icono::{Icono, Pintura, TrazoIcono};
use pixpin_render::letras::{Letra, SIN_PARTIR};
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::Atajo;
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::ajustes::{
    Ajustes, EsquinaPila, FormatoColor, ModoDeIdiomas, NivelPdf, PreferenciaIdioma,
    PreferenciaNivel, Suavizado,
};
use pixpin_store::comandos::{CATALOGO, Comando, Enlaces};
use pixpin_store::herramientas::SITIOS;
use pixpin_store::{Catalogo, Ubicacion};
use pixpin_ui::ajustes::{
    Control, Estado, Fila, Foco, Golpe, NAV_ANCHO, Parte, Recta, ancho_de_textos, borrar_busqueda,
    boton_restablecer_seccion, botones_del_pie, buscador, coincide, contenido, escribir, golpe_en,
    limitar_desplazamiento, numero_tras, partes_de_fila, rect_de_fila, rect_de_seccion,
};

/// Medidas de la ventana en pixeles logicos (se encogen si el monitor no
/// da para tanto).
const ANCHO: f32 = 1000.0;
const ALTO: f32 = 680.0;
/// Cuantos pixeles baja la lista por cada muesca de la rueda.
const PASO_RUEDA: i32 = 60;
/// Cuantos pasos guarda «Deshacer».
const HISTORIAL: usize = 100;

// --- Colores de la maqueta (tema oscuro, como el resto de ventanas) -------

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}
const fn blanco(a: f32) -> Color {
    Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a,
    }
}
const FONDO: Color = rgb(0x1C, 0x1C, 0x1E);
const FONDO_NAV: Color = rgb(0x23, 0x23, 0x26);
const TARJETA: Color = rgb(0x2C, 0x2C, 0x2E);
const HUNDIDO: Color = rgb(0x1C, 0x1C, 0x1E);
const SEG_ELEGIDO: Color = rgb(0x48, 0x48, 0x4A);
const TINTA: Color = rgb(0xF5, 0xF5, 0xF7);
const TEXTO: Color = rgb(0xE5, 0xE5, 0xEA);
const TENUE: Color = rgb(0x98, 0x98, 0x9D);
const CHIP_TEXTO: Color = rgb(0xC7, 0xC7, 0xCC);
const AZUL: Color = rgb(0x0A, 0x84, 0xFF);
const AZUL_CLARO: Color = rgb(0x64, 0xD2, 0xFF);
const VERDE: Color = rgb(0x30, 0xD1, 0x58);
const ROJO: Color = rgb(0xFF, 0x45, 0x3A);
const BORDE: Color = blanco(0.10);
const SEPARADOR: Color = blanco(0.06);
const BOTON: Color = blanco(0.08);
const BOTON_SOBRE: Color = blanco(0.13);
const SOBRE: Color = blanco(0.06);

// --- Iconos de Material que no estan en `material.rs` ---------------------

const fn relleno(d: &'static str) -> TrazoIcono {
    TrazoIcono {
        d,
        relleno: Pintura::Actual,
        trazo: Pintura::Nada,
        grosor: 0.0,
        extremo_redondo: false,
        union_redonda: false,
        opacidad: 1.0,
        par_impar: false,
        matriz: None,
        mascara: None,
    }
}
const VISTA: (f32, f32, f32, f32) = (0.0, 0.0, 24.0, 24.0);
/// `keyboard` de Material.
const TECLADO: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M20 5H4c-1.1 0-1.99.9-1.99 2L2 17c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm-9 3h2v2h-2V8zm0 3h2v2h-2v-2zM8 8h2v2H8V8zm0 3h2v2H8v-2zm-1 2H5v-2h2v2zm0-3H5V8h2v2zm9 7H8v-2h8v2zm0-4h-2v-2h2v2zm0-3h-2V8h2v2zm3 3h-2v-2h2v2zm0-3h-2V8h2v2z",
    )],
};
/// `sync` de Material.
const SINCRO: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12 4V1L8 5l4 4V6c3.31 0 6 2.69 6 6 0 1.01-.25 1.97-.7 2.8l1.46 1.46C19.54 15.03 20 13.57 20 12c0-4.42-3.58-8-8-8zm0 14c-3.31 0-6-2.69-6-6 0-1.01.25-1.97.7-2.8L5.24 7.74C4.46 8.97 4 10.43 4 12c0 4.42 3.58 8 8 8v3l4-4-4-4v3z",
    )],
};
/// `apps` de Material.
const APPS: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M4 8h4V4H4v4zm6 12h4v-4h-4v4zm-6 0h4v-4H4v4zm0-6h4v-4H4v4zm6 0h4v-4h-4v4zm6-10v4h4V4h-4zm-6 4h4V4h-4v4zm6 6h4v-4h-4v4zm0 6h4v-4h-4v4z",
    )],
};
/// `tune` de Material.
const MANDOS: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M3 17v2h6v-2H3zM3 5v2h10V5H3zm10 16v-2h8v-2h-8v-2h-2v6h2zM7 9v2H3v2h4v2h2V9H7zm14 4v-2H11v2h10zm-6-4h2V7h4V5h-4V3h-2v6z",
    )],
};
/// `restart_alt` de Material: volver al valor de fabrica.
const RESTABLECER: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12 5V2L8 6l4 4V7c3.31 0 6 2.69 6 6 0 2.97-2.17 5.43-5 5.91v2.02c3.95-.49 7-3.85 7-7.93 0-4.42-3.58-8-8-8zm-6 8c0-1.65.67-3.15 1.76-4.24L6.34 7.34C4.9 8.79 4 10.79 4 13c0 4.08 3.05 7.44 7 7.93v-2.02c-2.83-.48-5-2.94-5-5.91z",
    )],
};
/// `undo` de Material.
const DESHACER: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M12.5 8c-2.65 0-5.05.99-6.9 2.6L2 7v9h9l-3.62-3.62c1.39-1.16 3.16-1.88 5.12-1.88 3.54 0 6.55 2.31 7.6 5.5l2.37-.78C21.08 11.03 17.15 8 12.5 8z",
    )],
};

// --- Las secciones ----------------------------------------------------------

/// Las secciones de la columna, en orden. «Pines» no esta: no tiene aun
/// ningun ajuste propio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seccion {
    General,
    Captura,
    Lienzo,
    Voz,
    Atajos,
    Sincro,
    Apps,
    Apariencia,
    Avanzado,
}

const SECCIONES: [Seccion; 9] = [
    Seccion::General,
    Seccion::Captura,
    Seccion::Lienzo,
    Seccion::Voz,
    Seccion::Atajos,
    Seccion::Sincro,
    Seccion::Apps,
    Seccion::Apariencia,
    Seccion::Avanzado,
];

impl Seccion {
    fn nombre(self) -> &'static str {
        match self {
            Seccion::General => "general",
            Seccion::Captura => "captura",
            Seccion::Lienzo => "lienzo",
            Seccion::Voz => "voz",
            Seccion::Atajos => "atajos",
            Seccion::Sincro => "sincro",
            Seccion::Apps => "apps",
            Seccion::Apariencia => "apariencia",
            Seccion::Avanzado => "avanzado",
        }
    }

    fn titulo(self, t: &Catalogo) -> String {
        t.t(&format!("ajustes2-sec-{}", self.nombre()))
    }

    fn subtitulo(self, t: &Catalogo) -> String {
        t.t(&format!("ajustes2-sec-{}-sub", self.nombre()))
    }

    fn icono(self) -> &'static Icono {
        match self {
            Seccion::General => &mi::SETTINGS,
            Seccion::Captura => &mi::CROP,
            Seccion::Lienzo => &mi::DRAW,
            Seccion::Voz => &mi::MIC,
            Seccion::Atajos => &TECLADO,
            Seccion::Sincro => &SINCRO,
            Seccion::Apps => &APPS,
            Seccion::Apariencia => &mi::PALETTE,
            Seccion::Avanzado => &MANDOS,
        }
    }
}

/// Que hay detras de cada fila: a que campo de `Ajustes` corresponde, o que
/// accion hace. Anadir un ajuste es anadir una variante aqui, su fila en
/// `filas_de_seccion` y sus brazos en `aplicar_*` y `restablecer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Clave {
    Grupo,
    Comando(Comando),
    // General
    Arranque,
    Idioma,
    PdfAligerar,
    PdfNivel,
    // Captura
    RetardoCaptura,
    Color,
    LimiteScroll,
    GifRitmo,
    GifRetardo,
    Carpeta,
    Caducidad,
    Apilar,
    Esquina,
    IconoSegundos,
    /// Un programa de `ignorar_programas`, por su posicion.
    Ignorado(usize),
    AnadirIgnorado,
    /// Una de `regiones`, por su posicion: su atajo se graba aqui.
    Region(usize),
    // Lienzo
    ImanActivo,
    ImanEsquinas,
    ImanMedios,
    ImanCentros,
    ImanRadio,
    Suavizado,
    /// En que sitio se eligen las herramientas de debajo (no es un ajuste:
    /// es por donde se mira, `SITIO_HERRAMIENTAS`).
    SitioHerramientas,
    /// Una herramienta de dibujo, por su nombre estable del TOML.
    Herramienta(&'static str),
    // Voz
    VozSegundo,
    VozModo,
    // Sincronizar
    Presencia,
    LoMioManda,
    // Apps predeterminadas
    AbrirCon,
    Predeterminada,
    // Apariencia
    TemaCosmos,
    // Avanzado
    Nivel,
    MedirFotogramas,
    Ritmo,
    Paneo,
    CorteMinimo,
    Beta,
    Fichero,
}

impl Clave {
    /// Si tiene valor de fabrica al que volver. Las acciones (abrir algo) y
    /// lo que es una lista del usuario (sus programas, sus regiones) no.
    fn restablecible(self) -> bool {
        !matches!(
            self,
            Clave::Grupo
                | Clave::SitioHerramientas
                | Clave::Carpeta
                | Clave::Ignorado(_)
                | Clave::AnadirIgnorado
                | Clave::Region(_)
                | Clave::Predeterminada
                | Clave::Fichero
        )
    }
}

/// El segundo idioma de las notas de voz que se ofrece (B6). Un codigo
/// escrito a mano que no este aqui sale como «ninguno» pero se respeta
/// mientras no se toque.
const SEGUNDOS_IDIOMAS: [(&str, &str); 4] = [
    ("", "ajustes-voz-ninguno"),
    ("en", "ajustes-voz-ingles"),
    ("es", "ajustes-voz-espanol"),
    ("pt", "ajustes-voz-portugues"),
];

/// Las esquinas de la pila, en el orden de los botones.
const ESQUINAS: [(EsquinaPila, &str); 4] = [
    (EsquinaPila::ArribaIzquierda, "↖"),
    (EsquinaPila::ArribaDerecha, "↗"),
    (EsquinaPila::AbajoIzquierda, "↙"),
    (EsquinaPila::AbajoDerecha, "↘"),
];

/// Lo que las filas necesitan saber y no esta en `Ajustes`.
struct Contexto<'a> {
    t: &'a Catalogo,
    /// La carpeta de las capturas, para ensenarla y abrirla.
    carpeta: String,
}

impl Contexto<'_> {
    fn n(&self, clave: &str, n: impl Into<fluent_bundle::FluentValue<'static>>) -> String {
        let mut args = FluentArgs::new();
        args.set("n", n.into());
        self.t.t_args(clave, &args)
    }
}

/// **Distinto del de fabrica**: lo que pone el punto azul.
fn distinto_de_fabrica(clave: Clave, a: &Ajustes) -> bool {
    match clave {
        // Los atajos se comparan por lo que valen, no por como estan
        // escritos: la tabla `[comandos]` puede nombrar un atajo igual al de
        // fabrica, o la vieja `[atajos]` decir lo mismo que la nueva.
        Clave::Comando(c) => Enlaces::de_ajustes(a).0.atajo_de(c) != Enlaces::default().atajo_de(c),
        _ if !clave.restablecible() => false,
        _ => {
            let mut copia = a.clone();
            restablecer(clave, &mut copia);
            copia != *a
        }
    }
}

/// Una fila de opcion, con el punto azul ya decidido.
fn op(
    a: &Ajustes,
    clave: Clave,
    etiqueta: String,
    ayuda: String,
    control: Control,
) -> (Clave, Fila) {
    (
        clave,
        Fila {
            etiqueta,
            ayuda,
            control,
            cambiado: distinto_de_fabrica(clave, a),
            quitar: matches!(clave, Clave::Ignorado(_) | Clave::Region(_)),
        },
    )
}

fn grupo(t: &Catalogo, clave: &str) -> (Clave, Fila) {
    (Clave::Grupo, Fila::grupo(t.t(clave)))
}

fn numero(valor: u32, minimo: u32, maximo: u32, paso: u32, texto: String) -> Control {
    Control::Numero {
        valor,
        minimo,
        maximo,
        paso,
        texto,
    }
}

fn opcion(opciones: Vec<String>, elegida: usize) -> Control {
    Control::Opcion { opciones, elegida }
}

/// Las filas de una seccion, con la clave de lo que hay detras de cada una.
fn filas_de_seccion(s: Seccion, a: &Ajustes, cx: &Contexto) -> Vec<(Clave, Fila)> {
    let t = cx.t;
    let tt = |k: &str| t.t(k);
    match s {
        Seccion::General => vec![
            grupo(t, "ajustes2-g-arranque"),
            op(
                a,
                Clave::Arranque,
                tt("ajustes-arranque"),
                tt("ajustes2-ayuda-arranque"),
                Control::Interruptor(a.arranque_con_windows),
            ),
            op(
                a,
                Clave::Idioma,
                tt("ajustes-idioma"),
                tt("ajustes2-ayuda-idioma"),
                opcion(
                    vec![
                        tt("ajustes-idioma-sistema"),
                        "Español".into(),
                        "English".into(),
                    ],
                    match a.idioma {
                        PreferenciaIdioma::Sistema => 0,
                        PreferenciaIdioma::Espanol => 1,
                        PreferenciaIdioma::Ingles => 2,
                    },
                ),
            ),
            grupo(t, "ajustes2-g-pdf"),
            op(
                a,
                Clave::PdfAligerar,
                tt("ajustes-pdf-aligerar"),
                tt("ajustes2-ayuda-pdf-aligerar"),
                Control::Interruptor(a.pdf.aligerar_al_entrar),
            ),
            op(
                a,
                Clave::PdfNivel,
                tt("ajustes2-pdf-nivel"),
                tt("ajustes2-ayuda-pdf-nivel"),
                opcion(
                    vec![
                        tt("ajustes-pdf-nivel-sin-perdida"),
                        tt("ajustes-pdf-nivel-equilibrado"),
                        tt("ajustes-pdf-nivel-pequeno"),
                        tt("ajustes-pdf-nivel-extremo"),
                    ],
                    match a.pdf.nivel {
                        NivelPdf::SinPerdida => 0,
                        NivelPdf::Equilibrado => 1,
                        NivelPdf::Pequeno => 2,
                        NivelPdf::Extremo => 3,
                    },
                ),
            ),
        ],
        Seccion::Captura => {
            let mut v = vec![
                grupo(t, "ajustes2-g-al-capturar"),
                op(
                    a,
                    Clave::RetardoCaptura,
                    tt("ajustes2-retardo"),
                    tt("ajustes2-ayuda-retardo"),
                    numero(
                        a.retardo_captura_s,
                        0,
                        30,
                        1,
                        cx.n("ajustes2-u-s", a.retardo_captura_s),
                    ),
                ),
                op(
                    a,
                    Clave::Color,
                    tt("ajustes2-color"),
                    tt("ajustes2-ayuda-color"),
                    opcion(
                        vec!["HEX".into(), "RGB".into(), "HSL".into()],
                        match a.formato_color {
                            FormatoColor::Hex => 0,
                            FormatoColor::Rgb => 1,
                            FormatoColor::Hsl => 2,
                        },
                    ),
                ),
                op(
                    a,
                    Clave::LimiteScroll,
                    tt("ajustes2-scroll"),
                    tt("ajustes2-ayuda-scroll"),
                    numero(
                        a.limite_scroll_px,
                        2_000,
                        100_000,
                        2_000,
                        cx.n("ajustes2-u-px", a.limite_scroll_px),
                    ),
                ),
                grupo(t, "ajustes2-g-gif"),
                op(
                    a,
                    Clave::GifRitmo,
                    tt("ajustes2-gif-ritmo"),
                    tt("ajustes2-ayuda-gif-ritmo"),
                    numero(a.gif.por_segundo, 5, 30, 5, a.gif.por_segundo.to_string()),
                ),
                op(
                    a,
                    Clave::GifRetardo,
                    tt("ajustes2-gif-retardo"),
                    tt("ajustes2-ayuda-gif-retardo"),
                    numero(
                        a.gif.retardo_s,
                        0,
                        10,
                        1,
                        cx.n("ajustes2-u-s", a.gif.retardo_s),
                    ),
                ),
                grupo(t, "ajustes2-g-guardar"),
                op(
                    a,
                    Clave::Carpeta,
                    tt("ajustes2-carpeta"),
                    cx.carpeta.clone(),
                    Control::Boton(tt("ajustes2-abrir")),
                ),
                op(
                    a,
                    Clave::Caducidad,
                    tt("ajustes2-caducidad"),
                    tt("ajustes2-ayuda-caducidad"),
                    numero(
                        a.capturas.dias_caducidad,
                        0,
                        90,
                        1,
                        if a.capturas.dias_caducidad == 0 {
                            tt("ajustes2-nunca")
                        } else {
                            cx.n("ajustes2-u-dias", a.capturas.dias_caducidad)
                        },
                    ),
                ),
                grupo(t, "ajustes2-g-pila"),
                op(
                    a,
                    Clave::Apilar,
                    tt("ajustes2-apilar"),
                    tt("ajustes2-ayuda-apilar"),
                    Control::Interruptor(a.capturas.apilar_segundos != 0),
                ),
                op(
                    a,
                    Clave::Esquina,
                    tt("ajustes2-esquina"),
                    tt("ajustes2-ayuda-esquina"),
                    opcion(
                        ESQUINAS.iter().map(|(_, f)| f.to_string()).collect(),
                        ESQUINAS
                            .iter()
                            .position(|(e, _)| *e == a.capturas.esquina)
                            .unwrap_or(3),
                    ),
                ),
                op(
                    a,
                    Clave::IconoSegundos,
                    tt("ajustes2-icono"),
                    tt("ajustes2-ayuda-icono"),
                    numero(
                        a.capturas.icono_segundos,
                        0,
                        60,
                        1,
                        if a.capturas.icono_segundos == 0 {
                            tt("ajustes2-siempre")
                        } else {
                            cx.n("ajustes2-u-s", a.capturas.icono_segundos)
                        },
                    ),
                ),
                grupo(t, "ajustes2-g-ignorados"),
            ];
            for (i, programa) in a.ignorar_programas.iter().enumerate() {
                v.push(op(
                    a,
                    Clave::Ignorado(i),
                    programa.clone(),
                    tt("ajustes2-ayuda-ignorado"),
                    Control::Nada,
                ));
            }
            v.push(op(
                a,
                Clave::AnadirIgnorado,
                tt("ajustes2-anadir-programa"),
                tt("ajustes2-ayuda-anadir-programa"),
                Control::Entrada {
                    marcador: "programa.exe".into(),
                    boton: tt("ajustes2-anadir"),
                },
            ));
            // Las regiones guardadas solo salen si hay alguna: se crean
            // desde la captura, no desde aqui.
            if !a.regiones.is_empty() {
                v.push(grupo(t, "ajustes2-g-regiones"));
                for (i, r) in a.regiones.iter().enumerate() {
                    let mut args = FluentArgs::new();
                    args.set("x", r.x);
                    args.set("y", r.y);
                    args.set("ancho", r.ancho);
                    args.set("alto", r.alto);
                    v.push(op(
                        a,
                        Clave::Region(i),
                        r.nombre.clone(),
                        t.t_args("ajustes2-ayuda-region", &args),
                        Control::Atajo {
                            texto: r
                                .atajo
                                .clone()
                                .filter(|s| !s.trim().is_empty())
                                .unwrap_or_else(|| tt("ajustes-sin-atajo")),
                            choca: false,
                        },
                    ));
                }
            }
            v
        }
        Seccion::Lienzo => {
            let mut v = vec![
                grupo(t, "ajustes2-g-iman"),
                op(
                    a,
                    Clave::ImanActivo,
                    tt("ajustes-iman"),
                    tt("ajustes2-ayuda-iman"),
                    Control::Interruptor(a.enganche.activo),
                ),
                op(
                    a,
                    Clave::ImanEsquinas,
                    tt("ajustes-iman-esquinas"),
                    tt("ajustes2-ayuda-iman-esquinas"),
                    Control::Interruptor(a.enganche.esquinas),
                ),
                op(
                    a,
                    Clave::ImanMedios,
                    tt("ajustes-iman-medios"),
                    tt("ajustes2-ayuda-iman-medios"),
                    Control::Interruptor(a.enganche.medios),
                ),
                op(
                    a,
                    Clave::ImanCentros,
                    tt("ajustes-iman-centros"),
                    tt("ajustes2-ayuda-iman-centros"),
                    Control::Interruptor(a.enganche.centros),
                ),
                op(
                    a,
                    Clave::ImanRadio,
                    tt("ajustes2-iman-radio"),
                    tt("ajustes2-ayuda-iman-radio"),
                    // Por debajo de 6 px el iman no llega a nada; por encima
                    // de 40 agarra cosas que no estabas mirando.
                    numero(
                        a.enganche.radio_px as u32,
                        6,
                        40,
                        2,
                        cx.n("ajustes2-u-px", a.enganche.radio_px as u32),
                    ),
                ),
                grupo(t, "ajustes2-g-trazo"),
                op(
                    a,
                    Clave::Suavizado,
                    tt("ajustes2-suavizado"),
                    tt("ajustes2-ayuda-suavizado"),
                    opcion(
                        vec!["Excalidraw".into(), "Natural".into()],
                        match a.tinta.suavizado {
                            Suavizado::Excalidraw => 0,
                            Suavizado::Natural => 1,
                        },
                    ),
                ),
            ];
            // Las herramientas de dibujo, agrupadas como en la barra
            // (`permitidas::secciones_de_ajustes`). Un grupo con todas
            // apagadas desaparece de la barra.
            //
            // Encima, DONDE: en todos los sitios a la vez o en uno solo (el
            // usuario: «elegir cuantas herramientas mostrar en diferentes
            // situaciones... para que no se muestren siempre todas»). En un
            // sitio solo salen las que ese sitio sabe hacer.
            let sitio = sitio_elegido();
            let mut opciones = vec![tt("ajustes-herramientas-sitio-todos")];
            opciones.extend(
                SITIOS
                    .iter()
                    .map(|s| tt(&format!("ajustes-herramientas-sitio-{s}"))),
            );
            v.push(op(
                a,
                Clave::SitioHerramientas,
                tt("ajustes-herramientas-donde"),
                tt("ajustes-herramientas-donde-ayuda"),
                opcion(opciones, SITIO_HERRAMIENTAS.with(Cell::get)),
            ));
            let anfitrion = sitio.and_then(crate::dibujo::permitidas::Anfitrion::de_sitio);
            for (g, nombres) in crate::dibujo::permitidas::secciones_de_ajustes() {
                let nombres: Vec<&'static str> = nombres
                    .into_iter()
                    .filter(|n| {
                        anfitrion.is_none_or(|anf| {
                            crate::dibujo::permitidas::boton_de_nombre(n)
                                .is_none_or(|b| anf.admite_boton(b))
                        })
                    })
                    .collect();
                if nombres.is_empty() {
                    continue;
                }
                let titulo = match g {
                    Some(g) => t.t(&format!("barra-grupo-{}", g.nombre())),
                    None => tt("ajustes-herramientas-sueltas"),
                };
                v.push((Clave::Grupo, Fila::grupo(titulo)));
                for n in nombres {
                    let activa = match sitio {
                        None => a.herramientas.activa(n),
                        Some(s) => a.herramientas.activa_en(s, n),
                    };
                    v.push(op(
                        a,
                        Clave::Herramienta(n),
                        t.t(&format!("herramientas-{n}")),
                        String::new(),
                        Control::Interruptor(activa),
                    ));
                }
            }
            v
        }
        Seccion::Voz => vec![
            grupo(t, "ajustes2-g-voz"),
            op(
                a,
                Clave::VozSegundo,
                tt("ajustes-voz-segundo"),
                tt("ajustes2-ayuda-voz-segundo"),
                opcion(
                    SEGUNDOS_IDIOMAS.iter().map(|(_, k)| t.t(k)).collect(),
                    SEGUNDOS_IDIOMAS
                        .iter()
                        .position(|(c, _)| *c == a.voz.segundo_idioma)
                        .unwrap_or(0),
                ),
            ),
            op(
                a,
                Clave::VozModo,
                tt("ajustes-voz-modo"),
                tt("ajustes2-ayuda-voz-modo"),
                opcion(
                    vec![
                        tt("ajustes-voz-modo-cada-uno"),
                        tt("ajustes-voz-modo-todo-en-uno"),
                    ],
                    match a.voz.modo_de_idiomas {
                        ModoDeIdiomas::CadaUno => 0,
                        ModoDeIdiomas::TodoEnUno => 1,
                    },
                ),
            ),
        ],
        Seccion::Atajos => {
            let (enlaces, _) = Enlaces::de_ajustes(a);
            let mut v = vec![grupo(t, "ajustes2-g-atajos")];
            for d in CATALOGO {
                let atajo = enlaces.atajo_de(d.comando);
                // Choca si otro comando tiene exactamente el mismo atajo: se
                // mira aqui, con todos delante.
                let choca = atajo.is_some_and(|mio| {
                    CATALOGO
                        .iter()
                        .any(|o| o.comando != d.comando && enlaces.atajo_de(o.comando) == Some(mio))
                });
                v.push(op(
                    a,
                    Clave::Comando(d.comando),
                    t.t(d.clave_titulo),
                    String::new(),
                    Control::Atajo {
                        texto: atajo
                            .map(|x| x.to_string())
                            .unwrap_or_else(|| tt("ajustes-sin-atajo")),
                        choca,
                    },
                ));
            }
            v
        }
        Seccion::Sincro => vec![
            grupo(t, "ajustes2-g-sincro"),
            op(
                a,
                Clave::Presencia,
                tt("ajustes2-presencia"),
                tt("ajustes2-ayuda-presencia"),
                Control::Interruptor(a.sincro.presencia),
            ),
            op(
                a,
                Clave::LoMioManda,
                tt("ajustes2-lo-mio"),
                tt("ajustes2-ayuda-lo-mio"),
                Control::Interruptor(a.sincro.lo_mio_manda),
            ),
        ],
        Seccion::Apps => {
            let mut v = vec![
                grupo(t, "ajustes2-g-apps"),
                op(
                    a,
                    Clave::AbrirCon,
                    tt("ajustes2-abrir-con"),
                    tt("ajustes2-ayuda-abrir-con"),
                    Control::Interruptor(a.abrir_con),
                ),
            ];
            // Sin estar inscrita, Configuracion no tiene pagina de PixPin
            // que abrir: el boton seria un boton muerto.
            if a.abrir_con {
                v.push(op(
                    a,
                    Clave::Predeterminada,
                    tt("ajustes2-predeterminada"),
                    tt("ajustes2-ayuda-predeterminada"),
                    Control::Boton(tt("ajustes2-hacer-predeterminada")),
                ));
            }
            v
        }
        Seccion::Apariencia => vec![
            grupo(t, "ajustes2-g-tema"),
            op(
                a,
                Clave::TemaCosmos,
                tt("ajustes-tema-cosmos"),
                tt("ajustes2-ayuda-cosmos"),
                Control::Interruptor(a.tema_cosmos),
            ),
        ],
        Seccion::Avanzado => {
            let corte = a
                .tinta
                .corte_minimo
                .map(|v| (v * 10.0).round().clamp(1.0, 50.0) as u32)
                .unwrap_or(0);
            let beta = a
                .tinta
                .beta
                .map(|v| (v * 1000.0).round().clamp(1.0, 50.0) as u32)
                .unwrap_or(0);
            vec![
                grupo(t, "ajustes2-g-rendimiento"),
                op(
                    a,
                    Clave::Nivel,
                    tt("ajustes2-nivel"),
                    tt("ajustes2-ayuda-nivel"),
                    opcion(
                        vec![
                            tt("ajustes-nivel-auto"),
                            tt("ajustes-nivel-completo"),
                            tt("ajustes-nivel-ligero"),
                        ],
                        match a.rendimiento.nivel {
                            PreferenciaNivel::Auto => 0,
                            PreferenciaNivel::Completo => 1,
                            PreferenciaNivel::Ligero => 2,
                        },
                    ),
                ),
                op(
                    a,
                    Clave::Paneo,
                    tt("ajustes2-paneo"),
                    tt("ajustes2-ayuda-paneo"),
                    Control::Interruptor(a.rendimiento.paneo_por_composicion),
                ),
                op(
                    a,
                    Clave::Ritmo,
                    tt("ajustes2-ritmo"),
                    tt("ajustes2-ayuda-ritmo"),
                    Control::Interruptor(a.rendimiento.ritmo),
                ),
                op(
                    a,
                    Clave::MedirFotogramas,
                    tt("ajustes2-medir"),
                    tt("ajustes2-ayuda-medir"),
                    Control::Interruptor(a.rendimiento.medir_fotogramas),
                ),
                grupo(t, "ajustes2-g-lapiz"),
                op(
                    a,
                    Clave::CorteMinimo,
                    tt("ajustes2-corte"),
                    tt("ajustes2-ayuda-corte"),
                    numero(
                        corte,
                        0,
                        50,
                        1,
                        if corte == 0 {
                            tt("ajustes2-auto")
                        } else {
                            let mut args = FluentArgs::new();
                            args.set("n", format!("{:.1}", corte as f32 / 10.0));
                            t.t_args("ajustes2-u-hz", &args)
                        },
                    ),
                ),
                op(
                    a,
                    Clave::Beta,
                    tt("ajustes2-beta"),
                    tt("ajustes2-ayuda-beta"),
                    numero(
                        beta,
                        0,
                        50,
                        1,
                        if beta == 0 {
                            tt("ajustes2-auto")
                        } else {
                            format!("{:.3}", beta as f32 / 1000.0)
                        },
                    ),
                ),
                grupo(t, "ajustes2-g-fichero"),
                op(
                    a,
                    Clave::Fichero,
                    tt("ajustes2-fichero"),
                    tt("ajustes2-ayuda-fichero"),
                    Control::Boton(tt("ajustes2-abrir-fichero")),
                ),
            ]
        }
    }
}

/// Las filas que se ven: las de la seccion elegida o, buscando, las que
/// coinciden de TODAS, cada tanda bajo el nombre de su seccion.
fn filas_actuales(estado: &Estado, a: &Ajustes, cx: &Contexto) -> Vec<(Clave, Fila)> {
    if !estado.buscando() {
        let s = SECCIONES[estado.seccion.min(SECCIONES.len() - 1)];
        return filas_de_seccion(s, a, cx);
    }
    let mut v = Vec::new();
    for s in SECCIONES {
        // Cada opcion se busca tambien por el titulo de su grupo: «imán»
        // encuentra las cinco del iman aunque ninguna lo diga.
        let mut grupo_actual = String::new();
        let mut halladas: Vec<(Clave, Fila)> = Vec::new();
        for (c, f) in filas_de_seccion(s, a, cx) {
            if f.es_grupo() {
                grupo_actual = f.etiqueta.clone();
                continue;
            }
            let con_grupo = Fila {
                ayuda: format!("{} {grupo_actual}", f.ayuda),
                ..f.clone()
            };
            if coincide(&estado.busqueda, &con_grupo) {
                halladas.push((c, f));
            }
        }
        if !halladas.is_empty() {
            v.push((Clave::Grupo, Fila::grupo(s.titulo(cx.t))));
            v.extend(halladas);
        }
    }
    v
}

// --- Cambiar los ajustes ----------------------------------------------------

/// Vuelve una clave a su valor de fabrica.
fn restablecer(clave: Clave, a: &mut Ajustes) {
    let d = Ajustes::default();
    match clave {
        Clave::Comando(c) => {
            let (mut e, _) = Enlaces::de_ajustes(a);
            e.poner(c, Enlaces::default().atajo_de(c));
            a.comandos = e.a_tabla();
        }
        Clave::Arranque => a.arranque_con_windows = d.arranque_con_windows,
        Clave::Idioma => a.idioma = d.idioma,
        Clave::PdfAligerar => a.pdf.aligerar_al_entrar = d.pdf.aligerar_al_entrar,
        Clave::PdfNivel => a.pdf.nivel = d.pdf.nivel,
        Clave::RetardoCaptura => a.retardo_captura_s = d.retardo_captura_s,
        Clave::Color => a.formato_color = d.formato_color,
        Clave::LimiteScroll => a.limite_scroll_px = d.limite_scroll_px,
        Clave::GifRitmo => a.gif.por_segundo = d.gif.por_segundo,
        Clave::GifRetardo => a.gif.retardo_s = d.gif.retardo_s,
        Clave::Caducidad => a.capturas.dias_caducidad = d.capturas.dias_caducidad,
        Clave::Apilar => a.capturas.apilar_segundos = d.capturas.apilar_segundos,
        Clave::Esquina => a.capturas.esquina = d.capturas.esquina,
        Clave::IconoSegundos => a.capturas.icono_segundos = d.capturas.icono_segundos,
        Clave::ImanActivo => a.enganche.activo = d.enganche.activo,
        Clave::ImanEsquinas => a.enganche.esquinas = d.enganche.esquinas,
        Clave::ImanMedios => a.enganche.medios = d.enganche.medios,
        Clave::ImanCentros => a.enganche.centros = d.enganche.centros,
        Clave::ImanRadio => a.enganche.radio_px = d.enganche.radio_px,
        Clave::Suavizado => a.tinta.suavizado = d.tinta.suavizado,
        Clave::SitioHerramientas => {}
        Clave::Herramienta(n) => match sitio_elegido() {
            None => a.herramientas.poner(n, true),
            Some(s) => a.herramientas.poner_en(s, n, true),
        },
        Clave::VozSegundo => a.voz.segundo_idioma = d.voz.segundo_idioma,
        Clave::VozModo => a.voz.modo_de_idiomas = d.voz.modo_de_idiomas,
        Clave::Presencia => a.sincro.presencia = d.sincro.presencia,
        Clave::LoMioManda => a.sincro.lo_mio_manda = d.sincro.lo_mio_manda,
        Clave::AbrirCon => a.abrir_con = d.abrir_con,
        Clave::TemaCosmos => a.tema_cosmos = d.tema_cosmos,
        Clave::Nivel => a.rendimiento.nivel = d.rendimiento.nivel,
        Clave::MedirFotogramas => a.rendimiento.medir_fotogramas = d.rendimiento.medir_fotogramas,
        Clave::Ritmo => a.rendimiento.ritmo = d.rendimiento.ritmo,
        Clave::Paneo => a.rendimiento.paneo_por_composicion = d.rendimiento.paneo_por_composicion,
        Clave::CorteMinimo => a.tinta.corte_minimo = d.tinta.corte_minimo,
        Clave::Beta => a.tinta.beta = d.tinta.beta,
        Clave::Grupo
        | Clave::Carpeta
        | Clave::Ignorado(_)
        | Clave::AnadirIgnorado
        | Clave::Region(_)
        | Clave::Predeterminada
        | Clave::Fichero => {}
    }
}

thread_local! {
    /// En que sitio se estan eligiendo las herramientas: 0 = en todos, y
    /// desde 1 el de `SITIOS`. Es por donde se mira, no un ajuste: no se
    /// guarda ni se deshace.
    static SITIO_HERRAMIENTAS: Cell<usize> = const { Cell::new(0) };
}

/// El sitio elegido arriba de las herramientas, o `None` si son todos.
fn sitio_elegido() -> Option<&'static str> {
    SITIO_HERRAMIENTAS.with(|c| c.get().checked_sub(1).and_then(|i| SITIOS.get(i).copied()))
}

/// Enciende o apaga una herramienta: en todos los sitios (`None`) o en uno.
///
/// Encender en un sitio una que estaba apagada en TODOS la deja encendida
/// solo ahi: sale de la lista general y se apaga en los demas sitios, que
/// asi siguen como estaban.
fn alternar_herramienta(a: &mut Ajustes, sitio: Option<&str>, n: &str) {
    let h = &mut a.herramientas;
    match sitio {
        None => {
            let activa = h.activa(n);
            h.poner(n, !activa);
        }
        Some(s) if h.activa_en(s, n) => h.poner_en(s, n, false),
        Some(s) => {
            if !h.activa(n) {
                h.poner(n, true);
                for otro in SITIOS.iter().filter(|o| **o != s) {
                    h.poner_en(otro, n, false);
                }
            }
            h.poner_en(s, n, true);
        }
    }
}

/// «Restablecer <seccion>»: todo lo restablecible de la seccion. Las listas
/// del usuario (programas, regiones) no se tocan.
fn restablecer_seccion(s: Seccion, a: &mut Ajustes, cx: &Contexto) {
    let claves: Vec<Clave> = filas_de_seccion(s, a, cx)
        .into_iter()
        .map(|(c, _)| c)
        .collect();
    for c in claves {
        if c.restablecible() {
            restablecer(c, a);
        }
    }
}

fn aplicar_interruptor(a: &mut Ajustes, clave: Clave) {
    match clave {
        Clave::Arranque => a.arranque_con_windows = !a.arranque_con_windows,
        Clave::TemaCosmos => a.tema_cosmos = !a.tema_cosmos,
        Clave::PdfAligerar => a.pdf.aligerar_al_entrar = !a.pdf.aligerar_al_entrar,
        Clave::Apilar => {
            // Cero apaga la pila; encenderla vuelve a su numero de fabrica
            // (el numero ya no mide nada, ver `Capturas::apilar_segundos`).
            a.capturas.apilar_segundos = if a.capturas.apilar_segundos == 0 {
                Ajustes::default().capturas.apilar_segundos.max(1)
            } else {
                0
            };
        }
        Clave::ImanActivo => a.enganche.activo = !a.enganche.activo,
        Clave::ImanEsquinas => a.enganche.esquinas = !a.enganche.esquinas,
        Clave::ImanMedios => a.enganche.medios = !a.enganche.medios,
        Clave::ImanCentros => a.enganche.centros = !a.enganche.centros,
        Clave::Herramienta(n) => alternar_herramienta(a, sitio_elegido(), n),
        Clave::Presencia => a.sincro.presencia = !a.sincro.presencia,
        Clave::LoMioManda => a.sincro.lo_mio_manda = !a.sincro.lo_mio_manda,
        Clave::AbrirCon => a.abrir_con = !a.abrir_con,
        Clave::MedirFotogramas => a.rendimiento.medir_fotogramas = !a.rendimiento.medir_fotogramas,
        Clave::Ritmo => a.rendimiento.ritmo = !a.rendimiento.ritmo,
        Clave::Paneo => a.rendimiento.paneo_por_composicion = !a.rendimiento.paneo_por_composicion,
        _ => {}
    }
}

fn aplicar_opcion(a: &mut Ajustes, clave: Clave, cual: usize) {
    match clave {
        Clave::SitioHerramientas => SITIO_HERRAMIENTAS.with(|c| c.set(cual.min(SITIOS.len()))),
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
            a.voz.segundo_idioma = SEGUNDOS_IDIOMAS
                .get(cual)
                .map(|(c, _)| c.to_string())
                .unwrap_or_default();
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
        Clave::Esquina => {
            if let Some((e, _)) = ESQUINAS.get(cual) {
                a.capturas.esquina = *e;
            }
        }
        Clave::Suavizado => {
            a.tinta.suavizado = if cual == 1 {
                Suavizado::Natural
            } else {
                Suavizado::Excalidraw
            };
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
        Clave::Caducidad => a.capturas.dias_caducidad = n,
        Clave::IconoSegundos => a.capturas.icono_segundos = n,
        Clave::ImanRadio => a.enganche.radio_px = n as f32,
        // Cero es «Auto»: el valor del propio motor (`Mandos::default`).
        Clave::CorteMinimo => a.tinta.corte_minimo = (n > 0).then(|| n as f32 / 10.0),
        Clave::Beta => a.tinta.beta = (n > 0).then(|| n as f32 / 1000.0),
        _ => {}
    }
}

/// Quita un programa o una region de su lista.
fn quitar(a: &mut Ajustes, clave: Clave) {
    match clave {
        Clave::Ignorado(i) if i < a.ignorar_programas.len() => {
            a.ignorar_programas.remove(i);
        }
        Clave::Region(i) if i < a.regiones.len() => {
            a.regiones.remove(i);
        }
        _ => {}
    }
}

/// Anade un programa a la lista de ignorados. Vacio o repetido (sin
/// distinguir mayusculas ni el `.exe`) no anade nada.
fn anadir_ignorado(a: &mut Ajustes, texto: &str) -> bool {
    let nombre = texto.trim().trim_matches('"').trim();
    if nombre.is_empty() {
        return false;
    }
    let base = |s: &str| {
        let s = s.trim().to_lowercase();
        s.strip_suffix(".exe").map(str::to_string).unwrap_or(s)
    };
    if a.ignorar_programas.iter().any(|p| base(p) == base(nombre)) {
        return false;
    }
    a.ignorar_programas.push(nombre.to_string());
    true
}

/// Pone (o quita, con `None`) el atajo de un comando o de una region.
fn poner_atajo(a: &mut Ajustes, clave: Clave, atajo: Option<Atajo>) {
    match clave {
        Clave::Comando(c) => {
            let (mut e, _) = Enlaces::de_ajustes(a);
            e.poner(c, atajo);
            a.comandos = e.a_tabla();
        }
        Clave::Region(i) => {
            if let Some(r) = a.regiones.get_mut(i) {
                r.atajo = atajo.map(|x| x.to_string());
            }
        }
        _ => {}
    }
}

/// Los ajustes de trabajo y su «Deshacer».
struct Copia {
    a: Ajustes,
    historial: Vec<Ajustes>,
}

impl Copia {
    /// Cambia la copia y apunta como estaba, si de verdad cambio algo.
    fn cambiar(&mut self, f: impl FnOnce(&mut Ajustes)) {
        let antes = self.a.clone();
        f(&mut self.a);
        if self.a != antes {
            self.historial.push(antes);
            if self.historial.len() > HISTORIAL {
                self.historial.remove(0);
            }
        }
    }

    fn deshacer(&mut self) -> bool {
        match self.historial.pop() {
            Some(a) => {
                self.a = a;
                true
            }
            None => false,
        }
    }
}

/// La ruta del ejecutable instalado, la misma que inscribe `main.rs`.
fn ruta_exe() -> Option<std::path::PathBuf> {
    pixpin_shell::entorno::directorio_del_ejecutable()
        .ok()
        .map(|d| d.join("pixpinmax.exe"))
}

/// Lo que se hace al pulsar un `Control::Boton`.
fn pulsar(clave: Clave, a: &Ajustes, ubicacion: &Ubicacion) {
    let resultado = match clave {
        Clave::Carpeta => {
            let carpeta = crate::galeria_capturas::carpeta(ubicacion);
            let _ = std::fs::create_dir_all(&carpeta);
            pixpin_shell::abrir::abrir(&carpeta)
        }
        Clave::Fichero => {
            // Si aun no existe (nadie cambio nada nunca) se crea con lo de
            // fabrica, para que haya algo que abrir.
            let ruta = ubicacion.fichero_ajustes();
            if !ruta.exists()
                && let Err(e) = pixpin_store::ajustes::guardar_conservando(ubicacion, a)
            {
                tracing::warn!(?e, "no se pudo crear el fichero de ajustes");
            }
            pixpin_shell::abrir::abrir(&ruta)
        }
        Clave::Predeterminada => {
            // Si se acaba de encender «Ofrecer PixPin» y aun no se ha
            // guardado, se inscribe ya: sin eso Configuracion no la lista.
            if a.abrir_con
                && let Some(exe) = ruta_exe()
                && let Err(e) = pixpin_shell::asociaciones::registrar(&exe)
            {
                tracing::warn!(?e, "no se pudo registrar PixPin para imagenes y videos");
            }
            // ms-settings:defaultapps?registeredAppUser=PixPin%20Max
            pixpin_shell::asociaciones::abrir_configuracion();
            Ok(())
        }
        _ => Ok(()),
    };
    if let Err(e) = resultado {
        tracing::warn!(?e, ?clave, "no se pudo abrir desde los ajustes");
    }
}

/// Lo que toca a Windows y no se aplica solo: al cerrar, si cambio.
fn aplicar_al_cerrar(antes: &Ajustes, a: &Ajustes, ubicacion: &Ubicacion) {
    crate::caducidad_capturas::fijar_dias(a.capturas.dias_caducidad);
    let Some(exe) = ruta_exe() else { return };
    if a.arranque_con_windows != antes.arranque_con_windows {
        match pixpin_shell::arranque::establecer(
            a.arranque_con_windows,
            ubicacion.es_portable(),
            &exe,
        ) {
            Ok(()) | Err(pixpin_shell::arranque::ErrorArranque::ModoPortable) => {}
            Err(e) => tracing::warn!(?e, "no se pudo cambiar el arranque con Windows"),
        }
    }
    if a.abrir_con != antes.abrir_con {
        // Lo mismo que hace `main.rs` al arrancar.
        if a.abrir_con {
            if let Err(e) = pixpin_shell::asociaciones::registrar(&exe) {
                tracing::warn!(?e, "no se pudo registrar PixPin para imagenes y videos");
            }
            if let Err(e) = pixpin_shell::abrir_con::inscribir(&exe) {
                tracing::warn!(?e, "no se pudo inscribir en «Abrir con»");
            }
        } else {
            pixpin_shell::asociaciones::quitar();
            if let Err(e) = pixpin_shell::abrir_con::desinscribir(&exe) {
                tracing::warn!(?e, "no se pudo borrar de «Abrir con»");
            }
        }
    }
}

// --- La ventana ---------------------------------------------------------------

/// Abre la ventana y no vuelve hasta que se cierra.
///
/// Devuelve los ajustes nuevos si algo cambio (ya guardados en el fichero),
/// o `None` si se cerro sin tocar nada. Quien llama decide que hacer con
/// ellos: volver a registrar los atajos, sobre todo.
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
    // Si el monitor no da para la ventana entera, se encoge: la lista se
    // desplaza y la columna se ancla abajo, asi que cabe en cualquier alto.
    let ancho = ANCHO.min(monitor.area.ancho as f32 / e - 40.0).max(760.0);
    let alto = ALTO.min(monitor.area.alto as f32 / e - 60.0).max(520.0);
    let (fisico_ancho, fisico_alto) = ((ancho * e) as u32, (alto * e) as u32);
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

    let cx = Contexto {
        t: textos,
        carpeta: crate::galeria_capturas::carpeta(ubicacion)
            .display()
            .to_string(),
    };
    // Copia de trabajo: se toca esta y se guarda al cerrar.
    let mut copia = Copia {
        a: actual.clone(),
        historial: Vec::new(),
    };
    let mut estado = Estado::default();
    let mut resaltado: Option<Golpe> = None;
    let mut pintado: Option<(Estado, Option<Golpe>, u64)> = None;
    let mut version = 0u64;
    let zona = contenido(ancho, alto);

    loop {
        pixpin_shell::overlay::bombear_pendientes();
        let mut filas = filas_actuales(&estado, &copia.a, &cx);
        let mut cerrar = false;

        for (hwnd, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            let local = |p: Punto| Punto {
                x: ((p.x - marco.x) as f32 / e) as i32,
                y: ((p.y - marco.y) as f32 / e) as i32,
            };
            let solo: Vec<Fila> = filas.iter().map(|(_, f)| f.clone()).collect();
            let boton_seccion = (!estado.buscando())
                .then(|| boton_restablecer_seccion(ancho, &rotulo_restablecer(&estado, textos)));
            let golpe_aqui = |p: Punto, estado: &Estado, hay: bool| {
                golpe_en(
                    local(p),
                    ancho,
                    alto,
                    SECCIONES.len(),
                    &solo,
                    estado,
                    hay,
                    boton_seccion,
                )
            };
            match evento {
                EventoOverlay::BotonPulsado(p) => {
                    let golpe = golpe_aqui(p, &estado, !copia.historial.is_empty());
                    // Cualquier clic termina una grabacion de atajo en curso
                    // y decide a donde va lo que se escriba.
                    estado.capturando = None;
                    estado.foco = match golpe {
                        Some(Golpe::Buscador) | Some(Golpe::BorrarBusqueda) => Foco::Buscador,
                        Some(Golpe::Fila(i, Parte::Escribir)) => Foco::Entrada(i),
                        // Pulsar «Añadir» deja el foco en su caja.
                        Some(Golpe::Fila(i, Parte::Pulsar))
                            if matches!(filas.get(i), Some((Clave::AnadirIgnorado, _))) =>
                        {
                            Foco::Entrada(i)
                        }
                        _ => Foco::Nada,
                    };
                    match golpe {
                        Some(Golpe::Seccion(i)) => {
                            estado.seccion = i;
                            estado.busqueda.clear();
                            estado.desplazamiento = 0;
                        }
                        Some(Golpe::BorrarBusqueda) => {
                            estado.busqueda.clear();
                            estado.desplazamiento = 0;
                        }
                        Some(Golpe::Buscador) => {}
                        Some(Golpe::RestablecerSeccion) => {
                            let s = SECCIONES[estado.seccion];
                            copia.cambiar(|a| restablecer_seccion(s, a, &cx));
                        }
                        Some(Golpe::Deshacer) => {
                            copia.deshacer();
                        }
                        Some(Golpe::Listo) => cerrar = true,
                        Some(Golpe::Fila(i, parte)) => {
                            if let Some((clave, fila)) = filas.get(i).cloned() {
                                match parte {
                                    Parte::Alternar => {
                                        copia.cambiar(|a| aplicar_interruptor(a, clave))
                                    }
                                    Parte::Elegir(c) => {
                                        copia.cambiar(|a| aplicar_opcion(a, clave, c))
                                    }
                                    Parte::Menos | Parte::Mas => {
                                        if let Some(n) =
                                            numero_tras(&fila.control, parte == Parte::Mas)
                                        {
                                            copia.cambiar(|a| aplicar_numero(a, clave, n));
                                        }
                                    }
                                    Parte::Capturar => estado.capturando = Some(i),
                                    Parte::Restablecer => copia.cambiar(|a| restablecer(clave, a)),
                                    Parte::Quitar => copia.cambiar(|a| quitar(a, clave)),
                                    Parte::Escribir => {}
                                    Parte::Pulsar if clave == Clave::AnadirIgnorado => {
                                        let texto = std::mem::take(&mut estado.entrada);
                                        copia.cambiar(|a| {
                                            anadir_ignorado(a, &texto);
                                        });
                                    }
                                    Parte::Pulsar => pulsar(clave, &copia.a, ubicacion),
                                }
                            }
                        }
                        None => {}
                    }
                    version += 1;
                }
                EventoOverlay::RatonMovido(p) => {
                    let ahora = golpe_aqui(p, &estado, !copia.historial.is_empty());
                    if ahora != resaltado {
                        resaltado = ahora;
                        version += 1;
                    }
                }
                EventoOverlay::Rueda(delta) => {
                    // Rueda hacia arriba es delta positivo, y subir la lista
                    // es RESTAR desplazamiento.
                    let muescas = delta / 120;
                    estado.desplazamiento = limitar_desplazamiento(
                        estado.desplazamiento - muescas * PASO_RUEDA,
                        &solo,
                        zona,
                    );
                    version += 1;
                }
                EventoOverlay::Caracter(c) => {
                    let escrito = match estado.foco {
                        Foco::Buscador if estado.capturando.is_none() => {
                            estado.desplazamiento = 0;
                            escribir(&mut estado.busqueda, c)
                        }
                        Foco::Entrada(_) if estado.capturando.is_none() => {
                            escribir(&mut estado.entrada, c)
                        }
                        _ => false,
                    };
                    if escrito {
                        version += 1;
                    }
                }
                EventoOverlay::Tecla { vk, ctrl, .. } => {
                    if let Some(i) = estado.capturando {
                        match vk {
                            // Escape cancela la grabacion, NO cierra la
                            // ventana.
                            0x1B => estado.capturando = None,
                            // Suprimir o Retroceso: quitar el atajo.
                            0x2E | 0x08 => {
                                if let Some((clave, _)) = filas.get(i).cloned() {
                                    copia.cambiar(|a| poner_atajo(a, clave, None));
                                }
                                estado.capturando = None;
                            }
                            // Los modificadores solos no cuentan.
                            0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0..=0xA5 => {}
                            _ => {
                                let modificadores = pixpin_shell::modificadores_pulsados();
                                if let Some(atajo) = Atajo::desde_teclado(vk, modificadores) {
                                    if let Some((clave, _)) = filas.get(i).cloned() {
                                        copia.cambiar(|a| poner_atajo(a, clave, Some(atajo)));
                                    }
                                    estado.capturando = None;
                                }
                                // Una tecla que no vale como atajo se ignora y
                                // se sigue esperando.
                            }
                        }
                    } else if ctrl && vk == u32::from(b'F') {
                        estado.foco = Foco::Buscador;
                    } else if estado.foco != Foco::Nada {
                        match vk {
                            0x1B => {
                                // Escape primero vacia el buscador; luego suelta
                                // el foco. Cerrar la ventana es el tercero.
                                if estado.foco == Foco::Buscador && estado.buscando() {
                                    estado.busqueda.clear();
                                    estado.desplazamiento = 0;
                                } else {
                                    estado.foco = Foco::Nada;
                                }
                            }
                            0x08 => match estado.foco {
                                Foco::Buscador => {
                                    estado.busqueda.pop();
                                }
                                Foco::Entrada(_) => {
                                    estado.entrada.pop();
                                }
                                Foco::Nada => {}
                            },
                            0x0D => match estado.foco {
                                Foco::Entrada(i)
                                    if matches!(filas.get(i), Some((Clave::AnadirIgnorado, _))) =>
                                {
                                    let texto = std::mem::take(&mut estado.entrada);
                                    copia.cambiar(|a| {
                                        anadir_ignorado(a, &texto);
                                    });
                                }
                                _ => estado.foco = Foco::Nada,
                            },
                            _ => {}
                        }
                    } else if ctrl && vk == u32::from(b'Z') {
                        copia.deshacer();
                    } else if vk == 0x1B {
                        cerrar = true;
                    } else if !estado.buscando() && (vk == 0x26 || vk == 0x28) {
                        // Flechas arriba y abajo: la seccion anterior o la
                        // siguiente.
                        let n = SECCIONES.len();
                        estado.seccion = if vk == 0x26 {
                            (estado.seccion + n - 1) % n
                        } else {
                            (estado.seccion + 1) % n
                        };
                        estado.desplazamiento = 0;
                    }
                    version += 1;
                }
                EventoOverlay::Cerrar => cerrar = true,
                _ => {}
            }
            // Las filas pueden haber cambiado con el golpe: se rehacen para
            // el siguiente evento de esta misma vuelta.
            filas = filas_actuales(&estado, &copia.a, &cx);
            let solo: Vec<Fila> = filas.iter().map(|(_, f)| f.clone()).collect();
            estado.desplazamiento = limitar_desplazamiento(estado.desplazamiento, &solo, zona);
        }
        if cerrar {
            break;
        }

        let clave_pintado = (estado.clone(), resaltado, version);
        if pintado.as_ref() != Some(&clave_pintado) {
            pintado = Some(clave_pintado);
            let vista = Vista {
                ancho,
                alto,
                filas: &filas,
                estado: &estado,
                resaltado,
                hay_deshacer: !copia.historial.is_empty(),
            };
            if let Ok(destino) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&destino, |p| dibujar(p, e, &vista, textos));
                let _ = superficie.presentar();
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(8));
    }

    ventana.ocultar();
    pixpin_shell::overlay::bombear_pendientes();

    let nuevos = copia.a;
    if nuevos == *actual {
        return Ok(None);
    }
    // Conservando los comentarios: es la razon de que exista
    // `guardar_conservando`.
    pixpin_store::ajustes::guardar_conservando(ubicacion, &nuevos)
        .context("no se pudieron guardar los ajustes")?;
    aplicar_al_cerrar(actual, &nuevos, ubicacion);
    tracing::info!("ajustes guardados desde la ventana");
    Ok(Some(nuevos))
}

fn rotulo_restablecer(estado: &Estado, t: &Catalogo) -> String {
    let mut args = FluentArgs::new();
    args.set("seccion", SECCIONES[estado.seccion].titulo(t));
    t.t_args("ajustes2-restablecer", &args)
}

// --- El pintado -----------------------------------------------------------------

/// Lo que hace falta para pintar un fotograma.
struct Vista<'a> {
    ancho: f32,
    alto: f32,
    filas: &'a [(Clave, Fila)],
    estado: &'a Estado,
    resaltado: Option<Golpe>,
    hay_deshacer: bool,
}

fn rf(r: Recta, e: f32) -> RectF {
    RectF {
        x: r.x * e,
        y: r.y * e,
        ancho: r.ancho * e,
        alto: r.alto * e,
    }
}

fn negrita() -> Letra<'static> {
    Letra {
        negrita: true,
        ..Letra::de("Segoe UI")
    }
}

fn semi() -> Letra<'static> {
    Letra::de("Segoe UI Semibold")
}

/// Un texto centrado en una caja (fisica).
fn centrar(p: &Pintor, texto: &str, r: RectF, tam: f32, color: Color, negrita: bool) {
    let (w, h) = if negrita {
        p.medir_con_letra(texto, tam, SIN_PARTIR, &semi())
    } else {
        p.medir_texto(texto, tam)
    };
    let (x, y) = (r.x + (r.ancho - w) / 2.0, r.y + (r.alto - h) / 2.0);
    if negrita {
        p.texto_con_letra(texto, x, y, tam, SIN_PARTIR, &semi(), color);
    } else {
        p.texto(texto, x, y, tam, color);
    }
}

/// Un borde de 1 px alrededor de una caja redondeada: la caja de debajo un
/// poco mas grande en el color del borde.
fn caja_con_borde(p: &Pintor, r: RectF, radio: f32, fondo: Color, borde: Color, e: f32) {
    p.rellenar_redondeado(
        RectF {
            x: r.x - e,
            y: r.y - e,
            ancho: r.ancho + 2.0 * e,
            alto: r.alto + 2.0 * e,
        },
        radio + e,
        borde,
    );
    p.rellenar_redondeado(r, radio, fondo);
}

/// La chapita de un atajo («Ctrl F», «Esc»), con su borde derecho en `x`.
/// Devuelve su ancho.
fn chapita(p: &Pintor, texto: &str, derecha: f32, centro_y: f32, e: f32, sobre_azul: bool) -> f32 {
    let tam = 11.0 * e;
    let (w, h) = p.medir_texto(texto, tam);
    let caja = RectF {
        x: derecha - w - 12.0 * e,
        y: centro_y - 10.0 * e,
        ancho: w + 12.0 * e,
        alto: 20.0 * e,
    };
    if sobre_azul {
        p.rellenar_redondeado(caja, 5.0 * e, blanco(0.22));
    } else {
        caja_con_borde(p, caja, 5.0 * e, blanco(0.08), BORDE, e);
    }
    p.texto(
        texto,
        caja.x + 6.0 * e,
        centro_y - h / 2.0,
        tam,
        if sobre_azul { TINTA } else { CHIP_TEXTO },
    );
    caja.ancho
}

/// El icono centrado en una caja, a `lado` logicos.
fn icono_en(p: &Pintor, icono: &Icono, r: RectF, lado: f32, e: f32, color: Color) {
    let l = lado * e;
    p.icono(
        icono,
        RectF {
            x: r.x + (r.ancho - l) / 2.0,
            y: r.y + (r.alto - l) / 2.0,
            ancho: l,
            alto: l,
        },
        color,
    );
}

/// Lo que se ve en la ventana, en un pintor cualquiera: el de la ventana o,
/// en las pruebas, uno fuera de pantalla para dejar la muestra en PNG.
fn dibujar(p: &Pintor, e: f32, v: &Vista, t: &Catalogo) {
    let (ancho, alto) = (v.ancho, v.alto);
    p.limpiar_transparente();
    // El fondo: entero en el gris de la columna, y el contenido encima.
    let entera = RectF {
        x: 0.0,
        y: 0.0,
        ancho: ancho * e,
        alto: alto * e,
    };
    p.rellenar_redondeado(entera, 12.0 * e, FONDO_NAV);
    p.rellenar_redondeado(
        RectF {
            x: NAV_ANCHO * e,
            ancho: (ancho - NAV_ANCHO) * e,
            ..entera
        },
        12.0 * e,
        FONDO,
    );
    p.rellenar(
        RectF {
            x: NAV_ANCHO * e,
            ancho: 16.0 * e,
            ..entera
        },
        FONDO,
    );
    p.rellenar(
        RectF {
            x: NAV_ANCHO * e,
            ancho: e,
            ..entera
        },
        SEPARADOR,
    );

    dibujar_contenido(p, e, v, t);
    dibujar_cabecera(p, e, v, t);
    dibujar_columna(p, e, v, t);
}

fn dibujar_columna(p: &Pintor, e: f32, v: &Vista, t: &Catalogo) {
    let estado = v.estado;
    p.texto_con_letra(
        &t.t("ajustes2-titulo"),
        18.0 * e,
        16.0 * e,
        20.0 * e,
        SIN_PARTIR,
        &negrita(),
        TINTA,
    );

    // El buscador.
    let b = rf(buscador(), e);
    let enfocado = estado.foco == Foco::Buscador;
    if enfocado {
        caja_con_borde(p, b, 10.0 * e, blanco(0.07), AZUL, e);
    } else {
        p.rellenar_redondeado(b, 10.0 * e, blanco(0.07));
    }
    icono_en(
        p,
        &mi::SEARCH,
        RectF {
            x: b.x + 4.0 * e,
            ancho: 28.0 * e,
            ..b
        },
        18.0,
        e,
        TENUE,
    );
    let tam = 14.0 * e;
    let x_texto = b.x + 34.0 * e;
    let (texto, color) = if estado.busqueda.is_empty() {
        (t.t("ajustes2-buscar"), TENUE)
    } else {
        (estado.busqueda.clone(), TINTA)
    };
    let (w, h) = p.medir_texto(&texto, tam);
    let y_texto = b.y + (b.alto - h) / 2.0;
    p.con_recorte(
        RectF {
            x: x_texto,
            ancho: b.ancho - 34.0 * e - 44.0 * e,
            ..b
        },
        |p| p.texto(&texto, x_texto, y_texto, tam, color),
    );
    if enfocado {
        let x_cursor = if estado.busqueda.is_empty() {
            x_texto
        } else {
            x_texto + w + e
        };
        p.rellenar(
            RectF {
                x: x_cursor,
                y: y_texto + 2.0 * e,
                ancho: 1.5 * e,
                alto: h - 4.0 * e,
            },
            AZUL_CLARO,
        );
    }
    if estado.buscando() {
        let x = rf(borrar_busqueda(), e);
        if v.resaltado == Some(Golpe::BorrarBusqueda) {
            p.rellenar_redondeado(x, 8.0 * e, BOTON_SOBRE);
        }
        icono_en(p, &mi::CLOSE, x, 16.0, e, TEXTO);
    } else {
        chapita(
            p,
            "Ctrl F",
            b.x + b.ancho - 8.0 * e,
            b.y + b.alto / 2.0,
            e,
            false,
        );
    }

    // Las secciones.
    let (deshacer, listo) = botones_del_pie(v.alto);
    for (i, s) in SECCIONES.iter().enumerate() {
        let r = rect_de_seccion(i);
        if r.y + r.alto > deshacer.y - 50.0 {
            break;
        }
        let r = rf(r, e);
        let elegida = !estado.buscando() && estado.seccion == i;
        if elegida {
            p.rellenar_redondeado(r, 10.0 * e, Color { a: 0.18, ..AZUL });
            p.rellenar_redondeado(
                RectF {
                    ancho: 3.0 * e,
                    y: r.y + 8.0 * e,
                    alto: r.alto - 16.0 * e,
                    ..r
                },
                1.5 * e,
                AZUL,
            );
        } else if v.resaltado == Some(Golpe::Seccion(i)) {
            p.rellenar_redondeado(r, 10.0 * e, SOBRE);
        }
        icono_en(
            p,
            s.icono(),
            RectF {
                x: r.x + 6.0 * e,
                ancho: 32.0 * e,
                ..r
            },
            19.0,
            e,
            if elegida { AZUL_CLARO } else { TENUE },
        );
        let nombre = s.titulo(t);
        let (_, h) = p.medir_texto(&nombre, 14.0 * e);
        p.texto(
            &nombre,
            r.x + 44.0 * e,
            r.y + (r.alto - h) / 2.0,
            14.0 * e,
            if elegida { TINTA } else { TEXTO },
        );
    }

    // La nota, el «Deshacer» y el «Listo».
    let tam_nota = 12.0 * e;
    let x_nota = 20.0 * e;
    let y_nota = (deshacer.y - 44.0) * e;
    p.texto(&t.t("ajustes2-nota"), x_nota, y_nota, tam_nota, TENUE);
    let (_, hn) = p.medir_texto("Ag", tam_nota);
    let y2 = y_nota + hn + 2.0 * e;
    p.circulo((x_nota + 4.0 * e, y2 + hn / 2.0), 3.5 * e, AZUL);
    p.texto(
        &t.t("ajustes2-nota-punto"),
        x_nota + 12.0 * e,
        y2,
        tam_nota,
        TENUE,
    );

    if v.hay_deshacer {
        let r = rf(deshacer, e);
        p.rellenar_redondeado(
            r,
            10.0 * e,
            if v.resaltado == Some(Golpe::Deshacer) {
                BOTON_SOBRE
            } else {
                BOTON
            },
        );
        icono_en(
            p,
            &DESHACER,
            RectF {
                x: r.x + 6.0 * e,
                ancho: 32.0 * e,
                ..r
            },
            18.0,
            e,
            TEXTO,
        );
        let rotulo = t.t("ajustes2-deshacer");
        let (_, h) = p.medir_texto(&rotulo, 14.0 * e);
        p.texto(
            &rotulo,
            r.x + 42.0 * e,
            r.y + (r.alto - h) / 2.0,
            14.0 * e,
            TINTA,
        );
        chapita(
            p,
            "Ctrl Z",
            r.x + r.ancho - 10.0 * e,
            r.y + r.alto / 2.0,
            e,
            false,
        );
    }
    // La accion principal: azul y siempre en la misma esquina.
    let r = rf(listo, e);
    p.rellenar_redondeado(
        r,
        10.0 * e,
        if v.resaltado == Some(Golpe::Listo) {
            Color { a: 0.85, ..AZUL }
        } else {
            AZUL
        },
    );
    centrar(p, &t.t("ajustes2-listo"), r, 14.0 * e, blanco(1.0), true);
    chapita(
        p,
        "Esc",
        r.x + r.ancho - 10.0 * e,
        r.y + r.alto / 2.0,
        e,
        true,
    );
}

fn dibujar_cabecera(p: &Pintor, e: f32, v: &Vista, t: &Catalogo) {
    // Fondo opaco: tapa lo que la lista haya desplazado por debajo.
    p.rellenar(
        RectF {
            x: (NAV_ANCHO + 1.0) * e,
            y: 0.0,
            ancho: (v.ancho - NAV_ANCHO - 14.0) * e,
            alto: pixpin_ui::ajustes::CABECERA_ALTO * e,
        },
        FONDO,
    );
    let x = (NAV_ANCHO + pixpin_ui::ajustes::CONTENIDO_MARGEN) * e;
    let (titulo, sub) = if v.estado.buscando() {
        let mut args = FluentArgs::new();
        args.set("busqueda", v.estado.busqueda.trim().to_string());
        let n = v.filas.iter().filter(|(_, f)| !f.es_grupo()).count();
        let mut a2 = FluentArgs::new();
        a2.set("n", n);
        (
            t.t_args("ajustes2-resultados", &args),
            t.t_args("ajustes2-resultados-sub", &a2),
        )
    } else {
        let s = SECCIONES[v.estado.seccion];
        (s.titulo(t), s.subtitulo(t))
    };
    let ancho_titulo = if v.estado.buscando() {
        v.ancho - NAV_ANCHO - 60.0
    } else {
        v.ancho - NAV_ANCHO - 300.0
    };
    p.con_recorte(
        RectF {
            x,
            y: 0.0,
            ancho: ancho_titulo * e,
            alto: 90.0 * e,
        },
        |p| {
            p.texto_con_letra(
                &titulo,
                x,
                22.0 * e,
                24.0 * e,
                SIN_PARTIR,
                &negrita(),
                TINTA,
            );
        },
    );
    p.texto_ajustado(&sub, x, 58.0 * e, 13.0 * e, ancho_titulo * e, TENUE);

    if !v.estado.buscando() {
        let rotulo = rotulo_restablecer(v.estado, t);
        let r = rf(boton_restablecer_seccion(v.ancho, &rotulo), e);
        // Apagado si no hay nada que restablecer en la seccion.
        let algo = v.filas.iter().any(|(_, f)| f.cambiado);
        p.rellenar_redondeado(
            r,
            10.0 * e,
            if algo && v.resaltado == Some(Golpe::RestablecerSeccion) {
                BOTON_SOBRE
            } else {
                BOTON
            },
        );
        let color = if algo { TINTA } else { TENUE };
        let (w, h) = p.medir_texto(&rotulo, 13.0 * e);
        let total = 18.0 * e + 8.0 * e + w;
        let x0 = r.x + (r.ancho - total) / 2.0;
        p.icono(
            &RESTABLECER,
            RectF {
                x: x0,
                y: r.y + (r.alto - 18.0 * e) / 2.0,
                ancho: 18.0 * e,
                alto: 18.0 * e,
            },
            color,
        );
        p.texto(
            &rotulo,
            x0 + 26.0 * e,
            r.y + (r.alto - h) / 2.0,
            13.0 * e,
            color,
        );
    }
}

fn dibujar_contenido(p: &Pintor, e: f32, v: &Vista, t: &Catalogo) {
    let zona = contenido(v.ancho, v.alto);
    let solo: Vec<Fila> = v.filas.iter().map(|(_, f)| f.clone()).collect();
    let des = v.estado.desplazamiento;

    if v.estado.buscando() && solo.is_empty() {
        let mut args = FluentArgs::new();
        args.set("busqueda", v.estado.busqueda.trim().to_string());
        p.texto_ajustado(
            &t.t_args("ajustes2-sin-resultados", &args),
            zona.x * e,
            (zona.y + 24.0) * e,
            14.0 * e,
            zona.ancho * e,
            TENUE,
        );
        return;
    }

    // Recorte un poco mas ancho que la zona: el borde de las cajas con foco
    // sobresale 1 px.
    let recorte = RectF {
        x: (zona.x - 4.0) * e,
        y: zona.y * e,
        ancho: (zona.ancho + 8.0) * e,
        alto: zona.alto * e,
    };
    p.con_recorte(recorte, |p| {
        // Primero las tarjetas: cada tanda seguida de opciones va junta.
        let mut i = 0;
        while i < solo.len() {
            if solo[i].es_grupo() {
                i += 1;
                continue;
            }
            let primera = rect_de_fila(&solo, i, des, zona);
            let mut j = i;
            while j + 1 < solo.len() && !solo[j + 1].es_grupo() {
                j += 1;
            }
            let ultima = rect_de_fila(&solo, j, des, zona);
            p.rellenar_redondeado(
                RectF {
                    x: primera.x * e,
                    y: primera.y * e,
                    ancho: primera.ancho * e,
                    alto: (ultima.y + ultima.alto - primera.y) * e,
                },
                14.0 * e,
                TARJETA,
            );
            // Separadores entre opciones de la misma tarjeta.
            for k in i..j {
                let r = rect_de_fila(&solo, k, des, zona);
                p.rellenar(
                    RectF {
                        x: (r.x + 16.0) * e,
                        y: (r.y + r.alto - 1.0) * e,
                        ancho: (r.ancho - 32.0) * e,
                        alto: e,
                    },
                    SEPARADOR,
                );
            }
            i = j + 1;
        }

        for (i, (clave, fila)) in v.filas.iter().enumerate() {
            let r = rect_de_fila(&solo, i, des, zona);
            if r.y + r.alto < zona.y || r.y > zona.y + zona.alto {
                continue;
            }
            if fila.es_grupo() {
                let texto = fila.etiqueta.to_uppercase();
                let (_, h) = p.medir_con_letra(&texto, 12.0 * e, SIN_PARTIR, &semi());
                p.texto_con_letra(
                    &texto,
                    (r.x + 4.0) * e,
                    (r.y + r.alto - 8.0) * e - h,
                    12.0 * e,
                    SIN_PARTIR,
                    &semi(),
                    TENUE,
                );
                continue;
            }
            dibujar_fila(p, e, v, t, i, *clave, fila, r);
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn dibujar_fila(
    p: &Pintor,
    e: f32,
    v: &Vista,
    t: &Catalogo,
    i: usize,
    clave: Clave,
    fila: &Fila,
    r: Recta,
) {
    let sobre = |parte: Parte| v.resaltado == Some(Golpe::Fila(i, parte));
    // La etiqueta y la explicacion, centradas en vertical juntas.
    let ancho_textos = ancho_de_textos(fila, r) * e;
    let x = (r.x + pixpin_ui::ajustes::TARJETA_RELLENO) * e;
    let tam = 14.0 * e;
    let tam_ayuda = 12.0 * e;
    let (lw, lh) = p.medir_texto_ajustado(&fila.etiqueta, tam, ancho_textos);
    let (_, ah) = if fila.ayuda.is_empty() {
        (0.0, 0.0)
    } else {
        p.medir_texto_ajustado(&fila.ayuda, tam_ayuda, ancho_textos)
    };
    let total = lh + if ah > 0.0 { ah + 2.0 * e } else { 0.0 };
    let y0 = r.y * e + (r.alto * e - total) / 2.0;
    p.texto_ajustado(&fila.etiqueta, x, y0, tam, ancho_textos, TINTA);
    if fila.cambiado {
        p.circulo((x + lw + 8.0 * e, y0 + lh / 2.0), 3.5 * e, AZUL);
    }
    if ah > 0.0 {
        p.texto_ajustado(
            &fila.ayuda,
            x,
            y0 + lh + 2.0 * e,
            tam_ayuda,
            ancho_textos,
            TENUE,
        );
    }

    let partes = partes_de_fila(fila, r);
    // El fondo de una eleccion entera, debajo de sus botones.
    let elecciones: Vec<Recta> = partes
        .iter()
        .filter(|(pa, _)| matches!(pa, Parte::Elegir(_)))
        .map(|(_, c)| *c)
        .collect();
    if let (Some(a), Some(b)) = (elecciones.first(), elecciones.last()) {
        p.rellenar_redondeado(
            rf(
                Recta {
                    x: a.x - 3.0,
                    y: a.y - 3.0,
                    ancho: b.x + b.ancho - a.x + 6.0,
                    alto: a.alto + 6.0,
                },
                e,
            ),
            9.0 * e,
            HUNDIDO,
        );
    }
    // El fondo del numero, de «−» a «+».
    if let (Some((_, menos)), Some((_, mas))) = (
        partes.iter().find(|(pa, _)| *pa == Parte::Menos),
        partes.iter().find(|(pa, _)| *pa == Parte::Mas),
    ) {
        let fondo = rf(
            Recta {
                ancho: mas.x + mas.ancho - menos.x,
                ..*menos
            },
            e,
        );
        caja_con_borde(p, fondo, 9.0 * e, HUNDIDO, BORDE, e);
        if let Control::Numero { texto, .. } = &fila.control {
            centrar(
                p,
                texto,
                RectF {
                    x: (menos.x + menos.ancho) * e,
                    ancho: (mas.x - menos.x - menos.ancho) * e,
                    ..fondo
                },
                14.0 * e,
                TINTA,
                true,
            );
        }
    }

    for (parte, c) in &partes {
        let caja = rf(*c, e);
        match parte {
            Parte::Alternar => {
                let encendido = matches!(fila.control, Control::Interruptor(true));
                p.rellenar_redondeado(
                    caja,
                    caja.alto / 2.0,
                    if encendido {
                        VERDE
                    } else if sobre(Parte::Alternar) {
                        blanco(0.24)
                    } else {
                        blanco(0.16)
                    },
                );
                let bola = 20.0 * e;
                let bx = if encendido {
                    caja.x + caja.ancho - bola - 3.0 * e
                } else {
                    caja.x + 3.0 * e
                };
                p.circulo(
                    (bx + bola / 2.0, caja.y + caja.alto / 2.0),
                    bola / 2.0,
                    blanco(1.0),
                );
            }
            Parte::Elegir(k) => {
                let Control::Opcion { opciones, elegida } = &fila.control else {
                    continue;
                };
                let es = *k == *elegida;
                if es {
                    p.rellenar_redondeado(caja, 7.0 * e, SEG_ELEGIDO);
                } else if sobre(*parte) {
                    p.rellenar_redondeado(caja, 7.0 * e, blanco(0.07));
                }
                let tam = if clave == Clave::Esquina { 16.0 } else { 13.0 };
                centrar(
                    p,
                    &opciones[*k],
                    caja,
                    tam * e,
                    if es { blanco(1.0) } else { TEXTO },
                    es,
                );
            }
            Parte::Menos | Parte::Mas => {
                if sobre(*parte) {
                    p.rellenar_redondeado(caja, 8.0 * e, BOTON);
                }
                centrar(
                    p,
                    if *parte == Parte::Menos { "−" } else { "+" },
                    caja,
                    18.0 * e,
                    TINTA,
                    false,
                );
            }
            Parte::Capturar => {
                let Control::Atajo { texto, choca } = &fila.control else {
                    continue;
                };
                let grabando = v.estado.capturando == Some(i);
                if grabando {
                    p.rellenar_redondeado(caja, 9.0 * e, AZUL);
                } else {
                    caja_con_borde(
                        p,
                        caja,
                        9.0 * e,
                        if sobre(*parte) { blanco(0.05) } else { HUNDIDO },
                        if *choca { ROJO } else { BORDE },
                        e,
                    );
                }
                let sin = *texto == t.t("ajustes-sin-atajo");
                let (rotulo, color) = if grabando {
                    (t.t("ajustes-pulsa-combinacion"), blanco(1.0))
                } else if *choca {
                    (format!("{texto}  ·  {}", t.t("ajustes-choca")), ROJO)
                } else if sin {
                    (texto.clone(), TENUE)
                } else {
                    (texto.clone(), TINTA)
                };
                centrar(p, &rotulo, caja, 13.0 * e, color, !sin && !grabando);
            }
            Parte::Pulsar => {
                p.rellenar_redondeado(
                    caja,
                    9.0 * e,
                    if sobre(*parte) { BOTON_SOBRE } else { BOTON },
                );
                let rotulo = match &fila.control {
                    Control::Boton(s) => s.clone(),
                    Control::Entrada { boton, .. } => format!("+ {boton}"),
                    _ => String::new(),
                };
                let icono = match clave {
                    Clave::Carpeta => Some(&mi::FOLDER),
                    Clave::Predeterminada => Some(&mi::OPEN_IN_NEW),
                    Clave::Fichero => Some(&mi::DESCRIPTION),
                    _ => None,
                };
                match icono {
                    Some(ic) => {
                        let (w, h) = p.medir_texto(&rotulo, 13.0 * e);
                        let total = 16.0 * e + 8.0 * e + w;
                        let x0 = caja.x + (caja.ancho - total) / 2.0;
                        p.icono(
                            ic,
                            RectF {
                                x: x0,
                                y: caja.y + (caja.alto - 16.0 * e) / 2.0,
                                ancho: 16.0 * e,
                                alto: 16.0 * e,
                            },
                            TEXTO,
                        );
                        p.texto(
                            &rotulo,
                            x0 + 24.0 * e,
                            caja.y + (caja.alto - h) / 2.0,
                            13.0 * e,
                            TINTA,
                        );
                    }
                    None => centrar(p, &rotulo, caja, 13.0 * e, TINTA, false),
                }
            }
            Parte::Escribir => {
                let Control::Entrada { marcador, .. } = &fila.control else {
                    continue;
                };
                let enfocada = v.estado.foco == Foco::Entrada(i);
                caja_con_borde(
                    p,
                    caja,
                    9.0 * e,
                    HUNDIDO,
                    if enfocada { AZUL } else { BORDE },
                    e,
                );
                let (texto, color) = if v.estado.entrada.is_empty() {
                    (marcador.clone(), TENUE)
                } else {
                    (v.estado.entrada.clone(), TINTA)
                };
                let (w, h) = p.medir_texto(&texto, 13.0 * e);
                let xt = caja.x + 10.0 * e;
                let yt = caja.y + (caja.alto - h) / 2.0;
                p.con_recorte(
                    RectF {
                        x: xt,
                        ancho: caja.ancho - 20.0 * e,
                        ..caja
                    },
                    |p| p.texto(&texto, xt, yt, 13.0 * e, color),
                );
                if enfocada {
                    let xc = if v.estado.entrada.is_empty() {
                        xt
                    } else {
                        xt + w + e
                    };
                    p.rellenar(
                        RectF {
                            x: xc,
                            y: yt + 2.0 * e,
                            ancho: 1.5 * e,
                            alto: h - 4.0 * e,
                        },
                        AZUL_CLARO,
                    );
                }
            }
            Parte::Restablecer => {
                if sobre(*parte) {
                    p.rellenar_redondeado(caja, 9.0 * e, BOTON);
                }
                icono_en(
                    p,
                    &RESTABLECER,
                    caja,
                    17.0,
                    e,
                    if sobre(*parte) { TINTA } else { TENUE },
                );
            }
            Parte::Quitar => {
                // Quitar va en rojo al pasar por encima, y apartado a la
                // derecha del todo. Se deshace con Ctrl+Z.
                if sobre(*parte) {
                    p.rellenar_redondeado(caja, 9.0 * e, Color { a: 0.16, ..ROJO });
                }
                icono_en(
                    p,
                    &mi::DELETE,
                    caja,
                    18.0,
                    e,
                    if sobre(*parte) { ROJO } else { TENUE },
                );
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn encender_en_un_sitio_una_apagada_en_todos_la_deja_solo_ahi() {
        let mut a = Ajustes::default();
        alternar_herramienta(&mut a, None, "lazo");
        assert!(!a.herramientas.activa("lazo"));
        alternar_herramienta(&mut a, Some("pin"), "lazo");
        assert!(a.herramientas.activa_en("pin", "lazo"));
        for s in SITIOS.iter().filter(|s| **s != "pin") {
            assert!(!a.herramientas.activa_en(s, "lazo"), "{s}");
        }
        // Y apagarla en un sitio no toca los demas.
        let mut b = Ajustes::default();
        alternar_herramienta(&mut b, Some("lienzo"), "mosaico");
        assert!(!b.herramientas.activa_en("lienzo", "mosaico"));
        assert!(b.herramientas.activa_en("pantalla", "mosaico"));
    }

    fn textos() -> Catalogo {
        Catalogo::nuevo(pixpin_store::Idioma::Espanol)
    }

    fn cx(t: &Catalogo) -> Contexto<'_> {
        Contexto {
            t,
            carpeta: r"C:\Users\Max\AppData\Roaming\PixPinMax\capturas".into(),
        }
    }

    /// Cada clave de cada seccion, para recorrerlas todas.
    fn todas(a: &Ajustes, cx: &Contexto) -> Vec<(Seccion, Clave, Fila)> {
        SECCIONES
            .iter()
            .flat_map(|s| {
                filas_de_seccion(*s, a, cx)
                    .into_iter()
                    .map(move |(c, f)| (*s, c, f))
            })
            .collect()
    }

    #[test]
    fn de_fabrica_no_hay_ningun_punto_azul_y_cada_opcion_lleva_su_explicacion() {
        let t = textos();
        let a = Ajustes::default();
        for (s, c, f) in todas(&a, &cx(&t)) {
            assert!(!f.cambiado, "{s:?} {c:?} sale cambiado de fabrica");
            // Ningun texto se queda en su clave sin traducir.
            assert!(!f.etiqueta.starts_with("ajustes"), "{}", f.etiqueta);
            assert!(!f.ayuda.starts_with("ajustes"), "{}", f.ayuda);
            // Todas las opciones explican algo, salvo los atajos y las
            // herramientas (su nombre ya lo dice).
            let sin_ayuda = matches!(c, Clave::Comando(_) | Clave::Herramienta(_) | Clave::Grupo);
            assert!(sin_ayuda || !f.ayuda.is_empty(), "{c:?} sin explicacion");
        }
        // Las secciones vacias no existen: cada una trae al menos una opcion.
        for s in SECCIONES {
            assert!(
                filas_de_seccion(s, &a, &cx(&t))
                    .iter()
                    .any(|(_, f)| !f.es_grupo())
            );
            assert!(!s.titulo(&t).starts_with("ajustes2"));
            assert!(!s.subtitulo(&t).starts_with("ajustes2"));
        }
    }

    #[test]
    fn todo_lo_del_fichero_tiene_su_control() {
        // La lista de lo que el usuario puede tocar en el TOML. Si se anade
        // un ajuste nuevo y no sale aqui, esta prueba lo dice.
        let t = textos();
        let a = Ajustes::default();
        let claves: Vec<Clave> = todas(&a, &cx(&t)).into_iter().map(|(_, c, _)| c).collect();
        for c in [
            Clave::Arranque,
            Clave::Idioma,
            Clave::PdfAligerar,
            Clave::PdfNivel,
            Clave::RetardoCaptura,
            Clave::Color,
            Clave::LimiteScroll,
            Clave::GifRitmo,
            Clave::GifRetardo,
            Clave::Caducidad,
            Clave::Apilar,
            Clave::Esquina,
            Clave::IconoSegundos,
            Clave::AnadirIgnorado,
            Clave::ImanActivo,
            Clave::ImanEsquinas,
            Clave::ImanMedios,
            Clave::ImanCentros,
            Clave::ImanRadio,
            Clave::Suavizado,
            Clave::VozSegundo,
            Clave::VozModo,
            Clave::Presencia,
            Clave::LoMioManda,
            Clave::AbrirCon,
            Clave::Predeterminada,
            Clave::TemaCosmos,
            Clave::Nivel,
            Clave::MedirFotogramas,
            Clave::Ritmo,
            Clave::Paneo,
            Clave::CorteMinimo,
            Clave::Beta,
            Clave::Fichero,
        ] {
            assert!(claves.contains(&c), "falta {c:?}");
        }
        assert!(claves.contains(&Clave::Comando(Comando::CapturarRegion)));
        assert_eq!(
            claves
                .iter()
                .filter(|c| matches!(c, Clave::Herramienta(_)))
                .count(),
            pixpin_store::herramientas::NOMBRES.len()
        );
        // Caso negativo: sin regiones ni programas, no hay filas de ellos.
        assert!(
            !claves
                .iter()
                .any(|c| matches!(c, Clave::Region(_) | Clave::Ignorado(_)))
        );
    }

    #[test]
    fn cambiar_pone_el_punto_azul_y_restablecer_lo_quita() {
        let t = textos();
        let cx = cx(&t);
        let mut a = Ajustes::default();
        aplicar_numero(&mut a, Clave::GifRitmo, 15);
        let filas = filas_de_seccion(Seccion::Captura, &a, &cx);
        let (_, gif) = filas.iter().find(|(c, _)| *c == Clave::GifRitmo).unwrap();
        assert!(gif.cambiado);
        // Caso negativo: la de al lado sigue sin punto.
        let (_, otra) = filas.iter().find(|(c, _)| *c == Clave::GifRetardo).unwrap();
        assert!(!otra.cambiado);
        restablecer(Clave::GifRitmo, &mut a);
        assert_eq!(a, Ajustes::default());
    }

    #[test]
    fn restablecer_la_seccion_deja_sus_listas_y_no_toca_las_demas() {
        let t = textos();
        let cx = cx(&t);
        let mut a = Ajustes::default();
        aplicar_numero(&mut a, Clave::Caducidad, 30);
        aplicar_interruptor(&mut a, Clave::Apilar);
        aplicar_opcion(&mut a, Clave::Esquina, 0);
        assert!(anadir_ignorado(&mut a, "keepassxc.exe"));
        aplicar_interruptor(&mut a, Clave::TemaCosmos);
        restablecer_seccion(Seccion::Captura, &mut a, &cx);
        assert_eq!(a.capturas, Ajustes::default().capturas);
        // Los programas del usuario se quedan, y lo de otra seccion tambien.
        assert_eq!(a.ignorar_programas, vec!["keepassxc.exe".to_string()]);
        assert!(a.tema_cosmos);
    }

    #[test]
    fn los_atajos_se_graban_se_quitan_y_vuelven_al_de_fabrica() {
        let t = textos();
        let cx = cx(&t);
        let mut a = Ajustes::default();
        let otro: Atajo = "Ctrl+Alt+K".parse().unwrap();
        poner_atajo(&mut a, Clave::Comando(Comando::CapturarRegion), Some(otro));
        let filas = filas_de_seccion(Seccion::Atajos, &a, &cx);
        let (_, f) = filas
            .iter()
            .find(|(c, _)| *c == Clave::Comando(Comando::CapturarRegion))
            .unwrap();
        assert!(f.cambiado);
        assert_eq!(
            f.control,
            Control::Atajo {
                texto: otro.to_string(),
                choca: false
            }
        );
        restablecer(Clave::Comando(Comando::CapturarRegion), &mut a);
        assert_eq!(
            Enlaces::de_ajustes(&a).0.atajo_de(Comando::CapturarRegion),
            Enlaces::default().atajo_de(Comando::CapturarRegion)
        );
        // Caso negativo: quitar el atajo de fabrica SI es un cambio.
        poner_atajo(&mut a, Clave::Comando(Comando::CapturarRegion), None);
        assert!(distinto_de_fabrica(
            Clave::Comando(Comando::CapturarRegion),
            &a
        ));
    }

    #[test]
    fn el_atajo_de_una_region_se_graba_en_la_region() {
        let mut a = Ajustes::default();
        a.regiones.push(pixpin_store::regiones::Region {
            nombre: "panel".into(),
            x: 0,
            y: 0,
            ancho: 300,
            alto: 200,
            atajo: None,
        });
        poner_atajo(
            &mut a,
            Clave::Region(0),
            Some("Ctrl+Alt+1".parse().unwrap()),
        );
        assert_eq!(a.regiones[0].atajo.as_deref(), Some("Ctrl+Alt+1"));
        quitar(&mut a, Clave::Region(0));
        assert!(a.regiones.is_empty());
        // Caso negativo: quitar una que no existe no rompe nada.
        quitar(&mut a, Clave::Region(3));
    }

    #[test]
    fn los_programas_ignorados_no_se_repiten_ni_entran_vacios() {
        let mut a = Ajustes::default();
        assert!(anadir_ignorado(&mut a, "  KeePassXC.exe "));
        assert!(!anadir_ignorado(&mut a, "keepassxc"), "el .exe no cuenta");
        assert!(!anadir_ignorado(&mut a, "   "));
        assert!(anadir_ignorado(&mut a, "\"banca.exe\""));
        assert_eq!(a.ignorar_programas, vec!["KeePassXC.exe", "banca.exe"]);
        quitar(&mut a, Clave::Ignorado(0));
        assert_eq!(a.ignorar_programas, vec!["banca.exe"]);
    }

    #[test]
    fn apilar_es_un_interruptor_sobre_el_numero_de_siempre() {
        let mut a = Ajustes::default();
        assert_ne!(a.capturas.apilar_segundos, 0);
        aplicar_interruptor(&mut a, Clave::Apilar);
        assert_eq!(a.capturas.apilar_segundos, 0);
        aplicar_interruptor(&mut a, Clave::Apilar);
        assert_eq!(
            a.capturas.apilar_segundos,
            Ajustes::default().capturas.apilar_segundos
        );
    }

    #[test]
    fn el_filtro_del_lapiz_en_auto_es_none_y_con_numero_es_el_numero() {
        let mut a = Ajustes::default();
        aplicar_numero(&mut a, Clave::CorteMinimo, 5);
        aplicar_numero(&mut a, Clave::Beta, 20);
        assert_eq!(a.tinta.corte_minimo, Some(0.5));
        assert_eq!(a.tinta.beta, Some(0.02));
        // Caso negativo: cero no es 0 Hz, es «lo del motor».
        aplicar_numero(&mut a, Clave::CorteMinimo, 0);
        assert_eq!(a.tinta.corte_minimo, None);
    }

    #[test]
    fn el_boton_de_la_app_predeterminada_solo_sale_si_se_ofrece_pixpin() {
        let t = textos();
        let cx = cx(&t);
        let mut a = Ajustes::default();
        let tiene = |a: &Ajustes| {
            filas_de_seccion(Seccion::Apps, a, &cx)
                .iter()
                .any(|(c, _)| *c == Clave::Predeterminada)
        };
        assert!(tiene(&a));
        aplicar_interruptor(&mut a, Clave::AbrirCon);
        // Caso negativo: sin inscribirse, Configuracion no tiene su pagina.
        assert!(!tiene(&a));
    }

    #[test]
    fn el_buscador_encuentra_en_todas_las_secciones_bajo_su_nombre() {
        let t = textos();
        let cx = cx(&t);
        let a = Ajustes::default();
        let estado = Estado {
            busqueda: "dias".into(),
            ..Estado::default()
        };
        let filas = filas_actuales(&estado, &a, &cx);
        assert_eq!(filas[0].1.etiqueta, "Captura");
        assert!(filas.iter().any(|(c, _)| *c == Clave::Caducidad));
        let estado = Estado {
            busqueda: "imán".into(),
            ..Estado::default()
        };
        let filas = filas_actuales(&estado, &a, &cx);
        assert!(filas.iter().any(|(c, _)| *c == Clave::ImanActivo));
        // Caso negativo: lo que no esta en ningun sitio no da nada.
        let estado = Estado {
            busqueda: "zzzqx".into(),
            ..Estado::default()
        };
        assert!(filas_actuales(&estado, &a, &cx).is_empty());
    }

    #[test]
    fn deshacer_vuelve_paso_a_paso_y_lo_que_no_cambia_no_cuenta() {
        let mut c = Copia {
            a: Ajustes::default(),
            historial: Vec::new(),
        };
        c.cambiar(|a| aplicar_numero(a, Clave::GifRitmo, 20));
        c.cambiar(|a| aplicar_interruptor(a, Clave::TemaCosmos));
        // Caso negativo: un cambio que no cambia nada no deja paso.
        c.cambiar(|a| aplicar_numero(a, Clave::GifRitmo, 20));
        assert_eq!(c.historial.len(), 2);
        assert!(c.deshacer());
        assert!(!c.a.tema_cosmos && c.a.gif.por_segundo == 20);
        assert!(c.deshacer());
        assert_eq!(c.a, Ajustes::default());
        assert!(!c.deshacer());
    }

    /// **Muestras de la ventana**, a PNG: Captura con cambios, la caja de
    /// anadir con foco, la busqueda, la grabacion de un atajo y Avanzado. Se
    /// dejan en `PIXPIN_MUESTRAS` (o la carpeta temporal). Fuera de
    /// pantalla: no abre nada en el escritorio.
    #[test]
    #[ignore = "necesita GPU; se mira la imagen a mano"]
    fn muestras_de_la_ventana() {
        let t = textos();
        let cx = cx(&t);
        let mut a = Ajustes::default();
        aplicar_numero(&mut a, Clave::GifRitmo, 15);
        aplicar_numero(&mut a, Clave::Caducidad, 14);
        anadir_ignorado(&mut a, "KeePassXC.exe");
        anadir_ignorado(&mut a, "BancaBCP.exe");
        a.regiones.push(pixpin_store::regiones::Region {
            nombre: "Panel de la derecha".into(),
            x: 1520,
            y: 80,
            ancho: 400,
            alto: 900,
            atajo: Some("Ctrl+Alt+1".into()),
        });
        poner_atajo(
            &mut a,
            Clave::Comando(Comando::Pinear),
            Some("Ctrl+Alt+X".parse().unwrap()),
        );
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
        let motor = pixpin_render::MotorRender::nuevo(d.d3d()).expect("motor");
        let destino = pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(
            &motor,
            d.d3d(),
            ANCHO as u32,
            ALTO as u32,
        )
        .expect("textura");
        let dir = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        std::fs::create_dir_all(&dir).expect("carpeta");
        let casos = [
            (
                "captura",
                Estado {
                    seccion: 1,
                    ..Estado::default()
                },
                Some(Golpe::Seccion(2)),
            ),
            (
                "captura-abajo",
                Estado {
                    seccion: 1,
                    desplazamiento: 9_999,
                    foco: Foco::Entrada(0),
                    entrada: "notepad".into(),
                    ..Estado::default()
                },
                None,
            ),
            (
                "busqueda",
                Estado {
                    busqueda: "segundos".into(),
                    foco: Foco::Buscador,
                    ..Estado::default()
                },
                None,
            ),
            (
                "atajos",
                Estado {
                    seccion: 4,
                    capturando: Some(3),
                    ..Estado::default()
                },
                None,
            ),
            (
                "avanzado",
                Estado {
                    seccion: 8,
                    ..Estado::default()
                },
                None,
            ),
            (
                "apps",
                Estado {
                    seccion: 6,
                    ..Estado::default()
                },
                None,
            ),
        ];
        for (nombre, estado, resaltado) in casos {
            let mut estado = estado;
            let filas = filas_actuales(&estado, &a, &cx);
            if let Foco::Entrada(_) = estado.foco {
                let i = filas
                    .iter()
                    .position(|(c, _)| *c == Clave::AnadirIgnorado)
                    .unwrap();
                estado.foco = Foco::Entrada(i);
            }
            let solo: Vec<Fila> = filas.iter().map(|(_, f)| f.clone()).collect();
            estado.desplazamiento =
                limitar_desplazamiento(estado.desplazamiento, &solo, contenido(ANCHO, ALTO));
            let vista = Vista {
                ancho: ANCHO,
                alto: ALTO,
                filas: &filas,
                estado: &estado,
                resaltado,
                hay_deshacer: true,
            };
            motor
                .dibujar(&destino.destino, |p| dibujar(p, 1.0, &vista, &t))
                .expect("pinta");
            let (w, h, pixeles) = destino.leer_rgba().expect("lee");
            assert!(pixeles.chunks(4).filter(|px| px[3] > 0).count() > 1000);
            let png = pixpin_codec::codificar_png(&pixpin_codec::ImagenRgba {
                ancho: w,
                alto: h,
                pixeles,
            })
            .expect("png");
            std::fs::write(dir.join(format!("ajustes2-{nombre}.png")), png).expect("escribe");
        }
    }
}
