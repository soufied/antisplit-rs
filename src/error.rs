use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Zip(zip::result::ZipError),
    NoSplitEntries,
    NoBaseModule,
    AlreadyStandalone,
    Incompatible(String),
    Malformed(String),
    Verification(String),
    Unsupported(String),
    Tool(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Zip(error) => write!(formatter, "invalid ZIP container: {error}"),
            Self::NoSplitEntries => {
                formatter.write_str("no .apk entries were found in this archive")
            }
            Self::NoBaseModule => formatter.write_str(
                "no base module found: every APK is named like a config or split module",
            ),
            Self::AlreadyStandalone => {
                formatter.write_str("this file is already a single APK and needs no merging")
            }
            Self::Incompatible(message) => write!(formatter, "incompatible splits: {message}"),
            Self::Malformed(message) => write!(formatter, "malformed binary XML: {message}"),
            Self::Unsupported(message) => {
                write!(formatter, "unsupported resource data: {message}")
            }
            Self::Tool(message) => write!(formatter, "external tool failed: {message}"),
            Self::Verification(message) => {
                write!(formatter, "patch verification failed: {message}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Zip(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Zip(error)
    }
}
