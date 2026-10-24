//! **Lo que dibuja el asistente, al lienzo** (encargo 2).
//!
//! El turno corre en su hilo y el lienzo es de la ventana. Ademas, el
//! editor del lienzo es **modal sobre el hilo del chat**: mientras esta
//! abierto, el bucle del chat no da vueltas. Por eso el pedido no va por el
//! canal del chat sino por una cola de aqui, que vacia quien este vivo:
//!
//! - **Hay un lienzo abierto** ([`EditorAbierto`]): lo vacia el bucle del
//!   editor y lo dibujado cae **centrado en lo que se ve** ([`meter`]).
//! - **No hay**: lo vacia el chat, que lo mete sin abrirlo en el lienzo
//!   nombrado en la pregunta (`@lienzo:<nombre>`) o, si no, en el **lienzo
//!   propio del asistente** en esa conversacion (uno suyo, creado la primera
//!   vez); la respuesta lleva «Abrir lienzo». Como el editor encuadra lo
//!   dibujado al abrirse, lo nuevo se pone **a la derecha de lo que ya
//!   habia** ([`meter_al_lado`]) para no taparlo.
//!
//! Antes, sin lienzo abierto, iba al ultimo lienzo del chat, fuera de quien
//! fuera: el usuario no sabia donde buscarlo (13-oct).
//!
//! En los dos casos es `portapapeles::pegar`: **un solo paso de deshacer**,
//! y el lienzo se guarda por su camino de siempre (`guardar_hoja_dibujada`),
//! que es lo que mira la sincronizacion.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use pixpin_ia::edicion::{Anfitrion, Contexto, Edicion};
use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::Elemento;

/// Que se pide al lienzo (14-oct: ademas de dibujar, editar lo que hay y
/// resumirlo para el modelo; `pixpin_ia::edicion`).
#[derive(Debug, Clone, Default)]
pub enum Trabajo {
    /// Meter el `.excalidraw` de [`Pedido::documento`].
    #[default]
    Dibujar,
    /// Editar lo que ya hay, con lo que el turno sabe del lienzo y las
    /// imagenes adjuntas del chat (nombre y ruta) por si hay que pegar una.
    Editar {
        edicion: Box<Edicion>,
        ctx: Contexto,
        adjuntos: Vec<(String, std::path::PathBuf)>,
    },
    /// El resumen del lienzo destino, al empezar el turno.
    Resumir,
    /// **Mirarlo** (15-oct, `ojos.rs`): quien tiene la escena copia la zona
    /// y sus imagenes ([`Respuesta::vista`]); se pinta en el hilo de los ojos.
    Ver {
        mirada: pixpin_ia::ojos::Mirada,
        ctx: Contexto,
    },
    /// Lo que hizo un turno en el lienzo abierto, un solo paso de deshacer:
    /// si desde `desde` se cerraron justo `esperados` pasos (los del turno),
    /// se juntan; si el usuario hizo algo entre medias, no.
    Juntar { desde: u64, esperados: u64 },
    /// **La imagen del lienzo** para «calca esto» (15-oct): la elegida o,
    /// si no hay, la ultima pegada.
    Imagen,
    /// **Una plantilla de serie con otro intervalo** (22-oct, «damela del 1
    /// al 40», «de 10 en 10»): la elegida, rehecha en su sitio, o la de
    /// `nombre`. Ver [`poner_plantilla`].
    Plantilla {
        nombre: Option<String>,
        intervalo: pixpin_motor2d::figuras_de_serie::Intervalo,
    },
    /// **Las secciones de la plantilla del lienzo** (25-oct): la elegida o
    /// la unica que hay, para que el modelo las lea antes de personalizarla
    /// (`intervalo.secciones` en [`Trabajo::Plantilla`]). No cambia nada.
    Secciones,
    /// **Una copia de como esta** (23-oct, la revision con rubrica): antes
    /// de cada ronda de correccion, para volver si la deja peor.
    Copia,
    /// **Volver a una copia** de [`Trabajo::Copia`], como un paso mas
    /// (el `Ctrl+Z` del turno lo deshace todo junto).
    VolverA {
        copia: Vec<pixpin_motor2d::elemento::Elemento>,
    },
    /// **Las comprobaciones duras** de todo el lienzo (`pixpin_ia::lint`):
    /// textos encimados o que se salen, flechas sueltas, ortografia... una
    /// linea por fallo en [`Respuesta::texto`].
    Comprobar,
}

/// Lo que contesta el lienzo: el texto para el modelo, los ids que toco
/// (pasan a ser «lo ultimo») y, al resumir, el contexto del turno.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Respuesta {
    pub texto: String,
    pub ids: Vec<u64>,
    pub ctx: Option<Contexto>,
    /// Al mirar ([`Trabajo::Ver`]): lo copiado para pintarlo.
    pub vista: Option<Box<super::ojos::Encargo>>,
    /// En el lienzo abierto, los pasos de deshacer cerrados antes y despues
    /// de dibujar o editar (para [`Trabajo::Juntar`]).
    pub pasos: Option<(u64, u64)>,
    /// Al pedir la imagen ([`Trabajo::Imagen`]): la elegida o la ultima pegada.
    pub imagen: Option<Box<ImagenDelLienzo>>,
    /// Al pedir una copia ([`Trabajo::Copia`]): los elementos tal cual.
    pub copia: Option<Vec<pixpin_motor2d::elemento::Elemento>>,
}

/// Un pedido al lienzo.
pub struct Pedido {
    /// La conversacion desde la que se pidio (adonde va si no hay lienzo
    /// abierto).
    pub proyecto: String,
    /// El lienzo de esa conversacion que se nombro con `@lienzo:` (su
    /// referencia). Sin el, el propio del asistente.
    pub destino: Option<String>,
    /// Un `.excalidraw` hecho por `pixpin_ia::lienzo::documento` (al
    /// dibujar).
    pub documento: String,
    pub trabajo: Trabajo,
    /// Desde donde mide el modelo en este turno (para listar lo dibujado
    /// con las coordenadas que el entiende).
    pub marco: Option<(f32, f32)>,
    responder: Sender<Result<Respuesta, String>>,
    /// Si es una zona de una hoja (`hoja_a_lienzo`): va con las demas de su
    /// hoja, no centrada por su cuenta.
    pub hoja: Option<EnHoja>,
    /// Las imagenes nuevas que usan sus elementos `image` (su `fileId` es
    /// `pc<hex>` de este id): las fotos de una hoja.
    pub imagenes: Vec<(u64, pixpin_codec::ImagenRgba)>,
    /// La imagen de la que sale lo dibujado (su caja en el lienzo): va a su
    /// lado, sin taparla.
    pub junto_a: Option<(f32, f32, f32, f32)>,
}

impl Pedido {
    pub fn nuevo(
        proyecto: String,
        destino: Option<String>,
        documento: String,
        responder: Sender<Result<Respuesta, String>>,
    ) -> Pedido {
        Pedido {
            proyecto,
            destino,
            documento,
            trabajo: Trabajo::Dibujar,
            marco: None,
            responder,
            hoja: None,
            imagenes: Vec::new(),
            junto_a: None,
        }
    }

    /// Un pedido de editar o de resumir.
    pub fn de_trabajo(
        proyecto: String,
        destino: Option<String>,
        trabajo: Trabajo,
        responder: Sender<Result<Respuesta, String>>,
    ) -> Pedido {
        Pedido {
            trabajo,
            ..Pedido::nuevo(proyecto, destino, String::new(), responder)
        }
    }

    /// Una zona de una hoja.
    pub fn en_hoja(mut self, h: EnHoja) -> Pedido {
        self.hoja = Some(h);
        self
    }

    /// Con sus imagenes.
    pub fn con_imagenes(mut self, v: Vec<(u64, pixpin_codec::ImagenRgba)>) -> Pedido {
        self.imagenes = v;
        self
    }

    pub fn responder_con(self, r: Result<Respuesta, String>) {
        let _ = self.responder.send(r);
    }
}

// ---------------------------------------------------------------------------
// Las zonas de una hoja (`hoja_a_lienzo`, 14-oct)

/// **Una zona de una hoja.** Todas las de la misma `sesion` caen con el
/// mismo desplazamiento (la hoja entera centrada en lo que se ve, o a la
/// derecha de lo que habia si el lienzo esta cerrado), cada una en su sitio
/// de la hoja. `rehacer` cambia lo que ya puso esa zona (la comprobacion la
/// rehizo) y `ultima` cierra la hoja: si nadie toco el lienzo entre medias,
/// todas sus zonas quedan en **un solo paso de deshacer**.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnHoja {
    pub sesion: u64,
    pub zona: usize,
    pub rehacer: bool,
    pub ultima: bool,
    /// Lo que mide la hoja en el lienzo.
    pub tam: (f32, f32),
    /// La imagen de la que sale (su caja en el lienzo), si estaba en el: lo
    /// nuevo va a su lado (`sitio_libre`).
    pub junto_a: Option<(f32, f32, f32, f32)>,
}

/// Lo que se sabe de una hoja a medio meter.
struct Sesion {
    desplazamiento: (f32, f32),
    /// Los ids de fichero (`pc<hex>` o `ia-...`) de lo que puso cada zona:
    /// sirven igual en la escena abierta y en una releida del disco.
    zonas: std::collections::HashMap<usize, Vec<String>>,
    pasos_al_empezar: u64,
    pasos: usize,
}

static SESIONES: Mutex<Option<std::collections::HashMap<u64, Sesion>>> = Mutex::new(None);

/// **Mete una zona de una hoja.** `centro`: el de lo que se ve (lienzo
/// abierto); `None`, lienzo cerrado. Un documento vacio con `ultima` solo
/// cierra la hoja. Devuelve cuantos elementos entraron.
pub fn meter_de_hoja(
    escena: &mut Escena,
    documento: &str,
    h: &EnHoja,
    centro: Option<(f32, f32)>,
) -> Result<usize, String> {
    // Lo que se ve: una vista del tamano de la hoja alrededor del centro
    // (las pruebas de antes; el editor pasa la suya con `meter_de_hoja_en`).
    let vista = centro.map(|(cx, cy)| (cx - h.tam.0, cy - h.tam.1, cx + h.tam.0, cy + h.tam.1));
    meter_de_hoja_en(escena, documento, h, vista)
}

