//! **Que sabe hacer el panel de la bandeja**: cada boton es una
//! [`Accion`], y cada accion viaja al hilo principal por el MISMO camino que
//! una entrada del menu de la bandeja (un `WM_COMMAND` con su numero). Asi el
//! panel no duplica nada: capturar, ocultar los pines o salir los sigue
//! haciendo el bucle de `main.rs`, como si se hubieran elegido en el menu.
//!
//! Aqui va tambien lo puro del panel: el buscador de acciones, los
//! favoritos y su icono y color. Se prueba sin ventana.

use pixpin_render::Color;
use pixpin_render::icono::Icono;
use pixpin_store::comandos::{self, Comando};

use super::iconos as ic;
use crate::caja_dibujo::hex;

/// Una cosa que se puede pedir desde el panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accion {
    /// Un comando del catalogo (`pixpin_store::comandos`).
    Comando(Comando),
    /// Las entradas de la bandeja que no son comandos: sus numeros viven en
    /// `main.rs` (`ID_LECCIONES`…), y se mantienen porque herramientas y
    /// memorias del proyecto los usan.
    Lecciones,
    NuevaLeccion,
    Galeria,
    Tareas,
    AbrirDocumento,
    GruposVentanas,
}

/// Las que no son comandos, con su nombre estable (el del fichero de
/// ajustes) y su clave de titulo.
const OTRAS: [(Accion, &str, &str); 6] = [
    (Accion::Lecciones, "lecciones", "bandeja2-lecciones"),
    (
        Accion::NuevaLeccion,
        "nueva-leccion",
        "bandeja2-nueva-leccion",
    ),
    (Accion::Galeria, "galeria-capturas", "bandeja2-galeria"),
    (Accion::Tareas, "tareas", "bandeja2-tareas"),
    (
        Accion::AbrirDocumento,
        "abrir-documento",
        "bandeja2-documento",
    ),
    (Accion::GruposVentanas, "grupos-ventanas", "bandeja2-grupos"),
];

impl Accion {
    /// El numero que viaja en el `WM_COMMAND`.
    pub fn id(self) -> u32 {
        match self {
            Accion::Comando(c) => c.id(),
            Accion::Lecciones => crate::ID_LECCIONES,
            Accion::NuevaLeccion => crate::ID_LECCION_NUEVA,
            Accion::Galeria => crate::ID_GALERIA_CAPTURAS,
            Accion::Tareas => crate::ID_TAREAS,
            Accion::AbrirDocumento => crate::ID_ABRIR_DOCUMENTO,
            Accion::GruposVentanas => crate::ID_GRUPOS_VENTANAS,
        }
    }

