//! El cuaderno del movil: un chat contigo mismo.
//!
//! Fotos, ficheros, notas de voz que se pasan a texto, dibujos y paginas de
//! plano, todo en una conversacion. Es la pata que en Windows no existe.
//!
//! Se guarda en `guardados.jsonl`, **una linea de JSON por mensaje**, y se
//! escribe anadiendo al final. Esa forma no es casual y conviene entenderla
//! antes de tocarla:
//!
//! - Guardar un mensaje es escribir una linea. No hay que releer ni
//!   reescribir el fichero entero, asi que guardar cuesta lo mismo con diez
//!   mensajes que con diez mil.
//! - **Una linea rota no invalida el resto.** Si se corta la luz a mitad de
//!   escribir, se pierde ese mensaje y no el cuaderno. Con un JSON unico —un
//!   array de mensajes— un corte a mitad deja un fichero que no abre, y se
//!   pierde todo.
//!
//! Este lector respeta esa promesa: una linea que no se entienda se salta y
//! se cuenta, nunca tumba la lectura.

use serde::{Deserialize, Serialize};

/// De que es cada mensaje.
///
/// Se guardan en mayusculas porque es como las escribe el Kotlin del movil.
/// Lo que no se reconozca cae en `Otra`, con su palabra dentro: una clase
/// nueva del movil tiene que poder leerse aqui, aunque sea para decir que no
/// se sabe ensenarla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Clase {
    #[serde(rename = "NOTA")]
    Nota,
    #[serde(rename = "IMAGEN")]
    Imagen,
    #[serde(rename = "ARCHIVO")]
    Archivo,
    #[serde(rename = "VOZ")]
    Voz,
    #[serde(rename = "DIBUJO")]
    Dibujo,
    #[serde(rename = "PAGINA")]
    Pagina,
    /// Un proyecto entero, como acceso directo. No copia nada.
    #[serde(rename = "PROYECTO")]
    Proyecto,
    /// Una mini-aplicacion: una lista de tareas, unos gastos. El documento
    /// entero va en `texto`.
    #[serde(rename = "MINIAPP")]
    MiniApp,
    #[serde(untagged)]
    Otra(String),
}

/// Un mensaje del cuaderno.
///
/// Todo opcional salvo lo que identifica: el movil anade campos con el
/// tiempo, y un cuaderno escrito por una version mas nueva tiene que abrir
/// igual. Es la misma regla que los ajustes, el indice y el proyecto.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Mensaje {
    pub id: String,
    /// Milisegundos desde 1970.
    pub cuando: i64,
    pub clase: Option<Clase>,
    pub texto: String,
    /// Ruta en el APARATO de origen. En Windows no significa nada por si
    /// sola; sirve para saber que fichero acompanaba al mensaje.
    pub ruta: Option<String>,
    pub nombre: String,
    pub bytes: i64,
    pub referencia: Option<String>,
    /// Pagina dentro de un PDF, cuando la clase es `Pagina`.
    pub pagina: Option<u32>,
    /// Duracion del audio en milisegundos, en una nota de voz.
    #[serde(rename = "duracionMs")]
    pub duracion_ms: i64,
    /// De que conversacion es. `None` es la general — y `None` y no cadena
    /// vacia porque es lo que ya tenian escrito los mensajes de antes de que
    /// existieran los proyectos.
    pub proyecto: Option<String>,
    /// La etiqueta que se le puso: un emoji, o nada.
    pub emoji: Option<String>,
    pub fijado: bool,
    #[serde(rename = "enBuzon")]
    pub en_buzon: bool,
    /// El texto de una nota de voz, si ya se paso a texto.
    pub transcripcion: Option<String>,
    /// De que tipo es una mini-aplicacion. Se guarda la palabra y no un
    /// numero: un numero cambia de significado en cuanto alguien reordena la
    /// lista, y lo guardado no se puede reordenar.
    pub miniapp: Option<String>,
    #[serde(rename = "respondeA")]
    pub responde_a: Option<String>,
    /// Los tres codigos (Android v0.50): el numero en su conversacion, la
    /// letra del aparato de antes, el codigo unico, el aparato donde nacio y
    /// de donde se copio.
    pub numero: i64,
    pub letra: Option<String>,
    pub uid: Option<String>,
    pub aparato: Option<String>,
    pub origen: Option<String>,
    /// Lo que no se entiende, tal cual: guardar no puede perder lo que
    /// anada una version de Android.
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

