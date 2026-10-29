//! **«Sincronizar sola»: poner el minuto a cada linea de una letra pegada,
//! oyendo la cancion** (29-oct-2026, peticion del usuario).
//!
//! La letra se pega de internet sin tiempos. El reconocedor (Whisper) oye
//! la cancion y escribe lo que entiende, con el momento de cada tanda; con
//! musica debajo se equivoca mucho, pero acierta bastantes palabras. Aqui
//! se **alinea** la letra pegada con lo oido y cada linea se lleva el
//! minuto de su primera palabra reconocida. **El texto que se guarda es
//! siempre el pegado**: lo oido solo da los tiempos.
//!
//! ## El algoritmo (puro, sin sonido ni ventanas)
//!
//! 1. **Normalizar** las dos partes ([`normalizar`]): minusculas, sin
//!    tildes ni puntuacion, en palabras. Las lineas que son solo un rotulo
//!    entre corchetes (`[Coro]`, `[Verse 2]`) no se cantan: no aportan
//!    palabras y se llevan el minuto de la linea siguiente.
//! 2. **Alinear palabra a palabra** con Needleman–Wunsch: un camino
//!    monotono (nunca hacia atras en ninguna de las dos listas) que suma
//!    por cada pareja parecida ([`parecido`], distancia de edicion: «corazon»
//!    y «corason» casan) y resta por cada palabra suelta. Lo oido antes de
//!    la primera palabra y despues de la ultima no cuesta (la intro, un
//!    «gracias» final o lo que Whisper se invente en el silencio). Una
//!    palabra de la letra sin pareja cuesta mas que una oida sin pareja:
//!    los coros de fondo y los «yeah» sobran mucho mas a menudo que faltan
//!    palabras. Un estribillo repetido sale bien solo, porque el camino es
//!    monotono: el segundo estribillo de la letra casa con el segundo que
//!    se oye.
//! 3. **Cada linea con sus parejas**: su confianza es lo que se parecen sus
//!    palabras casadas, sobre las que tiene. Con bastante confianza es un
//!    **ancla**: su minuto es el de la primera palabra casada, corrido hacia
//!    atras lo que tardan en cantarse las palabras de delante que no
//!    casaron.
//! 4. **Las lineas sin ancla** se reparten entre las anclas de alrededor
//!    segun lo largas que son (en letras), pero nunca mas despacio que el
//!    ritmo al que se canta en esta cancion: asi un solo de guitarra entre
//!    dos estrofas no estira las lineas de la primera hasta el final del
//!    solo, sino que quedan detras de su ancla y el hueco va al final.
//!
//! Salen los tiempos en orden (nunca uno antes que el de la linea de
//! antes) y una confianza por linea. Sin ninguna ancla no hay nada que
//! decir: `None`, y la pantalla lo cuenta en vez de guardar tiempos
//! inventados.
//!
//! Es el mismo calculo que puede portar el movil: entra la lista de lineas
//! (texto) y la de palabras oidas (texto y milisegundo), y sale un
//! milisegundo y una confianza por linea. Ver `docs/ui-funciones-y-chats.md`.

use crate::letra_de_cancion::{Letra, Linea};
use crate::tiempos::{Segmento, repartir};

/// Una palabra de lo oido, ya normalizada, y en que milisegundo empieza.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PalabraOida {
    pub texto: String,
    pub ms: i64,
}

impl PalabraOida {
    pub fn new(texto: impl Into<String>, ms: i64) -> PalabraOida {
        PalabraOida {
            texto: texto.into(),
            ms,
        }
    }
}

/// El minuto que le toca a una linea y cuanto fiarse de el.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineaAlineada {
    pub ms: i64,
    /// De 0 a 1: lo que se parecen sus palabras a lo oido. Una linea sin
    /// ancla (repartida entre las de alrededor) lleva 0.
    pub confianza: f32,
    /// Si el minuto sale de palabras oidas y no de repartir.
    pub anclada: bool,
}

/// El resultado de alinear: una entrada por linea, en el mismo orden.
#[derive(Debug, Clone, PartialEq)]
pub struct Alineado {
    pub lineas: Vec<LineaAlineada>,
}