    /// El nombre con que se guarda en `[bandeja] favoritos`.
    pub fn nombre(self) -> &'static str {
        match self {
            Accion::Comando(c) => c.nombre(),
            otra => OTRAS
                .iter()
                .find(|(a, _, _)| *a == otra)
                .map(|(_, n, _)| *n)
                .unwrap_or(""),
        }
    }

    pub fn desde_nombre(nombre: &str) -> Option<Accion> {
        if let Some((a, _, _)) = OTRAS.iter().find(|(_, n, _)| *n == nombre) {
            return Some(*a);
        }
        Comando::desde_nombre(nombre)
            .map(Accion::Comando)
            .filter(|a| todas().contains(a))
    }

    /// La clave del catalogo de idiomas con su titulo.
    pub fn clave_titulo(self) -> &'static str {
        match self {
            // Los de la maqueta, mas cortos que los del menu de siempre (que
            // no se tocan: los usa el menu del clic derecho).
            Accion::Comando(Comando::PinearPortapapeles) => "bandeja2-fav-pinear-copiado",
            Accion::Comando(Comando::Anotar) => "bandeja2-fav-anotar",
            Accion::Comando(Comando::PinearEnVivo) => "bandeja2-fav-pin-vivo",
            Accion::Comando(Comando::AbrirChat) => "bandeja2-abrir-pixpin",
            Accion::Comando(c) => c.descriptor().clave_titulo,
            otra => OTRAS
                .iter()
                .find(|(a, _, _)| *a == otra)
                .map(|(_, _, k)| *k)
                .unwrap_or(""),
        }
    }

    /// Si al pedirla el panel se cierra. Los interruptores lo dejan abierto:
    /// se ve el cambio en el mismo sitio donde se pulso.
    pub fn cierra_el_panel(self) -> bool {
        !matches!(
            self,
            Accion::Comando(
                Comando::AlternarPines | Comando::AlternarPasoDeClics | Comando::SilenciarAtajos
            )
        )
    }

    /// Si lo que hace mira la pantalla (una captura, anotar encima): el panel
    /// tiene que haberse ido de verdad de la pantalla antes, o saldria en
    /// la captura.
    pub fn mira_la_pantalla(self) -> bool {
        matches!(
            self,
            Accion::Comando(
                Comando::CapturarRegion
                    | Comando::CapturarYCopiar
                    | Comando::CapturarConScroll
                    | Comando::CapturarYAnotar
                    | Comando::CapturarUltimaRegion
                    | Comando::GrabarGif
                    | Comando::Cuentagotas
                    | Comando::Pinear
                    | Comando::PinearEnVivo
                    | Comando::Anotar
                    | Comando::AnotarCongelada
                    | Comando::CapturarConRetardo
                    | Comando::CopiarTexto
            )
        )
    }

    pub fn icono(self) -> &'static Icono {
        match self {
            Accion::Comando(c) => match c {
                Comando::CapturarRegion | Comando::CapturarYCopiar => &ic::CAPTURAR,
                Comando::CapturarUltimaRegion => &ic::ZONA,
                Comando::CapturarConScroll => &ic::SCROLL,
                Comando::CapturarYAnotar | Comando::Anotar | Comando::AnotarCongelada => &ic::LAPIZ,
                Comando::GrabarGif => &ic::GRABAR,
                Comando::Cuentagotas => &ic::CUENTAGOTAS,
                Comando::Pinear | Comando::PinearEnVivo | Comando::PinearSeleccion => &ic::PIN,
                Comando::PinearPortapapeles => &ic::PORTAPAPELES,
                Comando::CerrarTodosLosPines => &ic::CERRAR,
                Comando::AlternarPines => &ic::OJO_TACHADO,
                Comando::AlternarPasoDeClics => &ic::CURSOR,
                Comando::SilenciarAtajos => &ic::TECLADO,
                Comando::CapturarConRetardo => &ic::RETARDO,
                Comando::RestaurarUltimoPin => &ic::DEVOLVER,
                Comando::CopiarTexto => &ic::TEXTO,
                Comando::VentanaEncima => &ic::ENCIMA,
                Comando::AbrirChat => &ic::CHAT,
                Comando::AbrirAjustes => &ic::AJUSTES,
                Comando::Sincronizar => &ic::SINCRONIZAR,
                Comando::Salir => &ic::SALIR,
                // «Recibir del movil» no sale en el panel; y un comando nuevo
                // que aun no tenga icono propio, con la lupa.
                _ => &ic::BUSCAR,
            },
            Accion::Lecciones | Accion::NuevaLeccion => &ic::BOMBILLA,
            Accion::Galeria => &ic::GALERIA,
            Accion::Tareas => &ic::TAREAS,
            Accion::AbrirDocumento => &ic::CARPETA,
            Accion::GruposVentanas => &ic::GRUPO,
        }
    }

    /// El color de la ficha de un favorito (los de la maqueta: verde para
    /// los pines, naranja para anotar, morado para la voz, azul para
    /// capturar; lo demas en gris).
    pub fn color(self) -> Color {
        match self {
            Accion::Comando(c) => match c {
                Comando::Pinear
                | Comando::PinearEnVivo
                | Comando::PinearPortapapeles
                | Comando::PinearSeleccion
                | Comando::RestaurarUltimoPin
                | Comando::CerrarTodosLosPines
                | Comando::AlternarPines
                | Comando::AlternarPasoDeClics => hex(0x248A3D),
                Comando::Anotar | Comando::AnotarCongelada | Comando::CapturarYAnotar => {
                    hex(0xB86E00)
                }
                Comando::GrabarGif | Comando::CopiarTexto | Comando::Cuentagotas => hex(0x8E44B8),
                Comando::CapturarRegion
                | Comando::CapturarYCopiar
                | Comando::CapturarConScroll
                | Comando::CapturarUltimaRegion
                | Comando::CapturarConRetardo => hex(0x0060DF),
                _ => hex(0x48484A),
            },
            Accion::Lecciones | Accion::NuevaLeccion => hex(0x9A7400),
            Accion::Tareas => hex(0x1F7A3A),
            _ => hex(0x48484A),
        }
    }
}

