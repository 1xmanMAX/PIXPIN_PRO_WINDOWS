// Los dos cajetines de medir (`pedir_medida` y `ventana_editor/cota.rs` de
// Windows, el `DialogoDeCota` del movil):
// - **calibrar**: tras arrastrar con «Calibrar», cuanto mide de verdad esa
//   raya y en que unidad. Desde ahi todas las cotas hablan en esa unidad.
// - **la cota**: tras trazarla, su largo y su angulo, ya rellenos con lo que
//   mide. Cancelar la deja como se trazo.
// Una hoja (`beginSheetModal`) sobre la ventana del lienzo: no bloquea la app.

import AppKit
import PixPinCore

@MainActor
enum CajetinMedida {
    /// Un numero tecleado con coma o con punto. `nil` si no lo es.
    nonisolated static func numero(_ texto: String) -> Float? {
        let limpio = texto.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: ",", with: ".")
        guard let v = Float(limpio), v.isFinite else { return nil }
        return v
    }

    /// Una cifra para el cajetin, sin ceros de mas y con la coma de aqui.
    nonisolated static func escrito(_ v: Float) -> String {
        let f = NumberFormatter()
        f.locale = Locale.current
        f.minimumFractionDigits = 0
        f.maximumFractionDigits = 2
        return f.string(from: NSNumber(value: v)) ?? String(v)
    }

    private static func campo(_ texto: String) -> NSTextField {
        let c = NSTextField(string: texto)
        c.frame = NSRect(x: 0, y: 0, width: 140, height: 24)
        return c
    }

    private static func fila(_ titulo: String, _ vista: NSView) -> NSStackView {
        let etiqueta = NSTextField(labelWithString: titulo)
        etiqueta.alignment = .right
        etiqueta.widthAnchor.constraint(equalToConstant: 70).isActive = true
        let f = NSStackView(views: [etiqueta, vista])
        f.orientation = .horizontal
        f.spacing = 8
        return f
    }

    private static func mostrar(_ alerta: NSAlert, en ventana: NSWindow?, hecho: @escaping (Bool) -> Void) {
        if let ventana {
            alerta.beginSheetModal(for: ventana) { r in hecho(r == .alertFirstButtonReturn) }
        } else {
            hecho(alerta.runModal() == .alertFirstButtonReturn)
        }
    }

    /// Pregunta cuanto mide la raya de calibrar. `hecho(valor, unidad)` solo
    /// si se acepta con algo que valga.
    static func calibrar(unidades: [String], en ventana: NSWindow?,
                         hecho: @escaping (Float, String) -> Void) {
        let alerta = NSAlert()
        alerta.messageText = "¿Cuánto mide de verdad?"
        alerta.informativeText = "Desde ahora las cotas de este lienzo dirán sus medidas en esa unidad."
        let valor = campo("")
        let unidad = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 80, height: 24), pullsDown: false)
        unidad.addItems(withTitles: unidades.isEmpty ? ["m"] : unidades)
        let fila = NSStackView(views: [valor, unidad])
        fila.orientation = .horizontal
        fila.spacing = 8
        fila.frame = NSRect(x: 0, y: 0, width: 240, height: 24)
        alerta.accessoryView = fila
        alerta.addButton(withTitle: "Calibrar")
        alerta.addButton(withTitle: "Cancelar")
        alerta.window.initialFirstResponder = valor
        mostrar(alerta, en: ventana) { aceptado in
            guard aceptado, let v = numero(valor.stringValue), v > 0,
                  let u = unidad.titleOfSelectedItem else { return }
            hecho(v, u)
        }
    }

    /// Pregunta el largo y el angulo de la cota recien trazada, rellenos con
    /// lo que mide. `hecho(largo, grados)` solo si se acepta.
    static func dictarCota(largo: Float, grados: Float, unidad: String, en ventana: NSWindow?,
                           hecho: @escaping (Float, Float) -> Void) {
        let alerta = NSAlert()
        alerta.messageText = "Medida de la cota"
        alerta.informativeText = unidad.isEmpty
            ? "Sin calibrar, el largo va en píxeles del lienzo."
            : "El largo va en \(unidad). El ángulo, como en un plano: 0° a la derecha, 90° hacia arriba."
        let campoLargo = campo(escrito(largo))
        let campoGrados = campo(escrito(grados))
        let pila = NSStackView(views: [fila(unidad.isEmpty ? "Largo" : "Largo (\(unidad))", campoLargo),
                                       fila("Ángulo (°)", campoGrados)])
        pila.orientation = .vertical
        pila.alignment = .leading
        pila.spacing = 6
        pila.frame = NSRect(x: 0, y: 0, width: 260, height: 56)
        alerta.accessoryView = pila
        alerta.addButton(withTitle: "Aplicar")
        alerta.addButton(withTitle: "Cancelar")
        alerta.window.initialFirstResponder = campoLargo
        mostrar(alerta, en: ventana) { aceptado in
            guard aceptado, let l = numero(campoLargo.stringValue), l > 0,
                  let g = numero(campoGrados.stringValue) else { return }
            hecho(l, g)
        }
    }
}
