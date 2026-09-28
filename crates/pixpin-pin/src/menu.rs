//! El menu del clic derecho (spec 4.3).
//!
//! Menu nativo de Win32, como el de la bandeja: se ve exactamente igual que
//! el resto del sistema, respeta el tema y no cuesta un fotograma dibujarlo.
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
/// Sacar la pagina que se ve como pin propio.
pub const CMD_EXTRAER_PAGINA: u32 = 14;
/// Sacar TODAS las paginas, una por pin.
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
}

/// Una linea de un submenu. `None` en la lista es un separador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcionSubmenu {
    pub id: u32,
    pub etiqueta: String,
    /// Con la marca de «esto es lo que hay ahora».
    pub marcada: bool,
}

/// Una linea del menu, ya decidida. `Separador` no lleva texto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntradaMenu {
    Accion {
        id: u32,
        etiqueta: String,
    },
    Separador,
    /// El submenu de grupos: sin grupo mas los ocho colores.
    SubmenuGrupo,
    /// Cualquier otro submenu, ya decidido entero.
    Submenu {
        etiqueta: String,
        opciones: Vec<Option<OpcionSubmenu>>,
    },
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
}

/// El submenu «Convertir en…» de una nota: todas las herramientas.
fn submenu_convertir(t: &TextosPin) -> EntradaMenu {
    EntradaMenu::Submenu {
        etiqueta: t.convertir_en.clone(),
        opciones: t
            .herramientas
            .iter()
            .enumerate()
            .map(|(i, nombre)| {
                Some(OpcionSubmenu {
                    id: CMD_CONVERTIR_BASE + i as u32,
                    etiqueta: nombre.clone(),
                    marcada: false,
                })
            })
            .collect(),
    }
}

