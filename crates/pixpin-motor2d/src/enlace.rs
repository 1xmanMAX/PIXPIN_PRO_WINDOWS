//! **La flecha atada a la figura**: que siga a la caja cuando la caja se
//! mueve.
//!
//! Port de la parte de anclaje de `Arrows.kt` del PixPin de Android
//! (`getHoveredElementForBinding`, `bindArrow`, `bindingPointOn`,
//! `updateBoundPoints`) y, por debajo, del `startBinding`/`endBinding` de
//! Excalidraw.
//!
//! # Por que es la pieza que mas se nota cuando falta
//!
//! Un organigrama es cajas y flechas. Sin esto, abrir en el PC un esquema
//! hecho en el movil y mover **una sola caja** descoloca todas las flechas
//! que salian de ella: siguen apuntando a donde la caja estaba. Y no es que
//! el dato se pierda —`startBinding` y `endBinding` viajan intactos desde
//! siempre por el carril de §5.1 del inventario—, es que quedan apuntando
//! donde ya no toca, que es peor: el fichero dice una cosa y el dibujo otra.
//!
//! # Las dos cifras que lo gobiernan
//!
//! - **`hueco` (`gap`)**: cuanto se queda la punta separada del borde. Sale
//!   del grosor de trazo del objetivo, porque es el grosor el que decide
//!   donde queda el borde de verdad.
//! - **`foco` (`focus`)**: cuanto se desvia del centro el punto de contacto,
//!   de -1 a 1, medido perpendicular a la flecha y normalizado contra el
//!   semitamano. Es lo que hace que al **agrandar** la caja la flecha siga
//!   senalando el mismo sitio relativo y no salte al centro.
//!
//! # Lo que aqui no se puede cerrar todavia
//!
//! `Enganche::elemento` guarda el id **de texto** del objetivo, que es como
//! lo escribe el fichero. Para un dibujo que viene del movil eso basta y
//! todo funciona: [`resolver`] deshace ese texto al id interno con la misma
//! cuenta que usa `excalidraw::leer`.
//!
//! Para un enganche nacido **aqui**, entre dos elementos que el PC creo, el
//! texto se inventa (`pc<hex>`) y vale mientras dura la sesion, pero **no
//! sobrevive a guardar y reabrir**: `excalidraw.rs` le pone al elemento un id
//! de texto con la hora dentro (`w<hora><id>`) y no lo guarda en ninguna
//! parte, asi que al releer el enganche apunta a un id que ya no existe.
//! Arreglarlo es una linea en `excalidraw.rs` —que un elemento recuerde el id
//! de texto con el que entro o salio— y ese fichero no es de este grupo. Lo
//! que si esta garantizado desde ya es que **un enganche que no resuelve no
//! rompe nada**: [`recolocar`] deja la flecha donde esta y el enganche viaja
//! intacto al movil.

use crate::elemento::{Elemento, Enganche, Figura, ModoEnganche};
use crate::escena::Escena;
use crate::perimetros::{self, PASO_PERIMETRO};
use crate::vector::Punto2;

/// Separacion base entre la punta y el borde (`BASE_BINDING_GAP`).
pub const HUECO_BASE: f32 = 5.0;

/// Cual de los dos extremos de la flecha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extremo {
    Inicio,
    Fin,
}

/// El hueco real contra esta figura (`getBindingGap`).
///
/// Depende de su grosor de trazo porque es el grosor el que decide donde
/// queda el borde que se ve.
pub fn hueco_de(objetivo: &Elemento) -> f32 {
    HUECO_BASE + objetivo.grosor / 2.0
}

/// Hasta donde atrae `objetivo` a una punta que se suelta **por fuera**.
///
/// **Es el propio hueco del anclaje, y ni un pixel mas.** La regla: si la
/// punta ya esta donde el anclaje la dejaria —rozando el borde—, atarla no la
/// mueve de sitio y se gana que la flecha siga a la caja para siempre. Un
/// poco mas lejos, atarla significa **llevarsela**, y ahi deja de ser una
/// ayuda: el sitio donde se posa lo decide un rayo trazado desde el otro
/// extremo, asi que ademas se corre a lo largo del contorno. Sueltas la punta
/// senalando un sitio y aparece senalando otro.
///
/// Se relaja al alejar la vista, hasta el doble: alejado, el mismo hueco son
/// menos pixeles de pantalla y no se acertaria nunca. Al acercar no se
/// aprieta —`z` se topa en uno—, porque de cerca ya se coloca la punta con
/// precision de sobra.
pub fn distancia_maxima(objetivo: &Elemento, zoom: f32) -> f32 {
    let roce = hueco_de(objetivo);
    let z = zoom.clamp(0.0001, 1.0);
    (roce / z).clamp(roce, roce * 2.0)
}

