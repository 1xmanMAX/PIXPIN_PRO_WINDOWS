//! **Las tablas anchas, cada una con su barra** (H12): una tabla conserva
//! el ancho que pide su texto aunque pase del de la columna, y si no cabe
//! en la ventana se corre de lado ella sola, con su barra fina debajo, como
//! las tablas de Claude. El texto de alrededor y las demas tablas no se
//! mueven.
//!
//! # Por que asi (medido en el `RichEdit` de Windows)
//!
//! El control solo tiene un desplazamiento de lado para todo el documento,
//! y una fila de tabla no puede empezar a la izquierda del borde de lo
//! escrito (`\trleft` negativo: el control lo recorta a 0 y ensancha la
//! primera celda). Lo que si se puede es correr una fila a la DERECHA
//! (`ITextRow::SetIndent` + `Apply`, unos 0,3 ms por fila). Asi que, cuando
//! hay alguna tabla mas ancha que la ventana, el control se pone mas ancho
//! que el papel por la izquierda (un **hueco** que no se ve, fuera del
//! marco) y todos los parrafos llevan ese hueco de sangria de mas: el texto
//! se ve donde siempre. Cada tabla ancha se sangra `hueco - desplazamiento`:
//! sin desplazar empieza en el borde del papel, y al desplazarla su
//! principio se mete en el hueco. Una tabla que cabe va centrada.
//!
//! La sangria de una fila tiene tope (medido: 21 500 veinteavos se aceptan
//! y 22 000 no; son 15 pulgadas, 1 440 px a 96 ppp), asi que el hueco es
//! ese y ninguna tabla se hace mas ancha que lo que deja ver
//! ([`TOPE_TABLA_PX`]).
//!
//! Se descartaron un `RichEdit` hijo por tabla (deshacer, elegir, copiar,
//! buscar y los comentarios partidos entre controles; el `.md` tendria que
//! salir de varios sitios) y un objeto OLE incrustado que pintara y
//! editara la tabla (rehacer el IME, la seleccion y Tab a mano). Asi la
//! tabla sigue siendo del control: IME, acentos, Tab, deshacer, copiar y
//! los comentarios funcionan igual que antes.

use std::cell::Cell;

/// El hueco de la izquierda, en veinteavos de punto (15 pulgadas): el tope
/// de la sangria de una fila, medido.
pub const HUECO_TWIPS: i32 = 21_500;

/// Lo mas ancha que se hace una tabla, en pixeles a 96 ppp: lo que se ve
/// en la ventana mas estrecha (unos 360 px de papel) mas el hueco, para que
/// siempre se pueda llegar a su ultima columna.
pub const TOPE_TABLA_PX: i32 = 1_780;

thread_local! {
    /// Hay hueco (alguna tabla ancha) y para que ppp.
    static ABIERTO: Cell<Option<i32>> = const { Cell::new(None) };
    /// El margen de la columna de texto a cada lado, en veinteavos: el
    /// hueco que no se ve es el de siempre menos este, asi lo escrito queda
    /// a la sangria entera y una tabla ancha sin desplazar empieza donde
    /// el texto (el usuario, 1-oct: «que se puedan mover hasta que esten
    /// alineadas con el texto, no al final del viewport»).
    static SOBRA: Cell<i32> = const { Cell::new(0) };
    /// El margen de la derecha, en veinteavos: el mismo, o mas si las
    /// tarjetas de los comentarios ocupan el margen del papel.
    static SOBRA_DER: Cell<i32> = const { Cell::new(0) };
}

/// Lo que no se ve del control a la izquierda del papel, en pixeles de la
/// pantalla (0 sin tablas anchas).
pub fn hueco() -> i32 {
    ABIERTO
        .with(|a| a.get())
        .map_or(0, |ppp| hueco_twips() * ppp / 1440)
}

/// Ese hueco en veinteavos: lo que los parrafos llevan de sangria de mas.
pub fn hueco_twips() -> i32 {
    if ABIERTO.with(|a| a.get()).is_some() {
        (HUECO_TWIPS - SOBRA.with(|s| s.get())).max(0)
    } else {
        0
    }
}

/// La sangria de lo escrito (y de una tabla ancha sin desplazar) desde el
/// borde del control, en pixeles: el hueco y el margen de la columna.
pub fn hueco_lleno() -> i32 {
    ABIERTO.with(|a| a.get()).map_or(0, hueco_de)
}

/// Pone el hueco (`true`) o lo quita, para los `ppp` de la pantalla.
pub fn poner_hueco(si: bool, ppp: i32) {
    ABIERTO.with(|a| a.set(si.then_some(ppp)));
}

/// El margen de la columna de texto, en veinteavos (ver [`SOBRA`]). `true`
/// si cambio con el hueco abierto: el control se tiene que recolocar.
pub fn poner_sobra(twips: i32) -> bool {
    let twips = twips.clamp(0, HUECO_TWIPS);
    let antes = SOBRA.with(|s| s.replace(twips));
    antes != twips && ABIERTO.with(|a| a.get()).is_some()
}

