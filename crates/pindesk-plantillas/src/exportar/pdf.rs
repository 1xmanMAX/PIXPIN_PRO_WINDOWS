//! **A PDF de vectores**, escrito a mano (PDF 1.7, sin bibliotecas): una
//! pagina por pagina de la repeticion, las rayas como rayas y las letras
//! como texto de verdad (se puede buscar y copiar). Las letras que se dan
//! van dentro enteras (`CIDFontType2`, `Identity-H`, con su `ToUnicode`);
//! sin ninguna, Helvetica con la codificacion de Windows (lo de fuera del
//! latin-1 sale como «?»).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::io::Write as _;

use super::letras::{Cara, Letras, Vista};
use crate::modelo::PT_POR_MM;
use crate::prim::{Caja, PaginaHecha, Prim, Raya};
use crate::tema::Color;
use crate::texto::{self, Medidor};

/// **El PDF de unas paginas.**
pub fn a_pdf(paginas: &[PaginaHecha], letras: &Letras, titulo: &str) -> Vec<u8> {
    let mut e = Escritor::default();
    // Los objetos fijos: 1 catalogo, 2 arbol de paginas, 3 informacion.
    let mut fuentes = Fuentes::default();
    let mut alfas: BTreeSet<u8> = BTreeSet::new();
    let mut contenidos = Vec::new();
    for p in paginas {
        contenidos.push(contenido(
            p,
            &letras.vista(&p.familias),
            &mut fuentes,
            &mut alfas,
        ));
    }
    e.reservar(3);
    // Las fuentes.
    let mut recursos_fuentes = String::new();
    for (i, f) in fuentes.usadas.iter().enumerate() {
        let id = escribir_fuente(&mut e, f);
        let _ = write!(recursos_fuentes, "/F{i} {id} 0 R ");
    }
    if fuentes.helvetica {
        let id = e.objeto(
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
                .to_vec(),
        );
        let _ = write!(recursos_fuentes, "/FH {id} 0 R ");
    }
    let mut estados = String::new();
    for a in &alfas {
        let v = f32::from(*a) / 255.0;
        let id = e.objeto(format!("<< /Type /ExtGState /ca {v:.3} /CA {v:.3} >>").into_bytes());
        let _ = write!(estados, "/A{a} {id} 0 R ");
    }
    let recursos = e.objeto(
        format!("<< /Font << {recursos_fuentes}>> /ExtGState << {estados}>> >>").into_bytes(),
    );
    let mut hijos = Vec::new();
    for (p, c) in paginas.iter().zip(contenidos) {
        let flujo = flujo(&c, "");
        let id_c = e.objeto(flujo);
        let (w, h) = (p.ancho * PT_POR_MM, p.alto * PT_POR_MM);
        let id = e.objeto(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.2} {h:.2}] /Resources {recursos} 0 R /Contents {id_c} 0 R >>"
            )
            .into_bytes(),
        );
        hijos.push(format!("{id} 0 R"));
    }
    e.poner(1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    e.poner(
        2,
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            hijos.join(" "),
            hijos.len()
        )
        .into_bytes(),
    );
    e.poner(
        3,
        format!(
            "<< /Title {} /Producer {} /Creator {} >>",
            texto_pdf(titulo),
            texto_pdf(crate::NOMBRE_PRODUCTO),
            texto_pdf(crate::NOMBRE_PRODUCTO)
        )
        .into_bytes(),
    );
    e.terminar(3)
}

// ---------------------------------------------------------------------------
// El fichero
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Escritor {
    objetos: Vec<Option<Vec<u8>>>,
}

impl Escritor {
    fn reservar(&mut self, n: usize) {
        self.objetos.resize(self.objetos.len().max(n), None);
    }
    fn objeto(&mut self, cuerpo: Vec<u8>) -> usize {
        self.objetos.push(Some(cuerpo));
        self.objetos.len()
    }
    fn poner(&mut self, id: usize, cuerpo: Vec<u8>) {
        self.objetos[id - 1] = Some(cuerpo);
    }
    fn terminar(self, info: usize) -> Vec<u8> {
        let mut sal: Vec<u8> = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut posiciones = Vec::new();
        for (i, o) in self.objetos.iter().enumerate() {
            posiciones.push(sal.len());
            let _ = writeln!(sal, "{} 0 obj", i + 1);
            sal.extend_from_slice(o.as_deref().unwrap_or(b"null"));
            sal.extend_from_slice(b"\nendobj\n");
        }
        let xref = sal.len();
        let _ = write!(
            sal,
            "xref\n0 {}\n0000000000 65535 f \n",
            self.objetos.len() + 1
        );
        for p in posiciones {
            let _ = writeln!(sal, "{p:010} 00000 n ");
        }
        let _ = write!(
            sal,
            "trailer\n<< /Size {} /Root 1 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            self.objetos.len() + 1
        );
        sal
    }
}

