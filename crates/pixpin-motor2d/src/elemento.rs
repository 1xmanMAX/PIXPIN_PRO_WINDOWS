//! Que es un elemento del dibujo y como se toca.
//!
//! Un elemento es su geometria mas su estilo mas **su semilla**. La semilla es
//! lo que hace que el dibujo sea el mismo cada vez que se abre (ver `azar`), y
//! por eso viaja en el fichero como cualquier otro dato.
//!
//! `version` sube en cada cambio. No es informacion para el usuario: es lo que
//! le dice al dibujante que su geometria cacheada ya no vale. Sin ella habria
//! que comparar el elemento entero en cada fotograma.

use std::mem::size_of;

use serde::{Deserialize, Serialize};

use crate::relleno::EstiloRelleno;
use crate::vector::Punto2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EstiloTrazo {
    Solido,
    Discontinuo,
    Punteado,
}

/// Color RGBA, cada canal en `[0,1]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColorRgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ColorRgba {
    pub const fn opaco(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }
}

/// La geometria propia de cada tipo. Lo que no depende del tipo (posicion,
/// color, grosor) vive en `Elemento`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "lowercase")]
pub enum Figura {
    /// Trazo a mano alzada. Las presiones son opcionales: sin tableta se
    /// simulan a partir de la velocidad.
    Lapiz {
        puntos: Vec<Punto2>,
        #[serde(default)]
        presiones: Vec<f32>,
        /// La pluma de Excalidraw (`strokeOptions`). `None` = trazo de antes
        /// de E1, cuyo grosor eran pixeles: ver `tinta::contorno_de_lapiz`.
        #[serde(default)]
        opciones: Option<crate::tinta::OpcionesTinta>,
    },
    /// Como el lapiz pero grueso, translucido y sin rugosidad: resaltar sobre
    /// texto tiene que dejarlo legible (D45).
    Resaltador {
        puntos: Vec<Punto2>,
    },
    Linea {
        puntos: Vec<Punto2>,
    },
    Flecha {
        puntos: Vec<Punto2>,
        #[serde(default)]
        punta_inicio: bool,
        #[serde(default = "verdadero")]
        punta_fin: bool,
    },
    Rectangulo,
    Elipse,
    /// Oscurece todo menos su caja (D51). El motor entrega el hueco; quien
    /// pinta sabe cuanto mide el lienzo y oscurece el resto.
    Foco {
        #[serde(default)]
        elipse: bool,
    },
    Texto {
        texto: String,
        tam: f32,
        #[serde(default = "familia_por_defecto")]
        familia: String,
    },
    /// El bitmap no vive aqui: lo aporta quien dibuja (el pin tiene el suyo,
    /// el PDF el suyo). Aqui solo va la referencia.
    Imagen {
        id_objeto: u64,
    },
    /// Una cota: el segmento acotado y nada mas.
    ///
    /// **No guarda su texto** (D34): ni el numero, ni la unidad, ni el
    /// angulo. El rotulo se deriva al pintar, cada vez. Una cota que
    /// guardara su texto podria decir una cosa y medir otra, y en un plano
    /// eso es peor que no tener cotas.
    ///
    /// Es figura propia y no una `Linea` con una bandera (D38): el movil usa
    /// el tipo `pixpin-measure`, y con una bandera se romperia la
    /// compatibilidad que esta tarea viene a asegurar.
    Cota {
        puntos: Vec<Punto2>,
    },
    /// La reglita a cuadros. Usa `x`/`y`/`ancho`/`alto` como el rectangulo;
    /// cuantos cuadros pone y cuanto mide cada uno sale de la escala de la
    /// escena al pintar.
    ///
    /// Es un **elemento del dibujo y no un adorno del editor** (D37): se
    /// guarda, se mueve, se estira y sale en la exportacion. Es lo que
    /// permite medir sobre la imagen que recibe otro.
    EscalaGrafica,
    /// Un marco: un recuadro con nombre que agrupa POR CONTENCION lo que
    /// cae dentro (`frame` de Excalidraw). Moverlo mueve lo de dentro, que
    /// es para lo que sirve: preparar laminas y moverlas de una pieza.
    ///
    /// Quien esta dentro no se guarda en ninguna lista: se mira la caja cada
    /// vez. Una lista seria una segunda verdad sobre lo mismo, y habria que
    /// mantenerla al mover, al borrar y al deshacer.
    Marco {
        #[serde(default)]
        nombre: String,
    },
    /// Un emoji suelto, del tamano de su caja (universo, §2.2). Se pinta
    /// con la fuente de color de Windows; no hay imagenes empaquetadas.
    Emoji {
        caracter: String,
    },
}

