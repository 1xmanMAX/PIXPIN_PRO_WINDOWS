//! **La letra de una cancion: leerla, guardarla y saber por que linea va**
//! (27-oct-2026). Es `guardados/LetraDeCancion.kt` del movil, clavado, con
//! sus pruebas (`LetraDeCancionTest`).
//!
//! ## Donde se guarda, y por que ahi
//!
//! Una cancion es una nota de voz (`clase: "VOZ"`) con
//! `estadoDelTexto: "letra"` y la letra en `transcripcion`: **no hay ningun
//! campo nuevo**. La letra sincronizada va con el MISMO formato que una
//! transcripcion con tiempos ([`crate::con_tiempos`]): una linea por
//! parrafo, `[m:ss] lo que se canta`, y un renglon en blanco entre ellas.
//! Asi la pantalla de la letra del PC y la del movil resaltan la linea que
//! suena y saltan a ella sin cambiar una coma.
//!
//! Lo que no se hace es guardar el `.lrc` tal cual: `[00:12.34]` no es una
//! marca para [`crate::tiempo_de`] (exige dos cifras justas en los
//! segundos) y un LRC no lleva renglones en blanco, asi que saldria todo de
//! un bloque con los corchetes a la vista. Lo que se pierde es la centesima:
//! se redondea al segundo, que para seguir una cancion cantando sobra.
//!
//! ## Lo que se acepta al pegar
//!
//! - Texto llano: tal cual, linea a linea.
//! - **LRC**: `[mm:ss.xx]`, `[mm:ss]`, varias marcas en una linea
//!   (estribillos), las etiquetas `[ti:]`, `[ar:]`, `[al:]`, `[by:]`,
//!   `[offset:]` (se aplica) y las marcas por palabra `<mm:ss.xx>` del LRC
//!   «mejorado» (se quitan).
//! - Lo ya guardado (`[m:ss]` con renglones en blanco, o `[h:mm:ss]`), que
//!   vuelve igual.
//!
//! Todo puro: sin ventanas y sin sonido.

/// Una linea: en que milisegundo empieza (`None` si no lo dice) y que se
/// canta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linea {
    pub ms: Option<i64>,
    pub texto: String,
}

impl Linea {
    pub fn new(ms: Option<i64>, texto: impl Into<String>) -> Linea {
        Linea {
            ms,
            texto: texto.into(),
        }
    }
}

/// Una letra leida: sus lineas y lo que dijera de si misma.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Letra {
    pub lineas: Vec<Linea>,
    pub titulo: Option<String>,
    pub artista: Option<String>,
}

impl Letra {
    /// Si lleva tiempos: entonces se resalta la linea que suena.
    pub fn con_tiempos(&self) -> bool {
        self.lineas.iter().any(|l| l.ms.is_some())
    }
}

/// Una etiqueta del LRC que no es letra (`[ti:Titulo]`, `[offset:+250]`):
/// `(clave en minusculas, valor)`. `^\[([A-Za-z#]+):(.*)]$` del movil.
fn etiqueta(linea: &str) -> Option<(String, &str)> {
    let dentro = linea.strip_prefix('[')?.strip_suffix(']')?;
    let (clave, valor) = dentro.split_once(':')?;
    if clave.is_empty() || !clave.chars().all(|c| c.is_ascii_alphabetic() || c == '#') {
        return None;
    }
    Some((clave.to_ascii_lowercase(), valor.trim()))
}

/// Las etiquetas que solo trae un LRC: con una de estas el texto es LRC
/// aunque no lleve ningun tiempo raro.
const ETIQUETAS_LRC: &[&str] = &[
    "ti", "ar", "al", "au", "by", "offset", "length", "re", "ve", "tool",
];

