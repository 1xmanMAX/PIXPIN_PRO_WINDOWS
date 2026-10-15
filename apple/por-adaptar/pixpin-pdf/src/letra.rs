//! **La letra de la pantalla, metida en el PDF.**
//!
//! El texto del lienzo se pinta con Segoe UI (`pixpin_render::lienzo`,
//! `CreateTextFormat`). El PDF lo escribia en Helvetica sin incrustar: otra
//! letra, otros anchos, y por eso partia las lineas en otro sitio que la
//! pantalla —una nota de tres lineas salia en dos o en cuatro— y con otra
//! cara. Aqui se lee la letra de verdad del sistema y se mete en el fichero
//! **solo con las letras que se usan** (un subconjunto): la Segoe UI entera
//! son 950 KB y un plano con cuatro rotulos no tiene por que pesar eso.
//!
//! Se lee con DirectWrite (`IDWriteFontFace::TryGetFontTable`), el mismo que
//! pinta la pantalla, y no abriendo `C:\Windows\Fonts\segoeui.ttf`: asi sale
//! la misma letra que DirectWrite elige, este donde este instalada.
//!
//! Lo demas es puro y se prueba sin Windows: leer las tablas de una letra
//! TrueType, sacar de ellas los anchos y los glifos, y escribir una letra
//! nueva con solo los glifos que hacen falta. No se trae ninguna biblioteca:
//! un subconjunto TrueType son unas pocas tablas que se copian y dos
//! (`glyf`, `loca`) que se rehacen, y una dependencia para eso seria mas
//! grande que el propio codigo.
//!
//! # Lo que no hace
//!
//! - **Sin kerning ni ligaduras**: DirectWrite ajusta algunos pares (`AV`,
//!   `To`) y aqui cada letra avanza lo suyo. La diferencia es de decimas de
//!   pixel por par; puede mover una palabra de linea si caia justo en el
//!   borde.
//! - **Sin letra de reserva**: lo que Segoe UI no tiene (un ideograma, un
//!   emoji) DirectWrite lo busca en otra letra; aqui sale el hueco de la
//!   letra (`.notdef`). Antes salia un `?`.

use std::collections::{BTreeMap, BTreeSet};

/// El nombre de cuatro letras de una tabla (`glyf`, `head`...).
pub type Etiqueta = [u8; 4];

/// Una letra TrueType leida: sus tablas y lo que hace falta de ellas para
/// medir y escribir texto.
#[derive(Clone)]
pub struct Letra {
    /// El nombre PostScript, sin espacios: `SegoeUI`.
    pub nombre: String,
    tablas: BTreeMap<Etiqueta, Vec<u8>>,
    /// Unidades de diseno por em (2048 en Segoe UI).
    pub unidades: u16,
    avances: Vec<u16>,
    mapa: BTreeMap<u32, u16>,
    loca: Vec<u32>,
    /// `(ascenso, descenso, altura de mayusculas)` en milesimas de em.
    pub alturas: (i32, i32, i32),
    /// La caja de todos los glifos, en milesimas de em.
    pub caja: [i32; 4],
}

fn u16_en(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

fn i16_en(d: &[u8], o: usize) -> Option<i16> {
    u16_en(d, o).map(|v| v as i16)
}

fn u32_en(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *d.get(o)?,
        *d.get(o + 1)?,
        *d.get(o + 2)?,
        *d.get(o + 3)?,
    ]))
}

/// Las tablas que se copian tal cual al subconjunto. `cvt `, `fpgm` y `prep`
/// son el ajuste a la rejilla: los glifos llaman a sus funciones y, sin
/// ellas, un lector que las ejecute se encuentra una llamada a nada y puede
/// no pintar la letra. `OS/2` y `name` son pequenas y hay lectores que no
/// cargan una letra sin ellas.
const COPIADAS: [&Etiqueta; 6] = [b"hhea", b"OS/2", b"name", b"cvt ", b"fpgm", b"prep"];

