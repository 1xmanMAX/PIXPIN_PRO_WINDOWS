//! Donde caen los nueve puntos de agarre y cual esta bajo el cursor.
//!
//! # Viven en el mundo, miden en pantalla
//!
//! Un tirador tiene ocho pixeles siempre: al 500 % no se convierte en un
//! ladrillo y al 20 % no desaparece. En unidades del mundo eso es
//! `LADO * escala`, donde `escala` es —como en todo este motor— unidades de
//! mundo por pixel de pantalla, o sea `1.0 / zoom`.
//!
//! # Por que la zona de picado es mas grande que el dibujo
//!
//! Diez pixeles contra ocho. Nadie acierta un cuadradito de ocho pixeles al
//! primer intento, y fallar un tirador no es un fallo pequeno: el clic cae
//! en el elemento de debajo y lo que el usuario queria redimensionar acaba
//! movido.

use pixpin_geom::Tirador;

use crate::elemento::{ColorRgba, Elemento, EstiloTrazo};
use crate::pintado::Orden;
use crate::vector::Punto2;

/// Lado del cuadradito que se dibuja, en pixeles de pantalla.
pub const LADO: f32 = 8.0;

/// Radio de la zona que responde al cursor, en pixeles de pantalla.
pub const ZONA: f32 = 10.0;

/// Cuanto se separa el tirador de giro del borde de arriba, en pixeles.
pub const SEPARACION_GIRO: f32 = 24.0;

const RELLENO: ColorRgba = ColorRgba {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};
const BORDE: ColorRgba = ColorRgba {
    r: 0.36,
    g: 0.42,
    b: 0.95,
    a: 1.0,
};

/// De que se ha agarrado el cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agarre {
    Tamano(Tirador),
    Giro,
}

/// Los nueve puntos de agarre, ya en coordenadas del mundo.
#[derive(Debug, Clone, PartialEq)]
pub struct Tiradores {
    pub tamano: [(Tirador, Punto2); 8],
    pub giro: Punto2,
    pub centro: Punto2,
    pub angulo: f32,
}

impl Tiradores {
    /// Los tiradores de una caja paralela a los ejes, girada `angulo`
    /// alrededor de su centro.
    pub fn de_caja(caja: (f32, f32, f32, f32), angulo: f32, escala: f32) -> Self {
        let (x0, y0, x1, y1) = caja;
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (mx, my) = (centro.x, centro.y);

        // Sin girar primero, y luego se gira todo de una vez: es una
        // operacion por punto en vez de una formula distinta por esquina.
        let sitios = [
            (Tirador::NoroesteEsquina, Punto2::nuevo(x0, y0)),
            (Tirador::NorteBorde, Punto2::nuevo(mx, y0)),
            (Tirador::NoresteEsquina, Punto2::nuevo(x1, y0)),
            (Tirador::EsteBorde, Punto2::nuevo(x1, my)),
            (Tirador::SuresteEsquina, Punto2::nuevo(x1, y1)),
            (Tirador::SurBorde, Punto2::nuevo(mx, y1)),
            (Tirador::SuroesteEsquina, Punto2::nuevo(x0, y1)),
            (Tirador::OesteBorde, Punto2::nuevo(x0, my)),
        ];
        let tamano = sitios.map(|(c, p)| (c, p.girar(centro, angulo)));

        // El de giro, separado por encima del borde norte. La separacion va
        // en pixeles de pantalla, como todo lo demas de aqui.
        let giro = Punto2::nuevo(mx, y0 - SEPARACION_GIRO * escala).girar(centro, angulo);

        Self {
            tamano,
            giro,
            centro,
            angulo,
        }
    }

    /// Los tiradores de un elemento.
    pub fn de_elemento(e: &Elemento, escala: f32) -> Self {
        Self::de_caja(e.caja(), e.angulo, escala)
    }

    /// De que hay agarre bajo el punto, si de alguno.
    ///
    /// El de giro se mira primero: con una caja muy pequena puede caer
    /// encima del borde norte, y girar es el gesto mas dificil de acertar
    /// de los dos.
    pub fn en(&self, p: Punto2, escala: f32) -> Option<Agarre> {
        let radio = ZONA * escala;
        if p.distancia(self.giro) <= radio {
            return Some(Agarre::Giro);
        }
        self.tamano
            .iter()
            .find(|(_, q)| p.distancia(*q) <= radio)
            .map(|(c, _)| Agarre::Tamano(*c))
    }

