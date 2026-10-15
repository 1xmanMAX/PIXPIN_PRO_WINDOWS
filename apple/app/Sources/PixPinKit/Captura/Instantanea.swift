// La foto congelada de cada pantalla (con ScreenCaptureKit, sin las
// ventanas de PixPin) y los marcos de las ventanas para el iman.
//
// Se hace UNA foto por pantalla al abrir la captura y se suelta al
// cerrarla: la superposicion la muestra tal cual (la tarjeta grafica la
// compone desde la capa, sin redibujarla en cada movimiento).

import AppKit
import ScreenCaptureKit

/// La foto de una pantalla, a su resolucion real (pixeles, no puntos).
struct FotoDePantalla {
    let pantalla: NSScreen
    let imagen: CGImage
}

/// Una ventana visible para el iman, en coordenadas globales de AppKit.
struct VentanaVisible {
    let marco: CGRect
    let nombre: String
}

extension NSScreen {
    /// El numero de pantalla de Core Graphics (el `displayID` de
    /// ScreenCaptureKit).
    var numeroDePantalla: CGDirectDisplayID? {
        (deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber)?.uint32Value
    }
}

enum Instantanea {
    /// Una foto de cada pantalla, sin las ventanas de esta app (pines,
    /// menus) y sin el cursor.
    @MainActor
    static func fotografiarTodas() async throws -> [FotoDePantalla] {
        let contenido = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        let yo = ProcessInfo.processInfo.processIdentifier
        let propias = contenido.applications.filter { $0.processID == yo }
        var fotos: [FotoDePantalla] = []
        for pantalla in NSScreen.screens {
            guard let numero = pantalla.numeroDePantalla,
                  let display = contenido.displays.first(where: { $0.displayID == numero }) else { continue }
            let filtro = SCContentFilter(display: display, excludingApplications: propias, exceptingWindows: [])
            let config = SCStreamConfiguration()
            let escala = pantalla.backingScaleFactor
            config.width = Int((CGFloat(display.width) * escala).rounded())
            config.height = Int((CGFloat(display.height) * escala).rounded())
            config.showsCursor = false
            config.captureResolution = .best
            let imagen = try await SCScreenshotManager.captureImage(contentFilter: filtro, configuration: config)
            fotos.append(FotoDePantalla(pantalla: pantalla, imagen: imagen))
        }
        return fotos
    }

    /// Las ventanas normales que se ven, de delante a atras (el orden de
    /// `CGWindowListCopyWindowInfo`), sin las de esta app ni el Dock. Se
    /// pide una vez al abrir la captura, no en cada movimiento del raton.
    @MainActor
    static func ventanasVisibles() -> [VentanaVisible] {
        let opciones: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
        guard let lista = CGWindowListCopyWindowInfo(opciones, kCGNullWindowID) as? [[String: Any]],
              let principal = NSScreen.screens.first else { return [] }
        let alto = principal.frame.height
        let yo = ProcessInfo.processInfo.processIdentifier
        let capaDock = Int(CGWindowLevelForKey(.dockWindow))
        var ventanas: [VentanaVisible] = []
        for info in lista {
            let capa = (info[kCGWindowLayer as String] as? NSNumber)?.intValue ?? -1
            guard capa >= 0, capa < capaDock else { continue }
            if (info[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == yo { continue }
            if let alfa = info[kCGWindowAlpha as String] as? NSNumber, alfa.doubleValue <= 0 { continue }
            guard let dic = info[kCGWindowBounds as String] as? NSDictionary,
                  let marcoCG = CGRect(dictionaryRepresentation: dic as CFDictionary),
                  marcoCG.width >= 20, marcoCG.height >= 20 else { continue }
            let app = info[kCGWindowOwnerName as String] as? String ?? ""
            let titulo = info[kCGWindowName as String] as? String ?? ""
            let nombre = titulo.isEmpty ? app : (app.isEmpty ? titulo : "\(app) — \(titulo)")
            ventanas.append(VentanaVisible(
                marco: GeometriaCaptura.desdeCoordenadasCG(marcoCG, altoPantallaPrincipal: alto),
                nombre: nombre))
        }
        return ventanas
    }
}
