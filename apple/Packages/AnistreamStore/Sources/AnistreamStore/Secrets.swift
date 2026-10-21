// The keychain the engine keeps the trackers' and plugin stores' tokens in, behind the app's host.

import Foundation
import Security
import Synchronization

/// Where secrets are kept, by id: the keychain in the app, memory in a test.
public protocol SecretBackend: Sendable {
    /// The secret kept under `id`, or `nil` when there is none or it could not be read.
    func secret(for id: String) -> String?
    /// Keep `value` under `id` in place of whatever was there. False when it was refused.
    func setSecret(_ value: String, for id: String) -> Bool
    /// Forget what is kept under `id`. True when nothing is kept there now, whether or not
    /// anything was.
    func deleteSecret(for id: String) -> Bool
}

/// The system keychain, as generic passwords under the service `anistream` with the secret's id
/// as the account: the very items the terminal app's `keyring` crate keeps, so that signing in
/// in one app signs in the other.
///
/// On macOS these are in the login keychain, where the terminal's are. An item the terminal made
/// trusts the terminal, so the first read from the app asks the user to allow it.
public struct KeychainSecrets: SecretBackend {
    /// The keyring crate's service name, which the terminal never changes.
    public static let anistream = "anistream"

    public let service: String

    public init(service: String = KeychainSecrets.anistream) {
        self.service = service
    }

    private func query(_ id: String) -> [CFString: Any] {
        [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: id,
        ]
    }

    public func secret(for id: String) -> String? {
        var query = query(id)
        query[kSecReturnData] = true
        query[kSecMatchLimit] = kSecMatchLimitOne
        var result: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
              let data = result as? Data
        else { return nil }
        return String(data: data, encoding: .utf8)
    }

    public func setSecret(_ value: String, for id: String) -> Bool {
        let data = Data(value.utf8)
        let updated = SecItemUpdate(
            query(id) as CFDictionary, [kSecValueData: data] as CFDictionary
        )
        if updated == errSecSuccess { return true }
        guard updated == errSecItemNotFound else { return false }
        var item = query(id)
        item[kSecValueData] = data
        return SecItemAdd(item as CFDictionary, nil) == errSecSuccess
    }

    public func deleteSecret(for id: String) -> Bool {
        let status = SecItemDelete(query(id) as CFDictionary)
        return status == errSecSuccess || status == errSecItemNotFound
    }
}

/// Secrets in memory, for tests: never the keychain.
public final class MemorySecrets: SecretBackend {
    private let items: Mutex<[String: String]>

    public init(_ items: [String: String] = [:]) {
        self.items = Mutex(items)
    }

    /// Everything kept, for a test to look at.
    public var all: [String: String] {
        items.withLock { $0 }
    }

    public func secret(for id: String) -> String? {
        items.withLock { $0[id] }
    }

    public func setSecret(_ value: String, for id: String) -> Bool {
        items.withLock { $0[id] = value }
        return true
    }

    public func deleteSecret(for id: String) -> Bool {
        items.withLock { _ = $0.removeValue(forKey: id) }
        return true
    }
}
