// How Home lays out: the width the spotlight's art is sized by, and the spotlight's row.

#if os(macOS)
import AnistreamKit
import AnistreamStore
@testable import AnistreamUI
import AppKit
import SwiftUI
import Testing

@MainActor
private final class Widths {
    var seen: [CGFloat] = []
}

/// Sized by the width it measures, as Home's spotlight art is, in a scroll view whose content
/// always asks for half a point more than that.
private struct SizedByItsWidth: View {
    let widths: Widths
    @State private var width: CGFloat = 1_000

    var body: some View {
        ScrollView {
            Color.clear.frame(width: width + 0.5, height: 10)
        }
        .onOfferedWidth { now in
            widths.seen.append(now)
            // Enough to tell a loop, short of spinning the test run forever.
            guard widths.seen.count < 50 else { return }
            width = now
        }
    }
}

@MainActor
struct HomeLayoutTests {
    /// A scroll view grows to content wider than it is given. Measured there, the width would
    /// follow the content and the layout would never settle. The width offered settles at once.
    @Test func theWidthMeasuredIsTheWidthGivenNotWhatTheContentAsks() async throws {
        let widths = Widths()
        let host = NSHostingView(rootView: SizedByItsWidth(widths: widths).frame(width: 500, height: 300))
        host.frame = NSRect(x: 0, y: 0, width: 500, height: 300)
        for _ in 0..<30 {
            host.layoutSubtreeIfNeeded()
            Self.turn()
            try await Task.sleep(for: .milliseconds(1))
        }
        #expect(widths.seen.last == 500)
        #expect(widths.seen.count <= 3, "measured \(widths.seen.count) times: \(widths.seen.prefix(6))")
    }

    private static func turn() { RunLoop.main.run(until: Date().addingTimeInterval(0.005)) }

    /// The spotlight's text takes what the art leaves, even where its buttons need more, so the
    /// row is never wider than the content Home sized the art by.
    @Test(arguments: [500.0, 640.25, 882.0])
    func theSpotlightRowIsNeverWiderThanTheContent(width: Double) {
        let spot = Spotlight(
            case: .continuing, label: .homeSpotlightContinuing,
            entry: entry { $0.synopsis = "A lamplighter walks the old star roads." },
            episode: 7, action: .resume, playable: .found, line: .homeTimeLeft(episode: 7, minutes: 14))
        let row = HomeSpotlightView(spot: spot, contentWidth: width)
            .environment(AppStore())
            .environment(Navigator())
        let fitted = NSHostingController(rootView: row).sizeThatFits(in: CGSize(width: width, height: 2_000))
        #expect(fitted.width <= width + 0.01)
    }
}
#endif
