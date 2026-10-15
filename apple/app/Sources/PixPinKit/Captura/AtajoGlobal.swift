// Un atajo de teclado que funciona en todo el Mac (de fabrica, capturar con
// Ctrl+Option+X, como el Ctrl+Alt+X de Windows).
//
// Con `RegisterEventHotKey` de Carbon: es lo que usan las apps de capturas
// y no pide el permiso de Accesibilidad (un `NSEvent` global si lo pediria).
// No hay nada sondeando: el sistema avisa solo cuando se pulsa.

import Carbon.HIToolbox
import Foundation

/// La firma de los atajos de PixPin ('PxPn'): asi el manejador sabe que el
/// aviso es nuestro. Global (y no `static`) para leerla desde la funcion C.
private let firmaPixPin: OSType = 0x5078_506E

@MainActor
public final class AtajoGlobal {
    /// Una tecla (codigo virtual, `kVK_...`) y sus modificadores de Carbon
    /// (`controlKey`, `optionKey`, `cmdKey`, `shiftKey`).
    public struct Combinacion: Equatable, Sendable {
        public var tecla: UInt32
        public var modificadores: UInt32

        public init(tecla: UInt32, modificadores: UInt32) {
            self.tecla = tecla
            self.modificadores = modificadores
        }

        /// Ctrl+Option+X: capturar una zona.
        public static let capturar = Combinacion(tecla: UInt32(kVK_ANSI_X),
                                                 modificadores: UInt32(controlKey | optionKey))
    }

    /// Los atajos vivos por su numero (debiles: soltar el atajo lo quita).
    private struct Debil { weak var atajo: AtajoGlobal? }
    private static var vivos: [UInt32: Debil] = [:]
    private static var siguiente: UInt32 = 1
    private static var manejador: EventHandlerRef?

    public private(set) var combinacion: Combinacion
    private let numero: UInt32
    private let accion: @MainActor () -> Void
    nonisolated(unsafe) private var registro: EventHotKeyRef?

    /// Registra el atajo; `nil` si el sistema no lo deja (otra app ya lo
    /// tiene). Vive mientras se guarde el objeto.
    public init?(_ combinacion: Combinacion, accion: @escaping @MainActor () -> Void) {
        AtajoGlobal.instalarManejador()
        self.combinacion = combinacion
        self.accion = accion
        numero = AtajoGlobal.siguiente
        AtajoGlobal.siguiente += 1
        guard let ref = AtajoGlobal.registrar(combinacion, numero: numero) else { return nil }
        registro = ref
        AtajoGlobal.vivos[numero] = Debil(atajo: self)
    }

    /// **Cambia la combinacion** (la de Ajustes): quita la de antes y
    /// registra la nueva con el mismo numero. Si el sistema no deja la
    /// nueva (otra app la tiene), vuelve a poner la de antes y da `false`.
    @discardableResult
    public func cambiar(a nueva: Combinacion) -> Bool {
        if nueva == combinacion, registro != nil { return true }
        if let registro { UnregisterEventHotKey(registro) }
        registro = nil
        if let ref = AtajoGlobal.registrar(nueva, numero: numero) {
            registro = ref
            combinacion = nueva
            AtajoGlobal.vivos[numero] = Debil(atajo: self)
            return true
        }
        registro = AtajoGlobal.registrar(combinacion, numero: numero)
        if registro != nil { AtajoGlobal.vivos[numero] = Debil(atajo: self) }
        return false
    }

    /// Registra una combinacion en Carbon; `nil` si el sistema no la deja.
    private static func registrar(_ c: Combinacion, numero: UInt32) -> EventHotKeyRef? {
        var ref: EventHotKeyRef?
        let estado = RegisterEventHotKey(c.tecla, c.modificadores,
                                         EventHotKeyID(signature: firmaPixPin, id: numero),
                                         GetApplicationEventTarget(), 0, &ref)
        guard estado == OSStatus(noErr) else { return nil }
        return ref
    }

    /// Quita el atajo ya (sin esperar a soltar el objeto).
    public func quitar() {
        if let registro { UnregisterEventHotKey(registro) }
        registro = nil
        AtajoGlobal.vivos[numero] = nil
    }

    deinit {
        if let registro { UnregisterEventHotKey(registro) }
    }

    fileprivate static func disparar(_ numero: UInt32) {
        guard let atajo = vivos[numero]?.atajo else {
            vivos[numero] = nil
            return
        }
        atajo.accion()
    }

    /// Un solo manejador para todos los atajos, instalado la primera vez.
    /// Carbon lo llama en el hilo principal.
    private static func instalarManejador() {
        guard manejador == nil else { return }
        var tipo = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        var ref: EventHandlerRef?
        let estado = InstallEventHandler(GetApplicationEventTarget(), manejarAtajoCarbon, 1, &tipo, nil, &ref)
        if estado == OSStatus(noErr) { manejador = ref }
    }
}

/// La funcion C que llama Carbon al pulsar un atajo registrado. Global para
/// que no herede el actor principal: dentro se comprueba que es nuestro y se
/// pasa al actor principal (Carbon ya llama en el hilo principal).
private func manejarAtajoCarbon(_ llamada: EventHandlerCallRef?, _ evento: EventRef?,
                                _ datos: UnsafeMutableRawPointer?) -> OSStatus {
    guard let evento else { return OSStatus(eventNotHandledErr) }
    var id = EventHotKeyID()
    let leido = GetEventParameter(evento, EventParamName(kEventParamDirectObject),
                                  EventParamType(typeEventHotKeyID), nil,
                                  MemoryLayout<EventHotKeyID>.size, nil, &id)
    guard leido == OSStatus(noErr), id.signature == firmaPixPin else {
        return OSStatus(eventNotHandledErr)
    }
    let numero = id.id
    MainActor.assumeIsolated { AtajoGlobal.disparar(numero) }
    return OSStatus(noErr)
}
