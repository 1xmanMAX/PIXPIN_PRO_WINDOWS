//! De elemento a ordenes de dibujo.
//!
//! El motor **no dibuja**: produce la lista de poligonos y polilineas que hay
//! que pintar, con su color y su grosor. Quien las pinta es el consumidor —el
//! pin, la capa de pantalla, el PDF—, que ya tiene su pintor.
//!
//! Esto no es un rodeo, son dos ventajas concretas: el motor entero queda puro
//! y se prueba sin GPU (aqui, en CI, sin escritorio), y las mismas ordenes
//! valen para Direct2D hoy y para exportar a SVG manana sin tocar una linea de
//! geometria.

use crate::azar::Azar;
use crate::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use crate::escena::Escena;
use crate::formas;
use crate::medida::Escala;
use crate::relleno::EstiloRelleno;
use crate::vector::Punto2;

/// Una cosa que pintar. El consumidor traduce cada variante a su API.
#[derive(Debug, Clone, PartialEq)]
pub enum Orden {
    /// Contorno cerrado que se rellena. Es como se pinta la tinta de un
    /// trazo a mano: no es una linea gruesa, es una mancha con forma.
    Poligono {
        puntos: Vec<Punto2>,
        color: ColorRgba,
    },
    /// El contorno cerrado de un trazo a mano (E1). Se rellena con curvas
    /// por puntos medios y regla *winding* (`pixpin_render::tinta`), no
    /// como un poligono de rectas: es lo que hace que se vea como en
    /// Excalidraw.
    Tinta {
        contorno: Vec<Punto2>,
        color: ColorRgba,
    },
    /// Linea abierta de grosor constante.
    Polilinea {
        puntos: Vec<Punto2>,
        color: ColorRgba,
        grosor: f32,
        estilo: EstiloTrazo,
    },
    /// Relleno de una figura cerrada (el interior de un rectangulo).
    Relleno {
        puntos: Vec<Punto2>,
        color: ColorRgba,
    },
    /// Oscurece TODO el lienzo salvo el poligono `hueco` (D51). El motor
    /// no conoce el tamano del lienzo; el consumidor si.
    Velo {
        hueco: Vec<Punto2>,
        color: ColorRgba,
    },
    /// Texto en su caja.
    Texto {
        texto: String,
        x: f32,
        y: f32,
        tam: f32,
        familia: String,
        color: ColorRgba,
        ancho_max: f32,
    },
    /// Un bitmap que el consumidor tiene que resolver por su id.
    Imagen {
        id_objeto: u64,
        x: f32,
        y: f32,
        ancho: f32,
        alto: f32,
        opacidad: f32,
    },
}

/// **El grano de una tinta: la tela que se estampa DENTRO del trazo.**
///
/// No lleva geometria, y eso es lo importante: la silueta a la que hay que
/// recortar la tela es **la que ya salio** en la `Orden::Tinta` (o en el
/// contorno de la figura), asi que repetirla aqui seria calcular dos veces lo
/// mismo y, peor, pagarla dos veces en la cache de teselado. Quien pinta ya
/// tiene la silueta en la mano: esto solo le dice con que tenirla.
///
/// Por eso tampoco es una variante mas de [`Orden`]: una variante nueva
/// obligaria a que todos los que pintan ordenes —la capa de pantalla, el pin,
/// el chat, el editor— la supieran pintar antes de poder compilar, y el grano
/// no tiene sentido en todos. Asi, quien quiera grano lo pide; quien no,
/// sigue igual que estaba.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grano {
    /// De que esta hecha la tinta.
    pub material: crate::tinta::MaterialTinta,
    /// El color de la tela: la **misma** tinta, oscurecida. Un rayado negro
    /// sobre una tinta azul son dos tintas y se lee como una mancha sucia;
    /// oscureciendo la suya, lo que se ve es relieve.
    pub color: ColorRgba,
    /// Cada cuantos pixeles del documento se repite el cuadro.
    pub paso: f32,
    /// Si la tela va inclinada 45 grados (`GRADOS_DEL_GRANO`).
    pub inclinada: bool,
}

/// Lo ancho que sale de verdad la tinta de `e`, que no siempre es su grosor.
///
/// Aqui estaba, en el movil, lo de «la tinta de luz no hace nada»: el lapiz
/// guarda el `strokeWidth` de Excalidraw y se pinta como una mancha de
/// `FACTOR_VARIABLE` veces eso, asi que con el numero guardado el grano salia
/// a la escala equivocada —repartido como si el trazo fuera cuatro veces mas
/// fino de lo que se ve—.
fn ancho_de_la_tinta(e: &Elemento) -> f32 {
    match &e.figura {
        Figura::Lapiz {
            opciones: Some(_), ..
        } => e.grosor * crate::tinta::FACTOR_VARIABLE,
        Figura::Resaltador { .. } => e.grosor * 3.0,
        _ => e.grosor,
    }
    .max(1.0)
}

/// El grano que hay que estampar dentro de `e`, si lleva alguno.
///
/// `None` para la tinta lisa y para las encendidas: lo que se ve de un tubo
/// encendido es el resplandor, y un rayado dentro de un resplandor no llega a
/// la pantalla.
pub fn grano_de(e: &Elemento) -> Option<Grano> {
    if e.borrado || !e.material.hay_grano() {
        return None;
    }
    let oscuro = 1.0 - crate::tinta::material::CUANTO_OSCURECE_EL_GRANO;
    Some(Grano {
        material: e.material,
        color: ColorRgba {
            r: e.trazo.r * oscuro,
            g: e.trazo.g * oscuro,
            b: e.trazo.b * oscuro,
            // El grano va a plena tinta aunque el cuerpo vaya flojo: es lo
            // que se tiene que leer.
            a: e.trazo.a * e.opacidad.clamp(0.0, 1.0),
        },
        paso: crate::tinta::paso_del_grano(ancho_de_la_tinta(e)),
        inclinada: e.material.se_inclina(),
    })
}

/// Aplica la opacidad del elemento a un color.
fn con_opacidad(c: ColorRgba, opacidad: f32) -> ColorRgba {
    ColorRgba {
        a: c.a * opacidad.clamp(0.0, 1.0),
        ..c
    }
}

/// Gira todos los puntos de una orden alrededor de `centro`.
///
/// Se gira **el resultado** y no se le pasa el angulo a cada generador de
/// geometria. Dos razones, y la segunda es la que manda:
///
/// 1. Es un solo sitio en vez de diez.
/// 2. La semilla tiene que seguir mandando sobre el aspecto (D38). Si el
///    angulo entrara en los generadores de rugosidad, girar un rectangulo lo
///    redibujaria con otro garabato y el dibujo temblaria al girarlo.
fn girar_orden(o: &mut Orden, centro: Punto2, angulo: f32) {
    let gira = |p: &mut Punto2| *p = p.girar(centro, angulo);
    match o {
        Orden::Poligono { puntos, .. }
        | Orden::Tinta {
            contorno: puntos, ..
        }
        | Orden::Polilinea { puntos, .. }
        | Orden::Relleno { puntos, .. }
        | Orden::Velo { hueco: puntos, .. } => puntos.iter_mut().for_each(gira),
        Orden::Texto { x, y, .. } | Orden::Imagen { x, y, .. } => {
            // El texto y la imagen giran por su esquina; quien pinta aplica
            // el resto con su propia transformacion.
            let mut p = Punto2::nuevo(*x, *y);
            gira(&mut p);
            *x = p.x;
            *y = p.y;
        }
    }
}

/// Lo que hay que pintar DENTRO de una figura cerrada, antes de su contorno.
///
/// Con estilo solido es una mancha; con rayado o cruzado, una polilinea por
/// raya —del color del relleno y con la pluma mas fina del relleno, no con la
/// del contorno.
///
/// Usa un generador propio sembrado con la misma semilla en vez del del
/// elemento: si gastara numeros del otro, el garabato del contorno cambiaria
/// segun como este relleno el interior, y rellenar una figura ya dibujada la
/// redibujaria distinta (D38).
fn ordenes_de_relleno(e: &Elemento, elipse: bool) -> Vec<Orden> {
    let Some(r) = e.relleno.filter(|c| c.a > 0.0) else {
        return Vec::new();
    };
    let color = con_opacidad(r, e.opacidad);
    let mut azar = Azar::nuevo(e.semilla);
    let rombo = matches!(e.figura, Figura::Rombo);

    if e.estilo_relleno == EstiloRelleno::Solido {
        // La figura LISA, no la rugosa: rellenar la temblorosa deja huecos
        // por donde se escapa el fondo.
        let puntos = if elipse {
            formas::elipse(e.x, e.y, e.ancho, e.alto, 0.0, &mut azar)
                .into_iter()
                .next()
                .unwrap_or_default()
        } else if rombo {
            formas::vertices_de_rombo(e.x, e.y, e.ancho, e.alto).to_vec()
        } else {
            vec![
                Punto2::nuevo(e.x, e.y),
                Punto2::nuevo(e.x + e.ancho, e.y),
                Punto2::nuevo(e.x + e.ancho, e.y + e.alto),
                Punto2::nuevo(e.x, e.y + e.alto),
            ]
        };
        if puntos.is_empty() {
            return Vec::new();
        }
        return vec![Orden::Relleno { puntos, color }];
    }

    crate::relleno::lineas_de_rayado(
        (e.x, e.y, e.ancho, e.alto),
        elipse,
        e.estilo_relleno,
        e.grosor,
        e.rugosidad,
        &mut azar,
    )
    .into_iter()
    // El rayado se genera para la caja; el rombo ocupa la mitad de ella, asi
    // que cada raya se recorta a sus cuatro lados. Sin esto el sombreado se
    // saldria por las cuatro esquinas y el rombo se leeria como un cuadrado.
    .filter_map(|(a, b)| {
        if rombo {
            crate::relleno::recortar_a_convexo(
                a,
                b,
                &formas::vertices_de_rombo(e.x, e.y, e.ancho, e.alto),
            )
        } else {
            Some((a, b))
        }
    })
    .map(|(a, b)| Orden::Polilinea {
        puntos: vec![a, b],
        color,
        grosor: crate::relleno::grosor_de_rayado(e.grosor),
        // Siempre solidas: unas rayas de relleno discontinuas se leerian
        // como suciedad y no como relleno.
        estilo: EstiloTrazo::Solido,
    })
    .collect()
}

