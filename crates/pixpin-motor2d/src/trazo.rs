//! El trazo a mano alzada: de puntos crudos del raton a un poligono de tinta.
//!
//! Un trazo NO es una polilinea con grosor. Si se dibuja asi, se nota al
//! instante: grosor constante, extremos cuadrados, esquinas que se cruzan. Lo
//! que hace que parezca tinta es que el grosor varie con la velocidad y que el
//! contorno se cierre con tapas redondas.
//!
//! El proceso, en tres pasos:
//!
//! 1. **Suavizar** (`linea_central`): cada punto crudo se mezcla con el
//!    anterior, asi el temblor del raton no llega al papel. El ultimo punto NO
//!    se suaviza: el trazo tiene que terminar donde el usuario solto.
//! 2. **Engordar** (`contorno`): a cada lado de la linea central se proyecta
//!    un punto a la distancia del radio, que depende de la presion; en las
//!    esquinas cerradas se dibuja un arco en vez de proyectar, porque si no
//!    los dos lados se cruzan y aparece un pico.
//! 3. **Cerrar**: tapa redonda al final, el lado derecho al reves, tapa al
//!    principio. Sale un anillo cerrado listo para rellenar.
//!
//! Las constantes (0,275 para el cambio de presion, 13 pasos por arco, el
//! factor 0,85 del suavizado) vienen del algoritmo estudiado y NO son
//! arbitrarias: cambiarlas cambia el caracter del trazo.

use crate::vector::Punto2;

/// A que ritmo puede cambiar la presion entre dos puntos. Mas alto seria un
/// trazo nervioso, mas bajo uno de grosor casi constante.
const CAMBIO_DE_PRESION: f32 = 0.275;
/// Pasos con los que se traza un arco (esquinas y tapas).
const PASOS_ARCO: usize = 13;

/// Un punto de la linea central, con lo que hace falta para engordarlo.
#[derive(Debug, Clone, Copy)]
pub struct PuntoTrazo {
    pub punto: Punto2,
    pub presion: f32,
    /// Unitario, apuntando HACIA el punto anterior.
    pub vector: Punto2,
    pub distancia: f32,
    pub recorrido: f32,
}

/// Ajustes del trazo. `tamano` es el grosor maximo en pixeles.
#[derive(Debug, Clone, Copy)]
pub struct Ajustes {
    pub tamano: f32,
    /// Cuanto adelgaza el trazo con poca presion (0 = grosor constante).
    pub adelgazado: f32,
    /// Cuanto se separan los puntos del contorno (evita amontonarlos).
    pub suavidad: f32,
    /// Cuanto se suaviza el temblor del raton (0 = crudo, 1 = muy suave).
    pub fluidez: f32,
    /// Si el trazo ya termino: el ultimo punto se respeta tal cual.
    pub terminado: bool,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            tamano: 8.0,
            adelgazado: 0.6,
            suavidad: 0.5,
            fluidez: 0.5,
            terminado: true,
        }
    }
}

/// Paso 1: la linea central suavizada.
pub fn linea_central(crudos: &[Punto2], a: &Ajustes) -> Vec<PuntoTrazo> {
    if crudos.is_empty() {
        return Vec::new();
    }

    // Un solo punto no tiene direccion: se duplica desplazado para que exista
    // un segmento y las tapas puedan formar un circulo.
    let mut puntos: Vec<Punto2> = crudos.to_vec();
    if puntos.len() == 1 {
        puntos.push(puntos[0].sumar(Punto2::nuevo(1.0, 1.0)));
    }

    let t = 0.15 + (1.0 - a.fluidez) * 0.85;
    let mut salida: Vec<PuntoTrazo> = Vec::with_capacity(puntos.len());
    let mut recorrido = 0.0;
    let mut anterior = puntos[0];

    salida.push(PuntoTrazo {
        punto: anterior,
        presion: 0.25,
        vector: Punto2::nuevo(1.0, 1.0),
        distancia: 0.0,
        recorrido: 0.0,
    });

    for (i, crudo) in puntos.iter().enumerate().skip(1) {
        // El ultimo punto se respeta tal cual: si se suavizara, el trazo
        // terminaria un poco antes de donde el usuario levanto el raton.
        let punto = if a.terminado && i == puntos.len() - 1 {
            *crudo
        } else {
            anterior.hacia(*crudo, t)
        };

        // Puntos repetidos no aportan nada y estropean los vectores.
        if punto.distancia(anterior) < f32::EPSILON {
            continue;
        }

        let distancia = punto.distancia(anterior);
        recorrido += distancia;
        salida.push(PuntoTrazo {
            punto,
            presion: 0.5,
            vector: anterior.restar(punto).unitario(),
            distancia,
            recorrido,
        });
        anterior = punto;
    }

    // El primer punto no tenia direccion propia: toma la del segundo.
    if salida.len() > 1 {
        salida[0].vector = salida[1].vector;
    }
    salida
}

