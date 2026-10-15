// El asistente del Mac: un chat con el motor que haya, en este orden:
//
// 1. Foundation Models (macOS 26 con Apple Intelligence), ver
//    `MotorApple.swift`.
// 2. Un `llama-server` del usuario en 127.0.0.1:8080: el nucleo de Rust le
//    habla con el cliente de `pixpin-ia` y hace el turno entero con sus
//    herramientas (`llamaLocalPreguntarEnVivo`). En vivo: el texto llega
//    por tandas a un `OyenteIa` y se puede parar (`CorteIa`).
// 3. Si no hay ninguno, se dice claro que no hay motor y como tener uno.
//
// Cada turno va en una `Task.detached` con prioridad `.utility` (§4bis.3):
// el modelo tarda segundos y no puede tocar el hilo de la interfaz.

import Foundation
import PixPinCore

@MainActor
public final class ModeloAsistente: ObservableObject {
    /// Que motor contesta.
    public enum Motor: Equatable, Sendable {
        case apple
        case llamaLocal(UInt16)
        case ninguno

        public var nombre: String {
            switch self {
            case .apple: return "Apple Intelligence (en este Mac)"
            case let .llamaLocal(p): return "llama-server local (127.0.0.1:\(p))"
            case .ninguno: return "Sin motor"
            }
        }
    }

    /// Una burbuja del chat.
    public struct Burbuja: Identifiable, Equatable, Sendable {
        public let id = UUID()
        public var delUsuario: Bool
        public var texto: String
        public var herramientas: [HerramientaUsada] = []
        /// Lo dibujado, como `.excalidraw` (se puede guardar en el chat).
        public var lienzo: String?
        /// Un aviso de la app (no lo dijo el modelo): no vuelve al modelo.
        public var esAviso = false
    }

    public static let textoSinMotor = """
    No hay motor para el asistente en este Mac. Opciones:
    • macOS 26 con Apple Intelligence activado (Ajustes del Sistema > Apple Intelligence y Siri).
    • O un llama-server propio escuchando en 127.0.0.1:8080 (por ejemplo: llama-server -m modelo.gguf --port 8080).
    """

    @Published public private(set) var burbujas: [Burbuja] = []
    @Published public private(set) var ocupado = false
    @Published public private(set) var motor: Motor?

    public let puertoLlama: UInt16
    private let usarApple: Bool
    private weak var almacen: Almacen?
    /// Lo que se manda como historial, como mucho.
    static let historialMaximo = 8

    public init(almacen: Almacen?, puertoLlama: UInt16 = 8080, usarApple: Bool = true) {
        self.almacen = almacen
        self.puertoLlama = puertoLlama
        self.usarApple = usarApple
    }

    /// Mira que motor hay ahora (Apple primero, luego llama-server). Barato:
    /// una consulta al sistema y, si hace falta, un `GET /health` local.
    @discardableResult
    public func comprobarMotor() async -> Motor {
        let elegido: Motor
        if usarApple, motorAppleDisponible() {
            elegido = .apple
        } else {
            let puerto = puertoLlama
            let hay = await Task.detached(priority: .utility) { llamaLocalActivo(puerto: puerto) }.value
            elegido = hay ? .llamaLocal(puerto) : .ninguno
        }
        motor = elegido
        return elegido
    }

    /// Manda un pedido y espera la respuesta entera.
    public func enviar(_ texto: String) async {
        let pedido = texto.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !pedido.isEmpty, !ocupado else { return }
        let historial = burbujas.filter { !$0.esAviso && !$0.texto.isEmpty }
            .suffix(ModeloAsistente.historialMaximo)
            .map { MensajeIa(delUsuario: $0.delUsuario, texto: $0.texto) }
        burbujas.append(Burbuja(delUsuario: true, texto: pedido))
        ocupado = true
        defer { ocupado = false }
        let motor = await comprobarMotor()
        do {
            let r: RespuestaIa
            switch motor {
            case .apple:
                guard let hecha = try await preguntarAApple(pedido, historial: Array(historial)) else {
                    burbujas.append(Burbuja(delUsuario: false, texto: ModeloAsistente.textoSinMotor, esAviso: true))
                    return
                }
                r = hecha
            case let .llamaLocal(puerto):
                await preguntarEnVivo(puerto: puerto, historial: Array(historial), pedido: pedido)
                return
            case .ninguno:
                burbujas.append(Burbuja(delUsuario: false, texto: ModeloAsistente.textoSinMotor, esAviso: true))
                return
            }
            burbujas.append(Burbuja(delUsuario: false, texto: r.texto, herramientas: r.herramientas, lienzo: r.lienzo))
        } catch {
            burbujas.append(Burbuja(delUsuario: false, texto: ModeloAjustes.texto(de: error), esAviso: true))
        }
    }

