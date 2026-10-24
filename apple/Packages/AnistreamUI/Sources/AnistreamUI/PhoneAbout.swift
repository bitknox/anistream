// About on iPhone, from Settings: what MacAbout's window says, as a page of the settings stack.
// Nagare, the name, the version, the licence, and the acknowledgements and the website; then
// updates: the daily check's switch, what it found, and the action.

#if os(iOS)
import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

struct PhoneAboutPage: View {
    @Environment(\.openURL) private var openURL
    @Environment(AppStore.self) private var store

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
                Button("Report a bug") { ReportBugAction(store: store, openURL: openURL)() }
                    .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 17))
                    .padding(.top, 10)
                Text("Opens a bug report on GitHub, where you sign in. It carries your versions and the last error, with no addresses, file paths or titles.")
                    .font(.ui(13))
                    .foregroundStyle(.haze)
                    .multilineTextAlignment(.center)
                    .padding(.top, 8)
                PhoneUpdatesCard()
                    .padding(.top, 32)
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
