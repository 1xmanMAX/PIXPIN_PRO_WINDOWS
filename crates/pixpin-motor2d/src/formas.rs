//! Rectangulos, elipses, lineas y flechas dibujados "a mano".
//!
//! Una linea recta perfecta no parece dibujada. El truco que usan todas las
//! herramientas de este estilo es doble:
//!
//! 1. **Desviar los extremos** un poco, con el azar reproducible del
//!    elemento, y curvar el trazo por el medio con una Bezier cubica cuyos
//!    puntos de control tambien estan desviados.
//! 2. **Trazar cada linea DOS veces**, con desvios distintos. Dos pasadas casi
//!    iguales es exactamente lo que hace una mano al repasar un boceto, y es
//!    lo que separa "linea temblorosa" de "linea dibujada".
//!
//! El azar viene del elemento (su semilla), asi que el mismo rectangulo se
//! dibuja igual siempre: es lo que permite guardar un documento y reabrirlo
//! sin que cambie de aspecto.

use crate::azar::Azar;
use crate::vector::Punto2;

/// Cuantos tramos tiene cada linea "a mano": suficientes para que la curva se
/// vea suave sin llenar la geometria de puntos.
const TRAMOS: usize = 12;

/// Una linea "a mano" de `a` a `b`: dos pasadas, cada una con su desvio.
///
/// `rugosidad` 0 devuelve la linea recta exacta (util para reglas y para el
/// resaltador); 1 es el aspecto normal; 2, muy marcado.
pub fn linea(a: Punto2, b: Punto2, rugosidad: f32, azar: &mut Azar) -> Vec<Vec<Punto2>> {
    if rugosidad <= 0.0 {
        return vec![vec![a, b]];
    }
    // El desvio crece con la longitud pero se estanca: en una linea de mil
    // pixeles, un temblor proporcional se veria como un garabato.
    let largo = a.distancia(b);
    let desvio = (largo / 40.0).clamp(1.0, 4.0) * rugosidad;

    (0..2)
        .map(|pasada| {
            // La segunda pasada desvia algo menos: al repasar, la mano ya
            // sabe por donde va.
            let d = if pasada == 0 { desvio } else { desvio * 0.7 };
            let inicio = Punto2::nuevo(a.x + azar.desvio(d), a.y + azar.desvio(d));
            let fin = Punto2::nuevo(b.x + azar.desvio(d), b.y + azar.desvio(d));
            // Dos puntos de control desviados: la panza de la linea.
            let c1 = a
                .hacia(b, 0.33)
                .sumar(Punto2::nuevo(azar.desvio(d * 2.0), azar.desvio(d * 2.0)));
            let c2 = a
                .hacia(b, 0.66)
                .sumar(Punto2::nuevo(azar.desvio(d * 2.0), azar.desvio(d * 2.0)));
            bezier(inicio, c1, c2, fin)
        })
        .collect()
}

/// Bezier cubica evaluada en `TRAMOS` tramos.
fn bezier(p0: Punto2, c1: Punto2, c2: Punto2, p3: Punto2) -> Vec<Punto2> {
    (0..=TRAMOS)
        .map(|i| {
            let t = i as f32 / TRAMOS as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            Punto2::nuevo(
                p0.x * a + c1.x * b + c2.x * c + p3.x * d,
                p0.y * a + c1.y * b + c2.y * c + p3.y * d,
            )
        })
        .collect()
}

/// Los cuatro lados de un rectangulo, cada uno con sus dos pasadas.
pub fn rectangulo(
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    rugosidad: f32,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    let e = [
        Punto2::nuevo(x, y),
        Punto2::nuevo(x + ancho, y),
        Punto2::nuevo(x + ancho, y + alto),
        Punto2::nuevo(x, y + alto),
    ];
    let mut salida = Vec::with_capacity(8);
    for i in 0..4 {
        salida.extend(linea(e[i], e[(i + 1) % 4], rugosidad, azar));
    }
    salida
}

/// Los cuatro lados de un rombo, cada uno con sus dos pasadas.
///
/// Los vertices son los puntos medios de los lados de su caja, que es como
/// los pone Excalidraw (`diamond`): asi un rombo y un rectangulo del mismo
/// tamano se estiran igual y los tiradores caen en el mismo sitio.
pub fn rombo(
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    rugosidad: f32,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    let e = vertices_de_rombo(x, y, ancho, alto);
    let mut salida = Vec::with_capacity(8);
    for i in 0..4 {
        salida.extend(linea(e[i], e[(i + 1) % 4], rugosidad, azar));
    }
    salida
}

