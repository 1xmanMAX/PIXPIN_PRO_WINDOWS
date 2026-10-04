//! **Un Excel del chat es un libro de tablas** (D11), como un PDF del chat
//! es un documento de paginas. Es `guardados/LibroDelChat.kt` del movil.
//!
//! Al tocar la burbuja de un `.xlsx`, `.ods`, `.csv` o `.tsv` se lee la
//! primera vez (`pixpin_proyecto::importar_hojas`) y cada hoja queda como
//! una tabla del proyecto, con una referencia **sacada del mensaje**
//! (`libro-<mensaje>-<hoja>`, la misma que el movil): la proxima vez que se
//! toque el archivo se abre esa tabla, con lo que se haya cambiado en ella,
//! en vez de volver a leer el libro encima.
//!
//! En el PC la tabla es un mensaje (ver la cabecera de
//! `pixpin_proyecto::tabla`), asi que las hojas aparecen en la conversacion,
//! detras del libro, y se abren y se comparten como cualquier otra tabla.

use super::*;
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_proyecto::importar_hojas::{self, NoSeLee};

/// Si el mensaje es un libro de hojas (o un `.xls`, que se reconoce para
/// decir que no se lee).
pub(super) fn es_libro(m: &Mensaje) -> bool {
    m.clase == Some(Clase::Archivo)
        && m.ruta.as_deref().is_some_and(|r| !r.is_empty())
        && (importar_hojas::es_libro(&m.nombre)
            || m.ruta.as_deref().is_some_and(importar_hojas::es_libro))
}

/// La referencia de la tabla de la hoja `hoja` del libro `m`
/// (`LibroDelChat.idDeHoja`).
pub(super) fn id_de_hoja(m: &Mensaje, hoja: usize) -> String {
    format!("libro-{}-{hoja}", m.id)
}

/// Las tablas que ya salieron de este libro, en el orden de sus hojas.
pub(super) fn tablas_hechas(mensajes: &[Mensaje], libro: &Mensaje) -> Vec<usize> {
    let prefijo = format!("libro-{}-", libro.id);
    let mut hechas: Vec<(usize, usize)> = mensajes
        .iter()
        .enumerate()
        .filter(|(_, m)| m.miniapp.as_deref() == Some(pixpin_proyecto::tabla::MINIAPP))
        .filter_map(|(i, m)| {
            let hoja = m
                .referencia
                .as_deref()?
                .strip_prefix(&prefijo)?
                .parse()
                .ok()?;
            Some((hoja, i))
        })
        .collect();
    hechas.sort();
    hechas.into_iter().map(|(_, i)| i).collect()
}

/// Una hoja ya leida y escrita en JSON, lista para ser mensaje: el JSON se
/// hace en el hilo que lee, que en una tabla de cuarenta mil celdas son
/// decenas de milisegundos que no le tocan a la ventana.
pub(super) struct HojaLista {
    pub nombre: String,
    pub documento: String,
}

/// Los mensajes de las tablas de un libro recien leido, uno por hoja.
pub(super) fn mensajes_de_hojas(
    libro: &Mensaje,
    hojas: &[HojaLista],
    sello: &pixpin_proyecto::cuaderno::Sello,
) -> Vec<Mensaje> {
    hojas
        .iter()
        .enumerate()
        .map(|(i, h)| {
            // El id es la hora: con varias hojas en el mismo milisegundo se
            // pisarian, asi que cada una va un milisegundo detras.
            let sello = pixpin_proyecto::cuaderno::Sello {
                cuando: sello.cuando + i as i64,
                numero: sello.numero + i as i64,
                ..sello.clone()
            };
            let mut m = Mensaje::miniapp(
                pixpin_proyecto::tabla::MINIAPP,
                &h.nombre,
                &h.documento,
                &sello,
            );
            m.referencia = Some(id_de_hoja(libro, i));
            m
        })
        .collect()
}

/// Lo que devuelve el hilo que lee el libro.
pub(super) struct Leido {
    /// Los mensajes de las hojas, ya sellados con los numeros que tocaban
    /// al tocar el libro, y su linea del cuaderno ya escrita: serializar el
    /// JSON de una tabla grande es lo mas caro de todo.
    mensajes: Vec<Mensaje>,
    lineas: Vec<String>,
    sello: pixpin_proyecto::cuaderno::Sello,
    /// Las tablas para la ojeada de cada burbuja.
    vistas: Vec<pixpin_proyecto::tabla::Tabla>,
    /// Y la primera otra vez, para la hoja que se abre: clonar cuarenta mil
    /// celdas tambien se hace en el hilo.
    primera: Option<pixpin_proyecto::tabla::Tabla>,
    /// Lo que vale cada celda de la primera, para pintarla sin calcular.
    valores: std::collections::BTreeMap<String, pixpin_proyecto::formula::Valor>,
    como_valor: usize,
}

