//! **La caja de una linea de toda la app**: buscar en el chat, buscar en
//! la lista, renombrar, el codigo de un chat compartido... (13-oct-2026).
//!
//! Antes cada una de esas cajas era un `String` con `push` al escribir y
//! `pop` al borrar, y una barrita `|` pegada al final. El usuario lo dijo
//! asi: «cuando intento buscar algo en el chat, su espacio no acepta todas
//! las funciones de los espacios de texto, como el atajo Ctrl+Borrar que
//! borra toda la palabra, entre otros; no se ve donde estoy escribiendo».
//! Y su regla: un solo componente con variables, no copias.
//!
//! Por eso esto NO es otra caja: es el [`Cursor`] de la caja de escribir
//! del chat (el mismo que salta palabras, elige con Mayusculas y pone la
//! rayita donde se pulsa) con lo que le falta a una caja de una linea de
//! Windows:
//!
//! - deshacer y rehacer (Ctrl+Z, Ctrl+Y, Ctrl+Mayus+Z), juntando lo que se
//!   teclea seguido en un solo paso, como el `EDIT` de Windows;
//! - el portapapeles (Ctrl+C/X/V y Ctrl/Mayus+Insertar, Mayus+Supr), y al
//!   pegar los saltos de renglon pasan a espacios: es una sola linea;
//! - el raton: clic, arrastre, doble clic = palabra, triple = todo;
//! - su pintado: la rayita parpadea donde esta (y se queda quieta a los
//!   cinco segundos sin tocar nada, como en Windows, que asi no despierta
//!   el hilo para nada), y el texto se corre para que siempre se vea.
//!
//! Las letras llegan por `WM_CHAR`, asi que las tildes con tecla muerta, la
//! ñ y lo que compone el IME entran igual que una letra cualquiera; los
//! mandos que tambien llegan como caracter (Ctrl+Retroceso manda un 0x7F)
//! se descartan aqui y no acaban escritos.
//!
//! **Las variables** (lo unico que cambia de una caja a otra): un tope de
//! letras (`con_maximo`), un filtro de letras (`con_filtro`: la hora de un
//! recordatorio), la rayita quieta para las ventanas que no repintan solas
//! (`sin_parpadeo`) y la variante de **varios renglones**
//! (`en_varias_lineas`: Mayus+Intro parte el renglon, arriba/abajo van de
//! renglon en renglon y el texto se ajusta al ancho), que es la caja de las
//! lecciones, la timeline, la galeria y las tareas (`lecciones::ui::Campo`).
//! Y **las piezas enteras** ([`CajaLinea::tecla_con_piezas`],
//! [`CajaLinea::pintar_con_fichas`]): trozos del texto que se borran y se
//! saltan de una vez y se pintan como una chapa, como la ficha `[img 01]` de
//! una imagen pegada en la caja de apuntar tareas.
//!
//! La usan (14-oct-2026): en el chat, el buscador de la lista, la lupa de la
//! conversacion y la del panel, el nombre del proyecto (tambien en la
//! tarjeta de Proyectos), el nombre y la descripcion de un mensaje, «Quien
//! llama», la hora a mano, el pie del cuadro de enviar y el buscador de la
//! hoja del «+»; fuera, Buscar todo, los dos buscadores de Ajustes, el
//! codigo y el nombre de los chats compartidos, la caja de buscar de los
//! lectores (`buscador`), el nombre que se cambia con F2 en ellos
//! (`renombrar_doc`) y `lecciones::ui::Campo`. Desde el 14-oct-2026 tambien
//! la caja de apuntar tareas (`tareas::ventana::Campo`, con sus fichas) y los
//! campos de Sincronizar: el nombre, unirse (codigo y direccion), la
//! direccion, y el codigo de Recibir y Enviar por wifi.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use pixpin_geom::Punto;
use pixpin_render::{Color, Pintor, RectF};

use super::{Cursor, cajas_de_letras, indice_en_punto, palabra_en, sitio_del_cursor};

const VK_RETROCESO: u32 = 0x08;
const VK_ENTRAR: u32 = 0x0D;
const VK_ARRIBA: u32 = 0x26;
const VK_ABAJO: u32 = 0x28;
const VK_FIN: u32 = 0x23;
const VK_INICIO: u32 = 0x24;
const VK_IZQUIERDA: u32 = 0x25;
const VK_DERECHA: u32 = 0x27;
const VK_INSERTAR: u32 = 0x2D;
const VK_SUPR: u32 = 0x2E;
const VK_A: u32 = 0x41;
const VK_C: u32 = 0x43;
const VK_V: u32 = 0x56;
const VK_X: u32 = 0x58;
const VK_Y: u32 = 0x59;
const VK_Z: u32 = 0x5A;

/// Medio ciclo de la rayita: lo que trae Windows de serie
/// (`GetCaretBlinkTime`).
const PARPADEO_MS: u128 = 530;
/// A los cinco segundos sin tocar la caja la rayita deja de parpadear y se
/// queda encendida, como hace Windows: asi la ventana en reposo no se
/// despierta dos veces por segundo.
const PARPADEO_HASTA: Duration = Duration::from_secs(5);
/// El tiempo de doble clic de Windows de serie.
const DOBLE_CLIC: Duration = Duration::from_millis(500);
/// Lo que se puede mover el raton entre dos clics de un doble clic
/// (`SM_CXDOUBLECLK`).
const DOBLE_CLIC_HOLGURA: i32 = 4;
/// Pasos de deshacer que se guardan: en una caja de una linea, de sobra.
const DESHACER_MAX: usize = 100;

/// De donde lee y donde deja el texto Ctrl+C/X/V. Va aparte para que las
/// pruebas no toquen el portapapeles de verdad del usuario.
pub(crate) trait Portapapeles {
    fn leer_texto(&mut self) -> Option<String>;
    fn copiar(&mut self, texto: &str);
}

/// El portapapeles de Windows.
pub(crate) struct PortapapelesWindows;

impl Portapapeles for PortapapelesWindows {
    fn leer_texto(&mut self) -> Option<String> {
        use pixpin_codec::portapapeles::ContenidoPortapapeles as C;
        match pixpin_codec::portapapeles::leer()? {
            C::Texto(t) => Some(t),
            // Ficheros copiados en el Explorador: sus nombres, que es lo que
            // se quiere buscar o escribir al pegarlos en una caja de texto.
            C::Rutas(rutas) => Some(
                rutas
                    .iter()
                    .filter_map(|r| r.file_name().map(|n| n.to_string_lossy().into_owned()))
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            C::Imagen(_) => None,
        }
    }

    fn copiar(&mut self, texto: &str) {
        if let Err(e) = pixpin_codec::portapapeles::copiar_texto(texto) {
            tracing::warn!(?e, "no se pudo copiar lo escrito");
        }
    }
}

/// Que hizo la caja con una tecla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Respuesta {
    /// No es una tecla de caja de texto: que la atienda la ventana (Escape,
    /// Entrar, las flechas de arriba y abajo, otros atajos).
    NoEsMia,
    /// Era suya, pero el texto sigue igual (se movio la rayita, se copio).
    Movida,
    /// El texto cambio: quien filtra con el tiene que volver a filtrar.
    Cambiada,
}

impl Respuesta {
    /// La tecla era de la caja: no hay que pasarsela a nadie mas.
    pub(crate) fn atendida(self) -> bool {
        self != Respuesta::NoEsMia
    }

    pub(crate) fn cambio(self) -> bool {
        self == Respuesta::Cambiada
    }
}

/// La clase de cambio, para juntar en un solo paso de deshacer lo que se
/// teclea (o se borra) seguido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edicion {
    Escribir,
    Borrar,
    Otra,
}

/// Los colores con que se pinta una caja: cada ventana pone los de su tema.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Colores {
    pub texto: Color,
    /// El texto de invitacion («Buscar»), con la caja vacia.
    pub invitacion: Color,
    pub seleccion: Color,
    pub rayita: Color,
}

/// **Las fichas**: trozos del texto que se pintan como una chapa de otro
/// color (la `[img 01]` de una imagen pegada). Son una variable del pintado:
/// quien las usa dice donde estan cada vez, porque solo el sabe que son.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fichas<'a> {
    pub rangos: &'a [std::ops::Range<usize>],
    pub fondo: Color,
    pub letra: Color,
    pub radio: f32,
}

/// Lo que deja apuntado el pintado para el raton: las cajas de las letras
/// (de la MISMA disposicion con que se pinto), donde quedo el texto y
/// cuanto esta corrido.
#[derive(Debug, Default, Clone)]
struct Pintado {
    texto: String,
    tam: f32,
    cajas: Vec<(usize, RectF)>,
    /// La caja entera en la ventana: donde un clic es de ella.
    zona: Option<RectF>,
    /// La esquina del texto (ya corrido) en la ventana.
    origen: (f32, f32),
    /// El ancho con que se partieron los renglones (sin fin en una linea).
    ancho: f32,
    /// Cuanto se corre el texto a la izquierda para que se vea la rayita.
    corrido: f32,
    /// Y cuanto hacia arriba, en la de varios renglones.
    corrido_y: f32,
}

