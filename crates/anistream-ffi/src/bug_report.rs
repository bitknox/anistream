//! "Report a bug" for the native apps: the same address the terminal builds.

use anistream_model::bug_report::{BugReport, issue_url};

/// The address of a new GitHub issue with the bug-report form's diagnostics filled in, for the
/// app to open in the browser. Nothing in it names a source, an address or a file: see
/// [`anistream_model::bug_report`]. At most 7,500 bytes.
#[uniffi::export]
pub fn bug_report_url(report: BugReport) -> String {
    issue_url(&report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_apps_get_the_models_address() {
        let report = BugReport { app_version: "2.0.0".into(), ..BugReport::default() };
        assert_eq!(bug_report_url(report.clone()), issue_url(&report));
        assert!(
            bug_report_url(report)
                .starts_with("https://github.com/bitknox/anistream/issues/new?")
        );
    }
}
