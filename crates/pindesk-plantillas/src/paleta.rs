//! **La paleta de bloques** (lo que se arrastra a la pagina) y **las
//! propiedades** de un bloque (lo que sale en el panel de la derecha).
//!
//! Las propiedades salen de la misma tabla que repara el JSON
//! (`formato::campos_de`): cada campo con su clase y sus topes. Asi el panel,
//! la IA y el fichero no pueden discrepar sobre que se puede poner.

use serde_json::Value;

use crate::locale::FormatoFecha;
use crate::modelo::{Alineacion, Bloque, HUECO, Pauta, QueCampo, Tam, Tipo, Trazo, hacer};
use crate::textos::Textos;

/// Los grupos de la paleta y sus piezas, en el orden en que salen.
pub const PALETA: [(&str, &[&str]); 5] = [
    (
        "estructura",
        &["fila", "columna", "rejilla", "seccion", "pila"],
    ),
    (
        "texto",
        &[
            "encabezado",
            "texto",
            "notas",
            "lineas",
            "puntos",
            "cuadricula",
            "divisor",
            "espacio",
            "imagen",
        ],
    ),
    ("listas", &["vinetas", "casillas", "numerada", "tabla"]),
    (
        "fechas",
        &[
            "campo_fecha",
            "campo_dia_semana",
            "campo_semana",
            "campo_mes",
            "campo_pagina",
            "calendario_mes",
            "calendario_mini",
            "calendario_semana",
            "horario",
        ],
    ),
    (
        "especiales",
        &["habitos", "animo", "prioridades", "cornell", "reunion"],
    ),
];

/// Todas las piezas.
pub fn piezas() -> impl Iterator<Item = &'static str> {
    PALETA.iter().flat_map(|(_, p)| p.iter().copied())
}

