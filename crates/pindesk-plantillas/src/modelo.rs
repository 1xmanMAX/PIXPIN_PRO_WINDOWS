//! **El documento**: la pagina, el tema, la repeticion y el arbol de bloques.
//!
//! Todo en milimetros (el papel se piensa en milimetros; la pantalla y el PDF
//! lo pasan a sus unidades al pintar). Los textos de letra en puntos
//! tipograficos, como en cualquier procesador de textos.
//!
//! El arbol es de **bloques**: los contenedores (fila, columna, pila,
//! rejilla, seccion) reparten su caja entre sus hijos con un tamaño de tipo
//! flex ([`Tam`]: fijo en mm, fraccion del sitio que queda, o lo que pida su
//! contenido); las hojas pintan algo (un encabezado, renglones, una tabla,
//! un calendario…). El formato en JSON esta en `docs/plantillas.md`.

use serde::{Deserialize, Serialize};

use crate::locale::FormatoFecha;
use crate::tema::Tema;

/// La version del formato `.pdplantilla`. Sube cuando un cambio no se puede
/// leer con el codigo de antes (ver `formato::migrar`).
pub const VERSION_FORMATO: u32 = 1;

/// Un milimetro en puntos tipograficos (y de PDF): 72 / 25,4.
pub const PT_POR_MM: f32 = 72.0 / 25.4;

/// **Una plantilla.**
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Documento {
    /// La version del formato con que se escribio.
    #[serde(default = "version_actual")]
    pub version: u32,
    #[serde(default)]
    pub nombre: String,
    #[serde(default)]
    pub pagina: Pagina,
    #[serde(default)]
    pub tema: Tema,
    /// El idioma de las fechas (`es-ES`, `en-US`…).
    #[serde(default = "idioma_por_defecto")]
    pub idioma: String,
    /// Una pagina por dia, semana o mes; sin ella, una sola pagina.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeticion: Option<Repeticion>,
    pub raiz: Bloque,
}

fn version_actual() -> u32 {
    VERSION_FORMATO
}

fn idioma_por_defecto() -> String {
    "es-ES".into()
}

impl Documento {
    /// Una plantilla vacia: una columna en un A4.
    pub fn nuevo(nombre: &str) -> Documento {
        Documento {
            version: VERSION_FORMATO,
            nombre: nombre.into(),
            pagina: Pagina::default(),
            tema: Tema::default(),
            idioma: idioma_por_defecto(),
            repeticion: None,
            raiz: Bloque::nuevo(Tipo::Columna {
                hijos: Vec::new(),
                hueco: HUECO,
            }),
        }
    }

    /// El bloque con ese id.
    pub fn buscar(&self, id: u64) -> Option<&Bloque> {
        self.raiz.buscar(id)
    }

    pub fn buscar_mut(&mut self, id: u64) -> Option<&mut Bloque> {
        self.raiz.buscar_mut(id)
    }

    /// El id mas alto que hay (para dar los siguientes).
    pub fn id_maximo(&self) -> u64 {
        let mut m = 0;
        self.raiz.recorrer(&mut |b, _| m = m.max(b.id));
        m
    }

    /// Da id a los bloques que no lo tienen (0) o lo repiten.
    pub fn numerar(&mut self) {
        let mut vistos = std::collections::HashSet::new();
        let mut siguiente = self.id_maximo() + 1;
        fn uno(b: &mut Bloque, vistos: &mut std::collections::HashSet<u64>, siguiente: &mut u64) {
            if b.id == 0 || !vistos.insert(b.id) {
                b.id = *siguiente;
                *siguiente += 1;
                vistos.insert(b.id);
            }
            if let Some(h) = b.tipo.hijos_mut() {
                for c in h {
                    uno(c, vistos, siguiente);
                }
            }
        }
        uno(&mut self.raiz, &mut vistos, &mut siguiente);
    }

    /// Cuantos bloques hay (la raiz incluida).
    pub fn cuantos(&self) -> usize {
        let mut n = 0;
        self.raiz.recorrer(&mut |_, _| n += 1);
        n
    }
}

/// El hueco de fabrica entre los hijos de un contenedor (mm).
pub const HUECO: f32 = 4.0;

// ---------------------------------------------------------------------------
// La pagina
// ---------------------------------------------------------------------------

