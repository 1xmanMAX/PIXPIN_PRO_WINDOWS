//! **Buscar dentro de todos los chats** desde el buscador de la lista, como
//! en Telegram: debajo de los chats que casan por su nombre sale una seccion
//! «Mensajes» con los mensajes de CUALQUIER chat que llevan lo escrito, del
//! mas nuevo al mas viejo, con su trozo de texto y lo encontrado resaltado.
//!
//! Aqui solo la parte pura, la que se prueba sin ventana: el indice ya
//! aplanado (minusculas y sin tildes, con las mismas reglas que
//! `resaltado`), la busqueda con su tope, el trozo que se ensena de cada
//! mensaje, la espera entre tecla y tecla y como se reparte la seccion en
//! la lista. Quien lee los cuadernos del disco y pinta es la app.
//!
//! Decisiones que no son evidentes:
//!
//! - **Se aplana una vez, al indexar**, y no en cada busqueda: con cien mil
//!   mensajes, normalizar todo en cada tecla se notaria. Buscar es entonces
//!   un `contains` sobre texto ya aplanado, que es lo mas rapido que hay sin
//!   montar un indice invertido.
//! - **El aplanado es de un caracter a uno** (como `resaltado`): asi «casa»
//!   en lo aplanado es «casa» en el original, y los tramos que se resaltan
//!   salen de `resaltado::coincidencias` sobre el texto de verdad.
//! - **Los blancos cuentan como espacio**: buscar «hola mundo» encuentra
//!   «hola⏎mundo», que en una linea de resultado se lee igual.
//! - **Se cuenta todo, se devuelve hasta el tope**: la seccion dice cuantos
//!   hay de verdad y «Ver más» pide el siguiente tramo.
//!
//! **Desde el 27-oct-2026, como si todo estuviera dentro de un chat** (el
//! usuario, en el movil: «arriba filtros como Archivos, Fotos…»): encima de
//! lo encontrado van las pestanas por tipo ([`Pestana`]) con su cuenta; con
//! una puesta y nada escrito salen todos los de esa clase de todos los
//! chats. Pulsar un resultado lo abre en su visor si lo tiene
//! ([`al_pulsar`]); la flechita de la fila lleva al mensaje. Es lo de
//! `BuscarEnChats.kt` de PixPin Android (8815b492).

use crate::resaltado::{self, Tramo};
use pixpin_geom::Rect;

/// Un trozo de texto de un mensaje (su texto, su transcripcion, el nombre
/// de su fichero), tal cual y aplanado para buscar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pieza {
    pub texto: String,
    plano: String,
}

impl Pieza {
    pub fn nueva(texto: &str) -> Pieza {
        Pieza {
            texto: texto.to_string(),
            plano: aplanar(texto),
        }
    }
}

/// Lo que se sabe de un mensaje para buscarlo y llevar a el.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    /// El chat donde esta (el `id` de su ficha).
    pub chat: String,
    /// Por lo que se le encuentra al abrir el chat (su codigo unico).
    pub codigo: String,
    /// Milisegundos desde 1970: ordena del mas nuevo al mas viejo.
    pub cuando: i64,
    pub piezas: Vec<Pieza>,
    /// En que pestanas sale, ademas de «Todo» (ver [`Tipos`]).
    pub tipos: Tipos,
}

impl Entrada {
    /// Una entrada con los textos que se le den, sin los vacios ni los
    /// repetidos (la transcripcion suele estar tambien en el texto).
    pub fn nueva<'a>(
        chat: &str,
        codigo: &str,
        cuando: i64,
        textos: impl IntoIterator<Item = &'a str>,
    ) -> Entrada {
        let mut piezas: Vec<Pieza> = Vec::new();
        for t in textos {
            let t = t.trim();
            if t.is_empty() || piezas.iter().any(|p| p.texto == t) {
                continue;
            }
            piezas.push(Pieza::nueva(t));
        }
        Entrada {
            chat: chat.to_string(),
            codigo: codigo.to_string(),
            cuando,
            piezas,
            tipos: Tipos::default(),
        }
    }

    /// La misma entrada, saliendo en las pestanas `tipos`.
    pub fn con_tipos(mut self, tipos: Tipos) -> Entrada {
        self.tipos = tipos;
        self
    }

    /// La primera pieza donde esta lo buscado (ya aplanado).
    fn pieza_con(&self, aguja_plana: &str) -> Option<usize> {
        self.piezas
            .iter()
            .position(|p| p.plano.contains(aguja_plana))
    }
}

/// El texto en minusculas y sin tildes, con cada blanco hecho espacio: de un
/// caracter a uno, para que lo encontrado aqui caiga en el mismo sitio del
/// original.
pub fn aplanar(texto: &str) -> String {
    texto
        .chars()
        .map(|c| {
            if c.is_whitespace() {
                ' '
            } else {
                resaltado::normalizar(c)
            }
        })
        .collect()
}

/// Lo que se busca, ya aplanado; `None` si no hay nada que buscar (la aguja
/// en blanco no encuentra nada, como en `resaltado`).
pub fn aguja_plana(aguja: &str) -> Option<String> {
    let a = aguja.trim();
    (!a.is_empty()).then(|| aplanar(a))
}

