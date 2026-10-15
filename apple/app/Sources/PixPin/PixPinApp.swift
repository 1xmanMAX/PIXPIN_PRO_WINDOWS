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
        Window("Lecciones", id: "lecciones") {
            if let almacen = estado.almacen {
                VentanaLecciones(nucleo: almacen.nucleo)
                    .frame(minWidth: 760, minHeight: 480)
            } else {
                ContentUnavailableView("No se pudo abrir la carpeta de datos",
                                       systemImage: "exclamationmark.triangle",
                                       description: Text(estado.error ?? ""))
            }
        }
        .defaultSize(width: 960, height: 640)
        Window("Asistente", id: "asistente") {
            VentanaAsistente(almacen: estado.almacen)
        }
        .defaultSize(width: 460, height: 640)
        // Ajustes (⌘,): lo que vale en Windows y Mac va a `pixpinmax.toml`.
        Settings {
            if let ajustes = estado.ajustes {
                VistaAjustes(modelo: ajustes, atajo: estado.atajoCaptura)
            } else {
                ContentUnavailableView("No se pudo abrir la carpeta de datos",
                                       systemImage: "exclamationmark.triangle",
                                       description: Text(estado.error ?? ""))
                    .frame(width: 420, height: 240)
            }
        }
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
    /// El atajo de capturar (se cambia en Ajustes).
    let atajoCaptura: AtajoDeCaptura
    /// Lo de Ajustes (`pixpinmax.toml`, tema, abrir al iniciar sesion).
    let ajustes: ModeloAjustes?

    init() {
        let capturador = Capturador()
        self.capturador = capturador
        do {
            almacen = try Almacen()
            error = nil
        } catch {
            almacen = nil
            self.error = "\(error)"
        }
        let sincro = almacen.map { Sincronizador(nucleo: $0.nucleo) }
        self.sincro = sincro
        atajoCaptura = AtajoDeCaptura { capturador.capturar() }
        ajustes = almacen.map { ModeloAjustes(nucleo: $0.nucleo, sincro: sincro) }
        if let almacen, let sincro {
            sincro.alTraerCambios = { [weak almacen] in Task { await almacen?.cargarChats() } }
            sincro.arrancar()
        }
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
        if let ajustes {
            // El tema y la sincronizacion sola, como se dejaron.
            ajustes.tema.aplicar()
            Task { await ajustes.cargar() }
        }
    }
}

struct MenuPrincipal: View {
    @ObservedObject var estado: EstadoApp
    @Environment(\.openWindow) private var abrirVentana
    @Environment(\.openSettings) private var abrirAjustes

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
        Button("Lecciones…") {
            abrirVentana(id: "lecciones")
            NSApp.activate()
        }
        Button("Asistente…") {
            abrirVentana(id: "asistente")
            NSApp.activate()
        }
        Button("Pinear desde el portapapeles") {
            if !GestorPines.compartido.pinearDesdePortapapeles() { NSSound.beep() }
        }
        Divider()
        Button("Ajustes…") {
            abrirAjustes()
            NSApp.activate()
        }
        .keyboardShortcut(",")
        Button("Salir de PixPin") { NSApp.terminate(nil) }
            .keyboardShortcut("q")
    }
}
