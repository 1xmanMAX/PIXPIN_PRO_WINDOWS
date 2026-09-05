//! La ventana de ajustes, como logica pura (P6).
//!
//! Aqui esta donde cae cada cosa y que pasa al pulsarla. No sabe dibujar,
//! no sabe leer el fichero de ajustes y no conoce los textos traducidos:
//! recibe las filas ya hechas y devuelve efectos. Asi la parte en la que
//! mas facil es equivocarse —que un clic acierte el control que se ve, que
//! la lista no se pueda desplazar mas alla del final— se comprueba en
//! milisegundos y sin abrir una ventana.
//!
//! Todas las medidas son pixeles LOGICOS. Multiplicarlas por la escala del
//! monitor es cosa de quien dibuja: mezclar las dos unidades aqui es lo que
//! hace que en una pantalla al 150 % los clics caigan a dos tercios de
//! donde se ven.

use pixpin_geom::Punto;

/// Alto de la barra de pestanas.
pub const PESTANAS_ALTO: f32 = 44.0;
/// Alto de cada fila de la lista.
pub const FILA_ALTO: f32 = 42.0;
/// Margen a los lados.
pub const MARGEN: f32 = 16.0;
/// Ancho de la zona de control, a la derecha de cada fila.
pub const CONTROL_ANCHO: f32 = 190.0;
/// Lado de los botones cuadrados («-» y «+»).
pub const BOTON_LADO: f32 = 26.0;
/// Alto de la barra de abajo, con «Cerrar» y «Abrir el fichero».
pub const PIE_ALTO: f32 = 52.0;

/// Lo que se puede tocar en una fila.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    /// Un atajo: el texto que se ensena y si choca con el de otro comando.
    ///
    /// El choque se guarda aqui y no se calcula al pintar porque decidirlo
    /// necesita ver TODAS las filas a la vez, y pintar solo ve una.
    Atajo { texto: String, choca: bool },
    /// Si o no.
    Interruptor(bool),
    /// Una de varias, excluyentes. Se muestran todas seguidas.
    Opcion {
        opciones: Vec<String>,
        elegida: usize,
    },
    /// Un numero con sus topes, que se cambia con «-» y «+».
    Numero {
        valor: u32,
        minimo: u32,
        maximo: u32,
        paso: u32,
    },
}

/// Una linea de la ventana: su etiqueta ya traducida y lo que se toca.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fila {
    pub etiqueta: String,
    pub control: Control,
}

/// Lo que el usuario acaba de tocar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Golpe {
    /// Cambiar de pestana.
    Pestana(usize),
    /// Empezar a capturar el atajo de esta fila.
    CapturarAtajo(usize),
    /// Dar la vuelta a un interruptor.
    Alternar(usize),
    /// Elegir la opcion `cual` de la fila `fila`.
    Elegir {
        fila: usize,
        cual: usize,
    },
    /// Bajar o subir un numero.
    Menos(usize),
    Mas(usize),
    /// Los dos botones del pie.
    Cerrar,
    AbrirFichero,
}

/// Estado de la ventana entre un evento y el siguiente.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Estado {
    pub pestana: usize,
    /// Cuanto se ha bajado la lista, en pixeles logicos.
    pub desplazamiento: i32,
    /// La fila cuyo atajo se esta capturando, si hay alguna.
    ///
    /// Mientras esto tiene valor, las teclas NO son atajos de la ventana:
    /// son lo que se esta grabando. Si no, pulsar Escape para cancelar la
    /// captura cerraria la ventana entera.
    pub capturando: Option<usize>,
}

/// Un rectangulo en coordenadas logicas.
///
/// Propio y no `RectF` de `pixpin-render` porque este crate no depende de
/// la capa de dibujo: es logica pura y tiene que poder probarse sola.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recta {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

impl Recta {
    pub fn contiene(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.ancho && y >= self.y && y < self.y + self.alto
    }
}

