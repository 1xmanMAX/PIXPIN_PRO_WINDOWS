//! Escribir: donde esta el cursor dentro de una cadena y que le hace cada
//! tecla.
//!
//! Aqui no hay ventana, ni escena, ni elemento: solo una cadena y un cursor.
//! Por eso esta separado de `gesto.rs`, que es quien sabe de escenas, y de
//! `ventana_editor.rs`, que es quien sabe de teclas de Windows. Lo que falla
//! de verdad al escribir -que el cursor se salga, que Retroceso parta una
//! letra por la mitad- se prueba entero aqui, sin abrir nada.
//!
//! # El cursor va en CARACTERES, nunca en bytes
//!
//! Es la decision entera de este fichero. El texto es UTF-8: una `ñ` ocupa
//! dos bytes y un emoji cuatro. Un cursor en bytes que avanzara de uno en
//! uno acabaria apuntando a mitad de una letra, y `String::insert` y
//! `String::remove` entran en panico exactamente ahi. Con el cursor en
//! caracteres eso no puede pasar: el indice de byte se calcula cuando hace
//! falta, siempre desde `char_indices`, que solo da comienzos de caracter.
//!
//! Lo que esto NO resuelve, y se sabe: un grafema compuesto (una `e` seguida
//! de una tilde combinante, o una familia de emojis con uniones de ancho
//! cero) son varios caracteres y se borran de uno en uno. Es lo que hace
//! Notepad, y arreglarlo pide una tabla de Unicode que no cabe sin
//! dependencias nuevas.

/// Cuanto separa dos renglones, en multiplos del tamano de letra.
///
/// Publica porque quien pinta el cursor necesita el mismo numero: dos
/// interlineados distintos ponen la barra entre dos lineas.
pub const INTERLINEADO: f32 = 1.3;

/// Lo que se supone que ocupa de ancho un caracter, en multiplos del tamano
/// de letra. Es un promedio a ojo: el motor no tiene DirectWrite y no puede
/// medir de verdad (la misma renuncia que ya hacia el anotador de pines).
const ANCHO_POR_CARACTER: f32 = 0.62;

/// El tamano de letra con el que nace un texto nuevo, en unidades del mundo.
pub const TAM_POR_DEFECTO: f32 = 20.0;

/// La letra con la que nace un texto nuevo. La misma que `Figura::Texto` da
/// por defecto al leer un fichero sin `familia`.
pub const FAMILIA_POR_DEFECTO: &str = "Segoe UI";

/// Una tecla de las que mueven o borran mientras se escribe.
///
/// No incluye las letras: esas llegan ya compuestas (con su IME y su
/// acentuacion) como un `char` y van por `EdicionTexto::insertar`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeclaTexto {
    Izquierda,
    Derecha,
    /// Al principio del RENGLON, no del texto entero: es lo que hace
    /// cualquier editor con varias lineas.
    Inicio,
    /// Al final del renglon, por lo mismo.
    Fin,
    /// Borra el caracter de la izquierda del cursor.
    Retroceso,
    /// Borra el caracter de la derecha del cursor.
    Suprimir,
    /// Hace renglon.
    Entrar,
}

/// Una cadena que se esta escribiendo, con su cursor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EdicionTexto {
    texto: String,
    /// En CARACTERES, nunca en bytes (ve la cabecera del modulo). Invariante:
    /// nunca pasa de `texto.chars().count()`.
    cursor: usize,
}

impl EdicionTexto {
    /// Abre un texto que ya existia, con el cursor al final: es donde lo pone
    /// cualquier editor al que le pides editar algo.
    pub fn nueva(texto: impl Into<String>) -> Self {
        let texto = texto.into();
        let cursor = texto.chars().count();
        Self { texto, cursor }
    }

    pub fn vacia() -> Self {
        Self::default()
    }

    pub fn texto(&self) -> &str {
        &self.texto
    }

    /// El cursor, en caracteres desde el principio.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Cuantos caracteres hay. No son los bytes: `"ñ".len()` es 2 y esto da 1.
    pub fn largo(&self) -> usize {
        self.texto.chars().count()
    }

    pub fn esta_vacio(&self) -> bool {
        self.texto.is_empty()
    }

    /// El indice de BYTE donde empieza el caracter numero `caracteres`.
    ///
    /// Es el unico sitio del fichero que convierte de uno a otro, y sale
    /// siempre de `char_indices`, que solo da comienzos de caracter. Pasarse
    /// del final da el largo en bytes, que es la posicion valida de «detras
    /// de todo».
    fn byte_de(&self, caracteres: usize) -> usize {
        self.texto
            .char_indices()
            .nth(caracteres)
            .map_or(self.texto.len(), |(i, _)| i)
    }

    /// Mete un caracter donde esta el cursor y lo deja detras de el.
    pub fn insertar(&mut self, c: char) {
        let b = self.byte_de(self.cursor);
        self.texto.insert(b, c);
        self.cursor += 1;
    }

