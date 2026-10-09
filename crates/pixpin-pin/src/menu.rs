//! El menu del clic derecho (spec 4.3, rediseno v2).
//!
//! Menu nativo de Win32, como el de la bandeja: se ve exactamente igual que
//! el resto del sistema, respeta el tema y no cuesta un fotograma dibujarlo.
//!
//! **El mismo orden y los mismos verbos que la barra del pin** (`barra.rs`):
//! primero el control propio del tipo, luego Copiar, Anotar, Copiar el
//! texto y Dejar pasar el clic; despues lo de ver (tamano original y
//! opacidad), lo raro dentro de «Mas» y «Cerrar el pin» aparte, al final.
//! Cada entrada lleva su atajo a la derecha (el texto tras el tabulador, que
//! Windows alinea solo).
//!
//! Que entradas aparecen depende del tipo de pin, y esa decision es PURA y
//! esta probada aparte: montar el menu con Win32 pide escritorio, pero
//! equivocarse de entradas —ofrecer «Abrir ubicacion» en una nota, por
//! ejemplo— es un fallo de logica que debe cazarse en CI.

use crate::contenido::Contenido;

/// Identificadores de comando. Los colores ocupan 100..108 para que anadir
/// entradas arriba no los desplace.
pub const CMD_COPIAR: u32 = 1;
pub const CMD_GUARDAR_COMO: u32 = 2;
pub const CMD_ABRIR_UBICACION: u32 = 3;
pub const CMD_TAMANO_ORIGINAL: u32 = 4;
pub const CMD_OCULTAR_GRUPO: u32 = 5;
pub const CMD_CERRAR: u32 = 6;
pub const CMD_ELIMINAR: u32 = 7;
/// Reproducir o pausar un video (D68); la etiqueta dice lo que hara.
pub const CMD_REPRODUCIR: u32 = 8;
/// Alternar el silencio de un video (D68).
pub const CMD_SONIDO: u32 = 9;
/// Dejar pasar los clics a lo que hay debajo (P1.4).
pub const CMD_PASANTE: u32 = 10;
/// Leer el texto del pin y copiarlo (P4.2).
pub const CMD_TEXTO: u32 = 11;
/// Pasar a la pagina siguiente y anterior de un PDF.
pub const CMD_PAGINA_SIGUIENTE: u32 = 12;
pub const CMD_PAGINA_ANTERIOR: u32 = 13;
/// Pinear la pagina que se ve como pin propio.
pub const CMD_EXTRAER_PAGINA: u32 = 14;
/// Pinear TODAS las paginas, una por pin.
pub const CMD_EXTRAER_TODAS: u32 = 15;
/// Abrir el pin en el lienzo infinito del editor, con la imagen de fondo
/// (D131). Solo en pines de imagen.
pub const CMD_ABRIR_LIENZO: u32 = 16;
/// Dejar el fotograma que se ve de un pin en vivo como pin de imagen
/// normal, guardado en el almacen.
pub const CMD_CONGELAR: u32 = 17;
/// Encender o apagar el manejo a distancia de un pin en vivo: los clics y
/// la rueda sobre el pin van a la zona que se esta viendo. La etiqueta dice
/// lo que hara, como la de reproducir.
pub const CMD_REMOTO: u32 = 18;
/// Anotar encima (lo mismo que el doble clic), v2.
pub const CMD_ANOTAR: u32 = 19;
/// Abrir el panel «Pines abiertos», v2.
pub const CMD_PINES_ABIERTOS: u32 = 20;
/// Abrir una ficha con su aplicacion (lo mismo que el doble clic), v2.
pub const CMD_ABRIR: u32 = 21;
pub const CMD_SIN_GRUPO: u32 = 100;
pub const CMD_COLOR_BASE: u32 = 101;
/// «Convertir en…» una nota: 200 + el indice en `MiniApp::TODAS`. En el PC
/// el pin ya existe, y sacar la herramienta desde el es mas directo que
/// volver a copiar la palabra magica.
pub const CMD_CONVERTIR_BASE: u32 = 200;
/// El fondo de la pizarra: 300 + uno de los cuatro colores del movil, y
/// 310 + una de sus cinco pautas.
pub const CMD_PIZARRA_COLOR_BASE: u32 = 300;
pub const CMD_PIZARRA_PAUTA_BASE: u32 = 310;
/// Cuantos colores y pautas tiene la pizarra (`BOARD_COLORS`, `BoardGrid`).
pub const COLORES_PIZARRA: u8 = 4;
pub const PAUTAS_PIZARRA: u8 = 5;
/// La opacidad: 400 + el indice en `OPACIDADES`.
pub const CMD_OPACIDAD_BASE: u32 = 400;
/// Las opacidades que ofrece el menu, en por ciento.
pub const OPACIDADES: [u8; 8] = [100, 90, 80, 70, 60, 50, 40, 30];

/// Textos nuevos del rediseno v2, ya traducidos. Van aparte para que
/// anadirlos no toque mas que una linea de quien monta `TextosPin`.
#[derive(Debug, Clone, Default)]
pub struct TextosV2 {
    pub anotar: String,
    pub mas: String,
    pub opacidad: String,
    pub pines_abiertos: String,
    pub abrir: String,
    pub pinear_pagina: String,
    pub pinear_todas: String,
    pub cerrar_pin: String,
    pub alejar: String,
    pub acercar: String,
    pub en_vivo: String,
    /// Nombres de teclas que cambian con el idioma.
    pub tecla_espacio: String,
    pub tecla_rueda: String,
    pub tecla_re_pag: String,
    pub tecla_av_pag: String,
    pub tecla_mayus_rueda: String,
    /// El panel «Pines abiertos».
    pub panel_titulo: String,
    pub panel_buscar: String,
    pub panel_mostrar_todos: String,
    pub panel_ocultar_todos: String,
    pub panel_cerrar_todos: String,
    pub panel_deshacer: String,
    pub panel_sin_grupo: String,
    pub panel_oculto: String,
    pub panel_ayuda: String,
    pub panel_vacio: String,
    pub panel_ocultar: String,
    pub panel_mostrar: String,
    pub panel_cerrar_este: String,
    pub panel_cerrar_panel: String,
    /// Como se llama cada tipo de pin en el panel.
    pub tipo_foto: String,
    pub tipo_nota: String,
    pub tipo_pdf: String,
    pub tipo_video: String,
    pub tipo_archivo: String,
    pub tipo_vivo: String,
    pub tipo_herramienta: String,
    /// La barra de marcar el texto reconocido (8-oct-2026), en el orden de
    /// `barra_marcas::BOTONES`: copiar, resaltar, ondulada, subrayar, tachar,
    /// tapar; y el del circulo tachado, quitar la marca.
    pub marcas: [String; 6],
    pub quitar_marca: String,
    /// El boton T de la barra: entrar o salir del modo texto.
    pub modo_texto: String,
}

