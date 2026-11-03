//! **El boton ☀/☾ redondo** de las cabeceras (2-nov-2026, el usuario: «el
//! boton de dia/noche deberia ser mas como el del telefono»): un circulo con
//! filete, arriba a la derecha, con el sol de dia y la luna de noche. Dice
//! el modo PUESTO (como el del movil), no al que se pasaria.
//!
//! Aqui solo las cuentas: donde va, cuanto mide y como se cambia el icono
//! (el viejo gira y se apaga mientras el nuevo entra girando). Lo pinta
//! `boton_dia_noche` de la app, el mismo en todas las ventanas.

use pindesk_geom::Rect;

/// Lo que mide de lado (logicos), el del movil.
pub const LADO: u32 = 36;
/// El icono de dentro (logicos).
pub const ICONO: u32 = 20;
/// Lo que dura el cambio de icono.
pub const DURA_MS: u64 = 260;
/// Lo que gira el icono al entrar o salir (radianes, un cuarto de vuelta).
pub const GIRO: f32 = std::f32::consts::FRAC_PI_2;

/// El boton redondo con su borde derecho en `derecha` y centrado en
/// `centro_y`.
pub fn caja(derecha: i32, centro_y: i32, escala: u32) -> Rect {
    let lado = LADO * escala / 100;
    Rect {
        x: derecha - lado as i32,
        y: centro_y - lado as i32 / 2,
        ancho: lado,
        alto: lado,
    }
}

/// El modo puesto y, si acaba de cambiar, cuando (ms de un reloj que no
/// salta hacia atras).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiaNoche {
    pub noche: bool,
    desde_ms: Option<u64>,
}

/// Como va el cambio en un momento: el icono que se va y el que llega, cada
/// uno con lo que se ve (0..1), lo que mide (0..1) y lo girado (radianes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paso {
    pub sale: Option<Trazo>,
    pub entra: Trazo,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trazo {
    /// `true`: la luna; `false`: el sol.
    pub luna: bool,
    pub alfa: f32,
    pub tam: f32,
    pub giro: f32,
}

/// La curva del cambio: sale rapido y frena al final.
fn suave(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t)
}

impl DiaNoche {
    pub fn nuevo(noche: bool) -> DiaNoche {
        DiaNoche {
            noche,
            desde_ms: None,
        }
    }

    /// Pone el modo. Si cambia, empieza el giro del icono; devuelve si
    /// cambio.
    pub fn poner(&mut self, noche: bool, ahora_ms: u64) -> bool {
        if noche == self.noche {
            return false;
        }
        self.noche = noche;
        self.desde_ms = Some(ahora_ms);
        true
    }

    /// Lo que va del cambio, de 0 a 1 (1 si no hay ninguno en marcha).
    pub fn progreso(&self, ahora_ms: u64) -> f32 {
        match self.desde_ms {
            None => 1.0,
            Some(d) => (ahora_ms.saturating_sub(d) as f32 / DURA_MS as f32).min(1.0),
        }
    }

    /// Si hay que seguir pintando para ver el giro.
    pub fn animando(&self, ahora_ms: u64) -> bool {
        self.progreso(ahora_ms) < 1.0
    }

    /// Los dos iconos en ese momento.
    pub fn paso(&self, ahora_ms: u64) -> Paso {
        paso(self.noche, self.progreso(ahora_ms))
    }
}

/// Los iconos con el modo `noche` ya puesto y el cambio en `progreso`.
pub fn paso(noche: bool, progreso: f32) -> Paso {
    let t = suave(progreso);
    let entra = Trazo {
        luna: noche,
        alfa: t,
        tam: 0.55 + 0.45 * t,
        giro: -GIRO * (1.0 - t),
    };
    let sale = (progreso < 1.0).then_some(Trazo {
        luna: !noche,
        alfa: 1.0 - t,
        tam: 1.0 - 0.45 * t,
        giro: GIRO * t,
    });
    Paso { sale, entra }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_caja_es_redonda_y_pegada_a_la_derecha() {
        let c = caja(500, 32, 150);
        assert_eq!(c.ancho, c.alto);
        assert_eq!(c.ancho, 54);
        assert_eq!(c.derecha(), 500);
        assert_eq!(c.y + c.alto as i32 / 2, 32);
    }

    #[test]
    fn quieto_solo_se_ve_el_del_modo_puesto() {
        let d = DiaNoche::nuevo(false);
        let p = d.paso(1_000);
        assert!(p.sale.is_none());
        assert!(!p.entra.luna);
        assert_eq!((p.entra.alfa, p.entra.tam, p.entra.giro), (1.0, 1.0, 0.0));
        assert!(!d.animando(1_000));
    }

    #[test]
    fn al_cambiar_el_sol_sale_girando_y_entra_la_luna() {
        let mut d = DiaNoche::nuevo(false);
        assert!(d.poner(true, 10_000));
        let medio = d.paso(10_000 + DURA_MS / 2);
        let sale = medio.sale.expect("el sol aun se ve");
        assert!(!sale.luna && medio.entra.luna);
        assert!(sale.alfa > 0.0 && sale.alfa < 1.0);
        assert!(medio.entra.alfa > 0.0 && medio.entra.alfa < 1.0);
        assert!(sale.giro > 0.0 && medio.entra.giro < 0.0);
        assert!(d.animando(10_000 + DURA_MS / 2));
        // Al acabar, solo la luna y quieta.
        let fin = d.paso(10_000 + DURA_MS);
        assert!(fin.sale.is_none());
        assert_eq!(fin.entra.giro, 0.0);
        assert!(!d.animando(10_000 + DURA_MS));
    }

    #[test]
    fn poner_el_mismo_modo_no_anima() {
        let mut d = DiaNoche::nuevo(true);
        assert!(!d.poner(true, 5));
        assert!(!d.animando(5));
        // Caso negativo: un reloj que va hacia atras no da un progreso raro.
        assert!(d.poner(false, 100));
        assert_eq!(d.progreso(50), 0.0);
    }
}
