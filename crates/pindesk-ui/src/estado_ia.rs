//! **El estado de las IA** (1-nov-2026, el usuario: «unos puntitos rojo o
//! verde, en un sitio donde no molesten, que digan si la API de Claude, de
//! DeepSeek o de Jev funciona o si falta la clave»).
//!
//! Logica pura: que se sabe de cada servicio (sin clave, sin comprobar,
//! funciona, falla y por que), de que color va su puntito y cuando toca
//! volver a comprobarlo. Quien llama le da lo que paso en cada peticion de
//! verdad ([`Desenlace`]) y la hora en ms de un reloj que solo avance; este
//! modulo no sabe de redes ni de ventanas.
//!
//! - **De lo que se usa de verdad**: cada peticion del asistente, de Jev o
//!   de «Probar» en Ajustes apunta su desenlace ([`Tablero::apuntar`]).
//! - **Una comprobacion que no gasta** (la lista de modelos de cada API) a
//!   los [`ESPERA_INICIAL_MS`] de arrancar, al cambiar una clave y, como
//!   mucho, cada [`CADA_MS`] desde lo ultimo que se supo. Sin red se vuelve a
//!   mirar antes, con espera creciente ([`espera_sin_red`]).

/// Los servicios de IA que usa PinDesk y que tienen puntito.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Servicio {
    Claude,
    DeepSeek,
    /// Jev, el modelo de decisiones de TypeSafe.
    Jev,
}

impl Servicio {
    /// En el orden de los puntitos (de arriba abajo).
    pub const TODOS: [Servicio; 3] = [Servicio::Claude, Servicio::DeepSeek, Servicio::Jev];

    /// El nombre que se ensena (es una marca: no se traduce).
    pub fn nombre(self) -> &'static str {
        match self {
            Servicio::Claude => "Claude",
            Servicio::DeepSeek => "DeepSeek",
            Servicio::Jev => "Jev",
        }
    }

    fn indice(self) -> usize {
        match self {
            Servicio::Claude => 0,
            Servicio::DeepSeek => 1,
            Servicio::Jev => 2,
        }
    }
}

/// Por que falla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motivo {
    /// 401 o 403: la clave esta mal copiada, se revoco o no tiene permiso.
    ClaveMala(u16),
    /// 402 (o un 400 que dice que no hay saldo): la cuenta no tiene saldo.
    SinSaldo,
    /// 429: demasiadas peticiones o cuota agotada por ahora.
    Limite,
    /// No se llego a hablar con el servidor (sin red, DNS, TLS, tiempo).
    Red,
    /// 5xx o 529: el servicio esta caido o saturado.
    Servidor(u16),
    /// Otro codigo raro (404: el modelo ya no esta).
    Otro(u16),
}

/// Lo que se sabe de un servicio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Estado {
    /// No hay clave guardada.
    #[default]
    SinClave,
    /// Hay clave, pero aun no se sabe si funciona.
    Desconocido,
    /// La ultima peticion de verdad o la ultima comprobacion salio bien.
    Funciona,
    Falla(Motivo),
}

/// El color del puntito.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Luz {
    /// Funciona: verde lleno.
    Verde,
    /// Falla: rojo lleno.
    Roja,
    /// Sin clave: solo el aro rojo (se ve, pero no grita como un fallo).
    Hueca,
    /// Hay clave y aun no se sabe: gris tenue.
    Gris,
}

impl Estado {
    pub fn luz(self) -> Luz {
        match self {
            Estado::Funciona => Luz::Verde,
            Estado::Falla(_) => Luz::Roja,
            Estado::SinClave => Luz::Hueca,
            Estado::Desconocido => Luz::Gris,
        }
    }

    /// La clave de Fluent de la frase del estado. Las que llevan `codigo`
    /// lo piden con [`Estado::codigo`].
    pub fn clave(self) -> &'static str {
        match self {
            Estado::SinClave => "ia-estado-sin-clave",
            Estado::Desconocido => "ia-estado-desconocido",
            Estado::Funciona => "ia-estado-funciona",
            Estado::Falla(Motivo::ClaveMala(_)) => "ia-estado-clave-mala",
            Estado::Falla(Motivo::SinSaldo) => "ia-estado-sin-saldo",
            Estado::Falla(Motivo::Limite) => "ia-estado-limite",
            Estado::Falla(Motivo::Red) => "ia-estado-red",
            Estado::Falla(Motivo::Servidor(_)) => "ia-estado-servidor",
            Estado::Falla(Motivo::Otro(_)) => "ia-estado-otro",
        }
    }

    /// El codigo HTTP que se ensena entre parentesis, si la frase lo lleva.
    pub fn codigo(self) -> Option<u16> {
        match self {
            Estado::Falla(Motivo::ClaveMala(c) | Motivo::Servidor(c) | Motivo::Otro(c)) => Some(c),
            _ => None,
        }
    }
}

