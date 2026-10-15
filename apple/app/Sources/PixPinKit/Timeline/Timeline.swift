// El timeline: a la izquierda «Hoy» (las ultimas 24 horas) y los dias con
// momentos, el mas nuevo arriba; a la derecha, los momentos del dia elegido
// como una linea de tiempo. Lo que se lee es `timeline/momentos.jsonl`, el
// mismo fichero de Windows, por el nucleo de Rust (`pixpin_timeline`).
//
// Rendimiento (§4bis del plan): los dias y los momentos llegan por paginas
// (la siguiente al llegar al final de la lista, `LazyVStack`), fuera del
// hilo principal; las fotos son miniaturas de ImageIO de la cache comun.

import AppKit
import Foundation
import PixPinCore
import QuickLook
import SwiftUI

@MainActor
public final class ModeloTimeline: ObservableObject {
    /// Que se ve: todos los momentos o solo los mandados a «Momentos».
    public enum Filtro: String, CaseIterable, Identifiable {
        case todo = "Todo"
        case momentos = "Momentos"
        public var id: String { rawValue }
    }

    /// Lo elegido en la lista de la izquierda.
    public enum Eleccion: Hashable, Sendable {
        case hoy
        case dia(String)
    }

    public nonisolated static let tamPaginaDias: UInt32 = 60
    public nonisolated static let tamPaginaMomentos: UInt32 = 80

    @Published public var filtro: Filtro = .todo
    @Published public private(set) var dias: [DiaTimeline] = []
    @Published public private(set) var totalDias: UInt64 = 0
    @Published public private(set) var momentos: [MomentoTimeline] = []
    @Published public private(set) var totalMomentos: UInt64 = 0
    @Published public private(set) var error: String?
    @Published public private(set) var eleccion: Eleccion = .hoy

    private let nucleo: Nucleo
    /// Sube con cada cambio de dia: una pagina que llega tarde de un dia que
    /// ya no se ve no pisa la del nuevo.
    private var generacion = 0
    private var cargandoDias = false
    private var cargandoMomentos = false

    public init(nucleo: Nucleo) {
        self.nucleo = nucleo
    }

    public var quedanDias: Bool { UInt64(dias.count) < totalDias }
    public var quedanMomentos: Bool { UInt64(momentos.count) < totalMomentos }

    /// Relee los dias (desde el principio) y lo elegido.
    public func recargar() async {
        await cargarDias(desde: 0, sustituir: true)
        await elegir(eleccion)
    }

    public func cargarMasDias() async {
        guard quedanDias else { return }
        await cargarDias(desde: UInt64(dias.count), sustituir: false)
    }

    private func cargarDias(desde: UInt64, sustituir: Bool) async {
        guard !cargandoDias else { return }
        cargandoDias = true
        defer { cargandoDias = false }
        let nucleo = self.nucleo
        let solo = filtro == .momentos
        let desfase = desfaseLocalMs()
        do {
            let p = try await Task.detached(priority: .userInitiated) {
                try nucleo.timelineDias(desfaseMs: desfase, soloEnMomentos: solo, desde: desde,
                                        cuantos: ModeloTimeline.tamPaginaDias)
            }.value
            dias = sustituir ? p.dias : dias + p.dias
            totalDias = p.total
            error = nil
        } catch {
            self.error = "\(error)"
        }
    }

    /// Ensena «Hoy» o un dia, desde su primer momento.
    public func elegir(_ e: Eleccion) async {
        eleccion = e
        generacion += 1
        cargandoMomentos = false
        momentos = []
        totalMomentos = 0
        await cargarMomentos(desde: 0)
    }

    public func cargarMasMomentos() async {
        guard quedanMomentos else { return }
        await cargarMomentos(desde: UInt64(momentos.count))
    }

    private func cargarMomentos(desde: UInt64) async {
        guard !cargandoMomentos else { return }
        cargandoMomentos = true
        let mia = generacion
        let nucleo = self.nucleo
        let solo = filtro == .momentos
        let desfase = desfaseLocalMs()
        let eleccion = self.eleccion
        let resultado = await Task.detached(priority: .userInitiated) { () -> Result<PaginaMomentos, Error> in
            Result<PaginaMomentos, Error>(catching: { () throws -> PaginaMomentos in
                switch eleccion {
                case .hoy:
                    return try nucleo.timelineUltimas24h(desfaseMs: desfase, desde: desde,
                                                         cuantos: ModeloTimeline.tamPaginaMomentos)
                case let .dia(iso):
                    guard let f = ModeloTimeline.partes(iso) else {
                        return PaginaMomentos(momentos: [], desde: 0, total: 0)
                    }
                    return try nucleo.timelineMomentos(anio: f.0, mes: f.1, dia: f.2, desfaseMs: desfase,
                                                       soloEnMomentos: solo, desde: desde,
                                                       cuantos: ModeloTimeline.tamPaginaMomentos)
                }
            })
        }.value
        guard mia == generacion else { return }
        cargandoMomentos = false
        switch resultado {
        case let .success(p):
            momentos.append(contentsOf: p.momentos)
            totalMomentos = p.total
            error = nil
        case let .failure(e):
            error = "\(e)"
        }
    }

    /// `AAAA-MM-DD` en sus tres numeros.
    nonisolated static func partes(_ iso: String) -> (Int32, UInt8, UInt8)? {
        let p = iso.split(separator: "-")
        guard p.count == 3, let a = Int32(p[0]), let m = UInt8(p[1]), let d = UInt8(p[2]) else { return nil }
        return (a, m, d)
    }
}

// MARK: - La ventana

