//! **Mandar una foto al lienzo que el movil tiene abierto**, desde el plugin
//! de Flow Launcher (`p movil` + Ctrl+V, pedido `enviar_al_movil`).
//!
//! El usuario, 6-oct-2026: «en el celular la app estara en un canvas y al
//! enviar la foto se enviara directamente a ese canvas sin que la pestaña de
//! sincronizar este abierta». El cable es `pixpin_sincro::al_lienzo`; aqui
//! esta lo de este equipo: a quien se manda (los del grupo, leidos de
//! `sincro/identidad.json`), donde esta ahora (la direccion recordada y, si
//! no contesta, el anuncio mDNS con su `id`, como `al_dia`), y que se le
//! dice al usuario al acabar.
//!
//! Todo en un hilo propio: localizar y mandar puede tardar segundos, y el
//! pedido llega al bucle principal, que no puede esperar. Lo que se dice al
//! acabar va al globo de la bandeja, que solo toca el bucle principal: se
//! deja en [`tomar_avisos`] y se le da un toque (`fijar_despertador`).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};

use pixpin_proyecto::identidad::Identidad;
use pixpin_sincro::al_lienzo::{Conexion, Donde, ErrorAlLienzo, Llegada};
use pixpin_sincro::mensajes as m;
use pixpin_store::{Catalogo, Ubicacion};

use super::al_dia;

/// Un aparato del grupo al que se le puede mandar, que no es este equipo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AparatoDelGrupo {
    pub id: String,
    pub nombre: String,
    pub letra: Option<String>,
    /// Donde contesto la ultima vez (`sincro/direcciones.txt`). Que este no
    /// quiere decir que este encendido ahora: eso solo lo sabe un PING.
    pub direccion: Option<(String, u16)>,
}

/// Los del grupo menos yo, con su ultima direccion. Sin grupo, ninguno.
pub fn aparatos(ubicacion: &Ubicacion) -> Vec<AparatoDelGrupo> {
    aparatos_de(ubicacion.raiz())
}

fn aparatos_de(raiz: &Path) -> Vec<AparatoDelGrupo> {
    let Ok(id) = super::leer_identidad(raiz) else {
        return Vec::new();
    };
    if id.codigo.as_deref().is_none_or(|c| c.trim().is_empty()) {
        return Vec::new();
    }
    let direcciones = super::leer_direcciones(raiz);
    id.miembros
        .iter()
        .filter(|x| x.id != id.yo.id && !x.id.is_empty())
        .map(|x| AparatoDelGrupo {
            id: x.id.clone(),
            nombre: x.nombre.clone(),
            letra: x.letra.clone(),
            direccion: direcciones
                .iter()
                .find(|(i, _, _)| *i == x.id)
                .map(|(_, h, p)| (h.clone(), *p)),
        })
        .collect()
}

/// Lo que llego, y adonde; y cuantos se negaron (no eran fotos y tenia
/// un lienzo delante).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entregado {
    pub movil: String,
    pub donde: Vec<Llegada>,
    pub negados: usize,
}

/// Por que no se pudo. Cada uno tiene su aviso: al usuario le sirve saber
/// QUE hacer (abrir PixPin en el movil, actualizarlo), no que algo fallo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fallo {
    SinGrupo,
    NoEsDelGrupo,
    SinFicheros,
    /// Ni contesta en su direccion ni se anuncia: PixPin no esta delante en
    /// el movil (solo escucha con una pantalla a la vista) o no hay Wi-Fi.
    NoEsta(String),
    /// Un PixPin de antes del 6-oct: no conoce «suelto».
    SinSoporte(String),
    Ocupado(String),
    OtraVersion(String),
    /// Tiene un lienzo delante y nada de lo mandado era una foto.
    SoloFotos(String),
    Cortado(String, String),
}

