//! **Lo que no se mueve bajo el raton** en la ventana de Tareas
//! (1-nov-2026), las cuentas sin pintar nada.
//!
//! El usuario: «cuando hago clic en la casilla de una tarea y voy a hacer
//! clic en la siguiente, la anterior se va y acabo pulsando otra. Que al
//! pulsar se quede en su sitio, marcada (todas las que pulse se quedan
//! marcadas); y cuando salga de la ventana de Tareas y vuelva, que ya no
//! esten: archivadas en «Hechas» como siempre».
//!
//! **La regla.** Marcar se guarda al momento (el documento y lo que viaja
//! al movil no esperan a nada); lo que se queda quieto es solo la VISTA:
//!
//! - lo recien tachado se queda arriba, en su sitio, tachado y apagado; lo
//!   recien destachado (desde «Hechas») se queda abajo, en su sitio;
//! - el orden que se ve —de las listas y de sus tareas— se congela: ni la
//!   prioridad que Jev contesta tarde ni una lista que se queda sin nada
//!   pendiente mueven lo que hay debajo del raton;
//! - lo nuevo (apuntado aqui o llegado de otro aparato) entra en el hueco
//!   que le toca, sin descolocar lo que ya se veia ([`colocar`]).
//!
//! Todo se suelta ([`Quietas::soltar`]) y se vuelve a ordenar cuando se
//! «sale» de Tareas: al cerrar la ventana, al minimizarla, al volver a
//! pedirla (el boton, el atajo, una lista pulsada en el chat), al cambiar de
//! vista y al buscar otra cosa. Perder el foco NO cuenta: el menu del boton
//! derecho, «Mover a…», el cuadro de borrar una lista o mirar una foto se
//! llevan el foco un momento, y soltar entonces moveria todo justo cuando
//! se vuelve a la ventana.

use std::collections::{HashMap, HashSet};

/// Una tarea: la clave de su lista y su texto tal cual esta guardado.
type Tarea = (String, String);

/// Lo congelado mientras la ventana de Tareas sigue abierta.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Quietas {
    /// Las recien tachadas: se quedan arriba, en su sitio.
    hechas: HashSet<Tarea>,
    /// Las recien destachadas desde «Hechas»: se quedan abajo.
    destachadas: HashSet<Tarea>,
    /// El orden en que se vieron las listas (sus claves).
    listas: Vec<String>,
    /// Por lista, el orden en que se vio lo de arriba y lo plegado.
    arriba: HashMap<String, Vec<String>>,
    plegadas: HashMap<String, Vec<String>>,
    /// Lo que se buscaba: buscar otra cosa es mirar de otra manera.
    busqueda: String,
}

impl Quietas {
    /// Se marco (`hecha`) o destaco la tarea `tarea` de la lista `lista`.
    /// Deshacer lo recien hecho la deja como estaba, sin marca.
    pub fn marcar(&mut self, lista: &str, tarea: &str, hecha: bool) {
        let t = (lista.to_string(), tarea.to_string());
        let (deshace, apunta) = if hecha {
            (&mut self.destachadas, &mut self.hechas)
        } else {
            (&mut self.hechas, &mut self.destachadas)
        };
        if !deshace.remove(&t) {
            apunta.insert(t);
        }
    }

    /// La tarea ya no esta (se quito o se movio a otra lista).
    pub fn quitar(&mut self, lista: &str, tarea: &str) {
        let t = (lista.to_string(), tarea.to_string());
        self.hechas.remove(&t);
        self.destachadas.remove(&t);
    }

    /// Si una tarea hecha se queda arriba, con lo pendiente.
    pub fn sigue_arriba(&self, lista: &str, tarea: &str) -> bool {
        self.hechas
            .contains(&(lista.to_string(), tarea.to_string()))
    }

    /// Si una tarea pendiente se queda abajo, con lo hecho.
    pub fn sigue_plegada(&self, lista: &str, tarea: &str) -> bool {
        self.destachadas
            .contains(&(lista.to_string(), tarea.to_string()))
    }

    /// Cuantas se marcaron o destacaron desde la ultima vez que se solto.
    pub fn tocadas(&self) -> usize {
        self.hechas.len() + self.destachadas.len()
    }

    /// Se sale de Tareas: lo tachado se va a «Hechas» y todo se vuelve a
    /// ordenar como toca.
    pub fn soltar(&mut self) {
        let busqueda = std::mem::take(&mut self.busqueda);
        *self = Quietas {
            busqueda,
            ..Quietas::default()
        };
    }

