// The words of a watch party, as the form that starts one, the panel in the player and the
// no-source sheet say them: who is ready, how the connection stands, which server is suggested
// and who runs it. Every sentence is a catalogue message (ui-party.ftl); the party itself, and the
// engine's own messages about it, are the engine's.

import AnistreamKit
import AnistreamStrings
import Foundation

public enum PartyWords {
    /// What the panel says of a member, beside their name.
    public static func state(_ member: PartyMember) -> String {
        switch member.ready {
        case true?: L.partyMemberReady
        case false?: L.partyMemberNotReady
        case nil: L.partyMemberNoReadyState
        }
    }

    /// A second line for a member whose episode is not this device's, or nil.
    public static func file(_ member: PartyMember) -> String? {
        if member.you { return nil }
        if member.fileMatches == false { return L.partyMemberOtherFile }
        if member.file == nil { return L.partyMemberNoFile }
        return nil
    }

    /// The member's name, with "(you)" for this device's.
    public static func name(_ member: PartyMember) -> String {
        member.you ? L.partyMemberYou(name: member.name) : member.name
    }

    /// Every member is ready, and there are at least two: when play starts on its own. A member
    /// whose client has no ready state does not hold the room up.
    public static func allReady(_ members: [PartyMember]) -> Bool {
        members.count >= 2 && members.allSatisfy { $0.ready != false }
    }

    /// The line under the members: everyone ready, who the room waits for, or alone.
    public static func readyLine(_ members: [PartyMember]) -> String {
        if members.count < 2 { return L.partyAlone }
        if allReady(members) { return L.partyAllReady }
        let waiting = members.filter { $0.ready == false }
        switch waiting.count {
        case 1 where waiting[0].you: return L.partyWaitYou
        case 1: return L.partyWaitOne(name: waiting[0].name)
        case 2:
            return L.partyWaitTwo(first: waiting[0].you ? L.partyYou : waiting[0].name,
                                  second: waiting[1].you ? L.partyYou : waiting[1].name)
        default: return L.partyWaitMany(count: waiting.count)
        }
    }

    /// How the connection stands, for the panel's header.
    public static func connection(_ party: PartyView) -> String {
        switch party.connection {
        case .reconnecting(let attempt, let of):
            return L.partyConnectionLost(attempt: Int(attempt), of: Int(of))
        case .connected:
            let line = party.encrypted ? L.partyConnectedEncrypted : L.partyConnectedPlain
            return party.latencyMs.map { L.partyConnectedLatency(line: line, ms: Int($0)) } ?? line
        }
    }

    /// The line under the server field: whose the suggested server is, so the viewer knows
    /// before joining.
    public static func serverHint(_ typed: String, form: PartyForm) -> String {
        isSuggested(typed, form: form)
            ? L.partyServerPublic(operator: form.suggestedRunBy)
            : L.partyServerSuggest(server: form.suggestedServer, operator: form.suggestedRunBy)
    }

    /// The line under an invite's room: whose server it is.
    public static func inviteServerHint(_ server: String, form: PartyForm) -> String {
        isSuggested(server, form: form)
            ? serverHint(server, form: form)
            : L.partyServerInvited
    }

    /// Whether `typed` is the suggested public server.
    public static func isSuggested(_ typed: String, form: PartyForm) -> Bool {
        typed.trimmingCharacters(in: .whitespaces).caseInsensitiveCompare(form.suggestedServer) == .orderedSame
    }

    /// What is wrong with a server as typed, or nil when it reads as `host`, `host:port` or
    /// `[v6]:port`. Empty is not wrong, only not yet chosen.
    public static func serverProblem(_ typed: String) -> String? {
        let text = typed.trimmingCharacters(in: .whitespaces)
        if text.isEmpty { return nil }
        let shape = L.partyServerShape
        if text.contains(" ") || text.contains("/") { return shape }
        let host: Substring
        let port: Substring
        if text.hasPrefix("[") {
            guard let close = text.firstIndex(of: "]") else { return shape }
            host = text[text.index(after: text.startIndex)..<close]
            let rest = text[text.index(after: close)...]
            if !rest.isEmpty, rest.first != ":" || rest.count == 1 { return shape }
            port = rest.isEmpty ? "" : rest.dropFirst()
        } else {
            let parts = text.split(separator: ":", omittingEmptySubsequences: false)
            switch parts.count {
            case 1: (host, port) = (parts[0], "")
            case 2:
                if parts[1].isEmpty { return shape }
                (host, port) = (parts[0], parts[1])
            default: return shape
            }
        }
        if host.isEmpty { return shape }
        if !port.isEmpty {
            guard port.allSatisfy(\.isASCII), port.allSatisfy(\.isNumber), let n = UInt16(port), n > 0 else { return shape }
        }
        return nil
    }

    /// A room name is anything but blank; the server takes it as it is.
    public static func roomOK(_ room: String) -> Bool {
        !room.trimmingCharacters(in: .whitespaces).isEmpty
    }

    /// The form's episode line: "Episode 7 of Stardust Wayfarer", or what the player's button
    /// starts.
    public static func episode(_ number: String?, title: String?) -> String {
        guard let number else { return L.partyFormNothing }
        let episode = episodeNumber(number)
        return title.map { L.partyFormEpisodeOf(episode: episode, title: $0) } ?? L.partyFormEpisode(episode: episode)
    }

    /// Where the viewer is while nothing plays: "You're in quiet-harbor on party.example:8999,
    /// with nothing playing."
    public static func place(_ party: PartyView?) -> String {
        party.map { L.partyInRoomIdle(room: $0.room, server: $0.server) } ?? L.partyInRoomIdlePlain
    }

    /// The no-source sheet's title and its explanation, for why the party's episode has no
    /// source here. `reason` is the engine's own message about it; `what` is "Episode 7 of
    /// Stardust Wayfarer".
    public static func noSource(why: NoSourceWhy, reason: Msg, episode: String, what: String) -> (title: String, body: String) {
        let said = reason.text.trimmingCharacters(in: Self.stops)
        switch why {
        case .notAired:
            // The schedule's own words lead: "Episode 6 airs tomorrow at 13:05", or the episode
            // not out yet with when the next one is.
            if case .noticeNotAiredYet = reason {
                return (said, L.partyNosourceUnaired(episode: episodeNumber(episode)))
            }
            return (said, L.partyNosourceUnairedPlain)
        case .noSources:
            return (L.partyNosourceNoneTitle, L.partyNosourceNoneDetail(what: what))
        case .notFound:
            return (L.partyNosourceMissingTitle, L.partyNosourceMissingDetail(why: said, what: what))
        }
    }

    /// How many sources list the episode: "1 source has it".
    public static func found(_ count: Int) -> String {
        L.partySourcesFound(count: count)
    }

    /// A source's second line: "sample-source · 1080p · 1.2 GiB · 40 seeders".
    public static func detail(_ candidate: SourceCandidate) -> String {
        var parts = [candidate.providerId]
        if let quality = candidate.quality { parts.append("\(quality)p") }
        if let size = candidate.size, !size.isEmpty { parts.append(size) }
        if let seeders = candidate.seeders { parts.append(L.sourceReleaseSeeders(count: Int(seeders))) }
        return parts.joined(separator: " · ")
    }

    /// The full stops that end a sentence, in the languages the catalogue has.
    private static let stops = CharacterSet(charactersIn: ".。")

    /// An episode's number as the messages take it: whole. A part-episode such as "7.5" shows
    /// as its whole number.
    static func episodeNumber(_ episode: String) -> Int {
        Int(episode) ?? Int(Double(episode) ?? 0)
    }
}