/// Un mensaje encontrado: cual (por su posicion en lo buscado) y en que
/// pieza esta lo escrito. Sin pieza al mirar una pestana sin haber escrito
/// nada, si el mensaje no tiene texto que ensenar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Acierto {
    pub entrada: usize,
    pub pieza: Option<usize>,
}

/// Lo que dio una busqueda: los primeros `tope` y cuantos hay en total en
/// la pestana pedida, y cuantos casan en cada pestana (el numero de su
/// chapa, por [`Pestana::indice`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hallado {
    pub aciertos: Vec<Acierto>,
    pub total: usize,
    pub por_pestana: [usize; Pestana::TODAS.len()],
}

impl Hallado {
    /// Si quedan por ensenar («Ver más»).
    pub fn hay_mas(&self) -> bool {
        self.total > self.aciertos.len()
    }
}

/// **Busca** `aguja` en las entradas, que tienen que venir ya del mas nuevo
/// al mas viejo (es el orden en que se ensenan). Devuelve los `tope`
/// primeros y cuenta todos. Es [`buscar_en`] en «Todo».
pub fn buscar<'a>(
    entradas: impl IntoIterator<Item = &'a Entrada>,
    aguja: &str,
    tope: usize,
) -> Hallado {
    buscar_en(entradas, aguja, tope, Pestana::Todo)
}

/// **Busca solo lo de la pestana `pestana`.** Se filtra antes del tope y no
/// despues: filtrar los veinte primeros dejaba «Archivos» con dos aunque
/// hubiera cuarenta. Con una pestana puesta **no hace falta escribir**: sin
/// aguja salen todos los de esa clase, como en la ficha de un chat. Sin
/// aguja y en «Todo», nada: eso seria el archivo entero, y para eso ya
/// esta la lista de chats.
pub fn buscar_en<'a>(
    entradas: impl IntoIterator<Item = &'a Entrada>,
    aguja: &str,
    tope: usize,
    pestana: Pestana,
) -> Hallado {
    let plana = aguja_plana(aguja);
    if plana.is_none() && pestana == Pestana::Todo {
        return Hallado::default();
    }
    let mut hallado = Hallado::default();
    for (i, e) in entradas.into_iter().enumerate() {
        let pieza = match &plana {
            Some(p) => match e.pieza_con(p) {
                Some(k) => Some(k),
                None => continue,
            },
            None => (!e.piezas.is_empty()).then_some(0),
        };
        for p in Pestana::TODAS {
            if e.tipos.tiene(p) {
                hallado.por_pestana[p.indice()] += 1;
            }
        }
        if !e.tipos.tiene(pestana) {
            continue;
        }
        hallado.total += 1;
        if hallado.aciertos.len() < tope {
            hallado.aciertos.push(Acierto { entrada: i, pieza });
        }
    }
    hallado
}

/// Ordena las entradas como se buscan: lo mas nuevo primero y, a la par,
/// por chat y codigo, para que dos busquedas iguales den lo mismo.
pub fn ordenar(entradas: &mut [&Entrada]) {
    entradas.sort_by(|a, b| comparar(a, b));
}

/// El orden de `ordenar`, para quien ordena sus propios indices.
pub fn comparar(a: &Entrada, b: &Entrada) -> std::cmp::Ordering {
    b.cuando
        .cmp(&a.cuando)
        .then_with(|| a.chat.cmp(&b.chat))
        .then_with(|| a.codigo.cmp(&b.codigo))
}

/// El trozo de un mensaje que se ensena en su fila, en una linea, y donde
/// resaltar lo encontrado (bytes de `texto`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fragmento {
    pub texto: String,
    pub tramos: Vec<Tramo>,
}

/// Los puntos que dicen «aqui hay mas».
const PUNTOS: &str = "…";

/// **El trozo de `texto` que se ensena** con lo encontrado: en una linea,
/// empezando como mucho `antes` caracteres antes de la primera
/// coincidencia (por el principio de una palabra, con «…» delante si se
/// corta) y de `largo` caracteres como mucho (con «…» detras si se corta).
/// Sin coincidencia, el principio del texto.
pub fn fragmento(texto: &str, aguja: &str, antes: usize, largo: usize) -> Fragmento {
    let linea: String = texto
        .trim()
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    let tramos = resaltado::coincidencias(&linea, aguja);
    // Donde empieza el trozo, en bytes de `linea`.
    let mut desde = 0;
    if let Some(primero) = tramos.first() {
        let previas: Vec<(usize, char)> = linea[..primero.desde].char_indices().collect();
        if previas.len() > antes {
            let corte = previas.len() - antes;
            // Hacia delante hasta el principio de una palabra, sin pasar de
            // la coincidencia; si no hay espacio, por donde caiga.
            desde = previas[corte..]
                .iter()
                .position(|(_, c)| *c == ' ')
                .map(|k| previas[corte + k].0 + 1)
                .unwrap_or(previas[corte].0);
        }
    }
    let resto = &linea[desde..];
    let hasta = resto
        .char_indices()
        .nth(largo)
        .map_or(linea.len(), |(k, _)| desde + k);
    let delante = if desde > 0 { PUNTOS } else { "" };
    let detras = if hasta < linea.len() { PUNTOS } else { "" };
    let cuerpo = &linea[desde..hasta];
    let corrido = delante.len();
    let tramos = tramos
        .into_iter()
        .filter(|t| t.desde >= desde && t.desde < hasta)
        .map(|t| Tramo {
            desde: t.desde - desde + corrido,
            hasta: t.hasta.min(hasta) - desde + corrido,
        })
        .collect();
    Fragmento {
        texto: format!("{delante}{cuerpo}{detras}"),
        tramos,
    }
}