/// Quita las marcas por palabra del LRC mejorado: `<00:12.34>`
/// (`<\d{1,3}:\d{1,2}(?:[.:]\d{1,3})?>`).
fn sin_marcas_por_palabra(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(i) = resto.find('<') {
        salida.push_str(&resto[..i]);
        let tras = &resto[i + 1..];
        match tras.find('>') {
            Some(j) if es_marca_por_palabra(&tras[..j]) => resto = &tras[j + 1..],
            _ => {
                salida.push('<');
                resto = tras;
            }
        }
    }
    salida.push_str(resto);
    salida
}

fn es_marca_por_palabra(dentro: &str) -> bool {
    let cifras = |t: &str, max: usize| {
        !t.is_empty() && t.len() <= max && t.bytes().all(|b| b.is_ascii_digit())
    };
    let Some((m, resto)) = dentro.split_once(':') else {
        return false;
    };
    if !cifras(m, 3) {
        return false;
    }
    match resto.find(['.', ':']) {
        Some(k) => cifras(&resto[..k], 2) && cifras(&resto[k + 1..], 3),
        None => cifras(resto, 2),
    }
}

/// **Lee una letra pegada**: LRC, la de PixPin con tiempos o texto llano.
/// Las lineas con tiempo salen ordenadas por tiempo (una linea con dos
/// marcas sale dos veces: el estribillo se canta dos veces).
pub fn leer(texto: &str) -> Letra {
    let texto = texto.replace("\r\n", "\n").replace('\r', "\n");
    let lineas: Vec<&str> = texto.split('\n').collect();
    // Sin ningun tiempo es texto llano y se respeta tal cual.
    let hay_tiempos = lineas
        .iter()
        .any(|l| !marcas_al_principio(l.trim()).0.is_empty());
    if !hay_tiempos {
        let llanas: Vec<&str> = lineas.iter().map(|l| l.trim_end()).collect();
        let desde = llanas
            .iter()
            .position(|l| !l.is_empty())
            .unwrap_or(llanas.len());
        let hasta = llanas
            .iter()
            .rposition(|l| !l.is_empty())
            .map_or(desde, |i| i + 1);
        return Letra {
            lineas: llanas[desde..hasta]
                .iter()
                .map(|l| Linea::new(None, *l))
                .collect(),
            ..Letra::default()
        };
    }
    let (mut titulo, mut artista, mut desfase) = (None, None, 0i64);
    let mut salida: Vec<Linea> = Vec::new();
    let mut anterior_en_blanco = true;
    for cruda in lineas {
        let linea = cruda.trim();
        if linea.is_empty() {
            anterior_en_blanco = true;
            continue;
        }
        if let Some((clave, valor)) = etiqueta(linea) {
            let valor = (!valor.is_empty()).then(|| valor.to_string());
            match clave.as_str() {
                "ti" => titulo = valor,
                "ar" => artista = valor,
                "offset" => {
                    desfase = valor
                        .as_deref()
                        .map(|v| v.strip_prefix('+').unwrap_or(v))
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0)
                }
                _ => {}
            }
            anterior_en_blanco = true;
            continue;
        }
        let (marcas, resto) = marcas_al_principio(linea);
        let letra = sin_marcas_por_palabra(resto).trim().to_string();
        if marcas.is_empty() {
            // Sin marca: si sigue pegada a una linea (un parrafo de PixPin
            // partido en dos renglones), es suya; si no, va suelta.
            match salida.last_mut() {
                Some(ultima) if !anterior_en_blanco => {
                    if !ultima.texto.is_empty() {
                        ultima.texto.push('\n');
                    }
                    ultima.texto.push_str(&letra);
                }
                _ => salida.push(Linea::new(None, letra)),
            }
        } else {
            // Un `offset` positivo adelanta la letra (LRC): sale antes.
            for ms in marcas {
                salida.push(Linea::new(Some((ms - desfase).max(0)), letra.clone()));
            }
        }
        anterior_en_blanco = false;
    }
    Letra {
        lineas: ordenar(salida),
        titulo,
        artista,
    }
}

