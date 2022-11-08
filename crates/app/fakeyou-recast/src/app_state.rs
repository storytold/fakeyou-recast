use rust_i18n::t;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AppState {
    Recording,
    PlaySource,
    PlayTarget,
    ReadingFile,
    WritingFile,
    Realtime,
    LoadingModel,
    ModelWarmup,
    Idle,
}

impl core::fmt::Display for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recording      => { f.write_str(&t!("app_state.recording")) },
            Self::PlaySource     => { f.write_str(&t!("app_state.play_source")) },
            Self::PlayTarget     => { f.write_str(&t!("app_state.play_target")) },
            Self::ReadingFile    => { f.write_str(&t!("app_state.reading_file")) },
            Self::WritingFile    => { f.write_str(&t!("app_state.writing_file")) },
            Self::Realtime       => { f.write_str(&t!("app_state.realtime")) },
            Self::LoadingModel   => { f.write_str(&t!("app_state.loading_model")) },
            Self::ModelWarmup    => { f.write_str(&t!("app_state.model_warmup")) },
            Self::Idle           => { f.write_str("") },
        }
    }
}
