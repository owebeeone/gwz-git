use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ErrorKind {
    InvalidObjectId,
    ObjectFormatMismatch,
    RepositoryOpen,
    NotARepository,
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

    /// The diagnostic text alone: the native message when libgit2 reported
    /// one, else the description of a failure this crate found itself. It has
    /// no kind prefix, unlike `Display`, and is not a stable match key.
    pub fn message(&self) -> &str {
        &self.message
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

    /// A failed open: libgit2's "not found" means the path is not a
    /// repository, which callers ask apart from the other open failures.
    pub(crate) fn from_open(error: git2::Error) -> Self {
        let kind = if error.code() == git2::ErrorCode::NotFound {
            ErrorKind::NotARepository
        } else {
            ErrorKind::RepositoryOpen
        };
        Self::from_native(kind, error)
    }

    pub(crate) fn from_native(kind: ErrorKind, error: git2::Error) -> Self {
        // raw_class() is unsigned on Unix and i32 on Windows: the cast is
        // needed on one platform and a no-op on the other.
        #[allow(clippy::unnecessary_cast)]
        let class = error.raw_class() as i32;
        let diagnostic = NativeDiagnostic {
            code: error.raw_code(),
            class,
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