/// **El tamaño del papel.**
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tamano {
    #[default]
    A4,
    A5,
    /// Carta de EE. UU. (8,5 × 11 in).
    Carta,
    /// Una tableta en vertical (4:3, la de un iPad de 11").
    Tableta,
    /// Una pantalla 16:9, para usarla de fondo o en el lienzo.
    Pantalla,
    /// A medida, en mm.
    Medida {
        ancho: f32,
        alto: f32,
    },
}

impl Tamano {
    pub const DE_SERIE: [Tamano; 5] = [
        Tamano::A4,
        Tamano::A5,
        Tamano::Carta,
        Tamano::Tableta,
        Tamano::Pantalla,
    ];

    /// Ancho y alto en vertical (mm).
    pub fn mm(self) -> (f32, f32) {
        match self {
            Tamano::A4 => (210.0, 297.0),
            Tamano::A5 => (148.0, 210.0),
            Tamano::Carta => (215.9, 279.4),
            Tamano::Tableta => (178.0, 237.0),
            Tamano::Pantalla => (180.0, 320.0),
            Tamano::Medida { ancho, alto } => (ancho.clamp(40.0, 1200.0), alto.clamp(40.0, 1200.0)),
        }
    }

    /// La clave del texto de la interfaz (`plt-papel-<clave>`).
    pub fn clave(self) -> &'static str {
        match self {
            Tamano::A4 => "a4",
            Tamano::A5 => "a5",
            Tamano::Carta => "carta",
            Tamano::Tableta => "tableta",
            Tamano::Pantalla => "pantalla",
            Tamano::Medida { .. } => "medida",
        }
    }
}

/// Los margenes (mm).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Margenes {
    pub arriba: f32,
    pub abajo: f32,
    pub izquierda: f32,
    pub derecha: f32,
}

impl Margenes {
    pub const fn iguales(m: f32) -> Margenes {
        Margenes {
            arriba: m,
            abajo: m,
            izquierda: m,
            derecha: m,
        }
    }
}

impl Default for Margenes {
    fn default() -> Self {
        Margenes::iguales(14.0)
    }
}

/// **La pagina**: papel, orientacion y margenes.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Pagina {
    #[serde(default)]
    pub tamano: Tamano,
    /// Apaisada.
    #[serde(default)]
    pub horizontal: bool,
    #[serde(default)]
    pub margenes: Margenes,
}

impl Pagina {
    /// Ancho y alto con la orientacion (mm).
    pub fn medidas(&self) -> (f32, f32) {
        let (a, b) = self.tamano.mm();
        if self.horizontal { (b, a) } else { (a, b) }
    }
}

// ---------------------------------------------------------------------------
// La repeticion
// ---------------------------------------------------------------------------

/// Cada cuanto sale una pagina nueva.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cada {
    Dia,
    Semana,
    Mes,
}

/// **Una pagina por dia, semana o mes** entre dos fechas (`2027-01-01`),
/// con los campos de fecha de cada una ya puestos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Repeticion {
    pub cada: Cada,
    pub desde: String,
    pub hasta: String,
    /// Saltar sabados y domingos (solo con `dia`).
    #[serde(default)]
    pub sin_fines_de_semana: bool,
}

// ---------------------------------------------------------------------------
// Los bloques
// ---------------------------------------------------------------------------

/// **El tamaño de un bloque** en el eje de su contenedor (lo alto en una
/// columna, lo ancho en una fila). En JSON es un texto: `"auto"`, `"2fr"`
/// o `"30mm"`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Tam {
    /// Lo que pide su contenido; si no pide nada (un area de notas), se
    /// estira como `1fr`.
    #[default]
    Auto,
    /// Milimetros.
    Fijo(f32),
    /// Una parte del sitio que queda.
    Fraccion(f32),
}

impl Tam {
    pub fn a_texto(self) -> String {
        match self {
            Tam::Auto => "auto".into(),
            Tam::Fijo(mm) => format!("{}mm", redondo(mm)),
            Tam::Fraccion(f) => format!("{}fr", redondo(f)),
        }
    }

