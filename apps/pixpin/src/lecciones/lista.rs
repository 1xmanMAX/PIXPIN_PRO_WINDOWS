//! **Las lecciones aprendidas**, rediseno v2 (maquetas
//! `Lecciones2.dc.html` y `Lecciones2-repaso.dc.html`, aprobadas el
//! 4-oct-2026).
//!
//! Una ventana en dos columnas (lista y ficha), con la barra de apuntar
//! arriba. El 5-oct-2026 el usuario pidio simplificarla («esta muy
//! saturada»): se quito la columna derecha de datos, los textos de ayuda
//! (la pista bajo la barra, «Ctrl V pega capturas», «clic para editar», la
//! leyenda de colores y «↑↓ moverse»), las fichas de filtro por area y el
//! recuadro grande de «Anadir una foto». Los atajos siguen valiendo; lo que
//! servia de la columna derecha cabe en una fila bajo el titulo y en «Mas
//! campos…».
//!
//! - **«¿Que aprendiste?»**: una frase (escrita, dictada con Ctrl+Mayus+D o
//!   con capturas pegadas con Ctrl+V) y Enter. El area, el proyecto y la
//!   gravedad se rellenan solos (`pixpin_lecciones::rapida`) y se cambian
//!   con un clic. Si ya habia una parecida, se ofrece «me volvio a pasar»,
//!   como la hoja del movil.
//! - **La lista**: buscador (Ctrl+F), filtros Todas / Repetidas / Graves con
//!   su cuenta, «Para repasar hoy» en una linea (R; solo si toca alguna) y
//!   las lecciones de este mes y de antes, con el punto de su gravedad.
//! - **La ficha**: el titulo; debajo, una fila con la gravedad (clic = la
//!   siguiente), cuantas veces paso («3×»), «Me volvio a pasar +1» y «Mas
//!   campos…», que despliega dentro de la ficha el area, el proyecto, las
//!   etiquetas, las relacionadas, el proximo repaso y el enlace a la ficha
//!   completa (tipo, causas, palabras). Luego los tres bloques (Que paso,
//!   Por que, La proxima vez), que se editan con un clic y se guardan solos,
//!   con las fotos y notas de voz en «Que paso». Abajo, «Ver en el chat»,
//!   «Pinear» (P) y «Borrar» (Supr, con «Deshacer» unos segundos en vez de
//!   preguntar).
//! - **El repaso** ([`repaso`]), de una en una: pensar que harias, mirar lo
//!   apuntado y contestar con 1, 2 o 3 (cada boton dice cuando vuelve).
//!
//! Lo de Android que sigue igual: el formato de cada leccion, donde vive,
//! el buscador, el repaso espaciado (con «a medias» de mas, que solo mueve
//! `caja` y `repasar`) y la lista de comprobacion (el boton de casillas
//! junto al buscador).
//!
//! Una sola ventana: pedirla otra vez la trae delante con lo pedido. Se
//! pone al dia sola cuando se guarda una leccion en otra ventana o llega
//! algo sincronizando.

#![forbid(unsafe_code)]

mod adjuntos;
mod pintar;
mod repaso;

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_geom::Rect;
use pixpin_lecciones::buscador::{self, Indice};
use pixpin_lecciones::etiquetador::{self, Aprendido};
use pixpin_lecciones::leccion::{self, AREAS};
use pixpin_lecciones::rapida::{self, Relleno};
use pixpin_lecciones::{Leccion, Repaso};
use pixpin_render::{Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma, Ubicacion};

use super::almacen::{self, Donde, Entrada};
use super::dictar::{Dictado, Salida};
use super::ficha;
use super::ui::{Campo, VK_ENTRAR, VK_ESCAPE, VK_SUPRIMIR, VK_TAB, VK_V};
use crate::overlay::Recursos;
use crate::ventanita::{Botones, centrado};

const VK_ESPACIO: u32 = 0x20;
const VK_ARRIBA: u32 = 0x26;
const VK_ABAJO: u32 = 0x28;
const VK_1: u32 = 0x31;
const VK_2: u32 = 0x32;
const VK_3: u32 = 0x33;
const VK_D: u32 = 0x44;
const VK_F: u32 = 0x46;
const VK_N: u32 = 0x4E;
const VK_P: u32 = 0x50;
const VK_R: u32 = 0x52;
const VK_S: u32 = 0x53;
const VK_Z: u32 = 0x5A;

/// Lo que dura el «Deshacer» de borrar antes de borrar de verdad.
const PARA_DESHACER: Duration = Duration::from_secs(6);

#[derive(Debug, Clone)]
pub struct Pedido {
    pub ubicacion: Ubicacion,
    pub idioma: Idioma,
    pub aparato: String,
    /// La ficha de un proyecto: se ven primero las suyas.
    pub proyecto: Option<String>,
    pub consulta: Option<String>,
    /// La leccion que se ensena al abrir (la de «editar»).
    pub seleccion: Option<String>,
}

/// La ventana abierta (su HWND), -1 mientras nace, 0 sin ventana.
static ABIERTA: AtomicIsize = AtomicIsize::new(0);
/// Lo pedido mientras ya estaba abierta: lo recoge su bucle.
static PENDIENTE: Mutex<Option<Pedido>> = Mutex::new(None);

/// Abre la lista, o la trae delante con lo pedido si ya estaba.
pub fn abrir(pedido: Pedido) {
    let ya = ABIERTA.load(Ordering::SeqCst);
    if ya > 0 {
        *PENDIENTE.lock().unwrap_or_else(|e| e.into_inner()) = Some(pedido);
        VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(ya as *mut _));
        pixpin_shell::overlay::despertar(ya);
        return;
    }
    if ya < 0 {
        // Naciendo: lo recoge en su primera vuelta.
        *PENDIENTE.lock().unwrap_or_else(|e| e.into_inner()) = Some(pedido);
        return;
    }
    ABIERTA.store(-1, Ordering::SeqCst);
    let lanzado = std::thread::Builder::new()
        .name("lecciones".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let hecho =
                crate::dispositivo_perdido::con_recursos("lecciones", |r| bucle(r, pedido.clone()));
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir la lista de lecciones");
            }
            ABIERTA.store(0, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de las lecciones");
        ABIERTA.store(0, Ordering::SeqCst);
    }
}

/// Donde va lo que se escribe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Foco {
    /// La barra «¿Que aprendiste?».
    Rapida,
    Buscar,
    /// Los textos de la ficha, editandose en su sitio.
    Titulo,
    Paso,
    PorQue,
    Proxima,
    /// «+ Anadir» etiqueta.
    Etiqueta,
    /// «¿Que harias?» del repaso.
    Respuesta,
}

impl Foco {
    fn es_de_la_ficha(self) -> bool {
        matches!(
            self,
            Foco::Titulo | Foco::Paso | Foco::PorQue | Foco::Proxima | Foco::Etiqueta
        )
    }

    fn multilinea(self) -> bool {
        matches!(
            self,
            Foco::Paso | Foco::PorQue | Foco::Proxima | Foco::Respuesta
        )
    }
}

/// El filtro de las fichas de arriba de la lista: uno a la vez. Las fichas
/// por area se quitaron al simplificar (5-oct-2026): ocupaban otro renglon
/// y el buscador ya encuentra por area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filtro {
    Todas,
    Repetidas,
    Graves,
}

/// Los menus que se despliegan bajo un boton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Menu {
    /// El area de la barra rapida.
    AreaRapida,
    /// El proyecto de la barra rapida.
    ProyectoRapida,
    /// El area de la leccion elegida.
    AreaFicha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Mover,
    Cerrar,
    Fondo,
    // La barra rapida.
    FocoRapida,
    DictarRapida,
    ImagenRapida,
    QuitarFoto(usize),
    Guardar,
    AbrirMenu(Menu),
    CicloGravedad,
    YaLaTengo,
    /// Una opcion del menu abierto, por su indice (ver `opciones_del_menu`).
    Opcion(usize),
    // La lista.
    FocoBuscar,
    BorrarConsulta,
    Filtro(usize),
    QuitarProyecto,
    Comprobacion,
    Marcar(usize),
    Repasar,
    Fila(usize),
    // La ficha.
    Editar(Foco),
    /// Despliega o pliega «Mas campos…» dentro de la ficha.
    MasCampos,
    /// La ficha de siempre en su ventana (tipo, causas, palabras).
    FichaCompleta,
    Adjunto(usize),
    AnadirAdjunto,
    VerEnChat,
    Pinear,
    Borrar,
    Deshacer,
    OtraVez,
    /// La gravedad de la elegida pasa a la siguiente (Leve → Importante →
    /// Grave → Leve): un solo punto de color en vez de tres botones.
    OtraGravedad,
    QuitarEtiqueta(usize),
    Relacionada(usize),
    // El repaso.
    Mostrar,
    Nota(usize),
    Siguiente,
    Saltar,
    SalirRepaso,
    AbrirCompleta,
    DictarRespuesta,
    FocoRespuesta,
}

