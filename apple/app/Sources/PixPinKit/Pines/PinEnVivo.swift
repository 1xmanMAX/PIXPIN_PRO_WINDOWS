// El pin en vivo: una zona de la pantalla vista en directo dentro de un pin
// (lo que en Windows es `pixpin-pin/src/vivo.rs` + `pixpin/src/pin_vivo.rs`
// + `pixpin-capture::RecorteVivo`).
//
// - ScreenCaptureKit (`SCStream`) captura SOLO la zona (`sourceRect`) ya al
//   tamano de sus pixeles, en BGRA, y deja cada fotograma en un `IOSurface`.
//   Ese `IOSurface` va tal cual de contenido a la capa del pin: **cero
//   copias**, la tarjeta grafica lo compone.
// - El ritmo se adapta (`PoliticaFotogramas`): 30 fotogramas por segundo
//   mientras la zona cambia y 2 cuando lleva un segundo quieta (lo dice
//   `SCFrameStatus.idle`). Pausado u oculto, el flujo se para del todo.
// - La zona se marca con un recuadro azul en una ventana que no recoge el
//   raton y no sale en ninguna captura (`sharingType = .none`).
// - Manejar a distancia (`ControlRemoto`): los clics, arrastres y la rueda
//   sobre el pin se mandan a la zona con `CGEvent`. Pide el permiso de
//   Accesibilidad y esta apagado mientras no se de.

import AppKit
import ApplicationServices
import CoreMedia
import CoreVideo
import ScreenCaptureKit
import VideoToolbox

/// Recibe los fotogramas de la zona en su cola y los pone en la capa.
final class FuenteViva: NSObject, SCStreamOutput, SCStreamDelegate, @unchecked Sendable {
    /// La zona en puntos de su pantalla, con el origen arriba a la izquierda.
    let zona: CGRect
    let pantalla: CGDirectDisplayID
    let escala: CGFloat
    /// La capa del pin: aqui van los `IOSurface`.
    let capa: CALayer
    /// El flujo se paro solo (se quito el permiso, se fue la pantalla).
    var alPararse: (@MainActor () -> Void)?

    private let cola = DispatchQueue(label: "PixPin pin en vivo", qos: .userInteractive)
    private let candado = NSLock()
    // Con el candado:
    private var flujo: SCStream?
    private var config: SCStreamConfiguration?
    /// El ultimo fotograma: se guarda para congelar o copiar (y para que su
    /// superficie no vuelva al bote mientras la capa la ensena).
    private var ultimo: CVPixelBuffer?
    // Solo en `cola`:
    private var politica = PoliticaFotogramas()

    init(zona: CGRect, pantalla: CGDirectDisplayID, escala: CGFloat, capa: CALayer) {
        self.zona = zona
        self.pantalla = pantalla
        self.escala = escala
        self.capa = capa
        super.init()
    }

    var capturando: Bool {
        candado.lock()
        defer { candado.unlock() }
        return flujo != nil
    }

    /// Empieza (o reanuda) la captura.
    func arrancar() async throws {
        guard !capturando else { return }
        let contenido = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        guard let pantalla = contenido.displays.first(where: { $0.displayID == self.pantalla }) else {
            throw ErrorGrabacion.sinPantalla
        }
        // Sin las ventanas de PixPin: ni el propio pin ni la barra se ven
        // dentro de si mismos.
        let yo = ProcessInfo.processInfo.processIdentifier
        let propias = contenido.applications.filter { $0.processID == yo }
        let filtro = SCContentFilter(display: pantalla, excludingApplications: propias, exceptingWindows: [])

        let limites = CGRect(x: 0, y: 0, width: pantalla.width, height: pantalla.height)
        let rect = zona.standardized.intersection(limites)
        guard !rect.isNull, rect.width >= 2, rect.height >= 2 else { throw ErrorGrabacion.zonaVacia }
        let px = ReglasVivo.pixeles(rect.size, escala: escala)

        let config = SCStreamConfiguration()
        config.sourceRect = rect
        config.width = px.ancho
        config.height = px.alto
        config.scalesToFit = true
        config.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(PoliticaFotogramas.rapido))
        config.pixelFormat = kCVPixelFormatType_32BGRA
        config.colorSpaceName = CGColorSpace.sRGB
        // Pocas superficies: una en la capa, otra llegando.
        config.queueDepth = 4
        // Se ve el raton cuando pasa por la zona.
        config.showsCursor = true

