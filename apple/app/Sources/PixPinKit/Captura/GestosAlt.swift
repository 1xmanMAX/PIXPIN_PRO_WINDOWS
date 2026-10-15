// Los gestos con Option (la Alt de Windows) en cualquier sitio del Mac:
// - Option + arrastrar con el izquierdo: copia esa zona;
// - Option + arrastrar con el derecho: la pinea encima, en su sitio;
// - Option + arrastrar con el central: pin en vivo de la zona;
// - Option + doble clic central: el anotador de pantalla.
//
// Hace falta el permiso de Accesibilidad (para ver y quedarse con los
// clics de las demas apps): esta APAGADO de fabrica, se enciende en
// Ajustes y solo entonces se pide el permiso y se instala nada.
//
// Rendimiento y respeto (§4bis del plan):
// - un solo grifo de eventos que solo ESCUCHA las teclas modificadoras
//   (`listenOnly`, no puede retrasar nada);
// - el grifo del raton (el que puede quedarse con un clic) esta apagado y
//   solo se enciende mientras Option esta pulsada; se apaga al soltarla;
// - un Option+clic sin arrastrar se devuelve tal cual (se vuelve a mandar
//   el clic con su Option), para no romper lo que hace cada app con el.

import AppKit
import ApplicationServices
import ScreenCaptureKit
import SwiftUI

/// Las cuentas de los gestos, sin eventos (las prueban las pruebas).
public enum GeometriaGestos {
    public enum Boton: Equatable, Sendable {
        case izquierdo, derecho, central
    }

    /// El boton de un evento de raton; el central es el numero 2.
    public static func boton(tipo: CGEventType, numero: Int64) -> Boton? {
        switch tipo {
        case .leftMouseDown, .leftMouseUp, .leftMouseDragged: return .izquierdo
        case .rightMouseDown, .rightMouseUp, .rightMouseDragged: return .derecho
        case .otherMouseDown, .otherMouseUp, .otherMouseDragged: return numero == 2 ? .central : nil
        default: return nil
        }
    }

    /// Solo Option (sin ⌘, ⌃ ni ⇧): asi Option+⇧+clic sigue siendo de la app.
    public static func soloOpcion(_ f: CGEventFlags) -> Bool {
        f.contains(.maskAlternate) && f.isDisjoint(with: [.maskCommand, .maskControl, .maskShift])
    }

    /// Un arrastre que vale como gesto: al menos 4 puntos por lado.
    public static func esGesto(desde a: CGPoint, hasta b: CGPoint) -> Bool {
        abs(a.x - b.x) >= 4 && abs(a.y - b.y) >= 4
    }
}

@MainActor
public final class GestosAlt: ObservableObject {
    public static let compartido = GestosAlt()

    private static let clave = "gestosAlt.activos"
    /// Marca de los clics que PixPin vuelve a mandar (para dejarlos pasar).
    nonisolated static let marca: Int64 = 0x5049_5850 // «PIXP»

    /// Copia una zona (imagen y pixeles por punto).
    public var alCopiar: ((CGImage, CGFloat) -> Void)?
    /// Pinea una zona: la imagen y su marco en coordenadas de AppKit.
    public var alPinear: ((CGImage, CGRect) -> Void)?
    /// Pin en vivo de una zona (coordenadas globales de AppKit, puntos). Lo
    /// pone quien hace los pines en vivo; sin el, el gesto no hace nada.
    public var alPinEnVivo: ((CGRect) -> Void)?
    /// El anotador de pantalla (Option + doble clic central).
    public var alAnotarPantalla: (() -> Void)?

    @Published public private(set) var instalado = false

    /// Encendidos en Ajustes (apagados de fabrica).
    public var activos: Bool {
        UserDefaults.standard.bool(forKey: GestosAlt.clave)
    }

    /// Si el sistema ya dio el permiso de Accesibilidad.
    public var permitido: Bool { AXIsProcessTrusted() }

    private struct Gesto {
        var boton: GeometriaGestos.Boton
        var inicio: CGPoint
        var actual: CGPoint
        var flags: CGEventFlags
        var clics: Int64
        /// Solo hay que quedarse con el soltar (el doble clic central).
        var soloComer = false
    }

    private var tapTeclas: CFMachPort?
    private var fuenteTeclas: CFRunLoopSource?
    private var tapRaton: CFMachPort?
    private var fuenteRaton: CFRunLoopSource?
    private var gesto: Gesto?
    private var opcion = false
    private var marco: NSPanel?

