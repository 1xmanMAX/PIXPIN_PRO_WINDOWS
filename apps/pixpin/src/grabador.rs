//! La grabacion en dos fases, como la especifica el original (P5b).
//!
//! Lanzar la funcion NO empieza a grabar. Primero sale un marco azul que se
//! puede mover y redimensionar con el raton, y una barra con los ajustes;
//! solo al pulsar «Grabar» el marco se pone rojo y empiezan a contar los
//! fotogramas. Es la diferencia entre elegir la zona y elegir el momento, y
//! es lo que hace utilizable grabar una animacion que tarda en aparecer.
//!
//! El marco no se graba a si mismo: el borde se pinta ENTERO por fuera de
//! la zona, y el centro de la ventana es un hueco que el raton atraviesa
//! (`poner_hueco`), asi que se sigue trabajando con la aplicacion de debajo
//! mientras el marco esta puesto.
//!
//! Aqui esta la geometria pura: donde caen los botones, que se agarra del
//! marco y cuanto se puede grabar de verdad. Todo comprobable sin abrir una
//! ventana, que es lo unico que se puede probar en la maquina de nadie.

use pixpin_codec::ImagenRgba;
use pixpin_geom::{Punto, Rect};
use pixpin_render::RectF;
use pixpin_shell::overlay::FormaCursorWin;
use std::time::Duration;

/// Grosor del borde del marco, en pixeles logicos. Se pinta por fuera de la
/// zona: dentro saldria en el GIF.
pub const GROSOR: i32 = 3;
/// Margen de agarre alrededor de la zona. Es lo que hay que acertar con el
/// raton para mover o redimensionar, asi que no puede ser tan fino como el
/// borde que se ve.
pub const MARGEN: i32 = 8;
/// Lado minimo de la zona. Por debajo de esto no se distingue el contenido
/// del propio marco.
pub const LADO_MINIMO: u32 = 32;

/// Ancho y alto de la barra de control, en pixeles logicos. Es fija para
/// que cambiar de fase no obligue a rehacer la ventana.
pub const BARRA_ANCHO: u32 = 306;
pub const BARRA_ALTO: u32 = 40;

/// Ritmos ofrecidos, en fotogramas por segundo. Diez es el de partida: se
/// ve fluido para ensenar una interfaz y no dispara el fichero.
pub const RITMOS: [u32; 6] = [5, 10, 15, 20, 25, 30];
pub const RITMO_POR_DEFECTO: usize = 1;

/// Tope de reloj. Mas largo que esto no es un GIF, es un video.
pub const TIEMPO_MAXIMO: Duration = Duration::from_secs(12);
/// Tope de los fotogramas en crudo, antes de comprimir.
pub const MEMORIA_MAXIMA: usize = 256 * 1024 * 1024;

/// En que punto esta la sesion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fase {
    /// Marco azul: eligiendo zona y ajustes, sin grabar todavia.
    Esperando,
    /// Marco rojo: contando fotogramas.
    Grabando,
    /// Marco rojo apagado: el reloj y la captura estan detenidos.
    Pausada,
}

/// Por que termino la grabacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fin {
    /// Pulso «Parar», Intro, o el mismo atajo por segunda vez.
    Usuario,
    Escape,
    Tiempo,
    Memoria,
}

pub struct Grabacion {
    pub fotogramas: Vec<ImagenRgba>,
    pub fin: Fin,
    /// Fotogramas por segundo a los que se grabo de verdad, para declarar
    /// el ritmo correcto en el fichero.
    pub por_segundo: u32,
}

impl Grabacion {
    /// Las centesimas de segundo que hay que declarar en el GIF. Se calcula
    /// y no se escribe a mano para que cambiar el ritmo no deje el fichero
    /// yendo a otra velocidad.
    pub fn centesimas_por_fotograma(&self) -> u16 {
        (100 / self.por_segundo.max(1)).max(1) as u16
    }
}

/// Los botones de la barra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boton {
    Grabar,
    Pausar,
    Parar,
    MenosRitmo,
    MasRitmo,
    Cerrar,
}

/// Lo que se puede agarrar del marco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asa {
    Mover,
    Norte,
    Sur,
    Este,
    Oeste,
    NorEste,
    NorOeste,
    SurEste,
    SurOeste,
}

