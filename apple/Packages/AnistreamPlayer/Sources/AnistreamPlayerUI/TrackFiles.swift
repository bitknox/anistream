// Load Subtitle File… and Load Audio File…, on the Mac: the open panel, for the kinds of file mpv
// adds as a track, then `PlayerModel.loadTrack`. The Playback menu and the player's own Subtitles
// and Audio menu both offer them. iPhone and iPad have no such files to hand.

#if os(macOS)
import AnistreamStrings
import AppKit
import UniformTypeIdentifiers

@MainActor
enum TrackFiles {
    /// The extensions offered for each kind, as Linux offers them.
    static func extensions(_ kind: Track.Kind) -> [String] {
        switch kind {
        case .sub: ["srt", "ass", "ssa", "vtt", "sub", "sup"]
        case .audio: ["mka", "m4a", "aac", "ac3", "eac3", "flac", "mp3", "ogg", "opus", "wav"]
        case .video: []
        }
    }

    /// The menus' item: "Load Subtitle File…", "Load Audio File…".
    static func title(_ kind: Track.Kind) -> String {
        kind == .audio ? L.menuLoadAudioFile : L.menuLoadSubtitleFile
    }

    /// Ask for a file of `kind`, and load it into the player if one is chosen.
    static func load(_ kind: Track.Kind, into model: PlayerModel) {
        let panel = NSOpenPanel()
        panel.title = title(kind)
        panel.prompt = L.playerLoadPrompt
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.allowedContentTypes = extensions(kind).compactMap { UTType(filenameExtension: $0) }
        if panel.runModal() == .OK, let url = panel.url {
            model.loadTrack(kind, from: url.path(percentEncoded: false))
        }
    }
}
#endif
