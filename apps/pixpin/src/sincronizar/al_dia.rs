//! **Llegar al otro aunque haya cambiado de direccion** (K6).
//!
//! Puerto de `AlDia.aparatos` del movil (`sincro/AlDia.kt:48-110`). La IP
//! de un telefono cambia cada vez que se reconecta a la Wi-Fi, y la
//! direccion recordada deja de valer. El usuario lo vio el 16-sep-2026: «se
//! queda buscando y no localiza a los que si estan conectados». La respuesta
//! del movil son dos fuentes, en este orden:
//!
//! 1. **La direccion recordada**, comprobada con un «PING» de poco mas de un
//!    segundo en vez de esperar a que una conexion entera se agote (8 s).
//! 2. **Si no contesta, el anuncio de la red** (mDNS) unos segundos, y lo
//!    que se encuentra **se apunta**: la proxima vez ya esta en la primera.
//!
//! En el PC la sonda de fondo (`sincronizar::sondear`) ya junta las dos cada
//! pocos segundos, pero el boton «Sincronizar» lleva la direccion que habia
//! al pintar. Si el otro cambio de IP entre medias, la vuelta se estrellaba
//! contra la vieja. Esto se mete justo antes de conectar.
//!
//! La decision es pura ([`donde_esta`]) para probarla sin red: quien llama
//! le da como sondear y como buscar.

/// Un aparato visto en el anuncio de la red, con lo que importa aqui.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Visto {
    /// El `id` del TXT; puede venir vacio de versiones viejas.
    pub id: String,
    /// El `n` del TXT (su nombre).
    pub nombre: String,
    pub host: String,
    pub puerto: u16,
}

/// Donde llamar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Donde {
    /// La de siempre contesto: se usa sin buscar, que buscar cuesta segundos.
    LaDeSiempre,
    /// La de siempre no contesto y el anuncio lo da en otra: hay que
    /// apuntarla.
    Nueva { host: String, puerto: u16 },
    /// Ni contesta ni se anuncia: apagado o fuera de esta Wi-Fi.
    NoEsta,
}

/// Decide donde llamar a un aparato. `id` es el suyo si se sabe; sin el se
/// reconoce por el nombre (como el movil, que usa el nombre cuando el `id`
/// del anuncio viene en blanco).
///
/// `contesta` solo se llama una vez y `buscar` solo si hace falta: con la
/// direccion buena no se sale a la red.
pub(crate) fn donde_esta(
    id: Option<&str>,
    nombre: &str,
    host: &str,
    puerto: u16,
    contesta: impl FnOnce(&str, u16) -> bool,
    buscar: impl FnOnce() -> Vec<Visto>,
) -> Donde {
    if contesta(host, puerto) {
        return Donde::LaDeSiempre;
    }
    let vistos = buscar();
    let es_el = |v: &&Visto| match id.filter(|i| !i.is_empty()) {
        // Con id, solo el id: dos aparatos pueden llamarse igual.
        Some(i) if !v.id.is_empty() => v.id == i,
        _ => !nombre.is_empty() && v.nombre == nombre,
    };
    match vistos.iter().find(es_el) {
        // Anunciado en la misma: el PING fallo por otra cosa (estaba
        // ocupado atendiendo). Se intenta ahi, que es donde esta.
        Some(v) if v.host == host && v.puerto == puerto => Donde::LaDeSiempre,
        Some(v) => Donde::Nueva {
            host: v.host.clone(),
            puerto: v.puerto,
        },
        None => Donde::NoEsta,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::cell::Cell;

    fn visto(id: &str, nombre: &str, host: &str, puerto: u16) -> Visto {
        Visto {
            id: id.into(),
            nombre: nombre.into(),
            host: host.into(),
            puerto,
        }
    }

    #[test]
    fn si_la_direccion_de_siempre_contesta_no_se_sale_a_buscar() {
        let buscado = Cell::new(false);
        let d = donde_esta(
            Some("tel"),
            "Telefono",
            "192.168.1.20",
            47474,
            |_, _| true,
            || {
                buscado.set(true);
                Vec::new()
            },
        );
        assert_eq!(d, Donde::LaDeSiempre);
        assert!(!buscado.get(), "buscar por la red cuesta segundos");
    }

    #[test]
    fn si_cambio_de_ip_se_le_encuentra_por_el_anuncio_con_su_id() {
        let d = donde_esta(
            Some("tel"),
            "Telefono",
            "192.168.1.20",
            47474,
            |_, _| false,
            || {
                vec![
                    visto("otro", "Telefono", "192.168.1.99", 47474),
                    visto("tel", "Telefono", "192.168.1.33", 47475),
                ]
            },
        );
        assert_eq!(
            d,
            Donde::Nueva {
                host: "192.168.1.33".into(),
                puerto: 47475
            }
        );
    }

    #[test]
    fn con_id_otro_aparato_que_se_llame_igual_no_vale() {
        let d = donde_esta(
            Some("tel"),
            "Telefono",
            "192.168.1.20",
            47474,
            |_, _| false,
            || vec![visto("otro", "Telefono", "192.168.1.99", 47474)],
        );
        assert_eq!(d, Donde::NoEsta);
    }

    #[test]
    fn sin_id_se_le_reconoce_por_el_nombre() {
        let d = donde_esta(
            None,
            "Telefono",
            "10.0.0.2",
            47474,
            |_, _| false,
            || {
                vec![
                    visto("", "Portatil", "10.0.0.8", 47474),
                    visto("", "Telefono", "10.0.0.9", 47474),
                ]
            },
        );
        assert_eq!(
            d,
            Donde::Nueva {
                host: "10.0.0.9".into(),
                puerto: 47474
            }
        );
    }

    #[test]
    fn apagado_o_fuera_de_la_wifi_no_esta() {
        let d = donde_esta(
            Some("tel"),
            "Telefono",
            "10.0.0.2",
            47474,
            |_, _| false,
            Vec::new,
        );
        assert_eq!(d, Donde::NoEsta);
    }

    #[test]
    fn anunciado_en_la_misma_direccion_se_intenta_ahi_aunque_el_ping_fallara() {
        let d = donde_esta(
            Some("tel"),
            "Telefono",
            "10.0.0.2",
            47474,
            |_, _| false,
            || vec![visto("tel", "Telefono", "10.0.0.2", 47474)],
        );
        assert_eq!(d, Donde::LaDeSiempre);
    }

    #[test]
    fn sin_id_ni_nombre_no_se_elige_a_cualquiera() {
        let d = donde_esta(
            None,
            "",
            "10.0.0.2",
            47474,
            |_, _| false,
            || vec![visto("", "", "10.0.0.9", 47474)],
        );
        assert_eq!(d, Donde::NoEsta);
    }
}