fn verdadero() -> bool {
    true
}

fn familia_por_defecto() -> String {
    "Segoe UI".to_string()
}

/// Un elemento del dibujo. `#[serde(default)]` en todo lo que se pueda: un
/// fichero de una version futura tiene que abrir igual (la misma regla que el
/// indice del almacen y los ajustes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Elemento {
    pub id: u64,
    pub figura: Figura,
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
    #[serde(default)]
    pub angulo: f32,
    pub trazo: ColorRgba,
    #[serde(default)]
    pub relleno: Option<ColorRgba>,
    /// Como se pinta ese relleno: plano o rayado (ver `relleno.rs`).
    ///
    /// El valor por omision del TIPO es rayado, que es lo que significa un
    /// elemento sin `fillStyle` en Excalidraw y en el movil; asi lo lee
    /// `excalidraw::leer`, explicitamente.
    ///
    /// Pero al leer un documento NUESTRO sin este campo se usa SOLIDO, que es
    /// como se veia cuando se guardo. Son dos origenes distintos y cada uno
    /// merece su respuesta: un `.excalidraw` sin `fillStyle` siempre quiso
    /// decir rayado, y un `.pixpin2d` de antes de que esto existiera se
    /// dibujo solido. Igualarlos cambiaria el aspecto de lo que el usuario ya
    /// tiene guardado, que es lo que no se puede hacer.
    #[serde(default = "relleno_solido")]
    pub estilo_relleno: EstiloRelleno,
    pub grosor: f32,
    #[serde(default = "estilo_por_defecto")]
    pub estilo: EstiloTrazo,
    #[serde(default = "uno")]
    pub rugosidad: f32,
    #[serde(default = "uno")]
    pub opacidad: f32,
    /// Sin ella el dibujo cambiaria de aspecto en cada apertura (D38).
    #[serde(default = "uno_u32")]
    pub semilla: u32,
    #[serde(default)]
    pub version: u32,
    /// Borrado logico: deshacer es volver a ponerlo en `false`, sin recuperar
    /// nada de disco.
    #[serde(default)]
    pub borrado: bool,
    /// Los grupos a los que pertenece, con los identificadores del movil
    /// (`groupIds` de Excalidraw).
    ///
    /// Cadenas y no numeros porque el movil las genera como cadenas y esto
    /// viaja de ida y vuelta sin tocarlas. Inventar aqui un `u64`
    /// obligaria a mantener una tabla de traduccion, que es una segunda
    /// verdad sobre lo mismo.
    #[serde(default)]
    pub grupos: Vec<String>,
    /// Bloqueado: se ve pero no se puede elegir ni mover (`locked` de
    /// Excalidraw). Es lo que se usa para dejar quieto un plano de fondo y
    /// dibujar encima sin arrastrarlo sin querer.
    #[serde(default)]
    pub bloqueado: bool,
    /// A donde lleva este elemento: el `enlace` de PixPin Android, que es el
    /// id del dibujo de otra hoja. Es lo que convierte un recuadro en la
    /// puerta a un sublienzo —la «zona» de una pagina—.
    #[serde(default)]
    pub enlace: Option<String>,
    /// Esquinas redondeadas (`roundness` de Excalidraw). Sin esto, un
    /// recuadro que el movil dibuja redondeado sale aqui en punta, y encima
    /// del suyo parece otro recuadro distinto.
    #[serde(default)]
    pub redondo: bool,
}

fn relleno_solido() -> EstiloRelleno {
    EstiloRelleno::Solido
}

fn estilo_por_defecto() -> EstiloTrazo {
    EstiloTrazo::Solido
}

fn uno() -> f32 {
    1.0
}

fn uno_u32() -> u32 {
    1
}

