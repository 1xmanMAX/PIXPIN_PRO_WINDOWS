// Capturar una zona de la pantalla: la foto congelada, elegir la zona y lo
// que se hace con ella (copiar, pinear, al chat, texto, guardar).
//
// Lo que no es de la captura lo hacen otros por los ganchos `alPinear`,
// `alMandarAlChat` y `alSacarTexto`: si un gancho no esta puesto, su boton
// sale apagado.
//
// Rendimiento (§4bis del plan):
// - una foto por pantalla al abrir, y se suelta al cerrar (las vistas
//   sueltan sus capas y los paneles se cierran y se olvidan);
// - el recorte es `CGImage.cropping(to:)`: sin copiar ni recodificar. Si lo
//   recortado va a vivir mas que la captura (pin, chat, OCR) y es pequeno
//   frente a la pantalla, se copia aparte para no retener la foto entera;
// - codificar el PNG/TIFF y escribir el fichero van fuera del hilo
//   principal, con prioridad baja.

import AppKit
import ImageIO
import UniformTypeIdentifiers

@MainActor
public final class Capturador {
    /// Hace un pin con la imagen recortada.
    public var alPinear: ((CGImage) -> Void)?
    /// Manda la imagen a un proyecto (el que elija el usuario).
    public var alMandarAlChat: ((CGImage) -> Void)?
    /// Saca el texto de la imagen (OCR con Vision).
    public var alSacarTexto: ((CGImage) -> Void)?
    /// Graba en video la zona elegida (`Grabar/GrabadorPantalla`).
    public var alGrabar: ((ZonaDeGrabacion) -> Void)?
    /// Avisa de que se guardo una captura (por si la galeria esta abierta).
    public var alGuardar: ((URL) -> Void)?
    /// Pin en vivo de la zona elegida (tecla `L`): la zona en coordenadas de
    /// pantalla de AppKit y su pantalla (`GestorPines.pinearEnVivo`).
    public var alPinearEnVivo: ((CGRect, NSScreen) -> Void)?

    /// Donde se guardan las capturas: `capturas/` de los datos, la misma
    /// carpeta que la galeria de Windows (`galeria_capturas::carpeta_en`),
    /// que es la que viaja entre aparatos.
    public var carpetaDeCapturas: URL

    /// El modo que manda en todas las pantallas (`Z` `V` `P` `T`).
    private(set) var modo: ModoCaptura = .zona
    private var paneles: [PanelCaptura] = []
    /// La vista de la pantalla donde esta el raton o donde se empezo.
    private weak var activa: VistaCaptura?
    private var abriendo = false
    /// Lo que hace Intro: copiar, o grabar si se abrio para grabar.
    private var intro: AccionCaptura = .copiar
    var accionIntro: AccionCaptura { intro }

    public init(carpetaDeCapturas: URL = carpetaDeDatos().appendingPathComponent("capturas", isDirectory: true)) {
        self.carpetaDeCapturas = carpetaDeCapturas
    }

    /// Si la captura esta en pantalla.
    public var abierta: Bool { !paneles.isEmpty || abriendo }

    /// Empieza una captura: comprueba el permiso, hace la foto de cada
    /// pantalla y pone encima la superposicion. Si ya hay una, no hace nada.
    /// Con `paraGrabar`, Intro graba la zona en video en vez de copiarla.
    public func capturar(paraGrabar: Bool = false) {
        guard !abierta else { return }
        guard PermisoCaptura.asegurar() else { return }
        intro = paraGrabar && alGrabar != nil ? .grabar : .copiar
        abriendo = true
        Task { @MainActor in
            defer { self.abriendo = false }
            do {
                let fotos = try await Instantanea.fotografiarTodas()
                guard !fotos.isEmpty else { return }
                self.abrir(fotos)
            } catch {
                PermisoCaptura.avisarDenegado(detalle: "No se pudo capturar la pantalla: \(error.localizedDescription)")
            }
        }
    }

    /// Cierra la captura sin hacer nada y suelta las fotos.
    public func cerrar() {
        let lista = paneles
        paneles = []
        activa = nil
        modo = .zona
        intro = .copiar
        for panel in lista {
            vista(de: panel)?.soltar()
            panel.orderOut(nil)
            panel.close()
        }
    }