/// **El contorno de una region con sus agujeros recortados, en UN solo
/// poligono.**
///
/// El primer intento fue mandar el contorno y cada agujero como ordenes de
/// relleno seguidas, el segundo con alfa cero, contando con que quien pinta
/// los restara por la regla par/impar. **Se probo y no**: `Pintor::poligono`
/// pinta una figura por llamada, asi que el agujero salia pintado igual y la
/// prueba de muestra lo enseno —un anillo relleno de lado a lado—.
///
/// Lo que si funciona es el ojo de cerradura de toda la vida: se recorre el
/// contorno, se sale por un puente hasta el agujero mas cercano, se le da la
/// vuelta entera al agujero y se vuelve por el mismo puente. Queda un solo
/// contorno cerrado, y las dos reglas de relleno —la par/impar de Direct2D y
/// la del giro— dejan el agujero vacio: el puente se recorre dos veces y se
/// anula solo.
///
/// El puente se busca entre el par de vertices MAS CERCANO, que es el que
/// menos se nota y el que menos riesgo corre de cruzar otro trozo del
/// contorno.
fn anillo_con_huecos(contorno: &[Punto2], huecos: &[Vec<Punto2>]) -> Vec<Punto2> {
    let mut anillo = contorno.to_vec();
    for hueco in huecos.iter().filter(|h| h.len() >= 3) {
        let (mut mi, mut mj, mut mejor) = (0usize, 0usize, f32::MAX);
        for (i, a) in anillo.iter().enumerate() {
            for (j, b) in hueco.iter().enumerate() {
                let d = a.distancia(*b);
                if d < mejor {
                    (mejor, mi, mj) = (d, i, j);
                }
            }
        }
        let n = hueco.len();
        // El agujero al reves: con la par/impar da igual el sentido, pero al
        // reves vale tambien para la regla del giro, y asi este poligono
        // sirve sea cual sea la que use quien lo pinte.
        let mut puente: Vec<Punto2> = (0..=n).map(|k| hueco[(mj + n - (k % n)) % n]).collect();
        // Y de vuelta al contorno por donde se salio.
        puente.push(anillo[mi]);
        anillo.splice(mi + 1..mi + 1, puente);
    }
    anillo
}

/// El arco de `e` muestreado en tramos rectos, en coordenadas del documento.
///
/// Con `barrido` puesto sale el tramo repasado; sin el, **el ovalo entero**,
/// que es la guia que todavia no se ha repasado y que tiene que verse para
/// poder repasarla.
///
/// Es una sola linea porque la cuenta esta en `arco.rs`, que es el puerto del
/// `Arco.kt` del movil. Aqui habia un segundo muestreo con sus propios 64
/// tramos: dos verdades sobre por donde pasa un arco significan que se pica
/// donde no se ve, porque `impacto.rs` usa esta y el reajuste usa la otra.
pub(crate) fn arco_muestreado(e: &Elemento, inicio: f32, barrido: Option<f32>) -> Vec<Punto2> {
    crate::arco::puntos_del_arco(
        (e.x, e.y, e.ancho, e.alto),
        inicio,
        barrido,
        crate::arco::PASOS_DEL_ARCO,
    )
}

/// Las ordenes de dibujo de un elemento, en orden de pintado.
/// El rotulo del marco: su tamano y cuanto sube por encima de la caja. Va
/// fuera de ella a proposito, como en Excalidraw: dentro taparia contenido.
const TAM_NOMBRE_MARCO: f32 = 12.0;
const ALTO_NOMBRE_MARCO: f32 = 16.0;

