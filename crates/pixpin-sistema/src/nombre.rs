//! Los nombres de la app en disco: los de ahora (PinDesk) y los de antes
//! (PixPin Max), y el paso de unos a otros.
//!
//! La regla de la transicion (docs/formato-pindesk.md): **se escribe con el
//! nombre nuevo, se lee tambien el viejo y se migra una vez**. Aqui viven las
//! carpetas de datos, el fichero de ajustes y la migracion que hace la app al
//! arrancar; los ayudantes (el MCP, el puente, el lanzador) solo usan
//! [`carpeta_de_datos`] y [`ajustes_en`], que miran la nueva y despues la
//! vieja, y nunca mueven nada.
//!
//! Todo lo de este modulo es puro o toca solo las rutas que se le pasan, para
//! probarlo con carpetas temporales.

use std::path::{Path, PathBuf};

/// La carpeta de datos dentro de `%APPDATA%` y de `%LOCALAPPDATA%`.
pub const CARPETA_DATOS: &str = "PinDesk";
/// La carpeta de antes de [`CARPETA_DATOS`] (hasta nov-2026).
pub const CARPETA_DATOS_VIEJA: &str = "PixPinMax";
/// El fichero de ajustes. Junto al exe, activa el modo portable.
pub const AJUSTES: &str = "pindesk.toml";
/// El de antes de [`AJUSTES`]: tambien activa el modo portable.
pub const AJUSTES_VIEJO: &str = "pixpinmax.toml";
/// La nota que queda en la carpeta vieja tras moverla.
pub const AVISO_MOVIDO: &str = "MOVIDO-A-PINDESK.txt";

/// La carpeta de datos bajo `base` (`%APPDATA%` o `%LOCALAPPDATA%`): la nueva
/// si existe; si no, la vieja si existe y aun tiene datos (no es solo la nota
/// de [`AVISO_MOVIDO`]); si no, la nueva. Asi un ayudante que corre antes de
/// que la app migre, o cuando la migracion no pudo hacerse, ve los datos de
/// siempre.
pub fn carpeta_de_datos(base: &Path) -> PathBuf {
    let nueva = base.join(CARPETA_DATOS);
    let vieja = base.join(CARPETA_DATOS_VIEJA);
    if !nueva.is_dir() && vieja.is_dir() && !vieja.join(AVISO_MOVIDO).is_file() {
        vieja
    } else {
        nueva
    }
}

/// `%LOCALAPPDATA%\PinDesk` (o la vieja, con la regla de
/// [`carpeta_de_datos`]): claves cifradas, modelos, la clave de los enlaces.
pub fn carpeta_local() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").filter(|l| !l.is_empty())?;
    Some(carpeta_de_datos(Path::new(&local)))
}

/// El fichero de ajustes de `dir`: `pindesk.toml` si esta; si no, el
/// `pixpinmax.toml` de antes si esta; si no, `pindesk.toml` (el que se crea).
pub fn ajustes_en(dir: &Path) -> PathBuf {
    let nuevo = dir.join(AJUSTES);
    let viejo = dir.join(AJUSTES_VIEJO);
    if !nuevo.is_file() && viejo.is_file() {
        viejo
    } else {
        nuevo
    }
}

/// Si junto al exe hay un fichero de ajustes, nuevo o viejo: modo portable.
pub fn es_portable(dir_exe: &Path) -> bool {
    dir_exe.join(AJUSTES).is_file() || dir_exe.join(AJUSTES_VIEJO).is_file()
}

/// Lo que paso con una carpeta al migrarla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Migracion {
    /// No habia carpeta vieja, o ya existia la nueva: no se toco nada.
    NadaQueHacer,
    /// Se renombro (lo normal: la misma unidad).
    Movida,
    /// No se pudo renombrar y se copio; la vieja se queda, con la nota.
    Copiada,
    /// No se pudo ni copiar. La vieja sigue intacta y se seguira usando.
    Fallo(String),
}