/// La barra «¿Que aprendiste?».
#[derive(Default)]
struct Rapida {
    campo: Campo,
    relleno: Relleno,
    /// La frase con que se calculo `relleno`.
    mirado: String,
    /// Lo cambiado a mano: gana a lo propuesto. `Some("")` = sin area / sin
    /// proyecto (chat general).
    area: Option<String>,
    gravedad: Option<i64>,
    proyecto: Option<String>,
    /// Capturas pegadas o elegidas: van con la leccion al guardar.
    fotos: Vec<almacen::Foto>,
    /// La que ya existe y se parece mucho (`parecidas` del movil).
    parecida: Option<Leccion>,
}

/// Lo que se ve a la izquierda, ya filtrado: indices en `todas`.
struct Estado {
    pedido: Pedido,
    textos: std::rc::Rc<Catalogo>,
    todas: Vec<Entrada>,
    indices: Vec<Indice>,
    aprendido: Aprendido,
    firma: Vec<Option<std::time::SystemTime>>,
    cambios: u64,
    /// `(ficha, nombre)` de los chats que no son el general, del mas tocado
    /// al menos.
    proyectos: Vec<(String, String)>,
    /// La ficha de «Mensajes guardados», si existe.
    general: Option<String>,
    consulta: Campo,
    filtro: Filtro,
    proyecto: Option<String>,
    visibles: Vec<usize>,
    #[allow(clippy::type_complexity)]
    mirado: Option<(String, Filtro, Option<String>, usize, Option<String>)>,
    sel: Option<String>,
    foco: Option<Foco>,
    /// Lo que se esta editando en la ficha.
    editando: Campo,
    /// «Mas campos…» desplegado. Sigue asi al cambiar de leccion: quien lo
    /// abre quiere ver esos datos en todas.
    mas_campos: bool,
    rapida: Rapida,
    menu: Option<Menu>,
    /// Donde se pinto el boton del menu abierto: el menu cuelga de ahi.
    ancla: RectF,
    comprobacion: bool,
    marcadas: HashSet<String>,
    hoy: Vec<Leccion>,
    repaso: Option<repaso::Repaso>,
    /// Borrada pero aun con «Deshacer»: se borra de verdad al acabar el
    /// plazo o al cerrar la ventana.
    borrando: Option<(Entrada, Instant)>,
    aviso: Option<(String, Instant)>,
    dictado: Dictado,
    /// A quien va lo dictado.
    dictando: Foco,
    botones: Botones<Accion>,
    moviendo: Option<(pixpin_geom::Punto, Rect)>,
    scroll_lista: f32,
    alto_lista: f32,
    scroll_ficha: f32,
    alto_ficha: f32,
    /// Lo que se ve de la lista y de la ficha (del ultimo fotograma), para
    /// llevar la rueda a la columna que toca.
    zona_lista: RectF,
    zona_ficha: RectF,
    /// La fila elegida debe verse: se desplaza al pintar.
    seguir_sel: bool,
    adjuntos: adjuntos::Cache,
    relacionadas: Option<(String, i64, Vec<Leccion>)>,
    ahora: i64,
    /// Para pinear la tarjeta y las miniaturas.
    motor: Option<std::rc::Rc<pixpin_render::MotorRender>>,
    d3d: Option<windows::Win32::Graphics::Direct3D11::ID3D11Device>,
    hwnd: isize,
}

impl Estado {
    fn nuevo(pedido: Pedido) -> Estado {
        let textos = std::rc::Rc::new(Catalogo::nuevo(pedido.idioma));
        let consulta = Campo::con(pedido.consulta.as_deref().unwrap_or(""));
        Estado {
            proyecto: pedido.proyecto.clone(),
            sel: pedido.seleccion.clone(),
            foco: Some(Foco::Rapida),
            pedido,
            textos,
            todas: Vec::new(),
            indices: Vec::new(),
            aprendido: Aprendido::default(),
            firma: Vec::new(),
            cambios: 0,
            proyectos: Vec::new(),
            general: None,
            consulta,
            filtro: Filtro::Todas,
            visibles: Vec::new(),
            mirado: None,
            editando: Campo::default(),
            mas_campos: false,
            rapida: Rapida::default(),
            menu: None,
            ancla: RectF {
                x: 0.0,
                y: 0.0,
                ancho: 0.0,
                alto: 0.0,
            },
            comprobacion: false,
            marcadas: HashSet::new(),
            hoy: Vec::new(),
            repaso: None,
            borrando: None,
            aviso: None,
            dictado: Dictado::Nada,
            dictando: Foco::Rapida,
            botones: Botones::default(),
            moviendo: None,
            scroll_lista: 0.0,
            alto_lista: 0.0,
            scroll_ficha: 0.0,
            alto_ficha: 0.0,
            zona_lista: RectF {
                x: 0.0,
                y: 0.0,
                ancho: 0.0,
                alto: 0.0,
            },
            zona_ficha: RectF {
                x: 0.0,
                y: 0.0,
                ancho: 0.0,
                alto: 0.0,
            },
            seguir_sel: true,
            adjuntos: adjuntos::Cache::default(),
            relacionadas: None,
            ahora: pixpin_shell::entorno::ahora_utc_ms(),
            motor: None,
            d3d: None,
            hwnd: 0,
        }
    }

    fn t(&self, clave: &str) -> String {
        self.textos.t(clave)
    }

    fn t1(
        &self,
        clave: &str,
        k: &str,
        v: impl Into<fluent_bundle::FluentValue<'static>>,
    ) -> String {
        t1(&self.textos, clave, k, v)
    }

    fn avisar(&mut self, texto: String) {
        self.aviso = Some((texto, Instant::now()));
    }

    fn raiz(&self) -> std::path::PathBuf {
        self.pedido.ubicacion.raiz().to_path_buf()
    }

    fn recargar(&mut self) {
        let raiz = self.raiz();
        self.todas = almacen::listar(&raiz);
        self.indices = self
            .todas
            .iter()
            .map(|e| Indice::nuevo(e.leccion.clone()))
            .collect();
        self.firma = almacen::firma(&raiz, &self.todas);
        self.cambios = almacen::cambios();
        let lecciones: Vec<Leccion> = self.todas.iter().map(|e| e.leccion.clone()).collect();
        self.aprendido = etiquetador::aprender(&lecciones);
        let indice = pixpin_proyecto::almacen::Indice::leer(&raiz);
        let mut proyectos: Vec<_> = indice.proyectos.iter().collect();
        proyectos.sort_by_key(|x| std::cmp::Reverse(x.tocado));
        self.general = proyectos
            .iter()
            .find(|f| f.es_guardados())
            .map(|f| f.id.clone());
        self.proyectos = proyectos
            .iter()
            .filter(|f| !f.es_guardados())
            .map(|f| (f.id.clone(), f.nombre.clone()))
            .collect();
        self.ahora = pixpin_shell::entorno::ahora_utc_ms();
        self.hoy = Repaso::de_hoy(&lecciones, self.ahora, 20);
        self.mirado = None;
        self.relacionadas = None;
        self.rapida.mirado = String::from("\u{0}");
        self.filtrar();
        self.recalcular_rapida();
    }

    /// Las areas: las de inicio y las que salen en alguna leccion.
    fn areas(&self) -> Vec<String> {
        let mut v: Vec<String> = AREAS.iter().map(|a| a.to_string()).collect();
        for e in &self.todas {
            if !e.leccion.area.trim().is_empty() && !v.contains(&e.leccion.area) {
                v.push(e.leccion.area.clone());
            }
        }
        v
    }

    /// Las fichas de filtro, con su cuenta: Todas, Repetidas y Graves.
    fn filtros(&self) -> Vec<(Filtro, usize)> {
        let vivas: Vec<&Leccion> = self
            .todas
            .iter()
            .map(|e| &e.leccion)
            .filter(|l| !self.se_esta_borrando(&l.id))
            .collect();
        vec![
            (Filtro::Todas, vivas.len()),
            (
                Filtro::Repetidas,
                vivas.iter().filter(|l| !l.repeticiones.is_empty()).count(),
            ),
            (
                Filtro::Graves,
                vivas.iter().filter(|l| l.gravedad >= 3).count(),
            ),
        ]
    }

    fn se_esta_borrando(&self, id: &str) -> bool {
        self.borrando
            .as_ref()
            .is_some_and(|(x, _)| x.leccion.id == id)
    }