        let flujo = SCStream(filter: filtro, configuration: config, delegate: self)
        try flujo.addStreamOutput(self, type: .screen, sampleHandlerQueue: cola)
        cola.sync { politica.reiniciar() }
        try await flujo.startCapture()
        candado.lock()
        self.flujo = flujo
        self.config = config
        candado.unlock()
    }

    /// Para la captura; la capa se queda con el ultimo fotograma.
    func parar() async {
        candado.lock()
        let flujo = self.flujo
        self.flujo = nil
        self.config = nil
        candado.unlock()
        guard let flujo else { return }
        try? flujo.removeStreamOutput(self, type: .screen)
        try? await flujo.stopCapture()
    }

    /// Suelta tambien el ultimo fotograma (al cerrar el pin).
    func soltar() async {
        await parar()
        candado.lock()
        ultimo = nil
        candado.unlock()
    }

    /// El fotograma de ahora como foto suelta (copiada: no retiene la
    /// superficie de la captura). `nil` si aun no llego ninguno.
    func fotograma() -> CGImage? {
        candado.lock()
        let pb = ultimo
        candado.unlock()
        guard let pb else { return nil }
        var imagen: CGImage?
        guard VTCreateCGImageFromCVPixelBuffer(pb, options: nil, imageOut: &imagen) == noErr, let imagen else {
            return nil
        }
        return FuenteViva.copia(imagen) ?? imagen
    }

    /// Una copia con sus propios pixeles (la de `VTCreateCGImageFromCVPixelBuffer`
    /// lee de la superficie de la captura).
    static func copia(_ imagen: CGImage) -> CGImage? {
        let espacio = imagen.colorSpace ?? CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB()
        let info = CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        guard let ctx = CGContext(data: nil, width: imagen.width, height: imagen.height, bitsPerComponent: 8,
                                  bytesPerRow: 0, space: espacio, bitmapInfo: info) else { return nil }
        ctx.interpolationQuality = .none
        ctx.draw(imagen, in: CGRect(x: 0, y: 0, width: imagen.width, height: imagen.height))
        return ctx.makeImage()
    }

    // MARK: SCStreamOutput (en `cola`)

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
                of type: SCStreamOutputType) {
        guard type == .screen, sampleBuffer.isValid else { return }
        let estado = FuenteViva.estado(de: sampleBuffer)
        if let ritmo = politica.registrar(estado, ahora: CACurrentMediaTime()) {
            cambiarRitmo(ritmo, en: stream)
        }
        guard estado == .nuevo, let pb = sampleBuffer.imageBuffer,
              let superficie = CVPixelBufferGetIOSurface(pb)?.takeUnretainedValue()
        else { return }
        candado.lock()
        ultimo = pb
        candado.unlock()
        let capa = self.capa
        DispatchQueue.main.async {
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            capa.contents = superficie
            CATransaction.commit()
        }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        candado.lock()
        let era = flujo === stream
        if era {
            flujo = nil
            config = nil
        }
        candado.unlock()
        guard era else { return }
        let aviso = alPararse
        DispatchQueue.main.async { MainActor.assumeIsolated { aviso?() } }
    }

    /// Lo que dice ScreenCaptureKit de un fotograma.
    static func estado(de sampleBuffer: CMSampleBuffer) -> EstadoFotograma {
        guard let adjuntos = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, createIfNecessary: false)
            as? [[SCStreamFrameInfo: Any]],
            let bruto = adjuntos.first?[.status] as? Int,
            let estado = SCFrameStatus(rawValue: bruto)
        else { return .otro }
        switch estado {
        case .complete: return .nuevo
        case .idle: return .quieto
        default: return .otro
        }
    }

    private func cambiarRitmo(_ porSegundo: Int, en stream: SCStream) {
        candado.lock()
        let config = self.config
        candado.unlock()
        guard let config else { return }
        config.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(porSegundo))
        stream.updateConfiguration(config) { _ in }
    }
}

/// El recuadro azul alrededor de la zona: no recoge el raton y no sale en
/// ninguna captura (ni en el propio pin en vivo).
@MainActor
final class MarcoZona {
    private let panel: NSPanel
    private static let grosor: CGFloat = 2

    init(zona: CGRect) {
        let g = MarcoZona.grosor
        panel = NSPanel(contentRect: zona.insetBy(dx: -g - 1, dy: -g - 1),
                        styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
        panel.isReleasedWhenClosed = false
        panel.ignoresMouseEvents = true
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = false
        panel.level = .floating
        panel.sharingType = .none
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        let vista = NSView(frame: panel.contentLayoutRect)
        vista.wantsLayer = true
        vista.layer?.borderWidth = g
        vista.layer?.borderColor = LineasGuia.azul.cgColor
        vista.layer?.cornerRadius = 3
        vista.autoresizingMask = [.width, .height]
        panel.contentView = vista
    }

    func ensenar() { panel.orderFrontRegardless() }
    func esconder() { panel.orderOut(nil) }

    func cerrar() {
        panel.orderOut(nil)
        panel.close()
    }
}

/// Mandar el raton a otra parte de la pantalla con `CGEvent` (el «modo
/// clic» de Windows, `pixpin-shell::entrada`).
@MainActor
enum ControlRemoto {
    /// Si el sistema deja mandar eventos (Accesibilidad).
    static var permitido: Bool { CGPreflightPostEventAccess() }

