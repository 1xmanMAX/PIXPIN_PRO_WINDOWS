// Un pin flotante: un `NSPanel` sin bordes, siempre encima, en todos los
// Espacios y sobre las apps a pantalla completa (lo que en Windows es
// `pixpin-pin/src/ventana.rs`).
//
// - Se mueve arrastrando: una foto o un pin en vivo, desde cualquier sitio;
//   un PDF, un video o una herramienta, desde su asa de arriba (el resto es
//   para seleccionar texto, los mandos o lo de dentro). Mientras se arrastra
//   salen guias azules con los bordes de los demas pines y al soltar se pega
//   a ellas (Opcion: sin guias).
// - Rueda o pellizco: zoom (en un PDF, Cmd + rueda; el pellizco lo hace el
//   propio PDFView). Mayusculas + rueda (como en Windows) u Opcion + rueda:
//   opacidad.
// - Al pasar el raton sale la barra comun (`BarraPin`): el control propio,
//   Copiar, Anotar, Mas y Cerrar.
// - Esc o doble clic (en el asa, si la tiene): cerrar. En un pin en vivo, el
//   doble clic pausa y reanuda.
// - Clic derecho: el menu.
//
// Rendimiento (§4bis del plan): el pin guarda solo la imagen ya reducida al
// tamano de la pantalla; la escala la hace Core Animation al cambiar el
// marco (no se redibuja nada a mano); Live Text se analiza solo la primera
// vez que el raton pasa por encima y se suelta con la presion de memoria;
// el reloj del video solo corre con la barra a la vista; al cerrar se
// suelta todo (el reproductor se vacia, la captura en vivo se para).

import AppKit
import AVFoundation
import AVKit
import PDFKit
import PixPinCore
import UniformTypeIdentifiers
import VisionKit

/// Lo que ensena un pin.
enum ContenidoPin {
    /// Una foto ya reducida; `fichero` si vino de un fichero (para guardar
    /// y copiar el original, no la reducida).
    case imagen(CGImage, fichero: URL?)
    case pdf(PDFDocument, fichero: URL)
    case video(AVPlayer, fichero: URL, soloAudio: Bool)
    /// Una zona de la pantalla en directo.
    case vivo(ZonaViva)
    /// Una nota o una mini-app.
    case herramienta(HerramientaPin)
}

/// El panel: solo reenvia los eventos al `Pin` antes que a las vistas.
final class PanelPin: NSPanel {
    weak var pin: Pin?

    // Sin bordes no podria ser clave, y sin serlo no llegan Esc ni la
    // seleccion de Live Text. `.nonactivatingPanel` hace que serlo no
    // active la app (no roba el foco a la ventana de debajo).
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    override func sendEvent(_ event: NSEvent) {
        if let pin, pin.atender(event) { return }
        super.sendEvent(event)
    }
}

/// La vista de fondo del pin: avisa al entrar y salir el raton.
final class VistaPin: NSView {
    var alEntrar: (@MainActor () -> Void)?
    var alSalir: (@MainActor () -> Void)?
    private var zona: NSTrackingArea?

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let zona { removeTrackingArea(zona) }
        let nueva = NSTrackingArea(rect: .zero,
                                   options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
                                   owner: self, userInfo: nil)
        addTrackingArea(nueva)
        zona = nueva
    }

    override func mouseEntered(with event: NSEvent) { alEntrar?() }
    override func mouseExited(with event: NSEvent) { alSalir?() }
}

@MainActor
final class Pin: NSObject {
    /// Uno para todos los pines, y solo si alguno llega a pedir Live Text.
    private static let analizador = ImageAnalyzer()
    private static let altoAsa: CGFloat = 22

    let id: Int
    let panel: PanelPin
    private(set) var contenido: ContenidoPin
    var alCerrar: (@MainActor (Pin) -> Void)?
    /// Algo que ensena el panel de pines abiertos cambio.
    var alCambiar: (@MainActor () -> Void)?
    /// El color de su grupo (0-7), o sin grupo.
    var grupo: Int? {
        didSet {
            pintarBorde()
            alCambiar?()
        }
    }
    /// Oculto con el ojo o con ⌃2 (sigue abierto).
    private(set) var oculto = false

    private let contenedor: VistaPin
    private var vistaImagen: NSImageView?
    private var vistaPDF: PDFView?
    private var vistaVivo: NSView?
    private var asa: NSView?
    private var superposicion: ImageAnalysisOverlayView?
    private var tareaAnalisis: Task<Void, Never>?
    private var aviso: NSView?
    private var barra: BarraPin?
    private var esconderBarra: Task<Void, Never>?
    private var relojVideo: Any?
    private var anotacion: VistaAnotacion?
    private var anotando = false
    private var avisoPagina: NSObjectProtocol?
    /// El tamano con el que nacio (o el del video al conocerlo) y cuanto se
    /// ha agrandado desde entonces.
    private var tamanoBase: CGSize
    private var escala: CGFloat = 1
    private var cerrado = false

    init(id: Int, contenido: ContenidoPin, marco: CGRect) {
        self.id = id
        self.contenido = contenido
        tamanoBase = marco.size
        panel = PanelPin(contentRect: marco, styleMask: [.borderless, .nonactivatingPanel],
                         backing: .buffered, defer: false)
        contenedor = VistaPin(frame: CGRect(origin: .zero, size: marco.size))
        super.init()
        configurarPanel()
        montarContenido()
    }

    private var soloAudio: Bool {
        if case let .video(_, _, audio) = contenido { return audio }
        return false
    }

    private var fichero: URL? {
        switch contenido {
        case let .imagen(_, url): return url
        case let .pdf(_, url): return url
        case let .video(_, url, _): return url
        case .vivo, .herramienta: return nil
        }
    }

    private var esImagen: Bool {
        if case .imagen = contenido { return true }
        return false
    }

    private var zonaViva: ZonaViva? {
        if case let .vivo(z) = contenido { return z }
        return nil
    }

    // MARK: Para el panel de pines abiertos

