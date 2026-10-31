//! Crash reports, demo facts, QR codes and subtitle options for the native apps.
//!
//! Saving a crash is off until the viewer turns on Save crash reports: until then every call
//! here that would write answers false and every list is empty. Nothing is sent from here.

use anistream_engine::crash;
use anistream_model::crash::{CrashRecord, CrashSummary};
use anistream_model::qr::{QrMatrix, qr_half_blocks as half_blocks, qr_matrix as matrix};
use anistream_model::sources::{DemoInfo, demo_info as info};
use anistream_model::subtitle_style::{
    MpvOption, SubtitleStyle, SystemCaptionStyle, mpv_subtitle_options as mpv_options,
    terminal_subtitle_options as terminal_options,
};

/// The QR code for `text`: dark modules and the quiet zone, for the app to draw. Empty when the
/// text is too long.
#[uniffi::export]
pub fn qr_matrix(text: String) -> QrMatrix {
    matrix(&text)
}

/// The same code in half blocks for a terminal, one string per line.
#[uniffi::export]
pub fn qr_half_blocks(text: String) -> Vec<String> {
    half_blocks(&matrix(&text))
}

/// Who made the demo, its licence and where it is hosted, for About the demo.
#[uniffi::export]
pub fn demo_info() -> DemoInfo {
    info()
}

/// The player options that draw subtitles in `style`. `system` is the platform's caption style
/// and `controls_lift` the percent of the picture's height the controls take.
#[uniffi::export]
pub fn mpv_subtitle_options(
    style: SubtitleStyle,
    system: Option<SystemCaptionStyle>,
    controls_lift: f64,
) -> Vec<MpvOption> {
    mpv_options(&style, system.as_ref(), controls_lift)
}

/// The options for the terminal's player, which has no system caption style.
#[uniffi::export]
pub fn terminal_subtitle_options(style: SubtitleStyle) -> Vec<MpvOption> {
    terminal_options(&style)
}

/// Keep a crash the app caught itself, redacted. False when saving is off or nothing was written.
#[uniffi::export]
pub fn crash_save(record: CrashRecord) -> bool {
    crash::save_record(&record)
}

/// The saved reports, newest first.
#[uniffi::export]
pub fn crash_list() -> Vec<CrashSummary> {
    crash::saved()
}

/// The whole text of one saved report, for Copy report.
#[uniffi::export]
pub fn crash_text(id: String) -> Option<String> {
    crash::saved_text(&id)
}

/// The redacted summary of one saved report: what a public issue carries.
#[uniffi::export]
pub fn crash_summary_text(id: String) -> Option<String> {
    crash::saved_summary_text(&id)
}

#[uniffi::export]
pub fn crash_delete(id: String) {
    crash::delete_saved(&id);
}

#[uniffi::export]
pub fn crash_delete_all() {
    crash::delete_all_saved();
}

/// The crash to mention at launch, once: only while saving and asking are both on.
#[uniffi::export]
pub fn crash_pending_notice() -> Option<CrashSummary> {
    crash::pending_notice()
}

/// Offer the records the system kept itself, from the week before saving was switched on. The
/// app passes what it read from the system; nothing is saved until `crash_add_held`.
#[uniffi::export]
pub fn crash_offer_held(records: Vec<CrashRecord>) -> Vec<CrashSummary> {
    crash::offer_held(records)
}

/// Save one offered record, redacted, like any other.
#[uniffi::export]
pub fn crash_add_held(id: String) -> bool {
    crash::add_held(&id)
}
