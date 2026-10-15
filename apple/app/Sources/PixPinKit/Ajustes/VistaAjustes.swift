// La ventana de Ajustes (⌘,), como la de Windows pero con lo que tiene
// sentido en un Mac: aspecto, atajo de capturar, galeria, carpeta de datos,
// abrir al iniciar sesion y sincronizar.

import AppKit
import SwiftUI

public struct VistaAjustes: View {
    @ObservedObject var modelo: ModeloAjustes
    @ObservedObject var atajo: AtajoDeCaptura

    public init(modelo: ModeloAjustes, atajo: AtajoDeCaptura) {
        self.modelo = modelo
        self.atajo = atajo
    }

    public var body: some View {
        Form {
            Section("General") {
                Picker("Aspecto", selection: $modelo.tema) {
                    ForEach(Tema.allCases) { t in Text(t.nombre).tag(t) }
                }
                Toggle("Abrir al iniciar sesión", isOn: Binding(
                    get: { modelo.abrirAlIniciar },
                    set: { modelo.fijar(abrirAlIniciar: $0) }
                ))
                if modelo.esperaAprobacion {
                    HStack {
                        Text("Falta permitirlo en Ajustes del Sistema.")
                            .font(.caption).foregroundStyle(.secondary)
                        Button("Abrir Elementos de inicio") { modelo.abrirElementosDeInicio() }
                            .controlSize(.small)
                    }
                }
            }
            Section("Capturar") {
                LabeledContent("Atajo para capturar una zona") {
                    GrabadorAtajo(atajo: atajo)
                }
                Stepper(value: Binding(
                    get: { modelo.diasCaducidad },
                    set: { modelo.fijar(diasCaducidad: $0) }
                ), in: 0 ... 400) {
                    LabeledContent("Las capturas de la galería se van a los",
                                   value: modelo.diasCaducidad == 0 ? "nunca" : "\(modelo.diasCaducidad) días")
                }
                Text("0 = no se va ninguna. Se guarda en pixpinmax.toml, igual que en Windows.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            SeccionGestosYPila()
            Section("Sincronizar") {
                Toggle("Sincronizar sola con los aparatos del grupo", isOn: Binding(
                    get: { modelo.sincroAutomatica },
                    set: { modelo.fijar(sincroAutomatica: $0) }
                ))
                Text("Al aparecer un aparato del grupo en la Wi‑Fi y 30 s después de un cambio. Se pausa en modo de bajo consumo.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            SeccionAjustesPdf(nucleo: modelo.nucleo)
            Section("Datos") {
                LabeledContent("Carpeta de datos") {
                    Text(modelo.carpetaDatos.path)
                        .textSelection(.enabled)
                        .lineLimit(2)
                        .truncationMode(.middle)
                        .foregroundStyle(.secondary)
                }
                HStack {
                    Spacer()
                    Button("Mostrar en Finder") { modelo.mostrarCarpetaEnFinder() }
                }
            }
            if let error = modelo.error {
                Section {
                    Label(error, systemImage: "exclamationmark.triangle")
                        .foregroundStyle(.red)
                }
            }
        }
        .formStyle(.grouped)
        .frame(width: 520)
        .frame(minHeight: 440)
        .task { if !modelo.cargado { await modelo.cargar() } }
    }
}

/// Ensena el atajo y graba uno nuevo: se pulsa «Cambiar» y luego la
/// combinacion (Esc cancela). Solo escucha mientras graba, y solo en esta
/// app (`addLocalMonitorForEvents`: no pide el permiso de Accesibilidad).
struct GrabadorAtajo: View {
    @ObservedObject var atajo: AtajoDeCaptura
    @State private var grabando = false
    @State private var monitor: Any?
    @State private var aviso: String?

    var body: some View {
        VStack(alignment: .trailing, spacing: 4) {
            HStack(spacing: 8) {
                Text(grabando ? "Pulsa la combinación…" : atajo.combinacion.descripcion)
                    .font(.body.monospaced())
                    .padding(.horizontal, 8).padding(.vertical, 3)
                    .background(RoundedRectangle(cornerRadius: 5)
                        .strokeBorder(grabando ? Color.accentColor : Color.secondary.opacity(0.4)))
                Button(grabando ? "Cancelar" : "Cambiar") {
                    if grabando { parar() } else { empezar() }
                }
                Button("Restablecer") {
                    parar()
                    aviso = atajo.restablecer() ? nil : "Ctrl+Option+X lo tiene otra app."
                }
                .disabled(atajo.combinacion == .capturar)
            }
            if let aviso {
                Text(aviso).font(.caption).foregroundStyle(.red)
            } else if !atajo.registrado {
                Text("El sistema no dejó registrar el atajo.").font(.caption).foregroundStyle(.red)
            }
        }
        .onDisappear { parar() }
    }

    private func empezar() {
        aviso = nil
        grabando = true
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { evento in
            // Solo valores (no el `NSEvent`) cruzan al actor principal.
            let codigo = evento.keyCode
            let flags = evento.modifierFlags
            let comido = MainActor.assumeIsolated { () -> Bool in
                manejar(codigo: codigo, flags: flags)
            }
            return comido ? nil : evento
        }
    }

    /// Lo que hace una tecla mientras se graba. `true`: se la come.
    private func manejar(codigo: UInt16, flags: NSEvent.ModifierFlags) -> Bool {
        guard grabando else { return false }
        if codigo == 53 { // Esc
            parar()
            return true
        }
        let nueva = AtajoGlobal.Combinacion(
            tecla: UInt32(codigo),
            modificadores: AtajoGlobal.Combinacion.modificadores(de: flags)
        )
        guard AtajoDeCaptura.vale(nueva) else {
            aviso = "Usa Control, Option o Comando con la tecla."
            return true
        }
        if !atajo.cambiar(a: nueva) {
            aviso = "\(nueva.descripcion) lo tiene otra app o el sistema."
        }
        parar()
        return true
    }

    private func parar() {
        grabando = false
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }
}
