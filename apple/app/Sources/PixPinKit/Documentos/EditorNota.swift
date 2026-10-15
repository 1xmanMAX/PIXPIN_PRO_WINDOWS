// **El editor de notas Markdown** (la nota `NOTA` del chat; en Windows,
// `pixpin-notas` con su `RichEdit`): un `NSTextView` (TextKit 2) cuyo texto
// ES el Markdown, letra a letra. El formato se pone encima mientras se
// escribe (titulos, negrita, cursiva, listas, casillas, codigo, citas,
// enlaces) con lo que dice `pixpin_docs::md_vivo` (Rust, `mdTramos`), y las
// formulas `$…$` se componen con `pixpin_docs::formulas` (`formulaToques`):
// `x^2` se ve x² fuera del renglon donde se escribe.
//
// Las marcas (`#`, `**`, `[`, los dolares) se esconden fuera del renglon del
// cursor; en el suyo se ven, para poder borrarlas sin adivinar.
//
// Guardar es leer el texto y ya: 1 s despues de la ultima tecla, fuera del
// hilo principal, con `Nucleo.editarNota` (la linea del cuaderno y, si es
// una nota del movil, su hoja `nota`), como Windows.
//
// Rendimiento: analizar es una pasada por el texto en Rust y se pide tras
// una pausa de 120 ms al escribir, no por cada tecla.

import AppKit
import PixPinCore

/// Los estilos de una nota, aplicados sobre un `NSTextStorage`. Sin
/// ventanas: se prueba solo.
@MainActor
public enum EstiloDeNota {
    public static let tamBase: CGFloat = 14

    public static var letraBase: NSFont { NSFont.systemFont(ofSize: tamBase) }

    static func tamTitulo(_ nivel: UInt8) -> CGFloat {
        switch nivel {
        case 1: return tamBase * 1.7
        case 2: return tamBase * 1.4
        case 3: return tamBase * 1.2
        default: return tamBase * 1.05
        }
    }

    private static func conRasgo(_ s: NSTextStorage, _ r: NSRange, _ rasgo: NSFontTraitMask) {
        s.enumerateAttribute(.font, in: r) { valor, sub, _ in
            let f = (valor as? NSFont) ?? letraBase
            s.addAttribute(.font, value: NSFontManager.shared.convert(f, toHaveTrait: rasgo), range: sub)
        }
    }

    private static func monoespaciada(_ s: NSTextStorage, _ r: NSRange) {
        s.enumerateAttribute(.font, in: r) { valor, sub, _ in
            let tam = (valor as? NSFont)?.pointSize ?? tamBase
            s.addAttribute(.font, value: NSFont.monospacedSystemFont(ofSize: tam * 0.95, weight: .regular), range: sub)
        }
    }

    /// Una marca escondida: diminuta y transparente.
    private static func esconder(_ s: NSTextStorage, _ r: NSRange) {
        s.addAttributes([.font: NSFont.systemFont(ofSize: 0.1), .foregroundColor: NSColor.clear], range: r)
    }

