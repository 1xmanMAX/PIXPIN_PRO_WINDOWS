//! **Buscar dentro de todos los chats** desde el buscador de la lista
//! (25-oct-2026, el usuario: «que el buscador busque tambien dentro de
//! todos los chats»). Como en Telegram: debajo de los chats que casan por
//! su nombre, la seccion «Mensajes» con los mensajes de cualquier chat que
//! llevan lo escrito, del mas nuevo al mas viejo; pulsar uno abre su chat
//! llevado a el y resaltado, igual que «Ver en el chat».
//!
//! Lo puro (aplanar, buscar, el trozo, la espera, la seccion) esta en
//! `pixpin_ui::buscar_en_chats`. Aqui, el hilo que lee los cuadernos y
//! busca, y como se pinta y se pulsa la seccion.
//!
//! **El indice vive en su hilo**, fuera del de la ventana: leer cien
//! cuadernos y aplanar sus mensajes no puede congelar la lista. Se hace la
//! primera vez que se busca y despues solo se relee el cuaderno que cambio
//! (por su fecha y tamano): sincronizar o escribir en un chat no obliga a
//! leerlos todos otra vez. Tambien busca alli, y la ventana solo pinta lo
//! que le llega, ya cortado en trozos.
//!
//! **Las pestanas por tipo** (27-oct-2026, como PixPin Android 8815b492):
//! mientras se busca, arriba de la seccion van Todo, Fotos, Videos,
//! Archivos, Enlaces, Voz y Dibujos con su cuenta. Con una puesta, los chats
//! no salen (lo que se mira son fotos, archivos…) y sin escribir nada salen
//! todos los de esa clase de todos los chats. Cada fila lleva delante su
//! miniatura o el icono de su tipo y arriba «en <chat>» con la foto del
//! chat. Pulsarla abre la cosa en su visor, como al tocar su burbuja
//! ([`b::al_pulsar`]); la flechita de la derecha o el clic derecho llevan
//! al mensaje, y si esta dentro de una carpeta, a la tarjeta de la carpeta
//! ([`adonde_saltar`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::SystemTime;

use pixpin_proyecto::carpeta_de_paginas::{self as carpeta, Carpeta};
use pixpin_proyecto::cuaderno::{self, Clase, Mensaje};
use pixpin_render::icono::{Icono, material as mi};
use pixpin_render::{Color, MotorRender, Pintor, RectF};
use pixpin_store::Catalogo;
use pixpin_ui::buscar_en_chats::{self as b, Elemento, Pestana, Seccion};
use pixpin_ui::chat::{self, Disposicion};

use super::{Abierto, Lista, Tema, color_avatar, icono_de_tipo, iniciales, logo, rf};

/// Cuantos caracteres del mensaje se ensenan antes de lo encontrado, y
/// cuantos como mucho en la linea.
const ANTES: usize = 22;
const LARGO: usize = 160;

/// Lo que se le pide al hilo.
struct Pedido {
    aguja: String,
    pestana: Pestana,
    tope: usize,
    turno: u64,
}

/// Lo que se pinta delante de un resultado: su miniatura o el icono de su
/// tipo. Una nota lleva el avatar de su chat, como antes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Muestra {
    Foto,
    Video,
    Archivo,
    Voz,
    Dibujo,
    Pagina,
    Carpeta,
    Proyecto,
    MiniApp,
    Enlace,
    Nota,
}

/// Un mensaje encontrado, listo para pintar.
#[derive(Debug, Clone)]
pub(super) struct Encontrado {
    /// El `id` de la ficha de su chat.
    pub chat: String,
    /// Su codigo unico: por el se le encuentra al abrir el chat.
    pub codigo: String,
    pub cuando: i64,
    pub fragmento: b::Fragmento,
    pub muestra: Muestra,
    /// El nombre de la cosa (el del fichero, el del lienzo): lo que se
    /// ensena si no hay trozo.
    pub nombre: String,
    /// La foto, si esta en este equipo: su miniatura va delante.
    pub foto: Option<PathBuf>,
    /// Que hace pulsarla (ver `b::al_pulsar`).
    pub al_pulsar: b::AlPulsar,
}

/// Lo que hace un clic en la seccion, para el bucle de la ventana.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pulsado {
    /// Se cambio de pestana: los chats de la lista se vuelven a filtrar.
    Pestana,
    /// Se pidio ir a un mensaje (o abrirlo): ya esta pedido.
    Mensaje,
    /// La cabecera, «Ver más», «Nada»: nada que hacer en el bucle.
    Otro,
}

/// Lo que contesta el hilo a un pedido.
#[derive(Debug, Clone, Default)]
pub(super) struct Resultado {
    turno: u64,
    pub encontrados: Vec<Encontrado>,
    pub total: usize,
    /// Cuantos casan en cada pestana (por `Pestana::indice`).
    pub por_pestana: [usize; Pestana::TODAS.len()],
}

/// **El buscador en todos los chats** de la ventana: lo escrito, la pestana,
/// la espera y lo que llego del hilo.
pub(super) struct BuscarEnChats {
    raiz: PathBuf,
    /// La ventana a despertar cuando el hilo conteste.
    hwnd: isize,
    /// Lo escrito la ultima vez que se miro (sin espacios de los lados).
    aguja: String,
    /// La pestana por tipo puesta.
    pestana: Pestana,
    /// Si se ven las pestanas: con el buscador con el teclado, con algo
    /// escrito o con una pestana puesta.
    activa: bool,
    /// El ancho de la lista y la escala de la ultima vuelta: con ellos se
    /// reparten las pestanas en lineas.
    ancho: u32,
    escala: u32,
    /// El nombre de cada pestana, en el idioma de la ventana.
    nombres: [String; Pestana::TODAS.len()],
    /// Las cuentas de la ultima busqueda con lo mismo escrito: cambiar de
    /// pestana no las borra mientras llega la nueva.
    cuentas: Option<[usize; Pestana::TODAS.len()]>,
    retardo: b::Retardo,
    /// Cuantos se piden: crece con «Ver más».
    tope: usize,
    /// El del ultimo pedido: lo que conteste el hilo a uno anterior se tira.
    turno: u64,
    /// El hilo, nacido con la primera busqueda.
    hilo: Option<(Sender<Pedido>, Receiver<Resultado>)>,
    resultado: Option<Resultado>,
    /// Las miniaturas de las fotos encontradas.
    miniaturas: crate::miniaturas::Miniaturas,
    /// Lo que hay que abrir al llegar a su chat: `(chat, codigo)`.
    abrir_al_llegar: Option<(String, String)>,
}

