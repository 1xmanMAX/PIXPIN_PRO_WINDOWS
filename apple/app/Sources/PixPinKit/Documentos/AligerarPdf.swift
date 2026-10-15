// **Aligerar los PDF del chat** con pdfsqueeze (Rust, `aligerarEnSuSitio`),
// como Windows (`aligerar.rs`) y el movil:
//
// - Al entrar un PDF al chat **no se espera a nadie**: se guarda tal cual y
//   se aligera despues, en una cola de un solo trabajador con prioridad
//   `.utility` (nucleos de eficiencia). Solo si esta encendido en
//   `pixpinmax.toml` (`[pdf] aligerar_al_entrar`, el mismo ajuste de
//   Windows) y con Exacto o Medio (`nivelParaEntrar`).
// - «Aligerar el PDF» del menu del mensaje va delante, con el nivel de los
//   ajustes, aunque al entrar este apagado.
// - Rust solo lo cambia si baja de verdad, con las mismas paginas, y si
//   nadie lo toco mientras tanto; apunta su huella para no repetir.

import AppKit
import Foundation
import PixPinCore

public extension Notification.Name {
    /// Un PDF del chat cambio en su sitio al aligerarse: `object` es su ruta.
    static let pixpinPdfAligerado = Notification.Name("PixPinPdfAligerado")
}

/// Lo que se ensena al acabar.
public func textoDeAligerado(_ d: DesenlaceAligerar) -> String {
    switch d {
    case let .aligerado(antes, despues):
        let f = ByteCountFormatter()
        f.countStyle = .file
        let ahorro = antes > 0 ? Int((1 - Double(despues) / Double(antes)) * 100) : 0
        return "Aligerado: \(f.string(fromByteCount: Int64(antes))) → \(f.string(fromByteCount: Int64(despues))) (−\(ahorro) %)"
    case .sinMejora: return "Ya estaba ligero."
    case .yaEstaba: return "Ya se había aligerado."
    case .cifrado: return "Está cifrado: no se toca."
    case .cancelado: return "Cancelado."
    case let .fallo(detalle): return "No se pudo aligerar: \(detalle)"
    }
}

/// El avance que manda Rust desde sus hilos, al hilo principal.
final class AvanceAlPrincipal: AvanceAligerar, @unchecked Sendable {
    private let ruta: String
    private let cerrojo = NSLock()
    private var cancelado = false

    init(ruta: String) { self.ruta = ruta }

    func cancelar() {
        cerrojo.lock()
        cancelado = true
        cerrojo.unlock()
    }

    func avance(porCiento: UInt8) -> Bool {
        let ruta = self.ruta
        DispatchQueue.main.async {
            MainActor.assumeIsolated { ColaAligerar.compartida.progreso[ruta] = porCiento }
        }
        cerrojo.lock()
        defer { cerrojo.unlock() }
        return !cancelado
    }
}

@MainActor
public final class ColaAligerar: ObservableObject {
    public static let compartida = ColaAligerar()

    /// Lo que va por cada ruta (0-100) mientras se aligera.
    @Published public internal(set) var progreso: [String: UInt8] = [:]
    /// Como acabo cada ruta (para ensenarlo en su burbuja).
    @Published public private(set) var resultado: [String: String] = [:]

    private let cola = DispatchQueue(label: "pixpin.aligerar", qos: .utility)
    private var enCola: Set<String> = []
    private var avances: [String: AvanceAlPrincipal] = [:]

    /// **Un PDF acaba de entrar al chat**: se aligera despues si esta
    /// encendido en Ajustes. No lee el fichero aqui.
    public func alEntrar(_ url: URL, nucleo: Nucleo) {
        encolar(url, nucleo: nucleo, aMano: false)
    }

    /// **«Aligerar el PDF»** del menu del mensaje.
    public func aMano(_ url: URL, nucleo: Nucleo) {
        encolar(url, nucleo: nucleo, aMano: true)
    }

    public func cancelar(_ url: URL) {
        avances[url.path]?.cancelar()
    }

    public func pendiente(_ url: URL) -> Bool {
        enCola.contains(url.path)
    }

    private func encolar(_ url: URL, nucleo: Nucleo, aMano: Bool) {
        let ruta = url.path
        guard url.pathExtension.lowercased() == "pdf", !enCola.contains(ruta) else { return }
        enCola.insert(ruta)
        resultado[ruta] = nil
        let avance = AvanceAlPrincipal(ruta: ruta)
        avances[ruta] = avance
        cola.async {
            let ajustes = try? nucleo.ajustesPdf()
            let nivel: NivelAligerar
            if aMano {
                nivel = ajustes?.nivel ?? .equilibrado
            } else {
                guard let a = ajustes, a.aligerarAlEntrar else {
                    DispatchQueue.main.async {
                        MainActor.assumeIsolated { ColaAligerar.compartida.acabar(ruta, nil) }
                    }
                    return
                }
                nivel = nivelParaEntrar(elegido: a.nivel)
            }
            let hecho = nucleo.aligerarEnSuSitio(ruta: ruta, nivel: nivel, aMano: aMano, avance: avance)
            DispatchQueue.main.async {
                MainActor.assumeIsolated { ColaAligerar.compartida.acabar(ruta, hecho) }
            }
        }
    }

    private func acabar(_ ruta: String, _ hecho: DesenlaceAligerar?) {
        enCola.remove(ruta)
        avances[ruta] = nil
        progreso[ruta] = nil
        guard let hecho else { return }
        // Al entrar, lo que no cambio nada no se dice.
        switch hecho {
        case .aligerado:
            resultado[ruta] = textoDeAligerado(hecho)
            NotificationCenter.default.post(name: .pixpinPdfAligerado, object: ruta)
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        default:
            resultado[ruta] = textoDeAligerado(hecho)
        }
    }
}
