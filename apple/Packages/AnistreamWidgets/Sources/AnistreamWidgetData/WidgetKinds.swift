// The widgets' kinds, as WidgetKit names them: what the app looks for among the widgets placed,
// since it keeps its background refresh while any of them is.

/// The kinds of anistream's widgets.
public enum WidgetKinds {
    /// Continue watching.
    public static let continueWatching = "tv.anistream.continue-watching"
    /// Airing today.
    public static let airingToday = "tv.anistream.airing-today"
    /// Both.
    public static let all: Set<String> = [continueWatching, airingToday]

    /// Whether any of `kinds`, the widgets placed, is one of anistream's.
    public static func anyPlaced(_ kinds: some Sequence<String>) -> Bool {
        kinds.contains { all.contains($0) }
    }
}
