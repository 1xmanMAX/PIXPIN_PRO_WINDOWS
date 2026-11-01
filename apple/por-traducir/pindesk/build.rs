//! Incrusta el manifiesto y el icono en el ejecutable.

fn main() {
    embed_resource::compile("pindesk.rc", embed_resource::NONE)
        .manifest_required()
        .expect("el manifiesto es obligatorio: sin el no hay PerMonitorV2");
    println!("cargo:rerun-if-changed=pindesk.rc");
    println!("cargo:rerun-if-changed=pindesk.manifest");
}
