// La barra comun de los pines (rediseno v2 de Windows, `pixpin-pin/src/
// barra.rs`): sale al pasar el raton, FUERA del pin y encima (o debajo si
// arriba no cabe), para no tapar lo que se mira.
//
// **La misma barra en todos los pines.** Primero el control propio del
// tipo (el zoom de una foto, la pagina de un PDF, reproducir / linea de
// tiempo / volumen de un video, «EN VIVO» y congelar en un pin en vivo),
// luego lo comun —Copiar, Anotar, Mas— y Cerrar aparte, a la derecha. Cada
// boton ensena debajo su chapita con el atajo («Copiar ⌘C»).
//
// La disposicion es pura (`ReglasBarra`, probada en `BarraPinTests`); la
// ventana es un `NSPanel` hijo del pin, asi que se mueve con el sin hacer
// nada. Mientras no se ve no hay nada vivo: ni temporizadores ni
// observadores del video (los pone el pin solo con la barra a la vista).

import AppKit

/// Lo que hace un boton de la barra.
enum AccionBarraPin: Hashable {
    case alejar, acercar
    case paginaAnterior, paginaSiguiente
    case reproducir, sonido
    case manejar, congelar
    case copiar, anotar, mas, cerrar
    // Mientras se anota.
    case herramienta(HerramientaAnotacion)
    case color(Int)
    case deshacer, fijar, listo
}

/// Un texto fijo de la barra.
enum RotuloBarra: Hashable {
    case zoom, pagina, tiempo, duracion, enVivo
}

enum PiezaBarra: Hashable {
    case boton(AccionBarraPin)
    case rotulo(RotuloBarra)
    /// La linea de tiempo de un video.
    case recorrido
    case separador
}

/// El control propio de cada tipo de pin.
enum PropioBarra: Equatable {
    /// Una herramienta: lo suyo ya esta dentro.
    case ninguno
    /// Foto: alejar, el zoom y acercar.
    case zoom
    /// Un PDF: pasar de pagina (si hay mas de una).
    case paginas(conPaso: Bool)
    case video
    case vivo
}

/// Que barra lleva un pin.
struct TipoBarraPin: Equatable {
    var propio: PropioBarra
    var copiable: Bool
    var anotable: Bool
    /// Se esta anotando: la barra lleva las herramientas.
    var anotando = false
}

enum ReglasBarra {
    static let alto: CGFloat = 40
    static let separacion: CGFloat = 6
    static let altoChapa: CGFloat = 22

    /// Las piezas de la barra de un tipo de pin, en orden.
    static func piezas(_ t: TipoBarraPin) -> [PiezaBarra] {
        if t.anotando {
            var v = HerramientaAnotacion.allCases.map { PiezaBarra.boton(.herramienta($0)) }
            v.append(.separador)
            v += ReglasAnotacion.colores.indices.map { PiezaBarra.boton(.color($0)) }
            v += [.separador, .boton(.deshacer), .boton(.fijar), .separador, .boton(.listo)]
            return v
        }
        var v: [PiezaBarra] = []
        switch t.propio {
        case .ninguno:
            break
        case .zoom:
            v += [.boton(.alejar), .rotulo(.zoom), .boton(.acercar)]
        case let .paginas(conPaso):
            if conPaso { v += [.boton(.paginaAnterior), .rotulo(.pagina), .boton(.paginaSiguiente)] }
        case .video:
            v += [.boton(.reproducir), .rotulo(.tiempo), .recorrido, .rotulo(.duracion), .boton(.sonido)]
        case .vivo:
            v += [.rotulo(.enVivo), .boton(.reproducir), .boton(.manejar), .boton(.congelar)]
        }
        if !v.isEmpty { v.append(.separador) }
        if t.copiable { v.append(.boton(.copiar)) }
        if t.anotable { v.append(.boton(.anotar)) }
        v += [.boton(.mas), .separador, .boton(.cerrar)]
        return v
    }

