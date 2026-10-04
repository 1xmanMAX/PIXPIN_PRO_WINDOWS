//! **Lo reciente del buscador**: las ultimas busquedas (las fichas de
//! arriba, con su ✕ y «Borrar todas») y lo ultimo que se abrio desde aqui
//! («Abierto hace poco»).
//!
//! Vive en `<raiz>/buscar-recientes.json`, solo en este equipo: no se
//! sincroniza (es como el historial de un navegador). Un fichero roto o que
//! no esta es una lista vacia, nunca un error.

use std::path::{Path, PathBuf};

use pixpin_lanzador::resultados::{Accion, Resultado};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::modelo;

/// Cuantas busquedas se recuerdan.
pub const BUSQUEDAS: usize = 8;
/// Cuantas cosas abiertas se recuerdan.
pub const ABIERTOS: usize = 6;

pub const NOMBRE: &str = "buscar-recientes.json";

/// Algo que se abrio, con lo justo para volver a ensenarlo y abrirlo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Abierto {
    pub titulo: String,
    #[serde(default)]
    pub subtitulo: String,
    #[serde(default)]
    pub glifo: String,
    /// `Accion::a_valor` del plugin.
    pub accion: Value,
    #[serde(default)]
    pub clave: Option<String>,
    #[serde(default)]
    pub contexto: Option<Value>,
    #[serde(default)]
    pub vista_previa: Option<String>,
    #[serde(default)]
    pub fichero: Option<String>,
    #[serde(default)]
    pub copiar: Option<String>,
}

impl Abierto {
    pub fn de(r: &Resultado) -> Abierto {
        Abierto {
            titulo: r.titulo.clone(),
            subtitulo: r.subtitulo.clone(),
            glifo: r.glifo.to_string(),
            accion: r.accion.a_valor(),
            clave: r.clave.clone(),
            contexto: r.contexto.clone(),
            vista_previa: r.vista_previa.clone(),
            fichero: r.fichero.clone(),
            copiar: r.copiar.clone(),
        }
    }

    /// De vuelta a un resultado. `None` si la accion guardada ya no se
    /// entiende (de una version vieja).
    pub fn resultado(&self) -> Option<Resultado> {
        let accion = Accion::de_valor(&self.accion)?;
        let mut r = modelo::resultado(
            &self.titulo,
            &self.subtitulo,
            modelo::glifo_estatico(&self.glifo),
            accion,
        );
        r.clave = self.clave.clone();
        r.contexto = self.contexto.clone();
        r.vista_previa = self.vista_previa.clone();
        r.fichero = self.fichero.clone();
        r.copiar = self.copiar.clone();
        Some(r)
    }

