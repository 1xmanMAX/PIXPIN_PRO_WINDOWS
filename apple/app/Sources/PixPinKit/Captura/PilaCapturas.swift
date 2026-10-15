// La pila de capturas (`pixpin-pila` + `pila_capturas.rs` de Windows): las
// capturas que se copian seguidas se apilan en una tanda y un solo ⌘V las
// pega todas. Tras cada copia sale un recuadrito en la esquina con la
// ultima y cuantas van; un clic lo ARMA (borde azul) y desde ahi todas se
// suman sin plazo hasta otro clic; sin armar, la tanda se cierra sola a los
// 10 s de la ultima (`apilar_segundos` de Windows) y el recuadro se va.
// Clic derecho: copiar todas, vaciar, ensenar en el Finder.
//
// Como en Windows, cada captura de la pila se escribe como PNG en
// `capturas/` (nombre de la hora): el portapapeles lleva los FICHEROS (y
// el PNG de cada uno, leido del disco solo si la app que pega lo pide),
// asi que la tanda no ocupa memoria. Tope de 24 por tanda: la mas vieja
// sale por abajo.

import AppKit
import ImageIO
import UniformTypeIdentifiers

/// La logica de la tanda, sin pantalla ni disco (la prueban las pruebas).
public struct TandaPila: Equatable {
    /// Cuantas caben (`TOPE_CAPTURAS` de Windows).
    public static let tope = 24
    /// Segundos desde la ultima en los que la siguiente se apila.
    public static let segundosParaApilar: Double = 10

    public private(set) var ficheros: [URL] = []
    public private(set) var ultima: Double?
    /// Armada: no se cierra sola.
    public var armada = false

    public init() {}

    public var cuantas: Int { ficheros.count }

    /// Si la tanda sigue abierta en `ahora` (segundos).
    public func abierta(en ahora: Double) -> Bool {
        guard !ficheros.isEmpty, let ultima else { return false }
        return armada || ahora - ultima <= TandaPila.segundosParaApilar
    }

    /// Mete una captura. Devuelve si empezo una tanda nueva.
    @discardableResult
    public mutating func meter(_ fichero: URL, en ahora: Double) -> Bool {
        let nueva = !abierta(en: ahora)
        if nueva {
            ficheros = []
            armada = false
        }
        ficheros.append(fichero)
        if ficheros.count > TandaPila.tope { ficheros.removeFirst(ficheros.count - TandaPila.tope) }
        ultima = ahora
        return nueva
    }

    public mutating func vaciar() {
        ficheros = []
        ultima = nil
        armada = false
    }
}

@MainActor
public final class PilaCapturas: ObservableObject {
    public static let compartida = PilaCapturas()

    private static let claveActiva = "pila.activa"

    /// Si las copias se apilan (encendida de fabrica, como Windows con
    /// `apilar_segundos = 10`). Apagada, copiar es copiar una imagen suelta.
    public var activa: Bool {
        get { UserDefaults.standard.object(forKey: PilaCapturas.claveActiva) as? Bool ?? true }
        set {
            UserDefaults.standard.set(newValue, forKey: PilaCapturas.claveActiva)
            objectWillChange.send()
            if !newValue { vaciar() }
        }
    }

    public var carpetaDeCapturas = carpetaDeDatos().appendingPathComponent("capturas", isDirectory: true)
    /// Avisa de cada PNG escrito (por la galeria).
    public var alGuardar: ((URL) -> Void)?

    @Published public private(set) var tanda = TandaPila()
    private var icono: IconoPila?
    private var cierre: Task<Void, Never>?
    /// Los proveedores del portapapeles (el tablero no los retiene).
    private var proveedores: [ProveedorPNG] = []

    public init() {}

    private var ahora: Double { Date().timeIntervalSinceReferenceDate }