/// Por tiempo, sin barajar: una linea sin tiempo se queda detras de la que
/// tenia delante, y las de igual tiempo siguen en su orden (`sort_by_key`
/// es estable).
fn ordenar(lineas: Vec<Linea>) -> Vec<Linea> {
    let mut ultimo = -1;
    let mut con_clave: Vec<(i64, Linea)> = lineas
        .into_iter()
        .map(|l| {
            if let Some(ms) = l.ms {
                ultimo = ms;
            }
            (ultimo, l)
        })
        .collect();
    con_clave.sort_by_key(|(k, _)| *k);
    con_clave.into_iter().map(|(_, l)| l).collect()
}

/// Las marcas de tiempo con las que empieza `linea` (en ms) y lo que queda
/// detras. Vale `[m:ss]`, `[mm:ss.x]`, `[mm:ss.xx]`, `[mm:ss.xxx]` y
/// `[h:mm:ss]`; lo que no, no es marca.
pub fn marcas_al_principio(linea: &str) -> (Vec<i64>, &str) {
    let mut marcas = Vec::new();
    let mut resto = linea;
    while let Some(tras) = resto.strip_prefix('[') {
        let Some(cierre) = tras.find(']') else {
            break;
        };
        let Some(ms) = tiempo_lrc(&tras[..cierre]) else {
            break;
        };
        marcas.push(ms);
        resto = tras[cierre + 1..].trim_start();
    }
    (marcas, resto)
}

/// `1:23`, `01:23.45`, `1:02:03` → milisegundos; `None` si no es un tiempo.
pub fn tiempo_lrc(dentro: &str) -> Option<i64> {
    let partes: Vec<&str> = dentro.split(':').collect();
    if !(2..=3).contains(&partes.len()) || partes.iter().any(|p| p.is_empty()) {
        return None;
    }
    let ultimo = partes[partes.len() - 1];
    let (segundos, fraccion) = ultimo.split_once('.').unwrap_or((ultimo, ""));
    let cifras = |t: &str| t.bytes().all(|b| b.is_ascii_digit());
    if segundos.len() != 2 || !cifras(segundos) {
        return None;
    }
    if fraccion.len() > 3 || !cifras(fraccion) {
        return None;
    }
    if !partes[..partes.len() - 1]
        .iter()
        .all(|p| cifras(p) && p.len() <= 3)
    {
        return None;
    }
    let s: i64 = segundos.parse().ok()?;
    if s > 59 {
        return None;
    }
    let (h, m): (i64, i64) = if partes.len() == 3 {
        if partes[1].len() != 2 {
            return None;
        }
        let m: i64 = partes[1].parse().ok()?;
        if m > 59 {
            return None;
        }
        (partes[0].parse().ok()?, m)
    } else {
        (0, partes[0].parse().ok()?)
    };
    let ms: i64 = if fraccion.is_empty() {
        0
    } else {
        format!("{fraccion:0<3}").parse().ok()?
    };
    Some((h * 3600 + m * 60 + s) * 1000 + ms)
}

