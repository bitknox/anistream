// Where the engine finds the command-line tools it runs, such as a VPN client's: a Mac app
// opened from the Finder gets launchd's PATH, /usr/bin:/bin:/usr/sbin:/sbin, where the terminal
// has the shell's, with Homebrew's and /usr/local/bin's tools on it.

#if os(macOS)
import Foundation

enum HelperPath {
    /// Where tools a terminal finds are installed outside launchd's PATH.
    static let usual = ["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin", "/usr/local/sbin"]

    /// Add the usual directories to this process's PATH, after what it has, and say what it is.
    @discardableResult
    static func extend(environment: [String: String] = ProcessInfo.processInfo.environment) -> String {
        let path = extended(environment["PATH"])
        setenv("PATH", path, 1)
        return path
    }

    /// `path` with each of the usual directories it lacks appended, in order.
    static func extended(_ path: String?) -> String {
        var parts = (path ?? "/usr/bin:/bin:/usr/sbin:/sbin").split(separator: ":").map(String.init)
        for directory in usual where !parts.contains(directory) {
            parts.append(directory)
        }
        return parts.joined(separator: ":")
    }
}
#endif
