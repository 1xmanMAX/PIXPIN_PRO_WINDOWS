//! **Juntar dos fuentes en una nota** (24-oct-2026, grabar clases y
//! reuniones): el microfono y el sonido del equipo, cada uno con su reloj.
//!
//! Lo que llega de cada tarjeta ya viene en mono, de 16 bits y al muestreo
//! de la grabacion (`mezcla`); aqui solo se reparte en el tiempo:
//!
//! - **Se suma muestra a muestra** lo que hay de las dos, recortado al
//!   maximo de 16 bits (dos voces altas a la vez no dan la vuelta y suenan a
//!   chasquido).
//! - **Una fuente callada no para a la otra.** Windows no entrega nada del
//!   sonido del equipo mientras no suena nada (`AUDCLNT_STREAMFLAGS_LOOPBACK`
//!   calla, no manda ceros): si una cola se adelanta mas de [`HOLGURA_MS`]
//!   a las demas, lo que les falta se toma por silencio.
//! - **La nota sigue al reloj de la pared.** Grabando solo el sonido del
//!   equipo, un minuto sin sonido no daria muestras y la nota se encogeria:
//!   las capturas hechas despues caerian antes de lo que se dijo con ellas.
//!   Con [`Mezclador::alinear`], lo que falte hasta lo que marca el reloj se
//!   rellena con silencio.
//!
//! Puro, sin Windows: se prueba entero con numeros.

use std::collections::VecDeque;

/// Cuanto puede adelantarse una fuente a otra antes de que a la otra se le
/// ponga silencio. Un cuarto de segundo: menos y un tiron del sistema
/// meteria huecos en una voz que solo llegaba tarde; mas y la otra se oiria
/// desplazada.
pub const HOLGURA_MS: u32 = 250;

/// Las colas de cada fuente, ya al muestreo de la grabacion.
#[derive(Debug, Clone)]
pub struct Mezclador {
    colas: Vec<VecDeque<i16>>,
    holgura: usize,
}

impl Mezclador {
    /// Para `fuentes` fuentes (una o dos) a `muestreo` hercios.
    pub fn nuevo(fuentes: usize, muestreo: u32) -> Mezclador {
        Mezclador {
            colas: vec![VecDeque::new(); fuentes.max(1)],
            holgura: (muestreo as usize * HOLGURA_MS as usize / 1000).max(1),
        }
    }

    /// Lo que acaba de llegar de la fuente `cual`.
    pub fn empuja(&mut self, cual: usize, muestras: &[i16]) {
        if let Some(c) = self.colas.get_mut(cual) {
            c.extend(muestras.iter().copied());
        }
    }

    /// Lo que espera en la cola mas larga.
    pub fn en_espera(&self) -> usize {
        self.colas.iter().map(VecDeque::len).max().unwrap_or(0)
    }

    /// **Al paso del reloj**: si con lo ya escrito (`escritas`) y lo que
    /// espera no se llega a las `esperadas` que marca el reloj, menos la
    /// holgura, lo que falta es silencio en todas las fuentes.
    pub fn alinear(&mut self, escritas: u64, esperadas: u64) {
        let larga = self.en_espera();
        let hay = escritas + larga as u64;
        if esperadas > hay + self.holgura as u64 {
            // Todas hasta la mas larga y lo que falta: lo que no llego de
            // ninguna, a estas alturas, es que no sono.
            let hasta = larga + (esperadas - hay) as usize;
            for c in &mut self.colas {
                c.resize(hasta, 0);
            }
        }
    }

    /// **Lo que ya se puede escribir**: lo que tienen todas las fuentes, y
    /// si una se adelanto mas de la holgura, tambien eso, con silencio en
    /// las que no llegaron.
    pub fn sacar(&mut self) -> Vec<i16> {
        let corta = self.colas.iter().map(VecDeque::len).min().unwrap_or(0);
        let larga = self.en_espera();
        let n = if larga > corta + self.holgura {
            larga - self.holgura / 2
        } else {
            corta
        };
        self.mezclar(n)
    }