impl Alineado {
    /// Cuantas lineas salen de lo oido.
    pub fn ancladas(&self) -> usize {
        self.lineas.iter().filter(|l| l.anclada).count()
    }

    /// **La letra con sus minutos**, con el texto pegado tal cual (nunca lo
    /// oido). `textos` son las mismas lineas que se alinearon.
    pub fn letra<S: AsRef<str>>(&self, textos: &[S]) -> Letra {
        Letra {
            lineas: textos
                .iter()
                .zip(&self.lineas)
                .map(|(t, l)| Linea::new(Some(l.ms), t.as_ref().trim()))
                .collect(),
            ..Letra::default()
        }
    }
}

/// Lo que tiene que parecerse una palabra a otra para casar (de 0 a 1).
const UMBRAL: f32 = 0.65;
/// Lo que cuesta una palabra de la letra que no se oyo.
const SUELTA_EN_LA_LETRA: f32 = 0.6;
/// Lo que cuesta una palabra oida que no esta en la letra (coros, «yeah»).
const SUELTA_EN_LO_OIDO: f32 = 0.4;
/// Una pareja que no se parece: un poco menos que dos sueltas.
const DISTINTAS: f32 = -0.9;
/// Lo que vale una pareja de palabras cortas («a», «de», «la»): estan en
/// todas las lineas y no pueden tirar del camino.
const CORTA: f32 = 0.5;
/// La confianza que hace falta para ser ancla.
const CONFIANZA_DE_ANCLA: f32 = 0.4;
/// Los milisegundos por letra si la cancion no deja medirlos, y sus topes.
const RITMO_POR_OMISION: f64 = 70.0;
const RITMO_MIN: f64 = 35.0;
const RITMO_MAX: f64 = 300.0;
/// Mas que esto entre dos lineas seguidas ya no es ritmo: es un hueco.
const HUECO_MS: i64 = 20_000;
/// Un tope de celdas para la tabla (n×m): 2 500 palabras por lado. Una
/// letra entera tiene unos cientos.
const CELDAS_MAX: usize = 6_250_000;

/// Una letra sin tilde: lo que se compara es como suena, no como se
/// escribe.
fn sin_tilde(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        'ý' | 'ÿ' => 'y',
        otra => otra,
    }
}

/// **Las palabras de un texto, para comparar**: en minusculas, sin tildes
/// ni puntuacion. El apostrofo junta (`can't` → `cant`); lo demas que no es
/// letra ni cifra separa. Los «x2» de repetir no son palabras.
pub fn normalizar(texto: &str) -> Vec<String> {
    let mut palabras = Vec::new();
    let mut actual = String::new();
    let cerrar = |actual: &mut String, palabras: &mut Vec<String>| {
        if !actual.is_empty() && !es_repetir(actual) {
            palabras.push(std::mem::take(actual));
        }
        actual.clear();
    };
    for c in texto.chars().flat_map(char::to_lowercase) {
        let c = sin_tilde(c);
        if c.is_alphanumeric() {
            actual.push(c);
        } else if !matches!(c, '\'' | '’' | '´' | '`') {
            cerrar(&mut actual, &mut palabras);
        }
    }
    cerrar(&mut actual, &mut palabras);
    palabras
}

/// `x2`, `x3`, `2x`: «esto se repite», no se canta.
fn es_repetir(p: &str) -> bool {
    let cifras = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    p.strip_prefix('x').is_some_and(cifras) || p.strip_suffix('x').is_some_and(cifras)
}

/// Si una linea es solo un rotulo (`[Coro]`, `[Verse 2: Ana]`): no se
/// canta.
fn es_rotulo(linea: &str) -> bool {
    let t = linea.trim();
    t.starts_with('[') && t.ends_with(']')
}

