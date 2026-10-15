// **Imprimir un plano en vectores**: `NSPrintOperation` con una vista que
// pinta los buferes del plano con Core Graphics (`CGPath`), no una foto de
// la pantalla. Los buferes se leen de la memoria de la tarjeta
// (`MTLBuffer.contents()`, compartida con la CPU): no hay otra copia.
//
// Lo que sale: rellenos, rayas (con el grosor de su capa), circulos y arcos
// exactos y las letras. Los sombreados con patron salen como un tono suave
// de su color (sus rayas las calcula la tarjeta pixel a pixel; en papel
// harian falta miles de trazos). Los colores, los de papel blanco: el color
// 7 en negro y lo muy claro oscurecido, como en el fondo claro del visor.

import AppKit
import Metal
import PixPinCore

@MainActor
enum ImpresionPlano {
    /// Imprime `region` (un marco para imprimir o lo que se ve) del plano.
    static func imprimir(_ d: PlanoEnTarjeta, visibles: Set<UInt32>, region: CajaPlano, titulo: String, ventana: NSWindow?) {
        guard region.x1 > region.x0, region.y1 > region.y0,
              let info = NSPrintInfo.shared.copy() as? NSPrintInfo else { return }
        info.orientation = (region.x1 - region.x0) >= (region.y1 - region.y0) ? .landscape : .portrait
        info.horizontalPagination = .fit
        info.verticalPagination = .fit
        info.isHorizontallyCentered = true
        info.isVerticallyCentered = true
        let tam = info.imageablePageBounds.size
        let vista = VistaImpresionPlano(frame: NSRect(origin: .zero, size: tam), datos: d, visibles: visibles, region: region)
        let op = NSPrintOperation(view: vista, printInfo: info)
        op.jobTitle = titulo
        op.showsPrintPanel = true
        op.showsProgressPanel = true
        if let ventana {
            op.runModal(for: ventana, delegate: nil, didRun: nil, contextInfo: nil)
        } else {
            op.run()
        }
    }
}

/// Lo que se manda a la impresora.
final class VistaImpresionPlano: NSView {
    let datos: PlanoEnTarjeta
    let visibles: Set<UInt32>
    let region: CajaPlano

    init(frame: NSRect, datos: PlanoEnTarjeta, visibles: Set<UInt32>, region: CajaPlano) {
        self.datos = datos
        self.visibles = visibles
        self.region = region
        super.init(frame: frame)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) no se usa") }

    override var isOpaque: Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        ctx.setFillColor(CGColor(gray: 1, alpha: 1))
        ctx.fill(bounds)
        let r = region
        let w = r.x1 - r.x0, h = r.y1 - r.y0
        guard w > 0, h > 0 else { return }
        // Puntos de papel por unidad del dibujo, el plano centrado.
        let k = min(Double(bounds.width) / w, Double(bounds.height) / h)
        let dx = (Double(bounds.width) - w * k) / 2, dy = (Double(bounds.height) - h * k) / 2
        ctx.clip(to: CGRect(x: dx, y: dy, width: w * k, height: h * k))
        ctx.setLineCap(.round)
        ctx.setLineJoin(.round)
        for g in datos.grupos where visibles.contains(g.ficha.indice) {
            let o = (g.ficha.origenX, g.ficha.origenY)
            ctx.saveGState()
            ctx.translateBy(x: dx + (o.0 - r.x0) * k, y: dy + (o.1 - r.y0) * k)
            ctx.scaleBy(x: k, y: k)
            let caja = CajaF(x0: Float(r.x0 - o.0), y0: Float(r.y0 - o.1), x1: Float(r.x1 - o.0), y1: Float(r.y1 - o.1))
            // Lo que en papel mide menos de un cuarto de punto no se ve.
            let minimo = Float(0.25 / k)
            // El grosor de la capa (centesimas de mm) en puntos; el normal, fino.
            let grosor = g.ficha.grosor > 0 ? Double(g.ficha.grosor) * 0.028346 : 0.25
            PintorPapel(ctx: ctx, grupo: g, caja: caja, minimo: minimo, ancho: max(grosor, 0.1) / k).pintar()
            ctx.restoreGState()
        }
    }
}