    /// Lo que se busca ahora. Si es otra cosa que antes, se suelta todo y
    /// devuelve `true`.
    pub fn buscar(&mut self, texto: &str) -> bool {
        let texto = texto.trim();
        if texto == self.busqueda {
            return false;
        }
        self.soltar();
        self.busqueda = texto.to_string();
        true
    }

    /// Las listas que se ven, en el orden congelado: lo que ya se veia en
    /// su sitio, lo nuevo en su hueco. `clave` dice de que lista es cada
    /// una. Lo que sale se recuerda para la proxima vez.
    pub fn ordenar_listas<T: Clone>(&mut self, v: &mut [T], clave: impl Fn(&T) -> String) {
        colocar(v, &self.listas, &clave);
        self.listas = v.iter().map(clave).collect();
    }

    /// Las tareas de la lista `lista` que se ven: `arriba` (lo pendiente y
    /// lo recien tachado) y `plegadas` (lo hecho). Lo recien destachado baja
    /// a `plegadas`; los dos montones salen en el orden congelado. `clave`
    /// dice que tarea es cada una (su texto guardado).
    pub fn ordenar_tareas<T: Clone>(
        &mut self,
        lista: &str,
        arriba: &mut Vec<T>,
        plegadas: &mut Vec<T>,
        clave: impl Fn(&T) -> String,
    ) {
        let (bajan, quedan): (Vec<T>, Vec<T>) = arriba
            .drain(..)
            .partition(|x| self.sigue_plegada(lista, &clave(x)));
        *arriba = quedan;
        plegadas.extend(bajan);
        for (v, visto) in [(arriba, &mut self.arriba), (plegadas, &mut self.plegadas)] {
            colocar(
                v,
                visto.get(lista).map(Vec::as_slice).unwrap_or(&[]),
                &clave,
            );
            visto.insert(lista.to_string(), v.iter().map(&clave).collect());
        }
    }
}

