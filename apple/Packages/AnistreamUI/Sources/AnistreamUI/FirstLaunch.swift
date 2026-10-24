// First launch (native-ux, First launch): three short pages, then the app. Welcome says plainly
// that anistream hosts nothing; AniList offers the sign-in; Sources takes an invite link and says
// whose code a plugin is. Every page can be skipped, and each step is in Settings afterwards. A
// full-screen cover on iPhone, a sheet on Mac (560 × 480 pt) and iPad.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// Whether first launch has been seen, kept in the app's defaults.
enum FirstLaunchFlag {
    static let key = "firstLaunchSeen"

    /// Whether to show it on this launch: once, in the app itself, and not to an install already
    /// in use (a source, a signed-in tracker or something watched), which is marked seen instead.
    /// The Fixtures app and the dev harness show it only when a shot asks with `sheet=welcome`.
    @MainActor
    static func shows(_ navigator: Navigator, store: AppStore) async -> Bool {
        if navigator.launchSheet == "welcome" { return true }
        guard Bundle.main.bundleIdentifier == "tv.anistream.app" else { return false }
        let seen = UserDefaults.standard.bool(forKey: key)
        let due = await store.firstLaunchDue(seen: seen)
        if !seen, !due { UserDefaults.standard.set(true, forKey: key) }
        return due
    }
}

/// Puts first launch over the app while it is due.
struct FirstLaunchPresenter: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var showing = false
    /// The invite link Join took, opened in the join sheet once first launch has gone.
    @State private var joining: String?
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #endif

    func body(content: Content) -> some View {
        content
            .task {
                if await FirstLaunchFlag.shows(navigator, store: store) {
                    if navigator.launchSheet == "welcome" { navigator.launchSheet = nil }
                    showing = true
                }
            }
            #if os(iOS)
            .fullScreenCover(isPresented: compact ? $showing : .constant(false), onDismiss: done) { flow }
            .sheet(isPresented: compact ? .constant(false) : $showing, onDismiss: done) { flow }
            #else
            .sheet(isPresented: $showing, onDismiss: done) {
                flow.frame(width: 560, height: 480)
            }
            #endif
    }

    private var flow: some View {
        FirstLaunch { link in
            joining = link
            showing = false
        }
    }

    private func done() {
        UserDefaults.standard.set(true, forKey: FirstLaunchFlag.key)
        if let joining {
            navigator.invite = joining
            self.joining = nil
        }
    }

    #if os(iOS)
    private var compact: Bool { sizeClass == .compact }
    #endif
}