impl BuscarEnChats {
    pub fn nuevo(raiz: &Path, hwnd: isize, textos: &Catalogo) -> BuscarEnChats {
        BuscarEnChats {
            raiz: raiz.to_path_buf(),
            hwnd,
            aguja: String::new(),
            pestana: Pestana::Todo,
            activa: false,
            ancho: 0,
            escala: 100,
            nombres: Pestana::TODAS.map(|p| textos.t(p.clave())),
            cuentas: None,
            retardo: b::Retardo::default(),
            tope: b::DE_PRIMERAS,
            turno: 0,
            hilo: None,
            resultado: None,
            miniaturas: crate::miniaturas::Miniaturas::nuevo(),
            abrir_al_llegar: None,
        }
    }

    /// Si hay algo que buscar: lo escrito, o una pestana puesta.
    fn buscable(&self) -> bool {
        !self.aguja.is_empty() || self.pestana != Pestana::Todo
    }

    /// Lo que habia se va ya, sin esperar: y lo que este por llegar del
    /// hilo ya no vale. Devuelve si habia algo.
    fn olvidar(&mut self) -> bool {
        self.turno += 1;
        self.retardo = b::Retardo::default();
        self.resultado.take().is_some()
    }

    /// **Una vuelta del bucle**: si cambio lo escrito, se rearma la espera;
    /// si la espera acabo, se le pide al hilo; si el hilo contesto, se
    /// guarda. `buscando` es si el buscador tiene el teclado, y `ancho` el
    /// de la lista. Devuelve si hay que volver a pintar.
    pub fn latido(
        &mut self,
        escrito: &str,
        buscando: bool,
        ancho: u32,
        escala: u32,
        ahora_ms: u64,
    ) -> bool {
        let mut cambio = (ancho, escala) != (self.ancho, self.escala);
        (self.ancho, self.escala) = (ancho, escala);
        let aguja = escrito.trim();
        if aguja != self.aguja {
            self.aguja = aguja.to_string();
            self.tope = b::DE_PRIMERAS;
            self.cuentas = None;
            if self.buscable() {
                self.retardo.cambio(ahora_ms);
            } else {
                cambio |= self.olvidar();
            }
        }
        let activa = buscando || self.buscable();
        if activa != self.activa {
            self.activa = activa;
            cambio = true;
        }
        if self.retardo.toca(ahora_ms) {
            self.pedir();
        }
        if let Some((_, llegan)) = &self.hilo {
            while let Ok(r) = llegan.try_recv() {
                if r.turno == self.turno && self.buscable() {
                    self.cuentas = Some(r.por_pestana);
                    self.resultado = Some(r);
                    cambio = true;
                }
            }
        }
        cambio
    }

    /// Cuando hay que volver a mirar: al acabar la espera. Lo que conteste
    /// el hilo despierta la ventana por su cuenta.
    pub fn ms_hasta(&self, ahora_ms: u64) -> Option<u32> {
        self.retardo.ms_hasta(ahora_ms)
    }

    /// Algo cambio en los chats (sincronizar, el indice): con una busqueda
    /// puesta se repite, y el hilo relee solo lo que cambio.
    pub fn releer(&mut self) {
        if self.buscable() && self.resultado.is_some() {
            self.pedir();
        }
    }

    /// «Ver más»: los siguientes.
    pub fn ver_mas(&mut self) {
        self.tope += b::DE_PRIMERAS;
        self.pedir();
    }

    /// **Pone la pestana `p`**; pulsar la que ya esta vuelve a «Todo». Se
    /// busca enseguida, sin la espera de las teclas.
    fn poner_pestana(&mut self, p: Pestana) {
        let p = if p == self.pestana { Pestana::Todo } else { p };
        self.pestana = p;
        self.tope = b::DE_PRIMERAS;
        self.olvidar();
        self.pedir();
    }

    /// Escape en el buscador: la pestana vuelve a «Todo» (lo escrito lo
    /// vacia quien llama).
    pub fn cerrar(&mut self) {
        self.pestana = Pestana::Todo;
        self.olvidar();
    }

    /// **Los chats de la lista** que casan con lo escrito, como siempre
    /// (`super::filtrar`); con una pestana puesta, ninguno: lo que se mira
    /// son fotos, archivos…
    pub fn filtrar(&self, fichas: &[pixpin_proyecto::almacen::Ficha], escrito: &str) -> Vec<usize> {
        if self.pestana != Pestana::Todo {
            return Vec::new();
        }
        super::filtrar(fichas, escrito)
    }

    fn pedir(&mut self) {
        if !self.buscable() {
            return;
        }
        if self.hilo.is_none() {
            self.hilo = lanzar_hilo(self.raiz.clone(), self.hwnd);
        }
        self.turno += 1;
        let pedido = Pedido {
            aguja: self.aguja.clone(),
            pestana: self.pestana,
            tope: self.tope,
            turno: self.turno,
        };
        if let Some((pedir, _)) = &self.hilo
            && pedir.send(pedido).is_err()
        {
            // El hilo murio (no deberia): la proxima vez se lanza otro.
            self.hilo = None;
        }
    }

    /// Lo encontrado que se ensena, si hay.
    pub fn resultado(&self) -> Option<&Resultado> {
        self.resultado
            .as_ref()
            .filter(|r| !r.encontrados.is_empty())
    }

    /// El rotulo de cada pestana, con su cuenta si se sabe.
    fn rotulos(&self) -> Vec<String> {
        Pestana::TODAS
            .iter()
            .map(|p| {
                let cuenta = self.cuentas.map(|c| c[p.indice()]);
                b::rotulo_de_pestana(&self.nombres[p.indice()], cuenta)
            })
            .collect()
    }

    /// Las cajas de las pestanas, desde la esquina de su banda, y cuantas
    /// lineas ocupan. Vacias si no se ven.
    fn pestanas(&self) -> (Vec<pixpin_geom::Rect>, u32) {
        if !self.activa || self.ancho == 0 {
            return (Vec::new(), 0);
        }
        let anchos: Vec<u32> = self
            .rotulos()
            .iter()
            .map(|r| b::ancho_de_pestana(r, self.escala))
            .collect();
        b::repartir_pestanas(&anchos, self.ancho, self.escala)
    }

