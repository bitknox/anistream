// swift-tools-version: 6.2
//
// The Mac dev harness: the app's shell on the testing engine, run with `swift run AnistreamDev`,
// which renders its own window to a PNG with `--snapshot`. See apple/README.md.
import PackageDescription

let package = Package(
    name: "AnistreamDev",
    platforms: [.macOS(.v26)],
    dependencies: [
        .package(path: "../AnistreamUI"),
        .package(path: "../AnistreamStore"),
        .package(path: "../AnistreamDesign"),
    ],
    targets: [
        .executableTarget(
            name: "AnistreamDev",
            dependencies: [
                .product(name: "AnistreamUI", package: "AnistreamUI"),
                .product(name: "AnistreamStore", package: "AnistreamStore"),
                .product(name: "AnistreamDesign", package: "AnistreamDesign"),
            ],
            swiftSettings: [.define("FIXTURES")]
        ),
    ]
)
