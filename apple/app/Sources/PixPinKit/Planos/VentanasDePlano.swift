// Abrir planos y modelos 3D en ventanas propias, una por fichero (abrir
// otra vez el mismo la trae al frente), desde un adjunto del chat (doble
// clic) o desde el Finder («Abrir con», arrastrar al icono).
//
// Que visor abre cada fichero lo dice Rust (`claseDeModelo`): DWG y DXF el
// de planos; IFC, RVT, LandXML y los ficheros de puntos (`.csv`, `.txt`…,
// mirando su contenido) el 3D. Lo demas sigue con Vista Rapida.

import AppKit
import PixPinCore
import SwiftUI

@MainActor
public final class VentanasDePlano: NSObject, NSWindowDelegate {
    public static let compartidas = VentanasDePlano()

    /// La carpeta de datos abierta (lo anotado sobre un plano que no es de
    /// ningun chat va a su cache). La pone la app al arrancar.
    public var nucleo: Nucleo?

    private struct Abierta {
        let ventana: NSWindow
        let visor: AnyObject
    }

    private var abiertas: [String: Abierta] = [:]
    private var memoria: DispatchSourceMemoryPressure?

    /// Las extensiones que pueden ser de un visor (las demas ni se miran).
    nonisolated static let extensiones: Set<String> = ["dwg", "dxf", "ifc", "rvt", "xml", "landxml",
                                           "csv", "txt", "xyz", "pts", "pnezd", "penzd"]

    /// Que visor abre `url` (lee como mucho 8 KB para un `.xml` o `.csv`).
    nonisolated public static func clase(de url: URL) -> ClaseDeModelo {
        guard url.isFileURL, extensiones.contains(url.pathExtension.lowercased()) else { return .ninguna }
        return claseDeModelo(ruta: url.path)
    }

    /// **Abre `url` en su visor si es de uno**; si no, `false` (y quien
    /// llama sigue con lo suyo: Vista Rapida, su app...).
    @discardableResult
    public func abrirSiEsSuyo(_ url: URL, titulo: String? = nil) -> Bool {
        let nombre = (titulo?.isEmpty == false ? titulo : nil) ?? url.lastPathComponent
        switch VentanasDePlano.clase(de: url) {
        case .plano:
            abrirPlano(url, titulo: nombre)
            return true
        case .modelo3d:
            abrirModelo(url, titulo: nombre)
            return true
        case .ninguna:
            return false
        }
    }

    /// El visor de planos.
    public func abrirPlano(_ url: URL, titulo: String) {
        let clave = "2d|" + url.path
        if traerAlFrente(clave) { return }
        let visor = VisorPlano(ruta: url.path, titulo: titulo)
        let vista = VentanaPlanoView(visor: visor) { [weak self] in
            self?.abrirModelo(url, titulo: titulo + " · 3D")
        }
        mostrar(NSHostingController(rootView: vista), visor: visor, clave: clave, titulo: titulo)
        if let nucleo {
            visor.cargar(nucleo: nucleo)
        } else {
            NSSound.beep()
        }
    }

    /// El visor 3D.
    public func abrirModelo(_ url: URL, titulo: String) {
        let clave = "3d|" + url.path
        if traerAlFrente(clave) { return }
        let visor = VisorModelo3D(ruta: url.path, titulo: titulo)
        mostrar(NSHostingController(rootView: VentanaModeloView(visor: visor)), visor: visor, clave: clave, titulo: titulo)
        visor.cargar()
    }

    private func traerAlFrente(_ clave: String) -> Bool {
        guard let a = abiertas[clave] else { return false }
        a.ventana.makeKeyAndOrderFront(nil)
        NSApp.activate()
        return true
    }

    private func mostrar(_ controlador: NSViewController, visor: AnyObject, clave: String, titulo: String) {
        let ventana = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1100, height: 760),
                               styleMask: [.titled, .closable, .miniaturizable, .resizable],
                               backing: .buffered, defer: false)
        ventana.title = titulo
        ventana.isReleasedWhenClosed = false
        ventana.contentViewController = controlador
        ventana.setContentSize(NSSize(width: 1100, height: 760))
        ventana.center()
        _ = ventana.setFrameAutosaveName("PixPinPlano")
        ventana.delegate = self
        ventana.identifier = NSUserInterfaceItemIdentifier(clave)
        abiertas[clave] = Abierta(ventana: ventana, visor: visor)
        vigilarMemoria()
        ventana.makeKeyAndOrderFront(nil)
        NSApp.activate()
    }

    public func windowWillClose(_ notification: Notification) {
        guard let ventana = notification.object as? NSWindow,
              let clave = ventana.identifier?.rawValue else { return }
        // Con la ventana se van el visor y sus buferes de la tarjeta.
        abiertas[clave] = nil
        if abiertas.isEmpty {
            memoria?.cancel()
            memoria = nil
        }
    }

    /// Con el aviso de memoria del sistema, los planos sueltan los indices
    /// de la regla (se rehacen al volver a medir).
    private func vigilarMemoria() {
        guard memoria == nil else { return }
        let fuente = DispatchSource.makeMemoryPressureSource(eventMask: [.warning, .critical], queue: .main)
        fuente.setEventHandler { [weak self] in
            MainActor.assumeIsolated {
                guard let self else { return }
                for a in self.abiertas.values {
                    (a.visor as? VisorPlano)?.datos?.plano.vaciarCaches()
                }
            }
        }
        fuente.resume()
        memoria = fuente
    }
}

/// **El delegado de la app para abrir ficheros desde el Finder** («Abrir
/// con PixPin», soltar en el icono): los planos y modelos van a su visor y
/// los documentos (PDF, Word, Excel…) a su lector o pin.
public final class DelegadoAbrirFicheros: NSObject, NSApplicationDelegate {
    /// Que hacer al abrir PixPin otra vez (doble clic en el Finder o en el
    /// Launchpad con la app ya abierta): la app vive en la barra de menus y,
    /// sin esto, no se veria nada.
    @MainActor public static var alReabrir: (() -> Void)?

    public func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        MainActor.assumeIsolated { DelegadoAbrirFicheros.alReabrir?() }
        return true
    }

    public func application(_ application: NSApplication, open urls: [URL]) {
        MainActor.assumeIsolated {
            for u in urls {
                if VentanasDePlano.compartidas.abrirSiEsSuyo(u) { continue }
                if AbrirDesdeFinder.compartido.abrir(u) { continue }
                NSSound.beep()
            }
        }
    }
}
