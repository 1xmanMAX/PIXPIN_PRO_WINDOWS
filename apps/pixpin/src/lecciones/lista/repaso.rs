//! **El repaso de hoy**, de una en una (maqueta `Lecciones2-repaso.dc.html`).
//!
//! Lo del movil (`ParaRepasar` de `LeccionesActivity.kt`): se ensena la
//! situacion, se intenta recordar que harias **antes** de destaparlo
//! —recordar fija mucho mas que releer— y se contesta. Lo que anade el v2:
//!
//! - una caja para escribir (o dictar, Ctrl+D) tu respuesta, que se pone al
//!   lado de lo apuntado para compararla. No se guarda: el formato de la
//!   leccion es el del movil y no tiene donde (pendiente si Android lo
//!   anade);
//! - tres botones en vez de dos (1 «Lo recordaba», 2 «A medias», 3 «Lo
//!   olvide»), y cada uno dice cuando vuelve;
//! - el progreso («1 de 2») y «Saltar esta» (S).

use pixpin_lecciones::{Leccion, Nota, Repaso as Espaciado};
use pixpin_render::icono::material as mi;
use pixpin_render::{Pintor, RectF};

use super::super::ui::{self, Campo, con_alfa, v2};
use super::{Accion, Estado, Foco, t1, t2};
use crate::caja_dibujo::hex;
use pixpin_store::Catalogo;

pub struct Repaso {
    /// Los ids de las que tocan, en orden.
    cola: Vec<String>,
    /// La de ahora.
    i: usize,
    /// Cuantas se contestaron (las saltadas no cuentan).
    pub hechas: usize,
    pub respuesta: Campo,
    pub mostrada: bool,
    pub nota: Option<Nota>,
}

impl Repaso {
    pub fn nuevo(cola: Vec<String>) -> Repaso {
        Repaso {
            cola,
            i: 0,
            hechas: 0,
            respuesta: Campo::default(),
            mostrada: false,
            nota: None,
        }
    }

    /// El id de la de ahora; `None` al acabar.
    pub fn actual(&self) -> Option<&str> {
        self.cola.get(self.i).map(String::as_str)
    }

    #[cfg(test)]
    pub fn total(&self) -> usize {
        self.cola.len()
    }

    /// Cual va (desde 1) de cuantas.
    pub fn progreso(&self) -> (usize, usize) {
        ((self.i + 1).min(self.cola.len()), self.cola.len())
    }

    /// A la siguiente. `contada`: si se contesto (y no se salto).
    pub fn pasar(&mut self, contada: bool) {
        if self.i < self.cola.len() {
            self.i += 1;
            if contada {
                self.hechas += 1;
            }
        }
        self.respuesta = Campo::default();
        self.mostrada = false;
        self.nota = None;
    }

    /// En que paso va: 1 pensar, 2 mirar, 3 contestar.
    pub fn paso(&self) -> u8 {
        if self.mostrada {
            3
        } else if !self.respuesta.texto.trim().is_empty() {
            2
        } else {
            1
        }
    }
}

/// «manana», «en 7 dias», «en 30 dias»: cuando vuelve una leccion.
pub fn cuando_vuelve(tx: &Catalogo, dias: i64) -> String {
    if dias <= 1 {
        tx.t("lec2-vuelve-manana")
    } else {
        t1(&tx, "lec2-vuelve-en", "dias", dias)
    }
}

/// Lo que se pregunta: la situacion (`quePaso`), o sus etiquetas, o el area,
/// como el movil.
pub fn situacion(tx: &Catalogo, l: &Leccion) -> String {
    if !l.que_paso.trim().is_empty() {
        return l.que_paso.clone();
    }
    let t = l
        .todas_las_etiquetas()
        .iter()
        .map(|t| format!("#{t}"))
        .collect::<Vec<_>>()
        .join(" ");
    if !t.is_empty() {
        return t1(&tx, "lec2-repaso-cuando", "pista", t);
    }
    if !l.area.trim().is_empty() {
        return t1(&tx, "lec2-repaso-cuando", "pista", l.area.clone());
    }
    tx.t("lec2-repaso-recuerda")
}

