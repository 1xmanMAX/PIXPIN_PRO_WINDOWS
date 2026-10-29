//! **La pantalla completa de la grabacion** (29-oct-2026, el usuario en el
//! movil: «una opcion de pantalla completa donde vea la lista de lo que voy
//! a decir, arriba las opciones y abajo la transcripcion, mas grande»).
//!
//! ⛶ en la barra la abre sobre la pantalla donde esta, y ⛶ (o Escape, si
//! tiene el teclado) vuelve a la barra. Es la misma ventana, asi que tampoco
//! sale en ninguna foto ni video (`excluir_de_capturas`): **«Captura»
//! desde aqui fotografia lo que hay debajo** sin tener que salir (en el
//! movil hay que salir; aqui no hace falta).
//!
//! - **Arriba**: el reloj grande (un clic pausa, mantenerlo termina),
//!   «Terminar», Foto, Captura, Pantalla y Clip en grande; a la derecha
//!   A− y A+ (lo grande del texto, que se recuerda), «Descartar» (dos
//!   clics, como ✕) y ⛶ para volver.
//! - **En medio, lo que voy a decir**: los temas de la reunion, para
//!   marcarlos a mano (los que marca Jev salen con su ✓ y su minuto).
//! - **Abajo, lo que se dice**: la transcripcion en vivo de la escucha
//!   ([`super::escucha`], `Vuelta::Frase`), frase a frase con su minuto, la
//!   ultima abajo. Solo se ensena: el texto de la nota sale al terminar,
//!   entero. Sin escucha (sin la clave de Jev o sin reconocedor) se dice
//!   por que.

use std::time::Duration;

use pixpin_render::{Color, Pintor, RectF};

use super::{
    Accion, Botones, Estado, Fase, OSCURO, ROJO_GRABAR, Rotulos, VERDE, boton_si, con, con_alfa,
    dentro, mas_claro, mi, minuto,
};

/// Lo que pide la pantalla completa, en logicos: mas que cualquier
/// pantalla, y `marco_para` lo recorta a su area de trabajo.
pub(super) const TODA: u32 = 100_000;
/// Lo que cambia la letra cada A− o A+.
pub(super) const PASO_LETRA: f32 = 0.15;
/// Las frases que se guardan para ensenar (las de antes ya no caben).
const FRASES: usize = 200;

/// Lo grande de la letra, dentro de lo que cabe (lo que no es un numero,
/// el de siempre).
pub(super) fn letra_valida(l: f32) -> f32 {
    if l.is_finite() {
        l.clamp(0.7, 2.5)
    } else {
        1.0
    }
}

impl Estado {
    /// **Una frase oida** en el minuto `ms`: a la transcripcion en vivo.
    pub(super) fn oida(&mut self, ms: i64, texto: String) {
        self.frases.push((ms, texto));
        if self.frases.len() > FRASES {
            let sobran = self.frases.len() - FRASES;
            self.frases.drain(..sobran);
        }
    }
}

/// Un boton con solo texto.
#[allow(clippy::too_many_arguments)]
fn boton_de_texto(
    p: &Pintor,
    botones: &mut Botones<Accion>,
    caja: RectF,
    que: Accion,
    rotulo: &str,
    fondo: Color,
    tinta: Color,
    e: f32,
) {
    let fondo = if dentro(caja, botones.raton) {
        mas_claro(fondo)
    } else {
        fondo
    };
    p.rellenar_redondeado(caja, 10.0 * e, fondo);
    let tam = 15.0 * e;
    let (tw, th) = p.medir_texto(rotulo, tam);
    p.texto(
        rotulo,
        caja.x + (caja.ancho - tw) / 2.0,
        caja.y + (caja.alto - th) / 2.0,
        tam,
        tinta,
    );
    botones.zona(caja, que);
}