/// Donde caen las pestanas, en coordenadas logicas de la ventana.
///
/// Se reparten el ancho a partes iguales. Con anchos distintos por texto,
/// cambiar de idioma moveria las pestanas de sitio, y la memoria muscular
/// de donde esta cada una es lo primero que se aprende de una ventana asi.
pub fn pestanas(ancho: f32, cuantas: usize) -> Vec<Recta> {
    if cuantas == 0 {
        return Vec::new();
    }
    let paso = ancho / cuantas as f32;
    (0..cuantas)
        .map(|i| Recta {
            x: i as f32 * paso,
            y: 0.0,
            ancho: paso,
            alto: PESTANAS_ALTO,
        })
        .collect()
}

/// El alto util de la lista: lo que queda entre las pestanas y el pie.
pub fn alto_de_lista(alto: f32) -> f32 {
    (alto - PESTANAS_ALTO - PIE_ALTO).max(0.0)
}

/// Lo maximo que se puede bajar la lista sin dejar hueco al final.
///
/// Cero cuando todo cabe: sin este tope, la rueda seguiria bajando una
/// lista que ya se ve entera y la pantalla se quedaria en blanco.
pub fn desplazamiento_maximo(cuantas_filas: usize, alto: f32) -> i32 {
    let total = cuantas_filas as f32 * FILA_ALTO;
    let hueco = alto_de_lista(alto);
    ((total - hueco).max(0.0)) as i32
}

/// Encaja un desplazamiento entre cero y su tope.
pub fn limitar_desplazamiento(valor: i32, cuantas_filas: usize, alto: f32) -> i32 {
    valor.clamp(0, desplazamiento_maximo(cuantas_filas, alto))
}

/// Donde cae la fila `indice`, ya descontado el desplazamiento.
///
/// Puede quedar fuera de la ventana: quien dibuja decide si la pinta. Se
/// devuelve igual para que el acierto del raton y el pintado usen la misma
/// cuenta y no puedan discrepar.
pub fn rect_de_fila(indice: usize, desplazamiento: i32, ancho: f32) -> Recta {
    Recta {
        x: 0.0,
        y: PESTANAS_ALTO + indice as f32 * FILA_ALTO - desplazamiento as f32,
        ancho,
        alto: FILA_ALTO,
    }
}

/// La zona de control de una fila: a la derecha, con su margen.
pub fn zona_de_control(fila: Recta) -> Recta {
    Recta {
        x: fila.x + fila.ancho - MARGEN - CONTROL_ANCHO,
        y: fila.y + (FILA_ALTO - BOTON_LADO) / 2.0,
        ancho: CONTROL_ANCHO,
        alto: BOTON_LADO,
    }
}

/// Los dos botones de un numero, «-» y «+», dentro de su zona de control.
///
/// Pegados al borde derecho y en ese orden, con el valor entre ellos: es la
/// disposicion que tiene todo el mundo, y cambiarla por gusto obliga a
/// mirar antes de pulsar.
pub fn botones_de_numero(zona: Recta) -> (Recta, Recta) {
    let mas = Recta {
        x: zona.x + zona.ancho - BOTON_LADO,
        y: zona.y,
        ancho: BOTON_LADO,
        alto: BOTON_LADO,
    };
    let menos = Recta {
        x: mas.x - BOTON_LADO - 6.0,
        ..mas
    };
    (menos, mas)
}

/// Donde cae cada opcion de una eleccion, dentro de su zona de control.
///
/// Se reparten la zona a partes iguales: con anchos por texto, elegir una
/// opcion mas larga moveria las demas y el siguiente clic caeria en otra.
pub fn cajas_de_opcion(zona: Recta, cuantas: usize) -> Vec<Recta> {
    if cuantas == 0 {
        return Vec::new();
    }
    let paso = zona.ancho / cuantas as f32;
    (0..cuantas)
        .map(|i| Recta {
            x: zona.x + i as f32 * paso,
            ancho: paso,
            ..zona
        })
        .collect()
}

