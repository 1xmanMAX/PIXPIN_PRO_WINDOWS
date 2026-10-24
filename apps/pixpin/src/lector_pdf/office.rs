//! **Un Word o un PowerPoint en el lector de PDF**, con sus paginas tal como
//! las imprime Office (`pixpin-office`) en vez de las de un PDF.
//!
//! El lector de PDF ya tiene todo lo que pide un documento de paginas: las
//! hojas una debajo de otra, el aumento con la rueda, el pin y su barra, la
//! tinta del lienzo (`lector_tinta::Tinta`) en cada hoja, los marcadores,
//! buscar, escuchar y el negativo. Aqui solo cambia **de donde salen las
//! hojas**: en un PDF las rasteriza un hilo y llegan como imagenes; en un
//! Office se maquetan en este mismo hilo (el medidor y el pintor de
//! DirectWrite no cruzan de hilo) **unas pocas cada vuelta del bucle**, y se
//! pintan **en vectores** con `PintorOffice` a la transformada de la hoja:
//! nitidas a cualquier aumento y sin esperar a ningun rasterizado.
//!
//! - **Abrir es casi inmediato**: solo se lee el ZIP y los estilos; la
//!   primera hoja se maqueta en la primera vuelta y se ensena enseguida. Las
//!   demas van llegando mientras se lee; las que faltan ocupan el sitio de la
//!   ultima conocida (Word guarda cuantas son en `docProps/app.xml`), asi que
//!   la barra de avance apenas salta.
//! - **El texto** de cada hoja sale de sus trozos ya colocados
//!   ([`texto_de_pagina`]), letra a letra con los mismos avances con que se
//!   pinta: sirve para buscar, para escuchar y para **elegir con el raton y
//!   copiar** (Ctrl+C). Un clic en un hipervinculo lo abre.
//! - **PowerPoint**: cada diapositiva es una hoja; AvPag/RePag y las flechas
//!   pasan de una en una, y «Presentar» (F5) la pone a pantalla completa con
//!   la presentacion de siempre (`diapositivas`), ya sin PowerPoint.
//! - **Imprimir** (Ctrl+P): el dialogo de Windows con vista previa, cada
//!   hoja con lo anotado encima, en vectores.

use std::ops::Range;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use pixpin_office::pagina::Texto;
use pixpin_office::{Documento, Medidor, Pagina, Recursos, Tipo};
use pixpin_pdf::texto::PaginaDeTexto;
use pixpin_render::pagina_office::PintorOffice;
use pixpin_shell::overlay::FormaCursorWin;

use super::*;

/// Lo que se maqueta como mucho en cada vuelta del bucle: menos de un
/// fotograma, para que leer y desplazar no noten nada mientras se hace.
const PRESUPUESTO: Duration = Duration::from_millis(8);
/// Con el texto pedido (buscar, escuchar) se va mas deprisa: es lo que se
/// esta esperando.
const PRESUPUESTO_CON_PRISA: Duration = Duration::from_millis(40);
/// Un A4 en puntos: el tamano de una hoja que aun no se conoce si no se
/// conoce ninguna.
const A4: (f32, f32) = (595.0, 842.0);
/// El color de lo elegido con el raton (el azul de elegir de Windows).
const ELEGIDO: Color = Color {
    r: 0.0,
    g: 0.47,
    b: 0.84,
    a: 0.30,
};

/// Lo que llega de fuera al abrir: el documento ya leido y desde cuando se
/// esta abriendo (para los tiempos del registro).
pub(crate) struct DeOffice {
    pub(crate) documento: Documento,
    pub(crate) idioma: pixpin_store::Idioma,
    pub(crate) empezo: Instant,
}

/// Lo elegido con el raton: desde donde se pulso (`ancla`) hasta donde va
/// (`punta`), cada uno como (hoja, letra) en el texto de su hoja.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Seleccion {
    pub(super) ancla: (usize, usize),
    pub(super) punta: (usize, usize),
}

impl Seleccion {
    /// El principio y el final, en orden de lectura.
    fn ordenada(&self) -> ((usize, usize), (usize, usize)) {
        if self.ancla <= self.punta {
            (self.ancla, self.punta)
        } else {
            (self.punta, self.ancla)
        }
    }

    /// Las letras elegidas de la hoja `i`, que tiene `largo` letras.
    pub(super) fn rango_en(&self, i: usize, largo: usize) -> Option<Range<usize>> {
        let (a, b) = self.ordenada();
        if i < a.0 || i > b.0 {
            return None;
        }
        let desde = if i == a.0 { a.1.min(largo) } else { 0 };
        let hasta = if i == b.0 { b.1.min(largo) } else { largo };
        (hasta > desde).then_some(desde..hasta)
    }

    pub(super) fn vacia(&self) -> bool {
        self.ancla == self.punta
    }
}

/// Un clic o un arrastre que empezo sobre texto.
#[derive(Debug, Clone)]
pub(super) struct Pulsado {
    desde: (f32, f32),
    movido: bool,
    enlace: Option<Arc<str>>,
}

pub(crate) struct HojasOffice {
    documento: Documento,
    pintor: PintorOffice,
    recursos: Arc<Recursos>,
    paginas: Vec<Arc<Pagina>>,
    /// El texto de cada hoja, cuando se pidio una vez.
    textos: Vec<Option<PaginaDeTexto>>,
    pub(super) tipo: Tipo,
    pub(super) idioma: pixpin_store::Idioma,
    acabado: bool,
    empezo: Instant,
    primera_pintada: bool,
    /// Quien espera el texto de todas las hojas (buscar, escuchar).
    texto_pedido: Option<mpsc::Sender<TextoPdf>>,
    pub(super) seleccion: Option<Seleccion>,
    pub(super) pulsado: Option<Pulsado>,
    cursor: Option<FormaCursorWin>,
    /// Pedidos que necesitan la ventana y los atiende el bucle.
    pub(super) pedir_imprimir: bool,
}

