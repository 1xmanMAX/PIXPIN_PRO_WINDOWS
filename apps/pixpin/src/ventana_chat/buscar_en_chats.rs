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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::SystemTime;

use pixpin_render::{Color, Pintor, RectF};
use pixpin_ui::buscar_en_chats::{self as b, Elemento, Seccion};
use pixpin_ui::chat::{self, Disposicion};

use super::{Lista, Tema, color_avatar, iniciales, logo, rf};

/// Cuantos caracteres del mensaje se ensenan antes de lo encontrado, y
/// cuantos como mucho en la linea.
const ANTES: usize = 22;
const LARGO: usize = 160;

/// Lo que se le pide al hilo.
struct Pedido {
    aguja: String,
    tope: usize,
    turno: u64,
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
}

/// Lo que contesta el hilo a un pedido.
#[derive(Debug, Clone, Default)]
pub(super) struct Resultado {
    turno: u64,
    pub encontrados: Vec<Encontrado>,
    pub total: usize,
}

/// **El buscador en todos los chats** de la ventana: lo escrito, la espera
/// y lo que llego del hilo.
pub(super) struct BuscarEnChats {
    raiz: PathBuf,
    /// La ventana a despertar cuando el hilo conteste.
    hwnd: isize,
    /// Lo escrito la ultima vez que se miro (sin espacios de los lados).
    aguja: String,
    retardo: b::Retardo,
    /// Cuantos se piden: crece con «Ver más».
    tope: usize,
    /// El del ultimo pedido: lo que conteste el hilo a uno anterior se tira.
    turno: u64,
    /// El hilo, nacido con la primera busqueda.
    hilo: Option<(Sender<Pedido>, Receiver<Resultado>)>,
    resultado: Option<Resultado>,
}

impl BuscarEnChats {
    pub fn nuevo(raiz: &Path, hwnd: isize) -> BuscarEnChats {
        BuscarEnChats {
            raiz: raiz.to_path_buf(),
            hwnd,
            aguja: String::new(),
            retardo: b::Retardo::default(),
            tope: b::DE_PRIMERAS,
            turno: 0,
            hilo: None,
            resultado: None,
        }
    }

