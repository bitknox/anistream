// iPhone and iPad: downloads go on after the app leaves, in iOS 26's continued-processing task.
// The system shows its own progress line for it, which the app reports to, and stops it when it
// runs out or the person stops it. The store's ContinuedDownloads decides when to ask and what
// the engine is told; this asks the system and hands back what it opens.

#if os(iOS)
import AnistreamKit
import AnistreamStore
import BackgroundTasks
import Foundation

@MainActor
enum BackgroundDownloads {
    /// What every window's identifier starts with: the bundle's, then `downloads`. Info.plist
    /// permits `<bundle>.downloads.*`; each request has a suffix of its own.
    static var prefix: String { "\(Bundle.main.bundleIdentifier ?? "tv.anistream.app").downloads" }

    /// Whether Info.plist permits the windows: the app's does, the Fixtures app's does not.
    static var permitted: Bool {
        let permitted = Bundle.main.object(forInfoDictionaryKey: "BGTaskSchedulerPermittedIdentifiers") as? [String]
        return permitted?.contains("\(prefix).*") == true
    }

    /// Let `store` ask the system for windows.
    static func install(in store: AppStore) {
        guard permitted else { return }
        store.continued.ask = { [weak store] title, subtitle in
            guard let store else { return false }
            return submit(title: title, subtitle: subtitle, to: store.continued, rows: { store.downloads.rows })
        }
    }

    /// Ask for a window now: registered under an identifier of its own, which a continued task
    /// may be after launch, and failed rather than queued when the system cannot open it at once.
    private static func submit(
        title: String, subtitle: String, to continued: ContinuedDownloads,
        rows: @escaping @MainActor () -> [DownloadRow]
    ) -> Bool {
        let identifier = "\(prefix).\(UUID().uuidString.prefix(8))"
        let registered = BGTaskScheduler.shared.register(forTaskWithIdentifier: identifier, using: .main) { task in
            let task = HandedTask(task)
            MainActor.assumeIsolated {
                guard let window = task.task as? BGContinuedProcessingTask else {
                    task.task.setTaskCompleted(success: false)
                    continued.refused()
                    return
                }
                window.expirationHandler = {
                    Task { @MainActor in continued.expired() }
                }
                continued.began(SystemDownloadWindow(window), rows: rows())
            }
        }
        guard registered else { return false }
        let request = BGContinuedProcessingTaskRequest(identifier: identifier, title: title, subtitle: subtitle)
        request.strategy = .fail
        do {
            try BGTaskScheduler.shared.submit(request)
            return true
        } catch {
            AppLog.info("downloads", "no background window for the downloads: \(error.localizedDescription)")
            return false
        }
    }
}

/// The system's window, reported to.
@MainActor
private final class SystemDownloadWindow: DownloadWindow {
    private let task: BGContinuedProcessingTask

    init(_ task: BGContinuedProcessingTask) {
        self.task = task
    }

    func report(completed: Int64, total: Int64, subtitle: String) {
        task.progress.totalUnitCount = total
        task.progress.completedUnitCount = completed
        task.updateTitle(task.title, subtitle: subtitle)
    }

    func end(success: Bool) {
        task.setTaskCompleted(success: success)
    }
}

/// A task handed from the scheduler's queue to the main actor, where it is used.
private struct HandedTask: @unchecked Sendable {
    let task: BGTask
    init(_ task: BGTask) { self.task = task }
}
#endif