/// **Leer el libro, fuera de la ventana**: todo lo caro (descomprimir,
/// recorrer el XML, comparar las formulas, escribir el JSON y calcular la
/// primera hoja).
pub(super) fn leer_libro(
    ruta: &std::path::Path,
    libro: &Mensaje,
    sello: pixpin_proyecto::cuaderno::Sello,
) -> Result<Leido, NoSeLee> {
    let hojas = importar_hojas::leer(ruta, &libro.nombre, sello.cuando)?;
    let como_valor = hojas.iter().map(|h| h.formulas_como_valor).sum();
    let primera = hojas.first().map(|h| h.tabla.clone());
    let valores = primera
        .as_ref()
        .map(pixpin_proyecto::formula::evaluar_todo)
        .unwrap_or_default();
    let listas: Vec<HojaLista> = hojas
        .iter()
        .filter_map(|h| {
            Some(HojaLista {
                nombre: h.nombre.clone(),
                documento: h.tabla.escribir().ok()?,
            })
        })
        .collect();
    let mensajes = mensajes_de_hojas(libro, &listas, &sello);
    let lineas = mensajes
        .iter()
        .map(pixpin_proyecto::cuaderno::linea_de)
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| NoSeLee::Disco(e.to_string()))?;
    Ok(Leido {
        mensajes,
        lineas,
        sello,
        vistas: hojas.into_iter().map(|h| h.tabla).collect(),
        primera,
        valores,
        como_valor,
    })
}

/// El libro que se esta leyendo: de que proyecto y mensaje es, y por donde
/// llega. Uno a la vez: tocar otra vez mientras se lee no lanza otro hilo.
struct Lectura {
    proyecto: String,
    libro: Mensaje,
    rx: std::sync::mpsc::Receiver<Result<Leido, NoSeLee>>,
}

thread_local! {
    /// Vive en el hilo de la ventana del chat, como el resto de su estado.
    static LEYENDO: std::cell::RefCell<Option<Lectura>> = const { std::cell::RefCell::new(None) };
}

/// Si hay un libro leyendose: la ventana vuelve a mirar cada poco, porque
/// del hilo no llega ningun evento.
pub(super) fn leyendo() -> bool {
    LEYENDO.with(|l| l.borrow().is_some())
}

/// **Tocar un libro**: abre su primera tabla; si es la primera vez, lanza su
/// lectura en otro hilo y la abre al llegar ([`latido`]). `true` si ya se
/// hizo lo que tocaba; `false` si no es un libro o se prefiere abrirlo fuera.
pub(super) fn abrir_libro(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    indice: usize,
    textos: &Catalogo,
) -> bool {
    let Some(libro) = a.mensajes.get(indice) else {
        return false;
    };
    if !es_libro(libro) {
        return false;
    }
    if let Some(&primera) = tablas_hechas(&a.mensajes, libro).first() {
        abrir_hoja(a, primera);
        return true;
    }
    if leyendo() {
        return true;
    }
    let Some(ruta) = libro
        .ruta
        .as_deref()
        .and_then(|r| pixpin_proyecto::vista::ruta_real(ubicacion.raiz(), &a.ficha.id, r))
        .filter(|r| r.is_file())
    else {
        return false;
    };
    // Un `.xls` se reconoce por el nombre, sin leer nada: se pregunta ya.
    if importar_hojas::extension(&libro.nombre) == "xls" {
        return !preguntar_xls(textos);
    }
    let (tx, rx) = std::sync::mpsc::channel();
    // El sello se pone ya: los mensajes salen hechos del hilo. Si mientras
    // se lee entra otro mensaje, al llegar se renumeran (`meter`).
    let sello = pixpin_proyecto::cuaderno::Sello {
        cuando: pixpin_shell::entorno::ahora_utc_ms(),
        numero: a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1,
        aparato: pixpin_proyecto::identidad::Identidad::leer_o_crear(ubicacion.raiz(), "PC")
            .map(|i| i.yo.codigo())
            .unwrap_or_default(),
        proyecto: a.ficha.id.clone(),
    };
    let para_el_hilo = libro.clone();
    let hilo = std::thread::Builder::new()
        .name("pixpin-libro".into())
        .spawn(move || {
            let _ = tx.send(leer_libro(&ruta, &para_el_hilo, sello));
        });
    if let Err(e) = hilo {
        tracing::warn!(?e, "no se pudo lanzar la lectura del libro");
        return false;
    }
    let lectura = Lectura {
        proyecto: a.ficha.id.clone(),
        libro: libro.clone(),
        rx,
    };
    LEYENDO.with(|l| *l.borrow_mut() = Some(lectura));
    true
}

