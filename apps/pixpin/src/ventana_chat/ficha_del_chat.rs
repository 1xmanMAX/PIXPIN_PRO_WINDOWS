//! **La ficha del chat**: la isla flotante que sale al pulsar el nombre en
//! la cabecera (25-oct-2026, el usuario: «su UI es mala. Mira Telegram: es
//! una isla flotante que muestra el chat, fotos, videos, archivos, enlaces…
//! muy bien presentado, y hacer click dentro no debe salir de esa zona»; y
//! despues: «funciona pero no funciona bien y no se ve bien»).
//!
//! Hecha como el panel de medios compartidos de Telegram Desktop:
//!
//! - **Arriba**, el avatar redondo del chat, su nombre en seminegrita y
//!   cuantas cosas tiene, con la lupa y el aspa en botones redondos.
//! - **Las pestanas**, texto con la rayita de color bajo la activa y su
//!   cuenta apagada detras. Solo salen las que tienen algo; si no caben, la
//!   tira se desplaza con la rueda y la activa siempre se ve.
//! - **El contenido, por meses** («octubre de 2026»), lo mas nuevo arriba:
//!   lo que se mira (fotos, videos, dibujos, hojas) en una rejilla de borde
//!   a borde de tres a cinco columnas, con su recuadro mientras carga y, en
//!   los videos, la insignia con el triangulo y lo que dura; lo que se lee
//!   en filas: archivos con la chapa de color de su extension, enlaces con
//!   la letra de su sitio, su direccion y lo que decia el mensaje, notas de
//!   voz con su boton de reproducir (suenan sin salir de la isla).
//! - **Lo que se pulsa se ve DENTRO**: la previa en grande, sobre fondo
//!   oscuro, con las flechas encima a los lados (o ← y →) y, en la cabecera,
//!   volver, «Ver en el chat» y «Abrir». Un enlace se abre en el navegador y
//!   una nota de voz suena; la isla se queda.
//! - **Con el teclado**: ← y → cambian de pestana; ↓ entra en el contenido
//!   y las flechas mueven el foco; Intro abre; Escape vuelve (de la previa a
//!   la rejilla, del foco a las pestanas) y, al final, cierra.
//!
//! **Los proyectos viven aqui** (25-oct-2026, el usuario: «quita la seccion
//! de proyectos o al menos muevela: quiero que esta zona de proyectos se
//! mueva al menu que se abre al darle click en la parte de arriba del
//! chat»). La pantalla aparte de Proyectos ya no existe; su sitio es la
//! primera pestana de la isla, con la que se abre: cada proyecto en su fila,
//! en el orden de la lista de chats, con su avatar, su nombre, cuantas hojas
//! tiene, su ultima linea y cuando se toco por ultima vez. La lupa los
//! filtra por nombre. Pulsar **otro** proyecto es salir a proposito: la isla
//! se cierra y el chat pasa a ese proyecto. Pulsar **el de este chat** no
//! sale: lleva a su «Galeria», que es lo que ensenaba su tarjeta (sus hojas).
//!
//! **Las fotos de las notas ricas** (`![…](pixpin:files/…)` en su texto)
//! salen en «Fotos» cada una como una miniatura mas, en el sitio de su nota,
//! y se ven en su previa como cualquier otra foto ([`Cosa`] con `foto`).
//!
//! Pintar y repartir los clics salen de las mismas cuentas
//! (`pixpin_ui::info`, [`colocar`] y [`zona_en`]): cuando eran dos, se veian
//! botones que no respondian.

use std::cell::Cell;

use pixpin_geom::{Punto, Rect};
use pixpin_proyecto::almacen::Ficha;
use pixpin_proyecto::cuaderno::{Clase, Mensaje, Seccion};
use pixpin_render::icono::Icono;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_ui::chat::{self, Disposicion};
use pixpin_ui::info::{self as ui, Cosa, Isla, Pieza};

use super::{
    Abierto, Ojeada, Pinta, color_avatar, colores_de_caja, con_alfa, encoger,
    indices_de_la_seccion, iniciales, logo, mi, negritas, papel_de_vista, pintar_lienzo,
    pintar_ojeada_tabla, pintar_tarjeta_galeria, rf, ruta_del_mensaje, tarjetas,
};

/// En que orden salen las pestanas: los proyectos delante (es con la que se
/// abre la isla), luego lo que se mira, lo que se lee, y «Todo» y el buzon
/// al final.
pub(super) const ORDEN: [Seccion; 11] = [
    Seccion::Proyectos,
    Seccion::Galeria,
    Seccion::Fotos,
    Seccion::Videos,
    Seccion::Archivos,
    Seccion::Enlaces,
    Seccion::Voz,
    Seccion::Dibujos,
    Seccion::Fijados,
    Seccion::Todo,
    Seccion::Buzon,
];

/// Lo que la isla recuerda entre fotograma y fotograma aparte de la
/// pestana y el desplazamiento: lo desplazada que esta la tira de pestanas,
/// para cual se desplazo por ultima vez (al cambiar de pestana se lleva la
/// activa a la vista) y que cosa tiene el foco del teclado.
#[derive(Debug, Default)]
pub(super) struct Navegacion {
    pub pestanas_x: Cell<i32>,
    pub pestana_vista: Cell<Option<Seccion>>,
    pub foco: Cell<Option<Cosa>>,
}

/// La ventana entera, que es lo que tapa el velo.
fn ventana_de(d: &Disposicion) -> Rect {
    Rect {
        x: 0,
        y: 0,
        ancho: d.barra.ancho,
        alto: d.chat.abajo().max(d.lista.abajo()).max(0) as u32,
    }
}

/// **Donde cae la isla**, para pintarla y para repartir sus clics: centrada
/// bajo la barra de titulo, que sigue siendo de la ventana (moverla,
/// cerrarla) y no se tapa.
pub(super) fn disposicion(d: &Disposicion, escala: u32) -> Isla {
    let todo = ventana_de(d);
    let bajo_la_barra = Rect {
        y: d.barra.abajo(),
        alto: (todo.abajo() - d.barra.abajo()).max(0) as u32,
        ..todo
    };
    let (caja, completa) = ui::isla_en(bajo_la_barra, escala);
    Isla::calcular(caja, completa, escala)
}

/// Las pestanas que salen, con cuantas cosas tiene cada una: las que tienen
/// algo y, aunque este vacia, la que esta puesta.
pub(super) fn pestanas(a: &Abierto) -> Vec<(Seccion, usize)> {
    ORDEN
        .iter()
        .map(|s| (*s, cosas_de(a, *s).len()))
        .filter(|(s, n)| *n > 0 || a.info == Some(*s))
        .collect()
}

/// La pestana con la que se abre: la primera que tenga algo; en un chat
/// vacio, «Todo», que dice que no hay nada.
pub(super) fn primera(a: &Abierto) -> Seccion {
    ORDEN
        .iter()
        .copied()
        .find(|s| !cosas_de(a, *s).is_empty())
        .unwrap_or(Seccion::Todo)
}

/// Todo lo de la seccion, **lo mas nuevo primero** (como Telegram): sus
/// mensajes y, en «Fotos», tambien cada foto de las notas ricas, en el
/// sitio de su nota y en el orden de su burbuja.
fn cosas_de(a: &Abierto, seccion: Seccion) -> Vec<Cosa> {
    // Los proyectos no son mensajes: cada cosa es el proyecto de ese lugar
    // en `proyectos_info`, en el orden de la lista de chats.
    if seccion == Seccion::Proyectos {
        return (0..a.proyectos_info.len()).map(Cosa::entera).collect();
    }
    let suyos = indices_de_la_seccion(a, seccion);
    let mut v = if seccion != Seccion::Fotos {
        suyos.into_iter().map(Cosa::entera).collect()
    } else {
        // Las notas, de las de «Todo» (sin buzon, sistema ni lecciones), y
        // solo las que la burbuja pinta como nota rica.
        let notas: Vec<usize> = indices_de_la_seccion(a, Seccion::Todo)
            .into_iter()
            .filter(|n| {
                let m = &a.mensajes[*n];
                m.texto.contains("![") && super::nota_rica::es_nota(m)
            })
            .collect();
        ui::con_las_fotos_de_las_notas(&suyos, &notas, |n| {
            ui::fotos_de_la_nota(&a.mensajes[n].texto).len()
        })
    };
    v.sort_by_key(|c| (std::cmp::Reverse(c.mensaje), c.foto));
    v
}

/// Lo que ensena la seccion, con lo buscado con la lupa ya filtrado.
pub(super) fn de_la_seccion(a: &Abierto, seccion: Seccion) -> Vec<Cosa> {
    let mut suyos = cosas_de(a, seccion);
    if seccion == Seccion::Proyectos {
        if !a.busqueda_info.trim().is_empty() {
            suyos.retain(|c| {
                a.proyectos_info.get(c.mensaje).is_some_and(|f| {
                    pixpin_ui::resaltado::hay_coincidencia(&f.nombre, &a.busqueda_info)
                })
            });
        }
        return suyos;
    }
    if !a.busqueda_info.trim().is_empty() {
        suyos.retain(|c| {
            let Some(m) = a.mensajes.get(c.mensaje) else {
                return false;
            };
            pixpin_ui::resaltado::hay_coincidencia(&m.resumen(), &a.busqueda_info)
                || pixpin_ui::resaltado::hay_coincidencia(&m.nombre, &a.busqueda_info)
        });
    }
    suyos
}

