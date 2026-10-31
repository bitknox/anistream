// What the player model drives: one mpv, or a script of one in the tests.

import AnistreamKit
import AnistreamPlayer
import CoreGraphics
import Foundation

/// One mpv for one player window. Every call is awaited off the main thread; the model never
/// waits on mpv from it.
public protocol PlayerBackend: AnyObject, Sendable {
    /// mpv's events from now on, until it shuts down. Opened before `begin(on:)`, so nothing
    /// is missed.
    nonisolated func events() -> AsyncStream<MpvEvent>
    /// Start mpv drawing into `layer`, observing what the engine reads.
    func begin(on layer: MpvLayer) async throws
    /// One call; a `restate` or a `sample` answers with the reports it reads.
    func perform(_ call: MpvCall) async throws -> [PlayerReport]
    /// `loadfile … replace`, answered with the playlist entry mpv made for the file, which its
    /// `start-file` names.
    func load(_ url: String, options: [String: String]) async throws -> Int64?
    /// The screen the surface is on.
    func display(_ display: DisplayInfo) async
    /// The surface changed size.
    func resized() async
    /// What the window shows now, as mpv renders it: for Picture in Picture while paused.
    func snapshot() async -> CGImage?
    /// Shut mpv down and end the events. Calls from here on throw.
    func end() async
}

/// The layer as a task on the main actor hands it to `begin(on:)`. `MpvLayer` is `Sendable`, but
/// Swift 6.4's optimised builds (Xcode 27.1, in Release) still fail capturing one in such a task
/// and passing it on as a data race; carried in this, it builds in every configuration.
struct HandedLayer: Sendable {
    let layer: MpvLayer
}

extension MpvClient: PlayerBackend {
    public func begin(on layer: MpvLayer) throws {
        try start(on: layer)
        try observe(reportedProperties)
    }

    public func perform(_ call: MpvCall) throws -> [PlayerReport] {
        switch call {
        case .load(let url, let options):
            _ = try load(url, options: options)
        case .addSubtitle(let url, let title, let language):
            try addSubtitle(url, title: title, language: language, select: false)
        case .set(let property, let value):
            try set(property, value)
        case .command(let arguments):
            try command(arguments)
        case .seek(let seconds, let mode):
            try seek(seconds, mode: mode)
        case .shaders(let paths):
            try setShaders(paths)
        case .restate(let property):
            return [.property(name: property, json: try get(property).json)]
        case .sample(let properties):
            // A property mpv has no value for now, such as the audio's with none playing, is
            // left out, and the engine leaves its row out.
            return properties.compactMap { property in
                (try? get(property)).map { .property(name: property, json: $0.json) }
            }
        }
        return []
    }

    public func display(_ display: DisplayInfo) {
        try? updateDisplay(display)
    }

    public func resized() {
        surfaceResized()
    }

    public func snapshot() -> CGImage? {
        try? screenshot(.window)
    }

    public func end() {
        shutdown()
    }
}
