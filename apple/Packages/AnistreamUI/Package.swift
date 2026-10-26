// swift-tools-version: 6.2
//
// The app's screens: the shells for Mac, iPad and iPhone and what they show, over the store and
// the design system. The app targets and the dev harness both show these.
import PackageDescription

let package = Package(
    name: "AnistreamUI",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "AnistreamUI", targets: ["AnistreamUI"]),
    ],
    dependencies: [
        .package(path: "../AnistreamStore"),
        .package(path: "../AnistreamDesign"),
        .package(path: "../AnistreamStrings"),
        .package(path: "../AnistreamWidgets"),
    ],
    targets: [
        .target(
            name: "AnistreamUI",
            dependencies: [
                .product(name: "AnistreamStore", package: "AnistreamStore"),
                .product(name: "AnistreamDesign", package: "AnistreamDesign"),
                .product(name: "AnistreamStrings", package: "AnistreamStrings"),
                .product(name: "AnistreamWidgetData", package: "AnistreamWidgets"),
            ]
        ),
        .testTarget(name: "AnistreamUITests", dependencies: ["AnistreamUI"]),
    ]
)
