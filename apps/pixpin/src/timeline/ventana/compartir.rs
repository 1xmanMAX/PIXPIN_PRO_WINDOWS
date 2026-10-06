//! **Compartir una tarjeta como imagen** (5-oct-2026): un momento o una
//! leccion, pintados solos a un PNG fuera de pantalla, al doble de tamano,
//! y copiados al portapapeles o guardados donde se diga.
//!
//! El usuario: «que se pueda compartir como imagen cada tarjeta». Se hace en
//! el bucle (aqui llega el pedido) porque hace falta el motor de dibujo, y
//! con el mismo motor de la ventana: asi las miniaturas ya cargadas valen
//! tal cual.
//!
//! Tambien vive aqui [`Contenido`]: lo que ensena de un momento o de una
//! leccion el detalle y la imagen compartida, para que los dos lo cuenten
//! igual.

use super::*;
use pixpin_render::MotorRender;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

/// Lo que se ensena de un momento o de una leccion.
#[derive(Debug, Clone)]
pub(super) struct Contenido {
    pub cuando: i64,
    pub titulo: String,
    pub texto: String,
    pub fotos: Vec<PathBuf>,
    /// El audio y lo que dura.
    pub audio: Option<(PathBuf, i64)>,
    /// De donde sale su color suave (el id).
    pub semilla: String,
    /// La gravedad, si es una leccion.
    pub gravedad: Option<i64>,
    /// Cuantas veces paso, si es una leccion.
    pub veces: usize,
    /// Un momento marcado como leccion: el emoticono de su marca (el de su
    /// gravedad, `archivo::marca_de_leccion`).
    pub marca: Option<&'static str>,
    /// El primer emoticono de un momento: su chapita de estado.
    pub emoticono: Option<String>,
}

pub(super) fn contenido_de(e: &Estado, item: &Item, textos: &Catalogo) -> Option<Contenido> {
    match item {
        Item::Momento(id) => {
            let m = e.momentos.iter().find(|m| &m.id == id)?;
            Some(Contenido {
                cuando: m.cuando,
                titulo: m.titulo.clone(),
                texto: m.descripcion.clone(),
                fotos: m.fotos.iter().map(|f| e.almacen.ruta(f)).collect(),
                audio: m.audio.as_ref().map(|a| (e.almacen.ruta(a), m.duracion_ms)),
                semilla: m.id.clone(),
                gravedad: None,
                veces: 0,
                marca: super::archivo::emoticono_de_leccion(e, m),
                emoticono: pixpin_timeline::emoticonos::primer_emoticono(&m.titulo)
                    .or_else(|| pixpin_timeline::emoticonos::primer_emoticono(&m.descripcion))
                    .map(str::to_string),
            })
        }
        Item::Leccion(id) => {
            let t = e.lecciones.iter().find(|t| &t.entrada.leccion.id == id)?;
            let l = &t.entrada.leccion;
            let mut texto = l.que_paso.trim().to_string();
            for (clave, campo) in [
                ("timeline-leccion-por-que", &l.por_que),
                ("timeline-leccion-proxima", &l.proxima),
            ] {
                if !campo.trim().is_empty() {
                    if !texto.is_empty() {
                        texto.push_str("\n\n");
                    }
                    texto.push_str(&format!("{}: {}", textos.t(clave), campo.trim()));
                }
            }
            Some(Contenido {
                cuando: l.creada,
                titulo: l.titulo.clone(),
                texto,
                fotos: t.fotos.clone(),
                audio: t.voz.clone(),
                semilla: l.id.clone(),
                gravedad: Some(l.gravedad),
                veces: l.veces_que_paso(),
                marca: None,
                emoticono: None,
            })
        }
    }
}

pub(super) fn contenido_del_detalle(e: &Estado, textos: &Catalogo) -> Option<Contenido> {
    contenido_de(e, e.item_en_detalle()?, textos)
}