    /// Lee `auto`, `2fr`, `30mm`, `30` (mm) o `0.5fr`. `None` si no se
    /// entiende.
    pub fn leer(s: &str) -> Option<Tam> {
        let s = s.trim().to_ascii_lowercase().replace(',', ".");
        if s.is_empty() || s == "auto" {
            return Some(Tam::Auto);
        }
        let numero = |t: &str| t.trim().parse::<f32>().ok().filter(|x| x.is_finite());
        if let Some(n) = s.strip_suffix("fr") {
            return numero(n).map(|f| Tam::Fraccion(f.clamp(0.05, 50.0)));
        }
        if let Some(n) = s.strip_suffix("mm") {
            return numero(n).map(|f| Tam::Fijo(f.clamp(1.0, 2000.0)));
        }
        if let Some(n) = s.strip_suffix("cm") {
            return numero(n).map(|f| Tam::Fijo((f * 10.0).clamp(1.0, 2000.0)));
        }
        if let Some(n) = s.strip_suffix('%') {
            // Un porcentaje se lee como fraccion (50 % y 50 % son mitad y mitad).
            return numero(n).map(|f| Tam::Fraccion((f / 100.0).clamp(0.05, 50.0)));
        }
        numero(&s).map(|f| Tam::Fijo(f.clamp(1.0, 2000.0)))
    }
}

fn redondo(x: f32) -> f32 {
    (x * 100.0).round() / 100.0
}

impl Serialize for Tam {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.a_texto())
    }
}

impl<'de> Deserialize<'de> for Tam {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Tam, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        Ok(match v {
            serde_json::Value::String(s) => Tam::leer(&s).unwrap_or_default(),
            serde_json::Value::Number(n) => {
                Tam::Fijo(n.as_f64().unwrap_or(10.0).clamp(1.0, 2000.0) as f32)
            }
            _ => Tam::Auto,
        })
    }
}

/// Alineacion de un texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alineacion {
    #[default]
    Izquierda,
    Centro,
    Derecha,
}

/// Lo que llevan las lineas de un area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pauta {
    /// Renglones.
    #[default]
    Rayado,
    /// Una rejilla de puntos.
    Puntos,
    /// Cuadricula.
    Cuadricula,
    /// Nada.
    Blanco,
}

impl Pauta {
    pub const TODAS: [Pauta; 4] = [
        Pauta::Rayado,
        Pauta::Puntos,
        Pauta::Cuadricula,
        Pauta::Blanco,
    ];
    pub fn clave(self) -> &'static str {
        match self {
            Pauta::Rayado => "rayado",
            Pauta::Puntos => "puntos",
            Pauta::Cuadricula => "cuadricula",
            Pauta::Blanco => "blanco",
        }
    }
}

/// Un campo que se llena solo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueCampo {
    #[default]
    Fecha,
    /// El dia del mes (6).
    Dia,
    /// El dia de la semana (viernes).
    DiaSemana,
    /// La semana ISO (45).
    Semana,
    Mes,
    Ano,
    /// El numero de pagina.
    Pagina,
}

impl QueCampo {
    pub const TODOS: [QueCampo; 7] = [
        QueCampo::Fecha,
        QueCampo::Dia,
        QueCampo::DiaSemana,
        QueCampo::Semana,
        QueCampo::Mes,
        QueCampo::Ano,
        QueCampo::Pagina,
    ];
    pub fn clave(self) -> &'static str {
        match self {
            QueCampo::Fecha => "fecha",
            QueCampo::Dia => "dia",
            QueCampo::DiaSemana => "dia_semana",
            QueCampo::Semana => "semana",
            QueCampo::Mes => "mes",
            QueCampo::Ano => "ano",
            QueCampo::Pagina => "pagina",
        }
    }
}

/// El estilo de un divisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trazo {
    #[default]
    Continuo,
    Discontinuo,
    Punteado,
    /// Una raya gruesa del color de acento.
    Acento,
}

impl Trazo {
    pub const TODOS: [Trazo; 4] = [
        Trazo::Continuo,
        Trazo::Discontinuo,
        Trazo::Punteado,
        Trazo::Acento,
    ];
    pub fn clave(self) -> &'static str {
        match self {
            Trazo::Continuo => "continuo",
            Trazo::Discontinuo => "discontinuo",
            Trazo::Punteado => "punteado",
            Trazo::Acento => "acento",
        }
    }
}

