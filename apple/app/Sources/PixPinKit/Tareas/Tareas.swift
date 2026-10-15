// Tareas: las listas de tareas de TODOS los chats en una ventana, como la de
// Windows. No hay una segunda copia: cada lista es la mini-app `tareas` de
// un mensaje del chat y marcar reescribe ese mensaje (en el nucleo de Rust,
// con `mini::alternar`), que es lo que viaja al movil al sincronizar.
//
// El formato no tiene fecha de vencimiento: la unica fecha es la de
// creacion (`➕ AAAA-MM-DD`). Por eso lo pendiente se agrupa por lo que
// lleva esperando (hoy, esta semana, este mes, mas, sin fecha), la mas vieja
// arriba como en Windows; lo hecho va aparte y plegado.

import AppKit
import Foundation
import PixPinCore
import SwiftUI

/// Una tarea con su lista, para ensenarla fuera de ella.
public struct TareaConLista: Identifiable, Hashable {
    public let lista: ListaTareas
    public let tarea: Tarea
    public var id: String { "\(lista.chat)/\(lista.mensaje)/\(tarea.indice)" }
}

/// Las tareas pendientes de un grupo.
public struct GrupoPendiente: Identifiable {
    public let grupo: GrupoDeTareas
    public let tareas: [TareaConLista]
    public var id: GrupoDeTareas { grupo }
}

/// Un grupo de lo pendiente por lo que lleva esperando.
public enum GrupoDeTareas: Int, CaseIterable, Identifiable, Comparable {
    case masDeUnMes, esteMes, estaSemana, hoy, sinFecha

    public var id: Int { rawValue }

    public var titulo: String {
        switch self {
        case .masDeUnMes: "Más de un mes"
        case .esteMes: "Este mes"
        case .estaSemana: "Esta semana"
        case .hoy: "Hoy"
        case .sinFecha: "Sin fecha"
        }
    }

    /// El grupo de una tarea por los dias que lleva.
    public static func de(_ t: Tarea) -> GrupoDeTareas {
        guard let d = t.dias else { return .sinFecha }
        switch d {
        case 0: return .hoy
        case 1...7: return .estaSemana
        case 8...31: return .esteMes
        default: return .masDeUnMes
        }
    }

    public static func < (a: GrupoDeTareas, b: GrupoDeTareas) -> Bool { a.rawValue < b.rawValue }
}

@MainActor
public final class ModeloTareas: ObservableObject {
    @Published public private(set) var listas: [ListaTareas] = []
    @Published public private(set) var error: String?
    @Published public private(set) var cargado = false

    private let nucleo: Nucleo

    public init(nucleo: Nucleo) {
        self.nucleo = nucleo
    }

    /// Lo pendiente de todas las listas por grupos, la mas vieja arriba (y
    /// a igual fecha, el orden de su lista), como `orden_de_pendiente` de
    /// Windows.
    public var pendientesPorGrupo: [GrupoPendiente] {
        var grupos: [GrupoDeTareas: [TareaConLista]] = [:]
        for l in listas {
            for t in l.tareas where !t.hecha {
                grupos[GrupoDeTareas.de(t), default: []].append(TareaConLista(lista: l, tarea: t))
            }
        }
        return GrupoDeTareas.allCases.compactMap { g in
            guard let v = grupos[g], !v.isEmpty else { return nil }
            let ordenadas = v.sorted { a, b in
                let fa = a.tarea.creada ?? "", fb = b.tarea.creada ?? ""
                if fa != fb { return fa < fb }
                return a.tarea.indice < b.tarea.indice
            }
            return GrupoPendiente(grupo: g, tareas: ordenadas)
        }
    }

    /// Lo hecho, en el orden de sus listas.
    public var hechas: [TareaConLista] {
        listas.flatMap { l in l.tareas.filter(\.hecha).map { TareaConLista(lista: l, tarea: $0) } }
    }

    public func cargar() async {
        let nucleo = self.nucleo
        let desfase = desfaseLocalMs()
        do {
            listas = try await Task.detached(priority: .userInitiated) {
                try nucleo.tareas(desfaseMs: desfase)
            }.value
            error = nil
        } catch {
            self.error = "\(error)"
        }
        cargado = true
    }

    /// Pone una tarea como hecha o pendiente en su mensaje. Si la lista
    /// cambio fuera (el numero ya es otra tarea), no se toca y se relee.
    public func marcar(_ t: TareaConLista, hecha: Bool) async {
        let nucleo = self.nucleo
        let (chat, mensaje, indice, crudo) = (t.lista.chat, t.lista.mensaje, t.tarea.indice, t.tarea.crudo)
        // El aviso se pone DESPUES de releer: `cargar` limpia el error al
        // leer bien, y el usuario tiene que saber que su clic no se aplico.
        var aviso: String?
        do {
            let hecho = try await Task.detached(priority: .userInitiated) {
                try nucleo.marcarTarea(chat: chat, mensaje: mensaje, indice: indice, crudo: crudo, hecha: hecha)
            }.value
            if hecho {
                NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            } else {
                aviso = "La lista cambió: se ha vuelto a leer"
            }
        } catch {
            aviso = "\(error)"
        }
        await cargar()
        if let aviso { error = aviso }
    }

