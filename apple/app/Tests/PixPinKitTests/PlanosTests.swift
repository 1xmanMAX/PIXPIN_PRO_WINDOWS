// Los visores de planos y modelos 3D: la disposicion de los buferes de Rust
// en Swift, abrir un DXF y un fichero de puntos por el puente de verdad, la
// camara del plano, lo anotado y, si hay tarjeta, compilar los sombreadores
// y pintar el plano fuera de pantalla. Sin tarjeta (una maquina virtual de
// la CI) lo de Metal se salta.

import Foundation
import Metal
import PixPinCore
@testable import PixPinKit
import XCTest

/// El DXF de las pruebas de Rust (`planos.rs`): «Muros» con 0,50 mm (dos
/// rayas en L) y «Ejes» apagada (un circulo).
let dxfDePrueba = "0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n70\n2\n0\nLAYER\n2\nMuros\n70\n0\n62\n1\n6\nCONTINUOUS\n370\n50\n0\nLAYER\n2\nEjes\n70\n0\n62\n-5\n6\nCONTINUOUS\n0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nMuros\n10\n0\n20\n0\n30\n0\n11\n100\n21\n0\n31\n0\n0\nLINE\n8\nMuros\n10\n100\n20\n0\n30\n0\n11\n100\n21\n60\n31\n0\n0\nCIRCLE\n8\nEjes\n10\n50\n20\n30\n30\n0\n40\n20\n0\nENDSEC\n0\nEOF\n"

/// Cinco puntos PNEZD (Civil 3D): un terreno con sus curvas.
let puntosDePrueba = "1,100,200,10,A\n2,100,210,11,B\n3,110,200,12,C\n4,110,210,10,D\n5,105,205,15,E\n"

private func abrirPlanoDePrueba(_ carpeta: URL) throws -> (Nucleo, Plano, URL) {
    let ruta = carpeta.appendingPathComponent("planta.dxf")
    try dxfDePrueba.write(to: ruta, atomically: true, encoding: .utf8)
    let nucleo = try Nucleo.abrir(raiz: carpeta.appendingPathComponent("datos").path, nombreDelAparato: "Mac de prueba")
    return (nucleo, try nucleo.abrirPlano(ruta: ruta.path), ruta)
}

final class DisposicionCADTests: XCTestCase {
    func testLasEstructurasDeSwiftTienenElTamanoYLosDesplazamientosDeRust() {
        XCTAssertEqual(MemoryLayout<VerticeCAD>.stride, PasosCAD.vertice)
        XCTAssertEqual(MemoryLayout<VerticeCAD>.offset(of: \.color), 8)
        XCTAssertEqual(MemoryLayout<VerticeTramaCAD>.stride, PasosCAD.verticeTrama)
        XCTAssertEqual(MemoryLayout<VerticeTramaCAD>.offset(of: \.trama), 12)
        XCTAssertEqual(MemoryLayout<TramaCAD>.stride, PasosCAD.trama)
        XCTAssertEqual(MemoryLayout<TramaCAD>.offset(of: \.desde), 28)
        XCTAssertEqual(MemoryLayout<FamiliaCAD>.stride, PasosCAD.familia)
        XCTAssertEqual(MemoryLayout<FamiliaCAD>.offset(of: \.trazos), 32)
        XCTAssertEqual(MemoryLayout<LetraCAD>.stride, PasosCAD.letra)
        XCTAssertEqual(MemoryLayout<LetraCAD>.offset(of: \.glifo), 28)
        XCTAssertEqual(MemoryLayout<ArcoCAD>.stride, PasosCAD.arco)
        XCTAssertEqual(MemoryLayout<ArcoCAD>.offset(of: \.color), 20)
        XCTAssertEqual(MemoryLayout<TramoCAD>.stride, PasosCAD.tramo)
        XCTAssertEqual(MemoryLayout<TramoCAD>.offset(of: \.tamano), 24)
        XCTAssertEqual(MemoryLayout<V3CAD>.stride, PasosCAD.v3)
        XCTAssertEqual(MemoryLayout<V3CAD>.offset(of: \.normal), 12)
        XCTAssertEqual(MemoryLayout<V3CAD>.offset(of: \.elemento), 20)
    }

