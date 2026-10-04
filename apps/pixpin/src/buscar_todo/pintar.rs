//! **Como se pinta el buscador** (maquetas `Buscar2-vacio` y
//! `Buscar2-resultados`): mide los textos, coloca con
//! [`super::disposicion`] y pinta. Devuelve la disposicion para saber que
//! hay bajo el raton.

use std::collections::HashMap;

use pixpin_lanzador::resultados::{Resultado, glifo};
use pixpin_render::icono::{Icono, material as mi};
use pixpin_render::letras::{Letra, SIN_PARTIR};
use pixpin_render::{Color, Pintor, RectF};
use pixpin_store::Catalogo;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use super::disposicion::{self, Disposicion, Entrada};
use super::modelo::{self, Boton, Estado, Grupo, Linea, Pestana};

/// Los colores. Los de la maqueta de noche, y su pareja de dia para el tema
/// claro de Windows (el chat sigue al sistema; el buscador tambien).
#[derive(Debug, Clone, Copy)]
pub struct Paleta {
    pub fondo: Color,
    pub panel: Color,
    pub borde: Color,
    pub texto: Color,
    pub texto2: Color,
    pub suave: Color,
    pub apagado: Color,
    pub raya: Color,
    pub azul: Color,
    pub azul_boton: Color,
    pub azul_fondo: Color,
    pub azul_borde: Color,
    pub ficha: Color,
    pub ficha_encima: Color,
    pub tecla: Color,
    pub tecla_borde: Color,
    pub encima: Color,
    pub enlace: Color,
    pub boton: Color,
    pub boton_encima: Color,
    pub rojo: Color,
    pub verde: Color,
    pub amarillo: Color,
    pub naranja: Color,
    pub naranja_texto: Color,
    pub foto: Color,
    pub sobre_azul: Color,
}

const fn rgb(c: u32) -> Color {
    rgba(c, 1.0)
}

const fn rgba(c: u32, a: f32) -> Color {
    Color {
        r: ((c >> 16) & 0xff) as f32 / 255.0,
        g: ((c >> 8) & 0xff) as f32 / 255.0,
        b: (c & 0xff) as f32 / 255.0,
        a,
    }
}

fn con_alfa(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

impl Paleta {
    pub const NOCHE: Paleta = Paleta {
        fondo: rgb(0x1C1C1E),
        panel: rgb(0x232326),
        borde: rgba(0xFFFFFF, 0.10),
        texto: rgb(0xF5F5F7),
        texto2: rgb(0xE5E5EA),
        suave: rgb(0xC7C7CC),
        apagado: rgb(0x98989D),
        raya: rgba(0xFFFFFF, 0.08),
        azul: rgb(0x0A84FF),
        azul_boton: rgb(0x0060DF),
        azul_fondo: rgba(0x0A84FF, 0.14),
        azul_borde: rgba(0x0A84FF, 0.5),
        ficha: rgb(0x2C2C2E),
        ficha_encima: rgb(0x3A3A3C),
        tecla: rgba(0xFFFFFF, 0.09),
        tecla_borde: rgba(0xFFFFFF, 0.08),
        encima: rgba(0xFFFFFF, 0.05),
        enlace: rgb(0x64D2FF),
        boton: rgba(0xFFFFFF, 0.10),
        boton_encima: rgba(0xFFFFFF, 0.16),
        rojo: rgb(0xFF6961),
        verde: rgb(0x30D158),
        amarillo: rgb(0xFFD60A),
        naranja: rgb(0xFF9F0A),
        naranja_texto: rgb(0xFFD08A),
        foto: rgb(0x34506F),
        sobre_azul: rgb(0xFFFFFF),
    };

    pub const DIA: Paleta = Paleta {
        fondo: rgb(0xFFFFFF),
        panel: rgb(0xF2F2F7),
        borde: rgba(0x000000, 0.12),
        texto: rgb(0x1C1C1E),
        texto2: rgb(0x2C2C2E),
        suave: rgb(0x48484A),
        apagado: rgb(0x6E6E73),
        raya: rgba(0x000000, 0.08),
        azul: rgb(0x007AFF),
        azul_boton: rgb(0x0060DF),
        azul_fondo: rgba(0x007AFF, 0.10),
        azul_borde: rgba(0x007AFF, 0.45),
        ficha: rgb(0xE5E5EA),
        ficha_encima: rgb(0xD8D8DD),
        tecla: rgba(0x000000, 0.05),
        tecla_borde: rgba(0x000000, 0.10),
        encima: rgba(0x000000, 0.04),
        enlace: rgb(0x0066CC),
        boton: rgba(0x000000, 0.06),
        boton_encima: rgba(0x000000, 0.10),
        rojo: rgb(0xD70015),
        verde: rgb(0x248A3D),
        amarillo: rgb(0xB58100),
        naranja: rgb(0xC93400),
        naranja_texto: rgb(0x8A4B00),
        foto: rgb(0x5D7FA6),
        sobre_azul: rgb(0xFFFFFF),
    };
}

/// Una letra rapida del panel de la derecha.
#[derive(Debug, Clone)]
pub struct LetraRapida {
    pub letra: char,
    pub titulo: String,
    pub sub: String,
    /// El atajo de Windows de esa funcion, si tiene («Ctrl Alt X»).
    pub atajo: Option<String>,
}

/// Lo que sabe la vista previa de la captura elegida.
#[derive(Debug, Clone, Default)]
pub struct Detalle {
    /// «1920 × 1080 · 412 KB».
    pub medidas: Option<String>,
    /// «Caduca en 7 días» / «Conservada: no caduca». `true`: se puede
    /// conservar.
    pub caducidad: Option<(String, bool)>,
}

/// Todo lo que se pinta.
pub struct Vista<'a> {
    pub estado: &'a Estado,
    pub busquedas: &'a [String],
    pub letras: &'a [LetraRapida],
    /// El atajo propio del buscador, si el usuario le puso uno.
    pub atajo: Option<&'a str>,
    /// Las fotos ya cargadas (miniaturas y vista previa), por ruta.
    pub fotos: &'a HashMap<String, (ID2D1Bitmap1, u32, u32)>,
    pub detalle: Detalle,
    /// Si el elegido tiene mas acciones (el «…»).
    pub hay_menu: bool,
    /// Un aviso abajo («Copiado», «Captura borrada»).
    pub aviso: Option<&'a str>,
    pub raton: (f32, f32),
    pub scroll: f32,
}

const TAM: f32 = 14.0;
const TAM_SUB: f32 = 12.0;
const TAM_CAB: f32 = 11.0;
const TAM_TECLA: f32 = 11.0;

fn negrita() -> Letra<'static> {
    Letra { familia: "Segoe UI", negrita: true, cursiva: false, interlineado: None }
}

