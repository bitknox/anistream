// Settings this device keeps for itself rather than in the engine: the keys the app's views store
// them under with `@AppStorage`, shared by the screens that set them and the parts that read them.

public enum DevicePreferences {
    /// Plain-text subtitles follow the system's caption style. On unless the viewer turns it off.
    public static let followSystemCaptions = "anistream.followSystemCaptions"
}