    var fila: FilaPin {
        let opacidad = ReglasPin.porcentaje(panel.alphaValue)
        let conOpacidad: (String) -> String = { opacidad < 100 ? "\($0) · opacidad \(opacidad) %" : $0 }
        let tipo: TipoFilaPin
        let nombre: String
        let detalle: String
        switch contenido {
        case let .imagen(_, url):
            tipo = .foto
            nombre = url?.lastPathComponent ?? "Captura"
            detalle = conOpacidad("Foto")
        case let .pdf(doc, url):
            tipo = .pdf
            nombre = url.lastPathComponent
            let actual = vistaPDF?.currentPage.map { doc.index(for: $0) + 1 } ?? 1
            detalle = "PDF · \(actual)/\(doc.pageCount)"
        case let .video(_, url, audio):
            tipo = audio ? .audio : .video
            nombre = url.lastPathComponent
            detalle = conOpacidad(audio ? "Audio" : "Vídeo")
        case let .vivo(z):
            tipo = .vivo
            nombre = "Pin en vivo"
            detalle = z.pausado ? "En pausa" : "En vivo"
        case let .herramienta(h):
            if case .nota = h.clase { tipo = .nota } else { tipo = .herramienta }
            nombre = h.nombre
            detalle = h.detalle
        }
        return FilaPin(id: id, nombre: nombre, detalle: detalle, tipo: tipo, grupo: grupo, oculto: oculto)
    }

    // MARK: Montar

    private func configurarPanel() {
        panel.pin = self
        panel.isFloatingPanel = true
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        // NSPanel se esconde por omision al desactivarse la app; un pin no.
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.becomesKeyOnlyIfNeeded = false
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = true
        panel.isMovableByWindowBackground = false
        panel.animationBehavior = .utilityWindow

        // Borde fino y esquinas apenas redondeadas; la sombra la pone el panel.
        contenedor.wantsLayer = true
        contenedor.layer?.cornerRadius = 4
        contenedor.layer?.masksToBounds = true
        contenedor.autoresizingMask = [.width, .height]
        contenedor.alEntrar = { [weak self] in self?.alEntrarRaton() }
        contenedor.alSalir = { [weak self] in self?.alSalirRaton() }
        panel.contentView = contenedor
        pintarBorde()
    }

    /// El borde: fino y oscuro, o del color de su grupo.
    private func pintarBorde() {
        if let grupo {
            contenedor.layer?.borderWidth = 2
            contenedor.layer?.borderColor = ColoresGrupo.nsColor(grupo).cgColor
        } else {
            contenedor.layer?.borderWidth = 0.5
            contenedor.layer?.borderColor = NSColor.black.withAlphaComponent(0.25).cgColor
        }
    }

