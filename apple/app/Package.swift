// swift-tools-version:6.0
//
// PixPin para Mac: la app nativa (SwiftUI + AppKit) sobre el nucleo de Rust
// (`../PixPinCore`, que hace `herramientas/xcframework.sh`).
//
// - `PixPinKit`: todo lo de la app que se puede probar (lector del lienzo,
//   almacen, vistas). Las pruebas (`swift test`) corren en la CI de macOS.
// - `PixPin`: solo el punto de entrada (`@main`).
//
// La `.app` la monta `herramientas/app.sh` (Info.plist, firma ad hoc): con
// SwiftPM (solo la app) o con Xcode (`apple/xcode/`: la app con sus
// extensiones, que SwiftPM no sabe compilar).
import PackageDescription

let package = Package(
    name: "PixPinMac",
    platforms: [.macOS(.v15)],
    products: [
        .executable(name: "PixPin", targets: ["PixPin"]),
        // Para el proyecto de Xcode (`apple/xcode/project.yml`), que monta la
        // app con sus extensiones: el mismo PixPinKit, sin copiar nada.
        .library(name: "PixPinKit", targets: ["PixPinKit"]),
    ],
    dependencies: [
        .package(path: "../PixPinCore"),
    ],
    targets: [
        .target(
            name: "PixPinKit",
            dependencies: [.product(name: "PixPinCore", package: "PixPinCore")],
            path: "Sources/PixPinKit"
        ),
        .executableTarget(
            name: "PixPin",
            dependencies: ["PixPinKit"],
            path: "Sources/PixPin"
        ),
        .testTarget(
            name: "PixPinKitTests",
            dependencies: ["PixPinKit"],
            path: "Tests/PixPinKitTests"
        ),
    ],
    // Swift 5 por ahora: los enlaces de UniFFI son de Swift 5 y el modo 6
    // estricto pediria marcar a mano cada cruce de hilo. Subir cuando la app
    // este asentada.
    swiftLanguageModes: [.v5]
)