    /// **Pone el formato** de `tramos` sobre todo el texto. `lineaDelCursor`:
    /// en ese renglon las marcas se ven (atenuadas).
    public static func aplicar(_ s: NSTextStorage, tramos: [TramoMd], lineaDelCursor: UInt32?) {
        let todo = NSRange(location: 0, length: s.length)
        guard todo.length > 0 else { return }
        s.beginEditing()
        defer { s.endEditing() }
        s.setAttributes([.font: letraBase, .foregroundColor: NSColor.textColor], range: todo)
        func rango(_ t: TramoMd) -> NSRange? {
            let a = Int(t.desde), b = min(Int(t.hasta), todo.length)
            return a < b ? NSRange(location: a, length: b - a) : nil
        }
        // Primero lo que cambia el tamano (titulos), luego los rasgos.
        for t in tramos {
            guard let r = rango(t) else { continue }
            switch t.estilo {
            case let .titulo(nivel):
                s.addAttribute(.font, value: NSFont.boldSystemFont(ofSize: tamTitulo(nivel)), range: r)
            case .cabecera:
                conRasgo(s, r, .boldFontMask)
            default:
                break
            }
        }
        for t in tramos {
            guard let r = rango(t) else { continue }
            switch t.estilo {
            case .negrita: conRasgo(s, r, .boldFontMask)
            case .cursiva: conRasgo(s, r, .italicFontMask)
            case .tachado:
                s.addAttribute(.strikethroughStyle, value: NSUnderlineStyle.single.rawValue, range: r)
            case .codigo:
                monoespaciada(s, r)
                s.addAttribute(.backgroundColor, value: NSColor.quaternaryLabelColor, range: r)
            case .bloqueCodigo:
                monoespaciada(s, r)
                s.addAttribute(.backgroundColor, value: NSColor.quaternaryLabelColor.withAlphaComponent(0.5), range: r)
            case .cita:
                s.addAttribute(.foregroundColor, value: NSColor.secondaryLabelColor, range: r)
                let p = NSMutableParagraphStyle()
                p.headIndent = 16
                p.firstLineHeadIndent = 16
                s.addAttribute(.paragraphStyle, value: p, range: r)
            case .enlace:
                s.addAttribute(.foregroundColor, value: NSColor.linkColor, range: r)
                s.addAttribute(.underlineStyle, value: NSUnderlineStyle.single.rawValue, range: r)
                if let u = t.url, let url = URL(string: u) { s.addAttribute(.link, value: url, range: r) }
            case .casilla:
                monoespaciada(s, r)
                s.addAttribute(.foregroundColor, value: NSColor.controlAccentColor, range: r)
                s.addAttribute(.cursor, value: NSCursor.pointingHand, range: r)
            case .hecha:
                s.addAttribute(.strikethroughStyle, value: NSUnderlineStyle.single.rawValue, range: r)
                s.addAttribute(.foregroundColor, value: NSColor.secondaryLabelColor, range: r)
            case .numero:
                s.addAttribute(.foregroundColor, value: NSColor.secondaryLabelColor, range: r)
            case .regla, .imagen:
                s.addAttribute(.foregroundColor, value: NSColor.tertiaryLabelColor, range: r)
            case .formula:
                formula(s, r, fueraDelCursor: t.linea != lineaDelCursor)
            default:
                break
            }
        }
        // Las marcas, al final: encima de todo lo demas.
        for t in tramos where t.estilo == .marca {
            guard let r = rango(t) else { continue }
            if t.linea == lineaDelCursor {
                s.addAttribute(.foregroundColor, value: NSColor.tertiaryLabelColor, range: r)
            } else {
                esconder(s, r)
            }
        }
    }

    /// Una formula entre dolares: en cursiva de imprenta y, fuera del
    /// renglon del cursor, compuesta (`formulaToques`): exponentes arriba,
    /// subindices abajo y sus marcas y dolares escondidos.
    private static func formula(_ s: NSTextStorage, _ r: NSRange, fueraDelCursor: Bool) {
        let serif = NSFont.systemFont(ofSize: tamBase).fontDescriptor.withDesign(.serif)
            .flatMap { NSFont(descriptor: $0, size: tamBase) } ?? letraBase
        let letra = NSFontManager.shared.convert(serif, toHaveTrait: .italicFontMask)
        s.addAttribute(.font, value: letra, range: r)
        guard fueraDelCursor, r.length > 0 else { return }
        // `md_tramos` da la formula sin sus dolares (son tramos `marca`);
        // si vinieran dentro, se esconden aqui.
        var dentroR = r
        let texto = (s.string as NSString).substring(with: r)
        if r.length >= 2, texto.hasPrefix("$"), texto.hasSuffix("$") {
            esconder(s, NSRange(location: r.location, length: 1))
            esconder(s, NSRange(location: r.location + r.length - 1, length: 1))
            dentroR = NSRange(location: r.location + 1, length: r.length - 2)
        }
        let dentro = (s.string as NSString).substring(with: dentroR)
        let pequena = NSFont(descriptor: letra.fontDescriptor, size: tamBase * 0.7) ?? letra
        for t in formulaToques(formula: dentro) {
            let a = dentroR.location + Int(t.desde)
            let largo = Int(t.hasta) - Int(t.desde)
            guard largo > 0, a + largo <= dentroR.location + dentroR.length else { continue }
            let sub = NSRange(location: a, length: largo)
            switch t.toque {
            case .arriba:
                s.addAttributes([.font: pequena, .baselineOffset: tamBase * 0.38], range: sub)
            case .abajo:
                s.addAttributes([.font: pequena, .baselineOffset: -tamBase * 0.18], range: sub)
            case .oculto:
                esconder(s, sub)
            }
        }
    }