pub fn ordenes(e: &Elemento) -> Vec<Orden> {
    if e.borrado {
        return Vec::new();
    }
    // Un generador propio, sembrado con la semilla del elemento: asi su
    // aspecto no depende de cuantos elementos se dibujaron antes (D38).
    let mut azar = Azar::nuevo(e.semilla);
    // **Las porosas pintan el cuerpo mas flojo.** En la tiza, el lapiz
    // blando y el rotulador seco lo que se tiene que leer es el grano; con el
    // cuerpo a plena tinta el grano se pierde encima de un trazo macizo y las
    // tres se ven iguales que la lisa. Ver `tinta::material`.
    let color = con_opacidad(e.trazo, e.opacidad * e.material.cuerpo());
    let mut salida = Vec::new();

    match &e.figura {
        Figura::Lapiz {
            puntos,
            presiones,
            opciones,
        } => {
            // **La presion firme.** Un trazo con `presionFirme` va de ancho
            // constante: es lo que hace usable escribir a mano, porque sin
            // ella la tinta adelgaza en las curvas y la letra se rompe. Se
            // pide por `contorno_de_lapiz_firme` y no aplicando el campo aqui
            // para que no pueda quedarse puesta en un sitio y no en el otro,
            // que es como se veia el fallo en el movil: firme mientras se
            // escribe y adelgazando al soltar.
            let contorno = crate::tinta::contorno_de_lapiz_firme(
                puntos,
                presiones,
                e.grosor,
                *opciones,
                e.extras.presion_firme,
            );
            if !contorno.is_empty() {
                salida.push(Orden::Tinta { contorno, color });
            }
        }

        Figura::Resaltador { puntos } => {
            // D45: grueso, translucido y SIN adelgazar. Un resaltador de
            // grosor variable deja el texto medio tapado.
            let contorno = crate::tinta::contorno_de_resaltador(puntos, e.grosor);
            if !contorno.is_empty() {
                salida.push(Orden::Tinta {
                    contorno,
                    color: ColorRgba {
                        a: 0.35 * e.opacidad,
                        ..e.trazo
                    },
                });
            }
        }

        Figura::Linea { puntos } => {
            for par in puntos.windows(2) {
                for pasada in formas::linea(par[0], par[1], e.rugosidad, &mut azar) {
                    salida.push(Orden::Polilinea {
                        puntos: pasada,
                        color,
                        grosor: e.grosor,
                        estilo: e.estilo,
                    });
                }
            }
        }

        Figura::Flecha {
            puntos,
            punta_inicio,
            punta_fin,
            codos,
        } => {
            // El camino de verdad. Con codos no es la lista de puntos: es la
            // escalera ortogonal que sale de ellos, y las puntas tienen que
            // mirar ESA -si miraran la lista, la punta saldria apuntando a la
            // diagonal que el conector no llega a dibujar-.
            //
            // Sin codos se trabaja sobre la lista tal cual, sin copiarla: una
            // flecha normal es el caso de siempre y no tiene por que pagar la
            // reserva de la de codos.
            let escalera;
            let trazado: &[Punto2] = if *codos {
                escalera = crate::codo::trazado_de_flecha(puntos, true);
                &escalera
            } else {
                puntos
            };
            for par in trazado.windows(2) {
                for pasada in formas::linea(par[0], par[1], e.rugosidad, &mut azar) {
                    salida.push(Orden::Polilinea {
                        puntos: pasada,
                        color,
                        grosor: e.grosor,
                        estilo: e.estilo,
                    });
                }
            }
            // Las dos puntas, cada una del tipo que le toque. El principio
            // primero y el final despues, que es el orden en el que estan en
            // el fichero.
            for (al_final, tipo) in [(false, *punta_inicio), (true, *punta_fin)] {
                let Some(forma) = formas::forma_de_punta(trazado, al_final, tipo, e.grosor) else {
                    continue;
                };
                let contorno = formas::contorno_de_punta(&forma);
                if contorno.len() < 2 {
                    continue;
                }
                // Las macizas van rellenas Y trazadas: solo rellenas, una
                // punta pequena se queda mas fina que la raya que remata y
                // parece que la flecha no llega.
                if tipo.es_maciza() {
                    salida.push(Orden::Relleno {
                        puntos: contorno.clone(),
                        color,
                    });
                }
                salida.push(Orden::Polilinea {
                    puntos: contorno,
                    color,
                    grosor: e.grosor,
                    // La punta siempre solida: una punta punteada no se lee
                    // como punta.
                    estilo: EstiloTrazo::Solido,
                });
            }
        }

        // **El mosaico: una banda opaca, y a propósito.**
        //
        // El movil tapa remuestreando los pixeles de debajo —bloques o
        // mancha, segun `desenfoque`—, y para eso hacen falta los pixeles,
        // que aqui no estan: el motor es puro y solo produce ordenes. Asi que
        // por ahora se tapa con una mancha maciza.
        //
        // Se parece menos, pero **tapa lo mismo**, que es lo unico que un
        // mosaico promete. Lo que habia antes era no pintar nada, y eso
        // significaba ensenar en el escritorio el numero de cuenta que el
        // usuario habia tapado en el telefono. Entre parecerse y tapar, tapa.
        Figura::Mosaico { .. } => {
            let tapa = e
                .relleno
                .filter(|c| c.a > 0.0)
                .unwrap_or(crate::mosaico::TAPA_MACIZA);
            // **Girada con el elemento.** El mosaico lleva tirador de giro
            // como cualquier otro y el marco de seleccion si giraba, asi que
            // la banda sin girar dejaba asomar las cuatro puntas de lo que el
            // usuario habia tapado. Se giran los cuatro puntos, no la caja
            // envolvente: el relleno es un poligono y tapa exactamente lo que
            // el marco promete.
            let centro = Punto2::nuevo(e.x + e.ancho / 2.0, e.y + e.alto / 2.0);
            let girar = |p: Punto2| {
                if e.angulo == 0.0 {
                    p
                } else {
                    p.girar(centro, e.angulo)
                }
            };
            salida.push(Orden::Relleno {
                puntos: vec![
                    girar(Punto2::nuevo(e.x, e.y)),
                    girar(Punto2::nuevo(e.x + e.ancho, e.y)),
                    girar(Punto2::nuevo(e.x + e.ancho, e.y + e.alto)),
                    girar(Punto2::nuevo(e.x, e.y + e.alto)),
                ],
                // **Sin la opacidad del elemento**: un mosaico a medio tapar
                // no tapa. Es la unica figura del lienzo a la que la
                // opacidad no se le aplica, y es por lo que existe.
                color: ColorRgba { a: 1.0, ..tapa },
            });
        }

        Figura::Rombo => {
            // El relleno va PRIMERO, como en el rectangulo: si fuera despues
            // taparia el trazo por dentro.
            salida.extend(ordenes_de_relleno(e, false));
            // **El rombo redondeado.** El estilo de fabrica del movil trae
            // `roundness`, asi que un rombo pide puntas redondeadas desde el
            // primer dia; dibujado en pico la diferencia canta, porque sus
            // cuatro vertices son angulos agudos. Va liso y no a mano alzada
            // porque el redondeo ya es la forma: temblarlo encima convierte
            // el cuarto de vuelta en un garabato.
            if e.redondo {
                salida.push(Orden::Polilinea {
                    puntos: formas::rombo_redondo(e.x, e.y, e.ancho, e.alto),
                    color,
                    grosor: e.grosor,
                    estilo: e.estilo,
                });
            } else {
                for pasada in formas::rombo(e.x, e.y, e.ancho, e.alto, e.rugosidad, &mut azar) {
                    salida.push(Orden::Polilinea {
                        puntos: pasada,
                        color,
                        grosor: e.grosor,
                        estilo: e.estilo,
                    });
                }
            }
        }

        Figura::Rectangulo => {
            // El relleno va PRIMERO: si fuera despues taparia el trazo.
            salida.extend(ordenes_de_relleno(e, false));
            // Redondeado y sin temblor es el recuadro de una «zona» del movil:
            // una sola pasada y las esquinas curvas, o no coincide con el
            // suyo y se lee como otro recuadro encima.
            if e.redondo && e.rugosidad <= 0.0 {
                salida.push(Orden::Polilinea {
                    puntos: formas::rectangulo_redondo(e.x, e.y, e.ancho, e.alto),
                    color,
                    grosor: e.grosor,
                    estilo: e.estilo,
                });
                return salida;
            }
            for pasada in formas::rectangulo(e.x, e.y, e.ancho, e.alto, e.rugosidad, &mut azar) {
                salida.push(Orden::Polilinea {
                    puntos: pasada,
                    color,
                    grosor: e.grosor,
                    estilo: e.estilo,
                });
            }
        }

        Figura::Elipse => {
            salida.extend(ordenes_de_relleno(e, true));
            for pasada in formas::elipse(e.x, e.y, e.ancho, e.alto, e.rugosidad, &mut azar) {
                salida.push(Orden::Polilinea {
                    puntos: pasada,
                    color,
                    grosor: e.grosor,
                    estilo: e.estilo,
                });
            }
        }

        Figura::Foco { elipse } => {
            // Hueco LISO siempre: un velo tembloroso deja rendijas por las
            // que se cuela el fondo oscurecido.
            let mut lisa = Azar::nuevo(e.semilla);
            let hueco = if *elipse {
                formas::elipse(e.x, e.y, e.ancho, e.alto, 0.0, &mut lisa)
                    .into_iter()
                    .next()
                    .unwrap_or_default()
            } else {
                vec![
                    Punto2::nuevo(e.x, e.y),
                    Punto2::nuevo(e.x + e.ancho, e.y),
                    Punto2::nuevo(e.x + e.ancho, e.y + e.alto),
                    Punto2::nuevo(e.x, e.y + e.alto),
                ]
            };
            // **El movil oscurece 45, no 60.** Aqui estaba puesto a mano y un
            // foco del telefono abierto en el escritorio apagaba de mas: lo
            // de alrededor, que un foco existe para dejar ver en contexto, se
            // perdia. El numero es el de `lupa_elemento`, que es donde vive lo
            // que se sabe de un foco, y no una segunda constante aqui.
            let oscuridad = e.relleno.unwrap_or(ColorRgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: crate::lupa_elemento::OSCURECER_POR_DEFECTO as f32 / 100.0,
            });
            salida.push(Orden::Velo {
                hueco: hueco.clone(),
                color: con_opacidad(oscuridad, e.opacidad),
            });
            // El borde del hueco, cerrado: ayuda a ver donde acaba el foco
            // sobre fondos ya oscuros.
            let mut borde = hueco;
            if let Some(primero) = borde.first().copied() {
                borde.push(primero);
            }
            salida.push(Orden::Polilinea {
                puntos: borde,
                color,
                grosor: e.grosor,
                estilo: EstiloTrazo::Solido,
            });
        }

        Figura::Texto {
            texto,
            tam,
            familia,
        } => {
            salida.push(Orden::Texto {
                texto: texto.clone(),
                x: e.x,
                y: e.y,
                tam: *tam,
                familia: familia.clone(),
                color,
                ancho_max: e.ancho.max(1.0),
            });
            // **El tachado, renglon a renglon.** No es un adorno de la fuente
            // sino una raya del dibujo: `Orden::Texto` no lleva tachado y el
            // motor es puro, asi que la altura la da `texto::raya_del_tachado`
            // -la misma cuenta para el editor, la exportacion y la miniatura,
            // porque tres copias acabarian poniendo la raya a tres alturas-.
            let estilo_texto = crate::texto::EstiloDeTexto {
                negrita: e.extras.negrita,
                cursiva: e.extras.cursiva,
                tachado: e.extras.tachado,
            };
            if estilo_texto.tachado {
                for (fila, renglon) in texto.split('\n').enumerate() {
                    let y = e.y + fila as f32 * *tam * crate::texto::INTERLINEADO;
                    let Some((a, b, gordo)) =
                        crate::texto::raya_del_tachado(renglon, e.x, y, *tam, estilo_texto)
                    else {
                        continue;
                    };
                    salida.push(Orden::Polilinea {
                        puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
                        color,
                        grosor: gordo,
                        estilo: EstiloTrazo::Solido,
                    });
                }
            }
        }

        // Un emoji es texto con la fuente de color de Windows. El 0,8 deja
        // sitio al glifo, que asoma por encima y por debajo de su cuerpo.
        Figura::Emoji { caracter } => salida.push(Orden::Texto {
            texto: caracter.clone(),
            x: e.x,
            y: e.y,
            tam: e.alto * 0.8,
            familia: "Segoe UI Emoji".to_string(),
            color,
            ancho_max: e.ancho.max(1.0),
        }),

        Figura::Marco { nombre } => {
            // Liso y gris, no a mano alzada: el marco es andamiaje para
            // ordenar laminas, no parte del dibujo. Si temblara como una
            // figura, se leeria como una mas.
            let gris = crate::ColorRgba::opaco(0.53, 0.55, 0.60);
            let (x0, y0) = (e.x, e.y);
            let (x1, y1) = (e.x + e.ancho, e.y + e.alto);
            salida.push(Orden::Polilinea {
                puntos: vec![
                    Punto2::nuevo(x0, y0),
                    Punto2::nuevo(x1, y0),
                    Punto2::nuevo(x1, y1),
                    Punto2::nuevo(x0, y1),
                    Punto2::nuevo(x0, y0),
                ],
                color: gris,
                grosor: 1.5,
                estilo: EstiloTrazo::Solido,
            });
            // El nombre va ENCIMA del marco, fuera de su caja: dentro se
            // confundiria con el contenido y ademas lo taparia.
            if !nombre.is_empty() {
                salida.push(Orden::Texto {
                    texto: nombre.clone(),
                    x: x0,
                    y: y0 - ALTO_NOMBRE_MARCO,
                    tam: TAM_NOMBRE_MARCO,
                    familia: "Segoe UI".to_string(),
                    color: gris,
                    ancho_max: e.ancho.max(1.0),
                });
            }
        }

        Figura::Imagen { id_objeto } => salida.push(Orden::Imagen {
            id_objeto: *id_objeto,
            x: e.x,
            y: e.y,
            ancho: e.ancho,
            alto: e.alto,
            opacidad: e.opacidad,
        }),

        // La raya y sus marcas en los extremos. **Sin texto**: el rotulo
        // depende de la escala y va en `ordenes_medibles`, fuera de la
        // cache, porque la escala puede cambiar sin que cambie el elemento.
        Figura::Cota { puntos } => {
            if puntos.len() >= 2 {
                let (a, b) = (puntos[0], puntos[puntos.len() - 1]);
                salida.push(Orden::Polilinea {
                    puntos: vec![a, b],
                    color: con_opacidad(e.trazo, e.opacidad),
                    grosor: e.grosor,
                    estilo: e.estilo,
                });
                // Las marcas de los extremos, perpendiculares a la raya.
                let perp = b
                    .restar(a)
                    .unitario()
                    .perpendicular()
                    .escalar(e.grosor * 3.0);
                for extremo in [a, b] {
                    salida.push(Orden::Polilinea {
                        puntos: vec![extremo.sumar(perp), extremo.restar(perp)],
                        color: con_opacidad(e.trazo, e.opacidad),
                        grosor: e.grosor,
                        estilo: EstiloTrazo::Solido,
                    });
                }
            }
        }
        // **El arco: el tramo del ovalo, y la guia entera si aun no se ha
        // repasado.** Liso y no tembloroso a proposito: el arco se usa como
        // plantilla y un transportador que tiembla no sirve de plantilla. El
        // detalle —el repasado a mano, el reajuste al estirar— es del grupo
        // que lo herede; esto es lo que hace falta para que se VEA.
        Figura::Arco { inicio, barrido } => {
            let puntos = arco_muestreado(e, *inicio, *barrido);
            if puntos.len() >= 2 {
                salida.push(Orden::Polilinea {
                    puntos,
                    color,
                    grosor: e.grosor,
                    // La guia sin repasar se ensena punteada: es un andamio,
                    // no una raya del dibujo, y confundirlas seria dejar
                    // ovalos enteros donde solo hay un arco a medias.
                    estilo: if barrido.is_some() {
                        e.estilo
                    } else {
                        EstiloTrazo::Punteado
                    },
                });
            }
        }

        // El circulito con su numero. El radio sale del alto de la caja, no
        // de un `fontSize` aparte: aqui la caja ES el circulo, y asi estirar
        // el elemento agranda el numero con el, que es lo que hace el movil.
        Figura::Serie { numero } => {
            salida.extend(ordenes_de_relleno(e, true));
            for pasada in formas::elipse(e.x, e.y, e.ancho, e.alto, e.rugosidad, &mut azar) {
                salida.push(Orden::Polilinea {
                    puntos: pasada,
                    color,
                    grosor: e.grosor,
                    estilo: e.estilo,
                });
            }
            let texto = numero.to_string();
            let tam = (e.alto.min(e.ancho) * 0.6).max(1.0);
            // **Centrado a ojo, y a proposito.** `Orden::Texto` no sabe
            // centrar —solo ajusta al ancho— y el motor es puro: no puede
            // medir una fuente sin DirectWrite. Se estima el ancho a
            // `ANCHO_DE_CIFRA` por cifra, que en Segoe UI es lo que mide un
            // digito, y se aparta lo que sobra. Un numero de serie sin esto
            // sale pegado al borde izquierdo del circulo y se lee como otra
            // cosa; con esto cae en el medio con el error de un pelo.
            const ANCHO_DE_CIFRA: f32 = 0.6;
            let ancho_texto = tam * ANCHO_DE_CIFRA * texto.chars().count() as f32;
            salida.push(Orden::Texto {
                texto,
                x: e.x + (e.ancho - ancho_texto).max(0.0) / 2.0,
                y: e.y + (e.alto - tam) / 2.0,
                tam,
                familia: "Segoe UI".to_string(),
                color,
                ancho_max: ancho_texto.max(1.0),
            });
        }

        // Lo que encontro el bote: el contorno macizo con sus agujeros
        // RECORTADOS DE VERDAD. Ver `anillo_con_huecos`.
        Figura::Region { contorno, huecos } => {
            if contorno.len() >= 3 {
                let relleno = e.relleno.filter(|c| c.a > 0.0);
                if let Some(r) = relleno {
                    salida.push(Orden::Relleno {
                        puntos: anillo_con_huecos(contorno, huecos),
                        color: con_opacidad(r, e.opacidad),
                    });
                }
                // El borde, siempre, y **tambien el de cada agujero**: sin
                // el, una region sin relleno es invisible y no habria como
                // seleccionarla, y un agujero sin borde no se distingue de
                // un trozo que nadie relleno.
                for anillo in std::iter::once(contorno).chain(huecos.iter()) {
                    if anillo.len() < 3 {
                        continue;
                    }
                    let mut borde = anillo.clone();
                    if let Some(p) = borde.first().copied() {
                        borde.push(p);
                    }
                    salida.push(Orden::Polilinea {
                        puntos: borde,
                        color,
                        grosor: e.grosor,
                        estilo: e.estilo,
                    });
                }
            }
        }

        // El punto y su letra. La letra ORBITA en polares alrededor del
        // punto: es lo que hace que al mover el punto la letra lo siga sin
        // pisar el dibujo.
        Figura::Punto {
            letra,
            angulo,
            radio,
        } => {
            let r = (e.grosor * 1.6).max(2.0);
            let mut circulo = Azar::nuevo(e.semilla);
            let disco = formas::elipse(e.x - r, e.y - r, r * 2.0, r * 2.0, 0.0, &mut circulo)
                .into_iter()
                .next()
                .unwrap_or_default();
            if disco.len() >= 3 {
                salida.push(Orden::Relleno {
                    puntos: disco,
                    color,
                });
            }
            if !letra.is_empty() {
                let tam = (e.grosor * 5.0).max(10.0);
                salida.push(Orden::Texto {
                    texto: letra.clone(),
                    x: e.x + radio * angulo.cos(),
                    y: e.y + radio * angulo.sin() - tam / 2.0,
                    tam,
                    familia: "Segoe UI".to_string(),
                    color,
                    ancho_max: tam * letra.chars().count() as f32,
                });
            }
        }

        // La barra entera depende de la escala: va en `ordenes_medibles`.
        Figura::EscalaGrafica => {}
    }

    // El angulo se aplica aqui, una vez, sobre la geometria ya generada.
    if e.angulo != 0.0 {
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        for o in salida.iter_mut() {
            girar_orden(o, centro, e.angulo);
        }
    }

    salida
}

