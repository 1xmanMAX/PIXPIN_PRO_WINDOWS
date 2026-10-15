// Los documentos por el puente de verdad: la tinta de una hoja de PDF se
// guarda con los nombres y el marco de Windows y Android, una nota y una
// tabla van y vuelven, aligerar un PDF pequeno lo deja menor; y las cuentas
// del lector, el bufer de una pagina de Office y el formato de las notas.

import AppKit
import Foundation
import PDFKit
import PixPinCore
@testable import PixPinKit
import XCTest

/// Un PDF de una hoja con una foto de 200x200 SIN comprimir: hay mucho que
/// ganar sin perder nada (escrito a mano, con su tabla `xref`).
func pdfConFotoSinComprimir() -> Data {
    let w = 200, h = 200
    var pixeles = Data(count: w * h * 3)
    pixeles.withUnsafeMutableBytes { (b: UnsafeMutableRawBufferPointer) in
        for y in 0..<h {
            for x in 0..<w {
                let i = (y * w + x) * 3
                b[i] = UInt8(x % 256)
                b[i + 1] = UInt8(y % 256)
                b[i + 2] = 128
            }
        }
    }
    var out = Data()
    var marcas: [Int] = []
    func poner(_ s: String) { out.append(s.data(using: .ascii)!) }
    poner("%PDF-1.4\n")
    marcas.append(out.count)
    poner("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n")
    marcas.append(out.count)
    poner("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n")
    marcas.append(out.count)
    poner("3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /Contents 4 0 R "
        + "/Resources << /XObject << /Im1 5 0 R >> >> >>\nendobj\n")
    let contenido = "q 200 0 0 200 50 100 cm /Im1 Do Q"
    marcas.append(out.count)
    poner("4 0 obj\n<< /Length \(contenido.utf8.count) >>\nstream\n\(contenido)\nendstream\nendobj\n")
    marcas.append(out.count)
    poner("5 0 obj\n<< /Type /XObject /Subtype /Image /Width \(w) /Height \(h) /ColorSpace /DeviceRGB "
        + "/BitsPerComponent 8 /Length \(pixeles.count) >>\nstream\n")
    out.append(pixeles)
    poner("\nendstream\nendobj\n")
    let xref = out.count
    poner("xref\n0 6\n0000000000 65535 f \n")
    for m in marcas { poner(String(format: "%010ld 00000 n \n", m)) }
    poner("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n\(xref)\n%%EOF\n")
    return out
}

/// Un trazo a mano en la hoja.
private func trazar(_ ed: EditorLienzo, desde: CGPoint, hasta: CGPoint) {
    try? ed.herramienta(nombre: "lapiz")
    _ = ed.puntero(fase: .pulsar, x: Float(desde.x), y: Float(desde.y), presion: nil, modificadores: 0, zoom: 1)
    for k in 1...10 {
        let t = CGFloat(k) / 10
        _ = ed.puntero(fase: .mover, x: Float(desde.x + (hasta.x - desde.x) * t),
                       y: Float(desde.y + (hasta.y - desde.y) * t), presion: nil, modificadores: 0, zoom: 1)
    }
    _ = ed.puntero(fase: .soltar, x: Float(hasta.x), y: Float(hasta.y), presion: nil, modificadores: 0, zoom: 1)
}

/// Los ficheros de debajo de `carpeta` cuyo nombre cumple `cumple`.
private func buscar(en carpeta: URL, _ cumple: (String) -> Bool) -> [URL] {
    let e = FileManager.default.enumerator(at: carpeta, includingPropertiesForKeys: nil)
    var v: [URL] = []
    while let u = e?.nextObject() as? URL {
        if cumple(u.lastPathComponent) { v.append(u) }
    }
    return v
}

final class DocumentosTests: XCTestCase {
    // MARK: Por el puente