impl Elemento {
    /// La caja que ocupa, en coordenadas del documento.
    ///
    /// Para las figuras con puntos se calcula de los puntos, no de los campos
    /// `x/ancho`: al dibujar un trazo, los puntos van cambiando y la caja
    /// tiene que seguirlos.
    pub fn caja(&self) -> (f32, f32, f32, f32) {
        match &self.figura {
            Figura::Lapiz { puntos, .. }
            | Figura::Resaltador { puntos }
            | Figura::Linea { puntos }
            | Figura::Flecha { puntos, .. }
            | Figura::Cota { puntos } => {
                if puntos.is_empty() {
                    return (self.x, self.y, self.x, self.y);
                }
                // Un lapiz de E1 (con `opciones`) guarda el `strokeWidth` de
                // Excalidraw, no pixeles: su tinta llega a `grosor *
                // FACTOR_VARIABLE` de ancho (ver `tinta::contorno_de_lapiz`).
                // Con la mitad del grosor a secas, el recorte de lo que no se
                // ve hacia saltar el trazo y el marco de seleccion lo cortaba.
                let mitad = match &self.figura {
                    Figura::Lapiz {
                        opciones: Some(_), ..
                    } => self.grosor * crate::tinta::FACTOR_VARIABLE / 2.0,
                    _ => self.grosor / 2.0,
                };
                (
                    puntos.iter().map(|p| p.x).fold(f32::MAX, f32::min) - mitad,
                    puntos.iter().map(|p| p.y).fold(f32::MAX, f32::min) - mitad,
                    puntos.iter().map(|p| p.x).fold(f32::MIN, f32::max) + mitad,
                    puntos.iter().map(|p| p.y).fold(f32::MIN, f32::max) + mitad,
                )
            }
            _ => (self.x, self.y, self.x + self.ancho, self.y + self.alto),
        }
    }

    /// Mueve el elemento. Con puntos propios se mueven los puntos: si solo se
    /// moviera `x/y`, un trazo se quedaria donde estaba.
    pub fn mover(&mut self, dx: f32, dy: f32) {
        self.x += dx;
        self.y += dy;
        match &mut self.figura {
            Figura::Lapiz { puntos, .. }
            | Figura::Resaltador { puntos }
            | Figura::Linea { puntos }
            | Figura::Flecha { puntos, .. }
            | Figura::Cota { puntos } => {
                for p in puntos.iter_mut() {
                    p.x += dx;
                    p.y += dy;
                }
            }
            _ => {}
        }
        self.version = self.version.wrapping_add(1);
    }

    /// Anota que el elemento cambio: invalida su geometria cacheada.
    pub fn tocar(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    /// Los puntos de la figura, si los tiene.
    pub fn puntos(&self) -> Option<&[Punto2]> {
        match &self.figura {
            Figura::Lapiz { puntos, .. }
            | Figura::Resaltador { puntos }
            | Figura::Linea { puntos }
            | Figura::Flecha { puntos, .. }
            | Figura::Cota { puntos } => Some(puntos),
            _ => None,
        }
    }

    /// Si el interior cuenta para el hit-test y para el dibujo.
    pub fn tiene_relleno(&self) -> bool {
        self.relleno.is_some_and(|c| c.a > 0.0)
    }

    /// Lo que ocupa de verdad, contando lo que hay al otro lado de los
    /// punteros. `size_of` solo cuenta la cabecera, y un trazo de 492
    /// puntos son cuatro kilobytes que no apareceran en el techo.
    pub fn bytes(&self) -> usize {
        let dentro = match &self.figura {
            Figura::Lapiz {
                puntos, presiones, ..
            } => puntos.len() * size_of::<Punto2>() + presiones.len() * size_of::<f32>(),
            Figura::Resaltador { puntos }
            | Figura::Linea { puntos }
            | Figura::Flecha { puntos, .. }
            | Figura::Cota { puntos } => puntos.len() * size_of::<Punto2>(),
            Figura::Texto { texto, familia, .. } => texto.len() + familia.len(),
            _ => 0,
        };
        let grupos: usize = self.grupos.iter().map(String::len).sum();
        size_of::<Elemento>() + dentro + grupos
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn lapiz() -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Lapiz {
                puntos: vec![
                    Punto2::nuevo(10.0, 10.0),
                    Punto2::nuevo(50.0, 30.0),
                    Punto2::nuevo(20.0, 60.0),
                ],
                presiones: vec![],
                opciones: None,
            },
            x: 0.0,
            y: 0.0,
            ancho: 0.0,
            alto: 0.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 4.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 42,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
        }
    }

    fn caja_rect() -> Elemento {
        Elemento {
            figura: Figura::Rectangulo,
            x: 100.0,
            y: 50.0,
            ancho: 200.0,
            alto: 80.0,
            ..lapiz()
        }
    }

    #[test]
    fn la_caja_de_un_trazo_sale_de_sus_puntos_y_cuenta_el_grosor() {
        // Si la caja no contara el grosor, la mitad del trazo quedaria fuera
        // y al borrar la zona quedarian restos de tinta.
        let (x0, y0, x1, y1) = lapiz().caja();
        assert_eq!((x0, y0, x1, y1), (8.0, 8.0, 52.0, 62.0));
    }

