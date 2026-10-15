// Los mandos de estilo de la barra del lienzo: color del trazo, fondo,
// relleno, grosor y linea. Es el panel lateral de Windows y Excalidraw: lo
// que se elige queda para lo proximo que se dibuje y, si hay algo elegido,
// se le aplica en un paso de deshacer (`EditorLienzo.cambiarEstilo`, ver
// `pixpin-nucleo/src/editor_estilo.rs`). Lo que ensena es `EditorLienzo.estilo`:
// el de lo elegido o, sin nada, el «actual».

import AppKit
import PixPinCore
import SwiftUI

/// Una opcion de un mando: su nombre (que hace de id), su simbolo y su valor.
public struct OpcionEstiloLienzo<Valor>: Identifiable {
    public let nombre: String
    public let simbolo: String
    public let valor: Valor
    public var id: String { nombre }

    init(_ nombre: String, _ valor: Valor, simbolo: String = "") {
        self.nombre = nombre
        self.simbolo = simbolo
        self.valor = valor
    }
}

/// Las opciones de cada mando. Aparte de la vista para probarlas sin
/// pantalla.
public enum OpcionesEstiloLienzo {
    /// Los colores de trazo de Excalidraw (`COLOR_PALETTE`, el tono de la barra).
    public static let trazos: [OpcionEstiloLienzo<ColorEstilo>] = [
        .init("Negro", hex(0x1e1e1e)), .init("Rojo", hex(0xe03131)), .init("Verde", hex(0x2f9e44)),
        .init("Azul", hex(0x1971c2)), .init("Naranja", hex(0xf08c00)),
    ]

    /// Los fondos de Excalidraw; `nil` es transparente.
    public static let fondos: [OpcionEstiloLienzo<ColorEstilo?>] = [
        .init("Transparente", nil), .init("Rosa", hex(0xffc9c9)), .init("Verde claro", hex(0xb2f2bb)),
        .init("Azul claro", hex(0xa5d8ff)), .init("Amarillo", hex(0xffec99)),
    ]

    public static let grosores: [OpcionEstiloLienzo<GrosorEstilo>] = [
        .init("Fino", .fino, simbolo: "minus"), .init("Medio", .medio, simbolo: "equal"),
        .init("Grueso", .grueso, simbolo: "rectangle.fill"),
    ]

    public static let lineas: [OpcionEstiloLienzo<LineaEstilo>] = [
        .init("Continua", .continua), .init("Discontinua", .discontinua), .init("Punteada", .punteada),
    ]

    public static let rellenos: [OpcionEstiloLienzo<RellenoEstilo>] = [
        .init("Sólido", .solido), .init("Rayado", .rayado), .init("Cruzado", .cruzado),
        .init("Zigzag", .zigzag), .init("Líneas", .lineas),
    ]

    public static func hex(_ v: UInt32) -> ColorEstilo {
        ColorEstilo(r: Float((v >> 16) & 0xff) / 255, g: Float((v >> 8) & 0xff) / 255,
                    b: Float(v & 0xff) / 255, a: 1)
    }

    public static func color(_ c: ColorEstilo) -> Color {
        Color(.sRGB, red: Double(c.r), green: Double(c.g), blue: Double(c.b), opacity: Double(c.a))
    }

    /// De un color del selector del sistema al del motor (sRGB).
    public static func estilo(de color: Color) -> ColorEstilo? {
        guard let c = NSColor(color).usingColorSpace(.sRGB) else { return nil }
        return ColorEstilo(r: Float(c.redComponent), g: Float(c.greenComponent),
                           b: Float(c.blueComponent), a: Float(c.alphaComponent))
    }

    /// Si dos colores son el mismo a la vista (un canal de 8 bits).
    public static func igual(_ a: ColorEstilo?, _ b: ColorEstilo?) -> Bool {
        switch (a, b) {
        case (nil, nil): return true
        case let (x?, y?):
            return abs(x.r - y.r) < 0.004 && abs(x.g - y.g) < 0.004 && abs(x.b - y.b) < 0.004
                && abs(x.a - y.a) < 0.004
        default: return false
        }
    }

    static func nombre(_ g: GrosorEstilo) -> String {
        grosores.first { mismo($0.valor, g) }?.nombre ?? ""
    }

    static func nombre(_ l: LineaEstilo) -> String {
        lineas.first { mismo($0.valor, l) }?.nombre ?? ""
    }