    public init() {}

    /// Al arrancar la app: instala los grifos si estan encendidos y hay
    /// permiso (sin preguntar nada).
    public func arrancarSiToca() {
        if activos, permitido { instalar() }
        objectWillChange.send()
    }

    /// Enciende o apaga los gestos. Al encender sin permiso, macOS ensena su
    /// aviso para darlo en Ajustes del Sistema (y se instalan en cuanto se
    /// vuelva a comprobar).
    public func fijar(activos: Bool) {
        UserDefaults.standard.set(activos, forKey: GestosAlt.clave)
        if activos {
            let opciones = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
            if AXIsProcessTrustedWithOptions(opciones) { instalar() }
        } else {
            desinstalar()
        }
        objectWillChange.send()
    }

    public func abrirAjustesDeAccesibilidad() {
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") {
            NSWorkspace.shared.open(url)
        }
    }

    // MARK: - Grifos

    private func instalar() {
        guard tapTeclas == nil else { return }
        let yo = Unmanaged.passUnretained(self).toOpaque()
        let teclas = CGEventMask(1 << CGEventType.flagsChanged.rawValue)
        guard let t = CGEvent.tapCreate(tap: .cgSessionEventTap, place: .headInsertEventTap, options: .listenOnly,
                                        eventsOfInterest: teclas, callback: llamadaGestosAlt, userInfo: yo) else { return }
        var raton: CGEventMask = 0
        for tipo: CGEventType in [.leftMouseDown, .leftMouseUp, .leftMouseDragged,
                                  .rightMouseDown, .rightMouseUp, .rightMouseDragged,
                                  .otherMouseDown, .otherMouseUp, .otherMouseDragged] {
            raton |= CGEventMask(1 << tipo.rawValue)
        }
        guard let r = CGEvent.tapCreate(tap: .cgSessionEventTap, place: .headInsertEventTap, options: .defaultTap,
                                        eventsOfInterest: raton, callback: llamadaGestosAlt, userInfo: yo) else {
            CFMachPortInvalidate(t)
            return
        }
        // El del raton nace apagado: solo se enciende con Option pulsada.
        CGEvent.tapEnable(tap: r, enable: false)
        let ft = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, t, 0)
        let fr = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, r, 0)
        CFRunLoopAddSource(CFRunLoopGetMain(), ft, .commonModes)
        CFRunLoopAddSource(CFRunLoopGetMain(), fr, .commonModes)
        tapTeclas = t
        tapRaton = r
        fuenteTeclas = ft
        fuenteRaton = fr
        instalado = true
    }

    private func desinstalar() {
        for (tap, fuente) in [(tapTeclas, fuenteTeclas), (tapRaton, fuenteRaton)] {
            if let fuente { CFRunLoopRemoveSource(CFRunLoopGetMain(), fuente, .commonModes) }
            if let tap {
                CGEvent.tapEnable(tap: tap, enable: false)
                CFMachPortInvalidate(tap)
            }
        }
        tapTeclas = nil
        tapRaton = nil
        fuenteTeclas = nil
        fuenteRaton = nil
        gesto = nil
        ocultarMarco()
        instalado = false
    }

    private func raton(encendido: Bool) {
        guard let tapRaton else { return }
        if CGEvent.tapIsEnabled(tap: tapRaton) != encendido { CGEvent.tapEnable(tap: tapRaton, enable: encendido) }
    }

    /// Un evento de un grifo. Devuelve si PixPin se lo queda.
    fileprivate func manejar(_ tipo: CGEventType, _ e: CGEvent) -> Bool {
        switch tipo {
        case .tapDisabledByTimeout, .tapDisabledByUserInput:
            if let tapTeclas { CGEvent.tapEnable(tap: tapTeclas, enable: true) }
            raton(encendido: opcion || gesto != nil)
            return false
        case .flagsChanged:
            opcion = e.flags.contains(.maskAlternate)
            if opcion { raton(encendido: true) } else if gesto == nil { raton(encendido: false) }
            return false
        default:
            if e.getIntegerValueField(.eventSourceUserData) == GestosAlt.marca { return false }
            return manejarRaton(tipo, e)
        }
    }

    private func manejarRaton(_ tipo: CGEventType, _ e: CGEvent) -> Bool {
        let p = e.location
        guard let boton = GeometriaGestos.boton(tipo: tipo, numero: e.getIntegerValueField(.mouseEventButtonNumber)) else {
            return false
        }
        switch tipo {
        case .leftMouseDown, .rightMouseDown, .otherMouseDown:
            guard gesto == nil, GeometriaGestos.soloOpcion(e.flags) else { return false }
            let clics = e.getIntegerValueField(.mouseEventClickState)
            if boton == .central, clics >= 2 {
                gesto = Gesto(boton: boton, inicio: p, actual: p, flags: e.flags, clics: clics, soloComer: true)
                alAnotarPantalla?()
                return true
            }
            gesto = Gesto(boton: boton, inicio: p, actual: p, flags: e.flags, clics: clics)
            return true
        case .leftMouseDragged, .rightMouseDragged, .otherMouseDragged:
            guard var g = gesto, g.boton == boton else { return false }
            g.actual = p
            gesto = g
            if !g.soloComer, GeometriaGestos.esGesto(desde: g.inicio, hasta: p) {
                ensenarMarco(GeometriaCaptura.rectEntre(g.inicio, p))
            }
            return true
        default: // soltar
            guard let g = gesto, g.boton == boton else { return false }
            gesto = nil
            ocultarMarco()
            if !opcion { raton(encendido: false) }
            if g.soloComer { return true }
            if GeometriaGestos.esGesto(desde: g.inicio, hasta: p) {
                hacer(g.boton, rectCG: GeometriaCaptura.rectEntre(g.inicio, p))
            } else {
                repetirClic(g)
            }
            return true
        }
    }

    /// Un Option+clic sin arrastre: se vuelve a mandar tal cual, marcado.
    private func repetirClic(_ g: Gesto) {
        let tipos: (CGEventType, CGEventType, CGMouseButton)
        switch g.boton {
        case .izquierdo: tipos = (.leftMouseDown, .leftMouseUp, .left)
        case .derecho: tipos = (.rightMouseDown, .rightMouseUp, .right)
        case .central: tipos = (.otherMouseDown, .otherMouseUp, .center)
        }
        for tipo in [tipos.0, tipos.1] {
            guard let e = CGEvent(mouseEventSource: nil, mouseType: tipo, mouseCursorPosition: g.inicio,
                                  mouseButton: tipos.2) else { continue }
            e.flags = g.flags
            e.setIntegerValueField(.mouseEventClickState, value: max(g.clics, 1))
            e.setIntegerValueField(.eventSourceUserData, value: GestosAlt.marca)
            e.post(tap: .cghidEventTap)
        }
    }

    // MARK: - Lo que hace cada gesto

    private func hacer(_ boton: GeometriaGestos.Boton, rectCG: CGRect) {
        let alto = NSScreen.screens.first?.frame.height ?? 0
        let marcoAppKit = GeometriaCaptura.desdeCoordenadasCG(rectCG, altoPantallaPrincipal: alto)
        if boton == .central {
            if let alPinEnVivo { alPinEnVivo(marcoAppKit) } else { NSSound.beep() }
            return
        }
        guard PermisoCaptura.asegurar() else { return }
        Task {
            do {
                guard let foto = try await Instantanea.fotografiar(rectGlobalCG: rectCG) else {
                    NSSound.beep()
                    return
                }
                if boton == .izquierdo {
                    alCopiar?(foto.0, foto.1)
                } else {
                    alPinear?(foto.0, marcoAppKit)
                }
            } catch {
                NSSound.beep()
            }
        }
    }

    private func ensenarMarco(_ rectCG: CGRect) {
        let alto = NSScreen.screens.first?.frame.height ?? 0
        let r = GeometriaCaptura.desdeCoordenadasCG(rectCG, altoPantallaPrincipal: alto)
        let panel: NSPanel
        if let marco {
            panel = marco
        } else {
            panel = NSPanel(contentRect: r, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
            panel.isOpaque = false
            panel.backgroundColor = NSColor.controlAccentColor.withAlphaComponent(0.12)
            panel.hasShadow = false
            panel.ignoresMouseEvents = true
            panel.level = .screenSaver
            panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
            panel.isReleasedWhenClosed = false
            let v = NSView()
            v.wantsLayer = true
            v.layer?.borderWidth = 1.5
            v.layer?.borderColor = NSColor.controlAccentColor.cgColor
            panel.contentView = v
            marco = panel
        }
        panel.setFrame(r, display: true)
        panel.orderFrontRegardless()
    }

    private func ocultarMarco() {
        marco?.orderOut(nil)
    }
}

