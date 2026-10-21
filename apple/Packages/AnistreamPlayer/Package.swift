// swift-tools-version: 6.2
//
// libmpv in the app's own view: the client over mpv's API, the layer and the SwiftUI surface it
// draws into, and screenshots. mpv itself is Mpv.xcframework, which apple/scripts/mpv.sh builds
// and links in here (Mpv/, gitignored). See apple/README.md.
import PackageDescription

let package = Package(
    name: "AnistreamPlayer",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "AnistreamPlayer", targets: ["AnistreamPlayer"]),
    ],
    targets: [
        // A dynamic framework per slice that exports only `_mpv_*`, with mpv's headers.
        .binaryTarget(name: "Mpv", path: "Mpv/Mpv.xcframework"),
        .target(name: "AnistreamPlayer", dependencies: ["Mpv"]),
        .testTarget(name: "AnistreamPlayerTests", dependencies: ["AnistreamPlayer"]),
    ]
)