    /// Como es la seccion en la lista ahora.
    pub fn seccion(&self) -> Seccion {
        let lineas_de_pestanas = self.pestanas().1;
        match self.resultado() {
            Some(r) => Seccion {
                lineas_de_pestanas,
                mensajes: r.encontrados.len(),
                ver_mas: r.total > r.encontrados.len(),
                nada: false,
            },
            None => Seccion {
                lineas_de_pestanas,
                // Se busco (llego la respuesta) y no hay nada.
                nada: self.buscable() && self.resultado.is_some(),
                ..Default::default()
            },
        }
    }

    /// El encontrado `n` de los que se ensenan.
    pub fn encontrado(&self, n: usize) -> Option<&Encontrado> {
        self.resultado()?.encontrados.get(n)
    }

    /// Que hay de la seccion bajo el punto `l` de la ventana, con la lista
    /// bajada `scroll` y `chats` filas de chats por encima.
    pub fn elemento_en(
        &self,
        d: &Disposicion,
        l: pixpin_geom::Punto,
        scroll: i32,
        chats: usize,
        escala: u32,
    ) -> Option<Elemento> {
        if !d.filas.contiene(l) || d.plegada {
            return None;
        }
        let y = l.y - inicio(d, scroll, chats, escala);
        self.seccion().elemento_en(y, escala)
    }

    /// **Un clic en la seccion** (`derecho`, con el boton derecho). En una
    /// pestana, la pone. En un resultado, lo abre en su visor si lo tiene
    /// (`b::al_pulsar`); en su flechita, o con el derecho, lleva al mensaje.
    /// `None` si el clic no cae en la seccion.
    #[allow(clippy::too_many_arguments)] // el contexto del clic, que va entero
    pub fn pulsar(
        &mut self,
        d: &Disposicion,
        l: pixpin_geom::Punto,
        scroll: i32,
        chats: usize,
        escala: u32,
        derecho: bool,
        abierto: &mut Option<Abierto>,
    ) -> Option<Pulsado> {
        let el = self.elemento_en(d, l, scroll, chats, escala)?;
        match el {
            Elemento::Pestanas if !derecho => {
                let y0 = inicio(d, scroll, chats, escala);
                let (cajas, _) = self.pestanas();
                let i = b::pestana_en(&cajas, l.x - d.filas.x, l.y - y0)?;
                self.poner_pestana(Pestana::TODAS[i]);
                Some(Pulsado::Pestana)
            }
            Elemento::Mensaje(n) => {
                let fila = caja_de(d, scroll, chats, escala, &self.seccion(), el);
                let enc = self.encontrado(n)?.clone();
                let a_la_flecha = derecho || b::flecha(fila, escala).contiene(l);
                self.abrir_al_llegar = (!a_la_flecha && enc.al_pulsar == b::AlPulsar::Abrir)
                    .then(|| (enc.chat.clone(), enc.codigo.clone()));
                ir_al_mensaje(abierto, &enc);
                Some(Pulsado::Mensaje)
            }
            Elemento::VerMas if !derecho => {
                self.ver_mas();
                Some(Pulsado::Otro)
            }
            _ => Some(Pulsado::Otro),
        }
    }

    /// **Lo que habia que abrir al llegar**, ya en su chat (despues de
    /// `tomar_ir_a`): su sitio, para tocar su burbuja como si se pulsara
    /// alli, que es lo que abre cada cosa en su visor. Una nota con enlace
    /// abre aqui mismo el enlace, que su burbuja no abre al tocarla.
    pub fn tomar_abrir(&mut self, abierto: Option<&Abierto>, textos: &Catalogo) -> Option<usize> {
        let (chat, codigo) = self.abrir_al_llegar.take()?;
        let a = abierto.filter(|a| a.ficha.id == chat)?;
        let j = super::indice_de_codigo(&a.mensajes, &codigo)?;
        if matches!(a.mensajes[j].clase, Some(Clase::Nota) | None) {
            let _ = super::abrir_enlace(a, j, textos);
            return None;
        }
        Some(j)
    }

    /// Los chats de lo encontrado, para tener sus logos listos al pintar.
    pub fn chats(&self) -> impl Iterator<Item = &str> {
        self.resultado()
            .into_iter()
            .flat_map(|r| r.encontrados.iter().map(|e| e.chat.as_str()))
    }

    /// Lee y sube las miniaturas de las fotos encontradas, ANTES de pintar.
    /// Devuelve si faltan (hay que pedir otro fotograma).
    pub fn preparar(&mut self, motor: &MotorRender) -> bool {
        let fotos: Vec<PathBuf> = self
            .resultado()
            .into_iter()
            .flat_map(|r| r.encontrados.iter().filter_map(|e| e.foto.clone()))
            .collect();
        !fotos.is_empty() && self.miniaturas.asegurar(&fotos, motor)
    }

    /// El dispositivo de dibujo se perdio: los bitmaps ya no valen.
    pub fn soltar(&mut self) {
        self.miniaturas.soltar();
    }
}

/// **Pide ir a un mensaje encontrado**: su chat, llevado a el y resaltado,
/// por el mismo camino que «Ver en el chat» (`ir_a`). Si es el chat abierto
/// y tiene su propia lupa o un chip puestos, se quitan: el mensaje podria
/// quedar escondido y no habria adonde ir.
fn ir_al_mensaje(abierto: &mut Option<Abierto>, enc: &Encontrado) {
    if let Some(a) = abierto.as_mut().filter(|a| a.ficha.id == enc.chat) {
        a.busqueda = None;
        a.por_etiqueta = None;
        a.colocado.borrow_mut().ancho = 0;
    }
    super::pedir_ir_a_mensaje(enc.chat.clone(), enc.codigo.clone());
}

/// **A que mensaje se salta para llegar al `j`**: a el si se ve suelto en
/// el chat; si esta dentro de una carpeta (que no lo ensena suelto, lo
/// representa su tarjeta), a la tarjeta de esa carpeta, que es donde esta a
/// la vista. Es `adondeSaltar` de PixPin Android.
pub(super) fn adonde_saltar(mensajes: &[Mensaje], j: usize) -> usize {
    let Some(id) = mensajes.get(j).map(|m| m.id.as_str()) else {
        return j;
    };
    mensajes
        .iter()
        .position(|c| {
            carpeta::es_carpeta(c) && Carpeta::de_mensaje(c).is_some_and(|k| k.contiene(id))
        })
        .unwrap_or(j)
}

