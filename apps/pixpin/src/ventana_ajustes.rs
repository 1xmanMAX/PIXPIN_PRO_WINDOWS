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
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use windows::Win32::Foundation::HWND;

use crate::caja_texto::CajaLinea;
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
    Ajustes, EsquinaPila, FormatoColor, ModoDeIdiomas, MotorOcr, NivelPdf, PreferenciaIdioma,
    PreferenciaNivel, Suavizado,
};
use pixpin_store::comandos::{CATALOGO, Comando, Enlaces};
use pixpin_store::herramientas::SITIOS;
use pixpin_store::{Catalogo, Idioma, Ubicacion};
use pixpin_ui::ajustes::{
    Control, Estado, Fila, Foco, Golpe, NAV_ANCHO, Parte, Recta, ancho_de_textos, borrar_busqueda,
    boton_restablecer_seccion, botones_del_pie, buscador, coincide, contenido, golpe_en,
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

/// Los colores de las cajas de escribir de Ajustes (la caja comun).
fn colores_de_caja() -> crate::caja_texto::Colores {
    crate::caja_texto::Colores {
        texto: TINTA,
        invitacion: TENUE,
        seleccion: Color { a: 0.35, ..AZUL },
        rayita: AZUL_CLARO,
    }
}
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
/// `smart_toy` de Material: el asistente.
const ROBOT: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M20 9V7c0-1.1-.9-2-2-2h-3c0-1.66-1.34-3-3-3S9 3.34 9 5H6c-1.1 0-2 .9-2 2v2c-1.66 0-3 1.34-3 3s1.34 3 3 3v4c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2v-4c1.66 0 3-1.34 3-3s-1.34-3-3-3zM7.5 11.5c0-.83.67-1.5 1.5-1.5s1.5.67 1.5 1.5S9.83 13 9 13s-1.5-.67-1.5-1.5zM16 17H8v-2h8v2zm-1-4c-.83 0-1.5-.67-1.5-1.5S14.17 10 15 10s1.5.67 1.5 1.5S15.83 13 15 13z",
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
    Pinear,
    Tareas,
    Lienzo,
    Voz,
    Asistente,
    Atajos,
    Sincro,
    Apps,
    Apariencia,
    Avanzado,
}

