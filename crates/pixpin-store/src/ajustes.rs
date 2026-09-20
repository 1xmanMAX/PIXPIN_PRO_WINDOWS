//! Los ajustes de la aplicacion, en TOML.
//!
//! Dos reglas gobiernan el formato, y las dos existen para que el fichero se
//! pueda editar a mano sin miedo:
//!
//! - **Todo campo que falte se rellena con su valor por defecto.** Un usuario
//!   que solo quiere cambiar un atajo escribe dos lineas, no el fichero entero.
//! - **Las claves desconocidas se ignoran.** Un fichero escrito por una version
//!   mas nueva no impide arrancar a una mas vieja.

use std::fs;
use std::path::PathBuf;

use pixpin_shell::Atajo;
use serde::{Deserialize, Serialize};

use crate::rutas::Ubicacion;

#[derive(Debug, thiserror::Error)]
pub enum ErrorAjustes {
    #[error("no se pudo leer {ruta}: {fuente}")]
    Lectura {
        ruta: PathBuf,
        #[source]
        fuente: std::io::Error,
    },
    #[error("no se pudo escribir {ruta}: {fuente}")]
    Escritura {
        ruta: PathBuf,
        #[source]
        fuente: std::io::Error,
    },
    #[error("el fichero de ajustes tiene un error: {0}")]
    Formato(#[from] toml::de::Error),
    #[error("no se pudieron serializar los ajustes: {0}")]
    Serializacion(#[from] toml::ser::Error),
    /// La del guardado que conserva comentarios, que usa otro escritor.
    #[error("no se pudieron serializar los ajustes: {0}")]
    SerializacionConservando(#[from] toml_edit::ser::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PreferenciaIdioma {
    /// Se toma del idioma de Windows.
    #[default]
    Sistema,
    #[serde(rename = "es")]
    Espanol,
    #[serde(rename = "en")]
    Ingles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PreferenciaNivel {
    /// La aplicacion mide el equipo al arrancar y decide.
    #[default]
    Auto,
    Completo,
    Ligero,
}

/// Seccion `[rendimiento]` del fichero de ajustes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Rendimiento {
    /// `auto` deja decidir a los hechos; `completo` y `ligero` fuerzan el
    /// nivel. Forzar `ligero` en una maquina potente es legitimo y util:
    /// es como se prueba la ruta ligera sin tener hardware flojo delante.
    pub nivel: PreferenciaNivel,
    /// Registrar cada 60 fotogramas del editor cuanto se tarda en vaciar la
    /// cola, pintar, presentar y esperar (D129). Es para diagnosticar en el
    /// equipo del usuario sin entrada sintetizada; apagado no registra nada.
    pub medir_fotogramas: bool,
}

/// Como se suaviza el trazo a mano (`[tinta] suavizado`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Suavizado {
    /// Lo de siempre: solo el `streamline` de perfect-freehand, que es lo
    /// que hace Excalidraw. Los ficheros salen identicos a los de antes.
    #[default]
    Excalidraw,
    /// Ademas, el filtro de 1 euro delante (`pixpin-tinta`). Quita el
    /// temblor de la mano parada sin retrasar la punta cuando se va deprisa,
    /// que es donde `streamline` se siente como una goma. Cambia un poco la
    /// forma del trazo: por eso no es el de por defecto.
    Natural,
}

/// Seccion `[tinta]` del fichero de ajustes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Tinta {
    pub suavizado: Suavizado,
    /// `mincutoff` del filtro de 1 euro, en hercios: el corte con el lapiz
    /// quieto. Bajarlo quita mas temblor y retrasa mas al arrancar.
    /// `None` = el valor conservador del propio motor.
    pub corte_minimo: Option<f32>,
    /// `beta`: cuanto sube el corte con la velocidad. Subirlo pega mas la
    /// punta al cursor a costa de dejar pasar mas temblor.
    pub beta: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FormatoColor {
    #[default]
    Hex,
    Rgb,
    Hsl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Atajos {
    /// Capturar region y mostrar la barra de resultado. Es el unico atajo
    /// de fabrica (D140): lleva a la seleccion y alli se elige que hacer.
    pub region: Option<Atajo>,
    /// Capturar region y copiar directo al portapapeles, sin confirmacion.
    pub copiar: Option<Atajo>,
    /// Captura larga con scroll.
    pub scroll: Option<Atajo>,
    /// Cuentagotas global. Sin atajo por defecto (D81).
    pub cuentagotas: Option<Atajo>,
    /// Recortar y dejar flotando como pin (S2).
    pub pin: Option<Atajo>,
    /// Pinear el contenido del portapapeles (S2-B).
    pub portapapeles: Option<Atajo>,
    /// Anotar sobre la pantalla con la capa viva (S3-C).
    pub anotar: Option<Atajo>,
    /// Anotar sobre una captura estatica de la pantalla (S3-C, D56). Sin
    /// atajo por defecto (D81).
    pub anotar_congelada: Option<Atajo>,
}

impl Default for Atajos {
    fn default() -> Self {
        // `expect` es correcto aqui: si una constante del propio codigo no
        // parsea, es un fallo de programacion y debe verse en el primer test.
        Self {
            // El unico atajo de fabrica (D140). Los demas nacen sin atajo:
            // lo rapido va por los gestos con Alt, y quien quiera alguno lo
            // escribe en el TOML.
            region: Some("Ctrl+Alt+X".parse().expect("atajo por defecto valido")),
            copiar: None,
            scroll: None,
            cuentagotas: None,
            pin: None,
            portapapeles: None,
            anotar: None,
            anotar_congelada: None,
        }
    }
}

/// Lo que se recuerda de una grabacion a la siguiente.
///
/// Ajustar el ritmo cada vez seria un impuesto: quien graba interfaces
/// suele quedarse en el mismo numero durante meses. El retardo esta por
/// lo mismo que en el original: da tiempo a poner el raton donde toca
/// antes de que empiece a contar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Gif {
    /// Fotogramas por segundo. Se guarda el numero y no el indice de la
    /// lista para que el fichero siga significando lo mismo si algun dia
    /// se ofrecen otros ritmos.
    pub por_segundo: u32,
    /// Segundos de cortesia entre pulsar «Grabar» y el primer fotograma.
    pub retardo_s: u32,
}

impl Default for Gif {
    fn default() -> Self {
        Self {
            por_segundo: 10,
            retardo_s: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ajustes {
    pub idioma: PreferenciaIdioma,
    /// La tabla vieja, de cuando cada atajo era un campo. Se sigue leyendo
    /// para no romper el fichero de nadie, pero lo que manda es `comandos`.
    pub atajos: Atajos,
    /// Nombre de comando -> atajo, la tabla nueva (ver `comandos.rs`). Lo
    /// que no se nombra conserva su atajo por defecto; una cadena vacia deja
    /// el comando sin atajo.
    pub comandos: std::collections::BTreeMap<String, String>,
    /// Si es `None` se usa la carpeta Imagenes del usuario.
    pub carpeta_capturas: Option<PathBuf>,
    pub formato_color: FormatoColor,
    pub rendimiento: Rendimiento,
    /// Como se siente el lapiz (`[tinta]`). Vacio = como siempre.
    pub tinta: Tinta,
    pub arranque_con_windows: bool,
    /// Tope de altura de la captura con scroll. Sin el, una pagina infinita
    /// capturaria hasta agotar la memoria.
    pub limite_scroll_px: u32,
    /// Lo que se recuerda de la grabacion (P5b).
    pub gif: Gif,
    /// Programas delante de los cuales los atajos no actuan (P1.8).
    ///
    /// Se nombran por ejecutable, con o sin `.exe`, sin distinguir
    /// mayusculas. Por defecto esta vacia: nadie ha pedido que PixPin
    /// deje de responder, y una lista con algo dentro de fabrica seria
    /// una sorpresa muy dificil de averiguar.
    pub ignorar_programas: Vec<String>,
    /// Segundos que espera la captura con retardo antes de abrirse
    /// (P2.1). Tres es lo justo para desplegar un menu y soltar el raton.
    pub retardo_captura_s: u32,
    /// Zonas guardadas con nombre, cada una con su atajo (P2.3).
    ///
    /// Es una tabla repetible (`[[regiones]]`) y por eso va DESPUES de
    /// las claves sueltas en el fichero, como cualquier seccion.
    pub regiones: Vec<crate::regiones::Region>,
    /// Salir en «Abrir con» de Windows para imagenes y videos.
    ///
    /// Es lo UNICO que escribe en el registro estando en modo portable, y
    /// por eso tiene interruptor: el modo portable promete no dejar rastro
    /// en el equipo. Apagarlo borra lo escrito, no solo deja de escribir.
    pub abrir_con: bool,
    /// A que se pega el cursor al dibujar (fase B.6).
    ///
    /// El tipo vive en el motor y no aqui, aunque eso ate `pixpin-store` a
    /// `pixpin-motor2d`: la alternativa era una copia del struct con los
    /// mismos campos y una traduccion, o sea dos sitios que hay que
    /// acordarse de tocar a la vez -y un campo que se olvide en la copia es
    /// un ajuste que se guarda y no se lee-. La regla de capas lo permite:
    /// store es capa 2 y motor2d es capa 1.
    pub enganche: pixpin_motor2d::enganche::Ajustes,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            idioma: PreferenciaIdioma::default(),
            atajos: Atajos::default(),
            comandos: std::collections::BTreeMap::new(),
            carpeta_capturas: None,
            formato_color: FormatoColor::default(),
            rendimiento: Rendimiento::default(),
            tinta: Tinta::default(),
            arranque_con_windows: false,
            limite_scroll_px: 30_000,
            gif: Gif::default(),
            ignorar_programas: Vec::new(),
            retardo_captura_s: 3,
            regiones: Vec::new(),
            abrir_con: true,
            enganche: pixpin_motor2d::enganche::Ajustes::default(),
        }
    }
}

/// Lee los ajustes. Si el fichero no existe, devuelve los valores por defecto.
pub fn cargar(ubicacion: &Ubicacion) -> Result<Ajustes, ErrorAjustes> {
    let ruta = ubicacion.fichero_ajustes();
    let texto = match fs::read_to_string(&ruta) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Ajustes::default()),
        Err(fuente) => return Err(ErrorAjustes::Lectura { ruta, fuente }),
    };
    Ok(toml::from_str(&texto)?)
}

/// Escribe los ajustes, creando el directorio si hace falta.
pub fn guardar(ubicacion: &Ubicacion, ajustes: &Ajustes) -> Result<(), ErrorAjustes> {
    let ruta = ubicacion.fichero_ajustes();
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre).map_err(|fuente| ErrorAjustes::Escritura {
            ruta: padre.to_path_buf(),
            fuente,
        })?;
    }
    let texto = toml::to_string_pretty(ajustes)?;
    fs::write(&ruta, texto).map_err(|fuente| ErrorAjustes::Escritura { ruta, fuente })
}

