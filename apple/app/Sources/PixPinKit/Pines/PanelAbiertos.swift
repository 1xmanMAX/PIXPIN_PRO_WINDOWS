// El panel «Pines abiertos» (rediseno v2 de Windows, `pixpin-pin/src/
// panel_abiertos.rs` y `pixpin/src/pines/abiertos.rs`): la lista de los
// pines que hay, agrupados por su color, para encontrar los que quedaron
// tapados o se ocultaron.
//
// Clic en una fila: trae el pin a la vista y lo resalta. El ojo lo oculta o
// lo vuelve a ensenar; la cruz lo cierra. Abajo, mostrar, ocultar y cerrar
// todos (cerrar se puede deshacer). La agrupacion y la busqueda son puras
// (`ReglasPanel`, probadas en `PanelAbiertosTests`).
//
// Rendimiento: el modelo solo se rehace cuando un pin cambia (abrir, cerrar,
// ocultar, color), y la lista se pinta solo con el panel abierto.

import AppKit
import SwiftUI

/// Que es cada pin, para su icono.
enum TipoFilaPin: Equatable {
    case foto, nota, pdf, video, audio, vivo, herramienta

    var simbolo: String {
        switch self {
        case .foto: "photo"
        case .nota: "note.text"
        case .pdf: "doc.richtext"
        case .video: "film"
        case .audio: "waveform"
        case .vivo: "dot.radiowaves.left.and.right"
        case .herramienta: "checklist"
        }
    }
}

/// Una fila del panel.
struct FilaPin: Identifiable, Equatable {
    let id: Int
    var nombre: String
    var detalle: String
    var tipo: TipoFilaPin
    /// El color de su grupo (0-7), o sin grupo.
    var grupo: Int?
    var oculto: Bool
}

/// Un grupo del panel: su color y sus filas.
struct GrupoFilas: Identifiable, Equatable {
    var grupo: Int?
    var filas: [FilaPin]
    var id: Int { grupo ?? -1 }
}

/// Los ocho colores de grupo de Windows, en el orden de la paleta.
enum ColoresGrupo {
    static let rgb: [(r: Int, g: Int, b: Int)] = [
        (219, 51, 46), (242, 115, 26), (242, 173, 26), (51, 168, 84),
        (26, 168, 173), (41, 112, 219), (122, 71, 199), (222, 71, 153),
    ]
    static let nombres = ["Rojo", "Naranja", "Ámbar", "Verde", "Cian", "Azul", "Violeta", "Rosa"]

    static func nsColor(_ i: Int) -> NSColor {
        let c = rgb[((i % rgb.count) + rgb.count) % rgb.count]
        return NSColor(srgbRed: CGFloat(c.r) / 255, green: CGFloat(c.g) / 255, blue: CGFloat(c.b) / 255, alpha: 1)
    }
}

enum ReglasPanel {
    /// Las filas que casan con la busqueda (sin distinguir mayusculas ni
    /// tildes), en su orden. Vacia, todas.
    static func filtrar(_ filas: [FilaPin], busqueda: String) -> [FilaPin] {
        let b = busqueda.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !b.isEmpty else { return filas }
        let opciones: String.CompareOptions = [.caseInsensitive, .diacriticInsensitive]
        return filas.filter {
            $0.nombre.range(of: b, options: opciones) != nil || $0.detalle.range(of: b, options: opciones) != nil
        }
    }

    /// Las filas por grupo: los colores en el orden de la paleta y «sin
    /// grupo» al final, cada grupo con sus filas en el orden de llegada.
    static func agrupar(_ filas: [FilaPin]) -> [GrupoFilas] {
        var claves: [Int?] = []
        for f in filas where !claves.contains(where: { $0 == f.grupo }) { claves.append(f.grupo) }
        claves.sort { ($0 ?? Int.max) < ($1 ?? Int.max) }
        return claves.map { g in GrupoFilas(grupo: g, filas: filas.filter { $0.grupo == g }) }
    }
}

/// Lo que ensena el panel; lo rellena `GestorPines`.
@MainActor
final class ModeloPinesAbiertos: ObservableObject {
    @Published var filas: [FilaPin] = []
    @Published var busqueda = ""
    @Published var puedeDeshacer = false

