//! **Lo que pasa en el buscador, sin ventana**: las pestanas, que fila esta
//! elegida, que hace cada tecla y que botones lleva la fila elegida.
//!
//! Los resultados los hace el plugin de Flow (`pixpin_lanzador::resultados`):
//! aqui solo se reparten en pestanas y se decide que hacer con ellos. Asi lo
//! que sale al escribir «grie» o «t comprar pan» es lo mismo en Flow (`p`) y
//! en la app.

use pixpin_lanzador::resultados::{Accion, Resultado, glifo, pedido};
use serde_json::{Value, json};

// Teclas virtuales.
pub const VK_TAB: u32 = 0x09;
pub const VK_RETURN: u32 = 0x0D;
pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_PRIOR: u32 = 0x21;
pub const VK_NEXT: u32 = 0x22;
pub const VK_LEFT: u32 = 0x25;
pub const VK_UP: u32 = 0x26;
pub const VK_RIGHT: u32 = 0x27;
pub const VK_DOWN: u32 = 0x28;
pub const VK_DELETE: u32 = 0x2E;
pub const VK_C: u32 = 0x43;
pub const VK_V: u32 = 0x56;
pub const VK_Z: u32 = 0x5A;

/// Cuantas letras se dejan escribir: mas es pegar un parrafo sin querer.
pub const LARGO_MAXIMO: usize = 300;

/// Las pestanas, en su orden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pestana {
    Todo,
    Archivos,
    Tareas,
    Lecciones,
    Capturas,
    Acciones,
}

impl Pestana {
    pub const TODAS: [Pestana; 6] = [
        Pestana::Todo,
        Pestana::Archivos,
        Pestana::Tareas,
        Pestana::Lecciones,
        Pestana::Capturas,
        Pestana::Acciones,
    ];

    pub fn clave(self) -> &'static str {
        match self {
            Pestana::Todo => "buscar-todo-pestana-todo",
            Pestana::Archivos => "buscar-todo-pestana-archivos",
            Pestana::Tareas => "buscar-todo-pestana-tareas",
            Pestana::Lecciones => "buscar-todo-pestana-lecciones",
            Pestana::Capturas => "buscar-todo-pestana-capturas",
            Pestana::Acciones => "buscar-todo-pestana-acciones",
        }
    }

    fn indice(self) -> usize {
        Pestana::TODAS.iter().position(|p| *p == self).unwrap_or(0)
    }

    /// La de al lado (con vuelta: de Acciones se pasa a Todo).
    pub fn siguiente(self, atras: bool) -> Pestana {
        let n = Pestana::TODAS.len();
        let i = self.indice();
        Pestana::TODAS[if atras { (i + n - 1) % n } else { (i + 1) % n }]
    }

    /// Lo que se le pregunta al plugin con la caja vacia y esta pestana
    /// elegida: asi «Tareas» sin escribir nada ensena las listas, como
    /// `p tareas` en Flow. `None`: lo de la caja vacia (los recientes).
    pub fn consulta_vacia(self) -> Option<&'static str> {
        match self {
            Pestana::Todo => None,
            // Sin escribir, el plugin da las funciones y los ultimos
            // proyectos: los proyectos van a Archivos y las funciones a
            // Acciones.
            Pestana::Archivos | Pestana::Acciones => Some(""),
            Pestana::Tareas => Some("tareas"),
            Pestana::Lecciones => Some("lecciones"),
            Pestana::Capturas => Some("capturas"),
        }
    }
}

/// **A que pestana va un resultado**, por su clave estable (`recordKey`, la
/// que pone el plugin a cada cosa). Lo que no tiene clave es una accion
/// («Apuntar tarea: …», «Nueva nota…»).
pub fn pestana_de(r: &Resultado) -> Pestana {
    let clave = r.clave.as_deref().unwrap_or("");
    let tipo = clave.split('/').next().unwrap_or("");
    match tipo {
        "captura" => Pestana::Capturas,
        "leccion" => Pestana::Lecciones,
        "tarea" | "lista" => Pestana::Tareas,
        "fichero" | "hoja" | "mensaje" | "proyecto" => Pestana::Archivos,
        _ => Pestana::Acciones,
    }
}

/// Los grupos de «Todo», en el orden de la maqueta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grupo {
    Mejor,
    TareasYLecciones,
    Archivos,
    Capturas,
    Acciones,
}