/// Lo que paso en una peticion a la API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desenlace {
    /// El servidor contesto con este codigo (2xx incluido).
    Http(u16),
    /// No se llego a hablar con el servidor.
    Red,
}

impl Desenlace {
    /// Lo que dicen las capas de red: `Some(codigo)` si contesto, `None` si
    /// no se llego. Un codigo que no cabe en `u16` es un codigo raro.
    pub fn de_codigo(codigo: Option<u32>) -> Desenlace {
        match codigo {
            Some(c) => Desenlace::Http(u16::try_from(c).unwrap_or(u16::MAX)),
            None => Desenlace::Red,
        }
    }

    /// El estado al que lleva.
    ///
    /// Un 400, 413 o 422 es de ESA peticion (muy grande, mal formada): el
    /// servidor ya miro la clave antes (401 va primero), asi que el servicio
    /// funciona.
    pub fn estado(self) -> Estado {
        match self {
            Desenlace::Red => Estado::Falla(Motivo::Red),
            Desenlace::Http(c) => match c {
                200..=299 | 400 | 413 | 422 => Estado::Funciona,
                401 | 403 => Estado::Falla(Motivo::ClaveMala(c)),
                402 => Estado::Falla(Motivo::SinSaldo),
                429 => Estado::Falla(Motivo::Limite),
                500..=599 => Estado::Falla(Motivo::Servidor(c)),
                otro => Estado::Falla(Motivo::Otro(otro)),
            },
        }
    }
}

/// Lo que se espera tras arrancar antes de la primera comprobacion: que la
/// app acabe de abrirse sin red de por medio.
pub const ESPERA_INICIAL_MS: u64 = 10_000;
/// Lo mas a menudo que se comprueba un servicio que funciona (o que falla
/// por otra cosa que la red): cada media hora desde lo ultimo que se supo.
pub const CADA_MS: u64 = 30 * 60_000;
/// La primera espera sin red; se dobla con cada fallo seguido.
pub const PRIMERA_SIN_RED_MS: u64 = 60_000;

/// Cuanto se espera tras `seguidos` fallos de red seguidos: 1, 2, 4, 8...
/// minutos, y nunca mas de [`CADA_MS`].
pub fn espera_sin_red(seguidos: u32) -> u64 {
    let doble = seguidos.saturating_sub(1).min(16);
    PRIMERA_SIN_RED_MS.saturating_mul(1 << doble).min(CADA_MS)
}

/// Lo de un servicio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ficha {
    pub estado: Estado,
    /// Cuando se supo `estado` por ultima vez (ms del reloj de quien llama).
    pub cuando: Option<u64>,
    /// Cuando empezo la ultima comprobacion (haya contestado o no).
    intento: Option<u64>,
    /// Fallos de red seguidos.
    sin_red: u32,
    /// Hay que comprobarlo ya (cambio la clave, «Comprobar ahora»).
    pendiente: bool,
    /// Hay una comprobacion en marcha.
    en_curso: bool,
}

impl Ficha {
    pub fn hay_clave(&self) -> bool {
        self.estado != Estado::SinClave
    }

    /// Si hay una comprobacion en marcha (el tooltip dice «comprobando…»).
    pub fn comprobando(&self) -> bool {
        self.en_curso
    }
}

/// **El tablero**: lo que se sabe de los tres servicios.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tablero {
    fichas: [Ficha; 3],
}

impl Tablero {
    pub fn nuevo() -> Tablero {
        Tablero::default()
    }

    pub fn ficha(&self, s: Servicio) -> &Ficha {
        &self.fichas[s.indice()]
    }

    pub fn estado(&self, s: Servicio) -> Estado {
        self.ficha(s).estado
    }

    fn ficha_mut(&mut self, s: Servicio) -> &mut Ficha {
        &mut self.fichas[s.indice()]
    }