/// Guarda los ajustes SIN perder los comentarios ni el orden del
/// fichero (P6).
///
/// `guardar` serializa la estructura entera y reescribe el fichero de
/// cero: sirve para crearlo, pero usarlo desde la ventana de ajustes le
/// borraria al usuario todas las explicaciones que tiene escritas dentro,
/// que en este fichero son la mitad del contenido. Esto lee lo que hay,
/// cambia solo los VALORES, y devuelve el resto tal cual: comentarios,
/// orden, lineas en blanco y hasta las claves que no conocemos.
///
/// La fusion es generica y no campo a campo a proposito. Escribir a mano
/// «pon idioma, pon comandos, pon gif...» significa que el dia que se
/// anada un ajuste, alguien se olvidara de anadirlo aqui y ese ajuste
/// dejara de guardarse sin que falle nada. Lo vigila ademas la prueba
/// `ningun_ajuste_se_queda_sin_guardar`.
pub fn guardar_conservando(ubicacion: &Ubicacion, ajustes: &Ajustes) -> Result<(), ErrorAjustes> {
    let ruta = ubicacion.fichero_ajustes();
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre).map_err(|fuente| ErrorAjustes::Escritura {
            ruta: padre.to_path_buf(),
            fuente,
        })?;
    }
    // Si no hay fichero, o el que hay no se puede leer como TOML, se
    // empieza de uno vacio. No se aborta: quedarse sin poder guardar
    // porque el fichero de antes estaba roto seria dejar al usuario
    // atrapado, y lo que va a escribirse es valido de todas formas.
    let existente = fs::read_to_string(&ruta).unwrap_or_default();
    let mut documento = existente
        .parse::<toml_edit::DocumentMut>()
        .unwrap_or_default();
    let nuevo = toml_edit::ser::to_document(ajustes)?;
    fusionar(documento.as_table_mut(), nuevo.as_table());
    fs::write(&ruta, documento.to_string())
        .map_err(|fuente| ErrorAjustes::Escritura { ruta, fuente })
}