impl Letra {
    /// **La letra a partir de sus tablas**, pedidas una a una a `tabla`.
    ///
    /// `None` si falta alguna de las imprescindibles o no se entiende: una
    /// letra CFF (`.otf`, sin `glyf`) no se sabe subconjuntar aqui, y
    /// entonces el PDF vuelve a Helvetica en vez de salir roto.
    pub fn desde_tablas(
        nombre: &str,
        tabla: &dyn Fn(&Etiqueta) -> Option<Vec<u8>>,
    ) -> Option<Letra> {
        let mut tablas = BTreeMap::new();
        for t in [
            b"head", b"maxp", b"hmtx", b"loca", b"glyf", b"cmap", b"post",
        ]
        .into_iter()
        .chain(COPIADAS)
        {
            if let Some(d) = tabla(t) {
                tablas.insert(*t, d);
            }
        }
        Letra::desde_mapa(nombre, tablas)
    }

    /// **La letra de un fichero `.ttf`**: para las pruebas, que no pasan por
    /// DirectWrite.
    pub fn desde_fichero(nombre: &str, datos: &[u8]) -> Option<Letra> {
        let n = u16_en(datos, 4)? as usize;
        let buscar = |t: &Etiqueta| -> Option<Vec<u8>> {
            (0..n).find_map(|i| {
                let r = 12 + 16 * i;
                if datos.get(r..r + 4)? != t {
                    return None;
                }
                let desde = u32_en(datos, r + 8)? as usize;
                let largo = u32_en(datos, r + 12)? as usize;
                datos.get(desde..desde + largo).map(<[u8]>::to_vec)
            })
        };
        Letra::desde_tablas(nombre, &buscar)
    }

    fn desde_mapa(nombre: &str, tablas: BTreeMap<Etiqueta, Vec<u8>>) -> Option<Letra> {
        let head = tablas.get(b"head")?;
        let hhea = tablas.get(b"hhea")?;
        let maxp = tablas.get(b"maxp")?;
        let hmtx = tablas.get(b"hmtx")?;
        let loca = tablas.get(b"loca")?;
        tablas.get(b"glyf")?;
        let unidades = u16_en(head, 18).filter(|u| *u > 0)?;
        let largo_loca = i16_en(head, 50)?;
        let glifos = u16_en(maxp, 4)? as usize;
        let n_metricas = (u16_en(hhea, 34)? as usize).clamp(1, glifos.max(1));
        let mut avances = Vec::with_capacity(glifos);
        for g in 0..glifos {
            avances.push(u16_en(hmtx, 4 * g.min(n_metricas - 1))?);
        }
        let loca: Vec<u32> = (0..=glifos)
            .map(|i| match largo_loca {
                0 => u16_en(loca, 2 * i).map(|v| v as u32 * 2),
                _ => u32_en(loca, 4 * i),
            })
            .collect::<Option<_>>()?;
        let mapa = leer_cmap(tablas.get(b"cmap")?)?;
        let mil = |v: i32| v * 1000 / unidades as i32;
        let os2 = tablas.get(b"OS/2");
        let mayusculas = os2
            .filter(|t| u16_en(t, 0).is_some_and(|v| v >= 2))
            .and_then(|t| i16_en(t, 88))
            .map_or(700, |v| mil(v as i32));
        let alturas = (
            mil(i16_en(hhea, 4)? as i32),
            mil(i16_en(hhea, 6)? as i32),
            mayusculas,
        );
        let caja = [
            mil(i16_en(head, 36)? as i32),
            mil(i16_en(head, 38)? as i32),
            mil(i16_en(head, 40)? as i32),
            mil(i16_en(head, 42)? as i32),
        ];
        Some(Letra {
            nombre: nombre
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect(),
            tablas,
            unidades,
            avances,
            mapa,
            loca,
            alturas,
            caja,
        })
    }

    /// Cuantos glifos tiene.
    pub fn glifos(&self) -> usize {
        self.avances.len()
    }

    /// El glifo de `c`, si la letra lo tiene.
    pub fn glifo(&self, c: char) -> Option<u16> {
        self.mapa.get(&(c as u32)).copied().filter(|g| *g != 0)
    }

    /// Lo que avanza el glifo `g`, en milesimas de em.
    pub fn avance(&self, g: u16) -> f32 {
        self.avances.get(g as usize).copied().unwrap_or(0) as f32 * 1000.0 / self.unidades as f32
    }

    /// Lo que avanza `c`, en milesimas de em: lo que no tiene la letra avanza
    /// lo de su hueco (`.notdef`), que es lo que se pintara.
    pub fn ancho(&self, c: char) -> f32 {
        self.avance(self.glifo(c).unwrap_or(0))
    }

