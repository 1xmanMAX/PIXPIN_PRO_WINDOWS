//! **Los temas de la reunion en «Grabar clase»** (28-oct-2026, el usuario:
//! «la persona crea una lista de TEMAS a tratar en la reunion; durante la
//! reunion … Jev marca que temas ya se trataron»).
//!
//! - **Preparar**: la fila «Temas de la reunion» abre su pantalla: se
//!   escribe un tema e Intro, se pegan varios (uno por linea: `Ctrl+V` o
//!   «Pegar») o se toman los pendientes de una lista de tareas del chat
//!   («De una lista», que va pasando de una a otra). Los que se pasaron
//!   «a la proxima reunion» al estudiar la anterior salen ya puestos.
//! - **Grabando**: en la barra, «Temas 2/5» abre debajo la lista con sus
//!   casillas. Un clic marca o desmarca a mano (**lo hecho a mano manda**:
//!   Jev ya no toca ese tema). Lo que marca Jev ([`super::escucha`]) sale
//!   con ✓ y su minuto, la fila se ilumina un momento y la barra dice
//!   «✓ Tema 2 tratado · 12:30». Sin Jev o si falla, la linea de arriba de
//!   la lista dice por que y se marcan a mano.
//! - **Al guardar**: `resto["temas"]` en la nota y la lista de tareas de la
//!   reunion (`crate::jev::temas::guardar_lista`), enlazada en
//!   `resto["temasLista"]`. Todo pasa tambien por el diario de la
//!   grabacion: si PixPin se cae, la clase recuperada los lleva.

use std::time::{Duration, Instant};

use pixpin_proyecto::temas::{self, Tema};
use pixpin_render::{Color, Pintor, RectF};
use pixpin_typesafe::temas::Pendiente;

use super::escucha::{Aviso, Escucha, NoSeEscucha, Vuelta};
use super::{
    Accion, Botones, Estado, Fase, OSCURO, Pedido, ROJO_GRABAR, Rotulos, VERDE, boton, con,
    con_alfa, dentro, mas_claro, mi, minuto,
};
use crate::jev::temas::{ListaDeTemas, SinJev};

/// Lo que dura el brillo de un tema recien marcado por Jev.
pub(super) const BRILLO: Duration = Duration::from_millis(1800);
/// Las filas de la lista de la barra que se ven (las demas, «+N»).
const FILAS_EN_LA_BARRA: usize = 10;
/// Lo alto de cada fila de esa lista y de su cabecera, en logicos.
const ALTO_FILA: u32 = 26;
const ALTO_CABECERA: u32 = 24;
/// Las filas de la pantalla de los temas.
const FILAS_EN_PREPARAR: usize = 8;

/// El segundo hueco de un rotulo con dos cosas dentro.
pub(super) const HUECO2: &str = "\u{2}";

/// Lo que se le suma a la barra con temas: el boton «Temas».
pub(super) const ANCHO_BOTON: u32 = 80;

/// **Lo alto de la lista de la barra**, en logicos (0 si esta plegada).
pub(super) fn alto_del_panel(estado: &Estado) -> u32 {
    if !estado.panel_temas || estado.temas.is_empty() {
        return 0;
    }
    let filas = estado.temas.len().min(FILAS_EN_LA_BARRA + 1) as u32;
    8 + ALTO_CABECERA + filas * ALTO_FILA + 8
}

/// Los temas de una lista elegida, como pendientes para Jev.
pub(super) fn pendientes(v: &[Tema]) -> Vec<Pendiente> {
    temas::para_jev(v)
        .into_iter()
        .map(|t| Pendiente {
            id: t.id.clone(),
            texto: t.texto.clone(),
        })
        .collect()
}

impl Rotulos {
    /// Por que no se escucha, para la cabecera de la lista.
    fn de_no_escuchar(&self, n: NoSeEscucha) -> String {
        match n {
            NoSeEscucha::Jev(SinJev::SinClave) => self.temas_sin_jev.clone(),
            NoSeEscucha::Jev(SinJev::Apagado) => self.temas_apagado.clone(),
            NoSeEscucha::Modelo | NoSeEscucha::Pcm => self.temas_sin_modelo.clone(),
        }
    }

