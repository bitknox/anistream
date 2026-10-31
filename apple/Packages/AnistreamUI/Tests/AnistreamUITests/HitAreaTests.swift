#if os(macOS)
import AnistreamKit
import AnistreamStore
@testable import AnistreamUI
import AppKit
import SwiftUI
import Testing

/// Clicks a view the way a hand does: real mouse events through a window, so a test sees the hit
/// area AppKit and SwiftUI give a button, not only the frame it is laid out in. A Mac button takes
/// clicks only on what its label draws or shapes; a frame or shape put on the button from outside
/// changes nothing.
@MainActor
private final class ClickWindow {
    private final class Window: NSWindow {
        // Offscreen and never key in a test run; a button takes no clicks in a window that isn't.
        override var canBecomeKey: Bool { true }
        override var isKeyWindow: Bool { true }
        override var isVisible: Bool { true }
    }

    private let window: Window
    let size: CGSize

    init(_ view: some View, size: CGSize) {
        _ = NSApplication.shared
        self.size = size
        window = Window(
            contentRect: NSRect(origin: .zero, size: size), styleMask: [.borderless],
            backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(
            rootView: view.frame(width: size.width, height: size.height, alignment: .topLeading))
        window.alphaValue = 0
        window.setFrameOrigin(NSPoint(x: -20_000, y: -20_000))
        window.orderFrontRegardless()
    }

    /// Lays the view out and lets it settle before the first click.
    func settle() async throws {
        for _ in 0..<20 { try await turn(0.01) }
    }

    /// A click at a point in the view's space, top left at zero.
    func click(_ point: CGPoint) async throws {
        let location = NSPoint(x: point.x, y: size.height - point.y)
        for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
            let event = NSEvent.mouseEvent(
                with: type, location: location, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1,
                pressure: type == .leftMouseDown ? 1 : 0)!
            window.sendEvent(event)
            try await turn(0.005)
        }
    }

    func close() { window.close() }

    /// A short turn of the run loop, where AppKit and SwiftUI deliver the click and lay out, then
    /// the main actor handed back, so the other tests' work on it goes on between clicks.
    private func turn(_ seconds: Double) async throws {
        Self.run(seconds)
        try await Task.sleep(for: .milliseconds(1))
    }

    private static func run(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }
}

@MainActor
private final class Taps { var count = 0 }

/// One at a time: every test's window says it is key, and a click in one must not land between
/// another's press and release.
@MainActor
@Suite(.serialized)
struct HitAreaTests {
    /// The header's Trailer and Play on TV, and the capsules like them: the padding around the
    /// words takes the click, to the capsule's edge.
    @Test(arguments: ["secondary", "primary"])
    func capsuleTakesClicksToItsEdge(style: String) async throws {
        let taps = Taps()
        let button = Button("Trailer") { taps.count += 1 }
        let view = Group {
            if style == "primary" {
                button.buttonStyle(PrimaryCapsuleStyle(height: 28))
            } else {
                button.buttonStyle(SecondaryCapsuleStyle(height: 28))
            }
        }
        .fixedSize()
        let size = NSHostingView(rootView: view).fittingSize
        #expect(size.width > 60)
        let window = ClickWindow(view, size: size)
        defer { window.close() }
        try await window.settle()
        let points = [
            CGPoint(x: 8, y: size.height / 2), // the padding before the word
            CGPoint(x: size.width - 8, y: size.height / 2), // and after it
            CGPoint(x: size.width / 2, y: 3), // above it
            CGPoint(x: size.width / 2, y: size.height - 3), // below it
        ]
        for point in points {
            let before = taps.count
            try await window.click(point)
            #expect(taps.count == before + 1, "no click at \(point) on a \(size) capsule")
        }
    }

    /// The episode list's All and Unwatched: the raised capsule around the word takes the click.
    @Test func segmentTakesClicksOnItsCapsule() async throws {
        struct Host: View {
            @State var unwatched = false
            let taps: Taps
            var body: some View {
                SegmentedCapsule(selection: $unwatched, options: [(false, "All"), (true, "Unwatched")])
                    .fixedSize()
                    .onChange(of: unwatched) { _, now in if now { taps.count += 1 } }
            }
        }
        let taps = Taps()
        let view = Host(taps: taps)
        let size = NSHostingView(rootView: view).fittingSize
        let window = ClickWindow(view, size: size)
        defer { window.close() }
        try await window.settle()
        // Unwatched's capsule, 6 pt in from its trailing end: on the padding, clear of the word.
        try await window.click(CGPoint(x: size.width - 2 - 6, y: size.height / 2))
        #expect(taps.count == 1)
    }

    /// A day in the calendar: each entry is one hit area the column's width, from the top of its
    /// banner to the foot of its last line, the gaps inside it included, and two entries meet
    /// with nothing between them that takes no click or goes to the farther one.
    @Test func calendarEntriesFillTheirColumn() async throws {
        let store = AppStore()
        let navigator = Navigator()
        let now = Date()
        let first = Broadcast(
            entry: entry { $0.title = "Lantern Ferry at Dusk" }, episode: 5, airs: now.addingTimeInterval(-13 * 3600))
        let second = Broadcast(
            entry: entry { $0.id = 900_002; $0.title = "Short" }, episode: 2, airs: now.addingTimeInterval(-3600))
        let third = Broadcast(
            entry: entry { $0.id = 900_003; $0.title = "Paper Moon Courier"; $0.progress = TitleProgress(done: 3, next: 4) },
            episode: 3, airs: now.addingTimeInterval(-7200))
        let width: CGFloat = 160
        // Not today, which puts the line for now in the column.
        let day = DayBroadcasts(day: now.addingTimeInterval(-3 * 86_400), broadcasts: [first, second, third], now: now)
            .frame(width: width)
            .fixedSize(horizontal: false, vertical: true)
            .environment(store)
            .environment(navigator)
        let height = NSHostingView(rootView: day).fittingSize.height
        let window = ClickWindow(day, size: CGSize(width: width, height: height))
        defer { window.close() }
        try await window.settle()

        // Down the column, every 4 pt from the first cell's top padding to the last's foot, and
        // across it from edge to edge: which entry each click opens.
        var rows: [(y: CGFloat, ids: Set<AnilistId?>)] = []
        let edge = 8 - BroadcastCell.inset.height
        for y in stride(from: edge + 1, through: height - edge - 1, by: 4) {
            var ids: Set<AnilistId?> = []
            for x in [1, width * 0.25, width / 2, width * 0.75, width - 1] {
                navigator.focusedTitle = nil
                try await window.click(CGPoint(x: x, y: y))
                ids.insert(navigator.focusedTitle)
            }
            rows.append((y, ids))
        }
        let missed = rows.filter { $0.ids.contains(nil) }.map(\.y)
        #expect(missed.isEmpty, "clicks that opened nothing, at these heights: \(missed)")
        let split = rows.filter { $0.ids.count > 1 }.map(\.y)
        #expect(split.isEmpty, "rows that open different entries across the column: \(split)")
        // Top to bottom, the three in order, each in one run.
        var order: [AnilistId] = []
        for row in rows {
            if let id = row.ids.first ?? nil, order.last != id { order.append(id) }
        }
        #expect(order == [900_001, 900_002, 900_003])
    }
}
#endif