    fn glifo_crudo(&self, g: u16) -> &[u8] {
        let (Some(&a), Some(&b)) = (self.loca.get(g as usize), self.loca.get(g as usize + 1))
        else {
            return &[];
        };
        self.tablas
            .get(b"glyf")
            .and_then(|t| t.get(a as usize..b as usize))
            .unwrap_or(&[])
    }

    /// Los glifos de los que esta hecho un glifo compuesto (una «a» con
    /// tilde es la «a» mas la tilde): si no van en el subconjunto, la «a»
    /// con tilde sale en blanco.
    fn piezas(&self, g: u16) -> Vec<u16> {
        let d = self.glifo_crudo(g);
        if i16_en(d, 0).is_none_or(|n| n >= 0) {
            return Vec::new();
        }
        let mut v = Vec::new();
        let mut o = 10;
        while let (Some(banderas), Some(pieza)) = (u16_en(d, o), u16_en(d, o + 2)) {
            v.push(pieza);
            o += 4;
            o += if banderas & 0x0001 != 0 { 4 } else { 2 };
            if banderas & 0x0008 != 0 {
                o += 2;
            } else if banderas & 0x0040 != 0 {
                o += 4;
            } else if banderas & 0x0080 != 0 {
                o += 8;
            }
            if banderas & 0x0020 == 0 {
                break;
            }
        }
        v
    }

    /// **La letra con solo los glifos `usados`** (mas el hueco, que siempre
    /// va), como fichero TrueType.
    ///
    /// Los numeros de glifo no cambian: los que sobran se quedan vacios. Asi
    /// el texto del PDF puede nombrar cada glifo por su numero de siempre
    /// (`CIDToGIDMap /Identity`) sin una tabla de traduccion que pudiera
    /// descuadrarse, y la letra pesa lo que pesan sus glifos: un glifo vacio
    /// es un hueco de cero bytes en `glyf`.
    pub fn subconjunto(&self, usados: &BTreeSet<u16>, caracteres: &BTreeMap<u16, char>) -> Vec<u8> {
        let total = self.glifos();
        let mut dentro: BTreeSet<u16> = usados
            .iter()
            .copied()
            .filter(|g| (*g as usize) < total)
            .collect();
        dentro.insert(0);
        let mut pendientes: Vec<u16> = dentro.iter().copied().collect();
        while let Some(g) = pendientes.pop() {
            for p in self.piezas(g) {
                if (p as usize) < total && dentro.insert(p) {
                    pendientes.push(p);
                }
            }
        }
        let mut glyf = Vec::new();
        let mut loca = Vec::with_capacity((total + 1) * 4);
        for g in 0..total {
            loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
            if dentro.contains(&(g as u16)) {
                glyf.extend_from_slice(self.glifo_crudo(g as u16));
                while glyf.len() % 4 != 0 {
                    glyf.push(0);
                }
            }
        }
        loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());

        let mut tablas: BTreeMap<Etiqueta, Vec<u8>> = BTreeMap::new();
        for t in COPIADAS {
            if let Some(d) = self.tablas.get(t) {
                tablas.insert(*t, d.clone());
            }
        }
        // `head` con `loca` larga (la que se acaba de escribir) y la suma de
        // comprobacion a cero: se pone al final, sobre el fichero entero.
        let mut head = self.tablas.get(b"head").cloned().unwrap_or_default();
        if head.len() >= 54 {
            head[8..12].copy_from_slice(&[0; 4]);
            head[50..52].copy_from_slice(&1i16.to_be_bytes());
        }
        tablas.insert(*b"head", head);
        tablas.insert(*b"glyf", glyf);
        tablas.insert(*b"loca", loca);
        // `maxp` tal cual: el numero de glifos no cambia.
        if let Some(m) = self.tablas.get(b"maxp") {
            tablas.insert(*b"maxp", m.clone());
        }
        // `hmtx` con las metricas de los que sobran a cero: el mismo tamano
        // (lo exige `hhea`) pero comprimida a casi nada.
        if let Some(h) = self.tablas.get(b"hmtx") {
            let mut h = h.clone();
            for g in 0..total {
                if !dentro.contains(&(g as u16)) && 4 * g + 4 <= h.len() {
                    h[4 * g..4 * g + 4].copy_from_slice(&[0; 4]);
                }
            }
            tablas.insert(*b"hmtx", h);
        }
        // `post` version 3: sin nombres de glifo, que son la mitad de la
        // tabla y ningun lector los necesita para pintar.
        let mut post = vec![0u8; 32];
        if let Some(p) = self.tablas.get(b"post") {
            let n = p.len().min(16);
            post[..n].copy_from_slice(&p[..n]);
        }
        post[0..4].copy_from_slice(&0x0003_0000u32.to_be_bytes());
        tablas.insert(*b"post", post);
        tablas.insert(*b"cmap", cmap_minima(caracteres));
        fichero_sfnt(&tablas)
    }
}