/// Un trazado que se va llenando mientras el color no cambie.
private struct Lote {
    var color: UInt32?
    var path = CGMutablePath()
    var vacio = true

    mutating func cambiar(_ c: UInt32, _ soltar: (CGPath, UInt32) -> Void) {
        if color != c {
            terminar(soltar)
            color = c
        }
    }

    mutating func terminar(_ soltar: (CGPath, UInt32) -> Void) {
        if !vacio, let c = color { soltar(path, c) }
        path = CGMutablePath()
        vacio = true
    }
}

/// `desde ..< desde + cuantos`, sin salirse de `n` (nunca al reves).
private func rango(_ desde: UInt32, _ cuantos: UInt32, _ n: Int) -> Range<Int> {
    let a = min(Int(desde), n)
    return a ..< max(a, min(Int(desde) + Int(cuantos), n))
}

/// Los buferes de un grupo en papel.
private struct PintorPapel {
    let ctx: CGContext
    let grupo: GrupoMetal
    let caja: CajaF
    let minimo: Float
    let ancho: Double

    private func ver<T>(_ b: MTLBuffer?, _: T.Type) -> UnsafeBufferPointer<T> {
        guard let b else { return UnsafeBufferPointer(start: nil, count: 0) }
        let n = b.length / MemoryLayout<T>.stride
        return UnsafeBufferPointer(start: b.contents().bindMemory(to: T.self, capacity: n), count: n)
    }

    private func rellenar(_ p: CGPath, _ c: UInt32, alfa: Double = 1) {
        let k = ColorCAD.enPapel(c)
        ctx.addPath(p)
        ctx.setFillColor(CGColor(srgbRed: k.r, green: k.g, blue: k.b, alpha: k.a * alfa))
        ctx.fillPath()
    }

    private func trazar(_ p: CGPath, _ c: UInt32) {
        ctx.addPath(p)
        ctx.setStrokeColor(ColorCAD.enPapel(c).cg)
        ctx.setLineWidth(ancho)
        ctx.strokePath()
    }

    func pintar() {
        let vs = ver(grupo.vertices, VerticeCAD.self)
        // Rellenos.
        let tri = ver(grupo.triangulos, UInt32.self)
        var lote = Lote()
        for (d, n) in tramosVisibles(grupo.tramosTriangulos, en: caja, minimo: minimo) {
            var i = Int(d)
            while i + 2 < Int(d + n), i + 2 < tri.count {
                let a = Int(tri[i]), b = Int(tri[i + 1]), c = Int(tri[i + 2])
                i += 3
                guard a < vs.count, b < vs.count, c < vs.count else { continue }
                lote.cambiar(vs[a].color) { rellenar($0, $1) }
                lote.path.addLines(between: [punto(vs[a]), punto(vs[b]), punto(vs[c])])
                lote.path.closeSubpath()
                lote.vacio = false
            }
        }
        lote.terminar { rellenar($0, $1) }
        // Sombreados: un tono de su color.
        let vt = ver(grupo.verticesTrama, VerticeTramaCAD.self)
        let tt = ver(grupo.triangulosTrama, UInt32.self)
        for (d, n) in tramosVisibles(grupo.tramosTrama, en: caja, minimo: minimo) {
            var i = Int(d)
            while i + 2 < Int(d + n), i + 2 < tt.count {
                let a = Int(tt[i]), b = Int(tt[i + 1]), c = Int(tt[i + 2])
                i += 3
                guard a < vt.count, b < vt.count, c < vt.count else { continue }
                lote.cambiar(vt[a].color) { rellenar($0, $1, alfa: 0.25) }
                lote.path.addLines(between: [CGPoint(x: Double(vt[a].x), y: Double(vt[a].y)),
                                             CGPoint(x: Double(vt[b].x), y: Double(vt[b].y)),
                                             CGPoint(x: Double(vt[c].x), y: Double(vt[c].y))])
                lote.path.closeSubpath()
                lote.vacio = false
            }
        }
        lote.terminar { rellenar($0, $1, alfa: 0.25) }
        // Rayas: tiras cortadas por 0xFFFFFFFF.
        let li = ver(grupo.lineas, UInt32.self)
        for (d, n) in tramosVisibles(grupo.tramosLineas, en: caja, minimo: minimo) {
            var empezada = false
            for i in rango(d, n, li.count) {
                let v = li[i]
                guard v != PasosCAD.corte, Int(v) < vs.count else {
                    empezada = false
                    continue
                }
                let p = punto(vs[Int(v)])
                if empezada {
                    lote.path.addLine(to: p)
                } else {
                    lote.cambiar(vs[Int(v)].color) { trazar($0, $1) }
                    lote.path.move(to: p)
                    empezada = true
                    lote.vacio = false
                }
            }
        }
        lote.terminar { trazar($0, $1) }
        // Circulos y arcos, exactos.
        let ar = ver(grupo.arcos, ArcoCAD.self)
        for (d, n) in tramosVisibles(grupo.tramosArcos, en: caja, minimo: minimo) {
            for i in rango(d, n, ar.count) {
                let a = ar[i]
                lote.cambiar(a.color) { trazar($0, $1) }
                let c = CGPoint(x: Double(a.centroX), y: Double(a.centroY))
                if a.barrido >= 6.2831 {
                    let r = Double(a.radio)
                    lote.path.addEllipse(in: CGRect(x: c.x - r, y: c.y - r, width: 2 * r, height: 2 * r))
                } else {
                    let desde = CGPoint(x: c.x + Double(a.radio) * cos(Double(a.inicio)), y: c.y + Double(a.radio) * sin(Double(a.inicio)))
                    lote.path.move(to: desde)
                    lote.path.addArc(center: c, radius: CGFloat(a.radio), startAngle: CGFloat(a.inicio),
                                     endAngle: CGFloat(a.inicio + a.barrido), clockwise: false)
                }
                lote.vacio = false
            }
        }
        lote.terminar { trazar($0, $1) }
        pintarLetras()
    }

