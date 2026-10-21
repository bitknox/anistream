//! mpv's child process as a playback's player: each call the command it has always been, over
//! the socket, and the overlay told to the companion script.

use anistream_player::{MpvSession, PlayerError, companion, protocol::Command};
use async_trait::async_trait;

use super::{LinkError, Overlay, PlayerLink};

impl From<PlayerError> for LinkError {
    fn from(error: PlayerError) -> Self {
        Self(error.to_string())
    }
}

#[async_trait]
impl PlayerLink for MpvSession {
    async fn load(&self, url: &str, options: Vec<(String, String)>) -> Result<(), LinkError> {
        tracing::info!(%url, ?options, "loading into mpv");
        let index_slot = self.loadfile_index();
        Ok(self.send(Command::LoadFile { url: url.to_owned(), options, index_slot }).await?)
    }

    async fn add_subtitle(
        &self,
        url: &str,
        title: Option<String>,
        language: Option<String>,
    ) -> Result<(), LinkError> {
        Ok(self.send(Command::SubAdd { url: url.to_owned(), title, language }).await?)
    }

    async fn set(
        &self,
        property: &'static str,
        value: serde_json::Value,
    ) -> Result<(), LinkError> {
        Ok(self.send(Command::SetProperty(property, value)).await?)
    }

    async fn cycle(&self, property: &'static str) -> Result<(), LinkError> {
        Ok(self.send(Command::Cycle(property)).await?)
    }

    async fn add(&self, property: &'static str, delta: f64) -> Result<(), LinkError> {
        Ok(self.send(Command::Add(property, delta)).await?)
    }

    async fn seek(&self, seconds: f64, absolute: bool) -> Result<(), LinkError> {
        Ok(self.send(Command::Seek { seconds, absolute }).await?)
    }

    async fn shaders(&self, paths: &[String]) -> Result<(), LinkError> {
        Ok(self.set_shaders(paths).await?)
    }

    async fn audio_filter(&self, add: bool, filter: &str) -> Result<(), LinkError> {
        Ok(self.send(Command::AudioFilter { add, filter: filter.to_owned() }).await?)
    }

    async fn restate_chapters(&self) -> Result<(), LinkError> {
        Ok(MpvSession::restate_chapters(self).await?)
    }

    async fn overlay(&self, overlay: Overlay) -> Result<(), LinkError> {
        let command = match overlay {
            Overlay::Status(status) => companion::state("status", &status),
            Overlay::Episode(episode) => companion::state("episode", &episode),
            Overlay::Next(Some(next)) => companion::state("next", &next),
            Overlay::Next(None) => companion::clear("next"),
            Overlay::Menus(menus) => companion::state("menus", &menus),
            Overlay::Media(media) => companion::state("media", &media),
            Overlay::ThumbnailUrl(Some(url)) => companion::state("thumbnail-url", &url),
            Overlay::ThumbnailUrl(None) => companion::clear("thumbnail-url"),
            Overlay::SkipOffer { kind, to } => {
                companion::skip_offer(kind.label(), kind.label(), to)
            }
            Overlay::SkipCleared => companion::skip_clear(),
            Overlay::Skipped { kind, from, to } => {
                companion::skipped(kind.label(), kind.label(), from, to)
            }
            Overlay::Toast { text, level } => companion::toast(&text, level),
            Overlay::Stall { seconds } => companion::stall(seconds),
            Overlay::StallCleared => companion::stall_clear(),
        };
        Ok(self.send(command).await?)
    }

    async fn stop(&self) -> Result<(), LinkError> {
        Ok(MpvSession::stop(self).await?)
    }

    async fn quit(&self) -> Result<(), LinkError> {
        Ok(MpvSession::quit(self).await?)
    }

    fn is_alive(&self) -> bool {
        MpvSession::is_alive(self)
    }

    fn has_quit(&self) -> bool {
        MpvSession::has_quit(self)
    }

    async fn diagnostics(&self) -> Option<String> {
        MpvSession::diagnostics(self).most_relevant().await
    }

    async fn shutdown(self: Box<Self>) {
        MpvSession::shutdown(*self).await;
    }
}
