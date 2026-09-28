//! **La vista de un documento que se lee**: el aumento propio, los margenes
//! para anotar y la columna de hojas del PDF. Puerto de las cuentas de
//! `motor/Lectura.kt` y `pdf/LectorPdfActivity.kt` del movil (v0.72-v0.85),
//! sin ventana, para poder comprobarlas.
//!
//! # Las unidades
//!
//! Todo va en **unidades del documento**, las mismas en las que vive lo
//! anotado:
//!
//! - En un Word o un libro, pixeles logicos del texto medido a escala 1, con
//!   el cero en el borde izquierdo de la columna y en lo alto del
//!   documento. El margen de la izquierda son las equis negativas, como en
//!   el movil (`vistaDeLaCapa`).
//! - En un PDF, la hoja mide [`ANCHO_HOJA`] de ancho (`PdfDoc.PAGE_WIDTH`
//!   del movil) y cada hoja tiene su propio cero en su esquina: asi lo
//!   anotado en una hoja es un dibujo de esa hoja, igual que alli.
//!
//! La vista es **el punto del documento que cae en la esquina de arriba a
//! la izquierda de la ventana** mas cuantos pixeles mide una unidad. Con eso
//! el texto y la tinta se colocan con la misma cuenta en la misma pasada de
//! pintado, y la tinta no puede ir un fotograma por detras del texto (el
//! fallo que la v0.85 del movil arreglo metiendola en la pagina).

use std::ops::Range;

/// Lo que mas se acerca un documento (`AUMENTO_MAXIMO` del movil).
pub const ZOOM_MAXIMO: f32 = 5.0;
/// Lo que mas se acerca un PDF: seis, como el pellizco del lector del
/// movil (`coerceIn(minimo, 6f)`); un plano pide ver cotas pequenas.
pub const ZOOM_MAXIMO_PDF: f32 = 6.0;
/// Por debajo de esto no se aleja nada: el texto ya no se distingue y un
/// aumento cero dejaria el documento sin tamano.
pub const ZOOM_MINIMO: f32 = 0.1;

/// Cuanto cambia el aumento por cada muesca de la rueda con Ctrl.
pub const PASO_DE_ZOOM: f32 = 1.15;

/// El ancho de una hoja de PDF en unidades del dibujo (`PdfDoc.PAGE_WIDTH`).
/// El mismo numero que el movil para que un dibujo de hoja hecho alli caiga
/// en el mismo sitio aqui.
pub const ANCHO_HOJA: f32 = 1400.0;
/// El aire entre dos hojas, en unidades (los 6 dp del movil a su escala).
pub const HUECO_ENTRE_HOJAS: f32 = 18.0;

/// **El margen de un PDF, para anotar**: tres cuartos del ancho de la hoja a
/// cada lado (`Lectura.MARGEN_DEL_PDF`).
pub const MARGEN_DEL_PDF: f32 = 0.75;
/// Los espacios para anotar, en bits (`ESPACIO_IZQUIERDA`/`ESPACIO_DERECHA`).
pub const ESPACIO_IZQUIERDA: u8 = 1;
pub const ESPACIO_DERECHA: u8 = 2;

/// Muy cerca del centro, se encaja siempre (`CERCA_DEL_CENTRO`).
const CERCA_DEL_CENTRO: f32 = 0.12;

/// Cuanto margen se abre a cada lado de la columna de un Word para anotar:
/// **dos tercios de su ancho** (`Lectura.margenDe`).
pub fn margen_de(columna: f32) -> f32 {
    columna * 2.0 / 3.0
}

/// La columna con sus dos margenes (`Lectura.anchoConMargenes`).
pub fn ancho_con_margenes(columna: f32) -> f32 {
    columna + 2.0 * margen_de(columna)
}

/// Pone o quita un espacio del PDF: el boton de cada lado es un
/// interruptor. Solo hay dos bits; lo demas se tira.
pub fn con_espacio(espacios: u8, lado: u8) -> u8 {
    (espacios ^ lado) & (ESPACIO_IZQUIERDA | ESPACIO_DERECHA)
}

