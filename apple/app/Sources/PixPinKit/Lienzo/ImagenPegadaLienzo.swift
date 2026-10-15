// Las imagenes que entran al lienzo (⌘V o soltarlas encima): de lo que haya
// en el portapapeles o en lo arrastrado a un PNG para `EditorLienzo.pegarImagen`,
// que lo guarda como Windows y el movil (`imagenes/<fileId>` del chat y su
// entrada en `files`, ver `pixpin-nucleo/src/editor_imagenes.rs`).
//
// Rendimiento (§4bis): lo hace ImageIO (hardware), fuera del hilo principal.
// Un PNG que ya cabe pasa TAL CUAL, sin decodificar; lo demas (TIFF de una
// captura, JPEG, HEIC) se reduce al leer (`CGImageSourceCreateThumbnailAtIndex`,
// lado mayor `ladoMaximo`, girado segun su EXIF) y se escribe en PNG.

import AppKit
import Foundation
import ImageIO
import UniformTypeIdentifiers

public enum ImagenPegadaLienzo {
    /// El lado mayor que se deja a una imagen pegada (`LADO_MAXIMO` de Rust,
    /// lo que admite cualquier GPU de las de Windows).
    public static let ladoMaximo = 4096

    /// Los tipos que se aceptan al soltar o pegar.
    public static let tiposDelTablero: [NSPasteboard.PasteboardType] = [.fileURL, .png, .tiff]

    /// Los bytes de una imagen (PNG, TIFF, JPEG, HEIC...) como PNG listo para
    /// pegar. `nil` si no es una imagen.
    public static func png(de datos: Data) -> Data? {
        guard let fuente = CGImageSourceCreateWithData(datos as CFData, nil),
              CGImageSourceGetCount(fuente) > 0 else { return nil }
        let props = CGImageSourceCopyPropertiesAtIndex(fuente, 0, nil) as? [CFString: Any]
        let ancho = (props?[kCGImagePropertyPixelWidth] as? Int) ?? 0
        let alto = (props?[kCGImagePropertyPixelHeight] as? Int) ?? 0
        let orientacion = (props?[kCGImagePropertyOrientation] as? Int) ?? 1
        let tipo = CGImageSourceGetType(fuente) as String?
        // Ya es un PNG que cabe y esta derecho: sin tocarlo.
        if tipo == UTType.png.identifier, ancho > 0, alto > 0,
           max(ancho, alto) <= ladoMaximo, orientacion == 1 {
            return datos
        }
        let opciones: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: min(max(ancho, alto, 1), ladoMaximo),
        ]
        guard let imagen = CGImageSourceCreateThumbnailAtIndex(fuente, 0, opciones as CFDictionary) else {
            return nil
        }
        return png(de: imagen)
    }

    /// Una imagen ya decodificada, en PNG.
    public static func png(de imagen: CGImage) -> Data? {
        let salida = NSMutableData()
        guard let destino = CGImageDestinationCreateWithData(salida as CFMutableData,
                                                             UTType.png.identifier as CFString, 1, nil)
        else { return nil }
        CGImageDestinationAddImage(destino, imagen, nil)
        guard CGImageDestinationFinalize(destino) else { return nil }
        return salida as Data
    }

    /// Lo que hay que leer de un tablero (el portapapeles o lo arrastrado):
    /// el primer fichero de imagen, o los bytes de una imagen copiada. Solo
    /// mira; leer el fichero va despues, fuera del hilo principal.
    public enum Origen: Sendable {
        case fichero(URL)
        case datos(Data)
    }

    public static func origen(en tablero: NSPasteboard) -> Origen? {
        let opciones: [NSPasteboard.ReadingOptionKey: Any] = [.urlReadingFileURLsOnly: true]
        if let urls = tablero.readObjects(forClasses: [NSURL.self], options: opciones) as? [URL] {
            for url in urls {
                if let tipo = UTType(filenameExtension: url.pathExtension), tipo.conforms(to: .image) {
                    return .fichero(url)
                }
            }
        }
        for tipo in [NSPasteboard.PasteboardType.png, .tiff] {
            if let datos = tablero.data(forType: tipo) { return .datos(datos) }
        }
        return nil
    }

    /// El PNG de un origen (lee el fichero mapeado, sin copiarlo a memoria).
    public static func png(de origen: Origen) -> Data? {
        switch origen {
        case let .fichero(url):
            guard let datos = try? Data(contentsOf: url, options: .alwaysMapped) else { return nil }
            return png(de: datos)
        case let .datos(datos):
            return png(de: datos)
        }
    }
}