    /// Lo que lo identifica: su clave, o su titulo si no tiene.
    fn identidad(&self) -> (&str, &str) {
        match &self.clave {
            Some(k) => ("k", k.as_str()),
            None => ("t", self.titulo.as_str()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Recientes {
    #[serde(default)]
    pub busquedas: Vec<String>,
    #[serde(default)]
    pub abiertos: Vec<Abierto>,
}

impl Recientes {
    pub fn ruta(raiz: &Path) -> PathBuf {
        raiz.join(NOMBRE)
    }

    pub fn leer(raiz: &Path) -> Recientes {
        std::fs::read_to_string(Self::ruta(raiz))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn guardar(&self, raiz: &Path) {
        let Ok(t) = serde_json::to_string_pretty(self) else {
            return;
        };
        let ruta = Self::ruta(raiz);
        let tmp = ruta.with_extension("json.tmp");
        if std::fs::write(&tmp, t).is_ok() && std::fs::rename(&tmp, &ruta).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }

    /// Apunta una busqueda, la primera. Se guarda limpia (sin blancos de mas)
    /// y una sola vez, sin mirar mayusculas.
    pub fn apuntar_busqueda(&mut self, consulta: &str) {
        let c = consulta.split_whitespace().collect::<Vec<_>>().join(" ");
        // Una letra sola no es una busqueda que valga la pena recordar.
        if c.chars().count() < 2 {
            return;
        }
        let baja = c.to_lowercase();
        self.busquedas.retain(|b| b.to_lowercase() != baja);
        self.busquedas.insert(0, c);
        self.busquedas.truncate(BUSQUEDAS);
    }

    pub fn quitar_busqueda(&mut self, i: usize) {
        if i < self.busquedas.len() {
            self.busquedas.remove(i);
        }
    }

    /// Apunta lo abierto, lo primero. Las acciones de una vez (apuntar una
    /// tarea, pegar una imagen, copiar) no: no son algo que se vuelva a abrir.
    pub fn apuntar_abierto(&mut self, r: &Resultado) {
        if !vale_como_abierto(r) {
            return;
        }
        let a = Abierto::de(r);
        self.abiertos.retain(|b| b.identidad() != a.identidad());
        self.abiertos.insert(0, a);
        self.abiertos.truncate(ABIERTOS);
    }
}

/// Si un resultado elegido se recuerda en «Abierto hace poco»: lo que es una
/// cosa (captura, archivo, tarea, leccion, proyecto, funcion conocida), no
/// lo que crea algo con el texto de ahora.
pub fn vale_como_abierto(r: &Resultado) -> bool {
    let Some(clave) = r.clave.as_deref() else {
        return false;
    };
    match &r.accion {
        Accion::Copiar(_) | Accion::PegarImagen { .. } => false,
        Accion::Pedido(p) => {
            !matches!(
                p.get("accion").and_then(Value::as_str),
                Some("anadir_tarea" | "chat" | "borrar_captura" | "conservar_captura")
            ) && !clave.is_empty()
        }
        _ => !clave.is_empty(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_lanzador::resultados::{glifo, pedido};
    use serde_json::json;

    fn temporal(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-buscar-recientes-{nombre}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn captura(n: &str) -> Resultado {
        let mut r = modelo::resultado(
            n,
            "Captura · hoy",
            glifo::IMAGEN,
            Accion::Pedido(pedido("pinear", json!({ "ruta": n }))),
        );
        r.clave = Some(format!("captura/{n}"));
        r.vista_previa = Some(n.into());
        r
    }

    #[test]
    fn las_busquedas_van_primero_sin_repetirse_y_con_tope() {
        let mut r = Recientes::default();
        r.apuntar_busqueda("grieta");
        r.apuntar_busqueda("  factura   temu ");
        r.apuntar_busqueda("GRIETA");
        assert_eq!(r.busquedas, vec!["GRIETA", "factura temu"]);
        for i in 0..20 {
            r.apuntar_busqueda(&format!("cosa {i}"));
        }
        assert_eq!(r.busquedas.len(), BUSQUEDAS);
        assert_eq!(r.busquedas[0], "cosa 19");
        r.quitar_busqueda(0);
        assert_eq!(r.busquedas[0], "cosa 18");
        // Caso negativo: una letra sola o vacio no se apunta; quitar fuera
        // de rango no rompe.
        let antes = r.busquedas.clone();
        r.apuntar_busqueda("t");
        r.apuntar_busqueda("   ");
        r.quitar_busqueda(99);
        assert_eq!(r.busquedas, antes);
    }

    #[test]
    fn lo_abierto_vuelve_igual_tras_guardar_y_leer() {
        let raiz = temporal("ida-vuelta");
        let mut r = Recientes::default();
        r.apuntar_abierto(&captura("a.png"));
        r.apuntar_abierto(&captura("b.png"));
        r.apuntar_abierto(&captura("a.png"));
        r.apuntar_busqueda("muro");
        r.guardar(&raiz);
        let leido = Recientes::leer(&raiz);
        assert_eq!(leido, r);
        let titulos: Vec<String> = leido.abiertos.iter().map(|a| a.titulo.clone()).collect();
        assert_eq!(
            titulos,
            vec!["a.png", "b.png"],
            "sin repetir y lo ultimo primero"
        );
        let vuelto = leido.abiertos[0].resultado().unwrap();
        assert_eq!(vuelto.accion, captura("a.png").accion);
        assert_eq!(vuelto.glifo, glifo::IMAGEN);
        assert_eq!(vuelto.vista_previa.as_deref(), Some("a.png"));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn caso_negativo_un_fichero_roto_es_una_lista_vacia() {
        let raiz = temporal("roto");
        std::fs::write(Recientes::ruta(&raiz), "{ esto no es json").unwrap();
        assert_eq!(Recientes::leer(&raiz), Recientes::default());
        // Y una accion que ya no se entiende no sale.
        let a = Abierto {
            accion: json!({ "metodo": "otro" }),
            ..Abierto::de(&captura("x"))
        };
        assert!(a.resultado().is_none());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn lo_que_crea_algo_no_se_recuerda_como_abierto() {
        let mut r = Recientes::default();
        let apuntar = modelo::resultado(
            "Apuntar tarea: pan",
            "",
            glifo::ANADIR,
            Accion::Pedido(pedido("anadir_tarea", json!({ "texto": "pan" }))),
        );
        r.apuntar_abierto(&apuntar);
        let mut copiar = modelo::resultado("Copiar", "", glifo::COPIAR, Accion::Copiar("x".into()));
        copiar.clave = Some("x".into());
        r.apuntar_abierto(&copiar);
        assert!(r.abiertos.is_empty());
        // Y lo que si es una cosa, si.
        r.apuntar_abierto(&captura("c.png"));
        assert_eq!(r.abiertos.len(), 1);
    }
}