/// **Lo que es un bloque.** En JSON, `{"tipo": "fila", ...}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Tipo {
    // --- Contenedores ---
    /// Los hijos uno al lado del otro.
    Fila {
        #[serde(default)]
        hijos: Vec<Bloque>,
        #[serde(default = "hueco")]
        hueco: f32,
    },
    /// Uno debajo del otro.
    Columna {
        #[serde(default)]
        hijos: Vec<Bloque>,
        #[serde(default = "hueco")]
        hueco: f32,
    },
    /// Uno encima del otro, en la misma caja (unos renglones y un rotulo
    /// encima, por ejemplo).
    Pila {
        #[serde(default)]
        hijos: Vec<Bloque>,
    },
    /// En una rejilla de `columnas` columnas, por filas.
    Rejilla {
        #[serde(default)]
        hijos: Vec<Bloque>,
        #[serde(default = "dos")]
        columnas: u32,
        #[serde(default = "hueco")]
        hueco: f32,
    },
    /// Una seccion con su rotulo y su raya de acento, y lo de dentro debajo.
    Seccion {
        #[serde(default)]
        titulo: String,
        #[serde(default)]
        hijos: Vec<Bloque>,
        /// Con un recuadro alrededor.
        #[serde(default)]
        recuadro: bool,
    },

    // --- Texto ---
    Encabezado {
        #[serde(default)]
        texto: String,
        /// 1 (titulo de la pagina), 2 o 3.
        #[serde(default = "uno")]
        nivel: u32,
        #[serde(default)]
        alineacion: Alineacion,
    },
    /// Un parrafo. Puede llevar campos: `{fecha}`, `{dia_semana}`… (ver
    /// `campos`).
    Texto {
        #[serde(default)]
        texto: String,
        /// El tamaño de la letra (pt).
        #[serde(default = "once")]
        puntos: f32,
        #[serde(default)]
        alineacion: Alineacion,
    },
    /// Un area de notas: rotulo y renglones.
    Notas {
        #[serde(default)]
        titulo: String,
        #[serde(default)]
        pauta: Pauta,
    },
    /// Solo la pauta: renglones, puntos, cuadricula o nada.
    Lineas {
        #[serde(default)]
        pauta: Pauta,
        /// Lo que va de una linea a otra (mm).
        #[serde(default = "paso")]
        paso: f32,
    },

    // --- Listas ---
    Vinetas {
        #[serde(default)]
        items: Vec<String>,
        /// Renglones vacios despues de los escritos.
        #[serde(default)]
        vacias: u32,
    },
    Casillas {
        #[serde(default)]
        items: Vec<String>,
        #[serde(default)]
        vacias: u32,
    },
    Numerada {
        #[serde(default)]
        items: Vec<String>,
        #[serde(default)]
        vacias: u32,
    },
    Tabla {
        #[serde(default = "tres")]
        filas: u32,
        #[serde(default = "tres")]
        columnas: u32,
        /// Lo que va en la primera fila.
        #[serde(default)]
        cabeceras: Vec<String>,
    },

    // --- Sueltos ---
    Divisor {
        #[serde(default)]
        trazo: Trazo,
    },
    Espacio {},
    Imagen {
        #[serde(default)]
        etiqueta: String,
    },

    // --- Fechas ---
    /// Un campo que se llena solo en cada pagina.
    Campo {
        #[serde(default)]
        campo: QueCampo,
        #[serde(default)]
        formato: FormatoFecha,
        /// Lo que va delante («Semana ») y detras.
        #[serde(default)]
        antes: String,
        #[serde(default)]
        despues: String,
        /// El tamaño de la letra (pt).
        #[serde(default = "doce")]
        puntos: f32,
        #[serde(default)]
        alineacion: Alineacion,
        #[serde(default)]
        negrita: bool,
    },
    /// El mes de la pagina (o uno fijo) en rejilla.
    CalendarioMes {
        /// `2027-01` para uno fijo; vacio, el de la pagina.
        #[serde(default)]
        mes: String,
        /// Pequeño, sin sitio para escribir en los dias.
        #[serde(default)]
        mini: bool,
    },
    /// La semana de la pagina, un dia por columna con renglones.
    CalendarioSemana {
        /// Con sabado y domingo.
        #[serde(default = "si")]
        fin_de_semana: bool,
    },
    /// Las horas del dia (o de la semana, con `dias` > 1).
    Horario {
        #[serde(default = "siete")]
        desde: u32,
        #[serde(default = "veintidos")]
        hasta: u32,
        /// Minutos de cada tramo (15, 30, 60).
        #[serde(default = "sesenta")]
        paso: u32,
        /// 1: el dia de la pagina; 5 o 7: la semana.
        #[serde(default = "uno")]
        dias: u32,
    },

    // --- Especiales ---
    /// Hábitos por dia: una fila por habito y una casilla por dia.
    Habitos {
        #[serde(default)]
        habitos: Vec<String>,
        /// 0: los del mes de la pagina; 7: la semana.
        #[serde(default)]
        dias: u32,
    },
    /// Como fue el dia: cinco caras.
    Animo {
        #[serde(default)]
        titulo: String,
    },
    /// Las prioridades: una caja por nivel con sus renglones.
    Prioridades {
        #[serde(default)]
        niveles: Vec<String>,
    },
    /// Notas Cornell: claves a la izquierda, notas y el resumen abajo.
    Cornell {
        #[serde(default)]
        titulo: String,
        /// Los rotulos de sus tres zonas (claves, notas, resumen), en el
        /// idioma de quien la puso.
        #[serde(default)]
        etiquetas: Vec<String>,
    },
    /// El acta de una reunion: asistentes, orden del dia, acuerdos y tareas.
    Reunion {
        #[serde(default)]
        titulo: String,
        /// Los rotulos (fecha, asistentes, orden del dia, acuerdos, tareas).
        #[serde(default)]
        etiquetas: Vec<String>,
    },
}

