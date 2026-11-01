//! **El historial que se recorre sin saltos** (el usuario, 1-nov-2026: «al
//! subir no se traba, pero carga: sale una imagen, puf, crece su sitio y me
//! empuja hacia arriba»).
//!
//! Tres piezas, las tres sin Win32 para poder probarlas:
//!
//! - [`Desliz`]: la rueda animada. Cada muesca mueve el destino y la vista
//!   lo persigue con una frenada exponencial (~150 ms), acumulando las
//!   muescas seguidas en vez de empezar de cero en cada una.
//! - [`ancla`] y [`reanclar`]: si algo de ENCIMA de lo que se ve cambia de
//!   alto (una pagina de PDF que llega, un texto que se recoloca), la
//!   primera burbuja que se ve se queda donde estaba en la pantalla y el
//!   desplazamiento absorbe la diferencia. Es el `overflow-anchor` de CSS.
//! - [`a_precargar`]: que burbujas, fuera de la pantalla, conviene ir
//!   leyendo ya, en el orden en que se van a necesitar.

use crate::historial::Puesto;

/// Cuanto tarda la vista en recorrer ~63 % de lo que le falta, en ms. Con
/// 38, a los 150 ms queda menos de un 2 %: se nota suave sin ir con retraso
/// respecto a la mano.
pub const DESLIZ_TAU_MS: f32 = 38.0;

/// A menos de esto del destino la animacion se da por terminada y se clava.
const DESLIZ_LLEGADO: f32 = 0.5;

/// Cada cuanto se pinta mientras se anima (~60 fotogramas por segundo).
pub const DESLIZ_FOTOGRAMA_MS: u32 = 15;

/// Lo que da una muesca de una rueda de verdad (`WHEEL_DELTA`).
pub const MUESCA: i32 = 120;

/// **Si un giro de rueda viene de una rueda con muescas** (se anima) o de
/// un panel tactil o una rueda de precision (se aplica tal cual: el propio
/// gesto ya es suave, y animarlo lo haria ir con retraso).
///
/// Windows manda multiplos de 120 por muesca; un panel manda trocitos.
pub fn es_de_muescas(delta: i32) -> bool {
    delta != 0 && delta % MUESCA == 0
}

/// La rueda animada del historial.
///
/// Guarda posiciones en pixeles del documento (las mismas que el
/// `scroll` de quien pinta), en coma flotante para que la frenada no se
/// quede clavada a un pixel por redondeo.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Desliz {
    actual: f32,
    objetivo: f32,
    /// El instante del ultimo paso, en ms de un reloj que solo avanza.
    ultimo_ms: u64,
    activo: bool,
    /// Lo ultimo que se escribio en el `scroll` de quien pinta: si al dar
    /// el siguiente paso ya no es eso, alguien lo movio por su cuenta (ir a
    /// un mensaje, volver al final) y la animacion se rinde.
    escrito: i32,
}

impl Desliz {
    /// Si hay una animacion en marcha.
    pub fn activo(&self) -> bool {
        self.activo
    }

    /// Adonde va a parar, si se esta moviendo.
    pub fn destino(&self) -> Option<i32> {
        self.activo.then(|| self.objetivo.round() as i32)
    }

    /// Una muesca: mueve el destino `cuanto` desde donde iba (o desde
    /// `desde`, lo que se ve ahora, si estaba quieta), sin salirse de
    /// `[minimo, maximo]`.
    pub fn empujar(&mut self, desde: i32, cuanto: i32, ahora_ms: u64, minimo: i32, maximo: i32) {
        if !self.activo || self.escrito != desde {
            self.actual = desde as f32;
            self.objetivo = desde as f32;
            self.escrito = desde;
            self.ultimo_ms = ahora_ms;
        }
        let (minimo, maximo) = (minimo.min(maximo) as f32, maximo as f32);
        self.objetivo = (self.objetivo + cuanto as f32).clamp(minimo, maximo);
        self.activo = (self.objetivo - self.actual).abs() >= DESLIZ_LLEGADO;
    }