impl Grupo {
    pub fn clave(self) -> &'static str {
        match self {
            Grupo::Mejor => "buscar-todo-mejor",
            Grupo::TareasYLecciones => "buscar-todo-grupo-tareas",
            Grupo::Archivos => "buscar-todo-grupo-archivos",
            Grupo::Capturas => "buscar-todo-grupo-capturas",
            Grupo::Acciones => "buscar-todo-grupo-acciones",
        }
    }

    fn de(p: Pestana) -> Grupo {
        match p {
            Pestana::Tareas | Pestana::Lecciones => Grupo::TareasYLecciones,
            Pestana::Archivos => Grupo::Archivos,
            Pestana::Capturas => Grupo::Capturas,
            Pestana::Todo | Pestana::Acciones => Grupo::Acciones,
        }
    }
}

/// Una linea de la lista: una cabecera de grupo o un resultado (su indice).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Linea {
    Cabecera(Grupo),
    Fila(usize),
}

/// Los botones de la fila elegida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boton {
    /// Lo de Intro (azul, siempre el primero).
    Principal,
    /// Ctrl+Intro.
    VerEnChat,
    /// Ctrl+C.
    Copiar,
    /// Flecha derecha: el menu de mas acciones.
    Mas,
}

/// Lo que la ventana tiene que hacer despues de una tecla o un clic.
#[derive(Debug, Clone, PartialEq)]
pub enum Orden {
    /// Nada que hacer (ni repintar).
    Nada,
    Repintar,
    /// Cambio lo escrito o la pestana: hay que volver a preguntar.
    Buscar,
    Cerrar,
    /// Hacer esta accion. `recordar`: es la eleccion del usuario (Intro sobre
    /// un resultado): se apunta en los recientes.
    Hacer {
        accion: Accion,
        recordar: bool,
    },
    /// Ctrl+V: pegar lo del portapapeles (imagen como `[img NN]`, o texto).
    Pegar,
    /// Supr sobre una captura: a la papelera de PixPin, con deshacer.
    BorrarCaptura(String),
    /// Ctrl+Z tras borrar.
    Deshacer,
    /// Alt+Intro: el «Abrir con…» de Windows.
    AbrirCon(String),
}

/// El menu de «mas acciones» (flecha derecha), sobre la vista previa.
#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    pub titulo: String,
    pub opciones: Vec<Resultado>,
    pub elegida: usize,
}

/// El estado del buscador.
#[derive(Debug, Clone)]
pub struct Estado {
    pub consulta: String,
    pub pestana: Pestana,
    /// Lo que salio para la consulta (o, con la caja vacia en «Todo», los
    /// abiertos hace poco).
    pub resultados: Vec<Resultado>,
    /// La fila elegida: posicion en [`Estado::visibles`].
    pub elegido: usize,
    pub menu: Option<Menu>,
    /// Si hay algo que deshacer (una captura borrada).
    pub se_puede_deshacer: bool,
}

impl Default for Estado {
    fn default() -> Self {
        Estado {
            consulta: String::new(),
            pestana: Pestana::Todo,
            resultados: Vec::new(),
            elegido: 0,
            menu: None,
            se_puede_deshacer: false,
        }
    }
}

impl Estado {
    /// La caja esta vacia y en «Todo»: se ensenan recientes y letras.
    pub fn es_inicio(&self) -> bool {
        self.consulta.trim().is_empty() && self.pestana == Pestana::Todo
    }

    /// Cuantos resultados hay en cada pestana (la de «Todo», todos).
    pub fn cuenta(&self, p: Pestana) -> usize {
        match p {
            Pestana::Todo => self.resultados.len(),
            _ => self
                .resultados
                .iter()
                .filter(|r| pestana_de(r) == p)
                .count(),
        }
    }

    /// **Las lineas de la lista.** En «Todo» con algo escrito: el mejor
    /// resultado arriba y el resto por grupos, cada grupo en el orden del
    /// plugin. En otra pestana, solo lo suyo y sin cabeceras.
    pub fn lineas(&self) -> Vec<Linea> {
        if self.pestana != Pestana::Todo {
            return (0..self.resultados.len())
                .filter(|i| pestana_de(&self.resultados[*i]) == self.pestana)
                .map(Linea::Fila)
                .collect();
        }
        if self.resultados.is_empty() {
            return Vec::new();
        }
        if self.es_inicio() {
            // Los abiertos hace poco: una sola lista, sin grupos.
            return (0..self.resultados.len()).map(Linea::Fila).collect();
        }
        let mut v = vec![Linea::Cabecera(Grupo::Mejor), Linea::Fila(0)];
        for g in [
            Grupo::TareasYLecciones,
            Grupo::Archivos,
            Grupo::Capturas,
            Grupo::Acciones,
        ] {
            let del_grupo: Vec<usize> = (1..self.resultados.len())
                .filter(|i| Grupo::de(pestana_de(&self.resultados[*i])) == g)
                .collect();
            if !del_grupo.is_empty() {
                v.push(Linea::Cabecera(g));
                v.extend(del_grupo.into_iter().map(Linea::Fila));
            }
        }
        v
    }

