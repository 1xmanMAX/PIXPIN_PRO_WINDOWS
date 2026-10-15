// La ventana de Sincronizar: quien es este Mac, el codigo del grupo, los
// aparatos del grupo que hay en la Wi-Fi y los botones. Como la ventana de
// Windows, en pequeno.

import AppKit
import PixPinCore
import SwiftUI

public struct VistaSincro: View {
    @ObservedObject var sincro: Sincronizador
    @ObservedObject var vecinos: Vecinos
    @State private var codigo = ""
    @State private var direccion = ""

    public init(sincro: Sincronizador) {
        _sincro = ObservedObject(wrappedValue: sincro)
        _vecinos = ObservedObject(wrappedValue: sincro.vecinos)
    }

    public var body: some View {
        Form {
            Section("Este Mac") {
                LabeledContent("Nombre", value: sincro.aparato?.nombre ?? "—")
                if let letra = sincro.aparato?.letra, !letra.isEmpty {
                    LabeledContent("Letra en el grupo", value: letra.uppercased())
                }
                if let puerto = sincro.puerto {
                    LabeledContent("Escucha en el puerto", value: String(puerto))
                }
            }

            if let codigo = sincro.codigoLegible {
                grupo(codigo)
            } else {
                sinGrupo
            }

            if sincro.fase != nil || sincro.ultimoResultado != nil || sincro.error != nil {
                Section("Actividad") {
                    if let fase = sincro.fase {
                        HStack { ProgressView().controlSize(.small); Text(fase) }
                    }
                    if let r = sincro.ultimoResultado {
                        Text(r).textSelection(.enabled)
                    }
                    if let e = sincro.error {
                        Label(e, systemImage: "exclamationmark.triangle").foregroundStyle(.red)
                            .textSelection(.enabled)
                    }
                }
            }
        }
        .formStyle(.grouped)
        .frame(minWidth: 460, minHeight: 420)
    }

    // MARK: - Con grupo

    private func grupo(_ legible: String) -> some View {
        Group {
            Section {
                HStack {
                    Text(legible)
                        .font(.system(.title2, design: .monospaced).weight(.semibold))
                        .textSelection(.enabled)
                    Spacer()
                    Button {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(legible, forType: .string)
                    } label: { Image(systemName: "doc.on.doc") }
                        .help("Copiar el código")
                }
            } header: {
                Text("Código del grupo")
            } footer: {
                Text("En el otro aparato: Sincronizar › Unirse a un grupo, con este código.")
                    .font(.caption).foregroundStyle(.secondary)
            }

            Section {
                if sincro.filas.isEmpty {
                    Text(vecinos.buscando ? "Buscando aparatos del grupo en esta Wi-Fi…"
                        : "No se está buscando.")
                        .foregroundStyle(.secondary)
                }
                ForEach(sincro.filas) { fila in
                    FilaVecino(fila: fila, ocupado: sincro.ocupado) {
                        Task { await sincro.sincronizarAhora(id: fila.id) }
                    }
                }
            } header: {
                Text("Aparatos del grupo")
            }

            Section {
                Toggle("Sincronizar solo (al aparecer un aparato y 30 s después de un cambio)",
                       isOn: $sincro.automatica)
                if sincro.bajoConsumo {
                    Label("Modo de bajo consumo: solo a mano.", systemImage: "battery.25")
                        .foregroundStyle(.secondary)
                }
                Button("Sincronizar ahora con todos") {
                    Task { await sincro.sincronizarAhora() }
                }
                .disabled(sincro.ocupado || !sincro.filas.contains { $0.direccion != nil })
            }
        }
    }

    // MARK: - Sin grupo

    private var sinGrupo: some View {
        Group {
            Section {
                Text("Crea un grupo aquí y únete desde tus otros aparatos, o únete al grupo que ya tienes en tu PC o tu móvil.")
                    .foregroundStyle(.secondary)
                Button("Crear un grupo") { Task { await sincro.crearGrupo() } }
                    .disabled(sincro.ocupado)
            } header: {
                Text("Grupo")
            }
            Section {
                TextField("Código (K7Q2M-9XMPA)", text: $codigo)
                    .font(.system(.body, design: .monospaced))
                TextField("Dirección (opcional: 192.168.1.20:47474)", text: $direccion)
                Button("Unirse") {
                    let dir = Direccion(texto: direccion)
                    Task { await sincro.unirse(codigo: codigo, direccion: dir) }
                }
                .disabled(sincro.ocupado || !codigoValido(codigo: codigo))
            } header: {
                Text("Unirse a un grupo")
            } footer: {
                Text("Sin dirección se busca en la Wi-Fi un aparato con ese código y PixPin abierto.")
                    .font(.caption).foregroundStyle(.secondary)
            }
        }
    }
}

struct FilaVecino: View {
    let fila: Sincronizador.Fila
    let ocupado: Bool
    let sincronizar: () -> Void

    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(fila.nombre).font(.headline)
                    if !fila.letra.isEmpty {
                        Text(fila.letra.uppercased()).font(.caption.monospaced())
                            .padding(.horizontal, 4)
                            .background(.quaternary, in: RoundedRectangle(cornerRadius: 3))
                    }
                }
                estado.font(.caption).foregroundStyle(.secondary).lineLimit(2)
            }
            Spacer()
            Button("Sincronizar", action: sincronizar)
                .disabled(ocupado || fila.direccion == nil)
        }
    }

    @ViewBuilder private var estado: some View {
        switch fila.estado {
        case .mirando: Text("Mirando si contesta…")
        case .contesta: Text("Contesta" + (fila.direccion.map { " (\($0.host))" } ?? ""))
        case .noContesta: Text("No contesta")
        case .sincronizando: HStack(spacing: 4) { ProgressView().controlSize(.mini); Text("Sincronizando…") }
        case let .hecho(texto): Text(texto)
        case let .fallo(texto): Text(texto).foregroundStyle(.red)
        }
    }
}
