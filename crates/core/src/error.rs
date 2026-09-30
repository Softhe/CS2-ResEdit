use std::fmt;
use std::io;

/// Errors produced by the CS2 ResEdit core library.
#[derive(Debug)]
pub enum CoreError {
    /// Configuration content is structurally invalid (mirrors InvalidDataException).
    InvalidData(String),
    /// The configuration differs from the snapshot loaded by the caller.
    ExternalChange(String),
    /// A resolution or argument value is invalid (mirrors ArgumentException).
    InvalidArgument(String),
    /// An argument value is out of range (mirrors ArgumentOutOfRangeException).
    OutOfRange(String),
    /// A backup selection is not recognized for the configuration.
    NotABackup(String),
    /// Wrapped I/O error.
    Io(io::Error),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(msg) => write!(f, "{msg}"),
            Self::ExternalChange(msg) => write!(f, "{msg}"),
            Self::InvalidArgument(msg) => write!(f, "{msg}"),
            Self::OutOfRange(msg) => write!(f, "{msg}"),
            Self::NotABackup(msg) => write!(f, "{msg}"),
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<io::Error> for CoreError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl CoreError {
    /// Prepends a stage description to I/O errors so failures surface as
    /// e.g. "Creating the backup failed: ...". Domain errors already carry
    /// actionable text and are returned unchanged.
    pub fn context(self, prefix: &str) -> Self {
        match self {
            Self::Io(err) => Self::Io(io::Error::new(err.kind(), format!("{prefix}: {err}"))),
            other => other,
        }
    }

    /// Category name used in privacy-safe diagnostics. Mirrors the C#
    /// implementation, which reports `ex.GetType().Name`.
    pub fn category(&self) -> &'static str {
        match self {
            Self::InvalidData(_) => "InvalidDataException",
            Self::ExternalChange(_) => "InvalidDataException",
            Self::InvalidArgument(_) => "ArgumentException",
            Self::OutOfRange(_) => "ArgumentOutOfRangeException",
            Self::NotABackup(_) => "InvalidOperationException",
            Self::Io(_) => "IOException",
        }
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
