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
pub const CMD_SIN_GRUPO: u32 = 100;
pub const CMD_COLOR_BASE: u32 = 101;

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
    } = estado;
    let mut v = Vec::new();

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
    if con_ocr && contenido.redimensionable() && !matches!(contenido, Contenido::Nota { .. }) {
        v.push(EntradaMenu::Accion {
            id: CMD_TEXTO,
            etiqueta: t.copiar_texto.clone(),
        });
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
        AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, MF_POPUP, MF_SEPARATOR, MF_STRING,
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
            pagina_siguiente: "Pagina siguiente".into(),
            pagina_anterior: "Pagina anterior".into(),
            extraer_pagina: "Extraer esta pagina".into(),
            extraer_todas: "Extraer todas las paginas".into(),
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