const SECCIONES: [Seccion; 12] = [
    Seccion::General,
    Seccion::Captura,
    Seccion::Pinear,
    Seccion::Tareas,
    Seccion::Lienzo,
    Seccion::Voz,
    Seccion::Asistente,
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
            Seccion::Pinear => "pinear",
            Seccion::Tareas => "tareas",
            Seccion::Lienzo => "lienzo",
            Seccion::Voz => "voz",
            Seccion::Asistente => "asistente",
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
            Seccion::Pinear => &mi::PUSH_PIN,
            Seccion::Tareas => &mi::CHECK_BOX,
            Seccion::Lienzo => &mi::DRAW,
            Seccion::Voz => &mi::MIC,
            Seccion::Asistente => &ROBOT,
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
#[allow(clippy::enum_variant_names)] // ClaveNube y BorrarClave dicen lo que son
enum Clave {
    Grupo,
    Comando(Comando),
    // General
    Arranque,
    Idioma,
    /// La escala de la interfaz del chat (25-oct-2026: salio de los tres
    /// puntos del chat). No esta en `Ajustes`: vive en el estado, al lado
    /// de donde quedo la ventana (`estado.escala_interfaz`).
    EscalaInterfaz,
    /// El menu de los grupos de ventanas guardados (H9), que tambien salio
    /// de los tres puntos del chat.
    GruposVentanas,
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
    // Reconocer texto (25-oct-2026, `[ocr]`)
    /// El motor: Windows o Indra. Elegir Indra sin bajar empieza la descarga.
    OcrMotor,
    /// Corregir con el diccionario espanol.
    OcrCorregir,
    /// «Mi vocabulario».
    OcrVocabulario,
    /// Abrir la licencia de los pesos de Indra.
    OcrLicencia,
    /// Borrar el lector bajado.
    OcrBorrar,
    // Pinear (Tanda 6)
    /// Las palabras magicas de una herramienta, por su indice en
    /// `MiniApp::TODAS` (`PalabrasMagicasCard` del movil: una fila por
    /// herramienta).
    PalabrasDe(u8),
    /// «De fabrica»: todas las palabras como vienen.
    PalabrasFabrica,
    // Tareas (18-oct-2026)
    /// Las palabras de una urgencia de las tareas: 0 urgente, 1 muy urgente
    /// (`[tareas]`).
    PalabrasUrgencia(u8),
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
    // Asistente (13-oct-2026)
    /// El modelo del asistente local: 2B o 0.8B (`[asistente] modelo`).
    AsistenteModelo,
    /// Si el asistente piensa antes de contestar (`[asistente] pensar`).
    AsistentePensar,
    /// Quien contesta: local, Claude o DeepSeek (`[asistente] motor`).
    AsistenteMotor,
    /// Seguir con el local si la nube falla (`[asistente] respaldo_local`).
    AsistenteRespaldo,
    /// Gemma 4 E2B para mirar fotos (`[asistente] mejor_vista`).
    AsistenteMejorVista,
    /// Si lo hallado en los documentos puede ir a la nube
    /// (`[asistente] documentos_a_la_nube`).
    AsistenteDocsNube,
    /// Ver lo dicho al microfono del asistente antes de enviarlo
    /// (`[asistente] revisar_lo_dicho`).
    AsistenteRevisarVoz,
    /// La caja de la clave de un proveedor (indice en `Proveedor::TODOS`).
    /// No es un ajuste: la clave va cifrada a su fichero, nunca al TOML.
    ClaveNube(u8),
    /// Borrar la clave guardada de un proveedor.
    BorrarClave(u8),
    /// La caja del ID de cliente de una conexion (indice en
    /// `conexiones::SERVICIOS`). Va cifrado a su fichero, nunca al TOML.
    ClienteConexion(u8),
    /// Conectar o desconectar una conexion (Google Drive...).
    Conexion(u8),
    /// Una automatizacion del asistente, por su posicion en la lista: su
    /// interruptor y su aspa (13-oct). No es un ajuste: vive en su JSON.
    Automatizacion(u16),
    /// «Correr ahora» de una automatizacion a mano.
    CorrerAutomatizacion(u16),
    /// La fila de «ninguna todavia».
    SinAutomatizaciones,
    // Sincronizar
    Presencia,
    LoMioManda,
    // Apps predeterminadas
    AbrirCon,
    Predeterminada,
    VistaRapida,
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
                | Clave::EscalaInterfaz
                | Clave::GruposVentanas
                | Clave::SitioHerramientas
                | Clave::Carpeta
                | Clave::Ignorado(_)
                | Clave::AnadirIgnorado
                | Clave::Region(_)
                | Clave::Predeterminada
                | Clave::Fichero
                | Clave::PalabrasFabrica
                | Clave::ClaveNube(_)
                | Clave::BorrarClave(_)
                | Clave::ClienteConexion(_)
                | Clave::Conexion(_)
                | Clave::Automatizacion(_)
                | Clave::CorrerAutomatizacion(_)
                | Clave::SinAutomatizaciones
                | Clave::OcrLicencia
                | Clave::OcrBorrar
        )
    }

    /// Las cajas de escribir que guardan con «Cambiar» o Intro (las
    /// palabras de una herramienta, de una urgencia y «Mi vocabulario»).
    fn es_de_palabras(self) -> bool {
        matches!(
            self,
            Clave::PalabrasDe(_) | Clave::PalabrasUrgencia(_) | Clave::OcrVocabulario
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
    /// Las automatizaciones del asistente, leidas de su JSON al abrir y
    /// despues de cada cambio (no estan en `Ajustes`).
    automatizaciones: Vec<crate::automatizaciones::Resumen>,
    /// La escala de la interfaz del chat, en por ciento (del estado, no de
    /// `Ajustes`): se cambia al momento, sin esperar a cerrar.
    escala: u32,
}

/// Las automatizaciones de este PC, para la lista. Sin chat «Asistente»
/// todavia, ninguna (no se crea solo por mirar).
fn leer_automatizaciones(ubicacion: &Ubicacion) -> Vec<crate::automatizaciones::Resumen> {
    let raiz = ubicacion.raiz();
    let indice = pixpin_proyecto::almacen::Indice::leer(raiz);
    let Some(f) = indice.proyectos.iter().find(|f| f.es_asistente()) else {
        return Vec::new();
    };
    let aparato = codigo_del_equipo(raiz);
    match crate::automatizaciones::leer_de(&pixpin_proyecto::almacen::carpeta(raiz, &f.id)) {
        Ok(l) => crate::automatizaciones::resumenes(&l, &aparato),
        Err(e) => {
            tracing::warn!(%e, "ajustes: las automatizaciones no se entienden");
            Vec::new()
        }
    }
}

/// El codigo de este equipo (`K7Q2`), el de las automatizaciones nuevas.
fn codigo_del_equipo(raiz: &std::path::Path) -> String {
    let nombre = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PixPin Max".into());
    pixpin_proyecto::identidad::Identidad::leer_o_crear(raiz, &nombre)
        .map(|i| i.yo.codigo())
        .unwrap_or_default()
}

/// Las filas de las automatizaciones: cada una con su interruptor y su
/// aspa; las «a mano», con «Correr ahora» debajo.
fn filas_de_automatizaciones(cx: &Contexto) -> Vec<(Clave, Fila)> {
    let t = cx.t;
    let mut v = vec![(
        Clave::Grupo,
        Fila::grupo(t.t("ajustes2-g-automatizaciones")),
    )];
    if cx.automatizaciones.is_empty() {
        v.push((
            Clave::SinAutomatizaciones,
            Fila {
                etiqueta: t.t("ajustes2-automatizaciones-vacio"),
                ayuda: t.t("ajustes2-ayuda-automatizaciones-vacio"),
                control: Control::Nada,
                cambiado: false,
                quitar: false,
            },
        ));
        return v;
    }
    for (i, r) in cx.automatizaciones.iter().enumerate() {
        let i = i as u16;
        let mut args = FluentArgs::new();
        let (ayuda, control) = match (&r.rota, &r.otro_aparato) {
            (Some(m), _) => {
                args.set("motivo", m.clone());
                (
                    t.t_args("ajustes2-automatizacion-rota", &args),
                    Control::Nada,
                )
            }
            (None, Some(ap)) => {
                args.set("aparato", ap.clone());
                (
                    format!(
                        "{} · {}",
                        r.detalle,
                        t.t_args("ajustes2-automatizacion-otro-aparato", &args)
                    ),
                    Control::Interruptor(r.activa),
                )
            }
            (None, None) => (r.detalle.clone(), Control::Interruptor(r.activa)),
        };
        v.push((
            Clave::Automatizacion(i),
            Fila {
                etiqueta: r.nombre.clone(),
                ayuda,
                control,
                cambiado: false,
                quitar: true,
            },
        ));
        if r.a_mano && r.otro_aparato.is_none() && r.rota.is_none() {
            v.push((
                Clave::CorrerAutomatizacion(i),
                Fila {
                    etiqueta: format!("▶ {}", r.nombre),
                    ayuda: r.detalle.clone(),
                    control: Control::Boton(t.t("ajustes2-automatizacion-correr")),
                    cambiado: false,
                    quitar: false,
                },
            ));
        }
    }
    v
}

/// **Lo que se toca de una automatizacion** en la lista: encender o apagar
/// (`Parte::Alternar`), borrarla preguntando (`Parte::Quitar`) o correrla
/// (`Parte::Pulsar`). Devuelve si cambio la lista (hay que releerla).
fn tocar_automatizacion(
    clave: Clave,
    parte: Parte,
    cx: &Contexto,
    ubicacion: &Ubicacion,
    hwnd: HWND,
) -> bool {
    use crate::automatizaciones as au;
    let (Clave::Automatizacion(i) | Clave::CorrerAutomatizacion(i)) = clave else {
        return false;
    };
    let Some(r) = cx.automatizaciones.get(i as usize) else {
        return false;
    };
    let raiz = ubicacion.raiz();
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let aparato = codigo_del_equipo(raiz);
    let hecho = match (clave, parte) {
        (Clave::Automatizacion(_), Parte::Alternar) if r.rota.is_none() => {
            au::cambiar(raiz, &aparato, ahora, |l| {
                Ok(l.activar(&r.id, !r.activa, ahora))
            })
        }
        (Clave::Automatizacion(_), Parte::Quitar) => {
            let mut args = FluentArgs::new();
            args.set("nombre", r.nombre.clone());
            let pregunta = cx.t.t_args("ajustes2-automatizacion-borrar", &args);
            if !pixpin_shell::dialogo::confirmar_destructivo(hwnd, "PixPin · Asistente", &pregunta)
            {
                return false;
            }
            au::cambiar(raiz, &aparato, ahora, |l| Ok(l.borrar(&r.id)))
        }
        (Clave::CorrerAutomatizacion(_), Parte::Pulsar) => {
            au::motor::a_mano(&r.id);
            return false;
        }
        _ => return false,
    };
    match hecho {
        Ok(cambio) => cambio,
        Err(e) => {
            tracing::warn!(%e, "ajustes: no se pudo cambiar la automatizacion");
            false
        }
    }
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

/// Licencia de los pesos de Indra (OpenRAIL-M modificada de Datalab).
const LICENCIA_INDRA: &str = "https://huggingface.co/datalab-to/indra-ocr/blob/main/LICENSE";

/// «Reconocer texto»: el motor de toda la app (`docs/ocr.md`). Indra se baja
/// solo al elegirlo, con la licencia de sus pesos dicha antes.
fn filas_de_ocr(a: &Ajustes, cx: &Contexto) -> Vec<(Clave, Fila)> {
    let t = cx.t;
    let tt = |k: &str| t.t(k);
    let raiz = crate::ocr_motor::raiz().unwrap_or_default();
    let bajado = crate::ocr_motor::indra_listo(&raiz);
    let mut v = vec![
        grupo(t, "ajustes2-g-ocr"),
        op(
            a,
            Clave::OcrMotor,
            tt("ajustes2-ocr-motor"),
            crate::ocr_motor::estado(t, &raiz, a.ocr.motor),
            opcion(
                vec![tt("ajustes2-ocr-windows"), tt("ajustes2-ocr-indra")],
                usize::from(a.ocr.motor == MotorOcr::Indra),
            ),
        ),
        op(
            a,
            Clave::OcrLicencia,
            tt("ajustes2-ocr-licencia"),
            tt("ajustes2-ayuda-ocr-licencia"),
            Control::Boton(tt("ajustes2-abrir")),
        ),
    ];
    if a.ocr.motor == MotorOcr::Indra || bajado {
        v.push(op(
            a,
            Clave::OcrCorregir,
            tt("ajustes2-ocr-corregir"),
            tt("ajustes2-ayuda-ocr-corregir"),
            Control::Interruptor(a.ocr.corregir),
        ));
        let terminos = pixpin_indra::lexico::Vocabulario::de_texto(&a.ocr.vocabulario).terminos;
        v.push(op(
            a,
            Clave::OcrVocabulario,
            tt("ajustes2-ocr-vocabulario"),
            if terminos.is_empty() {
                tt("ajustes2-ocr-vocabulario-ninguno")
            } else {
                terminos.join(", ")
            },
            Control::Entrada {
                marcador: tt("ajustes2-ocr-vocabulario-marcador"),
                boton: tt("ajustes2-palabras-cambiar"),
            },
        ));
    }
    if bajado {
        v.push(op(
            a,
            Clave::OcrBorrar,
            tt("ajustes2-ocr-borrar"),
            tt("ajustes2-ayuda-ocr-borrar"),
            Control::Boton(tt("ajustes2-ocr-borrar-boton")),
        ));
    }
    v
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
            grupo(t, "ajustes2-g-ventanas"),
            op(
                a,
                Clave::EscalaInterfaz,
                tt("ajustes2-escala"),
                tt("ajustes2-ayuda-escala"),
                numero(
                    cx.escala,
                    pixpin_ui::chat::ESCALAS[0],
                    pixpin_ui::chat::ESCALAS[pixpin_ui::chat::ESCALAS.len() - 1],
                    // El paso de verdad es el escalon de la lista: lo da
                    // `escala_tras`, no este numero.
                    1,
                    format!("{} %", cx.escala),
                ),
            ),
            op(
                a,
                Clave::GruposVentanas,
                tt("ajustes2-grupos-ventanas"),
                tt("ajustes2-ayuda-grupos-ventanas"),
                Control::Boton(tt("ajustes2-abrir")),
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
            ];
            v.extend(filas_de_ocr(a, cx));
            v.push(grupo(t, "ajustes2-g-ignorados"));
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
        Seccion::Asistente => {
            use crate::asistente::nube as ia_nube;
            use pixpin_ia::nube::Proveedor;
            let eleccion = ia_nube::eleccion(&a.asistente);
            let mut v = vec![
                grupo(t, "ajustes2-g-asistente"),
                // Quien contesta en el chat «Asistente» y con @asistente
                // (13-oct-2026). Los modelos de la nube ya vienen elegidos:
                // solo se pide la clave.
                op(
                    a,
                    Clave::AsistenteMotor,
                    tt("ajustes2-asistente-motor"),
                    tt("ajustes2-ayuda-asistente-motor"),
                    opcion(
                        vec![
                            tt("ajustes2-asistente-local"),
                            Proveedor::Claude.nombre().to_string(),
                            Proveedor::DeepSeek.nombre().to_string(),
                        ],
                        ia_nube::OPCIONES
                            .iter()
                            .position(|e| *e == eleccion)
                            .unwrap_or(0),
                    ),
                ),
                // El asistente local del chat: el modelo que se baja y se
                // usa. Cambiarlo no baja nada: lo pide la tarjeta del chat
                // «Asistente».
                op(
                    a,
                    Clave::AsistenteModelo,
                    tt("ajustes-asistente-modelo"),
                    tt("ajustes2-ayuda-asistente-modelo"),
                    opcion(
                        vec![tt("asistente-modelo-normal"), tt("asistente-modelo-ligero")],
                        usize::from(a.asistente.modelo == pixpin_ia::modelos::QWEN35_08B.id),
                    ),
                ),
                // Que piense antes de contestar: lo que piensa se ve, plegable,
                // en la burbuja (con Claude y DeepSeek tambien).
                op(
                    a,
                    Clave::AsistentePensar,
                    tt("ajustes-asistente-pensar"),
                    tt("ajustes2-ayuda-asistente-pensar"),
                    opcion(
                        vec![
                            tt("asistente-pensar-auto"),
                            tt("asistente-pensar-si"),
                            tt("asistente-pensar-no"),
                        ],
                        match a.asistente.pensar {
                            None => 0,
                            Some(true) => 1,
                            Some(false) => 2,
                        },
                    ),
                ),
                op(
                    a,
                    Clave::AsistenteRespaldo,
                    tt("ajustes2-asistente-respaldo"),
                    tt("ajustes2-ayuda-asistente-respaldo"),
                    Control::Interruptor(a.asistente.respaldo_local),
                ),
                // El modelo de mejor vista (Gemma 4 E2B): opcional, se baja
                // aparte desde el chat «Asistente» y solo se usa para MIRAR
                // una foto (`seleccion::pide_mejor_vista`).
                op(
                    a,
                    Clave::AsistenteMejorVista,
                    tt("ajustes2-asistente-mejor-vista"),
                    crate::asistente::motor::estado_mejor_vista(t),
                    Control::Interruptor(a.asistente.mejor_vista),
                ),
                // Lo hallado en los documentos del usuario, ¿a la nube?
                // (`buscar_en_mis_cosas`; por omision se pregunta la
                // primera vez).
                op(
                    a,
                    Clave::AsistenteDocsNube,
                    tt("ajustes2-asistente-docs-nube"),
                    tt("ajustes2-ayuda-asistente-docs-nube"),
                    opcion(
                        vec![
                            tt("ajustes2-docs-nube-preguntar"),
                            tt("ajustes2-docs-nube-si"),
                            tt("ajustes2-docs-nube-no"),
                        ],
                        match crate::asistente::documentos::PermisoNube::de(
                            &a.asistente.documentos_a_la_nube,
                        ) {
                            crate::asistente::documentos::PermisoNube::Preguntar => 0,
                            crate::asistente::documentos::PermisoNube::Si => 1,
                            crate::asistente::documentos::PermisoNube::No => 2,
                        },
                    ),
                ),
                // La voz: el microfono de la caja (Whisper) y el altavoz de
                // las respuestas (el lector de voz de la app). Valen igual
                // con los tres motores.
                grupo(t, "ajustes2-g-asistente-voz"),
                op(
                    a,
                    Clave::AsistenteRevisarVoz,
                    tt("ajustes2-asistente-revisar-voz"),
                    tt("ajustes2-ayuda-asistente-revisar-voz"),
                    Control::Interruptor(a.asistente.revisar_lo_dicho),
                ),
                grupo(t, "ajustes2-g-claves"),
            ];
            for (i, p) in Proveedor::TODOS.into_iter().enumerate() {
                let mut args = FluentArgs::new();
                args.set("proveedor", p.nombre());
                v.push(op(
                    a,
                    Clave::ClaveNube(i as u8),
                    t.t_args("ajustes2-clave-de", &args),
                    ia_nube::estado(p, t),
                    Control::Entrada {
                        marcador: t.t("ajustes2-clave-marcador"),
                        boton: tt("ajustes2-clave-probar"),
                    },
                ));
                if ia_nube::hay_clave(p) {
                    v.push(op(
                        a,
                        Clave::BorrarClave(i as u8),
                        t.t_args("ajustes2-clave-borrar-de", &args),
                        tt("ajustes2-ayuda-clave-borrar"),
                        Control::Boton(tt("ajustes2-clave-borrar")),
                    ));
                }
            }
            // Conexiones (17-oct-2026): Google Drive y los que vengan, un
            // par de filas por servicio con las mismas piezas que las claves.
            v.push(grupo(t, "ajustes2-g-conexiones"));
            for (i, s) in pixpin_ia::conexiones::SERVICIOS.iter().enumerate() {
                use crate::asistente::conexiones as conex;
                let mut args = FluentArgs::new();
                args.set("servicio", s.nombre);
                v.push(op(
                    a,
                    Clave::ClienteConexion(i as u8),
                    t.t_args("ajustes2-conexion-cliente", &args),
                    conex::estado_cliente(s, t),
                    Control::Entrada {
                        marcador: t.t("ajustes2-conexion-cliente-marcador"),
                        boton: tt("ajustes2-conexion-guardar"),
                    },
                ));
                if conex::hay_cliente(s) {
                    let boton = if conex::conectado(s) || conex::conectando(s) {
                        "ajustes2-conexion-desconectar"
                    } else {
                        "ajustes2-conexion-conectar"
                    };
                    v.push(op(
                        a,
                        Clave::Conexion(i as u8),
                        t.t_args("ajustes2-conexion-de", &args),
                        conex::estado(s, t),
                        Control::Boton(tt(boton)),
                    ));
                }
            }
            v.extend(filas_de_automatizaciones(cx));
            v
        }
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
                    Clave::VistaRapida,
                    tt("ajustes2-vista-rapida"),
                    tt("ajustes2-ayuda-vista-rapida"),
                    Control::Interruptor(a.vista_rapida),
                ),
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
        Seccion::Pinear => {
            use pixpin_pin::magia::{self, MiniApp};
            let palabras = magia::leer(a.pinear.guardadas());
            let mut v = vec![grupo(t, "ajustes2-g-palabras")];
            for (i, app) in MiniApp::TODAS.into_iter().enumerate() {
                let suyas = magia::de_la_app(&palabras, app);
                v.push(op(
                    a,
                    Clave::PalabrasDe(i as u8),
                    tt(nombre_de_herramienta(app)),
                    if suyas.is_empty() {
                        tt("ajustes2-palabras-ninguna")
                    } else {
                        suyas.join(", ")
                    },
                    Control::Entrada {
                        marcador: tt("ajustes2-palabras-marcador"),
                        boton: tt("ajustes2-palabras-cambiar"),
                    },
                ));
            }
            v.push(op(
                a,
                Clave::PalabrasFabrica,
                tt("ajustes2-palabras-fabrica"),
                tt("ajustes2-ayuda-palabras"),
                Control::Boton(tt("ajustes2-palabras-volver")),
            ));
            v
        }
        Seccion::Tareas => {
            // Las palabras de cada urgencia (18-oct-2026): las mismas cajas
            // que las palabras magicas, separadas por comas.
            let mut v = vec![grupo(t, "ajustes2-g-urgencia")];
            for (i, (clave, lista)) in [
                ("ajustes2-urgente", &a.tareas.urgente),
                ("ajustes2-muy-urgente", &a.tareas.muy_urgente),
            ]
            .into_iter()
            .enumerate()
            {
                v.push(op(
                    a,
                    Clave::PalabrasUrgencia(i as u8),
                    tt(clave),
                    if lista.is_empty() {
                        tt("ajustes2-urgencia-ninguna")
                    } else {
                        lista.join(", ")
                    },
                    Control::Entrada {
                        marcador: tt("ajustes2-palabras-marcador"),
                        boton: tt("ajustes2-palabras-cambiar"),
                    },
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
        Clave::OcrMotor => a.ocr.motor = d.ocr.motor,
        Clave::OcrCorregir => a.ocr.corregir = d.ocr.corregir,
        Clave::OcrVocabulario => a.ocr.vocabulario = d.ocr.vocabulario.clone(),
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
        Clave::AsistenteModelo => a.asistente.modelo = d.asistente.modelo.clone(),
        Clave::AsistentePensar => a.asistente.pensar = d.asistente.pensar,
        Clave::AsistenteMotor => a.asistente.motor = d.asistente.motor.clone(),
        Clave::AsistenteRespaldo => a.asistente.respaldo_local = d.asistente.respaldo_local,
        Clave::AsistenteMejorVista => a.asistente.mejor_vista = d.asistente.mejor_vista,
        Clave::AsistenteDocsNube => {
            a.asistente.documentos_a_la_nube = d.asistente.documentos_a_la_nube.clone()
        }
        Clave::AsistenteRevisarVoz => a.asistente.revisar_lo_dicho = d.asistente.revisar_lo_dicho,
        Clave::Presencia => a.sincro.presencia = d.sincro.presencia,
        Clave::LoMioManda => a.sincro.lo_mio_manda = d.sincro.lo_mio_manda,
        Clave::AbrirCon => a.abrir_con = d.abrir_con,
        Clave::VistaRapida => a.vista_rapida = d.vista_rapida,
        Clave::TemaCosmos => a.tema_cosmos = d.tema_cosmos,
        Clave::Nivel => a.rendimiento.nivel = d.rendimiento.nivel,
        Clave::MedirFotogramas => a.rendimiento.medir_fotogramas = d.rendimiento.medir_fotogramas,
        Clave::Ritmo => a.rendimiento.ritmo = d.rendimiento.ritmo,
        Clave::Paneo => a.rendimiento.paneo_por_composicion = d.rendimiento.paneo_por_composicion,
        Clave::CorteMinimo => a.tinta.corte_minimo = d.tinta.corte_minimo,
        Clave::Beta => a.tinta.beta = d.tinta.beta,
        Clave::PalabrasUrgencia(0) => a.tareas.urgente = d.tareas.urgente.clone(),
        Clave::PalabrasUrgencia(_) => a.tareas.muy_urgente = d.tareas.muy_urgente.clone(),
        Clave::PalabrasDe(i) => {
            if let Some(app) = pixpin_pin::magia::MiniApp::por_indice(i) {
                let de_fabrica =
                    pixpin_pin::magia::de_la_app(&pixpin_pin::magia::por_defecto(), app);
                let de_fabrica: Vec<&str> = de_fabrica.iter().map(String::as_str).collect();
                poner_palabras(a, app, &de_fabrica);
            }
        }
        Clave::Grupo
        | Clave::EscalaInterfaz
        | Clave::GruposVentanas
        | Clave::Carpeta
        | Clave::Ignorado(_)
        | Clave::AnadirIgnorado
        | Clave::Region(_)
        | Clave::Predeterminada
        | Clave::Fichero
        | Clave::PalabrasFabrica
        | Clave::ClaveNube(_)
        | Clave::BorrarClave(_)
        | Clave::ClienteConexion(_)
        | Clave::Conexion(_)
        | Clave::Automatizacion(_)
        | Clave::CorrerAutomatizacion(_)
        | Clave::SinAutomatizaciones
        | Clave::OcrLicencia
        | Clave::OcrBorrar => {}
    }
}

/// El nombre de una herramienta, el mismo del menu «Convertir en…».
fn nombre_de_herramienta(app: pixpin_pin::magia::MiniApp) -> &'static str {
    use pixpin_pin::magia::MiniApp;
    match app {
        MiniApp::Temporizador => "pin-herramienta-temporizador",
        MiniApp::Cronometro => "pin-herramienta-cronometro",
        MiniApp::Tareas => "pin-herramienta-tareas",
        MiniApp::Contador => "pin-herramienta-contador",
        MiniApp::Gastos => "pin-herramienta-gastos",
        MiniApp::Pizarra => "pin-herramienta-pizarra",
        MiniApp::Ruleta => "pin-herramienta-ruleta",
        MiniApp::Lienzo => "pin-herramienta-lienzo",
        MiniApp::Hoja => "pin-herramienta-hoja",
    }
}

/// Cambia las palabras de una herramienta (`conLasDe`) y las deja en los
/// ajustes. Si con eso quedan las de fabrica, se guarda «de fabrica» y no
/// una copia: asi no sale el punto azul por algo que no ha cambiado.
fn poner_palabras(a: &mut Ajustes, app: pixpin_pin::magia::MiniApp, nuevas: &[&str]) {
    use pixpin_pin::magia;
    let actuales = magia::leer(a.pinear.guardadas());
    let cambiadas = magia::con_las_de(&actuales, app, nuevas);
    a.pinear = if cambiadas == magia::por_defecto() {
        pixpin_store::ajustes::Pinear::default()
    } else {
        pixpin_store::ajustes::Pinear {
            palabras_propias: true,
            palabras: magia::escribir(&cambiadas),
        }
    };
}

/// Lo que se escribe en la caja de una herramienta, partido por comas y
/// renglones como en el dialogo del movil.
fn poner_palabras_escritas(a: &mut Ajustes, clave: Clave, escrito: &str) {
    if let Clave::PalabrasDe(i) = clave
        && let Some(app) = pixpin_pin::magia::MiniApp::por_indice(i)
    {
        poner_palabras(a, app, &pixpin_pin::magia::partir_escrito(escrito));
    }
    // «Mi vocabulario»: los terminos, sin repetir, uno por linea.
    if clave == Clave::OcrVocabulario {
        a.ocr.vocabulario = pixpin_indra::lexico::Vocabulario::de_texto(escrito)
            .terminos
            .join("\n");
    }
    // Las de una urgencia: partidas igual, sin repetir y sin vacias.
    if let Clave::PalabrasUrgencia(i) = clave {
        let mut nuevas: Vec<String> = Vec::new();
        for p in pixpin_pin::magia::partir_escrito(escrito) {
            let p = p.trim().to_string();
            if !p.is_empty() && !nuevas.iter().any(|x| x.to_lowercase() == p.to_lowercase()) {
                nuevas.push(p);
            }
        }
        match i {
            0 => a.tareas.urgente = nuevas,
            _ => a.tareas.muy_urgente = nuevas,
        }
    }
}

/// Lo que sale en la caja de una herramienta al ir a escribir: sus palabras
/// de ahora, para corregirlas en vez de teclearlas de nuevo (el dialogo del
/// movil se abre con ellas puestas).
fn palabras_para_editar(a: &Ajustes, clave: Clave) -> String {
    match clave {
        Clave::PalabrasDe(i) => pixpin_pin::magia::MiniApp::por_indice(i)
            .map(|app| {
                pixpin_pin::magia::de_la_app(&pixpin_pin::magia::leer(a.pinear.guardadas()), app)
                    .join(", ")
            })
            .unwrap_or_default(),
        Clave::PalabrasUrgencia(0) => a.tareas.urgente.join(", "),
        Clave::PalabrasUrgencia(_) => a.tareas.muy_urgente.join(", "),
        Clave::OcrVocabulario => pixpin_indra::lexico::Vocabulario::de_texto(&a.ocr.vocabulario)
            .terminos
            .join(", "),
        _ => String::new(),
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
        Clave::AsistenteRespaldo => a.asistente.respaldo_local = !a.asistente.respaldo_local,
        Clave::AsistenteMejorVista => {
            a.asistente.mejor_vista = !a.asistente.mejor_vista;
            // Apagado, Gemma no se queda ocupando memoria.
            if !a.asistente.mejor_vista {
                crate::asistente::motor::soltar_mejor_vista();
            }
        }
        Clave::AsistenteRevisarVoz => a.asistente.revisar_lo_dicho = !a.asistente.revisar_lo_dicho,
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
        Clave::VistaRapida => a.vista_rapida = !a.vista_rapida,
        Clave::MedirFotogramas => a.rendimiento.medir_fotogramas = !a.rendimiento.medir_fotogramas,
        Clave::Ritmo => a.rendimiento.ritmo = !a.rendimiento.ritmo,
        Clave::Paneo => a.rendimiento.paneo_por_composicion = !a.rendimiento.paneo_por_composicion,
        Clave::OcrCorregir => a.ocr.corregir = !a.ocr.corregir,
        _ => {}
    }
}

fn aplicar_opcion(a: &mut Ajustes, clave: Clave, cual: usize) {
    match clave {
        Clave::OcrMotor => {
            a.ocr.motor = if cual == 1 {
                MotorOcr::Indra
            } else {
                MotorOcr::Windows
            };
            // Elegir Indra es pedirlo: si no esta, se baja ya (la fila dice
            // antes cuanto pesa y de quien es la licencia). Volver a Windows
            // corta una descarga a medias.
            match (a.ocr.motor, crate::ocr_motor::raiz()) {
                (MotorOcr::Indra, Some(r)) if !crate::ocr_motor::indra_listo(&r) => {
                    crate::ocr_motor::empezar_descarga(r)
                }
                (MotorOcr::Windows, _) if crate::ocr_motor::bajando() => {
                    crate::ocr_motor::cancelar_descarga()
                }
                _ => {}
            }
        }
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
        Clave::AsistenteMotor => {
            let e = crate::asistente::nube::OPCIONES
                .get(cual)
                .copied()
                .unwrap_or(crate::asistente::nube::Eleccion::Local);
            a.asistente.motor = crate::asistente::nube::id_de(e).to_string();
        }
        Clave::AsistenteDocsNube => {
            use crate::asistente::documentos::PermisoNube;
            let p = [PermisoNube::Preguntar, PermisoNube::Si, PermisoNube::No]
                .get(cual)
                .copied()
                .unwrap_or(PermisoNube::Preguntar);
            a.asistente.documentos_a_la_nube = p.clave().to_string();
        }
        Clave::AsistenteModelo => {
            a.asistente.modelo = if cual == 1 {
                pixpin_ia::modelos::QWEN35_08B.id.to_string()
            } else {
                pixpin_ia::modelos::QWEN35_2B.id.to_string()
            };
            // El motor del otro modelo se suelta ya (su memoria vuelve).
            crate::asistente::motor::soltar_todo();
        }
        Clave::AsistentePensar => {
            a.asistente.pensar = match cual {
                1 => Some(true),
                2 => Some(false),
                _ => None,
            }
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

/// Si la fila `i` es la caja de una clave de API (se pinta tapada, no se
/// copia y se vacia al salir).
fn es_clave(filas: &[(Clave, Fila)], i: usize) -> bool {
    matches!(
        filas.get(i),
        Some((Clave::ClaveNube(_) | Clave::ClienteConexion(_), _))
    )
}

/// «Guardar y probar» la clave del proveedor `n`: lo escrito, o la guardada
/// si no se escribio nada. Vuelve en seguida; la prueba va en su hilo.
fn probar_clave(n: u8, escrito: &str, despertar: std::sync::Arc<dyn Fn() + Send + Sync>) {
    let Some(p) = pixpin_ia::nube::Proveedor::TODOS.get(n as usize).copied() else {
        return;
    };
    let clave = if escrito.trim().is_empty() {
        pixpin_ia::nube::claves::leer(p).unwrap_or_default()
    } else {
        escrito.to_string()
    };
    crate::asistente::nube::guardar_y_probar(p, &clave, despertar);
}

/// Lo escrito en una caja tapada: la clave de un proveedor (guardar y
/// probar) o el ID de cliente de una conexion (guardar).
fn guardar_secreto(clave: Clave, escrito: &str, despertar: std::sync::Arc<dyn Fn() + Send + Sync>) {
    match clave {
        Clave::ClaveNube(n) => probar_clave(n, escrito, despertar),
        Clave::ClienteConexion(n) => {
            if let Some(s) = pixpin_ia::conexiones::SERVICIOS.get(n as usize) {
                crate::asistente::conexiones::guardar_cliente(s, escrito, despertar);
            }
        }
        _ => {}
    }
}

/// Conectar (o desconectar, si ya lo esta o esta esperando al navegador).
/// Vuelve en seguida: la espera va en su hilo.
fn pulsar_conexion(clave: Clave, despertar: std::sync::Arc<dyn Fn() + Send + Sync>) {
    use crate::asistente::conexiones as conex;
    let Clave::Conexion(n) = clave else { return };
    let Some(s) = pixpin_ia::conexiones::SERVICIOS.get(n as usize) else {
        return;
    };
    if conex::conectado(s) || conex::conectando(s) {
        conex::desconectar(s, despertar);
    } else {
        conex::conectar(s, despertar);
    }
}

/// La escala de la interfaz del chat que hay guardada, en un escalon valido.
fn escala_guardada(ubicacion: &Ubicacion) -> u32 {
    pixpin_ui::chat::escala_valida(
        pixpin_store::estado::cargar(ubicacion)
            .escala_interfaz
            .unwrap_or(pixpin_ui::chat::ESCALA_POR_DEFECTO),
    )
}

/// El escalon de la escala de al lado: los mismos que Ctrl + «+» / «-» en
/// el chat, y en los topes se queda donde esta.
fn escala_tras(actual: u32, subir: bool) -> u32 {
    if subir {
        pixpin_ui::chat::escala_siguiente(actual)
    } else {
        pixpin_ui::chat::escala_anterior(actual)
    }
}

/// Lo que se hace al pulsar un `Control::Boton`.
fn pulsar(clave: Clave, a: &Ajustes, ubicacion: &Ubicacion) {
    let resultado = match clave {
        // Los grupos de ventanas abren lo suyo en el hilo principal, que es
        // quien sabe lanzar cada ventana: el mismo `WM_COMMAND` de la bandeja.
        Clave::GruposVentanas => {
            if !pixpin_shell::mensajero::pedir_ventana_principal(crate::ID_GRUPOS_VENTANAS) {
                tracing::warn!("ajustes: nadie escucha para abrir los grupos de ventanas");
            }
            Ok(())
        }
        Clave::BorrarClave(i) => {
            if let Some(p) = pixpin_ia::nube::Proveedor::TODOS.get(i as usize) {
                crate::asistente::nube::borrar(*p);
            }
            Ok(())
        }
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
        Clave::OcrLicencia => pixpin_shell::abrir::abrir(std::path::Path::new(LICENCIA_INDRA)),
        Clave::OcrBorrar => {
            crate::ocr_motor::borrar(ubicacion.raiz());
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

// --- En su propio hilo --------------------------------------------------------
//
// **Por que un hilo propio (10-oct-2026).** La ventana se abria DENTRO del
// bucle del hilo principal y no volvia hasta cerrarse: mientras tanto los
// atajos y los gestos con Alt se quedaban en la cola, y al cerrar salian
// todos de golpe (seis pines seguidos que nadie pidio, con recortes de
// segundos antes). Ahora vive en su hilo, como el chat, Sincronizar o la
// galeria: el principal sigue capturando con los ajustes abiertos y, al
// cerrarse con cambios, los recoge (`tomar_cambios`) y los aplica el.

/// La ventana abierta: su HWND, -1 mientras nace, 0 si no hay ninguna. Que
/// dos peticiones seguidas no abran dos ventanas de ajustes.
static ABIERTA: AtomicIsize = AtomicIsize::new(0);
/// Los ajustes nuevos que dejo la ventana al cerrarse con cambios (ya
/// guardados en el fichero), a la espera de que el hilo principal los aplique.
static CAMBIOS: Mutex<Option<Ajustes>> = Mutex::new(None);
/// Hay una fila grabando un atajo.
static GRABANDO: AtomicBool = AtomicBool::new(false);
/// La combinacion pulsada mientras se graba que ya era un atajo registrado:
/// `RegisterHotKey` se la come antes de que llegue a esta ventana, asi que se
/// la entrega el hilo principal (`entregar_atajo`).
static ATAJO_ENTREGADO: Mutex<Option<Atajo>> = Mutex::new(None);

/// Abre la ventana de ajustes en su propio hilo, o trae delante la que ya
/// estuviera abierta. `app` es la ventana de mensajes del hilo principal: se
/// la despierta al cerrar con cambios para que los aplique.
pub fn lanzar(idioma: Idioma, actual: Ajustes, ubicacion: Ubicacion, app: isize) {
    let ya = ABIERTA.load(Ordering::SeqCst);
    if ya != 0 {
        if ya > 0 {
            VentanaOverlay::restaurar_de_hwnd(HWND(ya as *mut _));
            pixpin_shell::overlay::despertar(ya);
        }
        return;
    }
    ABIERTA.store(-1, Ordering::SeqCst);
    let lanzado = std::thread::Builder::new()
        .name("ajustes".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            let mut cambiados: Option<Ajustes> = None;
            let hecho = crate::dispositivo_perdido::con_recursos("ajustes", |r| {
                cambiados = abrir(r, &actual, &textos, &ubicacion)?;
                Ok(())
            });
            GRABANDO.store(false, Ordering::SeqCst);
            match (hecho, cambiados) {
                (Err(e), _) => tracing::warn!(?e, "no se pudo abrir la ventana de ajustes"),
                (Ok(()), None) => tracing::info!("ajustes cerrados sin cambios"),
                (Ok(()), Some(nuevos)) => {
                    if let Ok(mut c) = CAMBIOS.lock() {
                        *c = Some(nuevos);
                    }
                    pixpin_shell::despertar(HWND(app as *mut _));
                }
            }
            ABIERTA.store(0, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        ABIERTA.store(0, Ordering::SeqCst);
        tracing::warn!(?e, "no se pudo lanzar el hilo de los ajustes");
    }
}

/// Los ajustes que dejo la ventana al cerrarse con cambios, una sola vez.
/// Lo mira el hilo principal en cada vuelta de su bucle.
pub fn tomar_cambios() -> Option<Ajustes> {
    CAMBIOS.lock().ok().and_then(|mut c| c.take())
}

/// Si la ventana esta grabando un atajo ahora mismo.
pub fn grabando_atajo() -> bool {
    GRABANDO.load(Ordering::SeqCst)
}

/// Entrega a la fila que graba la combinacion de un atajo ya registrado (ver
/// `ATAJO_ENTREGADO`) y despierta la ventana para que la recoja.
pub fn entregar_atajo(atajo: Atajo) {
    if let Ok(mut a) = ATAJO_ENTREGADO.lock() {
        *a = Some(atajo);
    }
    let ya = ABIERTA.load(Ordering::SeqCst);
    if ya > 0 {
        pixpin_shell::overlay::despertar(ya);
    }
}

fn tomar_atajo_entregado() -> Option<Atajo> {
    ATAJO_ENTREGADO.lock().ok().and_then(|mut a| a.take())
}

// --- La ventana ---------------------------------------------------------------

/// Abre la ventana y no vuelve hasta que se cierra. Corre en el hilo de
/// `lanzar`, nunca en el principal.
///
/// Devuelve los ajustes nuevos si algo cambio (ya guardados en el fichero),
/// o `None` si se cerro sin tocar nada. Quien llama decide que hacer con
/// ellos: volver a registrar los atajos, sobre todo.
fn abrir(
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
    ABIERTA.store(ventana.handle().0 as isize, Ordering::SeqCst);
    // Lo que se entregara para una grabacion vieja no vale para esta.
    let _ = tomar_atajo_entregado();

    let mut cx = Contexto {
        t: textos,
        carpeta: crate::galeria_capturas::carpeta(ubicacion)
            .display()
            .to_string(),
        automatizaciones: leer_automatizaciones(ubicacion),
        escala: escala_guardada(ubicacion),
    };
    // Copia de trabajo: se toca esta y se guarda al cerrar.
    let mut copia = Copia {
        a: actual.clone(),
        historial: Vec::new(),
    };
    let mut estado = Estado::default();
    // Las dos cajas de escribir (el buscador y la de la fila con el foco),
    // con el componente comun de la app. `Estado` (de `pixpin-ui`) sigue
    // guardando el texto como `String` para su logica; ver `Cajas`.
    let mut cajas = Cajas::default();
    let mut resaltado: Option<Golpe> = None;
    let mut pintado: Option<(Estado, Option<Golpe>, u64)> = None;
    let mut version = 0u64;
    let zona = contenido(ancho, alto);
    // La prueba de una clave acaba en otro hilo: cuando cambia esto, se
    // repinta su linea de estado.
    let mut novedades_claves =
        crate::asistente::nube::novedades() + crate::asistente::conexiones::novedades();
    let hwnd_ventana = ventana.handle().0 as isize;
    let despertar_ventana: std::sync::Arc<dyn Fn() + Send + Sync> =
        std::sync::Arc::new(move || pixpin_shell::overlay::despertar(hwnd_ventana));
    // La descarga de Indra avisa a esta ventana para pintar su avance.
    crate::ocr_motor::al_cambiar(despertar_ventana.clone());

    loop {
        pixpin_shell::overlay::bombear_pendientes();
        let n = crate::asistente::nube::novedades()
            + crate::asistente::conexiones::novedades()
            + crate::ocr_motor::novedades();
        if n != novedades_claves {
            novedades_claves = n;
            version += 1;
        }
        let mut filas = filas_actuales(&estado, &copia.a, &cx);
        let mut cerrar = false;

        for (hwnd, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            // Lo que la logica cambio por su cuenta (vaciar el buscador,
            // abrir unas palabras) pasa antes a las cajas.
            cajas.traer(&estado);
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
                    let foco_antes = estado.foco;
                    let es_palabras =
                        |i: usize| filas.get(i).is_some_and(|(c, _)| c.es_de_palabras());
                    // «Cambiar» en las palabras de una herramienta: la primera
                    // vez abre su caja con lo de ahora; con la caja abierta,
                    // guarda lo escrito (el Aceptar del dialogo del movil).
                    let guardar_palabras = match golpe {
                        Some(Golpe::Fila(i, Parte::Pulsar))
                            if es_palabras(i) && foco_antes == Foco::Entrada(i) =>
                        {
                            Some(i)
                        }
                        _ => None,
                    };
                    estado.foco = match golpe {
                        Some(Golpe::Buscador) | Some(Golpe::BorrarBusqueda) => Foco::Buscador,
                        Some(Golpe::Fila(i, Parte::Escribir)) => Foco::Entrada(i),
                        // Pulsar «Añadir» deja el foco en su caja.
                        Some(Golpe::Fila(i, Parte::Pulsar))
                            if matches!(filas.get(i), Some((Clave::AnadirIgnorado, _))) =>
                        {
                            Foco::Entrada(i)
                        }
                        Some(Golpe::Fila(i, Parte::Pulsar))
                            if es_palabras(i) && guardar_palabras.is_none() =>
                        {
                            Foco::Entrada(i)
                        }
                        _ => Foco::Nada,
                    };
                    // «Guardar y probar» una clave: con lo escrito en su
                    // caja, o la guardada si no se escribio nada.
                    if let Some(Golpe::Fila(i, Parte::Pulsar)) = golpe
                        && let Some((clave @ (Clave::ClaveNube(_) | Clave::ClienteConexion(_)), _)) =
                            filas.get(i)
                    {
                        let escrito = if foco_antes == Foco::Entrada(i) {
                            std::mem::take(&mut estado.entrada)
                        } else {
                            String::new()
                        };
                        guardar_secreto(*clave, &escrito, despertar_ventana.clone());
                    }
                    if let Some(i) = guardar_palabras
                        && let Some((clave, _)) = filas.get(i).cloned()
                    {
                        let escrito = std::mem::take(&mut estado.entrada);
                        copia.cambiar(|a| poner_palabras_escritas(a, clave, &escrito));
                    } else if estado.foco != foco_antes {
                        // Al salir de la caja de unas palabras, lo escrito
                        // sin guardar se olvida (como Cancelar); al entrar
                        // en una, sale con sus palabras de ahora. Lo mismo
                        // con una clave: no se queda en memoria a la vista.
                        if let Foco::Entrada(j) = foco_antes
                            && (es_palabras(j) || es_clave(&filas, j))
                        {
                            estado.entrada.clear();
                        }
                        if let Foco::Entrada(i) = estado.foco
                            && let Some((clave, _)) = filas.get(i)
                            && clave.es_de_palabras()
                        {
                            estado.entrada = palabras_para_editar(&copia.a, *clave);
                        }
                    }
                    // En la caja de escribir con el foco: la rayita donde
                    // cae el clic (dos, la palabra; tres, todo).
                    cajas.traer(&estado);
                    if let Some(caja) = cajas.con_foco(estado.foco) {
                        caja.pulsar(
                            Punto {
                                x: p.x - marco.x,
                                y: p.y - marco.y,
                            },
                            pixpin_shell::modificadores_pulsados().shift,
                            std::time::Instant::now(),
                        );
                        caja.soltar_raton();
                    }
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
                                    // Las automatizaciones no son ajustes:
                                    // se cambian en su JSON al momento.
                                    Parte::Alternar | Parte::Quitar | Parte::Pulsar
                                        if matches!(
                                            clave,
                                            Clave::Automatizacion(_)
                                                | Clave::CorrerAutomatizacion(_)
                                        ) =>
                                    {
                                        if tocar_automatizacion(
                                            clave,
                                            parte,
                                            &cx,
                                            ubicacion,
                                            ventana.handle(),
                                        ) {
                                            cx.automatizaciones = leer_automatizaciones(ubicacion);
                                        }
                                    }
                                    // La escala no es de `Ajustes`: se pone
                                    // ya, y el chat abierto la coge al vuelo.
                                    // Desde la guardada, no desde la que se
                                    // leyo al abrir: con Ctrl + «+» en el
                                    // chat pudo cambiar mientras tanto.
                                    Parte::Menos | Parte::Mas if clave == Clave::EscalaInterfaz => {
                                        cx.escala = escala_tras(
                                            escala_guardada(ubicacion),
                                            parte == Parte::Mas,
                                        );
                                        crate::ventana_chat::poner_escala_interfaz(
                                            ubicacion, cx.escala,
                                        );
                                    }
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
                                    // Ya resuelto arriba, con el foco.
                                    Parte::Pulsar
                                        if matches!(
                                            clave,
                                            Clave::PalabrasDe(_)
                                                | Clave::PalabrasUrgencia(_)
                                                | Clave::OcrVocabulario
                                                | Clave::ClaveNube(_)
                                                | Clave::ClienteConexion(_)
                                        ) => {}
                                    Parte::Pulsar if matches!(clave, Clave::Conexion(_)) => {
                                        pulsar_conexion(clave, despertar_ventana.clone())
                                    }
                                    Parte::Pulsar if clave == Clave::PalabrasFabrica => copia
                                        .cambiar(|a| {
                                            a.pinear = pixpin_store::ajustes::Pinear::default()
                                        }),
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
                    // En la caja con el foco (la comun de la app).
                    let escrito = estado.capturando.is_none()
                        && cajas
                            .con_foco(estado.foco)
                            .is_some_and(|caja| caja.caracter(c));
                    if escrito {
                        if estado.foco == Foco::Buscador {
                            estado.desplazamiento = 0;
                        }
                        cajas.llevar(&mut estado);
                        version += 1;
                    }
                }
                // Las teclas de una caja de texto (flechas, Inicio/Fin,
                // Retroceso y Supr con o sin Ctrl, Ctrl+A/C/X/V/Z/Y...) son de
                // la caja con el foco; Escape e Intro siguen abajo.
                EventoOverlay::Tecla {
                    vk, shift, ctrl, ..
                } if estado.capturando.is_none()
                        && estado.foco != Foco::Nada
                        && CajaLinea::es_suya(vk, shift, ctrl)
                        // De la caja de una clave no se copia ni se corta.
                        && !(ctrl
                            && (vk == u32::from(b'C') || vk == u32::from(b'X'))
                            && matches!(estado.foco, Foco::Entrada(i) if es_clave(&filas, i))) =>
                {
                    if let Some(caja) = cajas.con_foco(estado.foco)
                        && caja
                            .tecla(vk, shift, ctrl, &mut crate::caja_texto::PortapapelesWindows)
                            .cambio()
                    {
                        if estado.foco == Foco::Buscador {
                            estado.desplazamiento = 0;
                        }
                        cajas.llevar(&mut estado);
                    }
                    version += 1;
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
                                    // Escape en las palabras es Cancelar.
                                    if let Foco::Entrada(i) = estado.foco
                                        && (filas.get(i).is_some_and(|(c, _)| c.es_de_palabras())
                                            || es_clave(&filas, i))
                                    {
                                        estado.entrada.clear();
                                    }
                                    estado.foco = Foco::Nada;
                                }
                            }
                            0x0D => match estado.foco {
                                // Intro en una clave: guardar y probar.
                                Foco::Entrada(i) if es_clave(&filas, i) => {
                                    let texto = std::mem::take(&mut estado.entrada);
                                    if let Some((clave, _)) = filas.get(i) {
                                        guardar_secreto(*clave, &texto, despertar_ventana.clone());
                                    }
                                    estado.foco = Foco::Nada;
                                }
                                Foco::Entrada(i)
                                    if matches!(filas.get(i), Some((Clave::AnadirIgnorado, _))) =>
                                {
                                    let texto = std::mem::take(&mut estado.entrada);
                                    copia.cambiar(|a| {
                                        anadir_ignorado(a, &texto);
                                    });
                                }
                                // Intro guarda las palabras y cierra la caja.
                                Foco::Entrada(i)
                                    if filas.get(i).is_some_and(|(c, _)| c.es_de_palabras()) =>
                                {
                                    let texto = std::mem::take(&mut estado.entrada);
                                    if let Some((clave, _)) = filas.get(i).cloned() {
                                        copia
                                            .cambiar(|a| poner_palabras_escritas(a, clave, &texto));
                                    }
                                    estado.foco = Foco::Nada;
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
        // Un atajo ya registrado pulsado mientras se graba: no llega como
        // tecla (se lo come RegisterHotKey), lo entrega el hilo principal.
        if let Some(atajo) = tomar_atajo_entregado() {
            if let Some(i) = estado.capturando.take() {
                if let Some((clave, _)) = filas.get(i).cloned() {
                    copia.cambiar(|a| poner_atajo(a, clave, Some(atajo)));
                }
                version += 1;
            }
        }
        GRABANDO.store(estado.capturando.is_some(), Ordering::SeqCst);
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
                cajas: &cajas,
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

/// **Las cajas de escribir de Ajustes** con el componente comun de la app
/// (`caja_texto::CajaLinea`): la rayita donde se escribe, Ctrl+Retroceso,
/// Mayus+flechas, Ctrl+A/C/X/V/Z, el raton... `Estado` vive en `pixpin-ui`
/// (sin pintor ni portapapeles) y guarda lo escrito como `String` para su
/// logica; estas cajas son las que se editan y se pintan, y cada cambio se
/// copia alli. La rayita va quieta: esta ventana solo repinta al cambiar
/// algo.
struct Cajas {
    busqueda: CajaLinea,
    entrada: CajaLinea,
}

impl Default for Cajas {
    fn default() -> Self {
        let caja = || CajaLinea::nueva().sin_parpadeo().con_maximo(MAXIMO_CAJA);
        Cajas {
            busqueda: caja(),
            entrada: caja(),
        }
    }
}

impl Cajas {
    /// Lo que la logica de `pixpin-ui` cambio por su cuenta (vaciar el
    /// buscador, abrir las palabras de una herramienta) pasa a las cajas.
    fn traer(&mut self, e: &Estado) {
        for (caja, texto) in [
            (&mut self.busqueda, &e.busqueda),
            (&mut self.entrada, &e.entrada),
        ] {
            if caja.texto() != texto.as_str() {
                *caja = CajaLinea::nueva().sin_parpadeo().con_maximo(MAXIMO_CAJA);
                caja.poner_texto(texto);
            }
        }
    }

    /// La caja que tiene el foco, si es una de escribir.
    fn con_foco(&mut self, foco: Foco) -> Option<&mut CajaLinea> {
        match foco {
            Foco::Buscador => Some(&mut self.busqueda),
            Foco::Entrada(_) => Some(&mut self.entrada),
            Foco::Nada => None,
        }
    }

    /// Lo escrito en las cajas, de vuelta a `Estado`.
    fn llevar(&self, e: &mut Estado) {
        e.busqueda = self.busqueda.texto().to_owned();
        e.entrada = self.entrada.texto().to_owned();
    }
}

/// Lo mas largo de una caja de Ajustes (lo de `pixpin_ui::ajustes::escribir`).
/// 256 y no 120: una clave de API de Claude ya pasa de 100 letras.
const MAXIMO_CAJA: usize = 256;

// --- El pintado -----------------------------------------------------------------

/// Lo que hace falta para pintar un fotograma.
struct Vista<'a> {
    ancho: f32,
    alto: f32,
    filas: &'a [(Clave, Fila)],
    estado: &'a Estado,
    cajas: &'a Cajas,
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
    // La caja de una linea comun: la rayita donde se escribe, lo elegido y
    // el texto corrido si no cabe.
    let hueco = RectF {
        x: x_texto,
        ancho: (b.ancho - 34.0 * e - 44.0 * e).max(1.0),
        ..b
    };
    v.cajas.busqueda.pintar(
        p,
        hueco,
        hueco,
        tam,
        &t.t("ajustes2-buscar"),
        &colores_de_caja(),
        enfocado,
        std::time::Instant::now(),
    );
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
    // Las secciones llegan hasta «Deshacer» (donde las acierta `golpe_en`);
    // la nota de abajo solo se pinta si queda sitio (con «Asistente» son
    // once y en 680 de alto no caben las dos cosas).
    let mut fondo_secciones = 0.0f32;
    for (i, s) in SECCIONES.iter().enumerate() {
        let r = rect_de_seccion(i);
        if r.y + r.alto > deshacer.y - 6.0 {
            break;
        }
        fondo_secciones = r.y + r.alto;
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
    if fondo_secciones <= deshacer.y - 50.0 {
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
    }

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
                    Control::Entrada { boton, .. }
                        if matches!(clave, Clave::ClaveNube(_) | Clave::ClienteConexion(_)) =>
                    {
                        boton.clone()
                    }
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
                // Lo escrito, solo en la caja que tiene el foco: hay varias
                // (programas, palabras) y comparten el mismo hueco. Es la
                // caja de una linea comun, con su rayita donde se escribe.
                let hueco = RectF {
                    x: caja.x + 10.0 * e,
                    ancho: (caja.ancho - 20.0 * e).max(1.0),
                    ..caja
                };
                if enfocada && matches!(clave, Clave::ClaveNube(_) | Clave::ClienteConexion(_)) {
                    // Una clave se escribe tapada: puntos y la rayita al final
                    // (sin elegir trozos: se pega entera y se prueba).
                    let n = v.cajas.entrada.texto().chars().count();
                    let tapado = if n == 0 {
                        String::new()
                    } else {
                        "•".repeat(n.min(64))
                    };
                    let (w, h) = p.medir_texto(if n == 0 { marcador } else { &tapado }, 13.0 * e);
                    p.con_recorte(hueco, |p| {
                        let y = caja.y + (caja.alto - h) / 2.0;
                        if n == 0 {
                            p.texto(marcador, hueco.x, y, 13.0 * e, TENUE);
                        } else {
                            p.texto(&tapado, hueco.x, y, 13.0 * e, TINTA);
                        }
                        let x = hueco.x + if n == 0 { 0.0 } else { w } + 1.0;
                        p.rellenar(
                            RectF {
                                x,
                                y,
                                ancho: (1.5 * e).max(1.0),
                                alto: h,
                            },
                            AZUL_CLARO,
                        );
                    });
                } else if enfocada {
                    v.cajas.entrada.pintar(
                        p,
                        caja,
                        hueco,
                        13.0 * e,
                        marcador,
                        &colores_de_caja(),
                        true,
                        std::time::Instant::now(),
                    );
                } else {
                    let (_, h) = p.medir_texto(marcador, 13.0 * e);
                    p.con_recorte(hueco, |p| {
                        p.texto(
                            marcador,
                            hueco.x,
                            caja.y + (caja.alto - h) / 2.0,
                            13.0 * e,
                            TENUE,
                        )
                    });
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
            automatizaciones: Vec::new(),
            escala: pixpin_ui::chat::ESCALA_POR_DEFECTO,
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
            Clave::AsistenteModelo,
            Clave::AsistenteMejorVista,
            Clave::AsistenteDocsNube,
            Clave::AsistentePensar,
            Clave::Presencia,
            Clave::LoMioManda,
            Clave::AbrirCon,
            Clave::Predeterminada,
            Clave::VistaRapida,
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

    /// Ajustes → Tareas (18-oct-2026): las palabras de cada urgencia se
    /// cambian en su caja, ponen el punto azul y vuelven a fabrica.
    #[test]
    fn las_palabras_de_cada_urgencia_se_cambian_y_vuelven_a_fabrica() {
        let t = textos();
        let cx = cx(&t);
        let mut a = Ajustes::default();
        let filas = filas_de_seccion(Seccion::Tareas, &a, &cx);
        assert_eq!(filas.len(), 3, "el grupo y una fila por urgencia");
        assert_eq!(filas[1].1.etiqueta, "Urgente");
        assert_eq!(filas[2].1.etiqueta, "Muy urgente");
        assert!(
            filas[1].1.ayuda.contains("prioridad alta"),
            "{}",
            filas[1].1.ayuda
        );
        assert!(!filas[1].1.cambiado && !filas[2].1.cambiado);
        assert!(palabras_para_editar(&a, Clave::PalabrasUrgencia(1)).contains("!!"));
        poner_palabras_escritas(
            &mut a,
            Clave::PalabrasUrgencia(0),
            "pronto, Pronto,, ya mismo\n",
        );
        assert_eq!(
            a.tareas.urgente,
            ["pronto", "ya mismo"],
            "sin repetidas ni vacias"
        );
        let filas = filas_de_seccion(Seccion::Tareas, &a, &cx);
        assert!(filas[1].1.cambiado);
        assert!(!filas[2].1.cambiado, "caso negativo: muy urgente no cambio");
        // Sin ninguna: valido («todo normal») y se dice.
        poner_palabras_escritas(&mut a, Clave::PalabrasUrgencia(1), " ");
        assert!(a.tareas.muy_urgente.is_empty());
        let filas = filas_de_seccion(Seccion::Tareas, &a, &cx);
        assert_eq!(filas[2].1.ayuda, t.t("ajustes2-urgencia-ninguna"));
        restablecer_seccion(Seccion::Tareas, &mut a, &cx);
        assert_eq!(a.tareas, pixpin_store::ajustes::Tareas::default());
    }

    #[test]
    fn las_palabras_de_una_herramienta_se_cambian_y_vuelven_a_fabrica() {
        let t = textos();
        let cx = cx(&t);
        let mut a = Ajustes::default();
        let filas = filas_de_seccion(Seccion::Pinear, &a, &cx);
        // Una fila por herramienta y la de fabrica, sin punto azul.
        assert_eq!(
            filas
                .iter()
                .filter(|(c, _)| matches!(c, Clave::PalabrasDe(_)))
                .count(),
            pixpin_pin::magia::MiniApp::TODAS.len()
        );
        let (_, reloj) = &filas[1];
        assert_eq!(reloj.etiqueta, "Temporizador");
        assert!(reloj.ayuda.contains("pomodoro"), "{}", reloj.ayuda);
        // La caja se abre con las de ahora.
        assert!(palabras_para_editar(&a, Clave::PalabrasDe(0)).contains("time"));
        // Cambiar: punto azul, y lo guardado es el formato del movil.
        poner_palabras_escritas(&mut a, Clave::PalabrasDe(0), "Reloj, pomodoro");
        assert_eq!(
            a.pinear.guardadas().map(|g| g.contains("reloj=TIMER")),
            Some(true)
        );
        let filas = filas_de_seccion(Seccion::Pinear, &a, &cx);
        assert!(filas[1].1.cambiado);
        assert!(
            !filas[2].1.cambiado,
            "caso negativo: el cronometro no cambio"
        );
        // Restablecer esa fila deja TODO de fabrica, no una copia.
        restablecer(Clave::PalabrasDe(0), &mut a);
        assert_eq!(a.pinear, pixpin_store::ajustes::Pinear::default());
        // Sin ninguna palabra es una respuesta valida y distinta de fabrica.
        poner_palabras_escritas(&mut a, Clave::PalabrasDe(6), "");
        let mias = pixpin_pin::magia::leer(a.pinear.guardadas());
        assert_eq!(
            pixpin_pin::magia::detectar("ruleta", &mias),
            None,
            "la ruleta se quedo sin palabras"
        );
        restablecer_seccion(Seccion::Pinear, &mut a, &cx);
        assert_eq!(a.pinear.guardadas(), None);
        // Y el buscador las encuentra por su palabra.
        let estado = Estado {
            busqueda: "pomodoro".into(),
            ..Estado::default()
        };
        assert!(
            filas_actuales(&estado, &a, &cx)
                .iter()
                .any(|(c, _)| *c == Clave::PalabrasDe(0))
        );
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
                    seccion: 7,
                    capturando: Some(3),
                    ..Estado::default()
                },
                None,
            ),
            (
                "avanzado",
                Estado {
                    seccion: 11,
                    ..Estado::default()
                },
                None,
            ),
            (
                "apps",
                Estado {
                    seccion: 9,
                    ..Estado::default()
                },
                None,
            ),
            (
                "asistente",
                Estado {
                    seccion: 6,
                    ..Estado::default()
                },
                None,
            ),
            (
                "pinear",
                Estado {
                    seccion: 2,
                    foco: Foco::Entrada(0),
                    entrada: "reloj, pomodoro".into(),
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
                    .position(|(c, _)| *c == Clave::AnadirIgnorado || *c == Clave::PalabrasDe(0))
                    .unwrap();
                estado.foco = Foco::Entrada(i);
            }
            let solo: Vec<Fila> = filas.iter().map(|(_, f)| f.clone()).collect();
            estado.desplazamiento =
                limitar_desplazamiento(estado.desplazamiento, &solo, contenido(ANCHO, ALTO));
            let mut cajas = Cajas::default();
            cajas.traer(&estado);
            let vista = Vista {
                ancho: ANCHO,
                alto: ALTO,
                filas: &filas,
                estado: &estado,
                cajas: &cajas,
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

    /// Unas automatizaciones de ejemplo, leidas por el mismo camino que el
    /// fichero de verdad (una de otro aparato y una rota incluidas).
    fn automatizaciones_de_ejemplo() -> Vec<crate::automatizaciones::Resumen> {
        let json = r#"{"version":1,"automatizaciones":[
            {"id":"a1","nombre":"Capturas a la obra con OCR","activa":true,"aparato":"PC01",
             "disparador":{"tipo":"al_capturar"},
             "pasos":[{"tipo":"accion","herramienta":"escribir_en_chat","args":{"texto":"Captura {hora}"},"proyecto":"Obra","imagenes":["{captura}"]},
                      {"tipo":"asistente","pedido":"Hazle OCR a la captura"}]},
            {"id":"a2","nombre":"Resumen de pendientes","activa":true,"aparato":"PC01",
             "disparador":{"tipo":"a_la_hora","hora":"08:00","dias":[1,2,3,4,5]},
             "pasos":[{"tipo":"asistente","pedido":"Resume mis tareas pendientes"}]},
            {"id":"a3","nombre":"Lo del móvil al Inbox","activa":false,"aparato":"PC01",
             "disparador":{"tipo":"al_sincronizar"},
             "pasos":[{"tipo":"accion","herramienta":"anadir_tarea","args":{"texto":"Revisar lo que llegó: {sincro}"}}]},
            {"id":"a4","nombre":"Informe semanal","activa":true,"aparato":"PC01",
             "disparador":{"tipo":"a_mano"},
             "pasos":[{"tipo":"asistente","pedido":"Haz un resumen de la semana"}]},
            {"id":"a5","nombre":"Recordar fotos","activa":true,"aparato":"TEL9",
             "disparador":{"tipo":"a_la_hora","hora":"19:30"},
             "pasos":[{"tipo":"asistente","pedido":"¿Subí las fotos de hoy?"}]},
            {"id":"a6","nombre":"De una versión nueva","disparador":{"tipo":"al_llegar_correo"},"pasos":[]}
        ]}"#;
        let l = crate::automatizaciones::Lista::de_json(json).unwrap();
        crate::automatizaciones::resumenes(&l, "PC01")
    }

    #[test]
    fn las_automatizaciones_salen_en_el_asistente_con_su_interruptor_y_su_aspa() {
        let t = textos();
        let a = Ajustes::default();
        // Sin ninguna, una fila que dice como pedirlas.
        let filas = filas_de_seccion(Seccion::Asistente, &a, &cx(&t));
        let (c, f) = filas.last().unwrap();
        assert_eq!(*c, Clave::SinAutomatizaciones);
        assert!(f.ayuda.contains("cada vez que capture"), "{}", f.ayuda);
        let mut cx = cx(&t);
        cx.automatizaciones = automatizaciones_de_ejemplo();
        let filas = filas_de_seccion(Seccion::Asistente, &a, &cx);
        let autos: Vec<&(Clave, Fila)> = filas
            .iter()
            .filter(|(c, _)| matches!(c, Clave::Automatizacion(_)))
            .collect();
        assert_eq!(autos.len(), 6);
        assert!(autos.iter().all(|(_, f)| f.quitar && !f.cambiado));
        assert_eq!(autos[0].1.control, Control::Interruptor(true));
        assert_eq!(autos[2].1.control, Control::Interruptor(false));
        assert!(
            autos[1]
                .1
                .ayuda
                .starts_with("lun, mar, mié, jue, vie a las 08:00"),
            "{}",
            autos[1].1.ayuda
        );
        assert!(
            autos[4].1.ayuda.contains("corre en TEL9"),
            "{}",
            autos[4].1.ayuda
        );
        // Caso negativo: la rota no se enciende, dice por que.
        assert_eq!(autos[5].1.control, Control::Nada);
        assert!(
            autos[5].1.ayuda.starts_with("No se entiende"),
            "{}",
            autos[5].1.ayuda
        );
        // Solo la «a mano» de aqui tiene «Correr ahora».
        let correr: Vec<_> = filas
            .iter()
            .filter(|(c, _)| matches!(c, Clave::CorrerAutomatizacion(_)))
            .collect();
        assert_eq!(correr.len(), 1);
        assert_eq!(correr[0].0, Clave::CorrerAutomatizacion(3));
        // No son ajustes: restablecer no las toca.
        assert!(!Clave::Automatizacion(0).restablecible());
    }

    /// **La lista de automatizaciones, pintada a PNG** (`PIXPIN_MUESTRAS`).
    #[test]
    #[ignore = "necesita GPU; se mira la imagen a mano"]
    fn muestra_de_las_automatizaciones() {
        let t = textos();
        let mut cx = cx(&t);
        cx.automatizaciones = automatizaciones_de_ejemplo();
        let a = Ajustes::default();
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
        let mut estado = Estado {
            seccion: SECCIONES
                .iter()
                .position(|s| *s == Seccion::Asistente)
                .unwrap(),
            desplazamiento: 9_999,
            ..Estado::default()
        };
        let filas = filas_actuales(&estado, &a, &cx);
        let solo: Vec<Fila> = filas.iter().map(|(_, f)| f.clone()).collect();
        estado.desplazamiento =
            limitar_desplazamiento(estado.desplazamiento, &solo, contenido(ANCHO, ALTO));
        let mut cajas = Cajas::default();
        cajas.traer(&estado);
        let vista = Vista {
            ancho: ANCHO,
            alto: ALTO,
            filas: &filas,
            estado: &estado,
            cajas: &cajas,
            resaltado: None,
            hay_deshacer: false,
        };
        motor
            .dibujar(&destino.destino, |p| dibujar(p, 1.0, &vista, &t))
            .expect("pinta");
        let (w, h, pixeles) = destino.leer_rgba().expect("lee");
        let png = pixpin_codec::codificar_png(&pixpin_codec::ImagenRgba {
            ancho: w,
            alto: h,
            pixeles,
        })
        .expect("png");
        std::fs::write(dir.join("ajustes2-automatizaciones.png"), png).expect("escribe");
    }
}
