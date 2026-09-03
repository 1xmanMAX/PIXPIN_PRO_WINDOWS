//! El registro de comandos: una fila por cada cosa que el programa sabe
//! hacer.
//!
//! Antes cada atajo era una constante, una rama del `match` del bucle y una
//! entrada escrita a mano en el menu de la bandeja; anadir una funcion
//! costaba tocar cuatro sitios y era facil olvidarse de uno. El PixPin
//! original lo resuelve con un registro —nombre estable, titulo traducido,
//! atajo opcional y marca de si sale en la bandeja— y esa idea es la que se
//! copia aqui (ver `docs/investigacion/2026-09-03-pixpin-original-estructura.md`).
//!
//! Reglas que sostienen todo lo demas:
//!
//! - **Todo comando del catalogo hace algo.** No se anaden filas «para
//!   luego»: un atajo que no responde se vive como una averia. Anadir una
//!   funcion es anadir su fila Y atenderla.
//! - **El nombre es el contrato con el fichero de ajustes.** El titulo se
//!   traduce y el atajo se cambia; el nombre, no.
//! - **El identificador numerico sale de la posicion en el catalogo**, y lo
//!   comparten `RegisterHotKey` y el menu de la bandeja: un solo espacio de
//!   numeros para las dos vias.

use std::collections::BTreeMap;

use pixpin_shell::Atajo;
use serde::{Deserialize, Serialize};

/// Cada cosa que el programa sabe hacer y se puede atar a un atajo o sacar
/// en la bandeja.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Comando {
    /// Capturar una region y ensenar la barra de resultado.
    CapturarRegion,
    /// Capturar y copiar al portapapeles, sin confirmacion.
    CapturarYCopiar,
    /// Captura larga con scroll.
    CapturarConScroll,
    /// Cuentagotas: copiar el color bajo el cursor.
    Cuentagotas,
    /// Recortar y dejarlo flotando como pin.
    Pinear,
    /// Pinear lo que haya en el portapapeles.
    PinearPortapapeles,
    /// Anotar sobre la pantalla en vivo.
    Anotar,
    /// Anotar sobre una captura estatica de la pantalla.
    AnotarCongelada,
    /// Cerrar todos los pines de la pantalla.
    CerrarTodosLosPines,
    /// Quitar los pines de la pantalla, o devolverlos si ya no estan.
    AlternarPines,
    /// Devolver a la pantalla el ultimo pin que se cerro.
    RestaurarUltimoPin,
    /// Fijar encima de todo la ventana que haya bajo el raton, o bajarla.
    VentanaEncima,
    /// Abrir la ventana de ajustes.
    AbrirAjustes,
    /// Cerrar el programa.
    Salir,
}

/// La ficha de un comando en el catalogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Descriptor {
    pub comando: Comando,
    /// Clave estable para el fichero de ajustes. No cambia nunca.
    pub nombre: &'static str,
    /// Clave del catalogo de idiomas con el titulo que ve el usuario.
    pub clave_titulo: &'static str,
    /// Atajo con el que nace, o `None` si nace sin atajo.
    pub atajo_por_defecto: Option<&'static str>,
    /// Si aparece como entrada del menu de la bandeja.
    pub en_bandeja: bool,
}