    /// Lo que se ve con el filtro y la busqueda puestos.
    fn filtrar(&mut self) {
        let clave = (
            self.consulta.texto.clone(),
            self.filtro,
            self.proyecto.clone(),
            self.todas.len(),
            self.borrando.as_ref().map(|(x, _)| x.leccion.id.clone()),
        );
        if self.mirado.as_ref() == Some(&clave) {
            return;
        }
        let mut base: Vec<usize> = (0..self.todas.len())
            .filter(|i| {
                let l = &self.todas[*i].leccion;
                !self.se_esta_borrando(&l.id)
                    && match &self.filtro {
                        Filtro::Todas => true,
                        Filtro::Repetidas => !l.repeticiones.is_empty(),
                        Filtro::Graves => l.gravedad >= 3,
                    }
            })
            .collect();
        if let Some(p) = &self.proyecto {
            // Las del proyecto y las que hablan de lo mismo que su nombre.
            let nombre = self.nombre_de(p).unwrap_or_default();
            let ix: Vec<Indice> = base.iter().map(|i| self.indices[*i].clone()).collect();
            let cerca: HashSet<String> = buscador::para_el_contexto(&ix, &nombre, 10)
                .into_iter()
                .map(|l| l.id)
                .collect();
            let mut v: Vec<usize> = base
                .iter()
                .copied()
                .filter(|i| self.todas[*i].ficha == *p)
                .collect();
            for i in base {
                if cerca.contains(&self.todas[i].leccion.id) && !v.contains(&i) {
                    v.push(i);
                }
            }
            base = v;
        }
        if !self.consulta.texto.trim().is_empty() {
            let ix: Vec<Indice> = base.iter().map(|i| self.indices[*i].clone()).collect();
            let por_id: HashMap<String, usize> = base
                .iter()
                .map(|i| (self.todas[*i].leccion.id.clone(), *i))
                .collect();
            base = buscador::buscar(&ix, &self.consulta.texto)
                .into_iter()
                .filter_map(|r| por_id.get(&r.leccion.id).copied())
                .collect();
        }
        self.visibles = base;
        self.scroll_lista = 0.0;
        self.mirado = Some(clave);
        // La elegida sigue si se ve; si no, la primera.
        if !self.sel.as_ref().is_some_and(|s| {
            self.visibles
                .iter()
                .any(|i| self.todas[*i].leccion.id == *s)
        }) {
            self.sel = self
                .visibles
                .first()
                .map(|i| self.todas[*i].leccion.id.clone());
            self.seguir_sel = true;
        }
    }

    fn nombre_de(&self, ficha: &str) -> Option<String> {
        self.proyectos
            .iter()
            .find(|p| p.0 == ficha)
            .map(|p| p.1.clone())
    }

    /// La entrada elegida.
    fn elegida(&self) -> Option<&Entrada> {
        let id = self.sel.as_ref()?;
        self.todas
            .iter()
            .find(|e| e.leccion.id == *id && !self.se_esta_borrando(id))
    }

    fn aplicar(&mut self, p: Pedido) {
        self.proyecto = p.proyecto.clone();
        if let Some(c) = &p.consulta {
            self.consulta.poner(c);
        }
        if let Some(s) = &p.seleccion {
            self.sel = Some(s.clone());
            self.seguir_sel = true;
            self.consulta.poner("");
            self.filtro = Filtro::Todas;
            self.repaso = None;
            self.comprobacion = false;
        }
        self.pedido = p;
        self.mirado = None;
        self.filtrar();
    }

    fn ficha(&self, que: ficha::Que) -> ficha::Pedido {
        ficha::Pedido {
            ubicacion: self.pedido.ubicacion.clone(),
            idioma: self.pedido.idioma,
            aparato: self.pedido.aparato.clone(),
            que,
        }
    }

    // ------------------------------------------------- la barra rapida

    /// Lo propuesto para la frase, si cambio.
    fn recalcular_rapida(&mut self) {
        let frase = self.rapida.campo.texto.clone();
        if frase == self.rapida.mirado {
            return;
        }
        let quien: HashMap<String, String> = self
            .todas
            .iter()
            .filter(|e| !e.general)
            .map(|e| (e.leccion.id.clone(), e.ficha.clone()))
            .collect();
        let de_quien = |id: &str| quien.get(id).cloned();
        self.rapida.relleno = rapida::rellenar(
            &frase,
            &self.aprendido,
            &self.indices,
            &self.proyectos,
            &de_quien,
        );
        self.rapida.parecida = buscador::parecidas_por_defecto(&self.indices, &frase)
            .into_iter()
            .map(|r| r.leccion)
            .find(|l| !self.se_esta_borrando(&l.id));
        self.rapida.mirado = frase;
    }

    fn area_rapida(&self) -> Option<String> {
        self.rapida
            .area
            .clone()
            .or_else(|| self.rapida.relleno.propuesta.area.clone())
            .filter(|a| !a.is_empty())
    }

    fn gravedad_rapida(&self) -> i64 {
        self.rapida
            .gravedad
            .unwrap_or(self.rapida.relleno.gravedad)
            .clamp(1, 3)
    }

    /// La ficha donde ira la leccion de la barra: la elegida a mano, la
    /// propuesta, la del proyecto con que se abrio la lista o el chat
    /// general.
    fn proyecto_rapida(&self) -> Option<String> {
        match &self.rapida.proyecto {
            Some(p) if p.is_empty() => None,
            Some(p) => Some(p.clone()),
            None => self
                .rapida
                .relleno
                .proyecto
                .clone()
                .or_else(|| self.proyecto.clone())
                .filter(|p| self.proyectos.iter().any(|x| x.0 == *p)),
        }
    }

    /// **Guarda lo de la barra** como leccion nueva y la elige.
    fn guardar_rapida(&mut self) {
        let frase = self.rapida.campo.texto.trim().to_string();
        if frase.is_empty() {
            self.foco = Some(Foco::Rapida);
            return;
        }
        self.recalcular_rapida();
        let raiz = self.raiz();
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        let ficha = match self.proyecto_rapida().or_else(|| self.general.clone()) {
            Some(f) => f,
            None => match pixpin_proyecto::almacen::asegurar_guardados(
                &raiz,
                ahora,
                &self.pedido.aparato,
            ) {
                Ok(f) => f.id,
                Err(err) => {
                    tracing::warn!(?err, "sin chat general donde guardar la leccion");
                    self.avisar(self.t("lec2-no-se-pudo"));
                    return;
                }
            },
        };
        let area = self.area_rapida();
        let l = rapida::leccion(
            &frase,
            &leccion::nuevo_id(ahora),
            ahora,
            &self.rapida.relleno.propuesta,
            Some(area.as_deref().unwrap_or("")),
            self.gravedad_rapida(),
        );
        if l.titulo.is_empty() {
            self.foco = Some(Foco::Rapida);
            return;
        }
        match almacen::guardar_con_fotos(
            &raiz,
            &l,
            &Donde::Nueva {
                ficha: ficha.clone(),
            },
            &self.pedido.aparato,
            &self.rapida.fotos,
        ) {
            Ok(_) => {
                tracing::info!(id = %l.id, "leccion apuntada desde la barra");
                let donde = self
                    .nombre_de(&ficha)
                    .unwrap_or_else(|| self.t("lec2-sin-proyecto"));
                self.avisar(self.t1("lec2-guardada", "donde", donde));
                self.rapida = Rapida::default();
                self.rapida.mirado = String::from("\u{0}");
                self.sel = Some(l.id.clone());
                self.seguir_sel = true;
                self.filtro = Filtro::Todas;
                self.consulta.poner("");
                self.recargar();
                self.sel = Some(l.id);
            }
            Err(err) => {
                tracing::warn!(?err, "no se pudo guardar la leccion de la barra");
                self.avisar(self.t("lec2-no-se-pudo"));
            }
        }
    }

    /// Pega en la barra: una captura va como foto; un texto, al texto.
    fn pegar_en_la_barra(&mut self, contenido: Option<pixpin_codec::ContenidoPortapapeles>) {
        use pixpin_codec::ContenidoPortapapeles as C;
        match contenido {
            Some(C::Imagen(img)) => match pixpin_codec::imagen::codificar_png(&img) {
                Ok(png) => {
                    let n = self.rapida.fotos.len() + 1;
                    self.rapida
                        .fotos
                        .push((format!("leccion-{}-{n}.png", self.ahora), png));
                }
                Err(err) => tracing::warn!(?err, "la captura pegada no se pudo pasar a PNG"),
            },
            Some(C::Rutas(rutas)) => self.anadir_fotos_de(&rutas),
            Some(C::Texto(t)) => {
                let t = t.replace("\r\n", "\n").replace('\n', " ");
                self.rapida.campo.escribir(&t);
            }
            None => {}
        }
        self.foco = Some(Foco::Rapida);
    }

