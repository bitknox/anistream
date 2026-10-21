// What the engine asks of the app it runs in.

import AnistreamKit
import Foundation

/// The app's `FfiHost`: links opened, notifications posted and secrets kept, each through what the
/// app hands it, so the store needs nothing of AppKit or UIKit and a test hands it nothing real.
///
/// The engine calls it on its own threads, never the main one, and waits for each answer.
public final class AppHost: FfiHost {
    public let secrets: any SecretBackend
    private let open: @Sendable (URL) -> Bool
    private let post: @Sendable (String, String) -> Void

    /// - Parameters:
    ///   - secrets: where tokens are kept.
    ///   - open: opens a link in the browser or the app for it; false when it could not.
    ///   - notify: posts a notification, a copy of a notice for a user who has switched away.
    public init(
        secrets: any SecretBackend,
        open: @escaping @Sendable (URL) -> Bool = { _ in false },
        notify: @escaping @Sendable (String, String) -> Void = { _, _ in }
    ) {
        self.secrets = secrets
        self.open = open
        self.post = notify
    }

    public func openUrl(url: String) -> Bool {
        guard let url = URL(string: url) else { return false }
        return open(url)
    }

    public func notify(title: String, body: String) {
        post(title, body)
    }

    public func secretGet(id: String) -> String? {
        secrets.secret(for: id)
    }

    public func secretSet(id: String, value: String) -> Bool {
        secrets.setSecret(value, for: id)
    }

    public func secretDelete(id: String) -> Bool {
        secrets.deleteSecret(for: id)
    }
}