    /// Aplica una tecla. Devuelve si cambio algo: con `false` no hay nada que
    /// repintar, y una tecla en el borde (Retroceso al principio) no puede
    /// hacer creer a la ventana que el texto se movio.
    pub fn tecla(&mut self, t: TeclaTexto) -> bool {
        match t {
            TeclaTexto::Entrar => {
                self.insertar('\n');
                true
            }
            TeclaTexto::Izquierda => {
                if self.cursor == 0 {
                    return false;
                }
                self.cursor -= 1;
                true
            }
            TeclaTexto::Derecha => {
                if self.cursor >= self.largo() {
                    return false;
                }
                self.cursor += 1;
                true
            }
            TeclaTexto::Inicio => {
                let (inicio, _) = self.limites_del_renglon();
                let cambia = self.cursor != inicio;
                self.cursor = inicio;
                cambia
            }
            TeclaTexto::Fin => {
                let (_, fin) = self.limites_del_renglon();
                let cambia = self.cursor != fin;
                self.cursor = fin;
                cambia
            }
            TeclaTexto::Retroceso => {
                if self.cursor == 0 {
                    return false;
                }
                self.cursor -= 1;
                let b = self.byte_de(self.cursor);
                self.texto.remove(b);
                true
            }
            TeclaTexto::Suprimir => {
                let b = self.byte_de(self.cursor);
                if b >= self.texto.len() {
                    return false;
                }
                self.texto.remove(b);
                true
            }
        }
    }

    /// En que renglon esta el cursor, contando desde cero.
    pub fn fila(&self) -> usize {
        self.texto
            .chars()
            .take(self.cursor)
            .filter(|c| *c == '\n')
            .count()
    }

    /// Lo que va delante del cursor en SU renglon. Quien pinta lo mide con
    /// DirectWrite para saber a que altura de la linea va la barra; medirlo
    /// aqui seria inventarse el ancho de las letras.
    pub fn prefijo(&self) -> &str {
        let (inicio, _) = self.limites_del_renglon();
        &self.texto[self.byte_de(inicio)..self.byte_de(self.cursor)]
    }

    /// El primer y el ultimo caracter del renglon donde esta el cursor, en
    /// indices de caracter.
    ///
    /// Con el cursor justo delante de un salto de linea, el renglon es el de
    /// ARRIBA: esa posicion es el final de la linea que termina ahi, no el
    /// principio de la siguiente.
    fn limites_del_renglon(&self) -> (usize, usize) {
        let mut inicio = 0;
        let mut fin = self.largo();
        for (i, c) in self.texto.chars().enumerate() {
            if c != '\n' {
                continue;
            }
            if i < self.cursor {
                inicio = i + 1;
            } else {
                fin = i;
                break;
            }
        }
        (inicio, fin)
    }
}