    /// El renglon (desde 0) de una posicion UTF-16 del texto.
    public static func linea(de pos: Int, en texto: String) -> UInt32 {
        let ns = texto as NSString
        var n: UInt32 = 0
        var i = 0
        let fin = min(pos, ns.length)
        while i < fin {
            let r = ns.rangeOfCharacter(from: .newlines, options: [], range: NSRange(location: i, length: fin - i))
            if r.location == NSNotFound { break }
            n += 1
            i = r.location + r.length
        }
        return n
    }
}

/// El texto de la nota: Intro sigue las listas y un clic en una casilla la
/// marca.
final class VistaTextoNota: NSTextView {
    var alClicEnCasilla: (@MainActor (Int) -> Bool)?

    override func mouseDown(with event: NSEvent) {
        let p = convert(event.locationInWindow, from: nil)
        let i = characterIndexForInsertion(at: p)
        if event.clickCount == 1, let alClicEnCasilla, alClicEnCasilla(i) { return }
        super.mouseDown(with: event)
    }
}

/// **Una nota abierta para editar.**
@MainActor
final class ControladorNota: NSObject, NSTextViewDelegate, NSWindowDelegate {
    let chat: String
    let mensaje: String
    let almacen: Almacen
    let ventana: NSWindow
    let texto: VistaTextoNota
    private var guardado: String
    private var restilo: DispatchWorkItem?
    private var guardar: DispatchWorkItem?
    private var lineaDelCursor: UInt32?
    private let colaGuardar = DispatchQueue(label: "pixpin.nota.guardar", qos: .utility)
    var alCerrar: (@MainActor (ControladorNota) -> Void)?

