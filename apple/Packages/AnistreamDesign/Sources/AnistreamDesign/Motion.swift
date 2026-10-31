// The motion tokens from native-spec/motion.md. Springs for anything spatial, never a bounce;
// opacity on the fades. What the platform animates itself (navigation, sheets, menus, the tab
// bar, windows, full screen) keeps its own animation, and countdowns run linear, written in place.

import SwiftUI

extension Animation {
    /// Selection tint, segmented control, toggles, the row dim after marking watched.
    public static let quick = Animation.spring(duration: 0.2, bounce: 0)

    /// Content changes inside a screen: progress bars, Skip opening and Up Next appearing, list
    /// insert and remove, the Mac banner morph, and every Reduce Motion crossfade.
    public static let standard = Animation.spring(duration: 0.3, bounce: 0)

    /// Player controls appearing, artwork arriving, the first video frame over the poster, the
    /// Mac preview.
    public static let fadeIn = Animation.easeOut(duration: 0.2)

    /// A trailer coming out from under the art, as soon as it plays.
    public static let reveal = Animation.easeOut(duration: 0.3)

    /// Player controls hiding.
    public static let fadeOut = Animation.easeIn(duration: 0.3)
}
