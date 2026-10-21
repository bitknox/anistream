// Turning torrents on: what it exposes, the index torrents are searched on, and what protects
// the connection, said before anything starts.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// The turn-on sheet. The engine can turn torrents on today, under the protection the config
/// file names; choosing the protection here waits on E8's `SetTorrentProtection`, and the index
/// address on E9's `rss_url`, so those are drawn as the board has them and disabled.
struct TorrentSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let mode = store.settings.value(.vpnMode) ?? "socks5"
        VStack(alignment: .leading, spacing: 0) {
            Text("Turn on torrents")
                .font(.mac(15, weight: .semibold))
                .macLineHeight(20)
                .foregroundStyle(.silk)
            Text("Other people can see your IP address while you download and share a torrent. Choose what protects it before torrents start.")
                .font(.mac(13))
                .macLineHeight(18)
                .foregroundStyle(.silk)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 4)

            label("Torrent index").padding(.top, 18)
            field(prompt: "https://…/?q={query}").padding(.top, 6)
                .help("Comes with engine step E9")
            note("The RSS search address anistream asks for torrents. anistream ships none.").padding(.top, 6)

            label("Protect with").padding(.top, 18)
            VStack(alignment: .leading, spacing: 10) {
                choice("SOCKS5 proxy", "Torrent traffic goes through a proxy you run or rent.", on: mode == "socks5")
                if mode == "socks5" {
                    VStack(alignment: .leading, spacing: 10) {
                        HStack(spacing: 12) {
                            labelled("Host") { field(prompt: "") }
                            labelled("Port") { field(prompt: "") }.frame(width: 88)
                        }
                        HStack(spacing: 12) {
                            labelled("User name") { field(prompt: "") }
                            labelled("Password") { field(prompt: "") }
                        }
                    }
                    .padding(.leading, 26)
                }
                choice("VPN on this device", "Uses the VPN already running on this \(Device.name).", on: mode == "external")
                choice("No protection", "Your address is visible to everyone in the torrent.", on: mode == "none")
            }
            .padding(.top, 8)
            .disabled(true)
            .help("Comes with engine step E8. The config file’s choice is shown.")

            note("anistream checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and pauses torrents if that check fails. Your VPN or proxy is what protects you.")
                .padding(.top, 18)

            HStack(spacing: 10) {
                Spacer()
                Button("Cancel") { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .keyboardShortcut(.cancelAction)
                Button("Turn On") {
                    store.setSwitch(.torrents, on: true)
                    dismiss()
                }
                .buttonStyle(PrimaryCapsuleStyle(height: 28))
                .keyboardShortcut(.defaultAction)
            }
            .padding(.top, 20)
        }
        .padding(30)
        .sheetWidth(540)
        .background(.raised)
    }

    private func label(_ text: String) -> some View {
        Text(text).font(.mac(12)).macLineHeight(16).foregroundStyle(.haze)
    }

    private func note(_ text: String) -> some View {
        Text(text)
            .font(.mac(12))
            .macLineHeight(17)
            .foregroundStyle(.haze)
            .fixedSize(horizontal: false, vertical: true)
    }

    private func field(prompt: String) -> some View {
        TextField("", text: .constant(""), prompt: Text(prompt))
            .textFieldStyle(.plain)
            .font(.mac(13))
            .padding(.horizontal, 12)
            .frame(height: 30)
            .background(.ground, in: .rect(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
            .disabled(true)
    }

    private func labelled(_ title: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            label(title)
            content()
        }
    }

    private func choice(_ title: String, _ detail: String, on: Bool) -> some View {
        HStack(alignment: .top, spacing: 10) {
            ZStack {
                Circle().fill(on ? .amber : .ground)
                Circle().strokeBorder(on ? .clear : .haze, lineWidth: 1.5)
                if on { Circle().fill(.ground).frame(width: 6, height: 6) }
            }
            .frame(width: 16, height: 16)
            .padding(.top, 1)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.mac(13)).foregroundStyle(.silk)
                Text(detail).font(.mac(12)).foregroundStyle(.haze)
            }
            .macLineHeight(16)
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}
