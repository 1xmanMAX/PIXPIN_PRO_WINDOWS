//! El reloj del editor de grabaciones (P5b.4), sin nada de pantalla.
//!
//! Al parar de grabar no se guarda a lo bruto: se abre un editor donde se
//! ve lo grabado, se recorre con la linea de tiempo y se decide que hacer
//! con ello. Aqui esta lo que decide QUE fotograma toca y donde caen los
//! mandos, que es lo unico comprobable sin abrir una ventana.
//!
//! El avance va por tiempo transcurrido y no por vueltas del bucle. Contar
//! vueltas ata la velocidad de la animacion a lo rapido que vaya el
//! equipo: la misma grabacion se veria a otra velocidad en otra maquina, y
//! en la misma maquina cambiaria segun lo ocupada que estuviera.

use pixpin_render::RectF;
use std::time::Duration;

/// Velocidades que se ofrecen. La unidad esta en medio a proposito: se
/// llega a ella desde los dos lados sin dar la vuelta a la lista.
pub const VELOCIDADES: [f32; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];
pub const VELOCIDAD_NORMAL: usize = 2;

/// Alto de la barra de mandos del editor, en pixeles logicos.
pub const MANDOS_ALTO: u32 = 46;
/// Margen alrededor de la imagen dentro de la ventana.
pub const MARGEN: u32 = 12;
/// Lo mas estrecha que puede ser la ventana del editor.
///
/// No es un numero de gusto: por debajo de esto, el grupo de mandos de
/// la izquierda y el de la derecha se solapan y quedan botones que no se
/// pueden pulsar. Lo vigila la prueba `los_mandos_caben_y_no_se_pisan`,
/// que lo caza si alguien ensancha un rotulo sin mirar.
pub const ANCHO_MINIMO: u32 = 520;

/// Los mandos del editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mando {
    /// Reproducir o parar, segun este.
    Reproducir,
    /// Un fotograma atras, con la reproduccion parada.
    Anterior,
    /// Un fotograma adelante.
    Siguiente,
    /// Cambia a la velocidad siguiente de la lista, dando la vuelta.
    Velocidad,
    /// Cambia el formato con el que se va a guardar.
    Formato,
    /// Preguntar donde guardar.
    Guardar,
    /// Guardar en la carpeta de capturas sin preguntar.
    GuardadoRapido,
    /// Dejar el fichero en el portapapeles para pegarlo en otro sitio.
    Copiar,
    /// Tirar lo grabado.
    Descartar,
}

/// En que formato se guarda la grabacion.
///
/// El GIF se pega en cualquier sitio y se ve solo, pero pesa; el MP4
/// pesa una fraccion y lo entiende todo lo que reproduzca video, aunque
/// no se anima en las vistas previas. No hay uno mejor, por eso se
/// elige.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    Gif,
    Mp4,
}

impl Formato {
    pub fn extension(self) -> &'static str {
        match self {
            Formato::Gif => "gif",
            Formato::Mp4 => "mp4",
        }
    }

    /// El rotulo del boton. Va en mayusculas y sin traducir: los nombres
    /// de los formatos son los mismos en todos los idiomas.
    pub fn rotulo(self) -> &'static str {
        match self {
            Formato::Gif => "GIF",
            Formato::Mp4 => "MP4",
        }
    }

    pub fn siguiente(self) -> Formato {
        match self {
            Formato::Gif => Formato::Mp4,
            Formato::Mp4 => Formato::Gif,
        }
    }
}

/// Lo que el usuario decidio hacer con la grabacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Salida {
    Guardar,
    GuardadoRapido,
    Copiar,
    Descartar,
}

/// Donde esta la reproduccion y a que velocidad va.
#[derive(Debug, Clone)]
pub struct Reproductor {
    /// Cuantos fotogramas tiene la grabacion. Nunca cero.
    pub cuantos: usize,
    /// A cuantos por segundo se grabo.
    pub por_segundo: u32,
    /// Sitio en la lista de `VELOCIDADES`.
    pub indice_velocidad: usize,
    /// El fotograma que se ve, con la parte decimal de lo que lleva
    /// mostrado. Se guarda en coma flotante para que una velocidad de un
    /// cuarto avance de verdad en vez de quedarse clavada redondeando.
    pub posicion: f64,
    pub reproduciendo: bool,
}

