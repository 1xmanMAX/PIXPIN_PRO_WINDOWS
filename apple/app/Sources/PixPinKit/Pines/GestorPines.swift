// El gestor de pines: los crea y los tiene vivos mientras estan abiertos.
// Es la API que usan los demas (la captura, el menu de la barra):
//
//     GestorPines.compartido.pinear(imagen: captura, marco: zonaCapturada)
//     GestorPines.compartido.pinear(fichero: url)
//     GestorPines.compartido.pinearDesdePortapapeles()
//
// Rendimiento (§4bis del plan): las fotos se leen con ImageIO ya reducidas
// al tamano de la pantalla y fuera del hilo principal; el aviso de memoria
// solo se escucha mientras hay algun pin abierto.

import AppKit
import AVFoundation
import PDFKit
import UniformTypeIdentifiers

@MainActor
public final class GestorPines {
    public static let compartido = GestorPines()

    private var pines: [Pin] = []
    private var vigiaMemoria: DispatchSourceMemoryPressure?

    init() {}

    /// Cuantos pines hay abiertos.
    public var cuantos: Int { pines.count }

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

    /// Cierra todos los pines.
    public func cerrarTodos() {
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
        let pin = Pin(contenido: contenido, marco: marco)
        pin.alCerrar = { [weak self] cerrado in self?.quitar(cerrado) }
        pines.append(pin)
        vigilarMemoria()
        pin.mostrar()
    }

    private func quitar(_ pin: Pin) {
        pines.removeAll { $0 === pin }
        if pines.isEmpty {
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
