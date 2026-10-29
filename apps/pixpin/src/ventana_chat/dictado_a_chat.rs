//! **Dictar al chat** (29-oct-2026, la idea de Jev probada sin interfaz en
//! `docs/probar-ideas-sin-interfaz.md`, prototipo `dictado_a_chat`):
//! dictando en la caja del chat (clic derecho en el microfono), «anota en el
//! chat de la obra que faltan tornillos» o «anota esto en el chat de la
//! obra» **propone** anotarlo alli.
//!
//! La logica es la de la biblioteca, sin tocar: por cada frase dictada,
//! `pixpin_typesafe::vivo::preguntar` con solo la parte `chats` (los nombres
//! de los chats de la lista, los mas recientes) y la tabla
//! `pixpin_typesafe::dictado::decidir`. El chat lo elige Jev (Choice entre
//! los nombres); el texto **no se genera**: el codigo saca los candidatos
//! (lo que sigue a «:» o a «que», o la frase anterior para «esto»; la
//! anterior es la dictada antes o, si es la primera, lo que ya habia escrito
//! en la caja) y Jev elige cual.
//!
//! Lo decidido es **una propuesta, nunca hecho solo**: encima de la caja
//! sale la chapa «Anotar en «Obra Lima»: faltan tornillos · Intro». Intro
//! o un clic la anotan (un mensaje en ese chat, como escrito alli) y quitan
//! de la caja lo dictado para pedirlo; Escape o la ✕ la quitan. Sin chat
//! dicho («anótalo, que la grua se va el viernes») la chapa pregunta en que
//! chat, y aceptar abre la lista de chats para elegirlo.
//!
//! Sin clave, con `[jev] dictado` apagado o si Jev falla, el dictado es el
//! de siempre: lo dicho va a la caja y nada mas (un fallo es un error de la
//! frase, nunca un «no»; se apunta y ya). Una pregunta en vuelo como mucho;
//! lo que se dicta mientras tanto espera (las tres ultimas).

use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

use pixpin_geom::{Punto, Rect};
use pixpin_proyecto::almacen::Ficha;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_store::{Catalogo, Ubicacion};
use pixpin_typesafe::dictado::{self, Resultado};
use pixpin_typesafe::fechas::Fecha;
use pixpin_typesafe::vivo::{self, Frase, Partes};
use pixpin_ui::chat::Disposicion;

use super::{Tema, Zona, con_alfa};

/// Cuantos chats van de opciones: los mas recientes de la lista.
const CHATS: usize = 30;
/// Lo que se espera a Jev por frase (la app usa 2-4 s).
const TIEMPO: Duration = Duration::from_secs(4);
/// Las frases que esperan mientras una esta en vuelo.
const EN_COLA: usize = 3;
/// Quien dicta (solo va en el estado: «frase.quien»).
const QUIEN: &str = "Yo";

/// **Lo que se propone anotar.**
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Propuesta {
    /// El chat elegido: (id, nombre). `None`: no se dijo, se pregunta.
    pub chat: Option<(String, String)>,
    pub texto: String,
    /// Lo que se quita de la caja al anotarlo: la frase que lo pidio y, si
    /// el texto era la frase anterior, tambien ella (ya va al otro chat).
    pub quitar: Vec<String>,
}

/// Una frase por preguntar: ella, las de antes y los chats de entonces.
struct PorPreguntar {
    frase: Frase,
    antes: Vec<Frase>,
    chats: Vec<(String, String)>,
}

/// **El dictado al chat de una conversacion abierta** (vive en `Abierto`).
#[derive(Default)]
pub(super) struct Anotar {
    /// Las frases dictadas antes (las dos ultimas), para «esto».
    antes: Vec<Frase>,
    cola: VecDeque<PorPreguntar>,
    en_vuelo: Option<Receiver<Option<Propuesta>>>,
    pub propuesta: Option<Propuesta>,
    /// Donde se pinto la chapa (para colgar de ella la lista de chats).
    pub sitio: Cell<Option<Rect>>,
}

impl Anotar {
    /// Empieza un dictado nuevo: lo dictado antes ya no es «esto».
    pub fn empezar(&mut self) {
        self.antes.clear();
        self.cola.clear();
    }

    /// Si hay que volver a mirar (algo en vuelo o esperando).
    pub fn espera(&self) -> bool {
        self.en_vuelo.is_some() || !self.cola.is_empty()
    }