/// Si a esta figura se le puede atar una flecha.
///
/// Lo que encierra un area y se lee como «una cosa»: cajas, rombos, ovalos,
/// imagenes, rotulos, mosaicos y circulos numerados.
///
/// Se quedan fuera, y cada uno por su motivo:
///
/// - **las rayas y las propias flechas**, porque atar una flecha a otra
///   flecha hace cadenas que se persiguen y no se estabilizan;
/// - **el marco**, porque delimita hasta donde llega el dibujo y no es algo
///   dibujado: atar a sus bordes convertiria el papel en un iman;
/// - **el foco**, que es una sombra sobre todo lo demas y no encierra nada;
/// - **el punto etiquetado**, que no tiene area: su caja mide cero y el rayo
///   no cortaria nada.
pub fn se_puede_atar(e: &Elemento) -> bool {
    !e.borrado
        && !e.bloqueado
        && matches!(
            e.figura,
            Figura::Rectangulo
                | Figura::Rombo
                | Figura::Elipse
                | Figura::Imagen { .. }
                | Figura::Texto { .. }
                | Figura::Emoji { .. }
                | Figura::Mosaico { .. }
                | Figura::Serie { .. }
                | Figura::Arco { .. }
                | Figura::Region { .. }
        )
}

/// La figura a la que se engancharia la punta si se soltase en `p`
/// (`getHoveredElementForBinding`).
///
/// **Con la punta dentro tambien ancla**, que es como se comporta el original
/// y lo que lo hace usable: sueltas la flecha encima de la caja, no buscando
/// su borde con el raton.
///
/// Hubo en el movil un intento de probar **solo el contorno**, y venia de un
/// problema real: una flecha que atravesaba un rectangulo relleno para llegar
/// a otro sitio se quedaba atada al del medio. Pero la causa no era probar el
/// interior sino probarlo en el sitio equivocado. Las tres reglas que lo
/// resuelven, y que son las que se copian:
///
/// 1. se prueba **donde acaba la flecha**, no por donde pasa: una flecha que
///    cruza un rectangulo y termina mas alla tiene la punta fuera y no ancla;
/// 2. se recorre de delante hacia atras y **una figura opaca corta la
///    busqueda**: lo que queda tapado detras de ella no es candidato, porque
///    no se ve;
/// 3. entre los candidatos gana **el mas pequeno**: con una caja dentro de
///    otra, anclar a la de fuera nunca es lo que se quiere.
pub fn figura_bajo(elementos: &[Elemento], p: Punto2, zoom: f32, excluir: u64) -> Option<u64> {
    let mut candidatos: Vec<(u64, f32)> = Vec::new();
    for e in elementos.iter().rev() {
        if e.id == excluir || !se_puede_atar(e) {
            continue;
        }
        let dentro = crate::impacto::dentro_de_la_caja_girada(e, p);
        let roza = distancia_al_borde(e, p) <= distancia_maxima(e, zoom);
        if !dentro && !roza {
            continue;
        }
        let (x0, y0, x1, y1) = e.caja();
        candidatos.push((e.id, (x1 - x0) * (y1 - y0)));
        // Regla 2: lo que quede detras de una figura opaca no se ve, asi que
        // no es candidato.
        if dentro && e.tiene_relleno() {
            break;
        }
    }
    // Regla 3: gana el mas pequeno.
    candidatos
        .into_iter()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

/// Lo que se separa `p` del borde dibujado de `e`. Grande si no hay borde.
fn distancia_al_borde(e: &Elemento, p: Punto2) -> f32 {
    perimetros::segmentos_de(e, PASO_PERIMETRO)
        .into_iter()
        .map(|(a, b)| crate::vector::distancia_a_segmento(p, a, b))
        .fold(f32::MAX, f32::min)
}

/// El enganche que le tocaria a la punta `punta` de una flecha cuyo otro
/// extremo esta en `otro` (`bindArrow`).
///
/// `id_texto` es el id **del fichero** del objetivo: ver la nota de la
/// cabecera sobre por que no se deduce aqui.
pub fn enganche_para(
    objetivo: &Elemento,
    id_texto: String,
    punta: Punto2,
    otro: Punto2,
) -> Enganche {
    Enganche {
        elemento: id_texto,
        foco: determinar_foco(objetivo, punta, otro),
        hueco: hueco_de(objetivo),
        punto_fijo: Some(proporcion_en(objetivo, punta)),
        modo: modo_para(objetivo, punta),
    }
}

/// `p` como proporcion de la caja de `objetivo`: `(0,0)`..`(1,1)`.
///
/// Se mide **en el sistema sin girar**, deshaciendo su rotacion primero. Asi
/// el punto agarrado sigue siendo el mismo trozo de la figura aunque luego se
/// gire.
fn proporcion_en(objetivo: &Elemento, p: Punto2) -> (f32, f32) {
    let (x0, y0, x1, y1) = objetivo.caja();
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let local = p.girar(centro, -objetivo.angulo);
    let ancho = if x1 - x0 > 0.0 { x1 - x0 } else { 1.0 };
    let alto = if y1 - y0 > 0.0 { y1 - y0 } else { 1.0 };
    (
        ((local.x - x0) / ancho).clamp(0.0, 1.0),
        ((local.y - y0) / alto).clamp(0.0, 1.0),
    )
}

/// La punta se queda dentro o se posa en el contorno.
///
/// Dentro si se solto **bien adentro**; en el contorno si se solto en el
/// borde o cerca de el por fuera. El umbral es el mismo hueco de anclaje, asi
/// que la franja que cuenta como «el borde» es la que ya se usa para el
/// resto y no hay que inventar otra medida.
fn modo_para(objetivo: &Elemento, p: Punto2) -> ModoEnganche {
    let en_el_borde = distancia_al_borde(objetivo, p) <= hueco_de(objetivo) * 2.0;
    if !en_el_borde && crate::impacto::dentro_de_la_caja_girada(objetivo, p) {
        ModoEnganche::Dentro
    } else {
        ModoEnganche::Orbita
    }
}

/// Cuanto se desvia del centro el punto de contacto, en -1..1
/// (`determineFocusDistance`).
///
/// Se mide perpendicularmente a la linea de la flecha y se normaliza contra
/// el semitamano de la figura, para que el mismo foco senale el mismo sitio
/// relativo aunque la figura cambie de tamano.
fn determinar_foco(objetivo: &Elemento, punta: Punto2, otro: Punto2) -> f32 {
    let (x0, y0, x1, y1) = objetivo.caja();
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (dx, dy) = (punta.x - otro.x, punta.y - otro.y);
    let largo = (dx * dx + dy * dy).sqrt();
    if largo == 0.0 {
        return 0.0;
    }
    // Distancia con signo del centro a la recta que forma la flecha.
    let con_signo = ((centro.x - otro.x) * dy - (centro.y - otro.y) * dx) / largo;
    let semi = ((x1 - x0) / 2.0).max((y1 - y0) / 2.0);
    if semi == 0.0 {
        return 0.0;
    }
    (-con_signo / semi).clamp(-1.0, 1.0)
}

/// Donde debe posarse la punta sobre el borde de `objetivo` (`bindingPointOn`).
///
/// Se traza un rayo desde `desde` hacia el punto de enfoque y se corta con la
/// silueta; luego se retrocede el hueco. El resultado es que la punta toca el
/// borde sin invadirlo, venga de donde venga.
pub fn punto_de_enganche(objetivo: &Elemento, desde: Punto2, b: &Enganche) -> Punto2 {
    let (x0, y0, x1, y1) = objetivo.caja();
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);

    // **Modo «dentro»: manda el punto agarrado, no el contorno.** Es lo que
    // hace que la flecha se quede senalando el sitio exacto donde se solto y
    // lo siga al mover o estirar la figura, en vez de saltar al borde mas
    // cercano.
    if b.modo == ModoEnganche::Dentro {
        if let Some((fx, fy)) = b.punto_fijo {
            let sin_girar = Punto2::nuevo(x0 + fx * (x1 - x0), y0 + fy * (y1 - y0));
            return sin_girar.girar(centro, objetivo.angulo);
        }
    }

    let (dx, dy) = (centro.x - desde.x, centro.y - desde.y);
    let largo = (dx * dx + dy * dy).sqrt();
    if largo == 0.0 {
        return centro;
    }
    // El enfoque desplaza el objetivo perpendicular a la linea.
    let semi = ((x1 - x0) / 2.0).max((y1 - y0) / 2.0);
    let desvio = b.foco * semi;
    let hacia = Punto2::nuevo(
        centro.x + (-dy / largo) * desvio,
        centro.y + (dx / largo) * desvio,
    );

    let Some(corte) = cortar_silueta(objetivo, desde, hacia) else {
        return hacia;
    };
    // Retroceder el hueco por la misma direccion de llegada.
    let (bx, by) = (corte.x - desde.x, corte.y - desde.y);
    let blargo = (bx * bx + by * by).sqrt();
    if blargo == 0.0 {
        return corte;
    }
    Punto2::nuevo(
        corte.x - (bx / blargo) * b.hueco,
        corte.y - (by / blargo) * b.hueco,
    )
}

