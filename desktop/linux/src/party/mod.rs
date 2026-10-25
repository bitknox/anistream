//! Watch parties, the window's side: the dialog that starts one or answers an invite link
//! ([`form`]), the dialog for an episode none of the sources has ([`source`]), the panel on the
//! player's people button ([`panel`]), and the words all three share: who is ready, how the
//! connection stands, which server is suggested and who runs it. The party itself is the
//! engine's, in `docs/party-protocol.md`.

use std::rc::Rc;

use adw::prelude::*;
use anistream_model::{
    command::Command,
    party::{PartyConnection, PartyForm, PartyInvite, PartyMember, PartyView},
};

use crate::window::{Ctx, Shell};

pub mod form;
pub mod panel;
pub mod source;

/// What the panel says of a member, beside their name.
pub fn member_state(member: &PartyMember) -> &'static str {
    match member.ready {
        Some(true) => "Ready",
        Some(false) => "Not ready",
        None => "No ready state",
    }
}

/// A second line for a member whose episode is not this device's.
pub fn member_file(member: &PartyMember) -> Option<&'static str> {
    if member.you {
        return None;
    }
    if member.file_matches == Some(false) {
        Some("Playing a different file")
    } else if member.file.is_none() {
        Some("Nothing open yet")
    } else {
        None
    }
}

/// The member's name, with "(you)" for this device's.
pub fn member_name(member: &PartyMember) -> String {
    if member.you { format!("{} (you)", member.name) } else { member.name.clone() }
}

/// Every member is ready, and there are at least two: when play starts on its own. A member
/// whose client has no ready state does not hold the room up.
pub fn all_ready(members: &[PartyMember]) -> bool {
    members.len() >= 2 && members.iter().all(|m| m.ready != Some(false))
}

/// The line under the members: everyone ready, who the room waits for, or nobody else yet.
pub fn ready_line(members: &[PartyMember]) -> String {
    if members.len() < 2 {
        return "Nobody else is here yet. Send the invite link.".into();
    }
    if all_ready(members) {
        return "Everyone is ready".into();
    }
    let waiting: Vec<&str> = members
        .iter()
        .filter(|m| m.ready == Some(false))
        .map(|m| if m.you { "you" } else { m.name.as_str() })
        .collect();
    match waiting.as_slice() {
        ["you"] => "Play starts once you're ready".into(),
        [one] => format!("Play starts once {one} is ready"),
        [one, two] => format!("Play starts once {one} and {two} are ready"),
        more => format!("Play starts once {} more are ready", more.len()),
    }
}

/// How the connection stands, for the panel's header.
pub fn connection_line(party: &PartyView) -> String {
    match party.connection {
        PartyConnection::Reconnecting { attempt, of } => {
            format!("Connection lost. Trying again, {attempt} of {of}")
        }
        PartyConnection::Connected => {
            let how = if party.encrypted {
                "Connected, encrypted"
            } else {
                "Connected, not encrypted"
            };
            match party.latency_ms {
                Some(ms) => format!("{how} · {ms} ms"),
                None => how.into(),
            }
        }
    }
}

/// Whether `typed` is the suggested server.
pub fn is_suggested(typed: &str, form: &PartyForm) -> bool {
    typed.trim().eq_ignore_ascii_case(&form.suggested_server)
}

/// The line under the server field: whose the suggested server is, so the viewer knows before
/// joining.
pub fn server_hint(typed: &str, form: &PartyForm) -> String {
    if is_suggested(typed, form) {
        format!("A public server run by {}, not by anistream.", form.suggested_run_by)
    } else {
        format!(
            "No server of your own? {} is a public one, run by {}.",
            form.suggested_server, form.suggested_run_by
        )
    }
}