    private func punto(_ v: VerticeCAD) -> CGPoint { CGPoint(x: Double(v.x), y: Double(v.y)) }

    /// Cada letra es una instancia de su glifo (triangulos, o pares de
    /// rayas si es de una fuente SHX), colocada con su matriz.
    private func pintarLetras() {
        let le = ver(grupo.letras, LetraCAD.self)
        let gl = ver(grupo.glifos, UInt32.self)
        let ma = ver(grupo.malla, Float.self)
        var rellenos = Lote()
        var rayas = Lote()
        for t in grupo.tramosLetras where t.tamano >= minimo * 3 && caja.corta(t) {
            for i in rango(t.desde, t.cuantos, le.count) {
                let l = le[i]
                let g = Int(l.glifo) * 2
                guard g + 1 < gl.count else { continue }
                let desde = Int(gl[g]), cuantos = Int(gl[g + 1] & ~PasosCAD.glifoDeRayas)
                let deRayas = gl[g + 1] & PasosCAD.glifoDeRayas != 0
                guard (desde + cuantos) * 2 <= ma.count else { continue }
                func p(_ k: Int) -> CGPoint {
                    let ex = Double(ma[(desde + k) * 2]), ey = Double(ma[(desde + k) * 2 + 1])
                    return CGPoint(x: Double(l.x) + Double(l.m0) * ex + Double(l.m1) * ey,
                                   y: Double(l.y) + Double(l.m2) * ex + Double(l.m3) * ey)
                }
                if deRayas {
                    rayas.cambiar(l.color) { trazar($0, $1) }
                    var k = 0
                    while k + 1 < cuantos {
                        rayas.path.move(to: p(k))
                        rayas.path.addLine(to: p(k + 1))
                        k += 2
                    }
                    rayas.vacio = false
                } else {
                    rellenos.cambiar(l.color) { rellenar($0, $1) }
                    var k = 0
                    while k + 2 < cuantos {
                        rellenos.path.addLines(between: [p(k), p(k + 1), p(k + 2)])
                        rellenos.path.closeSubpath()
                        k += 3
                    }
                    rellenos.vacio = false
                }
            }
        }
        rellenos.terminar { rellenar($0, $1) }
        rayas.terminar { trazar($0, $1) }
    }
}