impl Asa {
    /// El cursor que anuncia lo que va a pasar si se arrastra.
    pub fn cursor(self) -> FormaCursorWin {
        match self {
            Asa::Mover => FormaCursorWin::Mover,
            Asa::Norte | Asa::Sur => FormaCursorWin::RedimNS,
            Asa::Este | Asa::Oeste => FormaCursorWin::RedimEO,
            Asa::NorOeste | Asa::SurEste => FormaCursorWin::RedimNoSe,
            Asa::NorEste | Asa::SurOeste => FormaCursorWin::RedimNeSo,
        }
    }
}

/// El rectangulo de la ventana del marco: la zona con su margen de agarre.
pub fn marco_de(zona: Rect) -> Rect {
    Rect {
        x: zona.x - MARGEN,
        y: zona.y - MARGEN,
        ancho: zona.ancho + 2 * MARGEN as u32,
        alto: zona.alto + 2 * MARGEN as u32,
    }
}

/// Que parte del marco cae bajo el punto, en coordenadas del escritorio.
///
/// `None` significa dentro de la zona o fuera del marco: ahi no hay nada
/// que agarrar, y en el centro el raton lo atraviesa.
pub fn asa_en(zona: Rect, punto: Punto) -> Option<Asa> {
    if !marco_de(zona).contiene(punto) || zona.contiene(punto) {
        return None;
    }
    // Las esquinas mandan sobre los lados: se prueban primero, y su area es
    // un cuadrado de lado 3*MARGEN para acertarlas sin puntear fino.
    let esquina = 3 * MARGEN;
    let oeste = punto.x < zona.x + esquina;
    let este = punto.x > zona.derecha() - esquina;
    let norte = punto.y < zona.y + esquina;
    let sur = punto.y > zona.abajo() - esquina;
    Some(match (norte, sur, oeste, este) {
        (true, _, true, _) => Asa::NorOeste,
        (true, _, _, true) => Asa::NorEste,
        (_, true, true, _) => Asa::SurOeste,
        (_, true, _, true) => Asa::SurEste,
        (true, ..) => Asa::Norte,
        (_, true, ..) => Asa::Sur,
        (_, _, true, _) => Asa::Oeste,
        (_, _, _, true) => Asa::Este,
        // El resto del anillo mueve la zona entera.
        _ => Asa::Mover,
    })
}

/// La zona que resulta de arrastrar `asa` un desplazamiento `(dx, dy)`.
///
/// Nunca devuelve algo mas pequeno que `LADO_MINIMO`: al llegar al tope el
/// lado deja de seguir al raton en vez de darse la vuelta, que es lo que
/// pasaria restando a secas.
pub fn aplicar_asa(zona: Rect, asa: Asa, dx: i32, dy: i32) -> Rect {
    if asa == Asa::Mover {
        return Rect {
            x: zona.x + dx,
            y: zona.y + dy,
            ..zona
        };
    }
    let minimo = LADO_MINIMO as i32;
    let (mut i, mut a, mut d, mut b) = (
        zona.izquierda(),
        zona.arriba(),
        zona.derecha(),
        zona.abajo(),
    );
    if matches!(asa, Asa::Oeste | Asa::NorOeste | Asa::SurOeste) {
        i = (i + dx).min(d - minimo);
    }
    if matches!(asa, Asa::Este | Asa::NorEste | Asa::SurEste) {
        d = (d + dx).max(i + minimo);
    }
    if matches!(asa, Asa::Norte | Asa::NorEste | Asa::NorOeste) {
        a = (a + dy).min(b - minimo);
    }
    if matches!(asa, Asa::Sur | Asa::SurEste | Asa::SurOeste) {
        b = (b + dy).max(a + minimo);
    }
    Rect {
        x: i,
        y: a,
        ancho: (d - i) as u32,
        alto: (b - a) as u32,
    }
}

/// Donde va el rotulo del ritmo, entre los dos botones que lo cambian.
pub const RITMO_HUECO: f32 = 46.0;

