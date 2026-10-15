// El atajo de capturar de ESTE Mac (Ctrl+Option+X de fabrica). Es de cada
// aparato, asi que vive en UserDefaults y no en `pixpinmax.toml`: un atajo
// de Carbon no significa nada en Windows.
//
// Cambiarlo en Ajustes vuelve a registrar el atajo de Carbon en caliente
// (`AtajoGlobal.cambiar(a:)`); si otra app ya lo tiene, se queda el de antes
// y se dice.

import AppKit
import Carbon.HIToolbox
import Foundation

@MainActor
public final class AtajoDeCaptura: ObservableObject {
    /// La combinacion en uso (la guardada, o la de fabrica).
    @Published public private(set) var combinacion: AtajoGlobal.Combinacion
    /// Si el sistema dejo registrarla (si no, otra app la tiene).
    @Published public private(set) var registrado: Bool

    static let claveTecla = "atajoCaptura.tecla"
    static let claveModificadores = "atajoCaptura.modificadores"

    private let defaults: UserDefaults
    private var atajo: AtajoGlobal?

    /// Lee la combinacion guardada y la registra. `registrar: false` solo
    /// para las pruebas (no toca Carbon).
    public init(defaults: UserDefaults = .standard, registrar: Bool = true,
                accion: @escaping @MainActor () -> Void) {
        self.defaults = defaults
        let guardada = AtajoDeCaptura.leer(defaults) ?? .capturar
        combinacion = guardada
        registrado = false
        guard registrar else { return }
        if let a = AtajoGlobal(guardada, accion: accion) {
            atajo = a
            registrado = true
        } else if guardada != .capturar, let a = AtajoGlobal(.capturar, accion: accion) {
            // La guardada ya no se puede (otra app la tomo): la de fabrica.
            atajo = a
            combinacion = .capturar
            registrado = true
        }
    }

    /// Cambia el atajo y lo guarda. `false` si el sistema no lo deja (se
    /// queda el de antes).
    @discardableResult
    public func cambiar(a nueva: AtajoGlobal.Combinacion) -> Bool {
        guard AtajoDeCaptura.vale(nueva) else { return false }
        if let atajo {
            guard atajo.cambiar(a: nueva) else { return false }
        }
        combinacion = nueva
        AtajoDeCaptura.guardar(nueva, en: defaults)
        return true
    }

    /// Vuelve a Ctrl+Option+X.
    @discardableResult
    public func restablecer() -> Bool {
        let ok = cambiar(a: .capturar)
        if ok {
            defaults.removeObject(forKey: AtajoDeCaptura.claveTecla)
            defaults.removeObject(forKey: AtajoDeCaptura.claveModificadores)
        }
        return ok
    }

    // MARK: Guardar y leer

    static func leer(_ d: UserDefaults) -> AtajoGlobal.Combinacion? {
        guard d.object(forKey: claveTecla) != nil, d.object(forKey: claveModificadores) != nil else { return nil }
        let c = AtajoGlobal.Combinacion(tecla: UInt32(truncatingIfNeeded: d.integer(forKey: claveTecla)),
                                        modificadores: UInt32(truncatingIfNeeded: d.integer(forKey: claveModificadores)))
        return vale(c) ? c : nil
    }

    static func guardar(_ c: AtajoGlobal.Combinacion, en d: UserDefaults) {
        d.set(Int(c.tecla), forKey: claveTecla)
        d.set(Int(c.modificadores), forKey: claveModificadores)
    }

    /// Un atajo global necesita Control, Option o Comando: una letra sola
    /// (o con Mayusculas) se comeria lo que el usuario escribe en otras apps.
    public nonisolated static func vale(_ c: AtajoGlobal.Combinacion) -> Bool {
        let fuertes = UInt32(controlKey | optionKey | cmdKey)
        return c.modificadores & fuertes != 0
    }
}

// MARK: - Como se ensena y como se graba

public extension AtajoGlobal.Combinacion {
    /// Los modificadores de Carbon a partir de los de una pulsacion.
    static func modificadores(de flags: NSEvent.ModifierFlags) -> UInt32 {
        var m = 0
        if flags.contains(.control) { m |= controlKey }
        if flags.contains(.option) { m |= optionKey }
        if flags.contains(.shift) { m |= shiftKey }
        if flags.contains(.command) { m |= cmdKey }
        return UInt32(m)
    }

    /// Como lo ensena el Mac: `⌃⌥X`.
    var descripcion: String {
        var s = ""
        if modificadores & UInt32(controlKey) != 0 { s += "⌃" }
        if modificadores & UInt32(optionKey) != 0 { s += "⌥" }
        if modificadores & UInt32(shiftKey) != 0 { s += "⇧" }
        if modificadores & UInt32(cmdKey) != 0 { s += "⌘" }
        return s + AtajoGlobal.Combinacion.nombreDeTecla(tecla)
    }

    /// El nombre de una tecla por su codigo virtual (teclado ANSI: el codigo
    /// es la posicion, no la letra impresa).
    static func nombreDeTecla(_ codigo: UInt32) -> String {
        if let n = nombres[Int(codigo)] { return n }
        return "#\(codigo)"
    }

    private static let nombres: [Int: String] = {
        var t: [Int: String] = [
            kVK_ANSI_A: "A", kVK_ANSI_B: "B", kVK_ANSI_C: "C", kVK_ANSI_D: "D", kVK_ANSI_E: "E",
            kVK_ANSI_F: "F", kVK_ANSI_G: "G", kVK_ANSI_H: "H", kVK_ANSI_I: "I", kVK_ANSI_J: "J",
            kVK_ANSI_K: "K", kVK_ANSI_L: "L", kVK_ANSI_M: "M", kVK_ANSI_N: "N", kVK_ANSI_O: "O",
            kVK_ANSI_P: "P", kVK_ANSI_Q: "Q", kVK_ANSI_R: "R", kVK_ANSI_S: "S", kVK_ANSI_T: "T",
            kVK_ANSI_U: "U", kVK_ANSI_V: "V", kVK_ANSI_W: "W", kVK_ANSI_X: "X", kVK_ANSI_Y: "Y",
            kVK_ANSI_Z: "Z",
            kVK_ANSI_0: "0", kVK_ANSI_1: "1", kVK_ANSI_2: "2", kVK_ANSI_3: "3", kVK_ANSI_4: "4",
            kVK_ANSI_5: "5", kVK_ANSI_6: "6", kVK_ANSI_7: "7", kVK_ANSI_8: "8", kVK_ANSI_9: "9",
            kVK_Space: "Espacio", kVK_Return: "↩", kVK_Tab: "⇥", kVK_Delete: "⌫",
            kVK_ANSI_Minus: "-", kVK_ANSI_Equal: "=", kVK_ANSI_LeftBracket: "[",
            kVK_ANSI_RightBracket: "]", kVK_ANSI_Semicolon: ";", kVK_ANSI_Quote: "'",
            kVK_ANSI_Comma: ",", kVK_ANSI_Period: ".", kVK_ANSI_Slash: "/",
            kVK_ANSI_Backslash: "\\", kVK_ANSI_Grave: "`",
            kVK_LeftArrow: "←", kVK_RightArrow: "→", kVK_UpArrow: "↑", kVK_DownArrow: "↓",
        ]
        let funciones = [kVK_F1, kVK_F2, kVK_F3, kVK_F4, kVK_F5, kVK_F6,
                         kVK_F7, kVK_F8, kVK_F9, kVK_F10, kVK_F11, kVK_F12]
        for (i, f) in funciones.enumerated() { t[f] = "F\(i + 1)" }
        return t
    }()
}
