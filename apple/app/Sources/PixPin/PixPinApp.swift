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
        // En un `Group`: un `SceneBuilder` admite diez escenas seguidas.
        Group {
            Window("Buscar en PixPin", id: IdVentana.buscar) {
                ConAlmacen(estado.almacen, error: estado.error) { VentanaBuscar(almacen: $0) }
            }
            .defaultSize(width: 900, height: 600)
            // Enviar y recibir con un codigo de seis cifras, y los chats
            // compartidos con personas (`Sincro/`).
            Window("Enviar y recibir por Wi-Fi", id: "envio") {
                ConAlmacen(estado.almacen, error: estado.error) { VistaEnvio(nucleo: $0.nucleo) }
            }
            .defaultSize(width: 460, height: 560)
            Window("Chats compartidos", id: "compartidos") {
                ConAlmacen(estado.almacen, error: estado.error) {
                    VistaCompartidos(almacen: $0, sincro: estado.sincro)
                }
            }
            .defaultSize(width: 520, height: 640)
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
    private var atajoGrabar: AtajoGlobal?
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
        // Grabar la pantalla: elegir la zona con la misma captura (Intro o
        // ⌘G graba) y parar desde la barra de menus o con el mismo atajo.
        let grabador = GrabadorPantalla.compartido
        capturador.alGrabar = { zona in Task { await grabador.empezar(zona) } }
        grabador.alGuardar = { NSWorkspace.shared.activateFileViewerSelecting([$0]) }
        atajoGrabar = AtajoGlobal(.grabar) { EstadoApp.grabarOParar(capturador) }
        if let ajustes {
            // El tema y la sincronizacion sola, como se dejaron.
            ajustes.tema.aplicar()
            Task { await ajustes.cargar() }
        }
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
    @Environment(\.openSettings) private var abrirAjustes

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
        if let sincro = estado.sincro {
            MenuEnviarA(sincro: sincro)
        }
        Button("Enviar y recibir por Wi-Fi…") { abrir("envio") }
        Button("Chats compartidos…") { abrir("compartidos") }
        Button("Lecciones…") {
            abrirVentana(id: "lecciones")
            NSApp.activate()
        }
        Button("Asistente…") {
            abrirVentana(id: "asistente")
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
        Button("Ajustes…") {
            abrirAjustes()
            NSApp.activate()
        }
        .keyboardShortcut(",")
        Button("Salir de PixPin") { NSApp.terminate(nil) }
            .keyboardShortcut("q")
    }

    private func abrir(_ id: String) {
        abrirVentana(id: id)
        NSApp.activate()
    }
}
