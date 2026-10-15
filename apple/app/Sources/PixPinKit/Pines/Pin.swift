// Un pin flotante: un `NSPanel` sin bordes, siempre encima, en todos los
// Espacios y sobre las apps a pantalla completa (lo que en Windows es
// `pixpin-pin/src/ventana.rs`).
//
// - Se mueve arrastrando: una foto, desde cualquier sitio; un PDF o un video,
//   desde su asa de arriba (el resto es para seleccionar texto o los mandos).
// - Rueda o pellizco: zoom (en un PDF, Cmd + rueda; el pellizco lo hace el
//   propio PDFView). Opcion + rueda: opacidad.
// - Esc o doble clic (en el asa, si la tiene): cerrar.
// - Clic derecho: el menu.
//
// Rendimiento (§4bis del plan): el pin guarda solo la imagen ya reducida al
// tamano de la pantalla; la escala la hace Core Animation al cambiar el
// marco (no se redibuja nada a mano); Live Text se analiza solo la primera
// vez que el raton pasa por encima y se suelta con la presion de memoria;
// al cerrar se suelta todo (el reproductor se vacia, la vista se quita).

import AppKit
import AVFoundation
import AVKit
import PDFKit
import UniformTypeIdentifiers
import VisionKit

/// Lo que ensena un pin.
enum ContenidoPin {
    /// Una foto ya reducida; `fichero` si vino de un fichero (para guardar
    /// y copiar el original, no la reducida).
    case imagen(CGImage, fichero: URL?)
    case pdf(PDFDocument, fichero: URL)
    case video(AVPlayer, fichero: URL, soloAudio: Bool)
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

    let panel: PanelPin
    let contenido: ContenidoPin
    var alCerrar: (@MainActor (Pin) -> Void)?

    private let contenedor: VistaPin
    private var vistaImagen: NSImageView?
    private var vistaPDF: PDFView?
    private var asa: NSView?
    private var superposicion: ImageAnalysisOverlayView?
    private var tareaAnalisis: Task<Void, Never>?
    private var aviso: NSView?
    /// El tamano con el que nacio (o el del video al conocerlo) y cuanto se
    /// ha agrandado desde entonces.
    private var tamanoBase: CGSize
    private var escala: CGFloat = 1
    private var cerrado = false

    init(contenido: ContenidoPin, marco: CGRect) {
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
        }
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
        contenedor.layer?.borderWidth = 0.5
        contenedor.layer?.borderColor = NSColor.black.withAlphaComponent(0.25).cgColor
        contenedor.autoresizingMask = [.width, .height]
        contenedor.alEntrar = { [weak self] in self?.alEntrarRaton() }
        contenedor.alSalir = { [weak self] in self?.alSalirRaton() }
        panel.contentView = contenedor
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
            montarAsa()

