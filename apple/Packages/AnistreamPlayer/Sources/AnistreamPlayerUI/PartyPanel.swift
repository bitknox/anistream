// The watch party in the player, from the people button: how the connection stands, who is in
// the room and who is ready, this member's ready switch, the invite link to copy, the server and
// room as text for someone joining with Syncplay itself, and Leave party. A popover on the Mac, a
// sheet on iPhone and iPad, as the player's other menus are; the Windows app's flyout (#179) says
// the same words. Each part keeps its place as the room changes.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

struct PartyPanel: View {
    @Environment(AppStore.self) private var store
    let metrics: PlayerMetrics
    /// Copy invite link was pressed: "Copied" stands in its label for 2 s.
    @State private var copied = false

    var body: some View {
        let party = store.party
        VStack(alignment: .leading, spacing: 0) {
            if let view = party.party {
                header(view)
                members(party.members)
                    .padding(.top, 10)
                Text(party.members.map(PartyWords.readyLine) ?? " ")
                    .font(.system(size: small))
                    .foregroundStyle(.haze)
                    .contentTransition(.opacity)
                    .animation(.quick, value: party.members.map(PartyWords.readyLine))
                    .padding(.top, 8)
                Toggle(
                    L.partyReadyAction,
                    isOn: Binding(get: { view.ready }, set: { party.setReady($0) })
                )
                .toggleStyle(.switch)
                .font(.system(size: metrics.body))
                .padding(.top, 10)
                Divider().padding(.vertical, 12)
                invite(view)
                plain(view)
                    .padding(.top, 12)
                if let motd = view.motd, !motd.isEmpty {
                    Text(motd)
                        .font(.system(size: small))
                        .foregroundStyle(.haze)
                        .fixedSize(horizontal: false, vertical: true)
                        .padding(.top, 10)
                }
                Button {
                    party.leave()
                } label: {
                    Text(party.leaving ? L.partyLeaving : L.partyLeave)
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .controlSize(metrics.touch ? .large : .regular)
                .disabled(party.leaving)
                .padding(.top, 14)
            }
        }
        .foregroundStyle(.silk)
        .padding(metrics.touch ? 20 : 16)
        // A readable column on a phone held sideways and on iPad, where the sheet runs wide.
        .frame(width: metrics.touch ? nil : 300, alignment: .leading)
        .frame(maxWidth: metrics.touch ? 520 : nil, alignment: .leading)
        .frame(maxWidth: metrics.touch ? .infinity : nil)
    }

    private var small: CGFloat { metrics.touch ? 13 : 11 }

    private func header(_ view: PartyView) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(L.partyTitle)
                .font(.system(size: metrics.body + 2, weight: .semibold))
            HStack(spacing: 6) {
                if case .reconnecting = view.connection {
                    ProgressView().controlSize(.mini)
                }
                Text(PartyWords.connection(view))
                    .contentTransition(.opacity)
            }
            .font(.system(size: small))
            .foregroundStyle(.haze)
            .animation(.quick, value: PartyWords.connection(view))
        }
        .accessibilityElement(children: .combine)
    }

    @ViewBuilder
    private func members(_ members: [PartyMember]?) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            if let members {
                ForEach(members, id: \.name) { member in
                    MemberRow(member: member, metrics: metrics)
                }
            } else {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(L.partyWaitingMembers)
                        .foregroundStyle(.haze)
                }
                .font(.system(size: metrics.body))
                .frame(minHeight: metrics.touch ? 44 : 36)
            }
        }
        .animation(.standard, value: members)
    }

    private func invite(_ view: PartyView) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Button {
                guard let link = view.invite else { return }
                PlayerModel.pasteboard(link)
                copied = true
            } label: {
                Label {
                    Text(copied ? "Copied" : L.partyCopyInvite)
                        .contentTransition(.opacity)
                } icon: {
                    Image(systemName: copied ? "checkmark" : "doc.on.doc")
                }
                .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .controlSize(metrics.touch ? .large : .regular)
            .disabled(view.invite == nil)
            .animation(.quick, value: copied)
            .task(id: copied) {
                guard copied else { return }
                try? await Task.sleep(for: .seconds(2))
                copied = false
            }
            Text(view.invite == nil
                 ? L.partyInviteAfterPlay
                 : L.partyInviteDetail)
                .font(.system(size: small))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    /// The server and room as text, for a friend with Syncplay's own app.
    private func plain(_ view: PartyView) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(L.partyOtherApp)
                .font(.system(size: small, weight: .semibold))
            Text(L.partyOtherAppDetail(server: view.server, room: view.room))
                .font(.system(size: small))
                .foregroundStyle(.haze)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

/// A member: ready or not, their name, whether their file is this one, and their state.
private struct MemberRow: View {
    let member: PartyMember
    let metrics: PlayerMetrics

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: member.ready == true ? "checkmark" : "circle")
                .font(.system(size: metrics.body - 2, weight: member.ready == true ? .semibold : .regular))
                .foregroundStyle(member.ready == true ? .silk : .haze)
                .frame(width: 16)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 1) {
                Text(PartyWords.name(member))
                    .font(.system(size: metrics.body))
                if let file = PartyWords.file(member) {
                    Text(file)
                        .font(.system(size: metrics.touch ? 13 : 11))
                        .foregroundStyle(.haze)
                }
            }
            .lineLimit(1)
            Spacer(minLength: 8)
            Text(PartyWords.state(member))
                .font(.system(size: metrics.touch ? 13 : 11, weight: member.ready == true ? .semibold : .regular))
                .foregroundStyle(member.ready == true ? .silk : .haze)
                .contentTransition(.opacity)
        }
        .frame(minHeight: metrics.touch ? 44 : 36)
        .animation(.quick, value: member.ready)
        .accessibilityElement(children: .combine)
    }
}

/// The people button's words: what it opens.
enum PartyButton {
    static func label(inParty: Bool) -> String {
        inParty ? L.partyTitle : L.partyOpen
    }

    static func symbol(inParty: Bool) -> String {
        inParty ? "person.2.fill" : "person.2"
    }
}