/// El texto de la nota que queda en la carpeta vieja.
pub fn texto_del_aviso(nueva: &Path, copiada: bool) -> String {
    let que = if copiada {
        "Se copiaron a la carpeta nueva; esta copia ya no se usa y se puede borrar \
         cuando compruebes que en PinDesk esta todo."
    } else {
        "Se movieron a la carpeta nueva."
    };
    format!(
        "La app ahora se llama PinDesk (antes PixPin Max).\r\n\
         Sus datos estan en:\r\n\r\n    {}\r\n\r\n{que}\r\n",
        nueva.display()
    )
}

/// Pasa `vieja` a `nueva` una sola vez. Nunca borra nada de `vieja` si algo
/// falla:
///
/// 1. Si `nueva` ya existe o `vieja` no, no hace nada.
/// 2. Intenta renombrar (instantaneo, en la misma unidad). Si sale, vuelve a
///    crear `vieja` solo con la nota [`AVISO_MOVIDO`].
/// 3. Si no (otra unidad, un fichero abierto por otro programa), copia todo a
///    `nueva.migrando` y, completa la copia, la renombra a `nueva` y deja la
///    nota en `vieja`, que conserva sus datos. Si la copia falla, se borra
///    solo la copia a medias.
///
/// Dentro de la carpeta ya migrada, el `pixpinmax.toml` pasa a `pindesk.toml`
/// ([`migrar_ajustes`]).
pub fn migrar_carpeta(vieja: &Path, nueva: &Path) -> Migracion {
    if nueva.exists() || !vieja.is_dir() || vieja.join(AVISO_MOVIDO).is_file() {
        return Migracion::NadaQueHacer;
    }
    if let Some(padre) = nueva.parent() {
        let _ = std::fs::create_dir_all(padre);
    }
    let resultado = if std::fs::rename(vieja, nueva).is_ok() {
        let _ = std::fs::create_dir_all(vieja);
        let _ = std::fs::write(vieja.join(AVISO_MOVIDO), texto_del_aviso(nueva, false));
        Migracion::Movida
    } else {
        let mut temporal = nueva.as_os_str().to_owned();
        temporal.push(".migrando");
        let temporal = PathBuf::from(temporal);
        let _ = std::fs::remove_dir_all(&temporal);
        match copiar_arbol(vieja, &temporal).and_then(|()| std::fs::rename(&temporal, nueva)) {
            Ok(()) => {
                let _ = std::fs::write(vieja.join(AVISO_MOVIDO), texto_del_aviso(nueva, true));
                Migracion::Copiada
            }
            Err(e) => {
                let _ = std::fs::remove_dir_all(&temporal);
                return Migracion::Fallo(format!(
                    "{} -> {}: {e}",
                    vieja.display(),
                    nueva.display()
                ));
            }
        }
    };
    let _ = migrar_ajustes(nueva);
    resultado
}

/// Renombra el `pixpinmax.toml` de `dir` a `pindesk.toml` si no hay ya uno
/// nuevo. Devuelve si lo hizo. Sirve para la carpeta de datos y para la
/// carpeta del exe en modo portable.
pub fn migrar_ajustes(dir: &Path) -> std::io::Result<bool> {
    let nuevo = dir.join(AJUSTES);
    let viejo = dir.join(AJUSTES_VIEJO);
    if nuevo.exists() || !viejo.is_file() {
        return Ok(false);
    }
    std::fs::rename(&viejo, &nuevo)?;
    Ok(true)
}

/// Copia `de` dentro de `a` (que no existe), carpetas y ficheros. No sigue
/// los enlaces ni las uniones: los recrea apuntando a lo mismo cuando puede
/// y, si no, los salta (en la carpeta de datos son atajos a carpetas del
/// usuario, que siguen donde estaban).
fn copiar_arbol(de: &Path, a: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(a)?;
    for entrada in std::fs::read_dir(de)? {
        let entrada = entrada?;
        let tipo = entrada.file_type()?;
        let origen = entrada.path();
        let destino = a.join(entrada.file_name());
        if tipo.is_symlink() || crate::union::es_union(&origen) {
            recrear_enlace(&origen, &destino);
        } else if tipo.is_dir() {
            copiar_arbol(&origen, &destino)?;
        } else {
            std::fs::copy(&origen, &destino)?;
        }
    }
    Ok(())
}