/// Lo mismo con lo que se ve `(x0, y0, x1, y1)` (`None`: lienzo cerrado). La
/// hoja entera va a un sitio libre (`sitio_libre`): junto a su imagen si
/// salio de una del lienzo, a la vista, sin tapar nada.
pub fn meter_de_hoja_en(
    escena: &mut Escena,
    documento: &str,
    h: &EnHoja,
    vista: Option<(f32, f32, f32, f32)>,
) -> Result<usize, String> {
    let mut g = SESIONES.lock().map_err(|_| "el lienzo esta ocupado")?;
    let sesiones = g.get_or_insert_with(Default::default);
    let s = sesiones.entry(h.sesion).or_insert_with(|| {
        let desplazamiento = sitio_libre(escena, h.tam, h.junto_a, vista);
        if let Some(v) = vista {
            let caja = (
                desplazamiento.0,
                desplazamiento.1,
                desplazamiento.0 + h.tam.0,
                desplazamiento.1 + h.tam.1,
            );
            if desplazar_para_ver(v, caja) != (0.0, 0.0) {
                pedir_mostrar(caja);
            }
        }
        Sesion {
            desplazamiento,
            zonas: Default::default(),
            pasos_al_empezar: escena.pasos_cerrados(),
            pasos: 0,
        }
    });
    let mut n = 0;
    if !documento.is_empty() {
        // Rehacer una zona con nada es quitarla (la tapaba otra que dice
        // lo mismo: banco del 15-oct, un renglon leido dos veces).
        let mut els = match elementos_de(documento) {
            Ok(v) => v,
            Err(_) if pixpin_motor2d::excalidraw::leer(documento).is_ok() => Vec::new(),
            Err(e) => return Err(e),
        };
        if h.rehacer
            && let Some(viejos) = s.zonas.get(&h.zona)
        {
            let quitar: Vec<u64> = escena
                .visibles()
                .filter(|e| viejos.contains(&pixpin_motor2d::enlace::id_de_texto_de(e)))
                .map(|e| e.id)
                .collect();
            // Quitar lo viejo y poner lo nuevo, en el mismo paso.
            escena.abrir_paso();
            for id in quitar {
                escena.borrar_apuntando(id);
            }
            if els.is_empty() {
                escena.cerrar_paso();
                s.pasos += 1;
                s.zonas.insert(h.zona, Vec::new());
                return Ok(0);
            }
        }
        // Ningun id repetido con lo que ya hay: luego se rehace por id.
        ids_sin_choques(escena, &mut els);
        let (dx, dy) = s.desplazamiento;
        let ids = pixpin_motor2d::portapapeles::pegar(escena, &els, dx, dy);
        n = ids.len();
        s.pasos += 1;
        let textos = ids
            .iter()
            .filter_map(|id| escena.buscar(*id))
            .map(pixpin_motor2d::enlace::id_de_texto_de)
            .collect();
        s.zonas.insert(h.zona, textos);
    }
    if h.ultima {
        let s = sesiones.remove(&h.sesion).expect("se acaba de usar");
        // Un solo paso de deshacer, si los ultimos son justo los nuestros.
        if s.pasos >= 2
            && escena.pasos_cerrados().saturating_sub(s.pasos_al_empezar) == s.pasos as u64
        {
            escena.juntar_pasos(s.pasos_al_empezar);
        }
    }
    Ok(n)
}

/// **Los ids de fichero de lo que entra, sin chocar con los del lienzo**
/// (ni entre si). Una hoja pasada dos veces, o una zona de otra hoja con la
/// misma semilla, traeria `ia-...` que ya estan: al rehacer una zona, que
/// borra por id, se llevaria piezas ajenas. El que choca estrena otro, y
/// los rotulos y flechas de la misma tanda que lo nombraban le siguen.
pub(crate) fn ids_sin_choques(escena: &Escena, els: &mut [Elemento]) {
    use std::collections::{HashMap, HashSet};
    let mut usados: HashSet<String> = escena
        .visibles()
        .map(pixpin_motor2d::enlace::id_de_texto_de)
        .collect();
    let mut cambio: HashMap<String, String> = HashMap::new();
    for e in els.iter_mut() {
        let Some(viejo) = e.extras.id_de_fichero.clone() else {
            continue;
        };
        if usados.insert(viejo.clone()) {
            continue;
        }
        let mut k = 2;
        let nuevo = loop {
            let c = format!("{viejo}-{k}");
            if !usados.contains(&c) {
                break c;
            }
            k += 1;
        };
        usados.insert(nuevo.clone());
        e.extras.id_de_fichero = Some(nuevo.clone());
        cambio.insert(viejo, nuevo);
    }
    if cambio.is_empty() {
        return;
    }
    let nuevo = |s: &mut String| {
        if let Some(n) = cambio.get(s.as_str()) {
            *s = n.clone();
        }
    };
    for e in els.iter_mut() {
        if let Some(c) = e.extras.contenedor.as_mut() {
            nuevo(c);
        }
        if let Some(g) = e.extras.enganche_inicio.as_mut() {
            nuevo(&mut g.elemento);
        }
        if let Some(g) = e.extras.enganche_fin.as_mut() {
            nuevo(&mut g.elemento);
        }
        for a in e.extras.atados.iter_mut() {
            nuevo(&mut a.id);
        }
    }
}

/// Si la hoja `sesion` sigue a medio meter (pruebas).
#[cfg(test)]
pub(crate) fn hay_sesion(sesion: u64) -> bool {
    SESIONES
        .lock()
        .is_ok_and(|g| g.as_ref().is_some_and(|m| m.contains_key(&sesion)))
}

/// **Las imagenes que el asistente pega**: las de un almacen de imagenes del
/// lienzo (el del editor abierto, o uno de paso para una hoja cerrada), con
/// los pixeles de un adjunto del chat o del portapapeles.
pub struct ConImagenes<'a> {
    pub imagenes: &'a mut crate::imagenes_lienzo::ImagenesLienzo,
    pub adjuntos: &'a [(String, std::path::PathBuf)],
}

impl Anfitrion for ConImagenes<'_> {
    fn imagen_nueva(&mut self, fuente: &str) -> Result<Elemento, String> {
        let img = imagen_de(fuente, self.adjuntos)?;
        let (w, h) = (img.ancho as f32, img.alto as f32);
        let id = self
            .imagenes
            .guardar(img)
            .ok_or("la imagen es demasiado grande para este equipo")?;
        Ok(crate::imagenes_lienzo::elemento_imagen(id, 0.0, 0.0, w, h))
    }
    fn tamano_de(&self, id_objeto: u64) -> Option<(u32, u32)> {
        self.imagenes.tamano(id_objeto)
    }
}

/// Los pixeles de lo que hay que pegar: «portapapeles», un adjunto por su
/// nombre, o (sin decir) la ultima imagen adjunta.
fn imagen_de(
    fuente: &str,
    adjuntos: &[(String, std::path::PathBuf)],
) -> Result<pixpin_codec::ImagenRgba, String> {
    let f = pixpin_ia::acciones::normalizar(fuente.trim());
    if f.contains("portapapeles") || f.contains("clipboard") {
        return match pixpin_codec::portapapeles::leer() {
            Some(pixpin_codec::ContenidoPortapapeles::Imagen(img))
                if img.ancho > 0 && img.alto > 0 =>
            {
                Ok(img)
            }
            Some(pixpin_codec::ContenidoPortapapeles::Rutas(r)) => {
                crate::imagenes_lienzo::imagen_de_rutas(&r)
                    .ok_or_else(|| "lo copiado no es una imagen".to_string())
            }
            _ => Err("no hay ninguna imagen copiada en el portapapeles".into()),
        };
    }
    let imagenes: Vec<&(String, std::path::PathBuf)> = adjuntos
        .iter()
        .filter(|(_, r)| {
            crate::imagenes_lienzo::imagen_de_rutas(std::slice::from_ref(r)).is_some()
                || es_imagen(r)
        })
        .collect();
    let elegida = if f.is_empty() {
        imagenes.last().copied()
    } else {
        imagenes
            .iter()
            .rev()
            .find(|(n, _)| pixpin_ia::acciones::normalizar(n).contains(&f))
            .copied()
            .or(imagenes.last().copied())
    };
    let (_, ruta) = elegida.ok_or("no hay ninguna imagen adjunta en este chat")?;
    crate::imagenes_lienzo::imagen_de_rutas(std::slice::from_ref(ruta))
        .ok_or_else(|| "no se pudo leer la imagen".to_string())
}

fn es_imagen(r: &std::path::Path) -> bool {
    r.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["png", "jpg", "jpeg", "bmp", "gif", "webp"].contains(&e.to_ascii_lowercase().as_str())
    })
}

/// **Hace un trabajo de editar o resumir sobre una escena** (la del editor
/// abierto o la de una hoja cerrada). `elegidos`: lo elegido en el editor;
/// `marco`: desde donde mide el modelo si es un resumen nuevo.
pub fn trabajar(
    escena: &mut Escena,
    trabajo: &Trabajo,
    elegidos: &[u64],
    marco: (f32, f32),
    imagenes: &mut crate::imagenes_lienzo::ImagenesLienzo,
) -> Result<Respuesta, String> {
    match trabajo {
        Trabajo::Resumir => {
            let ctx = Contexto {
                marco,
                elegidos: elegidos.to_vec(),
                ultimos: Vec::new(),
            };
            Ok(Respuesta {
                texto: format!(
                    "{}{}",
                    pixpin_ia::edicion::resumen(escena, &ctx),
                    guias_del_lienzo(escena)
                ),
                ids: Vec::new(),
                ctx: Some(ctx),
                ..Default::default()
            })
        }
        Trabajo::Editar {
            edicion,
            ctx,
            adjuntos,
        } => {
            let mut ctx = ctx.clone();
            // Lo elegido es lo de ahora, no lo de cuando empezo el turno.
            ctx.elegidos = elegidos.to_vec();
            let mut an = ConImagenes { imagenes, adjuntos };
            let h = pixpin_ia::edicion::aplicar(escena, edicion, &ctx, &mut an)?;
            Ok(Respuesta {
                texto: h.texto,
                ids: h.ids,
                ..Default::default()
            })
        }
        Trabajo::Ver { mirada, ctx } => {
            let mut ctx = ctx.clone();
            ctx.elegidos = elegidos.to_vec();
            let imagenes = &*imagenes;
            let enc = super::ojos::encargo_de(escena, mirada, &ctx, |id| {
                imagenes.rgba(id).cloned().map(super::ojos::Pixeles::Rgba)
            })?;
            Ok(Respuesta {
                vista: Some(Box::new(enc)),
                ..Default::default()
            })
        }
        Trabajo::Juntar { desde, esperados } => {
            if *esperados >= 2 && escena.pasos_cerrados().saturating_sub(*desde) == *esperados {
                escena.juntar_pasos(*desde);
            }
            Ok(Respuesta::default())
        }
        Trabajo::Imagen => {
            let (id, caja) =
                imagen_de_escena(escena, elegidos).ok_or("no hay ninguna imagen en el lienzo")?;
            let img = imagenes
                .rgba(id)
                .ok_or("la imagen del lienzo no esta cargada")?;
            Ok(Respuesta {
                imagen: Some(Box::new(ImagenDelLienzo {
                    rgba: img.pixeles.clone(),
                    ancho: img.ancho,
                    alto: img.alto,
                    caja,
                })),
                ..Default::default()
            })
        }
        Trabajo::Plantilla { nombre, intervalo } => {
            poner_plantilla(escena, elegidos, nombre.as_deref(), intervalo, None)
        }
        Trabajo::Secciones => secciones_de_la_plantilla(escena, elegidos).map(|texto| Respuesta {
            texto,
            ..Default::default()
        }),
        Trabajo::Copia => Ok(Respuesta {
            copia: Some(escena.elementos.clone()),
            ..Default::default()
        }),
        Trabajo::VolverA { copia } => {
            let cambio = escena.volver_a(copia);
            Ok(Respuesta {
                texto: if cambio {
                    "vuelto a la version guardada".into()
                } else {
                    "ya estaba como la version guardada".into()
                },
                ..Default::default()
            })
        }
        Trabajo::Comprobar => {
            let avisos = pixpin_ia::lint::revisar(escena, None);
            Ok(Respuesta {
                texto: avisos
                    .iter()
                    .map(|h| h.texto.clone())
                    .collect::<Vec<_>>()
                    .join("\n"),
                ..Default::default()
            })
        }
        Trabajo::Dibujar => Err("eso no es un trabajo de editar".into()),
    }
}