impl Reproductor {
    pub fn nuevo(cuantos: usize, por_segundo: u32) -> Reproductor {
        Reproductor {
            cuantos: cuantos.max(1),
            por_segundo: por_segundo.max(1),
            indice_velocidad: VELOCIDAD_NORMAL,
            posicion: 0.0,
            // Se abre andando: lo primero que se quiere es ver si salio
            // bien, y tener que pulsar para eso sobra.
            reproduciendo: true,
        }
    }

    pub fn velocidad(&self) -> f32 {
        VELOCIDADES[self.indice_velocidad.min(VELOCIDADES.len() - 1)]
    }

    /// El fotograma que toca ensenar.
    pub fn fotograma(&self) -> usize {
        (self.posicion as usize).min(self.cuantos - 1)
    }

    /// Lo que dura la grabacion entera a velocidad normal.
    pub fn duracion(&self) -> Duration {
        Duration::from_secs_f64(self.cuantos as f64 / self.por_segundo as f64)
    }

    /// Por donde va, de cero a uno, para pintar la linea de tiempo.
    pub fn avance(&self) -> f32 {
        if self.cuantos <= 1 {
            return 0.0;
        }
        (self.fotograma() as f32 / (self.cuantos - 1) as f32).clamp(0.0, 1.0)
    }

    /// Adelanta lo que corresponda al tiempo pasado.
    ///
    /// Da la vuelta al llegar al final en vez de pararse: una grabacion de
    /// tres segundos se entiende mejor viendola repetir que teniendo que
    /// rebobinar a mano cada vez.
    pub fn avanzar(&mut self, transcurrido: Duration) {
        if !self.reproduciendo {
            return;
        }
        let pasos = transcurrido.as_secs_f64() * self.por_segundo as f64 * self.velocidad() as f64;
        self.posicion = (self.posicion + pasos) % self.cuantos as f64;
    }

    /// Salta al fotograma que corresponde a una fraccion de la linea de
    /// tiempo. Cualquier numero vale: se recorta a lo que hay.
    pub fn ir_a(&mut self, fraccion: f32) {
        let f = fraccion.clamp(0.0, 1.0) as f64;
        self.posicion = f * (self.cuantos - 1) as f64;
    }

    /// Un fotograma adelante o atras, dando la vuelta por los dos lados.
    ///
    /// Mover a mano para la reproduccion: si siguiera andando, el fotograma
    /// elegido se escaparia antes de poder mirarlo.
    pub fn paso(&mut self, adelante: bool) {
        self.reproduciendo = false;
        let actual = self.fotograma() as i64;
        let cuantos = self.cuantos as i64;
        let nuevo = (actual + if adelante { 1 } else { -1 }).rem_euclid(cuantos);
        self.posicion = nuevo as f64;
    }

    pub fn siguiente_velocidad(&mut self) {
        self.indice_velocidad = (self.indice_velocidad + 1) % VELOCIDADES.len();
    }
}

/// Lo que mide la ventana del editor para una grabacion de este tamano,
/// sin pasarse de lo que cabe en la pantalla.
///
/// Devuelve `(ancho, alto, escala)`, donde la escala es lo que hay que
/// encoger la imagen. Nunca agranda: una grabacion pequena se ve a su
/// tamano, porque estirarla solo la emborrona.
pub fn medida_ventana(
    imagen_ancho: u32,
    imagen_alto: u32,
    disponible_ancho: u32,
    disponible_alto: u32,
) -> (u32, u32, f32) {
    let margen = 2 * MARGEN;
    let cabe_ancho = disponible_ancho.saturating_sub(margen).max(1) as f32;
    let cabe_alto = disponible_alto.saturating_sub(margen + MANDOS_ALTO).max(1) as f32;
    let escala = (cabe_ancho / imagen_ancho.max(1) as f32)
        .min(cabe_alto / imagen_alto.max(1) as f32)
        .min(1.0);
    let ancho = (imagen_ancho as f32 * escala).round().max(1.0) as u32;
    let alto = (imagen_alto as f32 * escala).round().max(1.0) as u32;
    // La ventana se ensancha si hace falta para que quepan los mandos,
    // aunque la imagen sea un sello. La imagen se queda centrada dentro;
    // estirarla para rellenar el hueco solo la emborronaria.
    (
        (ancho + margen).max(ANCHO_MINIMO),
        alto + margen + MANDOS_ALTO,
        escala,
    )
}