/// **Lo que se parecen dos palabras**, de 0 a 1: uno menos la distancia de
/// edicion (Levenshtein) sobre la mas larga.
pub fn parecido(a: &str, b: &str) -> f32 {
    if a == b {
        return 1.0;
    }
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let largo = a.len().max(b.len());
    if largo == 0 {
        return 1.0;
    }
    let mut fila: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = fila[0];
        fila[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let antes = fila[j + 1];
            fila[j + 1] = (diagonal + usize::from(ca != cb))
                .min(fila[j] + 1)
                .min(antes + 1);
            diagonal = antes;
        }
    }
    1.0 - fila[b.len()] as f32 / largo as f32
}

/// Lo que suma casar `a` (de la letra) con `b` (oida), y si casan.
fn puntos(a: &str, b: &str) -> (f32, f32) {
    let corta = a.chars().count() <= 2 || b.chars().count() <= 2;
    let p = if corta {
        if a == b { 1.0 } else { 0.0 }
    } else {
        parecido(a, b)
    };
    if p < UMBRAL {
        (DISTINTAS, 0.0)
    } else if corta {
        (CORTA, p)
    } else {
        (1.0 + p, p)
    }
}

/// **Las palabras oidas de las tandas del reconocedor**, con su tiempo:
/// dentro de cada tanda se reparte segun lo largas que son
/// ([`repartir`]), que es lo mas que se sabe.
pub fn palabras_oidas(segmentos: &[Segmento]) -> Vec<PalabraOida> {
    let mut salida = Vec::new();
    for s in segmentos {
        for p in repartir(s.desde_ms, s.hasta_ms, &s.texto) {
            for n in normalizar(&p.texto) {
                salida.push(PalabraOida::new(n, p.desde_ms));
            }
        }
    }
    salida
}

/// Las palabras mas corrientes de cada idioma, para adivinar en cual esta
/// una letra pegada. Solo las que no se comparten entre ellos.
const CORRIENTES: &[(&str, &[&str])] = &[
    (
        "es",
        &[
            "que", "el", "los", "las", "del", "con", "por", "para", "pero", "como", "mas", "yo",
            "mi", "tu", "esta", "estoy", "eres", "soy", "una", "porque", "corazon", "quiero",
            "nada", "cuando", "donde", "nunca", "siempre", "noche", "amor", "y",
        ],
    ),
    (
        "en",
        &[
            "the", "and", "you", "your", "is", "are", "my", "me", "it", "that", "with", "for",
            "this", "love", "baby", "dont", "cant", "im", "what", "when", "never", "know", "just",
            "all", "be", "we", "oh", "will", "hope",
        ],
    ),
    (
        "pt",
        &[
            "nao", "voce", "meu", "minha", "com", "uma", "isso", "eu", "ela", "ele", "coracao",
            "saudade", "tudo", "agora", "muito", "quero", "sou", "estou", "nos", "do", "da",
        ],
    ),
    (
        "fr",
        &[
            "le", "les", "et", "je", "tu", "est", "pas", "moi", "toi", "mon", "ma", "avec", "pour",
            "qui", "dans", "cest", "amour", "nous", "vous", "suis", "jamais", "toujours",
        ],
    ),
    (
        "it",
        &[
            "il", "che", "non", "sono", "di", "per", "una", "mio", "mia", "con", "ti", "amore",
            "cuore", "sei", "come", "tutto", "questo", "anche", "sempre", "perche",
        ],
    ),
    (
        "de",
        &[
            "und", "ich", "du", "der", "die", "das", "nicht", "ist", "mein", "dich", "mich", "ein",
            "eine", "wir", "auf", "liebe", "herz", "immer", "nie", "bin",
        ],
    ),
];

/// **En que idioma esta una letra**, por sus palabras corrientes: el codigo
/// de Whisper (`es`, `en`...) o `None` si no esta claro (pocas palabras, o
/// dos idiomas igualados). Asi una cancion en ingles se oye en ingles
/// aunque PixPin este en castellano.
pub fn idioma_de_la_letra<S: AsRef<str>>(lineas: &[S]) -> Option<&'static str> {
    let palabras: Vec<String> = lineas.iter().flat_map(|l| normalizar(l.as_ref())).collect();
    let mut cuentas: Vec<(&'static str, usize)> = CORRIENTES
        .iter()
        .map(|(codigo, lista)| {
            let n = palabras
                .iter()
                .filter(|p| lista.contains(&p.as_str()))
                .count();
            (*codigo, n)
        })
        .collect();
    cuentas.sort_by_key(|c| std::cmp::Reverse(c.1));
    let (codigo, n) = cuentas[0];
    let segundo = cuentas.get(1).map_or(0, |c| c.1);
    (n >= 4 && n * 2 >= segundo * 3).then_some(codigo)
}