    #[test]
    fn la_caja_de_un_lapiz_de_e1_cubre_la_tinta_ancha_de_excalidraw() {
        // Con opciones, el grosor es un strokeWidth y la tinta mide grosor *
        // FACTOR_VARIABLE: la caja crece la mitad de eso a cada lado. El
        // trazo viejo (sin opciones) conserva la mitad del grosor.
        let mut e = lapiz();
        e.figura = Figura::Lapiz {
            puntos: vec![Punto2::nuevo(10.0, 10.0), Punto2::nuevo(50.0, 60.0)],
            presiones: vec![],
            opciones: Some(crate::tinta::OpcionesTinta::default()),
        };
        e.grosor = 2.0;
        let margen = 2.0 * crate::tinta::FACTOR_VARIABLE / 2.0;
        assert_eq!(
            e.caja(),
            (10.0 - margen, 10.0 - margen, 50.0 + margen, 60.0 + margen)
        );
    }

    #[test]
    fn mover_un_trazo_mueve_sus_puntos_y_no_solo_su_origen() {
        // El fallo clasico: mover x/y y dejar los puntos donde estaban, con lo
        // que el trazo no se mueve pero su caja si.
        let mut e = lapiz();
        e.mover(100.0, -20.0);
        let p = e.puntos().unwrap();
        assert_eq!(p[0], Punto2::nuevo(110.0, -10.0));
        assert_eq!(p[2], Punto2::nuevo(120.0, 40.0));
        assert_eq!(e.caja().0, 108.0);
    }

    #[test]
    fn cualquier_cambio_sube_la_version() {
        // Es lo que invalida la geometria cacheada; sin esto el elemento se
        // seguiria dibujando en su sitio anterior.
        let mut e = lapiz();
        let antes = e.version;
        e.mover(1.0, 0.0);
        assert_eq!(e.version, antes + 1);
        e.tocar();
        assert_eq!(e.version, antes + 2);
    }

    #[test]
    fn la_caja_de_una_figura_sin_puntos_es_su_rectangulo() {
        assert_eq!(caja_rect().caja(), (100.0, 50.0, 300.0, 130.0));
    }

    #[test]
    fn un_trazo_vacio_no_entra_en_panico_al_medirse() {
        let mut e = lapiz();
        e.figura = Figura::Lapiz {
            puntos: vec![],
            presiones: vec![],
            opciones: None,
        };
        let (x0, y0, x1, y1) = e.caja();
        assert!(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite());
    }

    #[test]
    fn un_relleno_transparente_no_cuenta_como_relleno() {
        // Caso negativo: `Some(color)` con alfa 0 es "sin relleno", y tratarlo
        // como relleno haria que el interior de la figura capturase clics
        // destinados a lo que hay debajo.
        let mut e = caja_rect();
        assert!(!e.tiene_relleno());
        e.relleno = Some(ColorRgba {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        });
        assert!(!e.tiene_relleno());
        e.relleno = Some(ColorRgba::opaco(1.0, 0.0, 0.0));
        assert!(e.tiene_relleno());
    }

    #[test]
    fn un_elemento_de_una_version_futura_se_lee_con_lo_que_falta_por_defecto() {
        // La misma regla que el indice del almacen: campos nuevos y campos
        // que faltan no pueden impedir abrir el documento.
        let json = r#"{
            "id": 7,
            "figura": { "tipo": "rectangulo" },
            "x": 1.0, "y": 2.0, "ancho": 3.0, "alto": 4.0,
            "trazo": { "r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0 },
            "grosor": 2.0,
            "funcion_del_futuro": 42
        }"#;
        let e: Elemento = serde_json::from_str(json).unwrap();
        assert_eq!(e.id, 7);
        assert_eq!(e.rugosidad, 1.0, "la rugosidad por defecto es 1");
        assert_eq!(e.semilla, 1, "una semilla ausente vale 1, nunca 0");
        assert_eq!(e.estilo, EstiloTrazo::Solido);
        // Esto es un documento NUESTRO, no un `.excalidraw`: uno guardado
        // antes de que existiera este campo se dibujo solido, y reabrirlo
        // rayado cambiaria el aspecto de lo que el usuario ya tiene. El
        // rayado es lo que significa un `.excalidraw` SIN `fillStyle`, y de
        // eso se encarga `excalidraw::leer`, que lo pone explicitamente.
        assert_eq!(
            e.estilo_relleno,
            EstiloRelleno::Solido,
            "un documento nuestro de antes de este campo se dibujo solido"
        );
        assert!(!e.borrado);
    }

    #[test]
    fn la_ida_y_vuelta_por_json_conserva_el_elemento() {
        let e = lapiz();
        let texto = serde_json::to_string(&e).unwrap();
        let vuelta: Elemento = serde_json::from_str(&texto).unwrap();
        assert_eq!(e, vuelta);
    }
}