    /// El nombre de un boton (la chapita y la ayuda).
    static func titulo(_ a: AccionBarraPin) -> String {
        switch a {
        case .alejar: "Alejar"
        case .acercar: "Acercar"
        case .paginaAnterior: "Página anterior"
        case .paginaSiguiente: "Página siguiente"
        case .reproducir: "Reproducir o pausar"
        case .sonido: "Sonido (rueda: volumen)"
        case .manejar: "Manejar la zona"
        case .congelar: "Congelar"
        case .copiar: "Copiar"
        case .anotar: "Anotar"
        case .mas: "Más"
        case .cerrar: "Cerrar"
        case let .herramienta(h): h.titulo
        case let .color(i): ReglasAnotacion.nombresColores[min(max(i, 0), ReglasAnotacion.nombresColores.count - 1)]
        case .deshacer: "Deshacer"
        case .fijar: "Fijar en la imagen"
        case .listo: "Listo"
        }
    }

    /// El atajo que se ensena en la chapita, si lo tiene.
    static func atajo(_ a: AccionBarraPin) -> String? {
        switch a {
        case .alejar: "⌘−"
        case .acercar: "⌘+"
        case .paginaAnterior: "←"
        case .paginaSiguiente: "→"
        case .reproducir: "Espacio"
        case .sonido: "M"
        case .copiar: "⌘C"
        case .anotar: "A"
        case .cerrar: "Esc"
        case let .herramienta(h): h.tecla
        case .deshacer: "⌘Z"
        case .fijar: "↩"
        case .listo: "Esc"
        case .manejar, .congelar, .mas, .color: nil
        }
    }

    /// El texto de la chapita: «Copiar ⌘C».
    static func chapa(_ a: AccionBarraPin) -> String {
        guard let atajo = atajo(a) else { return titulo(a) }
        return "\(titulo(a))  \(atajo)"
    }

    /// El simbolo de SF Symbols de un boton.
    static func simbolo(_ a: AccionBarraPin, activo: Bool = false) -> String {
        switch a {
        case .alejar: "minus.magnifyingglass"
        case .acercar: "plus.magnifyingglass"
        case .paginaAnterior: "chevron.up"
        case .paginaSiguiente: "chevron.down"
        case .reproducir: activo ? "pause.fill" : "play.fill"
        case .sonido: activo ? "speaker.slash.fill" : "speaker.wave.2.fill"
        case .manejar: activo ? "cursorarrow.click.2" : "cursorarrow.click"
        case .congelar: "snowflake"
        case .copiar: "doc.on.doc"
        case .anotar: "pencil.tip.crop.circle"
        case .mas: "ellipsis"
        case .cerrar: "xmark"
        case let .herramienta(h): h.simbolo
        case .color: "circle.fill"
        case .deshacer: "arrow.uturn.backward"
        case .fijar: "checkmark.seal"
        case .listo: "checkmark"
        }
    }

    /// Donde va la barra: centrada encima del pin; si arriba no cabe,
    /// debajo; y siempre dentro de lo visible a lo ancho.
    static func marco(barra: CGSize, pin: CGRect, visible: CGRect) -> CGRect {
        var x = pin.midX - barra.width / 2
        x = ReglasPin.limitar(x, visible.minX, max(visible.minX, visible.maxX - barra.width))
        var y = pin.maxY + separacion
        if y + barra.height > visible.maxY {
            y = pin.minY - separacion - barra.height
        }
        return CGRect(x: x.rounded(), y: y.rounded(), width: barra.width, height: barra.height)
    }

    /// «100 %».
    static func textoZoom(_ porcentaje: Int) -> String { "\(porcentaje) %" }

    /// «m:ss», o «h:mm:ss» pasada la hora.
    static func textoTiempo(_ segundos: Double) -> String {
        guard segundos.isFinite, segundos > 0 else { return "0:00" }
        let s = Int(segundos.rounded(.down))
        let h = s / 3600, m = (s % 3600) / 60, r = s % 60
        if h > 0 { return String(format: "%d:%02d:%02d", h, m, r) }
        return String(format: "%d:%02d", m, r)
    }
}

