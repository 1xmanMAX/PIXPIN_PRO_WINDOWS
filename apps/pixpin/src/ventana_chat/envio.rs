//! **El cuadro de enviar fotos con su descripcion** (`Menus2.dc.html`, d).
//!
//! Lo que se va a meter en el proyecto, en miniaturas de 84 px numeradas:
//! se reordenan **arrastrando**, se quitan con su **✕** y se anaden mas con
//! la baldosa «Añadir». Debajo, la descripcion —que va con la primera, como
//! el pie de una foto en el movil— y abajo «Cancelar · Esc» y «Enviar ·
//! Intro» en azul.
//!
//! Sustituye al cuadro de `pixpin_ui::confirmar` en el chat; la logica de
//! meter los ficheros (`meter_ficheros`) no cambia: solo el orden y cuales.

use pixpin_geom::{Punto, Rect};
use pixpin_render::{Pintor, RectF};

use super::{Pendientes, Pinta, con_alfa, encoger, hex, mi, rf};

/// Medidas en pixeles logicos (al 100 %).
pub(super) const ANCHO: u32 = 460;
pub(super) const RELLENO: u32 = 14;
pub(super) const RADIO: u32 = 18;
pub(super) const FOTO: u32 = 84;
pub(super) const HUECO: u32 = 8;
pub(super) const QUITAR: u32 = 24;
pub(super) const NUMERO: u32 = 20;
pub(super) const PIE: u32 = 44;
pub(super) const BOTONES: u32 = 40;
pub(super) const MARGEN_VENTANA: u32 = 24;

/// El cuadro ya colocado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Cuadro {
    pub caja: Rect,
    /// Una por fichero que se ve, en su orden.
    pub fotos: Vec<Rect>,
    /// La baldosa de «Añadir», si cabe.
    pub anadir: Option<Rect>,
    /// Cuantos ficheros no caben y no se ven.
    pub ocultos: usize,
    pub pie: Rect,
    /// La linea de «3 fotos · a Obra», a la izquierda de los botones.
    pub resumen: Rect,
    pub cancelar: Rect,
    pub enviar: Rect,
}

/// Lo que hay bajo el raton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Sitio {
    Quitar(usize),
    Foto(usize),
    Anadir,
    Cancelar,
    Enviar,
    /// Dentro del cuadro, en nada que se pulse.
    Dentro,
}

/// Coloca el cuadro centrado en la ventana, con `cuantos` ficheros.
/// `ancho_cancelar` y `ancho_enviar` son lo que miden sus botones (rotulo y
/// chapita), medidos al pintar.
pub(super) fn colocar(
    ventana: Rect,
    cuantos: usize,
    ancho_botones: (u32, u32),
    escala: u32,
) -> Cuadro {
    let e = |v: u32| v * escala / 100;
    let ancho = e(ANCHO).min(ventana.ancho.saturating_sub(2 * e(MARGEN_VENTANA)));
    let dentro = ancho.saturating_sub(2 * e(RELLENO));
    let paso = e(FOTO) + e(HUECO);
    let columnas = ((dentro + e(HUECO)) / paso).max(1);
    // Las filas que caben: el resto del cuadro tiene alto fijo y, en una
    // ventana baja, lo que sobra se resume en vez de salirse.
    let fijo = 2 * e(RELLENO) + 2 * e(HUECO) + e(PIE) + e(HUECO) + e(BOTONES);
    let alto_libre = ventana.alto.saturating_sub(2 * e(MARGEN_VENTANA) + fijo);
    let filas_que_caben = ((alto_libre + e(HUECO)) / paso).max(1);
    let baldosas = cuantos + 1;
    let filas = (baldosas as u32).div_ceil(columnas).min(filas_que_caben);
    let sitios = (filas * columnas) as usize;
    // Si no cabe todo, «Añadir» cede su sitio a una foto mas.
    let (visibles, con_anadir) = if baldosas <= sitios {
        (cuantos, true)
    } else {
        (sitios.min(cuantos), false)
    };
    let alto = 2 * e(RELLENO)
        + filas * e(FOTO)
        + filas.saturating_sub(1) * e(HUECO)
        + e(HUECO)
        + e(PIE)
        + e(HUECO)
        + e(BOTONES);
    let caja = Rect {
        x: ventana.x + (ventana.ancho as i32 - ancho as i32) / 2,
        y: ventana.y + (ventana.alto as i32 - alto as i32) / 2,
        ancho,
        alto,
    };
    let x0 = caja.x + e(RELLENO) as i32;
    let y0 = caja.y + e(RELLENO) as i32;
    let sitio = |k: usize| Rect {
        x: x0 + ((k as u32 % columnas) * paso) as i32,
        y: y0 + ((k as u32 / columnas) * paso) as i32,
        ancho: e(FOTO),
        alto: e(FOTO),
    };
    let fotos: Vec<Rect> = (0..visibles).map(sitio).collect();
    let anadir = con_anadir.then(|| sitio(visibles));
    let fin_fotos = y0 + (filas * e(FOTO) + filas.saturating_sub(1) * e(HUECO)) as i32;
    let pie = Rect {
        x: x0,
        y: fin_fotos + e(HUECO) as i32,
        ancho: dentro,
        alto: e(PIE),
    };
    let fila_botones = pie.abajo() + e(HUECO) as i32;
    let enviar = Rect {
        x: caja.derecha() - e(RELLENO) as i32 - ancho_botones.1 as i32,
        y: fila_botones,
        ancho: ancho_botones.1,
        alto: e(BOTONES),
    };
    let cancelar = Rect {
        x: enviar.x - e(HUECO) as i32 - ancho_botones.0 as i32,
        ancho: ancho_botones.0,
        ..enviar
    };
    let resumen = Rect {
        x: x0,
        y: fila_botones,
        ancho: (cancelar.x - x0 - e(HUECO) as i32).max(0) as u32,
        alto: e(BOTONES),
    };
    Cuadro {
        caja,
        fotos,
        anadir,
        ocultos: cuantos - visibles,
        pie,
        resumen,
        cancelar,
        enviar,
    }
}