/// Las ordenes de la escena entera, de abajo arriba.
///
/// Encadena `ordenes_medibles`: sin esto, una cota sale sin su rotulo y una
/// barra de escala no sale en absoluto (D37, «sale en la exportacion»). Esta
/// funcion la usan la capa de pantalla y la recarga de un pin, no solo el
/// editor -y era justo ahi donde el rotulo desaparecia, invisible porque
/// nada llamaba a esta ruta a comprobarlo en pantalla.
pub fn ordenes_de_escena(escena: &Escena) -> Vec<Orden> {
    escena
        .visibles()
        .flat_map(|e| {
            ordenes(e)
                .into_iter()
                .chain(ordenes_medibles(e, escena.escala.as_ref(), ','))
        })
        .collect()
}

/// Holgura que se acepta perder al simplificar, en pixeles de pantalla.
///
/// Medio pixel es la mitad de lo que el ojo puede distinguir, asi que lo que
/// se quita por debajo de eso no se puede notar por definicion.
pub const HOLGURA_DETALLE: f32 = 0.5;

/// Por debajo de este tamano en pantalla un trazo de tinta se pinta como
/// una raya.
///
/// Era 48 px y era la economia del lienzo infinito, pero borraba la tinta
/// de todo lo pequeno: letras, tics y firmas salian como rayas de grosor
/// fijo y puntas cortadas (diagnostico de E1). Excalidraw no simplifica
/// nunca. Aqui se queda en 2 px (D115), donde de verdad no se distingue, y
/// la economia de lejos pasa a la rejilla, la capa congelada y la cache de
/// realizaciones de Direct2D.
pub const TINTA_MINIMA_PX: f32 = 2.0;

/// Las ordenes de un elemento visto a este aumento.
///
/// Hace dos cosas, y solo a las figuras que tienen puntos:
///
/// - Un trazo de tinta —lapiz o resaltador— que en pantalla queda por debajo
///   de `TINTA_MINIMA_PX` pasa a dibujarse como linea, adelgazada.
/// - Una linea o una flecha se adelgazan siempre, porque su salida es punto
///   por punto y ahi quitar puntos si quita trabajo.
///
/// Un `zoom` de cero o menos devuelve el dibujo entero: es la forma de pedir
/// «sin simplificar», que es lo que quiere quien exporta a un fichero.
///
/// Se trabaja sobre una **copia**: los puntos guardados del elemento no se
/// tocan nunca. Ver `aligerar` para por que eso no es negociable.
pub fn ordenes_a_distancia(e: &Elemento, zoom: f32) -> Vec<Orden> {
    if zoom <= 0.0 || e.borrado {
        return ordenes(e);
    }
    let tolerancia = HOLGURA_DETALLE / zoom;
    let flacos = |puntos: &[Punto2]| crate::aligerar::aligerar(puntos, tolerancia);

    // Cuanto ocupa en pantalla, por su lado mayor.
    let (x0, y0, x1, y1) = e.caja();
    let en_pantalla = (x1 - x0).max(y1 - y0) * zoom;

    match &e.figura {
        Figura::Lapiz { puntos, .. } | Figura::Resaltador { puntos }
            if en_pantalla < TINTA_MINIMA_PX =>
        {
            let mut puntos = flacos(puntos);
            if puntos.len() < 2 {
                return Vec::new();
            }
            // Los puntos crudos estan en marco local, igual que en `ordenes`:
            // esta rama no delega en ella, asi que gira aqui su propio
            // resultado.
            if e.angulo != 0.0 {
                let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                for p in puntos.iter_mut() {
                    *p = p.girar(centro, e.angulo);
                }
            }
            vec![Orden::Polilinea {
                puntos,
                color: con_opacidad(e.trazo, e.opacidad),
                grosor: e.grosor.max(1.0),
                estilo: EstiloTrazo::Solido,
            }]
        }
        Figura::Linea { puntos } => ordenes(&Elemento {
            figura: Figura::Linea {
                puntos: flacos(puntos),
            },
            ..e.clone()
        }),
        Figura::Flecha {
            puntos,
            punta_inicio,
            punta_fin,
            codos,
        } => ordenes(&Elemento {
            figura: Figura::Flecha {
                puntos: flacos(puntos),
                punta_inicio: *punta_inicio,
                punta_fin: *punta_fin,
                codos: *codos,
            },
            ..e.clone()
        }),
        _ => ordenes(e),
    }
}

/// Las ordenes de lo que se ve, y solo de lo que se ve.
///
/// Es la via que usa un lienzo infinito, y hace las dos economias que lo
/// sostienen: no mira siquiera los elementos que caen fuera de la pantalla, y
/// de los que entran no dibuja el detalle que a ese aumento no se distingue.
pub fn ordenes_de_escena_vista(
    escena: &Escena,
    camara: &crate::camara::Camara,
    ancho_px: f32,
    alto_px: f32,
) -> Vec<Orden> {
    crate::camara::recortar(escena, camara.ventana(ancho_px, alto_px))
        .flat_map(|e| {
            ordenes_a_distancia(e, camara.zoom)
                .into_iter()
                .chain(ordenes_medibles(e, escena.escala.as_ref(), ','))
        })
        .collect()
}

/// Holgura del marco de seleccion alrededor de la caja del elemento, en
/// pixeles logicos: pegado al borde no se distingue del propio trazo.
pub const HOLGURA_SELECCION: f32 = 4.0;

/// El azul del marco de seleccion. Un color que no esta en la paleta de
/// dibujo: asi se lee como interfaz y no como algo dibujado.
pub const COLOR_SELECCION: ColorRgba = ColorRgba {
    r: 0.36,
    g: 0.42,
    b: 0.95,
    a: 1.0,
};

/// El gris del rotulo de una cota sin escala valida (D35).
///
/// El gris es el aviso: es lo que dice que el numero es en pixeles y no es
/// medida de plano, sin tener que leer el sufijo `px`. Un gris medio y no el
/// trazo del elemento -que puede ser cualquier color de la paleta y no
/// avisaria de nada.
pub const COLOR_SIN_ESCALA: ColorRgba = ColorRgba {
    r: 0.55,
    g: 0.55,
    b: 0.55,
    a: 1.0,
};

/// Marco alrededor de lo seleccionado, para que se vea que esta elegido.
/// Devuelve `None` si el elemento no esta o esta borrado.
pub fn marco_de_seleccion(escena: &Escena, id: u64, escala: f32) -> Option<Orden> {
    let e = escena.buscar(id).filter(|e| !e.borrado)?;
    let (x0, y0, x1, y1) = e.caja();
    let h = HOLGURA_SELECCION * escala.max(0.01);
    let (x0, y0, x1, y1) = (x0 - h, y0 - h, x1 + h, y1 + h);
    Some(Orden::Polilinea {
        // Cerrado: el ultimo punto repite el primero.
        puntos: vec![
            Punto2::nuevo(x0, y0),
            Punto2::nuevo(x1, y0),
            Punto2::nuevo(x1, y1),
            Punto2::nuevo(x0, y1),
            Punto2::nuevo(x0, y0),
        ],
        color: COLOR_SELECCION,
        grosor: (1.5 * escala.max(0.01)).max(1.0),
        estilo: EstiloTrazo::Discontinuo,
    })
}

