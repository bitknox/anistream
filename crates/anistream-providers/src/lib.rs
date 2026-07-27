//! Provider registry, health tracking and failover.
//!
//! Sources are the volatile edge of anistream: they gain challenges, change their markup, or
//! disappear. This crate is what turns that from a crash into a visible, recoverable state —
//! the chain is walked in preference order, every failure is carried rather than discarded,
//! and the Providers screen can always say which source broke and why.

pub mod health;
pub mod mock;
pub mod registry;
pub mod remote;
pub mod resolve;
pub mod torrent;
pub mod vpn;

pub use health::{HealthTracker, ProviderHealth};
pub use mock::MockProvider;
pub use registry::{Attempt, ProviderRegistry};
pub use remote::RemoteHttpProvider;
pub use resolve::{Candidate, Resolution, confirm, resolve};
pub use vpn::{GuardState, KernelEnforcement, VpnGuard};
