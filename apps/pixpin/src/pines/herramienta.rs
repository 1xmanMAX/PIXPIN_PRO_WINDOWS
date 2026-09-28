//! Una herramienta dentro de un pin: la mini-app del movil (`pin/MiniApps.kt`)
//! o una tabla pegada (`TableBody`), pineada suelta en el escritorio (C3, L3).
//!
//! **No hay mini-apps nuevas.** Lo que se ve y lo que hace cada toque sale de
//! `mini_panel` —el mismo panel del chat, puerto de `MiniActivity.kt`—, y el
//! documento es el mismo texto que viaja por la sincronizacion. Aqui solo
//! vive lo propio de estar en un pin: la disposicion dentro de la tarjeta, el
//! toque que llega del pin, el latido y el aviso del temporizador.
//!
//! El documento vive en el almacen como una nota cuyo origen dice que es
//! (`mini:tareas`, `mini:tabla`...), asi que se restaura al arrancar como
//! cualquier pin, y un PixPin viejo que no lo entienda ensena el texto.

use pixpin_geom::{Punto, Rect};
use pixpin_pin::magia::MiniApp;
use pixpin_proyecto::mini::{self, gastos};
use pixpin_render::icono::{Icono, material as mi};
use pixpin_render::{Color, Pintor, RectF};
use pixpin_store::Catalogo;
use pixpin_ui::mini::{self as ui, Disposicion};

use crate::caja_dibujo::hex;
use crate::mini_panel::{self, Efecto, Orden, Rotulo, Teclado, Vista};

/// La tabla pegada: no es una mini-app del cuaderno, pero vive igual.
pub const TABLA: &str = "tabla";

/// Lo que va delante en el origen de la entrada del almacen.
pub const PREFIJO_ORIGEN: &str = "mini:";

/// La mini-app de cada palabra magica, si es una mini-app (la pizarra, el
/// lienzo y la hoja son dibujo, no documento).
pub fn cual_de(app: MiniApp) -> Option<&'static str> {
    Some(match app {
        MiniApp::Temporizador => mini::TEMPORIZADOR,
        MiniApp::Cronometro => mini::CRONOMETRO,
        MiniApp::Tareas => mini::TAREAS,
        MiniApp::Contador => mini::CONTADOR,
        MiniApp::Gastos => mini::GASTOS,
        MiniApp::Ruleta => mini::RULETA,
        MiniApp::Pizarra | MiniApp::Lienzo | MiniApp::Hoja => return None,
    })
}

pub fn origen_de(cual: &str) -> String {
    format!("{PREFIJO_ORIGEN}{cual}")
}

/// Que herramienta dice un origen del almacen, si es una que se sabe pintar.
/// Una de una version futura se queda como nota: mejor su texto que nada.
pub fn cual_de_origen(origen: &str) -> Option<&'static str> {
    let cual = origen.strip_prefix(PREFIJO_ORIGEN)?;
    if cual == TABLA {
        return Some(TABLA);
    }
    mini::TODAS.into_iter().find(|m| *m == cual)
}

/// El nombre de la herramienta, para la cabecera de un documento sin titulo.
pub fn clave_del_nombre(cual: &str) -> &'static str {
    match cual {
        mini::TAREAS => "mini-tareas",
        mini::GASTOS => "mini-gastos",
        mini::CRONOMETRO => "mini-cronometro",
        mini::TEMPORIZADOR => "mini-temporizador",
        mini::CONTADOR => "mini-contador",
        mini::RULETA => "mini-ruleta",
        mini::ALARMA => "mini-alarma",
        _ => "pin-herramienta-tabla",
    }
}

/// El documento con el que nace una herramienta, sembrado con `texto`.
///
/// Las que tienen lista (tareas, gastos, ruleta) reciben cada linea como si
/// se tecleara en su caja y se pulsara Intro: «Cena 42,50» es un gasto y
/// «- [x] pan» una tarea hecha, por el mismo camino que el teclado. Si la
/// primera linea es un titulo de Markdown, es el nombre. Las demas nacen
/// vacias, como con la palabra magica. La tabla es el texto tal cual.
pub fn documento_nuevo(cual: &str, texto: &str, moneda: &gastos::Moneda) -> Option<String> {
    if cual == TABLA {
        return Some(texto.to_string());
    }
    let titulo = mini::titulo(texto);
    let mut documento = mini::documento_nuevo(cual, &titulo, moneda)?;
    if matches!(cual, mini::TAREAS | mini::GASTOS | mini::RULETA) {
        let cuerpo = if titulo.is_empty() {
            texto
        } else {
            mini::cuerpo(texto)
        };
        for linea in cuerpo.lines().map(str::trim).filter(|l| !l.is_empty()) {
            documento = mini_panel::aplicar(
                cual,
                &documento,
                moneda,
                &Orden::Anadir(linea.to_string()),
                0,
            );
        }
    }
    Some(documento)
}

