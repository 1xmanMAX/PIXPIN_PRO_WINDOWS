//! **Los puntitos del estado de las IA** (1-nov-2026, el usuario: «unos
//! puntitos rojo o verde, en un sitio donde no molesten, para avisar de que
//! quiza tu API de DeepSeek, Jev o Claude no funciona»).
//!
//! Una columna de tres puntitos de 5 px pegada a la izquierda del boton de
//! sincronizar de la lista (`Disposicion::luces_ia`): Claude, DeepSeek y Jev,
//! de arriba abajo. Verde lleno, funciona; rojo lleno, falla; aro rojo, sin
//! clave; gris tenue, con clave y aun sin comprobar. Con el raton encima, un
//! globo con cada servicio y cuando se supo; al pulsar, un menu con lo
//! mismo, «Comprobar ahora» y las claves en Ajustes → Asistente.
//!
//! Lo que se sabe vive en `crate::estado_ia` (y sus reglas, con pruebas, en
//! `pixpin_ui::estado_ia`); aqui solo se pinta y se arma el menu.

use fluent_bundle::FluentArgs;
use pixpin_render::icono::material as mi;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_store::Catalogo;
use pixpin_ui::chat::Disposicion;
use pixpin_ui::estado_ia::{Ficha, Luz, Servicio, Tablero, hace};

use super::{Accion, EntradaMenu, Tema, entrada, hex, rf};

/// Lo que hace falta para pintarlos: lo que se sabe y la hora del estado.
pub(super) struct Luces {
    pub tablero: Tablero,
    pub ahora: u64,
}

impl Luces {
    pub fn de_ahora() -> Luces {
        Luces {
            tablero: crate::estado_ia::foto(),
            ahora: crate::estado_ia::ahora_ms(),
        }
    }
}

/// Si el tema es de dia (la cabecera clara): los verdes y rojos de noche
/// van un punto mas claros para que se vean igual.
fn de_dia(tema: &Tema) -> bool {
    let c = tema.cabecera;
    c.r + c.g + c.b > 1.5
}

fn color(luz: Luz, tema: &Tema) -> Color {
    let dia = de_dia(tema);
    match luz {
        Luz::Verde => hex(if dia { 0x43a047 } else { 0x66bb6a }),
        Luz::Roja | Luz::Hueca => hex(if dia { 0xe53935 } else { 0xef5350 }),
        Luz::Gris => Color {
            a: 0.55,
            ..tema.apagado
        },
    }
}

/// **Los puntitos**, en la cabecera de la lista.
pub(super) fn pintar(p: &Pintor, d: &Disposicion, tema: &Tema, escala: u32, luces: &Luces) {
    let puntos = d.puntitos_ia(Servicio::TODOS.len(), escala);
    for (s, r) in Servicio::TODOS.into_iter().zip(puntos) {
        let r = rf(r);
        let centro = (r.x + r.ancho / 2.0, r.y + r.alto / 2.0);
        let radio = r.ancho / 2.0;
        let luz = luces.tablero.estado(s).luz();
        p.circulo(centro, radio, color(luz, tema));
        if luz == Luz::Hueca {
            // Solo el aro: se ve que falta, sin gritar como un fallo.
            let grosor = (radio * 0.45).max(1.0);
            p.circulo(centro, (radio - grosor).max(0.5), tema.cabecera);
        }
    }
}

/// La frase de un servicio: «Claude · funciona · hace 3 min».
fn linea(textos: &Catalogo, s: Servicio, f: &Ficha, ahora: u64) -> String {
    if f.comprobando() {
        return format!("{} · {}", s.nombre(), textos.t("ia-estado-comprobando"));
    }
    let mut args = FluentArgs::new();
    if let Some(c) = f.estado.codigo() {
        args.set("codigo", c.to_string());
    }
    let mut l = format!(
        "{} · {}",
        s.nombre(),
        textos.t_args(f.estado.clave(), &args)
    );
    if f.hay_clave()
        && let Some(cuando) = f.cuando
    {
        let (clave, n) = hace(cuando, ahora);
        let mut args = FluentArgs::new();
        args.set("n", n.to_string());
        l.push_str(" · ");
        l.push_str(&textos.t_args(clave, &args));
    }
    l
}