/// Una palabra de la letra: de que linea es y cuantas letras de su linea
/// van delante (para correr el minuto hacia atras).
struct DeLaLetra {
    texto: String,
    linea: usize,
    letras_antes: usize,
}

/// **Alinea las lineas de una letra con lo oido.** `duracion_ms`, si se
/// sabe, es el tope de las lineas del final. `None` si ninguna linea se
/// pudo anclar (no se entendio nada de la letra, o era otra cancion).
pub fn alinear<S: AsRef<str>>(
    lineas: &[S],
    oidas: &[PalabraOida],
    duracion_ms: Option<i64>,
) -> Option<Alineado> {
    if lineas.is_empty() || oidas.is_empty() {
        return None;
    }
    // Las palabras de la letra, y las letras (normalizadas) de cada linea.
    let mut letra: Vec<DeLaLetra> = Vec::new();
    let mut letras: Vec<usize> = Vec::with_capacity(lineas.len());
    let mut palabras_de: Vec<usize> = Vec::with_capacity(lineas.len());
    for (i, l) in lineas.iter().enumerate() {
        let l = l.as_ref();
        let palabras = if es_rotulo(l) {
            Vec::new()
        } else {
            normalizar(l)
        };
        let mut antes = 0usize;
        palabras_de.push(palabras.len());
        for p in palabras {
            let n = p.chars().count();
            letra.push(DeLaLetra {
                texto: p,
                linea: i,
                letras_antes: antes,
            });
            antes += n + 1;
        }
        letras.push(antes);
    }
    if letra.is_empty() || letra.len().saturating_mul(oidas.len()) > CELDAS_MAX {
        return None;
    }
    let parejas = needleman_wunsch(&letra, oidas);

    // Cada linea: lo que se parecen sus palabras casadas y la primera.
    let mut suma = vec![0f32; lineas.len()];
    let mut casadas = vec![0usize; lineas.len()];
    let mut primera: Vec<Option<(usize, i64)>> = vec![None; lineas.len()];
    let mut ultima: Vec<Option<(usize, i64)>> = vec![None; lineas.len()];
    for &(i, j, p) in &parejas {
        let w = &letra[i];
        suma[w.linea] += p;
        casadas[w.linea] += 1;
        let aqui = (w.letras_antes, oidas[j].ms);
        primera[w.linea].get_or_insert(aqui);
        ultima[w.linea] = Some(aqui);
    }
    let confianza: Vec<f32> = (0..lineas.len())
        .map(|i| {
            if palabras_de[i] == 0 {
                0.0
            } else {
                (suma[i] / palabras_de[i] as f32).clamp(0.0, 1.0)
            }
        })
        .collect();
    let es_ancla = |i: usize| {
        let necesita = palabras_de[i].min(2);
        palabras_de[i] > 0 && casadas[i] >= necesita && confianza[i] >= CONFIANZA_DE_ANCLA
    };

    // El ritmo de esta cancion (ms por letra), medido entre palabras casadas
    // de una misma linea y entre dos lineas seguidas que empiezan casando
    // (eso cuenta tambien el respiro entre lineas); si no hay, el de por
    // omision.
    let mut medidas: Vec<f64> = (0..lineas.len())
        .filter_map(|i| {
            let ((a, ta), (b, tb)) = (primera[i]?, ultima[i]?);
            (b > a && tb > ta).then(|| (tb - ta) as f64 / (b - a) as f64)
        })
        .collect();
    medidas.extend((1..lineas.len()).filter_map(|i| {
        let ((0, ta), (0, tb)) = (primera[i - 1]?, primera[i]?) else {
            return None;
        };
        let dura = tb - ta;
        (es_ancla(i - 1) && es_ancla(i) && dura > 0 && dura < HUECO_MS && letras[i - 1] > 0)
            .then(|| dura as f64 / letras[i - 1] as f64)
    }));
    let ritmo = mediana(&mut medidas)
        .unwrap_or(RITMO_POR_OMISION)
        .clamp(RITMO_MIN, RITMO_MAX);

    // Las anclas: el minuto de su primera palabra casada, corrido hacia
    // atras lo de las palabras de delante. Nunca antes que el ancla de antes.
    let mut tiempos: Vec<Option<i64>> = vec![None; lineas.len()];
    let mut anterior = 0i64;
    for i in 0..lineas.len() {
        if !es_ancla(i) {
            continue;
        }
        let Some((antes, t)) = primera[i] else {
            continue;
        };
        let ms = (t - (antes as f64 * ritmo) as i64).max(anterior).max(0);
        tiempos[i] = Some(ms);
        anterior = ms;
    }
    let anclas: Vec<usize> = (0..lineas.len())
        .filter(|&i| tiempos[i].is_some())
        .collect();
    let (&primer_ancla, &ultima_ancla) = (anclas.first()?, anclas.last()?);

    // Lo que «pesa» cada linea al repartir: sus letras, y al menos una (una
    // linea de «oh» tambien ocupa su sitio). Los rotulos no pesan: van con
    // la siguiente.
    let peso: Vec<f64> = (0..lineas.len())
        .map(|i| {
            if palabras_de[i] == 0 {
                0.0
            } else {
                letras[i].max(1) as f64
            }
        })
        .collect();
    let mut ms: Vec<i64> = vec![0; lineas.len()];
    for i in 0..lineas.len() {
        if let Some(t) = tiempos[i] {
            ms[i] = t;
        }
    }
    // Antes de la primera ancla: hacia atras al ritmo, sin bajar de lo
    // repartido desde el principio de la cancion.
    {
        let tb = ms[primer_ancla] as f64;
        let total: f64 = peso[..primer_ancla].iter().sum();
        let mut detras = 0.0; // letras entre la linea y el ancla
        for j in (0..primer_ancla).rev() {
            detras += peso[j];
            let atras = tb - detras * ritmo;
            let repartido = if total > 0.0 {
                tb * (total - detras) / total
            } else {
                tb
            };
            ms[j] = atras.max(repartido).max(0.0) as i64;
        }
    }
    // Entre dos anclas: repartido por letras, sin ir mas despacio que el
    // ritmo (el hueco instrumental se queda al final, antes del ancla).
    for par in anclas.windows(2) {
        let (a, b) = (par[0], par[1]);
        if b == a + 1 {
            continue;
        }
        let (ta, tb) = (ms[a] as f64, ms[b] as f64);
        let total: f64 = peso[a..b].iter().sum();
        let mut hecho = 0.0;
        for j in a + 1..b {
            hecho += peso[j - 1];
            let repartido = if total > 0.0 {
                ta + (tb - ta) * hecho / total
            } else {
                ta
            };
            ms[j] = repartido.min(ta + hecho * ritmo) as i64;
        }
    }
    // Detras de la ultima: al ritmo, sin pasar del final de la cancion.
    {
        let ta = ms[ultima_ancla] as f64;
        let total: f64 = peso[ultima_ancla..].iter().sum();
        let fin = duracion_ms.map(|d| d as f64).filter(|d| *d > ta);
        let mut hecho = 0.0;
        for j in ultima_ancla + 1..lineas.len() {
            hecho += peso[j - 1];
            let al_ritmo = ta + hecho * ritmo;
            ms[j] = match fin {
                Some(fin) if total > 0.0 => al_ritmo.min(ta + (fin - ta) * hecho / total),
                _ => al_ritmo,
            } as i64;
        }
    }
    // Un rotulo va con la linea que lo sigue (o con la de antes si es el
    // ultimo): se resalta cuando empieza lo que anuncia.
    for i in (0..lineas.len()).rev() {
        if palabras_de[i] == 0 {
            ms[i] = if i + 1 < lineas.len() {
                ms[i + 1]
            } else if i > 0 {
                ms[i - 1]
            } else {
                0
            };
        }
    }
    // Y en orden, por si acaso.
    for i in 1..ms.len() {
        ms[i] = ms[i].max(ms[i - 1]);
    }
    Some(Alineado {
        lineas: (0..lineas.len())
            .map(|i| LineaAlineada {
                ms: ms[i],
                confianza: if tiempos[i].is_some() {
                    confianza[i]
                } else {
                    0.0
                },
                anclada: tiempos[i].is_some(),
            })
            .collect(),
    })
}

