// La vista previa de Quick Look (barra espaciadora en el Finder) de los
// dibujos: `.excalidraw` (el lienzo de Windows y Android) y `.pixpin2d` (el
// dibujo propio). El `vista_previa.rs` de Windows, §3 del plan.
//
// Lo lee el nucleo de Rust (`Lienzo.deFichero`, que mira la extension) en
// UN bufer plano, y lo pinta el mismo `PintorLienzo` de la app en un
// contexto vectorial (nitido a cualquier zoom). Las fotos pegadas en el
// lienzo no se cargan: salen como su hueco gris (la vista previa no lee
// mas ficheros que el que se le da).

import AppKit
import PixPinCore
import Quartz

@objc(ProveedorVistaPrevia)
final class ProveedorVistaPrevia: QLPreviewProvider, QLPreviewingController {
    /// Lo mas grande que sale el dibujo (puntos): uno enorme se reduce.
    static let ladoMaximo: CGFloat = 2400
    /// Y uno pequeno se agranda como mucho tanto.
    static let aumentoMaximo: CGFloat = 2
    static let margen: CGFloat = 24

    func providePreview(for request: QLFilePreviewRequest) async throws -> QLPreviewReply {
        let lienzo = try Lienzo.deFichero(ruta: request.fileURL.path)
        let leido = try leerLienzo(lienzo.plano())
        let caja = leido.caja ?? CGRect(x: 0, y: 0, width: 640, height: 480)
        let lado = max(caja.width, caja.height, 1)
        let escala = min(ProveedorVistaPrevia.ladoMaximo / lado, ProveedorVistaPrevia.aumentoMaximo)
        let m = ProveedorVistaPrevia.margen
        let tam = CGSize(width: ceil(max(caja.width, 1) * escala + 2 * m),
                         height: ceil(max(caja.height, 1) * escala + 2 * m))
        return QLPreviewReply(contextSize: tam, isBitmap: false) { ctx, _ in
            ctx.setFillColor(leido.fondo.cg)
            ctx.fill(CGRect(origin: .zero, size: tam))
            // El lienzo tiene la Y hacia abajo; el contexto, hacia arriba.
            ctx.translateBy(x: 0, y: tam.height)
            ctx.scaleBy(x: 1, y: -1)
            ctx.translateBy(x: m, y: m)
            ctx.scaleBy(x: escala, y: escala)
            ctx.translateBy(x: -caja.minX, y: -caja.minY)
            PintorLienzo(lienzo: leido).pintar(en: ctx, limites: caja)
        }
    }
}
