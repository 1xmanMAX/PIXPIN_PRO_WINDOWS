// El gestor de pines: los crea y los tiene vivos mientras estan abiertos.
// Es la API que usan los demas (la captura, el menu de la barra):
//
//     GestorPines.compartido.pinear(imagen: captura, marco: zonaCapturada)
//     GestorPines.compartido.pinear(fichero: url)
//     GestorPines.compartido.pinearDesdePortapapeles()
//     GestorPines.compartido.pinearEnVivo(zona: zona, pantalla: pantalla)
//     GestorPines.compartido.pinearNota()
//     GestorPines.compartido.pinearMiniapp("tareas")
//     GestorPines.compartido.alternarTodos()          // ⌃2
//     GestorPines.compartido.mostrarPanelAbiertos()
//
// Rendimiento (§4bis del plan): las fotos se leen con ImageIO ya reducidas
// al tamano de la pantalla y fuera del hilo principal; el aviso de memoria
// solo se escucha mientras hay algun pin abierto.

import AppKit
import AVFoundation
import Carbon.HIToolbox
import PDFKit
import PixPinCore
import UniformTypeIdentifiers

@MainActor
public final class GestorPines {
    public static let compartido = GestorPines()

    private var pines: [Pin] = []
    private var vigiaMemoria: DispatchSourceMemoryPressure?
    private var siguienteId = 1
    private let panelAbiertos = PanelPinesAbiertos()
    /// Los que aparto «Cerrar todos», por si se deshace.
    private var apartados: [Pin] = []
    private var olvidarApartados: Task<Void, Never>?
    private var atajoOcultar: AtajoGlobal?

    /// El nucleo de la carpeta de datos: donde viven las mini-apps (en
    /// «Mensajes guardados») y adonde va una nota. Lo pone la app al
    /// arrancar; sin el, las herramientas viven solo en memoria.
    public var nucleo: Nucleo?

    init() {
        let m = panelAbiertos.modelo
        m.alTraer = { [weak self] id in self?.pin(id)?.resaltar() }
        m.alAlternar = { [weak self] id in
            guard let pin = self?.pin(id) else { return }
            pin.ocultar(!pin.oculto)
        }
        m.alCerrar = { [weak self] id in self?.pin(id)?.cerrar() }
        m.alMostrarTodos = { [weak self] in self?.ocultarTodos(false) }
        m.alOcultarTodos = { [weak self] in self?.ocultarTodos(true) }
        m.alCerrarTodos = { [weak self] in self?.cerrarTodosConDeshacer() }
        m.alDeshacer = { [weak self] in self?.deshacerCerrarTodos() }
    }

    /// Cuantos pines hay abiertos.
    public var cuantos: Int { pines.count }

    // MARK: Pin en vivo

    /// **Pin en vivo**: la zona `zona` (coordenadas de pantalla de AppKit,
    /// globales) de `pantalla`, vista en directo. Nace al lado de su zona,
    /// a su tamano.
    public func pinearEnVivo(zona: CGRect, pantalla: NSScreen) {
        let zona = zona.standardized.integral
        guard zona.width >= 8, zona.height >= 8, PermisoCaptura.asegurar(),
              let viva = ZonaViva(zona: zona, pantalla: pantalla) else {
            NSSound.beep()
            return
        }
        abrir(.vivo(viva), marco: ReglasVivo.sitioJuntoA(zona, trabajo: pantalla.visibleFrame))
    }

    // MARK: Herramientas

    /// Una nota suelta en Markdown.
    public func pinearNota(texto: String = "") {
        let pantalla = Self.pantallaDelRaton()
        let visible = pantalla?.visibleFrame ?? CGRect(origin: .zero, size: Self.pantallaSupuesta)
        let marco = ReglasPin.marcoInicial(tamano: CGSize(width: 300, height: 260),
                                           centro: NSEvent.mouseLocation, visible: visible)
        abrir(.herramienta(HerramientaPin(.nota(ModeloNota(texto: texto)))), marco: marco)
    }