fn hueco() -> f32 {
    HUECO
}
fn dos() -> u32 {
    2
}
fn tres() -> u32 {
    3
}
fn uno() -> u32 {
    1
}
fn siete() -> u32 {
    7
}
fn veintidos() -> u32 {
    22
}
fn sesenta() -> u32 {
    60
}
fn once() -> f32 {
    11.0
}
fn doce() -> f32 {
    12.0
}
fn paso() -> f32 {
    8.0
}
fn si() -> bool {
    true
}

impl Tipo {
    /// La clave del tipo, la misma del JSON (`fila`, `calendario_mes`…).
    pub fn clave(&self) -> &'static str {
        match self {
            Tipo::Fila { .. } => "fila",
            Tipo::Columna { .. } => "columna",
            Tipo::Pila { .. } => "pila",
            Tipo::Rejilla { .. } => "rejilla",
            Tipo::Seccion { .. } => "seccion",
            Tipo::Encabezado { .. } => "encabezado",
            Tipo::Texto { .. } => "texto",
            Tipo::Notas { .. } => "notas",
            Tipo::Lineas { .. } => "lineas",
            Tipo::Vinetas { .. } => "vinetas",
            Tipo::Casillas { .. } => "casillas",
            Tipo::Numerada { .. } => "numerada",
            Tipo::Tabla { .. } => "tabla",
            Tipo::Divisor { .. } => "divisor",
            Tipo::Espacio {} => "espacio",
            Tipo::Imagen { .. } => "imagen",
            Tipo::Campo { .. } => "campo",
            Tipo::CalendarioMes { .. } => "calendario_mes",
            Tipo::CalendarioSemana { .. } => "calendario_semana",
            Tipo::Horario { .. } => "horario",
            Tipo::Habitos { .. } => "habitos",
            Tipo::Animo { .. } => "animo",
            Tipo::Prioridades { .. } => "prioridades",
            Tipo::Cornell { .. } => "cornell",
            Tipo::Reunion { .. } => "reunion",
        }
    }

    /// Los hijos, si es un contenedor.
    pub fn hijos(&self) -> Option<&Vec<Bloque>> {
        match self {
            Tipo::Fila { hijos, .. }
            | Tipo::Columna { hijos, .. }
            | Tipo::Pila { hijos }
            | Tipo::Rejilla { hijos, .. }
            | Tipo::Seccion { hijos, .. } => Some(hijos),
            _ => None,
        }
    }

    pub fn hijos_mut(&mut self) -> Option<&mut Vec<Bloque>> {
        match self {
            Tipo::Fila { hijos, .. }
            | Tipo::Columna { hijos, .. }
            | Tipo::Pila { hijos }
            | Tipo::Rejilla { hijos, .. }
            | Tipo::Seccion { hijos, .. } => Some(hijos),
            _ => None,
        }
    }

    pub fn es_contenedor(&self) -> bool {
        self.hijos().is_some()
    }

    /// El eje en que reparte a sus hijos (para soltar entre ellos).
    pub fn eje(&self) -> Option<Eje> {
        match self {
            Tipo::Fila { .. } => Some(Eje::Horizontal),
            Tipo::Columna { .. } | Tipo::Seccion { .. } => Some(Eje::Vertical),
            Tipo::Rejilla { .. } => Some(Eje::Horizontal),
            Tipo::Pila { .. } => Some(Eje::Vertical),
            _ => None,
        }
    }
}

