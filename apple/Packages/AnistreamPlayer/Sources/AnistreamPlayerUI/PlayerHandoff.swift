// Handoff of the episode playing: while the player is up, the episode at its place is offered to
// your other devices, which open it on its title's page. When one takes it over, this one pauses.

import AnistreamKit
import AnistreamStore
import AnistreamStrings

extension PlayerModel {
    /// What plays, as Handoff carries it: the episode at mpv's place, in its translation.
    func handoff(title: (AnilistId) -> String?) -> (handoff: Handoff, title: String)? {
        guard isPresented, let staged else { return nil }
        let name = title(staged.id) ?? staged.title
        return (
            .playing(staged.id, episode: staged.episode, position: position, translation: episode?.translation),
            L.playerTitleEpisodeLine(title: name, episode: L.playerEpisodeText(number: staged.episode))
        )
    }

    /// The episode playing as an activity, followed from now on, which pauses this player when
    /// another device takes it over. `title` names a title as its page heads it.
    public func playingActivity(
        types: HandoffTypes = .app,
        title: @escaping @MainActor (AnilistId) -> String?
    ) -> PlayingActivity {
        let activity = PlayingActivity(
            types: types,
            playing: { [weak self] in self?.handoff(title: title) },
            continued: { [weak self] in self?.pause() }
        )
        activity.follow()
        return activity
    }
}