    /// Una mini-app nueva (`tareas`, `contador`, `ruleta`, `gastos`...): se
    /// crea en «Mensajes guardados» (asi se guarda y viaja al movil) y se
    /// pinea. Sin nucleo, vive solo en memoria.
    public func pinearMiniapp(_ miniapp: String, titulo: String = "") {
        let moneda = monedaDelAparato()
        guard let documento = miniDocumentoNuevo(miniapp: miniapp, titulo: titulo, moneda: moneda) else {
            NSSound.beep()
            return
        }
        guard let nucleo else {
            abrirMiniapp(DocumentoMini(miniapp: miniapp, documento: documento, nucleo: nil, chat: nil, mensaje: nil))
            return
        }
        Task { [weak self] in
            let creado = await Task.detached(priority: .userInitiated) { () -> (String, MensajeChat)? in
                guard let chat = try? nucleo.chatGuardados(),
                      let m = try? nucleo.crearMiniapp(chat: chat.id, miniapp: miniapp, titulo: titulo, moneda: moneda)
                else { return nil }
                return (chat.id, m)
            }.value
            guard let self else { return }
            guard let creado else {
                NSSound.beep()
                return
            }
            let (chat, m) = creado
            self.abrirMiniapp(DocumentoMini(miniapp: miniapp, documento: m.texto, nucleo: nucleo,
                                            chat: chat, mensaje: m.id))
        }
    }

    /// Saca a la pantalla una mini-app que ya esta en un chat (su mensaje
    /// `MINIAPP`): lo que se toque en el pin se guarda en ese mensaje.
    public func pinearMiniapp(chat: String, mensaje: String, miniapp: String, documento: String) {
        abrirMiniapp(DocumentoMini(miniapp: miniapp, documento: documento, nucleo: nucleo,
                                   chat: chat, mensaje: mensaje))
    }

    private func abrirMiniapp(_ doc: DocumentoMini) {
        let pantalla = Self.pantallaDelRaton()
        let visible = pantalla?.visibleFrame ?? CGRect(origin: .zero, size: Self.pantallaSupuesta)
        let tam = doc.miniapp == "ruleta" ? CGSize(width: 300, height: 420) : CGSize(width: 300, height: 340)
        abrir(.herramienta(HerramientaPin(.mini(doc))),
              marco: ReglasPin.marcoInicial(tamano: tam, centro: NSEvent.mouseLocation, visible: visible))
    }

    // MARK: Todos

    /// Muestra u oculta todos los pines (⌃2): si hay alguno a la vista, los
    /// oculta; si no, los ensena.
    public func alternarTodos() {
        ocultarTodos(pines.contains { !$0.oculto })
    }

    public func ocultarTodos(_ ocultar: Bool) {
        olvidarApartadosYa()
        for pin in pines { pin.ocultar(ocultar) }
    }

    /// El panel «Pines abiertos».
    public func mostrarPanelAbiertos() {
        refrescarPanel()
        panelAbiertos.mostrar()
    }

    /// Registra ⌃2 en todo el Mac (como el Ctrl 2 de Windows). Si otra app
    /// ya lo tiene, se queda solo en el menu.
    public func activarAtajoOcultar() {
        guard atajoOcultar == nil else { return }
        let combinacion = AtajoGlobal.Combinacion(tecla: UInt32(kVK_ANSI_2), modificadores: UInt32(controlKey))
        atajoOcultar = AtajoGlobal(combinacion) { GestorPines.compartido.alternarTodos() }
    }

    /// Cierra todos de forma que se puede deshacer: se apartan (ocultos, sin
    /// soltar nada) y se cierran de verdad a los 30 s o al hacer otra cosa.
    public func cerrarTodosConDeshacer() {
        olvidarApartadosYa()
        apartados = pines
        for pin in apartados { pin.apartar() }
        pines = []
        refrescarPanel()
        olvidarApartados = Task { [weak self] in
            try? await Task.sleep(for: .seconds(30))
            guard !Task.isCancelled else { return }
            self?.olvidarApartadosYa()
        }
    }

    public func deshacerCerrarTodos() {
        olvidarApartados?.cancel()
        olvidarApartados = nil
        let vuelven = apartados.filter { !$0.estaCerrado }
        apartados = []
        pines += vuelven
        for pin in vuelven { pin.ocultar(false) }
        refrescarPanel()
    }

