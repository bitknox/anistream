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
    /// One call; a `restate` answers with the report it reads.
    func perform(_ call: MpvCall) async throws -> PlayerReport?
    /// The screen the surface is on.
    func display(_ display: DisplayInfo) async
    /// The surface changed size.
    func resized() async
    /// What the window shows now, as mpv renders it: for Picture in Picture while paused.
    func snapshot() async -> CGImage?
    /// Shut mpv down and end the events. Calls from here on throw.
    func end() async
}

extension MpvClient: PlayerBackend {
    public func begin(on layer: MpvLayer) throws {
        try start(on: layer)
        try observe(reportedProperties)
    }

    public func perform(_ call: MpvCall) throws -> PlayerReport? {
        switch call {
        case .load(let url, let options):
            try load(url, options: options)
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
            return .property(name: property, json: try get(property).json)
        }
        return nil
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
