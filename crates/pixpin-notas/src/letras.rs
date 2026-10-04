//! **Las letras de la nota**: Fraunces (serif) para los titulos y Work Sans
//! para el cuerpo, las mismas que ya lleva la aplicacion para el lienzo de
//! citas (`pixpin_render::letras`), como el editor de la captura (titulos
//! con serif elegante, cuerpo sans).
//!
//! El `RichEdit` pide sus letras a GDI por el nombre, asi que aqui se
//! desempaquetan del woff2 y se registran **solo para este proceso**
//! (`AddFontMemResourceEx`): nada se instala en Windows. El nombre con que
//! GDI las conoce se lee de la tabla `name` de cada fichero (el de la
//! familia de Windows, ID 1), que no siempre es el de la familia: el corte
//! de Fraunces se llama «Fraunces NonWonky» por dentro.
//!
//! Si algo falla se usan las del sistema, Segoe UI y Georgia: se ve igual
//! de claro con otra letra.

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;

/// Los nombres que hay que pedir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Letras {
    pub cuerpo: String,
    pub titulos: String,
}

thread_local! {
    static PUESTAS: OnceCell<Letras> = const { OnceCell::new() };
    /// Las familias ya registradas y con que nombre las conoce GDI.
    static NOMBRES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

/// Registra las dos (una vez por hilo; GDI las guarda para todo el
/// proceso mientras viva) y dice como pedirlas.
pub fn registrar() -> Letras {
    PUESTAS.with(|p| p.get_or_init(montar).clone())
}

fn montar() -> Letras {
    Letras {
        cuerpo: nombre_gdi("Work Sans", "Segoe UI"),
        titulos: nombre_gdi("Fraunces", "Georgia"),
    }
}

/// **El nombre con que pedirle a GDI una letra del lienzo** (H12, la letra
/// elegida para la nota): la registra la primera vez, solo para el proceso.
/// Una del sistema (Courier New) se pide por su nombre; si una propia no se
/// puede registrar, `reserva`.
pub fn nombre_gdi(familia: &str, reserva: &str) -> String {
    if let Some(n) = NOMBRES.with(|m| m.borrow().get(familia).cloned()) {
        return n;
    }
    let ficheros = pixpin_render::letras::ficheros_llanos(familia);
    let nombre = if ficheros.is_empty() && !pixpin_render::letras::es_propia(familia) {
        Some(familia.to_string())
    } else {
        let mut nombre = None;
        for f in ficheros {
            let mut cuantas = 0u32;
            // SAFETY: GDI copia los datos; el bufer vive durante la llamada.
            let h = unsafe {
                windows::Win32::Graphics::Gdi::AddFontMemResourceEx(
                    f.as_ptr().cast(),
                    f.len() as u32,
                    None,
                    // Puntero de salida: Windows escribe aqui cuantas letras puso.
                    &raw mut cuantas,
                )
            };
            if !h.is_invalid() && cuantas > 0 && nombre.is_none() {
                nombre = familia_gdi(&f);
            }
        }
        nombre
    };
    let n = nombre.unwrap_or_else(|| reserva.to_string());
    NOMBRES.with(|m| m.borrow_mut().insert(familia.to_string(), n.clone()));
    n
}

/// Las dos letras de una vista elegida (ver `vista`), ya registradas.
pub fn de_vista(v: &crate::vista::Vista) -> Letras {
    Letras {
        cuerpo: nombre_gdi(&v.cuerpo, "Segoe UI"),
        titulos: nombre_gdi(&v.titulos, "Georgia"),
    }
}

fn be16(d: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(i)?, *d.get(i + 1)?]))
}

fn be32(d: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *d.get(i)?,
        *d.get(i + 1)?,
        *d.get(i + 2)?,
        *d.get(i + 3)?,
    ]))
}

/// El nombre de familia de Windows (plataforma 3, nombre 1) de un TTF/OTF.
pub fn familia_gdi(fichero: &[u8]) -> Option<String> {
    let tablas = be16(fichero, 4)? as usize;
    let name = (0..tablas).find_map(|i| {
        let e = 12 + i * 16;
        (fichero.get(e..e + 4)? == b"name").then(|| be32(fichero, e + 8))?
    })? as usize;
    let cuantos = be16(fichero, name + 2)? as usize;
    let cadenas = name + be16(fichero, name + 4)? as usize;
    for i in 0..cuantos {
        let r = name + 6 + i * 12;
        let (plataforma, id) = (be16(fichero, r)?, be16(fichero, r + 6)?);
        if plataforma == 3 && id == 1 {
            let largo = be16(fichero, r + 8)? as usize;
            let desde = cadenas + be16(fichero, r + 10)? as usize;
            let bytes = fichero.get(desde..desde + largo)?;
            let u: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            return Some(String::from_utf16_lossy(&u));
        }
    }
    None
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_dos_letras_se_desempaquetan_y_dicen_su_nombre() {
        let cuerpo = pixpin_render::letras::ficheros_llanos("Work Sans");
        let titulos = pixpin_render::letras::ficheros_llanos("Fraunces");
        assert_eq!(cuerpo.len(), 1);
        assert_eq!(titulos.len(), 1);
        let n = familia_gdi(&cuerpo[0]).unwrap();
        assert!(n.starts_with("Work Sans"), "{n}");
        let n = familia_gdi(&titulos[0]).unwrap();
        assert!(n.starts_with("Fraunces"), "{n}");
    }

    #[test]
    fn un_fichero_que_no_es_una_letra_no_tiene_nombre() {
        assert_eq!(familia_gdi(b"no soy una letra"), None);
        assert_eq!(familia_gdi(&[]), None);
        assert!(pixpin_render::letras::ficheros_llanos("Comic Sans MS").is_empty());
    }

    #[test]
    fn registrar_da_nombres_que_gdi_encuentra() {
        let l = registrar();
        assert!(!l.cuerpo.is_empty() && !l.titulos.is_empty());
        assert_eq!(registrar(), l, "la segunda vez no registra otra vez");
    }
}

#[cfg(test)]
mod pruebas_de_la_vista {
    use super::*;

    #[test]
    fn las_ocho_letras_del_lienzo_tienen_nombre_para_gdi() {
        for f in crate::vista::LETRAS {
            let n = nombre_gdi(f, "RESERVA");
            assert_ne!(n, "RESERVA", "{f} no se pudo registrar");
            assert!(!n.is_empty());
        }
        // La del sistema se pide tal cual.
        assert_eq!(nombre_gdi("Courier New", "x"), "Courier New");
    }

    #[test]
    fn una_letra_que_no_es_del_lienzo_se_pide_por_su_nombre_y_la_de_siempre_no_cambia() {
        // Una del sistema (como Courier New) va por su nombre: GDI la busca.
        assert_eq!(
            nombre_gdi("Familia Que No Existe", "Segoe UI"),
            "Familia Que No Existe"
        );
        let v = crate::vista::Vista::default();
        assert_eq!(de_vista(&v), registrar());
    }
}
