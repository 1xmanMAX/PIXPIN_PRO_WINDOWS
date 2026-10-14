//! **El ejecutor de las herramientas del asistente** (encargos 2 y 3).
//!
//! Corre en el hilo del turno. Lo que va al lienzo no se puede tocar desde
//! aqui (el lienzo es de la ventana y de su hilo): se convierte a Excalidraw
//! con `pixpin_ia::lienzo`, se comprueba que el lienzo lo lee, y se le pide
//! a la UI por la cola de [`super::al_lienzo`]; el turno espera la respuesta para
//! contarle al modelo lo que paso de verdad (si no hay donde dibujar, el
//! modelo lo sabe y lo dice). El OCR si corre aqui: tarda, y asi la ventana
//! sigue suelta.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use pixpin_ia::agente::Ejecutor;
use pixpin_ia::herramientas::Llamada;
use pixpin_ia::lienzo;

use super::turno::{Adjunto, Aviso};

/// Cuanto se espera a que la UI meta lo dibujado. Normalmente es una vuelta
/// del bucle del chat (decimas); si la ventana estuviera colgada, mejor
/// decirselo al modelo que dejar el turno parado para siempre.
pub const ESPERA_LIENZO: Duration = Duration::from_secs(60);

/// **Lo ultimo que dibujo el asistente en cada conversacion** (el
/// documento), para «guarda esto como figura» sin nada elegido.
static ULTIMO_DIBUJO: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// Cuanto se espera a que el lienzo abierto diga que tiene elegido.
const ESPERA_ELEGIDO: Duration = Duration::from_secs(2);

/// Cuanto texto del OCR se le devuelve al modelo. El contexto es de 8192
/// tokens; el texto entero va aparte al chat ([`Aviso::TextoLeido`]).
pub const OCR_PARA_EL_MODELO: usize = 4000;

pub struct EjecutorDelChat {
    tx: Sender<Aviso>,
    proyecto: String,
    adjuntos: Vec<Adjunto>,
    parar: Arc<AtomicBool>,
    espera: Duration,
    llamadas: u64,
    /// El lienzo del chat donde dibujar si no hay uno abierto (la mencion
    /// `@lienzo:`), por su referencia.
    destino: Option<String>,
    /// Enganches de las acciones de la app (ver `acciones.rs`).
    confirmar: super::acciones::Confirmar,
    mandar: super::acciones::Mandar,
    /// Donde viven los chats: con ella, las tareas se piden por su nombre
    /// (`acciones::FuenteDeLaApp`).
    raiz: Option<std::path::PathBuf>,
    /// Si contesta un modelo de la nube (lo encontrado en los documentos
    /// saldria a internet: ver `documentos::permiso_nube`).
    nube: bool,
    /// Lo citado de los documentos en este turno, numerado seguido.
    citas: pixpin_ia::documentos::Citas,
}

impl EjecutorDelChat {
    pub fn nuevo(tx: Sender<Aviso>, proyecto: String, adjuntos: Vec<Adjunto>, parar: Arc<AtomicBool>) -> EjecutorDelChat {
        EjecutorDelChat {
            tx,
            proyecto,
            adjuntos,
            parar,
            espera: ESPERA_LIENZO,
            llamadas: 0,
            destino: None,
            confirmar: super::acciones::decir_que_no(),
            mandar: super::acciones::mandar_a_pixpin(),
            raiz: None,
            nube: false,
            citas: Default::default(),
        }
    }

    /// Donde estan los chats (para leer las listas de tareas).
    pub fn con_raiz(mut self, raiz: Option<std::path::PathBuf>) -> EjecutorDelChat {
        self.raiz = raiz;
        self
    }

    /// Adonde va lo dibujado si no hay un lienzo abierto.
    pub fn hacia(mut self, destino: Option<String>) -> EjecutorDelChat {
        self.destino = destino;
        self
    }