    private func olvidarApartadosYa() {
        olvidarApartados?.cancel()
        olvidarApartados = nil
        let lista = apartados
        apartados = []
        for pin in lista { pin.cerrar() }
        if !lista.isEmpty { refrescarPanel() }
    }

    /// Los marcos de los pines a la vista menos `pin` (para las guias).
    func marcosVisibles(salvo pin: Pin) -> [CGRect] {
        pines.filter { $0 !== pin && !$0.oculto }.map(\.panel.frame)
    }

    private func pin(_ id: Int) -> Pin? {
        pines.first { $0.id == id }
    }

    private func refrescarPanel() {
        let m = panelAbiertos.modelo
        m.filas = pines.map(\.fila)
        m.puedeDeshacer = !apartados.isEmpty
    }

    /// Pinea una imagen ya en memoria (una captura). Con `marco` (en
    /// coordenadas de pantalla) nace justo ahi, encima de lo capturado; sin
    /// el, a su tamano natural junto al raton. Si es mayor que la pantalla se
    /// guarda reducida.
    public func pinear(imagen: CGImage, marco: CGRect? = nil) {
        let pantalla = Self.pantallaDelRaton()
        let lado = ReglasPin.ladoMiniatura(pantalla: pantalla?.frame.size ?? Self.pantallaSupuesta,
                                           escala: pantalla?.backingScaleFactor ?? 2)
        let reducida = Miniatura.reducir(imagen, ladoMaximo: lado)
        if let marco, marco.width >= 1, marco.height >= 1 {
            abrir(.imagen(reducida, fichero: nil), marco: marco)
        } else {
            abrirImagen(reducida, pixelesNaturales: CGSize(width: imagen.width, height: imagen.height),
                        fichero: nil, pantalla: pantalla)
        }
    }

    /// Pinea un fichero: foto (ImageIO, reducida), PDF (PDFKit) o video y
    /// audio (AVKit). `false` si no es de ningun tipo que se pueda pinear;
    /// si la foto no se puede leer, suena un aviso al terminar de intentarlo.
    @discardableResult
    public func pinear(fichero url: URL) -> Bool {
        guard let tipo = TipoContenido.de(url) else { return false }
        let pantalla = Self.pantallaDelRaton()
        let visible = pantalla?.visibleFrame ?? CGRect(origin: .zero, size: Self.pantallaSupuesta)
        switch tipo {
        case .imagen:
            let lado = ReglasPin.ladoMiniatura(pantalla: pantalla?.frame.size ?? Self.pantallaSupuesta,
                                               escala: pantalla?.backingScaleFactor ?? 2)
            Task { [weak self] in
                let imagen = await Miniatura.fueraDelPrincipal { Miniatura.de(url: url, ladoMaximo: lado) }
                guard let self else { return }
                guard let imagen else {
                    NSSound.beep()
                    return
                }
                self.abrirImagen(imagen, pixelesNaturales: nil, fichero: url, pantalla: pantalla)
            }

        case .pdf:
            // PDFDocument lee las paginas segun se ven, no el fichero entero.
            guard let documento = PDFDocument(url: url) else { return false }
            var tamano = CGSize(width: 612, height: 792)
            if let pagina = documento.page(at: 0) {
                let caja = pagina.bounds(for: .cropBox).size
                let girada = pagina.rotation % 180 != 0
                tamano = girada ? CGSize(width: caja.height, height: caja.width) : caja
            }
            let tam = ReglasPin.tamanoInicial(pixeles: tamano, escala: 1, visible: visible.size)
            abrir(.pdf(documento, fichero: url),
                  marco: ReglasPin.marcoInicial(tamano: tam, centro: NSEvent.mouseLocation, visible: visible))

        case .video, .audio:
            // AVPlayer decodifica por hardware (VideoToolbox).
            let reproductor = AVPlayer(url: url)
            let audio = tipo == .audio
            let tam = audio ? CGSize(width: 360, height: 66) : CGSize(width: 480, height: 270)
            abrir(.video(reproductor, fichero: url, soloAudio: audio),
                  marco: ReglasPin.marcoInicial(tamano: tam, centro: NSEvent.mouseLocation, visible: visible))
        }
        return true
    }

