// The background refresh: what the app does when the system gives it a moment, iOS's app refresh
// task or the Mac's background activity. Say what aired that no notification said, schedule the
// week, and write the widgets' record, from the engine the app has or one started for the task;
// with nothing notifying and a widget placed, only the widgets' record.

import AnistreamKit
import Foundation

/// One refresh, over the engine `questions` asks.
public struct NoticeRefresh: Sendable {
    public let questions: EngineQuestions
    /// Where notifications go; `nil` schedules none.
    public let scheduler: NoticeScheduler?
    /// What writes the widgets' record, and reloads them: the store's `WidgetFeed`, as in the
    /// foreground (`WidgetFeed.writer(art:)`). `nil` writes none, as in the Fixtures app.
    public let widgets: WidgetWriter?
    /// Whether anything notifies: false when the refresh runs for the widgets alone, and then it
    /// only writes their record, asking nothing of the check or the week and posting nothing.
    public let notifying: Bool

    /// Writes the widgets' record it is handed.
    public typealias WidgetWriter = @MainActor @Sendable (WidgetData) async throws -> Void

    public init(
        questions: EngineQuestions, scheduler: NoticeScheduler?, widgets: WidgetWriter?,
        notifying: Bool = true
    ) {
        self.questions = questions
        self.scheduler = scheduler
        self.widgets = widgets
        self.notifying = notifying
    }

    /// What a refresh did.
    public struct Outcome: Sendable, Equatable {
        /// Notifications posted at once, for episodes no scheduled one said.
        public var posted: [PlannedNotice] = []
        /// The week's notifications, scheduled.
        public var scheduled: [PlannedNotice] = []
        /// Whether the widgets' record was written.
        public var wroteWidgets = false
        /// Whether it ran to the end, rather than being stopped.
        public var finished = false
    }

    /// Check for new episodes, then the week, then the widgets' record, each in turn so a task
    /// stopped part-way has done the most useful part first. With nothing notifying, the
    /// widgets' record alone.
    public func run() async -> Outcome {
        var outcome = Outcome()
        if notifying {
            let said = await questions.newEpisodes() ?? []
            guard !Task.isCancelled else { return outcome }
            let week = await questions.week()
            guard !Task.isCancelled else { return outcome }
            if let scheduler {
                outcome.posted = await scheduler.catchUp(said, week: week)
                if let week {
                    outcome.scheduled = await scheduler.reschedule(week)
                }
            }
        }
        guard !Task.isCancelled else { return outcome }
        if let widgets, let record = await questions.widgetRecord() {
            do {
                try await widgets(record)
                outcome.wroteWidgets = true
            } catch {
                AppLog.error("notices", "could not write the widgets' record: \(error.localizedDescription)")
            }
        }
        outcome.finished = !Task.isCancelled
        return outcome
    }
}

/// An engine started for a background task alone: none of the launch's own background work, so
/// it does what the task asks and nothing else, and is shut down when the task ends.
public final class BackgroundEngine: Sendable {
    private let client: EngineClient
    public let answers: EngineAnswers

    /// No launch work at all: no sweeping, no update check, no digest, no datasets, no sync
    /// timers, no plugin stores, no player check.
    public static let quiet = FfiBackground(
        prune: false, updateCheck: false, digest: false, datasets: false, syncLoops: false,
        storeRefresh: false, playerCheck: false)

    /// Start `launch`'s engine with no background work of its own. Blocks while it opens its
    /// files, so call it off the main thread.
    public init(_ launch: EngineLaunch, host: any FfiHost) throws {
        var launch = launch
        launch.background = Self.quiet
        let engine = try launch.start(host: host)
        let answers = EngineAnswers()
        self.answers = answers
        client = EngineClient(
            engine: engine,
            deliver: { events in answers.deliver(events) },
            stopped: {}
        )
    }

    public var questions: EngineQuestions {
        EngineQuestions(answers: answers) { [client] command in client.send(command) }
    }

    /// Send `command`, as a notification's Mark watched does.
    public func send(_ command: Command) {
        client.send(command)
    }

    /// Stop the engine after what was sent, waiting while it winds down.
    public func shutdown() {
        client.shutdown()
    }
}