/// Un boton de la barra: avisa al entrar y salir el raton (la chapita) y
/// acepta el primer clic (el panel no es clave).
final class BotonBarra: NSButton {
    var accionBarra: AccionBarraPin = .mas
    var alEntrar: ((BotonBarra) -> Void)?
    var alSalir: ((BotonBarra) -> Void)?
    /// La rueda encima (el volumen sobre el boton de sonido).
    var alRueda: ((CGFloat) -> Void)?
    private var zona: NSTrackingArea?

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let zona { removeTrackingArea(zona) }
        let nueva = NSTrackingArea(rect: .zero, options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
                                   owner: self, userInfo: nil)
        addTrackingArea(nueva)
        zona = nueva
    }

    override func mouseEntered(with event: NSEvent) { alEntrar?(self) }
    override func mouseExited(with event: NSEvent) { alSalir?(self) }

    override func scrollWheel(with event: NSEvent) {
        if let alRueda { alRueda(event.scrollingDeltaY) } else { super.scrollWheel(with: event) }
    }
}

/// Una linea de tiempo que acepta el primer clic.
final class RecorridoBarra: NSSlider {
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
}

/// La vista de fondo de la barra: avisa al entrar y salir el raton.
final class FondoBarra: NSVisualEffectView {
    var alEntrar: (() -> Void)?
    var alSalir: (() -> Void)?
    private var zona: NSTrackingArea?

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let zona { removeTrackingArea(zona) }
        let nueva = NSTrackingArea(rect: .zero, options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
                                   owner: self, userInfo: nil)
        addTrackingArea(nueva)
        zona = nueva
    }

    override func mouseEntered(with event: NSEvent) { alEntrar?() }
    override func mouseExited(with event: NSEvent) { alSalir?() }
}

/// La barra en pantalla.
@MainActor
final class BarraPin {
    let panel: NSPanel
    private let fondo: FondoBarra
    private let pila: NSStackView
    private let chapa: NSPanel
    private let textoChapa: NSTextField
    private var botones: [AccionBarraPin: BotonBarra] = [:]
    private var rotulos: [RotuloBarra: NSTextField] = [:]
    private(set) var recorrido: RecorridoBarra?
    private(set) var piezas: [PiezaBarra] = []

    var alPulsar: ((AccionBarraPin) -> Void)?
    /// Se movio la linea de tiempo (de 0 a 1).
    var alSaltar: ((Double) -> Void)?
    /// Rueda sobre el sonido.
    var alVolumen: ((CGFloat) -> Void)?
    var alEntrar: (() -> Void)?
    var alSalir: (() -> Void)?