/// Pone el margen de la derecha de la columna, en veinteavos.
pub fn poner_sobra_der(twips: i32) {
    SOBRA_DER.with(|s| s.set(twips.max(0)));
}

/// El margen de la derecha en veinteavos.
pub fn sobra_der_twips() -> i32 {
    SOBRA_DER.with(|s| s.get())
}

/// El margen de la derecha en pixeles, para unos `ppp`.
pub fn sobra_der_px(ppp: i32) -> i32 {
    sobra_der_twips() * ppp / 1440
}

/// El margen de la columna de texto en pixeles, para unos `ppp`.
pub fn sobra_px(ppp: i32) -> i32 {
    SOBRA.with(|s| s.get()) * ppp / 1440
}

/// El hueco entero en pixeles para unos `ppp`.
pub fn hueco_de(ppp: i32) -> i32 {
    HUECO_TWIPS * ppp / 1440
}

/// **Donde va una tabla**: su sangria (en pixeles, desde el borde de lo
/// escrito del control), cuanto esta desplazada y cuanto puede estarlo.
///
/// Una ancha va a la izquierda con esa sangria. Una que cabe va centrada
/// (`\trqc`): el control la centra en todo lo escrito, hueco incluido, y
/// una sangria corre una fila centrada la MITAD de lo que mide (medido), asi
/// que con el hueco entero de sangria queda centrada en lo que se ve. (Una
/// fila a la izquierda no puede llevar mas sangria que el hueco: ese tope
/// no deja centrarla asi.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colocacion {
    pub sangria: i32,
    pub desplazamiento: i32,
    pub maximo: i32,
}

impl Colocacion {
    /// Si la tabla no cabe y lleva barra.
    pub fn ancha(&self) -> bool {
        self.maximo > 0
    }
}

/// Coloca una tabla de `ancho` en un papel de `visible` (lo que se ve de lo
/// escrito, sin el hueco) con un `hueco` a la izquierda, desplazada lo que
/// se pide (recortado a lo que se puede).
pub fn colocar(ancho: i32, visible: i32, hueco: i32, desplazamiento: i32) -> Colocacion {
    if ancho <= visible || hueco <= 0 {
        // Cabe (o aun no hay hueco): centrada en lo que se ve.
        return Colocacion {
            sangria: hueco.max(0),
            desplazamiento: 0,
            maximo: 0,
        };
    }
    let maximo = (ancho - visible).min(hueco);
    let d = desplazamiento.clamp(0, maximo);
    Colocacion {
        sangria: hueco - d,
        desplazamiento: d,
        maximo,
    }
}

/// **Una tabla en la columna de texto**: `columna` es lo ancho que se ve
/// de ella (la de texto, no el papel), `lleno` la sangria de lo escrito
/// (donde empieza el texto, desde el borde del control) y `oculto` el
/// hueco que no se ve. Una ancha sin desplazar empieza donde el texto y se
/// puede correr hasta que su borde derecho llegue al del texto; la que
/// cabe va centrada (con la sangria del hueco, ver [`Colocacion`]).
pub fn colocar_en_columna(
    ancho: i32,
    columna: i32,
    lleno: i32,
    oculto: i32,
    desplazamiento: i32,
) -> Colocacion {
    if ancho <= columna || lleno <= 0 {
        return Colocacion {
            sangria: oculto.max(0),
            desplazamiento: 0,
            maximo: 0,
        };
    }
    let maximo = (ancho - columna).min(lleno);
    let d = desplazamiento.clamp(0, maximo);
    Colocacion {
        sangria: lleno - d,
        desplazamiento: d,
        maximo,
    }
}

/// **El pulgar de la barra**: su x y su ancho dentro de la barra que va de
/// `x` a `x + an`, para una tabla de `ancho` de la que se ven `an`.
pub fn pulgar(x: i32, an: i32, ancho: i32, c: &Colocacion, minimo: i32) -> (i32, i32) {
    if !c.ancha() || ancho <= 0 {
        return (x, an);
    }
    let largo = (an as i64 * an as i64 / ancho as i64) as i32;
    let largo = largo.clamp(minimo.min(an), an);
    let recorrido = an - largo;
    let px = x + (recorrido as i64 * c.desplazamiento as i64 / c.maximo as i64) as i32;
    (px, largo)
}

/// El desplazamiento que deja el pulgar con su principio en `px` (al
/// arrastrarlo).
pub fn desde_pulgar(px: i32, x: i32, an: i32, ancho: i32, c: &Colocacion, minimo: i32) -> i32 {
    let (_, largo) = pulgar(x, an, ancho, c, minimo);
    let recorrido = an - largo;
    if recorrido <= 0 {
        return 0;
    }
    let d = ((px - x) as i64 * c.maximo as i64 / recorrido as i64) as i32;
    d.clamp(0, c.maximo)
}