/// Donde cae la linea de tiempo dentro de la ventana, en pixeles logicos.
pub fn linea_tiempo(ventana_ancho: u32, ventana_alto: u32) -> RectF {
    let y = ventana_alto as f32 - MANDOS_ALTO as f32;
    RectF {
        x: MARGEN as f32,
        y: y + 6.0,
        ancho: (ventana_ancho.saturating_sub(2 * MARGEN)) as f32,
        alto: 8.0,
    }
}

/// Los mandos y donde caen, en pixeles logicos de la ventana.
///
/// Los de mirar van a la izquierda y los de decidir a la derecha, pegados
/// al borde: son los que terminan el trabajo, y separarlos evita darle a
/// «Descartar» buscando «Siguiente».
pub fn mandos(ventana_ancho: u32, ventana_alto: u32) -> Vec<(Mando, RectF)> {
    let alto = 24.0;
    let y = ventana_alto as f32 - MANDOS_ALTO as f32 + 18.0;
    let mut fila = Vec::new();
    let poner = |m: Mando, x: f32, ancho: f32, fila: &mut Vec<(Mando, RectF)>| {
        fila.push((m, RectF { x, y, ancho, alto }));
    };
    let mut x = MARGEN as f32;
    for (m, ancho) in [
        (Mando::Anterior, 26.0),
        (Mando::Reproducir, 34.0),
        (Mando::Siguiente, 26.0),
        (Mando::Velocidad, 44.0),
        (Mando::Formato, 48.0),
    ] {
        poner(m, x, ancho, &mut fila);
        x += ancho + 4.0;
    }
    // Los de decidir se colocan de derecha a izquierda para que queden
    // pegados al borde vengan como vengan los anchos.
    let mut derecha = ventana_ancho as f32 - MARGEN as f32;
    for (m, ancho) in [
        (Mando::Descartar, 70.0),
        (Mando::Copiar, 62.0),
        (Mando::GuardadoRapido, 62.0),
        (Mando::Guardar, 68.0),
    ] {
        derecha -= ancho;
        poner(m, derecha, ancho, &mut fila);
        derecha -= 4.0;
    }
    fila
}

