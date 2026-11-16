pub mod birthday;
pub mod settings;

/// Errors that can occur when performing command actions.
#[derive(Debug)]
pub enum CabbotCommandError {
    InvalidUser,
    InvalidDate,
    InvalidTextChannel,
    WrongChannel(u64),
    MissingOption,
    UnknownAction,
    InsufficientPermissions,
    LookupFailed(String),
    ResponseFailed,
    SaveFailed(String),
    #[allow(dead_code)]
    Unknown,
}

impl CabbotCommandError {
    fn response_failed(error: serenity::Error) -> Self {
        eprintln!("Failed to send command response: {error}");
        Self::ResponseFailed
    }

    fn save_failed(thing: &str, error: std::io::Error) -> Self {
        eprintln!("Failed to save {thing}: {error}");
        Self::SaveFailed(thing.to_string())
    }
}

impl std::fmt::Display for CabbotCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CabbotCommandError::InvalidUser => write!(f, "I don't know who that user is..."),
            CabbotCommandError::InvalidDate => {
                write!(f, "That doesn't seem right... make sure it's a valid date!")
            }
            CabbotCommandError::InvalidTextChannel => {
                write!(f, "I don't think that's a channel in this server...")
            }
            CabbotCommandError::MissingOption => write!(f, "You forgot a required field!"),
            CabbotCommandError::UnknownAction => {
                write!(f, "I don't know what you're trying to make me do!")
            }
            CabbotCommandError::WrongChannel(channel) => {
                write!(f, "This command can only be used in <#{channel}>.")
            }
            CabbotCommandError::InsufficientPermissions => {
                write!(f, "You need administrator permission to run this command.")
            }
            CabbotCommandError::LookupFailed(thing) => {
                write!(f, "Failed to look up {thing}")
            }
            CabbotCommandError::ResponseFailed => {
                write!(
                    f,
                    "Failed to send command response. I guess I can send this one though?"
                )
            }
            CabbotCommandError::SaveFailed(thing) => {
                write!(f, "Failed to save {thing}.")
            }
            CabbotCommandError::Unknown => {
                write!(f, "An unknown error occurred.")
            }
        }
    }
}