    func testLeerEstructurasNoNecesitaDatosAlineados() {
        var bytes = Data([0xAB]) // un byte delante: lo demas queda desalineado
        for x: UInt32 in [Float(1.5).bitPattern, Float(-2).bitPattern, 0xFF00_00FF] {
            withUnsafeBytes(of: x.littleEndian) { bytes.append(contentsOf: $0) }
        }
        let v = leerEstructuras(bytes.dropFirst(), como: VerticeCAD.self)
        XCTAssertEqual(v, [VerticeCAD(x: 1.5, y: -2, color: 0xFF00_00FF)])
        // Caso negativo: menos de una estructura, nada.
        XCTAssertTrue(leerEstructuras(Data([1, 2, 3]), como: VerticeCAD.self).isEmpty)
    }

    func testLosUniformesTienenElTamanoDeLosSombreadores() {
        let g = GrupoPlano(indice: 0, capa: "0", grosor: -3, encendido: true, origenX: 0, origenY: 0,
                           caja: CajaPlano(x0: 0, y0: 0, x1: 1, y1: 1), bytes: 0)
        let u = PintorPlano.vista(CamaraPlano(centroX: 0, centroY: 0, px: 1), grupo: g, ancho: 100, alto: 100, oscuro: true)
        XCTAssertEqual(u.bytes, 48)
        let c = Camara3d(objetivo: Punto3d(x: 0, y: 0, z: 0), rumbo: -0.8, altura: 0.5, dist: 10)
        let u3 = VistaModeloMetal.vista(c, ancho: 800, alto: 600, radio: 10, corte: nil, elegido: 3, aislado: nil,
                                        claro: false, radioPuntos: 4)
        XCTAssertEqual(u3.bytes, 176)
        // El elegido va + 1 (0 es ninguno), tras la matriz y cinco float4.
        XCTAssertEqual(u3.palabras[36], 4)
        XCTAssertEqual(u3.palabras[40], 0)
    }
}

final class PlanoPorElPuenteTests: XCTestCase {
    func testUnDxfSeAbrePorCapasYSusBuferesSeLeenEnSwift() throws {
        let carpeta = try carpetaTemporal("plano")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let (_, plano, _) = try abrirPlanoDePrueba(carpeta)
        XCTAssertEqual(plano.capas().map(\.nombre), ["Ejes", "Muros"])
        XCTAssertFalse(plano.capas()[0].encendida, "Ejes esta apagada en el plano")
        let r = plano.resumen()
        XCTAssertEqual(r.caja.x0, 0, accuracy: 1e-6)
        XCTAssertEqual(r.caja.x1, 100, accuracy: 1e-6)
        XCTAssertEqual(r.caja.y1, 60, accuracy: 1e-6)

        let muros = try XCTUnwrap(plano.grupos().first { $0.capa == "Muros" })
        let b = try plano.buferes(indice: muros.indice)
        let vs = leerEstructuras(b.vertices, como: VerticeCAD.self)
        XCTAssertEqual(vs.count * PasosCAD.vertice, b.vertices.count)
        // Los vertices, respecto al origen del grupo, caen en la caja del plano.
        for v in vs {
            let x = muros.origenX + Double(v.x), y = muros.origenY + Double(v.y)
            XCTAssertTrue(x > -1e-3 && x < 100 + 1e-3 && y > -1e-3 && y < 60 + 1e-3, "\(x), \(y)")
        }
        XCTAssertTrue(vs.contains { abs(muros.origenX + Double($0.x) - 100) < 1e-3 && abs(muros.origenY + Double($0.y) - 60) < 1e-3 })
        // Los indices no se salen y los tramos caben en su bufer.
        let lineas = leerEstructuras(b.lineas, como: UInt32.self)
        XCTAssertTrue(lineas.allSatisfy { $0 == PasosCAD.corte || Int($0) < vs.count })
        let tramos = leerEstructuras(b.tramosLineas, como: TramoCAD.self)
        XCTAssertFalse(tramos.isEmpty)
        XCTAssertTrue(tramos.allSatisfy { Int($0.desde + $0.cuantos) <= lineas.count })
        // El circulo de «Ejes» es un arco exacto de radio 20.
        let ejes = try XCTUnwrap(plano.grupos().first { $0.capa == "Ejes" })
        let arcos = leerEstructuras(try plano.buferes(indice: ejes.indice).arcos, como: ArcoCAD.self)
        XCTAssertEqual(arcos.count, 1)
        XCTAssertEqual(arcos[0].radio, 20, accuracy: 1e-4)
        XCTAssertEqual(ejes.origenX + Double(arcos[0].centroX), 50, accuracy: 1e-4)
        // Caso negativo: un grupo que no existe.
        XCTAssertThrowsError(try plano.buferes(indice: 99))
    }

