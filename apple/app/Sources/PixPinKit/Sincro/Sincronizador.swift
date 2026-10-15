// Sincronizar este Mac con los demas aparatos del grupo (PC, movil, otro
// Mac), como «Sincronizar» de Windows y su sincronizacion sola
// (`sincronizar/automatica.rs`).
//
// - **Presencia** (`presencia.rs` de Windows): mientras la app vive y el Mac
//   esta en un grupo, escucha (`Puerta`), se anuncia (`Anunciador`) y mira
//   quien del grupo hay en la Wi-Fi (`Vecinos`).
// - **Cuando**: a mano («Sincronizar ahora»), al aparecer un aparato del
//   grupo y cuando los cambios de aqui llevan `Calma.quieto` (30 s) quietos.
//   Lo automatico se apaga con el modo de bajo consumo.
// - **Como**: cada vuelta es `Nucleo.sincronizarCon(host:puerto:)` en una
//   `Task.detached` con prioridad `.utility` (§4bis.3), una detras de otra.
//
// Nada sondea (§4bis.6): Bonjour avisa de quien aparece, el cambio de modo
// de bajo consumo llega por notificacion y la espera de «quieto» es un solo
// sueno que se rearma con cada cambio.

import Foundation
import PixPinCore

public extension Notification.Name {
    /// Algo cambio en los chats de este aparato (una nota, un chat nuevo):
    /// la sincronizacion sola cuenta desde aqui sus 30 s de calma.
    static let pixpinCambioLocal = Notification.Name("PixPinCambioLocal")
    /// Llego de otro aparato una foto para el lienzo que esta delante
    /// (`FrenteMac.lienzo`). `object` es el `ArchivoLlegado` (su `ruta`).
    static let pixpinFotoAlLienzo = Notification.Name("PixPinFotoAlLienzo")
}

@MainActor
public final class Sincronizador: ObservableObject {
    /// Como esta cada aparato del grupo visto en la red.
    public enum Estado: Equatable, Sendable {
        case mirando
        case contesta
        case noContesta
        case sincronizando
        case hecho(String)
        case fallo(String)
    }

    public struct Fila: Identifiable, Equatable, Sendable {
        public var id: String
        public var nombre: String
        public var letra: String
        public var direccion: Direccion?
        public var estado: Estado
    }

    @Published public private(set) var aparato: InfoAparato?
    /// El codigo del grupo como se lee (`K7Q2M-9XMPA`), si hay grupo.
    @Published public private(set) var codigoLegible: String?
    @Published public private(set) var filas: [Fila] = []
    /// Una vuelta (o una union) en marcha.
    @Published public private(set) var ocupado = false
    /// Lo que se esta haciendo, en palabras («Buscando…»).
    @Published public private(set) var fase: String?
    /// La frase de la ultima vuelta (`ResumenSincro.texto`).
    @Published public private(set) var ultimoResultado: String?
    @Published public private(set) var error: String?
    @Published public private(set) var bajoConsumo: Bool
    /// El puerto donde escucha este Mac (para escribirlo a mano en otro).
    @Published public private(set) var puerto: UInt16?
    /// La sincronizacion sola (al aparecer y tras los cambios).
    @Published public var automatica = true {
        didSet { reprogramarQuieto() }
    }

    /// Como va «Enviar a…» con cada aparato (por su id).
    @Published public private(set) var envios: [String: EstadoEnvioSuelto] = [:]
    /// Lo ultimo que llego suelto de otro aparato (para ensenarlo).
    @Published public private(set) var ultimasLlegadas: [ArchivoLlegado] = []

    /// Se llama cuando una vuelta trajo algo (para releer la lista de chats).
    public var alTraerCambios: (@MainActor () -> Void)?

    public let nucleo: Nucleo
    public let vecinos = Vecinos()
    private let anunciador = Anunciador()
    private var puerta: Puerta?
    private var etiqueta: String?
    private var calma = Calma()
    private var esperaQuieto: Task<Void, Never>?
    private var ultimaVuelta: [String: Date] = [:]
    /// Una vuelta con todos pedida mientras habia otra en marcha (y por que).
    private var pendienteConTodos: MotivoSincro?
    private var observadores: [NSObjectProtocol] = []
    private let reloj: () -> Date

    public init(nucleo: Nucleo, reloj: @escaping () -> Date = { Date() }) {
        self.nucleo = nucleo
        self.reloj = reloj
        bajoConsumo = ProcessInfo.processInfo.isLowPowerModeEnabled
    }

