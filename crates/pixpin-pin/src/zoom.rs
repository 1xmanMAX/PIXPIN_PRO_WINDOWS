//! Zoom del pin: escala anclada al cursor y persecucion suave del destino.
//!
//! El zoom de la rueda ampliaba desde el centro y con una animacion de
//! duracion fija. Eso da dos problemas que se notan mucho: el detalle que el
//! usuario esta mirando se le escapa de debajo del cursor —porque el centro
//! del pin casi nunca es donde apunta—, y cada muesca nueva reinicia la
//! animacion, asi que encadenar muescas produce saltos en vez de un
//! movimiento continuo.
//!
//! Aqui se copia lo que hacen los visores rapidos: la escala se ancla en el
//! cursor, y el tamano en pantalla persigue al destino con interpolacion
//! exponencial, que por la propiedad `exp(a) * exp(b) = exp(a + b)` da el
//! mismo resultado se reparta el tiempo en los fotogramas que se reparta.
//!
//! Este modulo es solo la matematica: no conoce ventanas ni mensajes. Asi el
//! comportamiento del zoom se prueba entero sin abrir nada, igual que
//! `estado`.

use pixpin_geom::{Punto, Rect};

/// Escala `rect` por `factor` dejando `ancla` clavado donde estaba.
///
/// `ancla` va en las mismas coordenadas que `rect` (pixeles fisicos del
/// escritorio). Lo que crece o encoge es todo lo demas a su alrededor: el
/// pixel que hay bajo el cursor sigue bajo el cursor.
///
/// Los topes `minimo` y `maximo` son de lado, y se aplican limitando el
/// factor, nunca recortando un lado suelto: la proporcion del rect no cambia
/// jamas, ni siquiera al chocar contra un tope.
pub fn escalar_anclado(rect: Rect, factor: f32, ancla: Punto, minimo: u32, maximo: u32) -> Rect {
    // Un factor imposible —un NaN de una division por cero aguas arriba, o un
    // cero que colapsaria el pin hasta hacerlo invisible— se ignora: mas vale
    // no hacer zoom que dejar al usuario sin pin.
    if !factor.is_finite() || factor <= 0.0 {
        return rect;
    }

    // Un rect degenerado se trata como de un pixel para no dividir por cero;
    // el resultado sigue teniendo lado, que es lo unico que importa aqui.
    let ancho = rect.ancho.max(1) as f32;
    let alto = rect.alto.max(1) as f32;

    // Los topes de lado se traducen a topes del FACTOR. Recortar cada lado
    // por su cuenta seria lo facil, pero deforma el pin justo al llegar al
    // tope, que es cuando mas canta.
    let f_minimo = (minimo as f32 / ancho).max(minimo as f32 / alto);
    let f_maximo = (maximo as f32 / ancho).min(maximo as f32 / alto);
    // Si el rect ya viene fuera de rango las dos cotas se cruzan; ordenarlas
    // evita el panico de `clamp` y deja el factor pegado a la cota mas
    // cercana en vez de reventar.
    let factor = factor.clamp(f_minimo.min(f_maximo), f_minimo.max(f_maximo));

    let nuevo_ancho = (ancho * factor).round().max(1.0);
    let nuevo_alto = (alto * factor).round().max(1.0);

    // La posicion se calcula con el factor EFECTIVO, el de los lados ya
    // redondeados, y no con el teorico: con el teorico el ancla se corre una
    // fraccion de pixel por muesca y el desvio se acumula al encadenarlas.
    let fx = nuevo_ancho / ancho;
    let fy = nuevo_alto / alto;

    Rect {
        x: (ancla.x as f32 - (ancla.x as f32 - rect.x as f32) * fx).round() as i32,
        y: (ancla.y as f32 - (ancla.y as f32 - rect.y as f32) * fy).round() as i32,
        ancho: nuevo_ancho as u32,
        alto: nuevo_alto as u32,
    }
}

