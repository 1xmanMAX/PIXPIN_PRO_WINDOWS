//! **Los documentos del usuario como herramientas del asistente**
//! (14-oct-2026, pedido «integracion completa con la app»):
//!
//! 1. una foto o un PDF de una tabla → una tabla del chat o un Excel;
//! 2. anotar un PDF, un Word o un plano (resaltar o subrayar un texto, una
//!    nota, encerrar una zona, dibujar), en las MISMAS capas que el lector y
//!    el visor de planos, para que se vea al abrir y viaje al movil;
//! 3. buscar un texto en un plano o un PDF y abrir el visor ahi;
//! 4. abrir un archivo por su nombre o lo que trata («el plano de
//!    senalizacion de Tahuantinsuyo»), sin pedir rutas ni codigos;
//! 5. exportar por el componente «Exportar» comun, en el formato pedido.
//!
//! Son filas de la **misma tabla** que el resto de la app ([`Accion`]):
//! mismo generador de JSON Schema, misma validacion y mismas claves para
//! elegirlas. Lo distinto es quien las hace: no son un pedido que se manda
//! tal cual, porque necesitan los adjuntos del chat y devolverle al modelo
//! lo que salio («12 filas y 4 columnas», «3 sitios: ...»). Las hace el
//! ejecutor del chat (`apps/pixpin/src/asistente/documentos_acciones.rs`),
//! que acaba mandando pedidos normales del protocolo (`tabla_nueva`,
//! `abrir` con `buscar`/`zona`, `chat` con `archivos`), asi el camino para
//! escribir en los cuadernos sigue siendo uno.
//!
//! **El modelo decide poco a proposito**: en la tabla, solo «tabla o Excel»,
//! el nombre y la cabecera (las celdas las saca un algoritmo,
//! [`crate::tabla_escaneada`]); al anotar, que y donde por el TEXTO del
//! documento, no por coordenadas que no ve. Menos que inventar, menos que
//! esperar.

use crate::acciones::{Accion, Categoria, Param, Tipo};

const fn p(nombre: &'static str, tipo: Tipo, obligatorio: bool, descripcion: &'static str) -> Param {
    Param {
        nombre,
        destino: "",
        tipo,
        obligatorio,
        descripcion,
        defecto: None,
        largo: None,
    }
}

const fn d(defecto: &'static str, mut p: Param) -> Param {
    p.defecto = Some(defecto);
    p
}

const ARCHIVO: &str = "nombre o descripcion del documento (ej. «plano de senalizacion Tahuantinsuyo»); omitir: el adjunto mas reciente del chat";

pub const TABLA: &str = "tabla_de_imagen";
pub const ANOTAR: &str = "anotar_documento";
pub const BUSCAR_EN: &str = "buscar_en_documento";
pub const ABRIR: &str = "abrir_por_nombre";
pub const EXPORTAR: &str = "exportar_documento";

/// Lo que se puede hacer al anotar.
pub const TIPOS_DE_ANOTACION: &[&str] = &["resaltar", "subrayar", "tachar", "nota", "encerrar", "marco", "dibujar"];

/// Los formatos de «Exportar» (los ids de `compartir`).
pub const FORMATOS: &[&str] = &["pdf", "png", "jpg", "svg", "web", "dxf", "csv", "texto", "md", "original"];

