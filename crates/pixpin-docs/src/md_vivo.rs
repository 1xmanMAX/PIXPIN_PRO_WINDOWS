//! **El Markdown que se ve formateado mientras se escribe** (H12): el puerto
//! de lo que el editor de notas del movil (`motormd/EditorVivo.kt`,
//! `Vivo.kt`, `Inline.kt`) sabe del texto, sin nada de pintar.
//!
//! # Por que el texto no se toca
//!
//! En el movil el editor parte cada bloque en «texto limpio + estilos» y lo
//! vuelve a juntar al escribir, como Telegram. Aqui el que edita es el
//! `RichEdit` de Windows, y el camino barato y seguro es otro: **el texto del
//! control ES el Markdown**, letra por letra, y el formato se le pone
//! encima. Las marcas (`#`, `**`, `[`, `](url)`) se esconden con texto oculto
//! en todos los renglones menos en el que tiene el cursor, que es donde hace
//! falta verlas para no borrar a ciegas. Asi guardar es leer el texto y ya:
//! no hay traductor entre el dedo y el fichero, que es justo lo que el
//! comentario de `MarkdownEditorActivity.kt` pide evitar.
//!
//! Todo va en **unidades UTF-16**, que son las posiciones del `RichEdit`
//! (una letra fuera del plano basico, un emoji, cuenta dos). Un salto de
//! renglon cuenta uno, sea `\n` o el `\r` que devuelve el control.
//!
//! Este modulo es puro: se prueba sin ventanas.

/// Lo que se le pone a un tramo del texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estilo {
    /// Una marca de Markdown: se esconde fuera del renglon del cursor.
    Marca,
    /// El texto de un titulo, nivel 1 a 6.
    Titulo(u8),
    Negrita,
    Cursiva,
    Tachado,
    /// Codigo dentro de un parrafo, entre acentos graves.
    Codigo,
    /// Un renglon dentro de un bloque de codigo con vallas.
    BloqueCodigo,
    /// Un renglon de cita (`> `): el renglon entero.
    Cita,
    /// El texto visible de un enlace; la direccion va en `Tramo::url`.
    Enlace,
    /// La casilla de una lista de tareas (`[ ]` o `[x]`).
    Casilla { hecha: bool },
    /// Lo escrito tras una casilla marcada: se ve tachado y apagado.
    Hecha,
    /// Un renglon de lista con vineta: el renglon entero.
    Vineta,
    /// El numero de una lista numerada (`3.`), que se ve.
    Numero,
    /// Una formula entre dolares (`$x^2$`).
    Formula,
    /// Una raya (`---`): el renglon entero.
    Regla,
}

/// Un tramo con estilo, en posiciones UTF-16 del texto entero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tramo {
    pub desde: usize,
    pub hasta: usize,
    pub estilo: Estilo,
    /// El renglon (desde 0) al que pertenece: una marca solo se ve en el
    /// renglon del cursor.
    pub linea: usize,
    /// La direccion, solo en un [`Estilo::Enlace`].
    pub url: Option<String>,
}

/// Un renglon: de donde a donde va, sin su salto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Linea {
    pub desde: usize,
    pub hasta: usize,
}

fn es_salto(c: char) -> bool {
    c == '\n' || c == '\r'
}

/// Los renglones del texto, en UTF-16. Siempre hay al menos uno: el texto
/// vacio es un renglon vacio, igual que el documento vacio del movil es un
/// bloque vacio (`Vivo.trozos`).
pub fn lineas(texto: &str) -> Vec<Linea> {
    let mut v = Vec::new();
    let mut desde = 0;
    let mut pos = 0;
    for c in texto.chars() {
        if es_salto(c) {
            v.push(Linea { desde, hasta: pos });
            pos += 1;
            desde = pos;
        } else {
            pos += c.len_utf16();
        }
    }
    v.push(Linea { desde, hasta: pos });
    v
}

/// El renglon donde cae una posicion. Una posicion justo en el salto es del
/// renglon que acaba ahi, como el cursor al final de una linea.
pub fn linea_de(lineas: &[Linea], pos: usize) -> usize {
    lineas
        .iter()
        .position(|l| pos <= l.hasta)
        .unwrap_or(lineas.len().saturating_sub(1))
}

/// Un renglon ya partido en letras, con la posicion UTF-16 de cada una.
struct Renglon {
    letras: Vec<char>,
    /// `pos[i]` es donde empieza la letra `i`; `pos[len]` es el final.
    pos: Vec<usize>,
    numero: usize,
}

