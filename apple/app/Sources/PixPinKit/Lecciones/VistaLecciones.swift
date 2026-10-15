// La ventana de Lecciones: a la izquierda la lista (buscar, filtrar por
// etiqueta, «para repasar») por paginas; a la derecha la ficha para
// escribirla o repasarla. Como la pestana «Lecciones» del timeline de
// Windows y la ficha «Nueva leccion».

import PixPinCore
import SwiftUI

public struct VentanaLecciones: View {
    @StateObject private var modelo: ModeloLecciones
    @State private var elegida: String?
    @State private var nueva = false

    public init(nucleo: Nucleo) {
        _modelo = StateObject(wrappedValue: ModeloLecciones(nucleo: nucleo))
    }

    public var body: some View {
        NavigationSplitView {
            VStack(spacing: 0) {
                filtros
                List(selection: $elegida) {
                    ForEach(modelo.lecciones, id: \.id) { l in
                        FilaLeccion(leccion: l)
                            .tag(l.id)
                            .onAppear {
                                if l.id == modelo.lecciones.last?.id {
                                    Task { await modelo.cargarMas() }
                                }
                            }
                    }
                }
                .overlay {
                    if modelo.lecciones.isEmpty && !modelo.cargando {
                        let titulo: String = modelo.texto.isEmpty ? "Sin lecciones" : "Nada encontrado"
                        ContentUnavailableView { Label(titulo, systemImage: "lightbulb") }
                    }
                }
                Text("\(modelo.total) lecciones")
                    .font(.caption).foregroundStyle(.secondary)
                    .padding(6)
            }
            .navigationSplitViewColumnWidth(min: 260, ideal: 320)
            .searchable(text: $modelo.texto, placement: .sidebar, prompt: "Buscar lecciones")
            .toolbar {
                ToolbarItem {
                    Button {
                        elegida = nil
                        nueva = true
                    } label: { Label("Nueva lección", systemImage: "plus") }
                }
            }
        } detail: {
            if nueva {
                FichaLeccionVista(modelo: modelo, ficha: nil) { guardada in
                    nueva = false
                    elegida = guardada?.id
                }
                .id("nueva")
            } else if let id = elegida, let l = modelo.lecciones.first(where: { $0.id == id }) {
                FichaLeccionVista(modelo: modelo, ficha: l) { _ in }
                    .id(l.id + "\(l.tocada)")
            } else {
                ContentUnavailableView("Elige una lección o escribe una nueva", systemImage: "lightbulb")
            }
        }
        .onChange(of: elegida) { _, v in if v != nil { nueva = false } }
        .task { await modelo.recargar() }
        .overlay(alignment: .bottom) {
            if let error = modelo.error {
                Text(error).font(.caption).padding(6)
                    .background(.red.opacity(0.15), in: RoundedRectangle(cornerRadius: 6))
                    .padding()
            }
        }
    }

    private var filtros: some View {
        HStack {
            Toggle(isOn: $modelo.soloParaRepasar) {
                Text(modelo.paraRepasar > 0 ? "Para repasar (\(modelo.paraRepasar))" : "Para repasar")
            }
            .toggleStyle(.button)
            Spacer()
            Picker("Etiqueta", selection: $modelo.etiqueta) {
                Text("Todas").tag(String?.none)
                ForEach(modelo.etiquetas, id: \.etiqueta) { e in
                    Text("#\(e.etiqueta) (\(e.cuantas))").tag(Optional(e.etiqueta))
                }
            }
            .labelsHidden()
            .frame(maxWidth: 160)
        }
        .padding(8)
    }
}

struct FilaLeccion: View {
    let leccion: FichaLeccion

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(alignment: .firstTextBaseline) {
                Text(leccion.icono)
                Text(leccion.titulo).lineLimit(2)
                Spacer(minLength: 0)
                if leccion.veces > 1 {
                    Text("🔁\(leccion.veces)").font(.caption)
                }
                if leccion.tocaRepasar {
                    Image(systemName: "clock.badge.exclamationmark").foregroundStyle(.orange)
                        .help("Toca repasarla")
                }
            }
            let etiquetas = leccion.etiquetas + leccion.etiquetasAuto
            if !etiquetas.isEmpty {
                Text(etiquetas.map { "#\($0)" }.joined(separator: " "))
                    .font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
        }
        .padding(.vertical, 2)
    }
}

/// La ficha: escribir una nueva, editar una que existe y repasarla.
struct FichaLeccionVista: View {
    @ObservedObject var modelo: ModeloLecciones
    let ficha: FichaLeccion?
    let alAcabar: (FichaLeccion?) -> Void

    @State private var b = BorradorLeccion()
    @State private var preguntarBorrar = false
    @State private var guardando = false
    @State private var proponer: Task<Void, Never>?

