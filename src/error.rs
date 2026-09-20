use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ErrorKind {
    InvalidObjectId,
    ObjectFormatMismatch,
    RepositoryOpen,
    ObjectRead,
}

#[derive(Debug)]
pub struct NativeDiagnostic {
    pub code: i32,
    pub class: i32,
    pub message: String,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    native: Option<NativeDiagnostic>,
    message: String,
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn native(&self) -> Option<&NativeDiagnostic> {
        self.native.as_ref()
    }

    fn plain(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            native: None,
            message: message.into(),
        }
    }

    pub(crate) fn invalid_id(message: impl Into<String>) -> Self {
        Self::plain(ErrorKind::InvalidObjectId, message)
    }

    pub(crate) fn format_mismatch(message: impl Into<String>) -> Self {
        Self::plain(ErrorKind::ObjectFormatMismatch, message)
    }

    pub(crate) fn from_native(kind: ErrorKind, error: git2::Error) -> Self {
        let diagnostic = NativeDiagnostic {
            code: error.raw_code(),
            class: error.raw_class() as i32,
            message: error.message().to_owned(),
        };
        Self {
            kind,
            message: diagnostic.message.clone(),
            native: Some(diagnostic),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for Error {}