// --- Las pestanas por tipo (27-oct-2026) ---------------------------------

/// **Las pestanas de la busqueda**: las de la ficha de un chat con «Todo»
/// delante y sin las que no son una clase de cosa: «Proyectos» (ya son las
/// filas de chats), «Fijados» (es de cada chat, no de todos) y el buzon.
///
/// Que mensaje sale en cual lo decide la app con la misma regla que la ficha
/// (`cuaderno::indices_de_seccion`: un video no es foto ni archivo, una
/// pagina de PDF es archivo…), asi algo sale en «Archivos» aqui si y solo si
/// sale en la ficha de su chat.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Pestana {
    #[default]
    Todo,
    Fotos,
    Videos,
    Archivos,
    Enlaces,
    Voz,
    Dibujos,
}

impl Pestana {
    /// Todas, en el orden en que se ensenan.
    pub const TODAS: [Pestana; 7] = [
        Pestana::Todo,
        Pestana::Fotos,
        Pestana::Videos,
        Pestana::Archivos,
        Pestana::Enlaces,
        Pestana::Voz,
        Pestana::Dibujos,
    ];

    /// Su sitio en [`Pestana::TODAS`] (y en [`Hallado::por_pestana`]).
    pub fn indice(self) -> usize {
        self as usize
    }

    /// La clave de su nombre: la misma de la pestana de la ficha del chat.
    pub fn clave(self) -> &'static str {
        match self {
            Pestana::Todo => "info-todo",
            Pestana::Fotos => "info-fotos",
            Pestana::Videos => "info-videos",
            Pestana::Archivos => "info-archivos",
            Pestana::Enlaces => "info-enlaces",
            Pestana::Voz => "info-voz",
            Pestana::Dibujos => "info-dibujos",
        }
    }
}

/// **En que pestanas sale un mensaje**, sin contar «Todo» (ahi sale todo).
/// Lo del buzon no sale en ninguna, como en la ficha de su chat.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tipos(u8);

impl Tipos {
    pub fn tiene(self, p: Pestana) -> bool {
        p == Pestana::Todo || self.0 & (1 << p.indice()) != 0
    }
}

impl FromIterator<Pestana> for Tipos {
    fn from_iter<I: IntoIterator<Item = Pestana>>(i: I) -> Tipos {
        Tipos(
            i.into_iter()
                .filter(|p| *p != Pestana::Todo)
                .fold(0, |t, p| t | (1 << p.indice())),
        )
    }
}

// --- Que hace pulsar un resultado (27-oct-2026) ----------------------------

/// Lo que es un mensaje encontrado, para decidir que hace pulsarlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cosa {
    /// Una nota: con un enlace, se abre el enlace.
    Nota { enlace: bool },
    /// Una nota de voz: se escucha con la barra del reproductor, en el chat.
    Voz,
    /// Una foto o un archivo, y si su fichero esta en este equipo (la
    /// sincronizacion trae los mensajes antes que los ficheros).
    Fichero { esta: bool },
    /// Lo que se abre en su ventana desde la burbuja: un lienzo, una pagina,
    /// una mini-aplicacion, una carpeta, otro proyecto.
    ConVisor,
    /// Lo que no tiene a que apuntar (un lienzo sin su dibujo).
    SinNada,
}

/// Que hace pulsar un resultado.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AlPulsar {
    /// Abrirlo en su visor, como al tocarlo en su burbuja.
    Abrir,
    /// Llevar a su burbuja en el chat, resaltada.
    #[default]
    IrAlMensaje,
}

/// **Que hace pulsar un resultado** (el usuario, en el movil: «al tocar un
/// PDF, que lo abra directamente; o, si quiero, que me lleve a ese
/// mensaje»). Lo que tiene visor se abre en el; lo demas lleva a su sitio en
/// el chat: una nota sin enlace (no hay nada que abrir y se quiere leer con
/// lo de alrededor), una nota de voz, un fichero que aun no llego a este
/// equipo (en el chat se ve que falta) y lo que no apunta a nada. «Ir al
/// mensaje» (la flechita o el clic derecho) lleva siempre.
pub fn al_pulsar(cosa: Cosa) -> AlPulsar {
    let abre = match cosa {
        Cosa::Nota { enlace } => enlace,
        Cosa::Voz | Cosa::SinNada => false,
        Cosa::Fichero { esta } => esta,
        Cosa::ConVisor => true,
    };
    if abre {
        AlPulsar::Abrir
    } else {
        AlPulsar::IrAlMensaje
    }
}

// --- La banda de las pestanas ----------------------------------------------

/// Lo alta que es una pestana (una pildora) y su letra.
pub const PESTANA_ALTO: u32 = 28;
pub const PESTANA_TAM: f32 = 12.0;
/// El hueco entre dos pestanas, y entre dos lineas de ellas.
pub const PESTANA_HUECO: u32 = 6;
/// El margen de la banda: a los lados, arriba y abajo.
pub const PESTANAS_MARGEN: u32 = 8;
/// Lo que lleva la pildora a cada lado de su rotulo.
const PESTANA_RELLENO: u32 = 10;
/// Lo que se cuenta por letra del rotulo, a 12 pt: algo mas que la media
/// de Segoe UI, para que el rotulo quepa sin medirlo. Sin medir, la banda
/// se reparte igual al pintar y al pulsar, que no tienen el mismo pintor.
const PESTANA_LETRA: u32 = 7;