/// Velocidad de la persecucion, en 1/segundo.
///
/// De donde sale: en un fotograma de 60 Hz (16,7 ms) la fraccion recorrida es
/// `1 - exp(-72 * 0.0167) = 0.70`, o sea que cada fotograma se come el 70% de
/// lo que falta, y un salto de 800 px se planta a menos de medio pixel del
/// destino en algo mas de 0,1 s. Por debajo el zoom se siente perezoso y se
/// arrastra detras de la rueda; por encima vuelve a ser el salto seco de
/// antes, sin sensacion de continuidad.
pub const VELOCIDAD: f32 = 72.0;

/// Pixeles de arrastre vertical que valen lo mismo que un paso de rueda.
///
/// Ocho sale de probar: con menos, el pin da saltos y no se puede afinar;
/// con mas, hay que recorrer media pantalla para doblar el tamano.
pub const PIXELES_POR_PASO: i32 = 8;

/// Cuanto hay que moverse antes de dar el arrastre por empezado (P3.3).
///
/// Sin esto, el pulso de la mano al pulsar el boton derecho contaria como
/// arrastre y el menu del clic derecho no se abriria nunca. Cinco pixeles
/// es mas que el temblor de cualquiera y menos que un gesto a proposito.
pub const UMBRAL_ARRASTRE: i32 = 5;

/// El giro de rueda equivalente a arrastrar `dy` pixeles en vertical.
///
/// Positivo = acercar. Arrastrar HACIA ARRIBA acerca, que es como
/// funciona el zoom por arrastre en todas partes: la imagen sigue a la
/// mano, como si tiraras de ella hacia ti. En pantalla, arriba es `y`
/// menor, de ahi el signo cambiado.
///
/// Aparte y pura porque equivocarse de signo aqui se nota tarde y se
/// arregla a ciegas: el zoom Â«funcionaÂ», solo que al reves.
pub fn pasos_de_arrastre(dy: i32) -> i32 {
    -dy * 120 / PIXELES_POR_PASO
}

/// Cota baja del paso de tiempo (1/240 s). Un equipo muy rapido —o un bucle
/// que llame dos veces seguidas dentro del mismo fotograma— daria pasos de
/// microsegundos: nada de trabajo util y, con el redondeo a pixeles, riesgo
/// de no avanzar nunca.
const DT_MINIMO: f32 = 1.0 / 240.0;

/// Cota alta del paso de tiempo (50 ms). Sin ella, un parpadeo del sistema
/// —una pausa del recolector grafico, el portatil saliendo de suspension— se
/// traduciria en un dt enorme y el zoom saltaria de golpe al destino, que es
/// justo el defecto que este modulo viene a quitar.
const DT_MAXIMO: f32 = 0.050;

/// Distancia a la que se da por llegado. Menos de medio pixel ya no se ve al
/// redondear a enteros, asi que seguir persiguiendo solo gastaria fotogramas.
const MEDIO_PIXEL: f32 = 0.5;

/// Persigue un rectangulo destino con interpolacion exponencial.
///
/// Pedir un destino nuevo a mitad de camino no reinicia nada: el recorrido
/// simplemente se redirige desde donde va. Es lo que hace que encadenar
/// muescas de rueda salga fluido en vez de a tirones.
#[derive(Debug, Clone, Copy)]
pub struct ControlZoom {
    // El estado en curso vive en f32 y no en Rect porque con pasos cortos el
    // avance de un fotograma es de decimas de pixel: redondear a enteros en
    // cada paso se comeria ese avance y la animacion se quedaria clavada.
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    objetivo: Rect,
}

impl ControlZoom {
    /// Arranca parado en `actual`: sin destino pendiente, `terminado` ya es
    /// cierto y `paso` no mueve nada.
    pub fn nuevo(actual: Rect) -> Self {
        ControlZoom {
            x: actual.x as f32,
            y: actual.y as f32,
            ancho: actual.ancho as f32,
            alto: actual.alto as f32,
            objetivo: actual,
        }
    }

