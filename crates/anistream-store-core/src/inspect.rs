//! Asking a plugin what it is.
//!
//! Where a WebAssembly runtime is available — the reference server, the app — an [`Inspector`]
//! loads the plugin with no capabilities and asks. Where none is, a store reads the manifest the
//! plugin carries instead (see [`crate::manifest`]).

pub use crate::manifest::PluginInfo;
use crate::{
    MaybeSendSync,
    protocol::{IndexEntry, host_set},
};

#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait Inspector: MaybeSendSync {
    /// Load `wasm` with no capabilities at all and return its manifest.
    ///
    /// An error is a plugin that does not load, said in words a person can act on.
    async fn inspect(&self, wasm: &[u8]) -> Result<PluginInfo, String>;
}

/// Where a plugin's own description and a listing disagree, if they do.
///
/// The listing is what the user was shown and agreed to. A plugin that declares a different id,
/// version or set of hosts is not the thing they agreed to, whatever the cause.
pub fn mismatch(info: &PluginInfo, entry: &IndexEntry) -> Option<String> {
    differs(info, &entry.id, &entry.version, &entry.allowed_hosts)
}

/// Where two descriptions of the same plugin disagree, if they do.
pub fn differs(info: &PluginInfo, id: &str, version: &str, hosts: &[String]) -> Option<String> {
    if info.id != id {
        return Some(format!("the plugin calls itself {} but the store lists {id}", info.id));
    }
    if info.version != version {
        return Some(format!("{id} is version {} but the store lists {version}", info.version));
    }
    if host_set(&info.allowed_hosts) != host_set(hosts) {
        return Some(format!(
            "{id} asks to contact {} but the store lists {}",
            describe_hosts(&info.allowed_hosts),
            describe_hosts(hosts)
        ));
    }
    None
}

fn describe_hosts(hosts: &[String]) -> String {
    if hosts.is_empty() { "nothing".into() } else { hosts.join(", ") }
}

#[cfg(any(test, feature = "testing"))]
pub mod fake {
    //! An inspector that reads a manifest out of a JSON "plugin", for tests.

    use super::*;

    /// Bytes standing in for a component: the JSON of a manifest.
    pub fn plugin(id: &str, version: &str, hosts: &[&str]) -> Vec<u8> {
        serde_json::json!({
            "id": id,
            "name": format!("{id} plugin"),
            "version": version,
            "hosts": hosts,
        })
        .to_string()
        .into_bytes()
    }

    pub struct FakeInspector;

    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    impl Inspector for FakeInspector {
        async fn inspect(&self, wasm: &[u8]) -> Result<PluginInfo, String> {
            let value: serde_json::Value =
                serde_json::from_slice(wasm).map_err(|_| "not a plugin".to_owned())?;
            let text = |key: &str| value[key].as_str().unwrap_or_default().to_owned();
            Ok(PluginInfo {
                id: text("id"),
                name: text("name"),
                version: text("version"),
                allowed_hosts: value["hosts"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|h| h.as_str().map(str::to_owned)).collect())
                    .unwrap_or_default(),
                translations: vec!["sub".into()],
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(hosts: &[&str]) -> IndexEntry {
        IndexEntry {
            id: "example".into(),
            name: "Example".into(),
            version: "1.0.0".into(),
            allowed_hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
            translations: Vec::new(),
            sha256: String::new(),
            size: 0,
            published_at: 0,
        }
    }

    fn info(id: &str, version: &str, hosts: &[&str]) -> PluginInfo {
        PluginInfo {
            id: id.into(),
            name: String::new(),
            version: version.into(),
            allowed_hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
            translations: Vec::new(),
        }
    }

    #[test]
    fn a_plugin_matching_its_listing_passes() {
        let listed = entry(&["api.example", "cdn.example"]);
        assert_eq!(
            mismatch(&info("example", "1.0.0", &["CDN.example", "api.example"]), &listed),
            None
        );
    }

    #[test]
    fn an_extra_host_is_a_mismatch() {
        let listed = entry(&["api.example"]);
        let found =
            mismatch(&info("example", "1.0.0", &["api.example", "else.example"]), &listed);
        assert!(found.unwrap().contains("else.example"));
    }

    #[test]
    fn a_different_id_or_version_is_a_mismatch() {
        let listed = entry(&[]);
        assert!(mismatch(&info("other", "1.0.0", &[]), &listed).is_some());
        assert!(mismatch(&info("example", "1.0.1", &[]), &listed).is_some());
    }
}
