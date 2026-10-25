// The words of a watch party, as the form that starts one, the panel in the player and the
// no-source sheet say them: who is ready, how the connection stands, which server is suggested
// and who runs it. The same words as the Windows app's (Parties.cs); the party itself is the
// engine's.

import AnistreamKit
import Foundation

public enum PartyWords {
    /// What the panel says of a member, beside their name.
    public static func state(_ member: PartyMember) -> String {
        switch member.ready {
        case true?: "Ready"
        case false?: "Not ready"
        case nil: "No ready state"
        }
    }

    /// A second line for a member whose episode is not this device's, or nil.
    public static func file(_ member: PartyMember) -> String? {
        if member.you { return nil }
        if member.fileMatches == false { return "Playing a different file" }
        if member.file == nil { return "Nothing open yet" }
        return nil
    }

    /// The member's name, with "(you)" for this device's.
    public static func name(_ member: PartyMember) -> String {
        member.you ? "\(member.name) (you)" : member.name
    }

    /// Every member is ready, and there are at least two: when play starts on its own. A member
    /// whose client has no ready state does not hold the room up.
    public static func allReady(_ members: [PartyMember]) -> Bool {
        members.count >= 2 && members.allSatisfy { $0.ready != false }
    }

    /// The line under the members: everyone ready, who the room waits for, or alone.
    public static func readyLine(_ members: [PartyMember]) -> String {
        if members.count < 2 { return "Nobody else is here yet. Send the invite link." }
        if allReady(members) { return "Everyone is ready" }
        let waiting = members.filter { $0.ready == false }.map { $0.you ? "you" : $0.name }
        switch waiting.count {
        case 1 where waiting[0] == "you": return "Play starts once you're ready"
        case 1: return "Play starts once \(waiting[0]) is ready"
        case 2: return "Play starts once \(waiting[0]) and \(waiting[1]) are ready"
        default: return "Play starts once \(waiting.count) more are ready"
        }
    }

    /// How the connection stands, for the panel's header.
    public static func connection(_ party: PartyView) -> String {
        switch party.connection {
        case .reconnecting(let attempt, let of):
            return "Connection lost. Trying again, \(attempt) of \(of)"
        case .connected:
            let line = party.encrypted ? "Connected, encrypted" : "Connected, not encrypted"
            return party.latencyMs.map { "\(line) · \($0) ms" } ?? line
        }
    }

    /// The line under the server field: whose the suggested server is, so the viewer knows
    /// before joining.
    public static func serverHint(_ typed: String, form: PartyForm) -> String {
        isSuggested(typed, form: form)
            ? "A public server run by \(form.suggestedRunBy), not by anistream."
            : "No server of your own? \(form.suggestedServer) is a public one, run by \(form.suggestedRunBy)."
    }

    /// The line under an invite's room: whose server it is.
    public static func inviteServerHint(_ server: String, form: PartyForm) -> String {
        isSuggested(server, form: form)
            ? serverHint(server, form: form)
            : "A server the person who invited you chose. anistream doesn't run it."
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
        let shape = "Write the server as a host and a port, like party.example:8999."
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
        guard let number else { return "What's playing now plays with the room." }
        return title.map { "Episode \(number) of \($0)" } ?? "Episode \(number)"
    }

    /// Where the viewer is while nothing plays: "You're in quiet-harbor on party.example:8999,
    /// with nothing playing."
    public static func place(_ party: PartyView?) -> String {
        party.map { "You're in \($0.room) on \($0.server), with nothing playing." }
            ?? "You're in the room, with nothing playing."
    }

    /// The no-source sheet's title and its explanation, for why the party's episode has no
    /// source here. `what` is "Episode 7 of Stardust Wayfarer".
    public static func noSource(why: NoSourceWhy, reason: String, episode: String, what: String) -> (title: String, body: String) {
        switch why {
        case .notAired:
            // The schedule's own words lead: "Episode 6 airs Tuesday at 13:05", or the episode
            // not out yet with when the next one is.
            let said = sentence(reason.replacing(/\ (\d{1,2}:\d{2})\b/) { " at \($0.output.1)" })
            if let split = said.range(of: "; ") {
                let next = sentence(String(said[split.upperBound...]))
                return (String(said[..<split.lowerBound]), "\(next). Your sources won't have episode \(episode) before it's out.")
            }
            return (said, "Your sources won't have it before then. It plays with the room once one of them does.")
        case .noSources:
            return ("You have no sources yet", "Sources come from plugins, which Providers adds. \(what) plays with the room once one of them has it.")
        case .notFound:
            let why = sentence(reason).trimmingCharacters(in: CharacterSet(charactersIn: "."))
            return ("None of your sources has this episode", "\(why). \(what) plays with the room once you choose a source that has it.")
        }
    }

    /// How many sources list the episode: "1 source has it".
    public static func found(_ count: Int) -> String {
        count == 1 ? "1 source has it" : "\(count) sources have it"
    }

    /// A source's second line: "sample-source · 1080p · 1.2 GiB · 40 seeders".
    public static func detail(_ candidate: SourceCandidate) -> String {
        var parts = [candidate.providerId]
        if let quality = candidate.quality { parts.append("\(quality)p") }
        if let size = candidate.size, !size.isEmpty { parts.append(size) }
        if let seeders = candidate.seeders { parts.append("\(seeders) seeders") }
        return parts.joined(separator: " · ")
    }

    /// The engine's words as a sentence: each part it joins with a dash its own, capitalised.
    static func sentence(_ text: String) -> String {
        text.components(separatedBy: " — ")
            .map { part in
                let part = part.trimmingCharacters(in: .whitespaces)
                return part.prefix(1).uppercased() + part.dropFirst()
            }
            .joined(separator: ". ")
    }
}
