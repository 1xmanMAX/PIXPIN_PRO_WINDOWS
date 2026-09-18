//! Pasar algo de un aparato a otro UNA vez, por la misma wifi.
//!
//! No es sincronizar: no hace falta grupo, no queda nada vinculado y al
//! terminar el codigo deja de existir. Es el puerto de `sincro/Envio.kt` de
//! PixPin Android, y por eso los rotulos viajan como alli (`t`, `nombre`,
//! `bytes`...): son el contrato con el movil, no un nombre nuestro.
//!
//! **Los dos sentidos.** En el de siempre, quien envia espera y ensena su
//! codigo. En el otro —el que hace falta aqui— espera **quien recibe**: el
//! ordenador ensena su codigo, el movil lo escanea y llama. Es el mismo
//! protocolo con los papeles cambiados, y lo unico que cambia es quien
//! empieza el canal: **lo empieza el que llama**.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::canal::{Canal, ErrorCanal, Tipo};

/// Cuantas cifras tiene el codigo de un envio.
pub const CIFRAS: usize = 6;
/// El tipo mDNS que anuncia quien ESPERA para recibir.
pub const SERVICIO_RECIBIR: &str = "_pixpinrecibe._tcp.";

/// La clave del canal de un envio. Lleva `envio-` delante, como en Android.
pub fn clave(codigo: &str) -> [u8; 32] {
    crate::codigo::clave_cruda(&format!("envio-{codigo}"))
}

/// Solo cifras, y como mucho las que caben.
pub fn limpiar(tecleado: &str) -> String {
    tecleado
        .chars()
        .filter(char::is_ascii_digit)
        .take(CIFRAS)
        .collect()
}

pub fn valido(codigo: &str) -> bool {
    codigo.len() == CIFRAS && codigo.chars().all(|c| c.is_ascii_digit())
}

/// `482 913`: en dos grupos de tres, que se dicta y se lee sin perderse.
pub fn legible(codigo: &str) -> String {
    if codigo.len() == CIFRAS {
        format!("{} {}", &codigo[..3], &codigo[3..])
    } else {
        codigo.to_string()
    }
}

/// Lo que lleva el QR que ensena quien ESPERA para recibir, tal y como lo lee
/// `Envio.leerQr` del movil.
pub fn texto_del_qr(codigo: &str, host: &str, puerto: u16) -> String {
    format!("pixpin-recibe:1:{codigo}:{host}:{puerto}")
}

/// Una cosa que se manda. Los nombres de los campos son los del movil.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Elemento {
    /// `archivo`, `proyecto` (un `.pixpin`) o `lienzo`.
    pub tipo: String,
    pub nombre: String,
    pub bytes: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    /// Su sena entre envios: la misma cosa mandada otra vez lleva la misma.
    #[serde(default)]
    pub identidad: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proyecto: Option<String>,
    #[serde(
        rename = "proyectoNombre",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub proyecto_nombre: Option<String>,
    #[serde(default)]
    pub creado: i64,
    /// Los tres codigos (ver `pixpin_proyecto::codigos`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(
        rename = "codigoDeChat",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub codigo_de_chat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aparato: Option<String>,
}

/// Lo que ofrece quien envia.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Oferta {
    #[serde(default)]
    pub de: String,
    #[serde(rename = "deId", default)]
    pub de_id: String,
    #[serde(default)]
    pub elementos: Vec<Elemento>,
    #[serde(rename = "deCodigo", default)]
    pub de_codigo: String,
    /// Quien envia no aprobo a este aparato: lo dice en vez de cortar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rechazado: Option<String>,
}

