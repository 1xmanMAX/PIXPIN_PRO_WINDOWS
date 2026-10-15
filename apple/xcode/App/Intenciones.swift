// App Intents: lo que PixPin ofrece a Siri, Atajos y Spotlight (el
// sustituto de Flow Launcher en Windows, §3 del plan). Solo en la app que
// monta Xcode (`apple/xcode/project.yml`): el extractor de metadatos de App
// Intents es del sistema de compilacion de Xcode, no de SwiftPM.
//
// Cada intencion llama a `AccionesApp` (PixPinKit), que trabaja con el
// mismo almacen y el mismo sincronizador que la app.

import AppIntents
import PixPinKit

// MARK: - Proyectos

/// Un proyecto (un chat), para elegirlo en Atajos.
struct ProyectoEntidad: AppEntity {
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Proyecto"
    static let defaultQuery = ConsultaProyectos()

    var id: String
    var nombre: String

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(nombre)")
    }
}

struct ConsultaProyectos: EntityStringQuery {
    func entities(for identifiers: [String]) async throws -> [ProyectoEntidad] {
        try await todos().filter { identifiers.contains($0.id) }
    }

    func entities(matching string: String) async throws -> [ProyectoEntidad] {
        let buscado = string.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !buscado.isEmpty else { return try await todos() }
        return try await todos().filter {
            $0.nombre.range(of: buscado, options: [.caseInsensitive, .diacriticInsensitive]) != nil
        }
    }

    func suggestedEntities() async throws -> [ProyectoEntidad] {
        try await todos()
    }

    private func todos() async throws -> [ProyectoEntidad] {
        let lista = try await AccionesApp.compartidas.proyectos()
        return lista.map { ProyectoEntidad(id: $0.id, nombre: $0.nombre) }
    }
}

// MARK: - Intenciones

struct CapturarZona: AppIntent {
    static let title: LocalizedStringResource = "Capturar zona"
    static let description = IntentDescription(
        "Elige una zona de la pantalla para copiarla, dejarla flotando o mandarla a un proyecto.")
    /// La captura necesita la app abierta (pinta la seleccion encima).
    static let openAppWhenRun: Bool = true

    @MainActor
    func perform() async throws -> some IntentResult {
        try AccionesApp.compartidas.capturarZona()
        return .result()
    }
}

struct NuevaNotaEnProyecto: AppIntent {
    static let title: LocalizedStringResource = "Nueva nota en proyecto"
    static let description = IntentDescription("Escribe una nota al final de un proyecto de PixPin.")

    @Parameter(title: "Proyecto")
    var proyecto: ProyectoEntidad

    @Parameter(title: "Nota")
    var texto: String

    static var parameterSummary: some ParameterSummary {
        Summary("Escribir \(\.$texto) en \(\.$proyecto)")
    }

    @MainActor
    func perform() async throws -> some IntentResult & ProvidesDialog {
        try await AccionesApp.compartidas.escribirNota(texto, en: proyecto.id)
        let frase = "Nota guardada en «\(proyecto.nombre)»."
        return .result(dialog: "\(frase)")
    }
}

struct BuscarEnPixPin: AppIntent {
    static let title: LocalizedStringResource = "Buscar en PixPin"
    static let description = IntentDescription("Busca en todos los proyectos, notas, tareas y momentos.")

    @Parameter(title: "Buscar")
    var consulta: String

    static var parameterSummary: some ParameterSummary {
        Summary("Buscar \(\.$consulta) en PixPin")
    }

    @MainActor
    func perform() async throws -> some IntentResult & ReturnsValue<String> & ProvidesDialog {
        let encontrados = try await AccionesApp.compartidas.buscar(consulta)
        let lineas = encontrados.map { $0.detalle.isEmpty ? $0.titulo : "\($0.titulo) — \($0.detalle)" }
        let texto = lineas.joined(separator: "\n")
        let frase = lineas.isEmpty
            ? "No encontré nada con «\(consulta)»."
            : "Encontré \(lineas.count) resultados con «\(consulta)»:\n\(texto)"
        return .result(value: texto, dialog: "\(frase)")
    }
}

struct SincronizarAhora: AppIntent {
    static let title: LocalizedStringResource = "Sincronizar ahora"
    static let description = IntentDescription("Sincroniza con los aparatos de tu grupo que estén en la Wi-Fi.")

    @MainActor
    func perform() async throws -> some IntentResult & ProvidesDialog {
        let resumen = try await AccionesApp.compartidas.sincronizarAhora()
        return .result(dialog: "\(resumen)")
    }
}

// MARK: - Frases para Siri y Spotlight

struct AtajosPixPin: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: CapturarZona(),
            phrases: [
                "Capturar zona con \(.applicationName)",
                "Hacer una captura con \(.applicationName)",
            ],
            shortTitle: "Capturar zona",
            systemImageName: "camera.viewfinder")
        AppShortcut(
            intent: NuevaNotaEnProyecto(),
            phrases: [
                "Nueva nota en \(.applicationName)",
                "Apuntar una nota en \(.applicationName)",
            ],
            shortTitle: "Nueva nota",
            systemImageName: "square.and.pencil")
        AppShortcut(
            intent: BuscarEnPixPin(),
            phrases: [
                "Buscar en \(.applicationName)",
            ],
            shortTitle: "Buscar",
            systemImageName: "magnifyingglass")
        AppShortcut(
            intent: SincronizarAhora(),
            phrases: [
                "Sincronizar \(.applicationName)",
                "Sincronizar \(.applicationName) ahora",
            ],
            shortTitle: "Sincronizar ahora",
            systemImageName: "arrow.triangle.2.circlepath")
    }
}