/// Lo que el gestor tiene que hacer despues de un toque o una tecla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hecho {
    Nada,
    /// Cambio lo que se ve pero no el documento (la caja, la fila marcada).
    Repintar,
    /// Cambio el documento: repintar Y guardarlo ya, como el movil
    /// (`MiniActivity.kt:102-110`): un temporizador arrancado tiene que
    /// sobrevivir a que se cierre PixPin.
    Guardar,
    /// Escape sin nada a medias: cerrar el pin, como Esc en cualquier pin.
    Cerrar,
}

/// Una herramienta pineada: su documento y lo de la interfaz, que no se
/// guarda.
#[derive(Debug, Clone)]
pub struct Herramienta {
    pub cual: String,
    pub documento: String,
    pub teclado: Teclado,
    pub scroll: i32,
    /// A quien le toco en el ultimo sorteo. No va al documento, igual que en
    /// el movil y en el chat.
    pub elegido: Option<String>,
    /// Si ya se aviso de que el temporizador llego a cero: el aviso sale una
    /// vez (`timerAlerted`), no en cada latido.
    pub avisado: bool,
}

/// La moneda con la que nacen unos gastos: la del idioma elegido.
pub fn moneda(textos: &Catalogo) -> gastos::Moneda {
    gastos::Moneda::de_codigo(textos.t("mini-moneda").trim()).unwrap_or_else(gastos::Moneda::euro)
}

impl Herramienta {
    pub fn nueva(cual: &str, documento: String) -> Herramienta {
        Herramienta {
            cual: cual.to_string(),
            documento,
            teclado: Teclado::default(),
            scroll: 0,
            elegido: None,
            avisado: false,
        }
    }

    pub fn es_tabla(&self) -> bool {
        self.cual == TABLA
    }

    /// Lo que se pinta ahora, con lo del teclado encima. `None` en la tabla.
    pub fn vista(&self, moneda: &gastos::Moneda, ahora: i64) -> Option<Vista> {
        mini_panel::vista_con(&self.cual, &self.documento, moneda, ahora, &self.teclado)
    }

    /// Cada cuanto hay que repintar sin que nadie toque, en ms. `None`
    /// parado: un pin quieto no despierta a nadie (`esperaDelReloj`).
    pub fn late_cada_ms(&self, moneda: &gastos::Moneda, ahora: i64) -> Option<u32> {
        self.vista(moneda, ahora)?
            .late_cada_ms
            .map(|ms| ms.clamp(15, 60_000) as u32)
    }

    /// Si el temporizador acaba de llegar a cero. Verdad UNA vez por vuelta
    /// a cero; al volver a ponerlo en marcha se rearma.
    pub fn vencio(&mut self, moneda: &gastos::Moneda, ahora: i64) -> bool {
        let alerta = self.vista(moneda, ahora).is_some_and(|v| v.alerta);
        if !alerta {
            self.avisado = false;
            return false;
        }
        !std::mem::replace(&mut self.avisado, true)
    }

    /// La disposicion dentro de la tarjeta, en pixeles del contenido con el
    /// (0,0) en su esquina: la misma para pintar y para responder al clic.
    pub fn disposicion(v: &Vista, tam: (u32, u32), escala: u32) -> Disposicion {
        Disposicion::calcular(
            Rect {
                x: 0,
                y: 0,
                ancho: tam.0,
                alto: tam.1,
            },
            escala,
            v.reparto,
        )
    }

    /// Un toque dentro, en pixeles del contenido. Puerto de `pulsar_mini` y
    /// `zona_mini` del chat: botones, marcar, borrar por el aspa y, en la
    /// cabecera, corregir el nombre (en el pin no hay «volver»).
    pub fn clic(
        &mut self,
        p: Punto,
        tam: (u32, u32),
        escala: u32,
        moneda: &gastos::Moneda,
        ahora: i64,
    ) -> Efecto {
        let Some(v) = self.vista(moneda, ahora) else {
            return Efecto::Nada;
        };
        let t = Herramienta::disposicion(&v, tam, escala);
        if t.cabecera.contiene(p) {
            mini_panel::empezar_a_corregir(
                &self.cual,
                &self.documento,
                moneda,
                &mut self.teclado,
                None,
            );
            return Efecto::Repintar;
        }
        for (fila, botones) in v.filas_de_botones.iter().enumerate() {
            if let Some(n) = t.cual_boton(p, fila as u32, botones.len(), escala)
                && botones[n].activo
            {
                return Efecto::Hacer(botones[n].orden.clone());
            }
        }
        let Some(n) = t.cual_fila(p, v.lista.len(), self.scroll, escala) else {
            return Efecto::Nada;
        };
        let caja = t.fila(n, self.scroll, escala);
        if v.lista[n].se_borra && t.aspa(caja, escala).contiene(p) {
            return Efecto::Hacer(Orden::Quitar(n));
        }
        if v.lista[n].se_marca {
            mini_panel::clic_en_fila(&mut self.teclado, n);
            return Efecto::Hacer(Orden::Alternar(n));
        }
        // Un gasto o un nombre: queda marcado para el teclado.
        mini_panel::clic_en_fila(&mut self.teclado, n);
        Efecto::Repintar
    }