    /// Una captura copiada. La primera de una tanda va al portapapeles en
    /// el acto (la imagen suelta, como siempre); las siguientes, en cuanto
    /// su PNG esta escrito, junto con todas las de la tanda (asi la imagen
    /// suelta nunca pisa a la lista).
    public func copiar(_ imagen: CGImage, escala: CGFloat) {
        guard activa else {
            Capturador.copiar(imagen, escala: escala)
            return
        }
        if !tanda.abierta(en: ahora) { Capturador.copiar(imagen, escala: escala) }
        let carpeta = carpetaDeCapturas
        let fecha = Date()
        let mini = Miniatura.reducir(imagen, ladoMaximo: 160)
        Task.detached(priority: .utility) { [weak self] in
            var ruta: URL?
            do {
                try FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
                let destino = GeometriaCaptura.rutaNueva(en: carpeta, fecha: fecha)
                guard let datos = Capturador.codificar(imagen, tipo: .png, escala: escala) else {
                    throw CocoaError(.fileWriteUnknown)
                }
                try datos.write(to: destino, options: .atomic)
                ruta = destino
            } catch {
                ruta = nil
            }
            let escrita = ruta
            await MainActor.run {
                guard let self, let escrita else { return }
                self.alGuardar?(escrita)
                self.meter(escrita, miniatura: mini)
            }
        }
    }

    private func meter(_ fichero: URL, miniatura: CGImage) {
        tanda.meter(fichero, en: ahora)
        if tanda.cuantas > 1 { publicar() }
        if icono == nil { icono = IconoPila(pila: self) }
        icono?.ensenar(miniatura: miniatura, cuantas: tanda.cuantas, armada: tanda.armada)
        programarCierre()
    }

    /// El portapapeles con TODAS las de la tanda: un elemento por captura
    /// con su fichero y su PNG (leido del disco solo si se pide).
    public func publicar() {
        let lista = tanda.ficheros
        guard !lista.isEmpty else { return }
        let tablero = NSPasteboard.general
        tablero.clearContents()
        proveedores = lista.map { ProveedorPNG(fichero: $0) }
        let elementos = proveedores.map { p -> NSPasteboardItem in
            let e = NSPasteboardItem()
            e.setString(p.fichero.absoluteString, forType: .fileURL)
            e.setDataProvider(p, forTypes: [.png])
            return e
        }
        if !tablero.writeObjects(elementos) { NSSound.beep() }
    }

    /// Un clic en el recuadro: arma o suelta la tanda.
    func alternarArmada() {
        tanda.armada.toggle()
        if tanda.armada {
            cierre?.cancel()
            icono?.marcar(armada: true)
        } else {
            vaciar()
        }
    }

    public func vaciar() {
        cierre?.cancel()
        cierre = nil
        tanda.vaciar()
        icono?.cerrar()
        icono = nil
    }

    func mostrarEnFinder() {
        guard !tanda.ficheros.isEmpty else { return }
        NSWorkspace.shared.activateFileViewerSelecting(tanda.ficheros)
    }

    /// Sin armar, el recuadro se va (y la tanda se cierra) a los 10 s.
    private func programarCierre() {
        cierre?.cancel()
        guard !tanda.armada else { return }
        cierre = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(TandaPila.segundosParaApilar * 1_000_000_000))
            guard !Task.isCancelled, let self, !self.tanda.armada else { return }
            self.icono?.cerrar()
            self.icono = nil
            self.cierre = nil
        }
    }
}

/// Da el PNG de un fichero de la pila cuando una app lo pide al pegar.
final class ProveedorPNG: NSObject, NSPasteboardItemDataProvider {
    let fichero: URL
    init(fichero: URL) { self.fichero = fichero }

    func pasteboard(_ pasteboard: NSPasteboard?, item: NSPasteboardItem, provideDataForType type: NSPasteboard.PasteboardType) {
        guard type == .png, let datos = try? Data(contentsOf: fichero, options: .mappedIfSafe) else { return }
        item.setData(datos, forType: .png)
    }
}

/// El recuadrito de la esquina (abajo a la derecha de la pantalla del
/// raton): la ultima captura, cuantas van y el borde azul si esta armada.
@MainActor
final class IconoPila {
    private let panel: NSPanel
    private let vista: VistaIconoPila

