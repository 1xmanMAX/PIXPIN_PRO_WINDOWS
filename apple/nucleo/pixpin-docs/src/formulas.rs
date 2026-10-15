//! **Las formulas de las notas, entendidas** (13-oct-2026): el `object
//! Formulas` de Android (`motormd/Formulas.kt`), portado tal cual, con los
//! casos de su `FormulasTest.kt`.
//!
//! Una formula entre dolares (`$x^2$`) se lee a un arbol ([`Pieza`]) para
//! **componerla**: una fraccion con su raya, un exponente pequeño y subido,
//! una raiz con su tejado. Entiende el LaTeX de unas notas (`\frac`,
//! `\sqrt[3]{}`, griegas, operadores) y tambien **la escritura corriente**:
//! `1/2` es una fraccion, `sqrt(x)` o `raiz(x)` una raiz, `pi` es π, `<=` es
//! ≤. Lo que no entiende **se enseña tal cual**: antes texto de mas que
//! texto perdido.
//!
//! No es el interprete de la grafica del lienzo (`pixpin-motor2d::formula`,
//! que evalua): este solo sabe escribir.
//!
//! El editor de antes (el `RichEdit`) no puede apilar una fraccion: de aqui
//! saca [`toques`] — que subir, que bajar y que marcas esconder — para que
//! fuera del renglon del cursor `x^2` se vea x² y `a_1` se vea a₁.

/// Un trozo de formula ya entendido, listo para componer.
#[derive(Debug, Clone, PartialEq)]
pub enum Pieza {
    /// Letras, numeros y simbolos sueltos.
    Texto { texto: String, cursiva: bool },
    /// Varios seguidos, en la misma linea.
    Fila(Vec<Pieza>),
    /// Uno encima de otro, con su raya.
    Fraccion(Box<Pieza>, Box<Pieza>),
    /// Lo de arriba y lo de abajo pegados a una base: `x^2_i`.
    ConIndices { base: Box<Pieza>, arriba: Option<Box<Pieza>>, abajo: Option<Box<Pieza>> },
    /// Una raiz, con su indice si lo lleva.
    Raiz { dentro: Box<Pieza>, indice: Option<Box<Pieza>> },
    /// Entre parentesis que crecen con lo de dentro.
    Agrupado { dentro: Box<Pieza>, abre: String, cierra: String },
}

fn texto(t: &str) -> Pieza {
    Pieza::Texto { texto: t.to_string(), cursiva: false }
}

/// Los simbolos que se escriben con barra invertida.
const SIMBOLOS: &[(&str, &str)] = &[
    ("alpha", "α"), ("beta", "β"), ("gamma", "γ"), ("delta", "δ"),
    ("epsilon", "ε"), ("zeta", "ζ"), ("eta", "η"), ("theta", "θ"),
    ("iota", "ι"), ("kappa", "κ"), ("lambda", "λ"), ("mu", "μ"),
    ("nu", "ν"), ("xi", "ξ"), ("pi", "π"), ("rho", "ρ"),
    ("sigma", "σ"), ("tau", "τ"), ("upsilon", "υ"), ("phi", "φ"),
    ("chi", "χ"), ("psi", "ψ"), ("omega", "ω"),
    ("Gamma", "Γ"), ("Delta", "Δ"), ("Theta", "Θ"), ("Lambda", "Λ"),
    ("Xi", "Ξ"), ("Pi", "Π"), ("Sigma", "Σ"), ("Phi", "Φ"),
    ("Psi", "Ψ"), ("Omega", "Ω"),
    ("times", "×"), ("div", "÷"), ("pm", "±"), ("mp", "∓"),
    ("cdot", "·"), ("ast", "∗"), ("star", "⋆"),
    ("leq", "≤"), ("le", "≤"), ("geq", "≥"), ("ge", "≥"),
    ("neq", "≠"), ("ne", "≠"), ("approx", "≈"), ("equiv", "≡"),
    ("sim", "∼"), ("propto", "∝"),
    ("infty", "∞"), ("partial", "∂"), ("nabla", "∇"),
    ("sum", "∑"), ("prod", "∏"), ("int", "∫"), ("oint", "∮"),
    ("sqrt", "√"), ("angle", "∠"), ("degree", "°"),
    ("in", "∈"), ("notin", "∉"), ("subset", "⊂"), ("supset", "⊃"),
    ("cup", "∪"), ("cap", "∩"), ("emptyset", "∅"), ("forall", "∀"),
    ("exists", "∃"), ("neg", "¬"), ("land", "∧"), ("lor", "∨"),
    ("rightarrow", "→"), ("to", "→"), ("leftarrow", "←"),
    ("leftrightarrow", "↔"), ("Rightarrow", "⇒"), ("Leftarrow", "⇐"),
    ("ldots", "…"), ("cdots", "⋯"), ("perp", "⊥"), ("parallel", "∥"),
];

