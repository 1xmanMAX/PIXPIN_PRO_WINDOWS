//! **La galeria**: plantillas hechas para empezar rapido (calendario
//! mensual, diario, planificador semanal, tareas, Cornell, reunion,
//! habitos, planificador del dia y cuaderno de puntos). Cada una sale con
//! los textos en el idioma de la interfaz y, las de fechas, con su
//! repeticion puesta (el diario: un dia por pagina del año que viene).

use crate::fecha::Fecha;
use crate::locale::FormatoFecha;
use crate::modelo::{
    Alineacion, Bloque, Cada, Documento, Pauta, QueCampo, Repeticion, Tam, Tipo, hacer,
};
use crate::tema::Tema;
use crate::textos::Textos;

/// Las de la galeria, en el orden en que salen (su nombre: `plt-gal-<clave>`).
pub const CLAVES: [&str; 9] = [
    "mensual", "diario", "semanal", "dia", "tareas", "habitos", "cornell", "reunion", "puntos",
];

/// **Una de la galeria** por su clave.
pub fn por_clave(clave: &str, t: &Textos, hoy: Fecha) -> Option<Documento> {
    let mut d = Documento::nuevo(&t.t(&format!("plt-gal-{clave}")));
    d.idioma = t.etiqueta().to_string();
    let siguiente = |cada| Repeticion {
        cada,
        desde: Fecha::nueva(hoy.ano + 1, 1, 1).unwrap_or(hoy).to_string(),
        hasta: Fecha::nueva(hoy.ano + 1, 12, 31).unwrap_or(hoy).to_string(),
        sin_fines_de_semana: false,
    };
    let habitos: Vec<String> = t.lista("plt-def-habitos");
    let habitos: Vec<&str> = habitos.iter().map(String::as_str).collect();
    let hijos = match clave {
        "mensual" => {
            d.tema = tema("planificacion");
            d.pagina.horizontal = true;
            d.repeticion = Some(siguiente(Cada::Mes));
            vec![
                Bloque::nuevo(Tipo::CalendarioMes {
                    mes: String::new(),
                    mini: false,
                })
                .con_tam(Tam::Fraccion(3.2)),
                hacer::fila(vec![
                    hacer::notas(&t.t("plt-def-notas")).con_tam(Tam::Fraccion(2.0)),
                    hacer::seccion(&t.t("plt-def-metas"), vec![hacer::casillas(&[], 4)]),
                ])
                .con_tam(Tam::Fraccion(1.0)),
            ]
        }
        "diario" => {
            d.tema = tema("diario");
            d.repeticion = Some(siguiente(Cada::Dia));
            vec![
                cabecera("{dia_semana}", QueCampo::Fecha, FormatoFecha::Largo),
                hacer::fila(vec![
                    hacer::seccion(&t.t("plt-def-gratitud"), vec![hacer::numerada_vacia(3)]),
                    Bloque::nuevo(Tipo::Animo {
                        titulo: t.t("plt-def-animo"),
                    })
                    .con_tam(Tam::Fraccion(0.8)),
                ]),
                hacer::notas(&t.t("plt-def-hoy")),
                hacer::fila(vec![
                    hacer::seccion(&t.t("plt-def-manana"), vec![hacer::casillas(&[], 4)]),
                    Bloque::nuevo(Tipo::CalendarioMes {
                        mes: String::new(),
                        mini: true,
                    }),
                ]),
            ]
        }
        "semanal" => {
            d.tema = tema("planificacion");
            d.repeticion = Some(siguiente(Cada::Semana));
            vec![
                cabecera(
                    &format!("{} {{semana}}", t.t("plt-def-semana")),
                    QueCampo::Mes,
                    FormatoFecha::Largo,
                ),
                Bloque::nuevo(Tipo::CalendarioSemana {
                    fin_de_semana: true,
                })
                .con_tam(Tam::Fraccion(3.0)),
                hacer::fila(vec![
                    hacer::seccion(&t.t("plt-def-tareas"), vec![hacer::casillas(&[], 5)]),
                    hacer::seccion(&t.t("plt-pieza-habitos"), vec![hacer::habitos(&habitos, 7)]),
                ]),
            ]
        }
        "dia" => {
            d.tema = tema("planificacion");
            vec![
                cabecera("{dia_semana}", QueCampo::Fecha, FormatoFecha::Largo),
                hacer::fila(vec![
                    hacer::columna(vec![hacer::horario(6, 22, 1)]).con_tam(Tam::Fraccion(1.25)),
                    hacer::columna(vec![
                        hacer::seccion(
                            &t.t("plt-def-prioridades-titulo"),
                            vec![hacer::numerada_vacia(3)],
                        ),
                        hacer::seccion(&t.t("plt-def-tareas"), vec![hacer::casillas(&[], 9)]),
                        hacer::notas(&t.t("plt-def-notas")),
                    ]),
                ]),
            ]
        }
        "tareas" => {
            d.tema = tema("planificacion");
            vec![
                cabecera(&t.t("plt-gal-tareas"), QueCampo::Fecha, FormatoFecha::Largo),
                hacer::fila(vec![
                    hacer::seccion(&t.t("plt-def-importante"), vec![hacer::casillas(&[], 9)]),
                    hacer::seccion(&t.t("plt-def-cuando"), vec![hacer::casillas(&[], 9)]),
                ]),
                Bloque::nuevo(Tipo::Prioridades {
                    niveles: t.lista("plt-def-prioridades"),
                })
                .con_tam(Tam::Fijo(62.0)),
                hacer::notas(&t.t("plt-def-notas")),
            ]
        }
        "habitos" => {
            d.tema = tema("creativo");
            d.pagina.horizontal = true;
            d.repeticion = Some(siguiente(Cada::Mes));
            let mut lista = t.lista("plt-def-habitos");
            lista.extend(t.lista("plt-def-habitos-mas"));
            vec![
                cabecera(&t.t("plt-gal-habitos"), QueCampo::Mes, FormatoFecha::Largo),
                Bloque::nuevo(Tipo::Habitos {
                    habitos: lista,
                    dias: 0,
                }),
                hacer::fila(vec![
                    Bloque::nuevo(Tipo::Animo {
                        titulo: t.t("plt-def-animo"),
                    }),
                    hacer::notas(&t.t("plt-def-metas")).con_tam(Tam::Fraccion(1.5)),
                ]),
            ]
        }
        "cornell" => {
            d.tema = tema("academico");
            vec![
                cabecera(
                    &t.t("plt-gal-cornell"),
                    QueCampo::Fecha,
                    FormatoFecha::Corto,
                ),
                Bloque::nuevo(Tipo::Cornell {
                    titulo: String::new(),
                    etiquetas: t.lista("plt-def-cornell"),
                }),
            ]
        }
        "reunion" => {
            d.tema = tema("minimo");
            vec![Bloque::nuevo(Tipo::Reunion {
                titulo: t.t("plt-def-reunion"),
                etiquetas: t.lista("plt-def-reunion-rotulos"),
            })]
        }
        "puntos" => {
            d.tema = tema("minimo");
            let mut pie = hacer::campo(QueCampo::Pagina, FormatoFecha::Corto, 8.0);
            if let Tipo::Campo { alineacion, .. } = &mut pie.tipo {
                *alineacion = Alineacion::Centro;
            }
            vec![hacer::lineas(Pauta::Puntos), pie.con_tam(Tam::Fijo(5.0))]
        }
        _ => return None,
    };
    if let Some(h) = d.raiz.tipo.hijos_mut() {
        *h = hijos;
    }
    d.numerar();
    Some(d)
}