/// Los cuatro vertices de un rombo, en orden. Aparte de [`rombo`] porque el
/// relleno los necesita sin temblor: rellenar la figura temblorosa deja
/// huecos por donde se escapa el fondo.
pub fn vertices_de_rombo(x: f32, y: f32, ancho: f32, alto: f32) -> [Punto2; 4] {
    [
        Punto2::nuevo(x + ancho / 2.0, y),
        Punto2::nuevo(x + ancho, y + alto / 2.0),
        Punto2::nuevo(x + ancho / 2.0, y + alto),
        Punto2::nuevo(x, y + alto / 2.0),
    ]
}

/// Una elipse "a mano": dos vueltas completas ligeramente distintas.
///
/// No se cierra en el mismo punto donde empieza a proposito: una elipse
/// dibujada a mano casi nunca cierra exacta, y ese pequeño exceso es lo que
/// la hace creible.
pub fn elipse(
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    rugosidad: f32,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    let cx = x + ancho / 2.0;
    let cy = y + alto / 2.0;
    let rx = ancho / 2.0;
    let ry = alto / 2.0;

    if rugosidad <= 0.0 {
        let pasos = 48;
        return vec![
            (0..=pasos)
                .map(|i| {
                    let t = std::f32::consts::TAU * i as f32 / pasos as f32;
                    Punto2::nuevo(cx + rx * t.cos(), cy + ry * t.sin())
                })
                .collect(),
        ];
    }

    let desvio = ((rx + ry) / 40.0).clamp(1.0, 4.0) * rugosidad;
    let pasos = 32;
    (0..2)
        .map(|pasada| {
            let d = if pasada == 0 { desvio } else { desvio * 0.7 };
            // La segunda vuelta arranca con un pequeño desfase y se pasa un
            // poco del cierre: asi las dos pasadas no quedan superpuestas.
            let desfase = azar.desvio(0.4) + pasada as f32 * 0.3;
            let vuelta = std::f32::consts::TAU + if pasada == 1 { 0.35 } else { 0.0 };
            (0..=pasos)
                .map(|i| {
                    let t = desfase + vuelta * i as f32 / pasos as f32;
                    Punto2::nuevo(
                        cx + rx * t.cos() + azar.desvio(d),
                        cy + ry * t.sin() + azar.desvio(d),
                    )
                })
                .collect()
        })
        .collect()
}

/// La punta de una flecha: dos lineas cortas desde el extremo.
///
/// Se abre 25 grados a cada lado y mide una fraccion del tramo final, con un
/// tope: una flecha larguisima no puede tener una punta de doscientos pixeles.
pub fn punta_flecha(
    desde: Punto2,
    hasta: Punto2,
    rugosidad: f32,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    let largo = (desde.distancia(hasta) * 0.3).clamp(8.0, 40.0);
    let direccion = desde.restar(hasta).unitario();
    let apertura = 25.0f32.to_radians();

    let mut salida = Vec::with_capacity(4);
    for signo in [1.0f32, -1.0] {
        let (s, c) = (apertura * signo).sin_cos();
        let girado = Punto2::nuevo(
            direccion.x * c - direccion.y * s,
            direccion.x * s + direccion.y * c,
        );
        salida.extend(linea(
            hasta,
            hasta.proyectar(girado, largo),
            rugosidad,
            azar,
        ));
    }
    salida
}

/// **Las ocho puntas de flecha de Excalidraw**, con sus palabras del fichero.
///
/// Son las mismas ocho del movil (`Arrowhead` en `Element.kt:487-496`) y las
/// mismas de Excalidraw. Aqui solo habia «lleva punta o no»
/// (`Figura::Flecha{punta_inicio, punta_fin}`), asi que un diagrama entidad-
/// relacion del movil —donde la punta DICE la cardinalidad— se abria con ocho
/// flechas iguales y dejaba de decir nada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TipoPunta {
    /// Sin punta: es `null` en el fichero, no una palabra.
    #[default]
    Ninguna,
    /// Las dos rayas de siempre, abiertas.
    Flecha,
    /// Un travesano perpendicular.
    Barra,
    Circulo,
    CirculoHueco,
    Triangulo,
    TrianguloHueco,
    Rombo,
    RomboHueco,
}

/// Las ocho de la barra, en el orden del enum del movil. Sin `Ninguna`: no es
/// una punta, es no tener ninguna.
pub const PUNTAS: [TipoPunta; 8] = [
    TipoPunta::Flecha,
    TipoPunta::Barra,
    TipoPunta::Circulo,
    TipoPunta::CirculoHueco,
    TipoPunta::Triangulo,
    TipoPunta::TrianguloHueco,
    TipoPunta::Rombo,
    TipoPunta::RomboHueco,
];