    /// Un paso de la animacion. Devuelve el desplazamiento que hay que
    /// pintar ahora, o `None` si no hay animacion o `visto` (el desplazamiento
    /// que tiene quien pinta) ya no es el que dejo el paso anterior.
    pub fn avanzar(&mut self, visto: i32, ahora_ms: u64) -> Option<i32> {
        if !self.activo {
            return None;
        }
        if visto != self.escrito {
            self.activo = false;
            return None;
        }
        let dt = ahora_ms.saturating_sub(self.ultimo_ms) as f32;
        self.ultimo_ms = ahora_ms;
        let falta = self.objetivo - self.actual;
        let k = 1.0 - (-dt / DESLIZ_TAU_MS).exp();
        self.actual += falta * k;
        if (self.objetivo - self.actual).abs() < DESLIZ_LLEGADO {
            self.actual = self.objetivo;
            self.activo = false;
        }
        self.escrito = self.actual.round() as i32;
        Some(self.escrito)
    }

    /// Lo pintado se corrio `cuanto` por el ancla (algo de arriba cambio de
    /// alto): origen y destino se corren igual, o la animacion devolveria
    /// la vista al sitio de antes.
    pub fn correr(&mut self, cuanto: i32) {
        self.actual += cuanto as f32;
        self.objetivo += cuanto as f32;
        self.escrito += cuanto;
    }

    /// Para en seco donde este (un panel tactil, un salto a un mensaje).
    pub fn parar(&mut self) {
        self.activo = false;
    }

    /// Cuanto puede dormir el bucle antes del siguiente paso.
    pub fn ms_hasta(&self) -> Option<u32> {
        self.activo.then_some(DESLIZ_FOTOGRAMA_MS)
    }
}

/// La primera burbuja de la que se ve el borde de arriba (o, si una sola
/// llena la pantalla, la que se ve), y a cuanto queda ese borde del de
/// arriba de la vista. Es lo que se deja quieto cuando lo de encima cambia.
///
/// Se elige la que asoma ENTERA por arriba y no la cortada: si la que esta
/// a medio ver es justo la que crece (una pagina que llega), crecer hacia
/// arriba no movera nada de lo que se lee debajo.
pub fn ancla(puestos: &[Puesto], scroll: i32, alto_vista: u32) -> Option<(usize, i32)> {
    let abajo = scroll + alto_vista as i32;
    let primero = puestos.partition_point(|p| p.burbuja.abajo() < scroll);
    let cual = puestos[primero..]
        .iter()
        .position(|p| p.burbuja.y >= scroll)
        .map(|k| primero + k)
        .filter(|&k| puestos[k].burbuja.y < abajo)
        .or_else(|| (primero < puestos.len()).then_some(primero))?;
    Some((cual, puestos[cual].burbuja.y - scroll))
}

/// El desplazamiento con el que la burbuja `cual` de la colocacion NUEVA
/// queda a `desde_arriba` del borde de la vista, como estaba.
pub fn reanclar(puestos: &[Puesto], cual: usize, desde_arriba: i32) -> Option<i32> {
    Some(puestos.get(cual)?.burbuja.y - desde_arriba)
}

/// Hacia donde se mueve la vista, para precargar primero por ese lado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sentido {
    /// Hacia lo viejo.
    Arriba,
    /// Hacia lo nuevo (y quieta: se abre por abajo).
    #[default]
    Abajo,
}

impl Sentido {
    /// El sentido de ir de `antes` a `ahora`; quieto, el que ya llevaba.
    pub fn de(antes: i32, ahora: i32, llevaba: Sentido) -> Sentido {
        match ahora.cmp(&antes) {
            std::cmp::Ordering::Less => Sentido::Arriba,
            std::cmp::Ordering::Greater => Sentido::Abajo,
            std::cmp::Ordering::Equal => llevaba,
        }
    }
}

