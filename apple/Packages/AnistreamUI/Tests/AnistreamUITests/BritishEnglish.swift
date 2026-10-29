import AnistreamKit
import Foundation
import Testing

/// Runs a test or a suite with dates and numbers written as in Britain, so what it expects reads
/// the same whatever region the machine is set to.
struct BritishEnglish: TestTrait, SuiteTrait, TestScoping {
    var isRecursive: Bool { true }

    func provideScope(for test: Test, testCase: Test.Case?, performing function: @Sendable () async throws -> Void) async throws {
        try await AirTime.$locale.withValue(Locale(identifier: "en_GB")) {
            try await function()
        }
    }
}

extension Trait where Self == BritishEnglish {
    static var britishEnglish: Self { BritishEnglish() }
}
