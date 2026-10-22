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
///
/// On iOS each is readable from the first unlock after a restart until the next: a token is read
/// and renewed while the phone is locked, as it is through an episode's audio in the background,
/// and with the system's default an item is out of reach the moment the screen locks. An item an
/// earlier build kept with that default is kept again with this on its first read.
public struct KeychainSecrets: SecretBackend {
    /// The keyring crate's service name, which the terminal never changes.
    public static let anistream = "anistream"

    public let service: String

    public init(service: String = KeychainSecrets.anistream) {
        self.service = service
    }

    /// The item for `id`. Never with `kSecUseDataProtectionKeychain`: on macOS that is a
    /// different keychain from the login keychain the terminal's items are in.
    private func query(_ id: String) -> [CFString: Any] {
        [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: id,
        ]
    }

    /// Whether this is the phone's or the iPad's keychain, which keeps items by when they can be
    /// read, rather than the Mac's.
    static var phone: Bool {
        #if os(iOS)
        true
        #else
        false
        #endif
    }

    /// What an item is kept with beyond its service and account: on iOS, readable from the first
    /// unlock until the next restart. Nothing on the Mac, whose login keychain the terminal
    /// shares, and whose items stay as the terminal's `keyring` crate makes them.
    static func protection(phone: Bool) -> [CFString: Any] {
        phone ? [kSecAttrAccessible: kSecAttrAccessibleAfterFirstUnlock] : [:]
    }

    /// Whether an item read with `attributes` is to be kept again with `protection(phone:)`: on
    /// iOS, one kept readable only while unlocked, as every item was before.
    static func keptAgain(_ attributes: [CFString: Any], phone: Bool) -> Bool {
        guard phone else { return false }
        let accessible = attributes[kSecAttrAccessible] as? String
        return accessible != (kSecAttrAccessibleAfterFirstUnlock as String)
    }

    public func secret(for id: String) -> String? {
        var query = query(id)
        query[kSecReturnData] = true
        query[kSecReturnAttributes] = true
        query[kSecMatchLimit] = kSecMatchLimitOne
        var result: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
              let item = result as? [CFString: Any],
              let data = item[kSecValueData] as? Data,
              let value = String(data: data, encoding: .utf8)
        else { return nil }
        if Self.keptAgain(item, phone: Self.phone) {
            // Added again rather than updated, so the item is made afresh with the protection.
            if SecItemDelete(self.query(id) as CFDictionary) == errSecSuccess, !add(data, for: id) {
                AppLog.warn("keychain", "could not keep \(id) again for reading while locked")
            }
        }
        return value
    }

    public func setSecret(_ value: String, for id: String) -> Bool {
        let data = Data(value.utf8)
        var changes: [CFString: Any] = [kSecValueData: data]
        changes.merge(Self.protection(phone: Self.phone)) { _, protection in protection }
        let updated = SecItemUpdate(query(id) as CFDictionary, changes as CFDictionary)
        if updated == errSecSuccess { return true }
        guard updated == errSecItemNotFound else { return false }
        return add(data, for: id)
    }

    private func add(_ data: Data, for id: String) -> Bool {
        var item = query(id)
        item[kSecValueData] = data
        item.merge(Self.protection(phone: Self.phone)) { _, protection in protection }
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
