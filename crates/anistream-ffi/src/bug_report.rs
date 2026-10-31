//! "Report a bug" for the native apps: the same address the terminal builds.

use anistream_model::bug_report::{
    BugReport, issue_url_capped, issue_url_with, report_text as text,
};
use anistream_model::crash::CrashRecord;

/// The address of a new GitHub issue with the bug-report form's diagnostics filled in, for the
/// app to open in the browser. Nothing in it names a source, an address or a file: see
/// [`anistream_model::bug_report`]. At most 7,500 bytes.
#[uniffi::export]
pub fn bug_report_url(report: BugReport) -> String {
    let crashes = crashes_of(&report);
    issue_url_with(&report, &crashes)
}

/// [`bug_report_url`] cut to `max_bytes`: 900 for a code on a TV's screen.
#[uniffi::export]
pub fn bug_report_url_capped(report: BugReport, max_bytes: u32) -> String {
    let crashes = crashes_of(&report);
    issue_url_capped(&report, &crashes, max_bytes as usize)
}

/// The whole report as text, with the full crashes, for Copy report.
#[uniffi::export]
pub fn report_text(report: BugReport) -> String {
    let crashes = crashes_of(&report);
    text(&report, &crashes)
}

/// The saved crashes the report names; an id that is gone is left out.
fn crashes_of(report: &BugReport) -> Vec<CrashRecord> {
    report
        .crashes
        .iter()
        .filter_map(|id| anistream_engine::crash::armed_store()?.get(id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_apps_get_the_models_address() {
        let report = BugReport { app_version: "2.0.0".into(), ..BugReport::default() };
        assert_eq!(
            bug_report_url(report.clone()),
            anistream_model::bug_report::issue_url(&report)
        );
        assert!(
            bug_report_url(report)
                .starts_with("https://github.com/bitknox/anistream/issues/new?")
        );
    }
}
