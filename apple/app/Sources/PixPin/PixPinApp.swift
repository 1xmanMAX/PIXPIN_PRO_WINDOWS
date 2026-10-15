// PixPin para Mac: vive en la barra de menus (como la bandeja de Windows) y
// abre la ventana de Proyectos cuando se pide.

import PixPinKit
import SwiftUI

@main
struct PixPinApp: App {
    @StateObject private var estado = EstadoApp()

    var body: some Scene {
        MenuBarExtra {
            MenuPrincipal(estado: estado, grabador: GrabadorPantalla.compartido)
        } label: {
            EtiquetaBarra(grabador: GrabadorPantalla.compartido)
        }
        Window("Proyectos", id: "proyectos") {
            if let almacen = estado.almacen {
                VentanaProyectos(almacen: almacen)
                    .frame(minWidth: 720, minHeight: 480)
            } else {
                ContentUnavailableView("No se pudo abrir la carpeta de datos",
                                       systemImage: "exclamationmark.triangle",
                                       description: Text(estado.error ?? ""))
            }
        }
        .defaultSize(width: 980, height: 680)
        Window("Sincronizar", id: "sincronizar") {
            if let sincro = estado.sincro {
                VistaSincro(sincro: sincro)
            } else {
                ContentUnavailableView("No se pudo abrir la carpeta de datos",
                                       systemImage: "exclamationmark.triangle",
                                       description: Text(estado.error ?? ""))
            }
        }
        .defaultSize(width: 520, height: 600)
    }
}

/// El almacen se abre una vez al arrancar; si falla, se dice en la ventana.
@MainActor
final class EstadoApp: ObservableObject {
    let almacen: Almacen?
    let error: String?
    /// Sincronizar con los demas aparatos del grupo (escucha, anuncio y
    /// busqueda viven lo que la app, como la presencia de Windows).
    let sincro: Sincronizador?
    /// Capturar una zona (Ctrl+Option+X en todo el Mac).
    let capturador: Capturador
    private var atajoCaptura: AtajoGlobal?
    private var atajoGrabar: AtajoGlobal?

    init() {
        capturador = Capturador()
        do {
            almacen = try Almacen()
            error = nil
        } catch {
            almacen = nil
            self.error = "\(error)"
        }
        sincro = almacen.map { Sincronizador(nucleo: $0.nucleo) }
        if let almacen, let sincro {
            sincro.alTraerCambios = { [weak almacen] in Task { await almacen?.cargarChats() } }
            sincro.arrancar()
        }
        let capturador = self.capturador
        // Lo que se hace con la zona elegida: pinearla o sacar su texto.
        capturador.alPinear = { GestorPines.compartido.pinear(imagen: $0) }
        capturador.alSacarTexto = { imagen in
            Task {
                guard let texto = try? await sacarTexto(de: imagen), !texto.isEmpty else {
                    NSSound.beep()
                    return
                }
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(texto, forType: .string)
            }
        }
        // Al chat: a «Mensajes guardados», como en Windows sin chat abierto.
        if let almacen {
            capturador.alMandarAlChat = { imagen in
                Task {
                    guard let png = NSBitmapImageRep(cgImage: imagen).representation(using: .png, properties: [:]),
                          let guardados = try? almacen.nucleo.chatGuardados(),
                          await almacen.adjuntar(datos: png, formato: "png", en: guardados.id) != nil
                    else {
                        NSSound.beep()
                        return
                    }
                }
            }
        }
        atajoCaptura = AtajoGlobal(.capturar) { capturador.capturar() }
        // Grabar la pantalla: elegir la zona con la misma captura (Intro o
        // ⌘G graba) y parar desde la barra de menus o con el mismo atajo.
        let grabador = GrabadorPantalla.compartido
        capturador.alGrabar = { zona in Task { await grabador.empezar(zona) } }
        grabador.alGuardar = { NSWorkspace.shared.activateFileViewerSelecting([$0]) }
        atajoGrabar = AtajoGlobal(.grabar) { EstadoApp.grabarOParar(capturador) }
    }

    /// Empieza a elegir que grabar o, si ya graba, para.
    static func grabarOParar(_ capturador: Capturador) {
        let grabador = GrabadorPantalla.compartido
        if grabador.grabando {
            Task { await grabador.parar() }
        } else {
            capturador.capturar(paraGrabar: true)
        }
    }
}

/// El icono de la barra de menus: el pin, o un punto rojo mientras graba.
struct EtiquetaBarra: View {
    @ObservedObject var grabador: GrabadorPantalla

    var body: some View {
        if grabador.grabando {
            Image(nsImage: EtiquetaBarra.puntoRojo)
        } else {
            Image(systemName: "pin.fill")
        }
    }

    /// En color (no plantilla): la barra de menus pinta las plantillas en
    /// blanco o negro, y el rojo es justo lo que tiene que verse.
    static let puntoRojo: NSImage = {
        let config = NSImage.SymbolConfiguration(pointSize: 14, weight: .regular)
            .applying(NSImage.SymbolConfiguration(paletteColors: [.white, .systemRed]))
        let imagen = NSImage(systemSymbolName: "record.circle.fill", accessibilityDescription: "Grabando")?
            .withSymbolConfiguration(config) ?? NSImage()
        imagen.isTemplate = false
        return imagen
    }()
}

struct MenuPrincipal: View {
    @ObservedObject var estado: EstadoApp
    @ObservedObject var grabador: GrabadorPantalla
    @Environment(\.openWindow) private var abrirVentana

    var body: some View {
        Button("Capturar zona") { estado.capturador.capturar() }
            .keyboardShortcut("x", modifiers: [.control, .option])
        if grabador.grabando {
            Button("Detener grabación") { Task { await grabador.parar() } }
                .keyboardShortcut("r", modifiers: [.control, .option])
        } else {
            Button("Grabar pantalla") { EstadoApp.grabarOParar(estado.capturador) }
                .keyboardShortcut("r", modifiers: [.control, .option])
        }
        Toggle("Grabar con el sonido del sistema", isOn: Binding(
            get: { grabador.conSonido },
            set: { grabador.conSonido = $0 }))
            .disabled(grabador.grabando)
        Button("Proyectos…") {
            abrirVentana(id: "proyectos")
            NSApp.activate()
        }
        .keyboardShortcut("p", modifiers: [.command, .option])
        Button("Sincronizar…") {
            abrirVentana(id: "sincronizar")
            NSApp.activate()
        }
        Button("Pinear desde el portapapeles") {
            if !GestorPines.compartido.pinearDesdePortapapeles() { NSSound.beep() }
        }
        Divider()
        Button("Salir de PixPin") { NSApp.terminate(nil) }
            .keyboardShortcut("q")
    }
}