/// The three pages.
struct FirstLaunch: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    /// Ends first launch, with the invite link Join took, if it took one.
    let finish: (String?) -> Void

    enum Page: Int { case welcome, anilist, sources }
    @State private var page = Page.welcome
    @State private var link = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Spacer()
                if page != .sources {
                    Button(cased(mac: "Skip", touch: "Skip")) { finish(nil) }
                        .buttonStyle(.plain)
                        .font(.plat(mac: 13, touch: 17))
                        .foregroundStyle(.haze)
                        .pointerHover()
                }
            }
            .frame(height: 28)
            Group {
                switch page {
                case .welcome: welcome
                case .anilist: anilist
                case .sources: sources
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .transition(.opacity)
            .id(page)
        }
        .padding(padding)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(.ground)
        .animation(reduceMotion ? nil : .standard, value: page)
        // An invite link opened from Messages or Mail jumps here, filled in.
        .onChange(of: navigator.invite) { _, invite in
            if let invite {
                link = invite
                navigator.invite = nil
                page = .sources
            }
        }
        // Signed in: on to Sources.
        .onChange(of: store.accounts.tracker("anilist")?.connected ?? false) { _, connected in
            if connected, page == .anilist { page = .sources }
        }
    }

    private var padding: EdgeInsets {
        #if os(macOS)
        EdgeInsets(top: 16, leading: 40, bottom: 32, trailing: 24)
        #else
        EdgeInsets(top: 8, leading: 24, bottom: 24, trailing: 20)
        #endif
    }

    // MARK: Pages

    private var welcome: some View {
        VStack(alignment: .leading, spacing: 0) {
            Nagare(cell: 5)
                .accessibilityHidden(true)
            Text("anistream")
                .font(.plat(mac: 26, touch: 28, weight: .semibold))
                .foregroundStyle(.silk)
                .padding(.top, 24)
                .accessibilityAddTraits(.isHeader)
            Text("Keep track of anime and play it from sources you choose.")
                .font(.plat(mac: 15, touch: 17))
                .foregroundStyle(.silk)
                .padding(.top, 8)
            Text("anistream hosts no content.")
                .font(.plat(mac: 13, touch: 15))
                .foregroundStyle(.haze)
                .padding(.top, 6)
            Spacer(minLength: 24)
            actions {
                Button { page = .anilist } label: { Text("Continue").touchFill() }
                    .buttonStyle(PrimaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
                    .keyboardShortcut(.defaultAction)
            }
        }
        .fixedSize(horizontal: false, vertical: false)
    }

    private var anilist: some View {
        let working = store.accounts.isWorking("anilist")
        return VStack(alignment: .leading, spacing: 0) {
            heading("AniList")
            Text("Sign in to AniList to sync your list and where you stopped.")
                .font(.plat(mac: 15, touch: 17))
                .foregroundStyle(.silk)
                .padding(.top, 8)
            Text("Other trackers are in Settings › Accounts.")
                .font(.plat(mac: 13, touch: 15))
                .foregroundStyle(.haze)
                .padding(.top, 6)
            Spacer(minLength: 24)
            actions {
                #if os(macOS)
                notNow
                signIn(working)
                #else
                signIn(working)
                notNow
                #endif
            }
        }
    }

    private var sources: some View {
        let trimmed = link.trimmingCharacters(in: .whitespacesAndNewlines)
        return VStack(alignment: .leading, spacing: 0) {
            heading("Sources")
            Text("Sources are plugins, shared through plugin stores by invite. anistream hosts no content.")
                .font(.plat(mac: 15, touch: 17))
                .foregroundStyle(.silk)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 8)
            HStack(spacing: 8) {
                TextField(cased(mac: "Invite Link", touch: "Invite link"), text: $link)
                    .textFieldStyle(.roundedBorder)
                    .autocorrectionDisabled()
                    #if os(iOS)
                    .textInputAutocapitalization(.never)
                    .keyboardType(.URL)
                    #endif
                    .onSubmit { if !trimmed.isEmpty { finish(trimmed) } }
                Button(cased(mac: "Join", touch: "Join")) { finish(trimmed) }
                    .buttonStyle(PrimaryCapsuleStyle(height: fieldHeight, fontSize: buttonFont))
                    .disabled(trimmed.isEmpty)
            }
            .padding(.top, 20)
            Text("Each plugin is someone else's code connecting to someone else's service. Use them at your own risk.")
                .font(.plat(mac: 13, touch: 15))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 12)
            Spacer(minLength: 24)
            actions {
                Button { finish(nil) } label: { Text(cased(mac: "Skip for Now", touch: "Skip for now")).touchFill() }
                    .buttonStyle(SecondaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
            }
        }
    }

    // MARK: Parts

    private var notNow: some View {
        Button { page = .sources } label: { Text(cased(mac: "Not Now", touch: "Not now")).touchFill() }
            .buttonStyle(SecondaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
    }

    private func signIn(_ working: Bool) -> some View {
        Button {
            store.connect("anilist")
        } label: {
            HStack(spacing: 8) {
                if working { ProgressView().controlSize(.small) }
                Text(cased(mac: "Sign In to AniList", touch: "Sign in to AniList"))
            }
            .touchFill()
        }
        .buttonStyle(PrimaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
        .disabled(working)
        .keyboardShortcut(.defaultAction)
    }

    private func heading(_ text: String) -> some View {
        Text(text)
            .font(.plat(mac: 22, touch: 28, weight: .semibold))
            .foregroundStyle(.silk)
            .accessibilityAddTraits(.isHeader)
    }

    /// The page's buttons: trailing on the Mac, as a sheet's are, and full width at the foot on
    /// touch, where the thumb is.
    @ViewBuilder
    private func actions<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        #if os(macOS)
        HStack(spacing: 8) {
            Spacer()
            content()
        }
        #else
        VStack(spacing: 10) {
            content()
        }
        .frame(maxWidth: .infinity)
        #endif
    }

    private var buttonHeight: CGFloat {
        #if os(macOS)
        28
        #else
        50
        #endif
    }

    private var fieldHeight: CGFloat {
        #if os(macOS)
        24
        #else
        36
        #endif
    }

    private var buttonFont: CGFloat {
        #if os(macOS)
        13
        #else
        17
        #endif
    }
}

extension View {
    /// The whole width on touch, where a page's buttons fill its foot; as it is on the Mac.
    @ViewBuilder
    func touchFill() -> some View {
        #if os(iOS)
        frame(maxWidth: .infinity)
        #else
        self
        #endif
    }
}
