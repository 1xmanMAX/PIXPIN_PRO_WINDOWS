//! GIF animado escrito a mano, sin librerias.
//!
//! Grabar la pantalla en GIF con FFmpeg nos obligaria a decidir sobre la GPL
//! y a arrastrar decenas de megas de binario para producir un formato de
//! 1987. Un GIF es, por dentro, una paleta y un LZW: cabe en un fichero y le
//! basta con `std`. Este modulo es puro —entra un `&[ImagenRgba]`, sale un
//! `Vec<u8>`— porque quien graba decide donde escribirlo.
//!
//! Las decisiones que se tomaron, y por que:
//!
//! - **Una sola paleta global**, no una por fotograma. En una captura de
//!   pantalla los colores casi no cambian de un fotograma al siguiente, asi
//!   que una paleta local costaria 768 bytes por fotograma a cambio de nada.
//! - **Corte por la mediana** sobre un histograma de 5 bits por canal. El
//!   histograma exacto de una captura 4K puede tener millones de entradas y
//!   no cabe en memoria a lo tonto; con 5 bits son 32768 cubetas fijas (1 MB)
//!   y se guarda la suma exacta de cada canal, de modo que el color que
//!   representa a la cubeta es la media real de sus pixeles, no el centro de
//!   la rejilla. La interfaz de Windows es plana: pierde muy poco.
//! - **Se ignora el alfa.** El GIF solo sabe de transparencia binaria y lo
//!   que se graba es la pantalla, que es opaca. Mezclar el alfa aqui seria
//!   inventarse un fondo.
//! - **Sin diferencias entre fotogramas.** Cada fotograma se escribe entero.
//!   Recortar el rectangulo que cambia es la optimizacion evidente, pero
//!   duplica el codigo del ensamblado y aqui aun no hace falta.

use crate::imagen::ImagenRgba;
use std::collections::HashMap;

/// Ajustes de la animacion. El retardo va en centesimas de segundo porque es
/// la unidad del propio formato: traducir aqui desde milisegundos ocultaria
/// que el GIF no sabe de nada mas fino.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcionesGif {
    pub centesimas_por_fotograma: u16,
    pub bucle: bool,
}

