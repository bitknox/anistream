// swift-tools-version: 6.2
//
// Every string the apps show, generated from the Fluent catalogue in i18n/ by tools/i18n: the
// String Catalog and L.swift, one typed function per message. Nothing here is edited by hand
// except this file and the tests. The Xcode project builds these sources as a framework too
// (project.yml), twice like the other packages, which is why L finds its bundle both ways.
import PackageDescription

let package = Package(
    name: "AnistreamStrings",
    defaultLocalization: "en",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "AnistreamStrings", targets: ["AnistreamStrings"]),
    ],
    targets: [
        .target(
            name: "AnistreamStrings",
            resources: [.process("Localizable.xcstrings"), .process("Flags")]
        ),
        .testTarget(name: "AnistreamStringsTests", dependencies: ["AnistreamStrings"]),
    ]
)
