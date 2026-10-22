// Signing in to the trackers that do not go through the browser, the codes Simkl and Trakt ask
// for, and settling what a library pull found different. Also the browser sign-in AniList and
// MyAnimeList use, behind a protocol, which the engine asks for with `Event::Authorize`.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
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
                Text("Sign in to Kitsu")
                    .font(.mac(15, weight: .semibold))
                    .foregroundStyle(.silk)
                Text("Your password goes to Kitsu once, for a token kept on this \(Device.name).")
                    .font(.mac(13))
                    .foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
            }
            VStack(alignment: .leading, spacing: 10) {
                TextField("Email or user name", text: $username)
                    .textContentType(.username)
                    #if os(iOS)
                    .textInputAutocapitalization(.never)
                    #endif
                    .autocorrectionDisabled()
                SecureField("Password", text: $password)
                    .textContentType(.password)
                    .onSubmit(signIn)
            }
            .textFieldStyle(SheetFieldStyle())
            HStack(spacing: 10) {
                Spacer()
                if working { ProgressView().controlSize(.small) }
                Button("Cancel") { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .keyboardShortcut(.cancelAction)
                Button("Sign In", action: signIn)
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
                    .disabled(username.isEmpty || password.isEmpty || working)
            }
        }
        .padding(24)
        .sheetWidth(420)
        .background(.raised)
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

/// A code to type on another device, for Simkl or Trakt: the code large, where to type it, and
/// a way there. It stays until the engine says the sign-in ended.
struct DeviceCodeSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.openURL) private var openURL
    @Environment(\.dismiss) private var dismiss
    let prompt: DeviceCodePrompt

    var body: some View {
        let name = AccountsModel.name(prompt.tracker)
        VStack(alignment: .leading, spacing: 16) {
            Text("Sign in to \(name)")
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            Text("Go to \(host) and enter this code. anistream signs in once you have.")
                .font(.mac(13))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
            Text(prompt.code)
                .font(.mac(34, weight: .semibold, design: .monospaced))
                .kerning(4)
                .foregroundStyle(.silk)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 14)
                .background(.ground, in: .rect(cornerRadius: 10))
                .accessibilityLabel("Code \(prompt.code.map(String.init).joined(separator: " "))")
            // In a row where it fits, as on the Mac; on an iPhone the buttons go under the wait,
            // so "Open auth.trakt.tv" is never cut short.
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 10) {
                    waiting(name)
                    Spacer()
                    buttons
                }
                VStack(alignment: .leading, spacing: 12) {
                    waiting(name)
                    HStack(spacing: 10) {
                        Spacer()
                        buttons
                    }
                }
            }
        }
        .padding(24)
        .sheetWidth(420)
        .background(.raised)
    }

    private var host: String {
        URL(string: prompt.url)?.host ?? prompt.url
    }

    private func waiting(_ name: String) -> some View {
        HStack(spacing: 10) {
            ProgressView().controlSize(.small)
            Text("Waiting for \(name)")
                .font(.mac(12))
                .foregroundStyle(.haze)
        }
    }

    @ViewBuilder private var buttons: some View {
        Button("Close") { dismiss() }
            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            .keyboardShortcut(.cancelAction)
        Button("Open \(host)") {
            if let url = URL(string: prompt.url) { openURL(url) }
        }
        .buttonStyle(PrimaryCapsuleStyle(height: 28))
        .keyboardShortcut(.defaultAction)
        .fixedSize()
    }
}

/// What a library pull found different from here: each title, the field, and which side to keep.
struct ConflictsSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Different on AniList")
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            Text("These changed on AniList and here since the last sync. Keep one side of each.")
                .font(.mac(13))
                .foregroundStyle(.haze)
            ScrollView {
                VStack(spacing: 0) {
                    ForEach(Array(store.accounts.conflicts.enumerated()), id: \.offset) { index, row in
                        if index > 0 { Rectangle().fill(.faintRule).frame(height: 1) }
                        HStack(spacing: 12) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(row.title).font(.mac(13, weight: .medium)).foregroundStyle(.silk)
                                Text("\(row.field.firstCapitalized): \(row.local) here, \(row.remote) on AniList")
                                    .font(.mac(12))
                                    .foregroundStyle(.haze)
                            }
                            Spacer()
                            Button("Keep This \(Device.name)’s") { store.resolveConflict(row.anilistId, keepLocal: true) }
                            Button("Keep AniList’s") { store.resolveConflict(row.anilistId, keepLocal: false) }
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
                Button("Done") { dismiss() }
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