/// La caja de un elemento de la seccion en la ventana.
fn caja_de(
    d: &Disposicion,
    scroll: i32,
    chats: usize,
    escala: u32,
    seccion: &Seccion,
    el: Elemento,
) -> pixpin_geom::Rect {
    let (y, alto) = seccion.caja(el, escala);
    pixpin_geom::Rect {
        x: d.filas.x,
        y: inicio(d, scroll, chats, escala) + y,
        ancho: d.filas.ancho,
        alto,
    }
}

/// Donde empieza la seccion: debajo de la ultima fila de chats.
fn inicio(d: &Disposicion, scroll: i32, chats: usize, escala: u32) -> i32 {
    d.filas.y + chats as i32 * (chat::FILA * escala / 100) as i32 - scroll
}

// --- El hilo ---------------------------------------------------------------

/// Lanza el hilo del indice. `None` si el sistema no da un hilo.
fn lanzar_hilo(raiz: PathBuf, hwnd: isize) -> Option<(Sender<Pedido>, Receiver<Resultado>)> {
    let (pedir, piden) = channel::<Pedido>();
    let (contestar, llegan) = channel::<Resultado>();
    let lanzado = std::thread::Builder::new()
        .name("buscar-en-chats".into())
        .spawn(move || {
            let mut indice = Indice::default();
            // Acaba cuando la ventana suelta su lado (se cerro el chat).
            while let Ok(mut p) = piden.recv() {
                // Escribiendo deprisa se acumulan: solo vale el ultimo.
                while let Ok(otro) = piden.try_recv() {
                    p = otro;
                }
                indice.poner_al_dia(&raiz);
                if contestar.send(indice.buscar(&p)).is_err() {
                    break;
                }
                pixpin_shell::overlay::despertar(hwnd);
            }
        });
    match lanzado {
        Ok(_) => Some((pedir, llegan)),
        Err(e) => {
            tracing::warn!(?e, "no se pudo lanzar el hilo de buscar en los chats");
            None
        }
    }
}

/// Por lo que se sabe si un cuaderno cambio: su fecha y su tamano.
type Firma = Option<(SystemTime, u64)>;

fn firma_de(carpeta: &Path) -> Firma {
    let m = std::fs::metadata(carpeta.join("guardados.jsonl")).ok()?;
    Some((m.modified().ok()?, m.len()))
}

/// Los mensajes de un chat, ya aplanados, y lo que hace falta de cada uno
/// para pintarlo y abrirlo (en `extras`, a la par de `entradas`).
struct Chat {
    id: String,
    firma: Firma,
    entradas: Vec<b::Entrada>,
    extras: Vec<Extra>,
}

/// Lo de un mensaje que no se busca pero se ensena: que se pinta delante,
/// su nombre, donde esta su fichero y que hace pulsarlo.
#[derive(Debug, Clone)]
struct Extra {
    muestra: Muestra,
    nombre: String,
    /// Su fichero en este equipo, si tiene (puede no haber llegado aun).
    ruta: Option<PathBuf>,
    /// Que es, para `b::al_pulsar`; de un fichero se mira al ensenarlo si
    /// esta, que es mirar el disco solo para los que se ensenan.
    cosa: b::Cosa,
}

/// Lo que se busca de un mensaje: su texto (la nota rica sin las rutas de
/// sus fotos, que no son de nadie), su transcripcion y el nombre de su
/// fichero. El pie de una foto es su texto. Y en que pestanas sale.
pub(super) fn entrada_de(chat: &str, m: &Mensaje) -> b::Entrada {
    let texto = cuaderno::sin_rutas_de_fotos(&m.texto);
    b::Entrada::nueva(
        chat,
        &m.codigo_unico(),
        m.cuando,
        [
            texto.as_str(),
            m.transcripcion.as_deref().unwrap_or_default(),
            m.nombre.as_str(),
        ],
    )
    .con_tipos(tipos_de(m))
}

/// **En que pestanas sale un mensaje**, con la regla de la ficha de su chat
/// (`cuaderno::indices_de_seccion`): asi algo sale en «Archivos» aqui si y
/// solo si sale en la ficha de su chat. Lo del buzon, en ninguna.
pub(super) fn tipos_de(m: &Mensaje) -> b::Tipos {
    Pestana::TODAS
        .into_iter()
        .filter(|p| {
            seccion_de(*p).is_some_and(|s| {
                !cuaderno::indices_de_seccion(std::slice::from_ref(m), s).is_empty()
            })
        })
        .collect()
}

/// La seccion de la ficha del chat de cada pestana («Todo» no hace falta).
fn seccion_de(p: Pestana) -> Option<cuaderno::Seccion> {
    Some(match p {
        Pestana::Todo => return None,
        Pestana::Fotos => cuaderno::Seccion::Fotos,
        Pestana::Videos => cuaderno::Seccion::Videos,
        Pestana::Archivos => cuaderno::Seccion::Archivos,
        Pestana::Enlaces => cuaderno::Seccion::Enlaces,
        Pestana::Voz => cuaderno::Seccion::Voz,
        Pestana::Dibujos => cuaderno::Seccion::Dibujos,
    })
}

