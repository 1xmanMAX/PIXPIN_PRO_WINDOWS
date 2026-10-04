//! **La hoja del «+» de la caja** (`Menus2.dc.html`, b): una cuadricula con
//! lo de todos los dias —foto, archivo, lienzo, nota, lista de tareas,
//! tabla, conversacion y captura, con las teclas 1–8— y debajo, en chips,
//! lo que se usa menos. Arriba un buscador: se escribe y se filtra.
//!
//! Cada entrada hace EXACTAMENTE lo que hacia la del menu del clip de antes
//! (la misma `Accion`); la hoja solo cambia como se elige.

use std::cell::RefCell;

use pixpin_geom::{Punto, Rect};
use pixpin_render::icono::Icono;
use pixpin_render::{Pintor, RectF};
use pixpin_store::Catalogo;

use super::{Accion, Pinta, Respuesta, con_alfa, encoger, hex, mi, rf};

/// Medidas en pixeles logicos (al 100 %).
pub(super) const ANCHO: u32 = 460;
pub(super) const RELLENO: u32 = 14;
pub(super) const RADIO: u32 = 18;
pub(super) const BUSCADOR: u32 = 40;
pub(super) const HUECO: u32 = 10;
pub(super) const COLUMNAS: u32 = 4;
pub(super) const BALDOSA_ALTO: u32 = 96;
pub(super) const BALDOSA_HUECO: u32 = 4;
pub(super) const ICONO_CAJA: u32 = 48;
pub(super) const CABECERA_MAS: u32 = 20;
pub(super) const CHIP_ALTO: u32 = 32;
pub(super) const CHIP_HUECO: u32 = 6;
pub(super) const VACIO: u32 = 44;
/// Lo que se separa del boton que la abre.
pub(super) const SOBRE_EL_BOTON: u32 = 8;
const TAM: f32 = 13.0;
const TAM_BUSCADOR: f32 = 14.0;

/// Una entrada de la hoja, ya traducida.
pub(super) struct Opcion {
    pub rotulo: String,
    /// El rotulo y sus sinonimos, en minusculas y sin tildes: donde busca
    /// el buscador («pdf» encuentra Archivo).
    buscable: String,
    pub accion: Accion,
    /// El color de su cuadrado y su icono (solo las baldosas).
    pub color: u32,
    pub icono: &'static Icono,
    /// Del 1 al 8 en las baldosas; `None` en los chips.
    pub numero: Option<u8>,
}

/// La hoja abierta: lo escrito en el buscador y lo resaltado.
pub(super) struct Hoja {
    pub opciones: Vec<Opcion>,
    pub busqueda: String,
    /// Indice en `visibles()` bajo el raton.
    pub sobre: Option<usize>,
    /// Indice en `visibles()` elegido con las flechas (lo que hace Intro).
    pub elegido: Option<usize>,
    pista: String,
    titulo_mas: String,
    nada: String,
    /// Lo ancho de cada chip, medido al pintar (por indice de `opciones`).
    anchos: RefCell<Vec<f32>>,
}

/// Minusculas y sin tildes, para buscar sin pensar en acentos.
pub(super) fn normalizar(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            otra => otra,
        })
        .collect()
}