/// El `cmap` de una letra: de caracter a glifo. Se prefiere la tabla de
/// Unicode completo (formato 12, Windows 3/10) y si no la del plano basico
/// (formato 4, Windows 3/1).
fn leer_cmap(d: &[u8]) -> Option<BTreeMap<u32, u16>> {
    let n = u16_en(d, 2)? as usize;
    let mut basico = None;
    let mut completo = None;
    for i in 0..n {
        let r = 4 + 8 * i;
        let (plataforma, codificacion) = (u16_en(d, r)?, u16_en(d, r + 2)?);
        let desde = u32_en(d, r + 4)? as usize;
        let formato = u16_en(d, desde)?;
        match (plataforma, codificacion, formato) {
            (3, 10, 12) | (0, 4, 12) | (0, 6, 12) => completo = Some(desde),
            (3, 1, 4) | (0, 3, 4) => basico = Some(desde),
            _ => {}
        }
    }
    if let Some(o) = completo {
        let grupos = u32_en(d, o + 12)? as usize;
        let mut mapa = BTreeMap::new();
        for k in 0..grupos {
            let g = o + 16 + 12 * k;
            let (a, b, primero) = (u32_en(d, g)?, u32_en(d, g + 4)?, u32_en(d, g + 8)?);
            // Un grupo absurdo (millones de caracteres) no llena la memoria.
            if b < a || b - a > 0x10_0000 {
                continue;
            }
            for c in a..=b {
                mapa.insert(c, (primero + (c - a)) as u16);
            }
        }
        return Some(mapa);
    }
    let o = basico?;
    let segmentos = u16_en(d, o + 6)? as usize / 2;
    let fin = o + 14;
    let inicio = fin + 2 * segmentos + 2;
    let delta = inicio + 2 * segmentos;
    let rango = delta + 2 * segmentos;
    let mut mapa = BTreeMap::new();
    for s in 0..segmentos {
        let (a, b) = (u16_en(d, inicio + 2 * s)?, u16_en(d, fin + 2 * s)?);
        let (dl, ro) = (
            u16_en(d, delta + 2 * s)?,
            u16_en(d, rango + 2 * s)? as usize,
        );
        if a == 0xffff || b < a {
            continue;
        }
        for c in a..=b {
            let g = if ro == 0 {
                c.wrapping_add(dl)
            } else {
                let donde = rango + 2 * s + ro + 2 * (c - a) as usize;
                match u16_en(d, donde)? {
                    0 => 0,
                    g => g.wrapping_add(dl),
                }
            };
            mapa.insert(c as u32, g);
        }
    }
    Some(mapa)
}

/// Un `cmap` de formato 4 con solo los caracteres usados: la letra
/// subconjunto no lo necesita para el PDF (el texto va por numero de glifo),
/// pero hay lectores que no cargan una TrueType sin el.
fn cmap_minima(caracteres: &BTreeMap<u16, char>) -> Vec<u8> {
    let mut pares: Vec<(u16, u16)> = caracteres
        .iter()
        .filter_map(|(g, c)| {
            u16::try_from(*c as u32)
                .ok()
                .filter(|c| *c != 0xffff)
                .map(|c| (c, *g))
        })
        .collect();
    pares.sort();
    pares.dedup_by_key(|(c, _)| *c);
    let segmentos = pares.len() + 1;
    let mut t = Vec::new();
    let pon = |t: &mut Vec<u8>, v: u16| t.extend_from_slice(&v.to_be_bytes());
    // Cabecera: version 0, una subtabla Windows/Unicode BMP en el byte 12.
    for v in [0u16, 1, 3, 1] {
        pon(&mut t, v);
    }
    t.extend_from_slice(&12u32.to_be_bytes());
    let largo = 16 + 8 * segmentos;
    let potencia = (segmentos as f32).log2().floor() as u32;
    let busqueda = 2 * (1u16 << potencia);
    for v in [
        4u16,
        largo as u16,
        0,
        (2 * segmentos) as u16,
        busqueda,
        potencia as u16,
        (2 * segmentos) as u16 - busqueda,
    ] {
        pon(&mut t, v);
    }
    for (c, _) in &pares {
        pon(&mut t, *c);
    }
    pon(&mut t, 0xffff);
    pon(&mut t, 0);
    for (c, _) in &pares {
        pon(&mut t, *c);
    }
    pon(&mut t, 0xffff);
    for (c, g) in &pares {
        pon(&mut t, g.wrapping_sub(*c));
    }
    pon(&mut t, 1);
    for _ in 0..segmentos {
        pon(&mut t, 0);
    }
    t
}

