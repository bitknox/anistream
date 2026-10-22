// swift-tools-version: 6.2
//
// libmpv in the app's own view: the client over mpv's API, the layer and the SwiftUI surface it
// draws into, and screenshots. mpv itself is Mpv.xcframework, which apple/scripts/mpv.sh builds
// and links in here (Mpv/, gitignored). See apple/README.md.
//
// AnistreamPlayerUI is the player the app shows: the engine's hosted player protocol carried out
// on that client, and the player's screens for Mac, iPhone and iPad.
import PackageDescription

let package = Package(
    name: "AnistreamPlayer",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "AnistreamPlayer", targets: ["AnistreamPlayer"]),
        .library(name: "AnistreamPlayerUI", targets: ["AnistreamPlayerUI"]),
    ],
    dependencies: [
        .package(path: "../AnistreamStore"),
        .package(path: "../AnistreamDesign"),
    ],
    targets: [
        // A dynamic framework per slice that exports only `_mpv_*`, with mpv's headers.
        .binaryTarget(name: "Mpv", path: "Mpv/Mpv.xcframework"),
        .target(name: "AnistreamPlayer", dependencies: ["Mpv"]),
        .target(
            name: "AnistreamPlayerUI",
            dependencies: [
                "AnistreamPlayer",
                .product(name: "AnistreamStore", package: "AnistreamStore"),
                .product(name: "AnistreamDesign", package: "AnistreamDesign"),
            ],
            // On the testing engine, as the other packages are (see AnistreamKit): the Fixtures
            // app's previews of each state are there to name. The Xcode project defines this
            // for the Fixtures app's flavour only.
            swiftSettings: [.define("FIXTURES")]
        ),
        .testTarget(name: "AnistreamPlayerTests", dependencies: ["AnistreamPlayer"]),
        .testTarget(
            name: "AnistreamPlayerUITests",
            dependencies: ["AnistreamPlayerUI"],
            swiftSettings: [.define("FIXTURES")]
        ),
    ]
)