    init(pila: PilaCapturas) {
        let lado: CGFloat = 76
        panel = NSPanel(contentRect: NSRect(x: 0, y: 0, width: lado, height: lado),
                        styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        panel.isReleasedWhenClosed = false
        panel.hidesOnDeactivate = false
        vista = VistaIconoPila(frame: NSRect(x: 0, y: 0, width: lado, height: lado))
        vista.pila = pila
        panel.contentView = vista
    }

    func ensenar(miniatura: CGImage, cuantas: Int, armada: Bool) {
        vista.poner(miniatura: miniatura, cuantas: cuantas, armada: armada)
        let raton = NSEvent.mouseLocation
        let pantalla = NSScreen.screens.first { $0.frame.contains(raton) } ?? NSScreen.main
        if let zona = pantalla?.visibleFrame {
            panel.setFrameOrigin(CGPoint(x: zona.maxX - panel.frame.width - 16, y: zona.minY + 16))
        }
        panel.orderFrontRegardless()
    }

    func marcar(armada: Bool) { vista.marcar(armada: armada) }

    func cerrar() {
        panel.orderOut(nil)
        panel.close()
    }
}

final class VistaIconoPila: NSView {
    weak var pila: PilaCapturas?
    private let foto = CALayer()
    private let insignia = CATextLayer()

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.cornerRadius = 12
        layer?.backgroundColor = NSColor.windowBackgroundColor.withAlphaComponent(0.92).cgColor
        layer?.borderWidth = 1
        layer?.borderColor = NSColor.separatorColor.cgColor
        foto.frame = bounds.insetBy(dx: 8, dy: 8)
        foto.contentsGravity = .resizeAspect
        foto.cornerRadius = 4
        foto.masksToBounds = true
        layer?.addSublayer(foto)
        insignia.fontSize = 11
        insignia.font = NSFont.systemFont(ofSize: 11, weight: .bold)
        insignia.alignmentMode = .center
        insignia.foregroundColor = NSColor.white.cgColor
        insignia.backgroundColor = NSColor.controlAccentColor.cgColor
        insignia.cornerRadius = 9
        insignia.frame = CGRect(x: bounds.maxX - 26, y: bounds.maxY - 22, width: 22, height: 18)
        insignia.contentsScale = NSScreen.main?.backingScaleFactor ?? 2
        layer?.addSublayer(insignia)
        toolTip = "Pila de capturas: ⌘V las pega todas. Clic: seguir apilando. Clic derecho: más."
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("no se crea desde un nib") }

    func poner(miniatura: CGImage, cuantas: Int, armada: Bool) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        foto.contents = miniatura
        insignia.string = "\(cuantas)"
        insignia.isHidden = cuantas < 2
        CATransaction.commit()
        marcar(armada: armada)
    }

    func marcar(armada: Bool) {
        layer?.borderWidth = armada ? 3 : 1
        layer?.borderColor = (armada ? NSColor.controlAccentColor : NSColor.separatorColor).cgColor
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func mouseUp(with event: NSEvent) {
        pila?.alternarArmada()
    }

    override func rightMouseDown(with event: NSEvent) {
        guard let pila else { return }
        let menu = NSMenu()
        let n = pila.tanda.cuantas
        let objetivo = ObjetivoMenuPila(pila: pila)
        for (titulo, accion) in [("Copiar todas (\(n))", #selector(ObjetivoMenuPila.copiar)),
                                 ("Mostrar en Finder", #selector(ObjetivoMenuPila.finder)),
                                 ("Vaciar la pila", #selector(ObjetivoMenuPila.vaciar))] {
            let item = NSMenuItem(title: titulo, action: accion, keyEquivalent: "")
            item.target = objetivo
            item.representedObject = objetivo
            menu.addItem(item)
        }
        NSMenu.popUpContextMenu(menu, with: event, for: self)
    }
}

/// Las acciones del menu del recuadro.
@MainActor
final class ObjetivoMenuPila: NSObject {
    weak var pila: PilaCapturas?
    init(pila: PilaCapturas) { self.pila = pila }
    @objc func copiar() { pila?.publicar() }
    @objc func finder() { pila?.mostrarEnFinder() }
    @objc func vaciar() { pila?.vaciar() }
}