/// El mando que cae bajo un punto de la ventana, en pixeles logicos.
pub fn mando_en(ventana_ancho: u32, ventana_alto: u32, x: f32, y: f32) -> Option<Mando> {
    mandos(ventana_ancho, ventana_alto)
        .into_iter()
        .find_map(|(m, r)| {
            (x >= r.x && x < r.x + r.ancho && y >= r.y && y < r.y + r.alto).then_some(m)
        })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn empieza_por_el_principio_y_andando() {
        let r = Reproductor::nuevo(30, 10);
        assert_eq!(r.fotograma(), 0);
        assert!(r.reproduciendo);
        assert_eq!(r.velocidad(), 1.0);
        assert_eq!(r.duracion(), Duration::from_secs(3));
    }

    #[test]
    fn avanza_por_tiempo_y_no_por_vueltas() {
        // Medio segundo a diez por segundo son cinco fotogramas.
        let mut r = Reproductor::nuevo(30, 10);
        r.avanzar(Duration::from_millis(500));
        assert_eq!(r.fotograma(), 5);
        // Y salen los mismos cinco troceando ese medio segundo en cien
        // pasos, que es lo que hace el bucle de verdad. No se pide
        // igualdad exacta: cinco milesimas no son un numero redondo en
        // binario, y cien sumas se quedan una millonesima por debajo. Lo
        // que importa es que la animacion NO va mas lenta por trocear el
        // tiempo, y menos de un fotograma no lo ve nadie.
        let mut otro = Reproductor::nuevo(30, 10);
        for _ in 0..100 {
            otro.avanzar(Duration::from_millis(5));
        }
        assert!(
            otro.fotograma().abs_diff(5) <= 1,
            "troceado salio en {}",
            otro.fotograma()
        );
    }

    #[test]
    fn el_error_de_trocear_no_se_acumula() {
        // Lo anterior seria inutil si el desfase creciera con el rato: a
        // los diez segundos la grabacion iria visiblemente atrasada. Diez
        // mil pasos de una milesima son diez segundos, o sea cien
        // fotogramas, y el desfase tiene que seguir siendo de uno.
        let mut r = Reproductor::nuevo(100_000, 10);
        for _ in 0..10_000 {
            r.avanzar(Duration::from_millis(1));
        }
        assert!(
            r.fotograma().abs_diff(100) <= 1,
            "salio en {}",
            r.fotograma()
        );
    }

    #[test]
    fn la_velocidad_cambia_lo_que_avanza() {
        let mut lento = Reproductor::nuevo(100, 10);
        lento.indice_velocidad = 0;
        lento.avanzar(Duration::from_secs(1));
        assert_eq!(lento.fotograma(), 2, "a un cuarto");
        let mut rapido = Reproductor::nuevo(100, 10);
        rapido.indice_velocidad = 4;
        rapido.avanzar(Duration::from_secs(1));
        assert_eq!(rapido.fotograma(), 40, "al cuadruple");
    }

    #[test]
    fn a_velocidad_lenta_no_se_queda_clavado() {
        // Caso negativo del que guarda la posicion en un entero: a un
        // cuarto, un paso de reloj corto avanza menos de un fotograma, y
        // redondeando a cero la animacion no se moveria NUNCA.
        let mut r = Reproductor::nuevo(100, 10);
        r.indice_velocidad = 0;
        for _ in 0..40 {
            r.avanzar(Duration::from_millis(50));
        }
        assert!(r.fotograma() > 0, "clavado en {}", r.fotograma());
    }

    #[test]
    fn da_la_vuelta_al_llegar_al_final() {
        let mut r = Reproductor::nuevo(10, 10);
        r.avanzar(Duration::from_secs(2));
        assert!(r.fotograma() < 10, "se salio en {}", r.fotograma());
    }

    #[test]
    fn parado_no_avanza() {
        let mut r = Reproductor::nuevo(30, 10);
        r.reproduciendo = false;
        r.avanzar(Duration::from_secs(1));
        assert_eq!(r.fotograma(), 0);
    }

    #[test]
    fn mover_a_mano_para_la_reproduccion() {
        // Si siguiera andando, el fotograma elegido se escaparia antes de
        // poder mirarlo.
        let mut r = Reproductor::nuevo(30, 10);
        r.paso(true);
        assert!(!r.reproduciendo);
        assert_eq!(r.fotograma(), 1);
        r.paso(false);
        assert_eq!(r.fotograma(), 0);
        // Y da la vuelta por los dos lados.
        r.paso(false);
        assert_eq!(r.fotograma(), 29);
    }

    #[test]
    fn la_linea_de_tiempo_lleva_donde_se_pincha() {
        let mut r = Reproductor::nuevo(101, 10);
        r.ir_a(0.0);
        assert_eq!(r.fotograma(), 0);
        assert_eq!(r.avance(), 0.0);
        r.ir_a(0.5);
        assert_eq!(r.fotograma(), 50);
        r.ir_a(1.0);
        assert_eq!(r.fotograma(), 100);
        assert_eq!(r.avance(), 1.0);
        // Pinchar fuera de la barra no puede salirse de la grabacion.
        r.ir_a(-3.0);
        assert_eq!(r.fotograma(), 0);
        r.ir_a(9.0);
        assert_eq!(r.fotograma(), 100);
    }

    #[test]
    fn las_velocidades_dan_la_vuelta() {
        let mut r = Reproductor::nuevo(10, 10);
        for _ in 0..VELOCIDADES.len() {
            r.siguiente_velocidad();
        }
        assert_eq!(r.indice_velocidad, VELOCIDAD_NORMAL);
    }

    #[test]
    fn un_solo_fotograma_no_rompe_la_linea_de_tiempo() {
        // Caso negativo de la division por cero: con un fotograma, el
        // denominador `cuantos - 1` es cero.
        let mut r = Reproductor::nuevo(1, 10);
        assert_eq!(r.avance(), 0.0);
        r.ir_a(1.0);
        assert_eq!(r.fotograma(), 0);
        r.avanzar(Duration::from_secs(5));
        assert_eq!(r.fotograma(), 0);
    }

    #[test]
    fn la_ventana_encoge_lo_grande_y_respeta_lo_pequeno() {
        // Lo que no cabe se encoge...
        let (ancho, alto, escala) = medida_ventana(1920, 1080, 800, 600);
        assert!(escala < 1.0);
        assert!(ancho <= 800, "{ancho}");
        assert!(alto <= 600, "{alto}");
        // ...y lo que cabe se queda como esta, porque estirarlo solo lo
        // emborrona.
        let (_, alto, escala) = medida_ventana(200, 150, 1920, 1080);
        assert_eq!(escala, 1.0);
        assert_eq!(alto, 150 + 2 * MARGEN + MANDOS_ALTO);
        // Pero la ventana nunca baja del ancho minimo, aunque la
        // grabacion sea un sello: por debajo, los mandos se pisan.
        let (ancho, ..) = medida_ventana(40, 30, 1920, 1080);
        assert_eq!(ancho, ANCHO_MINIMO);
    }

    #[test]
    fn los_mandos_caben_y_no_se_pisan() {
        for ancho in [ANCHO_MINIMO, 700, 1200] {
            let alto = 400;
            let fila = mandos(ancho, alto);
            for (m, r) in &fila {
                assert!(r.x >= 0.0, "{m:?} se sale por la izquierda con {ancho}");
                assert!(
                    r.x + r.ancho <= ancho as f32,
                    "{m:?} se sale por la derecha con {ancho}"
                );
                assert!(r.y + r.alto <= alto as f32, "{m:?} se sale por abajo");
            }
            for (i, (m1, r1)) in fila.iter().enumerate() {
                for (m2, r2) in fila.iter().skip(i + 1) {
                    assert!(
                        r1.x + r1.ancho <= r2.x || r2.x + r2.ancho <= r1.x,
                        "{m1:?} pisa a {m2:?} con ancho {ancho}"
                    );
                }
            }
        }
    }

    #[test]
    fn los_formatos_dan_la_vuelta_y_traen_su_extension() {
        let mut f = Formato::Gif;
        assert_eq!(f.extension(), "gif");
        f = f.siguiente();
        assert_eq!(f, Formato::Mp4);
        assert_eq!(f.extension(), "mp4");
        // Dando la vuelta se llega al de partida: si no, el boton dejaria
        // de ofrecer el formato que se acaba de descartar.
        assert_eq!(f.siguiente(), Formato::Gif);
    }

    #[test]
    fn los_de_decidir_quedan_a_la_derecha() {
        // Separar mirar de decidir evita darle a «Descartar» buscando
        // «Siguiente».
        let (ancho, alto) = (700_u32, 400_u32);
        let fila = mandos(ancho, alto);
        let sitio = |m: Mando| {
            fila.iter()
                .find(|(x, _)| *x == m)
                .map(|(_, r)| r.x)
                .expect("mando")
        };
        assert!(sitio(Mando::Siguiente) < sitio(Mando::Guardar));
        assert!(sitio(Mando::Guardar) < sitio(Mando::Descartar));
        assert_eq!(
            mando_en(
                ancho,
                alto,
                sitio(Mando::Descartar) + 2.0,
                alto as f32 - 20.0
            ),
            Some(Mando::Descartar)
        );
    }
}
