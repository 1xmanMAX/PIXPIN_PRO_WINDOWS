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

use std::cell::RefCell;
use std::time::{Duration, Instant};

use pixpin_geom::Punto;
use pixpin_render::{Color, Pintor, RectF};

use super::{Cursor, cajas_de_letras, indice_en_punto, palabra_en, sitio_del_cursor};

const VK_RETROCESO: u32 = 0x08;
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
    /// Cuanto se corre el texto a la izquierda para que se vea la rayita.
    corrido: f32,
}

/// **Una caja de texto de una linea.** Se lee como un `&str` (`Deref`), asi
/// que filtrar con ella es igual que con el `String` de antes.
#[derive(Debug, Clone)]
pub(crate) struct CajaLinea {
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

    pub(crate) fn texto(&self) -> &str {
        &self.texto
    }

    /// Cambia todo lo escrito desde fuera (abrir la caja con un nombre ya
    /// puesto): la rayita al final y el deshacer en blanco.
    pub(crate) fn poner_texto(&mut self, t: &str) {
        self.texto = en_una_linea(t);
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

    /// Suelta lo elegido dejando la rayita donde estaba. Devuelve si habia
    /// algo elegido: Escape lo usa para soltar antes de cerrar.
    pub(crate) fn soltar_seleccion(&mut self) -> bool {
        if self.cursor.seleccion(&self.texto).is_none() {
            return false;
        }
        let p = self.pos();
        self.cursor.poner(&self.texto, p, false);
        true
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
        if c.is_control() || self.lo_que_cabe(letra).is_empty() {
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
        let s = en_una_linea(s);
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
        if !Self::es_suya(vk, shift, ctrl) {
            return Respuesta::NoEsMia;
        }
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
                self.cursor.inicio(t, shift, true);
                Respuesta::Movida
            }
            VK_FIN => {
                self.cursor.fin(t, shift, true);
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
        // Una sola linea: la altura no importa, se pregunta por su centro.
        let y = p.cajas.first().map_or(0.0, |(_, b)| b.y + b.alto / 2.0);
        indice_en_punto(&self.texto, &p.cajas, l.x as f32 - p.origen.0, y)
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
        t >= PARPADEO_HASTA || (t.as_millis() / PARPADEO_MS) % 2 == 0
    }

    /// Dentro de cuanto cambia la rayita (para dormir el bucle hasta
    /// entonces), o `None` si ya no parpadea.
    pub(crate) fn ms_hasta_parpadeo(&self, ahora: Instant) -> Option<u32> {
        let t = ahora.saturating_duration_since(self.tocada);
        if t >= PARPADEO_HASTA {
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
        let mut pin = self.pintado.borrow_mut();
        if pin.texto != self.texto || pin.tam != tam {
            pin.cajas = cajas_de_letras(&self.texto, |desde, largo| {
                p.cajas_de_trozo(&self.texto, tam, f32::MAX, &[], desde, largo)
                    .first()
                    .copied()
            });
            pin.texto = self.texto.clone();
            pin.tam = tam;
        }
        let (_, alto_linea) = p.medir_texto("Ág", tam);
        let y = hueco.y + (hueco.alto - alto_linea) / 2.0;
        let grosor = (tam / 14.0).max(1.0).round();
        let (cx, cy, alto_rayita) =
            sitio_del_cursor(&self.texto, &pin.cajas, self.pos(), alto_linea);
        let largo = pin.cajas.last().map_or(0.0, |(_, b)| b.x + b.ancho);
        pin.corrido = corrido_para_ver(pin.corrido, cx, largo, hueco.ancho - grosor);
        let ox = hueco.x - pin.corrido;
        pin.origen = (ox, y);
        pin.zona = Some(zona);

        p.empujar_recorte(hueco);
        if self.texto.is_empty() {
            p.texto_linea(invitacion, hueco.x, y, tam, hueco.ancho, colores.invitacion);
        } else {
            if enfocada && let Some(r) = self.cursor.seleccion(&self.texto) {
                for (_, b) in pin.cajas.iter().filter(|(i, _)| r.contains(i)) {
                    p.rellenar(
                        RectF {
                            x: ox + b.x,
                            y: y + b.y,
                            ..*b
                        },
                        colores.seleccion,
                    );
                }
            }
            p.texto(&self.texto, ox, y, tam, colores.texto);
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
/// hueco vacio a la derecha.
fn corrido_para_ver(antes: f32, x: f32, largo: f32, ancho: f32) -> f32 {
    let ancho = ancho.max(1.0);
    let mut c = antes;
    if x - c > ancho {
        c = x - ancho;
    }
    if x < c {
        c = x;
    }
    // Sin hueco a la derecha de lo escrito si se puede evitar.
    c.min((largo.max(x) - ancho).max(0.0)).max(0.0)
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
        assert_eq!(tecla(&mut c, VK_SUPR, false, true), Respuesta::Movida, "al final no hay nada delante");
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
        assert_eq!(tecla(&mut c, 0x1B, false, false), Respuesta::NoEsMia, "Escape es de la ventana");
        assert_eq!(tecla(&mut c, 0x0D, false, false), Respuesta::NoEsMia, "Entrar tambien");
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
        let mut pp = Pp(Some("primera linea\r\n\r\n   segunda\tcon tab\nfin\n".into()));
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
        assert_eq!(tecla(&mut c, VK_Z, false, true), Respuesta::Movida, "nada mas que deshacer");
        assert_eq!(tecla(&mut c, VK_Y, false, true), Respuesta::Cambiada);
        assert_eq!(c.texto(), "hola");
        assert_eq!(tecla(&mut c, VK_Z, true, true), Respuesta::Cambiada, "Ctrl+Mayus+Z rehace");
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
}
