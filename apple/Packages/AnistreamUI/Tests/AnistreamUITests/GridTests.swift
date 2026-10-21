// How covers line up in a grid.

#if os(macOS)
import AnistreamKit
import AnistreamStore
@testable import AnistreamUI
import AppKit
import SwiftUI
import Testing

@MainActor
struct GridTests {
    let store = AppStore()
    let navigator = Navigator()

    /// The height a grid cell takes at the Mac's 128 pt column, with an episode line and a meter.
    func height(of title: String) -> CGFloat {
        let cell = PosterCell(entry: entry { $0.secondary = title }, detail: "Ep 7 of 12", fraction: 0.5)
            .frame(width: 128)
            .environment(store)
            .environment(navigator)
        return NSHostingView(rootView: cell).fittingSize.height
    }

    @Test func aOneLineTitleTakesTheRoomOfTwoSoTheEpisodeLinesAlign() {
        let short = height(of: "Relay")
        let long = height(of: "Kumo no Ue no Toshokan")
        #expect(short > 0)
        #expect(short == long)
    }
}
#endif
