// PixPin para Mac: vive en la barra de menus (como la bandeja de Windows) y
// abre la ventana de Proyectos cuando se pide.

import PixPinKit
import SwiftUI

@main
struct PixPinApp: App {
    @StateObject private var estado = EstadoApp()
    /// Abrir planos y modelos 3D desde el Finder.
    @NSApplicationDelegateAdaptor(DelegadoAbrirFicheros.self) private var delegado

    var body: some Scene {
        MenuBarExtra {
            MenuPrincipal(estado: estado, grabador: GrabadorPantalla.compartido)
        } label: {
            EtiquetaBarra(grabador: GrabadorPantalla.compartido)
        }
        // Al abrir PixPin se ve la ventana de Proyectos (si no, solo estaria
        // el icono de la barra de menus, que tras la muesca ni se ve), y al
        // abrirla otra vez con la app ya abierta, vuelve a salir.
        Window("Proyectos", id: "proyectos") {
            Group {
                if let almacen = estado.almacen {
                    VentanaProyectos(almacen: almacen)
                        .frame(minWidth: 720, minHeight: 480)
                } else {
                    ContentUnavailableView("No se pudo abrir la carpeta de datos",
                                           systemImage: "exclamationmark.triangle",
                                           description: Text(estado.error ?? ""))
                }
            }
            .modifier(AlReabrirSaleProyectos())
        }
        .defaultSize(width: 980, height: 680)
        .defaultLaunchBehavior(.presented)
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
    /// Lleva las capturas caducadas a la Papelera.
    private var barrendero: BarrenderoCapturas?

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
        // «Abrir con PixPin» desde el Finder: PDF al lector, Office a un pin.
        if let almacen { AbrirDesdeFinder.compartido.instalar(nucleo: almacen.nucleo) }
        let sincro = almacen.map { Sincronizador(nucleo: $0.nucleo) }
        VentanasDePlano.compartidas.nucleo = almacen?.nucleo
        self.sincro = sincro
        atajoCaptura = AtajoDeCaptura { capturador.capturar() }
        ajustes = almacen.map { ModeloAjustes(nucleo: $0.nucleo, sincro: sincro) }
        if let almacen, let sincro {
            sincro.alTraerCambios = { [weak almacen] in Task { await almacen?.cargarChats() } }
            sincro.arrancar()
        }
        // Lo que se hace con la zona elegida: pinearla o sacar su texto.
        capturador.alPinear = { GestorPines.compartido.pinear(imagen: $0) }
        // Pines: en vivo (tecla L al capturar), las mini-apps en «Mensajes
        // guardados» y ⌃2 para mostrar u ocultar todos.
        // Pin en vivo de una zona (modo `L` de la captura y Option + boton
        // central): la pantalla es la que contiene la zona.
        let pinEnVivo: (CGRect) -> Void = { zona in
            let pantalla = NSScreen.screens.first { $0.frame.intersects(zona) } ?? NSScreen.main
            guard let pantalla else { NSSound.beep(); return }
            GestorPines.compartido.pinearEnVivo(zona: zona, pantalla: pantalla)
        }
        capturador.alPinEnVivo = pinEnVivo
        GestorPines.compartido.nucleo = almacen?.nucleo
        GestorPines.compartido.activarAtajoOcultar()
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
        // Al chat: el proyecto se elige en un paso (buscador y teclas 1-6).
        if let almacen {
            let selector = SelectorProyecto()
            capturador.alMandarAlChat = { selector.elegir(para: $0, almacen: almacen) }
        }
        // Copiar apila (⌘V pega la tanda); scroll y GIF de la zona elegida.
        let pila = PilaCapturas.compartida
        capturador.alCopiar = { pila.copiar($0, escala: $1) }
        let scroll = CapturaConScroll.compartida
        scroll.alTerminar = { imagen, escala in
            Capturador.copiar(imagen, escala: escala)
            GestorPines.compartido.pinear(imagen: imagen)
        }
        capturador.alScroll = { zona in Task { await scroll.empezar(zona) } }
        let gif = GrabadorGif.compartido
        gif.alGuardar = { NSWorkspace.shared.activateFileViewerSelecting([$0]) }
        capturador.alGif = { zona in Task { await gif.empezar(zona) } }
        // Los gestos con Option en todo el Mac (apagados de fabrica).
        let gestos = GestosAlt.compartido
        gestos.alCopiar = { pila.copiar($0, escala: $1) }
        gestos.alPinear = { GestorPines.compartido.pinear(imagen: $0, marco: $1) }
        gestos.alAnotarPantalla = { capturador.anotarPantalla() }
        gestos.alPinEnVivo = pinEnVivo
        gestos.arrancarSiToca()
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
        // Las capturas caducadas, a la Papelera (al arrancar y cada hora).
        barrendero = almacen.map { BarrenderoCapturas(nucleo: $0.nucleo) }
        barrendero?.arrancar()
        // Las claves del asistente, en el Llavero del Mac.
        LlaveroDelSistema.instalar()
        // Lo que piden Atajos, Siri y Spotlight (App Intents).
        AccionesApp.compartidas.registrar(almacen: almacen, sincro: sincro,
                                          capturar: { capturador.capturar() })
    }

    /// Empieza a elegir que grabar o, si ya graba, para.
    static func grabarOParar(_ capturador: Capturador) {
        let grabador = GrabadorPantalla.compartido
        if GrabadorGif.compartido.grabando {
            Task { await GrabadorGif.compartido.parar() }
        } else if grabador.grabando {
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

/// Guarda como abrir la ventana de Proyectos para cuando se abre PixPin
/// otra vez (`DelegadoAbrirFicheros.alReabrir`) y la trae al frente.
struct AlReabrirSaleProyectos: ViewModifier {
    @Environment(\.openWindow) private var abrirVentana

    func body(content: Content) -> some View {
        content.onAppear {
            let abrir = abrirVentana
            DelegadoAbrirFicheros.alReabrir = {
                abrir(id: "proyectos")
                NSApp.activate()
            }
            NSApp.activate()
        }
    }
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
        Button("Pin en vivo (elige la zona y pulsa L)") { estado.capturador.capturar() }
        Menu("Nuevo pin") {
            Button("Nota") { GestorPines.compartido.pinearNota() }
            Button("Lista de tareas") { GestorPines.compartido.pinearMiniapp("tareas") }
            Button("Contador") { GestorPines.compartido.pinearMiniapp("contador") }
            Button("Ruleta") { GestorPines.compartido.pinearMiniapp("ruleta") }
            Button("Gastos") { GestorPines.compartido.pinearMiniapp("gastos") }
        }
        Button("Pines abiertos…") { GestorPines.compartido.mostrarPanelAbiertos() }
        Button("Mostrar u ocultar los pines") { GestorPines.compartido.alternarTodos() }
            .keyboardShortcut("2", modifiers: .control)
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