impl Hoja {
    pub(super) fn nueva(textos: &Catalogo) -> Hoja {
        use pixpin_proyecto::mini;
        let baldosa = |n: u8, clave: &str, palabras: &str, accion: Accion, color: u32, icono: &'static Icono| {
            let rotulo = textos.t(clave);
            Opcion {
                buscable: normalizar(&format!("{rotulo} {palabras}")),
                rotulo,
                accion,
                color,
                icono,
                numero: Some(n),
            }
        };
        let chip = |clave: &str, palabras: &str, accion: Accion| {
            let rotulo = textos.t(clave);
            Opcion {
                buscable: normalizar(&format!("{rotulo} {palabras}")),
                rotulo,
                accion,
                color: 0,
                icono: &mi::CHECKLIST,
                numero: None,
            }
        };
        let opciones = vec![
            baldosa(1, "adjuntar-imagen", "imagen foto video photo image", Accion::AdjImagen, 0x0a6cd6, &mi::IMAGE),
            baldosa(2, "adjuntar-archivo", "pdf documento word excel fichero file", Accion::AdjArchivo, 0x636366, &mi::DESCRIPTION),
            baldosa(3, "v2menus-adj-lienzo", "dibujo pizarra canvas draw", Accion::AdjLienzo, 0xc77700, &mi::DRAW),
            baldosa(4, "chat-adj-nota-md", "nota markdown texto note", Accion::AdjNotaMd, 0x248a3d, &mi::LIST),
            baldosa(5, "v2menus-adj-tareas", "tareas lista todo checklist", Accion::AdjMini(mini::TAREAS), 0x1e7a4a, &mi::CHECKLIST),
            baldosa(6, "v2menus-adj-tabla", "tabla hoja calculo excel table", Accion::AdjTabla, 0x4b49c8, &mi::TABLE_CHART),
            baldosa(7, "v2menus-adj-conversacion", "conversacion voz traducir conversation", Accion::Conversacion, 0x9a3fc4, &mi::RECORD_VOICE_OVER),
            baldosa(8, "v2menus-adj-captura", "captura pantalla recorte screenshot", Accion::AdjCaptura, 0xc93c35, &mi::CROP),
            chip("v2menus-adj-pagina", "pagina proyecto page", Accion::AdjPagina),
            chip("v2menus-adj-proyecto", "proyecto entero project", Accion::AdjProyectos),
            chip("chat-adj-telepronter", "teleprompter leer", Accion::Telepronter),
            chip("chat-adj-pronunciar", "pronunciar ingles pronounce", Accion::Pronunciar),
            chip("mini-cronometro", "cronometro stopwatch mini-app", Accion::AdjMini(mini::CRONOMETRO)),
            chip("mini-temporizador", "temporizador timer mini-app", Accion::AdjMini(mini::TEMPORIZADOR)),
            chip("mini-contador", "contador counter mini-app", Accion::AdjMini(mini::CONTADOR)),
            chip("mini-ruleta", "ruleta sorteo mini-app", Accion::AdjMini(mini::RULETA)),
            chip("mini-gastos", "gastos dinero mini-app", Accion::AdjMini(mini::GASTOS)),
            chip("mini-alarma", "alarma despertador mini-app", Accion::AdjMini(mini::ALARMA)),
            chip("v2menus-adj-movil", "movil telefono wifi phone", Accion::AdjDelMovil),
        ];
        Hoja {
            anchos: RefCell::new(vec![100.0; opciones.len()]),
            opciones,
            busqueda: String::new(),
            sobre: None,
            elegido: None,
            pista: textos.t("v2menus-adj-buscar"),
            titulo_mas: textos.t("v2menus-adj-mas"),
            nada: textos.t("v2menus-adj-nada"),
        }
    }

    /// Las opciones que se ven con lo escrito, por indice de `opciones`, en
    /// el orden de la hoja (primero las baldosas, luego los chips).
    pub(super) fn visibles(&self) -> Vec<usize> {
        let q = normalizar(self.busqueda.trim());
        self.opciones
            .iter()
            .enumerate()
            .filter(|(_, o)| q.is_empty() || q.split_whitespace().all(|w| o.buscable.contains(w)))
            .map(|(n, _)| n)
            .collect()
    }

    /// Donde va todo, calculado igual al pintar y al pulsar.
    pub(super) fn colocar(&self, ancla: Punto, marco: Rect, escala: u32) -> Puesta {
        let anchos = self.anchos.borrow();
        colocar(&self.opciones, &self.visibles(), &anchos, ancla, marco, escala)
    }

    pub(super) fn pulsar(&mut self, p: Punto, ancla: Punto, marco: Rect, escala: u32) -> Respuesta {
        let puesta = self.colocar(ancla, marco, escala);
        match puesta.opcion_en(p) {
            Some(n) => Respuesta::Hacer(self.opciones[n].accion.clone()),
            None if puesta.caja.contiene(p) => Respuesta::Sigue,
            None => Respuesta::Fuera,
        }
    }

    pub(super) fn mover(&mut self, p: Punto, ancla: Punto, marco: Rect, escala: u32) -> bool {
        let puesta = self.colocar(ancla, marco, escala);
        let visibles = self.visibles();
        let nuevo = puesta.opcion_en(p).and_then(|n| visibles.iter().position(|v| *v == n));
        let cambia = nuevo != self.sobre;
        self.sobre = nuevo;
        cambia
    }

    pub(super) fn tecla(&mut self, vk: u32) -> Respuesta {
        let visibles = self.visibles();
        let baldosas = visibles.iter().filter(|n| self.opciones[**n].numero.is_some()).count();
        match vk {
            super::VK_ESCAPE if !self.busqueda.is_empty() => {
                self.busqueda.clear();
                self.elegido = None;
                Respuesta::Sigue
            }
            super::VK_ESCAPE => Respuesta::Fuera,
            super::VK_RETROCESO => {
                self.busqueda.pop();
                self.elegido = None;
                Respuesta::Sigue
            }
            super::VK_ENTRAR => {
                let i = self.elegido.unwrap_or(0);
                match visibles.get(i) {
                    Some(n) => Respuesta::Hacer(self.opciones[*n].accion.clone()),
                    None => Respuesta::Sigue,
                }
            }
            super::VK_DERECHA | super::VK_IZQUIERDA | super::VK_ABAJO | super::VK_ARRIBA => {
                self.elegido = mover_eleccion(self.elegido, vk, visibles.len(), baldosas);
                Respuesta::Sigue
            }
            _ => Respuesta::Sigue,
        }
    }

    /// Un numero del 1 al 8 elige su baldosa (si se ve); lo demas se
    /// escribe en el buscador.
    pub(super) fn caracter(&mut self, c: char) -> Respuesta {
        // Los numeros son teclas, no texto: ninguna opcion se busca por uno.
        if let Some(d) = c.to_digit(10) {
            let visibles = self.visibles();
            if let Some(n) = visibles.iter().find(|n| self.opciones[**n].numero == Some(d as u8)) {
                return Respuesta::Hacer(self.opciones[*n].accion.clone());
            }
            return Respuesta::Sigue;
        }
        if c >= ' ' {
            self.busqueda.push(c);
            self.elegido = None;
        }
        Respuesta::Sigue
    }
}