    /// Cumple lo que pidio un toque o una tecla. Puerto de `cumplir_mini`
    /// sin cuaderno: guardar lo hace el gestor cuando esto dice `Guardar`.
    /// `azar` es el del sorteo, de fuera para poder probarlo.
    #[allow(clippy::too_many_arguments)] // las mismas cuentas que pintar
    pub fn cumplir(
        &mut self,
        efecto: Efecto,
        tam: (u32, u32),
        escala: u32,
        moneda: &gastos::Moneda,
        ahora: i64,
        azar: f64,
    ) -> Hecho {
        let orden = match efecto {
            Efecto::Nada => return Hecho::Nada,
            Efecto::Cerrar => return Hecho::Cerrar,
            Efecto::Repintar => None,
            Efecto::Hacer(Orden::Girar(_)) => Some(Orden::Girar(azar)),
            Efecto::Hacer(o) => Some(o),
        };
        let mut hecho = Hecho::Repintar;
        let al_final = matches!(orden, Some(Orden::Anadir(_)));
        match orden {
            Some(Orden::Girar(azar)) => {
                self.elegido = mini_panel::girar(&self.documento, &mut self.teclado, azar);
            }
            Some(o) => {
                let nuevo = mini_panel::aplicar(&self.cual, &self.documento, moneda, &o, ahora);
                if nuevo != self.documento {
                    self.documento = nuevo;
                    hecho = Hecho::Guardar;
                }
            }
            None => {}
        }
        if let Some(v) = self.vista(moneda, ahora) {
            let t = Herramienta::disposicion(&v, tam, escala);
            self.scroll = if al_final {
                // Abajo del todo, donde acaba de caer lo anadido.
                t.tope_scroll(v.lista.len(), escala)
            } else {
                mini_panel::scroll_para_ver(
                    &t,
                    self.teclado.marcada,
                    v.lista.len(),
                    self.scroll,
                    escala,
                )
            };
        }
        hecho
    }

    /// Una tecla con el pin enfocado.
    #[allow(clippy::too_many_arguments)]
    pub fn tecla(
        &mut self,
        k: mini_panel::Tecla,
        tam: (u32, u32),
        escala: u32,
        moneda: &gastos::Moneda,
        ahora: i64,
        azar: f64,
    ) -> Hecho {
        if self.es_tabla() {
            // La tabla solo se lee: Esc la cierra, como a cualquier pin.
            return if k.vk == 0x1B { Hecho::Cerrar } else { Hecho::Nada };
        }
        let e = mini_panel::tecla(&self.cual, &self.documento, moneda, &mut self.teclado, k);
        self.cumplir(e, tam, escala, moneda, ahora, azar)
    }

    /// Un caracter escrito con el pin enfocado.
    pub fn caracter(
        &mut self,
        c: char,
        tam: (u32, u32),
        escala: u32,
        moneda: &gastos::Moneda,
        ahora: i64,
        azar: f64,
    ) -> Hecho {
        if self.es_tabla() {
            return Hecho::Nada;
        }
        let e = mini_panel::caracter(&self.cual, &self.documento, &mut self.teclado, c);
        self.cumplir(e, tam, escala, moneda, ahora, azar)
    }

    /// La rueda: tres filas por muesca, sin pasarse de los extremos.
    pub fn rueda(
        &mut self,
        delta: i32,
        tam: (u32, u32),
        escala: u32,
        moneda: &gastos::Moneda,
        ahora: i64,
    ) -> bool {
        let (cuantas, tope) = if self.es_tabla() {
            let filas = pixpin_pin::tabla::leer(&self.documento).len();
            let alto = (FILA_TABLA * escala / 100) as i64;
            let util = tam.1 as i64 - 2 * (ui::MARGEN * escala / 100) as i64;
            (filas, (filas as i64 * alto - util).max(0) as i32)
        } else {
            let Some(v) = self.vista(moneda, ahora) else {
                return false;
            };
            let t = Herramienta::disposicion(&v, tam, escala);
            (v.lista.len(), t.tope_scroll(v.lista.len(), escala))
        };
        if cuantas == 0 {
            return false;
        }
        let fila = (ui::FILA_ALTO * escala / 100) as i32;
        let nuevo = (self.scroll - delta * 3 * fila / 120).clamp(0, tope);
        let cambio = nuevo != self.scroll;
        self.scroll = nuevo;
        cambio
    }
}

/// Alto de una fila de la tabla, en pixeles logicos.
const FILA_TABLA: u32 = 26;
/// Ancho que se le da a una columna de la tabla al nacer.
const COLUMNA_TABLA: u32 = 110;