/// Los avisos cortos del dialogo: `hola`, `acepto`, `no`, `archivo`, `listo`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Aviso {
    pub t: String,
    #[serde(default)]
    pub nombre: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub bytes: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// La cola que va detras de los trozos de un archivo: su resumen.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cola {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resumen: Option<String>,
    /// El que manda dice que ese archivo no vale: cambio mientras lo leia.
    #[serde(default)]
    pub saltado: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorEnvio {
    #[error(transparent)]
    Canal(#[from] ErrorCanal),
    #[error("no se entendio lo que mando el otro aparato: {0}")]
    Formato(#[from] serde_json::Error),
    #[error("se esperaba {que} y llego otra cosa")]
    FueraDeOrden { que: &'static str },
    #[error("el otro aparato dijo: {0}")]
    Dijo(String),
    #[error("«{0}» llego distinto de como salio")]
    Corrupto(String),
    #[error("no se pudo escribir lo que llego: {0}")]
    Escritura(#[from] std::io::Error),
}

fn mandar_aviso<F: std::io::Read + std::io::Write>(
    c: &mut Canal<F>,
    a: &Aviso,
) -> Result<(), ErrorEnvio> {
    c.mandar(Tipo::Json, &serde_json::to_vec(a)?)?;
    Ok(())
}

fn leer_json<F: std::io::Read + std::io::Write>(
    c: &mut Canal<F>,
    que: &'static str,
) -> Result<Vec<u8>, ErrorEnvio> {
    let (tipo, datos) = c.recibir()?;
    if tipo != Tipo::Json {
        return Err(ErrorEnvio::FueraDeOrden { que });
    }
    Ok(datos)
}

/// El lado que RECIBE, que aqui es el ordenador.
pub struct Receptor<F: std::io::Read + std::io::Write> {
    canal: Canal<F>,
    pub oferta: Oferta,
}

impl<F: std::io::Read + std::io::Write> Receptor<F> {
    /// Se presenta y trae la oferta. `inicia` dice si es ESTE aparato el que
    /// llama: falso cuando el ordenador esperaba y el movil llamo.
    pub fn conectar(
        flujo: F,
        codigo: &str,
        nombre: &str,
        id: &str,
        nonce: [u8; crate::canal::NONCE],
        inicia: bool,
    ) -> Result<Self, ErrorEnvio> {
        let mut canal = Canal::saludar(flujo, &clave(codigo), nonce, inicia)?;
        mandar_aviso(
            &mut canal,
            &Aviso {
                t: "hola".into(),
                nombre: nombre.to_string(),
                id: id.to_string(),
                ..Default::default()
            },
        )?;
        let datos = leer_json(&mut canal, "la oferta")?;
        let oferta: Oferta = serde_json::from_slice(&datos)?;
        if let Some(motivo) = oferta.rechazado.clone() {
            return Err(ErrorEnvio::Dijo(motivo));
        }
        Ok(Receptor { canal, oferta })
    }

    /// Dice que no. Lo que falle aqui da igual: ya no se recibe nada.
    pub fn rechazar(&mut self) {
        let _ = mandar_aviso(
            &mut self.canal,
            &Aviso {
                t: "no".into(),
                ..Default::default()
            },
        );
    }

    /// Acepta y escribe cada cosa en `carpeta`. Devuelve donde quedo cada una.
    ///
    /// `avance` recibe los bytes que van llegando y el total, para poder
    /// ensenarlo mientras dura.
    pub fn aceptar(
        &mut self,
        carpeta: &std::path::Path,
        mut avance: impl FnMut(u64, u64),
    ) -> Result<Vec<(Elemento, std::path::PathBuf)>, ErrorEnvio> {
        mandar_aviso(
            &mut self.canal,
            &Aviso {
                t: "acepto".into(),
                ..Default::default()
            },
        )?;
        std::fs::create_dir_all(carpeta)?;
        let total: u64 = self
            .oferta
            .elementos
            .iter()
            .map(|e| e.bytes.max(0) as u64)
            .sum();
        let mut hechos = 0u64;
        let mut salida = Vec::new();
        for e in self.oferta.elementos.clone() {
            let datos = leer_json(&mut self.canal, "un archivo")?;
            let cabecera: Aviso = serde_json::from_slice(&datos)?;
            if let Some(error) = cabecera.error {
                return Err(ErrorEnvio::Dijo(error));
            }
            if cabecera.t != "archivo" {
                return Err(ErrorEnvio::FueraDeOrden { que: "un archivo" });
            }
            let destino = ruta_libre(carpeta, &nombre_sano(&e.nombre));
            let mut fichero = std::fs::File::create(&destino)?;
            let bien = self.recibir_trozos(cabecera.bytes.max(0) as u64, &mut fichero, |n| {
                hechos += n;
                avance(hechos, total);
            })?;
            drop(fichero);
            if !bien {
                // Lo que llego a medias no se queda: mejor nada que un
                // fichero roto con un nombre que promete estar entero.
                let _ = std::fs::remove_file(&destino);
                return Err(ErrorEnvio::Corrupto(e.nombre.clone()));
            }
            salida.push((e, destino));
        }
        mandar_aviso(
            &mut self.canal,
            &Aviso {
                t: "listo".into(),
                ..Default::default()
            },
        )?;
        Ok(salida)
    }

    /// Los trozos de un archivo y su cola. `false` si el que manda dice que
    /// no vale, o si lo que llego no es lo que salio.
    fn recibir_trozos(
        &mut self,
        largo: u64,
        salida: &mut impl std::io::Write,
        mut avance: impl FnMut(u64),
    ) -> Result<bool, ErrorEnvio> {
        let mut md = Sha256::new();
        let mut quedan = largo;
        while quedan > 0 {
            let (tipo, datos) = self.canal.recibir()?;
            if tipo != Tipo::Trozo {
                return Err(ErrorEnvio::FueraDeOrden { que: "un trozo" });
            }
            salida.write_all(&datos)?;
            md.update(&datos);
            quedan = quedan.saturating_sub(datos.len() as u64);
            avance(datos.len() as u64);
        }
        let datos = leer_json(&mut self.canal, "la cola del archivo")?;
        let cola: Cola = serde_json::from_slice(&datos)?;
        if cola.saltado {
            return Ok(false);
        }
        let resumen: String = md.finalize().iter().map(|b| format!("{b:02x}")).collect();
        match cola.resumen {
            Some(suyo) if suyo != resumen => Ok(false),
            _ => Ok(true),
        }
    }
}

/// Un nombre que Windows admita: sin los signos prohibidos y sin pasarse de
/// largo. El mismo criterio que el movil, para que el fichero se llame igual
/// en los dos aparatos.
pub fn nombre_sano(nombre: &str) -> String {
    let limpio: String = nombre
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .take(120)
        .collect();
    let limpio = limpio.trim().to_string();
    if limpio.is_empty() {
        "archivo".to_string()
    } else {
        limpio
    }
}

/// Un nombre que no pise nada: `plano (2).pdf` si `plano.pdf` ya esta.
pub fn ruta_libre(carpeta: &std::path::Path, nombre: &str) -> std::path::PathBuf {
    let mut destino = carpeta.join(nombre);
    let (base, ext) = match nombre.rsplit_once('.') {
        Some((b, e)) if !b.is_empty() => (b.to_string(), format!(".{e}")),
        _ => (nombre.to_string(), String::new()),
    };
    let mut n = 2;
    while destino.exists() {
        destino = carpeta.join(format!("{base} ({n}){ext}"));
        n += 1;
    }
    destino
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_codigo_se_limpia_y_se_lee_como_en_el_movil() {
        assert_eq!(limpiar("48-29 13x"), "482913");
        assert_eq!(limpiar("4829137"), "482913", "no pasa de seis cifras");
        assert!(valido("482913"));
        assert!(!valido("48291"), "cinco cifras no valen");
        assert!(!valido("48291a"), "ni una letra colada");
        assert_eq!(legible("482913"), "482 913");
    }

    #[test]
    fn el_qr_dice_donde_esperar() {
        assert_eq!(
            texto_del_qr("482913", "192.168.1.40", 47474),
            "pixpin-recibe:1:482913:192.168.1.40:47474"
        );
    }

    #[test]
    fn la_clave_del_envio_lleva_su_prefijo() {
        // Android: `Grupo.clave("envio-$codigo")`. Sin el prefijo, un codigo
        // de envio abriria un grupo, que es otra cosa y con otra vida.
        assert_eq!(clave("482913"), crate::codigo::clave_cruda("envio-482913"));
        assert_ne!(clave("482913"), crate::codigo::clave_cruda("482913"));
    }

    #[test]
    fn un_nombre_del_movil_no_escribe_fuera_de_su_carpeta() {
        // Caso negativo que importa: un nombre con rutas dentro no puede
        // sacar el fichero de la carpeta elegida.
        assert_eq!(
            nombre_sano("../../windows/system32/a.dll"),
            ".._.._windows_system32_a.dll"
        );
        assert_eq!(nombre_sano("  "), "archivo");
        assert_eq!(nombre_sano("plano:1.pdf"), "plano_1.pdf");
    }

    #[test]
    fn dos_ficheros_con_el_mismo_nombre_no_se_pisan() {
        let d = std::env::temp_dir().join(format!("pixpin-envio-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("plano.pdf"), b"x").unwrap();
        assert_eq!(ruta_libre(&d, "plano.pdf"), d.join("plano (2).pdf"));
        assert_eq!(ruta_libre(&d, "otro.pdf"), d.join("otro.pdf"));
    }
}