/// El grifo de eventos llama aqui (una funcion de C: sin capturar nada).
/// Los grifos estan en el bucle principal: se esta en el hilo principal.
private let llamadaGestosAlt: CGEventTapCallBack = { _, tipo, evento, usuario in
    guard let usuario else { return Unmanaged.passUnretained(evento) }
    let gestos = Unmanaged<GestosAlt>.fromOpaque(usuario).takeUnretainedValue()
    let comido = MainActor.assumeIsolated { gestos.manejar(tipo, evento) }
    return comido ? nil : Unmanaged.passUnretained(evento)
}

extension Instantanea {
    /// La foto de un rectangulo en coordenadas globales de Core Graphics
    /// (origen arriba a la izquierda de la pantalla principal), de la
    /// pantalla donde cae su centro, sin las ventanas de PixPin. Devuelve
    /// tambien los pixeles por punto.
    static func fotografiar(rectGlobalCG r: CGRect) async throws -> (CGImage, CGFloat)? {
        let contenido = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        let centro = CGPoint(x: r.midX, y: r.midY)
        guard let display = contenido.displays.first(where: { CGDisplayBounds($0.displayID).contains(centro) }) else {
            return nil
        }
        let bordes = CGDisplayBounds(display.displayID)
        let dentro = r.standardized.intersection(bordes)
        guard !dentro.isNull, dentro.width >= 1, dentro.height >= 1 else { return nil }
        let local = dentro.offsetBy(dx: -bordes.minX, dy: -bordes.minY)
        let escala = await MainActor.run {
            NSScreen.screens.first { $0.numeroDePantalla == display.displayID }?.backingScaleFactor ?? 2
        }
        let yo = ProcessInfo.processInfo.processIdentifier
        let propias = contenido.applications.filter { $0.processID == yo }
        let filtro = SCContentFilter(display: display, excludingApplications: propias, exceptingWindows: [])
        let config = SCStreamConfiguration()
        config.sourceRect = local
        config.width = max(Int((local.width * escala).rounded()), 1)
        config.height = max(Int((local.height * escala).rounded()), 1)
        config.showsCursor = false
        config.captureResolution = .best
        let imagen = try await SCScreenshotManager.captureImage(contentFilter: filtro, configuration: config)
        return (imagen, escala)
    }
}