/// Textos del pin, YA traducidos: este crate no conoce Fluent (vive en
/// `pixpin-store`, su misma capa).
#[derive(Debug, Clone)]
pub struct TextosPin {
    pub copiar: String,
    pub guardar_como: String,
    pub abrir_ubicacion: String,
    pub tamano_original: String,
    pub grupo: String,
    pub sin_grupo: String,
    pub colores: [String; 8],
    pub ocultar_grupo: String,
    pub cerrar: String,
    pub eliminar: String,
    pub no_encontrado: String,
    /// Los del video (D68).
    pub reproducir: String,
    pub pausar: String,
    pub sonido: String,
    /// El de dejar pasar el clic (P1.4). Solo hace falta el de activarlo:
    /// un pin pasante ya no puede abrir su menu, asi que la vuelta es por
    /// el comando global.
    pub dejar_pasar_clic: String,
    /// Los del PDF: pasar de pagina y sacarlas como pines propios.
    pub pagina_siguiente: String,
    pub pagina_anterior: String,
    pub extraer_pagina: String,
    pub extraer_todas: String,
    /// El de leer el texto de la imagen (P4.2).
    pub copiar_texto: String,
    /// El de abrir la imagen en el lienzo del editor (D131).
    pub abrir_en_lienzo: String,
    /// El de congelar un pin en vivo como imagen.
    pub congelar: String,
    /// Los de manejar a distancia la zona de un pin en vivo.
    pub manejar: String,
    pub dejar_de_manejar: String,
    /// «Convertir en…» y el nombre de cada herramienta, en el orden de
    /// `MiniApp::TODAS`.
    pub convertir_en: String,
    pub herramientas: [String; 9],
    /// «Fondo de la pizarra», sus cuatro colores y sus cinco pautas.
    pub fondo_pizarra: String,
    pub colores_pizarra: [String; 4],
    pub pautas_pizarra: [String; 5],
    /// Lo nuevo del rediseno v2.
    pub v2: TextosV2,
}

/// Una linea del menu, ya decidida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntradaMenu {
    Accion {
        id: u32,
        etiqueta: String,
        /// El atajo que se ensena a la derecha («Ctrl+C»), si lo hay.
        atajo: Option<String>,
        /// Con la marca de «esto es lo que hay ahora».
        marcada: bool,
    },
    Separador,
    /// Un submenu, ya decidido entero.
    Submenu {
        etiqueta: String,
        entradas: Vec<EntradaMenu>,
    },
}

fn accion(id: u32, etiqueta: &str, atajo: Option<&str>) -> EntradaMenu {
    EntradaMenu::Accion {
        id,
        etiqueta: etiqueta.to_string(),
        atajo: atajo.map(str::to_string),
        marcada: false,
    }
}

fn marcada(id: u32, etiqueta: &str, si: bool) -> EntradaMenu {
    EntradaMenu::Accion {
        id,
        etiqueta: etiqueta.to_string(),
        atajo: None,
        marcada: si,
    }
}

/// Lo que el menu necesita saber del pin, aparte de su contenido.
///
/// Agrupado en vez de seis parametros sueltos: con tantos booleanos
/// seguidos, cambiar dos de sitio compila igual y el menu ensena otra
/// cosa. Con nombres, el compilador no deja equivocarse.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EstadoMenu {
    pub con_grupo: bool,
    pub reproduciendo: bool,
    pub pasante: bool,
    /// El equipo sabe reconocer texto.
    pub con_ocr: bool,
    /// Cuantas paginas tiene, si es un PDF. `None` si no lo es o si
    /// todavia no se ha mirado.
    pub paginas: Option<u32>,
    /// La pagina que se ve ahora, desde 0.
    pub pagina: u32,
    /// El pin en vivo esta manejando su zona a distancia.
    pub remoto: bool,
    /// Si el pin es una pizarra, su color y su pauta de ahora.
    pub pizarra: Option<(u8, u8)>,
    /// La opacidad de ahora, en por ciento. 0 se lee como 100 (el
    /// `Default` de las pruebas viejas).
    pub opacidad: u8,
}

/// Si a este contenido se le puede anotar encima: lo mismo que decide el
/// doble clic (fotos y notas).
pub fn anotable(contenido: &Contenido) -> bool {
    matches!(contenido, Contenido::Imagen(_) | Contenido::Nota { .. })
}

/// El submenu «Convertir en…» de una nota: todas las herramientas.
fn submenu_convertir(t: &TextosPin) -> EntradaMenu {
    EntradaMenu::Submenu {
        etiqueta: t.convertir_en.clone(),
        entradas: t
            .herramientas
            .iter()
            .enumerate()
            .map(|(i, nombre)| accion(CMD_CONVERTIR_BASE + i as u32, nombre, None))
            .collect(),
    }
}

/// El submenu del fondo de la pizarra: los cuatro colores, una raya y las
/// cinco pautas, con lo de ahora marcado. En el movil es una paleta que se
/// queda abierta; aqui, un menu nativo que se vuelve a abrir, que es lo
/// que se espera de un escritorio.
fn submenu_pizarra(t: &TextosPin, color: u8, pauta: u8) -> EntradaMenu {
    let mut entradas: Vec<EntradaMenu> = t
        .colores_pizarra
        .iter()
        .enumerate()
        .map(|(i, nombre)| marcada(CMD_PIZARRA_COLOR_BASE + i as u32, nombre, i as u8 == color))
        .collect();
    entradas.push(EntradaMenu::Separador);
    entradas.extend(
        t.pautas_pizarra.iter().enumerate().map(|(i, nombre)| {
            marcada(CMD_PIZARRA_PAUTA_BASE + i as u32, nombre, i as u8 == pauta)
        }),
    );
    EntradaMenu::Submenu {
        etiqueta: t.fondo_pizarra.clone(),
        entradas,
    }
}