/// El nombre de una gravedad, con su emoticono delante.
pub(super) fn rotulo_de_gravedad(g: i64, textos: &Catalogo) -> String {
    let gr = super::super::tarjetas::gravedad(g);
    textos.t(gr.clave)
}

/// La chapita de gravedad: el emoticono en monocromo, pintado de su color,
/// y su nombre, sobre un fondo suave del mismo color. Devuelve su caja.
#[allow(clippy::too_many_arguments)] // pintor, gravedad, donde, fondo, escala y textos
pub(super) fn chapa_de_gravedad(
    p: &Pintor,
    g: i64,
    x: f32,
    y: f32,
    fondo: Option<Color>,
    s: f32,
    textos: &Catalogo,
) -> RectF {
    let gr = super::super::tarjetas::gravedad(g);
    let rot = rotulo_de_gravedad(g, textos);
    let tam = 13.0 * s;
    let (rw, rh) = ui::medir_negrita(p, &rot, tam, 300.0 * s);
    let alto = 30.0 * s;
    let caja = RectF {
        x,
        y,
        ancho: 34.0 * s + rw + 12.0 * s,
        alto,
    };
    p.rellenar_redondeado(
        caja,
        alto / 2.0,
        fondo.unwrap_or(super::super::tarjetas::fondo_de(&gr)),
    );
    // Sin la opcion de fuente en color: el glifo sale en monocromo y toma
    // el color del pincel. Es lo que pidio el usuario: «grave rojo pero el
    // emoticon pintado de rojo».
    let (ew, eh) = p.medir_texto(gr.emoticono, 16.0 * s);
    p.texto(
        gr.emoticono,
        caja.x + 8.0 * s + (20.0 * s - ew) / 2.0,
        caja.y + (alto - eh) / 2.0,
        16.0 * s,
        gr.color,
    );
    ui::negrita(
        p,
        &rot,
        caja.x + 32.0 * s,
        caja.y + (alto - rh) / 2.0,
        tam,
        rw + 2.0,
        gr.color,
    );
    caja
}

/// Copia o guarda como imagen lo pedido en el menu de compartir.
pub(super) fn hacer(
    e: &mut Estado,
    item: &Item,
    destino: Destino,
    motor: &MotorRender,
    d3d: &ID3D11Device,
    textos: &Catalogo,
) {
    let Some(c) = contenido_de(e, item, textos) else {
        return;
    };
    let img = match imagen(e, &c, motor, d3d, textos) {
        Ok(i) => i,
        Err(err) => {
            tracing::warn!(?err, "timeline: la tarjeta no se pudo pintar");
            e.decir(textos.t("timeline-imagen-fallo"));
            return;
        }
    };
    match destino {
        Destino::Copiar => match pixpin_codec::portapapeles::copiar_imagen(&img) {
            Ok(()) => e.decir(textos.t("timeline-imagen-copiada")),
            Err(err) => {
                tracing::warn!(?err, "timeline: no se pudo copiar la tarjeta");
                e.decir(textos.t("timeline-imagen-fallo"));
            }
        },
        Destino::Guardar => {
            let Some(h) = e.hwnd else { return };
            let d = Dia::de_instante(c.cuando, e.desfase);
            let sugerido = format!("timeline-{}.png", d.iso());
            let tipo = textos.t("timeline-tipo-png");
            let Some(ruta) = pixpin_shell::guardar::pedir_ruta_para(h, &sugerido, &tipo, "png")
            else {
                return;
            };
            match pixpin_codec::guardar(&img, &ruta, pixpin_codec::FormatoImagen::Png) {
                Ok(()) => exportado(e, ruta, textos),
                Err(err) => {
                    tracing::warn!(?err, "timeline: no se pudo guardar la tarjeta");
                    e.decir(textos.t("timeline-imagen-fallo"));
                }
            }
        }
    }
}