fn suma(d: &[u8]) -> u32 {
    d.chunks(4).fold(0u32, |s, c| {
        let mut b = [0u8; 4];
        b[..c.len()].copy_from_slice(c);
        s.wrapping_add(u32::from_be_bytes(b))
    })
}

/// Las tablas como fichero TrueType: el directorio, cada tabla alineada a
/// cuatro bytes con su suma, y el ajuste de `head` que hace que el fichero
/// entero sume lo que dice la norma.
fn fichero_sfnt(tablas: &BTreeMap<Etiqueta, Vec<u8>>) -> Vec<u8> {
    let n = tablas.len() as u16;
    let potencia = 15 - n.max(1).leading_zeros() as u16;
    let busqueda = 16 * (1u16 << potencia);
    let mut salida = Vec::new();
    salida.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    for v in [n, busqueda, potencia, 16 * n - busqueda] {
        salida.extend_from_slice(&v.to_be_bytes());
    }
    let mut desplazamiento = 12 + 16 * tablas.len();
    let mut cuerpo = Vec::new();
    let mut donde_head = None;
    for (t, d) in tablas {
        if t == b"head" {
            donde_head = Some(desplazamiento);
        }
        salida.extend_from_slice(t);
        salida.extend_from_slice(&suma(d).to_be_bytes());
        salida.extend_from_slice(&(desplazamiento as u32).to_be_bytes());
        salida.extend_from_slice(&(d.len() as u32).to_be_bytes());
        cuerpo.extend_from_slice(d);
        while cuerpo.len() % 4 != 0 {
            cuerpo.push(0);
        }
        desplazamiento = 12 + 16 * tablas.len() + cuerpo.len();
    }
    salida.extend_from_slice(&cuerpo);
    if let Some(h) = donde_head
        && h + 12 <= salida.len()
    {
        let ajuste = 0xb1b0_afbau32.wrapping_sub(suma(&salida));
        salida[h + 8..h + 12].copy_from_slice(&ajuste.to_be_bytes());
    }
    salida
}