/// El submenu de grupos: sin grupo mas los ocho colores.
fn submenu_grupo(t: &TextosPin) -> EntradaMenu {
    let mut entradas = vec![accion(CMD_SIN_GRUPO, &t.sin_grupo, None)];
    entradas.extend(
        t.colores
            .iter()
            .enumerate()
            .map(|(i, nombre)| accion(CMD_COLOR_BASE + i as u32, nombre, None)),
    );
    EntradaMenu::Submenu {
        etiqueta: t.grupo.clone(),
        entradas,
    }
}

/// El submenu de la opacidad, con la de ahora marcada.
fn submenu_opacidad(t: &TextosPin, ahora: u8) -> EntradaMenu {
    let ahora = if ahora == 0 { 100 } else { ahora };
    // La mas cercana a la de ahora: con Mayus + rueda se llega a valores
    // sueltos (85 %), y el menu marca el escalon mas parecido.
    let cerca = OPACIDADES
        .iter()
        .enumerate()
        .min_by_key(|(_, o)| (**o as i32 - ahora as i32).abs())
        .map(|(i, _)| i)
        .unwrap_or(0);
    EntradaMenu::Submenu {
        etiqueta: format!("{}\t{}", t.v2.opacidad, t.v2.tecla_mayus_rueda),
        entradas: OPACIDADES
            .iter()
            .enumerate()
            .map(|(i, o)| marcada(CMD_OPACIDAD_BASE + i as u32, &format!("{o} %"), i == cerca))
            .collect(),
    }
}

/// Que entradas tiene el menu de este pin. PURA y probada en CI.
pub fn entradas_del_menu(
    contenido: &Contenido,
    estado: EstadoMenu,
    t: &TextosPin,
) -> Vec<EntradaMenu> {
    let EstadoMenu {
        con_grupo,
        reproduciendo,
        pasante,
        con_ocr,
        paginas,
        pagina,
        remoto,
        pizarra,
        opacidad,
    } = estado;
    let mut v = Vec::new();
    let es_vivo = matches!(contenido, Contenido::Vivo { .. });
    let reproducir = || {
        accion(
            CMD_REPRODUCIR,
            if reproduciendo {
                &t.pausar
            } else {
                &t.reproducir
            },
            Some(&t.v2.tecla_espacio),
        )
    };

    // 1. El control propio, arriba: es lo que se busca al abrir el menu de
    //    un video, un PDF o un pin en vivo (D68). El mismo que abre la barra.
    match contenido {
        Contenido::Vivo { .. } => {
            v.push(reproducir());
            v.push(accion(CMD_CONGELAR, &t.congelar, None));
            // Con la pausa: es lo que distingue a este pin de una foto, y el
            // unico sitio por donde se apaga (con el modo encendido el clic
            // izquierdo ya no es del pin).
            v.push(accion(
                CMD_REMOTO,
                if remoto {
                    &t.dejar_de_manejar
                } else {
                    &t.manejar
                },
                None,
            ));
        }
        Contenido::Video { .. } => {
            v.push(reproducir());
            v.push(accion(CMD_SONIDO, &t.sonido, Some("M")));
        }
        Contenido::Archivo { .. } => {
            v.push(accion(CMD_ABRIR, &t.v2.abrir, None));
        }
        _ => {}
    }
    if let Some(cuantas) = paginas.filter(|c| *c > 1) {
        if pagina + 1 < cuantas {
            v.push(accion(
                CMD_PAGINA_SIGUIENTE,
                &t.pagina_siguiente,
                Some(&t.v2.tecla_av_pag),
            ));
        }
        if pagina > 0 {
            v.push(accion(
                CMD_PAGINA_ANTERIOR,
                &t.pagina_anterior,
                Some(&t.v2.tecla_re_pag),
            ));
        }
    }
    // Pinear se ofrece aunque solo haya una pagina: sacarla como imagen
    // propia para anotarla o copiarla es util igual.
    if paginas.is_some() {
        v.push(accion(CMD_EXTRAER_PAGINA, &t.v2.pinear_pagina, None));
    } else if matches!(contenido, Contenido::Documento { .. }) {
        v.push(accion(CMD_ABRIR, &t.v2.abrir, None));
    }
    if !v.is_empty() {
        v.push(EntradaMenu::Separador);
    }

    // 2. Lo comun, en el orden de la barra. El pin en vivo copia el
    //    fotograma que se ve.
    v.push(accion(CMD_COPIAR, &t.copiar, Some("Ctrl+C")));
    if anotable(contenido) {
        v.push(accion(CMD_ANOTAR, &t.v2.anotar, Some("A")));
    }
    // Leer el texto: solo en lo que ES una imagen —en una nota el texto ya
    // lo tienes, y de un archivo por referencia no hay pixeles que leer— y
    // solo si el equipo sabe reconocer texto: ofrecerlo sin motor daria un
    // error en vez de una funcion.
    if con_ocr
        && !es_vivo
        && contenido.redimensionable()
        && !matches!(
            contenido,
            Contenido::Nota { .. } | Contenido::Herramienta { .. }
        )
    {
        v.push(accion(CMD_TEXTO, &t.copiar_texto, Some("T")));
    }
    // «Dejar pasar el clic» solo se ofrece si NO lo esta ya: estando
    // pasante, este menu ni siquiera se abre, porque el clic derecho pasa
    // de largo. Ofrecer una entrada para desactivarlo seria prometer algo
    // a lo que no se puede llegar.
    if !pasante {
        v.push(accion(CMD_PASANTE, &t.dejar_pasar_clic, Some("Ctrl+T")));
    }

    // 3. Ver: el tamano y la opacidad.
    v.push(EntradaMenu::Separador);
    if contenido.redimensionable() && !matches!(contenido, Contenido::Video { .. }) {
        v.push(accion(
            CMD_TAMANO_ORIGINAL,
            &t.tamano_original,
            Some("Ctrl+0"),
        ));
    }
    v.push(submenu_opacidad(t, opacidad));

    // 4. «Mas»: lo que se usa poco.
    let mut mas = Vec::new();
    if !es_vivo {
        match contenido {
            // Un archivo no se «guarda como»: ya es un fichero del usuario y
            // esta donde el lo dejo. Lo util es llegar hasta el. El video y
            // el documento son archivos por referencia igual (D65).
            Contenido::Archivo { .. } | Contenido::Video { .. } | Contenido::Documento { .. } => {
                mas.push(accion(CMD_ABRIR_UBICACION, &t.abrir_ubicacion, None))
            }
            _ => mas.push(accion(CMD_GUARDAR_COMO, &t.guardar_como, None)),
        }
        // Abrir en el lienzo va con las acciones de la imagen: dibujar sin
        // el borde del pin. El doble clic sigue anotando dentro del pin.
        if matches!(contenido, Contenido::Imagen(_)) {
            mas.push(accion(CMD_ABRIR_LIENZO, &t.abrir_en_lienzo, None));
            // El fondo de la pizarra, junto al lienzo: los dos cambian el
            // papel sobre el que se dibuja.
            if let Some((color, pauta)) = pizarra {
                mas.push(submenu_pizarra(t, color, pauta));
            }
        }
        // Una nota se convierte en herramienta: sus lineas pasan a ser las
        // tareas, los gastos o los nombres de la ruleta (C3, propio del PC).
        if matches!(contenido, Contenido::Nota { .. }) {
            mas.push(submenu_convertir(t));
        }
        if paginas.is_some_and(|c| c > 1) {
            mas.push(accion(CMD_EXTRAER_TODAS, &t.v2.pinear_todas, None));
        }
        mas.push(EntradaMenu::Separador);
        mas.push(submenu_grupo(t));
        if con_grupo {
            mas.push(accion(CMD_OCULTAR_GRUPO, &t.ocultar_grupo, None));
        }
    }
    mas.push(accion(CMD_PINES_ABIERTOS, &t.v2.pines_abiertos, None));
    if !es_vivo {
        mas.push(EntradaMenu::Separador);
        mas.push(accion(CMD_ELIMINAR, &t.eliminar, None));
    }
    v.push(EntradaMenu::Submenu {
        etiqueta: t.v2.mas.clone(),
        entradas: mas,
    });

    // 5. Cerrar, aparte y al final.
    v.push(EntradaMenu::Separador);
    v.push(accion(CMD_CERRAR, &t.v2.cerrar_pin, Some("Esc")));
    v
}

