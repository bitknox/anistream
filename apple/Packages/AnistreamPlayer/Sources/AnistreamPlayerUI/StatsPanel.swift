// Stats for nerds (player-stats.md): the engine's rows on the player's control background, top
// trailing over the picture. The engine words every row and picks the one amber value; this
// only lays them out, with Copy stats and a close button.

import AnistreamDesign
import AnistreamKit
import AnistreamStrings
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
/// place, in monospaced digits, so nothing moves as they tick. Offered less height than its
/// rows need, on an iPhone held sideways, the rows scroll between the title and Copy stats.
struct StatsPanel: View {
    let model: PlayerModel
    let metrics: PlayerMetrics
    var width: CGFloat = StatsPanel.width
    /// On a phone held sideways, too short for the panel as the Mac has it: Copy stats moves up
    /// beside the close button, and the rows sit closer, so the panel ends above the subtitles.
    var compact = false

    /// Wide enough that the usual values ("1920 × 1080 · HEVC 10-bit · 23.976 fps") keep to one
    /// line; an unusual one wraps under its value column.
    static let width: CGFloat = 400

    var body: some View {
        if let panel = model.statsPanel {
            VStack(alignment: .leading, spacing: 0) {
                HStack(alignment: .center, spacing: 8) {
                    Text(L.playerStats)
                        .font(.system(size: metrics.body, weight: .semibold))
                        .frame(maxWidth: .infinity, alignment: .leading)
                    if compact { copyButton(waiting: model.statsWaitingSince != nil) }
                    Button { model.showStats(false) } label: {
                        Image(systemName: "xmark")
                            .font(.system(size: metrics.touch ? 13 : 10, weight: .semibold))
                            .frame(width: metrics.touch ? 32 : 22, height: metrics.touch ? 32 : 22)
                            .contentShape(.circle)
                    }
                    .buttonStyle(OverVideoButtonStyle())
                    .accessibilityLabel(L.playerStatsClose)
                    .help(L.actionClose)
                }
                if panel.waitingLine {
                    Text(L.playerStatsWaiting)
                        .foregroundStyle(.haze)
                        .padding(.top, 2)
                }
                ViewThatFits(in: .vertical) {
                    rows(panel.rows)
                    MenuScroll { rows(panel.rows) }
                }
                .padding(.top, compact ? 6 : 10)
                if !compact {
                    copyButton(waiting: model.statsWaitingSince != nil)
                        .padding(.top, 10)
                }
            }
            .font(.system(size: metrics.touch && !compact ? 13 : 12))
            .foregroundStyle(.silk)
            .padding(.horizontal, 14)
            .padding(.top, 10)
            .padding(.bottom, 12)
            .frame(width: width, alignment: .leading)
            // The player's menus' slate, opaque, so no frame ever shows through.
            .background(.statsFill, in: .rect(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Palette.glassEdge, lineWidth: 1))
            .shadow(color: Color(rgb: 0x0A0C18, opacity: 0.4), radius: 12, y: 8)
            .accessibilityElement(children: .contain)
            .accessibilityLabel(L.playerStats)
            .transition(.opacity)
        }
    }

    /// Label and value, the labels' column as wide as its longest label and the values taking
    /// the rest, where a long one wraps under itself.
    private func rows(_ rows: [DetailRow]) -> some View {
        Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 14, verticalSpacing: compact ? 3 : metrics.touch ? 4 : 5) {
            ForEach(rows, id: \.label) { row in
                GridRow {
                    Text(row.label)
                        .foregroundStyle(.haze)
                        .lineLimit(1)
                        .fixedSize()
                    Self.value(row)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .accessibilityElement(children: .combine)
            }
        }
    }

    /// A row's value: monospaced digits, so a figure ticking each second keeps its width, in
    /// amber only when the engine says it is the problem now.
    static func value(_ row: DetailRow) -> some View {
        Text(wrapping(row.value))
            .monospacedDigit()
            .foregroundStyle(row.warn ? Palette.amber : Palette.silk)
            .fixedSize(horizontal: false, vertical: true)
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

    /// Copy stats, a raised pill as the player's other text buttons are, and "Copied" in its
    /// place for a moment after.
    @ViewBuilder
    private func copyButton(waiting: Bool) -> some View {
        if model.statsCopiedAt != nil {
            Text(L.actionCopied)
                .font(.system(size: metrics.body))
                .foregroundStyle(.haze)
                // Where the pill's word was.
                .padding(.horizontal, metrics.touch ? 16 : 12)
                .frame(height: metrics.button)
        } else {
            Button(L.playerStatsCopy) { model.copyStats() }
                .buttonStyle(PillButtonStyle(kind: .secondary, metrics: metrics))
                .disabled(waiting)
                .opacity(waiting ? 0.4 : 1)
        }
    }
}

/// Stats for nerds' ground: the slate of the player's menus as it lands on the ground, opaque,
/// where their 72% let a bright frame through behind the dim labels; Raised under Reduce
/// Transparency.
struct StatsFill: ShapeStyle {
    func resolve(in environment: EnvironmentValues) -> Color {
        environment.accessibilityReduceTransparency ? Palette.raised : Color(rgb: 0x272B42)
    }
}

extension ShapeStyle where Self == StatsFill {
    static var statsFill: StatsFill { StatsFill() }
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
