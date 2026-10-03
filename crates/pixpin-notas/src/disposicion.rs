//! **Donde va cada cosa** en la ventana del editor, sin pintar nada: la
//! cabecera con el titulo y los botones de la derecha, la barra de
//! herramientas y el papel. Es la distribucion del editor de documentos de
//! Claude de las capturas del usuario: arriba el nombre en su pastilla con
//! su flecha, a la derecha «Compartir», pantalla completa y cerrar; debajo
//! insertar tabla, imagen, casillas y el «+» con su flecha. Se prueba sin
//! ventana.
//!
//! Todo en pixeles de la ventana; `escala` es `ppp / 96`.

/// Un rectangulo en pixeles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Caja {
    pub x: i32,
    pub y: i32,
    pub an: i32,
    pub al: i32,
}

impl Caja {
    pub fn contiene(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.an && y >= self.y && y < self.y + self.al
    }
    pub fn derecha(&self) -> i32 {
        self.x + self.an
    }
    pub fn abajo(&self) -> i32 {
        self.y + self.al
    }
}

/// Lo que se puede pulsar en el marco.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Boton {
    /// La pastilla del titulo: clic lo edita; su flecha abre el menu «mas».
    Titulo,
    TituloMenu,
    /// El globo de la cabecera: abre o cierra el panel de comentarios.
    Comentarios,
    Compartir,
    Minimizar,
    PantallaCompleta,
    Cerrar,
    Tabla,
    Imagen,
    Casillas,
    /// Comentar lo elegido (`editor::comentarios`).
    Comentar,
    Mas,
    // Los de la tabla, solo con el cursor dentro de una.
    FilaMas,
    FilaMenos,
    ColumnaMas,
    ColumnaMenos,
    /// Combina lo elegido (o separa la combinada del cursor).
    Combinar,
    /// La paleta de fondo y letra de las celdas.
    Color,
}

/// Lo que hace falta saber para colocar: el tamano y lo que miden los
/// textos (lo mide quien pinta, con su letra).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Medidas {
    pub ancho: i32,
    pub alto: i32,
    pub escala: f32,
    pub ancho_titulo: i32,
    pub ancho_compartir: i32,
    /// Con el cursor en una tabla salen sus botones, que miden esto.
    pub en_tabla: bool,
    pub anchos_tabla: [i32; 6],
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Marco {
    pub cabecera: Caja,
    pub barra: Caja,
    pub cuerpo: Caja,
    /// La raya entre los tres botones de insertar y el «+».
    pub separadores: Vec<Caja>,
    pub botones: Vec<(Boton, Caja)>,
}

/// Lo que hay bajo un punto de la ventana.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zona {
    Boton(Boton),
    /// Cabecera o barra sin boton: se arrastra la ventana desde ahi.
    Arrastre,
    Cuerpo,
}

/// Los botones de la tabla, en su orden en la barra.
pub const DE_TABLA: [Boton; 6] = [
    Boton::FilaMas,
    Boton::FilaMenos,
    Boton::ColumnaMas,
    Boton::ColumnaMenos,
    Boton::Combinar,
    Boton::Color,
];

pub const ALTO_CABECERA: i32 = 44;
pub const ALTO_BARRA: i32 = 38;
const LADO: i32 = 10;
const BOTON: i32 = 28;

