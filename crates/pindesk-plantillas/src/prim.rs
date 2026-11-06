//! **Las primitivas**: lo que sale de maquetar y lo unico que saben pintar
//! los que pintan (la pantalla con Direct2D, el PNG, el PDF, el SVG y las
//! figuras de PinDesk). En milimetros desde la esquina de arriba a la
//! izquierda de la pagina.

use crate::modelo::{Alineacion, Eje};
use crate::tema::{Color, Letra};

/// Una caja (mm).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Caja {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Caja {
    pub const fn nueva(x: f32, y: f32, w: f32, h: f32) -> Caja {
        Caja { x, y, w, h }
    }
    pub fn derecha(&self) -> f32 {
        self.x + self.w
    }
    pub fn abajo(&self) -> f32 {
        self.y + self.h
    }
    pub fn centro(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
    pub fn contiene(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.derecha() && y >= self.y && y <= self.abajo()
    }
    /// Encogida `m` por cada lado (nunca negativa).
    pub fn dentro(&self, m: f32) -> Caja {
        let mx = m.min(self.w / 2.0);
        let my = m.min(self.h / 2.0);
        Caja::nueva(
            self.x + mx,
            self.y + my,
            self.w - 2.0 * mx,
            self.h - 2.0 * my,
        )
    }
    /// Lo de arriba (`h` mm) y el resto.
    pub fn partir_arriba(&self, h: f32) -> (Caja, Caja) {
        let h = h.clamp(0.0, self.h);
        (
            Caja::nueva(self.x, self.y, self.w, h),
            Caja::nueva(self.x, self.y + h, self.w, self.h - h),
        )
    }
    pub fn partir_izquierda(&self, w: f32) -> (Caja, Caja) {
        let w = w.clamp(0.0, self.w);
        (
            Caja::nueva(self.x, self.y, w, self.h),
            Caja::nueva(self.x + w, self.y, self.w - w, self.h),
        )
    }
}

/// Como va una raya.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Raya {
    #[default]
    Continua,
    Discontinua,
    Punteada,
}

/// Donde va el texto en lo alto de su caja.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Vertical {
    #[default]
    Arriba,
    Centro,
    Abajo,
}

/// **Una primitiva.**
#[derive(Debug, Clone, PartialEq)]
pub enum Prim {
    Rect {
        caja: Caja,
        relleno: Option<Color>,
        trazo: Option<Color>,
        grosor: f32,
        radio: f32,
    },
    Linea {
        a: (f32, f32),
        b: (f32, f32),
        color: Color,
        grosor: f32,
        raya: Raya,
    },
    Circulo {
        centro: (f32, f32),
        radio: f32,
        relleno: Option<Color>,
        trazo: Option<Color>,
        grosor: f32,
    },
    /// Una linea quebrada (la boca de una cara, una marca).
    Poli {
        puntos: Vec<(f32, f32)>,
        color: Color,
        grosor: f32,
    },
    /// Muchos puntos iguales (la pauta de puntos), de una vez.
    Puntos {
        centros: Vec<(f32, f32)>,
        radio: f32,
        color: Color,
    },
    /// Un texto en su caja. `tam` es la altura de la letra (em) en mm.
    /// Con `partir` se parte en renglones a lo ancho de la caja; sin el, va
    /// en un renglon y, si no cabe, se encoge.
    Texto {
        caja: Caja,
        texto: String,
        tam: f32,
        letra: Letra,
        negrita: bool,
        color: Color,
        al: Alineacion,
        v: Vertical,
        partir: bool,
    },
}

/// **Donde quedo cada bloque**: para elegir con el raton, las guias de
/// soltar y las asas.
#[derive(Debug, Clone, PartialEq)]
pub struct CajaBloque {
    pub id: u64,
    pub caja: Caja,
    pub padre: Option<u64>,
    /// Su posicion entre sus hermanos.
    pub indice: usize,
    pub profundidad: usize,
    /// Si es contenedor, en que eje reparte.
    pub eje: Option<Eje>,
    /// Cuantos hijos tiene (contenedores).
    pub hijos: usize,
}

/// **Una pagina maquetada.**
#[derive(Debug, Clone, PartialEq)]
pub struct PaginaHecha {
    pub ancho: f32,
    pub alto: f32,
    pub papel: Color,
    /// Las familias de su tema: titulo, cuerpo y mano.
    pub familias: [String; 3],
    pub prims: Vec<Prim>,
    pub cajas: Vec<CajaBloque>,
}

impl PaginaHecha {
    /// El bloque mas hondo bajo un punto (mm de la pagina).
    pub fn bloque_en(&self, x: f32, y: f32) -> Option<&CajaBloque> {
        self.cajas
            .iter()
            .filter(|c| c.caja.contiene(x, y))
            .max_by_key(|c| c.profundidad)
    }

    pub fn caja_de(&self, id: u64) -> Option<&CajaBloque> {
        self.cajas.iter().find(|c| c.id == id)
    }
}