    /// Los resultados que se ven, en el orden en que se ven.
    pub fn visibles(&self) -> Vec<usize> {
        self.lineas()
            .into_iter()
            .filter_map(|l| match l {
                Linea::Fila(i) => Some(i),
                Linea::Cabecera(_) => None,
            })
            .collect()
    }

    /// El resultado elegido.
    pub fn elegido(&self) -> Option<&Resultado> {
        let v = self.visibles();
        v.get(self.elegido.min(v.len().saturating_sub(1)))
            .map(|i| &self.resultados[*i])
    }

    /// Pone resultados nuevos y vuelve a la primera fila.
    pub fn poner_resultados(&mut self, r: Vec<Resultado>) {
        self.resultados = r;
        self.elegido = 0;
        self.menu = None;
    }

    fn mover(&mut self, paso: isize) -> Orden {
        if let Some(m) = &mut self.menu {
            let n = m.opciones.len();
            if n == 0 {
                return Orden::Nada;
            }
            m.elegida = (m.elegida as isize + paso).clamp(0, n as isize - 1) as usize;
            return Orden::Repintar;
        }
        let n = self.visibles().len();
        if n == 0 {
            return Orden::Nada;
        }
        let nuevo = (self.elegido as isize + paso).clamp(0, n as isize - 1) as usize;
        if nuevo == self.elegido {
            return Orden::Nada;
        }
        self.elegido = nuevo;
        Orden::Repintar
    }

    /// Elige otra pestana.
    pub fn ir_a_pestana(&mut self, p: Pestana) -> Orden {
        if p == self.pestana {
            return Orden::Nada;
        }
        self.pestana = p;
        self.elegido = 0;
        self.menu = None;
        // Con algo escrito, los resultados ya estan: solo se filtran. Con la
        // caja vacia, cada pestana pregunta lo suyo.
        if self.consulta.trim().is_empty() {
            Orden::Buscar
        } else {
            Orden::Repintar
        }
    }

    /// Cambia lo escrito entero (una busqueda reciente, una letra rapida,
    /// lo que pide una accion del plugin).
    pub fn poner_consulta(&mut self, c: &str) -> Orden {
        self.consulta = c.chars().take(LARGO_MAXIMO).collect();
        self.menu = None;
        Orden::Buscar
    }

    /// Lo que hace un boton de la fila elegida.
    pub fn boton(&mut self, b: Boton) -> Orden {
        let Some(r) = self.elegido().cloned() else {
            return Orden::Nada;
        };
        match b {
            Boton::Principal => Orden::Hacer {
                accion: r.accion.clone(),
                recordar: true,
            },
            Boton::VerEnChat => match ver_en_chat(&r) {
                Some(a) => Orden::Hacer {
                    accion: a,
                    recordar: true,
                },
                None => Orden::Nada,
            },
            Boton::Copiar => match copiar(&r) {
                Some(a) => Orden::Hacer {
                    accion: a,
                    recordar: false,
                },
                None => Orden::Nada,
            },
            Boton::Mas => Orden::Repintar,
        }
    }

    /// Abre el menu de mas acciones con `opciones` (las de
    /// `pixpin_lanzador::menu::menu` para el elegido).
    pub fn abrir_menu(&mut self, opciones: Vec<Resultado>) -> Orden {
        let Some(r) = self.elegido() else {
            return Orden::Nada;
        };
        if opciones.is_empty() {
            return Orden::Nada;
        }
        self.menu = Some(Menu {
            titulo: r.titulo.clone(),
            opciones,
            elegida: 0,
        });
        Orden::Repintar
    }