/// Un flujo comprimido con `extra` en su diccionario.
fn flujo(datos: &[u8], extra: &str) -> Vec<u8> {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    let _ = z.write_all(datos);
    let comprimido = z.finish().unwrap_or_default();
    let mut sal = format!(
        "<< /Length {} /Filter /FlateDecode {extra}>>\nstream\n",
        comprimido.len()
    )
    .into_bytes();
    sal.extend_from_slice(&comprimido);
    sal.extend_from_slice(b"\nendstream");
    sal
}

/// Un texto del diccionario de informacion (UTF-16 con marca).
fn texto_pdf(s: &str) -> String {
    let mut h = String::from("<FEFF");
    for u in s.encode_utf16() {
        let _ = write!(h, "{u:04X}");
    }
    h.push('>');
    h
}

// ---------------------------------------------------------------------------
// Las fuentes
// ---------------------------------------------------------------------------

struct FuenteUsada {
    cara: Cara,
    /// glifo → (avance en milesimas de em, caracter).
    glifos: BTreeMap<u16, (i32, char)>,
}

#[derive(Default)]
struct Fuentes {
    usadas: Vec<FuenteUsada>,
    helvetica: bool,
}

impl Fuentes {
    fn indice(&mut self, cara: &Cara) -> usize {
        if let Some(i) = self
            .usadas
            .iter()
            .position(|f| std::sync::Arc::ptr_eq(&f.cara.bytes, &cara.bytes))
        {
            return i;
        }
        self.usadas.push(FuenteUsada {
            cara: cara.clone(),
            glifos: BTreeMap::new(),
        });
        self.usadas.len() - 1
    }
}