    func testLaReglaSeEnganchaYLasCotasViajanEnLaCapa() throws {
        let carpeta = try carpetaTemporal("cotas")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let (_, plano, _) = try abrirPlanoDePrueba(carpeta)
        let e = plano.enganchar(x: 100.3, y: 0.2, radio: 1, desde: nil)
        XCTAssertEqual(e.tipo, .punto)
        XCTAssertEqual(e.x, 100, accuracy: 1e-4)
        XCTAssertEqual(plano.enganchar(x: 40, y: 30, radio: 1, desde: nil).tipo, .libre)

        let cota = CotaPlano(puntos: [PuntoPlano(x: 0, y: 0), PuntoPlano(x: 100, y: 0)])
        XCTAssertTrue(try plano.guardarAnotado(anotado: AnotadoPlano(cotas: [cota], marcos: [])))
        XCTAssertTrue(FileManager.default.fileExists(atPath: plano.rutaDeLaCapa()))
        let leido = plano.anotado()
        XCTAssertEqual(leido.cotas.count, 1)
        XCTAssertEqual(leido.cotas[0].puntos[1].x, 100, accuracy: 1e-3)
        // Lo mismo otra vez no escribe nada.
        XCTAssertFalse(try plano.guardarAnotado(anotado: leido))
        XCTAssertGreaterThan(plano.encaje().u, 0)
    }

    func testCadaFicheroVaASuVisor() throws {
        let carpeta = try carpetaTemporal("clase")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let csv = carpeta.appendingPathComponent("puntos.csv")
        try puntosDePrueba.write(to: csv, atomically: true, encoding: .utf8)
        let notas = carpeta.appendingPathComponent("notas.txt")
        try "hola\nque tal\n".write(to: notas, atomically: true, encoding: .utf8)
        XCTAssertEqual(VentanasDePlano.clase(de: carpeta.appendingPathComponent("a.DWG")), .plano)
        XCTAssertEqual(VentanasDePlano.clase(de: carpeta.appendingPathComponent("a.ifc")), .modelo3d)
        XCTAssertEqual(VentanasDePlano.clase(de: csv), .modelo3d)
        // Casos negativos: un texto cualquiera y una foto.
        XCTAssertEqual(VentanasDePlano.clase(de: notas), .ninguna)
        XCTAssertEqual(VentanasDePlano.clase(de: carpeta.appendingPathComponent("a.png")), .ninguna)
    }