fn dentro(r: RectF, p: (f32, f32)) -> bool {
    p.0 >= r.x && p.0 < r.x + r.ancho && p.1 >= r.y && p.1 < r.y + r.alto
}

/// El ancho de una chapita de tecla.
fn ancho_tecla(p: &Pintor, t: &str, e: f32) -> f32 {
    p.medir_texto(t, TAM_TECLA * e).0 + 12.0 * e
}

/// **Una chapita de tecla** («Intro», «Ctrl C») con el centro vertical en
/// `cy`. Devuelve su ancho.
fn tecla(p: &Pintor, t: &str, x: f32, cy: f32, e: f32, pal: &Paleta, sobre_azul: bool) -> f32 {
    let (w, h) = p.medir_texto(t, TAM_TECLA * e);
    let caja = RectF { x, y: cy - 10.0 * e, ancho: w + 12.0 * e, alto: 20.0 * e };
    let (fondo, borde, color) = if sobre_azul {
        (rgba(0x000000, 0.25), rgba(0xFFFFFF, 0.12), rgb(0xE5E5EA))
    } else {
        (pal.tecla, pal.tecla_borde, pal.suave)
    };
    p.rellenar_redondeado(caja, 5.0 * e, fondo);
    p.trazar(caja, 1.0, borde);
    p.texto(t, x + 6.0 * e, cy - h / 2.0, TAM_TECLA * e, color);
    caja.ancho
}

/// Una tecla grande y en monoespaciada (la letra rapida).
fn tecla_grande(p: &Pintor, t: &str, x: f32, cy: f32, e: f32, pal: &Paleta) {
    let lado = 26.0 * e;
    let caja = RectF { x, y: cy - lado / 2.0, ancho: lado, alto: lado };
    p.rellenar_redondeado(caja, 5.0 * e, pal.tecla);
    p.trazar(caja, 1.0, pal.tecla_borde);
    let letra = Letra { familia: "Cascadia Mono", negrita: true, cursiva: false, interlineado: None };
    let (w, h) = p.medir_con_letra(t, 13.0 * e, SIN_PARTIR, &letra);
    p.texto_con_letra(t, x + (lado - w) / 2.0, cy - h / 2.0, 13.0 * e, SIN_PARTIR, &letra, pal.texto);
}

/// Un texto con sus letras resaltadas (las que coinciden con lo buscado),
/// en una linea con «…» si no cabe. `posiciones`: en UTF-16, como las da
/// el plugin.
#[allow(clippy::too_many_arguments)]
fn texto_resaltado(p: &Pintor, t: &str, posiciones: &[usize], x: f32, y: f32, tam: f32, ancho: f32, color: Color, marca: Color) {
    if !posiciones.is_empty() {
        let mut tramos: Vec<(u32, u32)> = Vec::new();
        for &i in posiciones {
            match tramos.last_mut() {
                Some((ini, largo)) if *ini + *largo == i as u32 => *largo += 1,
                _ => tramos.push((i as u32, 1)),
            }
        }
        p.con_recorte(RectF { x, y: y - 2.0, ancho, alto: tam * 2.0 }, |p| {
            for (ini, largo) in tramos {
                for c in p.cajas_de_trozo(t, tam, SIN_PARTIR, &[], ini, largo) {
                    p.rellenar_redondeado(
                        RectF { x: x + c.x - 1.0, y: y + c.y, ancho: c.ancho + 2.0, alto: c.alto },
                        3.0,
                        marca,
                    );
                }
            }
        });
    }
    p.texto_linea(t, x, y, tam, ancho, color);
}

/// El icono de material de un glifo del plugin (Segoe Fluent Icons en Flow).
pub fn icono_de_glifo(g: &str) -> &'static Icono {
    match g {
        x if x == glifo::CHAT => &mi::FORUM,
        x if x == glifo::TAREAS => &mi::CHECKLIST,
        x if x == glifo::LIENZO => &mi::DRAW,
        x if x == glifo::NOTA => &mi::EDIT,
        x if x == glifo::GRABAR => &mi::MIC,
        x if x == glifo::PIXPIN || x == glifo::PIN => &mi::PUSH_PIN,
        x if x == glifo::PROYECTO || x == glifo::CARPETA => &mi::FOLDER,
        x if x == glifo::ARCHIVO => &mi::DESCRIPTION,
        x if x == glifo::IMAGEN => &mi::IMAGE,
        x if x == glifo::AUDIO => &mi::LIBRARY_MUSIC,
        x if x == glifo::PENDIENTE => &mi::CHECK_BOX_OUTLINE_BLANK,
        x if x == glifo::HECHA => &mi::CHECK_BOX,
        x if x == glifo::ANADIR => &mi::LIBRARY_ADD,
        x if x == glifo::COPIAR => &mi::CONTENT_COPY,
        x if x == glifo::AVISO => &mi::FLAG,
        x if x == glifo::WINDOWS => &mi::LAUNCH,
        x if x == glifo::MINIAPP => &mi::TABLE_CHART,
        x if x == glifo::ADJUNTAR => &mi::ATTACH_FILE,
        x if x == glifo::LECCION => &mi::LIGHTBULB,
        x if x == glifo::CAPTURA => &mi::CROP,
        x if x == glifo::GALERIA => &mi::PHOTO_LIBRARY,
        x if x == glifo::MOVER => &mi::FORWARD,
        x if x == glifo::BORRAR => &mi::DELETE,
        x if x == glifo::CONSERVAR => &mi::BOOKMARK_ADD,
        x if x == glifo::VIDEO => &mi::PLAY_ARROW,
        _ => &mi::OPEN_IN_NEW,
    }
}

/// La extension del fichero de un resultado, si tiene («PDF»).
fn extension(r: &Resultado) -> Option<String> {
    let f = r.fichero.as_deref().or_else(|| r.ayuda_titulo.as_deref())?;
    let ext = std::path::Path::new(f).extension()?.to_str()?;
    (ext.len() <= 4).then(|| ext.to_uppercase())
}

