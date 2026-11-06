//! Incrusta el manifiesto (PerMonitorV2) y el icono en el ejecutable, como
//! `apps/pindesk/build.rs`. Con `PINDESK_SIN_RECURSOS` (revisar desde Linux)
//! no se compila el recurso.

fn main() {
    println!("cargo:rerun-if-env-changed=PINDESK_SIN_RECURSOS");
    println!("cargo:rerun-if-env-changed=PIXPIN_SIN_RECURSOS");
    if std::env::var_os("PINDESK_SIN_RECURSOS").is_some()
        || std::env::var_os("PIXPIN_SIN_RECURSOS").is_some()
    {
        return;
    }
    embed_resource::compile("pindesk-plantillas.rc", embed_resource::NONE)
        .manifest_required()
        .expect("el manifiesto es obligatorio: sin el no hay PerMonitorV2");
    println!("cargo:rerun-if-changed=pindesk-plantillas.rc");
    println!("cargo:rerun-if-changed=pindesk-plantillas.manifest");
}