/// **La tabla de los documentos.** Una fila por herramienta.
pub const ACCIONES_DOCUMENTOS: &[Accion] = &[
    Accion {
        herramienta: TABLA,
        accion: TABLA,
        descripcion: "Pasa la tabla de una foto o un PDF (escaneado o no) a una tabla del chat o a un Excel. Las celdas las lee PixPin; tu solo eliges destino, nombre y si la primera fila es cabecera.",
        categoria: Categoria::Notas,
        claves: &["tabla", "excel", "xlsx", "hoja de calculo", "celdas", "columnas", "filas"],
        fijos: "{}",
        params: &[
            d(r#""tabla""#, p("destino", Tipo::Opciones(&["tabla", "excel"]), false, "tabla del chat o fichero Excel")),
            p("nombre", Tipo::Texto, false, "nombre de la tabla"),
            p("cabecera", Tipo::SiNo, false, "si la primera fila son titulos; omitir si no se sabe"),
            p("archivo", Tipo::Texto, false, ARCHIVO),
            p("pagina", Tipo::Entero, false, "pagina del PDF, desde 1; omitir: la primera con tabla"),
        ],
        confirmar: None,
        // Lo hace `documentos_acciones` de la app y su resultado es la respuesta.
        hecho: "Hecho.",
    },
    Accion {
        herramienta: ANOTAR,
        accion: ANOTAR,
        descripcion: "Anota un PDF, Word o plano del chat donde lo vera el usuario al abrirlo (y en el movil): resaltar, subrayar o tachar un texto; encerrar o marcar (marco) un texto o una zona; poner una nota; dibujar un trazo.",
        categoria: Categoria::Notas,
        claves: &[
            "resalta", "subraya", "tacha", "anota", "anotale", "encierra", "rodea", "circula", "marca en", "marca el",
            "marca la", "pon una nota", "ponle una nota", "nota en", "senala", "dibuja sobre", "dibuja en el plano",
            "dibuja en el pdf", "en la pagina",
        ],
        fijos: "{}",
        params: &[
            p("tipo", Tipo::Opciones(TIPOS_DE_ANOTACION), true, "que se hace"),
            p("texto", Tipo::Texto, false, "el texto del documento sobre el que va (se busca)"),
            p("nota", Tipo::Texto, false, "lo que dice la nota"),
            p("archivo", Tipo::Texto, false, ARCHIVO),
            p("pagina", Tipo::Entero, false, "pagina, desde 1; omitir: donde este el texto"),
            p("zona", Tipo::Numeros, false, "[x, y, ancho, alto] en fracciones 0-1 de la pagina o del plano, si no hay texto"),
            p("puntos", Tipo::Numeros, false, "dibujar: [x1, y1, x2, y2, ...] en fracciones 0-1"),
            p("color", Tipo::Texto, false, "amarillo, rojo, verde, azul... omitir: el de siempre"),
        ],
        confirmar: None,
        // Lo hace `documentos_acciones` de la app y su resultado es la respuesta.
        hecho: "Hecho.",
    },
    Accion {
        herramienta: BUSCAR_EN,
        accion: BUSCAR_EN,
        descripcion: "Busca un texto dentro de un plano (DWG/DXF), PDF o Word y abre el visor ahi, ampliado y marcado. Usala para «¿donde dice...?» o «llevame a...».",
        categoria: Categoria::Abrir,
        claves: &[
            "donde dice", "donde sale", "donde aparece", "donde pone", "busca en el plano",
            "busca en el pdf", "busca en el documento", "llevame", "lleva a", "encuentra en", "ubica", "localiza",
        ],
        fijos: "{}",
        params: &[
            p("texto", Tipo::Texto, true, "lo que se busca, tal cual"),
            p("archivo", Tipo::Texto, false, ARCHIVO),
            d("true", p("abrir", Tipo::SiNo, false, "abrir el visor en lo encontrado")),
        ],
        confirmar: None,
        // Lo hace `documentos_acciones` de la app y su resultado es la respuesta.
        hecho: "Hecho.",
    },
    Accion {
        herramienta: ABRIR,
        accion: ABRIR,
        descripcion: "Abre un documento, plano, Excel o imagen de los chats del usuario por su nombre o de que trata. Si hay varios parecidos te los dice para que preguntes cual.",
        categoria: Categoria::Abrir,
        claves: &[
            "abre el", "abre la", "abre mi", "abreme", "abrir el", "abrir la", "muestrame el", "muestrame la",
            "ensename el", "ensename la", "plano de", "pdf de", "documento de", "archivo de", "excel de",
        ],
        fijos: "{}",
        params: &[p("descripcion", Tipo::Texto, true, "el nombre o lo que trata, con las palabras del usuario")],
        confirmar: None,
        // Lo hace `documentos_acciones` de la app y su resultado es la respuesta.
        hecho: "Hecho.",
    },
    Accion {
        herramienta: EXPORTAR,
        accion: EXPORTAR,
        descripcion: "Exporta un documento, plano, Excel o imagen (con lo anotado) en otro formato y lo deja en el chat.",
        categoria: Categoria::Abrir,
        claves: &["exporta", "exportar", "exportalo", "guarda como", "guardalo como", "convierte a", "convertir a", "pasalo a", "en pdf", "a pdf", "a png", "a dxf", "como imagen", "en png"],
        fijos: "{}",
        params: &[
            p("formato", Tipo::Opciones(FORMATOS), true, "el formato"),
            p("archivo", Tipo::Texto, false, ARCHIVO),
        ],
        confirmar: None,
        // Lo hace `documentos_acciones` de la app y su resultado es la respuesta.
        hecho: "Hecho.",
    },
];

/// Si la herramienta es de esta tabla (la hace el ejecutor del chat, no se
/// manda como pedido).
pub fn es_de_documentos(herramienta: &str) -> bool {
    ACCIONES_DOCUMENTOS.iter().any(|a| a.herramienta == herramienta)
}

/// **Para una pregunta** («¿donde dice JR. ANTISUYO en el plano?»): las de
/// esta tabla que encajan bien (una frase entera de sus claves). La
/// seleccion las pone antes que la ayuda, que si no se queda sola.
pub fn para_una_pregunta(q: &str) -> Vec<&'static Accion> {
    ACCIONES_DOCUMENTOS
        .iter()
        .filter(|a| a.herramienta == BUSCAR_EN && a.claves.iter().any(|c| c.contains(' ') && q.contains(c)))
        .collect()
}