/// Las palabras que van derechas y no en cursiva.
const FUNCIONES: &[&str] = &[
    "sin", "cos", "tan", "cot", "sec", "csc", "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh", "log", "ln", "exp", "lim", "max", "min",
    "det", "gcd", "mod",
];

/// Lo que se escribe de forma corriente y significa un simbolo.
const PALABRAS: &[(&str, &str)] = &[
    ("inf", "∞"), ("infinito", "∞"), ("infinity", "∞"), ("grados", "°"), ("deg", "°"), ("sum", "∑"), ("suma", "∑"), ("prod", "∏"),
    ("int", "∫"), ("raiz", "√"),
];

/// Parejas de signos que se escriben con dos teclas y significan una.
const PAREJAS: &[(&str, &str)] = &[
    ("<=", "≤"), (">=", "≥"), ("!=", "≠"), ("/=", "≠"), ("+-", "±"), ("-+", "∓"), ("==", "≡"), ("~=", "≈"), ("->", "→"), ("=>", "⇒"),
    ("<-", "←"), ("<->", "↔"),
];

fn simbolo(p: &str) -> Option<&'static str> {
    SIMBOLOS.iter().find(|(k, _)| *k == p).map(|(_, v)| *v)
}

/// **Lee una formula** a su arbol. Nunca falla: lo raro sale tal cual.
pub fn leer(formula: &str) -> Pieza {
    Lector { s: formula.chars().collect(), i: 0 }.expresion(None)
}

struct Lector {
    s: Vec<char>,
    i: usize,
}

impl Lector {
    fn en(&self, k: usize) -> Option<char> {
        self.s.get(k).copied()
    }

    fn es_fin(&self, fin: Option<char>) -> bool {
        self.i >= self.s.len() || (fin.is_some() && self.en(self.i) == fin)
    }

    /// Sumas, restas y comparaciones: el nivel mas suelto.
    fn expresion(&mut self, fin: Option<char>) -> Pieza {
        let mut partes = vec![self.termino(fin)];
        loop {
            // Los espacios se saltan antes de mirar el signo (`1/2 + x`).
            self.saltar_espacios();
            if self.es_fin(fin) {
                break;
            }
            let Some(signo) = self.signo_suelto() else { break };
            partes.push(texto(&format!(" {signo} ")));
            partes.push(self.termino(fin));
        }
        if partes.len() == 1 { partes.pop().unwrap() } else { Pieza::Fila(partes) }
    }

    fn saltar_espacios(&mut self) {
        while self.en(self.i) == Some(' ') {
            self.i += 1;
        }
    }

    fn empieza(&self, t: &str) -> bool {
        let c: Vec<char> = t.chars().collect();
        self.s.len() >= self.i + c.len() && self.s[self.i..self.i + c.len()] == c[..]
    }

    fn signo_suelto(&mut self) -> Option<String> {
        for (escrito, simbolo) in PAREJAS {
            if self.empieza(escrito) {
                self.i += escrito.chars().count();
                return Some(simbolo.to_string());
            }
        }
        let c = self.en(self.i)?;
        if "+-=<>≤≥≠≈".contains(c) {
            self.i += 1;
            return Some(c.to_string());
        }
        None
    }