static COLA: Mutex<Vec<Pedido>> = Mutex::new(Vec::new());
/// La ventana del editor **activo** (0: ninguno): el que estuvo delante el
/// ultimo. Recibe lo que el asistente dibuja sin decir en que lienzo.
static EDITOR: AtomicIsize = AtomicIsize::new(0);
/// La ventana del chat, para despertarla.
static CHAT: AtomicIsize = AtomicIsize::new(0);

/// El chat se apunta para que lo despierten.
pub fn avisar_al_chat(hwnd: isize) {
    CHAT.store(hwnd, Ordering::SeqCst);
}

/// **Los lienzos abiertos** (23-oct-2026: se pueden abrir varios a la vez),
/// cada uno con la hoja que tiene (la referencia de su dibujo), si la dijo. El
/// ultimo es el activo ([`EDITOR`]). Lo que se pide para la hoja de uno de
/// ellos (`@lienzo:`, o el que eligio el asistente con
/// `usar_lienzo`/`crear_lienzo`) lo hace ese; lo que se pide para una hoja que
/// no esta abierta, el chat en su fichero (17-oct); lo que no dice hoja, el
/// activo.
static EDITORES: Mutex<Vec<Abierto>> = Mutex::new(Vec::new());

/// Un lienzo abierto: su ventana, su hoja y lo que lo abrio ([`abrir_como`]).
struct Abierto {
    hwnd: isize,
    hoja: Option<String>,
    clave: Option<String>,
}