/// Lo que hay que pintar y **depende de la escala**, asi que no se cachea.
///
/// La cache guarda geometria indexada por `(id, version, nivel)`. Calibrar
/// cambia la escala sin tocar ningun elemento, asi que la version no sube y
/// la cache no se enteraria: se seguirian viendo pixeles despues de calibrar.
///
/// Por eso el rotulo de la cota y los cuadros de la barra salen por aqui,
/// fuera de la cache. Es barato: un texto por cota visible y unos rectangulos
/// por barra, contra el garabato con ruido que si es caro y si se cachea.
pub fn ordenes_medibles(e: &Elemento, escala: Option<&Escala>, coma: char) -> Vec<Orden> {
    if e.borrado {
        return Vec::new();
    }
    let mut salida = match &e.figura {
        Figura::Cota { puntos } if puntos.len() >= 2 => {
            let (a, b) = (puntos[0], puntos[puntos.len() - 1]);
            let medio = a.hacia(b, 0.5);
            // Un poco por encima de la raya, del lado que no la tapa.
            let alto = e.grosor.max(1.0) * 6.0;
            let mut arriba = b.restar(a).unitario().perpendicular().escalar(alto);
            // `rotulo_del_reves` (S5 del diseno): una cota que apunta "hacia
            // atras" (de 90 a 270 grados) da la vuelta al perpendicular, o
            // el rotulo saldria en el lado contrario de la raya de como sale
            // en la de siempre -que es leerlo boca abajo cuando el resto del
            // plano se lee del derecho.
            if crate::medida::rotulo_del_reves(crate::medida::angulo_de(e)) {
                arriba = arriba.escalar(-1.0);
            }
            let p = medio.sumar(arriba);
            // Con escala valida, el color del trazo, igual que el resto del
            // elemento. Sin ella, gris (D35): es el aviso de que el numero
            // es en pixeles y no es medida de plano.
            let color_base = match escala.filter(|x| x.valida()) {
                Some(_) => e.trazo,
                None => COLOR_SIN_ESCALA,
            };
            vec![Orden::Texto {
                texto: crate::medida::texto_de_cota(e, escala, coma),
                x: p.x,
                y: p.y,
                tam: alto * 2.0,
                familia: "Segoe UI".to_string(),
                color: con_opacidad(color_base, e.opacidad),
                ancho_max: a.distancia(b),
            }]
        }
        Figura::EscalaGrafica => barra(e, escala, coma),
        _ => Vec::new(),
    };

    // Mismo bloque que cierra `ordenes()`: el tirador de giro se ofrece para
    // cualquier elemento, `Cota` y `EscalaGrafica` incluidas, y esta salida
    // vive fuera de esa funcion (D-cache-escala), asi que no hereda su giro
    // por delegacion. Hay que aplicarlo aqui tambien o el rotulo gira sin la
    // raya y la barra gira sin sus cuadros.
    if e.angulo != 0.0 {
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        for o in salida.iter_mut() {
            girar_orden(o, centro, e.angulo);
        }
    }

    salida
}

