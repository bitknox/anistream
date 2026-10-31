// What the app's actions cost against AniList's rate limit: with `-AnistreamRequests`, the
// Fixtures app runs a fixed walk through the screens once it has settled, and prints after each
// step how many requests the fake AniList answered, by operation, and which commands went out.

#if FIXTURES
import AnistreamKit
import AnistreamStore
import Foundation

@MainActor
public enum RequestAudit {
    /// Whether the launch arguments ask for it.
    public static var requested: Bool {
        ProcessInfo.processInfo.arguments.contains("-AnistreamRequests")
    }

    /// The walk: launch, ten steps through Continue at key-repeat pace, a title page and back,
    /// the sections back and forth, each followed by two seconds for the requests to land.
    public static func run(navigator: Navigator, store: AppStore) async {
        var counted = (requests: 0, commands: 0)
        func settle() async { try? await Task.sleep(for: .seconds(2)) }
        func report(_ step: String) {
            let requests = store.anilistRequests.dropFirst(counted.requests)
            let commands = store.sentLog.dropFirst(counted.commands)
            counted = (counted.requests + requests.count, counted.commands + commands.count)
            print("ANISTREAM_REQUESTS \(step): \(requests.count) [\(tally(requests))] commands [\(tally(commands))]")
            fflush(stdout)
        }

        await settle()
        report("launch")

        let queue = store.continuing.queue(store.titles).all.map(\.id)
        if queue.count > 1 {
            var index = 0
            var step = 1
            for _ in 0..<10 {
                if !queue.indices.contains(index + step) { step = -step }
                index += step
                navigator.continueSelection = queue[index]
                try? await Task.sleep(for: .milliseconds(80))
            }
        }
        await settle()
        report("continue, 10 steps")

        if let id = queue.last {
            navigator.openTitle(id)
            await settle()
            report("open a title")
            navigator.back()
            await settle()
            report("back")
        }

        for section in [AppSection.library, .home, .seasonal, .home, .calendar, .home] {
            navigator.open(section)
            try? await Task.sleep(for: .seconds(1))
        }
        await settle()
        report("sections, 6 switches")
        print("ANISTREAM_REQUESTS done: \(counted.requests) in all")
        fflush(stdout)
    }

    /// "Detail×3 Many×1", in the order first seen.
    private static func tally(_ names: ArraySlice<String>) -> String {
        var order: [String] = []
        var counts: [String: Int] = [:]
        for name in names {
            if counts[name] == nil { order.append(name) }
            counts[name, default: 0] += 1
        }
        return order.map { "\($0)×\(counts[$0]!)" }.joined(separator: " ")
    }
}
#endif