impl Drop for HojasOffice {
    fn drop(&mut self) {
        // Las imagenes subidas a la GPU no esperan al siguiente documento.
        self.pintor.olvidar_imagenes();
    }
}

impl HojasOffice {
    /// El pintor sale del motor de esta ventana (uno por dispositivo: si el
    /// dispositivo se pierde, la ventana se rehace y con ella esto).
    pub(super) fn nuevo(de: DeOffice, motor: &pixpin_render::MotorRender) -> Option<HojasOffice> {
        let pintor = PintorOffice::nuevo(motor)?;
        let recursos = de.documento.recursos();
        Some(HojasOffice {
            tipo: de.documento.tipo(),
            documento: de.documento,
            pintor,
            recursos,
            paginas: Vec::new(),
            textos: Vec::new(),
            idioma: de.idioma,
            acabado: false,
            empezo: de.empezo,
            primera_pintada: false,
            texto_pedido: None,
            seleccion: None,
            pulsado: None,
            cursor: None,
            pedir_imprimir: false,
        })
    }

    /// Si aun quedan hojas por maquetar.
    pub(super) fn maquetando(&self) -> bool {
        !self.acabado
    }

    /// Maqueta hojas en orden hasta gastar `presupuesto`. `true` si llego
    /// alguna.
    fn maquetar(&mut self, presupuesto: Duration) -> bool {
        let t = Instant::now();
        let antes = self.paginas.len();
        while !self.acabado && (self.paginas.len() == antes || t.elapsed() < presupuesto) {
            match self
                .documento
                .pagina(self.paginas.len(), &self.pintor.letras)
            {
                Some(p) => {
                    self.paginas.push(p);
                    if self.tipo == Tipo::PowerPoint && self.paginas.len() >= self.documento.total()
                    {
                        self.acabado = true;
                    }
                }
                None => self.acabado = true,
            }
            if self.acabado {
                tracing::info!(
                    hojas = self.paginas.len(),
                    ms = self.empezo.elapsed().as_millis() as u64,
                    "office: maquetado entero"
                );
            }
        }
        self.paginas.len() > antes
    }

    /// Maqueta lo que falte de una vez (imprimir lo necesita entero).
    fn maquetar_todo(&mut self) {
        while self.maquetar(Duration::from_secs(3600)) {}
    }

    /// Lo que mide cada hoja, con las que aun faltan estimadas.
    fn medidas(&self) -> Vec<(f32, f32)> {
        let listas: Vec<(f32, f32)> = self.paginas.iter().map(|p| (p.ancho, p.alto)).collect();
        medidas_estimadas(&listas, self.documento.total(), self.acabado)
    }

    pub(super) fn pagina(&self, i: usize) -> Option<&Arc<Pagina>> {
        self.paginas.get(i)
    }

    /// El modo oscuro para las hojas que se pinten despues (luna/sol).
    pub(super) fn poner_oscuro(&self, si: bool) {
        self.pintor.poner_oscuro(si);
    }

    /// Pinta la hoja `i` en la transformada de la hoja (cero en su esquina,
    /// `vista::ANCHO_HOJA` de ancho). `false` si aun no esta maquetada.
    pub(super) fn pintar_hoja(&mut self, p: &Pintor, i: usize) -> bool {
        let Some(pagina) = self.paginas.get(i) else {
            return false;
        };
        let escala = vista::ANCHO_HOJA / pagina.ancho.max(1.0);
        let total = if self.acabado {
            self.paginas.len()
        } else {
            self.documento.total().max(self.paginas.len())
        };
        self.pintor
            .pintar(p, pagina, &self.recursos, escala, (0.0, 0.0), total as u32);
        if i == 0 && !self.primera_pintada {
            self.primera_pintada = true;
            tracing::info!(
                ms = self.empezo.elapsed().as_millis() as u64,
                "office: primera hoja pintada"
            );
        }
        true
    }

    /// El texto de la hoja `i` (se saca la primera vez que se pide).
    pub(super) fn texto(&mut self, i: usize) -> Option<&PaginaDeTexto> {
        let pagina = self.paginas.get(i)?;
        if self.textos.len() <= i {
            self.textos.resize(i + 1, None);
        }
        if self.textos[i].is_none() {
            self.textos[i] = Some(texto_de_pagina(pagina, &self.pintor.letras));
        }
        self.textos[i].as_ref()
    }

    fn todo_el_texto(&mut self) -> Vec<PaginaDeTexto> {
        (0..self.paginas.len())
            .map(|i| self.texto(i).cloned().unwrap_or_default())
            .collect()
    }

    /// El hipervinculo bajo un punto de la hoja `i` (en unidades del lector).
    fn enlace_en(&self, i: usize, x: f32, y: f32) -> Option<Arc<str>> {
        let p = self.paginas.get(i)?;
        let k = p.ancho / vista::ANCHO_HOJA;
        p.texto_en(x * k, y * k).and_then(|t| t.enlace.clone())
    }

    /// Lo elegido, como texto.
    pub(super) fn texto_elegido(&mut self) -> String {
        let Some(sel) = self.seleccion else {
            return String::new();
        };
        let (a, b) = sel.ordenada();
        let mut salida = String::new();
        for i in a.0..=b.0 {
            let Some(t) = self.texto(i) else { break };
            let largo = t.texto.chars().count();
            if let Some(r) = sel.rango_en(i, largo) {
                if !salida.is_empty() && !salida.ends_with('\n') {
                    salida.push('\n');
                }
                salida.extend(t.texto.chars().skip(r.start).take(r.end - r.start));
            }
        }
        salida
    }
}

