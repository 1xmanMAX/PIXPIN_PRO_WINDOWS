//! **Toda la app como herramientas del asistente**, sacadas de UNA tabla.
//!
//! Pedido del usuario (13-oct-2026): «que sepa manejar todas las funciones de
//! la app rapidamente». PixPin ya sabe hacer casi todo por encargo de otro
//! programa: el protocolo de pedidos (`docs/protocolo-pedidos.md`,
//! `apps/pixpin/src/pedidos.rs`) que usa el plugin de Flow Launcher. El
//! asistente es un programa mas: cada herramienta de aqui produce el JSON de
//! un pedido, y la app lo atiende por el mismo camino, con su sello y sus
//! avisos, identico a si lo hubiera pedido el usuario.
//!
//! Regla de la casa: un componente con variables, no una funcion a mano por
//! herramienta. Cada fila de [`ACCIONES`] dice su nombre para el modelo, la
//! `accion` del pedido, lo fijo del pedido, sus parametros (tipo, si es
//! obligatorio, a que campo del pedido va), si hay que preguntar antes y la
//! frase corta de «hecho» ([`Accion::hecho`], la burbuja del camino rapido).
//! [`herramienta`] genera el JSON Schema y [`validar`] el pedido, para todas.
//!
//! **Lo que borra o saca algo del PC pide permiso** ([`Accion::confirmar`]):
//! la llamada validada lleva la pregunta y el ejecutor de la UI la hace
//! («¿Sí?») antes de mandar nada.
//!
//! **Las tareas por su nombre** (14-oct): el modelo no conoce los ids de las
//! listas ni el numero de cada tarea, asi que esas filas piden «la lista
//! Compras», «la tarea comprar pan», «mañana a las 10» o «el chat Obra». Esos
//! parametros llevan un destino que empieza por `?` ([`POR_RESOLVER`]): no van
//! al pedido sino a [`AccionApp::por_resolver`], y `tareas::resolver` los
//! cambia por los `codigo`, `indice` y `proyecto` de verdad leyendo las
//! listas del usuario. Lo que no casa (una tarea que no esta, una fecha que no
//! existe, una lista de otro chat) vuelve al modelo como error con el motivo.
//!
//! Lo que no esta: `iconos` (interno del plugin).

use serde_json::{Map, Value, json};

use crate::herramientas::{ErrorHerramienta, Herramienta};

/// Para elegir las pocas herramientas que se ensenan en cada turno.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Categoria {
    Abrir,
    Notas,
    Tareas,
    Capturas,
    Audio,
    Lecciones,
    Movil,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Texto,
    /// Una ruta de fichero (texto, pero se le quitan las comillas).
    Ruta,
    /// Lista de rutas.
    Rutas,
    /// Lista de textos (varias tareas de una vez).
    Textos,
    Entero,
    SiNo,
    Opciones(&'static [&'static str]),
    /// Lista de numeros (una zona, unos puntos).
    Numeros,
}

#[derive(Debug, Clone, Copy)]
pub struct Param {
    pub nombre: &'static str,
    /// Donde va en el pedido, con puntos (`que.ruta`). Vacio: su nombre.
    /// Empezando por [`POR_RESOLVER`]: no va al pedido (lo resuelve
    /// `tareas::resolver`).
    pub destino: &'static str,
    pub tipo: Tipo,
    pub obligatorio: bool,
    pub descripcion: &'static str,
    /// Lo que vale si el modelo no lo pone (JSON).
    pub defecto: Option<&'static str>,
    /// Cuantas letras como mucho (texto o cada elemento de una lista de
    /// textos). `None`: [`LARGO_TEXTO`]. Va al esquema como `maxLength`, y
    /// con la gramatica de llama.cpp el modelo no puede pasarse.
    pub largo: Option<usize>,
}

/// El largo de un texto de la tabla si la fila no dice otro: de sobra para
/// una tarea, un titulo o una ruta.
pub const LARGO_TEXTO: usize = 2000;
/// Y el de los largos por naturaleza: una nota, un mensaje, los pasos de una
/// automatizacion.
pub const LARGO_LARGO: usize = 8000;

/// Un parametro con su propio largo maximo.
pub const fn l(largo: usize, mut p: Param) -> Param {
    p.largo = Some(largo);
    p
}

/// La marca de los parametros que no van tal cual al pedido.
pub const POR_RESOLVER: &str = "?";

/// La `accion` de lo que no es un pedido a la app sino una consulta que se
/// contesta aqui mismo (leer las tareas).
pub const CONSULTA: &str = "(consulta)";

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

const fn a(destino: &'static str, mut p: Param) -> Param {
    p.destino = destino;
    p
}

const fn d(defecto: &'static str, mut p: Param) -> Param {
    p.defecto = Some(defecto);
    p
}

/// Un parametro que resuelve `tareas::resolver` (no va al pedido).
const fn r(mut p: Param) -> Param {
    p.destino = POR_RESOLVER;
    p
}

/// Una accion de la app.
#[derive(Debug, Clone, Copy)]
pub struct Accion {
    /// Como la llama el modelo.
    pub herramienta: &'static str,
    /// La `accion` del pedido ([`CONSULTA`]: no es un pedido).
    pub accion: &'static str,
    pub descripcion: &'static str,
    pub categoria: Categoria,
    /// Raices de palabras (sin tildes, minusculas) que la delatan en el
    /// pedido del usuario.
    pub claves: &'static [&'static str],
    /// Lo fijo del pedido, en JSON (`{"que":{"tipo":"fichero"}}`).
    pub fijos: &'static str,
    pub params: &'static [Param],
    /// Si hay que preguntar antes: la pregunta, con `{campo}` sustituido.
    pub confirmar: Option<&'static str>,
    /// Lo que se dice al acabar (la burbuja del camino rapido y el resultado
    /// que ve el modelo), con `{campo}`; un trozo entre `[ ]` se quita si le
    /// falta algun campo.
    pub hecho: &'static str,
}