/// **Pinta la pantalla completa**, de `w` x `h` pixeles a la escala `e`.
#[allow(clippy::too_many_arguments)]
pub(super) fn pintar(
    p: &Pintor,
    w: f32,
    h: f32,
    e: f32,
    estado: &Estado,
    r: &Rotulos,
    llevado: Duration,
    botones: &mut Botones<Accion>,
) {
    let tema = &OSCURO;
    // Sin las esquinas redondeadas de la barra: ocupa toda la pantalla.
    p.rellenar(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: h,
        },
        tema.isla,
    );
    let g = match &estado.fase {
        Fase::Grabando(g) => Some(g),
        _ => None,
    };
    let activo = g.is_some();
    let m = 24.0 * e;
    let y = 18.0 * e;
    let alto = 64.0 * e;

    // --- Arriba: los mandos ---------------------------------------------
    super::pintar_reloj(
        p,
        botones,
        RectF {
            x: m,
            y,
            ancho: 156.0 * e,
            alto,
        },
        g,
        (llevado, estado.manteniendo),
        30.0 * e,
        e,
    );
    // A la derecha, de derecha a izquierda: ⛶, Descartar, A+ y A−.
    let mut derecha = w - m;
    let mut a_la_derecha = |ancho: f32| {
        derecha -= ancho * e;
        let c = RectF {
            x: derecha,
            y,
            ancho: ancho * e,
            alto,
        };
        derecha -= 8.0 * e;
        c
    };
    boton_si(
        p,
        botones,
        a_la_derecha(56.0),
        Accion::Completa,
        &mi::FULLSCREEN_EXIT,
        "",
        tema.campo,
        tema.texto,
        e,
        true,
    );
    let tirar = estado.tirar_desde.is_some();
    let descartar = a_la_derecha(112.0);
    if activo {
        boton_de_texto(
            p,
            botones,
            descartar,
            Accion::Tirar,
            &r.descartar,
            if tirar { ROJO_GRABAR } else { tema.campo },
            if tirar {
                Color::BLANCO
            } else {
                tema.campo_apagado
            },
            e,
        );
    }
    let mas = a_la_derecha(56.0);
    let menos = a_la_derecha(56.0);
    boton_de_texto(
        p,
        botones,
        menos,
        Accion::LetraMenos,
        "A−",
        tema.campo,
        tema.texto,
        e,
    );
    boton_de_texto(
        p,
        botones,
        mas,
        Accion::LetraMas,
        "A+",
        tema.campo,
        tema.texto,
        e,
    );
    // Entre el reloj y eso: «Terminar» y las cuatro acciones, con su
    // nombre si caben.
    let x0 = m + 156.0 * e + 12.0 * e;
    let libre = (derecha - x0).max(0.0);
    let con_nombre = libre >= (140.0 + 4.0 * 136.0) * e;
    let (ancho_terminar, ancho_accion) = if con_nombre {
        (140.0 * e, 128.0 * e)
    } else {
        (
            (libre / 5.0 - 8.0 * e).max(40.0 * e),
            (libre / 5.0 - 8.0 * e).max(40.0 * e),
        )
    };
    let mut x = x0;
    boton_si(
        p,
        botones,
        RectF {
            x,
            y,
            ancho: ancho_terminar,
            alto,
        },
        Accion::Terminar,
        &mi::STOP,
        if con_nombre { &r.terminar } else { "" },
        ROJO_GRABAR,
        Color::BLANCO,
        e,
        activo,
    );
    x += ancho_terminar + 8.0 * e;
    for (accion, icono, rotulo, fondo, tinta) in super::acciones(estado, r) {
        boton_si(
            p,
            botones,
            RectF {
                x,
                y,
                ancho: ancho_accion,
                alto,
            },
            accion,
            icono,
            if con_nombre { &rotulo } else { "" },
            fondo,
            tinta,
            e,
            activo,
        );
        x += ancho_accion + 8.0 * e;
    }
    // La linea de debajo: lo que acaba de pasar, o la pista.
    let linea = if g.is_none() {
        r.abriendo.clone()
    } else if tirar {
        r.descartar_otra.clone()
    } else if let Some((a, _)) = &estado.aviso {
        a.clone()
    } else if g.is_some_and(|g| g.en_pausa()) {
        r.en_pausa.clone()
    } else {
        let n = super::hechas(estado);
        if n == 0 {
            r.pista_unica.clone()
        } else {
            format!("{} · {}", con(&r.imagenes, &n.to_string()), r.pista_unica)
        }
    };
    p.texto_linea(
        &linea,
        m,
        y + alto + 12.0 * e,
        14.0 * e,
        w - 2.0 * m,
        if tirar {
            ROJO_GRABAR
        } else {
            tema.campo_apagado
        },
    );

    // --- En medio y abajo -------------------------------------------------
    let letra = letra_valida(estado.letra);
    let arriba = y + alto + 44.0 * e;
    let partida = arriba + (h - arriba - m) * 0.45;
    pintar_temas(
        p,
        RectF {
            x: m,
            y: arriba,
            ancho: w - 2.0 * m,
            alto: partida - arriba - 12.0 * e,
        },
        e,
        letra,
        estado,
        r,
        botones,
    );
    // Una raya entre los dos.
    p.rellenar(
        RectF {
            x: m,
            y: partida - 6.0 * e,
            ancho: w - 2.0 * m,
            alto: 1.0 * e,
        },
        tema.campo,
    );
    pintar_frases(
        p,
        RectF {
            x: m,
            y: partida + 6.0 * e,
            ancho: w - 2.0 * m,
            alto: h - partida - 6.0 * e - m,
        },
        e,
        letra,
        estado,
        r,
    );
}