    var alTraer: ((Int) -> Void)?
    var alAlternar: ((Int) -> Void)?
    var alCerrar: ((Int) -> Void)?
    var alMostrarTodos: (() -> Void)?
    var alOcultarTodos: (() -> Void)?
    var alCerrarTodos: (() -> Void)?
    var alDeshacer: (() -> Void)?
}

struct VistaPinesAbiertos: View {
    @ObservedObject var modelo: ModeloPinesAbiertos

    var body: some View {
        let grupos = ReglasPanel.agrupar(ReglasPanel.filtrar(modelo.filas, busqueda: modelo.busqueda))
        VStack(spacing: 0) {
            TextField("Buscar", text: $modelo.busqueda)
                .textFieldStyle(.roundedBorder)
                .padding(10)
            if modelo.filas.isEmpty {
                Spacer()
                Text("No hay pines abiertos").foregroundStyle(.secondary)
                Spacer()
            } else {
                List {
                    ForEach(grupos) { g in
                        Section {
                            ForEach(g.filas) { f in fila(f) }
                        } header: {
                            HStack(spacing: 6) {
                                if let c = g.grupo {
                                    Circle().fill(Color(nsColor: ColoresGrupo.nsColor(c))).frame(width: 9, height: 9)
                                    Text(ColoresGrupo.nombres[c % ColoresGrupo.nombres.count])
                                } else {
                                    Text("Sin grupo")
                                }
                            }
                        }
                    }
                }
                .listStyle(.sidebar)
            }
            Divider()
            HStack {
                Button("Mostrar todos") { modelo.alMostrarTodos?() }
                Button("Ocultar todos") { modelo.alOcultarTodos?() }
                Spacer()
                if modelo.puedeDeshacer {
                    Button("Deshacer") { modelo.alDeshacer?() }
                        .keyboardShortcut("z", modifiers: .command)
                } else {
                    Button("Cerrar todos", role: .destructive) { modelo.alCerrarTodos?() }
                        .disabled(modelo.filas.isEmpty)
                }
            }
            .controlSize(.small)
            .padding(10)
        }
        .frame(minWidth: 300, minHeight: 320)
    }

    private func fila(_ f: FilaPin) -> some View {
        HStack(spacing: 8) {
            Image(systemName: f.tipo.simbolo)
                .frame(width: 22)
                .foregroundStyle(f.oculto ? .secondary : .primary)
            VStack(alignment: .leading, spacing: 1) {
                Text(f.nombre).lineLimit(1)
                Text(f.detalle).font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer()
            Button {
                modelo.alAlternar?(f.id)
            } label: {
                Image(systemName: f.oculto ? "eye.slash" : "eye")
            }
            .buttonStyle(.borderless)
            .help(f.oculto ? "Mostrar" : "Ocultar")
            Button {
                modelo.alCerrar?(f.id)
            } label: {
                Image(systemName: "xmark")
            }
            .buttonStyle(.borderless)
            .help("Cerrar")
        }
        .contentShape(Rectangle())
        .onTapGesture { modelo.alTraer?(f.id) }
        .opacity(f.oculto ? 0.6 : 1)
    }
}

/// La ventana del panel: flotante, como los pines, y sin robar el foco.
@MainActor
final class PanelPinesAbiertos {
    let modelo = ModeloPinesAbiertos()
    private var panel: NSPanel?

    func mostrar() {
        if let panel {
            panel.orderFrontRegardless()
            return
        }
        let p = NSPanel(contentRect: CGRect(x: 0, y: 0, width: 320, height: 460),
                        styleMask: [.titled, .closable, .resizable, .utilityWindow, .nonactivatingPanel],
                        backing: .buffered, defer: true)
        p.title = "Pines abiertos"
        p.isFloatingPanel = true
        p.level = .floating
        p.hidesOnDeactivate = false
        p.isReleasedWhenClosed = false
        p.becomesKeyOnlyIfNeeded = true
        p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        p.contentView = NSHostingView(rootView: VistaPinesAbiertos(modelo: modelo))
        if let visible = (NSScreen.main ?? NSScreen.screens.first)?.visibleFrame {
            p.setFrameOrigin(CGPoint(x: visible.maxX - 340, y: visible.maxY - 480))
        }
        panel = p
        p.orderFrontRegardless()
    }

    var abierto: Bool { panel?.isVisible ?? false }
}
