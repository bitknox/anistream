// swift-tools-version: 6.2
//
// The app's state over the engine: the client that reads its events, the store that applies them
// to a model per feature, artwork, the keychain host and where the engine's files live.
import PackageDescription

let package = Package(
    name: "AnistreamStore",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "AnistreamStore", targets: ["AnistreamStore"]),
    ],
    dependencies: [
        .package(path: "../AnistreamKit"),
    ],
    targets: [
        .target(
            name: "AnistreamStore",
            dependencies: ["AnistreamKit"],
            // The packages build on the testing engine (see AnistreamKit), so its starts are
            // there to name. The Xcode project defines this for the Fixtures app's flavour only.
            swiftSettings: [.define("FIXTURES")]
        ),
        .testTarget(
            name: "AnistreamStoreTests",
            dependencies: ["AnistreamStore"],
            swiftSettings: [.define("FIXTURES")]
        ),
    ]
)