    /// Apunta una tarea en el Inbox de «Mensajes guardados».
    public func apuntar(_ texto: String) async {
        let limpio = texto.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !limpio.isEmpty else { return }
        let nucleo = self.nucleo
        let desfase = desfaseLocalMs()
        do {
            _ = try await Task.detached(priority: .userInitiated) {
                try nucleo.apuntarTarea(texto: limpio, desfaseMs: desfase)
            }.value
            error = nil
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        } catch {
            self.error = "\(error)"
        }
        await cargar()
    }
}

// MARK: - La ventana

public struct VentanaTareas: View {
    public enum Vista: String, CaseIterable, Identifiable {
        case porFecha = "Por fecha"
        case porLista = "Por lista"
        public var id: String { rawValue }
    }

    @StateObject private var modelo: ModeloTareas
    @State private var vista: Vista = .porFecha
    @State private var nueva = ""
    @State private var verHechas = false

    public init(almacen: Almacen) {
        _modelo = StateObject(wrappedValue: ModeloTareas(nucleo: almacen.nucleo))
    }

    public var body: some View {
        VStack(spacing: 0) {
            HStack {
                TextField("Apuntar en el Inbox", text: $nueva)
                    .textFieldStyle(.roundedBorder)
                    .onSubmit(apuntar)
                Button("Apuntar", action: apuntar)
                    .disabled(nueva.trimmingCharacters(in: .whitespaces).isEmpty)
            }
            .padding(10)
            Divider()
            List {
                switch vista {
                case .porFecha: porFecha
                case .porLista: porLista
                }
            }
            .overlay {
                if modelo.cargado, modelo.listas.allSatisfy({ $0.tareas.isEmpty }) {
                    ContentUnavailableView("Sin tareas", systemImage: "checklist",
                                           description: Text("Las listas de tareas de tus chats aparecen aquí."))
                }
            }
        }
        .overlay(alignment: .bottom) {
            if let e = modelo.error {
                Text(e).font(.caption).padding(6)
                    .background(.red.opacity(0.15), in: RoundedRectangle(cornerRadius: 6))
                    .padding()
            }
        }
        .toolbar {
            ToolbarItem(placement: .principal) {
                Picker("Ver", selection: $vista) {
                    ForEach(Vista.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented)
            }
            ToolbarItem {
                Button { Task { await modelo.cargar() } } label: { Label("Releer", systemImage: "arrow.clockwise") }
                    .keyboardShortcut("r")
            }
        }
        .navigationTitle("Tareas")
        .task { await modelo.cargar() }
        .frame(minWidth: 480, minHeight: 400)
    }

    @ViewBuilder
    private var porFecha: some View {
        ForEach(modelo.pendientesPorGrupo) { g in
            Section(g.grupo.titulo) {
                ForEach(g.tareas) { t in
                    FilaTarea(t: t, conLista: true) { hecha in Task { await modelo.marcar(t, hecha: hecha) } }
                }
            }
        }
        let hechas = modelo.hechas
        if !hechas.isEmpty {
            Section {
                DisclosureGroup("Hechas (\(hechas.count))", isExpanded: $verHechas) {
                    ForEach(hechas) { t in
                        FilaTarea(t: t, conLista: true) { hecha in Task { await modelo.marcar(t, hecha: hecha) } }
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var porLista: some View {
        ForEach(modelo.listas, id: \.mensaje) { l in
            Section {
                ForEach(l.tareas, id: \.indice) { t in
                    let x = TareaConLista(lista: l, tarea: t)
                    FilaTarea(t: x, conLista: false) { hecha in Task { await modelo.marcar(x, hecha: hecha) } }
                }
            } header: {
                HStack {
                    Text(l.titulo.isEmpty ? "Tareas" : l.titulo)
                    Text(l.guardados ? "Mensajes guardados" : l.nombreChat).foregroundStyle(.secondary)
                    Spacer()
                    Text("\(l.tareas.count - Int(l.pendientes)) de \(l.tareas.count)").monospacedDigit()
                }
            }
        }
    }

    private func apuntar() {
        let texto = nueva
        nueva = ""
        Task { await modelo.apuntar(texto) }
    }
}

/// Una tarea: su casilla, el texto (tachado si esta hecha), cuantos dias
/// lleva y, si se pide, su lista y su chat.
struct FilaTarea: View {
    let t: TareaConLista
    let conLista: Bool
    let alMarcar: (Bool) -> Void

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Toggle("", isOn: Binding(get: { t.tarea.hecha }, set: { alMarcar($0) }))
                .toggleStyle(.checkbox)
                .labelsHidden()
            VStack(alignment: .leading, spacing: 2) {
                Text(t.tarea.texto.isEmpty ? "(sin texto)" : t.tarea.texto)
                    .strikethrough(t.tarea.hecha)
                    .foregroundStyle(t.tarea.hecha ? Color.secondary : Color.primary)
                    .textSelection(.enabled)
                if conLista {
                    Text("\(t.lista.titulo) · \(t.lista.guardados ? "Mensajes guardados" : t.lista.nombreChat)")
                        .font(.caption).foregroundStyle(.secondary)
                }
            }
            Spacer()
            if !t.tarea.imagenes.isEmpty {
                Image(systemName: "photo").foregroundStyle(.secondary)
                    .help("\(t.tarea.imagenes.count) imágenes")
            }
            if let d = t.tarea.dias, !t.tarea.hecha {
                Text(d == 0 ? "hoy" : "\(d) d")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(d > 7 ? Color.orange : Color.secondary)
                    .help("Creada el \(t.tarea.creada ?? "")")
            }
        }
    }
}