impl Cuadro {
    /// El ✕ de una foto: un circulo arriba a la derecha.
    pub(super) fn quitar(&self, n: usize, escala: u32) -> Option<Rect> {
        let f = self.fotos.get(n)?;
        let lado = QUITAR * escala / 100;
        let m = 4 * escala / 100;
        Some(Rect {
            x: f.derecha() - m as i32 - lado as i32,
            y: f.y + m as i32,
            ancho: lado,
            alto: lado,
        })
    }

    pub(super) fn sitio_en(&self, p: Punto, escala: u32) -> Option<Sitio> {
        // El ✕ antes que su foto: esta encima.
        if let Some(n) =
            (0..self.fotos.len()).find(|n| self.quitar(*n, escala).is_some_and(|r| r.contiene(p)))
        {
            return Some(Sitio::Quitar(n));
        }
        if let Some(n) = self.fotos.iter().position(|r| r.contiene(p)) {
            return Some(Sitio::Foto(n));
        }
        if self.anadir.is_some_and(|r| r.contiene(p)) {
            return Some(Sitio::Anadir);
        }
        if self.cancelar.contiene(p) {
            return Some(Sitio::Cancelar);
        }
        if self.enviar.contiene(p) {
            return Some(Sitio::Enviar);
        }
        self.caja.contiene(p).then_some(Sitio::Dentro)
    }

    /// Donde caeria una foto soltada en `p`: el indice ANTES del cual se
    /// mete (de 0 a `fotos.len()`). Va por la fila del raton y, en ella,
    /// por la mitad de cada miniatura, que es lo que hace la mano.
    pub(super) fn hueco_en(&self, p: Punto) -> usize {
        if self.fotos.is_empty() {
            return 0;
        }
        // La fila cuya franja esta mas cerca del raton.
        let fila_y = self
            .fotos
            .iter()
            .map(|r| r.y)
            .min_by_key(|y| {
                let centro = y + self.fotos[0].alto as i32 / 2;
                (centro - p.y).abs()
            })
            .unwrap_or(self.fotos[0].y);
        let en_fila: Vec<usize> = (0..self.fotos.len())
            .filter(|n| self.fotos[*n].y == fila_y)
            .collect();
        for n in &en_fila {
            let r = self.fotos[*n];
            if p.x < r.x + r.ancho as i32 / 2 {
                return *n;
            }
        }
        en_fila.last().map(|n| n + 1).unwrap_or(self.fotos.len())
    }
}