/// El desplazamiento que deja ver de `izq` a `der` (pixeles en pantalla de
/// lo de dentro de la tabla, con el desplazamiento `c`) dentro de lo que va
/// de `vis_izq` a `vis_der`: lo justo, sin moverla si ya se ve.
pub fn para_ver(c: &Colocacion, izq: i32, der: i32, vis_izq: i32, vis_der: i32) -> i32 {
    let mut d = c.desplazamiento;
    if der > vis_der {
        d += der - vis_der;
    }
    if izq - (d - c.desplazamiento) < vis_izq {
        d -= vis_izq - (izq - (d - c.desplazamiento));
    }
    d.clamp(0, c.maximo)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_tabla_que_cabe_va_centrada_y_no_lleva_barra() {
        let c = colocar(500, 900, 0, 0);
        assert_eq!(
            c,
            Colocacion {
                sangria: 0,
                desplazamiento: 0,
                maximo: 0
            }
        );
        assert!(!c.ancha());
        // Con hueco, centrada con el hueco de sangria (corre la mitad).
        assert_eq!(colocar(500, 900, 1440, 0).sangria, 1440);
        // Y aunque se pida desplazarla, no se mueve.
        assert_eq!(colocar(500, 900, 1440, 300).desplazamiento, 0);
    }

    #[test]
    fn una_tabla_ancha_empieza_en_el_borde_y_se_desplaza_hasta_su_final() {
        let c = colocar(1500, 900, 1440, 0);
        assert_eq!((c.sangria, c.maximo), (1440, 600));
        assert!(c.ancha());
        let d = colocar(1500, 900, 1440, 250);
        assert_eq!((d.sangria, d.desplazamiento), (1440 - 250, 250));
        // Caso negativo: no se pasa de su final ni de su principio.
        assert_eq!(colocar(1500, 900, 1440, 5000).desplazamiento, 600);
        assert_eq!(colocar(1500, 900, 1440, -40).desplazamiento, 0);
    }

    #[test]
    fn el_desplazamiento_no_pasa_del_hueco() {
        let c = colocar(4000, 900, 1440, 9999);
        assert_eq!(c.maximo, 1440);
        assert_eq!(c.sangria, 0, "la fila no puede empezar antes del borde");
    }

    #[test]
    fn sin_hueco_una_tabla_ancha_no_se_puede_desplazar() {
        let c = colocar(1500, 900, 0, 100);
        assert_eq!((c.maximo, c.desplazamiento, c.sangria), (0, 0, 0));
    }

    #[test]
    fn el_pulgar_mide_lo_que_se_ve_y_recorre_la_barra() {
        let c = colocar(2000, 1000, 1440, 0);
        assert_eq!(pulgar(100, 1000, 2000, &c, 30), (100, 500));
        let fin = colocar(2000, 1000, 1440, 1000);
        assert_eq!(pulgar(100, 1000, 2000, &fin, 30), (600, 500));
        // Una tabla enorme no deja un pulgar invisible.
        let c = colocar(100_000, 1000, 1440, 0);
        assert_eq!(pulgar(0, 1000, 100_000, &c, 30).1, 30);
        // Arrastrar el pulgar y volver a leerlo da lo mismo.
        let medio = colocar(2000, 1000, 1440, 500);
        let (px, _) = pulgar(100, 1000, 2000, &medio, 30);
        assert_eq!(desde_pulgar(px, 100, 1000, 2000, &medio, 30), 500);
        assert_eq!(
            desde_pulgar(-500, 100, 1000, 2000, &medio, 30),
            0,
            "no se sale por la izquierda"
        );
        assert_eq!(
            desde_pulgar(5000, 100, 1000, 2000, &medio, 30),
            1000,
            "ni por la derecha"
        );
    }

    #[test]
    fn para_ver_mueve_lo_justo_y_no_mueve_si_ya_se_ve() {
        let c = colocar(2000, 1000, 1440, 0);
        assert_eq!(para_ver(&c, 200, 400, 0, 1000), 0, "ya se ve");
        assert_eq!(
            para_ver(&c, 1100, 1200, 0, 1000),
            200,
            "a la derecha: lo justo"
        );
        let d = colocar(2000, 1000, 1440, 600);
        assert_eq!(para_ver(&d, -300, -100, 0, 1000), 300, "a la izquierda");
        // Caso negativo: no pasa del maximo.
        assert_eq!(para_ver(&c, 5000, 5100, 0, 1000), 1000);
    }

    #[test]
    fn el_hueco_crece_con_los_puntos_por_pulgada_y_se_quita() {
        poner_hueco(true, 96);
        assert_eq!(hueco(), 1433);
        assert_eq!(hueco_twips(), HUECO_TWIPS);
        poner_hueco(true, 144);
        assert_eq!(hueco(), 2150);
        poner_hueco(false, 144);
        assert_eq!((hueco(), hueco_twips()), (0, 0));
        // La tabla mas ancha se ve entera en la ventana mas estrecha.
        assert!(TOPE_TABLA_PX <= 360 + hueco_de(96));
    }
}