thread_local! {
    /// El editor de este hilo (cada lienzo vive en el suyo).
    static ESTE: std::cell::Cell<isize> = const { std::cell::Cell::new(0) };
    /// Lo que abre el lienzo de este hilo (ver [`abrir_como`]).
    static CLAVE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// **Lo que va a abrir este hilo** (p. ej. «proyecto/mensaje»): el editor que
/// se abra aqui se apunta con ello y [`editor_de_clave`] lo encuentra, para
/// traerlo al frente en vez de abrir la misma hoja dos veces.
pub fn abrir_como(clave: &str) {
    CLAVE.with(|c| *c.borrow_mut() = Some(clave.to_string()));
}

/// El lienzo abierto con esa clave, si sigue abierto.
pub fn editor_de_clave(clave: &str) -> Option<isize> {
    EDITORES
        .lock()
        .ok()?
        .iter()
        .find(|a| a.clave.as_deref() == Some(clave))
        .map(|a| a.hwnd)
}

/// Mientras vive, hay un lienzo abierto y recibe lo que se dibuje para el.
pub struct EditorAbierto(isize);

impl EditorAbierto {
    /// Se apunta y pasa a ser el activo.
    pub fn nuevo(hwnd: isize) -> EditorAbierto {
        if let Ok(mut v) = EDITORES.lock() {
            v.retain(|a| a.hwnd != hwnd);
            v.push(Abierto {
                hwnd,
                hoja: None,
                clave: CLAVE.with(|c| c.borrow().clone()),
            });
        }
        EDITOR.store(hwnd, Ordering::SeqCst);
        ESTE.with(|e| e.set(hwnd));
        EditorAbierto(hwnd)
    }

    /// Con la hoja que tiene (la referencia de su dibujo).
    pub fn con_hoja(self, referencia: Option<String>) -> EditorAbierto {
        if let Ok(mut v) = EDITORES.lock()
            && let Some(e) = v.iter_mut().find(|a| a.hwnd == self.0)
        {
            e.hoja = referencia.filter(|r| !r.is_empty());
        }
        self
    }
}

/// **El lienzo que se pone delante pasa a ser el activo**: lo que el
/// asistente dibuje sin decir donde cae en el que el usuario mira.
pub fn activar(hwnd: isize) {
    if hwnd == 0 || EDITOR.load(Ordering::SeqCst) == hwnd {
        return;
    }
    if let Ok(mut v) = EDITORES.lock()
        && let Some(i) = v.iter().position(|a| a.hwnd == hwnd)
    {
        let e = v.remove(i);
        v.push(e);
        EDITOR.store(hwnd, Ordering::SeqCst);
    }
}

/// El editor que hace un pedido: el que tiene su hoja; sin hoja (o si
/// ninguno la tiene y el activo no dijo cual tiene), el activo; 0 si es del
/// chat.
fn editor_de(p: &Pedido) -> isize {
    let activo = EDITOR.load(Ordering::SeqCst);
    let Ok(v) = EDITORES.lock() else {
        return activo;
    };
    match &p.destino {
        None => activo,
        Some(d) => v
            .iter()
            .find(|a| a.hoja.as_deref() == Some(d.as_str()))
            .map(|a| a.hwnd)
            .or_else(|| {
                v.iter()
                    .any(|a| a.hwnd == activo && a.hoja.is_none())
                    .then_some(activo)
            })
            .unwrap_or(0),
    }
}

/// Si el pedido es para el editor de este hilo (o el activo, fuera de un
/// editor): sin destino si es el activo, o para su misma hoja, o si el
/// editor no dijo cual tiene y ningun otro la tiene.
pub fn es_para_el_editor(p: &Pedido) -> bool {
    let este = match ESTE.with(|e| e.get()) {
        0 => EDITOR.load(Ordering::SeqCst),
        h => h,
    };
    este != 0 && editor_de(p) == este
}

/// Si el editor de esta ventana es el activo.
pub fn es_el_activo(hwnd: isize) -> bool {
    hwnd != 0 && EDITOR.load(Ordering::SeqCst) == hwnd
}

impl Drop for EditorAbierto {
    fn drop(&mut self) {
        if let Ok(mut v) = EDITORES.lock() {
            v.retain(|a| a.hwnd != self.0);
            // El activo pasa al que estuvo delante antes.
            if EDITOR.load(Ordering::SeqCst) == self.0 {
                EDITOR.store(v.last().map_or(0, |a| a.hwnd), Ordering::SeqCst);
            }
        }
        ESTE.with(|e| {
            if e.get() == self.0 {
                e.set(0);
            }
        });
        // Lo que quedara en la cola es de otro: que se entere.
        if hay_pedidos() {
            despertar(EDITOR.load(Ordering::SeqCst));
            despertar(CHAT.load(Ordering::SeqCst));
        }
    }
}

/// Si alguien vaciaria la cola ahora (un editor abierto o el chat): sin
/// nadie, preguntar por el resumen del lienzo solo haria esperar.
pub fn hay_quien_conteste() -> bool {
    EDITOR.load(Ordering::SeqCst) != 0 || CHAT.load(Ordering::SeqCst) != 0
}

pub fn hay_editor() -> bool {
    EDITOR.load(Ordering::SeqCst) != 0
}

fn despertar(hwnd: isize) {
    if hwnd != 0 {
        pixpin_shell::overlay::despertar(hwnd);
    }
}

/// Deja el pedido en la cola y despierta a quien lo va a atender.
pub fn pedir(p: Pedido) {
    let editor = editor_de(&p);
    if let Ok(mut c) = COLA.lock() {
        c.push(p);
    }
    despertar(if editor != 0 {
        editor
    } else {
        CHAT.load(Ordering::SeqCst)
    });
}

/// Los pedidos que cumplen `para`; los demas se quedan en la cola.
pub fn tomar_si(para: impl Fn(&Pedido) -> bool) -> Vec<Pedido> {
    let Ok(mut c) = COLA.lock() else {
        return Vec::new();
    };
    let (si, no): (Vec<Pedido>, Vec<Pedido>) =
        std::mem::take(&mut *c).into_iter().partition(|p| para(p));
    *c = no;
    si
}

/// Lo que le toca al chat: todo si no hay editor; con editores, lo que es
/// para un lienzo que no esta abierto ([`es_para_el_editor`]).
pub fn tomar_para_el_chat() -> Vec<Pedido> {
    if hay_editor() {
        tomar_si(|p| editor_de(p) == 0)
    } else {
        tomar()
    }
}

pub fn hay_pedidos() -> bool {
    COLA.lock().is_ok_and(|c| !c.is_empty())
}

/// Los pedidos que haya, para atenderlos.
pub fn tomar() -> Vec<Pedido> {
    COLA.lock()
        .map(|mut c| std::mem::take(&mut *c))
        .unwrap_or_default()
}

/// Los elementos de un documento del asistente.
pub fn elementos_de(documento: &str) -> Result<Vec<Elemento>, String> {
    let lienzo = pixpin_motor2d::excalidraw::leer(documento)
        .map_err(|e| format!("el lienzo no lo entiende: {e}"))?;
    let els = lienzo.elementos();
    if els.is_empty() {
        return Err("no hay nada que dibujar".into());
    }
    Ok(els)
}

/// La caja que ocupan unos elementos: (x0, y0, x1, y1).
pub fn caja(els: &[Elemento]) -> Option<(f32, f32, f32, f32)> {
    els.iter()
        .map(Elemento::caja)
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
        .filter(|c| [c.0, c.1, c.2, c.3].iter().all(|v| v.is_finite()))
}

/// Hueco entre lo que habia y lo nuevo.
#[cfg_attr(not(test), allow(dead_code))]
const HUECO: f32 = 80.0;

/// **Mete lo dibujado a la derecha de lo que ya hay** (con un lienzo
/// cerrado), alineado arriba; en un lienzo vacio, donde lo puso el modelo.
#[cfg_attr(not(test), allow(dead_code))]
pub fn meter_al_lado(escena: &mut Escena, documento: &str) -> Result<usize, String> {
    meter_al_lado_ids(escena, documento).map(|v| v.len())
}

/// Lo mismo, devolviendo los ids de lo que entro.
#[cfg_attr(not(test), allow(dead_code))]
pub fn meter_al_lado_ids(escena: &mut Escena, documento: &str) -> Result<Vec<u64>, String> {
    let els = elementos_de(documento)?;
    let nueva = caja(&els).ok_or("las figuras no tienen medida")?;
    let habia = escena
        .visibles()
        .map(Elemento::caja)
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
        .filter(|c| [c.0, c.1, c.2, c.3].iter().all(|v| v.is_finite()));
    let (dx, dy) = match habia {
        Some(h) => (h.2 + HUECO - nueva.0, h.1 - nueva.1),
        None => (0.0, 0.0),
    };
    Ok(pixpin_motor2d::portapapeles::pegar(escena, &els, dx, dy))
}

/// El resumen que se le devuelve al modelo.
pub fn resumen(n: usize) -> String {
    format!("hecho: {n} elementos anadidos al lienzo")
}

/// El «hecho» de lo dibujado con sus `#id` (en el marco del modelo, si el
/// turno tiene uno), para poder editarlo en la llamada siguiente.
pub fn resumen_con_ids(escena: &Escena, ids: Vec<u64>, marco: Option<(f32, f32)>) -> Respuesta {
    let ctx = Contexto {
        marco: marco.unwrap_or_else(|| pixpin_ia::edicion::marco_de(escena)),
        ..Default::default()
    };
    Respuesta {
        texto: pixpin_ia::edicion::resumen_de_lo_anadido(escena, &ids, &ctx),
        ids,
        ..Default::default()
    }
}

/// Vacia la cola **en el editor abierto** sin imagenes ni seleccion (las
/// pruebas de antes). Ver [`atender_en_editor_con`].
#[cfg_attr(not(test), allow(dead_code))]
pub fn atender_en_editor(escena: &mut Escena, centro: (f32, f32)) -> bool {
    let mut imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(16_384);
    let vista = (
        centro.0 - 400.0,
        centro.1 - 300.0,
        centro.0 + 400.0,
        centro.1 + 300.0,
    );
    atender_en_editor_con(escena, vista, &[], &mut imagenes)
}

/// Vacia la cola **en el editor abierto**: lo llama su bucle en cada vuelta.
/// `vista` es lo que se ve `(x0, y0, x1, y1)`, en coordenadas del lienzo: lo
/// dibujado cae en su centro y el modelo mide desde su esquina. `elegidos`
/// es lo elegido ahora; `imagenes`, el almacen del editor (para pegar).
/// Devuelve si cambio algo (para repintar).
pub fn atender_en_editor_con(
    escena: &mut Escena,
    vista: (f32, f32, f32, f32),
    elegidos: &[u64],
    imagenes: &mut crate::imagenes_lienzo::ImagenesLienzo,
) -> bool {
    if !hay_pedidos() {
        return false;
    }
    let mut algo = false;
    for mut p in tomar_si(es_para_el_editor) {
        let r = match &p.trabajo {
            Trabajo::Dibujar if p.hoja.is_some() => {
                let h = p.hoja.expect("se acaba de mirar");
                let r =
                    meter_de_hoja_en(escena, &p.documento, &h, Some(vista)).map(|n| Respuesta {
                        texto: resumen(n),
                        ..Default::default()
                    });
                if r.is_ok() {
                    // Las fotos de la hoja, al almacen (nuevas: se guardan con el lienzo).
                    for (id, img) in std::mem::take(&mut p.imagenes) {
                        imagenes.reponer_nueva(id, img);
                    }
                }
                r
            }
            Trabajo::Dibujar => {
                let antes = escena.pasos_cerrados();
                meter_en_sitio(escena, &p.documento, p.junto_a, Some(vista)).map(|ids| Respuesta {
                    pasos: Some((antes, escena.pasos_cerrados())),
                    ..resumen_con_ids(escena, ids, p.marco)
                })
            }
            Trabajo::Plantilla { nombre, intervalo } => {
                let antes = escena.pasos_cerrados();
                let centro = ((vista.0 + vista.2) / 2.0, (vista.1 + vista.3) / 2.0);
                let r =
                    poner_plantilla(escena, elegidos, nombre.as_deref(), intervalo, Some(centro));
                algo |= escena.pasos_cerrados() != antes;
                r.map(|r| Respuesta {
                    pasos: Some((antes, escena.pasos_cerrados())),
                    ..r
                })
            }
            t => {
                let antes = escena.pasos_cerrados();
                let r = trabajar(
                    escena,
                    t,
                    elegidos,
                    (vista.0.floor(), vista.1.floor()),
                    imagenes,
                );
                algo |= escena.pasos_cerrados() != antes;
                // Lo que edito, para juntarlo al acabar el turno.
                r.map(|r| match t {
                    Trabajo::Editar { .. } | Trabajo::VolverA { .. } => Respuesta {
                        pasos: Some((antes, escena.pasos_cerrados())),
                        ..r
                    },
                    _ => r,
                })
            }
        };
        algo |= r.is_ok() && matches!(p.trabajo, Trabajo::Dibujar);
        p.responder_con(r);
    }
    algo
}

// ---------------------------------------------------------------------------
// «Guarda esto como figura»: lo elegido en el lienzo abierto
// ---------------------------------------------------------------------------

/// Quien espera lo elegido (el turno del asistente). `HAY_PREGUNTA` evita
/// mirar el candado en cada vuelta del editor.
static PREGUNTA: Mutex<Option<Sender<Vec<Elemento>>>> = Mutex::new(None);
static HAY_PREGUNTA: AtomicBool = AtomicBool::new(false);

/// **Pide al lienzo abierto lo que tiene elegido** (vacio si nada). Lo
/// contesta su bucle en la siguiente vuelta ([`contestar_lo_elegido`]).
pub fn pedir_lo_elegido() -> Receiver<Vec<Elemento>> {
    let (tx, rx) = channel();
    if let Ok(mut p) = PREGUNTA.lock() {
        *p = Some(tx);
        HAY_PREGUNTA.store(true, Ordering::SeqCst);
    }
    despertar(EDITOR.load(Ordering::SeqCst));
    rx
}

/// El bucle del editor contesta, si alguien pregunto, con copias de lo
/// elegido. Casi siempre no hay pregunta y no cuesta nada.
pub fn contestar_lo_elegido(escena: &Escena, elegidos: &[u64]) {
    if !HAY_PREGUNTA.load(Ordering::SeqCst) {
        return;
    }
    // Con varios lienzos abiertos contesta el activo.
    let este = ESTE.with(|e| e.get());
    if este != 0 && !es_el_activo(este) {
        return;
    }
    HAY_PREGUNTA.store(false, Ordering::SeqCst);
    let Some(tx) = PREGUNTA.lock().ok().and_then(|mut p| p.take()) else {
        return;
    };
    let piezas: Vec<Elemento> = escena
        .visibles()
        .filter(|e| elegidos.contains(&e.id))
        .cloned()
        .collect();
    let _ = tx.send(piezas);
}

#[cfg(test)]
pub(crate) fn vaciar_para_pruebas() {
    let _ = tomar();
    EDITOR.store(0, Ordering::SeqCst);
    if let Ok(mut v) = EDITORES.lock() {
        v.clear();
    }
    ESTE.with(|e| e.set(0));
    CLAVE.with(|c| *c.borrow_mut() = None);
}

// ---------------------------------------------------------------------------
// La imagen del lienzo como entrada y lo nuevo, a la vista (15-oct)

/// **Una imagen del lienzo** (la elegida o la ultima pegada) para calcarla o
/// pasarla: sus pixeles y su caja en el lienzo.
#[derive(Debug, Clone, PartialEq)]
pub struct ImagenDelLienzo {
    pub rgba: Vec<u8>,
    pub ancho: u32,
    pub alto: u32,
    /// (x0, y0, x1, y1) en el lienzo.
    pub caja: (f32, f32, f32, f32),
}

/// **Que imagen del lienzo se usa**: la elegida (si alguna de lo elegido es
/// una imagen) o, si no, la ultima pegada (la de encima). Su id de objeto y
/// su caja.
pub fn imagen_de_escena(escena: &Escena, elegidos: &[u64]) -> Option<(u64, (f32, f32, f32, f32))> {
    let es_imagen = |e: &&Elemento| matches!(e.figura, pixpin_motor2d::Figura::Imagen { .. });
    let elegida = escena
        .visibles()
        .filter(es_imagen)
        .filter(|e| elegidos.contains(&e.id))
        .last();
    let e = elegida.or_else(|| escena.visibles().filter(es_imagen).last())?;
    let pixpin_motor2d::Figura::Imagen { id_objeto } = e.figura else {
        return None;
    };
    Some((id_objeto, e.caja()))
}

/// Hueco entre lo que habia y lo nuevo, y con lo que no se puede pisar.
const AIRE: f32 = 40.0;

/// **Donde poner algo nuevo de `tam`** (su esquina de arriba a la
/// izquierda) sin tapar nada: a la derecha de la imagen de la que sale (o
/// debajo, o a su izquierda, o encima); si no hay imagen, en el centro de lo
/// que se ve o, con el lienzo cerrado, junto a lo ultimo que se puso. Con
/// lo que se ve, se prefiere un sitio que se vea entero.
///
/// Antes, con el lienzo cerrado, lo nuevo iba «a la derecha de todo lo que
/// habia»: en un lienzo ancho acababa en x = 15000, fuera de la vista y
/// lejos de la imagen de la que salia (queja del 15-oct, «traelo»).
pub fn sitio_libre(
    escena: &Escena,
    tam: (f32, f32),
    junto_a: Option<(f32, f32, f32, f32)>,
    vista: Option<(f32, f32, f32, f32)>,
) -> (f32, f32) {
    let (w, h) = (tam.0.max(1.0), tam.1.max(1.0));
    let cajas: Vec<(f32, f32, f32, f32)> = escena
        .visibles()
        .map(Elemento::caja)
        .filter(|c| [c.0, c.1, c.2, c.3].iter().all(|v| v.is_finite()))
        .collect();
    let libre = |x: f32, y: f32| {
        let (x0, y0, x1, y1) = (
            x - AIRE / 2.0,
            y - AIRE / 2.0,
            x + w + AIRE / 2.0,
            y + h + AIRE / 2.0,
        );
        !cajas
            .iter()
            .any(|c| c.0 < x1 && c.2 > x0 && c.1 < y1 && c.3 > y0)
    };
    let se_ve =
        |x: f32, y: f32| vista.is_none_or(|v| x >= v.0 && y >= v.1 && x + w <= v.2 && y + h <= v.3);
    let mut candidatos: Vec<(f32, f32)> = Vec::new();
    if let Some(s) = junto_a {
        candidatos.push((s.2 + AIRE, s.1));
        candidatos.push((s.0, s.3 + AIRE));
        candidatos.push((s.0 - AIRE - w, s.1));
        candidatos.push((s.0, s.1 - AIRE - h));
    }
    if let Some(v) = vista {
        candidatos.push(((v.0 + v.2) / 2.0 - w / 2.0, (v.1 + v.3) / 2.0 - h / 2.0));
    } else if let Some(u) = cajas.last() {
        // Lo ultimo que se puso (lo de encima).
        candidatos.push((u.2 + AIRE, u.1));
        candidatos.push((u.0, u.3 + AIRE));
    }
    if candidatos.is_empty() {
        return (0.0, 0.0);
    }
    // 1. Libre y a la vista; 2. libre.
    if let Some(&c) = candidatos
        .iter()
        .find(|c| libre(c.0, c.1) && se_ve(c.0, c.1))
    {
        return c;
    }
    if vista.is_none()
        && let Some(&c) = candidatos.iter().find(|c| libre(c.0, c.1))
    {
        return c;
    }
    // 3. En espiral alrededor del primero, hasta encontrar sitio (primero a
    //    la vista).
    let (bx, by) = candidatos[0];
    let paso = (w.max(h) / 2.0).max(40.0);
    let mut libres = Vec::new();
    for n in 1..=10i32 {
        for i in -n..=n {
            for j in -n..=n {
                if i.abs() != n && j.abs() != n {
                    continue;
                }
                let (x, y) = (bx + i as f32 * paso, by + j as f32 * paso);
                if libre(x, y) {
                    if se_ve(x, y) {
                        return (x, y);
                    }
                    libres.push((x, y));
                }
            }
        }
        if !libres.is_empty() && vista.is_none() {
            break;
        }
    }
    libres
        .first()
        .copied()
        .or_else(|| candidatos.iter().copied().find(|c| libre(c.0, c.1)))
        .unwrap_or(candidatos[0])
}

// Lo que se pide que la ventana del lienzo ensene (lo nuevo no se veia).
// Por hilo: cada lienzo abierto vive en el suyo y lo pide su propio bucle.
thread_local! {
    static MOSTRAR: std::cell::Cell<Option<(f32, f32, f32, f32)>> = const { std::cell::Cell::new(None) };
}

/// Pide que el lienzo de este hilo se desplace hasta ensenar la caja.
pub fn pedir_mostrar(caja: (f32, f32, f32, f32)) {
    MOSTRAR.with(|m| m.set(Some(caja)));
}

/// Lo que hay que ensenar (lo toma el bucle del lienzo).
pub fn tomar_mostrar() -> Option<(f32, f32, f32, f32)> {
    MOSTRAR.with(|m| m.take())
}

/// **Cuanto mover la vista para que se vea la caja**: nada si ya se ve
/// entera; si no, lo que lleva su centro al centro de la vista.
pub fn desplazar_para_ver(vista: (f32, f32, f32, f32), caja: (f32, f32, f32, f32)) -> (f32, f32) {
    let dentro = caja.0 >= vista.0 && caja.1 >= vista.1 && caja.2 <= vista.2 && caja.3 <= vista.3;
    if dentro {
        return (0.0, 0.0);
    }
    (
        ((caja.0 + caja.2) - (vista.0 + vista.2)) / 2.0,
        ((caja.1 + caja.3) - (vista.1 + vista.3)) / 2.0,
    )
}

/// **Mete lo dibujado en un sitio libre**: junto a la imagen de la que sale
/// (si hay), a la vista si la hay (y si no se ve, pide que la vista se
/// mueva). En un solo paso de deshacer. Devuelve los ids.
pub fn meter_en_sitio(
    escena: &mut Escena,
    documento: &str,
    junto_a: Option<(f32, f32, f32, f32)>,
    vista: Option<(f32, f32, f32, f32)>,
) -> Result<Vec<u64>, String> {
    let els = elementos_de(documento)?;
    let c = caja(&els).ok_or("las figuras no tienen medida")?;
    let (x, y) = sitio_libre(escena, (c.2 - c.0, c.3 - c.1), junto_a, vista);
    let (dx, dy) = (x - c.0, y - c.1);
    if let Some(v) = vista {
        let nueva = (x, y, x + c.2 - c.0, y + c.3 - c.1);
        if desplazar_para_ver(v, nueva) != (0.0, 0.0) {
            pedir_mostrar(nueva);
        }
    }
    Ok(pixpin_motor2d::portapapeles::pegar(escena, &els, dx, dy))
}

// ---------------------------------------------------------------------------
// Las plantillas de serie con otro intervalo (22-oct)

// Lo que el editor abierto tiene que dejar elegido al acabar (las hojas
// nuevas de una plantilla): lo recoge su bucle con `tomar_para_elegir`. Por
// hilo, como `MOSTRAR`.
thread_local! {
    static PARA_ELEGIR: std::cell::RefCell<Option<Vec<u64>>> = const { std::cell::RefCell::new(None) };
}

/// Lo que hay que dejar elegido en el editor, si algo.
pub fn tomar_para_elegir() -> Option<Vec<u64>> {
    PARA_ELEGIR.with(|p| p.borrow_mut().take())
}

/// Hueco entre dos hojas de la misma plantilla.
const ENTRE_HOJAS: f32 = 60.0;

/// **«Damela del 1 al 40», «de 10 en 10»**: la plantilla de serie elegida
/// (o la de `nombre`) rehecha con ese intervalo, con el mismo diseño
/// (`figuras_de_serie::alargar`). Con `intervalo.secciones`, tambien
/// **personalizada** (25-oct: quitar, reordenar, ensanchar secciones), con
/// el mismo codigo que la dibuja. Con `intervalo.noche`, **de noche** (o de
/// dia): sin decirlo, la que se rehace se queda en su modo, y la de
/// «nombre» sale como se llame («Cronograma (noche)» de noche).
///
/// Si lo elegido es esa plantilla, las hojas nuevas **ocupan su sitio**: la
/// primera donde estaba y las demas en fila a su derecha; la vieja se borra
/// en el mismo paso, asi que Ctrl+Z la devuelve. Si no, van centradas en
/// `centro` (lo que se ve) o, con el lienzo cerrado, a la derecha de lo que
/// hubiera. Las hojas quedan elegidas, listas para «Guardar en figuras».
pub fn poner_plantilla(
    escena: &mut Escena,
    elegidos: &[u64],
    nombre: Option<&str>,
    intervalo: &pixpin_motor2d::figuras_de_serie::Intervalo,
    centro: Option<(f32, f32)>,
) -> Result<Respuesta, String> {
    use pixpin_motor2d::figuras_de_serie as serie;
    let mut elegidas: Vec<Elemento> = escena
        .visibles()
        .filter(|e| elegidos.contains(&e.id))
        .cloned()
        .collect();
    let mut la_elegida = serie::cual_es(&elegidas);
    // Solo el modo («ponlo en modo noche»): vale cualquier plantilla de
    // pagina, tambien las que no se alargan ni se personalizan.
    let solo_modo = intervalo.noche.is_some()
        && intervalo.secciones.is_none()
        && intervalo.desde.is_none()
        && intervalo.hasta.is_none()
        && intervalo.de_a.is_none()
        && intervalo.semanas.is_none()
        && intervalo.mes.is_none()
        && intervalo.meses.is_none();
    // **Sin elegirla** (25-oct-2026, el usuario abrio el cronograma y pidio
    // agrandarlo sin seleccionarlo): si en el lienzo hay una sola plantilla
    // que se alarga (y es la pedida, si se nombro), es esa, y se rehace en
    // su sitio con su mismo diseño.
    if la_elegida.is_none() {
        let pedida = nombre.and_then(|n| {
            let catalogo = serie::catalogo();
            pixpin_ia::figuras::buscar(&catalogo, n)
                .and_then(serie::clave_de)
                .map(str::to_string)
        });
        if let Some((c, v)) = plantilla_en_la_escena(escena, pedida.as_deref(), solo_modo) {
            la_elegida = Some(c);
            elegidas = v;
        }
    }
    // Pedida por su nombre de noche («cronograma (noche)»): de noche.
    let mut nombrada_de_noche = false;
    let clave = match nombre {
        Some(n) => {
            let catalogo = serie::catalogo();
            let f = pixpin_ia::figuras::buscar(&catalogo, n).ok_or_else(|| {
                format!(
                    "no hay ninguna plantilla llamada «{n}»; se alargan: {}",
                    serie::las_que_se_alargan()
                )
            })?;
            nombrada_de_noche = serie::es_la_de_noche(f);
            serie::clave_de(f).unwrap_or_default().to_string()
        }
        None => la_elegida.clone().ok_or_else(|| {
            if elegidas.is_empty() {
                "no hay nada elegido: elige en el lienzo la plantilla (o di cual quieres)"
                    .to_string()
            } else {
                "lo elegido no es una plantilla de serie (se reconocen por su titulo)".to_string()
            }
        })?,
    };
    // **Lo que no se dice se queda como esta** (25-oct-2026: «redúcelo a 10
    // semanas» volvia a las 18 actividades de fabrica): rehaciendo la que hay,
    // las filas y las semanas que no se piden salen de las suyas.
    // Lo mismo con el modo (25-oct): la de noche se rehace de noche salvo
    // que se pida el de dia, y el mes de un mensual o unos habitos se queda.
    let completo;
    let intervalo = if la_elegida.as_deref() == Some(clave.as_str()) {
        let actual = tramo_actual(&elegidas);
        let mut iv = intervalo.clone();
        let sin_numeros =
            iv.desde.is_none() && iv.hasta.is_none() && iv.mes.is_none() && iv.de_a.is_none();
        let por_meses = matches!(clave.as_str(), "planificador-mensual" | "habitos");
        if sin_numeros && por_meses && actual.mes.is_some() {
            iv.mes = actual.mes;
        } else if sin_numeros && serie::se_alarga(&clave) && clave != "planificador-mensual" {
            if let Some((a, b)) = actual.filas {
                iv.desde = Some(a);
                iv.hasta = Some(b);
            }
        }
        if iv.semanas.is_none() && clave == "cronograma" {
            iv.semanas = actual.semanas;
        }
        if iv.noche.is_none() && (actual.noche || nombrada_de_noche) {
            iv.noche = Some(true);
        }
        completo = iv;
        &completo
    } else if intervalo.noche.is_none() && nombrada_de_noche {
        completo = pixpin_motor2d::figuras_de_serie::Intervalo {
            noche: Some(true),
            ..intervalo.clone()
        };
        &completo
    } else {
        intervalo
    };
    let hojas = serie::alargar(&clave, intervalo)?;
    let (ancho, alto) = medida_de_las_hojas(&hojas);
    let reemplaza = la_elegida.as_deref() == Some(clave.as_str());
    // La esquina de la primera hoja.
    let (x0, y0) = match (reemplaza, centro) {
        (true, _) => caja(&elegidas).map(|c| (c.0, c.1)).unwrap_or_default(),
        (false, Some((cx, cy))) => (cx - ancho / 2.0, cy - alto / 2.0),
        (false, None) => match caja(&escena.visibles().cloned().collect::<Vec<_>>()) {
            Some(c) => (c.2 + HUECO_PLANTILLA, c.1),
            None => (0.0, 0.0),
        },
    };
    escena.abrir_paso();
    if reemplaza {
        for e in &elegidas {
            escena.borrar_apuntando(e.id);
        }
    }
    let ids = colocar_hojas(escena, &hojas, (x0, y0));
    escena.cerrar_paso();
    // Solo con el editor abierto (`centro`): cerrado, nadie lo recogeria.
    if centro.is_some() {
        PARA_ELEGIR.with(|p| *p.borrow_mut() = Some(ids.clone()));
    }
    let que = serie::nombre_de(&clave);
    let texto = if solo_modo {
        format!(
            "listo: «{que}» rehecha {} con el mismo diseño ({})",
            if intervalo.noche == Some(true) {
                "en modo noche"
            } else {
                "de día"
            },
            hojas[0].nombre
        )
    } else if intervalo.secciones.is_some() && hojas.len() == 1 {
        format!(
            "listo: «{que}» rehecha con las secciones pedidas ({})",
            hojas[0].nombre
        )
    } else if hojas.len() == 1 {
        format!("listo: «{que}» rehecha en una hoja ({})", hojas[0].nombre)
    } else {
        format!(
            "listo: «{que}» en {} hojas, una al lado de otra",
            hojas.len()
        )
    };
    let texto = format!(
        "{texto}{}. Quedan elegidas: el usuario puede guardarlas con «Guardar en figuras» del panel lateral, o pedirtelo.",
        if reemplaza {
            ", en el sitio de la que estaba elegida (Ctrl+Z la devuelve)"
        } else {
            ""
        }
    );
    Ok(Respuesta {
        texto,
        ids,
        ..Default::default()
    })
}

/// Lo que lleva ahora una plantilla puesta (ver [`tramo_actual`]).
#[derive(Debug, Default, PartialEq)]
struct Actual {
    /// Sus filas: la numeracion «01», «02»… seguida mas larga.
    filas: Option<(u32, u32)>,
    /// En el cronograma, sus semanas («S1»…).
    semanas: Option<(u32, u32)>,
    /// El mes de un mensual o unos habitos con el calendario de verdad.
    mes: Option<(i32, u32)>,
    /// Si es de noche (su papel es oscuro).
    noche: bool,
}

/// **Lo que lleva ahora una plantilla puesta**: sus filas, sus semanas, su
/// mes y su modo, para rehacerla sin perder nada de eso.
fn tramo_actual(piezas: &[Elemento]) -> Actual {
    let textos: Vec<&str> = piezas
        .iter()
        .filter_map(|e| match &e.figura {
            pixpin_motor2d::Figura::Texto { texto, .. } => Some(texto.trim()),
            _ => None,
        })
        .collect();
    let mut numeros: Vec<u32> = textos
        .iter()
        .filter(|t| t.len() >= 2 && t.chars().all(|c| c.is_ascii_digit()))
        .filter_map(|t| t.parse().ok())
        .collect();
    numeros.sort_unstable();
    numeros.dedup();
    // La tirada seguida mas larga: la de las filas, no un año o un dia suelto.
    let mut mejor: Option<(u32, u32)> = None;
    let mut i = 0;
    while i < numeros.len() {
        let mut j = i;
        while j + 1 < numeros.len() && numeros[j + 1] == numeros[j] + 1 {
            j += 1;
        }
        if j > i && mejor.is_none_or(|(a, b)| numeros[j] - numeros[i] > b - a) {
            mejor = Some((numeros[i], numeros[j]));
        }
        i = j + 1;
    }
    let semanas: Vec<u32> = textos
        .iter()
        .filter_map(|t| t.strip_prefix('S')?.parse::<u32>().ok())
        .collect();
    let semanas = semanas
        .iter()
        .min()
        .zip(semanas.iter().max())
        .map(|(a, b)| (*a, *b));
    Actual {
        filas: mejor,
        semanas,
        mes: pixpin_motor2d::figuras_de_serie::mes_de_la_hoja(piezas),
        noche: pixpin_motor2d::figuras_de_serie::es_de_noche(piezas),
    }
}

/// **La plantilla de serie que hay en el lienzo** (la que se alarga y, si se
/// dice, la de `clave`), con sus piezas: las de un mismo grupo exterior, que
/// es el que pone estampar. `None` si no hay ninguna o hay varias (entonces
/// hay que elegirla). Con `cualquiera`, tambien las que no se alargan ni se
/// personalizan (para cambiarles solo el modo, de dia o de noche).
fn plantilla_en_la_escena(
    escena: &Escena,
    clave: Option<&str>,
    cualquiera: bool,
) -> Option<(String, Vec<Elemento>)> {
    use pixpin_motor2d::figuras_de_serie as serie;
    let mut grupos: Vec<(String, Vec<Elemento>)> = Vec::new();
    for e in escena.visibles() {
        let Some(g) = e.grupos.last() else {
            continue;
        };
        match grupos.iter_mut().find(|(n, _)| n == g) {
            Some((_, v)) => v.push(e.clone()),
            None => grupos.push((g.clone(), vec![e.clone()])),
        }
    }
    let mut halladas: Vec<(String, Vec<Elemento>)> = grupos
        .into_iter()
        .filter_map(|(_, v)| serie::cual_es(&v).map(|c| (c, v)))
        .filter(|(c, _)| {
            (cualquiera || serie::se_alarga(c) || serie::se_personaliza(c))
                && clave.is_none_or(|k| k == c)
        })
        .collect();
    if halladas.len() == 1 {
        return halladas.pop();
    }
    if !halladas.is_empty() {
        return None;
    }
    // **Sin grupo** (desagrupada, o venida de otro sitio): por su titulo, y
    // sus piezas son lo que cae dentro de la hoja que lo encierra.
    let vivos: Vec<Elemento> = escena.visibles().cloned().collect();
    let c = serie::cual_es(&vivos)
        .filter(|c| (cualquiera || serie::se_alarga(c)) && clave.is_none_or(|k| k == c))?;
    let hojas: Vec<&Elemento> = vivos
        .iter()
        .filter(|e| matches!(e.figura, pixpin_motor2d::Figura::Rectangulo))
        .filter(|r| {
            let caja = r.caja();
            let dentro: Vec<Elemento> = vivos
                .iter()
                .filter(|o| {
                    let oc = o.caja();
                    let (cx, cy) = ((oc.0 + oc.2) / 2.0, (oc.1 + oc.3) / 2.0);
                    cx >= caja.0 && cx <= caja.2 && cy >= caja.1 && cy <= caja.3
                })
                .cloned()
                .collect();
            dentro.len() >= 8 && serie::cual_es(&dentro).as_deref() == Some(c.as_str())
        })
        .collect();
    // La hoja: el rectangulo mas grande que la contiene, una sola vez.
    let hoja = hojas.iter().max_by(|a, b| {
        let area = |e: &Elemento| {
            let c = e.caja();
            (c.2 - c.0) * (c.3 - c.1)
        };
        area(a).total_cmp(&area(b))
    })?;
    let caja = hoja.caja();
    let piezas: Vec<Elemento> = vivos
        .into_iter()
        .filter(|o| {
            let oc = o.caja();
            let (cx, cy) = ((oc.0 + oc.2) / 2.0, (oc.1 + oc.3) / 2.0);
            cx >= caja.0 - 0.5 && cx <= caja.2 + 0.5 && cy >= caja.1 - 0.5 && cy <= caja.3 + 0.5
        })
        .collect();
    Some((c, piezas))
}

/// Con el lienzo cerrado, a cuanto de lo que habia.
const HUECO_PLANTILLA: f32 = 120.0;

/// **Lo que la plantilla tiene AHORA** (24-oct-2026, banco con Haiku): la
/// guia trae las secciones de serie, y con una ya personalizada el modelo
/// volvia a mandarlas y deshacia lo de antes. Sus bloques con nombre (las
/// partes `parte:<nombre>`), en orden de arriba abajo.
fn ahora_tiene(piezas: &[Elemento]) -> String {
    let mut vistos: Vec<(String, f32, f32)> = Vec::new();
    for e in piezas {
        let Some(n) = e
            .grupos
            .iter()
            .find_map(|g| pixpin_motor2d::biblioteca::nombre_de_parte(g))
        else {
            continue;
        };
        let c = e.caja();
        match vistos.iter_mut().find(|(v, _, _)| v == n) {
            Some(v) => {
                v.1 = v.1.min(c.1);
                v.2 = v.2.min(c.0);
            }
            None => vistos.push((n.to_string(), c.1, c.0)),
        }
    }
    if vistos.is_empty() {
        return String::new();
    }
    vistos.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)));
    let nombres: Vec<String> = vistos.iter().map(|(n, _, _)| format!("«{n}»")).collect();
    format!(
        "\nLo que tiene AHORA en el lienzo (de arriba abajo): {}. Si difiere de lo de serie es que ya se personalizó: \
parte de esto (no vuelvas a poner lo que se quitó y conserva lo que se añadió), cambiando solo lo pedido.",
        nombres.join(", ")
    )
}

