// Stats for nerds (player-stats.md): the engine's rows on the player's control background, top
// trailing over the picture. The engine words every row and picks the one amber value; this
// only lays them out, with Copy stats and a close button.

import AnistreamDesign
import AnistreamKit
import SwiftUI
#if os(macOS)
import Darwin
#else
import UIKit
#endif

/// What the panel shows: the rows, with dashes before the file opens, and whether it says it
/// is waiting for the stream.
public struct StatsPanelState: Equatable {
    public let rows: [DetailRow]
    public let waitingLine: Bool
}

/// The panel. It stays up while the controls hide, and fades in and out; values change in
/// place, in monospaced digits, so nothing moves as they tick.
struct StatsPanel: View {
    let model: PlayerModel
    let metrics: PlayerMetrics

    /// 300 pt on the Mac, as on every desktop.
    static let width: CGFloat = 300

    var body: some View {
        if let panel = model.statsPanel {
            VStack(alignment: .leading, spacing: 0) {
                HStack(alignment: .center, spacing: 8) {
                    Text("Stats for nerds")
                        .font(.system(size: metrics.body, weight: .semibold))
                        .frame(maxWidth: .infinity, alignment: .leading)
                    Button { model.showStats(false) } label: {
                        Image(systemName: "xmark")
                            .font(.system(size: 10, weight: .semibold))
                            .frame(width: 22, height: 22)
                            .contentShape(.circle)
                    }
                    .buttonStyle(OverVideoButtonStyle())
                    .accessibilityLabel("Close stats for nerds")
                    .help("Close")
                }
                if panel.waitingLine {
                    Text("Waiting for the stream")
                        .foregroundStyle(.haze)
                        .padding(.top, 2)
                }
                Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 12, verticalSpacing: 5) {
                    ForEach(panel.rows, id: \.label) { row in
                        GridRow {
                            Text(row.label)
                                .foregroundStyle(.haze)
                            Text(Self.wrapping(row.value))
                                .monospacedDigit()
                                .foregroundStyle(row.warn ? Palette.amber : Palette.silk)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        .accessibilityElement(children: .combine)
                    }
                }
                .padding(.top, 10)
                copyButton(waiting: model.statsWaitingSince != nil)
                    .padding(.top, 12)
            }
            .font(.system(size: metrics.touch ? 13 : 12))
            .foregroundStyle(.silk)
            .padding(.horizontal, 14)
            .padding(.top, 10)
            .padding(.bottom, 12)
            .frame(width: Self.width, alignment: .leading)
            // The player's control background, with no edge, shadow or blur of its own.
            .background(.overVideo(0.72), in: .rect(cornerRadius: 12))
            .accessibilityElement(children: .contain)
            .accessibilityLabel("Stats for nerds")
            .transition(.opacity)
        }
    }

    /// `value` as it may wrap in the panel's narrow column: a number stays with its unit and
    /// a size with its "×", and a " · " ends the line it is on rather than starting the next.
    /// Only for showing: Copy stats keeps the engine's plain spaces.
    static func wrapping(_ value: String) -> String {
        let keep = "\u{00A0}"
        return value
            .replacingOccurrences(of: " × ", with: "\(keep)×\(keep)")
            .replacingOccurrences(of: " · ", with: "\(keep)· ")
            .replacingOccurrences(
                of: #"(\d) (fps|Hz|kHz|MB/s|kB/s|GB/s|MB|kB|GB|s\b|threads?)"#, with: "$1\(keep)$2",
                options: .regularExpression)
            .replacingOccurrences(of: #" (\d+-bit)"#, with: "\(keep)$1", options: .regularExpression)
    }

    /// Copy stats, as words, and "Copied" in its place for a moment after.
    private func copyButton(waiting: Bool) -> some View {
        let copied = model.statsCopiedAt != nil
        return Button(copied ? "Copied" : metrics.label("Copy stats", mac: "Copy Stats")) { model.copyStats() }
            .buttonStyle(.plain)
            .fontWeight(.semibold)
            .foregroundStyle(copied || waiting ? Palette.haze : Palette.silk)
            .disabled(waiting || copied)
            .pointingHand(!waiting && !copied)
    }
}

extension StatsAbout {
    /// This app, system and device, for Copy stats: nothing that names the viewer.
    @MainActor public static var here: StatsAbout {
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
        let os = ProcessInfo.processInfo.operatingSystemVersion
        let number = os.patchVersion > 0
            ? "\(os.majorVersion).\(os.minorVersion).\(os.patchVersion)" : "\(os.majorVersion).\(os.minorVersion)"
        #if os(macOS)
        let system = "macOS \(number)"
        #else
        let system = "\(UIDevice.current.systemName) \(number)"
        #endif
        return StatsAbout(appVersion: version, system: system, device: deviceModel)
    }

    /// "Mac15,3", "iPhone17,1": the model, never the device's name.
    private static var deviceModel: String {
        #if os(macOS)
        var size = 0
        guard sysctlbyname("hw.model", nil, &size, nil, 0) == 0, size > 0 else { return "" }
        var bytes = [CChar](repeating: 0, count: size)
        guard sysctlbyname("hw.model", &bytes, &size, nil, 0) == 0 else { return "" }
        return String(decoding: bytes.prefix { $0 != 0 }.map { UInt8(bitPattern: $0) }, as: UTF8.self)
        #else
        var info = utsname()
        uname(&info)
        return withUnsafeBytes(of: &info.machine) { raw in
            String(decoding: raw.prefix { $0 != 0 }, as: UTF8.self)
        }
        #endif
    }
}
