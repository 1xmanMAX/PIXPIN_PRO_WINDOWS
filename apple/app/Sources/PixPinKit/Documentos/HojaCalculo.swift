// **El editor de hojas de calculo** del chat: una tabla del PC (mensaje
// `MINIAPP` tabla) o del movil (`TABLA`, `tablas/<ref>.json`), editada celda
// a celda y guardada donde la guarda Windows (`HojaDeCalculo` de Rust, el
// `hoja_pin` de Windows). Las formulas las calcula el motor de
// `pixpin-proyecto` (el mismo `=SUMA(...)` que Windows).
//
// Rendimiento (§4bis): la rejilla es un `NSTableView` (solo existen las
// filas que se ven) y las celdas se piden a Rust por bloques de 100 filas,
// ya calculadas; un cambio vacia los bloques (una formula puede depender de
// cualquier celda) y se piden otra vez solo los que se ven. Guardar va 1 s
// despues del ultimo cambio, fuera del hilo principal.

import AppKit
import PixPinCore

/// El texto de una celda con su formato de numero (`nf`: numero, moneda,
/// porcentaje, fecha), como lo ensena el pin de hoja de Windows.
public func textoDeCelda(_ c: CeldaHoja, locale: Locale = .current) -> String {
    guard let n = c.numero, let nf = c.estilo?.formatoNumero, !c.error else { return c.mostrado }
    let f = NumberFormatter()
    f.locale = locale
    switch nf {
    case "numero":
        f.numberStyle = .decimal
        f.minimumFractionDigits = 2
        f.maximumFractionDigits = 2
        f.usesGroupingSeparator = true
    case "moneda":
        f.numberStyle = .currency
    case "porcentaje":
        f.numberStyle = .percent
        f.minimumFractionDigits = 2
        f.maximumFractionDigits = 2
    case "fecha":
        // El numero es el serial de Excel: dias desde el 30-12-1899.
        let base = DateComponents(calendar: Calendar(identifier: .gregorian), timeZone: TimeZone(identifier: "UTC"),
                                  year: 1899, month: 12, day: 30).date ?? Date(timeIntervalSince1970: 0)
        let d = base.addingTimeInterval(n * 86_400)
        let df = DateFormatter()
        df.locale = locale
        df.timeZone = TimeZone(identifier: "UTC")
        df.dateStyle = .short
        df.timeStyle = .none
        return df.string(from: d)
    default:
        return c.mostrado
    }
    return f.string(from: NSNumber(value: n)) ?? c.mostrado
}

/// El nombre de una columna (`A`, `Z`, `AA`...), como `tabla::ref_a`.
public func letrasDeColumna(_ i: Int) -> String {
    var n = i + 1
    var s = ""
    while n > 0 {
        let r = (n - 1) % 26
        s = String(UnicodeScalar(UInt8(65 + r))) + s
        n = (n - 1) / 26
    }
    return s
}

/// Un color `#rrggbb` (el de los estilos de celda).
func colorDeHex(_ hex: String?) -> NSColor? {
    guard var h = hex?.trimmingCharacters(in: .whitespaces), h.hasPrefix("#") else { return nil }
    h.removeFirst()
    guard h.count == 6, let v = UInt32(h, radix: 16) else { return nil }
    return NSColor(srgbRed: CGFloat((v >> 16) & 0xFF) / 255, green: CGFloat((v >> 8) & 0xFF) / 255,
                   blue: CGFloat(v & 0xFF) / 255, alpha: 1)
}

/// La rejilla: las flechas mueven la celda elegida, escribir empieza a
/// editarla en la barra de formulas, Suprimir la vacia.
final class RejillaHoja: NSTableView {
    weak var controlador: ControladorHoja?

    override func keyDown(with event: NSEvent) {
        if let c = controlador, c.tecla(event) { return }
        super.keyDown(with: event)
    }
}