/// Lo de Ajustes para los gestos con Option y la pila de capturas. Va en
/// `VistaAjustes`, debajo de «Capturar».
public struct SeccionGestosYPila: View {
    @ObservedObject var gestos = GestosAlt.compartido
    @ObservedObject var pila = PilaCapturas.compartida

    public init() {}

    public var body: some View {
        Section("Gestos y pila de capturas") {
            Toggle("Gestos con Option en cualquier sitio", isOn: Binding(
                get: { gestos.activos },
                set: { gestos.fijar(activos: $0) }
            ))
            Text("Option + arrastrar: con el botón izquierdo copia la zona, con el derecho la pinea y con el central "
                + "saca un pin en vivo; Option + doble clic central abre el anotador. Necesita el permiso de "
                + "Accesibilidad para ver los clics de las demás apps; solo se queda con un clic mientras Option está pulsada.")
                .font(.caption).foregroundStyle(.secondary)
            if gestos.activos, !gestos.instalado {
                HStack {
                    Text(gestos.permitido ? "Permiso concedido." : "Falta el permiso de Accesibilidad.")
                        .font(.caption).foregroundStyle(gestos.permitido ? Color.secondary : Color.red)
                    Spacer()
                    Button("Abrir Accesibilidad") { gestos.abrirAjustesDeAccesibilidad() }
                        .controlSize(.small)
                    Button("Comprobar") { gestos.arrancarSiToca() }
                        .controlSize(.small)
                }
            }
            Toggle("Apilar las capturas que se copian seguidas", isOn: Binding(
                get: { pila.activa },
                set: { pila.activa = $0 }
            ))
            Text("Sale un recuadro en la esquina: ⌘V pega todas las de la tanda. Un clic lo deja abierto; "
                + "clic derecho para copiar o vaciar. Se guardan en la galería, como en Windows.")
                .font(.caption).foregroundStyle(.secondary)
        }
    }
}
