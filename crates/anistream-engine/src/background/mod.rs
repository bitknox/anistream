//! What the engine does on its own, behind whatever is on screen: nobody asks for it and
//! nothing waits on it. The airing digest, said once at launch, and the sweeps that keep what
//! anistream writes to disk within bounds.

pub mod digest;
pub mod prune;