/// **La tarjeta para compartir**, como imagen: la historia sin botones en
/// 1080 x 1350 (4:5, un post), con la primera foto de fondo. El usuario:
/// «que se pueda compartir como una tarjeta bonita, como una tarjeta de las
/// que dicen una frase».
pub(super) fn imagen(
    e: &mut Estado,
    c: &Contenido,
    motor: &MotorRender,
    d3d: &ID3D11Device,
    textos: &Catalogo,
) -> Result<pixpin_codec::ImagenRgba> {
    const W: u32 = 1080;
    const H: u32 = 1350;
    // La foto, a su tamano: una propia y de un solo uso (la de la ventana
    // es de 352 px y en 1350 se veria pastosa).
    let mut grande = crate::miniaturas::Miniaturas::con_lado(H);
    if let Some(f) = c.fotos.first() {
        for _ in 0..3 {
            if !grande.asegurar(std::slice::from_ref(f), motor) {
                break;
            }
        }
    }
    let bm = c.fotos.first().and_then(|f| grande.ya(f));
    let fuera = FueraDePantalla::nuevo(motor, d3d, W, H).context("sin superficie")?;
    motor
        .dibujar(&fuera.destino, |p| {
            p.limpiar(Color::NEGRO);
            super::historia::tarjeta(p, c, bm, W as f32, H as f32, textos, e.desfase, e.ingles);
        })
        .context("no se pudo pintar")?;
    fuera.esperar_gpu().context("la GPU no acabo")?;
    let (_, _, pixeles) = fuera.leer_rgba().context("no se pudo leer")?;
    Ok(pixpin_codec::ImagenRgba {
        ancho: W,
        alto: H,
        pixeles,
    })
}

/// El menu de compartir: copiar o guardar como imagen, junto al clic.
pub(super) fn pintar_menu(e: &mut Estado, p: &Pintor, w: f32, h: f32, s: f32, textos: &Catalogo) {
    let Some((_, (mx, my))) = e.menu_compartir.clone() else {
        return;
    };
    e.botones.zona(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: h,
        },
        Accion::SoltarMenu,
    );
    let ancho = 250.0 * s;
    let fila = 44.0 * s;
    let alto = 2.0 * fila + 8.0 * s;
    let caja = RectF {
        x: mx.min(w - ancho - 8.0 * s).max(8.0 * s),
        y: (my + 4.0 * s).min(h - alto - 8.0 * s).max(8.0 * s),
        ancho,
        alto,
    };
    p.rellenar_redondeado(caja, 12.0 * s, blanco(0.12));
    p.rellenar_redondeado(encoger(caja, 1.0 * s), 11.0 * s, hex(0x2C2C2E));
    e.botones.zona(caja, Accion::Fondo);
    for (k, (d, icono, clave)) in [
        (Destino::Copiar, &mi::CONTENT_COPY, "timeline-copiar-imagen"),
        (Destino::Guardar, &mi::IMAGE, "timeline-guardar-imagen"),
    ]
    .into_iter()
    .enumerate()
    {
        let r = RectF {
            x: caja.x + 4.0 * s,
            y: caja.y + 4.0 * s + k as f32 * fila,
            ancho: ancho - 8.0 * s,
            alto: fila,
        };
        if dentro(r, e.botones.raton) {
            p.rellenar_redondeado(r, 8.0 * s, blanco(0.08));
        }
        p.icono(
            icono,
            RectF {
                x: r.x + 10.0 * s,
                y: r.y + (fila - 20.0 * s) / 2.0,
                ancho: 20.0 * s,
                alto: 20.0 * s,
            },
            ui::v2::CIAN,
        );
        let t = textos.t(clave);
        let (_, th) = p.medir_texto(&t, 14.5 * s);
        p.texto(
            &t,
            r.x + 42.0 * s,
            r.y + (fila - th) / 2.0,
            14.5 * s,
            TEXTO_V,
        );
        e.botones.zona(r, Accion::Compartir(d));
    }
}