fn recrear_enlace(origen: &Path, destino: &Path) {
    let Some(apunta) = crate::union::destino(origen).or_else(|| std::fs::read_link(origen).ok())
    else {
        return;
    };
    let _ = crate::union::crear(destino, &apunta);
}

/// La migracion de arranque de la app de Windows: `%APPDATA%\PixPinMax` y
/// `%LOCALAPPDATA%\PixPinMax` a `PinDesk`, y el `pixpinmax.toml` del exe en
/// modo portable. Devuelve que paso con cada una, para el registro.
pub fn migrar_al_arrancar(
    appdata: &Path,
    local: Option<&Path>,
    dir_exe: &Path,
) -> Vec<(PathBuf, Migracion)> {
    let mut hechas = Vec::new();
    if dir_exe.join(AJUSTES_VIEJO).is_file() {
        let m = match migrar_ajustes(dir_exe) {
            Ok(true) => Migracion::Movida,
            Ok(false) => Migracion::NadaQueHacer,
            Err(e) => Migracion::Fallo(e.to_string()),
        };
        hechas.push((dir_exe.join(AJUSTES), m));
    } else if !dir_exe.join(AJUSTES).is_file() {
        let nueva = appdata.join(CARPETA_DATOS);
        let vieja = appdata.join(CARPETA_DATOS_VIEJA);
        let m = migrar_carpeta(&vieja, &nueva);
        if matches!(m, Migracion::Movida | Migracion::Copiada) {
            dejar_paso_a_la_impresora(&vieja, &nueva);
        }
        hechas.push((nueva.clone(), m));
        // Una carpeta ya migrada a mano puede traer aun el toml viejo.
        let _ = migrar_ajustes(&nueva);
    }
    if let Some(l) = local {
        let nueva = l.join(CARPETA_DATOS);
        hechas.push((
            nueva.clone(),
            migrar_carpeta(&l.join(CARPETA_DATOS_VIEJA), &nueva),
        ));
    }
    hechas
}