pub fn disponer(m: &Medidas) -> Marco {
    let e = |v: i32| (v as f32 * m.escala).round() as i32;
    let w = m.ancho.max(1);
    let cabecera = Caja {
        x: 0,
        y: 0,
        an: w,
        al: e(ALTO_CABECERA),
    };
    let barra = Caja {
        x: 0,
        y: cabecera.abajo(),
        an: w,
        al: e(ALTO_BARRA),
    };
    let cuerpo = Caja {
        x: 0,
        y: barra.abajo(),
        an: w,
        al: (m.alto - barra.abajo()).max(0),
    };
    let mut botones = Vec::new();
    let arriba = (cabecera.al - e(BOTON)) / 2;
    let cuadrado = |x: i32, y: i32| Caja {
        x,
        y,
        an: e(BOTON),
        al: e(BOTON),
    };
    // De derecha a izquierda: cerrar, pantalla completa, minimizar (una
    // ventana de Windows lo necesita; la captura es un panel de pagina) y
    // compartir.
    let cerrar = cuadrado(w - e(LADO) - e(BOTON), arriba);
    let completa = cuadrado(cerrar.x - e(4) - e(BOTON), arriba);
    let minimizar = cuadrado(completa.x - e(4) - e(BOTON), arriba);
    let an_compartir = m.ancho_compartir + e(16) + e(26);
    let compartir = Caja {
        x: minimizar.x - e(8) - an_compartir,
        y: arriba,
        an: an_compartir,
        al: e(BOTON),
    };
    // El globo de los comentarios, a la izquierda de compartir (como en la
    // captura).
    let comentarios = cuadrado(compartir.x - e(6) - e(BOTON), arriba);
    // El titulo con su flecha, hasta donde deje el globo.
    let flecha = e(22);
    let cabe = (comentarios.x - e(12) - e(LADO)).max(flecha + e(20));
    let an_titulo = (m.ancho_titulo + e(20) + flecha).min(cabe);
    let titulo = Caja {
        x: e(LADO),
        y: arriba,
        an: an_titulo - flecha,
        al: e(BOTON),
    };
    let titulo_menu = Caja {
        x: titulo.derecha(),
        y: arriba,
        an: flecha,
        al: e(BOTON),
    };
    botones.push((Boton::Titulo, titulo));
    botones.push((Boton::TituloMenu, titulo_menu));
    botones.push((Boton::Comentarios, comentarios));
    botones.push((Boton::Compartir, compartir));
    botones.push((Boton::Minimizar, minimizar));
    botones.push((Boton::PantallaCompleta, completa));
    botones.push((Boton::Cerrar, cerrar));

    // La barra: tabla, imagen, casillas | + ▾ | los de la tabla.
    let y = barra.y + (barra.al - e(BOTON)) / 2;
    let mut x = e(LADO);
    for b in [Boton::Tabla, Boton::Imagen, Boton::Casillas, Boton::Comentar] {
        botones.push((b, cuadrado(x, y)));
        x += e(BOTON) + e(4);
    }
    let mut separadores = Vec::new();
    let raya = |x: i32| Caja {
        x,
        y: y + e(6),
        an: e(1).max(1),
        al: e(BOTON) - e(12),
    };
    separadores.push(raya(x + e(2)));
    x += e(10);
    botones.push((
        Boton::Mas,
        Caja {
            x,
            y,
            an: e(40),
            al: e(BOTON),
        },
    ));
    x += e(40);
    if m.en_tabla {
        separadores.push(raya(x + e(6)));
        x += e(14);
        let de_tabla = DE_TABLA;
        for (b, t) in de_tabla.iter().zip(m.anchos_tabla) {
            let an = t + e(18);
            if x + an > w - e(LADO) {
                break;
            }
            botones.push((
                *b,
                Caja {
                    x,
                    y,
                    an,
                    al: e(BOTON),
                },
            ));
            x += an + e(4);
        }
    }
    Marco {
        cabecera,
        barra,
        cuerpo,
        separadores,
        botones,
    }
}

impl Marco {
    pub fn caja(&self, b: Boton) -> Option<Caja> {
        self.botones.iter().find(|(x, _)| *x == b).map(|(_, c)| *c)
    }