    /// **Una vuelta del bucle**: si cambio lo escrito, se rearma la espera;
    /// si la espera acabo, se le pide al hilo; si el hilo contesto, se
    /// guarda. Devuelve si hay que volver a pintar.
    pub fn latido(&mut self, escrito: &str, ahora_ms: u64) -> bool {
        let mut cambio = false;
        let aguja = escrito.trim();
        if aguja != self.aguja {
            self.aguja = aguja.to_string();
            self.tope = b::DE_PRIMERAS;
            if self.aguja.is_empty() {
                // Sin nada escrito, la seccion se va ya, sin esperar: y lo
                // que este por llegar del hilo ya no vale.
                self.turno += 1;
                self.retardo = b::Retardo::default();
                cambio |= self.resultado.take().is_some();
            } else {
                self.retardo.cambio(ahora_ms);
            }
        }
        if self.retardo.toca(ahora_ms) {
            self.pedir();
        }
        if let Some((_, llegan)) = &self.hilo {
            while let Ok(r) = llegan.try_recv() {
                if r.turno == self.turno && !self.aguja.is_empty() {
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
        if !self.aguja.is_empty() && self.resultado.is_some() {
            self.pedir();
        }
    }

    /// «Ver más»: los siguientes.
    pub fn ver_mas(&mut self) {
        self.tope += b::DE_PRIMERAS;
        self.pedir();
    }

    fn pedir(&mut self) {
        if self.aguja.is_empty() {
            return;
        }
        if self.hilo.is_none() {
            self.hilo = lanzar_hilo(self.raiz.clone(), self.hwnd);
        }
        self.turno += 1;
        let pedido = Pedido {
            aguja: self.aguja.clone(),
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

    /// Como es la seccion en la lista ahora.
    pub fn seccion(&self) -> Seccion {
        match self.resultado() {
            Some(r) => Seccion {
                mensajes: r.encontrados.len(),
                ver_mas: r.total > r.encontrados.len(),
            },
            None => Seccion::default(),
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

    /// Los chats de lo encontrado, para tener sus logos listos al pintar.
    pub fn chats(&self) -> impl Iterator<Item = &str> {
        self.resultado()
            .into_iter()
            .flat_map(|r| r.encontrados.iter().map(|e| e.chat.as_str()))
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

/// Los mensajes de un chat, ya aplanados.
struct Chat {
    id: String,
    firma: Firma,
    entradas: Vec<b::Entrada>,
}

/// Lo que se busca de un mensaje: su texto (la nota rica sin las rutas de
/// sus fotos, que no son de nadie), su transcripcion y el nombre de su
/// fichero. El pie de una foto es su texto.
pub(super) fn entrada_de(chat: &str, m: &pixpin_proyecto::cuaderno::Mensaje) -> b::Entrada {
    let texto = pixpin_proyecto::cuaderno::sin_rutas_de_fotos(&m.texto);
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
}

/// Si el mensaje tiene burbuja en su chat (ver `se_ve`): lo que no la tiene
/// (las paginas de un PDF unido, las lecciones, lo del sistema) no se
/// encuentra, porque pulsarlo no llevaria a ninguna parte.
fn se_ve_en_el_chat(m: &pixpin_proyecto::cuaderno::Mensaje) -> bool {
    !crate::pdf_en_chat::solo_galeria(m)
        && !crate::lecciones::almacen::es_de_leccion(m)
        && !pixpin_proyecto::cuaderno::es_de_sistema(m)
}

fn leer_chat(raiz: &Path, id: &str, firma: Firma) -> Chat {
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, id);
    let entradas = match pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta) {
        Ok(c) => c
            .mensajes
            .iter()
            .filter(|m| se_ve_en_el_chat(m))
            .map(|m| entrada_de(id, m))
            .collect(),
        // Sin cuaderno (chat nuevo, disco que no esta): nada que buscar.
        Err(_) => Vec::new(),
    };
    Chat {
        id: id.to_string(),
        firma,
        entradas,
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

    fn buscar(&self, p: &Pedido) -> Resultado {
        let hallado = b::buscar(
            self.orden.iter().map(|&par| self.entrada(par)),
            &p.aguja,
            p.tope,
        );
        let encontrados = hallado
            .aciertos
            .iter()
            .map(|a| {
                let e = self.entrada(self.orden[a.entrada]);
                Encontrado {
                    chat: e.chat.clone(),
                    codigo: e.codigo.clone(),
                    cuando: e.cuando,
                    fragmento: b::fragmento(&e.piezas[a.pieza].texto, &p.aguja, ANTES, LARGO),
                }
            })
            .collect();
        Resultado {
            turno: p.turno,
            encontrados,
            total: hallado.total,
        }
    }
}

// --- Pintar ------------------------------------------------------------------

/// **Pinta la seccion «Mensajes»** debajo de las filas de chats: solo lo
/// que cae dentro de la lista.
pub(super) fn pintar(
    p: &Pintor,
    d: &Disposicion,
    tema: &Tema,
    escala: u32,
    lista: &Lista,
    en_chats: &BuscarEnChats,
) {
    let Some(r) = en_chats.resultado() else {
        return;
    };
    if d.filas.ancho == 0 || d.filas.alto == 0 || d.plegada {
        return;
    }
    let e = escala as f32 / 100.0;
    let seccion = en_chats.seccion();
    let y0 = inicio(d, lista.scroll, lista.orden.len(), escala);
    let caja = |el: Elemento| {
        let (y, alto) = seccion.caja(el, escala);
        pixpin_geom::Rect {
            x: d.filas.x,
            y: y0 + y,
            ancho: d.filas.ancho,
            alto,
        }
    };
    let se_ve = |r: pixpin_geom::Rect| r.abajo() > d.filas.y && r.y < d.filas.abajo();
    p.empujar_recorte(rf(d.filas));

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
        if lista.sobre_mensaje == Some(Elemento::Mensaje(n)) {
            p.rellenar(rf(fila), tema.fila_sobre);
        }
        pintar_encontrado(p, tema, escala, lista, fila, enc);
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

/// Una fila de mensaje encontrado: el avatar y el nombre de su chat, la
/// fecha a la derecha y, debajo, el trozo con lo encontrado resaltado.
fn pintar_encontrado(
    p: &Pintor,
    tema: &Tema,
    escala: u32,
    lista: &Lista,
    fila: pixpin_geom::Rect,
    enc: &Encontrado,
) {
    let e = escala as f32 / 100.0;
    let partes = chat::partes_fila(fila, false, escala);
    let ficha = lista.fichas.iter().find(|f| f.id == enc.chat);
    let nombre = ficha.map_or("", |f| f.nombre.as_str());

    // El avatar del chat, como en su fila: su logo o su color con iniciales.
    let a = rf(partes.avatar);
    if !ficha.is_some_and(|f| logo::pintar(p, f, a)) {
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
    if partes.ancho_texto == 0 {
        return;
    }

    // La fecha primero: dice cuanto sitio le queda al nombre.
    let hora = chat::etiqueta_hora(pixpin_shell::entorno::a_local(enc.cuando), lista.ahora);
    let tam_hora = chat::CONTADOR_TAM * e;
    let mut hueco = partes.ancho_texto as f32;
    if !hora.is_empty() {
        let (w, h) = p.medir_texto(&hora, tam_hora);
        p.texto(
            &hora,
            partes.derecha as f32 - w,
            partes.nombre.y as f32 + (chat::TEXTO_TAM * e - h) / 2.0,
            tam_hora,
            tema.apagado,
        );
        hueco -= w + chat::HORA_HUECO as f32 * e;
    }
    p.texto_linea(
        nombre,
        partes.nombre.x as f32,
        partes.nombre.y as f32,
        chat::TEXTO_TAM * e,
        hueco.max(0.0),
        tema.texto,
    );

    // El trozo: en una linea sin partir (ancho de sobra) y recortado a su
    // hueco; lo encontrado en negrita y con un fondo del color de enviar.
    let texto = &enc.fragmento.texto;
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
    let tam = chat::TEXTO_TAM * e;
    const SIN_PARTIR: f32 = 100_000.0;
    let (x, y) = (partes.resumen.x as f32, partes.resumen.y as f32);
    p.empujar_recorte(RectF {
        x,
        y,
        ancho: partes.ancho_texto as f32,
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
}
