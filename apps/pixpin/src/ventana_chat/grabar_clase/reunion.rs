//! **Lo demas que se oye en la reunion, en la barra** (29-oct-2026,
//! `docs/jev.md`, «En la reunión»). Lo que sale de cada frase
//! ([`super::escucha`], `pixpin_typesafe::en_la_reunion`) llega aqui y:
//!
//! - **Tareas dichas** y **fechas al horario**: una ficha pequena debajo de
//!   la barra («＋ Tarea: mandar la cotización · vie 30/10», «📅 Evento: nos
//!   vemos el martes a las 3 · 3/11 15:00») con ✓ y ✕. Nada se apunta sin el
//!   ✓: la tarea va al Inbox de Tareas con su urgencia, su `⏰` y una nota con
//!   el responsable y la reunion; el evento, una tarea con su hora, que sale
//!   en el «Horario» de Tareas (`crate::jev::en_vivo`).
//! - **Urgencia en vivo**: lo unico que se aplica solo (la fecha nueva y lo
//!   que la apura, en Tareas), con su aviso y una ficha con «Deshacer».
//! - **Tema olvidado**: cerca del final previsto («Duración prevista» en
//!   Preparar), una ficha con los temas que faltan.
//! - **Decisiones**, **preguntas sin respuesta** y **minutos por tema**: se
//!   van apuntando y, al guardar, van a la nota (`acta_viva`), que es lo
//!   que leen «Estudiar la clase», la pagina compartida y el movil.
//!
//! Las fichas se ensenan de la mas nueva a la mas vieja, como mucho
//! [`FICHAS_A_LA_VISTA`]; las que no caben esperan detras. Una propuesta que
//! nadie toco queda como `pendiente` en la nota.

use std::time::Instant;

use pixpin_proyecto::acta_viva::{
    Acta, CambioDeUrgencia, Dicho, EstadoPropuesta, Propuesta, TiempoDeTema, TiempoPorTema, Tipo,
};
use pixpin_render::{Color, Pintor, RectF};
use pixpin_store::Catalogo;
use pixpin_typesafe::compromisos::Responsable;
use pixpin_typesafe::en_la_reunion::Suceso;
use pixpin_typesafe::fechas::Momento;
use pixpin_typesafe::tiempo_de_temas::Aviso as AvisoDeTemas;
use pixpin_typesafe::urgencia::Prioridad;
use pixpin_typesafe::urgencia_viva::TareaViva;

use super::{Accion, Botones, Estado, Fase, HUECO, OSCURO, Pedido, VERDE, dentro, mas_claro, mi};
use crate::caja_dibujo::hex;
use crate::jev::en_vivo::{self, Deshacer, TareaEnLista};

/// Las fichas que se ven a la vez debajo de la barra.
pub(super) const FICHAS_A_LA_VISTA: usize = 3;
/// Las que se guardan como mucho (las mas viejas se van: la propuesta
/// sigue en la nota como `pendiente`).
const MAXIMO_FICHAS: usize = 12;
/// Lo alto de cada ficha, en logicos.
const ALTO_FICHA: u32 = 30;
/// Las duraciones previstas que se ofrecen en Preparar, en minutos.
const DURACIONES: [u32; 7] = [15, 30, 45, 60, 90, 120, 180];

const AMBAR: Color = hex(0xf0a020);

/// Lo que dice esta parte de la barra, en el idioma de la app.
#[derive(Debug, Clone, Default)]
pub(crate) struct Rotulos {
    tarea: String,
    evento: String,
    deshacer: String,
    decision: String,
    tarea_apuntada: String,
    evento_apuntado: String,
    deshecho: String,
    no_se_apunto: String,
    falla: String,
    /// Con [`HUECO`] en los minutos y [`super::temas::HUECO2`] en los temas.
    queda_poco: String,
    /// Con [`HUECO`] en los temas.
    cumplido: String,
    cierran: String,
    /// Con [`HUECO`] en la fecha.
    fecha_nueva: String,
    /// Las prioridades: urgente, alta, normal, baja.
    prioridades: [String; 4],
    duracion: String,
    /// Con [`HUECO`] en los minutos.
    duracion_n: String,
}