/// Mover lo elegido con las flechas: izquierda y derecha de uno en uno;
/// arriba y abajo saltan una fila entre las baldosas.
pub(super) fn mover_eleccion(actual: Option<usize>, vk: u32, cuantas: usize, baldosas: usize) -> Option<usize> {
    if cuantas == 0 {
        return None;
    }
    let Some(n) = actual else {
        return Some(0);
    };
    let col = COLUMNAS as usize;
    let nuevo = match vk {
        super::VK_DERECHA => n + 1,
        super::VK_IZQUIERDA => n.saturating_sub(1),
        super::VK_ABAJO if n + col < baldosas => n + col,
        super::VK_ABAJO if n < baldosas => baldosas,
        super::VK_ABAJO => n + 1,
        super::VK_ARRIBA if n >= baldosas && n > 0 && baldosas > 0 => (n - 1).min(baldosas - 1),
        super::VK_ARRIBA => n.saturating_sub(col),
        _ => n,
    };
    Some(nuevo.min(cuantas - 1))
}

/// La hoja ya colocada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Puesta {
    pub caja: Rect,
    pub buscador: Rect,
    /// Indice de `opciones` y su sitio.
    pub baldosas: Vec<(usize, Rect)>,
    pub cabecera_mas: Option<Rect>,
    pub chips: Vec<(usize, Rect)>,
    /// La linea de «nada coincide», si no se ve ninguna.
    pub vacio: Option<Rect>,
}

impl Puesta {
    pub(super) fn opcion_en(&self, p: Punto) -> Option<usize> {
        self.baldosas
            .iter()
            .chain(self.chips.iter())
            .find(|(_, r)| r.contiene(p))
            .map(|(n, _)| *n)
    }
}