/// Como mucho cuantas guias de plantilla van en el resumen (son largas).
const MAX_GUIAS: usize = 2;

/// **La guia de las plantillas que hay en el lienzo** (25-oct): por cada
/// plantilla de serie que se personaliza, sus bloques y lo que se le puede
/// cambiar (`figuras_de_serie::guia_de`), para que el modelo la entienda
/// sin tener que pedirla. Vacio si no hay ninguna.
fn guias_del_lienzo(escena: &Escena) -> String {
    use pixpin_motor2d::figuras_de_serie as serie;
    let mut grupos: Vec<(String, Vec<Elemento>)> = Vec::new();
    for e in escena.visibles() {
        let Some(g) = e.grupos.last() else {
            continue;
        };
        match grupos.iter_mut().find(|(n, _)| n == g) {
            Some((_, v)) => v.push(e.clone()),
            None => grupos.push((g.clone(), vec![e.clone()])),
        }
    }
    let mut claves: Vec<(String, String)> = Vec::new();
    for (_, v) in &grupos {
        if let Some(c) = serie::cual_es(v)
            && serie::se_personaliza(&c)
            && !claves.iter().any(|(k, _)| *k == c)
        {
            claves.push((c, ahora_tiene(v)));
        }
    }
    claves
        .iter()
        .take(MAX_GUIAS)
        .filter_map(|(c, ahora)| serie::guia_de(c).map(|g| format!("\n\n{g}{ahora}")))
        .collect()
}