    /// **Una frase dictada**: `escrito_antes` es lo que habia en la caja
    /// antes de ella (la anterior, si es la primera del dictado).
    pub fn oir(
        &mut self,
        u: &Ubicacion,
        dicho: &str,
        escrito_antes: &str,
        fichas: &[Ficha],
        este_chat: &str,
    ) {
        let frase = Frase {
            ms: pixpin_shell::entorno::ahora_utc_ms(),
            quien: QUIEN.into(),
            dice: dicho.trim().to_string(),
        };
        let mut antes = self.antes.clone();
        if antes.is_empty() && !escrito_antes.trim().is_empty() {
            antes.push(Frase {
                ms: frase.ms,
                quien: QUIEN.into(),
                dice: escrito_antes.trim().to_string(),
            });
        }
        self.antes.push(frase.clone());
        if self.antes.len() > vivo::ANTES {
            self.antes.remove(0);
        }
        if !vivo::vale_la_pena(&frase)
            || crate::jev::para(u, crate::jev::Funcion::Dictado).is_none()
        {
            return;
        }
        let chats = chats_para(fichas, este_chat);
        if chats.is_empty() {
            return;
        }
        self.cola.push_back(PorPreguntar {
            frase,
            antes,
            chats,
        });
        while self.cola.len() > EN_COLA {
            self.cola.pop_front();
        }
        self.lanzar(u);
    }

    /// La siguiente de la cola, si no hay ninguna en vuelo.
    fn lanzar(&mut self, u: &Ubicacion) {
        if self.en_vuelo.is_some() {
            return;
        }
        let Some(p) = self.cola.pop_front() else {
            return;
        };
        let Some(cliente) = crate::jev::para(u, crate::jev::Funcion::Dictado) else {
            self.cola.clear();
            return;
        };
        let (enviar, recibir) = channel();
        let lanzado = std::thread::Builder::new()
            .name("jev-dictado".into())
            .spawn(move || {
                let cliente = cliente.con_tiempo(TIEMPO);
                let _ = enviar.send(preguntar(&cliente, &p));
            });
        if lanzado.is_ok() {
            self.en_vuelo = Some(recibir);
        }
    }

    /// **Recoge lo que dijo Jev** (cada vuelta del bucle, sin bloquear).
    /// Devuelve si hay que repintar.
    pub fn latido(&mut self, u: &Ubicacion) -> bool {
        let Some(r) = &self.en_vuelo else {
            if !self.cola.is_empty() {
                self.lanzar(u);
            }
            return false;
        };
        let llegado = match r.try_recv() {
            Ok(p) => p,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => None,
        };
        self.en_vuelo = None;
        self.lanzar(u);
        match llegado {
            Some(p) => {
                self.propuesta = Some(p);
                true
            }
            None => false,
        }
    }
}

/// Los chats que se ofrecen: (id, nombre), los mas recientes, sin este ni el
/// del asistente ni los que no tienen nombre.
pub(super) fn chats_para(fichas: &[Ficha], este_chat: &str) -> Vec<(String, String)> {
    let mut v: Vec<&Ficha> = fichas
        .iter()
        .filter(|f| f.id != este_chat && !f.es_asistente() && !f.nombre.trim().is_empty())
        .collect();
    v.sort_by_key(|f| std::cmp::Reverse(f.tocado));
    v.into_iter()
        .take(CHATS)
        .map(|f| (f.id.clone(), f.nombre.trim().to_string()))
        .collect()
}

/// Lo de [`Anotar::lanzar`], en su hilo. `None`: nada que proponer, o Jev
/// fallo (se apunta; el dictado sigue como siempre).
fn preguntar(cliente: &pixpin_typesafe::Cliente, p: &PorPreguntar) -> Option<Propuesta> {
    let nombres: Vec<String> = p.chats.iter().map(|(_, n)| n.clone()).collect();
    let hoy = Fecha::de_dias(pixpin_shell::entorno::ahora_local_ms().div_euclid(86_400_000));
    let ctx = vivo::Contexto {
        reunion: "",
        participantes: &[],
        agenda: &[],
        antes: &p.antes,
        frase: &p.frase,
        hoy,
        tema_anterior: None,
    };
    let partes = Partes {
        chats: &nombres,
        ..Default::default()
    };
    let (senales, _) = match vivo::preguntar(cliente, &ctx, &partes) {
        Ok(s) => s,
        Err(e) => {
            tracing::info!("jev: el dictado al chat no se pregunto: {e}");
            return None;
        }
    };
    let d = senales.dictado.as_ref();
    match dictado::decidir(&p.frase, d, &nombres) {
        Resultado::Nota(n) => {
            // `n.chat` es el nombre; el id sale del indice (dos chats pueden
            // llamarse igual).
            let chat = n
                .chat
                .as_ref()
                .and_then(|_| d.and_then(|d| p.chats.get(d.chat)).cloned());
            let mut quitar = vec![p.frase.dice.clone()];
            // Lo que no sale en la frase es la anterior («anota esto»).
            if let Some(a) = p.antes.last()
                && !p.frase.dice.contains(n.texto.trim_end_matches('…'))
            {
                quitar.push(a.dice.clone());
            }
            tracing::info!(p = n.p, con_chat = chat.is_some(), "jev: dictado al chat");
            Some(Propuesta {
                chat,
                texto: n.texto,
                quitar,
            })
        }
        Resultado::Dudoso(p) => {
            tracing::debug!(p, "jev: dictado al chat dudoso, no se propone");
            None
        }
        Resultado::SinTexto | Resultado::Nada => None,
    }
}