impl Renglon {
    fn nuevo(texto: &str, desde: usize, numero: usize) -> Renglon {
        let letras: Vec<char> = texto.chars().collect();
        let mut pos = Vec::with_capacity(letras.len() + 1);
        let mut p = desde;
        for c in &letras {
            pos.push(p);
            p += c.len_utf16();
        }
        pos.push(p);
        Renglon {
            letras,
            pos,
            numero,
        }
    }

    fn len(&self) -> usize {
        self.letras.len()
    }

    fn poner(&self, sal: &mut Vec<Tramo>, a: usize, b: usize, estilo: Estilo) {
        self.poner_url(sal, a, b, estilo, None);
    }

    fn poner_url(&self, sal: &mut Vec<Tramo>, a: usize, b: usize, estilo: Estilo, url: Option<String>) {
        if b > a {
            sal.push(Tramo {
                desde: self.pos[a],
                hasta: self.pos[b],
                estilo,
                linea: self.numero,
                url,
            });
        }
    }
}

/// Todos los tramos con estilo del texto, en orden de renglon.
///
/// Lo que no se entiende se queda como texto normal: es la misma promesa que
/// el parser del movil («antes texto de mas que texto perdido»).
pub fn analizar(texto: &str) -> Vec<Tramo> {
    let mut sal = Vec::new();
    let mut en_codigo = false;
    let mut desde = 0;
    for (numero, trozo) in texto.split(['\n', '\r']).enumerate() {
        let r = Renglon::nuevo(trozo, desde, numero);
        desde = r.pos[r.len()] + 1;
        bloque(&r, &mut en_codigo, &mut sal);
    }
    sal
}

/// Las vallas de un bloque de codigo: tres acentos graves o tres virgulillas.
fn es_valla(l: &[char]) -> bool {
    let t: String = l.iter().collect::<String>().trim_start().chars().take(3).collect();
    t == "```" || t == "~~~"
}

fn es_regla(l: &[char]) -> bool {
    let sin: Vec<char> = l.iter().copied().filter(|c| !c.is_whitespace()).collect();
    sin.len() >= 3 && ['-', '*', '_'].iter().any(|m| sin.iter().all(|c| c == m))
}

/// Lo que marca el principio de un renglon de lista: `- `, `* ` o `+ `,
/// con su sangria. Devuelve donde empieza el guion.
fn vineta(l: &[char]) -> Option<usize> {
    let k = l.iter().take_while(|c| **c == ' ' || **c == '\t').count();
    (l.len() >= k + 2 && matches!(l[k], '-' | '*' | '+') && l[k + 1] == ' ').then_some(k)
}

/// `[ ]`, `[x]` o `[X]` justo detras de la vineta.
fn casilla(l: &[char], tras: usize) -> Option<bool> {
    if l.len() < tras + 3 || l[tras] != '[' || l[tras + 2] != ']' {
        return None;
    }
    let hecha = match l[tras + 1] {
        ' ' => false,
        'x' | 'X' => true,
        _ => return None,
    };
    // Detras, un espacio o el final del renglon: `[x]coso` no es casilla.
    (l.len() == tras + 3 || l[tras + 3] == ' ').then_some(hecha)
}

/// `12. ` o `12) `: donde acaba el numero con su punto.
fn numero(l: &[char]) -> Option<(usize, usize)> {
    let k = l.iter().take_while(|c| **c == ' ' || **c == '\t').count();
    let d = l[k..].iter().take_while(|c| c.is_ascii_digit()).count();
    let fin = k + d;
    (d > 0 && d <= 9 && l.len() > fin + 1 && matches!(l[fin], '.' | ')') && l[fin + 1] == ' ')
        .then_some((k, fin + 1))
}