impl Rotulos {
    pub(crate) fn de(t: &Catalogo) -> Rotulos {
        let con = |clave: &str, nombres: &[&str]| {
            let mut args = fluent_bundle::FluentArgs::new();
            for (n, h) in nombres.iter().zip([HUECO, super::temas::HUECO2]) {
                args.set(n.to_string(), h);
            }
            t.t_args(clave, &args)
        };
        Rotulos {
            tarea: t.t("clase-vivo-tarea"),
            evento: t.t("clase-vivo-evento"),
            deshacer: t.t("clase-vivo-deshacer"),
            decision: con("clase-vivo-decision", &["minuto"]),
            tarea_apuntada: t.t("clase-vivo-tarea-apuntada"),
            evento_apuntado: t.t("clase-vivo-evento-apuntado"),
            deshecho: t.t("clase-vivo-deshecho"),
            no_se_apunto: t.t("clase-vivo-no-se-apunto"),
            falla: t.t("clase-vivo-falla"),
            queda_poco: con("clase-vivo-queda-poco", &["minutos", "temas"]),
            cumplido: con("clase-vivo-cumplido", &["temas"]),
            cierran: con("clase-vivo-cierran", &["temas"]),
            fecha_nueva: con("clase-vivo-fecha-nueva", &["fecha"]),
            prioridades: [
                t.t("tareas-prioridad-urgente"),
                t.t("tareas-prioridad-alta"),
                t.t("clase-vivo-prioridad-normal"),
                t.t("tareas-prioridad-baja"),
            ],
            duracion: t.t("clase-vivo-duracion"),
            duracion_n: con("clase-vivo-duracion-n", &["minutos"]),
        }
    }

    pub(super) fn falla(&self) -> String {
        self.falla.clone()
    }

    fn prioridad(&self, p: Prioridad) -> &str {
        let i = match p {
            Prioridad::Urgente => 0,
            Prioridad::Alta => 1,
            Prioridad::Normal => 2,
            Prioridad::Baja => 3,
        };
        &self.prioridades[i]
    }
}

/// Que es una ficha.
#[derive(Debug, Clone)]
enum Que {
    /// Una propuesta (por su `id` en el acta).
    Propuesta(String),
    /// Un cambio de urgencia ya hecho (`cambio`: su sitio en el acta), con
    /// lo que hace falta para deshacerlo.
    Cambio {
        cambio: usize,
        tarea: String,
        deshacer: Box<Deshacer>,
        antes: Box<TareaViva>,
    },
    /// Un aviso de tema olvidado.
    Aviso,
}

#[derive(Debug, Clone)]
struct Ficha {
    id: u32,
    que: Que,
    texto: String,
}

/// **Lo demas de la reunion** mientras se graba.
#[derive(Debug, Default)]
pub(super) struct EnVivo {
    pub acta: Acta,
    /// Las preguntas aun sin respuesta, por su id de `pendientes`.
    abiertas: Vec<(String, Dicho)>,
    fichas: Vec<Ficha>,
    n_fichas: u32,
    /// Donde vive cada tarea de la urgencia en vivo, por su id.
    sitios: Vec<(String, TareaEnLista)>,
    /// Los ids de los temas, en el orden de la agenda que se le dio a Jev.
    agenda: Vec<String>,
    /// El nombre de la reunion (la nota de lo que se apunta).
    pub nombre: String,
    /// La ultima vez que llego algo (el brillo de la ficha nueva).
    pub nuevo: Option<Instant>,
}

/// «30/10» o «30/10 15:00».
fn corta(m: &Momento) -> String {
    let f = format!("{}/{}", m.fecha.dia, m.fecha.mes);
    match m.hora {
        Some(h) => format!("{f} {h}"),
        None => f,
    }
}

impl EnVivo {
    fn ficha(&mut self, que: Que, texto: String) {
        self.n_fichas += 1;
        self.fichas.push(Ficha {
            id: self.n_fichas,
            que,
            texto,
        });
        if self.fichas.len() > MAXIMO_FICHAS {
            self.fichas.remove(0);
        }
        self.nuevo = Some(Instant::now());
    }

