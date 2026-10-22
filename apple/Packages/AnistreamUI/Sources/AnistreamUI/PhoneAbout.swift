// About on iPhone, from Settings: what MacAbout's window says, as a page of the settings stack.
// Nagare, the name, the version, the licence, and the acknowledgements and the website.

#if os(iOS)
import AnistreamDesign
import AnistreamKit
import SwiftUI

/// The app's version as its bundle says it: "1.1.0", and "Version 1.1.0 (1)" with the build.
enum AppVersion {
    static var short: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? ""
    }

    static var long: String {
        let build = Bundle.main.infoDictionary?["CFBundleVersion"] as? String
        return "Version \(short)" + (build.map { " (\($0))" } ?? "")
    }

    /// The website, which About opens.
    static let website = URL(string: "https://anistream.tv")!
}

struct PhoneAboutPage: View {
    @Environment(\.openURL) private var openURL

    var body: some View {
        ScrollView {
            VStack(spacing: 0) {
                Nagare(cell: 5)
                    .accessibilityHidden(true)
                Text("anistream")
                    .font(.ui(22, weight: .bold))
                    .foregroundStyle(.silk)
                    .padding(.top, 20)
                Text(AppVersion.long)
                    .font(.ui(15))
                    .monospacedDigit()
                    .foregroundStyle(.haze)
                    .padding(.top, 4)
                Text("Open source under the MIT licence.")
                    .font(.ui(15))
                    .foregroundStyle(.haze)
                    .multilineTextAlignment(.center)
                    .padding(.top, 12)
                HStack(spacing: 10) {
                    NavigationLink(value: PhoneSettingsPage.acknowledgements) {
                        Text("Acknowledgements")
                    }
                    .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 17))
                    Button("Website") { openURL(AppVersion.website) }
                        .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 17))
                }
                .padding(.top, 28)
            }
            .frame(maxWidth: .infinity)
            .padding(EdgeInsets(top: 48, leading: 16, bottom: 32, trailing: 16))
        }
        .background(.ground)
        .navigationTitle("About")
        .navigationBarTitleDisplayMode(.inline)
    }
}

/// The notices of what the app carries: libmpv and what it is built from, and the subtitle font.
struct PhoneAcknowledgementsPage: View {
    private static let files = ["NOTICES-libmpv", "NOTICES-fonts"]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                ForEach(Self.files, id: \.self) { name in
                    if let text = Self.text(name) {
                        Text(text)
                            .font(.ui(12, design: .monospaced))
                            .foregroundStyle(.haze)
                            .textSelection(.enabled)
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(16)
        }
        .background(.ground)
        .navigationTitle("Acknowledgements")
        .navigationBarTitleDisplayMode(.inline)
    }

    private static func text(_ name: String) -> String? {
        Bundle.main.url(forResource: name, withExtension: "txt").flatMap { try? String(contentsOf: $0, encoding: .utf8) }
    }
}
#endif