    func testUnFicheroDePuntosDaUnModelo3dConSusVertices() throws {
        let carpeta = try carpetaTemporal("puntos")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let csv = carpeta.appendingPathComponent("PNEZD.csv")
        try puntosDePrueba.write(to: csv, atomically: true, encoding: .utf8)
        let m = try Modelo3dApple.abrir(ruta: csv.path)
        let r = m.resumen()
        let vs = leerEstructuras(m.vertices(), como: V3CAD.self)
        XCTAssertEqual(UInt64(vs.count), r.vertices)
        for v in vs {
            XCTAssertTrue(v.x >= r.caja.x0 - 1e-3 && v.x <= r.caja.x1 + 1e-3 && v.z <= r.caja.z1 + 1e-3)
            XCTAssertLessThan(v.elemento, r.elementos)
        }
        let i = m.indices()
        XCTAssertFalse(i.opacos.isEmpty)
        XCTAssertEqual(i.puntos.count, 5 * 6 * 4)
        // Ocultar las curvas de nivel quita rayas y deja el terreno.
        let curvas = try XCTUnwrap(m.categorias().first { $0.tipo == "Curvas de nivel" })
        let v = m.visibles(filtro: Filtro3d(nivelesOcultos: [], tiposOcultos: [curvas.tipo], aislado: nil))
        XCTAssertLessThan(v.lineas.count, i.lineas.count)
        XCTAssertEqual(v.opacos, i.opacos)
        // Un rayo desde arriba toca el terreno.
        let c = r.caja
        let e = m.elegir(origen: Punto3d(x: Double(c.x0 + c.x1) / 2, y: Double(c.y0 + c.y1) / 2, z: Double(c.z1) + 50),
                         direccion: Punto3d(x: 0, y: 0, z: -1),
                         filtro: Filtro3d(nivelesOcultos: [], tiposOcultos: [], aislado: nil), corte: nil)
        XCTAssertNotNil(e)
        XCTAssertNotNil(m.elemento(id: try XCTUnwrap(e).elemento))
        // Caso negativo: un fichero que no esta.
        XCTAssertThrowsError(try Modelo3dApple.abrir(ruta: carpeta.appendingPathComponent("no.ifc").path))
    }
}

final class CamarasTests: XCTestCase {
    func testLaCamaraDelPlanoEncuadraYAcercaSinMoverElPuntoFijo() {
        let caja = CajaPlano(x0: 500_000, y0: 8_000_000, x1: 500_100, y1: 8_000_060)
        let c = CamaraPlano.encuadrar(caja, ancho: 800, alto: 600)
        let a = c.aPixel(PuntoPlano(x: caja.x0, y: caja.y0), ancho: 800, alto: 600)
        let b = c.aPixel(PuntoPlano(x: caja.x1, y: caja.y1), ancho: 800, alto: 600)
        XCTAssertTrue(a.x >= 0 && a.y >= 0 && b.x <= 800 && b.y <= 600, "\(a) \(b)")
        // Del pixel al dibujo y vuelta.
        let p = c.aDibujo(x: 123, y: 456, ancho: 800, alto: 600)
        let q = c.aPixel(p, ancho: 800, alto: 600)
        XCTAssertEqual(q.x, 123, accuracy: 1e-6)
        XCTAssertEqual(q.y, 456, accuracy: 1e-6)
        // Acercar deja quieto el punto bajo el puntero.
        let z = c.zoom(2, x: 123, y: 456, ancho: 800, alto: 600)
        let p2 = z.aDibujo(x: 123, y: 456, ancho: 800, alto: 600)
        XCTAssertEqual(p2.x, p.x, accuracy: 1e-6)
        XCTAssertEqual(z.px, c.px / 2, accuracy: 1e-12)
        // Mover: el dibujo sigue al dedo.
        let m = c.mover(dx: 10, dy: 0)
        XCTAssertEqual(m.aPixel(p, ancho: 800, alto: 600).x, 133, accuracy: 1e-6)
        // Caso negativo: un factor imposible no cambia nada.
        XCTAssertEqual(c.zoom(0, x: 0, y: 0, ancho: 800, alto: 600), c)
    }

