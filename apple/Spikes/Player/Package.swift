// swift-tools-version:6.2
import PackageDescription

// PLAYER_SPIKE_MPV=framework links the re-linked .mpv/Mpv.xcframework that scripts/relink.sh
// makes, in place of MPVKit's static libraries. Unset, the spike links MPVKit's LGPL product.
let framework = Context.environment["PLAYER_SPIKE_MPV"] == "framework"
let mpv: Target.Dependency = framework ? "Mpv" : .product(name: "MPVKit", package: "MPVKit")

var targets: [Target] = [
    // The player: the mpv client, the CAMetalLayer surface for AppKit and UIKit, the SwiftUI
    // controls and the engine's shader chains. Builds for macOS and iOS.
    .target(name: "SpikePlayer", dependencies: [mpv]),
    // The macOS command-line runner: `swift run PlayerSpike <url-or-file> …`.
    .executableTarget(name: "PlayerSpike", dependencies: ["SpikePlayer"]),
]
if framework {
    targets.append(.binaryTarget(name: "Mpv", path: ".mpv/Mpv.xcframework"))
}

let package = Package(
    name: "PlayerSpike",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "SpikePlayer", targets: ["SpikePlayer"])],
    // Declared in both modes: scripts/relink.sh reads MPVKit's artifacts, and a package that
    // loses its only dependency has SwiftPM delete Package.resolved.
    dependencies: [.package(url: "https://github.com/mpvkit/MPVKit", exact: "1.1.0-n9.0.2")],
    targets: targets
)
