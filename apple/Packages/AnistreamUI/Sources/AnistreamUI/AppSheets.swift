// The sheets that can come up over any screen: a sign-in code to type elsewhere, and joining a
// plugin store from an invite link the app was opened with.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

struct AppSheets: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    /// The code the viewer closed, so it does not come straight back.
    @State private var closedCode: String?

    func body(content: Content) -> some View {
        @Bindable var navigator = navigator
        content
            .sheet(item: Binding<DeviceCodeItem?>(
                get: {
                    guard let prompt = store.accounts.deviceCode, prompt.code != closedCode else { return nil }
                    return DeviceCodeItem(prompt: prompt)
                },
                set: { (item: DeviceCodeItem?) in
                    if item == nil { closedCode = store.accounts.deviceCode?.code }
                }
            )) { item in
                DeviceCodeSheet(prompt: item.prompt)
            }
            .sheet(item: Binding<InviteItem?>(
                get: { navigator.invite.map(InviteItem.init) },
                set: { (item: InviteItem?) in navigator.invite = item?.link }
            )) { item in
                JoinStoreSheet(link: item.link)
            }
            .onOpenURL { url in
                if let link = InviteLinks.invite(in: url) { navigator.invite = link }
            }
    }
}

/// An invite link as a sheet's item.
struct InviteItem: Identifiable {
    let link: String
    var id: String { link }
}