/// Manda `ficheros` a `aparato` en un hilo y avisa al acabar. Si el movil
/// aun no sabe recibir al lienzo, abre «Enviar por Wi-Fi» con los mismos
/// ficheros: el envio de siempre, que el movil viejo si entiende.
pub fn enviar(
    idioma: pixpin_store::Idioma,
    ubicacion: Ubicacion,
    aparato: String,
    ficheros: Vec<PathBuf>,
) {
    let lanzado = std::thread::Builder::new()
        .name("al-movil".into())
        .spawn(move || {
            let raiz = ubicacion.raiz().to_path_buf();
            let hecho = mandar_ya(&raiz, &aparato, &ficheros, super::vistos_del_grupo);
            match &hecho {
                Ok(e) => tracing::info!(movil = %e.movil, donde = ?e.donde, "foto al movil"),
                Err(f) => tracing::info!(?f, "foto al movil: no se pudo"),
            }
            if matches!(hecho, Err(Fallo::SinSoporte(_))) {
                super::enviar_por_wifi(idioma, ubicacion.clone(), ficheros.clone());
            }
            avisar(aviso(&Catalogo::nuevo(idioma), &hecho));
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de mandar al movil");
    }
}

/// Lo de [`enviar`], sin hilo. `buscar` sale a la red solo si la direccion
/// recordada no contesta (en las pruebas, nunca).
fn mandar_ya(
    raiz: &Path,
    aparato: &str,
    ficheros: &[PathBuf],
    buscar: impl FnOnce(&Identidad) -> Vec<al_dia::Visto>,
) -> Result<Entregado, Fallo> {
    let fotos: Vec<&PathBuf> = ficheros.iter().filter(|f| f.is_file()).collect();
    if fotos.is_empty() {
        return Err(Fallo::SinFicheros);
    }
    let id = super::leer_identidad(raiz).map_err(|_| Fallo::SinGrupo)?;
    let codigo = id
        .codigo
        .clone()
        .filter(|c| !c.trim().is_empty())
        .ok_or(Fallo::SinGrupo)?;
    let otro = aparatos_de(raiz)
        .into_iter()
        .find(|x| x.id == aparato)
        .ok_or(Fallo::NoEsDelGrupo)?;
    tracing::info!(movil = %otro.nombre, letra = ?otro.letra, "foto al movil: buscandolo");
    let nombre = otro.nombre.clone();
    let (host, puerto) =
        localizar(raiz, &otro, || buscar(&id)).ok_or(Fallo::NoEsta(nombre.clone()))?;
    let flujo = super::conectar(&host, puerto).map_err(|_| Fallo::NoEsta(nombre.clone()))?;
    let nonce = super::nonce().ok_or_else(|| Fallo::Cortado(nombre.clone(), "sin azar".into()))?;
    let hola = m::Hola {
        yo: super::al_cable(&id.yo),
        miembros: id.miembros.iter().map(super::al_cable).collect(),
        reloj: super::ahora_ms(),
        // Para que el movil apunte donde escucha este PC, como en una vuelta.
        puerto: super::presencia::puerto().map_or(0, u32::from),
        ..Default::default()
    };
    let mut c = Conexion::abrir(flujo, &codigo, nonce, hola).map_err(|e| traducir(e, &nombre))?;
    let puerto_bueno = u16::try_from(c.puerto_del_otro)
        .ok()
        .filter(|p| *p > 0)
        .unwrap_or(puerto);
    super::apuntar_direccion(raiz, &otro.id, &host, puerto_bueno);
    let mut donde = Vec::new();
    let mut negados = 0usize;
    for f in fotos {
        let nombre_de_fichero = f
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "imagen.png".into());
        match c.mandar(f, &nombre_de_fichero) {
            Ok(l) => donde.push(l),
            // Un lienzo delante y no es una foto: ese no, los demas si.
            Err(ErrorAlLienzo::SoloFotos) => negados += 1,
            Err(e) => return Err(traducir(e, &nombre)),
        }
    }
    c.adios();
    if donde.is_empty() && negados > 0 {
        return Err(Fallo::SoloFotos(nombre));
    }
    Ok(Entregado {
        movil: nombre,
        donde,
        negados,
    })
}