    func testSoloSeDibujanLosTramosQueSeVen() {
        func t(_ d: UInt32, _ n: UInt32, _ c: [Float], _ tam: Float) -> TramoCAD {
            TramoCAD(desde: d, cuantos: n, x0: c[0], y0: c[1], x1: c[2], y1: c[3], tamano: tam, clase: 0)
        }
        let tramos = [t(0, 10, [0, 0, 100, 100], 64), t(10, 5, [0, 0, 10, 10], 8),
                      t(15, 5, [500, 500, 510, 510], 8), t(20, 5, [0, 0, 1, 1], 0.5)]
        let v = tramosVisibles(tramos, en: CajaF(x0: -5, y0: -5, x1: 50, y1: 50), minimo: 1)
        XCTAssertEqual(v.map(\.desde), [0])
        XCTAssertEqual(v.map(\.cuantos), [15])
        XCTAssertEqual(tramosVisibles(tramos, en: CajaF(x0: -500, y0: -500, x1: 600, y1: 600), minimo: 16).map(\.cuantos), [10])
        XCTAssertTrue(tramosVisibles(tramos, en: CajaF(x0: 2000, y0: 2000, x1: 3000, y1: 3000), minimo: 1).isEmpty)
    }

    func testLosEjesDeLaCamara3dSonLosDeRust() {
        let c = Camara3d(objetivo: Punto3d(x: 1, y: 2, z: 3), rumbo: 0.4, altura: 0.6, dist: 20)
        let (r, u, f) = ejesDeCamara(c)
        let ojo = camaraOjo(camara: c)
        let haciaElObjetivo = SIMD3<Double>(c.objetivo.x - ojo.x, c.objetivo.y - ojo.y, c.objetivo.z - ojo.z) / 20
        let d = haciaElObjetivo - f
        XCTAssertLessThan(max(abs(d.x), abs(d.y), abs(d.z)), 1e-9)
        // Derecha y arriba, unitarios y perpendiculares a adelante.
        XCTAssertEqual((r * f).sum(), 0, accuracy: 1e-9)
        XCTAssertEqual((u * f).sum(), 0, accuracy: 1e-9)
        XCTAssertGreaterThan(u.z, 0)
        // El rayo del centro de la pantalla es «adelante».
        let rayo = camaraRayo(camara: c, x: 400, y: 300, ancho: 800, alto: 600)
        XCTAssertEqual(rayo.x, f.x, accuracy: 1e-9)
        let l = luzDeCamara(c)
        XCTAssertEqual(Double((l * l).sum()), 1, accuracy: 1e-5)
    }

    func testElColorEnPapelPoneElSieteEnNegroYOscureceLoClaro() {
        XCTAssertEqual(ColorCAD.enPapel(0x0000_0000), ColorCAD(r: 0, g: 0, b: 0, a: 1))
        let amarillo = ColorCAD.enPapel(0xFF00_FFFF)
        XCTAssertLessThan(amarillo.r, 1)
        // Caso negativo: un color oscuro no cambia.
        XCTAssertEqual(ColorCAD.enPapel(0xFF40_2010), ColorCAD(0xFF40_2010))
    }
}

final class MetalPlanosTests: XCTestCase {
    /// La tarjeta, o se salta (las maquinas virtuales de la CI pueden no tener).
    private func motor() throws -> MotorMetal {
        guard let m = MotorMetal(dispositivo: MTLCreateSystemDefaultDevice()) else {
            throw XCTSkip("Sin tarjeta con Metal")
        }
        return m
    }