/// **Las medidas de todas las hojas**, las que faltan estimadas: tantas como
/// diga el documento (o una mas que las ya listas, mientras se maqueta),
/// del tamano de la ultima conocida. Maquetado entero, exactamente las que
/// hay.
pub(super) fn medidas_estimadas(
    listas: &[(f32, f32)],
    total: usize,
    acabado: bool,
) -> Vec<(f32, f32)> {
    if acabado {
        return listas.to_vec();
    }
    let n = total.max(listas.len() + 1);
    let relleno = listas.last().copied().unwrap_or(A4);
    let mut v = listas.to_vec();
    v.resize(n, relleno);
    v
}

/// **El texto de una hoja maquetada**, letra a letra y con la caja de cada
/// una en fracciones de la hoja (como `pixpin_pdf::texto`, para que buscar,
/// escuchar y elegir sirvan igual). Los avances son los del medidor con el
/// interletrado y el justificado sumados, como los pinta `PintorOffice`.
/// Entre trozos de un mismo renglon se pone un espacio si estan separados;
/// al cambiar de renglon, un salto si hay hueco de parrafo, y si no un
/// espacio (un parrafo que sigue en el renglon de abajo).
pub(crate) fn texto_de_pagina(pagina: &Pagina, medidor: &dyn Medidor) -> PaginaDeTexto {
    let (w, h) = (pagina.ancho.max(1.0), pagina.alto.max(1.0));
    let mut texto = String::new();
    let mut cajas: Vec<Option<[f32; 4]>> = Vec::new();
    // La linea base, el tamano y donde acabo el trozo anterior.
    let mut antes: Option<(f32, f32, f32)> = None;
    let mut avances = Vec::new();
    for (t, r) in pagina.textos() {
        if t.texto.is_empty() {
            continue;
        }
        if let Some((y0, tam0, fin)) = antes {
            let tam = tam0.max(t.letra.tam);
            let blanco =
                texto.ends_with(char::is_whitespace) || t.texto.starts_with(char::is_whitespace);
            if (t.y - y0).abs() > tam * 0.5 {
                let salto = if (t.y - y0).abs() > tam * 1.9 || t.y < y0 {
                    '\n'
                } else {
                    ' '
                };
                if !blanco || salto == '\n' {
                    texto.push(salto);
                    cajas.push(None);
                }
            } else if t.x - fin > tam * 0.2 && !blanco {
                texto.push(' ');
                cajas.push(None);
            }
        }
        avances.clear();
        avances_de(t, medidor, &mut avances);
        let mut x = r.x;
        for (c, a) in t.texto.chars().zip(avances.iter()) {
            texto.push(c);
            cajas.push(Some([x / w, r.y / h, (x + a) / w, (r.y + r.alto) / h]));
            x += a;
        }
        antes = Some((t.y, t.letra.tam, r.x + t.ancho));
    }
    PaginaDeTexto { texto, cajas }
}

/// Lo que avanza cada letra de un trozo, con el interletrado y el
/// justificado. Si el medidor no da una por letra, el ancho del trozo
/// repartido a partes iguales.
fn avances_de(t: &Texto, medidor: &dyn Medidor, salida: &mut Vec<f32>) {
    let n = t.texto.chars().count();
    medidor.avances(&t.texto, &t.letra, salida);
    if salida.len() != n {
        salida.clear();
        salida.extend(std::iter::repeat_n(t.ancho / n.max(1) as f32, n));
        return;
    }
    for (a, c) in salida.iter_mut().zip(t.texto.chars()) {
        *a += t.espaciado;
        if c == ' ' {
            *a += t.extra_espacio;
        }
    }
}

/// **Donde cae el cursor de elegir** en una hoja, para un punto en
/// fracciones de ella: entre que dos letras (0 = antes de la primera). Se
/// busca el renglon mas cercano en vertical y en el la letra mas cercana en
/// horizontal; pasada su mitad, el cursor va detras. `None` si la hoja no
/// tiene texto.
pub(super) fn letra_cercana(t: &PaginaDeTexto, x: f32, y: f32) -> Option<usize> {
    let distancia = |a: f32, b0: f32, b1: f32| {
        if a < b0 {
            b0 - a
        } else if a > b1 {
            a - b1
        } else {
            0.0
        }
    };
    let mut mejor: Option<(f32, f32, usize, [f32; 4])> = None;
    for (i, c) in t.cajas.iter().enumerate() {
        let Some(c) = c else { continue };
        let dy = distancia(y, c[1], c[3]);
        let dx = distancia(x, c[0], c[2]);
        let mejora = match mejor {
            None => true,
            Some((my, mx, _, _)) => dy + 1e-4 < my || ((dy - my).abs() <= 1e-4 && dx < mx),
        };
        if mejora {
            mejor = Some((dy, dx, i, *c));
        }
    }
    let (_, _, i, c) = mejor?;
    Some(if x > (c[0] + c[2]) / 2.0 { i + 1 } else { i })
}

/// La letra que hay justo debajo de un punto (en fracciones de la hoja).
pub(super) fn letra_debajo(t: &PaginaDeTexto, x: f32, y: f32) -> bool {
    t.cajas
        .iter()
        .flatten()
        .any(|c| x >= c[0] && x <= c[2] && y >= c[1] && y <= c[3])
}

// ---------------------------------------------------------------------------
// En el bucle del lector

