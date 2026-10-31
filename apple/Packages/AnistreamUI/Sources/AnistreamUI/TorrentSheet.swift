// Turning torrents on: what it exposes, the index torrents are searched on, and what protects
// the connection, said before anything starts. The same sheet asks for a proxy later, and edits
// the saved one. A proxy is saved only once the connection check through it passes.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// The turn-on sheet, and the same sheet asking for a proxy once torrents are on, or editing the
/// saved one: the index, what protects the connection, and the proxy's address. No protection
/// asks to be confirmed.
struct TorrentSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    /// Whether this turns torrents on, changes their protection, or edits the saved proxy.
    var mode: TorrentSheetMode = .turnOn
    /// The choice to start on, in place of the file's.
    var start: TorrentProtection?
    @State private var draft: TorrentDraft?
    /// What the check under way is for, saved once it passes.
    @State private var checking: TorrentDraft?
    @State private var confirmingNone = false
    /// The draft as the sheet opened, to tell a change from it.
    @State private var opened: TorrentDraft?

    private var turningOn: Bool { mode == .turnOn }
    private var editing: Bool { mode == .editProxy }

    var body: some View {
        let draft = Binding(get: { self.draft ?? TorrentDraft(store) }, set: { self.draft = $0 })
        let trial = store.providers.trial
        VStack(alignment: .leading, spacing: 0) {
            Text(editing ? L.torrentsSheetEditProxy : turningOn ? L.torrentsSheetTurnOn : L.torrentsSheetProtect)
                .font(.mac(15, weight: .semibold))
                .macLineHeight(20)
                .foregroundStyle(.silk)
            Text(editing
                 ? ProtectionWords.editNote
                 : L.torrentsRisk)
                .font(.mac(13))
                .macLineHeight(18)
                .foregroundStyle(.silk)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 4)

            if editing {
                proxyFields(draft).padding(.top, 18)
                note(ProtectionWords.passwordKept).padding(.top, 8)
            } else {
                // Held still while the check through the proxy runs.
                choices(draft).disabled(trial == .checking)
            }

            if draft.wrappedValue.protection == .proxy {
                // As tall as the most the line can say, from the start, so the sheet and its
                // buttons hold still while the check runs and when it fails.
                ZStack(alignment: .topLeading) {
                    ProxyCheckLine(trial: .said(.unreachable), mode: mode)
                        .hidden()
                        .accessibilityHidden(true)
                    ProxyCheckLine(trial: trial, mode: mode)
                }
                .padding(.top, 18)
            }

            HStack(spacing: 10) {
                Spacer()
                Button(L.actionCancel) { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .keyboardShortcut(.cancelAction)
                Button(turningOn ? L.torrentsTurnOnButton : L.actionSave) {
                    if draft.wrappedValue.protection == TorrentProtection.none {
                        confirmingNone = true
                    } else {
                        send(draft.wrappedValue)
                    }
                }
                .buttonStyle(PrimaryCapsuleStyle(height: 28))
                .keyboardShortcut(.defaultAction)
                .disabled(!draft.wrappedValue.complete || trial == .checking)
            }
            .padding(.top, 20)
        }
        .padding(30)
        .sheetWidth(540)
        .background(.raised)
        .onAppear {
            var first = TorrentDraft(store)
            if let start { first.protection = start }
            self.draft = first
            opened = first
        }
        .modifier(TorrentSaving(checking: checking, draft: self.draft, mode: mode) { dismiss() })
        // Changed from what it opened with, or with a check under way, it stays open on iPad.
        .holdsInput(when: self.draft != opened || checking != nil)
        .confirmationDialog(ProtectionWords.noneTitle, isPresented: $confirmingNone, titleVisibility: .visible) {
            Button(L.torrentsProtectNone, role: .destructive) {
                send(draft.wrappedValue)
            }
            Button(L.actionCancel, role: .cancel) {}
        } message: {
            Text(ProtectionWords.noneMessage)
        }
    }

    /// The index when turning on, then what protects the connection, the proxy's fields under
    /// its choice, and what the check does.
    @ViewBuilder private func choices(_ draft: Binding<TorrentDraft>) -> some View {
        if turningOn {
            label(L.torrentsIndex).padding(.top, 18)
            field(L.torrentsIndexHint, text: draft.index).padding(.top, 6)
            note(L.torrentsIndexDetail + " " + L.legalHostsNoContent).padding(.top, 6)
        }

        label(L.torrentsProtectWith).padding(.top, 18)
        VStack(alignment: .leading, spacing: 10) {
            ForEach(ProtectionWords.order, id: \.self) { choice in
                Button { draft.wrappedValue.protection = choice } label: {
                    self.choice(ProtectionWords.label(choice), ProtectionWords.detail(choice),
                                on: draft.wrappedValue.protection == choice)
                }
                .buttonStyle(.plain)
                if choice == .proxy, draft.wrappedValue.protection == .proxy {
                    proxyFields(draft).padding(.leading, 26)
                }
            }
        }
        .padding(.top, 8)

        note(ProtectionWords.checkNote).padding(.top, 18)
    }

    /// The proxy's address and sign-in, held still while the check through them runs.
    private func proxyFields(_ draft: Binding<TorrentDraft>) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                labelled(L.torrentsProxyHost) { field("proxy.example", text: draft.host) }
                labelled(L.torrentsProxyPort) { field("1080", text: draft.port) }.frame(width: 88)
            }
            HStack(spacing: 12) {
                labelled(L.torrentsProxyUser) { field("", text: draft.user) }
                labelled(L.torrentsProxyPassword) { field("", text: draft.password, secure: true) }
            }
        }
        .disabled(store.providers.trial == .checking)
    }

    private func send(_ draft: TorrentDraft) {
        if store.save(draft, mode) {
            dismiss()
        } else {
            checking = draft
        }
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

    @ViewBuilder
    private func field(_ prompt: String, text: Binding<String>, secure: Bool = false) -> some View {
        Group {
            if secure {
                SecureField("", text: text, prompt: Text(prompt))
            } else {
                TextField("", text: text, prompt: Text(prompt))
            }
        }
        .textFieldStyle(.plain)
        .font(.mac(13))
        .padding(.horizontal, 12)
        .frame(height: 30)
        .background(.ground, in: .rect(cornerRadius: 8))
        .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
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
            Spacer(minLength: 0)
        }
        .contentShape(.rect)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}