/// Un eje.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eje {
    Horizontal,
    Vertical,
}

/// **Un bloque**: lo que es, su tamaño en su contenedor y su id (estable:
/// la seleccion y el deshacer lo usan).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bloque {
    #[serde(default)]
    pub id: u64,
    #[serde(default, skip_serializing_if = "es_auto")]
    pub tam: Tam,
    /// Un color propio para su acento (`#rrggbb`), si no el del tema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acento: Option<crate::tema::Color>,
    #[serde(flatten)]
    pub tipo: Tipo,
}

fn es_auto(t: &Tam) -> bool {
    *t == Tam::Auto
}

impl Bloque {
    pub fn nuevo(tipo: Tipo) -> Bloque {
        Bloque {
            id: 0,
            tam: Tam::Auto,
            acento: None,
            tipo,
        }
    }

    pub fn con_tam(mut self, tam: Tam) -> Bloque {
        self.tam = tam;
        self
    }

    pub fn buscar(&self, id: u64) -> Option<&Bloque> {
        if self.id == id {
            return Some(self);
        }
        self.tipo.hijos()?.iter().find_map(|h| h.buscar(id))
    }

    pub fn buscar_mut(&mut self, id: u64) -> Option<&mut Bloque> {
        if self.id == id {
            return Some(self);
        }
        self.tipo
            .hijos_mut()?
            .iter_mut()
            .find_map(|h| h.buscar_mut(id))
    }

    /// Recorre el arbol en profundidad con la profundidad de cada uno.
    pub fn recorrer(&self, f: &mut dyn FnMut(&Bloque, usize)) {
        fn uno(b: &Bloque, d: usize, f: &mut dyn FnMut(&Bloque, usize)) {
            f(b, d);
            if let Some(h) = b.tipo.hijos() {
                for c in h {
                    uno(c, d + 1, f);
                }
            }
        }
        uno(self, 0, f);
    }

    /// El padre de `id` y su posicion entre los hermanos.
    pub fn padre_de(&self, id: u64) -> Option<(u64, usize)> {
        let hijos = self.tipo.hijos()?;
        if let Some(i) = hijos.iter().position(|h| h.id == id) {
            return Some((self.id, i));
        }
        hijos.iter().find_map(|h| h.padre_de(id))
    }
}

/// Atajos para hacer bloques en el codigo (galeria, IA sin modelo, pruebas).
pub mod hacer {
    use super::*;

