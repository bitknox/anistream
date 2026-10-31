//! Reset subtitle style: every subtitle setting back to its default, in one go.

use anistream_model::{
    Msg,
    event::Event,
    notice::{Notice, NoticeHint},
    settings,
};

use crate::{outbox::Outbox, services::Services, state::EngineState};

/// Put each subtitle setting back to its default and write it. The notice offers Undo: the app
/// holds the style it had and sends each setting back.
pub fn reset(engine: &mut EngineState, services: &Services, tx: &Outbox) {
    // Quietly: the one notice below stands for all of them.
    let mut failed = None;
    for (key, value) in settings::subtitle_defaults() {
        let Some((table, file_key)) = key.path() else { continue };
        if !settings::put(engine.config.effective_mut(), key, &value) {
            continue;
        }
        if let Err(error) = engine.config.write(&services.paths, table, file_key, value) {
            failed = Some(error.to_string());
            break;
        }
    }
    let _ = tx.send(Event::Settings(engine.config.view()));
    let _ = tx.internal(crate::outbox::Internal::SettingsSaved);
    if let Some(reason) = failed {
        let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeNotSaved { reason })));
        return;
    }
    let notice =
        Notice::info(Msg::SettingsSubsResetDone).with_hint(NoticeHint::UndoSubtitleReset);
    let _ = tx.send(Event::Toast(notice));
}