/// El radio del trazo en un punto, segun su presion.
fn radio(tamano: f32, adelgazado: f32, presion: f32) -> f32 {
    if adelgazado == 0.0 {
        return tamano / 2.0;
    }
    // El seno suaviza los extremos: con esta curva, la diferencia entre
    // presion 0,9 y 1,0 casi no se nota, como pasa con una pluma de verdad.
    let p = (0.5 - adelgazado * (0.5 - presion)).clamp(0.0, 1.0);
    tamano * (p * std::f32::consts::FRAC_PI_2).sin()
}

/// Paso 2 y 3: el contorno cerrado, listo para rellenar.
pub fn contorno(centro: &[PuntoTrazo], a: &Ajustes) -> Vec<Punto2> {
    if centro.is_empty() {
        return Vec::new();
    }

    let total = centro.last().map(|p| p.recorrido).unwrap_or(0.0);
    let distancia_min = (a.tamano * a.suavidad).powi(2);

    // Presion inicial simulada: se acumula sobre los primeros puntos para que
    // el trazo no empiece de golpe con todo su grosor.
    let mut presion = 0.25f32;
    let mut izquierda: Vec<Punto2> = Vec::new();
    let mut derecha: Vec<Punto2> = Vec::new();

    for (i, p) in centro.iter().enumerate() {
        // La presion sale de la VELOCIDAD: cuanto mas deprisa va el raton,
        // menos presiona una mano de verdad, y mas fino sale el trazo.
        let velocidad = (p.distancia / a.tamano).min(1.0);
        let ritmo = (1.0 - velocidad).min(1.0);
        presion = (presion + (ritmo - presion) * (velocidad * CAMBIO_DE_PRESION)).clamp(0.0, 1.0);
        let radio_actual = radio(a.tamano, a.adelgazado, presion).max(0.01);

        // Afilado en los extremos: el trazo nace y muere en punta, como al
        // apoyar y levantar la pluma.
        let afilado_inicio = (p.recorrido / a.tamano).min(1.0);
        let restante = (total - p.recorrido) / a.tamano;
        let afilado_fin = restante.min(1.0);
        let r = radio_actual * afilado_inicio.min(afilado_fin).max(0.01);

        let siguiente = centro.get(i + 1);
        let vector_siguiente = siguiente.map(|s| s.vector).unwrap_or(p.vector);
        let giro = p.vector.producto(vector_siguiente);

        if giro < 0.0 && siguiente.is_some() {
            // Esquina cerrada: proyectar a los lados cruzaria los dos bordes
            // y saldria un pico. Se rodea el punto con un arco.
            let offset = p.vector.perpendicular().escalar(r);
            for paso in 0..=PASOS_ARCO {
                let t = paso as f32 / PASOS_ARCO as f32;
                let angulo = std::f32::consts::PI * t;
                izquierda.push(p.punto.sumar(offset).girar(p.punto, angulo));
                derecha.push(p.punto.restar(offset).girar(p.punto, -angulo));
            }
            continue;
        }

        // Punto normal: perpendicular a la direccion media entre este tramo
        // y el siguiente, para que las curvas no se acodillen.
        let direccion = p.vector.hacia(vector_siguiente, 0.5).unitario();
        let offset = direccion.perpendicular().escalar(r);
        let izq = p.punto.sumar(offset);
        let der = p.punto.restar(offset);

        // Sin este filtro, un raton parado genera miles de puntos en el mismo
        // sitio y la geometria se vuelve lentisima sin verse distinta.
        if izquierda
            .last()
            .is_none_or(|u| u.restar(izq).producto(u.restar(izq)) > distancia_min)
        {
            izquierda.push(izq);
        }
        if derecha
            .last()
            .is_none_or(|u| u.restar(der).producto(u.restar(der)) > distancia_min)
        {
            derecha.push(der);
        }
    }

    // Un trazo de un solo punto es un circulo, no un poligono vacio: el caso
    // que rompe toda implementacion ingenua (un clic sin arrastrar).
    if izquierda.len() < 2 && derecha.len() < 2 {
        let p = centro[0].punto;
        let r = radio(a.tamano, a.adelgazado, 0.5).max(1.0);
        return (0..PASOS_ARCO * 2)
            .map(|i| {
                let ang = std::f32::consts::TAU * i as f32 / (PASOS_ARCO * 2) as f32;
                Punto2::nuevo(p.x + r * ang.cos(), p.y + r * ang.sin())
            })
            .collect();
    }

    // Cerrar: lado izquierdo, tapa del final, lado derecho al reves, y la
    // tapa del principio la cierra sola al unirse con el primer punto.
    let ultimo = centro[centro.len() - 1].punto;
    let mut anillo = izquierda;
    if let Some(der_ultimo) = derecha.last() {
        let radio_tapa = ultimo.distancia(*der_ultimo).max(0.01);
        let dir = der_ultimo.restar(ultimo).unitario();
        for paso in 1..PASOS_ARCO {
            let t = paso as f32 / PASOS_ARCO as f32;
            anillo.push(
                ultimo
                    .proyectar(dir, radio_tapa)
                    .girar(ultimo, std::f32::consts::PI * (1.0 - t)),
            );
        }
    }
    anillo.extend(derecha.iter().rev().copied());
    anillo
}