impl TipoPunta {
    /// La palabra del `.excalidraw`. `None` es la ausencia de punta, que en el
    /// fichero se escribe `null` y no una cadena vacia.
    pub fn palabra(self) -> Option<&'static str> {
        Some(match self {
            TipoPunta::Ninguna => return None,
            TipoPunta::Flecha => "arrow",
            TipoPunta::Barra => "bar",
            TipoPunta::Circulo => "circle",
            TipoPunta::CirculoHueco => "circle_outline",
            TipoPunta::Triangulo => "triangle",
            TipoPunta::TrianguloHueco => "triangle_outline",
            TipoPunta::Rombo => "diamond",
            TipoPunta::RomboHueco => "diamond_outline",
        })
    }

    /// La punta de una palabra del fichero.
    ///
    /// Una palabra que no conocemos NO es «sin punta»: es una punta de una
    /// version futura, y leerla como ninguna la borraria al guardar. Devuelve
    /// `None` para que quien lee decida conservarla, la misma regla del
    /// `fillStyle` y del `material`.
    pub fn desde_palabra(s: &str) -> Option<TipoPunta> {
        PUNTAS.into_iter().find(|p| p.palabra() == Some(s))
    }

    /// Si se pinta maciza. Las «huecas» (`*_outline`) llevan solo el borde:
    /// es lo que distingue en un diagrama la herencia de la composicion.
    pub fn es_maciza(self) -> bool {
        matches!(
            self,
            TipoPunta::Circulo | TipoPunta::Triangulo | TipoPunta::Rombo
        )
    }

    /// Lo que mide la punta, en pixeles del documento (`arrowheadSize`).
    pub fn tamano(self) -> f32 {
        match self {
            TipoPunta::Flecha => 25.0,
            TipoPunta::Rombo | TipoPunta::RomboHueco => 12.0,
            _ => 15.0,
        }
    }

    /// Apertura de la punta, en grados (`getArrowheadAngle`). La barra se abre
    /// noventa: por eso sale perpendicular al trazo y no en pico.
    pub fn apertura(self) -> f32 {
        match self {
            TipoPunta::Barra => 90.0,
            TipoPunta::Flecha => 20.0,
            _ => 25.0,
        }
    }
}

/// La geometria de una punta, ya resuelta para dibujar (`ArrowheadShape`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormaDePunta {
    pub tipo: TipoPunta,
    /// El extremo del trazo, donde se posa la punta.
    pub punta: Punto2,
    /// Los dos puntos de las alas, o los extremos del travesano de la barra.
    pub alas: (Punto2, Punto2),
    /// El cuarto vertice: solo el rombo lo usa.
    pub opuesto: Option<Punto2>,
    /// Solo el circulo: su diametro.
    pub diametro: f32,
}

/// El punto desde el que se mira la direccion de la punta.
///
/// **No es el de al lado.** Con dos puntos da igual, pero una flecha trazada a
/// pulso tiene los puntos a un pixel unos de otros: mirando solo al anterior
/// sale una direccion temblorosa y, peor, una punta del tamano de ese pixel
/// —el tamano se acota a la mitad de esa distancia—, o sea invisible. Se
/// retrocede por el trazo hasta separarse lo que mide la punta, y asi apunta a
/// donde iba la mano y sale del tamano que le toca. Es lo que hace usable la
/// **flecha a mano alzada**, que por dentro es una flecha con todos los puntos
/// del trazo en vez de dos.
fn de_donde_viene(puntos: &[Punto2], al_final: bool, cuanto: f32) -> Punto2 {
    let n = puntos.len();
    let extremo = if al_final { puntos[n - 1] } else { puntos[0] };
    let mut ultimo = if al_final { puntos[n - 2] } else { puntos[1] };
    let recorrido: Box<dyn Iterator<Item = usize>> = if al_final {
        Box::new((0..n).rev())
    } else {
        Box::new(0..n)
    };
    for i in recorrido {
        let p = puntos[i];
        ultimo = p;
        if extremo.distancia(p) >= cuanto {
            return p;
        }
    }
    ultimo
}