/// Lo que se ensena de un mensaje y que hace pulsarlo.
fn extra_de(raiz: &Path, chat: &str, m: &Mensaje) -> Extra {
    let ruta = m
        .ruta
        .as_deref()
        .and_then(|r| pixpin_proyecto::vista::ruta_real(raiz, chat, r));
    let fichero = b::Cosa::Fichero { esta: false };
    let con_referencia = m.referencia.as_deref().is_some_and(|r| !r.is_empty());
    let a_algo = if con_referencia {
        b::Cosa::ConVisor
    } else {
        b::Cosa::SinNada
    };
    let (muestra, cosa) = match &m.clase {
        Some(Clase::Imagen | Clase::Archivo) if cuaderno::es_video(m) => (Muestra::Video, fichero),
        Some(Clase::Imagen) => (Muestra::Foto, fichero),
        Some(Clase::Archivo) => (Muestra::Archivo, fichero),
        Some(Clase::Voz) => (Muestra::Voz, b::Cosa::Voz),
        Some(Clase::Dibujo) => (Muestra::Dibujo, a_algo),
        Some(Clase::Pagina) => (Muestra::Pagina, b::Cosa::ConVisor),
        Some(Clase::Proyecto) => (Muestra::Proyecto, a_algo),
        Some(Clase::MiniApp) if carpeta::es_carpeta(m) => (Muestra::Carpeta, b::Cosa::ConVisor),
        Some(Clase::MiniApp) => (Muestra::MiniApp, b::Cosa::ConVisor),
        // Una nota (o una clase que aun no se conoce): con enlace, se abre.
        _ => {
            let enlace = cuaderno::tiene_enlace(&m.texto);
            let muestra = if enlace {
                Muestra::Enlace
            } else {
                Muestra::Nota
            };
            (muestra, b::Cosa::Nota { enlace })
        }
    };
    Extra {
        muestra,
        nombre: m.nombre.trim().to_string(),
        ruta,
        cosa,
    }
}

/// Si el mensaje tiene burbuja en su chat (ver `se_ve`): lo que no la tiene
/// (las paginas de un PDF unido, las lecciones, lo del sistema) no se
/// encuentra, porque pulsarlo no llevaria a ninguna parte.
fn se_ve_en_el_chat(m: &Mensaje) -> bool {
    !crate::pdf_en_chat::solo_galeria(m)
        && !crate::lecciones::almacen::es_de_leccion(m)
        && !cuaderno::es_de_sistema(m)
}

fn leer_chat(raiz: &Path, id: &str, firma: Firma) -> Chat {
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, id);
    let (entradas, extras) = match cuaderno::Cuaderno::leer_de(&carpeta) {
        Ok(c) => c
            .mensajes
            .iter()
            .filter(|m| se_ve_en_el_chat(m))
            .map(|m| (entrada_de(id, m), extra_de(raiz, id, m)))
            .unzip(),
        // Sin cuaderno (chat nuevo, disco que no esta): nada que buscar.
        Err(_) => (Vec::new(), Vec::new()),
    };
    Chat {
        id: id.to_string(),
        firma,
        entradas,
        extras,
    }
}

/// **El indice**: los mensajes de todos los chats y en que orden se buscan
/// (lo mas nuevo primero), como pares (chat, mensaje).
#[derive(Default)]
struct Indice {
    chats: Vec<Chat>,
    orden: Vec<(u32, u32)>,
}

impl Indice {
    /// Relee los chats nuevos o cambiados y suelta los que ya no estan.
    fn poner_al_dia(&mut self, raiz: &Path) {
        let fichas = pixpin_proyecto::almacen::Indice::leer(raiz).proyectos;
        // El orden va por la posicion de cada chat: si un chat cambia de
        // sitio (otro se borro, llego uno nuevo), hay que rehacerlo.
        let antes: Vec<String> = self.chats.iter().map(|c| c.id.clone()).collect();
        let mut viejos: HashMap<String, Chat> = std::mem::take(&mut self.chats)
            .into_iter()
            .map(|c| (c.id.clone(), c))
            .collect();
        let mut cambio = false;
        for f in &fichas {
            let firma = firma_de(&pixpin_proyecto::almacen::carpeta(raiz, &f.id));
            let chat = match viejos.remove(&f.id) {
                Some(c) if c.firma == firma => c,
                _ => {
                    cambio = true;
                    leer_chat(raiz, &f.id, firma)
                }
            };
            self.chats.push(chat);
        }
        cambio |= self.chats.iter().map(|c| &c.id).ne(antes.iter());
        if cambio {
            self.ordenar();
        }
    }

    fn ordenar(&mut self) {
        let mut orden: Vec<(u32, u32)> = self
            .chats
            .iter()
            .enumerate()
            .flat_map(|(c, chat)| (0..chat.entradas.len()).map(move |i| (c as u32, i as u32)))
            .collect();
        let chats = &self.chats;
        orden.sort_by(|x, y| {
            b::comparar(
                &chats[x.0 as usize].entradas[x.1 as usize],
                &chats[y.0 as usize].entradas[y.1 as usize],
            )
        });
        self.orden = orden;
    }

    fn entrada(&self, (c, i): (u32, u32)) -> &b::Entrada {
        &self.chats[c as usize].entradas[i as usize]
    }

    fn extra(&self, (c, i): (u32, u32)) -> &Extra {
        &self.chats[c as usize].extras[i as usize]
    }

    fn buscar(&self, p: &Pedido) -> Resultado {
        let hallado = b::buscar_en(
            self.orden.iter().map(|&par| self.entrada(par)),
            &p.aguja,
            p.tope,
            p.pestana,
        );
        let encontrados = hallado
            .aciertos
            .iter()
            .map(|a| {
                let par = self.orden[a.entrada];
                let (e, x) = (self.entrada(par), self.extra(par));
                // Se mira el disco solo para lo que se ensena.
                let esta = x.ruta.as_deref().is_some_and(Path::is_file);
                let cosa = match x.cosa {
                    b::Cosa::Fichero { .. } => b::Cosa::Fichero { esta },
                    c => c,
                };
                Encontrado {
                    chat: e.chat.clone(),
                    codigo: e.codigo.clone(),
                    cuando: e.cuando,
                    fragmento: a.pieza.map_or_else(b::Fragmento::default, |k| {
                        b::fragmento(&e.piezas[k].texto, &p.aguja, ANTES, LARGO)
                    }),
                    muestra: x.muestra,
                    nombre: x.nombre.clone(),
                    foto: x
                        .ruta
                        .clone()
                        .filter(|_| esta && x.muestra == Muestra::Foto),
                    al_pulsar: b::al_pulsar(cosa),
                }
            })
            .collect();
        Resultado {
            turno: p.turno,
            encontrados,
            total: hallado.total,
            por_pestana: hallado.por_pestana,
        }
    }
}

// --- Pintar ------------------------------------------------------------------

