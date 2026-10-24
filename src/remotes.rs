//! Configured remotes, moved from gwz-core's `Git2Backend::remotes` by step 5.3
//! (row G2).

use crate::error::Error;
use crate::repository::Repository;

/// A configured remote.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Remote {
    pub name: String,
    pub url: String,
    /// The separate push URL, when one is configured.
    pub push_url: Option<String>,
}

impl Repository {
    /// The configured remotes, in the order the configuration lists them.
    pub fn remotes(&self) -> Result<Vec<Remote>, Error> {
        let names = self.native.remotes().map_err(Error::state_read_native)?;
        let mut remotes = Vec::new();
        for name in names.iter() {
            let Some(name) = name.map_err(Error::state_read_native)? else {
                continue;
            };
            let remote = self
                .native
                .find_remote(name)
                .map_err(Error::state_read_native)?;
            remotes.push(Remote {
                name: name.to_owned(),
                url: remote.url().map_err(Error::state_read_native)?.to_owned(),
                push_url: remote
                    .pushurl()
                    .map_err(Error::state_read_native)?
                    .map(ToOwned::to_owned),
            });
        }
        Ok(remotes)
    }
}