    /// Las imagenes de unas rutas, a la barra. Lo que no es imagen se dice.
    fn anadir_fotos_de(&mut self, rutas: &[std::path::PathBuf]) {
        let mut no = 0;
        for r in rutas {
            match (pixpin_codec::cargar(r), std::fs::read(r)) {
                (Ok(_), Ok(bytes)) => {
                    let nombre = r
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "foto.png".into());
                    self.rapida.fotos.push((nombre, bytes));
                }
                _ => no += 1,
            }
        }
        if no > 0 {
            self.avisar(self.t("lec2-solo-imagenes"));
        }
    }

    // --------------------------------------------------------- la ficha

    /// Empieza a editar un texto de la elegida (o lo deja si ya estaba).
    fn editar(&mut self, f: Foco) {
        if self.foco == Some(f) {
            return;
        }
        self.terminar_edicion();
        let Some(l) = self.elegida().map(|e| e.leccion.clone()) else {
            return;
        };
        let texto = match f {
            Foco::Titulo => l.titulo,
            Foco::Paso => l.que_paso,
            Foco::PorQue => l.por_que,
            Foco::Proxima => l.proxima,
            _ => String::new(),
        };
        self.editando = Campo::con(&texto);
        self.foco = Some(f);
    }

    /// Guarda lo editado en la ficha (si cambio) y suelta el foco.
    fn terminar_edicion(&mut self) {
        let Some(f) = self.foco.filter(|f| f.es_de_la_ficha()) else {
            return;
        };
        self.foco = None;
        let texto = self.editando.texto.trim().to_string();
        let Some(x) = self.elegida().cloned() else {
            return;
        };
        let mut l = x.leccion.clone();
        match f {
            Foco::Titulo if !texto.is_empty() => l.titulo = texto,
            Foco::Titulo => {}
            Foco::Paso => l.que_paso = texto,
            Foco::PorQue => l.por_que = texto,
            Foco::Proxima => l.proxima = texto,
            Foco::Etiqueta => {
                let e = texto.trim_matches([' ', ',', '#']).to_lowercase();
                if !e.is_empty() && !l.todas_las_etiquetas().contains(&e) {
                    l.etiquetas.push(e.clone());
                }
                l.quitadas.retain(|q| *q != e);
            }
            _ => {}
        }
        self.editando = Campo::default();
        if l != x.leccion {
            self.guardar_elegida(l);
        }
    }

    /// Guarda la leccion elegida tal como queda.
    fn guardar_elegida(&mut self, mut l: Leccion) {
        let Some(x) = self.elegida().cloned() else {
            return;
        };
        l.tocada = pixpin_shell::entorno::ahora_utc_ms();
        match almacen::guardar(&self.raiz(), &l, &Donde::de(&x), &self.pedido.aparato) {
            Ok(_) => {
                if let Some(e) = self.todas.iter_mut().find(|e| e.leccion.id == l.id) {
                    e.leccion = l.clone();
                }
                if let Some(ix) = self.indices.iter_mut().find(|ix| ix.leccion.id == l.id) {
                    *ix = Indice::nuevo(l);
                }
                // Se da por vista: lo guardado aqui no hace recargar.
                self.cambios = almacen::cambios();
                self.firma = almacen::firma(&self.raiz(), &self.todas);
                self.relacionadas = None;
                self.mirado = None;
                self.filtrar();
            }
            Err(err) => {
                tracing::warn!(?err, "no se pudo guardar la leccion");
                self.avisar(self.t("lec2-no-se-pudo"));
            }
        }
    }

    /// Mueve la eleccion `paso` filas (las flechas).
    fn mover_sel(&mut self, paso: i32) {
        if self.visibles.is_empty() {
            return;
        }
        let i = self
            .sel
            .as_ref()
            .and_then(|s| {
                self.visibles
                    .iter()
                    .position(|i| self.todas[*i].leccion.id == *s)
            })
            .unwrap_or(0) as i32;
        let j = (i + paso).clamp(0, self.visibles.len() as i32 - 1) as usize;
        self.terminar_edicion();
        self.sel = Some(self.todas[self.visibles[j]].leccion.id.clone());
        self.seguir_sel = true;
        self.scroll_ficha = 0.0;
    }

    /// **Borrar con deshacer**: se quita de la vista ya y se borra de verdad
    /// al acabar el plazo (o al cerrar). Si ya habia otra esperando, esa se
    /// borra ahora.
    fn borrar_elegida(&mut self) {
        self.terminar_edicion();
        let Some(x) = self.elegida().cloned() else {
            return;
        };
        self.borrar_ya();
        // La de debajo queda elegida, como en una lista de correo.
        let pos = self
            .visibles
            .iter()
            .position(|i| self.todas[*i].leccion.id == x.leccion.id);
        let siguiente = pos.and_then(|p| {
            self.visibles
                .get(p + 1)
                .or_else(|| p.checked_sub(1).and_then(|q| self.visibles.get(q)))
        });
        self.sel = siguiente.map(|i| self.todas[*i].leccion.id.clone());
        let quitada = x.leccion.id.clone();
        self.borrando = Some((x, Instant::now()));
        self.mirado = None;
        self.filtrar();
        self.hoy.retain(|l| l.id != quitada);
        self.aviso = None;
    }

    /// Borra de verdad la que esperaba su «Deshacer».
    fn borrar_ya(&mut self) {
        if let Some((x, _)) = self.borrando.take() {
            match almacen::borrar(&self.raiz(), &x) {
                Ok(()) => tracing::info!(id = %x.leccion.id, "leccion borrada"),
                Err(err) => tracing::warn!(?err, "no se pudo borrar la leccion"),
            }
        }
    }

    fn deshacer(&mut self) {
        if let Some((x, _)) = self.borrando.take() {
            self.sel = Some(x.leccion.id.clone());
            self.seguir_sel = true;
            self.mirado = None;
            self.filtrar();
            let lecciones: Vec<Leccion> = self.todas.iter().map(|e| e.leccion.clone()).collect();
            self.hoy = Repaso::de_hoy(&lecciones, self.ahora, 20);
        }
    }

    /// **Pinear** la elegida: su tarjeta, pintada como imagen, a la ventana
    /// principal, que es quien tiene los pines.
    fn pinear(&mut self) {
        let Some(l) = self.elegida().map(|e| e.leccion.clone()) else {
            return;
        };
        let (Some(motor), Some(d3d)) = (self.motor.clone(), self.d3d.clone()) else {
            return;
        };
        match adjuntos::tarjeta_png(&motor, &d3d, &l, &self.textos) {
            Ok(ruta) if pixpin_shell::mensajero::enviar_ficheros(std::slice::from_ref(&ruta)) => {
                self.avisar(self.t("lec2-pineado"));
            }
            Ok(_) => self.avisar(self.t("lec2-no-se-pudo")),
            Err(err) => {
                tracing::warn!(?err, "no se pudo pintar la tarjeta de la leccion");
                self.avisar(self.t("lec2-no-se-pudo"));
            }
        }
    }

    /// Fotos nuevas para la elegida (elegidas en un dialogo o pegadas).
    fn anadir_a_la_elegida(&mut self, fotos: Vec<almacen::Foto>) {
        let Some(x) = self.elegida().cloned() else {
            return;
        };
        if fotos.is_empty() {
            return;
        }
        let mut l = x.leccion.clone();
        l.tocada = pixpin_shell::entorno::ahora_utc_ms();
        match almacen::guardar_con_fotos(
            &self.raiz(),
            &l,
            &Donde::de(&x),
            &self.pedido.aparato,
            &fotos,
        ) {
            Ok(_) => {
                self.adjuntos.olvidar(&l.id);
                self.recargar();
            }
            Err(err) => {
                tracing::warn!(?err, "no se pudieron anadir las fotos a la leccion");
                self.avisar(self.t("lec2-no-se-pudo"));
            }
        }
    }

    fn relacionadas(&mut self) -> Vec<Leccion> {
        let Some(l) = self.elegida().map(|e| e.leccion.clone()) else {
            return Vec::new();
        };
        if let Some((id, t, v)) = &self.relacionadas
            && *id == l.id
            && *t == l.tocada
        {
            return v.clone();
        }
        let vivos: Vec<Indice> = self
            .indices
            .iter()
            .filter(|ix| !self.se_esta_borrando(&ix.leccion.id))
            .cloned()
            .collect();
        let v = buscador::relacionadas(&vivos, &l, 3);
        self.relacionadas = Some((l.id.clone(), l.tocada, v.clone()));
        v
    }

    /// Las opciones del menu abierto: `(rotulo, valor)`. Valor vacio = sin
    /// area / sin proyecto.
    fn opciones_del_menu(&self, m: Menu) -> Vec<(String, String)> {
        match m {
            Menu::AreaRapida | Menu::AreaFicha => {
                let mut v: Vec<(String, String)> =
                    self.areas().into_iter().map(|a| (a.clone(), a)).collect();
                v.push((self.t("lec2-sin-area"), String::new()));
                v
            }
            Menu::ProyectoRapida => {
                let mut v: Vec<(String, String)> = self
                    .proyectos
                    .iter()
                    .take(14)
                    .map(|(f, n)| (n.clone(), f.clone()))
                    .collect();
                v.push((self.t("lec2-sin-proyecto"), String::new()));
                v
            }
        }
    }

    fn elegir_opcion(&mut self, i: usize) {
        let Some(m) = self.menu.take() else {
            return;
        };
        let Some((_, valor)) = self.opciones_del_menu(m).get(i).cloned() else {
            return;
        };
        match m {
            Menu::AreaRapida => self.rapida.area = Some(valor),
            Menu::ProyectoRapida => self.rapida.proyecto = Some(valor),
            Menu::AreaFicha => {
                if let Some(mut l) = self.elegida().map(|e| e.leccion.clone()) {
                    l.area = valor;
                    self.guardar_elegida(l);
                }
            }
        }
    }

    // --------------------------------------------------------- el repaso

    fn empezar_repaso(&mut self) {
        self.terminar_edicion();
        let ids: Vec<String> = self.hoy.iter().map(|l| l.id.clone()).collect();
        if ids.is_empty() {
            self.avisar(self.t("lec2-nada-que-repasar"));
            return;
        }
        self.repaso = Some(repaso::Repaso::nuevo(ids));
        self.menu = None;
        self.foco = Some(Foco::Respuesta);
    }

    /// Contesta la de ahora con `nota` y pasa a la siguiente.
    fn calificar(&mut self, nota: pixpin_lecciones::Nota) {
        let Some(id) = self
            .repaso
            .as_ref()
            .and_then(|r| r.actual().map(str::to_string))
        else {
            return;
        };
        if let Some(x) = self.todas.iter().find(|x| x.leccion.id == id).cloned() {
            let ahora = pixpin_shell::entorno::ahora_utc_ms();
            let nueva = Repaso::calificar(&x.leccion, nota, ahora);
            match almacen::guardar(&self.raiz(), &nueva, &Donde::de(&x), &self.pedido.aparato) {
                Ok(_) => {
                    if let Some(e) = self.todas.iter_mut().find(|e| e.leccion.id == id) {
                        e.leccion = nueva;
                    }
                    self.cambios = almacen::cambios();
                    self.firma = almacen::firma(&self.raiz(), &self.todas);
                }
                Err(err) => {
                    tracing::warn!(?err, "no se pudo guardar el repaso");
                    self.avisar(self.t("lec2-no-se-pudo"));
                }
            }
        }
        if let Some(r) = self.repaso.as_mut() {
            r.pasar(true);
        }
        self.foco = Some(Foco::Respuesta);
    }

    fn salir_del_repaso(&mut self) {
        self.repaso = None;
        self.foco = None;
        let lecciones: Vec<Leccion> = self.todas.iter().map(|e| e.leccion.clone()).collect();
        self.hoy = Repaso::de_hoy(&lecciones, pixpin_shell::entorno::ahora_utc_ms(), 20);
    }
}