/// El primer corte del segmento `desde -> hasta` con la silueta de `e`.
///
/// Va por [`crate::perimetros`] y no por una formula por tipo —el rectangulo
/// contra sus lados, la elipse contra su ecuacion, como hace el movil—
/// porque aquel ya reduce **cualquier** figura a tramos rectos y ya la
/// entrega girada. Una formula por tipo seria una segunda verdad sobre por
/// donde pasa cada figura, y el dia que una de las dos aprenda un tipo nuevo
/// y la otra no, la flecha se posaria en un borde que no es el que se pinta.
fn cortar_silueta(e: &Elemento, desde: Punto2, hasta: Punto2) -> Option<Punto2> {
    let mut mejor: Option<(f32, Punto2)> = None;
    for (a, b) in perimetros::segmentos_de(e, PASO_PERIMETRO) {
        if let Some(q) = perimetros::interseccion(desde, hasta, a, b) {
            let d = q.distancia(desde);
            if mejor.is_none_or(|(mejor_d, _)| d < mejor_d) {
                mejor = Some((d, q));
            }
        }
    }
    mejor.map(|(_, q)| q)
}

/// El id interno que le toca a un id de texto del fichero.
///
/// Es **la misma cuenta** que hace `excalidraw::leer` al entrar (FNV-1a de 64
/// bits). Esta repetida aqui porque alli es privada y `excalidraw.rs` no es
/// de este grupo; la prueba `un_enganche_del_movil_encuentra_a_su_figura` la
/// ata a la de alla, asi que si una cambia, la otra falla.
///
/// El prefijo `pc` es la puerta para lo que nace en el escritorio: ver la
/// nota de la cabecera sobre lo que todavia no se puede cerrar.
pub fn id_del_fichero(texto: &str) -> u64 {
    if let Some(hex) = texto.strip_prefix("pc") {
        if let Ok(n) = u64::from_str_radix(hex, 16) {
            return n;
        }
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in texto.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// El id de texto con el que este grupo ata un elemento nacido en el PC.
pub fn id_de_texto(id: u64) -> String {
    format!("pc{id:x}")
}

/// El elemento al que apunta un enganche, si sigue existiendo.
pub fn resolver<'a>(elementos: &'a [Elemento], b: &Enganche) -> Option<&'a Elemento> {
    let id = id_del_fichero(&b.elemento);
    elementos.iter().find(|e| e.id == id && !e.borrado)
}