    /// Si el motor es de la nube.
    pub fn en_la_nube(mut self, nube: bool) -> EjecutorDelChat {
        self.nube = nube;
        self
    }

    /// **Busca en los documentos del usuario** desde este chat. Con la nube,
    /// solo si el usuario lo permite.
    fn buscar(&mut self, p: &pixpin_ia::documentos::PeticionBusqueda) -> Result<String, String> {
        if self.nube && !super::documentos::permiso_nube(&self.confirmar) {
            return Err(super::documentos::SIN_PERMISO_NUBE.into());
        }
        let (hallazgos, aviso) = super::documentos::buscar(p, &self.proyecto)?;
        let texto = self.citas.anadir(&p.consulta, hallazgos, aviso.as_deref());
        if !self.citas.lista.is_empty() {
            let _ = self.tx.send(Aviso::Citas(self.citas.lista.clone()));
        }
        Ok(texto)
    }

    /// Cambia como se pregunta antes de lo que borra y adonde van los
    /// pedidos (la UI, o las pruebas para no tocar la PixPin del usuario).
    pub fn con_acciones(
        mut self,
        confirmar: super::acciones::Confirmar,
        mandar: super::acciones::Mandar,
    ) -> EjecutorDelChat {
        self.confirmar = confirmar;
        self.mandar = mandar;
        self
    }

    #[cfg(test)]
    pub fn con_espera(mut self, espera: Duration) -> EjecutorDelChat {
        self.espera = espera;
        self
    }

    /// Una semilla distinta en cada llamada, para que los ids de Excalidraw
    /// no choquen con los de otra respuesta ni con los de este mismo turno.
    fn semilla(&mut self) -> u64 {
        self.llamadas += 1;
        let ahora = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        ahora.wrapping_mul(31).wrapping_add(self.llamadas)
    }

    fn al_lienzo(&mut self, llamada: &Llamada) -> Result<String, String> {
        let semilla = self.semilla();
        let elementos = lienzo::elementos(llamada, (0.0, 0.0), semilla);
        self.al_lienzo_elementos(elementos)
    }

