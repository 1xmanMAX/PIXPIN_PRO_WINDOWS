//! Incrusta el manifiesto y el icono en el ejecutable.

fn main() {
    embed_resource::compile("pixpinmax.rc", embed_resource::NONE)
        .manifest_required()
        .expect("el manifiesto es obligatorio: sin el no hay PerMonitorV2");
    println!("cargo:rerun-if-changed=pixpinmax.rc");
    println!("cargo:rerun-if-changed=pixpinmax.manifest");
}