/// Todas las que el panel puede ofrecer, en el orden del catalogo y luego
/// las ventanas. Sin «Salir» (tiene su fila aparte, al fondo y en rojo, y
/// no debe salir al buscar otra cosa) ni «Recibir del movil» (ya cuelga de
/// Sincronizar, como en el movil).
pub fn todas() -> Vec<Accion> {
    let mut v: Vec<Accion> = comandos::CATALOGO
        .iter()
        .filter(|d| !matches!(d.comando, Comando::Salir | Comando::RecibirDelMovil))
        .map(|d| Accion::Comando(d.comando))
        .collect();
    v.extend(OTRAS.iter().map(|(a, _, _)| *a));
    v
}

/// Los favoritos que dicen los nombres guardados; lo que ya no existe se
/// salta (un comando que se quito, una mano que escribio mal el TOML).
pub fn favoritos_de(nombres: &[String]) -> Vec<Accion> {
    let mut v: Vec<Accion> = Vec::new();
    for a in nombres.iter().filter_map(|n| Accion::desde_nombre(n)) {
        if !v.contains(&a) {
            v.push(a);
        }
    }
    v.truncate(pixpin_store::bandeja::TOPE);
    v
}

/// Pone o quita un favorito. Devuelve `false` si no cabe (ya hay el tope).
pub fn alternar_favorito(favoritos: &mut Vec<Accion>, a: Accion) -> bool {
    if let Some(i) = favoritos.iter().position(|x| *x == a) {
        favoritos.remove(i);
        return true;
    }
    if favoritos.len() >= pixpin_store::bandeja::TOPE {
        return false;
    }
    favoritos.push(a);
    true
}

/// En minusculas y sin tildes: «Galería» y «galeria» son lo mismo al
/// buscar, que es como se escribe deprisa.
pub fn normalizar(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            otra => otra,
        })
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect()
}

/// **El buscador de acciones.** `titulos` son las acciones con su titulo ya
/// traducido. Vale una accion si cada palabra de la consulta aparece en su
/// titulo; primero las que EMPIEZAN por lo escrito, luego las que tienen una
/// palabra que empieza asi, y al final el resto, cada grupo en su orden.
pub fn buscar(consulta: &str, titulos: &[(Accion, String)]) -> Vec<Accion> {
    let q = normalizar(consulta);
    let palabras: Vec<&str> = q.split_whitespace().collect();
    if palabras.is_empty() {
        return Vec::new();
    }
    let mut con_nota: Vec<(u8, usize, Accion)> = titulos
        .iter()
        .enumerate()
        .filter_map(|(i, (a, t))| {
            let t = normalizar(t);
            if !palabras.iter().all(|p| t.contains(p)) {
                return None;
            }
            let nota = if t.starts_with(q.trim()) {
                0
            } else if t.split_whitespace().any(|w| w.starts_with(palabras[0])) {
                1
            } else {
                2
            };
            Some((nota, i, *a))
        })
        .collect();
    con_nota.sort_by_key(|(n, i, _)| (*n, *i));
    con_nota.into_iter().map(|(_, _, a)| a).collect()
}