    fn al_lienzo_elementos(&mut self, elementos: Vec<serde_json::Value>) -> Result<String, String> {
        if elementos.is_empty() {
            return Err("no salio ninguna figura".into());
        }
        let documento = lienzo::documento(elementos);
        // Que lo lea el lienzo de verdad ANTES de molestar a la ventana.
        pixpin_motor2d::excalidraw::leer(&documento).map_err(|e| {
            tracing::warn!(%e, "asistente: el lienzo no lee lo dibujado");
            "el lienzo no pudo leer las figuras".to_string()
        })?;
        if let Ok(mut u) = ULTIMO_DIBUJO.lock() {
            u.get_or_insert_with(HashMap::new).insert(self.proyecto.clone(), documento.clone());
        }
        let (responder, rx) = channel();
        super::al_lienzo::pedir(super::al_lienzo::Pedido::nuevo(
            self.proyecto.clone(),
            self.destino.clone(),
            documento,
            responder,
        ));
        let hasta = Instant::now() + self.espera;
        loop {
            if self.parar.load(Ordering::Relaxed) {
                return Err("el usuario lo paro".into());
            }
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(r) => return r,
                Err(RecvTimeoutError::Disconnected) => return Err("la ventana del chat no contesto".into()),
                Err(RecvTimeoutError::Timeout) if Instant::now() >= hasta => {
                    return Err("la ventana del chat no contesto a tiempo".into());
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }
}

impl EjecutorDelChat {
    /// **La biblioteca de figuras**, la misma del panel del lienzo.
    fn figuras(&mut self, p: &pixpin_ia::figuras::PedidoFiguras) -> Result<String, String> {
        use pixpin_ia::figuras::{PedidoFiguras, buscar, lista};
        use pixpin_motor2d::biblioteca;
        let ruta = crate::ventana_editor::ruta_biblioteca();
        match p {
            PedidoFiguras::Lista => Ok(lista(&biblioteca::cargar(&ruta))),
            PedidoFiguras::Poner { nombre } => {
                let todas = biblioteca::cargar(&ruta);
                let f = buscar(&todas, nombre)
                    .ok_or_else(|| format!("no hay ninguna figura guardada como «{nombre}» ({})", lista(&todas)))?;
                let semilla = self.semilla();
                let r = self.al_lienzo_elementos(lienzo::elementos_de_figura(f, (0.0, 0.0), semilla))?;
                Ok(format!("{r}: la figura «{}»", f.nombre))
            }
            PedidoFiguras::Guardar { nombre } => {
                let (piezas, de_donde) = self.lo_que_guardar()?;
                let figura = biblioteca::FiguraGuardada {
                    id: biblioteca::id_nuevo(),
                    nombre: nombre.clone(),
                    elementos: biblioteca::normalizar(&piezas),
                };
                if !crate::biblioteca_figuras::anadir(&ruta, figura) {
                    return Err("no se pudo escribir la biblioteca de figuras".into());
                }
                Ok(format!(
                    "guardada la figura «{nombre}» ({} piezas, {de_donde}); esta en el menu de figuras del lienzo",
                    piezas.len()
                ))
            }
        }
    }

    /// Lo elegido en el lienzo abierto; si no hay nada elegido (o ningun
    /// lienzo abierto), lo ultimo que dibujo el asistente en esta conversacion.
    fn lo_que_guardar(&self) -> Result<(Vec<pixpin_motor2d::Elemento>, &'static str), String> {
        if super::al_lienzo::hay_editor() {
            let rx = super::al_lienzo::pedir_lo_elegido();
            if let Ok(piezas) = rx.recv_timeout(ESPERA_ELEGIDO)
                && !piezas.is_empty()
            {
                return Ok((piezas, "lo elegido en el lienzo"));
            }
        }
        let ultimo = ULTIMO_DIBUJO
            .lock()
            .ok()
            .and_then(|u| u.as_ref().and_then(|m| m.get(&self.proyecto).cloned()))
            .ok_or("no hay nada elegido en el lienzo ni nada dibujado antes: elige lo que quieres guardar")?;
        let piezas = super::al_lienzo::elementos_de(&ultimo)?;
        Ok((piezas, "lo ultimo que dibuje"))
    }
}

impl Ejecutor for EjecutorDelChat {
    fn ejecutar(&mut self, llamada: &Llamada) -> Result<String, String> {
        match llamada {
            // Todo lo que se dibuja de la llamada sola (figuras, graficas,
            // SVG, funciones, ecuaciones, cronogramas): al lienzo.
            l if l.dibuja() => self.al_lienzo(l),
            Llamada::Figuras(p) => self.figuras(p),
            Llamada::Dibujar(_)
            | Llamada::Grafica(_)
            | Llamada::Svg(_)
            | Llamada::Funciones(_)
            | Llamada::Ecuacion(_)
            | Llamada::Cronograma(_) => self.al_lienzo(llamada),
            Llamada::Ocr(p) => {
                let adjunto = super::ocr::elegir(&self.adjuntos, p.archivo.as_deref())?.clone();
                let texto = super::ocr::leer(&adjunto.ruta, &p.paginas, &self.parar)?;
                let para_el_modelo = recortar(&texto, OCR_PARA_EL_MODELO);
                let _ = self.tx.send(Aviso::TextoLeido { de: adjunto, texto });
                Ok(para_el_modelo)
            }
            Llamada::Calcar(p) => {
                let ruta = super::acciones::imagen_para_calcar(&self.adjuntos, p.imagen.as_deref())?
                    .ruta
                    .clone();
                let semilla = self.semilla();
                let elementos = super::acciones::calcar_fichero(&ruta, p, semilla)?;
                self.al_lienzo_elementos(elementos)
            }
            // Los documentos (tabla escaneada, anotar, buscar dentro, abrir
            // por su nombre, exportar): con los adjuntos de este chat.
            Llamada::App(a) if pixpin_ia::acciones_documentos::es_de_documentos(a.herramienta) => {
                let ubicacion = crate::oscuro::ubicacion();
                let raiz = super::documentos::raiz()
                    .or_else(|| ubicacion.as_ref().map(|u| u.raiz().to_path_buf()))
                    .ok_or("no se sabe donde estan los datos de PixPin")?;
                let idioma = ubicacion
                    .as_ref()
                    .map(crate::compartir::idioma_de)
                    .unwrap_or(pixpin_store::Idioma::Espanol);
                super::documentos_acciones::hacer(
                    a,
                    &super::documentos_acciones::Contexto {
                        raiz: &raiz,
                        proyecto: &self.proyecto,
                        adjuntos: &self.adjuntos,
                        mandar: &self.mandar,
                        parar: &self.parar,
                        idioma,
                    },
                )
            }
            Llamada::App(a) => super::acciones::hacer(a, self.raiz.as_deref(), &self.confirmar, &self.mandar),
            Llamada::Ayuda(pregunta) => Ok(pixpin_ia::conocimiento::responder(pregunta)),
            Llamada::Buscar(p) => self.buscar(p),
        }
    }

    /// Dos o mas OCR en la misma respuesta (dos PDF adjuntos): **a la vez**,
    /// cada uno en su hilo con su COM, y el modelo espera solo al mas lento.
    /// Lo demas (lienzo, pedidos a la app), en su orden, como siempre.
    fn ejecutar_varias(&mut self, llamadas: &[Llamada]) -> Vec<Result<String, String>> {
        let ocr: Vec<usize> = (0..llamadas.len()).filter(|&i| matches!(llamadas[i], Llamada::Ocr(_))).collect();
        if ocr.len() < 2 {
            return llamadas.iter().map(|l| self.ejecutar(l)).collect();
        }
        let adjuntos = &self.adjuntos;
        let parar = &self.parar;
        let leidos: Vec<Result<(super::turno::Adjunto, String), String>> = std::thread::scope(|s| {
            let hilos: Vec<_> = ocr
                .iter()
                .map(|&i| {
                    let Llamada::Ocr(p) = &llamadas[i] else { unreachable!() };
                    s.spawn(move || {
                        let _com = pixpin_shell::ComDelHilo::iniciar();
                        let a = super::ocr::elegir(adjuntos, p.archivo.as_deref())?.clone();
                        let t = super::ocr::leer(&a.ruta, &p.paginas, parar)?;
                        Ok((a, t))
                    })
                })
                .collect();
            hilos
                .into_iter()
                .map(|h| h.join().unwrap_or_else(|_| Err("el OCR se detuvo de golpe".into())))
                .collect()
        });
        let mut salida: Vec<Option<Result<String, String>>> = vec![None; llamadas.len()];
        for (&i, r) in ocr.iter().zip(leidos) {
            salida[i] = Some(r.map(|(de, texto)| {
                let para_el_modelo = recortar(&texto, OCR_PARA_EL_MODELO);
                let _ = self.tx.send(Aviso::TextoLeido { de, texto });
                para_el_modelo
            }));
        }
        for (i, l) in llamadas.iter().enumerate() {
            if salida[i].is_none() {
                salida[i] = Some(self.ejecutar(l));
            }
        }
        salida.into_iter().map(|r| r.unwrap_or_else(|| Err("no se hizo".into()))).collect()
    }
}

/// Recorta a `n` letras diciendolo, para que el modelo no crea que el
/// documento acaba ahi.
pub fn recortar(texto: &str, n: usize) -> String {
    if texto.chars().count() <= n {
        return texto.to_string();
    }
    let mut s: String = texto.chars().take(n).collect();
    s.push_str("\n[...texto recortado: el usuario tiene el texto entero en el chat]");
    s
}