/// Todos los comandos, en el orden en que salen en la bandeja. La posicion
/// manda: de ella sale el identificador numerico.
pub const CATALOGO: &[Descriptor] = &[
    Descriptor {
        comando: Comando::CapturarRegion,
        nombre: "capturar-region",
        clave_titulo: "comando-capturar-region",
        // Sin atajo por defecto: el usuario quito Ctrl+Alt+X y prefiere el
        // gesto con Alt y la entrada de la bandeja (D81).
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::CapturarYCopiar,
        nombre: "capturar-y-copiar",
        clave_titulo: "comando-capturar-y-copiar",
        atajo_por_defecto: Some("Ctrl+Alt+C"),
        en_bandeja: false,
    },
    Descriptor {
        comando: Comando::CapturarConScroll,
        nombre: "capturar-con-scroll",
        clave_titulo: "comando-capturar-con-scroll",
        atajo_por_defecto: Some("Ctrl+Alt+S"),
        en_bandeja: false,
    },
    Descriptor {
        comando: Comando::Cuentagotas,
        nombre: "cuentagotas",
        clave_titulo: "comando-cuentagotas",
        atajo_por_defecto: None,
        en_bandeja: false,
    },
    Descriptor {
        comando: Comando::Pinear,
        nombre: "pinear",
        clave_titulo: "comando-pinear",
        atajo_por_defecto: Some("Ctrl+Alt+F"),
        en_bandeja: false,
    },
    Descriptor {
        comando: Comando::PinearPortapapeles,
        nombre: "pinear-portapapeles",
        clave_titulo: "comando-pinear-portapapeles",
        atajo_por_defecto: Some("Ctrl+Alt+V"),
        en_bandeja: false,
    },
    Descriptor {
        comando: Comando::Anotar,
        nombre: "anotar",
        clave_titulo: "comando-anotar",
        atajo_por_defecto: Some("Ctrl+Alt+A"),
        en_bandeja: false,
    },
    Descriptor {
        comando: Comando::AnotarCongelada,
        nombre: "anotar-congelada",
        clave_titulo: "comando-anotar-congelada",
        atajo_por_defecto: None,
        en_bandeja: false,
    },
    // Los tres de pines nacen SIN atajo a proposito: son de uso ocasional y
    // meter combinaciones nuevas por defecto es la forma mas rapida de
    // pisar las de otro programa. Quien los use a diario se los pone en el
    // fichero de ajustes.
    Descriptor {
        comando: Comando::AlternarPines,
        nombre: "alternar-pines",
        clave_titulo: "comando-alternar-pines",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::RestaurarUltimoPin,
        nombre: "restaurar-ultimo-pin",
        clave_titulo: "comando-restaurar-ultimo-pin",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::CerrarTodosLosPines,
        nombre: "cerrar-todos-los-pines",
        clave_titulo: "comando-cerrar-todos-los-pines",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::VentanaEncima,
        nombre: "ventana-encima",
        clave_titulo: "comando-ventana-encima",
        atajo_por_defecto: None,
        en_bandeja: false,
    },
    Descriptor {
        comando: Comando::AbrirAjustes,
        nombre: "abrir-ajustes",
        clave_titulo: "comando-abrir-ajustes",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::Salir,
        nombre: "salir",
        clave_titulo: "comando-salir",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
];

impl Comando {
    /// La posicion en el catalogo, que es de donde sale el identificador.
    fn indice(self) -> usize {
        CATALOGO
            .iter()
            .position(|d| d.comando == self)
            .expect("todo Comando esta en el catalogo; lo vigila una prueba")
    }

    pub fn descriptor(self) -> &'static Descriptor {
        &CATALOGO[self.indice()]
    }

    pub fn nombre(self) -> &'static str {
        self.descriptor().nombre
    }

    /// El numero con el que viaja por Win32. Empieza en 1 porque el cero no
    /// distingue de «sin identificador» en un `WM_COMMAND`.
    pub fn id(self) -> u32 {
        self.indice() as u32 + 1
    }

    pub fn desde_id(id: u32) -> Option<Comando> {
        let i = (id as usize).checked_sub(1)?;
        CATALOGO.get(i).map(|d| d.comando)
    }

    pub fn desde_nombre(nombre: &str) -> Option<Comando> {
        CATALOGO
            .iter()
            .find(|d| d.nombre == nombre)
            .map(|d| d.comando)
    }
}

/// Que atajo tiene cada comando ahora mismo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enlaces {
    /// Paralelo a `CATALOGO`.
    atajos: Vec<Option<Atajo>>,
}