    /// **Si hay clave**, mirado en el disco. Quitarla lo deja en «sin
    /// clave»; ponerla, en «sin comprobar» (se comprueba a su hora). Si no
    /// cambia, no toca nada. Devuelve si cambio.
    pub fn poner_clave(&mut self, s: Servicio, hay: bool) -> bool {
        let f = self.ficha_mut(s);
        if f.hay_clave() == hay {
            return false;
        }
        *f = Ficha {
            estado: if hay {
                Estado::Desconocido
            } else {
                Estado::SinClave
            },
            ..Ficha::default()
        };
        true
    }

    /// **El usuario guardo o borro la clave**: con clave, se olvida lo que
    /// se sabia de la anterior y se comprueba ya.
    pub fn clave_cambiada(&mut self, s: Servicio, hay: bool) {
        let f = self.ficha_mut(s);
        *f = Ficha {
            estado: if hay {
                Estado::Desconocido
            } else {
                Estado::SinClave
            },
            pendiente: hay,
            ..Ficha::default()
        };
    }

    /// «Comprobar ahora»: todos los que tienen clave, ya.
    pub fn pedir_ahora(&mut self) {
        for f in &mut self.fichas {
            if f.hay_clave() {
                f.pendiente = true;
            }
        }
    }

    /// **Lo que paso en una peticion** (de verdad o de comprobar). Si
    /// contesto algo, habia clave aunque el disco aun no lo dijera.
    pub fn apuntar(&mut self, s: Servicio, d: Desenlace, ahora: u64) {
        let f = self.ficha_mut(s);
        f.estado = d.estado();
        f.cuando = Some(ahora);
        f.sin_red = if d == Desenlace::Red {
            f.sin_red.saturating_add(1)
        } else {
            0
        };
        f.pendiente = false;
        f.en_curso = false;
    }

    /// Empieza una comprobacion de `s`.
    pub fn empezar(&mut self, s: Servicio, ahora: u64) {
        let f = self.ficha_mut(s);
        f.en_curso = true;
        f.pendiente = false;
        f.intento = Some(ahora);
    }

    /// Acabo la comprobacion de `s` (contestara o no por [`Tablero::apuntar`]).
    pub fn acabar(&mut self, s: Servicio) {
        self.ficha_mut(s).en_curso = false;
    }

    /// **Cuando toca comprobar** `s`, en ms del mismo reloj; `None`: nunca
    /// (sin clave, o ya en marcha). `arranque` es cuando arranco la app.
    pub fn cuando_toca(&self, s: Servicio, arranque: u64) -> Option<u64> {
        let f = self.ficha(s);
        if !f.hay_clave() || f.en_curso {
            return None;
        }
        if f.pendiente {
            return Some(0);
        }
        let base = match (f.cuando, f.intento) {
            (Some(a), Some(b)) => a.max(b),
            (a, b) => match a.or(b) {
                Some(t) => t,
                None => return Some(arranque.saturating_add(ESPERA_INICIAL_MS)),
            },
        };
        let espera = if f.estado == Estado::Falla(Motivo::Red) {
            espera_sin_red(f.sin_red.max(1))
        } else {
            CADA_MS
        };
        Some(base.saturating_add(espera))
    }

    /// Los que toca comprobar ya.
    pub fn por_comprobar(&self, ahora: u64, arranque: u64) -> Vec<Servicio> {
        Servicio::TODOS
            .into_iter()
            .filter(|s| self.cuando_toca(*s, arranque).is_some_and(|t| t <= ahora))
            .collect()
    }

    /// La proxima vez que toca comprobar alguno (para dormir hasta ahi).
    pub fn proxima(&self, arranque: u64) -> Option<u64> {
        Servicio::TODOS
            .into_iter()
            .filter_map(|s| self.cuando_toca(s, arranque))
            .min()
    }

    /// Si todo lo que tiene clave funciona (y hay al menos uno).
    pub fn todo_bien(&self) -> bool {
        let con_clave: Vec<_> = self.fichas.iter().filter(|f| f.hay_clave()).collect();
        !con_clave.is_empty() && con_clave.iter().all(|f| f.estado == Estado::Funciona)
    }
}