    /// Pinea lo que haya en el portapapeles: ficheros copiados en el Finder
    /// (hasta 10) o una imagen. `false` si no habia nada que pinear.
    @discardableResult
    public func pinearDesdePortapapeles() -> Bool {
        let portapapeles = NSPasteboard.general
        if let urls = portapapeles.readObjects(forClasses: [NSURL.self],
                                               options: [.urlReadingFileURLsOnly: true]) as? [URL],
           !urls.isEmpty {
            var alguno = false
            for url in urls.prefix(10) {
                if pinear(fichero: url) { alguno = true }
            }
            if alguno { return true }
        }
        let tipos: [NSPasteboard.PasteboardType] = [
            .png, .tiff,
            NSPasteboard.PasteboardType(UTType.heic.identifier),
            NSPasteboard.PasteboardType(UTType.jpeg.identifier),
        ]
        guard let tipo = portapapeles.availableType(from: tipos),
              let datos = portapapeles.data(forType: tipo) else { return false }
        let pantalla = Self.pantallaDelRaton()
        let lado = ReglasPin.ladoMiniatura(pantalla: pantalla?.frame.size ?? Self.pantallaSupuesta,
                                           escala: pantalla?.backingScaleFactor ?? 2)
        Task { [weak self] in
            let imagen = await Miniatura.fueraDelPrincipal { Miniatura.de(datos: datos, ladoMaximo: lado) }
            guard let self else { return }
            guard let imagen else {
                NSSound.beep()
                return
            }
            self.abrirImagen(imagen, pixelesNaturales: nil, fichero: nil, pantalla: pantalla)
        }
        return true
    }

    /// Cierra todos los pines (sin deshacer).
    public func cerrarTodos() {
        olvidarApartadosYa()
        for pin in pines { pin.cerrar() }
    }

    // MARK: Por dentro

    private static let pantallaSupuesta = CGSize(width: 1440, height: 900)

    /// La pantalla donde esta el raton (alli nace el pin).
    private static func pantallaDelRaton() -> NSScreen? {
        let raton = NSEvent.mouseLocation
        return NSScreen.screens.first { NSMouseInRect(raton, $0.frame, false) } ?? NSScreen.main
    }

    private func abrirImagen(_ imagen: CGImage, pixelesNaturales: CGSize?, fichero: URL?, pantalla: NSScreen?) {
        let visible = pantalla?.visibleFrame ?? CGRect(origin: .zero, size: Self.pantallaSupuesta)
        let pixeles = pixelesNaturales ?? CGSize(width: imagen.width, height: imagen.height)
        let tam = ReglasPin.tamanoInicial(pixeles: pixeles, escala: pantalla?.backingScaleFactor ?? 2,
                                          visible: visible.size)
        abrir(.imagen(imagen, fichero: fichero),
              marco: ReglasPin.marcoInicial(tamano: tam, centro: NSEvent.mouseLocation, visible: visible))
    }

    private func abrir(_ contenido: ContenidoPin, marco: CGRect) {
        let pin = Pin(id: siguienteId, contenido: contenido, marco: marco)
        siguienteId += 1
        pin.alCerrar = { [weak self] cerrado in self?.quitar(cerrado) }
        pin.alCambiar = { [weak self] in self?.refrescarPanel() }
        pines.append(pin)
        vigilarMemoria()
        pin.mostrar()
        refrescarPanel()
    }

    private func quitar(_ pin: Pin) {
        pines.removeAll { $0 === pin }
        apartados.removeAll { $0 === pin }
        refrescarPanel()
        if pines.isEmpty, apartados.isEmpty {
            vigiaMemoria?.cancel()
            vigiaMemoria = nil
        }
    }

    /// Con presion de memoria, cada pin suelta lo que puede rehacer.
    private func vigilarMemoria() {
        guard vigiaMemoria == nil else { return }
        let fuente = DispatchSource.makeMemoryPressureSource(eventMask: [.warning, .critical], queue: .main)
        fuente.setEventHandler { [weak self] in
            MainActor.assumeIsolated {
                self?.pines.forEach { $0.aligerar() }
            }
        }
        fuente.resume()
        vigiaMemoria = fuente
    }
}