/// Lo que se abre a la izquierda y a la derecha de las hojas, en unidades.
pub fn espacios_del_pdf(espacios: u8) -> (f32, f32) {
    let m = ANCHO_HOJA * MARGEN_DEL_PDF;
    (
        if espacios & ESPACIO_IZQUIERDA != 0 { m } else { 0.0 },
        if espacios & ESPACIO_DERECHA != 0 { m } else { 0.0 },
    )
}

/// Donde puede estar la vista sin salirse del documento
/// (`Lectura.dentroDelDocumento`), con dos cambios de escritorio:
///
/// - El documento va de `izq` a `der` a lo ancho (en un Word con margenes,
///   `izq` es negativo). Si cabe entero en la ventana **se centra**: en un
///   monitor ancho la columna tiene que quedar en medio, no pegada a un
///   lado, que es lo que hacia pegarla al cero.
/// - A lo alto no se sube por encima del principio ni se baja mas alla de
///   que el final toque el borde de abajo.
///
/// `vista_ancho` y `vista_alto` van en unidades (pixeles / escala).
pub fn dentro(
    x: f32,
    y: f32,
    izq: f32,
    der: f32,
    alto: f32,
    vista_ancho: f32,
    vista_alto: f32,
) -> (f32, f32) {
    let ancho = (der - izq).max(0.0);
    let x = if ancho <= vista_ancho || !x.is_finite() {
        izq - (vista_ancho - ancho) / 2.0
    } else {
        x.clamp(izq, der - vista_ancho)
    };
    let tope_y = (alto - vista_alto).max(0.0);
    let y = if y.is_finite() {
        y.clamp(0.0, tope_y)
    } else {
        0.0
    };
    (x, y)
}

/// **El punto bajo el raton se queda quieto** al cambiar el aumento: sin
/// esto el documento crece desde la esquina y lo que se queria mirar se
/// escapa por un lado. Devuelve la nueva esquina de la vista (una
/// coordenada; se llama una vez por eje). `foco` va en pixeles desde la
/// esquina de la ventana, `escala_*` en pixeles por unidad.
pub fn con_foco(esquina: f32, foco: f32, escala_antes: f32, escala_despues: f32) -> f32 {
    if escala_antes <= 0.0 || escala_despues <= 0.0 {
        return esquina;
    }
    let bajo_el_raton = esquina + foco / escala_antes;
    bajo_el_raton - foco / escala_despues
}

/// El aumento despues de `muescas` de rueda con Ctrl (positivo acerca),
/// dentro de `[minimo, maximo]`.
pub fn zoom_con_rueda(zoom: f32, muescas: f32, minimo: f32, maximo: f32) -> f32 {
    let z = zoom * PASO_DE_ZOOM.powf(muescas);
    if z.is_finite() {
        z.clamp(minimo, maximo)
    } else {
        zoom.clamp(minimo, maximo)
    }
}

/// **Lo mas que se aleja**: hasta que el documento entero (la columna y los
/// margenes que haya) cabe a lo ancho, y nunca por encima de 1 —de borde a
/// borde de la columna, como se abre—. Es la regla de la v0.73/v0.75 del
/// movil: alejar sirve para ver el texto con lo anotado a su lado, no para
/// convertir la pagina en un sello.
///
/// `ancho_doc` en unidades; `vista_px` en pixeles; `px_por_unidad` es lo
/// que mide una unidad a aumento 1.
pub fn zoom_minimo(ancho_doc: f32, vista_px: f32, px_por_unidad: f32) -> f32 {
    if ancho_doc <= 0.0 || px_por_unidad <= 0.0 {
        return 1.0;
    }
    (vista_px / (ancho_doc * px_por_unidad)).clamp(ZOOM_MINIMO, 1.0)
}