fn escribir_fuente(e: &mut Escritor, f: &FuenteUsada) -> usize {
    use ab_glyph::Font;
    let fuente = &f.cara.fuente;
    let em = fuente.units_per_em().unwrap_or(1000.0);
    let mil = |v: f32| (v / em * 1000.0).round() as i32;
    let nombre = format!("PDP{:04X}+Letra", f.cara.bytes.len() % 0xffff);
    let fichero = e.objeto(flujo(
        &f.cara.bytes,
        &format!("/Length1 {} ", f.cara.bytes.len()),
    ));
    let asc = mil(fuente.ascent_unscaled());
    let desc = mil(fuente.descent_unscaled());
    let descriptor = e.objeto(
        format!(
            "<< /Type /FontDescriptor /FontName /{nombre} /Flags 32 /FontBBox [-1000 {desc} 2000 {asc}] /ItalicAngle 0 /Ascent {asc} /Descent {desc} /CapHeight {asc} /StemV 80 /FontFile2 {fichero} 0 R >>"
        )
        .into_bytes(),
    );
    let mut anchos = String::new();
    for (g, (w, _)) in &f.glifos {
        let _ = write!(anchos, "{g} [{w}] ");
    }
    let cid = e.objeto(
        format!(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{nombre} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {descriptor} 0 R /W [{anchos}] /DW 500 /CIDToGIDMap /Identity >>"
        )
        .into_bytes(),
    );
    // Lo que dice cada glifo, para buscar y copiar.
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let pares: Vec<(u16, char)> = f.glifos.iter().map(|(g, (_, c))| (*g, *c)).collect();
    for trozo in pares.chunks(100) {
        let _ = writeln!(cmap, "{} beginbfchar", trozo.len());
        for (g, c) in trozo {
            let mut u = String::new();
            let mut b = [0u16; 2];
            for x in c.encode_utf16(&mut b) {
                let _ = write!(u, "{x:04X}");
            }
            let _ = writeln!(cmap, "<{g:04X}> <{u}>");
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    let unicode = e.objeto(flujo(cmap.as_bytes(), ""));
    e.objeto(
        format!(
            "<< /Type /Font /Subtype /Type0 /BaseFont /{nombre} /Encoding /Identity-H /DescendantFonts [{cid} 0 R] /ToUnicode {unicode} 0 R >>"
        )
        .into_bytes(),
    )
}

// ---------------------------------------------------------------------------
// El contenido de una pagina
// ---------------------------------------------------------------------------

struct Lapiz<'a> {
    s: String,
    alto: f32,
    alfas: &'a mut BTreeSet<u8>,
    alfa: u8,
}

impl Lapiz<'_> {
    fn x(&self, mm: f32) -> f32 {
        mm * PT_POR_MM
    }
    fn y(&self, mm: f32) -> f32 {
        (self.alto - mm) * PT_POR_MM
    }
    fn alfa(&mut self, a: u8) {
        if a != self.alfa {
            self.alfas.insert(a);
            let _ = writeln!(self.s, "/A{a} gs");
            self.alfa = a;
        }
    }
    fn relleno(&mut self, c: Color) {
        self.alfa(c.a);
        let [r, g, b, _] = c.f32s();
        let _ = writeln!(self.s, "{r:.3} {g:.3} {b:.3} rg");
    }
    fn trazo(&mut self, c: Color, grosor: f32, raya: Raya) {
        self.alfa(c.a);
        let [r, g, b, _] = c.f32s();
        let w = grosor.max(0.05) * PT_POR_MM;
        let _ = writeln!(self.s, "{r:.3} {g:.3} {b:.3} RG {w:.3} w");
        match raya {
            Raya::Continua => self.s.push_str("[] 0 d 0 J\n"),
            Raya::Discontinua => {
                let (a, b) = (
                    (grosor * 6.0 + 0.8) * PT_POR_MM,
                    (grosor * 4.0 + 0.6) * PT_POR_MM,
                );
                let _ = writeln!(self.s, "[{a:.2} {b:.2}] 0 d 0 J");
            }
            Raya::Punteada => {
                let b = (grosor * 3.0 + 0.4) * PT_POR_MM;
                let _ = writeln!(self.s, "[0 {b:.2}] 0 d 1 J");
            }
        }
    }
    fn mover(&mut self, x: f32, y: f32) {
        let (a, b) = (self.x(x), self.y(y));
        let _ = writeln!(self.s, "{a:.2} {b:.2} m");
    }
    fn hasta(&mut self, x: f32, y: f32) {
        let (a, b) = (self.x(x), self.y(y));
        let _ = writeln!(self.s, "{a:.2} {b:.2} l");
    }
    fn curva(&mut self, p: [(f32, f32); 3]) {
        let v: Vec<String> = p
            .iter()
            .map(|(x, y)| format!("{:.2} {:.2}", self.x(*x), self.y(*y)))
            .collect();
        let _ = writeln!(self.s, "{} c", v.join(" "));
    }
    fn rect(&mut self, c: Caja, r: f32) {
        let r = r.min(c.w / 2.0).min(c.h / 2.0).max(0.0);
        if r <= 0.01 {
            let _ = writeln!(
                self.s,
                "{:.2} {:.2} {:.2} {:.2} re",
                self.x(c.x),
                self.y(c.abajo()),
                c.w * PT_POR_MM,
                c.h * PT_POR_MM
            );
            return;
        }
        let k = r * 0.552_284_8;
        let (x0, y0, x1, y1) = (c.x, c.y, c.derecha(), c.abajo());
        self.mover(x0 + r, y0);
        self.hasta(x1 - r, y0);
        self.curva([(x1 - r + k, y0), (x1, y0 + r - k), (x1, y0 + r)]);
        self.hasta(x1, y1 - r);
        self.curva([(x1, y1 - r + k), (x1 - r + k, y1), (x1 - r, y1)]);
        self.hasta(x0 + r, y1);
        self.curva([(x0 + r - k, y1), (x0, y1 - r + k), (x0, y1 - r)]);
        self.hasta(x0, y0 + r);
        self.curva([(x0, y0 + r - k), (x0 + r - k, y0), (x0 + r, y0)]);
        self.s.push_str("h\n");
    }
    fn circulo(&mut self, cx: f32, cy: f32, r: f32) {
        let k = r * 0.552_284_8;
        self.mover(cx + r, cy);
        self.curva([(cx + r, cy + k), (cx + k, cy + r), (cx, cy + r)]);
        self.curva([(cx - k, cy + r), (cx - r, cy + k), (cx - r, cy)]);
        self.curva([(cx - r, cy - k), (cx - k, cy - r), (cx, cy - r)]);
        self.curva([(cx + k, cy - r), (cx + r, cy - k), (cx + r, cy)]);
        self.s.push_str("h\n");
    }
    fn pintar(&mut self, relleno: Option<Color>, trazo: Option<(Color, f32)>) {
        if let Some(c) = relleno {
            self.relleno(c);
        }
        if let Some((c, g)) = trazo {
            self.trazo(c, g, Raya::Continua);
        }
        self.s.push_str(match (relleno.is_some(), trazo.is_some()) {
            (true, true) => "B\n",
            (true, false) => "f\n",
            (false, true) => "S\n",
            (false, false) => "n\n",
        });
    }
}