/// **Ordena `v` sin descolocar lo ya visto.** Los sitios de `v` que ocupa
/// algo de `visto` se reparten entre eso mismo, en el orden de `visto`; lo
/// que no estaba se queda en el sitio que le dio el orden de siempre. Asi
/// lo de siempre decide donde entra lo nuevo, y lo visto no se mueve entre
/// si. Dos con la misma clave guardan su orden.
pub fn colocar<T: Clone>(v: &mut [T], visto: &[String], clave: &impl Fn(&T) -> String) {
    if visto.is_empty() {
        return;
    }
    let rango: HashMap<&str, usize> = visto
        .iter()
        .enumerate()
        .rev()
        .map(|(n, c)| (c.as_str(), n))
        .collect();
    let rangos: Vec<Option<usize>> = v
        .iter()
        .map(|x| rango.get(clave(x).as_str()).copied())
        .collect();
    let sitios: Vec<usize> = (0..v.len()).filter(|&i| rangos[i].is_some()).collect();
    let mut vistos = sitios.clone();
    vistos.sort_by_key(|&i| rangos[i]);
    let cogidos: Vec<T> = vistos.iter().map(|&i| v[i].clone()).collect();
    for (sitio, x) in sitios.into_iter().zip(cogidos) {
        v[sitio] = x;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn colocar_deja_lo_visto_quieto_y_lo_nuevo_en_su_hueco() {
        // Jev subio «c» arriba del todo: no se mueve mientras se mira.
        let mut v = s(&["c", "a", "b"]);
        colocar(&mut v, &s(&["a", "b", "c"]), &String::clone);
        assert_eq!(v, s(&["a", "b", "c"]));
        // Lo nuevo entra donde lo pone el orden de siempre.
        let mut v = s(&["n", "b", "a"]);
        colocar(&mut v, &s(&["a", "b"]), &String::clone);
        assert_eq!(v, s(&["n", "a", "b"]));
        // Sin nada visto, el orden de siempre.
        let mut v = s(&["b", "a"]);
        colocar(&mut v, &[], &String::clone);
        assert_eq!(v, s(&["b", "a"]));
    }

    #[test]
    fn lo_tachado_se_queda_arriba_hasta_soltar() {
        let mut q = Quietas::default();
        assert!(!q.sigue_arriba("L", "pan"));
        q.marcar("L", "pan", true);
        q.marcar("L", "leche", true);
        assert!(q.sigue_arriba("L", "pan") && q.sigue_arriba("L", "leche"));
        assert!(!q.sigue_arriba("Otra", "pan"), "es de su lista");
        // Destacharla la deja pendiente, sin marca: vuelve a ser normal.
        q.marcar("L", "pan", false);
        assert!(!q.sigue_arriba("L", "pan"));
        assert!(!q.sigue_plegada("L", "pan"));
        q.soltar();
        assert!(!q.sigue_arriba("L", "leche"), "al salir, a «Hechas»");
        assert_eq!(q.tocadas(), 0);
    }

    #[test]
    fn lo_destachado_desde_hechas_se_queda_abajo_en_su_sitio() {
        let mut q = Quietas::default();
        let (mut arriba, mut plegadas) = (s(&["a"]), s(&["h1", "h2", "h3"]));
        q.ordenar_tareas("L", &mut arriba, &mut plegadas, String::clone);
        // Se destacha «h2»: el orden de siempre la sube arriba.
        q.marcar("L", "h2", false);
        let (mut arriba, mut plegadas) = (s(&["a", "h2"]), s(&["h1", "h3"]));
        q.ordenar_tareas("L", &mut arriba, &mut plegadas, String::clone);
        assert_eq!(arriba, s(&["a"]));
        assert_eq!(plegadas, s(&["h1", "h2", "h3"]), "en su sitio");
        // Al salir, arriba con lo pendiente.
        q.soltar();
        let (mut arriba, mut plegadas) = (s(&["a", "h2"]), s(&["h1", "h3"]));
        q.ordenar_tareas("L", &mut arriba, &mut plegadas, String::clone);
        assert_eq!((arriba, plegadas), (s(&["a", "h2"]), s(&["h1", "h3"])));
    }

    #[test]
    fn el_orden_de_las_tareas_se_congela_y_se_suelta_al_salir() {
        let mut q = Quietas::default();
        let mut p = Vec::new();
        let mut arriba = s(&["a", "b", "c"]);
        q.ordenar_tareas("L", &mut arriba, &mut p, String::clone);
        // Jev contesta: «c» es urgente.
        let mut arriba = s(&["c", "a", "b"]);
        q.ordenar_tareas("L", &mut arriba, &mut p, String::clone);
        assert_eq!(arriba, s(&["a", "b", "c"]), "no salta bajo el raton");
        q.soltar();
        let mut arriba = s(&["c", "a", "b"]);
        q.ordenar_tareas("L", &mut arriba, &mut p, String::clone);
        assert_eq!(arriba, s(&["c", "a", "b"]), "al volver, ordenada");
    }

    #[test]
    fn una_lista_sin_pendientes_no_se_va_al_final_mientras_se_mira() {
        let mut q = Quietas::default();
        let mut v = s(&["Inbox", "Casa", "Clase"]);
        q.ordenar_listas(&mut v, String::clone);
        // Tachada la ultima de «Casa», el orden de siempre la manda al final.
        let mut v = s(&["Inbox", "Clase", "Casa"]);
        q.ordenar_listas(&mut v, String::clone);
        assert_eq!(v, s(&["Inbox", "Casa", "Clase"]));
        // Una lista nueva entra en su hueco.
        let mut v = s(&["Inbox", "Nueva", "Clase", "Casa"]);
        q.ordenar_listas(&mut v, String::clone);
        assert_eq!(v, s(&["Inbox", "Nueva", "Casa", "Clase"]));
    }

    #[test]
    fn buscar_otra_cosa_suelta_y_la_misma_no() {
        let mut q = Quietas::default();
        q.marcar("L", "pan", true);
        assert!(!q.buscar("  "), "nada buscado sigue siendo nada");
        assert!(q.sigue_arriba("L", "pan"));
        assert!(q.buscar("pan"));
        assert!(!q.sigue_arriba("L", "pan"));
        q.marcar("L", "pan", true);
        assert!(!q.buscar("pan "), "lo mismo con un espacio");
        assert!(q.sigue_arriba("L", "pan"));
    }

    #[test]
    fn quitar_la_olvida() {
        let mut q = Quietas::default();
        q.marcar("L", "pan", true);
        q.marcar("L", "sal", false);
        q.quitar("L", "pan");
        q.quitar("L", "sal");
        assert_eq!(q.tocadas(), 0);
    }
}