    init() {
        panel = NSPanel(contentRect: CGRect(x: 0, y: 0, width: 200, height: ReglasBarra.alto),
                        styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
        panel.isReleasedWhenClosed = false
        panel.isFloatingPanel = true
        panel.level = .floating
        panel.hidesOnDeactivate = false
        panel.becomesKeyOnlyIfNeeded = true
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = true
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        // La barra no sale en las capturas (ni en un pin en vivo).
        panel.sharingType = .none

        fondo = FondoBarra(frame: panel.contentLayoutRect)
        fondo.material = .hudWindow
        fondo.blendingMode = .behindWindow
        fondo.state = .active
        fondo.wantsLayer = true
        fondo.layer?.cornerRadius = 10
        fondo.layer?.masksToBounds = true
        fondo.autoresizingMask = [.width, .height]
        panel.contentView = fondo

        pila = NSStackView()
        pila.orientation = .horizontal
        pila.spacing = 2
        pila.alignment = .centerY
        pila.edgeInsets = NSEdgeInsets(top: 4, left: 6, bottom: 4, right: 6)
        pila.translatesAutoresizingMaskIntoConstraints = false
        fondo.addSubview(pila)
        NSLayoutConstraint.activate([
            pila.leadingAnchor.constraint(equalTo: fondo.leadingAnchor),
            pila.trailingAnchor.constraint(equalTo: fondo.trailingAnchor),
            pila.topAnchor.constraint(equalTo: fondo.topAnchor),
            pila.bottomAnchor.constraint(equalTo: fondo.bottomAnchor),
        ])

        chapa = NSPanel(contentRect: CGRect(x: 0, y: 0, width: 80, height: ReglasBarra.altoChapa),
                        styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
        chapa.isReleasedWhenClosed = false
        chapa.ignoresMouseEvents = true
        chapa.backgroundColor = .clear
        chapa.isOpaque = false
        chapa.hasShadow = false
        chapa.level = .floating
        chapa.sharingType = .none
        chapa.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        let caja = NSView(frame: chapa.contentLayoutRect)
        caja.wantsLayer = true
        caja.layer?.backgroundColor = NSColor.black.withAlphaComponent(0.8).cgColor
        caja.layer?.cornerRadius = 5
        caja.autoresizingMask = [.width, .height]
        textoChapa = NSTextField(labelWithString: "")
        textoChapa.font = .systemFont(ofSize: 11, weight: .medium)
        textoChapa.textColor = .white
        textoChapa.alignment = .center
        textoChapa.frame = caja.bounds.insetBy(dx: 6, dy: 3)
        textoChapa.autoresizingMask = [.width, .height]
        caja.addSubview(textoChapa)
        chapa.contentView = caja

        fondo.alEntrar = { [weak self] in self?.alEntrar?() }
        fondo.alSalir = { [weak self] in self?.alSalir?() }
    }

    /// Monta las piezas (solo si cambiaron).
    func montar(_ nuevas: [PiezaBarra]) {
        guard nuevas != piezas else { return }
        piezas = nuevas
        for v in pila.arrangedSubviews {
            pila.removeArrangedSubview(v)
            v.removeFromSuperview()
        }
        botones = [:]
        rotulos = [:]
        recorrido = nil
        for p in nuevas {
            switch p {
            case let .boton(a):
                let b = BotonBarra()
                b.accionBarra = a
                b.isBordered = false
                b.bezelStyle = .regularSquare
                b.imagePosition = .imageOnly
                b.image = NSImage(systemSymbolName: ReglasBarra.simbolo(a), accessibilityDescription: ReglasBarra.titulo(a))
                b.symbolConfiguration = NSImage.SymbolConfiguration(pointSize: 14, weight: .medium)
                if case let .color(i) = a {
                    b.contentTintColor = NSColor(cgColor: ReglasAnotacion.color(i))
                } else {
                    b.contentTintColor = a == .cerrar ? .systemRed : .labelColor
                }
                b.toolTip = ReglasBarra.chapa(a)
                b.setAccessibilityLabel(ReglasBarra.titulo(a))
                b.target = self
                b.action = #selector(pulsado(_:))
                b.translatesAutoresizingMaskIntoConstraints = false
                b.widthAnchor.constraint(equalToConstant: 30).isActive = true
                b.heightAnchor.constraint(equalToConstant: 30).isActive = true
                b.alEntrar = { [weak self] boton in self?.ensenarChapa(de: boton) }
                b.alSalir = { [weak self] _ in self?.chapa.orderOut(nil) }
                if a == .sonido { b.alRueda = { [weak self] d in self?.alVolumen?(d) } }
                pila.addArrangedSubview(b)
                botones[a] = b
            case let .rotulo(r):
                let t = NSTextField(labelWithString: r == .enVivo ? "● EN VIVO" : "")
                t.font = r == .enVivo ? .systemFont(ofSize: 11, weight: .bold)
                    : .monospacedDigitSystemFont(ofSize: 11, weight: .medium)
                t.textColor = r == .enVivo ? .systemRed : .labelColor
                t.alignment = .center
                t.translatesAutoresizingMaskIntoConstraints = false
                t.widthAnchor.constraint(greaterThanOrEqualToConstant: r == .enVivo ? 70 : 44).isActive = true
                pila.addArrangedSubview(t)
                rotulos[r] = t
            case .recorrido:
                let s = RecorridoBarra(value: 0, minValue: 0, maxValue: 1, target: self, action: #selector(saltado(_:)))
                s.isContinuous = true
                s.controlSize = .small
                s.translatesAutoresizingMaskIntoConstraints = false
                s.widthAnchor.constraint(equalToConstant: 120).isActive = true
                pila.addArrangedSubview(s)
                recorrido = s
            case .separador:
                let l = NSBox()
                l.boxType = .separator
                l.translatesAutoresizingMaskIntoConstraints = false
                l.heightAnchor.constraint(equalToConstant: 20).isActive = true
                l.widthAnchor.constraint(equalToConstant: 1).isActive = true
                pila.addArrangedSubview(l)
            }
        }
        pila.layoutSubtreeIfNeeded()
        let ancho = max(60, pila.fittingSize.width)
        panel.setContentSize(CGSize(width: ancho, height: ReglasBarra.alto))
    }

    func poner(_ rotulo: RotuloBarra, _ texto: String) {
        rotulos[rotulo]?.stringValue = texto
    }

    /// Cambia el simbolo de un boton de dos estados (reproducir, sonido,
    /// manejar) o marca la herramienta o el color elegidos.
    func marcar(_ accion: AccionBarraPin, activo: Bool) {
        guard let b = botones[accion] else { return }
        switch accion {
        case .herramienta, .color:
            b.wantsLayer = true
            b.layer?.cornerRadius = 6
            b.layer?.backgroundColor = activo ? NSColor.controlAccentColor.withAlphaComponent(0.35).cgColor : nil
        case .manejar:
            b.image = NSImage(systemSymbolName: ReglasBarra.simbolo(accion, activo: activo), accessibilityDescription: nil)
            b.contentTintColor = activo ? .controlAccentColor : .labelColor
        default:
            b.image = NSImage(systemSymbolName: ReglasBarra.simbolo(accion, activo: activo), accessibilityDescription: nil)
        }
    }

    func poner(progreso: Double) {
        // Mientras se arrastra la linea de tiempo, manda la mano.
        guard let recorrido, NSEvent.pressedMouseButtons & 1 == 0 else { return }
        recorrido.doubleValue = progreso.isFinite ? ReglasPin.limitar(progreso, 0, 1) : 0
    }

    /// Ensena la barra junto al pin `padre`.
    func mostrar(junto padre: NSWindow) {
        colocar(junto: padre)
        if panel.parent !== padre {
            panel.parent?.removeChildWindow(panel)
            padre.addChildWindow(panel, ordered: .above)
        }
        panel.orderFront(nil)
    }

    func colocar(junto padre: NSWindow) {
        let visible = (padre.screen ?? NSScreen.main)?.visibleFrame ?? padre.frame.insetBy(dx: -2000, dy: -2000)
        panel.setFrame(ReglasBarra.marco(barra: panel.frame.size, pin: padre.frame, visible: visible), display: true)
    }

    func esconder() {
        chapa.orderOut(nil)
        panel.parent?.removeChildWindow(panel)
        panel.orderOut(nil)
    }

    var visible: Bool { panel.isVisible }

    func cerrar() {
        esconder()
        alPulsar = nil
        alSaltar = nil
        alVolumen = nil
        alEntrar = nil
        alSalir = nil
        panel.close()
        chapa.close()
    }

    /// Si el raton esta encima de la barra.
    var tieneElRaton: Bool { panel.isVisible && panel.frame.contains(NSEvent.mouseLocation) }

    private func ensenarChapa(de boton: BotonBarra) {
        textoChapa.stringValue = ReglasBarra.chapa(boton.accionBarra)
        let ancho = max(40, textoChapa.fittingSize.width + 14)
        let enPantalla = boton.window?.convertToScreen(boton.convert(boton.bounds, to: nil)) ?? .zero
        let barra = panel.frame
        // Debajo de la barra (o encima si la barra va debajo del pin).
        var y = barra.minY - ReglasBarra.altoChapa - 4
        if let padre = panel.parent, barra.minY < padre.frame.minY { y = barra.maxY + 4 }
        chapa.setFrame(CGRect(x: (enPantalla.midX - ancho / 2).rounded(), y: y.rounded(),
                              width: ancho, height: ReglasBarra.altoChapa), display: true)
        chapa.orderFront(nil)
    }

    @objc private func pulsado(_ sender: BotonBarra) {
        alPulsar?(sender.accionBarra)
    }

    @objc private func saltado(_ sender: NSSlider) {
        alSaltar?(sender.doubleValue)
    }
}
