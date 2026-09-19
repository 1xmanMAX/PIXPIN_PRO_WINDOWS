//! El canal cifrado: saludo, claves de sesion y tramos.
//!
//! El mismo cable que `sincro/Canal.kt` de PixPin Android. Nada de esto se
//! puede «mejorar» por libre: cambiar un orden, un tamano o un rotulo deja de
//! entenderse con el movil, que es justo lo que se quiere evitar.
//!
//! Como va: los dos lados escriben a la vez `PXS1` y 32 bytes al azar, y de
//! ahi salen dos claves, una por sentido. A partir del saludo todo va cifrado
//! con AES-256-GCM. **El codigo del grupo no viaja**: si el otro tiene otro
//! codigo, su primer tramo no descifra, y eso es toda la autenticacion. Por
//! eso un fallo de descifrado hay que contarlo como «el otro tiene otro
//! codigo» y no como un fallo de red.

use std::io::{Read, Write};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};

use crate::codigo::hmac;

/// Los cuatro bytes con los que empieza todo. La `1` es la version del
/// saludo, que no es la del protocolo.
pub const MAGIA: &[u8; 4] = b"PXS1";
/// Lo que mide el numero al azar de cada lado.
pub const NONCE: usize = 32;
/// Lo mas grande que se admite en un tramo: 64 MiB. Un numero mayor solo
/// puede venir de basura, o de alguien que quiere que reservemos memoria.
pub const TOPE_DE_MENSAJE: usize = 64 << 20;
/// Lo mas grande que se manda de una vez al trocear un archivo: 1 MiB.
pub const TOPE_DE_TRAMO: usize = 1 << 20;

/// De que es un tramo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    /// Una peticion o una respuesta, en JSON UTF-8.
    Json,
    /// Un trozo de archivo, en crudo.
    Trozo,
}

impl Tipo {
    pub fn byte(self) -> u8 {
        match self {
            Tipo::Json => 1,
            Tipo::Trozo => 2,
        }
    }