/// Los botones de la barra y donde caen, en coordenadas locales de la barra
/// y en pixeles logicos.
///
/// Es una funcion pura y no un metodo para poder comprobar con pruebas que
/// los botones no se salen ni se pisan, que es el fallo tipico al mover un
/// rotulo de sitio.
pub fn botones(fase: Fase) -> Vec<(Boton, RectF)> {
    let alto = 28.0;
    let y = (BARRA_ALTO as f32 - alto) / 2.0;
    let mut x = 9.0;
    let mut fila = Vec::new();
    let mut poner = |b: Boton, ancho: f32, x: &mut f32| {
        fila.push((
            b,
            RectF {
                x: *x,
                y,
                ancho,
                alto,
            },
        ));
        *x += ancho + 6.0;
    };
    match fase {
        Fase::Esperando => {
            poner(Boton::Grabar, 94.0, &mut x);
            poner(Boton::MenosRitmo, 28.0, &mut x);
            // El hueco del rotulo «10/s» va aqui: se salta sin boton.
            x += RITMO_HUECO;
            poner(Boton::MasRitmo, 28.0, &mut x);
            poner(Boton::Cerrar, 72.0, &mut x);
        }
        Fase::Grabando | Fase::Pausada => {
            poner(Boton::Pausar, 84.0, &mut x);
            poner(Boton::Parar, 84.0, &mut x);
        }
    }
    fila
}

/// El boton que cae bajo un punto local de la barra.
pub fn boton_en(fase: Fase, x: f32, y: f32) -> Option<Boton> {
    botones(fase).into_iter().find_map(|(b, r)| {
        (x >= r.x && x < r.x + r.ancho && y >= r.y && y < r.y + r.alto).then_some(b)
    })
}

/// Cuantos segundos se pueden grabar de verdad a este ritmo y este tamano.
///
/// El tope de reloj por si solo seria mentira: un fotograma es RGBA sin
/// comprimir, asi que a treinta por segundo una zona grande llena la
/// memoria mucho antes de los doce segundos. Vale mas ensenar el numero
/// verdadero en la barra que cortar por sorpresa.
pub fn tope_segundos(zona: Rect, por_segundo: u32) -> u64 {
    let bytes = zona.ancho as usize * zona.alto as usize * 4;
    let por_reloj = TIEMPO_MAXIMO.as_secs();
    if bytes == 0 || por_segundo == 0 {
        return por_reloj;
    }
    let caben = MEMORIA_MAXIMA / bytes;
    let por_memoria = (caben / por_segundo as usize) as u64;
    por_memoria.min(por_reloj).max(1)
}