fn contenido(
    p: &PaginaHecha,
    letras: &Vista,
    fuentes: &mut Fuentes,
    alfas: &mut BTreeSet<u8>,
) -> Vec<u8> {
    let mut l = Lapiz {
        s: String::new(),
        alto: p.alto,
        alfas,
        alfa: 255,
    };
    for prim in &p.prims {
        match prim {
            Prim::Rect {
                caja,
                relleno,
                trazo,
                grosor,
                radio,
            } => {
                l.rect(*caja, *radio);
                l.pintar(*relleno, trazo.map(|c| (c, *grosor)));
            }
            Prim::Linea {
                a,
                b,
                color,
                grosor,
                raya,
            } => {
                l.trazo(*color, *grosor, *raya);
                l.mover(a.0, a.1);
                l.hasta(b.0, b.1);
                l.s.push_str("S\n");
            }
            Prim::Circulo {
                centro,
                radio,
                relleno,
                trazo,
                grosor,
            } => {
                l.circulo(centro.0, centro.1, *radio);
                l.pintar(*relleno, trazo.map(|c| (c, *grosor)));
            }
            Prim::Poli {
                puntos,
                color,
                grosor,
            } => {
                if puntos.len() < 2 {
                    continue;
                }
                l.trazo(*color, *grosor, Raya::Continua);
                l.s.push_str("1 J 1 j\n");
                l.mover(puntos[0].0, puntos[0].1);
                for q in &puntos[1..] {
                    l.hasta(q.0, q.1);
                }
                l.s.push_str("S\n");
            }
            Prim::Puntos {
                centros,
                radio,
                color,
            } => {
                l.relleno(*color);
                for (x, y) in centros {
                    l.circulo(*x, *y, *radio);
                }
                l.s.push_str("f\n");
            }
            Prim::Texto { .. } => {
                let medidor: &dyn Medidor = if letras.vacias() {
                    &texto::Estimado
                } else {
                    letras
                };
                for r in texto::renglones(prim, medidor) {
                    escribir_renglon(&mut l, &r, letras, fuentes);
                }
            }
        }
    }
    l.s.into_bytes()
}

fn escribir_renglon(l: &mut Lapiz, r: &texto::Renglon, letras: &Vista, fuentes: &mut Fuentes) {
    l.relleno(r.color);
    let tam = r.tam * PT_POR_MM;
    let (x, y) = (l.x(r.x), l.y(r.base));
    if letras.vacias() {
        fuentes.helvetica = true;
        let mut t = String::new();
        for c in r.texto.chars() {
            let b = latin1(c);
            match b {
                b'(' | b')' | b'\\' => {
                    t.push('\\');
                    t.push(b as char);
                }
                32..=126 => t.push(b as char),
                _ => {
                    let _ = write!(t, "\\{b:03o}");
                }
            }
        }
        let _ = writeln!(l.s, "BT /FH {tam:.2} Tf {x:.2} {y:.2} Td ({t}) Tj ET");
        return;
    }
    let _ = writeln!(l.s, "BT {x:.2} {y:.2} Td");
    let mut avances: HashMap<u16, i32> = HashMap::new();
    for t in letras.trozos(&r.texto, r.letra, r.negrita) {
        if !t.cara.suelta {
            // Una coleccion .ttc no entra en un PDF: esos caracteres, fuera.
            continue;
        }
        let i = fuentes.indice(t.cara);
        let mut hex = String::new();
        let mut kern = Vec::new();
        let mut antes = None;
        for (g, c) in &t.glifos {
            if let Some(a) = antes {
                let k = (t.cara.interletra(a, *g, 1000.0)).round() as i32;
                if k != 0 {
                    kern.push((hex.len(), k));
                }
            }
            let w = (t.cara.avance(*g) * 1000.0).round() as i32;
            avances.insert(g.0, w);
            fuentes.usadas[i].glifos.entry(g.0).or_insert((w, *c));
            let _ = write!(hex, "{:04X}", g.0);
            antes = Some(*g);
        }
        // Con el ajuste entre letras (TJ): numeros negativos acercan.
        let mut tj = String::from("[<");
        let mut desde = 0;
        for (pos, k) in kern {
            tj.push_str(&hex[desde..pos]);
            let _ = write!(tj, "> {} <", -k);
            desde = pos;
        }
        tj.push_str(&hex[desde..]);
        tj.push_str(">] TJ");
        let ancho: f32 = t
            .glifos
            .iter()
            .map(|(g, _)| avances.get(&g.0).copied().unwrap_or(0) as f32)
            .sum::<f32>()
            / 1000.0
            * tam;
        let _ = writeln!(l.s, "/F{i} {tam:.2} Tf {tj}");
        // El siguiente trozo, detras de este.
        let _ = writeln!(l.s, "{ancho:.2} 0 Td");
    }
    l.s.push_str("ET\n");
}

/// Un caracter en la codificacion de Windows (lo de fuera, «?»).
fn latin1(c: char) -> u8 {
    match c {
        '€' => 0x80,
        '‘' => 0x91,
        '’' => 0x92,
        '“' => 0x93,
        '”' => 0x94,
        '•' => 0x95,
        '–' => 0x96,
        '—' => 0x97,
        c if (c as u32) < 256 => c as u8,
        _ => b'?',
    }
}