/// El submenu del fondo de la pizarra: los cuatro colores, una raya y las
/// cinco pautas, con lo de ahora marcado. En el movil es una paleta que se
/// queda abierta; aqui, un menu nativo que se vuelve a abrir, que es lo
/// que se espera de un escritorio.
fn submenu_pizarra(t: &TextosPin, color: u8, pauta: u8) -> EntradaMenu {
    let mut opciones: Vec<Option<OpcionSubmenu>> = t
        .colores_pizarra
        .iter()
        .enumerate()
        .map(|(i, nombre)| {
            Some(OpcionSubmenu {
                id: CMD_PIZARRA_COLOR_BASE + i as u32,
                etiqueta: nombre.clone(),
                marcada: i as u8 == color,
            })
        })
        .collect();
    opciones.push(None);
    opciones.extend(t.pautas_pizarra.iter().enumerate().map(|(i, nombre)| {
        Some(OpcionSubmenu {
            id: CMD_PIZARRA_PAUTA_BASE + i as u32,
            etiqueta: nombre.clone(),
            marcada: i as u8 == pauta,
        })
    }));
    EntradaMenu::Submenu {
        etiqueta: t.fondo_pizarra.clone(),
        opciones,
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
    } = estado;
    let mut v = Vec::new();

    // El pin en vivo tiene menu propio y corto. No hay entrada en el
    // almacen detras —la zona no se guarda hasta congelarla—, asi que nada
    // de grupos, guardar, eliminar ni leer texto: prometerlos seria
    // ofrecer algo sin fichero sobre el que hacerlo.
    if let Contenido::Vivo { .. } = contenido {
        v.push(EntradaMenu::Accion {
            id: CMD_REPRODUCIR,
            etiqueta: if reproduciendo {
                t.pausar.clone()
            } else {
                t.reproducir.clone()
            },
        });
        v.push(EntradaMenu::Accion {
            id: CMD_CONGELAR,
            etiqueta: t.congelar.clone(),
        });
        // Arriba, con la pausa: es lo que distingue a este pin de una foto,
        // y el unico sitio por donde se apaga (con el modo encendido el clic
        // izquierdo ya no es del pin).
        v.push(EntradaMenu::Accion {
            id: CMD_REMOTO,
            etiqueta: if remoto {
                t.dejar_de_manejar.clone()
            } else {
                t.manejar.clone()
            },
        });
        v.push(EntradaMenu::Separador);
        v.push(EntradaMenu::Accion {
            id: CMD_COPIAR,
            etiqueta: t.copiar.clone(),
        });
        v.push(EntradaMenu::Accion {
            id: CMD_TAMANO_ORIGINAL,
            etiqueta: t.tamano_original.clone(),
        });
        if !pasante {
            v.push(EntradaMenu::Separador);
            v.push(EntradaMenu::Accion {
                id: CMD_PASANTE,
                etiqueta: t.dejar_pasar_clic.clone(),
            });
        }
        v.push(EntradaMenu::Separador);
        v.push(EntradaMenu::Accion {
            id: CMD_CERRAR,
            etiqueta: t.cerrar.clone(),
        });
        return v;
    }

    // El video lleva sus controles ARRIBA: son lo que se busca al abrir el
    // menu de un video (D68).
    if let Contenido::Video { .. } = contenido {
        v.push(EntradaMenu::Accion {
            id: CMD_REPRODUCIR,
            etiqueta: if reproduciendo {
                t.pausar.clone()
            } else {
                t.reproducir.clone()
            },
        });
        v.push(EntradaMenu::Accion {
            id: CMD_SONIDO,
            etiqueta: t.sonido.clone(),
        });
        v.push(EntradaMenu::Separador);
    }

    // Las paginas del PDF van ARRIBA, como las del video: es lo que se
    // busca al abrir el menu de un documento de varias paginas.
    if let Some(cuantas) = paginas.filter(|c| *c > 1) {
        if pagina + 1 < cuantas {
            v.push(EntradaMenu::Accion {
                id: CMD_PAGINA_SIGUIENTE,
                etiqueta: t.pagina_siguiente.clone(),
            });
        }
        if pagina > 0 {
            v.push(EntradaMenu::Accion {
                id: CMD_PAGINA_ANTERIOR,
                etiqueta: t.pagina_anterior.clone(),
            });
        }
    }
    // Extraer se ofrece aunque solo haya una pagina: sacarla como imagen
    // propia para anotarla o copiarla es util igual.
    if paginas.is_some() {
        v.push(EntradaMenu::Accion {
            id: CMD_EXTRAER_PAGINA,
            etiqueta: t.extraer_pagina.clone(),
        });
        if paginas.is_some_and(|c| c > 1) {
            v.push(EntradaMenu::Accion {
                id: CMD_EXTRAER_TODAS,
                etiqueta: t.extraer_todas.clone(),
            });
        }
        v.push(EntradaMenu::Separador);
    }

    v.push(EntradaMenu::Accion {
        id: CMD_COPIAR,
        etiqueta: t.copiar.clone(),
    });

    // Leer el texto va detras de «Copiar», que es su pariente: los dos
    // copian, uno la imagen y otro lo que pone en ella. Solo en lo que ES
    // una imagen —en una nota el texto ya lo tienes, y de un archivo por
    // referencia no hay pixeles que leer— y solo si el equipo sabe
    // reconocer texto: ofrecerlo sin motor daria un error en vez de una
    // funcion.
    if con_ocr
        && contenido.redimensionable()
        && !matches!(
            contenido,
            Contenido::Nota { .. } | Contenido::Herramienta { .. }
        )
    {
        v.push(EntradaMenu::Accion {
            id: CMD_TEXTO,
            etiqueta: t.copiar_texto.clone(),
        });
    }

    // Abrir en el lienzo va con las acciones de la imagen: dibujar sin el
    // borde del pin. El doble clic sigue anotando dentro del pin.
    if matches!(contenido, Contenido::Imagen(_)) {
        v.push(EntradaMenu::Accion {
            id: CMD_ABRIR_LIENZO,
            etiqueta: t.abrir_en_lienzo.clone(),
        });
        // El fondo de la pizarra, junto al lienzo: los dos cambian el papel
        // sobre el que se dibuja.
        if let Some((color, pauta)) = pizarra {
            v.push(submenu_pizarra(t, color, pauta));
        }
    }

    // Una nota se convierte en herramienta: sus lineas pasan a ser las
    // tareas, los gastos o los nombres de la ruleta (C3, propio del PC).
    if matches!(contenido, Contenido::Nota { .. }) {
        v.push(submenu_convertir(t));
    }

    match contenido {
        // Un archivo no se «guarda como»: ya es un fichero del usuario y
        // esta donde el lo dejo. Lo util es llegar hasta el. El video y el
        // documento son archivos por referencia igual (D65).
        Contenido::Archivo { .. } | Contenido::Video { .. } | Contenido::Documento { .. } => v
            .push(EntradaMenu::Accion {
                id: CMD_ABRIR_UBICACION,
                etiqueta: t.abrir_ubicacion.clone(),
            }),
        _ => {
            v.push(EntradaMenu::Accion {
                id: CMD_GUARDAR_COMO,
                etiqueta: t.guardar_como.clone(),
            });
            // La nota no se redimensiona: «Tamaño original» no diria nada.
            if contenido.redimensionable() {
                v.push(EntradaMenu::Accion {
                    id: CMD_TAMANO_ORIGINAL,
                    etiqueta: t.tamano_original.clone(),
                });
            }
        }
    }

    // «Dejar pasar el clic» solo se ofrece si NO lo esta ya: estando
    // pasante, este menu ni siquiera se abre, porque el clic derecho pasa
    // de largo. Ofrecer una entrada para desactivarlo seria prometer algo
    // a lo que no se puede llegar.
    if !pasante {
        v.push(EntradaMenu::Separador);
        v.push(EntradaMenu::Accion {
            id: CMD_PASANTE,
            etiqueta: t.dejar_pasar_clic.clone(),
        });
    }

    v.push(EntradaMenu::Separador);
    v.push(EntradaMenu::SubmenuGrupo);
    if con_grupo {
        v.push(EntradaMenu::Accion {
            id: CMD_OCULTAR_GRUPO,
            etiqueta: t.ocultar_grupo.clone(),
        });
    }

    v.push(EntradaMenu::Separador);
    v.push(EntradaMenu::Accion {
        id: CMD_CERRAR,
        etiqueta: t.cerrar.clone(),
    });
    v.push(EntradaMenu::Accion {
        id: CMD_ELIMINAR,
        etiqueta: t.eliminar.clone(),
    });
    v
}