fn bloque(r: &Renglon, en_codigo: &mut bool, sal: &mut Vec<Tramo>) {
    let l = &r.letras;
    let n = r.len();
    if es_valla(l) {
        r.poner(sal, 0, n, Estilo::Marca);
        *en_codigo = !*en_codigo;
        return;
    }
    if *en_codigo {
        r.poner(sal, 0, n, Estilo::BloqueCodigo);
        return;
    }
    if es_regla(l) {
        r.poner(sal, 0, n, Estilo::Regla);
        return;
    }
    // Titulo: de una a seis almohadillas y un espacio.
    let almohadillas = l.iter().take_while(|c| **c == '#').count();
    if (1..=6).contains(&almohadillas) && (n == almohadillas || l[almohadillas] == ' ') {
        let cuerpo = (almohadillas + 1).min(n);
        r.poner(sal, 0, cuerpo, Estilo::Marca);
        r.poner(sal, cuerpo, n, Estilo::Titulo(almohadillas as u8));
        en_linea(r, cuerpo, n, sal);
        return;
    }
    if n > 0 && l[0] == '>' {
        let cuerpo = if n > 1 && l[1] == ' ' { 2 } else { 1 };
        r.poner(sal, 0, n, Estilo::Cita);
        r.poner(sal, 0, cuerpo, Estilo::Marca);
        en_linea(r, cuerpo, n, sal);
        return;
    }
    if let Some(k) = vineta(l) {
        if let Some(hecha) = casilla(l, k + 2) {
            r.poner(sal, k, k + 2, Estilo::Marca);
            r.poner(sal, k + 2, k + 5, Estilo::Casilla { hecha });
            let cuerpo = (k + 6).min(n);
            if hecha {
                r.poner(sal, cuerpo, n, Estilo::Hecha);
            }
            en_linea(r, cuerpo, n, sal);
        } else {
            r.poner(sal, 0, n, Estilo::Vineta);
            r.poner(sal, k, k + 2, Estilo::Marca);
            en_linea(r, k + 2, n, sal);
        }
        return;
    }
    if let Some((k, fin)) = numero(l) {
        r.poner(sal, k, fin, Estilo::Numero);
        en_linea(r, fin + 1, n, sal);
        return;
    }
    en_linea(r, 0, n, sal);
}

fn alfanumerica(c: char) -> bool {
    c.is_alphanumeric()
}

/// Los estilos dentro de un renglon, entre las letras `desde` y `hasta`.
fn en_linea(r: &Renglon, desde: usize, hasta: usize, sal: &mut Vec<Tramo>) {
    let l = &r.letras;
    let mut i = desde;
    while i < hasta {
        let c = l[i];
        match c {
            // Una barra invertida deja la letra siguiente tal cual.
            '\\' if i + 1 < hasta && l[i + 1].is_ascii_punctuation() => {
                r.poner(sal, i, i + 1, Estilo::Marca);
                i += 2;
            }
            '`' => match (i + 1..hasta).find(|&j| l[j] == '`') {
                Some(j) if j > i + 1 => {
                    r.poner(sal, i, i + 1, Estilo::Marca);
                    r.poner(sal, i + 1, j, Estilo::Codigo);
                    r.poner(sal, j, j + 1, Estilo::Marca);
                    i = j + 1;
                }
                _ => i += 1,
            },
            // `$x$`, pero no «$5 y $6»: tras el dolar de abrir no va un
            // espacio, antes del de cerrar tampoco, y detras no va una cifra.
            '$' if i + 1 < hasta && l[i + 1] != ' ' && l[i + 1] != '$' => {
                let cierre = (i + 2..hasta).find(|&j| {
                    l[j] == '$' && l[j - 1] != ' ' && (j + 1 >= hasta || !l[j + 1].is_ascii_digit())
                });
                match cierre {
                    Some(j) => {
                        r.poner(sal, i, i + 1, Estilo::Marca);
                        r.poner(sal, i + 1, j, Estilo::Formula);
                        r.poner(sal, j, j + 1, Estilo::Marca);
                        i = j + 1;
                    }
                    None => i += 1,
                }
            }
            '[' | '!' => match enlace(l, i, hasta) {
                Some((texto_desde, texto_hasta, fin, url)) => {
                    r.poner(sal, i, texto_desde, Estilo::Marca);
                    r.poner_url(sal, texto_desde, texto_hasta, Estilo::Enlace, Some(url));
                    en_linea(r, texto_desde, texto_hasta, sal);
                    r.poner(sal, texto_hasta, fin, Estilo::Marca);
                    i = fin;
                }
                None => i += 1,
            },
            '~' if i + 1 < hasta && l[i + 1] == '~' => match cierre_doble(l, i + 2, hasta, '~') {
                Some(j) => {
                    r.poner(sal, i, i + 2, Estilo::Marca);
                    r.poner(sal, i + 2, j, Estilo::Tachado);
                    en_linea(r, i + 2, j, sal);
                    r.poner(sal, j, j + 2, Estilo::Marca);
                    i = j + 2;
                }
                None => i += 2,
            },
            '*' | '_' => {
                // `_` dentro de una palabra (`nombre_de_fichero`) no es nada.
                if c == '_' && i > 0 && alfanumerica(l[i - 1]) {
                    i += 1;
                    continue;
                }
                let doble = i + 1 < hasta && l[i + 1] == c;
                if doble
                    && let Some(j) = cierre_doble(l, i + 2, hasta, c)
                {
                    r.poner(sal, i, i + 2, Estilo::Marca);
                    r.poner(sal, i + 2, j, Estilo::Negrita);
                    en_linea(r, i + 2, j, sal);
                    r.poner(sal, j, j + 2, Estilo::Marca);
                    i = j + 2;
                    continue;
                }
                match cierre_simple(l, i + 1, hasta, c) {
                    Some(j) => {
                        r.poner(sal, i, i + 1, Estilo::Marca);
                        r.poner(sal, i + 1, j, Estilo::Cursiva);
                        en_linea(r, i + 1, j, sal);
                        r.poner(sal, j, j + 1, Estilo::Marca);
                        i = j + 1;
                    }
                    None => i += if doble { 2 } else { 1 },
                }
            }
            _ => i += 1,
        }
    }
}