public struct VentanaTimeline: View {
    @StateObject private var modelo: ModeloTimeline

    public init(almacen: Almacen) {
        _modelo = StateObject(wrappedValue: ModeloTimeline(nucleo: almacen.nucleo))
    }

    public var body: some View {
        NavigationSplitView {
            List(selection: Binding<ModeloTimeline.Eleccion?>(
                get: { modelo.eleccion },
                set: { nueva in
                    if let nueva { Task { await modelo.elegir(nueva) } }
                })) {
                Label("Hoy", systemImage: "sun.max").tag(ModeloTimeline.Eleccion.hoy)
                Section("Días") {
                    ForEach(modelo.dias, id: \.iso) { d in
                        HStack {
                            Text(ModeloTimeline.rotulo(d))
                            Spacer()
                            Text(String(d.cuantos)).foregroundStyle(.secondary).monospacedDigit()
                        }
                        .tag(ModeloTimeline.Eleccion.dia(d.iso))
                    }
                    if modelo.quedanDias {
                        ProgressView().frame(maxWidth: .infinity)
                            .task { await modelo.cargarMasDias() }
                    }
                }
            }
            .navigationSplitViewColumnWidth(min: 200, ideal: 240)
            .safeAreaInset(edge: .top) {
                Picker("Ver", selection: $modelo.filtro) {
                    ForEach(ModeloTimeline.Filtro.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .padding(8)
            }
        } detail: {
            LineaDeMomentos(modelo: modelo)
        }
        .onChange(of: modelo.filtro) { _, _ in Task { await modelo.recargar() } }
        .task { await modelo.recargar() }
        .toolbar {
            Button { Task { await modelo.recargar() } } label: {
                Label("Releer", systemImage: "arrow.clockwise")
            }
            .keyboardShortcut("r")
        }
        .navigationTitle("Timeline")
    }
}

extension ModeloTimeline {
    /// «jueves, 15 de octubre de 2026» en el idioma del sistema.
    nonisolated static func rotulo(_ d: DiaTimeline) -> String {
        var c = DateComponents()
        c.year = Int(d.anio)
        c.month = Int(d.mes)
        c.day = Int(d.dia)
        c.hour = 12
        guard let f = Calendar.current.date(from: c) else { return d.iso }
        return f.formatted(.dateTime.weekday(.wide).day().month(.wide).year())
    }
}

/// Los momentos de lo elegido, como una linea de tiempo vertical.
struct LineaDeMomentos: View {
    @ObservedObject var modelo: ModeloTimeline
    @State private var vistaRapida: URL?

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                ForEach(modelo.momentos, id: \.id) { m in
                    TarjetaMomento(momento: m, vistaRapida: $vistaRapida)
                }
                if modelo.quedanMomentos {
                    ProgressView().frame(maxWidth: .infinity).padding()
                        .task { await modelo.cargarMasMomentos() }
                }
            }
            .padding()
        }
        .overlay {
            if modelo.momentos.isEmpty, !modelo.quedanMomentos {
                ContentUnavailableView("Nada apuntado",
                                       systemImage: "clock",
                                       description: Text("Los momentos que apuntas en PixPin aparecen aquí, minuto a minuto."))
            }
        }
        .overlay(alignment: .bottom) {
            if let e = modelo.error {
                Text(e).font(.caption).padding(6)
                    .background(.red.opacity(0.15), in: RoundedRectangle(cornerRadius: 6))
                    .padding()
            }
        }
        .quickLookPreview($vistaRapida)
    }
}

/// Un momento: la hora y un punto en la linea, el titulo, lo contado, sus
/// fotos y su audio.
struct TarjetaMomento: View {
    let momento: MomentoTimeline
    @Binding var vistaRapida: URL?

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(spacing: 4) {
                Text(momento.hora).font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                Circle().fill(momento.enMomentos ? Color.orange : Color.accentColor).frame(width: 9, height: 9)
                Rectangle().fill(.quaternary).frame(width: 2).frame(maxHeight: .infinity)
            }
            .frame(width: 44)
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 6) {
                    Text(momento.titulo.isEmpty ? "Sin título" : momento.titulo).font(.headline)
                    if momento.leccion != nil {
                        Image(systemName: "lightbulb.fill").foregroundStyle(.yellow).help("Lección aprendida")
                    }
                    if momento.enMomentos {
                        Image(systemName: "star.fill").foregroundStyle(.orange).help("En Momentos")
                    }
                }
                if !momento.descripcion.isEmpty {
                    Text(momento.descripcion).textSelection(.enabled)
                }
                if !momento.fotos.isEmpty {
                    ScrollView(.horizontal, showsIndicators: false) {
                        LazyHStack(spacing: 6) {
                            ForEach(momento.fotos, id: \.self) { ruta in
                                let url = URL(fileURLWithPath: ruta)
                                MiniaturaVista(url: url, lado: 140)
                                    .onTapGesture { vistaRapida = url }
                            }
                        }
                    }
                }
                if let audio = momento.audio {
                    Button {
                        vistaRapida = URL(fileURLWithPath: audio)
                    } label: {
                        Label(momento.duracionMs > 0 ? TarjetaMomento.duracion(momento.duracionMs) : "Audio",
                              systemImage: "play.circle")
                    }
                    .buttonStyle(.borderless)
                }
            }
            .padding(.bottom, 16)
            Spacer(minLength: 0)
        }
    }

    nonisolated static func duracion(_ ms: Int64) -> String {
        let s = Int(ms / 1000)
        return String(format: "%d:%02d", s / 60, s % 60)
    }
}