    pub fn zona(&self, x: i32, y: i32) -> Zona {
        if let Some((b, _)) = self.botones.iter().find(|(_, c)| c.contiene(x, y)) {
            return Zona::Boton(*b);
        }
        if self.cabecera.contiene(x, y) || self.barra.contiene(x, y) {
            Zona::Arrastre
        } else {
            Zona::Cuerpo
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn medidas(ancho: i32, en_tabla: bool) -> Medidas {
        Medidas {
            ancho,
            alto: 700,
            escala: 1.0,
            ancho_titulo: 300,
            ancho_compartir: 60,
            en_tabla,
            anchos_tabla: [40, 40, 60, 60, 60, 50],
        }
    }

    #[test]
    fn cerrar_va_en_la_esquina_y_compartir_a_su_izquierda() {
        let m = disponer(&medidas(1000, false));
        let cerrar = m.caja(Boton::Cerrar).unwrap();
        assert_eq!(cerrar.derecha(), 990);
        let completa = m.caja(Boton::PantallaCompleta).unwrap();
        let compartir = m.caja(Boton::Compartir).unwrap();
        assert!(compartir.derecha() <= completa.x && completa.derecha() <= cerrar.x);
        assert_eq!(m.cuerpo.y, ALTO_CABECERA + ALTO_BARRA);
        assert_eq!(m.cuerpo.al, 700 - ALTO_CABECERA - ALTO_BARRA);
    }

    #[test]
    fn un_titulo_largo_no_pisa_compartir() {
        let mut md = medidas(420, false);
        md.ancho_titulo = 2000;
        let m = disponer(&md);
        let flecha = m.caja(Boton::TituloMenu).unwrap();
        assert!(flecha.derecha() < m.caja(Boton::Compartir).unwrap().x);
    }

    #[test]
    fn el_globo_de_los_comentarios_va_entre_el_titulo_y_compartir_y_comentar_en_la_barra() {
        let mut md = medidas(700, false);
        md.ancho_titulo = 2000;
        let m = disponer(&md);
        let g = m.caja(Boton::Comentarios).unwrap();
        assert!(m.caja(Boton::TituloMenu).unwrap().derecha() < g.x, "un titulo largo no lo pisa");
        assert!(g.derecha() < m.caja(Boton::Compartir).unwrap().x);
        let c = m.caja(Boton::Comentar).unwrap();
        assert!(c.x > m.caja(Boton::Casillas).unwrap().derecha());
        assert!(c.derecha() < m.caja(Boton::Mas).unwrap().x);
    }

    #[test]
    fn los_botones_de_la_tabla_solo_salen_dentro_de_una() {
        let fuera = disponer(&medidas(1000, false));
        assert!(fuera.caja(Boton::FilaMas).is_none());
        let dentro = disponer(&medidas(1000, true));
        let mas = dentro.caja(Boton::Mas).unwrap();
        let fila = dentro.caja(Boton::FilaMas).unwrap();
        assert!(fila.x > mas.derecha());
        assert!(dentro.caja(Boton::ColumnaMenos).is_some());
        // Si no caben, los que no caben no salen (no se montan encima).
        let estrecha = disponer(&medidas(260, true));
        assert!(estrecha.caja(Boton::ColumnaMenos).is_none());
    }

    #[test]
    fn un_clic_en_la_cabecera_sin_boton_arrastra_y_abajo_es_el_papel() {
        let m = disponer(&medidas(1000, false));
        assert_eq!(m.zona(600, 20), Zona::Arrastre);
        assert_eq!(m.zona(600, 60), Zona::Arrastre);
        assert_eq!(m.zona(600, 300), Zona::Cuerpo);
        let t = m.caja(Boton::Tabla).unwrap();
        assert_eq!(m.zona(t.x + 3, t.y + 3), Zona::Boton(Boton::Tabla));
        let c = m.caja(Boton::Cerrar).unwrap();
        assert_eq!(m.zona(c.x + 1, c.y + 1), Zona::Boton(Boton::Cerrar));
    }

    #[test]
    fn todo_crece_con_los_puntos_por_pulgada() {
        let mut md = medidas(1500, false);
        md.escala = 1.5;
        let m = disponer(&md);
        assert_eq!(m.cabecera.al, 66);
        assert_eq!(m.caja(Boton::Cerrar).unwrap().an, 42);
    }
}