/// Mueve el elemento `desde` para que quede delante de lo que estaba en
/// `hueco` (como lo devuelve `hueco_en`). Soltar una foto en su propio
/// sitio no cambia nada.
pub(super) fn mover<T>(v: &mut Vec<T>, desde: usize, hueco: usize) {
    if desde >= v.len() {
        return;
    }
    let hueco = hueco.min(v.len());
    let destino = if hueco > desde { hueco - 1 } else { hueco };
    if destino == desde {
        return;
    }
    let x = v.remove(desde);
    v.insert(destino, x);
}

/// Una foto que se esta arrastrando para cambiarla de sitio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Arrastre {
    pub desde: usize,
    pub agarre: Punto,
    pub ahora: Punto,
}

impl Arrastre {
    /// Un clic que no se movio no es un arrastre: unos pixeles de margen,
    /// que la mano tiembla.
    pub(super) fn se_movio(&self, escala: u32) -> bool {
        let umbral = (4 * escala / 100).max(2) as i32;
        (self.ahora.x - self.agarre.x).abs() > umbral
            || (self.ahora.y - self.agarre.y).abs() > umbral
    }
}

impl Pendientes {
    /// Quita el fichero `n` del envio.
    pub(super) fn quitar(&mut self, n: usize) {
        if n < self.rutas.len() {
            self.rutas.remove(n);
            if n < self.tamanos.len() {
                self.tamanos.remove(n);
            }
        }
    }

    /// Cambia de sitio el fichero `desde` (ver [`mover`]).
    pub(super) fn reordenar(&mut self, desde: usize, hueco: usize) {
        mover(&mut self.rutas, desde, hueco);
        mover(&mut self.tamanos, desde, hueco);
    }

    /// Anade mas ficheros al envio, sin repetir los que ya estan.
    pub(super) fn anadir(&mut self, rutas: Vec<std::path::PathBuf>) {
        for r in rutas {
            if !self.rutas.contains(&r) {
                self.tamanos
                    .push(std::fs::metadata(&r).map(|m| m.len()).unwrap_or(0));
                self.rutas.push(r);
            }
        }
    }

    /// Si todo lo que se envia son fotos (para decir «3 fotos» o «3
    /// archivos»).
    pub(super) fn todo_fotos(&self) -> bool {
        self.rutas.iter().all(|r| es_foto(r))
    }
}

fn es_foto(r: &std::path::Path) -> bool {
    r.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| {
            matches!(
                e.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tif" | "tiff" | "heic"
            )
        })
}

/// Lo que miden los dos botones con su chapita.
pub(super) fn medir_botones(p: &Pintor, c: &Pinta) -> (u32, u32) {
    let e = c.escala as f32 / 100.0;
    let tam = 14.0 * e;
    let uno = |clave: &str, tecla: &str| {
        let w = p.medir_texto(&c.textos.t(clave), tam).0;
        let ch = super::ancho_chapas(p, &[tecla.to_string()], e);
        (w + ch + 8.0 * e + 2.0 * 14.0 * e).ceil() as u32
    };
    (
        uno("confirmar-cancelar", "Esc"),
        uno("v2menus-envio-enviar", "Enter"),
    )
}

