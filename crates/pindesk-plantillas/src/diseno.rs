//! **Maquetar**: del documento y el contexto de una pagina a primitivas
//! colocadas. Funcion pura: lo mismo entra, lo mismo sale, sin letras ni
//! ventanas (los textos van en su caja y quien pinta los parte con su
//! medidor, `texto::renglones`).
//!
//! El reparto es el de un flex: en una fila (o columna) cada hijo pide su
//! ancho (o alto) con su [`Tam`]: los fijos se quitan primero, los `auto`
//! piden lo que mide su contenido (si lo sabe: una lista sabe sus
//! renglones; un area de notas no, y se estira como `1fr`), y lo que queda
//! se reparte entre las fracciones.

use crate::campos::Contexto;
use crate::fecha::{self, Fecha};
use crate::modelo::{Alineacion, Bloque, Documento, Eje, PT_POR_MM, Pauta, Tam, Tipo, Trazo};
use crate::prim::{Caja, CajaBloque, PaginaHecha, Prim, Raya, Vertical};
use crate::tema::{Color, Letra, Tema};

/// Lo alto de un renglon de lista o de tabla (mm).
pub const FILA_LISTA: f32 = 7.0;
pub const FILA_TABLA: f32 = 8.0;
/// Lo alto del rotulo de una seccion.
pub const ROTULO: f32 = 8.0;
/// La banda de color de arriba.
pub const BANDA: f32 = 3.0;
/// Lo mas hondo que se maqueta (un arbol mas hondo se corta).
pub const MAX_HONDO: usize = 12;

fn pt(p: f32) -> f32 {
    p / PT_POR_MM
}

/// **Maqueta una pagina.**
pub fn maquetar(doc: &Documento, ctx: &Contexto) -> PaginaHecha {
    let (ancho, alto) = doc.pagina.medidas();
    let t = &doc.tema;
    let mut s = Salida {
        prims: Vec::new(),
        cajas: Vec::new(),
        t,
        ctx,
    };
    s.prims.push(Prim::Rect {
        caja: Caja::nueva(0.0, 0.0, ancho, alto),
        relleno: Some(t.papel),
        trazo: None,
        grosor: 0.0,
        radio: 0.0,
    });
    if t.banda {
        s.prims.push(Prim::Rect {
            caja: Caja::nueva(0.0, 0.0, ancho, BANDA),
            relleno: Some(t.acento),
            trazo: None,
            grosor: 0.0,
            radio: 0.0,
        });
    }
    let m = doc.pagina.margenes;
    let arriba = m.arriba.max(if t.banda { BANDA + 4.0 } else { 0.0 });
    let contenido = Caja::nueva(
        m.izquierda,
        arriba,
        (ancho - m.izquierda - m.derecha).max(10.0),
        (alto - arriba - m.abajo).max(10.0),
    );
    s.colocar(&doc.raiz, contenido, None, 0, 0);
    PaginaHecha {
        ancho,
        alto,
        papel: t.papel,
        familias: [
            t.letra_titulo.clone(),
            t.letra_cuerpo.clone(),
            crate::tema::CAVEAT.to_string(),
        ],
        prims: s.prims,
        cajas: s.cajas,
    }
}

/// **Lo que mide un bloque a lo alto** con ese ancho, si lo sabe. `None`:
/// se estira.
pub fn alto_natural(b: &Bloque, ancho: f32) -> Option<f32> {
    let lista = |items: &Vec<String>, vacias: u32| {
        Some((items.len() as u32 + vacias).max(1) as f32 * FILA_LISTA)
    };
    match &b.tipo {
        Tipo::Encabezado { nivel, texto, .. } => {
            let tam = tam_encabezado(*nivel);
            let lineas = (crate::texto::ancho_estimado(texto, tam) / ancho.max(1.0))
                .ceil()
                .clamp(1.0, 3.0);
            Some(tam * 1.35 * lineas + if *nivel <= 1 { 2.0 } else { 1.0 })
        }
        Tipo::Texto { texto, puntos, .. } => {
            let tam = pt(*puntos);
            let mut lineas = 0.0;
            for p in texto.split('\n') {
                lineas += (crate::texto::ancho_estimado(p, tam) / ancho.max(1.0))
                    .ceil()
                    .max(1.0);
            }
            Some(lineas * tam * crate::texto::INTERLINEADO + 1.0)
        }
        Tipo::Campo { puntos, .. } => Some(pt(*puntos) * 1.45),
        Tipo::Vinetas { items, vacias }
        | Tipo::Casillas { items, vacias }
        | Tipo::Numerada { items, vacias } => lista(items, *vacias),
        Tipo::Tabla { filas, .. } => Some((*filas).max(1) as f32 * FILA_TABLA),
        Tipo::Divisor { .. } => Some(4.0),
        Tipo::Espacio {} => Some(6.0),
        Tipo::Habitos { habitos, .. } => Some((habitos.len().max(3) + 1) as f32 * FILA_LISTA),
        Tipo::Animo { titulo } => Some(if titulo.is_empty() {
            16.0
        } else {
            16.0 + ROTULO
        }),
        Tipo::CalendarioMes { mini: true, .. } => Some(ancho.min(80.0) * 0.74),
        Tipo::Columna { hijos, hueco } => {
            let mut total = 0.0;
            for h in hijos {
                total += match h.tam {
                    Tam::Fijo(mm) => mm,
                    Tam::Fraccion(_) => return None,
                    Tam::Auto => alto_natural(h, ancho)?,
                };
            }
            Some(total + hueco * hijos.len().saturating_sub(1) as f32)
        }
        Tipo::Seccion {
            hijos,
            titulo,
            recuadro,
        } => {
            let col = Bloque::nuevo(Tipo::Columna {
                hijos: hijos.clone(),
                hueco: 2.0,
            });
            let dentro = if *recuadro { 6.0 } else { 0.0 };
            let rotulo = if titulo.is_empty() { 0.0 } else { ROTULO };
            Some(alto_natural(&col, ancho - dentro)? + rotulo + dentro)
        }
        Tipo::Fila { hijos, hueco } => {
            if hijos.is_empty() {
                return Some(10.0);
            }
            let w = (ancho - hueco * (hijos.len() - 1) as f32) / hijos.len() as f32;
            let mut m: f32 = 0.0;
            for h in hijos {
                m = m.max(alto_natural(h, w)?);
            }
            Some(m)
        }
        Tipo::Pila { hijos } => {
            let mut m: f32 = 0.0;
            for h in hijos {
                m = m.max(alto_natural(h, ancho)?);
            }
            Some(m.max(6.0))
        }
        Tipo::Rejilla {
            hijos,
            columnas,
            hueco,
        } => {
            let n = (*columnas).clamp(1, 12) as usize;
            if hijos.is_empty() {
                return Some(20.0);
            }
            let w = (ancho - hueco * (n - 1) as f32) / n as f32;
            let mut total = 0.0;
            for fila in hijos.chunks(n) {
                let mut m: f32 = 0.0;
                for h in fila {
                    m = m.max(alto_natural(h, w)?);
                }
                total += m;
            }
            let filas = hijos.len().div_ceil(n);
            Some(total + hueco * filas.saturating_sub(1) as f32)
        }
        _ => None,
    }
}