/// Lo que hace una tecla en esta ventana, sin mirar la pantalla: se prueba
/// suelto. `None` = la tecla es de la caja de texto con el foco (o de nadie).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Atajo {
    Accion(Accion),
    /// Esc: cierra lo de encima (menu, edicion, busqueda) o la ventana.
    Atras,
    Arriba,
    Abajo,
    Pegar,
    /// Enter en un texto de la ficha: lo deja guardado.
    Terminar,
    /// Tab: al siguiente bloque de la ficha.
    SiguienteBloque(bool),
}

/// **Los atajos**, en un sitio. Las teclas de una letra (R, P, S, 1/2/3,
/// Espacio) solo valen sin una caja de texto con el foco: escribiendo son
/// letras.
fn atajo(
    foco: Option<Foco>,
    en_repaso: bool,
    mostrada: bool,
    vk: u32,
    ctrl: bool,
    shift: bool,
) -> Option<Atajo> {
    use Accion as A;
    if vk == VK_ESCAPE {
        return Some(Atajo::Atras);
    }
    if en_repaso {
        return match (vk, ctrl, foco) {
            (VK_D, true, _) => Some(Atajo::Accion(A::DictarRespuesta)),
            (VK_ENTRAR, _, Some(Foco::Respuesta)) if !shift && !mostrada => {
                Some(Atajo::Accion(A::Mostrar))
            }
            (VK_ENTRAR, _, Some(Foco::Respuesta)) if !shift => Some(Atajo::Accion(A::Siguiente)),
            (_, _, Some(_)) => None,
            (VK_ESPACIO, false, None) if !mostrada => Some(Atajo::Accion(A::Mostrar)),
            (VK_ENTRAR, _, None) => Some(Atajo::Accion(A::Siguiente)),
            (VK_1, false, None) => Some(Atajo::Accion(A::Nota(0))),
            (VK_2, false, None) => Some(Atajo::Accion(A::Nota(1))),
            (VK_3, false, None) => Some(Atajo::Accion(A::Nota(2))),
            (VK_S, false, None) => Some(Atajo::Accion(A::Saltar)),
            _ => None,
        };
    }
    match (vk, ctrl, shift, foco) {
        (VK_F, true, _, _) => Some(Atajo::Accion(A::FocoBuscar)),
        (VK_N, true, _, _) => Some(Atajo::Accion(A::FocoRapida)),
        (VK_D, true, true, _) => Some(Atajo::Accion(A::DictarRapida)),
        (VK_Z, true, _, None | Some(Foco::Rapida) | Some(Foco::Buscar)) => {
            Some(Atajo::Accion(A::Deshacer))
        }
        (VK_V, true, _, None | Some(Foco::Rapida)) => Some(Atajo::Pegar),
        (VK_ENTRAR, _, false, Some(Foco::Rapida)) => Some(Atajo::Accion(A::Guardar)),
        (VK_ENTRAR, _, false, Some(f)) if f.es_de_la_ficha() => Some(Atajo::Terminar),
        (VK_TAB, _, s, Some(f)) if f.es_de_la_ficha() => Some(Atajo::SiguienteBloque(s)),
        (VK_ARRIBA, false, _, None | Some(Foco::Buscar)) => Some(Atajo::Arriba),
        (VK_ABAJO, false, _, None | Some(Foco::Buscar)) => Some(Atajo::Abajo),
        (_, _, _, Some(_)) => None,
        (VK_R, false, false, None) => Some(Atajo::Accion(A::Repasar)),
        (VK_P, false, false, None) => Some(Atajo::Accion(A::Pinear)),
        (VK_SUPRIMIR, false, false, None) => Some(Atajo::Accion(A::Borrar)),
        _ => None,
    }
}

/// La gravedad que sigue al pulsar su punto: Leve → Importante → Grave →
/// Leve. Un valor raro (de otro aparato) cuenta como el extremo mas cercano.
fn siguiente_gravedad(g: i64) -> i64 {
    g.clamp(1, 3) % 3 + 1
}

/// Las dos columnas: lista y ficha, bajo la cabecera. La lista se encoge en
/// pantallas estrechas; la ficha se queda con todo lo demas (antes lo
/// partia con la columna de datos, que se quito al simplificar).
fn columnas(w: f32, h: f32, arriba: f32, s: f32) -> (RectF, RectF) {
    let lista = (w * 0.31).clamp(300.0 * s, 390.0 * s).min(w.max(0.0));
    let alto = (h - arriba).max(0.0);
    let ficha = (w - lista).max(0.0);
    (
        RectF {
            x: 0.0,
            y: arriba,
            ancho: lista,
            alto,
        },
        RectF {
            x: lista,
            y: arriba,
            ancho: ficha,
            alto,
        },
    )
}

/// Alto de lo que se pulsa en la ficha y en la lista (regla del rediseno:
/// objetivos de 40 px como poco).
const OBJETIVO: f32 = 40.0;

/// **La fila de datos bajo el titulo**: donde va cada pieza. Pintar y
/// acertar el clic salen de aqui (pintar.rs apunta la zona con estos mismos
/// rectangulos), para que lo que se ve sea lo que responde.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FilaDatos {
    /// El punto de la gravedad con su nombre (clic = la siguiente).
    gravedad: RectF,
    /// «3×»: solo si paso mas de una vez (un «1×» no dice nada).
    veces: Option<RectF>,
    /// «Me volvio a pasar +1».
    otra_vez: RectF,
    /// «Mas campos…» / «Menos campos», a la derecha; si no cabe, en un
    /// segundo renglon.
    mas: RectF,
    /// Lo que mide la fila entera.
    alto: f32,
}

/// Coloca la fila de datos desde `(x, y)` en `ancho`, con los anchos ya
/// medidos de cada pieza.
#[allow(clippy::too_many_arguments)] // sitio, ancho, cuatro anchos medidos y escala
fn fila_de_datos(
    x: f32,
    y: f32,
    ancho: f32,
    w_gravedad: f32,
    w_veces: Option<f32>,
    w_otra: f32,
    w_mas: f32,
    s: f32,
) -> FilaDatos {
    let alto = OBJETIVO * s;
    let hueco = 8.0 * s;
    let gravedad = RectF {
        x,
        y,
        ancho: w_gravedad,
        alto,
    };
    let mut fin = gravedad.x + gravedad.ancho;
    let veces = w_veces.map(|w| {
        let r = RectF {
            x: fin + hueco,
            y,
            ancho: w,
            alto,
        };
        fin = r.x + r.ancho;
        r
    });
    let otra_vez = RectF {
        x: fin + hueco,
        y,
        ancho: w_otra,
        alto,
    };
    fin = otra_vez.x + otra_vez.ancho;
    let derecha = x + ancho - w_mas;
    let (mas, total) = if derecha >= fin + 2.0 * hueco {
        (
            RectF {
                x: derecha,
                y,
                ancho: w_mas,
                alto,
            },
            alto,
        )
    } else {
        (
            RectF {
                x,
                y: y + alto + hueco,
                ancho: w_mas,
                alto,
            },
            2.0 * alto + hueco,
        )
    };
    FilaDatos {
        gravedad,
        veces,
        otra_vez,
        mas,
        alto: total,
    }
}