/// Como el movil: se dice que no se lee y por que. Y como aqui suele haber
/// un Excel instalado, se ofrece abrirlo con el. `true` si se quiere abrir
/// fuera.
fn preguntar_xls(textos: &Catalogo) -> bool {
    pixpin_shell::dialogo::preguntar(
        windows::Win32::Foundation::HWND::default(),
        &textos.t("libro-titulo"),
        &format!(
            "{}\n\n{}",
            textos.t("libro-xls-antiguo"),
            textos.t("libro-abrir-fuera")
        ),
    )
}

/// Lo que haya llegado del hilo, sin esperar.
fn recoger() -> Option<(Lectura, Option<Result<Leido, NoSeLee>>)> {
    use std::sync::mpsc::TryRecvError;
    LEYENDO.with(|l| {
        let mut l = l.borrow_mut();
        let r = match l.as_ref()?.rx.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => return None,
            // El hilo murio sin decir nada: se dice que no se pudo leer.
            Err(TryRecvError::Disconnected) => None,
        };
        Some((l.take()?, r))
    })
}

/// **Lo que llega del hilo**: se mira una vez por vuelta del bucle del chat.
/// Mete las tablas en la conversacion y abre la primera. `None` si no ha
/// llegado nada; si llego, lo que hay que decir abajo, si algo. Aqui solo va lo barato: hacer los mensajes, una
/// linea por hoja al cuaderno y colocar lo que ya viene hecho.
pub(super) fn latido(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    textos: &Catalogo,
) -> Option<Option<String>> {
    let (lectura, resultado) = recoger()?;
    Some(recibir(ubicacion, a, textos, lectura, resultado))
}

fn recibir(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    textos: &Catalogo,
    lectura: Lectura,
    resultado: Option<Result<Leido, NoSeLee>>,
) -> Option<String> {
    let leido = match resultado {
        None => return Some(textos.t("libro-no-es")),
        Some(Ok(l)) => l,
        Some(Err(NoSeLee::XlsAntiguo)) => {
            // Un .xls con otro nombre: se sabe al leer su firma.
            if preguntar_xls(textos)
                && let Some(ruta) = lectura.libro.ruta.as_deref().and_then(|r| {
                    pixpin_proyecto::vista::ruta_real(ubicacion.raiz(), &lectura.proyecto, r)
                })
                && let Err(e) = pixpin_shell::abrir::abrir(&ruta)
            {
                tracing::warn!(?e, "no se pudo abrir el libro fuera");
            }
            return None;
        }
        Some(Err(e)) => {
            tracing::warn!(?e, "libro que no se deja leer");
            return Some(textos.t(match e {
                NoSeLee::SinHojas => "libro-sin-hojas",
                _ => "libro-no-es",
            }));
        }
    };
    meter(ubicacion, a, &lectura.proyecto, leido);
    None
}