/// Lo que ocupa un texto, a ojo: el motor no tiene con que medir letras.
///
/// Sirve para el hit-test (`impacto::toca` trata el texto como caja solida) y
/// para el `ancho_max` con el que quien pinta reparte las lineas. Se le suma
/// un caracter de holgura a proposito: deja sitio a la barra del cursor al
/// final del renglon, y evita que DirectWrite -que mide de verdad- parta una
/// linea que aqui se estimo un poco corta.
///
/// Un texto vacio mide un renglon, no cero: si midiera cero no se podria
/// pulsar encima para volver a editarlo.
pub fn medida_estimada(texto: &str, tam: f32) -> (f32, f32) {
    let renglones = texto.split('\n').count().max(1);
    let mas_largo = texto
        .split('\n')
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0);
    (
        (mas_largo + 1) as f32 * tam * ANCHO_POR_CARACTER,
        renglones as f32 * tam * INTERLINEADO,
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn abrir_un_texto_que_ya_existia_pone_el_cursor_al_final() {
        let e = EdicionTexto::nueva("hola");
        assert_eq!(e.cursor(), 4);
        assert_eq!(e.prefijo(), "hola");
    }

    #[test]
    fn escribir_mete_la_letra_donde_esta_el_cursor_y_no_al_final() {
        let mut e = EdicionTexto::nueva("ac");
        e.tecla(TeclaTexto::Izquierda);
        e.insertar('b');
        assert_eq!(e.texto(), "abc");
        assert_eq!(e.cursor(), 2, "el cursor queda detras de lo escrito");
    }

    #[test]
    fn retroceso_borra_a_la_izquierda_y_suprimir_a_la_derecha() {
        let mut e = EdicionTexto::nueva("abc");
        e.tecla(TeclaTexto::Izquierda);
        assert!(e.tecla(TeclaTexto::Retroceso));
        assert_eq!(e.texto(), "ac");
        assert!(e.tecla(TeclaTexto::Suprimir));
        assert_eq!(e.texto(), "a");
    }

    #[test]
    fn el_cursor_no_se_sale_de_la_cadena_por_ninguno_de_los_dos_lados() {
        // Caso negativo: la tecla en el borde no mueve nada y lo dice.
        let mut e = EdicionTexto::nueva("ab");
        assert!(!e.tecla(TeclaTexto::Derecha), "ya estaba al final");
        assert_eq!(e.cursor(), 2);
        e.tecla(TeclaTexto::Inicio);
        assert!(!e.tecla(TeclaTexto::Izquierda), "ya estaba al principio");
        assert_eq!(e.cursor(), 0);
    }

    #[test]
    fn borrar_en_una_cadena_vacia_no_revienta_y_no_cambia_nada() {
        // Caso negativo: es la primera tecla que recibe un texto recien
        // abierto si el usuario se arrepiente.
        let mut e = EdicionTexto::vacia();
        assert!(!e.tecla(TeclaTexto::Retroceso));
        assert!(!e.tecla(TeclaTexto::Suprimir));
        assert!(e.esta_vacio());
        assert_eq!(e.cursor(), 0);
    }

    #[test]
    fn un_texto_con_acentos_no_se_parte_por_la_mitad() {
        // El fallo que este fichero existe para evitar: la `ñ` ocupa dos
        // bytes, y un cursor en bytes que retrocediera uno entraria en panico
        // al borrar. Aqui el cursor va en caracteres.
        let mut e = EdicionTexto::nueva("añoño");
        assert_eq!(e.largo(), 5, "cinco letras aunque sean ocho bytes");
        assert!(e.tecla(TeclaTexto::Retroceso));
        assert_eq!(e.texto(), "añoñ");
        e.tecla(TeclaTexto::Izquierda);
        e.insertar('í');
        assert_eq!(e.texto(), "añoíñ");
    }

    #[test]
    fn un_emoji_se_borra_entero_y_no_por_bytes() {
        let mut e = EdicionTexto::nueva("a🐟b");
        e.tecla(TeclaTexto::Izquierda);
        assert!(e.tecla(TeclaTexto::Retroceso));
        assert_eq!(e.texto(), "ab", "el pez se va de una vez, no a cuartos");
    }

    #[test]
    fn el_cursor_recorre_un_emoji_de_un_solo_paso() {
        let mut e = EdicionTexto::nueva("🐟");
        assert_eq!(e.largo(), 1);
        e.tecla(TeclaTexto::Izquierda);
        assert_eq!(e.cursor(), 0);
        e.insertar('x');
        assert_eq!(e.texto(), "x🐟");
    }

    #[test]
    fn entrar_hace_renglon_y_el_cursor_baja_de_fila() {
        let mut e = EdicionTexto::nueva("uno");
        assert_eq!(e.fila(), 0);
        e.tecla(TeclaTexto::Entrar);
        e.insertar('d');
        assert_eq!(e.texto(), "uno\nd");
        assert_eq!(e.fila(), 1);
        assert_eq!(e.prefijo(), "d", "el prefijo es solo lo de su renglon");
    }

    #[test]
    fn inicio_y_fin_van_a_los_extremos_del_renglon_y_no_del_texto() {
        let mut e = EdicionTexto::nueva("uno\ndos");
        e.tecla(TeclaTexto::Inicio);
        assert_eq!(e.cursor(), 4, "al principio de «dos», no del todo");
        assert_eq!(e.fila(), 1);
        assert_eq!(e.prefijo(), "");
        e.tecla(TeclaTexto::Fin);
        assert_eq!(e.cursor(), 7);
        assert_eq!(e.prefijo(), "dos");
    }

    #[test]
    fn el_cursor_delante_de_un_salto_pertenece_al_renglon_de_arriba() {
        // Si perteneciera al de abajo, Inicio saltaria una linea entera y la
        // barra se pintaria un renglon mas abajo de donde se escribe.
        let mut e = EdicionTexto::nueva("uno\ndos");
        for _ in 0..4 {
            e.tecla(TeclaTexto::Izquierda);
        }
        assert_eq!(e.cursor(), 3);
        assert_eq!(e.fila(), 0);
        assert_eq!(e.prefijo(), "uno");
        e.tecla(TeclaTexto::Inicio);
        assert_eq!(e.cursor(), 0);
    }

    #[test]
    fn retroceso_sobre_un_salto_junta_los_dos_renglones() {
        let mut e = EdicionTexto::nueva("uno\ndos");
        e.tecla(TeclaTexto::Inicio);
        assert!(e.tecla(TeclaTexto::Retroceso));
        assert_eq!(e.texto(), "unodos");
        assert_eq!(e.fila(), 0);
    }

    #[test]
    fn la_medida_estimada_crece_con_el_renglon_mas_largo_y_con_cuantos_hay() {
        let (a1, h1) = medida_estimada("ab", 10.0);
        let (a2, h2) = medida_estimada("ab\nabcd", 10.0);
        assert!(a2 > a1, "manda el renglon mas largo");
        assert!(h2 > h1, "dos renglones son mas altos que uno");
        assert!((h1 - 10.0 * INTERLINEADO).abs() < 1e-3);
    }

    #[test]
    fn un_texto_vacio_mide_un_renglon_y_nunca_cero() {
        // Caso negativo: con medida cero no se podria volver a pulsar encima
        // para editarlo, ni se veria la barra del cursor.
        let (ancho, alto) = medida_estimada("", TAM_POR_DEFECTO);
        assert!(ancho > 0.0, "ancho {ancho}");
        assert!(alto > 0.0, "alto {alto}");
    }
}
