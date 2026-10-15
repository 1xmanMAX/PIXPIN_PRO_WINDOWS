// «Al chat» en un paso (`captura2/al_chat.rs` de Windows): un panelito con
// buscador y los proyectos (el ultimo al que se mando una captura, el
// primero; luego los de la lista en su orden). Las teclas 1-6 eligen uno de
// los seis primeros, Intro el primero de lo filtrado y Esc cancela.
//
// La captura entra como una imagen del chat llamada `captura-<ms>.png`,
// como la deja Windows, con `Almacen.adjuntar` (la misma copia que soltar
// un fichero en el chat).

import AppKit
import PixPinCore
import SwiftUI

/// Ordenar y filtrar los proyectos, sin pantalla (lo prueban las pruebas).
public enum ListaDeProyectos {
    public static let clave = "captura.ultimoChat"
    /// Cuantos tienen tecla (1-6).
    public static let conTecla = 6

    /// El ultimo usado primero y lo demas en su orden (`ordenar` de
    /// Windows). Uno que ya no existe no cambia nada.
    public static func ordenar<T>(_ v: [T], id: (T) -> String, ultimo: String?) -> [T] {
        guard let ultimo, let i = v.firstIndex(where: { id($0) == ultimo }) else { return v }
        var o = v
        let p = o.remove(at: i)
        o.insert(p, at: 0)
        return o
    }

    /// Los que contienen `texto` en el nombre, sin mirar mayusculas ni
    /// tildes. Vacio: todos.
    public static func filtrar<T>(_ v: [T], nombre: (T) -> String, texto: String) -> [T] {
        let t = texto.trimmingCharacters(in: .whitespaces)
        guard !t.isEmpty else { return v }
        return v.filter { nombre($0).range(of: t, options: [.caseInsensitive, .diacriticInsensitive]) != nil }
    }

    /// El nombre del fichero de la captura en el chat: `captura-<ms>.png`.
    public static func nombreDeFichero(fecha: Date) -> String {
        "captura-\(Int64((fecha.timeIntervalSince1970 * 1000).rounded(.down))).png"
    }
}

/// El panel que elige el proyecto y manda la captura.
@MainActor
public final class SelectorProyecto {
    private var panel: NSPanel?

    public init() {}

    /// Ensena el selector; al elegir, adjunta `imagen` a ese chat.
    public func elegir(para imagen: CGImage, almacen: Almacen) {
        cerrar()
        Task {
            await almacen.cargarChats()
            let ultimo = UserDefaults.standard.string(forKey: ListaDeProyectos.clave)
            let fichas = ListaDeProyectos.ordenar(almacen.chats.filter { !$0.esAsistente }, id: \.id, ultimo: ultimo)
            guard !fichas.isEmpty else {
                NSSound.beep()
                return
            }
            let vista = VistaSelectorProyecto(fichas: fichas) { [weak self] elegida in
                self?.cerrar()
                guard let elegida else { return }
                UserDefaults.standard.set(elegida.id, forKey: ListaDeProyectos.clave)
                Task { await SelectorProyecto.mandar(imagen, a: elegida.id, almacen: almacen) }
            }
            let host = NSHostingView(rootView: vista)
            let tam = NSSize(width: 360, height: 380)
            host.frame = NSRect(origin: .zero, size: tam)
            let panel = PanelConTeclado(contentRect: NSRect(origin: .zero, size: tam),
                                        styleMask: [.titled, .closable, .fullSizeContentView, .nonactivatingPanel],
                                        backing: .buffered, defer: false)
            panel.title = "Al chat"
            panel.titlebarAppearsTransparent = true
            panel.isFloatingPanel = true
            panel.level = .floating
            panel.isReleasedWhenClosed = false
            panel.hidesOnDeactivate = false
            panel.contentView = host
            let raton = NSEvent.mouseLocation
            let pantalla = NSScreen.screens.first { $0.frame.contains(raton) } ?? NSScreen.main
            if let z = pantalla?.visibleFrame {
                panel.setFrameOrigin(CGPoint(x: min(max(raton.x - tam.width / 2, z.minX), z.maxX - tam.width),
                                             y: min(max(raton.y - tam.height / 2, z.minY), z.maxY - tam.height)))
            }
            NSApp.activate()
            panel.makeKeyAndOrderFront(nil)
            self.panel = panel
        }
    }

    func cerrar() {
        panel?.orderOut(nil)
        panel?.close()
        panel = nil
    }

    /// La captura al chat como `captura-<ms>.png` (PNG fuera del hilo
    /// principal; la copia al proyecto la hace `Almacen.adjuntar`).
    static func mandar(_ imagen: CGImage, a chat: String, almacen: Almacen) async {
        let nombre = ListaDeProyectos.nombreDeFichero(fecha: Date())
        let fichero = await Task.detached(priority: .userInitiated) { () -> URL? in
            guard let png = Capturador.codificar(imagen, tipo: .png, escala: 1) else { return nil }
            let carpeta = FileManager.default.temporaryDirectory
                .appendingPathComponent("pixpin-alchat-\(UUID().uuidString)", isDirectory: true)
            do {
                try FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
                let f = carpeta.appendingPathComponent(nombre)
                try png.write(to: f)
                return f
            } catch {
                return nil
            }
        }.value
        guard let fichero else { return NSSound.beep() }
        let hechos = await almacen.adjuntar(urls: [fichero], en: chat)
        try? FileManager.default.removeItem(at: fichero.deletingLastPathComponent())
        if hechos.isEmpty { NSSound.beep() }
    }
}

struct VistaSelectorProyecto: View {
    let fichas: [FichaChat]
    let alElegir: (FichaChat?) -> Void
    @State private var texto = ""
    @FocusState private var enBuscador: Bool

    private var filtradas: [FichaChat] {
        ListaDeProyectos.filtrar(fichas, nombre: \.nombre, texto: texto)
    }

    var body: some View {
        VStack(spacing: 8) {
            TextField("Buscar proyecto", text: $texto)
                .textFieldStyle(.roundedBorder)
                .focused($enBuscador)
                .onSubmit { alElegir(filtradas.first) }
            List {
                ForEach(Array(filtradas.prefix(60).enumerated()), id: \.element.id) { i, f in
                    Button {
                        alElegir(f)
                    } label: {
                        HStack {
                            if i < ListaDeProyectos.conTecla {
                                Text("\(i + 1)")
                                    .font(.caption.monospacedDigit().bold())
                                    .frame(width: 18, height: 18)
                                    .background(RoundedRectangle(cornerRadius: 4).fill(Color.secondary.opacity(0.2)))
                            } else {
                                Spacer().frame(width: 18)
                            }
                            Text(f.esGuardados ? "Mensajes guardados" : f.nombre).lineLimit(1)
                            Spacer()
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
            .listStyle(.plain)
            Text("1–6 elige · Intro el primero · Esc cancela")
                .font(.caption).foregroundStyle(.secondary)
        }
        .padding(12)
        .padding(.top, 16)
        .onAppear { enBuscador = true }
        .onExitCommand { alElegir(nil) }
        .onChange(of: texto) { antes, ahora in
            // Con el buscador vacio, una cifra 1-6 elige (no se busca).
            guard antes.isEmpty, ahora.count == 1, let n = Int(ahora), n >= 1,
                  n <= ListaDeProyectos.conTecla, n <= fichas.count else { return }
            alElegir(fichas[n - 1])
        }
    }
}
