use rubato::ResampleError;
use thiserror::Error;
#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid direction!")]
    InvalidDirection(u32),
    #[error("Invalid altitute!")]
    InvalidAltitute(i32),
    #[error("Invalid turn!")]
    InvalidTurn(u32),
    #[error("Serde Json (de)serialization failed!")]
    SerdeDeserialize(#[from] serde_json::Error),
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
}