/// **El cuadrado de la izquierda de una fila**: la foto si es una captura,
/// las letras de la extension si es un archivo, o el icono en el color de
/// su clase (verde tarea, amarillo leccion…).
fn cuadro(p: &Pintor, r: &Resultado, c: RectF, e: f32, pal: &Paleta, fotos: &HashMap<String, (ID2D1Bitmap1, u32, u32)>) {
    let radio = 8.0 * e;
    let foto = r.vista_previa.as_deref().or(r.icono.as_deref()).and_then(|f| fotos.get(f));
    if let Some((b, w, h)) = foto {
        p.rellenar_redondeado(c, radio, pal.foto);
        // Llenar el cuadro recortando lo que sobra (como `object-fit: cover`).
        let (w, h) = (*w as f32, *h as f32);
        let lado = w.min(h);
        let fuente = RectF { x: (w - lado) / 2.0, y: (h - lado) / 2.0, ancho: lado, alto: lado };
        p.con_recorte(c, |p| p.bitmap(b, c, Some(fuente), false));
        return;
    }
    let pestana = modelo::pestana_de(r);
    let (fondo, color) = match pestana {
        Pestana::Tareas => (con_alfa(pal.verde, 0.16), pal.verde),
        Pestana::Lecciones => (con_alfa(pal.amarillo, 0.14), pal.amarillo),
        Pestana::Capturas => (pal.foto, rgba(0xFFFFFF, 0.85)),
        Pestana::Archivos if extension(r).is_some() => (con_alfa(pal.rojo, 0.14), pal.rojo),
        _ => (pal.boton, pal.suave),
    };
    p.rellenar_redondeado(c, radio, fondo);
    if pestana == Pestana::Archivos {
        if let Some(ext) = extension(r) {
            let letra = negrita();
            let tam = 10.0 * e;
            let (w, h) = p.medir_con_letra(&ext, tam, SIN_PARTIR, &letra);
            p.texto_con_letra(&ext, c.x + (c.ancho - w) / 2.0, c.y + (c.alto - h) / 2.0, tam, SIN_PARTIR, &letra, color);
            return;
        }
    }
    let lado = 18.0 * e;
    p.icono(
        icono_de_glifo(r.glifo),
        RectF { x: c.x + (c.ancho - lado) / 2.0, y: c.y + (c.alto - lado) / 2.0, ancho: lado, alto: lado },
        color,
    );
}

fn icono_de_boton(b: Boton) -> &'static Icono {
    match b {
        Boton::Principal => &mi::PUSH_PIN,
        Boton::VerEnChat => &mi::FORUM,
        Boton::Copiar => &mi::CONTENT_COPY,
        Boton::Mas => &mi::MORE_VERT,
    }
}

fn rotulo_y_tecla(b: Boton, r: &Resultado, textos: &Catalogo) -> (String, &'static str) {
    match b {
        Boton::Principal => (textos.t(modelo::rotulo_principal(&r.accion)), "Intro"),
        Boton::VerEnChat => (textos.t("buscar-todo-ver-chat"), "Ctrl Intro"),
        Boton::Copiar => (textos.t("buscar-todo-accion-copiar"), "Ctrl C"),
        Boton::Mas => (String::new(), ""),
    }
}

/// El icono del boton azul: una chincheta si pinea, una flecha si abre…
fn icono_principal(r: &Resultado) -> &'static Icono {
    match modelo::rotulo_principal(&r.accion) {
        "buscar-todo-accion-pinear" => &mi::PUSH_PIN,
        "buscar-todo-accion-apuntar" | "buscar-todo-accion-crear" => &mi::LIBRARY_ADD,
        "buscar-todo-accion-capturar" => &mi::CROP,
        "buscar-todo-accion-grabar" => &mi::MIC,
        "buscar-todo-accion-enviar" => &mi::SEND,
        "buscar-todo-accion-copiar" => &mi::CONTENT_COPY,
        "buscar-todo-accion-hecha" => &mi::CHECK_BOX,
        "buscar-todo-accion-desmarcar" => &mi::CHECK_BOX_OUTLINE_BLANK,
        "buscar-todo-accion-pegar" => &mi::CONTENT_PASTE,
        "buscar-todo-accion-reproducir" => &mi::PLAY_ARROW,
        "buscar-todo-accion-mostrar" => &mi::FOLDER,
        _ => &mi::OPEN_IN_NEW,
    }
}

/// Lo que mide un boton de la fila (para colocarlo).
fn ancho_boton(p: &Pintor, b: Boton, r: &Resultado, textos: &Catalogo, e: f32) -> f32 {
    if b == Boton::Mas {
        return 34.0 * e;
    }
    let (rotulo, t) = rotulo_y_tecla(b, r, textos);
    10.0 * e + 15.0 * e + 7.0 * e + p.medir_texto(&rotulo, 13.0 * e).0 + 7.0 * e + ancho_tecla(p, t, e) + 10.0 * e
}

fn rotulo_pestana(p: Pestana, e: &Estado, textos: &Catalogo) -> (String, Option<String>) {
    let n = e.cuenta(p);
    let cuenta = (!e.consulta.trim().is_empty() || p != Pestana::Todo).then(|| n.to_string());
    (textos.t(p.clave()), cuenta.filter(|_| !e.es_inicio()))
}

/// **Pinta el buscador entero.** `ancho` y `alto` en pixeles; `e`, la
/// escala.
pub fn pintar(p: &Pintor, v: &Vista, pal: &Paleta, textos: &Catalogo, ancho: f32, alto: f32, e: f32) -> Disposicion {
    let est = v.estado;
    // Medir lo que hace falta para colocar.
    let anchos_pestanas: Vec<f32> = Pestana::TODAS
        .iter()
        .map(|pe| {
            let (t, n) = rotulo_pestana(*pe, est, textos);
            let mut a = 24.0 * e + p.medir_texto(&t, 13.0 * e).0;
            if let Some(n) = n {
                a += 6.0 * e + p.medir_texto(&n, 11.0 * e).0 + 12.0 * e;
            }
            a
        })
        .collect();
    let elegido = est.elegido();
    let botones: Vec<(Boton, f32)> = match elegido {
        Some(r) if !est.es_inicio() => modelo::botones_de(r, v.hay_menu)
            .into_iter()
            .map(|b| (b, ancho_boton(p, b, r, textos, e)))
            .collect(),
        _ => Vec::new(),
    };
    let anchos_fichas: Vec<f32> = v
        .busquedas
        .iter()
        .map(|b| 12.0 * e + p.medir_texto(b, 13.0 * e).0 + 6.0 * e + 22.0 * e + 6.0 * e)
        .collect();
    let letras: Vec<char> = v.letras.iter().map(|l| l.letra).collect();
    let lineas = est.lineas();
    let detalle_conservar = v.detalle.caducidad.as_ref().is_some_and(|(_, c)| *c);
    let d = disposicion::colocar(&Entrada {
        ancho,
        alto,
        e,
        inicio: est.es_inicio(),
        lineas: &lineas,
        elegido: est.elegido,
        anchos_pestanas: &anchos_pestanas,
        botones: &botones,
        anchos_fichas: &anchos_fichas,
        letras: &letras,
        opciones_menu: est.menu.as_ref().map_or(0, |m| m.opciones.len()),
        conservar: detalle_conservar && est.menu.is_none(),
        deshacer: v.aviso.is_some() && est.se_puede_deshacer,
        scroll: v.scroll,
    });

    // El fondo: la ventana entera redondeada, con su borde.
    p.limpiar_transparente();
    let todo = RectF { x: 0.0, y: 0.0, ancho, alto };
    p.rellenar_redondeado(todo, 18.0 * e, pal.fondo);
    // El panel de la derecha (sus esquinas de abajo las tapa el pie).
    p.con_recorte(d.panel, |p| p.rellenar(d.panel, pal.panel));
    p.rellenar(RectF { x: d.panel.x, y: d.panel.y, ancho: 1.0, alto: d.panel.alto }, pal.raya);

    pintar_campo(p, v, pal, textos, &d, e);
    pintar_pestanas(p, v, pal, textos, &d, e);
    if est.es_inicio() {
        pintar_inicio(p, v, pal, textos, &d, e);
    } else {
        pintar_lista(p, v, pal, textos, &d, &lineas, e);
        pintar_panel(p, v, pal, textos, &d, e);
    }
    pintar_pie(p, v, pal, textos, &d, e);
    if let Some(a) = v.aviso {
        pintar_aviso(p, a, v, pal, textos, &d, e);
    }
    p.trazar(RectF { x: 0.5, y: 0.5, ancho: ancho - 1.0, alto: alto - 1.0 }, 1.0, pal.borde);
    d
}