/// Lo alta que es cada fila de la seccion: los enlaces llevan tres lineas.
fn alto_de_fila(seccion: Seccion) -> u32 {
    match seccion {
        Seccion::Enlaces => ui::ISLA_FILA_ENLACE,
        _ => ui::ISLA_FILA,
    }
}

/// El mes de un mensaje, en la hora de este equipo, como una clave que
/// cambia de un mes al siguiente.
fn clave_del_mes(m: &Mensaje) -> i64 {
    let (ano, mes, _) = chat::partes_fecha(pixpin_shell::entorno::a_local(m.cuando));
    ano * 12 + mes as i64
}

/// Como queda el contenido de una seccion: lo que hay y donde.
pub(super) struct Colocado {
    /// Todo lo de la seccion, en el orden en que se ensena.
    pub suyos: Vec<Cosa>,
    /// Sus rotulos de mes y sus celdas o filas (`Pieza::Cosa(n)` es
    /// `suyos[n]`), sin desplazar.
    pub lista: ui::Lista,
    pub cuadricula: bool,
}

impl Colocado {
    /// Lo que se ve con el desplazamiento de ahora, ya en su sitio.
    pub fn visibles(&self, i: &Isla, scroll: i32) -> Vec<(Pieza, Rect)> {
        self.lista.visibles(i.contenido, scroll)
    }

    /// En que lugar de la lista va la cosa `c`.
    fn lugar_de(&self, c: Cosa) -> Option<usize> {
        self.suyos.iter().position(|x| *x == c)
    }
}

/// **Coloca el contenido** de la seccion en la isla: la misma cuenta para
/// pintar, para el clic, para el resalte del raton, para el foco y para
/// cargar las fotos.
pub(super) fn colocar(a: &Abierto, seccion: Seccion, i: &Isla, escala: u32) -> Colocado {
    let suyos = de_la_seccion(a, seccion);
    if seccion == Seccion::Proyectos {
        // Sin rotulos de mes: van en el orden de la lista de chats.
        let lista = ui::filas_isla(i.contenido, suyos.len(), ui::ISLA_FILA, escala);
        return Colocado {
            suyos,
            lista,
            cuadricula: false,
        };
    }
    let meses: Vec<i64> = suyos
        .iter()
        .map(|c| a.mensajes.get(c.mensaje).map_or(0, clave_del_mes))
        .collect();
    let cuadricula = seccion.es_cuadricula();
    let lista = ui::lista_isla(
        i.contenido,
        &meses,
        cuadricula,
        alto_de_fila(seccion),
        escala,
    );
    Colocado {
        suyos,
        lista,
        cuadricula,
    }
}

/// Las pestanas colocadas en la tira con lo que mide cada rotulo (lo apunta
/// el pintado) y lo desplazada que esta.
fn pestanas_colocadas(a: &Abierto, i: &Isla, escala: u32) -> ui::Pestanas {
    let anchos = a.anchos_pestanas.borrow();
    i.pestanas(&anchos, a.nav_info.pestanas_x.get(), escala)
}

/// La anterior (`paso` -1) o la siguiente (+1) a la que se esta viendo en
/// la previa, dentro de su seccion.
pub(super) fn vecina(a: &Abierto, paso: i32) -> Option<Cosa> {
    let (seccion, actual) = (a.info?, a.info_abierta?);
    let suyos = de_la_seccion(a, seccion);
    let k = suyos.iter().position(|n| *n == actual)? as i32 + paso;
    (k >= 0).then(|| suyos.get(k as usize).copied()).flatten()
}

/// Lo que hay bajo el raton en la isla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ZonaInfo {
    /// Fuera de la isla, en el velo: cierra.
    Fuera,
    Cerrar,
    Buscar,
    CajaBuscar,
    Pestana(Seccion),
    /// Una cosa de la rejilla o de las filas.
    Cosa(Cosa),
    /// Un proyecto de la pestana de proyectos, por su lugar en
    /// `proyectos_info`.
    Proyecto(usize),
    /// En la previa: la flecha de volver a la rejilla, las de los lados y
    /// los dos botones de la cabecera.
    Volver,
    Anterior,
    Siguiente,
    Abrir,
    EnElChat,
    /// El avatar de la cabecera: cambiar la foto del chat (25-oct-2026).
    Avatar,
    /// Dentro de la isla, sin nada que hacer: no cierra.
    Dentro,
}

/// **Que hay en `l`**. Las mismas cuentas que el pintado.
pub(super) fn zona_en(a: &Abierto, l: Punto, d: &Disposicion, escala: u32) -> ZonaInfo {
    let Some(seccion) = a.info else {
        return ZonaInfo::Fuera;
    };
    let i = disposicion(d, escala);
    if !i.panel.contiene(l) {
        return ZonaInfo::Fuera;
    }
    if i.cerrar.contiene(l) {
        return ZonaInfo::Cerrar;
    }
    if a.info_abierta.is_some() {
        let p = ui::previa(i.hueco_de_la_previa(), escala);
        return if i.avatar.contiene(l) {
            ZonaInfo::Volver
        } else if i.abrir.contiene(l) {
            ZonaInfo::Abrir
        } else if i.en_el_chat.contiene(l) {
            ZonaInfo::EnElChat
        } else if p.anterior.contiene(l) && vecina(a, -1).is_some() {
            ZonaInfo::Anterior
        } else if p.siguiente.contiene(l) && vecina(a, 1).is_some() {
            ZonaInfo::Siguiente
        } else {
            ZonaInfo::Dentro
        };
    }
    if i.buscar.contiene(l) {
        return ZonaInfo::Buscar;
    }
    if a.buscando_info && i.caja_buscar.contiene(l) {
        return ZonaInfo::CajaBuscar;
    }
    // Con la lupa apagada, el avatar de arriba abre la hoja de su foto.
    if !a.buscando_info && i.avatar.contiene(l) {
        return ZonaInfo::Avatar;
    }
    if i.tira.contiene(l) {
        let lista = pestanas(a);
        let colocadas = pestanas_colocadas(a, &i, escala);
        if colocadas.rects.len() == lista.len()
            && let Some(n) = colocadas.en(l, i.tira)
        {
            return ZonaInfo::Pestana(lista[n].0);
        }
        return ZonaInfo::Dentro;
    }
    if i.contenido.contiene(l) {
        let c = colocar(a, seccion, &i, escala);
        if let Some(n) = c
            .visibles(&i, a.scroll_info)
            .iter()
            .find_map(|(p, r)| match p {
                Pieza::Cosa(n) if r.contiene(l) => Some(*n),
                _ => None,
            })
        {
            return if seccion == Seccion::Proyectos {
                ZonaInfo::Proyecto(c.suyos[n].mensaje)
            } else {
                ZonaInfo::Cosa(c.suyos[n])
            };
        }
    }
    ZonaInfo::Dentro
}

/// Hasta donde baja la rueda en la seccion abierta.
pub(super) fn tope(a: &Abierto, d: &Disposicion, escala: u32) -> i32 {
    let i = disposicion(d, escala);
    (a.alto_info.get() as i32 - i.contenido.alto as i32).max(0)
}

/// **La rueda en la isla**: sobre la tira de pestanas, si no caben, la
/// desplaza de lado; en lo demas, el contenido. `giro` en pixeles, positivo
/// hacia arriba (como `giro_de_rueda`).
pub(super) fn rueda(a: &mut Abierto, d: &Disposicion, escala: u32, giro: i32) {
    let i = disposicion(d, escala);
    if a.info_abierta.is_some() {
        return;
    }
    if i.tira.contiene(a.raton_info) {
        let p = pestanas_colocadas(a, &i, escala);
        if p.tope > 0 {
            a.nav_info
                .pestanas_x
                .set((p.desplazado - giro).clamp(0, p.tope));
            return;
        }
    }
    let tope = tope(a, d, escala);
    a.scroll_info = (a.scroll_info - giro).clamp(0, tope);
}

/// Lo que pide una tecla en la isla, para quien tiene con que hacerlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tecla {
    /// No es suya: sigue su camino.
    NoEsMia,
    /// Ya esta hecho (se movio el foco, se cambio de pestana…).
    Hecho,
    /// Intro sobre una cosa: lo mismo que pulsarla.
    Pulsar(Cosa),
    /// Intro sobre un proyecto de su pestana: lo mismo que pulsarlo.
    Proyecto(usize),
    /// Intro en la previa: lo mismo que «Abrir».
    Abrir,
    /// Escape sin nada mas que deshacer: se cierra la isla.
    Cerrar,
}

const VK_ESCAPE: u32 = 0x1B;
const VK_ENTRAR: u32 = 0x0D;
const VK_IZQUIERDA: u32 = 0x25;
const VK_ARRIBA: u32 = 0x26;
const VK_DERECHA: u32 = 0x27;
const VK_ABAJO: u32 = 0x28;

