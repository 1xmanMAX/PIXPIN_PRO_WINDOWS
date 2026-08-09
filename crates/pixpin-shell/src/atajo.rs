//! Combinaciones de teclas globales.
//!
//! Vive en `pixpin-shell` y no en `pixpin-store` por la regla de capas: shell
//! es L1 y store es L2, asi que el tipo tiene que estar abajo para que ambos
//! puedan usarlo. `RegisterHotKey` tambien esta aqui, asi que es su sitio
//! natural.
//!
//! Se serializa como texto ("Ctrl+Alt+X") en vez de como estructura para que
//! el fichero TOML sea legible y editable a mano.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

// Constantes de Win32, repetidas aqui para que el parseo sea logica pura y no
// arrastre el crate `windows` a los tests.
const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const MOD_WIN: u32 = 0x0008;
const VK_F1: u32 = 0x70;
const VK_SNAPSHOT: u32 = 0x2C;
const VK_INSERT: u32 = 0x2D;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modificadores {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

impl Modificadores {
    fn alguno(&self) -> bool {
        self.ctrl || self.alt || self.shift || self.win
    }
}

/// La tecla final de la combinacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tecla {
    /// Siempre en mayuscula.
    Letra(char),
    /// 0..=9
    Digito(u8),
    /// 1..=24
    Funcion(u8),
    /// Impr Pant
    Imprimir,
    Insertar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Atajo {
    pub modificadores: Modificadores,
    pub tecla: Tecla,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ErrorAtajo {
    #[error("la combinacion esta vacia")]
    Vacia,
    #[error("falta la tecla final")]
    SinTecla,
    #[error(
        "hace falta al menos un modificador: una tecla suelta a nivel global \
         impediria escribirla en cualquier otra aplicacion"
    )]
    SinModificador,
    #[error("modificador repetido: `{0}`")]
    ModificadorRepetido(String),
    #[error("tecla desconocida: `{0}`")]
    TeclaDesconocida(String),
}

impl Atajo {
    /// Mascara de modificadores tal como la espera `RegisterHotKey`.
    pub fn modificadores_win32(&self) -> u32 {
        let m = self.modificadores;
        (if m.alt { MOD_ALT } else { 0 })
            | (if m.ctrl { MOD_CONTROL } else { 0 })
            | (if m.shift { MOD_SHIFT } else { 0 })
            | (if m.win { MOD_WIN } else { 0 })
    }

    /// Codigo de tecla virtual tal como lo espera `RegisterHotKey`.
    pub fn tecla_win32(&self) -> u32 {
        match self.tecla {
            Tecla::Letra(c) => c as u32,
            Tecla::Digito(d) => u32::from(b'0' + d),
            Tecla::Funcion(n) => VK_F1 + u32::from(n - 1),
            Tecla::Imprimir => VK_SNAPSHOT,
            Tecla::Insertar => VK_INSERT,
        }
    }
}

impl FromStr for Atajo {
    type Err = ErrorAtajo;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        let texto = texto.trim();
        if texto.is_empty() {
            return Err(ErrorAtajo::Vacia);
        }

        let partes: Vec<&str> = texto.split('+').map(str::trim).collect();
        if partes.iter().any(|p| p.is_empty()) {
            return Err(ErrorAtajo::SinTecla);
        }

        let (ultima, previas) = partes.split_last().expect("nunca vacio tras el trim");

        let mut modificadores = Modificadores::default();
        for parte in previas {
            let campo = match parte.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => &mut modificadores.ctrl,
                "alt" => &mut modificadores.alt,
                "shift" | "mayus" => &mut modificadores.shift,
                "win" => &mut modificadores.win,
                _ => return Err(ErrorAtajo::TeclaDesconocida((*parte).to_string())),
            };
            if *campo {
                return Err(ErrorAtajo::ModificadorRepetido((*parte).to_string()));
            }
            *campo = true;
        }

        if !modificadores.alguno() {
            return Err(ErrorAtajo::SinModificador);
        }

        let tecla = parsear_tecla(ultima)?;
        Ok(Atajo {
            modificadores,
            tecla,
        })
    }
}