/// **Las burbujas que conviene ir leyendo**, por su posicion en `puestos`
/// y en el orden en que se van a ver: primero las de `pantallas` vistas
/// hacia donde se va (de la mas cercana a la mas lejana) y luego las del
/// otro lado, que puede que se vuelva. Las que se ven no van: esas se piden
/// aparte y antes.
pub fn a_precargar(
    puestos: &[Puesto],
    scroll: i32,
    alto_vista: u32,
    pantallas: f32,
    sentido: Sentido,
) -> Vec<usize> {
    let alto = alto_vista as i32;
    let margen = (alto as f32 * pantallas.max(0.0)) as i32;
    let primero = puestos.partition_point(|p| p.burbuja.abajo() < scroll);
    let fin = primero + puestos[primero..].partition_point(|p| p.arriba() <= scroll + alto);
    // Lo de arriba, de la mas cercana a la mas lejana.
    let desde_arriba = puestos.partition_point(|p| p.burbuja.abajo() < scroll - margen);
    let arriba = (desde_arriba..primero).rev();
    // Lo de abajo, igual.
    let hasta_abajo =
        fin + puestos[fin..].partition_point(|p| p.arriba() <= scroll + alto + margen);
    let abajo = fin..hasta_abajo;
    match sentido {
        Sentido::Arriba => arriba.chain(abajo).collect(),
        Sentido::Abajo => abajo.chain(arriba).collect(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pindesk_geom::Rect;

    /// Una columna de burbujas de `alto` cada una, sin hueco entre ellas.
    fn columna(altos: &[u32]) -> Vec<Puesto> {
        let mut y = 0;
        altos
            .iter()
            .map(|&alto| {
                let p = Puesto {
                    burbuja: Rect {
                        x: 0,
                        y,
                        ancho: 100,
                        alto,
                    },
                    separador: None,
                    nombre: None,
                };
                y += alto as i32;
                p
            })
            .collect()
    }

    #[test]
    fn una_muesca_llega_a_su_sitio_con_frenada_y_en_unos_150_ms() {
        let mut d = Desliz::default();
        d.empujar(1000, -120, 0, 0, 5000);
        assert!(d.activo());
        let mut visto = 1000;
        let mut pasos = Vec::new();
        for t in (16..=400).step_by(16) {
            match d.avanzar(visto, t) {
                Some(s) => {
                    pasos.push(s);
                    visto = s;
                }
                None => break,
            }
        }
        assert_eq!(*pasos.last().unwrap(), 880, "acaba justo en el destino");
        // Frena: arranca rapido y llega despacio.
        let saltos: Vec<i32> = pasos.windows(2).map(|w| (w[0] - w[1]).abs()).collect();
        assert!(1000 - pasos[0] >= 30, "{pasos:?}");
        assert!(saltos[0] > *saltos.last().unwrap(), "{saltos:?}");
        // A los ~150 ms ya casi ha llegado.
        assert!((pasos[8] - 880).abs() <= 3, "{pasos:?}");
        assert!(!d.activo());
    }

    #[test]
    fn las_muescas_seguidas_se_suman_en_vez_de_empezar_de_cero() {
        let mut d = Desliz::default();
        d.empujar(1000, -120, 0, 0, 5000);
        let visto = d.avanzar(1000, 16).unwrap();
        // La segunda muesca llega con la primera a medias: el destino es
        // la suma de las dos, no «120 desde donde va».
        d.empujar(visto, -120, 16, 0, 5000);
        assert_eq!(d.destino(), Some(760));
    }

    #[test]
    fn el_destino_no_se_sale_del_historial() {
        let mut d = Desliz::default();
        d.empujar(50, -500, 0, 0, 5000);
        assert_eq!(d.destino(), Some(0));
        let mut d = Desliz::default();
        // Contra el tope de abajo y ya en el: no hay nada que animar.
        d.empujar(5000, 360, 0, 0, 5000);
        assert!(!d.activo());
    }

    #[test]
    fn si_otro_mueve_la_vista_la_animacion_se_rinde() {
        let mut d = Desliz::default();
        d.empujar(1000, -120, 0, 0, 5000);
        // Caso negativo: un salto a un mensaje puso otro desplazamiento.
        assert_eq!(d.avanzar(300, 16), None);
        assert!(!d.activo());
    }

    #[test]
    fn el_ancla_corre_origen_y_destino_por_igual() {
        let mut d = Desliz::default();
        d.empujar(1000, -120, 0, 0, 5000);
        let visto = d.avanzar(1000, 16).unwrap();
        d.correr(200);
        assert_eq!(d.destino(), Some(1080));
        assert!(d.avanzar(visto + 200, 32).is_some(), "sigue animando");
    }

    #[test]
    fn un_panel_tactil_no_se_anima_y_una_rueda_si() {
        assert!(es_de_muescas(120));
        assert!(es_de_muescas(-240));
        assert!(!es_de_muescas(15), "trocito de panel tactil");
        assert!(!es_de_muescas(-130));
        assert!(!es_de_muescas(0));
    }

    #[test]
    fn el_ancla_es_la_primera_que_asoma_entera_por_arriba() {
        let p = columna(&[100, 100, 100, 100]);
        // Con la vista en 150, la segunda (100..200) esta cortada; la
        // tercera empieza a 50 del borde.
        assert_eq!(ancla(&p, 150, 200), Some((2, 50)));
        // Justo en un borde, esa misma a cero.
        assert_eq!(ancla(&p, 100, 200), Some((1, 0)));
    }

    #[test]
    fn una_burbuja_que_llena_la_vista_es_su_propia_ancla() {
        let p = columna(&[100, 1000, 100]);
        assert_eq!(ancla(&p, 300, 200), Some((1, -200)));
        // Sin nada colocado no hay ancla (caso negativo).
        assert_eq!(ancla(&[], 0, 200), None);
    }

    #[test]
    fn si_crece_lo_de_arriba_lo_que_se_ve_no_se_mueve_en_pantalla() {
        let antes = columna(&[100, 100, 100, 100, 100]);
        let (cual, desde_arriba) = ancla(&antes, 250, 200).unwrap();
        // Llega la pagina del primer mensaje: pasa de 100 a 340.
        let despues = columna(&[340, 100, 100, 100, 100]);
        let nuevo = reanclar(&despues, cual, desde_arriba).unwrap();
        assert_eq!(nuevo, 250 + 240, "el desplazamiento absorbe lo crecido");
        assert_eq!(
            despues[cual].burbuja.y - nuevo,
            antes[cual].burbuja.y - 250,
            "la burbuja de arriba de la vista sigue en el mismo pixel"
        );
        // Y si lo que crece esta debajo, nada cambia.
        let debajo = columna(&[100, 100, 100, 100, 500]);
        assert_eq!(reanclar(&debajo, cual, desde_arriba), Some(250));
        // Una burbuja que ya no existe no ancla (caso negativo).
        assert_eq!(reanclar(&debajo, 99, 0), None);
    }

    #[test]
    fn se_precarga_primero_hacia_donde_se_va() {
        // Diez de 100; la vista (200 de alto) en 450 ve de la 4 a la 6.
        let p = columna(&[100; 10]);
        let subiendo = a_precargar(&p, 450, 200, 1.0, Sentido::Arriba);
        // Una pantalla por cada lado: arriba la 3 y la 2 (de cerca a
        // lejos), luego abajo la 7 y la 8; las que se ven no van.
        assert_eq!(subiendo, vec![3, 2, 7, 8]);
        let bajando = a_precargar(&p, 450, 200, 1.0, Sentido::Abajo);
        assert_eq!(bajando, vec![7, 8, 3, 2], "bajando, primero lo de debajo");
        // Lo que queda lejos no se pide (se cancela solo).
        assert!(
            !bajando.contains(&0) && !bajando.contains(&9),
            "{bajando:?}"
        );
    }

    #[test]
    fn sin_pantallas_de_margen_no_se_precarga_nada_fuera_de_la_vista() {
        let p = columna(&[100; 10]);
        let nada = a_precargar(&p, 400, 200, 0.0, Sentido::Abajo);
        assert!(
            nada.len() <= 2,
            "como mucho las que tocan el borde: {nada:?}"
        );
        assert!(a_precargar(&[], 0, 200, 2.0, Sentido::Arriba).is_empty());
    }

    #[test]
    fn el_sentido_sale_de_como_cambia_el_desplazamiento() {
        assert_eq!(Sentido::de(500, 400, Sentido::Abajo), Sentido::Arriba);
        assert_eq!(Sentido::de(400, 500, Sentido::Arriba), Sentido::Abajo);
        assert_eq!(Sentido::de(400, 400, Sentido::Arriba), Sentido::Arriba);
    }
}