    static func nombre(_ r: RellenoEstilo) -> String {
        rellenos.first { mismo($0.valor, r) }?.nombre ?? ""
    }

    // Comparar sin depender de que los enlaces generen `Equatable`.
    static func mismo(_ a: GrosorEstilo, _ b: GrosorEstilo) -> Bool { "\(a)" == "\(b)" }
    static func mismo(_ a: LineaEstilo, _ b: LineaEstilo) -> Bool { "\(a)" == "\(b)" }
    static func mismo(_ a: RellenoEstilo, _ b: RellenoEstilo) -> Bool { "\(a)" == "\(b)" }
}

/// Un cuadradito de color (o el damero de «transparente»).
private struct Muestra: View {
    let color: ColorEstilo?
    let elegida: Bool

    var body: some View {
        ZStack {
            if let color {
                RoundedRectangle(cornerRadius: 3).fill(OpcionesEstiloLienzo.color(color))
            } else {
                RoundedRectangle(cornerRadius: 3).fill(Color.white)
                Path { p in
                    p.move(to: CGPoint(x: 2, y: 14))
                    p.addLine(to: CGPoint(x: 14, y: 2))
                }.stroke(Color.red, lineWidth: 1.5)
            }
            RoundedRectangle(cornerRadius: 3)
                .stroke(elegida ? Color.accentColor : Color.secondary.opacity(0.5), lineWidth: elegida ? 2 : 0.5)
        }
        .frame(width: 16, height: 16)
    }
}

/// Los mandos, en la barra del editor.
struct BarraEstiloLienzo: View {
    @ObservedObject var modelo: ModeloEditorLienzo

    private var estilo: EstiloLienzo { modelo.estilo }

    var body: some View {
        HStack(spacing: 4) {
            ForEach(OpcionesEstiloLienzo.trazos) { o in
                Button { modelo.cambiarEstilo(.trazo(color: o.valor)) } label: {
                    Muestra(color: o.valor, elegida: OpcionesEstiloLienzo.igual(estilo.trazo, o.valor))
                }
                .buttonStyle(.plain)
                .help("Trazo: \(o.nombre)")
            }
            ColorPicker("", selection: Binding(
                get: { OpcionesEstiloLienzo.color(estilo.trazo) },
                set: { nuevo in
                    if let c = OpcionesEstiloLienzo.estilo(de: nuevo) { modelo.cambiarEstilo(.trazo(color: c)) }
                }
            ), supportsOpacity: false)
            .labelsHidden()
            .frame(width: 30)
            .help("Otro color de trazo")

            Menu {
                ForEach(OpcionesEstiloLienzo.fondos) { o in
                    Button(o.nombre) { modelo.cambiarEstilo(.fondo(color: o.valor)) }
                }
                Divider()
                ForEach(OpcionesEstiloLienzo.rellenos) { o in
                    Button {
                        modelo.cambiarEstilo(.relleno(relleno: o.valor))
                    } label: {
                        if OpcionesEstiloLienzo.mismo(estilo.relleno, o.valor) {
                            Label("Relleno: \(o.nombre)", systemImage: "checkmark")
                        } else {
                            Text("Relleno: \(o.nombre)")
                        }
                    }
                }
            } label: {
                Muestra(color: estilo.fondo, elegida: false)
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .help("Fondo (\(OpcionesEstiloLienzo.nombre(estilo.relleno)))")

            Menu {
                ForEach(OpcionesEstiloLienzo.grosores) { o in
                    Button {
                        modelo.cambiarEstilo(.grosor(grosor: o.valor))
                    } label: {
                        Label(o.nombre,
                              systemImage: OpcionesEstiloLienzo.mismo(estilo.grosor, o.valor) ? "checkmark" : o.simbolo)
                    }
                }
            } label: {
                Image(systemName: "lineweight")
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .help("Grosor: \(OpcionesEstiloLienzo.nombre(estilo.grosor))")

            Menu {
                ForEach(OpcionesEstiloLienzo.lineas) { o in
                    Button {
                        modelo.cambiarEstilo(.linea(linea: o.valor))
                    } label: {
                        if OpcionesEstiloLienzo.mismo(estilo.linea, o.valor) {
                            Label(o.nombre, systemImage: "checkmark")
                        } else {
                            Text(o.nombre)
                        }
                    }
                }
            } label: {
                Image(systemName: "line.diagonal")
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .help("Línea: \(OpcionesEstiloLienzo.nombre(estilo.linea))")
        }
    }
}