/// **Las secciones de la plantilla del lienzo**, en palabras para el
/// modelo (`figuras_de_serie::guia_de`): la elegida o, sin
/// elegir, la unica plantilla que hay. Error en palabras si no hay ninguna
/// o no se personaliza.
fn secciones_de_la_plantilla(escena: &Escena, elegidos: &[u64]) -> Result<String, String> {
    use pixpin_motor2d::figuras_de_serie as serie;
    let elegidas: Vec<Elemento> = escena
        .visibles()
        .filter(|e| elegidos.contains(&e.id))
        .cloned()
        .collect();
    let (clave, piezas) = serie::cual_es(&elegidas)
        .map(|c| (c, elegidas.clone()))
        .or_else(|| plantilla_en_la_escena(escena, None, false))
        .ok_or_else(|| {
            format!(
                "no encuentro la plantilla en el lienzo: elígela o di su «nombre» (se personalizan: {})",
                serie::las_que_se_personalizan()
            )
        })?;
    let ahora = ahora_tiene(&piezas);
    serie::guia_de(&clave)
        .map(|g| format!("{g}{ahora}"))
        .ok_or_else(|| {
            format!(
                "«{}» no tiene secciones que cambiar; se personalizan: {}",
                serie::nombre_de(&clave),
                serie::las_que_se_personalizan()
            )
        })
}