/// Donde llamarle: la direccion recordada si contesta al PING; si no, la que
/// anuncie con su `id`, que se apunta para la proxima.
fn localizar(
    raiz: &Path,
    otro: &AparatoDelGrupo,
    buscar: impl FnOnce() -> Vec<al_dia::Visto>,
) -> Option<(String, u16)> {
    let id = otro.id.as_str();
    // Sin direccion recordada, la sonda falla al instante y se busca.
    let (host, puerto) = otro.direccion.clone().unwrap_or_default();
    match al_dia::donde_esta(
        Some(id),
        &otro.nombre,
        &host,
        puerto,
        super::sondear_uno,
        buscar,
    ) {
        al_dia::Donde::LaDeSiempre if !host.is_empty() => Some((host, puerto)),
        al_dia::Donde::LaDeSiempre | al_dia::Donde::NoEsta => None,
        al_dia::Donde::Nueva { host, puerto } => {
            super::apuntar_direccion(raiz, id, &host, puerto);
            Some((host, puerto))
        }
    }
}

fn traducir(e: ErrorAlLienzo, movil: &str) -> Fallo {
    let movil = movil.to_string();
    match e {
        ErrorAlLienzo::MovilSinSoporte => Fallo::SinSoporte(movil),
        ErrorAlLienzo::Ocupado => Fallo::Ocupado(movil),
        ErrorAlLienzo::OtraVersion(_) => Fallo::OtraVersion(movil),
        // El codigo no viaja: un tramo que no descifra, o un corte justo al
        // saludar, es casi siempre otro codigo de grupo.
        e if e.es_codigo_distinto() => {
            Fallo::Cortado(movil, "¿tiene el mismo código de grupo?".into())
        }
        e => Fallo::Cortado(movil, e.to_string()),
    }
}

/// El aviso del globo.
fn aviso(t: &Catalogo, hecho: &Result<Entregado, Fallo>) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    let (clave, movil) = match hecho {
        Ok(e) => {
            args.set("n", e.donde.len() as i64);
            let chat = e.donde.iter().find_map(|l| l.chat.clone());
            let clave = if e.donde.iter().all(|l| l.donde == Donde::Lienzo) {
                "al-movil-lienzo"
            } else if let (true, Some(c)) =
                (e.donde.iter().all(|l| l.donde == Donde::ChatAbierto), chat)
            {
                args.set("chat", c);
                "al-movil-chat-abierto"
            } else {
                "al-movil-chat"
            };
            let mut texto = {
                args.set("movil", e.movil.clone());
                t.t_args(clave, &args)
            };
            if e.negados > 0 {
                args.set("negados", e.negados as i64);
                texto.push('\n');
                texto.push_str(&t.t_args("al-movil-negados", &args));
            }
            return texto;
        }
        Err(Fallo::SoloFotos(n)) => ("al-movil-solo-fotos", Some(n)),
        Err(Fallo::SinGrupo) => ("al-movil-sin-grupo", None),
        Err(Fallo::NoEsDelGrupo) => ("al-movil-no-es-del-grupo", None),
        Err(Fallo::SinFicheros) => ("al-movil-sin-imagenes", None),
        Err(Fallo::NoEsta(n)) => ("al-movil-no-esta", Some(n)),
        Err(Fallo::SinSoporte(n)) => ("al-movil-actualiza", Some(n)),
        Err(Fallo::Ocupado(n)) => ("al-movil-ocupado", Some(n)),
        Err(Fallo::OtraVersion(n)) => ("al-movil-otra-version", Some(n)),
        Err(Fallo::Cortado(n, motivo)) => {
            args.set("motivo", motivo.clone());
            ("al-movil-cortado", Some(n))
        }
    };
    if let Some(n) = movil {
        args.set("movil", n.clone());
    }
    t.t_args(clave, &args)
}

