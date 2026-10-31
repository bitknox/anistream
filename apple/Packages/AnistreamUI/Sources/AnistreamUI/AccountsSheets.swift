// Signing in to the trackers that do not go through the browser, the codes Simkl and Trakt ask
// for, and settling what a library pull found different. Also the browser sign-in AniList and
// MyAnimeList use, behind a protocol, which the engine asks for with `Event::Authorize`.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import AuthenticationServices
import SwiftUI

/// Kitsu's sign-in: its user name or email and password, exchanged for a token by the engine.
struct KitsuSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var username = ""
    @State private var password = ""

    var body: some View {
        let working = store.accounts.isWorking("kitsu")
        VStack(alignment: .leading, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text(L.accountsSignInTo(tracker: AccountsModel.name("kitsu")))
                    .font(.mac(15, weight: .semibold))
                    .foregroundStyle(.silk)
                Text(L.accountsFormDetail(tracker: AccountsModel.name("kitsu")))
                    .font(.mac(13))
                    .foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
            }
            VStack(alignment: .leading, spacing: 10) {
                TextField(L.accountsUserOrEmail, text: $username)
                    .textContentType(.username)
                    #if os(iOS)
                    .textInputAutocapitalization(.never)
                    #endif
                    .autocorrectionDisabled()
                SecureField(L.accountsPassword, text: $password)
                    .textContentType(.password)
                    .onSubmit(signIn)
            }
            .textFieldStyle(SheetFieldStyle())
            HStack(spacing: 10) {
                Spacer()
                if working { ProgressView().controlSize(.small) }
                Button(L.actionCancel) { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .keyboardShortcut(.cancelAction)
                Button(L.actionSignIn, action: signIn)
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
                    .disabled(username.isEmpty || password.isEmpty || working)
            }
        }
        .padding(24)
        .sheetWidth(420)
        .background(.raised)
        .holdsInput([username, password])
        .onChange(of: store.accounts.tracker("kitsu")?.connected) { _, connected in
            if connected == true { dismiss() }
        }
    }

    private func signIn() {
        guard !username.isEmpty, !password.isEmpty else { return }
        store.connectKitsu(username: username, password: password)
        password = ""
    }
}