/// El texto de una entrada con su atajo detras de un tabulador: Windows lo
/// alinea a la derecha del menu.
fn con_atajo(etiqueta: &str, atajo: &Option<String>) -> String {
    match atajo {
        Some(a) if !a.is_empty() => format!("{etiqueta}\t{a}"),
        _ => etiqueta.to_string(),
    }
}

/// Muestra el menu en `punto` (pantalla), o donde este el raton si es
/// `None`, y devuelve el comando elegido, o `None` si se cerro sin elegir.
///
/// El patron es el de la bandeja de S1-A, con sus dos trampas: el menu se
/// destruye SIEMPRE (por eso el cierre intermedio), y `SetForegroundWindow`
/// va antes de `TrackPopupMenu` o el menu no se cierra al pulsar fuera.
pub fn mostrar(
    hwnd: windows::Win32::Foundation::HWND,
    contenido: &Contenido,
    estado: EstadoMenu,
    t: &TextosPin,
    punto: Option<(i32, i32)>,
) -> Option<u32> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, HMENU, MF_CHECKED, MF_POPUP,
        MF_SEPARATOR, MF_STRING, SetForegroundWindow, TPM_LEFTALIGN, TPM_RETURNCMD,
        TPM_RIGHTBUTTON, TrackPopupMenu,
    };
    use windows::core::HSTRING;

    /// Cuelga las entradas de `menu`, submenus incluidos. Cada submenu se
    /// cuelga del padre en cuanto se crea: desde ese momento lo destruye el
    /// `DestroyMenu` del de arriba, y un fallo a medias no deja huerfanos.
    fn colgar(menu: HMENU, entradas: &[EntradaMenu]) -> windows::core::Result<()> {
        for e in entradas {
            // SAFETY: menus vivos creados en esta misma llamada; las cadenas
            // viajan como HSTRING, que vive durante la llamada.
            unsafe {
                match e {
                    EntradaMenu::Separador => AppendMenuW(menu, MF_SEPARATOR, 0, None)?,
                    EntradaMenu::Accion {
                        id,
                        etiqueta,
                        atajo,
                        marcada,
                    } => AppendMenuW(
                        menu,
                        if *marcada {
                            MF_STRING | MF_CHECKED
                        } else {
                            MF_STRING
                        },
                        *id as usize,
                        &HSTRING::from(con_atajo(etiqueta, atajo)),
                    )?,
                    EntradaMenu::Submenu { etiqueta, entradas } => {
                        let hijo = CreatePopupMenu()?;
                        AppendMenuW(
                            menu,
                            MF_POPUP,
                            hijo.0 as usize,
                            &HSTRING::from(etiqueta.as_str()),
                        )?;
                        colgar(hijo, entradas)?;
                    }
                }
            }
        }
        Ok(())
    }

    // SAFETY: todo lo que se crea aqui se destruye antes de salir; el hwnd
    // es la ventana propia del pin.
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        let armado = colgar(menu, &entradas_del_menu(contenido, estado, t));
        let elegido = if armado.is_ok() {
            let (x, y) = punto.unwrap_or_else(|| {
                let mut p = POINT::default();
                let _ = GetCursorPos(&mut p);
                (p.x, p.y)
            });
            // Sin esto el menu se queda abierto al pulsar fuera. Requisito
            // documentado de TrackPopupMenu que se olvida siempre.
            let _ = SetForegroundWindow(hwnd);
            let r = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_LEFTALIGN | TPM_RIGHTBUTTON,
                x,
                y,
                None,
                hwnd,
                None,
            );
            if r.0 == 0 { None } else { Some(r.0 as u32) }
        } else {
            None
        };
        // Los submenus se destruyen con el padre; destruirlos aparte seria
        // un doble libre.
        let _ = DestroyMenu(menu);
        elegido
    }
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;
    use pixpin_codec::ImagenRgba;

    pub(crate) fn textos() -> TextosPin {
        TextosPin {
            copiar: "Copiar".into(),
            guardar_como: "Guardar como…".into(),
            abrir_ubicacion: "Abrir ubicación".into(),
            tamano_original: "Tamaño original".into(),
            grupo: "Grupo".into(),
            sin_grupo: "Sin grupo".into(),
            colores: [
                "Rojo".into(),
                "Naranja".into(),
                "Ámbar".into(),
                "Verde".into(),
                "Cian".into(),
                "Azul".into(),
                "Violeta".into(),
                "Rosa".into(),
            ],
            ocultar_grupo: "Ocultar este grupo".into(),
            cerrar: "Cerrar".into(),
            eliminar: "Eliminar del almacén…".into(),
            no_encontrado: "No encontrado".into(),
            reproducir: "Reproducir".into(),
            pausar: "Pausar".into(),
            sonido: "Sonido".into(),
            dejar_pasar_clic: "Dejar pasar el clic".into(),
            copiar_texto: "Copiar el texto".into(),
            abrir_en_lienzo: "Abrir en lienzo".into(),
            pagina_siguiente: "Pagina siguiente".into(),
            pagina_anterior: "Pagina anterior".into(),
            extraer_pagina: "Extraer esta pagina".into(),
            extraer_todas: "Extraer todas las paginas".into(),
            congelar: "Congelar como imagen".into(),
            manejar: "Manejar a distancia".into(),
            dejar_de_manejar: "Dejar de manejar".into(),
            convertir_en: "Convertir en".into(),
            herramientas: [
                "Temporizador".into(),
                "Cronometro".into(),
                "Tareas".into(),
                "Contador".into(),
                "Gastos".into(),
                "Pizarra".into(),
                "Ruleta".into(),
                "Lienzo".into(),
                "Hoja".into(),
            ],
            fondo_pizarra: "Fondo de la pizarra".into(),
            colores_pizarra: [
                "Blanco".into(),
                "Negro".into(),
                "Azul".into(),
                "Verde".into(),
            ],
            pautas_pizarra: [
                "Lisa".into(),
                "Cuadros".into(),
                "Rayas".into(),
                "Columnas".into(),
                "Puntos".into(),
            ],
            v2: TextosV2 {
                anotar: "Anotar".into(),
                mas: "Más".into(),
                opacidad: "Opacidad".into(),
                pines_abiertos: "Pines abiertos…".into(),
                abrir: "Abrir".into(),
                pinear_pagina: "Pinear esta página".into(),
                pinear_todas: "Pinear todas las páginas".into(),
                cerrar_pin: "Cerrar el pin".into(),
                tecla_espacio: "Espacio".into(),
                tecla_re_pag: "RePág".into(),
                tecla_av_pag: "AvPág".into(),
                tecla_mayus_rueda: "Mayús+Rueda".into(),
                tecla_rueda: "Rueda".into(),
                alejar: "Alejar".into(),
                acercar: "Acercar".into(),
                en_vivo: "EN VIVO".into(),
                panel_titulo: "Pines abiertos".into(),
                panel_buscar: "Buscar un pin".into(),
                panel_mostrar_todos: "Mostrar todos".into(),
                panel_ocultar_todos: "Ocultar todos".into(),
                panel_cerrar_todos: "Cerrar todos · se puede deshacer".into(),
                panel_deshacer: "Deshacer: volver a abrirlos".into(),
                panel_sin_grupo: "Sin grupo".into(),
                panel_oculto: "Oculto".into(),
                panel_ayuda: "Clic en una fila: trae el pin al centro y lo resalta.".into(),
                panel_vacio: "No hay pines abiertos.".into(),
                ..Default::default()
            },
        }
    }

    /// Todas las entradas, submenus incluidos, en orden.
    fn todas(v: &[EntradaMenu]) -> Vec<&EntradaMenu> {
        let mut r = Vec::new();
        for e in v {
            r.push(e);
            if let EntradaMenu::Submenu { entradas, .. } = e {
                r.extend(todas(entradas));
            }
        }
        r
    }

    fn submenu<'a>(v: &'a [EntradaMenu], etiqueta: &str) -> Option<&'a [EntradaMenu]> {
        todas(v).into_iter().find_map(|e| match e {
            EntradaMenu::Submenu {
                etiqueta: t,
                entradas,
            } if t.starts_with(etiqueta) => Some(entradas.as_slice()),
            _ => None,
        })
    }

    /// Los ids de todas las acciones, submenus incluidos.
    fn ids(v: &[EntradaMenu]) -> Vec<u32> {
        todas(v)
            .into_iter()
            .filter_map(|e| match e {
                EntradaMenu::Accion { id, .. } => Some(*id),
                _ => None,
            })
            .collect()
    }

    /// Los ids del primer nivel solo.
    fn ids_arriba(v: &[EntradaMenu]) -> Vec<u32> {
        v.iter()
            .filter_map(|e| match e {
                EntradaMenu::Accion { id, .. } => Some(*id),
                _ => None,
            })
            .collect()
    }

    fn atajo_de(v: &[EntradaMenu], buscado: u32) -> Option<String> {
        todas(v).into_iter().find_map(|e| match e {
            EntradaMenu::Accion { id, atajo, .. } if *id == buscado => atajo.clone(),
            _ => None,
        })
    }

    fn etiqueta_de(v: &[EntradaMenu], buscado: u32) -> Option<String> {
        todas(v).into_iter().find_map(|e| match e {
            EntradaMenu::Accion { id, etiqueta, .. } if *id == buscado => Some(etiqueta.clone()),
            _ => None,
        })
    }

    fn imagen() -> Contenido {
        Contenido::Imagen(ImagenRgba {
            ancho: 1,
            alto: 1,
            pixeles: vec![0, 0, 0, 255],
        })
    }

    fn archivo() -> Contenido {
        Contenido::Archivo {
            nombre: "informe.pdf".into(),
            detalle: "1,2 MB".into(),
            icono: None,
            existe: true,
        }
    }

    fn video() -> Contenido {
        Contenido::Video {
            nombre: "clip.mp4".into(),
            ruta: std::path::PathBuf::from("clip.mp4"),
            ancho: 0,
            alto: 0,
        }
    }

    fn con_ocr() -> EstadoMenu {
        EstadoMenu {
            con_ocr: true,
            ..Default::default()
        }
    }

    #[test]
    fn el_menu_sigue_el_orden_de_la_barra_y_cerrar_va_al_final() {
        let v = entradas_del_menu(&imagen(), con_ocr(), &textos());
        assert_eq!(
            &ids_arriba(&v)[..4],
            &[CMD_COPIAR, CMD_ANOTAR, CMD_TEXTO, CMD_PASANTE],
            "Copiar, Anotar, Copiar el texto, Dejar pasar el clic"
        );
        assert_eq!(
            v.last(),
            Some(&EntradaMenu::Accion {
                id: CMD_CERRAR,
                etiqueta: "Cerrar el pin".into(),
                atajo: Some("Esc".into()),
                marcada: false,
            })
        );
        // Justo antes de Cerrar, una raya: va aparte.
        assert_eq!(v[v.len() - 2], EntradaMenu::Separador);
        // «Mas» es el ultimo submenu antes de cerrar.
        assert!(
            matches!(&v[v.len() - 3], EntradaMenu::Submenu { etiqueta, .. } if etiqueta == "Más")
        );
    }

    #[test]
    fn cada_accion_de_la_barra_ensena_su_atajo() {
        let v = entradas_del_menu(&imagen(), con_ocr(), &textos());
        assert_eq!(atajo_de(&v, CMD_COPIAR).as_deref(), Some("Ctrl+C"));
        assert_eq!(atajo_de(&v, CMD_ANOTAR).as_deref(), Some("A"));
        assert_eq!(atajo_de(&v, CMD_TEXTO).as_deref(), Some("T"));
        assert_eq!(atajo_de(&v, CMD_TAMANO_ORIGINAL).as_deref(), Some("Ctrl+0"));
        // Caso negativo: lo de «Mas» no tiene tecla y no se inventa una.
        assert_eq!(atajo_de(&v, CMD_GUARDAR_COMO), None);
    }

    #[test]
    fn el_atajo_va_tras_un_tabulador_y_sin_atajo_no_hay_tabulador() {
        assert_eq!(
            con_atajo("Copiar", &Some("Ctrl+C".into())),
            "Copiar\tCtrl+C"
        );
        assert_eq!(con_atajo("Más", &None), "Más");
        assert_eq!(con_atajo("Más", &Some(String::new())), "Más");
    }

    #[test]
    fn la_opacidad_marca_el_escalon_mas_cercano() {
        let v = entradas_del_menu(
            &imagen(),
            EstadoMenu {
                opacidad: 85,
                ..Default::default()
            },
            &textos(),
        );
        let s = submenu(&v, "Opacidad").expect("siempre se ofrece");
        let marcadas: Vec<u32> = s
            .iter()
            .filter_map(|e| match e {
                EntradaMenu::Accion {
                    id, marcada: true, ..
                } => Some(*id),
                _ => None,
            })
            .collect();
        // 85 esta entre 90 y 80; gana el primero, 90.
        assert_eq!(marcadas, vec![CMD_OPACIDAD_BASE + 1]);
        // Caso negativo: el Default (0) se lee como opaco, no como invisible.
        let d = entradas_del_menu(&imagen(), EstadoMenu::default(), &textos());
        let s = submenu(&d, "Opacidad").unwrap();
        assert!(matches!(&s[0], EntradaMenu::Accion { marcada: true, .. }));
    }

    #[test]
    fn pinear_se_llama_pinear_en_el_pdf() {
        let v = entradas_del_menu(
            &imagen(),
            EstadoMenu {
                paginas: Some(5),
                pagina: 1,
                ..Default::default()
            },
            &textos(),
        );
        assert_eq!(
            etiqueta_de(&v, CMD_EXTRAER_PAGINA).as_deref(),
            Some("Pinear esta página")
        );
        assert_eq!(
            etiqueta_de(&v, CMD_EXTRAER_TODAS).as_deref(),
            Some("Pinear todas las páginas")
        );
        // Las paginas, arriba: el control propio va primero.
        assert_eq!(ids_arriba(&v)[0], CMD_PAGINA_SIGUIENTE);
    }

    #[test]
    fn una_nota_se_puede_convertir_en_cualquier_herramienta() {
        let nota = Contenido::Nota {
            texto: "pan\nleche".into(),
        };
        let v = entradas_del_menu(&nota, EstadoMenu::default(), &textos());
        let s = submenu(&v, "Convertir en").expect("la nota ofrece convertir");
        assert_eq!(s.len(), 9);
        assert_eq!(
            s[2],
            EntradaMenu::Accion {
                id: CMD_CONVERTIR_BASE + 2,
                etiqueta: "Tareas".into(),
                atajo: None,
                marcada: false
            }
        );
        // Caso negativo: una imagen no tiene lineas que convertir.
        let i = entradas_del_menu(&imagen(), EstadoMenu::default(), &textos());
        assert!(submenu(&i, "Convertir en").is_none());
    }

    #[test]
    fn la_pizarra_ofrece_su_fondo_con_lo_de_ahora_marcado() {
        let v = entradas_del_menu(
            &imagen(),
            EstadoMenu {
                pizarra: Some((2, 4)),
                ..Default::default()
            },
            &textos(),
        );
        let s = submenu(&v, "Fondo de la pizarra").expect("una pizarra ofrece su fondo");
        let marcadas: Vec<u32> = s
            .iter()
            .filter_map(|e| match e {
                EntradaMenu::Accion {
                    id, marcada: true, ..
                } => Some(*id),
                _ => None,
            })
            .collect();
        assert_eq!(
            marcadas,
            vec![CMD_PIZARRA_COLOR_BASE + 2, CMD_PIZARRA_PAUTA_BASE + 4]
        );
        assert_eq!(
            s.iter().filter(|e| **e == EntradaMenu::Separador).count(),
            1,
            "una raya entre las dos"
        );
        // Caso negativo: una imagen corriente no es una pizarra.
        let normal = entradas_del_menu(&imagen(), EstadoMenu::default(), &textos());
        assert!(submenu(&normal, "Fondo de la pizarra").is_none());
    }

    #[test]
    fn una_herramienta_no_ofrece_leer_su_texto_ni_convertirse_ni_anotar() {
        let h = Contenido::Herramienta {
            ancho: 300,
            alto: 300,
        };
        let v = entradas_del_menu(&h, con_ocr(), &textos());
        let ids = ids(&v);
        assert!(ids.contains(&CMD_COPIAR) && ids.contains(&CMD_CERRAR));
        assert!(!ids.contains(&CMD_TEXTO));
        assert!(!ids.contains(&CMD_ABRIR_LIENZO));
        assert!(!ids.contains(&CMD_ANOTAR));
        assert!(submenu(&v, "Convertir en").is_none());
    }

    #[test]
    fn el_pin_en_vivo_ofrece_manejar_y_encendido_ofrece_dejarlo() {
        let vivo = Contenido::Vivo {
            ancho: 300,
            alto: 200,
        };
        let etiqueta = |remoto: bool| {
            etiqueta_de(
                &entradas_del_menu(
                    &vivo,
                    EstadoMenu {
                        remoto,
                        ..Default::default()
                    },
                    &textos(),
                ),
                CMD_REMOTO,
            )
        };
        assert_eq!(etiqueta(false).as_deref(), Some("Manejar a distancia"));
        assert_eq!(
            etiqueta(true).as_deref(),
            Some("Dejar de manejar"),
            "encendido, el menu es la unica salida y tiene que decirlo"
        );
    }

    #[test]
    fn una_imagen_quieta_no_ofrece_manejar_a_distancia() {
        // Caso negativo: detras de una foto no hay zona viva que manejar.
        let e = entradas_del_menu(&imagen(), EstadoMenu::default(), &textos());
        assert!(!ids(&e).contains(&CMD_REMOTO));
    }

    #[test]
    fn el_pin_en_vivo_ofrece_pausar_congelar_y_cerrar_arriba_la_pausa() {
        let vivo = Contenido::Vivo {
            ancho: 300,
            alto: 200,
        };
        let e = entradas_del_menu(
            &vivo,
            EstadoMenu {
                reproduciendo: true,
                ..Default::default()
            },
            &textos(),
        );
        assert_eq!(etiqueta_de(&e, CMD_REPRODUCIR).as_deref(), Some("Pausar"));
        assert_eq!(
            ids_arriba(&e)[0],
            CMD_REPRODUCIR,
            "corriendo, lo primero es pausar"
        );
        let ids = ids(&e);
        for esperado in [
            CMD_CONGELAR,
            CMD_COPIAR,
            CMD_TAMANO_ORIGINAL,
            CMD_CERRAR,
            CMD_PINES_ABIERTOS,
        ] {
            assert!(ids.contains(&esperado), "falta {esperado}");
        }
        // Caso negativo: sin entrada en el almacen no hay nada que
        // eliminar, guardar, agrupar ni leer (copiar si: el fotograma).
        for ausente in [
            CMD_ELIMINAR,
            CMD_GUARDAR_COMO,
            CMD_TEXTO,
            CMD_ABRIR_LIENZO,
            CMD_SIN_GRUPO,
            CMD_ANOTAR,
        ] {
            assert!(!ids.contains(&ausente), "sobra {ausente}");
        }
    }

    #[test]
    fn el_pin_en_vivo_en_pausa_ofrece_reanudar() {
        let vivo = Contenido::Vivo {
            ancho: 300,
            alto: 200,
        };
        let e = entradas_del_menu(&vivo, EstadoMenu::default(), &textos());
        assert_eq!(
            etiqueta_de(&e, CMD_REPRODUCIR).as_deref(),
            Some("Reproducir")
        );
    }

    #[test]
    fn el_video_tiene_reproducir_y_sonido_arriba_y_no_guardar_como() {
        let v = entradas_del_menu(
            &video(),
            EstadoMenu {
                reproduciendo: true,
                con_ocr: true,
                ..Default::default()
            },
            &textos(),
        );
        assert_eq!(&ids_arriba(&v)[..2], &[CMD_REPRODUCIR, CMD_SONIDO]);
        assert_eq!(etiqueta_de(&v, CMD_REPRODUCIR).as_deref(), Some("Pausar"));
        assert_eq!(atajo_de(&v, CMD_REPRODUCIR).as_deref(), Some("Espacio"));
        assert_eq!(atajo_de(&v, CMD_SONIDO).as_deref(), Some("M"));
        let ids = ids(&v);
        assert!(ids.contains(&CMD_ABRIR_UBICACION));
        // Caso negativo: un video es un archivo del usuario, no se «guarda
        // como» ni tiene «tamano original» de pixeles hasta que se conoce.
        assert!(!ids.contains(&CMD_GUARDAR_COMO));
        assert!(!ids.contains(&CMD_TAMANO_ORIGINAL));
        assert!(!ids.contains(&CMD_ANOTAR));
    }

    #[test]
    fn el_documento_abre_ubicacion_como_la_ficha_y_no_tiene_controles() {
        let d = Contenido::Documento {
            nombre: "informe.docx".into(),
            vista: ImagenRgba {
                ancho: 1,
                alto: 1,
                pixeles: vec![0; 4],
            },
        };
        let ids = ids(&entradas_del_menu(&d, con_ocr(), &textos()));
        assert!(ids.contains(&CMD_ABRIR_UBICACION));
        assert!(ids.contains(&CMD_ABRIR), "abrir, como el doble clic");
        assert!(!ids.contains(&CMD_REPRODUCIR));
        assert!(!ids.contains(&CMD_SONIDO));
        assert!(!ids.contains(&CMD_GUARDAR_COMO));
    }

    #[test]
    fn una_imagen_sin_grupo_no_ofrece_ocultar_ni_abrir_ubicacion() {
        let v = entradas_del_menu(&imagen(), con_ocr(), &textos());
        let ids = ids(&v);
        assert!(ids.contains(&CMD_GUARDAR_COMO));
        assert!(ids.contains(&CMD_TAMANO_ORIGINAL));
        assert!(
            !ids.contains(&CMD_OCULTAR_GRUPO),
            "sin grupo no hay grupo que ocultar"
        );
        assert!(
            !ids.contains(&CMD_ABRIR_UBICACION),
            "una imagen del almacen no tiene ubicacion que abrir"
        );
    }

    #[test]
    fn una_nota_se_guarda_se_anota_y_tiene_tamano_original() {
        let nota = Contenido::Nota { texto: "x".into() };
        let ids = ids(&entradas_del_menu(&nota, con_ocr(), &textos()));
        assert!(ids.contains(&CMD_GUARDAR_COMO));
        assert!(ids.contains(&CMD_ANOTAR));
        assert!(
            ids.contains(&CMD_TAMANO_ORIGINAL),
            "el doble clic devuelve la nota a como nacio"
        );
    }

    #[test]
    fn una_imagen_con_grupo_si_ofrece_ocultarlo() {
        let v = entradas_del_menu(
            &imagen(),
            EstadoMenu {
                con_grupo: true,
                ..Default::default()
            },
            &textos(),
        );
        assert!(ids(&v).contains(&CMD_OCULTAR_GRUPO));
    }

    #[test]
    fn un_archivo_ofrece_abrir_y_su_ubicacion_y_no_tamano_original() {
        // Caso negativo del tipo: «Tamaño original» sobre una ficha no
        // significa nada, y «Guardar como» duplicaria un fichero que ya
        // existe donde el usuario lo puso.
        let v = entradas_del_menu(&archivo(), con_ocr(), &textos());
        assert_eq!(ids_arriba(&v)[0], CMD_ABRIR, "su control propio, arriba");
        let ids = ids(&v);
        assert!(ids.contains(&CMD_ABRIR_UBICACION));
        assert!(!ids.contains(&CMD_TAMANO_ORIGINAL));
        assert!(!ids.contains(&CMD_GUARDAR_COMO));
        assert!(!ids.contains(&CMD_ANOTAR));
    }

    #[test]
    fn leer_el_texto_solo_se_ofrece_donde_hay_pixeles_y_motor() {
        assert!(ids(&entradas_del_menu(&imagen(), con_ocr(), &textos())).contains(&CMD_TEXTO));
        // Caso negativo, el importante: sin motor de reconocimiento NO se
        // ofrece.
        let sin = ids(&entradas_del_menu(
            &imagen(),
            EstadoMenu::default(),
            &textos(),
        ));
        assert!(!sin.contains(&CMD_TEXTO));
        // Y en una nota tampoco: el texto ya lo tienes escrito.
        let nota = Contenido::Nota {
            texto: "hola".into(),
        };
        assert!(!ids(&entradas_del_menu(&nota, con_ocr(), &textos())).contains(&CMD_TEXTO));
    }

    #[test]
    fn un_pin_pasante_no_ofrece_volver_a_dejarlo_pasar() {
        let normal = ids(&entradas_del_menu(&imagen(), con_ocr(), &textos()));
        assert!(normal.contains(&CMD_PASANTE));
        let pasante = ids(&entradas_del_menu(
            &imagen(),
            EstadoMenu {
                pasante: true,
                con_ocr: true,
                ..Default::default()
            },
            &textos(),
        ));
        assert!(!pasante.contains(&CMD_PASANTE));
        // Y lo demas sigue estando: no se pierde nada por el camino.
        assert!(pasante.contains(&CMD_COPIAR));
        assert!(pasante.contains(&CMD_CERRAR));
    }

    #[test]
    fn todos_los_menus_ofrecen_copiar_cerrar_eliminar_y_el_panel() {
        for (c, grupo) in [(imagen(), false), (archivo(), true)] {
            let ids = ids(&entradas_del_menu(
                &c,
                EstadoMenu {
                    con_grupo: grupo,
                    con_ocr: true,
                    ..Default::default()
                },
                &textos(),
            ));
            for esperado in [CMD_COPIAR, CMD_CERRAR, CMD_ELIMINAR, CMD_PINES_ABIERTOS] {
                assert!(ids.contains(&esperado), "falta la entrada {esperado}");
            }
        }
    }

    #[test]
    fn abrir_en_lienzo_solo_esta_en_los_pines_de_imagen() {
        assert!(
            ids(&entradas_del_menu(&imagen(), con_ocr(), &textos())).contains(&CMD_ABRIR_LIENZO)
        );
        let nota = Contenido::Nota { texto: "x".into() };
        for c in [
            nota,
            archivo(),
            video(),
            Contenido::Documento {
                nombre: "a.pdf".into(),
                vista: ImagenRgba {
                    ancho: 1,
                    alto: 1,
                    pixeles: vec![0; 4],
                },
            },
        ] {
            assert!(
                !ids(&entradas_del_menu(&c, con_ocr(), &textos())).contains(&CMD_ABRIR_LIENZO),
                "{c:?}"
            );
        }
    }

    #[test]
    fn el_submenu_de_grupo_esta_siempre_dentro_de_mas() {
        let v = entradas_del_menu(&imagen(), con_ocr(), &textos());
        let mas = submenu(&v, "Más").unwrap();
        let grupo = submenu(mas, "Grupo").expect("el grupo va en «Mas»");
        assert_eq!(grupo.len(), 9, "sin grupo y los ocho colores");
        // Caso negativo: no esta tambien arriba.
        assert!(
            !v.iter()
                .any(|e| matches!(e, EntradaMenu::Submenu { etiqueta, .. } if etiqueta == "Grupo"))
        );
    }
}
