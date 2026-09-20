//! **La biblioteca de figuras**: guardar una seleccion y volver a estamparla.
//!
//! Es la herramienta mas barata de toda la lista y la que mas tiempo ahorra a
//! quien dibuja lo mismo cada dia: una flecha de norte, un sello de «borrador»,
//! el bloque de titulo de una lamina.
//!
//! Tres decisiones, las tres del movil (`Biblioteca.kt`) y las tres con motivo:
//!
//! 1. **Lo guardado se normaliza al origen.** Una figura guardada no recuerda
//!    donde estaba, porque donde estaba no es parte de la figura. Sin esto,
//!    estampar tendria que restar la posicion antigua cada vez y cualquier
//!    error de redondeo se acumularia.
//! 2. **Al estampar nacen identificadores y semillas nuevos.** La semilla es
//!    lo que hace que el garabato sea el mismo cada vez que se abre; repetirla
//!    haria que dos sellos de la misma figura salieran calcados, y calcados se
//!    leen como una copia y no como dos cosas.
//! 3. **Lo estampado nace agrupado.** Se guardo como una cosa y tiene que
//!    volver como una cosa: si naciera suelto, el primer clic lo desharia.
//!
//! Lo que NO hace, y es a proposito: no viaja en el `.excalidraw`. Una figura
//! guardada es un contenedor aparte, igual que alli —no hay campo del elemento
//! que la mencione— y por eso abrir un `.pixpin` no trae la biblioteca del
//! otro ni se la lleva.

use serde::{Deserialize, Serialize};

use crate::elemento::Elemento;
use crate::vector::Punto2;

/// Una figura guardada: su nombre y las piezas de las que esta hecha, ya
/// normalizadas al origen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FiguraGuardada {
    pub nombre: String,
    /// Las piezas, con su esquina superior izquierda comun en `(0, 0)`.
    pub elementos: Vec<Elemento>,
}

impl FiguraGuardada {
    /// Lo que mide, calculado y no guardado: guardarlo seria una segunda
    /// verdad sobre las piezas y quedaria desfasado en cuanto se editara una.
    pub fn tamano(&self) -> (f32, f32) {
        match caja_de(&self.elementos) {
            Some((x0, y0, x1, y1)) => (x1 - x0, y1 - y0),
            None => (0.0, 0.0),
        }
    }
}

/// La caja que ocupan varios elementos juntos. `None` si no hay ninguno vivo.
pub fn caja_de(elementos: &[Elemento]) -> Option<(f32, f32, f32, f32)> {
    let mut caja: Option<(f32, f32, f32, f32)> = None;
    for e in elementos.iter().filter(|e| !e.borrado) {
        let (x0, y0, x1, y1) = e.caja();
        caja = Some(match caja {
            None => (x0, y0, x1, y1),
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
        });
    }
    caja
}

/// Lleva las piezas a que su esquina comun caiga en `(0, 0)`.
pub fn normalizar(elementos: &[Elemento]) -> Vec<Elemento> {
    let mut piezas: Vec<Elemento> = elementos.iter().filter(|e| !e.borrado).cloned().collect();
    if let Some((x0, y0, _, _)) = caja_de(&piezas) {
        for e in &mut piezas {
            e.mover(-x0, -y0);
        }
    }
    piezas
}

/// Guarda lo elegido como figura. `None` si no habia nada que guardar.
pub fn de_la_seleccion(
    elementos: &[Elemento],
    elegidos: &[u64],
    nombre: &str,
) -> Option<FiguraGuardada> {
    let piezas: Vec<Elemento> = elementos
        .iter()
        .filter(|e| elegidos.contains(&e.id) && !e.borrado)
        .cloned()
        .collect();
    if piezas.is_empty() {
        return None;
    }
    Some(FiguraGuardada {
        nombre: nombre.to_string(),
        elementos: normalizar(&piezas),
    })
}

/// Estampa la figura **centrada** en `centro`.
///
/// `primer_id` es el identificador libre con el que empezar: la escena es
/// quien los reparte, y aqui solo se consumen correlativos desde ese. El
/// nombre del grupo lo pone el llamante por lo mismo que los ids —en el
/// fichero es una cadena que genera el movil y aqui viaja intacta—.
pub fn estampar(
    figura: &FiguraGuardada,
    centro: Punto2,
    primer_id: u64,
    grupo: &str,
) -> Vec<Elemento> {
    let mut piezas = figura.elementos.clone();
    let (ancho, alto) = figura.tamano();
    // Centrado y no con la esquina en el cursor: lo que el usuario apunta con
    // el raton es donde quiere que quede la figura, no donde quiere que
    // empiece su caja invisible.
    let (dx, dy) = (centro.x - ancho / 2.0, centro.y - alto / 2.0);
    for (i, e) in piezas.iter_mut().enumerate() {
        e.mover(dx, dy);
        e.id = primer_id + i as u64;
        // Semilla nueva **derivada del id**, no al azar: asi estampar dos
        // veces en el mismo sitio da dos garabatos distintos, y estampar en
        // una sesion y en la siguiente da el mismo si el id coincide, que es
        // lo que hace que el documento se reabra igual.
        e.semilla = semilla_de(e.id);
        e.version = 0;
        e.borrado = false;
        // El grupo de la figura va el ULTIMO de la pila: `groupIds` es una
        // pila donde el ultimo es el mas exterior, asi que desagrupar una vez
        // deshace la estampacion y deja intactos los grupos que la figura ya
        // traia dentro.
        e.grupos.push(grupo.to_string());
    }
    piezas
}