/// Muestra el menu donde este el raton y devuelve el comando elegido, o
/// `None` si se cerro sin elegir.
///
/// El patron es el de la bandeja de S1-A, con sus dos trampas: el menu se
/// destruye SIEMPRE (por eso el cierre intermedio), y `SetForegroundWindow`
/// va antes de `TrackPopupMenu` o el menu no se cierra al pulsar fuera.
pub fn mostrar(
    hwnd: windows::Win32::Foundation::HWND,
    contenido: &Contenido,
    estado: EstadoMenu,
    t: &TextosPin,
) -> Option<u32> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, MF_CHECKED, MF_POPUP,
        MF_SEPARATOR, MF_STRING,
        SetForegroundWindow, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
    };
    use windows::core::HSTRING;

    // SAFETY: todo lo que se crea aqui se destruye antes de salir; las
    // cadenas viajan como HSTRING, que gestiona su propia memoria durante
    // la llamada, y el hwnd es la ventana propia del pin.
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        let submenu = match CreatePopupMenu() {
            Ok(s) => s,
            Err(_) => {
                let _ = DestroyMenu(menu);
                return None;
            }
        };

        let armado = (|| -> windows::core::Result<()> {
            AppendMenuW(
                submenu,
                MF_STRING,
                CMD_SIN_GRUPO as usize,
                &HSTRING::from(t.sin_grupo.as_str()),
            )?;
            for (i, nombre) in t.colores.iter().enumerate() {
                AppendMenuW(
                    submenu,
                    MF_STRING,
                    (CMD_COLOR_BASE + i as u32) as usize,
                    &HSTRING::from(nombre.as_str()),
                )?;
            }
            for entrada in entradas_del_menu(contenido, estado, t) {
                match entrada {
                    EntradaMenu::Separador => AppendMenuW(menu, MF_SEPARATOR, 0, None)?,
                    EntradaMenu::SubmenuGrupo => AppendMenuW(
                        menu,
                        MF_POPUP,
                        submenu.0 as usize,
                        &HSTRING::from(t.grupo.as_str()),
                    )?,
                    EntradaMenu::Accion { id, etiqueta } => AppendMenuW(
                        menu,
                        MF_STRING,
                        id as usize,
                        &HSTRING::from(etiqueta.as_str()),
                    )?,
                    EntradaMenu::Submenu { etiqueta, opciones } => {
                        // Se cuelga del padre en cuanto se crea: desde ese
                        // momento lo destruye el `DestroyMenu` de abajo, y
                        // un fallo a medias no deja un menu huerfano.
                        let hijo = CreatePopupMenu()?;
                        AppendMenuW(
                            menu,
                            MF_POPUP,
                            hijo.0 as usize,
                            &HSTRING::from(etiqueta.as_str()),
                        )?;
                        for o in opciones {
                            match o {
                                None => AppendMenuW(hijo, MF_SEPARATOR, 0, None)?,
                                Some(o) => AppendMenuW(
                                    hijo,
                                    if o.marcada {
                                        MF_STRING | MF_CHECKED
                                    } else {
                                        MF_STRING
                                    },
                                    o.id as usize,
                                    &HSTRING::from(o.etiqueta.as_str()),
                                )?,
                            }
                        }
                    }
                }
            }
            Ok(())
        })();

        let elegido = if armado.is_ok() {
            let mut punto = POINT::default();
            let _ = GetCursorPos(&mut punto);
            // Sin esto el menu se queda abierto al pulsar fuera. Requisito
            // documentado de TrackPopupMenu que se olvida siempre.
            let _ = SetForegroundWindow(hwnd);
            let r = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_LEFTALIGN | TPM_RIGHTBUTTON,
                punto.x,
                punto.y,
                None,
                hwnd,
                None,
            );
            if r.0 == 0 { None } else { Some(r.0 as u32) }
        } else {
            None
        };

        // El submenu se destruye con el padre; destruirlo aparte seria un
        // doble libre.
        let _ = DestroyMenu(menu);
        elegido
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_codec::ImagenRgba;

    fn textos() -> TextosPin {
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
        }
    }

    fn submenu<'a>(v: &'a [EntradaMenu], etiqueta: &str) -> Option<&'a [Option<OpcionSubmenu>]> {
        v.iter().find_map(|e| match e {
            EntradaMenu::Submenu {
                etiqueta: t,
                opciones,
            } if t == etiqueta => Some(opciones.as_slice()),
            _ => None,
        })
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
            Some(OpcionSubmenu {
                id: CMD_CONVERTIR_BASE + 2,
                etiqueta: "Tareas".into(),
                marcada: false
            })
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
        let marcadas: Vec<u32> = s.iter().flatten().filter(|o| o.marcada).map(|o| o.id).collect();
        assert_eq!(marcadas, vec![CMD_PIZARRA_COLOR_BASE + 2, CMD_PIZARRA_PAUTA_BASE + 4]);
        assert_eq!(s.iter().filter(|o| o.is_none()).count(), 1, "una raya entre las dos");
        // Caso negativo: una imagen corriente no es una pizarra.
        let normal = entradas_del_menu(&imagen(), EstadoMenu::default(), &textos());
        assert!(submenu(&normal, "Fondo de la pizarra").is_none());
    }

    #[test]
    fn una_herramienta_no_ofrece_leer_su_texto_ni_convertirse() {
        let h = Contenido::Herramienta {
            ancho: 300,
            alto: 300,
        };
        let v = entradas_del_menu(
            &h,
            EstadoMenu {
                con_ocr: true,
                ..Default::default()
            },
            &textos(),
        );
        let ids = ids(&v);
        assert!(ids.contains(&CMD_COPIAR) && ids.contains(&CMD_CERRAR));
        assert!(!ids.contains(&CMD_TEXTO));
        assert!(!ids.contains(&CMD_ABRIR_LIENZO));
        assert!(submenu(&v, "Convertir en").is_none());
    }

    #[test]
    fn el_pin_en_vivo_ofrece_manejar_y_encendido_ofrece_dejarlo() {
        let vivo = Contenido::Vivo {
            ancho: 300,
            alto: 200,
        };
        let etiqueta = |remoto: bool| {
            entradas_del_menu(
                &vivo,
                EstadoMenu {
                    remoto,
                    ..Default::default()
                },
                &textos(),
            )
            .into_iter()
            .find_map(|e| match e {
                EntradaMenu::Accion { id, etiqueta } if id == CMD_REMOTO => Some(etiqueta),
                _ => None,
            })
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
        let imagen = Contenido::Imagen(ImagenRgba {
            ancho: 1,
            alto: 1,
            pixeles: vec![0, 0, 0, 255],
        });
        let e = entradas_del_menu(&imagen, EstadoMenu::default(), &textos());
        assert!(!ids(&e).contains(&CMD_REMOTO));
    }

    #[test]
    fn el_pin_en_vivo_ofrece_pausar_congelar_copiar_y_cerrar_arriba_la_pausa() {
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
        assert_eq!(
            e[0],
            EntradaMenu::Accion {
                id: CMD_REPRODUCIR,
                etiqueta: "Pausar".into()
            },
            "corriendo, lo primero es pausar"
        );
        let ids = ids(&e);
        for esperado in [CMD_CONGELAR, CMD_COPIAR, CMD_TAMANO_ORIGINAL, CMD_CERRAR] {
            assert!(ids.contains(&esperado), "falta {esperado}");
        }
        // Caso negativo: sin entrada en el almacen no hay nada que
        // eliminar, guardar, agrupar ni leer.
        for ausente in [CMD_ELIMINAR, CMD_GUARDAR_COMO, CMD_TEXTO, CMD_ABRIR_LIENZO] {
            assert!(!ids.contains(&ausente), "sobra {ausente}");
        }
        assert!(!e.contains(&EntradaMenu::SubmenuGrupo));
    }

    #[test]
    fn el_pin_en_vivo_en_pausa_ofrece_reanudar() {
        let vivo = Contenido::Vivo {
            ancho: 300,
            alto: 200,
        };
        let e = entradas_del_menu(&vivo, EstadoMenu::default(), &textos());
        assert_eq!(
            e[0],
            EntradaMenu::Accion {
                id: CMD_REPRODUCIR,
                etiqueta: "Reproducir".into()
            }
        );
    }

    fn video() -> Contenido {
        Contenido::Video {
            nombre: "clip.mp4".into(),
            ruta: std::path::PathBuf::from("clip.mp4"),
            ancho: 0,
            alto: 0,
        }
    }

    #[test]
    fn el_video_tiene_reproducir_y_sonido_arriba_y_no_guardar_como() {
        let v = entradas_del_menu(
            &video(),
            EstadoMenu {
                con_grupo: false,
                reproduciendo: true,
                pasante: false,
                con_ocr: true,
                ..Default::default()
            },
            &textos(),
        );
        assert_eq!(
            v[0],
            EntradaMenu::Accion {
                id: CMD_REPRODUCIR,
                etiqueta: "Pausar".into()
            },
            "reproduciendo, la primera entrada ofrece pausar"
        );
        assert_eq!(
            v[1],
            EntradaMenu::Accion {
                id: CMD_SONIDO,
                etiqueta: "Sonido".into()
            }
        );
        let ids = ids(&v);
        assert!(ids.contains(&CMD_ABRIR_UBICACION));
        // Caso negativo: un video es un archivo del usuario, no se «guarda
        // como» ni tiene «tamano original» de pixeles hasta que se conoce.
        assert!(!ids.contains(&CMD_GUARDAR_COMO));
        assert!(!ids.contains(&CMD_TAMANO_ORIGINAL));

        let parado = entradas_del_menu(
            &video(),
            EstadoMenu {
                con_grupo: false,
                reproduciendo: false,
                pasante: false,
                con_ocr: true,
                ..Default::default()
            },
            &textos(),
        );
        assert_eq!(
            parado[0],
            EntradaMenu::Accion {
                id: CMD_REPRODUCIR,
                etiqueta: "Reproducir".into()
            }
        );
    }

    #[test]
    fn el_documento_abre_ubicacion_como_la_ficha_y_no_tiene_controles() {
        let d = Contenido::Documento {
            nombre: "informe.pdf".into(),
            vista: ImagenRgba {
                ancho: 1,
                alto: 1,
                pixeles: vec![0; 4],
            },
        };
        let ids = ids(&entradas_del_menu(
            &d,
            EstadoMenu {
                con_grupo: false,
                reproduciendo: false,
                pasante: false,
                con_ocr: true,
                ..Default::default()
            },
            &textos(),
        ));
        assert!(ids.contains(&CMD_ABRIR_UBICACION));
        assert!(!ids.contains(&CMD_REPRODUCIR));
        assert!(!ids.contains(&CMD_SONIDO));
        assert!(!ids.contains(&CMD_GUARDAR_COMO));
    }

    fn ids(v: &[EntradaMenu]) -> Vec<u32> {
        v.iter()
            .filter_map(|e| match e {
                EntradaMenu::Accion { id, .. } => Some(*id),
                _ => None,
            })
            .collect()
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

    #[test]
    fn una_imagen_sin_grupo_no_ofrece_ocultar_ni_abrir_ubicacion() {
        let v = entradas_del_menu(
            &imagen(),
            EstadoMenu {
                con_grupo: false,
                reproduciendo: false,
                pasante: false,
                con_ocr: true,
                ..Default::default()
            },
            &textos(),
        );
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
    fn una_nota_se_guarda_y_tiene_tamano_original() {
        // La nota se estira por la esquina y se escala con la rueda; el
        // doble clic ("Tamaño original") la devuelve a como nacio.
        let nota = Contenido::Nota { texto: "x".into() };
        let v = entradas_del_menu(
            &nota,
            EstadoMenu {
                con_grupo: false,
                reproduciendo: false,
                pasante: false,
                con_ocr: true,
                ..Default::default()
            },
            &textos(),
        );
        let ids = ids(&v);
        assert!(ids.contains(&CMD_GUARDAR_COMO));
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
                reproduciendo: false,
                pasante: false,
                con_ocr: true,
                ..Default::default()
            },
            &textos(),
        );
        assert!(ids(&v).contains(&CMD_OCULTAR_GRUPO));
    }

    #[test]
    fn un_archivo_ofrece_su_ubicacion_y_no_tamano_original() {
        // Caso negativo del tipo: «Tamaño original» sobre una ficha no
        // significa nada, y «Guardar como» duplicaria un fichero que ya
        // existe donde el usuario lo puso.
        let v = entradas_del_menu(
            &archivo(),
            EstadoMenu {
                con_grupo: false,
                reproduciendo: false,
                pasante: false,
                con_ocr: true,
                ..Default::default()
            },
            &textos(),
        );
        let ids = ids(&v);
        assert!(ids.contains(&CMD_ABRIR_UBICACION));
        assert!(!ids.contains(&CMD_TAMANO_ORIGINAL));
        assert!(!ids.contains(&CMD_GUARDAR_COMO));
    }

    #[test]
    fn leer_el_texto_solo_se_ofrece_donde_hay_pixeles_y_motor() {
        // En una imagen si: es para lo que sirve.
        let con = ids(&entradas_del_menu(
            &imagen(),
            EstadoMenu {
                con_grupo: false,
                reproduciendo: false,
                pasante: false,
                con_ocr: true,
                ..Default::default()
            },
            &textos(),
        ));
        assert!(con.contains(&CMD_TEXTO));
        // Caso negativo, el importante: sin motor de reconocimiento NO se
        // ofrece. Ofrecerlo en un equipo sin idiomas instalados daria un
        // error en vez de una funcion, y el usuario no sabria por que.
        let sin = ids(&entradas_del_menu(
            &imagen(),
            EstadoMenu {
                con_grupo: false,
                reproduciendo: false,
                pasante: false,
                con_ocr: false,
                ..Default::default()
            },
            &textos(),
        ));
        assert!(!sin.contains(&CMD_TEXTO));
        // Y en una nota tampoco: el texto ya lo tienes escrito, leerlo de
        // sus pixeles seria dar un rodeo para llegar a lo mismo peor.
        let nota = Contenido::Nota {
            texto: "hola".into(),
        };
        let en_nota = ids(&entradas_del_menu(
            &nota,
            EstadoMenu {
                con_grupo: false,
                reproduciendo: false,
                pasante: false,
                con_ocr: true,
                ..Default::default()
            },
            &textos(),
        ));
        assert!(!en_nota.contains(&CMD_TEXTO));
    }

    #[test]
    fn un_pin_pasante_no_ofrece_volver_a_dejarlo_pasar() {
        // Estando pasante, este menu ni siquiera se abre: el clic derecho
        // pasa de largo hacia lo que hay debajo. Ofrecer la entrada seria
        // prometer algo a lo que no se puede llegar, asi que solo esta
        // cuando sirve de algo. La vuelta es por el comando global.
        let normal = ids(&entradas_del_menu(
            &imagen(),
            EstadoMenu {
                con_grupo: false,
                reproduciendo: false,
                pasante: false,
                con_ocr: true,
                ..Default::default()
            },
            &textos(),
        ));
        assert!(normal.contains(&CMD_PASANTE));
        let pasante = ids(&entradas_del_menu(
            &imagen(),
            EstadoMenu {
                con_grupo: false,
                reproduciendo: false,
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
    fn todos_los_menus_ofrecen_copiar_cerrar_y_eliminar() {
        for (c, grupo) in [(imagen(), false), (archivo(), true)] {
            let ids = ids(&entradas_del_menu(
                &c,
                EstadoMenu {
                    con_grupo: grupo,
                    reproduciendo: false,
                    pasante: false,
                    con_ocr: true,
                    ..Default::default()
                },
                &textos(),
            ));
            for esperado in [CMD_COPIAR, CMD_CERRAR, CMD_ELIMINAR] {
                assert!(ids.contains(&esperado), "falta la entrada {esperado}");
            }
        }
    }

    #[test]
    fn abrir_en_lienzo_solo_esta_en_los_pines_de_imagen() {
        let estado = EstadoMenu {
            con_ocr: true,
            ..Default::default()
        };
        assert!(ids(&entradas_del_menu(&imagen(), estado, &textos())).contains(&CMD_ABRIR_LIENZO));
        // Casos negativos: una nota, una ficha, un video o un documento no
        // tienen pixeles propios que poner de fondo.
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
                !ids(&entradas_del_menu(&c, estado, &textos())).contains(&CMD_ABRIR_LIENZO),
                "{c:?}"
            );
        }
    }

    #[test]
    fn el_submenu_de_grupo_esta_siempre() {
        let v = entradas_del_menu(
            &imagen(),
            EstadoMenu {
                con_grupo: false,
                reproduciendo: false,
                pasante: false,
                con_ocr: true,
                ..Default::default()
            },
            &textos(),
        );
        assert!(v.contains(&EntradaMenu::SubmenuGrupo));
    }
}
