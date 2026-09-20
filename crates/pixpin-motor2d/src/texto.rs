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

/// Cuanto se inclina la cursiva, en unidades de sesgo horizontal por unidad de
/// alto (`SESGO_DE_LA_CURSIVA` del movil).
///
/// Un cuarto es lo que usa el propio Android para su italica falsa: mas se lee
/// como un error de pintado y menos no se distingue de la letra recta. Es
/// italica **falsa** en los dos aparatos y a proposito: las tres fuentes de
/// Excalidraw no traen corte italico, asi que o se sesga o no hay cursiva.
pub const SESGO_DE_LA_CURSIVA: f32 = -0.25;

/// A que altura del renglon va la raya del tachado, en multiplos del tamano de
/// letra medidos desde la linea de base hacia arriba.
///
/// Por la mitad de la equis, que es donde la pone cualquier tipografia: mas
/// arriba parece un subrayado del renglon de encima y mas abajo se confunde
/// con la linea de base.
pub const ALTURA_DEL_TACHADO: f32 = 0.30;

/// Lo gorda que va la raya del tachado, en multiplos del tamano de letra.
pub const GROSOR_DEL_TACHADO: f32 = 0.06;

/// **Como va escrito un texto**: negrita, cursiva y tachado.
///
/// Son los tres campos propios de PixPin (`Element.kt:686-688`) y el propio
/// codigo del movil avisa de que se pierden al exportar a `.excalidraw`
/// —Excalidraw no los tiene—, pero **dentro del puente PixPin tienen que
/// viajar**: sin ellos, el titulo en negrita de un esquema vuelve del PC en
/// redonda y lo tachado deja de estar tachado.
///
/// Va como tipo y no como tres booleanos sueltos porque los tres se piden, se
/// pintan y se guardan juntos: quien pinta un texto necesita los tres o
/// ninguno.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EstiloDeTexto {
    pub negrita: bool,
    pub cursiva: bool,
    pub tachado: bool,
}

impl EstiloDeTexto {
    /// Si no hay nada que aplicar: el texto se pinta tal cual.
    pub fn liso(self) -> bool {
        self == EstiloDeTexto::default()
    }

    /// El sesgo horizontal que hay que aplicar al pintar, o cero.
    pub fn sesgo(self) -> f32 {
        if self.cursiva {
            SESGO_DE_LA_CURSIVA
        } else {
            0.0
        }
    }
}

/// La raya que cruza un renglon tachado: de donde a donde, y lo gorda que va.
pub type RayaDeTachado = ((f32, f32), (f32, f32), f32);

/// La raya del tachado de un renglon.
///
/// `x`/`y` son la esquina de arriba a la izquierda del renglon, como en
/// `Orden::Texto`. `None` cuando no hay nada que tachar —sin la marca, o un
/// renglon vacio—: una raya suelta en el aire se lee como un guion largo.
///
/// Se calcula aqui y no en quien pinta porque es la misma cuenta para el
/// editor, para la exportacion y para la miniatura, y tres copias acabarian
/// poniendo la raya a tres alturas.
pub fn raya_del_tachado(
    texto: &str,
    x: f32,
    y: f32,
    tam: f32,
    estilo: EstiloDeTexto,
) -> Option<RayaDeTachado> {
    if !estilo.tachado || texto.trim().is_empty() || tam <= 0.0 {
        return None;
    }
    // Sin la holgura de `medida_estimada`: esa existe para dejar sitio a la
    // barra del cursor, y una raya de tachado que sobresale un caracter por la
    // derecha se lee como un guion pegado a la palabra.
    let ancho = texto.trim_end().chars().count() as f32 * tam * ANCHO_POR_CARACTER;
    if ancho <= 0.0 {
        return None;
    }
    // La base del renglon esta a un alto de letra de su borde de arriba; de
    // ahi se sube la altura del tachado.
    let alto_raya = y + tam - tam * ALTURA_DEL_TACHADO;
    Some((
        (x, alto_raya),
        (x + ancho, alto_raya),
        (tam * GROSOR_DEL_TACHADO).max(1.0),
    ))
}

// -------------------------------------------------------------------------
// Las tres letras de Excalidraw, por su numero
// -------------------------------------------------------------------------

/// Excalifont, la de a mano. La de por omision.
pub const FUENTE_EXCALIFONT: u8 = 5;
/// Nunito, la «normal», para cuando hace falta que se lea limpio.
pub const FUENTE_NUNITO: u8 = 6;
/// Comic Shanns, la monoespaciada.
pub const FUENTE_COMIC_SHANNS: u8 = 8;

/// La familia con la que hay que pintar, **resolviendo los alias viejos**.
///
/// Los numeros no son correlativos y **van en el fichero**: 5, 6 y 8, con 1
/// (Virgil), 2 (Helvetica) y 3 (Cascadia) como alias de los dibujos de antes.
/// Un dibujo guardado con aquellos numeros tiene que seguir viendose con la
/// letra que le toca, no caer al valor por omision.
pub fn familia_resuelta(id: Option<u8>) -> u8 {
    match id {
        None | Some(1) | Some(FUENTE_EXCALIFONT) => FUENTE_EXCALIFONT,
        Some(2) | Some(FUENTE_NUNITO) => FUENTE_NUNITO,
        Some(3) | Some(FUENTE_COMIC_SHANNS) => FUENTE_COMIC_SHANNS,
        _ => FUENTE_EXCALIFONT,
    }
}