fn parsear_tecla(texto: &str) -> Result<Tecla, ErrorAtajo> {
    let arriba = texto.to_ascii_uppercase();

    if arriba.len() == 1 {
        let c = arriba.chars().next().expect("longitud comprobada");
        if c.is_ascii_alphabetic() {
            return Ok(Tecla::Letra(c));
        }
        if let Some(d) = c.to_digit(10) {
            return Ok(Tecla::Digito(d as u8));
        }
    }

    if let Some(resto) = arriba.strip_prefix('F') {
        if let Ok(n) = resto.parse::<u8>() {
            if (1..=24).contains(&n) {
                return Ok(Tecla::Funcion(n));
            }
        }
    }

    match arriba.as_str() {
        "IMPR" | "IMPRPANT" | "PRTSC" => Ok(Tecla::Imprimir),
        "INS" | "INSERT" | "INSERTAR" => Ok(Tecla::Insertar),
        _ => Err(ErrorAtajo::TeclaDesconocida(texto.to_string())),
    }
}

impl fmt::Display for Atajo {
    /// Forma canonica: siempre en el orden Ctrl, Alt, Shift, Win.
    ///
    /// El orden fijo es lo que hace que la ida y vuelta sea estable y que dos
    /// ficheros de ajustes equivalentes se vean iguales.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.modificadores;
        if m.ctrl {
            write!(f, "Ctrl+")?;
        }
        if m.alt {
            write!(f, "Alt+")?;
        }
        if m.shift {
            write!(f, "Shift+")?;
        }
        if m.win {
            write!(f, "Win+")?;
        }
        match self.tecla {
            Tecla::Letra(c) => write!(f, "{c}"),
            Tecla::Digito(d) => write!(f, "{d}"),
            Tecla::Funcion(n) => write!(f, "F{n}"),
            Tecla::Imprimir => write!(f, "Impr"),
            Tecla::Insertar => write!(f, "Ins"),
        }
    }
}

// Serde va por texto para que el TOML quede legible y editable a mano.
impl Serialize for Atajo {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Atajo {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)?;
        texto.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn parsea_la_combinacion_por_defecto() {
        let a: Atajo = "Ctrl+Alt+X".parse().unwrap();
        assert!(a.modificadores.ctrl);
        assert!(a.modificadores.alt);
        assert!(!a.modificadores.shift);
        assert!(!a.modificadores.win);
        assert_eq!(a.tecla, Tecla::Letra('X'));
    }

    #[test]
    fn el_formato_es_reversible() {
        // Ojo: se escriben ya en forma canonica (Ctrl, Alt, Shift, Win). Ese
        // orden fijo es lo que hace estable la ida y vuelta.
        for texto in ["Ctrl+Alt+X", "Ctrl+Shift+F12", "Alt+Win+9", "Ctrl+Impr"] {
            let a: Atajo = texto.parse().unwrap();
            assert_eq!(a.to_string(), texto, "no sobrevive la ida y vuelta");
        }
    }

    #[test]
    fn no_distingue_mayusculas_al_leer() {
        let a: Atajo = "ctrl+alt+x".parse().unwrap();
        let b: Atajo = "CTRL+ALT+X".parse().unwrap();
        assert_eq!(a, b);
        // Pero al escribir siempre sale en la forma canonica.
        assert_eq!(a.to_string(), "Ctrl+Alt+X");
    }

    #[test]
    fn rechaza_combinaciones_invalidas() {
        // Sin tecla final.
        assert!("Ctrl+".parse::<Atajo>().is_err());
        // Sin ningun modificador: RegisterHotKey lo aceptaria, pero secuestrar
        // una tecla suelta a nivel global rompe la escritura en cualquier app.
        assert!("X".parse::<Atajo>().is_err());
        // Tecla desconocida.
        assert!("Ctrl+Alt+Inexistente".parse::<Atajo>().is_err());
        // Modificador repetido.
        assert!("Ctrl+Ctrl+X".parse::<Atajo>().is_err());
        // Vacio.
        assert!("".parse::<Atajo>().is_err());
    }

    #[test]
    fn traduce_a_los_codigos_de_win32() {
        let a: Atajo = "Ctrl+Alt+X".parse().unwrap();
        // MOD_ALT = 0x0001, MOD_CONTROL = 0x0002
        assert_eq!(a.modificadores_win32(), 0x0001 | 0x0002);
        // El codigo virtual de una letra es su mayuscula ASCII.
        assert_eq!(a.tecla_win32(), 'X' as u32);

        let f: Atajo = "Ctrl+Shift+F12".parse().unwrap();
        // VK_F1 = 0x70, luego F12 = 0x7B
        assert_eq!(f.tecla_win32(), 0x7B);
    }

    #[test]
    fn se_serializa_como_texto_plano() {
        let a: Atajo = "Ctrl+Alt+X".parse().unwrap();
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, "\"Ctrl+Alt+X\"");
        let vuelta: Atajo = serde_json::from_str(&json).unwrap();
        assert_eq!(a, vuelta);
    }
}
