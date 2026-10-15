// El panel del asistente: burbujas, la caja de escribir y que motor
// contesta. Lo dibujado se puede guardar en «Mensajes guardados».

import PixPinCore
import SwiftUI

public struct VentanaAsistente: View {
    @StateObject private var modelo: ModeloAsistente
    @State private var texto = ""
    @State private var guardado: UUID?

    public init(almacen: Almacen?) {
        _modelo = StateObject(wrappedValue: ModeloAsistente(almacen: almacen))
    }

    public var body: some View {
        VStack(spacing: 0) {
            cabecera
            Divider()
            ScrollViewReader { lector in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 10) {
                        if modelo.burbujas.isEmpty {
                            Text(modelo.motor == .ninguno ? ModeloAsistente.textoSinMotor
                                : "Pregunta algo o pide un dibujo («dibuja una casa con un árbol»).")
                                .foregroundStyle(.secondary)
                                .padding()
                        }
                        ForEach(modelo.burbujas) { b in
                            BurbujaAsistente(burbuja: b, guardada: guardado == b.id) {
                                Task {
                                    if await modelo.guardarDibujo(b) { guardado = b.id }
                                }
                            }
                            .id(b.id)
                        }
                        if modelo.ocupado {
                            HStack {
                                ProgressView().controlSize(.small)
                                if modelo.sePuedeParar {
                                    Button("Parar") { modelo.parar() }
                                        .controlSize(.small)
                                        .keyboardShortcut(".", modifiers: .command)
                                }
                            }
                            .padding(.leading, 8).id("pensando")
                        }
                    }
                    .padding(12)
                }
                .onChange(of: modelo.burbujas.count) { _, _ in
                    if let ultima = modelo.burbujas.last?.id {
                        withAnimation { lector.scrollTo(ultima, anchor: .bottom) }
                    }
                }
                // La respuesta en vivo crece: se sigue su final (sin animar).
                .onChange(of: modelo.burbujas.last?.texto.count) { _, _ in
                    if modelo.ocupado, let ultima = modelo.burbujas.last?.id {
                        lector.scrollTo(ultima, anchor: .bottom)
                    }
                }
            }
            Divider()
            HStack(alignment: .bottom) {
                TextField("Escribe al asistente…", text: $texto, axis: .vertical)
                    .textFieldStyle(.plain)
                    .lineLimit(1 ... 6)
                    .onSubmit(enviar)
                Button(action: enviar) { Image(systemName: "arrow.up.circle.fill").font(.title2) }
                    .buttonStyle(.borderless)
                    .disabled(texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || modelo.ocupado)
                    .keyboardShortcut(.return, modifiers: .command)
            }
            .padding(10)
        }
        .frame(minWidth: 380, minHeight: 420)
        .task { await modelo.comprobarMotor() }
    }

    private var cabecera: some View {
        HStack {
            Image(systemName: modelo.motor == .ninguno ? "exclamationmark.triangle" : "sparkles")
                .foregroundStyle(modelo.motor == .ninguno ? Color.orange : Color.accentColor)
            Text(modelo.motor?.nombre ?? "Buscando motor…")
                .font(.caption).foregroundStyle(.secondary)
            Spacer()
            Button {
                Task { await modelo.comprobarMotor() }
            } label: { Image(systemName: "arrow.clockwise") }
                .buttonStyle(.borderless)
                .help("Volver a buscar un motor")
            Button("Nueva") { modelo.nuevaConversacion() }
                .disabled(modelo.burbujas.isEmpty || modelo.ocupado)
        }
        .padding(8)
    }

    private func enviar() {
        let t = texto
        guard !t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, !modelo.ocupado else { return }
        texto = ""
        Task { await modelo.enviar(t) }
    }
}

struct BurbujaAsistente: View {
    let burbuja: ModeloAsistente.Burbuja
    let guardada: Bool
    let alGuardar: () -> Void

    var body: some View {
        HStack {
            if burbuja.delUsuario { Spacer(minLength: 40) }
            VStack(alignment: .leading, spacing: 6) {
                Text(burbuja.texto.isEmpty ? "…" : burbuja.texto).textSelection(.enabled)
                ForEach(Array(burbuja.herramientas.enumerated()), id: \.offset) { _, h in
                    Label(h.nombre + ": " + h.resumen, systemImage: h.ok ? "checkmark.circle" : "xmark.circle")
                        .font(.caption)
                        .foregroundStyle(h.ok ? Color.secondary : Color.orange)
                }
                if burbuja.lienzo != nil {
                    let rotulo: String = guardada ? "Guardado en Mensajes guardados"
                        : "Guardar el dibujo en Mensajes guardados"
                    Button(rotulo, action: alGuardar)
                        .controlSize(.small)
                        .disabled(guardada)
                }
            }
            .padding(10)
            .background(fondo, in: RoundedRectangle(cornerRadius: 10))
            if !burbuja.delUsuario { Spacer(minLength: 40) }
        }
    }

    private var fondo: Color {
        if burbuja.esAviso { return Color.orange.opacity(0.15) }
        return burbuja.delUsuario ? Color.accentColor.opacity(0.18) : Color.secondary.opacity(0.12)
    }
}
