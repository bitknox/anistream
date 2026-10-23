//! Links the system's libmpv when the `libmpv` feature is on: the Flatpak bundles its own, and a
//! distribution's package is found the same way.

fn main() {
    #[cfg(feature = "libmpv")]
    if let Err(e) = pkg_config::Config::new().atleast_version("2.0").probe("mpv") {
        panic!("the libmpv feature needs libmpv's development files (pkg-config mpv): {e}");
    }
}