const RUTA: &str = "ruta absoluta del fichero en este PC";
const LISTA: &str = "nombre de la lista; omitir si no la dice";
const TAREA: &str = "texto de la tarea (o un trozo)";
const FECHA: &str = "fecha tal como la dijo («mañana a las 10», «el viernes»)";
const CHAT: &str = "chat de la lista, solo si lo dice";

/// **La tabla.** Una fila por herramienta.
pub const ACCIONES: &[Accion] = &[
    Accion {
        herramienta: "abrir_chat",
        accion: "ventana_principal",
        descripcion: "Abre (o trae delante) la ventana del chat de proyectos.",
        categoria: Categoria::Abrir,
        claves: &["chat", "proyecto", "ventana principal", "conversacion"],
        fijos: "{}",
        params: &[],
        confirmar: None,
        hecho: "Abriendo el chat.",
    },
    Accion {
        herramienta: "abrir_fichero",
        accion: "abrir",
        descripcion: "Abre un fichero en PixPin (PDF, Word, Excel, imagen, plano...), como «Abrir con PixPin».",
        categoria: Categoria::Abrir,
        claves: &["abre", "abrir", "archivo", "fichero", "documento", "pdf", "docx", "xlsx", "dwg", ":\\"],
        fijos: r#"{"que":{"tipo":"fichero"}}"#,
        params: &[a("que.ruta", p("ruta", Tipo::Ruta, true, RUTA))],
        confirmar: None,
        hecho: "Abriendo {ruta}.",
    },
    Accion {
        herramienta: "abrir_mensaje",
        accion: "abrir",
        descripcion: "Abre el chat en un mensaje concreto, resaltado.",
        categoria: Categoria::Abrir,
        claves: &["mensaje"],
        fijos: r#"{"que":{"tipo":"mensaje"}}"#,
        params: &[a("que.codigo", p("codigo", Tipo::Texto, true, "id del mensaje"))],
        confirmar: None,
        hecho: "Abriendo el mensaje.",
    },
    Accion {
        herramienta: "abrir_nota",
        accion: "abrir",
        descripcion: "Abre una nota existente en el editor de notas.",
        categoria: Categoria::Notas,
        claves: &["abre la nota", "editar nota", "abrir nota"],
        fijos: r#"{"que":{"tipo":"nota"}}"#,
        params: &[a("que.codigo", p("codigo", Tipo::Texto, true, "id del mensaje de la nota"))],
        confirmar: None,
        hecho: "Abriendo la nota.",
    },
    Accion {
        herramienta: "abrir_hoja",
        accion: "abrir",
        descripcion: "Abre un lienzo u hoja de un proyecto por su referencia.",
        categoria: Categoria::Abrir,
        claves: &["hoja", "abre el lienzo", "abrir lienzo"],
        fijos: r#"{"que":{"tipo":"hoja"}}"#,
        params: &[a("que.referencia", p("referencia", Tipo::Texto, true, "referencia de la hoja o lienzo"))],
        confirmar: None,
        hecho: "Abriendo «{referencia}».",
    },
    Accion {
        herramienta: "escribir_en_chat",
        accion: "chat",
        descripcion: "Deja un mensaje de texto en el chat de «Mensajes guardados».",
        categoria: Categoria::Notas,
        claves: &["guarda en el chat", "escribe en el chat", "manda al chat", "mensaje al chat", "apunta en el chat"],
        fijos: "{}",
        params: &[l(LARGO_LARGO, p("texto", Tipo::Texto, true, "el mensaje"))],
        confirmar: None,
        hecho: "Escrito en «Mensajes guardados».",
    },
    Accion {
        herramienta: "nota_nueva",
        accion: "nota_nueva",
        descripcion: "Crea una nota nueva (Markdown) y la abre en el editor.",
        categoria: Categoria::Notas,
        claves: &["nota", "apunte", "markdown", "escribe una nota"],
        fijos: "{}",
        params: &[l(LARGO_LARGO, p("texto", Tipo::Texto, false, "el contenido de la nota"))],
        confirmar: None,
        hecho: "Nota nueva creada[: «{texto}»].",
    },
    Accion {
        herramienta: "lienzo_nuevo",
        accion: "lienzo_nuevo",
        descripcion: "Crea un lienzo nuevo, vacio, y lo abre.",
        categoria: Categoria::Notas,
        claves: &["lienzo nuevo", "nuevo lienzo", "crea un lienzo", "pizarra nueva", "lienzo llamado"],
        fijos: "{}",
        params: &[p("nombre", Tipo::Texto, false, "nombre del lienzo")],
        confirmar: None,
        hecho: "Lienzo nuevo creado[: «{nombre}»].",
    },
    Accion {
        herramienta: "grabar_voz",
        accion: "grabar",
        descripcion: "Saca el microfono flotante y graba una nota de voz con ese nombre.",
        categoria: Categoria::Audio,
        claves: &["graba", "grabar", "nota de voz", "audio", "microfono"],
        fijos: "{}",
        params: &[p("nombre", Tipo::Texto, true, "nombre de la grabacion")],
        confirmar: None,
        hecho: "Grabando «{nombre}»…",
    },
    Accion {
        herramienta: "reproducir_audio",
        accion: "reproducir",
        descripcion: "Hace sonar un audio (m4a, mp3, wav) en el reproductor flotante.",
        categoria: Categoria::Audio,
        claves: &["reproduc", "escucha", "suena", "pon el audio", "mp3", "m4a", "wav"],
        fijos: "{}",
        params: &[p("ruta", Tipo::Ruta, true, RUTA), p("titulo", Tipo::Texto, false, "lo que se lee en la barra")],
        confirmar: None,
        hecho: "Reproduciendo[ «{titulo}»].",
    },
    // --- Tareas (14-oct: por su nombre, con fecha, de varias en varias) ---
    Accion {
        herramienta: "lista_nueva",
        accion: "lista_nueva",
        descripcion: "Crea una lista de tareas nueva con ese titulo (en «Mensajes guardados» si no se dice el chat).",
        categoria: Categoria::Tareas,
        claves: &["lista", "checklist"],
        fijos: "{}",
        params: &[p("titulo", Tipo::Texto, true, "titulo de la lista"), r(p("chat", Tipo::Texto, false, CHAT))],
        confirmar: None,
        hecho: "Lista «{titulo}» creada[ en «{chat}»].",
    },
    Accion {
        herramienta: "anadir_tarea",
        accion: "anadir_tarea",
        descripcion: "Apunta UNA tarea pendiente, con su fecha si la dice (va al Inbox si no se dice la lista).",
        categoria: Categoria::Tareas,
        claves: &["tarea", "pendiente", "apunta", "recuerdame", "por hacer", "inbox", "to do", "todo", "anade", "agrega"],
        fijos: "{}",
        params: &[
            p("texto", Tipo::Texto, true, "lo que hay que hacer, sin la fecha"),
            r(p("lista", Tipo::Texto, false, LISTA)),
            r(p("fecha", Tipo::Texto, false, FECHA)),
            r(p("chat", Tipo::Texto, false, CHAT)),
        ],
        confirmar: None,
        hecho: "Apuntada «{texto}»[ en «{lista}»][ para {fecha}].",
    },
    Accion {
        herramienta: "anadir_tareas",
        accion: "anadir_tarea",
        descripcion: "Apunta VARIAS tareas (de un acta, foto o documento) en una lista existente o nueva.",
        categoria: Categoria::Tareas,
        claves: &["tareas de", "saca las tareas", "acta", "varias tareas", "estas tareas", "pendientes de"],
        fijos: "{}",
        params: &[
            r(p("tareas", Tipo::Textos, true, "las tareas, una por elemento, cortas y sin numerar")),
            r(p("lista", Tipo::Texto, false, LISTA)),
            r(p("lista_nueva", Tipo::Texto, false, "titulo de una lista NUEVA, si la pide")),
            r(p("chat", Tipo::Texto, false, CHAT)),
        ],
        confirmar: None,
        hecho: "Apuntadas {n} tareas en «{lista}».",
    },
    Accion {
        herramienta: "marcar_tarea",
        accion: "marcar_tarea",
        descripcion: "Marca una tarea como hecha (o la vuelve a dejar pendiente).",
        categoria: Categoria::Tareas,
        claves: &["marca", "hecha", "completa", "termin", "tacha", "desmarca", "acabe", "hice"],
        fijos: "{}",
        params: &[
            r(p("tarea", Tipo::Texto, true, TAREA)),
            r(p("lista", Tipo::Texto, false, LISTA)),
            d("true", p("hecha", Tipo::SiNo, false, "true hecha, false pendiente otra vez")),
        ],
        confirmar: None,
        hecho: "«{tarea}» marcada como {estado}.",
    },
    Accion {
        herramienta: "mover_tarea",
        accion: "mover_tarea",
        descripcion: "Pasa una tarea a otra lista.",
        categoria: Categoria::Tareas,
        claves: &["mueve", "mover", "pasa la tarea", "llevate"],
        fijos: "{}",
        params: &[
            r(p("tarea", Tipo::Texto, true, TAREA)),
            r(p("lista", Tipo::Texto, false, "lista donde esta ahora; omitir si no la dice")),
            r(p("a_lista", Tipo::Texto, true, "nombre de la lista de destino")),
        ],
        confirmar: None,
        hecho: "«{tarea}» movida a «{a_lista}».",
    },
    Accion {
        herramienta: "editar_tarea",
        accion: "corregir_tarea",
        descripcion: "Cambia el texto de una tarea o le pone otra fecha.",
        categoria: Categoria::Tareas,
        claves: &["cambia la tarea", "edita", "renombra", "corrige", "cambiale", "pon la tarea", "posterga", "aplaza"],
        fijos: "{}",
        params: &[
            r(p("tarea", Tipo::Texto, true, TAREA)),
            r(p("lista", Tipo::Texto, false, LISTA)),
            p("texto", Tipo::Texto, false, "el texto nuevo; omitir si solo cambia la fecha"),
            r(p("fecha", Tipo::Texto, false, FECHA)),
        ],
        confirmar: None,
        hecho: "Tarea cambiada: «{texto}»[ para {fecha}].",
    },
    Accion {
        herramienta: "quitar_tarea",
        accion: "quitar_tarea",
        descripcion: "Quita (borra) una tarea de su lista.",
        categoria: Categoria::Tareas,
        claves: &["quita", "elimina la tarea", "borra la tarea"],
        fijos: "{}",
        params: &[r(p("tarea", Tipo::Texto, true, TAREA)), r(p("lista", Tipo::Texto, false, LISTA))],
        confirmar: Some("¿Quito la tarea «{tarea}» de «{lista}»?"),
        hecho: "Quitada «{tarea}» de «{lista}».",
    },
    Accion {
        herramienta: "borrar_lista",
        accion: "borrar_lista",
        descripcion: "Borra una lista de tareas entera.",
        categoria: Categoria::Tareas,
        claves: &["borra", "elimina", "borrar lista"],
        fijos: "{}",
        params: &[r(p("lista", Tipo::Texto, true, "nombre de la lista que hay que borrar")), r(p("chat", Tipo::Texto, false, CHAT))],
        confirmar: Some("¿Borro la lista «{lista}» entera, con todas sus tareas? No se puede deshacer."),
        hecho: "Lista «{lista}» borrada.",
    },
    Accion {
        herramienta: "consultar_tareas",
        accion: CONSULTA,
        descripcion: "Lee las tareas: pendientes, hechas o vencidas, por periodo, lista o chat.",
        categoria: Categoria::Tareas,
        claves: &["que tengo", "pendientes", "vencid", "venc", "atrasad", "mis tareas", "que tareas", "cuantas tareas", "que me falta"],
        fijos: "{}",
        params: &[
            r(d(r#""pendientes""#, p("estado", Tipo::Opciones(&["pendientes", "hechas", "vencidas", "todas"]), false, "que tareas"))),
            r(d(r#""todo""#, p("periodo", Tipo::Opciones(&["hoy", "semana", "mes", "todo"]), false, "por su fecha: hoy, esta semana, este mes o todas"))),
            r(p("lista", Tipo::Texto, false, LISTA)),
            r(p("chat", Tipo::Texto, false, CHAT)),
            r(p("buscar", Tipo::Texto, false, "palabras que deben tener las tareas")),
        ],
        confirmar: None,
        hecho: "",
    },
    Accion {
        herramienta: "abrir_ventana",
        accion: "ventana",
        descripcion: "Abre una ventana de PixPin: tareas, galeria (de capturas), lecciones o sincronizar.",
        categoria: Categoria::Abrir,
        claves: &["ventana", "galeria", "tareas", "lecciones", "sincronizar", "abre", "ensena", "muestra"],
        fijos: "{}",
        params: &[p(
            "cual",
            Tipo::Opciones(&["tareas", "galeria", "lecciones", "sincronizar"]),
            true,
            "que ventana",
        )],
        confirmar: None,
        hecho: "Abriendo la ventana de {cual}.",
    },
    Accion {
        herramienta: "capturar_pantalla",
        accion: "capturar",
        descripcion: "Empieza una captura de una zona de la pantalla (el usuario elige la zona).",
        categoria: Categoria::Capturas,
        claves: &["captura", "capturar", "pantallazo", "screenshot", "recorta la pantalla"],
        fijos: r#"{"modo":"zona"}"#,
        params: &[],
        confirmar: None,
        hecho: "Elige la zona que quieres capturar.",
    },
    Accion {
        herramienta: "pinear_ultima_captura",
        accion: "pinear_ultima",
        descripcion: "Saca a la pantalla como pin la captura mas reciente.",
        categoria: Categoria::Capturas,
        claves: &["pin", "pinea", "ultima captura", "fija"],
        fijos: "{}",
        params: &[],
        confirmar: None,
        hecho: "La última captura, como pin en la pantalla.",
    },
    Accion {
        herramienta: "pinear_fichero",
        accion: "pinear",
        descripcion: "Saca un fichero (imagen, PDF...) a la pantalla como pin.",
        categoria: Categoria::Capturas,
        claves: &["pin", "pinea", "fija en pantalla", "saca a la pantalla"],
        fijos: "{}",
        params: &[p("ruta", Tipo::Ruta, true, RUTA)],
        confirmar: None,
        hecho: "{ruta}, como pin en la pantalla.",
    },
    Accion {
        herramienta: "conservar_captura",
        accion: "conservar_captura",
        descripcion: "Conserva una captura (no caduca y va a «Mensajes guardados»).",
        categoria: Categoria::Capturas,
        claves: &["conserva", "no borres", "guarda la captura", "caduc"],
        fijos: "{}",
        params: &[p("ruta", Tipo::Ruta, true, "ruta de la captura")],
        confirmar: None,
        hecho: "Captura conservada.",
    },
    Accion {
        herramienta: "borrar_captura",
        accion: "borrar_captura",
        descripcion: "Manda una captura a la papelera de PixPin.",
        categoria: Categoria::Capturas,
        claves: &["borra la captura", "elimina la captura", "papelera"],
        fijos: "{}",
        params: &[p("ruta", Tipo::Ruta, true, "ruta de la captura")],
        confirmar: Some("¿Mando a la papelera la captura {ruta}?"),
        hecho: "Captura enviada a la papelera.",
    },
    Accion {
        herramienta: "copiar_imagen",
        accion: "copiar_imagen",
        descripcion: "Pone una imagen en el portapapeles, como imagen.",
        categoria: Categoria::Capturas,
        claves: &["copia la imagen", "portapapeles", "copiar imagen"],
        fijos: "{}",
        params: &[p("ruta", Tipo::Ruta, true, RUTA)],
        confirmar: None,
        hecho: "Imagen copiada al portapapeles.",
    },
    Accion {
        herramienta: "caja_para_soltar",
        accion: "soltar",
        descripcion: "Saca una caja flotante a la que soltar ficheros para meterlos en el chat.",
        categoria: Categoria::Abrir,
        claves: &["soltar", "arrastrar", "caja flotante", "suelta"],
        fijos: "{}",
        params: &[],
        confirmar: None,
        hecho: "Suelta los ficheros en la caja.",
    },
    Accion {
        herramienta: "leccion_nueva",
        accion: "leccion_nueva",
        descripcion: "Abre la ficha de una leccion aprendida nueva (paso que..., porque..., la proxima vez...).",
        categoria: Categoria::Lecciones,
        claves: &["leccion nueva", "nueva leccion", "aprendi", "aprendida"],
        fijos: "{}",
        params: &[l(LARGO_LARGO, p("texto", Tipo::Texto, false, "lo que paso y lo aprendido"))],
        confirmar: None,
        hecho: "Ficha de lección nueva abierta.",
    },
    Accion {
        herramienta: "buscar_lecciones",
        accion: "lecciones",
        descripcion: "Abre la lista de lecciones aprendidas, buscando lo que se diga.",
        categoria: Categoria::Lecciones,
        claves: &["lecciones", "leccion", "repasar", "repaso"],
        fijos: "{}",
        params: &[p("consulta", Tipo::Texto, false, "lo que se busca")],
        confirmar: None,
        hecho: "Abriendo las lecciones[ de «{consulta}»].",
    },
    Accion {
        herramienta: "sincronizar_movil",
        accion: "sincronizar",
        descripcion: "Sincroniza ahora con el movil (o con todos los aparatos del grupo).",
        categoria: Categoria::Movil,
        claves: &["sincroniz", "sync", "movil", "celular", "telefono", "android"],
        fijos: "{}",
        params: &[d(r#""todos""#, p("aparato", Tipo::Texto, false, "id del aparato; omitir para todos"))],
        confirmar: Some("¿Sincronizo ahora con el movil?"),
        hecho: "Sincronizando con el móvil.",
    },
    Accion {
        herramienta: "enviar_al_movil",
        accion: "enviar_al_movil",
        descripcion: "Manda imagenes al lienzo abierto en el movil.",
        categoria: Categoria::Movil,
        claves: &["envia", "manda", "pasa", "movil", "celular", "telefono"],
        fijos: "{}",
        params: &[
            p("imagenes", Tipo::Rutas, true, "rutas absolutas de las imagenes"),
            d(r#""todos""#, p("aparato", Tipo::Texto, false, "id del aparato; omitir para todos")),
        ],
        confirmar: Some("¿Mando {imagenes} al movil?"),
        hecho: "Enviado al móvil.",
    },
    // 13-oct: las automatizaciones («cada vez que capture…», «cada dia a
    // las 8…»), `apps/pixpin/src/automatizaciones.rs`. Se crean preguntando.
    Accion {
        herramienta: "crear_automatizacion",
        accion: "automatizacion",
        // Corta a proposito: con el catalogo entero (la nube) cada letra
        // cuenta en el contexto (`agente::tope_de_tokens`).
        descripcion: "Crea una automatizacion (se dispara sola y hace pasos).",
        categoria: Categoria::Tareas,
        claves: &[
            "cada vez",
            "cada dia",
            "todos los dias",
            "cada manana",
            "cada semana",
            "siempre que",
            "automatiz",
            "al capturar",
            "cuando capture",
            "cuando sincronice",
        ],
        fijos: r#"{"operacion":"crear"}"#,
        params: &[
            a("receta.nombre", p("nombre", Tipo::Texto, true, "nombre corto")),
            a(
                "receta.disparador.tipo",
                p(
                    "disparador",
                    Tipo::Opciones(&["al_capturar", "a_la_hora", "al_sincronizar", "a_mano"]),
                    true,
                    "cuando",
                ),
            ),
            a("receta.disparador.hora", p("hora", Tipo::Texto, false, "HH:MM")),
            a("receta.pasos", l(LARGO_LARGO, p("pasos", Tipo::Texto, true, "lista JSON: texto para el asistente u {herramienta,args,proyecto}; {captura} {resultado}"))),
        ],
        confirmar: Some("¿Creo la automatización «{nombre}» ({disparador}) con estos pasos: {pasos}?"),
        hecho: "Automatización «{nombre}» creada.",
    },
];

/// **Todas las filas**: las de esta tabla y las de los documentos
/// ([`crate::acciones_documentos`], que hace el chat con sus adjuntos).
pub fn todas() -> impl Iterator<Item = &'static Accion> {
    ACCIONES.iter().chain(crate::acciones_documentos::ACCIONES_DOCUMENTOS.iter())
}

/// Busca una fila por el nombre de su herramienta.
pub fn por_nombre(nombre: &str) -> Option<&'static Accion> {
    todas().find(|a| a.herramienta == nombre)
}

/// **La herramienta que ve el modelo**, generada de la fila.
pub fn herramienta(a: &Accion) -> Herramienta {
    let mut propiedades = Map::new();
    let mut obligatorios = Vec::new();
    for p in a.params {
        let mut esquema = match p.tipo {
            Tipo::Texto | Tipo::Ruta => json!({"type": "string", "maxLength": p.largo.unwrap_or(LARGO_TEXTO)}),
            Tipo::Rutas | Tipo::Textos => {
                json!({"type": "array", "items": {"type": "string", "maxLength": p.largo.unwrap_or(LARGO_TEXTO)}})
            }
            Tipo::Entero => json!({"type": "integer"}),
            Tipo::SiNo => json!({"type": "boolean"}),
            Tipo::Opciones(o) => json!({"type": "string", "enum": o}),
            Tipo::Numeros => json!({"type": "array", "items": {"type": "number"}}),
        };
        esquema["description"] = json!(p.descripcion);
        propiedades.insert(p.nombre.into(), esquema);
        if p.obligatorio {
            obligatorios.push(p.nombre);
        }
    }
    let mut parametros = json!({"type": "object", "properties": propiedades});
    if !obligatorios.is_empty() {
        parametros["required"] = json!(obligatorios);
    }
    Herramienta {
        nombre: a.herramienta.into(),
        descripcion: a.descripcion.into(),
        parametros,
    }
}

/// Todas las de la app.
pub fn herramientas() -> Vec<Herramienta> {
    todas().map(herramienta).collect()
}

/// Una accion de la app ya validada: el pedido listo para
/// `pedidos::atender` y, si hay que preguntar, la pregunta.
#[derive(Debug, Clone, PartialEq)]
pub struct AccionApp {
    pub herramienta: &'static str,
    /// `{"pixpin":1,"accion":...}` completo (sin lo que falta resolver).
    pub pedido: Value,
    pub confirmar: Option<String>,
    /// Lo que pide nombres de listas, tareas, chats o fechas: lo cambia por
    /// ids y numeros `tareas::resolver`. Vacio en las demas.
    pub por_resolver: Map<String, Value>,
}

impl AccionApp {
    /// La fila de la tabla de la que salio.
    pub fn fila(&self) -> &'static Accion {
        por_nombre(self.herramienta).expect("salio de la tabla")
    }

    /// Si es una consulta (se contesta aqui, no se manda nada).
    pub fn es_consulta(&self) -> bool {
        self.fila().accion == CONSULTA
    }

    /// Todos los valores tal cual (los del pedido y los por resolver), por
    /// nombre de parametro: para las frases de [`Accion::hecho`].
    pub fn valores(&self) -> Vec<(String, String)> {
        let mut v = Vec::new();
        for p in self.fila().params {
            let x = if p.destino.starts_with(POR_RESOLVER) {
                self.por_resolver.get(p.nombre).cloned()
            } else {
                let mut actual = &self.pedido;
                for t in (if p.destino.is_empty() { p.nombre } else { p.destino }).split('.') {
                    actual = &actual[t];
                }
                (!actual.is_null()).then(|| actual.clone())
            };
            if let Some(x) = x {
                v.push((p.nombre.to_string(), leible(&x)));
            }
        }
        v
    }
}

/// Un valor JSON como se lee en una frase.
pub fn leible(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(l) => l.iter().map(leible).collect::<Vec<_>>().join(", "),
        otro => otro.to_string(),
    }
}

/// **Rellena una plantilla** de [`Accion::hecho`] o [`Accion::confirmar`]:
/// `{campo}` por su valor y los trozos `[...]` a los que les falte un campo,
/// fuera.
pub fn rellenar(plantilla: &str, valores: &[(String, String)]) -> String {
    fn sustituir(s: &str, valores: &[(String, String)]) -> Option<String> {
        let mut out = String::new();
        let mut resto = s;
        while let Some(i) = resto.find('{') {
            out.push_str(&resto[..i]);
            let j = resto[i..].find('}')? + i;
            let campo = &resto[i + 1..j];
            let v = valores.iter().find(|(k, v)| k == campo && !v.trim().is_empty())?;
            out.push_str(&v.1);
            resto = &resto[j + 1..];
        }
        out.push_str(resto);
        Some(out)
    }
    let mut out = String::new();
    let mut resto = plantilla;
    while let Some(i) = resto.find('[') {
        let Some(j) = resto[i..].find(']').map(|j| j + i) else { break };
        out.push_str(&resto[..i]);
        if let Some(s) = sustituir(&resto[i + 1..j], valores) {
            out.push_str(&s);
        }
        resto = &resto[j + 1..];
    }
    out.push_str(resto);
    // Lo que no estaba entre corchetes y falta se queda sin decir.
    sustituir(&out, valores).unwrap_or_else(|| {
        let mut s = out.clone();
        while let (Some(i), Some(j)) = (s.find('{'), s.find('}')) {
            if j < i {
                break;
            }
            s.replace_range(i..=j, "");
        }
        s.replace("«»", "").split_whitespace().collect::<Vec<_>>().join(" ")
    })
}

fn mal(m: impl Into<String>) -> ErrorHerramienta {
    ErrorHerramienta::Parametro(m.into())
}

/// Minusculas y sin tildes (para las opciones y para elegir herramientas).
pub fn normalizar(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' => 'a',
            'é' | 'è' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            'ñ' => 'n',
            otro => otro,
        })
        .collect()
}

fn valor(p: &Param, v: &Value) -> Result<Value, ErrorHerramienta> {
    let campo = p.nombre;
    Ok(match p.tipo {
        Tipo::Texto => match v {
            Value::String(s) if !s.trim().is_empty() => json!(s.trim().chars().take(p.largo.unwrap_or(LARGO_TEXTO)).collect::<String>()),
            Value::Number(n) => json!(n.to_string()),
            _ => return Err(mal(format!("«{campo}» tiene que ser un texto"))),
        },
        Tipo::Ruta => match v {
            Value::String(s) => {
                let r = s.trim().trim_matches(|c| c == '"' || c == '\'' || c == '«' || c == '»');
                if r.is_empty() {
                    return Err(mal(format!("«{campo}» esta vacio")));
                }
                json!(r)
            }
            _ => return Err(mal(format!("«{campo}» tiene que ser una ruta"))),
        },
        Tipo::Rutas | Tipo::Textos => match v {
            Value::String(s) if !s.trim().is_empty() => json!([s.trim()]),
            Value::Array(l) if !l.is_empty() => {
                let todos: Option<Vec<&str>> = l.iter().map(|r| r.as_str().map(str::trim)).collect();
                let mut todos = todos.ok_or_else(|| mal(format!("«{campo}» tiene que ser una lista de textos")))?;
                todos.retain(|t| !t.is_empty());
                if todos.is_empty() {
                    return Err(mal(format!("«{campo}» esta vacio")));
                }
                json!(todos)
            }
            _ => return Err(mal(format!("«{campo}» tiene que ser una lista de textos"))),
        },
        Tipo::Entero => {
            let n = match v {
                Value::Number(n) => n.as_f64(),
                Value::String(s) => s.trim().parse().ok(),
                _ => None,
            };
            match n {
                Some(n) if n >= 0.0 && n.fract() == 0.0 && n < 100_000.0 => json!(n as u64),
                _ => return Err(mal(format!("«{campo}» tiene que ser un numero entero desde 0"))),
            }
        }
        Tipo::SiNo => match v {
            Value::Bool(b) => json!(b),
            Value::String(s) => match normalizar(s.trim()).as_str() {
                "true" | "si" | "yes" | "hecha" => json!(true),
                "false" | "no" | "pendiente" => json!(false),
                _ => return Err(mal(format!("«{campo}» tiene que ser true o false"))),
            },
            _ => return Err(mal(format!("«{campo}» tiene que ser true o false"))),
        },
        Tipo::Numeros => {
            // Plana o de pares, y tambien «[0.1, 0.2]» escrito como texto.
            let lista = match v {
                Value::String(s) => serde_json::from_str::<Value>(s.trim()).unwrap_or(Value::Null),
                otro => otro.clone(),
            };
            let mut n: Vec<f64> = Vec::new();
            let bien = match &lista {
                Value::Array(l) if !l.is_empty() => l.iter().all(|e| match e {
                    Value::Array(par) => par.iter().all(|x| x.as_f64().filter(|f| f.is_finite()).map(|f| n.push(f)).is_some()),
                    x => x.as_f64().filter(|f| f.is_finite()).map(|f| n.push(f)).is_some(),
                }),
                _ => false,
            };
            if !bien || n.len() > 400 {
                return Err(mal(format!("«{campo}» tiene que ser una lista de numeros")));
            }
            json!(n)
        }
        Tipo::Opciones(o) => {
            let s = v.as_str().map(|s| normalizar(s.trim())).unwrap_or_default();
            match o.iter().find(|x| **x == s || (s.len() >= 4 && x.starts_with(&s[..4]))) {
                Some(x) => json!(x),
                None => return Err(mal(format!("«{campo}» tiene que ser una de: {}", o.join(", ")))),
            }
        }
    })
}

/// Mete `v` en `pedido` siguiendo `que.ruta`.
fn poner(pedido: &mut Value, destino: &str, v: Value) {
    let mut actual = pedido;
    let trozos: Vec<&str> = destino.split('.').collect();
    for (i, t) in trozos.iter().enumerate() {
        if i + 1 == trozos.len() {
            actual[*t] = v;
            return;
        }
        if !actual[*t].is_object() {
            actual[*t] = json!({});
        }
        actual = &mut actual[*t];
    }
}

/// **Valida** los argumentos del modelo contra la fila y arma el pedido.
pub fn validar(a: &'static Accion, args: &Map<String, Value>) -> Result<AccionApp, ErrorHerramienta> {
    let mut pedido: Value = serde_json::from_str(a.fijos).unwrap_or_else(|_| json!({}));
    pedido["pixpin"] = json!(1);
    pedido["accion"] = json!(a.accion);
    let mut por_resolver = Map::new();
    for p in a.params {
        let v = match args.get(p.nombre).filter(|v| !v.is_null() && v.as_str().is_none_or(|s| !s.trim().is_empty())) {
            Some(v) => valor(p, v)?,
            None => match p.defecto {
                Some(d) => serde_json::from_str(d).unwrap_or(Value::Null),
                None if p.obligatorio => return Err(mal(format!("falta «{}» ({})", p.nombre, p.descripcion))),
                None => continue,
            },
        };
        if p.destino.starts_with(POR_RESOLVER) {
            por_resolver.insert(p.nombre.to_string(), v);
        } else {
            poner(&mut pedido, if p.destino.is_empty() { p.nombre } else { p.destino }, v);
        }
    }
    let mut hecha = AccionApp {
        herramienta: a.herramienta,
        pedido,
        confirmar: None,
        por_resolver,
    };
    hecha.confirmar = a.confirmar.map(|q| rellenar(q, &hecha.valores()));
    Ok(hecha)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn args(s: &str) -> Map<String, Value> {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn los_nombres_no_se_repiten_y_cada_accion_existe_en_el_protocolo() {
        let mut nombres: Vec<&str> = ACCIONES.iter().map(|a| a.herramienta).collect();
        nombres.sort();
        let n = nombres.len();
        nombres.dedup();
        assert_eq!(nombres.len(), n);
        let protocolo = include_str!("../../../../docs/protocolo-pedidos.md");
        for a in ACCIONES {
            assert!(
                a.accion == CONSULTA || protocolo.contains(&format!("| `{}` |", a.accion)),
                "{} no esta en el protocolo",
                a.accion
            );
            assert!(serde_json::from_str::<Value>(a.fijos).is_ok(), "{}", a.herramienta);
            assert!(a.accion == CONSULTA || !a.hecho.is_empty(), "{} sin frase de hecho", a.herramienta);
            for p in a.params {
                if let Some(d) = p.defecto {
                    assert!(serde_json::from_str::<Value>(d).is_ok(), "{}.{}", a.herramienta, p.nombre);
                }
            }
        }
    }

    #[test]
    fn el_esquema_sale_de_la_tabla() {
        let h = herramienta(por_nombre("abrir_ventana").unwrap());
        assert_eq!(h.parametros["required"], json!(["cual"]));
        assert_eq!(h.parametros["properties"]["cual"]["enum"][1], "galeria");
        let sin = herramienta(por_nombre("capturar_pantalla").unwrap());
        assert!(sin.parametros.get("required").is_none());
        let varias = herramienta(por_nombre("anadir_tareas").unwrap());
        assert_eq!(varias.parametros["properties"]["tareas"]["items"]["type"], "string");
    }

    #[test]
    fn el_pedido_sale_bien_armado() {
        let a = validar(por_nombre("abrir_fichero").unwrap(), &args(r#"{"ruta":" «C:\\x\\plano.pdf» "}"#)).unwrap();
        assert_eq!(a.pedido, json!({"pixpin":1,"accion":"abrir","que":{"tipo":"fichero","ruta":"C:\\x\\plano.pdf"}}));
        assert!(a.confirmar.is_none() && a.por_resolver.is_empty());
        let a = validar(por_nombre("capturar_pantalla").unwrap(), &Map::new()).unwrap();
        assert_eq!(a.pedido, json!({"pixpin":1,"accion":"capturar","modo":"zona"}));
        let a = validar(por_nombre("abrir_ventana").unwrap(), &args(r#"{"cual":"Galería"}"#)).unwrap();
        assert_eq!(a.pedido["cual"], "galeria");
    }

    #[test]
    fn las_tareas_se_piden_por_su_nombre() {
        let a = validar(por_nombre("marcar_tarea").unwrap(), &args(r#"{"tarea":"comprar pan","lista":"Compras"}"#)).unwrap();
        assert_eq!(a.pedido, json!({"pixpin":1,"accion":"marcar_tarea","hecha":true}));
        assert_eq!(a.por_resolver, args(r#"{"tarea":"comprar pan","lista":"Compras"}"#));
        let a = validar(por_nombre("anadir_tarea").unwrap(), &args(r#"{"texto":"pan","fecha":"mañana","lista":"  "}"#)).unwrap();
        assert_eq!(a.pedido["texto"], "pan");
        assert_eq!(a.por_resolver, args(r#"{"fecha":"mañana"}"#), "lo vacio es como no ponerlo");
        let a = validar(por_nombre("consultar_tareas").unwrap(), &Map::new()).unwrap();
        assert!(a.es_consulta());
        assert_eq!(a.por_resolver["estado"], "pendientes");
        let a = validar(por_nombre("anadir_tareas").unwrap(), &args(r#"{"tareas":["a"," ","b"]}"#)).unwrap();
        assert_eq!(a.por_resolver["tareas"], json!(["a", "b"]));
    }

    #[test]
    fn lo_que_borra_o_sale_del_pc_pregunta() {
        let a = validar(por_nombre("borrar_lista").unwrap(), &args(r#"{"lista":"Obra"}"#)).unwrap();
        assert_eq!(
            a.confirmar.as_deref(),
            Some("¿Borro la lista «Obra» entera, con todas sus tareas? No se puede deshacer.")
        );
        let a = validar(por_nombre("sincronizar_movil").unwrap(), &Map::new()).unwrap();
        assert_eq!(a.pedido["aparato"], "todos");
        assert!(a.confirmar.is_some());
        for nombre in ["quitar_tarea", "borrar_captura", "enviar_al_movil"] {
            assert!(por_nombre(nombre).unwrap().confirmar.is_some(), "{nombre}");
        }
    }

    #[test]
    fn las_frases_se_rellenan_sin_huecos() {
        let v = |x: &[(&str, &str)]| x.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>();
        let h = por_nombre("anadir_tarea").unwrap().hecho;
        assert_eq!(rellenar(h, &v(&[("texto", "pan")])), "Apuntada «pan».");
        assert_eq!(
            rellenar(h, &v(&[("texto", "pan"), ("lista", "Compras"), ("fecha", "mañana 10:00")])),
            "Apuntada «pan» en «Compras» para mañana 10:00."
        );
        assert_eq!(rellenar("Abriendo {ruta}.", &v(&[])), "Abriendo .");
    }

    #[test]
    fn lo_mal_puesto_se_rechaza_con_el_motivo() {
        let e = validar(por_nombre("lista_nueva").unwrap(), &Map::new()).unwrap_err();
        assert!(e.to_string().contains("«titulo»"), "{e}");
        let e = validar(por_nombre("abrir_ventana").unwrap(), &args(r#"{"cual":"cocina"}"#)).unwrap_err();
        assert!(e.to_string().contains("tareas, galeria"), "{e}");
        assert!(validar(por_nombre("marcar_tarea").unwrap(), &args(r#"{"lista":"x"}"#)).is_err(), "sin tarea");
        assert!(validar(por_nombre("anadir_tarea").unwrap(), &args(r#"{"texto":"  "}"#)).is_err());
        assert!(validar(por_nombre("anadir_tareas").unwrap(), &args(r#"{"tareas":[" "]}"#)).is_err());
        assert!(validar(por_nombre("enviar_al_movil").unwrap(), &args(r#"{"imagenes":[]}"#)).is_err());
    }
}