/// El rotulo de una pestana: su nombre y, si se sabe, cuantos hay.
pub fn rotulo_de_pestana(nombre: &str, cuenta: Option<usize>) -> String {
    match cuenta {
        Some(n) => format!("{nombre} {n}"),
        None => nombre.to_string(),
    }
}

/// Lo ancha que es la pestana de `rotulo`.
pub fn ancho_de_pestana(rotulo: &str, escala_por_cien: u32) -> u32 {
    let letras = rotulo.chars().count() as u32;
    (2 * PESTANA_RELLENO + letras * PESTANA_LETRA) * escala_por_cien / 100
}

/// **Reparte las pestanas** de anchos `anchos` en lineas de `ancho` px:
/// sus cajas, desde la esquina de arriba de la banda, y cuantas lineas
/// ocupan. Una que no cabe en lo que queda de linea baja a la siguiente;
/// una mas ancha que la linea se queda sola en ella, recortada.
pub fn repartir_pestanas(anchos: &[u32], ancho: u32, escala_por_cien: u32) -> (Vec<Rect>, u32) {
    let e = |v: u32| v * escala_por_cien / 100;
    let (margen, hueco, alto) = (e(PESTANAS_MARGEN), e(PESTANA_HUECO), e(PESTANA_ALTO));
    let util = ancho.saturating_sub(2 * margen).max(1);
    let mut cajas = Vec::with_capacity(anchos.len());
    let (mut x, mut linea) = (0u32, 0u32);
    for &w in anchos {
        let w = w.min(util);
        if x > 0 && x + w > util {
            x = 0;
            linea += 1;
        }
        cajas.push(Rect {
            x: (margen + x) as i32,
            y: (margen + linea * (alto + hueco)) as i32,
            ancho: w,
            alto,
        });
        x += w + hueco;
    }
    let lineas = if anchos.is_empty() { 0 } else { linea + 1 };
    (cajas, lineas)
}

/// Lo alta que es la banda con `lineas` lineas de pestanas.
pub fn alto_de_pestanas(lineas: u32, escala_por_cien: u32) -> u32 {
    if lineas == 0 {
        return 0;
    }
    let e = |v: u32| v * escala_por_cien / 100;
    2 * e(PESTANAS_MARGEN) + lineas * e(PESTANA_ALTO) + (lineas - 1) * e(PESTANA_HUECO)
}

/// La pestana bajo `(x, y)`, desde la esquina de la banda.
pub fn pestana_en(cajas: &[Rect], x: i32, y: i32) -> Option<usize> {
    cajas
        .iter()
        .position(|c| c.contiene(pixpin_geom::Punto { x, y }))
}

// --- La flechita de «ir al mensaje» ----------------------------------------

/// El lado de la flechita de la fila que lleva al mensaje.
pub const FLECHA: u32 = 26;

/// **La flechita de «ir al mensaje»** de una fila de resultado: a la
/// derecha, a la altura del trozo (la fecha va encima, en la del nombre).
pub fn flecha(fila: Rect, escala_por_cien: u32) -> Rect {
    let e = |v: u32| v * escala_por_cien / 100;
    let lado = e(FLECHA);
    Rect {
        x: fila.derecha() - e(crate::chat::MARGEN_DERECHO) as i32 - lado as i32,
        y: fila.y + e(crate::chat::RESUMEN_Y) as i32 + e(9) as i32 - lado as i32 / 2,
        ancho: lado,
        alto: lado,
    }
}

// --- Esperar a que se deje de escribir -----------------------------------

/// Lo que se espera desde la ultima tecla antes de buscar en todos los
/// chats: escribiendo seguido no se busca en cada letra.
pub const ESPERA_MS: u64 = 180;

/// **La espera entre tecla y tecla** (el «debounce»): cada cambio la
/// rearma, y toca buscar cuando pasa `ESPERA_MS` sin cambios.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Retardo {
    /// Cuando fue el ultimo cambio sin buscar, en ms de cualquier reloj.
    desde: Option<u64>,
}

impl Retardo {
    /// Lo escrito cambio a los `ahora` ms.
    pub fn cambio(&mut self, ahora: u64) {
        self.desde = Some(ahora);
    }

    /// Si toca buscar ya. Si toca, se desarma: una vez por cambio.
    pub fn toca(&mut self, ahora: u64) -> bool {
        match self.desde {
            Some(d) if ahora.saturating_sub(d) >= ESPERA_MS => {
                self.desde = None;
                true
            }
            _ => false,
        }
    }

    /// Cuanto falta para que toque, si hay algo esperando.
    pub fn ms_hasta(&self, ahora: u64) -> Option<u32> {
        self.desde
            .map(|d| ESPERA_MS.saturating_sub(ahora.saturating_sub(d)) as u32)
    }
}

// --- La seccion en la lista ----------------------------------------------