/// A code to type on another device, for Simkl or Trakt: the code large, how long it has left,
/// where to type it, and a way there. It stays until the engine says the sign-in ended; a code
/// that runs out stays to say so, with a way to a new one.
struct DeviceCodeSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.openURL) private var openURL
    private var dismiss = SurfaceDismiss()
    let prompt: DeviceCodePrompt
    /// A new code was asked for and is on its way.
    @State private var asked = false

    var body: some View {
        // Each second, for the countdown: the code's state is read from the clock, not kept.
        TimelineView(.periodic(from: .now, by: 1)) { context in
            sheet(now: context.date)
        }
        .padding(24)
        .sheetWidth(420)
        .background(.raised)
    }

    private func sheet(now: Date) -> some View {
        let name = AccountsModel.name(prompt.tracker)
        let expired = AccountsModel.expired(prompt, at: now)
        return VStack(alignment: .leading, spacing: 16) {
            Text(L.accountsSignInTo(tracker: name))
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            Text(expired
                 ? L.accountsCodeExpiredDetail(tracker: name)
                 : L.accountsCodeDetail(host: host))
                .font(.mac(13))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
            VStack(spacing: 6) {
                Text(prompt.code)
                    .font(.mac(34, weight: .semibold, design: .monospaced))
                    .kerning(4)
                    .foregroundStyle(expired ? .haze : .silk)
                    .textSelection(.enabled)
                    .accessibilityLabel(L.accountsCodeLabel(code: prompt.code.map(String.init).joined(separator: " ")))
                if let expiry = expiry(now: now) {
                    Text(expiry)
                        .font(.mac(12))
                        .monospacedDigit()
                        .foregroundStyle(.haze)
                }
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 14)
            .background(.ground, in: .rect(cornerRadius: 10))
            // In a row where it fits, as on the Mac; on an iPhone the buttons go under the wait,
            // so "Open auth.trakt.tv" is never cut short.
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 10) {
                    waiting(name, expired: expired)
                    Spacer()
                    buttons(expired: expired)
                }
                VStack(alignment: .leading, spacing: 12) {
                    waiting(name, expired: expired)
                    HStack(spacing: 10) {
                        Spacer()
                        buttons(expired: expired)
                    }
                }
            }
        }
    }

    /// "Expires in 15 min", "Expires in 45 s" in the last minute, then "Code expired"; nothing
    /// when the tracker gave no lifetime.
    private func expiry(now: Date) -> String? {
        prompt.expiresAt.map { deviceCodeExpiry(secondsLeft: $0 - Int64(now.timeIntervalSince1970)) }
    }

    private var host: String {
        URL(string: prompt.url)?.host ?? prompt.url
    }

    @ViewBuilder private func waiting(_ name: String, expired: Bool) -> some View {
        if !expired || asked {
            HStack(spacing: 10) {
                ProgressView().controlSize(.small)
                Text(expired ? L.accountsGettingCode : L.accountsCodeWaiting(tracker: name))
                    .font(.mac(12))
                    .foregroundStyle(.haze)
            }
        }
    }

    @ViewBuilder private func buttons(expired: Bool) -> some View {
        Button(L.actionClose) { dismiss() }
            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            .keyboardShortcut(.cancelAction)
        if expired {
            Button(L.actionNewCode) {
                asked = true
                store.connect(prompt.tracker)
            }
            .buttonStyle(PrimaryCapsuleStyle(height: 28))
            .keyboardShortcut(.defaultAction)
            .fixedSize()
            .disabled(asked)
        } else {
            Button(L.accountsOpenHost(host: host)) {
                if let url = URL(string: prompt.url) { openURL(url) }
            }
            .buttonStyle(PrimaryCapsuleStyle(height: 28))
            .keyboardShortcut(.defaultAction)
            .fixedSize()
        }
    }
}

/// What a library pull found different from here: each title, the field, and which side to keep.
struct ConflictsSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(L.accountsClashTitle(tracker: AccountsModel.name("anilist")))
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            Text(L.accountsClashDetail(tracker: AccountsModel.name("anilist")))
                .font(.mac(13))
                .foregroundStyle(.haze)
            ScrollView {
                VStack(spacing: 0) {
                    ForEach(Array(store.accounts.conflicts.enumerated()), id: \.offset) { index, row in
                        if index > 0 { Rectangle().fill(.faintRule).frame(height: 1) }
                        HStack(spacing: 12) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(row.title).font(.mac(13, weight: .medium)).foregroundStyle(.silk)
                                Text(L.accountsClashRow(field: row.field.firstCapitalized, local: row.local, remote: row.remote, tracker: AccountsModel.name("anilist")))
                                    .font(.mac(12))
                                    .foregroundStyle(.haze)
                            }
                            Spacer()
                            Button(L.accountsKeepMine) { store.resolveConflict(row.anilistId, keepLocal: true) }
                            Button(L.accountsKeepTheirs(tracker: AccountsModel.name("anilist"))) { store.resolveConflict(row.anilistId, keepLocal: false) }
                        }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        .padding(.vertical, 10)
                    }
                }
                .padding(.horizontal, 14)
                .background(.raisedSelected.opacity(0.4), in: .rect(cornerRadius: 12))
            }
            .frame(maxHeight: 360)
            HStack {
                Spacer()
                Button(L.actionDone) { dismiss() }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .sheetWidth(560)
        .background(.raised)
        .closesOnEscape { dismiss() }
        .onChange(of: store.accounts.conflicts.isEmpty) { _, empty in if empty { dismiss() } }
    }
}