/// Coloca la hoja: encima de `ancla` (el boton «+»), alineada con el y
/// dentro de la ventana. Lo que no se ve con lo escrito no ocupa sitio.
pub(super) fn colocar(
    opciones: &[Opcion],
    visibles: &[usize],
    anchos_chips: &[f32],
    ancla: Punto,
    marco: Rect,
    escala: u32,
) -> Puesta {
    let e = |v: u32| v * escala / 100;
    let ancho = e(ANCHO).min(marco.ancho.saturating_sub(e(16)));
    let dentro = ancho.saturating_sub(2 * e(RELLENO));
    // Primero todo con la caja en (0, 0); luego se mueve a su sitio.
    let x0 = e(RELLENO) as i32;
    let mut y = e(RELLENO) as i32;
    let buscador = Rect {
        x: x0,
        y,
        ancho: dentro,
        alto: e(BUSCADOR),
    };
    y = buscador.abajo() + e(HUECO) as i32;
    let col = COLUMNAS;
    let ancho_baldosa = dentro.saturating_sub((col - 1) * e(BALDOSA_HUECO)) / col;
    let mut baldosas = Vec::new();
    let de_baldosa: Vec<usize> = visibles.iter().copied().filter(|n| opciones[*n].numero.is_some()).collect();
    for (k, n) in de_baldosa.iter().enumerate() {
        let (f, c) = (k as u32 / col, k as u32 % col);
        baldosas.push((
            *n,
            Rect {
                x: x0 + (c * (ancho_baldosa + e(BALDOSA_HUECO))) as i32,
                y: y + (f * (e(BALDOSA_ALTO) + e(BALDOSA_HUECO))) as i32,
                ancho: ancho_baldosa,
                alto: e(BALDOSA_ALTO),
            },
        ));
    }
    if !de_baldosa.is_empty() {
        let filas = (de_baldosa.len() as u32).div_ceil(col);
        y += (filas * e(BALDOSA_ALTO) + (filas - 1) * e(BALDOSA_HUECO)) as i32;
    }
    let de_chip: Vec<usize> = visibles.iter().copied().filter(|n| opciones[*n].numero.is_none()).collect();
    let mut cabecera_mas = None;
    let mut chips = Vec::new();
    if !de_chip.is_empty() {
        y += e(HUECO) as i32;
        let cab = Rect {
            x: x0,
            y,
            ancho: dentro,
            alto: e(CABECERA_MAS),
        };
        cabecera_mas = Some(cab);
        y = cab.abajo() + e(CHIP_HUECO) as i32;
        let mut x = x0;
        for n in de_chip {
            let w = (anchos_chips.get(n).copied().unwrap_or(100.0).ceil() as u32 + e(24)).min(dentro);
            if x > x0 && x + w as i32 > x0 + dentro as i32 {
                x = x0;
                y += (e(CHIP_ALTO) + e(CHIP_HUECO)) as i32;
            }
            chips.push((
                n,
                Rect {
                    x,
                    y,
                    ancho: w,
                    alto: e(CHIP_ALTO),
                },
            ));
            x += (w + e(CHIP_HUECO)) as i32;
        }
        y += e(CHIP_ALTO) as i32;
    }
    let mut vacio = None;
    if visibles.is_empty() {
        let r = Rect {
            x: x0,
            y,
            ancho: dentro,
            alto: e(VACIO),
        };
        vacio = Some(r);
        y = r.abajo();
    }
    let alto = (y + e(RELLENO) as i32).max(0) as u32;
    // A su sitio: la base encima del boton, el borde izquierdo con el suyo.
    let limite_x = marco.ancho as i32 - ancho as i32 - e(8) as i32;
    let x = (ancla.x - e(48) as i32).min(limite_x).max(e(8) as i32);
    let y = (ancla.y - e(SOBRE_EL_BOTON) as i32 - alto as i32).max(e(8) as i32);
    let mover = |r: Rect| Rect {
        x: r.x + x,
        y: r.y + y,
        ..r
    };
    Puesta {
        caja: Rect { x, y, ancho, alto },
        buscador: mover(buscador),
        baldosas: baldosas.into_iter().map(|(n, r)| (n, mover(r))).collect(),
        cabecera_mas: cabecera_mas.map(mover),
        chips: chips.into_iter().map(|(n, r)| (n, mover(r))).collect(),
        vacio: vacio.map(mover),
    }
}