    /// Fija a donde va. No toca el estado en curso a proposito: reiniciarlo
    /// es exactamente el salto que queremos evitar.
    pub fn pedir(&mut self, objetivo: Rect) {
        self.objetivo = objetivo;
    }

    pub fn objetivo(&self) -> Rect {
        self.objetivo
    }

    /// El rectangulo de este fotograma, ya redondeado a pixeles enteros.
    pub fn actual(&self) -> Rect {
        Rect {
            x: self.x.round() as i32,
            y: self.y.round() as i32,
            ancho: self.ancho.round().max(0.0) as u32,
            alto: self.alto.round().max(0.0) as u32,
        }
    }

    /// Avanza un fotograma y devuelve el rectangulo resultante.
    pub fn paso(&mut self, dt_segundos: f32) -> Rect {
        if self.terminado() {
            // Clavar tambien aqui: si el destino se movio menos de medio
            // pixel, la persecucion no llegaria a arrancar y quedaria ese
            // resto colgando para siempre.
            self.clavar();
            return self.actual();
        }

        // Un dt que no es un numero solo puede venir de un reloj roto; se
        // trata como el paso mas corto posible en vez de contaminar el estado
        // con NaN, del que ya no se sale.
        let dt = if dt_segundos.is_finite() {
            dt_segundos.clamp(DT_MINIMO, DT_MAXIMO)
        } else {
            DT_MINIMO
        };

        // Lo que queda por recorrer se multiplica por exp(-VELOCIDAD * dt) en
        // cada paso. Como exp(a) * exp(b) = exp(a + b), el resultado depende
        // solo del tiempo TOTAL transcurrido y no de en cuantos fotogramas se
        // haya repartido: el zoom se ve igual a 60 que a 144 Hz.
        let avance = 1.0 - (-VELOCIDAD * dt).exp();
        self.x += (self.objetivo.x as f32 - self.x) * avance;
        self.y += (self.objetivo.y as f32 - self.y) * avance;
        self.ancho += (self.objetivo.ancho as f32 - self.ancho) * avance;
        self.alto += (self.objetivo.alto as f32 - self.alto) * avance;

        if self.terminado() {
            // La exponencial nunca llega del todo; sin clavar quedaria un
            // error de redondeo permanente de hasta medio pixel y el pin no
            // acabaria en el tamano que el usuario pidio.
            self.clavar();
        }
        self.actual()
    }

    /// Cierto cuando cada lado esta a menos de medio pixel del destino.
    pub fn terminado(&self) -> bool {
        (self.x - self.objetivo.x as f32).abs() < MEDIO_PIXEL
            && (self.y - self.objetivo.y as f32).abs() < MEDIO_PIXEL
            && (self.ancho - self.objetivo.ancho as f32).abs() < MEDIO_PIXEL
            && (self.alto - self.objetivo.alto as f32).abs() < MEDIO_PIXEL
    }

