//! Updates. Outside Flatpak the engine checks GitHub's latest release once a day and when asked,
//! and a newer one is told once, with its page to open: there is no Linux file to install from
//! here. GitHub's latest release is never a pre-release, so the `deps-libmpv` builds are never
//! offered. Inside Flatpak, Flatpak updates the app, from Software or `flatpak update`: the
//! check still says a version is out in About, and offers nothing to install.

use anistream_model::updates::UpdateStatus;

/// Whether this copy runs inside Flatpak, which then owns its updates.
pub fn flatpak() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

/// Where the check stands, as the Updates row says it under "anistream 1.0".
pub fn words(status: Option<&UpdateStatus>, offline: bool, flatpak: bool) -> String {
    match status {
        Some(UpdateStatus::Checking) => "Checking for a newer version…".into(),
        Some(UpdateStatus::UpToDate) => "Up to date".into(),
        Some(UpdateStatus::Available { release, .. }) if flatpak => {
            format!("Version {} is out. Software installs it.", release.version)
        }
        Some(UpdateStatus::Available { release, .. }) => {
            format!("Version {} is out", release.version)
        }
        Some(UpdateStatus::Failed { reason }) => {
            format!("Couldn't check: {}", crate::catalog::shown(reason))
        }
        None if flatpak => "Flatpak keeps anistream up to date".into(),
        None if offline => "Checks once you're back online".into(),
        None => "Not checked yet".into(),
    }
}

/// The release page to open, while a newer version is out.
pub fn page(status: Option<&UpdateStatus>) -> Option<&str> {
    match status {
        Some(UpdateStatus::Available { release, .. }) => Some(&release.page_url),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::updates::Release;

    fn available() -> UpdateStatus {
        UpdateStatus::Available {
            release: Box::new(Release {
                version: "1.4.0".into(),
                published_at: 0,
                page_url: "https://example.test/releases/v1.4.0".into(),
                notes: String::new(),
                macos: None,
                ios: None,
                android: None,
                android_tv: None,
                windows_x64: None,
                windows_arm64: None,
                linux_x86_64: None,
                linux_aarch64: None,
            }),
            notify: true,
        }
    }

    #[test]
    fn a_newer_version_is_said_with_its_page_and_flatpak_says_who_installs_it() {
        let status = available();
        assert_eq!(words(Some(&status), false, false), "Version 1.4.0 is out");
        assert_eq!(
            words(Some(&status), false, true),
            "Version 1.4.0 is out. Software installs it."
        );
        assert_eq!(page(Some(&status)), Some("https://example.test/releases/v1.4.0"));
        assert_eq!(page(Some(&UpdateStatus::UpToDate)), None);
    }

    #[test]
    fn before_a_check_the_row_says_why_there_is_none() {
        assert_eq!(words(None, true, false), "Checks once you're back online");
        assert_eq!(words(None, false, true), "Flatpak keeps anistream up to date");
        let failed = UpdateStatus::Failed { reason: "no answer".into() };
        assert_eq!(words(Some(&failed), false, false), "Couldn't check: No answer");
    }
}
