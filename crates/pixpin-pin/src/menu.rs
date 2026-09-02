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

/// Que entradas tiene el menu de este pin. PURA y probada en CI.
pub fn entradas_del_menu(
    contenido: &Contenido,
    con_grupo: bool,
    t: &TextosPin,
) -> Vec<EntradaMenu> {
    let mut v = vec![EntradaMenu::Accion {
        id: CMD_COPIAR,
        etiqueta: t.copiar.clone(),
    }];

    match contenido {
        // Un archivo no se «guarda como»: ya es un fichero del usuario y
        // esta donde el lo dejo. Lo util es llegar hasta el.
        Contenido::Archivo { .. } => v.push(EntradaMenu::Accion {
            id: CMD_ABRIR_UBICACION,
            etiqueta: t.abrir_ubicacion.clone(),
        }),
        _ => {
            v.push(EntradaMenu::Accion {
                id: CMD_GUARDAR_COMO,
                etiqueta: t.guardar_como.clone(),
            });
            v.push(EntradaMenu::Accion {
                id: CMD_TAMANO_ORIGINAL,
                etiqueta: t.tamano_original.clone(),
            });
        }
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
    con_grupo: bool,
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
            for entrada in entradas_del_menu(contenido, con_grupo, t) {
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
        }
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
        let v = entradas_del_menu(&imagen(), false, &textos());
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
    fn una_imagen_con_grupo_si_ofrece_ocultarlo() {
        let v = entradas_del_menu(&imagen(), true, &textos());
        assert!(ids(&v).contains(&CMD_OCULTAR_GRUPO));
    }

    #[test]
    fn un_archivo_ofrece_su_ubicacion_y_no_tamano_original() {
        // Caso negativo del tipo: «Tamaño original» sobre una ficha no
        // significa nada, y «Guardar como» duplicaria un fichero que ya
        // existe donde el usuario lo puso.
        let v = entradas_del_menu(&archivo(), false, &textos());
        let ids = ids(&v);
        assert!(ids.contains(&CMD_ABRIR_UBICACION));
        assert!(!ids.contains(&CMD_TAMANO_ORIGINAL));
        assert!(!ids.contains(&CMD_GUARDAR_COMO));
    }

    #[test]
    fn todos_los_menus_ofrecen_copiar_cerrar_y_eliminar() {
        for (c, grupo) in [(imagen(), false), (archivo(), true)] {
            let ids = ids(&entradas_del_menu(&c, grupo, &textos()));
            for esperado in [CMD_COPIAR, CMD_CERRAR, CMD_ELIMINAR] {
                assert!(ids.contains(&esperado), "falta la entrada {esperado}");
            }
        }
    }

    #[test]
    fn el_submenu_de_grupo_esta_siempre() {
        let v = entradas_del_menu(&imagen(), false, &textos());
        assert!(v.contains(&EntradaMenu::SubmenuGrupo));
    }
}