    fn de_un_aviso(&self, a: Aviso) -> String {
        match a {
            Aviso::Escuchando => self.temas_escuchando.clone(),
            Aviso::SinModelo => self.temas_sin_modelo.clone(),
            Aviso::JevFalla => self.temas_jev_fallo.clone(),
            Aviso::Cargado => self.temas_cargado.clone(),
        }
    }

    /// «✓ Tema 2 tratado · 12:30».
    fn tratado(&self, n: usize, ms: i64) -> String {
        self.temas_tratado
            .replace(super::HUECO, &n.to_string())
            .replace(HUECO2, &minuto(ms))
    }
}

impl Estado {
    /// Los temas que esperaban de la reunion anterior, al abrir Preparar.
    pub(super) fn con_los_de_la_proxima(mut self, pedido: &Pedido, r: &Rotulos) -> Estado {
        let v = crate::jev::temas::para_la_proxima(pedido.ubicacion.raiz());
        if temas::anadir_varios(&mut self.temas, &v) > 0 {
            self.temas_de_la_proxima = true;
            self.avisar(con(&r.temas_anteriores, &self.temas.len().to_string()));
        }
        self
    }

    /// **Una tecla en la pantalla de los temas.** Devuelve la accion que
    /// toque (Intro sin nada escrito y Escape vuelven a Preparar).
    pub(super) fn tecla_de_temas(&mut self, vk: u32, ctrl: bool, r: &Rotulos) -> Option<Accion> {
        const VK_RETROCESO: u32 = 0x08;
        const VK_V: u32 = 0x56;
        match vk {
            super::VK_ESCAPE => Some(Accion::TemasListo),
            super::VK_ENTRAR if self.escrito.trim().is_empty() => Some(Accion::TemasListo),
            super::VK_ENTRAR => {
                let escrito = std::mem::take(&mut self.escrito);
                if temas::anadir(&mut self.temas, &escrito).is_none() {
                    // Repetido o ya no caben: se queda escrito para verlo.
                    self.escrito = escrito;
                }
                None
            }
            VK_RETROCESO => {
                self.escrito.pop();
                None
            }
            VK_V if ctrl => {
                self.pegar_temas(r);
                None
            }
            _ => None,
        }
    }

    /// Un caracter escrito en la pantalla de los temas.
    pub(super) fn escribir_tema(&mut self, c: char) {
        if !c.is_control() && self.escrito.chars().count() < temas::LARGO_MAXIMO {
            self.escrito.push(c);
        }
    }

    /// «Pegar»: lo del portapapeles, un tema por linea.
    pub(super) fn pegar_temas(&mut self, r: &Rotulos) {
        let Some(texto) = pixpin_codec::portapapeles::leer_texto_solo() else {
            self.avisar(r.temas_nada_que_pegar.clone());
            return;
        };
        let n = temas::anadir_varios(&mut self.temas, &temas::de_texto(&texto));
        self.avisar(con(&r.temas_pegados, &n.to_string()));
    }

    /// «De una lista»: la siguiente lista de tareas con algo pendiente
    /// (primero las de este chat); sus temas cambian los de ahora y la
    /// reunion queda enlazada a ella.
    pub(super) fn temas_de_una_lista(&mut self, pedido: &Pedido, r: &Rotulos) {
        if self.listas_de_temas.is_none() {
            let proyecto = pedido
                .proyectos
                .get(self.proyecto)
                .map(|(id, _)| id.as_str())
                .unwrap_or("");
            self.listas_de_temas =
                Some(crate::jev::temas::listas(pedido.ubicacion.raiz(), proyecto));
            self.lista_de_temas = 0;
        }
        let listas: &[ListaDeTemas] = self.listas_de_temas.as_deref().unwrap_or_default();
        if listas.is_empty() {
            self.avisar(r.temas_sin_listas.clone());
            return;
        }
        let l = listas[self.lista_de_temas % listas.len()].clone();
        self.lista_de_temas += 1;
        self.temas.clear();
        temas::anadir_varios(&mut self.temas, &l.temas);
        self.temas_lista = Some((l.proyecto, l.codigo));
        self.temas_de_la_proxima = false;
        self.avisar(
            r.temas_lista
                .replace(super::HUECO, &l.titulo)
                .replace(HUECO2, &self.temas.len().to_string()),
        );
    }