/// **Pinta la seccion «Mensajes»** debajo de las filas de chats: las
/// pestanas, la cabecera, los encontrados y «Ver más» o «Nada con eso»,
/// solo lo que cae dentro de la lista.
pub(super) fn pintar(
    p: &Pintor,
    d: &Disposicion,
    tema: &Tema,
    escala: u32,
    lista: &Lista,
    en_chats: &BuscarEnChats,
) {
    let seccion = en_chats.seccion();
    if !seccion.hay() || d.filas.ancho == 0 || d.filas.alto == 0 || d.plegada {
        return;
    }
    let e = escala as f32 / 100.0;
    let chats = lista.orden.len();
    let caja = |el: Elemento| caja_de(d, lista.scroll, chats, escala, &seccion, el);
    let se_ve = |r: pixpin_geom::Rect| r.alto > 0 && r.abajo() > d.filas.y && r.y < d.filas.abajo();
    p.empujar_recorte(rf(d.filas));

    // Las pestanas por tipo, como las pildoras de la ficha de un chat.
    let banda = caja(Elemento::Pestanas);
    if se_ve(banda) {
        let (cajas, _) = en_chats.pestanas();
        let tam = b::PESTANA_TAM * e;
        for ((c, pestana), rotulo) in cajas.iter().zip(Pestana::TODAS).zip(en_chats.rotulos()) {
            let c = pixpin_geom::Rect {
                x: banda.x + c.x,
                y: banda.y + c.y,
                ..*c
            };
            let puesta = pestana == en_chats.pestana;
            let (fondo, letra) = if puesta {
                (tema.enviar, tema.texto_elegido)
            } else {
                (tema.pildora, tema.pildora_texto)
            };
            p.rellenar_redondeado(rf(c), c.alto as f32 / 2.0, fondo);
            let hueco = (c.ancho as f32 - 12.0 * e).max(0.0);
            let (w, h) = p.medir_texto(&rotulo, tam);
            p.texto_linea(
                &rotulo,
                c.x as f32 + (c.ancho as f32 - w.min(hueco)) / 2.0,
                c.y as f32 + (c.alto as f32 - h) / 2.0,
                tam,
                hueco,
                letra,
            );
        }
    }

    if seccion.nada {
        let fila = caja(Elemento::Nada);
        if se_ve(fila) {
            let rotulo = lista.textos.t("chat-buscar-nada");
            let tam = chat::TEXTO_TAM * e;
            let (w, h) = p.medir_texto(&rotulo, tam);
            p.texto(
                &rotulo,
                fila.x as f32 + (fila.ancho as f32 - w) / 2.0,
                fila.y as f32 + (fila.alto as f32 - h) / 2.0,
                tam,
                tema.apagado,
            );
        }
    }

    let Some(r) = en_chats.resultado() else {
        p.soltar_recorte();
        return;
    };

    // La cabecera: «Mensajes» y cuantos, en una banda como la de Telegram.
    let cab = caja(Elemento::Cabecera);
    if se_ve(cab) {
        p.rellenar(rf(cab), tema.cabecera);
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("n", r.total);
        let rotulo = lista.textos.t_args("chat-buscar-mensajes", &args);
        let tam = chat::CONTADOR_TAM * e;
        let (_, h) = p.medir_texto(&rotulo, tam);
        p.texto_linea(
            &rotulo,
            cab.x as f32 + chat::AVATAR_X as f32 * e + 2.0 * e,
            cab.y as f32 + (cab.alto as f32 - h) / 2.0,
            tam,
            cab.ancho as f32 - 20.0 * e,
            tema.apagado,
        );
    }

    for (n, enc) in r.encontrados.iter().enumerate() {
        let fila = caja(Elemento::Mensaje(n));
        if !se_ve(fila) {
            continue;
        }
        let sobre = lista.sobre_mensaje == Some(Elemento::Mensaje(n));
        if sobre {
            p.rellenar(rf(fila), tema.fila_sobre);
        }
        pintar_encontrado(
            p,
            tema,
            escala,
            lista,
            fila,
            enc,
            &en_chats.miniaturas,
            sobre,
        );
    }

    if seccion.ver_mas {
        let fila = caja(Elemento::VerMas);
        if se_ve(fila) {
            if lista.sobre_mensaje == Some(Elemento::VerMas) {
                p.rellenar(rf(fila), tema.fila_sobre);
            }
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("n", r.total - r.encontrados.len());
            let rotulo = lista.textos.t_args("chat-buscar-ver-mas", &args);
            let tam = chat::TEXTO_TAM * e;
            let (w, h) = p.medir_texto(&rotulo, tam);
            p.texto(
                &rotulo,
                fila.x as f32 + (fila.ancho as f32 - w) / 2.0,
                fila.y as f32 + (fila.alto as f32 - h) / 2.0,
                tam,
                tema.enviar,
            );
        }
    }
    p.soltar_recorte();
}

/// El avatar de un chat en `a`, como en su fila: su logo o su color con
/// iniciales.
fn pintar_avatar_del_chat(
    p: &Pintor,
    tema: &Tema,
    ficha: Option<&pixpin_proyecto::almacen::Ficha>,
    nombre: &str,
    a: RectF,
) {
    if ficha.is_some_and(|f| logo::pintar(p, f, a)) {
        return;
    }
    let color = ficha.map_or(tema.apagado, |f| color_avatar(&f.codigo_unico()));
    p.rellenar_redondeado(a, a.ancho / 2.0, color);
    let letras = iniciales(nombre);
    let tam = a.alto * 0.4;
    let (w, h) = p.medir_texto(&letras, tam);
    p.texto(
        &letras,
        a.x + (a.ancho - w) / 2.0,
        a.y + (a.alto - h) / 2.0,
        tam,
        Color::BLANCO,
    );
}

/// El icono de lo que no tiene miniatura, por su tipo.
fn icono_de(muestra: Muestra) -> &'static Icono {
    match muestra {
        Muestra::Foto => &mi::IMAGE,
        Muestra::Video => &mi::VIDEOCAM,
        Muestra::Voz => &mi::MIC,
        Muestra::Dibujo => &mi::DRAW,
        Muestra::Pagina | Muestra::Carpeta => &mi::MENU_BOOK,
        Muestra::Proyecto => &mi::FOLDER,
        Muestra::Enlace => &mi::LINK,
        Muestra::Archivo | Muestra::MiniApp | Muestra::Nota => &mi::DESCRIPTION,
    }
}