    /// Lo que se multiplica y se divide. **La barra hace una fraccion**.
    fn termino(&mut self, fin: Option<char>) -> Pieza {
        let mut izquierda = self.factor(fin);
        loop {
            let antes = self.i;
            self.saltar_espacios();
            if self.es_fin(fin) {
                self.i = antes;
                break;
            }
            let c = self.s[self.i];
            if c == '/' {
                self.i += 1;
                izquierda = Pieza::Fraccion(Box::new(izquierda), Box::new(self.factor(fin)));
            } else if c == '*' {
                self.i += 1;
                let d = self.factor(fin);
                izquierda = Pieza::Fila(vec![izquierda, texto("·"), d]);
            } else if es_de_factor(c) {
                // Pegado es multiplicado: `2x`, sin signo, como en un libro.
                let d = self.factor(fin);
                izquierda = Pieza::Fila(vec![izquierda, d]);
            } else {
                self.i = antes;
                break;
            }
        }
        izquierda
    }

    /// Un factor con sus exponentes y subindices pegados.
    fn factor(&mut self, fin: Option<char>) -> Pieza {
        let mut base = self.atomo(fin);
        while let Some(c @ ('^' | '_')) = self.en(self.i) {
            self.i += 1;
            let indice = self.atomo(fin);
            base = con_indice(base, indice, c == '^');
        }
        base
    }

    /// Lo mas pequeño: un numero, una letra, un parentesis, un mandato.
    fn atomo(&mut self, fin: Option<char>) -> Pieza {
        self.saltar_espacios();
        if self.es_fin(fin) {
            return texto("");
        }
        let c = self.s[self.i];
        match c {
            '\\' => self.mandato(),
            '(' | '[' => self.agrupado(c),
            '{' => {
                self.i += 1;
                let d = self.expresion(Some('}'));
                if self.i < self.s.len() {
                    self.i += 1;
                }
                d
            }
            _ if c.is_ascii_digit() || c == '.' => self.numero(),
            _ if c.is_alphabetic() => self.palabra(),
            _ => {
                self.i += 1;
                texto(&c.to_string())
            }
        }
    }