use pixpin_motor2d::biblioteca::FiguraGuardada;

#[cfg(test)]
mod pruebas_plantilla {
    use super::*;
    use pixpin_motor2d::biblioteca;
    use pixpin_motor2d::figuras_de_serie::{self as serie, Intervalo};

    /// Un lienzo con la lista de tareas de serie estampada y elegida.
    fn con_la_lista() -> (Escena, Vec<u64>) {
        let mut escena = Escena::nueva();
        let f = serie::catalogo()
            .into_iter()
            .find(|f| f.id == "serie-lista-de-tareas")
            .unwrap();
        let ids = biblioteca::estampar_en_escena(
            &mut escena,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(500.0, 600.0),
        );
        (escena, ids)
    }

    #[test]
    fn la_plantilla_elegida_se_rehace_en_su_sitio_en_hojas_y_se_deshace_de_una() {
        let (mut escena, ids) = con_la_lista();
        let caja_vieja = caja(&escena.visibles().cloned().collect::<Vec<_>>()).unwrap();
        let iv = Intervalo {
            desde: Some(1),
            hasta: Some(40),
            de_a: Some(10),
            ..Default::default()
        };
        let r = poner_plantilla(&mut escena, &ids, None, &iv, Some((0.0, 0.0))).unwrap();
        assert!(r.texto.contains("4 hojas"), "{}", r.texto);
        assert!(
            ids.iter()
                .all(|id| escena.buscar(*id).is_some_and(|e| e.borrado)),
            "la vieja se va"
        );
        let nuevas: Vec<Elemento> = escena.visibles().cloned().collect();
        let grupos: std::collections::HashSet<&String> =
            nuevas.iter().filter_map(|e| e.grupos.last()).collect();
        assert_eq!(grupos.len(), 4, "una hoja, un grupo");
        let c = caja(&nuevas).unwrap();
        assert!(
            (c.0 - caja_vieja.0).abs() < 0.5 && (c.1 - caja_vieja.1).abs() < 0.5,
            "empieza donde estaba la vieja"
        );
        assert_eq!(
            tomar_para_elegir().map(|v| v.len()),
            Some(nuevas.len()),
            "quedan elegidas"
        );
        assert_eq!(r.ids.len(), nuevas.len());
        // Un Ctrl+Z la devuelve tal cual.
        escena.deshacer();
        assert_eq!(escena.visibles().count(), ids.len());
        assert!(
            ids.iter()
                .all(|id| escena.buscar(*id).is_some_and(|e| !e.borrado))
        );
    }

