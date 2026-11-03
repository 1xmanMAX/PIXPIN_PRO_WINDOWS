//! **El comentario, su hilo y sus adjuntos**, y lo que se les hace.
//!
//! Un [`Comentario`] es la pegatina: su [`Tipo`] (tres de urgencia y la
//! nota), su [`Ancla`], si esta resuelto y su hilo de [`Entrada`]s (el
//! minichat). Cada entrada lleva texto, adjuntos o las dos cosas.
//!
//! **Nada se borra del fichero**: quitar una pegatina o una entrada la marca
//! `borrado` con su hora (una lapida). Sin eso, al juntar con el otro
//! aparato, lo quitado volveria de su lado (la sincronizacion no lleva
//! borrados de ficheros, y aqui tampoco de cosas).
//!
//! Las horas son milisegundos UTC. `cambiado` es la de lo ultimo que se
//! cambio de la pegatina (tipo, sitio, resuelto, borrado, tarea): al juntar,
//! gana el mas nuevo ([`crate::fusion`]).

use std::collections::HashSet;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::ancla::Ancla;

/// **Los cuatro tipos**: tres urgencias y la nota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub enum Tipo {
    /// Rojo: hay que hacerlo ya.
    Urgente,
    /// Naranja.
    Alta,
    /// Amarillo.
    Media,
    /// El bocadillo: un comentario sin urgencia.
    #[default]
    Nota,
}

impl Tipo {
    /// En el orden de la lista y de la tira de elegir: de mas a menos urgente.
    pub const TODOS: [Tipo; 4] = [Tipo::Urgente, Tipo::Alta, Tipo::Media, Tipo::Nota];

    /// El nombre del fichero.
    pub fn nombre(self) -> &'static str {
        match self {
            Tipo::Urgente => "urgente",
            Tipo::Alta => "alta",
            Tipo::Media => "media",
            Tipo::Nota => "nota",
        }
    }

    /// Lo que no se conoce (de una version mas nueva) se lee como nota.
    pub fn de_nombre(s: &str) -> Tipo {
        match s.trim().to_ascii_lowercase().as_str() {
            "urgente" => Tipo::Urgente,
            "alta" => Tipo::Alta,
            "media" => Tipo::Media,
            _ => Tipo::Nota,
        }
    }

    /// El color de la pegatina (`0xRRGGBB`): rojo, naranja, amarillo y el
    /// azul del bocadillo. Los mismos en todos los aparatos.
    pub fn color(self) -> u32 {
        match self {
            Tipo::Urgente => 0xE53935,
            Tipo::Alta => 0xFB8C00,
            Tipo::Media => 0xFDD835,
            Tipo::Nota => 0x5C6BC0,
        }
    }

    /// El color del signo encima (blanco, o casi negro sobre el amarillo).
    pub fn color_del_signo(self) -> u32 {
        match self {
            Tipo::Media => 0x3E2F00,
            _ => 0xFFFFFF,
        }
    }

    /// El emoticono que lo representa donde no se pinta la pegatina (el
    /// elemento espejo del lienzo, la tarea, un texto).
    pub fn emoji(self) -> &'static str {
        match self {
            Tipo::Urgente => "\u{1F534}",
            Tipo::Alta => "\u{1F7E0}",
            Tipo::Media => "\u{1F7E1}",
            Tipo::Nota => "\u{1F4AC}",
        }
    }

    /// La clave de su nombre en los catalogos (`comentario-tipo-urgente`...).
    pub fn clave(self) -> &'static str {
        match self {
            Tipo::Urgente => "comentario-tipo-urgente",
            Tipo::Alta => "comentario-tipo-alta",
            Tipo::Media => "comentario-tipo-media",
            Tipo::Nota => "comentario-tipo-nota",
        }
    }

    /// De mas urgente (0) a menos (3): el orden de la lista.
    pub fn orden(self) -> u8 {
        self as u8
    }

    /// El siguiente en la tira (el ultimo vuelve al primero): un clic en el
    /// redondel del minichat cambia el tipo asi.
    pub fn siguiente(self) -> Tipo {
        let i = Tipo::TODOS.iter().position(|t| *t == self).unwrap_or(0);
        Tipo::TODOS[(i + 1) % Tipo::TODOS.len()]
    }

    pub fn es_urgencia(self) -> bool {
        self != Tipo::Nota
    }
}

