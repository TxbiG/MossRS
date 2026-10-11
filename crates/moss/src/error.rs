use std::fmt;

pub type Result<T> = std::result::Result<T, MossError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MossError {
    InvalidArgument,
    OutOfMemory,
    NotInitialized,
    Unsupported,
    Native(i32),
}

impl MossError {
    pub(crate) fn from_code(code: i32) -> Result<()> {
        match code {
            0 => Ok(()),
            2 => Err(Self::InvalidArgument),
            3 => Err(Self::OutOfMemory),
            4 => Err(Self::NotInitialized),
            5 => Err(Self::Unsupported),
            other => Err(Self::Native(other)),
        }
    }
}

impl fmt::Display for MossError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument => write!(f, "invalid argument"),
            Self::OutOfMemory => write!(f, "out of memory"),
            Self::NotInitialized => write!(f, "Moss is not initialized"),
            Self::Unsupported => write!(f, "operation is unsupported"),
            Self::Native(code) => write!(f, "native Moss error code {code}"),
        }
    }
}

impl std::error::Error for MossError {}