fn pintar_campo(p: &Pintor, v: &Vista, pal: &Paleta, textos: &Catalogo, d: &Disposicion, e: f32) {
    let est = v.estado;
    let c = d.campo;
    let lado = 22.0 * e;
    p.icono(&mi::SEARCH, RectF { x: 18.0 * e, y: c.y + (c.alto - lado) / 2.0, ancho: lado, alto: lado }, pal.apagado);
    let x = 18.0 * e + lado + 12.0 * e;
    let tam = if est.consulta.is_empty() { 20.0 } else { 22.0 } * e;
    // A la derecha: «Esc» en el inicio; la cuenta y el ✕ con resultados.
    let mut derecha = c.ancho - 18.0 * e;
    if let Some(b) = d.borrar {
        let encima = dentro(b, v.raton);
        p.rellenar_redondeado(b, 8.0 * e, if encima { pal.boton_encima } else { pal.boton });
        let l = 14.0 * e;
        p.icono(&mi::CLOSE, RectF { x: b.x + (b.ancho - l) / 2.0, y: b.y + (b.alto - l) / 2.0, ancho: l, alto: l }, pal.suave);
        derecha = b.x - 12.0 * e;
        let n = est.cuenta(est.pestana);
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("n", n);
        let cuenta = textos.t_args("buscar-todo-resultados", &args);
        let (w, h) = p.medir_texto(&cuenta, TAM_SUB * e);
        p.texto(&cuenta, derecha - w, c.y + (c.alto - h) / 2.0, TAM_SUB * e, pal.apagado);
        derecha -= w + 12.0 * e;
    } else {
        let w = ancho_tecla(p, "Esc", e);
        tecla(p, "Esc", derecha - w, c.y + c.alto / 2.0, e, pal, false);
        derecha -= w + 12.0 * e;
    }
    let hueco = (derecha - x).max(20.0 * e);
    // El cursor azul delante de la pista, o detras de lo escrito.
    let (texto, color) = if est.consulta.is_empty() {
        (textos.t("buscar-todo-pista"), pal.apagado)
    } else {
        (est.consulta.clone(), pal.texto)
    };
    let (w, h) = p.medir_texto(&texto, tam);
    let y = c.y + (c.alto - h) / 2.0;
    if est.consulta.is_empty() {
        p.rellenar(RectF { x: x - 2.0 * e, y: c.y + (c.alto - 26.0 * e) / 2.0, ancho: 2.0 * e, alto: 26.0 * e }, pal.azul);
        p.texto_linea(&texto, x + 6.0 * e, y, tam, hueco - 6.0 * e, color);
    } else {
        // Lo escrito largo se ve por el final, que es donde se escribe.
        let desplazado = (w - hueco + 4.0 * e).max(0.0);
        p.con_recorte(RectF { x, y: c.y, ancho: hueco, alto: c.alto }, |p| {
            p.texto(&texto, x - desplazado, y, tam, color);
        });
        let xc = x + w.min(hueco - 4.0 * e) + 1.0;
        p.rellenar(RectF { x: xc, y: c.y + (c.alto - 26.0 * e) / 2.0, ancho: 2.0 * e, alto: 26.0 * e }, pal.azul);
    }
}

fn pintar_pestanas(p: &Pintor, v: &Vista, pal: &Paleta, textos: &Catalogo, d: &Disposicion, e: f32) {
    let est = v.estado;
    let b = d.barra_pestanas;
    p.rellenar(RectF { x: 0.0, y: b.y + b.alto - 1.0, ancho: b.ancho, alto: 1.0 }, pal.raya);
    for (pe, r) in &d.pestanas {
        let on = *pe == est.pestana;
        if on {
            p.rellenar_redondeado(*r, 8.0 * e, pal.azul_fondo);
            p.trazar(*r, 1.0, pal.azul_borde);
        } else if dentro(disposicion::agrandada(*r, disposicion::OBJETIVO), v.raton) {
            p.rellenar_redondeado(*r, 8.0 * e, pal.encima);
        }
        let (t, n) = rotulo_pestana(*pe, est, textos);
        let (w, h) = p.medir_texto(&t, 13.0 * e);
        let x = r.x + 12.0 * e;
        p.texto(&t, x, r.y + (r.alto - h) / 2.0, 13.0 * e, if on { pal.texto } else { pal.suave });
        if let Some(n) = n {
            let letra = negrita();
            let (wn, hn) = p.medir_con_letra(&n, 11.0 * e, SIN_PARTIR, &letra);
            let chapa = RectF { x: x + w + 6.0 * e, y: r.y + (r.alto - 18.0 * e) / 2.0, ancho: wn + 12.0 * e, alto: 18.0 * e };
            p.rellenar_redondeado(chapa, 8.0 * e, if on { pal.azul_boton } else { pal.tecla });
            p.texto_con_letra(&n, chapa.x + 6.0 * e, chapa.y + (chapa.alto - hn) / 2.0, 11.0 * e, SIN_PARTIR, &letra, if on { pal.sobre_azul } else { pal.apagado });
        }
    }
    // «Ctrl Tab cambia», a la derecha.
    let rotulo = textos.t("buscar-todo-cambia");
    let (w, h) = p.medir_texto(&rotulo, TAM_SUB * e);
    let x = b.ancho - 14.0 * e - w;
    p.texto(&rotulo, x, b.y + (b.alto - h) / 2.0, TAM_SUB * e, pal.apagado);
    let wt = ancho_tecla(p, "Ctrl Tab", e);
    let ultima = d.pestanas.last().map_or(0.0, |(_, r)| r.x + r.ancho);
    if x - 6.0 * e - wt > ultima + 8.0 * e {
        tecla(p, "Ctrl Tab", x - 6.0 * e - wt, b.y + b.alto / 2.0, e, pal, false);
    }
}

