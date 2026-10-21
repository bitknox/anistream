// swift-tools-version: 6.2
//
// The Obi & Silk design system as SwiftUI takes it: the colour, motion and type tokens, the show
// title face, Nagare, and the cover placeholder. Nothing here knows the engine.
import PackageDescription

let package = Package(
    name: "AnistreamDesign",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "AnistreamDesign", targets: ["AnistreamDesign"]),
    ],
    targets: [
        .target(
            name: "AnistreamDesign",
            // Zen Kaku Gothic New Bold, under the SIL Open Font License beside it.
            resources: [.copy("Fonts")]
        ),
        .testTarget(name: "AnistreamDesignTests", dependencies: ["AnistreamDesign"]),
    ]
)