    /// Explica para que hace falta el permiso y lo pide. `true` si ya esta.
    static func pedirPermiso() -> Bool {
        if permitido { return true }
        let aviso = NSAlert()
        aviso.messageText = "Manejar la zona a distancia"
        aviso.informativeText = """
        Para que los clics, los arrastres y la rueda sobre el pin en vivo lleguen a la zona que \
        copia, PixPin necesita el permiso de Accesibilidad. Solo se usa con este modo encendido.

        Actívalo en Ajustes del Sistema › Privacidad y seguridad › Accesibilidad y vuelve a pulsar \
        el botón del ratón del pin.
        """
        aviso.addButton(withTitle: "Abrir Ajustes")
        aviso.addButton(withTitle: "Ahora no")
        NSApp.activate()
        if aviso.runModal() == .alertFirstButtonReturn {
            // Pide el permiso (el sistema lo apunta en la lista) y abre el
            // panel por si ya se nego antes.
            _ = CGRequestPostEventAccess()
            if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") {
                NSWorkspace.shared.open(url)
            }
        }
        return permitido
    }

    /// Donde esta el cursor ahora, en el espacio de Quartz.
    static func cursorQuartz() -> CGPoint {
        CGEvent(source: nil)?.location ?? .zero
    }

    /// Manda un evento de raton en `p` (Quartz) y devuelve el cursor a
    /// `volver` (para seguir manejando desde el pin).
    static func raton(_ tipo: CGEventType, en p: CGPoint, boton: CGMouseButton, clics: Int, volver: CGPoint?) {
        guard let e = CGEvent(mouseEventSource: nil, mouseType: tipo, mouseCursorPosition: p, mouseButton: boton)
        else { return }
        e.setIntegerValueField(.mouseEventClickState, value: Int64(max(1, clics)))
        e.post(tap: .cghidEventTap)
        if let volver { devolverCursor(a: volver) }
    }

    /// La rueda en `p` (Quartz), en puntos.
    static func rueda(dy: CGFloat, dx: CGFloat, en p: CGPoint, volver: CGPoint?) {
        CGWarpMouseCursorPosition(p)
        if let e = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 2,
                           wheel1: Int32(dy.rounded()), wheel2: Int32(dx.rounded()), wheel3: 0) {
            e.location = p
            e.post(tap: .cghidEventTap)
        }
        if let volver { devolverCursor(a: volver) }
    }

    private static func devolverCursor(a p: CGPoint) {
        CGWarpMouseCursorPosition(p)
        // Tras mover el cursor a mano, el raton tarda un momento en volver a
        // moverlo; asi no se nota.
        CGAssociateMouseAndMouseCursorPosition(1)
    }
}

/// Todo lo de un pin en vivo: la fuente, el recuadro y su estado.
@MainActor
final class ZonaViva {
    /// La zona en coordenadas de AppKit (globales, y hacia arriba).
    let zona: CGRect
    let fuente: FuenteViva
    let marco: MarcoZona
    let capa: CALayer
    /// Pausado por el usuario (▶/⏸ o doble clic).
    private(set) var pausado = false
    /// Oculto (el pin no se ve): tambien para el flujo.
    private var oculto = false
    /// Manejar a distancia encendido.
    var manejando = false

    init?(zona: CGRect, pantalla: NSScreen) {
        guard let numero = pantalla.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber
        else { return nil }
        let capa = CALayer()
        capa.contentsGravity = .resize
        capa.backgroundColor = NSColor.black.cgColor
        capa.contentsScale = pantalla.backingScaleFactor
        self.capa = capa
        self.zona = zona
        let local = ReglasVivo.zonaEnPantalla(zona, pantalla: pantalla.frame)
        fuente = FuenteViva(zona: local, pantalla: CGDirectDisplayID(numero.uint32Value),
                            escala: pantalla.backingScaleFactor, capa: capa)
        marco = MarcoZona(zona: zona)
    }

    /// Empieza a capturar (si no esta pausado ni oculto).
    func arrancar() async throws {
        guard !pausado, !oculto else { return }
        marco.ensenar()
        try await fuente.arrancar()
    }

    func pausar() async {
        pausado = true
        await fuente.parar()
    }

    func reanudar() async throws {
        pausado = false
        try await arrancar()
    }

    func ocultar(_ si: Bool) async {
        oculto = si
        if si {
            marco.esconder()
            await fuente.parar()
        } else {
            try? await arrancar()
        }
    }

    func cerrar() async {
        marco.cerrar()
        await fuente.soltar()
        capa.contents = nil
    }

    /// Un punto del pin (desde su esquina de abajo, en puntos de la vista de
    /// tamano `contenido`) en el espacio de Quartz, o `nil` si cae fuera.
    func puntoQuartz(_ p: CGPoint, contenido: CGSize) -> CGPoint? {
        guard let q = ReglasVivo.puntoEnLaZona(p, contenido: contenido, zona: zona.size) else { return nil }
        let global = CGPoint(x: zona.minX + q.x, y: zona.minY + q.y)
        let alto = NSScreen.screens.first?.frame.height ?? 0
        return ReglasVivo.aQuartz(global, altoPrincipal: alto)
    }
}