/// Los dos botones del pie: «Abrir el fichero» a la izquierda y «Cerrar»
/// pegado a la esquina de abajo a la derecha.
pub fn botones_del_pie(ancho: f32, alto: f32) -> (Recta, Recta) {
    let alto_boton = 30.0;
    let y = alto - PIE_ALTO + (PIE_ALTO - alto_boton) / 2.0;
    let cerrar = Recta {
        x: ancho - MARGEN - 90.0,
        y,
        ancho: 90.0,
        alto: alto_boton,
    };
    let abrir = Recta {
        x: MARGEN,
        y,
        ancho: 200.0,
        alto: alto_boton,
    };
    (abrir, cerrar)
}

/// Que se ha tocado al pulsar en `punto`, en coordenadas logicas.
///
/// El orden importa: primero el pie y las pestanas, que estan SIEMPRE
/// encima de la lista aunque esta se haya desplazado por debajo de ellos.
/// Al reves, una fila desplazada hasta la zona del pie robaria sus clics.
pub fn golpe_en(
    punto: Punto,
    ancho: f32,
    alto: f32,
    cuantas_pestanas: usize,
    filas: &[Fila],
    desplazamiento: i32,
) -> Option<Golpe> {
    let (x, y) = (punto.x as f32, punto.y as f32);

    let (abrir, cerrar) = botones_del_pie(ancho, alto);
    if cerrar.contiene(x, y) {
        return Some(Golpe::Cerrar);
    }
    if abrir.contiene(x, y) {
        return Some(Golpe::AbrirFichero);
    }
    if y < PESTANAS_ALTO {
        return pestanas(ancho, cuantas_pestanas)
            .into_iter()
            .position(|r| r.contiene(x, y))
            .map(Golpe::Pestana);
    }
    if y >= alto - PIE_ALTO {
        return None;
    }

    for (indice, fila) in filas.iter().enumerate() {
        let caja = rect_de_fila(indice, desplazamiento, ancho);
        if !caja.contiene(x, y) {
            continue;
        }
        let zona = zona_de_control(caja);
        return match &fila.control {
            // El atajo se agarra por toda la fila y no solo por su recuadro:
            // es lo unico que se toca ahi, y afinar el raton para nada seria
            // un impuesto.
            Control::Atajo { .. } => Some(Golpe::CapturarAtajo(indice)),
            Control::Interruptor(_) => Some(Golpe::Alternar(indice)),
            Control::Numero { .. } => {
                let (menos, mas) = botones_de_numero(zona);
                if menos.contiene(x, y) {
                    Some(Golpe::Menos(indice))
                } else if mas.contiene(x, y) {
                    Some(Golpe::Mas(indice))
                } else {
                    None
                }
            }
            Control::Opcion { opciones, .. } => cajas_de_opcion(zona, opciones.len())
                .into_iter()
                .position(|r| r.contiene(x, y))
                .map(|cual| Golpe::Elegir { fila: indice, cual }),
        };
    }
    None
}