/// What is wrong with a server as typed, when it does not read as `host`, `host:port` or
/// `[v6]:port`. Empty is not wrong, only not chosen yet.
pub fn server_problem(typed: &str) -> Option<&'static str> {
    const SHAPE: &str = "Write the server as a host and a port, like party.example:8999.";
    let text = typed.trim();
    if text.is_empty() {
        return None;
    }
    if text.contains([' ', '/']) {
        return Some(SHAPE);
    }
    let (host, port) = if let Some(rest) = text.strip_prefix('[') {
        let Some((host, rest)) = rest.split_once(']') else { return Some(SHAPE) };
        match rest.strip_prefix(':') {
            Some("") => return Some(SHAPE),
            Some(port) => (host, port),
            None if rest.is_empty() => (host, ""),
            None => return Some(SHAPE),
        }
    } else {
        match text.split(':').collect::<Vec<_>>().as_slice() {
            [host] => (*host, ""),
            [_, ""] => return Some(SHAPE),
            [host, port] => (*host, *port),
            _ => return Some(SHAPE),
        }
    };
    if host.is_empty() {
        return Some(SHAPE);
    }
    let port_ok = port.is_empty()
        || (port.bytes().all(|b| b.is_ascii_digit())
            && port.parse::<u16>().is_ok_and(|n| n > 0));
    (!port_ok).then_some(SHAPE)
}

/// "Episode 7 of Stardust Wayfarer", or "Episode 7" before the title is known.
pub fn episode_of(ctx: &Ctx, id: anistream_core::ids::AnilistId, episode: &str) -> String {
    match ctx.hub.entry(id) {
        Some(entry) => format!("Episode {episode} of {}", entry.title),
        None => format!("Episode {episode}"),
    }
}

/// What the window says as a party starts and ends, and the dialog an episode without a
/// source asks for. Over the player the engine says its own notices, and while the form is
/// joining it says why a party ended itself.
pub fn heard(shell: &Rc<Shell>, event: &anistream_model::event::Event) {
    use anistream_model::event::Event;
    let ctx = &shell.ctx;
    match event {
        Event::PartyJoined(_) if !shell.in_party.replace(true) => {
            if shell.is_player() {
                ctx.toast("You're in the party. The people button has the invite link.");
            } else {
                ctx.toast(
                    "You're in the party. The people button in the player has the invite link.",
                );
            }
        }
        Event::PartyLeft { reason } => {
            if shell.in_party.replace(false) && !form::joining() && !shell.is_player() {
                match reason {
                    Some(why) => ctx.alert(&crate::catalog::shown(why)),
                    None => {
                        ctx.toast("You left the party. What's playing goes on, just for you.")
                    }
                }
            }
        }
        Event::PartyNoSource { id, episode, why, reason } => {
            source::open(ctx, *id, episode, *why, reason);
        }
        _ => {}
    }
}

/// An `anistream://party` link: its title's page, and the dialog that asks Join.
pub fn open_invite(shell: &Rc<Shell>, link: &str) {
    let ctx = &shell.ctx;
    let Some(invite) = PartyInvite::parse(link) else {
        ctx.toast("That party link is missing its server, room or episode");
        return;
    };
    let same = ctx
        .hub
        .known
        .borrow()
        .party
        .as_ref()
        .is_some_and(|p| p.server == invite.server && p.room == invite.room);
    if same {
        ctx.toast("You're already in this party");
        return;
    }
    if !shell.is_player() {
        shell.open_title(invite.id);
    }
    form::open(ctx, form::For::Invite(invite));
}