    fn numero(&mut self) -> Pieza {
        let mut n = String::new();
        while let Some(c) = self.en(self.i).filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',') {
            n.push(c);
            self.i += 1;
        }
        texto(&n)
    }

    /// `pi` sale π sin barra, `sqrt(x)` y `raiz(x)` hacen raiz, `sin(x)` sale
    /// derecho, y `xy` son dos letras que se multiplican.
    fn palabra(&mut self) -> Pieza {
        let desde = self.i;
        let mut w = String::new();
        while let Some(c) = self.en(self.i).filter(|c| c.is_alphabetic()) {
            w.push(c);
            self.i += 1;
        }
        let minus = w.to_lowercase();
        if w == "sqrt" || minus == "raiz" || minus == "raíz" {
            return Pieza::Raiz { dentro: Box::new(self.argumento()), indice: None };
        }
        if FUNCIONES.contains(&minus.as_str()) {
            let a = self.argumento();
            return Pieza::Fila(vec![texto(&minus), a]);
        }
        if let Some(s) = simbolo(&w) {
            return texto(s);
        }
        if let Some((_, s)) = PALABRAS.iter().find(|(k, _)| *k == minus) {
            return texto(s);
        }
        // Letras sueltas: cada una es una variable.
        self.i = desde + 1;
        Pieza::Texto { texto: w.chars().next().unwrap_or(' ').to_string(), cursiva: true }
    }

    /// Lo que va detras de una funcion o de una raiz.
    fn argumento(&mut self) -> Pieza {
        self.saltar_espacios();
        if let Some(c @ ('(' | '{')) = self.en(self.i) {
            let cierra = if c == '(' { ')' } else { '}' };
            self.i += 1;
            let dentro = self.expresion(Some(cierra));
            if self.i < self.s.len() {
                self.i += 1;
            }
            return dentro;
        }
        self.atomo(None)
    }

    /// Un grupo entre llaves, o el siguiente caracter suelto.
    fn grupo(&mut self) -> Pieza {
        let Some(c) = self.en(self.i) else { return texto("") };
        if c == '{' {
            self.i += 1;
            let dentro = self.expresion(Some('}'));
            if self.i < self.s.len() {
                self.i += 1;
            }
            return dentro;
        }
        if c == '\\' {
            return self.mandato();
        }
        self.i += 1;
        Pieza::Texto { texto: c.to_string(), cursiva: c.is_alphabetic() }
    }

    fn mandato(&mut self) -> Pieza {
        self.i += 1; // la barra
        let mut palabra = String::new();
        while let Some(c) = self.en(self.i).filter(|c| c.is_alphabetic()) {
            palabra.push(c);
            self.i += 1;
        }
        match palabra.as_str() {
            "frac" | "dfrac" | "tfrac" => {
                let a = self.grupo();
                let b = self.grupo();
                Pieza::Fraccion(Box::new(a), Box::new(b))
            }
            "sqrt" => {
                let mut indice = None;
                if self.en(self.i) == Some('[') {
                    self.i += 1;
                    indice = Some(Box::new(self.expresion(Some(']'))));
                    if self.i < self.s.len() {
                        self.i += 1;
                    }
                }
                Pieza::Raiz { dentro: Box::new(self.grupo()), indice }
            }
            "left" | "right" => texto(""),
            p if FUNCIONES.contains(&p) => texto(p),
            p if simbolo(p).is_some() => texto(simbolo(p).unwrap_or_default()),
            "" => {
                // `\{`, `\,`...: el caracter de detras, tal cual.
                let c = self.en(self.i).map(|c| c.to_string()).unwrap_or_default();
                self.i += 1;
                texto(&c)
            }
            // Ni idea: se enseña con su barra.
            p => texto(&format!("\\{p}")),
        }
    }

    fn agrupado(&mut self, abre: char) -> Pieza {
        let cierra = if abre == '(' { ')' } else { ']' };
        let desde = self.i;
        self.i += 1;
        let dentro = self.expresion(Some(cierra));
        // Sin cierre no hay grupo: se devuelve tal cual para no perder nada.
        if self.en(self.i) == Some(cierra) {
            self.i += 1;
            Pieza::Agrupado { dentro: Box::new(dentro), abre: abre.to_string(), cierra: cierra.to_string() }
        } else {
            self.i = desde + 1;
            texto(&abre.to_string())
        }
    }
}

fn es_de_factor(c: char) -> bool {
    c.is_alphanumeric() || c == '(' || c == '{' || c == '\\' || c == '.'
}

/// Junta indices sobre la misma base: `x^2_i` es una sola cosa.
fn con_indice(base: Pieza, indice: Pieza, arriba: bool) -> Pieza {
    match base {
        Pieza::ConIndices { base, arriba: a, abajo: b } => {
            if arriba {
                Pieza::ConIndices { base, arriba: Some(Box::new(indice)), abajo: b }
            } else {
                Pieza::ConIndices { base, arriba: a, abajo: Some(Box::new(indice)) }
            }
        }
        otra => {
            if arriba {
                Pieza::ConIndices { base: Box::new(otra), arriba: Some(Box::new(indice)), abajo: None }
            } else {
                Pieza::ConIndices { base: Box::new(otra), arriba: None, abajo: Some(Box::new(indice)) }
            }
        }
    }
}

/// El arbol en texto plano (el de `FormulasTest`): `(a/b)`, `x^2_i`,
/// `sqrt(x)`. Para pruebas y para leer en voz alta o copiar.
pub fn plano(p: &Pieza) -> String {
    match p {
        Pieza::Texto { texto, .. } => texto.clone(),
        Pieza::Fila(v) => v.iter().map(plano).collect(),
        Pieza::Fraccion(a, b) => format!("({}/{})", plano(a), plano(b)),
        Pieza::ConIndices { base, arriba, abajo } => {
            let mut s = plano(base);
            if let Some(a) = arriba {
                s.push('^');
                s.push_str(&plano(a));
            }
            if let Some(b) = abajo {
                s.push('_');
                s.push_str(&plano(b));
            }
            s
        }
        Pieza::Raiz { dentro, .. } => format!("sqrt({})", plano(dentro)),
        Pieza::Agrupado { dentro, abre, cierra } => format!("{abre}{}{cierra}", plano(dentro)),
    }
}