/// Una vuelta: maqueta un poco, coloca las hojas que llegaron sin mover lo
/// que se esta leyendo y entrega el texto si alguien lo esperaba. `true` si
/// hay que repintar.
pub(super) fn vuelta(e: &mut Estado, ruta: &Path, m: Marco) -> bool {
    let Some(o) = e.office.as_mut() else {
        return false;
    };
    let primera = o.paginas.is_empty();
    let prisa = if o.texto_pedido.is_some() {
        PRESUPUESTO_CON_PRISA
    } else {
        PRESUPUESTO
    };
    let antes = o.paginas.len();
    let llegaron = !o.acabado && o.maquetar(prisa);
    let nuevas = antes..o.paginas.len();
    let mut cambio = false;
    if llegaron || primera {
        let medidas = o.medidas();
        if medidas != e.medidas {
            let sitio = (!e.hojas.arriba.is_empty() && !primera).then(|| e.hojas.sitio(e.y));
            e.medidas = medidas;
            e.hojas = vista::Hojas::colocar(&e.medidas);
            // La primera vez, a donde se dejo; despues, donde se esta.
            e.y = e.hojas.y_de(sitio.unwrap_or(e.ajustes.pagina));
            cambio = true;
        }
        if primera {
            tracing::info!(
                ms = o.empezo.elapsed().as_millis() as u64,
                estimadas = e.medidas.len(),
                "office: primera hoja maquetada"
            );
        }
    }
    let Some(o) = e.office.as_mut() else {
        return cambio;
    };
    if o.acabado && llegaron {
        al_saber_las_hojas(e, ruta);
    }
    let Some(o) = e.office.as_mut() else {
        return cambio;
    };
    if o.acabado
        && let Some(tx) = o.texto_pedido.take()
    {
        let _ = tx.send(Ok(o.todo_el_texto()));
        cambio = true;
    }
    // Solo se repinta si cambio donde va cada hoja o llego una que se ve:
    // maquetar el resto mientras se lee no cuesta fotogramas.
    let vis = visibles(e, m);
    cambio || (llegaron && nuevas.start < vis.end && vis.start < nuevas.end)
}

/// Buscar o escuchar piden el texto de todas las hojas: llega cuando se
/// acaben de maquetar (ver [`vuelta`]), por el mismo camino que el del PDF.
pub(super) fn pedir_el_texto(e: &mut Estado) {
    if e.hallar.texto.is_some() || e.hallar.leyendo.is_some() {
        return;
    }
    let Some(o) = e.office.as_mut() else { return };
    let (tx, rx) = mpsc::channel();
    o.texto_pedido = Some(tx);
    e.hallar.leyendo = Some(rx);
}

/// La hoja bajo el raton, su texto y el punto en fracciones de ella.
fn bajo_el_raton(e: &mut Estado, m: Marco) -> Option<(usize, f32, f32)> {
    let (i, punto) = en_la_hoja(e, m)?;
    let alto = *e.hojas.altos.get(i)?;
    e.office.as_ref()?.pagina(i)?;
    Some((i, punto.x / vista::ANCHO_HOJA, punto.y / alto))
}

/// Al pulsar leyendo (sin anotar): sobre texto empieza a elegir (y si es un
/// hipervinculo, se apunta para abrirlo al soltar sin moverse). `false` si
/// no hay texto debajo: entonces el lector arrastra la hoja, como siempre.
pub(super) fn pulsar(e: &mut Estado, m: Marco) -> bool {
    let Some((i, fx, fy)) = bajo_el_raton(e, m) else {
        return false;
    };
    let Some((_, punto)) = en_la_hoja(e, m) else {
        return false;
    };
    let Some(o) = e.office.as_mut() else {
        return false;
    };
    let Some(t) = o.texto(i) else { return false };
    if !letra_debajo(t, fx, fy) {
        o.seleccion = None;
        return false;
    }
    let letra = letra_cercana(t, fx, fy).unwrap_or(0);
    let enlace = o.enlace_en(i, punto.x, punto.y);
    o.seleccion = Some(Seleccion {
        ancla: (i, letra),
        punta: (i, letra),
    });
    o.pulsado = Some(Pulsado {
        desde: e.raton,
        movido: false,
        enlace,
    });
    true
}

/// Arrastrando sobre texto: la punta de lo elegido va al raton (a la hoja
/// que haya debajo, aunque sea otra).
pub(super) fn mover(e: &mut Estado, m: Marco) {
    let raton = e.raton;
    let Some(p) = e.office.as_mut().and_then(|o| o.pulsado.as_mut()) else {
        return;
    };
    if !p.movido
        && (raton.0 - p.desde.0).abs().max((raton.1 - p.desde.1).abs()) > lector::UMBRAL_DE_ARRASTRE
    {
        p.movido = true;
    }
    let Some((i, fx, fy)) = bajo_el_raton(e, m) else {
        return;
    };
    let Some(o) = e.office.as_mut() else { return };
    let Some(letra) = o.texto(i).and_then(|t| letra_cercana(t, fx, fy)) else {
        return;
    };
    if let Some(s) = o.seleccion.as_mut() {
        s.punta = (i, letra);
    }
}

/// Al soltar: un clic sin moverse en un hipervinculo lo abre; sin moverse
/// en texto normal, se quita lo elegido.
pub(super) fn soltar(e: &mut Estado) -> bool {
    let Some(o) = e.office.as_mut() else {
        return false;
    };
    let Some(p) = o.pulsado.take() else {
        return false;
    };
    if !p.movido {
        o.seleccion = None;
        if let Some(enlace) = p.enlace {
            abrir_enlace(&enlace);
        }
    } else if o.seleccion.is_some_and(|s| s.vacia()) {
        o.seleccion = None;
    }
    true
}

/// Solo se abre lo que es de la web o del correo: un enlace a un fichero o a
/// un programa dentro de un documento no se ejecuta con un clic.
pub(super) fn enlace_que_se_abre(enlace: &str) -> bool {
    let e = enlace.trim().to_ascii_lowercase();
    e.starts_with("http://") || e.starts_with("https://") || e.starts_with("mailto:")
}

fn abrir_enlace(enlace: &str) {
    if !enlace_que_se_abre(enlace) {
        tracing::info!("office: hipervinculo que no se abre desde el lector (no es web ni correo)");
        return;
    }
    if let Err(err) = pixpin_shell::abrir::abrir(Path::new(enlace.trim())) {
        tracing::warn!(?err, "office: no se pudo abrir el hipervinculo");
    }
}

