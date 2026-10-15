//! Guardian del baseline del procesador (D17 del diseno de rendimiento).
//!
//! La maquina suelo es un Core i3 de 3a generacion (Ivy Bridge): tiene AVX
//! pero NO AVX2 ni FMA3. Un binario compilado con `target-cpu=native` en una
//! maquina moderna, o con `x86-64-v3`, muere alli con instruccion ilegal al
//! arrancar y sin mensaje util. Este test convierte esa regla en algo que la
//! maquina hace cumplir, como el test de capas hace con las dependencias.

use std::fs;
use std::path::Path;

fn config() -> String {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("el paquete debe estar en <raiz>/apps/pixpin");
    fs::read_to_string(raiz.join(".cargo").join("config.toml")).expect(
        "falta .cargo/config.toml: el baseline del procesador debe ser una \
         decision escrita, no el valor por defecto por accidente",
    )
}

#[test]
fn el_baseline_esta_fijado_explicitamente() {
    assert!(
        config().contains("target-cpu=x86-64"),
        ".cargo/config.toml existe pero no fija target-cpu=x86-64"
    );
}

#[test]
fn nada_sube_del_baseline() {
    let texto = config();
    for prohibido in [
        "target-cpu=native",
        "x86-64-v2",
        "x86-64-v3",
        "x86-64-v4",
        "+avx2",
    ] {
        assert!(
            !texto.contains(prohibido),
            "`{prohibido}` en .cargo/config.toml: la maquina suelo (Ivy \
             Bridge) no tiene AVX2 y el binario moriria con instruccion \
             ilegal. El SIMD moderno se despacha en tiempo de ejecucion."
        );
    }
}