/// **Lo que mide a lo ancho** (en una fila), si lo sabe.
pub fn ancho_natural(b: &Bloque) -> Option<f32> {
    match &b.tipo {
        Tipo::Espacio {} => Some(6.0),
        Tipo::Divisor { .. } => Some(4.0),
        Tipo::Campo {
            puntos,
            antes,
            despues,
            formato,
            ..
        } if !matches!(
            formato,
            crate::locale::FormatoFecha::Largo | crate::locale::FormatoFecha::Completo
        ) =>
        {
            Some(
                crate::texto::ancho_estimado(&format!("{antes}00/00/0000{despues}"), pt(*puntos))
                    + 2.0,
            )
        }
        Tipo::Animo { .. } => Some(60.0),
        Tipo::CalendarioMes { mini: true, .. } => Some(56.0),
        _ => None,
    }
}

fn tam_encabezado(nivel: u32) -> f32 {
    match nivel {
        0 | 1 => pt(26.0),
        2 => pt(16.0),
        _ => pt(10.0),
    }
}

/// **Reparte** `largo` entre unos hijos segun sus tamaños.
pub fn repartir(hijos: &[Bloque], largo: f32, hueco: f32, eje: Eje, cruzado: f32) -> Vec<f32> {
    let n = hijos.len();
    if n == 0 {
        return Vec::new();
    }
    let libre = (largo - hueco * (n - 1) as f32).max(0.0);
    let mut fijos = vec![None; n];
    let mut fr = vec![0.0f32; n];
    for (i, h) in hijos.iter().enumerate() {
        match h.tam {
            Tam::Fijo(mm) => fijos[i] = Some(mm),
            Tam::Fraccion(f) => fr[i] = f.max(0.01),
            Tam::Auto => {
                let natural = match eje {
                    Eje::Vertical => alto_natural(h, cruzado),
                    Eje::Horizontal => ancho_natural(h),
                };
                match natural {
                    Some(mm) => fijos[i] = Some(mm),
                    None => fr[i] = 1.0,
                }
            }
        }
    }
    let suma_fijos: f32 = fijos.iter().flatten().sum();
    let suma_fr: f32 = fr.iter().sum();
    // Lo fijo no cabe: se encoge todo a la par, dejando algo a lo que se
    // estira (10 mm cada uno) si puede.
    let reserva = if suma_fr > 0.0 {
        (10.0 * fr.iter().filter(|f| **f > 0.0).count() as f32).min(libre * 0.5)
    } else {
        0.0
    };
    let escala = if suma_fijos > libre - reserva && suma_fijos > 0.0 {
        ((libre - reserva) / suma_fijos).max(0.0)
    } else {
        1.0
    };
    let resto = (libre - suma_fijos * escala).max(0.0);
    (0..n)
        .map(|i| match fijos[i] {
            Some(mm) => mm * escala,
            None if suma_fr > 0.0 => resto * fr[i] / suma_fr,
            None => 0.0,
        })
        .collect()
}

struct Salida<'a> {
    prims: Vec<Prim>,
    cajas: Vec<CajaBloque>,
    t: &'a Tema,
    ctx: &'a Contexto,
}