/// Los puntos de la punta de `tipo` en un extremo del trazo
/// (`getArrowheadPoints`).
///
/// `al_final` elige el extremo: falso es el principio (`startArrowhead`).
/// `grosor` es el del trazo, y solo lo usa el circulo para su diametro.
///
/// `None` cuando no hay punta que dibujar: sin dos puntos, con los dos
/// extremos encima, o sin tipo.
pub fn forma_de_punta(
    puntos: &[Punto2],
    al_final: bool,
    tipo: TipoPunta,
    grosor: f32,
) -> Option<FormaDePunta> {
    if tipo == TipoPunta::Ninguna || puntos.len() < 2 {
        return None;
    }
    let extremo = if al_final {
        puntos[puntos.len() - 1]
    } else {
        puntos[0]
    };
    let previo = de_donde_viene(puntos, al_final, tipo.tamano());
    let distancia = extremo.distancia(previo);
    if distancia == 0.0 {
        return None;
    }
    let direccion = extremo.restar(previo).escalar(1.0 / distancia);

    // La punta se encoge en flechas cortas: una punta de 25 px en un trazo de
    // 20 es toda la flecha y se ve como un borron.
    let fraccion = if matches!(tipo, TipoPunta::Rombo | TipoPunta::RomboHueco) {
        0.25
    } else {
        0.5
    };
    let medida = tipo.tamano().min(distancia * fraccion);
    let atras = extremo.restar(direccion.escalar(medida));

    if matches!(tipo, TipoPunta::Circulo | TipoPunta::CirculoHueco) {
        // El circulo no tiene alas: se centra en la punta y lo unico que hace
        // falta es lo gordo que es. El `-2` es del original: compensa que el
        // trazo ya ocupa su propio ancho.
        return Some(FormaDePunta {
            tipo,
            punta: extremo,
            alas: (extremo, extremo),
            opuesto: None,
            diametro: atras.distancia(extremo) + grosor - 2.0,
        });
    }

    let apertura = tipo.apertura().to_radians();
    let alas = (
        atras.girar(extremo, -apertura),
        atras.girar(extremo, apertura),
    );
    let opuesto = matches!(tipo, TipoPunta::Rombo | TipoPunta::RomboHueco).then(|| {
        let dir = (extremo.y - previo.y).atan2(extremo.x - previo.x);
        Punto2::nuevo(extremo.x - medida * 2.0, extremo.y).girar(extremo, dir)
    });
    Some(FormaDePunta {
        tipo,
        punta: extremo,
        alas,
        opuesto,
        diametro: 0.0,
    })
}

/// El contorno CERRADO de una punta, para rellenarla o para recorrerlo.
///
/// La flecha y la barra no cierran nada —son dos rayas y un travesano— y
/// devuelven la lista abierta que hay que trazar; el triangulo y el rombo
/// devuelven su poligono, y el circulo, su borde muestreado. Quien pinta mira
/// [`TipoPunta::es_maciza`] para decidir si lo rellena o solo lo traza.
pub fn contorno_de_punta(f: &FormaDePunta) -> Vec<Punto2> {
    match f.tipo {
        TipoPunta::Ninguna => Vec::new(),
        // Las dos rayas de siempre: del ala a la punta y de la punta a la
        // otra ala. Abierta a proposito: cerrarla la convertiria en un
        // triangulo, que es otra punta distinta.
        TipoPunta::Flecha => vec![f.alas.0, f.punta, f.alas.1],
        // El travesano: de ala a ala, pasando de largo de la punta.
        TipoPunta::Barra => vec![f.alas.0, f.alas.1],
        TipoPunta::Triangulo | TipoPunta::TrianguloHueco => {
            vec![f.alas.0, f.punta, f.alas.1, f.alas.0]
        }
        TipoPunta::Rombo | TipoPunta::RomboHueco => {
            let opuesto = f.opuesto.unwrap_or(f.punta);
            vec![f.alas.0, f.punta, f.alas.1, opuesto, f.alas.0]
        }
        TipoPunta::Circulo | TipoPunta::CirculoHueco => {
            const LADOS: usize = 24;
            let r = (f.diametro / 2.0).max(0.5);
            (0..=LADOS)
                .map(|i| {
                    let t = std::f32::consts::TAU * i as f32 / LADOS as f32;
                    Punto2::nuevo(f.punta.x + r * t.cos(), f.punta.y + r * t.sin())
                })
                .collect()
        }
    }
}