fn mediana(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

/// El camino de Needleman–Wunsch entre la letra y lo oido: las parejas
/// `(palabra de la letra, palabra oida, parecido)` que casan, en orden.
/// Lo oido antes de la primera pareja y despues de la ultima no cuesta.
fn needleman_wunsch(letra: &[DeLaLetra], oidas: &[PalabraOida]) -> Vec<(usize, usize, f32)> {
    const DIAGONAL: u8 = 0;
    const ARRIBA: u8 = 1; // palabra de la letra suelta
    const IZQUIERDA: u8 = 2; // palabra oida suelta
    let (n, m) = (letra.len(), oidas.len());
    let ancho = m + 1;
    let mut nota = vec![0f32; (n + 1) * ancho];
    let mut desde = vec![IZQUIERDA; (n + 1) * ancho];
    for i in 1..=n {
        nota[i * ancho] = -(i as f32) * SUELTA_EN_LA_LETRA;
        desde[i * ancho] = ARRIBA;
    }
    for i in 1..=n {
        for j in 1..=m {
            let (p, _) = puntos(&letra[i - 1].texto, &oidas[j - 1].texto);
            let diagonal = nota[(i - 1) * ancho + j - 1] + p;
            let arriba = nota[(i - 1) * ancho + j] - SUELTA_EN_LA_LETRA;
            // Al final de la letra, lo oido que sobra no cuesta.
            let izquierda = nota[i * ancho + j - 1] - if i == n { 0.0 } else { SUELTA_EN_LO_OIDO };
            let (v, d) = if diagonal >= arriba && diagonal >= izquierda {
                (diagonal, DIAGONAL)
            } else if arriba >= izquierda {
                (arriba, ARRIBA)
            } else {
                (izquierda, IZQUIERDA)
            };
            nota[i * ancho + j] = v;
            desde[i * ancho + j] = d;
        }
    }
    let mut parejas = Vec::new();
    let (mut i, mut j) = (n, m);
    while i > 0 {
        match if j == 0 { ARRIBA } else { desde[i * ancho + j] } {
            DIAGONAL => {
                let (_, p) = puntos(&letra[i - 1].texto, &oidas[j - 1].texto);
                if p > 0.0 {
                    parejas.push((i - 1, j - 1, p));
                }
                i -= 1;
                j -= 1;
            }
            ARRIBA => i -= 1,
            _ => j -= 1,
        }
    }
    parejas.reverse();
    parejas
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Lo oido de unas lineas, palabra a palabra desde `desde` ms, a
    /// `paso` ms por palabra.
    fn cantar(lineas: &[(&str, i64)], paso: i64) -> Vec<PalabraOida> {
        let mut v = Vec::new();
        for (texto, desde) in lineas {
            for (k, p) in normalizar(texto).into_iter().enumerate() {
                v.push(PalabraOida::new(p, desde + k as i64 * paso));
            }
        }
        v
    }

    fn tiempos(a: &Alineado) -> Vec<i64> {
        a.lineas.iter().map(|l| l.ms).collect()
    }

    const LETRA: [&str; 4] = [
        "Bésame, bésame mucho",
        "como si fuera esta noche la última vez",
        "que tengo miedo a perderte",
        "perderte después",
    ];

    #[test]
    fn normalizar_quita_tildes_puntuacion_y_mayusculas() {
        assert_eq!(normalizar("¡Bésame, MUCHO!"), ["besame", "mucho"]);
        assert_eq!(normalizar("can't stop—now"), ["cant", "stop", "now"]);
        assert_eq!(normalizar("Corazón (x2)"), ["corazon"]);
        assert_eq!(normalizar("  ...  "), Vec::<String>::new());
        assert_eq!(normalizar("año 2x2"), ["ano", "2x2"]);
    }

    #[test]
    fn el_parecido_es_la_distancia_de_edicion() {
        assert_eq!(parecido("hola", "hola"), 1.0);
        assert!((parecido("corazon", "corason") - 6.0 / 7.0).abs() < 1e-6);
        assert_eq!(parecido("abc", "xyz"), 0.0);
        assert_eq!(parecido("", ""), 1.0);
    }

    #[test]
    fn lo_oido_igual_que_la_letra_da_el_minuto_de_cada_linea() {
        let oido = cantar(
            &[
                (LETRA[0], 12_000),
                (LETRA[1], 16_000),
                (LETRA[2], 22_000),
                (LETRA[3], 27_000),
            ],
            400,
        );
        let a = alinear(&LETRA, &oido, None).unwrap();
        assert_eq!(tiempos(&a), [12_000, 16_000, 22_000, 27_000]);
        assert_eq!(a.ancladas(), 4);
        assert!(a.lineas.iter().all(|l| l.confianza > 0.95));
        // Y se guarda con el texto pegado, no con lo oido.
        let guardado = crate::letra_de_cancion::a_guardar(&a.letra(&LETRA));
        assert!(guardado.starts_with("[0:12] Bésame, bésame mucho\n\n[0:16] como si"));
    }

    #[test]
    fn los_errores_del_reconocedor_no_lo_tumban() {
        // Whisper con musica: palabras mal oidas, alguna que falta, y una
        // inventada en la intro.
        let oido = cantar(
            &[
                ("gracias por ver el video", 1_000),
                ("besame besamé muchos", 12_000),
                ("como si fuera esta noche la ultima", 16_000),
                ("tengo midio a perderte", 22_400),
                ("perder te despues", 27_000),
            ],
            400,
        );
        let a = alinear(&LETRA, &oido, Some(200_000)).unwrap();
        let t = tiempos(&a);
        assert_eq!(t[0], 12_000);
        assert_eq!(t[1], 16_000);
        // «que» no se oyo: el minuto de «tengo», corrido una palabra atras.
        assert!((21_900..=22_400).contains(&t[2]), "{t:?}");
        assert!((26_000..=27_500).contains(&t[3]), "{t:?}");
        assert!(a.lineas[2].anclada);
    }

    #[test]
    fn el_estribillo_repetido_casa_con_su_vez() {
        let letra = [
            "en la orilla del mar te conoci",
            "y tu risa me llevo",
            "baila baila conmigo esta noche",
            "que la luna nos mira",
            "otro verano otra ciudad distinta",
            "baila baila conmigo esta noche",
            "que la luna nos mira",
        ];
        let oido = cantar(
            &[
                (letra[0], 10_000),
                (letra[1], 14_000),
                (letra[2], 20_000),
                (letra[3], 24_000),
                (letra[4], 40_000),
                (letra[5], 50_000),
                (letra[6], 54_000),
            ],
            300,
        );
        let a = alinear(&letra, &oido, None).unwrap();
        assert_eq!(
            tiempos(&a),
            [10_000, 14_000, 20_000, 24_000, 40_000, 50_000, 54_000]
        );
    }

    #[test]
    fn una_linea_que_no_se_oye_se_reparte_entre_sus_vecinas() {
        let oido = cantar(
            &[(LETRA[0], 12_000), (LETRA[1], 16_000), (LETRA[3], 27_000)],
            400,
        );
        let a = alinear(&LETRA, &oido, None).unwrap();
        let t = tiempos(&a);
        assert!(!a.lineas[2].anclada);
        assert_eq!(a.lineas[2].confianza, 0.0);
        assert!(t[1] < t[2] && t[2] < t[3], "{t:?}");
    }

    #[test]
    fn los_coros_de_fondo_y_los_yeah_no_mueven_nada() {
        let oido = cantar(
            &[
                (LETRA[0], 12_000),
                ("yeah yeah oh oh oh uoh", 14_000),
                (LETRA[1], 16_000),
                ("eh eh baby", 20_000),
                (LETRA[2], 22_000),
                (LETRA[3], 27_000),
                ("uh uh uh na na na", 30_000),
            ],
            400,
        );
        let a = alinear(&LETRA, &oido, None).unwrap();
        assert_eq!(tiempos(&a), [12_000, 16_000, 22_000, 27_000]);
    }

    #[test]
    fn el_hueco_instrumental_se_queda_antes_de_la_estrofa_siguiente() {
        let letra = [
            "primera estrofa que se canta",
            "una linea que el reconocedor no pillo",
            "despues del solo de guitarra larguisimo",
        ];
        let oido = cantar(&[(letra[0], 10_000), (letra[2], 90_000)], 400);
        let a = alinear(&letra, &oido, None).unwrap();
        let t = tiempos(&a);
        // Al ritmo, detras de la primera, y no a mitad del solo (50 s).
        assert!(t[1] > 10_000 && t[1] < 15_000, "{t:?}");
        assert_eq!(t[2], 90_000);
    }

    #[test]
    fn un_rotulo_va_con_la_linea_que_lo_sigue() {
        let letra = ["[Coro]", "baila baila conmigo", "[Final]"];
        let oido = cantar(&[("baila baila conmigo", 5_000)], 300);
        let a = alinear(&letra, &oido, None).unwrap();
        assert_eq!(tiempos(&a), [5_000, 5_000, 5_000]);
        assert!(!a.lineas[0].anclada);
    }

    #[test]
    fn las_lineas_del_final_no_pasan_de_lo_que_dura() {
        let letra = ["la primera linea cantada", "segunda", "tercera", "cuarta"];
        let oido = cantar(&[(letra[0], 170_000)], 300);
        let a = alinear(&letra, &oido, Some(172_000)).unwrap();
        let t = tiempos(&a);
        assert!(t.windows(2).all(|w| w[0] <= w[1]), "{t:?}");
        assert!(t[3] < 172_000, "{t:?}");
    }

    #[test]
    fn sin_nada_reconocible_no_se_inventan_tiempos() {
        let oido = cantar(&[("zzz qqq www", 1_000)], 300);
        assert_eq!(alinear(&LETRA, &oido, None), None);
        assert_eq!(alinear(&LETRA, &[], None), None);
        assert_eq!(alinear::<&str>(&[], &oido, None), None);
    }

    #[test]
    fn las_palabras_oidas_de_las_tandas_reparten_su_tiempo() {
        let s = [Segmento::entre(10_000, 12_000, "Hola, mundo")];
        let p = palabras_oidas(&s);
        assert_eq!(
            p,
            [
                PalabraOida::new("hola", 10_000),
                PalabraOida::new("mundo", 11_000)
            ]
        );
    }

    #[test]
    fn se_adivina_el_idioma_de_la_letra() {
        assert_eq!(idioma_de_la_letra(&LETRA), Some("es"));
        let ingles = [
            "I will always love you",
            "and I hope that life treats you kind",
            "you know that I'm not what you need",
        ];
        assert_eq!(idioma_de_la_letra(&ingles), Some("en"));
        assert_eq!(idioma_de_la_letra(&["la la la"]), None);
    }

    #[test]
    fn las_lineas_salen_siempre_en_orden() {
        // Lo oido desordenado en el tiempo (una tanda mal medida) no puede
        // dar un minuto antes que el de la linea de antes.
        let mut oido = cantar(&[(LETRA[0], 12_000), (LETRA[1], 16_000)], 400);
        oido.extend(cantar(&[(LETRA[2], 15_000), (LETRA[3], 27_000)], 100));
        let a = alinear(&LETRA, &oido, None).unwrap();
        let t = tiempos(&a);
        assert!(t.windows(2).all(|w| w[0] <= w[1]), "{t:?}");
    }
}
