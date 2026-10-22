import AnistreamWidgetData
import Foundation
import SwiftUI
import Testing
@testable import AnistreamWidgetUI

@Suite("Every layout draws")
@MainActor
struct RenderTests {
    let continuing = [
        SnapshotContinue(id: 900001, title: "Hoshikuzu no Tabibito", episode: 7, fraction: 0.42, resume: true, minutesLeft: 14),
        SnapshotContinue(id: 900002, title: "Kumo no Ue no Toshokan", episode: 4, resume: false),
    ]

    func draws(_ view: some View) -> Bool {
        let renderer = ImageRenderer(content: view.environment(\.widgetDrawsTaps, false).environment(\.widgetNow, now)
            .frame(width: 338, height: 354))
        return renderer.cgImage != nil
    }

    @Test("Continue watching, in each state at each size")
    func continueWatching() {
        var signedOut = signedIn()
        signedOut.signedIn = false
        var stale = signedIn(continuing)
        stale.generatedAt = at(day: 18, 9)
        for snapshot in [nil, signedOut, signedIn(), signedIn(continuing), stale] {
            for layout in WidgetLayout.allCases {
                #expect(draws(ContinueWidgetView(snapshot: snapshot, layout: layout, now: now, format: format)))
            }
        }
    }

    @Test("Airing today, in each state at each size")
    func airingToday() {
        for (snapshot, schedule) in [(signedIn(), nil), (nil, tuesday), (nil, nil), (signedIn(airing: []), nil)] {
            for layout in WidgetLayout.allCases {
                for filter in AiringFilter.allCases {
                    #expect(draws(AiringWidgetView(
                        snapshot: snapshot, publicSchedule: schedule, filter: filter, layout: layout, now: now,
                        format: format)))
                }
            }
        }
    }
}