    /// Arranca con la app: lee quien es este aparato y, si esta en un grupo,
    /// abre la puerta, se anuncia y busca a los suyos.
    public func arrancar() {
        guard observadores.isEmpty else { return }
        let centro = NotificationCenter.default
        observadores.append(centro.addObserver(forName: .NSProcessInfoPowerStateDidChange, object: nil,
                                               queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.cambioDeEnergia() }
        })
        observadores.append(centro.addObserver(forName: .pixpinCambioLocal, object: nil,
                                               queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.cambioLocal() }
        })
        vecinos.alCambiar = { [weak self] _ in self?.vecinosCambiaron() }
        Task { await refrescar() }
    }

    /// Lo deja todo: deja de anunciarse, de escuchar y de buscar.
    public func parar() {
        esperaQuieto?.cancel()
        esperaQuieto = nil
        anunciador.retirar()
        puerta?.cerrar()
        puerta = nil
        puerto = nil
        vecinos.parar()
        for o in observadores { NotificationCenter.default.removeObserver(o) }
        observadores = []
    }

    // MARK: - El grupo

    /// Crea un grupo nuevo con este Mac dentro y ensena su codigo.
    public func crearGrupo() async {
        await trabajar("Creando el grupo…") { nucleo in
            _ = try nucleo.crearGrupo()
        }
        await refrescar()
    }

    /// **Se une al grupo** del codigo `codigo`. Sin `direccion`, busca en la
    /// Wi-Fi (25 s, como Windows) un aparato que anuncie la etiqueta de ese
    /// codigo; con ella (`192.168.1.20:47474`), va directo. Despues hace la
    /// primera vuelta con el, que es la que trae los chats.
    public func unirse(codigo: String, direccion: Direccion? = nil) async {
        error = nil
        guard codigoValido(codigo: codigo) else {
            error = "El código de grupo son diez signos (como K7Q2M-9XMPA)."
            return
        }
        guard !ocupado else { return }
        ocupado = true
        defer { ocupado = false; fase = nil }
        var destino = direccion
        if destino == nil {
            fase = "Buscando un aparato del grupo en esta Wi-Fi…\nEn el otro aparato, ten PixPin abierto."
            let yo = aparato?.id ?? ""
            let suya = await Task.detached(priority: .utility) { etiquetaDeGrupo(codigo: codigo) }.value
            if let v = await vecinos.esperar(tope: 25, { $0.esDelGrupo(etiqueta: suya, yo: yo) }) {
                destino = await Vecinos.sondear(v.punto)
            }
        }
        guard let destino else {
            error = "No apareció ningún aparato con ese código. Comprueba el código, que los dos "
                + "estáis en la misma Wi-Fi y que el otro tiene PixPin abierto. También puedes "
                + "escribir su dirección."
            return
        }
        fase = "Uniéndose al grupo…"
        let nucleo = self.nucleo
        do {
            _ = try await Task.detached(priority: .utility) {
                try nucleo.unirseAGrupo(host: destino.host, puerto: destino.puerto, codigo: codigo)
            }.value
        } catch {
            self.error = textoDeError(error)
            return
        }
        await refrescar()
        fase = "Trayendo los chats…"
        await vuelta(con: destino, id: nil, nombre: nil)
    }

    // MARK: - Sincronizar

    /// «Sincronizar ahora» con un aparato (`id`) o con todos los que
    /// contestan (`nil`).
    public func sincronizarAhora(id: String? = nil) async {
        if let id {
            guard let fila = filas.first(where: { $0.id == id }) else { return }
            await sincronizar(fila, motivo: .manual)
        } else {
            await conTodos(motivo: .manual)
        }
    }

    private func conTodos(motivo: MotivoSincro) async {
        guard sePuedeSincronizar(motivo, automatica: automatica, bajoConsumo: bajoConsumo) else { return }
        if ocupado {
            // Lo manual manda sobre lo automatico.
            if pendienteConTodos != .manual { pendienteConTodos = motivo }
            return
        }
        calma.hecha()
        esperaQuieto?.cancel()
        for fila in filas where fila.direccion != nil {
            await sincronizar(fila, motivo: motivo)
        }
    }

    private func sincronizar(_ fila: Fila, motivo: MotivoSincro) async {
        guard sePuedeSincronizar(motivo, automatica: automatica, bajoConsumo: bajoConsumo) else { return }
        // La direccion puede haber cambiado desde que se vio: se sondea otra
        // vez si no la hay.
        var direccion = fila.direccion
        if direccion == nil, let v = vecinos.todos.first(where: { $0.id == fila.id }) {
            direccion = await Vecinos.sondear(v.punto)
        }
        guard let direccion else {
            poner(fila.id, .noContesta, direccion: nil)
            return
        }
        guard !ocupado else {
            if motivo != .manual, pendienteConTodos == nil { pendienteConTodos = motivo }
            return
        }
        ocupado = true
        await vuelta(con: direccion, id: fila.id, nombre: fila.nombre)
        ocupado = false
        if let pendiente = pendienteConTodos {
            pendienteConTodos = nil
            await conTodos(motivo: pendiente)
        }
    }

    /// Una vuelta con `direccion`, fuera del hilo principal.
    private func vuelta(con direccion: Direccion, id: String?, nombre: String?) async {
        if let id { poner(id, .sincronizando, direccion: direccion) }
        let nucleo = self.nucleo
        do {
            let r = try await Task.detached(priority: .utility) {
                try nucleo.sincronizarCon(host: direccion.host, puerto: direccion.puerto)
            }.value
            ultimaVuelta[r.otroId] = reloj()
            ultimoResultado = r.avisos.isEmpty ? r.texto : ([r.texto] + r.avisos).joined(separator: "\n")
            error = nil
            poner(r.otroId, .hecho(r.texto), direccion: direccion)
            if r.traidos > 0 || r.archivos > 0 { alTraerCambios?() }
        } catch {
            let texto = textoDeError(error)
            self.error = nombre.map { "\($0): \(texto)" } ?? texto
            if let id { poner(id, .fallo(texto), direccion: direccion) }
        }
    }

    // MARK: - Enviar a…

    /// **«Enviar a…» un aparato del grupo**: manda `urls` a lo que ese
    /// aparato tenga delante (su chat abierto, su lienzo si son fotos, o la
    /// conversacion general), fuera del hilo principal.
    public func enviar(_ urls: [URL], a id: String) async {
        guard !urls.isEmpty, let fila = filas.first(where: { $0.id == id }) else { return }
        var direccion = fila.direccion
        if direccion == nil, let v = vecinos.todos.first(where: { $0.id == id }) {
            direccion = await Vecinos.sondear(v.punto)
        }
        guard let direccion else {
            envios[id] = .fallo("\(fila.nombre) no contesta.")
            return
        }
        envios[id] = .enviando(urls.count)
        let nucleo = self.nucleo
        let rutas = urls.map(\.path)
        do {
            let r = try await Task.detached(priority: .utility) {
                try nucleo.enviarAAparato(host: direccion.host, puerto: direccion.puerto, rutas: rutas)
            }.value
            envios[id] = estadoDeEnvio(r, a: fila.nombre)
        } catch {
            envios[id] = .fallo(textoDeError(error))
        }
    }

    // MARK: - Presencia

    /// Relee la identidad y el anuncio (fuera del hilo principal: la
    /// etiqueta cuesta PBKDF2 la primera vez) y enciende o apaga la
    /// presencia segun haya grupo.
    public func refrescar() async {
        let nucleo = self.nucleo
        do {
            let (info, anuncio, presencia) = try await Task.detached(priority: .utility) {
                try (nucleo.aparato(), nucleo.anuncio(), nucleo.anuncioDePresencia())
            }.value
            aparato = info
            codigoLegible = info.grupo.map { PixPinCore.codigoLegible(codigo: $0) }
            etiqueta = anuncio?.etiqueta
            // Con grupo, o sin el pero dentro de un chat compartido con
            // personas (que llaman aqui): se escucha y se anuncia.
            if let presencia {
                try encender(presencia)
            } else {
                apagar()
            }
            vecinosCambiaron()
        } catch {
            self.error = textoDeError(error)
        }
    }

    private func encender(_ anuncio: AnuncioBonjour) throws {
        if puerta == nil {
            let p = try Puerta(nucleo: nucleo)
            p.arrancar { [weak self] resultado in
                DispatchQueue.main.async {
                    MainActor.assumeIsolated { self?.atendido(resultado) }
                }
            }
            puerta = p
            puerto = p.puerto
        }
        if let puerto { anunciador.anunciar(anuncio, puerto: puerto) }
        vecinos.empezar()
    }

    private func apagar() {
        anunciador.retirar()
        puerta?.cerrar()
        puerta = nil
        puerto = nil
        filas = []
    }

    /// Alguien llamo a la puerta de este Mac.
    private func atendido(_ resultado: Result<Atendido, Error>) {
        switch resultado {
        case let .success(.aparato(id, nombre)):
            // Sincronizo o se unio: puede haber chats nuevos y miembros
            // nuevos en el grupo.
            ultimaVuelta[id] = reloj()
            ultimoResultado = "\(nombre) sincronizó con este Mac."
            alTraerCambios?()
            Task { await refrescar() }
        case let .success(.persona(_, nombre)):
            // Una persona de un chat compartido se junto con este Mac.
            ultimoResultado = "\(nombre) sincronizó un chat compartido con este Mac."
            alTraerCambios?()
        case let .success(.archivos(_, nombre, llegadas)):
            ultimasLlegadas = llegadas
            ultimoResultado = textoDeLlegadas(llegadas, de: nombre)
            for l in llegadas where l.donde == .lienzo {
                NotificationCenter.default.post(name: .pixpinFotoAlLienzo, object: l)
            }
            alTraerCambios?()
        case .success:
            break
        case let .failure(e):
            NSLog("PixPin: una llamada de otro aparato fallo: %@", textoDeError(e))
        }
    }

    /// La lista de Bonjour cambio: rehace las filas de mi grupo y sondea a
    /// los nuevos (la sonda da su direccion).
    private func vecinosCambiaron() {
        guard let etiqueta, let yo = aparato?.id else {
            filas = []
            return
        }
        let antes = Dictionary(filas.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        var nuevas: [Fila] = []
        var aSondear: [Vecino] = []
        for v in vecinos.delGrupo(etiqueta: etiqueta, yo: yo) {
            if let f = antes[v.id] {
                nuevas.append(Fila(id: v.id, nombre: v.nombre, letra: v.datos.letra,
                                   direccion: f.direccion, estado: f.estado))
            } else {
                nuevas.append(Fila(id: v.id, nombre: v.nombre, letra: v.datos.letra,
                                   direccion: nil, estado: .mirando))
                aSondear.append(v)
            }
        }
        filas = nuevas
        for v in aSondear {
            Task {
                let d = await Vecinos.sondear(v.punto)
                poner(v.id, d == nil ? .noContesta : .contesta, direccion: d)
                guard d != nil, tocaPorAparecer(ultimaVuelta: ultimaVuelta[v.id], ahora: reloj()),
                      let fila = filas.first(where: { $0.id == v.id }) else { return }
                await sincronizar(fila, motivo: .aparecio)
            }
        }
    }

    private func poner(_ id: String, _ estado: Estado, direccion: Direccion?) {
        guard let i = filas.firstIndex(where: { $0.id == id }) else { return }
        filas[i].estado = estado
        if let direccion { filas[i].direccion = direccion }
    }

    // MARK: - La espera de «quieto» y el bajo consumo

    /// Algo cambio aqui: la vuelta con todos sale tras 30 s sin mas cambios.
    public func cambioLocal() {
        calma.cambio(en: reloj())
        reprogramarQuieto()
    }

    private func cambioDeEnergia() {
        bajoConsumo = ProcessInfo.processInfo.isLowPowerModeEnabled
        reprogramarQuieto()
    }

    /// Un solo sueno hasta que toque (o ninguno si no se puede): cada cambio
    /// lo cancela y lo vuelve a armar.
    private func reprogramarQuieto() {
        esperaQuieto?.cancel()
        esperaQuieto = nil
        guard etiqueta != nil,
              sePuedeSincronizar(.quieto, automatica: automatica, bajoConsumo: bajoConsumo),
              let falta = calma.falta(en: reloj()) else { return }
        esperaQuieto = Task { [weak self] in
            try? await Task.sleep(for: .seconds(falta))
            guard !Task.isCancelled, let self else { return }
            guard self.calma.toca(en: self.reloj()) else {
                self.reprogramarQuieto()
                return
            }
            await self.conTodos(motivo: .quieto)
        }
    }

    /// Hace `trabajo` con el nucleo fuera del hilo principal, con su fase.
    private func trabajar(_ texto: String, _ trabajo: @escaping @Sendable (Nucleo) throws -> Void) async {
        guard !ocupado else { return }
        ocupado = true
        fase = texto
        error = nil
        let nucleo = self.nucleo
        do {
            try await Task.detached(priority: .utility) { try trabajo(nucleo) }.value
        } catch {
            self.error = textoDeError(error)
        }
        ocupado = false
        fase = nil
    }
}
