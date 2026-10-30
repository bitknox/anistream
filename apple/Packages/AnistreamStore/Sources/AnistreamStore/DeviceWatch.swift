// What the device says about itself, told to the engine: the inputs to when a trailer may start by
// itself besides the network (`Command::DeviceState`), said once and again whenever one changes.

import AnistreamKit
import Foundation
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// Follows the system's Reduce Motion, Auto-Play Video Previews, Low Power Mode, thermal state and
/// screen reader, and says them to the engine as one `Command::DeviceState`.
@MainActor
public final class DeviceWatch {
    /// What the device holds back, as the engine takes it.
    public struct State: Equatable, Sendable {
        public var reduceMotion = false
        public var autoplayPreviews = true
        public var lowPower = false
        public var thermalSerious = false
        public var screenReader = false
        public var lowMemory = false

        public init() {}

        /// The state as the system says it now. Apple devices are not counted as low-memory ones.
        public static func current() -> State {
            var state = State()
            let info = ProcessInfo.processInfo
            state.lowPower = info.isLowPowerModeEnabled
            state.thermalSerious = info.thermalState == .serious || info.thermalState == .critical
            #if canImport(UIKit)
            state.reduceMotion = UIAccessibility.isReduceMotionEnabled
            state.autoplayPreviews = UIAccessibility.isVideoAutoplayEnabled
            state.screenReader = UIAccessibility.isVoiceOverRunning
            #else
            state.reduceMotion = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
            state.screenReader = NSWorkspace.shared.isVoiceOverEnabled
            #endif
            return state
        }
    }

    private let send: @MainActor (Command) -> Void
    private let read: @MainActor () -> State
    private var said: State?
    private var observers: [any NSObjectProtocol] = []

    public init(send: @escaping @MainActor (Command) -> Void, read: @escaping @MainActor () -> State = State.current) {
        self.send = send
        self.read = read
    }

    /// Say the state as it stands, and again at each change.
    public func start() {
        guard observers.isEmpty else { return }
        let center = NotificationCenter.default
        var names: [Notification.Name] = [
            .NSProcessInfoPowerStateDidChange, ProcessInfo.thermalStateDidChangeNotification,
        ]
        #if canImport(UIKit)
        names += [
            UIAccessibility.reduceMotionStatusDidChangeNotification,
            UIAccessibility.videoAutoplayStatusDidChangeNotification,
            UIAccessibility.voiceOverStatusDidChangeNotification,
        ]
        #endif
        observers = names.map { name in
            center.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                Task { @MainActor in self?.tell() }
            }
        }
        #if os(macOS)
        // Reduce Motion and VoiceOver on the Mac both come through the display options.
        observers.append(
            NSWorkspace.shared.notificationCenter.addObserver(
                forName: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil, queue: .main
            ) { [weak self] _ in Task { @MainActor in self?.tell() } })
        #endif
        tell()
    }

    /// Say it if it changed.
    public func tell() {
        let state = read()
        guard state != said else { return }
        said = state
        AppLog.info("device", "reduce motion \(state.reduceMotion), low power \(state.lowPower), screen reader \(state.screenReader)")
        send(.deviceState(
            reduceMotion: state.reduceMotion, autoplayPreviews: state.autoplayPreviews, lowPower: state.lowPower,
            thermalSerious: state.thermalSerious, screenReader: state.screenReader, lowMemory: state.lowMemory))
    }
}