/// Pinta el cuadro. `proyecto` es el nombre de donde va.
pub(super) fn pintar(p: &Pintor, c: &Pinta, pend: &Pendientes, marco: Rect, proyecto: &str) {
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let ventana = Rect {
        x: 0,
        y: 0,
        ancho: marco.ancho,
        alto: marco.alto,
    };
    // El velo dice que lo de debajo esta esperando una respuesta.
    p.rellenar(rf(ventana), tema.velo);
    let botones = medir_botones(p, c);
    pend.botones.set(botones);
    let d = colocar(ventana, pend.rutas.len(), botones, escala);
    let caja = rf(d.caja);
    let radio = RADIO as f32 * e;
    p.rellenar_redondeado(caja, radio, tema.pildora_borde);
    p.rellenar_redondeado(encoger(caja, 1.0), radio - 1.0, tema.cabecera);

    let arrastrando = pend.arrastre.filter(|a| a.se_movio(escala));
    let hueco = arrastrando.map(|a| d.hueco_en(a.ahora));
    let radio_foto = 10.0 * e;
    let pintar_foto = |n: usize, r: RectF, opacidad: f32| {
        let Some(ruta) = pend.rutas.get(n) else {
            return;
        };
        match c.miniaturas.ya(ruta) {
            Some((b, w, h)) if opacidad >= 1.0 => {
                p.empujar_recorte(r);
                crate::miniaturas::pintar_recortado(p, b, r, w, h);
                p.soltar_recorte();
            }
            _ => {
                p.rellenar_redondeado(r, radio_foto, con_alfa(tema.chat, opacidad));
                let lado = 28.0 * e;
                p.icono(
                    &mi::DESCRIPTION,
                    RectF {
                        x: r.x + (r.ancho - lado) / 2.0,
                        y: r.y + 12.0 * e,
                        ancho: lado,
                        alto: lado,
                    },
                    con_alfa(tema.apagado, opacidad),
                );
                let nombre = ruta
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                p.texto_linea(
                    &nombre,
                    r.x + 6.0 * e,
                    r.y + r.alto - 22.0 * e,
                    11.0 * e,
                    r.ancho - 12.0 * e,
                    con_alfa(tema.texto, opacidad),
                );
            }
        }
    };
    for (n, r) in d.fotos.iter().enumerate() {
        let rr = rf(*r);
        if arrastrando.is_some_and(|a| a.desde == n) {
            // Su sitio vacio, con el borde: de ahi sale la que se arrastra.
            p.trazar(rr, 1.0, con_alfa(tema.texto, 0.25));
            continue;
        }
        pintar_foto(n, rr, 1.0);
        // La primera lleva la descripcion: va marcada en azul.
        if n == 0 {
            p.trazar(encoger(rr, -1.0 * e), 2.0 * e, hex(0x0a84ff));
        }
        // El numero de orden, arriba a la izquierda.
        let lado = NUMERO as f32 * e;
        let centro = (rr.x + 5.0 * e + lado / 2.0, rr.y + 5.0 * e + lado / 2.0);
        p.circulo(
            centro,
            lado / 2.0,
            if n == 0 {
                super::AZUL_ELEGIDO
            } else {
                con_alfa(hex(0x000000), 0.6)
            },
        );
        let num = (n + 1).to_string();
        let (w, h) = p.medir_texto(&num, 11.0 * e);
        p.texto_linea(
            &num,
            centro.0 - w / 2.0,
            centro.1 - h / 2.0,
            11.0 * e,
            w + 2.0,
            hex(0xffffff),
        );
        // El ✕ para quitarla.
        if let Some(q) = d.quitar(n, escala) {
            let q = rf(q);
            let centro = (q.x + q.ancho / 2.0, q.y + q.alto / 2.0);
            let encima = pend.sobre == Some(Sitio::Quitar(n));
            p.circulo(
                centro,
                q.ancho / 2.0,
                con_alfa(hex(0x000000), if encima { 0.85 } else { 0.6 }),
            );
            let b = 4.0 * e;
            p.linea(
                (centro.0 - b, centro.1 - b),
                (centro.0 + b, centro.1 + b),
                2.0 * e,
                hex(0xffffff),
            );
            p.linea(
                (centro.0 + b, centro.1 - b),
                (centro.0 - b, centro.1 + b),
                2.0 * e,
                hex(0xffffff),
            );
        }
    }
    if d.ocultos > 0
        && let Some(r) = d.fotos.last()
    {
        let rr = rf(*r);
        p.rellenar_redondeado(rr, radio_foto, con_alfa(hex(0x000000), 0.55));
        let mas = format!("+{}", d.ocultos);
        let (w, h) = p.medir_texto(&mas, 16.0 * e);
        p.texto_linea(
            &mas,
            rr.x + (rr.ancho - w) / 2.0,
            rr.y + (rr.alto - h) / 2.0,
            16.0 * e,
            w + 2.0,
            hex(0xffffff),
        );
    }
    // «Añadir», con borde de puntos.
    if let Some(r) = d.anadir {
        let rr = rf(r);
        if pend.sobre == Some(Sitio::Anadir) {
            p.rellenar_redondeado(rr, radio_foto, con_alfa(tema.texto, 0.06));
        }
        p.trazar_discontinuo(rr, 1.0, con_alfa(tema.texto, 0.3));
        let cx = rr.x + rr.ancho / 2.0;
        let cy = rr.y + rr.alto / 2.0 - 8.0 * e;
        let b = 8.0 * e;
        p.linea((cx - b, cy), (cx + b, cy), 2.0 * e, tema.apagado);
        p.linea((cx, cy - b), (cx, cy + b), 2.0 * e, tema.apagado);
        let rotulo = textos.t("v2menus-envio-anadir");
        let (w, _) = p.medir_texto(&rotulo, 12.0 * e);
        p.texto_linea(
            &rotulo,
            cx - w / 2.0,
            cy + 14.0 * e,
            12.0 * e,
            rr.ancho,
            tema.apagado,
        );
    }
    // La raya azul donde caera la que se arrastra, y ella bajo el raton.
    if let (Some(a), Some(h)) = (arrastrando, hueco) {
        let (x, r) = match d.fotos.get(h) {
            Some(r) => (r.x as f32 - HUECO as f32 * e / 2.0, *r),
            None => {
                let r = *d.fotos.last().unwrap_or(&d.caja);
                (r.derecha() as f32 + HUECO as f32 * e / 2.0, r)
            }
        };
        p.rellenar(
            RectF {
                x: x - 1.5 * e,
                y: r.y as f32,
                ancho: 3.0 * e,
                alto: r.alto as f32,
            },
            hex(0x0a84ff),
        );
        if let Some(origen) = d.fotos.get(a.desde) {
            let rr = RectF {
                x: (origen.x + a.ahora.x - a.agarre.x) as f32,
                y: (origen.y + a.ahora.y - a.agarre.y) as f32 - 4.0 * e,
                ancho: origen.ancho as f32,
                alto: origen.alto as f32,
            };
            p.rellenar_redondeado(
                RectF {
                    y: rr.y + 6.0 * e,
                    ..rr
                },
                radio_foto,
                con_alfa(hex(0x000000), 0.35),
            );
            pintar_foto(a.desde, rr, 1.0);
        }
    }

    // La descripcion, en su caja.
    let pie = rf(d.pie);
    p.rellenar_redondeado(pie, 12.0 * e, tema.pildora_borde);
    p.rellenar_redondeado(encoger(pie, 1.0), 11.0 * e, tema.chat);
    let (texto, color) = if pend.pie.is_empty() {
        (textos.t("confirmar-pie"), tema.apagado)
    } else {
        (format!("{}|", pend.pie), tema.texto)
    };
    let tam = 14.0 * e;
    let (_, alto) = p.medir_texto("X", tam);
    p.texto_linea(
        &texto,
        pie.x + 12.0 * e,
        pie.y + (pie.alto - alto) / 2.0,
        tam,
        pie.ancho - 24.0 * e,
        color,
    );

    // «3 fotos · a Obra», y los dos botones.
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("cuantos", pend.rutas.len());
    args.set("proyecto", proyecto.to_string());
    let clave = if pend.todo_fotos() {
        "v2menus-envio-fotos"
    } else {
        "v2menus-envio-archivos"
    };
    let resumen = textos.t_args(clave, &args);
    let tam_r = 12.0 * e;
    let (_, alto) = p.medir_texto("X", tam_r);
    p.texto_linea(
        &resumen,
        d.resumen.x as f32,
        d.resumen.y as f32 + (d.resumen.alto as f32 - alto) / 2.0,
        tam_r,
        d.resumen.ancho as f32,
        tema.apagado,
    );
    for (r, clave, tecla, fuerte) in [
        (d.cancelar, "confirmar-cancelar", "Esc", false),
        (d.enviar, "v2menus-envio-enviar", "Enter", true),
    ] {
        let rr = rf(r);
        let encima = pend.sobre
            == Some(if fuerte {
                Sitio::Enviar
            } else {
                Sitio::Cancelar
            });
        if fuerte {
            p.rellenar_redondeado(rr, 10.0 * e, super::AZUL_ELEGIDO);
        } else if encima {
            p.rellenar_redondeado(rr, 10.0 * e, con_alfa(tema.texto, 0.08));
        }
        let rotulo = textos.t(clave);
        let (w, h) = p.medir_texto(&rotulo, tam);
        let x = rr.x + 14.0 * e;
        let color = if fuerte { hex(0xffffff) } else { tema.texto };
        p.texto_linea(&rotulo, x, rr.y + (rr.alto - h) / 2.0, tam, w + 2.0, color);
        super::pintar_chapas(
            p,
            tema,
            &[tecla.to_string()],
            rr.x + rr.ancho - 14.0 * e,
            r,
            e,
            fuerte,
        );
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ventana() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1024,
            alto: 800,
        }
    }

    #[test]
    fn tres_fotos_y_anadir_en_una_fila() {
        let d = colocar(ventana(), 3, (100, 110), 100);
        assert_eq!(d.fotos.len(), 3);
        assert!(d.anadir.is_some());
        assert!(
            d.fotos
                .iter()
                .all(|r| r.y == d.fotos[0].y && r.ancho == FOTO)
        );
        assert_eq!(d.anadir.unwrap().y, d.fotos[0].y);
        // El pie debajo de las fotos y los botones debajo del pie, dentro.
        assert!(d.pie.y > d.fotos[0].abajo());
        assert!(d.enviar.y >= d.pie.abajo() && d.enviar.abajo() <= d.caja.abajo());
        // Enviar a la derecha de Cancelar, sin pisarse.
        assert!(d.cancelar.derecha() < d.enviar.x);
        // Caso negativo: nada oculto.
        assert_eq!(d.ocultos, 0);
    }

    #[test]
    fn en_una_ventana_baja_lo_que_no_cabe_se_resume() {
        let baja = Rect {
            alto: 320,
            ..ventana()
        };
        let d = colocar(baja, 30, (100, 110), 100);
        assert!(d.fotos.len() < 30);
        assert_eq!(d.ocultos, 30 - d.fotos.len());
        assert!(d.anadir.is_none(), "sin sitio, Añadir cede el suyo");
        assert!(d.caja.abajo() <= 320 && d.caja.y >= 0);
        // Caso negativo: con sitio de sobra se ven todas.
        let d = colocar(ventana(), 6, (100, 110), 100);
        assert_eq!((d.fotos.len(), d.ocultos), (6, 0));
    }

    #[test]
    fn el_aspa_va_antes_que_su_foto() {
        let d = colocar(ventana(), 2, (100, 110), 100);
        let q = d.quitar(1, 100).unwrap();
        assert_eq!(
            d.sitio_en(
                Punto {
                    x: q.x + 3,
                    y: q.y + 3
                },
                100
            ),
            Some(Sitio::Quitar(1))
        );
        let f = d.fotos[1];
        assert_eq!(
            d.sitio_en(
                Punto {
                    x: f.x + 5,
                    y: f.abajo() - 5
                },
                100
            ),
            Some(Sitio::Foto(1))
        );
        assert_eq!(
            d.sitio_en(
                Punto {
                    x: d.enviar.x + 3,
                    y: d.enviar.y + 3
                },
                100
            ),
            Some(Sitio::Enviar)
        );
        // Casos negativos: el pie no es ningun boton, y fuera no es nada.
        assert_eq!(
            d.sitio_en(
                Punto {
                    x: d.pie.x + 3,
                    y: d.pie.y + 3
                },
                100
            ),
            Some(Sitio::Dentro)
        );
        assert_eq!(d.sitio_en(Punto { x: 1, y: 1 }, 100), None);
    }

    #[test]
    fn soltar_entre_dos_fotos_la_mete_ahi() {
        let d = colocar(ventana(), 3, (100, 110), 100);
        let f = &d.fotos;
        // A la izquierda de la mitad de la primera: delante de todo.
        assert_eq!(
            d.hueco_en(Punto {
                x: f[0].x + 2,
                y: f[0].y + 10
            }),
            0
        );
        // Pasada la mitad de la segunda: delante de la tercera.
        assert_eq!(
            d.hueco_en(Punto {
                x: f[1].x + 60,
                y: f[1].y + 10
            }),
            2
        );
        // Mas alla de la ultima: al final.
        assert_eq!(
            d.hueco_en(Punto {
                x: f[2].derecha() + 30,
                y: f[2].y + 10
            }),
            3
        );

        let mut v = vec!['a', 'b', 'c'];
        mover(&mut v, 2, 0);
        assert_eq!(v, ['c', 'a', 'b']);
        let mut v = vec!['a', 'b', 'c'];
        mover(&mut v, 0, 3);
        assert_eq!(v, ['b', 'c', 'a']);
        // Casos negativos: soltarla en su sitio (delante o detras de si
        // misma) no cambia nada, y un indice fuera no rompe.
        let mut v = vec!['a', 'b', 'c'];
        mover(&mut v, 1, 1);
        mover(&mut v, 1, 2);
        mover(&mut v, 7, 0);
        assert_eq!(v, ['a', 'b', 'c']);
    }

    #[test]
    fn un_clic_quieto_no_es_arrastrar() {
        let a = Arrastre {
            desde: 0,
            agarre: Punto { x: 100, y: 100 },
            ahora: Punto { x: 102, y: 101 },
        };
        assert!(!a.se_movio(100));
        let a = Arrastre {
            ahora: Punto { x: 120, y: 100 },
            ..a
        };
        assert!(a.se_movio(100));
    }
}
