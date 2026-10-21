// swift-tools-version: 6.0
//
// The engine's Swift bindings, over the Rust library that apple/build.sh builds and generates
// them from. See apple/README.md.
import PackageDescription

let package = Package(
    name: "AnistreamKit",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "AnistreamKit", targets: ["AnistreamKit"]),
    ],
    targets: [
        // The C side of the bindings: a header per crate with UniFFI types in it, and one module
        // map declaring each crate's C module under the name its generated Swift imports.
        .systemLibrary(name: "AnistreamFFI", path: "Sources/AnistreamFFI"),
        // The Swift side UniFFI generates, a file per crate in one module, linked against the
        // engine's static library.
        .target(
            name: "AnistreamKit",
            dependencies: ["AnistreamFFI"],
            linkerSettings: [
                // Where build.sh links the library in from cargo's target directory. Alone there,
                // since the linker takes a dynamic library over a static one beside it.
                .unsafeFlags(["-L", Context.packageDirectory + "/.rust"]),
                .linkedLibrary("anistream_ffi"),
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