/// **Lo que voy a decir**: los temas, con su casilla (un clic la marca o
/// la desmarca a mano), su minuto y quien lo marco.
#[allow(clippy::too_many_arguments)]
fn pintar_temas(
    p: &Pintor,
    caja: RectF,
    e: f32,
    letra: f32,
    estado: &Estado,
    r: &Rotulos,
    botones: &mut Botones<Accion>,
) {
    let tema = &OSCURO;
    let titulo = match (&estado.estado_escucha, estado.temas.is_empty()) {
        (Some(s), false) => format!("{} · {s}", r.completa_temas),
        _ => r.completa_temas.clone(),
    };
    p.texto_linea(
        &titulo,
        caja.x,
        caja.y,
        14.0 * e * letra.min(1.4),
        caja.ancho,
        tema.campo_apagado,
    );
    let mut y = caja.y + 30.0 * e * letra.min(1.4);
    if estado.temas.is_empty() {
        p.texto_ajustado(
            &r.completa_sin_temas,
            caja.x,
            y,
            16.0 * e * letra,
            caja.ancho,
            tema.campo_apagado,
        );
        return;
    }
    let tam = 20.0 * e * letra;
    let alto_fila = tam * 1.7;
    let fondo = caja.y + caja.alto;
    for (i, t) in estado.temas.iter().enumerate() {
        if y + alto_fila > fondo {
            let quedan = estado.temas.len() - i;
            p.texto(
                &format!("+{quedan}"),
                caja.x,
                y,
                14.0 * e,
                tema.campo_apagado,
            );
            break;
        }
        let fila = RectF {
            x: caja.x,
            y,
            ancho: caja.ancho,
            alto: alto_fila - 4.0 * e,
        };
        let brillo = estado
            .recien_tratados
            .iter()
            .find(|(id, _)| *id == t.id)
            .map(|(_, d)| 1.0 - d.elapsed().as_secs_f32() / super::temas::BRILLO.as_secs_f32());
        if let Some(b) = brillo.filter(|b| *b > 0.0) {
            p.rellenar_redondeado(fila, 8.0 * e, con_alfa(VERDE, 0.35 * b));
        } else if dentro(fila, botones.raton) {
            p.rellenar_redondeado(fila, 8.0 * e, tema.campo);
        }
        let lado = tam * 1.05;
        p.icono(
            if t.hecho {
                &mi::CHECK_BOX
            } else {
                &mi::CHECK_BOX_OUTLINE_BLANK
            },
            RectF {
                x: fila.x + 8.0 * e,
                y: fila.y + (fila.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            if t.hecho { VERDE } else { tema.campo_apagado },
        );
        let derecha = match (t.hecho, t.ms) {
            (true, Some(ms)) if t.por_jev => format!("Jev · {}", minuto(ms)),
            (true, Some(ms)) => minuto(ms),
            (true, None) if t.por_jev => "Jev".to_string(),
            _ => String::new(),
        };
        let tam_d = tam * 0.7;
        let (dw, dh) = p.medir_texto(&derecha, tam_d);
        if !derecha.is_empty() {
            p.texto(
                &derecha,
                fila.x + fila.ancho - dw - 10.0 * e,
                fila.y + (fila.alto - dh) / 2.0,
                tam_d,
                if t.por_jev { VERDE } else { tema.campo_apagado },
            );
        }
        let texto = format!("{}. {}", i + 1, t.texto);
        let (_, th) = p.medir_texto(&texto, tam);
        p.texto_linea(
            &texto,
            fila.x + 16.0 * e + lado,
            fila.y + (fila.alto - th) / 2.0,
            tam,
            fila.ancho - lado - 36.0 * e - dw,
            if t.hecho {
                tema.campo_apagado
            } else {
                tema.texto
            },
        );
        botones.zona(fila, Accion::Tema(i));
        y += alto_fila;
    }
}

/// **Lo que se dice**: las frases de la escucha en vivo, la ultima abajo y
/// las de antes hacia arriba hasta donde quepan.
fn pintar_frases(p: &Pintor, caja: RectF, e: f32, letra: f32, estado: &Estado, r: &Rotulos) {
    let tema = &OSCURO;
    p.texto_linea(
        &r.completa_dice,
        caja.x,
        caja.y,
        14.0 * e * letra.min(1.4),
        caja.ancho,
        tema.campo_apagado,
    );
    let arriba = caja.y + 30.0 * e * letra.min(1.4);
    let tam = 18.0 * e * letra;
    if estado.frases.is_empty() {
        // Sin escucha, por que (la clave de Jev, el reconocedor); con ella,
        // que aun no se oye nada.
        let texto = if estado.escucha.is_some() {
            r.completa_sin_frases.clone()
        } else {
            estado
                .estado_escucha
                .clone()
                .unwrap_or_else(|| r.completa_sin_escucha.clone())
        };
        p.texto_ajustado(
            &texto,
            caja.x,
            arriba,
            16.0 * e * letra,
            caja.ancho,
            tema.campo_apagado,
        );
        return;
    }
    let ancho_minuto = p.medir_texto("00:00:00", tam * 0.75).0 + 16.0 * e;
    let ancho_texto = (caja.ancho - ancho_minuto).max(40.0 * e);
    let mut fondo = caja.y + caja.alto;
    let ultima = estado.frases.len() - 1;
    for (i, (ms, texto)) in estado.frases.iter().enumerate().rev() {
        let (_, th) = p.medir_texto_ajustado(texto, tam, ancho_texto);
        let alto = th + 10.0 * e;
        if fondo - alto < arriba {
            break;
        }
        fondo -= alto;
        // La ultima, mas clara: es la de ahora.
        let tinta = if i == ultima {
            tema.texto
        } else {
            con_alfa(tema.texto, 0.78)
        };
        p.texto(&minuto(*ms), caja.x, fondo, tam * 0.75, tema.campo_apagado);
        p.texto_ajustado(texto, caja.x + ancho_minuto, fondo, tam, ancho_texto, tinta);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_letra_se_queda_dentro_de_lo_que_cabe() {
        assert_eq!(letra_valida(1.0), 1.0);
        assert_eq!(letra_valida(9.0), 2.5);
        assert_eq!(letra_valida(0.1), 0.7);
        // Caso negativo: lo que no es un numero vuelve a la de siempre.
        assert_eq!(letra_valida(f32::NAN), 1.0);
        assert_eq!(letra_valida(f32::INFINITY), 1.0);
    }
}