    /// Empieza otra conversacion (lo de antes no se manda mas).
    public func nuevaConversacion() {
        burbujas = []
    }

    // MARK: En vivo (llama-server)

    /// El corte del turno en vivo que esta en marcha.
    private var corte: CorteIa?
    /// Las burbujas en vivo que ya tienen su texto final: lo que llegue
    /// tarde por el oyente no se pega dos veces.
    private var cerradas: Set<UUID> = []

    /// Si hay un turno en vivo que se puede parar.
    public var sePuedeParar: Bool { corte != nil }

    /// «Parar»: el modelo deja de escribir y queda lo que llevaba.
    public func parar() {
        corte?.cancelar()
    }

    /// Un turno con el `llama-server` del usuario: la burbuja del asistente
    /// aparece vacia y se va llenando con las tandas que manda Rust (cada
    /// ~60 ms como mucho, nunca una llamada por palabra); al acabar se pone
    /// la respuesta entera, con sus herramientas y lo dibujado.
    func preguntarEnVivo(puerto: UInt16, historial: [MensajeIa], pedido: String) async {
        let corte = CorteIa.nuevo()
        self.corte = corte
        defer { self.corte = nil }
        let burbuja = Burbuja(delUsuario: false, texto: "")
        let id = burbuja.id
        burbujas.append(burbuja)
        let oyente = OyenteDelAsistente(
            alTexto: { [weak self] trozo in
                DispatchQueue.main.async {
                    MainActor.assumeIsolated { self?.pegar(trozo, en: id) }
                }
            },
            alHerramienta: { [weak self] usada in
                DispatchQueue.main.async {
                    MainActor.assumeIsolated { self?.anadir(usada, en: id) }
                }
            })
        do {
            let r = try await Task.detached(priority: .utility) {
                try llamaLocalPreguntarEnVivo(puerto: puerto, historial: historial, pedido: pedido,
                                              oyente: oyente, corte: corte)
            }.value
            cerrar(id) { b in
                b.texto = r.texto.isEmpty && corte.cortado() ? "(parado)" : r.texto
                b.herramientas = r.herramientas
                b.lienzo = r.lienzo
            }
        } catch {
            cerrar(id) { b in
                b.texto = ModeloAjustes.texto(de: error)
                b.esAviso = true
            }
        }
    }

    func pegar(_ trozo: String, en id: UUID) {
        guard !cerradas.contains(id), let i = burbujas.firstIndex(where: { $0.id == id }) else { return }
        burbujas[i].texto += trozo
    }

    func anadir(_ usada: HerramientaUsada, en id: UUID) {
        guard !cerradas.contains(id), let i = burbujas.firstIndex(where: { $0.id == id }) else { return }
        burbujas[i].herramientas.append(usada)
    }

    private func cerrar(_ id: UUID, _ cambio: (inout Burbuja) -> Void) {
        cerradas.insert(id)
        guard let i = burbujas.firstIndex(where: { $0.id == id }) else { return }
        cambio(&burbujas[i])
    }

    /// Guarda lo dibujado en «Mensajes guardados» como un `.excalidraw`
    /// (el formato del lienzo de Windows y Android).
    @discardableResult
    public func guardarDibujo(_ b: Burbuja) async -> Bool {
        guard let lienzo = b.lienzo, let almacen else { return false }
        let carpeta = FileManager.default.temporaryDirectory
            .appendingPathComponent("pixpin-asistente-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: carpeta) }
        do {
            try FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
            let ms = Int64(Date().timeIntervalSince1970 * 1000)
            let fichero = carpeta.appendingPathComponent("asistente-\(ms).excalidraw")
            try Data(lienzo.utf8).write(to: fichero)
            let nucleo = almacen.nucleo
            let guardados = try await Task.detached(priority: .utility) { try nucleo.chatGuardados() }.value
            return !(await almacen.adjuntar(urls: [fichero], en: guardados.id)).isEmpty
        } catch {
            return false
        }
    }
}