/// **Hace cuanto**, para el tooltip: la clave de Fluent y el numero.
/// Menos de un minuto, «ahora mismo»; menos de dos horas, en minutos; y si
/// no, en horas.
pub fn hace(desde: u64, ahora: u64) -> (&'static str, u64) {
    let min = ahora.saturating_sub(desde) / 60_000;
    match min {
        0 => ("ia-estado-hace-nada", 0),
        1..=119 => ("ia-estado-hace-min", min),
        _ => ("ia-estado-hace-horas", min / 60),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_codigo_lleva_a_su_estado() {
        let e = |c| Desenlace::Http(c).estado();
        assert_eq!(e(200), Estado::Funciona);
        assert_eq!(e(204), Estado::Funciona);
        // De esa peticion, no del servicio.
        assert_eq!(e(400), Estado::Funciona);
        assert_eq!(e(413), Estado::Funciona);
        assert_eq!(e(401), Estado::Falla(Motivo::ClaveMala(401)));
        assert_eq!(e(403), Estado::Falla(Motivo::ClaveMala(403)));
        assert_eq!(e(402), Estado::Falla(Motivo::SinSaldo));
        assert_eq!(e(429), Estado::Falla(Motivo::Limite));
        assert_eq!(e(500), Estado::Falla(Motivo::Servidor(500)));
        assert_eq!(e(529), Estado::Falla(Motivo::Servidor(529)));
        assert_eq!(e(404), Estado::Falla(Motivo::Otro(404)));
        assert_eq!(Desenlace::Red.estado(), Estado::Falla(Motivo::Red));
        assert_eq!(Desenlace::de_codigo(None), Desenlace::Red);
        assert_eq!(Desenlace::de_codigo(Some(401)), Desenlace::Http(401));
        assert_eq!(
            Desenlace::de_codigo(Some(70_000)),
            Desenlace::Http(u16::MAX)
        );
    }

    #[test]
    fn los_colores_y_las_frases() {
        assert_eq!(Estado::Funciona.luz(), Luz::Verde);
        assert_eq!(Estado::Falla(Motivo::Red).luz(), Luz::Roja);
        assert_eq!(Estado::SinClave.luz(), Luz::Hueca);
        assert_eq!(Estado::Desconocido.luz(), Luz::Gris);
        assert_eq!(
            Estado::Falla(Motivo::ClaveMala(401)).clave(),
            "ia-estado-clave-mala"
        );
        assert_eq!(Estado::Falla(Motivo::ClaveMala(401)).codigo(), Some(401));
        assert_eq!(Estado::Falla(Motivo::Limite).codigo(), None);
        assert_eq!(Estado::Funciona.codigo(), None);
    }

    #[test]
    fn sin_clave_no_se_comprueba_nunca() {
        let t = Tablero::nuevo();
        for s in Servicio::TODOS {
            assert_eq!(t.estado(s), Estado::SinClave);
            assert_eq!(t.cuando_toca(s, 0), None);
        }
        assert!(t.por_comprobar(u64::MAX, 0).is_empty());
        assert_eq!(t.proxima(0), None);
        assert!(!t.todo_bien());
    }

    #[test]
    fn con_clave_se_comprueba_a_los_diez_segundos_de_arrancar() {
        let mut t = Tablero::nuevo();
        assert!(t.poner_clave(Servicio::Claude, true));
        assert!(!t.poner_clave(Servicio::Claude, true), "sin cambio");
        assert_eq!(t.estado(Servicio::Claude), Estado::Desconocido);
        let arranque = 5_000;
        assert!(t.por_comprobar(arranque + 9_999, arranque).is_empty());
        assert_eq!(
            t.por_comprobar(arranque + ESPERA_INICIAL_MS, arranque),
            vec![Servicio::Claude]
        );
        assert_eq!(t.proxima(arranque), Some(arranque + ESPERA_INICIAL_MS));
    }

    #[test]
    fn lo_de_verdad_cuenta_y_aplaza_la_comprobacion() {
        let mut t = Tablero::nuevo();
        t.poner_clave(Servicio::DeepSeek, true);
        // Una peticion del asistente salio bien antes de la primera
        // comprobacion: ya no hace falta mirar hasta dentro de media hora.
        t.apuntar(Servicio::DeepSeek, Desenlace::Http(200), 3_000);
        assert_eq!(t.estado(Servicio::DeepSeek), Estado::Funciona);
        assert!(t.todo_bien());
        assert!(t.por_comprobar(20_000, 0).is_empty());
        assert_eq!(t.cuando_toca(Servicio::DeepSeek, 0), Some(3_000 + CADA_MS));
        // Y una que falla lo pone en rojo al momento.
        t.apuntar(Servicio::DeepSeek, Desenlace::Http(401), 4_000);
        assert_eq!(t.estado(Servicio::DeepSeek).luz(), Luz::Roja);
        assert!(!t.todo_bien());
    }

    #[test]
    fn sin_red_se_vuelve_a_mirar_con_espera_creciente() {
        assert_eq!(espera_sin_red(1), 60_000);
        assert_eq!(espera_sin_red(2), 120_000);
        assert_eq!(espera_sin_red(3), 240_000);
        assert_eq!(espera_sin_red(10), CADA_MS);
        assert_eq!(espera_sin_red(u32::MAX), CADA_MS);
        let mut t = Tablero::nuevo();
        t.poner_clave(Servicio::Jev, true);
        t.empezar(Servicio::Jev, 10_000);
        assert_eq!(t.cuando_toca(Servicio::Jev, 0), None, "en marcha");
        t.apuntar(Servicio::Jev, Desenlace::Red, 11_000);
        t.acabar(Servicio::Jev);
        assert_eq!(t.cuando_toca(Servicio::Jev, 0), Some(11_000 + 60_000));
        t.empezar(Servicio::Jev, 71_000);
        t.apuntar(Servicio::Jev, Desenlace::Red, 72_000);
        t.acabar(Servicio::Jev);
        assert_eq!(t.cuando_toca(Servicio::Jev, 0), Some(72_000 + 120_000));
        // Vuelve la red: la cuenta se pone a cero.
        t.apuntar(Servicio::Jev, Desenlace::Http(200), 200_000);
        assert_eq!(t.cuando_toca(Servicio::Jev, 0), Some(200_000 + CADA_MS));
    }

    #[test]
    fn una_comprobacion_que_no_contesto_tampoco_se_repite_enseguida() {
        let mut t = Tablero::nuevo();
        t.poner_clave(Servicio::Claude, true);
        t.empezar(Servicio::Claude, 10_000);
        t.acabar(Servicio::Claude);
        assert_eq!(t.cuando_toca(Servicio::Claude, 0), Some(10_000 + CADA_MS));
    }

    #[test]
    fn cambiar_la_clave_olvida_lo_de_antes_y_comprueba_ya() {
        let mut t = Tablero::nuevo();
        t.poner_clave(Servicio::Claude, true);
        t.apuntar(Servicio::Claude, Desenlace::Http(401), 1_000);
        t.clave_cambiada(Servicio::Claude, true);
        assert_eq!(t.estado(Servicio::Claude), Estado::Desconocido);
        assert_eq!(t.por_comprobar(1_001, 0), vec![Servicio::Claude]);
        t.clave_cambiada(Servicio::Claude, false);
        assert_eq!(t.estado(Servicio::Claude), Estado::SinClave);
        assert!(t.por_comprobar(u64::MAX, 0).is_empty());
    }

    #[test]
    fn comprobar_ahora_pide_solo_los_que_tienen_clave() {
        let mut t = Tablero::nuevo();
        t.poner_clave(Servicio::Claude, true);
        t.poner_clave(Servicio::Jev, true);
        t.apuntar(Servicio::Claude, Desenlace::Http(200), 1_000);
        t.apuntar(Servicio::Jev, Desenlace::Http(200), 1_000);
        assert!(t.por_comprobar(2_000, 0).is_empty());
        t.pedir_ahora();
        assert_eq!(
            t.por_comprobar(2_000, 0),
            vec![Servicio::Claude, Servicio::Jev]
        );
        assert_eq!(t.proxima(0), Some(0));
    }

    #[test]
    fn quitar_la_clave_del_disco_lo_deja_sin_clave() {
        let mut t = Tablero::nuevo();
        t.poner_clave(Servicio::Claude, true);
        t.apuntar(Servicio::Claude, Desenlace::Http(200), 1_000);
        assert!(t.poner_clave(Servicio::Claude, false));
        assert_eq!(t.estado(Servicio::Claude), Estado::SinClave);
        assert_eq!(t.ficha(Servicio::Claude).cuando, None);
    }

    #[test]
    fn hace_cuanto() {
        assert_eq!(hace(0, 59_999), ("ia-estado-hace-nada", 0));
        assert_eq!(hace(0, 60_000), ("ia-estado-hace-min", 1));
        assert_eq!(hace(0, 119 * 60_000), ("ia-estado-hace-min", 119));
        assert_eq!(hace(0, 3 * 3_600_000), ("ia-estado-hace-horas", 3));
        // Un reloj que no avanzo no da negativos.
        assert_eq!(hace(5_000, 1_000), ("ia-estado-hace-nada", 0));
    }
}