fn cabecera(p: &Pintor, t: &str, r: RectF, e: f32, pal: &Paleta) {
    let letra = negrita();
    let t = t.to_uppercase();
    let (_, h) = p.medir_con_letra(&t, TAM_CAB * e, SIN_PARTIR, &letra);
    p.texto_con_letra(&t, r.x + 12.0 * e, r.y + (r.alto - h) / 2.0 + 2.0 * e, TAM_CAB * e, SIN_PARTIR, &letra, pal.apagado);
}

/// Una fila de resultado: cuadro, titulo con lo buscado resaltado y
/// subtitulo. `derecha`: lo que ocupa lo de su derecha.
#[allow(clippy::too_many_arguments)]
fn fila(p: &Pintor, r: &Resultado, caja: RectF, alto_fila: f32, derecha: f32, e: f32, pal: &Paleta, fotos: &HashMap<String, (ID2D1Bitmap1, u32, u32)>) {
    let lado = 34.0 * e;
    let c = RectF { x: caja.x + 12.0 * e, y: caja.y + (alto_fila - lado) / 2.0, ancho: lado, alto: lado };
    cuadro(p, r, c, e, pal, fotos);
    let x = c.x + lado + 12.0 * e;
    let ancho = (caja.x + caja.ancho - 12.0 * e - derecha - x).max(10.0);
    let (_, ht) = p.medir_texto("Ág", TAM * e);
    let (_, hs) = p.medir_texto("Ág", TAM_SUB * e);
    let y = caja.y + (alto_fila - ht - hs - 2.0 * e) / 2.0;
    let titulo = pixpin_render::lienzo::en_una_linea(&r.titulo);
    texto_resaltado(p, &titulo, &r.resaltado, x, y, TAM * e, ancho, pal.texto, con_alfa(pal.azul, 0.28));
    p.texto_linea(&pixpin_render::lienzo::en_una_linea(&r.subtitulo), x, y + ht + 2.0 * e, TAM_SUB * e, ancho, pal.apagado);
}

fn pintar_inicio(p: &Pintor, v: &Vista, pal: &Paleta, textos: &Catalogo, d: &Disposicion, e: f32) {
    let est = v.estado;
    if let Some(c) = d.cabecera_recientes {
        cabecera(p, &textos.t("buscar-todo-recientes"), c, e, pal);
        if let Some(b) = d.borrar_todas {
            let t = textos.t("buscar-todo-borrar-todas");
            let (w, h) = p.medir_texto(&t, TAM_SUB * e);
            p.texto(&t, b.x + b.ancho - w - 12.0 * e, b.y + (b.alto - h) / 2.0 + 2.0 * e, TAM_SUB * e, pal.enlace);
        }
    }
    for (i, f, x) in &d.fichas {
        let encima = dentro(*f, v.raton);
        p.rellenar_redondeado(*f, f.alto / 2.0, if encima { pal.ficha_encima } else { pal.ficha });
        let t = &v.busquedas[*i];
        let (_, h) = p.medir_texto(t, 13.0 * e);
        p.texto_linea(t, f.x + 12.0 * e, f.y + (f.alto - h) / 2.0, 13.0 * e, (x.x - f.x - 16.0 * e).max(4.0), pal.texto2);
        if dentro(disposicion::agrandada(*x, 28.0), v.raton) {
            p.rellenar_redondeado(*x, x.alto / 2.0, pal.boton_encima);
        }
        let l = 10.0 * e;
        p.icono(&mi::CLOSE, RectF { x: x.x + (x.ancho - l) / 2.0, y: x.y + (x.alto - l) / 2.0, ancho: l, alto: l }, pal.apagado);
    }
    if let Some(c) = d.cabecera_abiertos {
        cabecera(p, &textos.t("buscar-todo-abierto-hace-poco"), c, e, pal);
    }
    for (i, caja) in &d.lineas {
        let Some(Linea::Fila(ix)) = est.lineas().get(*i).copied() else { continue };
        let r = &est.resultados[ix];
        let elegida = *i == est.elegido;
        if elegida {
            p.rellenar_redondeado(*caja, 10.0 * e, pal.azul_fondo);
            p.trazar(*caja, 1.5 * e, pal.azul);
        } else if dentro(*caja, v.raton) {
            p.rellenar_redondeado(*caja, 10.0 * e, pal.encima);
        }
        let mut derecha = 0.0;
        if elegida {
            // «Intro Pinear» a la derecha de la elegida.
            let verbo = textos.t(modelo::rotulo_principal(&r.accion));
            let (wv, hv) = p.medir_texto(&verbo, TAM_SUB * e);
            let xv = caja.x + caja.ancho - 12.0 * e - wv;
            p.texto(&verbo, xv, caja.y + (caja.alto - hv) / 2.0, TAM_SUB * e, pal.apagado);
            let wt = ancho_tecla(p, "Intro", e);
            tecla(p, "Intro", xv - 6.0 * e - wt, caja.y + caja.alto / 2.0, e, pal, false);
            derecha = wv + wt + 18.0 * e;
        }
        fila(p, r, *caja, caja.alto, derecha, e, pal, v.fotos);
    }
    if let Some(pista) = d.pista_pegar {
        p.rellenar_redondeado(pista, 12.0 * e, con_alfa(pal.azul, 0.06));
        p.trazar_discontinuo(pista, 1.5 * e, con_alfa(pal.azul, 0.45));
        let l = 22.0 * e;
        p.icono(&mi::IMAGE, RectF { x: pista.x + 12.0 * e, y: pista.y + (pista.alto - l) / 2.0, ancho: l, alto: l }, pal.enlace);
        let x = pista.x + 12.0 * e + l + 12.0 * e;
        let t1 = textos.t("buscar-todo-pegar");
        let (w1, h1) = p.medir_texto(&t1, 13.0 * e);
        let y1 = pista.y + 10.0 * e;
        p.texto(&t1, x, y1, 13.0 * e, pal.texto2);
        let wt = tecla(p, "Ctrl V", x + w1 + 6.0 * e, y1 + h1 / 2.0, e, pal, false);
        let t2 = textos.t("buscar-todo-pegar-2");
        p.texto_linea(&t2, x + w1 + 12.0 * e + wt, y1, 13.0 * e, pista.x + pista.ancho - (x + w1 + 12.0 * e + wt) - 8.0 * e, pal.texto2);
        let ejemplo = textos.t("buscar-todo-pegar-ejemplo");
        let mono = Letra::de("Cascadia Mono");
        let y2 = y1 + h1 + 6.0 * e;
        let (we, he) = p.medir_con_letra(&ejemplo, 12.0 * e, SIN_PARTIR, &mono);
        p.texto_con_letra(&ejemplo, x, y2, 12.0 * e, SIN_PARTIR, &mono, pal.texto);
        let ficha = "[img 01]";
        let (wf, hf) = p.medir_con_letra(ficha, 12.0 * e, SIN_PARTIR, &mono);
        let cf = RectF { x: x + we + 6.0 * e, y: y2 - 1.0 * e, ancho: wf + 12.0 * e, alto: hf.max(he) + 2.0 * e };
        p.rellenar_redondeado(cf, 5.0 * e, pal.azul_boton);
        p.texto_con_letra(ficha, cf.x + 6.0 * e, y2, 12.0 * e, SIN_PARTIR, &mono, pal.sobre_azul);
    }
    // Las letras.
    if let Some(c) = d.cabecera_letras {
        cabecera(p, &textos.t("buscar-todo-letras"), c, e, pal);
    }
    for ((_, caja), l) in d.letras.iter().zip(v.letras) {
        if dentro(*caja, v.raton) {
            p.rellenar_redondeado(*caja, 10.0 * e, pal.encima);
        }
        tecla_grande(p, &l.letra.to_string(), caja.x + 12.0 * e, caja.y + caja.alto / 2.0, e, pal);
        let x = caja.x + 12.0 * e + 26.0 * e + 12.0 * e;
        let mut derecha = caja.x + caja.ancho - 12.0 * e;
        if let Some(a) = &l.atajo {
            let w = ancho_tecla(p, a, e);
            tecla(p, a, derecha - w, caja.y + caja.alto / 2.0, e, pal, false);
            derecha -= w + 8.0 * e;
        }
        let (_, ht) = p.medir_texto("Ág", TAM * e);
        let (_, hs) = p.medir_texto("Ág", TAM_SUB * e);
        let y = caja.y + (caja.alto - ht - hs - 2.0 * e) / 2.0;
        p.texto_linea(&l.titulo, x, y, TAM * e, derecha - x, pal.texto);
        p.texto_linea(&l.sub, x, y + ht + 2.0 * e, TAM_SUB * e, derecha - x, pal.apagado);
    }
}