/// El numero que resulta de pulsar «-» o «+», sin salirse de sus topes.
///
/// Se para en el tope en vez de dar la vuelta: dar la vuelta convierte un
/// pulsado de mas en el valor contrario del que se buscaba, y en un ajuste
/// eso se descubre tarde.
pub fn numero_tras(control: &Control, subir: bool) -> Option<u32> {
    let Control::Numero {
        valor,
        minimo,
        maximo,
        paso,
    } = control
    else {
        return None;
    };
    let nuevo = if subir {
        valor.saturating_add(*paso)
    } else {
        valor.saturating_sub(*paso)
    };
    Some(nuevo.clamp(*minimo, *maximo))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn punto(x: i32, y: i32) -> Punto {
        Punto { x, y }
    }

    fn filas() -> Vec<Fila> {
        vec![
            Fila {
                etiqueta: "Capturar".into(),
                control: Control::Atajo {
                    texto: "Ctrl+Alt+C".into(),
                    choca: false,
                },
            },
            Fila {
                etiqueta: "Arrancar con Windows".into(),
                control: Control::Interruptor(false),
            },
            Fila {
                etiqueta: "Retardo".into(),
                control: Control::Numero {
                    valor: 3,
                    minimo: 0,
                    maximo: 10,
                    paso: 1,
                },
            },
            Fila {
                etiqueta: "Color".into(),
                control: Control::Opcion {
                    opciones: vec!["Hex".into(), "RGB".into(), "HSL".into()],
                    elegida: 0,
                },
            },
        ]
    }

    fn muchas() -> Vec<Fila> {
        (0..40)
            .map(|_| Fila {
                etiqueta: "x".into(),
                control: Control::Interruptor(false),
            })
            .collect()
    }

    const ANCHO: f32 = 600.0;
    const ALTO: f32 = 400.0;

    #[test]
    fn las_pestanas_se_reparten_el_ancho_entero() {
        let p = pestanas(ANCHO, 3);
        assert_eq!(p.len(), 3);
        assert_eq!(p[0].x, 0.0);
        // Sin huecos ni solapes entre una y la siguiente.
        assert_eq!(p[0].x + p[0].ancho, p[1].x);
        assert_eq!(p[1].x + p[1].ancho, p[2].x);
        assert!((p[2].x + p[2].ancho - ANCHO).abs() < 0.01);
    }

    #[test]
    fn sin_pestanas_ni_opciones_no_hay_reparto() {
        // Caso negativo: dividir por cero daria infinito y todo caeria en
        // la primera, que no existe.
        assert!(pestanas(ANCHO, 0).is_empty());
        let zona = Recta {
            x: 0.0,
            y: 0.0,
            ancho: 10.0,
            alto: 10.0,
        };
        assert!(cajas_de_opcion(zona, 0).is_empty());
    }

    #[test]
    fn la_lista_no_se_desplaza_si_cabe_entera() {
        // Sin el tope, la rueda seguiria bajando una lista que ya se ve
        // entera y la pantalla se quedaria en blanco.
        assert_eq!(desplazamiento_maximo(2, ALTO), 0);
        assert_eq!(limitar_desplazamiento(500, 2, ALTO), 0);
        assert_eq!(limitar_desplazamiento(-500, 2, ALTO), 0);
    }

    #[test]
    fn con_muchas_filas_se_baja_justo_hasta_el_final() {
        let cuantas = 30;
        let tope = desplazamiento_maximo(cuantas, ALTO);
        assert!(tope > 0);
        // Bajado del todo, la ULTIMA fila termina justo donde acaba la
        // lista: ni antes (quedaria hueco) ni despues (no se veria).
        let ultima = rect_de_fila(cuantas - 1, tope, ANCHO);
        let fin_de_lista = PESTANAS_ALTO + alto_de_lista(ALTO);
        assert!(
            (ultima.y + ultima.alto - fin_de_lista).abs() < 1.0,
            "acaba en {} y la lista en {fin_de_lista}",
            ultima.y + ultima.alto
        );
    }

    #[test]
    fn una_pestana_se_acierta_por_su_mitad() {
        let g = golpe_en(punto(100, 20), ANCHO, ALTO, 3, &filas(), 0);
        assert_eq!(g, Some(Golpe::Pestana(0)));
        let g = golpe_en(punto(500, 20), ANCHO, ALTO, 3, &filas(), 0);
        assert_eq!(g, Some(Golpe::Pestana(2)));
    }

    #[test]
    fn el_atajo_se_agarra_por_toda_su_fila() {
        // Afinar el raton hasta el recuadro seria un impuesto: en esa fila
        // no hay otra cosa que tocar.
        let f = filas();
        let caja = rect_de_fila(0, 0, ANCHO);
        let y = caja.y as i32 + 10;
        assert_eq!(
            golpe_en(punto(20, y), ANCHO, ALTO, 3, &f, 0),
            Some(Golpe::CapturarAtajo(0))
        );
        assert_eq!(
            golpe_en(punto(500, y), ANCHO, ALTO, 3, &f, 0),
            Some(Golpe::CapturarAtajo(0))
        );
    }

    #[test]
    fn los_botones_de_un_numero_se_aciertan_por_separado() {
        let f = filas();
        let caja = rect_de_fila(2, 0, ANCHO);
        let (menos, mas) = botones_de_numero(zona_de_control(caja));
        let en = |r: Recta| {
            golpe_en(
                punto((r.x + r.ancho / 2.0) as i32, (r.y + r.alto / 2.0) as i32),
                ANCHO,
                ALTO,
                3,
                &f,
                0,
            )
        };
        assert_eq!(en(menos), Some(Golpe::Menos(2)));
        assert_eq!(en(mas), Some(Golpe::Mas(2)));
        // Y no se pisan.
        assert!(menos.x + menos.ancho <= mas.x);
    }

    #[test]
    fn cada_opcion_tiene_su_sitio() {
        let f = filas();
        let caja = rect_de_fila(3, 0, ANCHO);
        let cajas = cajas_de_opcion(zona_de_control(caja), 3);
        for (cual, r) in cajas.iter().enumerate() {
            let g = golpe_en(
                punto((r.x + r.ancho / 2.0) as i32, (r.y + r.alto / 2.0) as i32),
                ANCHO,
                ALTO,
                3,
                &f,
                0,
            );
            assert_eq!(g, Some(Golpe::Elegir { fila: 3, cual }));
        }
    }

    #[test]
    fn el_pie_manda_sobre_una_fila_desplazada_debajo() {
        // Caso negativo del orden: con la lista larga, una fila cae encima
        // del pie. Si se comprobara la lista primero, esa fila robaria los
        // clics del boton de cerrar.
        let (_, cerrar) = botones_del_pie(ANCHO, ALTO);
        let g = golpe_en(
            punto(
                (cerrar.x + cerrar.ancho / 2.0) as i32,
                (cerrar.y + cerrar.alto / 2.0) as i32,
            ),
            ANCHO,
            ALTO,
            3,
            &muchas(),
            0,
        );
        assert_eq!(g, Some(Golpe::Cerrar));
    }

    #[test]
    fn en_la_banda_del_pie_no_se_tocan_filas() {
        // Justo en la banda del pie, pero lejos de sus dos botones.
        let g = golpe_en(
            punto(320, (ALTO - 20.0) as i32),
            ANCHO,
            ALTO,
            3,
            &muchas(),
            0,
        );
        assert_eq!(g, None);
    }

    #[test]
    fn los_numeros_se_paran_en_sus_topes() {
        // Dar la vuelta convertiria un pulsado de mas en el valor contrario
        // del que se buscaba, y en un ajuste eso se descubre tarde.
        let arriba = Control::Numero {
            valor: 10,
            minimo: 0,
            maximo: 10,
            paso: 1,
        };
        assert_eq!(numero_tras(&arriba, true), Some(10));
        let abajo = Control::Numero {
            valor: 0,
            minimo: 0,
            maximo: 10,
            paso: 1,
        };
        assert_eq!(numero_tras(&abajo, false), Some(0));
        // Y un paso que se pasaria de largo se queda en el tope.
        let salto = Control::Numero {
            valor: 9,
            minimo: 0,
            maximo: 10,
            paso: 5,
        };
        assert_eq!(numero_tras(&salto, true), Some(10));
    }

    #[test]
    fn un_control_que_no_es_numero_no_da_numero() {
        // Caso negativo: pedirle un numero a un interruptor no puede
        // devolver un cero que luego se guarde como si fuera un ajuste.
        assert_eq!(numero_tras(&Control::Interruptor(true), true), None);
    }
}