    init(mensaje: MensajeChat, chat: String, almacen: Almacen) {
        self.chat = chat
        self.mensaje = mensaje.id
        self.almacen = almacen
        guardado = mensaje.texto
        ventana = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 640, height: 720),
                           styleMask: [.titled, .closable, .miniaturizable, .resizable],
                           backing: .buffered, defer: false)
        let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 640, height: 720))
        scroll.hasVerticalScroller = true
        scroll.autoresizingMask = [.width, .height]
        texto = VistaTextoNota(frame: scroll.contentView.bounds)
        super.init()
        texto.autoresizingMask = [.width]
        texto.isVerticallyResizable = true
        texto.isHorizontallyResizable = false
        texto.textContainer?.widthTracksTextView = true
        texto.minSize = NSSize(width: 0, height: 0)
        texto.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        texto.textContainerInset = NSSize(width: 24, height: 20)
        texto.isRichText = false
        texto.allowsUndo = true
        texto.isAutomaticQuoteSubstitutionEnabled = false
        texto.isAutomaticDashSubstitutionEnabled = false
        texto.isAutomaticTextReplacementEnabled = false
        texto.font = EstiloDeNota.letraBase
        texto.string = mensaje.texto
        texto.delegate = self
        texto.alClicEnCasilla = { [weak self] i in self?.clicEnCasilla(i) ?? false }
        scroll.documentView = texto
        ventana.contentView = scroll
        ventana.isReleasedWhenClosed = false
        ventana.delegate = self
        ventana.title = ControladorNota.titulo(mensaje.texto)
        ventana.center()
        _ = ventana.setFrameAutosaveName("PixPinNota")
        restilar()
    }

    static func titulo(_ texto: String) -> String {
        let t = mdTitulo(texto: texto)
        return t.isEmpty ? "Nota" : t
    }

    func mostrar() {
        ventana.makeKeyAndOrderFront(nil)
        ventana.makeFirstResponder(texto)
        NSApp.activate()
    }

    private func restilar() {
        guard let s = texto.textStorage else { return }
        let tramos = mdTramos(texto: s.string)
        let seleccion = texto.selectedRanges
        EstiloDeNota.aplicar(s, tramos: tramos, lineaDelCursor: lineaDelCursor)
        texto.selectedRanges = seleccion
        texto.typingAttributes = [.font: EstiloDeNota.letraBase, .foregroundColor: NSColor.textColor]
    }

    private func programarRestilo() {
        restilo?.cancel()
        let tarea = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated { self?.restilar() }
        }
        restilo = tarea
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.12, execute: tarea)
    }

    private func programarGuardado(dentroDe s: Double = 1.0) {
        guardar?.cancel()
        let tarea = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated { self?.guardarYa() }
        }
        guardar = tarea
        DispatchQueue.main.asyncAfter(deadline: .now() + s, execute: tarea)
    }

    /// Escribe en el cuaderno (fuera del hilo principal) si cambio.
    private func guardarYa() {
        let actual = texto.string
        guard actual != guardado else { return }
        guardado = actual
        let nucleo = almacen.nucleo, chat = self.chat, id = mensaje
        let almacen = self.almacen
        colaGuardar.async {
            do {
                let m = try nucleo.editarNota(chat: chat, mensaje: id, texto: actual)
                DispatchQueue.main.async {
                    MainActor.assumeIsolated { almacen.sustituir(m, en: chat) }
                }
            } catch {
                let e = "\(error)"
                DispatchQueue.main.async {
                    MainActor.assumeIsolated { almacen.ponerError("No se pudo guardar la nota: \(e)") }
                }
            }
        }
        ventana.title = ControladorNota.titulo(actual)
    }

    // MARK: Casillas e Intro

    /// Un clic en una casilla (`[ ]`/`[x]`) la marca o desmarca.
    private func clicEnCasilla(_ i: Int) -> Bool {
        let s = texto.string
        guard let t = mdTramos(texto: s).first(where: {
            if case .casilla = $0.estilo { return Int($0.desde) <= i && i < Int($0.hasta) }
            return false
        }) else { return false }
        let pos = Int(t.desde) + 1
        let ns = s as NSString
        guard pos < ns.length else { return false }
        let ahora = ns.substring(with: NSRange(location: pos, length: 1))
        let nuevo = ahora == " " ? "x" : " "
        let r = NSRange(location: pos, length: 1)
        if texto.shouldChangeText(in: r, replacementString: nuevo) {
            texto.textStorage?.replaceCharacters(in: r, with: nuevo)
            texto.didChangeText()
        }
        return true
    }

    func textView(_ textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        guard selector == #selector(NSResponder.insertNewline(_:)) else { return false }
        let ns = textView.string as NSString
        let cursor = textView.selectedRange().location
        let renglon = ns.lineRange(for: NSRange(location: cursor, length: 0))
        var sinSalto = ns.substring(with: renglon)
        while sinSalto.hasSuffix("\n") || sinSalto.hasSuffix("\r") { sinSalto.removeLast() }
        switch mdContinuar(renglon: sinSalto) {
        case .nada:
            return false
        case let .con(prefijo):
            textView.insertText("\n" + prefijo, replacementRange: textView.selectedRange())
            return true
        case .cortar:
            // El renglon solo tenia la marca: se quita y se sale de la lista.
            let r = NSRange(location: renglon.location, length: (sinSalto as NSString).length)
            if textView.shouldChangeText(in: r, replacementString: "") {
                textView.textStorage?.replaceCharacters(in: r, with: "")
                textView.didChangeText()
            }
            return true
        }
    }

    // MARK: Delegados

    func textDidChange(_ notification: Notification) {
        programarRestilo()
        programarGuardado()
    }

    func textViewDidChangeSelection(_ notification: Notification) {
        let l = EstiloDeNota.linea(de: texto.selectedRange().location, en: texto.string)
        if l != lineaDelCursor {
            lineaDelCursor = l
            programarRestilo()
        }
    }

    func windowWillClose(_ notification: Notification) {
        guardar?.cancel()
        restilo?.cancel()
        guardarYa()
        alCerrar?(self)
    }
}

/// **Las notas abiertas**: una ventana por nota.
@MainActor
public final class EditoresDeNotas {
    public static let compartidos = EditoresDeNotas()
    private var abiertos: [String: ControladorNota] = [:]

    public func abrir(_ mensaje: MensajeChat, en chat: String, almacen: Almacen) {
        let clave = chat + "\u{0}" + mensaje.id
        if let c = abiertos[clave] {
            c.mostrar()
            return
        }
        let c = ControladorNota(mensaje: mensaje, chat: chat, almacen: almacen)
        c.alCerrar = { [weak self] _ in self?.abiertos[clave] = nil }
        abiertos[clave] = c
        c.mostrar()
    }
}