#[allow(clippy::too_many_arguments)]
fn pintar_lista(p: &Pintor, v: &Vista, pal: &Paleta, textos: &Catalogo, d: &Disposicion, lineas: &[Linea], e: f32) {
    let est = v.estado;
    if lineas.is_empty() {
        // Nada: lo dice, y como seguir.
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("consulta", est.consulta.trim().to_string());
        let t = if est.consulta.trim().is_empty() { textos.t("buscar-todo-vacia") } else { textos.t_args("buscar-todo-ninguno", &args) };
        let sub = textos.t("buscar-todo-ninguno-pista");
        let x = d.lista.x + 24.0 * e;
        p.texto_linea(&t, x, d.lista.y + 28.0 * e, 15.0 * e, d.lista.ancho - 48.0 * e, pal.texto);
        p.texto_linea(&sub, x, d.lista.y + 54.0 * e, TAM_SUB * e, d.lista.ancho - 48.0 * e, pal.apagado);
        return;
    }
    let visibles = est.visibles().len();
    p.con_recorte(d.lista, |p| {
        for (i, caja) in &d.lineas {
            match lineas[*i] {
                Linea::Cabecera(g) => {
                    cabecera(p, &textos.t(g.clave()), *caja, e, pal);
                }
                Linea::Fila(ix) => {
                    let r = &est.resultados[ix];
                    let tarjeta = d.tarjeta.is_some_and(|t| t.y == caja.y);
                    let alto_fila = if tarjeta { disposicion::FILA_ELEGIDA * e } else { caja.alto };
                    if tarjeta {
                        p.rellenar_redondeado(*caja, 12.0 * e, pal.azul_fondo);
                        p.trazar(*caja, 1.5 * e, pal.azul);
                    } else if dentro(*caja, v.raton) {
                        p.rellenar_redondeado(*caja, 10.0 * e, pal.encima);
                    }
                    let mut derecha = 0.0;
                    if tarjeta && lineas.first() == Some(&Linea::Cabecera(Grupo::Mejor)) {
                        // «1 de 9», como en la maqueta.
                        let mut args = fluent_bundle::FluentArgs::new();
                        args.set("n", est.elegido.min(visibles.saturating_sub(1)) + 1);
                        args.set("total", visibles);
                        let t = textos.t_args("buscar-todo-n-de-m", &args);
                        let (w, h) = p.medir_texto(&t, TAM_SUB * e);
                        p.texto(&t, caja.x + caja.ancho - 12.0 * e - w, caja.y + (alto_fila - h) / 2.0, TAM_SUB * e, pal.apagado);
                        derecha = w + 8.0 * e;
                    } else if modelo::pestana_de(r) == Pestana::Acciones {
                        // La letra de la funcion, si tiene (la «t» de apuntar).
                        if let Some(l) = letra_de(r) {
                            let w = ancho_tecla(p, &l.to_string(), e);
                            tecla(p, &l.to_string(), caja.x + caja.ancho - 12.0 * e - w, caja.y + alto_fila / 2.0, e, pal, false);
                            derecha = w + 8.0 * e;
                        }
                    }
                    fila(p, r, *caja, alto_fila, derecha, e, pal, v.fotos);
                }
            }
        }
        // Los botones de la elegida.
        if let Some(r) = est.elegido() {
            for (b, caja) in &d.botones {
                let encima = dentro(*caja, v.raton);
                let azul = *b == Boton::Principal;
                let fondo = match (azul, encima) {
                    (true, _) => pal.azul_boton,
                    (false, true) => pal.boton_encima,
                    (false, false) => pal.boton,
                };
                p.rellenar_redondeado(*caja, 8.0 * e, fondo);
                let color = if azul { pal.sobre_azul } else { pal.texto };
                let l = 15.0 * e;
                let icono = if azul { icono_principal(r) } else { icono_de_boton(*b) };
                if *b == Boton::Mas {
                    p.icono(icono, RectF { x: caja.x + (caja.ancho - l) / 2.0, y: caja.y + (caja.alto - l) / 2.0, ancho: l, alto: l }, color);
                    continue;
                }
                let x = caja.x + 10.0 * e;
                p.icono(icono, RectF { x, y: caja.y + (caja.alto - l) / 2.0, ancho: l, alto: l }, color);
                let (rotulo, t) = rotulo_y_tecla(*b, r, textos);
                let (w, h) = p.medir_texto(&rotulo, 13.0 * e);
                let xr = x + l + 7.0 * e;
                p.texto(&rotulo, xr, caja.y + (caja.alto - h) / 2.0, 13.0 * e, color);
                tecla(p, t, xr + w + 7.0 * e, caja.y + caja.alto / 2.0, e, pal, azul);
            }
        }
    });
}