/// Un rombo con las puntas redondeadas.
///
/// **Faltaba entero.** El estilo de fabrica trae `roundness`, asi que el rombo
/// pide puntas redondeadas desde el primer dia y aqui se dibuja en pico igual
/// que los demas; en un rombo la diferencia canta, porque sus cuatro vertices
/// son angulos agudos.
///
/// La clave esta en **como recorta**: no avanza por la arista una distancia,
/// sino que se desplaza `vr` en horizontal y `hr` en vertical, con **un radio
/// distinto por eje** —el de la media anchura y el de la media altura—. Por
/// eso un rombo aplastado no se redondea igual arriba que a los lados, que es
/// como tiene que ser: con un radio unico las puntas laterales quedarian romas
/// y las de arriba en pico.
pub fn rombo_redondo(x: f32, y: f32, ancho: f32, alto: f32) -> Vec<Punto2> {
    let v = vertices_de_rombo(x, y, ancho, alto);
    let (arriba, derecha, abajo, izquierda) = (v[0], v[1], v[2], v[3]);
    let (media_ancho, media_alto) = (ancho / 2.0, alto / 2.0);
    if media_ancho <= 0.0 || media_alto <= 0.0 {
        return v.to_vec();
    }
    let vr = radio_de_esquina(media_ancho);
    let hr = radio_de_esquina(media_alto);
    if vr <= 0.0 && hr <= 0.0 {
        return v.to_vec();
    }

    // Seis tramos por punta, el mismo criterio que el rectangulo redondeado y
    // que el codo: es un cuarto de vuelta corto.
    const TRAMOS_PUNTA: usize = 6;
    let mut salida: Vec<Punto2> = Vec::with_capacity(4 * (TRAMOS_PUNTA + 2) + 1);
    // La costura queda justo despues de la punta de arriba y sobre el tramo
    // recto que baja a la derecha, como en el original: partirla en mitad de
    // una curva dejaria el empalme a la vista.
    salida.push(Punto2::nuevo(arriba.x + vr, arriba.y + hr));
    for (vertice, entrada, salida_p) in [
        (
            derecha,
            Punto2::nuevo(derecha.x - vr, derecha.y - hr),
            Punto2::nuevo(derecha.x - vr, derecha.y + hr),
        ),
        (
            abajo,
            Punto2::nuevo(abajo.x + vr, abajo.y - hr),
            Punto2::nuevo(abajo.x - vr, abajo.y - hr),
        ),
        (
            izquierda,
            Punto2::nuevo(izquierda.x + vr, izquierda.y + hr),
            Punto2::nuevo(izquierda.x + vr, izquierda.y - hr),
        ),
        (
            arriba,
            Punto2::nuevo(arriba.x - vr, arriba.y + hr),
            Punto2::nuevo(arriba.x + vr, arriba.y + hr),
        ),
    ] {
        salida.push(entrada);
        // Cuadratica con el vertice de tirador: la misma que redondea el codo.
        for s in 1..=TRAMOS_PUNTA {
            let t = s as f32 / TRAMOS_PUNTA as f32;
            let u = 1.0 - t;
            salida.push(Punto2::nuevo(
                u * u * entrada.x + 2.0 * u * t * vertice.x + t * t * salida_p.x,
                u * u * entrada.y + 2.0 * u * t * vertice.y + t * t * salida_p.y,
            ));
        }
    }
    salida
}

/// El radio de una esquina redondeada para un lado de `corto` (el
/// `getCornerRadius` del movil con `ADAPTIVE_RADIUS`, que es el que escribe
/// Excalidraw hoy: proporcional en formas pequenas y fijo a partir de cierto
/// tamano, o un rectangulo enorme quedaria con unas curvas desmedidas).
pub fn radio_de_esquina(corto: f32) -> f32 {
    const PROPORCIONAL: f32 = 0.25;
    const FIJO: f32 = 32.0;
    let corte = FIJO / PROPORCIONAL;
    if corto <= corte {
        (corto * PROPORCIONAL).max(0.0)
    } else {
        FIJO
    }
}