/// **Lo que va delante de un resultado**: la miniatura de la foto, el icono
/// del formato de un archivo (el de su fila en el chat) o el de su tipo.
fn pintar_muestra(
    p: &Pintor,
    tema: &Tema,
    escala: u32,
    enc: &Encontrado,
    miniaturas: &crate::miniaturas::Miniaturas,
    a: RectF,
) {
    let e = escala as f32 / 100.0;
    if let Some((bitmap, ancho, alto)) = enc.foto.as_deref().and_then(|f| miniaturas.ya(f)) {
        crate::miniaturas::pintar_recortado(p, bitmap, a, ancho, alto);
        return;
    }
    if enc.muestra == Muestra::Archivo && !enc.nombre.is_empty() {
        icono_de_tipo::pintar(p, &enc.nombre, a);
        return;
    }
    p.rellenar_redondeado(a, 8.0 * e, tema.pildora);
    let lado = a.ancho * 0.5;
    p.icono(
        icono_de(enc.muestra),
        RectF {
            x: a.x + (a.ancho - lado) / 2.0,
            y: a.y + (a.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        tema.pildora_texto,
    );
}

/// **Una fila de resultado**: delante, la cosa misma (su miniatura o su
/// icono; una nota, el avatar de su chat); arriba «en <chat>» con la foto
/// del chat y la fecha a la derecha; debajo, el trozo con lo encontrado
/// resaltado (o su nombre, si no hay trozo) y la flechita de «ir al
/// mensaje».
#[allow(clippy::too_many_arguments)] // lo que hace falta para pintarla
fn pintar_encontrado(
    p: &Pintor,
    tema: &Tema,
    escala: u32,
    lista: &Lista,
    fila: pixpin_geom::Rect,
    enc: &Encontrado,
    miniaturas: &crate::miniaturas::Miniaturas,
    sobre: bool,
) {
    let e = escala as f32 / 100.0;
    let partes = chat::partes_fila(fila, false, escala);
    let ficha = lista.fichas.iter().find(|f| f.id == enc.chat);
    let nombre = ficha.map_or("", |f| f.nombre.as_str());

    let a = rf(partes.avatar);
    let es_nota = enc.muestra == Muestra::Nota;
    if es_nota {
        pintar_avatar_del_chat(p, tema, ficha, nombre, a);
    } else {
        pintar_muestra(p, tema, escala, enc, miniaturas, a);
    }
    if partes.ancho_texto == 0 {
        return;
    }

    // La fecha primero: dice cuanto sitio le queda al nombre.
    let hora = chat::etiqueta_hora(pixpin_shell::entorno::a_local(enc.cuando), lista.ahora);
    let tam_hora = chat::CONTADOR_TAM * e;
    let tam = chat::TEXTO_TAM * e;
    let mut hueco = partes.ancho_texto as f32;
    if !hora.is_empty() {
        let (w, h) = p.medir_texto(&hora, tam_hora);
        p.texto(
            &hora,
            partes.derecha as f32 - w,
            partes.nombre.y as f32 + (tam - h) / 2.0,
            tam_hora,
            tema.apagado,
        );
        hueco -= w + chat::HORA_HUECO as f32 * e;
    }
    // Arriba, de que chat es: en una nota, su nombre (el avatar ya es el
    // del chat); en lo demas, «en <chat>» con su foto pequena delante.
    let (mut x, y) = (partes.nombre.x as f32, partes.nombre.y as f32);
    let arriba = if es_nota {
        nombre.to_string()
    } else {
        let lado = 16.0 * e;
        let caja = RectF {
            x,
            y: y + (tam * 1.3 - lado) / 2.0,
            ancho: lado,
            alto: lado,
        };
        pintar_avatar_del_chat(p, tema, ficha, nombre, caja);
        x += lado + 6.0 * e;
        hueco -= lado + 6.0 * e;
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("chat", nombre);
        lista.textos.t_args("chat-buscar-en", &args)
    };
    p.texto_linea(&arriba, x, y, tam, hueco.max(0.0), tema.texto);

    // La flechita de «ir al mensaje», con su circulo al pasar por la fila.
    let flecha = b::flecha(fila, escala);
    if sobre {
        p.rellenar_redondeado(rf(flecha), flecha.ancho as f32 / 2.0, tema.pildora);
    }
    let lado = flecha.ancho as f32 * 0.7;
    p.icono(
        &mi::ARROW_FORWARD,
        RectF {
            x: flecha.x as f32 + (flecha.ancho as f32 - lado) / 2.0,
            y: flecha.y as f32 + (flecha.alto as f32 - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        tema.apagado,
    );

    // El trozo: en una linea sin partir (ancho de sobra) y recortado a su
    // hueco; lo encontrado en negrita y con un fondo del color de enviar.
    // Sin trozo (una pestana sin nada escrito y sin texto), su nombre.
    let texto = if enc.fragmento.texto.is_empty() {
        &enc.nombre
    } else {
        &enc.fragmento.texto
    };
    let tramos: Vec<pixpin_render::Tramo> = enc
        .fragmento
        .tramos
        .iter()
        .filter_map(|t| {
            let (inicio, longitud) = t.en_utf16(texto);
            (longitud > 0).then_some(pixpin_render::Tramo {
                inicio,
                longitud,
                estilo: pixpin_render::EstiloTexto {
                    negrita: true,
                    ..Default::default()
                },
            })
        })
        .collect();
    const SIN_PARTIR: f32 = 100_000.0;
    let (x, y) = (partes.resumen.x as f32, partes.resumen.y as f32);
    let ancho = (flecha.x as f32 - 4.0 * e - x).max(0.0);
    p.empujar_recorte(RectF {
        x,
        y,
        ancho,
        alto: tam * 1.6,
    });
    let fondo = Color {
        a: 0.28,
        ..tema.enviar
    };
    for t in &tramos {
        for c in p.cajas_de_trozo(texto, tam, SIN_PARTIR, &tramos, t.inicio, t.longitud) {
            p.rellenar_redondeado(
                RectF {
                    x: x + c.x - 1.0 * e,
                    y: y + c.y,
                    ancho: c.ancho + 2.0 * e,
                    alto: c.alto,
                },
                3.0 * e,
                fondo,
            );
        }
    }
    p.parrafo(texto, x, y, tam, SIN_PARTIR, &tramos, tema.apagado);
    p.soltar_recorte();
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::cuaderno::Mensaje;

    #[test]
    fn de_la_nota_rica_se_busca_el_texto_y_no_las_rutas_de_sus_fotos() {
        let m = Mensaje {
            id: "m1".into(),
            cuando: 7,
            texto: "Fachada norte ![grieta](pixpin:files/fotos/IMG_0042.jpg) revisar".into(),
            transcripcion: Some("lo dicho en la obra".into()),
            nombre: "plano.pdf".into(),
            ..Default::default()
        };
        let e = entrada_de("chat", &m);
        assert_eq!(b::buscar([&e], "img_0042", 5).total, 0, "la ruta no cuenta");
        assert_eq!(b::buscar([&e], "grieta", 5).total, 1, "su pie si");
        assert_eq!(b::buscar([&e], "LO DICHO", 5).total, 1);
        assert_eq!(b::buscar([&e], "plano", 5).total, 1);
        assert_eq!(e.codigo, m.codigo_unico());
    }

    #[test]
    fn lo_que_no_tiene_burbuja_no_se_encuentra() {
        let sistema = Mensaje {
            id: format!("{}x", pixpin_proyecto::cuaderno::PREFIJO_DE_SISTEMA),
            ..Default::default()
        };
        assert!(!se_ve_en_el_chat(&sistema));
        let normal = Mensaje {
            id: "normal".into(),
            ..Default::default()
        };
        assert!(se_ve_en_el_chat(&normal));
    }

    fn mensaje(id: &str, clase: Clase, nombre: &str, texto: &str) -> Mensaje {
        Mensaje {
            id: id.into(),
            clase: Some(clase),
            nombre: nombre.into(),
            texto: texto.into(),
            ..Default::default()
        }
    }

    #[test]
    fn cada_mensaje_sale_en_las_pestanas_que_en_la_ficha_de_su_chat() {
        let tipos = |m: &Mensaje| -> Vec<Pestana> {
            Pestana::TODAS
                .into_iter()
                .filter(|p| *p != Pestana::Todo && tipos_de(m).tiene(*p))
                .collect()
        };
        let foto = mensaje("f", Clase::Imagen, "obra.jpg", "");
        assert_eq!(tipos(&foto), [Pestana::Fotos]);
        // Un video no cuenta como foto ni como archivo.
        let video = mensaje("v", Clase::Archivo, "visita.mp4", "");
        assert_eq!(tipos(&video), [Pestana::Videos]);
        let pdf = mensaje("p", Clase::Archivo, "plano.pdf", "");
        assert_eq!(tipos(&pdf), [Pestana::Archivos]);
        let enlace = mensaje("e", Clase::Nota, "", "mira https://ejemplo.com/obra");
        assert_eq!(tipos(&enlace), [Pestana::Enlaces]);
        assert_eq!(
            tipos(&mensaje("z", Clase::Voz, "a.m4a", "")),
            [Pestana::Voz]
        );
        assert_eq!(
            tipos(&mensaje("d", Clase::Dibujo, "Croquis", "")),
            [Pestana::Dibujos]
        );
        assert!(tipos(&mensaje("n", Clase::Nota, "", "repasar")).is_empty());
        // Lo del buzon no sale en ninguna (en su chat tampoco): solo en «Todo».
        let buzon = Mensaje {
            en_buzon: true,
            ..pdf.clone()
        };
        assert!(tipos(&buzon).is_empty());
        assert!(entrada_de("c", &pdf).tipos.tiene(Pestana::Archivos));
    }

    #[test]
    fn lo_que_se_ensena_y_lo_que_hace_pulsarlo() {
        let raiz = Path::new("/no/esta");
        let pdf = Mensaje {
            ruta: Some("files/plano.pdf".into()),
            ..mensaje("p", Clase::Archivo, "plano.pdf", "")
        };
        let x = extra_de(raiz, "c", &pdf);
        assert_eq!(x.muestra, Muestra::Archivo);
        assert_eq!(x.cosa, b::Cosa::Fichero { esta: false });
        assert!(x.ruta.is_some());
        let x = extra_de(raiz, "c", &mensaje("v", Clase::Imagen, "a.mov", ""));
        assert_eq!(x.muestra, Muestra::Video);
        let x = extra_de(raiz, "c", &mensaje("d", Clase::Dibujo, "Croquis", ""));
        assert_eq!((x.muestra, x.cosa), (Muestra::Dibujo, b::Cosa::SinNada));
        let lienzo = Mensaje {
            referencia: Some("dib1".into()),
            ..mensaje("d", Clase::Dibujo, "Croquis", "")
        };
        assert_eq!(extra_de(raiz, "c", &lienzo).cosa, b::Cosa::ConVisor);
        let x = extra_de(raiz, "c", &mensaje("e", Clase::Nota, "", "en www.obra.es"));
        assert_eq!(
            (x.muestra, x.cosa),
            (Muestra::Enlace, b::Cosa::Nota { enlace: true })
        );
        let x = extra_de(raiz, "c", &mensaje("n", Clase::Nota, "", "repasar"));
        assert_eq!(
            (x.muestra, x.cosa),
            (Muestra::Nota, b::Cosa::Nota { enlace: false })
        );
        let x = extra_de(raiz, "c", &mensaje("z", Clase::Voz, "a.m4a", ""));
        assert_eq!((x.muestra, x.cosa), (Muestra::Voz, b::Cosa::Voz));
        let cuaderno = Mensaje {
            miniapp: Some(carpeta::MINIAPP.into()),
            ..mensaje("k", Clase::MiniApp, "Diario", "")
        };
        assert_eq!(extra_de(raiz, "c", &cuaderno).muestra, Muestra::Carpeta);
    }

    #[test]
    fn ir_al_mensaje_llega_a_el_o_a_la_tarjeta_de_su_carpeta() {
        let pagina = mensaje("d1", Clase::Dibujo, "Dia 1", "");
        let mut c = Carpeta::default();
        assert!(c.anadir("d1", "Día 1"));
        let carpeta = Mensaje {
            miniapp: Some(carpeta::MINIAPP.into()),
            ..mensaje("c", Clase::MiniApp, "Diario", &c.escribir())
        };
        let suelto = mensaje("n", Clase::Nota, "", "suelto");
        let mensajes = [carpeta, pagina, suelto];
        // Dentro de una carpeta: a la tarjeta de la carpeta.
        assert_eq!(adonde_saltar(&mensajes, 1), 0);
        // Suelto, a el.
        assert_eq!(adonde_saltar(&mensajes, 2), 2);
        assert_eq!(adonde_saltar(&mensajes, 9), 9);
    }
}