/// Copia los valores de `origen` sobre `destino`, entrando en las tablas
/// en vez de reemplazarlas.
///
/// Reemplazar la tabla entera se llevaria por delante los comentarios de
/// cada linea de dentro, que es justo lo que hay que conservar. Y lo que
/// esta en `destino` y no en `origen` se QUEDA: si el usuario escribio una
/// clave que no conocemos, no es asunto nuestro borrarsela.
fn fusionar(destino: &mut toml_edit::Table, origen: &toml_edit::Table) {
    for (clave, valor) in origen.iter() {
        // Si los dos lados son una tabla, se entra. Ojo: el serializador
        // produce tablas EN LINEA (`{ a = 1 }`) donde el fichero escrito a
        // mano tiene secciones (`[a]`), asi que hay que reconocer las dos
        // formas. Sin esto, `[comandos]` se sustituia entera por una tabla
        // en linea y se perdian de golpe el comentario de la seccion y el
        // de cada atajo de dentro.
        if let (Some(toml_edit::Item::Table(d)), Some(o)) =
            (destino.get_mut(clave), como_tabla(valor))
        {
            fusionar(d, &o);
            continue;
        }
        // El adorno de la clave (sus comentarios y su sangria) va pegado a
        // la clave y no al valor, asi que asignar el valor los conserva
        // sin hacer nada mas.
        destino[clave] = valor.clone();
    }
}