/// Closing the window leaves the party: asked first. Says whether it asked, so the close waits.
pub fn ask_before_closing(shell: &Rc<Shell>) -> bool {
    let Some(room) = shell.ctx.hub.known.borrow().party.as_ref().map(|p| p.room.clone()) else {
        return false;
    };
    let dialog = adw::AlertDialog::new(
        Some("Leave the Party?"),
        Some(&format!(
            "Closing anistream leaves {room}. Everyone else carries on without you."
        )),
    );
    dialog.add_responses(&[("stay", "Stay"), ("leave", "Leave and Close")]);
    dialog.set_response_appearance("leave", adw::ResponseAppearance::Destructive);
    // Stay is the default: Return and Escape both keep the party.
    dialog.set_default_response(Some("stay"));
    dialog.set_close_response("stay");
    let weak = Rc::downgrade(shell);
    dialog.connect_response(None, move |_, response| {
        let Some(shell) = weak.upgrade() else { return };
        if response == "leave" {
            shell.ctx.hub.send(Command::PartyLeave);
            shell.left_party_to_close();
        }
    });
    dialog.present(Some(&shell.ctx.window));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(name: &str, ready: Option<bool>, you: bool) -> PartyMember {
        PartyMember {
            name: name.into(),
            ready,
            file: Some("Stardust Wayfarer · Episode 7".into()),
            duration: Some(1440.0),
            file_matches: Some(true),
            you,
        }
    }

    #[test]
    fn the_ready_line_names_who_the_room_waits_for() {
        let ada = member("Ada", Some(false), true);
        let mika = member("Mika", Some(true), false);
        let ren = member("Ren", Some(false), false);
        let kai = member("Kai", Some(false), false);
        assert_eq!(
            ready_line(std::slice::from_ref(&ada)),
            "Nobody else is here yet. Send the invite link."
        );
        assert_eq!(ready_line(&[ada.clone(), mika.clone()]), "Play starts once you're ready");
        assert_eq!(ready_line(&[mika.clone(), ren.clone()]), "Play starts once Ren is ready");
        assert_eq!(
            ready_line(&[ada.clone(), mika.clone(), ren.clone()]),
            "Play starts once you and Ren are ready"
        );
        assert_eq!(ready_line(&[ada, ren, kai]), "Play starts once 3 more are ready");
        let unknown = member("Sol", None, false);
        assert_eq!(ready_line(&[mika, unknown]), "Everyone is ready");
    }

    #[test]
    fn a_member_line_says_what_differs_from_this_device() {
        let mut other = member("Mika", Some(true), false);
        assert_eq!(member_file(&other), None);
        other.file_matches = Some(false);
        assert_eq!(member_file(&other), Some("Playing a different file"));
        other.file = None;
        other.file_matches = None;
        assert_eq!(member_file(&other), Some("Nothing open yet"));
        let mut you = member("Ada", Some(false), true);
        you.file = None;
        assert_eq!(member_file(&you), None);
        assert_eq!(member_name(&you), "Ada (you)");
        assert_eq!(member_state(&you), "Not ready");
    }

    #[test]
    fn a_server_reads_as_a_host_and_a_port() {
        for good in
            ["", "party.example", "party.example:8999", "[::1]:8999", "[::1]", "10.0.0.2:1"]
        {
            assert_eq!(server_problem(good), None, "{good}");
        }
        for bad in [
            "party.example:",
            "party.example:0",
            "party.example:99999",
            "party.example:89a",
            "a:b:c",
            "https://party.example",
            "party example",
            ":8999",
            "[::1",
            "[::1]x",
            "[::1]:",
        ] {
            assert!(server_problem(bad).is_some(), "{bad}");
        }
    }

    #[test]
    fn the_server_hint_says_whose_the_suggestion_is() {
        let form = PartyForm {
            recent_servers: vec![],
            suggested_server: "syncplay.pl:8995".into(),
            suggested_run_by: "the Syncplay project".into(),
            room: "quiet-harbor-k7m2xq9ad4tnwe3p".into(),
            name: "Ada".into(),
        };
        assert_eq!(
            server_hint(" SyncPlay.pl:8995 ", &form),
            "A public server run by the Syncplay project, not by anistream."
        );
        assert_eq!(
            server_hint("", &form),
            "No server of your own? syncplay.pl:8995 is a public one, run by the Syncplay project."
        );
    }

    #[test]
    fn the_connection_line_says_how_it_stands() {
        let mut party = PartyView {
            server: "party.example:8999".into(),
            room: "quiet-harbor".into(),
            name: "Ada".into(),
            connection: PartyConnection::Connected,
            encrypted: true,
            ready: false,
            episode: None,
            invite: None,
            motd: None,
            latency_ms: Some(42),
        };
        assert_eq!(connection_line(&party), "Connected, encrypted · 42 ms");
        party.encrypted = false;
        party.latency_ms = None;
        assert_eq!(connection_line(&party), "Connected, not encrypted");
        party.connection = PartyConnection::Reconnecting { attempt: 2, of: 6 };
        assert_eq!(connection_line(&party), "Connection lost. Trying again, 2 of 6");
    }
}