    /// **Al parar**: todo lo que quede, con silencio en las que se quedaron
    /// cortas.
    pub fn resto(&mut self) -> Vec<i16> {
        let n = self.en_espera();
        self.mezclar(n)
    }

    /// En pausa: lo que llega se tira.
    pub fn vaciar(&mut self) {
        for c in &mut self.colas {
            c.clear();
        }
    }

    fn mezclar(&mut self, n: usize) -> Vec<i16> {
        let mut suma = vec![0i32; n];
        for c in &mut self.colas {
            let toma = n.min(c.len());
            for (s, m) in suma.iter_mut().zip(c.drain(..toma)) {
                *s += i32::from(m);
            }
        }
        suma.into_iter()
            .map(|s| s.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16)
            .collect()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const FS: u32 = 48_000;
    const CUARTO: usize = FS as usize / 4;

    #[test]
    fn con_las_dos_a_la_par_se_suman_muestra_a_muestra() {
        let mut m = Mezclador::nuevo(2, FS);
        m.empuja(0, &[100, 200, 300]);
        m.empuja(1, &[10, 20]);
        assert_eq!(m.sacar(), vec![110, 220], "solo lo que tienen las dos");
        m.empuja(1, &[30]);
        assert_eq!(m.sacar(), vec![330]);
        assert_eq!(m.en_espera(), 0);
    }

    #[test]
    fn dos_voces_altas_se_recortan_y_no_dan_la_vuelta() {
        let mut m = Mezclador::nuevo(2, FS);
        m.empuja(0, &[30_000, -30_000]);
        m.empuja(1, &[30_000, -30_000]);
        assert_eq!(m.sacar(), vec![i16::MAX, i16::MIN]);
    }

    #[test]
    fn el_sonido_del_equipo_callado_no_para_al_microfono() {
        // Windows no manda nada del equipo mientras no suena: pasada la
        // holgura, el microfono sale solo y el equipo cuenta como silencio.
        let mut m = Mezclador::nuevo(2, FS);
        m.empuja(0, &vec![7; CUARTO / 2]);
        assert!(
            m.sacar().is_empty(),
            "medio cuarto de segundo: aun se espera"
        );
        m.empuja(0, &vec![7; CUARTO]);
        let salio = m.sacar();
        assert!(
            !salio.is_empty() && salio.iter().all(|&v| v == 7),
            "el microfono, con silencio debajo"
        );
        // Caso negativo: no se lo come todo; deja algo por si el equipo
        // solo llegaba tarde.
        assert!(m.en_espera() > 0 && m.en_espera() < CUARTO);
    }

    #[test]
    fn la_nota_sigue_al_reloj_aunque_no_llegue_nada() {
        // Solo el sonido del equipo y nada sonando: un segundo de reloj es un
        // segundo de silencio en la nota.
        let mut m = Mezclador::nuevo(1, FS);
        m.alinear(0, FS as u64);
        let salio = m.sacar();
        assert_eq!(salio.len(), FS as usize);
        assert!(salio.iter().all(|&v| v == 0));
        // Caso negativo: dentro de la holgura no se mete nada (lo normal es
        // que el sonido este en camino).
        let mut m = Mezclador::nuevo(1, FS);
        m.alinear(0, (CUARTO / 2) as u64);
        assert_eq!(m.en_espera(), 0);
    }

    #[test]
    fn al_parar_sale_todo_y_en_pausa_se_tira() {
        let mut m = Mezclador::nuevo(2, FS);
        m.empuja(0, &[1, 2, 3]);
        m.empuja(1, &[1]);
        assert_eq!(m.resto(), vec![2, 2, 3]);
        m.empuja(0, &[5; 10]);
        m.vaciar();
        assert_eq!(m.en_espera(), 0);
        assert!(m.resto().is_empty());
    }
}
