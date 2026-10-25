//! **Que se baja para leer con Indra**, de donde, cuanto pesa y su sha256.
//!
//! Los mismos ficheros que el escaner (`ModelCatalog.FAST_READER`), fijados
//! al commit de la etiqueta `v1.1.0` de `datalab-to/indra-ocr` (los de una
//! etiqueta no cambian): el movil y el PC leen igual. Solo lo de la
//! escritura latina —el paquete entero son 76 MB con CJK, arabe e indicos—.
//! Comprobados con `sha256sum` el 2026-10-25.

/// Commit de la etiqueta v1.1.0 de `datalab-to/indra-ocr`.
pub const REVISION: &str = "bd02167fdf2005a19962cc15188f8496a2c87d3b";

/// Un fichero que se baja.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fichero {
    pub nombre: &'static str,
    pub url: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

macro_rules! indra {
    ($n:literal, $b:expr, $s:literal) => {
        Fichero {
            nombre: $n,
            url: concat!(
                "https://huggingface.co/datalab-to/indra-ocr/resolve/bd02167fdf2005a19962cc15188f8496a2c87d3b/",
                $n
            ),
            bytes: $b,
            sha256: $s,
        }
    };
}

/// Los pesos (licencia OpenRAIL-M modificada de Datalab) y el diccionario
/// espanol (RLA-ES por LibreOffice, MPL), por orden de bajada: primero lo
/// pequeno, para que un fallo de red se note en el acto.
pub const FICHEROS: [Fichero; 8] = [
    indra!(
        "meta.json",
        211_869,
        "4ce4c94b46f5d644033c02e718322014ff4081270f385038c571912b56696339"
    ),
    indra!(
        "det.onnx",
        389_948,
        "f2bd4cba23e9c328cf7ec824634e423e191a2c6aacbbf35c62bd79dd70f62f59"
    ),
    indra!(
        "orient.onnx",
        371_802,
        "20eaf670f55a7cf9facfe1bdd6c7b8e40be75bbf4d48a3df43f66aa3bbd874fd"
    ),
    indra!(
        "order.onnx",
        2_973_138,
        "a42db2e8b536ad3d84045a4b7f9b0a72667cf8be4aa02d63f7b38004719fb180"
    ),
    indra!(
        "rec.onnx",
        5_042_161,
        "475972a2710be4a3cbfeb2bc7f3624ffd563de07de42476fc7a290d4e1ece484"
    ),
    indra!(
        "lm.npz",
        15_695_957,
        "6c0fb618a363a0acdd05d4f5b0edd9e8146d2d990541b87f6130bbfe17cea58d"
    ),
    Fichero {
        nombre: "es_ES.aff",
        url: "https://raw.githubusercontent.com/LibreOffice/dictionaries/libreoffice-25.8.2.2/es/es_ES.aff",
        bytes: 169_202,
        sha256: "e73a9bf8e1383f4986a5dc9e2fbed49371c0c61f511c626d15586bd433c1cad9",
    },
    Fichero {
        nombre: "es_ES.dic",
        url: "https://raw.githubusercontent.com/LibreOffice/dictionaries/libreoffice-25.8.2.2/es/es_ES.dic",
        bytes: 715_989,
        sha256: "6975dddec3d5d2c676069537bc67b4b5f786c65c5d4cf6703a82acf779ac9ec1",
    },
];

/// Lo que se ensena al usuario: «unos 26 MB».
pub fn megas() -> u64 {
    FICHEROS
        .iter()
        .map(|f| f.bytes)
        .sum::<u64>()
        .div_ceil(1_000_000)
}

/// **ONNX Runtime oficial** (MIT), para los Windows que no lo traen en
/// `System32` (Windows 10 y los 11 anteriores a 24H2). No se baja el ZIP
/// entero de 83 MB: solo la entrada de `onnxruntime.dll`, con una peticion
/// `Range` (lo mismo que hace el escaner con el AAR de Android). Medido el
/// 2026-10-25 sobre la release v1.31.0 de `microsoft/onnxruntime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeEnZip {
    pub version: &'static str,
    pub url: &'static str,
    /// Donde empieza la cabecera local de la entrada dentro del ZIP.
    pub desde: u64,
    /// Bytes a pedir: cabecera local (30 + nombre + extra) + comprimido.
    pub largo: u64,
    /// Tamano comprimido (deflate) y descomprimido de la DLL.
    pub comprimido: u64,
    pub bytes: u64,
    /// sha256 de la DLL ya descomprimida.
    pub sha256: &'static str,
}

pub const RUNTIME: RuntimeEnZip = RuntimeEnZip {
    version: "1.31.0",
    url: "https://github.com/microsoft/onnxruntime/releases/download/v1.31.0/onnxruntime-win-x64-1.31.0.zip",
    desde: 221_047,
    largo: 6_178_089,
    comprimido: 6_178_013,
    bytes: 17_253_176,
    sha256: "0c42dc4e278f0968fdf9ebf1b1e54a74c657a9942e70803e453c91993451cf4c",
};

/// Donde empiezan los datos comprimidos dentro de lo pedido, leyendo la
/// cabecera local del ZIP (`PK\x03\x04`); `None` si no lo es o no es
/// deflate.
pub fn datos_de_la_entrada(b: &[u8]) -> Option<usize> {
    if b.len() < 30 || &b[..4] != b"PK\x03\x04" {
        return None;
    }
    let metodo = u16::from_le_bytes([b[8], b[9]]);
    let nombre = u16::from_le_bytes([b[26], b[27]]) as usize;
    let extra = u16::from_le_bytes([b[28], b[29]]) as usize;
    (metodo == 8).then_some(30 + nombre + extra)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_pesos_estan_fijados_a_la_etiqueta_y_traen_su_sha() {
        for f in FICHEROS {
            assert_eq!(f.sha256.len(), 64, "{}", f.nombre);
            assert!(f.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(f.url.starts_with("https://"));
            assert!(f.url.ends_with(f.nombre));
            if f.nombre.ends_with(".onnx") || f.nombre.ends_with(".npz") || f.nombre == "meta.json"
            {
                assert!(f.url.contains(REVISION), "{}", f.nombre);
            }
        }
        assert_eq!(megas(), 26);
        assert_eq!(RUNTIME.largo, RUNTIME.comprimido + 30 + 46);
    }

    #[test]
    fn la_cabecera_local_del_zip_dice_donde_empiezan_los_datos() {
        let mut b = vec![0u8; 30];
        b[..4].copy_from_slice(b"PK\x03\x04");
        b[8] = 8;
        b[26] = 46;
        assert_eq!(datos_de_la_entrada(&b), Some(76));
        b[8] = 0;
        assert_eq!(datos_de_la_entrada(&b), None, "sin comprimir no se espera");
        assert_eq!(datos_de_la_entrada(b"nada"), None);
    }
}
