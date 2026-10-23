//! Incrusta el manifiesto y el icono en el ejecutable.

fn main() {
    // Solo para revisar el codigo desde Linux o Mac (`cargo check --target
    // x86_64-pc-windows-msvc`): alli no hay compilador de recursos de
    // Microsoft. Un ejecutable de verdad siempre lleva el manifiesto.
    println!("cargo:rerun-if-env-changed=PIXPIN_SIN_RECURSOS");
    if std::env::var_os("PIXPIN_SIN_RECURSOS").is_some() {
        return;
    }
    embed_resource::compile("pixpinmax.rc", embed_resource::NONE)
        .manifest_required()
        .expect("el manifiesto es obligatorio: sin el no hay PerMonitorV2");
    println!("cargo:rerun-if-changed=pixpinmax.rc");
    println!("cargo:rerun-if-changed=pixpinmax.manifest");
}