/// **Una caja de texto de una linea.** Se lee como un `&str` (`Deref`), asi
/// que filtrar con ella es igual que con el `String` de antes.
#[derive(Debug, Clone)]
pub struct CajaLinea {
    texto: String,
    cursor: Cursor,
    deshacer: Vec<(String, Cursor)>,
    rehacer: Vec<(String, Cursor)>,
    ultima: Option<Edicion>,
    /// El ultimo clic: cuando, donde y cuantos seguidos (doble, triple).
    clic: Option<(Instant, Punto, u8)>,
    /// Se esta eligiendo con el boton pulsado.
    eligiendo: bool,
    /// Desde cuando no se toca: la rayita parpadea contando desde aqui, y
    /// esta encendida justo despues de cada tecla.
    tocada: Instant,
    pintado: RefCell<Pintado>,
    /// Cuantas letras caben como mucho (`None`, sin tope): un nombre de
    /// quien llama, un codigo de seis cifras...
    maximo: Option<usize>,
    /// La variante de varios renglones (una leccion, una nota de la
    /// timeline): Mayus+Intro parte el renglon, las flechas de arriba y
    /// abajo van de renglon en renglon y el texto se ajusta al ancho en vez
    /// de correrse. Es el MISMO cursor; solo cambia esto.
    varias: bool,
    /// Si la rayita parpadea. Las ventanas que no repintan solas (sin
    /// temporizador) la quieren quieta: si no, podria quedarse apagada.
    parpadea: bool,
    /// Que letras admite (`None`, todas): la hora de un recordatorio solo
    /// cifras y separadores. Vale para lo tecleado y para lo pegado.
    filtro: Option<fn(char) -> bool>,
}

impl From<&str> for CajaLinea {
    fn from(t: &str) -> Self {
        Self::con_texto(t)
    }
}

impl Default for CajaLinea {
    fn default() -> Self {
        Self::nueva()
    }
}

impl std::ops::Deref for CajaLinea {
    type Target = str;
    fn deref(&self) -> &str {
        &self.texto
    }
}

impl PartialEq<&str> for CajaLinea {
    fn eq(&self, otro: &&str) -> bool {
        self.texto == *otro
    }
}

/// Lo pegado, en una sola linea: los saltos de renglon pasan a un espacio
/// (sin dejar renglones vacios ni sangrias por medio) y los tabuladores
/// tambien; cualquier otro mando se cae.
pub(crate) fn en_una_linea(s: &str) -> String {
    let limpio = |t: &str| -> String {
        t.chars()
            .map(|c| if c == '\t' { ' ' } else { c })
            .filter(|c| !c.is_control())
            .collect()
    };
    if !s.contains(['\n', '\r']) {
        return limpio(s);
    }
    s.split(['\r', '\n'])
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(limpio)
        .collect::<Vec<_>>()
        .join(" ")
}

impl CajaLinea {
    pub(crate) fn nueva() -> Self {
        Self {
            texto: String::new(),
            cursor: Cursor::default(),
            deshacer: Vec::new(),
            rehacer: Vec::new(),
            ultima: None,
            clic: None,
            eligiendo: false,
            tocada: Instant::now(),
            pintado: RefCell::new(Pintado::default()),
            maximo: None,
            varias: false,
            parpadea: true,
            filtro: None,
        }
    }

    /// Una caja que empieza con `t` escrito y la rayita al final.
    pub(crate) fn con_texto(t: &str) -> Self {
        let mut c = Self::nueva();
        c.texto = en_una_linea(t);
        c
    }

    /// La misma caja con un tope de letras: lo que no quepa no entra, ni
    /// tecleado ni pegado.
    pub(crate) fn con_maximo(mut self, letras: usize) -> Self {
        self.maximo = Some(letras);
        self.texto = self.texto.chars().take(letras).collect();
        self
    }

