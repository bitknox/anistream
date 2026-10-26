// The TV sheet going down after Done: what it shows stays what it showed, the remote at its
// full height, rather than the Pair a TV picker the closed model would make of it.

@testable import AnistreamUI
import AnistreamStore
import Testing

@MainActor
struct TVSheetTests {
    @Test func theRemoteStaysTheRemoteWhileItCloses() {
        let tv = TVRemoteModel()
        tv.sheet = .remote
        var last = TVSheetShown()
        last = TVSheetShown(tv)
        #expect(TVSheetShown(tv, last: last).sheet == .remote)

        // Done: the model's sheet goes at once, while the sheet slides away.
        tv.sheetClosed()
        #expect(tv.sheet == nil)
        #expect(TVSheetShown(tv, last: last).sheet == .remote)
    }

    @Test func theSheetFollowsTheModelWhileItIsUp() {
        let tv = TVRemoteModel()
        tv.sheet = .remote
        let last = TVSheetShown(tv)
        // Choose another TV: the picker, in the same sheet.
        tv.sheet = .picker
        #expect(TVSheetShown(tv, last: last).sheet == .picker)
    }
}