/// Donde cierra `**`/`__`/`~~` abierto justo antes de `desde`. Con una racha
/// de tres (`***x***`) se queda con las dos ultimas: lo de dentro es `*x*`,
/// una cursiva dentro de la negrita.
fn cierre_doble(l: &[char], desde: usize, hasta: usize, c: char) -> Option<usize> {
    if desde >= hasta || l[desde] == ' ' {
        return None;
    }
    let mut j = desde + 1;
    while j + 1 < hasta {
        if l[j] == c && l[j + 1] == c && l[j - 1] != ' ' {
            while j + 2 < hasta && l[j + 2] == c {
                j += 1;
            }
            return Some(j);
        }
        j += 1;
    }
    None
}

fn cierre_simple(l: &[char], desde: usize, hasta: usize, c: char) -> Option<usize> {
    if desde >= hasta || l[desde] == ' ' || l[desde] == c {
        return None;
    }
    (desde + 1..hasta).find(|&j| {
        l[j] == c
            && l[j - 1] != ' '
            && (c != '_' || j + 1 >= hasta || !alfanumerica(l[j + 1]))
    })
}

/// `[texto](url)` o `![texto](url)` empezando en `i`: donde empieza y acaba
/// el texto, donde acaba todo y la direccion.
fn enlace(l: &[char], i: usize, hasta: usize) -> Option<(usize, usize, usize, String)> {
    let abre = if l[i] == '!' {
        (i + 1 < hasta && l[i + 1] == '[').then_some(i + 1)?
    } else {
        i
    };
    let cierra = (abre + 1..hasta).find(|&j| l[j] == ']')?;
    if cierra + 1 >= hasta || l[cierra + 1] != '(' {
        return None;
    }
    let fin = (cierra + 2..hasta).find(|&j| l[j] == ')')?;
    let url: String = l[cierra + 2..fin].iter().collect();
    Some((abre + 1, cierra, fin + 1, url.trim().to_string()))
}

// ---------------------------------------------------------------------------
// Lo que hace el teclado: las cuentas de `Vivo.kt` y `Formato.kt`.

/// Que hacer al pulsar Intro en un renglon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Continuar {
    /// Nada especial: un salto normal.
    Nada,
    /// Seguir la lista: el renglon nuevo empieza con esto.
    Con(String),
    /// El renglon solo tenia la marca: se quita y se sale de la lista, como
    /// en el movil (`Vivo.partir` sobre un bloque vacio).
    Cortar,
}

/// La marca de bloque con la que sigue el renglon de debajo.
pub fn continuar(renglon: &str) -> Continuar {
    let l: Vec<char> = renglon.chars().collect();
    let sangria: String = l.iter().take_while(|c| **c == ' ' || **c == '\t').collect();
    let (marca, resto): (String, usize) = if let Some(k) = vineta(&l) {
        match casilla(&l, k + 2) {
            // La casilla de debajo nace sin marcar aunque esta lo este.
            Some(_) => (format!("{sangria}{} [ ] ", l[k]), (k + 6).min(l.len())),
            None => (format!("{sangria}{} ", l[k]), k + 2),
        }
    } else if let Some((k, fin)) = numero(&l) {
        let n: u64 = l[k..fin - 1].iter().collect::<String>().parse().unwrap_or(0);
        (format!("{sangria}{}{} ", n + 1, l[fin - 1]), fin + 1)
    } else if l.first() == Some(&'>') {
        ("> ".to_string(), if l.get(1) == Some(&' ') { 2 } else { 1 })
    } else {
        return Continuar::Nada;
    };
    if l[resto.min(l.len())..].iter().all(|c| c.is_whitespace()) {
        Continuar::Cortar
    } else {
        Continuar::Con(marca)
    }
}