/// **«Para repasar hoy · N» en una linea**: la tarjeta y su boton «Repasar
/// R» a la derecha, centrado en alto. Como la fila de datos, pintar y el
/// clic usan estos mismos rectangulos.
fn tarjeta_de_repaso(x: f32, y: f32, ancho: f32, w_boton: f32, s: f32) -> (RectF, RectF) {
    let margen = 6.0 * s;
    let caja = RectF {
        x,
        y,
        ancho,
        alto: OBJETIVO * s + 2.0 * margen,
    };
    let w = w_boton.min(ancho - 2.0 * margen).max(0.0);
    let boton = RectF {
        x: x + ancho - w - margen,
        y: y + margen,
        ancho: w,
        alto: OBJETIVO * s,
    };
    (caja, boton)
}

/// Una pieza de la fila de etiquetas de «Mas campos…».
#[derive(Debug, Clone, PartialEq)]
enum PiezaEtiqueta {
    /// Una etiqueta con su «×» (el indice es el de `todas_las_etiquetas`).
    Puesta(usize),
    /// La caja donde se esta escribiendo una nueva.
    Escribiendo,
    /// «+ Anadir».
    Anadir,
}

/// Lo que se le pide a [`disponer_mas_campos`]: los textos ya resueltos.
struct PedidoMasCampos<'a> {
    /// «Area», «Proyecto», «Etiquetas».
    rotulos: [&'a str; 3],
    area: &'a str,
    /// Los rotulos de las etiquetas (con ✨ las automaticas).
    etiquetas: &'a [String],
    escribiendo: bool,
    anadir: &'a str,
    relacionadas: usize,
    completa: &'a str,
}

/// **Donde va cada cosa de «Mas campos…»**. Como [`fila_de_datos`], el
/// pintar y las zonas del clic salen de aqui. `medir` da el ancho de un
/// texto a 13 px (se pasa para poder probarlo sin pantalla).
#[derive(Debug, Clone, PartialEq)]
struct MasCampos {
    /// El fondo del desplegable entero.
    caja: RectF,
    x_rotulo: f32,
    /// La y de cada fila con rotulo: area, proyecto, etiquetas.
    filas: [f32; 3],
    x_valor: f32,
    ancho_valor: f32,
    area: RectF,
    etiquetas: Vec<(RectF, PiezaEtiqueta)>,
    /// La y del rotulo «Relacionadas», si hay alguna.
    rotulo_relacionadas: Option<f32>,
    relacionadas: Vec<RectF>,
    /// La fila del proximo repaso; a su derecha, `completa`.
    repaso: f32,
    /// «Tipo, causas y palabras…»: la ficha de siempre en su ventana.
    completa: RectF,
}

fn disponer_mas_campos(
    q: &PedidoMasCampos,
    medir: impl Fn(&str) -> f32,
    x: f32,
    y0: f32,
    ancho: f32,
    s: f32,
) -> MasCampos {
    let obj = OBJETIVO * s;
    let pad = 16.0 * s;
    let hueco = 6.0 * s;
    let col = q.rotulos.iter().map(|t| medir(t)).fold(0.0, f32::max) + 20.0 * s;
    let x_rotulo = x + pad;
    let x_valor = x_rotulo + col;
    let ancho_valor = (x + ancho - pad - x_valor).max(80.0 * s);
    let fin = x_valor + ancho_valor;
    let mut y = y0 + 8.0 * s;
    let area = RectF {
        x: x_valor,
        y,
        ancho: (medir(q.area) + 66.0 * s).min(ancho_valor),
        alto: obj,
    };
    let fila_area = y;
    y += obj + 4.0 * s;
    let fila_proyecto = y;
    y += obj + 4.0 * s;
    let fila_etiquetas = y;
    // Las etiquetas en renglones que se parten al llegar al borde.
    let mut piezas: Vec<(f32, PiezaEtiqueta)> = q
        .etiquetas
        .iter()
        .enumerate()
        .map(|(i, t)| (medir(t) + 36.0 * s, PiezaEtiqueta::Puesta(i)))
        .collect();
    piezas.push(if q.escribiendo {
        (140.0 * s, PiezaEtiqueta::Escribiendo)
    } else {
        (medir(q.anadir) + 20.0 * s, PiezaEtiqueta::Anadir)
    });
    let mut etiquetas = Vec::with_capacity(piezas.len());
    let mut ex = x_valor;
    for (w, pieza) in piezas {
        let w = w.min(ancho_valor);
        if ex > x_valor && ex + w > fin {
            ex = x_valor;
            y += obj + hueco;
        }
        etiquetas.push((
            RectF {
                x: ex,
                y,
                ancho: w,
                alto: obj,
            },
            pieza,
        ));
        ex += w + hueco;
    }
    y += obj + 8.0 * s;
    let (rotulo_relacionadas, relacionadas) = if q.relacionadas > 0 {
        let r = y;
        y += 28.0 * s;
        let v = (0..q.relacionadas)
            .map(|_| {
                let c = RectF {
                    x: x_rotulo,
                    y,
                    ancho: ancho - 2.0 * pad,
                    alto: obj + 4.0 * s,
                };
                y += c.alto + hueco;
                c
            })
            .collect();
        (Some(r), v)
    } else {
        (None, Vec::new())
    };
    let repaso = y;
    let cw = medir(q.completa) + 20.0 * s;
    let completa = RectF {
        x: x + ancho - pad - cw + 10.0 * s,
        y,
        ancho: cw,
        alto: obj,
    };
    y += obj + 4.0 * s;
    MasCampos {
        caja: RectF {
            x,
            y: y0,
            ancho,
            alto: y - y0,
        },
        x_rotulo,
        filas: [fila_area, fila_proyecto, fila_etiquetas],
        x_valor,
        ancho_valor,
        area,
        etiquetas,
        rotulo_relacionadas,
        relacionadas,
        repaso,
        completa,
    }
}

/// Lo que queda de `r` dentro de `vista` (una fila medio tapada por el
/// buscador solo responde en lo que se ve), o nada si queda fuera.
fn recortar(r: RectF, vista: RectF) -> Option<RectF> {
    let x0 = r.x.max(vista.x);
    let y0 = r.y.max(vista.y);
    let x1 = (r.x + r.ancho).min(vista.x + vista.ancho);
    let y1 = (r.y + r.alto).min(vista.y + vista.alto);
    (x1 > x0 && y1 > y0).then_some(RectF {
        x: x0,
        y: y0,
        ancho: x1 - x0,
        alto: y1 - y0,
    })
}