/// Recoloca los extremos atados de `flecha` (`updateBoundPoints`).
///
/// Devuelve si movio algo. **Un enganche que no resuelve no es un error**: la
/// figura puede estar borrada, o venir de un movil con un id que aqui no se
/// sabe deshacer. En ese caso la flecha se queda donde esta y su enganche
/// sigue viajando intacto al fichero, que es lo unico que se le puede
/// prometer.
pub fn recolocar(flecha: &mut Elemento, elementos: &[Elemento]) -> bool {
    if flecha.extras.enganche_inicio.is_none() && flecha.extras.enganche_fin.is_none() {
        return false;
    }
    let Figura::Flecha { puntos, .. } = &flecha.figura else {
        return false;
    };
    if puntos.len() < 2 {
        return false;
    }
    let (primero, ultimo) = (puntos[0], puntos[puntos.len() - 1]);

    let nuevo_inicio = flecha
        .extras
        .enganche_inicio
        .as_ref()
        .and_then(|b| resolver(elementos, b).map(|o| punto_de_enganche(o, ultimo, b)));
    let nuevo_fin = flecha
        .extras
        .enganche_fin
        .as_ref()
        .and_then(|b| resolver(elementos, b).map(|o| punto_de_enganche(o, primero, b)));

    if nuevo_inicio.is_none() && nuevo_fin.is_none() {
        return false;
    }
    let Figura::Flecha { puntos, .. } = &mut flecha.figura else {
        return false;
    };
    let ultimo_indice = puntos.len() - 1;
    if let Some(p) = nuevo_inicio {
        puntos[0] = p;
    }
    if let Some(p) = nuevo_fin {
        puntos[ultimo_indice] = p;
    }
    // `x`/`y` pasan a ser el primer punto, como en el movil: los puntos se
    // escriben en el fichero relativos a ellos.
    flecha.x = puntos[0].x;
    flecha.y = puntos[0].y;
    flecha.tocar();
    true
}