impl Default for Enlaces {
    fn default() -> Self {
        Self {
            atajos: CATALOGO
                .iter()
                .map(|d| {
                    d.atajo_por_defecto.map(|a| {
                        a.parse()
                            .expect("los atajos por defecto son constantes de este fichero")
                    })
                })
                .collect(),
        }
    }
}

impl Enlaces {
    pub fn atajo_de(&self, c: Comando) -> Option<Atajo> {
        self.atajos[c.indice()]
    }

    pub fn poner(&mut self, c: Comando, atajo: Option<Atajo>) {
        self.atajos[c.indice()] = atajo;
    }

    /// Los que hay que registrar de verdad en el sistema.
    pub fn registrables(&self) -> Vec<(u32, Atajo)> {
        CATALOGO
            .iter()
            .filter_map(|d| self.atajo_de(d.comando).map(|a| (d.comando.id(), a)))
            .collect()
    }

    /// Dos comandos con el mismo atajo. El segundo nunca llegaria a
    /// dispararse, asi que conviene avisar en vez de dejarlo pasar.
    pub fn choque(&self) -> Option<(Comando, Comando)> {
        for (i, a) in CATALOGO.iter().enumerate() {
            let Some(atajo_a) = self.atajo_de(a.comando) else {
                continue;
            };
            for b in CATALOGO.iter().skip(i + 1) {
                if self.atajo_de(b.comando) == Some(atajo_a) {
                    return Some((a.comando, b.comando));
                }
            }
        }
        None
    }

    /// Aplica lo que diga la tabla `[comandos]` del fichero de ajustes.
    /// Devuelve los nombres que no reconoce, para dejarlos en el registro:
    /// un fichero escrito por una version mas nueva no debe impedir
    /// arrancar, pero tampoco callarse.
    pub fn aplicar_tabla(&mut self, tabla: &BTreeMap<String, String>) -> Vec<String> {
        let mut desconocidos = Vec::new();
        for (nombre, texto) in tabla {
            match Comando::desde_nombre(nombre) {
                // Cadena vacia: el usuario quiere ese comando SIN atajo. Es
                // distinto de no nombrarlo, que deja el de por defecto.
                Some(c) if texto.trim().is_empty() => self.poner(c, None),
                Some(c) => match texto.parse() {
                    Ok(a) => self.poner(c, Some(a)),
                    Err(_) => desconocidos.push(format!("{nombre} = {texto}")),
                },
                None => desconocidos.push(nombre.clone()),
            }
        }
        desconocidos
    }

    /// Los enlaces que salen de un fichero de ajustes, en tres capas: los
    /// de por defecto, encima la tabla vieja `[atajos]` y encima la nueva
    /// `[comandos]`. Asi un fichero de antes sigue valiendo, y quien ya use
    /// la tabla nueva manda sobre la vieja.
    pub fn de_ajustes(a: &crate::ajustes::Ajustes) -> (Enlaces, Vec<String>) {
        let mut e = Enlaces::default();
        let v = &a.atajos;
        for (comando, atajo) in [
            (Comando::CapturarRegion, v.region),
            (Comando::CapturarYCopiar, Some(v.copiar)),
            (Comando::CapturarConScroll, Some(v.scroll)),
            (Comando::Cuentagotas, v.cuentagotas),
            (Comando::Pinear, Some(v.pin)),
            (Comando::PinearPortapapeles, Some(v.portapapeles)),
            (Comando::Anotar, Some(v.anotar)),
            (Comando::AnotarCongelada, v.anotar_congelada),
        ] {
            e.poner(comando, atajo);
        }
        let avisos = e.aplicar_tabla(&a.comandos);
        (e, avisos)
    }