impl Salida<'_> {
    fn colocar(&mut self, b: &Bloque, caja: Caja, padre: Option<u64>, indice: usize, hondo: usize) {
        if hondo > MAX_HONDO || caja.w <= 0.1 || caja.h <= 0.1 {
            return;
        }
        let hijos = b.tipo.hijos().map_or(0, Vec::len);
        self.cajas.push(CajaBloque {
            id: b.id,
            caja,
            padre,
            indice,
            profundidad: hondo,
            eje: b.tipo.eje(),
            hijos,
        });
        let acento = b.acento.unwrap_or(self.t.acento);
        match &b.tipo {
            Tipo::Fila { hijos, hueco } => {
                let anchos = repartir(hijos, caja.w, *hueco, Eje::Horizontal, caja.h);
                let mut x = caja.x;
                for (i, (h, w)) in hijos.iter().zip(anchos).enumerate() {
                    self.colocar(
                        h,
                        Caja::nueva(x, caja.y, w, caja.h),
                        Some(b.id),
                        i,
                        hondo + 1,
                    );
                    x += w + hueco;
                }
            }
            Tipo::Columna { hijos, hueco } => self.columna(b.id, hijos, caja, *hueco, hondo),
            Tipo::Pila { hijos } => {
                for (i, h) in hijos.iter().enumerate() {
                    self.colocar(h, caja, Some(b.id), i, hondo + 1);
                }
            }
            Tipo::Rejilla {
                hijos,
                columnas,
                hueco,
            } => self.rejilla(
                b.id,
                hijos,
                (*columnas).clamp(1, 12) as usize,
                *hueco,
                caja,
                hondo,
            ),
            Tipo::Seccion {
                titulo,
                hijos,
                recuadro,
            } => {
                let mut dentro = caja;
                if *recuadro {
                    self.prims.push(Prim::Rect {
                        caja,
                        relleno: None,
                        trazo: Some(self.t.borde),
                        grosor: self.t.grosor,
                        radio: 2.5,
                    });
                    dentro = caja.dentro(3.0);
                }
                if !titulo.is_empty() {
                    let (r, resto) = dentro.partir_arriba(ROTULO);
                    self.rotulo(r, titulo, acento);
                    dentro = resto;
                }
                self.columna(b.id, hijos, dentro, 2.0, hondo);
            }
            Tipo::Encabezado {
                texto,
                nivel,
                alineacion,
            } => self.encabezado(caja, texto, *nivel, *alineacion, acento),
            Tipo::Texto {
                texto,
                puntos,
                alineacion,
            } => {
                let texto = self.ctx.rellenar(texto);
                self.texto(
                    caja,
                    &texto,
                    pt(*puntos),
                    Letra::Cuerpo,
                    false,
                    self.t.tinta,
                    *alineacion,
                    true,
                );
            }
            Tipo::Notas { titulo, pauta } => {
                let mut zona = caja;
                if !titulo.is_empty() {
                    let (r, resto) = caja.partir_arriba(ROTULO);
                    self.rotulo(r, titulo, acento);
                    zona = resto;
                }
                self.pauta(zona, *pauta, 8.0);
            }
            Tipo::Lineas { pauta, paso } => self.pauta(caja, *pauta, paso.clamp(3.0, 30.0)),
            Tipo::Vinetas { items, vacias } => {
                self.lista(caja, items, *vacias, Marca::Vineta, acento)
            }
            Tipo::Casillas { items, vacias } => {
                self.lista(caja, items, *vacias, Marca::Casilla, acento)
            }
            Tipo::Numerada { items, vacias } => {
                self.lista(caja, items, *vacias, Marca::Numero, acento)
            }
            Tipo::Tabla {
                filas,
                columnas,
                cabeceras,
            } => self.tabla(
                caja,
                (*filas).clamp(1, 80),
                (*columnas).clamp(1, 20),
                cabeceras,
            ),
            Tipo::Divisor { trazo } => {
                let y = caja.y + caja.h / 2.0;
                let (color, grosor, raya) = match trazo {
                    Trazo::Continuo => (self.t.borde, self.t.grosor, Raya::Continua),
                    Trazo::Discontinuo => (self.t.borde, self.t.grosor, Raya::Discontinua),
                    Trazo::Punteado => (self.t.borde, self.t.grosor * 1.6, Raya::Punteada),
                    Trazo::Acento => (acento, self.t.grosor * 3.0, Raya::Continua),
                };
                if caja.w >= caja.h {
                    self.linea((caja.x, y), (caja.derecha(), y), color, grosor, raya);
                } else {
                    let x = caja.x + caja.w / 2.0;
                    self.linea((x, caja.y), (x, caja.abajo()), color, grosor, raya);
                }
            }
            Tipo::Espacio {} => {}
            Tipo::Imagen { etiqueta } => self.imagen(caja, etiqueta),
            Tipo::Campo {
                campo,
                formato,
                antes,
                despues,
                puntos,
                alineacion,
                negrita,
            } => {
                let v = format!(
                    "{antes}{}{despues}",
                    capital(&self.ctx.valor(*campo, *formato))
                );
                let tam = pt(*puntos);
                let letra = if tam >= pt(20.0) {
                    Letra::Titulo
                } else {
                    Letra::Cuerpo
                };
                let color = if *negrita || tam >= pt(16.0) {
                    self.t.tinta
                } else {
                    self.t.rotulo
                };
                // Abajo, para que case con la linea de un titulo a su lado.
                let c = Caja::nueva(caja.x, caja.y, caja.w, (caja.h - 1.6).max(tam * 1.3));
                self.prims.push(Prim::Texto {
                    caja: c,
                    texto: v,
                    tam,
                    letra,
                    negrita: *negrita,
                    color,
                    al: *alineacion,
                    v: Vertical::Abajo,
                    partir: false,
                });
            }
            Tipo::CalendarioMes { mes, mini } => {
                let fecha = leer_mes(mes).unwrap_or(self.ctx.fecha);
                self.calendario_mes(caja, fecha, *mini, acento);
            }
            Tipo::CalendarioSemana { fin_de_semana } => self.semana(caja, *fin_de_semana, acento),
            Tipo::Horario {
                desde,
                hasta,
                paso,
                dias,
            } => self.horario(caja, *desde, *hasta, *paso, *dias),
            Tipo::Habitos { habitos, dias } => self.habitos(caja, habitos, *dias, acento),
            Tipo::Animo { titulo } => self.animo(caja, titulo, acento),
            Tipo::Prioridades { niveles } => self.prioridades(caja, niveles),
            Tipo::Cornell { titulo, etiquetas } => self.cornell(caja, titulo, etiquetas, acento),
            Tipo::Reunion { titulo, etiquetas } => self.reunion(caja, titulo, etiquetas, acento),
        }
    }

    fn columna(&mut self, id: u64, hijos: &[Bloque], caja: Caja, hueco: f32, hondo: usize) {
        let altos = repartir(hijos, caja.h, hueco, Eje::Vertical, caja.w);
        let mut y = caja.y;
        for (i, (h, alto)) in hijos.iter().zip(altos).enumerate() {
            self.colocar(
                h,
                Caja::nueva(caja.x, y, caja.w, alto),
                Some(id),
                i,
                hondo + 1,
            );
            y += alto + hueco;
        }
    }

    fn rejilla(
        &mut self,
        id: u64,
        hijos: &[Bloque],
        n: usize,
        hueco: f32,
        caja: Caja,
        hondo: usize,
    ) {
        if hijos.is_empty() {
            return;
        }
        let w = ((caja.w - hueco * (n - 1) as f32) / n as f32).max(0.0);
        let filas: Vec<&[Bloque]> = hijos.chunks(n).collect();
        // Cada fila, lo que pida su hijo mas alto; si alguno se estira, todas
        // a partes iguales.
        let naturales: Option<Vec<f32>> = filas
            .iter()
            .map(|f| {
                f.iter()
                    .map(|h| match h.tam {
                        Tam::Fijo(mm) => Some(mm),
                        _ => alto_natural(h, w),
                    })
                    .try_fold(0.0f32, |m, x| x.map(|x| m.max(x)))
            })
            .collect();
        let libre = (caja.h - hueco * (filas.len() - 1) as f32).max(0.0);
        let altos = match naturales {
            Some(v) if v.iter().sum::<f32>() <= libre => v,
            _ => vec![libre / filas.len() as f32; filas.len()],
        };
        let mut y = caja.y;
        for (fi, (fila, alto)) in filas.iter().zip(altos).enumerate() {
            for (ci, h) in fila.iter().enumerate() {
                let x = caja.x + (w + hueco) * ci as f32;
                self.colocar(
                    h,
                    Caja::nueva(x, y, w, alto),
                    Some(id),
                    fi * n + ci,
                    hondo + 1,
                );
            }
            y += alto + hueco;
        }
    }

    // --- El lapiz ---------------------------------------------------------

    fn linea(&mut self, a: (f32, f32), b: (f32, f32), color: Color, grosor: f32, raya: Raya) {
        self.prims.push(Prim::Linea {
            a,
            b,
            color,
            grosor,
            raya,
        });
    }

    fn rect(&mut self, caja: Caja, relleno: Option<Color>, trazo: Option<Color>, radio: f32) {
        self.prims.push(Prim::Rect {
            caja,
            relleno,
            trazo,
            grosor: self.t.grosor,
            radio,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn texto(
        &mut self,
        caja: Caja,
        texto: &str,
        tam: f32,
        letra: Letra,
        negrita: bool,
        color: Color,
        al: Alineacion,
        partir: bool,
    ) {
        if texto.trim().is_empty() {
            return;
        }
        self.prims.push(Prim::Texto {
            caja,
            texto: texto.to_string(),
            tam,
            letra,
            negrita,
            color,
            al,
            v: if partir {
                Vertical::Arriba
            } else {
                Vertical::Centro
            },
            partir,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn texto_en(
        &mut self,
        caja: Caja,
        texto: &str,
        tam: f32,
        color: Color,
        al: Alineacion,
        v: Vertical,
        negrita: bool,
    ) {
        if texto.trim().is_empty() {
            return;
        }
        self.prims.push(Prim::Texto {
            caja,
            texto: texto.to_string(),
            tam,
            letra: Letra::Cuerpo,
            negrita,
            color,
            al,
            v,
            partir: false,
        });
    }

    /// El rotulo de una seccion: en mayusculas pequeñas del acento, con su
    /// raya debajo.
    fn rotulo(&mut self, caja: Caja, titulo: &str, acento: Color) {
        let titulo = self.ctx.rellenar(titulo);
        let t = Caja::nueva(caja.x, caja.y, caja.w, caja.h - 1.6);
        self.texto_en(
            t,
            &titulo.to_uppercase(),
            pt(8.5),
            acento,
            Alineacion::Izquierda,
            Vertical::Centro,
            true,
        );
        let y = caja.abajo() - 1.2;
        self.linea(
            (caja.x, y),
            (caja.derecha(), y),
            self.t.raya,
            self.t.grosor,
            Raya::Continua,
        );
        let w = (caja.w * 0.18).clamp(8.0, 30.0);
        self.linea(
            (caja.x, y),
            (caja.x + w, y),
            acento,
            self.t.grosor * 2.6,
            Raya::Continua,
        );
    }

    fn encabezado(&mut self, caja: Caja, texto: &str, nivel: u32, al: Alineacion, acento: Color) {
        let texto = capital(&self.ctx.rellenar(texto));
        match nivel {
            0 | 1 => {
                let manuscrita = self.t.letra_titulo == crate::tema::CAVEAT;
                let tam = tam_encabezado(1) * if manuscrita { 1.25 } else { 1.0 };
                let color = if manuscrita { acento } else { self.t.tinta };
                self.texto(caja, &texto, tam, Letra::Titulo, true, color, al, true);
            }
            2 => {
                self.texto(
                    caja,
                    &texto,
                    tam_encabezado(2),
                    Letra::Titulo,
                    true,
                    self.t.tinta,
                    al,
                    true,
                );
            }
            _ => {
                let c = Caja::nueva(caja.x, caja.y, caja.w, caja.h);
                self.texto_en(
                    c,
                    &texto.to_uppercase(),
                    tam_encabezado(3),
                    acento,
                    al,
                    Vertical::Centro,
                    true,
                );
            }
        }
    }

    fn pauta(&mut self, caja: Caja, pauta: Pauta, paso: f32) {
        let t = self.t;
        match pauta {
            Pauta::Rayado => {
                let mut y = caja.y + paso;
                while y <= caja.abajo() + 0.01 {
                    self.linea(
                        (caja.x, y),
                        (caja.derecha(), y),
                        t.raya,
                        t.grosor,
                        Raya::Continua,
                    );
                    y += paso;
                }
            }
            Pauta::Puntos => {
                let p = 5.0;
                let mut centros = Vec::new();
                let nx = (caja.w / p).floor() as usize;
                let ny = (caja.h / p).floor() as usize;
                let x0 = caja.x + (caja.w - nx as f32 * p) / 2.0;
                let y0 = caja.y + (caja.h - ny as f32 * p) / 2.0;
                for j in 0..=ny {
                    for i in 0..=nx {
                        centros.push((x0 + i as f32 * p, y0 + j as f32 * p));
                    }
                }
                self.prims.push(Prim::Puntos {
                    centros,
                    radio: 0.32,
                    color: t.borde,
                });
            }
            Pauta::Cuadricula => {
                let p = 5.0;
                let nx = (caja.w / p).floor() as usize;
                let ny = (caja.h / p).floor() as usize;
                let x0 = caja.x + (caja.w - nx as f32 * p) / 2.0;
                let y0 = caja.y + (caja.h - ny as f32 * p) / 2.0;
                let (x1, y1) = (x0 + nx as f32 * p, y0 + ny as f32 * p);
                let fina = t.raya.mezcla(t.papel, 0.35);
                for i in 0..=nx {
                    let x = x0 + i as f32 * p;
                    self.linea((x, y0), (x, y1), fina, t.grosor * 0.8, Raya::Continua);
                }
                for j in 0..=ny {
                    let y = y0 + j as f32 * p;
                    self.linea((x0, y), (x1, y), fina, t.grosor * 0.8, Raya::Continua);
                }
            }
            Pauta::Blanco => {}
        }
    }

    fn lista(&mut self, caja: Caja, items: &[String], vacias: u32, marca: Marca, acento: Color) {
        let n = (items.len() + vacias as usize).max(1);
        let alto = (caja.h / n as f32).clamp(3.0, FILA_LISTA * 1.6);
        let t = self.t;
        for i in 0..n {
            let y = caja.y + alto * i as f32;
            if y + alto > caja.abajo() + 0.5 {
                break;
            }
            let fila = Caja::nueva(caja.x, y, caja.w, alto);
            let cy = y + alto * 0.55;
            let x_texto = caja.x + 7.0;
            match marca {
                Marca::Vineta => self.prims.push(Prim::Circulo {
                    centro: (caja.x + 2.2, cy),
                    radio: 0.85,
                    relleno: Some(acento),
                    trazo: None,
                    grosor: 0.0,
                }),
                Marca::Casilla => {
                    let l = 3.6;
                    self.prims.push(Prim::Rect {
                        caja: Caja::nueva(caja.x + 0.4, cy - l / 2.0, l, l),
                        relleno: Some(t.papel),
                        trazo: Some(t.rotulo.mezcla(t.papel, 0.25)),
                        grosor: t.grosor * 1.4,
                        radio: 0.8,
                    });
                }
                Marca::Numero => {
                    let c = Caja::nueva(caja.x, y, 6.0, alto * 1.1);
                    self.texto_en(
                        c,
                        &format!("{}.", i + 1),
                        pt(9.0),
                        acento,
                        Alineacion::Izquierda,
                        Vertical::Centro,
                        true,
                    );
                }
            }
            let base = fila.abajo() - 0.6;
            self.linea(
                (x_texto, base),
                (caja.derecha(), base),
                t.raya,
                t.grosor,
                Raya::Continua,
            );
            if let Some(s) = items.get(i) {
                let s = self.ctx.rellenar(s);
                let c = Caja::nueva(
                    x_texto,
                    y + alto * 0.12,
                    caja.derecha() - x_texto,
                    alto * 0.86,
                );
                self.texto_en(
                    c,
                    &s,
                    pt(10.5),
                    t.tinta,
                    Alineacion::Izquierda,
                    Vertical::Centro,
                    false,
                );
            }
        }
    }

    fn tabla(&mut self, caja: Caja, filas: u32, columnas: u32, cabeceras: &[String]) {
        let t = self.t;
        let alto = caja.h / filas as f32;
        let ancho = caja.w / columnas as f32;
        let con_cabecera = cabeceras.iter().any(|c| !c.trim().is_empty());
        if con_cabecera {
            self.rect(
                Caja::nueva(caja.x, caja.y, caja.w, alto),
                Some(t.suave),
                None,
                0.0,
            );
            for (i, c) in cabeceras.iter().take(columnas as usize).enumerate() {
                let c = self.ctx.rellenar(c);
                let celda = Caja::nueva(caja.x + ancho * i as f32 + 2.0, caja.y, ancho - 4.0, alto);
                self.texto_en(
                    celda,
                    &c.to_uppercase(),
                    pt(8.0),
                    t.rotulo,
                    Alineacion::Izquierda,
                    Vertical::Centro,
                    true,
                );
            }
        }
        for j in 1..filas {
            let y = caja.y + alto * j as f32;
            let color = if j == 1 && con_cabecera {
                t.borde
            } else {
                t.raya
            };
            self.linea(
                (caja.x, y),
                (caja.derecha(), y),
                color,
                t.grosor,
                Raya::Continua,
            );
        }
        for i in 1..columnas {
            let x = caja.x + ancho * i as f32;
            self.linea(
                (x, caja.y),
                (x, caja.abajo()),
                t.raya,
                t.grosor,
                Raya::Continua,
            );
        }
        self.prims.push(Prim::Rect {
            caja,
            relleno: None,
            trazo: Some(t.borde),
            grosor: t.grosor * 1.2,
            radio: 1.6,
        });
    }

    fn imagen(&mut self, caja: Caja, etiqueta: &str) {
        let t = self.t;
        self.prims.push(Prim::Rect {
            caja,
            relleno: Some(t.suave.con_alfa(150)),
            trazo: Some(t.borde),
            grosor: t.grosor * 1.2,
            radio: 2.0,
        });
        // Un sol y dos montes, en el centro.
        let (cx, cy) = caja.centro();
        let l = (caja.w.min(caja.h) * 0.22).clamp(4.0, 22.0);
        let color = t.rotulo.mezcla(t.papel, 0.35);
        self.prims.push(Prim::Circulo {
            centro: (cx + l * 0.45, cy - l * 0.45),
            radio: l * 0.16,
            relleno: Some(color),
            trazo: None,
            grosor: 0.0,
        });
        self.prims.push(Prim::Poli {
            puntos: vec![
                (cx - l, cy + l * 0.5),
                (cx - l * 0.3, cy - l * 0.25),
                (cx + l * 0.1, cy + l * 0.15),
                (cx + l * 0.45, cy - l * 0.05),
                (cx + l, cy + l * 0.5),
            ],
            color,
            grosor: t.grosor * 2.4,
        });
        if !etiqueta.is_empty() {
            let c = Caja::nueva(caja.x + 2.0, cy + l * 0.6, caja.w - 4.0, pt(9.0) * 1.6);
            self.texto_en(
                c,
                etiqueta,
                pt(9.0),
                t.rotulo,
                Alineacion::Centro,
                Vertical::Centro,
                false,
            );
        }
    }

    fn calendario_mes(&mut self, caja: Caja, fecha: Fecha, mini: bool, acento: Color) {
        let t = self.t;
        let loc = self.ctx.locale;
        let filas = fecha::rejilla_del_mes(fecha.ano, fecha.mes, loc.primer_dia);
        let titulo_alto = if mini { pt(10.0) * 1.8 } else { pt(15.0) * 1.9 };
        let (cab, resto) = caja.partir_arriba(titulo_alto);
        let titulo = loc.titulo_mes(fecha);
        if mini {
            self.texto_en(
                cab,
                &titulo,
                pt(10.0),
                t.tinta,
                Alineacion::Centro,
                Vertical::Centro,
                true,
            );
        } else {
            self.texto(
                cab,
                &titulo,
                pt(15.0),
                Letra::Titulo,
                true,
                t.tinta,
                Alineacion::Izquierda,
                false,
            );
        }
        let dias_alto = if mini { pt(7.0) * 1.9 } else { pt(8.0) * 2.4 };
        let (dias, rejilla) = resto.partir_arriba(dias_alto);
        let w = caja.w / 7.0;
        let orden = loc.semana_ordenada();
        for (i, d) in orden.iter().enumerate() {
            let c = Caja::nueva(caja.x + w * i as f32, dias.y, w, dias.h);
            let nombre = if mini {
                loc.dias_letra[*d as usize].to_string()
            } else {
                loc.dias_cortos[*d as usize].to_uppercase()
            };
            let color = if *d >= 5 { acento } else { t.rotulo };
            self.texto_en(
                c,
                &nombre,
                if mini { pt(7.0) } else { pt(8.0) },
                color,
                Alineacion::Centro,
                Vertical::Centro,
                true,
            );
        }
        if filas.is_empty() || rejilla.h <= 1.0 {
            return;
        }
        let h = rejilla.h / filas.len() as f32;
        let hoy = self.ctx.fecha;
        if !mini {
            // Los fines de semana, con el acento suave de fondo.
            for (i, d) in orden.iter().enumerate() {
                if *d >= 5 {
                    let c = Caja::nueva(rejilla.x + w * i as f32, rejilla.y, w, rejilla.h);
                    self.rect(c, Some(t.suave.con_alfa(120)), None, 0.0);
                }
            }
        }
        for (j, fila) in filas.iter().enumerate() {
            for (i, f) in fila.iter().enumerate() {
                let Some(f) = f else { continue };
                let celda = Caja::nueva(rejilla.x + w * i as f32, rejilla.y + h * j as f32, w, h);
                let es_hoy = *f == hoy && fecha.mes == hoy.mes && fecha.ano == hoy.ano;
                if mini {
                    if es_hoy {
                        let (cx, cy) = celda.centro();
                        self.prims.push(Prim::Circulo {
                            centro: (cx, cy),
                            radio: (w.min(h) * 0.42).max(1.0),
                            relleno: Some(acento),
                            trazo: None,
                            grosor: 0.0,
                        });
                    }
                    let color = if es_hoy { t.papel } else { t.tinta };
                    self.texto_en(
                        celda,
                        &f.dia.to_string(),
                        pt(7.5),
                        color,
                        Alineacion::Centro,
                        Vertical::Centro,
                        es_hoy,
                    );
                } else {
                    let n = Caja::nueva(celda.x + 1.6, celda.y + 1.0, celda.w - 3.2, pt(9.0) * 1.5);
                    if es_hoy {
                        let w_num = crate::texto::ancho_estimado(&f.dia.to_string(), pt(9.0));
                        self.prims.push(Prim::Circulo {
                            centro: (n.x + w_num / 2.0, n.y + n.h / 2.0),
                            radio: pt(9.0) * 0.75,
                            relleno: Some(acento),
                            trazo: None,
                            grosor: 0.0,
                        });
                    }
                    let color = if es_hoy { t.papel } else { t.tinta };
                    self.texto_en(
                        n,
                        &f.dia.to_string(),
                        pt(9.0),
                        color,
                        Alineacion::Izquierda,
                        Vertical::Centro,
                        true,
                    );
                }
            }
        }
        if !mini {
            for j in 0..=filas.len() {
                let y = rejilla.y + h * j as f32;
                let color = if j == 0 { t.borde } else { t.raya };
                self.linea(
                    (rejilla.x, y),
                    (rejilla.derecha(), y),
                    color,
                    t.grosor,
                    Raya::Continua,
                );
            }
            for i in 1..7 {
                let x = rejilla.x + w * i as f32;
                self.linea(
                    (x, rejilla.y),
                    (x, rejilla.abajo()),
                    t.raya,
                    t.grosor,
                    Raya::Continua,
                );
            }
        }
    }

    /// Los dias de la semana de la pagina.
    fn dias_de_la_semana(&self, cuantos: u32) -> Vec<Fecha> {
        let loc = self.ctx.locale;
        let mut inicio = self.ctx.fecha.inicio_de_semana(loc.primer_dia);
        // Sin fin de semana y con la semana en domingo, de lunes a viernes.
        if cuantos == 5 && loc.primer_dia == 6 {
            inicio = inicio.mas_dias(1);
        }
        (0..cuantos)
            .map(|i| inicio.mas_dias(i64::from(i)))
            .collect()
    }

    fn semana(&mut self, caja: Caja, fin_de_semana: bool, acento: Color) {
        let t = self.t;
        let loc = self.ctx.locale;
        let dias = self.dias_de_la_semana(if fin_de_semana { 7 } else { 5 });
        let n = dias.len() as f32;
        let hueco = 3.0;
        if caja.w >= caja.h * 1.2 {
            // En columnas.
            let w = (caja.w - hueco * (n - 1.0)) / n;
            for (i, f) in dias.iter().enumerate() {
                let c = Caja::nueva(caja.x + (w + hueco) * i as f32, caja.y, w, caja.h);
                self.dia_de_semana(c, *f, loc, acento, true);
            }
            let _ = t;
        } else {
            // En filas.
            let h = (caja.h - hueco * (n - 1.0)) / n;
            for (i, f) in dias.iter().enumerate() {
                let c = Caja::nueva(caja.x, caja.y + (h + hueco) * i as f32, caja.w, h);
                self.dia_de_semana(c, *f, loc, acento, false);
            }
        }
    }

    fn dia_de_semana(
        &mut self,
        c: Caja,
        f: Fecha,
        loc: &crate::locale::Locale,
        acento: Color,
        columna: bool,
    ) {
        let t = self.t;
        let finde = f.dia_semana() >= 5;
        let color = if finde { acento } else { t.tinta };
        if columna {
            let (cab, resto) = c.partir_arriba(12.0);
            self.rect(cab, Some(t.suave), None, 1.6);
            let (a, b) = cab.partir_arriba(5.5);
            self.texto_en(
                a.dentro(0.5),
                &loc.dias_cortos[f.dia_semana() as usize].to_uppercase(),
                pt(7.5),
                t.rotulo,
                Alineacion::Centro,
                Vertical::Abajo,
                true,
            );
            self.texto(
                b,
                &f.dia.to_string(),
                pt(13.0),
                Letra::Titulo,
                true,
                color,
                Alineacion::Centro,
                false,
            );
            self.pauta(
                Caja::nueva(resto.x, resto.y + 1.0, resto.w, resto.h - 1.0),
                Pauta::Rayado,
                7.0,
            );
        } else {
            let ancho_rotulo = (c.w * 0.18).clamp(16.0, 30.0);
            let (izq, der) = c.partir_izquierda(ancho_rotulo);
            self.rect(izq, Some(t.suave), None, 1.6);
            let (a, b) = izq.dentro(1.0).partir_arriba(izq.h * 0.42);
            self.texto(
                a,
                &f.dia.to_string(),
                pt(16.0),
                Letra::Titulo,
                true,
                color,
                Alineacion::Centro,
                false,
            );
            self.texto_en(
                b,
                &capital(loc.dia(f.dia_semana())),
                pt(8.0),
                t.rotulo,
                Alineacion::Centro,
                Vertical::Arriba,
                true,
            );
            let lineas = Caja::nueva(der.x + 3.0, der.y, der.w - 3.0, der.h);
            self.pauta(lineas, Pauta::Rayado, (der.h / 4.0).clamp(6.0, 8.0));
            self.linea(
                (c.x, c.abajo()),
                (c.derecha(), c.abajo()),
                t.borde,
                t.grosor,
                Raya::Continua,
            );
        }
    }

    fn horario(&mut self, caja: Caja, desde: u32, hasta: u32, paso: u32, dias: u32) {
        let t = self.t;
        let loc = self.ctx.locale;
        let desde = desde.min(23);
        let hasta = hasta.clamp(desde + 1, 24);
        let paso = match paso {
            0..=20 => 15,
            21..=40 => 30,
            _ => 60,
        };
        let tramos = ((hasta - desde) * 60 / paso).max(1);
        let dias = dias.clamp(1, 7);
        let ancho_horas = 13.0;
        let (horas, zona) = caja.partir_izquierda(ancho_horas);
        let (cab, rejilla) = if dias > 1 {
            zona.partir_arriba(10.0)
        } else {
            (Caja::nueva(zona.x, zona.y, zona.w, 0.0), zona)
        };
        let horas = Caja::nueva(horas.x, rejilla.y, horas.w, rejilla.h);
        let h = rejilla.h / tramos as f32;
        for k in 0..=tramos {
            let y = rejilla.y + h * k as f32;
            let minutos = desde * 60 + k * paso;
            let en_punto = minutos % 60 == 0;
            let (color, raya) = if en_punto {
                (t.borde, Raya::Continua)
            } else {
                (t.raya, Raya::Discontinua)
            };
            self.linea(
                (rejilla.x, y),
                (rejilla.derecha(), y),
                color,
                t.grosor,
                raya,
            );
            if en_punto && k < tramos {
                let c = Caja::nueva(horas.x, y + 0.6, horas.w - 2.0, pt(8.0) * 1.4);
                self.texto_en(
                    c,
                    &format!("{:02}:00", minutos / 60),
                    pt(8.0),
                    t.rotulo,
                    Alineacion::Derecha,
                    Vertical::Arriba,
                    false,
                );
            }
        }
        if dias > 1 {
            let fechas = self.dias_de_la_semana(dias);
            let w = rejilla.w / dias as f32;
            for (i, f) in fechas.iter().enumerate() {
                let c = Caja::nueva(rejilla.x + w * i as f32, cab.y, w, cab.h - 1.0);
                if f.dia_semana() >= 5 {
                    self.rect(
                        Caja::nueva(c.x, rejilla.y, w, rejilla.h),
                        Some(t.suave.con_alfa(110)),
                        None,
                        0.0,
                    );
                }
                let s = format!(
                    "{} {}",
                    loc.dias_cortos[f.dia_semana() as usize].to_uppercase(),
                    f.dia
                );
                self.texto_en(
                    c,
                    &s,
                    pt(8.0),
                    t.rotulo,
                    Alineacion::Centro,
                    Vertical::Centro,
                    true,
                );
                if i > 0 {
                    let x = c.x;
                    self.linea(
                        (x, rejilla.y),
                        (x, rejilla.abajo()),
                        t.raya,
                        t.grosor,
                        Raya::Continua,
                    );
                }
            }
        }
        self.linea(
            (rejilla.x, rejilla.y),
            (rejilla.x, rejilla.abajo()),
            t.borde,
            t.grosor,
            Raya::Continua,
        );
    }

    fn habitos(&mut self, caja: Caja, habitos: &[String], dias: u32, acento: Color) {
        let t = self.t;
        let loc = self.ctx.locale;
        let f = self.ctx.fecha;
        let fechas: Vec<Fecha> = match dias {
            0 => (1..=fecha::dias_del_mes(f.ano, f.mes))
                .filter_map(|d| Fecha::nueva(f.ano, f.mes, d))
                .collect(),
            n => {
                let inicio = f.inicio_de_semana(loc.primer_dia);
                (0..n.clamp(1, 31))
                    .map(|i| inicio.mas_dias(i64::from(i)))
                    .collect()
            }
        };
        let filas = habitos.len().max(3);
        let alto = (caja.h / (filas + 1) as f32).min(9.0);
        let ancho_nombre = (caja.w * 0.3).clamp(24.0, 55.0);
        let zona = Caja::nueva(
            caja.x + ancho_nombre,
            caja.y,
            caja.w - ancho_nombre,
            alto * (filas + 1) as f32,
        );
        let w = zona.w / fechas.len().max(1) as f32;
        let letra = fechas.len() <= 10;
        for (i, d) in fechas.iter().enumerate() {
            let c = Caja::nueva(zona.x + w * i as f32, zona.y, w, alto);
            let s = if letra {
                loc.dias_letra[d.dia_semana() as usize].to_string()
            } else {
                d.dia.to_string()
            };
            let color = if d.dia_semana() >= 5 {
                acento
            } else {
                t.rotulo
            };
            self.texto_en(
                c,
                &s,
                pt(if letra { 8.0 } else { 6.5 }),
                color,
                Alineacion::Centro,
                Vertical::Centro,
                true,
            );
        }
        for j in 0..filas {
            let y = caja.y + alto * (j + 1) as f32;
            let fila = Caja::nueva(caja.x, y, ancho_nombre - 2.0, alto);
            if let Some(n) = habitos.get(j) {
                self.texto_en(
                    fila,
                    n,
                    pt(9.5),
                    t.tinta,
                    Alineacion::Izquierda,
                    Vertical::Centro,
                    false,
                );
            } else {
                self.linea(
                    (caja.x, y + alto - 1.5),
                    (caja.x + ancho_nombre - 4.0, y + alto - 1.5),
                    t.raya,
                    t.grosor,
                    Raya::Continua,
                );
            }
            let r = (w.min(alto) * 0.34).clamp(0.6, 2.4);
            for i in 0..fechas.len() {
                let cx = zona.x + w * (i as f32 + 0.5);
                self.prims.push(Prim::Circulo {
                    centro: (cx, y + alto / 2.0),
                    radio: r,
                    relleno: None,
                    trazo: Some(t.borde),
                    grosor: t.grosor,
                });
            }
            self.linea(
                (caja.x, y),
                (zona.derecha(), y),
                t.raya,
                t.grosor,
                Raya::Continua,
            );
        }
    }

    fn animo(&mut self, caja: Caja, titulo: &str, acento: Color) {
        let t = self.t;
        let mut zona = caja;
        if !titulo.is_empty() {
            let (r, resto) = caja.partir_arriba(ROTULO);
            self.rotulo(r, titulo, acento);
            zona = resto;
        }
        let n = 5.0;
        let r = (zona.h * 0.36).min(zona.w / n * 0.36).max(1.0);
        let paso = zona.w.min(r * 2.9 * n) / n;
        let x0 = zona.x + paso / 2.0;
        let cy = zona.y + (r + 2.5).min(zona.h / 2.0);
        for i in 0..5 {
            let cx = x0 + paso * i as f32;
            let color = t.rotulo.mezcla(acento, i as f32 / 4.0);
            self.prims.push(Prim::Circulo {
                centro: (cx, cy),
                radio: r,
                relleno: None,
                trazo: Some(color),
                grosor: t.grosor * 1.6,
            });
            for ojo in [-0.33, 0.33] {
                self.prims.push(Prim::Circulo {
                    centro: (cx + r * ojo, cy - r * 0.22),
                    radio: r * 0.09,
                    relleno: Some(color),
                    trazo: None,
                    grosor: 0.0,
                });
            }
            // La boca: de triste (-1) a feliz (+1).
            let curva = (i as f32 - 2.0) / 2.0;
            let puntos: Vec<(f32, f32)> = (0..=10)
                .map(|k| {
                    let u = k as f32 / 10.0 * 2.0 - 1.0;
                    (
                        cx + u * r * 0.45,
                        cy + r * 0.3 + curva * r * 0.22 * (1.0 - u * u),
                    )
                })
                .collect();
            self.prims.push(Prim::Poli {
                puntos,
                color,
                grosor: t.grosor * 1.6,
            });
        }
    }

    fn prioridades(&mut self, caja: Caja, niveles: &[String]) {
        let t = self.t;
        let colores = [
            Color::hex(0xd9534f),
            Color::hex(0xe0a030),
            Color::hex(0x4a9d6a),
            Color::hex(0x5d6b82),
        ];
        let n = niveles.len().clamp(1, 4);
        let hueco = 3.0;
        let w = (caja.w - hueco * (n - 1) as f32) / n as f32;
        for i in 0..n {
            let c = Caja::nueva(caja.x + (w + hueco) * i as f32, caja.y, w, caja.h);
            self.rect(c, None, Some(t.borde), 2.0);
            let (cab, resto) = c.dentro(2.5).partir_arriba(7.0);
            self.prims.push(Prim::Circulo {
                centro: (cab.x + 1.8, cab.y + cab.h / 2.0),
                radio: 1.4,
                relleno: Some(colores[i % colores.len()]),
                trazo: None,
                grosor: 0.0,
            });
            let nombre = niveles.get(i).map(String::as_str).unwrap_or("");
            let c2 = Caja::nueva(cab.x + 5.0, cab.y, cab.w - 5.0, cab.h);
            self.texto_en(
                c2,
                &nombre.to_uppercase(),
                pt(8.5),
                t.tinta,
                Alineacion::Izquierda,
                Vertical::Centro,
                true,
            );
            self.pauta(resto, Pauta::Rayado, 7.0);
        }
    }

    fn cornell(&mut self, caja: Caja, titulo: &str, et: &[String], acento: Color) {
        let t = self.t;
        let e = |i: usize| et.get(i).map(String::as_str).unwrap_or("");
        let mut zona = caja;
        if !titulo.is_empty() {
            let (cab, resto) = caja.partir_arriba(10.0);
            let titulo = self.ctx.rellenar(titulo);
            self.texto(
                cab,
                &titulo,
                pt(14.0),
                Letra::Titulo,
                true,
                t.tinta,
                Alineacion::Izquierda,
                false,
            );
            zona = resto;
        }
        let alto_resumen = (zona.h * 0.22).clamp(20.0, 60.0);
        let (arriba, resumen) = zona.partir_arriba(zona.h - alto_resumen);
        let (claves, notas) = arriba.partir_izquierda((arriba.w * 0.3).clamp(30.0, 70.0));
        // Las claves con el acento suave de fondo.
        self.rect(claves, Some(t.suave), None, 0.0);
        let rot = |s: &mut Self, c: Caja, txt: &str| {
            let r = Caja::nueva(c.x + 2.0, c.y + 1.0, c.w - 4.0, 6.0);
            s.texto_en(
                r,
                &txt.to_uppercase(),
                pt(8.0),
                acento,
                Alineacion::Izquierda,
                Vertical::Centro,
                true,
            );
        };
        rot(self, claves, e(0));
        rot(self, notas, e(1));
        self.pauta(
            Caja::nueva(notas.x + 3.0, notas.y + 7.0, notas.w - 3.0, notas.h - 7.0),
            Pauta::Rayado,
            7.5,
        );
        self.linea(
            (claves.derecha(), arriba.y),
            (claves.derecha(), arriba.abajo()),
            acento,
            t.grosor * 2.0,
            Raya::Continua,
        );
        self.linea(
            (zona.x, resumen.y),
            (zona.derecha(), resumen.y),
            acento,
            t.grosor * 2.0,
            Raya::Continua,
        );
        rot(self, resumen, e(2));
        self.pauta(
            Caja::nueva(resumen.x, resumen.y + 7.0, resumen.w, resumen.h - 7.0),
            Pauta::Rayado,
            7.5,
        );
    }

    fn reunion(&mut self, caja: Caja, titulo: &str, et: &[String], acento: Color) {
        let t = self.t;
        let e = |i: usize| et.get(i).cloned().unwrap_or_default();
        let mut zona = caja;
        if !titulo.is_empty() {
            let (cab, resto) = caja.partir_arriba(14.0);
            let titulo = self.ctx.rellenar(titulo);
            self.texto(
                cab,
                &titulo,
                pt(22.0),
                Letra::Titulo,
                true,
                t.tinta,
                Alineacion::Izquierda,
                false,
            );
            zona = resto;
        }
        // Fecha | asistentes, en una fila de dos campos con su raya.
        let (fila, resto) = zona.partir_arriba(12.0);
        let (izq, der) = fila.partir_izquierda(fila.w * 0.35);
        for (c, rot, valor) in [
            (
                izq,
                e(0),
                self.ctx.valor(
                    crate::modelo::QueCampo::Fecha,
                    crate::locale::FormatoFecha::Corto,
                ),
            ),
            (
                Caja::nueva(der.x + 4.0, der.y, der.w - 4.0, der.h),
                e(1),
                String::new(),
            ),
        ] {
            let (r, v) = c.partir_arriba(5.0);
            self.texto_en(
                r,
                &rot.to_uppercase(),
                pt(7.5),
                t.rotulo,
                Alineacion::Izquierda,
                Vertical::Centro,
                true,
            );
            self.texto_en(
                Caja::nueva(v.x, v.y, v.w, v.h - 1.0),
                &valor,
                pt(10.0),
                t.tinta,
                Alineacion::Izquierda,
                Vertical::Centro,
                false,
            );
            self.linea(
                (c.x, c.abajo() - 0.5),
                (c.derecha(), c.abajo() - 0.5),
                t.borde,
                t.grosor,
                Raya::Continua,
            );
        }
        let resto = Caja::nueva(resto.x, resto.y + 4.0, resto.w, resto.h - 4.0);
        let h_orden = (resto.h * 0.3).max(20.0);
        let h_tareas = (resto.h * 0.3).max(20.0);
        let (orden, resto) = resto.partir_arriba(h_orden);
        let (acuerdos, tareas) = resto.partir_arriba(resto.h - h_tareas);
        let seccion = |s: &mut Self, c: Caja, rot: &str| -> Caja {
            let (r, d) = c.partir_arriba(ROTULO);
            s.rotulo(r, rot, acento);
            Caja::nueva(d.x, d.y + 1.0, d.w, (d.h - 3.0).max(0.0))
        };
        let d = seccion(self, orden, &e(2));
        let n = ((d.h / FILA_LISTA).floor() as u32).max(1);
        self.lista(d, &[], n, Marca::Numero, acento);
        let d = seccion(self, acuerdos, &e(3));
        self.pauta(d, Pauta::Rayado, FILA_LISTA);
        let d = seccion(self, tareas, &e(4));
        let n = ((d.h / FILA_LISTA).floor() as u32).max(1);
        self.lista(d, &[], n, Marca::Casilla, acento);
    }
}

#[derive(Clone, Copy)]
enum Marca {
    Vineta,
    Casilla,
    Numero,
}

/// `2027-01` → el 1 de ese mes.
pub fn leer_mes(s: &str) -> Option<Fecha> {
    let (a, m) = s.trim().split_once('-')?;
    Fecha::nueva(a.trim().parse().ok()?, m.trim().parse().ok()?, 1)
}

/// La primera letra en mayuscula («noviembre» → «Noviembre»).
pub fn capital(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}