/// Las flechas de la escena que cuelgan de alguno de estos elementos.
///
/// Se mira el enganche de la flecha y no la lista `atados` de la figura: son
/// dos verdades sobre lo mismo y la del movil puede venir desactualizada de
/// un editor de terceros, mientras que el enganche es el que de verdad decide
/// donde se posa la punta.
pub fn flechas_colgando_de(elementos: &[Elemento], ids: &[u64]) -> Vec<u64> {
    elementos
        .iter()
        .filter(|e| !e.borrado && matches!(e.figura, Figura::Flecha { .. }))
        .filter(|e| {
            [&e.extras.enganche_inicio, &e.extras.enganche_fin]
                .into_iter()
                .flatten()
                .any(|b| ids.contains(&id_del_fichero(&b.elemento)))
        })
        .map(|e| e.id)
        .collect()
}

/// **Lo que hay que llamar despues de mover, estirar o girar algo.**
///
/// Recoloca las flechas que colgaban de `movidos` dentro del paso de deshacer
/// que este abierto: la flecha que sigue a la caja tiene que deshacerse con
/// ella, no en un segundo Ctrl+Z que descoloca el dibujo a medias.
///
/// Devuelve cuantas flechas se recolocaron.
pub fn seguir(escena: &mut Escena, movidos: &[u64]) -> usize {
    let instantanea: Vec<Elemento> = escena.visibles().cloned().collect();
    let flechas = flechas_colgando_de(&instantanea, movidos);
    let mut cuantas = 0;
    for id in flechas {
        if movidos.contains(&id) {
            // Una flecha que se mueve con su caja ya va donde tiene que ir:
            // recolocarla ademas la estiraria hacia un objetivo que tambien
            // se movio, y el arrastre de una seleccion entera se deformaria.
            continue;
        }
        escena.apuntar_edicion(id);
        let Some(f) = escena.buscar_mut(id) else {
            continue;
        };
        let mut copia = f.clone();
        if recolocar(&mut copia, &instantanea) {
            *escena.buscar_mut(id).expect("acaba de encontrarse") = copia;
            cuantas += 1;
        }
    }
    cuantas
}