impl Mensaje {
    pub fn codigo_unico(&self) -> String {
        crate::codigos::unico(self.uid.as_deref(), "m:", &self.id)
    }

    /// `47·K7Q2`, `47a` o nada.
    pub fn codigo_chat(&self) -> Option<String> {
        crate::codigos::de_chat(self.numero, self.aparato.as_deref(), self.letra.as_deref())
    }

    /// Si dos mensajes son la misma cosa: los tres codigos iguales.
    pub fn mismo_que(&self, otro: &Mensaje) -> bool {
        self.codigo_unico() == otro.codigo_unico()
            && self.codigo_chat().is_some()
            && self.codigo_chat() == otro.codigo_chat()
            && self.cuando == otro.cuando
    }

    /// Lo que se ensena de un mensaje en una linea.
    ///
    /// Para una nota de voz se prefiere su transcripcion: el texto de un
    /// mensaje de voz suele estar vacio, y ensenar el nombre del fichero de
    /// audio no le dice nada a nadie.
    pub fn resumen(&self) -> String {
        if let Some(t) = self.transcripcion.as_ref().filter(|t| !t.trim().is_empty()) {
            return t.clone();
        }
        if !self.texto.trim().is_empty() {
            return self.texto.clone();
        }
        if !self.nombre.trim().is_empty() {
            return self.nombre.clone();
        }
        String::new()
    }
}

impl Mensaje {
    /// Una nota escrita aqui, con sus tres codigos ya puestos.
    ///
    /// `numero` es el siguiente de su conversacion (ver
    /// `Cuaderno::siguiente_numero`) y `aparato` el codigo de este equipo:
    /// juntos hacen el codigo de chat (`47·K7Q2`) que ensena PixPin Android.
    pub fn nota(texto: &str, cuando: i64, numero: i64, aparato: &str, proyecto: &str) -> Mensaje {
        Mensaje {
            // El id lleva la hora, como en el movil: ordena solo y no choca
            // con los que ya hay.
            id: format!("{cuando}"),
            cuando,
            clase: Some(Clase::Nota),
            texto: texto.to_string(),
            numero,
            uid: Some(crate::codigos::nuevo()),
            aparato: Some(aparato.to_string()),
            proyecto: Some(proyecto.to_string()),
            ..Default::default()
        }
    }
}

/// Anade un mensaje al cuaderno de una carpeta: **una linea al final**, sin
/// releer ni reescribir nada. Guardar cuesta lo mismo con diez mensajes que
/// con diez mil, y un corte a mitad se lleva ese mensaje y no el cuaderno.
pub fn anadir(carpeta: &std::path::Path, m: &Mensaje) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(carpeta)?;
    let linea = serde_json::to_string(m).map_err(std::io::Error::other)?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(carpeta.join("guardados.jsonl"))?;
    // El salto va DESPUES de la linea: si el fichero se corta, lo roto es lo
    // ultimo y lo anterior sigue entero.
    writeln!(f, "{linea}")
}

/// Un cuaderno leido.
#[derive(Debug, Clone, Default)]
pub struct Cuaderno {
    pub mensajes: Vec<Mensaje>,
    /// Cuantas lineas no se pudieron entender.
    ///
    /// Se cuentan y se ensenan en vez de callarlas: si un cuaderno de mil
    /// mensajes ensena novecientos, el usuario tiene que enterarse de que
    /// faltan cien y no creer que nunca existieron.
    pub lineas_rotas: usize,
}