/// La forma del cursor leyendo: la mano en un hipervinculo, la barra en el
/// texto, la flecha en lo demas. `None` si no cambia.
pub(super) fn cursor(e: &mut Estado, m: Marco) -> Option<FormaCursorWin> {
    if e.anotando {
        e.office.as_mut()?.cursor = None;
        return None;
    }
    let forma = match bajo_el_raton(e, m) {
        Some((i, fx, fy)) => {
            let (_, punto) = en_la_hoja(e, m)?;
            let o = e.office.as_mut()?;
            if o.enlace_en(i, punto.x, punto.y)
                .is_some_and(|l| enlace_que_se_abre(&l))
            {
                FormaCursorWin::Giro
            } else if o.texto(i).is_some_and(|t| letra_debajo(t, fx, fy)) {
                FormaCursorWin::Texto
            } else {
                FormaCursorWin::Flecha
            }
        }
        None => FormaCursorWin::Flecha,
    };
    let o = e.office.as_mut()?;
    if o.cursor == Some(forma) {
        return None;
    }
    o.cursor = Some(forma);
    Some(forma)
}

/// Ctrl+C: lo elegido al portapapeles. `false` si no habia nada elegido.
pub(super) fn copiar(e: &mut Estado, textos: Option<&Catalogo>) -> bool {
    let Some(o) = e.office.as_mut() else {
        return false;
    };
    let texto = o.texto_elegido();
    if texto.is_empty() {
        return false;
    }
    match pixpin_codec::portapapeles::copiar_texto(&texto) {
        Ok(()) => {
            if let Some(t) = textos {
                e.aviso = Some((t.t("lector-copiado"), ahora_ms() + 1500));
            }
        }
        Err(err) => tracing::warn!(?err, "office: no se pudo copiar el texto elegido"),
    }
    true
}

/// Lo elegido en la hoja `i`, ya con la transformada de la hoja puesta.
pub(super) fn pintar_seleccion(e: &mut Estado, p: &Pintor, i: usize) {
    let Some(alto) = e.hojas.altos.get(i).copied() else {
        return;
    };
    let Some(o) = e.office.as_mut() else { return };
    let Some(sel) = o.seleccion else { return };
    if sel.vacia() {
        return;
    }
    let Some(t) = o.texto(i) else { return };
    let Some(r) = sel.rango_en(i, t.cajas.len()) else {
        return;
    };
    for c in t.cajas_de(r.start, r.end - r.start) {
        p.rellenar(
            RectF {
                x: c[0] * vista::ANCHO_HOJA,
                y: c[1] * alto,
                ancho: (c[2] - c[0]) * vista::ANCHO_HOJA,
                alto: (c[3] - c[1]) * alto,
            },
            ELEGIDO,
        );
    }
}

// ---------------------------------------------------------------------------
// PowerPoint: pasar diapositivas y presentar

/// La diapositiva que se esta mirando: la que ocupa el centro de la ventana.
pub(super) fn la_de_ahora(e: &Estado, m: Marco) -> usize {
    if e.hojas.cuantas() == 0 {
        return 0;
    }
    let s = px(e, m);
    e.hojas.en((e.y + m.alto / s / 2.0).max(0.0))
}

/// Lleva la vista a la diapositiva `i`, centrada si cabe entera.
pub(super) fn ir_a(e: &mut Estado, i: usize, m: Marco) {
    let Some((&arriba, &alto)) = e.hojas.arriba.get(i).zip(e.hojas.altos.get(i)) else {
        return;
    };
    let s = px(e, m);
    let visto = m.alto / s;
    e.y = if alto <= visto {
        arriba - (visto - alto) / 2.0
    } else {
        arriba
    };
}

/// Las teclas de pasar diapositivas en un PowerPoint (las de la
/// presentacion: AvPag/RePag, flechas, Espacio, Inicio/Fin). `true` si era
/// una de ellas.
pub(super) fn tecla_de_diapositivas(
    e: &mut Estado,
    vk: u32,
    shift: bool,
    ctrl: bool,
    m: Marco,
) -> bool {
    if ctrl
        || e.office.as_ref().is_none_or(|o| o.tipo != Tipo::PowerPoint)
        || e.hojas.cuantas() == 0
    {
        return false;
    }
    let ahora = la_de_ahora(e, m);
    let ultima = e.hojas.cuantas() - 1;
    let destino = match vk {
        VK_NEXT | VK_RIGHT | VK_DOWN => (ahora + 1).min(ultima),
        VK_ESPACIO if !shift => (ahora + 1).min(ultima),
        VK_PRIOR | VK_LEFT | VK_UP => ahora.saturating_sub(1),
        VK_ESPACIO => ahora.saturating_sub(1),
        VK_HOME => 0,
        VK_END => ultima,
        _ => return false,
    };
    ir_a(e, destino, m);
    true
}

/// «Presentar»: la presentacion a pantalla completa desde la diapositiva que
/// se mira, en su ventana (`diapositivas`), sin PowerPoint.
pub(super) fn presentar(e: &Estado, ubicacion: &Ubicacion, ruta: &Path, m: Marco) {
    let Some(o) = &e.office else { return };
    crate::diapositivas::presentar_nativa(o.idioma, ubicacion.clone(), ruta, la_de_ahora(e, m));
}

/// «Modo lectura»: el Word reflujado en el lector de documentos de siempre.
pub(super) fn modo_lectura(e: &Estado, ubicacion: &Ubicacion, ruta: &Path) {
    let Some(o) = &e.office else { return };
    crate::visor::lanzar(o.idioma, ubicacion.clone(), ruta);
}

// ---------------------------------------------------------------------------
// Imprimir

/// Lo que se imprime: las hojas maquetadas, sus imagenes y lo anotado.
struct OfficeImpreso {
    paginas: Vec<Arc<Pagina>>,
    recursos: Arc<Recursos>,
    capas: Vec<Option<lector_tinta::Capa>>,
    tinta: std::cell::RefCell<Tinta>,
    pintor: Option<PintorOffice>,
}