/// La cabecera «Mensajes» de la seccion, mas baja que una fila (como la de
/// Telegram).
pub const CABECERA_SECCION: u32 = 30;
/// La fila de «Ver más».
pub const VER_MAS: u32 = 40;
/// Cuantos mensajes se ensenan de primeras, y cuantos mas con cada «Ver más».
pub const DE_PRIMERAS: usize = 20;

/// Lo que hay en un punto de la seccion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elemento {
    /// La banda de las pestanas por tipo (cual, con [`pestana_en`]).
    Pestanas,
    Cabecera,
    /// El mensaje encontrado `n` (de los que se ensenan).
    Mensaje(usize),
    VerMas,
    /// «Nada con eso», debajo de las pestanas: se busco y no hay nada.
    Nada,
}

/// **La seccion «Mensajes»** debajo de los chats: las pestanas por tipo (si
/// se esta buscando), su cabecera, una fila de lista (`chat::FILA`) por
/// mensaje y, si quedan, «Ver más». Todo en pixeles desde donde empieza la
/// seccion.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Seccion {
    /// Cuantas lineas de pestanas lleva arriba (0, sin pestanas).
    pub lineas_de_pestanas: u32,
    pub mensajes: usize,
    pub ver_mas: bool,
    /// Se busco y no salio nada: debajo de las pestanas, «Nada con eso».
    pub nada: bool,
}

impl Seccion {
    /// Si hay seccion: sin pestanas ni mensajes no se pinta ni la cabecera.
    pub fn hay(&self) -> bool {
        self.lineas_de_pestanas > 0 || self.mensajes > 0
    }

    /// Si se dice «Nada con eso»: solo debajo de las pestanas.
    fn con_nada(&self) -> bool {
        self.nada && self.mensajes == 0 && self.lineas_de_pestanas > 0
    }

    /// Donde empieza y lo alto que es cada elemento.
    pub fn caja(&self, el: Elemento, escala_por_cien: u32) -> (i32, u32) {
        let e = |v: u32| v * escala_por_cien / 100;
        let fila = e(crate::chat::FILA);
        let pestanas = alto_de_pestanas(self.lineas_de_pestanas, escala_por_cien);
        let cab = if self.mensajes > 0 {
            e(CABECERA_SECCION)
        } else {
            0
        };
        let debajo = (pestanas + cab) as i32;
        match el {
            Elemento::Pestanas => (0, pestanas),
            Elemento::Cabecera => (pestanas as i32, cab),
            Elemento::Mensaje(n) => (debajo + n as i32 * fila as i32, fila),
            Elemento::VerMas => (debajo + self.mensajes as i32 * fila as i32, e(VER_MAS)),
            Elemento::Nada => (pestanas as i32, e(VER_MAS)),
        }
    }

    /// Lo alto que es entera.
    pub fn alto(&self, escala_por_cien: u32) -> u32 {
        if !self.hay() {
            return 0;
        }
        let ultimo = if self.mensajes == 0 {
            if self.con_nada() {
                Elemento::Nada
            } else {
                Elemento::Pestanas
            }
        } else if self.ver_mas {
            Elemento::VerMas
        } else {
            Elemento::Mensaje(self.mensajes - 1)
        };
        let (y, h) = self.caja(ultimo, escala_por_cien);
        y as u32 + h
    }

    /// Cuantas filas de lista ocupa, redondeando hacia arriba: es lo que se
    /// suma a los chats para saber hasta donde se puede bajar la lista.
    pub fn filas(&self, escala_por_cien: u32) -> usize {
        let fila = (crate::chat::FILA * escala_por_cien / 100).max(1);
        self.alto(escala_por_cien).div_ceil(fila) as usize
    }