/// **Quita de lo escrito un trozo dictado** (la ultima vez que sale), con el
/// espacio que lo separaba. Al dictarlo, su primera letra pudo cambiar de
/// mayuscula a minuscula (`juntar_dictado`): esa no cuenta.
pub(super) fn quitar_dicho(escrito: &str, trozo: &str) -> String {
    let trozo = trozo.trim();
    let mut letras = trozo.chars();
    let Some(primera) = letras.next() else {
        return escrito.to_string();
    };
    let resto = letras.as_str();
    let sitios: Vec<usize> = escrito.match_indices(resto).map(|(i, _)| i).collect();
    for i in sitios.into_iter().rev() {
        let Some(c) = escrito[..i].chars().last() else {
            continue;
        };
        if !c.to_lowercase().eq(primera.to_lowercase()) {
            continue;
        }
        let fuera = escrito[..i - c.len_utf8()].trim_end_matches(' ');
        let dentro = &escrito[i + resto.len()..];
        let dentro = dentro.strip_prefix(' ').unwrap_or(dentro);
        return if fuera.is_empty() || dentro.is_empty() || fuera.ends_with('\n') {
            format!("{fuera}{dentro}")
        } else {
            format!("{fuera} {dentro}")
        };
    }
    escrito.to_string()
}

/// El rotulo de la chapa.
pub(super) fn rotulo(p: &Propuesta, textos: &Catalogo) -> String {
    let corto: String = if p.texto.chars().count() > 90 {
        format!("{}…", p.texto.chars().take(90).collect::<String>())
    } else {
        p.texto.clone()
    };
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("texto", corto);
    match &p.chat {
        Some((_, nombre)) => {
            args.set("chat", nombre.clone());
            textos.t_args("chat-anotar-en", &args)
        }
        None => textos.t_args("chat-anotar-donde", &args),
    }
}

/// **Anota la propuesta** en su chat: un mensaje como escrito alli, con su
/// numero, y el chat sube en la lista. Devuelve el nombre del chat.
pub(super) fn anotar(
    raiz: &std::path::Path,
    aparato: &str,
    id: &str,
    texto: &str,
) -> std::io::Result<()> {
    let texto = super::juntar_dictado("", texto);
    let cuando = super::descripcion::hora_sin_repetir();
    let numero = super::siguiente_numero_en(raiz, id)?;
    super::escribir_nota(raiz, id, aparato, &texto, cuando, numero, None)?;
    super::subir_en_la_lista(raiz, id, cuando, &texto)
}

/// **Pinta la chapa** encima de la caja de escribir y apunta sus zonas: la
/// chapa entera anota (o pregunta el chat) y la ✕ la quita.
pub(super) fn pintar(
    p: &Pintor,
    d: &Disposicion,
    tema: &Tema,
    escala: u32,
    alto_caja: u32,
    a: &super::Abierto,
    textos: &Catalogo,
) {
    let anotar = &a.anotar;
    anotar.sitio.set(None);
    let Some(prop) = &anotar.propuesta else {
        return;
    };
    let e = escala as f32 / 100.0;
    let columna = if d.chat.ancho > 0 { d.chat } else { d.lista };
    let tam = 13.0 * e;
    let texto = rotulo(prop, textos);
    let tecla = textos.t("chat-anotar-tecla");
    let (wt, _) = p.medir_texto(&tecla, 12.0 * e);
    let cruz = 26.0 * e;
    let tope = (columna.ancho as f32 - 96.0 * e - wt - cruz).max(1.0);
    let (w, alto) = p.medir_texto_ajustado(&texto, tam, tope);
    let ancho = w + wt + cruz + 48.0 * e;
    let caja = RectF {
        x: columna.x as f32 + (columna.ancho as f32 - ancho) / 2.0,
        y: d.redaccion(alto_caja, escala).y as f32 - alto - 18.0 * e - 10.0 * e,
        ancho,
        alto: alto + 18.0 * e,
    };
    let radio = caja.alto.min(40.0 * e) / 2.0;
    p.rellenar_redondeado(caja, radio, con_alfa(tema.enviar, 0.95));
    p.parrafo(
        &texto,
        caja.x + 16.0 * e,
        caja.y + 9.0 * e,
        tam,
        w + 1.0,
        &[],
        Color::BLANCO,
    );
    // «Intro», en su pildora.
    let pildora = RectF {
        x: caja.x + 16.0 * e + w + 10.0 * e,
        y: caja.y + (caja.alto - 20.0 * e) / 2.0,
        ancho: wt + 12.0 * e,
        alto: 20.0 * e,
    };
    p.rellenar_redondeado(pildora, 10.0 * e, con_alfa(Color::BLANCO, 0.22));
    p.texto(
        &tecla,
        pildora.x + 6.0 * e,
        pildora.y + 3.0 * e,
        12.0 * e,
        Color::BLANCO,
    );
    // La ✕.
    let x = RectF {
        x: caja.x + caja.ancho - cruz - 8.0 * e,
        y: caja.y + (caja.alto - cruz) / 2.0,
        ancho: cruz,
        alto: cruz,
    };
    p.texto("✕", x.x + 7.0 * e, x.y + 4.0 * e, 13.0 * e, Color::BLANCO);
    let entera = rect_de(caja);
    anotar.sitio.set(Some(entera));
    let mut zonas = a.zonas.borrow_mut();
    zonas.push((entera, Zona::Anotar));
    zonas.push((rect_de(x), Zona::NoAnotar));
}