impl pixpin_render::imprimir_moderno::Documento for OfficeImpreso {
    fn paginar(&mut self, motor: &pixpin_render::MotorRender, _alcance: Option<&str>) -> usize {
        if self.pintor.is_none() {
            self.pintor = PintorOffice::nuevo(motor);
        }
        self.paginas.len()
    }

    fn pintar(&self, i: usize, hoja: pixpin_render::imprimir_moderno::Pagina, p: &Pintor) {
        let (Some(pagina), Some(pintor)) = (self.paginas.get(i), &self.pintor) else {
            return;
        };
        let (x, y, k) = encajar_en_el_papel((pagina.ancho, pagina.alto), hoja.papel);
        // Las unidades del lector: la hoja mide `ANCHO_HOJA` de ancho, como
        // la tinta.
        let unidades = k * pagina.ancho / vista::ANCHO_HOJA * hoja.escala;
        p.poner_vista((x * hoja.escala, y * hoja.escala), unidades, (0.0, 0.0));
        let alto = vista::ANCHO_HOJA * pagina.alto / pagina.ancho.max(1.0);
        pintor.pintar(
            p,
            pagina,
            &self.recursos,
            vista::ANCHO_HOJA / pagina.ancho.max(1.0),
            (0.0, 0.0),
            self.paginas.len() as u32,
        );
        if let Some(Some(capa)) = self.capas.get(i) {
            p.con_recorte(
                RectF {
                    x: 0.0,
                    y: 0.0,
                    ancho: vista::ANCHO_HOJA,
                    alto,
                },
                |p| {
                    self.tinta.borrow_mut().pintar_capa(
                        p,
                        i,
                        capa,
                        (0.0, 0.0, vista::ANCHO_HOJA, alto),
                        unidades,
                        lector_tinta::papel(Color::BLANCO),
                        false,
                    );
                },
            );
        }
    }
}

/// Donde va una hoja de `pagina` puntos en un papel de `papel` DIP: a su
/// tamano si cabe (centrada), y si no, reducida hasta caber. Devuelve la
/// esquina en DIP y los DIP por punto.
pub(super) fn encajar_en_el_papel(pagina: (f32, f32), papel: (f32, f32)) -> (f32, f32, f32) {
    let dip = 96.0 / 72.0;
    let (w, h) = (pagina.0.max(1.0) * dip, pagina.1.max(1.0) * dip);
    let cabe = (papel.0 / w).min(papel.1 / h).min(1.0);
    let k = dip * cabe;
    (
        (papel.0 - pagina.0 * k) / 2.0,
        (papel.1 - pagina.1 * k) / 2.0,
        k,
    )
}

/// Ctrl+P: el dialogo de imprimir de Windows con todas las hojas y lo
/// anotado (lo pendiente se guarda antes).
pub(super) fn imprimir(e: &mut Estado, ventana: isize, ruta: &Path, textos: &Catalogo) {
    let Some((doc, titulo, apaisada)) = impreso(e, ruta) else {
        return;
    };
    let hecho = (|| -> Result<()> {
        let d = pixpin_capture::Dispositivo::nuevo().context("sin dispositivo para imprimir")?;
        let motor =
            pixpin_render::MotorRender::nuevo(d.d3d()).context("sin motor para imprimir")?;
        let pedido = pixpin_render::imprimir_moderno::Pedido {
            titulo,
            apaisada,
            opcion: None,
            ppp_previa: 144.0,
        };
        pixpin_render::imprimir_moderno::mostrar(
            windows::Win32::Foundation::HWND(ventana as *mut _),
            pedido,
            d.d3d().clone(),
            motor,
            Box::new(doc),
        )
        .context("el dialogo de imprimir no se abrio")
    })();
    if let Err(err) = hecho {
        tracing::warn!(?err, "office: no se pudo imprimir");
        e.aviso = Some((textos.t("lector-no-imprime"), ahora_ms() + 4000));
    }
}

/// **Lo que exporta un Word o un PowerPoint con hoja** (el «Exportar»
/// comun): sus hojas como se imprimen, con lo anotado, y el original.
pub(super) fn exportable(
    e: &mut Estado,
    ruta: &Path,
) -> Option<crate::compartir::exportable::Exportable> {
    let (mut doc, titulo, apaisada) = impreso(e, ruta)?;
    let papel = if apaisada {
        (1123.0, 794.0)
    } else {
        (794.0, 1123.0)
    };
    let hojas = crate::compartir::exportable::hojas_impresas(&mut doc, papel, 2.0, 400)
        .inspect_err(|err| {
            tracing::warn!(
                ?err,
                "office: no se pudieron pintar las hojas para exportar"
            )
        })
        .ok()?;
    let mut x = crate::compartir::exportable::Exportable::nuevo(titulo);
    x.raiz = Some(e.ubicacion.raiz().to_path_buf());
    x.original = Some(ruta.to_path_buf());
    x.hojas = hojas;
    Some(x)
}