fn tema(clave: &str) -> Tema {
    Tema::de_serie(clave).unwrap_or_default()
}

/// Un titulo a la izquierda y un campo a la derecha.
fn cabecera(titulo: &str, campo: QueCampo, formato: FormatoFecha) -> Bloque {
    let mut c = hacer::campo(campo, formato, 11.0);
    if let Tipo::Campo { alineacion, .. } = &mut c.tipo {
        *alineacion = Alineacion::Derecha;
    }
    hacer::fila(vec![
        hacer::encabezado(titulo, 1).con_tam(Tam::Fraccion(1.6)),
        c,
    ])
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::exportar;

    #[test]
    fn todas_las_de_la_galeria_salen_y_se_maquetan_en_todos_los_idiomas() {
        let hoy = Fecha::nueva(2026, 11, 6).unwrap();
        for idioma in crate::textos::IDIOMAS {
            let t = Textos::nuevos(idioma);
            for clave in CLAVES {
                let d = por_clave(clave, &t, hoy).unwrap_or_else(|| panic!("{clave}"));
                assert!(
                    !d.nombre.starts_with("plt-"),
                    "{idioma} {clave}: sin nombre"
                );
                let p = exportar::pagina(&d, hoy, 0);
                assert!(p.prims.len() >= 3, "{idioma} {clave}: casi vacia");
                // Nada se sale de la pagina.
                for c in &p.cajas {
                    assert!(
                        c.caja.x >= -0.01 && c.caja.derecha() <= p.ancho + 0.01,
                        "{idioma} {clave}"
                    );
                    assert!(
                        c.caja.y >= -0.01 && c.caja.abajo() <= p.alto + 0.01,
                        "{idioma} {clave}"
                    );
                }
            }
        }
        assert!(por_clave("cohete", &Textos::nuevos("es"), hoy).is_none());
        // El diario: una pagina por dia del año que viene.
        let d = por_clave("diario", &Textos::nuevos("es"), hoy).unwrap();
        assert_eq!(exportar::contextos(&d, hoy).len(), 365);
    }
}