/// **Lo que se guarda en `transcripcion`**: con tiempos, `[m:ss] linea` y un
/// renglon en blanco entre lineas (lo que leen el PC y el movil); sin
/// tiempos, el texto tal cual, linea a linea. Una linea con tiempo y sin
/// texto (un silencio del LRC) se guarda solo con su marca: asi la que
/// sonaba deja de resaltarse.
pub fn a_guardar(letra: &Letra) -> String {
    if !letra.con_tiempos() {
        return letra
            .lineas
            .iter()
            .map(|l| l.texto.as_str())
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();
    }
    letra
        .lineas
        .iter()
        .filter_map(|l| {
            // Un parrafo no puede llevar renglones en blanco dentro: partiria
            // la linea en dos.
            let t = l
                .texto
                .lines()
                .filter(|r| !r.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            match l.ms {
                Some(ms) if t.is_empty() => Some(format!("[{}]", marca(ms))),
                Some(ms) => Some(format!("[{}] {t}", marca(ms))),
                None if t.is_empty() => None,
                None => Some(t),
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// El minuto como lo escriben el telefono y el PC (`1:23`, `1:02:09`),
/// **redondeado al segundo mas cercano**, no truncado: asi la linea se
/// resalta como mucho medio segundo antes o despues.
pub fn marca(ms: i64) -> String {
    crate::marca_de_tiempo((ms.max(0) + 500) / 1000 * 1000)
}

/// **La letra en LRC** (`[mm:ss.xx]linea`), para llevarla a otra aplicacion
/// de karaoke.
pub fn a_lrc(letra: &Letra, titulo: Option<&str>, artista: Option<&str>) -> String {
    let mut s = String::new();
    let titulo = titulo.or(letra.titulo.as_deref());
    let artista = artista.or(letra.artista.as_deref());
    if let Some(t) = titulo.filter(|t| !t.trim().is_empty()) {
        s.push_str(&format!("[ti:{t}]\n"));
    }
    if let Some(a) = artista.filter(|a| !a.trim().is_empty()) {
        s.push_str(&format!("[ar:{a}]\n"));
    }
    for l in &letra.lineas {
        let t = l
            .texto
            .lines()
            .filter(|r| !r.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" / ");
        if let Some(ms) = l.ms {
            let cs = ms.max(0) / 10;
            s.push_str(&format!(
                "[{:02}:{:02}.{:02}]{t}\n",
                cs / 6000,
                (cs / 100) % 60,
                cs % 100
            ));
        } else if !t.is_empty() {
            s.push_str(&t);
            s.push('\n');
        }
    }
    s.trim_end().to_string()
}

/// **La linea que suena en `ms`**: la ultima cuyo tiempo ya paso. `None`
/// antes de la primera o si no hay tiempos (resaltar una al azar seria
/// mentir). La misma regla que `voz.rs::por_donde_va`.
pub fn linea_actual(lineas: &[Linea], ms: i64) -> Option<usize> {
    lineas.iter().rposition(|l| l.ms.is_some_and(|t| t <= ms))
}

/// Si un texto parece una letra: mas de tres renglones con algo, o con
/// tiempos.
pub fn parece_letra(texto: Option<&str>) -> bool {
    let Some(texto) = texto.filter(|t| !t.trim().is_empty()) else {
        return false;
    };
    if leer(texto).con_tiempos() {
        return true;
    }
    texto.lines().filter(|l| !l.trim().is_empty()).count() >= 4
}

/// **Si un texto es un LRC** y no una letra o transcripcion de PixPin: trae
/// una etiqueta del LRC, una marca que [`crate::tiempo_de`] no reconoce
/// (`[00:12.34]`), varias marcas seguidas, marcas por palabra, o dos lineas
/// con tiempo sin el renglon en blanco que las separa en PixPin.
///
/// Es lo que decide si el lapiz de la letra convierte lo pegado: lo de
/// PixPin se deja tal cual, byte a byte.
pub fn es_lrc(texto: &str) -> bool {
    let texto = texto.replace("\r\n", "\n");
    let mut en_parrafo_con_marca = false;
    for linea in texto.split('\n') {
        let t = linea.trim();
        if t.is_empty() {
            en_parrafo_con_marca = false;
            continue;
        }
        if etiqueta(t).is_some_and(|(clave, _)| ETIQUETAS_LRC.contains(&clave.as_str())) {
            return true;
        }
        let (marcas, resto) = marcas_al_principio(t);
        match marcas.len() {
            0 => {}
            1 => {
                let de_pixpin = crate::tiempo_de(t).is_some_and(|(ms, _)| ms == marcas[0]);
                if en_parrafo_con_marca || !de_pixpin || sin_marcas_por_palabra(resto) != resto {
                    return true;
                }
                en_parrafo_con_marca = true;
            }
            _ => return true,
        }
    }
    false
}

/// Lo que hay que guardar de un texto pegado: el LRC convertido al formato
/// de PixPin, o `None` si no era un LRC (y se guarda tal cual).
pub fn convertir_si_es_lrc(texto: &str) -> Option<String> {
    es_lrc(texto).then(|| a_guardar(&leer(texto)))
}

/// **Sincronizar una letra cantandola**: con la cancion sonando, un toque al
/// empezar cada linea le pone su tiempo. Puro: la pantalla le da el
/// milisegundo del reproductor ([`Sincronizador::marcar`]) y al acabar saca
/// la letra con tiempos ([`Sincronizador::letra`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sincronizador {
    textos: Vec<String>,
    tiempos: Vec<Option<i64>>,
    siguiente: usize,
}

impl Sincronizador {
    /// `textos`: las lineas de la letra; las vacias se quitan (no se canta
    /// un renglon en blanco).
    pub fn new<S: AsRef<str>>(textos: &[S]) -> Sincronizador {
        let textos: Vec<String> = textos
            .iter()
            .map(|t| t.as_ref().trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        Sincronizador {
            tiempos: vec![None; textos.len()],
            textos,
            siguiente: 0,
        }
    }

    /// Las lineas para sincronizar de una letra ya guardada (con o sin
    /// tiempos): cada renglon, una linea.
    pub fn de(letra: &Letra) -> Sincronizador {
        let renglones: Vec<&str> = letra.lineas.iter().flat_map(|l| l.texto.lines()).collect();
        Sincronizador::new(&renglones)
    }

    pub fn textos(&self) -> &[String] {
        &self.textos
    }

    /// La linea que toca marcar ahora. Igual al numero de lineas: ya estan
    /// todas.
    pub fn siguiente(&self) -> usize {
        self.siguiente
    }

    pub fn terminado(&self) -> bool {
        self.siguiente >= self.textos.len()
    }

    /// Pone `ms` a la linea que toca y pasa a la siguiente. Nunca antes que
    /// la anterior (un toque despues de saltar atras no puede desordenar la
    /// letra). Devuelve si quedan lineas.
    pub fn marcar(&mut self, ms: i64) -> bool {
        if self.terminado() {
            return false;
        }
        let anterior = self.anterior();
        self.tiempos[self.siguiente] = Some(ms.max(0).max(anterior));
        self.siguiente += 1;
        !self.terminado()
    }

    fn anterior(&self) -> i64 {
        self.siguiente
            .checked_sub(1)
            .and_then(|i| self.tiempos[i])
            .unwrap_or(0)
    }

    /// Quita la ultima marca. Devuelve a que milisegundo conviene volver
    /// para repetirla (dos segundos antes de la anterior, para cogerla con
    /// tiempo), o `None` si no habia ninguna.
    pub fn deshacer(&mut self) -> Option<i64> {
        if self.siguiente == 0 {
            return None;
        }
        self.siguiente -= 1;
        self.tiempos[self.siguiente] = None;
        Some((self.anterior() - 2000).max(0))
    }

    /// El tiempo puesto a la linea `i`.
    pub fn tiempo(&self, i: usize) -> Option<i64> {
        self.tiempos.get(i).copied().flatten()
    }

    /// La letra con lo marcado: las que no se llegaron a marcar van detras,
    /// sin tiempo.
    pub fn letra(&self) -> Letra {
        Letra {
            lineas: self
                .textos
                .iter()
                .zip(&self.tiempos)
                .map(|(t, ms)| Linea::new(*ms, t.clone()))
                .collect(),
            ..Letra::default()
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn l(ms: Option<i64>, t: &str) -> Linea {
        Linea::new(ms, t)
    }

    #[test]
    fn un_lrc_se_lee_con_sus_tiempos_titulo_y_artista() {
        let letra = leer(
            "[ti:Bésame mucho]\n[ar:Consuelo Velázquez]\n[by:alguien]\n\
             [00:12.30]Bésame, bésame mucho\n[00:17.80]como si fuera esta noche la última vez",
        );
        assert_eq!(letra.titulo.as_deref(), Some("Bésame mucho"));
        assert_eq!(letra.artista.as_deref(), Some("Consuelo Velázquez"));
        assert!(letra.con_tiempos());
        assert_eq!(
            letra.lineas,
            vec![
                l(Some(12_300), "Bésame, bésame mucho"),
                l(Some(17_800), "como si fuera esta noche la última vez"),
            ]
        );
    }

    #[test]
    fn varias_marcas_en_una_linea_salen_en_su_orden_y_el_offset_se_aplica() {
        let letra = leer("[offset:+500]\n[00:10.00][00:30.00]Estribillo\n[00:20.00]Estrofa");
        assert_eq!(
            letra.lineas,
            vec![
                l(Some(9_500), "Estribillo"),
                l(Some(19_500), "Estrofa"),
                l(Some(29_500), "Estribillo"),
            ]
        );
    }

    #[test]
    fn las_marcas_por_palabra_del_lrc_mejorado_se_quitan() {
        let letra = leer("[00:01.00]<00:01.00>Hola <00:01.50>mundo <no es marca>");
        assert_eq!(
            letra.lineas,
            vec![l(Some(1_000), "Hola mundo <no es marca>")]
        );
    }

    #[test]
    fn los_tiempos_validos_y_los_que_no() {
        assert_eq!(tiempo_lrc("1:23"), Some(83_000));
        assert_eq!(tiempo_lrc("01:23.45"), Some(83_450));
        assert_eq!(tiempo_lrc("01:23.4"), Some(83_400));
        assert_eq!(tiempo_lrc("01:23.456"), Some(83_456));
        assert_eq!(tiempo_lrc("1:02:03"), Some(3_723_000));
        assert_eq!(tiempo_lrc("1:5"), None);
        assert_eq!(tiempo_lrc("1:75"), None);
        assert_eq!(tiempo_lrc("Coro"), None);
        assert_eq!(tiempo_lrc("ti:Algo"), None);
    }

    #[test]
    fn el_texto_llano_se_respeta_tal_cual() {
        let texto = "[Coro]\nPrimera línea\n\nSegunda estrofa";
        let letra = leer(texto);
        assert!(!letra.con_tiempos());
        assert_eq!(a_guardar(&letra), texto);
        assert!(!es_lrc(texto));
    }

    #[test]
    fn se_guarda_en_el_formato_que_leen_el_pc_y_el_movil() {
        let guardado = a_guardar(&leer(
            "[00:12.30]Uno\n[00:17.80]Dos\n[01:02.00]\n[61:00.00]Tres",
        ));
        // `[m:ss] texto`, renglon en blanco entre lineas, redondeado al
        // segundo; un silencio, solo su marca.
        assert_eq!(
            guardado,
            "[0:12] Uno\n\n[0:18] Dos\n\n[1:02]\n\n[1:01:00] Tres"
        );
        // Y lo lee de vuelta el mismo reconocedor de minutos de la letra.
        let bloques: Vec<&str> = guardado.split("\n\n").collect();
        assert_eq!(crate::tiempo_de(bloques[0]), Some((12_000, "Uno")));
        assert_eq!(crate::tiempo_de(bloques[3]), Some((3_660_000, "Tres")));
    }

    #[test]
    fn lo_guardado_vuelve_igual_al_leerlo_otra_vez() {
        let guardado = "[0:12] Uno\nsigue uno\n\n[0:18] Dos\n\nNota suelta";
        let letra = leer(guardado);
        assert_eq!(
            letra.lineas,
            vec![
                l(Some(12_000), "Uno\nsigue uno"),
                l(Some(18_000), "Dos"),
                l(None, "Nota suelta"),
            ]
        );
        assert_eq!(a_guardar(&letra), guardado);
        assert!(!es_lrc(guardado), "lo de PixPin no se toca");
    }

    #[test]
    fn la_linea_que_suena_es_la_ultima_cuyo_tiempo_ya_paso() {
        let lineas = vec![
            l(Some(1_000), "a"),
            l(Some(5_000), "b"),
            l(None, "c"),
            l(Some(9_000), "d"),
        ];
        assert_eq!(linea_actual(&lineas, 500), None);
        assert_eq!(linea_actual(&lineas, 1_000), Some(0));
        assert_eq!(linea_actual(&lineas, 8_999), Some(1));
        assert_eq!(linea_actual(&lineas, 60_000), Some(3));
        assert_eq!(linea_actual(&[l(None, "sin tiempos")], 60_000), None);
    }

    #[test]
    fn a_lrc_y_de_vuelta() {
        let letra = Letra {
            lineas: vec![l(Some(12_340), "Uno"), l(Some(75_000), "Dos")],
            ..Letra::default()
        };
        let lrc = a_lrc(&letra, Some("Canción"), Some("Artista"));
        assert_eq!(
            lrc,
            "[ti:Canción]\n[ar:Artista]\n[00:12.34]Uno\n[01:15.00]Dos"
        );
        let vuelta = leer(&lrc);
        assert_eq!(vuelta.lineas, letra.lineas);
        assert_eq!(vuelta.titulo.as_deref(), Some("Canción"));
    }

    #[test]
    fn parece_letra_lo_que_tiene_renglones_o_tiempos() {
        assert!(parece_letra(Some("uno\ndos\ntres\ncuatro")));
        assert!(parece_letra(Some("[00:01.00]sola")));
        assert!(!parece_letra(Some("https://ejemplo.com")));
        assert!(!parece_letra(None));
    }

    #[test]
    fn que_es_un_lrc_y_que_no() {
        assert!(es_lrc("[00:12.30]Uno"));
        assert!(es_lrc("[ti:Algo]\nUno"));
        assert!(es_lrc("[00:12]Uno\n[00:15]Dos"), "sin renglon en blanco");
        assert!(es_lrc("[00:12][00:40]Coro"));
        assert!(es_lrc("[0:01] <00:01.00>Hola"));
        assert!(!es_lrc("[0:00] llamar al aparejador\n\n[0:21] y pedir"));
        assert!(!es_lrc("[1:02:03] pasada la hora"));
        assert!(!es_lrc("[Coro: todos]\nla la"));
        assert_eq!(
            convertir_si_es_lrc("[00:01.60]Hola\r\n[00:04.20]Adiós").as_deref(),
            Some("[0:02] Hola\n\n[0:04] Adiós")
        );
        assert_eq!(convertir_si_es_lrc("[0:02] Hola"), None);
    }

    #[test]
    fn sincronizar_a_toques() {
        let mut s = Sincronizador::new(&["Uno", "", "Dos", "Tres"]);
        assert_eq!(s.textos(), ["Uno", "Dos", "Tres"]);
        assert!(s.marcar(1_000));
        assert!(s.marcar(4_000));
        // Deshacer quita la ultima y vuelve un poco antes de la anterior.
        assert_eq!(s.deshacer(), Some(0));
        assert_eq!(s.siguiente(), 1);
        // Un toque «antes» de la anterior no desordena la letra.
        assert!(s.marcar(500));
        assert_eq!(s.tiempo(1), Some(1_000));
        assert!(!s.marcar(7_000));
        assert!(s.terminado());
        assert!(!s.marcar(9_000));
        assert_eq!(
            a_guardar(&s.letra()),
            "[0:01] Uno\n\n[0:01] Dos\n\n[0:07] Tres"
        );
    }

    #[test]
    fn sincronizar_a_medias_deja_lo_que_falta_sin_tiempo() {
        let mut s = Sincronizador::de(&leer("Uno\nDos\nTres"));
        s.marcar(2_000);
        assert_eq!(
            s.letra().lineas,
            vec![l(Some(2_000), "Uno"), l(None, "Dos"), l(None, "Tres")]
        );
        assert_eq!(Sincronizador::new::<&str>(&[]).deshacer(), None);
    }
}