impl Cuaderno {
    /// Lee un cuaderno de su texto.
    ///
    /// Nunca falla: un cuaderno es una lista de lineas independientes, y la
    /// unica respuesta razonable a una linea rota es saltarsela.
    pub fn leer(texto: &str) -> Cuaderno {
        let mut mensajes = Vec::new();
        let mut lineas_rotas = 0;
        for linea in texto.lines() {
            if linea.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Mensaje>(linea) {
                Ok(m) => mensajes.push(m),
                Err(_) => lineas_rotas += 1,
            }
        }
        Cuaderno {
            mensajes,
            lineas_rotas,
        }
    }

    /// Lee el `guardados.jsonl` de una carpeta.
    pub fn leer_de(carpeta: &std::path::Path) -> std::io::Result<Cuaderno> {
        let texto = std::fs::read_to_string(carpeta.join("guardados.jsonl"))?;
        Ok(Cuaderno::leer(&texto))
    }

    /// El numero que le toca al siguiente mensaje de una conversacion.
    ///
    /// Se mira el mayor y se suma uno, en vez de contar cuantos hay: un
    /// mensaje borrado dejaria hueco y dos mensajes distintos acabarian con
    /// el mismo numero, que es justo lo que los tres codigos evitan.
    pub fn siguiente_numero(&self, proyecto: Option<&str>) -> i64 {
        self.de_conversacion(proyecto)
            .iter()
            .map(|m| m.numero)
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Los mensajes de una conversacion. `None` es la general.
    pub fn de_conversacion(&self, proyecto: Option<&str>) -> Vec<&Mensaje> {
        self.mensajes
            .iter()
            .filter(|m| m.proyecto.as_deref() == proyecto)
            .collect()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_mensaje_con_sus_tres_codigos_se_reconoce_y_no_pierde_campos() {
        let original = r#"{"id":"1757939357123","cuando":1757939357123,"texto":"hola","numero":47,"uid":"RVK5YHKCX7","aparato":"K7Q2","recibidoDe":"Max phone · K7Q2","vieneDe":{"x":1}}"#;
        let m: Mensaje = serde_json::from_str(original).unwrap();
        assert_eq!(m.codigo_chat().as_deref(), Some("47·K7Q2"));
        assert_eq!(m.codigo_unico(), "RVK5YHKCX7");
        let mut otro = m.clone();
        assert!(m.mismo_que(&otro));
        otro.cuando += 1;
        assert!(!m.mismo_que(&otro), "otra fecha: no es el mismo");
        let vuelta = serde_json::to_value(&m).unwrap();
        for campo in ["recibidoDe", "vieneDe", "numero", "uid", "aparato"] {
            assert!(vuelta.get(campo).is_some(), "se perdio {campo}");
        }
    }

    fn cuaderno_de_prueba() -> &'static str {
        concat!(
            r#"{"id":"m1","cuando":1725500000000,"clase":"NOTA","texto":"comprar cemento"}"#,
            "\n",
            r#"{"id":"m2","cuando":1725500001000,"clase":"VOZ","nombre":"voz-1.m4a","duracionMs":4200,"transcripcion":"llamar al aparejador"}"#,
            "\n",
            r#"{"id":"m3","cuando":1725500002000,"clase":"IMAGEN","nombre":"fachada.jpg","bytes":204800,"proyecto":"pr-1","emoji":"P","fijado":true}"#,
            "\n",
            r#"{"id":"m4","cuando":1725500003000,"clase":"COSA_NUEVA","texto":"de una version futura"}"#,
            "\n"
        )
    }

    #[test]
    fn se_leen_los_mensajes_con_sus_campos() {
        let c = Cuaderno::leer(cuaderno_de_prueba());
        assert_eq!(c.mensajes.len(), 4);
        assert_eq!(c.lineas_rotas, 0);
        assert_eq!(c.mensajes[0].clase, Some(Clase::Nota));
        assert_eq!(c.mensajes[0].texto, "comprar cemento");
        assert_eq!(c.mensajes[2].emoji.as_deref(), Some("P"));
        assert!(c.mensajes[2].fijado);
        assert_eq!(c.mensajes[2].bytes, 204_800);
    }

    #[test]
    fn una_linea_rota_no_se_lleva_el_cuaderno() {
        // Es la razon de ser del formato: si un corte de luz a mitad de
        // escribir tumbara la lectura entera, el usuario perderia todo el
        // cuaderno en vez de un mensaje.
        let texto = concat!(
            r#"{"id":"a","cuando":1,"clase":"NOTA","texto":"antes"}"#,
            "\n",
            r#"{"id":"b","cuando":2,"clase":"NOT"#,
            "\n",
            r#"{"id":"c","cuando":3,"clase":"NOTA","texto":"despues"}"#,
            "\n"
        );
        let c = Cuaderno::leer(texto);
        assert_eq!(c.mensajes.len(), 2, "se perdieron los buenos");
        assert_eq!(c.lineas_rotas, 1);
        assert_eq!(c.mensajes[1].texto, "despues");
    }

    #[test]
    fn las_lineas_rotas_se_cuentan_y_no_se_callan() {
        // Si un cuaderno de mil ensena novecientos, el usuario tiene que
        // enterarse de que faltan cien y no creer que nunca existieron.
        let c = Cuaderno::leer("{roto\n{tambien roto\n");
        assert!(c.mensajes.is_empty());
        assert_eq!(c.lineas_rotas, 2);
    }

    #[test]
    fn una_clase_que_no_conocemos_se_lee_igual() {
        // El movil anade clases con el tiempo. Rechazar la linea entera por
        // una palabra desconocida perderia su texto, que si se entiende.
        let c = Cuaderno::leer(cuaderno_de_prueba());
        assert_eq!(
            c.mensajes[3].clase,
            Some(Clase::Otra("COSA_NUEVA".into())),
            "una clase futura tiene que llegar con su nombre"
        );
        assert_eq!(c.mensajes[3].texto, "de una version futura");
    }

    #[test]
    fn de_una_nota_de_voz_se_ensena_lo_que_dijo() {
        // El texto de un mensaje de voz suele venir vacio, y ensenar
        // «voz-1.m4a» no le dice nada a nadie.
        let c = Cuaderno::leer(cuaderno_de_prueba());
        assert_eq!(c.mensajes[1].resumen(), "llamar al aparejador");
        // Y sin transcripcion, al menos el nombre.
        let sin = Cuaderno::leer(r#"{"id":"z","clase":"VOZ","nombre":"voz-9.m4a"}"#);
        assert_eq!(sin.mensajes[0].resumen(), "voz-9.m4a");
        // Un mensaje sin nada no inventa texto.
        let vacio = Cuaderno::leer(r#"{"id":"z"}"#);
        assert_eq!(vacio.mensajes[0].resumen(), "");
    }

    #[test]
    fn cada_conversacion_tiene_lo_suyo() {
        let c = Cuaderno::leer(cuaderno_de_prueba());
        // La general son los que no llevan proyecto, y `None` no es lo mismo
        // que una cadena vacia: los mensajes de antes de que existieran los
        // proyectos siguen siendo de la general.
        assert_eq!(c.de_conversacion(None).len(), 3);
        assert_eq!(c.de_conversacion(Some("pr-1")).len(), 1);
        assert!(c.de_conversacion(Some("no-existe")).is_empty());
    }

    #[test]
    fn un_cuaderno_vacio_es_un_cuaderno() {
        // Caso negativo: cero mensajes no es un error, es un cuaderno recien
        // estrenado.
        let c = Cuaderno::leer("");
        assert!(c.mensajes.is_empty());
        assert_eq!(c.lineas_rotas, 0);
        // Y las lineas en blanco no cuentan como rotas.
        let d = Cuaderno::leer("\n\n   \n");
        assert_eq!(d.lineas_rotas, 0);
    }

    #[test]
    fn un_mensaje_al_que_le_faltan_campos_abre_igual() {
        // La misma regla que los ajustes: lo que falta toma su valor por
        // defecto en vez de tumbar la linea.
        let c = Cuaderno::leer(r#"{"id":"solo-id"}"#);
        assert_eq!(c.mensajes.len(), 1);
        assert_eq!(c.mensajes[0].id, "solo-id");
        assert_eq!(c.mensajes[0].cuando, 0);
        assert!(c.mensajes[0].clase.is_none());
        assert!(!c.mensajes[0].fijado);
    }

    fn carpeta(etiqueta: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("pixpin-cuaderno-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn escribir_una_nota_anade_una_linea_y_no_toca_las_de_antes() {
        let d = carpeta("anadir");
        let vieja = Mensaje::nota("lo de ayer", 1_725_500_000_000, 1, "K7Q2", "pr-1");
        anadir(&d, &vieja).unwrap();
        let antes = std::fs::read_to_string(d.join("guardados.jsonl")).unwrap();
        let nueva = Mensaje::nota("lo de hoy", 1_725_600_000_000, 2, "K7Q2", "pr-1");
        anadir(&d, &nueva).unwrap();
        let texto = std::fs::read_to_string(d.join("guardados.jsonl")).unwrap();
        assert!(texto.starts_with(&antes), "la linea de antes, intacta");
        assert_eq!(texto.lines().count(), 2);

        let c = Cuaderno::leer(&texto);
        assert_eq!(c.lineas_rotas, 0);
        assert_eq!(c.mensajes[1].texto, "lo de hoy");
        assert_eq!(c.mensajes[1].clase, Some(Clase::Nota));
        assert_eq!(c.mensajes[1].codigo_chat().as_deref(), Some("2·K7Q2"));
        assert_eq!(c.mensajes[1].codigo_unico().len(), crate::codigos::LARGO);
        // Dos notas seguidas no comparten codigo unico.
        assert_ne!(c.mensajes[0].codigo_unico(), c.mensajes[1].codigo_unico());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn el_numero_siguiente_es_el_mayor_mas_uno_y_va_por_conversacion() {
        let c = Cuaderno::leer(concat!(
            r#"{"id":"a","numero":7,"proyecto":"pr-1"}"#,
            "\n",
            r#"{"id":"b","numero":3,"proyecto":"pr-1"}"#,
            "\n",
            r#"{"id":"c","numero":41,"proyecto":"otro"}"#,
            "\n",
            r#"{"id":"d","numero":2}"#,
            "\n"
        ));
        assert_eq!(
            c.siguiente_numero(Some("pr-1")),
            8,
            "el mayor, no cuantos hay"
        );
        assert_eq!(c.siguiente_numero(Some("otro")), 42);
        assert_eq!(c.siguiente_numero(None), 3, "la general va aparte");
        // Una conversacion sin nada empieza por el uno.
        assert_eq!(c.siguiente_numero(Some("nueva")), 1);
    }

    #[test]
    fn una_linea_rota_al_final_no_se_lleva_las_buenas() {
        let d = carpeta("rota");
        anadir(&d, &Mensaje::nota("buena", 1, 1, "K7Q2", "pr-1")).unwrap();
        // Como si se hubiera cortado la luz a mitad de escribir la segunda.
        std::fs::write(
            d.join("guardados.jsonl"),
            format!(
                "{}{}",
                std::fs::read_to_string(d.join("guardados.jsonl")).unwrap(),
                r#"{"id":"a","cuando":"#
            ),
        )
        .unwrap();
        let c = Cuaderno::leer_de(&d).unwrap();
        assert_eq!(c.mensajes.len(), 1, "la buena se lee");
        assert_eq!(c.lineas_rotas, 1, "y la rota se cuenta, no se calla");
        let _ = std::fs::remove_dir_all(&d);
    }
}