    pub fn de_byte(b: u8) -> Option<Tipo> {
        match b {
            1 => Some(Tipo::Json),
            2 => Some(Tipo::Trozo),
            _ => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorCanal {
    #[error("no se pudo hablar con el otro aparato: {0}")]
    Io(#[from] std::io::Error),
    /// Lo que llego no empieza por `PXS1`: no es PixPin.
    #[error("eso no es PixPin")]
    NoEsPixPin,
    /// No descifra. En la practica siempre significa lo mismo.
    #[error("el otro aparato tiene otro codigo de grupo")]
    CodigoDistinto,
    #[error("tramo de {0} bytes, fuera de lo admitido")]
    TramoRaro(i32),
    #[error("tipo de tramo desconocido: {0}")]
    TipoDesconocido(u8),
}

/// Las dos claves de una sesion, ya repartidas por sentido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Claves {
    pub salida: [u8; 32],
    pub entrada: [u8; 32],
}

/// Reparte las claves del saludo.
///
/// `inicia` es quien abrio la conexion. Los dos derivan lo mismo y luego se
/// las reparten al reves, que es lo que hace que cada sentido tenga su clave
/// y su contador sin negociar nada.
pub fn claves_de_sesion(
    clave_grupo: &[u8; 32],
    nonce_inicia: &[u8; NONCE],
    nonce_responde: &[u8; NONCE],
    inicia: bool,
) -> Claves {
    let mut semilla = Vec::with_capacity(6 + NONCE * 2);
    semilla.extend_from_slice(b"sesion");
    semilla.extend_from_slice(nonce_inicia);
    semilla.extend_from_slice(nonce_responde);
    let sesion = hmac(clave_grupo, &semilla);
    let ida = hmac(&sesion, b"ida");
    let vuelta = hmac(&sesion, b"vuelta");
    if inicia {
        Claves {
            salida: ida,
            entrada: vuelta,
        }
    } else {
        Claves {
            salida: vuelta,
            entrada: ida,
        }
    }
}

/// El IV de un tramo: cuatro ceros y el contador, en big-endian.
///
/// El contador no se repite nunca dentro de una sesion, que es lo que GCM
/// exige para no romperse; por eso hay uno por sentido y no uno compartido.
fn iv(contador: u64) -> [u8; 12] {
    let mut v = [0u8; 12];
    v[4..].copy_from_slice(&contador.to_be_bytes());
    v
}

/// Un canal ya saludado: cifra al escribir y descifra al leer.
pub struct Canal<F> {
    flujo: F,
    claves: Claves,
    contador_salida: u64,
    contador_entrada: u64,
}

impl<F: Read + Write> Canal<F> {
    /// Hace el saludo y devuelve el canal listo.
    ///
    /// `nonce_propio` se pasa desde fuera para poder probar esto sin azar; en
    /// la aplicacion sale de un generador criptografico.
    pub fn saludar(
        mut flujo: F,
        clave_grupo: &[u8; 32],
        nonce_propio: [u8; NONCE],
        inicia: bool,
    ) -> Result<Canal<F>, ErrorCanal> {
        // Los dos escriben antes de leer: si los dos leyeran primero, se
        // quedarian esperandose el uno al otro para siempre.
        flujo.write_all(MAGIA)?;
        flujo.write_all(&nonce_propio)?;
        flujo.flush()?;

        let mut saludo = [0u8; 4 + NONCE];
        flujo.read_exact(&mut saludo)?;
        if &saludo[..4] != MAGIA {
            return Err(ErrorCanal::NoEsPixPin);
        }
        let mut nonce_otro = [0u8; NONCE];
        nonce_otro.copy_from_slice(&saludo[4..]);

        let (ni, nr) = if inicia {
            (nonce_propio, nonce_otro)
        } else {
            (nonce_otro, nonce_propio)
        };
        Ok(Canal {
            flujo,
            claves: claves_de_sesion(clave_grupo, &ni, &nr, inicia),
            contador_salida: 0,
            contador_entrada: 0,
        })
    }

    pub fn claves(&self) -> Claves {
        self.claves
    }

    /// Manda un tramo.
    pub fn mandar(&mut self, tipo: Tipo, carga: &[u8]) -> Result<(), ErrorCanal> {
        let mut claro = Vec::with_capacity(1 + carga.len());
        claro.push(tipo.byte());
        claro.extend_from_slice(carga);

        let cifrador = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.claves.salida));
        let cifrado = cifrador
            .encrypt(
                Nonce::from_slice(&iv(self.contador_salida)),
                Payload {
                    msg: &claro,
                    aad: &[],
                },
            )
            .map_err(|_| ErrorCanal::CodigoDistinto)?;
        self.contador_salida += 1;

        // El largo es el del criptograma, en cuatro bytes big-endian CON
        // signo: es lo que escribe `DataOutputStream.writeInt` de Java.
        self.flujo
            .write_all(&(cifrado.len() as i32).to_be_bytes())?;
        self.flujo.write_all(&cifrado)?;
        Ok(())
    }

    /// Empuja lo pendiente de salida (`Canal.vaciar`). Hace falta antes de
    /// cerrar sin leer nada mas: `recibir` vacia solo, pero un ultimo
    /// «no, gracias» sobre un flujo con bufer se quedaria sin salir.
    pub fn vaciar(&mut self) -> Result<(), ErrorCanal> {
        self.flujo.flush()?;
        Ok(())
    }

    /// Espera un tramo. Vacia lo pendiente de salida antes de bloquearse: si
    /// no, los dos lados podrian quedarse esperando.
    pub fn recibir(&mut self) -> Result<(Tipo, Vec<u8>), ErrorCanal> {
        self.flujo.flush()?;
        let mut largo = [0u8; 4];
        self.flujo.read_exact(&mut largo)?;
        let largo = i32::from_be_bytes(largo);
        // Lo minimo que puede medir un tramo es el byte de tipo y la etiqueta
        // de GCM; nada por debajo de 17 es un tramo de verdad.
        if !(17..=TOPE_DE_MENSAJE as i32).contains(&largo) {
            return Err(ErrorCanal::TramoRaro(largo));
        }

        let mut cifrado = vec![0u8; largo as usize];
        self.flujo.read_exact(&mut cifrado)?;
        let descifrador = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.claves.entrada));
        let claro = descifrador
            .decrypt(
                Nonce::from_slice(&iv(self.contador_entrada)),
                Payload {
                    msg: &cifrado,
                    aad: &[],
                },
            )
            // Que no descifre solo puede querer decir una cosa: el otro
            // deriva otra clave, o sea que tiene otro codigo.
            .map_err(|_| ErrorCanal::CodigoDistinto)?;
        self.contador_entrada += 1;

        let (tipo, carga) = claro.split_first().ok_or(ErrorCanal::TramoRaro(largo))?;
        let tipo = Tipo::de_byte(*tipo).ok_or(ErrorCanal::TipoDesconocido(*tipo))?;
        Ok((tipo, carga.to_vec()))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::codigo::clave_de_grupo;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn las_claves_de_sesion_son_las_del_movil() {
        // Vectores calculados aparte, con otra implementacion de HMAC.
        let clave = clave_de_grupo("K7Q2M9XMPA");
        let ni = [1u8; NONCE];
        let nr = [2u8; NONCE];
        let quien_inicia = claves_de_sesion(&clave, &ni, &nr, true);
        assert_eq!(
            hex(&quien_inicia.salida),
            "2aa9bc6d16d774506e126d534783d0bb693d96e63d8cf3cea86dabc506411f3c",
            "la clave de ida"
        );
        assert_eq!(
            hex(&quien_inicia.entrada),
            "515ffbec20029392a540ef8dcc9579b8957e2eb8f2a8fdde2feeaa39b0ed4c9e",
            "la de vuelta"
        );
        // El otro lado deriva lo mismo y lo usa al reves: lo que uno manda,
        // el otro lo lee.
        let quien_responde = claves_de_sesion(&clave, &ni, &nr, false);
        assert_eq!(quien_responde.entrada, quien_inicia.salida);
        assert_eq!(quien_responde.salida, quien_inicia.entrada);
    }