impl Default for OpcionesGif {
    fn default() -> Self {
        // 10 centesimas son 10 fotogramas por segundo: el limite de lo que
        // los visores respetan de verdad y suficiente para una demo.
        OpcionesGif {
            centesimas_por_fotograma: 10,
            bucle: true,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorGif {
    #[error("no hay ningun fotograma que codificar")]
    SinFotogramas,
    #[error("el tamano {ancho}x{alto} no cabe en un GIF (ha de ir de 1x1 a 65535x65535)")]
    TamanoInvalido { ancho: u32, alto: u32 },
    #[error(
        "el fotograma {indice} mide {ancho}x{alto} y el primero mide {ancho_esperado}x{alto_esperado}"
    )]
    TamanosDistintos {
        indice: usize,
        ancho: u32,
        alto: u32,
        ancho_esperado: u32,
        alto_esperado: u32,
    },
    #[error("el fotograma {indice} tiene {tiene} bytes pero {ancho}x{alto} necesita {espera}")]
    TamanoIncoherente {
        indice: usize,
        ancho: u32,
        alto: u32,
        tiene: usize,
        espera: usize,
    },
}

/// Bits por canal del histograma. Cinco dan 32768 cubetas: un vector de 1 MB
/// que se reserva una vez por codificacion.
const BITS_CUBETA: u32 = 5;
const NUM_CUBETAS: usize = 1 << (BITS_CUBETA * 3);
const MAXIMO_COLORES: usize = 256;

/// Codifica los fotogramas como un unico GIF89a animado.
pub fn codificar(fotogramas: &[ImagenRgba], opciones: OpcionesGif) -> Result<Vec<u8>, ErrorGif> {
    let (ancho, alto) = validar(fotogramas)?;

    let mut entradas = histograma(fotogramas);
    let paleta = paleta_por_mediana(&mut entradas, MAXIMO_COLORES);
    let tabla = tabla_de_busqueda(&entradas, &paleta);

    let bits_paleta = bits_de_paleta(paleta.len());
    // La especificacion prohibe un tamano de codigo minimo menor que 2: con
    // uno solo no quedarian huecos para los codigos de limpieza y de fin.
    let bits_codigo = bits_paleta.max(2);

    let mut salida = Vec::new();
    escribir_cabecera(&mut salida, ancho, alto, &paleta, bits_paleta);
    if opciones.bucle {
        escribir_netscape(&mut salida);
    }
    for fotograma in fotogramas {
        let indices: Vec<u8> = fotograma
            .pixeles
            .chunks_exact(4)
            .map(|pixel| tabla[clave(pixel[0], pixel[1], pixel[2])])
            .collect();
        escribir_control_grafico(&mut salida, opciones.centesimas_por_fotograma);
        escribir_imagen(&mut salida, ancho, alto, &indices, bits_codigo);
    }
    salida.push(0x3B);
    Ok(salida)
}

/// Medidas comunes a todos los fotogramas, o el motivo por el que no las hay.
fn validar(fotogramas: &[ImagenRgba]) -> Result<(u16, u16), ErrorGif> {
    let primero = fotogramas.first().ok_or(ErrorGif::SinFotogramas)?;
    let (ancho, alto) = (primero.ancho, primero.alto);
    if ancho == 0 || alto == 0 || ancho > u16::MAX as u32 || alto > u16::MAX as u32 {
        return Err(ErrorGif::TamanoInvalido { ancho, alto });
    }
    for (indice, fotograma) in fotogramas.iter().enumerate() {
        if fotograma.ancho != ancho || fotograma.alto != alto {
            return Err(ErrorGif::TamanosDistintos {
                indice,
                ancho: fotograma.ancho,
                alto: fotograma.alto,
                ancho_esperado: ancho,
                alto_esperado: alto,
            });
        }
        let espera = fotograma.bytes_esperados();
        if fotograma.pixeles.len() != espera {
            return Err(ErrorGif::TamanoIncoherente {
                indice,
                ancho: fotograma.ancho,
                alto: fotograma.alto,
                tiene: fotograma.pixeles.len(),
                espera,
            });
        }
    }
    Ok((ancho as u16, alto as u16))
}

// --- Cuantizacion -------------------------------------------------------

/// Cubeta del histograma ya resumida: su color medio y cuantos pixeles cayeron
/// en ella. El corte por la mediana trabaja sobre estas, no sobre los pixeles.
#[derive(Clone, Copy)]
struct Entrada {
    color: [u8; 3],
    cuenta: u64,
    clave: u32,
}

#[derive(Clone, Copy, Default)]
struct Acumulador {
    cuenta: u64,
    suma: [u64; 3],
}

fn clave(r: u8, g: u8, b: u8) -> usize {
    let desplazamiento = 8 - BITS_CUBETA;
    let (r, g, b) = (
        (r as usize) >> desplazamiento,
        (g as usize) >> desplazamiento,
        (b as usize) >> desplazamiento,
    );
    (r << (BITS_CUBETA * 2)) | (g << BITS_CUBETA) | b
}

fn histograma(fotogramas: &[ImagenRgba]) -> Vec<Entrada> {
    let mut cubetas = vec![Acumulador::default(); NUM_CUBETAS];
    for fotograma in fotogramas {
        for pixel in fotograma.pixeles.chunks_exact(4) {
            let cubeta = &mut cubetas[clave(pixel[0], pixel[1], pixel[2])];
            cubeta.cuenta += 1;
            for (suma, canal) in cubeta.suma.iter_mut().zip(pixel) {
                *suma += *canal as u64;
            }
        }
    }
    cubetas
        .iter()
        .enumerate()
        .filter(|(_, cubeta)| cubeta.cuenta > 0)
        .map(|(indice, cubeta)| Entrada {
            color: [
                (cubeta.suma[0] / cubeta.cuenta) as u8,
                (cubeta.suma[1] / cubeta.cuenta) as u8,
                (cubeta.suma[2] / cubeta.cuenta) as u8,
            ],
            cuenta: cubeta.cuenta,
            clave: indice as u32,
        })
        .collect()
}

/// Corte por la mediana: se parte una y otra vez la caja con mas pixeles
/// hasta tener `maximo` cajas, y cada caja aporta un color a la paleta.
/// Reordena `entradas`, porque las cajas son tramos contiguos de ese vector.
fn paleta_por_mediana(entradas: &mut [Entrada], maximo: usize) -> Vec<[u8; 3]> {
    if entradas.is_empty() {
        // Solo pasa si no hay pixeles, y validar() ya lo impide; aun asi la
        // paleta nunca puede quedar vacia o el GIF no tendria tabla de color.
        return vec![[0, 0, 0]];
    }
    let mut cajas = vec![(0usize, entradas.len())];
    while cajas.len() < maximo {
        // Se parte la caja mas poblada: es la que mas pixeles empeora si se
        // queda con un solo color para todos ellos.
        let mut elegida: Option<(usize, u64)> = None;
        for (indice, &(inicio, fin)) in cajas.iter().enumerate() {
            if fin - inicio < 2 {
                continue;
            }
            let peso: u64 = entradas[inicio..fin].iter().map(|e| e.cuenta).sum();
            if elegida.is_none_or(|(_, mejor)| peso > mejor) {
                elegida = Some((indice, peso));
            }
        }
        let Some((indice, _)) = elegida else {
            // Todas las cajas tienen un solo color: la imagen tiene menos
            // colores que el maximo y ya no hay nada que repartir.
            break;
        };
        let (inicio, fin) = cajas[indice];
        let corte = inicio + partir(&mut entradas[inicio..fin]);
        cajas[indice] = (inicio, corte);
        cajas.push((corte, fin));
    }
    cajas
        .iter()
        .map(|&(inicio, fin)| color_medio(&entradas[inicio..fin]))
        .collect()
}

/// Ordena la caja por su canal mas ancho y devuelve por donde partirla, en
/// indices relativos a la propia caja. Siempre deja al menos un color a cada
/// lado, o el bucle de arriba no terminaria.
fn partir(caja: &mut [Entrada]) -> usize {
    let canal = canal_mas_ancho(caja);
    caja.sort_unstable_by_key(|entrada| entrada.color[canal]);
    let total: u64 = caja.iter().map(|entrada| entrada.cuenta).sum();
    let mut acumulado = 0u64;
    for (indice, entrada) in caja.iter().enumerate() {
        acumulado += entrada.cuenta;
        if acumulado * 2 >= total {
            return (indice + 1).clamp(1, caja.len() - 1);
        }
    }
    caja.len() / 2
}

fn canal_mas_ancho(caja: &[Entrada]) -> usize {
    let mut mejor = 0;
    let mut mejor_rango = 0u8;
    for canal in 0..3 {
        let minimo = caja.iter().map(|e| e.color[canal]).min().unwrap_or(0);
        let maximo = caja.iter().map(|e| e.color[canal]).max().unwrap_or(0);
        if maximo - minimo >= mejor_rango {
            mejor_rango = maximo - minimo;
            mejor = canal;
        }
    }
    mejor
}

/// Media de la caja pesada por pixeles: un color que aparece mil veces manda
/// mas sobre el resultado que otro que aparece una.
fn color_medio(caja: &[Entrada]) -> [u8; 3] {
    let total: u64 = caja.iter().map(|entrada| entrada.cuenta).sum();
    if total == 0 {
        return [0, 0, 0];
    }
    let mut color = [0u8; 3];
    for (canal, valor) in color.iter_mut().enumerate() {
        let suma: u64 = caja
            .iter()
            .map(|entrada| entrada.color[canal] as u64 * entrada.cuenta)
            .sum();
        // Se redondea al entero mas cercano en vez de truncar: truncar
        // oscurece toda la imagen medio nivel.
        *valor = ((suma + total / 2) / total) as u8;
    }
    color
}

/// Para cada cubeta con pixeles, el color de la paleta que menos se le parece
/// de lejos. Se resuelve una vez por cubeta —como mucho 32768— y no por
/// pixel, que en 4K serian ocho millones de busquedas entre 256 colores.
fn tabla_de_busqueda(entradas: &[Entrada], paleta: &[[u8; 3]]) -> Vec<u8> {
    let mut tabla = vec![0u8; NUM_CUBETAS];
    for entrada in entradas {
        let mut mejor = 0usize;
        let mut mejor_distancia = u32::MAX;
        for (indice, color) in paleta.iter().enumerate() {
            let distancia = distancia_al_cuadrado(entrada.color, *color);
            if distancia < mejor_distancia {
                mejor_distancia = distancia;
                mejor = indice;
            }
        }
        tabla[entrada.clave as usize] = mejor as u8;
    }
    tabla
}

fn distancia_al_cuadrado(uno: [u8; 3], otro: [u8; 3]) -> u32 {
    (0..3)
        .map(|canal| {
            let diferencia = uno[canal] as i32 - otro[canal] as i32;
            (diferencia * diferencia) as u32
        })
        .sum()
}

/// Bits que hacen falta para indexar la paleta. La tabla de color del GIF
/// siempre ocupa una potencia de dos, con un minimo de dos colores.
fn bits_de_paleta(colores: usize) -> u8 {
    let mut bits = 1u8;
    while (1usize << bits) < colores {
        bits += 1;
    }
    bits
}

// --- Ensamblado del fichero ---------------------------------------------

fn escribir_u16(salida: &mut Vec<u8>, valor: u16) {
    salida.extend_from_slice(&valor.to_le_bytes());
}

fn escribir_cabecera(
    salida: &mut Vec<u8>,
    ancho: u16,
    alto: u16,
    paleta: &[[u8; 3]],
    bits_paleta: u8,
) {
    salida.extend_from_slice(b"GIF89a");
    escribir_u16(salida, ancho);
    escribir_u16(salida, alto);
    // Hay tabla global (bit 7), la profundidad de origen se declara igual que
    // la de la paleta, no se ordena por frecuencia (bit 3) y los tres bits
    // bajos dicen que la tabla tiene 2^(n+1) colores.
    let campo = (bits_paleta - 1) & 0b111;
    salida.push(0b1000_0000 | (campo << 4) | campo);
    salida.push(0); // Color de fondo: el primero de la paleta.
    salida.push(0); // Relacion de aspecto: pixeles cuadrados.
    for indice in 0..(1usize << bits_paleta) {
        let color = paleta.get(indice).copied().unwrap_or([0, 0, 0]);
        salida.extend_from_slice(&color);
    }
}

/// La extension de aplicacion NETSCAPE2.0, que es como se pide un bucle
/// infinito: el formato nunca tuvo una forma oficial de decirlo.
fn escribir_netscape(salida: &mut Vec<u8>) {
    salida.extend_from_slice(&[0x21, 0xFF, 0x0B]);
    salida.extend_from_slice(b"NETSCAPE2.0");
    salida.extend_from_slice(&[0x03, 0x01]);
    escribir_u16(salida, 0); // Cero repeticiones significa "sin fin".
    salida.push(0);
}

fn escribir_control_grafico(salida: &mut Vec<u8>, centesimas: u16) {
    salida.extend_from_slice(&[0x21, 0xF9, 0x04]);
    // Metodo de descarte 1 ("dejar lo pintado"): cada fotograma cubre la
    // pantalla entera, asi que no hay nada que borrar antes del siguiente.
    salida.push(0b0000_0100);
    escribir_u16(salida, centesimas);
    salida.push(0); // Sin color transparente.
    salida.push(0);
}

fn escribir_imagen(salida: &mut Vec<u8>, ancho: u16, alto: u16, indices: &[u8], bits_codigo: u8) {
    salida.push(0x2C);
    escribir_u16(salida, 0);
    escribir_u16(salida, 0);
    escribir_u16(salida, ancho);
    escribir_u16(salida, alto);
    salida.push(0); // Sin tabla local ni entrelazado.
    salida.push(bits_codigo);
    let comprimido = comprimir_lzw(indices, bits_codigo);
    for trozo in comprimido.chunks(255) {
        salida.push(trozo.len() as u8);
        salida.extend_from_slice(trozo);
    }
    salida.push(0); // Sub-bloque vacio: fin de los datos de la imagen.
}

// --- LZW ----------------------------------------------------------------

/// Los codigos se empaquetan de bit menos significativo a mas significativo y
/// pueden cruzar la frontera del byte, asi que hace falta un acumulador.
#[derive(Default)]
struct EscritorBits {
    bytes: Vec<u8>,
    acumulador: u32,
    bits: u8,
}

impl EscritorBits {
    fn escribir(&mut self, codigo: u16, ancho: u8) {
        self.acumulador |= (codigo as u32) << self.bits;
        self.bits += ancho;
        while self.bits >= 8 {
            self.bytes.push((self.acumulador & 0xFF) as u8);
            self.acumulador >>= 8;
            self.bits -= 8;
        }
    }

