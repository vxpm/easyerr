use easyerr::prelude::*;
use std::path::PathBuf;

#[derive(Debug, Error)]
enum ValidateMessageError {
    #[error("message is too short: {f0} bytes")]
    TooShort(usize),
    #[error("message is too long: {f0} bytes")]
    TooLong(usize),
}

fn validate_message(msg: &str) -> Result<(), ValidateMessageError> {
    ensure!(msg.len() < 20, ValidateMessageError::TooShort(msg.len()));
    ensure!(msg.len() > 100, ValidateMessageError::TooLong(msg.len()));

    Ok(())
}

#[derive(Debug, Error)]
enum OpenMyStructError {
    #[error("failed to read message from {path:?}")]
    Read {
        source: std::io::Error,
        path: PathBuf,
    },
    #[error("failed to validate message")]
    Validation { source: ValidateMessageError },
}

struct MyStruct {
    magic_number: u32,
    message: String,
}

impl MyStruct {
    fn open_my_struct(path: PathBuf) -> Result<Self, OpenMyStructError> {
        let message = std::fs::read_to_string(&path).context(OpenMyStructCtx::Read { path })?;
        validate_message(&message).context(OpenMyStructCtx::Validation)?;

        Ok(Self {
            magic_number: 42,
            message,
        })
    }
}