    #[test]
    fn el_iv_es_el_contador_en_big_endian_detras_de_cuatro_ceros() {
        assert_eq!(hex(&iv(0)), "000000000000000000000000");
        assert_eq!(hex(&iv(1)), "000000000000000000000001");
        assert_eq!(hex(&iv(258)), "000000000000000000000102");
        // Dos contadores distintos nunca dan el mismo IV, que es lo que GCM
        // exige para no romperse.
        assert_ne!(iv(7), iv(8));
    }

    /// Dos canales enfrentados sobre tuberias en memoria, para probar el
    /// cable entero sin red.
    struct Tuberia {
        lee: std::sync::mpsc::Receiver<u8>,
        escribe: std::sync::mpsc::Sender<u8>,
    }

    impl Read for Tuberia {
        fn read(&mut self, destino: &mut [u8]) -> std::io::Result<usize> {
            for hueco in destino.iter_mut() {
                *hueco = self
                    .lee
                    .recv()
                    .map_err(|_| std::io::Error::from(std::io::ErrorKind::UnexpectedEof))?;
            }
            Ok(destino.len())
        }
    }

    impl Write for Tuberia {
        // Se manda al momento, como un socket sin bufer: guardarlo hasta un
        // `flush` que el otro lado no provoca deja a los dos esperandose.
        fn write(&mut self, datos: &[u8]) -> std::io::Result<usize> {
            for b in datos {
                let _ = self.escribe.send(*b);
            }
            Ok(datos.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn par() -> (Tuberia, Tuberia) {
        let (a_tx, a_rx) = std::sync::mpsc::channel();
        let (b_tx, b_rx) = std::sync::mpsc::channel();
        (
            Tuberia {
                lee: b_rx,
                escribe: a_tx,
            },
            Tuberia {
                lee: a_rx,
                escribe: b_tx,
            },
        )
    }

    /// Saluda los dos lados a la vez. Tiene que ser en hilos: cada uno
    /// escribe lo suyo y se queda esperando lo del otro, asi que hacerlos
    /// uno detras de otro se bloquea para siempre (y se bloqueo).
    fn saludar_los_dos(
        uno: Tuberia,
        otro: Tuberia,
        clave_uno: [u8; 32],
        clave_otro: [u8; 32],
    ) -> (Canal<Tuberia>, Canal<Tuberia>) {
        let hilo =
            std::thread::spawn(move || Canal::saludar(otro, &clave_otro, [9u8; NONCE], false));
        let cliente = Canal::saludar(uno, &clave_uno, [7u8; NONCE], true).unwrap();
        let servidor = hilo
            .join()
            .expect("el hilo del servidor no puede reventar")
            .unwrap();
        (cliente, servidor)
    }

    #[test]
    fn dos_canales_con_el_mismo_codigo_se_entienden() {
        let clave = clave_de_grupo("K7Q2M9XMPA");
        let (uno, otro) = par();
        let (mut cliente, mut servidor) = saludar_los_dos(uno, otro, clave, clave);

        cliente.mandar(Tipo::Json, br#"{"t":"hola"}"#).unwrap();
        let (tipo, carga) = servidor.recibir().unwrap();
        assert_eq!(tipo, Tipo::Json);
        assert_eq!(carga, br#"{"t":"hola"}"#);

        // Y de vuelta, con otro tipo y el contador ya andando.
        servidor.mandar(Tipo::Trozo, &[1, 2, 3]).unwrap();
        cliente.mandar(Tipo::Json, b"{}").unwrap();
        assert_eq!(cliente.recibir().unwrap(), (Tipo::Trozo, vec![1, 2, 3]));
        assert_eq!(servidor.recibir().unwrap(), (Tipo::Json, b"{}".to_vec()));
    }

    #[test]
    fn con_otro_codigo_no_descifra_y_se_dice_por_que() {
        let (uno, otro) = par();
        let (mut cliente, mut servidor) = saludar_los_dos(
            uno,
            otro,
            clave_de_grupo("K7Q2M9XMPA"),
            clave_de_grupo("K7Q2M9XMPB"),
        );
        cliente.mandar(Tipo::Json, b"{}").unwrap();
        // El saludo va en claro, asi que no falla ahi: falla al descifrar el
        // primer tramo, y eso hay que contarlo como «otro codigo», no como un
        // fallo de red.
        assert!(matches!(
            servidor.recibir(),
            Err(ErrorCanal::CodigoDistinto)
        ));
    }

    #[test]
    fn lo_que_no_empieza_por_pxs1_se_rechaza() {
        let (mut uno, otro) = par();
        // Alguien que habla otro idioma en el mismo puerto.
        uno.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();
        uno.write_all(&[0u8; NONCE]).unwrap();
        uno.flush().unwrap();
        let r = Canal::saludar(otro, &clave_de_grupo("K7Q2M9XMPA"), [1u8; NONCE], false);
        assert!(matches!(r, Err(ErrorCanal::NoEsPixPin)));
    }
}
