// The sheets that can come up over any screen: the system's sign-in sheet, a sign-in code to
// type elsewhere, and joining a plugin store from an invite link the app was opened with.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

struct AppSheets: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.webSignIn) private var webSignIn

    func body(content: Content) -> some View {
        @Bindable var navigator = navigator
        content
            // Over the iPhone's Settings sheet, the sheet itself shows the code: a window shows
            // one sheet at a time.
            .modifier(DeviceCodePresenter(here: navigator.phoneSettings == nil))
            .sheet(item: Binding<InviteItem?>(
                get: { navigator.invite.map(InviteItem.init) },
                set: { (item: InviteItem?) in navigator.invite = item?.link }
            )) { item in
                #if os(iOS)
                PhoneJoinStoreSheet(link: item.link)
                #else
                JoinStoreSheet(link: item.link)
                #endif
            }
            .onOpenURL { url in
                if let link = InviteLinks.invite(in: url) { navigator.invite = link }
            }
            // AniList's and MyAnimeList's sign-in, when the engine asks for it: the sheet comes
            // back with the redirect, which goes straight to the engine, or was closed.
            .task(id: store.accounts.signIn) {
                guard let request = store.accounts.signIn else { return }
                do {
                    let callback = try await webSignIn.signIn(url: request.url, callbackScheme: request.callbackScheme)
                    store.finishSignIn(request, callback: callback)
                } catch {
                    store.cancelSignIn(request)
                }
            }
    }
}

/// A sign-in code to type on another device, Simkl's or Trakt's, as a sheet over the view, while
/// `here` says this is where it shows. Closed, the same code does not come straight back.
struct DeviceCodePresenter: ViewModifier {
    @Environment(AppStore.self) private var store
    let here: Bool
    /// The code the viewer closed.
    @State private var closedCode: String?

    func body(content: Content) -> some View {
        content.sheet(item: Binding<DeviceCodeItem?>(
            get: {
                guard here, let prompt = store.accounts.deviceCode, prompt.code != closedCode else { return nil }
                return DeviceCodeItem(prompt: prompt)
            },
            set: { (item: DeviceCodeItem?) in
                if item == nil { closedCode = store.accounts.deviceCode?.code }
            }
        )) { item in
            DeviceCodeSheet(prompt: item.prompt)
                #if os(iOS)
                .presentationDetents([.medium])
                #endif
        }
    }
}

/// The title whose "More about this show" sheet is open, as a sheet's item.
struct MoreAboutItem: Identifiable {
    let id: AnilistId
}

extension View {
    /// The "More about this show" sheet, for whichever title the navigator names: a window sheet
    /// 880 wide on the Mac, a page sheet on iPad, the large detent on iPhone.
    func moreAboutSheet(_ navigator: Navigator) -> some View {
        sheet(item: Binding<MoreAboutItem?>(
            get: { navigator.moreAbout.map(MoreAboutItem.init) },
            set: { navigator.moreAbout = $0?.id }
        )) { item in
            #if os(macOS)
            MoreAboutSheet(id: item.id)
                .frame(width: 880)
                .frame(minHeight: 480, idealHeight: 760)
            #else
            if UIDevice.current.userInterfaceIdiom == .phone {
                MoreAboutSheet(id: item.id).presentationDetents([.large])
            } else {
                MoreAboutSheet(id: item.id).presentationSizing(.page)
            }
            #endif
        }
    }
}

/// A title as the per-show sheet's item.
struct ShowItem: Identifiable {
    let entry: Entry
    var id: AnilistId { entry.id }
}

/// An invite link as a sheet's item.
struct InviteItem: Identifiable {
    let link: String
    var id: String { link }
}