    /// «Vaciar».
    pub(super) fn vaciar_temas(&mut self) {
        self.temas.clear();
        self.temas_lista = None;
        self.temas_de_la_proxima = false;
        self.escrito.clear();
    }

    /// **Un clic en un tema** de la barra: lo marca o desmarca a mano, en el
    /// minuto de ahora. Jev ya no lo toca.
    pub(super) fn tocar_tema(&mut self, i: usize) {
        let ms = match &self.fase {
            Fase::Grabando(g) => Some(g.llevado_ms()),
            _ => None,
        };
        let Some(t) = self.temas.get(i) else {
            return;
        };
        let (id, hecho) = (t.id.clone(), !t.hecho);
        if temas::a_mano(&mut self.temas, &id, hecho, ms) {
            self.recien_tratados.retain(|(x, _)| *x != id);
            self.temas_cambiados();
        }
    }

    /// Lo que hay que hacer cada vez que cambian los temas grabando: la
    /// escucha deja de preguntar por lo ya hecho y el diario lo apunta.
    fn temas_cambiados(&mut self) {
        if let Some(e) = &self.escucha {
            e.pendientes(pendientes(&self.temas));
        }
        self.apuntar_temas();
    }

    /// **Empieza a escuchar** con la grabadora ya abierta (si hay temas).
    pub(super) fn empezar_escucha(
        &mut self,
        g: &pixpin_audio::Grabadora,
        pedido: &Pedido,
        r: &Rotulos,
    ) {
        if self.temas.is_empty() {
            return;
        }
        let titulo = con(
            &self.plantilla,
            &super::fecha_del_nombre(pixpin_shell::entorno::ahora_local_ms()),
        );
        let agenda: Vec<String> = self.temas.iter().map(|t| t.texto.clone()).collect();
        match Escucha::empezar(
            g,
            &pedido.ubicacion,
            pedido.idioma,
            titulo,
            agenda,
            pendientes(&self.temas),
        ) {
            Ok(e) => self.escucha = Some(e),
            Err(n) => {
                tracing::info!(?n, "temas: sin escucha en vivo; a mano");
                let texto = r.de_no_escuchar(n);
                self.estado_escucha = Some(texto.clone());
                self.avisar(texto);
            }
        }
    }

    /// **Lo que llega de la escucha**: lo que Jev marco (si nadie lo toco)
    /// y como va.
    pub(super) fn de_la_escucha(&mut self, r: &Rotulos) {
        let Some(e) = &self.escucha else {
            return;
        };
        let mut cambio = false;
        for v in e.recoger() {
            match v {
                Vuelta::Tratado(t, tarde) => {
                    // Lo que tardo el ✓ desde que se dejo de hablar: en las
                    // compilaciones de depuracion, en la cabecera de la lista
                    // (en todas, al registro, `escucha`).
                    if cfg!(debug_assertions) {
                        self.estado_escucha = Some(format!(
                            "{} · ✓ {:.1} s",
                            r.temas_escuchando,
                            tarde.as_secs_f32()
                        ));
                    }
                    if temas::por_jev(&mut self.temas, &t.id, t.ms, t.p) {
                        cambio = true;
                        let n = self
                            .temas
                            .iter()
                            .position(|x| x.id == t.id)
                            .map_or(0, |i| i + 1);
                        self.avisar(r.tratado(n, t.ms));
                        self.destello_temas = Some(Instant::now());
                        self.recien_tratados.push((t.id, Instant::now()));
                    }
                }
                Vuelta::Aviso(a) => {
                    let texto = r.de_un_aviso(a);
                    if a != Aviso::Escuchando && self.estado_escucha.as_ref() != Some(&texto) {
                        self.avisar(texto.clone());
                    }
                    self.estado_escucha = Some(texto);
                }
            }
        }
        if cambio {
            self.temas_cambiados();
        }
        self.recien_tratados.retain(|(_, d)| d.elapsed() < BRILLO);
    }

