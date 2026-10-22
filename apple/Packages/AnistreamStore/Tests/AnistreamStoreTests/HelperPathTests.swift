// The Mac app finds the tools a terminal finds.

#if os(macOS)
import Testing

@testable import AnistreamStore

struct HelperPathTests {
    @Test func launchdsPathGainsHomebrewsAndUsrLocalsTools() {
        #expect(HelperPath.extended("/usr/bin:/bin:/usr/sbin:/sbin")
            == "/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/local/sbin")
        // A terminal's PATH keeps its own order, with nothing twice.
        #expect(HelperPath.extended("/usr/local/bin:/usr/bin") == "/usr/local/bin:/usr/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/sbin")
    }
}
#endif