    func testLosSombreadoresCompilanYLasTuberiasSeGuardan() throws {
        let m = try motor()
        let plano = try m.biblioteca(SombreadoresCAD.plano, clave: "plano")
        for f in SombreadoresCAD.funcionesPlano { XCTAssertNotNil(plano.makeFunction(name: f), f) }
        let modelo = try m.biblioteca(SombreadoresCAD.modelo, clave: "modelo")
        for f in SombreadoresCAD.funcionesModelo { XCTAssertNotNil(modelo.makeFunction(name: f), f) }
        try PintorPlano(motor: m, color: .bgra8Unorm, muestras: m.muestras).preparar()
        let antes = m.cuantasTuberias
        try PintorPlano(motor: m, color: .bgra8Unorm, muestras: m.muestras).preparar()
        XCTAssertEqual(m.cuantasTuberias, antes, "la segunda vez salen de la cache")
        for (v, f, mezcla) in [("vs_3d", "ps_3d", false), ("vs_3d", "ps_arista", true), ("vs_3d", "ps_tapa", false),
                               ("vs_3d", "ps_linea", true), ("vs_punto", "ps_punto", false), ("vs_3d", "ps_vidrio", true)] {
            XCTAssertNoThrow(try m.tuberia(fuente: SombreadoresCAD.modelo, claveFuente: "modelo", vertice: v, fragmento: f,
                                           color: .bgra8Unorm, profundidad: .depth32Float, muestras: m.muestras, mezcla: mezcla))
        }
        // Caso negativo: una funcion que no existe.
        XCTAssertThrowsError(try m.tuberia(fuente: SombreadoresCAD.plano, claveFuente: "plano", vertice: "vs_no_esta",
                                           fragmento: "ps_color", color: .bgra8Unorm, profundidad: .invalid,
                                           muestras: 1, mezcla: true))
    }

    func testElPlanoSePintaFueraDePantalla() throws {
        let m = try motor()
        let carpeta = try carpetaTemporal("pintar")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let ruta = carpeta.appendingPathComponent("planta.dxf")
        try dxfDePrueba.write(to: ruta, atomically: true, encoding: .utf8)
        let nucleo = try Nucleo.abrir(raiz: carpeta.appendingPathComponent("datos").path, nombreDelAparato: "Mac")
        let datos = try PlanoEnTarjeta.cargar(ruta: ruta.path, nucleo: nucleo, motor: m)
        XCTAssertEqual(datos.grupos.count, 2)

        let lado = 128
        let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: lado, height: lado, mipmapped: false)
        d.usage = [.renderTarget, .shaderRead]
        d.storageMode = m.dispositivo.hasUnifiedMemory ? .shared : .managed
        let tex = try XCTUnwrap(m.dispositivo.makeTexture(descriptor: d))
        let pasada = MTLRenderPassDescriptor()
        pasada.colorAttachments[0].texture = tex
        pasada.colorAttachments[0].loadAction = .clear
        pasada.colorAttachments[0].storeAction = .store
        pasada.colorAttachments[0].clearColor = MTLClearColor(red: 1, green: 1, blue: 1, alpha: 1)
        let cb = try XCTUnwrap(m.cola.makeCommandBuffer())
        let enc = try XCTUnwrap(cb.makeRenderCommandEncoder(descriptor: pasada))
        let camara = CamaraPlano.encuadrar(datos.resumen.cajaUtil, ancho: Double(lado), alto: Double(lado))
        try PintorPlano(motor: m, color: .bgra8Unorm, muestras: 1)
            .pintar(datos, visibles: Set(datos.plano.gruposVisibles()), camara: camara,
                    ancho: Double(lado), alto: Double(lado), oscuro: false, en: enc)
        enc.endEncoding()
        if tex.storageMode == .managed, let blit = cb.makeBlitCommandEncoder() {
            blit.synchronize(resource: tex)
            blit.endEncoding()
        }
        cb.commit()
        cb.waitUntilCompleted()
        XCTAssertNil(cb.error)
        var pixeles = [UInt8](repeating: 0, count: lado * lado * 4)
        tex.getBytes(&pixeles, bytesPerRow: lado * 4, from: MTLRegionMake2D(0, 0, lado, lado), mipmapLevel: 0)
        // Las rayas de «Muros» (rojas en el fichero) dejan pixeles que no son blancos.
        let pintados = stride(from: 0, to: pixeles.count, by: 4).filter { pixeles[$0] < 200 || pixeles[$0 + 1] < 200 }.count
        XCTAssertGreaterThan(pintados, 20)
    }
}