/// La impresora «PixPin» de antes escribe en
/// `PixPinMax\cache\impresora\PixPin.pdf`, una ruta fija de Windows que no
/// se puede cambiar sin administrador. Tras la mudanza esa carpeta pasa a ser
/// una union a la nueva, para que lo impreso siga llegando a la app hasta
/// que se ponga la impresora «PinDesk» (`impresora-pindesk.ps1`).
fn dejar_paso_a_la_impresora(vieja: &Path, nueva: &Path) {
    let de = vieja.join("cache").join("impresora");
    let a = nueva.join("cache").join("impresora");
    if !a.is_dir() {
        return;
    }
    // Copiada: la vieja aun tiene su carpeta (es cache: se puede quitar).
    if de.is_dir() && !crate::union::es_union(&de) && std::fs::remove_dir_all(&de).is_err() {
        return;
    }
    if !de.exists() {
        let _ = std::fs::create_dir_all(vieja.join("cache"));
        let _ = crate::union::crear(&de, &a);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    fn temporal(etiqueta: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("pindesk-nombre-{etiqueta}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn la_carpeta_vieja_se_mueve_y_deja_la_nota() {
        let base = temporal("mover");
        let vieja = base.join(CARPETA_DATOS_VIEJA);
        fs::create_dir_all(vieja.join("proyectos/p1")).unwrap();
        fs::write(vieja.join("proyectos/p1/cuaderno.json"), b"{}").unwrap();
        fs::write(vieja.join(AJUSTES_VIEJO), b"idioma = \"es\"").unwrap();
        let nueva = base.join(CARPETA_DATOS);

        assert_eq!(
            carpeta_de_datos(&base),
            vieja,
            "antes de migrar se lee la vieja"
        );
        assert_eq!(migrar_carpeta(&vieja, &nueva), Migracion::Movida);

        assert_eq!(
            fs::read(nueva.join("proyectos/p1/cuaderno.json")).unwrap(),
            b"{}"
        );
        assert!(nueva.join(AJUSTES).is_file() && !nueva.join(AJUSTES_VIEJO).exists());
        assert!(vieja.join(AVISO_MOVIDO).is_file());
        assert_eq!(fs::read_dir(&vieja).unwrap().count(), 1, "solo la nota");
        assert_eq!(carpeta_de_datos(&base), nueva);
        // Caso negativo: una segunda vez no hace nada (y no pisa la nueva).
        assert_eq!(migrar_carpeta(&vieja, &nueva), Migracion::NadaQueHacer);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn si_ya_hay_carpeta_nueva_no_se_toca_ninguna() {
        let base = temporal("ya-nueva");
        let (vieja, nueva) = (base.join(CARPETA_DATOS_VIEJA), base.join(CARPETA_DATOS));
        fs::create_dir_all(&vieja).unwrap();
        fs::write(vieja.join("a.txt"), b"viejo").unwrap();
        fs::create_dir_all(&nueva).unwrap();
        fs::write(nueva.join("a.txt"), b"nuevo").unwrap();

        assert_eq!(migrar_carpeta(&vieja, &nueva), Migracion::NadaQueHacer);
        assert_eq!(fs::read(vieja.join("a.txt")).unwrap(), b"viejo");
        assert_eq!(fs::read(nueva.join("a.txt")).unwrap(), b"nuevo");
        assert_eq!(carpeta_de_datos(&base), nueva, "manda la nueva");
        // Caso negativo: sin carpeta vieja tampoco hay nada que hacer.
        let otra = temporal("sin-vieja");
        assert_eq!(
            migrar_carpeta(&otra.join(CARPETA_DATOS_VIEJA), &otra.join(CARPETA_DATOS)),
            Migracion::NadaQueHacer
        );
        assert!(!otra.join(CARPETA_DATOS).exists(), "no se crea de la nada");
        assert_eq!(carpeta_de_datos(&otra), otra.join(CARPETA_DATOS));
        let _ = fs::remove_dir_all(&base);
        let _ = fs::remove_dir_all(&otra);
    }

    #[test]
    fn la_copia_completa_la_nueva_y_conserva_la_vieja() {
        // `migrar_carpeta` copia cuando no puede renombrar; aqui se prueba la
        // copia y su paso final directamente.
        let base = temporal("copiar");
        let vieja = base.join("v");
        fs::create_dir_all(vieja.join("a/b")).unwrap();
        fs::write(vieja.join("a/b/c.bin"), [1u8, 2, 3]).unwrap();
        fs::write(vieja.join("raiz.txt"), b"r").unwrap();
        let destino = base.join("n");
        copiar_arbol(&vieja, &destino).unwrap();
        assert_eq!(fs::read(destino.join("a/b/c.bin")).unwrap(), [1, 2, 3]);
        assert_eq!(fs::read(destino.join("raiz.txt")).unwrap(), b"r");
        assert!(vieja.join("a/b/c.bin").is_file(), "la vieja sigue entera");
        // Caso negativo: copiar algo que no existe falla sin crear basura util.
        assert!(copiar_arbol(&base.join("no-esta"), &base.join("x")).is_err());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn una_vieja_que_solo_tiene_la_nota_no_cuenta_como_datos() {
        let base = temporal("solo-nota");
        let vieja = base.join(CARPETA_DATOS_VIEJA);
        fs::create_dir_all(&vieja).unwrap();
        fs::write(vieja.join(AVISO_MOVIDO), b"x").unwrap();
        assert_eq!(carpeta_de_datos(&base), base.join(CARPETA_DATOS));
        // Caso negativo: no se vuelve a "migrar" la nota a una carpeta nueva.
        assert_eq!(
            migrar_carpeta(&vieja, &base.join(CARPETA_DATOS)),
            Migracion::NadaQueHacer
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn los_ajustes_se_leen_nuevos_o_viejos_y_el_viejo_se_renombra() {
        let dir = temporal("ajustes");
        assert_eq!(ajustes_en(&dir), dir.join(AJUSTES), "sin ninguno, el nuevo");
        assert!(!es_portable(&dir));
        fs::write(dir.join(AJUSTES_VIEJO), b"x").unwrap();
        assert!(es_portable(&dir), "el viejo tambien es portable");
        assert_eq!(ajustes_en(&dir), dir.join(AJUSTES_VIEJO));
        assert!(migrar_ajustes(&dir).unwrap());
        assert_eq!(ajustes_en(&dir), dir.join(AJUSTES));
        assert_eq!(fs::read(dir.join(AJUSTES)).unwrap(), b"x");
        // Caso negativo: con los dos, no se pisa el nuevo.
        fs::write(dir.join(AJUSTES_VIEJO), b"viejo").unwrap();
        assert!(!migrar_ajustes(&dir).unwrap());
        assert_eq!(fs::read(dir.join(AJUSTES)).unwrap(), b"x");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn al_arrancar_se_migran_appdata_y_local_o_solo_el_toml_portable() {
        let base = temporal("arrancar");
        let (appdata, local, exe) = (base.join("roaming"), base.join("local"), base.join("exe"));
        for d in [&appdata, &local, &exe] {
            fs::create_dir_all(d.join(CARPETA_DATOS_VIEJA)).unwrap();
        }
        fs::write(local.join(CARPETA_DATOS_VIEJA).join("enlaces.clave"), b"k").unwrap();
        fs::create_dir_all(appdata.join(CARPETA_DATOS_VIEJA).join("cache/impresora")).unwrap();
        let hechas = migrar_al_arrancar(&appdata, Some(&local), &exe);
        assert_eq!(hechas.len(), 2);
        assert!(
            hechas.iter().all(|(_, m)| *m == Migracion::Movida),
            "{hechas:?}"
        );
        assert!(local.join(CARPETA_DATOS).join("enlaces.clave").is_file());
        // La impresora de antes sigue escribiendo en la ruta vieja: lleva a
        // la carpeta nueva.
        let puerto_viejo = appdata
            .join(CARPETA_DATOS_VIEJA)
            .join("cache/impresora/PixPin.pdf");
        fs::write(&puerto_viejo, b"%PDF-").unwrap();
        assert!(
            appdata
                .join(CARPETA_DATOS)
                .join("cache/impresora/PixPin.pdf")
                .is_file()
        );
        assert_eq!(carpeta_de_datos(&appdata), appdata.join(CARPETA_DATOS));

        // Portable: solo se renombra el toml del exe; APPDATA no se toca.
        let base2 = temporal("arrancar-portable");
        let (appdata2, exe2) = (base2.join("roaming"), base2.join("exe"));
        fs::create_dir_all(appdata2.join(CARPETA_DATOS_VIEJA)).unwrap();
        fs::create_dir_all(&exe2).unwrap();
        fs::write(exe2.join(AJUSTES_VIEJO), b"p").unwrap();
        let hechas = migrar_al_arrancar(&appdata2, None, &exe2);
        assert_eq!(hechas, [(exe2.join(AJUSTES), Migracion::Movida)]);
        assert!(exe2.join(AJUSTES).is_file());
        // Caso negativo: la carpeta de APPDATA del portable sigue donde estaba.
        assert!(appdata2.join(CARPETA_DATOS_VIEJA).is_dir());
        assert!(!appdata2.join(CARPETA_DATOS).exists());
        let _ = fs::remove_dir_all(&base);
        let _ = fs::remove_dir_all(&base2);
    }
}