/// **El teclado en la isla**: ← y → cambian de pestana (o recorren la
/// previa); ↓ entra en el contenido y las flechas mueven el foco; Intro
/// abre; Escape vuelve un paso y, al final, cierra.
pub(super) fn tecla(a: &mut Abierto, vk: u32, d: &Disposicion, escala: u32) -> Tecla {
    let Some(seccion) = a.info else {
        return Tecla::NoEsMia;
    };
    if a.info_abierta.is_some() {
        return match vk {
            VK_ESCAPE => {
                a.info_abierta = None;
                Tecla::Hecho
            }
            VK_IZQUIERDA | VK_DERECHA => {
                let paso = if vk == VK_IZQUIERDA { -1 } else { 1 };
                a.info_abierta = vecina(a, paso).or(a.info_abierta);
                Tecla::Hecho
            }
            VK_ENTRAR => Tecla::Abrir,
            _ => Tecla::NoEsMia,
        };
    }
    let i = disposicion(d, escala);
    let c = colocar(a, seccion, &i, escala);
    let foco = a.nav_info.foco.get().and_then(|f| c.lugar_de(f));
    let poner_foco = |a: &mut Abierto, n: usize| {
        a.nav_info.foco.set(c.suyos.get(n).copied());
        a.scroll_info = c.lista.para_ver(n, i.contenido, a.scroll_info);
    };
    match (vk, foco) {
        (VK_ESCAPE, Some(_)) => {
            a.nav_info.foco.set(None);
            Tecla::Hecho
        }
        (VK_ESCAPE, None) => Tecla::Cerrar,
        (VK_ENTRAR, Some(n)) if seccion == Seccion::Proyectos => {
            Tecla::Proyecto(c.suyos[n].mensaje)
        }
        (VK_ENTRAR, Some(n)) => Tecla::Pulsar(c.suyos[n]),
        (VK_IZQUIERDA | VK_DERECHA, None) => {
            let lista = pestanas(a);
            let k = lista.iter().position(|(s, _)| *s == seccion).unwrap_or(0) as i32
                + if vk == VK_IZQUIERDA { -1 } else { 1 };
            if let Some((s, _)) = usize::try_from(k).ok().and_then(|k| lista.get(k)) {
                a.info = Some(*s);
                a.scroll_info = 0;
            }
            Tecla::Hecho
        }
        (VK_ABAJO, None) => {
            // Entra en lo que se ve: la primera cosa visible entera.
            if let Some(n) = c
                .visibles(&i, a.scroll_info)
                .iter()
                .find_map(|(p, r)| match p {
                    Pieza::Cosa(n) if r.y >= i.contenido.y => Some(*n),
                    _ => None,
                })
            {
                poner_foco(a, n);
            }
            Tecla::Hecho
        }
        (VK_IZQUIERDA | VK_DERECHA | VK_ARRIBA | VK_ABAJO, Some(n)) => {
            let (dx, dy) = match vk {
                VK_IZQUIERDA => (-1, 0),
                VK_DERECHA => (1, 0),
                VK_ARRIBA => (0, -1),
                _ => (0, 1),
            };
            match c.lista.vecina(n, dx, dy) {
                Some(k) => poner_foco(a, k),
                // Arriba del todo, el foco vuelve a las pestanas.
                None if vk == VK_ARRIBA => a.nav_info.foco.set(None),
                None => {}
            }
            Tecla::Hecho
        }
        _ => Tecla::NoEsMia,
    }
}

/// Las fotos que hay que tener leidas para el proximo fotograma: las
/// miniaturas de lo que se ve y, en la previa, la grande de la abierta. Y los
/// fondos de los lienzos que se ven (paginas de PDF), de las vistas previas.
pub(super) fn a_cargar(
    a: &Abierto,
    d: &Disposicion,
    escala: u32,
) -> (Vec<std::path::PathBuf>, Vec<std::path::PathBuf>) {
    // Los proyectos no tienen fotos que leer: sus logos van aparte
    // (`logos_a_preparar`).
    let Some(seccion) = a.info.filter(|s| *s != Seccion::Proyectos) else {
        return (Vec::new(), Vec::new());
    };
    let foto = |c: Cosa| ruta_de_la_foto(a, c);
    let fondo = |c: Cosa| match a.vistas.get(c.mensaje).filter(|_| c.foto.is_none()) {
        Some(Some(Ojeada::Lienzo(l))) => l.fondo.as_ref().map(|(r, _)| r.clone()),
        _ => None,
    };
    if let Some(n) = a.info_abierta {
        let grandes = foto(n).into_iter().chain(fondo(n)).collect();
        return (Vec::new(), grandes);
    }
    let i = disposicion(d, escala);
    let c = colocar(a, seccion, &i, escala);
    let cosas: Vec<Cosa> = c
        .visibles(&i, a.scroll_info)
        .iter()
        .filter_map(|(p, _)| match p {
            Pieza::Cosa(n) => Some(c.suyos[*n]),
            Pieza::Mes(_) => None,
        })
        .collect();
    let miniaturas = cosas.iter().filter_map(|n| foto(*n)).collect();
    let fondos = cosas.iter().filter_map(|n| fondo(*n)).collect();
    (miniaturas, fondos)
}

/// El fichero de la foto que es `c`: el de un mensaje de foto (no un video)
/// o, en una nota rica, el de su foto numero `k`. Solo si esta en el aparato.
pub(super) fn ruta_de_la_foto(a: &Abierto, c: Cosa) -> Option<std::path::PathBuf> {
    let m = a.mensajes.get(c.mensaje)?;
    match c.foto {
        Some(k) => {
            let enlace = ui::fotos_de_la_nota(&m.texto).into_iter().nth(k)?.enlace;
            super::nota_rica::ruta_de(&a.raiz, &a.ficha.id, &enlace)
        }
        None => (m.clase == Some(Clase::Imagen) && !pixpin_proyecto::cuaderno::es_video(m))
            .then(|| ruta_del_mensaje(&a.raiz, &a.ficha.id, m))
            .flatten(),
    }
}

/// Si pulsar `c` abre un enlace en el navegador: un mensaje sin fichero con
/// una direccion web. La foto de una nota rica es una foto aunque su nota
/// lleve un enlace.
pub(super) fn es_enlace(a: &Abierto, c: Cosa) -> bool {
    c.foto.is_none()
        && a.mensajes
            .get(c.mensaje)
            .is_some_and(|m| m.ruta.is_none() && tarjetas::primer_enlace(&m.texto).is_some())
}

/// Si pulsar `c` la hace sonar sin salir de la isla: una nota de voz.
pub(super) fn es_voz(a: &Abierto, c: Cosa) -> bool {
    c.foto.is_none()
        && a.mensajes
            .get(c.mensaje)
            .is_some_and(|m| m.clase == Some(Clase::Voz))
}

/// Lo que la ficha tiene abierto en su previa, por el id de su mensaje: se
/// apunta ANTES de que la lista cambie (una sincronizacion que mete
/// mensajes en medio, un borrado), como `releer_lo_abierto` hace con lo
/// demas que va por posicion.
pub(super) fn apuntar_la_abierta(a: &Abierto) -> Option<(String, Option<usize>)> {
    let c = a.info_abierta?;
    Some((a.mensajes.get(c.mensaje)?.id.clone(), c.foto))
}

/// Y se vuelve a buscar DESPUES: si ya no esta (se borro, o la nota perdio
/// esa foto), la ficha vuelve a su rejilla en vez de ensenar otra cosa. Lo
/// desplazado se sujeta a lo que mide ahora la seccion, que puede ser mas
/// corta, y el foco del teclado se suelta (iba por posicion).
pub(super) fn reubicar_la_abierta(a: &mut Abierto, apunte: Option<(String, Option<usize>)>) {
    a.info_abierta = apunte.and_then(|(id, foto)| {
        let c = Cosa {
            mensaje: a.mensajes.iter().position(|m| m.id == id)?,
            foto,
        };
        let seccion = a.info?;
        de_la_seccion(a, seccion).contains(&c).then_some(c)
    });
    a.nav_info.foco.set(None);
    a.scroll_info = a.scroll_info.clamp(0, a.alto_info.get() as i32);
}

/// Si lo que se teclea es para la lupa de la ficha: con la isla abierta y
/// en su rejilla. En la previa la caja no se ve (las flechas pasan de cosa y
/// Escape vuelve), y con la isla cerrada por otro camino (Ctrl+F, ir a un
/// mensaje) la lupa puede haberse quedado encendida: entonces lo escrito es
/// para la caja que se ve, no para una escondida.
pub(super) fn teclado_en_la_lupa(a: &Abierto) -> bool {
    a.buscando_info && a.info.is_some() && a.info_abierta.is_none()
}

// --- Los proyectos -----------------------------------------------------------

