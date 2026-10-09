//! **Donde va cada cosa** en las pestanas «Momentos» y «Estado» y en el
//! detalle de un momento (5-oct-2026), en funciones puras.
//!
//! Regla del proyecto: si algo se ve pero no responde, es que el pintado y
//! el clic usan cuentas distintas. Por eso estas cajas las usan las dos
//! cosas: el pintado pinta en ellas y apunta en ellas sus zonas de clic, y
//! las pruebas comprueban la geometria sin abrir ninguna ventana.
//!
//! Todo va en pixeles fisicos (ya multiplicado por la escala `s`) y, salvo
//! el detalle, con `y` contada desde lo alto del contenido: quien pinta le
//! suma donde empieza el area y le resta lo desplazado.

use pixpin_render::RectF;
use pixpin_timeline::archivo::AnioDelArchivo;
use pixpin_timeline::dias::{Dia, Mes};

/// Margen a los lados del contenido.
pub const MARGEN: f32 = 16.0;

// ------------------------------------------------------------ Momentos

/// La columna del numero del dia (en la captura de WeChat, ~120 px).
pub const COLUMNA_DIA: f32 = 104.0;
/// El lado de cada miniatura cuadrada (en la captura, ~124 px).
pub const MINI: f32 = 124.0;
pub const HUECO: f32 = 6.0;
const ALTO_ANIO: f32 = 70.0;
const ALTO_MES: f32 = 34.0;
const TRAS_DIA: f32 = 20.0;

#[derive(Debug, Clone, PartialEq)]
pub enum PiezaArchivo {
    /// El rotulo grande del ano; `y` es lo alto de su renglon.
    Anio { anio: i32, y: f32 },
    /// El rotulo pequeno del mes.
    Mes { mes: Mes, y: f32 },
    /// Un dia: la caja de su numero y la de cada miniatura, en el orden de
    /// sus momentos.
    Dia {
        dia: Dia,
        numero: RectF,
        minis: Vec<RectF>,
    },
    /// La raya de «no hay mas».
    Fin { y: f32 },
}

/// Cuantas miniaturas caben por renglon en un contenido de `ancho`.
pub fn minis_por_fila(ancho: f32, s: f32) -> usize {
    let libre = ancho - 2.0 * MARGEN * s - COLUMNA_DIA * s;
    (((libre + HUECO * s) / ((MINI + HUECO) * s)).floor() as usize).max(1)
}

