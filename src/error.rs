use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ErrorKind {
    InvalidObjectId,
    ObjectFormatMismatch,
    RepositoryOpen,
    NotARepository,
    ObjectRead,
    /// A commit or tag operation was refused or failed: the message says why,
    /// and native diagnostics are present when libgit2 reported the failure.
    Operation,
    /// A path argument is not a normalized repository-relative path (it is
    /// absolute, empty, or has a `.`, `..` or root component). No native call
    /// was made.
    InvalidPath,
    /// Reading repository state that is neither an object nor a reference
    /// failed: the status of the worktree, the configured remotes, or the
    /// `HEAD` file of an unborn branch.
    StateRead,
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

    pub(crate) fn invalid_path(message: impl Into<String>) -> Self {
        Self::plain(ErrorKind::InvalidPath, message)
    }

    /// A state read that failed for a reason this crate found.
    pub(crate) fn state_read(message: impl Into<String>) -> Self {
        Self::plain(ErrorKind::StateRead, message)
    }

    /// A reference, revision, object or tree read that libgit2 failed.
    pub(crate) fn object_read(error: git2::Error) -> Self {
        Self::from_native(ErrorKind::ObjectRead, error)
    }

    /// A status or configuration read that libgit2 failed.
    pub(crate) fn state_read_native(error: git2::Error) -> Self {
        Self::from_native(ErrorKind::StateRead, error)
    }

    /// A plain description of an object read this crate refused (a path that
    /// names a directory where a file was asked for).
    pub(crate) fn object_read_plain(message: impl Into<String>) -> Self {
        Self::plain(ErrorKind::ObjectRead, message)
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

    /// A commit or tag operation that failed for a reason this crate found.
    pub(crate) fn operation(message: impl Into<String>) -> Self {
        Self::plain(ErrorKind::Operation, message)
    }

    /// A commit or tag operation that libgit2 failed.
    pub(crate) fn operation_native(error: git2::Error) -> Self {
        Self::from_native(ErrorKind::Operation, error)
    }

    /// A verification read that failed after the write it checks: an
    /// `Operation` error whose text says the write happened. The native
    /// diagnostics of the failed read are kept.
    pub(crate) fn written_but_unverified(what: &str, error: Error) -> Self {
        Self {
            kind: ErrorKind::Operation,
            message: format!(
                "{what}: written; verification read failed: {}",
                error.message
            ),
            native: error.native,
        }
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
