use rubato::ResampleError;
use thiserror::Error;
#[derive(Error, Debug)]
pub enum Error {
    #[error("Std Io Error!")]
    StdIo(#[from] std::io::Error),
    #[error("Did not find default input device!")]
    FailedToFindDefaultInputDevice,
    #[error("Cpal default stream config error!")]
    CpalDefaultStreamConfig(#[from] cpal::DefaultStreamConfigError),
    #[error("Cpal build stream error!")]
    CpalBuildStreamError(#[from] cpal::BuildStreamError),
    #[error("Cpal play stream error!")]
    CpalPlayStreamError(#[from] cpal::PlayStreamError),
    #[error("Rubatu resample error!")]
    RubatuResample(#[from] ResampleError),
    #[error("Aviation Helper")]
    AviationHelper(#[from] aviation_helper_rs::errors::Error),
    #[error("Whisper error: {0}")]
    WhisperError(String),
}