    /// **Una tecla.** La flecha derecha la mira la ventana antes (necesita
    /// el menu del plugin): aqui llega solo para cerrar el menu al reves.
    pub fn tecla(&mut self, vk: u32, shift: bool, ctrl: bool, alt: bool) -> Orden {
        match vk {
            VK_ESCAPE => {
                if self.menu.take().is_some() {
                    return Orden::Repintar;
                }
                if !self.consulta.is_empty() {
                    self.consulta.clear();
                    return Orden::Buscar;
                }
                Orden::Cerrar
            }
            VK_UP => self.mover(-1),
            VK_DOWN => self.mover(1),
            VK_PRIOR => self.mover(-6),
            VK_NEXT => self.mover(6),
            VK_LEFT if self.menu.is_some() => {
                self.menu = None;
                Orden::Repintar
            }
            VK_TAB => self.ir_a_pestana(self.pestana.siguiente(shift)),
            VK_RETURN => {
                if let Some(m) = &self.menu {
                    return match m.opciones.get(m.elegida) {
                        Some(o) => Orden::Hacer {
                            accion: o.accion.clone(),
                            recordar: false,
                        },
                        None => Orden::Nada,
                    };
                }
                if alt {
                    return match self.elegido().and_then(|r| r.fichero.clone()) {
                        Some(f) => Orden::AbrirCon(f),
                        None => Orden::Nada,
                    };
                }
                self.boton(if ctrl {
                    Boton::VerEnChat
                } else {
                    Boton::Principal
                })
            }
            VK_DELETE if !ctrl && !alt => match self.elegido().and_then(ruta_de_captura) {
                Some(r) => Orden::BorrarCaptura(r),
                None => Orden::Nada,
            },
            VK_C if ctrl => self.boton(Boton::Copiar),
            VK_V if ctrl => Orden::Pegar,
            VK_Z if ctrl && self.se_puede_deshacer => Orden::Deshacer,
            _ => Orden::Nada,
        }
    }

    /// **Un caracter tecleado** (WM_CHAR, ya compuesto). Borrar llega aqui
    /// tambien; Ctrl+Borrar (`0x7f`) quita la ultima palabra.
    pub fn caracter(&mut self, c: char) -> Orden {
        match c {
            '\u{8}' => {
                if self.consulta.pop().is_none() {
                    return Orden::Nada;
                }
            }
            '\u{7f}' => {
                if self.consulta.is_empty() {
                    return Orden::Nada;
                }
                let sin_blancos = self.consulta.trim_end().len();
                self.consulta.truncate(sin_blancos);
                let corte = self
                    .consulta
                    .rfind(char::is_whitespace)
                    .map_or(0, |i| i + 1);
                self.consulta.truncate(corte);
            }
            // Intro, Tab, Esc y demas de control: ya los atendio la tecla.
            c if c < ' ' => return Orden::Nada,
            c => {
                if self.consulta.chars().count() >= LARGO_MAXIMO {
                    return Orden::Nada;
                }
                self.consulta.push(c);
            }
        }
        self.menu = None;
        Orden::Buscar
    }
}

/// La ruta de la captura, si el resultado es una (para Supr y la vista
/// previa).
pub fn ruta_de_captura(r: &Resultado) -> Option<String> {
    if pestana_de(r) != Pestana::Capturas {
        return None;
    }
    r.contexto
        .as_ref()
        .and_then(|c| c.get("ruta"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| r.fichero.clone())
}

fn campo<'a>(r: &'a Resultado, k: &str) -> Option<&'a Value> {
    r.contexto.as_ref().and_then(|c| c.get(k))
}

/// «Ver en el chat»: el mensaje de un resultado que vive en un chat (un
/// mensaje, un archivo del chat, una tarea: su lista) o el chat de un
/// proyecto. Lo mismo que ofrece el menu del plugin.
pub fn ver_en_chat(r: &Resultado) -> Option<Accion> {
    let proyecto = campo(r, "proyecto").cloned().unwrap_or(Value::Null);
    let tipo = campo(r, "tipo").and_then(Value::as_str).unwrap_or("");
    if tipo == "proyecto" {
        return Some(Accion::Pedido(pedido(
            "abrir",
            json!({ "que": { "tipo": "proyecto", "proyecto": proyecto } }),
        )));
    }
    let codigo = campo(r, "codigo")
        .and_then(Value::as_str)
        .filter(|c| !c.trim().is_empty())?;
    Some(Accion::Pedido(pedido(
        "abrir",
        json!({ "que": { "tipo": "mensaje", "proyecto": proyecto, "codigo": codigo } }),
    )))
}

/// «Copiar»: la imagen de una captura (como imagen, la pega cualquier
/// programa) o el texto del resultado.
pub fn copiar(r: &Resultado) -> Option<Accion> {
    if let Some(ruta) = ruta_de_captura(r) {
        return Some(Accion::Pedido(pedido(
            "copiar_imagen",
            json!({ "ruta": ruta }),
        )));
    }
    r.copiar
        .clone()
        .filter(|t| !t.trim().is_empty())
        .map(Accion::Copiar)
}