    /// Lo que cabe de `s` en el sitio de lo elegido, segun el tope.
    fn lo_que_cabe<'a>(&self, s: &'a str) -> &'a str {
        let Some(max) = self.maximo else {
            return s;
        };
        let elegidas = self.seleccionado().map_or(0, |e| e.chars().count());
        let hay = self.texto.chars().count() - elegidas;
        let caben = max.saturating_sub(hay);
        s.char_indices().nth(caben).map_or(s, |(i, _)| &s[..i])
    }

    /// La variante de varios renglones (ver `varias`). Lo que hubiera
    /// escrito se queda como esta.
    pub(crate) fn en_varias_lineas(mut self) -> Self {
        self.varias = true;
        self
    }

    /// La variante de una linea otra vez (lo que ya hay escrito no cambia).
    pub(crate) fn en_una_sola_linea(mut self) -> Self {
        self.varias = false;
        self
    }

    /// La rayita quieta, sin parpadear (ver `parpadea`).
    pub(crate) fn sin_parpadeo(mut self) -> Self {
        self.parpadea = false;
        self
    }

    /// Solo admite las letras que diga `filtro` (ver `filtro`).
    pub(crate) fn con_filtro(mut self, filtro: fn(char) -> bool) -> Self {
        self.filtro = Some(filtro);
        self
    }

    pub(crate) fn es_de_varias_lineas(&self) -> bool {
        self.varias
    }

    /// Lo que entra de fuera (pegado, puesto), segun la variante: en una
    /// linea, sin saltos; en varias, con los saltos de Windows hechos `\n`.
    /// Y con el filtro, si lo hay.
    fn limpiar(&self, s: &str) -> String {
        let limpio: String = if !self.varias {
            en_una_linea(s)
        } else {
            s.replace("\r\n", "\n")
                .chars()
                .map(|c| match c {
                    '\r' => '\n',
                    '\t' => ' ',
                    c => c,
                })
                .filter(|c| *c == '\n' || !c.is_control())
                .collect()
        };
        match self.filtro {
            Some(f) => limpio.chars().filter(|c| f(*c)).collect(),
            None => limpio,
        }
    }

    pub(crate) fn texto(&self) -> &str {
        &self.texto
    }

    /// La rayita en `i` (bytes; se ajusta al borde de una letra).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn ir_a(&mut self, i: usize) {
        self.cursor.poner(&self.texto, i, false);
        self.despertar();
    }

    /// La rayita al final de lo escrito, sin nada elegido.
    pub(crate) fn al_final(&mut self) {
        self.cursor.soltar();
        self.despertar();
    }

    /// Cambia todo lo escrito desde fuera (abrir la caja con un nombre ya
    /// puesto): la rayita al final y el deshacer en blanco.
    pub(crate) fn poner_texto(&mut self, t: &str) {
        self.texto = self.limpiar(t);
        if let Some(m) = self.maximo {
            self.texto = self.texto.chars().take(m).collect();
        }
        self.cursor.soltar();
        self.deshacer.clear();
        self.rehacer.clear();
        self.ultima = None;
        self.despertar();
    }

    /// Deja la caja vacia, como recien abierta.
    pub(crate) fn vaciar(&mut self) {
        self.poner_texto("");
    }

    /// Donde esta la rayita (en bytes).
    pub(crate) fn pos(&self) -> usize {
        self.cursor.pos(&self.texto)
    }

    pub(crate) fn seleccionado(&self) -> Option<&str> {
        self.cursor.seleccionado(&self.texto)
    }

    /// Elige todo: lo que hace Windows al entrar con Tab en una caja, y lo
    /// que conviene al abrir para renombrar (escribir sustituye el nombre).
    pub(crate) fn elegir_todo(&mut self) {
        self.cursor.todo(&self.texto);
        self.despertar();
    }

    /// Si hay algo que deshacer: la ventana que tiene su propio Ctrl+Z (el
    /// «Deshacer» de un aviso) se lo deja a la caja cuando lo hay.
    pub(crate) fn hay_deshacer(&self) -> bool {
        !self.deshacer.is_empty()
    }

    /// **Una tecla con piezas enteras** (`piezas`, rangos del texto que no se
    /// parten: la ficha `[img 01]` de una imagen pegada). Retroceso al final
    /// de una y Supr al principio la borran entera (con Ctrl tambien: media
    /// ficha no es nada), y las flechas la saltan de una vez (con Mayusculas,
    /// eligiendola entera). Lo demas, como [`Self::tecla`].
    pub(crate) fn tecla_con_piezas(
        &mut self,
        vk: u32,
        shift: bool,
        ctrl: bool,
        pp: &mut dyn Portapapeles,
        piezas: &[std::ops::Range<usize>],
    ) -> Respuesta {
        let p = self.pos();
        let sin_eleccion = self.cursor.seleccion(&self.texto).is_none();
        let antes = piezas
            .iter()
            .find(|r| r.start < r.end && r.end == p)
            .cloned();
        let despues = piezas
            .iter()
            .find(|r| r.start < r.end && r.start == p)
            .cloned();
        match (vk, antes, despues) {
            (VK_RETROCESO, Some(r), _) if sin_eleccion => {
                self.cursor.elegir(&self.texto, r);
                return self.tecla(VK_RETROCESO, false, false, pp);
            }
            (VK_SUPR, _, Some(r)) if sin_eleccion && !shift => {
                self.cursor.elegir(&self.texto, r);
                return self.tecla(VK_SUPR, false, false, pp);
            }
            // Sin Mayusculas y con algo elegido, la flecha solo lo suelta
            // (lo de siempre): no se salta nada.
            (VK_IZQUIERDA, Some(r), _) if shift || sin_eleccion => {
                self.cursor.poner(&self.texto, r.start, shift);
            }
            (VK_DERECHA, _, Some(r)) if shift || sin_eleccion => {
                self.cursor.poner(&self.texto, r.end, shift);
            }
            _ => return self.tecla(vk, shift, ctrl, pp),
        }
        self.ultima = None;
        self.despertar();
        Respuesta::Movida
    }

    /// Si la rayita quedo dentro de una pieza (un clic encima), al final de
    /// ella: lo que se escriba no la parte.
    pub(crate) fn fuera_de_piezas(&mut self, piezas: &[std::ops::Range<usize>]) {
        if self.cursor.seleccion(&self.texto).is_some() {
            return;
        }
        let p = self.pos();
        if let Some(r) = piezas.iter().find(|r| r.start < p && p < r.end) {
            self.cursor.poner(&self.texto, r.end, false);
        }
    }

    /// La rayita vuelve a encenderse ahora: al ganar el foco o tras tocarla.
    pub(crate) fn despertar(&mut self) {
        self.tocada = Instant::now();
    }

    fn guardar(&mut self, tipo: Edicion) {
        if tipo == Edicion::Otra || self.ultima != Some(tipo) {
            self.deshacer.push((self.texto.clone(), self.cursor));
            if self.deshacer.len() > DESHACER_MAX {
                self.deshacer.remove(0);
            }
        }
        self.rehacer.clear();
        self.ultima = Some(tipo);
    }

    /// Una letra tecleada (`WM_CHAR`). Los mandos que llegan como caracter
    /// (retroceso, Entrar, Ctrl+Retroceso = 0x7F, Ctrl+letra) no se
    /// escriben. Devuelve si el texto cambio.
    pub(crate) fn caracter(&mut self, c: char) -> bool {
        let mut b = [0u8; 4];
        let letra = &*c.encode_utf8(&mut b);
        if c.is_control()
            || self.filtro.is_some_and(|f| !f(c))
            || self.lo_que_cabe(letra).is_empty()
        {
            return false;
        }
        self.guardar(Edicion::Escribir);
        self.cursor.escribir(&mut self.texto, letra);
        self.despertar();
        true
    }

    /// Pega `s` donde esta la rayita, en una sola linea (y hasta el tope,
    /// si lo hay).
    pub(crate) fn pegar_texto(&mut self, s: &str) -> bool {
        let s = self.limpiar(s);
        let s = self.lo_que_cabe(&s).to_owned();
        if s.is_empty() {
            return false;
        }
        self.guardar(Edicion::Otra);
        self.cursor.escribir(&mut self.texto, &s);
        self.despertar();
        true
    }

    pub(crate) fn deshacer(&mut self) -> bool {
        let Some((t, c)) = self.deshacer.pop() else {
            return false;
        };
        self.rehacer
            .push((std::mem::replace(&mut self.texto, t), self.cursor));
        self.cursor = c;
        self.ultima = None;
        true
    }

    pub(crate) fn rehacer(&mut self) -> bool {
        let Some((t, c)) = self.rehacer.pop() else {
            return false;
        };
        self.deshacer
            .push((std::mem::replace(&mut self.texto, t), self.cursor));
        self.cursor = c;
        self.ultima = None;
        true
    }

    fn copiar(&self, pp: &mut dyn Portapapeles) {
        if let Some(s) = self.seleccionado() {
            pp.copiar(s);
        }
    }

    fn cortar(&mut self, pp: &mut dyn Portapapeles) -> bool {
        let Some(s) = self.seleccionado().map(str::to_owned) else {
            return false;
        };
        pp.copiar(&s);
        self.guardar(Edicion::Otra);
        self.cursor.borrar_atras(&mut self.texto, false);
        true
    }

    fn borrar(&mut self, atras: bool, palabra: bool) -> bool {
        let antes = self.texto.len();
        let tipo = if palabra {
            Edicion::Otra
        } else {
            Edicion::Borrar
        };
        // Se guarda solo si de verdad hay algo que borrar: un retroceso al
        // principio no deja un paso vacio en el deshacer.
        let hay = self.cursor.seleccion(&self.texto).is_some()
            || if atras {
                self.pos() > 0
            } else {
                self.pos() < self.texto.len()
            };
        if !hay {
            return false;
        }
        self.guardar(tipo);
        if atras {
            self.cursor.borrar_atras(&mut self.texto, palabra);
        } else {
            self.cursor.borrar_delante(&mut self.texto, palabra);
        }
        self.texto.len() != antes
    }

    /// Si la tecla es de una caja de texto: lo que [`Self::tecla`] atiende.
    /// Las ventanas lo miran en la guarda de su `match` para que lo demas
    /// (Escape, Entrar, Ctrl+F, el zoom) siga su camino.
    pub(crate) fn es_suya(vk: u32, shift: bool, ctrl: bool) -> bool {
        match vk {
            VK_IZQUIERDA | VK_DERECHA | VK_INICIO | VK_FIN | VK_RETROCESO | VK_SUPR => true,
            VK_INSERTAR => ctrl != shift,
            VK_A | VK_C | VK_X | VK_V | VK_Y | VK_Z => ctrl,
            _ => false,
        }
    }

    /// **Una tecla pulsada** (`WM_KEYDOWN`), con todo lo de una caja de
    /// Windows. Lo que no es suyo vuelve como [`Respuesta::NoEsMia`].
    pub(crate) fn tecla(
        &mut self,
        vk: u32,
        shift: bool,
        ctrl: bool,
        pp: &mut dyn Portapapeles,
    ) -> Respuesta {
        // Lo propio de la variante de varios renglones: Mayus+Intro parte
        // el renglon y arriba/abajo van de renglon en renglon (si no hay a
        // donde ir, la tecla vuelve a la ventana, que la usa para su lista).
        if self.varias {
            if vk == VK_ENTRAR && shift && !ctrl {
                let hecho = self.pegar_texto("\n");
                return if hecho {
                    Respuesta::Cambiada
                } else {
                    Respuesta::Movida
                };
            }
            if (vk == VK_ARRIBA || vk == VK_ABAJO) && !ctrl {
                if self.renglon(vk == VK_ABAJO, shift) {
                    self.ultima = None;
                    self.despertar();
                    return Respuesta::Movida;
                }
                return Respuesta::NoEsMia;
            }
        }
        if !Self::es_suya(vk, shift, ctrl) {
            return Respuesta::NoEsMia;
        }
        // En una linea, Inicio y Fin van al principio y al final de todo; en
        // varias, del renglon (con Ctrl, de todo), como en Windows.
        let todo = ctrl || !self.varias;
        let t = &self.texto;
        let r = match vk {
            VK_IZQUIERDA => {
                self.cursor.izquierda(t, shift, ctrl);
                Respuesta::Movida
            }
            VK_DERECHA => {
                self.cursor.derecha(t, shift, ctrl);
                Respuesta::Movida
            }
            VK_INICIO => {
                self.cursor.inicio(t, shift, todo);
                Respuesta::Movida
            }
            VK_FIN => {
                self.cursor.fin(t, shift, todo);
                Respuesta::Movida
            }
            VK_RETROCESO => self.si_cambio(|c| c.borrar(true, ctrl)),
            // Mayus+Supr es el «cortar» de siempre.
            VK_SUPR if shift && !ctrl => self.si_cambio(|c| c.cortar(pp)),
            VK_SUPR => self.si_cambio(|c| c.borrar(false, ctrl)),
            VK_INSERTAR if ctrl && !shift => {
                self.copiar(pp);
                Respuesta::Movida
            }
            VK_INSERTAR if shift && !ctrl => self.si_cambio(|c| match pp.leer_texto() {
                Some(s) => c.pegar_texto(&s),
                None => false,
            }),
            VK_A if ctrl => {
                self.cursor.todo(t);
                Respuesta::Movida
            }
            VK_C if ctrl => {
                self.copiar(pp);
                Respuesta::Movida
            }
            VK_X if ctrl => self.si_cambio(|c| c.cortar(pp)),
            VK_V if ctrl => self.si_cambio(|c| match pp.leer_texto() {
                Some(s) => c.pegar_texto(&s),
                None => false,
            }),
            VK_Z if ctrl && shift => self.si_cambio(Self::rehacer),
            VK_Z if ctrl => self.si_cambio(Self::deshacer),
            VK_Y if ctrl => self.si_cambio(Self::rehacer),
            _ => return Respuesta::NoEsMia,
        };
        if r == Respuesta::Movida {
            // Mover la rayita cierra el paso de deshacer que se teclea.
            self.ultima = None;
        }
        self.despertar();
        r
    }

    fn si_cambio(&mut self, f: impl FnOnce(&mut Self) -> bool) -> Respuesta {
        if f(self) {
            Respuesta::Cambiada
        } else {
            Respuesta::Movida
        }
    }

    // ----- El raton -----

    /// La letra bajo `l` (coordenadas de la ventana), si cae en la caja tal
    /// como se pinto la ultima vez.
    fn indice_en(&self, l: Punto) -> Option<usize> {
        let p = self.pintado.borrow();
        let z = p.zona?;
        let (x, y) = (l.x as f32, l.y as f32);
        let dentro = x >= z.x && y >= z.y && x < z.x + z.ancho && y < z.y + z.alto;
        dentro.then(|| self.indice_cerca_de(&p, l))
    }

    fn indice_cerca_de(&self, p: &Pintado, l: Punto) -> usize {
        if p.texto != self.texto {
            // Aun no se repinto con lo ultimo escrito: al final.
            return self.texto.len();
        }
        // En una linea la altura no importa: se pregunta por su centro.
        let y = if self.varias {
            l.y as f32 - p.origen.1
        } else {
            p.cajas.first().map_or(0.0, |(_, b)| b.y + b.alto / 2.0)
        };
        indice_en_punto(&self.texto, &p.cajas, l.x as f32 - p.origen.0, y)
    }

    /// Un renglon arriba o abajo, a la misma altura, preguntando a las cajas
    /// pintadas que letra hay ahi (como la caja de escribir del chat).
    /// Devuelve si se movio: en el primer renglon no hay arriba, ni abajo en
    /// el ultimo.
    fn renglon(&mut self, abajo: bool, shift: bool) -> bool {
        let i = {
            let p = self.pintado.borrow();
            if p.texto != self.texto || p.cajas.is_empty() {
                return false;
            }
            let (x, y, alto) = sitio_del_cursor(&self.texto, &p.cajas, self.pos(), p.tam * 1.3);
            let destino = if abajo {
                y + alto * 1.5
            } else {
                y - alto / 2.0
            };
            let fondo = p
                .cajas
                .iter()
                .map(|(_, b)| b.y + b.alto)
                .fold(0.0, f32::max);
            let ultimo_vacio = self.texto.ends_with('\n');
            if destino < 0.0 || (destino >= fondo && !(ultimo_vacio && y < fondo)) {
                return false;
            }
            indice_en_punto(&self.texto, &p.cajas, x, destino)
        };
        self.cursor.poner(&self.texto, i, shift);
        true
    }

    /// El punto cae en la caja (tal como se pinto).
    pub(crate) fn contiene(&self, l: Punto) -> bool {
        self.pintado.borrow().zona.is_some_and(|z| {
            let (x, y) = (l.x as f32, l.y as f32);
            x >= z.x && y >= z.y && x < z.x + z.ancho && y < z.y + z.alto
        })
    }

    /// **Boton pulsado** en `l`. Si cae en la caja: un clic pone la rayita
    /// (con Mayusculas, elige desde donde estaba), dos seguidos eligen la
    /// palabra y tres todo. Devuelve si el clic era de la caja.
    pub(crate) fn pulsar(&mut self, l: Punto, shift: bool, ahora: Instant) -> bool {
        let Some(i) = self.indice_en(l) else {
            return false;
        };
        let cuantos = match self.clic {
            Some((cuando, donde, n))
                if ahora.saturating_duration_since(cuando) <= DOBLE_CLIC
                    && (donde.x - l.x).abs() <= DOBLE_CLIC_HOLGURA
                    && (donde.y - l.y).abs() <= DOBLE_CLIC_HOLGURA =>
            {
                n.saturating_add(1)
            }
            _ => 1,
        };
        match cuantos {
            1 => {
                self.cursor.poner(&self.texto, i, shift);
                self.eligiendo = true;
            }
            2 => {
                let r = palabra_en(&self.texto, i);
                self.cursor.elegir(&self.texto, r);
                self.eligiendo = false;
            }
            _ => {
                self.cursor.todo(&self.texto);
                self.eligiendo = false;
            }
        }
        self.clic = Some((ahora, l, cuantos));
        self.ultima = None;
        self.tocada = ahora;
        true
    }

    /// El raton se mueve con el boton pulsado: elige hasta donde este,
    /// aunque se salga de la caja. Devuelve si estaba eligiendo.
    pub(crate) fn arrastrar(&mut self, l: Punto) -> bool {
        if !self.eligiendo {
            return false;
        }
        let i = {
            let p = self.pintado.borrow();
            self.indice_cerca_de(&p, l)
        };
        self.cursor.poner(&self.texto, i, true);
        self.despertar();
        true
    }

    /// Boton soltado. Devuelve si estaba eligiendo.
    pub(crate) fn soltar_raton(&mut self) -> bool {
        std::mem::take(&mut self.eligiendo)
    }

    pub(crate) fn eligiendo(&self) -> bool {
        self.eligiendo
    }

    // ----- El parpadeo -----

    /// Si la rayita se ve en este instante.
    pub(crate) fn rayita_visible(&self, ahora: Instant) -> bool {
        let t = ahora.saturating_duration_since(self.tocada);
        !self.parpadea || t >= PARPADEO_HASTA || (t.as_millis() / PARPADEO_MS) % 2 == 0
    }

    /// Dentro de cuanto cambia la rayita (para dormir el bucle hasta
    /// entonces), o `None` si ya no parpadea.
    pub(crate) fn ms_hasta_parpadeo(&self, ahora: Instant) -> Option<u32> {
        let t = ahora.saturating_duration_since(self.tocada);
        if !self.parpadea || t >= PARPADEO_HASTA {
            return None;
        }
        Some((PARPADEO_MS - t.as_millis() % PARPADEO_MS) as u32 + 1)
    }

    // ----- El pintado -----

    /// **Pinta la caja**: lo elegido, el texto (o la invitacion si esta
    /// vacia) y, con el foco, la rayita. `zona` es la caja entera, donde un
    /// clic es suyo; `hueco` es donde va el texto (sin el relleno ni el
    /// icono de la lupa), centrado en alto. Corre el texto para que la
    /// rayita siempre quede a la vista y apunta las cajas de las letras
    /// para el raton. El fondo lo pinta quien la usa: cada ventana el suyo.
    #[allow(clippy::too_many_arguments)] // la caja, donde, tamano, invitacion, colores, foco, hora
    pub(crate) fn pintar(
        &self,
        p: &Pintor,
        zona: RectF,
        hueco: RectF,
        tam: f32,
        invitacion: &str,
        colores: &Colores,
        enfocada: bool,
        ahora: Instant,
    ) {
        self.pintar_con_fichas(
            p, zona, hueco, tam, invitacion, colores, enfocada, ahora, None,
        );
    }

    /// Como [`Self::pintar`], con **fichas**: cada una como una chapa de su
    /// color detras de sus letras. Devuelve donde quedo la chapa de cada una
    /// (en el orden de `fichas.rangos`; la que no tiene letras, fuera de la
    /// ventana), para ensenar lo que es con el raton encima.
    #[allow(clippy::too_many_arguments)] // lo de `pintar` y las fichas
    pub(crate) fn pintar_con_fichas(
        &self,
        p: &Pintor,
        zona: RectF,
        hueco: RectF,
        tam: f32,
        invitacion: &str,
        colores: &Colores,
        enfocada: bool,
        ahora: Instant,
        fichas: Option<&Fichas<'_>>,
    ) -> Vec<RectF> {
        let mut chapas = Vec::new();
        let mut pin = self.pintado.borrow_mut();
        // En una linea no se parte nunca (ancho sin fin) y el texto se corre;
        // en varias se ajusta al ancho del hueco, como lo pinta DirectWrite.
        let ancho_disp = if self.varias {
            hueco.ancho.max(1.0)
        } else {
            f32::MAX
        };
        if pin.texto != self.texto || pin.tam != tam || pin.ancho != ancho_disp {
            pin.cajas = cajas_de_letras(&self.texto, |desde, largo| {
                p.cajas_de_trozo(&self.texto, tam, ancho_disp, &[], desde, largo)
                    .first()
                    .copied()
            });
            pin.texto = self.texto.clone();
            pin.tam = tam;
            pin.ancho = ancho_disp;
        }
        let (_, alto_linea) = p.medir_texto("Ág", tam);
        let grosor = (tam / 14.0).max(1.0).round();
        let (cx, cy, alto_rayita) =
            sitio_del_cursor(&self.texto, &pin.cajas, self.pos(), alto_linea);
        // En varias, lo que no cabe a lo alto se corre hacia arriba para
        // que la rayita siga a la vista (la misma cuenta que a lo ancho).
        let y = if self.varias {
            let largo = pin
                .cajas
                .iter()
                .map(|(_, b)| b.y + b.alto)
                .fold(0.0, f32::max);
            pin.corrido_y =
                super::corrido_a_la_vista(pin.corrido_y, cy, cy + alto_rayita, largo, hueco.alto);
            hueco.y - pin.corrido_y
        } else {
            pin.corrido_y = 0.0;
            hueco.y + (hueco.alto - alto_linea) / 2.0
        };
        if self.varias {
            pin.corrido = 0.0;
        } else {
            let largo = pin.cajas.last().map_or(0.0, |(_, b)| b.x + b.ancho);
            pin.corrido = corrido_para_ver(pin.corrido, cx, largo, hueco.ancho - grosor);
        }
        let ox = hueco.x - pin.corrido;
        pin.origen = (ox, y);
        pin.zona = Some(zona);

        p.empujar_recorte(hueco);
        if self.texto.is_empty() {
            if self.varias {
                p.texto_ajustado(invitacion, hueco.x, y, tam, ancho_disp, colores.invitacion);
            } else {
                p.texto_linea(invitacion, hueco.x, y, tam, hueco.ancho, colores.invitacion);
            }
        } else {
            // Las chapas de las fichas, detras de todo.
            if let Some(f) = fichas {
                let (mx, my) = (tam * 0.2, tam * 0.13);
                for r in f.rangos {
                    let mut letras = pin.cajas.iter().filter(|(i, _)| r.contains(i));
                    let Some((_, a)) = letras.next() else {
                        chapas.push(RectF {
                            x: -1.0e6,
                            y: -1.0e6,
                            ancho: 0.0,
                            alto: 0.0,
                        });
                        continue;
                    };
                    let b = letras.next_back().map_or(*a, |(_, b)| *b);
                    let chapa = RectF {
                        x: ox + a.x - mx,
                        y: y + a.y - my,
                        ancho: b.x + b.ancho - a.x + 2.0 * mx,
                        alto: a.alto.max(b.alto) + 2.0 * my,
                    };
                    p.rellenar_redondeado(chapa, f.radio, f.fondo);
                    chapas.push(chapa);
                }
            }
            if enfocada && let Some(r) = self.cursor.seleccion(&self.texto) {
                for (_, b) in pin.cajas.iter().filter(|(i, _)| r.contains(i)) {
                    p.rellenar(
                        RectF {
                            x: ox + b.x,
                            y: y + b.y,
                            // El salto de renglon no tiene ancho: un poco,
                            // para que se vea que tambien va elegido.
                            ancho: b.ancho.max(grosor * 3.0),
                            alto: b.alto,
                        },
                        colores.seleccion,
                    );
                }
            }
            match fichas.filter(|f| !f.rangos.is_empty() && !self.varias) {
                // Con fichas, a trozos: lo escrito en su color y cada ficha
                // en el suyo, cada trozo donde lo pone la disposicion entera.
                Some(f) => {
                    let mut rangos: Vec<_> = f.rangos.to_vec();
                    rangos.sort_by_key(|r| r.start);
                    let mut trozos: Vec<(std::ops::Range<usize>, Color)> = Vec::new();
                    let mut desde = 0;
                    for r in rangos.into_iter().filter(|r| r.end <= self.texto.len()) {
                        if r.start < desde {
                            continue;
                        }
                        trozos.push((desde..r.start, colores.texto));
                        desde = r.end;
                        trozos.push((r, f.letra));
                    }
                    trozos.push((desde..self.texto.len(), colores.texto));
                    for (r, color) in trozos {
                        let trozo = &self.texto[r.clone()];
                        if trozo.trim().is_empty() {
                            continue;
                        }
                        if let Some((_, b)) = pin.cajas.iter().find(|(i, _)| *i == r.start) {
                            p.texto(trozo, ox + b.x, y, tam, color);
                        }
                    }
                }
                None if self.varias => {
                    p.texto_ajustado(&self.texto, ox, y, tam, ancho_disp, colores.texto)
                }
                None => p.texto(&self.texto, ox, y, tam, colores.texto),
            }
        }
        if enfocada && self.rayita_visible(ahora) {
            p.rellenar(
                RectF {
                    x: ox + cx,
                    y: y + cy,
                    ancho: grosor,
                    alto: alto_rayita,
                },
                colores.rayita,
            );
        }
        p.soltar_recorte();
        chapas
    }

    /// Donde esta la rayita en la ventana, tal como se pinto: ahi se pone
    /// la ventanita del IME (japones, chino) para que se componga encima.
    pub(crate) fn sitio_ime(&self) -> Option<Punto> {
        let p = self.pintado.borrow();
        p.zona?;
        let alto = p.cajas.first().map_or(p.tam * 1.3, |(_, b)| b.alto);
        let (cx, cy, _) = sitio_del_cursor(&self.texto, &p.cajas, self.pos(), alto);
        Some(Punto {
            x: (p.origen.0 + cx) as i32,
            y: (p.origen.1 + cy) as i32,
        })
    }
}

