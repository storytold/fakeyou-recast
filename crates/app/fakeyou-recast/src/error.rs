use std::path::PathBuf;
use rust_i18n::t;

#[derive(Debug)]
pub enum AppError {
    NoDevice(String),
    NoFile(String),
    NoCuda,
    StreamConfigError(cpal::DefaultStreamConfigError),
    BuildInputStreamError(cpal::BuildStreamError, String),
    BuildOutputStreamError(cpal::BuildStreamError, String),
    PlayStreamError(cpal::PlayStreamError, String),
    ChannelRecvError,
    ChannelSendError,
    IoError(std::io::Error),
    GenericFormatError,
    HoundError(hound::Error),
    LockError,
    ModelLoadError,
    NoModel,
    ModelError(tch::TchError),
    FileDialogError(PathBuf),
    ResamplerConstructionError(rubato::ResamplerConstructionError),
    ResampleError(rubato::ResampleError)
}

impl core::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::NoDevice(inout) => f.write_str(t!("log.error.no_device", io = inout).as_str()),
            AppError::NoFile(inout) => f.write_str(t!("log.error.no_file", io = inout).as_str()),
            AppError::NoCuda => f.write_str(t!("log.error.no_cuda").as_str()),
            AppError::StreamConfigError(e) => f.write_fmt(format_args!("{}", e)),
            AppError::BuildInputStreamError(_e, bufsize) => f.write_str(t!("log.error.build_input_stream", bufsz = bufsize).as_str()),
            AppError::BuildOutputStreamError(_e, bufsize) => f.write_str(t!("log.error.build_output_stream", bufsz = bufsize).as_str()),
            AppError::PlayStreamError(_e, inout) => f.write_str(t!("log.error.play_stream", io = inout).as_str()),
            AppError::ChannelRecvError => f.write_str(t!("log.error.channel_recv").as_str()),
            AppError::ChannelSendError => f.write_str(t!("log.error.channel_send").as_str()),
            AppError::IoError(e) => f.write_str(t!("log.error.io", error = e.to_string().as_str()).as_str()),
            AppError::GenericFormatError => f.write_str(t!("log.error.generic_format").as_str()),
            AppError::HoundError(e) => f.write_str(t!("log.error.hound", error = e.to_string().as_str()).as_str()),
            AppError::LockError =>  f.write_str(t!("log.error.lock").as_str()),
            AppError::ModelLoadError => f.write_str(t!("log.error.model_load").as_str()),
            AppError::NoModel => f.write_str(t!("log.error.no_model").as_str()),
            AppError::ModelError(e) => f.write_str(t!("log.error.model_error").as_str()),
            AppError::FileDialogError(p) => f.write_str(t!("log.error.filedlg", path = p.to_str().unwrap_or_default()).as_str()),
            AppError::IoError(e) => f.write_str(t!("log.error.io", error = e.to_string().as_str()).as_str()),
            AppError::ResamplerConstructionError(e) => f.write_str(t!("log.error.resampler_construction", error = e.to_string().as_str()).as_str()),
            AppError::ResampleError(e) => f.write_str(t!("log.error.resample", error = e.to_string().as_str()).as_str()),

        }
    }
}

impl std::error::Error for AppError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    InvalidLoginCredentials,
    InvalidSession,
    ServerError,
    UnknownError(String),
    ReqwestError,
    ServerOverloaded,
    Parse,
}

impl core::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::InvalidLoginCredentials => f.write_str(t!("log.error.api.invalid_login_creds").as_str()),
            ApiError::InvalidSession => f.write_str(t!("log.error.api.invalid_session").as_str()),
            ApiError::ServerError => f.write_str(t!("log.error.api.server").as_str()),
            ApiError::UnknownError(e) => f.write_str(t!("log.error.api.unknown", error = e.to_string().as_str()).as_str()),
            ApiError::ReqwestError => f.write_str(t!("log.error.api.reqwest").as_str()),
            ApiError::ServerOverloaded => f.write_str(t!("log.error.api.overloaded").as_str()),
            ApiError::Parse => f.write_str(t!("log.error.api.parse").as_str()),
        }
    }
}
impl std::error::Error for ApiError {}