fn bucle(recursos: &Recursos, pedido: Pedido) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = pixpin_shell::pantalla_de::monitor_de_la_ventana_activa()
        .and_then(|r| monitores.monitores().iter().find(|m| m.area == r))
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let mut marco = centrado(monitor.area_trabajo, 1280, 820, monitor.escala_por_cien);
    let mut e = Estado::nuevo(pedido);
    let mut ventana = VentanaOverlay::nueva_normal(marco, &e.t("chat-lecciones"))
        .context("no se pudo abrir la lista de lecciones")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para las lecciones")?;
    ventana.mostrar();
    ventana.enfocar();
    let hwnd = ventana.handle().0 as isize;
    ABIERTA.store(hwnd, Ordering::SeqCst);
    almacen::apuntar_ventana(hwnd);
    e.motor = Some(motor.clone());
    e.d3d = Some(recursos.d3d());
    e.hwnd = hwnd;
    e.recargar();
    if e.pedido.seleccion.is_some() {
        e.foco = None;
    }
    tracing::info!(lecciones = e.todas.len(), "lista de lecciones abierta");
    let mut mirado = Instant::now();
    let mut vivo = true;
    let mut pintar = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        for (h, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if h != ventana.handle() {
                continue;
            }
            pintar = true;
            let local = |p: pixpin_geom::Punto, m: Rect| ((p.x - m.x) as f32, (p.y - m.y) as f32);
            match ev {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::RatonMovido(p) => {
                    if let Some((desde, origen)) = e.moviendo {
                        marco = Rect {
                            x: origen.x + (p.x - desde.x),
                            y: origen.y + (p.y - desde.y),
                            ..origen
                        };
                        ventana.mover(marco);
                    }
                    e.botones.raton = local(p, marco);
                }
                EventoOverlay::BotonPulsado(p) => {
                    e.botones.raton = local(p, marco);
                    match e.botones.bajo_el_raton() {
                        Some(Accion::Mover) => {
                            e.moviendo = Some((p, marco));
                            ventana.capturar_raton();
                        }
                        Some(a) => vivo = hacer(&mut e, a),
                        None => {}
                    }
                }
                EventoOverlay::BotonSoltado(_) => {
                    if e.moviendo.take().is_some() {
                        ventana.soltar_raton();
                    }
                }
                EventoOverlay::Rueda(m) => {
                    // 120 por muesca: unos 100 px cada una.
                    let d = m as f32 / 120.0 * 100.0 * escala;
                    if crate::ventanita::dentro(e.zona_lista, e.botones.raton) {
                        e.scroll_lista -= d;
                    } else {
                        e.scroll_ficha -= d;
                    }
                }
                EventoOverlay::Caracter(c) => escribir(&mut e, c),
                EventoOverlay::Tecla {
                    vk, ctrl, shift, ..
                } => vivo = tecla(&mut e, vk, ctrl, shift),
                _ => {}
            }
        }
        if !vivo {
            break;
        }
        if let Some(p) = PENDIENTE.lock().unwrap_or_else(|x| x.into_inner()).take() {
            e.aplicar(p);
            pintar = true;
        }
        // Lo guardado en otra ventana, al momento; lo que llega sincronizando,
        // mirando las fechas cada dos segundos. Mientras se edita no se
        // recarga: se perderia lo escrito.
        let editando = e.foco.is_some_and(Foco::es_de_la_ficha);
        if !editando && almacen::cambios() != e.cambios {
            e.recargar();
            pintar = true;
        } else if !editando && mirado.elapsed() >= Duration::from_secs(2) {
            mirado = Instant::now();
            if almacen::firma(e.pedido.ubicacion.raiz(), &e.todas) != e.firma {
                e.recargar();
                pintar = true;
            }
        }
        match e
            .dictado
            .avanzar(&e.pedido.ubicacion.clone(), e.pedido.idioma)
        {
            Salida::Nada => {}
            Salida::Texto(t) => {
                let campo = match e.dictando {
                    Foco::Respuesta => e.repaso.as_mut().map(|r| &mut r.respuesta),
                    Foco::Buscar => Some(&mut e.consulta),
                    _ => Some(&mut e.rapida.campo),
                };
                if let Some(c) = campo {
                    if !c.texto.trim().is_empty() && !c.texto.ends_with(' ') {
                        c.escribir(" ");
                    }
                    c.escribir(&t);
                }
                e.recalcular_rapida();
                e.filtrar();
                pintar = true;
            }
            Salida::Fallo(m) => {
                e.avisar(m);
                pintar = true;
            }
        }
        if e.dictado.activo() {
            pintar = true;
        }
        if e.borrando
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed() > PARA_DESHACER)
        {
            e.borrar_ya();
            pintar = true;
        }
        if e.aviso
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed() > Duration::from_millis(3_000))
        {
            e.aviso = None;
            pintar = true;
        }
        if pintar {
            e.ahora = pixpin_shell::entorno::ahora_utc_ms();
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| pintar::todo(&mut e, p, marco, escala));
                let _ = superficie.presentar();
            }
            pintar = false;
        }
        let espera = if e.dictado.activo() {
            120
        } else if e.aviso.is_some() || e.borrando.is_some() {
            250
        } else {
            1_000
        };
        pixpin_shell::overlay::esperar_eventos(Some(espera));
    }
    e.terminar_edicion();
    e.borrar_ya();
    almacen::quitar_ventana(hwnd);
    tracing::info!("lista de lecciones cerrada");
    Ok(())
}

/// Una letra escrita: a la caja con el foco.
fn escribir(e: &mut Estado, c: char) {
    match e.foco {
        Some(Foco::Rapida) => {
            if e.rapida.campo.letra(c) {
                e.recalcular_rapida();
            }
        }
        Some(Foco::Buscar) => {
            if e.consulta.letra(c) {
                e.filtrar();
            }
        }
        Some(Foco::Respuesta) => {
            if let Some(r) = e.repaso.as_mut() {
                r.respuesta.letra(c);
            }
        }
        Some(Foco::Etiqueta) if c == ' ' || c == ',' => {
            e.terminar_edicion();
            e.foco = Some(Foco::Etiqueta);
        }
        Some(f) if f.es_de_la_ficha() => {
            e.editando.letra(c);
        }
        _ => {}
    }
}

/// Una tecla. Devuelve si la ventana sigue.
fn tecla(e: &mut Estado, vk: u32, ctrl: bool, shift: bool) -> bool {
    let mostrada = e.repaso.as_ref().is_some_and(|r| r.mostrada);
    match atajo(e.foco, e.repaso.is_some(), mostrada, vk, ctrl, shift) {
        Some(Atajo::Accion(a)) => return hacer(e, a),
        Some(Atajo::Atras) => return atras(e),
        Some(Atajo::Arriba) => e.mover_sel(-1),
        Some(Atajo::Abajo) => e.mover_sel(1),
        Some(Atajo::Pegar) => {
            let contenido = pixpin_codec::portapapeles::leer();
            e.pegar_en_la_barra(contenido);
            e.recalcular_rapida();
        }
        Some(Atajo::Terminar) => {
            let era = e.foco;
            e.terminar_edicion();
            if era == Some(Foco::Etiqueta) {
                e.foco = Some(Foco::Etiqueta);
            }
        }
        Some(Atajo::SiguienteBloque(atras)) => {
            const ORDEN: [Foco; 4] = [Foco::Titulo, Foco::Paso, Foco::PorQue, Foco::Proxima];
            let i = ORDEN.iter().position(|f| Some(*f) == e.foco).unwrap_or(0);
            let j = if atras {
                (i + ORDEN.len() - 1) % ORDEN.len()
            } else {
                (i + 1) % ORDEN.len()
            };
            e.editar(ORDEN[j]);
        }
        None => match e.foco {
            Some(Foco::Rapida) => {
                if e.rapida.campo.tecla(vk, ctrl, shift, false) {
                    e.recalcular_rapida();
                }
            }
            Some(Foco::Buscar) => {
                if e.consulta.tecla(vk, ctrl, shift, false) {
                    e.filtrar();
                }
            }
            Some(Foco::Respuesta) => {
                if let Some(r) = e.repaso.as_mut() {
                    r.respuesta.tecla(vk, ctrl, shift, true);
                }
            }
            Some(f) if f.es_de_la_ficha() => {
                e.editando.tecla(vk, ctrl, shift, f.multilinea());
            }
            _ => {}
        },
    }
    true
}

/// Esc: lo de encima primero. Devuelve si la ventana sigue.
fn atras(e: &mut Estado) -> bool {
    if e.menu.take().is_some() {
        return true;
    }
    if e.repaso.is_some() {
        if e.foco == Some(Foco::Respuesta)
            && e.repaso
                .as_ref()
                .is_some_and(|r| !r.respuesta.texto.is_empty())
        {
            e.foco = None;
        } else {
            e.salir_del_repaso();
        }
        return true;
    }
    match e.foco {
        Some(f) if f.es_de_la_ficha() => e.terminar_edicion(),
        Some(Foco::Buscar) if !e.consulta.texto.is_empty() => {
            e.consulta.poner("");
            e.filtrar();
        }
        Some(Foco::Rapida) if !e.rapida.campo.texto.is_empty() || !e.rapida.fotos.is_empty() => {
            e.foco = None;
        }
        Some(_) => e.foco = None,
        None if e.comprobacion => e.comprobacion = false,
        None => return false,
    }
    true
}

