// swift-tools-version: 6.2
//
// The engine's Swift bindings over its static library, for the other packages to build on. Both
// come from apple/scripts/engine.sh, which links the testing flavour in here: the engine the app
// ships, with `startTesting` and `startScene` beside `start`. See apple/README.md.
import PackageDescription

let package = Package(
    name: "AnistreamKit",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "AnistreamKit", targets: ["AnistreamKit"]),
    ],
    dependencies: [
        .package(path: "../AnistreamStrings"),
    ],
    targets: [
        // The engine as a static library per slice, with the C headers of the bindings and one
        // module map declaring each crate's C module under the name its generated Swift imports.
        .binaryTarget(name: "AnistreamEngine", path: "Engine/AnistreamEngine.xcframework"),
        // The Swift UniFFI generates, a file per crate in one module, in Generated/, and what
        // the app adds to it.
        .target(
            name: "AnistreamKit",
            dependencies: [
                "AnistreamEngine",
                .product(name: "AnistreamStrings", package: "AnistreamStrings"),
            ],
            linkerSettings: [
                // What the Rust library itself links against, as
                // `cargo rustc -p anistream-ffi --lib -- --print native-static-libs` lists it.
                .linkedFramework("CoreFoundation"),
                .linkedFramework("Security"),
                .linkedFramework("SystemConfiguration"),
                .linkedLibrary("c++"),
                .linkedLibrary("iconv"),
            ]
        ),
        .testTarget(name: "AnistreamKitTests", dependencies: ["AnistreamKit"]),
    ]
)