/// **El iman del centro** (`Lectura.imanDelCentro`). Con margenes a los
/// lados lo que hay que ver siempre es el texto: se puede ir a un margen y
/// quedarse en el, pero en cuanto se empuja de vuelta hacia el centro, la
/// vista se va al centro y encaja. `antes` y `ahora` son la esquina de la
/// vista al empezar y al acabar el gesto; `centro`, la esquina con el texto
/// de borde a borde. Devuelve a donde ir, o `None` para quedarse.
pub fn iman_del_centro(antes: f32, ahora: f32, centro: f32, margen: f32) -> Option<f32> {
    let lejos = (ahora - centro).abs();
    if lejos < 0.5 {
        return None;
    }
    let casi = lejos <= margen * CERCA_DEL_CENTRO;
    let hacia = lejos < (antes - centro).abs() - 1.0;
    (casi || hacia).then_some(centro)
}

/// Las hojas de un PDF una debajo de otra, en unidades: donde empieza cada
/// una y cuanto mide. Se calcula una vez con las medidas de las paginas,
/// antes de pintar ninguna: sin eso el desplazamiento pega saltos segun van
/// llegando las hojas (el `Spacer` con la altura que le toca del movil).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Hojas {
    pub arriba: Vec<f32>,
    pub altos: Vec<f32>,
    pub total: f32,
}

/// La proporcion de un A4 vertical: la de una hoja cuyas medidas no se
/// pudieron leer. Mejor una hoja de tamano corriente que una de alto cero.
const PROPORCION_A4: f32 = 1.414;

impl Hojas {
    /// `medidas` son el ancho y el alto de cada pagina, en lo que sea
    /// (solo cuenta la proporcion).
    pub fn colocar(medidas: &[(f32, f32)]) -> Hojas {
        let mut arriba = Vec::with_capacity(medidas.len());
        let mut altos = Vec::with_capacity(medidas.len());
        let mut y = 0.0f32;
        for &(w, h) in medidas {
            let proporcion = if w > 0.0 && h > 0.0 && (h / w).is_finite() {
                (h / w).clamp(0.05, 20.0)
            } else {
                PROPORCION_A4
            };
            let alto = ANCHO_HOJA * proporcion;
            arriba.push(y);
            altos.push(alto);
            y += alto + HUECO_ENTRE_HOJAS;
        }
        Hojas {
            arriba,
            altos,
            total: (y - HUECO_ENTRE_HOJAS).max(0.0),
        }
    }

    pub fn cuantas(&self) -> usize {
        self.arriba.len()
    }

    /// La hoja que hay a la altura `y` (la de encima si cae en el hueco).
    pub fn en(&self, y: f32) -> usize {
        if self.arriba.is_empty() {
            return 0;
        }
        let despues = self.arriba.partition_point(|&a| a <= y);
        despues.saturating_sub(1).min(self.arriba.len() - 1)
    }

    /// Las hojas que asoman entre `y0` e `y1`.
    pub fn visibles(&self, y0: f32, y1: f32) -> Range<usize> {
        if self.arriba.is_empty() || y1 < y0 {
            return 0..0;
        }
        let desde = self.en(y0);
        let hasta = self.arriba.partition_point(|&a| a < y1);
        desde..hasta.max(desde + 1).min(self.arriba.len())
    }

    /// **Por donde va la lectura**: la hoja mas la fraccion de su alto que
    /// queda por encima de `y` (`dondeEstoy` del movil). Es lo que se guarda
    /// y lo que lleva una marca en su `y`.
    pub fn sitio(&self, y: f32) -> f64 {
        if self.arriba.is_empty() {
            return 0.0;
        }
        let i = self.en(y);
        let dentro = ((y - self.arriba[i]) / self.altos[i].max(1e-3)).clamp(0.0, 1.0);
        i as f64 + dentro as f64
    }