/// **Apunta los proyectos que lista la isla**: los de la lista de chats, tal
/// cual. Se llama antes de pintar, y lo que se pulsa despues es lo que se
/// pinto. Solo se copian si cambiaron; si cambian, el foco del teclado (que
/// va por lugar en la lista) se suelta.
pub(super) fn poner_proyectos(a: &mut Abierto, fichas: &[Ficha]) {
    if a.proyectos_info.as_slice() != fichas {
        a.proyectos_info = fichas.to_vec();
        if a.info == Some(Seccion::Proyectos) {
            a.nav_info.foco.set(None);
        }
    }
}

/// **Pulsar el proyecto `n`** de la pestana. El de este chat no sale de la
/// isla: lleva a su «Galeria», sus hojas, que es lo que ensenaba su tarjeta
/// en la pantalla de Proyectos. Otro es salir a proposito: la isla se cierra
/// y devuelve su `id` para que el chat pase a el.
pub(super) fn pulsar_proyecto(a: &mut Abierto, n: usize) -> Option<String> {
    let id = a.proyectos_info.get(n)?.id.clone();
    // La lupa se apaga en los dos casos: lo buscado era un nombre de
    // proyecto, y en la galeria o en el otro chat no significa nada.
    a.buscando_info = false;
    a.busqueda_info.vaciar();
    a.scroll_info = 0;
    a.nav_info.foco.set(None);
    if id == a.ficha.id {
        a.info = Some(Seccion::Galeria);
        return None;
    }
    a.info = None;
    a.info_abierta = None;
    Some(id)
}

/// Los proyectos que se ven en la pestana, por su `id`: sus logos se leen
/// antes de pintar, como los de las filas de la lista.
pub(super) fn logos_a_preparar(a: &Abierto, d: &Disposicion, escala: u32) -> Vec<String> {
    if a.info != Some(Seccion::Proyectos) || a.info_abierta.is_some() {
        return Vec::new();
    }
    let i = disposicion(d, escala);
    let c = colocar(a, Seccion::Proyectos, &i, escala);
    c.visibles(&i, a.scroll_info)
        .iter()
        .filter_map(|(p, _)| match p {
            Pieza::Cosa(n) => a.proyectos_info.get(c.suyos[*n].mensaje),
            Pieza::Mes(_) => None,
        })
        .map(|f| f.id.clone())
        .collect()
}

// --- Pintar ------------------------------------------------------------------

/// Una linea en seminegrita que no se sale de `ancho`: si no cabe, se corta
/// con «…».
fn linea_negrita(p: &Pintor, texto: &str, x: f32, y: f32, tam: f32, ancho: f32, color: Color) {
    let llana: String = texto
        .split(['\n', '\r', '\t'])
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    // La negrita es algo mas ancha que lo que se mide en redonda.
    let cabe = |t: &str| p.medir_texto(t, tam).0 * 1.07 <= ancho;
    let t = if cabe(&llana) {
        llana
    } else {
        let letras: Vec<char> = llana.chars().collect();
        let (mut bajo, mut alto) = (0usize, letras.len());
        while bajo < alto {
            let medio = (bajo + alto).div_ceil(2);
            let prueba: String = letras[..medio].iter().collect::<String>() + "…";
            if cabe(&prueba) {
                bajo = medio;
            } else {
                alto = medio - 1;
            }
        }
        letras[..bajo]
            .iter()
            .collect::<String>()
            .trim_end()
            .to_string()
            + "…"
    };
    let largo = t.encode_utf16().count() as u32;
    p.parrafo(
        &t,
        x,
        y,
        tam,
        ancho * 2.0 + 200.0,
        &[pixpin_render::Tramo {
            inicio: 0,
            longitud: largo,
            estilo: pixpin_render::EstiloTexto {
                negrita: true,
                ..Default::default()
            },
        }],
        color,
    );
}

/// **Pinta la isla** encima de todo, con el velo detras.
pub(super) fn pintar(p: &Pintor, d: &Disposicion, c: &Pinta, a: &Abierto, seccion: Seccion) {
    let (tema, escala) = (c.tema, c.escala);
    let e = escala as f32 / 100.0;
    // El velo, bajo la barra de titulo: dice que lo de debajo espera.
    let todo = ventana_de(d);
    p.rellenar(
        rf(Rect {
            y: d.barra.abajo(),
            alto: (todo.abajo() - d.barra.abajo()).max(0) as u32,
            ..todo
        }),
        tema.velo,
    );
    let i = disposicion(d, escala);
    let sobre = zona_en(a, a.raton_info, d, escala);
    let radio = if i.pantalla_completa {
        0.0
    } else {
        ui::ISLA_RADIO as f32 * e
    };
    let caja = rf(i.panel);
    if !i.pantalla_completa {
        // La sombra, suave y en dos capas: una ancha y otra cercana. Y el
        // filete, que la despega del velo en el tema claro y en el oscuro.
        for (baja, abre, alfa) in [(12.0, 6.0, 0.10), (4.0, 1.0, 0.16)] {
            p.rellenar_redondeado(
                RectF {
                    y: caja.y + baja * e,
                    ..encoger(caja, -abre * e)
                },
                radio + abre * e,
                con_alfa(Color::NEGRO, alfa),
            );
        }
        p.rellenar_redondeado(caja, radio, tema.isla_borde);
    }
    p.rellenar_redondeado(encoger(caja, 1.0), (radio - 1.0).max(0.0), tema.lista);
    p.empujar_recorte(encoger(caja, 1.0));

    match a.info_abierta {
        Some(cosa) => {
            pintar_cabecera_de_la_previa(p, c, a, &i, cosa, sobre);
            raya_bajo(p, c, i.cabecera);
            pintar_previa(p, c, a, &i, cosa, sobre);
        }
        None => {
            pintar_cabecera(p, c, a, &i, sobre);
            pintar_pestanas(p, c, a, &i, seccion, sobre);
            raya_bajo(p, c, i.tira);
            pintar_contenido(p, c, a, &i, seccion, sobre);
        }
    }
    p.soltar_recorte();
}

/// La raya fina que separa una franja de lo de debajo.
fn raya_bajo(p: &Pintor, c: &Pinta, r: Rect) {
    p.rellenar(
        RectF {
            x: r.x as f32,
            y: r.abajo() as f32 - 1.0,
            ancho: r.ancho as f32,
            alto: 1.0,
        },
        c.tema.separador,
    );
}