/// La clave del rotulo del boton azul (lo de Intro) segun lo que hace.
pub fn rotulo_principal(a: &Accion) -> &'static str {
    match a {
        Accion::Pedido(p) => match p.get("accion").and_then(Value::as_str).unwrap_or("") {
            "pinear" | "pinear_ultima" => "buscar-todo-accion-pinear",
            "anadir_tarea" => "buscar-todo-accion-apuntar",
            "capturar" => "buscar-todo-accion-capturar",
            "nota_nueva" | "lienzo_nuevo" | "leccion_nueva" | "lista_nueva" => {
                "buscar-todo-accion-crear"
            }
            "grabar" => "buscar-todo-accion-grabar",
            "chat" => "buscar-todo-accion-enviar",
            "copiar_imagen" => "buscar-todo-accion-copiar",
            "reproducir" => "buscar-todo-accion-reproducir",
            _ => "buscar-todo-accion-abrir",
        },
        Accion::PedirYSeguir { pedido, .. } => {
            let hecha = pedido.get("hecha").and_then(Value::as_bool);
            match hecha {
                Some(true) => "buscar-todo-accion-hecha",
                Some(false) => "buscar-todo-accion-desmarcar",
                None => "buscar-todo-accion-abrir",
            }
        }
        Accion::Consulta(_) => "buscar-todo-accion-entrar",
        Accion::Copiar(_) => "buscar-todo-accion-copiar",
        Accion::Carpeta { .. } => "buscar-todo-accion-mostrar",
        Accion::Windows(_) => "buscar-todo-accion-abrir",
        Accion::PegarImagen { .. } => "buscar-todo-accion-pegar",
    }
}

/// Los botones de la fila elegida, en orden: el azul primero, «…» al final.
/// `hay_menu`: el plugin tiene mas acciones para ella.
pub fn botones_de(r: &Resultado, hay_menu: bool) -> Vec<Boton> {
    let mut v = vec![Boton::Principal];
    if ver_en_chat(r).is_some() && campo(r, "tipo").and_then(Value::as_str) != Some("proyecto") {
        v.push(Boton::VerEnChat);
    }
    if copiar(r).is_some() {
        v.push(Boton::Copiar);
    }
    if hay_menu {
        v.push(Boton::Mas);
    }
    v
}

/// El glifo guardado (en los recientes) de vuelta a su constante.
pub fn glifo_estatico(g: &str) -> &'static str {
    const TODOS: [&str; 30] = [
        glifo::CHAT,
        glifo::TAREAS,
        glifo::LIENZO,
        glifo::NOTA,
        glifo::GRABAR,
        glifo::PIXPIN,
        glifo::PROYECTO,
        glifo::ARCHIVO,
        glifo::IMAGEN,
        glifo::AUDIO,
        glifo::PENDIENTE,
        glifo::HECHA,
        glifo::ANADIR,
        glifo::COPIAR,
        glifo::CARPETA,
        glifo::AVISO,
        glifo::ABRIR,
        glifo::PIN,
        glifo::WINDOWS,
        glifo::MINIAPP,
        glifo::ADJUNTAR,
        glifo::LECCION,
        glifo::CAPTURA,
        glifo::GALERIA,
        glifo::MOVER,
        glifo::BORRAR,
        glifo::CONSERVAR,
        glifo::VIDEO,
        glifo::VENTANA,
        glifo::PIXPIN,
    ];
    TODOS
        .iter()
        .copied()
        .find(|t| *t == g)
        .unwrap_or(glifo::ABRIR)
}