    private func abrir(_ fotos: [FotoDePantalla]) {
        let ventanas = Instantanea.ventanasVisibles()
        modo = .zona
        paneles = fotos.map { PanelCaptura.abrir(foto: $0, ventanas: ventanas, capturador: self) }
        // La pantalla donde esta el raton es la que recibe el teclado.
        let raton = NSEvent.mouseLocation
        let principal = paneles.first { $0.frame.contains(raton) } ?? paneles.first
        for panel in paneles where panel !== principal { panel.orderFrontRegardless() }
        principal?.makeKeyAndOrderFront(nil)
        if let principal, let vista = vista(de: principal) {
            activa = vista
            vista.seguirRaton(a: vista.convert(principal.convertPoint(fromScreen: raton), from: nil))
        }
    }

    private func vista(de panel: NSPanel) -> VistaCaptura? {
        panel.contentView?.subviews.first as? VistaCaptura
    }

    private var vistas: [VistaCaptura] { paneles.compactMap { vista(de: $0) } }

    // MARK: - Lo que avisan las vistas

    /// Si una accion tiene a alguien detras (si no, su boton sale apagado).
    func disponible(_ accion: AccionCaptura) -> Bool {
        switch accion {
        case .copiar, .guardar, .cerrar: true
        case .pinear: alPinear != nil
        case .alChat: alMandarAlChat != nil
        case .texto: alSacarTexto != nil
        case .grabar: alGrabar != nil
        }
    }

    /// El raton entro en otra pantalla.
    func activar(_ vista: VistaCaptura) {
        activa = vista
    }

    /// Se empezo una zona en `vista`: las demas pantallas se vacian.
    func empezo(en vista: VistaCaptura) {
        activa = vista
        for otra in vistas where otra !== vista && otra.elegida { otra.reiniciar() }
    }

    /// El teclado, venga de la pantalla que venga. Devuelve si se uso.
    func tecla(_ e: NSEvent) -> Bool {
        guard let vista = activa ?? vistas.first else { return false }
        let mods = e.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let conMando = mods.contains(.command) || mods.contains(.control)
        let letra = (e.charactersIgnoringModifiers ?? "").lowercased()

        switch e.keyCode {
        case 53: // Esc
            cerrar()
            return true
        case 36, 76: // Intro y el Intro del teclado numerico
            if vista.elegida { accion(intro, en: vista) }
            return true
        default:
            break
        }

        if conMando {
            guard vista.elegida else { return false }
            let accion: AccionCaptura?
            switch letra {
            case "c": accion = .copiar
            case "p": accion = .pinear
            case "m": accion = .alChat
            case "t": accion = .texto
            case "s": accion = .guardar
            case "g": accion = .grabar
            default: accion = nil
            }
            guard let accion else { return false }
            self.accion(accion, en: vista)
            return true
        }

        guard mods.isDisjoint(with: [.option, .shift]) else { return false }
        switch letra {
        case "z": cambiarModo(.zona)
        case "v": cambiarModo(.ventana)
        case "t": cambiarModo(.texto)
        case "p":
            cambiarModo(.pantalla)
            vista.elegirPantalla()
        case "l":
            // Pin en vivo de la zona elegida (como el modo `L` de Windows).
            guard vista.elegida, let pantalla = vista.window?.screen, let alPinearEnVivo else {
                NSSound.beep()
                return true
            }
            let zona = vista.seleccion.standardized.offsetBy(dx: pantalla.frame.minX, dy: pantalla.frame.minY)
            cerrar()
            alPinearEnVivo(zona, pantalla)
        case "s", "g":
            // Con scroll y GIF: aun no en Mac.
            NSSound.beep()
        default:
            return false
        }
        return true
    }

    private func cambiarModo(_ nuevo: ModoCaptura) {
        modo = nuevo
        for v in vistas where v.elegida { v.reiniciar() }
    }