/// Un rectangulo de esquinas redondeadas, en una sola pasada.
///
/// El `roundness` de Excalidraw con radio proporcional, como el movil: un
/// cuarto del lado menor, con tope. Va aparte de `rectangulo` porque este no
/// tiembla: los recuadros redondeados que manda el movil son las «zonas» que
/// enlazan con otra hoja, y ahi la forma tiene que coincidir exactamente o
/// parece otro recuadro dibujado encima del suyo.
pub fn rectangulo_redondo(x: f32, y: f32, ancho: f32, alto: f32) -> Vec<Punto2> {
    let radio = (ancho.abs().min(alto.abs()) * 0.25).clamp(0.0, 32.0);
    if radio <= 0.5 {
        return vec![
            Punto2::nuevo(x, y),
            Punto2::nuevo(x + ancho, y),
            Punto2::nuevo(x + ancho, y + alto),
            Punto2::nuevo(x, y + alto),
            Punto2::nuevo(x, y),
        ];
    }
    // Cuatro esquinas, cada una un cuarto de vuelta. Ocho tramos por esquina
    // bastan: a los zooms de trabajo no se distingue de una curva de verdad.
    const TRAMOS_ESQUINA: usize = 8;
    let (x1, y1) = (x + ancho, y + alto);
    let cuarto = std::f32::consts::FRAC_PI_2;
    let esquinas = [
        (x1 - radio, y + radio, -cuarto, 0.0),
        (x1 - radio, y1 - radio, 0.0, cuarto),
        (x + radio, y1 - radio, cuarto, cuarto * 2.0),
        (x + radio, y + radio, cuarto * 2.0, cuarto * 3.0),
    ];
    let mut salida = Vec::with_capacity(4 * (TRAMOS_ESQUINA + 1) + 1);
    for (cx, cy, desde, hasta) in esquinas {
        for i in 0..=TRAMOS_ESQUINA {
            let t = i as f32 / TRAMOS_ESQUINA as f32;
            let a = desde + (hasta - desde) * t;
            salida.push(Punto2::nuevo(cx + radio * a.cos(), cy + radio * a.sin()));
        }
    }
    if let Some(primero) = salida.first().copied() {
        salida.push(primero);
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn caja(trazos: &[Vec<Punto2>]) -> (f32, f32, f32, f32) {
        let todos: Vec<Punto2> = trazos.iter().flatten().copied().collect();
        (
            todos.iter().map(|p| p.x).fold(f32::MAX, f32::min),
            todos.iter().map(|p| p.y).fold(f32::MAX, f32::min),
            todos.iter().map(|p| p.x).fold(f32::MIN, f32::max),
            todos.iter().map(|p| p.y).fold(f32::MIN, f32::max),
        )
    }

    #[test]
    fn sin_rugosidad_la_linea_es_exactamente_recta() {
        // El modo regla: dos puntos, los pedidos, sin un pixel de desvio.
        let mut a = Azar::nuevo(1);
        let t = linea(
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(100.0, 0.0),
            0.0,
            &mut a,
        );
        assert_eq!(t.len(), 1, "sin rugosidad basta una pasada");
        assert_eq!(
            t[0],
            vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)]
        );
    }

    #[test]
    fn con_rugosidad_hay_dos_pasadas_y_ninguna_es_recta() {
        let mut a = Azar::nuevo(42);
        let t = linea(
            Punto2::nuevo(0.0, 50.0),
            Punto2::nuevo(200.0, 50.0),
            1.0,
            &mut a,
        );
        assert_eq!(t.len(), 2, "una mano repasa la linea");
        for pasada in &t {
            let torcida = pasada.iter().any(|p| (p.y - 50.0).abs() > 0.3);
            assert!(torcida, "la pasada salio recta: no parece dibujada");
        }
        // Y las dos pasadas son DISTINTAS: si fueran iguales se veria una
        // sola linea y el efecto se pierde.
        assert_ne!(t[0], t[1]);
    }

    #[test]
    fn la_misma_semilla_dibuja_exactamente_la_misma_forma() {
        // La puerta de la fase: reabrir el documento no puede cambiar nada.
        let hacer = || {
            let mut a = Azar::nuevo(777);
            rectangulo(10.0, 20.0, 100.0, 60.0, 1.0, &mut a)
        };
        assert_eq!(hacer(), hacer());
    }

    #[test]
    fn semillas_distintas_dibujan_formas_distintas() {
        // Caso negativo: si la semilla no influyera, todos los rectangulos
        // del documento saldrian calcados y el efecto "a mano" desapareceria.
        let hacer = |s| {
            let mut a = Azar::nuevo(s);
            rectangulo(10.0, 20.0, 100.0, 60.0, 1.0, &mut a)
        };
        assert_ne!(hacer(777), hacer(778));
    }

    #[test]
    fn el_rectangulo_no_se_aleja_de_su_caja() {
        // Puede temblar, pero no puede irse: si el desvio creciera sin tope,
        // un rectangulo grande se saldria del elemento y del pin.
        let mut a = Azar::nuevo(5);
        let t = rectangulo(100.0, 100.0, 400.0, 300.0, 2.0, &mut a);
        let (x0, y0, x1, y1) = caja(&t);
        assert!(
            x0 > 100.0 - 20.0 && y0 > 100.0 - 20.0,
            "se sale por arriba: {x0},{y0}"
        );
        assert!(
            x1 < 500.0 + 20.0 && y1 < 400.0 + 20.0,
            "se sale por abajo: {x1},{y1}"
        );
    }

    #[test]
    fn la_elipse_cubre_su_caja_por_los_cuatro_lados() {
        let mut a = Azar::nuevo(9);
        let t = elipse(0.0, 0.0, 200.0, 100.0, 1.0, &mut a);
        let (x0, y0, x1, y1) = caja(&t);
        assert!(x0 < 10.0 && x1 > 190.0, "no llega a los lados: {x0}..{x1}");
        assert!(
            y0 < 10.0 && y1 > 90.0,
            "no llega arriba y abajo: {y0}..{y1}"
        );
    }

    #[test]
    fn la_punta_de_la_flecha_esta_en_el_extremo_y_apunta_hacia_atras() {
        let mut a = Azar::nuevo(11);
        let desde = Punto2::nuevo(0.0, 0.0);
        let hasta = Punto2::nuevo(100.0, 0.0);
        let t = punta_flecha(desde, hasta, 0.0, &mut a);
        assert_eq!(t.len(), 2, "la punta son dos lineas");
        for pasada in &t {
            // Cada linea empieza en la punta...
            assert!(pasada[0].distancia(hasta) < 0.001);
            // ...y termina hacia atras, nunca mas alla del extremo.
            assert!(
                pasada.last().unwrap().x < hasta.x,
                "la punta apunta al reves"
            );
        }
    }

    /// Una flecha recta de cien pixeles hacia la derecha.
    fn recta() -> Vec<Punto2> {
        vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)]
    }

    #[test]
    fn las_ocho_puntas_llevan_las_palabras_del_fichero_una_por_una() {
        // Si una sola deja de coincidir, un diagrama entidad-relacion del
        // movil se abre aqui con otra cardinalidad: la punta DICE algo.
        let esperadas = [
            "arrow",
            "bar",
            "circle",
            "circle_outline",
            "triangle",
            "triangle_outline",
            "diamond",
            "diamond_outline",
        ];
        let nuestras: Vec<&str> = PUNTAS.iter().filter_map(|p| p.palabra()).collect();
        assert_eq!(nuestras, esperadas);
        for p in PUNTAS {
            assert_eq!(TipoPunta::desde_palabra(p.palabra().unwrap()), Some(p));
        }
    }

    #[test]
    fn sin_punta_no_hay_palabra_y_una_palabra_rara_no_se_lee_como_sin_punta() {
        // Caso negativo, y el que protege el fichero: leer una punta de una
        // version futura como «ninguna» la borraria al guardar.
        assert_eq!(TipoPunta::Ninguna.palabra(), None);
        assert_eq!(TipoPunta::desde_palabra("crowfoot-del-futuro"), None);
        assert_eq!(TipoPunta::desde_palabra(""), None);
    }

    #[test]
    fn solo_las_tres_macizas_se_rellenan() {
        // Es lo que distingue en un diagrama la herencia de la composicion:
        // el mismo triangulo, uno relleno y otro hueco.
        for p in [TipoPunta::Circulo, TipoPunta::Triangulo, TipoPunta::Rombo] {
            assert!(p.es_maciza(), "{p:?}");
        }
        for p in [
            TipoPunta::Flecha,
            TipoPunta::Barra,
            TipoPunta::CirculoHueco,
            TipoPunta::TrianguloHueco,
            TipoPunta::RomboHueco,
        ] {
            assert!(!p.es_maciza(), "{p:?}");
        }
    }

    #[test]
    fn cada_punta_se_posa_en_el_extremo_y_ninguna_se_pasa_de_largo() {
        for tipo in PUNTAS {
            let f = forma_de_punta(&recta(), true, tipo, 2.0).expect("{tipo:?}");
            assert_eq!(f.punta, Punto2::nuevo(100.0, 0.0), "{tipo:?}");
            for p in contorno_de_punta(&f) {
                assert!(p.x <= 100.01 + f.diametro / 2.0, "{tipo:?} se pasa: {p:?}");
            }
        }
    }

    #[test]
    fn la_punta_del_principio_apunta_al_reves_que_la_del_final() {
        // Caso negativo del extremo: si las dos miraran al mismo lado, una
        // flecha de doble punta saldria con las dos en el mismo sentido.
        let fin = forma_de_punta(&recta(), true, TipoPunta::Flecha, 2.0).unwrap();
        let ini = forma_de_punta(&recta(), false, TipoPunta::Flecha, 2.0).unwrap();
        assert!(fin.alas.0.x < fin.punta.x, "la del final no mira atras");
        assert!(ini.alas.0.x > ini.punta.x, "la del principio no mira atras");
    }

    #[test]
    fn la_barra_sale_perpendicular_al_trazo_y_la_flecha_no() {
        // Los noventa grados de apertura: es lo unico que separa una barra de
        // una punta abierta.
        let barra = forma_de_punta(&recta(), true, TipoPunta::Barra, 2.0).unwrap();
        assert!(
            (barra.alas.0.x - barra.punta.x).abs() < 0.01
                && (barra.alas.1.x - barra.punta.x).abs() < 0.01,
            "la barra no es perpendicular: {barra:?}"
        );
        let flecha = forma_de_punta(&recta(), true, TipoPunta::Flecha, 2.0).unwrap();
        assert!((flecha.alas.0.x - flecha.punta.x).abs() > 1.0);
    }

    #[test]
    fn el_rombo_lleva_su_cuarto_vertice_y_las_demas_puntas_no() {
        let rombo = forma_de_punta(&recta(), true, TipoPunta::Rombo, 2.0).unwrap();
        let opuesto = rombo.opuesto.expect("el rombo necesita cuatro vertices");
        assert!(opuesto.x < rombo.punta.x, "el cuarto vertice va detras");
        assert!(
            forma_de_punta(&recta(), true, TipoPunta::Flecha, 2.0)
                .unwrap()
                .opuesto
                .is_none()
        );
        assert_eq!(contorno_de_punta(&rombo).len(), 5, "rombo cerrado");
    }

    #[test]
    fn en_un_trazo_a_pulso_la_punta_mira_hacia_donde_iba_la_mano() {
        // El fallo que esto viene a impedir: con los puntos a un pixel unos de
        // otros, mirando solo al anterior la punta salia del tamano de ese
        // pixel —invisible— y temblorosa. Es lo que hace usable la flecha a
        // mano alzada, que por dentro es una flecha con todos sus puntos.
        let mut pulso: Vec<Punto2> = (0..200)
            .map(|i| Punto2::nuevo(i as f32 * 0.5, (i as f32 * 0.3).sin()))
            .collect();
        pulso.push(Punto2::nuevo(100.0, 0.0));
        let f = forma_de_punta(&pulso, true, TipoPunta::Flecha, 2.0).unwrap();
        let ala = f.punta.distancia(f.alas.0);
        assert!(ala > 5.0, "la punta salio del tamano de un pixel: {ala}");
    }

    #[test]
    fn una_flecha_corta_no_se_queda_en_pura_punta() {
        // El acotado a la mitad del trazo: una punta de 25 px en una flecha de
        // 20 es toda la flecha y se ve como un borron.
        let corta = vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(20.0, 0.0)];
        let f = forma_de_punta(&corta, true, TipoPunta::Flecha, 2.0).unwrap();
        assert!(f.punta.distancia(f.alas.0) <= 12.0, "{f:?}");
    }

    #[test]
    fn sin_punta_o_sin_dos_puntos_no_hay_nada_que_dibujar() {
        assert!(forma_de_punta(&recta(), true, TipoPunta::Ninguna, 2.0).is_none());
        assert!(forma_de_punta(&[Punto2::nuevo(0.0, 0.0)], true, TipoPunta::Flecha, 2.0).is_none());
        // Caso negativo del cero: los dos extremos encima no tienen direccion,
        // y normalizarla daria NaN, que borra la geometria entera.
        let pegados = vec![Punto2::nuevo(5.0, 5.0), Punto2::nuevo(5.0, 5.0)];
        assert!(forma_de_punta(&pegados, true, TipoPunta::Flecha, 2.0).is_none());
    }

    #[test]
    fn el_rombo_redondo_no_se_sale_de_su_caja_y_ya_no_tiene_picos() {
        let r = rombo_redondo(0.0, 0.0, 200.0, 100.0);
        assert!(r.len() > 4, "no se redondeo nada");
        for p in &r {
            assert!((-0.01..=200.01).contains(&p.x), "{p:?}");
            assert!((-0.01..=100.01).contains(&p.y), "{p:?}");
        }
        // Y los cuatro vertices en pico ya no estan: si siguieran, seria el
        // mismo rombo de antes con mas puntos.
        for v in vertices_de_rombo(0.0, 0.0, 200.0, 100.0) {
            assert!(
                !r.iter().any(|p| p.distancia(v) < 1e-4),
                "la punta {v:?} sigue en pico"
            );
        }
    }

    #[test]
    fn un_rombo_aplastado_se_redondea_distinto_por_cada_eje() {
        // Con un radio unico las puntas laterales quedarian romas y las de
        // arriba en pico. Con 400x40 el radio horizontal se topa en el fijo y
        // el vertical se queda proporcional.
        assert_eq!(radio_de_esquina(200.0), 32.0, "el radio se topa");
        assert_eq!(radio_de_esquina(20.0), 5.0, "y por debajo es proporcional");
        let r = rombo_redondo(0.0, 0.0, 400.0, 40.0);
        for p in &r {
            assert!((-0.01..=400.01).contains(&p.x) && (-0.01..=40.01).contains(&p.y));
        }
    }

    #[test]
    fn un_rombo_sin_tamano_devuelve_sus_cuatro_vertices_y_no_entra_en_panico() {
        let r = rombo_redondo(10.0, 10.0, 0.0, 0.0);
        assert_eq!(r.len(), 4);
        for p in &r {
            assert!(p.x.is_finite() && p.y.is_finite());
        }
    }

    #[test]
    fn una_forma_sin_tamano_no_entra_en_panico() {
        let mut a = Azar::nuevo(1);
        let _ = rectangulo(10.0, 10.0, 0.0, 0.0, 1.0, &mut a);
        let e = elipse(10.0, 10.0, 0.0, 0.0, 1.0, &mut a);
        for p in e.iter().flatten() {
            assert!(p.x.is_finite() && p.y.is_finite());
        }
    }
}