    fn clavar(&mut self) {
        self.x = self.objetivo.x as f32;
        self.y = self.objetivo.y as f32;
        self.ancho = self.objetivo.ancho as f32;
        self.alto = self.objetivo.alto as f32;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn arrastrar_hacia_arriba_acerca() {
        // El signo: en pantalla, arriba es `y` menor. Si esto se
        // invirtiera, el zoom Â«funcionariaÂ» pero al reves, y eso se nota
        // tarde y se arregla a ciegas.
        assert!(pasos_de_arrastre(-PIXELES_POR_PASO) > 0, "hacia arriba");
        assert!(pasos_de_arrastre(PIXELES_POR_PASO) < 0, "hacia abajo");
    }

    #[test]
    fn un_paso_completo_vale_una_muesca_de_rueda() {
        assert_eq!(pasos_de_arrastre(-PIXELES_POR_PASO), 120);
        assert_eq!(pasos_de_arrastre(-PIXELES_POR_PASO * 3), 360);
    }

    #[test]
    fn quieto_no_mueve_nada() {
        // Caso negativo: sin movimiento no puede salir un giro, o el pin
        // se pondria a crecer solo mientras el boton esta pulsado.
        assert_eq!(pasos_de_arrastre(0), 0);
    }

    const MINIMO: u32 = 48;
    const MAXIMO: u32 = 20_000;

    fn rect(x: i32, y: i32, ancho: u32, alto: u32) -> Rect {
        Rect { x, y, ancho, alto }
    }

    /// Cuanto se ha movido en pantalla el pixel del pin que estaba bajo el
    /// ancla. Cero seria perfecto; el redondeo a enteros permite hasta 1 px.
    fn desvio_del_ancla(antes: Rect, despues: Rect, ancla: Punto) -> (f32, f32) {
        let u = (ancla.x - antes.x) as f32 / antes.ancho as f32;
        let v = (ancla.y - antes.y) as f32 / antes.alto as f32;
        (
            despues.x as f32 + u * despues.ancho as f32 - ancla.x as f32,
            despues.y as f32 + v * despues.alto as f32 - ancla.y as f32,
        )
    }

    fn proporcion(r: Rect) -> f32 {
        r.ancho as f32 / r.alto as f32
    }

    #[test]
    fn el_punto_anclado_no_se_mueve_al_ampliar() {
        let antes = rect(100, 100, 400, 300);
        let ancla = Punto { x: 200, y: 150 };
        let despues = escalar_anclado(antes, 2.0, ancla, MINIMO, MAXIMO);

        assert_eq!(despues.ancho, 800);
        assert_eq!(despues.alto, 600);
        let (dx, dy) = desvio_del_ancla(antes, despues, ancla);
        assert!(dx.abs() <= 1.0 && dy.abs() <= 1.0, "{despues:?} {dx} {dy}");
    }

    #[test]
    fn el_punto_anclado_no_se_mueve_al_reducir() {
        // Un ancla lejos del centro es el caso que mas delata a un zoom que
        // en realidad sigue escalando desde el centro.
        let antes = rect(-40, 220, 640, 480);
        let ancla = Punto { x: 590, y: 690 };
        let despues = escalar_anclado(antes, 0.5, ancla, MINIMO, MAXIMO);

        let (dx, dy) = desvio_del_ancla(antes, despues, ancla);
        assert!(dx.abs() <= 1.0 && dy.abs() <= 1.0, "{despues:?} {dx} {dy}");
    }

    #[test]
    fn anclar_en_el_centro_equivale_al_zoom_de_toda_la_vida() {
        // El comportamiento antiguo tiene que seguir siendo un caso
        // particular de este, o el zoom por arrastre cambiaria sin querer.
        let antes = rect(100, 100, 400, 300);
        let centro = Punto {
            x: antes.x + antes.ancho as i32 / 2,
            y: antes.y + antes.alto as i32 / 2,
        };
        let despues = escalar_anclado(antes, 1.5, centro, MINIMO, MAXIMO);

        let clasico = rect(
            antes.x + (antes.ancho as i32 - despues.ancho as i32) / 2,
            antes.y + (antes.alto as i32 - despues.alto as i32) / 2,
            despues.ancho,
            despues.alto,
        );
        assert!(
            (despues.x - clasico.x).abs() <= 1,
            "{despues:?} {clasico:?}"
        );
        assert!(
            (despues.y - clasico.y).abs() <= 1,
            "{despues:?} {clasico:?}"
        );
    }

    #[test]
    fn la_proporcion_se_conserva_al_ampliar_y_al_reducir() {
        let antes = rect(0, 0, 1920, 1080);
        let ancla = Punto { x: 300, y: 900 };
        for factor in [0.31, 0.5, 1.0, 1.1, 2.0, 7.5] {
            let despues = escalar_anclado(antes, factor, ancla, MINIMO, MAXIMO);
            let error = (proporcion(despues) - proporcion(antes)).abs();
            // La tolerancia es la del redondeo de los lados a pixeles enteros.
            assert!(error < 0.01, "factor {factor}: {despues:?}");
        }
    }

    #[test]
    fn no_baja_del_minimo_y_al_toparse_sigue_en_proporcion() {
        // Caso negativo: un factor ridiculo no puede colapsar el pin. Manda
        // el lado corto: si se recortara cada lado por su cuenta, el pin
        // saldria cuadrado en vez de mantener su forma.
        let antes = rect(0, 0, 400, 100);
        let ancla = Punto { x: 0, y: 0 };
        let despues = escalar_anclado(antes, 0.001, ancla, MINIMO, MAXIMO);

        assert!(
            despues.ancho >= MINIMO && despues.alto >= MINIMO,
            "{despues:?}"
        );
        assert_eq!(despues.alto, MINIMO);
        let error = (proporcion(despues) - proporcion(antes)).abs();
        assert!(error < 0.01, "{despues:?}");
    }

    #[test]
    fn no_sube_del_maximo_y_al_toparse_sigue_en_proporcion() {
        // Caso negativo: aqui topa primero el lado largo. Un recorte por lado
        // dejaria el ancho clavado en el maximo y el alto creciendo solo.
        let antes = rect(0, 0, 400, 100);
        let ancla = Punto { x: 200, y: 50 };
        let despues = escalar_anclado(antes, 10_000.0, ancla, MINIMO, MAXIMO);

        assert!(
            despues.ancho <= MAXIMO && despues.alto <= MAXIMO,
            "{despues:?}"
        );
        assert_eq!(despues.ancho, MAXIMO);
        let error = (proporcion(despues) - proporcion(antes)).abs();
        assert!(error < 0.01, "{despues:?}");
    }

    #[test]
    fn un_factor_imposible_deja_el_rect_como_estaba() {
        // Caso negativo: cero, negativo, NaN e infinito no pueden producir un
        // pin de area cero ni un rect con coordenadas basura.
        let antes = rect(10, 20, 300, 200);
        let ancla = Punto { x: 50, y: 60 };
        for factor in [0.0, -2.0, f32::NAN, f32::INFINITY] {
            assert_eq!(escalar_anclado(antes, factor, ancla, MINIMO, MAXIMO), antes);
        }
    }

    #[test]
    fn el_control_converge_y_clava_el_destino_exactamente() {
        let inicial = rect(0, 0, 400, 300);
        let destino = rect(-120, 40, 1600, 1200);
        let mut c = ControlZoom::nuevo(inicial);
        assert!(c.terminado(), "recien creado no persigue nada");

        c.pedir(destino);
        assert!(!c.terminado(), "el destino esta lejos");
        assert_eq!(c.actual(), inicial, "pedir no mueve nada por si solo");

        let mut fotogramas = 0;
        while !c.terminado() && fotogramas < 1000 {
            c.paso(0.016);
            fotogramas += 1;
        }
        assert!(c.terminado(), "no convergio en {fotogramas} fotogramas");
        // Exactamente, no "casi": sin clavar quedaria medio pixel de error
        // permanente y el pin no acabaria en el tamano pedido.
        assert_eq!(c.actual(), destino);
        assert_eq!(c.actual(), c.objetivo());
    }

    #[test]
    fn llegar_en_pasos_cortos_o_largos_da_el_mismo_tamano() {
        // La prueba que define el modulo: el zoom debe verse igual a 120 Hz
        // que a 30 Hz. Una interpolacion de fraccion fija por fotograma —el
        // fallo clasico— falla aqui de largo.
        let inicial = rect(0, 0, 400, 300);
        let destino = rect(0, 0, 1600, 1200);

        // A media persecucion, que es donde mas se separan las dos formas.
        let mut rapido = ControlZoom::nuevo(inicial);
        rapido.pedir(destino);
        for _ in 0..8 {
            rapido.paso(0.005);
        }
        let mut lento = ControlZoom::nuevo(inicial);
        lento.pedir(destino);
        for _ in 0..2 {
            lento.paso(0.020);
        }
        let (a, b) = (rapido.actual(), lento.actual());
        assert!(!rapido.terminado() && !lento.terminado(), "{a:?} {b:?}");
        assert!((a.ancho as i32 - b.ancho as i32).abs() <= 3, "{a:?} {b:?}");
        assert!((a.alto as i32 - b.alto as i32).abs() <= 3, "{a:?} {b:?}");

        // Y con el mismo tiempo total (264 ms) los dos han llegado igual.
        let mut rapido = ControlZoom::nuevo(inicial);
        rapido.pedir(destino);
        for _ in 0..33 {
            rapido.paso(0.008);
        }
        let mut lento = ControlZoom::nuevo(inicial);
        lento.pedir(destino);
        for _ in 0..8 {
            lento.paso(0.033);
        }
        assert_eq!(rapido.actual(), lento.actual());
        assert_eq!(rapido.actual(), destino);
    }

    #[test]
    fn un_dt_enorme_se_acota_y_no_salta_al_destino() {
        // Caso negativo: un paron del sistema no puede teletransportar el
        // pin. Un dt de 10 s tiene que valer exactamente lo mismo que la
        // cota alta.
        let inicial = rect(0, 0, 400, 300);
        let destino = rect(0, 0, 1600, 1200);

        let mut parado = ControlZoom::nuevo(inicial);
        parado.pedir(destino);
        let tras_el_paron = parado.paso(10.0);

        let mut acotado = ControlZoom::nuevo(inicial);
        acotado.pedir(destino);
        let tras_la_cota = acotado.paso(DT_MAXIMO);

        assert_eq!(tras_el_paron, tras_la_cota);
        assert!(!parado.terminado(), "{tras_el_paron:?}");
        assert!(tras_el_paron.ancho < destino.ancho, "{tras_el_paron:?}");
    }

    #[test]
    fn un_dt_cero_negativo_o_absurdo_no_rompe_ni_retrocede() {
        // Caso negativo: un reloj que devuelve cero, un salto de hora hacia
        // atras o un NaN no pueden hacer que el pin encoja ni envenenar el
        // estado hasta dejarlo sin converger.
        let inicial = rect(0, 0, 400, 300);
        let destino = rect(0, 0, 1600, 1200);
        for dt in [0.0, -1.0, f32::NAN] {
            let mut c = ControlZoom::nuevo(inicial);
            c.pedir(destino);
            let r = c.paso(dt);
            assert!(r.ancho >= inicial.ancho, "dt {dt}: {r:?}");
            assert!(r.ancho <= destino.ancho, "dt {dt}: {r:?}");
            // Y sigue vivo: converge igual despues del paso raro.
            for _ in 0..200 {
                c.paso(0.016);
            }
            assert_eq!(c.actual(), destino, "dt {dt}");
        }
    }

    #[test]
    fn pedir_a_mitad_de_camino_redirige_sin_volver_al_principio() {
        let inicial = rect(0, 0, 400, 300);
        let lejos = rect(0, 0, 1600, 1200);
        let mut c = ControlZoom::nuevo(inicial);
        c.pedir(lejos);
        let a_medias = c.paso(0.008);
        assert!(a_medias.ancho > inicial.ancho && a_medias.ancho < lejos.ancho);

        let cerca = rect(0, 0, 600, 450);
        c.pedir(cerca);
        // El estado en curso no se toca: reiniciarlo es el salto que se
        // quiere evitar al encadenar muescas de rueda.
        assert_eq!(c.actual(), a_medias);
        assert_eq!(c.objetivo(), cerca);

        let siguiente = c.paso(0.008);
        // Va hacia el destino nuevo desde donde estaba, sin pasar otra vez
        // por el tamano de partida.
        assert!(
            siguiente.ancho < a_medias.ancho && siguiente.ancho > cerca.ancho,
            "{siguiente:?}"
        );
        for _ in 0..200 {
            c.paso(0.016);
        }
        assert_eq!(c.actual(), cerca);
    }
}