/// Ve un elemento como tabla, venga como seccion o como tabla en linea.
fn como_tabla(item: &toml_edit::Item) -> Option<toml_edit::Table> {
    match item {
        toml_edit::Item::Table(t) => Some(t.clone()),
        toml_edit::Item::Value(toml_edit::Value::InlineTable(t)) => Some(t.clone().into_table()),
        _ => None,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    fn temporal(etiqueta: &str) -> Ubicacion {
        let dir = std::env::temp_dir().join(format!("pixpin-ajustes-{etiqueta}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Ubicacion::Instalado { raiz: dir }
    }

    /// Unos ajustes con TODO cambiado respecto a los de fabrica.
    ///
    /// Que no quede ni un campo en su valor por defecto es lo que hace
    /// util la prueba de ida y vuelta: si un campo se quedara sin
    /// guardar, volveria a su valor por defecto y la comparacion no lo
    /// notaria si ese ya era su valor.
    fn todo_cambiado() -> Ajustes {
        let atajos = Atajos {
            copiar: Some("Ctrl+Shift+F9".parse().unwrap()),
            ..Atajos::default()
        };
        let mut comandos = std::collections::BTreeMap::new();
        comandos.insert("grabar-gif".to_string(), "Ctrl+Alt+G".to_string());
        Ajustes {
            idioma: PreferenciaIdioma::Ingles,
            atajos,
            comandos,
            carpeta_capturas: Some(PathBuf::from("C:/capturas")),
            formato_color: FormatoColor::Hsl,
            arranque_con_windows: true,
            limite_scroll_px: 12_345,
            gif: Gif {
                por_segundo: 25,
                retardo_s: 4,
            },
            ignorar_programas: vec!["juego.exe".into()],
            retardo_captura_s: 7,
            regiones: vec![crate::regiones::Region {
                nombre: "panel".into(),
                x: 1,
                y: 2,
                ancho: 300,
                alto: 200,
                atajo: Some("Ctrl+Alt+1".into()),
            }],
            enganche: pixpin_motor2d::enganche::Ajustes {
                activo: false,
                esquinas: false,
                medios: false,
                centros: false,
                radio_px: 20.0,
            },
            rendimiento: Rendimiento {
                nivel: PreferenciaNivel::Ligero,
                medir_fotogramas: true,
            },
            ..Ajustes::default()
        }
    }

    #[test]
    fn ningun_ajuste_se_queda_sin_guardar() {
        // La red de seguridad de `guardar_conservando`: como la fusion es
        // generica, anadir un ajuste nuevo NO deberia pedir tocarla. Esta
        // prueba lo comprueba de verdad, con todos los campos cambiados y
        // una vuelta completa por el disco.
        let u = temporal("conservando-todo");
        let esperado = todo_cambiado();
        guardar_conservando(&u, &esperado).unwrap();
        assert_eq!(cargar(&u).unwrap(), esperado);
    }

    #[test]
    fn guardar_no_se_lleva_por_delante_los_comentarios() {
        // Es la razon de ser de esta funcion. `guardar` a secas reescribe
        // el fichero de cero y borraria todo esto, que en el fichero real
        // del usuario es la mitad del contenido.
        let u = temporal("conservando-comentarios");
        let antes = "# Lo que explica el fichero entero.
\n                     limite_scroll_px = 100

\n                     # Lo que explica los comandos.
\n                     [comandos]
\n                     # Este captura y copia.
\n                     capturar-y-copiar = \"Ctrl+Alt+C\"
\n                     # Una clave que no conocemos, escrita a mano.
\n                     invento-del-usuario = \"Ctrl+F12\"
";
        fs::write(u.fichero_ajustes(), antes).unwrap();

        let mut a = cargar(&u).unwrap();
        a.limite_scroll_px = 999;
        guardar_conservando(&u, &a).unwrap();

        let despues = fs::read_to_string(u.fichero_ajustes()).unwrap();
        assert!(despues.contains("# Lo que explica el fichero entero."));
        assert!(despues.contains("# Lo que explica los comandos."));
        assert!(despues.contains("# Este captura y copia."));
        // Lo que el usuario escribio y nosotros no entendemos SE QUEDA:
        // borrarselo no es asunto nuestro.
        assert!(
            despues.contains("invento-del-usuario"),
            "se perdio una clave ajena:
{despues}"
        );
        // Y el valor si cambio.
        assert_eq!(cargar(&u).unwrap().limite_scroll_px, 999);
    }

    #[test]
    fn un_fichero_roto_no_impide_guardar() {
        // Caso negativo: quedarse sin poder guardar porque lo que habia
        // antes estaba mal escrito dejaria al usuario atrapado, sin manera
        // de arreglarlo desde el programa. Se empieza de cero y se guarda.
        let u = temporal("conservando-roto");
        fs::write(u.fichero_ajustes(), "esto no [[[ es TOML").unwrap();
        let a = Ajustes {
            limite_scroll_px: 42,
            ..Ajustes::default()
        };
        guardar_conservando(&u, &a).unwrap();
        assert_eq!(cargar(&u).unwrap().limite_scroll_px, 42);
    }

    #[test]
    fn una_clave_suelta_tras_una_seccion_no_es_del_nivel_de_arriba() {
        // Esto no prueba nuestro codigo, prueba TOML â y esta aqui porque
        // ya me costo un fallo en el fichero del usuario: puse
        // `ignorar_programas` DEBAJO de `[gif]`, y TOML lo leyo como
        // `gif.ignorar_programas`. Serde ignora lo que no conoce, asi que
        // el ajuste desaparecio sin una sola queja.
        //
        // Una clave de primer nivel tiene que ir ANTES de la primera
        // seccion. Si algun dia se genera este fichero desde codigo, esta
        // prueba dice por que el orden importa.
        let mal = "[gif]
por_segundo = 20
ignorar_programas = ['x.exe']
";
        let a: Ajustes = toml::from_str(mal).expect("carga igual, y ese es el problema");
        assert_eq!(a.gif.por_segundo, 20);
        assert!(
            a.ignorar_programas.is_empty(),
            "la clave se perdio dentro de [gif], como se esperaba"
        );
        // Puesta donde toca, si llega.
        let bien = "ignorar_programas = ['x.exe']
[gif]
por_segundo = 20
";
        let a: Ajustes = toml::from_str(bien).unwrap();
        assert_eq!(a.ignorar_programas, vec!["x.exe".to_string()]);
        assert_eq!(a.gif.por_segundo, 20);
    }

    #[test]
    fn lo_que_se_elige_al_grabar_vuelve_igual() {
        let u = temporal("gif");
        let mut a = Ajustes::default();
        assert_eq!(a.gif.por_segundo, 10);
        assert_eq!(a.gif.retardo_s, 0);
        a.gif.por_segundo = 25;
        a.gif.retardo_s = 3;
        guardar(&u, &a).unwrap();
        assert_eq!(cargar(&u).unwrap().gif, a.gif);
    }

    #[test]
    fn un_fichero_sin_la_seccion_de_grabar_sigue_valiendo() {
        // Caso negativo del que se rompe al anadir un ajuste: el fichero
        // de quien ya tenia la aplicacion no lleva la seccion nueva, y no
        // puede dejar de cargar por eso. Lo que falta toma su valor por
        // defecto y el resto se respeta.
        let u = temporal("gif-viejo");
        fs::write(
            u.fichero_ajustes(),
            "limite_scroll_px = 1234
arranque_con_windows = true
",
        )
        .unwrap();
        let a = cargar(&u).unwrap();
        assert_eq!(a.limite_scroll_px, 1234);
        assert!(a.arranque_con_windows);
        assert_eq!(a.gif, Gif::default());
    }

    #[test]
    fn los_valores_por_defecto_son_los_del_diseno() {
        let a = Ajustes::default();
        // Un solo atajo general (D140); lo demas va por gesto (Alt + boton)
        // y por la bandeja.
        assert_eq!(
            a.atajos.region.map(|r| r.to_string()).as_deref(),
            Some("Ctrl+Alt+X")
        );
        for otro in [
            a.atajos.copiar,
            a.atajos.scroll,
            a.atajos.cuentagotas,
            a.atajos.pin,
            a.atajos.portapapeles,
            a.atajos.anotar,
            a.atajos.anotar_congelada,
        ] {
            assert_eq!(otro, None, "ningun otro atajo de fabrica");
        }
        assert_eq!(a.idioma, PreferenciaIdioma::Sistema);
        assert_eq!(a.formato_color, FormatoColor::Hex);
        assert!(!a.arranque_con_windows);
    }

    #[test]
    fn sobrevive_la_ida_y_vuelta_por_toml() {
        let original = Ajustes {
            idioma: PreferenciaIdioma::Ingles,
            arranque_con_windows: true,
            atajos: Atajos {
                region: Some("Ctrl+Shift+F1".parse().unwrap()),
                ..Default::default()
            },
            ..Default::default()
        };

        let texto = toml::to_string_pretty(&original).unwrap();
        let vuelta: Ajustes = toml::from_str(&texto).unwrap();

        assert_eq!(original, vuelta);
    }

    #[test]
    fn si_no_hay_fichero_se_usan_los_valores_por_defecto() {
        let u = temporal("sin-fichero");
        let a = cargar(&u).unwrap();
        assert_eq!(a, Ajustes::default());
    }

    #[test]
    fn un_fichero_a_medias_completa_con_los_valores_por_defecto() {
        // Es el caso real de un usuario que edita el TOML a mano y solo
        // escribe lo que quiere cambiar. No debe romper nada.
        let u = temporal("parcial");
        fs::write(u.fichero_ajustes(), "arranque_con_windows = true\n").unwrap();

        let a = cargar(&u).unwrap();

        assert!(a.arranque_con_windows);
        assert_eq!(a.atajos, Atajos::default());
    }

    #[test]
    fn las_claves_desconocidas_se_ignoran() {
        // Compatibilidad hacia atras: un fichero escrito por una version mas
        // nueva no debe impedir que arranque una version mas vieja.
        let u = temporal("desconocidas");
        fs::write(
            u.fichero_ajustes(),
            "arranque_con_windows = true\nfuncion_del_futuro = 42\n",
        )
        .unwrap();

        let a = cargar(&u).unwrap();

        assert!(a.arranque_con_windows);
    }

    #[test]
    fn un_atajo_invalido_da_error_con_mensaje_util() {
        let u = temporal("atajo-malo");
        fs::write(u.fichero_ajustes(), "[atajos]\nregion = \"NoEsUnAtajo\"\n").unwrap();

        let e = cargar(&u).unwrap_err();

        assert!(
            e.to_string().contains("NoEsUnAtajo"),
            "el error debe decir que valor concreto esta mal, dijo: {e}"
        );
    }

    #[test]
    fn el_nivel_de_rendimiento_se_lee_y_por_defecto_es_auto() {
        let ajustes: Ajustes = toml::from_str("[rendimiento]\nnivel = \"ligero\"").unwrap();
        assert_eq!(ajustes.rendimiento.nivel, PreferenciaNivel::Ligero);
        // Un fichero sin la seccion conserva el valor por defecto: la regla
        // de "todo campo que falte se rellena" tambien vale para secciones.
        let vacios: Ajustes = toml::from_str("").unwrap();
        assert_eq!(vacios.rendimiento.nivel, PreferenciaNivel::Auto);
    }

    #[test]
    fn la_tinta_nace_como_siempre_y_el_toml_la_puede_poner_natural() {
        // Lo que no se escribe no cambia el trazo: un fichero sin `[tinta]`
        // tiene que dar exactamente el lapiz de antes.
        let vacios: Ajustes = toml::from_str("").unwrap();
        assert_eq!(vacios.tinta.suavizado, Suavizado::Excalidraw);
        assert_eq!(vacios.tinta.corte_minimo, None);

        let a: Ajustes = toml::from_str("[tinta]\nsuavizado = \"natural\"\nbeta = 0.05").unwrap();
        assert_eq!(a.tinta.suavizado, Suavizado::Natural);
        assert_eq!(a.tinta.beta, Some(0.05));
        // Lo que no se nombra sigue en `None`, que significa «el valor
        // conservador del motor», no cero.
        assert_eq!(a.tinta.corte_minimo, None);
    }

    #[test]
    fn un_suavizado_desconocido_da_error_en_vez_de_adivinar() {
        // Caso negativo: si "suabe" pasara por bueno, el usuario creeria
        // haber encendido algo que no esta pasando.
        assert!(toml::from_str::<Ajustes>("[tinta]\nsuavizado = \"suabe\"").is_err());
    }

    #[test]
    fn un_nivel_desconocido_da_error_en_vez_de_adivinar() {
        // Caso negativo: "turbo" no existe. Adivinar seria peor que fallar,
        // porque el usuario cree haber forzado algo que no esta pasando.
        let resultado = toml::from_str::<Ajustes>("[rendimiento]\nnivel = \"turbo\"");
        assert!(resultado.is_err());
    }

    #[test]
    fn medir_fotogramas_se_lee_y_por_defecto_esta_apagado() {
        let a: Ajustes = toml::from_str("[rendimiento]\nmedir_fotogramas = true").unwrap();
        assert!(a.rendimiento.medir_fotogramas);
        assert_eq!(
            a.rendimiento.nivel,
            PreferenciaNivel::Auto,
            "lo que no se nombra conserva su valor"
        );
        // Caso negativo: un fichero de antes no tiene la clave y no mide nada.
        let viejo: Ajustes = toml::from_str("[rendimiento]\nnivel = \"ligero\"").unwrap();
        assert!(!viejo.rendimiento.medir_fotogramas);
    }

    #[test]
    fn guardar_crea_el_directorio_si_no_existe() {
        let dir = std::env::temp_dir().join("pixpin-ajustes-crear/anidado");
        let _ = fs::remove_dir_all(dir.parent().unwrap());
        let u = Ubicacion::Instalado { raiz: dir.clone() };

        guardar(&u, &Ajustes::default()).unwrap();

        assert!(u.fichero_ajustes().is_file());
        assert_eq!(cargar(&u).unwrap(), Ajustes::default());
    }

    #[test]
    fn los_ajustes_del_iman_van_y_vuelven() {
        let mut a = Ajustes::default();
        a.enganche.centros = false;
        a.enganche.radio_px = 20.0;

        let texto = toml::to_string(&a).expect("serializa");
        let vuelta: Ajustes = toml::from_str(&texto).expect("deserializa");

        assert!(!vuelta.enganche.centros);
        assert_eq!(vuelta.enganche.radio_px, 20.0);
        assert!(vuelta.enganche.activo, "lo no tocado conserva su valor");
    }

    #[test]
    fn un_fichero_viejo_sin_iman_sigue_abriendo() {
        // Nadie tiene la seccion `[enganche]` en su fichero todavia. Si
        // faltar la rompiera, la actualizacion le borraria los ajustes.
        // El TOML de abajo se parece a un fichero real de hoy -claves
        // sueltas, `[atajos]`, `[comandos]`, `[gif]`, `[rendimiento]` y una
        // tabla repetible `[[regiones]]`- y no a un minimo artificial que
        // no ejercite el caso.
        let viejo = "arranque_con_windows = true
limite_scroll_px = 20000
retardo_captura_s = 5
abrir_con = true

[atajos]
copiar = \"Ctrl+Alt+C\"
scroll = \"Ctrl+Alt+S\"
pin = \"Ctrl+Alt+F\"
portapapeles = \"Ctrl+Alt+V\"
anotar = \"Ctrl+Alt+A\"

[comandos]
capturar-y-copiar = \"Ctrl+Alt+C\"

[gif]
por_segundo = 15
retardo_s = 2

[rendimiento]
nivel = \"auto\"

[[regiones]]
nombre = \"panel\"
x = 1
y = 2
ancho = 300
alto = 200
";
        let a: Ajustes = toml::from_str(viejo).expect("un fichero viejo tiene que abrir");
        assert!(a.enganche.activo, "el iman nace encendido");
        assert_eq!(a.enganche.radio_px, 14.0);
        // Y el resto del fichero viejo se sigue leyendo tal cual.
        assert_eq!(a.limite_scroll_px, 20000);
        assert_eq!(a.regiones.len(), 1);
    }

    #[test]
    fn el_iman_sobrevive_la_ida_y_vuelta_con_regiones_de_por_medio() {
        // Esta prueba fija un supuesto del que dependemos sin haberlo
        // escrito en ningun sitio: que `toml` coloca las TABLAS (una
        // seccion `[tabla]`, o una tabla repetible `[[tabla]]`) despues de
        // las claves sueltas del fichero de salida, sin importar en que
        // orden aparecen los campos en el struct de Rust. Por eso
        // `abrir_con` (booleano suelto) y `enganche` (tabla) pueden
        // convivir con `regiones` (tabla repetible) sin que el fichero
        // salga invalido, aunque `abrir_con` y `enganche` esten declarados
        // DESPUES de `regiones` en el struct `Ajustes`.
        //
        // Las otras pruebas de ida-y-vuelta no lo cubren: las de `toml`
        // puro (`sobrevive_la_ida_y_vuelta_por_toml`,
        // `los_ajustes_del_iman_van_y_vuelven`) parten de `regiones`
        // vacio, y la que si tiene `regiones` poblado
        // (`ningun_ajuste_se_queda_sin_guardar`) pasa por
        // `guardar_conservando` -> `toml_edit` -> `fusionar`, que asigna
        // por nombre de clave y no tiene la restriccion de orden que aqui
        // se comprueba. Esta prueba usa `toml::to_string_pretty` sobre el
        // struct entero, que es lo que hace `guardar()`.
        let original = Ajustes {
            regiones: vec![crate::regiones::Region {
                nombre: "panel".into(),
                x: 1,
                y: 2,
                ancho: 300,
                alto: 200,
                atajo: Some("Ctrl+Alt+1".into()),
            }],
            enganche: pixpin_motor2d::enganche::Ajustes {
                activo: false,
                esquinas: false,
                medios: false,
                centros: false,
                radio_px: 20.0,
            },
            ..Ajustes::default()
        };

        let texto = toml::to_string_pretty(&original).expect("serializa");
        let vuelta: Ajustes = toml::from_str(&texto).expect("un fichero valido tiene que releerse");

        assert_eq!(
            vuelta.regiones, original.regiones,
            "las regiones sobreviven a la ida y vuelta:\n{texto}"
        );
        assert_eq!(
            vuelta.enganche, original.enganche,
            "el iman sobrevive a la ida y vuelta con regiones de por medio:\n{texto}"
        );
    }
}
