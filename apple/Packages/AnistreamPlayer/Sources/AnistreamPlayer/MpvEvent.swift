// What mpv says happened, copied out of its events, and handed to every listener.

import Foundation
import Mpv

/// One of mpv's events, in mpv's own vocabulary.
public enum MpvEvent: Sendable, Equatable {
    /// An observed property changed; `.none` when it became unavailable.
    case property(name: String, value: MpvValue)
    case log(MpvLogLine)
    /// mpv starts the file of playlist entry `entry`, the id its `loadfile` answered with.
    case startFile(entry: Int64)
    case fileLoaded
    /// The first frame after a load or a seek is up.
    case playbackRestart
    case videoReconfig
    case audioReconfig
    case seek
    case idle
    case endFile(MpvEnd)
    case shutdown

    /// mpv's name for the event, as its JSON IPC spells it.
    public var name: String {
        switch self {
        case .property: "property-change"
        case .log: "log-message"
        case .startFile: "start-file"
        case .fileLoaded: "file-loaded"
        case .playbackRestart: "playback-restart"
        case .videoReconfig: "video-reconfig"
        case .audioReconfig: "audio-reconfig"
        case .seek: "seek"
        case .idle: "idle"
        case .endFile: "end-file"
        case .shutdown: "shutdown"
        }
    }
}

public struct MpvLogLine: Sendable, Equatable {
    public let prefix: String
    /// mpv's level name: fatal, error, warn, info, v, debug, trace.
    public let level: String
    public let text: String

    public init(prefix: String, level: String, text: String) {
        self.prefix = prefix
        self.level = level
        self.text = text
    }
}

/// Why a file ended, with mpv's reason names: eof, stop, quit, error, redirect.
public struct MpvEnd: Sendable, Equatable {
    public let reason: String
    /// mpv's error text when `reason` is "error".
    public let error: String?

    public init(reason: String, error: String?) {
        self.reason = reason
        self.error = error
    }
}

extension MpvEvent {
    /// Decodes an event mpv owns, or nil for the ones nobody listens to.
    init?(_ event: mpv_event) {
        switch event.event_id {
        case MPV_EVENT_PROPERTY_CHANGE:
            let property = event.data.assumingMemoryBound(to: mpv_event_property.self).pointee
            let value: MpvValue =
                property.format == MPV_FORMAT_NODE
                ? MpvValue(property.data.assumingMemoryBound(to: mpv_node.self).pointee) : .none
            self = .property(name: String(cString: property.name), value: value)
        case MPV_EVENT_LOG_MESSAGE:
            let message = event.data.assumingMemoryBound(to: mpv_event_log_message.self).pointee
            self = .log(
                MpvLogLine(
                    prefix: String(cString: message.prefix), level: String(cString: message.level),
                    text: String(cString: message.text).trimmingCharacters(in: .newlines)))
        case MPV_EVENT_START_FILE:
            let start = event.data.assumingMemoryBound(to: mpv_event_start_file.self).pointee
            self = .startFile(entry: start.playlist_entry_id)
        case MPV_EVENT_FILE_LOADED: self = .fileLoaded
        case MPV_EVENT_PLAYBACK_RESTART: self = .playbackRestart
        case MPV_EVENT_VIDEO_RECONFIG: self = .videoReconfig
        case MPV_EVENT_AUDIO_RECONFIG: self = .audioReconfig
        case MPV_EVENT_SEEK: self = .seek
        case MPV_EVENT_IDLE: self = .idle
        case MPV_EVENT_SHUTDOWN: self = .shutdown
        case MPV_EVENT_END_FILE:
            let end = event.data.assumingMemoryBound(to: mpv_event_end_file.self).pointee
            let reason =
                switch end.reason {
                case MPV_END_FILE_REASON_EOF: "eof"
                case MPV_END_FILE_REASON_STOP: "stop"
                case MPV_END_FILE_REASON_QUIT: "quit"
                case MPV_END_FILE_REASON_ERROR: "error"
                case MPV_END_FILE_REASON_REDIRECT: "redirect"
                default: "unknown"
                }
            let error = end.error < 0 ? String(cString: mpv_error_string(end.error)) : nil
            self = .endFile(MpvEnd(reason: reason, error: error))
        default:
            return nil
        }
    }
}