/// **La letra de la pantalla, pedida a DirectWrite**: la normal de la
/// familia `familia`, la misma que elige `CreateTextFormat` con peso,
/// anchura y estilo normales.
///
/// `None` si la familia no esta o no es TrueType; quien escribe el PDF
/// vuelve entonces a Helvetica.
pub fn del_sistema(familia: &str) -> Option<Letra> {
    use windows::Win32::Graphics::DirectWrite::{
        DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_WEIGHT_NORMAL, DWriteCreateFactory, IDWriteFactory,
    };
    use windows::core::{BOOL, HSTRING};
    // SAFETY: llamadas COM de solo lectura sobre objetos que se crean aqui y
    // viven hasta el final de la funcion; los punteros de cada tabla se
    // copian antes de devolverla con `ReleaseFontTable`.
    unsafe {
        let fabrica: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok()?;
        let mut coleccion = None;
        fabrica
            .GetSystemFontCollection(&mut coleccion, false)
            .ok()?;
        let coleccion = coleccion?;
        let (mut indice, mut existe) = (0u32, BOOL(0));
        coleccion
            .FindFamilyName(&HSTRING::from(familia), &mut indice, &mut existe)
            .ok()?;
        if !existe.as_bool() {
            return None;
        }
        let cara = coleccion
            .GetFontFamily(indice)
            .ok()?
            .GetFirstMatchingFont(
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
            )
            .ok()?
            .CreateFontFace()
            .ok()?;
        let tabla = |t: &Etiqueta| -> Option<Vec<u8>> {
            // `DWRITE_MAKE_OPENTYPE_TAG`: las cuatro letras al reves.
            let etiqueta = u32::from_le_bytes(*t);
            let (mut datos, mut largo, mut contexto, mut hay) =
                (std::ptr::null_mut(), 0u32, std::ptr::null_mut(), BOOL(0));
            cara.TryGetFontTable(etiqueta, &mut datos, &mut largo, &mut contexto, &mut hay)
                .ok()?;
            if !hay.as_bool() || datos.is_null() {
                return None;
            }
            let copia = std::slice::from_raw_parts(datos as *const u8, largo as usize).to_vec();
            cara.ReleaseFontTable(contexto);
            Some(copia)
        };
        Letra::desde_tablas(&familia.replace(' ', ""), &tabla)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn segoe() -> Option<Letra> {
        let d = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf").ok()?;
        Letra::desde_fichero("Segoe UI", &d)
    }

    #[test]
    fn la_letra_del_sistema_es_la_misma_que_la_del_fichero() {
        let Some(fichero) = segoe() else {
            return;
        };
        let sistema = del_sistema("Segoe UI").expect("Segoe UI esta en todo Windows");
        assert_eq!(sistema.nombre, "SegoeUI");
        assert_eq!(sistema.glifos(), fichero.glifos());
        assert_eq!(sistema.ancho('W'), fichero.ancho('W'));
        // Caso negativo: una familia que no existe no da letra.
        assert!(del_sistema("Letra Que No Existe 123").is_none());
    }

    #[test]
    fn los_anchos_salen_de_la_letra_y_lo_que_no_tiene_avanza_su_hueco() {
        let Some(l) = segoe() else {
            return;
        };
        assert_eq!(l.unidades, 2048);
        assert!(l.ancho('W') > l.ancho('i') * 2.0);
        assert_eq!(l.ancho(' '), l.avance(l.glifo(' ').expect("espacio")));
        assert!(l.glifo('ñ').is_some() && l.glifo('€').is_some());
        // Caso negativo: un ideograma no esta en Segoe UI.
        assert_eq!(l.glifo('漢'), None);
        assert_eq!(l.ancho('漢'), l.avance(0));
    }

    #[test]
    fn el_subconjunto_es_pequeno_y_se_vuelve_a_leer_con_sus_glifos() {
        let Some(l) = segoe() else {
            return;
        };
        let texto = "Planta baja, año 2026";
        let caracteres: BTreeMap<u16, char> = texto
            .chars()
            .filter_map(|c| Some((l.glifo(c)?, c)))
            .collect();
        let usados: BTreeSet<u16> = caracteres.keys().copied().collect();
        let sub = l.subconjunto(&usados, &caracteres);
        assert!(
            sub.len() < 150_000,
            "mucho menos que la letra entera: {}",
            sub.len()
        );
        let otra = Letra::desde_fichero("sub", &sub).expect("el subconjunto es una letra");
        assert_eq!(otra.glifos(), l.glifos(), "los numeros de glifo no cambian");
        for c in texto.chars() {
            assert_eq!(otra.glifo(c), l.glifo(c), "{c}");
            assert_eq!(otra.ancho(c), l.ancho(c), "{c}");
            assert!(!otra.glifo_crudo(l.glifo(c).expect("esta")).is_empty() || c == ' ');
        }
        // Caso negativo: lo que no se uso no va dentro.
        let x = l.glifo('X').expect("X");
        assert!(otra.glifo_crudo(x).is_empty());
        // Y el fichero suma lo que dice la norma.
        assert_eq!(suma(&sub), 0xb1b0_afba);
    }

    #[test]
    fn una_letra_sin_sus_tablas_no_se_lee() {
        // Casos negativos: basura, o una letra sin `glyf` (una CFF).
        assert!(Letra::desde_fichero("x", b"no es una letra").is_none());
        assert!(Letra::desde_tablas("x", &|_| None).is_none());
    }

    /// Los anchos de Segoe UI para la tabla del motor
    /// (`pixpin_motor2d::exportar`). No es una prueba: se lanza a mano con
    /// `--ignored --nocapture` si la tabla hubiera que rehacerla.
    #[test]
    #[ignore]
    fn imprimir_la_tabla_de_anchos() {
        let l = segoe().expect("segoe");
        let fila = |a: u32, b: u32| {
            (a..=b)
                .map(|c| {
                    format!(
                        "{}",
                        l.ancho(char::from_u32(c).unwrap_or(' ')).round() as u16
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        println!("ASCII: [{}]", fila(32, 126));
        println!("LATIN1: [{}]", fila(160, 255));
    }
}