    fn terminar(mut self) -> Vec<u8> {
        if self.bits > 0 {
            self.bytes.push((self.acumulador & 0xFF) as u8);
        }
        self.bytes
    }
}

/// LZW del GIF, que no es el de TIFF: aqui el ancho de codigo crece cuando el
/// *siguiente* codigo por repartir ya no cabe, sin el desfase de una unidad
/// que usa TIFF. El descodificador va siempre una entrada por detras del
/// codificador, y de ahi que la comprobacion se haga justo despues de emitir.
fn comprimir_lzw(indices: &[u8], bits_codigo: u8) -> Vec<u8> {
    let limpieza = 1u16 << bits_codigo;
    let fin = limpieza + 1;
    let mut escritor = EscritorBits::default();
    let mut ancho = bits_codigo + 1;
    let mut siguiente = fin + 1;
    let mut diccionario: HashMap<(u16, u8), u16> = HashMap::new();

    // El flujo empieza siempre por una limpieza para que el descodificador
    // no dependa de lo que hubiera antes.
    escritor.escribir(limpieza, ancho);

    let mut resto = indices.iter();
    let Some(&primero) = resto.next() else {
        escritor.escribir(fin, ancho);
        return escritor.terminar();
    };
    let mut actual = primero as u16;
    for &indice in resto {
        if let Some(&codigo) = diccionario.get(&(actual, indice)) {
            actual = codigo;
            continue;
        }
        escritor.escribir(actual, ancho);
        if siguiente == 1u16 << ancho {
            if ancho < 12 {
                ancho += 1;
                diccionario.insert((actual, indice), siguiente);
                siguiente += 1;
            } else {
                // Diccionario lleno y sin bits para crecer: se limpia y se
                // vuelve a empezar. No se anade nada tras la limpieza porque
                // el descodificador trata el codigo siguiente como raiz.
                escritor.escribir(limpieza, ancho);
                diccionario.clear();
                ancho = bits_codigo + 1;
                siguiente = fin + 1;
            }
        } else {
            diccionario.insert((actual, indice), siguiente);
            siguiente += 1;
        }
        actual = indice as u16;
    }
    escritor.escribir(actual, ancho);
    // Tras leer ese ultimo codigo el descodificador aun anade una entrada, y
    // con ella puede ensancharse: el codigo de fin ha de ir ya con el ancho
    // nuevo o lo leeria descolocado.
    if siguiente == 1u16 << ancho && ancho < 12 {
        ancho += 1;
    }
    escritor.escribir(fin, ancho);
    escritor.terminar()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn imagen(ancho: u32, alto: u32, colorear: impl Fn(u32, u32) -> [u8; 4]) -> ImagenRgba {
        let mut pixeles = Vec::with_capacity((ancho * alto * 4) as usize);
        for y in 0..alto {
            for x in 0..ancho {
                pixeles.extend_from_slice(&colorear(x, y));
            }
        }
        ImagenRgba {
            ancho,
            alto,
            pixeles,
        }
    }

    fn plana(ancho: u32, alto: u32, color: [u8; 4]) -> ImagenRgba {
        imagen(ancho, alto, |_, _| color)
    }

    /// Generador deterministico: las pruebas del LZW necesitan una entrada
    /// larga y poco comprimible, pero que sea siempre la misma.
    fn pseudoaleatorios(cuantos: usize, modulo: u32) -> Vec<u8> {
        let mut estado = 0x1234_5678u32;
        (0..cuantos)
            .map(|_| {
                estado = estado.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                ((estado >> 16) % modulo) as u8
            })
            .collect()
    }

    // --- Lector de GIF minimo, solo para poder afirmar cosas -------------

    struct BloqueImagen {
        ancho: u16,
        alto: u16,
        bits_codigo: u8,
        datos: Vec<u8>,
    }

    struct GifLeido {
        ancho: u16,
        alto: u16,
        colores_paleta: usize,
        paleta: Vec<[u8; 3]>,
        retardos: Vec<u16>,
        imagenes: Vec<BloqueImagen>,
        bucle: bool,
    }

    /// Recorre los bloques de verdad en vez de buscar bytes sueltos: 0x2C y
    /// 0x21 aparecen tambien dentro de los datos comprimidos, asi que contar
    /// apariciones daria falsos positivos.
    fn leer_gif(bytes: &[u8]) -> GifLeido {
        assert_eq!(&bytes[..6], b"GIF89a");
        let ancho = u16::from_le_bytes([bytes[6], bytes[7]]);
        let alto = u16::from_le_bytes([bytes[8], bytes[9]]);
        let campo = bytes[10];
        assert_eq!(campo & 0b1000_0000, 0b1000_0000, "falta la tabla global");
        let colores_paleta = 1usize << ((campo & 0b111) + 1);
        let mut cursor = 13;
        let paleta = bytes[cursor..cursor + colores_paleta * 3]
            .chunks_exact(3)
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        cursor += colores_paleta * 3;

        let mut leido = GifLeido {
            ancho,
            alto,
            colores_paleta,
            paleta,
            retardos: Vec::new(),
            imagenes: Vec::new(),
            bucle: false,
        };
        loop {
            match bytes[cursor] {
                0x3B => {
                    assert_eq!(cursor, bytes.len() - 1, "hay basura tras el terminador");
                    return leido;
                }
                0x21 => {
                    let etiqueta = bytes[cursor + 1];
                    cursor += 2;
                    if etiqueta == 0xFF && bytes[cursor + 1..cursor + 12] == *b"NETSCAPE2.0" {
                        leido.bucle = true;
                    }
                    if etiqueta == 0xF9 {
                        leido
                            .retardos
                            .push(u16::from_le_bytes([bytes[cursor + 2], bytes[cursor + 3]]));
                    }
                    cursor = saltar_subbloques(bytes, cursor);
                }
                0x2C => {
                    let ancho = u16::from_le_bytes([bytes[cursor + 5], bytes[cursor + 6]]);
                    let alto = u16::from_le_bytes([bytes[cursor + 7], bytes[cursor + 8]]);
                    assert_eq!(bytes[cursor + 9] & 0b1000_0000, 0, "no se usa tabla local");
                    let bits_codigo = bytes[cursor + 10];
                    let mut cursor_datos = cursor + 11;
                    let mut datos = Vec::new();
                    while bytes[cursor_datos] != 0 {
                        let largo = bytes[cursor_datos] as usize;
                        datos.extend_from_slice(&bytes[cursor_datos + 1..cursor_datos + 1 + largo]);
                        cursor_datos += 1 + largo;
                    }
                    leido.imagenes.push(BloqueImagen {
                        ancho,
                        alto,
                        bits_codigo,
                        datos,
                    });
                    cursor = cursor_datos + 1;
                }
                otro => panic!("bloque desconocido {otro:#04X} en {cursor}"),
            }
        }
    }

    /// Desde el primer byte de longitud, devuelve la posicion siguiente al
    /// sub-bloque vacio que cierra la cadena.
    fn saltar_subbloques(bytes: &[u8], mut cursor: usize) -> usize {
        while bytes[cursor] != 0 {
            cursor += 1 + bytes[cursor] as usize;
        }
        cursor + 1
    }

    /// Descompresor de LZW escrito aparte, a partir de la especificacion. Es
    /// la unica forma de comprobar que el compresor dice lo que cree decir:
    /// una prueba contra su propio codigo no probaria nada.
    fn descomprimir_lzw(datos: &[u8], bits_codigo: u8) -> Vec<u8> {
        let limpieza = 1u16 << bits_codigo;
        let fin = limpieza + 1;
        let mut tabla: Vec<Vec<u8>> = Vec::new();
        let reiniciar = |tabla: &mut Vec<Vec<u8>>| {
            tabla.clear();
            for valor in 0..=fin {
                tabla.push(if valor < limpieza {
                    vec![valor as u8]
                } else {
                    Vec::new()
                });
            }
        };
        reiniciar(&mut tabla);

        let mut ancho = bits_codigo + 1;
        let mut salida = Vec::new();
        let mut anterior: Option<u16> = None;
        let mut bit = 0usize;
        loop {
            if bit + ancho as usize > datos.len() * 8 {
                panic!("el flujo se acaba sin codigo de fin");
            }
            let mut codigo = 0u16;
            for desplazamiento in 0..ancho {
                let posicion = bit + desplazamiento as usize;
                let valor = (datos[posicion / 8] >> (posicion % 8)) & 1;
                codigo |= u16::from(valor) << desplazamiento;
            }
            bit += ancho as usize;

            if codigo == limpieza {
                reiniciar(&mut tabla);
                ancho = bits_codigo + 1;
                anterior = None;
                continue;
            }
            if codigo == fin {
                return salida;
            }
            let secuencia = match anterior {
                None => tabla[codigo as usize].clone(),
                Some(previo) => {
                    let mut nueva = tabla[previo as usize].clone();
                    if (codigo as usize) < tabla.len() {
                        nueva.push(tabla[codigo as usize][0]);
                    } else {
                        // El caso en que el codificador usa una entrada que el
                        // descodificador aun no ha creado.
                        nueva.push(tabla[previo as usize][0]);
                    }
                    tabla.push(nueva);
                    if tabla.len() == 1 << ancho && ancho < 12 {
                        ancho += 1;
                    }
                    if (codigo as usize) < tabla.len() - 1 {
                        tabla[codigo as usize].clone()
                    } else {
                        tabla[tabla.len() - 1].clone()
                    }
                }
            };
            salida.extend_from_slice(&secuencia);
            anterior = Some(codigo);
        }
    }

    #[test]
    fn un_gif_de_un_fotograma_empieza_por_la_firma_gif89a_y_acaba_en_el_terminador() {
        let bytes = codificar(&[plana(4, 3, [200, 30, 30, 255])], OpcionesGif::default()).unwrap();
        assert_eq!(&bytes[..6], b"GIF89a");
        assert_eq!(*bytes.last().unwrap(), 0x3B);
    }

    #[test]
    fn un_fotograma_de_un_pixel_declara_medir_uno_por_uno() {
        let bytes = codificar(&[plana(1, 1, [10, 20, 30, 255])], OpcionesGif::default()).unwrap();
        let leido = leer_gif(&bytes);
        assert_eq!((leido.ancho, leido.alto), (1, 1));
        assert_eq!(leido.imagenes.len(), 1);
        assert_eq!((leido.imagenes[0].ancho, leido.imagenes[0].alto), (1, 1));
    }

    #[test]
    fn una_imagen_de_miles_de_colores_se_queda_en_256_o_menos() {
        // Un degradado de 64x64 con los tres canales moviendose: bastantes
        // mas de 256 colores distintos.
        let fotograma = imagen(64, 64, |x, y| {
            [(x * 4) as u8, (y * 4) as u8, ((x + y) * 2) as u8, 255]
        });
        let distintos: std::collections::HashSet<[u8; 3]> = fotograma
            .pixeles
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2]])
            .collect();
        assert!(
            distintos.len() > 256,
            "la prueba necesita mas de 256 colores"
        );

        let bytes = codificar(&[fotograma], OpcionesGif::default()).unwrap();
        let leido = leer_gif(&bytes);
        assert!(leido.colores_paleta <= 256);
        assert_eq!(leido.paleta.len(), leido.colores_paleta);
        // Y ningun indice se sale de la tabla que se acaba de declarar.
        let indices = descomprimir_lzw(&leido.imagenes[0].datos, leido.imagenes[0].bits_codigo);
        assert_eq!(indices.len(), 64 * 64);
        assert!(indices.iter().all(|i| (*i as usize) < leido.colores_paleta));
    }

    #[test]
    fn dos_fotogramas_dan_dos_bloques_de_imagen_con_el_retardo_pedido() {
        let bytes = codificar(
            &[plana(2, 2, [255, 0, 0, 255]), plana(2, 2, [0, 0, 255, 255])],
            OpcionesGif {
                centesimas_por_fotograma: 7,
                bucle: true,
            },
        )
        .unwrap();
        let leido = leer_gif(&bytes);
        assert_eq!(leido.imagenes.len(), 2);
        assert_eq!(leido.retardos, [7, 7]);
    }

    #[test]
    fn el_bucle_infinito_solo_se_declara_si_se_pide() {
        let fotogramas = [plana(2, 2, [1, 2, 3, 255])];
        let con = codificar(
            &fotogramas,
            OpcionesGif {
                centesimas_por_fotograma: 5,
                bucle: true,
            },
        )
        .unwrap();
        assert!(leer_gif(&con).bucle);
        // Caso negativo: sin bucle no debe aparecer la extension NETSCAPE2.0,
        // o los visores repetirian la animacion para siempre.
        let sin = codificar(
            &fotogramas,
            OpcionesGif {
                centesimas_por_fotograma: 5,
                bucle: false,
            },
        )
        .unwrap();
        assert!(!leer_gif(&sin).bucle);
        assert!(!sin.windows(11).any(|v| v == b"NETSCAPE2.0"));
    }

    #[test]
    fn los_colores_de_una_imagen_sencilla_sobreviven_a_la_cuantizacion() {
        let fotograma = imagen(2, 2, |x, y| match (x, y) {
            (0, 0) => [255, 0, 0, 255],
            (1, 0) => [0, 255, 0, 255],
            (0, 1) => [0, 0, 255, 255],
            _ => [255, 255, 255, 255],
        });
        let bytes = codificar(&[fotograma], OpcionesGif::default()).unwrap();
        let leido = leer_gif(&bytes);
        let indices = descomprimir_lzw(&leido.imagenes[0].datos, leido.imagenes[0].bits_codigo);
        let pintados: Vec<[u8; 3]> = indices.iter().map(|i| leido.paleta[*i as usize]).collect();
        assert_eq!(
            pintados,
            [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]]
        );
    }

    #[test]
    fn el_lzw_comprimido_vuelve_a_ser_los_mismos_indices() {
        // Cuatro simbolos y un tamano de codigo minimo de 2: el ancho tiene
        // que crecer varias veces a lo largo del flujo.
        let indices = pseudoaleatorios(5_000, 4);
        let comprimido = comprimir_lzw(&indices, 2);
        assert_eq!(descomprimir_lzw(&comprimido, 2), indices);

        // Y algo cortisimo, donde el diccionario apenas arranca.
        let cortos = [0u8, 0, 1, 1, 0, 1, 0, 0];
        assert_eq!(descomprimir_lzw(&comprimir_lzw(&cortos, 2), 2), cortos);

        // Un solo simbolo: el compresor emite limpieza, codigo y fin.
        assert_eq!(descomprimir_lzw(&comprimir_lzw(&[3], 2), 2), [3]);
    }

    #[test]
    fn el_lzw_se_recupera_cuando_el_diccionario_se_llena() {
        // Con 256 simbolos al azar el diccionario llega a las 4096 entradas
        // mucho antes del final, asi que este flujo pasa por una limpieza.
        let indices = pseudoaleatorios(60_000, 256);
        let comprimido = comprimir_lzw(&indices, 8);
        assert_eq!(descomprimir_lzw(&comprimido, 8), indices);
    }

    #[test]
    fn los_indices_de_un_fotograma_se_leen_enteros_desde_el_gif() {
        let fotograma = imagen(37, 21, |x, y| {
            [(x * 7) as u8, (y * 11) as u8, ((x ^ y) * 3) as u8, 255]
        });
        let bytes = codificar(&[fotograma], OpcionesGif::default()).unwrap();
        let leido = leer_gif(&bytes);
        let indices = descomprimir_lzw(&leido.imagenes[0].datos, leido.imagenes[0].bits_codigo);
        assert_eq!(indices.len(), 37 * 21);
    }

    #[test]
    fn los_datos_comprimidos_van_en_bloques_de_255_como_mucho() {
        // Ruido de 200x200: seguro que no cabe en un solo sub-bloque.
        let ruido = pseudoaleatorios(200 * 200 * 4, 256);
        let fotograma = ImagenRgba {
            ancho: 200,
            alto: 200,
            pixeles: ruido,
        };
        let bytes = codificar(&[fotograma], OpcionesGif::default()).unwrap();
        // Si algun sub-bloque midiera de mas, el lector se perderia y el
        // terminador no caeria justo al final.
        let leido = leer_gif(&bytes);
        assert_eq!(leido.imagenes.len(), 1);
    }

    #[test]
    fn sin_fotogramas_no_hay_gif() {
        // Caso negativo: una grabacion que no llego a capturar nada no puede
        // dar un fichero, porque ni siquiera se sabria de que tamano es.
        assert!(matches!(
            codificar(&[], OpcionesGif::default()),
            Err(ErrorGif::SinFotogramas)
        ));
    }

    #[test]
    fn los_fotogramas_han_de_medir_todos_lo_mismo() {
        // Caso negativo: el GIF declara el tamano una sola vez, en la
        // cabecera; un fotograma de otro tamano no tendria donde decirlo.
        let error = codificar(
            &[plana(4, 4, [0, 0, 0, 255]), plana(4, 5, [0, 0, 0, 255])],
            OpcionesGif::default(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ErrorGif::TamanosDistintos {
                indice: 1,
                alto: 5,
                ..
            }
        ));
    }

    #[test]
    fn un_fotograma_vacio_o_gigante_no_se_codifica() {
        // Caso negativo: cero pixeles no es una imagen.
        assert!(matches!(
            codificar(
                &[ImagenRgba {
                    ancho: 0,
                    alto: 4,
                    pixeles: Vec::new()
                }],
                OpcionesGif::default()
            ),
            Err(ErrorGif::TamanoInvalido { .. })
        ));
        // Caso negativo: el formato guarda las medidas en 16 bits, asi que
        // pasarse de 65535 daria un fichero que miente sobre su tamano.
        assert!(matches!(
            codificar(
                &[ImagenRgba {
                    ancho: 70_000,
                    alto: 1,
                    pixeles: Vec::new()
                }],
                OpcionesGif::default()
            ),
            Err(ErrorGif::TamanoInvalido { .. })
        ));
    }

    #[test]
    fn un_fotograma_con_menos_pixeles_de_los_que_dice_no_se_codifica() {
        // Caso negativo: leer 3x3 pixeles de un buffer de 2 seria leer fuera,
        // asi que se rechaza antes de tocarlo.
        let error = codificar(
            &[ImagenRgba {
                ancho: 3,
                alto: 3,
                pixeles: vec![0; 8],
            }],
            OpcionesGif::default(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ErrorGif::TamanoIncoherente {
                indice: 0,
                tiene: 8,
                espera: 36,
                ..
            }
        ));
    }
}