/// Suelta un extremo (`unbindBindingElement`).
pub fn desatar(flecha: &mut Elemento, extremo: Extremo) {
    match extremo {
        Extremo::Inicio => flecha.extras.enganche_inicio = None,
        Extremo::Fin => flecha.extras.enganche_fin = None,
    }
    flecha.tocar();
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::ColorRgba;
    use crate::formas::TipoPunta;

    fn caja(id: u64, figura: Figura, x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id,
            figura,
            x,
            y,
            ancho,
            alto,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            ..Default::default()
        }
    }

    fn flecha(id: u64, a: Punto2, b: Punto2) -> Elemento {
        let mut e = caja(id, Figura::Rectangulo, a.x, a.y, b.x - a.x, b.y - a.y);
        e.figura = Figura::Flecha {
            puntos: vec![a, b],
            punta_inicio: TipoPunta::Ninguna,
            punta_fin: TipoPunta::Flecha,
            codos: false,
        };
        e
    }

    #[test]
    fn la_punta_se_posa_en_el_borde_y_no_dentro_de_la_caja() {
        let objetivo = caja(1, Figura::Rectangulo, 100.0, 0.0, 100.0, 100.0);
        let b = Enganche {
            elemento: id_de_texto(1),
            foco: 0.0,
            hueco: hueco_de(&objetivo),
            punto_fijo: None,
            modo: ModoEnganche::Orbita,
        };
        let q = punto_de_enganche(&objetivo, Punto2::nuevo(0.0, 50.0), &b);
        // El borde izquierdo esta en x=100; la punta se queda el hueco antes.
        assert!(
            (q.x - (100.0 - b.hueco)).abs() < 0.5,
            "se poso en {q:?}, no rozando el borde"
        );
        assert!((q.y - 50.0).abs() < 0.5);
    }

    #[test]
    fn mover_la_caja_arrastra_la_flecha_consigo() {
        // La razon de ser del modulo, y el fallo que se viene a arreglar: un
        // organigrama del movil abierto aqui se descolocaba en cuanto se
        // movia una caja.
        let objetivo = caja(1, Figura::Rectangulo, 100.0, 0.0, 100.0, 100.0);
        let mut f = flecha(2, Punto2::nuevo(0.0, 50.0), Punto2::nuevo(95.0, 50.0));
        f.extras.enganche_fin = Some(enganche_para(
            &objetivo,
            id_de_texto(1),
            Punto2::nuevo(95.0, 50.0),
            Punto2::nuevo(0.0, 50.0),
        ));

        let mut movida = objetivo.clone();
        movida.mover(200.0, 0.0);
        assert!(recolocar(&mut f, &[movida]));
        let Figura::Flecha { puntos, .. } = &f.figura else {
            unreachable!()
        };
        assert!(
            puntos[1].x > 280.0,
            "la punta tenia que seguir a la caja hasta x=300, esta en {:?}",
            puntos[1]
        );
    }

    #[test]
    fn el_foco_desvia_el_contacto_del_centro_tambien_con_la_caja_mas_grande() {
        // Para esto existe `foco`: sin el, todas las flechas se posan
        // apuntando al centro y un esquema con seis entrando por sitios
        // distintos se convierte en un abanico. El foco se guarda como
        // proporcion del semitamano, asi que **sigue desviando despues de
        // estirar la caja**, que es lo que aqui se comprueba.
        let objetivo = caja(1, Figura::Rectangulo, 100.0, 0.0, 100.0, 100.0);
        let punta = Punto2::nuevo(95.0, 20.0);
        let otro = Punto2::nuevo(0.0, 20.0);
        let b = enganche_para(&objetivo, id_de_texto(1), punta, otro);
        assert!(
            b.foco != 0.0,
            "entra por arriba del centro: el foco no es 0"
        );
        let centrado = Enganche {
            foco: 0.0,
            ..b.clone()
        };

        let mut grande = objetivo.clone();
        grande.ancho = 200.0;
        grande.alto = 200.0;
        let con_foco = punto_de_enganche(&grande, otro, &b);
        let al_centro = punto_de_enganche(&grande, otro, &centrado);
        assert!(
            con_foco.distancia(al_centro) > 1.0,
            "el foco dejo de desviar: los dos se posan en {con_foco:?}"
        );
    }

    #[test]
    fn el_modo_dentro_se_queda_donde_se_solto_y_sigue_a_la_figura() {
        // Soltar la punta bien dentro de una caja significa «senala AQUI», no
        // «senala a esta caja».
        let objetivo = caja(1, Figura::Rectangulo, 0.0, 0.0, 200.0, 200.0);
        let punta = Punto2::nuevo(150.0, 50.0);
        let b = enganche_para(
            &objetivo,
            id_de_texto(1),
            punta,
            Punto2::nuevo(-500.0, 50.0),
        );
        assert_eq!(b.modo, ModoEnganche::Dentro);
        let q = punto_de_enganche(&objetivo, Punto2::nuevo(-500.0, 50.0), &b);
        assert!(q.distancia(punta) < 0.01, "se movio a {q:?}");

        // Y al mover la figura, el punto agarrado va con ella.
        let mut movida = objetivo.clone();
        movida.mover(10.0, 10.0);
        let q = punto_de_enganche(&movida, Punto2::nuevo(-500.0, 50.0), &b);
        assert!(q.distancia(Punto2::nuevo(160.0, 60.0)) < 0.01, "{q:?}");
    }

    #[test]
    fn entre_una_caja_dentro_de_otra_gana_la_pequena() {
        // Regla 3 del original: anclar a la de fuera nunca es lo que se
        // quiere.
        let grande = caja(1, Figura::Rectangulo, 0.0, 0.0, 400.0, 400.0);
        let pequena = caja(2, Figura::Rectangulo, 150.0, 150.0, 60.0, 60.0);
        assert_eq!(
            figura_bajo(&[grande, pequena], Punto2::nuevo(180.0, 180.0), 1.0, 99),
            Some(2)
        );
    }

    #[test]
    fn una_punta_que_cae_lejos_no_ancla_a_nadie() {
        // Caso negativo: lo que se suelta a una distancia razonable se queda
        // donde se solto. Estuvo en quince pixeles —el numero del original— y
        // media pantalla alrededor de cada caja era zona de captura.
        let objetivo = caja(1, Figura::Rectangulo, 100.0, 0.0, 100.0, 100.0);
        assert_eq!(
            figura_bajo(
                std::slice::from_ref(&objetivo),
                Punto2::nuevo(60.0, 50.0),
                1.0,
                99
            ),
            None
        );
        // Rozando el borde si.
        assert_eq!(
            figura_bajo(&[objetivo], Punto2::nuevo(97.0, 50.0), 1.0, 99),
            Some(1)
        );
    }

    #[test]
    fn a_una_raya_o_a_un_marco_no_se_ata_nada() {
        // Caso negativo: atar flecha con flecha hace cadenas que se
        // persiguen, y el marco es el papel, no un dibujo.
        let mut raya = caja(1, Figura::Rectangulo, 0.0, 0.0, 100.0, 0.0);
        raya.figura = Figura::Linea {
            puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
        };
        let marco = caja(
            2,
            Figura::Marco {
                nombre: "hoja".into(),
            },
            0.0,
            0.0,
            500.0,
            500.0,
        );
        assert!(!se_puede_atar(&raya));
        assert!(!se_puede_atar(&marco));
        assert_eq!(
            figura_bajo(&[raya, marco], Punto2::nuevo(50.0, 0.0), 1.0, 99),
            None
        );
    }

    #[test]
    fn lo_bloqueado_no_recibe_flechas() {
        let mut e = caja(1, Figura::Rectangulo, 0.0, 0.0, 100.0, 100.0);
        e.bloqueado = true;
        assert!(!se_puede_atar(&e));
    }

    #[test]
    fn un_enganche_que_no_resuelve_deja_la_flecha_quieta() {
        // Caso negativo y de los importantes: una figura borrada, o un id de
        // un movil que aqui no se sabe deshacer, no puede mover la punta a
        // ninguna parte ni hacer saltar nada.
        let mut f = flecha(2, Punto2::nuevo(0.0, 0.0), Punto2::nuevo(50.0, 0.0));
        f.extras.enganche_fin = Some(Enganche {
            elemento: "no-existe".into(),
            foco: 0.0,
            hueco: 5.0,
            punto_fijo: None,
            modo: ModoEnganche::Orbita,
        });
        let antes = f.clone();
        assert!(!recolocar(&mut f, &[]));
        assert_eq!(f, antes, "ni los puntos ni la version");
    }

    #[test]
    fn una_flecha_sin_enganches_no_se_toca() {
        let mut f = flecha(2, Punto2::nuevo(0.0, 0.0), Punto2::nuevo(50.0, 0.0));
        let antes = f.clone();
        assert!(!recolocar(&mut f, &[]));
        assert_eq!(f, antes);
    }

    #[test]
    fn un_enganche_del_movil_encuentra_a_su_figura() {
        // **La prueba que ata este modulo a `excalidraw.rs`.** El id interno
        // de un elemento leido del fichero lo calcula alli una funcion
        // privada; aqui se repite la cuenta para poder deshacer un
        // `startBinding`. Si una de las dos cambia, esto falla.
        let json = r##"{"elements":[
            {"id":"a1","type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "strokeColor":"#000000","seed":1}
        ]}"##;
        let lienzo = crate::excalidraw::leer(json).expect("json valido");
        let id_interno = lienzo
            .entradas
            .iter()
            .find_map(|e| match e {
                crate::excalidraw::Entrada::Nuestro { elemento, .. } => Some(elemento.id),
                crate::excalidraw::Entrada::Ajeno(_) => None,
            })
            .expect("el rectangulo entra por el carril nuestro");
        assert_eq!(
            id_del_fichero("a1"),
            id_interno,
            "la cuenta de los ids se ha separado de la de excalidraw.rs"
        );
    }

    #[test]
    fn el_id_de_texto_del_pc_va_y_vuelve() {
        assert_eq!(id_del_fichero(&id_de_texto(48_879)), 48_879);
    }

    #[test]
    fn seguir_recoloca_dentro_del_mismo_paso_de_deshacer() {
        // La flecha que sigue a la caja tiene que deshacerse CON ella: si
        // abriera su propio paso, un solo Ctrl+Z dejaria la caja en su sitio
        // viejo y la flecha en el nuevo.
        let mut escena = Escena::nueva();
        let caja_id = escena.anadir(caja(0, Figura::Rectangulo, 100.0, 0.0, 100.0, 100.0));
        let objetivo = escena.buscar(caja_id).unwrap().clone();
        let mut f = flecha(0, Punto2::nuevo(0.0, 50.0), Punto2::nuevo(95.0, 50.0));
        f.extras.enganche_fin = Some(enganche_para(
            &objetivo,
            id_de_texto(caja_id),
            Punto2::nuevo(95.0, 50.0),
            Punto2::nuevo(0.0, 50.0),
        ));
        let flecha_id = escena.anadir(f);

        escena.abrir_paso();
        escena.apuntar_movimiento(caja_id, 200.0, 0.0);
        escena.buscar_mut(caja_id).unwrap().mover(200.0, 0.0);
        assert_eq!(seguir(&mut escena, &[caja_id]), 1);
        escena.cerrar_paso();

        let punta_movida = match &escena.buscar(flecha_id).unwrap().figura {
            Figura::Flecha { puntos, .. } => puntos[1],
            _ => unreachable!(),
        };
        assert!(punta_movida.x > 280.0);

        assert!(escena.deshacer());
        let punta_vuelta = match &escena.buscar(flecha_id).unwrap().figura {
            Figura::Flecha { puntos, .. } => puntos[1],
            _ => unreachable!(),
        };
        assert!(
            (punta_vuelta.x - 95.0).abs() < 0.01,
            "un solo Ctrl+Z tiene que devolver las dos cosas: {punta_vuelta:?}"
        );
    }

    #[test]
    fn una_flecha_que_se_mueve_con_su_caja_no_se_estira_ademas() {
        // Arrastrar la caja Y la flecha seleccionadas a la vez: si ademas se
        // recolocara la punta, el conjunto se deformaria.
        let mut escena = Escena::nueva();
        let caja_id = escena.anadir(caja(0, Figura::Rectangulo, 100.0, 0.0, 100.0, 100.0));
        let objetivo = escena.buscar(caja_id).unwrap().clone();
        let mut f = flecha(0, Punto2::nuevo(0.0, 50.0), Punto2::nuevo(95.0, 50.0));
        f.extras.enganche_fin = Some(enganche_para(
            &objetivo,
            id_de_texto(caja_id),
            Punto2::nuevo(95.0, 50.0),
            Punto2::nuevo(0.0, 50.0),
        ));
        let flecha_id = escena.anadir(f);
        escena.abrir_paso();
        assert_eq!(seguir(&mut escena, &[caja_id, flecha_id]), 0);
        escena.cerrar_paso();
    }
}
