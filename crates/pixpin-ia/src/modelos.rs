//! **Que se baja**: los modelos y el motor, con su peso y su huella.
//!
//! Todo con huella SHA-256 fija, comprobada contra lo que publica Hugging
//! Face/GitHub el 13-oct-2026 (ver el documento de investigacion): si alguien
//! cambia un fichero en el servidor, la descarga falla en vez de cargar algo
//! que no se probo.

/// Un fichero que se baja tal cual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fichero {
    /// Nombre en disco, dentro de la carpeta del modelo.
    pub nombre: &'static str,
    pub url: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

/// Un modelo: los pesos y, si ve imagenes, su proyector de vision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modelo {
    /// Identificador estable (carpeta en disco y ajuste guardado).
    pub id: &'static str,
    /// Como se le ensena al usuario.
    pub nombre: &'static str,
    pub licencia: &'static str,
    pub pesos: Fichero,
    pub vision: Option<Fichero>,
    /// RAM que ocupa el proceso con el modelo cargado y una imagen vista
    /// (medida en este PC; ver el documento).
    pub ram_mb: u32,
}

impl Modelo {
    pub fn ficheros(&self) -> Vec<Fichero> {
        let mut v = vec![self.pesos];
        v.extend(self.vision);
        v
    }

    pub fn bytes(&self) -> u64 {
        self.ficheros().iter().map(|f| f.bytes).sum()
    }
}

/// **El de siempre**: Qwen3.5-2B, 4 bits, con vision. ~1,9 GB de descarga.
pub const QWEN35_2B: Modelo = Modelo {
    id: "qwen3.5-2b",
    nombre: "Qwen3.5 2B (recomendado)",
    licencia: "Apache-2.0",
    pesos: Fichero {
        nombre: "Qwen3.5-2B-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/resolve/main/Qwen3.5-2B-Q4_K_M.gguf",
        bytes: 1_280_835_840,
        sha256: "aaf42c8b7c3cab2bf3d69c355048d4a0ee9973d48f16c731c0520ee914699223",
    },
    vision: Some(Fichero {
        nombre: "mmproj-F16.gguf",
        url: "https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/resolve/main/mmproj-F16.gguf",
        bytes: 668_227_264,
        sha256: "7035e9cb8d7c6a9681d07eef9a364783e86ea4cd73faab2eabb4f43a101830c7",
    }),
    ram_mb: 2900,
};

/// **El ligero**: Qwen3.5-0.8B, 4 bits, con vision. ~740 MB. Para equipos
/// con poca memoria y para el movil; dibuja peor y ve mucho peor (medido).
pub const QWEN35_08B: Modelo = Modelo {
    id: "qwen3.5-0.8b",
    nombre: "Qwen3.5 0.8B (ligero)",
    licencia: "Apache-2.0",
    pesos: Fichero {
        nombre: "Qwen3.5-0.8B-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/Qwen3.5-0.8B-GGUF/resolve/main/Qwen3.5-0.8B-Q4_K_M.gguf",
        bytes: 532_517_120,
        sha256: "bd258782e35f7f458f8aced1adc053e6e92e89bc735ba3be89d38a06121dc517",
    },
    vision: Some(Fichero {
        nombre: "mmproj-F16.gguf",
        url: "https://huggingface.co/unsloth/Qwen3.5-0.8B-GGUF/resolve/main/mmproj-F16.gguf",
        bytes: 204_987_232,
        sha256: "56e4c6cfe73b0c82e3e82bc518d7591997e61d81f723fc41a586f4fa69ea2453",
    }),
    ram_mb: 1400,
};

pub const MODELOS: [Modelo; 2] = [QWEN35_2B, QWEN35_08B];

pub fn por_id(id: &str) -> Option<Modelo> {
    MODELOS.iter().copied().find(|m| m.id == id)
}

/// El motor: la version de llama.cpp con Vulkan para Windows x64. Trae
/// tambien todos los motores de CPU (se elige solo el que sabe el
/// procesador) y funciona igual sin tarjeta grafica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Runtime {
    /// La version (`b11580`): tambien el nombre de su carpeta.
    pub version: &'static str,
    pub zip: Fichero,
    /// Lo que se saca del zip (el resto son herramientas de linea de ordenes
    /// que no hacen falta).
    pub extraer: &'static [&'static str],
}

pub const LLAMA_CPP: Runtime = Runtime {
    version: "b11580",
    zip: Fichero {
        nombre: "llama-b11580-bin-win-vulkan-x64.zip",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11580/llama-b11580-bin-win-vulkan-x64.zip",
        bytes: 32_112_251,
        sha256: "6e520f0b8bbc6608b53d70065ed80ee90d33acf1636f886d965e0c87f708dff2",
    },
    extraer: &[
        "llama-server.exe",
        "llama-server-impl.dll",
        "llama-common.dll",
        "llama.dll",
        "mtmd.dll",
        "ggml.dll",
        "ggml-base.dll",
        "ggml-vulkan.dll",
        "ggml-cpu-*.dll",
        "libomp.dll",
        "LICENSE*",
    ],
};

/// ¿Este nombre del zip entra en la lista? Admite un `*` al final o en medio
/// (`ggml-cpu-*.dll`).
pub fn se_extrae(patrones: &[&str], nombre: &str) -> bool {
    // Nada de carpetas: el zip trae todo en la raiz, y un nombre con ruta
    // podria escaparse de la carpeta de destino.
    if nombre.contains(['/', '\\']) || nombre.contains("..") {
        return false;
    }
    patrones.iter().any(|p| match p.split_once('*') {
        Some((antes, despues)) => {
            nombre.len() >= antes.len() + despues.len() && nombre.starts_with(antes) && nombre.ends_with(despues)
        }
        None => *p == nombre,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_huellas_son_sha256_en_hexadecimal() {
        let mut todas: Vec<Fichero> = MODELOS.iter().flat_map(|m| m.ficheros()).collect();
        todas.push(LLAMA_CPP.zip);
        for f in todas {
            assert_eq!(f.sha256.len(), 64, "{}", f.nombre);
            assert!(f.sha256.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
            assert!(f.url.starts_with("https://"));
            assert!(f.bytes > 1_000_000);
        }
    }

    #[test]
    fn la_lista_de_extraer_no_deja_escapar_rutas() {
        let p = LLAMA_CPP.extraer;
        assert!(se_extrae(p, "llama-server.exe"));
        assert!(se_extrae(p, "ggml-cpu-haswell.dll"));
        assert!(se_extrae(p, "LICENSE-LLVM-OpenMP"));
        assert!(!se_extrae(p, "llama-cli.exe"));
        assert!(!se_extrae(p, "ggml-cpu-"));
        assert!(!se_extrae(p, "../ggml-cpu-x.dll"));
        assert!(!se_extrae(p, "sub/llama-server.exe"));
    }

    #[test]
    fn se_encuentran_por_id() {
        assert_eq!(por_id("qwen3.5-2b"), Some(QWEN35_2B));
        assert_eq!(por_id("gpt-9"), None);
    }
}