    fn quitar(&mut self, id: u32) -> Option<Ficha> {
        let i = self.fichas.iter().position(|f| f.id == id)?;
        Some(self.fichas.remove(i))
    }

    /// Las que se ven, de la mas nueva a la mas vieja.
    fn a_la_vista(&self) -> impl Iterator<Item = &Ficha> {
        self.fichas.iter().rev().take(FICHAS_A_LA_VISTA)
    }

    /// Los ids de los temas en el orden de la agenda que se le dio a Jev.
    pub(super) fn agenda_ids(&self) -> &[String] {
        &self.agenda
    }

    fn propuesta(&mut self, id: &str) -> Option<&mut Propuesta> {
        self.acta.propuestas.iter_mut().find(|p| p.id == id)
    }

    /// **El acta para la nota**: lo apuntado y las preguntas que siguen
    /// abiertas.
    pub(super) fn acta(&self) -> Acta {
        let mut a = self.acta.clone();
        a.preguntas_abiertas = self.abiertas.iter().map(|(_, d)| d.clone()).collect();
        a
    }
}

impl Estado {
    /// **Lo de la reunion al empezar**: los temas que van a Jev (en su
    /// orden) y donde vive cada tarea de la urgencia en vivo.
    pub(super) fn preparar_reunion(&mut self, nombre: String, sitios: Vec<(String, TareaEnLista)>) {
        self.reunion = EnVivo {
            agenda: self.temas.iter().map(|t| t.id.clone()).collect(),
            nombre,
            sitios,
            ..EnVivo::default()
        };
    }