/// El tamano con que nace el pin de una herramienta, en pixeles fisicos:
/// el ancho de una tarjeta del movil y el alto que pide su disposicion, con
/// sitio para cinco lineas de lista. Luego se estira libre.
pub fn tamano_natural(h: &Herramienta, escala: u32, moneda: &gastos::Moneda) -> (u32, u32) {
    let e = |v: u32| v * escala / 100;
    if h.es_tabla() {
        let filas = pixpin_pin::tabla::leer(&h.documento);
        let columnas = filas.first().map_or(1, Vec::len) as u32;
        let ancho = (columnas * COLUMNA_TABLA + 2 * ui::MARGEN).clamp(240, 640);
        let alto = (filas.len() as u32 * FILA_TABLA + 2 * ui::MARGEN).clamp(80, 480);
        return (e(ancho), e(alto));
    }
    let Some(v) = h.vista(moneda, 0) else {
        return (e(300), e(200));
    };
    let r = v.reparto;
    let mut alto = ui::CABECERA_ALTO + r.filas_de_botones * ui::BOTONES_ALTO;
    if r.con_tablero {
        alto += ui::TABLERO_ALTO;
    }
    if r.con_lista {
        alto += 5 * ui::FILA_ALTO;
    }
    if r.con_anadir {
        alto += ui::ANADIR_ALTO;
    }
    (e(320), e(alto + ui::MARGEN))
}

/// Los colores de la tarjeta: los del pin (blanca o casi negra segun el
/// tema), con el acento del chat para lo que se pulsa.
struct Tema {
    papel: Color,
    texto: Color,
    apagado: Color,
    acento: Color,
    borde: Color,
    separador: Color,
    caja: Color,
}

fn tema(claro: bool) -> Tema {
    if claro {
        Tema {
            papel: hex(0xffffff),
            texto: hex(0x111111),
            apagado: hex(0x999999),
            acento: hex(0x40a7e3),
            borde: Color {
                a: 0.10,
                ..hex(0x101b24)
            },
            separador: hex(0xebebeb),
            caja: hex(0xf1f1f1),
        }
    } else {
        Tema {
            papel: Color {
                r: 0.11,
                g: 0.11,
                b: 0.12,
                a: 1.0,
            },
            texto: hex(0xeeeef0),
            apagado: hex(0x7f91a4),
            acento: hex(0x5288c1),
            borde: hex(0x44495c),
            separador: hex(0x2a2d33),
            caja: hex(0x242730),
        }
    }
}

fn encoger(r: RectF, cuanto: f32) -> RectF {
    RectF {
        x: r.x + cuanto,
        y: r.y + cuanto,
        ancho: (r.ancho - 2.0 * cuanto).max(0.0),
        alto: (r.alto - 2.0 * cuanto).max(0.0),
    }
}

/// Lo que se necesita para pintar, todo por valor: el pin lo guarda en su
/// pintor y lo llama en cada `WM_PAINT`.
pub struct Cuadro {
    pub herramienta: Herramienta,
    pub escala: u32,
    pub claro: bool,
    pub moneda: gastos::Moneda,
}

impl Cuadro {
    /// Pinta la herramienta en `caja` (la tarjeta del pin). La hora se
    /// pregunta aqui, en UTC como los documentos: el cronometro corre sin
    /// que el gestor rehaga nada.
    pub fn pintar(&self, p: &Pintor, caja: RectF, textos: &Catalogo) {
        let t = tema(self.claro);
        p.rellenar(caja, t.papel);
        if self.herramienta.es_tabla() {
            pintar_tabla(p, caja, &self.herramienta, self.escala, &t);
            return;
        }
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        let Some(v) = self.herramienta.vista(&self.moneda, ahora) else {
            return;
        };
        pintar_mini(p, caja, &self.herramienta, &v, self.escala, &t, textos);
    }
}

