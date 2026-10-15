// Las cuentas de grabar la pantalla que no tocan la pantalla: que zona,
// a que tamano sale el video, cuantos bits por segundo y como se llama el
// fichero. Puras: las prueban las pruebas sin permiso de grabacion.

import Carbon.HIToolbox
import CoreGraphics
import Foundation

/// Lo que se graba: una zona de una pantalla.
public struct ZonaDeGrabacion: Equatable, Sendable {
    /// La pantalla (`CGDirectDisplayID`, el `displayID` de ScreenCaptureKit).
    public var pantalla: CGDirectDisplayID
    /// La zona en PUNTOS de esa pantalla, con el origen arriba a la
    /// izquierda (lo que pide `SCStreamConfiguration.sourceRect`).
    public var rect: CGRect
    /// Pixeles por punto de la pantalla.
    public var escala: CGFloat

    public init(pantalla: CGDirectDisplayID, rect: CGRect, escala: CGFloat) {
        self.pantalla = pantalla
        self.rect = rect
        self.escala = escala
    }
}

public enum GeometriaGrabacion {
    /// Fotogramas por segundo, los de Windows (`OpcionesMp4::default`).
    public static let porSegundo: Int32 = 30
    /// Lo mas grande que admite H.264 por hardware (nivel 5.2): una pantalla
    /// 5K a 2x se reduce en la propia captura (lo hace la tarjeta grafica)
    /// para que el MP4 sea H.264 y lo abran Windows y Android sin extensiones.
    public static let maximo = CGSize(width: 4096, height: 2304)

    /// Una zona de la vista de la captura (puntos, origen ABAJO a la
    /// izquierda, la vista cubre la pantalla entera de alto `altoPantalla`)
    /// con el origen arriba a la izquierda, como la quiere ScreenCaptureKit.
    public static func desdeVista(_ r: CGRect, altoPantalla: CGFloat) -> CGRect {
        let r = r.standardized
        return CGRect(x: r.minX, y: altoPantalla - r.maxY, width: r.width, height: r.height)
    }

    /// El tamano del video en pixeles: la zona a la resolucion de la
    /// pantalla, reducida (sin deformar) para caber en `maximo`, y con
    /// ancho y alto PARES (H.264 submuestrea el color de dos en dos; como
    /// Windows, se recorta el pixel impar en vez de rellenar).
    public static func tamSalida(puntos: CGSize, escala: CGFloat, maximo: CGSize = GeometriaGrabacion.maximo) -> (ancho: Int, alto: Int) {
        let w = max(puntos.width * max(escala, 1), 2)
        let h = max(puntos.height * max(escala, 1), 2)
        let f = min(1, maximo.width / w, maximo.height / h)
        func par(_ v: CGFloat) -> Int { max(Int((v * f).rounded(.down)) & ~1, 2) }
        return (par(w), par(h))
    }

    /// Bits por segundo del video: 0,1 bits por pixel y segundo con topes,
    /// la misma cuenta que Windows (`bitrate_por_defecto`): 6,2 Mb/s para
    /// 1920x1080 a 30, holgado para que el texto se lea nitido.
    public static func bitsPorSegundo(ancho: Int, alto: Int, porSegundo: Int32 = GeometriaGrabacion.porSegundo) -> Int {
        let pps = Int64(ancho) * Int64(alto) * Int64(porSegundo)
        return Int(min(max(pps / 10, 500_000), 20_000_000))
    }

    /// Donde se guarda una grabacion: `capturas/captura-AAAAMMDD-HHMMSS.mp4`
    /// (con `-2`, `-3`... si ya hay una en ese segundo), el nombre de
    /// Windows, que es el que viaja con la galeria.
    public static func rutaNueva(en carpeta: URL, fecha: Date, zona: TimeZone = .current,
                                 existe: (URL) -> Bool = { FileManager.default.fileExists(atPath: $0.path) }) -> URL {
        GeometriaCaptura.rutaNueva(en: carpeta, fecha: fecha, zona: zona, existe: existe)
            .deletingPathExtension().appendingPathExtension("mp4")
    }
}

extension AtajoGlobal.Combinacion {
    /// Ctrl+Option+R: empezar o parar de grabar la pantalla.
    public static let grabar = AtajoGlobal.Combinacion(tecla: UInt32(kVK_ANSI_R),
                                                       modificadores: UInt32(controlKey | optionKey))
}