/// Mete lo leido en la conversacion `proyecto`. Si mientras se leia se
/// cambio de proyecto, las tablas van a su cuaderno igual (el libro es de
/// alli) y se veran al volver; si no, se colocan y se abre la primera.
fn meter(ubicacion: &Ubicacion, a: &mut Abierto, proyecto: &str, leido: Leido) {
    let raiz = ubicacion.raiz();
    let es_este = a.ficha.id == proyecto;
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, proyecto);
    let Leido {
        mut mensajes,
        mut lineas,
        sello,
        vistas,
        primera: tabla_primera,
        valores,
        como_valor,
    } = leido;
    // Si mientras se leia entro otro mensaje, los numeros que se reservaron
    // al tocar ya estan cogidos: se corren y se vuelven a escribir las
    // lineas. Es lo lento, y casi nunca pasa.
    let siguiente = if es_este {
        a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1
    } else {
        pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta)
            .map(|c| c.siguiente_numero(Some(proyecto)))
            .unwrap_or(sello.numero)
    };
    if siguiente > sello.numero {
        let corrimiento = siguiente - sello.numero;
        for m in &mut mensajes {
            m.numero += corrimiento;
        }
        lineas = mensajes
            .iter()
            .filter_map(|m| pixpin_proyecto::cuaderno::linea_de(m).ok())
            .collect();
    }
    if let Err(e) = pixpin_proyecto::cuaderno::anadir_lineas(&carpeta, &lineas) {
        tracing::warn!(?e, "no se pudo guardar las tablas del libro");
        return;
    }
    let cuantas = mensajes.len();
    let mut primera = None;
    if es_este {
        for (m, vista) in mensajes.into_iter().zip(
            vistas
                .into_iter()
                .map(Some)
                .chain(std::iter::repeat_with(|| None)),
        ) {
            // Una tabla sin nada escrito no ensena rejilla (`leer_vista`).
            a.vistas.push(
                vista
                    .filter(|t| !t.celdas.is_empty())
                    .map(|t| Ojeada::Tabla(Box::new(t))),
            );
            a.mensajes.push(m);
            primera.get_or_insert(a.mensajes.len() - 1);
        }
    }
    tracing::info!(hojas = cuantas, como_valor, "libro leido como tablas");
    let mut indice = pixpin_proyecto::almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == proyecto) {
        f.tocado = ahora;
        if let Err(e) = indice.guardar(raiz) {
            tracing::warn!(?e, "no se pudo apuntar el cambio del proyecto");
        }
    }
    if !es_este {
        return;
    }
    a.ficha.tocado = ahora;
    a.colocado.borrow_mut().ancho = 0;
    if let (Some(i), Some(tabla)) = (primera, tabla_primera) {
        // Se abre con la tabla y los valores que ya trae el hilo: releer el
        // JSON del mensaje y recalcular aqui seria hacerlo dos veces, y en la
        // ventana.
        let celda_valores = std::cell::OnceCell::new();
        let _ = celda_valores.set(valores);
        a.hoja = Some(HojaAbierta {
            indice: i,
            tabla,
            sel: pixpin_proyecto::tabla::Ref {
                columna: 0,
                fila: 0,
            },
            edicion: None,
            scroll_x: 0,
            scroll_y: 0,
            tocada: false,
            valores: celda_valores,
        });
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn libro() -> Mensaje {
        Mensaje {
            id: "1700".into(),
            clase: Some(Clase::Archivo),
            nombre: "Cuentas.xlsx".into(),
            ruta: Some("archivos/Cuentas.xlsx".into()),
            ..Default::default()
        }
    }

    #[test]
    fn un_excel_del_chat_es_un_libro_y_una_foto_no() {
        assert!(es_libro(&libro()));
        let mut csv = libro();
        csv.nombre = "datos.csv".into();
        assert!(es_libro(&csv));
        let mut sin_nombre = libro();
        sin_nombre.nombre = String::new();
        assert!(es_libro(&sin_nombre), "sin nombre, por la ruta");
        // Casos negativos: otra clase, sin fichero u otra extension.
        let mut nota = libro();
        nota.clase = Some(Clase::Nota);
        assert!(!es_libro(&nota));
        let mut sin_ruta = libro();
        sin_ruta.ruta = None;
        assert!(!es_libro(&sin_ruta));
        let mut pdf = libro();
        pdf.nombre = "a.pdf".into();
        pdf.ruta = Some("archivos/a.pdf".into());
        assert!(!es_libro(&pdf));
    }

    #[test]
    fn cada_hoja_es_una_tabla_con_la_referencia_del_libro_y_en_su_orden() {
        let l = libro();
        let hojas = vec![
            HojaLista {
                nombre: "Cuentas · Gastos".into(),
                documento: importar_hojas::de_csv("a;1", "Cuentas · Gastos", false, 0)
                    .tabla
                    .escribir()
                    .unwrap(),
            },
            HojaLista {
                nombre: "Cuentas · Resumen".into(),
                documento: importar_hojas::de_csv("b;2", "Cuentas · Resumen", false, 0)
                    .tabla
                    .escribir()
                    .unwrap(),
            },
        ];
        let sello = pixpin_proyecto::cuaderno::Sello {
            cuando: 5000,
            numero: 9,
            aparato: "K7Q2".into(),
            proyecto: "p".into(),
        };
        let nuevos = mensajes_de_hojas(&l, &hojas, &sello);
        assert_eq!(nuevos.len(), 2);
        assert_eq!(nuevos[0].referencia.as_deref(), Some("libro-1700-0"));
        assert_eq!(nuevos[1].referencia.as_deref(), Some("libro-1700-1"));
        assert_ne!(
            nuevos[0].id, nuevos[1].id,
            "dos hojas en el mismo milisegundo no se pisan"
        );
        assert_eq!(nuevos[1].numero, 10);
        assert_eq!(nuevos[0].nombre, "Cuentas · Gastos");
        let t = pixpin_proyecto::tabla::Tabla::leer(&nuevos[1].texto).unwrap();
        assert_eq!(t.celda(pixpin_proyecto::tabla::ref_de("B1").unwrap()), "2");

        // Y la segunda vez se encuentran, en el orden de las hojas aunque
        // esten desordenadas en el chat.
        let mut chat = vec![
            l.clone(),
            nuevos[1].clone(),
            Mensaje::default(),
            nuevos[0].clone(),
        ];
        assert_eq!(tablas_hechas(&chat, &l), vec![3, 1]);
        // Caso negativo: las tablas de otro libro no son de este.
        let mut otro = l.clone();
        otro.id = "17".into();
        assert!(
            tablas_hechas(&chat, &otro).is_empty(),
            "libro-17- no es prefijo de libro-1700-"
        );
        chat.clear();
        assert!(tablas_hechas(&chat, &l).is_empty());
    }

    #[test]
    fn un_libro_grande_se_lee_en_otro_hilo_sin_parar_la_ventana() {
        // 10 000 filas con una columna de saldos (40 000 celdas): leerlo en
        // la ventana la dejaba congelada. Ni tocarlo ni recibirlo pueden
        // pasar de 50 ms en el hilo de la interfaz.
        let raiz = std::env::temp_dir().join(format!("pixpin-libro-hilo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let u = Ubicacion::Portable { raiz: raiz.clone() };
        let ficha = pixpin_proyecto::almacen::Ficha::nueva("obra", 0, "PC");
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, &ficha.id);
        std::fs::create_dir_all(&carpeta).unwrap();
        let mut csv = String::from("Concepto;Importe;Doble;Saldo\n");
        for f in 2..=10_001 {
            let saldo = if f == 2 {
                "=C2".to_string()
            } else {
                format!("=D{}+C{f}", f - 1)
            };
            csv.push_str(&format!("Fila {f};{};=B{f}*2;{saldo}\n", f % 7));
        }
        std::fs::write(carpeta.join("grande.csv"), csv).unwrap();
        let mut a = abrir_proyecto(&u, &ficha);
        let mut l = libro();
        l.nombre = "grande.csv".into();
        l.ruta = Some("grande.csv".into());
        a.mensajes = vec![l];
        a.vistas = vec![None];
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);

        let t0 = std::time::Instant::now();
        assert!(abrir_libro(&u, &mut a, 0, &textos));
        let tocar = t0.elapsed();
        assert!(
            tocar.as_millis() < 50,
            "tocar el libro tardo {tocar:?} en la ventana"
        );
        assert!(leyendo(), "la lectura sigue en su hilo");
        assert!(a.hoja.is_none(), "todavia no hay nada que abrir");
        // Tocar otra vez mientras se lee no lanza otro hilo.
        assert!(abrir_libro(&u, &mut a, 0, &textos));

        let limite = std::time::Instant::now() + std::time::Duration::from_secs(120);
        let mut peor = std::time::Duration::ZERO;
        while leyendo() {
            assert!(std::time::Instant::now() < limite, "el hilo no acabo");
            let t = std::time::Instant::now();
            let dicho = latido(&u, &mut a, &textos);
            peor = peor.max(t.elapsed());
            assert!(matches!(dicho, None | Some(None)), "{dicho:?}");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        eprintln!("libro de 40 000 celdas: tocar {tocar:?}, recibir {peor:?} en la ventana");
        assert!(
            peor.as_millis() < 50,
            "recibirlo tardo {peor:?} en la ventana"
        );
        let h = a.hoja.as_ref().expect("se abre la tabla al llegar");
        assert_eq!(a.mensajes.len(), 2);
        assert_eq!(h.indice, 1);
        let valores = h.valores.get().expect("con los valores ya calculados");
        assert_eq!(
            valores["D10001"].to_string(),
            (2..=10_001).map(|f| 2 * (f % 7)).sum::<u32>().to_string()
        );
        // Y la segunda vez abre la tabla hecha, sin leer nada.
        a.hoja = None;
        assert!(abrir_libro(&u, &mut a, 0, &textos));
        assert!(!leyendo());
        assert_eq!(a.hoja.as_ref().map(|h| h.indice), Some(1));
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