    var body: some View {
        Form {
            Section("Lo que aprendí") {
                TextField("En una frase (lo único obligatorio)", text: $b.titulo, axis: .vertical)
                    .lineLimit(1 ... 3)
                Picker("Tipo", selection: $b.tipo) {
                    ForEach(tiposDeLeccion) { t in Text(t.nombre).tag(t.id) }
                }
                Picker("Gravedad", selection: $b.gravedad) {
                    Text("Leve").tag(Int64(1))
                    Text("Importante").tag(Int64(2))
                    Text("Grave").tag(Int64(3))
                }
                .pickerStyle(.segmented)
            }
            Section("Detalle") {
                TextField("Qué pasó", text: $b.quePaso, axis: .vertical).lineLimit(1 ... 5)
                TextField("Por qué pasó", text: $b.porQue, axis: .vertical).lineLimit(1 ... 5)
                TextField("La próxima vez (si…, entonces…)", text: $b.proxima, axis: .vertical).lineLimit(1 ... 5)
                Picker("Área", selection: $b.area) {
                    Text("Sin área").tag("")
                    ForEach(areasDeLeccion, id: \.self) { Text($0).tag($0) }
                    if !b.area.isEmpty && !areasDeLeccion.contains(b.area) {
                        Text(b.area).tag(b.area)
                    }
                }
            }
            Section("Etiquetas") {
                TextField("Etiquetas (separadas por comas)", text: $b.etiquetasEscritas)
                if !b.etiquetasAuto.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Propuestas ✨").font(.caption).foregroundStyle(.secondary)
                        HStack {
                            ForEach(b.etiquetasAuto, id: \.self) { e in
                                HStack(spacing: 2) {
                                    Button("#\(e)") { b.aceptar(e) }
                                        .buttonStyle(.borderless)
                                        .help("Ponerla")
                                    Button { b.quitar(e) } label: { Image(systemName: "xmark.circle.fill") }
                                        .buttonStyle(.borderless)
                                        .foregroundStyle(.secondary)
                                        .help("Quitarla: no vuelve a salir")
                                }
                            }
                        }
                    }
                }
                DisclosureGroup("Causas (\(b.causas.count))") {
                    ForEach(causasDeLeccion, id: \.self) { c in
                        Toggle(c, isOn: Binding(
                            get: { b.causas.contains(c) },
                            set: { on in
                                if on { if !b.causas.contains(c) { b.causas.append(c) } } else { b.causas.removeAll { $0 == c } }
                            }
                        ))
                    }
                }
            }
            if let f = ficha {
                Section("Repaso") {
                    Text(textoDeRepaso(f))
                        .foregroundStyle(f.tocaRepasar ? Color.orange : Color.secondary)
                    HStack {
                        Button("Lo recordaba") { repasar(.recordaba) }
                        Button("A medias") { repasar(.aMedias) }
                        Button("Lo olvidé") { repasar(.olvide) }
                        Spacer()
                        Button("Volvió a pasar") { repasar(.repetida) }
                    }
                    if f.veces > 1 {
                        Text("Pasó \(f.veces) veces").font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
        }
        .formStyle(.grouped)
        .toolbar {
            ToolbarItemGroup {
                if ficha != nil {
                    Button(role: .destructive) { preguntarBorrar = true } label: {
                        Label("Borrar", systemImage: "trash")
                    }
                }
                Button(ficha == nil ? "Guardar lección" : "Guardar cambios") { guardar() }
                    .keyboardShortcut("s")
                    .disabled(b.titulo.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || guardando)
            }
        }
        .confirmationDialog("¿Borrar esta lección?", isPresented: $preguntarBorrar) {
            Button("Borrar", role: .destructive) {
                guard let id = ficha?.id else { return }
                Task {
                    await modelo.borrar(id)
                    alAcabar(nil)
                }
            }
        } message: {
            Text("Se borra también en los demás aparatos al sincronizar.")
        }
        .onAppear {
            if let ficha { b = BorradorLeccion(ficha) }
        }
        .onChange(of: b.textoEntero) { _, _ in programarPropuesta() }
        .onDisappear { proponer?.cancel() }
    }

    /// Las etiquetas propuestas, tras una pausa al escribir (sin pisar lo
    /// que el usuario ya puso o quito).
    private func programarPropuesta() {
        proponer?.cancel()
        let texto = b.textoEntero
        let quitadas = b.quitadas
        proponer = Task {
            try? await Task.sleep(nanoseconds: 400_000_000)
            guard !Task.isCancelled, !texto.isEmpty else { return }
            let p = await modelo.proponer(texto: texto, quitadas: quitadas)
            guard !Task.isCancelled else { return }
            b.etiquetasAuto = p.etiquetas.filter { !b.etiquetas.contains($0) && !b.quitadas.contains($0) }
            if ficha == nil {
                if b.area.isEmpty, let a = p.area { b.area = a }
                if b.tipo == "leccion", let t = p.tipo { b.tipo = t }
            }
        }
    }

    private func guardar() {
        guardando = true
        let editada = b.editada
        Task {
            let f = await modelo.guardar(editada)
            guardando = false
            if f != nil { alAcabar(f) }
        }
    }

    private func textoDeRepaso(_ f: FichaLeccion) -> String {
        if f.tocaRepasar { return "Toca repasarla hoy." }
        let cuando = Date(timeIntervalSince1970: Double(f.repasar) / 1000)
        return "Próximo repaso: " + cuando.formatted(date: .abbreviated, time: .omitted)
    }

    private func repasar(_ nota: NotaRepaso) {
        guard let id = ficha?.id else { return }
        Task { _ = await modelo.repasar(id, nota: nota) }
    }
}
