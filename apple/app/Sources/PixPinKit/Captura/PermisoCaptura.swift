// El permiso de Grabacion de pantalla. Sin el, ScreenCaptureKit solo
// entrega el fondo del escritorio y la barra de menus: hay que pedirlo y,
// si se nego, decir claramente donde se da.

import AppKit
import CoreGraphics

@MainActor
public enum PermisoCaptura {
    private static let clavePedido = "captura.permisoPedido"

    /// Si ya se puede capturar (no pregunta nada).
    public static var concedido: Bool { CGPreflightScreenCaptureAccess() }

    /// Comprueba el permiso antes de capturar. La primera vez deja que el
    /// sistema muestre su propio aviso; las siguientes, si sigue negado,
    /// explica donde se activa. Devuelve si se puede capturar ya.
    public static func asegurar() -> Bool {
        if concedido { return true }
        let defaults = UserDefaults.standard
        if !defaults.bool(forKey: clavePedido) {
            defaults.set(true, forKey: clavePedido)
            // El aviso del sistema sale solo una vez en la vida de la app.
            if CGRequestScreenCaptureAccess() { return true }
            return false
        }
        avisarDenegado()
        return false
    }

    /// El aviso de que falta el permiso, con un boton a Ajustes.
    public static func avisarDenegado(detalle: String? = nil) {
        let alerta = NSAlert()
        alerta.alertStyle = .warning
        alerta.messageText = "PixPin no tiene permiso para capturar la pantalla"
        var texto = "Abre Ajustes del Sistema > Privacidad y seguridad > Grabacion de pantalla y audio del sistema, "
            + "activa PixPin y vuelve a abrir la app (macOS solo aplica el permiso al reiniciarla)."
        if let detalle { texto += "\n\n\(detalle)" }
        alerta.informativeText = texto
        alerta.addButton(withTitle: "Abrir Ajustes")
        alerta.addButton(withTitle: "Ahora no")
        NSApp.activate()
        if alerta.runModal() == .alertFirstButtonReturn,
           let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture") {
            NSWorkspace.shared.open(url)
        }
    }
}