/// Las lineas del globo: un servicio por linea y, al final, que hace el clic.
fn lineas(textos: &Catalogo, luces: &Luces) -> Vec<(String, Option<Luz>)> {
    let mut v: Vec<(String, Option<Luz>)> = Servicio::TODOS
        .into_iter()
        .map(|s| {
            let f = luces.tablero.ficha(s);
            (linea(textos, s, f, luces.ahora), Some(f.estado.luz()))
        })
        .collect();
    v.push((textos.t("ia-estado-pista"), None));
    v
}

/// **El globo** con el raton encima: debajo de los puntitos, alineado a la
/// derecha con el boton de sincronizar. Se pinta el ultimo, encima de todo.
pub(super) fn pintar_globo(
    p: &Pintor,
    d: &Disposicion,
    tema: &Tema,
    escala: u32,
    textos: &Catalogo,
    luces: &Luces,
) {
    let zona = d.luces_ia(escala);
    if zona.ancho == 0 {
        return;
    }
    let e = escala as f32 / 100.0;
    let tam = 12.0 * e;
    let lineas = lineas(textos, luces);
    let alto_linea = (tam * 1.5).ceil();
    let (relleno, punto, hueco) = (10.0 * e, 6.0 * e, 7.0 * e);
    let ancho_texto = lineas
        .iter()
        .map(|(t, _)| p.medir_texto(t, tam).0)
        .fold(0.0f32, f32::max);
    let ancho = ancho_texto + 2.0 * relleno + punto + hueco;
    let alto = lineas.len() as f32 * alto_linea + 2.0 * relleno - (alto_linea - tam * 1.2);
    let boton = d.boton_sincro(escala);
    let derecha = (boton.x + boton.ancho as i32) as f32;
    let caja = RectF {
        x: (derecha - ancho).max(4.0 * e),
        y: (zona.y + zona.alto as i32) as f32 + 6.0 * e,
        ancho,
        alto,
    };
    // El mismo globo oscuro que el de los botones de las barras
    // (`globo_pista`), de dia y de noche.
    p.rellenar_redondeado(
        caja,
        8.0 * e,
        Color {
            r: 0.106,
            g: 0.106,
            b: 0.122,
            a: 0.96,
        },
    );
    let mut y = caja.y + relleno;
    for (texto, luz) in &lineas {
        let x = caja.x + relleno;
        match luz {
            Some(l) => {
                let centro = (x + punto / 2.0, y + tam * 0.62);
                // Sobre el globo oscuro, los colores de noche.
                p.circulo(centro, punto / 2.0, color(*l, tema));
                if *l == Luz::Hueca {
                    p.circulo(
                        centro,
                        (punto / 2.0 - (1.4 * e).max(1.0)).max(0.5),
                        Color {
                            r: 0.106,
                            g: 0.106,
                            b: 0.122,
                            a: 1.0,
                        },
                    );
                }
                p.texto(texto, x + punto + hueco, y, tam, Color::BLANCO);
            }
            None => p.texto(
                texto,
                x + punto + hueco,
                y,
                tam,
                Color {
                    a: 0.6,
                    ..Color::BLANCO
                },
            ),
        }
        y += alto_linea;
    }
}

/// **El menu** al pulsar los puntitos: cada servicio (lleva a sus claves),
/// «Comprobar ahora» y las claves en Ajustes.
pub(super) fn menu(textos: &Catalogo, luces: &Luces) -> Vec<EntradaMenu> {
    let mut v: Vec<EntradaMenu> = Servicio::TODOS
        .into_iter()
        .map(|s| {
            let f = luces.tablero.ficha(s);
            entrada(None, linea(textos, s, f, luces.ahora), Accion::ClavesIa)
        })
        .collect();
    v.push(
        entrada(
            Some(&mi::REPLAY),
            textos.t("ia-estado-comprobar"),
            Accion::ComprobarIa,
        )
        .separada(),
    );
    v.push(entrada(
        Some(&mi::SETTINGS),
        textos.t("ia-estado-claves"),
        Accion::ClavesIa,
    ));
    v
}

/// **Ir a las claves**: Ajustes → Asistente. Los ajustes viven en el hilo
/// principal: se piden con el mismo `WM_COMMAND` que manda la bandeja.
/// `false` si no se pudo pedir.
pub(super) fn abrir_las_claves() -> bool {
    crate::ventana_ajustes::ir_a_las_claves();
    pixpin_shell::mensajero::pedir_ventana_principal(
        pixpin_store::comandos::Comando::AbrirAjustes.id(),
    )
}