/// Una semilla nunca cero a partir de un identificador. Cero significaria
/// «sin sembrar» y el dibujo temblaria en cada apertura.
fn semilla_de(id: u64) -> u32 {
    // Mezcla barata (xorshift de una vuelta): ids correlativos tienen que dar
    // garabatos distintos, y `id as u32` daria semillas casi iguales.
    let mut x = (id as u32) ^ 0x9E37_79B9;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x.max(1)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::Figura;

    fn pieza(id: u64, x: f32, y: f32) -> Elemento {
        Elemento {
            id,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho: 10.0,
            alto: 10.0,
            semilla: 7,
            ..Default::default()
        }
    }

    fn guardada() -> FiguraGuardada {
        de_la_seleccion(
            &[pieza(1, 100.0, 200.0), pieza(2, 120.0, 200.0)],
            &[1, 2],
            "flecha",
        )
        .unwrap()
    }

    #[test]
    fn lo_guardado_olvida_donde_estaba() {
        let f = guardada();
        assert_eq!(caja_de(&f.elementos).unwrap().0, 0.0);
        assert_eq!(caja_de(&f.elementos).unwrap().1, 0.0);
        assert_eq!(f.tamano(), (30.0, 10.0));
    }

    #[test]
    fn guardar_sin_nada_elegido_no_crea_una_figura_vacia() {
        // Caso negativo: una biblioteca con figuras de cero piezas es una
        // lista de botones que no hacen nada.
        assert!(de_la_seleccion(&[pieza(1, 0.0, 0.0)], &[], "x").is_none());
        let mut borrado = pieza(1, 0.0, 0.0);
        borrado.borrado = true;
        assert!(de_la_seleccion(&[borrado], &[1], "x").is_none());
    }

    #[test]
    fn estampar_deja_la_figura_centrada_en_el_cursor() {
        let e = estampar(&guardada(), Punto2::nuevo(500.0, 500.0), 40, "g1");
        let (x0, y0, x1, y1) = caja_de(&e).unwrap();
        assert_eq!(((x0 + x1) / 2.0, (y0 + y1) / 2.0), (500.0, 500.0));
    }

    #[test]
    fn estampar_da_identificadores_y_semillas_nuevos() {
        let e = estampar(&guardada(), Punto2::nuevo(0.0, 0.0), 40, "g1");
        assert_eq!(e[0].id, 40);
        assert_eq!(e[1].id, 41);
        assert_ne!(
            e[0].semilla, e[1].semilla,
            "dos sellos calcados se leen mal"
        );
        assert_ne!(e[0].semilla, 7, "la semilla guardada no se reutiliza");
        assert!(e.iter().all(|x| x.semilla != 0), "cero seria sin sembrar");
    }

    #[test]
    fn estampar_dos_veces_en_el_mismo_sitio_no_da_dos_dibujos_identicos() {
        let a = estampar(&guardada(), Punto2::nuevo(0.0, 0.0), 40, "g1");
        let b = estampar(&guardada(), Punto2::nuevo(0.0, 0.0), 80, "g2");
        assert_ne!(a[0].semilla, b[0].semilla);
    }

    #[test]
    fn lo_estampado_nace_agrupado_y_el_grupo_de_la_figura_va_por_fuera() {
        let mut f = guardada();
        f.elementos[0].grupos = vec!["interno".into()];
        let e = estampar(&f, Punto2::nuevo(0.0, 0.0), 1, "sello");
        assert_eq!(
            e[0].grupos,
            vec!["interno".to_string(), "sello".to_string()]
        );
        assert_eq!(e[1].grupos, vec!["sello".to_string()]);
    }

    #[test]
    fn la_figura_guardada_va_y_vuelve_por_json() {
        // La biblioteca se guarda en su propio fichero, no en el
        // `.excalidraw`: esto es lo que promete que se puede.
        let f = guardada();
        let texto = serde_json::to_string(&f).unwrap();
        let vuelta: FiguraGuardada = serde_json::from_str(&texto).unwrap();
        assert_eq!(f, vuelta);
    }
}