/// Los cuadros de la barra y sus numeros.
///
/// Sin escala no dibuja nada: una barra que no puede decir cuanto mide cada
/// cuadro es un adorno que engana.
fn barra(e: &Elemento, escala: Option<&Escala>, coma: char) -> Vec<Orden> {
    let Some(esc) = escala.filter(|x| x.valida()) else {
        return Vec::new();
    };
    let Some(rep) = crate::escalabarra::repartir(e.ancho, esc, 4) else {
        return Vec::new();
    };
    let ancho_cuadro = rep.ancho_usado_px / rep.cuadros as f32;
    let alto = e.alto.max(4.0) * 0.5;
    let tinta = con_opacidad(e.trazo, e.opacidad);
    let claro = ColorRgba {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: tinta.a,
    };

    let mut fuera = Vec::with_capacity(rep.cuadros as usize * 2 + 2);
    for i in 0..rep.cuadros {
        let x0 = e.x + i as f32 * ancho_cuadro;
        let x1 = x0 + ancho_cuadro;
        // A cuadros alternos: todos iguales no se podrian contar.
        let color = if i % 2 == 0 { tinta } else { claro };
        fuera.push(Orden::Relleno {
            puntos: vec![
                Punto2::nuevo(x0, e.y),
                Punto2::nuevo(x1, e.y),
                Punto2::nuevo(x1, e.y + alto),
                Punto2::nuevo(x0, e.y + alto),
            ],
            color,
        });
    }
    // El numero del principio y el del final: con esos dos se lee la barra.
    for (i, valor) in [(0u32, 0.0f32), (rep.cuadros, rep.cuadros as f32 * rep.paso)] {
        let x = e.x + i as f32 * ancho_cuadro;
        fuera.push(Orden::Texto {
            texto: crate::medida::formatear_valor(valor, &esc.unidad, esc.decimales, coma),
            x,
            y: e.y + alto + 2.0,
            tam: alto,
            familia: "Segoe UI".to_string(),
            color: tinta,
            ancho_max: ancho_cuadro * 2.0,
        });
    }
    fuera
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::formas::TipoPunta;

    #[test]
    fn el_marco_de_seleccion_rodea_la_caja_con_holgura() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(base());
        let (x0, y0, x1, y1) = escena.buscar(id).unwrap().caja();

        let Some(Orden::Polilinea { puntos, .. }) = marco_de_seleccion(&escena, id, 1.0) else {
            panic!("tiene que haber marco");
        };
        assert_eq!(puntos.len(), 5, "cerrado: el ultimo repite el primero");
        assert_eq!(puntos[0], puntos[4]);
        let h = HOLGURA_SELECCION;
        assert_eq!((puntos[0].x, puntos[0].y), (x0 - h, y0 - h));
        assert_eq!((puntos[2].x, puntos[2].y), (x1 + h, y1 + h));

        // Caso negativo: lo borrado no se marca, y un id que no existe
        // tampoco. Si no, quedaria un marco flotando sobre nada.
        escena.borrar(id);
        assert!(marco_de_seleccion(&escena, id, 1.0).is_none());
        assert!(marco_de_seleccion(&escena, 9999, 1.0).is_none());
    }

    fn base() -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Rectangulo,
            x: 10.0,
            y: 10.0,
            ancho: 100.0,
            alto: 50.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 99,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }
    }

    #[test]
    fn un_elemento_borrado_no_produce_ninguna_orden() {
        let e = Elemento {
            borrado: true,
            ..base()
        };
        assert!(ordenes(&e).is_empty());
    }

    #[test]
    fn el_relleno_se_pinta_antes_que_el_trazo() {
        // Al reves, el relleno taparia el borde y la figura se veria sin
        // contorno.
        let e = Elemento {
            relleno: Some(ColorRgba::opaco(1.0, 1.0, 0.0)),
            estilo_relleno: EstiloRelleno::Solido,
            ..base()
        };
        let o = ordenes(&e);
        assert!(matches!(o[0], Orden::Relleno { .. }), "primero el relleno");
        assert!(o[1..].iter().any(|x| matches!(x, Orden::Polilinea { .. })));
    }

    /// Las polilineas del color del relleno: las rayas, y no el contorno.
    fn rayas_de(e: &Elemento) -> Vec<Vec<Punto2>> {
        let relleno = e.relleno.expect("el elemento del caso tiene relleno");
        ordenes(e)
            .into_iter()
            .filter_map(|o| match o {
                Orden::Polilinea { puntos, color, .. } if color == relleno => Some(puntos),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn un_relleno_rayado_sale_como_rayas_y_no_como_mancha() {
        // Lo que faltaba: un dibujo rayado del movil se veia aqui como una
        // mancha de color plano.
        let e = Elemento {
            relleno: Some(ColorRgba::opaco(1.0, 1.0, 0.0)),
            estilo_relleno: EstiloRelleno::Rayado,
            ..base()
        };
        let o = ordenes(&e);
        assert!(
            !o.iter().any(|x| matches!(x, Orden::Relleno { .. })),
            "el rayado no puede traer ademas la mancha solida"
        );
        let rayas = rayas_de(&e);
        assert!(rayas.len() > 3, "solo salieron {} rayas", rayas.len());
        for r in &rayas {
            assert_eq!(r.len(), 2, "cada raya es un segmento");
        }
        // Y van antes del contorno, como el relleno solido.
        assert!(matches!(o[0], Orden::Polilinea { .. }));
    }

    #[test]
    fn las_rayas_del_relleno_son_mas_finas_que_el_contorno() {
        // `fillWeight: strokeWidth / 2`: con la pluma del contorno, el
        // interior pesaria mas que la propia figura.
        let e = Elemento {
            relleno: Some(ColorRgba::opaco(1.0, 1.0, 0.0)),
            estilo_relleno: EstiloRelleno::Rayado,
            grosor: 4.0,
            ..base()
        };
        for o in ordenes(&e) {
            if let Orden::Polilinea { color, grosor, .. } = o {
                if color == e.relleno.unwrap() {
                    assert_eq!(grosor, 2.0, "la raya tiene que ir a media pluma");
                }
            }
        }
    }

    #[test]
    fn una_figura_sin_relleno_no_produce_ni_una_raya() {
        // Caso negativo: el estilo de relleno por omision es rayado, asi que
        // sin esto CUALQUIER rectangulo saldria rayado por dentro.
        let e = base();
        assert!(e.relleno.is_none());
        assert_eq!(e.estilo_relleno, EstiloRelleno::Rayado);
        let solo_contorno = ordenes(&e);
        let sin_estilo = ordenes(&Elemento {
            estilo_relleno: EstiloRelleno::Solido,
            ..base()
        });
        assert_eq!(
            solo_contorno, sin_estilo,
            "sin relleno, el estilo de relleno no puede cambiar nada"
        );
    }

    #[test]
    fn el_rayado_de_una_elipse_se_queda_dentro_de_la_elipse() {
        let e = Elemento {
            figura: Figura::Elipse,
            relleno: Some(ColorRgba::opaco(0.0, 0.0, 1.0)),
            estilo_relleno: EstiloRelleno::Cruzado,
            rugosidad: 0.0,
            ..base()
        };
        // base(): x 10, y 10, ancho 100, alto 50.
        let (cx, cy, rx, ry) = (60.0f32, 35.0f32, 50.0f32, 25.0f32);
        let rayas = rayas_de(&e);
        assert!(!rayas.is_empty(), "una elipse rellena tiene que rayarse");
        for p in rayas.iter().flatten() {
            let dentro = ((p.x - cx) / rx).powi(2) + ((p.y - cy) / ry).powi(2);
            assert!(dentro <= 1.01, "la raya sale de la elipse en {p:?}");
        }
    }

    #[test]
    fn rellenar_una_figura_no_le_cambia_el_garabato_del_contorno() {
        // El rayado usa su propio generador: si gastara numeros del azar del
        // elemento, rellenar una figura ya dibujada la redibujaria con otro
        // temblor y el dibujo daria un salto.
        let contorno_de = |e: &Elemento| -> Vec<Vec<Punto2>> {
            let tinta = e.trazo;
            ordenes(e)
                .into_iter()
                .filter_map(|o| match o {
                    Orden::Polilinea { puntos, color, .. } if color == tinta => Some(puntos),
                    _ => None,
                })
                .collect()
        };
        let vacia = base();
        let rayada = Elemento {
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
            estilo_relleno: EstiloRelleno::Rayado,
            ..base()
        };
        assert_eq!(contorno_de(&vacia), contorno_de(&rayada));
    }

    #[test]
    fn el_mismo_elemento_produce_siempre_las_mismas_ordenes() {
        // La puerta de la fase, a nivel de dibujo: reabrir el documento no
        // cambia ni un punto.
        let e = base();
        assert_eq!(ordenes(&e), ordenes(&e));
    }

    #[test]
    fn cambiar_la_semilla_cambia_el_dibujo() {
        // Caso negativo: si la semilla no llegara hasta aqui, todas las
        // figuras del documento saldrian calcadas.
        let a = base();
        let b = Elemento {
            semilla: 100,
            ..base()
        };
        assert_ne!(ordenes(&a), ordenes(&b));
    }

    #[test]
    fn el_resaltador_es_translucido_y_de_grosor_constante() {
        // D45: si adelgazara o fuera opaco, el texto de debajo quedaria
        // ilegible, que es justo lo contrario de resaltar.
        let e = Elemento {
            figura: Figura::Resaltador {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
            },
            trazo: ColorRgba::opaco(1.0, 1.0, 0.0),
            ..base()
        };
        match &ordenes(&e)[0] {
            Orden::Tinta { color, .. } => {
                assert!(color.a < 0.5, "no es translucido: alfa {}", color.a)
            }
            otra => panic!("el resaltador deberia ser tinta, es {otra:?}"),
        }
    }

    #[test]
    fn una_flecha_con_las_dos_puntas_dibuja_mas_que_una_con_una() {
        let con_una = Elemento {
            figura: Figura::Flecha {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
                punta_inicio: TipoPunta::Ninguna,
                punta_fin: TipoPunta::Flecha,
                codos: false,
            },
            ..base()
        };
        let con_dos = Elemento {
            figura: Figura::Flecha {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
                punta_inicio: TipoPunta::Flecha,
                punta_fin: TipoPunta::Flecha,
                codos: false,
            },
            ..base()
        };
        assert!(ordenes(&con_dos).len() > ordenes(&con_una).len());
    }

    #[test]
    fn la_opacidad_del_elemento_llega_al_color() {
        let e = Elemento {
            opacidad: 0.5,
            ..base()
        };
        match &ordenes(&e)[0] {
            Orden::Polilinea { color, .. } => assert_eq!(color.a, 0.5),
            otra => panic!("se esperaba una polilinea, es {otra:?}"),
        }
    }

    #[test]
    fn la_escena_dibuja_de_abajo_arriba_y_salta_lo_borrado() {
        let mut escena = Escena::nueva();
        escena.anadir(base());
        let dos = escena.anadir(Elemento {
            figura: Figura::Elipse,
            ..base()
        });
        let con_ambos = ordenes_de_escena(&escena).len();
        escena.borrar(dos);
        assert!(ordenes_de_escena(&escena).len() < con_ambos);
    }

    #[test]
    fn un_emoji_se_pinta_como_texto_con_la_fuente_de_emojis_a_su_tamano() {
        let e = Elemento {
            figura: Figura::Emoji {
                caracter: "🪐".into(),
            },
            x: 10.0,
            y: 20.0,
            ancho: 100.0,
            alto: 100.0,
            ..base()
        };
        let o = ordenes(&e);
        assert!(o.iter().any(|o| matches!(o,
            Orden::Texto { texto, familia, tam, .. }
                if texto == "🪐" && familia == "Segoe UI Emoji" && (*tam - 80.0).abs() < 0.01)));
        // Caso negativo: un emoji no tiene contorno que pintar.
        assert!(
            !o.iter().any(|o| matches!(o, Orden::Polilinea { .. })),
            "un emoji no lleva trazo"
        );
    }

    #[test]
    fn un_texto_produce_una_orden_de_texto_con_su_ancho() {
        let e = Elemento {
            figura: Figura::Texto {
                texto: "hola".into(),
                tam: 14.0,
                familia: "Segoe UI".into(),
            },
            ..base()
        };
        match &ordenes(&e)[0] {
            Orden::Texto {
                texto, ancho_max, ..
            } => {
                assert_eq!(texto, "hola");
                assert_eq!(*ancho_max, 100.0);
            }
            otra => panic!("se esperaba texto, es {otra:?}"),
        }
    }

    fn foco(elipse: bool) -> Elemento {
        Elemento {
            figura: Figura::Foco { elipse },
            x: 10.0,
            y: 20.0,
            ancho: 100.0,
            alto: 50.0,
            trazo: ColorRgba::opaco(1.0, 1.0, 1.0),
            relleno: Some(ColorRgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.6,
            }),
            rugosidad: 0.0,
            ..base()
        }
    }

    #[test]
    fn el_foco_produce_un_velo_con_hueco_rectangular_y_su_borde() {
        // D51: el motor no sabe cuanto mide el lienzo, asi que entrega el
        // HUECO y el consumidor oscurece todo lo demas.
        let o = ordenes(&foco(false));
        let Orden::Velo { hueco, color } = &o[0] else {
            panic!("la primera orden del foco debe ser el velo, fue {:?}", o[0]);
        };
        assert_eq!(hueco.len(), 4);
        assert_eq!(hueco[0], Punto2::nuevo(10.0, 20.0));
        assert_eq!(hueco[2], Punto2::nuevo(110.0, 70.0));
        assert!((color.a - 0.6).abs() < 1e-6);
        assert!(
            matches!(o[1], Orden::Polilinea { .. }),
            "tras el velo va el borde del hueco"
        );
    }

    #[test]
    fn el_foco_eliptico_tiene_un_hueco_redondo() {
        let o = ordenes(&foco(true));
        let Orden::Velo { hueco, .. } = &o[0] else {
            panic!("velo esperado");
        };
        // Una elipse lisa tiene muchos mas vertices que un rectangulo.
        assert!(hueco.len() > 16, "hueco con {} puntos", hueco.len());
    }

    /// Un foco sin relleno oscurece lo que oscurece el movil, que son 45 y no
    /// 60: con 60, un foco del telefono abierto aqui apagaba de mas y lo de
    /// alrededor —que es para lo que sirve un foco— dejaba de verse.
    #[test]
    fn el_foco_sin_relleno_oscurece_lo_mismo_que_en_el_movil() {
        // Y el caso negativo del que ya estaba: un fichero antiguo o un
        // consumidor descuidado que no ponga relleno tampoco puede dejar el
        // velo transparente.
        let e = Elemento {
            relleno: None,
            ..foco(false)
        };
        let Orden::Velo { color, .. } = &ordenes(&e)[0] else {
            panic!("velo esperado");
        };
        let esperado = crate::lupa_elemento::OSCURECER_POR_DEFECTO as f32 / 100.0;
        assert!((color.a - esperado).abs() < 1e-6, "oscurece {}", color.a);
        assert!(color.a > 0.0, "un velo transparente no es un foco");
    }

    /// Todos los puntos de una lista de ordenes, para poder compararlas.
    fn puntos_de(ordenes: &[Orden]) -> Vec<Punto2> {
        ordenes
            .iter()
            .flat_map(|o| match o {
                Orden::Poligono { puntos, .. }
                | Orden::Tinta {
                    contorno: puntos, ..
                }
                | Orden::Polilinea { puntos, .. }
                | Orden::Relleno { puntos, .. }
                | Orden::Velo { hueco: puntos, .. } => puntos.clone(),
                Orden::Texto { x, y, .. } => vec![Punto2::nuevo(*x, *y)],
                Orden::Imagen { x, y, .. } => vec![Punto2::nuevo(*x, *y)],
            })
            .collect()
    }

    #[test]
    fn un_elemento_girado_se_dibuja_girado() {
        // El fallo que motiva esta tarea: hasta ahora `angulo` no llegaba al
        // dibujo, asi que un plano girado en el movil se abria sin girar.
        let recto = base();
        let mut girado = base();
        girado.angulo = std::f32::consts::FRAC_PI_2;

        let a = puntos_de(&ordenes(&recto));
        let b = puntos_de(&ordenes(&girado));

        assert_eq!(a.len(), b.len(), "la misma geometria, en otro sitio");
        assert!(
            a.iter().zip(&b).any(|(p, q)| p.distancia(*q) > 1.0),
            "girar un cuarto de vuelta tiene que mover algo"
        );
    }

    #[test]
    fn girar_no_cambia_el_garabato_solo_lo_orienta() {
        // D38: la semilla manda sobre el aspecto. Si el angulo entrara en los
        // generadores de rugosidad, girar un rectangulo lo redibujaria distinto
        // y el dibujo "temblaria" al girarlo.
        let recto = base();
        let mut girado = base();
        girado.angulo = 0.7;

        let (x0, y0, x1, y1) = recto.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);

        let a = puntos_de(&ordenes(&recto));
        let b = puntos_de(&ordenes(&girado));

        for (p, q) in a.iter().zip(&b) {
            let esperado = p.girar(centro, 0.7);
            assert!(
                esperado.distancia(*q) < 1e-3,
                "cada punto es el mismo, girado: esperaba {esperado:?}, es {q:?}"
            );
        }
    }

    #[test]
    fn un_elemento_sin_girar_produce_exactamente_lo_de_antes() {
        // La red de seguridad: con angulo cero, ni un punto se mueve. Todo lo
        // que ya funcionaba tiene que seguir dando byte a byte lo mismo.
        let e = base();
        assert_eq!(e.angulo, 0.0, "la base no esta girada");
        let dos_veces = ordenes(&e);
        assert_eq!(ordenes(&e), dos_veces, "y sigue siendo reproducible");
    }

    #[test]
    fn un_lapiz_se_pinta_como_tinta_y_no_como_poligono() {
        let e = Elemento {
            figura: Figura::Lapiz {
                puntos: (0..20)
                    .map(|i| Punto2::nuevo(i as f32 * 4.0, 0.0))
                    .collect(),
                presiones: Vec::new(),
                opciones: Some(crate::tinta::OpcionesTinta::default()),
            },
            ..base()
        };
        assert!(matches!(ordenes(&e).first(), Some(Orden::Tinta { .. })));
    }

    #[test]
    fn una_firma_de_cuarenta_pixeles_sigue_siendo_tinta() {
        // Antes de E1, por debajo de 48 px salia una raya de grosor fijo: las
        // letras y las firmas pequenas nunca se veian como tinta.
        let e = Elemento {
            figura: Figura::Lapiz {
                puntos: (0..20)
                    .map(|i| Punto2::nuevo(i as f32 * 2.0, (i as f32).sin() * 5.0))
                    .collect(),
                presiones: Vec::new(),
                opciones: Some(crate::tinta::OpcionesTinta::default()),
            },
            ..base()
        };
        assert!(matches!(
            ordenes_a_distancia(&e, 1.0).first(),
            Some(Orden::Tinta { .. })
        ));
    }

    #[test]
    fn el_trazo_a_mano_tambien_se_gira() {
        // Es el caso que junta esta tarea con la 4: los puntos van en marco
        // local y es el dibujo quien los orienta.
        let mut e = Elemento {
            figura: Figura::Lapiz {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
                presiones: Vec::new(),
                opciones: None,
            },
            ..base()
        };
        let recto = puntos_de(&ordenes(&e));

        e.angulo = std::f32::consts::FRAC_PI_2;
        let girado = puntos_de(&ordenes(&e));

        assert_eq!(recto.len(), girado.len());
        assert!(
            recto
                .iter()
                .zip(&girado)
                .any(|(p, q)| p.distancia(*q) > 1.0),
            "un trazo girado no se dibuja igual que uno recto"
        );
    }

    #[test]
    fn el_trazo_simplificado_a_linea_tambien_se_gira() {
        // `ordenes_a_distancia` no siempre delega en `ordenes`: la rama que
        // convierte tinta pequena en linea construye su propia orden a partir
        // de los puntos crudos, y esos tambien estan en marco local.
        let mut e = Elemento {
            figura: Figura::Lapiz {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(10.0, 0.0)],
                presiones: Vec::new(),
                opciones: None,
            },
            ..base()
        };
        let recto = puntos_de(&ordenes_a_distancia(&e, 1.0));

        e.angulo = std::f32::consts::FRAC_PI_2;
        let girado = puntos_de(&ordenes_a_distancia(&e, 1.0));

        assert_eq!(recto.len(), girado.len());
        assert!(
            recto
                .iter()
                .zip(&girado)
                .any(|(p, q)| p.distancia(*q) > 1.0),
            "un trazo simplificado y girado no se dibuja igual que uno recto"
        );
    }

    #[test]
    fn el_dibujo_y_el_picado_coinciden_en_un_elemento_girado() {
        // La prueba que cierra el agujero: donde se ve es donde se toca.
        // Antes de esta tarea, `impacto` des-giraba y `pintado` no giraba, asi
        // que el elemento se veia en un sitio y se tocaba en otro.
        let mut e = base();
        e.angulo = std::f32::consts::FRAC_PI_2;
        e.relleno = Some(ColorRgba::opaco(1.0, 0.0, 0.0));

        for p in puntos_de(&ordenes(&e)) {
            assert!(
                crate::impacto::toca(&e, p),
                "se dibuja en {p:?} pero ahi no se toca"
            );
        }
    }

    fn cota_de(a: Punto2, b: Punto2) -> Elemento {
        Elemento {
            figura: Figura::Cota { puntos: vec![a, b] },
            x: a.x.min(b.x),
            y: a.y.min(b.y),
            ancho: (b.x - a.x).abs(),
            alto: (b.y - a.y).abs(),
            ..base()
        }
    }

    fn metros(por_pixel: f32) -> Escala {
        Escala {
            unidades_por_pixel: por_pixel,
            unidad: "m".to_string(),
            decimales: 2,
        }
    }

    fn textos_de(ordenes: &[Orden]) -> Vec<String> {
        ordenes
            .iter()
            .filter_map(|o| match o {
                Orden::Texto { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn la_raya_de_la_cota_se_dibuja_sin_texto() {
        // La geometria no depende de la escala, asi que se puede cachear.
        let c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        let o = ordenes(&c);
        assert!(!o.is_empty(), "algo dibuja");
        assert!(textos_de(&o).is_empty(), "pero el texto no va aqui");
    }

    #[test]
    fn el_rotulo_dice_pixeles_sin_escala_y_unidades_con_ella() {
        let c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));

        let sin = textos_de(&ordenes_medibles(&c, None, ','));
        assert_eq!(sin, vec!["100 px".to_string()]);

        let con = textos_de(&ordenes_medibles(&c, Some(&metros(0.01)), ','));
        assert_eq!(con, vec!["1,00 m".to_string()]);
    }

    #[test]
    fn una_cota_no_puede_mentir() {
        // LA prueba que justifica no guardar el texto (D34). Se mueve un
        // extremo y el numero cambia solo, porque se deriva al pintar.
        let mut c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        let e = metros(0.01);
        let antes = textos_de(&ordenes_medibles(&c, Some(&e), ','));

        let Figura::Cota { puntos } = &mut c.figura else {
            panic!()
        };
        puntos[1] = Punto2::nuevo(200.0, 0.0);

        let despues = textos_de(&ordenes_medibles(&c, Some(&e), ','));
        assert_ne!(antes, despues, "el rotulo tiene que haber cambiado");
        assert_eq!(despues, vec!["2,00 m".to_string()]);
    }

    #[test]
    fn calibrar_cambia_los_rotulos_sin_tocar_el_elemento() {
        // El motivo de sacar el rotulo de la cache: la escala cambia y el
        // elemento no, asi que la version no sube y la cache no se enteraria.
        let c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        let a = textos_de(&ordenes_medibles(&c, Some(&metros(0.01)), ','));
        let b = textos_de(&ordenes_medibles(&c, Some(&metros(0.02)), ','));
        assert_ne!(a, b, "otra escala, otro rotulo");
    }

    #[test]
    fn el_rotulo_va_en_medio_de_la_raya() {
        let c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        let o = ordenes_medibles(&c, Some(&metros(0.01)), ',');
        let Some(Orden::Texto { x, y, .. }) = o.iter().find(|o| matches!(o, Orden::Texto { .. }))
        else {
            panic!("hay rotulo")
        };
        assert!((x - 50.0).abs() < 20.0, "cerca del medio en x: {x}");
        assert!(y.abs() < 30.0, "y cerca de la raya: {y}");
    }

    #[test]
    fn una_barra_sin_escala_no_dibuja_cuadros() {
        // Una barra que no puede decir cuanto mide cada cuadro es un adorno
        // que engana.
        let b = Elemento {
            figura: Figura::EscalaGrafica,
            x: 0.0,
            y: 0.0,
            ancho: 400.0,
            alto: 24.0,
            ..base()
        };
        assert!(ordenes_medibles(&b, None, ',').is_empty());
    }

    #[test]
    fn una_barra_con_escala_dibuja_sus_cuadros_y_sus_numeros() {
        let b = Elemento {
            figura: Figura::EscalaGrafica,
            x: 0.0,
            y: 0.0,
            ancho: 400.0,
            alto: 24.0,
            ..base()
        };
        let o = ordenes_medibles(&b, Some(&metros(0.01)), ',');
        let rellenos = o
            .iter()
            .filter(|x| matches!(x, Orden::Relleno { .. }))
            .count();
        assert!(rellenos >= 2, "al menos dos cuadros: {rellenos}");
        let textos = textos_de(&o);
        assert!(!textos.is_empty(), "y sus numeros");
        assert!(
            textos.iter().any(|t| t.contains('m')),
            "con la unidad: {textos:?}"
        );
    }

    #[test]
    fn los_cuadros_de_la_barra_alternan() {
        // La reglita es a cuadros blancos y negros: si fueran todos iguales
        // no se podrian contar.
        let b = Elemento {
            figura: Figura::EscalaGrafica,
            x: 0.0,
            y: 0.0,
            ancho: 400.0,
            alto: 24.0,
            ..base()
        };
        let o = ordenes_medibles(&b, Some(&metros(0.01)), ',');
        let colores: Vec<ColorRgba> = o
            .iter()
            .filter_map(|x| match x {
                Orden::Relleno { color, .. } => Some(*color),
                _ => None,
            })
            .collect();
        assert!(colores.len() >= 2);
        assert_ne!(
            colores[0], colores[1],
            "el segundo cuadro es del otro color"
        );
    }

    #[test]
    fn una_figura_que_no_mide_no_produce_ordenes_medibles() {
        assert!(ordenes_medibles(&base(), Some(&metros(0.01)), ',').is_empty());
    }

    #[test]
    fn el_rotulo_de_una_cota_girada_cae_donde_lo_deja_el_giro() {
        // El fallo de esta ronda: `ordenes()` aplica `e.angulo` al final,
        // `ordenes_medibles()` no lo aplicaba nunca. Sin escala calculamos a
        // mano donde tiene que caer el rotulo antes de girar, y comparamos
        // ese punto GIRADO con `Punto2::girar` contra lo que sale del
        // programa. No se copia el valor de la salida.
        let mut c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        c.angulo = std::f32::consts::FRAC_PI_2;

        // Sin girar: medio de la raya (50, 0), desplazado "alto" =
        // grosor.max(1.0) * 6.0 = 12.0 en la perpendicular a (1, 0), que es
        // (0, -1). El centro de giro es el centro de la caja de la cota:
        // con grosor 2.0 la caja es (-1, -1, 101, 1), centro (50, 0).
        let sin_girar = Punto2::nuevo(50.0, -12.0);
        let centro = Punto2::nuevo(50.0, 0.0);
        let esperado = sin_girar.girar(centro, c.angulo);

        let o = ordenes_medibles(&c, Some(&metros(0.01)), ',');
        let Some(Orden::Texto { x, y, .. }) = o.iter().find(|o| matches!(o, Orden::Texto { .. }))
        else {
            panic!("hay rotulo")
        };
        assert!(
            (*x - esperado.x).abs() < 1e-2 && (*y - esperado.y).abs() < 1e-2,
            "esperaba el rotulo en {esperado:?}, salio en ({x}, {y})"
        );
    }

    #[test]
    fn el_rotulo_sigue_pegado_a_su_raya_al_girar() {
        // Si el rotulo no girara con la raya, se despegaria de ella al girar
        // la cota: es justo el fallo que motiva esta ronda. La distancia del
        // rotulo al centro de la cota NO sirve para cazarlo aqui: en una
        // cota de dos puntos el centro de giro coincide con el punto medio
        // de la raya, y la rotacion conserva la distancia a su propio
        // centro se aplique o no al rotulo. Lo que si delata el fallo es la
        // perpendicularidad: el rotulo se pinta perpendicular a la raya, asi
        // que si la raya gira y el rotulo no, dejan de ser perpendiculares.
        let mut c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        c.angulo = 0.7;

        let ordenes_raya = ordenes(&c);
        let Some(Orden::Polilinea { puntos, .. }) = ordenes_raya.first() else {
            panic!("la raya es la primera orden de ordenes()")
        };
        let (a, b) = (puntos[0], puntos[1]);
        let medio = a.hacia(b, 0.5);
        let direccion = b.restar(a).unitario();

        let o = ordenes_medibles(&c, Some(&metros(0.01)), ',');
        let Some(Orden::Texto { x, y, .. }) = o.iter().find(|o| matches!(o, Orden::Texto { .. }))
        else {
            panic!("hay rotulo")
        };
        let hacia_rotulo = Punto2::nuevo(*x, *y).restar(medio);

        assert!(
            hacia_rotulo.producto(direccion).abs() < 1e-2,
            "el rotulo ya no es perpendicular a su raya girada: producto {}",
            hacia_rotulo.producto(direccion)
        );
    }

    #[test]
    fn una_barra_girada_dibuja_sus_cuadros_girados() {
        let recta = Elemento {
            figura: Figura::EscalaGrafica,
            x: 0.0,
            y: 0.0,
            ancho: 400.0,
            alto: 24.0,
            ..base()
        };
        let girada = Elemento {
            angulo: std::f32::consts::FRAC_PI_2,
            ..recta.clone()
        };

        let a = puntos_de(&ordenes_medibles(&recta, Some(&metros(0.01)), ','));
        let b = puntos_de(&ordenes_medibles(&girada, Some(&metros(0.01)), ','));

        assert_eq!(a.len(), b.len(), "la misma geometria, en otro sitio");
        assert!(
            a.iter().zip(&b).any(|(p, q)| p.distancia(*q) > 1.0),
            "girar la barra tiene que mover algo"
        );
    }

    #[test]
    fn ordenes_medibles_con_escala_invalida_cae_a_pixeles_y_la_barra_no_dibuja() {
        let invalida = Escala {
            unidades_por_pixel: 0.0,
            unidad: "m".to_string(),
            decimales: 2,
        };
        assert!(
            !invalida.valida(),
            "la escala del caso tiene que ser invalida"
        );

        let c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        let textos = textos_de(&ordenes_medibles(&c, Some(&invalida), ','));
        assert_eq!(
            textos,
            vec!["100 px".to_string()],
            "escala invalida se trata como si no hubiera escala"
        );

        let b = Elemento {
            figura: Figura::EscalaGrafica,
            x: 0.0,
            y: 0.0,
            ancho: 400.0,
            alto: 24.0,
            ..base()
        };
        assert!(
            ordenes_medibles(&b, Some(&invalida), ',').is_empty(),
            "una barra que no puede decir cuanto mide cada cuadro no dibuja nada"
        );
    }

    #[test]
    fn una_escena_con_cota_y_barra_saca_texto_y_cuadros_por_los_dos_caminos() {
        // El hallazgo 2 de la revision final: `ordenes_medibles` es la unica
        // que produce el rotulo de una cota y los cuadros de una barra, y
        // solo la llamaba la ventana del editor. La capa de pantalla, la
        // recarga de un pin y la exportacion pasan por `ordenes_de_escena` o
        // `ordenes_de_escena_vista`, que no la encadenaban: una escena con
        // una barra calibrada salia sin un solo texto, y la barra sin sus
        // cuadros era invisible del todo (D37: «sale en la exportacion»).
        let mut escena = Escena::nueva();
        escena.escala = Some(metros(0.01));
        escena.anadir(cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)));
        escena.anadir(Elemento {
            figura: Figura::EscalaGrafica,
            x: 200.0,
            y: 0.0,
            ancho: 400.0,
            alto: 24.0,
            ..base()
        });

        let de_escena = ordenes_de_escena(&escena);
        assert!(
            de_escena.iter().any(|o| matches!(o, Orden::Texto { .. })),
            "el rotulo de la cota tiene que salir en ordenes_de_escena"
        );
        assert!(
            de_escena
                .iter()
                .filter(|o| matches!(o, Orden::Relleno { .. }))
                .count()
                >= 2,
            "los cuadros de la barra tienen que salir en ordenes_de_escena"
        );

        let camara = crate::camara::Camara {
            x: 200.0,
            y: 0.0,
            zoom: 1.0,
        };
        let de_vista = ordenes_de_escena_vista(&escena, &camara, 2000.0, 2000.0);
        assert!(
            de_vista.iter().any(|o| matches!(o, Orden::Texto { .. })),
            "el rotulo de la cota tiene que salir en ordenes_de_escena_vista"
        );
        assert!(
            de_vista
                .iter()
                .filter(|o| matches!(o, Orden::Relleno { .. }))
                .count()
                >= 2,
            "los cuadros de la barra tienen que salir en ordenes_de_escena_vista"
        );

        for o in de_escena.iter().chain(de_vista.iter()) {
            if let Orden::Texto { texto, .. } = o {
                assert!(
                    texto.contains(','),
                    "el separador decimal es la coma (D39): {texto}"
                );
            }
        }
    }

    #[test]
    fn el_rotulo_sin_escala_valida_va_en_gris_y_con_escala_en_el_trazo() {
        // D35: sin calibrar se mide en pixeles, y el gris es el aviso de que
        // no es medida de plano. El comentario de `medida.rs:130` decia
        // «quien lo pinta lo pone en gris» sin que nadie lo hiciera: nadie
        // usaba otro color que `e.trazo`, calibrada o no.
        let mut c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        c.trazo = ColorRgba::opaco(1.0, 0.0, 0.0); // rojo, para distinguirlo del gris

        let sin_escala = ordenes_medibles(&c, None, ',');
        let Some(Orden::Texto { color, .. }) =
            sin_escala.iter().find(|o| matches!(o, Orden::Texto { .. }))
        else {
            panic!("hay rotulo sin escala")
        };
        assert_eq!(
            *color, COLOR_SIN_ESCALA,
            "sin escala valida el rotulo tiene que ser gris, no {color:?}"
        );

        let con_escala = ordenes_medibles(&c, Some(&metros(0.01)), ',');
        let Some(Orden::Texto { color, .. }) =
            con_escala.iter().find(|o| matches!(o, Orden::Texto { .. }))
        else {
            panic!("hay rotulo con escala")
        };
        assert_eq!(
            *color, c.trazo,
            "con escala valida el rotulo va del color del trazo"
        );

        let invalida = Escala {
            unidades_por_pixel: 0.0,
            unidad: "m".to_string(),
            decimales: 2,
        };
        let con_invalida = ordenes_medibles(&c, Some(&invalida), ',');
        let Some(Orden::Texto { color, .. }) = con_invalida
            .iter()
            .find(|o| matches!(o, Orden::Texto { .. }))
        else {
            panic!("hay rotulo con escala invalida")
        };
        assert_eq!(
            *color, COLOR_SIN_ESCALA,
            "una escala invalida se trata como si no la hubiera"
        );
    }

    #[test]
    fn el_rotulo_no_sale_boca_abajo_en_una_cota_que_apunta_a_la_izquierda() {
        // `rotulo_del_reves` estaba implementada, probada y sin conectar, ni
        // reexportada (hallazgo 6). Sin usarla, una cota que apunta "hacia
        // atras" (mas de 90 grados) pone el rotulo del lado contrario de la
        // raya al de la misma cota apuntando "hacia adelante": ese cambio de
        // lado es exactamente leerlo boca abajo cuando el resto del plano se
        // lee del derecho.
        let derecha = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        let izquierda = cota_de(Punto2::nuevo(100.0, 0.0), Punto2::nuevo(0.0, 0.0));

        let y_de = |e: &Elemento| -> f32 {
            let o = ordenes_medibles(e, Some(&metros(0.01)), ',');
            match o.iter().find(|o| matches!(o, Orden::Texto { .. })) {
                Some(Orden::Texto { y, .. }) => *y,
                _ => panic!("hay rotulo"),
            }
        };

        assert!(
            (y_de(&derecha) - y_de(&izquierda)).abs() < 1e-3,
            "el rotulo tiene que quedar del mismo lado de la raya apunte hacia \
             donde apunte: {} vs {}",
            y_de(&derecha),
            y_de(&izquierda)
        );
    }

    #[test]
    fn una_cota_de_longitud_cero_no_da_panico_ni_nan() {
        let c = cota_de(Punto2::nuevo(5.0, 5.0), Punto2::nuevo(5.0, 5.0));
        let o = ordenes_medibles(&c, Some(&metros(0.01)), ',');
        for orden in &o {
            if let Orden::Texto { x, y, .. } = orden {
                assert!(
                    x.is_finite() && y.is_finite(),
                    "coordenadas no finitas: ({x}, {y})"
                );
            }
        }
    }
}