/// **Un bloque nuevo** de una pieza, con los textos de fabrica en el idioma
/// de la interfaz. `None` si la pieza no existe.
pub fn nuevo(pieza: &str, t: &Textos) -> Option<Bloque> {
    let campo = |c: QueCampo, f: FormatoFecha, p: f32| hacer::campo(c, f, p);
    Some(match pieza {
        "fila" => hacer::fila(Vec::new()).con_tam(Tam::Fijo(40.0)),
        "columna" => hacer::columna(Vec::new()),
        "rejilla" => hacer::rejilla(2, Vec::new()),
        "pila" => Bloque::nuevo(Tipo::Pila { hijos: Vec::new() }),
        "seccion" => hacer::seccion(&t.t("plt-def-seccion"), Vec::new()),
        "encabezado" => hacer::encabezado(&t.t("plt-def-titulo"), 1),
        "texto" => hacer::texto(&t.t("plt-def-texto")),
        "notas" => hacer::notas(&t.t("plt-def-notas")),
        "lineas" => hacer::lineas(Pauta::Rayado),
        "puntos" => hacer::lineas(Pauta::Puntos),
        "cuadricula" => hacer::lineas(Pauta::Cuadricula),
        "divisor" => hacer::divisor(),
        "espacio" => Bloque::nuevo(Tipo::Espacio {}),
        "imagen" => Bloque::nuevo(Tipo::Imagen {
            etiqueta: t.t("plt-def-imagen"),
        })
        .con_tam(Tam::Fijo(50.0)),
        "vinetas" => hacer::vinetas(&[], 5),
        "casillas" => hacer::casillas(&[], 6),
        "numerada" => Bloque::nuevo(Tipo::Numerada {
            items: Vec::new(),
            vacias: 5,
        }),
        "tabla" => Bloque::nuevo(Tipo::Tabla {
            filas: 5,
            columnas: 3,
            cabeceras: Vec::new(),
        }),
        "campo_fecha" => campo(QueCampo::Fecha, FormatoFecha::Largo, 12.0),
        "campo_dia_semana" => campo(QueCampo::DiaSemana, FormatoFecha::Largo, 12.0),
        "campo_semana" => {
            let mut b = campo(QueCampo::Semana, FormatoFecha::Largo, 12.0);
            if let Tipo::Campo { negrita, .. } = &mut b.tipo {
                *negrita = true;
            }
            b
        }
        "campo_mes" => campo(QueCampo::Mes, FormatoFecha::Largo, 20.0),
        "campo_pagina" => {
            let mut b = campo(QueCampo::Pagina, FormatoFecha::Corto, 9.0);
            if let Tipo::Campo { alineacion, .. } = &mut b.tipo {
                *alineacion = Alineacion::Derecha;
            }
            b
        }
        "calendario_mes" => Bloque::nuevo(Tipo::CalendarioMes {
            mes: String::new(),
            mini: false,
        }),
        "calendario_mini" => Bloque::nuevo(Tipo::CalendarioMes {
            mes: String::new(),
            mini: true,
        }),
        "calendario_semana" => Bloque::nuevo(Tipo::CalendarioSemana {
            fin_de_semana: true,
        }),
        "horario" => hacer::horario(7, 22, 1),
        "habitos" => Bloque::nuevo(Tipo::Habitos {
            habitos: t.lista("plt-def-habitos"),
            dias: 0,
        }),
        "animo" => Bloque::nuevo(Tipo::Animo {
            titulo: t.t("plt-def-animo"),
        }),
        "prioridades" => Bloque::nuevo(Tipo::Prioridades {
            niveles: t.lista("plt-def-prioridades"),
        })
        .con_tam(Tam::Fijo(60.0)),
        "cornell" => Bloque::nuevo(Tipo::Cornell {
            titulo: String::new(),
            etiquetas: t.lista("plt-def-cornell"),
        }),
        "reunion" => Bloque::nuevo(Tipo::Reunion {
            titulo: t.t("plt-def-reunion"),
            etiquetas: t.lista("plt-def-reunion-rotulos"),
        }),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Las propiedades
// ---------------------------------------------------------------------------

/// **El valor de una propiedad**, con lo que hace falta para editarlo.
#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Numero {
        valor: f64,
        min: f64,
        max: f64,
        paso: f64,
    },
    Texto(String),
    /// Uno de unos cuantos (el texto de cada opcion: `plt-opcion-<valor>`).
    Opcion {
        valor: String,
        opciones: Vec<&'static str>,
    },
    Si(bool),
    /// Una lista de textos (se edita como texto con comas).
    Lista(Vec<String>),
}

/// **Una propiedad**: su clave (el campo del JSON; el texto de la interfaz
/// es `plt-prop-<clave>`) y su valor.
#[derive(Debug, Clone, PartialEq)]
pub struct Propiedad {
    pub clave: &'static str,
    pub valor: Valor,
}

/// Las opciones de un campo enumerado.
pub fn opciones(campo: &str) -> Vec<&'static str> {
    match campo {
        "alineacion" => vec!["izquierda", "centro", "derecha"],
        "pauta" => Pauta::TODAS.iter().map(|p| p.clave()).collect(),
        "trazo" => Trazo::TODOS.iter().map(|p| p.clave()).collect(),
        "campo" => QueCampo::TODOS.iter().map(|p| p.clave()).collect(),
        "formato" => vec![
            "corto",
            "largo",
            "completo",
            "iso",
            "numero",
            "abreviado",
            "letra",
        ],
        "tam" => vec!["auto", "mm", "fr"],
        _ => Vec::new(),
    }
}

