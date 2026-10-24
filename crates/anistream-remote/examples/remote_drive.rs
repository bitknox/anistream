//! A phone's side of the remote, typed line by line, to drive a TV by hand: pair with it, then
//! press its keys, type into its search and step its volume. One identity for the whole run.
//!
//! ```text
//! cargo run -p anistream-remote --example remote_drive -- 127.0.0.1:47811
//! request            ask the TV to show a pairing code
//! pair 042917        pair with the code on the TV
//! key down           up, down, left, right, select, back, home
//! text Kiriya [go]   the search field's whole text; "go" submits it
//! volume 5           step the TV's volume, + or -
//! open 1001          open a title by its AniList id
//! pause              play or pause
//! ```

use std::{io::BufRead, net::SocketAddr};

use anistream_core::ids::AnilistId;
use anistream_remote::{
    client::{Hello, RemoteClient},
    command::{Key, RemoteCommand},
    identity::Identity,
};

// Lines are read blocking on one worker while the client reads the TV on another.
#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = std::env::args().nth(1).ok_or("the TV's address, host:port")?.parse()?;
    let phone = Identity::generate()?;
    let hello = Hello { name: "Test phone".into(), app: "remote_drive/0".into() };
    let mut client = RemoteClient::connect(addr, &phone, None, &hello).await?;
    println!("connected to {}", client.tv_name());

    for line in std::io::stdin().lock().lines() {
        let line = line?;
        let words: Vec<&str> = line.split_whitespace().collect();
        let answer = match words.as_slice() {
            [] => continue,
            ["request"] => client.request_code().await.map(|()| "asked for a code".to_owned()),
            ["pair", code] => client.pair(code).await.map(|tv| format!("paired with {tv:?}")),
            ["key", key] => {
                let key = match *key {
                    "up" => Key::Up,
                    "down" => Key::Down,
                    "left" => Key::Left,
                    "right" => Key::Right,
                    "select" => Key::Select,
                    "back" => Key::Back,
                    "home" => Key::Home,
                    other => {
                        println!("no key {other}");
                        continue;
                    }
                };
                client.send(RemoteCommand::Navigate { key }).await.map(|()| format!("{key:?}"))
            }
            ["text", rest @ ..] => {
                let submit = rest.last() == Some(&"go");
                let text = rest[..rest.len() - usize::from(submit)].join(" ");
                client.send(RemoteCommand::Text { text: text.clone(), submit }).await.map(|()| text)
            }
            ["volume", step] => {
                let step: f64 = step.parse()?;
                client.send(RemoteCommand::Volume { step }).await.map(|()| format!("volume {step}"))
            }
            ["open", id] => {
                let id = AnilistId::new(id.parse()?);
                client.send(RemoteCommand::Open { id }).await.map(|()| "opened".to_owned())
            }
            ["pause"] => client.send(RemoteCommand::TogglePause).await.map(|()| "pause".to_owned()),
            ["sleep", seconds] => {
                tokio::time::sleep(std::time::Duration::from_secs_f64(seconds.parse()?)).await;
                continue;
            }
            _ => {
                println!("not understood: {line}");
                continue;
            }
        };
        match answer {
            Ok(done) => println!("{done}"),
            Err(error) => println!("refused: {error}"),
        }
    }
    Ok(())
}