/// El atajo como chapita: «Ctrl+Alt+X» se ensena «Ctrl Alt X».
pub fn chapita(atajo: &str) -> String {
    atajo.replace('+', " ")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn con_titulos() -> Vec<(Accion, String)> {
        vec![
            (Accion::Comando(Comando::CapturarRegion), "Capturar".into()),
            (Accion::Galeria, "Galería de capturas".into()),
            (
                Accion::Comando(Comando::CapturarConScroll),
                "Captura larga con scroll".into(),
            ),
            (Accion::Tareas, "Tareas".into()),
        ]
    }

    #[test]
    fn buscar_sin_tildes_y_lo_que_empieza_asi_primero() {
        let r = buscar("galeria", &con_titulos());
        assert_eq!(r, vec![Accion::Galeria]);
        let r = buscar("CAPT", &con_titulos());
        assert_eq!(
            r,
            vec![
                Accion::Comando(Comando::CapturarRegion),
                Accion::Comando(Comando::CapturarConScroll),
                Accion::Galeria,
            ],
            "primero las que empiezan por lo escrito; «Galería de capturas» al final"
        );
        assert_eq!(
            buscar("larga scroll", &con_titulos()),
            vec![Accion::Comando(Comando::CapturarConScroll)]
        );
    }

    #[test]
    fn caso_negativo_buscar_lo_que_no_hay_o_nada() {
        assert!(buscar("xyzzy", &con_titulos()).is_empty());
        assert!(
            buscar("   ", &con_titulos()).is_empty(),
            "una consulta vacia no es «todas»"
        );
        assert!(
            buscar("capturar tareas", &con_titulos()).is_empty(),
            "cada palabra cuenta"
        );
    }

    #[test]
    fn los_numeros_son_los_del_menu_de_la_bandeja() {
        // Herramientas y memorias del proyecto pulsan estos numeros: no
        // pueden cambiar por haber anadido el panel.
        assert_eq!(Accion::Galeria.id(), 904);
        assert_eq!(Accion::Lecciones.id(), 905);
        assert_eq!(Accion::Tareas.id(), 907);
        assert_eq!(Accion::AbrirDocumento.id(), 902);
        assert_eq!(Accion::Comando(Comando::Salir).id(), Comando::Salir.id());
    }

    #[test]
    fn los_nombres_van_y_vuelven_y_salir_no_es_favorito() {
        for a in todas() {
            assert_eq!(Accion::desde_nombre(a.nombre()), Some(a), "{a:?}");
            assert!(!a.clave_titulo().is_empty(), "{a:?} sin titulo");
        }
        assert_eq!(
            Accion::desde_nombre("salir"),
            None,
            "Salir no se puede poner de favorito"
        );
        assert_eq!(Accion::desde_nombre("no-existe"), None);
    }

    #[test]
    fn favoritos_sin_repetir_ni_pasar_del_tope() {
        let n = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let f = favoritos_de(&n(&["anotar", "anotar", "no-existe", "tareas"]));
        assert_eq!(f, vec![Accion::Comando(Comando::Anotar), Accion::Tareas]);
        let mut f: Vec<Accion> = todas()
            .into_iter()
            .take(pixpin_store::bandeja::TOPE)
            .collect();
        let fuera = todas()[pixpin_store::bandeja::TOPE];
        assert!(
            !alternar_favorito(&mut f, fuera),
            "caso negativo: lleno, no entra"
        );
        assert!(
            alternar_favorito(&mut f, todas()[0]),
            "quitar siempre se puede"
        );
        assert!(alternar_favorito(&mut f, fuera));
        assert!(f.contains(&fuera));
    }

    #[test]
    fn la_chapita_lleva_espacios() {
        assert_eq!(chapita("Ctrl+Alt+X"), "Ctrl Alt X");
        assert_eq!(chapita("Ctrl+2"), "Ctrl 2");
    }

    #[test]
    fn los_interruptores_dejan_el_panel_abierto_y_capturar_mira_la_pantalla() {
        assert!(!Accion::Comando(Comando::AlternarPines).cierra_el_panel());
        assert!(Accion::Comando(Comando::CapturarRegion).cierra_el_panel());
        assert!(Accion::Comando(Comando::CapturarRegion).mira_la_pantalla());
        assert!(
            !Accion::Tareas.mira_la_pantalla(),
            "caso negativo: abrir una ventana no"
        );
    }
}