/// Pinta el repaso en toda la ventana.
pub fn pintar(e: &mut Estado, p: &Pintor, w: f32, h: f32, s: f32) {
    let tx = e.textos.clone();
    let ancho = (w - 56.0 * s).min(900.0 * s);
    let x0 = (w - ancho) / 2.0;
    let mut y = 20.0 * s;
    e.botones.zona(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: 64.0 * s,
        },
        Accion::Mover,
    );
    let Some(r) = e.repaso.as_ref() else {
        return;
    };
    let actual = r.actual().map(str::to_string);
    let (va, total) = r.progreso();
    let paso = r.paso();
    let (hechas, mostrada, nota) = (r.hechas, r.mostrada, r.nota);

    // Cabecera: titulo, «1 de 2», la barra de progreso y Salir.
    let cy = y + 20.0 * s;
    p.icono(
        &mi::LIBRARY_ADD,
        RectF {
            x: x0,
            y: cy - 10.0 * s,
            ancho: 20.0 * s,
            alto: 20.0 * s,
        },
        v2::AMARILLO,
    );
    let titulo = tx.t("lec2-repaso-titulo");
    ui::negrita(
        p,
        &titulo,
        x0 + 34.0 * s,
        cy - 11.0 * s,
        16.0 * s,
        400.0 * s,
        v2::TEXTO,
    );
    let tw = ui::medir_negrita(p, &titulo, 16.0 * s, 400.0 * s).0;
    let de = t2(
        &tx,
        "lec2-repaso-de",
        "va",
        va as i64,
        "total",
        total as i64,
    );
    let de = if actual.is_some() { de } else { String::new() };
    let mut xx = x0 + 34.0 * s + tw + 14.0 * s;
    p.texto(&de, xx, cy - 9.0 * s, 14.0 * s, v2::SUAVE);
    xx += p.medir_texto(&de, 14.0 * s).0 + 14.0 * s;
    let salir_w = ui::ancho_de_boton(p, false, &tx.t("lec2-salir"), Some("Esc"), s);
    let salir = RectF {
        x: x0 + ancho - salir_w,
        y: cy - 20.0 * s,
        ancho: salir_w,
        alto: 40.0 * s,
    };
    let barra = RectF {
        x: xx,
        y: cy - 3.0 * s,
        ancho: (salir.x - 14.0 * s - xx).max(0.0),
        alto: 6.0 * s,
    };
    p.rellenar_redondeado(barra, 3.0 * s, con_alfa(v2::BLANCO, 0.10));
    if total > 0 {
        let hueco = 4.0 * s;
        let tramo = (barra.ancho - hueco * (total - 1) as f32) / total as f32;
        let hechos = if actual.is_some() { va - 1 } else { total };
        for k in 0..total {
            let t = RectF {
                x: barra.x + k as f32 * (tramo + hueco),
                ancho: tramo.max(0.0),
                ..barra
            };
            if k < hechos || (k == hechos && actual.is_some()) {
                p.rellenar_redondeado(
                    t,
                    3.0 * s,
                    if k < hechos {
                        v2::AMARILLO
                    } else {
                        con_alfa(v2::AMARILLO, 0.45)
                    },
                );
            }
        }
    }
    ui::boton_v2(
        p,
        &mut e.botones,
        salir,
        Accion::SalirRepaso,
        None,
        &tx.t("lec2-salir"),
        Some("Esc"),
        None,
        v2::SUAVE,
        s,
    );
    y += 52.0 * s;

    let Some(id) = actual else {
        // Terminado.
        let caja = RectF {
            x: x0,
            y: y + 40.0 * s,
            ancho,
            alto: 220.0 * s,
        };
        p.rellenar_redondeado(caja, 18.0 * s, v2::COLUMNA);
        let t = tx.t("lec2-repaso-hecho");
        let (tw, _) = ui::medir_negrita(p, &t, 22.0 * s, ancho);
        ui::negrita(
            p,
            &t,
            x0 + (ancho - tw) / 2.0,
            caja.y + 50.0 * s,
            22.0 * s,
            ancho,
            v2::TEXTO,
        );
        let sub = t1(&tx, "lec2-repaso-hechas", "n", hechas as i64);
        let (sw, _) = p.medir_texto(&sub, 14.0 * s);
        p.texto(
            &sub,
            x0 + (ancho - sw) / 2.0,
            caja.y + 92.0 * s,
            14.0 * s,
            v2::APAGADO,
        );
        let rot = tx.t("lec2-repaso-volver");
        let bw = ui::ancho_de_boton(p, false, &rot, Some("Enter"), s);
        let b = RectF {
            x: x0 + (ancho - bw) / 2.0,
            y: caja.y + 140.0 * s,
            ancho: bw,
            alto: 44.0 * s,
        };
        ui::boton_v2(
            p,
            &mut e.botones,
            b,
            Accion::SalirRepaso,
            None,
            &rot,
            Some("Enter"),
            Some(v2::AZUL),
            v2::BLANCO,
            s,
        );
        return;
    };
    let Some(l) = e
        .todas
        .iter()
        .find(|x| x.leccion.id == id)
        .map(|x| x.leccion.clone())
    else {
        return;
    };

    // Los tres pasos.
    let pasos = [
        tx.t("lec2-paso-pensar"),
        tx.t("lec2-paso-mirar"),
        tx.t("lec2-paso-recordar"),
    ];
    let mut px = x0 + 34.0 * s;
    for (k, t) in pasos.iter().enumerate() {
        let n = k as u8 + 1;
        let (fondo, tinta, signo) = if n < paso || (n == 3 && nota.is_some()) {
            (hex(0x248a3d), v2::BLANCO, "✓".to_string())
        } else if n == paso {
            (v2::AZUL, v2::BLANCO, n.to_string())
        } else {
            (hex(0x3a3a3c), v2::SUAVE, n.to_string())
        };
        let c = (px + 10.0 * s, y + 10.0 * s);
        p.circulo(c, 10.0 * s, fondo);
        let (sw, sh) = p.medir_texto(&signo, 11.0 * s);
        p.texto(&signo, c.0 - sw / 2.0, c.1 - sh / 2.0, 11.0 * s, tinta);
        let color = if n == paso { v2::TEXTO } else { v2::APAGADO };
        p.texto(t, px + 26.0 * s, y + 2.0 * s, 12.5 * s, color);
        px += 26.0 * s + p.medir_texto(t, 12.5 * s).0 + 22.0 * s;
    }
    y += 34.0 * s;

    // La tarjeta.
    let pie = 62.0 * s;
    let tarjeta = RectF {
        x: x0,
        y,
        ancho,
        alto: (h - y - pie - 14.0 * s).max(200.0 * s),
    };
    p.rellenar_redondeado(tarjeta, 18.0 * s, con_alfa(v2::BLANCO, 0.08));
    p.rellenar_redondeado(ui::encoger(tarjeta, 1.0 * s), 17.0 * s, v2::COLUMNA);
    let pad = 22.0 * s;
    let ix = tarjeta.x + pad;
    let iw = tarjeta.ancho - 2.0 * pad;
    let mut ty = tarjeta.y + 20.0 * s;

    // La situacion.
    ui::negrita(
        p,
        &tx.t("lec2-situacion").to_uppercase(),
        ix,
        ty,
        12.0 * s,
        iw,
        v2::CIAN,
    );
    ty += 22.0 * s;
    let sit = situacion(&tx, &l);
    let (_, hs) = ui::medir_negrita(p, &sit, 19.0 * s, iw);
    let hs = hs.min(19.0 * s * 1.4 * 3.0);
    p.con_recorte(
        RectF {
            x: ix,
            y: ty,
            ancho: iw,
            alto: hs,
        },
        |p| ui::negrita(p, &sit, ix, ty, 19.0 * s, iw, v2::TEXTO),
    );
    ty += hs + 6.0 * s;
    let mut sub = Vec::new();
    if !l.area.trim().is_empty() {
        sub.push(l.area.clone());
    }
    sub.push(tx.t(nombre_de_gravedad(l.gravedad)));
    if !l.repeticiones.is_empty() {
        sub.push(t1(&tx, "lec2-repaso-veces", "n", l.veces_que_paso() as i64));
    }
    p.texto(&sub.join(" · "), ix, ty, 13.0 * s, v2::APAGADO);
    ty += 30.0 * s;

    // Las dos columnas: tu respuesta y lo apuntado.
    let botones_alto = 108.0 * s;
    let col_alto = (tarjeta.y + tarjeta.alto - 20.0 * s - botones_alto - ty).max(120.0 * s);
    let cw = (iw - 14.0 * s) / 2.0;
    let izq = RectF {
        x: ix,
        y: ty,
        ancho: cw,
        alto: col_alto,
    };
    let der = RectF {
        x: ix + cw + 14.0 * s,
        y: ty,
        ancho: cw,
        alto: col_alto,
    };

    ui::negrita(
        p,
        &tx.t("lec2-que-harias").to_uppercase(),
        izq.x,
        izq.y,
        12.0 * s,
        cw,
        v2::SUAVE,
    );
    let boton_mostrar = 44.0 * s;
    let caja = RectF {
        x: izq.x,
        y: izq.y + 22.0 * s,
        ancho: cw,
        alto: (col_alto - 22.0 * s - boton_mostrar - 10.0 * s).max(80.0 * s),
    };
    let foco = e.foco == Some(Foco::Respuesta);
    if foco {
        p.rellenar_redondeado(caja, 12.0 * s, v2::ELEGIDO);
        p.rellenar_redondeado(ui::encoger(caja, 2.0 * s), 10.0 * s, v2::CAJA);
    } else {
        p.rellenar_redondeado(caja, 12.0 * s, con_alfa(v2::BLANCO, 0.10));
        p.rellenar_redondeado(ui::encoger(caja, 1.0 * s), 11.0 * s, v2::CAJA);
    }
    e.botones.zona(caja, Accion::FocoRespuesta);
    let texto_caja = RectF {
        x: caja.x + 14.0 * s,
        y: caja.y + 12.0 * s,
        ancho: caja.ancho - 28.0 * s,
        alto: caja.alto - 60.0 * s,
    };
    let pista = tx.t("lec2-escribe-o-dicta");
    if let Some(r) = e.repaso.as_ref() {
        p.con_recorte(texto_caja, |p| {
            r.respuesta.pintar_texto(
                p,
                texto_caja.x,
                texto_caja.y,
                texto_caja.ancho,
                15.0 * s,
                foco,
                &pista,
                s,
            );
        });
    }
    // El dictado de la respuesta.
    let grabando = matches!(e.dictado, super::super::dictar::Dictado::Grabando(_))
        && e.dictando == Foco::Respuesta;
    let estado = if e.dictando == Foco::Respuesta {
        e.dictado.estado()
    } else {
        None
    };
    let dw = ui::ancho_de_boton(p, true, &tx.t("lec2-dictar"), Some("Ctrl D"), s);
    let dictar = RectF {
        x: caja.x + caja.ancho - dw - 10.0 * s,
        y: caja.y + caja.alto - 46.0 * s,
        ancho: dw,
        alto: 36.0 * s,
    };
    ui::boton_v2(
        p,
        &mut e.botones,
        dictar,
        Accion::DictarRespuesta,
        Some(&mi::MIC),
        &tx.t("lec2-dictar"),
        Some("Ctrl D"),
        Some(if grabando { v2::ROJO } else { hex(0x3a3a3c) }),
        v2::TEXTO,
        s,
    );
    if let Some(t) = estado {
        p.texto_linea(
            &t,
            caja.x + 14.0 * s,
            dictar.y + 10.0 * s,
            12.0 * s,
            dictar.x - caja.x - 24.0 * s,
            if grabando { v2::ROJO } else { v2::APAGADO },
        );
    }
    // Mostrar la respuesta: Enter escribiendo, Espacio si no.
    let mb = RectF {
        x: izq.x,
        y: caja.y + caja.alto + 10.0 * s,
        ancho: cw,
        alto: boton_mostrar,
    };
    let tecla = if foco { "Enter" } else { "Espacio" };
    p.rellenar_redondeado(mb, 12.0 * s, con_alfa(v2::ELEGIDO, 0.40));
    p.rellenar_redondeado(
        ui::encoger(mb, 1.0 * s),
        11.0 * s,
        if crate::ventanita::dentro(mb, e.botones.raton) {
            hex(0x1d3550)
        } else {
            hex(0x182a3e)
        },
    );
    let rot = if mostrada {
        tx.t("lec2-mostrada")
    } else {
        tx.t("lec2-mostrar")
    };
    let bw = ui::ancho_de_boton(p, true, &rot, (!mostrada).then_some(tecla), s);
    let mut bx = mb.x + (mb.ancho - bw) / 2.0 + 14.0 * s;
    let bcy = mb.y + mb.alto / 2.0;
    p.icono(
        &mi::LIGHTBULB,
        RectF {
            x: bx,
            y: bcy - 8.0 * s,
            ancho: 16.0 * s,
            alto: 16.0 * s,
        },
        v2::CIAN,
    );
    bx += 26.0 * s;
    let (rw, rh) = p.medir_texto(&rot, 14.0 * s);
    p.texto(&rot, bx, bcy - rh / 2.0, 14.0 * s, v2::CIAN);
    if !mostrada {
        ui::chapa(p, tecla, bx + rw + 8.0 * s, bcy, v2::CIAN, hex(0x1f3550), s);
    }
    e.botones.zona(mb, Accion::Mostrar);

    // Lo que apuntaste.
    p.icono(
        &mi::CHECK_CIRCLE,
        RectF {
            x: der.x,
            y: der.y - 1.0 * s,
            ancho: 14.0 * s,
            alto: 14.0 * s,
        },
        v2::VERDE,
    );
    ui::negrita(
        p,
        &tx.t("lec2-lo-que-apuntaste").to_uppercase(),
        der.x + 20.0 * s,
        der.y,
        12.0 * s,
        cw,
        v2::VERDE,
    );
    let ap = RectF {
        x: der.x,
        y: der.y + 22.0 * s,
        ancho: cw,
        alto: (col_alto - 22.0 * s - 50.0 * s).max(60.0 * s),
    };
    p.rellenar_redondeado(ap, 12.0 * s, con_alfa(v2::VERDE, 0.10));
    p.rellenar(
        RectF {
            x: ap.x,
            y: ap.y + 6.0 * s,
            ancho: 3.0 * s,
            alto: ap.alto - 12.0 * s,
        },
        v2::VERDE,
    );
    let aw = ap.ancho - 28.0 * s;
    if mostrada {
        let respuesta = if l.proxima.trim().is_empty() {
            l.titulo.clone()
        } else {
            l.proxima.clone()
        };
        p.con_recorte(ap, |p| {
            p.texto_ajustado(
                &respuesta,
                ap.x + 14.0 * s,
                ap.y + 12.0 * s,
                16.0 * s,
                aw,
                v2::TEXTO,
            );
            let (_, hr) = p.medir_texto_ajustado(&respuesta, 16.0 * s, aw);
            if !l.por_que.trim().is_empty() {
                let pq = t1(&tx, "lec2-por-que-paso", "causa", l.por_que.clone());
                p.texto_ajustado(
                    &pq,
                    ap.x + 14.0 * s,
                    ap.y + 20.0 * s + hr,
                    13.0 * s,
                    aw,
                    hex(0xaeaeb2),
                );
            }
        });
    } else {
        let t = tx.t("lec2-oculta");
        p.texto_ajustado(
            &t,
            ap.x + 14.0 * s,
            ap.y + 12.0 * s,
            14.0 * s,
            aw,
            v2::APAGADO,
        );
        e.botones.zona(ap, Accion::Mostrar);
    }
    let enlace = tx.t("lec2-abrir-completa");
    let ew = p.medir_texto(&enlace, 13.0 * s).0 + 22.0 * s;
    let el = RectF {
        x: der.x,
        y: ap.y + ap.alto + 6.0 * s,
        ancho: ew,
        alto: 40.0 * s,
    };
    let encima = crate::ventanita::dentro(el, e.botones.raton);
    p.texto(
        &enlace,
        el.x,
        el.y + 11.0 * s,
        13.0 * s,
        if encima { hex(0xa0e4ff) } else { v2::CIAN },
    );
    p.icono(
        &mi::ARROW_FORWARD,
        RectF {
            x: el.x + ew - 16.0 * s,
            y: el.y + 13.0 * s,
            ancho: 14.0 * s,
            alto: 14.0 * s,
        },
        v2::CIAN,
    );
    e.botones.zona(el, Accion::AbrirCompleta);

    // ¿Lo recordabas? 1 / 2 / 3, cada uno con cuando vuelve.
    let by = tarjeta.y + tarjeta.alto - 20.0 * s - botones_alto;
    let pregunta = tx.t("lec2-lo-recordabas");
    p.texto(&pregunta, ix, by, 14.0 * s, v2::SUAVE);
    let mut kx = ix + p.medir_texto(&pregunta, 14.0 * s).0 + 8.0 * s;
    for k in ["1", "2", "3"] {
        kx += ui::chapa(p, k, kx, by + 9.0 * s, v2::SUAVE, hex(0x2a2a2d), s) + 4.0 * s;
    }
    let gy = by + 28.0 * s;
    let gw = (iw - 20.0 * s) / 3.0;
    let notas = [
        (
            Nota::Recordaba,
            "lec2-nota-recordaba",
            v2::VERDE,
            &mi::CHECK_CIRCLE,
        ),
        (
            Nota::AMedias,
            "lec2-nota-a-medias",
            v2::NARANJA,
            &mi::REPLAY,
        ),
        (Nota::Olvide, "lec2-nota-olvide", v2::ROJO_TEXTO, &mi::CLOSE),
    ];
    for (k, (n, clave, color, icono)) in notas.iter().enumerate() {
        let b = RectF {
            x: ix + k as f32 * (gw + 10.0 * s),
            y: gy,
            ancho: gw,
            alto: 72.0 * s,
        };
        let elegida = nota == Some(*n);
        let encima = crate::ventanita::dentro(b, e.botones.raton);
        let borde = if elegida {
            *color
        } else {
            con_alfa(v2::BLANCO, 0.10)
        };
        p.rellenar_redondeado(b, 14.0 * s, borde);
        let fondo = if elegida {
            ui::mezclar(v2::CAJA, *color, 0.14)
        } else if encima {
            v2::ENCIMA
        } else {
            v2::CAJA
        };
        p.rellenar_redondeado(
            ui::encoger(b, if elegida { 1.5 } else { 1.0 } * s),
            13.0 * s,
            fondo,
        );
        p.icono(
            icono,
            RectF {
                x: b.x + 14.0 * s,
                y: b.y + 13.0 * s,
                ancho: 16.0 * s,
                alto: 16.0 * s,
            },
            *color,
        );
        ui::negrita(
            p,
            &tx.t(clave),
            b.x + 38.0 * s,
            b.y + 10.0 * s,
            15.0 * s,
            gw - 80.0 * s,
            v2::TEXTO,
        );
        ui::chapa(
            p,
            &(k + 1).to_string(),
            b.x + b.ancho - 34.0 * s,
            b.y + 21.0 * s,
            v2::SUAVE,
            hex(0x2a2a2d),
            s,
        );
        let vuelve = cuando_vuelve(&tx, Espaciado::dias_hasta(&l, *n));
        p.texto(
            &vuelve,
            b.x + 14.0 * s,
            b.y + 42.0 * s,
            12.5 * s,
            if elegida { v2::SUAVE } else { v2::APAGADO },
        );
        e.botones.zona(b, Accion::Nota(k));
    }

    // El pie: Saltar a la izquierda, Siguiente (azul) a la derecha.
    let fy = tarjeta.y + tarjeta.alto + 14.0 * s;
    let saltar_w = ui::ancho_de_boton(p, false, &tx.t("lec2-saltar"), Some("S"), s);
    let saltar = RectF {
        x: x0,
        y: fy,
        ancho: saltar_w,
        alto: 44.0 * s,
    };
    ui::boton_v2(
        p,
        &mut e.botones,
        saltar,
        Accion::Saltar,
        None,
        &tx.t("lec2-saltar"),
        Some("S"),
        None,
        v2::SUAVE,
        s,
    );
    let nota_pie = tx.t("lec2-repaso-pie");
    p.texto_linea(
        &nota_pie,
        saltar.x + saltar.ancho + 10.0 * s,
        fy + 15.0 * s,
        12.0 * s,
        ancho * 0.5,
        v2::APAGADO,
    );
    let sig = tx.t("lec2-siguiente");
    let sw = ui::ancho_de_boton(p, true, &sig, Some("Enter"), s) + 8.0 * s;
    let siguiente = RectF {
        x: x0 + ancho - sw,
        y: fy,
        ancho: sw,
        alto: 44.0 * s,
    };
    ui::boton_v2(
        p,
        &mut e.botones,
        siguiente,
        Accion::Siguiente,
        Some(&mi::ARROW_FORWARD),
        &sig,
        Some("Enter"),
        Some(v2::AZUL),
        v2::BLANCO,
        s,
    );
}

pub fn nombre_de_gravedad(g: i64) -> &'static str {
    match g {
        i64::MIN..=1 => "lec2-leve",
        2 => "lec2-importante",
        _ => "lec2-grave",
    }
}