/// A text field in a sheet: on the ground, with a hairline edge.
struct SheetFieldStyle: TextFieldStyle {
    func _body(configuration: TextField<Self._Label>) -> some View {
        configuration
            .textFieldStyle(.plain)
            .font(.mac(13))
            .padding(.horizontal, 12)
            .frame(height: 32)
            .background(.ground, in: .rect(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
    }
}

/// A device code as a sheet's item.
struct DeviceCodeItem: Identifiable {
    let prompt: DeviceCodePrompt
    var id: String { prompt.tracker + prompt.code }
}

// MARK: - Browser sign-in

/// A sign-in that goes through the browser and comes back to the app: AniList's implicit grant
/// and MyAnimeList's PKCE. The engine asks for one with `Event::Authorize` and takes the answer
/// with `Command::FinishSignIn`; `AppSheets` runs it.
public protocol WebSignIn: Sendable {
    /// Open `url` and return the URL the provider sent back to `callbackScheme`.
    @MainActor
    func signIn(url: URL, callbackScheme: String) async throws -> URL
}

/// The system's own: `ASWebAuthenticationSession`, in an ephemeral browser session so no
/// provider cookie outlives the sign-in.
public struct SystemWebSignIn: WebSignIn {
    public init() {}

    @MainActor
    public func signIn(url: URL, callbackScheme: String) async throws -> URL {
        let anchor = PresentationAnchor()
        return try await withCheckedThrowingContinuation { continuation in
            let answer = Once(continuation)
            let session = ASWebAuthenticationSession(
                url: url, callback: .customScheme(callbackScheme)
            ) { callback, error in
                if let callback {
                    answer.resume(.success(callback))
                } else {
                    answer.resume(.failure(error ?? ASWebAuthenticationSessionError(.canceledLogin)))
                }
            }
            session.prefersEphemeralWebBrowserSession = true
            session.presentationContextProvider = anchor
            // Held until it answers: the system keeps no session of its own, and one let go of
            // closes its sheet, or never answers at all. Its presentation context is a weak
            // reference, held the same way.
            answer.keep(session, anchor)
            if !session.start() {
                answer.resume(.failure(ASWebAuthenticationSessionError(.presentationContextInvalid)))
            }
        }
    }
}

/// A continuation resumed once, whichever of the session's answers comes first, and what has to
/// live until then.
final class Once: @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: CheckedContinuation<URL, any Error>?
    private var kept: [AnyObject] = []

    init(_ continuation: CheckedContinuation<URL, any Error>) {
        self.continuation = continuation
    }

    /// Hold `objects` until the continuation is resumed, then let go of them.
    func keep(_ objects: AnyObject...) {
        lock.lock()
        if continuation != nil { kept.append(contentsOf: objects) }
        lock.unlock()
    }

    func resume(_ result: Result<URL, any Error>) {
        lock.lock()
        let continuation = self.continuation
        self.continuation = nil
        let kept = self.kept
        self.kept = []
        lock.unlock()
        continuation?.resume(with: result)
        withExtendedLifetime(kept) {}
    }
}

/// The window a browser sign-in shows over: the key window.
private final class PresentationAnchor: NSObject, ASWebAuthenticationPresentationContextProviding {
    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        MainActor.assumeIsolated {
            #if os(macOS)
            NSApp.keyWindow ?? NSApp.windows.first ?? ASPresentationAnchor()
            #else
            UIApplication.shared.connectedScenes
                .compactMap { ($0 as? UIWindowScene)?.keyWindow }
                .first ?? ASPresentationAnchor()
            #endif
        }
    }
}

/// The sign-in AniList and MyAnimeList will use, in the environment for a screen to call.
struct WebSignInKey: EnvironmentKey {
    static let defaultValue: any WebSignIn = SystemWebSignIn()
}

extension EnvironmentValues {
    var webSignIn: any WebSignIn {
        get { self[WebSignInKey.self] }
        set { self[WebSignInKey.self] = newValue }
    }
}