    func testLaTintaDeUnaHojaDelChatVaYVuelveConElFormatoDeWindows() throws {
        let raiz = try carpetaTemporal("tinta")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "Mac")
        let chat = try nucleo.crearChat(nombre: "Tesis")
        let origen = raiz.appendingPathComponent("informe.pdf")
        try pdfConFotoSinComprimir().write(to: origen)
        let m = try nucleo.adjuntar(chat: chat.id, rutaOrigen: origen.path, nombre: nil)
        let doc = try nucleo.rutaAbsoluta(chat: chat.id, rutaRelativa: XCTUnwrap(m.ruta))
        XCTAssertEqual(nucleo.dondeAnota(documento: doc), .chat)
        XCTAssertTrue(nucleo.hojasConTinta(documento: doc, paginas: 1).isEmpty)

        // La pagina mide 300 x 400 puntos: la hoja, 1400 x 1866,67.
        let alto = GeometriaHoja.altoDeHoja(ancho: 300, alto: 400)
        XCTAssertEqual(alto, 1866.6667, accuracy: 0.01)
        let ed = try nucleo.anotarHoja(documento: doc, pagina: 0, alto: alto)
        XCTAssertTrue(ed.esDeDocumento())
        XCTAssertFalse(try ed.guardar(), "abrir no escribe nada")
        ed.tintaColor(r: 0.88, g: 0.19, b: 0.19, a: 1)
        trazar(ed, desde: CGPoint(x: 100, y: 200), hasta: CGPoint(x: 600, y: 260))
        XCTAssertTrue(try ed.guardar())

        // Los nombres del movil y de Windows: `anot-<uid>-p0` y su marco.
        let carpetaChat = raiz.appendingPathComponent("proyectos").appendingPathComponent(chat.id)
        let tintas = buscar(en: carpetaChat) { $0.hasPrefix("anot-") && $0.hasSuffix("-p0.excalidraw") }
        XCTAssertEqual(tintas.count, 1, "\(tintas)")
        let marcos = buscar(en: carpetaChat) { $0.hasPrefix("anot-") && $0.hasSuffix("-p0.hoja") }
        XCTAssertEqual(marcos.count, 1)
        XCTAssertEqual(try String(contentsOf: marcos[0], encoding: .utf8), "0,0,1400,1866.667\nv1\n")
        let json = try String(contentsOf: tintas[0], encoding: .utf8)
        XCTAssertTrue(json.contains("\"freedraw\""))
        XCTAssertEqual(nucleo.hojasConTinta(documento: doc, paginas: 1), [0])

        // Vuelve a abrirse en el mismo sitio.
        let otra = try nucleo.anotarHoja(documento: doc, pagina: 0, alto: alto)
        XCTAssertEqual(otra.cuantosElementos(), 1)
        let caja = try XCTUnwrap(otra.caja())
        XCTAssertEqual(caja.x0, 100, accuracy: 4)
        XCTAssertEqual(caja.y0, 200, accuracy: 4)