/// El panel de la mini-app dentro de la tarjeta: puerto de `pintar_mini`
/// del chat con los colores del pin y la cabecera sin «volver».
fn pintar_mini(
    p: &Pintor,
    caja: RectF,
    h: &Herramienta,
    v: &Vista,
    escala: u32,
    tema: &Tema,
    textos: &Catalogo,
) {
    let e = escala as f32 / 100.0;
    let tam = (caja.ancho.max(1.0) as u32, caja.alto.max(1.0) as u32);
    let d = Herramienta::disposicion(v, tam, escala);
    // La disposicion va en el (0,0) del contenido; aqui se lleva a la caja.
    let rf = |r: Rect| RectF {
        x: caja.x + r.x as f32,
        y: caja.y + r.y as f32,
        ancho: r.ancho as f32,
        alto: r.alto as f32,
    };
    let margen = ui::MARGEN as f32 * e;

    // La cabecera: el nombre, o el de la herramienta si no tiene.
    let tam_titulo = ui::TITULO_TAM * e;
    let mut titulo = mini::titulo(&h.documento);
    if titulo.is_empty() {
        titulo = textos.t(clave_del_nombre(&h.cual));
    }
    let cab = rf(d.cabecera);
    let (_, alto_titulo) = p.medir_texto("Ag", tam_titulo);
    p.texto_linea(
        &titulo,
        cab.x + margen,
        cab.y + (cab.alto - alto_titulo) / 2.0,
        tam_titulo,
        (cab.ancho - 2.0 * margen).max(0.0),
        tema.texto,
    );
    p.rellenar(
        RectF {
            x: cab.x,
            y: cab.y + cab.alto - 1.0,
            ancho: cab.ancho,
            alto: 1.0,
        },
        tema.separador,
    );

    // El tablero: el numero grande, lo que se lee desde lejos.
    if d.tablero.alto > 0 {
        let texto = match (v.tablero.is_empty(), h.elegido.as_deref()) {
            (true, Some(quien)) => quien.to_string(),
            (true, None) => String::new(),
            _ => v.tablero.clone(),
        };
        if !texto.is_empty() {
            let tab = rf(d.tablero);
            let tam = ui::TABLERO_TAM * e;
            let (ancho, alto) = p.medir_texto(&texto, tam);
            p.texto(
                &texto,
                tab.x + (tab.ancho - ancho) / 2.0,
                tab.y + (tab.alto - alto) / 2.0,
                tam,
                if v.alerta { hex(0xe5534b) } else { tema.texto },
            );
        }
    }

    // Los botones, a partes iguales.
    let tam_boton = ui::BOTON_TAM * e;
    for (fila, botones) in v.filas_de_botones.iter().enumerate() {
        for (n, b) in botones.iter().enumerate() {
            let r = rf(d.boton(fila as u32, n, botones.len(), escala));
            let radio = 8.0 * e;
            if b.principal && b.activo {
                p.rellenar_redondeado(r, radio, tema.acento);
            } else {
                p.rellenar_redondeado(r, radio, tema.borde);
                p.rellenar_redondeado(encoger(r, 1.0), (radio - 1.0).max(0.0), tema.papel);
            }
            let rotulo = match &b.rotulo {
                Rotulo::Clave(k) => textos.t(k),
                Rotulo::Tal(s) => s.clone(),
            };
            let (ancho, alto) = p.medir_texto(&rotulo, tam_boton);
            let color = match (b.activo, b.principal) {
                (false, _) => tema.apagado,
                (true, true) => hex(0xffffff),
                (true, false) => tema.texto,
            };
            p.texto_linea(
                &rotulo,
                r.x + (r.ancho - ancho).max(0.0) / 2.0,
                r.y + (r.alto - alto) / 2.0,
                tam_boton,
                r.ancho,
                color,
            );
        }
    }

    // La lista, lo unico que se desplaza.
    if d.lista.alto > 0 {
        let tam = ui::FILA_TAM * e;
        p.empujar_recorte(rf(d.lista));
        let (_, alto_letra) = p.medir_texto("Ag", tam);
        for (n, f) in v.lista.iter().enumerate() {
            let fila = d.fila(n, h.scroll, escala);
            // Lo que no se ve no se mide: mil nombres no cuestan mil medidas.
            if fila.abajo() < d.lista.y || fila.y > d.lista.abajo() {
                continue;
            }
            let r = rf(fila);
            if f.marcada {
                p.rellenar_redondeado(r, 6.0 * e, tema.borde);
            }
            let y = r.y + (r.alto - alto_letra) / 2.0;
            let color = if f.hecha { tema.apagado } else { tema.texto };
            let mut derecha = r.x + r.ancho;
            if f.se_borra {
                let aspa = rf(d.aspa(fila, escala));
                p.icono(&mi::CLOSE, encoger(aspa, 7.0 * e), tema.apagado);
                derecha = aspa.x;
            }
            if !f.detalle.is_empty() {
                let (ancho_d, _) = p.medir_texto(&f.detalle, tam);
                p.texto(&f.detalle, derecha - ancho_d - 6.0 * e, y, tam, color);
                derecha -= ancho_d + 12.0 * e;
            }
            let mut x = r.x;
            if f.se_marca {
                let icono: &'static Icono = if f.hecha {
                    &mi::CHECK_BOX
                } else {
                    &mi::CHECK_BOX_OUTLINE_BLANK
                };
                p.icono(
                    icono,
                    RectF {
                        x,
                        y: r.y + (r.alto - 18.0 * e) / 2.0,
                        ancho: 18.0 * e,
                        alto: 18.0 * e,
                    },
                    if f.hecha { tema.acento } else { tema.apagado },
                );
                x += 26.0 * e;
            }
            p.texto_linea(&f.texto, x, y, tam, (derecha - x).max(0.0), color);
            p.rellenar(
                RectF {
                    x: r.x,
                    y: r.y + r.alto - 1.0,
                    ancho: r.ancho,
                    alto: 1.0,
                },
                tema.separador,
            );
        }
        p.soltar_recorte();
    }

    // La caja de escribir, abajo.
    if d.anadir.alto > 0 {
        let tam = ui::ANADIR_TAM * e;
        let r = rf(d.anadir);
        p.rellenar(r, tema.caja);
        let (_, alto) = p.medir_texto("Ag", tam);
        let (texto, color) = if h.teclado.borrador.is_empty() {
            (v.guia.map(|g| textos.t(g)).unwrap_or_default(), tema.apagado)
        } else {
            (format!("{}|", h.teclado.borrador), tema.texto)
        };
        p.texto_linea(
            &texto,
            r.x + margen,
            r.y + (r.alto - alto) / 2.0,
            tam,
            (r.ancho - 2.0 * margen).max(0.0),
            color,
        );
    }
}

