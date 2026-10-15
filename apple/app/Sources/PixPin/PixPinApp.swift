// PixPin para Mac: vive en la barra de menus (como la bandeja de Windows) y
// abre la ventana de Proyectos cuando se pide.

import PixPinKit
import SwiftUI

@main
struct PixPinApp: App {
    @StateObject private var estado = EstadoApp()

    var body: some Scene {
        MenuBarExtra("PixPin", systemImage: "pin.fill") {
            MenuPrincipal(estado: estado)
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
        Window("Galería de capturas", id: IdVentana.galeria) {
            ConAlmacen(estado.almacen, error: estado.error) { VentanaGaleria(almacen: $0) }
        }
        .defaultSize(width: 900, height: 640)
        Window("Timeline", id: IdVentana.timeline) {
            ConAlmacen(estado.almacen, error: estado.error) { VentanaTimeline(almacen: $0) }
        }
        .defaultSize(width: 860, height: 640)
        Window("Tareas", id: IdVentana.tareas) {
            ConAlmacen(estado.almacen, error: estado.error) { VentanaTareas(almacen: $0) }
        }
        .defaultSize(width: 620, height: 640)
        Window("Buscar en PixPin", id: IdVentana.buscar) {
            ConAlmacen(estado.almacen, error: estado.error) { VentanaBuscar(almacen: $0) }
        }
        .defaultSize(width: 900, height: 600)
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
    }
}

struct MenuPrincipal: View {
    @ObservedObject var estado: EstadoApp
    @Environment(\.openWindow) private var abrirVentana

    var body: some View {
        Button("Capturar zona") { estado.capturador.capturar() }
            .keyboardShortcut("x", modifiers: [.control, .option])
        Button("Proyectos…") {
            abrirVentana(id: "proyectos")
            NSApp.activate()
        }
        .keyboardShortcut("p", modifiers: [.command, .option])
        Button("Sincronizar…") {
            abrirVentana(id: "sincronizar")
            NSApp.activate()
        }
        Button("Buscar en PixPin…") { abrir(IdVentana.buscar) }
            .keyboardShortcut("f", modifiers: [.command, .option])
        Button("Galería de capturas…") { abrir(IdVentana.galeria) }
        Button("Timeline…") { abrir(IdVentana.timeline) }
        Button("Tareas…") { abrir(IdVentana.tareas) }
        Button("Pinear desde el portapapeles") {
            if !GestorPines.compartido.pinearDesdePortapapeles() { NSSound.beep() }
        }
        Divider()
        Button("Salir de PixPin") { NSApp.terminate(nil) }
            .keyboardShortcut("q")
    }

    private func abrir(_ id: String) {
        abrirVentana(id: id)
        NSApp.activate()
    }
}