    /// La altura que corresponde a un sitio (la inversa de [`Hojas::sitio`]).
    /// Un sitio de una hoja que ya no existe (el PDF cambio) cae en la
    /// ultima.
    pub fn y_de(&self, sitio: f64) -> f32 {
        if self.arriba.is_empty() || !sitio.is_finite() {
            return 0.0;
        }
        let i = (sitio.max(0.0).floor() as usize).min(self.arriba.len() - 1);
        let dentro = (sitio - i as f64).clamp(0.0, 1.0) as f32;
        self.arriba[i] + dentro * self.altos[i]
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_margen_de_un_word_son_dos_tercios_de_la_columna_a_cada_lado() {
        assert_eq!(margen_de(600.0), 400.0);
        assert_eq!(ancho_con_margenes(600.0), 1400.0);
    }

    #[test]
    fn cada_boton_de_espacio_enciende_y_apaga_su_lado() {
        let mut e = 0;
        e = con_espacio(e, ESPACIO_IZQUIERDA);
        assert_eq!(e, 1);
        e = con_espacio(e, ESPACIO_DERECHA);
        assert_eq!(e, 3);
        e = con_espacio(e, ESPACIO_IZQUIERDA);
        assert_eq!(e, 2, "pulsar otra vez quita solo ese lado");
        assert_eq!(con_espacio(0, 0xFC), 0, "bits que no son lados no entran");
    }

    #[test]
    fn los_espacios_del_pdf_son_tres_cuartos_de_hoja() {
        assert_eq!(espacios_del_pdf(0), (0.0, 0.0));
        assert_eq!(espacios_del_pdf(1), (1050.0, 0.0));
        assert_eq!(espacios_del_pdf(3), (1050.0, 1050.0));
    }

    #[test]
    fn un_documento_mas_estrecho_que_la_ventana_se_centra() {
        // Columna de 800 en una ventana de 1600: 400 de aire a cada lado.
        let (x, _) = dentro(123.0, 0.0, 0.0, 800.0, 5000.0, 1600.0, 900.0);
        assert_eq!(x, -400.0);
        // Con margenes (izquierdo negativo) tambien se centra el conjunto.
        let (x, _) = dentro(0.0, 0.0, -100.0, 900.0, 5000.0, 2000.0, 900.0);
        assert_eq!(x, -600.0);
    }

    #[test]
    fn un_documento_ancho_no_deja_ver_fuera_de_sus_bordes() {
        let (x, _) = dentro(-5000.0, 0.0, -500.0, 1500.0, 5000.0, 1000.0, 900.0);
        assert_eq!(x, -500.0);
        let (x, _) = dentro(5000.0, 0.0, -500.0, 1500.0, 5000.0, 1000.0, 900.0);
        assert_eq!(x, 500.0);
    }

    #[test]
    fn a_lo_alto_no_se_sube_del_principio_ni_se_baja_del_final() {
        let (_, y) = dentro(0.0, -40.0, 0.0, 10.0, 5000.0, 100.0, 900.0);
        assert_eq!(y, 0.0);
        let (_, y) = dentro(0.0, 99_999.0, 0.0, 10.0, 5000.0, 100.0, 900.0);
        assert_eq!(y, 4100.0);
        let (_, y) = dentro(0.0, 300.0, 0.0, 10.0, 200.0, 100.0, 900.0);
        assert_eq!(y, 0.0, "un documento corto se queda arriba");
        let (x, y) = dentro(f32::NAN, f32::NAN, 0.0, 10.0, 5000.0, 100.0, 900.0);
        assert!(x.is_finite() && y == 0.0, "un NaN no envenena la vista");
    }

    #[test]
    fn el_punto_bajo_el_raton_no_se_mueve_al_acercar() {
        let esquina = 100.0;
        let foco = 300.0;
        let antes = esquina + foco / 1.0;
        let nueva = con_foco(esquina, foco, 1.0, 2.0);
        let despues = nueva + foco / 2.0;
        assert!((antes - despues).abs() < 1e-3);
        assert_eq!(con_foco(7.0, 300.0, 0.0, 2.0), 7.0, "escala rota: quieto");
    }

    #[test]
    fn la_rueda_acerca_poco_a_poco_y_no_pasa_los_topes() {
        let z = zoom_con_rueda(1.0, 1.0, 0.5, 5.0);
        assert!((z - PASO_DE_ZOOM).abs() < 1e-5);
        assert_eq!(zoom_con_rueda(4.9, 10.0, 0.5, 5.0), 5.0);
        assert_eq!(zoom_con_rueda(0.6, -10.0, 0.5, 5.0), 0.5);
        assert_eq!(zoom_con_rueda(1.0, f32::INFINITY, 0.5, 5.0), 1.0);
    }

    #[test]
    fn se_aleja_hasta_ver_la_columna_con_sus_dos_margenes_y_no_mas() {
        // Columna de 600 con margenes: 1400 de documento en 1000 px.
        let z = zoom_minimo(ancho_con_margenes(600.0), 1000.0, 1.0);
        assert!((z - 1000.0 / 1400.0).abs() < 1e-5);
        // Sin margenes y en un monitor ancho no se aleja por debajo de 1.
        assert_eq!(zoom_minimo(600.0, 1920.0, 1.0), 1.0);
        assert_eq!(zoom_minimo(0.0, 1000.0, 1.0), 1.0, "sin documento, 1");
        assert_eq!(zoom_minimo(1e9, 10.0, 1.0), ZOOM_MINIMO);
    }

    #[test]
    fn el_iman_lleva_al_centro_si_se_vuelve_hacia_el() {
        // Estaba en el margen (0) y se empuja hacia el centro (400): encaja.
        assert_eq!(iman_del_centro(0.0, 150.0, 400.0, 400.0), Some(400.0));
        // Se aleja del centro hacia el margen: se queda donde se solto.
        assert_eq!(iman_del_centro(400.0, 150.0, 400.0, 400.0), None);
        // Casi en el centro: encaja siempre.
        assert_eq!(iman_del_centro(0.0, 390.0, 400.0, 400.0), Some(400.0));
        // Ya en el centro: nada que hacer.
        assert_eq!(iman_del_centro(0.0, 400.2, 400.0, 400.0), None);
    }

    fn tres() -> Hojas {
        // A4 vertical, apaisada y otra A4.
        Hojas::colocar(&[(595.0, 842.0), (842.0, 595.0), (595.0, 842.0)])
    }

    #[test]
    fn las_hojas_se_apilan_con_su_proporcion_y_su_hueco() {
        let h = tres();
        assert_eq!(h.cuantas(), 3);
        assert_eq!(h.arriba[0], 0.0);
        assert!((h.altos[0] - ANCHO_HOJA * 842.0 / 595.0).abs() < 0.01);
        assert!((h.arriba[1] - (h.altos[0] + HUECO_ENTRE_HOJAS)).abs() < 0.01);
        assert!(h.altos[1] < ANCHO_HOJA, "la apaisada es mas baja que ancha");
        let fin = h.arriba[2] + h.altos[2];
        assert!((h.total - fin).abs() < 0.01);
    }

    #[test]
    fn una_hoja_sin_medidas_sale_como_un_a4_y_no_de_alto_cero() {
        let h = Hojas::colocar(&[(0.0, 0.0), (f32::NAN, 3.0)]);
        assert!(h.altos.iter().all(|a| *a > 1000.0));
    }

    #[test]
    fn se_sabe_en_que_hoja_se_esta_y_que_hojas_asoman() {
        let h = tres();
        assert_eq!(h.en(-50.0), 0);
        assert_eq!(h.en(h.arriba[1] + 1.0), 1);
        assert_eq!(h.en(h.arriba[1] - 1.0), 0, "en el hueco manda la de encima");
        assert_eq!(h.en(1e9), 2);
        assert_eq!(h.visibles(0.0, 10.0), 0..1);
        assert_eq!(h.visibles(h.arriba[1] - 5.0, h.arriba[2] + 5.0), 0..3);
        assert_eq!(h.visibles(10.0, 0.0), 0..0);
        assert_eq!(Hojas::default().visibles(0.0, 100.0), 0..0);
    }

    #[test]
    fn el_sitio_es_la_hoja_mas_la_fraccion_y_vuelve_igual() {
        let h = tres();
        let y = h.arriba[1] + h.altos[1] * 0.5;
        let s = h.sitio(y);
        assert!((s - 1.5).abs() < 1e-4);
        assert!((h.y_de(s) - y).abs() < 0.01);
        assert_eq!(h.y_de(99.0), h.arriba[2] + h.altos[2], "una hoja que ya no esta cae en la ultima");
        assert_eq!(h.y_de(f64::NAN), 0.0);
        assert_eq!(Hojas::default().sitio(100.0), 0.0);
    }
}