// ------------------------------------------------------------ los avisos

/// Lo que espera a que el bucle principal lo saque en el globo.
static AVISOS: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// La ventana principal, para darle el toque. 0: aun no se sabe.
static DESPERTADOR: AtomicIsize = AtomicIsize::new(0);

/// La ventana del bucle principal. Se pone una vez, al arrancar.
pub fn fijar_despertador(hwnd: isize) {
    DESPERTADOR.store(hwnd, Ordering::SeqCst);
}

/// Los avisos pendientes, ya fuera de la cola.
pub fn tomar_avisos() -> Vec<String> {
    AVISOS
        .lock()
        .map(|mut v| std::mem::take(&mut *v))
        .unwrap_or_default()
}

pub(super) fn avisar(texto: String) {
    if let Ok(mut v) = AVISOS.lock() {
        v.push(texto);
    }
    let h = DESPERTADOR.load(Ordering::SeqCst);
    if h != 0 {
        pixpin_shell::despertar(windows::Win32::Foundation::HWND(h as *mut _));
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::identidad::Aparato;
    use pixpin_sincro::canal::{Canal, Tipo};
    use std::io::{Read, Write};
    use std::net::TcpListener;

    const CODIGO: &str = "ABCD2345";

    fn raiz(nombre: &str) -> PathBuf {
        let r =
            std::env::temp_dir().join(format!("pixpin-al-movil-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    fn aparato(id: &str, nombre: &str, letra: &str) -> Aparato {
        Aparato {
            id: id.into(),
            nombre: nombre.into(),
            letra: Some(letra.into()),
            ..Default::default()
        }
    }

    /// Este PC (`pc`, A) en un grupo con un telefono (`tel`, B).
    fn en_grupo(r: &Path, codigo: Option<&str>) {
        let yo = aparato("pc", "Portatil", "A");
        Identidad {
            yo: yo.clone(),
            codigo: codigo.map(Into::into),
            miembros: vec![yo, aparato("tel", "Telefono", "B")],
            ..Default::default()
        }
        .guardar(r)
        .unwrap();
    }

    #[test]
    fn los_aparatos_del_grupo_son_los_demas_con_su_ultima_direccion() {
        let r = raiz("aparatos");
        en_grupo(&r, Some(CODIGO));
        super::super::apuntar_direccion(&r, "tel", "192.168.1.20", 47474);
        assert_eq!(
            aparatos_de(&r),
            [AparatoDelGrupo {
                id: "tel".into(),
                nombre: "Telefono".into(),
                letra: Some("B".into()),
                direccion: Some(("192.168.1.20".into(), 47474)),
            }]
        );
    }

    #[test]
    fn caso_negativo_sin_codigo_de_grupo_no_hay_a_quien_mandar() {
        let r = raiz("sin-grupo");
        en_grupo(&r, None);
        assert!(aparatos_de(&r).is_empty());
        let foto = r.join("f.png");
        std::fs::write(&foto, b"png").unwrap();
        assert_eq!(
            mandar_ya(&r, "tel", &[foto], |_| panic!("sin grupo no se busca")),
            Err(Fallo::SinGrupo)
        );
    }

    #[test]
    fn caso_negativo_a_quien_no_es_del_grupo_o_sin_imagenes_no_se_intenta() {
        let r = raiz("no-es");
        en_grupo(&r, Some(CODIGO));
        let foto = r.join("f.png");
        std::fs::write(&foto, b"png").unwrap();
        let nunca = |_: &Identidad| -> Vec<al_dia::Visto> { panic!("no se sale a la red") };
        assert_eq!(
            mandar_ya(&r, "otro", std::slice::from_ref(&foto), nunca),
            Err(Fallo::NoEsDelGrupo)
        );
        // Ni a uno mismo.
        assert_eq!(
            mandar_ya(&r, "pc", std::slice::from_ref(&foto), nunca),
            Err(Fallo::NoEsDelGrupo)
        );
        assert_eq!(
            mandar_ya(&r, "tel", &[r.join("no-esta.png")], nunca),
            Err(Fallo::SinFicheros)
        );
    }

    #[test]
    fn caso_negativo_si_no_contesta_ni_se_anuncia_es_que_no_esta_abierto() {
        let r = raiz("no-esta");
        en_grupo(&r, Some(CODIGO));
        // Un puerto que nadie escucha.
        let libre = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let puerto = libre.local_addr().unwrap().port();
        drop(libre);
        super::super::apuntar_direccion(&r, "tel", "127.0.0.1", puerto);
        let foto = r.join("f.png");
        std::fs::write(&foto, b"png").unwrap();
        assert_eq!(
            mandar_ya(&r, "tel", &[foto], |_| Vec::new()),
            Err(Fallo::NoEsta("Telefono".into()))
        );
    }

    /// El que recibe en el movil de mentira: con lienzo delante, solo fotos;
    /// si no, el chat «Tesis» abierto.
    struct Falso<'a> {
        lienzo: bool,
        fotos: &'a mut Vec<(String, Vec<u8>)>,
    }

    impl pixpin_sincro::al_lienzo::Recibe for Falso<'_> {
        fn aceptar(&mut self, p: &pixpin_sincro::al_lienzo::Suelto) -> Result<(), String> {
            if self.lienzo && !p.es_imagen() {
                return Err(format!("Telefono {}", pixpin_sincro::al_lienzo::SOLO_FOTOS));
            }
            Ok(())
        }
        fn guardar(
            &mut self,
            p: &pixpin_sincro::al_lienzo::Suelto,
            fichero: &Path,
        ) -> Result<Llegada, String> {
            self.fotos
                .push((p.nombre.clone(), std::fs::read(fichero).unwrap()));
            Ok(if self.lienzo {
                Llegada::en(Donde::Lienzo)
            } else {
                Llegada {
                    donde: Donde::ChatAbierto,
                    chat: Some("Tesis".into()),
                }
            })
        }
    }

    /// Lo que le llego al movil de mentira: nombre y bytes.
    type Recibidas = Vec<(String, Vec<u8>)>;

    /// Un movil en 127.0.0.1: contesta a la sonda y atiende una conexion
    /// con `suelto` (lo nuevo de la guia) o sin el (un PixPin de antes).
    fn movil(sabe: bool, lienzo: bool) -> (u16, std::thread::JoinHandle<Recibidas>) {
        let escucha = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let puerto = escucha.local_addr().unwrap().port();
        let hilo = std::thread::spawn(move || {
            let mut fotos = Vec::new();
            loop {
                let (mut flujo, _) = escucha.accept().unwrap();
                let mut cuatro = [0u8; 4];
                flujo.peek(&mut cuatro).unwrap();
                if &cuatro == pixpin_sincro::SONDA {
                    flujo.read_exact(&mut cuatro).unwrap();
                    flujo.write_all(pixpin_sincro::SONDA_RESPUESTA).unwrap();
                    continue;
                }
                let clave = pixpin_sincro::codigo::clave_de_grupo(CODIGO);
                let mut c = Canal::saludar(flujo, &clave, [7; 32], false).unwrap();
                let leer = |c: &mut Canal<_>| -> serde_json::Value {
                    let (tipo, datos) = c.recibir().unwrap();
                    assert_eq!(tipo, Tipo::Json, "un movil viejo cortaria aqui");
                    serde_json::from_slice(&datos).unwrap()
                };
                let mandar = |c: &mut Canal<_>, v: serde_json::Value| {
                    c.mandar(Tipo::Json, v.to_string().as_bytes()).unwrap()
                };
                let hola = leer(&mut c);
                assert_eq!(hola["hola"]["yo"]["id"], "pc");
                mandar(
                    &mut c,
                    serde_json::json!({ "hola": {
                        "yo": { "id": "tel", "nombre": "Telefono", "letra": "B" },
                        "version": pixpin_sincro::VERSION, "puerto": puerto,
                    }}),
                );
                loop {
                    let v = leer(&mut c);
                    match v["t"].as_str().unwrap_or_default() {
                        "adios" => {
                            mandar(&mut c, serde_json::json!({}));
                            return fotos;
                        }
                        "suelto" if sabe => {
                            let p: pixpin_sincro::al_lienzo::Suelto =
                                serde_json::from_value(v).unwrap();
                            let mut quien = Falso {
                                lienzo,
                                fotos: &mut fotos,
                            };
                            let carpeta = std::env::temp_dir()
                                .join(format!("pixpin-al-movil-rec-{}", std::process::id()));
                            pixpin_sincro::al_lienzo::responder(&mut c, &p, &carpeta, &mut quien)
                                .unwrap();
                        }
                        t => mandar(
                            &mut c,
                            serde_json::json!({ "error": format!("No sé qué es «{t}»") }),
                        ),
                    }
                }
            }
        });
        (puerto, hilo)
    }

    #[test]
    fn con_la_direccion_recordada_las_fotos_llegan_sin_buscar_por_la_red() {
        let r = raiz("llegan");
        en_grupo(&r, Some(CODIGO));
        let (puerto, hilo) = movil(true, true);
        super::super::apuntar_direccion(&r, "tel", "127.0.0.1", puerto);
        let a = r.join("a.png");
        let b = r.join("b.jpg");
        std::fs::write(&a, b"png a").unwrap();
        std::fs::write(&b, b"jpg b").unwrap();
        let hecho = mandar_ya(&r, "tel", &[a, b], |_| panic!("contesto: no se busca"));
        assert_eq!(
            hecho,
            Ok(Entregado {
                movil: "Telefono".into(),
                donde: vec![Llegada::en(Donde::Lienzo), Llegada::en(Donde::Lienzo)],
                negados: 0,
            })
        );
        let fotos = hilo.join().unwrap();
        assert_eq!(
            fotos,
            [
                ("a.png".to_string(), b"png a".to_vec()),
                ("b.jpg".to_string(), b"jpg b".to_vec())
            ]
        );
        let t = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        assert_eq!(
            aviso(&t, &hecho),
            "2 imágenes enviadas al lienzo de Telefono"
        );
    }

    #[test]
    fn caso_negativo_un_movil_viejo_pide_actualizar_y_se_despide_bien() {
        let r = raiz("viejo");
        en_grupo(&r, Some(CODIGO));
        let (puerto, hilo) = movil(false, false);
        super::super::apuntar_direccion(&r, "tel", "127.0.0.1", puerto);
        let a = r.join("a.png");
        std::fs::write(&a, b"png a").unwrap();
        let hecho = mandar_ya(&r, "tel", &[a], |_| Vec::new());
        assert_eq!(hecho, Err(Fallo::SinSoporte("Telefono".into())));
        // El movil viejo no recibio ningun trozo y oyo el «adios».
        assert!(hilo.join().unwrap().is_empty());
        let t = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        assert!(aviso(&t, &hecho).starts_with("Actualiza PixPin en Telefono"));
    }

    #[test]
    fn los_avisos_dicen_donde_quedo_y_que_hacer() {
        let t = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let una = Ok(Entregado {
            movil: "Pixel".into(),
            donde: vec![Llegada::en(Donde::Lienzo)],
            negados: 0,
        });
        assert_eq!(aviso(&t, &una), "Enviada al lienzo de Pixel");
        let al_chat = Ok(Entregado {
            movil: "Pixel".into(),
            donde: vec![Llegada::en(Donde::Chat)],
            negados: 0,
        });
        assert_eq!(
            aviso(&t, &al_chat),
            "Pixel lo guardó en la conversación general (no tenía un chat ni un lienzo abierto)"
        );
        assert_eq!(
            aviso(&t, &Err(Fallo::NoEsta("Pixel".into()))),
            "Pixel no está abierto: abre PixPin en el móvil"
        );
        // Caso negativo: el globo nunca ensena una clave sin traducir.
        for f in [
            Fallo::SinGrupo,
            Fallo::NoEsDelGrupo,
            Fallo::SinFicheros,
            Fallo::Ocupado("P".into()),
            Fallo::OtraVersion("P".into()),
            Fallo::Cortado("P".into(), "x".into()),
            Fallo::SoloFotos("P".into()),
        ] {
            assert!(!aviso(&t, &Err(f.clone())).starts_with("al-movil"), "{f:?}");
        }
    }

    #[test]
    fn con_un_chat_abierto_llega_cualquier_archivo_y_se_dice_en_cual() {
        let r = raiz("chat-abierto");
        en_grupo(&r, Some(CODIGO));
        let (puerto, hilo) = movil(true, false);
        super::super::apuntar_direccion(&r, "tel", "127.0.0.1", puerto);
        let pdf = r.join("informe.pdf");
        std::fs::write(&pdf, b"%PDF").unwrap();
        let hecho = mandar_ya(&r, "tel", &[pdf], |_| Vec::new());
        assert_eq!(
            hecho,
            Ok(Entregado {
                movil: "Telefono".into(),
                donde: vec![Llegada {
                    donde: Donde::ChatAbierto,
                    chat: Some("Tesis".into()),
                }],
                negados: 0,
            })
        );
        assert_eq!(hilo.join().unwrap()[0].0, "informe.pdf");
        let t = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        assert_eq!(aviso(&t, &hecho), "Enviado al chat «Tesis» de Telefono");
    }

    #[test]
    fn caso_negativo_un_lienzo_abierto_niega_lo_que_no_es_foto() {
        let r = raiz("solo-fotos");
        en_grupo(&r, Some(CODIGO));
        let (puerto, hilo) = movil(true, true);
        super::super::apuntar_direccion(&r, "tel", "127.0.0.1", puerto);
        let pdf = r.join("informe.pdf");
        let png = r.join("foto.png");
        std::fs::write(&pdf, b"%PDF").unwrap();
        std::fs::write(&png, b"png").unwrap();
        // Con una foto al lado: la foto entra y del PDF se avisa.
        let hecho = mandar_ya(&r, "tel", &[pdf.clone(), png], |_| Vec::new());
        assert_eq!(
            hecho,
            Ok(Entregado {
                movil: "Telefono".into(),
                donde: vec![Llegada::en(Donde::Lienzo)],
                negados: 1,
            })
        );
        let t = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        assert_eq!(
            aviso(&t, &hecho),
            "Enviada al lienzo de Telefono\n1 archivo no se envió: no era una foto y tiene un lienzo abierto"
        );
        let fotos = hilo.join().unwrap();
        assert_eq!(fotos.len(), 1, "el PDF no viajo");
        // Solo el PDF: se niega entero.
        let (puerto, hilo) = movil(true, true);
        super::super::apuntar_direccion(&r, "tel", "127.0.0.1", puerto);
        let hecho = mandar_ya(&r, "tel", &[pdf], |_| Vec::new());
        assert_eq!(hecho, Err(Fallo::SoloFotos("Telefono".into())));
        assert!(hilo.join().unwrap().is_empty());
        assert!(
            aviso(&t, &hecho).starts_with("Telefono tiene un lienzo abierto: solo acepta fotos")
        );
    }

    #[test]
    fn los_avisos_esperan_al_bucle_principal_y_salen_una_vez() {
        let _ = tomar_avisos();
        avisar("uno".into());
        avisar("dos".into());
        assert_eq!(tomar_avisos(), ["uno", "dos"]);
        assert!(tomar_avisos().is_empty());
    }
}