/// Lo que se imprime (y se exporta): las hojas maquetadas y su tinta.
fn impreso(e: &mut Estado, ruta: &Path) -> Option<(OfficeImpreso, String, bool)> {
    guardar_capas(e, ruta);
    let o = e.office.as_mut()?;
    o.maquetar_todo();
    let paginas = o.paginas.clone();
    let recursos = o.recursos.clone();
    let capas = (0..paginas.len())
        .map(|i| {
            let alto = vista::ANCHO_HOJA * paginas[i].alto / paginas[i].ancho.max(1.0);
            let r = e.donde.para_leer(ruta, i);
            r.is_file()
                .then(|| e.donde.leer_capa(ruta, i, e.ajustes.espacios, alto))
                .filter(|c| !c.vacia())
        })
        .collect();
    let titulo = pixpin_docs::sin_extension(&pixpin_docs::nombre(ruta));
    let apaisada = paginas.first().is_some_and(|p| p.ancho > p.alto);
    let doc = OfficeImpreso {
        paginas,
        recursos,
        capas,
        tinta: std::cell::RefCell::new(Tinta::nueva()),
        pintor: None,
    };
    Some((doc, titulo, apaisada))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_office::MedidorFijo;
    use pixpin_office::pagina::{Color as ColorO, Elemento, Letra};

    fn trozo(x: f32, y: f32, texto: &str, enlace: Option<&str>) -> Elemento {
        Elemento::Texto(Texto {
            x,
            y,
            letra: Letra::nueva("Arial", 10.0),
            color: ColorO::NEGRO,
            texto: texto.into(),
            subrayado: None,
            tachado: false,
            espaciado: 0.0,
            extra_espacio: 0.0,
            ancho: texto.chars().count() as f32 * 5.0,
            campo: None,
            enlace: enlace.map(Arc::from),
        })
    }

    fn hoja(elementos: Vec<Elemento>) -> Pagina {
        Pagina {
            ancho: 200.0,
            alto: 100.0,
            elementos,
            ..Default::default()
        }
    }

    #[test]
    fn las_hojas_que_faltan_ocupan_el_sitio_de_la_ultima_conocida() {
        let listas = [(595.0, 842.0), (842.0, 595.0)];
        let v = medidas_estimadas(&listas, 5, false);
        assert_eq!(v.len(), 5);
        assert_eq!(v[4], (842.0, 595.0));
        // Mas listas que las que dijo Word: siempre una de mas por llegar.
        assert_eq!(medidas_estimadas(&listas, 1, false).len(), 3);
        // Sin ninguna conocida, un A4.
        assert_eq!(medidas_estimadas(&[], 2, false), vec![A4, A4]);
        // Caso negativo: acabado, ni una de mas aunque Word dijera otra cosa.
        assert_eq!(medidas_estimadas(&listas, 9, true), listas.to_vec());
    }

    #[test]
    fn el_texto_de_la_hoja_va_letra_a_letra_con_su_caja() {
        let p = hoja(vec![
            trozo(10.0, 20.0, "Hola", None),
            trozo(40.0, 20.0, "mundo", None),
            trozo(10.0, 32.0, "dos", None),
        ]);
        let t = texto_de_pagina(&p, &MedidorFijo::default());
        // Separados en el mismo renglon: un espacio; renglon siguiente
        // pegado: otro espacio (el parrafo sigue).
        assert_eq!(t.texto, "Hola mundo dos");
        assert_eq!(t.cajas.len(), t.texto.chars().count());
        // La «H» empieza en x = 10 de 200 y mide 5 puntos (la mitad de 10).
        let h = t.cajas[0].unwrap();
        assert!(
            (h[0] - 0.05).abs() < 1e-5 && (h[2] - 0.075).abs() < 1e-5,
            "{h:?}"
        );
        // El espacio puesto no tiene caja.
        assert!(t.cajas[4].is_none());
    }

    #[test]
    fn un_parrafo_lejos_va_tras_un_salto_de_renglon() {
        let p = hoja(vec![
            trozo(10.0, 20.0, "Uno", None),
            trozo(10.0, 60.0, "Dos", None),
        ]);
        assert_eq!(
            texto_de_pagina(&p, &MedidorFijo::default()).texto,
            "Uno\nDos"
        );
        // Caso negativo: trozos pegados en el mismo renglon no llevan espacio.
        let p = hoja(vec![
            trozo(10.0, 20.0, "Pal", None),
            trozo(25.0, 20.0, "abra", None),
        ]);
        assert_eq!(
            texto_de_pagina(&p, &MedidorFijo::default()).texto,
            "Palabra"
        );
    }

    #[test]
    fn el_cursor_de_elegir_cae_en_la_letra_mas_cercana_de_su_renglon() {
        let p = hoja(vec![
            trozo(10.0, 20.0, "abcd", None),
            trozo(10.0, 60.0, "efgh", None),
        ]);
        let t = texto_de_pagina(&p, &MedidorFijo::default());
        // En la segunda mitad de la «b» (x 15..20 de 200): detras de ella.
        assert_eq!(letra_cercana(&t, 19.0 / 200.0, 17.0 / 100.0), Some(2));
        // Muy a la derecha del primer renglon: al final de el.
        assert_eq!(letra_cercana(&t, 0.9, 0.17), Some(4));
        // Mas cerca del segundo renglon: en el.
        assert_eq!(letra_cercana(&t, 0.0, 0.58), Some(5));
        // Caso negativo: una hoja sin texto no tiene donde poner el cursor.
        assert_eq!(letra_cercana(&PaginaDeTexto::default(), 0.5, 0.5), None);
    }

    #[test]
    fn lo_elegido_va_de_la_ancla_a_la_punta_en_cualquier_sentido_y_entre_hojas() {
        let s = Seleccion {
            ancla: (2, 5),
            punta: (0, 3),
        };
        assert_eq!(s.rango_en(0, 10), Some(3..10));
        assert_eq!(s.rango_en(1, 7), Some(0..7));
        assert_eq!(s.rango_en(2, 10), Some(0..5));
        // Caso negativo: fuera de las hojas elegidas, o vacia, nada.
        assert_eq!(s.rango_en(3, 10), None);
        assert_eq!(
            Seleccion {
                ancla: (1, 4),
                punta: (1, 4)
            }
            .rango_en(1, 10),
            None
        );
    }

    #[test]
    fn solo_se_abren_los_enlaces_de_la_web_y_del_correo() {
        assert!(enlace_que_se_abre("https://pixpin.app"));
        assert!(enlace_que_se_abre(" HTTP://a.b "));
        assert!(enlace_que_se_abre("mailto:max@ejemplo.com"));
        // Caso negativo: un fichero o un programa no se ejecutan con un clic.
        assert!(!enlace_que_se_abre("file:///C:/Windows/System32/calc.exe"));
        assert!(!enlace_que_se_abre("C:\\a.exe"));
        assert!(!enlace_que_se_abre("#_Toc123"));
    }

    /// **El lector con un Word de verdad**, pintado fuera de pantalla: la
    /// hoja en vectores con texto elegido, la misma en negativo y la hoja tal
    /// como sale a la impresora, en PNG para mirarlos; y el texto elegido
    /// tal como lo copia Ctrl+C. `PIXPIN_OFFICE` es el `.docx` (por defecto
    /// una muestra publica de `E:\pixpin-office-muestras`). Necesita GPU:
    /// `cargo test -p pixpin --bin pixpinmax muestra_del_lector_de_office -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU y un .docx de muestra"]
    fn muestra_del_lector_de_office() {
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        use pixpin_render::imprimir_moderno::Documento as _;

        let ruta = std::env::var_os("PIXPIN_OFFICE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "E:/pixpin-office-muestras/docx/poi-imagenes-listas.docx".into());
        if !ruta.is_file() {
            println!("sin muestra en {}", ruta.display());
            return;
        }
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let _com = pixpin_shell::ComDelHilo::iniciar();
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = pixpin_render::MotorRender::nuevo(d.d3d()).expect("motor");
        let m = super::super::pruebas::marco();
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let empezo = Instant::now();
        let documento = Documento::abrir(&ruta).expect("abre");
        let mut e = super::super::pruebas::estado(0);
        e.office = HojasOffice::nuevo(
            DeOffice {
                documento,
                idioma: pixpin_store::Idioma::Espanol,
                empezo,
            },
            &motor,
        );
        assert!(e.office.is_some(), "pintor de Office");
        while e.office.as_ref().is_some_and(|o| o.maquetando()) {
            vuelta(&mut e, &ruta, m);
        }
        println!(
            "maquetado en {} ms, {} hojas",
            empezo.elapsed().as_millis(),
            e.hojas.cuantas()
        );
        acotar(&mut e, m);
        // Lo que se elegiria arrastrando por el principio de la primera hoja.
        let largo = e
            .office
            .as_mut()
            .and_then(|o| o.texto(0))
            .map(|t| t.texto.chars().count())
            .unwrap_or(0);
        assert!(largo > 0, "la primera hoja tiene texto");
        e.office.as_mut().unwrap().seleccion = Some(Seleccion {
            ancla: (0, 0),
            punta: (0, largo.min(120)),
        });
        let copiado = e.office.as_mut().unwrap().texto_elegido();
        println!("Ctrl+C copiaria: {copiado:?}");
        assert!(!copiado.trim().is_empty());
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), m.ancho as u32, m.alto as u32)
            .expect("superficie");
        let guardar = |nombre: &str, fuera: &FueraDePantalla| {
            fuera.esperar_gpu().expect("esperar");
            let (ancho, alto, pixeles) = fuera.leer_rgba().expect("leer");
            let png = pixpin_codec::imagen::codificar_png(&ImagenRgba {
                ancho,
                alto,
                pixeles,
            })
            .expect("png");
            let salida = carpeta.join(nombre);
            std::fs::write(&salida, png).expect("guardar");
            println!("{}", salida.display());
        };
        motor
            .dibujar(&fuera.destino, |p| pintar(&mut e, p, m, &textos))
            .expect("pintar");
        guardar("lector-office-elegido.png", &fuera);
        e.en_negativo = true;
        motor
            .dibujar(&fuera.destino, |p| pintar(&mut e, p, m, &textos))
            .expect("pintar");
        guardar("lector-office-negativo.png", &fuera);
        // En oscuro (luna/sol): el papel oscuro y el texto claro; las
        // imagenes y los rellenos con sus colores.
        e.en_negativo = false;
        e.oscuro = true;
        motor
            .dibujar(&fuera.destino, |p| pintar(&mut e, p, m, &textos))
            .expect("pintar");
        guardar("lector-office-oscuro.png", &fuera);
        e.oscuro = false;

        // La primera hoja tal como va a la impresora, en un A4.
        let o = e.office.as_ref().unwrap();
        let mut impreso = OfficeImpreso {
            paginas: o.paginas.clone(),
            recursos: o.recursos.clone(),
            capas: (0..o.paginas.len()).map(|_| None).collect(),
            tinta: std::cell::RefCell::new(Tinta::nueva()),
            pintor: None,
        };
        assert_eq!(impreso.paginar(&motor, None), o.paginas.len());
        let papel = pixpin_render::imprimir_moderno::A4;
        let hoja =
            FueraDePantalla::nuevo(&motor, d.d3d(), papel.0 as u32, papel.1 as u32).expect("papel");
        motor
            .dibujar(&hoja.destino, |p| {
                p.limpiar(Color::BLANCO);
                impreso.pintar(
                    0,
                    pixpin_render::imprimir_moderno::Pagina { papel, escala: 1.0 },
                    p,
                );
            })
            .expect("imprimir");
        guardar("lector-office-impreso.png", &hoja);
    }

    #[test]
    fn la_hoja_va_a_su_tamano_en_el_papel_y_si_no_cabe_se_reduce() {
        // Un A4 en un A4: a su tamano (96/72 DIP por punto), sin margen.
        let (x, y, k) = encajar_en_el_papel((595.0, 842.0), (793.7, 1122.5));
        assert!(
            (k - 96.0 / 72.0).abs() < 1e-3 && x.abs() < 1.0 && y.abs() < 1.0,
            "{x} {y} {k}"
        );
        // Una hoja apaisada en papel vertical: reducida y centrada en alto.
        let (x, y, k) = encajar_en_el_papel((842.0, 595.0), (793.7, 1122.5));
        assert!(k < 96.0 / 72.0 && x.abs() < 1.0 && y > 100.0, "{x} {y} {k}");
    }
}
