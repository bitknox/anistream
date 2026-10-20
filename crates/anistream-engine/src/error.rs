//! Why an engine did not start.

/// Why [`crate::engine::Engine::start`] could not stand the engine up.
///
/// One enum for every way a start can fail, so a front end has one thing to match on, and one
/// error type to hand across the FFI. Each variant says, in words, what failed: what the app
/// says when it does not start.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The plugin stores could not be set up: their client could not be built.
    #[error("{reason}")]
    PluginStores { reason: String },
}