    /// Como se pintan: un cuadradito blanco con borde por cada uno.
    ///
    /// Girados con el elemento, para que en una figura a 45 grados los
    /// cuadraditos vayan a 45 grados y no de canto.
    pub fn ordenes(&self, escala: f32) -> Vec<Orden> {
        let mitad = LADO * escala / 2.0;
        let mut fuera = Vec::with_capacity(9);
        let sitios = self.tamano.iter().map(|(_, p)| p).chain([&self.giro]);
        for p in sitios {
            let esquinas = [
                Punto2::nuevo(p.x - mitad, p.y - mitad),
                Punto2::nuevo(p.x + mitad, p.y - mitad),
                Punto2::nuevo(p.x + mitad, p.y + mitad),
                Punto2::nuevo(p.x - mitad, p.y + mitad),
            ];
            let puntos: Vec<Punto2> = esquinas.iter().map(|q| q.girar(*p, self.angulo)).collect();
            fuera.push(Orden::Relleno {
                puntos: puntos.clone(),
                color: RELLENO,
            });
            let mut cerrado = puntos;
            cerrado.push(cerrado[0]);
            fuera.push(Orden::Polilinea {
                puntos: cerrado,
                color: BORDE,
                grosor: (1.0 * escala).max(0.5),
                estilo: EstiloTrazo::Solido,
            });
        }
        fuera
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    /// Una caja de 100x50 en el origen.
    const CAJA: (f32, f32, f32, f32) = (0.0, 0.0, 100.0, 50.0);

    fn busca(t: &Tiradores, cual: Tirador) -> Punto2 {
        t.tamano.iter().find(|(c, _)| *c == cual).unwrap().1
    }

    fn cerca(a: Punto2, b: Punto2, que: &str) {
        assert!(
            (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3,
            "{que}: esperaba ({}, {}), es ({}, {})",
            b.x,
            b.y,
            a.x,
            a.y
        );
    }

    #[test]
    fn los_ocho_caen_en_las_esquinas_y_en_los_medios() {
        let t = Tiradores::de_caja(CAJA, 0.0, 1.0);
        cerca(
            busca(&t, Tirador::NoroesteEsquina),
            Punto2::nuevo(0.0, 0.0),
            "NO",
        );
        cerca(
            busca(&t, Tirador::NorteBorde),
            Punto2::nuevo(50.0, 0.0),
            "N",
        );
        cerca(
            busca(&t, Tirador::NoresteEsquina),
            Punto2::nuevo(100.0, 0.0),
            "NE",
        );
        cerca(
            busca(&t, Tirador::EsteBorde),
            Punto2::nuevo(100.0, 25.0),
            "E",
        );
        cerca(
            busca(&t, Tirador::SuresteEsquina),
            Punto2::nuevo(100.0, 50.0),
            "SE",
        );
        cerca(busca(&t, Tirador::SurBorde), Punto2::nuevo(50.0, 50.0), "S");
        cerca(
            busca(&t, Tirador::SuroesteEsquina),
            Punto2::nuevo(0.0, 50.0),
            "SO",
        );
        cerca(
            busca(&t, Tirador::OesteBorde),
            Punto2::nuevo(0.0, 25.0),
            "O",
        );
    }

    #[test]
    fn con_la_caja_girada_los_tiradores_giran_con_ella() {
        let t = Tiradores::de_caja(CAJA, FRAC_PI_2, 1.0);
        let centro = Punto2::nuevo(50.0, 25.0);
        // La esquina noroeste, girada un cuarto de vuelta sobre el centro.
        cerca(
            busca(&t, Tirador::NoroesteEsquina),
            Punto2::nuevo(0.0, 0.0).girar(centro, FRAC_PI_2),
            "NO girada",
        );
    }

    #[test]
    fn el_de_giro_queda_separado_por_encima_y_gira_tambien() {
        // Encima del borde norte, a SEPARACION_GIRO pixeles de pantalla.
        let t = Tiradores::de_caja(CAJA, 0.0, 1.0);
        cerca(t.giro, Punto2::nuevo(50.0, -SEPARACION_GIRO), "sin girar");

        let g = Tiradores::de_caja(CAJA, FRAC_PI_2, 1.0);
        let centro = Punto2::nuevo(50.0, 25.0);
        cerca(
            g.giro,
            Punto2::nuevo(50.0, -SEPARACION_GIRO).girar(centro, FRAC_PI_2),
            "girado",
        );
    }

    #[test]
    fn la_separacion_del_giro_se_mide_en_pixeles_de_pantalla() {
        // Al 20 % de aumento (escala 5.0) el tirador se separa cinco veces
        // mas en el mundo, para verse igual de separado en la pantalla.
        let t = Tiradores::de_caja(CAJA, 0.0, 5.0);
        cerca(
            t.giro,
            Punto2::nuevo(50.0, -SEPARACION_GIRO * 5.0),
            "al 20 %",
        );
    }

    #[test]
    fn picar_un_tirador_lo_encuentra_por_su_zona_y_no_por_su_dibujo() {
        let t = Tiradores::de_caja(CAJA, 0.0, 1.0);

        // Justo encima: lo encuentra.
        assert_eq!(
            t.en(Punto2::nuevo(100.0, 50.0), 1.0),
            Some(Agarre::Tamano(Tirador::SuresteEsquina))
        );
        // A nueve pixeles: dentro de la zona de diez, aunque el dibujo mida
        // ocho. Nadie acierta un cuadradito de ocho pixeles al primer
        // intento.
        assert_eq!(
            t.en(Punto2::nuevo(100.0 + 9.0, 50.0), 1.0),
            Some(Agarre::Tamano(Tirador::SuresteEsquina))
        );
        // A veinte: fuera.
        assert_eq!(t.en(Punto2::nuevo(100.0 + 20.0, 50.0), 1.0), None);
    }

    #[test]
    fn la_zona_de_picado_tambien_se_mide_en_pixeles_de_pantalla() {
        // Al 20 % (escala 5.0), diez pixeles de pantalla son cincuenta del
        // mundo. Si no, al alejarse los tiradores serian inalcanzables.
        let t = Tiradores::de_caja(CAJA, 0.0, 5.0);
        assert_eq!(
            t.en(Punto2::nuevo(100.0 + 45.0, 50.0), 5.0),
            Some(Agarre::Tamano(Tirador::SuresteEsquina))
        );
    }

    #[test]
    fn el_de_giro_manda_sobre_el_de_tamano_si_se_solapan() {
        // Con una caja muy pequena, el de giro puede caer encima del borde
        // norte. Girar es el gesto mas dificil de acertar de los dos, asi
        // que gana.
        let minuscula = (0.0, 0.0, 2.0, 2.0);
        let t = Tiradores::de_caja(minuscula, 0.0, 1.0);
        assert_eq!(t.en(t.giro, 1.0), Some(Agarre::Giro));
    }

    #[test]
    fn picar_lejos_de_todo_no_encuentra_nada() {
        let t = Tiradores::de_caja(CAJA, 0.0, 1.0);
        assert_eq!(t.en(Punto2::nuevo(500.0, 500.0), 1.0), None);
    }

    #[test]
    fn cada_tirador_se_pinta_con_relleno_y_borde() {
        // Nueve tiradores por dos ordenes cada uno. El borde no es adorno:
        // un cuadrado blanco sin el es invisible sobre fondo claro.
        let t = Tiradores::de_caja(CAJA, 0.0, 1.0);
        assert_eq!(t.ordenes(1.0).len(), 18);
    }

    #[test]
    fn los_cuadraditos_giran_con_el_elemento() {
        // En una figura a 45 grados, los cuadraditos van a 45 grados y no
        // de canto. Cuesta una linea y es de lo que separa un editor que
        // se siente bien de uno que no.
        let t = Tiradores::de_caja(CAJA, FRAC_PI_2, 1.0);
        let Orden::Relleno { puntos, .. } = &t.ordenes(1.0)[0] else {
            panic!("la primera orden es el relleno del primer tirador");
        };
        let lado = puntos[0].distancia(puntos[1]);
        assert!((lado - LADO).abs() < 1e-3, "sigue siendo cuadrado: {lado}");
        // Girado un cuarto de vuelta, el lado que iba en x ahora va en y.
        assert!(
            (puntos[0].x - puntos[1].x).abs() < 1e-3,
            "el primer lado quedo vertical"
        );
    }
}