/// Un resultado hecho a mano (los recientes, las pruebas): el plugin no
/// deja construirlos desde fuera con `Resultado::nuevo`.
pub fn resultado(titulo: &str, subtitulo: &str, glifo: &'static str, accion: Accion) -> Resultado {
    Resultado {
        titulo: titulo.to_string(),
        subtitulo: subtitulo.to_string(),
        glifo,
        accion,
        autocompletar: None,
        copiar: None,
        contexto: None,
        icono: None,
        resaltado: Vec::new(),
        ayuda_titulo: None,
        ayuda_subtitulo: None,
        clave: None,
        vista_previa: None,
        fichero: None,
        progreso: None,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn con_clave(titulo: &str, clave: Option<&str>) -> Resultado {
        let mut r = resultado(titulo, "", glifo::ABRIR, Accion::Consulta(titulo.into()));
        r.clave = clave.map(str::to_string);
        r
    }

    fn captura(nombre: &str) -> Resultado {
        let ruta = format!("C:\\datos\\capturas\\{nombre}");
        let mut r = resultado(
            nombre,
            "Captura",
            glifo::IMAGEN,
            Accion::Pedido(pedido("pinear", json!({ "ruta": ruta }))),
        );
        r.clave = Some(format!("captura/{nombre}"));
        r.contexto = Some(json!({ "tipo": "captura", "ruta": ruta, "conservada": false }));
        r.fichero = Some(ruta);
        r
    }

    fn mezcla() -> Estado {
        let mut e = Estado {
            consulta: "grie".into(),
            ..Default::default()
        };
        e.poner_resultados(vec![
            captura("grieta.png"),
            con_clave("Apuntar «grie»", None),
            con_clave("Informe de grietas.pdf", Some("fichero/C:/x.pdf")),
            con_clave("Revisar la grieta", Some("tarea/p/c/Revisar la grieta")),
            con_clave("Sellar las grietas", Some("leccion/7")),
            captura("grieta2.png"),
        ]);
        e
    }

    #[test]
    fn cada_resultado_va_a_su_pestana_por_su_clave() {
        assert_eq!(pestana_de(&captura("a.png")), Pestana::Capturas);
        assert_eq!(
            pestana_de(&con_clave("x", Some("leccion/1"))),
            Pestana::Lecciones
        );
        assert_eq!(
            pestana_de(&con_clave("x", Some("tarea/p/c/t"))),
            Pestana::Tareas
        );
        assert_eq!(
            pestana_de(&con_clave("x", Some("lista/p/c"))),
            Pestana::Tareas
        );
        for k in ["fichero/x", "hoja/p/d", "mensaje/p/m", "proyecto/p"] {
            assert_eq!(
                pestana_de(&con_clave("x", Some(k))),
                Pestana::Archivos,
                "{k}"
            );
        }
        // Caso negativo: sin clave, o con una de funcion o ventana, es una
        // accion (no se cuela en Archivos).
        assert_eq!(pestana_de(&con_clave("x", None)), Pestana::Acciones);
        assert_eq!(
            pestana_de(&con_clave("x", Some("funcion/chat"))),
            Pestana::Acciones
        );
        assert_eq!(
            pestana_de(&con_clave("x", Some("ventana/galeria"))),
            Pestana::Acciones
        );
        assert_eq!(
            pestana_de(&con_clave("x", Some("capturax/1"))),
            Pestana::Acciones
        );
    }

    #[test]
    fn las_cuentas_de_las_pestanas_suman_todo() {
        let e = mezcla();
        assert_eq!(e.cuenta(Pestana::Todo), 6);
        assert_eq!(e.cuenta(Pestana::Capturas), 2);
        assert_eq!(e.cuenta(Pestana::Archivos), 1);
        assert_eq!(e.cuenta(Pestana::Tareas), 1);
        assert_eq!(e.cuenta(Pestana::Lecciones), 1);
        assert_eq!(e.cuenta(Pestana::Acciones), 1);
        let suma: usize = Pestana::TODAS[1..].iter().map(|p| e.cuenta(*p)).sum();
        assert_eq!(suma, e.cuenta(Pestana::Todo));
    }

    #[test]
    fn en_todo_el_mejor_arriba_y_el_resto_por_grupos() {
        let e = mezcla();
        assert_eq!(
            e.lineas(),
            vec![
                Linea::Cabecera(Grupo::Mejor),
                Linea::Fila(0),
                Linea::Cabecera(Grupo::TareasYLecciones),
                Linea::Fila(3),
                Linea::Fila(4),
                Linea::Cabecera(Grupo::Archivos),
                Linea::Fila(2),
                Linea::Cabecera(Grupo::Capturas),
                Linea::Fila(5),
                Linea::Cabecera(Grupo::Acciones),
                Linea::Fila(1),
            ]
        );
        assert_eq!(e.visibles(), vec![0, 3, 4, 2, 5, 1]);
        // Caso negativo: en otra pestana no hay cabeceras ni lo de otras.
        let mut e = e;
        e.ir_a_pestana(Pestana::Capturas);
        assert_eq!(e.lineas(), vec![Linea::Fila(0), Linea::Fila(5)]);
    }

    #[test]
    fn flechas_mueven_sin_salirse_y_tab_cambia_de_pestana() {
        let mut e = mezcla();
        assert_eq!(
            e.tecla(VK_UP, false, false, false),
            Orden::Nada,
            "arriba del todo no se mueve"
        );
        assert_eq!(e.tecla(VK_DOWN, false, false, false), Orden::Repintar);
        assert_eq!(
            e.elegido().map(|r| r.titulo.as_str()),
            Some("Revisar la grieta")
        );
        for _ in 0..20 {
            e.tecla(VK_DOWN, false, false, false);
        }
        assert_eq!(
            e.elegido().map(|r| r.titulo.as_str()),
            Some("Apuntar «grie»")
        );
        // Tab avanza, Mayus+Tab vuelve, y se da la vuelta.
        assert_eq!(
            e.tecla(VK_TAB, false, false, false),
            Orden::Repintar,
            "con algo escrito solo se filtra"
        );
        assert_eq!(e.pestana, Pestana::Archivos);
        assert_eq!(e.elegido, 0, "al cambiar de pestana se empieza arriba");
        e.tecla(VK_TAB, true, false, false);
        e.tecla(VK_TAB, true, false, false);
        assert_eq!(e.pestana, Pestana::Acciones);
        // Ctrl+Tab tambien (lo que dice la chapita).
        e.tecla(VK_TAB, false, true, false);
        assert_eq!(e.pestana, Pestana::Todo);
    }

    #[test]
    fn con_la_caja_vacia_cada_pestana_vuelve_a_preguntar() {
        let mut e = Estado::default();
        assert!(e.es_inicio());
        assert_eq!(e.tecla(VK_TAB, false, false, false), Orden::Buscar);
        assert_eq!(Pestana::Tareas.consulta_vacia(), Some("tareas"));
        assert_eq!(Pestana::Capturas.consulta_vacia(), Some("capturas"));
        // Caso negativo: «Todo» vacio no pregunta nada al plugin: son los
        // recientes.
        assert_eq!(Pestana::Todo.consulta_vacia(), None);
    }

    #[test]
    fn escape_cierra_el_menu_luego_borra_y_luego_cierra() {
        let mut e = mezcla();
        e.abrir_menu(vec![resultado(
            "Copiar",
            "",
            glifo::COPIAR,
            Accion::Copiar("x".into()),
        )]);
        assert!(e.menu.is_some());
        assert_eq!(e.tecla(VK_ESCAPE, false, false, false), Orden::Repintar);
        assert!(e.menu.is_none());
        assert_eq!(e.tecla(VK_ESCAPE, false, false, false), Orden::Buscar);
        assert!(e.consulta.is_empty());
        assert_eq!(e.tecla(VK_ESCAPE, false, false, false), Orden::Cerrar);
    }

    #[test]
    fn intro_hace_lo_principal_y_ctrl_intro_va_al_chat() {
        let mut e = Estado::default();
        let mut tarea = con_clave("☐ Revisar", Some("tarea/p/LST/Revisar"));
        tarea.contexto = Some(json!({ "tipo": "tarea", "proyecto": "p1", "codigo": "LST" }));
        tarea.copiar = Some("Revisar".into());
        e.poner_resultados(vec![tarea]);
        e.consulta = "rev".into();
        match e.tecla(VK_RETURN, false, false, false) {
            Orden::Hacer {
                accion: Accion::Consulta(c),
                recordar: true,
            } => assert_eq!(c, "☐ Revisar"),
            o => panic!("{o:?}"),
        }
        match e.tecla(VK_RETURN, false, true, false) {
            Orden::Hacer {
                accion: Accion::Pedido(p),
                ..
            } => {
                assert_eq!(p["accion"], "abrir");
                assert_eq!(p["que"]["tipo"], "mensaje");
                assert_eq!(p["que"]["codigo"], "LST");
                assert_eq!(p["que"]["proyecto"], "p1");
            }
            o => panic!("{o:?}"),
        }
        assert_eq!(
            e.tecla(VK_C, false, true, false),
            Orden::Hacer {
                accion: Accion::Copiar("Revisar".into()),
                recordar: false
            }
        );
        // Caso negativo: Alt+Intro sin fichero no hace nada, y Supr en algo
        // que no es una captura tampoco.
        assert_eq!(e.tecla(VK_RETURN, false, false, true), Orden::Nada);
        assert_eq!(e.tecla(VK_DELETE, false, false, false), Orden::Nada);
    }

    #[test]
    fn la_captura_se_pinea_se_copia_como_imagen_y_supr_la_borra() {
        let mut e = Estado {
            consulta: "g".into(),
            ..Estado::default()
        };
        e.poner_resultados(vec![captura("muro.png")]);
        let r = e.elegido().cloned().unwrap();
        assert_eq!(rotulo_principal(&r.accion), "buscar-todo-accion-pinear");
        assert_eq!(
            botones_de(&r, true),
            vec![Boton::Principal, Boton::Copiar, Boton::Mas]
        );
        match e.tecla(VK_C, false, true, false) {
            Orden::Hacer {
                accion: Accion::Pedido(p),
                ..
            } => assert_eq!(p["accion"], "copiar_imagen"),
            o => panic!("{o:?}"),
        }
        assert_eq!(
            e.tecla(VK_DELETE, false, false, false),
            Orden::BorrarCaptura("C:\\datos\\capturas\\muro.png".into())
        );
        assert_eq!(
            e.tecla(VK_RETURN, false, false, true),
            Orden::AbrirCon("C:\\datos\\capturas\\muro.png".into())
        );
        // Caso negativo: Ctrl+Z sin nada borrado no hace nada.
        assert_eq!(e.tecla(VK_Z, false, true, false), Orden::Nada);
        e.se_puede_deshacer = true;
        assert_eq!(e.tecla(VK_Z, false, true, false), Orden::Deshacer);
    }

    #[test]
    fn el_menu_se_recorre_con_flechas_y_se_elige_con_intro() {
        let mut e = mezcla();
        e.abrir_menu(vec![
            resultado("Uno", "", glifo::ABRIR, Accion::Copiar("1".into())),
            resultado("Dos", "", glifo::ABRIR, Accion::Copiar("2".into())),
        ]);
        e.tecla(VK_DOWN, false, false, false);
        e.tecla(VK_DOWN, false, false, false);
        assert_eq!(e.elegido, 0, "con el menu abierto las flechas son suyas");
        assert_eq!(
            e.tecla(VK_RETURN, false, false, false),
            Orden::Hacer {
                accion: Accion::Copiar("2".into()),
                recordar: false
            }
        );
        assert_eq!(e.tecla(VK_LEFT, false, false, false), Orden::Repintar);
        assert!(e.menu.is_none());
        // Caso negativo: un menu sin opciones no se abre.
        assert_eq!(e.abrir_menu(Vec::new()), Orden::Nada);
        assert!(e.menu.is_none());
    }

    #[test]
    fn lo_tecleado_se_escribe_y_ctrl_borrar_quita_una_palabra() {
        let mut e = Estado::default();
        for c in "t comprar pan".chars() {
            assert_eq!(e.caracter(c), Orden::Buscar);
        }
        assert_eq!(e.consulta, "t comprar pan");
        assert_eq!(e.caracter('\u{7f}'), Orden::Buscar);
        assert_eq!(e.consulta, "t comprar ");
        e.caracter('\u{8}');
        assert_eq!(e.consulta, "t comprar");
        // Caso negativo: Intro, Tab y Esc no se escriben; borrar en vacio no
        // busca otra vez.
        assert_eq!(e.caracter('\r'), Orden::Nada);
        assert_eq!(e.caracter('\t'), Orden::Nada);
        assert_eq!(e.caracter('\u{1b}'), Orden::Nada);
        e.consulta.clear();
        assert_eq!(e.caracter('\u{8}'), Orden::Nada);
        for _ in 0..1000 {
            e.caracter('a');
        }
        assert_eq!(e.consulta.chars().count(), LARGO_MAXIMO);
    }

    #[test]
    fn el_boton_azul_dice_lo_que_hace() {
        let p = |a: &str| Accion::Pedido(pedido(a, json!({})));
        assert_eq!(rotulo_principal(&p("pinear")), "buscar-todo-accion-pinear");
        assert_eq!(
            rotulo_principal(&p("anadir_tarea")),
            "buscar-todo-accion-apuntar"
        );
        assert_eq!(
            rotulo_principal(&p("nota_nueva")),
            "buscar-todo-accion-crear"
        );
        assert_eq!(rotulo_principal(&p("abrir")), "buscar-todo-accion-abrir");
        let marcar = |h: bool| Accion::PedirYSeguir {
            pedido: pedido("marcar_tarea", json!({ "hecha": h })),
            consulta: String::new(),
        };
        assert_eq!(rotulo_principal(&marcar(true)), "buscar-todo-accion-hecha");
        assert_eq!(
            rotulo_principal(&marcar(false)),
            "buscar-todo-accion-desmarcar"
        );
        // Caso negativo: un pedido desconocido no inventa un verbo: «Abrir».
        assert_eq!(
            rotulo_principal(&p("algo_nuevo")),
            "buscar-todo-accion-abrir"
        );
    }

    #[test]
    fn el_proyecto_no_repite_ver_en_el_chat_en_su_fila() {
        let mut r = con_clave("Thesis", Some("proyecto/t"));
        r.contexto = Some(json!({ "tipo": "proyecto", "proyecto": "t" }));
        // Intro ya abre su chat: el boton sobraria. Pero Ctrl+Intro vale.
        assert_eq!(botones_de(&r, false), vec![Boton::Principal]);
        assert!(ver_en_chat(&r).is_some());
        // Caso negativo: algo sin contexto no tiene «Ver en el chat».
        assert!(ver_en_chat(&con_clave("x", None)).is_none());
    }

    #[test]
    fn el_glifo_guardado_vuelve_a_su_constante() {
        assert_eq!(glifo_estatico(glifo::LECCION), glifo::LECCION);
        assert_eq!(
            glifo_estatico("zz"),
            glifo::ABRIR,
            "uno desconocido no rompe nada"
        );
    }
}