    /// 25-oct-2026: el mensual en el lienzo, sin elegir, y «quita
    /// prioridades, objetivos a 6 lineas y a todo el ancho, tareas a 4»: se
    /// rehace en su sitio con sus secciones, y Ctrl+Z lo devuelve.
    #[test]
    fn el_mensual_se_personaliza_en_su_sitio() {
        use pixpin_motor2d::figuras_de_serie::{AnchoSeccion, SeccionPedida};
        let mut escena = Escena::nueva();
        let f = serie::catalogo()
            .into_iter()
            .find(|f| f.id == "serie-planificador-mensual")
            .unwrap();
        let ids = biblioteca::estampar_en_escena(
            &mut escena,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(200.0, 100.0),
        );
        let caja_vieja = caja(&escena.visibles().cloned().collect::<Vec<_>>()).unwrap();
        // Primero se leen sus secciones; el resumen ya trae su guia.
        let t = secciones_de_la_plantilla(&escena, &[]).unwrap();
        assert!(t.contains("Prioridades"), "{t}");
        assert!(
            guias_del_lienzo(&escena).contains("Calendario")
                || guias_del_lienzo(&escena).contains("calendario")
        );
        assert!(guias_del_lienzo(&Escena::nueva()).is_empty());
        let iv = Intervalo {
            secciones: Some(vec![
                SeccionPedida {
                    titulo: "Objetivo del mes".into(),
                    lineas: Some(6),
                    ancho: Some(AnchoSeccion::Completo),
                    ..Default::default()
                },
                SeccionPedida {
                    titulo: "Tareas".into(),
                    lineas: Some(4),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };
        let r = poner_plantilla(&mut escena, &[], None, &iv, None).unwrap();
        assert!(r.texto.contains("secciones pedidas"), "{}", r.texto);
        let nuevas: Vec<Elemento> = escena.visibles().cloned().collect();
        let textos: Vec<&str> = nuevas
            .iter()
            .filter_map(|e| match &e.figura {
                pixpin_motor2d::elemento::Figura::Texto { texto, .. } => Some(texto.as_str()),
                _ => None,
            })
            .collect();
        assert!(!textos.contains(&"PRIORIDADES") && textos.contains(&"TAREAS"));
        // La guia dice lo que tiene AHORA (sin Prioridades), no solo lo de serie.
        let t = secciones_de_la_plantilla(&escena, &[]).unwrap();
        let ahora = &t[t.find("AHORA").expect("dice lo de ahora")..];
        assert!(
            ahora.contains("«Tareas»") && !ahora.contains("«Prioridades»"),
            "{ahora}"
        );
        let c = caja(&nuevas).unwrap();
        assert!((c.0 - caja_vieja.0).abs() < 0.5 && (c.1 - caja_vieja.1).abs() < 0.5);
        escena.deshacer();
        assert!(
            ids.iter()
                .all(|id| escena.buscar(*id).is_some_and(|e| !e.borrado))
        );
        // Caso negativo: un lienzo vacio no tiene plantilla que leer.
        assert!(secciones_de_la_plantilla(&Escena::nueva(), &[]).is_err());
    }

    /// 24-oct-2026 (banco con Haiku): «lo de los habitos no lo uso, pon un
    /// espacio en blanco grande». Sus «01–04» son las prioridades, no filas
    /// que alargar: personalizar el semanal no debe pedir numeros.
    #[test]
    fn el_semanal_cambia_habitos_por_un_espacio_libre() {
        use pixpin_motor2d::figuras_de_serie::{SeccionPedida, TipoSeccion};
        let mut escena = Escena::nueva();
        let f = serie::catalogo()
            .into_iter()
            .find(|f| f.id == "serie-planificador-semanal")
            .unwrap();
        biblioteca::estampar_en_escena(
            &mut escena,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(300.0, 400.0),
        );
        let titulo = |t: &str| SeccionPedida {
            titulo: t.into(),
            ..Default::default()
        };
        let iv = Intervalo {
            secciones: Some(vec![
                titulo("Objetivo de la semana"),
                titulo("Prioridades"),
                titulo("Otras tareas"),
                SeccionPedida {
                    titulo: "Espacio libre".into(),
                    tipo: Some(TipoSeccion::Libre),
                    lineas: Some(8),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };
        let r = poner_plantilla(&mut escena, &[], None, &iv, None).unwrap();
        let textos: Vec<String> = escena
            .visibles()
            .filter_map(|e| match &e.figura {
                pixpin_motor2d::elemento::Figura::Texto { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect();
        assert!(
            textos.iter().any(|t| t == "ESPACIO LIBRE") && !textos.iter().any(|t| t == "HÁBITOS"),
            "{}: {textos:?}",
            r.texto
        );
        // Caso negativo: con numeros de verdad (el cronograma) si se conservan.
        assert_eq!(
            serie::cuenta_de("cronograma"),
            Some("filas"),
            "el cronograma sigue contando sus filas"
        );
        assert_eq!(serie::cuenta_de("planificador-semanal"), None);
    }

    /// 25-oct-2026: el cronograma abierto, SIN elegirlo, y «que sean 30
    /// semanas y 30 actividades»: se rehace en su sitio con su diseño.
    #[test]
    fn el_cronograma_sin_elegir_se_agranda_en_su_sitio() {
        let mut escena = Escena::nueva();
        let f = serie::catalogo()
            .into_iter()
            .find(|f| f.id == "serie-cronograma")
            .unwrap();
        let ids = biblioteca::estampar_en_escena(
            &mut escena,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(500.0, 600.0),
        );
        let caja_vieja = caja(&escena.visibles().cloned().collect::<Vec<_>>()).unwrap();
        let iv = Intervalo {
            desde: Some(1),
            hasta: Some(30),
            semanas: Some((1, 30)),
            ..Default::default()
        };
        let r = poner_plantilla(&mut escena, &[], Some("cronograma"), &iv, None).unwrap();
        assert!(r.texto.contains("una hoja"), "{}", r.texto);
        assert!(
            ids.iter()
                .all(|id| escena.buscar(*id).is_some_and(|e| e.borrado)),
            "la vieja se va: no quedan dos cronogramas"
        );
        let nuevas: Vec<Elemento> = escena.visibles().cloned().collect();
        let c = caja(&nuevas).unwrap();
        assert!(
            (c.0 - caja_vieja.0).abs() < 0.5 && (c.1 - caja_vieja.1).abs() < 0.5,
            "empieza donde estaba"
        );
        assert!(
            c.2 - c.0 > caja_vieja.2 - caja_vieja.0 && c.3 - c.1 > caja_vieja.3 - caja_vieja.1,
            "mas ancha (semanas) y mas alta (actividades)"
        );
        // Caso negativo: con dos cronogramas en el lienzo no se adivina.
        let mut dos = Escena::nueva();
        for x in [0.0, 3000.0] {
            biblioteca::estampar_en_escena(
                &mut dos,
                &f,
                pixpin_motor2d::vector::Punto2::nuevo(x, 0.0),
            );
        }
        // «Redúcelo a 10 semanas» deja las 30 actividades que tenia.
        let mut e2 = Escena::nueva();
        biblioteca::estampar_en_escena(
            &mut e2,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(0.0, 0.0),
        );
        let iv30 = Intervalo {
            desde: Some(1),
            hasta: Some(30),
            semanas: Some((1, 30)),
            ..Default::default()
        };
        poner_plantilla(&mut e2, &[], Some("cronograma"), &iv30, None).unwrap();
        let ancho_30 = caja(&e2.visibles().cloned().collect::<Vec<_>>())
            .map(|c| c.2 - c.0)
            .unwrap();
        let solo_semanas = Intervalo {
            semanas: Some((1, 10)),
            ..Default::default()
        };
        poner_plantilla(&mut e2, &[], None, &solo_semanas, None).unwrap();
        let vivos: Vec<Elemento> = e2.visibles().cloned().collect();
        let actual = tramo_actual(&vivos);
        assert_eq!(actual.filas, Some((1, 30)), "las actividades se quedan");
        assert_eq!(actual.semanas, Some((1, 10)));
        assert!(!actual.noche);
        let ancho_10 = caja(&vivos).map(|c| c.2 - c.0).unwrap();
        assert!(
            ancho_10 < ancho_30,
            "la hoja se reajusta, no solo borra columnas"
        );
        assert!(plantilla_en_la_escena(&dos, Some("cronograma"), false).is_none());
        // Desagrupada: se reconoce igual, por su titulo y su hoja.
        let mut suelta = Escena::nueva();
        let ids = biblioteca::estampar_en_escena(
            &mut suelta,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(0.0, 0.0),
        );
        for id in &ids {
            if let Some(x) = suelta.buscar_mut(*id) {
                x.grupos.clear();
            }
        }
        let (c, piezas) = plantilla_en_la_escena(&suelta, None, false).expect("sin grupo tambien");
        assert_eq!(c, "cronograma");
        assert_eq!(piezas.len(), ids.len(), "todas sus piezas y nada mas");
    }

    #[test]
    fn por_su_nombre_va_en_medio_y_sin_nada_elegido_se_dice() {
        let mut escena = Escena::nueva();
        let iv = Intervalo {
            desde: Some(7),
            hasta: Some(18),
            ..Default::default()
        };
        let r = poner_plantilla(
            &mut escena,
            &[],
            Some("storyboard de 6 viñetas"),
            &iv,
            Some((1000.0, 1000.0)),
        )
        .unwrap();
        let c = caja(&escena.visibles().cloned().collect::<Vec<_>>()).unwrap();
        assert!(((c.0 + c.2) / 2.0 - 1000.0).abs() < 1.0, "{c:?}");
        assert!(!r.texto.contains("Ctrl+Z"));
        let _ = tomar_para_elegir();
        let e = poner_plantilla(&mut Escena::nueva(), &[], None, &iv, None).unwrap_err();
        assert!(e.contains("elige"), "{e}");
        let (mut escena, ids) = con_la_lista();
        let e = poner_plantilla(&mut escena, &ids, Some("curso"), &iv, None).unwrap_err();
        assert!(e.contains("no lleva números"), "{e}");
        assert!(
            tomar_para_elegir().is_none(),
            "cerrado no deja nada que elegir"
        );
    }

    /// 25-oct-2026: «ponlo en modo noche» la rehace en su sitio en oscuro;
    /// alargarla despues sigue de noche, el mes no se pierde y «de día» la
    /// devuelve.
    #[test]
    fn el_modo_noche_se_pide_y_se_queda() {
        let vivos = |e: &Escena| e.visibles().cloned().collect::<Vec<_>>();
        let solo = |noche| Intervalo {
            noche: Some(noche),
            ..Default::default()
        };
        // El cronograma de 30 actividades, en el lienzo sin elegir.
        let mut escena = Escena::nueva();
        let iv30 = Intervalo {
            desde: Some(1),
            hasta: Some(30),
            ..Default::default()
        };
        poner_plantilla(&mut escena, &[], Some("cronograma"), &iv30, None).unwrap();
        let antes = caja(&vivos(&escena)).unwrap();
        let r = poner_plantilla(&mut escena, &[], None, &solo(true), None).unwrap();
        assert!(r.texto.contains("modo noche"), "{}", r.texto);
        let ahora = tramo_actual(&vivos(&escena));
        assert!(ahora.noche);
        assert_eq!(ahora.filas, Some((1, 30)), "no pierde filas");
        let despues = caja(&vivos(&escena)).unwrap();
        assert!(
            [
                despues.0 - antes.0,
                despues.1 - antes.1,
                despues.2 - antes.2,
                despues.3 - antes.3
            ]
            .iter()
            .all(|d| d.abs() < 0.5),
            "en su sitio, igual de grande: {antes:?} {despues:?}"
        );
        // Alargarla sin decir el modo: sigue de noche.
        let iv40 = Intervalo {
            hasta: Some(40),
            ..Default::default()
        };
        poner_plantilla(&mut escena, &[], None, &iv40, None).unwrap();
        let ahora = tramo_actual(&vivos(&escena));
        assert!(ahora.noche && ahora.filas == Some((1, 40)), "{ahora:?}");
        // De dia otra vez.
        poner_plantilla(&mut escena, &[], None, &solo(false), None).unwrap();
        assert!(!tramo_actual(&vivos(&escena)).noche);
        // El mensual de noviembre: de noche sigue siendo noviembre.
        let mut m = Escena::nueva();
        let nov = Intervalo {
            mes: Some((2026, 11)),
            ..Default::default()
        };
        poner_plantilla(&mut m, &[], Some("planificador mensual"), &nov, None).unwrap();
        poner_plantilla(&mut m, &[], None, &solo(true), None).unwrap();
        let ahora = tramo_actual(&vivos(&m));
        assert!(ahora.noche);
        assert_eq!(ahora.mes, Some((2026, 11)));
        // Una que no se alarga ni se personaliza (la gratitud) tambien.
        let mut g = Escena::nueva();
        poner_plantilla(&mut g, &[], Some("gratitud"), &solo(false), None).unwrap();
        poner_plantilla(&mut g, &[], None, &solo(true), None).unwrap();
        let v = vivos(&g);
        assert!(serie::es_de_noche(&v));
        assert_eq!(serie::cual_es(&v).as_deref(), Some("gratitud"));
        // Pedida por su nombre de noche, sale de noche.
        let mut n = Escena::nueva();
        poner_plantilla(&mut n, &[], Some("cronograma (noche)"), &iv30, None).unwrap();
        assert!(tramo_actual(&vivos(&n)).noche);
        // Caso negativo: sin plantilla en el lienzo, el modo solo no adivina.
        let e = poner_plantilla(&mut Escena::nueva(), &[], None, &solo(true), None).unwrap_err();
        assert!(e.contains("elige"), "{e}");
        let _ = tomar_para_elegir();
    }
}

/// Lo que miden unas hojas puestas en fila.
fn medida_de_las_hojas(hojas: &[FiguraGuardada]) -> (f32, f32) {
    let ancho = hojas.iter().map(|f| f.tamano().0).sum::<f32>()
        + ENTRE_HOJAS * hojas.len().saturating_sub(1) as f32;
    (
        ancho,
        hojas.iter().map(|f| f.tamano().1).fold(0.0, f32::max),
    )
}

/// **Unas hojas en fila** desde `(x0, y0)`, cada una su grupo, en el paso
/// de deshacer que este abierto. Devuelve los ids.
fn colocar_hojas(escena: &mut Escena, hojas: &[FiguraGuardada], (x0, y0): (f32, f32)) -> Vec<u64> {
    let mut ids = Vec::new();
    let mut x = x0;
    for f in hojas {
        let (w, h) = f.tamano();
        let primero = escena.siguiente_id.max(1);
        let grupo = format!("g{primero}");
        let centro = pixpin_motor2d::vector::Punto2::nuevo(x + w / 2.0, y0 + h / 2.0);
        for e in pixpin_motor2d::biblioteca::estampar(f, centro, primero, &grupo) {
            ids.push(escena.anadir(e));
        }
        x += w + ENTRE_HOJAS;
    }
    ids
}

/// Las hojas de una plantilla alargada en medio de `centro` (la ventana de
/// las figuras), en un solo paso de deshacer.
pub fn colocar_hojas_en_medio(
    escena: &mut Escena,
    hojas: &[FiguraGuardada],
    centro: (f32, f32),
) -> Vec<u64> {
    let (ancho, alto) = medida_de_las_hojas(hojas);
    escena.abrir_paso();
    let ids = colocar_hojas(
        escena,
        hojas,
        (centro.0 - ancho / 2.0, centro.1 - alto / 2.0),
    );
    escena.cerrar_paso();
    ids
}

#[cfg(test)]
mod pruebas_revision {
    use super::*;
    use pixpin_motor2d::biblioteca;
    use pixpin_motor2d::figuras_de_serie as serie;

    fn trabajo(escena: &mut Escena, t: Trabajo) -> Respuesta {
        let mut imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(1024);
        trabajar(escena, &t, &[], (0.0, 0.0), &mut imagenes).unwrap()
    }

    /// La revision guarda una copia, la correccion la estropea y se vuelve a
    /// la copia en un solo paso que el Ctrl+Z deshace.
    #[test]
    fn la_copia_vuelve_tal_cual_y_se_deshace_de_una() {
        let mut escena = Escena::nueva();
        let f = serie::catalogo()
            .into_iter()
            .find(|f| f.id == "serie-lista-de-tareas")
            .unwrap();
        let ids = biblioteca::estampar_en_escena(
            &mut escena,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(500.0, 600.0),
        );
        let antes: Vec<_> = escena.visibles().map(|e| (e.id, e.caja())).collect();
        let copia = trabajo(&mut escena, Trabajo::Copia)
            .copia
            .expect("la copia");
        // La «correccion» mueve una cosa y borra otra.
        escena.abrir_paso();
        escena.mover(ids[0], 40.0, 0.0);
        escena.borrar_apuntando(ids[1]);
        escena.cerrar_paso();
        assert_ne!(
            escena
                .visibles()
                .map(|e| (e.id, e.caja()))
                .collect::<Vec<_>>(),
            antes
        );
        let pasos = escena.pasos_cerrados();
        let r = trabajo(
            &mut escena,
            Trabajo::VolverA {
                copia: copia.clone(),
            },
        );
        assert!(r.texto.starts_with("vuelto"), "{}", r.texto);
        assert_eq!(
            escena
                .visibles()
                .map(|e| (e.id, e.caja()))
                .collect::<Vec<_>>(),
            antes
        );
        assert_eq!(escena.pasos_cerrados(), pasos + 1, "volver es un paso");
        // Otra vez: sigue igual (puede apuntar un paso, por las versiones).
        trabajo(&mut escena, Trabajo::VolverA { copia });
        assert_eq!(
            escena
                .visibles()
                .map(|e| (e.id, e.caja()))
                .collect::<Vec<_>>(),
            antes
        );
        // Y comprobar contesta con una linea por fallo (o ninguna).
        let r = trabajo(&mut escena, Trabajo::Comprobar);
        assert!(r.texto.lines().count() <= pixpin_ia::lint::MAX_LINEAS.max(64));
    }
}