/// Cuanto mide la marca de bloque de un renglon (titulo, cita, lista,
/// casilla o numero), en letras. 0 si no tiene.
fn marca_de_bloque(l: &[char]) -> usize {
    let almohadillas = l.iter().take_while(|c| **c == '#').count();
    if (1..=6).contains(&almohadillas) && l.get(almohadillas) == Some(&' ') {
        return almohadillas + 1;
    }
    if l.first() == Some(&'>') {
        return if l.get(1) == Some(&' ') { 2 } else { 1 };
    }
    if let Some(k) = vineta(l) {
        return match casilla(l, k + 2) {
            Some(_) => (k + 6).min(l.len()),
            None => k + 2,
        };
    }
    if let Some((_, fin)) = numero(l) {
        return fin + 1;
    }
    0
}

/// Pone a un renglon la marca de bloque `prefijo` (`"# "`, `"- [ ] "`,
/// `"> "`…) quitando la que tuviera. Si ya tenia esa misma, se la quita:
/// pulsar dos veces «Titulo» vuelve a parrafo, como los botones del movil.
///
/// Devuelve el texto entero nuevo y cuanto se corrio el renglon (en UTF-16,
/// puede ser negativo), para mover el cursor.
pub fn con_prefijo(texto: &str, linea: usize, prefijo: &str) -> Option<(String, isize)> {
    let mut renglones: Vec<&str> = texto.split('\n').collect();
    let r = *renglones.get(linea)?;
    let l: Vec<char> = r.chars().collect();
    let m = marca_de_bloque(&l);
    let puesta: String = l[..m].iter().collect();
    let cuerpo: String = l[m..].iter().collect();
    let nuevo = if puesta == prefijo {
        cuerpo
    } else {
        format!("{prefijo}{cuerpo}")
    };
    let corrido = utf16(&nuevo) as isize - utf16(r) as isize;
    renglones[linea] = &nuevo;
    Some((renglones.join("\n"), corrido))
}

fn utf16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Marca o desmarca la casilla del renglon `linea`. `None` si no tiene.
pub fn alternar_casilla(texto: &str, linea: usize) -> Option<String> {
    let mut renglones: Vec<String> = texto.split('\n').map(str::to_string).collect();
    let l: Vec<char> = renglones.get(linea)?.chars().collect();
    let k = vineta(&l)?;
    let hecha = casilla(&l, k + 2)?;
    let mut nueva = l.clone();
    nueva[k + 3] = if hecha { ' ' } else { 'x' };
    renglones[linea] = nueva.into_iter().collect();
    Some(renglones.join("\n"))
}

/// Envuelve lo elegido (`desde..hasta`, UTF-16) con `marca` por los dos
/// lados, o se la quita si ya la tenia justo por fuera: es el
/// `toggleStyleForSelection` de Telegram que copio el movil.
///
/// Devuelve el texto nuevo y lo elegido despues, para volver a elegirlo.
pub fn envolver(texto: &str, desde: usize, hasta: usize, marca: &str) -> (String, usize, usize) {
    let u: Vec<u16> = texto.encode_utf16().collect();
    let m: Vec<u16> = marca.encode_utf16().collect();
    let (desde, hasta) = (desde.min(u.len()), hasta.min(u.len()));
    let (desde, hasta) = (desde.min(hasta), desde.max(hasta));
    let k = m.len();
    let ya = desde >= k && hasta + k <= u.len() && u[desde - k..desde] == m[..] && u[hasta..hasta + k] == m[..];
    let mut sal: Vec<u16> = Vec::with_capacity(u.len() + 2 * k);
    if ya {
        sal.extend_from_slice(&u[..desde - k]);
        sal.extend_from_slice(&u[desde..hasta]);
        sal.extend_from_slice(&u[hasta + k..]);
        (String::from_utf16_lossy(&sal), desde - k, hasta - k)
    } else {
        sal.extend_from_slice(&u[..desde]);
        sal.extend_from_slice(&m);
        sal.extend_from_slice(&u[desde..hasta]);
        sal.extend_from_slice(&m);
        sal.extend_from_slice(&u[hasta..]);
        (String::from_utf16_lossy(&sal), desde + k, hasta + k)
    }
}

/// Hace un enlace con lo elegido: `[lo elegido](url)`. Deja elegida la
/// direccion si hay que escribirla (no la habia en el portapapeles) y el
/// cursor detras del enlace si ya esta puesta, como el dialogo de enlace
/// del movil con su boton de pegar.
pub fn poner_enlace(texto: &str, desde: usize, hasta: usize, url: &str) -> (String, usize, usize) {
    let u: Vec<u16> = texto.encode_utf16().collect();
    let (desde, hasta) = (desde.min(hasta).min(u.len()), hasta.max(desde).min(u.len()));
    let dentro = String::from_utf16_lossy(&u[desde..hasta]);
    let puesto = format!("[{dentro}]({url})");
    let nuevo = format!(
        "{}{puesto}{}",
        String::from_utf16_lossy(&u[..desde]),
        String::from_utf16_lossy(&u[hasta..])
    );
    let url_desde = desde + utf16(&dentro) + 3;
    if url.is_empty() {
        (nuevo, url_desde, url_desde)
    } else {
        let fin = desde + utf16(&puesto);
        (nuevo, fin, fin)
    }
}

