//! Incrusta el manifiesto y el icono en el ejecutable.

fn main() {
    // Solo para revisar el codigo desde Linux o Mac (`cargo check --target
    // x86_64-pc-windows-msvc`): alli no hay compilador de recursos de
    // Microsoft. Un ejecutable de verdad siempre lleva el manifiesto.
    println!("cargo:rerun-if-env-changed=PIXPIN_SIN_RECURSOS");
    if std::env::var_os("PIXPIN_SIN_RECURSOS").is_some() {
        return;
    }
    println!("cargo:rerun-if-changed=pixpinmax.rc");
    println!("cargo:rerun-if-changed=pixpinmax.manifest");
    // La version de la Microsoft Store (rama `microsoft-store`): el mismo
    // manifiesto sin el elemento <msix>, que es del paquete de IDENTIDAD
    // (`ForgePixPin.PixPinMax`). Dentro del paquete de la Store la identidad
    // la da el propio paquete (`tienda/AppxManifest.xml`) y un exe que dice
    // ser de otro paquete solo puede confundir a Windows.
    let rc = if std::env::var_os("CARGO_FEATURE_TIENDA").is_some() {
        rc_de_la_tienda()
    } else {
        "pixpinmax.rc".to_string()
    };
    embed_resource::compile(&rc, embed_resource::NONE)
        .manifest_required()
        .expect("el manifiesto es obligatorio: sin el no hay PerMonitorV2");
}

/// Escribe en `OUT_DIR` el manifiesto sin <msix> y un `.rc` que lo usa, con
/// rutas absolutas (y barras `/`, que `rc.exe` acepta sin escapar).
fn rc_de_la_tienda() -> String {
    let dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("lo da cargo"));
    let fuera = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("lo da cargo"));
    let manifiesto =
        std::fs::read_to_string(dir.join("pixpinmax.manifest")).expect("se lee el manifiesto");
    let sin_msix = quitar_msix(&manifiesto);
    let destino = fuera.join("tienda.manifest");
    std::fs::write(&destino, sin_msix).expect("se escribe en OUT_DIR");
    let barras = |p: &std::path::Path| p.display().to_string().replace('\\', "/");
    let rc = format!(
        "#include <winuser.h>\n1 24 \"{}\"\n1 ICON \"{}\"\n",
        barras(&destino),
        barras(&dir.join("recursos").join("pixpinmax.ico")),
    );
    let ruta_rc = fuera.join("tienda.rc");
    std::fs::write(&ruta_rc, rc).expect("se escribe en OUT_DIR");
    ruta_rc.display().to_string()
}

/// El manifiesto sin el elemento `<msix ... />` (puede ocupar varias lineas).
fn quitar_msix(m: &str) -> String {
    let Some(ini) = m.find("<msix ") else {
        return m.to_string();
    };
    let Some(largo) = m[ini..].find("/>") else {
        return m.to_string();
    };
    format!("{}{}", &m[..ini], &m[ini + largo + 2..])
}