/// Un boton redondo con su icono, con el fondo puesto al pasar por encima
/// (o mientras esta encendido, como la lupa).
fn boton_redondo(p: &Pintor, c: &Pinta, r: Rect, icono: &Icono, encima: bool, puesto: bool) {
    let e = c.escala as f32 / 100.0;
    let caja = rf(r);
    if encima || puesto {
        p.rellenar_redondeado(
            caja,
            caja.ancho.min(caja.alto) / 2.0,
            if puesto {
                c.tema.fila_elegida
            } else {
                c.tema.fila_sobre
            },
        );
    }
    let lado = 20.0 * e;
    p.icono(
        icono,
        RectF {
            x: caja.x + (caja.ancho - lado) / 2.0,
            y: caja.y + (caja.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        if puesto {
            c.tema.texto_elegido
        } else {
            c.tema.texto
        },
    );
}

/// El avatar de un proyecto, como en su fila de la lista: su foto (imagen,
/// emoji o color), o el circulo de su color con las iniciales.
fn avatar(p: &Pintor, f: &Ficha, caja: Rect) {
    let av = rf(caja);
    if logo::pintar(p, f, av) {
        return;
    }
    p.rellenar_redondeado(av, av.ancho / 2.0, color_avatar(&f.codigo_unico()));
    let letras = iniciales(&f.nombre);
    let tam = av.alto * 0.4;
    let (w, h) = p.medir_texto(&letras, tam);
    p.texto(
        &letras,
        av.x + (av.ancho - w) / 2.0,
        av.y + (av.alto - h) / 2.0,
        tam,
        Color::BLANCO,
    );
}

fn pintar_cabecera(p: &Pintor, c: &Pinta, a: &Abierto, i: &Isla, sobre: ZonaInfo) {
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    boton_redondo(p, c, i.cerrar, &mi::CLOSE, sobre == ZonaInfo::Cerrar, false);
    boton_redondo(
        p,
        c,
        i.buscar,
        &mi::SEARCH,
        sobre == ZonaInfo::Buscar,
        a.buscando_info,
    );
    if a.buscando_info {
        // Con la lupa encendida, la caja de buscar ocupa el sitio del avatar
        // y del nombre.
        let b = i.caja_buscar;
        p.rellenar_redondeado(rf(b), b.alto as f32 / 2.0, tema.buscador);
        let margen = ui::BUSCAR_TEXTO_X as f32 * e;
        a.busqueda_info.pintar(
            p,
            rf(b),
            RectF {
                x: b.x as f32 + margen,
                ancho: (b.ancho as f32 - 2.0 * margen).max(0.0),
                ..rf(b)
            },
            ui::BUSCAR_TAM * e,
            &textos.t("chat-buscar"),
            &colores_de_caja(tema, tema.texto, tema.apagado),
            true,
            std::time::Instant::now(),
        );
        return;
    }
    avatar(p, &a.ficha, i.avatar);
    // Se puede pulsar: cambia la foto del chat. Con el raton encima, un aro.
    if sobre == ZonaInfo::Avatar {
        let av = rf(i.avatar);
        p.anillo(
            (av.x + av.ancho / 2.0, av.y + av.alto / 2.0),
            av.ancho / 2.0 + 1.5 * e,
            (2.0 * e).max(1.0),
            tema.enviar,
        );
    }
    // Debajo del nombre, cuantas cosas tiene y el codigo unico del chat: es
    // el que hay que mirar para saber si lo de aqui y lo del movil son la
    // misma cosa (en el movil sale igual).
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("n", cosas_de(a, Seccion::Todo).len() as i64);
    args.set("codigo", a.ficha.codigo_unico());
    titulo_y_subtitulo(
        p,
        c,
        i.titulo,
        &a.ficha.nombre,
        &textos.t_args("info-resumen", &args),
    );
}

/// En la previa: volver en el sitio del avatar, el nombre de la cosa y
/// «3 de 24 · fecha · tamano», y sus dos botones junto al aspa.
fn pintar_cabecera_de_la_previa(
    p: &Pintor,
    c: &Pinta,
    a: &Abierto,
    i: &Isla,
    cosa: Cosa,
    sobre: ZonaInfo,
) {
    let textos = c.textos;
    boton_redondo(p, c, i.cerrar, &mi::CLOSE, sobre == ZonaInfo::Cerrar, false);
    boton_redondo(
        p,
        c,
        i.avatar,
        &mi::ARROW_BACK,
        sobre == ZonaInfo::Volver,
        false,
    );
    let Some(m) = a.mensajes.get(cosa.mensaje) else {
        return;
    };
    let video = cosa.foto.is_none() && pixpin_proyecto::cuaderno::es_video(m);
    let icono_abrir: &Icono = if es_enlace(a, cosa) {
        &mi::LINK
    } else if es_voz(a, cosa) || video {
        &mi::PLAY_ARROW
    } else {
        &mi::OPEN_IN_NEW
    };
    boton_redondo(p, c, i.abrir, icono_abrir, sobre == ZonaInfo::Abrir, false);
    boton_redondo(
        p,
        c,
        i.en_el_chat,
        &mi::FORUM,
        sobre == ZonaInfo::EnElChat,
        false,
    );
    let suyos = a.info.map(|s| de_la_seccion(a, s)).unwrap_or_default();
    let k = suyos.iter().position(|x| *x == cosa).map_or(0, |k| k + 1);
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("k", k as i64);
    args.set("n", suyos.len() as i64);
    let mut sub = vec![
        textos.t_args("info-previa-posicion", &args),
        chat::etiqueta_hora(pixpin_shell::entorno::a_local(m.cuando), c.ahora),
    ];
    // Lo que pesa la nota no es lo que pesa su foto.
    if m.bytes > 0 && cosa.foto.is_none() {
        sub.push(chat::tamano_corto(m.bytes as u64));
    }
    titulo_y_subtitulo(
        p,
        c,
        i.titulo_previa,
        &nombre_de_la_cosa(m, cosa),
        &sub.join("  ·  "),
    );
}

fn titulo_y_subtitulo(p: &Pintor, c: &Pinta, r: Rect, titulo: &str, subtitulo: &str) {
    let e = c.escala as f32 / 100.0;
    let (t, s) = (ui::ISLA_TITULO_TAM * e, ui::ISLA_SUBTITULO_TAM * e);
    let (_, th) = p.medir_texto("Ag", t);
    let (_, sh) = p.medir_texto("Ag", s);
    let hueco = 2.0 * e;
    let y = r.y as f32 + (r.alto as f32 - th - hueco - sh) / 2.0;
    let (x, ancho) = (r.x as f32, r.ancho as f32);
    linea_negrita(p, titulo, x, y, t, ancho, c.tema.texto);
    p.texto_linea(subtitulo, x, y + th + hueco, s, ancho, c.tema.apagado);
}

/// El nombre de una foto de nota: lo que dice su alt (sin el ancho del
/// editor, `planta|300`) o, si no dice nada, la nota.
fn nombre_de_la_cosa(m: &Mensaje, c: Cosa) -> String {
    let alt = c
        .foto
        .and_then(|k| ui::fotos_de_la_nota(&m.texto).into_iter().nth(k))
        .map(|f| {
            f.alt
                .split('|')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string()
        })
        .unwrap_or_default();
    if alt.is_empty() { nombre_de(m) } else { alt }
}

/// El nombre con el que se ensena una cosa: el del fichero, o su texto.
fn nombre_de(m: &Mensaje) -> String {
    let r = m.resumen();
    if r.trim().is_empty() {
        m.nombre.clone()
    } else {
        r.lines().next().unwrap_or_default().to_string()
    }
}

fn pintar_pestanas(
    p: &Pintor,
    c: &Pinta,
    a: &Abierto,
    i: &Isla,
    seccion: Seccion,
    sobre: ZonaInfo,
) {
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let tam = ui::ISLA_PESTANA_TAM * e;
    let tam_n = (ui::ISLA_PESTANA_TAM - 1.5) * e;
    let hueco = 5.0 * e;
    // Cada pestana: su nombre y, mas apagada, su cuenta. Se mide aqui, que
    // es donde esta la fuente, y se apunta para que el clic sepa en cual cae.
    // La activa va en negrita: se mide un poco mas ancha.
    let lista: Vec<(Seccion, String, String)> = pestanas(a)
        .into_iter()
        .map(|(s, n)| (s, textos.t(s.clave()), n.to_string()))
        .collect();
    let medidas: Vec<(f32, f32)> = lista
        .iter()
        .map(|(_, r, n)| (p.medir_texto(r, tam).0 * 1.07, p.medir_texto(n, tam_n).0))
        .collect();
    let anchos: Vec<f32> = medidas.iter().map(|(w, wn)| w + hueco + wn).collect();
    *a.anchos_pestanas.borrow_mut() = anchos.clone();
    // Al cambiar de pestana, la tira se lleva a que la activa se vea.
    let activa = lista.iter().position(|(s, _, _)| *s == seccion);
    let mut colocadas = i.pestanas(&anchos, a.nav_info.pestanas_x.get(), escala);
    if a.nav_info.pestana_vista.get() != Some(seccion) {
        a.nav_info.pestana_vista.set(Some(seccion));
        if let Some(n) = activa {
            let d = colocadas.para_ver(n, i.tira, escala);
            colocadas = i.pestanas(&anchos, d, escala);
        }
    }
    a.nav_info.pestanas_x.set(colocadas.desplazado);
    p.empujar_recorte(rf(i.tira));
    for (n, ((s, rotulo, cuenta), r)) in lista.iter().zip(&colocadas.rects).enumerate() {
        let es_activa = *s == seccion;
        if !es_activa && sobre == ZonaInfo::Pestana(*s) {
            let fondo = encoger(rf(*r), 3.0 * e);
            p.rellenar_redondeado(
                RectF {
                    y: fondo.y + 3.0 * e,
                    alto: (fondo.alto - 6.0 * e).max(0.0),
                    ..fondo
                },
                8.0 * e,
                tema.fila_sobre,
            );
        }
        let (w, wn) = medidas[n];
        let (_, h) = p.medir_texto(rotulo, tam);
        let x = r.x as f32 + (r.ancho as f32 - w - hueco - wn) / 2.0;
        let y = r.y as f32 + (r.alto as f32 - h) / 2.0;
        if es_activa {
            linea_negrita(p, rotulo, x, y, tam, w + 4.0 * e, tema.enviar);
        } else {
            p.texto(rotulo, x, y, tam, tema.texto);
        }
        let (_, hn) = p.medir_texto(cuenta, tam_n);
        p.texto(
            cuenta,
            x + w + hueco,
            y + (h - hn) / 2.0 + 0.5 * e,
            tam_n,
            if es_activa {
                con_alfa(tema.enviar, 0.75)
            } else {
                tema.apagado
            },
        );
        if es_activa {
            let s = ui::subrayado(*r, anchos[n], escala);
            p.rellenar_redondeado(rf(s), s.alto as f32 / 2.0, tema.enviar);
        }
    }
    p.soltar_recorte();
}

fn pintar_contenido(
    p: &Pintor,
    c: &Pinta,
    a: &Abierto,
    i: &Isla,
    seccion: Seccion,
    sobre: ZonaInfo,
) {
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let colocado = colocar(a, seccion, i, escala);
    a.alto_info.set(colocado.lista.alto);
    if colocado.suyos.is_empty() {
        // Vacio: un icono grande y una frase de lo que falta, en medio.
        let lado = 40.0 * e;
        let cx = i.contenido.x as f32 + i.contenido.ancho as f32 / 2.0;
        let cy = i.contenido.y as f32 + i.contenido.alto as f32 / 2.0;
        p.circulo((cx, cy - lado / 2.0), lado, con_alfa(tema.apagado, 0.12));
        p.icono(
            icono_de_la_seccion(seccion),
            RectF {
                x: cx - lado / 2.0,
                y: cy - lado,
                ancho: lado,
                alto: lado,
            },
            con_alfa(tema.apagado, 0.8),
        );
        let vacio = if a.busqueda_info.trim().is_empty() {
            textos.t(clave_del_vacio(seccion))
        } else {
            textos.t("chat-buscar-nada")
        };
        let tam = 13.5 * e;
        let ancho = (i.contenido.ancho as f32 - 48.0 * e).max(0.0);
        let (w, _) = p.medir_texto(&vacio, tam);
        let w = w.min(ancho);
        p.texto_linea(
            &vacio,
            cx - w / 2.0,
            cy + lado / 2.0 + 14.0 * e,
            tam,
            ancho,
            tema.apagado,
        );
        return;
    }
    let foco = a.nav_info.foco.get();
    p.empujar_recorte(rf(i.contenido));
    for (pieza, r) in colocado.visibles(i, a.scroll_info) {
        match pieza {
            Pieza::Mes(n) => {
                if let Some(m) = colocado
                    .suyos
                    .get(n)
                    .and_then(|c| a.mensajes.get(c.mensaje))
                {
                    rotulo_del_mes(p, c, m, r);
                }
            }
            Pieza::Cosa(n) => {
                let cosa = colocado.suyos[n];
                let encima = sobre == ZonaInfo::Cosa(cosa);
                if seccion == Seccion::Proyectos {
                    let encima = sobre == ZonaInfo::Proyecto(cosa.mensaje);
                    pintar_fila_de_proyecto(p, c, a, cosa.mensaje, r, encima);
                } else if colocado.cuadricula {
                    pintar_celda(p, c, a, cosa, r, encima);
                } else {
                    pintar_fila(p, c, a, seccion, cosa.mensaje, r, encima);
                }
                if foco == Some(cosa) {
                    // El foco del teclado: el filete del color de la casa.
                    let caja = encoger(rf(r), 1.0 * e);
                    let radio = if colocado.cuadricula { 0.0 } else { 10.0 * e };
                    p.rellenar_redondeado(caja, radio, con_alfa(tema.enviar, 0.10));
                    p.trazar(caja, (2.0 * e).max(1.0), tema.enviar);
                }
            }
        }
    }
    p.soltar_recorte();
    // La barra de desplazamiento, encima, fina y discreta.
    if let Some(b) =
        ui::barra_de_desplazamiento(i.contenido, colocado.lista.alto, a.scroll_info, escala)
    {
        p.rellenar_redondeado(rf(b), b.ancho as f32 / 2.0, con_alfa(tema.apagado, 0.45));
    }
}

/// «Octubre de 2026»: el rotulo de un mes, con su primera letra en
/// mayuscula.
fn rotulo_del_mes(p: &Pintor, c: &Pinta, m: &Mensaje, r: Rect) {
    let e = c.escala as f32 / 100.0;
    let (ano, mes, _) = chat::partes_fecha(pixpin_shell::entorno::a_local(m.cuando));
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("mes", c.textos.t(&format!("chat-mes-{mes}")));
    args.set("ano", ano);
    let texto = c.textos.t_args("info-mes", &args);
    let mut letras = texto.chars();
    let texto: String = letras
        .next()
        .map(|l| l.to_uppercase().chain(letras).collect())
        .unwrap_or_default();
    let tam = ui::ISLA_MES_TAM * e;
    let (_, h) = p.medir_texto(&texto, tam);
    linea_negrita(
        p,
        &texto,
        r.x as f32 + 8.0 * e,
        r.y as f32 + (r.alto as f32 - h) / 2.0 + 2.0 * e,
        tam,
        (r.ancho as f32 - 16.0 * e).max(0.0),
        c.tema.apagado,
    );
}

/// La frase de una seccion vacia.
fn clave_del_vacio(s: Seccion) -> &'static str {
    match s {
        Seccion::Galeria => "info-vacia-galeria",
        Seccion::Fotos => "info-vacia-fotos",
        Seccion::Videos => "info-vacia-videos",
        Seccion::Archivos => "info-vacia-archivos",
        Seccion::Enlaces => "info-vacia-enlaces",
        Seccion::Voz => "info-vacia-voz",
        Seccion::Dibujos => "info-vacia-dibujos",
        Seccion::Fijados => "info-vacia-fijados",
        Seccion::Buzon => "info-vacia-buzon",
        Seccion::Todo => "info-vacia",
        Seccion::Proyectos => "info-vacia-proyectos",
    }
}

/// El icono que representa una seccion (para el vacio).
fn icono_de_la_seccion(s: Seccion) -> &'static Icono {
    match s {
        Seccion::Fotos | Seccion::Galeria => &mi::PHOTO_LIBRARY,
        Seccion::Videos => &mi::VIDEOCAM,
        Seccion::Enlaces => &mi::LINK,
        Seccion::Voz => &mi::MIC,
        Seccion::Dibujos => &mi::DRAW,
        Seccion::Fijados => &mi::PUSH_PIN,
        Seccion::Proyectos => &mi::FOLDER,
        _ => &mi::DESCRIPTION,
    }
}