/// Pinta la hoja abierta.
pub(super) fn pintar(p: &Pintor, c: &Pinta, h: &Hoja, ancla: Punto, marco: Rect) {
    let (tema, escala) = (c.tema, c.escala);
    let e = escala as f32 / 100.0;
    // Primero se miden los chips: de eso depende donde cae cada uno.
    *h.anchos.borrow_mut() = h.opciones.iter().map(|o| p.medir_texto(&o.rotulo, TAM * e).0).collect();
    let puesta = h.colocar(ancla, marco, escala);
    let visibles = h.visibles();
    let resaltada = |n: usize| -> (bool, bool) {
        let k = visibles.iter().position(|v| *v == n);
        let sobre = k.is_some() && k == h.sobre;
        // Sin nada elegido, Intro hace la primera: se marca tambien.
        let elegida = k.is_some() && (k == h.elegido || (h.elegido.is_none() && k == Some(0) && !h.busqueda.is_empty()));
        (sobre, elegida)
    };

    let caja = rf(puesta.caja);
    let radio = RADIO as f32 * e;
    p.rellenar_redondeado(
        RectF {
            y: caja.y + 8.0 * e,
            ..encoger(caja, -2.0 * e)
        },
        radio + 2.0,
        con_alfa(hex(0x000000), 0.25),
    );
    p.rellenar_redondeado(caja, radio, tema.pildora_borde);
    p.rellenar_redondeado(encoger(caja, 1.0), radio - 1.0, tema.cabecera);

    // El buscador, con su anillo azul: es donde va lo que se teclea.
    let b = rf(puesta.buscador);
    p.rellenar_redondeado(encoger(b, -2.0 * e), 12.0 * e, super::AZUL_ELEGIDO);
    p.rellenar_redondeado(b, 10.0 * e, tema.chat);
    let lupa = 16.0 * e;
    p.icono(
        &mi::SEARCH,
        RectF {
            x: b.x + 12.0 * e,
            y: b.y + (b.alto - lupa) / 2.0,
            ancho: lupa,
            alto: lupa,
        },
        tema.apagado,
    );
    let tam_b = TAM_BUSCADOR * e;
    let (texto, color) = if h.busqueda.is_empty() {
        (h.pista.clone(), tema.apagado)
    } else {
        (format!("{}|", h.busqueda), tema.texto)
    };
    let (_, alto_b) = p.medir_texto("X", tam_b);
    let esc = vec!["Esc".to_string()];
    let ancho_esc = super::ancho_chapas(p, &esc, e);
    p.texto_linea(
        &texto,
        b.x + 36.0 * e,
        b.y + (b.alto - alto_b) / 2.0,
        tam_b,
        (b.ancho - 56.0 * e - ancho_esc).max(0.0),
        color,
    );
    super::pintar_chapas(p, tema, &esc, b.x + b.ancho - 10.0 * e, puesta.buscador, e, false);

    // Las baldosas: el cuadrado de color con su icono, el nombre debajo y
    // el numero arriba a la derecha.
    for (n, r) in &puesta.baldosas {
        let o = &h.opciones[*n];
        let (sobre, elegida) = resaltada(*n);
        if elegida {
            p.rellenar_redondeado(rf(*r), 14.0 * e, con_alfa(super::AZUL_ELEGIDO, 0.35));
        } else if sobre {
            p.rellenar_redondeado(rf(*r), 14.0 * e, con_alfa(tema.texto, 0.08));
        }
        let lado = ICONO_CAJA as f32 * e;
        let cuadro = RectF {
            x: r.x as f32 + (r.ancho as f32 - lado) / 2.0,
            y: r.y as f32 + 12.0 * e,
            ancho: lado,
            alto: lado,
        };
        p.rellenar_redondeado(cuadro, 14.0 * e, hex(o.color));
        let ic = 24.0 * e;
        p.icono(
            o.icono,
            RectF {
                x: cuadro.x + (lado - ic) / 2.0,
                y: cuadro.y + (lado - ic) / 2.0,
                ancho: ic,
                alto: ic,
            },
            hex(0xffffff),
        );
        let tam = TAM * e;
        let (w, alto) = p.medir_texto(&o.rotulo, tam);
        let w = w.min(r.ancho as f32 - 4.0 * e);
        p.texto_linea(
            &o.rotulo,
            r.x as f32 + (r.ancho as f32 - w) / 2.0,
            cuadro.y + lado + 8.0 * e,
            tam,
            r.ancho as f32 - 4.0 * e,
            tema.texto,
        );
        let _ = alto;
        if let Some(num) = o.numero {
            let chapa = vec![num.to_string()];
            super::pintar_chapas(
                p,
                tema,
                &chapa,
                r.derecha() as f32 - 8.0 * e,
                Rect {
                    y: r.y + (6.0 * e) as i32,
                    alto: (super::menu_v2::CHAPA_ALTO as f32 * e) as u32,
                    ..*r
                },
                e,
                false,
            );
        }
    }

    // «MÁS» con su raya, y los chips.
    if let Some(cab) = puesta.cabecera_mas {
        let tam = 12.0 * e;
        let (w, alto) = p.medir_texto(&h.titulo_mas, tam);
        p.texto_linea(
            &h.titulo_mas,
            cab.x as f32 + 4.0 * e,
            cab.y as f32 + (cab.alto as f32 - alto) / 2.0,
            tam,
            w + 2.0,
            tema.apagado,
        );
        p.rellenar(
            RectF {
                x: cab.x as f32 + w + 14.0 * e,
                y: cab.y as f32 + cab.alto as f32 / 2.0,
                ancho: (cab.ancho as f32 - w - 18.0 * e).max(0.0),
                alto: e.max(1.0),
            },
            tema.separador,
        );
    }
    for (n, r) in &puesta.chips {
        let o = &h.opciones[*n];
        let (sobre, elegida) = resaltada(*n);
        let fondo = if elegida {
            super::AZUL_ELEGIDO
        } else if sobre {
            con_alfa(tema.texto, 0.16)
        } else {
            con_alfa(tema.texto, 0.08)
        };
        p.rellenar_redondeado(rf(*r), r.alto as f32 / 2.0, fondo);
        let tam = TAM * e;
        let (w, alto) = p.medir_texto(&o.rotulo, tam);
        p.texto_linea(
            &o.rotulo,
            r.x as f32 + (r.ancho as f32 - w) / 2.0,
            r.y as f32 + (r.alto as f32 - alto) / 2.0,
            tam,
            r.ancho as f32,
            if elegida { hex(0xffffff) } else { tema.texto },
        );
    }
    if let Some(r) = puesta.vacio {
        let tam = TAM * e;
        let (_, alto) = p.medir_texto(&h.nada, tam);
        p.texto_linea(
            &h.nada,
            r.x as f32 + 4.0 * e,
            r.y as f32 + (r.alto as f32 - alto) / 2.0,
            tam,
            r.ancho as f32,
            tema.apagado,
        );
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_store::Idioma;

    fn hoja() -> Hoja {
        Hoja::nueva(&Catalogo::nuevo(Idioma::Espanol))
    }

    fn marco() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1024,
            alto: 800,
        }
    }

    #[test]
    fn ocho_baldosas_numeradas_y_los_chips_debajo() {
        let h = hoja();
        let numeros: Vec<u8> = h.opciones.iter().filter_map(|o| o.numero).collect();
        assert_eq!(numeros, [1, 2, 3, 4, 5, 6, 7, 8]);
        let p = h.colocar(Punto { x: 300, y: 740 }, marco(), 100);
        assert_eq!(p.baldosas.len(), 8);
        // Dos filas de cuatro.
        assert_eq!(p.baldosas[0].1.y, p.baldosas[3].1.y);
        assert!(p.baldosas[4].1.y > p.baldosas[0].1.y);
        // Los chips por debajo de las baldosas y dentro de la caja.
        let ultima = p.baldosas[7].1.abajo();
        assert!(p.chips.iter().all(|(_, r)| r.y > ultima && r.abajo() <= p.caja.abajo()));
        // La hoja apoya su base encima del boton.
        assert!(p.caja.abajo() <= 740);
        // Caso negativo: nada se sale de la ventana.
        assert!(p.caja.x >= 0 && p.caja.derecha() <= 1024 && p.caja.y >= 0);
    }

    #[test]
    fn cada_entrada_hace_lo_del_clip_de_antes() {
        let h = hoja();
        let acciones: Vec<&Accion> = h.opciones.iter().map(|o| &o.accion).collect();
        for a in [
            Accion::AdjImagen,
            Accion::AdjArchivo,
            Accion::AdjLienzo,
            Accion::AdjNotaMd,
            Accion::AdjTabla,
            Accion::AdjPagina,
            Accion::AdjProyectos,
            Accion::Conversacion,
            Accion::Telepronter,
            Accion::Pronunciar,
            Accion::AdjDelMovil,
        ] {
            assert!(acciones.contains(&&a), "falta {a:?}");
        }
        // Las siete mini-apps del movil, cada una.
        for cual in pixpin_proyecto::mini::TODAS {
            assert!(acciones.contains(&&Accion::AdjMini(cual)), "falta {cual}");
        }
        // Caso negativo: ninguna repetida.
        for (n, a) in acciones.iter().enumerate() {
            assert!(!acciones[..n].contains(a), "{a:?} dos veces");
        }
    }

    #[test]
    fn el_buscador_filtra_sin_tildes_y_por_sinonimos() {
        let mut h = hoja();
        for c in "cronometro".chars() {
            h.caracter(c);
        }
        let v = h.visibles();
        assert_eq!(v.len(), 1);
        assert_eq!(h.opciones[v[0]].accion, Accion::AdjMini(pixpin_proyecto::mini::CRONOMETRO));
        // «pdf» encuentra Archivo, e Intro lo hace.
        h.busqueda = "PDF".into();
        assert_eq!(h.tecla(super::super::VK_ENTRAR), Respuesta::Hacer(Accion::AdjArchivo));
        // Caso negativo: lo que no esta no deja nada, e Intro no hace nada.
        h.busqueda = "zzzz".into();
        assert!(h.visibles().is_empty());
        assert_eq!(h.tecla(super::super::VK_ENTRAR), Respuesta::Sigue);
        let p = h.colocar(Punto { x: 300, y: 740 }, marco(), 100);
        assert!(p.vacio.is_some() && p.baldosas.is_empty() && p.chips.is_empty());
    }

    #[test]
    fn los_numeros_eligen_su_baldosa() {
        let mut h = hoja();
        assert_eq!(h.caracter('1'), Respuesta::Hacer(Accion::AdjImagen));
        assert_eq!(h.caracter('6'), Respuesta::Hacer(Accion::AdjTabla));
        assert_eq!(h.caracter('8'), Respuesta::Hacer(Accion::AdjCaptura));
        // Casos negativos: el 9 y el 0 no son de nadie y no se escriben; y
        // un numero cuya baldosa no se ve con lo buscado no hace nada.
        assert_eq!(h.caracter('9'), Respuesta::Sigue);
        assert!(h.busqueda.is_empty());
        h.busqueda = "tabla".into();
        assert_eq!(h.caracter('1'), Respuesta::Sigue);
    }

    #[test]
    fn esc_borra_lo_buscado_y_luego_cierra() {
        let mut h = hoja();
        h.caracter('t');
        assert_eq!(h.tecla(super::super::VK_ESCAPE), Respuesta::Sigue);
        assert!(h.busqueda.is_empty());
        assert_eq!(h.tecla(super::super::VK_ESCAPE), Respuesta::Fuera);
    }

    #[test]
    fn un_clic_en_una_baldosa_la_hace_y_fuera_cierra() {
        let mut h = hoja();
        let ancla = Punto { x: 300, y: 740 };
        let p = h.colocar(ancla, marco(), 100);
        let (n, r) = p.baldosas[2];
        let dentro = Punto { x: r.x + 5, y: r.y + 5 };
        assert_eq!(h.pulsar(dentro, ancla, marco(), 100), Respuesta::Hacer(h.opciones[n].accion.clone()));
        // Caso negativo: el relleno no hace nada y fuera cierra.
        let relleno = Punto { x: p.caja.x + 3, y: p.caja.y + 3 };
        assert_eq!(h.pulsar(relleno, ancla, marco(), 100), Respuesta::Sigue);
        assert_eq!(h.pulsar(Punto { x: 2, y: 790 }, ancla, marco(), 100), Respuesta::Fuera);
    }

    #[test]
    fn las_flechas_saltan_por_filas_entre_baldosas() {
        use super::super::{VK_ABAJO, VK_ARRIBA, VK_DERECHA, VK_IZQUIERDA};
        assert_eq!(mover_eleccion(None, VK_DERECHA, 19, 8), Some(0));
        assert_eq!(mover_eleccion(Some(1), VK_ABAJO, 19, 8), Some(5));
        assert_eq!(mover_eleccion(Some(5), VK_ABAJO, 19, 8), Some(8), "de la ultima fila a los chips");
        assert_eq!(mover_eleccion(Some(9), VK_ARRIBA, 19, 8), Some(7));
        assert_eq!(mover_eleccion(Some(5), VK_ARRIBA, 19, 8), Some(1));
        // Casos negativos: no se sale por los extremos.
        assert_eq!(mover_eleccion(Some(0), VK_IZQUIERDA, 19, 8), Some(0));
        assert_eq!(mover_eleccion(Some(18), VK_DERECHA, 19, 8), Some(18));
        assert_eq!(mover_eleccion(Some(0), VK_DERECHA, 0, 0), None);
    }
}