    /// Hace `accion` con la zona elegida en `vista` y cierra la captura.
    func accion(_ accion: AccionCaptura, en vista: VistaCaptura) {
        if accion == .cerrar {
            cerrar()
            return
        }
        guard disponible(accion) else {
            NSSound.beep()
            return
        }
        if accion == .grabar {
            guard let zona = vista.zonaDeGrabacion() else { return }
            cerrar()
            alGrabar?(zona)
            return
        }
        guard let recorte = vista.recorte() else { return }
        let escala = vista.escalaFoto
        let entera = vista.tamFoto
        cerrar()

        switch accion {
        case .copiar:
            Capturador.copiar(recorte, escala: escala)
        case .guardar:
            guardar(recorte, escala: escala)
        case .pinear:
            alPinear?(Capturador.suelto(recorte, entera: entera))
        case .alChat:
            alMandarAlChat?(Capturador.suelto(recorte, entera: entera))
        case .texto:
            alSacarTexto?(Capturador.suelto(recorte, entera: entera))
        case .grabar, .cerrar:
            break
        }
    }

    // MARK: - Copiar y guardar

    /// Al portapapeles como PNG y TIFF (los dos que piden las apps de Mac).
    /// Se codifica fuera del hilo principal; el recorte retiene la foto
    /// solo hasta entonces.
    static func copiar(_ imagen: CGImage, escala: CGFloat) {
        Task.detached(priority: .userInitiated) {
            let png = Capturador.codificar(imagen, tipo: .png, escala: escala)
            let tiff = Capturador.codificar(imagen, tipo: .tiff, escala: escala)
            await MainActor.run {
                let tablero = NSPasteboard.general
                var tipos: [NSPasteboard.PasteboardType] = []
                if png != nil { tipos.append(.png) }
                if tiff != nil { tipos.append(.tiff) }
                guard !tipos.isEmpty else { return NSSound.beep() }
                _ = tablero.declareTypes(tipos, owner: nil)
                if let png { _ = tablero.setData(png, forType: .png) }
                if let tiff { _ = tablero.setData(tiff, forType: .tiff) }
            }
        }
    }

    /// Guarda un PNG en `carpetaDeCapturas` con el nombre de la hora.
    private func guardar(_ imagen: CGImage, escala: CGFloat) {
        let carpeta = carpetaDeCapturas
        let fecha = Date()
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
            let guardada = ruta
            await MainActor.run {
                if let guardada { self?.alGuardar?(guardada) } else { NSSound.beep() }
            }
        }
    }

    /// PNG o TIFF con ImageIO, con los PPP de la pantalla (72 x escala) para
    /// que al pegarla salga a su tamano en puntos y no al doble.
    nonisolated static func codificar(_ imagen: CGImage, tipo: UTType, escala: CGFloat) -> Data? {
        let datos = NSMutableData()
        guard let destino = CGImageDestinationCreateWithData(datos as CFMutableData, tipo.identifier as CFString, 1, nil) else {
            return nil
        }
        let ppp = 72 * max(escala, 1)
        let propiedades: [CFString: Any] = [kCGImagePropertyDPIWidth: ppp, kCGImagePropertyDPIHeight: ppp]
        CGImageDestinationAddImage(destino, imagen, propiedades as CFDictionary)
        guard CGImageDestinationFinalize(destino) else { return nil }
        return datos as Data
    }

    /// El recorte listo para vivir mas que la captura: si es pequeno frente
    /// a la foto entera, se copian solo sus pixeles (y la foto se puede
    /// soltar); si es casi toda, se queda compartido (sin copiar nada).
    static func suelto(_ recorte: CGImage, entera: CGSize) -> CGImage {
        let tam = CGSize(width: recorte.width, height: recorte.height)
        guard GeometriaCaptura.convieneCopiarAparte(recorte: tam, entera: entera) else { return recorte }
        let espacio = recorte.colorSpace ?? CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB()
        let info = CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        guard let lienzo = CGContext(data: nil, width: recorte.width, height: recorte.height, bitsPerComponent: 8,
                                     bytesPerRow: 0, space: espacio, bitmapInfo: info) else { return recorte }
        lienzo.interpolationQuality = .none
        lienzo.draw(recorte, in: CGRect(origin: .zero, size: tam))
        return lienzo.makeImage() ?? recorte
    }
}