/// El recuadro neutro de lo que aun no se puede ensenar (la foto que carga,
/// o que todavia no esta en el aparato), con su icono en medio.
fn recuadro_con_icono(p: &Pintor, c: &Pinta, caja: RectF, radio: f32, icono: &Icono) {
    let e = c.escala as f32 / 100.0;
    p.rellenar_redondeado(caja, radio, c.tema.buscador);
    let lado = (caja.ancho.min(caja.alto) * 0.3).clamp(1.0, (40.0 * e).max(1.0));
    p.icono(
        icono,
        RectF {
            x: caja.x + (caja.ancho - lado) / 2.0,
            y: caja.y + (caja.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        con_alfa(c.tema.apagado, 0.7),
    );
}

/// Una celda de la rejilla: la foto recortada al cuadrado, el dibujo sobre
/// su papel, o la tarjeta de lo que no tiene vista previa. Los videos llevan
/// su insignia abajo a la izquierda.
fn pintar_celda(p: &Pintor, c: &Pinta, a: &Abierto, cosa: Cosa, celda: Rect, encima: bool) {
    let n = cosa.mensaje;
    let Some(m) = a.mensajes.get(n) else {
        return;
    };
    let video = cosa.foto.is_none() && pixpin_proyecto::cuaderno::es_video(m);
    let ruta = match cosa.foto {
        Some(_) => ruta_de_la_foto(a, cosa),
        None => (!video)
            .then(|| ruta_del_mensaje(&a.raiz, &a.ficha.id, m))
            .flatten(),
    };
    let es_foto = cosa.foto.is_some() || m.clase == Some(Clase::Imagen);
    if let Some((b, w, h)) = ruta.as_ref().and_then(|r| c.miniaturas.ya(r)) {
        p.empujar_recorte(rf(celda));
        crate::miniaturas::pintar_recortado(p, b, rf(celda), w, h);
        p.soltar_recorte();
    } else if video {
        recuadro_con_icono(p, c, rf(celda), 0.0, &mi::VIDEOCAM);
    } else if es_foto {
        recuadro_con_icono(p, c, rf(celda), 0.0, &mi::IMAGE);
    } else {
        match a
            .vistas
            .get(n)
            .and_then(|v| v.as_ref())
            .filter(|v| !matches!(v, Ojeada::Foto { .. }))
        {
            Some(vista) => pintar_ojeada(p, c, vista, rf(celda), 0.0),
            None => pintar_tarjeta_galeria(p, c, m, celda),
        }
    }
    if video {
        insignia_de_video(p, c, rf(celda), m.duracion_ms);
    }
    if encima {
        // Al pasar por encima se aclara: se sabe que se puede pulsar.
        p.rellenar(rf(celda), con_alfa(Color::BLANCO, 0.12));
    }
}

/// Una vista ya leida (dibujo, tabla, cuaderno) dentro de `caja`, sobre su
/// papel.
fn pintar_ojeada(p: &Pintor, c: &Pinta, vista: &Ojeada, caja: RectF, radio: f32) {
    let (tema, e) = (c.tema, c.escala as f32 / 100.0);
    let papel = match vista {
        Ojeada::Lienzo(l) => papel_de_vista(l, tema.papel),
        _ => tema.papel,
    };
    p.rellenar_redondeado(caja, radio, papel);
    p.empujar_recorte(caja);
    let dentro = encoger(caja, 6.0 * e);
    match vista {
        Ojeada::Lienzo(l) => pintar_lienzo(p, l, dentro, Some(c.previas)),
        Ojeada::Tabla(t) => pintar_ojeada_tabla(p, tema, c.escala, t, dentro),
        Ojeada::Carpeta(_) => {
            let lado = (dentro.ancho.min(dentro.alto) * 0.5).max(1.0);
            p.icono(
                &mi::MENU_BOOK,
                RectF {
                    x: dentro.x + (dentro.ancho - lado) / 2.0,
                    y: dentro.y + (dentro.alto - lado) / 2.0,
                    ancho: lado,
                    alto: lado,
                },
                tema.texto_papel,
            );
        }
        Ojeada::Foto { .. } => {}
    }
    p.soltar_recorte();
}

/// La insignia de un video, abajo a la izquierda: el triangulo y lo que
/// dura (si se sabe), en una pildora oscura.
fn insignia_de_video(p: &Pintor, c: &Pinta, celda: RectF, duracion_ms: i64) {
    let e = c.escala as f32 / 100.0;
    let texto = ui::duracion_corta(duracion_ms);
    let tam = 11.0 * e;
    let (w, h) = if texto.is_empty() {
        (0.0, 0.0)
    } else {
        p.medir_texto(&texto, tam)
    };
    let icono = 12.0 * e;
    let alto = (h.max(icono) + 6.0 * e).min(celda.alto);
    let ancho = 6.0 * e
        + icono
        + if texto.is_empty() {
            6.0 * e
        } else {
            3.0 * e + w + 8.0 * e
        };
    let pildora = RectF {
        x: celda.x + 6.0 * e,
        y: celda.y + celda.alto - alto - 6.0 * e,
        ancho: ancho.min(celda.ancho),
        alto,
    };
    p.rellenar_redondeado(pildora, alto / 2.0, con_alfa(Color::NEGRO, 0.6));
    p.icono(
        &mi::PLAY_ARROW,
        RectF {
            x: pildora.x + 5.0 * e,
            y: pildora.y + (alto - icono) / 2.0,
            ancho: icono,
            alto: icono,
        },
        Color::BLANCO,
    );
    if !texto.is_empty() {
        p.texto(
            &texto,
            pildora.x + 6.0 * e + icono + 3.0 * e,
            pildora.y + (alto - h) / 2.0,
            tam,
            Color::BLANCO,
        );
    }
}

/// La chapa de 44 de una fila: la miniatura de la foto, o el cuadrado de
/// color de su extension con las letras («PDF» rojo, «DOCX» azul…). Dice si
/// la pinto.
fn chapa_de_fila(p: &Pintor, c: &Pinta, a: &Abierto, m: &Mensaje, caja: Rect) -> bool {
    let e = c.escala as f32 / 100.0;
    let foto = (m.clase == Some(Clase::Imagen))
        .then(|| ruta_del_mensaje(&a.raiz, &a.ficha.id, m))
        .flatten()
        .and_then(|r| c.miniaturas.ya(&r));
    if let Some((b, w, h)) = foto {
        p.empujar_recorte(rf(caja));
        crate::miniaturas::pintar_recortado(p, b, rf(caja), w, h);
        p.soltar_recorte();
        return true;
    }
    let ext = chat::extension_corta(&m.nombre);
    if ext.is_empty() {
        return false;
    }
    let (fondo, letra) = super::icono_de_tipo::colores(&m.nombre);
    p.rellenar_redondeado(rf(caja), 10.0 * e, fondo);
    let tam = if ext.chars().count() > 3 { 10.5 } else { 12.0 } * e;
    let (w, h) = p.medir_texto(&ext, tam);
    linea_negrita(
        p,
        &ext,
        caja.x as f32 + (caja.ancho as f32 - w * 1.07) / 2.0,
        caja.y as f32 + (caja.alto as f32 - h) / 2.0,
        tam,
        caja.ancho as f32,
        letra,
    );
    true
}

/// Un circulo de color con su icono blanco.
fn circulo_con(p: &Pintor, caja: Rect, icono: &Icono, fondo: Color) {
    let r = rf(caja);
    p.rellenar_redondeado(r, r.ancho / 2.0, fondo);
    let lado = r.ancho * 0.5;
    p.icono(
        icono,
        RectF {
            x: r.x + (r.ancho - lado) / 2.0,
            y: r.y + (r.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        Color::BLANCO,
    );
}

/// **Una fila**: archivos, enlaces, voz, notas. A la izquierda su chapa de
/// 44; a la derecha el nombre (cortado con «…» si no cabe) y, apagado, de
/// que es, cuanto pesa y cuando. Un enlace ensena su sitio, su direccion y
/// lo que decia el mensaje; una nota de voz, su boton y lo que dura.
#[allow(clippy::too_many_arguments)]
fn pintar_fila(
    p: &Pintor,
    c: &Pinta,
    a: &Abierto,
    seccion: Seccion,
    n: usize,
    fila: Rect,
    encima: bool,
) {
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let Some(m) = a.mensajes.get(n) else {
        return;
    };
    if encima {
        p.rellenar_redondeado(rf(fila), 10.0 * e, tema.fila_sobre);
    }
    let f = ui::dentro_de_la_fila(fila, escala);
    let ancho = f.ancho_texto as f32;
    let (tam, tam_2) = (13.5 * e, 12.0 * e);
    let cuando = chat::etiqueta_hora(pixpin_shell::entorno::a_local(m.cuando), c.ahora);
    let enlace = tarjetas::primer_enlace(&m.texto).filter(|_| m.ruta.is_none());

    // Un enlace: la letra de su sitio, el sitio en negrita, la direccion y,
    // si el mensaje decia algo mas, eso.
    if let Some(en) = &enlace {
        let letra: String = en
            .donde
            .chars()
            .next()
            .map(|l| l.to_uppercase().collect())
            .unwrap_or_default();
        let r = rf(f.miniatura);
        p.rellenar_redondeado(r, 10.0 * e, color_avatar(&en.donde));
        let t = r.alto * 0.45;
        let (w, h) = p.medir_texto(&letra, t);
        p.texto(
            &letra,
            r.x + (r.ancho - w) / 2.0,
            r.y + (r.alto - h) / 2.0,
            t,
            Color::BLANCO,
        );
        linea_negrita(
            p,
            &en.donde,
            f.nombre.x as f32,
            f.nombre.y as f32,
            tam,
            ancho,
            tema.texto,
        );
        p.texto_linea(
            &en.url,
            f.estado.x as f32,
            f.estado.y as f32,
            tam_2,
            ancho,
            tema.enlace,
        );
        let resto = m.texto.replace(&en.url, " ");
        let resto = resto.split_whitespace().collect::<Vec<_>>().join(" ");
        let tercera = if resto.is_empty() { cuando } else { resto };
        if let Some(t) = f.tercera {
            p.texto_linea(&tercera, t.x as f32, t.y as f32, tam_2, ancho, tema.apagado);
        }
        return;
    }

    // Una nota de voz: el boton de reproducir (o de pausa, si es la que
    // suena) y lo que dura.
    if m.clase == Some(Clase::Voz) {
        let suena =
            ruta_del_mensaje(&a.raiz, &a.ficha.id, m).is_some_and(|r| crate::audio::suena(&r));
        circulo_con(
            p,
            f.miniatura,
            if suena { &mi::PAUSE } else { &mi::PLAY_ARROW },
            tema.enviar,
        );
        let nombre = if m.nombre.is_empty() {
            textos.t("chat-clase-voz")
        } else {
            nombre_de(m)
        };
        p.texto_linea(
            &nombre,
            f.nombre.x as f32,
            f.nombre.y as f32,
            tam,
            ancho,
            tema.texto,
        );
        let mut partes = Vec::new();
        let dura = ui::duracion_corta(m.duracion_ms);
        if !dura.is_empty() {
            partes.push(dura);
        }
        partes.push(cuando);
        p.texto_linea(
            &partes.join("  ·  "),
            f.estado.x as f32,
            f.estado.y as f32,
            tam_2,
            ancho,
            tema.apagado,
        );
        return;
    }

    // Lo demas: su chapa (o un circulo con el icono de su clase), el nombre
    // y «de que es · cuanto pesa · cuando».
    if !chapa_de_fila(p, c, a, m, f.miniatura) {
        let icono: &Icono = match m.clase {
            Some(Clase::Dibujo) => &mi::DRAW,
            Some(Clase::Pagina) => &mi::MENU_BOOK,
            Some(Clase::Imagen) => &mi::IMAGE,
            _ => &mi::DESCRIPTION,
        };
        circulo_con(p, f.miniatura, icono, con_alfa(tema.apagado, 0.6));
    }
    let nombre = if seccion == Seccion::Archivos && !m.nombre.is_empty() {
        m.nombre.clone()
    } else {
        nombre_de(m)
    };
    let marcas = negritas(&nombre, &a.busqueda_info);
    if marcas.is_empty() {
        p.texto_linea(
            &nombre,
            f.nombre.x as f32,
            f.nombre.y as f32,
            tam,
            ancho,
            tema.texto,
        );
    } else {
        // Con lo buscado en negrita: en una linea, recortada a su sitio.
        p.empujar_recorte(RectF {
            x: f.nombre.x as f32,
            y: f.nombre.y as f32,
            ancho,
            alto: 20.0 * e,
        });
        p.parrafo(
            &crate::caja_texto::en_una_linea(&nombre),
            f.nombre.x as f32,
            f.nombre.y as f32,
            tam,
            ancho * 2.0 + 200.0,
            &marcas,
            tema.texto,
        );
        p.soltar_recorte();
    }
    let mut partes: Vec<String> = Vec::new();
    if seccion != Seccion::Archivos
        && let Some(clase) = super::clase_de(m, textos)
    {
        partes.push(clase);
    }
    if m.bytes > 0 {
        partes.push(chat::tamano_corto(m.bytes as u64));
    }
    partes.push(cuando);
    p.texto_linea(
        &partes.join("  ·  "),
        f.estado.x as f32,
        f.estado.y as f32,
        tam_2,
        ancho,
        tema.apagado,
    );
}

/// **Un proyecto de su pestana**: su avatar, su nombre en seminegrita con lo
/// buscado resaltado y, a la derecha, cuando se toco; debajo, cuantas hojas
/// tiene y su ultima linea. El de este chat va marcado como la fila elegida
/// de la lista y lo dice delante.
fn pintar_fila_de_proyecto(p: &Pintor, c: &Pinta, a: &Abierto, n: usize, fila: Rect, encima: bool) {
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let Some(f) = a.proyectos_info.get(n) else {
        return;
    };
    let actual = f.id == a.ficha.id;
    if actual {
        p.rellenar_redondeado(rf(fila), 10.0 * e, tema.fila_elegida);
    } else if encima {
        p.rellenar_redondeado(rf(fila), 10.0 * e, tema.fila_sobre);
    }
    let (texto, apagado) = if actual {
        (tema.texto_elegido, tema.texto_elegido)
    } else {
        (tema.texto, tema.apagado)
    };
    let d = ui::dentro_de_la_fila(fila, escala);
    avatar(p, f, d.miniatura);
    let (tam, tam_2) = (13.5 * e, 12.0 * e);
    // La hora primero: dice cuanto sitio le queda al nombre.
    let hora = chat::etiqueta_hora(pixpin_shell::entorno::a_local(f.tocado), c.ahora);
    let mut ancho_nombre = d.ancho_texto as f32;
    if !hora.is_empty() {
        let (w, _) = p.medir_texto(&hora, tam_2);
        p.texto(
            &hora,
            d.fila.derecha() as f32 - 8.0 * e - w,
            d.nombre.y as f32 + 1.0 * e,
            tam_2,
            apagado,
        );
        ancho_nombre -= w + 8.0 * e;
    }
    let marcas = negritas(&f.nombre, &a.busqueda_info);
    if marcas.is_empty() {
        linea_negrita(
            p,
            &f.nombre,
            d.nombre.x as f32,
            d.nombre.y as f32,
            tam,
            ancho_nombre.max(0.0),
            texto,
        );
    } else {
        p.empujar_recorte(RectF {
            x: d.nombre.x as f32,
            y: d.nombre.y as f32,
            ancho: ancho_nombre.max(0.0),
            alto: 20.0 * e,
        });
        p.parrafo(
            &crate::caja_texto::en_una_linea(&f.nombre),
            d.nombre.x as f32,
            d.nombre.y as f32,
            tam,
            ancho_nombre.max(0.0) * 2.0 + 200.0,
            &marcas,
            texto,
        );
        p.soltar_recorte();
    }
    // Debajo: «Este chat · 12 hojas · lo ultimo que se dijo». Guardado en un
    // disco que ahora no esta, eso manda: es lo que explica por que no abre.
    let mut partes = Vec::new();
    if actual {
        partes.push(textos.t("info-proyecto-este"));
    }
    if !pixpin_proyecto::ubicacion::disponible(f) {
        partes.push(textos.t("ubicacion-no-disponible"));
    } else {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("cuantas", f.hojas);
        partes.push(textos.t_args("chat-hojas", &args));
        let resumen = f.resumen.lines().next().unwrap_or_default().trim();
        if !resumen.is_empty() {
            partes.push(resumen.to_string());
        }
    }
    p.texto_linea(
        &partes.join("  ·  "),
        d.estado.x as f32,
        d.estado.y as f32,
        tam_2,
        d.ancho_texto as f32,
        apagado,
    );
}

/// **La previa de una cosa**, en grande dentro de la isla: una foto sobre
/// fondo oscuro, entera y sin deformar; lo demas en su tarjeta. Las flechas
/// van encima, a los lados.
fn pintar_previa(p: &Pintor, c: &Pinta, a: &Abierto, i: &Isla, cosa: Cosa, sobre: ZonaInfo) {
    let (tema, escala) = (c.tema, c.escala);
    let e = escala as f32 / 100.0;
    let n = cosa.mensaje;
    let Some(m) = a.mensajes.get(n) else {
        return;
    };
    let pr = ui::previa(i.hueco_de_la_previa(), escala);
    let vista = rf(pr.vista);
    let de_nota = cosa.foto.is_some();
    let video = !de_nota && pixpin_proyecto::cuaderno::es_video(m);
    let foto =
        ruta_de_la_foto(a, cosa).and_then(|r| c.previas.ya(&r).or_else(|| c.miniaturas.ya(&r)));
    let ojeada = a
        .vistas
        .get(n)
        .filter(|_| !de_nota)
        .and_then(|v| v.as_ref())
        .filter(|v| !matches!(v, Ojeada::Foto { .. }));
    let es_foto = de_nota || (m.clase == Some(Clase::Imagen) && !video);
    if let Some((b, w, h)) = foto {
        // La foto sobre su fondo oscuro, con bandas donde no llena.
        p.rellenar_redondeado(vista, 12.0 * e, con_alfa(Color::NEGRO, 0.88));
        p.bitmap(b, rf(ui::encajar(w, h, pr.vista)), None, false);
    } else if es_foto {
        recuadro_con_icono(p, c, vista, 12.0 * e, &mi::IMAGE);
    } else if let Some(o) = ojeada {
        pintar_ojeada(p, c, o, vista, 12.0 * e);
    } else if matches!(m.clase, Some(Clase::Nota) | None) {
        // Una nota (o un enlace): su texto, en una tarjeta.
        p.rellenar_redondeado(vista, 12.0 * e, tema.burbuja_otra);
        let margen = 18.0 * e;
        p.empujar_recorte(vista);
        p.parrafo(
            &m.texto,
            vista.x + margen,
            vista.y + margen,
            15.0 * e,
            (vista.ancho - 2.0 * margen).max(0.0),
            &[],
            tema.texto,
        );
        p.soltar_recorte();
    } else {
        // Lo demas: su chapa (o el icono de su clase) en un cuadrado, y
        // debajo su nombre entero.
        let lado = (vista.ancho.min(vista.alto) * 0.45).max(1.0);
        let tarjeta = Rect {
            x: (vista.x + (vista.ancho - lado) / 2.0) as i32,
            y: (vista.y + (vista.alto - lado) / 2.0 - 18.0 * e) as i32,
            ancho: lado as u32,
            alto: lado as u32,
        };
        pintar_tarjeta_galeria(p, c, m, tarjeta);
        if video {
            insignia_de_video(p, c, rf(tarjeta), m.duracion_ms);
        }
        let nombre = if m.nombre.is_empty() {
            nombre_de(m)
        } else {
            m.nombre.clone()
        };
        let (w, _) = p.medir_texto(&nombre, 14.0 * e);
        let w = w.min(vista.ancho);
        p.texto_linea(
            &nombre,
            vista.x + (vista.ancho - w) / 2.0,
            tarjeta.abajo() as f32 + 14.0 * e,
            14.0 * e,
            vista.ancho,
            tema.texto,
        );
    }

    // Las flechas, encima de la cosa a los lados, solo si hay a donde ir.
    for (r, paso, icono, zona) in [
        (pr.anterior, -1, &mi::CHEVRON_LEFT, ZonaInfo::Anterior),
        (pr.siguiente, 1, &mi::CHEVRON_RIGHT, ZonaInfo::Siguiente),
    ] {
        if vecina(a, paso).is_none() {
            continue;
        }
        let caja = rf(r);
        p.rellenar_redondeado(
            caja,
            caja.ancho / 2.0,
            con_alfa(Color::NEGRO, if sobre == zona { 0.7 } else { 0.45 }),
        );
        let lado = caja.ancho * 0.6;
        p.icono(
            icono,
            RectF {
                x: caja.x + (caja.ancho - lado) / 2.0,
                y: caja.y + (caja.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            Color::BLANCO,
        );
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_orden_lleva_cada_seccion_una_vez() {
        for s in Seccion::TODAS {
            assert_eq!(
                ORDEN.iter().filter(|x| **x == s).count(),
                1,
                "{s:?} tiene que salir una vez"
            );
        }
        // Lo que se mira va antes que lo que se lee, y el buzon al final.
        let sitio = |s: Seccion| ORDEN.iter().position(|x| *x == s).unwrap();
        assert!(sitio(Seccion::Fotos) < sitio(Seccion::Archivos));
        assert!(sitio(Seccion::Videos) < sitio(Seccion::Enlaces));
        assert_eq!(ORDEN[ORDEN.len() - 1], Seccion::Buzon);
        // Los proyectos, delante: la isla se abre en ellos.
        assert_eq!(ORDEN[0], Seccion::Proyectos);
        assert_eq!(ORDEN[1], Seccion::Galeria);
    }

    #[test]
    fn cada_seccion_tiene_su_frase_de_vacia() {
        let mut claves: Vec<&str> = Seccion::TODAS.iter().map(|s| clave_del_vacio(*s)).collect();
        claves.sort_unstable();
        claves.dedup();
        assert_eq!(claves.len(), Seccion::TODAS.len());
    }
}