/// Mete lo elegido en un bloque de codigo con sus vallas, cada una en su
/// renglon. Sin nada elegido deja el cursor dentro del bloque vacio.
pub fn bloque_de_codigo(texto: &str, desde: usize, hasta: usize) -> (String, usize, usize) {
    let u: Vec<u16> = texto.encode_utf16().collect();
    let (desde, hasta) = (desde.min(hasta).min(u.len()), hasta.max(desde).min(u.len()));
    let antes = String::from_utf16_lossy(&u[..desde]);
    let dentro = String::from_utf16_lossy(&u[desde..hasta]);
    let despues = String::from_utf16_lossy(&u[hasta..]);
    // La valla va en su propio renglon: si no se empieza a principio de uno,
    // se abre uno nuevo.
    let salto_antes = if antes.is_empty() || antes.ends_with('\n') { "" } else { "\n" };
    let salto_despues = if despues.is_empty() || despues.starts_with('\n') { "" } else { "\n" };
    let abre = format!("{salto_antes}```\n");
    let nuevo = format!("{antes}{abre}{dentro}\n```{salto_despues}{despues}");
    let a = desde + utf16(&abre);
    (nuevo, a, a + utf16(&dentro))
}

/// El enlace que hay en esa posicion, si hay uno: lo que abre Ctrl+clic.
pub fn enlace_en(texto: &str, pos: usize) -> Option<String> {
    analizar(texto)
        .into_iter()
        .find(|t| t.estilo == Estilo::Enlace && t.desde <= pos && pos <= t.hasta)
        .and_then(|t| t.url)
}

/// El renglon cuya casilla esta en esa posicion: lo que alterna un clic.
pub fn casilla_en(texto: &str, pos: usize) -> Option<usize> {
    analizar(texto)
        .into_iter()
        .find(|t| matches!(t.estilo, Estilo::Casilla { .. }) && t.desde <= pos && pos < t.hasta)
        .map(|t| t.linea)
}