fn rect_de(r: RectF) -> Rect {
    Rect {
        x: r.x.floor() as i32,
        y: r.y.floor() as i32,
        ancho: r.ancho.ceil().max(0.0) as u32,
        alto: r.alto.ceil().max(0.0) as u32,
    }
}

/// Donde cuelga la lista de chats (bajo la chapa, a su derecha).
pub(super) fn ancla(anotar: &Anotar) -> Option<Punto> {
    anotar.sitio.get().map(|r| Punto {
        x: r.derecha(),
        y: r.y,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ficha(id: &str, nombre: &str, tocado: i64) -> Ficha {
        Ficha {
            id: id.into(),
            nombre: nombre.into(),
            tocado,
            ..Default::default()
        }
    }

    #[test]
    fn se_quita_de_la_caja_lo_dictado_para_pedirlo() {
        assert_eq!(
            quitar_dicho(
                "Hola. Anota en el chat de la obra que faltan tornillos",
                "anota en el chat de la obra que faltan tornillos"
            ),
            "Hola."
        );
        assert_eq!(
            quitar_dicho(
                "El cemento llega el jueves. Anota esto en el chat de la obra",
                "anota esto en el chat de la obra"
            ),
            "El cemento llega el jueves."
        );
        assert_eq!(
            quitar_dicho("El cemento llega el jueves.", "el cemento llega el jueves."),
            ""
        );
        // Lo que no esta (lo borro a mano) deja la caja como estaba.
        assert_eq!(quitar_dicho("otra cosa", "anótalo"), "otra cosa");
        // La ultima vez que sale, y en medio deja un espacio.
        assert_eq!(quitar_dicho("a x b x c", "x"), "a x b c");
        assert_eq!(quitar_dicho("ñandú ñandú", "Ñandú"), "ñandú");
    }

    #[test]
    fn los_chats_que_se_ofrecen_son_los_recientes_sin_este() {
        let f = vec![
            ficha("1", "Obra Calle Lima", 10),
            ficha("2", "Compras", 30),
            ficha("3", "  ", 40),
            ficha("4", "Este", 50),
        ];
        assert_eq!(
            chats_para(&f, "4"),
            vec![
                ("2".to_string(), "Compras".to_string()),
                ("1".to_string(), "Obra Calle Lima".to_string()),
            ]
        );
    }

    #[test]
    fn la_propuesta_sale_con_respuestas_grabadas() {
        use pixpin_typesafe::transporte::Grabado;
        use std::sync::Arc;
        let r = r#"{"model":"jev-1.14.0","answers":{
            "dictado":{"type":"noul","noul":0.95},
            "dictado_chat_dicho":{"type":"noul","noul":0.9},
            "dictado_chat":{"type":"choice","choice":"obra_calle_lima","confidence":0.6,
                "probabilities":{"obra_calle_lima":0.8,"compras":0.2}}},
            "usage":{"input_tokens":180,"output_tokens":9}}"#;
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(r)]));
        let c = pixpin_typesafe::Cliente::con_transporte("k", t).con_cache_propio();
        let p = PorPreguntar {
            frase: Frase {
                ms: 0,
                quien: QUIEN.into(),
                dice: "Anota en el chat de la obra que faltan tornillos".into(),
            },
            antes: vec![],
            chats: vec![
                ("1".into(), "Obra Calle Lima".into()),
                ("2".into(), "Compras".into()),
            ],
        };
        let prop = preguntar(&c, &p).expect("una propuesta");
        assert_eq!(
            prop.chat,
            Some(("1".to_string(), "Obra Calle Lima".to_string()))
        );
        assert_eq!(prop.texto, "faltan tornillos");
        assert_eq!(prop.quitar, vec![p.frase.dice.clone()]);
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        assert!(rotulo(&prop, &textos).contains("Obra Calle Lima"));
    }
}
