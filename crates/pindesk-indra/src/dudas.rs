//! **Lineas dudosas** (`IndraDoubt.kt`): las que el reconocedor lee con una
//! confianza entre [`SUELO_DUDA`](crate::canal::SUELO_DUDA) y
//! [`CONFIANZA_MIN`](crate::canal::CONFIANZA_MIN). El original las vaciaba;
//! el escaner las conserva, marcadas, si no son basura evidente: en campos
//! vacios, casillas y rayas el reconocedor lee «-», «I», «1», «•» o letras
//! sueltas; una lectura con dos letras o cifras y altura de renglon se
//! queda. Banco del escaner (154 lineas bajo 0,5 en 54 imagenes): pasan 34,
//! con el mismo % de palabras.

const BASURA: &str = "Il1|!-–—_=~.,:;'\"`•·*()[]{}<>/\\^°ºoO0 ";

/// Si se conserva una linea dudosa. `alto_rel` = alto de la linea / la
/// mediana de la pagina.
pub fn conservar(texto: &str, confianza: f32, alto_rel: f32, suelo: f32) -> bool {
    let t = texto.trim();
    if confianza < suelo || t.is_empty() {
        return false;
    }
    if t.chars().filter(|c| c.is_alphanumeric()).count() < 2 {
        return false;
    }
    if t.chars().all(|c| BASURA.contains(c)) {
        return false;
    }
    // Trazos (muy bajos) o varios renglones fundidos (muy altos).
    if !(0.45..=2.6).contains(&alto_rel) {
        return false;
    }
    let trozos: Vec<&str> = t.split(' ').filter(|w| !w.is_empty()).collect();
    let sin_cifras = !t.chars().any(|c| c.is_numeric());
    if sin_cifras && trozos.iter().all(|w| w.chars().count() <= 2) {
        return false;
    }
    if confianza < 0.4 && sin_cifras && !trozos.iter().any(|w| w.chars().count() >= 4) {
        return false;
    }
    true
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn dudosas_sin_basura_evidente() {
        // IndraAfinadoTest.dudosasSinBasuraEvidente
        let s = 0.3;
        assert!(conservar("14,0", 0.42, 1.2, s));
        assert!(conservar("Ing. Sismorresistente", 0.45, 1.0, s));
        assert!(conservar("Erick Horev", 0.35, 1.0, s));
        assert!(!conservar("-", 0.45, 1.0, s), "raya");
        assert!(!conservar("I", 0.3, 1.0, s), "casilla");
        assert!(!conservar("...", 0.45, 1.0, s), "puntos");
        assert!(!conservar("gi", 0.44, 1.0, s), "letras sueltas");
        assert!(!conservar("Texto largo", 0.2, 1.0, s), "bajo el suelo");
        assert!(
            !conservar("la dei son de", 0.45, 3.5, s),
            "renglones fundidos"
        );
        assert!(!conservar("an t", 0.35, 1.0, s), "muy baja y corta");
        assert!(conservar("27 50", 0.35, 1.0, s), "cifras a baja confianza");
    }
}