/// La letra rapida de un resultado-funcion (para su chapita).
pub fn letra_de(r: &Resultado) -> Option<char> {
    use pixpin_lanzador::consulta::Funcion;
    use pixpin_lanzador::resultados::Accion;
    let de_clave = r
        .clave
        .as_deref()
        .and_then(|c| c.strip_prefix("funcion/"))
        .and_then(Funcion::de_palabra)
        .and_then(Funcion::letra);
    if de_clave.is_some() {
        return de_clave;
    }
    // Lo que crea algo con lo escrito: la letra que lo hace directo.
    let Accion::Pedido(p) = &r.accion else { return None };
    match p.get("accion").and_then(serde_json::Value::as_str)? {
        "anadir_tarea" => Some('t'),
        "nota_nueva" => Some('n'),
        "lienzo_nuevo" => Some('l'),
        "capturar" => Some('c'),
        "pinear_ultima" => Some('u'),
        "leccion_nueva" => Some('a'),
        _ => None,
    }
}

fn pintar_panel(p: &Pintor, v: &Vista, pal: &Paleta, textos: &Catalogo, d: &Disposicion, e: f32) {
    let est = v.estado;
    let x = d.panel.x + 16.0 * e;
    let ancho = d.panel.ancho - 32.0 * e;
    if let Some(m) = &est.menu {
        if let Some(c) = d.cabecera_menu {
            let letra = negrita();
            p.texto_con_letra(&textos.t("buscar-todo-mas"), c.x, c.y + 4.0 * e, 15.0 * e, SIN_PARTIR, &letra, pal.texto);
            p.texto_linea(&m.titulo, c.x, c.y + 28.0 * e, TAM_SUB * e, c.ancho, pal.apagado);
        }
        for (i, caja) in &d.opciones_menu {
            let o = &m.opciones[*i];
            if *i == m.elegida {
                p.rellenar_redondeado(*caja, 10.0 * e, pal.azul_fondo);
                p.trazar(*caja, 1.5 * e, pal.azul);
            } else if dentro(*caja, v.raton) {
                p.rellenar_redondeado(*caja, 10.0 * e, pal.encima);
            }
            let l = 18.0 * e;
            p.icono(icono_de_glifo(o.glifo), RectF { x: caja.x + 10.0 * e, y: caja.y + (caja.alto - l) / 2.0, ancho: l, alto: l }, pal.suave);
            let xt = caja.x + 10.0 * e + l + 10.0 * e;
            let (_, h) = p.medir_texto(&o.titulo, 13.0 * e);
            p.texto_linea(&o.titulo, xt, caja.y + (caja.alto - h) / 2.0, 13.0 * e, caja.x + caja.ancho - xt - 8.0 * e, pal.texto);
        }
        return;
    }
    let Some(r) = est.elegido() else { return };
    let mut y = d.panel.y + 14.0 * e;
    // La foto grande.
    if let Some((b, w, h)) = r.vista_previa.as_deref().and_then(|f| v.fotos.get(f)) {
        let alto = 150.0 * e;
        let marco = RectF { x, y, ancho, alto };
        p.rellenar_redondeado(marco, 12.0 * e, pal.foto);
        let (w, h) = (*w as f32, *h as f32);
        let k = (ancho / w).max(alto / h);
        let fuente_w = ancho / k;
        let fuente_h = alto / k;
        let fuente = RectF { x: (w - fuente_w) / 2.0, y: (h - fuente_h) / 2.0, ancho: fuente_w, alto: fuente_h };
        p.con_recorte(marco, |p| p.bitmap(b, marco, Some(fuente), false));
        y += alto + 10.0 * e;
    } else {
        let lado = 56.0 * e;
        cuadro(p, r, RectF { x, y, ancho: lado, alto: lado }, e * 1.6, pal, v.fotos);
        y += lado + 12.0 * e;
    }
    let letra = negrita();
    let titulo = pixpin_render::lienzo::en_una_linea(&r.titulo);
    let (_, h) = p.medir_con_letra("Ág", 15.0 * e, SIN_PARTIR, &letra);
    p.con_recorte(RectF { x, y, ancho, alto: h + 2.0 }, |p| {
        p.texto_con_letra(&titulo, x, y, 15.0 * e, SIN_PARTIR, &letra, pal.texto);
    });
    y += h + 3.0 * e;
    let sub = v.detalle.medidas.clone().unwrap_or_else(|| r.subtitulo.clone());
    let (_, hs) = p.medir_texto_ajustado(&sub, TAM_SUB * e, ancho);
    let hs = hs.min(48.0 * e);
    p.con_recorte(RectF { x, y, ancho, alto: hs }, |p| p.texto_ajustado(&sub, x, y, TAM_SUB * e, ancho, pal.apagado));
    y += hs + 10.0 * e;
    // El texto entero (la ayuda del subtitulo: lo que dice la leccion, la
    // tarea, el mensaje), en su tarjeta.
    let tope_abajo = d.panel.y + d.panel.alto - 70.0 * e - if d.conservar.is_some() || v.detalle.caducidad.is_some() { 50.0 * e } else { 0.0 };
    if let Some(t) = r.ayuda_subtitulo.as_deref().filter(|t| !t.trim().is_empty() && *t != r.subtitulo) {
        let alto_max = (tope_abajo - y - 20.0 * e).max(0.0);
        if alto_max > 30.0 * e {
            let (_, ht) = p.medir_texto_ajustado(t, 13.0 * e, ancho - 24.0 * e);
            let alto = (ht + 20.0 * e).min(alto_max);
            let tarjeta = RectF { x, y, ancho, alto };
            p.rellenar_redondeado(tarjeta, 10.0 * e, pal.ficha);
            p.con_recorte(RectF { x: x + 12.0 * e, y: y + 10.0 * e, ancho: ancho - 24.0 * e, alto: alto - 20.0 * e }, |p| {
                p.texto_ajustado(t, x + 12.0 * e, y + 10.0 * e, 13.0 * e, ancho - 24.0 * e, pal.texto2);
            });
        }
    }
    // La caducidad de una captura.
    if let Some((t, se_puede)) = &v.detalle.caducidad {
        let banda_y = d.conservar.map_or(d.panel.y + d.panel.alto - 114.0 * e, |c| c.y - 4.0 * e);
        let banda = RectF { x, y: banda_y, ancho, alto: 40.0 * e };
        p.rellenar_redondeado(banda, 10.0 * e, con_alfa(pal.naranja, 0.10));
        let l = 16.0 * e;
        p.icono(&mi::ALARM, RectF { x: banda.x + 10.0 * e, y: banda.y + (banda.alto - l) / 2.0, ancho: l, alto: l }, pal.naranja);
        let xt = banda.x + 10.0 * e + l + 8.0 * e;
        let fin = d.conservar.map_or(banda.x + banda.ancho - 8.0 * e, |c| c.x - 8.0 * e);
        let (_, h) = p.medir_texto(t, TAM_SUB * e);
        p.texto_linea(t, xt, banda.y + (banda.alto - h) / 2.0, TAM_SUB * e, fin - xt, pal.naranja_texto);
        if let (Some(c), true) = (d.conservar, *se_puede) {
            p.rellenar_redondeado(c, 8.0 * e, if dentro(c, v.raton) { pal.boton_encima } else { pal.boton });
            let t = textos.t("buscar-todo-conservar");
            let (w, h) = p.medir_texto(&t, TAM_SUB * e);
            p.texto(&t, c.x + (c.ancho - w) / 2.0, c.y + (c.alto - h) / 2.0, TAM_SUB * e, pal.texto);
        }
    }
    // Abajo, las teclas de lo demas.
    let mut yb = d.panel.y + d.panel.alto - 14.0 * e;
    let mut pista = |tecla_t: &str, clave: &str| {
        yb -= 26.0 * e;
        let w = tecla(p, tecla_t, x, yb + 10.0 * e, e, pal, false);
        let t = textos.t(clave);
        let (_, h) = p.medir_texto(&t, TAM_SUB * e);
        p.texto_linea(&t, x + w + 6.0 * e, yb + 10.0 * e - h / 2.0, TAM_SUB * e, ancho - w - 6.0 * e, pal.apagado);
    };
    if modelo::ruta_de_captura(r).is_some() {
        pista("Supr", "buscar-todo-borrar");
    }
    if r.fichero.is_some() {
        pista("Alt Intro", "buscar-todo-abrir-con");
    }
}

