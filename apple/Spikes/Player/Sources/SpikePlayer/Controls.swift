import SwiftUI

/// The surface with native controls over it, IINA-style: a floating bar that shows on hover
/// or touch and while paused, and hides itself after a few seconds.
public struct PlayerScreen: View {
    let player: Player
    @State private var visible = true
    @State private var hideTask: Task<Void, Never>?

    public init(player: Player) { self.player = player }

    public var body: some View {
        ZStack(alignment: .bottom) {
            PlayerSurface(player: player)
                .ignoresSafeArea()
            if visible || player.paused {
                ControlsBar(player: player)
                    .padding(.horizontal, 24)
                    .padding(.bottom, 20)
                    .transition(.opacity.combined(with: .move(edge: .bottom)))
            }
        }
        .background(.black)
        #if os(macOS)
        .onContinuousHover { phase in
            if case .active = phase { reveal() }
        }
        #else
        .onTapGesture { reveal() }
        #endif
        .onAppear { reveal() }
    }

    private func reveal() {
        withAnimation(.easeOut(duration: 0.2)) { visible = true }
        hideTask?.cancel()
        hideTask = Task {
            try? await Task.sleep(for: .seconds(3))
            guard !Task.isCancelled else { return }
            withAnimation(.easeIn(duration: 0.3)) { visible = false }
        }
    }
}

struct ControlsBar: View {
    let player: Player
    @State private var scrub: Double?

    var body: some View {
        HStack(spacing: 14) {
            Button {
                player.togglePause()
            } label: {
                Image(systemName: player.paused ? "play.fill" : "pause.fill")
                    .font(.title2)
                    .frame(width: 28)
            }
            .buttonStyle(.plain)
            .keyboardShortcut(.space, modifiers: [])

            Text(Self.clock(scrub ?? player.timePos))
                .monospacedDigit()
                .foregroundStyle(.secondary)

            Slider(
                value: Binding(
                    get: { scrub ?? player.timePos },
                    set: { scrub = $0 }),
                in: 0...max(player.duration, 1)
            ) { editing in
                if !editing, let target = scrub {
                    player.seek(to: target)
                    scrub = nil
                }
            }

            Text(Self.clock(player.duration))
                .monospacedDigit()
                .foregroundStyle(.secondary)

            Menu {
                Picker("Upscaling", selection: Binding(get: { player.look }, set: { player.setLook($0) })) {
                    ForEach(Look.allCases) { Text($0.label).tag($0) }
                }
                .pickerStyle(.inline)
                Text(player.chain.label)
            } label: {
                Image(systemName: "sparkles.tv")
            }
            .menuIndicator(.hidden)
            .fixedSize()

            let subtitles = player.tracks.filter { $0.type == "sub" }
            if !subtitles.isEmpty {
                Menu {
                    Button("Off") { player.selectSubtitle(nil) }
                    ForEach(subtitles) { track in
                        Button {
                            player.selectSubtitle(track.id)
                        } label: {
                            if track.selected { Label(track.name, systemImage: "checkmark") } else { Text(track.name) }
                        }
                    }
                } label: {
                    Image(systemName: "captions.bubble")
                }
                .menuIndicator(.hidden)
                .fixedSize()
            }
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 12)
        .frame(maxWidth: 760)
        .glassEffect(.regular, in: .capsule)
    }

    static func clock(_ seconds: Double) -> String {
        guard seconds.isFinite else { return "0:00" }
        let s = Int(seconds.rounded(.down))
        return s >= 3600
            ? String(format: "%d:%02d:%02d", s / 3600, s / 60 % 60, s % 60)
            : String(format: "%d:%02d", s / 60, s % 60)
    }
}