    pub fn fila(hijos: Vec<Bloque>) -> Bloque {
        Bloque::nuevo(Tipo::Fila {
            hijos,
            hueco: HUECO,
        })
    }
    pub fn columna(hijos: Vec<Bloque>) -> Bloque {
        Bloque::nuevo(Tipo::Columna {
            hijos,
            hueco: HUECO,
        })
    }
    pub fn seccion(titulo: &str, hijos: Vec<Bloque>) -> Bloque {
        Bloque::nuevo(Tipo::Seccion {
            titulo: titulo.into(),
            hijos,
            recuadro: false,
        })
    }
    pub fn rejilla(columnas: u32, hijos: Vec<Bloque>) -> Bloque {
        Bloque::nuevo(Tipo::Rejilla {
            hijos,
            columnas,
            hueco: HUECO,
        })
    }
    pub fn encabezado(texto: &str, nivel: u32) -> Bloque {
        Bloque::nuevo(Tipo::Encabezado {
            texto: texto.into(),
            nivel,
            alineacion: Alineacion::Izquierda,
        })
    }
    pub fn texto(texto: &str) -> Bloque {
        Bloque::nuevo(Tipo::Texto {
            texto: texto.into(),
            puntos: 11.0,
            alineacion: Alineacion::Izquierda,
        })
    }
    pub fn notas(titulo: &str) -> Bloque {
        Bloque::nuevo(Tipo::Notas {
            titulo: titulo.into(),
            pauta: Pauta::Rayado,
        })
    }
    pub fn lineas(pauta: Pauta) -> Bloque {
        Bloque::nuevo(Tipo::Lineas { pauta, paso: 8.0 })
    }
    pub fn casillas(items: &[&str], vacias: u32) -> Bloque {
        Bloque::nuevo(Tipo::Casillas {
            items: items.iter().map(|s| s.to_string()).collect(),
            vacias,
        })
    }
    pub fn vinetas(items: &[&str], vacias: u32) -> Bloque {
        Bloque::nuevo(Tipo::Vinetas {
            items: items.iter().map(|s| s.to_string()).collect(),
            vacias,
        })
    }
    pub fn campo(campo: QueCampo, formato: FormatoFecha, puntos: f32) -> Bloque {
        Bloque::nuevo(Tipo::Campo {
            campo,
            formato,
            antes: String::new(),
            despues: String::new(),
            puntos,
            alineacion: Alineacion::Izquierda,
            negrita: false,
        })
    }
    pub fn horario(desde: u32, hasta: u32, dias: u32) -> Bloque {
        Bloque::nuevo(Tipo::Horario {
            desde,
            hasta,
            paso: 60,
            dias,
        })
    }
    pub fn habitos(habitos: &[&str], dias: u32) -> Bloque {
        Bloque::nuevo(Tipo::Habitos {
            habitos: habitos.iter().map(|s| s.to_string()).collect(),
            dias,
        })
    }
    pub fn numerada_vacia(vacias: u32) -> Bloque {
        Bloque::nuevo(Tipo::Numerada {
            items: Vec::new(),
            vacias,
        })
    }
    pub fn divisor() -> Bloque {
        Bloque::nuevo(Tipo::Divisor {
            trazo: Trazo::Continuo,
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_tamano_se_lee_y_se_escribe_como_texto() {
        assert_eq!(Tam::leer("auto"), Some(Tam::Auto));
        assert_eq!(Tam::leer("2fr"), Some(Tam::Fraccion(2.0)));
        assert_eq!(Tam::leer(" 30 mm "), Some(Tam::Fijo(30.0)));
        assert_eq!(Tam::leer("3cm"), Some(Tam::Fijo(30.0)));
        assert_eq!(Tam::leer("25"), Some(Tam::Fijo(25.0)));
        assert_eq!(Tam::leer("50%"), Some(Tam::Fraccion(0.5)));
        assert_eq!(Tam::leer("1,5fr"), Some(Tam::Fraccion(1.5)));
        assert_eq!(Tam::leer("ancho"), None);
        assert_eq!(Tam::leer("-4mm"), Some(Tam::Fijo(1.0)), "nunca negativo");
        assert_eq!(Tam::Fraccion(1.5).a_texto(), "1.5fr");
        let b = hacer::texto("hola").con_tam(Tam::Fijo(12.0));
        let j = serde_json::to_value(&b).unwrap();
        assert_eq!(j["tam"], "12mm");
        assert_eq!(j["tipo"], "texto");
        let otra: Bloque = serde_json::from_value(j).unwrap();
        assert_eq!(otra, b);
    }

    #[test]
    fn numerar_da_ids_unicos_y_buscar_los_encuentra() {
        let mut d = Documento::nuevo("x");
        if let Some(h) = d.raiz.tipo.hijos_mut() {
            h.push(hacer::fila(vec![hacer::texto("a"), hacer::texto("b")]));
            h.push(hacer::texto("c"));
        }
        d.numerar();
        let mut ids = Vec::new();
        d.raiz.recorrer(&mut |b, _| ids.push(b.id));
        let mut u = ids.clone();
        u.sort();
        u.dedup();
        assert_eq!(u.len(), ids.len());
        assert!(ids.iter().all(|i| *i > 0));
        assert_eq!(d.cuantos(), 5);
        let c = ids[4];
        assert!(d.buscar(c).is_some());
        assert_eq!(d.raiz.padre_de(c), Some((d.raiz.id, 1)));
        assert!(d.buscar(9999).is_none());
    }

    #[test]
    fn la_pagina_gira() {
        let mut p = Pagina::default();
        assert_eq!(p.medidas(), (210.0, 297.0));
        p.horizontal = true;
        assert_eq!(p.medidas(), (297.0, 210.0));
        p.tamano = Tamano::Medida {
            ancho: 5.0,
            alto: 100.0,
        };
        assert_eq!(p.medidas(), (100.0, 40.0), "a medida, con topes");
    }
}