/// La tabla pegada: columnas del mismo ancho y la primera fila de cabecera
/// (`TableBody` del movil). En un pin estrecho lo que salva la lectura es
/// que las cifras queden unas bajo otras, no que cada columna se ajuste.
fn pintar_tabla(p: &Pintor, caja: RectF, h: &Herramienta, escala: u32, tema: &Tema) {
    let filas = pixpin_pin::tabla::leer(&h.documento);
    let Some(columnas) = filas.first().map(Vec::len).filter(|c| *c > 0) else {
        return;
    };
    let e = escala as f32 / 100.0;
    let margen = ui::MARGEN as f32 * e;
    let alto_fila = FILA_TABLA as f32 * e;
    let tam = 12.0 * e;
    let ancho_col = ((caja.ancho - 2.0 * margen) / columnas as f32).max(1.0);
    let (_, alto_letra) = p.medir_texto("Ag", tam);
    p.empujar_recorte(encoger(caja, margen / 2.0));
    for (n, fila) in filas.iter().enumerate() {
        let y = caja.y + margen + n as f32 * alto_fila - h.scroll as f32;
        if y + alto_fila < caja.y || y > caja.y + caja.alto {
            continue;
        }
        let cabecera = n == 0;
        for (c, celda) in fila.iter().enumerate() {
            p.texto_linea(
                celda,
                caja.x + margen + c as f32 * ancho_col + 4.0 * e,
                y + (alto_fila - alto_letra) / 2.0,
                tam,
                (ancho_col - 8.0 * e).max(0.0),
                // Los datos son lo que se lee: van en el color del texto; la
                // cabecera ya se distingue por la raya de debajo.
                tema.texto,
            );
        }
        if cabecera {
            p.rellenar(
                RectF {
                    x: caja.x + margen,
                    y: y + alto_fila - 1.0,
                    ancho: caja.ancho - 2.0 * margen,
                    alto: 1.0,
                },
                tema.separador,
            );
        }
    }
    p.soltar_recorte();
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn euro() -> gastos::Moneda {
        gastos::Moneda::euro()
    }

    #[test]
    fn cada_palabra_de_mini_app_lleva_a_su_documento_y_el_dibujo_no() {
        assert_eq!(cual_de(MiniApp::Tareas), Some(mini::TAREAS));
        assert_eq!(cual_de(MiniApp::Temporizador), Some(mini::TEMPORIZADOR));
        assert_eq!(cual_de(MiniApp::Ruleta), Some(mini::RULETA));
        for dibujo in [MiniApp::Pizarra, MiniApp::Lienzo, MiniApp::Hoja] {
            assert_eq!(cual_de(dibujo), None, "{dibujo:?}");
        }
    }

    #[test]
    fn el_origen_vuelve_y_uno_desconocido_se_queda_en_nota() {
        assert_eq!(cual_de_origen(&origen_de(mini::GASTOS)), Some(mini::GASTOS));
        assert_eq!(cual_de_origen("mini:tabla"), Some(TABLA));
        assert_eq!(cual_de_origen("mini:holograma"), None);
        assert_eq!(cual_de_origen("portapapeles"), None);
    }

    #[test]
    fn las_lineas_de_una_nota_se_vuelven_tareas_y_el_titulo_nombre() {
        let d = documento_nuevo(mini::TAREAS, "# Compra\npan\n- [x] leche\n\n", &euro()).unwrap();
        assert_eq!(mini::titulo(&d), "Compra");
        let tareas = mini::leer_tareas(&d);
        assert_eq!(tareas.len(), 2, "{d}");
        assert_eq!(tareas[0].texto, "pan");
        // Como al teclearla (`Tareas.saneado`): la casilla pegada se limpia
        // y la tarea entra por hacer, igual que en el movil.
        assert_eq!(tareas[1].texto, "leche", "{d}");
        assert!(!tareas[1].hecha, "{d}");
    }

    #[test]
    fn las_lineas_de_una_nota_se_vuelven_gastos_con_su_importe() {
        let d = documento_nuevo(mini::GASTOS, "Cena 42,50\nTaxi 12", &euro()).unwrap();
        let libro = gastos::leer(&d, &euro());
        assert_eq!(libro.gastos.len(), 2, "{d}");
        assert_eq!(gastos::total(&libro.gastos), 5450);
    }

    #[test]
    fn un_cronometro_nace_vacio_aunque_la_nota_traiga_texto() {
        let d = documento_nuevo(mini::CRONOMETRO, "lo que sea", &euro()).unwrap();
        assert_eq!(d, mini::documento_nuevo(mini::CRONOMETRO, "", &euro()).unwrap());
        // Caso negativo: una palabra que no es mini-app no da documento.
        assert_eq!(documento_nuevo("holograma", "x", &euro()), None);
    }

    fn tareas(n: usize) -> Herramienta {
        let texto: Vec<String> = (0..n).map(|i| format!("tarea {i}")).collect();
        Herramienta::nueva(
            mini::TAREAS,
            documento_nuevo(mini::TAREAS, &texto.join("\n"), &euro()).unwrap(),
        )
    }

    #[test]
    fn tocar_una_tarea_la_tacha_y_pide_guardar() {
        let mut h = tareas(3);
        let tam = (320, 400);
        let v = h.vista(&euro(), 0).unwrap();
        let d = Herramienta::disposicion(&v, tam, 100);
        let fila = d.fila(1, 0, 100);
        let p = Punto {
            x: fila.x + 40,
            y: fila.y + fila.alto as i32 / 2,
        };
        let e = h.clic(p, tam, 100, &euro(), 0);
        assert_eq!(h.cumplir(e, tam, 100, &euro(), 0, 0.5), Hecho::Guardar);
        assert!(mini::leer_tareas(&h.documento)[1].hecha);
        assert!(!mini::leer_tareas(&h.documento)[0].hecha, "solo la tocada");
    }

    #[test]
    fn tocar_la_cabecera_corrige_el_nombre_sin_tocar_el_documento() {
        let mut h = tareas(1);
        let antes = h.documento.clone();
        let e = h.clic(Punto { x: 50, y: 10 }, (320, 400), 100, &euro(), 0);
        assert_eq!(e, Efecto::Repintar);
        assert_eq!(h.teclado.edicion, mini_panel::Edicion::Titulo);
        assert_eq!(h.documento, antes);
    }

    #[test]
    fn un_toque_fuera_de_todo_no_hace_nada() {
        let mut h = tareas(1);
        // Debajo de la unica fila, en la lista vacia.
        let e = h.clic(Punto { x: 50, y: 300 }, (320, 400), 100, &euro(), 0);
        assert_eq!(e, Efecto::Nada);
    }

    #[test]
    fn escribir_e_intro_anade_y_baja_al_final() {
        let mut h = tareas(12);
        let tam = (320, 300);
        for c in "nueva".chars() {
            h.caracter(c, tam, 100, &euro(), 0, 0.0);
        }
        let intro = mini_panel::Tecla {
            vk: 0x0D,
            shift: false,
            ctrl: false,
            alt: false,
        };
        assert_eq!(h.tecla(intro, tam, 100, &euro(), 0, 0.0), Hecho::Guardar);
        let t = mini::leer_tareas(&h.documento);
        assert_eq!(t.last().unwrap().texto, "nueva");
        assert!(h.scroll > 0, "se baja para ver lo anadido");
    }

    #[test]
    fn escape_sin_nada_a_medias_cierra_el_pin() {
        let mut h = tareas(1);
        let esc = mini_panel::Tecla {
            vk: 0x1B,
            shift: false,
            ctrl: false,
            alt: false,
        };
        assert_eq!(h.tecla(esc, (320, 400), 100, &euro(), 0, 0.0), Hecho::Cerrar);
        // Caso negativo: con algo escrito, Esc solo lo borra.
        h.caracter('x', (320, 400), 100, &euro(), 0, 0.0);
        assert_eq!(h.tecla(esc, (320, 400), 100, &euro(), 0, 0.0), Hecho::Repintar);
    }

    #[test]
    fn el_contador_suma_con_su_boton_grande() {
        let mut h = Herramienta::nueva(
            mini::CONTADOR,
            documento_nuevo(mini::CONTADOR, "", &euro()).unwrap(),
        );
        let tam = (320, 300);
        let v = h.vista(&euro(), 0).unwrap();
        let d = Herramienta::disposicion(&v, tam, 100);
        let (fila, n) = v
            .filas_de_botones
            .iter()
            .enumerate()
            .find_map(|(f, bs)| bs.iter().position(|b| b.orden == Orden::Mas).map(|n| (f, n)))
            .expect("el contador tiene boton de sumar");
        let r = d.boton(fila as u32, n, v.filas_de_botones[fila].len(), 100);
        let p = Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        };
        let e = h.clic(p, tam, 100, &euro(), 0);
        assert_eq!(h.cumplir(e, tam, 100, &euro(), 0, 0.0), Hecho::Guardar);
        assert_eq!(pixpin_proyecto::mini::contador::leer(&h.documento).valor, 1);
    }

    #[test]
    fn el_temporizador_avisa_una_vez_al_llegar_a_cero() {
        let mut h = Herramienta::nueva(
            mini::TEMPORIZADOR,
            documento_nuevo(mini::TEMPORIZADOR, "", &euro()).unwrap(),
        );
        let tam = (320, 300);
        // Un minuto y en marcha, a las 0.
        h.cumplir(Efecto::Hacer(Orden::Duracion(60_000)), tam, 100, &euro(), 0, 0.0);
        h.cumplir(Efecto::Hacer(Orden::ArrancarOParar), tam, 100, &euro(), 0, 0.0);
        assert!(h.late_cada_ms(&euro(), 1_000).is_some(), "corriendo, late");
        assert!(!h.vencio(&euro(), 30_000), "a medias no avisa");
        assert!(h.vencio(&euro(), 61_000), "a cero avisa");
        assert!(!h.vencio(&euro(), 62_000), "y solo una vez");
    }

    #[test]
    fn la_rueda_baja_por_la_lista_sin_pasarse() {
        let mut h = tareas(30);
        let tam = (320, 300);
        assert!(h.rueda(-120, tam, 100, &euro(), 0));
        assert!(h.scroll > 0);
        for _ in 0..100 {
            h.rueda(-120, tam, 100, &euro(), 0);
        }
        let v = h.vista(&euro(), 0).unwrap();
        let tope = Herramienta::disposicion(&v, tam, 100).tope_scroll(30, 100);
        assert_eq!(h.scroll, tope);
        // Caso negativo: hacia arriba del todo no baja de cero.
        for _ in 0..100 {
            h.rueda(120, tam, 100, &euro(), 0);
        }
        assert_eq!(h.scroll, 0);
    }

    #[test]
    fn la_tabla_nace_a_la_medida_de_sus_columnas_y_solo_se_lee() {
        let mut t = Herramienta::nueva(TABLA, "a\tb\tc\n1\t2\t3".into());
        let (w, h) = tamano_natural(&t, 100, &euro());
        assert_eq!(w, 3 * COLUMNA_TABLA + 2 * ui::MARGEN);
        assert!(h >= 80);
        let intro = mini_panel::Tecla {
            vk: 0x0D,
            shift: false,
            ctrl: false,
            alt: false,
        };
        assert_eq!(t.tecla(intro, (w, h), 100, &euro(), 0, 0.0), Hecho::Nada);
        assert_eq!(t.caracter('x', (w, h), 100, &euro(), 0, 0.0), Hecho::Nada);
    }

    /// Las muestras para mirar a ojo: cada herramienta pintada fuera de
    /// pantalla, en claro y en oscuro, a `PIXPIN_MUESTRAS_PINES` (o a la
    /// carpeta temporal).
    #[test]
    #[ignore = "necesita GPU; genera PNG de muestra"]
    fn muestras_de_las_herramientas_pineadas() {
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let dir = std::env::var_os("PIXPIN_MUESTRAS_PINES")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let d = pixpin_capture::Dispositivo::nuevo().unwrap();
        let motor = pixpin_render::MotorRender::nuevo(d.d3d()).unwrap();
        let mut tareas = tareas(4);
        tareas.cumplir(Efecto::Hacer(Orden::Alternar(1)), (320, 400), 125, &euro(), 0, 0.0);
        let gastos = Herramienta::nueva(
            mini::GASTOS,
            documento_nuevo(mini::GASTOS, "# Viaje\nCena 42,50\nTaxi 12", &euro()).unwrap(),
        );
        let contador = Herramienta::nueva(
            mini::CONTADOR,
            documento_nuevo(mini::CONTADOR, "", &euro()).unwrap(),
        );
        let tabla = Herramienta::nueva(TABLA, "Mes\tIngresos\tGastos\nEnero\t1200\t800\nFebrero\t1350\t910".into());
        for (nombre, h) in [
            ("tareas", tareas),
            ("gastos", gastos),
            ("contador", contador),
            ("tabla", tabla),
        ] {
            for claro in [true, false] {
                let (w, alto) = tamano_natural(&h, 125, &euro());
                let fuera = pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(
                    &motor,
                    d.d3d(),
                    w,
                    alto,
                )
                .unwrap();
                let cuadro = Cuadro {
                    herramienta: h.clone(),
                    escala: 125,
                    claro,
                    moneda: euro(),
                };
                motor
                    .dibujar(&fuera.destino, |p| {
                        cuadro.pintar(
                            p,
                            RectF {
                                x: 0.0,
                                y: 0.0,
                                ancho: w as f32,
                                alto: alto as f32,
                            },
                            &textos,
                        )
                    })
                    .unwrap();
                let (ancho, alto, pixeles) = fuera.leer_rgba().unwrap();
                let img = pixpin_codec::ImagenRgba {
                    ancho,
                    alto,
                    pixeles,
                };
                let tema = if claro { "claro" } else { "oscuro" };
                std::fs::write(
                    dir.join(format!("pin-{nombre}-{tema}.png")),
                    pixpin_codec::codificar_png(&img).unwrap(),
                )
                .unwrap();
            }
        }
    }

    #[test]
    fn una_lista_nace_con_sitio_para_cinco_lineas() {
        let h = tareas(0);
        let (w, alto) = tamano_natural(&h, 150, &euro());
        assert_eq!(w, 480);
        assert!(alto >= (ui::CABECERA_ALTO + 5 * ui::FILA_ALTO) * 150 / 100);
    }
}