impl Serialize for Tipo {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nombre())
    }
}

impl<'de> Deserialize<'de> for Tipo {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        Ok(Tipo::de_nombre(v.as_str().unwrap_or_default()))
    }
}

/// **Que es un adjunto.**
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TipoAdjunto {
    /// Un fichero cualquiera (`ruta`, `nombre`).
    #[default]
    Archivo,
    /// Una imagen (`ruta`, `nombre`); `tinta` es lo anotado encima en el
    /// lienzo, si se anoto.
    Imagen,
    /// Un atajo a un mensaje del chat (`chat`: el id del proyecto;
    /// `mensaje`: su `Codigos.unico`).
    Mensaje,
    /// Un atajo a un elemento de un lienzo (`lienzo`, `elemento`).
    Elemento,
    /// Un lienzo propio en blanco (`tinta`).
    Lienzo,
    /// De una version mas nueva: se conserva tal cual y no se abre.
    Otro(String),
}

impl TipoAdjunto {
    pub fn nombre(&self) -> &str {
        match self {
            TipoAdjunto::Archivo => "archivo",
            TipoAdjunto::Imagen => "imagen",
            TipoAdjunto::Mensaje => "mensaje",
            TipoAdjunto::Elemento => "elemento",
            TipoAdjunto::Lienzo => "lienzo",
            TipoAdjunto::Otro(s) => s,
        }
    }

    pub fn de_nombre(s: &str) -> TipoAdjunto {
        match s {
            "archivo" => TipoAdjunto::Archivo,
            "imagen" => TipoAdjunto::Imagen,
            "mensaje" => TipoAdjunto::Mensaje,
            "elemento" => TipoAdjunto::Elemento,
            "lienzo" => TipoAdjunto::Lienzo,
            otro => TipoAdjunto::Otro(otro.to_string()),
        }
    }
}

impl Serialize for TipoAdjunto {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nombre())
    }
}

impl<'de> Deserialize<'de> for TipoAdjunto {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        Ok(TipoAdjunto::de_nombre(v.as_str().unwrap_or("archivo")))
    }
}

/// **Un adjunto de una entrada.** Las rutas son **portatiles** (las del
/// chat: `pins/draw/anot-<uid>-a<huella>.adjunto`), iguales en todos los
/// aparatos; fuera de un chat, la ruta del disco (no viaja).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Adjunto {
    pub tipo: TipoAdjunto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruta: Option<String>,
    /// El nombre con el que se ensena (y su extension, que la ruta no lleva).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nombre: Option<String>,
    /// El lienzo de lo anotado sobre la imagen, o el lienzo propio: ruta
    /// portatil de un `.excalidraw.gz`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tinta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat: Option<String>,
    /// El `Codigos.unico` del mensaje.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mensaje: Option<String>,
    /// La referencia del lienzo del elemento (`pins/draw/<lienzo>.excalidraw.gz`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lienzo: Option<String>,
    /// El `id` Excalidraw del elemento.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elemento: Option<String>,
    /// Donde estaba el elemento al hacer el atajo (su centro, coordenadas
    /// del mundo del lienzo): para ir a el aunque su id no se encuentre.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub punto: Option<[f64; 2]>,
    /// Lo que se ensena del atajo (el texto del mensaje, el del elemento).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotulo: Option<String>,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

impl Adjunto {
    pub fn archivo(ruta: &str, nombre: &str) -> Adjunto {
        let imagen = es_imagen(nombre);
        Adjunto {
            tipo: if imagen {
                TipoAdjunto::Imagen
            } else {
                TipoAdjunto::Archivo
            },
            ruta: Some(ruta.into()),
            nombre: Some(nombre.into()),
            ..Adjunto::default()
        }
    }