/// Lo que hace un clic (o su atajo). Devuelve si la ventana sigue.
fn hacer(e: &mut Estado, a: Accion) -> bool {
    use pixpin_lecciones::Nota;
    // Un clic fuera de lo que se edita lo deja guardado; uno fuera del menu
    // lo cierra.
    let sigue_en_la_ficha = matches!(a, Accion::Editar(_));
    if !sigue_en_la_ficha && e.foco.is_some_and(Foco::es_de_la_ficha) {
        e.terminar_edicion();
    }
    if !matches!(a, Accion::Opcion(_) | Accion::AbrirMenu(_)) {
        e.menu = None;
    }
    match a {
        Accion::Mover => {}
        Accion::Cerrar => return false,
        Accion::Fondo => {
            if e.repaso.is_none() {
                e.foco = None;
            }
        }
        Accion::FocoRapida => {
            e.repaso = None;
            e.foco = Some(Foco::Rapida);
        }
        Accion::DictarRapida | Accion::DictarRespuesta => {
            e.dictando = if a == Accion::DictarRespuesta {
                Foco::Respuesta
            } else {
                Foco::Rapida
            };
            e.foco = Some(e.dictando);
            let (u, i) = (e.pedido.ubicacion.clone(), e.pedido.idioma);
            if let Some(m) = e.dictado.pulsar(&u, i) {
                e.avisar(m);
            }
        }
        Accion::ImagenRapida => {
            let rutas = pixpin_shell::elegir::pedir_imagenes(windows::Win32::Foundation::HWND(
                e.hwnd as *mut _,
            ));
            e.anadir_fotos_de(&rutas);
            e.foco = Some(Foco::Rapida);
        }
        Accion::QuitarFoto(i) => {
            if i < e.rapida.fotos.len() {
                e.rapida.fotos.remove(i);
            }
        }
        Accion::Guardar => e.guardar_rapida(),
        Accion::AbrirMenu(m) => e.menu = if e.menu == Some(m) { None } else { Some(m) },
        Accion::CicloGravedad => {
            e.rapida.gravedad = Some(siguiente_gravedad(e.gravedad_rapida()));
        }
        Accion::YaLaTengo => {
            if let Some(l) = e.rapida.parecida.clone()
                && let Some(x) = e.todas.iter().find(|x| x.leccion.id == l.id).cloned()
            {
                let ahora = pixpin_shell::entorno::ahora_utc_ms();
                match almacen::guardar(
                    &e.raiz(),
                    &Repaso::repetida(&x.leccion, ahora),
                    &Donde::de(&x),
                    &e.pedido.aparato,
                ) {
                    Ok(_) => {
                        e.rapida = Rapida::default();
                        e.sel = Some(l.id.clone());
                        e.seguir_sel = true;
                        e.avisar(e.t("lec2-apuntado-otra-vez"));
                        e.recargar();
                    }
                    Err(err) => {
                        tracing::warn!(?err, "no se pudo apuntar que volvio a pasar");
                        e.avisar(e.t("lec2-no-se-pudo"));
                    }
                }
            }
        }
        Accion::Opcion(i) => e.elegir_opcion(i),
        Accion::FocoBuscar => {
            e.repaso = None;
            e.foco = Some(Foco::Buscar);
        }
        Accion::BorrarConsulta => {
            e.consulta.poner("");
            e.filtrar();
        }
        Accion::Filtro(i) => {
            if let Some((f, _)) = e.filtros().get(i).copied() {
                e.filtro = if e.filtro == f { Filtro::Todas } else { f };
                e.filtrar();
            }
        }
        Accion::QuitarProyecto => {
            e.proyecto = None;
            e.filtrar();
        }
        Accion::Comprobacion => e.comprobacion = !e.comprobacion,
        Accion::Marcar(i) => {
            if let Some(x) = e.todas.get(i) {
                let id = x.leccion.id.clone();
                if !e.marcadas.remove(&id) {
                    e.marcadas.insert(id);
                }
            }
        }
        Accion::Repasar => e.empezar_repaso(),
        Accion::Fila(i) => {
            if let Some(x) = e.todas.get(i) {
                if e.sel.as_deref() != Some(x.leccion.id.as_str()) {
                    e.scroll_ficha = 0.0;
                }
                e.sel = Some(x.leccion.id.clone());
                e.comprobacion = false;
            }
            e.foco = None;
        }
        Accion::Editar(f) => e.editar(f),
        Accion::MasCampos => e.mas_campos = !e.mas_campos,
        Accion::FichaCompleta => {
            if let Some(x) = e.elegida() {
                ficha::abrir(e.ficha(ficha::Que::Editar {
                    id: x.leccion.id.clone(),
                }));
            }
        }
        Accion::Adjunto(i) => {
            let raiz = e.raiz();
            if let Some(x) = e.elegida().cloned()
                && let Some(a) = e.adjuntos.de(&raiz, &x, e.motor.as_deref()).get(i).cloned()
            {
                match a.clase {
                    adjuntos::Clase::Foto => {
                        if !pixpin_shell::mensajero::enviar_ficheros(std::slice::from_ref(&a.ruta))
                        {
                            e.avisar(e.t("lec2-no-se-pudo"));
                        } else {
                            e.avisar(e.t("lec2-pineado"));
                        }
                    }
                    adjuntos::Clase::Voz => {
                        crate::ventana_chat::reproducir_flotante(
                            &e.textos,
                            a.ruta.clone(),
                            x.leccion.titulo.clone(),
                        );
                    }
                }
            }
        }
        Accion::AnadirAdjunto => {
            let rutas = pixpin_shell::elegir::pedir_imagenes(windows::Win32::Foundation::HWND(
                e.hwnd as *mut _,
            ));
            let fotos: Vec<almacen::Foto> = rutas
                .iter()
                .filter(|r| pixpin_codec::cargar(r).is_ok())
                .filter_map(|r| {
                    let nombre = r.file_name()?.to_string_lossy().into_owned();
                    Some((nombre, std::fs::read(r).ok()?))
                })
                .collect();
            if fotos.len() < rutas.len() {
                e.avisar(e.t("lec2-solo-imagenes"));
            }
            e.anadir_a_la_elegida(fotos);
        }
        Accion::VerEnChat => {
            if let Some(x) = e.elegida() {
                // El chat ya abierto solo cambia de proyecto; si estaba
                // cerrado, se abre con las opciones de lienzo de siempre.
                let opciones = crate::ventana_chat::OpcionesLienzo {
                    enganche: Default::default(),
                    nivel: pixpin_nivel::Nivel::Ligero,
                    medir_fotogramas: false,
                };
                crate::ventana_chat::ir_a(
                    e.pedido.idioma,
                    e.pedido.ubicacion.clone(),
                    opciones,
                    x.ficha.clone(),
                    None,
                );
            }
        }
        Accion::Pinear => e.pinear(),
        Accion::Borrar => e.borrar_elegida(),
        Accion::Deshacer => e.deshacer(),
        Accion::OtraVez => {
            if let Some(l) = e.elegida().map(|x| x.leccion.clone()) {
                let ahora = pixpin_shell::entorno::ahora_utc_ms();
                e.guardar_elegida(Repaso::repetida(&l, ahora));
                e.avisar(e.t("lec2-apuntado-otra-vez"));
            }
        }
        Accion::OtraGravedad => {
            if let Some(mut l) = e.elegida().map(|x| x.leccion.clone()) {
                l.gravedad = siguiente_gravedad(l.gravedad);
                e.guardar_elegida(l);
            }
        }
        Accion::QuitarEtiqueta(i) => {
            if let Some(mut l) = e.elegida().map(|x| x.leccion.clone())
                && let Some(t) = l.todas_las_etiquetas().get(i).cloned()
            {
                if l.etiquetas.contains(&t) {
                    l.etiquetas.retain(|x| *x != t);
                } else {
                    // Una automatica quitada no vuelve (`quitadas`).
                    l.etiquetas_auto.retain(|x| *x != t);
                    if !l.quitadas.contains(&t) {
                        l.quitadas.push(t);
                    }
                }
                e.guardar_elegida(l);
            }
        }
        Accion::Relacionada(i) => {
            if let Some(l) = e.relacionadas().get(i) {
                e.sel = Some(l.id.clone());
                e.seguir_sel = true;
                e.scroll_ficha = 0.0;
                if !e.visibles.iter().any(|i| e.todas[*i].leccion.id == l.id) {
                    e.filtro = Filtro::Todas;
                    e.consulta.poner("");
                    e.proyecto = None;
                    let id = l.id.clone();
                    e.filtrar();
                    e.sel = Some(id);
                }
            }
        }
        Accion::Mostrar => {
            if let Some(r) = e.repaso.as_mut() {
                r.mostrada = true;
            }
            e.foco = None;
        }
        Accion::Nota(i) => {
            if let Some(r) = e.repaso.as_mut()
                && r.actual().is_some()
            {
                if !r.mostrada {
                    r.mostrada = true;
                }
                r.nota = Nota::TODAS.get(i).copied();
                e.foco = None;
            }
        }
        Accion::Siguiente => match e
            .repaso
            .as_ref()
            .map(|r| (r.actual().is_some(), r.mostrada, r.nota))
        {
            Some((false, _, _)) => e.salir_del_repaso(),
            Some((true, false, _)) => {
                if let Some(r) = e.repaso.as_mut() {
                    r.mostrada = true;
                }
                e.foco = None;
            }
            Some((true, true, None)) => e.avisar(e.t("lec2-repaso-elige")),
            Some((true, true, Some(n))) => e.calificar(n),
            None => {}
        },
        Accion::Saltar => {
            if let Some(r) = e.repaso.as_mut() {
                r.pasar(false);
            }
            e.foco = Some(Foco::Respuesta);
        }
        Accion::SalirRepaso => e.salir_del_repaso(),
        Accion::AbrirCompleta => {
            let id = e
                .repaso
                .as_ref()
                .and_then(|r| r.actual().map(str::to_string));
            e.salir_del_repaso();
            if let Some(id) = id {
                e.filtro = Filtro::Todas;
                e.consulta.poner("");
                e.proyecto = None;
                e.mirado = None;
                e.filtrar();
                e.sel = Some(id);
                e.seguir_sel = true;
            }
        }
        Accion::FocoRespuesta => e.foco = Some(Foco::Respuesta),
    }
    true
}

/// Un texto del catalogo con un argumento.
fn t1(
    tx: &Catalogo,
    clave: &str,
    k: &str,
    v: impl Into<fluent_bundle::FluentValue<'static>>,
) -> String {
    let mut a = fluent_bundle::FluentArgs::new();
    a.set(k.to_string(), v.into());
    tx.t_args(clave, &a)
}

/// Un texto del catalogo con dos argumentos.
fn t2(
    tx: &Catalogo,
    clave: &str,
    k1: &str,
    v1: impl Into<fluent_bundle::FluentValue<'static>>,
    k2: &str,
    v2: impl Into<fluent_bundle::FluentValue<'static>>,
) -> String {
    let mut a = fluent_bundle::FluentArgs::new();
    a.set(k1.to_string(), v1.into());
    a.set(k2.to_string(), v2.into());
    tx.t_args(clave, &a)
}

#[cfg(test)]
mod pruebas;