    private func montarContenido() {
        let todo = contenedor.bounds
        switch contenido {
        case let .imagen(imagen, _):
            let vista = NSImageView(frame: todo)
            vista.image = NSImage(cgImage: imagen, size: .zero)
            vista.imageScaling = .scaleProportionallyUpOrDown
            vista.imageFrameStyle = .none
            vista.isEditable = false
            vista.allowsCutCopyPaste = false
            vista.animates = false
            vista.autoresizingMask = [.width, .height]
            contenedor.addSubview(vista)
            vistaImagen = vista

        case let .pdf(documento, _):
            let vista = PDFView(frame: todo)
            vista.document = documento
            vista.displayMode = .singlePageContinuous
            vista.autoScales = true
            vista.backgroundColor = .windowBackgroundColor
            vista.autoresizingMask = [.width, .height]
            contenedor.addSubview(vista)
            vistaPDF = vista
            // Un aviso del propio PDFView, no un sondeo.
            avisoPagina = NotificationCenter.default.addObserver(forName: .PDFViewPageChanged, object: vista,
                                                                 queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.ponerPagina() }
            }
            montarAsa()

        case let .video(reproductor, _, audio):
            var marco = todo
            if audio { marco.size.height = max(1, marco.height - Pin.altoAsa) }
            let vista = AVPlayerView(frame: marco)
            vista.player = reproductor
            // Los mandos del video van en la barra comun; el audio se queda
            // con los suyos (es casi todo el pin).
            vista.controlsStyle = audio ? .inline : .none
            vista.showsFullScreenToggleButton = false
            vista.videoGravity = .resizeAspect
            vista.autoresizingMask = [.width, .height]
            contenedor.addSubview(vista)
            montarAsa()

        case let .vivo(zona):
            // Una vista que aloja la capa: la captura pone ahi sus
            // `IOSurface` y Core Animation la estira al tamano del pin.
            let vista = NSView(frame: todo)
            vista.layer = zona.capa
            vista.wantsLayer = true
            vista.autoresizingMask = [.width, .height]
            contenedor.addSubview(vista)
            vistaVivo = vista

        case let .herramienta(h):
            let vista = h.crearVista()
            vista.frame = todo
            vista.autoresizingMask = [.width, .height]
            contenedor.addSubview(vista)
            h.alCambiar = { [weak self] in self?.alCambiar?() }
            montarAsa()
        }
    }

    /// La franja de arriba para mover un PDF, un video o una herramienta. Se
    /// ve al pasar el raton (en un audio, siempre: es casi todo el pin).
    private func montarAsa() {
        let b = contenedor.bounds
        let asa = NSVisualEffectView(frame: CGRect(x: 0, y: b.height - Pin.altoAsa,
                                                   width: b.width, height: Pin.altoAsa))
        asa.material = .hudWindow
        asa.blendingMode = .withinWindow
        asa.state = .active
        asa.autoresizingMask = [.width, .minYMargin]
        if let simbolo = NSImage(systemSymbolName: "line.3.horizontal", accessibilityDescription: "Mover") {
            let icono = NSImageView(image: simbolo)
            icono.frame = asa.bounds
            icono.imageScaling = .scaleNone
            icono.contentTintColor = .secondaryLabelColor
            icono.autoresizingMask = [.width, .height]
            asa.addSubview(icono)
        }
        let siempre: Bool
        if case .herramienta = contenido { siempre = true } else { siempre = soloAudio }
        asa.alphaValue = siempre ? 1 : 0
        contenedor.addSubview(asa)
        self.asa = asa
    }

    func mostrar() {
        panel.orderFrontRegardless()
        panel.makeKey()
        if case let .video(reproductor, _, audio) = contenido {
            reproductor.play()
            if !audio { ajustarAlVideo(reproductor) }
        }
        if let zona = zonaViva {
            zona.fuente.alPararse = { [weak self] in self?.vivoParado() }
            arrancarVivo(zona)
        }
    }

    /// El video nace con 16:9; al saber su tamano de verdad se ajusta (sin
    /// moverlo de sitio).
    private func ajustarAlVideo(_ reproductor: AVPlayer) {
        guard let asset = reproductor.currentItem?.asset else { return }
        Task { [weak self] in
            guard let pista = try? await asset.loadTracks(withMediaType: .video).first,
                  let datos = try? await pista.load(.naturalSize, .preferredTransform)
            else { return }
            let natural = CGRect(origin: .zero, size: datos.0).applying(datos.1).size
            guard let self, !self.cerrado, natural.width > 0, natural.height > 0 else { return }
            let visible = (self.panel.screen ?? NSScreen.main)?.visibleFrame.size
                ?? CGSize(width: 1440, height: 900)
            let tam = ReglasPin.tamanoInicial(pixeles: CGSize(width: abs(natural.width), height: abs(natural.height)),
                                              escala: 1, visible: visible)
            let marco = self.panel.frame
            self.tamanoBase = tam
            self.escala = 1
            self.panel.setFrame(CGRect(x: (marco.midX - tam.width / 2).rounded(),
                                       y: (marco.midY - tam.height / 2).rounded(),
                                       width: tam.width, height: tam.height), display: true)
            self.barra?.colocar(junto: self.panel)
        }
    }

    // MARK: Eventos

    /// `true` si el pin se queda el evento (no sigue a las vistas).
    func atender(_ evento: NSEvent) -> Bool {
        guard !cerrado else { return false }
        switch evento.type {
        case .keyDown:
            return tecla(evento)

        case .rightMouseDown:
            if let zona = zonaViva, zona.manejando {
                manejarClic(evento, boton: .right)
                return true
            }
            mostrarMenu(evento)
            return true

        case .leftMouseDown:
            if evento.modifierFlags.contains(.control) {
                mostrarMenu(evento)
                return true
            }
            if anotando { return false }
            let punto = contenedor.convert(evento.locationInWindow, from: nil)
            let enAsa = asa.map { $0.frame.contains(punto) } ?? false
            switch contenido {
            case .imagen:
                // Sobre un texto de Live Text, el clic es para seleccionarlo.
                if let capa = superposicion, capa.analysis != nil,
                   capa.hasInteractiveItem(at: capa.convert(evento.locationInWindow, from: nil)) {
                    return false
                }
            case let .vivo(zona):
                // Manejando, el clic va a la zona; Cmd + arrastrar mueve el pin.
                if zona.manejando, !evento.modifierFlags.contains(.command) {
                    manejarClic(evento, boton: .left)
                    return true
                }
                if evento.clickCount >= 2 {
                    alternarPausa()
                    return true
                }
            default:
                guard enAsa else { return false }
            }
            if evento.clickCount >= 2 {
                cerrar()
            } else {
                panel.makeKey()
                arrastrar(desde: evento)
            }
            return true

        case .scrollWheel:
            let mods = evento.modifierFlags
            if let zona = zonaViva, zona.manejando, !mods.contains(.command) {
                manejarRueda(evento, zona: zona)
                return true
            }
            let opacidad = mods.contains(.option) || mods.contains(.shift)
            // En un PDF, la rueda sola pasa paginas (con su inercia); en una
            // herramienta, desplaza lo de dentro.
            if case .pdf = contenido, !opacidad, !mods.contains(.command) { return false }
            if case .herramienta = contenido, !opacidad, !mods.contains(.command) { return false }
            // La inercia del trackpad no cuenta: el pin se queda donde se solto.
            if !evento.momentumPhase.isEmpty { return true }
            if opacidad {
                // Con Mayusculas, una rueda de raton manda el giro en X.
                let delta = ReglasPin.deltaDeRueda(x: evento.scrollingDeltaX, y: evento.scrollingDeltaY)
                panel.alphaValue = ReglasPin.opacidadTrasRueda(panel.alphaValue, deltaY: delta,
                                                               precisa: evento.hasPreciseScrollingDeltas)
                alCambiar?()
                return true
            }
            zoom(por: ReglasPin.factorZoom(deltaY: evento.scrollingDeltaY, precisa: evento.hasPreciseScrollingDeltas),
                 ancla: NSEvent.mouseLocation)
            return true

        case .magnify:
            if case .pdf = contenido { return false }
            if case .herramienta = contenido { return false }
            zoom(por: 1 + evento.magnification, ancla: NSEvent.mouseLocation)
            return true

        default:
            return false
        }
    }

    /// El teclado del pin. Lo que se escribe dentro (una nota, una tarea, el
    /// texto de anotar) no se toca.
    private func tecla(_ e: NSEvent) -> Bool {
        let escribiendo = panel.firstResponder is NSText || (anotacion?.escribiendo ?? false)
        let mods = e.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let letra = (e.charactersIgnoringModifiers ?? "").lowercased()
        if e.keyCode == 53 { // Esc
            if anotacion?.escribiendo == true {
                anotacion?.terminarTexto()
            } else if anotando {
                terminarAnotar()
            } else if escribiendo {
                panel.makeFirstResponder(nil)
            } else {
                cerrar()
            }
            return true
        }
        if anotando {
            guard !escribiendo else { return false }
            if mods.contains(.command), letra == "z" {
                anotacion?.deshacer()
                return true
            }
            if e.keyCode == 36 || e.keyCode == 76 {
                fijarAnotacion()
                return true
            }
            if let h = HerramientaAnotacion.allCases.first(where: { $0.tecla == letra }) {
                elegir(h)
                return true
            }
            return false
        }
        guard !escribiendo else { return false }
        if mods.contains(.command) {
            switch letra {
            case "c": accion(.copiar)
            case "0": tamanoOriginal()
            case "=", "+": zoom(por: 1.25, ancla: CGPoint(x: panel.frame.midX, y: panel.frame.midY))
            case "-": zoom(por: 0.8, ancla: CGPoint(x: panel.frame.midX, y: panel.frame.midY))
            default: return false
            }
            return true
        }
        guard mods.isDisjoint(with: [.control, .option]) else { return false }
        if case .herramienta = contenido { return false }
        switch e.keyCode {
        case 123, 126: // izquierda, arriba
            guard vistaPDF != nil else { return false }
            accion(.paginaAnterior)
            return true
        case 124, 125: // derecha, abajo
            guard vistaPDF != nil else { return false }
            accion(.paginaSiguiente)
            return true
        case 49: // espacio
            if case .video = contenido { accion(.reproducir); return true }
            if zonaViva != nil { accion(.reproducir); return true }
            return false
        default:
            break
        }
        switch letra {
        case "a" where esImagen:
            accion(.anotar)
            return true
        case "m":
            if case .video = contenido { accion(.sonido); return true }
            return false
        default:
            return false
        }
    }

    private func zoom(por factor: CGFloat, ancla: CGPoint) {
        let nueva = ReglasPin.escalaLimitada(escala * factor, base: tamanoBase)
        guard abs(nueva - escala) > 0.0001 else { return }
        escala = nueva
        let tamano = CGSize(width: tamanoBase.width * nueva, height: tamanoBase.height * nueva)
        panel.setFrame(ReglasPin.marcoTrasZoom(panel.frame, ancla: ancla, tamano: tamano), display: true)
        barra?.colocar(junto: panel)
        ponerZoom()
    }

    /// Un pixel de la foto (o de la zona) por pixel de la pantalla.
    private func tamanoOriginal() {
        let natural: CGSize
        switch contenido {
        case let .imagen(imagen, _):
            let s = max(panel.backingScaleFactor, 1)
            natural = CGSize(width: CGFloat(imagen.width) / s, height: CGFloat(imagen.height) / s)
        case let .vivo(z):
            natural = z.zona.size
        default:
            natural = tamanoBase
        }
        guard natural.width > 0, tamanoBase.width > 0 else { return }
        zoom(por: (natural.width / tamanoBase.width) / escala,
             ancla: CGPoint(x: panel.frame.midX, y: panel.frame.midY))
    }

    /// Mover el pin a mano: con guias azules hacia los bordes de los demas
    /// y, al soltar, el iman suave hacia la guia (Opcion: sin guias).
    private func arrastrar(desde evento: NSEvent) {
        let inicioRaton = NSEvent.mouseLocation
        let inicioMarco = panel.frame
        let otros = GestorPines.compartido.marcosVisibles(salvo: self)
        var ajuste: AjusteGuias?
        var movido = false
        let lineas = LineasGuia.compartidas
        while let e = panel.nextEvent(matching: [.leftMouseDragged, .leftMouseUp, .flagsChanged]) {
            if e.type == .leftMouseUp { break }
            let raton = NSEvent.mouseLocation
            if !movido, ReglasVivo.fueClic(inicioRaton, raton) { continue }
            movido = true
            let marco = inicioMarco.offsetBy(dx: (raton.x - inicioRaton.x).rounded(),
                                             dy: (raton.y - inicioRaton.y).rounded())
            panel.setFrameOrigin(marco.origin)
            if NSEvent.modifierFlags.contains(.option) || otros.isEmpty {
                ajuste = nil
                lineas.esconder()
                continue
            }
            let a = ReglasGuias.alinear(marco, otros: otros)
            ajuste = a
            lineas.ensenar(a.guias)
        }
        lineas.esconder()
        if let a = ajuste, !a.guias.isEmpty, a.rect.origin != panel.frame.origin {
            panel.setFrame(CGRect(origin: a.rect.origin, size: panel.frame.size), display: true, animate: true)
        }
        if movido { barra?.colocar(junto: panel) }
    }

    // MARK: Barra

    private var tipoBarra: TipoBarraPin {
        let propio: PropioBarra
        switch contenido {
        case .imagen: propio = .zoom
        case let .pdf(doc, _): propio = .paginas(conPaso: doc.pageCount > 1)
        case .video: propio = .video
        case .vivo: propio = .vivo
        case .herramienta: propio = .ninguno
        }
        return TipoBarraPin(propio: propio, copiable: !soloAudio, anotable: esImagen, anotando: anotando)
    }

    private func alEntrarRaton() {
        prepararTextoVivo()
        if let asa, !soloAudio { asa.animator().alphaValue = 1 }
        mostrarBarra()
    }

    private func alSalirRaton() {
        if let asa, !soloAudio {
            if case .herramienta = contenido {} else { asa.animator().alphaValue = 0 }
        }
        programarEsconderBarra()
    }

    private func mostrarBarra() {
        esconderBarra?.cancel()
        esconderBarra = nil
        guard !cerrado, !oculto else { return }
        let b: BarraPin
        if let ya = barra {
            b = ya
        } else {
            b = BarraPin()
            b.alPulsar = { [weak self] a in self?.accion(a) }
            b.alSaltar = { [weak self] f in self?.saltarVideo(a: f) }
            b.alVolumen = { [weak self] d in self?.cambiarVolumen(d) }
            b.alEntrar = { [weak self] in
                self?.esconderBarra?.cancel()
                self?.esconderBarra = nil
            }
            b.alSalir = { [weak self] in self?.programarEsconderBarra() }
            barra = b
        }
        b.montar(ReglasBarra.piezas(tipoBarra))
        refrescarBarra()
        b.mostrar(junto: panel)
        empezarRelojVideo()
    }

    private func programarEsconderBarra() {
        guard barra?.visible == true, !anotando else { return }
        esconderBarra?.cancel()
        esconderBarra = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(450))
            guard !Task.isCancelled, let self else { return }
            if self.barra?.tieneElRaton == true || self.panel.frame.contains(NSEvent.mouseLocation) { return }
            self.barra?.esconder()
            self.pararRelojVideo()
        }
    }

    /// Pone en la barra lo de ahora (zoom, pagina, tiempo, estados).
    private func refrescarBarra() {
        guard let barra else { return }
        ponerZoom()
        ponerPagina()
        switch contenido {
        case let .video(reproductor, _, _):
            barra.marcar(.reproducir, activo: reproductor.rate != 0)
            barra.marcar(.sonido, activo: reproductor.isMuted)
            ponerTiempo(reproductor)
        case let .vivo(z):
            barra.marcar(.reproducir, activo: !z.pausado)
            barra.marcar(.manejar, activo: z.manejando)
            barra.poner(.enVivo, z.pausado ? "❚❚ EN PAUSA" : "● EN VIVO")
        default:
            break
        }
        if anotando, let a = anotacion {
            for h in HerramientaAnotacion.allCases { barra.marcar(.herramienta(h), activo: h == a.herramienta) }
            for i in ReglasAnotacion.colores.indices { barra.marcar(.color(i), activo: i == a.color) }
        }
    }

    private func ponerZoom() {
        let porcentaje: CGFloat
        switch contenido {
        case let .imagen(imagen, _):
            porcentaje = panel.frame.width * max(panel.backingScaleFactor, 1) / CGFloat(max(imagen.width, 1)) * 100
        case let .vivo(z):
            porcentaje = panel.frame.width / max(z.zona.width, 1) * 100
        default:
            porcentaje = escala * 100
        }
        barra?.poner(.zoom, ReglasBarra.textoZoom(Int(porcentaje.rounded())))
    }

    private func ponerPagina() {
        guard case let .pdf(doc, _) = contenido, let vista = vistaPDF else { return }
        let actual = vista.currentPage.map { doc.index(for: $0) + 1 } ?? 1
        barra?.poner(.pagina, "\(actual) / \(doc.pageCount)")
    }

    private func ponerTiempo(_ reproductor: AVPlayer) {
        let ahora = reproductor.currentTime().seconds
        let total = reproductor.currentItem?.duration.seconds ?? 0
        barra?.poner(.tiempo, ReglasBarra.textoTiempo(ahora))
        barra?.poner(.duracion, ReglasBarra.textoTiempo(total))
        barra?.poner(progreso: total > 0 && total.isFinite ? ahora / total : 0)
    }

    /// El reloj del video, solo con la barra a la vista.
    private func empezarRelojVideo() {
        guard relojVideo == nil, case let .video(reproductor, _, _) = contenido else { return }
        relojVideo = reproductor.addPeriodicTimeObserver(forInterval: CMTime(value: 1, timescale: 2),
                                                         queue: .main) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, case let .video(r, _, _) = self.contenido else { return }
                self.ponerTiempo(r)
                self.barra?.marcar(.reproducir, activo: r.rate != 0)
            }
        }
    }

    private func pararRelojVideo() {
        guard let reloj = relojVideo, case let .video(reproductor, _, _) = contenido else { return }
        reproductor.removeTimeObserver(reloj)
        relojVideo = nil
    }

    private func saltarVideo(a fraccion: Double) {
        guard case let .video(r, _, _) = contenido, let total = r.currentItem?.duration.seconds,
              total.isFinite, total > 0 else { return }
        r.seek(to: CMTime(seconds: fraccion * total, preferredTimescale: 600),
               toleranceBefore: .zero, toleranceAfter: .zero)
        barra?.poner(.tiempo, ReglasBarra.textoTiempo(fraccion * total))
    }

    private func cambiarVolumen(_ delta: CGFloat) {
        guard case let .video(r, _, _) = contenido else { return }
        r.volume = Float(ReglasPin.limitar(CGFloat(r.volume) + (delta > 0 ? 0.05 : -0.05), 0, 1))
        if r.volume > 0 { r.isMuted = false }
        barra?.marcar(.sonido, activo: r.isMuted || r.volume == 0)
    }

    /// Lo que pide un boton de la barra (o su atajo).
    func accion(_ a: AccionBarraPin) {
        switch a {
        case .alejar: zoom(por: 0.8, ancla: CGPoint(x: panel.frame.midX, y: panel.frame.midY))
        case .acercar: zoom(por: 1.25, ancla: CGPoint(x: panel.frame.midX, y: panel.frame.midY))
        case .paginaAnterior:
            vistaPDF?.goToPreviousPage(nil)
            ponerPagina()
        case .paginaSiguiente:
            vistaPDF?.goToNextPage(nil)
            ponerPagina()
        case .reproducir:
            if case let .video(r, _, _) = contenido {
                if r.rate != 0 { r.pause() } else { r.play() }
                barra?.marcar(.reproducir, activo: r.rate != 0)
            } else if zonaViva != nil {
                alternarPausa()
            }
        case .sonido:
            if case let .video(r, _, _) = contenido {
                r.isMuted.toggle()
                barra?.marcar(.sonido, activo: r.isMuted)
            }
        case .manejar: alternarManejar()
        case .congelar: congelar()
        case .copiar: copiar(nil)
        case .anotar: empezarAnotar()
        case .mas:
            // Bajo la esquina de arriba del pin (el clic fue en la barra).
            construirMenu().popUp(positioning: nil, at: CGPoint(x: 0, y: contenedor.bounds.maxY), in: contenedor)
        case .cerrar: cerrar()
        case let .herramienta(h): elegir(h)
        case let .color(i):
            anotacion?.color = i
            refrescarBarra()
        case .deshacer: anotacion?.deshacer()
        case .fijar: fijarAnotacion()
        case .listo: terminarAnotar()
        }
    }

    // MARK: Pin en vivo

    private func arrancarVivo(_ zona: ZonaViva) {
        Task { [weak self] in
            do {
                try await zona.arrancar()
            } catch {
                guard let self, !self.cerrado else { return }
                PermisoCaptura.avisarDenegado(detalle: "No se pudo ver la zona en vivo: \(error.localizedDescription)")
                self.cerrar()
            }
        }
    }

    private func alternarPausa() {
        guard let zona = zonaViva else { return }
        Task { [weak self] in
            if zona.pausado {
                try? await zona.reanudar()
            } else {
                await zona.pausar()
            }
            self?.refrescarBarra()
            self?.alCambiar?()
        }
    }

    /// La captura se paro sola: se queda en pausa con lo ultimo que vio.
    private func vivoParado() {
        guard let zona = zonaViva, !cerrado else { return }
        Task { [weak self] in
            await zona.pausar()
            self?.refrescarBarra()
            self?.alCambiar?()
        }
    }

    private func alternarManejar() {
        guard let zona = zonaViva else { return }
        if !zona.manejando, !ControlRemoto.pedirPermiso() { return }
        zona.manejando.toggle()
        refrescarBarra()
        avisar(zona.manejando ? "Manejando la zona: ⌘ + arrastrar mueve el pin" : "Ya no se maneja la zona")
    }

    /// Congela: el fotograma de ahora como pin de foto normal, en el mismo
    /// sitio, y este se cierra.
    private func congelar() {
        guard let zona = zonaViva, let foto = zona.fuente.fotograma() else { return NSSound.beep() }
        let marco = panel.frame
        GestorPines.compartido.pinear(imagen: foto, marco: marco)
        cerrar()
    }

    /// Un clic (o un arrastre) sobre el pin, mandado a la zona.
    private func manejarClic(_ evento: NSEvent, boton: CGMouseButton) {
        guard let zona = zonaViva, let vista = vistaVivo else { return }
        let tam = vista.bounds.size
        let volver = ControlRemoto.cursorQuartz()
        let enVista = vista.convert(evento.locationInWindow, from: nil)
        guard let destino = zona.puntoQuartz(enVista, contenido: tam) else { return }
        let (bajar, mover, subir): (CGEventType, CGEventType, CGEventType) =
            boton == .right ? (.rightMouseDown, .rightMouseDragged, .rightMouseUp)
            : (.leftMouseDown, .leftMouseDragged, .leftMouseUp)
        let clics = evento.clickCount
        ControlRemoto.raton(bajar, en: destino, boton: boton, clics: clics, volver: volver)
        let inicio = NSEvent.mouseLocation
        var ultimo = destino
        let mascara: NSEvent.EventTypeMask = boton == .right ? [.rightMouseDragged, .rightMouseUp]
            : [.leftMouseDragged, .leftMouseUp]
        while let e = panel.nextEvent(matching: mascara) {
            if e.type == .leftMouseUp || e.type == .rightMouseUp { break }
            // Un temblor no es un arrastre.
            if ReglasVivo.fueClic(inicio, NSEvent.mouseLocation) { continue }
            let p = vista.convert(e.locationInWindow, from: nil)
            if let q = zona.puntoQuartz(p, contenido: tam) {
                ultimo = q
                ControlRemoto.raton(mover, en: q, boton: boton, clics: 1, volver: ControlRemoto.cursorQuartz())
            }
        }
        ControlRemoto.raton(subir, en: ultimo, boton: boton, clics: clics, volver: ControlRemoto.cursorQuartz())
    }

    private func manejarRueda(_ evento: NSEvent, zona: ZonaViva) {
        guard let vista = vistaVivo else { return }
        let p = vista.convert(evento.locationInWindow, from: nil)
        guard let q = zona.puntoQuartz(p, contenido: vista.bounds.size) else { return }
        ControlRemoto.rueda(dy: evento.scrollingDeltaY, dx: evento.scrollingDeltaX, en: q,
                            volver: ControlRemoto.cursorQuartz())
    }

    // MARK: Anotar

    private func empezarAnotar() {
        guard esImagen, let vista = vistaImagen else { return }
        let capa: VistaAnotacion
        if let ya = anotacion {
            capa = ya
        } else {
            capa = VistaAnotacion(frame: vista.bounds)
            capa.autoresizingMask = [.width, .height]
            vista.addSubview(capa)
            anotacion = capa
        }
        anotando = true
        capa.editable = true
        // Live Text no puede quedarse con los clics mientras se anota.
        superposicion?.isHidden = true
        panel.makeKey()
        mostrarBarra()
    }

    private func elegir(_ h: HerramientaAnotacion) {
        anotacion?.terminarTexto()
        anotacion?.herramienta = h
        anotacion?.editable = true
        refrescarBarra()
    }

    /// Sale de anotar: lo dibujado se queda encima como capa.
    private func terminarAnotar() {
        anotacion?.terminarTexto()
        anotacion?.editable = false
        anotando = false
        superposicion?.isHidden = false
        mostrarBarra()
    }

    /// Quema lo dibujado en la foto (una vez, a su tamano) y quita la capa.
    private func fijarAnotacion() {
        guard case let .imagen(imagen, _) = contenido, let capa = anotacion else { return }
        capa.terminarTexto()
        guard !capa.trazos.isEmpty else { return terminarAnotar() }
        guard let nueva = ReglasAnotacion.quemar(capa.trazos, en: imagen) else { return NSSound.beep() }
        // Ya no es el fichero de antes: copiar y guardar dan la anotada.
        contenido = .imagen(nueva, fichero: nil)
        vistaImagen?.image = NSImage(cgImage: nueva, size: .zero)
        capa.removeFromSuperview()
        anotacion = nil
        // El texto reconocido era de la foto sin anotar.
        aligerar()
        terminarAnotar()
    }

    // MARK: Live Text

    /// Analiza la foto la primera vez que hace falta (raton encima) y pone la
    /// capa de Live Text encima: el texto se selecciona sobre el propio pin.
    private func prepararTextoVivo() {
        guard case let .imagen(imagen, _) = contenido, let vista = vistaImagen, !anotando,
              superposicion?.analysis == nil, tareaAnalisis == nil, ImageAnalyzer.isSupported
        else { return }
        tareaAnalisis = Task { [weak self] in
            let ajustes = ImageAnalyzer.Configuration([.text])
            let analisis = try? await Pin.analizador.analyze(imagen, orientation: .up, configuration: ajustes)
            guard let self else { return }
            let cancelada = Task.isCancelled
            self.tareaAnalisis = nil
            guard let analisis, !cancelada, !self.cerrado else { return }
            let capa: ImageAnalysisOverlayView
            if let ya = self.superposicion {
                capa = ya
            } else {
                capa = ImageAnalysisOverlayView()
                capa.frame = vista.bounds
                capa.autoresizingMask = [.width, .height]
                capa.preferredInteractionTypes = .textSelection
                capa.trackingImageView = vista
                // Debajo de lo anotado.
                vista.addSubview(capa, positioned: .below, relativeTo: self.anotacion)
                self.superposicion = capa
            }
            capa.analysis = analisis
        }
    }

    /// Con presion de memoria: fuera el analisis (se rehace si se vuelve a
    /// pasar el raton).
    func aligerar() {
        tareaAnalisis?.cancel()
        tareaAnalisis = nil
        superposicion?.analysis = nil
    }

    // MARK: Ocultar y resaltar

    /// Lo esconde (sigue abierto: el panel de pines abiertos lo devuelve).
    func ocultar(_ si: Bool) {
        guard si != oculto, !cerrado else { return }
        oculto = si
        if si {
            barra?.esconder()
            pararRelojVideo()
            panel.orderOut(nil)
        } else {
            panel.orderFrontRegardless()
        }
        if let zona = zonaViva { Task { await zona.ocultar(si) } }
        alCambiar?()
    }

    /// Lo trae a la vista y lo hace parpadear un momento (desde el panel).
    func resaltar() {
        ocultar(false)
        panel.orderFrontRegardless()
        guard let capa = contenedor.layer else { return }
        let animacion = CABasicAnimation(keyPath: "borderWidth")
        animacion.fromValue = 6
        animacion.toValue = capa.borderWidth
        animacion.duration = 0.6
        animacion.repeatCount = 2
        let color = CABasicAnimation(keyPath: "borderColor")
        color.fromValue = LineasGuia.azul.cgColor
        color.toValue = capa.borderColor
        color.duration = 0.6
        color.repeatCount = 2
        capa.add(animacion, forKey: "resaltar")
        capa.add(color, forKey: "resaltarColor")
    }

    // MARK: Menu

    private func mostrarMenu(_ evento: NSEvent) {
        NSMenu.popUpContextMenu(construirMenu(), with: evento, for: contenedor)
    }

    /// El menu del pin, en el mismo orden que la barra.
    private func construirMenu() -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false
        menu.addItem(elemento("Copiar", #selector(copiar(_:)), tecla: "c"))
        if fichero != nil || esImagen {
            menu.addItem(elemento("Guardar como…", #selector(guardarComo(_:))))
        }
        if esImagen {
            menu.addItem(elemento("Anotar", #selector(anotarDesdeMenu(_:)), tecla: "a", conMando: false))
        }
        if let zona = zonaViva {
            menu.addItem(.separator())
            menu.addItem(elemento(zona.pausado ? "Reanudar" : "Pausar", #selector(pausaDesdeMenu(_:))))
            let manejar = elemento("Manejar la zona", #selector(manejarDesdeMenu(_:)))
            manejar.state = zona.manejando ? .on : .off
            menu.addItem(manejar)
            menu.addItem(elemento("Congelar", #selector(congelarDesdeMenu(_:))))
        }
        if case let .herramienta(h) = contenido, case .nota = h.clase {
            menu.addItem(elemento("Guardar en «Mensajes guardados»", #selector(notaAlChat(_:))))
        }
        menu.addItem(elemento("Tamaño original", #selector(tamanoOriginalDesdeMenu(_:)), tecla: "0"))
        menu.addItem(.separator())

        let opacidad = NSMenuItem(title: "Opacidad", action: nil, keyEquivalent: "")
        let submenu = NSMenu()
        submenu.autoenablesItems = false
        let actual = ReglasPin.porcentaje(panel.alphaValue)
        for p in ReglasPin.opacidades {
            let item = elemento("\(p) %", #selector(elegirOpacidad(_:)))
            item.tag = p
            item.state = p == actual ? .on : .off
            submenu.addItem(item)
        }
        opacidad.submenu = submenu
        menu.addItem(opacidad)

        let grupoItem = NSMenuItem(title: "Grupo", action: nil, keyEquivalent: "")
        let colores = NSMenu()
        colores.autoenablesItems = false
        for (i, nombre) in ColoresGrupo.nombres.enumerated() {
            let item = elemento(nombre, #selector(elegirGrupo(_:)))
            item.tag = i
            item.state = grupo == i ? .on : .off
            item.image = Pin.muestra(ColoresGrupo.nsColor(i))
            colores.addItem(item)
        }
        colores.addItem(.separator())
        let sin = elemento("Sin grupo", #selector(elegirGrupo(_:)))
        sin.tag = -1
        sin.state = grupo == nil ? .on : .off
        colores.addItem(sin)
        grupoItem.submenu = colores
        menu.addItem(grupoItem)

        let encima = elemento("Siempre encima", #selector(alternarEncima(_:)))
        encima.state = panel.level == .floating ? .on : .off
        menu.addItem(encima)
        menu.addItem(elemento("Ocultar", #selector(ocultarDesdeMenu(_:))))
        menu.addItem(elemento("Pines abiertos…", #selector(panelDesdeMenu(_:))))
        menu.addItem(.separator())

        let texto = elemento("Sacar texto", #selector(sacarTextoDelPin(_:)))
        texto.isEnabled = !soloAudio
        menu.addItem(texto)
        menu.addItem(.separator())
        menu.addItem(elemento("Cerrar", #selector(cerrarDesdeMenu(_:))))
        return menu
    }

    private func elemento(_ titulo: String, _ accion: Selector, tecla: String = "", conMando: Bool = true) -> NSMenuItem {
        let item = NSMenuItem(title: titulo, action: accion, keyEquivalent: tecla)
        item.keyEquivalentModifierMask = conMando && !tecla.isEmpty ? .command : []
        item.target = self
        return item
    }

    /// Un circulito de color para el menu de grupos.
    private static func muestra(_ color: NSColor) -> NSImage {
        NSImage(size: CGSize(width: 12, height: 12), flipped: false) { r in
            color.setFill()
            NSBezierPath(ovalIn: r.insetBy(dx: 1, dy: 1)).fill()
            return true
        }
    }

    /// Copia la foto (y el fichero original, si lo hay); de un PDF o un
    /// video, el fichero; de un pin en vivo, lo que se ve ahora; de una
    /// herramienta, su texto.
    @objc private func copiar(_ sender: Any?) {
        var objetos: [NSPasteboardWriting] = []
        switch contenido {
        case let .imagen(imagen, _):
            objetos.append(NSImage(cgImage: imagen, size: .zero))
        case let .vivo(zona):
            guard let foto = zona.fuente.fotograma() else { return NSSound.beep() }
            objetos.append(NSImage(cgImage: foto, size: .zero))
        case let .herramienta(h):
            objetos.append(h.texto as NSString)
        default:
            break
        }
        if let fichero { objetos.append(fichero as NSURL) }
        guard !objetos.isEmpty else { return NSSound.beep() }
        let portapapeles = NSPasteboard.general
        portapapeles.clearContents()
        portapapeles.writeObjects(objetos)
        avisar("Copiado")
    }

    /// Si vino de un fichero se guarda una copia del original (sin perder
    /// calidad); una captura, como PNG.
    @objc private func guardarComo(_ sender: Any?) {
        let dialogo = NSSavePanel()
        dialogo.canCreateDirectories = true
        if let fichero {
            dialogo.nameFieldStringValue = fichero.lastPathComponent
            if let tipo = UTType(filenameExtension: fichero.pathExtension) {
                dialogo.allowedContentTypes = [tipo]
            }
        } else {
            dialogo.nameFieldStringValue = "Pin.png"
            dialogo.allowedContentTypes = [.png]
        }
        NSApp.activate()
        guard dialogo.runModal() == .OK, let destino = dialogo.url else { return }
        do {
            if let fichero {
                if FileManager.default.fileExists(atPath: destino.path) {
                    try FileManager.default.removeItem(at: destino)
                }
                try FileManager.default.copyItem(at: fichero, to: destino)
            } else if case let .imagen(imagen, _) = contenido {
                guard let salida = CGImageDestinationCreateWithURL(destino as CFURL, UTType.png.identifier as CFString, 1, nil)
                else { throw CocoaError(.fileWriteUnknown) }
                CGImageDestinationAddImage(salida, imagen, nil)
                guard CGImageDestinationFinalize(salida) else { throw CocoaError(.fileWriteUnknown) }
            }
        } catch {
            avisar("No se pudo guardar: \(error.localizedDescription)")
        }
    }

    @objc private func elegirOpacidad(_ sender: NSMenuItem) {
        panel.alphaValue = ReglasPin.limitarOpacidad(CGFloat(sender.tag) / 100)
        alCambiar?()
    }

    @objc private func elegirGrupo(_ sender: NSMenuItem) {
        grupo = sender.tag >= 0 ? sender.tag : nil
    }

    @objc private func alternarEncima(_ sender: Any?) {
        let encima = panel.level != .floating
        panel.isFloatingPanel = encima
        panel.level = encima ? .floating : .normal
    }

    @objc private func anotarDesdeMenu(_ sender: Any?) { empezarAnotar() }
    @objc private func pausaDesdeMenu(_ sender: Any?) { alternarPausa() }
    @objc private func manejarDesdeMenu(_ sender: Any?) { alternarManejar() }
    @objc private func congelarDesdeMenu(_ sender: Any?) { congelar() }
    @objc private func tamanoOriginalDesdeMenu(_ sender: Any?) { tamanoOriginal() }
    @objc private func ocultarDesdeMenu(_ sender: Any?) { ocultar(true) }
    @objc private func panelDesdeMenu(_ sender: Any?) { GestorPines.compartido.mostrarPanelAbiertos() }
    @objc private func cerrarDesdeMenu(_ sender: Any?) { cerrar() }

    /// Una nota a «Mensajes guardados» (como escribirla en el chat).
    @objc private func notaAlChat(_ sender: Any?) {
        guard case let .herramienta(h) = contenido, case let .nota(m) = h.clase,
              let nucleo = GestorPines.compartido.nucleo else { return NSSound.beep() }
        let texto = m.texto.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !texto.isEmpty else { return NSSound.beep() }
        Task { [weak self] in
            let ok = await Task.detached(priority: .utility) { () -> Bool in
                guard let chat = try? nucleo.chatGuardados() else { return false }
                return (try? nucleo.escribirNota(chat: chat.id, texto: texto, respondeA: nil)) != nil
            }.value
            self?.avisar(ok ? "Guardada en «Mensajes guardados»" : "No se pudo guardar la nota")
        }
    }

    // MARK: Sacar texto

    @objc private func sacarTextoDelPin(_ sender: Any?) {
        if case let .herramienta(h) = contenido {
            let portapapeles = NSPasteboard.general
            portapapeles.clearContents()
            portapapeles.setString(h.texto, forType: .string)
            avisar("Texto copiado")
            return
        }
        avisar("Buscando texto…")
        Task { [weak self] in
            guard let self else { return }
            do {
                let texto = try await self.textoDelContenido()
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                guard !self.cerrado else { return }
                if texto.isEmpty {
                    self.avisar("No se encontro texto")
                } else {
                    let portapapeles = NSPasteboard.general
                    portapapeles.clearContents()
                    portapapeles.setString(texto, forType: .string)
                    self.avisar("Texto copiado:\n\(texto)")
                }
            } catch {
                self.avisar("No se pudo sacar el texto: \(error.localizedDescription)")
            }
        }
    }

    /// El texto de lo que se ve: la foto con Vision; en un PDF, el texto de
    /// la pagina (o Vision si es escaneado); en un video, el fotograma
    /// actual; en un pin en vivo, lo que se ve ahora.
    private func textoDelContenido() async throws -> String {
        let lado = ReglasPin.ladoMiniatura(pantalla: (panel.screen ?? NSScreen.main)?.frame.size ?? CGSize(width: 1440, height: 900),
                                           escala: panel.backingScaleFactor)
        switch contenido {
        case let .imagen(imagen, _):
            return try await sacarTexto(de: imagen)

        case let .pdf(documento, _):
            guard let pagina = vistaPDF?.currentPage ?? documento.page(at: 0) else { return "" }
            if let texto = pagina.string, !texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                return texto
            }
            let foto = pagina.thumbnail(of: CGSize(width: lado, height: lado), for: .cropBox)
            guard let imagen = foto.cgImage(forProposedRect: nil, context: nil, hints: nil) else { return "" }
            return try await sacarTexto(de: imagen)

        case let .video(reproductor, _, audio):
            guard !audio, let asset = reproductor.currentItem?.asset else { return "" }
            let generador = AVAssetImageGenerator(asset: asset)
            generador.appliesPreferredTrackTransform = true
            generador.requestedTimeToleranceBefore = .zero
            generador.requestedTimeToleranceAfter = .zero
            generador.maximumSize = CGSize(width: lado, height: lado)
            let fotograma = try await generador.image(at: reproductor.currentTime())
            return try await sacarTexto(de: fotograma.image)

        case let .vivo(zona):
            guard let foto = zona.fuente.fotograma() else { return "" }
            return try await sacarTexto(de: foto)

        case let .herramienta(h):
            return h.texto
        }
    }

    /// Un aviso breve abajo del pin (2,5 s).
    private func avisar(_ texto: String) {
        aviso?.removeFromSuperview()
        let etiqueta = NSTextField(wrappingLabelWithString: texto)
        etiqueta.font = .systemFont(ofSize: 12)
        etiqueta.textColor = .white
        etiqueta.maximumNumberOfLines = 4
        etiqueta.lineBreakMode = .byTruncatingTail
        etiqueta.preferredMaxLayoutWidth = max(80, contenedor.bounds.width - 40)
        etiqueta.translatesAutoresizingMaskIntoConstraints = false

        let caja = NSView()
        caja.wantsLayer = true
        caja.layer?.backgroundColor = NSColor.black.withAlphaComponent(0.75).cgColor
        caja.layer?.cornerRadius = 6
        caja.translatesAutoresizingMaskIntoConstraints = false
        caja.addSubview(etiqueta)
        contenedor.addSubview(caja)
        NSLayoutConstraint.activate([
            etiqueta.leadingAnchor.constraint(equalTo: caja.leadingAnchor, constant: 8),
            etiqueta.trailingAnchor.constraint(equalTo: caja.trailingAnchor, constant: -8),
            etiqueta.topAnchor.constraint(equalTo: caja.topAnchor, constant: 5),
            etiqueta.bottomAnchor.constraint(equalTo: caja.bottomAnchor, constant: -5),
            caja.centerXAnchor.constraint(equalTo: contenedor.centerXAnchor),
            caja.bottomAnchor.constraint(equalTo: contenedor.bottomAnchor, constant: -8),
            caja.leadingAnchor.constraint(greaterThanOrEqualTo: contenedor.leadingAnchor, constant: 8),
            caja.trailingAnchor.constraint(lessThanOrEqualTo: contenedor.trailingAnchor, constant: -8),
        ])
        aviso = caja
        Task { [weak self, weak caja] in
            try? await Task.sleep(for: .seconds(2.5))
            guard let caja else { return }
            caja.removeFromSuperview()
            if self?.aviso === caja { self?.aviso = nil }
        }
    }

    // MARK: Cerrar

    /// Suelta todo: el analisis, el reproductor, la captura en vivo, la barra
    /// y las vistas. El `Pin` (con su imagen) muere cuando el gestor lo quita
    /// de su lista.
    func cerrar() {
        guard !cerrado else { return }
        cerrado = true
        aligerar()
        esconderBarra?.cancel()
        pararRelojVideo()
        barra?.cerrar()
        barra = nil
        if let avisoPagina { NotificationCenter.default.removeObserver(avisoPagina) }
        avisoPagina = nil
        anotacion?.terminarTexto()
        if case let .video(reproductor, _, _) = contenido {
            reproductor.pause()
            reproductor.replaceCurrentItem(with: nil)
        }
        if let zona = zonaViva {
            zona.fuente.alPararse = nil
            Task { await zona.cerrar() }
        }
        vistaPDF?.document = nil
        contenedor.alEntrar = nil
        contenedor.alSalir = nil
        panel.pin = nil
        panel.orderOut(nil)
        panel.contentView = nil
        panel.close()
        alCerrar?(self)
    }

    /// Cerrar «de mentira» para poder deshacer (el «Cerrar todos» del
    /// panel): se esconde sin soltar nada.
    func apartar() { ocultar(true) }

    var estaCerrado: Bool { cerrado }
}