#[cfg(test)]
mod pruebas {
    use serde_json::{Map, Value, json};

    use super::*;
    use crate::acciones::{herramienta, por_nombre, validar};

    fn args(s: &str) -> Map<String, Value> {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn salen_de_la_misma_tabla_y_no_chocan_con_las_de_la_app() {
        let mut nombres: Vec<&str> = crate::acciones::todas().map(|a| a.herramienta).collect();
        let n = nombres.len();
        nombres.sort();
        nombres.dedup();
        assert_eq!(nombres.len(), n, "un nombre repetido");
        for a in ACCIONES_DOCUMENTOS {
            assert!(por_nombre(a.herramienta).is_some());
            assert!(es_de_documentos(a.herramienta));
            let h = herramienta(a);
            assert_eq!(h.parametros["type"], "object", "{}", a.herramienta);
        }
        assert!(!es_de_documentos("abrir_fichero"));
        // Estan en el catalogo que se le da a la nube.
        let cat: Vec<String> = crate::herramientas::catalogo().into_iter().map(|h| h.nombre).collect();
        for a in ACCIONES_DOCUMENTOS {
            assert!(cat.contains(&a.herramienta.to_string()), "{}", a.herramienta);
        }
    }

    #[test]
    fn la_validacion_es_la_comun() {
        let t = validar(por_nombre(TABLA).unwrap(), &args(r#"{"destino":"Excel","nombre":"Costos"}"#)).unwrap();
        assert_eq!(t.pedido["destino"], "excel");
        let t = validar(por_nombre(TABLA).unwrap(), &Map::new()).unwrap();
        assert_eq!(t.pedido["destino"], "tabla");
        let a = validar(
            por_nombre(ANOTAR).unwrap(),
            &args(r#"{"tipo":"resaltar","texto":"JR. ANTISUYO","zona":[[0.1,0.2],[0.3,0.4]]}"#),
        )
        .unwrap();
        assert_eq!(a.pedido["zona"], json!([0.1, 0.2, 0.3, 0.4]));
        // Una zona escrita como texto tambien vale.
        let a = validar(por_nombre(ANOTAR).unwrap(), &args(r#"{"tipo":"encerrar","zona":"[0.1, 0.2, 0.3, 0.4]"}"#)).unwrap();
        assert_eq!(a.pedido["zona"], json!([0.1, 0.2, 0.3, 0.4]));
        let b = validar(por_nombre(BUSCAR_EN).unwrap(), &args(r#"{"texto":"JR. ANTISUYO"}"#)).unwrap();
        assert_eq!(b.pedido["abrir"], true);
        assert!(b.confirmar.is_none());
        // Casos negativos: lo que falta o no se entiende no pasa.
        assert!(validar(por_nombre(ANOTAR).unwrap(), &args(r#"{"tipo":"pintar"}"#)).is_err());
        assert!(validar(por_nombre(ANOTAR).unwrap(), &args(r#"{"tipo":"nota","zona":["a"]}"#)).is_err());
        assert!(validar(por_nombre(BUSCAR_EN).unwrap(), &Map::new()).is_err());
        assert!(validar(por_nombre(EXPORTAR).unwrap(), &args(r#"{"formato":"docx"}"#)).is_err());
        assert!(validar(por_nombre(ABRIR).unwrap(), &args(r#"{"descripcion":"  "}"#)).is_err());
    }

    #[test]
    fn una_pregunta_de_donde_dice_trae_la_busqueda() {
        let q = crate::acciones::normalizar("¿Dónde dice JR. ANTISUYO en el plano?");
        let n: Vec<&str> = para_una_pregunta(&q).iter().map(|a| a.herramienta).collect();
        assert_eq!(n, vec![BUSCAR_EN]);
        assert!(para_una_pregunta("como hago una captura").is_empty());
    }
}