/// El numero de familia que le corresponde a un nombre de fuente de Windows.
///
/// **Esto es lo que impide que el movil reabra el texto con otra letra.** Aqui
/// `Figura::Texto` guarda el nombre de una fuente del sistema («Segoe UI») y
/// el fichero espera uno de esos tres numeros: escribir el nombre haria que el
/// movil no reconociera la familia y cayera a la de por omision.
///
/// No adivina: lo que no es ninguna de las tres cae en Excalifont, que es lo
/// que hace el movil con un numero que no conoce.
pub fn numero_de_familia(nombre: &str) -> u8 {
    let n = nombre.trim();
    if n.eq_ignore_ascii_case("Nunito") {
        FUENTE_NUNITO
    } else if n.eq_ignore_ascii_case("Comic Shanns") || n.eq_ignore_ascii_case("Comic Shanns 2") {
        FUENTE_COMIC_SHANNS
    } else {
        FUENTE_EXCALIFONT
    }
}

/// Y al reves: el nombre con el que pedirle esa letra a Windows.
///
/// Si la fuente no esta instalada, quien pinta se queda con la del sistema: es
/// preferible a que no se vea el texto, que es lo que decide el movil con la
/// misma disyuntiva.
pub fn nombre_de_familia(id: Option<u8>) -> &'static str {
    match familia_resuelta(id) {
        FUENTE_NUNITO => "Nunito",
        FUENTE_COMIC_SHANNS => "Comic Shanns",
        _ => "Excalifont",
    }
}

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
mod pruebas_de_estilo {
    use super::*;

    #[test]
    fn un_texto_sin_marcas_se_pinta_tal_cual() {
        let liso = EstiloDeTexto::default();
        assert!(liso.liso());
        assert_eq!(liso.sesgo(), 0.0);
        assert!(raya_del_tachado("hola", 0.0, 0.0, 20.0, liso).is_none());
    }

    #[test]
    fn la_cursiva_inclina_y_la_negrita_no() {
        // Caso negativo: si la negrita tocara el sesgo, un titulo en negrita
        // saldria ademas torcido.
        let cursiva = EstiloDeTexto {
            cursiva: true,
            ..Default::default()
        };
        assert_eq!(cursiva.sesgo(), SESGO_DE_LA_CURSIVA);
        let negrita = EstiloDeTexto {
            negrita: true,
            ..Default::default()
        };
        assert_eq!(negrita.sesgo(), 0.0);
        assert!(!negrita.liso());
    }

    #[test]
    fn la_raya_del_tachado_cruza_la_palabra_por_la_mitad_de_la_equis() {
        let t = EstiloDeTexto {
            tachado: true,
            ..Default::default()
        };
        let ((x0, y0), (x1, y1), grosor) =
            raya_del_tachado("hola", 10.0, 100.0, 20.0, t).expect("hay que tachar");
        assert_eq!(x0, 10.0, "la raya empieza donde el texto");
        assert!(x1 > x0, "la raya no tiene largo");
        assert_eq!(y0, y1, "la raya sale torcida");
        // Ni pegada al borde de arriba del renglon ni en la linea de base.
        assert!(y0 > 100.0 && y0 < 120.0, "la raya cae en {y0}");
        assert!(grosor >= 1.0, "una raya de menos de un pixel no se ve");
    }

    #[test]
    fn un_renglon_en_blanco_no_lleva_raya_suelta_en_el_aire() {
        // Caso negativo: una raya sin texto debajo se lee como un guion largo.
        let t = EstiloDeTexto {
            tachado: true,
            ..Default::default()
        };
        assert!(raya_del_tachado("", 0.0, 0.0, 20.0, t).is_none());
        assert!(raya_del_tachado("   ", 0.0, 0.0, 20.0, t).is_none());
        assert!(raya_del_tachado("hola", 0.0, 0.0, 0.0, t).is_none());
    }

    #[test]
    fn los_numeros_de_fuente_son_los_del_fichero_y_los_alias_viejos_se_resuelven() {
        // Si esto se pierde, un dibujo guardado con la numeracion vieja se
        // reabre con otra letra.
        assert_eq!(familia_resuelta(None), FUENTE_EXCALIFONT);
        assert_eq!(familia_resuelta(Some(1)), FUENTE_EXCALIFONT, "Virgil");
        assert_eq!(familia_resuelta(Some(2)), FUENTE_NUNITO, "Helvetica");
        assert_eq!(familia_resuelta(Some(3)), FUENTE_COMIC_SHANNS, "Cascadia");
        assert_eq!(familia_resuelta(Some(5)), FUENTE_EXCALIFONT);
        assert_eq!(familia_resuelta(Some(6)), FUENTE_NUNITO);
        assert_eq!(familia_resuelta(Some(8)), FUENTE_COMIC_SHANNS);
        // Caso negativo: un numero que no es de nadie cae en la de por
        // omision, no en un panico ni en un hueco.
        assert_eq!(familia_resuelta(Some(99)), FUENTE_EXCALIFONT);
    }

    #[test]
    fn una_fuente_de_windows_no_viaja_al_fichero_con_su_nombre() {
        // **Esto es lo que impide que el movil reabra el texto con otra
        // letra**: el fichero espera 5, 6 u 8, no «Segoe UI».
        assert_eq!(numero_de_familia("Segoe UI"), FUENTE_EXCALIFONT);
        assert_eq!(numero_de_familia("Nunito"), FUENTE_NUNITO);
        assert_eq!(numero_de_familia("comic shanns"), FUENTE_COMIC_SHANNS);
        // Y la ida y vuelta es estable para las tres de verdad.
        for id in [FUENTE_EXCALIFONT, FUENTE_NUNITO, FUENTE_COMIC_SHANNS] {
            assert_eq!(numero_de_familia(nombre_de_familia(Some(id))), id);
        }
    }
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