@MainActor
final class ControladorHoja: NSObject, NSTableViewDataSource, NSTableViewDelegate, NSWindowDelegate,
    NSTextFieldDelegate {
    let hoja: HojaDeCalculo
    let ventana: NSWindow
    private let rejilla = RejillaHoja()
    private let barra = NSTextField()
    private let referencia = NSTextField(labelWithString: "A1")
    private var columnas = 26
    private var filas = 200
    private var bloques: [Int: [Int: [Int: CeldaHoja]]] = [:]
    private static let filasPorBloque = 100
    private var fila = 0
    private var columna = 0
    private let colaGuardar = DispatchQueue(label: "pixpin.hoja.guardar", qos: .utility)
    private var guardado: DispatchWorkItem?
    var alCerrar: (@MainActor (ControladorHoja) -> Void)?

    init(hoja: HojaDeCalculo, titulo: String) {
        self.hoja = hoja
        ventana = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 900, height: 640),
                           styleMask: [.titled, .closable, .miniaturizable, .resizable],
                           backing: .buffered, defer: false)
        super.init()
        ventana.title = titulo
        ventana.isReleasedWhenClosed = false
        ventana.delegate = self
        montar()
        recalcularTamano()
    }

    private func boton(_ simbolo: String, _ ayuda: String, _ accion: Selector) -> NSButton {
        let b = NSButton(image: NSImage(systemSymbolName: simbolo, accessibilityDescription: ayuda) ?? NSImage(),
                         target: self, action: accion)
        b.bezelStyle = .rounded
        b.toolTip = ayuda
        return b
    }

    private func montar() {
        referencia.font = .monospacedSystemFont(ofSize: 12, weight: .medium)
        referencia.setContentHuggingPriority(.required, for: .horizontal)
        barra.placeholderString = "Valor o =fórmula"
        barra.delegate = self
        barra.target = self
        barra.action = #selector(confirmar)
        let formato = NSPopUpButton(frame: .zero, pullsDown: false)
        formato.addItems(withTitles: ["General", "Número", "Moneda", "Porcentaje", "Fecha"])
        formato.target = self
        formato.action = #selector(elegirFormato(_:))
        formato.toolTip = "Formato del número"
        let fondo = NSPopUpButton(frame: .zero, pullsDown: true)
        fondo.addItems(withTitles: ["Fondo", "Sin fondo", "Amarillo", "Verde", "Azul", "Rojo", "Gris"])
        fondo.target = self
        fondo.action = #selector(elegirFondo(_:))
        let arriba = NSStackView(views: [
            referencia, barra,
            boton("bold", "Negrita (⌘B)", #selector(negrita)),
            boton("italic", "Cursiva (⌘I)", #selector(cursiva)),
            boton("underline", "Subrayado (⌘U)", #selector(subrayado)),
            boton("strikethrough", "Tachado", #selector(tachado)),
            boton("text.alignleft", "Izquierda", #selector(alinearIzquierda)),
            boton("text.aligncenter", "Centro", #selector(alinearCentro)),
            boton("text.alignright", "Derecha", #selector(alinearDerecha)),
            formato, fondo,
        ])
        arriba.orientation = .horizontal
        arriba.spacing = 6
        arriba.edgeInsets = NSEdgeInsets(top: 6, left: 8, bottom: 6, right: 8)
        arriba.translatesAutoresizingMaskIntoConstraints = false

        rejilla.controlador = self
        rejilla.dataSource = self
        rejilla.delegate = self
        rejilla.usesAlternatingRowBackgroundColors = false
        rejilla.gridStyleMask = [.solidHorizontalGridLineMask, .solidVerticalGridLineMask]
        rejilla.selectionHighlightStyle = .none
        rejilla.allowsColumnReordering = false
        rejilla.allowsColumnResizing = true
        rejilla.rowHeight = 22
        rejilla.intercellSpacing = NSSize(width: 0, height: 0)
        rejilla.target = self
        rejilla.action = #selector(clic)
        rejilla.doubleAction = #selector(dobleClic)
        let scroll = NSScrollView()
        scroll.documentView = rejilla
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = true
        scroll.translatesAutoresizingMaskIntoConstraints = false

        let raiz = NSView()
        raiz.addSubview(arriba)
        raiz.addSubview(scroll)
        NSLayoutConstraint.activate([
            arriba.topAnchor.constraint(equalTo: raiz.topAnchor),
            arriba.leadingAnchor.constraint(equalTo: raiz.leadingAnchor),
            arriba.trailingAnchor.constraint(equalTo: raiz.trailingAnchor),
            scroll.topAnchor.constraint(equalTo: arriba.bottomAnchor),
            scroll.leadingAnchor.constraint(equalTo: raiz.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: raiz.trailingAnchor),
            scroll.bottomAnchor.constraint(equalTo: raiz.bottomAnchor),
        ])
        ventana.contentView = raiz
        ventana.setContentSize(NSSize(width: 900, height: 640))
        ventana.center()
        _ = ventana.setFrameAutosaveName("PixPinHoja")
        NotificationCenter.default.addObserver(self, selector: #selector(columnaCambiada(_:)),
                                               name: NSTableView.columnDidResizeNotification, object: rejilla)
    }

    /// Lo que se ensena: lo escrito y un margen para seguir escribiendo.
    private func recalcularTamano() {
        let t = hoja.tamano()
        let c = max(Int(t.first ?? 0) + 5, 26)
        filas = max(Int(t.count > 1 ? t[1] : 0) + 50, 200)
        if c != columnas || rejilla.tableColumns.isEmpty {
            columnas = min(c, 16_384)
            ponerColumnas()
        }
        rejilla.reloadData()
    }

    private func ponerColumnas() {
        for col in rejilla.tableColumns { rejilla.removeTableColumn(col) }
        let cabecera = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("#"))
        cabecera.title = ""
        cabecera.width = 44
        cabecera.resizingMask = []
        rejilla.addTableColumn(cabecera)
        let anchos = Dictionary(hoja.anchos().map { (Int($0.columna), CGFloat($0.ancho)) }, uniquingKeysWith: { a, _ in a })
        for i in 0..<columnas {
            let col = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("\(i)"))
            col.title = letrasDeColumna(i)
            col.width = anchos[i] ?? 90
            col.minWidth = 24
            col.headerCell.alignment = .center
            rejilla.addTableColumn(col)
        }
    }

    // MARK: Datos

    private func celda(_ f: Int, _ c: Int) -> CeldaHoja? {
        let b = f / ControladorHoja.filasPorBloque
        if bloques[b] == nil {
            let desde = UInt32(b * ControladorHoja.filasPorBloque)
            var mapa: [Int: [Int: CeldaHoja]] = [:]
            for x in hoja.celdas(desdeFila: desde, hastaFila: desde + UInt32(ControladorHoja.filasPorBloque),
                                 desdeColumna: 0, hastaColumna: UInt32(columnas)) {
                mapa[Int(x.fila), default: [:]][Int(x.columna)] = x
            }
            bloques[b] = mapa
        }
        return bloques[b]?[f]?[c]
    }

    func numberOfRows(in tableView: NSTableView) -> Int { filas }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard let id = tableColumn?.identifier else { return nil }
        let vista = (tableView.makeView(withIdentifier: id, owner: self) as? NSTextField) ?? {
            let t = NSTextField(labelWithString: "")
            t.identifier = id
            t.lineBreakMode = .byTruncatingTail
            t.drawsBackground = true
            return t
        }()
        if id.rawValue == "#" {
            vista.stringValue = "\(row + 1)"
            vista.alignment = .center
            vista.textColor = .secondaryLabelColor
            vista.font = .monospacedDigitSystemFont(ofSize: 11, weight: .regular)
            vista.backgroundColor = .controlBackgroundColor
            return vista
        }
        let c = Int(id.rawValue) ?? 0
        let x = celda(row, c)
        let e = x?.estilo
        vista.stringValue = x.map { textoDeCelda($0) } ?? ""
        var letra = NSFont.systemFont(ofSize: 12)
        if e?.negrita == true { letra = NSFontManager.shared.convert(letra, toHaveTrait: .boldFontMask) }
        if e?.cursiva == true { letra = NSFontManager.shared.convert(letra, toHaveTrait: .italicFontMask) }
        vista.font = letra
        vista.textColor = x?.error == true ? .systemRed : (colorDeHex(e?.color) ?? .labelColor)
        switch e?.alineacion {
        case "c": vista.alignment = .center
        case "d": vista.alignment = .right
        case "i": vista.alignment = .left
        default: vista.alignment = x?.numero != nil ? .right : .left
        }
        let elegida = row == fila && c == columna
        vista.backgroundColor = elegida ? NSColor.selectedContentBackgroundColor.withAlphaComponent(0.25)
            : (colorDeHex(e?.fondo) ?? .clear)
        var adornos: [NSAttributedString.Key: Any] = [.font: letra, .foregroundColor: vista.textColor ?? NSColor.labelColor]
        if e?.subrayado == true { adornos[.underlineStyle] = NSUnderlineStyle.single.rawValue }
        if e?.tachado == true { adornos[.strikethroughStyle] = NSUnderlineStyle.single.rawValue }
        if adornos.count > 2 {
            let p = NSMutableParagraphStyle()
            p.alignment = vista.alignment
            adornos[.paragraphStyle] = p
            vista.attributedStringValue = NSAttributedString(string: vista.stringValue, attributes: adornos)
        }
        return vista
    }

    /// Tras un cambio: todo se vuelve a calcular y se repinta lo que se ve.
    private func trasCambio() {
        bloques = [:]
        recalcularTamano()
        programarGuardado()
    }

    // MARK: Elegir y editar

    private func elegir(_ f: Int, _ c: Int) {
        let antes = (fila, columna)
        fila = max(0, min(f, filas - 1))
        columna = max(0, min(c, columnas - 1))
        referencia.stringValue = "\(letrasDeColumna(columna))\(fila + 1)"
        barra.stringValue = hoja.contenido(fila: UInt32(fila), columna: UInt32(columna))
        rejilla.reloadData(forRowIndexes: IndexSet([antes.0, fila]), columnIndexes: IndexSet(integersIn: 0..<rejilla.numberOfColumns))
        rejilla.scrollRowToVisible(fila)
        rejilla.scrollColumnToVisible(columna + 1)
        _ = antes
    }

    @objc private func clic() {
        let f = rejilla.clickedRow, c = rejilla.clickedColumn - 1
        guard f >= 0, c >= 0 else { return }
        elegir(f, c)
    }

    @objc private func dobleClic() {
        clic()
        ventana.makeFirstResponder(barra)
    }

    /// Enter en la barra: se escribe la celda y se baja una.
    @objc private func confirmar() {
        if hoja.poner(fila: UInt32(fila), columna: UInt32(columna), contenido: barra.stringValue) {
            trasCambio()
        }
        elegir(fila + 1, columna)
        ventana.makeFirstResponder(rejilla)
    }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        if selector == #selector(NSResponder.cancelOperation(_:)) {
            barra.stringValue = hoja.contenido(fila: UInt32(fila), columna: UInt32(columna))
            ventana.makeFirstResponder(rejilla)
            return true
        }
        return false
    }

    /// Las teclas de la rejilla. `true` si era suya.
    func tecla(_ e: NSEvent) -> Bool {
        let m = e.modifierFlags.intersection(.deviceIndependentFlagsMask).subtracting([.capsLock, .numericPad, .function])
        switch e.keyCode {
        case 123: elegir(fila, columna - 1); return true
        case 124: elegir(fila, columna + 1); return true
        case 125: elegir(fila + 1, columna); return true
        case 126: elegir(fila - 1, columna); return true
        case 48: elegir(fila, columna + (m.contains(.shift) ? -1 : 1)); return true
        case 36, 76: ventana.makeFirstResponder(barra); return true
        case 51, 117:
            if hoja.poner(fila: UInt32(fila), columna: UInt32(columna), contenido: "") { trasCambio() }
            barra.stringValue = ""
            return true
        default: break
        }
        if m == .command, let c = e.charactersIgnoringModifiers?.lowercased() {
            switch c {
            case "b": negrita(); return true
            case "i": cursiva(); return true
            case "u": subrayado(); return true
            default: return false
            }
        }
        // Escribir algo empieza a editar la celda en la barra.
        if m.subtracting(.shift).isEmpty, let s = e.characters, let u = s.unicodeScalars.first,
           !CharacterSet.controlCharacters.contains(u) {
            barra.stringValue = s
            ventana.makeFirstResponder(barra)
            barra.currentEditor()?.selectedRange = NSRange(location: (s as NSString).length, length: 0)
            return true
        }
        return false
    }

    // MARK: Formato

    private func cambiarEstilo(_ cambio: (inout EstiloCeldaHoja) -> Void) {
        var e = hoja.estilo(fila: UInt32(fila), columna: UInt32(columna))
            ?? EstiloCeldaHoja(negrita: false, cursiva: false, subrayado: false, tachado: false, alineacion: nil,
                               fondo: nil, color: nil, ajustar: false, formatoNumero: nil)
        cambio(&e)
        if hoja.ponerEstilo(fila: UInt32(fila), columna: UInt32(columna), estilo: e) { trasCambio() }
    }

    @objc private func negrita() { cambiarEstilo { $0.negrita.toggle() } }
    @objc private func cursiva() { cambiarEstilo { $0.cursiva.toggle() } }
    @objc private func subrayado() { cambiarEstilo { $0.subrayado.toggle() } }
    @objc private func tachado() { cambiarEstilo { $0.tachado.toggle() } }
    @objc private func alinearIzquierda() { cambiarEstilo { $0.alineacion = $0.alineacion == "i" ? nil : "i" } }
    @objc private func alinearCentro() { cambiarEstilo { $0.alineacion = $0.alineacion == "c" ? nil : "c" } }
    @objc private func alinearDerecha() { cambiarEstilo { $0.alineacion = $0.alineacion == "d" ? nil : "d" } }

    @objc private func elegirFormato(_ s: NSPopUpButton) {
        let nf: String? = [nil, "numero", "moneda", "porcentaje", "fecha"][max(0, min(s.indexOfSelectedItem, 4))]
        cambiarEstilo { $0.formatoNumero = nf }
    }

    @objc private func elegirFondo(_ s: NSPopUpButton) {
        let colores: [String?] = [nil, nil, "#fff3bf", "#d3f9d8", "#d0ebff", "#ffe3e3", "#e9ecef"]
        let i = s.indexOfSelectedItem
        guard i >= 1, i < colores.count else { return }
        let c = colores[i]
        cambiarEstilo { $0.fondo = c }
    }

    @objc private func columnaCambiada(_ n: Notification) {
        guard let col = n.userInfo?["NSTableColumn"] as? NSTableColumn, let i = Int(col.identifier.rawValue) else { return }
        if hoja.ponerAncho(columna: UInt32(i), ancho: UInt32(max(col.width, 16).rounded())) { programarGuardado() }
    }

    // MARK: Guardar

    private func programarGuardado() {
        guardado?.cancel()
        let hoja = self.hoja
        let tarea = DispatchWorkItem { ControladorHoja.guardarYa(hoja) }
        guardado = tarea
        colaGuardar.asyncAfter(deadline: .now() + 1.0, execute: tarea)
    }

    private nonisolated static func guardarYa(_ hoja: HojaDeCalculo) {
        do {
            if try hoja.guardar() {
                DispatchQueue.main.async {
                    NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                }
            }
        } catch {
            let e = "\(error)"
            DispatchQueue.main.async { NSLog("PixPin: no se pudo guardar la hoja: %@", e) }
        }
    }

    func windowWillClose(_ notification: Notification) {
        if ventana.firstResponder === barra.currentEditor() { confirmar() }
        guardado?.cancel()
        let hoja = self.hoja
        colaGuardar.async { ControladorHoja.guardarYa(hoja) }
        NotificationCenter.default.removeObserver(self)
        alCerrar?(self)
    }
}

/// **Las hojas de calculo abiertas**: una ventana por mensaje.
@MainActor
public final class HojasDeCalculo {
    public static let compartidas = HojasDeCalculo()
    private var abiertas: [String: ControladorHoja] = [:]

    public func abrir(_ mensaje: MensajeChat, en chat: String, almacen: Almacen) {
        let clave = chat + "\u{0}" + mensaje.id
        if let c = abiertas[clave] {
            c.ventana.makeKeyAndOrderFront(nil)
            NSApp.activate()
            return
        }
        let nucleo = almacen.nucleo
        let id = mensaje.id
        let titulo = mensaje.nombre.isEmpty ? "Tabla" : mensaje.nombre
        Task {
            let hecho = await Task.detached(priority: .userInitiated) { () -> Result<HojaDeCalculo, Error> in
                Result { try nucleo.abrirHojaDeCalculo(chat: chat, mensaje: id) }
            }.value
            switch hecho {
            case let .success(hoja):
                let c = ControladorHoja(hoja: hoja, titulo: titulo)
                c.alCerrar = { [weak self] _ in self?.abiertas[clave] = nil }
                self.abiertas[clave] = c
                c.ventana.makeKeyAndOrderFront(nil)
                NSApp.activate()
            case let .failure(e):
                almacen.ponerError("No se puede abrir la tabla: \(e)")
            }
        }
    }
}