    /// Los nombres de unos temas de la agenda (por indice).
    fn nombres_de(&self, v: &[usize]) -> String {
        v.iter()
            .filter_map(|i| self.reunion.agenda.get(*i))
            .filter_map(|id| self.temas.iter().find(|t| t.id == *id))
            .map(|t| t.texto.clone())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// **Lo que llego de una frase.**
    pub(super) fn de_la_reunion(&mut self, sucesos: Vec<Suceso>, pedido: &Pedido, r: &Rotulos) {
        let ahora = match &self.fase {
            Fase::Grabando(g) => g.llevado_ms(),
            _ => 0,
        };
        for s in sucesos {
            match s {
                Suceso::Tarea(t) => {
                    let id = format!("p{}", self.reunion.acta.propuestas.len() + 1);
                    let responsable = match &t.responsable {
                        Responsable::Nombre(n) => Some(n.clone()),
                        _ => None,
                    };
                    let mut texto = format!("＋ {}: {}", r.tarea, t.texto);
                    if let Some(n) = &responsable {
                        texto.push_str(&format!(" · {n}"));
                    }
                    if let Some(m) = &t.fecha {
                        texto.push_str(&format!(" · {}", corta(m)));
                    }
                    self.reunion.acta.propuestas.push(Propuesta {
                        id: id.clone(),
                        tipo: Tipo::Tarea,
                        ms: t.ms,
                        texto: t.texto.clone(),
                        responsable,
                        fecha: t.fecha.map(|m| m.fecha.to_string()),
                        hora: t.fecha.and_then(|m| m.hora).map(|h| h.to_string()),
                        prioridad: Some(t.prioridad.id().to_string()),
                        estado: EstadoPropuesta::Pendiente,
                    });
                    self.reunion.ficha(Que::Propuesta(id), texto);
                }
                Suceso::Evento(e) => {
                    let id = format!("p{}", self.reunion.acta.propuestas.len() + 1);
                    let texto = format!("📅 {}: {} · {}", r.evento, e.titulo, corta(&e.momento));
                    self.reunion.acta.propuestas.push(Propuesta {
                        id: id.clone(),
                        tipo: Tipo::Evento,
                        ms: e.ms,
                        texto: e.titulo.clone(),
                        responsable: None,
                        fecha: Some(e.momento.fecha.to_string()),
                        hora: e.momento.hora.map(|h| h.to_string()),
                        prioridad: None,
                        estado: EstadoPropuesta::Pendiente,
                    });
                    self.reunion.ficha(Que::Propuesta(id), texto);
                }
                Suceso::Cambio(c, tarea) => self.cambio_de_urgencia(c, tarea, ahora, pedido, r),
                Suceso::Decision { ms, texto } => {
                    self.reunion.acta.decisiones.push(Dicho { ms, texto });
                    self.avisar(r.decision.replace(HUECO, &super::minuto(ms)));
                }
                Suceso::Abierta(a) => self.reunion.abiertas.push((
                    a.id,
                    Dicho {
                        ms: a.ms,
                        texto: a.texto,
                    },
                )),
                Suceso::Contestada(c) => {
                    self.reunion.abiertas.retain(|(id, _)| *id != c.pregunta.id);
                }
                Suceso::Tiempos { por_tema, otro, .. } => {
                    let temas = self
                        .reunion
                        .agenda
                        .iter()
                        .zip(por_tema)
                        .map(|(id, ms)| TiempoDeTema { id: id.clone(), ms })
                        .collect();
                    self.reunion.acta.tiempo_por_tema = Some(TiempoPorTema { temas, otro });
                }
                Suceso::Aviso(a) => {
                    let texto = match &a {
                        AvisoDeTemas::QuedaPoco { quedan_ms, temas } => r
                            .queda_poco
                            .replace(HUECO, &((quedan_ms + 59_999) / 60_000).to_string())
                            .replace(super::temas::HUECO2, &self.nombres_de(temas)),
                        AvisoDeTemas::TiempoCumplido { temas } => {
                            r.cumplido.replace(HUECO, &self.nombres_de(temas))
                        }
                        AvisoDeTemas::Cierran { temas } => {
                            r.cierran.replace(HUECO, &self.nombres_de(temas))
                        }
                    };
                    self.avisar(texto.clone());
                    self.reunion.ficha(Que::Aviso, format!("⏳ {texto}"));
                }
            }
        }
    }

    /// **Un cambio de urgencia**: se aplica ya (la fecha en la tarea, lo
    /// que la apura en Tareas), se dice y queda su «Deshacer».
    fn cambio_de_urgencia(
        &mut self,
        c: pixpin_typesafe::urgencia_viva::Cambio,
        tarea: String,
        ahora: i64,
        pedido: &Pedido,
        r: &Rotulos,
    ) {
        let Some(sitio) = self
            .reunion
            .sitios
            .iter()
            .find(|(id, _)| *id == c.id)
            .map(|(_, s)| s.clone())
        else {
            return;
        };
        let viva = self.escucha.as_ref().and_then(|e| e.tarea_de(&c.id));
        let fecha = (c.fecha_despues != c.fecha_antes)
            .then_some(c.fecha_despues)
            .flatten()
            .map(|f| (f, None));
        let senales = viva
            .as_ref()
            .filter(|_| c.antes != c.despues)
            .and_then(|t| t.senales.clone());
        match en_vivo::aplicar_cambio(pedido.ubicacion.raiz(), &sitio, fecha, senales) {
            Ok((ahora_sitio, deshacer)) => {
                if let Some((_, s)) = self.reunion.sitios.iter_mut().find(|(id, _)| *id == c.id) {
                    *s = ahora_sitio;
                }
                let mut texto = tarea.clone();
                if c.antes != c.despues {
                    let flecha = if c.despues < c.antes { "↑" } else { "↓" };
                    texto = format!(
                        "{flecha} {tarea}: {} → {}",
                        r.prioridad(c.antes),
                        r.prioridad(c.despues)
                    );
                }
                if let Some(f) = c.fecha_despues.filter(|_| c.fecha_despues != c.fecha_antes) {
                    let cuando = format!("{}/{}", f.dia, f.mes);
                    texto = format!("{texto} · {}", r.fecha_nueva.replace(HUECO, &cuando));
                }
                self.avisar(texto.clone());
                let cambio = self.reunion.acta.cambios.len();
                self.reunion.acta.cambios.push(CambioDeUrgencia {
                    ms: ahora,
                    tarea: tarea.clone(),
                    antes: c.antes.id().into(),
                    despues: c.despues.id().into(),
                    fecha_antes: c.fecha_antes.map(|f| f.to_string()),
                    fecha_despues: c.fecha_despues.map(|f| f.to_string()),
                    deshecho: false,
                });
                // Como estaba, para el «Deshacer» de la mesa.
                let antes = viva.map(|mut t| {
                    t.fecha = c.fecha_antes;
                    t.prioridad = c.antes;
                    t
                });
                if let Some(antes) = antes {
                    self.reunion.ficha(
                        Que::Cambio {
                            cambio,
                            tarea: c.id.clone(),
                            deshacer: Box::new(deshacer),
                            antes: Box::new(antes),
                        },
                        texto,
                    );
                }
            }
            // La tarea cambio fuera mientras tanto: no se toca.
            Err(e) => tracing::warn!(?e, "reunion: la urgencia de una tarea no se cambio"),
        }
    }

    /// **✓ de una ficha**: aceptar la propuesta o deshacer el cambio.
    pub(super) fn ficha_si(&mut self, id: u32, pedido: &Pedido, r: &Rotulos) {
        let Some(f) = self.reunion.fichas.iter().find(|f| f.id == id).cloned() else {
            return;
        };
        let raiz = pedido.ubicacion.raiz();
        match f.que {
            Que::Propuesta(pid) => {
                let Some(p) = self.reunion.propuesta(&pid).cloned() else {
                    return;
                };
                let nombre = self.reunion.nombre.clone();
                let hecho = match p.tipo {
                    Tipo::Tarea => {
                        en_vivo::apuntar_tarea(raiz, &p, &nombre).map(|(sitio, viva)| {
                            // Desde ahora tambien puede cambiar de urgencia.
                            if let Some(e) = &self.escucha {
                                e.tarea(viva.clone());
                            }
                            self.reunion.sitios.push((viva.id, sitio));
                            r.tarea_apuntada.clone()
                        })
                    }
                    Tipo::Evento => en_vivo::apuntar_evento(raiz, &p, &nombre)
                        .map(|_| r.evento_apuntado.clone()),
                };
                match hecho {
                    Ok(aviso) => {
                        if let Some(p) = self.reunion.propuesta(&pid) {
                            p.estado = EstadoPropuesta::Aceptada;
                        }
                        self.reunion.quitar(id);
                        self.avisar(aviso);
                    }
                    Err(e) => {
                        tracing::warn!(?e, "reunion: la propuesta no se apunto");
                        self.avisar(r.no_se_apunto.clone());
                    }
                }
            }
            Que::Cambio {
                cambio,
                tarea,
                deshacer,
                antes,
            } => match en_vivo::deshacer(raiz, &deshacer) {
                Ok(()) => {
                    if let Some(c) = self.reunion.acta.cambios.get_mut(cambio) {
                        c.deshecho = true;
                    }
                    if let Some((_, s)) =
                        self.reunion.sitios.iter_mut().find(|(id, _)| *id == tarea)
                    {
                        *s = deshacer.antes.clone();
                    }
                    if let Some(e) = &self.escucha {
                        e.tarea(*antes);
                    }
                    self.reunion.quitar(id);
                    self.avisar(r.deshecho.clone());
                }
                Err(e) => {
                    tracing::warn!(?e, "reunion: no se pudo deshacer la urgencia");
                    self.avisar(r.no_se_apunto.clone());
                }
            },
            Que::Aviso => {
                self.reunion.quitar(id);
            }
        }
    }

    /// **✕ de una ficha**: la propuesta se descarta; un cambio o un aviso
    /// solo se quitan de la vista.
    pub(super) fn ficha_no(&mut self, id: u32) {
        if let Some(Ficha {
            que: Que::Propuesta(pid),
            ..
        }) = self.reunion.quitar(id)
            && let Some(p) = self.reunion.propuesta(&pid)
        {
            p.estado = EstadoPropuesta::Descartada;
        }
    }

    /// «Duración prevista» de Preparar: la siguiente (y despues de la
    /// ultima, ninguna).
    pub(super) fn otra_duracion(&mut self) {
        self.duracion_prevista = match self.duracion_prevista {
            None => Some(DURACIONES[0]),
            Some(d) => DURACIONES.iter().copied().find(|x| *x > d),
        };
    }
}

/// **Lo alto de las fichas** debajo de la barra, en logicos.
pub(super) fn alto(estado: &Estado) -> u32 {
    let n = estado.reunion.fichas.len().min(FICHAS_A_LA_VISTA) as u32;
    if n == 0 { 0 } else { 6 + n * ALTO_FICHA + 4 }
}

/// **Las fichas**, debajo de la barra (y de la lista de los temas, si esta
/// abierta).
pub(super) fn pintar(
    p: &Pintor,
    w: f32,
    e: f32,
    estado: &Estado,
    r: &Rotulos,
    botones: &mut Botones<Accion>,
) {
    if alto(estado) == 0 {
        return;
    }
    let tema = &OSCURO;
    let x0 = 16.0 * e;
    let ancho = w - 32.0 * e;
    let mut y = (92 + super::temas::alto_del_panel(estado) + 6) as f32 * e;
    let brilla = estado
        .reunion
        .nuevo
        .is_some_and(|d| d.elapsed().as_millis() < 1200);
    for (k, f) in estado.reunion.a_la_vista().enumerate() {
        let fila = RectF {
            x: x0,
            y,
            ancho,
            alto: (ALTO_FICHA - 4) as f32 * e,
        };
        let fondo = if k == 0 && brilla {
            Color { a: 0.35, ..VERDE }
        } else if dentro(fila, botones.raton) {
            mas_claro(tema.campo)
        } else {
            tema.campo
        };
        p.rellenar_redondeado(fila, fila.alto / 2.0, fondo);
        let (icono, color) = match &f.que {
            Que::Propuesta(_) => (&mi::ADD, VERDE),
            Que::Cambio { .. } => (&mi::FLAG, AMBAR),
            Que::Aviso => (&mi::TIMER, AMBAR),
        };
        let lado = 16.0 * e;
        p.icono(
            icono,
            RectF {
                x: fila.x + 8.0 * e,
                y: fila.y + (fila.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            color,
        );
        // A la derecha: ✕ y, antes, ✓ (o «Deshacer»).
        let boton = 22.0 * e;
        let mut xd = fila.x + fila.ancho - 4.0 * e;
        let mut redondo = |p: &Pintor,
                           icono: &pixpin_render::icono::Icono,
                           tinta: Color,
                           que: Accion,
                           botones: &mut Botones<Accion>| {
            let caja = RectF {
                x: xd - boton,
                y: fila.y + (fila.alto - boton) / 2.0,
                ancho: boton,
                alto: boton,
            };
            if dentro(caja, botones.raton) {
                p.rellenar_redondeado(caja, boton / 2.0, mas_claro(tema.campo));
            }
            let l = 16.0 * e;
            p.icono(
                icono,
                RectF {
                    x: caja.x + (boton - l) / 2.0,
                    y: caja.y + (boton - l) / 2.0,
                    ancho: l,
                    alto: l,
                },
                tinta,
            );
            botones.zona(caja, que);
            xd = caja.x - 2.0 * e;
        };
        redondo(
            p,
            &mi::CLOSE,
            tema.campo_apagado,
            Accion::FichaNo(f.id),
            botones,
        );
        match &f.que {
            Que::Propuesta(_) => {
                redondo(p, &mi::CHECK_CIRCLE, VERDE, Accion::FichaSi(f.id), botones)
            }
            Que::Cambio { .. } => {
                let tam = 12.0 * e;
                let (tw, th) = p.medir_texto(&r.deshacer, tam);
                let caja = RectF {
                    x: xd - tw - 16.0 * e,
                    y: fila.y + 3.0 * e,
                    ancho: tw + 16.0 * e,
                    alto: fila.alto - 6.0 * e,
                };
                p.rellenar_redondeado(
                    caja,
                    caja.alto / 2.0,
                    if dentro(caja, botones.raton) {
                        mas_claro(tema.campo_apagado)
                    } else {
                        tema.campo_apagado
                    },
                );
                p.texto(
                    &r.deshacer,
                    caja.x + 8.0 * e,
                    caja.y + (caja.alto - th) / 2.0,
                    tam,
                    Color::BLANCO,
                );
                botones.zona(caja, Accion::FichaSi(f.id));
                xd = caja.x - 2.0 * e;
            }
            Que::Aviso => {}
        }
        let tam = 12.5 * e;
        let (_, th) = p.medir_texto(&f.texto, tam);
        let x = fila.x + 30.0 * e;
        p.texto_linea(
            &f.texto,
            x,
            fila.y + (fila.alto - th) / 2.0,
            tam,
            (xd - x - 4.0 * e).max(0.0),
            tema.texto,
        );
        y += ALTO_FICHA as f32 * e;
    }
}

/// **La «Duración prevista»** a la derecha de la fila de los temas de
/// Preparar: un clic pasa a la siguiente. Con ella, el tema olvidado avisa
/// cerca del final.
pub(super) fn pintar_duracion(
    p: &Pintor,
    fila: RectF,
    e: f32,
    estado: &Estado,
    r: &Rotulos,
    botones: &mut Botones<Accion>,
) {
    let tema = &OSCURO;
    let rotulo = match estado.duracion_prevista {
        Some(m) => r.duracion_n.replace(HUECO, &m.to_string()),
        None => r.duracion.clone(),
    };
    let tam = 12.0 * e;
    let (tw, th) = p.medir_texto(&rotulo, tam);
    let alto = 24.0 * e;
    let caja = RectF {
        x: fila.x + fila.ancho - 40.0 * e - tw - 16.0 * e,
        y: fila.y + (fila.alto - alto) / 2.0,
        ancho: tw + 16.0 * e,
        alto,
    };
    let fondo = if dentro(caja, botones.raton) {
        mas_claro(tema.campo_apagado)
    } else if estado.duracion_prevista.is_some() {
        tema.campo_apagado
    } else {
        mas_claro(tema.campo)
    };
    p.rellenar_redondeado(caja, alto / 2.0, fondo);
    p.texto(
        &rotulo,
        caja.x + 8.0 * e,
        caja.y + (alto - th) / 2.0,
        tam,
        if estado.duracion_prevista.is_some() {
            Color::BLANCO
        } else {
            tema.campo_apagado
        },
    );
    botones.zona(caja, Accion::Duracion);
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_typesafe::fechas::{Fecha, Hora};

    #[test]
    fn la_fecha_corta_de_una_ficha() {
        let m = Momento {
            fecha: Fecha::nueva(2026, 10, 30).unwrap(),
            hora: None,
            aproximado: false,
        };
        assert_eq!(corta(&m), "30/10");
        let m = Momento {
            hora: Some(Hora { h: 15, m: 0 }),
            ..m
        };
        assert_eq!(corta(&m), "30/10 15:00");
    }

    #[test]
    fn las_fichas_van_de_la_nueva_a_la_vieja_y_las_viejas_se_van() {
        let mut v = EnVivo::default();
        for i in 0..(MAXIMO_FICHAS + 2) {
            v.ficha(Que::Aviso, format!("{i}"));
        }
        assert_eq!(v.fichas.len(), MAXIMO_FICHAS);
        let vistas: Vec<&str> = v.a_la_vista().map(|f| f.texto.as_str()).collect();
        assert_eq!(vistas, ["13", "12", "11"]);
        let id = v.fichas.last().unwrap().id;
        assert!(v.quitar(id).is_some());
        assert!(v.quitar(id).is_none());
    }

    #[test]
    fn el_acta_lleva_las_preguntas_que_siguen_abiertas() {
        let mut v = EnVivo::default();
        v.abiertas.push((
            "q2".into(),
            Dicho {
                ms: 5,
                texto: "¿quién paga?".into(),
            },
        ));
        let a = v.acta();
        assert_eq!(a.preguntas_abiertas.len(), 1);
        assert!(v.acta.preguntas_abiertas.is_empty());
    }
}