    pub fn mensaje(chat: &str, mensaje: &str, rotulo: &str) -> Adjunto {
        Adjunto {
            tipo: TipoAdjunto::Mensaje,
            chat: Some(chat.into()),
            mensaje: Some(mensaje.into()),
            rotulo: Some(rotulo.into()).filter(|r: &String| !r.is_empty()),
            ..Adjunto::default()
        }
    }

    pub fn elemento(
        lienzo: &str,
        elemento: &str,
        punto: Option<[f64; 2]>,
        rotulo: &str,
    ) -> Adjunto {
        Adjunto {
            tipo: TipoAdjunto::Elemento,
            lienzo: Some(lienzo.into()),
            elemento: Some(elemento.into()),
            punto,
            rotulo: Some(rotulo.into()).filter(|r: &String| !r.is_empty()),
            ..Adjunto::default()
        }
    }

    pub fn lienzo(tinta: &str) -> Adjunto {
        Adjunto {
            tipo: TipoAdjunto::Lienzo,
            tinta: Some(tinta.into()),
            ..Adjunto::default()
        }
    }

    /// Las rutas portatiles de ficheros que lleva (para que viajen con el
    /// documento: el fichero y su lienzo).
    pub fn rutas(&self) -> impl Iterator<Item = &str> {
        self.ruta
            .iter()
            .chain(self.tinta.iter())
            .map(String::as_str)
    }
}

/// Si un nombre es de una imagen que se puede abrir en el lienzo.
pub fn es_imagen(nombre: &str) -> bool {
    let n = nombre.to_ascii_lowercase();
    [
        ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".webp", ".tif", ".tiff",
    ]
    .iter()
    .any(|e| n.ends_with(e))
}

