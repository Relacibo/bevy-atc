use thiserror::Error;
#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid direction: {}",.0)]
    InvalidDirection(u32),
    #[error("Invalid altitute: {}",.0)]
    InvalidAltitute(i32),
    #[error("Invalid turn: {}",.0)]
    InvalidTurn(u32),
    #[error("Invalid frequency: {}",.0)]
    InvalidFrequency(String),
    #[error("Serde Json (de)serialization failed!")]
    SerdeDeserialize(#[from] serde_json::Error),
    #[error("Std Io Error!")]
    StdIo(#[from] std::io::Error),
    #[error("Did not find default input device!")]
    FailedToFindDefaultInputDevice,
}