    /// Que hay a `y` pixeles del principio de la seccion.
    pub fn elemento_en(&self, y: i32, escala_por_cien: u32) -> Option<Elemento> {
        if !self.hay() || y < 0 || y >= self.alto(escala_por_cien) as i32 {
            return None;
        }
        let dentro = |el: Elemento| {
            let (desde, alto) = self.caja(el, escala_por_cien);
            alto > 0 && y >= desde && y < desde + alto as i32
        };
        if dentro(Elemento::Pestanas) {
            return Some(Elemento::Pestanas);
        }
        if self.mensajes == 0 {
            return self.con_nada().then_some(Elemento::Nada);
        }
        if dentro(Elemento::Cabecera) {
            return Some(Elemento::Cabecera);
        }
        let (fila_y, fila_h) = self.caja(Elemento::Mensaje(0), escala_por_cien);
        let n = ((y - fila_y) / fila_h.max(1) as i32) as usize;
        Some(if n < self.mensajes {
            Elemento::Mensaje(n)
        } else {
            Elemento::VerMas
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn entrada(chat: &str, codigo: &str, cuando: i64, textos: &[&str]) -> Entrada {
        Entrada::nueva(chat, codigo, cuando, textos.iter().copied())
    }

    #[test]
    fn busca_sin_mirar_mayusculas_ni_tildes_y_en_cualquier_pieza() {
        let es = [
            entrada("a", "1", 30, &["La ARAÑA del tejado"]),
            entrada("b", "2", 20, &["", "nota de voz: cemento listo"]),
            entrada("c", "3", 10, &["", "", "Plano_Cimentación.pdf"]),
            entrada("c", "4", 5, &["nada que ver"]),
        ];
        let h = buscar(&es, "arana", 10);
        assert_eq!(
            h.aciertos,
            vec![Acierto {
                entrada: 0,
                pieza: Some(0)
            }]
        );
        let h = buscar(&es, "CEMENTO", 10);
        assert_eq!(
            h.aciertos,
            vec![Acierto {
                entrada: 1,
                pieza: Some(0)
            }]
        );
        let h = buscar(&es, "cimentacion", 10);
        assert_eq!(
            h.aciertos,
            vec![Acierto {
                entrada: 2,
                pieza: Some(0)
            }]
        );
        assert_eq!(buscar(&es, "   ", 10), Hallado::default());
    }

    #[test]
    fn los_vacios_y_repetidos_no_son_piezas() {
        let e = entrada("a", "1", 0, &["  ", "hola", "hola", "hola.png"]);
        let textos: Vec<&str> = e.piezas.iter().map(|p| p.texto.as_str()).collect();
        assert_eq!(textos, ["hola", "hola.png"]);
    }

    #[test]
    fn el_tope_corta_lo_que_se_devuelve_pero_no_la_cuenta() {
        let es: Vec<Entrada> = (0..50)
            .map(|i| entrada("a", &i.to_string(), 100 - i, &["pan y vino"]))
            .collect();
        let h = buscar(&es, "vino", 20);
        assert_eq!(h.aciertos.len(), 20);
        assert_eq!(h.total, 50);
        assert!(h.hay_mas());
        assert!(!buscar(&es, "vino", 50).hay_mas());
    }

    #[test]
    fn los_blancos_cuentan_como_espacio() {
        let es = [entrada("a", "1", 0, &["hola\nmundo"])];
        assert_eq!(buscar(&es, "hola mundo", 5).total, 1);
        let f = fragmento("hola\nmundo", "hola mundo", 10, 50);
        assert_eq!(f.texto, "hola mundo");
        assert_eq!(
            f.tramos,
            vec![Tramo {
                desde: 0,
                hasta: 10
            }]
        );
    }

    #[test]
    fn se_ordena_de_lo_mas_nuevo_a_lo_mas_viejo() {
        let a = entrada("a", "1", 10, &["x"]);
        let b = entrada("b", "2", 30, &["x"]);
        let c = entrada("a", "3", 20, &["x"]);
        let mut v = vec![&a, &b, &c];
        ordenar(&mut v);
        let codigos: Vec<&str> = v.iter().map(|e| e.codigo.as_str()).collect();
        assert_eq!(codigos, ["2", "3", "1"]);
    }

    #[test]
    fn el_fragmento_empieza_cerca_de_lo_encontrado_por_una_palabra() {
        let texto = "uno dos tres cuatro cinco seis siete ocho nueve diez once";
        let f = fragmento(texto, "nueve", 12, 100);
        assert_eq!(f.texto, "…siete ocho nueve diez once");
        let t = f.tramos[0];
        assert_eq!(&f.texto[t.desde..t.hasta], "nueve");
    }

    #[test]
    fn el_fragmento_corto_no_lleva_puntos_y_resalta_todas() {
        let f = fragmento("  Casa y casa  ", "casa", 20, 100);
        assert_eq!(f.texto, "Casa y casa");
        let recortes: Vec<&str> = f
            .tramos
            .iter()
            .map(|t| &f.texto[t.desde..t.hasta])
            .collect();
        assert_eq!(recortes, ["Casa", "casa"]);
    }

    #[test]
    fn el_fragmento_largo_se_corta_con_puntos_y_respeta_las_tildes() {
        let f = fragmento("Árbol ñandú camión árbol más texto aquí", "arbol", 5, 12);
        assert_eq!(f.texto, "Árbol ñandú …");
        let t = f.tramos[0];
        assert_eq!(&f.texto[t.desde..t.hasta], "Árbol");
        // La segunda coincidencia queda fuera del trozo: no se resalta.
        assert_eq!(f.tramos.len(), 1);
    }

    #[test]
    fn el_fragmento_sin_coincidencia_es_el_principio() {
        let f = fragmento("abc def", "zzz", 3, 3);
        assert_eq!(f.texto, "abc…");
        assert!(f.tramos.is_empty());
    }

    #[test]
    fn la_espera_busca_una_vez_cuando_se_deja_de_escribir() {
        let mut r = Retardo::default();
        assert!(!r.toca(0));
        assert_eq!(r.ms_hasta(0), None);
        r.cambio(1000);
        assert_eq!(r.ms_hasta(1000), Some(ESPERA_MS as u32));
        assert!(!r.toca(1100));
        r.cambio(1100);
        assert!(!r.toca(1100 + ESPERA_MS - 1));
        assert!(r.toca(1100 + ESPERA_MS));
        assert!(!r.toca(5000), "una vez por cambio");
    }

    #[test]
    fn la_seccion_reparte_cabecera_mensajes_y_ver_mas() {
        let s = Seccion {
            mensajes: 2,
            ver_mas: true,
            ..Default::default()
        };
        let fila = crate::chat::FILA as i32;
        let cab = CABECERA_SECCION as i32;
        assert_eq!(s.alto(100), (cab + 2 * fila) as u32 + VER_MAS);
        assert_eq!(s.elemento_en(0, 100), Some(Elemento::Cabecera));
        assert_eq!(s.elemento_en(cab, 100), Some(Elemento::Mensaje(0)));
        assert_eq!(s.elemento_en(cab + fila, 100), Some(Elemento::Mensaje(1)));
        assert_eq!(s.elemento_en(cab + 2 * fila, 100), Some(Elemento::VerMas));
        assert_eq!(s.elemento_en(s.alto(100) as i32, 100), None);
        assert_eq!(s.elemento_en(-1, 100), None);
        // 30 + 2 x 62 + 40 = 194 px: algo mas de tres filas, cuatro.
        assert_eq!(s.filas(100), 4);
        // A doble escala, todo el doble.
        assert_eq!(s.elemento_en(2 * cab, 200), Some(Elemento::Mensaje(0)));
        // Sin mensajes no hay seccion.
        let vacia = Seccion::default();
        assert_eq!((vacia.alto(100), vacia.filas(100)), (0, 0));
        assert_eq!(vacia.elemento_en(0, 100), None);
    }

    // --- 27-oct-2026: las pestanas, que hace pulsar y la flechita ---------

    fn con(e: Entrada, tipos: &[Pestana]) -> Entrada {
        e.con_tipos(tipos.iter().copied().collect())
    }

    fn de_todo() -> Vec<Entrada> {
        vec![
            con(
                entrada("a", "f", 60, &["fachada de la obra", "obra.jpg"]),
                &[Pestana::Fotos],
            ),
            con(
                entrada("a", "v", 50, &["visita obra.mp4"]),
                &[Pestana::Videos],
            ),
            con(
                entrada("casa", "p", 40, &["Plano obra.pdf"]),
                &[Pestana::Archivos],
            ),
            con(
                entrada("b", "e", 30, &["la obra en https://ejemplo.com"]),
                &[Pestana::Enlaces],
            ),
            con(
                entrada("b", "z", 20, &["llamar por la obra", "a.m4a"]),
                &[Pestana::Voz],
            ),
            con(
                entrada("c", "d", 10, &["Croquis obra"]),
                &[Pestana::Dibujos],
            ),
            entrada("c", "n", 5, &["repasar la obra"]),
        ]
    }

    #[test]
    fn los_tipos_dicen_en_que_pestanas_sale_y_todo_es_siempre() {
        let t: Tipos = [Pestana::Fotos, Pestana::Todo].into_iter().collect();
        assert!(t.tiene(Pestana::Todo));
        assert!(t.tiene(Pestana::Fotos));
        assert!(!t.tiene(Pestana::Videos));
        assert!(Tipos::default().tiene(Pestana::Todo));
        assert!(!Tipos::default().tiene(Pestana::Archivos));
        // Las pestanas, en su orden y con «Todo» delante.
        assert_eq!(Pestana::TODAS[0], Pestana::Todo);
        for (i, p) in Pestana::TODAS.iter().enumerate() {
            assert_eq!(p.indice(), i);
            assert!(p.clave().starts_with("info-"));
        }
    }

    #[test]
    fn la_pestana_filtra_antes_del_tope_y_cuenta_cada_una() {
        let es = de_todo();
        let todo = buscar_en(&es, "obra", 3, Pestana::Todo);
        assert_eq!(todo.total, 7);
        let codigos: Vec<&str> = todo
            .aciertos
            .iter()
            .map(|a| es[a.entrada].codigo.as_str())
            .collect();
        assert_eq!(codigos, ["f", "v", "p"]);
        assert_eq!(todo.por_pestana[Pestana::Todo.indice()], 7);
        assert_eq!(todo.por_pestana[Pestana::Archivos.indice()], 1);
        assert_eq!(todo.por_pestana[Pestana::Voz.indice()], 1);
        // Con «Archivos», solo el PDF, aunque no fuera de los primeros.
        let archivos = buscar_en(&es, "plano", 1, Pestana::Archivos);
        assert_eq!(archivos.aciertos.len(), 1);
        assert_eq!(es[archivos.aciertos[0].entrada].chat, "casa");
        let fachada = buscar_en(&es, "fachada", 20, Pestana::Archivos);
        assert_eq!(fachada.total, 0);
        // Las cuentas son de lo escrito, en cualquier pestana.
        assert_eq!(fachada.por_pestana[Pestana::Fotos.indice()], 1);
        assert_eq!(buscar(&es, "obra", 3), todo);
    }

    #[test]
    fn con_pestana_y_sin_escribir_salen_todos_los_de_esa_clase() {
        let es = de_todo();
        // Sin escribir y en «Todo», nada: seria el archivo entero.
        assert_eq!(buscar_en(&es, "  ", 10, Pestana::Todo), Hallado::default());
        let fotos = buscar_en(&es, "", 10, Pestana::Fotos);
        assert_eq!(fotos.total, 1);
        assert_eq!(
            fotos.aciertos,
            vec![Acierto {
                entrada: 0,
                pieza: Some(0)
            }]
        );
        assert_eq!(fotos.por_pestana[Pestana::Todo.indice()], 7);
        // Sin texto que ensenar, sin pieza.
        let mudo = [con(entrada("a", "x", 1, &[]), &[Pestana::Dibujos])];
        let h = buscar_en(&mudo, "", 10, Pestana::Dibujos);
        assert_eq!(h.aciertos[0].pieza, None);
        // El trozo de una sin escribir es el principio de su texto, sin
        // nada resaltado.
        let f = fragmento("Croquis obra", "", 22, 160);
        assert_eq!(f.texto, "Croquis obra");
        assert!(f.tramos.is_empty());
    }

    #[test]
    fn pulsar_abre_lo_que_tiene_visor_y_lo_demas_lleva_al_mensaje() {
        assert_eq!(al_pulsar(Cosa::Fichero { esta: true }), AlPulsar::Abrir);
        assert_eq!(al_pulsar(Cosa::ConVisor), AlPulsar::Abrir);
        assert_eq!(al_pulsar(Cosa::Nota { enlace: true }), AlPulsar::Abrir);
        // Una nota sin enlace no tiene nada que abrir: a su burbuja.
        assert_eq!(
            al_pulsar(Cosa::Nota { enlace: false }),
            AlPulsar::IrAlMensaje
        );
        // La voz se escucha en el chat, con su barra.
        assert_eq!(al_pulsar(Cosa::Voz), AlPulsar::IrAlMensaje);
        // El fichero aun no llego: al mensaje, donde se ve que falta.
        assert_eq!(
            al_pulsar(Cosa::Fichero { esta: false }),
            AlPulsar::IrAlMensaje
        );
        assert_eq!(al_pulsar(Cosa::SinNada), AlPulsar::IrAlMensaje);
    }

    #[test]
    fn las_pestanas_se_reparten_en_lineas_que_caben() {
        assert_eq!(rotulo_de_pestana("Fotos", Some(3)), "Fotos 3");
        assert_eq!(rotulo_de_pestana("Fotos", None), "Fotos");
        assert_eq!(ancho_de_pestana("Voz", 100), 2 * 10 + 3 * 7);
        assert_eq!(ancho_de_pestana("Voz", 200), 2 * (2 * 10 + 3 * 7));
        // 8 de margen a cada lado: en 216 caben 200 por linea.
        let (cajas, lineas) = repartir_pestanas(&[90, 90, 90, 300], 216, 100);
        assert_eq!(lineas, 3);
        let (m, h, a) = (
            PESTANAS_MARGEN as i32,
            PESTANA_HUECO as i32,
            PESTANA_ALTO as i32,
        );
        assert_eq!((cajas[0].x, cajas[0].y), (m, m));
        assert_eq!((cajas[1].x, cajas[1].y), (m + 90 + h, m));
        // La tercera no cabe: a la siguiente linea.
        assert_eq!((cajas[2].x, cajas[2].y), (m, m + a + h));
        // La que no cabe en ninguna, sola y recortada.
        assert_eq!((cajas[3].y, cajas[3].ancho), (m + 2 * (a + h), 200));
        assert_eq!(
            alto_de_pestanas(lineas, 100),
            (2 * m + 3 * a + 2 * h) as u32
        );
        assert_eq!(alto_de_pestanas(0, 100), 0);
        assert_eq!(repartir_pestanas(&[], 216, 100), (vec![], 0));
        assert_eq!(pestana_en(&cajas, m + 1, m + 1), Some(0));
        assert_eq!(pestana_en(&cajas, m + 90 + 1, m + 1), None, "el hueco");
        assert_eq!(pestana_en(&cajas, m + 1, m + a + h + 1), Some(2));
    }

    #[test]
    fn la_seccion_lleva_las_pestanas_arriba_y_nada_si_no_hay() {
        let fila = crate::chat::FILA as i32;
        let cab = CABECERA_SECCION as i32;
        let banda = alto_de_pestanas(2, 100) as i32;
        let s = Seccion {
            lineas_de_pestanas: 2,
            mensajes: 1,
            ver_mas: false,
            nada: false,
        };
        assert_eq!(s.elemento_en(0, 100), Some(Elemento::Pestanas));
        assert_eq!(s.elemento_en(banda, 100), Some(Elemento::Cabecera));
        assert_eq!(s.elemento_en(banda + cab, 100), Some(Elemento::Mensaje(0)));
        assert_eq!(s.alto(100), (banda + cab + fila) as u32);
        // Solo pestanas: no hay cabecera ni «Nada» hasta que se busca.
        let solo = Seccion {
            lineas_de_pestanas: 1,
            ..Default::default()
        };
        assert!(solo.hay());
        assert_eq!(solo.alto(100), alto_de_pestanas(1, 100));
        let nada = Seccion { nada: true, ..solo };
        let banda1 = alto_de_pestanas(1, 100) as i32;
        assert_eq!(nada.elemento_en(banda1, 100), Some(Elemento::Nada));
        assert_eq!(nada.alto(100), banda1 as u32 + VER_MAS);
        // Sin pestanas, «nada» no se pinta: no hay seccion.
        let sin = Seccion {
            nada: true,
            ..Default::default()
        };
        assert!(!sin.hay());
        assert_eq!(sin.alto(100), 0);
    }

    #[test]
    fn la_flechita_va_a_la_derecha_a_la_altura_del_trozo() {
        let fila = Rect {
            x: 0,
            y: 100,
            ancho: 300,
            alto: crate::chat::FILA,
        };
        let f = flecha(fila, 100);
        assert_eq!(f.derecha(), 300 - crate::chat::MARGEN_DERECHO as i32);
        assert!(f.y >= fila.y + crate::chat::NOMBRE_Y as i32);
        assert!(f.abajo() <= fila.abajo());
        assert_eq!((f.ancho, f.alto), (FLECHA, FLECHA));
    }
}
