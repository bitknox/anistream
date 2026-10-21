// The app's window: the Mac's split view, or the tab bar on iPad and iPhone.

import AnistreamDesign
import AnistreamStore
import SwiftUI

/// What the app's window shows, over the store in the environment.
///
/// The app hands it the store and a `Navigator` in the environment.
public struct RootView: View {
    @Environment(AppStore.self) private var store

    public init() {}

    public var body: some View {
        Group {
            switch store.phase {
            case .failed(let reason):
                StartFailed(reason: reason)
            default:
                #if os(macOS)
                MacShell()
                #else
                TabShell()
                #endif
            }
        }
        .tint(.amber)
        .preferredColorScheme(.dark)
        .modifier(SpotlightResults())
    }
}

/// The engine did not start: what it said, plainly, where the screens would be.
struct StartFailed: View {
    let reason: String

    var body: some View {
        VStack(spacing: 8) {
            Text("anistream could not start")
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            Text(reason)
                .font(.mac(13))
                .foregroundStyle(.haze)
                .multilineTextAlignment(.center)
                .textSelection(.enabled)
                .frame(maxWidth: 480)
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(.ground)
    }
}
