//! Diagnostico a mano: en un portatil con dos graficas, el duplicador debe
//! encontrar al menos la pantalla principal (antes caia siempre a WGC).
//! `cargo test -p pixpin-capture --test duplicador_en_cada_monitor -- --ignored --nocapture`

use pixpin_capture::{Dispositivo, Duplicador, enumerar_monitores};

#[test]
#[ignore = "necesita pantallas de verdad"]
fn el_duplicador_encuentra_la_pantalla_principal() {
    let d = Dispositivo::nuevo().expect("dispositivo");
    let monitores = enumerar_monitores().expect("monitores");
    let mut principal_ok = false;
    for m in monitores.monitores() {
        let r = Duplicador::nuevo(&d, m.id, m.area);
        println!(
            "monitor {} {:?} principal={} -> {:?}",
            m.id,
            m.area,
            m.principal,
            r.as_ref().err()
        );
        if m.principal {
            principal_ok = r.is_ok();
        }
    }
    assert!(principal_ok, "la pantalla principal tiene duplicador");
}
