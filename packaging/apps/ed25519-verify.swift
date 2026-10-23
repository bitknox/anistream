// Checks an Ed25519 signature over a file against a public key, both base64, as Sparkle checks
// the Mac app's update: packaging/apps/mac-update.sh runs it against the key the app carries.
//
//   swift packaging/apps/ed25519-verify.swift <public key> <file> <signature>
//
// Exits 0 when the signature is good, 1 when it is not, 2 when the arguments do not read.

import CryptoKit
import Foundation

let arguments = CommandLine.arguments
guard arguments.count == 4,
      let key = Data(base64Encoded: arguments[1]),
      let signature = Data(base64Encoded: arguments[3].trimmingCharacters(in: .whitespacesAndNewlines)),
      let publicKey = try? Curve25519.Signing.PublicKey(rawRepresentation: key),
      let file = try? Data(contentsOf: URL(fileURLWithPath: arguments[2]), options: .mappedIfSafe)
else {
    FileHandle.standardError.write(Data("usage: swift ed25519-verify.swift <public key> <file> <signature>\n".utf8))
    exit(2)
}
exit(publicKey.isValidSignature(signature, for: file) ? 0 : 1)