/// Todo el proceso de una vez: puntos crudos a poligono de tinta.
pub fn poligono(crudos: &[Punto2], a: &Ajustes) -> Vec<Punto2> {
    contorno(&linea_central(crudos, a), a)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn linea_recta(n: usize) -> Vec<Punto2> {
        (0..n)
            .map(|i| Punto2::nuevo(i as f32 * 5.0, 50.0))
            .collect()
    }

    #[test]
    fn un_solo_punto_produce_un_circulo_y_no_un_poligono_vacio() {
        // Un clic sin arrastrar. La implementacion ingenua devuelve una lista
        // vacia y el usuario ve que "no pinta".
        let p = poligono(&[Punto2::nuevo(10.0, 10.0)], &Ajustes::default());
        assert!(
            p.len() >= 8,
            "deberia ser un circulo, tiene {} puntos",
            p.len()
        );

        // Y es un circulo de verdad: todos los puntos a la misma distancia
        // del centro, dentro de un pelo.
        let centro = Punto2::nuevo(10.0, 10.0);
        let radios: Vec<f32> = p.iter().map(|q| q.distancia(centro)).collect();
        let max = radios.iter().cloned().fold(0.0f32, f32::max);
        let min = radios.iter().cloned().fold(f32::MAX, f32::min);
        assert!(max - min < 0.5, "no es redondo: radios entre {min} y {max}");
    }

    #[test]
    fn una_lista_vacia_no_entra_en_panico() {
        assert!(poligono(&[], &Ajustes::default()).is_empty());
    }

    #[test]
    fn el_trazo_termina_donde_el_usuario_solto() {
        // Si el ultimo punto se suavizara, el trazo se quedaria corto y la
        // punta de una flecha dibujada a mano no llegaria a su sitio.
        let crudos = linea_recta(20);
        let centro = linea_central(&crudos, &Ajustes::default());
        let ultimo = centro.last().unwrap().punto;
        let esperado = *crudos.last().unwrap();
        assert!(
            ultimo.distancia(esperado) < 0.001,
            "el trazo termina en {ultimo:?} y deberia terminar en {esperado:?}"
        );
    }

    #[test]
    fn el_suavizado_recorta_el_temblor() {
        // Una linea con un pico de un pixel arriba y abajo en cada punto: la
        // linea central debe quedar mas plana que la entrada.
        let temblorosa: Vec<Punto2> = (0..40)
            .map(|i| {
                let y = if i % 2 == 0 { 48.0 } else { 52.0 };
                Punto2::nuevo(i as f32 * 4.0, y)
            })
            .collect();
        let centro = linea_central(&temblorosa, &Ajustes::default());
        let variacion: f32 = centro
            .windows(2)
            .map(|w| (w[1].punto.y - w[0].punto.y).abs())
            .sum();
        let variacion_cruda: f32 = temblorosa.windows(2).map(|w| (w[1].y - w[0].y).abs()).sum();
        assert!(
            variacion < variacion_cruda * 0.8,
            "el suavizado no recorto nada: {variacion} frente a {variacion_cruda}"
        );
    }

    #[test]
    fn el_contorno_envuelve_la_linea_central() {
        // El poligono tiene que rodear el trazo: su caja debe contener todos
        // los puntos de entrada, no quedarse a un lado.
        let crudos = linea_recta(30);
        let p = poligono(&crudos, &Ajustes::default());
        let min_x = p.iter().map(|q| q.x).fold(f32::MAX, f32::min);
        let max_x = p.iter().map(|q| q.x).fold(f32::MIN, f32::max);
        let min_y = p.iter().map(|q| q.y).fold(f32::MAX, f32::min);
        let max_y = p.iter().map(|q| q.y).fold(f32::MIN, f32::max);
        assert!(
            min_x <= 1.0 && max_x >= 140.0,
            "no cubre el largo: {min_x}..{max_x}"
        );
        assert!(
            min_y < 50.0 && max_y > 50.0,
            "no tiene grosor: {min_y}..{max_y}"
        );
    }

    #[test]
    fn sin_adelgazado_el_grosor_es_constante() {
        let a = Ajustes {
            adelgazado: 0.0,
            ..Default::default()
        };
        assert_eq!(radio(10.0, a.adelgazado, 0.1), 5.0);
        assert_eq!(radio(10.0, a.adelgazado, 0.9), 5.0);
        // Caso negativo: CON adelgazado si cambia.
        assert_ne!(radio(10.0, 0.6, 0.1), radio(10.0, 0.6, 0.9));
    }

    #[test]
    fn el_poligono_no_tiene_ningun_valor_no_finito() {
        // Un solo NaN borra el elemento entero de la pantalla sin dar error.
        // Entradas hostiles: puntos repetidos, saltos enormes, retrocesos.
        let hostiles = vec![
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(1e6, 1e6),
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(-1e6, 5.0),
            Punto2::nuevo(-1e6, 5.0),
        ];
        for q in poligono(&hostiles, &Ajustes::default()) {
            assert!(q.x.is_finite() && q.y.is_finite(), "punto no finito: {q:?}");
        }
    }

    #[test]
    fn el_filtro_de_cercania_evita_amontonar_puntos() {
        // Un raton parado manda cientos de eventos en el mismo pixel; sin
        // filtro, el poligono creceria sin limite y el dibujo se atascaria.
        let parado: Vec<Punto2> = (0..500).map(|_| Punto2::nuevo(10.0, 10.0)).collect();
        let p = poligono(&parado, &Ajustes::default());
        assert!(p.len() < 100, "el poligono tiene {} puntos", p.len());
    }
}