        // Marcadores y espacios: viajan en `anot-<uid>.marcas` y `.espacios`.
        var lectura = nucleo.lecturaDeDocumento(documento: doc)
        lectura.espacios = 3
        lectura.marcas = conMarca([], x: 0.5, y: 0.25, emoji: "⭐", ahora: 1700)
        try nucleo.guardarLecturaDeDocumento(documento: doc, lectura: lectura, altos: [alto])
        let marcas = buscar(en: carpetaChat) { $0.hasPrefix("anot-") && $0.hasSuffix(".marcas") }
        XCTAssertEqual(try String(contentsOf: XCTUnwrap(marcas.first), encoding: .utf8), "1700:0.5:0.25:⭐")
        let releida = nucleo.lecturaDeDocumento(documento: doc)
        XCTAssertEqual(releida.espacios, 3)
        XCTAssertEqual(releida.marcas.map(\.emoji), ["⭐"])
    }

    @MainActor
    func testUnaNotaSeEditaYVuelveAlCuaderno() throws {
        let raiz = try carpetaTemporal("nota")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "Mac")
        let chat = try nucleo.crearChat(nombre: "Notas")
        let m = try nucleo.escribirNota(chat: chat.id, texto: "# Compra\n- [ ] pan", respondeA: nil)
        let editada = try nucleo.editarNota(chat: chat.id, mensaje: m.id, texto: "# Compra\n- [x] pan\n$x^2$")
        XCTAssertEqual(editada.texto, "# Compra\n- [x] pan\n$x^2$")
        let ultimos = try nucleo.ultimosMensajes(chat: chat.id, cuantos: 5)
        XCTAssertEqual(ultimos.mensajes.last?.texto, "# Compra\n- [x] pan\n$x^2$")
        XCTAssertThrowsError(try nucleo.editarNota(chat: chat.id, mensaje: "no-esta", texto: "x"))

        // El formato encima del texto: el titulo grande y su `#` escondido
        // fuera del renglon del cursor; el exponente de la formula, subido.
        let s = NSTextStorage(string: editada.texto)
        EstiloDeNota.aplicar(s, tramos: mdTramos(texto: s.string), lineaDelCursor: 1)
        let titulo = s.attribute(.font, at: 3, effectiveRange: nil) as? NSFont
        XCTAssertGreaterThan(titulo?.pointSize ?? 0, EstiloDeNota.tamBase)
        let almohadilla = s.attribute(.font, at: 0, effectiveRange: nil) as? NSFont
        XCTAssertLessThan(almohadilla?.pointSize ?? 99, 1)
        let dos = (s.string as NSString).range(of: "2")
        let subida = s.attribute(.baselineOffset, at: dos.location, effectiveRange: nil) as? CGFloat
        XCTAssertGreaterThan(subida ?? 0, 0)
        XCTAssertEqual(EstiloDeNota.linea(de: 10, en: s.string), 1)
    }

    func testUnaTablaSeEditaCeldaACeldaYSeGuarda() throws {
        let raiz = try carpetaTemporal("tabla")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "Mac")
        let chat = try nucleo.crearChat(nombre: "Cuentas")
        let m = try nucleo.crearTabla(chat: chat.id, nombre: "Gastos")
        XCTAssertEqual(m.clase, "MINIAPP")
        XCTAssertTrue(BurbujaTabla.esTabla(m))
        XCTAssertTrue(nucleo.esHojaDeCalculo(chat: chat.id, mensaje: m.id))
        let hoja = try nucleo.abrirHojaDeCalculo(chat: chat.id, mensaje: m.id)
        XCTAssertTrue(hoja.poner(fila: 0, columna: 0, contenido: "2"))
        XCTAssertTrue(hoja.poner(fila: 1, columna: 0, contenido: "=A1*3"))
        XCTAssertTrue(hoja.ponerEstilo(fila: 1, columna: 0, estilo: EstiloCeldaHoja(
            negrita: true, cursiva: false, subrayado: false, tachado: false, alineacion: nil, fondo: "#fff3bf",
            color: nil, ajustar: false, formatoNumero: "porcentaje")))
        let celdas = hoja.celdas(desdeFila: 0, hastaFila: 10, desdeColumna: 0, hastaColumna: 5)
        let b = try XCTUnwrap(celdas.first { $0.fila == 1 })
        XCTAssertEqual(b.numero, 6)
        XCTAssertEqual(textoDeCelda(b, locale: Locale(identifier: "en_US")), "600.00%")
        XCTAssertTrue(try hoja.guardar())
        XCTAssertFalse(try hoja.guardar(), "sin cambios no se escribe")

        let otra = try nucleo.abrirHojaDeCalculo(chat: chat.id, mensaje: m.id)
        XCTAssertEqual(otra.contenido(fila: 1, columna: 0), "=A1*3")
        XCTAssertEqual(otra.estilo(fila: 1, columna: 0)?.negrita, true)
        // En el cuaderno, el JSON de `TablaDeCalculo` en el texto del mensaje.
        let ultimo = try XCTUnwrap(nucleo.ultimosMensajes(chat: chat.id, cuantos: 5).mensajes.last)
        XCTAssertTrue(ultimo.texto.contains("\"A2\": \"=A1*3\""), ultimo.texto)
        XCTAssertTrue(ultimo.texto.contains("\"nf\": \"porcentaje\""))
    }

    func testAligerarUnPdfPequenoLoDejaMenorYSeSigueAbriendo() throws {
        let carpeta = try carpetaTemporal("aligerar")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let origen = carpeta.appendingPathComponent("pesado.pdf")
        let destino = carpeta.appendingPathComponent("ligero.pdf")
        try pdfConFotoSinComprimir().write(to: origen)
        XCTAssertEqual(PDFDocument(url: origen)?.pageCount, 1, "el PDF escrito a mano se abre")
        let r = try aligerarPdf(origen: origen.path, destino: destino.path, nivel: .sinPerdida)
        XCTAssertTrue(r.escrito)
        XCTAssertLessThan(r.despues, r.antes)
        XCTAssertEqual(r.paginas, 1)
        XCTAssertEqual(PDFDocument(url: destino)?.pageCount, 1)
        XCTAssertEqual(nivelParaEntrar(elegido: .extremo), .equilibrado)
        XCTAssertTrue(textoDeAligerado(.aligerado(antes: 1000, despues: 400)).contains("60"))
        // Caso negativo: lo que no es un PDF no se aligera.
        let falso = carpeta.appendingPathComponent("falso.pdf")
        try Data("hola".utf8).write(to: falso)
        XCTAssertThrowsError(try aligerarPdf(origen: falso.path, destino: destino.path, nivel: .equilibrado))
    }

    // MARK: Sin puente

    func testLasCuentasDeLaHojaVanYVuelven() {
        let g = GeometriaHoja(rect: CGRect(x: 100, y: 50, width: 700, height: 990))
        XCTAssertEqual(g.escala, 0.5, accuracy: 1e-9)
        XCTAssertEqual(g.alto, 1980, accuracy: 1e-6)
        let p = CGPoint(x: 333, y: 444)
        let ida = g.aVista(g.aHoja(p))
        XCTAssertEqual(ida.x, p.x, accuracy: 1e-9)
        XCTAssertEqual(ida.y, p.y, accuracy: 1e-9)
        XCTAssertEqual(g.aHoja(CGPoint(x: 100, y: 50)), .zero)
        XCTAssertEqual(g.aHoja(CGPoint(x: 800, y: 50)).x, 1400, accuracy: 1e-9)
        let (izq, der) = g.espacios(1)
        XCTAssertEqual(izq?.width ?? 0, 525, accuracy: 1e-9)
        XCTAssertNil(der)
        // Un marcador de la pagina 2 cae en su hoja y en ninguna otra.
        let m = MarcaDocumento(id: 1, x: 0.5, y: 2.25, emoji: "🔖")
        let punto = g.punto(de: m, pagina: 2)
        XCTAssertEqual(punto?.x ?? 0, 450, accuracy: 1e-9)
        XCTAssertEqual(punto?.y ?? 0, 50 + 990 * 0.25, accuracy: 1e-9)
        XCTAssertNil(g.punto(de: m, pagina: 1))
        let sitio = g.sitio(de: CGPoint(x: 450, y: 297.5), pagina: 2)
        XCTAssertEqual(sitio.x, 0.5, accuracy: 1e-9)
        XCTAssertEqual(sitio.y, 2.25, accuracy: 1e-9)
        // Caben 24: el mas viejo se va.
        var lista: [MarcaDocumento] = []
        for i in 0..<25 { lista = conMarca(lista, x: 0, y: Double(i), emoji: "⭐", ahora: Int64(i)) }
        XCTAssertEqual(lista.count, 24)
        XCTAssertFalse(lista.contains { $0.id == 0 })
    }

    func testQueSeHaceConCadaFichero() {
        XCTAssertEqual(TiposDeDocumento.clase(deNombre: "a.PDF"), .pdf)
        XCTAssertEqual(TiposDeDocumento.clase(deNombre: "acta.docx"), .officePaginado)
        XCTAssertEqual(TiposDeDocumento.clase(deNombre: "deck.pptx"), .officePaginado)
        XCTAssertEqual(TiposDeDocumento.clase(deNombre: "cuentas.xlsx"), .vistaRapida)
        XCTAssertEqual(TiposDeDocumento.clase(deNombre: "libro.epub"), .vistaRapida)
        XCTAssertEqual(TiposDeDocumento.clase(deNombre: "datos.csv"), .vistaRapida)
        XCTAssertEqual(TiposDeDocumento.clase(deNombre: "foto.png"), .otro)
        XCTAssertTrue(esDocumentoOffice(nombre: "acta.docx"))
        XCTAssertEqual(letrasDeColumna(0), "A")
        XCTAssertEqual(letrasDeColumna(25), "Z")
        XCTAssertEqual(letrasDeColumna(26), "AA")
        XCTAssertEqual(letrasDeColumna(701), "ZZ")
    }

    func testUnaPaginaDeOfficeSeLeeYSePintaEnUnPdf() throws {
        // version, un rectangulo con relleno, un grupo con un texto dentro.
        var f: [Float] = [1]
        f += [2, 10, 10, 100, 50, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]
        f += [6, 1, 0, 0, 1, 20, 30, 0, 0, 0, 0, 0]
        f += [1, 5, 40, 0, 0, 12, 0, 0, 0, 1, 1, 0, 0, 0, 40]
        f += [7]
        let datos = f.withUnsafeBufferPointer { Data(buffer: $0) }
        let pagina = PaginaOffice(indice: 0, ancho: 200, alto: 300, fondo: [1, 1, 1, 1], datos: datos,
                                  textos: ["Hola"],
                                  letras: [LetraOffice(familia: "Helvetica", negrita: false, cursiva: false)],
                                  imagenes: [], enlaces: [], textoPlano: "Hola")
        let ordenes = try leerPaginaOffice(pagina)
        XCTAssertEqual(ordenes.count, 4)
        XCTAssertEqual(ordenes[0], .rect(CGRect(x: 10, y: 10, width: 100, height: 50), relleno: nil, borde: nil,
                                         grosor: 0, discontinuo: 0))
        XCTAssertEqual(ordenes[3], .finDeGrupo)
        let pdf = try XCTUnwrap(PintorOffice(pagina: pagina).pdf())
        let doc = try XCTUnwrap(PDFDocument(data: pdf))
        XCTAssertEqual(doc.pageCount, 1)
        XCTAssertEqual(doc.page(at: 0)?.bounds(for: .mediaBox).width ?? 0, 200, accuracy: 0.5)
        XCTAssertTrue(doc.string?.contains("Hola") ?? false, doc.string ?? "")
        // Caso negativo: un bufer de otra version no se lee.
        var otra = pagina
        otra.datos = [Float(9)].withUnsafeBufferPointer { Data(buffer: $0) }
        XCTAssertThrowsError(try leerPaginaOffice(otra))
        // El medidor de Core Text mide lo mismo con que se pinta.
        let medidor = MedidorCoreText()
        let a = medidor.avances(caracteres: "ii", familia: "Helvetica", negrita: false, cursiva: false)
        XCTAssertEqual(a.count, 2)
        XCTAssertGreaterThan(a[0], 0)
        XCTAssertGreaterThan(medidor.metricas(familia: "Helvetica", negrita: false, cursiva: false).ascenso, 0)
    }
}