/// **Las propiedades de un bloque**: su tamaño en el contenedor y despues
/// las de su tipo.
pub fn de(b: &Bloque) -> Vec<Propiedad> {
    let mut v = Vec::new();
    let (modo, n) = match b.tam {
        Tam::Auto => ("auto", 0.0),
        Tam::Fijo(mm) => ("mm", f64::from(mm)),
        Tam::Fraccion(f) => ("fr", f64::from(f)),
    };
    v.push(Propiedad {
        clave: "tam",
        valor: Valor::Opcion {
            valor: modo.into(),
            opciones: opciones("tam"),
        },
    });
    match b.tam {
        Tam::Fijo(_) => v.push(Propiedad {
            clave: "tam_mm",
            valor: Valor::Numero {
                valor: n,
                min: 4.0,
                max: 400.0,
                paso: 5.0,
            },
        }),
        Tam::Fraccion(_) => v.push(Propiedad {
            clave: "tam_fr",
            valor: Valor::Numero {
                valor: n,
                min: 0.25,
                max: 10.0,
                paso: 0.25,
            },
        }),
        Tam::Auto => {}
    }
    let json = serde_json::to_value(b).unwrap_or(Value::Null);
    for (campo, clase) in crate::formato::campos_de(b.tipo.clave()) {
        let x = json.get(*campo);
        let valor = match clase {
            crate::formato::Clase::Texto => {
                Valor::Texto(x.and_then(Value::as_str).unwrap_or("").into())
            }
            crate::formato::Clase::Entero(a, z) => Valor::Numero {
                valor: x.and_then(Value::as_f64).unwrap_or(*a as f64),
                min: *a as f64,
                max: *z as f64,
                paso: if *campo == "paso" { 15.0 } else { 1.0 },
            },
            crate::formato::Clase::Real(a, z) => Valor::Numero {
                valor: x.and_then(Value::as_f64).unwrap_or(*a),
                min: *a,
                max: *z,
                paso: if *z > 40.0 { 1.0 } else { 0.5 },
            },
            crate::formato::Clase::Si => Valor::Si(x.and_then(Value::as_bool).unwrap_or(false)),
            crate::formato::Clase::Lista => Valor::Lista(
                x.and_then(Value::as_array)
                    .map(|l| {
                        l.iter()
                            .filter_map(|s| s.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
            ),
            crate::formato::Clase::Enum => Valor::Opcion {
                valor: x.and_then(Value::as_str).unwrap_or("").into(),
                opciones: opciones(campo),
            },
        };
        v.push(Propiedad {
            clave: campo,
            valor,
        });
    }
    v
}

/// **Cambia una propiedad**, pasando por la reparacion (los topes del
/// fichero). Devuelve si cambio algo.
pub fn poner(b: &mut Bloque, clave: &str, valor: &Valor) -> bool {
    let antes = b.clone();
    match (clave, valor) {
        ("tam", Valor::Opcion { valor, .. }) => {
            b.tam = match valor.as_str() {
                "mm" => Tam::Fijo(match b.tam {
                    Tam::Fijo(x) => x,
                    _ => 30.0,
                }),
                "fr" => Tam::Fraccion(match b.tam {
                    Tam::Fraccion(x) => x,
                    _ => 1.0,
                }),
                _ => Tam::Auto,
            };
        }
        ("tam_mm", Valor::Numero { valor, .. }) => {
            b.tam = Tam::Fijo((*valor as f32).clamp(4.0, 400.0))
        }
        ("tam_fr", Valor::Numero { valor, .. }) => {
            b.tam = Tam::Fraccion((*valor as f32).clamp(0.25, 10.0))
        }
        _ => {
            let mut json = serde_json::to_value(&*b).unwrap_or(Value::Null);
            let Some(m) = json.as_object_mut() else {
                return false;
            };
            let nuevo = match valor {
                Valor::Numero { valor, .. } => Value::from(*valor),
                Valor::Texto(s) => Value::String(s.clone()),
                Valor::Opcion { valor, .. } => Value::String(valor.clone()),
                Valor::Si(s) => Value::Bool(*s),
                Valor::Lista(l) => Value::Array(l.iter().cloned().map(Value::String).collect()),
            };
            m.insert(clave.to_string(), nuevo);
            // Los hijos no pasan por la reparacion: se guardan y se devuelven.
            let hijos = b.tipo.hijos().cloned();
            m.remove("hijos");
            let mut avisos = Vec::new();
            let mut cuenta = 0;
            let Some(mut otro) = crate::formato::bloque(&json, 0, &mut cuenta, &mut avisos) else {
                return false;
            };
            if let (Some(h), Some(dest)) = (hijos, otro.tipo.hijos_mut()) {
                *dest = h;
            }
            otro.id = b.id;
            otro.tam = b.tam;
            otro.acento = b.acento;
            *b = otro;
        }
    }
    *b != antes
}

/// Cambia un contenedor de tipo (fila ↔ columna ↔ rejilla ↔ pila ↔
/// seccion) conservando sus hijos.
pub fn cambiar_contenedor(b: &mut Bloque, a: &str) -> bool {
    let Some(hijos) = b.tipo.hijos().cloned() else {
        return false;
    };
    b.tipo = match a {
        "fila" => Tipo::Fila {
            hijos,
            hueco: HUECO,
        },
        "columna" => Tipo::Columna {
            hijos,
            hueco: HUECO,
        },
        "rejilla" => Tipo::Rejilla {
            hijos,
            columnas: 2,
            hueco: HUECO,
        },
        "pila" => Tipo::Pila { hijos },
        "seccion" => Tipo::Seccion {
            titulo: String::new(),
            hijos,
            recuadro: false,
        },
        _ => return false,
    };
    true
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_pieza_de_la_paleta_hace_su_bloque() {
        let t = Textos::nuevos("es");
        for p in piezas() {
            let b = nuevo(p, &t).unwrap_or_else(|| panic!("{p}"));
            // Ida y vuelta por el JSON sin perder nada.
            let j = serde_json::to_value(&b).unwrap();
            let otro: Bloque = serde_json::from_value(j).unwrap();
            assert_eq!(otro, b, "{p}");
        }
        assert!(nuevo("cohete", &t).is_none());
        match nuevo("habitos", &t).unwrap().tipo {
            Tipo::Habitos { habitos, .. } => assert!(habitos.len() >= 3),
            otro => panic!("{otro:?}"),
        }
        match nuevo("prioridades", &Textos::nuevos("en")).unwrap().tipo {
            Tipo::Prioridades { niveles } => assert_eq!(niveles[0], "High"),
            otro => panic!("{otro:?}"),
        }
    }

    #[test]
    fn las_propiedades_se_leen_y_se_cambian_con_sus_topes() {
        let mut b = Bloque::nuevo(Tipo::Tabla {
            filas: 5,
            columnas: 3,
            cabeceras: vec![],
        });
        let props = de(&b);
        assert!(props.iter().any(|p| p.clave == "filas"
            && p.valor
                == Valor::Numero {
                    valor: 5.0,
                    min: 1.0,
                    max: 80.0,
                    paso: 1.0
                }));
        assert!(poner(
            &mut b,
            "filas",
            &Valor::Numero {
                valor: 9.0,
                min: 0.0,
                max: 0.0,
                paso: 0.0
            }
        ));
        assert!(poner(
            &mut b,
            "columnas",
            &Valor::Numero {
                valor: 500.0,
                min: 0.0,
                max: 0.0,
                paso: 0.0
            }
        ));
        assert!(matches!(
            b.tipo,
            Tipo::Tabla {
                filas: 9,
                columnas: 20,
                ..
            }
        ));
        assert!(poner(
            &mut b,
            "cabeceras",
            &Valor::Lista(vec!["A".into(), "B".into()])
        ));
        assert!(poner(
            &mut b,
            "tam",
            &Valor::Opcion {
                valor: "mm".into(),
                opciones: vec![]
            }
        ));
        assert_eq!(b.tam, Tam::Fijo(30.0));
        assert!(poner(
            &mut b,
            "tam_mm",
            &Valor::Numero {
                valor: 55.0,
                min: 0.0,
                max: 0.0,
                paso: 0.0
            }
        ));
        assert_eq!(b.tam, Tam::Fijo(55.0));
        // Caso negativo: lo mismo no cambia nada.
        assert!(!poner(
            &mut b,
            "filas",
            &Valor::Numero {
                valor: 9.0,
                min: 0.0,
                max: 0.0,
                paso: 0.0
            }
        ));
        // Un contenedor conserva sus hijos al cambiar una propiedad.
        let mut s = hacer::seccion("A", vec![hacer::texto("x")]);
        assert!(poner(&mut s, "titulo", &Valor::Texto("B".into())));
        assert_eq!(s.tipo.hijos().unwrap().len(), 1);
        assert!(cambiar_contenedor(&mut s, "fila"));
        assert!(matches!(s.tipo, Tipo::Fila { .. }));
        assert_eq!(s.tipo.hijos().unwrap().len(), 1);
    }
}