/// Un tiempo en `m:ss`, que es como se lee de un vistazo.
pub fn reloj(segundos: u64) -> String {
    format!("{}:{:02}", segundos / 60, segundos % 60)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn zona() -> Rect {
        Rect {
            x: 100,
            y: 100,
            ancho: 400,
            alto: 300,
        }
    }

    #[test]
    fn el_centro_de_la_zona_no_agarra_nada() {
        // Si esto fallara, el marco secuestraria los clics de la aplicacion
        // que se esta grabando, que es justo lo que hay que evitar.
        assert_eq!(asa_en(zona(), Punto { x: 300, y: 250 }), None);
        assert_eq!(asa_en(zona(), Punto { x: 0, y: 0 }), None);
    }

    #[test]
    fn cada_borde_da_su_asa_y_cada_esquina_la_suya() {
        let z = zona();
        assert_eq!(asa_en(z, Punto { x: 96, y: 96 }), Some(Asa::NorOeste));
        assert_eq!(asa_en(z, Punto { x: 504, y: 96 }), Some(Asa::NorEste));
        assert_eq!(asa_en(z, Punto { x: 96, y: 404 }), Some(Asa::SurOeste));
        assert_eq!(asa_en(z, Punto { x: 504, y: 404 }), Some(Asa::SurEste));
        assert_eq!(asa_en(z, Punto { x: 300, y: 96 }), Some(Asa::Norte));
        assert_eq!(asa_en(z, Punto { x: 300, y: 404 }), Some(Asa::Sur));
        assert_eq!(asa_en(z, Punto { x: 96, y: 250 }), Some(Asa::Oeste));
        assert_eq!(asa_en(z, Punto { x: 504, y: 250 }), Some(Asa::Este));
    }

    #[test]
    fn mover_no_cambia_el_tamano() {
        let z = aplicar_asa(zona(), Asa::Mover, 40, -25);
        assert_eq!((z.x, z.y), (140, 75));
        assert_eq!((z.ancho, z.alto), (400, 300));
    }

    #[test]
    fn arrastrar_una_esquina_mueve_solo_sus_dos_lados() {
        let z = aplicar_asa(zona(), Asa::NorOeste, 10, 20);
        // La esquina contraria se queda donde estaba.
        assert_eq!((z.derecha(), z.abajo()), (500, 400));
        assert_eq!((z.x, z.y), (110, 120));
    }

    #[test]
    fn la_zona_nunca_se_da_la_vuelta() {
        // Caso negativo: arrastrar el lado oeste mil pixeles a la derecha
        // no puede dar un ancho negativo ni cruzar el lado este.
        let z = aplicar_asa(zona(), Asa::Oeste, 1000, 0);
        assert_eq!(z.ancho, LADO_MINIMO);
        assert_eq!(z.derecha(), 500);
        let z = aplicar_asa(zona(), Asa::Sur, 0, -1000);
        assert_eq!(z.alto, LADO_MINIMO);
        assert_eq!(z.arriba(), 100);
    }

    #[test]
    fn los_botones_caben_en_la_barra_y_no_se_pisan() {
        for fase in [Fase::Esperando, Fase::Grabando, Fase::Pausada] {
            let fila = botones(fase);
            assert!(!fila.is_empty(), "{fase:?} sin botones");
            for (b, r) in &fila {
                assert!(r.x >= 0.0, "{b:?} se sale por la izquierda");
                assert!(
                    r.x + r.ancho <= BARRA_ANCHO as f32,
                    "{b:?} se sale por la derecha: {}",
                    r.x + r.ancho
                );
                assert!(r.y + r.alto <= BARRA_ALTO as f32, "{b:?} se sale por abajo");
            }
            for (i, (b1, r1)) in fila.iter().enumerate() {
                for (b2, r2) in fila.iter().skip(i + 1) {
                    assert!(
                        r1.x + r1.ancho <= r2.x || r2.x + r2.ancho <= r1.x,
                        "{b1:?} pisa a {b2:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn se_acierta_el_boton_que_se_ve() {
        let (_, r) = botones(Fase::Esperando)[0];
        let dentro = boton_en(Fase::Esperando, r.x + r.ancho / 2.0, r.y + r.alto / 2.0);
        assert_eq!(dentro, Some(Boton::Grabar));
        // Justo encima de la fila no hay boton: la barra tiene margen.
        assert_eq!(boton_en(Fase::Esperando, r.x + 1.0, 1.0), None);
        // Y en la fase de grabar, el mismo sitio ya es otra cosa.
        assert_eq!(
            boton_en(Fase::Grabando, r.x + 1.0, r.y + 1.0),
            Some(Boton::Pausar)
        );
    }

    #[test]
    fn el_tope_que_se_ensena_es_el_que_se_cumple() {
        // Una zona pequena llega al tope de reloj.
        let chica = Rect {
            x: 0,
            y: 0,
            ancho: 320,
            alto: 240,
        };
        assert_eq!(tope_segundos(chica, 10), TIEMPO_MAXIMO.as_secs());
        // Una zona grande a mucho ritmo no: manda la memoria, y el numero
        // que sale en la barra tiene que ser ese y no los doce segundos.
        let grande = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        };
        let tope = tope_segundos(grande, 30);
        assert!(tope < TIEMPO_MAXIMO.as_secs(), "salio {tope}");
        let bytes = 1920usize * 1080 * 4;
        assert!(tope as usize * 30 * bytes <= MEMORIA_MAXIMA);
    }

    #[test]
    fn el_ritmo_declarado_cuadra_con_el_de_grabacion() {
        for por_segundo in RITMOS {
            let g = Grabacion {
                fotogramas: Vec::new(),
                fin: Fin::Usuario,
                por_segundo,
            };
            let centesimas = g.centesimas_por_fotograma() as u32;
            // El GIF cuenta en centesimas enteras, asi que no todo ritmo es
            // representable; lo que no puede pasar es que se aleje mas de
            // una centesima del pedido.
            let error = (centesimas as i64 - (100 / por_segundo) as i64).abs();
            assert!(error <= 1, "{por_segundo}/s declara {centesimas} cs");
        }
    }

    #[test]
    fn el_reloj_se_lee_de_un_vistazo() {
        assert_eq!(reloj(0), "0:00");
        assert_eq!(reloj(9), "0:09");
        assert_eq!(reloj(75), "1:15");
    }
}