/// El nombre de una nota: su primer renglon con algo, sin marcas y con
/// cuarenta letras como mucho (`aUnProyecto` del movil).
pub fn titulo(texto: &str) -> String {
    let tramos = analizar(texto);
    let ls = lineas(texto);
    let u: Vec<u16> = texto.encode_utf16().collect();
    for (n, l) in ls.iter().enumerate() {
        let visible: Vec<u16> = (l.desde..l.hasta)
            .filter(|p| {
                !tramos
                    .iter()
                    .any(|t| t.linea == n && t.estilo == Estilo::Marca && t.desde <= *p && *p < t.hasta)
            })
            .map(|p| u[p])
            .collect();
        let s = String::from_utf16_lossy(&visible);
        let s = s.trim();
        if !s.is_empty() {
            return s.chars().take(40).collect();
        }
    }
    String::new()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Los tramos como (texto que cubren, estilo): se leen mejor que numeros.
    fn vistos(texto: &str) -> Vec<(String, Estilo)> {
        let u: Vec<u16> = texto.encode_utf16().collect();
        analizar(texto)
            .into_iter()
            .map(|t| (String::from_utf16_lossy(&u[t.desde..t.hasta]), t.estilo))
            .collect()
    }

    fn tiene(texto: &str, trozo: &str, estilo: Estilo) -> bool {
        vistos(texto).iter().any(|(t, e)| t == trozo && *e == estilo)
    }

    #[test]
    fn un_titulo_esconde_su_almohadilla_y_agranda_el_resto() {
        assert!(tiene("## Planta baja", "## ", Estilo::Marca));
        assert!(tiene("## Planta baja", "Planta baja", Estilo::Titulo(2)));
    }

    #[test]
    fn siete_almohadillas_o_sin_espacio_no_son_titulo() {
        assert!(vistos("####### no").is_empty());
        assert!(vistos("#etiqueta").is_empty());
    }

    #[test]
    fn negrita_cursiva_y_tachado_con_sus_marcas() {
        let t = "hay **mucho** y *poco* y ~~nada~~";
        assert!(tiene(t, "mucho", Estilo::Negrita));
        assert!(tiene(t, "poco", Estilo::Cursiva));
        assert!(tiene(t, "nada", Estilo::Tachado));
        assert_eq!(vistos(t).iter().filter(|(_, e)| *e == Estilo::Marca).count(), 6);
    }

    #[test]
    fn tres_asteriscos_son_cursiva_dentro_de_negrita() {
        let t = "***fuerte***";
        assert!(tiene(t, "*fuerte*", Estilo::Negrita));
        assert!(tiene(t, "fuerte", Estilo::Cursiva));
    }

    #[test]
    fn un_guion_bajo_dentro_de_una_palabra_no_es_cursiva() {
        assert!(vistos("plano_de_obra_final").is_empty());
        assert!(tiene("un _poco_ mas", "poco", Estilo::Cursiva));
    }

    #[test]
    fn un_asterisco_suelto_o_con_espacio_no_abre_nada() {
        assert!(vistos("3 * 4 = 12").is_empty());
        assert!(vistos("sin cerrar *aqui").is_empty());
    }

    #[test]
    fn el_codigo_no_se_mira_por_dentro() {
        let t = "usa `a*b*c` asi";
        assert!(tiene(t, "a*b*c", Estilo::Codigo));
        assert!(!vistos(t).iter().any(|(_, e)| *e == Estilo::Cursiva));
    }

    #[test]
    fn un_bloque_de_codigo_va_entre_vallas_y_no_se_formatea() {
        let t = "```\n# no es titulo\n**ni negrita**\n```\n# ya si";
        assert!(tiene(t, "# no es titulo", Estilo::BloqueCodigo));
        assert!(tiene(t, "**ni negrita**", Estilo::BloqueCodigo));
        assert!(tiene(t, "ya si", Estilo::Titulo(1)));
        assert!(!vistos(t).iter().any(|(_, e)| *e == Estilo::Negrita));
    }

    #[test]
    fn un_enlace_ensena_su_texto_y_guarda_la_direccion() {
        let t = "ver [el plano](https://x.es/p) hoy";
        assert!(tiene(t, "el plano", Estilo::Enlace));
        assert!(tiene(t, "[", Estilo::Marca));
        assert!(tiene(t, "](https://x.es/p)", Estilo::Marca));
        assert_eq!(enlace_en(t, 6).as_deref(), Some("https://x.es/p"));
        assert_eq!(enlace_en(t, 0), None);
    }

    #[test]
    fn corchetes_sin_parentesis_no_son_enlace() {
        assert!(vistos("[nota] sin enlace").is_empty());
    }

    #[test]
    fn las_casillas_se_ven_y_la_hecha_tacha_lo_suyo() {
        let t = "- [ ] comprar cemento\n- [x] pedir grua";
        assert!(tiene(t, "[ ]", Estilo::Casilla { hecha: false }));
        assert!(tiene(t, "[x]", Estilo::Casilla { hecha: true }));
        assert!(tiene(t, "pedir grua", Estilo::Hecha));
        assert!(!tiene(t, "comprar cemento", Estilo::Hecha));
        assert!(tiene(t, "- ", Estilo::Marca));
    }

    #[test]
    fn una_vineta_esconde_el_guion_y_una_lista_numerada_no() {
        assert!(tiene("- uno", "- uno", Estilo::Vineta));
        assert!(tiene("- uno", "- ", Estilo::Marca));
        assert!(tiene("12. doce", "12.", Estilo::Numero));
        assert!(!vistos("12. doce").iter().any(|(_, e)| *e == Estilo::Marca));
    }

    #[test]
    fn una_cita_y_una_raya() {
        assert!(tiene("> dijo el jefe", "> dijo el jefe", Estilo::Cita));
        assert!(tiene("> dijo el jefe", "> ", Estilo::Marca));
        assert!(tiene("---", "---", Estilo::Regla));
        assert!(!tiene("-- no", "-- no", Estilo::Regla));
    }

    #[test]
    fn una_formula_va_entre_dolares_pero_un_precio_no() {
        assert!(tiene("area $\\pi r^2$ m2", "\\pi r^2", Estilo::Formula));
        assert!(vistos("cuesta $5 y $6").is_empty());
    }

    #[test]
    fn las_posiciones_van_en_utf16_como_el_richedit() {
        // El emoji ocupa dos: la negrita empieza en 3, no en 2.
        let t = "😀 **si**";
        let n = analizar(t)
            .into_iter()
            .find(|t| t.estilo == Estilo::Negrita)
            .unwrap();
        assert_eq!((n.desde, n.hasta), (5, 7));
    }

    #[test]
    fn el_retorno_del_richedit_cuenta_como_salto() {
        let a = analizar("# uno\n**dos**");
        let b = analizar("# uno\r**dos**");
        assert_eq!(a, b);
        assert_eq!(a.last().unwrap().linea, 1);
    }

    #[test]
    fn los_renglones_y_el_del_cursor() {
        let ls = lineas("ab\n\ncd");
        assert_eq!(ls.len(), 3);
        assert_eq!(linea_de(&ls, 2), 0);
        assert_eq!(linea_de(&ls, 3), 1);
        assert_eq!(linea_de(&ls, 6), 2);
        assert_eq!(lineas("").len(), 1);
    }

    #[test]
    fn intro_sigue_la_lista_y_la_casilla_nace_sin_marcar() {
        assert_eq!(continuar("- uno"), Continuar::Con("- ".into()));
        assert_eq!(continuar("  * dos"), Continuar::Con("  * ".into()));
        assert_eq!(continuar("- [x] hecho"), Continuar::Con("- [ ] ".into()));
        assert_eq!(continuar("9. nueve"), Continuar::Con("10. ".into()));
        assert_eq!(continuar("> cita"), Continuar::Con("> ".into()));
    }

    #[test]
    fn intro_en_un_renglon_de_lista_vacio_sale_de_la_lista() {
        assert_eq!(continuar("- "), Continuar::Cortar);
        assert_eq!(continuar("- [ ] "), Continuar::Cortar);
        assert_eq!(continuar("3. "), Continuar::Cortar);
        assert_eq!(continuar("un parrafo"), Continuar::Nada);
    }

    #[test]
    fn alternar_una_casilla_la_marca_y_la_desmarca() {
        let t = "a\n- [ ] b";
        let u = alternar_casilla(t, 1).unwrap();
        assert_eq!(u, "a\n- [x] b");
        assert_eq!(alternar_casilla(&u, 1).unwrap(), t);
        assert_eq!(alternar_casilla(t, 0), None);
        assert_eq!(casilla_en(t, 5), Some(1));
        assert_eq!(casilla_en(t, 1), None);
    }

    #[test]
    fn envolver_pone_la_marca_y_la_segunda_vez_la_quita() {
        let (t, a, b) = envolver("hola mundo", 5, 10, "**");
        assert_eq!(t, "hola **mundo**");
        assert_eq!((a, b), (7, 12));
        let (t2, a2, b2) = envolver(&t, a, b, "**");
        assert_eq!(t2, "hola mundo");
        assert_eq!((a2, b2), (5, 10));
    }

    #[test]
    fn un_prefijo_cambia_el_tipo_del_renglon_y_repetido_lo_quita() {
        let (t, d) = con_prefijo("uno\ndos", 1, "# ").unwrap();
        assert_eq!(t, "uno\n# dos");
        assert_eq!(d, 2);
        let (t, d) = con_prefijo(&t, 1, "- [ ] ").unwrap();
        assert_eq!(t, "uno\n- [ ] dos");
        assert_eq!(d, 4);
        let (t, _) = con_prefijo(&t, 1, "- [ ] ").unwrap();
        assert_eq!(t, "uno\ndos");
        assert!(con_prefijo("uno", 5, "# ").is_none());
    }

    #[test]
    fn un_enlace_con_la_direccion_copiada_deja_el_cursor_detras() {
        let (t, a, b) = poner_enlace("ver plano hoy", 4, 9, "https://x.es");
        assert_eq!(t, "ver [plano](https://x.es) hoy");
        assert_eq!((a, b), (25, 25));
    }

    #[test]
    fn un_enlace_sin_direccion_deja_el_cursor_donde_escribirla() {
        let (t, a, b) = poner_enlace("ver plano", 4, 9, "");
        assert_eq!(t, "ver [plano]()");
        assert_eq!((a, b), (12, 12));
        assert_eq!(&t[a..], ")");
    }

    #[test]
    fn un_bloque_de_codigo_pone_sus_vallas_en_renglones_propios() {
        let (t, a, b) = bloque_de_codigo("mira x=1 ya", 5, 8);
        assert_eq!(t, "mira \n```\nx=1\n```\n ya");
        assert_eq!(&t[a..b], "x=1");
        let (t, a, b) = bloque_de_codigo("", 0, 0);
        assert_eq!(t, "```\n\n```");
        assert_eq!((a, b), (4, 4));
    }

    #[test]
    fn el_titulo_de_una_nota_es_su_primer_renglon_sin_marcas() {
        assert_eq!(titulo("\n\n## **Obra** de Juan\nresto"), "Obra de Juan");
        assert_eq!(titulo(""), "");
        assert_eq!(titulo(&"a".repeat(80)).chars().count(), 40);
    }
}