        case let .video(reproductor, _, audio):
            var marco = todo
            if audio { marco.size.height = max(1, marco.height - Pin.altoAsa) }
            let vista = AVPlayerView(frame: marco)
            vista.player = reproductor
            vista.controlsStyle = audio ? .inline : .floating
            vista.showsFullScreenToggleButton = false
            vista.videoGravity = .resizeAspect
            vista.autoresizingMask = [.width, .height]
            contenedor.addSubview(vista)
            montarAsa()
        }
    }

    /// La franja de arriba para mover un PDF o un video. Se ve al pasar el
    /// raton (en un audio, siempre: es casi todo el pin).
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
        asa.alphaValue = soloAudio ? 1 : 0
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
        }
    }

    // MARK: Eventos

    /// `true` si el pin se queda el evento (no sigue a las vistas).
    func atender(_ evento: NSEvent) -> Bool {
        guard !cerrado else { return false }
        switch evento.type {
        case .keyDown:
            if evento.keyCode == 53 { // Esc
                cerrar()
                return true
            }
            return false

        case .rightMouseDown:
            mostrarMenu(evento)
            return true

        case .leftMouseDown:
            if evento.modifierFlags.contains(.control) {
                mostrarMenu(evento)
                return true
            }
            if case .imagen = contenido {
                // Sobre un texto de Live Text, el clic es para seleccionarlo.
                if let capa = superposicion, capa.analysis != nil,
                   capa.hasInteractiveItem(at: capa.convert(evento.locationInWindow, from: nil)) {
                    return false
                }
            } else {
                guard let asa, asa.frame.contains(contenedor.convert(evento.locationInWindow, from: nil))
                else { return false }
            }
            if evento.clickCount >= 2 {
                cerrar()
            } else {
                panel.makeKey()
                panel.performDrag(with: evento)
            }
            return true

        case .scrollWheel:
            let opcion = evento.modifierFlags.contains(.option)
            // En un PDF, la rueda sola pasa paginas (con su inercia).
            if case .pdf = contenido, !opcion, !evento.modifierFlags.contains(.command) { return false }
            // La inercia del trackpad no cuenta: el pin se queda donde se solto.
            if !evento.momentumPhase.isEmpty { return true }
            if opcion {
                panel.alphaValue = ReglasPin.opacidadTrasRueda(panel.alphaValue, deltaY: evento.scrollingDeltaY,
                                                               precisa: evento.hasPreciseScrollingDeltas)
                return true
            }
            zoom(por: ReglasPin.factorZoom(deltaY: evento.scrollingDeltaY, precisa: evento.hasPreciseScrollingDeltas),
                 ancla: NSEvent.mouseLocation)
            return true

        case .magnify:
            if case .pdf = contenido { return false }
            zoom(por: 1 + evento.magnification, ancla: NSEvent.mouseLocation)
            return true

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
    }

    private func alEntrarRaton() {
        prepararTextoVivo()
        if let asa, !soloAudio { asa.animator().alphaValue = 1 }
    }

    private func alSalirRaton() {
        if let asa, !soloAudio { asa.animator().alphaValue = 0 }
    }

    // MARK: Live Text

    /// Analiza la foto la primera vez que hace falta (raton encima) y pone la
    /// capa de Live Text encima: el texto se selecciona sobre el propio pin.
    private func prepararTextoVivo() {
        guard case let .imagen(imagen, _) = contenido, let vista = vistaImagen,
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
                vista.addSubview(capa)
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

    // MARK: Menu

    private func mostrarMenu(_ evento: NSEvent) {
        let menu = NSMenu()
        menu.autoenablesItems = false
        menu.addItem(elemento("Copiar", #selector(copiar(_:))))
        menu.addItem(elemento("Guardar como…", #selector(guardarComo(_:))))
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

        let encima = elemento("Siempre encima", #selector(alternarEncima(_:)))
        encima.state = panel.level == .floating ? .on : .off
        menu.addItem(encima)
        menu.addItem(.separator())

        let texto = elemento("Sacar texto", #selector(sacarTextoDelPin(_:)))
        texto.isEnabled = !soloAudio
        menu.addItem(texto)
        menu.addItem(.separator())
        menu.addItem(elemento("Cerrar", #selector(cerrarDesdeMenu(_:))))

        NSMenu.popUpContextMenu(menu, with: evento, for: contenedor)
    }

    private func elemento(_ titulo: String, _ accion: Selector) -> NSMenuItem {
        let item = NSMenuItem(title: titulo, action: accion, keyEquivalent: "")
        item.target = self
        return item
    }

    /// Copia la foto (y el fichero original, si lo hay); de un PDF o un
    /// video, el fichero.
    @objc private func copiar(_ sender: Any?) {
        var objetos: [NSPasteboardWriting] = []
        if case let .imagen(imagen, _) = contenido {
            objetos.append(NSImage(cgImage: imagen, size: .zero))
        }
        if let fichero { objetos.append(fichero as NSURL) }
        let portapapeles = NSPasteboard.general
        portapapeles.clearContents()
        portapapeles.writeObjects(objetos)
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
    }

    @objc private func alternarEncima(_ sender: Any?) {
        let encima = panel.level != .floating
        panel.isFloatingPanel = encima
        panel.level = encima ? .floating : .normal
    }

    @objc private func cerrarDesdeMenu(_ sender: Any?) { cerrar() }

    // MARK: Sacar texto

    @objc private func sacarTextoDelPin(_ sender: Any?) {
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
    /// la pagina (o Vision si es escaneado); en un video, el fotograma actual.
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

    /// Suelta todo: el analisis, el reproductor y las vistas. El `Pin` (con
    /// su imagen) muere cuando el gestor lo quita de su lista.
    func cerrar() {
        guard !cerrado else { return }
        cerrado = true
        aligerar()
        if case let .video(reproductor, _, _) = contenido {
            reproductor.pause()
            reproductor.replaceCurrentItem(with: nil)
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
}
