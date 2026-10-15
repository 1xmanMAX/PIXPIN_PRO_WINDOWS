// **Word, Excel y PowerPoint al instante, como un pin**: el visor del propio
// sistema (Vista Rapida, `QLPreviewView`) dentro de un panel flotante. Es lo
// mas rapido que hay en Mac para un `.docx`, `.xlsx` o `.pptx` (y ODT, RTF,
// CSV, EPUB...): lo pinta el renderizador de Apple, sin cargar nada nuestro.
//
// Perezoso (§4bis): la vista nace al abrir el pin y se cierra con el
// (`QLPreviewView.close()` suelta el proceso de vista previa). Para anotar
// un Word o un PowerPoint, el boton «Leer y anotar» lo abre en el lector
// paginado (`LectoresDeDocumentos`).

import AppKit
import PixPinCore
import Quartz

@MainActor
public final class PinesDeOffice: NSObject, NSWindowDelegate {
    public static let compartidos = PinesDeOffice()

    private struct Abierto {
        let panel: NSPanel
        let vista: QLPreviewView
        /// La accion del boton «Leer y anotar» (el boton no la retiene).
        let accion: AccionDeBoton?
    }

    private var abiertos: [String: Abierto] = [:]

    /// Abre `url` en un pin con Vista Rapida. `nucleo`: si se da, el pin de
    /// un Word o un PowerPoint ofrece abrirlo en el lector para anotar.
    public func abrir(_ url: URL, nucleo: Nucleo? = nil) {
        let clave = url.standardizedFileURL.path
        if let a = abiertos[clave] {
            a.panel.makeKeyAndOrderFront(nil)
            return
        }
        guard FileManager.default.fileExists(atPath: url.path) else {
            NSSound.beep()
            return
        }
        let marco = PinesDeOffice.marcoInicial()
        let panel = NSPanel(contentRect: marco,
                            styleMask: [.titled, .closable, .resizable, .utilityWindow, .nonactivatingPanel],
                            backing: .buffered, defer: false)
        panel.title = url.lastPathComponent
        panel.isFloatingPanel = true
        panel.level = .floating
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.delegate = self
        panel.identifier = NSUserInterfaceItemIdentifier(clave)
        guard let vista = QLPreviewView(frame: CGRect(origin: .zero, size: marco.size), style: .normal) else {
            NSSound.beep()
            return
        }
        vista.autoresizingMask = [.width, .height]
        vista.shouldCloseWithWindow = true
        vista.autostarts = true
        vista.previewItem = url as NSURL
        let contenedor = NSView(frame: CGRect(origin: .zero, size: marco.size))
        contenedor.addSubview(vista)
        var accionGuardada: AccionDeBoton?
        if let nucleo, TiposDeDocumento.clase(de: url) == .officePaginado {
            // «Leer y anotar»: el lector paginado como Office, con tinta.
            let boton = NSButton(title: "Leer y anotar", target: nil, action: nil)
            boton.bezelStyle = .rounded
            boton.controlSize = .small
            boton.sizeToFit()
            boton.frame.origin = CGPoint(x: marco.width - boton.frame.width - 8, y: 8)
            boton.autoresizingMask = [.minXMargin, .maxYMargin]
            let accion = AccionDeBoton { [weak panel] in
                LectoresDeDocumentos.compartidos.abrir(url, nucleo: nucleo)
                panel?.close()
            }
            boton.target = accion
            boton.action = #selector(AccionDeBoton.pulsar)
            accionGuardada = accion
            contenedor.addSubview(boton)
        }
        panel.contentView = contenedor
        abiertos[clave] = Abierto(panel: panel, vista: vista, accion: accionGuardada)
        panel.makeKeyAndOrderFront(nil)
    }

    /// Un tercio de la pantalla a la derecha, de alto casi entero.
    private static func marcoInicial() -> CGRect {
        let pantalla = NSScreen.main?.visibleFrame ?? CGRect(x: 0, y: 0, width: 1440, height: 900)
        let ancho = min(max(pantalla.width / 3, 480), 900)
        let alto = pantalla.height * 0.85
        return CGRect(x: pantalla.maxX - ancho - 24, y: pantalla.minY + (pantalla.height - alto) / 2,
                      width: ancho, height: alto)
    }

    public func windowWillClose(_ notification: Notification) {
        guard let panel = notification.object as? NSPanel,
              let clave = panel.identifier?.rawValue,
              let a = abiertos.removeValue(forKey: clave) else { return }
        a.vista.close()
    }
}

/// Un boton de AppKit con un cierre (sin subclases de controladores).
@MainActor
final class AccionDeBoton: NSObject {
    private let hacer: @MainActor () -> Void
    init(_ hacer: @escaping @MainActor () -> Void) { self.hacer = hacer }
    @objc func pulsar() { hacer() }
}