/// Lo que el `RichEdit` hace con un trozo de la formula.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toque {
    /// Pequeño y subido (exponente, numerador de una `a/b`).
    Arriba,
    /// Pequeño y bajado (subindice, denominador).
    Abajo,
    /// Una marca que no se ve (`^`, `_`, las llaves de un grupo).
    Oculto,
}

/// **Los toques de una formula para el editor de antes**: posiciones en
/// UTF-16 dentro del texto de la formula (sin sus dolares). Exponentes y
/// subindices suben o bajan con sus marcas escondidas; una fraccion
/// sencilla `a/b` queda como ᵃ/ᵦ. Lo que no se sabe tocar se deja.
pub fn toques(formula: &str) -> Vec<(usize, usize, Toque)> {
    let c: Vec<char> = formula.chars().collect();
    // Posicion UTF-16 de cada caracter.
    let mut u = Vec::with_capacity(c.len() + 1);
    let mut n = 0;
    for ch in &c {
        u.push(n);
        n += ch.len_utf16();
    }
    u.push(n);
    let mut v = Vec::new();
    // Lo que va en un indice: un grupo entre llaves o un caracter.
    let indice = |i: usize| -> Option<(usize, usize, bool)> {
        match c.get(i)? {
            '{' => {
                let mut prof = 0;
                for (j, ch) in c.iter().enumerate().skip(i) {
                    match ch {
                        '{' => prof += 1,
                        '}' => {
                            prof -= 1;
                            if prof == 0 {
                                return Some((i, j + 1, true));
                            }
                        }
                        _ => {}
                    }
                }
                None
            }
            ch if ch.is_alphanumeric() => {
                // Un numero entero de golpe (`x^10` no, ese pide llaves: uno solo, como LaTeX).
                Some((i, i + 1, false))
            }
            _ => None,
        }
    };
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            m @ ('^' | '_') => {
                let toque = if m == '^' { Toque::Arriba } else { Toque::Abajo };
                if let Some((a, b, llaves)) = indice(i + 1) {
                    v.push((u[i], u[i + 1], Toque::Oculto));
                    if llaves {
                        v.push((u[a], u[a + 1], Toque::Oculto));
                        v.push((u[a + 1], u[b - 1], toque));
                        v.push((u[b - 1], u[b], Toque::Oculto));
                    } else {
                        v.push((u[a], u[b], toque));
                    }
                    i = b;
                    continue;
                }
                i += 1;
            }
            '/' => {
                // `a/b` con un trozo de letras o cifras a cada lado, pegado.
                let es = |ch: char| ch.is_alphanumeric() || ch == '.';
                let mut a = i;
                while a > 0 && es(c[a - 1]) {
                    a -= 1;
                }
                let mut b = i + 1;
                while b < c.len() && es(c[b]) {
                    b += 1;
                }
                if a < i && b > i + 1 && !v.iter().any(|(x, y, _)| *x < u[i] && *y > u[a]) {
                    v.push((u[a], u[i], Toque::Arriba));
                    v.push((u[i + 1], u[b], Toque::Abajo));
                    i = b;
                    continue;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    v
}

#[cfg(test)]
mod pruebas {
    //! Los casos de `FormulasTest.kt` de Android, uno a uno.
    use super::*;

    /// El arbol en texto plano, sin los espacios que pone el compositor.
    fn t(p: &Pieza) -> String {
        plano(p).replace(' ', "")
    }

    fn l(s: &str) -> Pieza {
        leer(s)
    }

    #[test]
    fn un_exponente_se_cuelga_de_su_base() {
        let p = l("x^2");
        assert!(matches!(p, Pieza::ConIndices { .. }), "salio {p:?}");
        assert_eq!(t(&p), "x^2");
    }

    #[test]
    fn el_exponente_entre_llaves_puede_ser_largo() {
        assert_eq!(t(&l("x^{n+1}")), "x^n+1");
    }

    #[test]
    fn arriba_y_abajo_a_la_vez_sobre_la_misma_base() {
        let Pieza::ConIndices { arriba, abajo, .. } = l("x^2_i") else { panic!() };
        assert_eq!(t(&arriba.unwrap()), "2");
        assert_eq!(t(&abajo.unwrap()), "i");
    }

    #[test]
    fn un_subindice_solo() {
        let Pieza::ConIndices { arriba, abajo, .. } = l("a_1") else { panic!() };
        assert_eq!(t(&abajo.unwrap()), "1");
        assert!(arriba.is_none());
    }

    #[test]
    fn una_fraccion_tiene_arriba_y_abajo() {
        let Pieza::Fraccion(a, b) = l("\\frac{a}{b}") else { panic!() };
        assert_eq!(t(&a), "a");
        assert_eq!(t(&b), "b");
    }

    #[test]
    fn una_fraccion_dentro_de_otra() {
        let p = l("\\frac{\\frac{a}{b}}{c}");
        let Pieza::Fraccion(a, _) = &p else { panic!() };
        assert!(matches!(**a, Pieza::Fraccion(..)));
        assert_eq!(t(&p), "((a/b)/c)");
    }

    #[test]
    fn la_fraccion_sin_llaves_coge_un_caracter_de_cada_lado() {
        assert_eq!(t(&l("\\frac12")), "(1/2)");
    }

    #[test]
    fn una_raiz_normal_y_una_con_indice() {
        assert_eq!(t(&l("\\sqrt{x+1}")), "sqrt(x+1)");
        let Pieza::Raiz { dentro, indice } = l("\\sqrt[3]{x}") else { panic!() };
        assert_eq!(t(&indice.unwrap()), "3");
        assert_eq!(t(&dentro), "x");
    }

    #[test]
    fn las_letras_griegas_y_los_operadores_salen_como_simbolos() {
        assert_eq!(t(&l("\\pi")), "π");
        assert_eq!(t(&l("\\leq")), "≤");
        assert_eq!(t(&l("\\sum")), "∑");
        assert_eq!(t(&l("\\infty")), "∞");
        assert_eq!(t(&l("\\Delta")), "Δ");
    }

    #[test]
    fn las_funciones_se_quedan_como_palabra() {
        assert_eq!(t(&l("\\sin")), "sin");
        assert_eq!(t(&l("\\log")), "log");
    }

    #[test]
    fn un_mandato_desconocido_se_ensena_tal_cual() {
        assert!(t(&l("\\loquesea")).contains("loquesea"));
    }

    #[test]
    fn las_letras_van_en_cursiva_y_los_numeros_no() {
        let Pieza::Fila(v) = l("2x") else { panic!() };
        assert!(matches!(&v[0], Pieza::Texto { cursiva: false, .. }));
        assert!(matches!(&v[1], Pieza::Texto { cursiva: true, .. }));
    }

    #[test]
    fn una_formula_completa_se_entiende_entera() {
        let p = t(&l("x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}"));
        assert!(p.contains('±'), "{p}");
        assert!(p.contains("sqrt("), "{p}");
        assert!(p.contains("b^2"), "{p}");
        assert!(p.contains("/2"), "{p}");
    }

    #[test]
    fn los_parentesis_agrupan() {
        let Pieza::ConIndices { base, arriba, .. } = l("(a+b)^2") else { panic!() };
        assert!(matches!(*base, Pieza::Agrupado { .. }));
        assert_eq!(t(&arriba.unwrap()), "2");
    }

    #[test]
    fn la_barra_hace_una_fraccion() {
        let p = l("1/2");
        assert!(matches!(p, Pieza::Fraccion(..)), "salio {p:?}");
        assert_eq!(t(&p), "(1/2)");
    }

    #[test]
    fn la_fraccion_se_ata_mas_fuerte_que_la_suma() {
        let p = l("1/2 + x");
        let Pieza::Fila(v) = &p else { panic!() };
        assert!(matches!(v[0], Pieza::Fraccion(..)));
        assert!(t(&p).contains("(1/2)"));
    }

    #[test]
    fn los_exponentes_se_escriben_con_el_circunflejo_de_siempre() {
        assert_eq!(t(&l("x^2")), "x^2");
        assert_eq!(t(&l("x^{10}")), "x^10");
    }

    #[test]
    fn las_palabras_corrientes_hacen_su_simbolo() {
        assert_eq!(t(&l("pi")), "π");
        assert_eq!(t(&l("inf")), "∞");
        assert_eq!(t(&l("alpha")), "α");
    }

    #[test]
    fn raiz_y_sqrt_hacen_lo_mismo() {
        assert!(matches!(l("raiz(9)"), Pieza::Raiz { .. }));
        assert!(matches!(l("sqrt(9)"), Pieza::Raiz { .. }));
        assert_eq!(t(&l("raiz(x+1)")), "sqrt(x+1)");
    }

    #[test]
    fn las_parejas_de_signos_se_convierten() {
        assert!(t(&l("a <= b")).contains('≤'));
        assert!(t(&l("a != b")).contains('≠'));
        assert!(t(&l("a +- b")).contains('±'));
        assert!(t(&l("a -> b")).contains('→'));
    }

    #[test]
    fn pegado_significa_multiplicado() {
        let p = t(&l("2x"));
        assert_eq!(p, "2x");
        assert!(!p.contains('·'));
    }

    #[test]
    fn el_asterisco_si_pinta_su_punto() {
        assert!(t(&l("2*x")).contains('·'));
    }

    #[test]
    fn una_funcion_sale_derecha_y_con_su_argumento() {
        let p = t(&l("sin(x)"));
        assert!(p.starts_with("sin") && p.contains('x'), "{p}");
    }

    #[test]
    fn el_latex_de_antes_sigue_funcionando() {
        assert_eq!(t(&l("\\frac{a}{b}")), "(a/b)");
        assert_eq!(t(&l("\\pi")), "π");
        assert!(matches!(l("\\sqrt{x}"), Pieza::Raiz { .. }));
    }

    #[test]
    fn una_formula_de_verdad_escrita_a_lo_facil() {
        let p = t(&l("x = (-b +- sqrt(b^2 - 4ac)) / (2a)"));
        assert!(p.contains('±') && p.contains("sqrt(") && p.contains("b^2"), "{p}");
    }

    #[test]
    fn nada_de_esto_revienta() {
        for s in ["", "^", "_", "{", "}", "\\", "\\frac", "\\sqrt[", "((((", "x^^2", "a/", "/b", "x_{"] {
            leer(s);
            toques(s);
        }
    }

    #[test]
    fn una_llave_sin_cerrar_no_pierde_lo_de_dentro() {
        assert!(t(&l("x^{2")).contains('2'));
    }

    // ---- Los toques del editor de antes ----

    #[test]
    fn el_exponente_sube_y_sus_marcas_se_esconden() {
        assert_eq!(toques("x^2"), vec![(1, 2, Toque::Oculto), (2, 3, Toque::Arriba)]);
        assert_eq!(
            toques("x^{n+1}_i"),
            vec![(1, 2, Toque::Oculto), (2, 3, Toque::Oculto), (3, 6, Toque::Arriba), (6, 7, Toque::Oculto), (7, 8, Toque::Oculto), (8, 9, Toque::Abajo)]
        );
        assert_eq!(toques("1/2"), vec![(0, 1, Toque::Arriba), (2, 3, Toque::Abajo)]);
    }

    #[test]
    fn sin_nada_que_tocar_no_se_toca() {
        assert!(toques("a + b").is_empty());
        // Un circunflejo sin nada detras, o una barra suelta, se dejan a la vista.
        assert!(toques("x^").is_empty());
        assert!(toques("a / b").is_empty());
        assert!(toques("x^{2").is_empty());
        // Las posiciones van en UTF-16 (una letra de dos unidades delante).
        assert_eq!(toques("𝑥^2"), vec![(2, 3, Toque::Oculto), (3, 4, Toque::Arriba)]);
    }
}
