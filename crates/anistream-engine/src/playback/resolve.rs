//! Finding what an episode plays from: its streams, and the context its playback records
//! history against.

use crate::playback;

/// An episode's streams and playback context, as resolution produces them.
pub type Resolved = (Vec<anistream_core::stream::Stream>, playback::PlaybackContext);