/// Las piezas del archivo y el alto de todo.
pub fn archivo(anios: &[AnioDelArchivo<'_>], ancho: f32, s: f32) -> (Vec<PiezaArchivo>, f32) {
    let mut v = Vec::new();
    let por_fila = minis_por_fila(ancho, s);
    let x_minis = MARGEN * s + COLUMNA_DIA * s;
    let mut y = 12.0 * s;
    for a in anios {
        v.push(PiezaArchivo::Anio { anio: a.anio, y });
        y += ALTO_ANIO * s;
        for m in &a.meses {
            v.push(PiezaArchivo::Mes { mes: m.mes, y });
            y += ALTO_MES * s;
            for d in &m.dias {
                let n = d.momentos.len().max(1);
                let minis: Vec<RectF> = (0..d.momentos.len())
                    .map(|k| RectF {
                        x: x_minis + (k % por_fila) as f32 * (MINI + HUECO) * s,
                        y: y + (k / por_fila) as f32 * (MINI + HUECO) * s,
                        ancho: MINI * s,
                        alto: MINI * s,
                    })
                    .collect();
                let filas = n.div_ceil(por_fila);
                v.push(PiezaArchivo::Dia {
                    dia: d.dia,
                    numero: RectF {
                        x: MARGEN * s,
                        y,
                        ancho: (COLUMNA_DIA - 16.0) * s,
                        alto: 58.0 * s,
                    },
                    minis,
                });
                y += filas as f32 * (MINI + HUECO) * s - HUECO * s + TRAS_DIA * s;
            }
        }
    }
    if !anios.is_empty() {
        v.push(PiezaArchivo::Fin { y });
        y += 48.0 * s;
    }
    (v, y)
}

// -------------------------------------------------------------- Estado

/// Lo que se deja arriba de «Estado» antes de la primera tarjeta.
pub const PISTA_ESTADO: f32 = 12.0;
/// La columna del numero del mes.
pub const COLUMNA_MES: f32 = 116.0;
const PAD: f32 = 16.0;
/// Lo menos que mide la celda de un dia antes de pasar a menos por fila.
const CELDA_MIN: f32 = 60.0;
const RADIO: f32 = 24.0;
/// El alto de una fila sin ningun dia con algo: solo puntitos. Asi un mes
/// con tres dias no ocupa cuatro filas de numeros (lo vio un revisor).
const FILA_VACIA: f32 = 30.0;
/// La fila de las iniciales de la semana, encima de los dias.
const SEMANA: f32 = 24.0;

#[derive(Debug, Clone, PartialEq)]
pub struct CeldaDia {
    pub dia: Dia,
    /// Si el dia tiene algo (circulo y clic) o es solo un puntito.
    pub con_algo: bool,
    /// Toda la celda: numero y circulo. Es la zona del clic.
    pub caja: RectF,
    /// El centro del circulo (o del puntito) y su radio.
    pub centro: (f32, f32),
    pub radio: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TarjetaMes {
    pub mes: Mes,
    pub caja: RectF,
    pub celdas: Vec<CeldaDia>,
    /// Las iniciales de la semana (lunes a domingo) encima de sus columnas.
    pub semana: Vec<RectF>,
}

fn derecha_de_la_tarjeta(ancho: f32, s: f32) -> f32 {
    (ancho - 2.0 * MARGEN * s - COLUMNA_MES * s - 2.0 * PAD * s).max(CELDA_MIN * s)
}

/// Las tarjetas de los meses, una debajo de otra, y el alto de todo. Cada
/// mes llega con sus dias ya en el orden en que se ven (al reves) y si
/// tienen algo. `abajo` es lo que hay que dejar libre al final (la barra
/// flotante de los dias elegidos, que no debe tapar la ultima fila).
///
/// **Un calendario de verdad** (8-oct-2026, el usuario: «que sea llamada
/// calendario»): siete columnas de lunes a domingo con sus iniciales arriba,
/// y cada dia en la de su dia de la semana. Antes eran diez por fila, del
/// mas reciente al 1, como las tarjetas de estado de WeChat.
pub fn estado(meses: &[(Mes, Vec<(Dia, bool)>)], ancho: f32, s: f32, abajo: f32) -> (Vec<TarjetaMes>, f32) {
    let libre = derecha_de_la_tarjeta(ancho, s);
    let celda_w = libre / 7.0;
    let x_de = |col: usize| MARGEN * s + PAD * s + COLUMNA_MES * s + col as f32 * celda_w;
    let radio = (RADIO * s).min(celda_w / 2.0 - 5.0 * s).max(8.0 * s);
    let fila_llena = 22.0 * s + 6.0 * s + 2.0 * radio + 12.0 * s;
    let mut y = PISTA_ESTADO * s;
    let mut v = Vec::new();
    for (mes, dias) in meses {
        let y0 = y + PAD * s;
        let semana: Vec<RectF> = (0..7)
            .map(|k| RectF {
                x: x_de(k),
                y: y0,
                ancho: celda_w,
                alto: SEMANA * s,
            })
            .collect();
        let mut fy = y0 + SEMANA * s;
        let mut celdas = Vec::with_capacity(dias.len());
        // Del 1 al ultimo, cada uno en su semana y su columna.
        let mut orden = dias.clone();
        orden.sort_by_key(|(d, _)| d.numero());
        let hueco = mes.primero().dia_de_la_semana() as usize;
        let mut semanas: Vec<Vec<(usize, Dia, bool)>> = Vec::new();
        for (d, algo) in orden {
            let k = hueco + d.dia as usize - 1;
            let (fila, col) = (k / 7, k % 7);
            while semanas.len() <= fila {
                semanas.push(Vec::new());
            }
            semanas[fila].push((col, d, algo));
        }
        for fila in &semanas {
            let llena = fila.iter().any(|(_, _, algo)| *algo);
            let alto = if llena { fila_llena } else { FILA_VACIA * s };
            for &(col, d, algo) in fila {
                let algo = &algo;
                let d = &d;
                let c = RectF {
                    x: x_de(col),
                    y: fy,
                    ancho: celda_w,
                    alto,
                };
                let centro = if llena {
                    (c.x + celda_w / 2.0, c.y + 28.0 * s + radio)
                } else {
                    (c.x + celda_w / 2.0, c.y + alto / 2.0)
                };
                celdas.push(CeldaDia {
                    dia: *d,
                    con_algo: *algo,
                    caja: c,
                    centro,
                    radio: if *algo { radio } else { 2.0 * s },
                });
            }
            fy += alto;
        }
        let caja = RectF {
            x: MARGEN * s,
            y,
            ancho: ancho - 2.0 * MARGEN * s,
            alto: (fy - y + PAD * s).max(150.0 * s),
        };
        v.push(TarjetaMes {
            mes: *mes,
            caja,
            celdas,
            semana,
        });
        y += caja.alto + 12.0 * s;
    }
    (v, y + 24.0 * s + abajo)
}

// ------------------------------------------------------------ Historia

/// Lo que ocupa la franja de arriba de la historia (barras, fecha, ✕): por
/// debajo empiezan las zonas de pasar.
pub const ARRIBA_DETALLE: f32 = 70.0;
/// Lo que ocupa abajo (la chapita de estado y la nota de voz).
pub const ABAJO_HISTORIA: f32 = 150.0;

/// **La historia** de un momento o de una leccion (5-oct-2026, noche): como
/// las de Instagram, que es lo que pidio el usuario. La foto llena el
/// fondo, el texto va en el medio, las barritas de arriba dicen cuantas
/// fotos hay y un clic en el tercio izquierdo o derecho pasa a la anterior
/// o la siguiente.
#[derive(Debug, Clone, PartialEq)]
pub struct Historia {
    pub cerrar: RectF,
    /// Marcar el momento como leccion (en una leccion no hay: su gravedad
    /// es la chapita de abajo).
    pub leccion: RectF,
    /// Pinear la foto que se ve (solo con foto).
    pub pinear: Option<RectF>,
    /// Compartir como imagen.
    pub compartir: RectF,
    /// Una barrita por foto (solo con varias), y su zona de clic.
    pub barras: Vec<RectF>,
    /// Los tercios de pasar: anterior y siguiente.
    pub anterior: RectF,
    pub siguiente: RectF,
    /// Donde va el texto, centrado.
    pub texto: RectF,
    /// La nota de voz, grande y centrada abajo (solo con audio).
    pub voz: Option<RectF>,
    /// Donde se centra la fila de chapitas (estado, gravedad, «+1»).
    pub chapas_y: f32,
}

pub fn historia(w: f32, h: f32, s: f32, fotos: usize, con_voz: bool) -> Historia {
    let lado = 40.0 * s;
    let y = 22.0 * s;
    let cerrar = RectF {
        x: w - 16.0 * s - lado,
        y,
        ancho: lado,
        alto: lado,
    };
    let leccion = RectF {
        x: cerrar.x - 8.0 * s - lado,
        ..cerrar
    };
    let pinear = (fotos > 0).then_some(RectF {
        x: leccion.x - 8.0 * s - lado,
        ..cerrar
    });
    let compartir = RectF {
        x: pinear.unwrap_or(leccion).x - 8.0 * s - lado,
        ..cerrar
    };
    let barras = if fotos > 1 {
        let (x0, x1, hueco) = (12.0 * s, w - 12.0 * s, 4.0 * s);
        let ancho = (x1 - x0 - (fotos - 1) as f32 * hueco) / fotos as f32;
        (0..fotos)
            .map(|k| RectF {
                x: x0 + k as f32 * (ancho + hueco),
                y: 10.0 * s,
                ancho,
                alto: 3.0 * s,
            })
            .collect()
    } else {
        Vec::new()
    };
    let arriba = ARRIBA_DETALLE * s;
    let abajo = ABAJO_HISTORIA * s;
    let tercio = w / 3.0;
    let alto_pasar = (h - arriba - abajo).max(1.0);
    let anterior = RectF {
        x: 0.0,
        y: arriba,
        ancho: tercio,
        alto: alto_pasar,
    };
    let siguiente = RectF {
        x: w - tercio,
        ..anterior
    };
    let ancho_texto = (w - 64.0 * s).min(680.0 * s);
    let voz = con_voz.then_some(RectF {
        x: w / 2.0 - 150.0 * s,
        y: h - 34.0 * s - 56.0 * s,
        ancho: 300.0 * s,
        alto: 56.0 * s,
    });
    let chapas_y = voz.map_or(h - 50.0 * s, |v| v.y - 30.0 * s);
    Historia {
        cerrar,
        leccion,
        pinear,
        compartir,
        barras,
        anterior,
        siguiente,
        texto: RectF {
            x: (w - ancho_texto) / 2.0,
            y: arriba + 10.0 * s,
            ancho: ancho_texto,
            alto: (h - arriba - abajo - 20.0 * s).max(1.0),
        },
        voz,
        chapas_y,
    }
}

/// Los renglones de `texto` en un ancho, partiendo por palabras (y una
/// palabra que no cabe sola, por letras). `medir` da el ancho de un trozo.
/// Pura para probarla: el pintado le pasa la medida de DirectWrite.
pub fn renglones(texto: &str, ancho: f32, medir: &dyn Fn(&str) -> f32) -> Vec<String> {
    let mut v = Vec::new();
    for parrafo in texto.split('\n') {
        let mut linea = String::new();
        for palabra in parrafo.split(' ').filter(|p| !p.is_empty()) {
            let prueba = if linea.is_empty() {
                palabra.to_string()
            } else {
                format!("{linea} {palabra}")
            };
            if medir(&prueba) <= ancho {
                linea = prueba;
                continue;
            }
            if !linea.is_empty() {
                v.push(std::mem::take(&mut linea));
            }
            if medir(palabra) <= ancho {
                linea = palabra.to_string();
            } else {
                // Sin espacios que valgan: por letras.
                for c in palabra.chars() {
                    let mut p = linea.clone();
                    p.push(c);
                    if !linea.is_empty() && medir(&p) > ancho {
                        v.push(std::mem::take(&mut linea));
                        linea.push(c);
                    } else {
                        linea = p;
                    }
                }
            }
        }
        v.push(linea);
    }
    while v.last().is_some_and(String::is_empty) {
        v.pop();
    }
    v
}

/// Lo primero de `v` que cabe en `max` renglones; si sobra, el ultimo
/// acaba en «…».
pub fn cortar_renglones(mut v: Vec<String>, max: usize) -> Vec<String> {
    if v.len() > max {
        v.truncate(max.max(1));
        if let Some(u) = v.last_mut() {
            let mut t: String = u.trim_end().to_string();
            if t.chars().count() > 2 {
                t.pop();
            }
            t.push('…');
            *u = t;
        }
    }
    v
}

/// La caja del boton «Exportar» de la barra de arriba. `cabecera::pintar`
/// lo pone el ultimo de los propios, solo de icono, junto a la ✕: estas
/// cuentas son las suyas, y el menu de exportar cuelga de aqui.
pub fn caja_exportar(w: f32, s: f32) -> RectF {
    let lado = 40.0 * s;
    let y = (crate::cabecera::ALTO * s - lado) / 2.0;
    RectF {
        x: w - 16.0 * s - lado - 10.0 * s - lado,
        y,
        ancho: lado,
        alto: lado,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::ventanita::dentro;
    use pixpin_timeline::Momento;
    use pixpin_timeline::archivo;
    use pixpin_timeline::dias::DIA_MS;

    fn m(id: &str, dia: Dia, minuto: i64) -> Momento {
        Momento::nuevo(
            id.into(),
            dia.numero() * DIA_MS + minuto * 60_000,
            id.into(),
            String::new(),
        )
    }

    fn d(anio: i32, mes: u8, dia: u8) -> Dia {
        Dia { anio, mes, dia }
    }

    fn centro(r: RectF) -> (f32, f32) {
        (r.x + r.ancho / 2.0, r.y + r.alto / 2.0)
    }

    #[test]
    fn el_archivo_pone_ano_mes_y_dia_y_las_minis_saltan_de_renglon() {
        let mut v: Vec<Momento> = (0..7)
            .map(|k| m(&format!("a{k}"), d(2026, 10, 5), k))
            .collect();
        v.push(m("viejo", d(2025, 4, 20), 0));
        let a = archivo::agrupar(&v, 0);
        // 900 de ancho: caben cinco por renglon.
        assert_eq!(minis_por_fila(900.0, 1.0), 5);
        let (piezas, alto) = archivo(&a, 900.0, 1.0);
        assert!(matches!(piezas[0], PiezaArchivo::Anio { anio: 2026, .. }));
        assert!(matches!(piezas[1], PiezaArchivo::Mes { .. }));
        let PiezaArchivo::Dia { minis, numero, .. } = &piezas[2] else {
            panic!("se esperaba un dia")
        };
        assert_eq!(minis.len(), 7);
        // La sexta baja al segundo renglon, bajo la primera.
        assert_eq!(minis[5].x, minis[0].x);
        assert!(minis[5].y > minis[0].y);
        // El numero va a la izquierda de las miniaturas, sin pisarlas.
        assert!(numero.x + numero.ancho <= minis[0].x);
        assert!(matches!(piezas.last(), Some(PiezaArchivo::Fin { .. })));
        assert!(alto > minis[6].y + minis[6].alto);
    }

    #[test]
    fn caso_negativo_ninguna_miniatura_se_sale_por_la_derecha() {
        let v: Vec<Momento> = (0..9)
            .map(|k| m(&format!("a{k}"), d(2026, 10, 5), k))
            .collect();
        let a = archivo::agrupar(&v, 0);
        for ancho in [400.0, 760.0, 1300.0] {
            let (piezas, _) = archivo(&a, ancho, 1.0);
            for p in &piezas {
                if let PiezaArchivo::Dia { minis, .. } = p {
                    for r in minis {
                        assert!(r.x + r.ancho <= ancho - MARGEN + 0.5, "se sale a {ancho}");
                    }
                }
            }
        }
        // Sin momentos no hay ni la raya del final.
        assert!(archivo(&[], 900.0, 1.0).0.is_empty());
    }

    #[test]
    fn el_calendario_pone_cada_dia_en_la_columna_de_su_dia_de_la_semana() {
        // Septiembre de 2026 empieza en martes.
        let mes = Mes { anio: 2026, mes: 9 };
        let dias: Vec<(Dia, bool)> = archivo::dias_al_reves(mes, d(2026, 10, 5))
            .into_iter()
            .map(|x| (x, true))
            .collect();
        let (t, _) = estado(&[(mes, dias)], 900.0, 1.0, 0.0);
        let (c, semana) = (&t[0].celdas, &t[0].semana);
        assert_eq!(semana.len(), 7);
        assert_eq!(c.len(), 30);
        assert_eq!(c[0].dia.dia, 1, "del 1 al ultimo");
        assert_eq!(c[0].caja.x, semana[1].x, "el 1 cae en martes");
        // El 7 es lunes: primera columna y semana siguiente.
        assert_eq!(c[6].dia.dia, 7);
        assert_eq!(c[6].caja.x, semana[0].x);
        assert!(c[6].caja.y > c[0].caja.y);
        // Los dias, debajo de las iniciales.
        assert!(c[0].caja.y >= semana[0].y + semana[0].alto);
        // Todo dentro de su tarjeta.
        for x in c {
            assert!(x.caja.y + x.caja.alto <= t[0].caja.y + t[0].caja.alto + 0.5);
            assert!(x.caja.x + x.caja.ancho <= t[0].caja.x + t[0].caja.ancho + 0.5);
        }
    }

    #[test]
    fn caso_negativo_el_clic_en_un_circulo_da_su_dia_y_no_el_de_al_lado() {
        let mes = Mes {
            anio: 2026,
            mes: 10,
        };
        let dias: Vec<(Dia, bool)> = archivo::dias_al_reves(mes, d(2026, 10, 5))
            .into_iter()
            .map(|x| (x, true))
            .collect();
        let (t, _) = estado(&[(mes, dias)], 900.0, 1.0, 0.0);
        let c = &t[0].celdas;
        let punto = c[1].centro;
        let tocadas: Vec<u8> = c
            .iter()
            .filter(|x| dentro(x.caja, punto))
            .map(|x| x.dia.dia)
            .collect();
        assert_eq!(tocadas, [2]);
    }

    #[test]
    fn las_filas_sin_nada_son_bajitas_y_la_barra_de_abajo_suma_al_final() {
        let mes = Mes { anio: 2026, mes: 8 };
        // Agosto con solo tres dias con algo, todos en la primera fila.
        let dias: Vec<(Dia, bool)> = archivo::dias_al_reves(mes, d(2026, 10, 5))
            .into_iter()
            .map(|x| (x, x.dia >= 29))
            .collect();
        let (t, alto) = estado(&[(mes, dias.clone())], 900.0, 1.0, 0.0);
        let c = &t[0].celdas;
        let de = |n: u8| c.iter().find(|x| x.dia.dia == n).unwrap();
        let alta = de(29).caja.alto;
        let baja = de(10).caja.alto;
        assert!(baja < alta / 2.0, "{baja} vs {alta}");
        assert!(de(29).con_algo && !de(10).con_algo);
        // Caso negativo: sin la barra no se reserva nada; con ella, su alto.
        let (_, con_barra) = estado(&[(mes, dias)], 900.0, 1.0, 76.0);
        assert_eq!(con_barra, alto + 76.0);
    }

    #[test]
    fn la_historia_pone_barras_solo_con_varias_fotos_y_los_tercios_debajo_de_los_botones() {
        let una = historia(900.0, 800.0, 1.0, 1, false);
        assert!(una.barras.is_empty() && una.voz.is_none());
        assert!(una.pinear.is_some());
        let tres = historia(900.0, 800.0, 1.0, 3, true);
        assert_eq!(tres.barras.len(), 3);
        assert!(tres.barras[2].x + tres.barras[2].ancho <= 900.0 - 11.5);
        // Los tercios no tapan los botones de arriba ni la nota de voz.
        let v = tres.voz.expect("voz");
        for b in [tres.cerrar, tres.leccion, tres.compartir, v] {
            assert!(!dentro(tres.siguiente, centro(b)) && !dentro(tres.anterior, centro(b)));
        }
        // El texto, centrado y entre los tercios de arriba y la voz.
        assert!((tres.texto.x + tres.texto.ancho / 2.0 - 450.0).abs() < 0.5);
        assert!(tres.texto.y + tres.texto.alto <= v.y);
        // Caso negativo: sin foto no hay nada que pinear, y los botones no
        // se pisan.
        let sin = historia(900.0, 800.0, 1.0, 0, false);
        assert!(sin.pinear.is_none());
        assert!(!dentro(sin.cerrar, centro(sin.leccion)));
        assert!(!dentro(sin.leccion, centro(sin.compartir)));
        assert!(!dentro(tres.anterior, centro(tres.siguiente)));
    }

    #[test]
    fn los_renglones_parten_por_palabras_y_cortan_con_puntos() {
        let medir = |t: &str| t.chars().count() as f32;
        assert_eq!(renglones("hola que tal", 8.0, &medir), ["hola que", "tal"]);
        assert_eq!(renglones("a\nb", 8.0, &medir), ["a", "b"]);
        // Una palabra que no cabe se parte por letras.
        assert_eq!(renglones("abcdefghij", 4.0, &medir), ["abcd", "efgh", "ij"]);
        let v = cortar_renglones(renglones("uno dos tres cuatro", 4.0, &medir), 2);
        assert_eq!(v, ["uno", "do…"]);
        // Caso negativo: lo que cabe no se toca.
        assert_eq!(cortar_renglones(vec!["a".into()], 3), ["a"]);
        assert!(renglones("", 10.0, &medir).is_empty());
    }
}
