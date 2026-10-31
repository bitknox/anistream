//! Provider registry, health tracking and failover.
//!
//! Sources are the edge of anistream that other people run: they change, move or go away. This crate
//! turns that from a crash into a visible, recoverable state. The chain is walked in preference
//! order, every failure is carried rather than discarded, and the Providers screen can always say
//! which source broke and why.

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
pub use resolve::{
    Answer, Candidate, Contribution, Gathered, Resolution, Surveyed, confirm, first_answer,
    gather, resolve, survey,
};
pub use vpn::{GuardState, KernelEnforcement, VpnGuard};
