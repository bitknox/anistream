// Handoff of the episode playing: while the player is up, the scene's activity is the episode at
// its place, which your other device offers to resume. It opens there on the title's page.

import AnistreamPlayerUI
import AnistreamStore
import AnistreamUI
import SwiftUI

struct PlaybackHandoff: ViewModifier {
    let player: PlayerModel
    let store: AppStore

    func body(content: Content) -> some View {
        let staged = player.isPresented ? player.staged : nil
        // Read again every ten seconds of playback, so the place handed over is never far
        // behind; the activity itself reads the place when it is asked for.
        let _ = Int(player.position / 10)
        content.userActivity(HandoffTypes.app.playing, isActive: staged != nil) { activity in
            guard let staged = player.staged else { return }
            // Named as the title's page heads it.
            let title = store.titles[staged.id]?.showTitle ?? staged.title
            Handoff.playing(staged.id, episode: staged.episode, position: player.position, translation: player.episode?.translation)
                .describe(activity, title: "\(title) · Episode \(staged.episode)")
        }
    }
}
