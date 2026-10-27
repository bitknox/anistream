// swift-tools-version: 6.2
//
// The home-screen widgets: Continue watching and Airing today, on the Mac, iPad and iPhone.
//
// - AnistreamWidgetData: the snapshot the app writes into the App Group container and the widgets
//   read, where its files are, when it is stale, the timelines' dates, what each state shows, the
//   links and the play intent into the app, and AniList's public schedule for a widget with no
//   snapshot. Engine-free, so the app's store and the widget extension both link it.
// - AnistreamWidgetUI: the widgets themselves, their layouts per size and state, and their
//   timeline providers. The Xcode project compiles these sources into the widget extension.
// - WidgetShots: renders every size and state from a fixture snapshot, for comparing with the
//   boards (`swift run WidgetShots <directory>`).
import PackageDescription

let package = Package(
    name: "AnistreamWidgets",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "AnistreamWidgetData", targets: ["AnistreamWidgetData"]),
        .library(name: "AnistreamWidgetUI", targets: ["AnistreamWidgetUI"]),
        .executable(name: "WidgetShots", targets: ["WidgetShots"]),
    ],
    dependencies: [
        .package(path: "../AnistreamDesign"),
        .package(path: "../AnistreamStrings"),
    ],
    targets: [
        .target(name: "AnistreamWidgetData"),
        .target(
            name: "AnistreamWidgetUI",
            dependencies: [
                "AnistreamWidgetData",
                .product(name: "AnistreamDesign", package: "AnistreamDesign"),
                .product(name: "AnistreamStrings", package: "AnistreamStrings"),
            ]
        ),
        .executableTarget(
            name: "WidgetShots",
            dependencies: ["AnistreamWidgetUI", "AnistreamWidgetData"]
        ),
        .testTarget(
            name: "AnistreamWidgetsTests",
            dependencies: ["AnistreamWidgetData", "AnistreamWidgetUI"],
            resources: [.copy("Snapshots")]
        ),
    ]
)