    /// Como se escriben en el fichero de ajustes.
    pub fn a_tabla(&self) -> BTreeMap<String, String> {
        CATALOGO
            .iter()
            .map(|d| {
                let valor = self
                    .atajo_de(d.comando)
                    .map(|a| a.to_string())
                    .unwrap_or_default();
                (d.nombre.to_string(), valor)
            })
            .collect()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Todos los comandos que existen, para poder recorrerlos en las
    /// pruebas. Si se anade uno y no se anade aqui, falla la prueba de
    /// cobertura de abajo.
    const TODOS: &[Comando] = &[
        Comando::CapturarRegion,
        Comando::CapturarYCopiar,
        Comando::CapturarConScroll,
        Comando::Cuentagotas,
        Comando::Pinear,
        Comando::PinearPortapapeles,
        Comando::Anotar,
        Comando::AnotarCongelada,
        Comando::AlternarPines,
        Comando::RestaurarUltimoPin,
        Comando::CerrarTodosLosPines,
        Comando::VentanaEncima,
        Comando::AbrirAjustes,
        Comando::Salir,
    ];

    #[test]
    fn el_catalogo_cubre_todos_los_comandos_una_sola_vez() {
        assert_eq!(CATALOGO.len(), TODOS.len(), "sobra o falta una fila");
        for c in TODOS {
            let cuantas = CATALOGO.iter().filter(|d| d.comando == *c).count();
            assert_eq!(cuantas, 1, "{c:?} aparece {cuantas} veces");
        }
    }

    #[test]
    fn los_nombres_y_las_claves_de_titulo_son_unicos() {
        // El nombre es el contrato con el fichero de ajustes: dos iguales
        // harian que uno pisara al otro en silencio.
        for (i, a) in CATALOGO.iter().enumerate() {
            for b in CATALOGO.iter().skip(i + 1) {
                assert_ne!(a.nombre, b.nombre, "nombre repetido: {}", a.nombre);
                assert_ne!(
                    a.clave_titulo, b.clave_titulo,
                    "titulo repetido: {}",
                    a.clave_titulo
                );
            }
        }
    }

    #[test]
    fn los_nombres_son_estables_y_legibles() {
        // Van a un fichero que el usuario edita a mano: minusculas, guiones
        // y nada mas.
        for d in CATALOGO {
            assert!(
                !d.nombre.is_empty()
                    && d.nombre.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "nombre no valido: {}",
                d.nombre
            );
        }
    }

    #[test]
    fn ningun_comando_nace_pisando_el_atajo_de_otro() {
        assert_eq!(Enlaces::default().choque(), None);
    }

    #[test]
    fn el_identificador_va_y_vuelve() {
        for c in TODOS {
            assert_eq!(Comando::desde_id(c.id()), Some(*c));
            assert!(c.id() >= 1, "el cero no vale como identificador");
        }
        // Caso negativo: fuera de rango no inventa un comando.
        assert_eq!(Comando::desde_id(0), None);
        assert_eq!(Comando::desde_id(CATALOGO.len() as u32 + 1), None);
    }

    #[test]
    fn los_identificadores_no_chocan_con_los_grupos_ocultos() {
        // La bandeja usa 200 en adelante para los grupos; los comandos
        // tienen que quedarse muy por debajo.
        assert!(
            CATALOGO.len() < 200,
            "el catalogo invade el espacio de los grupos"
        );
    }

    #[test]
    fn la_tabla_del_fichero_va_y_vuelve() {
        let mut e = Enlaces::default();
        e.poner(Comando::Cuentagotas, Some("Ctrl+Shift+F9".parse().unwrap()));
        e.poner(Comando::Pinear, None);

        let tabla = e.a_tabla();
        let mut vuelta = Enlaces::default();
        assert!(vuelta.aplicar_tabla(&tabla).is_empty());
        assert_eq!(vuelta, e);
    }

    #[test]
    fn una_cadena_vacia_deja_el_comando_sin_atajo() {
        // Distinto de no nombrarlo: no nombrarlo conserva el de por defecto.
        let mut e = Enlaces::default();
        assert!(e.atajo_de(Comando::Pinear).is_some());
        let tabla = BTreeMap::from([("pinear".to_string(), String::new())]);
        assert!(e.aplicar_tabla(&tabla).is_empty());
        assert_eq!(e.atajo_de(Comando::Pinear), None);
        assert!(
            e.atajo_de(Comando::Anotar).is_some(),
            "los que no se nombran no se tocan"
        );
    }

    #[test]
    fn lo_que_no_se_reconoce_se_ignora_pero_se_avisa() {
        // Un fichero escrito por una version mas nueva no debe impedir
        // arrancar; tampoco debe desaparecer sin dejar rastro.
        let mut e = Enlaces::default();
        let tabla = BTreeMap::from([
            ("funcion-del-futuro".to_string(), "Ctrl+Alt+Z".to_string()),
            ("pinear".to_string(), "NoEsUnAtajo".to_string()),
        ]);
        let avisos = e.aplicar_tabla(&tabla);
        assert_eq!(avisos.len(), 2, "los dos casos se avisan: {avisos:?}");
        assert_eq!(
            e.atajo_de(Comando::Pinear),
            Enlaces::default().atajo_de(Comando::Pinear),
            "un atajo ilegible deja el de antes, no borra el comando"
        );
    }

    #[test]
    fn solo_se_registran_los_que_tienen_atajo() {
        let e = Enlaces::default();
        let ids: Vec<u32> = e.registrables().iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&Comando::Pinear.id()));
        assert!(
            !ids.contains(&Comando::Cuentagotas.id()),
            "el cuentagotas nace sin atajo (D81)"
        );
        assert!(!ids.contains(&Comando::Salir.id()));
    }

    #[test]
    fn detecta_dos_comandos_con_el_mismo_atajo() {
        let mut e = Enlaces::default();
        let repetido = e.atajo_de(Comando::Pinear);
        e.poner(Comando::Cuentagotas, repetido);
        let (a, b) = e.choque().expect("tiene que detectarlo");
        assert!(
            [a, b].contains(&Comando::Pinear) && [a, b].contains(&Comando::Cuentagotas),
            "{a:?} y {b:?}"
        );
    }

    #[test]
    fn un_fichero_con_la_tabla_vieja_sigue_valiendo() {
        // Migracion: quien tenga el TOML de antes no puede quedarse sin
        // atajos de golpe.
        let mut a = crate::ajustes::Ajustes::default();
        a.atajos.pin = "Ctrl+Shift+F2".parse().unwrap();
        a.atajos.cuentagotas = Some("Ctrl+Shift+F3".parse().unwrap());

        let (e, avisos) = Enlaces::de_ajustes(&a);
        assert!(avisos.is_empty());
        assert_eq!(
            e.atajo_de(Comando::Pinear).unwrap().to_string(),
            "Ctrl+Shift+F2"
        );
        assert_eq!(
            e.atajo_de(Comando::Cuentagotas).unwrap().to_string(),
            "Ctrl+Shift+F3"
        );
        assert_eq!(
            e.atajo_de(Comando::CapturarYCopiar).unwrap().to_string(),
            "Ctrl+Alt+C",
            "lo que no toco sigue como estaba"
        );
    }

    #[test]
    fn la_tabla_nueva_manda_sobre_la_vieja() {
        let mut a = crate::ajustes::Ajustes::default();
        a.atajos.pin = "Ctrl+Shift+F2".parse().unwrap();
        a.comandos
            .insert("pinear".to_string(), "Ctrl+Shift+F8".to_string());

        let (e, _) = Enlaces::de_ajustes(&a);
        assert_eq!(
            e.atajo_de(Comando::Pinear).unwrap().to_string(),
            "Ctrl+Shift+F8"
        );
    }

    #[test]
    fn la_bandeja_sale_del_catalogo_y_no_esta_vacia() {
        let en_bandeja: Vec<&str> = CATALOGO
            .iter()
            .filter(|d| d.en_bandeja)
            .map(|d| d.nombre)
            .collect();
        assert!(
            en_bandeja.contains(&"salir"),
            "sin «salir» no se puede cerrar: {en_bandeja:?}"
        );
        assert!(en_bandeja.len() >= 2);
    }
}