/// **Una entrada del minichat.**
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Entrada {
    pub id: String,
    /// El nombre del aparato (o de la persona) que la escribio.
    pub autor: String,
    /// Su codigo de aparato (el de la sincronizacion).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub aparato: String,
    pub cuando: i64,
    /// Lo ultimo que se le cambio (editar, quitar, anotar su imagen).
    pub cambiado: i64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub texto: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub adjuntos: Vec<Adjunto>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub borrado: bool,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

/// **Una pegatina de comentario.**
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Comentario {
    pub id: String,
    pub tipo: Tipo,
    pub ancla: Ancla,
    pub autor: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub aparato: String,
    pub creado: i64,
    pub cambiado: i64,
    pub resuelto: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub borrado: bool,
    /// El id de la tarea que se creo de el («Crear tarea»).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tarea: Option<String>,
    pub hilo: Vec<Entrada>,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

impl Comentario {
    /// Las entradas que se ven, en su orden.
    pub fn entradas(&self) -> impl Iterator<Item = &Entrada> {
        self.hilo.iter().filter(|e| !e.borrado)
    }

    /// Lo primero que se escribio: el titulo en la lista y en la tarea.
    pub fn resumen(&self) -> String {
        self.entradas()
            .find(|e| !e.texto.trim().is_empty())
            .map(|e| {
                let linea = e.texto.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
                let mut s: String = linea.trim().chars().take(80).collect();
                if linea.trim().chars().count() > 80 {
                    s.push('\u{2026}');
                }
                s
            })
            .unwrap_or_default()
    }

    /// La hora de lo ultimo que paso: el ultimo cambio o la ultima entrada.
    pub fn ultimo(&self) -> i64 {
        self.hilo
            .iter()
            .map(|e| e.cambiado.max(e.cuando))
            .fold(self.cambiado.max(self.creado), i64::max)
    }

    pub fn entrada(&self, id: &str) -> Option<&Entrada> {
        self.hilo.iter().find(|e| e.id == id)
    }
}

/// Quien escribe.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Quien {
    pub autor: String,
    pub aparato: String,
}

/// **Las pegatinas de un documento.**
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pegatinas {
    /// Todas, tambien las quitadas (lapidas), en el orden en que se pusieron.
    pub lista: Vec<Comentario>,
}

/// El texto limpio de una entrada: sin blancos en los bordes y con saltos
/// de linea de Unix.
fn limpio(t: &str) -> String {
    t.trim().replace("\r\n", "\n")
}

impl Pegatinas {
    /// Las que se ven (no quitadas).
    pub fn vivas(&self) -> impl Iterator<Item = &Comentario> {
        self.lista.iter().filter(|c| !c.borrado)
    }

    pub fn get(&self, id: &str) -> Option<&Comentario> {
        self.lista.iter().find(|c| c.id == id && !c.borrado)
    }

    fn get_mut(&mut self, id: &str) -> Option<&mut Comentario> {
        self.lista.iter_mut().find(|c| c.id == id && !c.borrado)
    }

    /// Las que no estan resueltas: el numero del boton.
    pub fn abiertas(&self) -> usize {
        self.vivas().filter(|c| !c.resuelto).count()
    }

    /// Un id que no esta en el fichero: el aparato, la hora y un contador.
    pub fn id_nuevo(&self, aparato: &str, cuando: i64) -> String {
        let usados: HashSet<&str> = self
            .lista
            .iter()
            .flat_map(|c| {
                std::iter::once(c.id.as_str()).chain(c.hilo.iter().map(|e| e.id.as_str()))
            })
            .collect();
        let aparato: String = aparato
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(16)
            .collect();
        let aparato = if aparato.is_empty() {
            "x".into()
        } else {
            aparato
        };
        (0..)
            .map(|n| format!("{aparato}-{cuando:x}-{n}"))
            .find(|id| !usados.contains(id.as_str()))
            .unwrap_or_default()
    }

    /// **Pega una pegatina** nueva, sin hilo aun. Devuelve su id.
    pub fn poner(&mut self, tipo: Tipo, ancla: Ancla, quien: &Quien, cuando: i64) -> String {
        let id = self.id_nuevo(&quien.aparato, cuando);
        self.lista.push(Comentario {
            id: id.clone(),
            tipo,
            ancla,
            autor: quien.autor.clone(),
            aparato: quien.aparato.clone(),
            creado: cuando,
            cambiado: cuando,
            ..Comentario::default()
        });
        id
    }

    /// Cambia algo de la pegatina y apunta la hora. `false` si no esta o si
    /// no cambio nada (no se apunta una hora sin cambio: viajaria).
    fn tocar(&mut self, id: &str, cuando: i64, f: impl FnOnce(&mut Comentario)) -> bool {
        let Some(c) = self.get_mut(id) else {
            return false;
        };
        let antes = c.clone();
        f(c);
        if *c == antes {
            return false;
        }
        c.cambiado = cuando.max(antes.cambiado + 1);
        true
    }

    /// La lleva a otro sitio (arrastrar).
    pub fn mover(&mut self, id: &str, ancla: Ancla, cuando: i64) -> bool {
        self.tocar(id, cuando, |c| c.ancla = ancla)
    }

    pub fn cambiar_tipo(&mut self, id: &str, tipo: Tipo, cuando: i64) -> bool {
        self.tocar(id, cuando, |c| c.tipo = tipo)
    }

    pub fn resolver(&mut self, id: &str, si: bool, cuando: i64) -> bool {
        self.tocar(id, cuando, |c| c.resuelto = si)
    }

    /// Apunta la tarea creada de esta pegatina.
    pub fn vincular_tarea(&mut self, id: &str, tarea: &str, cuando: i64) -> bool {
        self.tocar(id, cuando, |c| c.tarea = Some(tarea.to_string()))
    }

    /// **Quita la pegatina**: queda la lapida (id, tipo y hora), sin hilo.
    pub fn quitar(&mut self, id: &str, cuando: i64) -> bool {
        self.tocar(id, cuando, |c| {
            c.borrado = true;
            for e in &mut c.hilo {
                e.borrado = true;
                e.texto.clear();
                e.adjuntos.clear();
                e.cambiado = e.cambiado.max(cuando);
            }
        })
    }

    /// **Escribe en el minichat**. `None` si no hay nada que mandar (ni
    /// texto ni adjuntos) o la pegatina no esta.
    pub fn escribir(
        &mut self,
        id: &str,
        quien: &Quien,
        cuando: i64,
        texto: &str,
        adjuntos: Vec<Adjunto>,
    ) -> Option<String> {
        let texto = limpio(texto);
        if texto.is_empty() && adjuntos.is_empty() {
            return None;
        }
        let eid = self.id_nuevo(&quien.aparato, cuando);
        let c = self.get_mut(id)?;
        c.hilo.push(Entrada {
            id: eid.clone(),
            autor: quien.autor.clone(),
            aparato: quien.aparato.clone(),
            cuando,
            cambiado: cuando,
            texto,
            adjuntos,
            ..Entrada::default()
        });
        Some(eid)
    }

    fn tocar_entrada(
        &mut self,
        id: &str,
        entrada: &str,
        cuando: i64,
        f: impl FnOnce(&mut Entrada),
    ) -> bool {
        let Some(e) = self
            .get_mut(id)
            .and_then(|c| c.hilo.iter_mut().find(|e| e.id == entrada && !e.borrado))
        else {
            return false;
        };
        let antes = e.clone();
        f(e);
        if *e == antes {
            return false;
        }
        e.cambiado = cuando.max(antes.cambiado + 1);
        true
    }

    /// Cambia el texto de una entrada. Vacio y sin adjuntos es quitarla.
    pub fn editar(&mut self, id: &str, entrada: &str, texto: &str, cuando: i64) -> bool {
        let t = limpio(texto);
        self.tocar_entrada(id, entrada, cuando, |e| {
            if t.is_empty() && e.adjuntos.is_empty() {
                e.borrado = true;
            }
            e.texto = t;
        })
    }

    /// Quita una entrada (queda su lapida).
    pub fn quitar_entrada(&mut self, id: &str, entrada: &str, cuando: i64) -> bool {
        self.tocar_entrada(id, entrada, cuando, |e| {
            e.borrado = true;
            e.texto.clear();
            e.adjuntos.clear();
        })
    }

    /// Le pone a la imagen `k` de una entrada el lienzo de lo anotado encima
    /// (la primera vez que se abre para anotar).
    pub fn poner_tinta(
        &mut self,
        id: &str,
        entrada: &str,
        k: usize,
        tinta: &str,
        cuando: i64,
    ) -> bool {
        self.tocar_entrada(id, entrada, cuando, |e| {
            if let Some(a) = e.adjuntos.get_mut(k) {
                a.tinta = Some(tinta.to_string());
            }
        })
    }

    /// Las rutas portatiles de todos los ficheros que cuelgan de las
    /// pegatinas (adjuntos y lienzos), sin repetir: lo que tiene que viajar
    /// con el documento. Las lapidas no llevan.
    pub fn rutas(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for c in self.vivas() {
            for e in c.entradas() {
                for a in &e.adjuntos {
                    for r in a.rutas() {
                        if !v.iter().any(|x| x == r) {
                            v.push(r.to_string());
                        }
                    }
                }
            }
        }
        v
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn quien() -> Quien {
        Quien {
            autor: "PC de Max".into(),
            aparato: "pc1".into(),
        }
    }

    #[test]
    fn poner_escribir_y_resolver() {
        let mut p = Pegatinas::default();
        let id = p.poner(Tipo::Urgente, Ancla::escena(10.0, 20.0), &quien(), 1000);
        assert_eq!(id, "pc1-3e8-0");
        assert_eq!(p.abiertas(), 1);
        let e = p
            .escribir(&id, &quien(), 1100, "  Revisar la losa\r\nya  ", vec![])
            .unwrap();
        assert_eq!(p.get(&id).unwrap().hilo[0].texto, "Revisar la losa\nya");
        assert_eq!(p.get(&id).unwrap().resumen(), "Revisar la losa");
        // Caso negativo: nada que mandar no es entrada.
        assert!(p.escribir(&id, &quien(), 1200, "   ", vec![]).is_none());
        assert!(p.escribir("otro", &quien(), 1200, "x", vec![]).is_none());
        assert!(p.resolver(&id, true, 1300));
        assert_eq!(p.abiertas(), 0);
        // Sin cambio no se apunta hora.
        assert!(!p.resolver(&id, true, 1400));
        assert_eq!(p.get(&id).unwrap().cambiado, 1300);
        assert!(p.editar(&id, &e, "Revisar la losa hoy", 1500));
        assert_eq!(p.get(&id).unwrap().ultimo(), 1500);
    }

    #[test]
    fn quitar_deja_lapida_sin_contenido() {
        let mut p = Pegatinas::default();
        let id = p.poner(Tipo::Nota, Ancla::pagina(0, 1.0, 2.0), &quien(), 10);
        p.escribir(
            &id,
            &quien(),
            11,
            "secreto",
            vec![Adjunto::archivo("pins/draw/a.adjunto", "a.png")],
        );
        assert_eq!(p.rutas(), vec!["pins/draw/a.adjunto"]);
        assert!(p.quitar(&id, 20));
        assert!(p.get(&id).is_none());
        assert_eq!(p.vivas().count(), 0);
        let lapida = &p.lista[0];
        assert!(lapida.borrado && lapida.hilo[0].borrado);
        assert!(lapida.hilo[0].texto.is_empty() && lapida.hilo[0].adjuntos.is_empty());
        assert!(p.rutas().is_empty());
        // Lo quitado ya no se toca.
        assert!(!p.mover(&id, Ancla::pagina(0, 5.0, 5.0), 30));
    }

    #[test]
    fn la_hora_de_un_cambio_siempre_avanza() {
        // Un reloj que va para atras (otro aparato, o el cambio de hora) no
        // puede dejar un cambio con hora de antes: perderia al juntar.
        let mut p = Pegatinas::default();
        let id = p.poner(Tipo::Media, Ancla::escena(0.0, 0.0), &quien(), 5000);
        assert!(p.cambiar_tipo(&id, Tipo::Alta, 100));
        assert_eq!(p.get(&id).unwrap().cambiado, 5001);
    }

    #[test]
    fn el_tipo_lee_lo_que_no_conoce_como_nota_y_tiene_sus_colores() {
        let t: Tipo = serde_json::from_str("\"critica\"").unwrap();
        assert_eq!(t, Tipo::Nota);
        assert_eq!(serde_json::to_string(&Tipo::Alta).unwrap(), "\"alta\"");
        let colores: HashSet<u32> = Tipo::TODOS.iter().map(|t| t.color()).collect();
        assert_eq!(colores.len(), 4);
        assert!(Tipo::Urgente.orden() < Tipo::Media.orden());
        assert!(!Tipo::Nota.es_urgencia());
        assert_eq!(Tipo::Media.siguiente(), Tipo::Nota);
        assert_eq!(Tipo::Nota.siguiente(), Tipo::Urgente);
    }

    #[test]
    fn los_adjuntos_dicen_lo_que_son() {
        assert_eq!(Adjunto::archivo("r", "Foto.JPG").tipo, TipoAdjunto::Imagen);
        assert_eq!(Adjunto::archivo("r", "plan.pdf").tipo, TipoAdjunto::Archivo);
        let a: Adjunto = serde_json::from_str(r#"{"tipo":"video3d","x":1}"#).unwrap();
        assert_eq!(a.tipo, TipoAdjunto::Otro("video3d".into()));
        let v = serde_json::to_value(&a).unwrap();
        assert_eq!(v, serde_json::json!({"tipo":"video3d","x":1}));
        let m = Adjunto::mensaje("chat1", "ABCDE23456", "");
        assert_eq!(m.rotulo, None);
        let mut p = Pegatinas::default();
        let id = p.poner(Tipo::Nota, Ancla::escena(0.0, 0.0), &quien(), 1);
        let e = p
            .escribir(
                &id,
                &quien(),
                2,
                "",
                vec![Adjunto::archivo("pins/draw/x.adjunto", "x.png")],
            )
            .unwrap();
        assert!(p.poner_tinta(&id, &e, 0, "pins/draw/x-t.excalidraw.gz", 3));
        assert_eq!(
            p.rutas(),
            vec!["pins/draw/x.adjunto", "pins/draw/x-t.excalidraw.gz"]
        );
    }
}