    /// **Para la escucha** (al terminar o tirar): recoge lo ultimo y la
    /// suelta, que cancela Whisper en el acto. La transcripcion de despues
    /// es la de siempre.
    pub(super) fn acabar_escucha(&mut self, r: Option<&Rotulos>) {
        if let Some(r) = r {
            self.de_la_escucha(r);
        }
        self.escucha = None;
    }
}

// --- Pintar -----------------------------------------------------------------

/// **La fila de Preparar**: «Temas de la reunion · 3», que abre su pantalla.
pub(super) fn pintar_fila(
    p: &Pintor,
    caja: RectF,
    e: f32,
    estado: &Estado,
    r: &Rotulos,
    botones: &mut Botones<Accion>,
) {
    let tema = &OSCURO;
    let rotulo = if estado.temas.is_empty() {
        r.temas_ninguno.clone()
    } else {
        con(&r.temas_n, &estado.temas.len().to_string())
    };
    super::fila_sin_pildora(
        p,
        botones,
        caja,
        Some(Accion::Temas),
        &mi::CHECKLIST,
        &rotulo,
        28.0 * e,
        e,
    );
    let lado = 20.0 * e;
    p.icono(
        &mi::CHEVRON_RIGHT,
        RectF {
            x: caja.x + caja.ancho - 32.0 * e,
            y: caja.y + (caja.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        tema.campo_apagado,
    );
}

/// **La pantalla de los temas** (dentro de Preparar).
pub(super) fn pintar_editor(
    p: &Pintor,
    w: f32,
    h: f32,
    e: f32,
    estado: &Estado,
    r: &Rotulos,
    llevado: Duration,
    botones: &mut Botones<Accion>,
) {
    let tema = &OSCURO;
    let m = 18.0 * e;
    p.texto_linea(
        &r.temas_titulo,
        m,
        16.0 * e,
        17.0 * e,
        w - 2.0 * m,
        tema.texto,
    );
    let (sub, color) = match &estado.aviso {
        Some((a, _)) => (a.as_str(), VERDE),
        None => (r.temas_sub.as_str(), tema.campo_apagado),
    };
    p.texto_linea(sub, m, 42.0 * e, 12.0 * e, w - 2.0 * m, color);
    // El campo: lo escrito con su cursor, o la pista.
    let campo = RectF {
        x: m,
        y: 68.0 * e,
        ancho: w - 2.0 * m,
        alto: 40.0 * e,
    };
    p.rellenar_redondeado(campo, 10.0 * e, tema.campo);
    let tam = 14.0 * e;
    let cursor = (llevado.as_millis() % 1000) < 550;
    let (texto, tinta) = if estado.escrito.is_empty() {
        (r.temas_escribe.clone(), tema.campo_apagado)
    } else {
        (
            format!("{}{}", estado.escrito, if cursor { "▏" } else { " " }),
            tema.texto,
        )
    };
    let (_, th) = p.medir_texto(&texto, tam);
    p.texto_linea(
        &texto,
        campo.x + 12.0 * e,
        campo.y + (campo.alto - th) / 2.0,
        tam,
        campo.ancho - 24.0 * e,
        tinta,
    );
    // Pegar · De una lista · Vaciar.
    let y = 116.0 * e;
    let n = if estado.temas.is_empty() { 2.0 } else { 3.0 };
    let hueco = 8.0 * e;
    let ancho = (w - 2.0 * m - hueco * (n - 1.0)) / n;
    let caja = |i: f32| RectF {
        x: m + i * (ancho + hueco),
        y,
        ancho,
        alto: 34.0 * e,
    };
    boton(
        p,
        botones,
        caja(0.0),
        Accion::TemasPegar,
        &mi::CONTENT_PASTE,
        &r.temas_pegar,
        tema.campo,
        tema.texto,
        e,
    );
    boton(
        p,
        botones,
        caja(1.0),
        Accion::TemasDeLista,
        &mi::CHECKLIST,
        &r.temas_de_lista,
        tema.campo,
        tema.texto,
        e,
    );
    if !estado.temas.is_empty() {
        boton(
            p,
            botones,
            caja(2.0),
            Accion::TemasVaciar,
            &mi::CLOSE,
            &r.temas_vaciar,
            tema.campo,
            tema.campo_apagado,
            e,
        );
    }
    // La lista, numerada; ✕ al pasar por encima.
    let mut y = 160.0 * e;
    for (i, t) in estado.temas.iter().enumerate().take(FILAS_EN_PREPARAR) {
        let fila = RectF {
            x: m,
            y,
            ancho: w - 2.0 * m,
            alto: 26.0 * e,
        };
        let encima = dentro(fila, botones.raton);
        if encima {
            p.rellenar_redondeado(fila, 7.0 * e, tema.campo);
        }
        p.texto_linea(
            &format!("{}. {}", i + 1, t.texto),
            fila.x + 8.0 * e,
            fila.y + 5.0 * e,
            13.0 * e,
            fila.ancho - 40.0 * e,
            tema.texto,
        );
        if encima {
            let x = RectF {
                x: fila.x + fila.ancho - 26.0 * e,
                y: fila.y + 3.0 * e,
                ancho: 20.0 * e,
                alto: 20.0 * e,
            };
            p.icono(
                &mi::CLOSE,
                x,
                if dentro(x, botones.raton) {
                    ROJO_GRABAR
                } else {
                    tema.campo_apagado
                },
            );
            botones.zona(x, Accion::QuitarTema(i));
        }
        y += 28.0 * e;
    }
    if estado.temas.len() > FILAS_EN_PREPARAR {
        p.texto(
            &format!("+{}", estado.temas.len() - FILAS_EN_PREPARAR),
            m + 8.0 * e,
            y + 4.0 * e,
            13.0 * e,
            tema.campo_apagado,
        );
    }
    // «Listo», abajo.
    let listo = RectF {
        x: m,
        y: h - 54.0 * e,
        ancho: w - 2.0 * m,
        alto: 40.0 * e,
    };
    boton(
        p,
        botones,
        listo,
        Accion::TemasListo,
        &mi::CHECK_CIRCLE,
        &r.temas_listo,
        tema.enviar,
        Color::BLANCO,
        e,
    );
}

/// **El boton «Temas 2/5» de la barra**: abre o pliega la lista. Verde un
/// momento cuando Jev marca uno.
pub(super) fn pintar_boton(
    p: &Pintor,
    caja: RectF,
    e: f32,
    estado: &Estado,
    activo: bool,
    botones: &mut Botones<Accion>,
) {
    let tema = &OSCURO;
    let (hechos, total) = temas::cuenta(&estado.temas);
    let brilla = estado
        .destello_temas
        .is_some_and(|d| d.elapsed() < Duration::from_millis(900));
    let (fondo, tinta) = if brilla {
        (VERDE, Color::BLANCO)
    } else if estado.panel_temas {
        (mas_claro(tema.campo), tema.texto)
    } else {
        (tema.campo, tema.texto)
    };
    super::boton_si(
        p,
        botones,
        caja,
        Accion::PanelTemas,
        &mi::CHECKLIST,
        &format!("{hechos}/{total}"),
        fondo,
        tinta,
        e,
        activo,
    );
}

/// **La lista de los temas debajo de la barra**, con su casilla, su minuto
/// y lo que dice la escucha arriba.
pub(super) fn pintar_panel(
    p: &Pintor,
    w: f32,
    e: f32,
    estado: &Estado,
    r: &Rotulos,
    botones: &mut Botones<Accion>,
) {
    if alto_del_panel(estado) == 0 {
        return;
    }
    let tema = &OSCURO;
    let x0 = 16.0 * e;
    let ancho = w - 32.0 * e;
    let mut y = 92.0 * e;
    // Una raya entre la barra y la lista.
    p.rellenar_redondeado(
        RectF {
            x: x0,
            y: y - 4.0 * e,
            ancho,
            alto: 1.0 * e,
        },
        0.5 * e,
        tema.campo,
    );
    let cabecera = estado
        .estado_escucha
        .clone()
        .unwrap_or_else(|| r.temas_titulo.clone());
    p.texto_linea(
        &cabecera,
        x0,
        y + 4.0 * e,
        12.0 * e,
        ancho,
        tema.campo_apagado,
    );
    y += ALTO_CABECERA as f32 * e;
    for (i, t) in estado.temas.iter().enumerate().take(FILAS_EN_LA_BARRA) {
        let fila = RectF {
            x: x0,
            y,
            ancho,
            alto: (ALTO_FILA - 2) as f32 * e,
        };
        // El brillo de lo recien marcado por Jev se apaga poco a poco.
        let brillo = estado
            .recien_tratados
            .iter()
            .find(|(id, _)| *id == t.id)
            .map(|(_, d)| 1.0 - d.elapsed().as_secs_f32() / BRILLO.as_secs_f32());
        if let Some(b) = brillo.filter(|b| *b > 0.0) {
            p.rellenar_redondeado(fila, 6.0 * e, con_alfa(VERDE, 0.35 * b));
        } else if dentro(fila, botones.raton) {
            p.rellenar_redondeado(fila, 6.0 * e, tema.campo);
        }
        let lado = 16.0 * e;
        p.icono(
            if t.hecho {
                &mi::CHECK_BOX
            } else {
                &mi::CHECK_BOX_OUTLINE_BLANK
            },
            RectF {
                x: fila.x + 4.0 * e,
                y: fila.y + (fila.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            if t.hecho { VERDE } else { tema.campo_apagado },
        );
        // A la derecha, el minuto en que se trato (y «Jev» si lo marco el).
        let tam = 12.0 * e;
        let derecha = match (t.hecho, t.ms) {
            (true, Some(ms)) if t.por_jev => format!("Jev · {}", minuto(ms)),
            (true, Some(ms)) => minuto(ms),
            (true, None) if t.por_jev => "Jev".to_string(),
            _ => String::new(),
        };
        let (dw, th) = p.medir_texto(&derecha, tam);
        if !derecha.is_empty() {
            p.texto(
                &derecha,
                fila.x + fila.ancho - dw - 6.0 * e,
                fila.y + (fila.alto - th) / 2.0,
                tam,
                if t.por_jev { VERDE } else { tema.campo_apagado },
            );
        }
        let texto = format!("{}. {}", i + 1, t.texto);
        let (_, th) = p.medir_texto(&texto, 13.0 * e);
        p.texto_linea(
            &texto,
            fila.x + 28.0 * e,
            fila.y + (fila.alto - th) / 2.0,
            13.0 * e,
            fila.ancho - 40.0 * e - dw,
            if t.hecho {
                tema.campo_apagado
            } else {
                tema.texto
            },
        );
        botones.zona(fila, Accion::Tema(i));
        y += ALTO_FILA as f32 * e;
    }
    if estado.temas.len() > FILAS_EN_LA_BARRA {
        p.texto(
            &format!("+{}", estado.temas.len() - FILAS_EN_LA_BARRA),
            x0 + 28.0 * e,
            y + 4.0 * e,
            12.0 * e,
            tema.campo_apagado,
        );
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_pendientes_para_jev_son_lo_que_nadie_toco() {
        let mut v = vec![
            Tema::nuevo("t1", "A"),
            Tema::nuevo("t2", "B"),
            Tema::nuevo("t3", "C"),
        ];
        temas::a_mano(&mut v, "t1", true, Some(0));
        temas::a_mano(&mut v, "t2", false, None);
        temas::por_jev(&mut v, "t3", 0, 0.9);
        assert!(pendientes(&v).is_empty());
        let v = vec![Tema::nuevo("t9", "D")];
        assert_eq!(
            pendientes(&v),
            vec![Pendiente {
                id: "t9".into(),
                texto: "D".into()
            }]
        );
    }
}