fn pintar_pie(p: &Pintor, v: &Vista, pal: &Paleta, textos: &Catalogo, d: &Disposicion, e: f32) {
    let est = v.estado;
    let pie = d.pie;
    p.rellenar(RectF { x: 0.0, y: pie.y, ancho: pie.ancho, alto: 1.0 }, pal.raya);
    let cy = pie.y + pie.alto / 2.0;
    let mut x = 18.0 * e;
    let texto = |p: &Pintor, x: &mut f32, t: &str| {
        let (w, h) = p.medir_texto(t, TAM_SUB * e);
        p.texto(t, *x, cy - h / 2.0, TAM_SUB * e, pal.apagado);
        *x += w + 16.0 * e;
    };
    x += tecla(p, "↑", x, cy, e, pal, false) + 4.0 * e;
    x += tecla(p, "↓", x, cy, e, pal, false) + 6.0 * e;
    texto(p, &mut x, &textos.t("buscar-todo-pie-moverse"));
    let verbo = match est.elegido() {
        Some(r) if !est.es_inicio() && est.menu.is_none() => textos.t(modelo::rotulo_principal(&r.accion)),
        _ => textos.t("buscar-todo-pie-abrir"),
    };
    x += tecla(p, "Intro", x, cy, e, pal, false) + 6.0 * e;
    texto(p, &mut x, &verbo);
    if est.es_inicio() {
        x += tecla(p, "Ctrl V", x, cy, e, pal, false) + 6.0 * e;
        texto(p, &mut x, &textos.t("buscar-todo-pie-adjuntar"));
    } else if est.menu.is_some() {
        x += tecla(p, "←", x, cy, e, pal, false) + 6.0 * e;
        texto(p, &mut x, &textos.t("buscar-todo-pie-volver"));
    } else if v.hay_menu {
        x += tecla(p, "→", x, cy, e, pal, false) + 6.0 * e;
        texto(p, &mut x, &textos.t("buscar-todo-pie-mas"));
    }
    x += tecla(p, "Esc", x, cy, e, pal, false) + 6.0 * e;
    let cerrar = if est.consulta.is_empty() { "buscar-todo-pie-cerrar" } else { "buscar-todo-pie-borrar" };
    texto(p, &mut x, &textos.t(cerrar));
    // A la derecha: Flow y el atajo propio, si lo tiene.
    let mut derecha = pie.ancho - 18.0 * e;
    if let Some(a) = v.atajo {
        let w = ancho_tecla(p, a, e);
        tecla(p, a, derecha - w, cy, e, pal, false);
        derecha -= w + 10.0 * e;
    }
    let flow = textos.t("buscar-todo-pie-flow");
    let (w, h) = p.medir_texto(&flow, TAM_SUB * e);
    if derecha - w > x {
        p.texto(&flow, derecha - w, cy - h / 2.0, TAM_SUB * e, pal.apagado);
    }
}

#[allow(clippy::too_many_arguments)]
fn pintar_aviso(p: &Pintor, a: &str, v: &Vista, pal: &Paleta, textos: &Catalogo, d: &Disposicion, e: f32) {
    let (w, h) = p.medir_texto(a, 13.0 * e);
    let deshacer = d.deshacer;
    let extra = deshacer.map_or(0.0, |r| r.ancho + 8.0 * e);
    let ancho = w + 32.0 * e + extra;
    let fin = d.lista.x + d.lista.ancho - 16.0 * e;
    let caja = RectF { x: fin - ancho, y: d.pie.y - disposicion::AVISO_ARRIBA * e, ancho, alto: 44.0 * e };
    p.rellenar_redondeado(caja, 12.0 * e, pal.ficha_encima);
    p.trazar(caja, 1.0, pal.borde);
    p.texto(a, caja.x + 16.0 * e, caja.y + (caja.alto - h) / 2.0, 13.0 * e, pal.texto);
    if let Some(r) = deshacer {
        p.rellenar_redondeado(r, 8.0 * e, if dentro(r, v.raton) { pal.boton_encima } else { pal.boton });
        let t = textos.t("buscar-todo-deshacer");
        let (wt, ht) = p.medir_texto(&t, 13.0 * e);
        let wk = ancho_tecla(p, "Ctrl Z", e);
        let x = r.x + (r.ancho - wt - 6.0 * e - wk) / 2.0;
        p.texto(&t, x, r.y + (r.alto - ht) / 2.0, 13.0 * e, pal.enlace);
        tecla(p, "Ctrl Z", x + wt + 6.0 * e, r.y + r.alto / 2.0, e, pal, false);
    }
}