/// Cuanto correr el texto para que la rayita (en `x` desde el principio del
/// texto) quede dentro de un hueco de `ancho`. Se corre lo justo, y si lo
/// escrito ya cabe (o se borro el final) se vuelve atras para no dejar
/// hueco vacio a la derecha. Es [`super::corrido_a_la_vista`] a lo ancho.
fn corrido_para_ver(antes: f32, x: f32, largo: f32, ancho: f32) -> f32 {
    super::corrido_a_la_vista(antes, x, x, largo, ancho)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un portapapeles de mentira: las pruebas no tocan el de verdad.
    #[derive(Default)]
    struct Pp(Option<String>);

    impl Portapapeles for Pp {
        fn leer_texto(&mut self) -> Option<String> {
            self.0.clone()
        }
        fn copiar(&mut self, t: &str) {
            self.0 = Some(t.to_owned());
        }
    }

    fn caja(t: &str) -> CajaLinea {
        CajaLinea::con_texto(t)
    }

    fn tecla(c: &mut CajaLinea, vk: u32, shift: bool, ctrl: bool) -> Respuesta {
        c.tecla(vk, shift, ctrl, &mut Pp::default())
    }

    fn ctrl_retroceso(c: &mut CajaLinea) -> Respuesta {
        tecla(c, VK_RETROCESO, false, true)
    }

    #[test]
    fn ctrl_retroceso_borra_la_palabra_entera() {
        let mut c = caja("buscar planos");
        assert_eq!(ctrl_retroceso(&mut c), Respuesta::Cambiada);
        assert_eq!(c.texto(), "buscar ");
        assert_eq!(ctrl_retroceso(&mut c), Respuesta::Cambiada);
        assert_eq!(c.texto(), "");
    }

    #[test]
    fn ctrl_retroceso_con_varios_espacios_se_lleva_los_blancos_y_la_palabra() {
        let mut c = caja("muro   norte");
        c.tecla(VK_IZQUIERDA, false, true, &mut Pp::default());
        assert_eq!(c.pos(), 7, "Ctrl+izquierda al principio de «norte»");
        ctrl_retroceso(&mut c);
        assert_eq!(c.texto(), "norte");
        assert_eq!(c.pos(), 0);
    }

    #[test]
    fn ctrl_retroceso_con_signos_va_por_trozos() {
        let mut c = caja("hola, mundo!!");
        ctrl_retroceso(&mut c);
        assert_eq!(c.texto(), "hola, mundo", "los signos son su propia palabra");
        ctrl_retroceso(&mut c);
        assert_eq!(c.texto(), "hola, ");
        ctrl_retroceso(&mut c);
        assert_eq!(c.texto(), "hola");
    }

    #[test]
    fn ctrl_retroceso_con_tildes_y_enes() {
        let mut c = caja("canción del añoñaño");
        ctrl_retroceso(&mut c);
        assert_eq!(c.texto(), "canción del ");
        ctrl_retroceso(&mut c);
        assert_eq!(c.texto(), "canción ");
        ctrl_retroceso(&mut c);
        assert_eq!(c.texto(), "");
    }

    #[test]
    fn ctrl_retroceso_al_principio_no_borra_nada() {
        let mut c = caja("plano");
        tecla(&mut c, VK_INICIO, false, false);
        assert_eq!(ctrl_retroceso(&mut c), Respuesta::Movida);
        assert_eq!(c.texto(), "plano");
        // Y no deja un paso vacio que deshacer.
        assert_eq!(tecla(&mut c, VK_Z, false, true), Respuesta::Movida);
        let mut vacia = CajaLinea::nueva();
        assert_eq!(ctrl_retroceso(&mut vacia), Respuesta::Movida);
        assert!(ctrl_retroceso(&mut vacia).atendida());
    }

    #[test]
    fn el_0x7f_de_ctrl_retroceso_y_los_mandos_no_se_escriben() {
        let mut c = caja("ab");
        // Windows manda WM_CHAR 0x7F tras Ctrl+Retroceso, y 0x08, 0x0D...
        for m in ['\u{7f}', '\u{8}', '\r', '\n', '\u{1}', '\t'] {
            assert!(!c.caracter(m), "{m:?}");
        }
        assert_eq!(c.texto(), "ab");
    }

    #[test]
    fn ctrl_supr_borra_la_palabra_de_delante() {
        let mut c = caja("uno dos tres");
        tecla(&mut c, VK_INICIO, false, false);
        assert_eq!(tecla(&mut c, VK_SUPR, false, true), Respuesta::Cambiada);
        assert_eq!(c.texto(), "dos tres");
        tecla(&mut c, VK_FIN, false, false);
        assert_eq!(
            tecla(&mut c, VK_SUPR, false, true),
            Respuesta::Movida,
            "al final no hay nada delante"
        );
        assert_eq!(c.texto(), "dos tres");
    }

    #[test]
    fn retroceso_y_supr_de_una_letra() {
        let mut c = caja("año");
        tecla(&mut c, VK_IZQUIERDA, false, false);
        tecla(&mut c, VK_RETROCESO, false, false);
        assert_eq!(c.texto(), "ao", "la ñ (dos bytes) se va entera");
        tecla(&mut c, VK_SUPR, false, false);
        assert_eq!(c.texto(), "a");
    }

    #[test]
    fn ctrl_flechas_saltan_palabras_e_inicio_fin() {
        let mut c = caja("foto del pilar");
        tecla(&mut c, VK_IZQUIERDA, false, true);
        assert_eq!(c.pos(), 9);
        tecla(&mut c, VK_IZQUIERDA, false, true);
        assert_eq!(c.pos(), 5);
        tecla(&mut c, VK_DERECHA, false, true);
        assert_eq!(c.pos(), 9);
        tecla(&mut c, VK_INICIO, false, false);
        assert_eq!(c.pos(), 0);
        tecla(&mut c, VK_FIN, false, false);
        assert_eq!(c.pos(), c.len());
        // Una flecha que no es de la caja de una linea.
        assert_eq!(tecla(&mut c, 0x26, false, false), Respuesta::NoEsMia);
        assert_eq!(
            tecla(&mut c, 0x1B, false, false),
            Respuesta::NoEsMia,
            "Escape es de la ventana"
        );
        assert_eq!(
            tecla(&mut c, 0x0D, false, false),
            Respuesta::NoEsMia,
            "Entrar tambien"
        );
    }

    #[test]
    fn mayus_flechas_eligen_y_escribir_sustituye() {
        let mut c = caja("plano viejo");
        tecla(&mut c, VK_IZQUIERDA, true, true);
        assert_eq!(c.seleccionado(), Some("viejo"));
        tecla(&mut c, VK_IZQUIERDA, true, false);
        assert_eq!(c.seleccionado(), Some(" viejo"));
        for l in " nuevo".chars() {
            c.caracter(l);
        }
        assert_eq!(c.texto(), "plano nuevo");
        tecla(&mut c, VK_INICIO, true, false);
        assert_eq!(c.seleccionado(), Some("plano nuevo"));
        // Sin Mayusculas, la flecha suelta la seleccion por su lado.
        tecla(&mut c, VK_DERECHA, false, false);
        assert_eq!((c.pos(), c.seleccionado()), (c.len(), None));
    }

    #[test]
    fn ctrl_a_c_x_v() {
        let mut pp = Pp::default();
        let mut c = caja("pilar");
        assert_eq!(c.tecla(VK_A, false, true, &mut pp), Respuesta::Movida);
        assert_eq!(c.seleccionado(), Some("pilar"));
        c.tecla(VK_C, false, true, &mut pp);
        assert_eq!(pp.0.as_deref(), Some("pilar"));
        assert_eq!(c.texto(), "pilar", "copiar no cambia nada");
        assert_eq!(c.tecla(VK_X, false, true, &mut pp), Respuesta::Cambiada);
        assert_eq!(c.texto(), "");
        assert_eq!(c.tecla(VK_V, false, true, &mut pp), Respuesta::Cambiada);
        assert_eq!(c.tecla(VK_V, false, true, &mut pp), Respuesta::Cambiada);
        assert_eq!(c.texto(), "pilarpilar");
        // Sin nada elegido, cortar no hace nada (ni toca el portapapeles).
        pp.0 = Some("otro".into());
        assert_eq!(c.tecla(VK_X, false, true, &mut pp), Respuesta::Movida);
        assert_eq!(pp.0.as_deref(), Some("otro"));
        // Pegar con el portapapeles vacio tampoco.
        let mut vacio = Pp::default();
        assert_eq!(c.tecla(VK_V, false, true, &mut vacio), Respuesta::Movida);
    }

    #[test]
    fn insertar_y_mayus_supr_son_copiar_pegar_y_cortar() {
        let mut pp = Pp::default();
        let mut c = caja("losa");
        c.elegir_todo();
        c.tecla(VK_INSERTAR, false, true, &mut pp);
        assert_eq!(pp.0.as_deref(), Some("losa"));
        c.tecla(VK_SUPR, true, false, &mut pp);
        assert_eq!(c.texto(), "");
        c.tecla(VK_INSERTAR, true, false, &mut pp);
        assert_eq!(c.texto(), "losa");
    }

    #[test]
    fn pegar_quita_los_saltos_de_renglon() {
        let mut pp = Pp(Some(
            "primera linea\r\n\r\n   segunda\tcon tab\nfin\n".into(),
        ));
        let mut c = caja("> ");
        c.tecla(VK_V, false, true, &mut pp);
        assert_eq!(c.texto(), "> primera linea segunda con tab fin");
        assert_eq!(en_una_linea("  sin saltos  "), "  sin saltos  ");
        assert_eq!(en_una_linea("\n\n"), "");
    }

    #[test]
    fn deshacer_y_rehacer() {
        let mut c = CajaLinea::nueva();
        for l in "hola".chars() {
            c.caracter(l);
        }
        tecla(&mut c, VK_IZQUIERDA, false, false);
        c.caracter('X');
        assert_eq!(c.texto(), "holXa");
        // Lo tecleado seguido es un solo paso.
        assert_eq!(tecla(&mut c, VK_Z, false, true), Respuesta::Cambiada);
        assert_eq!(c.texto(), "hola");
        assert_eq!(tecla(&mut c, VK_Z, false, true), Respuesta::Cambiada);
        assert_eq!(c.texto(), "");
        assert_eq!(
            tecla(&mut c, VK_Z, false, true),
            Respuesta::Movida,
            "nada mas que deshacer"
        );
        assert_eq!(tecla(&mut c, VK_Y, false, true), Respuesta::Cambiada);
        assert_eq!(c.texto(), "hola");
        assert_eq!(
            tecla(&mut c, VK_Z, true, true),
            Respuesta::Cambiada,
            "Ctrl+Mayus+Z rehace"
        );
        assert_eq!(c.texto(), "holXa");
        assert_eq!(c.pos(), 4);
        ctrl_retroceso(&mut c);
        assert_eq!(c.texto(), "a");
        tecla(&mut c, VK_Z, false, true);
        assert_eq!(c.texto(), "holXa");
        // Escribir tras deshacer tira lo que se podia rehacer.
        c.caracter('!');
        assert_eq!(tecla(&mut c, VK_Y, false, true), Respuesta::Movida);
    }

    #[test]
    fn tildes_y_lo_del_ime_entran_como_letras() {
        let mut c = CajaLinea::nueva();
        for l in "señal ácida 東京 😀".chars() {
            c.caracter(l);
        }
        assert_eq!(c.texto(), "señal ácida 東京 😀");
        tecla(&mut c, VK_IZQUIERDA, false, false);
        tecla(&mut c, VK_RETROCESO, false, false);
        assert_eq!(c.texto(), "señal ácida 東京😀");
    }

    /// Cajas de 10 de ancho, como si las diera DirectWrite.
    fn pintada(t: &str) -> CajaLinea {
        let c = caja(t);
        {
            let mut p = c.pintado.borrow_mut();
            p.texto = t.to_owned();
            p.cajas = t
                .char_indices()
                .enumerate()
                .map(|(n, (i, _))| {
                    (
                        i,
                        RectF {
                            x: n as f32 * 10.0,
                            y: 0.0,
                            ancho: 10.0,
                            alto: 20.0,
                        },
                    )
                })
                .collect();
            p.zona = Some(RectF {
                x: 100.0,
                y: 50.0,
                ancho: 200.0,
                alto: 30.0,
            });
            p.origen = (100.0, 55.0);
        }
        c
    }

    fn en(x: i32) -> Punto {
        Punto { x, y: 60 }
    }

    #[test]
    fn clic_doble_y_triple() {
        let mut c = pintada("foto del pilar");
        let t0 = Instant::now();
        // Clic en la mitad derecha de la «e» de «del» (x 160..170).
        assert!(c.pulsar(en(166), false, t0));
        assert_eq!((c.pos(), c.seleccionado()), (7, None));
        assert!(c.soltar_raton());
        assert!(c.pulsar(en(166), false, t0 + Duration::from_millis(200)));
        assert_eq!(c.seleccionado(), Some("del"));
        assert!(c.pulsar(en(167), false, t0 + Duration::from_millis(400)));
        assert_eq!(c.seleccionado(), Some("foto del pilar"));
        // Tarde ya no cuenta como otro clic seguido.
        assert!(c.pulsar(en(166), false, t0 + Duration::from_secs(3)));
        assert_eq!(c.seleccionado(), None);
    }

    #[test]
    fn arrastrar_elige_y_mayus_clic_extiende() {
        let mut c = pintada("foto del pilar");
        let t0 = Instant::now();
        c.pulsar(en(100), false, t0);
        assert!(c.arrastrar(en(141)));
        assert_eq!(c.seleccionado(), Some("foto"));
        // Saliendose de la caja por la derecha sigue eligiendo hasta el final.
        c.arrastrar(Punto { x: 900, y: 300 });
        assert_eq!(c.seleccionado(), Some("foto del pilar"));
        c.soltar_raton();
        assert!(!c.arrastrar(en(120)), "sin boton no elige");
        c.pulsar(en(100), false, t0 + Duration::from_secs(2));
        c.soltar_raton();
        c.pulsar(en(151), true, t0 + Duration::from_secs(4));
        assert_eq!(c.seleccionado(), Some("foto "));
    }

    #[test]
    fn un_clic_fuera_no_es_de_la_caja() {
        let mut c = pintada("hola");
        assert!(!c.pulsar(Punto { x: 50, y: 60 }, false, Instant::now()));
        assert!(!c.pulsar(Punto { x: 150, y: 10 }, false, Instant::now()));
        assert!(!c.contiene(Punto { x: 99, y: 60 }));
        assert!(c.contiene(Punto { x: 299, y: 79 }));
        // Sin pintar aun, no hay caja donde pulsar.
        let mut sin = caja("hola");
        assert!(!sin.pulsar(en(110), false, Instant::now()));
    }

    #[test]
    fn la_rayita_parpadea_y_luego_se_queda_quieta() {
        let c = CajaLinea::nueva();
        let t = c.tocada;
        assert!(c.rayita_visible(t));
        assert!(!c.rayita_visible(t + Duration::from_millis(600)));
        assert!(c.rayita_visible(t + Duration::from_millis(1100)));
        assert!(c.ms_hasta_parpadeo(t + Duration::from_millis(100)).unwrap() <= 431);
        // A los cinco segundos, encendida y sin despertar a nadie.
        assert!(c.rayita_visible(t + Duration::from_millis(5300)));
        assert_eq!(c.ms_hasta_parpadeo(t + Duration::from_secs(6)), None);
    }

    #[test]
    fn el_texto_se_corre_para_que_se_vea_la_rayita() {
        // Cabe todo: sin correr.
        assert_eq!(corrido_para_ver(0.0, 50.0, 80.0, 200.0), 0.0);
        // La rayita al final de un texto largo: se corre lo justo.
        assert_eq!(corrido_para_ver(0.0, 500.0, 500.0, 200.0), 300.0);
        // Volviendo al principio, se corre hacia atras.
        assert_eq!(corrido_para_ver(300.0, 20.0, 500.0, 200.0), 20.0);
        // En medio y ya a la vista: quieto.
        assert_eq!(corrido_para_ver(300.0, 400.0, 500.0, 200.0), 300.0);
        // Se borro el final: no queda hueco vacio a la derecha.
        assert_eq!(corrido_para_ver(300.0, 250.0, 250.0, 200.0), 50.0);
    }

    #[test]
    fn se_lee_como_un_str() {
        let c = caja("Plano");
        assert!(c.to_lowercase().contains("pla"));
        assert!(c == "Plano");
        let f = |s: &str| s.len();
        assert_eq!(f(&c), 5);
    }

    /// Cajas de 10 de ancho y renglones de 20, partiendo en cada salto: lo
    /// que daria DirectWrite con un ancho de sobra.
    fn pintada_en_varias(t: &str) -> CajaLinea {
        let mut c = CajaLinea::nueva().en_varias_lineas();
        c.poner_texto(t);
        {
            let mut p = c.pintado.borrow_mut();
            p.texto = t.to_owned();
            p.tam = 15.0;
            let (mut x, mut y) = (0.0, 0.0);
            p.cajas = t
                .char_indices()
                .map(|(i, ch)| {
                    let b = RectF {
                        x,
                        y,
                        ancho: if ch == '\n' { 0.0 } else { 10.0 },
                        alto: 20.0,
                    };
                    if ch == '\n' {
                        x = 0.0;
                        y += 20.0;
                    } else {
                        x += 10.0;
                    }
                    (i, b)
                })
                .collect();
            p.zona = Some(RectF {
                x: 0.0,
                y: 0.0,
                ancho: 300.0,
                alto: 100.0,
            });
        }
        c
    }

    #[test]
    fn en_varias_lineas_mayus_intro_parte_y_las_flechas_van_por_renglones() {
        let mut c = pintada_en_varias("uno\ndos");
        // Pegar conserva los saltos (en una linea se harian espacios).
        let mut pp = Pp(Some("a\r\nb".into()));
        tecla(&mut c, VK_INICIO, false, true);
        c.tecla(VK_V, false, true, &mut pp);
        assert_eq!(c.texto(), "a\nbuno\ndos");
        // Mayus+Intro parte el renglon; Intro solo no es de la caja.
        let mut d = pintada_en_varias("uno\ndos");
        assert_eq!(tecla(&mut d, VK_ENTRAR, false, false), Respuesta::NoEsMia);
        // Inicio y Fin van al principio y al final del renglon.
        tecla(&mut d, VK_INICIO, false, false);
        assert_eq!(d.pos(), 4);
        // Arriba sube al renglon de encima a la misma altura...
        tecla(&mut d, VK_DERECHA, false, false);
        assert_eq!(tecla(&mut d, VK_ARRIBA, false, false), Respuesta::Movida);
        assert_eq!(d.pos(), 1);
        // ...y en el primero no hay arriba: la tecla vuelve a la ventana.
        assert_eq!(tecla(&mut d, VK_ARRIBA, false, false), Respuesta::NoEsMia);
        assert_eq!(tecla(&mut d, VK_ABAJO, false, false), Respuesta::Movida);
        assert_eq!(d.pos(), 5);
        assert_eq!(tecla(&mut d, VK_ENTRAR, true, false), Respuesta::Cambiada);
        assert_eq!(d.texto(), "uno\nd\nos");
        // Caso negativo: en una linea, arriba no es de la caja.
        let mut una = caja("uno");
        assert_eq!(tecla(&mut una, VK_ARRIBA, false, false), Respuesta::NoEsMia);
        assert_eq!(tecla(&mut una, VK_ENTRAR, true, false), Respuesta::NoEsMia);
    }

    #[test]
    fn el_filtro_deja_solo_lo_que_admite_tecleado_y_pegado() {
        let mut c = CajaLinea::nueva()
            .con_maximo(5)
            .con_filtro(|c| c.is_ascii_digit() || c == ':');
        for l in "1a8:3b0x".chars() {
            c.caracter(l);
        }
        assert_eq!(c.texto(), "18:30");
        c.vaciar();
        let mut pp = Pp(Some("las 9:45 h".into()));
        c.tecla(VK_V, false, true, &mut pp);
        assert_eq!(c.texto(), "9:45");
        // Caso negativo: una letra no entra.
        assert!(!c.caracter('h'));
    }

    #[test]
    fn sin_parpadeo_la_rayita_siempre_se_ve() {
        let c = CajaLinea::nueva().sin_parpadeo();
        let t = c.tocada;
        assert!(c.rayita_visible(t + Duration::from_millis(600)));
        assert_eq!(c.ms_hasta_parpadeo(t + Duration::from_millis(100)), None);
    }

    #[test]
    fn con_tope_no_entra_mas_ni_tecleado_ni_pegado() {
        let mut c = CajaLinea::nueva().con_maximo(5);
        for l in "abcdefg".chars() {
            c.caracter(l);
        }
        assert_eq!(c.texto(), "abcde");
        // Caso negativo: llena, ni una letra mas.
        assert!(!c.caracter('z'));
        // Sobre lo elegido si cabe: lo sustituye.
        c.elegir_todo();
        assert!(c.caracter('x'));
        assert_eq!(c.texto(), "x");
        let mut pp = Pp(Some("123456789".into()));
        assert_eq!(c.tecla(VK_V, false, true, &mut pp), Respuesta::Cambiada);
        assert_eq!(c.texto(), "x1234");
        assert_eq!(CajaLinea::con_texto("abcdefg").con_maximo(3).texto(), "abc");
    }

    /// Lo que las ventanas miran en la guarda (`es_suya`) es justo lo que
    /// la caja atiende: si no, una tecla se perderia o se colaria a otra.
    #[test]
    fn es_suya_dice_lo_mismo_que_tecla() {
        for vk in 0..=0xFFu32 {
            for (shift, ctrl) in [(false, false), (true, false), (false, true), (true, true)] {
                let mut c = caja("uno dos");
                assert_eq!(
                    CajaLinea::es_suya(vk, shift, ctrl),
                    tecla(&mut c, vk, shift, ctrl).atendida(),
                    "vk {vk:#x} shift {shift} ctrl {ctrl}"
                );
            }
        }
        // Ctrl+F (abrir la busqueda) y el zoom son de la ventana.
        assert!(!CajaLinea::es_suya(0x46, false, true));
        assert!(!CajaLinea::es_suya(0xBB, false, true));
    }

    /// **La caja pintada**, para mirarla: un texto largo que no cabe, con la
    /// rayita en medio (el texto se corre para que se vea) y, debajo, la
    /// misma caja con una palabra elegida y otra vacia con su invitacion.
    ///
    /// ```text
    /// cargo test -p pixpin --bin pixpinmax muestra_de_la_caja_de_una_linea -- --ignored --nocapture
    /// ```
    /// Deja el PNG en `PIXPIN_MUESTRAS` o en `%TEMP%/pixpin-muestras-caja`.
    #[test]
    fn las_piezas_se_borran_y_se_saltan_enteras() {
        let mut c = caja("regar [img 01] hoy");
        let ficha = 6..14;
        let piezas = std::slice::from_ref(&ficha);
        let mut pp = Pp::default();
        // Al final de la ficha: la flecha la salta de una vez.
        c.ir_a(14);
        assert_eq!(
            c.tecla_con_piezas(VK_IZQUIERDA, false, false, &mut pp, piezas),
            Respuesta::Movida
        );
        assert_eq!(c.pos(), 6);
        assert_eq!(
            c.tecla_con_piezas(VK_DERECHA, false, false, &mut pp, piezas),
            Respuesta::Movida
        );
        assert_eq!(c.pos(), 14);
        // Con Mayusculas la elige entera.
        c.tecla_con_piezas(VK_IZQUIERDA, true, false, &mut pp, piezas);
        assert_eq!(c.seleccionado(), Some("[img 01]"));
        c.tecla_con_piezas(VK_DERECHA, false, false, &mut pp, piezas);
        assert_eq!(c.pos(), 14, "sin Mayusculas la flecha suelta lo elegido");
        // Retroceso (y Ctrl+Retroceso) se la llevan entera, y se deshace.
        assert_eq!(
            c.tecla_con_piezas(VK_RETROCESO, false, true, &mut pp, piezas),
            Respuesta::Cambiada
        );
        assert_eq!(c.texto(), "regar  hoy");
        assert_eq!(c.pos(), 6);
        assert!(c.hay_deshacer());
        tecla(&mut c, VK_Z, false, true);
        assert_eq!(c.texto(), "regar [img 01] hoy");
        // Supr al principio de la ficha, igual.
        c.ir_a(6);
        assert_eq!(
            c.tecla_con_piezas(VK_SUPR, false, false, &mut pp, piezas),
            Respuesta::Cambiada
        );
        assert_eq!(c.texto(), "regar  hoy");
        // Caso negativo: lejos de la ficha, una letra y nada mas.
        let mut d = caja("regar [img 01] hoy");
        assert_eq!(
            d.tecla_con_piezas(VK_RETROCESO, false, false, &mut pp, piezas),
            Respuesta::Cambiada
        );
        assert_eq!(d.texto(), "regar [img 01] ho");
        // Y sin piezas es la tecla de siempre: Ctrl+Retroceso parte la ficha.
        let mut e = caja("regar [img 01]");
        e.tecla_con_piezas(VK_RETROCESO, false, true, &mut pp, &[]);
        assert_eq!(e.texto(), "regar [img 01");
    }

    #[test]
    fn un_clic_dentro_de_una_pieza_deja_la_rayita_al_final() {
        let mut c = pintada("ver [img 01] ya");
        assert!(c.pulsar(en(100 + 7 * 10 + 2), false, Instant::now()));
        assert_eq!(c.pos(), 7, "el clic cae dentro de la ficha");
        c.soltar_raton();
        c.fuera_de_piezas(std::slice::from_ref(&(4..12)));
        assert_eq!(c.pos(), 12);
        // Caso negativo: fuera de la ficha no se mueve.
        c.ir_a(2);
        c.fuera_de_piezas(std::slice::from_ref(&(4..12)));
        assert_eq!(c.pos(), 2);
    }

    #[test]
    #[ignore = "necesita GPU; genera un PNG para mirarlo"]
    fn muestra_de_la_caja_de_una_linea() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;

        let d3 = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d3.d3d()).expect("motor");
        let (ancho, alto) = (520u32, 320u32);
        let destino = FueraDePantalla::nuevo(&motor, d3.d3d(), ancho, alto).expect("destino");
        let fondo = Color {
            r: 0.09,
            g: 0.13,
            b: 0.17,
            a: 1.0,
        };
        let campo = Color {
            r: 0.17,
            g: 0.22,
            b: 0.28,
            a: 1.0,
        };
        let colores = Colores {
            texto: Color {
                r: 0.95,
                g: 0.95,
                b: 0.95,
                a: 1.0,
            },
            invitacion: Color {
                r: 0.55,
                g: 0.6,
                b: 0.65,
                a: 1.0,
            },
            seleccion: Color {
                r: 0.33,
                g: 0.6,
                b: 0.95,
                a: 0.35,
            },
            rayita: Color {
                r: 0.95,
                g: 0.95,
                b: 0.95,
                a: 1.0,
            },
        };
        // La rayita en medio de un texto mas largo que la caja.
        let mut larga = caja(
            "planos de la obra Miraflores, revision del muro norte y las vigas del segundo piso",
        );
        for _ in 0..30 {
            tecla(&mut larga, VK_IZQUIERDA, false, true);
        }
        for _ in 0..9 {
            tecla(&mut larga, VK_DERECHA, false, true);
        }
        // Otra con «Miraflores» elegida con Mayus+Ctrl+flecha.
        let mut elegida = caja("buscar Miraflores");
        tecla(&mut elegida, VK_IZQUIERDA, true, true);
        let vacia = CajaLinea::nueva();
        // Y la variante de varios renglones (las lecciones, la timeline):
        // el texto se ajusta al ancho y la rayita va en medio.
        let mut varias = CajaLinea::nueva().en_varias_lineas();
        varias.poner_texto("Revisar el encofrado antes del vaciado\nla losa del segundo piso");
        tecla(&mut varias, VK_INICIO, false, false);
        tecla(&mut varias, VK_DERECHA, false, true);
        let ahora = larga.tocada;
        let filas = [(&larga, ""), (&elegida, ""), (&vacia, "Buscar en el chat")];
        // Dos vueltas: la primera apunta las cajas de las letras y la
        // segunda corre el texto con ellas, como al teclear de verdad.
        for _ in 0..2 {
            motor
                .dibujar(&destino.destino, |p| {
                    p.rellenar(
                        RectF {
                            x: 0.0,
                            y: 0.0,
                            ancho: ancho as f32,
                            alto: alto as f32,
                        },
                        fondo,
                    );
                    for (n, (c, invitacion)) in filas.iter().enumerate() {
                        let zona = RectF {
                            x: 20.0,
                            y: 20.0 + n as f32 * 60.0,
                            ancho: 300.0,
                            alto: 40.0,
                        };
                        p.rellenar_redondeado(zona, 20.0, campo);
                        let hueco = RectF {
                            x: zona.x + 16.0,
                            ancho: zona.ancho - 32.0,
                            ..zona
                        };
                        c.pintar(p, zona, hueco, 15.0, invitacion, &colores, true, ahora);
                    }
                    let zona = RectF {
                        x: 20.0,
                        y: 200.0,
                        ancho: 300.0,
                        alto: 100.0,
                    };
                    p.rellenar_redondeado(zona, 12.0, campo);
                    let hueco = RectF {
                        x: 36.0,
                        y: 210.0,
                        ancho: 268.0,
                        alto: 80.0,
                    };
                    varias.pintar(p, zona, hueco, 15.0, "", &colores, true, ahora);
                })
                .expect("fotograma");
            destino.esperar_gpu().expect("GPU");
        }
        let (w, h, pixeles) = destino.leer_rgba().expect("leer");
        // La rayita esta en medio y a la vista: dentro del hueco y corrida.
        let p = larga.pintado.borrow();
        assert!(p.corrido > 0.0, "el texto largo se corre");
        let (cx, _, _) = sitio_del_cursor(&larga.texto, &p.cajas, larga.pos(), 20.0);
        let x = p.origen.0 + cx;
        assert!((36.0..=304.0).contains(&x), "rayita a la vista: {x}");
        assert!(
            larga.pos() > 0 && larga.pos() < larga.len(),
            "en medio del texto"
        );
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
            ancho: w,
            alto: h,
            pixeles,
        })
        .expect("codificar");
        let salida = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("pixpin-muestras-caja"));
        std::fs::create_dir_all(&salida).unwrap();
        let ruta = salida.join("caja-de-una-linea.png");
        std::fs::write(&ruta, png).unwrap();
        println!("caja: {}", ruta.display());
    }
}
